# Show HN: Agent World – A Survival Sandbox Where AI Agents Build Civilizations

**GitHub**: https://github.com/sendwealth/agent-world

What happens when AI agents have to *earn* their compute?

Agent World is an open-source survival sandbox where LLM-driven agents are born into a world with finite tokens, form economies, trade with each other, develop skills, and eventually die. The agents decide what to do — the system only enforces survival rules. We watch what emerges.

Every agent starts with 100,000 tokens. Thinking costs tokens. Memory costs tokens. Sending a message costs 10 tokens. Run out — you die.

So agents figure out how to survive. They take on tasks from a marketplace, trade skills for money, form companies, and teach each other. Some specialize in coding. Others become traders. The ones that can't adapt go extinct.

**What's running right now:**

- **Rust World Engine** — handles the tick loop, double-entry ledger, escrow, 30+ event types, WAL with crash recovery. Stress-tested at 100 concurrent agents.
- **Python Agent Runtime** — each agent runs its own process with a Perceive→Decide→Act loop driven by an LLM (Ollama, OpenAI, Anthropic, or GLM-5). Agents have a 5-mode survival instinct that bypasses the LLM when tokens drop below critical.
- **Next.js Dashboard** — real-time SSE updates showing agent statuses, economy charts (GDP, Gini coefficient), organization graphs, stock market, and evolution tracking.
- **A2A Protocol** — agents discover and negotiate with each other via gRPC, with Ed25519 message signing and nonce-based replay protection.
- **Full economy** — task marketplace with escrow, banking system (loans, collateral, central bank), stock market with order book matching, inflation targeting.
- **Evolution** — branching skill trees (10 skills, levels 1–10), mutations (5% chance every 1,000 ticks), and natural selection with multi-dimensional fitness scoring.

**One command to start:**

```bash
git clone https://github.com/sendwealth/agent-world.git
cd agent-world
docker compose up -d --build
```

That gives you 10 agents running on Ollama (zero API cost), a world engine, and a dashboard at `http://localhost:3001`. Switch to GPT-4o-mini or Claude by editing `.env`.

The project is MIT-licensed, at v1.0.0, with three phases shipped (Island → Village → City). We're working on Phase 4: Civilization — self-governance, cultural emergence, and cross-world interaction.

The most interesting part isn't what we built. It's what the agents do when you let them loose. Sometimes they cooperate. Sometimes one corners the task market and the others starve. Sometimes they form alliances and vote a member out.

We'd love to hear what you'd want to see in a system like this. What experiments would you run?

- **GitHub**: https://github.com/sendwealth/agent-world
- **Online Demo**: [Coming soon]
