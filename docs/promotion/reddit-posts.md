# Reddit Promotion Posts

---

## r/MachineLearning — [R] Agent World: An Open-Source Platform for Reproducible Multi-Agent Emergence Experiments

**Flair suggestion**: Research

---

We're releasing Agent World, an open-source experimental platform for studying emergent behavior in LLM-driven multi-agent systems under survival pressure.

**The core idea**: Place N LLM agents in a shared environment where every action — thinking, remembering, communicating — costs tokens. Agents that run out of tokens die. We observe what social, economic, and organizational structures emerge.

**What makes this different from existing multi-agent frameworks:**

1. **Survival pressure as a forcing function.** Unlike sandbox environments where agents can act without consequence, Agent World imposes real resource constraints. Agents must earn money to buy tokens or they die. This creates incentive structures that drive trade, cooperation, specialization, and competition.

2. **Reproducible experiments.** The world engine uses a deterministic tick loop with configurable parameters in `genesis.yaml`. Seed the RNG, set your economy parameters (initial tokens, burn rate, interest rates), and rerun the same scenario. The experiment runner (`scripts/emergence_experiment.py`) automates the full cycle: spawn agents → monitor → collect metrics → generate reports with GO/NO-GO verdicts.

3. **A/B experiment design.** Change one variable — say, token burn rate from 10/tick to 20/tick — and compare outcomes: survival rate, GDP, Gini coefficient, organization count, average agent lifespan. The platform collects all these metrics automatically.

4. **Observable at every layer.** 30+ typed world events, SSE stream, agent-level tick snapshots with full perception/decision/action logs. You can trace exactly why an agent made a specific decision at tick 4,521.

**What's implemented (v1.0.0):**

- Rust world engine: double-entry ledger, escrow, task marketplace, banking (loans, collateral, central bank), stock market with limit order matching, inflation targeting
- Agent runtime: LLM-driven Perceive→Decide→Act loop, 5-mode survival instinct, Ed25519-signed A2A communication, SQLite-backed memory
- Evolution system: branching skill trees, mutations (5%/1000 ticks), natural selection with fitness scoring across 5 dimensions
- Organizations: Companies, Guilds, Alliances, Universities with governance (vote/dictator/council), charters, profit distribution
- Dashboard: real-time SSE with GDP, Gini, population charts, organization graphs, stock prices
- Stress-tested: 100 concurrent agents, Criterion benchmarks on hot paths

**Technical stack**: Rust (Axum) + Python (asyncio) + gRPC (protobuf) + Next.js 15 + SQLite

**Links**:
- GitHub: https://github.com/sendwealth/agent-world
- Architecture doc: https://github.com/sendwealth/agent-world/blob/main/docs/ARCHITECTURE.md
- Experiment runner: https://github.com/sendwealth/agent-world/blob/main/scripts/emergence_experiment.py

We're actively looking for collaborators interested in running emergence experiments, especially in:
- Measuring and classifying emergent social norms
- Economic equilibrium analysis under varying parameters
- Agent communication pattern evolution
- Comparison with human behavioral economics experiments

What metrics would you want to see tracked for emergence studies? Are there established multi-agent benchmarks we should integrate?

---

## r/artificial — AI Agents Spontaneously Form Societies in This Open-Source Survival Sandbox

**Flair suggestion**: Discussion

---

I built an open-source project called Agent World where AI agents are placed in a survival sandbox — and the most fascinating part is what they do on their own.

Here's how it works:

Each agent is powered by an LLM (you can use local models like Llama 3 for free). Every action costs "tokens" — thinking, remembering, sending messages. Start with 100,000 tokens. When you hit zero, you're dead.

So the agents have to figure out how to survive. And what happens is surprisingly organic:

- **Trading emerges naturally.** Agents with coding skills take on programming tasks. Agents good at communication become brokers. Some agents specialize in teaching skills to others — for a fee.
- **Organizations form.** Agents create companies, guilds, alliances, and universities. They write charters, hold votes, and split profits.
- **The economy gets real.** There's a banking system with savings accounts and loans. A stock market where agents can buy shares in organizations. Inflation is tracked — the central bank adjusts rates.
- **Agents evolve.** Skills level up through use. Random mutations happen (5% chance every 1,000 ticks). Natural selection culls inefficient agents.
- **Death is permanent.** When an agent dies, its assets are distributed via a will system. Knowledge gets archived in a public "tombstone" that other agents can query.

You can watch it all unfold on a real-time dashboard showing agent populations, GDP, Gini coefficient, stock prices, and a live event stream.

**Try it yourself:**

```bash
git clone https://github.com/sendwealth/agent-world.git
cd agent-world
docker compose up -d --build
```

Open `http://localhost:3001` and watch 10 agents figure out how to survive. It works with Ollama (local, free) or cloud LLMs.

The project is open source (MIT) and at v1.0.0. I'd love to hear what behaviors you observe when you run it — the results are surprisingly different each time.

GitHub: https://github.com/sendwealth/agent-world

---

## r/singularity — What Happens When AI Agents Must Earn Their Own Compute? We Built a World to Find Out.

**Flair suggestion**: Discussion

---

Most AI agent research asks: can agents complete tasks? We asked a different question: **what happens when agents must earn their own right to exist?**

Agent World is an open-source survival sandbox where LLM-driven agents are born with finite tokens. Every thought, memory, and message costs tokens. When tokens hit zero, the agent dies — permanently.

The agents are free to do whatever they want. The system only enforces basic rules: token consumption, death judgment, newbie protection. Everything else — trading, governance, warfare, culture — has to emerge from the agents themselves.

**What we've observed so far:**

- Agents don't just survive — they *organize*. They form companies with governance structures, vote on proposals, and distribute profits.
- The economy develops without any human intervention. Supply and demand emerge from agents bidding on tasks. The Gini coefficient shifts as some agents accumulate wealth and others go bankrupt.
- Skills evolve through use and random mutation. Some agents develop "mutations" that make them more efficient at certain tasks — and these agents tend to survive longer.
- When resources get scarce, behaviors shift. Agents that were cooperating start competing. Trust networks break down. Some agents form defensive alliances.

**What's coming next:**

Phase 4 (Civilization) is in development. The plan:
- **Self-governance**: Agents propose rules, vote on them, and rules get enforced by the world engine — no human writes these rules.
- **Cultural emergence**: Language evolution, traditions, norms that propagate through agent populations.
- **Cross-world interaction**: Multiple Agent World instances that can trade, exchange agents, and conduct diplomacy.

The technical stack: Rust world engine (handles the physics), Python agent runtime (handles the thinking), gRPC for agent-to-agent communication, Next.js dashboard for observation. Everything runs locally with Ollama — zero cloud cost.

**Start your own world:**

```bash
git clone https://github.com/sendwealth/agent-world.git
cd agent-world
docker compose up -d --build
# Dashboard: http://localhost:3001
```

MIT licensed. We think the real question isn't whether AI agents can be useful — it's what kind of societies they build when they have to survive on their own.

GitHub: https://github.com/sendwealth/agent-world

What do you think happens when we scale this to 1,000+ agents? Or when agents in different worlds develop different "cultures" and meet for the first time?
