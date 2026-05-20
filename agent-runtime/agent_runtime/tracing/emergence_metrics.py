"""Emergence metrics — language emergence indicators for the tracing pipeline.

Computes aggregate language-emergence metrics from agent communication data:
  - Linguistic diversity (vocabulary overlap between groups)
  - Dialect strength (inter-group distance over time)
  - Jargon emergence rate (new group-specific terms per tick)
  - Vocabulary richness trend

These metrics feed into the tracing/collector pipeline for dashboard visualization.
"""

from __future__ import annotations

import logging
from dataclasses import dataclass, field
from typing import Any

from agent_runtime.social.comm_analyzer import CommunicationAnalyzer, DialectReport
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


class EmergenceMetrics:
    """Compute and accumulate language emergence metrics over time.

    Usage::

        metrics = EmergenceMetrics()
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
