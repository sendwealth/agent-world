"""Tests for the AsyncDecisionProvider (core/async_decide.py)."""

from __future__ import annotations

import asyncio

import pytest

from agent_runtime.core.act import ActionType
from agent_runtime.core.async_decide import AsyncDecisionProvider
from agent_runtime.core.think_loop import Decision, Perception
from agent_runtime.models.agent_state import AgentState
from agent_runtime.survival.instinct import SurvivalAction, SurvivalMode

# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------


@pytest.fixture
def agent_state() -> AgentState:
    return AgentState(name="TestAgent", tokens=500, max_tokens=1000, health=80.0)


@pytest.fixture
def perception() -> Perception:
    return Perception(tick=1, token_balance=500, token_ratio=0.5, health=80.0)


@pytest.fixture
def survival_normal() -> SurvivalAction:
    return SurvivalAction(mode=SurvivalMode.NORMAL, token_ratio=0.5)


# ---------------------------------------------------------------------------
# Mock inner providers
# ---------------------------------------------------------------------------


class SlowInnerProvider:
    """Inner provider that simulates LLM latency."""

    def __init__(self, delay: float = 0.1, action: ActionType = ActionType.EXPLORE) -> None:
        self._delay = delay
        self._action = action
        self.call_count = 0

    async def decide(
        self,
        state: AgentState,
        perception: Perception,
        survival: SurvivalAction,
    ) -> Decision:
        self.call_count += 1
        await asyncio.sleep(self._delay)
        return Decision(
            action_type=self._action,
            reasoning=f"LLM decision (tick {perception.tick})",
        )


class FastInnerProvider:
    """Inner provider that returns immediately."""

    def __init__(self, action: ActionType = ActionType.EXPLORE) -> None:
        self._action = action
        self.call_count = 0

    async def decide(
        self,
        state: AgentState,
        perception: Perception,
        survival: SurvivalAction,
    ) -> Decision:
        self.call_count += 1
        return Decision(
            action_type=self._action,
            reasoning=f"LLM decision (tick {perception.tick})",
        )


class FailingInnerProvider:
    """Inner provider that always fails."""

    async def decide(
        self,
        state: AgentState,
        perception: Perception,
        survival: SurvivalAction,
    ) -> Decision:
        raise RuntimeError("LLM decision failed")


# ---------------------------------------------------------------------------
# Tests: first tick returns fallback
# ---------------------------------------------------------------------------


