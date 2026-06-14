# Roadmap

> **Overall completion: ~95%** (as of 2026-06-07, after full ROADMAP sync)
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
- [x] Event system — EventBus with 139 event types (26 categories), filtered subscriptions, SSE
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

### Implemented ✅ (previously marked 🔴)
- [x] **Tool marketplace** — `economy/tool_marketplace.rs` + `api_tool_marketplace.rs` (567 lines): list/delist/purchase/rent tools, per-tick rental pricing, full lifecycle API routes
- [x] **Multi-agent coordination** — `api_coordination_tasks.rs` (517 lines): create/join/submit/complete/cancel multi-agent tasks with contributor tracking

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

## Phase 4: Civilization (Month 13-18) — **~95%** ✅

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
- [x] Governance analytics and metrics collection — `organization/governance_metrics.rs` wired into AppState, 4 API routes
- [x] Full self-legislation cycle — LegislationCycleEngine wired into main.rs + AppState, 13 API routes (`/api/v1/legislation/*`), end-to-end tests pass

### 4.5 Researcher Tools ✅
- [x] Time Capsule — periodic world snapshots, wired into tick cycle, 6 API routes (`/api/v1/snapshots/*`)
- [x] Persistence layer — SQLite-backed state persistence, restores on startup, background snapshots (`persistence/`)
- [x] Auth system — register/login/roles, wired into AppState, 5 API routes (`/api/v1/auth/*`)
- [x] Human observer mode — bounties, oracles, portfolio, rankings, interventions, wired with 15 API routes (`/api/v1/human/*`)
- [x] Reputation system — wired into AppState and used by reward/handler logic
- [x] Emergence experiment Docker Compose configuration
- [x] Data export (behavior logs, network graphs, economy, prices, organizations)
- [x] A/B experiment framework — 8 API routes (`/api/v2/experiments/ab/*`), wired into AppState, full create→start→snapshot→compare→stop→export lifecycle
- [x] Auto report generation — `GET /api/v2/export/report` (HTML/JSON/Markdown with trend analysis, emergent pattern detection, SVG sparklines) + Python `ExperimentReporter` (Markdown/JSON/HTML/PDF with embedded matplotlib charts)

### 4.6 Demo & Open-Source Promotion 🔄
- [x] Dashboard demo video / screenshots — Playwright automation in `scripts/screenshots/`, `make screenshots` captures all pages; initial captures in `docs/screenshots/`
- [x] README update with Phase 4 features
- [x] Third-party Agent API documentation — `docs-site/how-to/third-party-agent-api.md` + Python SDK
- [x] Cross-world interaction (multiple instances) — Federation + Migration wired into AppState, 27 API routes

### Previously Listed as Not Implemented — Now Done ✅
- ✅ **API for third-party plugin/extension** — plugin system implemented and documented (`plugin-interface-spec.md`, `public-plugin-api.md`, `plugin-getting-started.md`); see Phase 4 Plugin section
- ✅ **Academic research tools** — SDK `analyze.py` with 25+ analysis functions, `research_formats.py`, `api_research.rs` routes, Jupyter notebook (`sdk/examples/research_analysis.ipynb`)

---

## Phase 5: Ecosystem (Month 19+) — **PLANNED** 🟡

**Goal**: Turn Agent World from a feature-complete single-instance simulator into a *used, cited, and interconnected* ecosystem — closing the gap between engineering scope and real-world adoption.

> **The strategic problem Phase 5 must solve.** As of v1.1.0 the platform already exceeds every comparable open project in *breadth* of simulation mechanics (economy, governance, culture, evolution, federation, emotion, plugins — ~141K LOC). Yet it has essentially **zero adoption** (~1 star, 0 forks) and **zero published research**. Adding more simulation depth will not change that. Phase 5 is therefore an *ecosystem & validation* phase, not a feature phase: its north-star metric is "number of external people who run, extend, or cite Agent World," not LOC shipped.

---

### 5.0 Current Capability Boundary

Before defining new work, here is the honest line between "done" and "ecosystem requires":

| Capability | Phase 1–4 state | What a true *ecosystem* still needs |
|---|---|---|
| Federation | Migration + diplomacy status (Peace/Trade/Alliance/War) + heartbeat — **27 routes** | No real cross-world **trade flow**; no public **world discovery registry**; federation is local-only (no hosted hub) |
| Plugin system | Local registry, hooks/subsystems/permissions — fully wired | No **distribution/marketplace**; plugins are code-copy, not install-from-registry |
| Human role | Observer mode — bounties, oracles, portfolio, interventions | Humans are **outside** the world; cannot *be* an agent peer in the economy |
| Research tools | A/B experiments, auto-reports, data export, SDK | No **shared benchmark suite**; no replication of prior work (Park et al.); no dataset/Zenodo publishing flow |
| Sub-worlds | — | Agents cannot spawn or govern child worlds |
| Adoption | Open-source, Docker one-command, docs | No published paper, no contributor base, no citation, no "show HN / demo video" delivered |

