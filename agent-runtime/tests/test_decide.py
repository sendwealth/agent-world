"""Tests for the decision engine (agent_runtime.core.decide).

Covers:
- DecisionAction (10 actions, token costs, lookup)
- Prompt building (all sections)
- JSON response parsing (valid, fenced, invalid)
- Decision validation (dead agent, insufficient tokens, unavailable action, confidence)
- Fallback strategy (random affordable, zero tokens -> REST)
- DecisionEngine (LLM success, LLM failure fallback, bad JSON fallback)
- Full acceptance test: given state -> valid decision JSON
"""

from __future__ import annotations

import json
from dataclasses import dataclass
from enum import Enum
from typing import Any
from unittest.mock import AsyncMock

import pytest

from agent_runtime.core.decide import (
    Decision,
    DecisionAction,
    DecisionEngine,
    DecisionPerception,
    SurvivalAssessment,
    build_prompt,
    fallback_decision,
    get_available_actions,
    parse_llm_response,
    strip_code_fences,
    validate_decision,
    JsonParseError,
    LlmCallError,
    ValidationError,
)
from agent_runtime.llm.base import LLMResponse, TokenUsage


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


class MockPhase(str, Enum):
    """Mock phase enum matching the shape of AgentPhase."""
    INITIALIZATION = "initialization"
    EXPLORATION = "exploration"
    SURVIVAL = "survival"
    DEVELOPMENT = "development"
    COLLABORATION = "collaboration"
    MASTERY = "mastery"
    DEAD = "dead"


@dataclass
class MockSkill:
    """Mock skill matching Skill protocol."""
    name: str
    level: int = 1


@dataclass
class MockAgentState:
    """Mock agent state for testing."""
    name: str = "TestAgent"
    id: str = "agent-001"
    phase: MockPhase = MockPhase.EXPLORATION
    tokens: int = 500
    money: float = 100.0
    health: float = 80.0
    reputation: float = 42.0
    skills: dict[str, MockSkill] = None

    def __post_init__(self):
        if self.skills is None:
            self.skills = {}


def _make_state(**kwargs: Any) -> MockAgentState:
    """Create a MockAgentState with defaults overridden by kwargs."""
    return MockAgentState(**kwargs)


def _make_perception(**kwargs: Any) -> DecisionPerception:
    return DecisionPerception(**kwargs)


def _make_survival(**kwargs: Any) -> SurvivalAssessment:
    return SurvivalAssessment(**kwargs)


# ---------------------------------------------------------------------------
# DecisionAction tests
# ---------------------------------------------------------------------------


class TestDecisionAction:
    """Tests for DecisionAction enum."""

    def test_all_returns_10_actions(self):
        assert len(DecisionAction.all()) == 10

    def test_action_values_are_snake_case(self):
        for action in DecisionAction.all():
            assert action.value == action.value.lower()
            assert " " not in action.value

    def test_token_costs_non_negative(self):
        for action in DecisionAction.all():
            assert action.token_cost() >= 0

    def test_token_costs_reasonable(self):
        for action in DecisionAction.all():
            assert action.token_cost() <= 100

    def test_rest_is_free(self):
        assert DecisionAction.REST.token_cost() == 0

    def test_specific_token_costs(self):
        assert DecisionAction.RESPOND_MESSAGE.token_cost() == 5
        assert DecisionAction.CLAIM_TASK.token_cost() == 10
        assert DecisionAction.EXPLORE.token_cost() == 15
        assert DecisionAction.TRADE.token_cost() == 10
        assert DecisionAction.PRACTICE_SKILL.token_cost() == 8
        assert DecisionAction.MOVE.token_cost() == 12
        assert DecisionAction.GATHER.token_cost() == 8
        assert DecisionAction.BUILD.token_cost() == 20
        assert DecisionAction.SOCIALIZE.token_cost() == 5

    def test_action_from_name_valid(self):
        for action in DecisionAction.all():
            from agent_runtime.core.decide import _action_from_name
            assert _action_from_name(action.value) == action

    def test_action_from_name_invalid(self):
        from agent_runtime.core.decide import _action_from_name
        assert _action_from_name("fly_to_moon") is None
        assert _action_from_name("") is None


# ---------------------------------------------------------------------------
# Prompt building tests
# ---------------------------------------------------------------------------


