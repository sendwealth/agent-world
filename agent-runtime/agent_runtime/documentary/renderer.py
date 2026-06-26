"""Visualisation renderer — produces PNG frame sequences from a storyboard.

Each *scene* in the storyboard is rendered into a directory of numbered PNG
frames (``frame_0000.png``, ``frame_0001.png``, …).  The composer later
stitches these directories together into the final video.

Visualisation types:
- ``title_card``  — full-screen chapter title with caption.
- ``population``  — animated line chart of population over ticks.
- ``gdp``         — animated line chart of GDP.
- ``gini``        — animated line chart of Gini coefficient.
- ``network``     — animated social-network graph (NetworkX spring layout).

The renderer uses the non-interactive ``Agg`` backend so it works in CI
without a display.
"""

from __future__ import annotations

import math
from pathlib import Path
from typing import Any

import matplotlib

matplotlib.use("Agg")  # noqa: E402 — must be set before pyplot import
import matplotlib.font_manager as fm  # noqa: E402
import matplotlib.pyplot as plt  # noqa: E402

from .collector import DocumentaryData  # noqa: E402
from .narrator import Scene, Storyboard  # noqa: E402

# Optional networkx import — renderer still works without it (skips network scenes).
try:
    import networkx as nx  # type: ignore[import-untyped]
    _HAS_NETWORKX = True
except ImportError:
    _HAS_NETWORKX = False


# ── Constants ─────────────────────────────────────────────────────────────

FIG_W, FIG_H = 16, 9  # 16:9 aspect ratio
DPI = 100

# Colour palette
BG_COLOUR = "#0f172a"
TEXT_COLOUR = "#e2e8f0"
ACCENT_COLOUR = "#38bdf8"
ACCENT_COLOUR_2 = "#f472b6"
ACCENT_COLOUR_3 = "#34d399"
GRID_COLOUR = "#1e293b"

# CJK font candidates (tried in order)
_CJK_FONT_CANDIDATES = [
    "PingFang SC",
    "Heiti SC",
    "Hiragino Sans GB",
    "STHeiti",
    "Microsoft YaHei",
    "WenQuanYi Micro Hei",
    "Noto Sans CJK SC",
    "SimHei",
    "Arial Unicode MS",
    "DejaVu Sans",
]


def _resolve_cjk_font() -> str:
    """Find a font available on this system that can render CJK glyphs."""
    available = {f.name for f in fm.fontManager.ttflist}
    for candidate in _CJK_FONT_CANDIDATES:
        if candidate in available:
            return candidate
    # Fallback: let matplotlib pick whatever it has
    return "sans-serif"


_CJK_FONT: str = _resolve_cjk_font()


# ── Public API ────────────────────────────────────────────────────────────


def render_frames(
    storyboard: Storyboard,
    data: DocumentaryData,
    output_dir: Path,
) -> list[dict[str, Any]]:
    """Render every scene in ``storyboard`` into ``output_dir``.

    Returns a manifest: ``[{chapter, scene_index, scene_type, frame_dir,
    frame_count, caption}, ...]`` — the composer uses this to know the
    ordering and narration text.
    """
    output_dir.mkdir(parents=True, exist_ok=True)
    manifest: list[dict[str, Any]] = []

    for ch in storyboard.chapters:
        for si, scene in enumerate(ch.scenes):
            frame_dir = output_dir / f"ch{ch.chapter_num}_s{si}_{scene.scene_type}"
            frame_dir.mkdir(parents=True, exist_ok=True)
            frame_count = _render_scene(scene, data, frame_dir, storyboard)
            manifest.append(
                {
                    "chapter": ch.chapter_num,
                    "chapter_title": ch.title,
                    "scene_index": si,
                    "scene_type": scene.scene_type,
                    "frame_dir": str(frame_dir),
                    "frame_count": frame_count,
                    "caption": scene.caption,
                    "title": scene.title,
                    "subtitle": scene.subtitle,
                }
            )

    return manifest


# ── Scene dispatch ────────────────────────────────────────────────────────


def _render_scene(
    scene: Scene,
    data: DocumentaryData,
    frame_dir: Path,
    storyboard: Storyboard,
) -> int:
    """Dispatch to the correct renderer and return the number of frames."""
    renderer_map = {
        "title_card": _render_title_card,
        "population": _render_population,
        "gdp": _render_gdp,
        "gini": _render_gini,
        "network": _render_network,
    }
    renderer = renderer_map.get(scene.scene_type, _render_title_card)
    return renderer(scene, data, frame_dir, storyboard)


# ── Helpers ───────────────────────────────────────────────────────────────


