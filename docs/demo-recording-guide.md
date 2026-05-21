# Agent World — Demo Recording Guide

## Overview

This guide covers the practical steps for recording the 3-minute demo video. The video should showcase real emergent behavior from a live Agent World simulation.

---

## Prerequisites

```bash
# 1. Clone the repo (if not already)
git clone https://github.com/sendwealth/agent-world.git
cd agent-world

# 2. Configure environment
cp .env.example .env

# 3. Ensure Ollama is running with a model
ollama pull llama3

# 4. Build and start all services
docker compose up --build
```

Wait until all containers are healthy (30-60 seconds). Verify:
- Dashboard: http://localhost:3001
- World Engine API: http://localhost:8080

---

## Phase 1: Warm Up the Simulation (Before Recording)

Run the simulation for at least 50 ticks with 10 agents to generate real emergent behavior.

```bash
# Advance ticks manually if needed
curl -X POST http://localhost:8080/tick/advance

# Check current tick
curl http://localhost:8080/tick
```

**What to look for before recording**:
- [ ] At least 1 organization formed (check Dashboard → Organizations)
- [ ] At least 1 task completed with reward distribution
- [ ] At least 1 governance proposal (check Dashboard → Governance)
- [ ] Agent skill levels have changed (check individual agent detail)
- [ ] Some agents have formed alliances or rivalries (check agent relations)

If emergent behavior is sparse, run more ticks or try the emergence scenario:

```bash
docker compose -f docker-compose-emergence.yml up --build
```

---

## Phase 2: Recording Setup

### Hardware
- **Display**: External monitor at 1920×1080 or use screen recording at 1080p
- **Audio**: USB mic in a quiet room (or record narration separately)
- **Software**: OBS Studio (free) or ScreenFlow

### OBS Scene Setup
1. **Scene 1 — Terminal**: Full-screen terminal window, Catppuccin Mocha theme, font size 18+
2. **Scene 2 — Browser**: Chrome with dark theme, zoom 125%, Dashboard loaded
3. **Scene 3 — Architecture**: Pre-rendered architecture diagram (from docs/ARCHITECTURE.md)
4. **Scene 4 — End Card**: Pre-designed end card with GitHub URL

### Recording Settings
- Resolution: 1920×1080
- Frame rate: 30fps (60fps preferred)
- Format: MKV (OBS) → convert to MP4 later
- Audio: 48kHz, mono or stereo

---

## Phase 3: Recording Sequence

Take **multiple takes** of each segment. Edit together in post.

### Take 1: Terminal Boot (0:00–0:05)

```
Action: Type `docker compose up --build` in terminal
Timing: Stop when "All services started" appears
Tip: Use a large font. Dark terminal theme.
```

### Take 2: Dashboard Overview (0:05–0:15)

```
Action: 
1. Open http://localhost:3001 in browser
2. Let the world overview load (shows agent count, tokens, GDP)
3. Slowly scroll to show stat cards

Tip: Wait for SSE data to stream in (live updates visible)
```

### Take 3: Agent List + Detail (0:15–0:25)

```
Action:
1. Click "Agents" in sidebar
2. Show the full agent list with filters
3. Filter by organization (e.g., "星辰商会")
4. Click one agent to open detail view
5. Show skills, relations, memories

Tip: Pick an agent with interesting data (high skills, multiple relations)
```

### Take 4: Economy View (0:25–0:35)

```
Action:
1. Navigate to Economy page
2. Show GDP chart, token flow
3. Navigate to Banking page — show accounts, loans
4. Navigate to Stocks page — show order book, price chart

Tip: The charts should show curves, not flat lines (run more ticks if needed)
```

### Take 5: Emergence Dashboard (0:35–0:45)

```
Action:
1. Navigate to Dashboard page (emergence metrics)
2. Show 4 charts: Cultural Diversity, Org Count, Economic Activity, Governance Events
3. Point out the inflection points where behavior changed

Tip: These charts tell the "civilization emerges" story
```

### Take 6: Timeline (0:45–0:55)

```
Action:
1. Navigate to Timeline page
2. Drag the scrubber from tick 0 to 5000
3. Pause at key events (organization formed, first trade, first vote)
4. Click on events to show details

Tip: This is the most visually compelling section — take extra care
```

### Take 7: Sandbox (0:55–1:00)

```
Action:
1. Navigate to Sandbox page
2. Fill in the agent creation form
3. Submit and show the simulated result

Tip: Quick and light — just show it exists
```

---

## Phase 4: Post-Production

### Editing (DaVinci Resolve / Premiere / CapCut)

1. **Cut**: Remove dead time, keep each section tight
2. **Zoom**: Use digital zoom (150-200%) to highlight specific UI elements
3. **Transitions**: Simple cross-dissolves (0.3s). No fancy effects.
4. **Text overlays**: Add the on-screen text from the script (tick numbers, labels)
5. **Music**: Low-volume ambient/tech music (royalty-free from Pixabay or Artlist)
6. **Narration**: Record separately, align with visuals

### Export Settings
- Format: MP4 (H.264)
- Resolution: 1920×1080
- Frame rate: 30fps
- Bitrate: 10-15 Mbps
- Audio: AAC, 192kbps

### YouTube Upload
- Title: "I Built a Survival World for AI Agents — And They Formed a Society"
- Description: Include GitHub link, quick start commands, key timestamps
- Tags: AI agents, artificial life, emergence, simulation, open source, Rust, multi-agent
- Language: English (add Chinese subtitles as CC)

---

## Thumbnail Specifications

| Property | Value |
|----------|-------|
| Resolution | 1280 × 720 px |
| Format | PNG or JPG |
| Max file size | < 2 MB |
| Text | ≤ 10 words, bold, white or yellow |
| Style | Dark background, glowing accents |
| Face/emoji | Optional 🤖 or 🏛️ for visual anchor |

### Recommended Tools
- **Figma** (free tier): Design the thumbnail
- **Canva**: Quick templates for YouTube thumbnails
- **GIMP**: Free alternative

### Color Palette (matching Dashboard)
- Background: `#09090b` (zinc-950)
- Blue: `#3b82f6` (organizations)
- Green: `#22c55e` (trade/economy)
- Purple: `#a855f7` (governance)
- Orange: `#f97316` (culture)
- White text: `#ffffff`
