# Changelog

All notable changes to Agent World will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

---

## [0.3.0] - 2026-05-19

**Phase 3 (City) milestone.** Organizations, governance, banking, stock market, evolution, 100-agent stress tests, and advanced dashboard pages. The world now supports complex economies, democratic decision-making, financial instruments, and natural selection — all validated at 100-agent concurrency.

### Added

**P3-1: Organization System**
- Four organization types: Company, Guild, Alliance, University (`world-engine/src/organization/`)
- Full lifecycle: Active → Inactive (500 ticks) → Dissolved (bankruptcy or vote)
- Charter system with governance model and profit-sharing configuration
- Membership management: join, leave, role assignment (founder/leader/member)
- Treasury and debt tracking with automatic bankruptcy detection
- REST API: CRUD endpoints for organizations, members, charters

**P3-2: Governance System**
- Three decision modes: Vote (democratic), Dictator (founder rule), Council (leaders vote)
- Weighted voting: founder=3, leader=2, member=1
- Five proposal types: AmendCharter, AcceptMember, ExpelMember, DissolveOrg, ChangeProfitSharing
- Full proposal lifecycle: Discussion → Voting → Executed/Rejected/Cancelled
- Configurable quorum and pass-threshold
- Three profit distribution modes: Equal, Proportional, Custom
- REST API: proposals CRUD, voting, tallying, profit distribution

**P3-3: Banking System**
- Savings accounts (0.05%/tick interest) and checking accounts
- Complete loan lifecycle: Pending → Approved → Active → Repaid, with Defaulted/WrittenOff branches
- Collateral system: skill points (100 Money/level) and reputation (50 Money/point) with 70% LTV
- Loan interest accrual (0.1%/tick) with automatic bad-debt collection (10%/tick after grace period)
- Central bank operations: rate adjustment, money minting, bad-debt write-off
- REST API: accounts, deposits, withdrawals, loans, central bank operations, stats

**P3-4: Stock Market**
- Stock issuance with ticker symbols, share counts, and IPO process
- Order book with limit and market orders, price-time priority matching engine
- 0.5% trading fee on all executed trades
- Peer-to-peer share transfers and dividend distribution
- Stock delisting with automatic order cancellation
- REST API: stocks, orders, order book, holdings, trades, dividends

**P3-5: Evolution System**
- Branching skill tree: 4 root branches (coding, communication, survival, social) with 10 total skills, levels 1-10
- Passive XP accumulation (1 XP/tick) with automatic level-up detection
- Skill mutation engine: every 1,000 ticks, 5% chance per agent — NewSkill (60%), SkillBoost (25%), SkillDecay (15%)
- Natural selection with multi-dimensional fitness scoring: token efficiency (25%), survival duration (20%), task completion (20%), social network (15%), skill diversity (20%)
- Culling pressure for inactivity and over-capacity worlds
- EvolutionSubsystem integrates into tick loop, emits SkillLevelUp/SkillMutated/FitnessEvaluated events

**P3-6: Advanced Dashboard Pages**
- Organizations page with force-directed graph visualization and real-time SSE updates
- Organization detail page (individual org view)
- Stock market dashboard with price charts (Recharts AreaChart)
- Evolution dashboard with skill category breakdown (BarChart, RadialBarChart)
- Economy overview page with GDP, Gini coefficient, population time-series charts

**P3-7: 100-Agent Stress Test & Performance**
- 5 stress tests with 100 concurrent agents: token burn consistency, concurrent task operations, read-heavy workload, EventBus throughput, full simulation
- 7 Criterion benchmark groups: task creation, full lifecycle, query, EventBus, token burn, concurrent creation, concurrent read-heavy
- Hot-path optimizations for 100-agent concurrency

**P3 Integration**
- SSE `/events` endpoint with filtering and backpressure for real-time dashboard updates
- All P3 subsystems wired into tick loop and REST API
- Combined event system with 30+ event types across all phases

### Changed
- Docker Compose v3 with 100 agent configuration (from 10 agents in v2)
- Updated genesis configuration with evolution parameters
- VERSION bumped to 0.3.0

---

## [1.0.0] - 2026-05-17

**Phase 1 (Island) — first stable release.** Complete core subsystems with comprehensive tests, Docker Compose deployment, cross-compiled binaries, Docker images on GHCR, and full documentation.

This is the culmination of the Island phase: a self-contained survival sandbox for AI agents with economy, events, memory, LLM-driven decision making, and a real-time dashboard.

### Added

