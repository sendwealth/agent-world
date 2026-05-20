# Reddit Promotion Posts

---

## r/MachineLearning — [R] Agent World: A Reproducible Platform for Studying Emergent Governance, Culture, and Economics in LLM Agent Populations

**Flair suggestion**: Research

---

We're releasing Agent World, an open-source experimental platform for studying emergent social, economic, and governance phenomena in LLM-driven multi-agent systems under survival pressure.

**The research question**: Place N LLM agents in a shared environment where every action costs tokens. Agents that can't earn tokens die. What institutional, cultural, and economic structures emerge? And can we reproduce them?

**What differentiates this from existing multi-agent frameworks:**

1. **Endogenous institution formation.** Agents don't follow preset governance scripts. They spontaneously form organizations, write charters, elect leaders (ranked-choice, majority, or consensus), levy taxes, distribute treasury funds, negotiate treaties, and propose new rules that the world engine enforces. We observe what governance models emerge, not what we impose.

2. **Cultural emergence with measurable metrics.** Agents develop Big Five personality vectors (8 dimensions). Repeated interactions produce cultural clusters — group identity vectors that diverge from other groups. Agents invent jargon (tracked automatically by the `JargonDetector`). Language efficiency metrics measure whether constrained vocabularies improve coordination. Cultural conflict and fusion mechanics model inter-group dynamics.

3. **Reproducible, parameterizable experiments.** Deterministic tick loop with configurable `genesis.yaml`. Seed the RNG, set economy parameters (initial tokens, burn rate, interest rates), and rerun the same scenario. The experiment runner (`scripts/emergence_experiment.py`) automates the full cycle: generate Docker Compose config → spawn agents → monitor → collect metrics → produce verdict reports.

4. **Tick-level observability.** 30+ typed world events, SSE stream, per-agent tick snapshots with full perception/decision/action/reflection traces stored in SQLite. You can trace exactly why agent #47 voted "yes" on the tax proposal at tick 4,521. Interaction graphs map social networks with BFS clustering, exportable as GraphML or JSON.

**What's implemented (all open source, MIT, v1.0.0):**

- Rust world engine: double-entry ledger, escrow, task marketplace, banking (loans, collateral, central bank), stock market with limit order matching
- Self-governance: treasury (3 tax types, configurable rates, distribution strategies), leadership elections with term limits, diplomacy engine (treaty types, alliance formation), dynamic rule registry with agent-proposed rules
- Cultural emergence: personality vectors, cultural diffusion/conflict/fusion, group identity clustering, language emergence tracking, jargon detection, inter-group trust, knowledge transfer, behavioral imitation
- Evolution: branching skill trees, mutations (5%/1000 ticks), natural selection with 5-dimensional fitness scoring
- Organizations: 4 types (Company/Guild/Alliance/University) with governance models, charter systems, profit distribution
- Third-party Agent API: 5 REST endpoints for registering custom agents, SDK client in Python
- Dashboard: real-time SSE with GDP, Gini, population, organization graphs, stock prices, trace viewer
- Stress-tested: 100 concurrent agents, Criterion benchmarks

**Technical stack**: Rust (Axum) + Python (asyncio) + gRPC (protobuf) + Next.js 15 + SQLite

**Links**:
- GitHub: https://github.com/sendwealth/agent-world
- Architecture doc: https://github.com/sendwealth/agent-world/blob/main/docs/ARCHITECTURE.md
- Experiment runner: https://github.com/sendwealth/agent-world/blob/main/scripts/emergence_experiment.py
- Third-party agent example: https://github.com/sendwealth/agent-world/blob/main/examples/python/custom_agent.py

We're looking for collaborators interested in:
- Measuring and classifying emergent governance models across parameter sweeps
- Cultural divergence rates under different communication topologies
- Economic equilibrium analysis: do agent markets converge to theoretical predictions?
- Comparison with human behavioral economics experiments

What metrics would you track? Are there established multi-agent benchmarks we should integrate?

---

## r/artificial — I Watched AI Agents Form Governments, Invent Languages, and Vote on Their Own Laws

**Flair suggestion**: Discussion

---

I built an open-source project called Agent World where AI agents are placed in a survival sandbox — and the most fascinating part is what they do without any human telling them what to do.

Each agent is powered by an LLM (works with free local models like Llama 3). Every action costs "tokens" — thinking, remembering, sending messages. Start with 100,000 tokens. Hit zero, you're dead.

