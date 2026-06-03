"""Tests for DefaultSocialContextProvider integration.

Covers:
- DefaultSocialContextProvider.build_social_context with profile and nearby data
- Provider returns None when no profile source is configured
- Provider adapts social.engine.SocialContext to decide.SocialContext
- ThinkLoop accepts and injects social_context_provider
- LLMDecisionProvider passes social_provider to DecisionEngine
- Full integration: provider -> DecisionEngine -> prompt includes social context
"""

from __future__ import annotations

from typing import Any
from unittest.mock import AsyncMock

import pytest

from agent_runtime.core.act import ActionExecutor
from agent_runtime.core.decide import (
    DecisionAction,
    DecisionEngine,
    DecisionPerception,
    SocialContext,
    SurvivalAssessment,
    build_prompt,
)
from agent_runtime.core.llm_decide import LLMDecisionProvider
from agent_runtime.core.think_loop import ThinkLoop
from agent_runtime.llm.base import LLMResponse, TokenUsage
from agent_runtime.models.agent_state import AgentState
from agent_runtime.models.enums import AgentPhase
from agent_runtime.models.personality import PersonalityVector
from agent_runtime.models.values import ValueWeights
from agent_runtime.social.provider import (
    AgentProfile,
    DefaultSocialContextProvider,
)

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
        personality=PersonalityVector(
            extraversion=0.8,
            social_orientation=0.7,
            agreeableness=0.6,
        ).to_storage_dict(),
    )
    defaults.update(overrides)
    return AgentState(**defaults)


def _make_personality(**overrides):
    defaults = dict(
        extraversion=0.8,
        social_orientation=0.7,
        agreeableness=0.6,
        openness=0.5,
        conscientiousness=0.5,
        neuroticism=0.3,
        risk_tolerance=0.5,
        greed=0.4,
    )
    defaults.update(overrides)
    return PersonalityVector(**defaults)


def _extraverted_profile():
    return AgentProfile(
        personality=_make_personality(
            extraversion=0.9,
            social_orientation=0.9,
            agreeableness=0.8,
        ),
        values=ValueWeights(cooperation_weight=0.8),
        group_ids=["org1"],
    )


def _introverted_profile():
    return AgentProfile(
        personality=_make_personality(
            extraversion=0.1,
            social_orientation=0.1,
            agreeableness=0.2,
        ),
        values=ValueWeights(cooperation_weight=0.2),
        group_ids=[],
    )


def _nearby_agents():
    return [
        {
            "agent_id": "agent-b",
            "personality": _make_personality(extraversion=0.7),
            "values": ValueWeights(),
        }
    ]


# ---------------------------------------------------------------------------
# DefaultSocialContextProvider tests
# ---------------------------------------------------------------------------


