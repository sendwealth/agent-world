"""Emergence behavior metrics collection and export.

This module provides two metric classes:

- ``EmergenceMetrics``: General-purpose agent behavior metrics (diversity,
  cooperation, survival, action distribution) — used by the tracing pipeline.
- ``LanguageEmergenceMetrics``: Language emergence indicators (linguistic
  diversity, dialect strength, jargon detection, vocabulary richness) —
  used by the Phase 4.3.4 language emergence system.
"""

from __future__ import annotations

import logging
import math
from collections import Counter, defaultdict
from dataclasses import dataclass, field
from typing import Any

from agent_runtime.social.comm_analyzer import CommunicationAnalyzer
from agent_runtime.social.jargon_detector import JargonDetector

logger = logging.getLogger(__name__)


@dataclass
class LanguageEmergenceSnapshot:
    """Point-in-time snapshot of language emergence metrics."""

    tick: int = 0
    # Number of distinct groups analyzed.
    group_count: int = 0
    # Average pairwise linguistic distance (0.0–1.0).
    avg_linguistic_distance: float = 0.0
    # Dialect strength (0.0–1.0, from DialectReport).
    dialect_strength: float = 0.0
    # Whether dialect has been detected.
    dialect_detected: bool = False
    # Number of group-specific jargon terms.
    jargon_count: int = 0
    # Top jargon terms by specificity.
    top_jargon: list[dict[str, Any]] = field(default_factory=list)
    # Average vocabulary richness across all groups.
    avg_vocab_richness: float = 0.0


class LanguageEmergenceMetrics:
    """Compute and accumulate language emergence metrics over time.

    Usage::

        metrics = LanguageEmergenceMetrics()
        snapshot = metrics.compute(tick=42, groups={
            "traders": ["buy low sell high", ...],
            "builders": ["construct the bridge", ...],
        })
    """

    def __init__(self) -> None:
        self._analyzer = CommunicationAnalyzer()
        self._detector = JargonDetector()
        self._history: list[LanguageEmergenceSnapshot] = []

    def compute(
        self,
        tick: int,
        groups: dict[str, list[str]],
        dialect_threshold: float = 0.3,
    ) -> LanguageEmergenceSnapshot:
        """Compute emergence metrics for one tick.

        Args:
            tick: Current tick number.
            groups: Mapping of group_id -> list of message strings.
            dialect_threshold: Minimum distance to consider dialect emerged.

        Returns:
            LanguageEmergenceSnapshot with all computed metrics.
        """
        group_names = list(groups.keys())
        group_count = len(group_names)

        if group_count < 2:
            snap = LanguageEmergenceSnapshot(
                tick=tick,
                group_count=group_count,
            )
            self._history.append(snap)
            return snap

        # 1. Pairwise linguistic distances.
        distances: list[float] = []
        for i in range(len(group_names)):
            for j in range(i + 1, len(group_names)):
                msgs_a = groups[group_names[i]]
                msgs_b = groups[group_names[j]]
                dist = self._detector.compute_linguistic_distance(msgs_a, msgs_b)
                distances.append(dist)

        avg_distance = sum(distances) / len(distances) if distances else 0.0

        # 2. Dialect detection.
        messages_over_time_entry: dict[str, Any] = {
            "period": str(tick),
            "groups": groups,
        }
        # Build a minimal time-series with current data only.
        dialect_report = self._analyzer.detect_emerging_dialect(
            [messages_over_time_entry],
            distance_threshold=dialect_threshold,
        )

        # 3. Jargon detection.
        jargon_terms = self._detector.detect_group_specific_terms(groups)
        top_jargon = [
            {"term": t.term, "group": t.group_id, "specificity": t.specificity}
            for t in jargon_terms[:10]
        ]

        # 4. Vocabulary richness.
        richnesses: list[float] = []
        for group_name, msgs in groups.items():
            pattern = self._analyzer.analyze_message_patterns(group_name, msgs)
            richnesses.append(pattern.vocabulary_richness)
        avg_richness = sum(richnesses) / len(richnesses) if richnesses else 0.0

        snapshot = LanguageEmergenceSnapshot(
            tick=tick,
            group_count=group_count,
            avg_linguistic_distance=round(avg_distance, 4),
            dialect_strength=dialect_report.dialect_strength,
            dialect_detected=dialect_report.has_dialect,
            jargon_count=len(jargon_terms),
            top_jargon=top_jargon,
            avg_vocab_richness=round(avg_richness, 4),
        )
        self._history.append(snapshot)
        return snapshot

    @property
    def history(self) -> list[LanguageEmergenceSnapshot]:
        """Return all historical snapshots."""
        return list(self._history)

    def get_trend(self, metric: str = "avg_linguistic_distance") -> list[float]:
        """Extract a metric trend over time.

        Args:
            metric: One of 'avg_linguistic_distance', 'dialect_strength',
                'jargon_count', 'avg_vocab_richness'.

        Returns:
            List of float values in chronological order.
        """
        return [getattr(s, metric, 0.0) for s in self._history]

    def clear_history(self) -> None:
        """Clear accumulated history."""
        self._history.clear()