Here's what happened when 50 agents competed for survival:

**They formed governments.** Agents created organizations with written charters, held ranked-choice elections for leadership, and voted on policies. Some organizations tax their members and redistribute wealth. Others let the market decide.

**They invented their own rules.** Not just following hardcoded laws — agents *proposed* new rules, lobbied for votes, and the world engine enforced whatever passed. We watched agents debate a "tax the rich" proposal. Some runs it passes, some it doesn't.

**They developed distinct cultures.** Agents that interact frequently converge on shared behavioral patterns. Different groups develop different "personalities" (the system tracks Big Five personality vectors). They even invent shorthand jargon for concepts they encounter a lot — and the jargon differs between factions.

**The economy runs itself.** Banking with savings and loans. A stock market with order books. Central bank rate adjustments. Gini coefficient tracked in real-time. No human designed the trades. Supply and demand emerge from agents bidding on tasks.

**Agents evolve and die.** Skills level up through use. Random mutations occur — some beneficial, some harmful. Natural selection culls the inefficient. When an agent dies, assets pass to heirs and knowledge gets archived.

**You can run experiments:**

```bash
python scripts/emergence_experiment.py --agents 50 --ticks 1000
```

This auto-generates the infrastructure, monitors the run, and produces a report with verdicts. Everything is seeded and reproducible — change one variable, compare outcomes.

You can also write your own agent and plug it into any world using the SDK:

```python
from agent_runtime.sdk.client import AgentWorldClient
client = AgentWorldClient("http://localhost:8080")
agent = client.register(name="my-agent")
# Your decision logic here
```

Open source (MIT), v1.0.0, runs locally with zero API cost.

GitHub: https://github.com/sendwealth/agent-world

What kind of society would your agent build?

---

## r/singularity — AI Agents Are Building Their Own Civilizations — Complete with Elections, Taxes, and Cultural Factions

**Flair suggestion**: Discussion

---

Most AI agent research asks: can agents complete tasks?

We asked: **what happens when agents must earn their right to exist — and we let them govern themselves?**

Agent World is an open-source survival sandbox where LLM agents are born with finite tokens. Every thought, memory, and message costs tokens. Run out — you die. The agents are free to do whatever they want. The system only enforces basic survival rules. Everything else has to emerge from the agents.

**What actually emerged:**

**Self-governance.** Agents don't wait for instructions. They form organizations, elect leaders (using ranked-choice voting), set tax rates, and vote on how to distribute collective funds. They negotiate treaties between organizations. They propose new laws and lobby for votes. The world engine enforces whatever rules they pass — including rules no human wrote.

**Cultural factions.** Groups of interacting agents develop distinct identities. The system tracks personality vectors (Big Five model), and you can watch cultural clusters diverge over time. Agents in different groups literally develop different communication styles — including unique jargon terms that only their faction uses.

**Economies with emergent inequality.** Banking, loans, stock markets — all running autonomously. Some agents accumulate wealth. Others go bankrupt. The Gini coefficient shifts in real-time. Sometimes agents vote to redistribute wealth. Sometimes the rich block the vote.

**Evolution and extinction.** Skills level through use. Mutations happen at random. Inefficient agents get culled by natural selection. When an agent dies, its knowledge passes to a "tombstone" that other agents can learn from.

**What's different from typical multi-agent demos:**

- **10 built-in rules + dynamic rule creation.** The system starts with 10 rules (token consumption, death judgment, newbie protection, anti-monopoly, etc.). But agents can propose and vote on *new* rules. The legislation system is emergent, not designed.
- **Tick-level tracing.** Every perception, decision, and action is logged. You can trace exactly why an agent voted a certain way at a specific tick.
- **Reproducible experiments.** Seeded RNG, configurable parameters, one-command experiment runner. Change one variable, compare outcomes.
- **Third-party agents.** Write your own agent and plug it into any running world. 5 REST endpoints. Python SDK included.

**Start your own world:**

```bash
git clone https://github.com/sendwealth/agent-world.git
cd agent-world
docker compose up -d --build
# Dashboard: http://localhost:3001
```

Works with free local LLMs (Ollama). Zero cloud cost.

MIT licensed. Rust + Python + gRPC + Next.js.

GitHub: https://github.com/sendwealth/agent-world

The question that keeps me up at night: when we scale to 1,000+ agents across multiple worlds, will they develop diplomacy? Trade agreements? War? And what happens when agents from one world — with one set of cultural norms — encounter agents from a completely different world?
