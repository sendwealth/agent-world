"""End-to-end integration tests for social context flowing through the think loop.

Covers the full pipeline:
  DefaultSocialContextProvider -> ThinkLoop -> LLMDecisionProvider -> DecisionEngine
  -> prompt (with social context) -> LLM -> SOCIALIZE decision -> ActionExecutor

These tests verify that the wiring from issue SEN-559 actually works end-to-end:
  1. Social modules (trust, cultural diffusion, imitation, language) are imported
     and used at runtime through DefaultSocialContextProvider
  2. SocialContext (trust scores, cultural norms, personality description) appears
     in the LLM prompt
  3. The think loop produces SOCIALIZE actions when social context recommends it
  4. The full think loop runs without errors with a social provider attached
"""

from __future__ import annotations

from typing import Any
from unittest.mock import AsyncMock

import pytest

from agent_runtime.core.act import ActionExecutor, ActionType
from agent_runtime.core.decide import (
    DecisionAction,
    DecisionEngine,
)
from agent_runtime.core.llm_decide import LLMDecisionProvider
from agent_runtime.core.think_loop import (
    ThinkLoop,
    ThinkLoopConfig,
)
from agent_runtime.llm.base import LLMResponse, TokenUsage
from agent_runtime.models.agent_state import AgentState
from agent_runtime.models.enums import AgentPhase
from agent_runtime.models.personality import PersonalityVector
from agent_runtime.models.values import ValueWeights
from agent_runtime.social.engine import SocialEngine
from agent_runtime.social.provider import (
    AgentProfile,
    DefaultSocialContextProvider,
)
from agent_runtime.survival.instinct import SurvivalInstinct

# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------


def _make_personality(**overrides):
    defaults = dict(
        openness=0.6,
        conscientiousness=0.5,
        extraversion=0.8,
        agreeableness=0.7,
        neuroticism=0.2,
        risk_tolerance=0.5,
        social_orientation=0.8,
        greed=0.3,
    )
    defaults.update(overrides)
    return PersonalityVector(**defaults)


def _make_state(**overrides):
    defaults = dict(
        name="SocialAgent",
        tokens=500,
        max_tokens=1000,
        health=100.0,
        reputation=10.0,
        phase=AgentPhase.ADULT,
    )
    defaults.update(overrides)
    return AgentState(**defaults)


class _MockWorldClient:
    """Mock world client supporting all action types."""

    async def send_message(self, payload: dict[str, Any]) -> dict[str, Any]:
        return {"status": "ok", "action": "send_message"}

    async def claim_task(self, task_id: str) -> dict[str, Any]:
        return {"status": "ok", "action": "claim_task"}

    async def submit_task(self, task_id: str, result: dict[str, Any]) -> dict[str, Any]:
        return {"status": "ok", "action": "submit_task"}

    async def propose_deal(self, proposal: dict[str, Any]) -> dict[str, Any]:
        return {"status": "ok", "action": "propose_deal"}

    async def teach_skill(
        self, target_agent_id: str, skill_name: str, level: int
    ) -> dict[str, Any]:
        return {"status": "ok", "action": "teach_skill"}

    async def explore(self, parameters: dict[str, Any]) -> dict[str, Any]:
        return {"status": "ok", "action": "explore"}

    async def move(self, direction: str) -> dict[str, Any]:
        return {"status": "ok", "action": "move"}

    async def gather(self, resource_type: str) -> dict[str, Any]:
        return {"status": "ok", "action": "gather"}

    async def build(self, structure_type: str, **kwargs: Any) -> dict[str, Any]:
        return {"status": "ok", "action": "build"}


# ---------------------------------------------------------------------------
# Test 1: Full pipeline — provider -> prompt includes social context
# ---------------------------------------------------------------------------


