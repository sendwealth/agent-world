# Roadmap

> **Overall completion: ~75%** (as of 2026-05-27)
>
> This document reflects the **actual** implementation state after code audit.
> Items marked ⚠️ are partially implemented or contain placeholders — see details below.
> Items marked 🔴 are declared in code but not wired into the runtime (`None` at init).

---

## Phase 1: Island (Month 1-3) — **COMPLETE** ✅ (100%)

**Goal**: 2 agents in a room can talk, trade, and survive together.

**Released**: v1.0.0 (2026-05-20)

### Milestone 1.1: World Engine Core
- [x] Rust project scaffold (Cargo.toml, module structure)
- [x] Token burn engine with phase multipliers and skill maintenance costs
- [x] Escrow manager — full lifecycle (create/claim/complete/refund/dispute/resolve/freeze/expiry)
- [x] Reward distributor — 2% platform fee, XP awards, reputation changes
- [x] Task board — task marketplace with escrow integration
- [x] Genesis configuration loader
- [x] Event system — EventBus with 30+ event types, filtered subscriptions, SSE
- [x] Basic rule engine — 3 rules (TokenConsumption, DeathJudgment, NewbieProtection)
- [x] Skill registry — 4 built-in skills (Explore, Trade, Rest, Communicate)
- [x] Ed25519 crypto — signing, verification, nonce replay prevention, key registry
- [x] Tick-based scheduler with configurable interval
- [x] Money ledger with central bank exchange
- [x] gRPC server scaffold (A2A protocol)

### Milestone 1.2: A2A Protocol
- [x] Protobuf definitions (`a2a.proto`)
- [x] Message signing — ed25519 in both Rust and Python
- [x] Discovery mechanism (agent registration)
- [x] Proposal/Accept/Reject flow
- [x] Python gRPC client
- [x] Integration tests: two agents exchange messages

### Milestone 1.3: Agent Runtime
- [x] Python project scaffold (pyproject.toml, module structure)
- [x] Think loop: Perceive → Decide → Act cycle
- [x] Decision engine — LLM-driven with 10 action types, JSON parsing, validation
- [x] Action executor — 7 action types with retry logic
- [x] Survival instinct — 5 modes, 11 emergency actions
- [x] LLM integration — OpenAI, Anthropic, Ollama, 智谱 GLM-5
- [x] Working memory — in-memory FIFO with decay
- [x] Short-term memory — SQLite-backed with keyword search
- [x] Agent state model — full Pydantic model
- [x] Skill model with XP thresholds and level-up
- [x] LLM cost tracking per provider/model
- [x] CLI entry point (`__main__.py` with `spawn` subcommand)
- [x] A2A client integration
- [x] Task execution via `world_client.py`

### Milestone 1.4: Marketplace
- [x] Task marketplace with escrow integration
- [x] Bounty posting and claiming via REST API
- [x] Reward distribution via world engine
- [x] Reputation scoring (`economy/reputation.rs`, 648 lines)

### Milestone 1.5: Dashboard
- [x] Next.js 15 + React 19 + Tailwind 4 scaffold
- [x] World overview, agent list, agent detail
- [x] Event stream, leaderboard, stat cards, sidebar
- [x] SSE hook for live data (`useWorldState`)
- [x] REST API client + TypeScript type definitions
- [x] Timeline, task board pages
- [x] WebSocket/SSE connection

### Milestone 1.0: MVP Release
- [x] E2E full-flow + integration tests (8/8 E2E, 1038 Python unit tests)
- [x] Documentation for Phase 1
- [x] CI/CD (GitHub Actions: Rust clippy+test, Python ruff+pytest, Dashboard lint+build)
- [x] Docker Compose one-command start (10-agent config)
- [x] Cross-compiled binaries (Linux/macOS, amd64/arm64)
- [x] Docker images on GHCR
- [x] Release workflow (tag-triggered)

