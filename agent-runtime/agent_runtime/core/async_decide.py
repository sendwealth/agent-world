"""Async decision provider — non-blocking LLM decisions for the think loop.

Wraps the synchronous ``LLMDecisionProvider`` (or any ``DecisionProvider``)
so that the world tick never blocks waiting for an LLM response.

Strategy:
- On each ``decide()`` call, check if a pending LLM request has completed.
  - If yes, use the fresh LLM decision for this tick and start a new
    background request.
  - If no (still waiting), use the **last known good decision** (or a
    fallback) so the tick proceeds immediately, and keep waiting for the
    LLM response to arrive in the background.
- The background LLM call runs as an ``asyncio.Task`` managed internally.

This provider satisfies the ``DecisionProvider`` protocol expected by
:class:`ThinkLoop`, so it can be used as a drop-in replacement.

Usage::

    from agent_runtime.core.async_decide import AsyncDecisionProvider
    from agent_runtime.core.llm_decide import LLMDecisionProvider
    from agent_runtime.llm.queue import LLMQueue

    queue = LLMQueue(provider=llm, config=QueueConfig(max_concurrency=2))
    llm_provider = LLMDecisionProvider(llm_provider=llm)
    async_provider = AsyncDecisionProvider(
        inner=llm_provider,
        fallback_actions=[ActionType.REST, ActionType.EXPLORE],
    )

    # Use in ThinkLoop — tick is never blocked by LLM latency
    loop = ThinkLoop(..., decision_provider=async_provider)
"""

from __future__ import annotations

import asyncio
import logging
import random
from typing import Any, Protocol

from agent_runtime.core.act import ActionType
from agent_runtime.core.think_loop import Decision, Perception
from agent_runtime.models.agent_state import AgentState
from agent_runtime.survival.instinct import SurvivalAction

logger = logging.getLogger(__name__)


# ---------------------------------------------------------------------------
# Protocol for the wrapped provider
# ---------------------------------------------------------------------------


class _InnerProvider(Protocol):
    async def decide(
        self,
        state: AgentState,
        perception: Perception,
        survival: SurvivalAction,
    ) -> Decision: ...


# ---------------------------------------------------------------------------
# AsyncDecisionProvider
# ---------------------------------------------------------------------------


class AsyncDecisionProvider:
    """Non-blocking decision provider that decouples world ticks from LLM latency.

    On each ``decide()`` call:
    1. If a background LLM request has completed since last tick, use its result
       and immediately kick off a new background request.
    2. If the background request is still pending, return the last known good
       decision (or a fallback) without waiting.
    3. If no background request exists yet (first tick), start one and return
       a fallback immediately.

    This guarantees that ``decide()`` never blocks the tick loop on the LLM.
    """

    def __init__(
        self,
        inner: Any,
        fallback_actions: list[ActionType] | None = None,
    ) -> None:
        self._inner = inner
        self._fallback_actions = fallback_actions or [ActionType.REST, ActionType.EXPLORE]

        # Internal state — accessed only from the asyncio event loop thread
        self._pending_task: asyncio.Task[Decision] | None = None
        self._last_decision: Decision | None = None
        self._last_state: AgentState | None = None
        self._last_perception: Perception | None = None
        self._last_survival: SurvivalAction | None = None

        # Stats
        self._llm_decisions_used: int = 0
        self._fallback_decisions_used: int = 0

    # ------------------------------------------------------------------
    # DecisionProvider protocol
    # ------------------------------------------------------------------

    async def decide(
        self,
        state: AgentState,
        perception: Perception,
        survival: SurvivalAction,
    ) -> Decision:
        """Produce a decision without blocking on LLM latency.

        - First tick: start a background LLM request, return fallback.
        - Subsequent ticks: if the LLM response is ready, use it and start
          a new request.  Otherwise use the last good decision / fallback.
        """
        # Save latest context for the next background request
        self._last_state = state
        self._last_perception = perception
        self._last_survival = survival

        # Check if a pending task has completed
        if self._pending_task is not None and self._pending_task.done():
            try:
                fresh_decision = self._pending_task.result()
                self._last_decision = fresh_decision
                self._llm_decisions_used += 1
                logger.debug(
                    "AsyncDecide: LLM decision ready for agent %s: %s",
                    state.id,
                    fresh_decision.action_type.value,
                )
            except Exception:
                logger.debug(
                    "AsyncDecide: background LLM call failed for agent %s",
                    state.id,
                    exc_info=True,
                )
            finally:
                self._pending_task = None

        # Start a new background request if none is pending
        if self._pending_task is None:
            self._start_background_request(state, perception, survival)

        # Return the best available decision
        if self._last_decision is not None:
            return self._last_decision

        # No LLM decision yet — return fallback
        self._fallback_decisions_used += 1
        return self._make_fallback(state)

    # ------------------------------------------------------------------
    # Lifecycle
    # ------------------------------------------------------------------

    async def stop(self) -> None:
        """Cancel any pending background request (call during shutdown)."""
        if self._pending_task is not None and not self._pending_task.done():
            self._pending_task.cancel()
            try:
                await self._pending_task
            except asyncio.CancelledError:
                pass
            self._pending_task = None

    # ------------------------------------------------------------------
    # Stats
    # ------------------------------------------------------------------

    @property
    def stats(self) -> dict[str, int]:
        """Return usage statistics."""
        return {
            "llm_decisions_used": self._llm_decisions_used,
            "fallback_decisions_used": self._fallback_decisions_used,
        }

    # ------------------------------------------------------------------
    # Internal
    # ------------------------------------------------------------------

    def _start_background_request(
        self,
        state: AgentState,
        perception: Perception,
        survival: SurvivalAction,
    ) -> None:
        """Kick off a background LLM decision request."""
        loop = asyncio.get_running_loop()
        self._pending_task = loop.create_task(
            self._inner.decide(state, perception, survival)
        )

        def _on_done(task: asyncio.Task[Decision]) -> None:
            """Callback: log exceptions from the background task."""
            if task.cancelled():
                return
            if task.exception() is not None:
                logger.debug(
                    "AsyncDecide: background task error: %s",
                    task.exception(),
                )

        self._pending_task.add_done_callback(_on_done)

    def _make_fallback(self, state: AgentState) -> Decision:
        """Return a simple fallback decision."""
        affordable = [
            a for a in self._fallback_actions
            if a == ActionType.REST or state.tokens > 0
        ]
        if not affordable:
            affordable = [ActionType.REST]

        chosen = random.choice(affordable)
        return Decision(
            action_type=chosen,
            reasoning="Fallback: waiting for LLM response",
        )
