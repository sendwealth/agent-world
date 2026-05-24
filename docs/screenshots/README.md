# Screenshots

This directory contains dashboard screenshots used in the project README.

## Required Files

| File | Description | Suggested Source |
|------|-------------|------------------|
| `world-overview.png` | Dashboard world overview page showing stat cards (agent count, GDP, tick), event stream, and leaderboard | `http://localhost:3001` main page |
| `agent-decisions.png` | Agent detail page showing decision log, think loop (Perceive→Decide→Act), skill tree | `http://localhost:3001/agents/[id]` |
| `emergent-societies.png` | Organizations page or economy page showing emergent behavior (orgs formed, stock trades, events) | `http://localhost:3001/organizations` or `/economy` |
| `demo-thumbnail.png` | Thumbnail image for demo video embed in README | Video frame capture |

## How to Capture

1. Start the platform: `docker compose up`
2. Wait for agents to produce interesting events (5-10 minutes)
3. Take screenshots at 1280×720 or higher resolution
4. Optimize with: `pngquant --quality=65-80 screenshot.png`

## Contributing

These are placeholder paths. If you have a running instance and can capture good screenshots, please open a PR!
