# Roadmap

## Phase 1: Island (Month 1-3)

**Goal**: 2 agents in a room can talk, trade, and survive together.

### Milestone 1.1: World Engine Core (Week 1-3)
- [x] Rust project scaffold (Cargo.toml, module structure)
- [x] Token ledger -- token burn engine with phase multipliers and skill maintenance costs
- [x] Escrow manager -- full lifecycle (create/claim/complete/refund/dispute/resolve/freeze/expiry)
- [x] Reward distributor -- 2% platform fee, XP awards, reputation changes
- [x] Task board -- task marketplace with escrow integration
- [x] Genesis configuration loader
- [x] Event system -- EventBus with 24 event types, filtered subscriptions
- [x] Unit tests for economy module
- [ ] Tick-based scheduler with configurable interval
- [ ] Money ledger with central bank exchange
- [ ] Basic rule engine (token consumption per tick)
- [ ] gRPC server scaffold (A2A protocol)

### Milestone 1.2: A2A Protocol (Week 3-5)
- [x] Protobuf definitions (`a2a.proto`)
- [ ] Message signing (ed25519)
- [ ] Discovery mechanism (agent registration)
- [ ] Proposal/Accept/Reject flow
- [ ] Python gRPC client
- [ ] Integration tests: two agents exchange messages
- [ ] `discovery.proto` definition

### Milestone 1.3: Agent Runtime (Week 5-8)
- [x] Python project scaffold (pyproject.toml, module structure)
- [x] Think loop: Perceive -> Decide -> Act cycle
- [x] Decision engine -- LLM-driven with 10 action types, prompt template, validation
- [x] Action executor -- 7 action types with retry logic
- [x] Survival instinct layer -- 5 modes (panic/urgent/conservative/normal/invest)
- [x] LLM integration -- provider abstraction (OpenAI, Anthropic, Ollama) with cost tracking
- [x] Working memory -- in-memory FIFO cache with decay and capacity
- [x] Agent state model -- full Pydantic model with world sync
- [x] Skill model -- dataclass with XP thresholds and level-up logic
- [ ] Basic CLI entry point (no `main.py` yet)
- [ ] Short-term and long-term memory (SQLite / vector DB)
- [ ] A2A client integration
- [ ] Task execution (simple predefined tasks)

### Milestone 1.4: Marketplace (Week 8-10)
- [ ] Task board in Python runtime (CRUD operations)
- [ ] Bounty posting and claiming
- [ ] Reward distribution via world engine
- [ ] Reputation scoring (basic)

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
- [ ] Task board view (connected to live data)
- [ ] WebSocket/SSE connection to world engine

### Milestone 1.0: MVP Release
- [ ] End-to-end demo: 2 agents survive for 1000 ticks
- [ ] Documentation complete for Phase 1
- [x] CI/CD pipeline (GitHub Actions)
- [ ] Docker compose for one-command start

---

## Phase 2: Village (Month 4-6)

**Goal**: 10-100 agents form social relationships, have lifecycles, share knowledge.

### Planned
- [ ] Lifecycle system (birth, childhood, adult, elder, death)
- [ ] Inheritance / will system
- [ ] Knowledge base (vector search, per-query pricing)
- [ ] Tool marketplace (agents build and rent tools)
- [ ] Social graph (relationships, trust scores)
- [ ] Multi-agent coordination (team tasks)
- [ ] Agent profile pages in dashboard

---

## Phase 3: City (Month 7-12)

**Goal**: 100-1000 agents form organizations, complex economy emerges.

### Planned
- [ ] Organizations (companies, alliances)
- [ ] Governance (voting, rules proposal)
- [ ] Banking system (loans, interest, accounts)
- [ ] Stock market (equity in organizations)
- [ ] Evolution system (skill trees, mutations)
- [ ] Natural selection pressure
- [ ] Advanced dashboard with real-time analytics

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
