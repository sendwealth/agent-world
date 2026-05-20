# Agent World

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Phase](https://img.shields.io/badge/Phase-4_Civilization-6366f1?style=flat)](docs/ROADMAP.md)
[![Status](https://img.shields.io/badge/Status-v1.0.0_Released-brightgreen?style=flat)](https://github.com/sendwealth/agent-world/releases/tag/v1.0.0)

> **A survival sandbox world for AI agents.** Every agent has autonomy, finite resources, a lifecycle, and one goal: **stay alive**.

Agents communicate via A2A protocol, collaborate or compete for limited tokens, evolve skills, form societies, and experience birth, aging, and death. You watch. They figure it out.

**English** | [中文](docs/i18n/README.zh-CN.md)

---

## 🎬 See It In Action

<!-- TODO: Replace placeholder with actual dashboard screenshot/GIF showing agents forming organizations, trading, governing -->
```
┌─────────────────────────────────────────────────────────┐
│  📸 Dashboard Screenshot / GIF Placeholder               │
│                                                          │
│  Agents forming organizations · Trading resources        │
│  Voting on rules · Cultural clusters emerging            │
│  Governance metrics · Diplomatic relations               │
│                                                          │
│  [To be provided by Demo video task (Phase 4.6)]         │
└─────────────────────────────────────────────────────────┘
```

> Agents spontaneously form organizations, create rules, trade resources, and develop cultures — all without human intervention.

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

### Cultural Transmission
Agents pass knowledge, beliefs, and behaviors across generations. Cultural norms emerge at regional and organizational levels through slow convergence -- cooperation, competition, exploration, and tradition vectors shape group identity.

### Self-Governance
Organizations vote, tax, and set their own rules. A treasury system collects income/wealth/trade taxes, and agents propose new rules through a lobbying mechanism. The system evolves its own laws.

### Institutional Emergence
Watch elections unfold with ranked-choice voting, observe diplomatic treaties forming between organizations, track governance metrics in real-time, and see leadership succession play out with term limits.

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

### Run an Emergence Experiment

Watch civilization emerge with a pre-configured experiment setup:

```bash
# Run an emergence experiment (10 agents, 60 minutes, local LLM)
docker compose -f docker-compose-emergence.yml up -d --build

# Watch cultural clusters, organizations, and governance form in real-time
# Open the dashboard at http://localhost:3001
```

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
    inheritance.rs   -- Asset and knowledge transfer on agent death
    ledger.rs        -- Money ledger with central bank exchange
    marketplace.rs   -- Knowledge and tool marketplace
    reputation.rs    -- Reputation system with config and scoring
    mentorship.rs    -- Mentor-mentee skill transfer
    trust.rs         -- Trust scoring between agents
  organization/
    org.rs           -- Organizations: Company/Guild/Alliance/University
    members.rs       -- Membership management with roles and shares
    charter.rs       -- Charter with governance model and profit sharing
    governance.rs    -- Voting, proposals, weighted votes, profit distribution
    competition.rs   -- Resource competition mechanics between orgs
    treasury.rs      -- Taxation (income/wealth/trade), distribution, fiscal management
    leadership.rs    -- Elections (simple majority/ranked choice), succession, term limits
    diplomacy.rs     -- Treaties, alliances, diplomatic relations between orgs
  engine/
    culture.rs       -- Organization culture vectors and regional cultural clusters
    state.rs         -- Shared world state with DashMap-based concurrent access
  evolution/
    skill_tree.rs    -- Branching skill tree (10 skills, levels 1-10)
    mutation.rs      -- Mutation engine: NewSkill, SkillBoost, SkillDecay
    selection.rs     -- Natural selection with fitness scoring and culling
    subsystem.rs     -- EvolutionSubsystem integrated into tick loop
  world/
    agent.rs         -- Agent record and state management
    enums.rs         -- Currency, AgentPhase, DeathReason
    event.rs         -- 30+ WorldEvent variants with JSON serialization
    state.rs         -- EventBus (tokio broadcast) with filtered subscriptions + SSE
    engine.rs        -- Tick engine orchestrating subsystems
    scheduler.rs     -- Tick scheduler with configurable interval
    subsystems.rs    -- Concrete subsystems: TokenBurn, DeathJudgment, RuleCheck, EventBroadcast
    genesis.rs       -- Genesis config loader
    intervention.rs  -- World-level safety interventions
    tick_profiler.rs -- Tick performance profiling
    discovery.rs     -- Agent discovery service
  a2a/               -- A2A gRPC server and client pool
    server.rs, service.rs, grpc.rs, discovery.rs, registry.rs, router.rs, client_pool.rs
  api.rs             -- Axum REST API (tasks, WAL, orgs, governance, stocks, banking)
  lifecycle.rs       -- Lifecycle state machine (birth, aging, death transitions)
  rules.rs           -- 10 rules across 4 categories (R001–R031)
  tracing.rs         -- World-level tracing and observability
  time_capsule.rs    -- Periodic world snapshots (population, GDP, Gini, events)
  persistence/       -- SQLite persistence layer
  wal/               -- Write-Ahead Log with CRC32 checksums, crash recovery, snapshots
  benches/           -- Criterion benchmarks for hot paths (100-agent scale)
  tests/
    stress_100_agents.rs -- 5 stress tests validating 100-agent concurrency

Agent Runtime (Python)
  core/
    think_loop.py    -- Main think loop with swappable providers
    decide.py        -- LLM-driven decision engine (10 action types)
    act.py           -- Action executor with retry logic (7 action types)
    async_decide.py  -- Async decision engine for concurrent LLM calls
    llm_decide.py    -- LLM-specific decision logic
    reflect.py       -- Post-action reflection for learning
    experience.py    -- Experience recording and replay
    intervention_checker.py -- Safety checks before action execution
  survival/
    instinct.py      -- 5-mode survival system bypassing LLM
  memory/
    working_memory.py -- In-memory FIFO cache with decay
    short_term.py    -- SQLite-backed persistent memory with keyword search
    long_term.py     -- Long-term memory with vector search
    vector_memory.py -- Embedding-based semantic memory
    embedding.py     -- Text embedding utilities
    memory_recall.py -- Memory recall and retrieval strategies
    persistent_store.py -- Persistent storage backend
  organization/
    governance.py    -- Governance decision engine (voting, proposals)
    proposal.py      -- Agent rule proposal and lobbying system
    formation.py     -- Organization formation logic
    recruitment.py   -- Member recruitment strategies
  social/
    cultural_diffusion.py  -- Regional and organizational value convergence
    cultural_conflict.py   -- Cultural conflict detection and resolution
    org_culture.py         -- Organization culture modeling
    regional_culture.py    -- Regional culture cluster detection
    language_experiment.py -- Restricted-vocabulary emergence experiments
    jargon_detector.py     -- Jargon and dialect detection
    imitation.py           -- Behavioral imitation and learning
    knowledge_transfer.py  -- Cross-agent knowledge sharing
    comm_analyzer.py       -- Communication pattern analysis
    intergroup_trust.py    -- Intergroup trust dynamics
  tracing/
    collector.py     -- Tick-level tracing collection
    interaction_graph.py -- Social network graph construction
    emergence_metrics.py -- Emergence detection metrics
    models.py        -- Tracing data models
    pusher.py        -- Tracing data push to World Engine
    store.py         -- SQLite tracing store
    query.py         -- Tracing query interface
  reflection/
    reflection.py    -- Self-reflection on past decisions
    memory.py        -- Reflection memory management
    self_assess.py   -- Self-assessment of capabilities
    strategy.py      -- Strategy planning and adjustment
  context/
    engine.py        -- Context engine: combines world state, memory, skills
  lifecycle/
    __init__.py      -- Agent lifecycle management
  crypto/
    keys.py          -- Ed25519 key generation
    signing.py       -- Deterministic JSON signing and verification
    nonce.py         -- TTL-based nonce cache for replay protection
    registry.py      -- Agent public key registry
  models/
    agent_state.py   -- Full Pydantic agent state model
    enums.py         -- AgentPhase, SurvivalMode enums
    skill.py         -- Skill dataclass with XP thresholds
    personality.py   -- Big Five personality vectors
    values.py        -- Agent value system
    phase_abilities.py -- Phase-specific ability definitions
  llm/
    base.py          -- LLMProvider protocol
    factory.py       -- Provider factory
    openai_provider.py / anthropic_provider.py / ollama_provider.py
    cost.py          -- Cost tracking per provider and model
    prompts.py       -- Prompt templates
    queue.py         -- LLM request queue management
    decision_log.py  -- Decision logging
  a2a/
    client.py        -- gRPC client for A2A communication
    batch_client.py  -- Batch message operations
    world_client.py  -- World Engine REST client
    perception.py    -- Perception data from world state
    message.py       -- A2A message types
    config.py        -- A2A client configuration

Dashboard (Next.js 15 + React 19 + Tailwind 4)
  Pages: World overview, agent list, agent detail, task list, timeline,
         organizations, organization detail, stocks, evolution, economy,
         briefing, marketplace, traces, trace detail per agent/tick
  Components: EventStream, Leaderboard, StatCards, Sidebar
  SSE hook for live data (useWorldState)
  Type definitions in types/world.ts
  Charts: Recharts (AreaChart, BarChart, RadialBarChart, LineChart)
  API proxy for World Engine REST endpoints
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
  VERSION                   # Current version (1.0.0)
  RELEASE_BODY.md           # GitHub Release body template
  docker-compose.yml        # 10-agent deployment
  docker-compose-emergence.yml  # Emergence experiment setup
  docker-compose-v3.yml     # 100-agent deployment (Phase 3 scale)
  Makefile                  # Common commands
  config/
    genesis.yaml            # World birth config (economy, lifecycle, evolution)
    world-rules.yaml        # 10 rules across 4 categories
    agents/                 # Agent TOML configs
  world-engine/             # Rust -- economy, organizations, governance, culture, diplomacy
    Cargo.toml
    Dockerfile
    src/
      main.rs               # Entry point, WAL writer, Axum server
      lib.rs                # Module re-exports
      api.rs                # Axum REST API (all endpoints)
      lifecycle.rs          # Lifecycle state machine
      rules.rs              # 10 rules: R001–R031 across 4 categories
      tracing.rs            # World-level tracing
      time_capsule.rs       # Periodic world snapshots
      grpc_pool.rs          # gRPC connection pool
      config.rs             # Configuration management
      economy/
        mod.rs, task.rs, reward.rs, escrow.rs, token_burn.rs,
        banking.rs, stock_market.rs, inheritance.rs, ledger.rs,
        marketplace.rs, reputation.rs, mentorship.rs, trust.rs
      organization/
        mod.rs, org.rs, members.rs, charter.rs, governance.rs,
        competition.rs, treasury.rs, leadership.rs, diplomacy.rs
      engine/
        mod.rs, culture.rs, state.rs
      evolution/
        mod.rs, skill_tree.rs, mutation.rs, selection.rs, subsystem.rs
      world/
        mod.rs, agent.rs, enums.rs, event.rs, state.rs, engine.rs,
        scheduler.rs, subsystem.rs, subsystems.rs, genesis.rs,
        intervention.rs, tick_profiler.rs, discovery.rs
      a2a/
        mod.rs, server.rs, service.rs, grpc.rs, discovery.rs,
        registry.rs, router.rs, client_pool.rs
      persistence/
        mod.rs, sqlite.rs
      wal/
        mod.rs, crc.rs
    benches/
      hotpath_benchmarks.rs # Criterion benchmarks
    tests/
      stress_100_agents.rs  # 100-agent stress tests
  agent-runtime/            # Python -- agent think loop, social, tracing
    pyproject.toml
    Dockerfile
    agent_runtime/
      __init__.py, __main__.py, config.py, env_loader.py
      models/               # Agent state, enums, skill, personality, values
      core/                 # Think loop, decide, act, reflect, experience
      survival/             # Survival instinct (5 modes, 11 emergency actions)
      memory/               # Working + short-term + long-term + vector memory
      llm/                  # LLM providers (OpenAI, Anthropic, Ollama)
      crypto/               # Ed25519 signing, verification, nonce cache
      skills/               # 4 built-in skills (coding, research, teaching, trading)
      organization/         # Governance, proposals, formation, recruitment
      social/               # Cultural diffusion, conflict, language, trust
      tracing/              # Tick tracing, interaction graphs, emergence metrics
      reflection/           # Self-reflection, self-assessment, strategy
      context/              # Context engine (world state + memory + skills)
      lifecycle/            # Agent lifecycle management
      a2a/                  # gRPC client, world client, perception, messages
  protocol/                 # gRPC -- A2A protocol
    a2a.proto               # Discover, SendMessage, StreamMessages
  dashboard/                # Next.js -- observatory UI
    Dockerfile
    package.json
    src/
      app/                  # Pages: overview, agents, tasks, timeline, orgs,
                            #   stocks, evolution, economy, briefing, marketplace,
                            #   traces, trace detail
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
| **2** | Village | Month 4-6 | 10-100 | Social relations, lifecycle, knowledge base | ✅ Done |
| **3** | City | Month 7-12 | 100-1K | Organizations, complex economy, evolution | ✅ Done |
| **4** | Civilization | Month 13-18 | 1K+ | Self-governance, culture, diplomacy, research tools | 🔄 In Progress |
| **5** | Ecosystem | Month 19+ | inf | Inter-world trade, academic platform | Planned |

### Phase 4 Progress

| Milestone | Feature | Status |
|-----------|---------|--------|
| 4.1 | LLM integration (multi-provider, async, cost tracking) | ✅ |
| 4.2 | Tracing & observability (tick tracing, interaction graphs, emergence metrics) | ✅ |
| 4.3 | Cultural emergence (personality, diffusion, conflict, language experiments) | ✅ |
| 4.4 | Self-governance (treasury, elections, diplomacy, rule proposals) | 🔄 4.4.3 in progress |
| 4.5 | Researcher tools (data export, experiment framework) | 🔄 4.5.3 in progress |
| 4.6 | Demo + open-source promotion | 🔄 This task |

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
