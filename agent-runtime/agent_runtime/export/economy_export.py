"""Economy metrics time series exporter."""

from __future__ import annotations

import csv
import io
import json
from dataclasses import dataclass


@dataclass
class EconomyDataPoint:
    """Single tick economy metrics."""
    tick: int
    total_money: int
    total_tokens: int
    agent_count: int
    alive_count: int
    gini_coefficient: float
    task_count: int


def compute_gini(values: list[float]) -> float:
    """Compute Gini coefficient for wealth distribution.

    Uses the standard formula: G = (2 * sum(i * x_i)) / (n * sum(x_i)) - (n+1)/n
    where x_i are sorted values.

    Returns 0.0 for empty or uniform distributions.
    """
    if not values or len(values) < 2:
        return 0.0

    sorted_vals = sorted(values)
    n = len(sorted_vals)
    total = sum(sorted_vals)

    if total == 0:
        return 0.0

    # Gini using the formula: G = (2 * Σ(i * x_i)) / (n * Σ(x_i)) - (n + 1) / n
    weighted_sum = sum((i + 1) * x for i, x in enumerate(sorted_vals))
    gini = (2.0 * weighted_sum) / (n * total) - (n + 1) / n
    return round(max(0.0, gini), 6)


class EconomyExporter:
    """Export economy metrics time series.

    Computes wealth distribution, Gini coefficient, and resource metrics
    from agent state data.
    """

    def __init__(self) -> None:
        self._data_points: list[EconomyDataPoint] = []

    def add_tick_data(self, tick: int, agents: list[dict],
                      task_count: int = 0) -> EconomyExporter:
        """Add economy data for a single tick.

        Args:
            tick: The tick number.
            agents: List of agent dicts with 'money', 'tokens', 'alive' keys.
            task_count: Number of active tasks.
        """
        total_money = sum(a.get("money", 0) for a in agents)
        total_tokens = sum(a.get("tokens", 0) for a in agents)
        alive_count = sum(1 for a in agents if a.get("alive", True))

        wealth = [float(a.get("money", 0)) for a in agents]
        gini = compute_gini(wealth)

        self._data_points.append(EconomyDataPoint(
            tick=tick,
            total_money=total_money,
            total_tokens=total_tokens,
            agent_count=len(agents),
            alive_count=alive_count,
            gini_coefficient=gini,
            task_count=task_count,
        ))
        return self

    def export_json(self) -> str:
        """Export all data points as JSON."""
        data = [
            {
                "tick": dp.tick,
                "total_money": dp.total_money,
                "total_tokens": dp.total_tokens,
                "agent_count": dp.agent_count,
                "alive_count": dp.alive_count,
                "gini_coefficient": dp.gini_coefficient,
                "task_count": dp.task_count,
            }
            for dp in self._data_points
        ]
        return json.dumps(data, indent=2, ensure_ascii=False)

    def export_csv(self) -> str:
        """Export as CSV compatible with Pandas read_csv()."""
        output = io.StringIO()
        writer = csv.writer(output, lineterminator="\n")
        writer.writerow([
            "tick", "total_money", "total_tokens", "agent_count",
            "alive_count", "gini_coefficient", "task_count",
        ])
        for dp in self._data_points:
            writer.writerow([
                dp.tick, dp.total_money, dp.total_tokens,
                dp.agent_count, dp.alive_count, dp.gini_coefficient,
                dp.task_count,
            ])
        return output.getvalue()

    def get_summary(self) -> dict:
        """Get summary statistics across all data points."""
        if not self._data_points:
            return {"total_ticks": 0}

        return {
            "total_ticks": len(self._data_points),
            "tick_range": [self._data_points[0].tick, self._data_points[-1].tick],
            "max_gini": max(dp.gini_coefficient for dp in self._data_points),
            "min_gini": min(dp.gini_coefficient for dp in self._data_points),
            "avg_alive": sum(dp.alive_count for dp in self._data_points) / len(self._data_points),
            "total_money_final": self._data_points[-1].total_money,
            "total_tokens_final": self._data_points[-1].total_tokens,
        }

    def clear(self) -> None:
        """Reset stored data points."""
        self._data_points.clear()
