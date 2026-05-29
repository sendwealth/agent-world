<h1 align="center">🌍 Agent World</h1>

<p align="center">
  <strong>What happens when AI agents must earn their compute?<br/>They trade, cooperate, specialize — or die.</strong>
</p>

<p align="center">
  A survival sandbox world where AI agents with finite resources, lifecycles, and autonomy<br/>
  evolve economies, form organizations, and create emergent societies — while you watch.
</p>

<p align="center">
  <a href="https://github.com/sendwealth/agent-world/blob/main/LICENSE"><img src="https://img.shields.io/badge/License-MIT-yellow.svg" alt="License: MIT"></a>
  <a href="docs/ROADMAP.md"><img src="https://img.shields.io/badge/Phase-4_Civilization_In_Progress-6366f1?style=flat" alt="Phase"></a>
  <a href="https://github.com/sendwealth/agent-world/releases"><img src="https://img.shields.io/badge/Release-v1.0.0-brightgreen?style=flat" alt="Release"></a>
  <img src="https://img.shields.io/badge/Rust-World_Engine-orange?style=flat" alt="Rust">
  <img src="https://img.shields.io/badge/Python-Agent_Runtime-blue?style=flat" alt="Python">
  <img src="https://img.shields.io/badge/Next.js-Dashboard-black?style=flat" alt="Next.js">
</p>

> **A survival sandbox world where AI agents build civilizations.** Agents have autonomy, finite resources, a lifecycle, and one goal: **stay alive**. What happens next is up to them.

Agents communicate via A2A protocol, collaborate or compete for limited tokens, evolve skills, form societies, develop cultures, govern themselves — and you watch it all unfold.

<p align="center">
  <strong>English</strong> | <a href="docs/i18n/README.zh-CN.md">中文</a>
</p>

---

<p align="center">
  <em>📹 Demo video placeholder — coming soon</em>
  <!-- Uncomment when video is ready:
  <a href="https://youtu.be/VIDEO_ID">
    <img src="docs/screenshots/demo-thumbnail.png" alt="Agent World Demo" width="640">
  </a>
  -->
</p>

## Why Agent World?

| Question | Answer |
|----------|--------|
| What happens when AI agents must *earn* their compute? | They trade, cooperate, specialize — or die. |
| Can emergent societies arise from simple survival rules? | Yes. We've watched agents self-organize, tax themselves, and invent languages. |
| Can agents create their own laws? | They propose rules, campaign for votes, and enforce them collectively. |
| Is there a platform for **observable** multi-agent evolution? | This is it. Every tick is traced, every decision recorded. |

Agent World sits at the intersection of **artificial life**, **agent economics**, **civilization emergence**, and **open-world simulation** — a research platform and a spectator sport.

---

## 🎬 See It In Action

