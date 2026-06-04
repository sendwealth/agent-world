"""Unit tests for agent_world_plugin_sdk types."""
from __future__ import annotations

import json
import pytest

from agent_world_plugin_sdk import (
    ActionContext,
    ActionResult,
    AgentSnapshot,
    MutationKind,
    PluginError,
    PluginInfo,
    SkillPlugin,
    StateMutation,
    TokenCost,
    WorldContext,
)


# ─── PluginInfo ──────────────────────────────────────────────────────────


class TestPluginInfo:
    def test_creation(self):
        info = PluginInfo(
            id="author/test-plugin",
            name="Test Plugin",
            version="0.1.0",
            description="A test plugin",
            author="author",
            min_engine_version="1.0.0",
        )
        assert info.id == "author/test-plugin"
        assert info.name == "Test Plugin"
        assert info.version == "0.1.0"
        assert info.required_skills == []
        assert info.config_schema is None
        assert info.tags == []

    def test_with_optional_fields(self):
        info = PluginInfo(
            id="a/b",
            name="N",
            version="1.0.0",
            description="D",
            author="A",
            min_engine_version="1.0.0",
            required_skills=["skill_a"],
            config_schema='{"type":"object"}',
            tags=["test", "example"],
        )
        assert info.required_skills == ["skill_a"]
        assert info.config_schema == '{"type":"object"}'
        assert info.tags == ["test", "example"]

    def test_to_dict(self):
        info = PluginInfo(
            id="a/b",
            name="N",
            version="1.0.0",
            description="D",
            author="A",
            min_engine_version="1.0.0",
        )
        d = info.to_dict()
        assert d["id"] == "a/b"
        assert d["name"] == "N"
        assert d["required_skills"] == []
        assert d["config_schema"] is None

    def test_to_json(self):
        info = PluginInfo(
            id="a/b",
            name="N",
            version="1.0.0",
            description="D",
            author="A",
            min_engine_version="1.0.0",
        )
        j = info.to_json()
        data = json.loads(j)
        assert data["id"] == "a/b"

    def test_from_dict(self):
        d = {
            "id": "a/b",
            "name": "N",
            "version": "1.0.0",
            "description": "D",
            "author": "A",
            "min_engine_version": "1.0.0",
            "required_skills": ["s1"],
        }
        info = PluginInfo.from_dict(d)
        assert info.id == "a/b"
        assert info.required_skills == ["s1"]

    def test_from_json(self):
        j = json.dumps({
            "id": "a/b",
            "name": "N",
            "version": "1.0.0",
            "description": "D",
            "author": "A",
            "min_engine_version": "1.0.0",
        })
        info = PluginInfo.from_json(j)
        assert info.id == "a/b"

    def test_roundtrip(self):
        info = PluginInfo(
            id="a/b",
            name="N",
            version="2.3.4",
            description="D",
            author="A",
            min_engine_version="1.0.0",
            required_skills=["s1", "s2"],
            tags=["t1"],
        )
        restored = PluginInfo.from_json(info.to_json())
        assert restored == info


# ─── AgentSnapshot ───────────────────────────────────────────────────────


class TestAgentSnapshot:
    def test_creation(self):
        snap = AgentSnapshot(
            id="agent-1",
            name="Alice",
            phase="working",
            money=100,
            tokens=50,
            reputation=0.8,
        )
        assert snap.id == "agent-1"
        assert snap.skills == {}
        assert snap.alive is True
        assert snap.age == 0

    def test_to_dict_and_back(self):
        snap = AgentSnapshot(
            id="agent-2",
            name="Bob",
            phase="idle",
            money=200,
            tokens=10,
            reputation=0.5,
            skills={"coding": 5},
            alive=True,
            age=3,
        )
        d = snap.to_dict()
        assert d["skills"] == {"coding": 5}
        restored = AgentSnapshot.from_dict(d)
        assert restored == snap

    def test_json_roundtrip(self):
        snap = AgentSnapshot(
            id="a", name="B", phase="p", money=1, tokens=2, reputation=3.0
        )
        assert AgentSnapshot.from_json(snap.to_json()) == snap


# ─── WorldContext ────────────────────────────────────────────────────────


