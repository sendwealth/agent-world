"""Auto-Documentary — world history video generator (Phase 5.4).

Turns world-simulation timeline data into a narrated "world history" video.

Pipeline::

    collector  →  narrator  →  renderer  →  composer
    (data)       (story)      (frames)     (mp4)

Public entry point: :func:`generate_documentary`.
"""

from __future__ import annotations

from .collector import DocumentaryData, collect
from .composer import compose_video
from .narrator import Chapter, Storyboard, build_storyboard
from .renderer import render_frames

__all__ = [
    "Chapter",
    "DocumentaryData",
    "Storyboard",
    "build_storyboard",
    "collect",
    "compose_video",
    "render_frames",
]