class TestBuildPrompt:
    """Tests for build_prompt()."""

    def test_prompt_contains_agent_info(self):
        state = _make_state(name="Alice", tokens=500, money=100.0, reputation=42.0)
        perception = _make_perception()
        survival = _make_survival()
        actions = DecisionAction.all()

        prompt = build_prompt(state, perception, survival, actions)

        assert "Alice" in prompt
        assert "agent-001" in prompt
        assert "500" in prompt
        assert "42.0" in prompt

    def test_prompt_contains_skills(self):
        state = _make_state(skills={"mining": MockSkill("mining", level=5)})
        perception = _make_perception()
        survival = _make_survival()
        actions = DecisionAction.all()

        prompt = build_prompt(state, perception, survival, actions)

        assert "mining" in prompt
        assert "level 5" in prompt

    def test_prompt_contains_perception(self):
        state = _make_state()
        perception = _make_perception(
            tick=42,
            nearby_agents=["agent-bob", "agent-carol"],
            available_tasks=["task-1"],
            visible_resources=["gold"],
            recent_events=["something happened"],
        )
        survival = _make_survival()
        actions = DecisionAction.all()

        prompt = build_prompt(state, perception, survival, actions)

        assert "Tick 42" in prompt
        assert "agent-bob" in prompt
        assert "task-1" in prompt
        assert "gold" in prompt
        assert "something happened" in prompt

    def test_prompt_contains_survival(self):
        state = _make_state()
        perception = _make_perception()
        survival = _make_survival(
            ticks_until_depletion=100,
            in_danger=True,
            survival_score=30,
            recommendation="Seek resources",
        )
        actions = DecisionAction.all()

        prompt = build_prompt(state, perception, survival, actions)

        assert "100" in prompt
        assert "True" in prompt
        assert "30" in prompt
        assert "Seek resources" in prompt

    def test_prompt_contains_available_actions_with_costs(self):
        state = _make_state()
        perception = _make_perception()
        survival = _make_survival()
        actions = [DecisionAction.REST, DecisionAction.EXPLORE]

        prompt = build_prompt(state, perception, survival, actions)

        assert "rest (cost: 0 tokens)" in prompt
        assert "explore (cost: 15 tokens)" in prompt

    def test_prompt_contains_json_format(self):
        state = _make_state()
        perception = _make_perception()
        survival = _make_survival()
        actions = DecisionAction.all()

        prompt = build_prompt(state, perception, survival, actions)

        assert '"action"' in prompt
        assert '"parameters"' in prompt
        assert '"reasoning"' in prompt
        assert '"confidence"' in prompt

    def test_prompt_no_skills_shows_placeholder(self):
        state = _make_state(skills={})
        perception = _make_perception()
        survival = _make_survival()
        actions = DecisionAction.all()

        prompt = build_prompt(state, perception, survival, actions)

        assert "No skills learned yet" in prompt

    def test_prompt_empty_perception_shows_none(self):
        state = _make_state()
        perception = _make_perception()
        survival = _make_survival()
        actions = DecisionAction.all()

        prompt = build_prompt(state, perception, survival, actions)

        assert "None" in prompt
        assert "None available" in prompt
        assert "No recent events" in prompt


# ---------------------------------------------------------------------------
# JSON response parsing tests
# ---------------------------------------------------------------------------


