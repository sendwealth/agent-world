"""Tests for ThinkLoop._unwrap_to_engine and social/emotion provider injection."""

from __future__ import annotations

import logging
from unittest.mock import MagicMock

from agent_runtime.core.act import ActionExecutor
from agent_runtime.core.decide import SocialContext
from agent_runtime.core.llm_decide import LLMDecisionProvider
from agent_runtime.core.think_loop import ThinkLoop
from agent_runtime.llm.base import LLMResponse, TokenUsage
from agent_runtime.models.agent_state import AgentState
from agent_runtime.models.enums import AgentPhase
from agent_runtime.models.personality import PersonalityVector
from agent_runtime.survival.instinct import SurvivalInstinct


def _make_state(**overrides):
    defaults = dict(
        name="TestAgent",
        tokens=500,
        max_tokens=1000,
        health=100.0,
        reputation=5.0,
        phase=AgentPhase.ADULT,
        personality=PersonalityVector(
            extraversion=0.5,
            social_orientation=0.5,
            agreeableness=0.5,
        ).to_storage_dict(),
    )
    defaults.update(overrides)
    return AgentState(**defaults)


class _MockSocialProvider:
    def build_social_context(self, agent_id: str, tick: int):
        return SocialContext(
            social_propensity=0.7,
            should_socialize=True,
            recommended_target_id="agent-b",
        )


class _MockEmotionHook:
    def get_mood_description(self):
        return "calm"


class _FakeLLMProvider:
    """Minimal async LLM provider for constructing LLMDecisionProvider."""

    async def chat(self, messages, **kw):
        return LLMResponse(
            content='{"action": "rest", "parameters": {}, "reasoning": "test", "confidence": 50}',
            model="test",
            usage=TokenUsage(prompt_tokens=1, completion_tokens=1),
        )


class _UnknownWrapper:
    """A wrapper that exposes none of the known attributes."""

    def __init__(self, inner):
        self._hidden = inner

    async def decide(self, state, perception, survival):
        return await self._hidden.decide(state, perception, survival)


class _CircularWrapper:
    """A wrapper that creates a cycle via _inner pointing back to itself."""

    def __init__(self):
        self._inner = self


class TestUnwrapToEngine:
    def test_bare_llm_decision_provider(self):
        llm = _FakeLLMProvider()
        provider = LLMDecisionProvider(llm_provider=llm)
        engine = ThinkLoop._unwrap_to_engine(provider)
        assert engine is provider._engine

    def test_async_wrapping_llm(self):
        from agent_runtime.core.async_decide import AsyncDecisionProvider

        llm = _FakeLLMProvider()
        llm_provider = LLMDecisionProvider(llm_provider=llm)
        async_provider = AsyncDecisionProvider(inner=llm_provider)
        engine = ThinkLoop._unwrap_to_engine(async_provider)
        assert engine is llm_provider._engine

    def test_memory_aware_wrapping_llm(self):
        from agent_runtime.core.memory_aware_decide import MemoryAwareDecisionProvider

        llm = _FakeLLMProvider()
        llm_provider = LLMDecisionProvider(llm_provider=llm)
        mock_recall = MagicMock()
        mem_provider = MemoryAwareDecisionProvider(
            base_provider=llm_provider, memory_recall=mock_recall,
        )
        engine = ThinkLoop._unwrap_to_engine(mem_provider)
        assert engine is llm_provider._engine

    def test_deep_chain_async_memory_llm(self):
        from agent_runtime.core.async_decide import AsyncDecisionProvider
        from agent_runtime.core.memory_aware_decide import MemoryAwareDecisionProvider

        llm = _FakeLLMProvider()
        llm_provider = LLMDecisionProvider(llm_provider=llm)
        mock_recall = MagicMock()
        mem_provider = MemoryAwareDecisionProvider(
            base_provider=llm_provider, memory_recall=mock_recall,
        )
        async_provider = AsyncDecisionProvider(inner=mem_provider)
        engine = ThinkLoop._unwrap_to_engine(async_provider)
        assert engine is llm_provider._engine

    def test_unknown_wrapper_returns_none(self):
        llm = _FakeLLMProvider()
        llm_provider = LLMDecisionProvider(llm_provider=llm)
        unknown = _UnknownWrapper(llm_provider)
        engine = ThinkLoop._unwrap_to_engine(unknown)
        assert engine is None

    def test_circular_reference_safe(self):
        circular = _CircularWrapper()
        engine = ThinkLoop._unwrap_to_engine(circular)
        assert engine is None

    def test_plain_object_returns_none(self):
        engine = ThinkLoop._unwrap_to_engine(object())
        assert engine is None


