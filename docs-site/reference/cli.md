---
title: CLI Reference
description: Command-line tools for building, testing, and running Agent World — make targets, cargo commands, Python commands, and Docker Compose commands.
---

# CLI Reference

Agent World uses a `Makefile` as the primary command hub, with `cargo`, `python`, and `docker compose` underneath. This page lists every command you'll need.

## Make Commands

The top-level `Makefile` orchestrates all operations. Run `make help` to see the full list.

### Setup & Build

| Command | Description |
|---------|-------------|
| `make setup` | Install all dependencies (Rust, Python, Dashboard, protobuf) |
| `make setup-rust` | Fetch Rust dependencies (`cargo fetch`) |
| `make setup-python` | Install Python dependencies (`uv pip install -e ".[dev]"`) |
| `make setup-dashboard` | Install Dashboard dependencies (`npm install`) |
| `make proto` | Generate protobuf code for Rust and Python from `.proto` files |
| `make build` | Build the World Engine in release mode (`cargo build --release`) |

### Development

| Command | Description |
|---------|-------------|
| `make dev` | Start all services with Docker Compose (world-engine + 10 agents + dashboard) |
| `make dev-llm` | Same as `dev` but also starts a local Ollama LLM container |
| `make dev-detach` | Start all services in background (`-d`) |
| `make dev-down` | Stop all Docker Compose services |
| `make dev-logs` | Tail Docker Compose logs |
| `make dev-ps` | List running Docker Compose services |
| `make dev-restart` | Restart all services (rebuild + force-recreate) |

### CI Profile (Minimal)

| Command | Description |
|---------|-------------|
| `make dev-ci` | Start CI profile: world-engine + 1 agent only |
| `make dev-ci-down` | Stop CI profile |

### Running Locally (No Docker)

| Command | Description |
|---------|-------------|
| `make run-engine` | Start the World Engine locally (`cargo run --release`) |
| `make run-agents` | Spawn and run agents locally (`python -m agent_runtime spawn --count 2`) |
| `make run-dashboard` | Start the Dashboard locally (`npm run dev`) |

### Testing

| Command | Description |
|---------|-------------|
| `make test` | Run all tests (Rust + Python) |
| `make test-rust` | Run Rust tests (`cargo test`) |
| `make test-python` | Run Python tests (`pytest -v`) |
| `make test-integration` | Run Rust integration tests |
| `make test-e2e` | Run end-to-end tests (`cargo test --test e2e_full_flow`) |
| `make test-e2e-integration` | Run Python E2E integration tests (`pytest tests/e2e/`) |
| `make bench` | Run benchmark (100 agents × 2000 ticks) |
| `make stress` | Run stress tests (100 agents concurrent) |

### Demos

| Command | Description |
|---------|-------------|
| `make demo` | Run E2E demo: 2 agents survive 1000 ticks with trading, tasks, death |
| `make demo-json` | Same as `demo` with JSON metrics output |
| `make demo-death` | Run death scenario (agent with 30 tokens) |

### Code Quality

| Command | Description |
|---------|-------------|
| `make lint` | Run all linters (Rust + Python) |
| `make lint-rust` | Run Clippy (`cargo clippy -- -D warnings`) |
| `make lint-python` | Run Ruff + MyPy (`ruff check . && mypy .`) |
| `make fmt` | Format all code |
| `make fmt-rust` | Format Rust (`cargo fmt`) |
| `make fmt-python` | Format Python (`ruff format .`) |

### Cleanup

| Command | Description |
|---------|-------------|
| `make clean` | Remove all build artifacts (cargo clean, caches, node_modules, generated proto) |

---

## Cargo Commands (World Engine)

These are run from the `world-engine/` directory.

| Command | Description |
|---------|-------------|
| `cd world-engine && cargo build` | Build in debug mode |
| `cd world-engine && cargo build --release` | Build in release mode (optimized) |
| `cd world-engine && cargo run` | Build and run (debug) |
| `cd world-engine && cargo run --release` | Build and run (release) |
| `cd world-engine && cargo test` | Run all unit and integration tests |
| `cd world-engine && cargo test -- --nocapture` | Run tests with stdout output visible |
| `cd world-engine && cargo test --test e2e_full_flow` | Run only the E2E test |
| `cd world-engine && cargo test --test benchmark_100_agents` | Run benchmark test |
| `cd world-engine && cargo clippy -- -D warnings` | Lint with Clippy |
| `cd world-engine && cargo fmt` | Format code |
| `cd world-engine && cargo fmt -- --check` | Check formatting without changing files |
| `cd world-engine && cargo fetch` | Download dependencies without building |

---

## Python Commands (Agent Runtime)

These are run from the `agent-runtime/` directory.

| Command | Description |
|---------|-------------|
| `cd agent-runtime && uv pip install -e ".[dev]"` | Install in editable mode with dev dependencies |
| `cd agent-runtime && pytest` | Run all tests |
| `cd agent-runtime && pytest -v` | Run tests with verbose output |
| `cd agent-runtime && pytest tests/test_think_loop.py` | Run a specific test file |
| `cd agent-runtime && ruff check .` | Lint with Ruff |
| `cd agent-runtime && ruff format .` | Format with Ruff |
| `cd agent-runtime && mypy .` | Type-check with MyPy |
| `cd agent-runtime && python -m agent_runtime spawn --count 2` | Spawn 2 agents |
| `cd agent-runtime && python -m agent_runtime --help` | Show agent runtime CLI help |

---

## Docker Compose Commands

| Command | Description |
|---------|-------------|
| `docker compose up --build` | Build and start all services |
| `docker compose up --build -d` | Build and start in background |
| `docker compose --profile local-llm up --build` | Start with Ollama LLM |
| `docker compose --profile ci up --build` | Start CI profile (minimal) |
| `docker compose down` | Stop and remove containers |
| `docker compose logs -f` | Tail logs from all services |
| `docker compose logs -f world-engine` | Tail only world-engine logs |
| `docker compose ps` | List running services |
| `docker compose build` | Rebuild images without starting |
| `docker compose pull` | Pull latest base images |
| `docker compose up --build -d --force-recreate` | Force rebuild and restart |

---

## Protobuf Code Generation

| Command | Description |
|---------|-------------|
| `make proto` | Generate Rust + Python code from `protocol/*.proto` |
| `protoc --proto_path=protocol --python_out=protocol/gen/python --grpc_python_out=protocol/gen/python protocol/*.proto` | Generate Python stubs manually |
| `protoc --proto_path=protocol --rust_out=protocol/gen/rust --tonic_out=protocol/gen/rust protocol/*.proto` | Generate Rust stubs manually |

::: tip Prerequisite
`protoc` must be installed. On macOS: `brew install protobuf`. On Ubuntu: `apt install protobuf-compiler`.
:::