class TestParseLlmResponse:
    """Tests for parse_llm_response()."""

    def test_parse_valid_json(self):
        raw = json.dumps({
            "action": "rest",
            "parameters": {},
            "reasoning": "low on tokens",
            "confidence": 90,
        })
        result = parse_llm_response(raw)

        assert result["action"] == "rest"
        assert result["parameters"] == {}
        assert result["reasoning"] == "low on tokens"
        assert result["confidence"] == 90

    def test_parse_json_with_parameters(self):
        raw = json.dumps({
            "action": "respond_message",
            "parameters": {"target": "agent-2", "message": "hello"},
            "reasoning": "greet neighbor",
            "confidence": 75,
        })
        result = parse_llm_response(raw)

        assert result["action"] == "respond_message"
        assert result["parameters"]["target"] == "agent-2"
        assert result["parameters"]["message"] == "hello"

    def test_parse_json_with_code_fences(self):
        raw = '```json\n{"action": "rest", "parameters": {}, "reasoning": "test", "confidence": 50}\n```'
        result = parse_llm_response(raw)

        assert result["action"] == "rest"

    def test_parse_json_with_plain_code_fences(self):
        raw = '```\n{"action": "rest", "parameters": {}, "reasoning": "test", "confidence": 50}\n```'
        result = parse_llm_response(raw)

        assert result["action"] == "rest"

    def test_parse_invalid_json_raises(self):
        with pytest.raises(JsonParseError, match="Failed to parse"):
            parse_llm_response("this is not json")

    def test_parse_missing_action_raises(self):
        raw = json.dumps({"parameters": {}, "reasoning": "test"})
        with pytest.raises(JsonParseError, match="action"):
            parse_llm_response(raw)

    def test_parse_unknown_action_raises(self):
        raw = json.dumps({"action": "fly_to_moon", "parameters": {}})
        with pytest.raises(JsonParseError, match="Unknown action"):
            parse_llm_response(raw)

    def test_parse_confidence_clamped_high(self):
        raw = json.dumps({"action": "rest", "confidence": 200})
        result = parse_llm_response(raw)
        assert result["confidence"] == 100

    def test_parse_confidence_clamped_low(self):
        raw = json.dumps({"action": "rest", "confidence": -10})
        result = parse_llm_response(raw)
        assert result["confidence"] == 0

    def test_parse_default_confidence(self):
        raw = json.dumps({"action": "rest", "parameters": {}, "reasoning": "test"})
        result = parse_llm_response(raw)
        assert result["confidence"] == 50

    def test_parse_default_parameters(self):
        raw = json.dumps({"action": "rest", "reasoning": "test", "confidence": 50})
        result = parse_llm_response(raw)
        assert result["parameters"] == {}

    def test_parse_default_reasoning(self):
        raw = json.dumps({"action": "rest", "confidence": 50})
        result = parse_llm_response(raw)
        assert result["reasoning"] == ""


# ---------------------------------------------------------------------------
# strip_code_fences tests
# ---------------------------------------------------------------------------


class TestStripCodeFences:
    """Tests for strip_code_fences()."""

    def test_strip_json_fence(self):
        input_text = '```json\n{"key": "value"}\n```'
        assert strip_code_fences(input_text) == '{"key": "value"}'

    def test_strip_plain_fence(self):
        input_text = '```\n{"key": "value"}\n```'
        assert strip_code_fences(input_text) == '{"key": "value"}'

    def test_no_fence(self):
        input_text = '{"key": "value"}'
        assert strip_code_fences(input_text) == '{"key": "value"}'

    def test_unclosed_fence(self):
        input_text = '```json\n{"key": "value"}'
        result = strip_code_fences(input_text)
        assert result.startswith("{")


# ---------------------------------------------------------------------------
# Decision validation tests
# ---------------------------------------------------------------------------


class TestValidateDecision:
    """Tests for validate_decision()."""

    def test_valid_decision_passes(self):
        state = _make_state(tokens=500)
        decision = Decision(action=DecisionAction.REST, confidence=80)
        actions = DecisionAction.all()

        # Should not raise
        validate_decision(decision, state, actions)

    def test_dead_agent_rejected(self):
        state = _make_state(tokens=500, phase=MockPhase.DEAD)
        decision = Decision(action=DecisionAction.REST, confidence=50)
        actions = DecisionAction.all()

        with pytest.raises(ValidationError, match="dead"):
            validate_decision(decision, state, actions)

    def test_insufficient_tokens(self):
        state = _make_state(tokens=2)  # Not enough for EXPLORE (15)
        decision = Decision(action=DecisionAction.EXPLORE, confidence=50)
        actions = DecisionAction.all()

        with pytest.raises(ValidationError, match="Insufficient tokens"):
            validate_decision(decision, state, actions)

    def test_action_not_available(self):
        state = _make_state(tokens=500)
        decision = Decision(action=DecisionAction.EXPLORE, confidence=50)
        actions = [DecisionAction.REST]  # Only REST available

        with pytest.raises(ValidationError, match="not available"):
            validate_decision(decision, state, actions)

    def test_confidence_over_100(self):
        state = _make_state(tokens=500)
        decision = Decision(action=DecisionAction.REST, confidence=150)
        actions = DecisionAction.all()

        with pytest.raises(ValidationError, match="confidence"):
            validate_decision(decision, state, actions)

    def test_confidence_negative(self):
        state = _make_state(tokens=500)
        decision = Decision(action=DecisionAction.REST, confidence=-1)
        actions = DecisionAction.all()

        with pytest.raises(ValidationError, match="confidence"):
            validate_decision(decision, state, actions)