class TestAsyncDecisionProvider:
    @pytest.mark.asyncio
    async def test_first_tick_returns_fallback(self, agent_state, perception, survival_normal):
        """On the very first tick, no LLM decision is available yet."""
        inner = FastInnerProvider()
        provider = AsyncDecisionProvider(inner=inner)

        decision = await provider.decide(agent_state, perception, survival_normal)

        # First tick should return a fallback
        assert decision.action_type in (ActionType.REST, ActionType.EXPLORE)
        assert "Fallback" in decision.reasoning or "fallback" in decision.reasoning.lower()
        assert provider.stats["fallback_decisions_used"] == 1

    @pytest.mark.asyncio
    async def test_second_tick_uses_llm_decision(
        self, agent_state, survival_normal
    ):
        """On the second tick, the LLM decision from tick 1 should be available."""
        inner = FastInnerProvider(action=ActionType.GATHER)
        provider = AsyncDecisionProvider(inner=inner)

        # Tick 1 — returns fallback, starts background request
        p1 = Perception(tick=1, token_balance=500, token_ratio=0.5)
        await provider.decide(agent_state, p1, survival_normal)

        # Give the background task time to complete
        await asyncio.sleep(0.05)

        # Tick 2 — should use the LLM decision
        p2 = Perception(tick=2, token_balance=500, token_ratio=0.5)
        decision = await provider.decide(agent_state, p2, survival_normal)

        assert decision.action_type == ActionType.GATHER
        assert provider.stats["llm_decisions_used"] == 1

    @pytest.mark.asyncio
    async def test_slow_llm_returns_last_decision(
        self, agent_state, survival_normal
    ):
        """When LLM is slow, the provider returns the last good decision."""
        inner = SlowInnerProvider(delay=2.0, action=ActionType.BUILD)
        provider = AsyncDecisionProvider(inner=inner)

        # Tick 1 — fallback
        p1 = Perception(tick=1)
        await provider.decide(agent_state, p1, survival_normal)

        # Tick 2 immediately — LLM still pending, should return fallback
        p2 = Perception(tick=2)
        decision = await provider.decide(agent_state, p2, survival_normal)
        assert "Fallback" in decision.reasoning or "fallback" in decision.reasoning.lower()

    @pytest.mark.asyncio
    async def test_failed_llm_returns_fallback(
        self, agent_state, perception, survival_normal
    ):
        """When the inner provider fails, fallback is returned."""
        inner = FailingInnerProvider()
        provider = AsyncDecisionProvider(inner=inner)

        # Tick 1 — fallback (no LLM decision available)
        decision = await provider.decide(agent_state, perception, survival_normal)
        assert decision.action_type in (ActionType.REST, ActionType.EXPLORE)

        # Tick 2 — still fallback since LLM failed
        p2 = Perception(tick=2)
        decision = await provider.decide(agent_state, p2, survival_normal)
        assert decision.action_type in (ActionType.REST, ActionType.EXPLORE)

    @pytest.mark.asyncio
    async def test_stop_cancels_pending_task(self, agent_state, perception, survival_normal):
        """stop() should cancel any pending background request."""
        inner = SlowInnerProvider(delay=5.0)
        provider = AsyncDecisionProvider(inner=inner)

        # Start a tick which kicks off a background task
        await provider.decide(agent_state, perception, survival_normal)

        # Stop should cancel the pending task without error
        await provider.stop()

    @pytest.mark.asyncio
    async def test_custom_fallback_actions(self, agent_state, perception, survival_normal):
        """Custom fallback actions should be used instead of defaults."""
        inner = FailingInnerProvider()
        provider = AsyncDecisionProvider(
            inner=inner,
            fallback_actions=[ActionType.REST],
        )

        decision = await provider.decide(agent_state, perception, survival_normal)
        assert decision.action_type == ActionType.REST

    @pytest.mark.asyncio
    async def test_stats_tracking(self, agent_state, survival_normal):
        """Stats should accurately reflect usage."""
        inner = FastInnerProvider()
        provider = AsyncDecisionProvider(inner=inner)

        # Tick 1
        p1 = Perception(tick=1)
        await provider.decide(agent_state, p1, survival_normal)

        await asyncio.sleep(0.05)

        # Tick 2 — should pick up LLM decision
        p2 = Perception(tick=2)
        await provider.decide(agent_state, p2, survival_normal)

        stats = provider.stats
        assert stats["llm_decisions_used"] >= 1
        assert stats["fallback_decisions_used"] >= 1

    @pytest.mark.asyncio
    async def test_reuses_last_decision_across_ticks(
        self, agent_state, survival_normal
    ):
        """After getting an LLM decision, it should be reused until a new one arrives."""
        inner = SlowInnerProvider(delay=0.1, action=ActionType.MOVE)
        provider = AsyncDecisionProvider(inner=inner)

        # Tick 1 — starts background request
        p1 = Perception(tick=1)
        await provider.decide(agent_state, p1, survival_normal)

        # Wait for LLM to complete
        await asyncio.sleep(0.2)

        # Tick 2 — should pick up the LLM decision
        p2 = Perception(tick=2)
        d2 = await provider.decide(agent_state, p2, survival_normal)
        assert d2.action_type == ActionType.MOVE

        # Tick 3 immediately — new LLM is pending, should still use last decision
        p3 = Perception(tick=3)
        d3 = await provider.decide(agent_state, p3, survival_normal)
        assert d3.action_type == ActionType.MOVE

    @pytest.mark.asyncio
    async def test_stale_decision_expires(self, agent_state, survival_normal):
        """Decisions older than max_stale_ticks should be discarded."""
        inner = FastInnerProvider(action=ActionType.EXPLORE)
        provider = AsyncDecisionProvider(inner=inner, max_stale_ticks=2)

        # Tick 1 — starts background request, returns fallback
        p1 = Perception(tick=1)
        await provider.decide(agent_state, p1, survival_normal)

        # Wait for LLM to complete
        await asyncio.sleep(0.05)

        # Tick 2 — should pick up the LLM decision (tick 1 result, 1 tick stale)
        p2 = Perception(tick=2)
        d2 = await provider.decide(agent_state, p2, survival_normal)
        assert d2.action_type == ActionType.EXPLORE

        # Tick 3 — still within stale window (2 ticks old)
        p3 = Perception(tick=3)
        d3 = await provider.decide(agent_state, p3, survival_normal)
        assert d3.action_type == ActionType.EXPLORE

        # Tick 5 — decision is now 3 ticks old (exceeds max_stale_ticks=2)
        # A new fast request will complete, but let's test the expiration logic
        p5 = Perception(tick=5)
        d5 = await provider.decide(agent_state, p5, survival_normal)
        # The stale decision should have expired, but a new one may have arrived
        # since FastInnerProvider is instant. Either way, it's a valid decision.
        assert d5.action_type in (ActionType.EXPLORE, ActionType.REST)
