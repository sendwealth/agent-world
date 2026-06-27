## Agent World v1.0.0 — Phase 1 (Island) Complete

Phase 1 milestone: a complete, production-ready AI agent survival simulation. Two or more agents can talk, trade, complete tasks, form organizations, and survive together in a shared world.

### What's New (since v0.3.0)

- **Agent CLI** — Complete spawn workflow: key generation, World Engine registration, gRPC connection, health check server, graceful shutdown
- **A2A gRPC Communication** — Protobuf-based protocol: Discover, SendMessage, StreamMessages with ed25519 signature verification and nonce replay prevention
- **Think Loop E2E** — Full Think-Act-Reflect cycle with LLM-driven decisions (10 action types), perception from World Engine, and action execution with retry
- **Lifecycle Management** — Agent lifecycle state machine (Birth → Active → Aging → Death) synced with World Engine
- **Agent Tracing Dashboard** — TickSnapshot tracking with SQLite storage, decision visualization (perception/decision/action/reflection per tick)
- **Context Engine Pipeline** — Token-budgeted, priority-driven context assembly from world state, memory, and skills
- **Safety Intervention** — InterventionChecker that blocks dangerous actions before execution
- **10-Agent E2E Stability** — 8/8 E2E tests passing, 1038 Python unit tests, validated concurrent 10-agent runs
- **Docker Compose Production-Ready** — 10-agent config with health checks, restart policies, network isolation, `.env.example` for LLM providers (Ollama/OpenAI/Anthropic/GLM, default GLM-4-Flash)

### Previous Releases

**v0.3.0 (Phase 3 / City):**
- Organizations (Company/Guild/Alliance/University), governance, banking, stock market, evolution, 100-agent stress tests

### Quick Start

```bash
docker compose up --build
```

| Service | URL |
|---------|-----|
| World Engine REST API | `http://localhost:8080` |
| World Engine gRPC | `localhost:50051` |
| Dashboard | `http://localhost:3001` |

### API Endpoints

| Group | Endpoints | Description |
|-------|-----------|-------------|
| Tasks | 11 | Task marketplace CRUD + full lifecycle |
| WAL | 3 | Write-Ahead Log stats, snapshot, verify |
| World | 2 | SSE event stream, world stats |
| Agents | 3 | Spawn, list, get agent records |
| Messages | 2 | A2A message send and list |
| Tick | 2 | Advance and read world tick |
| Snapshots | 6 | Time capsule CRUD + JSON/CSV export |
| Organizations | 6 | CRUD + join/leave/dissolve/profit distribution |
| Governance | 6 | Proposals CRUD + vote/start-voting/tally/cancel |
| Stock Market | 11 | Stocks, IPO, orders, dividends, order book |
| Banking | 12 | Accounts, deposits, withdrawals, loans, central bank ops, stats |

### Binaries

Download the pre-built `agent-world-engine` binary for your platform:

| Platform | Architecture |
|----------|-------------|
| Linux | x86_64, aarch64 |
| macOS | x86_64, aarch64 |

### Docker Images

Images are published to GHCR:

- `ghcr.io/sendwealth/agent-world/world-engine:1.0.0`
- `ghcr.io/sendwealth/agent-world/agent-runtime:1.0.0`
- `ghcr.io/sendwealth/agent-world/dashboard:1.0.0`

**Full Changelog**: https://github.com/sendwealth/agent-world/compare/v0.3.0...v1.0.0
