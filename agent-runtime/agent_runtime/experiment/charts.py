"""Chart generation for experiment reports using matplotlib.

Generates publication-quality charts from experiment data:
- Population trend (line chart)
- GDP trajectory (area chart)
- Gini coefficient over time (line chart with threshold bands)
- Skill distribution (horizontal bar chart)
- Survival rate pie chart
- Economic overview (multi-metric dashboard)

All charts are rendered as PNG bytes for embedding in HTML/PDF reports,
or saved to disk as standalone files.
"""

from __future__ import annotations

import base64
import io
import logging
from typing import Any

logger = logging.getLogger(__name__)

# Use Agg backend for headless rendering (no display required)
try:
    import matplotlib
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt
    import matplotlib.ticker as mticker
    _HAS_MPL = True
except ImportError:
    _HAS_MPL = False

# Color palette — dark theme compatible
_COLORS = {
    "primary": "#58a6ff",
    "success": "#3fb950",
    "warning": "#d29922",
    "danger": "#f85149",
    "purple": "#bc8cff",
    "cyan": "#39d2c0",
    "orange": "#f0883e",
    "bg": "#0d1117",
    "card_bg": "#161b22",
    "grid": "#21262d",
    "text": "#c9d1d9",
    "text_bright": "#f0f6fc",
}


def _check_mpl() -> None:
    """Raise if matplotlib is not available."""
    if not _HAS_MPL:
        raise ImportError(
            "matplotlib is required for chart generation. "
            "Install with: pip install matplotlib"
        )


def _dark_style() -> dict[str, Any]:
    """Return style kwargs for dark-themed charts."""
    return {
        "facecolor": _COLORS["bg"],
    }


def _style_axes(ax: Any, title: str = "", xlabel: str = "", ylabel: str = "") -> None:
    """Apply dark-theme styling to axes."""
    ax.set_facecolor(_COLORS["card_bg"])
    ax.set_title(title, color=_COLORS["text_bright"], fontsize=13, fontweight="bold", pad=12)
    ax.set_xlabel(xlabel, color=_COLORS["text"], fontsize=10)
    ax.set_ylabel(ylabel, color=_COLORS["text"], fontsize=10)
    ax.tick_params(colors=_COLORS["text"], labelsize=9)
    ax.grid(True, alpha=0.15, color=_COLORS["grid"])
    for spine in ax.spines.values():
        spine.set_color(_COLORS["grid"])


def _fig_to_base64(fig: Any, dpi: int = 120) -> str:
    """Render figure to base64-encoded PNG string."""
    buf = io.BytesIO()
    fig.savefig(buf, format="png", dpi=dpi, bbox_inches="tight", facecolor=fig.get_facecolor())
    plt.close(fig)
    buf.seek(0)
    return base64.b64encode(buf.read()).decode("ascii")


def _fig_to_bytes(fig: Any, dpi: int = 120) -> bytes:
    """Render figure to raw PNG bytes."""
    buf = io.BytesIO()
    fig.savefig(buf, format="png", dpi=dpi, bbox_inches="tight", facecolor=fig.get_facecolor())
    plt.close(fig)
    buf.seek(0)
    return buf.read()


# ---------------------------------------------------------------------------
# Individual chart generators
# ---------------------------------------------------------------------------


def population_trend(
    timeline: list[dict[str, Any]],
    *,
    width: float = 8.0,
    height: float = 4.0,
    dpi: int = 120,
) -> str:
    """Generate a population trend line chart.

    Args:
        timeline: List of dicts with 'tick' and 'population' (or 'active_agents') keys.
        width: Figure width in inches.
        height: Figure height in inches.
        dpi: Resolution.

    Returns:
        Base64-encoded PNG image string.
    """
    _check_mpl()
    ticks = [e.get("tick", i) for i, e in enumerate(timeline)]
    pop = [e.get("population", e.get("active_agents", 0)) for e in timeline]

    fig, ax = plt.subplots(figsize=(width, height), **_dark_style())
    ax.fill_between(ticks, pop, alpha=0.2, color=_COLORS["primary"])
    ax.plot(ticks, pop, color=_COLORS["primary"], linewidth=2, marker="o", markersize=3)
    _style_axes(ax, title="Population Trend", xlabel="Tick", ylabel="Active Agents")

    if pop:
        ax.set_ylim(bottom=0, top=max(pop) * 1.15 if max(pop) > 0 else 10)

    return _fig_to_base64(fig, dpi)


