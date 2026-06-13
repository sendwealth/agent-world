"""Tests for the prompt template system (llm/prompts.py)."""

from __future__ import annotations

from agent_runtime.core.decide import DecisionAction
from agent_runtime.llm.prompts import (
    DEFAULT_TEMPLATE,
    SURVIVAL_TEMPLATE,
    PromptTemplate,
    get_template,
)

# ---------------------------------------------------------------------------
# Mock state
# ---------------------------------------------------------------------------


class MockState:
    def __init__(self) -> None:
        self.name = "TestAgent"
        self.id = "agent-001"
        self.phase = _MockPhase("settler")
        self.tokens = 500
        self.money = 50.0
        self.health = 80.0
        self.reputation = 5.0
        self.skills = {}


class _MockPhase:
    def __init__(self, value: str) -> None:
        self.value = value


class MockPerception:
    def __init__(self) -> None:
        self.tick = 42
        self.nearby_agents = ["Alice", "Bob"]
        self.available_tasks = ["task-1", "task-2"]
        self.visible_resources = ["wood", "stone"]
        self.recent_events = ["Alice traded with Bob"]


class MockSurvival:
    def __init__(self) -> None:
        self.ticks_until_depletion = 100
        self.in_danger = False
        self.survival_score = 80
        self.recommendation = "Agent is stable"


# ---------------------------------------------------------------------------
# Tests: PromptTemplate.render
# ---------------------------------------------------------------------------


class TestPromptTemplate:
    def test_render_default_template(self):
        tpl = PromptTemplate(DEFAULT_TEMPLATE)
        result = tpl.render(
            MockState(),
            MockPerception(),
            MockSurvival(),
            [DecisionAction.REST, DecisionAction.EXPLORE],
        )

        assert "TestAgent" in result
        assert "agent-001" in result
        assert "80/100" in result  # health
        assert "500" in result  # tokens
        assert "Alice, Bob" in result
        assert "task-1" in result
        assert "wood, stone" in result
        assert "Alice traded with Bob" in result
        assert "100" in result  # ticks_until_depletion
        assert "rest (cost: 0 tokens)" in result
        assert "explore (cost: 3 tokens)" in result
        assert "JSON object" in result

    def test_render_survival_template(self):
        tpl = PromptTemplate(SURVIVAL_TEMPLATE, name="survival")
        result = tpl.render(
            MockState(),
            MockPerception(),
            MockSurvival(),
            [DecisionAction.REST],
        )

        assert "CRITICAL" in result
        assert "TestAgent" in result
        assert "SAFEST" in result

    def test_render_with_dict_skills(self):
        state = MockState()
        state.skills = {"coding": _MockSkill(3), "trading": _MockSkill(1)}

        tpl = PromptTemplate()
        result = tpl.render(
            state,
            MockPerception(),
            MockSurvival(),
            [DecisionAction.REST],
        )

        assert "coding: level 3" in result
        assert "trading: level 1" in result

    def test_render_empty_perception(self):
        p = MockPerception()
        p.nearby_agents = []
        p.available_tasks = []
        p.visible_resources = []
        p.recent_events = []

        tpl = PromptTemplate()
        result = tpl.render(MockState(), p, MockSurvival(), [DecisionAction.REST])

        assert "None" in result
        assert "None available" in result
        assert "No recent events" in result

    def test_render_high_reputation(self):
        state = MockState()
        state.reputation = 15.0

        tpl = PromptTemplate()
        result = tpl.render(
            state,
            MockPerception(),
            MockSurvival(),
            [DecisionAction.REST],
        )

        assert "You CAN claim high-value tasks" in result

    def test_render_low_reputation(self):
        state = MockState()
        state.reputation = 3.0

        tpl = PromptTemplate()
        result = tpl.render(
            state,
            MockPerception(),
            MockSurvival(),
            [DecisionAction.REST],
        )

        assert "You CANNOT claim high-value tasks" in result


class _MockSkill:
    def __init__(self, level: int) -> None:
        self.level = level


# ---------------------------------------------------------------------------
# Tests: template registry
# ---------------------------------------------------------------------------


class TestTemplateRegistry:
    def test_get_default(self):
        tpl = get_template("default")
        assert tpl == DEFAULT_TEMPLATE

    def test_get_survival(self):
        tpl = get_template("survival")
        assert tpl == SURVIVAL_TEMPLATE

    def test_get_unknown_returns_default(self):
        tpl = get_template("nonexistent")
        assert tpl == DEFAULT_TEMPLATE

    def test_register_custom(self, monkeypatch):
        custom = "Custom template for {name}"
        from agent_runtime.llm import prompts
        original = dict(prompts._TEMPLATES)
        monkeypatch.setattr(prompts, "_TEMPLATES", {**original, "custom": custom})
        assert get_template("custom") == custom
