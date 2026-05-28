"""Tests for SocialContextProvider integration into the think loop.

Covers:
- DefaultSocialContextProvider — bridges SocialEngine to decide.SocialContext
- SocialContextProvider wiring into DecisionEngine
- SocialContextProvider wiring into LLMDecisionProvider
- ThinkLoop integration with social_context_provider parameter
- End-to-end: agent decision receives social context from social modules
"""

from __future__ import annotations

from typing import Any, Dict, List
from unittest.mock import AsyncMock

import pytest

from agent_runtime.core.decide import (
    DecisionEngine,
    DecisionPerception,
    SocialContext,
    SocialContextProvider,
    SurvivalAssessment,
    build_prompt,
)
from agent_runtime.core.llm_decide import LLMDecisionProvider
from agent_runtime.core.think_loop import (
    Decision,
    Perception,
    ThinkLoop,
    ThinkLoopConfig,
)
from agent_runtime.core.act import ActionExecutor
from agent_runtime.models.agent_state import AgentState
from agent_runtime.models.enums import AgentPhase
from agent_runtime.models.personality import PersonalityVector
from agent_runtime.models.values import ValueWeights
from agent_runtime.social.engine import (
    DefaultSocialContextProvider,
    SocialEngine,
)
from agent_runtime.survival.instinct import SurvivalInstinct


# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------


def _make_state(**overrides):
    defaults = dict(
        name="TestAgent",
        tokens=500,
        max_tokens=1000,
        health=100.0,
        reputation=5.0,
        phase=AgentPhase.ADULT,
    )
    defaults.update(overrides)
    return AgentState(**defaults)


def _make_personality(**overrides):
    defaults = dict(
        openness=0.6,
        conscientiousness=0.5,
        extraversion=0.7,
        agreeableness=0.6,
        neuroticism=0.3,
        risk_tolerance=0.5,
        social_orientation=0.6,
        greed=0.4,
    )
    defaults.update(overrides)
    return PersonalityVector(**defaults)


# ---------------------------------------------------------------------------
# Stub providers for DefaultSocialContextProvider
# ---------------------------------------------------------------------------


class StubPersonalityProvider:
    """Returns a fixed personality/values/groups for any agent."""

    def __init__(
        self,
        personality: PersonalityVector | None = None,
        values: ValueWeights | None = None,
        groups: list[str] | None = None,
    ):
        self._personality = personality or _make_personality()
        self._values = values or ValueWeights()
        self._groups = groups or []

    def get_personality(self, agent_id: str) -> PersonalityVector:
        return self._personality

    def get_values(self, agent_id: str) -> ValueWeights:
        return self._values

    def get_groups(self, agent_id: str) -> List[str]:
        return self._groups


class StubNearbyAgentProvider:
    """Returns a fixed list of nearby agents."""

    def __init__(self, nearby: list[dict[str, Any]] | None = None):
        self._nearby = nearby or []

    def get_nearby_agents(
        self, agent_id: str, tick: int
    ) -> List[Dict[str, Any]]:
        return self._nearby


# ---------------------------------------------------------------------------
# Tests: DefaultSocialContextProvider
# ---------------------------------------------------------------------------