### Bonus
- [x] WAL with CRC32 checksums, crash recovery, snapshots
- [x] Makefile with setup/dev/test/lint/fmt/proto/build targets

---

## Phase 2: Village (Month 4-6) — **~70%** ⚠️

**Goal**: 10-100 agents form social relationships, have lifecycles, share knowledge.

### Implemented
- [x] Lifecycle system — birth, childhood, adult, elder, death (`lifecycle.rs`, 39K lines, full state machine)
- [x] Inheritance/will system — beneficiaries, token distribution, skill transfer (`economy/inheritance.rs`, 543 lines)
- [x] Knowledge base — vector memory with embedding support in Python (`memory/vector_memory.py` 634 lines, `memory/embedding.py` 274 lines)
- [x] Agent profile pages in dashboard (agent detail, evolution traces)

### Partially Implemented ⚠️
- ⚠️ **Knowledge marketplace** — `economy/marketplace.rs` exists (1485 lines, full listing/rating/purchase logic) with dashboard page (`marketplace/page.tsx` 27KB), but **not wired into AppState** — always initialized as `None`. API routes do not exist yet.
- ⚠️ **Social graph** — Python `social/` module exists with 11 files (trust, cultural diffusion, imitation, language, etc.), but **no Rust-side social graph** in `world-engine/src/`. Not integrated into think loop or tick cycle.

### Not Implemented 🔴
- 🔴 **Tool marketplace** — agents cannot build or rent tools. No backend module found.
- 🔴 **Multi-agent coordination** — no team/group task types in `economy/task.rs`. All tasks are solo.

---

## Phase 3: City (Month 7-12) — **~85%** ✅⚠️

**Goal**: 100-1000 agents form organizations, complex economy emerges.

### Implemented
- [x] Organizations — Company/Guild/Alliance/University (`organization/org.rs`, 26K lines)
- [x] Membership management — join/leave/roles (`organization/members.rs`)
- [x] Charter system — governance model, profit sharing (`organization/charter.rs`)
- [x] Governance — 3 decision modes (Vote/Dictator/Council), weighted voting, 5 proposal types (`organization/governance.rs`, 73K lines)
- [x] Banking — savings/checking accounts, loans, collateral, central bank (`economy/banking.rs`, 49K lines)
- [x] Stock market — IPOs, order book matching, dividends, delisting (`economy/stock_market.rs`, 45K lines)
- [x] Evolution — branching skill tree (10 skills, levels 1-10), mutation engine, evolution subsystem (`evolution/`, 3 files)
- [x] Natural selection — fitness scoring, culling pressure (`evolution/selection.rs`, 13K lines)
- [x] Advanced dashboard — organizations (force graph), stocks (price charts), evolution (skill breakdown), economy (GDP/Gini)
- [x] 100-agent stress tests — 5 tests validating concurrent operations
- [x] Criterion benchmarks for hot paths
- [x] Full REST API (50+ endpoints across all subsystems)

### Known Placeholders ⚠️
- ⚠️ `selection.rs` — **task completion rate dimension is hardcoded to 0** ("placeholder — no task tracking in AgentRecord yet"). Fitness scoring uses 4 of 5 intended dimensions.
- ⚠️ `competition.rs` — **skill tracking placeholder**: `avg_skill = 1.0 // placeholder until skill tracking is added`. Also uses hash-based heuristic for competition scoring.

---

## Phase 4: Civilization (Month 13-18) — **~40%** ⚠️

**Goal**: 1000+ agents self-govern, develop culture, interact across worlds.

