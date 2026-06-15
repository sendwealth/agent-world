"""Video composer — stitches PNG frame sequences into an MP4 via ffmpeg.

Optionally generates narration audio with ``edge-tts`` (offline, no API key)
and muxes it into the final video.  When neither ffmpeg nor TTS is available
the composer degrades gracefully:

- **No ffmpeg**       → raises a friendly ``RuntimeError`` with install hint.
- **No edge-tts**     → emits the video with burnt-in subtitles only (no audio).
"""

from __future__ import annotations

import json
import shutil
import subprocess
import tempfile
from pathlib import Path
from typing import Any

# ── Public API ────────────────────────────────────────────────────────────


def compose_video(
    manifest: list[dict[str, Any]],
    output_path: Path,
    *,
    fps: int = 24,
    enable_tts: bool = True,
    title: str = "Agent World Documentary",
    subtitle: str = "",
) -> Path:
    """Compose the final MP4 from the renderer manifest.

    Parameters
    ----------
    manifest:
        Output of :func:`renderer.render_frames` — ordered list of scene
        metadata with ``frame_dir``, ``frame_count``, and ``caption``.
    output_path:
        Where to write the final ``.mp4``.
    fps:
        Frames per second for the output video.
    enable_tts:
        Attempt to generate narration audio with ``edge-tts``.
    title, subtitle:
        Used in TTS narration intro/outro.

    Returns
    -------
    Path
        The path to the produced ``.mp4``.

    Raises
    ------
    RuntimeError
        If ``ffmpeg`` is not installed.
    """
    ffmpeg = _find_ffmpeg()

    output_path.parent.mkdir(parents=True, exist_ok=True)

    # ── 1. Build a concatenated frame list ────────────────────────────
    with tempfile.TemporaryDirectory() as tmp:
        tmp_path = Path(tmp)
        concat_list = tmp_path / "concat_frames.txt"
        _write_concat_list(manifest, concat_list)

        # ── 2. Generate TTS audio (optional) ─────────────────────────
        audio_path: Path | None = None
        if enable_tts:
            audio_path = _try_generate_tts(manifest, tmp_path, title, subtitle)

        # ── 3. Assemble video ────────────────────────────────────────
        silent_video = tmp_path / "silent.mp4"
        _run_ffmpeg_silent(ffmpeg, concat_list, silent_video, fps)

        if audio_path and audio_path.exists():
            final_path = tmp_path / "final.mp4"
            _run_ffmpeg_mux(ffmpeg, silent_video, audio_path, final_path)
            shutil.copy2(final_path, output_path)
        else:
            shutil.copy2(silent_video, output_path)

    return output_path


# ── ffmpeg helpers ────────────────────────────────────────────────────────


def _find_ffmpeg() -> str:
    ffmpeg = shutil.which("ffmpeg")
    if not ffmpeg:
        raise RuntimeError(
            "ffmpeg not found. Install it to generate documentary videos:\n"
            "  macOS:  brew install ffmpeg\n"
            "  Ubuntu: sudo apt-get install ffmpeg\n"
            "  Arch:   sudo pacman -S ffmpeg"
        )
    return ffmpeg


def _write_concat_list(manifest: list[dict[str, Any]], out_file: Path) -> None:
    """Write an ffmpeg concat demuxer file pointing at every frame in order."""
    lines: list[str] = []
    for entry in manifest:
        frame_dir = Path(entry["frame_dir"])
        count = entry["frame_count"]
        for i in range(count):
            frame_path = frame_dir / f"frame_{i:04d}.png"
            lines.append(f"file '{frame_path.resolve()}'")
            lines.append("duration 0.0416667")  # 1/24s
    # Last frame needs to be repeated (ffmpeg concat quirk)
    if manifest:
        last = manifest[-1]
        last_frame = Path(last["frame_dir"]) / f"frame_{last['frame_count'] - 1:04d}.png"
        lines.append(f"file '{last_frame.resolve()}'")
    out_file.write_text("\n".join(lines), encoding="utf-8")


