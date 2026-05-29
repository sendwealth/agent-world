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

## Demo Video Thumbnail

The demo video embed (`demo-thumbnail.png`) is pending completion of the demo video.
When ready, uncomment the video section in README and add the thumbnail.

## Contributing

If you have a running instance and can capture better screenshots, please open a PR!
