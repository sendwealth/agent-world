# Screenshots

This directory contains dashboard preview illustrations used in the project README.

## Files

| File | Description |
|------|-------------|
| `world-overview.svg` | Dashboard world overview — stat cards, event stream, leaderboard |
| `agent-decisions.svg` | Agent detail — decision log, think loop (Perceive→Decide→Act), skills |
| `emergent-societies.svg` | Organizations & stock market — emergent behavior |

These are **illustrated SVG previews** (dark-theme mockups). They should be replaced with real screenshots when a running instance is available.

## How to Replace with Real Screenshots

1. Start the platform: `docker compose up`
2. Wait for agents to produce interesting events (5-10 minutes)
3. Take screenshots at 1280×720 or higher resolution
4. Save as PNG or update the SVG paths in README
5. Optimize with: `pngquant --quality=65-80 screenshot.png`

## Demo Video Thumbnail

The demo video embed (`demo-thumbnail.png`) is pending completion of the demo video.
When ready, uncomment the video section in README and add the thumbnail.

## Contributing

If you have a running instance and can capture good screenshots, please open a PR!
