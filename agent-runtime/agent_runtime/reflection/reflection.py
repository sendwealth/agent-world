"""Reflection layer — periodic self-assessment and strategy adjustment.

Triggered every N ticks (default 10). Evaluates recent action outcomes,
computes success rate and token efficiency per action type, updates strategy
preferences, and writes key decisions and lessons to long-term memory.
"""

from __future__ import annotations

import asyncio
import logging
import time
from collections.abc import Sequence
from dataclasses import dataclass
from typing import Protocol, runtime_checkable

from agent_runtime.reflection.memory import LongTermMemory, MemoryCategory, MemoryEntry
from agent_runtime.reflection.strategy import StrategyRegistry

logger = logging.getLogger(__name__)


# ---------------------------------------------------------------------------
# Action status constants
# ---------------------------------------------------------------------------

class ActionStatus:
    """Known action outcome status values."""
    SUCCESS = "success"
    FAILED = "failed"


# ---------------------------------------------------------------------------
# Data structures
# ---------------------------------------------------------------------------

@dataclass(frozen=True)
class ActionTypeStats:
    """Statistics for a single action type within a reflection window."""

    action_type: str
    total: int = 0
    successes: int = 0
    failures: int = 0
    tokens_spent: int = 0
    rewards: float = 0.0

    @property
    def success_rate(self) -> float:
        if self.total == 0:
            return 0.0
        return self.successes / self.total

    @property
    def token_efficiency(self) -> float:
        if self.tokens_spent == 0:
            return 0.0
        return self.rewards / self.tokens_spent


@dataclass
class _StatBucket:
    """Mutable accumulator used during stat aggregation."""

    total: int = 0
    successes: int = 0
    failures: int = 0
    tokens_spent: int = 0
    rewards: float = 0.0


@dataclass(frozen=True)
class ReflectionResult:
    """Outcome of a single reflection cycle."""

    tick: int
    total_actions_evaluated: int
    overall_success_rate: float
    overall_token_efficiency: float
    action_stats: list[ActionTypeStats]
    strategy_changes: list[str]
    memories_stored: int
    top_actions: list[tuple[str, float]]
    reflected_at: float = 0.0

    def __post_init__(self) -> None:
        if self.reflected_at == 0.0:
            object.__setattr__(self, "reflected_at", time.time())


# ---------------------------------------------------------------------------
# Action outcome protocol
# ---------------------------------------------------------------------------

@runtime_checkable
class ActionOutcome(Protocol):
    """Minimal interface for an action result that reflection can evaluate."""

    @property
    def action_type(self) -> str: ...

    @property
    def status(self) -> str: ...

    @property
    def token_cost(self) -> int: ...


# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------

@dataclass
class ReflectionConfig:
    """Configuration for the reflection layer."""

    interval: int = 10  # Trigger every N ticks
    min_actions_for_reflection: int = 1  # Minimum actions needed to reflect
    importance_threshold: float = 0.6  # Min importance for auto-storing memories
    decay_factor: float = 0.95  # Strategy preference decay per reflection
    max_memory_per_reflection: int = 5  # Max memories stored per cycle
    window_size: int = 0  # How many recent actions to evaluate (0 = all)
    top_n_actions: int = 5  # Number of top actions to report


# ---------------------------------------------------------------------------
# Core ReflectionLayer
# ---------------------------------------------------------------------------

