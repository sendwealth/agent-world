#!/usr/bin/env python3
"""Generate a world-history documentary video (Phase 5.4).

Pipeline:  collector → narrator → renderer → composer → MP4

Usage
-----
Offline / synthetic (CI-safe, default)::

    python scripts/generate_documentary.py --mode synthetic

From dashboard sample data::

    python scripts/generate_documentary.py --mode sample

Live engine::

    python scripts/generate_documentary.py --mode live --engine-url http://localhost:8080

Outputs
-------
- ``reports/documentary/world-history-<timestamp>.mp4``  — the video
- ``reports/documentary/world-history-<timestamp>.json`` — storyboard manifest
"""

from __future__ import annotations

import argparse
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

# Ensure agent_runtime is importable when running from the project root.
PROJECT_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(PROJECT_ROOT / "agent-runtime"))

from agent_runtime.documentary import (  # noqa: E402
    build_storyboard,
    collect,
    compose_video,
    render_frames,
)
from agent_runtime.documentary.composer import save_manifest  # noqa: E402

REPORT_DIR = PROJECT_ROOT / "reports" / "documentary"


def _parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    p = argparse.ArgumentParser(
        prog="generate_documentary",
        description="Generate a narrated world-history documentary video (Phase 5.4).",
    )
    p.add_argument(
        "--mode",
        choices=["synthetic", "sample", "live"],
        default="synthetic",
        help="Data source: synthetic (default), sample (dashboard JSON), or live engine.",
    )
    p.add_argument(
        "--engine-url",
        default="http://localhost:8080",
        help="World-engine URL (live mode only).",
    )
    p.add_argument(
        "--output",
        default=str(REPORT_DIR),
        help="Output directory for the MP4 and manifest.",
    )
    p.add_argument(
        "--title",
        default="Agent World — 世界历史纪录片",
        help="Documentary title.",
    )
    p.add_argument(
        "--subtitle",
        default="自动生成 · Phase 5.4",
        help="Documentary subtitle.",
    )
    p.add_argument(
        "--no-tts",
        action="store_true",
        help="Disable TTS narration (subtitles only).",
    )
    p.add_argument(
        "--fps",
        type=int,
        default=24,
        help="Output frames per second.",
    )
    p.add_argument(
        "--scene-duration",
        type=float,
        default=4.0,
        help="Duration of each scene in seconds.",
    )
    p.add_argument(
        "--seed",
        type=int,
        default=42,
        help="RNG seed for synthetic mode.",
    )
    p.add_argument(
        "--agents",
        type=int,
        default=50,
        help="Agent population for synthetic mode.",
    )
    p.add_argument(
        "--ticks",
        type=int,
        default=5000,
        help="Total ticks for synthetic mode.",
    )
    return p.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = _parse_args(argv)

    out_dir = Path(args.output)
    out_dir.mkdir(parents=True, exist_ok=True)
    ts = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    video_path = out_dir / f"world-history-{ts}.mp4"
    manifest_path = out_dir / f"world-history-{ts}.json"

    print(f"[documentary] mode={args.mode}", file=sys.stderr)

    # ── 1. Collect ──────────────────────────────────────────────────
    t0 = time.monotonic()
    data = collect(
        args.mode,
        engine_url=args.engine_url,
        seed=args.seed,
        n_agents=args.agents,
        total_ticks=args.ticks,
    )
    print(
        f"[documentary] collected {len(data.events)} events, "
        f"{len(data.metrics)} metric snapshots ({time.monotonic() - t0:.2f}s)",
        file=sys.stderr,
    )

    # ── 2. Narrate ──────────────────────────────────────────────────
    storyboard = build_storyboard(
        data,
        title=args.title,
        subtitle=args.subtitle,
        fps=args.fps,
        scene_duration=args.scene_duration,
    )
    total_scenes = sum(len(c.scenes) for c in storyboard.chapters)
    print(
        f"[documentary] storyboard: {len(storyboard.chapters)} chapters, "
        f"{total_scenes} scenes, ~{storyboard.estimated_duration_seconds:.0f}s",
        file=sys.stderr,
    )

    # ── 3. Render frames ────────────────────────────────────────────
    t1 = time.monotonic()
    frames_dir = out_dir / f"frames-{ts}"
    manifest = render_frames(storyboard, data, frames_dir)
    total_frames = sum(e["frame_count"] for e in manifest)
    print(
        f"[documentary] rendered {total_frames} frames in {time.monotonic() - t1:.2f}s",
        file=sys.stderr,
    )

    # ── 4. Compose video ────────────────────────────────────────────
    t2 = time.monotonic()
    try:
        compose_video(
            manifest,
            video_path,
            fps=args.fps,
            enable_tts=not args.no_tts,
            title=args.title,
            subtitle=args.subtitle,
        )
    except RuntimeError as e:
        print(f"[documentary] ERROR: {e}", file=sys.stderr)
        # Still save the manifest for debugging
        save_manifest(manifest, manifest_path)
        return 2
    print(
        f"[documentary] composed video in {time.monotonic() - t2:.2f}s",
        file=sys.stderr,
    )

    # ── 5. Save manifest ────────────────────────────────────────────
    save_manifest(manifest, manifest_path)

    # ── 6. Cleanup frames (optional — keep for debugging if small) ──
    # We keep the frames directory; user can delete manually or via make clean.

    print(f"[documentary] video:  {video_path}", file=sys.stderr)
    print(f"[documentary] manifest: {manifest_path}", file=sys.stderr)
    return 0


if __name__ == "__main__":  # pragma: no cover
    raise SystemExit(main())