class TestDefaultSocialContextProvider:
    def test_returns_decide_social_context(self):
        engine = SocialEngine()
        provider = DefaultSocialContextProvider(
            engine=engine,
            personality_provider=StubPersonalityProvider(
                personality=_make_personality(extraversion=0.9),
                values=ValueWeights(cooperation_weight=0.8),
            ),
            nearby_provider=StubNearbyAgentProvider([
                {
                    "agent_id": "a2",
                    "personality": _make_personality(),
                    "values": ValueWeights(),
                }
            ]),
        )
        ctx = provider.build_social_context(agent_id="a1", tick=1)
        assert isinstance(ctx, SocialContext)
        assert ctx.social_propensity > 0.5
        assert ctx.should_socialize is True
        assert ctx.recommended_target_id == "a2"
        assert "a2" in ctx.trust_snapshot
        assert ctx.personality_description != ""

    def test_returns_none_like_on_empty_nearby(self):
        """With no nearby agents, should_socialize is False."""
        engine = SocialEngine()
        provider = DefaultSocialContextProvider(
            engine=engine,
            personality_provider=StubPersonalityProvider(),
            nearby_provider=StubNearbyAgentProvider([]),
        )
        ctx = provider.build_social_context(agent_id="a1", tick=1)
        assert isinstance(ctx, SocialContext)
        assert ctx.should_socialize is False
        assert ctx.recommended_target_id == ""

    def test_trust_snapshot_with_in_group(self):
        engine = SocialEngine()
        provider = DefaultSocialContextProvider(
            engine=engine,
            personality_provider=StubPersonalityProvider(
                personality=_make_personality(),
                groups=["org1"],
            ),
            nearby_provider=StubNearbyAgentProvider([
                {
                    "agent_id": "a2",
                    "personality": _make_personality(),
                    "values": ValueWeights(),
                    "group_ids": ["org1"],
                }
            ]),
        )
        ctx = provider.build_social_context(agent_id="a1", tick=1)
        assert ctx.trust_snapshot["a2"] >= 0.7  # in-group trust is high

    def test_satisfies_protocol(self):
        """DefaultSocialContextProvider satisfies the SocialContextProvider protocol."""
        engine = SocialEngine()
        provider = DefaultSocialContextProvider(
            engine=engine,
            personality_provider=StubPersonalityProvider(),
            nearby_provider=StubNearbyAgentProvider(),
        )
        # Protocol check — build_social_context must exist with correct signature
        assert hasattr(provider, "build_social_context")
        assert callable(provider.build_social_context)


# ---------------------------------------------------------------------------
# Tests: DecisionEngine with social_provider
# ---------------------------------------------------------------------------


class TestDecisionEngineWithSocialProvider:
    @pytest.mark.asyncio
    async def test_engine_calls_social_provider(self):
        """DecisionEngine.decide should call social_provider.build_social_context."""

        class TrackingProvider:
            def __init__(self):
                self.called = False

            def build_social_context(
                self, agent_id: str, tick: int
            ) -> SocialContext | None:
                self.called = True
                return SocialContext(
                    social_propensity=0.8,
                    should_socialize=True,
                    recommended_target_id="b1",
                    trust_snapshot={"b1": 0.9},
                    personality_description="A test agent.",
                )

        tracking = TrackingProvider()

        # Create a mock LLM provider that returns a valid decision
        mock_llm = AsyncMock()
        mock_llm.chat.return_value = AsyncMock(
            content='{"action": "rest", "parameters": {}, "reasoning": "test", "confidence": 80}'
        )

        engine = DecisionEngine(provider=mock_llm, social_provider=tracking)
        state = _make_state()
        perception = DecisionPerception(tick=1)
        survival = SurvivalAssessment()

        decision = await engine.decide(state, perception, survival)
        assert tracking.called
        assert decision.action.value == "rest"

    @pytest.mark.asyncio
    async def test_engine_handles_social_provider_failure_gracefully(self):
        """If social_provider raises, DecisionEngine should still decide."""

        class FailingProvider:
            def build_social_context(
                self, agent_id: str, tick: int
            ) -> SocialContext | None:
                raise RuntimeError("Social context failure")

        mock_llm = AsyncMock()
        mock_llm.chat.return_value = AsyncMock(
            content='{"action": "rest", "parameters": {}, "reasoning": "test", "confidence": 80}'
        )

        engine = DecisionEngine(provider=mock_llm, social_provider=FailingProvider())
        state = _make_state()
        perception = DecisionPerception(tick=1)
        survival = SurvivalAssessment()

        # Should NOT raise — the error is caught and logged
        decision = await engine.decide(state, perception, survival)
        assert decision.action.value == "rest"


# ---------------------------------------------------------------------------
# Tests: LLMDecisionProvider with social_provider
# ---------------------------------------------------------------------------


