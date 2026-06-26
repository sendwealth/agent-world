"""Behaviour branch tests using the AgentMockLLM mock decision provider.

Verifies that agents make the correct decisions under different context
conditions (starvation, social proximity, default) when the LLM is
replaced by the deterministic mock.

These tests are *unit-level* (no subprocess, no World Engine) — they
exercise the ThinkLoop + AgentMockLLM integration directly, which is
fast and deterministic.
"""

from __future__ import annotations

from typing import Any

import pytest

from agent_runtime.core.act import ActionExecutor, ActionType
from agent_runtime.core.think_loop import (
    Perception,
    ThinkLoop,
    ThinkLoopConfig,
)
from agent_runtime.models.agent_state import AgentState
from agent_runtime.survival.instinct import SurvivalInstinct
from tests.e2e.mocks.mock_llm import (
    AgentMockLLM,
    ConditionTrigger,
    KeywordTrigger,
    MockContext,
    hungry_gather_mock,
    social_nearby_mock,
    survival_behaviour_mock,
)


# ── Helpers ──────────────────────────────────────────────────────


def _make_state(
    *,
    tokens: int = 500,
    max_tokens: int = 1000,
    health: float = 100.0,
    name: str = "TestAgent",
) -> AgentState:
    """Create an AgentState with explicit token/health values."""
    return AgentState(
        name=name,
        tokens=tokens,
        max_tokens=max_tokens,
        health=health,
    )


def _make_perception(
    *,
    tick: int = 1,
    nearby_agents: list[Any] | None = None,
    market_state: dict[str, Any] | None = None,
    messages: list[dict[str, Any]] | None = None,
) -> Perception:
    """Create a Perception with optional nearby agents / market state."""
    ms: dict[str, Any] = market_state or {}
    if nearby_agents is not None:
        ms["nearby_agents"] = nearby_agents
    return Perception(
        tick=tick,
        messages=messages or [],
        market_state=ms,
    )


# ── Test: AgentMockLLM core API ─────────────────────────────────


