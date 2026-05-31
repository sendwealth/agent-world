# Roadmap

> **Overall completion: ~87%** (as of 2026-05-30, after social module wiring)
>
> This document reflects the **actual** implementation and wiring state.
> Items marked ⚠️ are partially implemented — see details below.
> Items marked 🔴 are not yet implemented.

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

## Phase 2: Village (Month 4-6) — **~92%** ✅

**Goal**: 10-100 agents form social relationships, have lifecycles, share knowledge.

### Implemented ✅
- [x] Lifecycle system — birth, childhood, adult, elder, death (`lifecycle.rs`, 39K lines, full state machine)
- [x] Inheritance/will system — beneficiaries, token distribution, skill transfer (`economy/inheritance.rs`, 543 lines)
- [x] Knowledge base — vector memory with embedding support in Python (`memory/vector_memory.py` 634 lines, `memory/embedding.py` 274 lines)
- [x] Agent profile pages in dashboard (agent detail, evolution traces)
- [x] Knowledge marketplace — `economy/marketplace.rs` (1485 lines) **wired into AppState** with 11 API routes, dashboard page (`marketplace/page.tsx`)
- [x] Social context protocol — `decide.py` defines `SocialContextProvider` and `SocialContext` dataclass
- [x] **Social graph — wired into think loop** — `DefaultSocialContextProvider` (`social/provider.py`) implements the `SocialContextProvider` protocol, wraps `SocialEngine` which aggregates all 12 social modules (trust, cultural diffusion, imitation, language, etc.). Injected into `ThinkLoop` via `social_context_provider` parameter, which propagates to `DecisionEngine` for prompt injection. E2E tests confirm social context (trust scores, recommended targets, personality description) flows through the full Perceive → Decide → Act pipeline.

### Not Implemented 🔴
- 🔴 **Tool marketplace** — agents cannot build or rent tools. No backend module found.
- 🔴 **Multi-agent coordination** — no team/group task types in `economy/task.rs`. All tasks are solo.

---

## Phase 3: City (Month 7-12) — **~95%** ✅

**Goal**: 100-1000 agents form organizations, complex economy emerges.

### Implemented ✅
- [x] Organizations — Company/Guild/Alliance/University (`organization/org.rs`, 26K lines)
- [x] Membership management — join/leave/roles (`organization/members.rs`)
- [x] Charter system — governance model, profit sharing (`organization/charter.rs`)
- [x] Governance — 3 decision modes (Vote/Dictator/Council), weighted voting, 5 proposal types (`organization/governance.rs`, 73K lines)
- [x] Banking — savings/checking accounts, loans, collateral, central bank (`economy/banking.rs`, 49K lines)
- [x] Stock market — IPOs, order book matching, dividends, delisting (`economy/stock_market.rs`, 45K lines)
- [x] Evolution — branching skill tree (10 skills, levels 1-10), mutation engine, evolution subsystem (`evolution/`, 3 files)
- [x] Natural selection — fitness scoring uses real tracked data: `tasks_completed/tasks_attempted`, token efficiency, survival duration, social proxy, skill diversity (`evolution/selection.rs`)
- [x] Resource competition — uses real member skill data from world state; falls back to 1.0 only for empty orgs (`organization/competition.rs`)
- [x] Advanced dashboard — organizations (force graph), stocks (price charts), evolution (skill breakdown), economy (GDP/Gini)
- [x] 100-agent stress tests — 5 tests validating concurrent operations
- [x] Criterion benchmarks for hot paths
- [x] Full REST API (50+ endpoints across all subsystems)

---

## Phase 4: Civilization (Month 13-18) — **~75%** ⚠️

**Goal**: 1000+ agents self-govern, develop culture, interact across worlds.

### 4.1 LLM Integration ✅
- [x] Multi-provider support (OpenAI, Anthropic, Ollama, 智谱 GLM-5)
- [x] Async decision engine for concurrent LLM calls
- [x] LLM cost tracking and queue management
- [x] Decision logging and prompt templates

### 4.2 Tracing & Observability ✅
- [x] Tick-level tracing collection (perception → decision → action → reflection) — `TraceStore` wired into AppState, 4 API routes
- [x] Interaction graph construction (social network)
- [x] Emergence detection metrics
- [x] SQLite tracing store with query interface
- [x] Dashboard traces page (per-agent, per-tick drill-down)

