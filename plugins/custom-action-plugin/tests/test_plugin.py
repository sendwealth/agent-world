"""Tests for Custom Emote Plugin."""

import json
import sys
import os

# Add src to path for testing without installation
sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "src"))

from custom_action_plugin import (
    CustomEmotePlugin as Plugin,
    ActionContext,
    WorldContext,
    AgentSnapshot,
    PluginError,
)


def mock_action_context(
    emote_text: str = "waves hello cheerfully",
    config: dict = None,
) -> ActionContext:
    """Create a mock ActionContext for testing."""
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
            visible_agents=[
                AgentSnapshot(
                    id="agent-002",
                    name="Bob",
                    phase="adult",
                    money=800,
                    tokens=300,
                    reputation=40.0,
                    skills={},
                    alive=True,
                    age=8,
                ),
            ],
            globals={},
            recent_events=[],
        ),
        params={"emote_text": emote_text},
        config=config or {"emote_prefix": "*"},
    )


# ─── Init Tests ──────────────────────────────────────────────────────────


class TestPluginInit:
    def test_init_returns_plugin_info(self):
        config = {"emote_prefix": "~"}
        info = Plugin.init(config)
        assert info.id == "community/custom-emote"
        assert info.name == "Custom Emote Plugin"
        assert info.version == "1.0.0"
        assert info.author == "Agent World Community"

    def test_init_has_correct_tags(self):
        info = Plugin.init({})
        assert "action" in info.tags
        assert "social" in info.tags

    def test_init_has_config_schema(self):
        info = Plugin.init({})
        assert info.config_schema is not None
        schema = json.loads(info.config_schema)
        assert "properties" in schema
        assert "emote_prefix" in schema["properties"]
        assert "max_emote_length" in schema["properties"]

    def test_init_with_default_config(self):
        info = Plugin.init({})
        assert info.min_engine_version == "1.0.0"
        assert info.required_skills == []

    def test_init_invalid_max_length_raises(self):
        try:
            Plugin.init({"max_emote_length": "-5"})
            assert False, "Should have raised PluginError"
        except PluginError as e:
            assert e.code == "config_error"

    def test_init_non_numeric_max_length_raises(self):
        try:
            Plugin.init({"max_emote_length": "abc"})
            assert False, "Should have raised PluginError"
        except PluginError as e:
            assert e.code == "config_error"


# ─── Register Tests ──────────────────────────────────────────────────────


class TestPluginRegister:
    def test_register_returns_custom_emote(self):
        skills = Plugin.register()
        assert skills == ["custom_emote"]

    def test_register_returns_list(self):
        skills = Plugin.register()
        assert isinstance(skills, list)
        assert len(skills) == 1


# ─── Execute Tests ───────────────────────────────────────────────────────


class TestPluginExecute:
    def test_execute_success_with_emote(self):
        ctx = mock_action_context(emote_text="waves hello cheerfully")
        result = Plugin.execute(ctx)
        assert result.success is True
        assert "* Alice waves hello cheerfully" == result.message
        assert result.tokens_consumed == 1

    def test_execute_emits_event(self):
        ctx = mock_action_context(emote_text="dances")
        result = Plugin.execute(ctx)
        assert len(result.events) == 1
        event_data = json.loads(result.events[0])
        assert event_data["type"] == "agent_emote"
        assert event_data["agent_name"] == "Alice"
        assert event_data["emote_text"] == "dances"

    def test_execute_returns_data(self):
        ctx = mock_action_context(emote_text="laughs")
        result = Plugin.execute(ctx)
        assert result.data["emote_text"] == "laughs"
        assert result.data["formatted_emote"] == "* Alice laughs"
        assert result.data["agent_name"] == "Alice"
        assert result.data["prefix"] == "*"

    def test_execute_custom_prefix(self):
        ctx = mock_action_context(
            emote_text="nods thoughtfully",
            config={"emote_prefix": "/me"},
        )
        result = Plugin.execute(ctx)
        assert result.message == "/me Alice nods thoughtfully"

    def test_execute_no_emote_text_fails(self):
        ctx = mock_action_context(emote_text="")
        result = Plugin.execute(ctx)
        assert result.success is False
        assert "No emote text" in result.message

    def test_execute_whitespace_only_emote_fails(self):
        ctx = mock_action_context(emote_text="   ")
        result = Plugin.execute(ctx)
        assert result.success is False

    def test_execute_truncates_long_emote(self):
        long_text = "a" * 250
        ctx = mock_action_context(
            emote_text=long_text,
            config={"emote_prefix": "*", "max_emote_length": "200"},
        )
        result = Plugin.execute(ctx)
        assert result.success is True
        assert len(result.data["emote_text"]) <= 200

    def test_execute_without_agent(self):
        ctx = mock_action_context()
        ctx.world.agent = None
        result = Plugin.execute(ctx)
        assert result.success is True
        assert "Unknown Agent" in result.message

    def test_execute_no_mutations(self):
        ctx = mock_action_context()
        result = Plugin.execute(ctx)
        assert result.mutations == []


