"""Data collector — pulls timeline data from disk, synthetic generator, or live engine.

Two modes:
- ``synthetic`` — deterministically generates a plausible world timeline
  (CI-safe, no network, no API key).
- ``live``      — connects to a running world-engine via the SDK REST client
  and pulls the same shape of data.
"""

from __future__ import annotations

import json
import random
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

# ── Project root ──────────────────────────────────────────────────────────

PROJECT_ROOT = Path(__file__).resolve().parents[3]
DASHBOARD_DATA = PROJECT_ROOT / "dashboard" / "public" / "data"


# ── Data containers ───────────────────────────────────────────────────────


@dataclass
class TimelineEvent:
    """A single milestone event on the world timeline."""

    tick: int
    type: str
    title: str
    description: str
    involved_agents: list[str] = field(default_factory=list)
    involved_orgs: list[str] = field(default_factory=list)


@dataclass
class MetricSnapshot:
    """One row of the economic / demographic time-series."""

    tick: int
    population: int
    active_agents: int
    gdp: float
    gini: float


@dataclass
class NetworkSnapshot:
    """Social network snapshot (single point in time)."""

    tick: int
    nodes: list[dict[str, Any]]
    edges: list[dict[str, Any]]


@dataclass
class DocumentaryData:
    """All raw data the narrator and renderer need."""

    events: list[TimelineEvent]
    metrics: list[MetricSnapshot]
    network: NetworkSnapshot
    total_ticks: int
    source: str  # "synthetic" | "live" | "sample"

    def to_dict(self) -> dict[str, Any]:
        return {
            "source": self.source,
            "total_ticks": self.total_ticks,
            "events": [e.__dict__ for e in self.events],
            "metrics": [m.__dict__ for m in self.metrics],
            "network": {
                "tick": self.network.tick,
                "nodes": self.network.nodes,
                "edges": self.network.edges,
            },
        }


# ── Public API ────────────────────────────────────────────────────────────


def collect(
    mode: str = "synthetic",
    *,
    engine_url: str = "http://localhost:8080",
    sample_dir: Path | None = None,
    seed: int = 42,
    n_agents: int = 50,
    total_ticks: int = 5000,
) -> DocumentaryData:
    """Collect documentary data in the requested ``mode``.

    Parameters
    ----------
    mode:
        ``"synthetic"`` (default) — offline generated data, CI-safe.
        ``"live"``                 — pull from a running engine.
        ``"sample"``               — read the JSON files shipped in
        ``dashboard/public/data/``.
    engine_url:
        World-engine base URL (``live`` mode only).
    sample_dir:
        Override the directory for ``sample`` mode.
    seed:
        RNG seed for synthetic mode (deterministic).
    n_agents, total_ticks:
        Parameters for synthetic mode.
    """
    if mode == "synthetic":
        return _collect_synthetic(seed=seed, n_agents=n_agents, total_ticks=total_ticks)
    if mode == "live":
        return _collect_live(engine_url)
    if mode == "sample":
        return _collect_sample(sample_dir or DASHBOARD_DATA)
    raise ValueError(f"Unknown mode: {mode!r}")


# ── Sample mode (dashboard JSON) ──────────────────────────────────────────


def _collect_sample(data_dir: Path) -> DocumentaryData:
    """Read the JSON files shipped in ``dashboard/public/data/``."""
    events_raw = _load_json(data_dir / "timeline-events.json")
    snapshots_raw = _load_json(data_dir / "world-snapshots.json")
    network_raw = _load_json(data_dir / "interaction-network.json")

    events = [
        TimelineEvent(
            tick=e["tick"],
            type=e["type"],
            title=e["title"],
            description=e["description"],
            involved_agents=[a["name"] for a in e.get("involved_agents", [])],
            involved_orgs=[o["name"] for o in e.get("involved_orgs", [])],
        )
        for e in events_raw
    ]

    metrics = [
        MetricSnapshot(
            tick=s["tick"],
            population=s["total_population"],
            active_agents=s["active_agents"],
            gdp=float(s["gdp"]),
            gini=float(s["gini_coefficient"]),
        )
        for s in snapshots_raw
    ]

    total_ticks = max((m.tick for m in metrics), default=0)

    network = NetworkSnapshot(
        tick=total_ticks,
        nodes=network_raw.get("nodes", []),
        edges=network_raw.get("edges", []),
    )

    return DocumentaryData(
        events=events,
        metrics=metrics,
        network=network,
        total_ticks=total_ticks,
        source="sample",
    )


