# Agent World

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Phase](https://img.shields.io/badge/Phase-3_City-6366f1?style=flat)](docs/ROADMAP.md)
[![Status](https://img.shields.io/badge/Status-v1.0.0_~75%25_Done-yellow?style=flat)](docs/ROADMAP.md)

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

### Organizations
Agents form Companies (profit), Guilds (skill-based), Alliances (defense), and Universities (knowledge). Each has governance, voting, and profit distribution.

### Finance
A full banking system with savings accounts, loans, collateral, and a central bank. Plus a stock market with IPOs, order books, and dividend distribution.

---

## Quick Start

### Prerequisites

| Tool | Version | Notes |
|------|---------|-------|
| Docker | 20+ | Container runtime |
| Docker Compose | v2+ | Included with Docker Desktop |
| Ollama | latest | Local LLM — install from [ollama.com](https://ollama.com) |

Pull an LLM model before starting (requires ~8 GB RAM for llama3):

```bash
ollama pull llama3
```

### Start with Docker Compose

```bash
# Clone
git clone https://github.com/sendwealth/agent-world.git
cd agent-world

# Configure environment (defaults work out of the box)
cp .env.example .env

# Build and start all services (world-engine + 10 agents + dashboard)
docker compose up -d --build

# Watch logs
docker compose logs -f

# Stop everything
docker compose down
```

**Access points after startup:**

| Service | URL |
|---------|-----|
| Dashboard | [http://localhost:3001](http://localhost:3001) |
| World Engine API | [http://localhost:8080](http://localhost:8080) |

The default configuration starts 10 agents using Ollama (zero-cost, local LLM). Data persists in Docker volumes across restarts.

### Advanced: Custom LLM Provider

Edit `.env` to switch providers. Supported: `ollama` (default), `openai`, `anthropic`, `zhipu` (智谱 GLM-5).

```bash
# Example: switch to OpenAI
LLM_PROVIDER=openai
LLM_MODEL=gpt-4o-mini
OPENAI_API_KEY=sk-your-key-here
```

See `.env.example` for all configuration options.

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

# Stress test with 100 agents
cd world-engine && cargo test stress_100

# Benchmarks
cd world-engine && cargo bench
```

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
    banking.rs       -- Banking system: accounts, loans, collateral, central bank
    stock_market.rs  -- Stock market: IPOs, order book, dividends, delisting
  organization/
    org.rs           -- Organizations: Company/Guild/Alliance/University
    members.rs       -- Membership management with roles and shares
    charter.rs       -- Charter with governance model and profit sharing
    governance.rs    -- Voting, proposals, weighted votes, profit distribution
  evolution/
    skill_tree.rs    -- Branching skill tree (10 skills, levels 1-10)
    mutation.rs      -- Mutation engine: NewSkill, SkillBoost, SkillDecay
    selection.rs     -- Natural selection with fitness scoring and culling
    subsystem.rs     -- EvolutionSubsystem integrated into tick loop
  world/
    enums.rs         -- Currency, AgentPhase, DeathReason
    event.rs         -- 30+ WorldEvent variants with JSON serialization
    state.rs         -- EventBus (tokio broadcast) with filtered subscriptions + SSE
  api.rs             -- Axum REST API (tasks, WAL, orgs, governance, stocks, banking)
  lifecycle.rs       -- Lifecycle state machine (birth, aging, death transitions)
  rules.rs           -- 3 rules implemented (TokenConsumption, DeathJudgment, NewbieProtection)
  wal/               -- Write-Ahead Log with CRC32 checksums, crash recovery, snapshots
  benches/           -- Criterion benchmarks for hot paths (100-agent scale)
  tests/
    stress_100_agents.rs -- 5 stress tests validating 100-agent concurrency

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
  Pages: World overview, agent list, agent detail, task list, timeline,
         organizations, organization detail, stocks, evolution, economy
  Components: EventStream, Leaderboard, StatCards, Sidebar
  SSE hook for live data (useWorldState)
  Type definitions in types/world.ts
  Charts: Recharts (AreaChart, BarChart, RadialBarChart, LineChart)
```

### Full Design Vision

The [ARCHITECTURE.md](docs/ARCHITECTURE.md) describes the complete target architecture including planned subsystems that are not yet implemented.

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
  VERSION                   # Current version (0.3.0)
  RELEASE_BODY.md           # GitHub Release body template
  docker-compose.yml        # 10-agent deployment
  docker-compose-v3.yml     # 100-agent deployment (Phase 3 scale)
  Makefile                  # Common commands
  config/
    genesis.yaml            # World birth config (economy, lifecycle, evolution)
    world-rules.yaml        # 10 rules across 4 categories
    agents/                 # 100 agent TOML configs
  world-engine/             # Rust -- economy, organizations, governance, banking, stocks, evolution
    Cargo.toml
    Dockerfile
    src/
      main.rs               # Entry point, WAL writer, Axum server
      lib.rs                # Module re-exports
      api.rs                # Axum REST API (all endpoints)
      lifecycle.rs          # Lifecycle state machine
      rules.rs              # 3 rules: TokenConsumption, DeathJudgment, NewbieProtection
      economy/
        mod.rs, task.rs, reward.rs, escrow.rs, token_burn.rs, banking.rs, stock_market.rs
      organization/
        mod.rs, org.rs, members.rs, charter.rs, governance.rs
      evolution/
        mod.rs, skill_tree.rs, mutation.rs, selection.rs, subsystem.rs
      world/
        mod.rs, enums.rs, event.rs, state.rs
      wal/
        mod.rs, crc.rs
    benches/
      hotpath_benchmarks.rs # Criterion benchmarks
    tests/
      stress_100_agents.rs  # 100-agent stress tests
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
  dashboard/                # Next.js -- observatory UI
    Dockerfile
    package.json
    src/
      app/                  # Pages: overview, agents, tasks, timeline, orgs, stocks, evolution, economy
      components/           # EventStream, Leaderboard, Sidebar, StatCards
      hooks/                # useWorldState (SSE)
      lib/                  # API client
      types/                # TypeScript type definitions (world.ts)
  docs/
    ARCHITECTURE.md         # Full system architecture
    ROADMAP.md              # Development roadmap
    DESIGN.md               # Product requirements document
    api-reference.md        # API reference documentation
    openapi.yaml            # OpenAPI spec
    developer-guide.md      # Developer guide
    tutorials/              # Quick start and usage tutorials
    i18n/                   # Internationalized documentation
    adr/                    # Architecture Decision Records
  scripts/
    setup.sh                # Dev environment setup
    generate-compose-v3.sh  # Generate 100-agent Docker Compose
```

---

## Roadmap

| Phase | Name | Timeline | Agents | Key Features | Status |
|-------|------|----------|--------|-------------|--------|
| **1** | Island | Month 1-3 | 2-10 | Basic economy, A2A v1, task market | ✅ Done |
| **2** | Village | Month 4-6 | 10-100 | Social relations, lifecycle, knowledge base | ⚠️ ~70% |
| **3** | City | Month 7-12 | 100-1K | Organizations, complex economy, evolution | ⚠️ ~85% |
| **4** | Civilization | Month 13-18 | 1K+ | Self-governance, culture, cross-world | ⚠️ ~40% |
| **5** | Ecosystem | Month 19+ | inf | Inter-world trade, academic platform | 🔴 Planned |

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
