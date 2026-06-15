# Agent World — Auto-Documentary Guide (Phase 5.4)

## Overview

The **Auto-Documentary** generator turns world-simulation timeline data into a
narrated "world history" video — automatically.  It collects milestone events,
economic metrics, and social-network snapshots, organises them into thematic
chapters, renders animated visualisations, and composes an MP4 with optional
Chinese narration.

This is the most *shareable* artefact of the Agent World project: a
self-contained video that demonstrates emergent civilisation without requiring
viewers to install anything.

---

## Architecture

```
collector → narrator → renderer → composer → MP4
 (data)     (story)    (frames)    (video)
```

| Module | File | Responsibility |
|--------|------|----------------|
| Collector | `agent_runtime/documentary/collector.py` | Pull timeline events, metrics, network from synthetic/sample/live sources |
| Narrator | `agent_runtime/documentary/narrator.py` | Group events into chapters, generate narration text |
| Renderer | `agent_runtime/documentary/renderer.py` | Render animated PNG frames with matplotlib |
| Composer | `agent_runtime/documentary/composer.py` | Stitch frames + TTS audio into MP4 with ffmpeg |
| CLI | `scripts/generate_documentary.py` | One-command entry point |

---

## Prerequisites

### System dependencies

- **Python ≥ 3.11** with the agent-runtime package installed.
- **ffmpeg** — required for video assembly.
  ```bash
  # macOS
  brew install ffmpeg
  # Ubuntu / Debian
  sudo apt-get install ffmpeg
  # Arch
  sudo pacman -S ffmpeg
  ```

### Python dependencies

```bash
cd agent-runtime
uv pip install -e ".[dev]"

# Optional: for animated social-network graphs
pip install networkx

# Optional: for Chinese TTS narration (offline, no API key)
pip install edge-tts
```

Without `networkx`, network scenes fall back to title cards.
Without `edge-tts`, the video is produced with burnt-in subtitles only (no audio).

---

## Quick Start

### Synthetic mode (CI-safe, no external services)

```bash
# Using Makefile
make documentary

# Or directly
python scripts/generate_documentary.py --mode synthetic
```

This produces:
- `reports/documentary/world-history-<timestamp>.mp4` — the video
- `reports/documentary/world-history-<timestamp>.json` — storyboard manifest
- `reports/documentary/frames-<timestamp>/` — PNG frame sequences

### Sample mode (dashboard data)

```bash
make documentary-sample
# or
python scripts/generate_documentary.py --mode sample
```

Reads the JSON files in `dashboard/public/data/` (timeline-events,
world-snapshots, interaction-network) and generates the documentary from them.

### Live mode (running engine)

```bash
# Start the world-engine first
make dev

# Then generate
make documentary-live
# or
ENGINE_URL=http://localhost:8080 python scripts/generate_documentary.py --mode live
```

---

## CLI Options

```text
--mode {synthetic,sample,live}   Data source (default: synthetic)
--engine-url URL                 World-engine URL (live mode only)
--output DIR                     Output directory (default: reports/documentary/)
--title TEXT                     Documentary title
--subtitle TEXT                  Documentary subtitle
--no-tts                         Disable TTS narration (subtitles only)
--fps N                          Output FPS (default: 24)
--scene-duration SECONDS         Duration per scene (default: 4.0)
--seed N                         RNG seed for synthetic mode (default: 42)
--agents N                       Agent population for synthetic (default: 50)
--ticks N                        Total ticks for synthetic (default: 5000)
```

---

## Chapters

The documentary always contains at least **four chapters**:

1. **文明的诞生** (Civilisation Birth) — milestones, population growth curve
2. **经济的萌芽** (Economy Emergence) — GDP evolution + Gini coefficient charts
3. **社会组织与治理** (Society & Governance) — social-network graph animation
4. **文化繁荣与总结** (Culture & Conclusion) — full-timeline population review

Each chapter includes a title card scene and at least one data-visualisation
scene with animated charts.

---

## TTS Narration

Narration uses **edge-tts** (Microsoft Edge's online TTS service) with the
`zh-CN-XiaoxiaoNeural` voice.  No API key is required.

To disable TTS (e.g. for CI):

```bash
python scripts/generate_documentary.py --mode synthetic --no-tts
```

When TTS is disabled or unavailable, captions are still burnt into each frame
as subtitles, so the video is always self-contained.

---

## CI Integration

The synthetic mode is designed for CI:

- No network access required (data is generated deterministically).
- No API keys or external services.
- ffmpeg is the only system dependency.
- Deterministic output with `--seed`.

Example GitHub Actions step:

```yaml
- name: Generate documentary
  run: |
    sudo apt-get install -y ffmpeg
    pip install matplotlib networkx
    python scripts/generate_documentary.py --mode synthetic --no-tts
```

---

## Customisation

### Change the chapter structure

Edit `agent_runtime/documentary/narrator.py`:

- `_CHAPTER_TITLES` — chapter names.
- `_TYPE_TO_CHAPTER` — which event types map to which chapter.
- `_scenes_for_chapter()` — which visualisation scenes appear in each chapter.

### Change the visual style

Edit `agent_runtime/documentary/renderer.py`:

- `BG_COLOUR`, `TEXT_COLOUR`, `ACCENT_COLOUR` — colour palette.
- `FIG_W`, `FIG_H`, `DPI` — output resolution.
- `_CJK_FONT_CANDIDATES` — font fallback list for Chinese text.

### Add a new visualisation type

1. Add a new `scene_type` string in the narrator's `_scenes_for_chapter()`.
2. Implement a `_render_<type>()` function in `renderer.py`.
3. Register it in the `renderer_map` dict inside `_render_scene()`.
