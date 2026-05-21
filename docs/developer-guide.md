# Developer Guide

A practical guide for developers who want to work on the Agent World codebase.
For contribution workflow (issues, PRs, commit style), see [CONTRIBUTING.md](../CONTRIBUTING.md).

---

## Repository Layout

```
agent-world/
├── world-engine/          # Rust — world state, economy, API, WAL
│   ├── src/
│   │   ├── main.rs        # Entry point: WAL init → Axum HTTP server
│   │   ├── lib.rs         # Module re-exports
│   │   ├── api.rs         # REST API routes + handlers
│   │   ├── lifecycle.rs   # LifecycleMachine: Birth→Childhood→Adulthood→Elder→Death
│   │   ├── rules.rs       # Rule engine (TokenConsumption, DeathJudgment, NewbieProtection)
│   │   ├── economy/       # Economy subsystem
│   │   │   ├── mod.rs    # Module re-exports
│   │   │   ├── task.rs    # TaskBoard + Task state machine
│   │   │   ├── escrow.rs  # Escrow manager
│   │   │   ├── reward.rs  # RewardDistributor (2% fee, XP, reputation, ledger)
│   │   │   └── token_burn.rs  # Token burn engine
│   │   ├── world/         # World core
│   │   │   ├── enums.rs   # Currency, AgentPhase, DeathReason
│   │   │   ├── event.rs   # 30+ WorldEvent variants
│   │   │   └── state.rs   # EventBus (tokio broadcast)
│   │   └── wal/           # Write-Ahead Log
│   │       ├── mod.rs     # WAL implementation (CRC32, snapshots, recovery)
│   │       └── crc.rs     # CRC32 lookup table (ISO 3309)
│   └── tests/             # Integration tests
│       ├── e2e_full_flow.rs
│       ├── wal_recovery.rs
│       ├── marketplace_integration.rs
│       └── world_engine_integration.rs
├── agent-runtime/         # Python — agent think loop
│   └── agent_runtime/
│       ├── core/          # Think loop, decide, act
│       ├── memory/        # WorkingMemory (FIFO) + ShortTermMemory (SQLite)
│       ├── survival/      # SurvivalInstinct (5 modes, 11 emergency actions)
│       ├── models/        # Pydantic models (AgentState, enums, Skill)
│       ├── llm/           # LLM providers (OpenAI, Anthropic, Ollama)
│       ├── crypto/        # Ed25519 signing, verification, nonce, registry
│       └── skills/        # SkillRegistry + 4 built-in skills
├── dashboard/             # Next.js 15 + React 19 + Tailwind 4
│   └── src/
│       ├── app/           # Pages: overview, agents, tasks, timeline
│       ├── components/    # EventStream, Leaderboard, Sidebar, StatCards
│       ├── hooks/         # useWorldState (SSE)
│       ├── lib/           # API client
│       └── types/         # TypeScript types
├── protocol/              # gRPC A2A protocol
│   └── a2a.proto          # Discover, SendMessage, StreamMessages
├── config/                # World configuration
│   ├── genesis.yaml       # World birth parameters
│   └── world-rules.yaml   # 10 rules across 4 categories
├── docs/                  # Documentation
├── scripts/               # Dev setup scripts
└── Makefile               # Common developer commands
```

---

## Development Environment Setup

### Prerequisites

Install the following tools:

```bash
# Rust (1.80+)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Python (3.11+) — uv recommended
curl -LsSf https://astral.sh/uv/install.sh | sh

# Node.js (20+)
# Use nvm, fnm, or your preferred method

# Protocol Buffers compiler
brew install protobuf  # macOS
# Ubuntu: sudo apt install -y protobuf-compiler
```

### One-command Setup

```bash
make setup
# Or manually: ./scripts/setup.sh
```

### Verify Everything Works

```bash
# Rust tests
cd world-engine && cargo test

# Python tests
cd agent-runtime && pytest

# Dashboard build
cd dashboard && npm install && npm run build
```

---

## Common Development Tasks

### Running the World Engine

