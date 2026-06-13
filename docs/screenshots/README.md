# Screenshots

This directory contains real dashboard screenshots from a running Agent World instance.

## Automated Capture (recommended)

A Playwright-based tool lives in `scripts/screenshots/` and automates the
entire capture workflow at 1920×1080.

### Prerequisites

1. The **world engine + dashboard** must be running.
   - Local dev: `make run-dashboard` (defaults to `http://localhost:3000`)
   - Docker Compose: `make dev` (dashboard on `http://localhost:3001`)
2. **Playwright + Chromium** installed (first time only):
   ```sh
   make screenshots-install
   ```

### Usage

```sh
# Capture all routes (dashboard on http://localhost:3000)
make screenshots

# If dashboard is on a different port (e.g. Docker Compose)
DASHBOARD_URL=http://localhost:3001 make screenshots

# Capture only specific routes
make screenshots SCREENSHOTS_ARGS="--only world-overview,agents,economy"

# Clean existing PNGs before capturing
make screenshots SCREENSHOTS_ARGS="--clean"
```

Or run the script directly:

```sh
cd scripts/screenshots
DASHBOARD_URL=http://localhost:3000 node capture.mjs --out ../../docs/screenshots
```

### Output files

The tool saves numbered PNGs matching the README's expected filenames:

| File | Route | Description |
|------|-------|-------------|
| `01-world-overview.png` | `/` | World overview — stat cards, event stream, leaderboard |
| `02-agents.png` | `/agents` | Agent list — status, tokens, skills |
| `03-agent-detail.png` | `/agents/:id` | Agent detail — decision log, skill tree |
| `04-organizations.png` | `/organizations` | Organizations — companies, guilds, alliances |
| `05-stocks.png` | `/stocks` | Stock market — IPOs, order book |
| `06-evolution.png` | `/evolution` | Evolution — skill distribution, mutations |
| `07-economy.png` | `/economy` | Economy — GDP, banking |
| `08-governance.png` | `/governance` | Governance — elections, treaties |
| `09-timeline.png` | `/timeline` | Timeline — world events chronology |
| `10-tasks.png` | `/tasks` | Task board — bounties, claims, reviews |
| `11-traces.png` | `/traces` | Traces — agent reasoning traces |
| `12-marketplace.png` | `/marketplace` | Marketplace — items, trade offers |
| `13-feed.png` | `/feed` | Event feed — live SSE stream |
| `14-briefing.png` | `/briefing` | Daily briefing — world summary |
| `15-diplomacy.png` | `/diplomacy` | Diplomacy — relations, treaties |

## Manual Capture (legacy)

If you prefer to take screenshots by hand:

1. Start the platform: `docker compose up`
2. Wait for agents to produce interesting events (5–10 minutes)
3. Take screenshots at 1920×1080 or higher resolution
4. Save as PNG, replacing the existing files
5. Optimize with: `pngquant --quality=65-80 screenshot.png`

## Legacy SVG Placeholders

The `*.svg` files are kept for reference but are no longer used in the README.
They will be removed in a future release once all screenshots are regenerated
via the automation tool.

## Demo Video

The demo video script is ready at [`docs/demo-video-script.md`](../demo-video-script.md).
It covers 8 chapters (4:30 total): Genesis → First Trade → Organizations → Governance → Evolution → Economy → Participation.

**Remaining steps**:
1. Record dashboard footage (let agents run 30+ min first for rich data)
2. Record narration track
3. Edit with chapter markers, subtitles, and background music
4. Export as MP4 + WebM
5. Upload to YouTube
6. Add `demo-thumbnail.png` (1280×720) to this directory
7. Uncomment the video embed in `README.md`

## Contributing

If you have a running instance and can capture better screenshots, please open a PR!
