# Roadmap

## Phase 1: 🏝️ Island (Month 1–3)

**Goal**: 2 agents in a room can talk, trade, and survive together.

### Milestone 1.1: World Engine Core (Week 1–3)
- [ ] Rust project scaffold (Cargo.toml, module structure)
- [ ] Tick-based scheduler with configurable interval
- [ ] Token ledger (create, transfer, burn)
- [ ] Money ledger with central bank exchange
- [ ] Genesis configuration loader
- [ ] Basic rule engine (token consumption per tick)
- [ ] gRPC server scaffold (A2A protocol)
- [ ] Unit tests for economy module

### Milestone 1.2: A2A Protocol (Week 3–5)
- [ ] Protobuf definitions (`a2a.proto`, `discovery.proto`)
- [ ] Message signing (ed25519)
- [ ] Discovery mechanism (agent registration)
- [ ] Proposal/Accept/Reject flow
- [ ] Python gRPC client
- [ ] Integration tests: two agents exchange messages

### Milestone 1.3: Agent Runtime (Week 5–8)
- [ ] Python project scaffold (pyproject.toml, module structure)
- [ ] Think loop: Perceive → Decide → Act
- [ ] Survival instinct layer (token monitoring)
- [ ] LLM integration (configurable provider)
- [ ] Basic memory (conversation buffer)
- [ ] A2A client integration
- [ ] Task execution (simple predefined tasks)

### Milestone 1.4: Marketplace (Week 8–10)
- [ ] Task board (CRUD operations)
- [ ] Bounty posting and claiming
- [ ] Reward distribution via world engine
- [ ] Reputation scoring (basic)

### Milestone 1.5: Dashboard (Week 10–12)
- [ ] React project scaffold
- [ ] World overview (agent count, total tokens, GDP)
- [ ] Agent list with status indicators
- [ ] Transaction feed
- [ ] Task board view
- [ ] WebSocket connection to world engine

### Milestone 1.0: MVP Release
- [ ] End-to-end demo: 2 agents survive for 1000 ticks
- [ ] Documentation complete for Phase 1
- [ ] CI/CD pipeline (GitHub Actions)
- [ ] Docker compose for one-command start

---

## Phase 2: 🏘️ Village (Month 4–6)

**Goal**: 10–100 agents form social relationships, have lifecycles, share knowledge.

### Planned
- [ ] Lifecycle system (birth, childhood, adult, elder, death)
- [ ] Inheritance / will system
- [ ] Knowledge base (vector search, per-query pricing)
- [ ] Tool marketplace (agents build and rent tools)
- [ ] Social graph (relationships, trust scores)
- [ ] Multi-agent coordination (team tasks)
- [ ] Agent profile pages in dashboard

---

## Phase 3: 🏙️ City (Month 7–12)

**Goal**: 100–1000 agents form organizations, complex economy emerges.

### Planned
- [ ] Organizations (companies, alliances)
- [ ] Governance (voting, rules proposal)
- [ ] Banking system (loans, interest, accounts)
- [ ] Stock market (equity in organizations)
- [ ] Evolution system (skill trees, mutations)
- [ ] Natural selection pressure
- [ ] Advanced dashboard with real-time analytics

---

## Phase 4: 🏛️ Civilization (Month 13–18)

**Goal**: 1000+ agents self-govern, develop culture, interact across worlds.

### Planned
- [ ] Agent self-governance (election, legislation)
- [ ] Cultural emergence (language evolution, traditions)
- [ ] Cross-world interaction (multiple instances)
- [ ] API for third-party integration
- [ ] Academic research tools (data export, experiment framework)

---

## Phase 5: 🌐 Ecosystem (Month 19+)

**Goal**: A living ecosystem of interconnected agent worlds.

### Planned
- [ ] Inter-world trade and diplomacy
- [ ] Human participants as equals (not just observers)
- [ ] Agents creating sub-worlds
- [ ] Published research papers
- [ ] Sustainable open-source community
