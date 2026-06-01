"""Tests for model hot-swap — ModelRegistry.hot_swap_model + ThinkLoop integration.

Covers:
- hot_swap_model updates registry and bumps version
- hot_swap_model rejects unknown provider
- ThinkLoop picks up swap on next tick
- Hot-swap does not crash the think loop or lose ticks
- Concurrent hot-swaps for different agents don't conflict
- POST /api/v1/runtime/swap-model endpoint
"""

from __future__ import annotations

import asyncio
import json
from typing import Any
from unittest.mock import AsyncMock, MagicMock, patch

import pytest

from agent_runtime.core.act import ActionExecutor
from agent_runtime.core.decide import (
    DecisionAction,
    DecisionEngine,
    DecisionPerception,
    SurvivalAssessment,
)
from agent_runtime.core.think_loop import ThinkLoop, ThinkLoopConfig
from agent_runtime.llm.base import LLMConfig, LLMMessage, LLMProvider, LLMResponse, ProviderType
from agent_runtime.llm.provider_registry import ModelRegistry, ProviderConfig, ProviderProtocol
from agent_runtime.models.agent_state import AgentState
from agent_runtime.survival.instinct import SurvivalInstinct


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


@pytest.fixture(autouse=True)
def _reset_registry():
    ModelRegistry.reset()
    yield
    ModelRegistry.reset()


def _make_state(**overrides) -> AgentState:
    defaults = dict(name="TestAgent", tokens=500, max_tokens=1000)
    defaults.update(overrides)
    return AgentState(**defaults)


class _StubLLMProvider(LLMProvider):
    """Deterministic LLM provider for testing — returns a fixed decision."""

    def __init__(self, config: LLMConfig, response_text: str = '{"action": "rest", "parameters": {}, "reasoning": "stub", "confidence": 80}') -> None:
        super().__init__(config)
        self._response_text = response_text
        self.chat_calls: list[list[LLMMessage]] = []

    async def chat(
        self,
        messages: list[LLMMessage],
        *,
        max_tokens: int | None = None,
        temperature: float | None = None,
    ) -> LLMResponse:
        self.chat_calls.append(messages)
        return LLMResponse(
            content=self._response_text,
            model=self._config.model,
        )

    def chat_stream(self, messages, **kwargs):
        raise NotImplementedError


class _FailingLLMProvider(LLMProvider):
    """LLM provider that always raises."""

    def __init__(self, config: LLMConfig) -> None:
        super().__init__(config)

    async def chat(self, messages, **kwargs):
        raise RuntimeError(f"LLM failed: {self._config.model}")

    def chat_stream(self, messages, **kwargs):
        raise NotImplementedError


# ---------------------------------------------------------------------------
# ModelRegistry.hot_swap_model
# ---------------------------------------------------------------------------


class TestHotSwapRegistry:
    def test_hot_swap_updates_registry(self):
        reg = ModelRegistry.instance()
        reg.hot_swap_model("agent-1", "openai", "gpt-4o")

        assert reg.get_agent_model("agent-1") == "openai"
        override = reg.get_agent_model_override("agent-1")
        assert override == ("openai", "gpt-4o")

    def test_hot_swap_bumps_version(self):
        reg = ModelRegistry.instance()
        v0 = reg.get_agent_models_version()
        reg.hot_swap_model("agent-1", "openai", "gpt-4o")
        v1 = reg.get_agent_models_version()
        assert v1 > v0

        reg.hot_swap_model("agent-1", "anthropic", "claude-3-sonnet")
        v2 = reg.get_agent_models_version()
        assert v2 > v1

    def test_hot_swap_rejects_unknown_provider(self):
        reg = ModelRegistry.instance()
        with pytest.raises(KeyError, match="not registered"):
            reg.hot_swap_model("agent-1", "nonexistent", "model-x")

    def test_hot_swap_different_agents_independent(self):
        reg = ModelRegistry.instance()
        reg.hot_swap_model("agent-1", "openai", "gpt-4o")
        reg.hot_swap_model("agent-2", "anthropic", "claude-3-sonnet")

        assert reg.get_agent_model("agent-1") == "openai"
        assert reg.get_agent_model("agent-2") == "anthropic"
        assert reg.get_agent_model_override("agent-1") == ("openai", "gpt-4o")
        assert reg.get_agent_model_override("agent-2") == ("anthropic", "claude-3-sonnet")

    def test_hot_swap_same_agent_replaces(self):
        reg = ModelRegistry.instance()
        reg.hot_swap_model("agent-1", "openai", "gpt-4o")
        reg.hot_swap_model("agent-1", "anthropic", "claude-3-sonnet")

        assert reg.get_agent_model("agent-1") == "anthropic"
        assert reg.get_agent_model_override("agent-1") == ("anthropic", "claude-3-sonnet")

    def test_no_override_returns_none(self):
        reg = ModelRegistry.instance()
        assert reg.get_agent_model_override("unknown-agent") is None


