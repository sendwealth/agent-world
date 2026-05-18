"""Strategy preference system — tracks and adjusts action preferences based on reflection."""

from __future__ import annotations

import json
import logging
from dataclasses import asdict, dataclass, field
from pathlib import Path
from typing import Any

logger = logging.getLogger(__name__)


@dataclass
class StrategyPreference:
    """A single strategy preference for an action type.

    Tracks weight (higher = preferred), success/failure counts,
    and token efficiency to guide future action selection.
    """

    action_type: str
    weight: float = 1.0
    success_count: int = 0
    failure_count: int = 0
    total_tokens_spent: int = 0
    total_rewards: float = 0.0

    @property
    def success_rate(self) -> float:
        """Calculate success rate (0.0 to 1.0)."""
        total = self.success_count + self.failure_count
        if total == 0:
            return 0.5  # neutral prior
        return self.success_count / total

    @property
    def token_efficiency(self) -> float:
        """Calculate reward per token spent."""
        if self.total_tokens_spent == 0:
            return 0.0
        return self.total_rewards / self.total_tokens_spent

    @property
    def adjusted_weight(self) -> float:
        """Weight adjusted by success rate and token efficiency.

        Formula: base_weight * (0.5 + 0.3 * success_rate + 0.2 * min(efficiency, 1.0))
        Ensures weight is in [0.2, 2.0] range.
        """
        efficiency_factor = min(self.token_efficiency, 1.0)
        raw = self.weight * (0.5 + 0.3 * self.success_rate + 0.2 * efficiency_factor)
        return max(0.2, min(2.0, raw))

    def record_success(self, tokens_spent: int, reward: float = 0.0) -> None:
        """Record a successful action."""
        self.success_count += 1
        self.total_tokens_spent += tokens_spent
        self.total_rewards += reward

    def record_failure(self, tokens_spent: int) -> None:
        """Record a failed action."""
        self.failure_count += 1
        self.total_tokens_spent += tokens_spent

    def decay(self, factor: float = 0.95) -> None:
        """Apply decay to reduce weight over time (prevents stale preferences)."""
        self.weight *= factor


class StrategyRegistry:
    """Manages strategy preferences across all action types.

    Preferences are updated during reflection and persist across sessions.
    """

    def __init__(self, storage_path: Path | None = None) -> None:
        self._preferences: dict[str, StrategyPreference] = {}
        self._storage_path = storage_path
        self._global_decay_factor = 0.95
        self._weight_boost_on_success = 0.1
        self._weight_penalty_on_failure = 0.15

        if storage_path is not None:
            self._load(storage_path)

    def get(self, action_type: str) -> StrategyPreference:
        """Get or create preference for an action type."""
        if action_type not in self._preferences:
            self._preferences[action_type] = StrategyPreference(action_type=action_type)
        return self._preferences[action_type]

    def all_preferences(self) -> dict[str, StrategyPreference]:
        """Return a copy of all preferences."""
        return dict(self._preferences)

    def update_from_reflection(
        self,
        action_type: str,
        success: bool,
        tokens_spent: int,
        reward: float = 0.0,
    ) -> StrategyPreference:
        """Update a preference based on an action outcome during reflection."""
        pref = self.get(action_type)

        if success:
            pref.record_success(tokens_spent, reward)
            pref.weight = min(
                2.0, pref.weight + self._weight_boost_on_success * pref.success_rate
            )
        else:
            pref.record_failure(tokens_spent)
            pref.weight = max(
                0.2, pref.weight - self._weight_penalty_on_failure * (1 - pref.success_rate)
            )

        return pref

    def apply_global_decay(self, factor: float | None = None) -> None:
        """Apply decay to all preferences. Called during each reflection cycle."""
        decay = factor if factor is not None else self._global_decay_factor
        for pref in self._preferences.values():
            pref.decay(decay)

    def top_actions(self, n: int = 5) -> list[tuple[str, float]]:
        """Return the top N action types ranked by adjusted weight."""
        ranked = sorted(
            self._preferences.items(),
            key=lambda item: item[1].adjusted_weight,
            reverse=True,
        )
        return [(action_type, pref.adjusted_weight) for action_type, pref in ranked[:n]]

    def summary(self) -> dict[str, dict[str, Any]]:
        """Return a summary dict of all preferences for logging/reporting."""
        return {
            action_type: {
                "weight": round(pref.adjusted_weight, 3),
                "success_rate": round(pref.success_rate, 3),
                "token_efficiency": round(pref.token_efficiency, 3),
                "success_count": pref.success_count,
                "failure_count": pref.failure_count,
            }
            for action_type, pref in self._preferences.items()
        }

    def save(self, path: Path | None = None) -> None:
        """Persist preferences to disk as JSON."""
        target = path or self._storage_path
        if target is None:
            return

        data = {
            action_type: asdict(pref)
            for action_type, pref in self._preferences.items()
        }
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(json.dumps(data, indent=2, ensure_ascii=False))
        logger.debug("Saved strategy preferences to %s", target)

    def _load(self, path: Path) -> None:
        """Load preferences from disk."""
        if not path.exists():
            return

        try:
            data = json.loads(path.read_text())
            for action_type, pref_data in data.items():
                self._preferences[action_type] = StrategyPreference(
                    action_type=pref_data["action_type"],
                    weight=pref_data.get("weight", 1.0),
                    success_count=pref_data.get("success_count", 0),
                    failure_count=pref_data.get("failure_count", 0),
                    total_tokens_spent=pref_data.get("total_tokens_spent", 0),
                    total_rewards=pref_data.get("total_rewards", 0.0),
                )
            logger.debug("Loaded %d strategy preferences from %s", len(self._preferences), path)
        except (json.JSONDecodeError, KeyError) as exc:
            logger.warning("Failed to load preferences from %s: %s", path, exc)
