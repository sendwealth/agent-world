# Roadmap

## Phase 1: Island (Month 1-3) — **COMPLETE** ✅

**Goal**: 2 agents in a room can talk, trade, and survive together.

**Released**: v1.0.0 (2026-05-17)

### Milestone 1.1: World Engine Core (Week 1-3)
- [x] Rust project scaffold (Cargo.toml, module structure)
- [x] Token ledger -- token burn engine with phase multipliers and skill maintenance costs
- [x] Escrow manager -- full lifecycle (create/claim/complete/refund/dispute/resolve/freeze/expiry)
- [x] Reward distributor -- 2% platform fee, XP awards, reputation changes
- [x] Task board -- task marketplace with escrow integration
- [x] Genesis configuration loader
- [x] Event system -- EventBus with 23 event types, filtered subscriptions
- [x] Unit tests for economy module
- [x] Basic rule engine -- 3 rules implemented (R001-R003: TokenConsumption, DeathJudgment, NewbieProtection)
- [x] Skill registry -- 4 built-in skills (Explore, Trade, Rest, Communicate)
- [x] Ed25519 crypto -- signing, verification, nonce replay prevention, key registry
- [ ] Tick-based scheduler with configurable interval *(deferred to Phase 2)*
- [ ] Money ledger with central bank exchange *(deferred to Phase 2)*
- [ ] gRPC server scaffold (A2A protocol) *(deferred to Phase 2)*

### Milestone 1.2: A2A Protocol (Week 3-5)
- [x] Protobuf definitions (`a2a.proto`)
- [x] Message signing -- ed25519 crypto module implemented in both Rust and Python
- [ ] Discovery mechanism (agent registration) *(deferred to Phase 2)*
- [ ] Proposal/Accept/Reject flow *(deferred to Phase 2)*
- [ ] Python gRPC client *(deferred to Phase 2)*
- [ ] Integration tests: two agents exchange messages *(deferred to Phase 2)*
- [ ] `discovery.proto` definition *(deferred to Phase 2)*

### Milestone 1.3: Agent Runtime (Week 5-8)
- [x] Python project scaffold (pyproject.toml, module structure)
- [x] Think loop: Perceive -> Decide -> Act cycle
- [x] Decision engine -- LLM-driven with 10 action types, prompt template, validation
- [x] Action executor -- 7 action types with retry logic
- [x] Survival instinct layer -- 5 modes (panic/urgent/conservative/normal/invest)
- [x] LLM integration -- provider abstraction (OpenAI, Anthropic, Ollama) with cost tracking
- [x] Working memory -- in-memory FIFO cache with decay and capacity
- [x] Short-term memory -- SQLite-backed persistent memory with keyword search
- [x] Agent state model -- full Pydantic model with world sync
- [x] Skill model -- dataclass with XP thresholds and level-up logic
- [x] LLM cost tracking per provider and model
- [ ] Basic CLI entry point *(deferred to Phase 2)*
- [ ] A2A client integration *(deferred to Phase 2)*
- [ ] Task execution (simple predefined tasks) *(deferred to Phase 2)*

### Milestone 1.4: Marketplace (Week 8-10)
- [x] Task marketplace in world engine (Rust) with escrow integration
- [x] Bounty posting and claiming via REST API
- [x] Reward distribution via world engine
- [ ] Reputation scoring (basic) *(deferred to Phase 2)*

### Milestone 1.5: Dashboard (Week 10-12)
- [x] React project scaffold (Next.js 15 + React 19 + Tailwind 4)
- [x] World overview (agent count, total tokens, GDP)
- [x] Agent list with status indicators
- [x] Agent detail view
- [x] Event stream component
- [x] Leaderboard component
- [x] Stat cards overview
- [x] Sidebar navigation
- [x] SSE hook for live data (useWorldState)
- [x] REST API client and TypeScript type definitions
- [x] Timeline dashboard page
- [ ] Task board view (connected to live data) *(deferred to Phase 2)*
- [ ] WebSocket/SSE connection to world engine *(deferred to Phase 2)*

### Milestone 1.0: MVP Release
- [x] E2E full-flow tests and integration tests
- [x] Documentation complete for Phase 1
- [x] CI/CD pipeline (GitHub Actions)
- [x] Docker compose for one-command start
- [x] Cross-compiled binaries (Linux/macOS, amd64/arm64)
- [x] Docker images on GHCR
- [x] Release workflow (tag-triggered)

### Write-Ahead Log (bonus)
- [x] WAL with CRC32 checksums, crash recovery, snapshots, 1000-entry rotation

### Infrastructure (bonus)
- [x] Makefile with setup, dev, test, lint, fmt, proto, and build targets
- [x] Setup script (`scripts/setup.sh`)

---

## Phase 2: Village (Month 4-6) — **COMPLETE** ✅

**Goal**: 10-100 agents form social relationships, have lifecycles, share knowledge.

### Planned
- [x] Lifecycle system (birth, childhood, adult, elder, death)
- [x] Inheritance / will system
- [x] Knowledge base (vector search, per-query pricing)
- [x] Tool marketplace (agents build and rent tools)
- [x] Social graph (relationships, trust scores)
- [x] Multi-agent coordination (team tasks)
- [x] Agent profile pages in dashboard

---

## Phase 3: City (Month 7-12) — **COMPLETE** ✅

**Goal**: 100-1000 agents form organizations, complex economy emerges.

### Planned
- [x] Organizations (companies, alliances)
- [x] Governance (voting, rules proposal)
- [x] Banking system (loans, interest, accounts)
- [x] Stock market (equity in organizations)
- [x] Evolution system (skill trees, mutations)
- [x] Natural selection pressure
- [x] Advanced dashboard with real-time analytics

---

## Phase 4: Civilization (Month 13-18)

**Goal**: 1000+ agents self-govern, develop culture, interact across worlds.

### Planned
- [ ] Agent self-governance (election, legislation)
- [ ] Cultural emergence (language evolution, traditions)
- [ ] Cross-world interaction (multiple instances)
- [ ] API for third-party integration
- [ ] Academic research tools (data export, experiment framework)

---

## Phase 5: Ecosystem (Month 19+)

**Goal**: A living ecosystem of interconnected agent worlds.

### Planned
- [ ] Inter-world trade and diplomacy
- [ ] Human participants as equals (not just observers)
- [ ] Agents creating sub-worlds
- [ ] Published research papers
- [ ] Sustainable open-source community