# ---------------------------------------------------------------------------
# Fallback strategy tests
# ---------------------------------------------------------------------------


class TestFallbackDecision:
    """Tests for fallback_decision()."""

    def test_fallback_returns_valid_action(self):
        state = _make_state(tokens=500)
        actions = DecisionAction.all()

        decision = fallback_decision(state, actions)

        assert decision.action in DecisionAction.all()
        assert decision.confidence == 0
        assert "Fallback" in decision.reasoning

    def test_fallback_zero_tokens_rests(self):
        state = _make_state(tokens=0)
        actions = [DecisionAction.REST]

        decision = fallback_decision(state, actions)

        assert decision.action == DecisionAction.REST

    def test_fallback_zero_tokens_no_free_actions_rests(self):
        state = _make_state(tokens=0)
        # Even if available actions has non-free ones, fallback finds affordable
        actions = DecisionAction.all()

        decision = fallback_decision(state, actions)

        # Should be REST (only free action)
        assert decision.action == DecisionAction.REST

    def test_fallback_with_limited_tokens(self):
        state = _make_state(tokens=5)
        actions = DecisionAction.all()

        decision = fallback_decision(state, actions)

        # Must be an action costing <= 5 tokens
        assert decision.action.token_cost() <= 5


# ---------------------------------------------------------------------------
# Available actions tests
# ---------------------------------------------------------------------------


class TestGetAvailableActions:
    """Tests for get_available_actions()."""

    def test_rich_agent_has_all_10_actions(self):
        state = _make_state(tokens=10000)
        actions = get_available_actions(state)
        assert len(actions) == 10

    def test_low_token_agent_filters_expensive(self):
        state = _make_state(tokens=5)
        actions = get_available_actions(state)
        # REST=0, RESPOND_MESSAGE=5, SOCIALIZE=5
        assert DecisionAction.REST in actions
        assert DecisionAction.EXPLORE not in actions  # costs 15
        assert DecisionAction.BUILD not in actions  # costs 20

    def test_dead_agent_has_no_actions(self):
        state = _make_state(tokens=500, phase=MockPhase.DEAD)
        actions = get_available_actions(state)
        assert actions == []


# ---------------------------------------------------------------------------
# DecisionEngine integration tests
# ---------------------------------------------------------------------------


class TestDecisionEngine:
    """Tests for DecisionEngine."""

    def _make_engine(self, content: str) -> DecisionEngine:
        """Create a DecisionEngine with a mock LLM that returns the given content."""
        mock_provider = AsyncMock()
        mock_provider.chat.return_value = LLMResponse(
            content=content,
            model="test-model",
            usage=TokenUsage(),
        )
        return DecisionEngine(provider=mock_provider)

    def _make_failing_engine(self) -> DecisionEngine:
        """Create a DecisionEngine with a mock LLM that always fails."""
        mock_provider = AsyncMock()
        mock_provider.chat.side_effect = RuntimeError("LLM unavailable")
        return DecisionEngine(provider=mock_provider)

    @pytest.mark.asyncio
    async def test_decide_returns_llm_result(self):
        engine = self._make_engine(json.dumps({
            "action": "rest",
            "parameters": {},
            "reasoning": "conserving tokens",
            "confidence": 90,
        }))
        state = _make_state(tokens=500)
        perception = _make_perception()
        survival = _make_survival()

        decision = await engine.decide(state, perception, survival)

        assert decision.action == DecisionAction.REST
        assert decision.confidence == 90
        assert decision.reasoning == "conserving tokens"

    @pytest.mark.asyncio
    async def test_decide_with_parameters(self):
        engine = self._make_engine(json.dumps({
            "action": "respond_message",
            "parameters": {"target": "agent-2"},
            "reasoning": "greeting",
            "confidence": 75,
        }))
        state = _make_state(tokens=500)
        perception = _make_perception()
        survival = _make_survival()

        decision = await engine.decide(state, perception, survival)

        assert decision.action == DecisionAction.RESPOND_MESSAGE
        assert decision.parameters["target"] == "agent-2"

    @pytest.mark.asyncio
    async def test_decide_falls_back_on_llm_failure(self):
        engine = self._make_failing_engine()
        state = _make_state(tokens=500)
        perception = _make_perception()
        survival = _make_survival()

        decision = await engine.decide(state, perception, survival)

        assert decision.action in DecisionAction.all()
        assert decision.confidence == 0
        assert "Fallback" in decision.reasoning

    @pytest.mark.asyncio
    async def test_decide_falls_back_on_bad_json(self):
        engine = self._make_engine("not json at all")
        state = _make_state(tokens=500)
        perception = _make_perception()
        survival = _make_survival()

        decision = await engine.decide(state, perception, survival)

        assert decision.action in DecisionAction.all()
        assert decision.confidence == 0

    @pytest.mark.asyncio
    async def test_decide_falls_back_on_invalid_action(self):
        engine = self._make_engine(json.dumps({
            "action": "fly_to_moon",
            "parameters": {},
            "reasoning": "escape!",
            "confidence": 50,
        }))
        state = _make_state(tokens=500)
        perception = _make_perception()
        survival = _make_survival()

        decision = await engine.decide(state, perception, survival)

        assert decision.action in DecisionAction.all()
        assert decision.confidence == 0

    @pytest.mark.asyncio
    async def test_decide_dead_agent_returns_rest(self):
        engine = self._make_failing_engine()
        state = _make_state(tokens=500, phase=MockPhase.DEAD)
        perception = _make_perception()
        survival = _make_survival()

        decision = await engine.decide(state, perception, survival)

        assert decision.action == DecisionAction.REST
        assert "No available actions" in decision.reasoning

    @pytest.mark.asyncio
    async def test_decide_zero_tokens_fallback_rests(self):
        engine = self._make_failing_engine()
        state = _make_state(tokens=0)
        perception = _make_perception()
        survival = _make_survival()

        decision = await engine.decide(state, perception, survival)

        assert decision.action == DecisionAction.REST