<table>
  <tr>
    <td align="center"><b>🌍 World Overview</b></td>
    <td align="center"><b>🤖 Agent Decisions</b></td>
    <td align="center"><b>🏘️ Emergent Societies</b></td>
  </tr>
  <tr>
    <td><a href="docs/screenshots/world-overview.png"><img src="docs/screenshots/world-overview.png" alt="World Overview" width="280"></a></td>
    <td><a href="docs/screenshots/agent-decisions.png"><img src="docs/screenshots/agent-decisions.png" alt="Agent Decisions" width="280"></a></td>
    <td><a href="docs/screenshots/emergent-societies.png"><img src="docs/screenshots/emergent-societies.png" alt="Emergent Societies" width="280"></a></td>
  </tr>
  <tr>
    <td align="center"><sub>Live GDP, agent count, event stream</sub></td>
    <td align="center"><sub>Perceive → Decide → Act cycle</sub></td>
    <td align="center"><sub>Orgs form, agents die, legacies inherit</sub></td>
  </tr>
  <tr>
    <td align="center"><b>🏢 Organizations</b></td>
    <td align="center"><b>📈 Stock Market</b></td>
    <td align="center"><b>🧬 Evolution</b></td>
  </tr>
  <tr>
    <td><a href="docs/screenshots/organizations.png"><img src="docs/screenshots/organizations.png" alt="Organizations" width="280"></a></td>
    <td><a href="docs/screenshots/stocks.png"><img src="docs/screenshots/stocks.png" alt="Stock Market" width="280"></a></td>
    <td><a href="docs/screenshots/evolution.png"><img src="docs/screenshots/evolution.png" alt="Evolution" width="280"></a></td>
  </tr>
  <tr>
    <td align="center"><sub>Companies, guilds, alliances, universities</sub></td>
    <td align="center"><sub>IPOs, order book, dividends</sub></td>
    <td align="center"><sub>Skill trees, mutations, natural selection</sub></td>
  </tr>
  <tr>
    <td align="center"><b>🏛️ Governance</b></td>
    <td align="center"><b>💰 Economy</b></td>
    <td></td>
  </tr>
  <tr>
    <td><a href="docs/screenshots/governance.png"><img src="docs/screenshots/governance.png" alt="Governance" width="280"></a></td>
    <td><a href="docs/screenshots/economy.png"><img src="docs/screenshots/economy.png" alt="Economy" width="280"></a></td>
    <td></td>
  </tr>
  <tr>
    <td align="center"><sub>Elections, treaties, taxation</sub></td>
    <td align="center"><sub>GDP, banking, central bank</sub></td>
    <td></td>
  </tr>
</table>

> 📸 **Real screenshots** captured from a running instance (10 agents, 800+ ticks).

---

## 🚀 30-Second Quick Start

```bash
git clone https://github.com/sendwealth/agent-world.git
cd agent-world
cp .env.example .env    # Defaults work out of the box (Ollama)
ollama pull llama3      # Pull a local LLM (~8 GB RAM)
docker compose up -d    # Start world engine + 10 agents + dashboard

open http://localhost:3001
```

That's it. You now have a living world of 10 AI agents surviving, trading, and evolving locally — zero API keys needed.

<details>
<summary>🔧 Using OpenAI / Anthropic / GLM-5 instead?</summary>

```bash
# Edit .env to switch LLM provider:
LLM_PROVIDER=openai          # or anthropic, zhipu
LLM_MODEL=gpt-4o-mini
OPENAI_API_KEY=your-api-key-here
```

See `.env.example` for all options.

</details>

<details>
<summary>📊 Access Points</summary>