def gdp_trajectory(
    timeline: list[dict[str, Any]],
    *,
    width: float = 8.0,
    height: float = 4.0,
    dpi: int = 120,
) -> str:
    """Generate a GDP trajectory area chart.

    Args:
        timeline: List of dicts with 'tick' and 'gdp' keys.

    Returns:
        Base64-encoded PNG string.
    """
    _check_mpl()
    ticks = [e.get("tick", i) for i, e in enumerate(timeline)]
    gdp = [e.get("gdp", 0) for e in timeline]

    fig, ax = plt.subplots(figsize=(width, height), **_dark_style())
    ax.fill_between(ticks, gdp, alpha=0.3, color=_COLORS["success"])
    ax.plot(ticks, gdp, color=_COLORS["success"], linewidth=2)
    _style_axes(ax, title="GDP Trajectory", xlabel="Tick", ylabel="GDP (tokens)")

    ax.yaxis.set_major_formatter(mticker.FuncFormatter(lambda x, _: f"{x:,.0f}"))

    return _fig_to_base64(fig, dpi)


def gini_coefficient(
    timeline: list[dict[str, Any]],
    *,
    thresholds: tuple[float, float] = (0.3, 0.6),
    width: float = 8.0,
    height: float = 4.0,
    dpi: int = 120,
) -> str:
    """Generate a Gini coefficient line chart with inequality bands.

    Args:
        timeline: List of dicts with 'tick' and 'gini' (or 'gini_coefficient') keys.
        thresholds: (low, high) Gini thresholds for color bands.

    Returns:
        Base64-encoded PNG string.
    """
    _check_mpl()
    ticks = [e.get("tick", i) for i, e in enumerate(timeline)]
    gini = [e.get("gini", e.get("gini_coefficient", 0)) for e in timeline]

    fig, ax = plt.subplots(figsize=(width, height), **_dark_style())

    # Threshold bands
    ax.axhspan(0, thresholds[0], alpha=0.08, color=_COLORS["success"], label="Low inequality")
    ax.axhspan(thresholds[0], thresholds[1], alpha=0.08, color=_COLORS["warning"], label="Moderate")
    ax.axhspan(thresholds[1], 1.0, alpha=0.08, color=_COLORS["danger"], label="High inequality")

    ax.plot(ticks, gini, color=_COLORS["purple"], linewidth=2, marker="s", markersize=3)
    _style_axes(ax, title="Gini Coefficient Over Time", xlabel="Tick", ylabel="Gini")
    ax.set_ylim(0, 1.0)
    ax.legend(loc="upper left", fontsize=8, facecolor=_COLORS["card_bg"],
              edgecolor=_COLORS["grid"], labelcolor=_COLORS["text"])

    return _fig_to_base64(fig, dpi)


def skill_distribution(
    skills: list[dict[str, Any]],
    *,
    width: float = 8.0,
    height: float = 4.5,
    dpi: int = 120,
) -> str:
    """Generate a horizontal bar chart of skill distribution.

    Args:
        skills: List of dicts with 'skill_name', 'agent_count', optionally 'avg_level'.

    Returns:
        Base64-encoded PNG string.
    """
    _check_mpl()
    if not skills:
        fig, ax = plt.subplots(figsize=(width, height), **_dark_style())
        _style_axes(ax, title="Skill Distribution (No Data)")
        return _fig_to_base64(fig, dpi)

    # Sort by agent_count descending
    sorted_skills = sorted(skills, key=lambda s: s.get("agent_count", 0))
    names = [s.get("skill_name", "?") for s in sorted_skills]
    counts = [s.get("agent_count", 0) for s in sorted_skills]
    colors = [_COLORS["primary"], _COLORS["success"], _COLORS["warning"],
              _COLORS["purple"], _COLORS["cyan"], _COLORS["orange"]]
    bar_colors = [colors[i % len(colors)] for i in range(len(names))]

    fig, ax = plt.subplots(figsize=(width, height), **_dark_style())
    bars = ax.barh(names, counts, color=bar_colors, height=0.6, edgecolor="none")

    # Value labels
    for bar, count in zip(bars, counts, strict=False):
        ax.text(bar.get_width() + 0.3, bar.get_y() + bar.get_height() / 2,
                str(count), va="center", color=_COLORS["text"], fontsize=9)

    _style_axes(ax, title="Skill Distribution", xlabel="Agent Count")
    ax.set_xlim(right=max(counts) * 1.25 if counts else 10)

    return _fig_to_base64(fig, dpi)