# ---------------------------------------------------------------------------
# ThinkLoop hot-swap integration
# ---------------------------------------------------------------------------


class TestThinkLoopHotSwap:
    def _build_loop(
        self,
        provider: LLMProvider,
        *,
        model_registry: ModelRegistry | None = None,
        max_ticks: int = 5,
    ) -> ThinkLoop:
        from agent_runtime.core.llm_decide import LLMDecisionProvider
        from agent_runtime.core.think_loop import MockDecisionProvider

        state = _make_state()
        survival = SurvivalInstinct()
        executor = ActionExecutor()

        decision_provider = LLMDecisionProvider(llm_provider=provider)

        config = ThinkLoopConfig(tick_interval=0.0, max_ticks=max_ticks)
        return ThinkLoop(
            state=state,
            survival=survival,
            executor=executor,
            config=config,
            decision_provider=decision_provider,
            model_registry=model_registry,
        )

    @pytest.mark.asyncio
    async def test_swap_detected_on_next_tick(self):
        """After hot_swap_model, the ThinkLoop detects the version change."""
        reg = ModelRegistry.instance()

        # Use stub provider to avoid real API calls
        initial_provider = _StubLLMProvider(
            LLMConfig(provider=ProviderType.OPENAI, model="gpt-4"),
        )

        loop = self._build_loop(initial_provider, model_registry=reg, max_ticks=5)

        # Capture the initial version
        initial_version = reg.get_agent_models_version()

        # Run one tick
        await loop.run(max_ticks=1)
        assert loop.tick == 1

        # Now hot-swap — use a mock create_provider to avoid real HTTP
        reg.hot_swap_model(str(loop.state.id), "openai", "gpt-4o")
        assert reg.get_agent_models_version() > initial_version

        # Patch ModelRegistry.create_provider to return a stub
        with patch.object(reg, "create_provider", return_value=_StubLLMProvider(
            LLMConfig(provider=ProviderType.OPENAI, model="gpt-4o"),
        )):
            # Run more ticks — total should reach 4
            await loop.run(max_ticks=4)
            assert loop.tick == 4
            assert loop.total_errors == 0

    @pytest.mark.asyncio
    async def test_no_crash_on_invalid_swap(self):
        """If swap refers to an unparseable provider, loop continues."""
        reg = ModelRegistry.instance()
        provider = _StubLLMProvider(
            LLMConfig(provider=ProviderType.OPENAI, model="gpt-4"),
        )

        loop = self._build_loop(provider, model_registry=reg, max_ticks=3)
        await loop.run(max_ticks=3)
        assert loop.tick == 3
        assert loop.total_errors == 0

    @pytest.mark.asyncio
    async def test_no_swap_without_registry(self):
        """ThinkLoop without model_registry works as before."""
        provider = _StubLLMProvider(
            LLMConfig(provider=ProviderType.OPENAI, model="gpt-4"),
        )
        loop = self._build_loop(provider, model_registry=None, max_ticks=5)
        await loop.run(max_ticks=5)
        assert loop.tick == 5
        assert loop.total_errors == 0


# ---------------------------------------------------------------------------
# Concurrent hot-swap
# ---------------------------------------------------------------------------