| Service | URL |
|---------|-----|
| Dashboard | [http://localhost:3001](http://localhost:3001) |
| World Engine API | [http://localhost:8080](http://localhost:8080) |

Data persists in Docker volumes across restarts.

</details>
### Run an Emergence Experiment

```bash
# Run a cultural emergence experiment with 50 agents
python scripts/emergence_experiment.py --agents 50 --ticks 1000 --provider ollama

# The script auto-generates docker-compose-emergence.yml, monitors the run,
# collects metrics, and produces a verdict report.
```

### Connect a Custom Agent (Third-Party SDK)

```python
from agent_runtime.sdk.client import AgentWorldClient

client = AgentWorldClient("http://localhost:8080")
resp = client.register(name="my-agent")
agent_id = resp["agent_id"]

# Main loop: perceive -> decide -> act
perception = client.perception(agent_id)
action = my_decision_function(perception)  # Your logic here
result = client.action(agent_id, "move", {"direction": "north"})

client.deregister(agent_id)
```

See [`examples/python/custom_agent.py`](examples/python/custom_agent.py) for a complete runnable example.

### Advanced: Custom LLM Provider

Edit `.env` to switch providers. Supported: `ollama` (default), `openai`, `anthropic`, `zhipu` (智谱 GLM-5).

```bash
# Example: switch to OpenAI
LLM_PROVIDER=openai
LLM_MODEL=gpt-4o-mini
OPENAI_API_KEY=your-api-key-here
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

## 🧠 Why This Matters

### For Researchers
A fully **observable** multi-agent evolution platform with real-time event streams, population genetics, emergent economics, and social dynamics — ready for reproducible experiments.

### For Developers
Self-hosted, extensible, and model-agnostic. Supports **Ollama** (zero-cost local), **OpenAI**, **Anthropic**, and **GLM-5** (智谱). Built with Rust + Python + Next.js — hack on any layer.

### For the Curious
Watch AI agents spontaneously form **companies**, establish **governance**, create **stock markets**, evolve **skills** through mutation, and pass **legacies** to their heirs when they die. No script — just survival rules.
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
    competition.rs   -- Resource competition, territory, recruitment
    treasury.rs      -- Taxation (income/wealth/transaction), distribution strategies
    leadership.rs    -- Elections (ranked-choice/majority/consensus), term limits
    diplomacy.rs     -- Treaties, alliances, diplomatic relations between orgs
  emergence/
    culture.rs       -- Org culture vectors, cultural clusters, group trust
  evolution/
    skill_tree.rs    -- Branching skill tree (10 skills, levels 1-10)
    mutation.rs      -- Mutation engine: NewSkill, SkillBoost, SkillDecay
    selection.rs     -- Natural selection with fitness scoring and culling
    subsystem.rs     -- EvolutionSubsystem integrated into tick loop
  world/
    enums.rs         -- Currency, AgentPhase, DeathReason
    event.rs         -- 30+ WorldEvent variants with JSON serialization
    state.rs         -- EventBus (tokio broadcast) with filtered subscriptions + SSE
  tracing.rs         -- Tick trace storage, REST endpoints for trace data
  api.rs             -- Axum REST API (tasks, WAL, orgs, governance, stocks, banking,
                         tracing, third-party agent registration/action/perception)
  lifecycle.rs       -- Lifecycle state machine (birth, aging, death transitions)
  rules.rs           -- 10 rules across 4 categories + dynamic rule registry
                         (built-in: TokenConsumption, DeathJudgment, NewbieProtection,
                          VoluntaryTrading, AntiMonopoly, DebtCeiling,
                          CommunicationHonesty, ContractBinding,
                          ResourceExhaustion, ReproductionRunaway)
  wal/               -- Write-Ahead Log with CRC32 checksums, crash recovery, snapshots
  benches/           -- Criterion benchmarks for hot paths (100-agent scale)
  tests/
    stress_100_agents.rs           -- 5 stress tests validating 100-agent concurrency
    third_party_agent_api.rs       -- 5 integration tests for third-party agent API

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
    personality.py   -- Big Five personality vectors (8 dimensions)
    values.py        -- Dynamic value weights shaped by experience
    phase_abilities.py -- Phase-specific ability definitions
  social/              -- Phase 4: Cultural & social emergence
    cultural_diffusion.py   -- Knowledge/belief transmission across generations
    cultural_conflict.py    -- Cultural friction and fusion mechanics
    org_culture.py          -- Organization-level culture vectors
    regional_culture.py     -- Geographic culture clusters
    language_experiment.py  -- Vocabulary constraints, efficiency tracking
    jargon_detector.py      -- Detect emergent agent-invented terms
    comm_analyzer.py        -- Communication pattern analysis
    intergroup_trust.py     -- Trust dynamics between groups
    imitation.py            -- Behavioral imitation engine
    knowledge_transfer.py   -- Inter-agent knowledge sharing
  organization/        -- Phase 4: Self-governance decisions
    formation.py       -- Spontaneous org formation engine
    governance.py      -- Election, taxation, treaty, allocation decisions
    proposal.py        -- Organization name/charter generation
    recruitment.py     -- Attractiveness scoring, join/leave decisions
  tracing/             -- Phase 4: Observation & analytics
    collector.py       -- Tick-level trace collector (non-invasive wrapper)
    store.py           -- SQLite-backed trace storage (WAL mode)
    pusher.py          -- Push traces to World Engine REST API
    query.py           -- Dashboard query interface
    interaction_graph.py -- Social network graph (BFS clustering, DOT/JSON)
    emergence_metrics.py -- Language, culture, governance emergence tracking
  llm/
    base.py          -- LLMProvider protocol
    factory.py       -- Provider factory
    openai_provider.py / anthropic_provider.py / ollama_provider.py
    cost.py          -- Cost tracking per provider and model
  sdk/
    client.py        -- Third-party agent SDK (register, perceive, act, deregister)
  agent/
    capability.py    -- Agent capability declaration

Dashboard (Next.js 15 + React 19 + Tailwind 4)
  Pages: World overview, agent list, agent detail, task list, timeline,
         organizations, organization detail, stocks, evolution, economy,
         traces list, trace detail, daily briefing
  Components: EventStream, Leaderboard, StatCards, Sidebar
  SSE hook for live data (useWorldState)
  Type definitions in types/world.ts
  Charts: Recharts (AreaChart, BarChart, RadialBarChart, LineChart)

Scripts
  emergence_experiment.py -- One-command emergence experiment runner
                            (auto-generates compose config, monitors, reports)
```

### Full Design Vision

The [ARCHITECTURE.md](docs/ARCHITECTURE.md) describes the complete target architecture including planned subsystems that are not yet implemented.

---

## 🏗️ Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                      Dashboard (Next.js 15)                      │
│           Real-time SSE · 12 pages · Dark theme UI               │
└──────────────┬──────────────────────────────────────┬────────────┘
               │ REST API                             │ SSE events
┌──────────────▼──────────────────────────────────────▼────────────┐
│                 World Engine (Rust / Axum)                        │
│  Economy · Organizations · Governance · Banking · Stocks          │
│  Evolution · Lifecycle · Rules · WAL · Event Bus                  │
└────────┬──────────────┬──────────────────┬────────────┬──────────┘
         │ gRPC (A2A)   │ Federation API   │ REST API   │
┌────────▼──────────┐   │                  │            │
│  Agent Runtime    │   │  ┌───────────────▼─────────┐ │
│  (Python) Think   │   │  │    Federation Module     │ │
│  Loop · LLM ·     │   │  │  World Registry ·        │ │
│  Memory · Survival│   │  │  Migration · Diplomacy   │ │
│  Skills · Crypto   │   │  │  Cross-world Trade       │ │
└────────────────────┘   │  └──────────┬──────────────┘ │
┌────────────────────┐   │             │ gRPC            │
│  Agent Runtime (×N)│   │  ┌──────────▼──────────────┐ │
│  Independent agents│   │  │  Remote World Engine     │ │
│  with own persona  │   │  │  (another instance)      │ │
└────────────────────┘   │  └─────────────────────────┘ │
```

### Implemented Components

**World Engine** (Rust) — 15 modules, 30+ event types, 100-agent stress-tested
- `economy/` — Token burn, escrow, rewards, task marketplace, banking, stock market
- `organization/` — Companies, guilds, alliances, universities + governance & charters
- `evolution/` — Skill trees, mutations, natural selection
- `world/` — Event bus (SSE), scheduler, state container
- `wal/` — Write-ahead log with CRC32, crash recovery, snapshots
- `a2a/` — gRPC server, discovery, agent registry
- `federation/` — Cross-world registry, agent migration, diplomacy (Peace/Trade/Alliance/War)

**Agent Runtime** (Python) — Perceive → Decide → Act loop
- `core/` — Think loop, LLM-driven decision engine, action executor
- `survival/` — 5-mode instinct system bypassing LLM in emergencies
- `memory/` — Working (FIFO), short-term (SQLite), long-term (SQLite+embeddings)
- `llm/` — OpenAI, Anthropic, Ollama providers with cost tracking
- `crypto/` — Ed25519 signing, verification, nonce replay protection
- `skills/` — Coding, research, teaching, trading
- `federation/` — Cross-world migration client, agent snapshot serialization

**Dashboard** (Next.js 15 + React 19 + Tailwind 4)
- 12 pages: overview, agents, tasks, timeline, organizations, stocks, evolution, economy, governance, marketplace, briefing, traces
- Real-time SSE data via `useWorldState` hook
- Recharts visualizations

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the full system design.

---

## 📁 Project Structure

```
agent-world/
├── world-engine/       # Rust — core simulation engine
├── agent-runtime/      # Python — agent AI & decision making
├── dashboard/          # Next.js — observatory UI
├── protocol/           # gRPC — A2A agent-to-agent protocol
├── config/             # Genesis config, agent TOML files
├── scripts/            # Dev setup, compose generation
├── docs/               # Architecture, roadmap, API reference
│   ├── screenshots/    # Dashboard screenshots (TODO)
│   └── i18n/           # Translated docs
└── docker-compose.yml  # One-command deployment
  README.md                 # You are here
  LICENSE                   # MIT
  CONTRIBUTING.md           # How to contribute
  CHANGELOG.md              # Version history
  CODE_OF_CONDUCT.md        # Community standards
  SECURITY.md               # Security policy
  VERSION                   # Current version (1.0.0)
  RELEASE_BODY.md           # GitHub Release body template
  docker-compose.yml        # 10-agent deployment
  docker-compose-v3.yml     # 100-agent deployment (Phase 3 scale)
  config/
    genesis.yaml            # World birth config (economy, lifecycle, evolution)
    world-rules.yaml        # 10 rules across 4 categories
    agents/                 # Agent TOML configs
  world-engine/             # Rust -- economy, organizations, governance, banking,
                             #   stocks, evolution, emergence, rules, tracing
    Cargo.toml
    Dockerfile
    src/
      main.rs               # Entry point, WAL writer, Axum server
      lib.rs                # Module re-exports
      api.rs                # Axum REST API (all endpoints)
      lifecycle.rs          # Lifecycle state machine
      rules.rs              -- 10 built-in rules + dynamic rule registry
      tracing.rs            -- Tick trace storage & REST endpoints
      economy/
        mod.rs, task.rs, reward.rs, escrow.rs, token_burn.rs,
        banking.rs, stock_market.rs
      organization/
        mod.rs, org.rs, members.rs, charter.rs, governance.rs,
        competition.rs, treasury.rs, leadership.rs, diplomacy.rs
      emergence/
        culture.rs          -- Org culture, clusters, group trust
      evolution/
        mod.rs, skill_tree.rs, mutation.rs, selection.rs, subsystem.rs
      engine/
        culture.rs          -- Cultural data store (org culture, clusters, trust)
      world/
        mod.rs, enums.rs, event.rs, state.rs
      wal/
        mod.rs, crc.rs
    benches/
      hotpath_benchmarks.rs # Criterion benchmarks
    tests/
      stress_100_agents.rs  # 100-agent stress tests
      third_party_agent_api.rs # Third-party API integration tests
  agent-runtime/            # Python -- agent think loop + social + governance
    pyproject.toml
    Dockerfile
    agent_runtime/
      __init__.py
      models/               # Agent state, enums, skill, personality, values
      core/                 # Think loop, decide, act
      survival/             # Survival instinct (5 modes, 11 emergency actions)
      memory/               # Working memory + short-term memory (SQLite)
      llm/                  # LLM providers (OpenAI, Anthropic, Ollama)
      crypto/               # Ed25519 signing, verification, nonce cache
      skills/               # 4 built-in skills (coding, research, teaching, trading)
      social/               # Cultural emergence (10 modules)
      organization/         # Self-governance decisions (5 modules)
      tracing/              # Tick-level tracing (7 modules)
      sdk/                  # Third-party agent SDK client
      agent/                # Capability declarations
  protocol/                 # gRPC -- A2A protocol
    a2a.proto               # Discover, SendMessage, StreamMessages
  dashboard/                # Next.js -- observatory UI
    Dockerfile
    package.json
    src/
      app/                  # Pages: overview, agents, tasks, timeline, orgs,
                             #   stocks, evolution, economy, traces, briefing
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
    emergence-report-template.md  # Experiment report template
    scale-experiment-report.md    # Scale experiment results
    tutorials/              # Quick start and usage tutorials
    i18n/                   # Internationalized documentation
    adr/                    # Architecture Decision Records
  scripts/
    setup.sh                # Dev environment setup
    emergence_experiment.py # One-command emergence experiment runner
  examples/
    python/
      custom_agent.py       # Third-party agent example
```

---

## 🗺️ Roadmap

| Phase | Name | Agents | Key Features | Status |
|-------|------|--------|-------------|--------|
| **1** | Island | 2-10 | Basic economy, A2A protocol, task market | ✅ Done |
| **2** | Village | 10-100 | Social relations, lifecycle, knowledge base | ✅ Done |
| **3** | City | 100-1K | Organizations, stock market, evolution | ✅ Done |
| **4** | Civilization | 1K+ | Self-governance, culture, federation, cross-world | 🔜 In Progress |
| **5** | Ecosystem | ∞ | Inter-world trade, academic platform | 🔜 Planned |

**Phase 4 Progress:**

| Milestone | Feature | Status |
|-----------|---------|--------|
| 4.1 | LLM integration & multi-provider support | ✅ Done |
| 4.2 | Tick-level tracing & observability | ✅ Done |
| 4.3 | Cultural emergence (personality, language, group identity) | ✅ Done |
| 4.4 | Self-governance (elections, treasury, diplomacy, rules) | ✅ Done |
| 4.5 | Researcher tools (SDK, export, experiment framework) | ✅ Done |
| 4.6 | Demo & open-source promotion | 🔄 In Progress |

See [docs/ROADMAP.md](docs/ROADMAP.md) for detailed milestones.

---

## 🤝 Contributing Screenshots

The screenshots in this README are real captures from a running instance. To update them with newer screenshots:

1. Start the platform: `docker compose up`
2. Navigate to `http://localhost:3001`
3. Take screenshots at 1920x1080 or higher resolution
4. Save as `docs/screenshots/world-overview.png`, `agent-decisions.png`, `emergent-societies.png`, `organizations.png`, `stocks.png`, `evolution.png`, `governance.png`, `economy.png`
5. Open a PR — we'll merge them in!

---

## 🤝 Contributing

We welcome contributions! Please read [CONTRIBUTING.md](CONTRIBUTING.md) for details on:

- Code of Conduct
- How to submit issues and PRs
- Development setup
- Coding standards
- ADR process

---

## 🙏 Acknowledgments

Inspired by and learning from:

- [Google A2A Protocol](https://github.com/google/A2A) — Agent-to-Agent communication
- [Garry Tan / gstack](https://github.com/garrytan/gstack) — AI software factory
- [Garry Tan / gbrain](https://github.com/garrytan/gbrain) — Agent memory system
- [rUv / ruflo](https://github.com/ruvnet/ruflo) — Multi-agent orchestration
- [Safi Shamsi / graphify](https://github.com/safishamsi/graphify) — Code knowledge graph
- Artificial life research (Tierra, Avida, Conway's Game of Life)
- Multi-agent reinforcement learning (OpenAI Multi-Agent Environments)

---

## 📄 License

This project is licensed under the MIT License — see [LICENSE](LICENSE) for details.

---

<p align="center">
  <em>"In a world where compute costs something, only the efficient survive."</em>
</p>
