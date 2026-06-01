"""Tests for Fallback Chain — DecisionEngine fallback + FallbackChainProvider.

Covers:
- DecisionEngine with fallback_providers tries fallbacks on primary failure
- DecisionEngine returns random decision when all providers fail
- FallbackChainProvider (LLMProvider wrapper) tries providers in order
- FallbackChainProvider raises LLMError when all fail
- Single provider (no fallbacks) works as before
- Empty fallback list works
- Logging emits "Fallback triggered" events
"""

from __future__ import annotations

import logging

import pytest

from agent_runtime.core.decide import (
    DecisionAction,
    DecisionEngine,
    DecisionPerception,
    SurvivalAssessment,
)
from agent_runtime.llm.base import (
    LLMConfig,
    LLMError,
    LLMMessage,
    LLMProvider,
    LLMResponse,
    ProviderType,
)
from agent_runtime.llm.fallback import FallbackChainProvider, ModelFallback
from agent_runtime.models.agent_state import AgentState

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


class _StubLLMProvider(LLMProvider):
    """Deterministic LLM provider that returns a fixed response."""

    def __init__(
        self,
        config: LLMConfig,
        response_text: str = (
            '{"action": "rest", "parameters": {},'
            ' "reasoning": "stub", "confidence": 80}'
        ),
    ) -> None:
        super().__init__(config)
        self._response_text = response_text
        self.call_count: int = 0

    async def chat(
        self,
        messages: list[LLMMessage],
        *,
        max_tokens: int | None = None,
        temperature: float | None = None,
    ) -> LLMResponse:
        self.call_count += 1
        return LLMResponse(
            content=self._response_text,
            model=self._config.model,
        )

    def chat_stream(self, messages, **kwargs):
        raise NotImplementedError


class _FailingLLMProvider(LLMProvider):
    """LLM provider that always raises RuntimeError."""

    def __init__(self, config: LLMConfig, error_msg: str = "LLM unavailable") -> None:
        super().__init__(config)
        self._error_msg = error_msg
        self.call_count: int = 0

    async def chat(self, messages, **kwargs):
        self.call_count += 1
        raise RuntimeError(self._error_msg)

    def chat_stream(self, messages, **kwargs):
        raise NotImplementedError


class _LLMErrorProvider(LLMProvider):
    """LLM provider that always raises LLMError."""

    def __init__(self, config: LLMConfig) -> None:
        super().__init__(config)
        self.call_count: int = 0

    async def chat(self, messages, **kwargs):
        self.call_count += 1
        raise LLMError(
            "Provider error",
            provider=self._config.provider.value,
            model=self._config.model,
        )

    def chat_stream(self, messages, **kwargs):
        raise NotImplementedError


def _make_state(**overrides) -> AgentState:
    defaults = dict(name="TestAgent", tokens=500, max_tokens=1000)
    defaults.update(overrides)
    return AgentState(**defaults)


def _make_perception() -> DecisionPerception:
    return DecisionPerception(tick=1)


def _make_survival() -> SurvivalAssessment:
    return SurvivalAssessment()


# ---------------------------------------------------------------------------
# DecisionEngine fallback integration
# ---------------------------------------------------------------------------


