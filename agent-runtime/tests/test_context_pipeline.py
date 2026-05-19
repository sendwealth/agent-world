"""Tests for Context Engine Pipeline integration with DecisionEngine.

Covers:
- DecisionEngine with pipeline=None uses build_prompt (default behaviour)
- DecisionEngine with pipeline uses pipeline.run output
- Pipeline fallback: when pipeline is set, its formatted_context is used
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any
from unittest.mock import AsyncMock

import pytest

from agent_runtime.context.engine import (
    ContextEnginePipeline,
    ContextItem,
    ContextPriority,
    ContextSource,
    PipelineConfig,
    PipelineResult,
    PipelineStats,
)
from agent_runtime.core.decide import (
    DecisionAction,
    DecisionEngine,
    DecisionPerception,
    SurvivalAssessment,
)


# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------


@dataclass
class FakeState:
    name: str = "TestAgent"
    id: str = "agent-1"
    phase: Any = None
    tokens: int = 500
    max_tokens: int = 1000
    money: float = 50.0
    health: float = 100.0
    reputation: float = 10.0
    skills: Any = None


class FakePhase:
    value = "adult"


@dataclass
class FakeLLMResponse:
    content: str


def _make_provider(response_content: str) -> AsyncMock:
    provider = AsyncMock()
    provider.chat.return_value = FakeLLMResponse(content=response_content)
    return provider


def _valid_decision_json(action: str = "rest") -> str:
    return (
        '{"action": "rest", "parameters": {}, '
        '"reasoning": "test", "confidence": 80}'
    )


# ---------------------------------------------------------------------------
# Tests
# ---------------------------------------------------------------------------


class TestDecisionEngineWithoutPipeline:
    """Verify that DecisionEngine without pipeline uses build_prompt."""

    @pytest.mark.asyncio
    async def test_no_pipeline_uses_build_prompt(self) -> None:
        provider = _make_provider(_valid_decision_json())
        engine = DecisionEngine(provider)

        state = FakeState(phase=FakePhase())
        perception = DecisionPerception(tick=1)
        survival = SurvivalAssessment()

        decision = await engine.decide(state, perception, survival)
        assert decision.action == DecisionAction.REST
        assert decision.confidence == 80

        # Verify provider was called (build_prompt was used)
        provider.chat.assert_called_once()
        call_args = provider.chat.call_args
        prompt = call_args[0][0][0].content
        # build_prompt produces a template-based prompt with agent identity
        assert "TestAgent" in prompt
        assert "You are" in prompt


class TestDecisionEngineWithPipeline:
    """Verify that DecisionEngine with pipeline uses pipeline.run output."""

    @pytest.mark.asyncio
    async def test_with_pipeline_uses_pipeline_context(self) -> None:
        provider = _make_provider(_valid_decision_json())

        # Create a pipeline that will produce context
        pipeline = ContextEnginePipeline(
            config=PipelineConfig(max_tokens=4096),
        )
        engine = DecisionEngine(provider, pipeline=pipeline)

        state = FakeState(phase=FakePhase())
        perception = DecisionPerception(tick=1)
        survival = SurvivalAssessment()

        decision = await engine.decide(state, perception, survival)
        assert decision.action == DecisionAction.REST

        # The prompt should come from pipeline, not build_prompt
        provider.chat.assert_called_once()
        call_args = provider.chat.call_args
        prompt = call_args[0][0][0].content
        # Pipeline output contains state info but NOT the build_prompt template
        assert "Agent state:" in prompt or "Tick" in prompt

    @pytest.mark.asyncio
    async def test_pipeline_fallback_on_llm_failure(self) -> None:
        provider = AsyncMock()
        provider.chat.side_effect = RuntimeError("LLM unavailable")

        pipeline = ContextEnginePipeline(
            config=PipelineConfig(max_tokens=4096),
        )
        engine = DecisionEngine(provider, pipeline=pipeline)

        state = FakeState(phase=FakePhase())
        perception = DecisionPerception(tick=1)
        survival = SurvivalAssessment()

        # Should fallback to random decision
        decision = await engine.decide(state, perception, survival)
        assert isinstance(decision.action, DecisionAction)
        assert decision.confidence == 0

    @pytest.mark.asyncio
    async def test_pipeline_produces_token_budgeted_output(self) -> None:
        """Verify pipeline keeps output within token budget."""
        pipeline = ContextEnginePipeline(
            config=PipelineConfig(max_tokens=200),
        )

        # Create perception with many messages
        msgs = [
            {"type": "INFORM", "payload": {"content": f"Message {i} " + "x" * 100}, "trust_score": 0.5}
            for i in range(50)
        ]
        result = pipeline.run(
            perception=type("P", (), {
                "messages": msgs,
                "market_state": {},
                "tick": 1,
                "health": 100.0,
                "token_ratio": 0.5,
            })(),
            survival=type("S", (), {
                "mode": type("M", (), {"value": "normal"})(),
                "token_ratio": 0.5,
                "actions": [],
            })(),
            state=FakeState(phase=FakePhase()),
        )
        assert result.stats.final_token_count <= 200
        assert result.stats.items_trimmed > 0

    @pytest.mark.asyncio
    async def test_pipeline_survival_items_never_filtered(self) -> None:
        """Verify SURVIVAL items are always kept even with tight budget."""
        pipeline = ContextEnginePipeline(
            config=PipelineConfig(max_tokens=50),
        )

        result = pipeline.run(
            survival=type("S", (), {
                "mode": type("M", (), {"value": "critical"})(),
                "token_ratio": 0.05,
                "actions": [],
            })(),
            state=FakeState(health=10.0, tokens=50, max_tokens=1000, phase=FakePhase()),
        )

        survival_items = [i for i in result.items if i.source == ContextSource.SURVIVAL]
        assert len(survival_items) >= 1
        assert all(i.protected for i in survival_items)