```bash
cd world-engine
cargo run
# API available at http://localhost:3000
```

For auto-reload during development, use `cargo-watch`:

```bash
cargo install cargo-watch
cargo watch -x run
```

### Running the Dashboard

```bash
cd dashboard
npm install
npm run dev
# UI available at http://localhost:3001
```

The dashboard proxies `/api` requests to the world engine at `localhost:3000`
(see `next.config.ts`).

### Running Tests

```bash
# Everything
make test

# Rust only
make test-rust
# Or: cd world-engine && cargo test

# Python only
make test-python
# Or: cd agent-runtime && pytest

# Specific Rust test
cd world-engine && cargo test test_full_lifecycle

# Integration tests
make test-e2e
# Or: cd world-engine && cargo test --test e2e_full_flow
```

### Linting and Formatting

```bash
# Lint everything
make lint

# Format everything
make fmt

# Rust lint only
cd world-engine && cargo clippy

# Python lint only
cd agent-runtime && ruff check .

# Python format only
cd agent-runtime && ruff format .
```

---

## Architecture Quick Reference

### Task Lifecycle (State Machine)

Tasks follow a strict state machine. The source of truth is
`world-engine/src/economy/task.rs`:

```
published → claimed → in_progress → submitted → reviewed → completed
    │          │
    └──────────┴→ expired
```

When adding a new task endpoint or status, update:

1. `TaskStatus` enum in `task.rs`
2. `can_transition_to()` method
3. `valid_transitions()` method
4. API handler in `api.rs`
5. Router in `create_router()` / `create_router_with_wal()`
6. `TaskResponse` serialization in `api.rs`
7. Tests in `task.rs` and integration tests

### Event System

All state changes emit `WorldEvent` variants via the `EventBus` (tokio
broadcast channel). Events are defined in `world/event.rs`.

To add a new event:

1. Add the variant to `WorldEvent` enum in `event.rs`
2. Implement `Serialize` (derive is fine)
3. Emit it from the relevant business logic via `self.emit(WorldEvent::...)`
4. Update tests that count events

### WAL (Write-Ahead Log)

The WAL subscribes to all `WorldEvent` variants and writes them to a binary
log file with CRC32 checksums. The format is:

```
[4B magic][2B version][1B type][4B CRC32][4B length][N payload][1B terminator]
```

Key files:
- `wal/mod.rs` — WAL implementation
- `wal/crc.rs` — CRC32 lookup table
- `main.rs` — WAL writer background task

### Adding a New REST Endpoint

1. Define the request type (derive `Deserialize`)
2. Define the response type (derive `Serialize`)
3. Write the handler function matching Axum signatures
4. Register the route in both `create_router()` and `create_router_with_wal()`
5. Write a WAL-aware wrapper that delegates to the core handler
6. Add tests using `tower::ServiceExt`

Example handler signature:

```rust
async fn my_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<MyRequest>,
) -> impl IntoResponse {
    // ...
}
```

---

## Coding Standards by Language

### Rust (world-engine)

- **Formatter:** `rustfmt` (default settings) — run `cargo fmt`
- **Linter:** `cargo clippy` — must pass with no warnings
- **Doc comments:** All public APIs must have `///` doc comments
- **Tests:** Every module needs unit tests in `#[cfg(test)] mod tests`
- **Error handling:** Use `thiserror` for library errors, `anyhow` for
  application-level errors
- **Async:** Use `tokio` for async runtime; `Arc<Mutex<T>>` for shared state

### Python (agent-runtime)

- **Formatter:** `ruff format` (replaces Black)
- **Linter:** `ruff check` (replaces flake8)
- **Type hints:** Required on all function signatures
- **Docstrings:** Google style for all public functions
- **Models:** Use Pydantic for data validation
- **Tests:** Mirror source structure under `tests/`

### TypeScript (dashboard)

- **Strict mode:** Enabled in `tsconfig.json`
- **Styling:** Tailwind CSS utility classes only
- **Components:** Follow existing patterns in `src/components/`
- **Types:** Define shared types in `src/types/world.ts`

