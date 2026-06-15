"""Narrative generator — organises raw timeline data into a narrated storyboard.

The narrator groups events into thematic *chapters* (Civilisation Birth,
Economy, Society/Governance, Culture, Conclusion), assigns each chapter a set
of dynamic-visualisation *scenes*, and emits a structured
:class:`Storyboard` that the renderer and composer consume.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any

from .collector import DocumentaryData

# ── Data containers ───────────────────────────────────────────────────────


@dataclass
class Scene:
    """One visual segment inside a chapter."""

    scene_type: str  # "population" | "gdp" | "gini" | "network" | "title_card"
    title: str
    subtitle: str = ""
    caption: str = ""  # shown as subtitle / narration
    data_key: str = ""  # which data slice the renderer needs


@dataclass
class Chapter:
    """A thematic chapter of the documentary."""

    title: str
    chapter_num: int
    narration: str
    scenes: list[Scene] = field(default_factory=list)
    tick_range: tuple[int, int] = (0, 0)


@dataclass
class Storyboard:
    """Top-level narrative structure consumed by renderer + composer."""

    title: str
    subtitle: str
    chapters: list[Chapter]
    fps: int = 24
    scene_duration_seconds: float = 4.0  # per scene

    @property
    def estimated_duration_seconds(self) -> float:
        total_scenes = sum(len(c.scenes) for c in self.chapters)
        return total_scenes * self.scene_duration_seconds

    def to_dict(self) -> dict[str, Any]:
        return {
            "title": self.title,
            "subtitle": self.subtitle,
            "fps": self.fps,
            "scene_duration_seconds": self.scene_duration_seconds,
            "estimated_duration_seconds": self.estimated_duration_seconds,
            "chapters": [
                {
                    "title": c.title,
                    "chapter_num": c.chapter_num,
                    "narration": c.narration,
                    "tick_range": c.tick_range,
                    "scenes": [s.__dict__ for s in c.scenes],
                }
                for c in self.chapters
            ],
        }


# ── Chapter definitions ──────────────────────────────────────────────────

# Maps event type → chapter index
_TYPE_TO_CHAPTER: dict[str, int] = {
    "milestone": 0,     # Civilisation Birth
    "economic": 1,      # Economy
    "organization": 2,  # Society & Governance
    "governance": 2,    # Society & Governance
    "cultural": 3,      # Culture
}

_CHAPTER_TITLES = [
    "文明的诞生",
    "经济的萌芽",
    "社会组织与治理",
    "文化繁荣与总结",
]


# ── Public API ────────────────────────────────────────────────────────────


def build_storyboard(
    data: DocumentaryData,
    *,
    title: str = "Agent World — 世界历史纪录片",
    subtitle: str = "自动生成 · Phase 5.4",
    fps: int = 24,
    scene_duration: float = 4.0,
) -> Storyboard:
    """Build a structured :class:`Storyboard` from :class:`DocumentaryData`.

    The storyboard always has at least four chapters to satisfy the Phase 5.4
    acceptance criterion ("at least 4 chapters").
    """
    # Bucket events into chapters
    chapter_events: list[list[Any]] = [[] for _ in range(4)]
    for ev in data.events:
        idx = _TYPE_TO_CHAPTER.get(ev.type, 0)
        chapter_events[idx].append(ev)

    chapters: list[Chapter] = []
    for i, title_str in enumerate(_CHAPTER_TITLES):
        events = sorted(chapter_events[i], key=lambda e: e.tick)
        if events:
            t0, t1 = events[0].tick, events[-1].tick
        else:
            t0, t1 = 0, data.total_ticks

        narration = _narrate_chapter(i, events, data)
        scenes = _scenes_for_chapter(i, events, data)

        chapters.append(
            Chapter(
                title=title_str,
                chapter_num=i + 1,
                narration=narration,
                scenes=scenes,
                tick_range=(t0, t1),
            )
        )

    return Storyboard(
        title=title,
        subtitle=subtitle,
        chapters=chapters,
        fps=fps,
        scene_duration_seconds=scene_duration,
    )


# ── Narration generation ──────────────────────────────────────────────────


def _narrate_chapter(chapter_idx: int, events: list[Any], data: DocumentaryData) -> str:
    """Generate Chinese narration text for a chapter."""
    if not events:
        return _default_narration(chapter_idx, data)

    event_lines = "；".join(e.title for e in events[:5])
    if len(events) > 5:
        event_lines += f"等 {len(events)} 个事件"

    templates = [
        # Chapter 0 — Civilisation Birth
        f"在世界诞生之初，代理们从零开始探索生存之道。{event_lines}。"
        f"从最初的 {data.metrics[0].population if data.metrics else '?'} 位居民，"
        "文明的火种被点燃。",

        # Chapter 1 — Economy
        f"经济活动逐渐成为社会的命脉。{event_lines}。"
        + _economic_summary(data),

        # Chapter 2 — Society & Governance
        f"代理们发现，合作比独自生存更加有效。{event_lines}。"
        "组织、选举、规则——治理结构一步步成型。",

        # Chapter 3 — Culture & Conclusion
        f"文化多样性在历史长河中绽放。{event_lines}。"
        + _conclusion_summary(data),
    ]
    return templates[chapter_idx]


def _economic_summary(data: DocumentaryData) -> str:
    if not data.metrics:
        return ""
    first, last = data.metrics[0], data.metrics[-1]
    gdp_growth = ((last.gdp - first.gdp) / max(first.gdp, 0.01)) * 100
    gini_change = last.gini - first.gini
    return (
        f"GDP 从 {first.gdp:.1f} 增长至 {last.gdp:.1f}"
        f"（{'增长' if gdp_growth >= 0 else '下降'} {abs(gdp_growth):.1f}%），"
        f"Gini 系数从 {first.gini:.3f} 变为 {last.gini:.3f}"
        f"（{'上升' if gini_change >= 0 else '下降'} {abs(gini_change):.3f}）。"
    )


def _conclusion_summary(data: DocumentaryData) -> str:
    if not data.metrics:
        return "这段历史，见证了代理文明的兴衰。"
    last = data.metrics[-1]
    return (
        f"最终，世界拥有 {last.population} 位居民、"
        f"GDP 达 {last.gdp:.1f}。"
        "这段历史，见证了代理文明的兴衰。"
    )


def _default_narration(chapter_idx: int, data: DocumentaryData) -> str:
    defaults = [
        "在世界诞生之初，代理们从零开始探索生存之道，文明的火种被点燃。",
        "经济活动逐渐成为社会的命脉，贸易网络与金融体系一步步成型。",
        "代理们发现合作比独自生存更有效，组织、选举、规则构成了治理的基石。",
        "文化多样性在历史长河中绽放，这段历史见证了代理文明的兴衰。",
    ]
    return defaults[chapter_idx]


# ── Scene assignment ──────────────────────────────────────────────────────


def _scenes_for_chapter(
    chapter_idx: int, events: list[Any], data: DocumentaryData
) -> list[Scene]:
    """Assign visualisation scenes to a chapter.

    Every chapter gets at least one title-card scene plus at least one
    data-visualisation scene, ensuring the acceptance criterion
    "each chapter has dynamic visualisation" is met.
    """
    scenes: list[Scene] = []

    # Title card for the chapter
    title_ev = events[0] if events else None
    if title_ev:
        subtitle_str = f"第 {chapter_idx + 1} 章 · Tick {title_ev.tick}"
    else:
        subtitle_str = f"第 {chapter_idx + 1} 章"
    scenes.append(
        Scene(
            scene_type="title_card",
            title=_CHAPTER_TITLES[chapter_idx],
            subtitle=subtitle_str,
            caption=title_ev.description if title_ev else "",
        )
    )

    # Data-visualisation scenes per chapter
    if chapter_idx == 0:
        scenes.append(
            Scene(
                scene_type="population",
                title="人口增长",
                subtitle="Population Growth",
                caption="世界人口从初始状态逐渐增长，代理们建立了第一个聚落。",
                data_key="population",
            )
        )
    elif chapter_idx == 1:
        scenes.append(
            Scene(
                scene_type="gdp",
                title="GDP 演变",
                subtitle="GDP Over Time",
                caption="经济产出随时间持续增长，贸易网络推动了繁荣。",
                data_key="gdp",
            )
        )
        scenes.append(
            Scene(
                scene_type="gini",
                title="Gini 系数",
                subtitle="Wealth Inequality (Gini)",
                caption="财富分配的不平等程度在经济周期中波动。",
                data_key="gini",
            )
        )
    elif chapter_idx == 2:
        scenes.append(
            Scene(
                scene_type="network",
                title="社交网络",
                subtitle="Social Network Evolution",
                caption="代理之间的关系网络不断扩展，形成了复杂的社会结构。",
                data_key="network",
            )
        )
    elif chapter_idx == 3:
        scenes.append(
            Scene(
                scene_type="population",
                title="文明回顾",
                subtitle="Population — Full Timeline",
                caption="回顾整个文明历程，人口经历了增长、迁移与沉淀。",
                data_key="population",
            )
        )

    return scenes
