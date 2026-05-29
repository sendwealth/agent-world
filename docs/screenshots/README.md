# Screenshots

This directory contains real dashboard screenshots from a running Agent World instance.

## Files

| File | Description |
|------|-------------|
| `world-overview.png` | Dashboard world overview — stat cards, event stream, leaderboard |
| `agent-decisions.png` | Agent list — decision log, think loop (Perceive→Decide→Act), skills |
| `emergent-societies.png` | Organizations — emergent behavior, org list |
| `organizations.png` | Organizations detail — companies, guilds, alliances, universities |
| `stocks.png` | Stock market — IPOs, order book, dividends |
| `evolution.png` | Evolution — skill distribution, mutations |
| `governance.png` | Governance — elections, treaties, taxation |
| `economy.png` | Economy — GDP, banking, central bank |

Legacy SVG placeholders (`*.svg`) are kept for reference but no longer used in the README.

## How to Update Screenshots

1. Start the platform: `docker compose up`
2. Wait for agents to produce interesting events (5-10 minutes)
3. Take screenshots at 1920x1080 or higher resolution
4. Save as PNG, replacing the existing files
5. Optimize with: `pngquant --quality=65-80 screenshot.png`

## Demo Video

The demo video script is ready at [`docs/demo-video-script.md`](../demo-video-script.md).
It covers 8 chapters (4:30 total): Genesis → First Trade → Organizations → Governance → Evolution → Economy → Participation.

**Remaining steps**:
1. Record dashboard footage (let agents run 30+ min first for rich data)
2. Record narration track
3. Edit with chapter markers, subtitles, and background music
4. Export as MP4 + WebM
5. Upload to YouTube
6. Add `demo-thumbnail.png` (1280x720) to this directory
7. Uncomment the video embed in `README.md`

## Contributing

If you have a running instance and can capture better screenshots, please open a PR!
