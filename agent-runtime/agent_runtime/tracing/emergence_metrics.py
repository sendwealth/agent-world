"""Emergence behavior metrics collection and export.

Collects per-tick agent state snapshots and computes aggregate metrics
for diversity, cooperation, survival, and action distribution.
"""

from __future__ import annotations

import math
from collections import Counter, defaultdict
from typing import Any


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