class TestDecisionEngineFallback:
    @pytest.mark.asyncio
    async def test_primary_succeeds_no_fallback(self):
        """When primary succeeds, fallbacks are not tried."""
        primary = _StubLLMProvider(
            LLMConfig(provider=ProviderType.OPENAI, model="gpt-4"),
        )
        fallback = _StubLLMProvider(
            LLMConfig(provider=ProviderType.ANTHROPIC, model="claude-3"),
        )

        engine = DecisionEngine(
            provider=primary,
            fallback_providers=[fallback],
        )

        state = _make_state()
        decision = await engine.decide(state, _make_perception(), _make_survival())

        assert decision.action == DecisionAction.REST
        assert primary.call_count == 1
        assert fallback.call_count == 0

    @pytest.mark.asyncio
    async def test_primary_fails_fallback_succeeds(self):
        """When primary fails, fallback is tried and succeeds."""
        primary = _FailingLLMProvider(
            LLMConfig(provider=ProviderType.OPENAI, model="gpt-4"),
        )
        fallback = _StubLLMProvider(
            LLMConfig(provider=ProviderType.ANTHROPIC, model="claude-3"),
        )

        engine = DecisionEngine(
            provider=primary,
            fallback_providers=[fallback],
        )

        state = _make_state()
        decision = await engine.decide(state, _make_perception(), _make_survival())

        assert decision.action == DecisionAction.REST
        assert primary.call_count == 1
        assert fallback.call_count == 1

    @pytest.mark.asyncio
    async def test_multiple_fallbacks_tried_in_order(self):
        """Fallbacks are tried in order until one succeeds."""
        primary = _FailingLLMProvider(
            LLMConfig(provider=ProviderType.OPENAI, model="gpt-4"),
        )
        fallback1 = _FailingLLMProvider(
            LLMConfig(provider=ProviderType.ANTHROPIC, model="claude-3"),
        )
        fallback2 = _StubLLMProvider(
            LLMConfig(provider=ProviderType.OLLAMA, model="qwen3:8b"),
        )

        engine = DecisionEngine(
            provider=primary,
            fallback_providers=[fallback1, fallback2],
        )

        state = _make_state()
        decision = await engine.decide(state, _make_perception(), _make_survival())

        assert decision.action == DecisionAction.REST
        assert primary.call_count == 1
        assert fallback1.call_count == 1
        assert fallback2.call_count == 1

    @pytest.mark.asyncio
    async def test_all_fail_returns_random_decision(self):
        """When all providers fail, a random decision is returned."""
        primary = _FailingLLMProvider(
            LLMConfig(provider=ProviderType.OPENAI, model="gpt-4"),
        )
        fallback = _FailingLLMProvider(
            LLMConfig(provider=ProviderType.ANTHROPIC, model="claude-3"),
        )

        engine = DecisionEngine(
            provider=primary,
            fallback_providers=[fallback],
        )

        state = _make_state()
        decision = await engine.decide(state, _make_perception(), _make_survival())

        # Should be a random fallback decision (confidence=0)
        assert decision.confidence == 0
        assert decision.action in DecisionAction.__members__.values()

    @pytest.mark.asyncio
    async def test_no_fallbacks_primary_fails(self):
        """Without fallbacks, primary failure returns random decision."""
        primary = _FailingLLMProvider(
            LLMConfig(provider=ProviderType.OPENAI, model="gpt-4"),
        )

        engine = DecisionEngine(provider=primary)

        state = _make_state()
        decision = await engine.decide(state, _make_perception(), _make_survival())

        assert decision.confidence == 0
        assert "Fallback" in decision.reasoning or "random" in decision.reasoning

    @pytest.mark.asyncio
    async def test_llm_error_triggers_fallback(self):
        """LLMError (not just RuntimeError) triggers fallback."""
        primary = _LLMErrorProvider(
            LLMConfig(provider=ProviderType.OPENAI, model="gpt-4"),
        )
        fallback = _StubLLMProvider(
            LLMConfig(provider=ProviderType.OLLAMA, model="qwen3:8b"),
        )

        engine = DecisionEngine(
            provider=primary,
            fallback_providers=[fallback],
        )

        state = _make_state()
        decision = await engine.decide(state, _make_perception(), _make_survival())

        assert decision.action == DecisionAction.REST
        assert primary.call_count == 1
        assert fallback.call_count == 1

    @pytest.mark.asyncio
    async def test_json_parse_error_in_primary_tries_fallback(self):
        """If primary returns invalid JSON, fallback is tried."""
        primary = _StubLLMProvider(
            LLMConfig(provider=ProviderType.OPENAI, model="gpt-4"),
            response_text="not valid json",
        )
        fallback = _StubLLMProvider(
            LLMConfig(provider=ProviderType.OLLAMA, model="qwen3:8b"),
        )

        engine = DecisionEngine(
            provider=primary,
            fallback_providers=[fallback],
        )

        state = _make_state()
        decision = await engine.decide(state, _make_perception(), _make_survival())

        # Fallback should succeed
        assert decision.action == DecisionAction.REST
        assert fallback.call_count == 1

    @pytest.mark.asyncio
    async def test_empty_fallback_list(self):
        """Empty fallback list works — same as no fallbacks."""
        primary = _StubLLMProvider(
            LLMConfig(provider=ProviderType.OPENAI, model="gpt-4"),
        )

        engine = DecisionEngine(provider=primary, fallback_providers=[])

        state = _make_state()
        decision = await engine.decide(state, _make_perception(), _make_survival())

        assert decision.action == DecisionAction.REST
        assert primary.call_count == 1