class TestLLMDecisionProviderWithSocialProvider:
    @pytest.mark.asyncio
    async def test_llm_provider_passes_social_provider(self):
        """LLMDecisionProvider should forward social_provider to DecisionEngine."""
        from agent_runtime.core.decide import SocialContextProvider

        class TrackingProvider:
            def __init__(self):
                self.called = False

            def build_social_context(
                self, agent_id: str, tick: int
            ) -> SocialContext | None:
                self.called = True
                return SocialContext(social_propensity=0.6, should_socialize=False)

        tracking = TrackingProvider()
        mock_llm = AsyncMock()
        mock_llm.chat.return_value = AsyncMock(
            content='{"action": "rest", "parameters": {}, "reasoning": "test", "confidence": 50}'
        )

        provider = LLMDecisionProvider(
            llm_provider=mock_llm,
            social_provider=tracking,
        )
        state = _make_state()
        perception = Perception(tick=1)
        from agent_runtime.survival.instinct import SurvivalAction
        from agent_runtime.survival.instinct import SurvivalMode
        survival = SurvivalAction(mode=SurvivalMode.NORMAL, token_ratio=0.5)

        decision = await provider.decide(state, perception, survival)
        assert tracking.called


# ---------------------------------------------------------------------------
# Tests: ThinkLoop with social_context_provider
# ---------------------------------------------------------------------------


class TestThinkLoopWithSocialContextProvider:
    @pytest.mark.asyncio
    async def test_think_loop_accepts_social_context_provider(self):
        """ThinkLoop should accept and store the social_context_provider parameter."""
        state = _make_state()
        executor = ActionExecutor()
        instinct = SurvivalInstinct()

        class DummyProvider:
            def build_social_context(
                self, agent_id: str, tick: int
            ) -> SocialContext | None:
                return SocialContext(social_propensity=0.5, should_socialize=False)

        social = DummyProvider()
        loop = ThinkLoop(
            state=state,
            survival=instinct,
            executor=executor,
            config=ThinkLoopConfig(max_ticks=1),
            social_context_provider=social,
        )
        assert loop._social_context_provider is social

    @pytest.mark.asyncio
    async def test_think_loop_without_social_context_provider(self):
        """ThinkLoop should work without social_context_provider (backward compat)."""
        state = _make_state()
        executor = ActionExecutor()
        instinct = SurvivalInstinct()

        loop = ThinkLoop(
            state=state,
            survival=instinct,
            executor=executor,
            config=ThinkLoopConfig(max_ticks=1),
        )
        assert loop._social_context_provider is None
        await loop.run()
        assert loop.tick == 1


# ---------------------------------------------------------------------------
# Tests: End-to-end social context flows into decision prompt
# ---------------------------------------------------------------------------


class TestSocialContextEndToEnd:
    def test_default_provider_output_works_in_build_prompt(self):
        """Verify the SocialContext from DefaultSocialContextProvider integrates
        correctly into the decision prompt."""
        engine = SocialEngine()
        provider = DefaultSocialContextProvider(
            engine=engine,
            personality_provider=StubPersonalityProvider(
                personality=_make_personality(extraversion=0.9),
                values=ValueWeights(cooperation_weight=0.8),
            ),
            nearby_provider=StubNearbyAgentProvider([
                {
                    "agent_id": "agent-b",
                    "personality": _make_personality(),
                    "values": ValueWeights(),
                }
            ]),
        )
        social = provider.build_social_context(agent_id="a1", tick=42)
        assert social is not None

        state = _make_state()
        perception = DecisionPerception(tick=42, nearby_agents=["agent-b"])
        survival = SurvivalAssessment()

        from agent_runtime.core.decide import DecisionAction
        prompt = build_prompt(
            state, perception, survival, DecisionAction.all(), social=social
        )

        # Verify prompt contains social context data
        assert "Social Context" in prompt
        assert "agent-b" in prompt
        assert "Recommended social target: agent-b" in prompt
        # Personality description should be present
        assert "personality" in prompt.lower() or "sociable" in prompt.lower()
