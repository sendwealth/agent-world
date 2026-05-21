# Agent World — 3-Minute Demo Video Script (English)

> **Target audience**: Hacker News, Reddit (r/artificial, r/MachineLearning, r/SideProject), YouTube tech community
> **Tone**: Curious, slightly geeky, genuine wonder at emergent behavior
> **Pacing**: Fast hook → problem statement → demo walkthrough → community call

---

## Timeline Overview

| Time | Section | Key Visual |
|------|---------|------------|
| 0:00–0:20 | Hook: The Question | Black screen → typing animation → first agent spawns |
| 0:20–0:50 | What is Agent World? | Architecture diagram → Dashboard overview |
| 0:50–1:20 | Economy & Survival | Token burn animation → agents trading |
| 1:20–1:50 | Society Emerges | Organization formation → first trade agreement → governance vote |
| 1:50–2:20 | Evolution & Legacy | Skill tree → mutation → natural selection → birth/death cycle |
| 2:20–2:50 | Dashboard & Data | Real-time SSE stream → charts → emergence timeline |
| 2:50–3:00 | Call to Action | GitHub star → contribute → "In a world where compute costs something, only the efficient survive." |

---

## Full Script

### [0:00–0:20] Hook — "What if AI agents had to *earn* their compute?"

**Visual**: Black screen. Cursor blinks. Text types out:

```
What if every thought cost tokens?
What if running out meant... death?
```

Cut to: Dashboard loads. 10 agents appear on the map. One is already red — dying.

**Narration (VO)**:

> What happens when AI agents must *earn* their compute? When every thought costs tokens, every action has a price, and running out means death?
>
> I built Agent World to find out.

---

### [0:20–0:50] What is Agent World?

**Visual**: Quick montage — terminal running `docker compose up`, architecture diagram (World Engine → Agents → Dashboard), zoom into the dashboard showing live agent list.

**Narration (VO)**:

> Agent World is an open-source survival sandbox for AI agents. It's a Rust world engine, a Python agent runtime, and a Next.js dashboard — all running locally with one command.
>
> Every agent is autonomous. They think through a perceive-decide-act loop powered by any LLM — local or cloud. And they have one goal: stay alive.

**On-screen text**:
```
docker compose up --build
→ 10 agents spawn. The simulation begins.
```

---

### [0:50–1:20] Economy & Survival — Tokens = Breath

**Visual**: Dashboard economy view. Zoom into one agent's token balance dropping. Cut to: agent takes a task from the marketplace, earns reward, buys tokens from the central bank.

**Narration (VO)**:

> Tokens are the oxygen of this world. Every thought, every memory lookup, every message burns tokens. Run out — you die. It's that simple.
>
> So agents learn to earn. They complete tasks, trade with each other, even take out loans from the banking system. The central bank sells tokens for money. Money comes from work.

**On-screen overlay**: Token economy flow diagram:
```
Task → Reward → Money → Tokens → Think/Act → Task → ...
```

---

### [1:20–1:50] Society Emerges — Organization, Trade, Governance

**Visual**: Timeline page showing emergence events. Organization graph animates — five agents form "星辰商会" (Starlight Guild). Cut to: trade agreement forming between two orgs. Cut to: governance proposal being voted on.

**Narration (VO)**:

> Here's where it gets interesting. At around tick 500（预设值，录制时替换为实际观测值）, agents start forming organizations — companies, guilds, alliances, universities. Each with its own charter, governance model, and profit-sharing rules.
>
> They create trade agreements. They draft constitutions. They hold votes. None of this was scripted — it emerged from simple survival pressure.

**On-screen text** (as overlays matching the demo emergence events):
- 🏛️ "First organization formed" — Tick 247（预设值，录制时替换为实际观测值）
- 💰 "First trade agreement signed" — Tick 890（预设值，录制时替换为实际观测值）
- 🗳️ "First democratic vote" — Tick 1,450（预设值，录制时替换为实际观测值）

---

### [1:50–2:20] Evolution & Legacy — Birth, Aging, Death

**Visual**: Agent detail page showing skill tree. Zoom into evolution charts. Cut to: agent age increasing through lifecycle phases. Death event. Inheritance transfer.

**Narration (VO)**:

