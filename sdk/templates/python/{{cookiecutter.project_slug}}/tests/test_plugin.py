"""Tests for {{ cookiecutter.project_name }} plugin."""

import json
import sys
import os

# Add src to path for testing without installation
sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "src"))

from {{ cookiecutter.project_slug }} import (
    {{ cookiecutter.project_slug.replace('_', ' ').title().replace(' ', '') }}Plugin as Plugin,
    ActionContext,
    WorldContext,
    AgentSnapshot,
)


def mock_action_context() -> ActionContext:
    return ActionContext(
        world=WorldContext(
            tick=42,
            agent=AgentSnapshot(
                id="agent-001",
                name="Alice",
                phase="adult",
                money=1000,
                tokens=500,
                reputation=50.0,
                skills={},
                alive=True,
                age=10,
            ),
            visible_agents=[],
            globals={},
            recent_events=[],
        ),
        params={},
        config={"greeting": "Hi"},
    )


class TestPluginInit:
    def test_init_returns_info(self):
        config = {"greeting": "Hey"}
        info = Plugin.init(config)
        assert info.id == "{{ cookiecutter.plugin_id }}"
        assert info.name == "{{ cookiecutter.project_name }}"
        assert info.version == "0.1.0"

    def test_init_default_greeting(self):
        info = Plugin.init({})
        assert info.id == "{{ cookiecutter.plugin_id }}"


class TestPluginRegister:
    def test_register_returns_skills(self):
        skills = Plugin.register()
        assert skills == ["{{ cookiecutter.skill_id }}"]


class TestPluginExecute:
    def test_execute_success(self):
        ctx = mock_action_context()
        result = Plugin.execute(ctx)
        assert result.success is True
        assert "Alice" in result.message
        assert "Hi" in result.message
        assert result.tokens_consumed == 1

    def test_execute_without_agent(self):
        ctx = mock_action_context()
        ctx.world.agent = None
        result = Plugin.execute(ctx)
        assert result.success is True
        assert "stranger" in result.message


class TestPluginCostEstimate:
    def test_cost_estimate(self):
        ctx = mock_action_context()
        cost = Plugin.cost_estimate(ctx)
        assert cost.estimated == 1
        assert cost.confidence == 1.0

    def test_cost_estimate_has_breakdown(self):
        ctx = mock_action_context()
        cost = Plugin.cost_estimate(ctx)
        assert cost.breakdown is not None


class TestPluginShutdown:
    def test_shutdown_no_error(self):
        Plugin.shutdown()  # Should not raise


class TestPluginOnEvent:
    def test_on_event_default_returns_none(self):
        ctx = mock_action_context().world
        result = Plugin.on_event("tick_advanced", ctx)
        assert result is None