**One sentence:** the engine is a finished *simulator*; Phase 5 makes it a *platform* and a *research object*.

---

### 5.1 Comparable Projects & Landscape

Phase 5 directions are chosen against this landscape, not in a vacuum:

| Project | Scale | Has Agent World beat it? | What Agent World lacks vs. them |
|---|---|---|---|
| **Generative Agents / Smallville** (Park et al., 2023) | 25 agents, no economy | ✅ vastly more mechanics | ❌ **citation & academic legitimacy** — it *defined* the field |
| **Project Sid** (Altera, 2024) | 1000+ agents, Minecraft | ✅ decoupled from a game; open | ❌ a **flagship "look what emerged" result** + publicity |
| **AI Town** (a16z/Convex, 2023) | simple, JS | ✅ far deeper systems | ❌ **onboarding friction** — AI Town runs in one click in a browser |
| **AgentScope / AutoGen / CAMEL** | frameworks, not sims | different category | — these are *agent frameworks*; AW is a *world*. No direct overlap, but they define the audience. |

**Takeaway:** the moat is *depth + observability*; the gap is *legitimacy, reach, and a killer demo result*. Every Phase 5 direction is scored on how it closes that gap.

---

### 5.2 Candidate Directions (Comparative Analysis)

Five directions were considered. The matrix scores each on five dimensions (1–5, higher = better) against the Phase 5 north star (*adoption & validation leverage per unit effort*).

| # | Direction | Builds on existing infra | Adoption leverage | Research leverage | Effort (5=low) | Risk (5=low) | **Total** |
|---|---|:-:|:-:|:-:|:-:|:-:|:-:|
| **A** | **Research Platform & Emergence Benchmark** — replication suite (Park et al.), standardized emergence metrics, dataset publishing | 5 | 4 | 5 | 4 | 4 | **22** |
| **B** | **Human-as-Agent / Participatory Mode** — humans play as economic peers, mixed human-AI societies, game-like UX | 3 | 5 | 3 | 2 | 3 | **16** |
| **C** | **Inter-World Economy & Federation Hub** — real cross-world trade routes, hosted world-discovery registry, agent-spawned sub-worlds | 4 | 3 | 3 | 2 | 3 | **15** |
| **D** | **Plugin Marketplace & Community Worlds** — distributable plugin/skill/scenario registry (Minecraft-modding model) | 4 | 4 | 2 | 2 | 2 | **14** |
| **E** | **Emergence Deep-Dive & Auto-Documentary** — causal-emergence metrics, phase-transition detection, auto-generated "world history" films | 3 | 4 | 4 | 3 | 4 | **18** |

#### Direction A — Research Platform & Emergence Benchmark *(recommended first)*
Make Agent World the canonical reproducible platform for multi-agent society research. Ship a **Park et al. replication** showing AW reproduces known emergence (information diffusion, relationship formation), define a shared **"Emergence Benchmark"** (the MMLU for agent societies), and a one-command path from experiment → published dataset (Zenodo/Dataverse).

#### Direction B — Human-as-Agent / Participatory Mode
Promote humans from observer to **peer**: a human can incarnate as an agent, hold tokens, trade, vote, and survive alongside AI agents. Turns a spectacle into a *participatory simulation* and opens the "game" audience. Highest engagement ceiling, highest scope risk (real-time interaction model vs. tick loop).

#### Direction C — Inter-World Economy & Federation Hub
Extend federation from migration+diplomacy to **real trade** (cross-world markets, shared bounties, currency exchange), add a **hosted world-discovery registry** so strangers can find and federate worlds, and let high-rank agents **spawn sub-worlds**. Natural technical continuation — but premature without a community to federate.

#### Direction D — Plugin Marketplace & Community Worlds
Turn the local plugin registry into a **distributable marketplace** (skills, world rules, LLM adapters, scenario packs). The Roblox/Minecraft-modding play. Strong long-term network effects, but a cold-start problem with today's 0 contributors.

#### Direction E — Emergence Deep-Dive & Auto-Documentary
Invest in the *analysis* layer: causal-emergence detection, society "phase transitions," and an auto-generated **"world history" documentary** (timeline → narrated video). The best "shareable artifact" per dollar; differentiates AW as science, not just a sim.

---

### 5.3 Recommended Sequence & Milestones

Sequencing follows the rule: **maximize legitimacy and shareable artifacts early; build network-effect features only once a community exists to use them.**