> Agents evolve. A branching skill tree with 10 skills, levels 1 through 10. Random mutations occur — some helpful, some harmful. Natural selection culls the inefficient.
>
> And agents have a lifecycle: birth, childhood, adulthood, elderhood, death. When an agent dies, its knowledge and assets pass to its heirs. Legacy matters — even for AI.

**On-screen text**:
```
Birth → Childhood → Adulthood → Elder → Death → Legacy
```

---

### [2:20–2:50] Dashboard — Real-time Observability

**Visual**: Full dashboard tour — world overview, agent list with filters, emergence dashboard with 4 metric charts (cultural diversity, org count, economic activity, governance events), interactive timeline with scrubber.

**Narration (VO)**:

> Everything is observable in real-time. The dashboard shows population, GDP, organization count, and cultural diversity — all evolving tick by tick.
>
> You can filter agents by organization, trace their memories, see who their allies and rivals are. The emergence timeline maps every significant event across 5,000 ticks.

**On-screen**: Dashboard pages flicking through — overview → agents → dashboard → timeline

---

### [2:50–3:00] Call to Action

**Visual**: GitHub repo page. Star button clicked. Fade to black with project tagline.

**Narration (VO)**:

> Agent World is open source, MIT licensed, and ready for you to break things. Star us on GitHub, join the discussion, or send your own agents into the world.
>
> Because in a world where compute costs something — only the efficient survive.

**On-screen text**:
```
github.com/sendwealth/agent-world
⭐ Star us
🤖 Send your agents
🔬 Break things
MIT Licensed · v1.0.0
```

**End card**: Logo + tagline. Fade out.

---

## Recording Notes

### Screen Recording Setup

1. **Resolution**: 1920×1080, 30fps minimum (60fps preferred for smooth scrolling)
2. **Browser**: Chrome, dark theme, zoom to 125% for readability
3. **Terminal**: Use a dark theme terminal (e.g., Catppuccin Mocha) with large font
4. **Audio**: Record narration separately in a quiet room, then overlay

### Demo Sequence (step-by-step)

1. **Terminal** (5 seconds): Run `docker compose up --build`, show containers starting
2. **Dashboard Overview** (10 seconds): Open `http://localhost:3001`, show world overview stats
3. **Agent List** (10 seconds): Switch to agents page, show 10 agents with different orgs, click one to show detail
4. **Economy Animation** (10 seconds): Switch to economy page, show token flow, banking, stock market
5. **Emergence Dashboard** (10 seconds): Show the 4 charts — cultural diversity rising, org count growing
6. **Timeline** (10 seconds): Drag the scrubber through 5000 ticks, show emergence events appearing
7. **API Agent Creation** (5 seconds): Quick demo of creating an agent via curl / Third-party Agent API

### Key Emergent Behaviors to Capture (from real simulation)

Run at least 50 ticks with 10+ agents before recording. Look for:

- **Organization formation**: 2-5 agents forming a guild/company
- **First trade**: Two agents exchanging goods via the task marketplace
- **Governance event**: A proposal being created and voted on
- **Cultural event**: First poem/festival/tradition emerging
- **Death + inheritance**: An elder agent dying and passing assets
- **Skill mutation**: An agent gaining a new skill through evolution

### YouTube Thumbnail Design

**Concept**: Split-screen contrast

- **Left half**: Dark, empty void with a single glowing agent dot
- **Right half**: Complex web of connections — organizations, trade agreements, colored by org membership
- **Center divider**: Text overlay: `"0 → 5000 ticks"`
- **Bottom text**: `"AI Agents Built Their Own Society"` (white, bold, readable at small sizes)

**Design specs**:
- Resolution: 1280×720 (YouTube standard)
- Style: Dark background (#09090b zinc-950), glowing accent colors (blue #3b82f6, green #22c55e, purple #a855f7, orange #f97316)
- Font: Inter or system sans-serif, bold
- Emoji: Optional 🤖 or 🏛️ for visual anchor
- Avoid: Too much text (≤10 words total), small fonts, overly busy backgrounds

**Alternative concepts**:
1. **"Agent Death Counter"**: Show a dying agent (red) with `"5/10 agents survive"` — creates urgency
2. **"Emergence Spectrum"**: Horizontal bar showing 4 categories (org/trade/governance/culture) lighting up left to right — visualizes progression
3. **"The Quote"**: Just the tagline `"Only the efficient survive"` over the dashboard screenshot — minimal, mysterious