# ---------------------------------------------------------------------------
# FallbackChainProvider (LLMProvider wrapper)
# ---------------------------------------------------------------------------


class TestFallbackChainProvider:
    @pytest.mark.asyncio
    async def test_primary_succeeds(self):
        primary = _StubLLMProvider(
            LLMConfig(provider=ProviderType.OPENAI, model="gpt-4"),
        )
        fallback = _StubLLMProvider(
            LLMConfig(provider=ProviderType.OLLAMA, model="qwen3:8b"),
        )

        chain = ModelFallback(primary=primary, fallbacks=[fallback])
        provider = FallbackChainProvider(chain)

        messages = [LLMMessage(role="user", content="test")]
        response = await provider.chat(messages)

        assert response.model == "gpt-4"
        assert primary.call_count == 1
        assert fallback.call_count == 0

    @pytest.mark.asyncio
    async def test_primary_fails_fallback_succeeds(self):
        primary = _FailingLLMProvider(
            LLMConfig(provider=ProviderType.OPENAI, model="gpt-4"),
        )
        fallback = _StubLLMProvider(
            LLMConfig(provider=ProviderType.OLLAMA, model="qwen3:8b"),
        )

        chain = ModelFallback(primary=primary, fallbacks=[fallback])
        provider = FallbackChainProvider(chain)

        messages = [LLMMessage(role="user", content="test")]
        response = await provider.chat(messages)

        assert response.model == "qwen3:8b"
        assert primary.call_count == 1
        assert fallback.call_count == 1

    @pytest.mark.asyncio
    async def test_all_fail_raises_llm_error(self):
        primary = _FailingLLMProvider(
            LLMConfig(provider=ProviderType.OPENAI, model="gpt-4"),
        )
        fallback = _FailingLLMProvider(
            LLMConfig(provider=ProviderType.OLLAMA, model="qwen3:8b"),
        )

        chain = ModelFallback(primary=primary, fallbacks=[fallback])
        provider = FallbackChainProvider(chain)

        messages = [LLMMessage(role="user", content="test")]
        with pytest.raises(LLMError, match="All providers failed"):
            await provider.chat(messages)

    @pytest.mark.asyncio
    async def test_chain_no_fallbacks(self):
        """Chain with no fallbacks — primary failure raises."""
        primary = _FailingLLMProvider(
            LLMConfig(provider=ProviderType.OPENAI, model="gpt-4"),
        )

        chain = ModelFallback(primary=primary, fallbacks=[])
        provider = FallbackChainProvider(chain)

        messages = [LLMMessage(role="user", content="test")]
        with pytest.raises(LLMError, match="All providers failed"):
            await provider.chat(messages)

    @pytest.mark.asyncio
    async def test_streaming_not_supported(self):
        primary = _StubLLMProvider(
            LLMConfig(provider=ProviderType.OPENAI, model="gpt-4"),
        )

        chain = ModelFallback(primary=primary)
        provider = FallbackChainProvider(chain)

        messages = [LLMMessage(role="user", content="test")]
        with pytest.raises(NotImplementedError, match="does not support streaming"):
            provider.chat_stream(messages)

    def test_all_providers_property(self):
        primary = _StubLLMProvider(
            LLMConfig(provider=ProviderType.OPENAI, model="gpt-4"),
        )
        fallback = _StubLLMProvider(
            LLMConfig(provider=ProviderType.OLLAMA, model="qwen3:8b"),
        )

        chain = ModelFallback(primary=primary, fallbacks=[fallback])
        assert len(chain.all_providers) == 2
        assert chain.all_providers[0] is primary
        assert chain.all_providers[1] is fallback

    @pytest.mark.asyncio
    async def test_close_closes_all(self):
        primary = _StubLLMProvider(
            LLMConfig(provider=ProviderType.OPENAI, model="gpt-4"),
        )
        fallback = _StubLLMProvider(
            LLMConfig(provider=ProviderType.OLLAMA, model="qwen3:8b"),
        )

        chain = ModelFallback(primary=primary, fallbacks=[fallback])
        provider = FallbackChainProvider(chain)

        # Should not raise
        await provider.close()

    @pytest.mark.asyncio
    async def test_multiple_fallbacks_tried_in_order(self):
        primary = _FailingLLMProvider(
            LLMConfig(provider=ProviderType.OPENAI, model="gpt-4"),
        )
        fb1 = _FailingLLMProvider(
            LLMConfig(provider=ProviderType.ANTHROPIC, model="claude-3"),
        )
        fb2 = _FailingLLMProvider(
            LLMConfig(provider=ProviderType.OLLAMA, model="qwen3:8b-a"),
        )
        fb3 = _StubLLMProvider(
            LLMConfig(provider=ProviderType.OLLAMA, model="qwen3:8b-b"),
        )

        chain = ModelFallback(primary=primary, fallbacks=[fb1, fb2, fb3])
        provider = FallbackChainProvider(chain)

        messages = [LLMMessage(role="user", content="test")]
        response = await provider.chat(messages)

        assert response.model == "qwen3:8b-b"
        assert primary.call_count == 1
        assert fb1.call_count == 1
        assert fb2.call_count == 1
        assert fb3.call_count == 1