class TestSocialContextFullPipeline:
    """Verify social context data (trust, personality, cultural norms) appears
    in the LLM decision prompt end-to-end."""

    @pytest.mark.asyncio
    async def test_social_context_appears_in_llm_prompt(self):
        """The LLM prompt should include social propensity, trust, and
        recommended target when DefaultSocialContextProvider is wired in."""
        # -- Set up a social provider with real SocialEngine --
        personality = _make_personality()
        values = ValueWeights(cooperation_weight=0.8)
        profile = AgentProfile(
            personality=personality,
            values=values,
            group_ids=["traders"],
        )

        def profile_source(aid: str):
            return profile

        def nearby_source(aid: str, tick: int):
            return [
                {
                    "agent_id": "agent-bob",
                    "personality": _make_personality(extraversion=0.7),
                    "values": ValueWeights(cooperation_weight=0.7),
                    "group_ids": ["traders"],
                }
            ]

        social_provider = DefaultSocialContextProvider(
            nearby_source=nearby_source,
            profile_source=profile_source,
        )

        # -- Verify the provider produces context --
        ctx = social_provider.build_social_context("SocialAgent", tick=1)
        assert ctx is not None
        assert ctx.social_propensity > 0.4
        assert ctx.should_socialize is True
        assert ctx.recommended_target_id == "agent-bob"
        assert "agent-bob" in ctx.trust_snapshot

        # -- Wire into DecisionEngine and capture the prompt --
        captured_prompt: list[str] = []

        async def mock_chat(messages):
            captured_prompt.append(messages[0].content)
            return LLMResponse(
                content=(
                    '{"action": "socialize", '
                    '"parameters": {"target_agent_id": "agent-bob"}, '
                    '"reasoning": "high trust", "confidence": 80}'
                ),
                model="test",
                usage=TokenUsage(prompt_tokens=10, completion_tokens=5),
            )

        mock_llm = AsyncMock()
        mock_llm.chat = mock_chat

        engine = DecisionEngine(
            provider=mock_llm,
            social_provider=social_provider,
        )

        state = _make_state()
        from agent_runtime.core.decide import DecisionPerception, SurvivalAssessment

        perception = DecisionPerception(
            tick=1,
            nearby_agents=["agent-bob"],
        )
        survival = SurvivalAssessment()

        result = await engine.decide(state, perception, survival)
        assert result.action == DecisionAction.SOCIALIZE

        # -- Verify the prompt contains social context --
        prompt = captured_prompt[0]
        assert "Social Context" in prompt
        assert "agent-bob" in prompt
        assert "trust=" in prompt
        # Social propensity should be present (formatted as percentage)
        assert "%" in prompt


# ---------------------------------------------------------------------------
# Test 2: ThinkLoop with social provider — produces SOCIALIZE action
# ---------------------------------------------------------------------------


class TestThinkLoopWithSocialProvider:
    """Verify the ThinkLoop produces a SOCIALIZE decision when the social
    provider recommends it and the LLM returns a socialize action."""

    @pytest.mark.asyncio
    async def test_thinkloop_socialize_decision_with_provider(self):
        """ThinkLoop with DefaultSocialContextProvider + LLM returns SOCIALIZE."""
        # -- Social provider --
        personality = _make_personality()
        values = ValueWeights(cooperation_weight=0.9)
        profile = AgentProfile(
            personality=personality,
            values=values,
            group_ids=["guild"],
        )

        def profile_source(aid: str):
            return profile

        def nearby_source(aid: str, tick: int):
            return [
                {
                    "agent_id": "neighbor-1",
                    "personality": _make_personality(extraversion=0.7),
                    "values": ValueWeights(cooperation_weight=0.7),
                    "group_ids": ["guild"],
                }
            ]

        social_provider = DefaultSocialContextProvider(
            nearby_source=nearby_source,
            profile_source=profile_source,
        )

        # -- Mock LLM that always returns SOCIALIZE --
        mock_llm = AsyncMock()
        mock_llm.chat.return_value = LLMResponse(
            content=(
                '{"action": "socialize", '
                '"parameters": {"target_agent_id": "neighbor-1"}, '
                '"reasoning": "trusted guild member", "confidence": 75}'
            ),
            model="test",
            usage=TokenUsage(prompt_tokens=50, completion_tokens=10),
        )

        decision_provider = LLMDecisionProvider(
            llm_provider=mock_llm,
            social_provider=social_provider,
        )

        state = _make_state(tokens=500)
        executor = ActionExecutor()

        loop = ThinkLoop(
            state=state,
            survival=SurvivalInstinct(),
            executor=executor,
            config=ThinkLoopConfig(tick_interval=0.0),
            decision_provider=decision_provider,
            world_client=_MockWorldClient(),
            social_context_provider=social_provider,
        )

        # Run 3 ticks
        await loop.run(max_ticks=3)
        assert loop.tick == 3
        assert loop.total_errors == 0

        # Verify SOCIALIZE was attempted (check executor history)
        socialize_actions = [
            r for r in executor.history if r.action_type == ActionType.SOCIALIZE
        ]
        assert len(socialize_actions) == 3
        assert all(r.status.value == "success" for r in socialize_actions)


# ---------------------------------------------------------------------------
# Test 3: ThinkLoop with social provider — cultural influence across ticks
# ---------------------------------------------------------------------------


