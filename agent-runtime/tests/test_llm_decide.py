"""Tests for the LLMDecisionProvider bridge."""

from __future__ import annotations

import json

import pytest

from agent_runtime.core.act import ActionType
from agent_runtime.core.decide import DecisionAction, DecisionPerception
from agent_runtime.core.llm_decide import (
    LLMDecisionProvider,
    _inject_action_args,
    _map_decision_action,
    _perception_to_decision,
    _random_fallback,
    _survival_to_assessment,
)
from agent_runtime.core.think_loop import Decision, Perception
from agent_runtime.llm.base import LLMMessage, LLMResponse, TokenUsage
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
    return Perception(
        tick=42,
        token_balance=500,
        token_ratio=0.5,
        health=80.0,
        active_task=None,
    )


@pytest.fixture
def survival_normal() -> SurvivalAction:
    return SurvivalAction(mode=SurvivalMode.NORMAL, token_ratio=0.5)


@pytest.fixture
def survival_panic() -> SurvivalAction:
    return SurvivalAction(mode=SurvivalMode.PANIC, token_ratio=0.05)


# ---------------------------------------------------------------------------
# Mock LLM Provider
# ---------------------------------------------------------------------------


class MockLLMProvider:
    """Mock LLM provider that returns predetermined JSON decisions."""

    def __init__(self, response_action: str = "rest", reasoning: str = "Test decision"):
        self._response_action = response_action
        self._reasoning = reasoning
        self.last_messages: list[LLMMessage] = []

    async def chat(self, messages: list[LLMMessage], **kwargs) -> LLMResponse:
        self.last_messages = messages
        content = json.dumps({
            "action": self._response_action,
            "parameters": {},
            "reasoning": self._reasoning,
            "confidence": 75,
        })
        return LLMResponse(
            content=content,
            model="test-model",
            usage=TokenUsage(prompt_tokens=10, completion_tokens=20, total_tokens=30),
        )


class FailingLLMProvider:
    """LLM provider that always raises an exception."""

    async def chat(self, messages: list[LLMMessage], **kwargs) -> LLMResponse:
        raise RuntimeError("LLM service unavailable")


# ---------------------------------------------------------------------------
# Tests: type conversion
# ---------------------------------------------------------------------------


class TestPerceptionConversion:
    def test_basic_conversion(self, perception):
        result = _perception_to_decision(perception)
        assert result.tick == 42
        assert result.nearby_agents == []
        assert result.available_tasks == []
        assert result.visible_resources == []
        assert result.recent_events == []

    def test_with_active_task(self):
        p = Perception(tick=10, active_task="task-123")
        result = _perception_to_decision(p)
        assert result.available_tasks == ["task-123"]


class TestSurvivalConversion:
    def test_normal_mode(self, survival_normal):
        result = _survival_to_assessment(survival_normal)
        assert not result.in_danger
        assert result.survival_score == 50
        assert "normal" in result.recommendation

    def test_panic_mode(self, survival_panic):
        result = _survival_to_assessment(survival_panic)
        assert result.in_danger
        assert result.survival_score == 5
        assert result.ticks_until_depletion == 10

    def test_invest_mode(self):
        sa = SurvivalAction(mode=SurvivalMode.INVEST, token_ratio=0.9)
        result = _survival_to_assessment(sa)
        assert not result.in_danger
        assert result.survival_score == 90
        assert result.ticks_until_depletion == 1000


class TestActionMapping:
    def test_rest(self):
        assert _map_decision_action(DecisionAction.REST) == ActionType.REST

    def test_explore(self):
        assert _map_decision_action(DecisionAction.EXPLORE) == ActionType.EXPLORE

    def test_claim_task(self):
        assert _map_decision_action(DecisionAction.CLAIM_TASK) == ActionType.CLAIM_TASK

    def test_respond_message(self):
        assert _map_decision_action(DecisionAction.RESPOND_MESSAGE) == ActionType.SEND_MESSAGE

    def test_trade(self):
        assert _map_decision_action(DecisionAction.TRADE) == ActionType.PROPOSE_DEAL

    def test_move_gather_build_mapped(self):
        # MOVE, GATHER, BUILD now have direct ActionType counterparts
        assert _map_decision_action(DecisionAction.MOVE) == ActionType.MOVE
        assert _map_decision_action(DecisionAction.GATHER) == ActionType.GATHER
        assert _map_decision_action(DecisionAction.BUILD) == ActionType.BUILD

    def test_practice_skill_mapped_directly(self):
        # Regression: PRACTICE_SKILL must NOT map to TEACH_SKILL (which requires
        # a target_agent_id the self-practice scenario never provides).
        assert (
            _map_decision_action(DecisionAction.PRACTICE_SKILL)
            == ActionType.PRACTICE_SKILL
        )


class TestRandomFallback:
    def test_returns_decision(self, agent_state):
        result = _random_fallback(agent_state, [ActionType.REST, ActionType.EXPLORE])
        assert isinstance(result, Decision)
        assert result.action_type in (ActionType.REST, ActionType.EXPLORE)

    def test_always_returns_something(self, agent_state):
        result = _random_fallback(agent_state, [])
        assert result.action_type == ActionType.REST


# ---------------------------------------------------------------------------
# Tests: LLMDecisionProvider
# ---------------------------------------------------------------------------