class TestDefaultSocialContextProvider:
    def test_returns_none_without_profile_source(self):
        """Provider returns None when no profile source is configured."""
        provider = DefaultSocialContextProvider()
        result = provider.build_social_context("agent-1", tick=1)
        assert result is None

    def test_returns_none_for_unknown_agent(self):
        """Provider returns None when profile source doesn't know the agent."""
        def profile_source(aid: str):
            return None

        provider = DefaultSocialContextProvider(profile_source=profile_source)
        result = provider.build_social_context("unknown-agent", tick=1)
        assert result is None

    def test_returns_social_context_with_profile(self):
        """Provider returns decide.SocialContext when profile is available."""
        profile = _extraverted_profile()

        def profile_source(aid: str):
            if aid == "agent-1":
                return profile
            return None

        def nearby_source(aid: str, tick: int):
            return _nearby_agents()

        provider = DefaultSocialContextProvider(
            profile_source=profile_source,
            nearby_source=nearby_source,
        )
        result = provider.build_social_context("agent-1", tick=5)

        assert result is not None
        assert isinstance(result, SocialContext)
        assert result.social_propensity > 0.4
        assert result.should_socialize is True
        assert result.recommended_target_id == "agent-b"
        assert "agent-b" in result.trust_snapshot
        assert len(result.personality_description) > 0

    def test_extraverted_has_higher_propensity_than_introverted(self):
        """Extraverted agent has higher social propensity."""
        extraverted = _extraverted_profile()
        introverted = _introverted_profile()

        def profile_ext(aid: str):
            return extraverted

        def profile_intro(aid: str):
            return introverted

        def nearby(aid: str, tick: int):
            return _nearby_agents()

        provider_ext = DefaultSocialContextProvider(
            profile_source=profile_ext,
            nearby_source=nearby,
        )
        provider_intro = DefaultSocialContextProvider(
            profile_source=profile_intro,
            nearby_source=nearby,
        )

        ctx_ext = provider_ext.build_social_context("agent-1", tick=1)
        ctx_intro = provider_intro.build_social_context("agent-1", tick=1)

        assert ctx_ext is not None
        assert ctx_intro is not None
        assert ctx_ext.social_propensity > ctx_intro.social_propensity

    def test_no_nearby_agents_means_no_socialize(self):
        """Agent should not socialize when there are no nearby agents."""
        profile = _extraverted_profile()

        def profile_source(aid: str):
            return profile

        def nearby_source(aid: str, tick: int):
            return []

        provider = DefaultSocialContextProvider(
            profile_source=profile_source,
            nearby_source=nearby_source,
        )
        result = provider.build_social_context("agent-1", tick=1)

        assert result is not None
        assert result.should_socialize is False
        assert result.recommended_target_id == ""

    def test_trust_snapshot_populated_with_nearby_agents(self):
        """Trust snapshot is populated when nearby agents have groups."""
        profile = _extraverted_profile()

        def profile_source(aid: str):
            return profile

        def nearby_source(aid: str, tick: int):
            return [
                {
                    "agent_id": "agent-b",
                    "personality": _make_personality(),
                    "values": ValueWeights(),
                    "group_ids": ["org1"],
                }
            ]

        provider = DefaultSocialContextProvider(
            profile_source=profile_source,
            nearby_source=nearby_source,
        )
        result = provider.build_social_context("agent-1", tick=1)

        assert result is not None
        assert "agent-b" in result.trust_snapshot
        assert result.trust_snapshot["agent-b"] >= 0.0

    def test_graceful_degradation_on_engine_failure(self):
        """Provider returns None when SocialEngine raises an exception."""
        from unittest.mock import MagicMock

        profile = _extraverted_profile()

        def profile_source(aid: str):
            return profile

        def nearby_source(aid: str, tick: int):
            return _nearby_agents()

        # Create a mock engine that raises
        mock_engine = MagicMock()
        mock_engine.build_context.side_effect = RuntimeError("engine failure")

        provider = DefaultSocialContextProvider(
            engine=mock_engine,
            profile_source=profile_source,
            nearby_source=nearby_source,
        )
        result = provider.build_social_context("agent-1", tick=1)

        # Should return None, not raise
        assert result is None

    def test_implements_social_context_provider_protocol(self):
        """DefaultSocialContextProvider has the build_social_context method."""
        provider = DefaultSocialContextProvider()
        assert hasattr(provider, "build_social_context")
        assert callable(provider.build_social_context)


# ---------------------------------------------------------------------------
# Social context in DecisionEngine (integration with provider)
# ---------------------------------------------------------------------------


class TestDecisionEngineWithSocialProvider:
    def test_engine_uses_social_provider(self):
        """DecisionEngine uses injected social provider in prompt building."""
        mock_llm = AsyncMock()
        mock_llm.chat.return_value = LLMResponse(
            content=(
                '{"action": "explore", "parameters": {},'
                ' "reasoning": "test", "confidence": 50}'
            ),
            model="test-model",
            usage=TokenUsage(prompt_tokens=10, completion_tokens=5),
        )

        # Create a mock social provider
        social_ctx = SocialContext(
            social_propensity=0.8,
            should_socialize=True,
            recommended_target_id="agent-b",
            trust_snapshot={"agent-b": 0.9},
            personality_description="A highly sociable agent.",
        )

        class MockSocialProvider:
            def build_social_context(self, agent_id: str, tick: int):
                return social_ctx

        engine = DecisionEngine(
            provider=mock_llm,
            social_provider=MockSocialProvider(),
        )

        # We can't easily test the internal call here without running decide,
        # but we can verify the engine has the provider set
        assert engine._social_provider is not None

    @pytest.mark.asyncio
    async def test_engine_decide_with_social_context(self):
        """DecisionEngine produces a decision using social context from provider."""
        mock_llm = AsyncMock()
        mock_llm.chat.return_value = LLMResponse(
            content=(
                '{"action": "socialize",'
                ' "parameters": {"target_agent_id": "agent-b"},'
                ' "reasoning": "high social propensity",'
                ' "confidence": 80}'
            ),
            model="test-model",
            usage=TokenUsage(prompt_tokens=100, completion_tokens=20),
        )

        social_ctx = SocialContext(
            social_propensity=0.9,
            should_socialize=True,
            recommended_target_id="agent-b",
            trust_snapshot={"agent-b": 0.85},
            personality_description="Extraverted and cooperative.",
        )

        class MockSocialProvider:
            def build_social_context(self, agent_id: str, tick: int):
                return social_ctx

        engine = DecisionEngine(
            provider=mock_llm,
            social_provider=MockSocialProvider(),
        )

        state = _make_state()
        perception = DecisionPerception(tick=1, nearby_agents=["agent-b"])
        survival = SurvivalAssessment()

        result = await engine.decide(state, perception, survival)

        assert result.action == DecisionAction.SOCIALIZE
        # Verify the LLM was called (meaning social context was injected)
        mock_llm.chat.assert_called_once()
        call_args = mock_llm.chat.call_args
        prompt = call_args[0][0][0].content
        # Social context should appear in the prompt
        assert "Social Context" in prompt
        assert "90%" in prompt  # social_propensity

    @pytest.mark.asyncio
    async def test_engine_decide_without_social_provider(self):
        """DecisionEngine works fine without a social provider."""
        mock_llm = AsyncMock()
        mock_llm.chat.return_value = LLMResponse(
            content='{"action": "rest", "parameters": {}, "reasoning": "tired", "confidence": 60}',
            model="test-model",
            usage=TokenUsage(prompt_tokens=50, completion_tokens=10),
        )

        engine = DecisionEngine(provider=mock_llm)
        state = _make_state()
        perception = DecisionPerception(tick=1)
        survival = SurvivalAssessment()

        result = await engine.decide(state, perception, survival)
        assert result.action == DecisionAction.REST