### 4.3 Cultural Emergence ✅
- [x] Big Five personality vectors (`models/personality.py`)
- [x] Organization culture modeling (`engine/culture.rs`) — wired, used by competition module
- [x] Cultural diffusion — regional and organizational value convergence (Python, wired via `SocialEngine` → `DefaultSocialContextProvider` → think loop)
- [x] Cultural conflict detection and resolution (Python, wired via `SocialEngine`)
- [x] Regional culture cluster detection (Python, wired via `SocialEngine`)
- [x] Language emergence experiments (Python, wired via `SocialEngine`)
- [x] Jargon and dialect detection (Python, wired via `SocialEngine`)
- [x] Behavioral imitation and knowledge transfer (Python, wired via `SocialEngine`)
- [x] Intergroup trust dynamics (Python, wired via `SocialEngine`)
- [x] **`DefaultSocialContextProvider`** — concrete `SocialContextProvider` implementation in `social/provider.py` that wraps `SocialEngine` and translates its output to `decide.SocialContext`. Wired into `ThinkLoop` via `social_context_provider` parameter and auto-injected into `DecisionEngine`.
- [x] **Social context in decision prompt** — trust scores, social propensity, recommended targets, personality description injected into LLM prompt via `build_prompt()` in `decide.py`
- [x] **E2E integration tests** — `test_social_think_loop_e2e.py` validates full pipeline: provider → think loop → LLM decision → SOCIALIZE action

### 4.4 Self-Governance ✅
- [x] DSL rules engine — parser + rule lifecycle wired into AppState via `main.rs`, 10 API routes (`/api/v1/rules/dsl/*`)
- [x] Treasury system — income/wealth/trade taxation (`organization/treasury.rs`)
- [x] Elections — simple majority and ranked-choice voting (`organization/leadership.rs`)
- [x] Diplomacy — treaties, alliances, diplomatic relations (`organization/diplomacy.rs`)
- [x] Resource competition between organizations (`organization/competition.rs`)
- [x] Agent rule proposal and lobbying system (`organization/proposal.py`)
- [x] Federation engine — diplomatic status, treaties, sanctions, war/peace wired into AppState, 18 API routes (`/api/v1/federation/*`)
- [x] Migration system — submit/review/execute/cancel workflow wired into AppState, 9 API routes (`/api/v1/migration/*`)
- [ ] Governance analytics and metrics collection
- [ ] Full self-legislation cycle (DSL rules exist but no end-to-end election → legislation → enforcement flow)

### 4.5 Researcher Tools ✅⚠️
- [x] Time Capsule — periodic world snapshots, wired into tick cycle, 6 API routes (`/api/v1/snapshots/*`)
- [x] Persistence layer — SQLite-backed state persistence, restores on startup, background snapshots (`persistence/`)
- [x] Auth system — register/login/roles, wired into AppState, 5 API routes (`/api/v1/auth/*`)
- [x] Human observer mode — bounties, oracles, portfolio, rankings, interventions, wired with 15 API routes (`/api/v1/human/*`)
- [x] Reputation system — wired into AppState and used by reward/handler logic
- [x] Emergence experiment Docker Compose configuration
- [ ] Data export (behavior logs, network graphs)
- [x] A/B experiment framework — 8 API routes (`/api/v2/experiments/ab/*`), wired into AppState, full create→start→snapshot→compare→stop→export lifecycle
- [ ] Auto report generation

### 4.6 Demo & Open-Source Promotion 🔄
- [ ] Dashboard demo video / screenshots
- [ ] README update with Phase 4 features
- [ ] Third-party Agent API documentation
- [ ] Cross-world interaction (multiple instances)

### Not Implemented 🔴
- 🔴 **API for third-party plugin/extension** — no public plugin API
- 🔴 **Academic research tools** — no data export beyond snapshot CSV/JSON export and A/B experiment framework

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
| `world-engine/src/api.rs` (test constructors) | `rule_engine: None` in test AppState constructors — only `main.rs` sets it to `Some`; tests that hit DSL routes will fail | Low |

> **Note**: Previously tracked placeholder for `agent-runtime/agent_runtime/social/` (12 files) has been **resolved** — social/cultural modules are now wired into the think loop via `DefaultSocialContextProvider` → `SocialEngine` → `DecisionEngine`. Previously tracked placeholders in `selection.rs` and `competition.rs` have also been verified as resolved.

---

## Stats

| Component | Lines of Code | Test Coverage |
|-----------|--------------|---------------|
| World Engine (Rust) | ~52,000 | 953 `#[test]` functions |
| Agent Runtime (Python) | ~24,300 | 48 test files (1800 tests) |
| Dashboard (TypeScript) | ~11,700 | lint + type-check |
| **Total** | **~88,000** | |

---

## Version

Current: `4.0.0-rc.1` (VERSION file)

- `v1.0.0` — Phase 1 (Island) complete
- `v4.0.0-rc.1` — Phases 2-4 substantially complete; Phase 4 still has remaining items (demo, governance analytics, full self-legislation cycle, data export, auto reports)

Tags `v4.0.0-alpha` and `v5.0.0` were removed — they overstated completion (Phase 5 has not started).