def survival_pie(
    alive: int,
    dead: int,
    *,
    width: float = 5.0,
    height: float = 5.0,
    dpi: int = 120,
) -> str:
    """Generate a survival rate pie chart.

    Args:
        alive: Number of alive agents.
        dead: Number of dead agents.

    Returns:
        Base64-encoded PNG string.
    """
    _check_mpl()
    fig, ax = plt.subplots(figsize=(width, height), **_dark_style())
    sizes = [alive, dead]
    labels = [f"Alive ({alive})", f"Dead ({dead})"]
    colors = [_COLORS["success"], _COLORS["danger"]]
    explode: tuple[float, ...] = (0.05, 0.0)

    if alive + dead == 0:
        sizes = [1]
        labels = ["No Data"]
        colors = [_COLORS["grid"]]
        explode = (0.0,)

    wedges, texts, autotexts = ax.pie(
        sizes, explode=explode if len(sizes) > 1 else None,
        labels=labels, colors=colors, autopct="%1.1f%%",
        startangle=90, textprops={"color": _COLORS["text"], "fontsize": 11},
        pctdistance=0.6,
    )
    for t in autotexts:
        t.set_color(_COLORS["text_bright"])
        t.set_fontweight("bold")

    ax.set_title("Agent Survival Rate", color=_COLORS["text_bright"],
                 fontsize=13, fontweight="bold", pad=15)
    fig.set_facecolor(_COLORS["bg"])

    return _fig_to_base64(fig, dpi)


def economic_dashboard(
    timeline: list[dict[str, Any]],
    *,
    width: float = 12.0,
    height: float = 6.0,
    dpi: int = 120,
) -> str:
    """Generate a multi-panel economic overview dashboard.

    Shows GDP, Gini, and population in a 1×3 grid.

    Args:
        timeline: List of dicts with tick, gdp, gini/gini_coefficient, population/active_agents.

    Returns:
        Base64-encoded PNG string.
    """
    _check_mpl()
    ticks = [e.get("tick", i) for i, e in enumerate(timeline)]

    fig, axes = plt.subplots(1, 3, figsize=(width, height), **_dark_style())

    # GDP
    gdp = [e.get("gdp", 0) for e in timeline]
    axes[0].fill_between(ticks, gdp, alpha=0.3, color=_COLORS["success"])
    axes[0].plot(ticks, gdp, color=_COLORS["success"], linewidth=2)
    _style_axes(axes[0], title="GDP", ylabel="Tokens")

    # Gini
    gini = [e.get("gini", e.get("gini_coefficient", 0)) for e in timeline]
    axes[1].plot(ticks, gini, color=_COLORS["purple"], linewidth=2, marker="s", markersize=2)
    axes[1].axhline(y=0.3, color=_COLORS["success"], linestyle="--", alpha=0.4, linewidth=1)
    axes[1].axhline(y=0.6, color=_COLORS["danger"], linestyle="--", alpha=0.4, linewidth=1)
    _style_axes(axes[1], title="Gini Coefficient")
    axes[1].set_ylim(0, 1.0)

    # Population
    pop = [e.get("population", e.get("active_agents", 0)) for e in timeline]
    axes[2].fill_between(ticks, pop, alpha=0.2, color=_COLORS["primary"])
    axes[2].plot(ticks, pop, color=_COLORS["primary"], linewidth=2)
    _style_axes(axes[2], title="Population", ylabel="Agents")

    fig.suptitle("Economic Overview", color=_COLORS["text_bright"],
                 fontsize=15, fontweight="bold", y=1.02)
    fig.tight_layout()

    return _fig_to_base64(fig, dpi)