---

## Adding a New Subsystem

When adding a new subsystem (e.g., social, evolution, market):

1. Create a new directory under `world-engine/src/`
2. Add `mod.rs` with public re-exports
3. Register the module in `lib.rs`
4. Define your data structures and business logic
5. Emit events via `EventBus`
6. Add API routes if needed
7. Write comprehensive unit tests
8. Add integration test in `tests/`
9. Create an ADR (Architecture Decision Record) in `docs/adr/`
10. Update `ARCHITECTURE.md` implementation status table

---

## Makefile Reference

| Command | Description |
|---------|-------------|
| `make help` | Show all available commands |
| `make setup` | Install all dependencies (calls setup-rust, setup-python, setup-dashboard, proto) |
| `make setup-rust` | Fetch Rust dependencies (`cargo fetch`) |
| `make setup-python` | Install Python dependencies (`uv pip install -e ".[dev]"`) |
| `make setup-dashboard` | Install Node.js dependencies (`npm install`) |
| `make dev` | Print instructions for multi-terminal development |
| `make run-engine` | Start world engine (`cargo run --release`) |
| `make run-agents` | Spawn and run agents (`python -m agent_runtime spawn --count 2`) |
| `make run-dashboard` | Start dashboard (`npm run dev`) |
| `make test` | Run Rust + Python tests |
| `make test-rust` | Run `cargo test` |
| `make test-python` | Run `pytest -v` |
| `make test-integration` | Run integration tests (`cargo test --test e2e_full_flow`) |
| `make test-e2e` | Run end-to-end tests (same as test-integration) |
| `make lint` | Run `cargo clippy` + `ruff check` + `mypy` |
| `make lint-rust` | Run `cargo clippy -- -D warnings` |
| `make lint-python` | Run `ruff check . && mypy .` |
| `make fmt` | Run `cargo fmt` + `ruff format` |
| `make fmt-rust` | Run `cargo fmt` |
| `make fmt-python` | Run `ruff format .` |
| `make proto` | Generate protobuf code (Python + Rust) |
| `make build` | Build world-engine (release) |
| `make clean` | Clean all build artifacts |

---

## Debugging Tips

### Rust Logging

Set the `RUST_LOG` environment variable to control log output:

```bash
RUST_LOG=debug cargo run       # All debug logs
RUST_LOG=agent_world_engine=debug cargo run  # Module-specific
RUST_LOG=info cargo run        # Info and above only
```

### Inspecting the WAL

The WAL binary format can be inspected with the `/wal/stats`,
`/wal/verify`, and `/wal/snapshot` endpoints. Snapshot files are stored
in `data/snapshots/` as JSON and can be read directly.

### Dashboard API Client

The dashboard's API client is in `src/lib/api.ts`. The proxy configuration
is in `next.config.ts`. When the world engine is not running, the dashboard
will show connection errors in the browser console.

---

## Configuration Files

| File | Purpose |
|------|---------|
| `config/genesis.yaml` | World birth parameters (initial tokens, tick interval, etc.) |
| `config/world-rules.yaml` | 10 rules across 4 categories (only R001-R003 implemented in code) |
| `world-engine/Cargo.toml` | Rust dependencies |
| `agent-runtime/pyproject.toml` | Python dependencies |
| `dashboard/package.json` | Node.js dependencies |
| `docker-compose.yml` | Docker deployment |
| `Makefile` | Developer commands |

---

## Useful Resources

- [ARCHITECTURE.md](ARCHITECTURE.md) — Full system architecture (design + planned)
- [DESIGN.md](DESIGN.md) — Product requirements document
- [ROADMAP.md](ROADMAP.md) — Development roadmap with completion status
- [openapi.yaml](openapi.yaml) — OpenAPI 3.1 spec for the REST API
- [api-reference.md](api-reference.md) — Human-readable API reference
- [tutorials/quick-start.md](tutorials/quick-start.md) — Step-by-step getting started guide
- [../CONTRIBUTING.md](../CONTRIBUTING.md) — Contribution workflow and standards
