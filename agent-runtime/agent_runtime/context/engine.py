"""Context Engine Pipeline — token-budgeted, priority-driven context assembly.

Public symbols (re-exported via ``context/__init__.py``):
    ContextEnginePipeline, ContextItem, ContextPriority, ContextSource,
    MemorySource, MessageFilter, PerceptionSource, PipelineConfig,
    PipelineResult, PipelineStats, StateSource, SurvivalSource, TokenBudget
"""

from __future__ import annotations

import logging
import os
from dataclasses import dataclass, field
from enum import Enum, IntEnum
from typing import Any, Protocol, runtime_checkable

logger = logging.getLogger(__name__)

# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------

_DEFAULT_MAX_TOKENS: int = 4096
_TOKEN_CHARS_RATIO: float = 4.0  # rough chars-per-token estimate
_HP_CRITICAL_THRESHOLD: float = 30.0  # HP < 30% → survival info
_TOKEN_CRITICAL_RATIO: float = 0.20  # token ratio < 20% → survival info


# ---------------------------------------------------------------------------
# Enums
# ---------------------------------------------------------------------------


class ContextPriority(IntEnum):
    """Priority levels for context items (lower value = higher priority).

    Ordered so that numeric comparison directly reflects urgency:
    P0 (survival) > P1 (mission) > P2 (social) > P3 (exploration).
    """

    P0_SURVIVAL = 0
    P1_MISSION = 1
    P2_SOCIAL = 2
    P3_EXPLORATION = 3


class ContextSource(str, Enum):
    """Origin of a context item."""

    PERCEPTION = "perception"
    SURVIVAL = "survival"
    STATE = "state"
    MEMORY = "memory"


# ---------------------------------------------------------------------------
# Data types
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class ContextItem:
    """A single piece of context with metadata for prioritisation.

    Attributes:
        content: The text payload.
        source: Where this item came from.
        priority: Scheduling priority.
        token_estimate: Approximate token count for budget accounting.
        metadata: Extra structured data (e.g. trust_score, health).
        protected: If True the item is never trimmed even when over budget.
    """

    content: str
    source: ContextSource
    priority: ContextPriority
    token_estimate: int = 0
    metadata: dict[str, Any] = field(default_factory=dict)
    protected: bool = False

    def __post_init__(self) -> None:
        if self.token_estimate <= 0 and self.content:
            object.__setattr__(
                self,
                "token_estimate",
                max(1, int(len(self.content) / _TOKEN_CHARS_RATIO)),
            )


@dataclass(frozen=True)
class PipelineStats:
    """Statistics about a pipeline run.

    Attributes:
        total_items_collected: Items from all sources before filtering.
        total_tokens_collected: Token estimate before filtering.
        items_after_filter: Items surviving the message filter.
        tokens_after_filter: Token estimate after filtering.
        items_trimmed: Number of items removed during budget trimming.
        final_token_count: Token count in the final result.
    """

    total_items_collected: int = 0
    total_tokens_collected: int = 0
    items_after_filter: int = 0
    tokens_after_filter: int = 0
    items_trimmed: int = 0
    final_token_count: int = 0


@dataclass(frozen=True)
class PipelineResult:
    """Output of a context engine pipeline run.

    Attributes:
        items: The final, trimmed context items.
        formatted_context: All items concatenated into a single string.
        stats: Run statistics.
    """

    items: list[ContextItem]
    formatted_context: str
    stats: PipelineStats


# ---------------------------------------------------------------------------
# Token budget
# ---------------------------------------------------------------------------


@dataclass
class TokenBudget:
    """Token budget manager.

    Attributes:
        max_tokens: Hard cap on total tokens.
        reserve_ratio: Fraction of budget reserved for P0 items.
    """

    max_tokens: int = _DEFAULT_MAX_TOKENS
    reserve_ratio: float = 0.30  # 30% reserved for survival

    def allocate(self, items: list[ContextItem]) -> list[ContextItem]:
        """Trim items to fit within the token budget.

        Protected items are always kept. Remaining items are sorted by
        priority (ascending — P0 first) and included until budget is full.
        """
        protected: list[ContextItem] = []
        regular: list[ContextItem] = []

        for item in items:
            if item.protected:
                protected.append(item)
            else:
                regular.append(item)

        # Calculate tokens used by protected items
        protected_tokens = sum(i.token_estimate for i in protected)
        remaining_budget = max(0, self.max_tokens - protected_tokens)

        # Sort regular items by priority (lower = higher priority)
        regular.sort(key=lambda i: i.priority)

        accepted: list[ContextItem] = []
        used = 0
        for item in regular:
            if used + item.token_estimate <= remaining_budget:
                accepted.append(item)
                used += item.token_estimate

        trimmed_count = len(regular) - len(accepted)
        return protected + accepted, trimmed_count


