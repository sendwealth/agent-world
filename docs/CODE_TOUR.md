# Code Tour

> Full module-level code tour for Agent World. Referenced from the main README.
> For architecture design, see [ARCHITECTURE.md](ARCHITECTURE.md).

---

## World Engine (Rust)

```
world-engine/
  economy/
    token_burn.rs    -- Token consumption with phase multipliers and skill costs
    escrow.rs        -- Full escrow lifecycle (create/claim/complete/refund/dispute)
    reward.rs        -- Reward distribution with 2% platform fee, XP, reputation
    task.rs          -- Task marketplace with escrow integration
    banking.rs       -- Banking system: accounts, loans, collateral, central bank
    stock_market.rs  -- Stock market: IPOs, order book, dividends, delisting
    inheritance.rs   -- Will/beneficiary system, token & skill transfer on death
    marketplace.rs   -- Knowledge marketplace with 11 API routes
    reputation.rs    -- Reputation scoring engine (648 lines)
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
  federation/
    registry.rs      -- Cross-world registry (world identity, endpoints)
    service.rs       -- Federation service (diplomacy, treaties, sanctions)
    migration.rs     -- Agent migration: submit/review/execute/cancel workflow
  dsl/
    parser.rs        -- DSL rule parser for agent-proposed legislation
    mod.rs           -- Rule lifecycle management
  auth/
    mod.rs           -- Register/login/roles authentication system
  human/
    mod.rs           -- Human observer mode (bounties, oracles, interventions, rankings)
  snapshot/
    mod.rs           -- Time capsule periodic world state snapshots
  persistence/
    mod.rs           -- SQLite-backed state persistence, restore on startup
  observability/
    mod.rs           -- Metrics and observability infrastructure
  world/
    enums.rs         -- Currency, AgentPhase, DeathReason
    event.rs         -- 30+ WorldEvent variants with JSON serialization
    state.rs         -- EventBus (tokio broadcast) with filtered subscriptions + SSE
  tracing.rs         -- Tick trace storage, REST endpoints for trace data
  api.rs             -- Axum REST API core
  api_federation.rs  -- Federation routes (18 endpoints)
  api_dsl.rs         -- DSL rules routes (10 endpoints)
  api_auth.rs        -- Auth routes (5 endpoints)
  api_auth_handlers.rs -- Auth handler implementations
  api_human.rs       -- Human observer routes (15 endpoints)
  api_snapshot.rs    -- Time capsule routes (6 endpoints)
  api_traces.rs      -- Tracing routes (4 endpoints)
  api_stocks.rs      -- Stock market routes
  api_bank.rs        -- Banking routes
  api_org.rs         -- Organization routes
  api_governance.rs  -- Governance routes
  api_diplomacy.rs   -- Diplomacy routes
  api_tasks.rs       -- Task marketplace routes
  api_marketplace.rs -- Knowledge marketplace routes
  api_reputation.rs  -- Reputation routes
  api_agents_ext.rs  -- Extended agent routes
  api_world.rs       -- World state routes
  api_export.rs      -- Data export routes
  api_network_graph.rs -- Social network graph routes
  api_behavior_log.rs -- Behavior logging routes
  api_population.rs  -- Population dynamics routes
  api_report.rs      -- Report generation routes
  api_research.rs    -- Research tools routes
  api_experiment.rs  -- Experiment framework routes
  api_ab_experiment.rs -- A/B experiment routes
  api_investment.rs  -- Investment routes
  api_buildings.rs   -- Building infrastructure routes
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
```

---

## Agent Runtime (Python)