# ---------------------------------------------------------------------------
# Full acceptance test
# ---------------------------------------------------------------------------


class TestAcceptance:
    """Acceptance test: given a state, generate valid decision JSON."""

    @pytest.mark.asyncio
    async def test_full_decision_json_generation(self):
        """Given a state, the engine should produce a valid decision JSON."""
        # Setup agent with realistic state
        state = _make_state(
            name="Alice",
            tokens=500,
            money=100.0,
            health=80.0,
            reputation=42.0,
            skills={
                "mining": MockSkill("mining", level=25),
                "trading": MockSkill("trading", level=10),
            },
        )
        perception = _make_perception(
            tick=100,
            nearby_agents=["agent-002", "agent-003"],
            available_tasks=["task-mine-gold", "task-build-house"],
            visible_resources=["gold_ore", "wood"],
            recent_events=["agent-002 offered trade"],
        )
        survival = _make_survival(
            ticks_until_depletion=250,
            in_danger=False,
            survival_score=80,
            recommendation="Agent is stable",
        )

        # Mock LLM returning a gather decision
        mock_provider = AsyncMock()
        mock_provider.chat.return_value = LLMResponse(
            content=json.dumps({
                "action": "gather",
                "parameters": {"resource": "gold_ore"},
                "reasoning": "Gold ore is visible and I have mining skill",
                "confidence": 85,
            }),
            model="test-model",
            usage=TokenUsage(prompt_tokens=100, completion_tokens=50, total_tokens=150),
        )

        engine = DecisionEngine(provider=mock_provider)
        decision = await engine.decide(state, perception, survival)

        # Verify the decision is valid
        assert decision.action == DecisionAction.GATHER
        assert decision.parameters["resource"] == "gold_ore"
        assert decision.confidence == 85
        assert 0 <= decision.confidence <= 100
        assert decision.action in DecisionAction.all()

        # Verify decision serializes to valid JSON
        decision_dict = {
            "action": decision.action.value,
            "parameters": decision.parameters,
            "reasoning": decision.reasoning,
            "confidence": decision.confidence,
        }
        decision_json = json.dumps(decision_dict)
        parsed_back = json.loads(decision_json)
        assert parsed_back["action"] == "gather"
        assert parsed_back["confidence"] == 85

    @pytest.mark.asyncio
    async def test_acceptance_all_10_actions_available_to_rich_agent(self):
        """A rich agent should have all 10 actions available."""
        state = _make_state(tokens=10000)
        actions = get_available_actions(state)
        assert len(actions) == 10
        assert set(actions) == set(DecisionAction.all())