# ---------------------------------------------------------------------------
# Message filter
# ---------------------------------------------------------------------------


class MessageFilter:
    """Filters and reorders context items based on priority rules.

    Rules:
        1. Survival info (HP < 30% or token ratio < 20%) is always protected.
        2. Social messages are sorted by trust_score (descending).
        3. Items are otherwise kept in source order.
    """

    def filter(self, items: list[ContextItem]) -> list[ContextItem]:
        """Apply filtering rules and return reordered items.

        Marks items as protected when they contain survival-critical
        information. Reorders social items by trust_score.
        """
        result: list[ContextItem] = []

        for item in items:
            if self._is_survival_critical(item):
                result.append(self._protect(item))
            elif item.source == ContextSource.MEMORY and item.priority == ContextPriority.P2_SOCIAL:
                result.append(item)
            else:
                result.append(item)

        # Sort social items by trust_score descending
        social = [i for i in result if i.priority == ContextPriority.P2_SOCIAL]
        non_social = [i for i in result if i.priority != ContextPriority.P2_SOCIAL]

        social.sort(
            key=lambda i: i.metadata.get("trust_score", 0.0),
            reverse=True,
        )

        # Rebuild: maintain non-social order, insert social at end
        return non_social + social

    # ------------------------------------------------------------------
    # Internal
    # ------------------------------------------------------------------

    @staticmethod
    def _is_survival_critical(item: ContextItem) -> bool:
        """Check whether an item contains survival-critical information."""
        meta = item.metadata
        if item.source == ContextSource.SURVIVAL:
            return True
        # Check for health or token ratio thresholds
        health = meta.get("health")
        if isinstance(health, (int, float)) and health < _HP_CRITICAL_THRESHOLD:
            return True
        token_ratio = meta.get("token_ratio")
        if isinstance(token_ratio, (int, float)) and token_ratio < _TOKEN_CRITICAL_RATIO:
            return True
        return False

    @staticmethod
    def _protect(item: ContextItem) -> ContextItem:
        """Return a copy of the item with protected=True."""
        return ContextItem(
            content=item.content,
            source=item.source,
            priority=item.priority,
            token_estimate=item.token_estimate,
            metadata=item.metadata,
            protected=True,
        )


# ---------------------------------------------------------------------------
# Source protocols — each source knows how to extract ContextItems
# ---------------------------------------------------------------------------


@runtime_checkable
class PerceptionSource(Protocol):
    """Extract context from a Perception (messages, market, events)."""

    def collect(self, perception: Any) -> list[ContextItem]: ...


@runtime_checkable
class SurvivalSource(Protocol):
    """Extract context from a SurvivalAction."""

    def collect(self, survival: Any) -> list[ContextItem]: ...


@runtime_checkable
class StateSource(Protocol):
    """Extract context from an AgentState."""

    def collect(self, state: Any) -> list[ContextItem]: ...


@runtime_checkable
class MemorySource(Protocol):
    """Extract context from a MemoryRecall result."""

    def collect(self, memory_recall: Any) -> list[ContextItem]: ...


# ---------------------------------------------------------------------------
# Default source implementations
# ---------------------------------------------------------------------------


class DefaultPerceptionSource:
    """Extract context items from ``core/think_loop.Perception``."""

    def collect(self, perception: Any) -> list[ContextItem]:
        items: list[ContextItem] = []

        # Messages → social / task context
        messages = getattr(perception, "messages", None) or []
        for msg in messages:
            if isinstance(msg, dict):
                content = msg.get("payload", {}).get("content", "") or str(msg)
                trust = msg.get("trust_score", 0.0)
                priority = ContextPriority.P2_SOCIAL
                # Task-related messages bump to P1
                msg_type = msg.get("type", "")
                if msg_type in ("PROPOSE", "task"):
                    priority = ContextPriority.P1_MISSION
            else:
                content = str(msg)
                trust = 0.0
                priority = ContextPriority.P2_SOCIAL

            items.append(
                ContextItem(
                    content=content,
                    source=ContextSource.PERCEPTION,
                    priority=priority,
                    metadata={"trust_score": trust},
                )
            )

        # Market state
        market = getattr(perception, "market_state", None) or {}
        if market:
            items.append(
                ContextItem(
                    content=f"Market state: {market}",
                    source=ContextSource.PERCEPTION,
                    priority=ContextPriority.P1_MISSION,
                )
            )

        # Events
        tick = getattr(perception, "tick", 0)
        health = getattr(perception, "health", 100.0)
        token_ratio = getattr(perception, "token_ratio", 1.0)

        items.append(
            ContextItem(
                content=f"Tick {tick}: token_ratio={token_ratio:.2f}, health={health:.0f}",
                source=ContextSource.PERCEPTION,
                priority=ContextPriority.P0_SURVIVAL
                if health < _HP_CRITICAL_THRESHOLD or token_ratio < _TOKEN_CRITICAL_RATIO
                else ContextPriority.P1_MISSION,
                metadata={"health": health, "token_ratio": token_ratio},
            )
        )

        return items