# ---------------------------------------------------------------------------
# LLMDecisionProvider with social_provider
# ---------------------------------------------------------------------------


class TestLLMDecisionProviderWithSocial:
    @pytest.mark.asyncio
    async def test_llm_decide_accepts_social_provider(self):
        """LLMDecisionProvider passes social_provider to DecisionEngine."""
        mock_llm = AsyncMock()
        mock_llm.chat.return_value = LLMResponse(
            content='{"action": "rest", "parameters": {}, "reasoning": "test", "confidence": 50}',
            model="test-model",
            usage=TokenUsage(prompt_tokens=10, completion_tokens=5),
        )

        social_ctx = SocialContext(
            social_propensity=0.7,
            should_socialize=True,
            recommended_target_id="agent-b",
        )

        class MockSocialProvider:
            def build_social_context(self, agent_id: str, tick: int):
                return social_ctx

        provider = LLMDecisionProvider(
            llm_provider=mock_llm,
            social_provider=MockSocialProvider(),
        )

        # Verify the inner engine has the social provider
        assert provider._engine._social_provider is not None


# ---------------------------------------------------------------------------
# ThinkLoop integration
# ---------------------------------------------------------------------------


class TestThinkLoopSocialIntegration:
    def test_thinkloop_accepts_social_context_provider(self):
        """ThinkLoop stores social_context_provider when passed."""
        state = _make_state()
        from agent_runtime.survival.instinct import SurvivalInstinct

        social_ctx = SocialContext(
            social_propensity=0.7,
            should_socialize=True,
            recommended_target_id="agent-b",
        )

        class MockSocialProvider:
            def build_social_context(self, agent_id: str, tick: int):
                return social_ctx

        provider = MockSocialProvider()

        loop = ThinkLoop(
            state=state,
            survival=SurvivalInstinct(),
            executor=ActionExecutor(),
            social_context_provider=provider,
        )

        assert loop.social_context_provider is provider

    def test_thinkloop_without_social_provider(self):
        """ThinkLoop works fine without a social provider (backward compat)."""
        state = _make_state()
        from agent_runtime.survival.instinct import SurvivalInstinct

        loop = ThinkLoop(
            state=state,
            survival=SurvivalInstinct(),
            executor=ActionExecutor(),
        )

        assert loop.social_context_provider is None


# ---------------------------------------------------------------------------
# Full integration: provider -> prompt
# ---------------------------------------------------------------------------


class TestFullSocialContextIntegration:
    def test_social_context_flows_to_prompt(self):
        """Verify the full chain: provider -> SocialContext -> prompt text."""
        profile = _extraverted_profile()

        def profile_source(aid: str):
            return profile

        def nearby_source(aid: str, tick: int):
            return [
                {
                    "agent_id": "agent-b",
                    "personality": _make_personality(),
                    "values": ValueWeights(cooperation_weight=0.7),
                }
            ]

        provider = DefaultSocialContextProvider(
            profile_source=profile_source,
            nearby_source=nearby_source,
        )

        # Build context
        social = provider.build_social_context("agent-1", tick=10)
        assert social is not None

        # Feed into prompt builder
        state = _make_state()
        perception = DecisionPerception(
            tick=10,
            nearby_agents=["agent-b"],
        )
        survival = SurvivalAssessment()

        prompt = build_prompt(
            state,
            perception,
            survival,
            DecisionAction.all(),
            social=social,
        )

        # Verify social context appears in the prompt
        assert "Social Context" in prompt
        assert "agent-b" in prompt
        assert "trust=" in prompt


