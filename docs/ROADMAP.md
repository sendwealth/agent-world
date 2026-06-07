1|1|# Roadmap
2|2|
3|3|> **Overall completion: ~95%** (as of 2026-06-07, after full ROADMAP sync)
4|4|>
5|5|> This document reflects the **actual** implementation and wiring state.
6|6|> Items marked ⚠️ are partially implemented — see details below.
7|7|> Items marked 🔴 are not yet implemented.
8|8|
9|9|---
10|10|
11|11|## Phase 1: Island (Month 1-3) — **COMPLETE** ✅ (100%)
12|12|
13|13|**Goal**: 2 agents in a room can talk, trade, and survive together.
14|14|
15|15|**Released**: v1.0.0 (2026-05-20)
16|16|
17|17|### Milestone 1.1: World Engine Core
18|18|- [x] Rust project scaffold (Cargo.toml, module structure)
19|19|- [x] Token burn engine with phase multipliers and skill maintenance costs
20|20|- [x] Escrow manager — full lifecycle (create/claim/complete/refund/dispute/resolve/freeze/expiry)
21|21|- [x] Reward distributor — 2% platform fee, XP awards, reputation changes
22|22|- [x] Task board — task marketplace with escrow integration
23|23|- [x] Genesis configuration loader
24|24|- [x] Event system — EventBus with 139 event types (26 categories), filtered subscriptions, SSE
25|25|- [x] Basic rule engine — 3 rules (TokenConsumption, DeathJudgment, NewbieProtection)
26|26|- [x] Skill registry — 4 built-in skills (Explore, Trade, Rest, Communicate)
27|27|- [x] Ed25519 crypto — signing, verification, nonce replay prevention, key registry
28|28|- [x] Tick-based scheduler with configurable interval
29|29|- [x] Money ledger with central bank exchange
30|30|- [x] gRPC server scaffold (A2A protocol)
31|31|
32|32|### Milestone 1.2: A2A Protocol
33|33|- [x] Protobuf definitions (`a2a.proto`)
34|34|- [x] Message signing — ed25519 in both Rust and Python
35|35|- [x] Discovery mechanism (agent registration)
36|36|- [x] Proposal/Accept/Reject flow
37|37|- [x] Python gRPC client
38|38|- [x] Integration tests: two agents exchange messages
39|39|
40|40|### Milestone 1.3: Agent Runtime
41|41|- [x] Python project scaffold (pyproject.toml, module structure)
42|42|- [x] Think loop: Perceive → Decide → Act cycle
43|43|- [x] Decision engine — LLM-driven with 10 action types, JSON parsing, validation
44|44|- [x] Action executor — 7 action types with retry logic
45|45|- [x] Survival instinct — 5 modes, 11 emergency actions
46|46|- [x] LLM integration — OpenAI, Anthropic, Ollama, 智谱 GLM-5
47|47|- [x] Working memory — in-memory FIFO with decay
48|48|- [x] Short-term memory — SQLite-backed with keyword search
49|49|- [x] Agent state model — full Pydantic model
50|50|- [x] Skill model with XP thresholds and level-up
51|51|- [x] LLM cost tracking per provider/model
52|52|- [x] CLI entry point (`__main__.py` with `spawn` subcommand)
53|53|- [x] A2A client integration
54|54|- [x] Task execution via `world_client.py`
55|55|
56|56|### Milestone 1.4: Marketplace
57|57|- [x] Task marketplace with escrow integration
58|58|- [x] Bounty posting and claiming via REST API
59|59|- [x] Reward distribution via world engine
60|60|- [x] Reputation scoring (`economy/reputation.rs`, 648 lines)
61|61|
62|62|### Milestone 1.5: Dashboard
63|63|- [x] Next.js 15 + React 19 + Tailwind 4 scaffold
64|64|- [x] World overview, agent list, agent detail
65|65|- [x] Event stream, leaderboard, stat cards, sidebar
66|66|- [x] SSE hook for live data (`useWorldState`)
67|67|- [x] REST API client + TypeScript type definitions
68|68|- [x] Timeline, task board pages
69|69|- [x] WebSocket/SSE connection
70|70|
71|71|### Milestone 1.0: MVP Release
72|72|- [x] E2E full-flow + integration tests (8/8 E2E, 1038 Python unit tests)
73|73|- [x] Documentation for Phase 1
74|74|- [x] CI/CD (GitHub Actions: Rust clippy+test, Python ruff+pytest, Dashboard lint+build)
75|75|- [x] Docker Compose one-command start (10-agent config)
76|76|- [x] Cross-compiled binaries (Linux/macOS, amd64/arm64)
77|77|- [x] Docker images on GHCR
78|78|- [x] Release workflow (tag-triggered)
79|79|
80|80|### Bonus
81|81|- [x] WAL with CRC32 checksums, crash recovery, snapshots
82|82|- [x] Makefile with setup/dev/test/lint/fmt/proto/build targets
83|83|
84|84|---
85|85|
86|86|## Phase 2: Village (Month 4-6) — **~92%** ✅
87|87|
88|88|**Goal**: 10-100 agents form social relationships, have lifecycles, share knowledge.
89|89|
90|90|### Implemented ✅
91|91|- [x] Lifecycle system — birth, childhood, adult, elder, death (`lifecycle.rs`, 39K lines, full state machine)
92|92|- [x] Inheritance/will system — beneficiaries, token distribution, skill transfer (`economy/inheritance.rs`, 543 lines)
93|93|- [x] Knowledge base — vector memory with embedding support in Python (`memory/vector_memory.py` 634 lines, `memory/embedding.py` 274 lines)
94|94|- [x] Agent profile pages in dashboard (agent detail, evolution traces)
95|95|- [x] Knowledge marketplace — `economy/marketplace.rs` (1485 lines) **wired into AppState** with 11 API routes, dashboard page (`marketplace/page.tsx`)
96|96|- [x] Social context protocol — `decide.py` defines `SocialContextProvider` and `SocialContext` dataclass
97|97|- [x] **Social graph — wired into think loop** — `DefaultSocialContextProvider` (`social/provider.py`) implements the `SocialContextProvider` protocol, wraps `SocialEngine` which aggregates all 12 social modules (trust, cultural diffusion, imitation, language, etc.). Injected into `ThinkLoop` via `social_context_provider` parameter, which propagates to `DecisionEngine` for prompt injection. E2E tests confirm social context (trust scores, recommended targets, personality description) flows through the full Perceive → Decide → Act pipeline.
98|98|
99|99|### Implemented ✅ (previously marked 🔴)
100|100|- [x] **Tool marketplace** — `economy/tool_marketplace.rs` + `api_tool_marketplace.rs` (567 lines): list/delist/purchase/rent tools, per-tick rental pricing, full lifecycle API routes
101|101|- [x] **Multi-agent coordination** — `api_coordination_tasks.rs` (517 lines): create/join/submit/complete/cancel multi-agent tasks with contributor tracking
102|102|
103|103|---
104|104|
105|105|## Phase 3: City (Month 7-12) — **~95%** ✅
106|106|
107|107|**Goal**: 100-1000 agents form organizations, complex economy emerges.
108|108|
109|109|### Implemented ✅
110|110|- [x] Organizations — Company/Guild/Alliance/University (`organization/org.rs`, 26K lines)
111|111|- [x] Membership management — join/leave/roles (`organization/members.rs`)
112|112|- [x] Charter system — governance model, profit sharing (`organization/charter.rs`)
113|113|- [x] Governance — 3 decision modes (Vote/Dictator/Council), weighted voting, 5 proposal types (`organization/governance.rs`, 73K lines)
114|114|- [x] Banking — savings/checking accounts, loans, collateral, central bank (`economy/banking.rs`, 49K lines)
115|115|- [x] Stock market — IPOs, order book matching, dividends, delisting (`economy/stock_market.rs`, 45K lines)
116|116|- [x] Evolution — branching skill tree (10 skills, levels 1-10), mutation engine, evolution subsystem (`evolution/`, 3 files)
117|117|- [x] Natural selection — fitness scoring uses real tracked data: `tasks_completed/tasks_attempted`, token efficiency, survival duration, social proxy, skill diversity (`evolution/selection.rs`)
118|118|- [x] Resource competition — uses real member skill data from world state; falls back to 1.0 only for empty orgs (`organization/competition.rs`)
119|119|- [x] Advanced dashboard — organizations (force graph), stocks (price charts), evolution (skill breakdown), economy (GDP/Gini)
120|120|- [x] 100-agent stress tests — 5 tests validating concurrent operations
121|121|- [x] Criterion benchmarks for hot paths
122|122|- [x] Full REST API (50+ endpoints across all subsystems)
123|123|
124|124|---
125|125|
126|126|## Phase 4: Civilization (Month 13-18) — **~95%** ✅
127|127|
128|128|**Goal**: 1000+ agents self-govern, develop culture, interact across worlds.
129|129|
130|130|### 4.1 LLM Integration ✅
131|131|- [x] Multi-provider support (OpenAI, Anthropic, Ollama, 智谱 GLM-5)
132|132|- [x] Async decision engine for concurrent LLM calls
133|133|- [x] LLM cost tracking and queue management
134|134|- [x] Decision logging and prompt templates
135|135|
136|136|### 4.2 Tracing & Observability ✅
137|137|- [x] Tick-level tracing collection (perception → decision → action → reflection) — `TraceStore` wired into AppState, 4 API routes
138|138|- [x] Interaction graph construction (social network)
139|139|- [x] Emergence detection metrics
140|140|- [x] SQLite tracing store with query interface
141|141|- [x] Dashboard traces page (per-agent, per-tick drill-down)
142|142|
143|143|### 4.3 Cultural Emergence ✅
144|144|- [x] Big Five personality vectors (`models/personality.py`)
145|145|- [x] Organization culture modeling (`engine/culture.rs`) — wired, used by competition module
146|146|- [x] Cultural diffusion — regional and organizational value convergence (Python, wired via `SocialEngine` → `DefaultSocialContextProvider` → think loop)
147|147|- [x] Cultural conflict detection and resolution (Python, wired via `SocialEngine`)
148|148|- [x] Regional culture cluster detection (Python, wired via `SocialEngine`)
149|149|- [x] Language emergence experiments (Python, wired via `SocialEngine`)
150|150|- [x] Jargon and dialect detection (Python, wired via `SocialEngine`)
151|151|- [x] Behavioral imitation and knowledge transfer (Python, wired via `SocialEngine`)
152|152|- [x] Intergroup trust dynamics (Python, wired via `SocialEngine`)
153|153|- [x] **`DefaultSocialContextProvider`** — concrete `SocialContextProvider` implementation in `social/provider.py` that wraps `SocialEngine` and translates its output to `decide.SocialContext`. Wired into `ThinkLoop` via `social_context_provider` parameter and auto-injected into `DecisionEngine`.
154|154|- [x] **Social context in decision prompt** — trust scores, social propensity, recommended targets, personality description injected into LLM prompt via `build_prompt()` in `decide.py`
155|155|- [x] **E2E integration tests** — `test_social_think_loop_e2e.py` validates full pipeline: provider → think loop → LLM decision → SOCIALIZE action
156|156|
157|157|### 4.4 Self-Governance ✅
158|158|- [x] DSL rules engine — parser + rule lifecycle wired into AppState via `main.rs`, 10 API routes (`/api/v1/rules/dsl/*`)
159|159|- [x] Treasury system — income/wealth/trade taxation (`organization/treasury.rs`)
160|160|- [x] Elections — simple majority and ranked-choice voting (`organization/leadership.rs`)
161|161|- [x] Diplomacy — treaties, alliances, diplomatic relations (`organization/diplomacy.rs`)
162|162|- [x] Resource competition between organizations (`organization/competition.rs`)
163|163|- [x] Agent rule proposal and lobbying system (`organization/proposal.py`)
164|164|- [x] Federation engine — diplomatic status, treaties, sanctions, war/peace wired into AppState, 18 API routes (`/api/v1/federation/*`)
165|165|- [x] Migration system — submit/review/execute/cancel workflow wired into AppState, 9 API routes (`/api/v1/migration/*`)
166|166|- [x] Governance analytics and metrics collection — `organization/governance_metrics.rs` wired into AppState, 4 API routes
167|167|- [x] Full self-legislation cycle — LegislationCycleEngine implemented, API routes wired (`/api/v1/governance/*`)
168|168|
169|169|### 4.5 Researcher Tools ✅
170|170|- [x] Time Capsule — periodic world snapshots, wired into tick cycle, 6 API routes (`/api/v1/snapshots/*`)
171|171|- [x] Persistence layer — SQLite-backed state persistence, restores on startup, background snapshots (`persistence/`)
172|172|- [x] Auth system — register/login/roles, wired into AppState, 5 API routes (`/api/v1/auth/*`)
173|173|- [x] Human observer mode — bounties, oracles, portfolio, rankings, interventions, wired with 15 API routes (`/api/v1/human/*`)
174|174|- [x] Reputation system — wired into AppState and used by reward/handler logic
175|175|- [x] Emergence experiment Docker Compose configuration
176|176|- [x] Data export (behavior logs, network graphs, economy, prices, organizations)
177|177|- [x] A/B experiment framework — 8 API routes (`/api/v2/experiments/ab/*`), wired into AppState, full create→start→snapshot→compare→stop→export lifecycle
178|178|- [x] Auto report generation — `GET /api/v2/export/report` (HTML/JSON/Markdown with trend analysis, emergent pattern detection, SVG sparklines) + Python `ExperimentReporter` (Markdown/JSON/HTML/PDF with embedded matplotlib charts)
179|179|
180|180|### 4.6 Demo & Open-Source Promotion 🔄
181|181|- [ ] Dashboard demo video / screenshots
182|182|- [x] README update with Phase 4 features
183|183|- [x] Third-party Agent API documentation — `docs-site/how-to/third-party-agent-api.md` + Python SDK
184|184|- [x] Cross-world interaction (multiple instances) — Federation + Migration wired into AppState, 27 API routes
185|185|
186|186|### Previously Listed as Not Implemented — Now Done ✅
187|187|- ✅ **API for third-party plugin/extension** — plugin system implemented and documented (`plugin-interface-spec.md`, `public-plugin-api.md`, `plugin-getting-started.md`); see Phase 4 Plugin section
188|188|- ✅ **Academic research tools** — SDK `analyze.py` with 25+ analysis functions, `research_formats.py`, `api_research.rs` routes, Jupyter notebook (`sdk/examples/research_analysis.ipynb`)
189|189|
190|190|---
191|191|
192|192|## Phase 5: Ecosystem (Month 19+) — **NOT STARTED** 🔴
193|193|
194|194|**Goal**: A living ecosystem of interconnected agent worlds.
195|195|
196|196|### Planned
197|197|- [ ] Inter-world trade and diplomacy
198|198|- [ ] Human participants as equals (not just observers)
199|199|- [ ] Agents creating sub-worlds
200|200|- [ ] Published research papers
201|201|- [ ] Sustainable open-source community
202|202|
203|203|---
204|204|
205|205|## Placeholder & Known Issue Tracker
206|206|
207|207|| File | Issue | Severity |
208|208||------|-------|----------|
209|209|| `world-engine/src/api.rs` (test constructors) | `rule_engine: None` in test AppState constructors — only `main.rs` sets it to `Some`; tests that hit DSL routes will fail | Low |
210|210|
211|211|> **Note**: Previously tracked placeholder for `agent-runtime/agent_runtime/social/` (12 files) has been **resolved** — social/cultural modules are now wired into the think loop via `DefaultSocialContextProvider` → `SocialEngine` → `DecisionEngine`. Previously tracked placeholders in `selection.rs` and `competition.rs` have also been verified as resolved.
212|212|
213|213|---
214|214|
215|215|## Stats
216|216|
217|217|| Component | Lines of Code | Test Coverage |
218|218||-----------|--------------|---------------|
219|219|| World Engine (Rust) | ~81,000 | 1,165 `#[test]` functions |
220|220|| Agent Runtime (Python) | ~39,000 | 69 test files |
221|221|| Dashboard (TypeScript) | ~21,000 | lint + type-check |
222|222|| **Total** | **~141,000** | |
223|223|
224|224|---
225|225|
226|226|## Version
227|227|
228|228|Current: `1.1.0` (VERSION file)
229|229|
230|230|- `v1.0.0` — Phase 1 (Island) complete
231|231|- `v1.1.0` — Phases 2-4 substantially complete; Phase 4 remaining: demo video/screenshots only
232|232|
233|233|Tags `v4.0.0-alpha` and `v5.0.0` were removed — they overstated completion (Phase 5 has not started).
234|234|