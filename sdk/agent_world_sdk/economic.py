"""Economic analysis sub-module — wealth distribution, price trends, inflation.

These helpers operate on data dicts returned by the SDK or loaded from
exported files.  They do not make HTTP requests.
"""

from __future__ import annotations

import math
from typing import Any


class EconomicModule:
    """Economic analysis functions for Agent World data."""

    # -- Gini coefficient ----------------------------------------------------

    @staticmethod
    def gini(values: list[float]) -> float:
        """Compute the Gini coefficient for a list of non-negative values.

        Returns 0.0 for perfectly equal distribution, approaching 1.0 for
        maximal inequality.  Returns 0.0 for empty or single-element lists.
        """
        n = len(values)
        if n < 2:
            return 0.0
        sorted_vals = sorted(values)
        total = sum(sorted_vals)
        if total == 0:
            return 0.0

        weighted_sum = sum(
            (2 * (i + 1) - n - 1) * v for i, v in enumerate(sorted_vals)
        )
        return round(weighted_sum / (n * total), 6)

    @staticmethod
    def top_percent_share(values: list[float], pct: float = 0.1) -> float:
        """Fraction of total wealth held by the top *pct* of holders.

        ``pct=0.1`` gives the top-10 % share.  Returns 0.0 for empty input.
        """
        if not values:
            return 0.0
        sorted_desc = sorted(values, reverse=True)
        total = sum(sorted_desc)
        if total == 0:
            return 0.0
        top_n = max(1, math.ceil(len(sorted_desc) * pct))
        top_n = min(top_n, len(sorted_desc))
        return round(sum(sorted_desc[:top_n]) / total, 6)

    # -- Agent-level helpers -------------------------------------------------

    def wealth_distribution(self, agents: list[dict]) -> dict[str, Any]:
        """Analyse wealth distribution across agents.

        Each agent dict should have ``money`` and/or ``tokens`` keys.

        Returns:
        - ``money_gini`` / ``tokens_gini`` — Gini coefficients
        - ``top_10_money_share`` / ``top_10_tokens_share`` — top-10 % shares
        - ``money_mean`` / ``money_median`` / ``money_std``
        - ``tokens_mean`` / ``tokens_median`` / ``tokens_std``
        - ``total_money`` / ``total_tokens``
        - ``alive_count``
        """
        alive = [a for a in agents if a.get("alive", True)]
        if not alive:
            return self._empty_wealth()

        money = [float(a.get("money", 0)) for a in alive]
        tokens = [float(a.get("tokens", 0)) for a in alive]
        n = len(alive)

        return {
            "alive_count": n,
            "total_money": sum(money),
            "total_tokens": sum(tokens),
            "money_gini": self.gini(money),
            "tokens_gini": self.gini(tokens),
            "top_10_money_share": self.top_percent_share(money, 0.1),
            "top_10_tokens_share": self.top_percent_share(tokens, 0.1),
            "money_mean": round(sum(money) / n, 4),
            "money_median": round(self._median(money), 4),
            "money_std": round(self._std(money), 4),
            "tokens_mean": round(sum(tokens) / n, 4),
            "tokens_median": round(self._median(tokens), 4),
            "tokens_std": round(self._std(tokens), 4),
        }

    # -- Price / inflation over time -----------------------------------------

    def price_trend(
        self,
        history: list[dict],
        field: str = "total_tokens",
    ) -> dict[str, Any]:
        """Compute price / value trend from a time-ordered list of snapshots.

        Each snapshot dict should have ``tick`` and the specified *field*.

        Returns:
        - ``ticks`` — list of tick values
        - ``values`` — list of the field values
        - ``change_pct`` — total percentage change from first to last
        - ``min`` / ``max`` / ``mean``
        - ``slope`` — per-tick linear regression slope
        """
        if len(history) < 2:
            return {"ticks": [], "values": [], "change_pct": 0.0,
                    "min": 0.0, "max": 0.0, "mean": 0.0, "slope": 0.0}

        ticks = [h.get("tick", i) for i, h in enumerate(history)]
        values = [float(h.get(field, 0)) for h in history]
        n = len(values)

        first, last = values[0], values[-1]
        change_pct = ((last - first) / first * 100) if first else 0.0
        slope = self._linear_slope(ticks, values)

        return {
            "ticks": ticks,
            "values": values,
            "change_pct": round(change_pct, 4),
            "min": min(values),
            "max": max(values),
            "mean": round(sum(values) / n, 4),
            "slope": round(slope, 6),
        }

    def inflation_rate(self, history: list[dict]) -> dict[str, Any]:
        """Estimate inflation rate from money supply history.

        Each snapshot should have ``tick`` and ``total_money``.
        Computes per-interval and cumulative inflation rate.

        Returns:
        - ``per_interval`` — list of (tick, inflation_pct) tuples
        - ``cumulative`` — total inflation from first to last
        - ``annualized_equiv`` — inflation rate scaled to a standard
          "year" of 365 ticks
        """
        if len(history) < 2:
            return {"per_interval": [], "cumulative": 0.0,
                    "annualized_equiv": 0.0}

        intervals: list[dict[str, float]] = []
        for i in range(1, len(history)):
            prev = float(history[i - 1].get("total_money", 0))
            curr = float(history[i].get("total_money", 0))
            tick = history[i].get("tick", i)
            rate = ((curr - prev) / prev * 100) if prev else 0.0
            intervals.append({"tick": tick, "inflation_pct": round(rate, 4)})

        first_money = float(history[0].get("total_money", 0))
        last_money = float(history[-1].get("total_money", 0))
        cumulative = (
            ((last_money - first_money) / first_money * 100)
            if first_money else 0.0
        )

        total_ticks = history[-1].get("tick", len(history) - 1) - history[0].get("tick", 0)
        annualized = (cumulative / total_ticks * 365) if total_ticks > 0 else 0.0

        return {
            "per_interval": intervals,
            "cumulative": round(cumulative, 4),
            "annualized_equiv": round(annualized, 4),
        }

    # -- Helpers -------------------------------------------------------------

    @staticmethod
    def _empty_wealth() -> dict[str, Any]:
        return {
            "alive_count": 0,
            "total_money": 0,
            "total_tokens": 0,
            "money_gini": 0.0,
            "tokens_gini": 0.0,
            "top_10_money_share": 0.0,
            "top_10_tokens_share": 0.0,
            "money_mean": 0.0,
            "money_median": 0.0,
            "money_std": 0.0,
            "tokens_mean": 0.0,
            "tokens_median": 0.0,
            "tokens_std": 0.0,
        }

    @staticmethod
    def _median(values: list[float]) -> float:
        n = len(values)
        if n == 0:
            return 0.0
        s = sorted(values)
        mid = n // 2
        if n % 2 == 0:
            return (s[mid - 1] + s[mid]) / 2
        return s[mid]

    @staticmethod
    def _std(values: list[float]) -> float:
        n = len(values)
        if n < 2:
            return 0.0
        mean = sum(values) / n
        variance = sum((v - mean) ** 2 for v in values) / (n - 1)
        return math.sqrt(variance)

    @staticmethod
    def _linear_slope(xs: list, ys: list[float]) -> float:
        """Ordinary-least-squares slope."""
        n = len(xs)
        if n < 2:
            return 0.0
        sx = sum(xs)
        sy = sum(ys)
        sxy = sum(x * y for x, y in zip(xs, ys))
        sx2 = sum(x * x for x in xs)
        denom = n * sx2 - sx * sx
        if denom == 0:
            return 0.0
        return (n * sxy - sx * sy) / denom