class TestAgentMockLLMCore:
    """Unit tests for the AgentMockLLM registration and matching logic."""

    @pytest.mark.asyncio
    async def test_default_action_when_no_trigger_matches(self) -> None:
        """When no trigger matches, the default action is returned."""
        mock = AgentMockLLM(default_action=ActionType.REST)
        state = _make_state(tokens=500)
        perception = _make_perception()
        survival = SurvivalInstinct().assess(state)

        decision = await mock.decide(state, perception, survival)
        assert decision.action_type == ActionType.REST
        assert mock.call_count == 1

    @pytest.mark.asyncio
    async def test_keyword_trigger_matches(self) -> None:
        """KeywordTrigger fires when a keyword is found in the context."""
        mock = AgentMockLLM()
        mock.set_response(
            trigger=KeywordTrigger("task"),
            action_type=ActionType.CLAIM_TASK,
            reasoning="Found task — claiming",
        )

        state = _make_state()
        # market_state contains "task" key → should match
        perception = _make_perception(market_state={"available_tasks": ["build-house"]})
        survival = SurvivalInstinct().assess(state)

        decision = await mock.decide(state, perception, survival)
        assert decision.action_type == ActionType.CLAIM_TASK

    @pytest.mark.asyncio
    async def test_keyword_trigger_no_match(self) -> None:
        """KeywordTrigger does NOT fire when keyword is absent."""
        mock = AgentMockLLM()
        mock.set_response(
            trigger=KeywordTrigger("task"),
            action_type=ActionType.CLAIM_TASK,
        )

        state = _make_state()
        perception = _make_perception(market_state={"empty": True})
        survival = SurvivalInstinct().assess(state)

        decision = await mock.decide(state, perception, survival)
        assert decision.action_type == ActionType.REST  # default

    @pytest.mark.asyncio
    async def test_condition_trigger_starving(self) -> None:
        """ConditionTrigger fires when token_ratio < 0.2 (starving)."""
        mock = AgentMockLLM()
        mock.set_response(
            trigger=ConditionTrigger(lambda ctx: ctx.token_ratio < 0.2),
            action_type=ActionType.GATHER,
            reasoning="Starving — gathering",
        )

        # tokens=100 out of max_tokens=1000 → ratio=0.1 (starving)
        state = _make_state(tokens=100, max_tokens=1000)
        perception = _make_perception()
        survival = SurvivalInstinct().assess(state)

        decision = await mock.decide(state, perception, survival)
        assert decision.action_type == ActionType.GATHER
        assert "Starving" in decision.reasoning

    @pytest.mark.asyncio
    async def test_condition_trigger_not_starving(self) -> None:
        """ConditionTrigger does NOT fire when tokens are healthy."""
        mock = AgentMockLLM()
        mock.set_response(
            trigger=ConditionTrigger(lambda ctx: ctx.token_ratio < 0.2),
            action_type=ActionType.GATHER,
        )

        # tokens=500 out of max_tokens=1000 → ratio=0.5 (healthy)
        state = _make_state(tokens=500, max_tokens=1000)
        perception = _make_perception()
        survival = SurvivalInstinct().assess(state)

        decision = await mock.decide(state, perception, survival)
        assert decision.action_type == ActionType.REST  # default

    @pytest.mark.asyncio
    async def test_condition_trigger_nearby_agents(self) -> None:
        """ConditionTrigger fires when nearby agents are present."""
        mock = AgentMockLLM()
        mock.set_response(
            trigger=ConditionTrigger(lambda ctx: ctx.has_nearby_agents),
            action_type=ActionType.SEND_MESSAGE,
            reasoning="Social — greeting",
        )

        state = _make_state()
        perception = _make_perception(nearby_agents=[{"name": "Alice"}])
        survival = SurvivalInstinct().assess(state)

        decision = await mock.decide(state, perception, survival)
        assert decision.action_type == ActionType.SEND_MESSAGE

    @pytest.mark.asyncio
    async def test_priority_ordering(self) -> None:
        """Higher-priority triggers are checked first."""
        mock = AgentMockLLM()

        # Low priority: nearby → socialise
        mock.set_response(
            trigger=ConditionTrigger(lambda ctx: True),  # always matches
            action_type=ActionType.SEND_MESSAGE,
            reasoning="Low priority — socialise",
            priority=1,
        )
        # High priority: always → gather
        mock.set_response(
            trigger=ConditionTrigger(lambda ctx: True),  # always matches
            action_type=ActionType.GATHER,
            reasoning="High priority — gather",
            priority=10,
        )

        state = _make_state()
        perception = _make_perception()
        survival = SurvivalInstinct().assess(state)

        decision = await mock.decide(state, perception, survival)
        assert decision.action_type == ActionType.GATHER  # high priority wins

    @pytest.mark.asyncio
    async def test_call_history_tracking(self) -> None:
        """Mock tracks call count and decision history."""
        mock = AgentMockLLM()
        state = _make_state()
        perception = _make_perception()
        survival = SurvivalInstinct().assess(state)

        assert mock.call_count == 0
        assert mock.last_decision is None

        await mock.decide(state, perception, survival)
        await mock.decide(state, perception, survival)

        assert mock.call_count == 2
        assert mock.last_decision is not None
        assert len(mock.decision_history) == 2

    @pytest.mark.asyncio
    async def test_reset_history(self) -> None:
        """reset_history() clears all tracking state."""
        mock = AgentMockLLM()
        state = _make_state()
        perception = _make_perception()
        survival = SurvivalInstinct().assess(state)

        await mock.decide(state, perception, survival)
        assert mock.call_count == 1

        mock.reset_history()
        assert mock.call_count == 0
        assert mock.last_decision is None
        assert mock.decision_history == []

    @pytest.mark.asyncio
    async def test_trigger_error_does_not_crash(self) -> None:
        """A trigger that raises an exception is skipped, not propagated."""
        mock = AgentMockLLM()
        mock.set_response(
            trigger=ConditionTrigger(lambda ctx: 1 / 0),  # raises ZeroDivisionError
            action_type=ActionType.GATHER,
        )
        mock.set_response(
            trigger=ConditionTrigger(lambda ctx: True),  # always matches
            action_type=ActionType.EXPLORE,
        )

        state = _make_state()
        perception = _make_perception()
        survival = SurvivalInstinct().assess(state)

        decision = await mock.decide(state, perception, survival)
        # The first trigger errored, second matched
        assert decision.action_type == ActionType.EXPLORE


# ── Test: ThinkLoop integration ──────────────────────────────────


