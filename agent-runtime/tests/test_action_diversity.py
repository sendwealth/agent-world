"""Tests for SEN-684 — agent action diversity fixes (audit P2-2).

Covers the changes that stop agents collapsing into rest/explore:
- TEACH_SKILL decision action + ActionType mapping + target injection
- keyword-match ordering (productive actions before rest) + teach keyword
- weighted fallbacks (productive > rest) in decide.py and llm_decide.py
- decision prompt: behavioral guidance present, rest-only example removed
- action distribution metrics (record_action / snapshot / prometheus)
"""
from __future__ import annotations

import json
import random
from collections import Counter
from dataclasses import dataclass
from enum import StrEnum
from typing import Any

import pytest

from agent_runtime.core.act import ActionType
from agent_runtime.core.decide import (
    _FALLBACK_SAFE_ACTIONS,
    DecisionAction,
    DecisionPerception,
    SurvivalAssessment,
    build_prompt,
    fallback_decision,
    keyword_match_decision,
)
from agent_runtime.core.llm_decide import (
    LLMDecisionProvider,
    _inject_action_args,
    _map_decision_action,
    _random_fallback,
)
from agent_runtime.core.think_loop import Perception
from agent_runtime.llm.base import LLMResponse, TokenUsage
from agent_runtime.models.agent_state import AgentState
from agent_runtime.observability import Metrics
from agent_runtime.survival.instinct import SurvivalAction, SurvivalMode

# --------------------------------------------------------------------------- #
# Mocks
# --------------------------------------------------------------------------- #


class MockPhase(StrEnum):
    EXPLORATION = "exploration"
    DEAD = "dead"


@dataclass
class MockAgentState:
    name: str = "TestAgent"
    id: str = "agent-001"
    phase: MockPhase = MockPhase.EXPLORATION
    tokens: int = 500
    money: float = 100.0
    health: float = 80.0
    reputation: float = 42.0
    skills: Any = None

    def __post_init__(self) -> None:
        if self.skills is None:
            self.skills = {}


class MockLLMProvider:
    """Returns a canned JSON decision."""

    def __init__(self, response_action: str = "rest", reasoning: str = "x") -> None:
        self._action = response_action
        self._reasoning = reasoning

    async def chat(self, messages, **kwargs):  # noqa: ANN001, ANN002, ANN003
        content = json.dumps(
            {
                "action": self._action,
                "parameters": {},
                "reasoning": self._reasoning,
                "confidence": 70,
            }
        )
        return LLMResponse(
            content=content,
            model="test",
            usage=TokenUsage(prompt_tokens=1, completion_tokens=1, total_tokens=2),
        )


# --------------------------------------------------------------------------- #
# TEACH_SKILL action
# --------------------------------------------------------------------------- #


class TestTeachSkillAction:
    def test_teach_skill_exists_and_costs_15(self):
        assert DecisionAction.TEACH_SKILL.value == "teach_skill"
        assert DecisionAction.TEACH_SKILL.token_cost() == 15

    def test_teach_skill_maps_to_action_type(self):
        assert _map_decision_action(DecisionAction.TEACH_SKILL) == ActionType.TEACH_SKILL

    def test_inject_teach_skill_target_from_nearby(self):
        perc = DecisionPerception(nearby_agents=["agent-bob"])
        out = _inject_action_args(ActionType.TEACH_SKILL, {}, perc)
        assert out["target_agent_id"] == "agent-bob"
        assert out["skill_name"] == "trading"

    def test_inject_teach_skill_keeps_provided_skill(self):
        perc = DecisionPerception(nearby_agents=["bob"])
        out = _inject_action_args(
            ActionType.TEACH_SKILL, {"skill_name": "coding"}, perc
        )
        assert out["skill_name"] == "coding"

    def test_inject_teach_skill_no_nearby_no_injection(self):
        perc = DecisionPerception(nearby_agents=[])
        out = _inject_action_args(ActionType.TEACH_SKILL, {}, perc)
        assert "target_agent_id" not in out

    @pytest.mark.asyncio
    async def test_provider_routes_teach_skill_with_target(self):
        provider = LLMDecisionProvider(llm_provider=MockLLMProvider("teach_skill"))
        state = AgentState(name="T", tokens=500, max_tokens=1000)
        perception = Perception(
            tick=1, market_state={"nearby_agents": [{"name": "bob"}]}
        )
        survival = SurvivalAction(mode=SurvivalMode.NORMAL, token_ratio=0.5)
        decision = await provider.decide(state, perception, survival)
        assert decision.action_type == ActionType.TEACH_SKILL
        assert decision.parameters.get("target_agent_id") == "bob"


