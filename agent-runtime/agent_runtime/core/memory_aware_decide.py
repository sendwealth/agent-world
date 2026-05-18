"""Memory-aware decision provider — enhances decide step with recalled memories.

Wraps a ``DecisionProvider`` to inject relevant past memories into the
decision process.  This is the integration point between the vector
memory system and the think loop's decide phase.

Usage::

    from agent_runtime.core.memory_aware_decide import MemoryAwareDecisionProvider
    from agent_runtime.memory.vector_memory import VectorMemory
    from agent_runtime.memory.memory_recall import MemoryRecall

    vm = VectorMemory()
    recall = MemoryRecall(vector_memory=vm)
    provider = MemoryAwareDecisionProvider(
        base_provider=my_decision_provider,
        memory_recall=recall,
    )

    # Use in ThinkLoop
    loop = ThinkLoop(..., decision_provider=provider)
"""

from __future__ import annotations

import logging
from typing import Any, Protocol

from agent_runtime.core.think_loop import (
    Decision,
    Perception,
)
from agent_runtime.models.agent_state import AgentState
from agent_runtime.survival.instinct import SurvivalAction

logger = logging.getLogger(__name__)


# ---------------------------------------------------------------------------
# Protocol for the memory recall dependency
# ---------------------------------------------------------------------------


class MemoryRecaller(Protocol):
    """Interface for the memory recall component."""

    def build_context(
        self,
        query: str,
        *,
        situation: str = "",
    ) -> str: ...


class InnerDecisionProvider(Protocol):
    """Interface for the wrapped decision provider."""

    async def decide(
        self,
        state: AgentState,
        perception: Perception,
        survival: SurvivalAction,
    ) -> Decision: ...


# ---------------------------------------------------------------------------
# MemoryAwareDecisionProvider
# ---------------------------------------------------------------------------


class MemoryAwareDecisionProvider:
    """Decision provider that augments decisions with recalled memories.

    During each ``decide`` call:
    1. Builds a memory query from the current perception and survival state.
    2. Recalls relevant memories from the vector store.
    3. Attaches memory context to the decision reasoning.

    This provider wraps any existing ``DecisionProvider`` (LLM-based, mock,
    etc.) and enhances its output without modifying the inner provider.

    Parameters
    ----------
    base_provider : InnerDecisionProvider
        The underlying decision provider to wrap.
    memory_recall : MemoryRecaller
        The memory recall component for retrieving relevant memories.
    """

    def __init__(
        self,
        base_provider: Any,
        memory_recall: MemoryRecaller,
    ) -> None:
        self._base = base_provider
        self._recall = memory_recall

    async def decide(
        self,
        state: AgentState,
        perception: Perception,
        survival: SurvivalAction,
    ) -> Decision:
        """Produce a decision enhanced with recalled memories.

        Recalls memories relevant to the current situation, then delegates
        to the base decision provider.  The recalled memory context is
        appended to the decision reasoning for transparency.
        """
        # Build query from current context
        query = self._build_query(state, perception, survival)
        situation = (
            f"tokens={state.tokens}, health={state.health:.0f}, "
            f"mode={survival.mode.value}"
        )

        # Recall relevant memories
        memory_context = self._recall.build_context(query, situation=situation)

        # Delegate to base provider
        decision = await self._base.decide(state, perception, survival)

        # If we have memory context, enhance the decision reasoning
        if memory_context and decision.reasoning:
            enhanced_reasoning = (
                f"{decision.reasoning}\n\n"
                f"[Memory context used]\n{memory_context}"
            )
            # Decision is frozen, create a new one with enhanced reasoning
            return Decision(
                action_type=decision.action_type,
                parameters=decision.parameters,
                reasoning=enhanced_reasoning,
            )

        return decision

    def _build_query(
        self,
        state: AgentState,
        perception: Perception,
        survival: SurvivalAction,
    ) -> str:
        """Build a memory recall query from the current context."""
        parts: list[str] = []

        # Survival context
        if survival.mode.value in ("panic", "urgent"):
            parts.append(f"emergency survival {survival.mode.value}")
        else:
            parts.append(f"survival mode {survival.mode.value}")

        # Resource context
        if state.tokens < 100:
            parts.append("low tokens resource scarcity")
        if state.health < 30:
            parts.append("critical health danger")

        # Perception context
        if perception.messages:
            parts.append("responding to messages communication")
        if perception.active_task:
            parts.append(f"working on task {perception.active_task}")

        return " ".join(parts) if parts else "agent decision making"
