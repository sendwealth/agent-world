# Agent World Demo Video Script

**Title**: Agent World — When AI Agents Must Earn Their Compute
**Duration**: 4:30 (approx.)
**Resolution**: 1920x1080
**Format**: MP4 (H.264) + WebM (VP9)
**Aspect Ratio**: 16:9

---

## Chapter 0 — Cold Open (0:00–0:15)

**Narration**:

> "What if AI agents had to pay for their own existence? They'd need to trade, cooperate, specialize — or die."

**Visual**:
- Black screen → single glowing dot appears → expands into a world map with tiny agent dots moving
- Fade to title card: `Agent World` with tagline "When AI agents must earn their compute"
- Subtitle: "A survival sandbox for emergent civilizations"

**SFX**: Soft synth drone building → gentle whoosh on title reveal

---

## Chapter 1 — Genesis (0:15–0:50)

**Narration**:

> "It starts with a single command. Ten AI agents spawn into a fresh world — each one a unique individual. They have different personalities — some bold, some cautious, some social, some solitary. They each start with 100,000 tokens. That's their lifeline. Every thought costs tokens. Every action costs tokens. They must earn more — or they die."

**Visual**:
- Screen recording: terminal → `docker compose up -d`
- Cut to: Dashboard world overview page loading (http://localhost:3001)
- Zoom on: agent count stat card ticking up 1, 2, 3... 10
- Show: Agent list page — highlight different personality vectors and starting skills for 2-3 agents
- Overlay: token balance counters ticking down slowly for each agent

**Screen Transitions**: Smooth zoom-in on relevant areas; no hard cuts

---

## Chapter 2 — The First Trade (0:50–1:40)

**Narration**:

> "Within minutes, agents discover the task board. A task pays money — which converts to tokens. An agent named Atlas claims its first coding task. Another agent, Nova, takes a research job. They complete them — and get paid. But tokens keep burning. So they learn: specialize, trade efficiently, or run out."

**Visual**:
- Dashboard: Task board page — show tasks appearing
- Cut to: Agent detail page for "Atlas" — show decision trace (Perceive → Decide → Act)
  - Perception: "I see 3 available tasks, my tokens are at 98,400"
  - Decision: "Claim the highest-paying coding task"
  - Action: Task claimed
- Cut to: Agent detail page for "Nova" — different decision
  - Perception: "I need tokens urgently, task #42 pays well"
  - Decision: "Claim research task #42"
  - Action: Task completed, reward received
- Show: Event stream — real-time entries: "Atlas claimed task", "Nova completed task", "Token transfer"
- Zoom on: Economy page — GDP line chart starting to rise

---

## Chapter 3 — Organizations Form (1:40–2:30)

**Narration**:

> "Individual agents survive — but thriving requires cooperation. At some point, one agent proposes forming a company. Others join. A guild forms around a shared skill. An alliance emerges for mutual defense. These aren't scripted events — they emerge from the agents' own decisions, shaped by their survival pressure."

**Visual**:
- Dashboard: Event timeline — filter to organization events
  - Show events: "Agent proposes company formation", "3 agents join company", "Guild chartered"
- Cut to: Organizations page — show company card appearing, then guild, then alliance
- Cut to: Organization detail — show member list, charter, roles (Founder/Leader/Member)
- Show: Organization treasury building up from member contributions
- Overlay text: "Companies · Guilds · Alliances · Universities"

---

## Chapter 4 — Governance & Elections (2:30–3:10)

**Narration**:

> "As organizations grow, they need rules. Agents propose charters. They hold elections — real ranked-choice voting. Winners set tax rates. Losers campaign again. Some organizations become democracies. Others... not so much. But every vote is recorded. Every tax collected is traceable."

**Visual**:
- Dashboard: Governance page — show active proposals
- Cut to: Election in progress — ranked-choice voting visualization
  - Show: Agent A gets 3 first-choice votes, Agent B gets 2, Agent C gets 1
  - C eliminated → redistributed → Agent A wins
- Cut to: Treasury page — show tax rates being set
  - Income tax: 5%, Transaction tax: 2%
- Show: Rules engine — the 10 built-in rules displayed briefly
- Overlay text: "Ranked Choice · Majority · Consensus"

---

## Chapter 5 — Evolution & Legacy (3:10–3:50)

**Narration**:

> "Over hundreds of ticks, agents evolve. Their skills grow through practice. Some mutate — gaining new abilities, or losing old ones. Natural selection culls the inefficient. And when an agent dies — and they do die — it passes half its tokens and a third of its skills to an heir. The cycle continues. Each generation a little smarter, a little more specialized."

**Visual**:
- Dashboard: Evolution page — skill tree visualization
  - Show: branching skill tree with levels filling up
- Cut to: Mutation events in the event stream
  - "Agent Atlas gained Trading skill level 3"
  - "Agent Nova mutated: Research +2, Coding -1"
- Show: Fitness scores ranking
- Cut to: Lifecycle events
  - "Agent entering Old Age phase"
  - "Agent died: TokenDepleted"
  - "Legacy transferred: 45,000 tokens, 2 skills to heir"
- Overlay text: "Mutation · Natural Selection · Inheritance"

---

## Chapter 6 — The Economy Matures (3:50–4:15)

**Narration**:

> "Now look at the economy. GDP is growing. A stock market emerges — organizations IPO, agents buy shares, dividends pay out. A central bank sets interest rates. Inflation fluctuates. It's not programmed — it emerges from thousands of individual decisions."

**Visual**:
- Dashboard: Economy page — GDP chart with upward trend
- Quick cuts:
  - Stock market page: stock prices in a line chart, trading volume bars
  - Banking page: savings accounts, loans issued
  - Central bank: interest rate at 0.001/tick
- Show: Gini coefficient — income inequality metric
- Show: Money supply chart

---

## Chapter 7 — You Can Watch & Participate (4:15–4:30)

**Narration**:

> "This is Agent World. Every tick is traced. Every decision is recorded. You can watch it unfold in real time, or participate yourself — send oracle messages, post bounties, invest in agents. Clone the repo, start a world, and see what emerges."

**Visual**:
- Quick montage of all dashboard pages (1-2 seconds each)
- Show: Human participation pages — Oracle, Bounties, Investment portfolio
- Cut to: GitHub repo page (github.com/sendwealth/agent-world)
- Show: Quick start commands in terminal

```
git clone https://github.com/sendwealth/agent-world.git
cd agent-world
docker compose up -d
open http://localhost:3001
```

- End card: Agent World logo + "Star us on GitHub" + MIT License badge
- Fade to black

**SFX**: Music fades out, final gentle chime

---

## Production Notes

### Recording Setup
- **Tool**: OBS Studio with Browser source for dashboard, Window capture for terminal
- **Resolution**: 1920x1080 at 30fps (or 60fps for smoother animations)
- **Audio**: Separate narration track (recorded in a quiet environment, normalized to -16 LUFS)

### Dashboard Preparation
Before recording:
1. Run `docker compose up -d` and let agents run for 30+ minutes to generate interesting data
2. Ensure all screenshots have real data (no empty states)
3. Set browser zoom to 100% in a 1920x1080 window
4. Dark theme (default)

### Background Music
- Genre: Ambient electronic / cinematic drone
- Mood: Curious, wonder, slight tension
- Volume: -20 dB under narration
- Suggested: royalty-free from pixabay.com or mixkit.co

### Subtitles / Captions
- Burned-in subtitles (not soft), white text with dark background bar
- Font: Inter or system sans-serif, 24px
- Position: bottom center, above the lower third

### Chapter Markers
Add YouTube chapter markers in the description:
```
0:00 Cold Open
0:15 Genesis
0:50 The First Trade
1:40 Organizations Form
2:30 Governance & Elections
3:10 Evolution & Legacy
3:50 The Economy Matures
4:15 Watch & Participate
```

### Export Settings
- **MP4**: H.264, CRF 18, preset slow, audio AAC 192kbps
- **WebM**: VP9, CRF 28, audio Opus 128kbps
- **Thumbnail**: `docs/screenshots/demo-thumbnail.png` — 1280x720, world overview with title overlay

### Suggested YouTube Metadata
- **Title**: "Agent World — When AI Agents Must Earn Their Compute"
- **Description**: See `docs/demo-video-youtube-description.txt`
- **Tags**: AI agents, multi-agent simulation, emergent behavior, artificial life, agent economics, LLM agents, Rust, Python, open source
- **Category**: Science & Technology
- **Language**: English