def _new_figure() -> tuple[plt.Figure, plt.Axes]:
    fig, ax = plt.subplots(figsize=(FIG_W, FIG_H), dpi=DPI)
    fig.patch.set_facecolor(BG_COLOUR)
    ax.set_facecolor(BG_COLOUR)
    ax.tick_params(colors=TEXT_COLOUR)
    for spine in ax.spines.values():
        spine.set_color(GRID_COLOUR)
    ax.xaxis.label.set_color(TEXT_COLOUR)
    ax.yaxis.label.set_color(TEXT_COLOUR)
    ax.title.set_color(TEXT_COLOUR)
    return fig, ax


def _style_grid(ax: plt.Axes) -> None:
    ax.grid(True, color=GRID_COLOUR, linewidth=0.5, alpha=0.7)


def _save_frame(fig: plt.Figure, frame_dir: Path, idx: int) -> None:
    path = frame_dir / f"frame_{idx:04d}.png"
    fig.savefig(path, facecolor=fig.get_facecolor(), pad_inches=0)
    plt.close(fig)


def _wrap_caption(text: str, width: int = 40) -> str:
    """Wrap text for display.  Splits on characters for CJK support."""
    if len(text) <= width:
        return text
    lines: list[str] = []
    current = ""
    for char in text:
        current += char
        if len(current) >= width and char in "，。；！？、 ":
            lines.append(current)
            current = ""
    if current:
        lines.append(current)
    return "\n".join(lines)


def _draw_caption_bar(fig: plt.Figure, caption: str) -> None:
    """Draw a semi-transparent caption bar at the bottom with narration text."""
    if not caption:
        return
    wrapped = _wrap_caption(caption, width=48)
    fig.text(
        0.5,
        0.04,
        wrapped,
        ha="center",
        va="bottom",
        fontsize=16,
        color=TEXT_COLOUR,
        fontfamily=_CJK_FONT,
        bbox=dict(boxstyle="round,pad=0.5", facecolor="#1e293b", alpha=0.85),
    )


# ── Title card ────────────────────────────────────────────────────────────


def _render_title_card(
    scene: Scene,
    data: DocumentaryData,
    frame_dir: Path,
    storyboard: Storyboard,
) -> int:
    """Render a full-screen title card.  Simple fade-in animation."""
    n_frames = max(1, int(storyboard.scene_duration_seconds * storyboard.fps))
    for i in range(n_frames):
        fig = plt.figure(figsize=(FIG_W, FIG_H), dpi=DPI)
        fig.patch.set_facecolor(BG_COLOUR)
        ax = fig.add_axes((0, 0, 1, 1))
        ax.set_facecolor(BG_COLOUR)
        ax.set_xlim(0, 1)
        ax.set_ylim(0, 1)
        ax.axis("off")

        alpha = min(1.0, (i + 1) / (n_frames * 0.4))  # fade in over first 40%

        ax.text(
            0.5,
            0.6,
            scene.title,
            ha="center",
            va="center",
            fontsize=52,
            fontweight="bold",
            color=TEXT_COLOUR,
            fontfamily=_CJK_FONT,
            alpha=alpha,
        )
        if scene.subtitle:
            ax.text(
                0.5,
                0.48,
                scene.subtitle,
                ha="center",
                va="center",
                fontsize=24,
                color=ACCENT_COLOUR,
                fontfamily=_CJK_FONT,
                alpha=alpha,
            )
        _draw_caption_bar(fig, scene.caption)
        _save_frame(fig, frame_dir, i)
    return n_frames


# ── Line-chart scenes (population, gdp, gini) ─────────────────────────────


def _render_line_chart(
    scene: Scene,
    data: DocumentaryData,
    frame_dir: Path,
    storyboard: Storyboard,
    *,
    extractor: Any,
    ylabel: str,
    colour: str,
) -> int:
    """Generic animated line-chart renderer.

    ``extractor`` is a callable ``(MetricSnapshot) -> float``.
    """
    ticks = [m.tick for m in data.metrics]
    values = [extractor(m) for m in data.metrics]
    if not ticks:
        ticks = [0]
        values = [0.0]

    n_frames = max(1, int(storyboard.scene_duration_seconds * storyboard.fps))
    total = len(ticks)

    for i in range(n_frames):
        progress = (i + 1) / n_frames
        show_count = max(2, int(total * progress))
        show_count = min(show_count, total)

        fig, ax = _new_figure()
        _style_grid(ax)

        x = ticks[:show_count]
        y = values[:show_count]

        ax.plot(x, y, color=colour, linewidth=2.5, zorder=3)
        ax.fill_between(x, y, min(y) if y else 0, alpha=0.15, color=colour)

        # Highlight current point
        ax.scatter(
            [x[-1]], [y[-1]],
            color=colour, s=80, zorder=5,
            edgecolors="white", linewidths=1.5,
        )

        ax.set_xlabel("Tick", fontsize=14, fontfamily=_CJK_FONT)
        ax.set_ylabel(ylabel, fontsize=14, fontfamily=_CJK_FONT)
        ax.set_title(f"{scene.title} — {scene.subtitle}", fontsize=20, fontfamily=_CJK_FONT, pad=15)

        _draw_caption_bar(fig, scene.caption)
        _save_frame(fig, frame_dir, i)

    return n_frames