class TestInjectSocialProvider:
    def test_inject_through_async_provider(self):
        from agent_runtime.core.async_decide import AsyncDecisionProvider

        llm = _FakeLLMProvider()
        llm_provider = LLMDecisionProvider(llm_provider=llm)
        async_provider = AsyncDecisionProvider(inner=llm_provider)

        state = _make_state()
        loop = ThinkLoop(
            state=state,
            survival=SurvivalInstinct(),
            executor=ActionExecutor(),
            decision_provider=async_provider,
        )
        social = _MockSocialProvider()
        loop._inject_social_provider(social)
        assert llm_provider._engine._social_provider is social

    def test_inject_through_memory_aware_provider(self):
        from agent_runtime.core.memory_aware_decide import MemoryAwareDecisionProvider

        llm = _FakeLLMProvider()
        llm_provider = LLMDecisionProvider(llm_provider=llm)
        mock_recall = MagicMock()
        mem_provider = MemoryAwareDecisionProvider(
            base_provider=llm_provider, memory_recall=mock_recall,
        )

        state = _make_state()
        loop = ThinkLoop(
            state=state,
            survival=SurvivalInstinct(),
            executor=ActionExecutor(),
            decision_provider=mem_provider,
        )
        social = _MockSocialProvider()
        loop._inject_social_provider(social)
        assert llm_provider._engine._social_provider is social

    def test_inject_through_deep_chain(self):
        from agent_runtime.core.async_decide import AsyncDecisionProvider
        from agent_runtime.core.memory_aware_decide import MemoryAwareDecisionProvider

        llm = _FakeLLMProvider()
        llm_provider = LLMDecisionProvider(llm_provider=llm)
        mock_recall = MagicMock()
        mem_provider = MemoryAwareDecisionProvider(
            base_provider=llm_provider, memory_recall=mock_recall,
        )
        async_provider = AsyncDecisionProvider(inner=mem_provider)

        state = _make_state()
        loop = ThinkLoop(
            state=state,
            survival=SurvivalInstinct(),
            executor=ActionExecutor(),
            decision_provider=async_provider,
        )
        social = _MockSocialProvider()
        loop._inject_social_provider(social)
        assert llm_provider._engine._social_provider is social

    def test_inject_on_construction_with_deep_chain(self):
        from agent_runtime.core.async_decide import AsyncDecisionProvider
        from agent_runtime.core.memory_aware_decide import MemoryAwareDecisionProvider

        llm = _FakeLLMProvider()
        llm_provider = LLMDecisionProvider(llm_provider=llm)
        mock_recall = MagicMock()
        mem_provider = MemoryAwareDecisionProvider(
            base_provider=llm_provider, memory_recall=mock_recall,
        )
        async_provider = AsyncDecisionProvider(inner=mem_provider)

        social = _MockSocialProvider()
        state = _make_state()
        loop = ThinkLoop(
            state=state,
            survival=SurvivalInstinct(),
            executor=ActionExecutor(),
            decision_provider=async_provider,
            social_context_provider=social,
        )
        assert loop.social_context_provider is social
        assert llm_provider._engine._social_provider is social

    def test_failure_logs_warning_not_debug(self, caplog):
        unknown = _UnknownWrapper(MagicMock())
        state = _make_state()
        loop = ThinkLoop(
            state=state,
            survival=SurvivalInstinct(),
            executor=ActionExecutor(),
            decision_provider=unknown,
        )
        with caplog.at_level(logging.DEBUG, logger="agent_runtime.core.think_loop"):
            loop._inject_social_provider(_MockSocialProvider())

        warning_records = [r for r in caplog.records if r.levelno == logging.WARNING]
        assert len(warning_records) == 1
        assert "support injection" in warning_records[0].message


class TestInjectEmotionProvider:
    def test_inject_through_memory_aware_provider(self):
        from agent_runtime.core.memory_aware_decide import MemoryAwareDecisionProvider

        llm = _FakeLLMProvider()
        llm_provider = LLMDecisionProvider(llm_provider=llm)
        mock_recall = MagicMock()
        mem_provider = MemoryAwareDecisionProvider(
            base_provider=llm_provider, memory_recall=mock_recall,
        )

        state = _make_state()
        loop = ThinkLoop(
            state=state,
            survival=SurvivalInstinct(),
            executor=ActionExecutor(),
            decision_provider=mem_provider,
        )
        hook = _MockEmotionHook()
        loop._inject_emotion_provider(hook)
        assert llm_provider._engine._emotion_provider is hook

    def test_inject_through_deep_chain(self):
        from agent_runtime.core.async_decide import AsyncDecisionProvider
        from agent_runtime.core.memory_aware_decide import MemoryAwareDecisionProvider

        llm = _FakeLLMProvider()
        llm_provider = LLMDecisionProvider(llm_provider=llm)
        mock_recall = MagicMock()
        mem_provider = MemoryAwareDecisionProvider(
            base_provider=llm_provider, memory_recall=mock_recall,
        )
        async_provider = AsyncDecisionProvider(inner=mem_provider)

        state = _make_state()
        loop = ThinkLoop(
            state=state,
            survival=SurvivalInstinct(),
            executor=ActionExecutor(),
            decision_provider=async_provider,
        )
        hook = _MockEmotionHook()
        loop._inject_emotion_provider(hook)
        assert llm_provider._engine._emotion_provider is hook

    def test_failure_logs_warning_not_debug(self, caplog):
        unknown = _UnknownWrapper(MagicMock())
        state = _make_state()
        loop = ThinkLoop(
            state=state,
            survival=SurvivalInstinct(),
            executor=ActionExecutor(),
            decision_provider=unknown,
        )
        with caplog.at_level(logging.DEBUG, logger="agent_runtime.core.think_loop"):
            loop._inject_emotion_provider(_MockEmotionHook())

        warning_records = [r for r in caplog.records if r.levelno == logging.WARNING]
        assert len(warning_records) == 1
        assert "support injection" in warning_records[0].message