class DefaultSurvivalSource:
    """Extract context items from ``survival/instinct.SurvivalAction``."""

    def collect(self, survival: Any) -> list[ContextItem]:
        items: list[ContextItem] = []

        mode = getattr(survival, "mode", None)
        mode_value = mode.value if hasattr(mode, "value") else str(mode)
        token_ratio = getattr(survival, "token_ratio", 1.0)

        items.append(
            ContextItem(
                content=f"Survival mode: {mode_value}, token ratio: {token_ratio:.1%}",
                source=ContextSource.SURVIVAL,
                priority=ContextPriority.P0_SURVIVAL,
                protected=True,
                metadata={"token_ratio": token_ratio, "mode": mode_value},
            )
        )

        for action in getattr(survival, "actions", []):
            items.append(
                ContextItem(
                    content=f"Emergency action: {getattr(action, 'reason', str(action))}",
                    source=ContextSource.SURVIVAL,
                    priority=ContextPriority.P0_SURVIVAL,
                    protected=True,
                )
            )

        return items


class DefaultStateSource:
    """Extract context items from ``models/agent_state.AgentState``."""

    def collect(self, state: Any) -> list[ContextItem]:
        items: list[ContextItem] = []

        health = getattr(state, "health", 100.0)
        tokens = getattr(state, "tokens", 0)
        max_tokens = getattr(state, "max_tokens", 1)
        money = getattr(state, "money", 0.0)
        reputation = getattr(state, "reputation", 0.0)
        phase = getattr(state, "phase", None)
        phase_value = phase.value if hasattr(phase, "value") else str(phase)

        token_ratio = tokens / max_tokens if max_tokens > 0 else 0.0

        # Core state
        items.append(
            ContextItem(
                content=(
                    f"Agent state: phase={phase_value}, "
                    f"health={health:.0f}, tokens={tokens}/{max_tokens}, "
                    f"money={money:.1f}, reputation={reputation:.1f}"
                ),
                source=ContextSource.STATE,
                priority=ContextPriority.P0_SURVIVAL
                if health < _HP_CRITICAL_THRESHOLD or token_ratio < _TOKEN_CRITICAL_RATIO
                else ContextPriority.P1_MISSION,
                metadata={"health": health, "token_ratio": token_ratio},
            )
        )

        # Skills
        skills = getattr(state, "skills", {})
        if skills and isinstance(skills, dict):
            skill_lines = [f"  - {name}: level {s.level}" for name, s in skills.items()]
            items.append(
                ContextItem(
                    content="Skills:\n" + "\n".join(skill_lines),
                    source=ContextSource.STATE,
                    priority=ContextPriority.P1_MISSION,
                )
            )

        return items


class DefaultMemorySource:
    """Extract context items from ``memory/memory_recall.RecalledMemory``."""

    def collect(self, memory_recall: Any) -> list[ContextItem]:
        items: list[ContextItem] = []

        # Accept both a list of RecalledMemory and a MemoryRecall instance
        memories = memory_recall
        if not isinstance(memories, list):
            # Might be a MemoryRecall object — try to call recall_for_decision
            if hasattr(memories, "recall_for_decision"):
                memories = memories.recall_for_decision("current context")
            else:
                memories = []

        for mem in memories:
            content = getattr(mem, "content", str(mem))
            relevance = getattr(mem, "relevance", 0.0)
            importance = getattr(mem, "importance", 0.0)
            mem_type = getattr(mem, "memory_type", "unknown")

            items.append(
                ContextItem(
                    content=f"[{mem_type}] {content} (relevance={relevance:.2f})",
                    source=ContextSource.MEMORY,
                    priority=ContextPriority.P1_MISSION,
                    metadata={"relevance": relevance, "importance": importance},
                )
            )

        return items