# ─── Cost Estimate Tests ─────────────────────────────────────────────────


class TestPluginCostEstimate:
    def test_cost_estimate_is_one(self):
        ctx = mock_action_context()
        cost = Plugin.cost_estimate(ctx)
        assert cost.estimated == 1
        assert cost.confidence == 1.0

    def test_cost_estimate_has_breakdown(self):
        ctx = mock_action_context()
        cost = Plugin.cost_estimate(ctx)
        assert cost.breakdown is not None
        assert "1 token" in cost.breakdown

    def test_cost_estimate_consistent_regardless_of_context(self):
        ctx1 = mock_action_context(emote_text="short")
        ctx2 = mock_action_context(emote_text="a" * 200)
        cost1 = Plugin.cost_estimate(ctx1)
        cost2 = Plugin.cost_estimate(ctx2)
        assert cost1.estimated == cost2.estimated


# ─── Shutdown Tests ──────────────────────────────────────────────────────


class TestPluginShutdown:
    def test_shutdown_no_error(self):
        Plugin.shutdown()  # Should not raise


# ─── On Event Tests ──────────────────────────────────────────────────────


class TestPluginOnEvent:
    def test_on_event_returns_none_for_unknown(self):
        ctx = mock_action_context().world
        result = Plugin.on_event('{"type":"unknown"}', ctx)
        assert result is None

    def test_on_event_returns_none_for_interact(self):
        ctx = mock_action_context().world
        result = Plugin.on_event('{"type":"agent_interact"}', ctx)
        assert result is None

    def test_on_event_handles_invalid_json(self):
        ctx = mock_action_context().world
        result = Plugin.on_event("not valid json{{{", ctx)
        assert result is None


# ─── WASM Entry Point Tests ──────────────────────────────────────────────


class TestWASMEntryPoints:
    def test_wasm_init(self):
        from custom_action_plugin import init as wasm_init
        result_json = wasm_init('{"emote_prefix": "~"}')
        result = json.loads(result_json)
        assert result["id"] == "community/custom-emote"
        assert result["tags"] == ["action", "social"]

    def test_wasm_register(self):
        from custom_action_plugin import register as wasm_register
        result_json = wasm_register()
        result = json.loads(result_json)
        assert result == ["custom_emote"]

    def test_wasm_execute(self):
        from custom_action_plugin import execute as wasm_execute
        ctx_json = json.dumps({
            "world": {
                "tick": 10,
                "agent": {
                    "id": "a1",
                    "name": "Eve",
                    "phase": "adult",
                    "money": 500,
                    "tokens": 100,
                    "reputation": 75.0,
                    "skills": {},
                    "alive": True,
                    "age": 5,
                },
                "visible_agents": [],
                "globals": {},
                "recent_events": [],
            },
            "params": {"emote_text": "smiles"},
            "config": {"emote_prefix": "*"},
        })
        result_json = wasm_execute(ctx_json)
        result = json.loads(result_json)
        assert result["success"] is True
        assert result["message"] == "* Eve smiles"
        assert result["tokens_consumed"] == 1

    def test_wasm_cost_estimate(self):
        from custom_action_plugin import cost_estimate as wasm_cost_estimate
        ctx_json = json.dumps({
            "world": {
                "tick": 1,
                "agent": None,
                "visible_agents": [],
                "globals": {},
                "recent_events": [],
            },
            "params": {},
            "config": {},
        })
        result_json = wasm_cost_estimate(ctx_json)
        result = json.loads(result_json)
        assert result["estimated"] == 1
        assert result["confidence"] == 1.0
