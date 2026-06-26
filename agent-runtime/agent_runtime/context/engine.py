"""Context Engine Pipeline — token-budgeted, priority-driven context assembly.

Public symbols (re-exported via ``context/__init__.py``):
    ContextEngine, ContextEnginePipeline, ContextItem, ContextPriority,
    ContextSource, MemorySource, MessageFilter, PerceptionSource,
    PipelineConfig, PipelineResult, PipelineStats, StateSource,
    SurvivalSource, TokenBudget
"""

from __future__ import annotations

import json
import logging
from collections.abc import Callable
from dataclasses import dataclass, field
from enum import IntEnum, StrEnum
from typing import Any, Protocol, runtime_checkable

from agent_runtime.context.budget import PipelineConfig, TokenBudget
from agent_runtime.context.processors import ContextProcessor

logger = logging.getLogger(__name__)

# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------

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


class ContextSource(StrEnum):
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
        protected_overflow: True when protected items alone exceed
            ``max_tokens``.  Protected items are **never** trimmed (by
            design), so the caller must decide how to handle the overflow
            — e.g. skip the regular budget check or log an alert.
    """

    total_items_collected: int = 0
    total_tokens_collected: int = 0
    items_after_filter: int = 0
    tokens_after_filter: int = 0
    items_trimmed: int = 0
    final_token_count: int = 0
    protected_overflow: bool = False


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


# TokenBudget and PipelineConfig are imported from budget.py above and
# re-exported for backward compatibility.  Importers can use either:
#   from agent_runtime.context.engine import TokenBudget
#   from agent_runtime.context.budget import TokenBudget


# ---------------------------------------------------------------------------
# Message filter
# ---------------------------------------------------------------------------


class MessageFilter:
    """Filters and reorders context items based on priority rules.

    Rules:
        1. Survival info (HP < 30% or token ratio < 20%) is always protected.
        2. Social messages are sorted by trust_score (descending).
        3. Items are otherwise kept in source order.

    **Social items ordering:** All ``P2_SOCIAL`` items are moved to the
    end of the output list (after every non-social item) and sorted by
    ``trust_score`` in descending order within that tail section.
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
                    content=f"Market state: {json.dumps(market, ensure_ascii=False)}",
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
        mode_value = mode.value if mode is not None and hasattr(mode, "value") else str(mode)
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
        phase_value = phase.value if phase is not None and hasattr(phase, "value") else str(phase)

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
    """Extract context items from ``memory/memory_recall.RecalledMemory``.

    Args:
        query_builder: Optional callable that returns the query string
            passed to ``recall_for_decision()``.  Defaults to
            ``"current context"``.  Override to build dynamic queries
            based on tick, task, or other context at integration time.
    """

    def __init__(self, query_builder: Callable[[], str] | None = None) -> None:
        self._query_builder = query_builder

    def collect(self, memory_recall: Any) -> list[ContextItem]:
        items: list[ContextItem] = []

        # Accept both a list of RecalledMemory and a MemoryRecall instance
        memories = memory_recall
        if not isinstance(memories, list):
            # Might be a MemoryRecall object — try to call recall_for_decision
            if hasattr(memories, "recall_for_decision"):
                query = self._query_builder() if self._query_builder else "current context"
                memories = memories.recall_for_decision(query)
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
# Pipeline configuration — re-exported from budget.py
# ---------------------------------------------------------------------------


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
        context_processor: ContextProcessor | None = None,
    ) -> None:
        self._config = config or PipelineConfig()
        self._perception = perception_source or DefaultPerceptionSource()
        self._survival = survival_source or DefaultSurvivalSource()
        self._state = state_source or DefaultStateSource()
        self._memory = memory_source or DefaultMemorySource()
        self._filter = message_filter or MessageFilter()
        self._processor = context_processor or ContextProcessor()
        self._budget = token_budget or TokenBudget(
            max_tokens=self._config.max_tokens,
            safety_margin=self._config.safety_margin,
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
        rank_query: str = "",
        current_tick: int = 0,
    ) -> PipelineResult:
        """Execute the pipeline: collect → filter → rank → budget-trim → format.

        When all inputs are ``None``, returns an empty ``PipelineResult``
        (``formatted_context=""``).  Callers should guard with
        ``if result.formatted_context:`` before injecting the context into
        a prompt.

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
        rank_query : str
            Query string for keyword-based relevance ranking of items.
            When empty, the ranking stage is skipped (items keep filter order).
        current_tick : int
            Current tick for time-decay ranking.  Defaults to 0.

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

        # 3. Rank by relevance (keyword + time decay)
        if rank_query:
            items = self._processor.process(items, rank_query, current_tick)

        # 4. Budget trim
        items, trimmed_count, protected_overflow = self._budget.allocate(items)
        final_tokens = sum(i.token_estimate for i in items)

        # 5. Format
        formatted = "\n\n".join(i.content for i in items)

        stats = PipelineStats(
            total_items_collected=collected_count,
            total_tokens_collected=collected_tokens,
            items_after_filter=filtered_count,
            tokens_after_filter=filtered_tokens,
            items_trimmed=trimmed_count,
            final_token_count=final_tokens,
            protected_overflow=protected_overflow,
        )

        logger.debug(
            "Context pipeline: collected=%d tokens=%d → final=%d items, %d tokens, trimmed=%d",
            collected_count,
            collected_tokens,
            len(items),
            final_tokens,
            trimmed_count,
        )

        return PipelineResult(
            items=items,
            formatted_context=formatted,
            stats=stats,
        )


# ---------------------------------------------------------------------------
# ContextEngine — high-level interface matching the issue spec
# ---------------------------------------------------------------------------


class ContextEngine:
    """Three-stage context processing: filter → rank → budget-truncate.

    This is the primary integration point described in the architecture
    spec.  It wraps ``ContextEnginePipeline`` and exposes a single
    ``build_context()`` method that ``decide.py`` can call instead of the
    old hardcoded ``build_prompt()``.

    Stage 1 — **Collect**: gather candidate context from perception,
    survival state, agent state, and memory.
    Stage 2 — **Rank**: sort items by relevance (keyword matching +
    time decay, no vector DB).
    Stage 3 — **Truncate**: enforce the token budget with a 100-token
    safety margin.

    Usage::

        engine = ContextEngine(token_budget=2000)
        context = engine.build_context(
            agent_state=state,
            perception=perception,
            working_memory=working_memory,
            short_term_memory=short_term_memory,
        )
        # context is a string ready for the LLM prompt
    """

    def __init__(
        self,
        token_budget: int = 2000,
        safety_margin: int = 100,
        *,
        pipeline: ContextEnginePipeline | None = None,
    ) -> None:
        if pipeline is not None:
            self._pipeline = pipeline
        else:
            config = PipelineConfig(max_tokens=token_budget, safety_margin=safety_margin)
            self._pipeline = ContextEnginePipeline(config=config)

    @property
    def pipeline(self) -> ContextEnginePipeline:
        """Underlying pipeline instance."""
        return self._pipeline

    def build_context(
        self,
        agent_state: Any = None,
        perception: Any = None,
        working_memory: Any = None,
        short_term_memory: Any = None,
        survival: Any = None,
        memory: Any = None,
    ) -> str:
        """Build token-budgeted context for the decision prompt.

        Parameters
        ----------
        agent_state : AgentState or similar
            Current agent state (health, tokens, skills, etc.).
        perception : Perception or similar
            Current tick perception (messages, market, events).
        working_memory : WorkingMemory or list
            Working memory entries.  If a ``WorkingMemory`` instance is
            provided, ``read_all()`` is called automatically.
        short_term_memory : list or None
            Optional short-term memory entries to inject.
        survival : SurvivalAction or similar
            Survival assessment.
        memory : list[RecalledMemory] or MemoryRecall
            Recalled memories from the memory subsystem.

        Returns
        -------
        str
            Formatted, token-budgeted context string for the LLM prompt.
        """
        # Normalize working_memory: accept WorkingMemory objects
        memory_input = memory
        if memory_input is None:
            memory_items: list = []

            # Collect from working memory
            if working_memory is not None:
                entries = (
                    working_memory.read_all()
                    if hasattr(working_memory, "read_all")
                    else list(working_memory)
                )
                memory_items.extend(entries)

            # Collect from short-term memory
            if short_term_memory is not None:
                if isinstance(short_term_memory, list):
                    memory_items.extend(short_term_memory)
                elif hasattr(short_term_memory, "search"):
                    # ShortTermMemory-like object — grab recent entries
                    memory_items.extend(short_term_memory.search("", top_k=5, tick=0))

            if memory_items:
                memory_input = memory_items

        result = self._pipeline.run(
            perception=perception,
            survival=survival,
            state=agent_state,
            memory=memory_input,
        )

        if result.stats.protected_overflow:
            logger.warning(
                "ContextEngine: protected items overflow budget (%d tokens > %d cap)",
                result.stats.final_token_count,
                self._pipeline.config.max_tokens,
            )

        return result.formatted_context