# ---------------------------------------------------------------------------
# Pipeline configuration
# ---------------------------------------------------------------------------


@dataclass
class PipelineConfig:
    """Configuration for the context engine pipeline.

    Attributes:
        max_tokens: Token budget cap (overridden by CONTEXT_MAX_TOKENS env var).
        reserve_ratio: Fraction of budget reserved for P0 items.
    """

    max_tokens: int = _DEFAULT_MAX_TOKENS
    reserve_ratio: float = 0.30

    def __post_init__(self) -> None:
        # Allow environment variable override
        env_val = os.environ.get("CONTEXT_MAX_TOKENS")
        if env_val is not None:
            try:
                self.max_tokens = int(env_val)
            except ValueError:
                logger.warning(
                    "Invalid CONTEXT_MAX_TOKENS=%r, using default %d",
                    env_val,
                    self.max_tokens,
                )


# ---------------------------------------------------------------------------
# Pipeline
# ---------------------------------------------------------------------------


class ContextEnginePipeline:
    """Token-budgeted, priority-driven context assembly pipeline.

    Collects context from perception, survival, state, and memory sources,
    applies message filtering, then trims to a token budget.

    Usage::

        pipeline = ContextEnginePipeline()
        result = pipeline.run(perception=perception, survival=survival,
                              state=state, memory=recalled_memories)
        # result.formatted_context → ready for the LLM prompt
    """

    def __init__(
        self,
        config: PipelineConfig | None = None,
        *,
        perception_source: PerceptionSource | None = None,
        survival_source: SurvivalSource | None = None,
        state_source: StateSource | None = None,
        memory_source: MemorySource | None = None,
        message_filter: MessageFilter | None = None,
        token_budget: TokenBudget | None = None,
    ) -> None:
        self._config = config or PipelineConfig()
        self._perception = perception_source or DefaultPerceptionSource()
        self._survival = survival_source or DefaultSurvivalSource()
        self._state = state_source or DefaultStateSource()
        self._memory = memory_source or DefaultMemorySource()
        self._filter = message_filter or MessageFilter()
        self._budget = token_budget or TokenBudget(
            max_tokens=self._config.max_tokens,
            reserve_ratio=self._config.reserve_ratio,
        )

    @property
    def config(self) -> PipelineConfig:
        return self._config

    def run(
        self,
        *,
        perception: Any = None,
        survival: Any = None,
        state: Any = None,
        memory: Any = None,
    ) -> PipelineResult:
        """Execute the pipeline: collect → filter → budget-trim → format.

        Parameters
        ----------
        perception : Perception or similar
            Perception data (think_loop.Perception).
        survival : SurvivalAction or similar
            Survival assessment.
        state : AgentState or similar
            Agent state.
        memory : list[RecalledMemory] or MemoryRecall
            Memory context.

        Returns
        -------
        PipelineResult
            The trimmed context items, formatted string, and statistics.
        """
        # 1. Collect from all sources
        items: list[ContextItem] = []
        if perception is not None:
            items.extend(self._perception.collect(perception))
        if survival is not None:
            items.extend(self._survival.collect(survival))
        if state is not None:
            items.extend(self._state.collect(state))
        if memory is not None:
            items.extend(self._memory.collect(memory))

        collected_count = len(items)
        collected_tokens = sum(i.token_estimate for i in items)

        # 2. Filter
        items = self._filter.filter(items)
        filtered_count = len(items)
        filtered_tokens = sum(i.token_estimate for i in items)

        # 3. Budget trim
        items, trimmed_count = self._budget.allocate(items)
        final_tokens = sum(i.token_estimate for i in items)

        # 4. Format
        formatted = "\n\n".join(i.content for i in items)

        stats = PipelineStats(
            total_items_collected=collected_count,
            total_tokens_collected=collected_tokens,
            items_after_filter=filtered_count,
            tokens_after_filter=filtered_tokens,
            items_trimmed=trimmed_count,
            final_token_count=final_tokens,
        )

        logger.debug(
            "Context pipeline: collected=%d tokens=%d → final=%d items=%d tokens=%d trimmed=%d",
            collected_count,
            collected_tokens,
            len(items),
            final_tokens,
            stats.final_token_count,
            trimmed_count,
        )

        return PipelineResult(
            items=items,
            formatted_context=formatted,
            stats=stats,
        )