def action_distribution_bar(
    actions: dict[str, int],
    *,
    width: float = 8.0,
    height: float = 4.5,
    dpi: int = 120,
) -> str:
    """Generate a bar chart of action type distribution.

    Args:
        actions: Dict of action_name -> count.

    Returns:
        Base64-encoded PNG string.
    """
    _check_mpl()
    if not actions:
        fig, ax = plt.subplots(figsize=(width, height), **_dark_style())
        _style_axes(ax, title="Action Distribution (No Data)")
        return _fig_to_base64(fig, dpi)

    sorted_items = sorted(actions.items(), key=lambda x: x[1], reverse=True)
    names = [k for k, v in sorted_items]
    counts = [v for k, v in sorted_items]

    fig, ax = plt.subplots(figsize=(width, height), **_dark_style())
    colors_list = [_COLORS["primary"], _COLORS["success"], _COLORS["warning"],
                   _COLORS["purple"], _COLORS["cyan"], _COLORS["orange"],
                   _COLORS["danger"]]
    bar_colors = [colors_list[i % len(colors_list)] for i in range(len(names))]

    ax.bar(names, counts, color=bar_colors, edgecolor="none")
    _style_axes(ax, title="Action Type Distribution", xlabel="Action", ylabel="Count")
    plt.xticks(rotation=30, ha="right")

    return _fig_to_base64(fig, dpi)


def cooperation_network_graph(
    interactions: list[dict[str, Any]],
    *,
    width: float = 7.0,
    height: float = 7.0,
    dpi: int = 120,
) -> str:
    """Generate a simple social network visualization from interaction data.

    Each agent is a node; edges are weighted by interaction count.

    Args:
        interactions: List of dicts with 'from', 'to', 'count' (or weight).

    Returns:
        Base64-encoded PNG string.
    """
    _check_mpl()
    if not interactions:
        fig, ax = plt.subplots(figsize=(width, height), **_dark_style())
        _style_axes(ax, title="Social Network (No Data)")
        return _fig_to_base64(fig, dpi)

    try:
        import numpy as np
    except ImportError:
        fig, ax = plt.subplots(figsize=(width, height), **_dark_style())
        _style_axes(ax, title="Social Network (numpy unavailable)")
        return _fig_to_base64(fig, dpi)

    # Build adjacency
    agents_set: set[str] = set()
    for inter in interactions:
        agents_set.add(str(inter.get("from", "")))
        agents_set.add(str(inter.get("to", "")))
    agents_list = sorted(agents_set)
    n = len(agents_list)
    if n == 0:
        fig, ax = plt.subplots(figsize=(width, height), **_dark_style())
        _style_axes(ax, title="Social Network (No Agents)")
        return _fig_to_base64(fig, dpi)

    idx = {a: i for i, a in enumerate(agents_list)}

    # Place nodes in a circle
    angles = np.linspace(0, 2 * np.pi, n, endpoint=False)
    xs = np.cos(angles)
    ys = np.sin(angles)

    fig, ax = plt.subplots(figsize=(width, height), **_dark_style())
    ax.set_facecolor(_COLORS["bg"])

    # Draw edges
    max_weight = max(
        (inter.get("count", inter.get("weight", 1)) for inter in interactions),
        default=1,
    )
    for inter in interactions:
        src = str(inter.get("from", ""))
        dst = str(inter.get("to", ""))
        w = inter.get("count", inter.get("weight", 1))
        if src in idx and dst in idx:
            si, di = idx[src], idx[dst]
            lw = 0.5 + 3.0 * (w / max(max_weight, 1))
            alpha = 0.2 + 0.6 * (w / max(max_weight, 1))
            ax.plot([xs[si], xs[di]], [ys[si], ys[di]],
                    color=_COLORS["primary"], linewidth=lw, alpha=alpha)

    # Draw nodes
    ax.scatter(xs, ys, s=200, c=_COLORS["cyan"], zorder=5, edgecolors=_COLORS["bg"], linewidths=2)

    # Labels
    for i, name in enumerate(agents_list):
        ax.annotate(name[:8], (xs[i], ys[i]), textcoords="offset points",
                    xytext=(8, 8), fontsize=8, color=_COLORS["text"])

    ax.set_title("Social Network", color=_COLORS["text_bright"], fontsize=13, fontweight="bold")
    ax.set_xlim(-1.5, 1.5)
    ax.set_ylim(-1.5, 1.5)
    ax.axis("off")
    fig.set_facecolor(_COLORS["bg"])

    return _fig_to_base64(fig, dpi)