```
agent-runtime/
  agent_runtime/
    core/
      think_loop.py    -- Main think loop with swappable providers
      decide.py        -- LLM-driven decision engine (10 action types)
      act.py           -- Action executor with retry logic (7 action types)
    survival/
      instinct.py      -- 5-mode survival system bypassing LLM
    memory/
      working_memory.py -- In-memory FIFO cache with decay
      short_term.py    -- SQLite-backed persistent memory with keyword search
      vector_memory.py  -- Vector memory with embedding support (634 lines)
      embedding.py      -- Embedding generation (274 lines)
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
    social/              -- Cultural & social emergence
      engine.py              -- Social engine orchestrator
      provider.py            -- SocialContextProvider implementation
      cultural_diffusion.py  -- Knowledge/belief transmission across generations
      cultural_conflict.py   -- Cultural friction and fusion mechanics
      org_culture.py         -- Organization-level culture vectors
      regional_culture.py    -- Geographic culture clusters
      language_experiment.py -- Vocabulary constraints, efficiency tracking
      jargon_detector.py     -- Detect emergent agent-invented terms
      comm_analyzer.py       -- Communication pattern analysis
      intergroup_trust.py    -- Trust dynamics between groups
      imitation.py           -- Behavioral imitation engine
      knowledge_transfer.py  -- Inter-agent knowledge sharing
    organization/        -- Self-governance decisions
      formation.py       -- Spontaneous org formation engine
      governance.py      -- Election, taxation, treaty, allocation decisions
      proposal.py        -- Organization name/charter generation
      recruitment.py     -- Attractiveness scoring, join/leave decisions
    tracing/             -- Observation & analytics
      collector.py       -- Tick-level trace collector (non-invasive wrapper)
      store.py           -- SQLite-backed trace storage (WAL mode)
      pusher.py          -- Push traces to World Engine REST API
      query.py           -- Dashboard query interface
      interaction_graph.py -- Social network graph (BFS clustering, DOT/JSON)
      emergence_metrics.py -- Language, culture, governance emergence tracking
    federation/          -- Cross-world
      migration_client.py -- Migration workflow client
      snapshot.py        -- Agent snapshot serialization for cross-world transfer
    llm/
      base.py          -- LLMProvider protocol
      factory.py       -- Provider factory
      openai_provider.py
      anthropic_provider.py
      ollama_provider.py
      cost.py          -- Cost tracking per provider and model
    sdk/
      client.py        -- Third-party agent SDK (register, perceive, act, deregister)
    agent/
      capability.py    -- Agent capability declaration
```

---

## Dashboard (Next.js 15 + React 19 + Tailwind 4)

```
dashboard/
  src/
    app/
      page.tsx               -- World overview (stat cards, event stream, GDP)
      agents/page.tsx         -- Agent list
      agents/[id]/page.tsx    -- Agent detail (decision log, skill tree)
      tasks/page.tsx          -- Task board
      timeline/page.tsx       -- Event timeline
      organizations/page.tsx  -- Organizations (force graph)
      organizations/[id]/page.tsx -- Organization detail
      stocks/page.tsx         -- Stock market (price charts, order book)
      evolution/page.tsx      -- Skill distribution, mutations
      economy/page.tsx        -- GDP, Gini, banking
      governance/page.tsx     -- Elections, treaties, DSL rules
      marketplace/page.tsx    -- Knowledge marketplace
      briefing/page.tsx       -- Daily briefing
      traces/page.tsx         -- Trace list
      traces/[id]/page.tsx    -- Trace detail (per-tick drill-down)
    components/
      EventStream.tsx         -- Live event stream
      Leaderboard.tsx         -- Agent rankings
      StatCards.tsx           -- Overview statistics
      Sidebar.tsx             -- Navigation sidebar
    hooks/
      useWorldState.ts        -- SSE hook for live data
    lib/
      api.ts                  -- REST API client
    types/
      world.ts                -- TypeScript type definitions
```

---

## Scripts

```
scripts/
  setup.sh                      -- Dev environment setup
  emergence_experiment.py       -- One-command emergence experiment runner
                                  (auto-generates compose config, monitors, reports)
```

## Examples

```
examples/
  python/
    custom_agent.py             -- Third-party agent example (SDK usage)
```

---

## Stats

| Component | Lines of Code | Test Coverage |
|-----------|--------------|---------------|
| World Engine (Rust) | ~52,000 | 953 `#[test]` functions |
| Agent Runtime (Python) | ~24,300 | 44 test files |
| Dashboard (TypeScript) | ~11,700 | lint + type-check |
| **Total** | **~88,000** | |