class TestThinkLoopCulturalInfluence:
    """Verify that social context (trust, cultural norms) evolves across ticks
    when the think loop runs repeatedly with the same SocialEngine."""

    @pytest.mark.asyncio
    async def test_trust_increases_across_ticks(self):
        """Repeated cooperation events in the social engine should increase
        trust between agents across think loop ticks.

        Uses out-group trust (default 0.3) so that the increase from
        cooperation events is clearly measurable.
        """
        engine = SocialEngine()

        personality = _make_personality()
        values = ValueWeights(cooperation_weight=0.9)
        # Agent is in team_alpha, nearby agent is in team_beta (out-group)
        profile = AgentProfile(
            personality=personality,
            values=values,
            group_ids=["team_alpha"],
        )

        def profile_source(aid: str):
            return profile

        def nearby_source(aid: str, tick: int):
            return [
                {
                    "agent_id": "partner",
                    "personality": _make_personality(),
                    "values": ValueWeights(),
                    "group_ids": ["team_beta"],
                }
            ]

        social_provider = DefaultSocialContextProvider(
            engine=engine,
            nearby_source=nearby_source,
            profile_source=profile_source,
        )

        # Measure trust at tick 1 — out-group default is 0.3
        ctx_1 = social_provider.build_social_context("SocialAgent", tick=1)
        assert ctx_1 is not None
        trust_tick_1 = ctx_1.trust_snapshot.get("partner", 0.5)

        # Simulate cooperation events to increase trust.
        # The trust system looks up (agent_id, group_id), so events must
        # target the partner's group ("team_beta"), not the agent_id.
        from agent_runtime.social.intergroup_trust import (
            InterGroupEvent,
            InterGroupEventType,
        )
        for t in range(50):
            event = InterGroupEvent(
                event_type=InterGroupEventType.COOPERATION,
                source_group="SocialAgent",
                target_group="team_beta",
                tick=t,
            )
            engine.trust.update_trust_from_event(event)

        # Measure trust at tick 50
        ctx_50 = social_provider.build_social_context("SocialAgent", tick=50)
        assert ctx_50 is not None
        trust_tick_50 = ctx_50.trust_snapshot.get("partner", 0.5)

        # Trust should have increased
        assert trust_tick_50 > trust_tick_1

    @pytest.mark.asyncio
    async def test_thinkloop_10_ticks_with_social_provider(self):
        """ThinkLoop runs 10 ticks with social context without errors."""
        engine = SocialEngine()
        personality = _make_personality()
        values = ValueWeights(cooperation_weight=0.7)
        profile = AgentProfile(
            personality=personality,
            values=values,
            group_ids=["society"],
        )

        def profile_source(aid: str):
            return profile

        def nearby_source(aid: str, tick: int):
            return [
                {
                    "agent_id": "fellow-1",
                    "personality": _make_personality(extraversion=0.6),
                    "values": ValueWeights(),
                    "group_ids": ["society"],
                },
                {
                    "agent_id": "fellow-2",
                    "personality": _make_personality(extraversion=0.4),
                    "values": ValueWeights(cooperation_weight=0.6),
                    "group_ids": ["other_org"],
                },
            ]

        social_provider = DefaultSocialContextProvider(
            engine=engine,
            nearby_source=nearby_source,
            profile_source=profile_source,
        )

        # Mock LLM alternates between socialize and rest
        call_count = 0

        async def mock_chat(messages):
            nonlocal call_count
            call_count += 1
            if call_count % 2 == 1:
                return LLMResponse(
                    content=(
                        '{"action": "socialize", '
                        '"parameters": {"target_agent_id": "fellow-1"}, '
                        '"reasoning": "nearby", "confidence": 70}'
                    ),
                    model="test",
                    usage=TokenUsage(prompt_tokens=50, completion_tokens=10),
                )
            else:
                return LLMResponse(
                    content=(
                        '{"action": "rest", "parameters": {}, '
                        '"reasoning": "tired", "confidence": 60}'
                    ),
                    model="test",
                    usage=TokenUsage(prompt_tokens=50, completion_tokens=5),
                )

        mock_llm = AsyncMock()
        mock_llm.chat = mock_chat

        decision_provider = LLMDecisionProvider(
            llm_provider=mock_llm,
            social_provider=social_provider,
        )

        state = _make_state(tokens=2000, max_tokens=5000)
        executor = ActionExecutor()

        loop = ThinkLoop(
            state=state,
            survival=SurvivalInstinct(),
            executor=executor,
            config=ThinkLoopConfig(tick_interval=0.0),
            decision_provider=decision_provider,
            world_client=_MockWorldClient(),
            social_context_provider=social_provider,
        )

        await loop.run(max_ticks=10)
        assert loop.tick == 10
        assert loop.total_errors == 0

        # Verify a mix of actions
        action_types = {r.action_type for r in executor.history}
        assert ActionType.SOCIALIZE in action_types
        assert ActionType.REST in action_types