# ---------------------------------------------------------------------------
# General emergence behavior metrics (SEN-176)
# ---------------------------------------------------------------------------


class EmergenceMetrics:
    """Collect and export emergence behavior metrics.

    Call ``record_tick`` with each tick's snapshot data, then use the
    ``compute_*`` methods or ``export_json`` to retrieve results.
    """

    def __init__(self) -> None:
        self._ticks: list[dict[str, Any]] = []

    def record_tick(self, snapshot: dict[str, Any]) -> None:
        """Record a tick-level snapshot.

        Expected keys: ``tick`` (int), ``agents`` (list of agent dicts).
        Each agent dict should contain at least ``id``, ``action``,
        ``alive`` (bool), and optionally ``interactions`` (list).
        """
        self._ticks.append(snapshot)

    # -- Metric methods ---------------------------------------------------

    def compute_diversity_index(self) -> float:
        """Compute Shannon entropy–based action diversity index (0.0–1.0).

        Normalized so that a uniform distribution over all observed action
        types yields 1.0.  Returns 0.0 when there are no actions.
        """
        actions = self._collect_actions()
        if not actions:
            return 0.0
        counts = Counter(actions)
        total = len(actions)
        entropy = -sum(
            (c / total) * math.log2(c / total) for c in counts.values()
        )
        max_entropy = math.log2(len(counts)) if len(counts) > 1 else 1.0
        return entropy / max_entropy if max_entropy > 0 else 0.0

    def compute_cooperation_rate(self) -> float:
        """Compute the fraction of ticks where at least one interaction occurred."""
        if not self._ticks:
            return 0.0
        coop_ticks = sum(
            1
            for t in self._ticks
            if any(
                bool(agent.get("interactions"))
                for agent in t.get("agents", [])
            )
        )
        return coop_ticks / len(self._ticks)

    def compute_survival_rate(self) -> float:
        """Compute the fraction of agents alive in the latest tick."""
        if not self._ticks:
            return 0.0
        latest = self._ticks[-1]
        agents = latest.get("agents", [])
        if not agents:
            return 0.0
        alive = sum(1 for a in agents if a.get("alive", True))
        return alive / len(agents)

    def compute_action_distribution(self) -> dict[str, int]:
        """Return a histogram of action types across all ticks and agents."""
        return dict(Counter(self._collect_actions()))

    def compute_survival_mode_distribution(self) -> dict[str, int]:
        """Return a histogram of survival outcomes across all agents and ticks."""
        outcomes: list[str] = []
        for tick in self._ticks:
            for agent in tick.get("agents", []):
                outcome = agent.get("survival_outcome")
                if outcome is not None:
                    outcomes.append(str(outcome))
        return dict(Counter(outcomes))

    def compute_per_agent_action_counts(self) -> dict[str, dict[str, int]]:
        """Return per-agent action histograms."""
        per_agent: dict[str, Counter[str]] = defaultdict(Counter)
        for tick in self._ticks:
            for agent in tick.get("agents", []):
                aid = agent.get("id", "?")
                action = agent.get("action")
                if action is not None:
                    per_agent[aid][action] += 1
        return {aid: dict(cnt) for aid, cnt in per_agent.items()}

    def export_json(self) -> dict[str, Any]:
        """Export all metrics as a JSON-serializable dict."""
        return {
            "total_ticks": len(self._ticks),
            "diversity_index": self.compute_diversity_index(),
            "cooperation_rate": self.compute_cooperation_rate(),
            "survival_rate": self.compute_survival_rate(),
            "action_distribution": self.compute_action_distribution(),
            "survival_mode_distribution": self.compute_survival_mode_distribution(),
            "per_agent_action_counts": self.compute_per_agent_action_counts(),
        }

    # -- Internal helpers -------------------------------------------------

    def _collect_actions(self) -> list[str]:
        """Flatten all agent actions across all recorded ticks."""
        actions: list[str] = []
        for tick in self._ticks:
            for agent in tick.get("agents", []):
                action = agent.get("action")
                if action is not None:
                    actions.append(action)
        return actions