class ReflectionLayer:
    """Periodic self-assessment engine.

    Called by the think loop every `config.interval` ticks. Aggregates
    recent action outcomes, updates strategy preferences, and writes
    key insights to long-term memory.
    """

    def __init__(
        self,
        strategy: StrategyRegistry,
        memory: LongTermMemory,
        *,
        config: ReflectionConfig | None = None,
    ) -> None:
        self._strategy = strategy
        self._memory = memory
        self._config = config or ReflectionConfig()
        self._last_reflection_tick = 0
        self._reflection_count = 0

    @property
    def config(self) -> ReflectionConfig:
        return self._config

    @property
    def last_reflection_tick(self) -> int:
        return self._last_reflection_tick

    @property
    def reflection_count(self) -> int:
        return self._reflection_count

    def should_reflect(self, current_tick: int) -> bool:
        """Determine if a reflection should occur at this tick."""
        if self._config.interval <= 0:
            return False
        ticks_since_last = current_tick - self._last_reflection_tick
        return ticks_since_last >= self._config.interval

    def reflect(
        self,
        current_tick: int,
        action_history: Sequence[ActionOutcome],
    ) -> ReflectionResult | None:
        """Execute a reflection cycle.

        Args:
            current_tick: The current tick number.
            action_history: Recent action outcomes to evaluate.

        Returns:
            ReflectionResult if reflection occurred, None if skipped.
        """
        if not self.should_reflect(current_tick):
            return None

        if len(action_history) < self._config.min_actions_for_reflection:
            logger.debug(
                "Skipping reflection at tick %d: only %d actions (min %d)",
                current_tick,
                len(action_history),
                self._config.min_actions_for_reflection,
            )
            return None

        # Filter to window
        window = self._filter_window(action_history)

        # Aggregate stats per action type
        action_stats = self._compute_stats(window)

        # Update strategy preferences
        strategy_changes = self._update_strategies(action_stats)

        # Apply global decay using config's decay_factor
        self._strategy.apply_global_decay(self._config.decay_factor)

        # Persist strategy to disk
        self._strategy.save()

        # Write to long-term memory
        memories_stored = self._write_memories(
            current_tick, action_stats, strategy_changes
        )

        # Compute overall metrics
        total_actions = sum(s.total for s in action_stats)
        total_successes = sum(s.successes for s in action_stats)
        total_tokens = sum(s.tokens_spent for s in action_stats)
        total_rewards = sum(s.rewards for s in action_stats)

        overall_success_rate = total_successes / total_actions if total_actions > 0 else 0.0
        overall_efficiency = total_rewards / total_tokens if total_tokens > 0 else 0.0

        result = ReflectionResult(
            tick=current_tick,
            total_actions_evaluated=total_actions,
            overall_success_rate=overall_success_rate,
            overall_token_efficiency=overall_efficiency,
            action_stats=action_stats,
            strategy_changes=strategy_changes,
            memories_stored=memories_stored,
            top_actions=self._strategy.top_actions(self._config.top_n_actions),
        )

        self._last_reflection_tick = current_tick
        self._reflection_count += 1

        logger.info(
            "Reflection #%d at tick %d: %d actions, %.1f%% success, %.3f token efficiency, "
            "%d strategy changes, %d memories stored",
            self._reflection_count,
            current_tick,
            total_actions,
            overall_success_rate * 100,
            overall_efficiency,
            len(strategy_changes),
            memories_stored,
        )

        return result

    def _filter_window(
        self, action_history: Sequence[ActionOutcome],
    ) -> Sequence[ActionOutcome]:
        """Filter action history to the configured window size.

        When ``window_size > 0``, only the last N actions by position
        (not by tick) are returned. When ``window_size == 0`` (default),
        all actions in *action_history* are evaluated.
        """
        if self._config.window_size > 0:
            cutoff = max(0, len(action_history) - self._config.window_size)
            return action_history[cutoff:]
        return action_history

    def _compute_stats(self, actions: Sequence[ActionOutcome]) -> list[ActionTypeStats]:
        """Aggregate per-action-type statistics from action history."""
        buckets: dict[str, _StatBucket] = {}

        for action in actions:
            at = action.action_type
            if at not in buckets:
                buckets[at] = _StatBucket()
            bucket = buckets[at]
            bucket.total += 1
            bucket.tokens_spent += action.token_cost

            if action.status == ActionStatus.SUCCESS:
                bucket.successes += 1
                bucket.rewards += max(1.0, 10.0 - action.token_cost * 0.5)
            else:
                bucket.failures += 1

        return [
            ActionTypeStats(
                action_type=at,
                total=b.total,
                successes=b.successes,
                failures=b.failures,
                tokens_spent=b.tokens_spent,
                rewards=b.rewards,
            )
            for at, b in buckets.items()
        ]

    def _update_strategies(
        self, action_stats: list[ActionTypeStats]
    ) -> list[str]:
        """Update strategy preferences based on computed stats. Returns changes made."""
        changes: list[str] = []

        for stats in action_stats:
            old_pref = self._strategy.get(stats.action_type)
            old_weight = old_pref.adjusted_weight

            # Update all outcomes in this window
            for _ in range(stats.successes):
                reward = stats.token_efficiency if stats.token_efficiency > 0 else 0.5
                self._strategy.update_from_reflection(
                    stats.action_type,
                    success=True,
                    tokens_spent=stats.tokens_spent // max(stats.successes, 1),
                    reward=reward,
                )
            for _ in range(stats.failures):
                self._strategy.update_from_reflection(
                    stats.action_type,
                    success=False,
                    tokens_spent=stats.tokens_spent // max(stats.failures, 1),
                )

            new_pref = self._strategy.get(stats.action_type)
            new_weight = new_pref.adjusted_weight

            if abs(new_weight - old_weight) > 0.05:
                direction = "up" if new_weight > old_weight else "down"
                changes.append(
                    f"{stats.action_type}: {old_weight:.2f} -> {new_weight:.2f} ({direction})"
                )

        return changes

    def _write_memories(
        self,
        current_tick: int,
        action_stats: list[ActionTypeStats],
        strategy_changes: list[str],
    ) -> int:
        """Write important insights to long-term memory. Returns count stored."""
        entries: list[MemoryEntry] = []
        stored = 0

        # Store a reflection summary
        total_actions = sum(s.total for s in action_stats)
        overall_rate = (
            sum(s.successes for s in action_stats) / total_actions
            if total_actions > 0
            else 0.0
        )
        entries.append(
            MemoryEntry(
                category=MemoryCategory.REFLECTION,
                content=(
                    f"Tick {current_tick}: Evaluated {total_actions} actions. "
                    f"Overall success rate: {overall_rate:.1%}. "
                    f"{len(strategy_changes)} strategy adjustments."
                ),
                tick=current_tick,
                importance=min(0.8, 0.4 + overall_rate * 0.4),
                metadata={"overall_success_rate": overall_rate, "total_actions": total_actions},
            )
        )

        # Store significant strategy changes as decisions
        for change in strategy_changes:
            entries.append(
                MemoryEntry(
                    category=MemoryCategory.STRATEGY_CHANGE,
                    content=f"Tick {current_tick}: Strategy weight adjusted — {change}",
                    tick=current_tick,
                    importance=0.7,
                    metadata={"change": change},
                )
            )

        # Store lessons from action types with low success rates
        for stats in action_stats:
            if stats.total >= 3 and stats.success_rate < 0.3:
                entries.append(
                    MemoryEntry(
                        category=MemoryCategory.LESSON,
                        content=(
                            f"Tick {current_tick}: {stats.action_type} has low success rate "
                            f"({stats.success_rate:.0%}) — consider reducing usage"
                        ),
                        tick=current_tick,
                        importance=0.8,
                        metadata={
                            "action_type": stats.action_type,
                            "success_rate": stats.success_rate,
                        },
                    )
                )

        # Store high-efficiency action types as positive lessons
        for stats in action_stats:
            if stats.total >= 3 and stats.token_efficiency > 0.5:
                entries.append(
                    MemoryEntry(
                        category=MemoryCategory.LESSON,
                        content=(
                            f"Tick {current_tick}: {stats.action_type} is token-efficient "
                            f"(efficiency: {stats.token_efficiency:.2f}) — consider prioritizing"
                        ),
                        tick=current_tick,
                        importance=0.6,
                        metadata={
                            "action_type": stats.action_type,
                            "efficiency": stats.token_efficiency,
                        },
                    )
                )

        # Store up to max_memory_per_reflection entries, prioritized by importance
        entries.sort(key=lambda e: e.importance, reverse=True)
        for entry in entries[: self._config.max_memory_per_reflection]:
            if entry.importance >= self._config.importance_threshold:
                self._memory.store(entry)
                stored += 1

        return stored

    # -------------------------------------------------------------------
    # Provider interface for ThinkLoop integration
    # -------------------------------------------------------------------

    async def reflect_async(
        self,
        current_tick: int,
        action_history: Sequence[ActionOutcome],
    ) -> ReflectionResult | None:
        """Async wrapper for reflect() — matches the ReflectionProvider protocol.

        Offloads the synchronous reflect() call (which includes SQLite I/O)
        to a thread so it does not block the event loop.
        """
        return await asyncio.to_thread(self.reflect, current_tick, action_history)