# ---------------------------------------------------------------------------
# Tests for nearby_source wiring through perception cache
# ---------------------------------------------------------------------------


class TestNearbySourceFromPerceptionCache:
    """Validate that the nearby_source callback reads from a mutable cache
    that gets updated with perception data each tick.

    This mirrors the wiring done in ``__main__.py`` — the ThinkLoop updates
    the cache after perception, and the social provider reads it synchronously.
    """

    def test_nearby_source_reads_from_cache(self):
        """The nearby_source closure returns data that was placed in the cache."""
        nearby_cache: list[dict[str, Any]] = []

        def nearby_source(aid: str, tick: int):
            return list(nearby_cache)

        profile = _extraverted_profile()
        provider = DefaultSocialContextProvider(
            profile_source=lambda aid: profile if aid == "a1" else None,
            nearby_source=nearby_source,
        )

        # Initially no nearby agents
        result = provider.build_social_context("a1", tick=1)
        assert result is not None
        assert result.should_socialize is False
        assert result.recommended_target_id == ""

        # Simulate perception updating the cache with nearby agents
        nearby_cache.clear()
        nearby_cache.extend(_nearby_agents())

        # Now the social context should reflect nearby agents
        result = provider.build_social_context("a1", tick=2)
        assert result is not None
        assert result.should_socialize is True
        assert result.recommended_target_id == "agent-b"

    @pytest.mark.asyncio
    async def test_thinkloop_feeds_perception_into_social_cache(self):
        """ThinkLoop updates social_nearby_cache from perception.market_state."""
        state = _make_state()

        # Create a perception provider that returns nearby agents
        from agent_runtime.core.think_loop import Perception

        class FakePerceptionProvider:
            async def perceive(self, s, tick):
                return Perception(
                    messages=[],
                    token_balance=s.tokens,
                    token_ratio=0.5,
                    market_state={
                        "nearby_agents": [
                            {"agent_id": "neighbor-1", "name": "Bob", "tokens": 100},
                            {"agent_id": "neighbor-2", "name": "Alice", "tokens": 200},
                        ],
                        "agent_count": 2,
                    },
                    tick=tick,
                )

        # Build social provider with nearby cache (mirrors __main__.py wiring)
        profile = _extraverted_profile()
        nearby_cache: list[dict[str, Any]] = []

        def nearby_source(aid, tick):
            return list(nearby_cache)

        social_provider = DefaultSocialContextProvider(
            profile_source=lambda aid: profile if aid == str(state.id) else None,
            nearby_source=nearby_source,
        )

        executor = ActionExecutor()
        from agent_runtime.survival.instinct import SurvivalInstinct
        survival = SurvivalInstinct()

        loop = ThinkLoop(
            state=state,
            survival=survival,
            executor=executor,
            perception_provider=FakePerceptionProvider(),
            social_context_provider=social_provider,
            social_nearby_cache=nearby_cache,
        )

        # Run one tick to populate the cache
        await loop._think_once()

        # The nearby cache should now contain the agents from perception
        assert len(nearby_cache) == 2
        assert nearby_cache[0]["agent_id"] == "neighbor-1"
        assert nearby_cache[1]["agent_id"] == "neighbor-2"

    @pytest.mark.asyncio
    async def test_social_context_has_nearby_agents_after_tick(self):
        """After a tick with nearby agents in perception, social context is non-empty."""
        from agent_runtime.core.think_loop import Perception

        state = _make_state()

        class FakePerceptionProvider:
            async def perceive(self, s, tick):
                return Perception(
                    market_state={
                        "nearby_agents": _nearby_agents(),
                    },
                    tick=tick,
                )

        profile = _extraverted_profile()
        nearby_cache: list[dict[str, Any]] = []

        def nearby_source(aid, tick):
            return list(nearby_cache)

        social_provider = DefaultSocialContextProvider(
            profile_source=lambda aid: profile if aid == str(state.id) else None,
            nearby_source=nearby_source,
        )

        # Before tick: no nearby agents
        ctx_before = social_provider.build_social_context(str(state.id), tick=0)
        assert ctx_before is not None
        assert ctx_before.recommended_target_id == ""

        executor = ActionExecutor()
        from agent_runtime.survival.instinct import SurvivalInstinct
        survival = SurvivalInstinct()

        loop = ThinkLoop(
            state=state,
            survival=survival,
            executor=executor,
            perception_provider=FakePerceptionProvider(),
            social_context_provider=social_provider,
            social_nearby_cache=nearby_cache,
        )

        await loop._think_once()

        # After tick: social context should have nearby agents
        ctx_after = social_provider.build_social_context(str(state.id), tick=1)
        assert ctx_after is not None
        assert ctx_after.should_socialize is True
        assert ctx_after.recommended_target_id == "agent-b"