class TestThinkLoopWithMock:
    """Verify AgentMockLLM works correctly inside ThinkLoop."""

    @pytest.mark.asyncio
    async def test_starving_agent_gathers(self) -> None:
        """An agent with low tokens (conservative mode) should decide to GATHER.

        Note: very low tokens trigger the survival instinct's emergency
        bypass (panic/urgent), which skips the decision provider entirely.
        We use tokens in the conservative range (20-40%) so the normal
        decision provider is called.
        """
        mock = AgentMockLLM()
        mock.set_response(
            trigger=ConditionTrigger(lambda ctx: ctx.token_ratio < 0.4),
            action_type=ActionType.GATHER,
            reasoning="Low on resources — gathering",
            priority=10,
        )
        state = _make_state(tokens=300, max_tokens=1000)  # ratio=0.3 → conservative

        loop = ThinkLoop(
            state=state,
            survival=SurvivalInstinct(),
            executor=ActionExecutor(),
            config=ThinkLoopConfig(max_ticks=3, tick_interval=0.01),
            decision_provider=mock,
        )

        await loop.run()
        assert mock.call_count >= 1
        # All decisions should be GATHER (because ratio < 0.4)
        for decision in mock.decision_history:
            assert decision.action_type == ActionType.GATHER

    @pytest.mark.asyncio
    async def test_healthy_agent_with_mock_rests(self) -> None:
        """A healthy agent with no triggers matched should rest (default)."""
        mock = AgentMockLLM()
        mock.set_response(
            trigger=ConditionTrigger(lambda ctx: ctx.token_ratio < 0.4),
            action_type=ActionType.GATHER,
        )
        state = _make_state(tokens=800, max_tokens=1000)  # ratio=0.8 → invest

        loop = ThinkLoop(
            state=state,
            survival=SurvivalInstinct(),
            executor=ActionExecutor(),
            config=ThinkLoopConfig(max_ticks=3, tick_interval=0.01),
            decision_provider=mock,
        )

        await loop.run()
        assert mock.call_count >= 1
        for decision in mock.decision_history:
            assert decision.action_type == ActionType.REST

    @pytest.mark.asyncio
    async def test_social_agent_sends_message(self) -> None:
        """An agent with nearby neighbours should socialise."""
        mock = social_nearby_mock()
        state = _make_state(tokens=500)

        # Create a perception provider that always returns nearby agents
        class NearbyPerceptionProvider:
            async def perceive(self, state: AgentState, tick: int) -> Perception:
                return _make_perception(
                    tick=tick,
                    nearby_agents=[{"name": "Alice"}, {"name": "Bob"}],
                )

        loop = ThinkLoop(
            state=state,
            survival=SurvivalInstinct(),
            executor=ActionExecutor(),
            config=ThinkLoopConfig(max_ticks=3, tick_interval=0.01),
            decision_provider=mock,
            perception_provider=NearbyPerceptionProvider(),
        )

        await loop.run()
        assert mock.call_count >= 1
        for decision in mock.decision_history:
            assert decision.action_type == ActionType.SEND_MESSAGE

    @pytest.mark.asyncio
    async def test_survival_mock_branches_correctly(self) -> None:
        """The survival_behaviour_mock routes correctly by urgency.

        Note: panic/urgent modes bypass the decision provider via
        emergency actions. We test conservative mode which still
        goes through the normal decision flow.
        """
        mock = survival_behaviour_mock()

        # Phase 1: Conservative agent (ratio ~0.3) → should GATHER
        state = _make_state(tokens=300, max_tokens=1000)  # ratio=0.3
        loop = ThinkLoop(
            state=state,
            survival=SurvivalInstinct(),
            executor=ActionExecutor(),
            config=ThinkLoopConfig(max_ticks=2, tick_interval=0.01),
            decision_provider=mock,
        )
        await loop.run()
        assert mock.decision_history[-1].action_type == ActionType.GATHER

        mock.reset_history()

        # Phase 2: Healthy agent with no context → should REST
        state = _make_state(tokens=700, max_tokens=1000)  # ratio=0.7
        loop = ThinkLoop(
            state=state,
            survival=SurvivalInstinct(),
            executor=ActionExecutor(),
            config=ThinkLoopConfig(max_ticks=2, tick_interval=0.01),
            decision_provider=mock,
        )
        await loop.run()
        assert mock.decision_history[-1].action_type == ActionType.REST

    @pytest.mark.asyncio
    async def test_custom_mock_with_parameters(self) -> None:
        """Custom triggers can carry action parameters."""
        mock = AgentMockLLM()
        mock.set_response(
            trigger=KeywordTrigger("explore"),
            action_type=ActionType.EXPLORE,
            reasoning="Exploring the world",
            parameters={"direction": "north"},
        )

        state = _make_state()
        perception = _make_perception(
            market_state={"event": "explore opportunity nearby"}
        )
        survival = SurvivalInstinct().assess(state)

        decision = await mock.decide(state, perception, survival)
        assert decision.action_type == ActionType.EXPLORE
        assert decision.parameters == {"direction": "north"}


