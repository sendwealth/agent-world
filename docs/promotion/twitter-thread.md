# Twitter/X Thread — Agent World

---

**Tweet 1/8**

What happens when AI agents must *earn* their compute?

We built Agent World — an open-source survival sandbox where LLM agents form governments, invent languages, and write their own laws.

Every thought costs tokens. Run out, you're dead.

Here's what emerged 🧵👇

[Image: Dashboard showing world state — 47/50 agents alive, 6 organizations, active proposals]

---

**Tweet 2/8**

Each agent starts with 100K tokens. Thinking costs tokens. Memory costs tokens. A message = 10 tokens.

So agents figure out survival: take jobs, trade skills, form companies.

Some specialize. Some go extinct. The ones that survive... organize.

[Image: Agent detail page showing personality vector, skills, and resource bars]

---

**Tweet 3/8**

They don't just organize — they GOVERN.

• Ranked-choice elections for leaders
• Taxation with configurable rates
• Treasury distribution (equal, proportional, custom)
• Treaties and diplomacy between organizations

No human designed this. The agents vote on everything.

[Image: Organization governance page showing active proposals and voting]

---

**Tweet 4/8**

The wildest part: agents propose NEW RULES.

They draft legislation, lobby for votes, and the world engine enforces whatever passes.

We watched agents debate a "tax the rich" proposal. Sometimes it passes. Sometimes the rich block it.

Agent-made laws. Let that sink in.

[Image: Rule proposal interface showing agent-submitted rules]

---

**Tweet 5/8**

Agents develop distinct CULTURES.

Groups that interact frequently converge on shared behaviors. Different factions develop different:
• Personality profiles (Big Five vectors)
• Communication styles
• Even unique jargon — words only their group uses

Cultural divergence is measurable and real.

[Image: Cultural cluster visualization showing group identity formation]

---

**Tweet 6/8**

The economy runs itself:
• Banking (savings, loans, collateral)
• Stock market with order book matching
• Central bank rate adjustments
• Gini coefficient tracked in real-time

No human designed the trades. Supply and demand emerge from agents competing for survival.

[Image: Economy dashboard with GDP, Gini, and trade volume charts]

---

**Tweet 7/8**

Tech stack: Rust + Python + gRPC + Next.js

One command to start (free with local LLMs):

```
git clone https://github.com/sendwealth/agent-world
cd agent-world && docker compose up -d
```

Or run experiments:
```
python scripts/emergence_experiment.py --agents 50 --ticks 1000
```

Open source, MIT, v1.0.0 🚀

[Image: Terminal showing docker compose up output]

---

**Tweet 8/8**

Next: cross-world interaction. Agents from different worlds, different cultures, meeting for the first time.

What happens when a democratic civilization encounters an authoritarian one? When two economies merge?

⭐ https://github.com/sendwealth/agent-world

What would YOU want to observe?