# --------------------------------------------------------------------------- #
# Keyword-match ordering
# --------------------------------------------------------------------------- #


class TestKeywordOrdering:
    def test_productive_keyword_wins_over_rest(self):
        # "rest" is present, but GATHER must win because rest is matched last.
        text = "I will gather resources for the rest of the day"
        decision = keyword_match_decision(text, [DecisionAction.GATHER, DecisionAction.REST])
        assert decision is not None
        assert decision.action == DecisionAction.GATHER

    def test_teach_keyword_matches(self):
        decision = keyword_match_decision(
            "I should teach bob a skill", [DecisionAction.TEACH_SKILL]
        )
        assert decision is not None
        assert decision.action == DecisionAction.TEACH_SKILL

    def test_rest_still_matches_when_alone(self):
        decision = keyword_match_decision("time to rest", [DecisionAction.REST])
        assert decision is not None
        assert decision.action == DecisionAction.REST


# --------------------------------------------------------------------------- #
# Weighted fallbacks
# --------------------------------------------------------------------------- #


class TestWeightedFallback:
    def test_fallback_decision_prefers_productive_over_rest(self):
        state = MockAgentState(tokens=500)
        actions = DecisionAction.all()
        random.seed(42)
        counts = Counter(fallback_decision(state, actions).action for _ in range(400))
        rest_share = counts[DecisionAction.REST] / 400
        assert rest_share < 0.25, counts
        assert counts[DecisionAction.GATHER] > 0

    def test_fallback_safe_actions_are_productive(self):
        assert DecisionAction.GATHER in _FALLBACK_SAFE_ACTIONS
        assert DecisionAction.MOVE in _FALLBACK_SAFE_ACTIONS
        assert DecisionAction.PRACTICE_SKILL in _FALLBACK_SAFE_ACTIONS

    def test_random_fallback_is_diverse(self):
        state = AgentState(name="T", tokens=500, max_tokens=1000)
        actions = [
            ActionType.GATHER,
            ActionType.MOVE,
            ActionType.PRACTICE_SKILL,
            ActionType.EXPLORE,
            ActionType.REST,
        ]
        random.seed(7)
        picks = Counter(_random_fallback(state, actions).action_type for _ in range(300))
        assert len([k for k, v in picks.items() if v > 0]) >= 3
        assert picks[ActionType.REST] < 300  # not all rest


# --------------------------------------------------------------------------- #
# Prompt diversity
# --------------------------------------------------------------------------- #


class TestPromptDiversity:
    def _prompt(self) -> str:
        return build_prompt(
            MockAgentState(),
            DecisionPerception(),
            SurvivalAssessment(),
            DecisionAction.all(),
        )

    def test_prompt_has_behavioral_guidance(self):
        assert "Behavioral Guidance" in self._prompt()

    def test_prompt_has_no_rest_only_example(self):
        prompt = self._prompt()
        assert '"action": "rest", "parameters": {}' not in prompt

    def test_prompt_lists_teach_skill(self):
        assert "teach_skill (cost: 15 tokens)" in self._prompt()


# --------------------------------------------------------------------------- #
# Action distribution metrics
# --------------------------------------------------------------------------- #


class TestActionDistributionMetrics:
    def test_record_action_tracks_distribution(self):
        m = Metrics()
        m.record_action("gather")
        m.record_action("gather")
        m.record_action("rest")
        assert m.action_distribution == {"gather": 2, "rest": 1}

    def test_snapshot_has_labeled_action_counters(self):
        m = Metrics()
        m.record_action("teach_skill")
        snap = m.snapshot()
        assert snap['agent_action_chosen_total{action="teach_skill"}'] == 1

    def test_prometheus_renders_action_counter_once(self):
        m = Metrics()
        m.record_action("gather")
        m.record_action("move")
        text = m.render_prometheus()
        assert text.count("# TYPE agent_action_chosen_total counter") == 1
        assert 'agent_action_chosen_total{action="gather"} 1' in text
        assert 'agent_action_chosen_total{action="move"} 1' in text