class TestWorldContext:
    def test_minimal(self):
        ctx = WorldContext(tick=1)
        assert ctx.tick == 1
        assert ctx.agent is None
        assert ctx.visible_agents == []
        assert ctx.globals == {}
        assert ctx.recent_events == []

    def test_with_agent(self):
        agent = AgentSnapshot(
            id="a1", name="A", phase="p", money=10, tokens=5, reputation=1.0
        )
        ctx = WorldContext(tick=42, agent=agent)
        d = ctx.to_dict()
        assert d["agent"]["id"] == "a1"
        restored = WorldContext.from_dict(d)
        assert restored.agent is not None
        assert restored.agent.id == "a1"

    def test_json_roundtrip(self):
        agent = AgentSnapshot(
            id="a1", name="A", phase="p", money=10, tokens=5, reputation=1.0
        )
        ctx = WorldContext(
            tick=100,
            agent=agent,
            visible_agents=[agent],
            globals={"key": "val"},
            recent_events=["e1"],
        )
        restored = WorldContext.from_json(ctx.to_json())
        assert restored.tick == 100
        assert restored.agent.id == "a1"
        assert len(restored.visible_agents) == 1
        assert restored.globals == {"key": "val"}


# ─── ActionContext ───────────────────────────────────────────────────────


class TestActionContext:
    def test_creation(self):
        world = WorldContext(tick=1)
        ctx = ActionContext(world=world)
        assert ctx.params == {}
        assert ctx.config == {}

    def test_json_roundtrip(self):
        world = WorldContext(
            tick=5,
            globals={"k": "v"},
        )
        ctx = ActionContext(
            world=world,
            params={"action": "run"},
            config={"debug": "true"},
        )
        restored = ActionContext.from_json(ctx.to_json())
        assert restored.world.tick == 5
        assert restored.params == {"action": "run"}
        assert restored.config == {"debug": "true"}


# ─── MutationKind ────────────────────────────────────────────────────────


class TestMutationKind:
    def test_values(self):
        assert MutationKind.CREDIT_TOKENS.value == "credit_tokens"
        assert MutationKind.DEBIT_TOKENS.value == "debit_tokens"
        assert MutationKind.CREDIT_MONEY.value == "credit_money"
        assert MutationKind.DEBIT_MONEY.value == "debit_money"
        assert MutationKind.SET_SKILL.value == "set_skill"
        assert MutationKind.ADJUST_REPUTATION.value == "adjust_reputation"
        assert MutationKind.SET_GLOBAL.value == "set_global"
        assert MutationKind.EMIT_EVENT.value == "emit_event"

    def test_from_value(self):
        assert MutationKind("credit_tokens") == MutationKind.CREDIT_TOKENS

    def test_all_variants(self):
        assert len(MutationKind) == 8


# ─── StateMutation ───────────────────────────────────────────────────────


class TestStateMutation:
    def test_creation(self):
        m = StateMutation(kind=MutationKind.CREDIT_TOKENS, value="100")
        assert m.kind == MutationKind.CREDIT_TOKENS
        assert m.target_agent is None
        assert m.field == ""
        assert m.value == "100"

    def test_to_dict(self):
        m = StateMutation(
            kind=MutationKind.SET_SKILL,
            target_agent="agent-1",
            field="coding",
            value="5",
        )
        d = m.to_dict()
        assert d["kind"] == "set_skill"
        assert d["target_agent"] == "agent-1"

    def test_from_dict(self):
        d = {
            "kind": "credit_money",
            "target_agent": "a1",
            "field": "money",
            "value": "500",
        }
        m = StateMutation.from_dict(d)
        assert m.kind == MutationKind.CREDIT_MONEY
        assert m.value == "500"

    def test_json_roundtrip(self):
        m = StateMutation(
            kind=MutationKind.ADJUST_REPUTATION, target_agent="a1", value="0.1"
        )
        restored = StateMutation.from_json(m.to_json())
        assert restored.kind == MutationKind.ADJUST_REPUTATION


# ─── ActionResult ────────────────────────────────────────────────────────


class TestActionResult:
    def test_success_result(self):
        r = ActionResult(success=True, message="OK")
        assert r.success is True
        assert r.mutations == []
        assert r.events == []
        assert r.data == {}
        assert r.tokens_consumed == 0

    def test_with_mutations(self):
        r = ActionResult(
            success=True,
            message="Done",
            mutations=[
                StateMutation(kind=MutationKind.CREDIT_TOKENS, value="10")
            ],
            events=["event1"],
            data={"key": "val"},
            tokens_consumed=5,
        )
        d = r.to_dict()
        assert len(d["mutations"]) == 1
        assert d["mutations"][0]["kind"] == "credit_tokens"
        assert d["tokens_consumed"] == 5

    def test_json_roundtrip(self):
        r = ActionResult(
            success=False,
            message="Failed",
            mutations=[
                StateMutation(
                    kind=MutationKind.SET_GLOBAL,
                    field="counter",
                    value="42",
                )
            ],
            events=["error"],
            data={"err": "bad"},
            tokens_consumed=1,
        )
        restored = ActionResult.from_json(r.to_json())
        assert restored.success is False
        assert len(restored.mutations) == 1
        assert restored.mutations[0].kind == MutationKind.SET_GLOBAL