# ---------------------------------------------------------------------------
# Logging events
# ---------------------------------------------------------------------------


class TestFallbackLogging:
    @pytest.mark.asyncio
    async def test_fallback_triggered_logged(self, caplog):
        """Fallback triggered event is logged at WARNING level."""
        primary = _FailingLLMProvider(
            LLMConfig(provider=ProviderType.OPENAI, model="gpt-4"),
        )
        fallback = _StubLLMProvider(
            LLMConfig(provider=ProviderType.OLLAMA, model="qwen3:8b"),
        )

        chain = ModelFallback(primary=primary, fallbacks=[fallback])
        provider = FallbackChainProvider(chain)

        with caplog.at_level(logging.WARNING, logger="agent_runtime.llm.fallback"):
            messages = [LLMMessage(role="user", content="test")]
            await provider.chat(messages)

        assert any("Fallback triggered" in r.message for r in caplog.records)

    @pytest.mark.asyncio
    async def test_all_failed_logged(self, caplog):
        """All providers failed event is logged at ERROR level."""
        primary = _FailingLLMProvider(
            LLMConfig(provider=ProviderType.OPENAI, model="gpt-4"),
        )

        chain = ModelFallback(primary=primary, fallbacks=[])
        provider = FallbackChainProvider(chain)

        with caplog.at_level(logging.ERROR, logger="agent_runtime.llm.fallback"):
            messages = [LLMMessage(role="user", content="test")]
            with pytest.raises(LLMError):
                await provider.chat(messages)

        assert any("All providers failed" in r.message for r in caplog.records)

    @pytest.mark.asyncio
    async def test_decision_engine_fallback_logged(self, caplog):
        """DecisionEngine fallback events are logged."""
        primary = _FailingLLMProvider(
            LLMConfig(provider=ProviderType.OPENAI, model="gpt-4"),
        )
        fallback = _StubLLMProvider(
            LLMConfig(provider=ProviderType.OLLAMA, model="qwen3:8b"),
        )

        engine = DecisionEngine(
            provider=primary,
            fallback_providers=[fallback],
        )

        state = _make_state()
        with caplog.at_level(logging.WARNING, logger="agent_runtime.core.decide"):
            await engine.decide(state, _make_perception(), _make_survival())

        assert any("Fallback triggered" in r.message for r in caplog.records)