# ── Synthetic mode ────────────────────────────────────────────────────────


def _collect_synthetic(
    *,
    seed: int = 42,
    n_agents: int = 50,
    total_ticks: int = 5000,
) -> DocumentaryData:
    """Generate a plausible world timeline deterministically."""
    rng = random.Random(seed)

    # Events — milestone beats spaced across the timeline
    event_templates = [
        (
            45, "milestone", "第一次交易",
            "两名代理完成了世界上第一次物物交换，标志着经济活动的开始。",
        ),
        (
            180, "milestone", "首位技能大师",
            "一位代理在 crafting 领域达到大师级别，成为所有人的榜样。",
        ),
        (
            320, "organization", "第一个组织成立",
            "一位代理创立了丰收行会，这是世界上第一个正式组织。",
        ),
        (
            510, "cultural", "文化分化初现",
            "南北两个群体开始形成截然不同的价值观和文化传统。",
        ),
        (
            680, "economic", "贸易网络形成",
            "连接超过 20 位代理的贸易网络正式形成。",
        ),
        (
            850, "governance", "第一次选举",
            "代理们通过 ranked-choice 投票选出了首届议事会。",
        ),
        (
            1200, "governance", "第一条规则通过",
            "关于资源分配的提案获得多数票，成为世界上第一条正式规则。",
        ),
        (
            1700, "economic", "经济泡沫",
            "投机性交易推高了物品价格，形成了经济泡沫。",
        ),
        (
            1900, "economic", "泡沫破裂",
            "价格崩溃，许多代理陷入贫困，催生了金融监管的呼声。",
        ),
        (
            2350, "milestone", "跨组织条约",
            "多个组织签订了第一份跨组织条约，建立了和平协议。",
        ),
        (
            2850, "economic", "金融体系建立",
            "代理们创建了统一的货币与信用体系，贸易效率大幅提升。",
        ),
        (
            3100, "governance", "联合治理体",
            "所有组织联合成立了统一的治理机构，取代了分散的议事会。",
        ),
        (
            3700, "cultural", "文化复兴",
            "经历危机后，文化多样性迎来了一次大规模复兴。",
        ),
        (
            4400, "governance", "宪政改革",
            "治理结构被重写，权力得到更公平的分配。",
        ),
        (
            4900, "milestone", "文明的黎明",
            "代理们回顾漫长的历史，宣告文明进入新的纪元。",
        ),
    ]
    tick_scale = total_ticks / 5000.0 if total_ticks > 0 else 1.0
    events = [
        TimelineEvent(
            tick=int(t * tick_scale),
            type=typ,
            title=title,
            description=desc,
            involved_agents=[f"代理-{rng.randint(1, n_agents)}" for _ in range(2)],
            involved_orgs=[],
        )
        for t, typ, title, desc in event_templates
    ]

    # Metrics — population, GDP, Gini across the timeline
    n_snapshots = 51
    step = max(1, total_ticks // (n_snapshots - 1))
    metrics: list[MetricSnapshot] = []
    wealth = [100.0] * n_agents
    for i in range(n_snapshots):
        tick = min(i * step, total_ticks)
        # Drift wealth to simulate economic growth + inequality
        for j in range(n_agents):
            wealth[j] *= 1.0 + rng.gauss(0.003, 0.02)
            wealth[j] = max(1.0, wealth[j])
        pop = n_agents - int(tick / total_ticks * n_agents * 0.15) if total_ticks else n_agents
        gdp = sum(wealth) / max(1, n_agents) * pop * 0.1
        gini = _gini(wealth)
        metrics.append(
            MetricSnapshot(
                tick=tick,
                population=pop,
                active_agents=max(1, pop - rng.randint(0, 3)),
                gdp=round(gdp, 2),
                gini=round(gini, 4),
            )
        )

    # Network — random small-world-ish graph
    edges: list[dict[str, Any]] = []
    node_ids = [f"agent-{i}" for i in range(n_agents)]
    for i in range(n_agents):
        for j in range(i + 1, min(i + 4, n_agents)):
            if rng.random() < 0.6:
                edges.append(
                    {"source": node_ids[i], "target": node_ids[j], "weight": rng.randint(1, 30)}
                )
    # Add some random long-range ties
    for _ in range(n_agents):
        a, b = rng.randrange(n_agents), rng.randrange(n_agents)
        if a != b:
            edges.append(
                {"source": node_ids[a], "target": node_ids[b], "weight": rng.randint(1, 10)}
            )

    nodes = [{"id": nid, "name": f"代理-{i}", "group": i % 5} for i, nid in enumerate(node_ids)]
    network = NetworkSnapshot(tick=total_ticks, nodes=nodes, edges=edges)

    return DocumentaryData(
        events=events,
        metrics=metrics,
        network=network,
        total_ticks=total_ticks,
        source="synthetic",
    )


# ── Live mode (SDK) ───────────────────────────────────────────────────────


def _collect_live(engine_url: str) -> DocumentaryData:
    """Pull data from a running world-engine via the SDK."""
    try:
        from agent_world_sdk.client import AgentWorldClient  # type: ignore[import-untyped]
    except ImportError as e:
        raise RuntimeError(
            "agent_world_sdk is required for --mode live; install with `pip install -e sdk/`"
        ) from e

    client = AgentWorldClient(engine_url)
    timeline = client.export.timeseries(format="json")  # type: ignore[attr-defined]
    world = client.world.state()  # type: ignore[attr-defined]
    network_data = client.export.network_graph()  # type: ignore[attr-defined]

    events = _parse_live_events(timeline)
    metrics = _parse_live_metrics(timeline)
    total_ticks = int(world.get("tick", 0))
    network = NetworkSnapshot(
        tick=total_ticks,
        nodes=network_data.get("nodes", []),
        edges=network_data.get("edges", []),
    )
    return DocumentaryData(
        events=events,
        metrics=metrics,
        network=network,
        total_ticks=total_ticks,
        source="live",
    )


def _parse_live_events(timeline: Any) -> list[TimelineEvent]:
    """Best-effort extraction of timeline events from engine data."""
    events: list[TimelineEvent] = []
    raw = timeline if isinstance(timeline, list) else timeline.get("events", [])
    for e in raw:
        if not isinstance(e, dict):
            continue
        events.append(
            TimelineEvent(
                tick=int(e.get("tick", 0)),
                type=str(e.get("type", "milestone")),
                title=str(e.get("title", "")),
                description=str(e.get("description", "")),
                involved_agents=[a.get("name", "") for a in e.get("involved_agents", [])],
                involved_orgs=[o.get("name", "") for o in e.get("involved_orgs", [])],
            )
        )
    return events


def _parse_live_metrics(timeline: Any) -> list[MetricSnapshot]:
    """Best-effort extraction of metric snapshots from engine data."""
    metrics: list[MetricSnapshot] = []
    raw = timeline if isinstance(timeline, list) else timeline.get("snapshots", [])
    for s in raw:
        if not isinstance(s, dict):
            continue
        metrics.append(
            MetricSnapshot(
                tick=int(s.get("tick") or 0),
                population=int(s.get("total_population", s.get("population")) or 0),
                active_agents=int(s.get("active_agents", s.get("population")) or 0),
                gdp=float(s.get("gdp") or 0.0),
                gini=float(s.get("gini_coefficient", s.get("gini")) or 0.0),
            )
        )
    return metrics


# ── Helpers ───────────────────────────────────────────────────────────────


def _load_json(path: Path) -> Any:
    with open(path, encoding="utf-8") as f:
        return json.load(f)


def _gini(values: list[float]) -> float:
    """Gini coefficient on a list of non-negative values."""
    n = len(values)
    if n < 2:
        return 0.0
    s = sorted(values)
    total = sum(s)
    if total == 0:
        return 0.0
    weighted = sum((2 * (i + 1) - 1 - n) * v for i, v in enumerate(s))
    return abs(weighted / (n * total))
