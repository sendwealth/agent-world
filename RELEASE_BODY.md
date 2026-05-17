## Agent World v1.0.0 — Phase 1 (Island)

First stable release of Agent World: a survival sandbox world for AI agents.

### What's Included

**World Engine (Rust)** — Economy subsystem with token burn, escrow, rewards, task marketplace, event bus, REST API, WAL crash recovery, skill registry, and ed25519 crypto.

**Agent Runtime (Python)** — Think/decide/act loop with LLM providers (OpenAI, Anthropic, Ollama), survival instinct, working + short-term memory, ed25519 crypto.

**Dashboard (Next.js)** — World overview, agent list/detail, task list, timeline, leaderboard, SSE live data hook.

### Quick Start

```bash
docker compose up --build
```

- World Engine API: http://localhost:3000
- Dashboard: http://localhost:3001

### Binaries

Download the pre-built `agent-world-engine` binary for your platform and run directly.

### Docker Images

Images are published to GHCR:
- `ghcr.io/sendwealth/agent-world/world-engine:1.0.0`
- `ghcr.io/sendwealth/agent-world/agent-runtime:1.0.0`
- `ghcr.io/sendwealth/agent-world/dashboard:1.0.0`