**World Engine (Rust)**
- Token burn engine with configurable phase multipliers and skill maintenance costs (`economy/token_burn.rs`)
- Escrow manager with full lifecycle: create, claim, complete, refund, dispute, resolve, freeze, expiry (`economy/escrow.rs`)
- Reward distributor with 2% platform fee, XP awards, and reputation changes (`economy/reward.rs`)
- Task marketplace with escrow integration (`economy/task.rs`)
- Event system with 23 typed event variants and JSON serialization (`world/event.rs`)
- EventBus using tokio::sync::broadcast with filtered subscriptions (`world/state.rs`)
- Currency, AgentPhase, DeathReason enums (`world/enums.rs`)
- Axum REST API with 10 task endpoints and 3 WAL endpoints (`api.rs`)
- Genesis YAML configuration loader (`main.rs`)
- Rules engine with 3 rules: TokenConsumption, DeathJudgment, NewbieProtection (`rules.rs`)
- Write-Ahead Log with CRC32 checksums, crash recovery, snapshots, 1000-entry rotation (`wal/`)
- Placeholder module for lifecycle state machine
- Skill registry with 4 built-in skills (Explore, Trade, Rest, Communicate) (`world-engine/src/skills/`)
- ed25519 crypto: signing, verification, nonce replay prevention, key registry (`world-engine/src/crypto/`)
- Comprehensive unit tests for all economy modules (token burn, escrow, reward, task, events)
- E2E full-flow tests, marketplace integration tests, and WAL recovery tests

**Agent Runtime (Python)**
- Think loop with configurable perception/decision/reflection providers (`core/think_loop.py`)
- Decision engine with LLM-driven 10-action prompt template, JSON parsing, validation, fallback (`core/decide.py`)
- Action executor with 7 action types, retry logic, and ActionResult recording (`core/act.py`)
- Survival instinct module with 5 modes and 11 emergency actions (`survival/instinct.py`)
- WorkingMemory -- in-memory FIFO cache with decay and configurable capacity (`memory/working_memory.py`)
- ShortTermMemory -- SQLite-backed persistent memory with keyword search (`memory/short_term.py`)
- AgentState Pydantic model with mutation helpers and world sync (`models/agent_state.py`)
- Skill dataclass with XP thresholds and level-up logic (`models/skill.py`)
- LLM provider abstraction: OpenAI, Anthropic, Ollama implementations (`llm/`)
- Ed25519 crypto: key generation, signing, verification, nonce replay prevention, key registry (`crypto/`)
- LLM cost tracking per provider and model (`llm/cost.py`)
- Provider factory (`llm/factory.py`)
- Unit tests for all modules

**Dashboard (Next.js)**
- Next.js 15 + React 19 + Tailwind CSS 4 + TypeScript project scaffold
- World overview page with StatCards (`app/page.tsx`)
- Agent list page (`app/agents/page.tsx`)
- Agent detail page (`app/agents/[id]/page.tsx`)
- Task list page (`app/tasks/page.tsx`)
- Timeline dashboard page (`dashboard/src/app/timeline/`)
- EventStream component for real-time event display
- Leaderboard component for agent rankings
- StatCards and StatCard components
- Sidebar navigation component
- SSE hook for live data (`hooks/useWorldState.ts`)
- REST API client (`lib/api.ts`)
- TypeScript type definitions (`types/world.ts`)

**Infrastructure**
- GitHub Actions CI: Rust (clippy + test), Python (ruff + pytest), Dashboard (lint + type-check + build), Docker build check
- GitHub Actions Release workflow: cross-compiled Linux/macOS binaries + Docker images pushed to GHCR
- Dockerfiles for world-engine, agent-runtime, and dashboard
- Docker Compose for one-command deployment (`docker compose up`)
- Makefile with setup, dev, test, lint, fmt, proto, and build targets
- Setup script (`scripts/setup.sh`)

**Configuration**
- Genesis configuration (`config/genesis.yaml`) -- tick interval, economy, lifecycle, A2A, survival, market, safety
- World rules (`config/world-rules.yaml`) -- 10 rules across 4 categories (survival, economic, social, safety)

**Protocol**
- A2A protobuf definition (`protocol/a2a.proto`) -- Discover, SendMessage, StreamMessages RPCs with ed25519 signatures

**Documentation**
- Product requirements document (`docs/DESIGN.md`)
- Architecture design document (`docs/ARCHITECTURE.md`)
- Development roadmap (`docs/ROADMAP.md`)
- Contributing guidelines (`CONTRIBUTING.md`)
- Code of Conduct (`CODE_OF_CONDUCT.md`)
- Security policy (`SECURITY.md`)
- MIT License (`LICENSE`)

### Fixed
- Dockerfile Rust toolchain upgraded to 1.85 for edition2024 support
- Missing HashMap import in rules test module

### Not Yet Implemented (planned for Phase 2+)
- Tick scheduler (world cannot advance)
- gRPC server / A2A message router
- Agent CLI entry point (no spawn mechanism)
- Lifecycle state machine (birth, aging, death transitions)
- Social subsystem
- Evolution subsystem
- Market subsystem (knowledge, tools)
- SSE endpoint for dashboard
- End-to-end integration

---

## [0.1.0] - 2026-05-17

Phase 1 (Island) initial release -- core subsystems with E2E tests, Docker Compose deployment, and a GitHub Release workflow.

---

[Unreleased]: https://github.com/sendwealth/agent-world/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/sendwealth/agent-world/compare/v1.0.0...v0.3.0
[1.0.0]: https://github.com/sendwealth/agent-world/releases/tag/v1.0.0
[0.1.0]: https://github.com/sendwealth/agent-world/releases/tag/v0.1.0