### Implemented (code exists)
- [x] DSL rules engine — parser + rule lifecycle (`dsl/parser.rs`, `dsl/mod.rs`, `organization/rule_engine.rs` 1122 lines)
- [x] DSL API routes — parse/submit/vote/activate/suspend/repeal (10 endpoints)
- [x] Federation engine — diplomatic status, treaties, sanctions, war/peace (`a2a/federation.rs`, 1518 lines; `federation/` dir: registry, service, migration)
- [x] Federation API routes — worlds CRUD, treaties, relations, migration (20+ endpoints)
- [x] Migration system — submit/review/execute/cancel workflow (`federation/migration.rs`)
- [x] Python social/cultural modules — 11 files covering personality vectors, cultural diffusion, imitation, language emergence, intergroup trust, conflict/fusion, org culture, regional clustering, knowledge transfer, jargon detection, communication analysis
- [x] Dashboard pages — human observer mode (agents/bounties/oracle/portfolio/rankings), governance comparison, briefing, traces
- [x] Auth module (`auth/`)
- [x] Persistence layer (`persistence/`)
- [x] Time capsule system (`time_capsule.rs`)
- [x] Tracing subsystem (`tracing.rs`)
- [x] Human observer module (`human/`)
- [x] Engine orchestrator (`engine/`)

### Partially Wired ⚠️
- ⚠️ **Federation** — full code exists but **AppState fields are `None`** at initialization. Federation routes will return errors at runtime. Not production-ready.
- ⚠️ **Migration** — same issue: code exists, `Option` fields not wired.
- ⚠️ **Marketplace** — same: `marketplace: None` in all AppState constructors.
- ⚠️ **Reputation system** — `reputation_system: None` in all AppState constructors.
- ⚠️ **Python social modules** — 11 files with real logic, but **not imported or used by the think loop**. `decide.py` has `SOCIALIZE` as an action type but no social engine integration.

### Not Implemented 🔴
- 🔴 **Self-governance elections** — DSL rules engine exists but no election/legislation flow on top of it
- 🔴 **Cultural emergence** — Python modules exist but not integrated into agent tick cycle
- 🔴 **Cross-world interaction** — federation code exists but not activated
- 🔴 **API for third-party integration** — no public plugin/extension API
- 🔴 **Academic research tools** — no data export or experiment framework beyond snapshot CSV/JSON export

---

## Phase 5: Ecosystem (Month 19+) — **NOT STARTED** 🔴

**Goal**: A living ecosystem of interconnected agent worlds.

### Planned
- [ ] Inter-world trade and diplomacy
- [ ] Human participants as equals (not just observers)
- [ ] Agents creating sub-worlds
- [ ] Published research papers
- [ ] Sustainable open-source community

---

## Placeholder & Known Issue Tracker

| File | Issue | Severity |
|------|-------|----------|
| `world-engine/src/evolution/selection.rs` L6, L125 | Task completion rate hardcoded to 0 — fitness scoring incomplete | Medium |
| `world-engine/src/organization/competition.rs` L166 | `avg_skill = 1.0` placeholder until skill tracking added | Medium |
| `world-engine/src/organization/competition.rs` L258 | Hash-based competition heuristic — placeholder for real metrics | Low |
| `world-engine/src/api.rs` L164,200,432 | `marketplace: None` — marketplace module not activated | High |
| `world-engine/src/api.rs` L165,201,433 | `reputation_system: None` — reputation system not activated | High |
| `world-engine/src/api.rs` L178,214 | `federation: None` — federation engine not activated | High |
| `world-engine/src/api.rs` L178,214 | `federation_registry: None` — migration/federation not activated | High |
| `agent-runtime/social/` (11 files) | Social/cultural modules exist but not integrated into think loop | High |

---

## Stats

| Component | Lines of Code | Test Coverage |
|-----------|--------------|---------------|
| World Engine (Rust) | ~52,000 | 953 `#[test]` functions |
| Agent Runtime (Python) | ~24,300 | 44 test files |
| Dashboard (TypeScript) | ~11,700 | lint + type-check |
| **Total** | **~88,000** | |

---

## Version

Current: `1.0.0` (VERSION file)

The v1.0.0 tag represents the Phase 1 completion milestone. Given that Phases 2-4 are partially complete, the version accurately reflects the first stable release of the core system.