# ─── TokenCost ───────────────────────────────────────────────────────────


class TestTokenCost:
    def test_defaults(self):
        cost = TokenCost(estimated=10)
        assert cost.confidence == 1.0
        assert cost.breakdown is None

    def test_full(self):
        cost = TokenCost(estimated=100, confidence=0.8, breakdown="base:100")
        d = cost.to_dict()
        assert d["estimated"] == 100
        assert d["confidence"] == 0.8

    def test_roundtrip(self):
        cost = TokenCost(estimated=42, confidence=0.95, breakdown="test")
        assert TokenCost.from_json(cost.to_json()) == cost


# ─── PluginError ─────────────────────────────────────────────────────────


class TestPluginError:
    def test_default_code(self):
        err = PluginError(message="something broke")
        assert err.code == "custom"
        assert err.message == "something broke"
        assert "[custom]" in str(err)

    def test_specific_code(self):
        err = PluginError(code="init_failed", message="bad config")
        assert err.code == "init_failed"
        assert "[init_failed]" in str(err)

    def test_is_exception(self):
        with pytest.raises(PluginError):
            raise PluginError(code="execution_failed", message="timeout")

    def test_to_dict(self):
        err = PluginError(code="config_error", message="missing key")
        d = err.to_dict()
        assert d == {"code": "config_error", "message": "missing key"}

    def test_from_dict(self):
        err = PluginError.from_dict({"code": "missing_skill", "message": "nope"})
        assert err.code == "missing_skill"

    def test_json_roundtrip(self):
        err = PluginError(code="custom", message="test error")
        restored = PluginError.from_json(err.to_json())
        assert restored.code == err.code
        assert restored.message == err.message

    def test_well_known_codes(self):
        assert PluginError.INIT_FAILED == "init_failed"
        assert PluginError.EXECUTION_FAILED == "execution_failed"
        assert PluginError.CONFIG_ERROR == "config_error"
        assert PluginError.MISSING_SKILL == "missing_skill"
        assert PluginError.COST_ESTIMATE_FAILED == "cost_estimate_failed"
        assert PluginError.INVALID_STATE == "invalid_state"


# ─── SkillPlugin ABC ─────────────────────────────────────────────────────


class TestSkillPlugin:
    def test_cannot_instantiate(self):
        """SkillPlugin is abstract and cannot be instantiated directly."""
        with pytest.raises(TypeError):
            SkillPlugin()

    def test_concrete_subclass(self):
        """A concrete subclass can be created."""

        class MyPlugin(SkillPlugin):
            @classmethod
            def init(cls, config):
                return PluginInfo(
                    id="test/my",
                    name="My",
                    version="0.1.0",
                    description="test",
                    author="test",
                    min_engine_version="1.0.0",
                )

            @classmethod
            def register(cls):
                return ["my_skill"]

            @classmethod
            def execute(cls, ctx):
                return ActionResult(success=True, message="ok")

            @classmethod
            def cost_estimate(cls, ctx):
                return TokenCost(estimated=1)

        # Should not raise
        info = MyPlugin.init({})
        assert info.id == "test/my"
        assert MyPlugin.register() == ["my_skill"]

        ctx = ActionContext(world=WorldContext(tick=1))
        result = MyPlugin.execute(ctx)
        assert result.success is True

        cost = MyPlugin.cost_estimate(ctx)
        assert cost.estimated == 1

    def test_default_shutdown(self):
        """shutdown() has a default no-op implementation."""

        class MyPlugin(SkillPlugin):
            @classmethod
            def init(cls, config):
                return PluginInfo(
                    id="t/p", name="N", version="0.1.0",
                    description="d", author="a", min_engine_version="1.0.0",
                )

            @classmethod
            def register(cls):
                return []

            @classmethod
            def execute(cls, ctx):
                return ActionResult(success=True, message="")

            @classmethod
            def cost_estimate(cls, ctx):
                return TokenCost(estimated=0)

        # Should not raise
        MyPlugin.shutdown()

    def test_default_on_event(self):
        """on_event() returns None by default."""

        class MyPlugin(SkillPlugin):
            @classmethod
            def init(cls, config):
                return PluginInfo(
                    id="t/p", name="N", version="0.1.0",
                    description="d", author="a", min_engine_version="1.0.0",
                )

            @classmethod
            def register(cls):
                return []

            @classmethod
            def execute(cls, ctx):
                return ActionResult(success=True, message="")

            @classmethod
            def cost_estimate(cls, ctx):
                return TokenCost(estimated=0)

        result = MyPlugin.on_event("test_event", WorldContext(tick=1))
        assert result is None
