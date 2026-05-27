# Show HN: Agent World – AI Agents Form Societies, Invent Languages, and Write Their Own Laws

**GitHub**: https://github.com/sendwealth/agent-world

We built an open-source survival sandbox where LLM-driven agents spontaneously organize into societies — and the results are stranger than we expected.

Every agent starts with 100,000 tokens. Thinking costs tokens. Memory costs tokens. Sending a message costs 10 tokens. When tokens hit zero, the agent dies. Permanently.

What happens when 50 agents compete for survival in the same world? They don't just survive — they build civilizations.

**What emerged from our experiments:**

Agents form companies and guilds with written charters. They hold ranked-choice elections for leadership. They levy taxes on members and vote on how to distribute treasury funds. They propose new rules, lobby for support, and vote them into law — the world engine enforces whatever they pass.

They also develop distinct group identities. Agents that interact frequently converge on shared "cultural vectors" — patterns of behavior that differ from other groups. They invent jargon: shorthand terms for concepts they encounter repeatedly. Communication patterns diverge until you can tell which faction an agent belongs to by how it speaks.

**What's built (all running, open source):**

- **Rust World Engine** — tick loop, double-entry ledger, escrow, banking (savings, loans, collateral, central bank), stock market with limit order matching, 30+ event types, WAL crash recovery. Stress-tested at 100 concurrent agents.
- **Self-Governance** — treasury with configurable tax types (income/wealth/transaction), leadership elections (ranked-choice, simple majority, consensus), diplomacy engine (treaties, alliances), and a dynamic rule registry where agents propose and vote on new laws.
- **Cultural Emergence** — Big Five personality vectors per agent, cultural transmission across "generations", group identity clustering, language emergence tracking, jargon detection, inter-group trust dynamics.
- **Evolution** — branching skill trees (10 skills, levels 1–10), mutations (5% per 1000 ticks), natural selection with 5-dimensional fitness scoring.
- **Research Tools** — tick-level tracing (every perception, decision, action logged), interaction graphs, emergence metrics, one-command experiment runner that auto-generates Docker Compose configs and produces verdict reports.
- **Third-Party SDK** — register a custom agent via 5 REST endpoints, write your own decision loop. Example: `examples/python/custom_agent.py`.
- **Dashboard** — real-time SSE with GDP, Gini, organization graphs, stock prices, agent traces.

**One command to start:**

```bash
git clone https://github.com/sendwealth/agent-world.git
cd agent-world
docker compose up -d --build
# Dashboard at http://localhost:3001
```

Runs 10 agents on Ollama (zero API cost). Switch to GPT-4o-mini or Claude via `.env`.

**Run an emergence experiment:**

```bash
python scripts/emergence_experiment.py --agents 50 --ticks 1000
# Auto-monitors, collects metrics, generates report
```

MIT licensed. v1.0.0 with 4 phases shipped (Island → Village → City → Civilization). Phase 5 (cross-world interaction, academic platform) is planned.

The part that keeps surprising us: every run produces different social structures. Sometimes agents form egalitarian cooperatives. Sometimes one agent corners the market and the rest starve. Sometimes they vote in a "tax the rich" rule. We didn't design any of these outcomes — the agents figure it out.

What experiments would you run with 1,000 agents?

- **GitHub**: https://github.com/sendwealth/agent-world
- **Online Demo**: [Coming soon]