def _render_population(
    scene: Scene, data: DocumentaryData, frame_dir: Path, storyboard: Storyboard
) -> int:
    return _render_line_chart(
        scene, data, frame_dir, storyboard,
        extractor=lambda m: m.population,
        ylabel="人口 (Population)",
        colour=ACCENT_COLOUR,
    )


def _render_gdp(
    scene: Scene, data: DocumentaryData, frame_dir: Path, storyboard: Storyboard
) -> int:
    return _render_line_chart(
        scene, data, frame_dir, storyboard,
        extractor=lambda m: m.gdp,
        ylabel="GDP",
        colour=ACCENT_COLOUR_3,
    )


def _render_gini(
    scene: Scene, data: DocumentaryData, frame_dir: Path, storyboard: Storyboard
) -> int:
    return _render_line_chart(
        scene, data, frame_dir, storyboard,
        extractor=lambda m: m.gini,
        ylabel="Gini 系数",
        colour=ACCENT_COLOUR_2,
    )


# ── Network scene ─────────────────────────────────────────────────────────


def _render_network(
    scene: Scene, data: DocumentaryData, frame_dir: Path, storyboard: Storyboard
) -> int:
    """Render an animated social-network graph.

    Nodes fade in progressively; edges appear over time to give an
    organic 'growing' feel.
    """
    if not _HAS_NETWORKX:
        # Fallback to a title-card style scene explaining the network
        scene.caption = scene.caption or "社交网络可视化需要 networkx 依赖。"
        return _render_title_card(scene, data, frame_dir, storyboard)

    nodes = data.network.nodes
    edges = data.network.edges
    n_total = len(nodes)

    if n_total == 0:
        scene.caption = "无社交网络数据。"
        return _render_title_card(scene, data, frame_dir, storyboard)

    graph = nx.Graph()
    for n in nodes:
        graph.add_node(n["id"], name=n.get("name", n["id"]), group=n.get("group", 0))
    for e in edges:
        src, tgt = e["source"], e["target"]
        if graph.has_node(src) and graph.has_node(tgt):
            graph.add_edge(src, tgt, weight=e.get("weight", 1))

    pos = nx.spring_layout(graph, seed=42, k=1.5 / math.sqrt(max(1, n_total)))

    groups = sorted({d.get("group", 0) for _, d in graph.nodes(data=True)})
    cmap = plt.cm.get_cmap("tab10", max(len(groups), 1))
    group_colour = {g: cmap(i) for i, g in enumerate(groups)}

    n_frames = max(1, int(storyboard.scene_duration_seconds * storyboard.fps))
    node_list = list(graph.nodes())
    edge_list = list(graph.edges())

    for i in range(n_frames):
        progress = (i + 1) / n_frames
        show_nodes = max(1, int(n_total * progress))
        show_edges = int(len(edge_list) * progress)

        visible_nodes = node_list[:show_nodes]
        visible_edges = edge_list[:show_edges]

        fig, ax = _new_figure()
        ax.set_xlim(-0.05, 1.05)
        ax.set_ylim(-0.05, 1.05)
        ax.axis("off")
        ax.set_title(f"{scene.title} — {scene.subtitle}", fontsize=20, fontfamily=_CJK_FONT, pad=15)

        if visible_edges:
            nx.draw_networkx_edges(
                graph, pos, ax=ax, edgelist=visible_edges,
                edge_color=GRID_COLOUR, alpha=0.4, width=0.8,
            )
        node_colours = [
            group_colour[graph.nodes[n].get("group", 0)] for n in visible_nodes
        ]
        nx.draw_networkx_nodes(
            graph, pos, ax=ax, nodelist=visible_nodes,
            node_color=node_colours, node_size=50, alpha=0.9,
            edgecolors="white", linewidths=0.3,
        )

        info_text = f"节点: {show_nodes}/{n_total}  边: {show_edges}/{len(edge_list)}"
        ax.text(
            0.02, 0.97, info_text,
            transform=ax.transAxes, fontsize=12, color=TEXT_COLOUR,
            fontfamily=_CJK_FONT, va="top",
        )

        _draw_caption_bar(fig, scene.caption)
        _save_frame(fig, frame_dir, i)

    return n_frames