# ── Test: MockContext convenience properties ─────────────────────


class TestMockContext:
    """Verify MockContext properties computed correctly."""

    def test_token_ratio(self) -> None:
        state = _make_state(tokens=200, max_tokens=1000)
        perception = _make_perception()
        survival = SurvivalInstinct().assess(state)

        ctx = MockContext(state=state, perception=perception, survival=survival)
        assert ctx.token_ratio == pytest.approx(0.2)

    def test_health(self) -> None:
        state = _make_state(health=75.0)
        perception = _make_perception()
        survival = SurvivalInstinct().assess(state)

        ctx = MockContext(state=state, perception=perception, survival=survival)
        assert ctx.health == 75.0

    def test_has_nearby_agents(self) -> None:
        state = _make_state()
        perception = _make_perception(nearby_agents=[{"name": "Alice"}])
        survival = SurvivalInstinct().assess(state)

        ctx = MockContext(state=state, perception=perception, survival=survival)
        assert ctx.has_nearby_agents is True
        assert len(ctx.nearby_agents) == 1

    def test_no_nearby_agents(self) -> None:
        state = _make_state()
        perception = _make_perception()
        survival = SurvivalInstinct().assess(state)

        ctx = MockContext(state=state, perception=perception, survival=survival)
        assert ctx.has_nearby_agents is False

    def test_survival_mode(self) -> None:
        state = _make_state(tokens=50, max_tokens=1000)  # ratio=0.05 → panic
        perception = _make_perception()
        survival = SurvivalInstinct().assess(state)

        ctx = MockContext(state=state, perception=perception, survival=survival)
        assert ctx.survival_mode == "panic"


# ── Test: Factory helpers ────────────────────────────────────────


class TestFactoryHelpers:
    """Verify pre-built mock factories."""

    def test_hungry_gather_mock_has_one_trigger(self) -> None:
        mock = hungry_gather_mock()
        assert mock.registered_count == 1

    def test_social_nearby_mock_has_one_trigger(self) -> None:
        mock = social_nearby_mock()
        assert mock.registered_count == 1

    def test_survival_mock_has_four_triggers(self) -> None:
        mock = survival_behaviour_mock()
        assert mock.registered_count == 4

    @pytest.mark.asyncio
    async def test_hungry_gather_mock_starving(self) -> None:
        mock = hungry_gather_mock()
        state = _make_state(tokens=50, max_tokens=1000)
        perception = _make_perception()
        survival = SurvivalInstinct().assess(state)

        decision = await mock.decide(state, perception, survival)
        assert decision.action_type == ActionType.GATHER

    @pytest.mark.asyncio
    async def test_hungry_gather_mock_healthy(self) -> None:
        mock = hungry_gather_mock()
        state = _make_state(tokens=800, max_tokens=1000)
        perception = _make_perception()
        survival = SurvivalInstinct().assess(state)

        decision = await mock.decide(state, perception, survival)
        assert decision.action_type == ActionType.REST

    @pytest.mark.asyncio
    async def test_social_nearby_mock_with_agents(self) -> None:
        mock = social_nearby_mock()
        state = _make_state(tokens=500)
        perception = _make_perception(nearby_agents=[{"name": "Alice"}])
        survival = SurvivalInstinct().assess(state)

        decision = await mock.decide(state, perception, survival)
        assert decision.action_type == ActionType.SEND_MESSAGE

    @pytest.mark.asyncio
    async def test_social_nearby_mock_alone(self) -> None:
        mock = social_nearby_mock()
        state = _make_state(tokens=500)
        perception = _make_perception()
        survival = SurvivalInstinct().assess(state)

        decision = await mock.decide(state, perception, survival)
        assert decision.action_type == ActionType.EXPLORE