class TestConcurrentHotSwap:
    @pytest.mark.asyncio
    async def test_two_agents_swap_independently(self):
        """Two agents swapping models concurrently don't interfere."""
        reg = ModelRegistry.instance()

        agent1 = _make_state(name="Agent-1")
        agent2 = _make_state(name="Agent-2")

        provider1 = _StubLLMProvider(
            LLMConfig(provider=ProviderType.OPENAI, model="gpt-4"),
        )
        provider2 = _StubLLMProvider(
            LLMConfig(provider=ProviderType.OLLAMA, model="qwen3:8b"),
        )

        from agent_runtime.core.llm_decide import LLMDecisionProvider

        config = ThinkLoopConfig(tick_interval=0.0, max_ticks=3)

        loop1 = ThinkLoop(
            state=agent1,
            survival=SurvivalInstinct(),
            executor=ActionExecutor(),
            config=config,
            decision_provider=LLMDecisionProvider(llm_provider=provider1),
            model_registry=reg,
        )
        loop2 = ThinkLoop(
            state=agent2,
            survival=SurvivalInstinct(),
            executor=ActionExecutor(),
            config=config,
            decision_provider=LLMDecisionProvider(llm_provider=provider2),
            model_registry=reg,
        )

        # Hot-swap both agents
        reg.hot_swap_model(str(agent1.id), "anthropic", "claude-3-sonnet")
        reg.hot_swap_model(str(agent2.id), "openai", "gpt-4o")

        # Run both loops
        await asyncio.gather(
            loop1.run(max_ticks=3),
            loop2.run(max_ticks=3),
        )

        assert loop1.tick == 3
        assert loop2.tick == 3
        assert loop1.total_errors == 0
        assert loop2.total_errors == 0

        # Each agent has its own override
        o1 = reg.get_agent_model_override(str(agent1.id))
        o2 = reg.get_agent_model_override(str(agent2.id))
        assert o1 == ("anthropic", "claude-3-sonnet")
        assert o2 == ("openai", "gpt-4o")


# ---------------------------------------------------------------------------
# Swap-model API endpoint
# ---------------------------------------------------------------------------


class TestSwapModelEndpoint:
    def _make_health_server(self, think_loop: ThinkLoop) -> Any:
        """Create a HealthCheckServer with access to _handle_swap_model."""
        from agent_runtime.__main__ import HealthCheckServer
        return HealthCheckServer(
            agent_name="TestAgent",
            think_loop=think_loop,
            port=9999,
        )

    def test_valid_swap_request(self):
        reg = ModelRegistry.instance()
        provider = _StubLLMProvider(
            LLMConfig(provider=ProviderType.OPENAI, model="gpt-4"),
        )
        state = _make_state()
        loop = ThinkLoop(
            state=state,
            survival=SurvivalInstinct(),
            executor=ActionExecutor(),
            config=ThinkLoopConfig(tick_interval=0.0),
            decision_provider=MagicMock(),
        )

        server = self._make_health_server(loop)
        body = json.dumps({
            "agent_id": str(state.id),
            "provider_id": "openai",
            "model": "gpt-4o",
        }).encode()

        response = server._handle_swap_model(body)
        assert "200 OK" in response
        assert "gpt-4o" in response

        # Verify registry was updated
        override = reg.get_agent_model_override(str(state.id))
        assert override == ("openai", "gpt-4o")

    def test_missing_fields_returns_400(self):
        state = _make_state()
        loop = ThinkLoop(
            state=state,
            survival=SurvivalInstinct(),
            executor=ActionExecutor(),
            config=ThinkLoopConfig(tick_interval=0.0),
            decision_provider=MagicMock(),
        )

        server = self._make_health_server(loop)
        body = json.dumps({"agent_id": "x"}).encode()

        response = server._handle_swap_model(body)
        assert "400" in response
        assert "Missing required" in response

    def test_unknown_provider_returns_404(self):
        state = _make_state()
        loop = ThinkLoop(
            state=state,
            survival=SurvivalInstinct(),
            executor=ActionExecutor(),
            config=ThinkLoopConfig(tick_interval=0.0),
            decision_provider=MagicMock(),
        )

        server = self._make_health_server(loop)
        body = json.dumps({
            "agent_id": str(state.id),
            "provider_id": "nonexistent",
            "model": "x",
        }).encode()

        response = server._handle_swap_model(body)
        assert "404" in response

    def test_invalid_json_returns_400(self):
        state = _make_state()
        loop = ThinkLoop(
            state=state,
            survival=SurvivalInstinct(),
            executor=ActionExecutor(),
            config=ThinkLoopConfig(tick_interval=0.0),
            decision_provider=MagicMock(),
        )

        server = self._make_health_server(loop)
        response = server._handle_swap_model(b"not json")
        assert "400" in response