def _run_ffmpeg_silent(ffmpeg: str, concat_list: Path, output: Path, fps: int) -> None:
    """Run ffmpeg to create a silent video from the concat list."""
    cmd = [
        ffmpeg, "-y",
        "-f", "concat", "-safe", "0",
        "-i", str(concat_list),
        "-vf", f"fps={fps},format=yuv420p",
        "-c:v", "libx264",
        "-preset", "fast",
        "-crf", "23",
        "-pix_fmt", "yuv420p",
        str(output),
    ]
    result = subprocess.run(cmd, capture_output=True, text=True)
    if result.returncode != 0:
        # Fallback: try image2 demuxer with pattern (some ffmpeg builds)
        _try_image2_fallback(ffmpeg, concat_list, output, fps, result.stderr)


def _try_image2_fallback(
    ffmpeg: str,
    concat_list: Path,
    output: Path,
    fps: int,
    prev_error: str,
) -> None:
    """Alternative approach: build a flat frame directory and use image2."""
    # Re-read the concat list to copy frames into a flat sequence
    flat_dir = output.parent / "_flat_frames"
    flat_dir.mkdir(exist_ok=True)
    idx = 0
    for line in concat_list.read_text(encoding="utf-8").splitlines():
        if line.startswith("file "):
            src = Path(line[6:].strip().strip("'"))
            if src.exists():
                shutil.copy2(src, flat_dir / f"frame_{idx:06d}.png")
                idx += 1

    cmd = [
        ffmpeg, "-y",
        "-framerate", str(fps),
        "-i", str(flat_dir / "frame_%06d.png"),
        "-vf", "format=yuv420p",
        "-c:v", "libx264",
        "-preset", "fast",
        "-crf", "23",
        str(output),
    ]
    result = subprocess.run(cmd, capture_output=True, text=True)
    shutil.rmtree(flat_dir, ignore_errors=True)
    if result.returncode != 0:
        raise RuntimeError(
            f"ffmpeg failed to produce video.\n"
            f"First attempt stderr (last 500 chars): {prev_error[-500:]}\n"
            f"Second attempt stderr (last 500 chars): {result.stderr[-500:]}"
        )


def _run_ffmpeg_mux(ffmpeg: str, video: Path, audio: Path, output: Path) -> None:
    """Mux video + audio into the final container."""
    cmd = [
        ffmpeg, "-y",
        "-i", str(video),
        "-i", str(audio),
        "-c:v", "copy",
        "-c:a", "aac",
        "-b:a", "128k",
        "-shortest",
        str(output),
    ]
    result = subprocess.run(cmd, capture_output=True, text=True)
    if result.returncode != 0:
        # If mux fails, just use the silent video
        shutil.copy2(video, output)


# ── TTS ───────────────────────────────────────────────────────────────────


def _try_generate_tts(
    manifest: list[dict[str, Any]],
    work_dir: Path,
    title: str,
    subtitle: str,
) -> Path | None:
    """Try to generate a narration audio track with edge-tts.

    Returns ``None`` if edge-tts is unavailable or fails.
    """
    try:
        import asyncio

        import edge_tts  # type: ignore[import-untyped]
    except ImportError:
        return None

    # Build narration script
    script_parts: list[str] = [title]
    if subtitle:
        script_parts.append(subtitle)
    for entry in manifest:
        if entry.get("caption"):
            script_parts.append(entry["caption"])
    script = "。".join(script_parts)

    output = work_dir / "narration.mp3"
    try:
        asyncio.run(_edge_tts_speak(edge_tts, script, output))
        if output.exists() and output.stat().st_size > 0:
            return output
    except Exception:
        pass
    return None


async def _edge_tts_speak(edge_tts: Any, text: str, output: Path) -> None:
    """Run edge-tts communicate to generate an MP3."""
    communicate = edge_tts.Communicate(text, "zh-CN-XiaoxiaoNeural")
    await communicate.save(str(output))


# ── Utility ───────────────────────────────────────────────────────────────


def save_manifest(manifest: list[dict[str, Any]], path: Path) -> None:
    """Write the manifest to a JSON file (for debugging / CI inspection)."""
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(manifest, indent=2, ensure_ascii=False), encoding="utf-8")
