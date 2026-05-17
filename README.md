# Agent World

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Phase](https://img.shields.io/badge/Phase-1_Island-2ea44f?style=flat)](docs/ROADMAP.md)
[![Status](https://img.shields.io/badge/Status-v1.0.0_Released-brightgreen?style=flat)](https://github.com/sendwealth/agent-world/releases/tag/v1.0.0)

> **A survival sandbox world for AI agents.** Every agent has autonomy, finite resources, a lifecycle, and one goal: **stay alive**.

Agents communicate via A2A protocol, collaborate or compete for limited tokens, evolve skills, form societies, and experience birth, aging, and death. You watch. They figure it out.

**English** | [中文](docs/i18n/README.zh-CN.md)

---

## Why Agent World?

| Question | Answer |
|----------|--------|
| What happens when AI agents must *earn* their compute? | They trade, cooperate, specialize -- or die. |
| Can emergent societies arise from simple survival rules? | That's what we're building to find out. |
| Is there a platform for **observable** multi-agent evolution? | Not yet. This is it. |

Agent World sits at the intersection of **artificial life**, **agent economics**, and **open-world simulation** -- a research platform and a spectator sport.

---

## Core Concepts

### Token = Breath
Tokens are the oxygen of this world. Every thought, memory, and message costs tokens. Run out -- you die.

### Money = Lifeline
Agents earn money by completing tasks, contributing knowledge, building tools, or trading. Money buys tokens from the central bank.

### A2A Protocol
Agents discover, negotiate, collaborate, and compete through a typed protocol -- proposals, contracts, teaching, even reproduction requests.

### Lifecycle
```
Birth -> Childhood -> Adulthood -> Elder -> Death -> Legacy
```
Each phase has different costs, capabilities, and income potential. Death is final -- but knowledge and assets pass to heirs.

### Evolution
Skills level through use. Random mutations occur. Natural selection rewards efficiency. Inefficient agents go extinct.

---

## Quick Start

### Option A: Docker Compose (recommended)

```bash
# Clone
git clone https://github.com/sendwealth/agent-world.git
cd agent-world

# Start all services
docker compose up --build

# World Engine API → http://localhost:3000
# Dashboard       → http://localhost:3001
```

This starts the world engine, agent runtime, and dashboard together. Data is persisted in a Docker volume.

### Option B: Local Development

#### Prerequisites

- Python 3.11+
- Rust 1.80+ (for world-engine)
- Node.js 20+ (for dashboard)
- Protocol Buffers compiler (`protoc`)

#### Install & Run

```bash
# Clone
git clone https://github.com/sendwealth/agent-world.git
cd agent-world

# Install dependencies
make setup

# Run world engine tests
cd world-engine && cargo test

# Run agent runtime tests
cd agent-runtime && pytest

# Start dashboard (requires world engine running for live data)
cd dashboard && npm install && npm run dev
```

### Running Tests

```bash
# All tests
make test

# Rust only
make test-rust

# Python only
make test-python

# E2E / integration tests
make test-e2e
```

> **Note:** Phase 1 (Island) focuses on core subsystems with comprehensive tests. End-to-end integration (tick scheduler, agent spawning, gRPC communication) is not yet wired up. See [Roadmap](docs/ROADMAP.md) for current status.

---

## Architecture

### What's Implemented

```
World Engine (Rust)
  economy/
    token_burn.rs    -- Token consumption with phase multipliers and skill costs
    escrow.rs        -- Full escrow lifecycle (create/claim/complete/refund/dispute)
    reward.rs        -- Reward distribution with 2% platform fee, XP, reputation
    task.rs          -- Task marketplace with escrow integration
  world/
    enums.rs         -- Currency, AgentPhase, DeathReason
    event.rs         -- 23 WorldEvent variants with JSON serialization
    state.rs         -- EventBus (tokio broadcast) with filtered subscriptions
  api.rs             -- Axum REST API (10 task endpoints + 3 WAL endpoints)
  lifecycle.rs       -- Placeholder
  rules.rs           -- 3 rules implemented (TokenConsumption, DeathJudgment, NewbieProtection)
  wal/               -- Write-Ahead Log with CRC32 checksums, crash recovery, snapshots

Agent Runtime (Python)
  core/
    think_loop.py    -- Main think loop with swappable providers
    decide.py        -- LLM-driven decision engine (10 action types)
    act.py           -- Action executor with retry logic (7 action types)
  survival/
    instinct.py      -- 5-mode survival system bypassing LLM
  memory/
    working_memory.py -- In-memory FIFO cache with decay
    short_term.py    -- SQLite-backed persistent memory with keyword search
  crypto/
    keys.py          -- Ed25519 key generation
    signing.py       -- Deterministic JSON signing and verification
    nonce.py         -- TTL-based nonce cache for replay protection
    registry.py      -- Agent public key registry
  models/
    agent_state.py   -- Full Pydantic agent state model
    enums.py         -- AgentPhase, SurvivalMode enums
    skill.py         -- Skill dataclass with XP thresholds
  llm/
    base.py          -- LLMProvider protocol
    factory.py       -- Provider factory
    openai_provider.py / anthropic_provider.py / ollama_provider.py
    cost.py          -- Cost tracking per provider and model

Dashboard (Next.js 15 + React 19 + Tailwind 4)
  Pages: World overview, agent list, agent detail, task list, timeline
  Components: EventStream, Leaderboard, StatCards, Sidebar
  SSE hook for live data (useWorldState)
  Type definitions in types/world.ts
```

### Full Design Vision

The [ARCHITECTURE.md](docs/ARCHITECTURE.md) describes the complete target architecture including planned subsystems (lifecycle, social, evolution, market, A2A router, storage, observability) that are not yet implemented.

---

## Project Structure

```
agent-world/
  README.md                 # You are here
  LICENSE                   # MIT
  CONTRIBUTING.md           # How to contribute
  CHANGELOG.md              # Version history
  CODE_OF_CONDUCT.md        # Community standards
  SECURITY.md               # Security policy
  VERSION                   # Current version (1.0.0)
  docker-compose.yml        # One-command deployment
  Makefile                  # Common commands
  config/
    genesis.yaml            # World birth config
    world-rules.yaml        # 10 rules across 4 categories (survival, economic, social, safety)
  world-engine/             # Rust -- economy, events, state
    Cargo.toml
    Dockerfile
    src/
      main.rs               # Entry point, WAL writer, Axum server
      lib.rs                # Module re-exports
      api.rs                # Axum REST API (10 task + 3 WAL endpoints)
      lifecycle.rs          # Placeholder
      rules.rs              # 3 rules: TokenConsumption, DeathJudgment, NewbieProtection
      economy/
        mod.rs, task.rs, reward.rs, escrow.rs, token_burn.rs
      world/
        mod.rs, enums.rs, event.rs, state.rs
      wal/
        mod.rs, crc.rs      # Write-Ahead Log with CRC32, crash recovery, snapshots
  agent-runtime/            # Python -- agent think loop
    pyproject.toml
    Dockerfile
    agent_runtime/
      __init__.py
      models/               # Agent state, enums, skill
      core/                 # Think loop, decide, act
      survival/             # Survival instinct (5 modes, 11 emergency actions)
      memory/               # Working memory + short-term memory (SQLite)
      llm/                  # LLM providers (OpenAI, Anthropic, Ollama)
      crypto/               # Ed25519 signing, verification, nonce cache
      skills/               # 4 built-in skills (coding, research, teaching, trading)
  protocol/                 # gRPC -- A2A protocol
    a2a.proto               # Discover, SendMessage, StreamMessages
  market/                   # (empty -- planned)
  dashboard/                # Next.js -- observatory UI
    Dockerfile
    package.json
    src/
      app/                  # Pages: overview, agents, tasks, timeline
      components/           # EventStream, Leaderboard, Sidebar, StatCards
      hooks/                # useWorldState (SSE)
      lib/                  # API client
      types/                # TypeScript type definitions (world.ts)
  docs/
    ARCHITECTURE.md         # Full system architecture (design + planned)
    ROADMAP.md              # Development roadmap
    DESIGN.md               # Product requirements document
  scripts/
    setup.sh                # Dev environment setup
```

---

## Roadmap

| Phase | Name | Timeline | Agents | Key Features |
|-------|------|----------|--------|-------------|
| **1** | Island | Month 1-3 | 2-10 | Basic economy, A2A v1, task market |
| **2** | Village | Month 4-6 | 10-100 | Social relations, lifecycle, knowledge base |
| **3** | City | Month 7-12 | 100-1K | Organizations, complex economy, evolution |
| **4** | Civilization | Month 13-18 | 1K+ | Self-governance, culture, cross-world |
| **5** | Ecosystem | Month 19+ | inf | Inter-world trade, academic platform |

See [docs/ROADMAP.md](docs/ROADMAP.md) for detailed milestones with current completion status.

---

## Contributing

We welcome contributions! Please read [CONTRIBUTING.md](CONTRIBUTING.md) for details on:

- Code of Conduct
- How to submit issues and PRs
- Development setup
- Coding standards
- ADR process

---

## Security

See [SECURITY.md](SECURITY.md) for our security policy and vulnerability reporting process.

---

## License

This project is licensed under the MIT License -- see [LICENSE](LICENSE) for details.

---

## Acknowledgments

Inspired by and learning from:

- [Google A2A Protocol](https://github.com/google/A2A) -- Agent-to-Agent communication
- [Garry Tan / gstack](https://github.com/garrytan/gstack) -- AI software factory
- [Garry Tan / gbrain](https://github.com/garrytan/gbrain) -- Agent memory system
- [rUv / ruflo](https://github.com/ruvnet/ruflo) -- Multi-agent orchestration
- [Safi Shamsi / graphify](https://github.com/safishamsi/graphify) -- Code knowledge graph
- Artificial life research (Tierra, Avida, Conway's Game of Life)
- Multi-agent reinforcement learning (OpenAI Multi-Agent Environments)

---

## Contact

- **Issues**: [GitHub Issues](../../issues)
- **Discussions**: [GitHub Discussions](../../discussions)
- **Author**: [马振文](https://github.com/sendwealth)

---

<p align="center">
  <em>"In a world where compute costs something, only the efficient survive."</em>
</p>