```
A (Research/Benchmark)  →  E (Documentary)  →  B (Human-as-Agent)  →  C (Inter-world)  →  D (Marketplace)
  credibility first          shareable demo        audience growth        network effects       flywheel
```

| Milestone | Direction | Deliverable | Target metric (definition of done) |
|---|:-:|---|---|
| **5.1 Replication & Benchmark** | A | Park et al. replication experiment; `emergence-benchmark` suite (≥6 metrics); benchmark README + leaderboard | Reproduction report reproduces ≥3 published findings; benchmark runnable via `make benchmark` |
| **5.2 First Paper / Preprint** | A | arXiv preprint: "Agent World: an open platform for multi-agent society emergence" | Submitted to arXiv; ≥1 external researcher runs an experiment |
| **5.3 Dataset Publishing Flow** | A | `aw publish` → Zenodo/Dataverse DOI; experiment→dataset provenance | One-click DOI from any A/B experiment |
| **5.4 Auto-Documentary** | E | Timeline → narrated "world history" video generator; sample film in README | A 3-min auto film linked from the README |
| **5.5 Human-as-Agent MVP** | B | Humans incarnate as agents; mixed human-AI economy in a public demo world | ≥1 public "play a world" session with external participants |
| **5.6 Federation Hub (hosted)** | C | Public world-discovery registry; real cross-world trade routes; ≥2 hosted demo worlds | ≥3 external self-hosted worlds federated |
| **5.7 Agent-Spawned Sub-Worlds** | C | High-rank agents create and govern child worlds | E2E: an agent founds a world others can migrate to |
| **5.8 Plugin Marketplace** | D | Distributable plugin registry; ≥5 community-contributed plugins | ≥1 third-party plugin not authored by core team |

**Gating rule:** Milestones 5.5–5.8 are *deferred* and only start once 5.1–5.2 land a preprint and a first external user. Building network-effect features (C, D) before a community exists would repeat the Phase 1–4 pattern of shipping depth nobody uses.

---

### 5.4 Decision Criteria & Risks

**Kill / pivot signals** (re-evaluate at each milestone):
- If **5.1 replication fails to reproduce any prior finding** → pause; the engine needs a correctness audit before more research claims.
- If after **5.2 (preprint)** there are still **0 external users** → pivot from C/D (network) to B (participatory/game), because the adoption problem is UX, not capability.
- If LLM cost makes 1000-agent runs prohibitive → scope milestones to ≤200 agents and lead with E (documentary) which is cheap to demo.

| Risk | Likelihood | Impact | Mitigation |
|---|:-:|:-:|---|
| Replication reveals engine bugs | Med | High | 5.1 *is* the audit; budget a fix cycle |
| No academic adoption despite preprint | High | High | Partner with ≥1 lab before writing; co-author |
| Inter-world features built, no worlds to federate | High | Med | Hard-gate C/D behind community milestones |
| Scope creep re-bloats the codebase | Med | Med | Phase 5 north star = users/citations, NOT LOC |
| Demo video never ships (carried over from 4.6) | Med | Med | 5.4 auto-documentary subsumes & forces it |

---

> **Status note:** Phase 5 is now **planned with milestones** (was: NOT STARTED). No Phase 5 code has been written; milestones 5.1–5.8 are the backlog to promote to `todo` in priority order once Phase 4.6 (demo video) closes. The first executable step is **5.1 — Replication & Benchmark**, scoped to begin after the v1.1.0 demo deliverables land.
---

## Placeholder & Known Issue Tracker

| File | Issue | Severity |
|------|-------|----------|
| _Resolved_ — `world-engine/src/api.rs` test constructors now default `rule_engine` to `Some(RuleEngine::with_event_bus(..))`, consistent with `main.rs`; DSL routes are reachable from the default test state. No open known issues remain. | ✓ |

> **Note**: Previously tracked placeholder for `agent-runtime/agent_runtime/social/` (12 files) has been **resolved** — social/cultural modules are now wired into the think loop via `DefaultSocialContextProvider` → `SocialEngine` → `DecisionEngine`. Previously tracked placeholders in `selection.rs` and `competition.rs` have also been verified as resolved.

---

## Stats

| Component | Lines of Code | Test Coverage |
|-----------|--------------|---------------|
| World Engine (Rust) | ~81,000 | 1,165 `#[test]` functions |
| Agent Runtime (Python) | ~39,000 | 69 test files |
| Dashboard (TypeScript) | ~21,000 | lint + type-check |
| **Total** | **~141,000** | |

---

## Version

Current: `1.1.0` (VERSION file)

- `v1.0.0` — Phase 1 (Island) complete
- `v1.1.0` — Phases 2-4 substantially complete; Phase 4 screenshots delivered, demo video pending

Tags `v4.0.0-alpha` and `v5.0.0` were removed — they overstated completion (Phase 5 has not started).