class TestLLMDecisionProvider:
    @pytest.mark.asyncio
    async def test_successful_llm_decision(self, agent_state, perception, survival_normal):
        mock_llm = MockLLMProvider(response_action="rest", reasoning="Testing rest action")
        provider = LLMDecisionProvider(llm_provider=mock_llm)

        decision = await provider.decide(agent_state, perception, survival_normal)

        assert decision.action_type == ActionType.REST
        assert "Testing rest action" in decision.reasoning
        # Verify the LLM was actually called
        assert len(mock_llm.last_messages) == 1
        assert "TestAgent" in mock_llm.last_messages[0].content

    @pytest.mark.asyncio
    async def test_explore_action(self, agent_state, perception, survival_normal):
        mock_llm = MockLLMProvider(response_action="explore")
        provider = LLMDecisionProvider(llm_provider=mock_llm)

        decision = await provider.decide(agent_state, perception, survival_normal)
        assert decision.action_type == ActionType.EXPLORE

    @pytest.mark.asyncio
    async def test_claim_task_action(self, agent_state, perception, survival_normal):
        mock_llm = MockLLMProvider(response_action="claim_task")
        provider = LLMDecisionProvider(llm_provider=mock_llm)

        decision = await provider.decide(agent_state, perception, survival_normal)
        assert decision.action_type == ActionType.CLAIM_TASK

    @pytest.mark.asyncio
    async def test_trade_maps_to_propose_deal(self, agent_state, perception, survival_normal):
        mock_llm = MockLLMProvider(response_action="trade")
        provider = LLMDecisionProvider(llm_provider=mock_llm)

        decision = await provider.decide(agent_state, perception, survival_normal)
        assert decision.action_type == ActionType.PROPOSE_DEAL

    @pytest.mark.asyncio
    async def test_llm_failure_fallback(self, agent_state, perception, survival_normal):
        mock_llm = FailingLLMProvider()
        provider = LLMDecisionProvider(llm_provider=mock_llm)

        decision = await provider.decide(agent_state, perception, survival_normal)

        # Should fall back to a random action (any affordable action is possible)
        assert isinstance(decision, Decision)
        assert "Fallback" in decision.reasoning or "fallback" in decision.reasoning.lower()

    @pytest.mark.asyncio
    async def test_move_action_mapped_directly(
        self, agent_state, perception, survival_normal
    ):
        mock_llm = MockLLMProvider(response_action="move")
        provider = LLMDecisionProvider(llm_provider=mock_llm)

        decision = await provider.decide(agent_state, perception, survival_normal)
        assert decision.action_type == ActionType.MOVE
        assert "Remapped" not in decision.reasoning

    @pytest.mark.asyncio
    async def test_survival_context_passed_to_llm(
        self, agent_state, perception, survival_panic
    ):
        mock_llm = MockLLMProvider(response_action="rest")
        provider = LLMDecisionProvider(llm_provider=mock_llm)

        await provider.decide(agent_state, perception, survival_panic)

        # The prompt should contain survival-related information
        prompt = mock_llm.last_messages[0].content
        assert "panic" in prompt.lower() or "danger" in prompt.lower() or "10" in prompt

    @pytest.mark.asyncio
    async def test_claim_task_injects_task_id_from_available_tasks(
        self, agent_state, survival_normal
    ):
        """claim_task decision auto-selects a task_id from available_tasks.

        Regression for P2-1: the LLM picks claim_task without naming a task,
        so the bridge injects the first available task into the parameters.
        """
        mock_llm = MockLLMProvider(response_action="claim_task")
        provider = LLMDecisionProvider(llm_provider=mock_llm)
        p = Perception(
            tick=5,
            market_state={"available_tasks": ["task-A", "task-B"]},
        )

        decision = await provider.decide(agent_state, p, survival_normal)

        assert decision.action_type == ActionType.CLAIM_TASK
        assert decision.parameters["task_id"] == "task-A"

    @pytest.mark.asyncio
    async def test_claim_task_without_tasks_leaves_no_task_id(
        self, agent_state, perception, survival_normal
    ):
        """When no tasks are available, no task_id is injected — the action
        handler degrades gracefully to no_tasks_available instead."""
        mock_llm = MockLLMProvider(response_action="claim_task")
        provider = LLMDecisionProvider(llm_provider=mock_llm)

        decision = await provider.decide(agent_state, perception, survival_normal)

        assert decision.action_type == ActionType.CLAIM_TASK
        assert "task_id" not in decision.parameters


class TestInjectActionArgs:
    """Unit tests for the _inject_action_args helper."""

    def test_claim_task_injects_first_task(self):
        params = {}
        perc = DecisionPerception(available_tasks=["t1", "t2"])
        out = _inject_action_args(ActionType.CLAIM_TASK, params, perc)
        assert out["task_id"] == "t1"
        # original dict untouched
        assert params == {}

    def test_claim_task_keeps_existing_task_id(self):
        params = {"task_id": "chosen"}
        perc = DecisionPerception(available_tasks=["t1"])
        out = _inject_action_args(ActionType.CLAIM_TASK, params, perc)
        assert out["task_id"] == "chosen"

    def test_claim_task_no_tasks_no_injection(self):
        params = {}
        perc = DecisionPerception(available_tasks=[])
        out = _inject_action_args(ActionType.CLAIM_TASK, params, perc)
        assert "task_id" not in out

    def test_other_actions_untouched(self):
        params = {"direction": "north"}
        perc = DecisionPerception(available_tasks=["t1"])
        out = _inject_action_args(ActionType.MOVE, params, perc)
        assert out == {"direction": "north"}
