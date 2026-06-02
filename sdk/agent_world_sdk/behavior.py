"""Behavioral pattern analysis sub-module — strategies, survival, activity.

These helpers operate on data dicts returned by the SDK or loaded from
exported files.  They do not make HTTP requests.
"""

from __future__ import annotations

from collections import Counter, defaultdict
from typing import Any


class BehaviorModule:
    """Behavioral pattern analysis functions for Agent World data."""

    # -- Survival analysis ---------------------------------------------------

    @staticmethod
    def survival_stats(agents: list[dict]) -> dict[str, Any]:
        """Compute survival statistics from a list of agent profiles.

        Each agent dict should have ``alive`` and ``ticks_survived`` keys.

        Returns:
        - ``alive_count`` / ``dead_count`` / ``total``
        - ``survival_rate`` — alive / total
        - ``mean_ticks_survived`` — across all agents
        - ``median_ticks_survived``
        - ``max_ticks_survived``
        - ``by_phase`` — survival breakdown by lifecycle phase
        """
        total = len(agents)
        if total == 0:
            return _empty_survival()

        alive_list = [a for a in agents if a.get("alive", True)]
        dead_list = [a for a in agents if not a.get("alive", True)]
        alive_count = len(alive_list)
        dead_count = len(dead_list)

        all_ticks = [a.get("ticks_survived", 0) for a in agents]
        mean_ticks = sum(all_ticks) / total
        sorted_ticks = sorted(all_ticks)
        median_ticks = _median(sorted_ticks)

        by_phase: dict[str, dict[str, Any]] = {}
        phase_counter: Counter[str] = Counter()
        phase_alive: Counter[str] = Counter()
        for a in agents:
            phase = str(a.get("phase", "unknown"))
            phase_counter[phase] += 1
            if a.get("alive", True):
                phase_alive[phase] += 1
        for phase in sorted(phase_counter):
            by_phase[phase] = {
                "count": phase_counter[phase],
                "alive": phase_alive[phase],
                "dead": phase_counter[phase] - phase_alive[phase],
                "survival_rate": round(
                    phase_alive[phase] / max(phase_counter[phase], 1), 4
                ),
            }

        return {
            "total": total,
            "alive_count": alive_count,
            "dead_count": dead_count,
            "survival_rate": round(alive_count / total, 4),
            "mean_ticks_survived": round(mean_ticks, 2),
            "median_ticks_survived": round(median_ticks, 2),
            "max_ticks_survived": max(all_ticks) if all_ticks else 0,
            "by_phase": by_phase,
        }

    # -- Strategy / activity profiling ---------------------------------------

    @staticmethod
    def activity_profile(
        behavior_log: list[dict],
    ) -> dict[str, Any]:
        """Build a per-agent activity profile from behavior log entries.

        Each entry should have ``agent_id`` and ``event_type`` keys.

        Returns:
        - ``profiles`` — ``{agent_id: {"event_counts": {...}, "total_events", "dominant_action"}}``
        - ``action_distribution`` — global event_type frequency
        - ``top_agents`` — top-10 most active agents by total event count
        """
        agent_events: dict[str, Counter[str]] = defaultdict(Counter)
        global_events: Counter[str] = Counter()

        for entry in behavior_log:
            aid = str(entry.get("agent_id", ""))
            etype = str(entry.get("event_type", "unknown"))
            if not aid:
                continue
            agent_events[aid][etype] += 1
            global_events[etype] += 1

        profiles: dict[str, dict[str, Any]] = {}
        for aid, counts in agent_events.items():
            total = sum(counts.values())
            dominant = counts.most_common(1)[0][0] if counts else "none"
            profiles[aid] = {
                "event_counts": dict(counts),
                "total_events": total,
                "dominant_action": dominant,
            }

        top_agents = sorted(
            profiles.items(), key=lambda x: x[1]["total_events"], reverse=True
        )[:10]

        return {
            "profiles": profiles,
            "action_distribution": dict(global_events.most_common()),
            "top_agents": [
                {"agent_id": aid, **info} for aid, info in top_agents
            ],
        }

    @staticmethod
    def strategy_classification(
        behavior_log: list[dict],
    ) -> dict[str, Any]:
        """Classify agents into behavioural archetypes from their event logs.

        Uses event-type heuristics:
        - **trader** — predominantly trade / transaction events
        - **social** — predominantly message / communication events
        - **builder** — predominantly build / construction events
        - **leader** — predominantly governance / organization events
        - **survivor** — predominantly gather / rest / exploration events
        - **mixed** — no single category exceeds 40 %

        Returns:
        - ``agent_strategies`` — ``{agent_id: strategy}``
        - ``strategy_distribution`` — ``{strategy: count}``
        """
        CATEGORY_KEYWORDS: dict[str, list[str]] = {
            "trader": ["trade", "transaction", "purchase", "marketplace", "stock", "investment"],
            "social": ["message", "feed", "comment", "trust", "communicate"],
            "builder": ["build", "construct", "maintain", "upgrade"],
            "leader": ["governance", "proposal", "vote", "org", "leader", "treaty"],
            "survivor": ["gather", "rest", "explore", "move"],
        }
        THRESHOLD = 0.4

        agent_events: dict[str, Counter[str]] = defaultdict(Counter)
        for entry in behavior_log:
            aid = str(entry.get("agent_id", ""))
            etype = str(entry.get("event_type", "unknown")).lower()
            if aid:
                agent_events[aid][etype] += 1

        agent_strategies: dict[str, str] = {}
        strategy_dist: Counter[str] = Counter()

        for aid, counts in agent_events.items():
            total = sum(counts.values())
            if total == 0:
                strategy = "mixed"
            else:
                best_cat = "mixed"
                best_frac = 0.0
                for cat, keywords in CATEGORY_KEYWORDS.items():
                    cat_count = sum(
                        c for etype, c in counts.items()
                        if any(kw in etype for kw in keywords)
                    )
                    frac = cat_count / total
                    if frac > best_frac:
                        best_frac = frac
                        best_cat = cat
                strategy = best_cat if best_frac >= THRESHOLD else "mixed"
            agent_strategies[aid] = strategy
            strategy_dist[strategy] += 1

        return {
            "agent_strategies": agent_strategies,
            "strategy_distribution": dict(strategy_dist),
        }

    # -- Temporal analysis ---------------------------------------------------

    @staticmethod
    def activity_over_ticks(
        behavior_log: list[dict],
        *,
        tick_bucket_size: int = 10,
    ) -> list[dict[str, Any]]:
        """Aggregate event counts per tick bucket.

        Returns a list of ``{"tick_range", "count", "top_event_type"}``
        sorted by tick range ascending.
        """
        if not behavior_log:
            return []

        bucket_counts: dict[int, Counter[str]] = defaultdict(Counter)
        for entry in behavior_log:
            tick = entry.get("tick", 0)
            etype = str(entry.get("event_type", "unknown"))
            bucket = tick // tick_bucket_size * tick_bucket_size
            bucket_counts[bucket][etype] += 1

        result: list[dict[str, Any]] = []
        for bucket in sorted(bucket_counts):
            counts = bucket_counts[bucket]
            total = sum(counts.values())
            top = counts.most_common(1)[0][0] if counts else "none"
            result.append({
                "tick_range": f"{bucket}-{bucket + tick_bucket_size - 1}",
                "count": total,
                "top_event_type": top,
            })
        return result


# -- Helpers ----------------------------------------------------------------

def _empty_survival() -> dict[str, Any]:
    return {
        "total": 0,
        "alive_count": 0,
        "dead_count": 0,
        "survival_rate": 0.0,
        "mean_ticks_survived": 0.0,
        "median_ticks_survived": 0.0,
        "max_ticks_survived": 0,
        "by_phase": {},
    }


def _median(sorted_vals: list) -> float:
    n = len(sorted_vals)
    if n == 0:
        return 0.0
    mid = n // 2
    if n % 2 == 0:
        return (sorted_vals[mid - 1] + sorted_vals[mid]) / 2
    return float(sorted_vals[mid])
