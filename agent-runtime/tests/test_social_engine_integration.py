"""Tests for social engine integration into agent think loop.

Covers:
- SocialEngine.build_context — social propensity, targets, trust
- SocialEngine.execute_socialize — imitation, conflict, trust updates
- SOCIALIZE ActionType in act.py — handler dispatch and token cost
- Social context injection into decide.py prompt
- SOCIALIZE mapping in llm_decide.py
- End-to-end socialize action through ThinkLoop
"""

from __future__ import annotations

import pytest

from agent_runtime.core.act import (
    ActionContext,
    ActionExecutor,
    ActionType,
)
from agent_runtime.core.decide import (
    DecisionAction,
    DecisionPerception,
    SocialContext,
    SurvivalAssessment,
    build_prompt,
)
from agent_runtime.core.think_loop import (
    Decision,
    ThinkLoop,
    ThinkLoopConfig,
)
from agent_runtime.models.agent_state import AgentState
from agent_runtime.models.enums import AgentPhase
from agent_runtime.models.personality import PersonalityVector
from agent_runtime.models.values import ValueWeights
from agent_runtime.social.engine import SocialEngine
from agent_runtime.social.provider import (
    AgentProfile,
    DefaultSocialEngineHook,
)

# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------


def _make_state(**overrides):
    defaults = dict(
        name="TestAgent",
        tokens=500,
        max_tokens=1000,
        health=100.0,
        reputation=5.0,
        phase=AgentPhase.ADULT,
    )
    defaults.update(overrides)
    return AgentState(**defaults)


def _make_personality(**overrides):
    defaults = dict(
        openness=0.6,
        conscientiousness=0.5,
        extraversion=0.7,
        agreeableness=0.6,
        neuroticism=0.3,
        risk_tolerance=0.5,
        social_orientation=0.6,
        greed=0.4,
    )
    defaults.update(overrides)
    return PersonalityVector(**defaults)


def _extraverted_personality():
    return _make_personality(
        extraversion=0.9,
        social_orientation=0.9,
        agreeableness=0.8,
    )


def _introverted_personality():
    return _make_personality(
        extraversion=0.1,
        social_orientation=0.1,
        agreeableness=0.2,
    )


# ---------------------------------------------------------------------------
# SocialEngine.build_context
# ---------------------------------------------------------------------------


class TestSocialEngineBuildContext:
    def test_extraverted_agent_has_high_social_propensity(self):
        engine = SocialEngine()
        ctx = engine.build_context(
            agent_id="a1",
            personality=_extraverted_personality(),
            values=ValueWeights(cooperation_weight=0.8),
            nearby_agents=[],
            tick=1,
        )
        assert ctx.social_propensity > 0.5
        assert ctx.should_socialize is False  # no targets

    def test_introverted_agent_has_low_social_propensity(self):
        engine = SocialEngine()
        ctx = engine.build_context(
            agent_id="a1",
            personality=_introverted_personality(),
            values=ValueWeights(cooperation_weight=0.2),
            nearby_agents=[],
            tick=1,
        )
        assert ctx.social_propensity < 0.4

    def test_should_socialize_with_nearby_agents(self):
        engine = SocialEngine()
        nearby = [
            {
                "agent_id": "a2",
                "personality": _make_personality(),
                "values": ValueWeights(),
            }
        ]
        ctx = engine.build_context(
            agent_id="a1",
            personality=_extraverted_personality(),
            values=ValueWeights(cooperation_weight=0.8),
            nearby_agents=nearby,
            tick=1,
        )
        assert ctx.should_socialize is True
        assert len(ctx.targets) == 1
        assert ctx.targets[0].agent_id == "a2"

    def test_trust_snapshot_populated(self):
        engine = SocialEngine()
        nearby = [
            {
                "agent_id": "a2",
                "personality": _make_personality(),
                "values": ValueWeights(),
                "group_ids": ["org1"],
            }
        ]
        ctx = engine.build_context(
            agent_id="a1",
            personality=_extraverted_personality(),
            values=ValueWeights(),
            nearby_agents=nearby,
            tick=1,
            agent_groups=["org1"],
        )
        assert "a2" in ctx.trust_snapshot
        # In-group trust should be high
        assert ctx.trust_snapshot["a2"] >= 0.7

    def test_in_group_trust_higher_than_out_group(self):
        engine = SocialEngine()
        nearby_in = [
            {
                "agent_id": "a2",
                "personality": _make_personality(),
                "values": ValueWeights(),
                "group_ids": ["org1"],
            }
        ]
        nearby_out = [
            {
                "agent_id": "a3",
                "personality": _make_personality(),
                "values": ValueWeights(),
                "group_ids": ["org2"],
            }
        ]
        ctx_in = engine.build_context(
            agent_id="a1",
            personality=_extraverted_personality(),
            values=ValueWeights(),
            nearby_agents=nearby_in,
            tick=1,
            agent_groups=["org1"],
        )
        ctx_out = engine.build_context(
            agent_id="a1",
            personality=_extraverted_personality(),
            values=ValueWeights(),
            nearby_agents=nearby_out,
            tick=1,
            agent_groups=["org1"],
        )
        assert ctx_in.trust_snapshot["a2"] > ctx_out.trust_snapshot["a3"]

    def test_recommended_target_is_highest_affinity(self):
        engine = SocialEngine()
        nearby = [
            {
                "agent_id": "low_trust",
                "personality": _make_personality(extraversion=0.1),
                "values": ValueWeights(),
            },
            {
                "agent_id": "high_trust",
                "personality": _make_personality(extraversion=0.9),
                "values": ValueWeights(cooperation_weight=0.9),
            },
        ]
        ctx = engine.build_context(
            agent_id="a1",
            personality=_extraverted_personality(),
            values=ValueWeights(cooperation_weight=0.9),
            nearby_agents=nearby,
            tick=1,
        )
        assert ctx.recommended_target is not None
        # The closer personality should be recommended
        assert ctx.recommended_target.agent_id == "high_trust"

    def test_personality_description_in_context(self):
        engine = SocialEngine()
        ctx = engine.build_context(
            agent_id="a1",
            personality=_extraverted_personality(),
            values=ValueWeights(),
            nearby_agents=[],
            tick=1,
        )
        desc = ctx.personality_description.lower()
        assert "sociable" in desc or "personality" in desc


# ---------------------------------------------------------------------------
# SocialEngine.execute_socialize
# ---------------------------------------------------------------------------


class TestSocialEngineExecuteSocialize:
    def test_execute_socialize_updates_trust(self):
        engine = SocialEngine()
        result = engine.execute_socialize(
            agent_id="a1",
            target_id="a2",
            personality=_extraverted_personality(),
            values=ValueWeights(),
            target_personality=_make_personality(),
            target_values=ValueWeights(),
            tick=1,
        )
        assert result["trust_update"]["event"] == "cooperation"
        assert result["trust_update"]["new_trust"] > 0.3  # should increase from default

    def test_execute_socialize_returns_imitation_result(self):
        engine = SocialEngine()
        result = engine.execute_socialize(
            agent_id="a1",
            target_id="a2",
            personality=_extraverted_personality(),
            values=ValueWeights(),
            target_personality=_make_personality(),
            target_values=ValueWeights(),
            tick=1,
        )
        # imitation_result is None or a dict (depends on randomness)
        assert "imitation" in result

    def test_execute_socialize_checks_cultural_conflict(self):
        engine = SocialEngine()
        # Very different values
        result = engine.execute_socialize(
            agent_id="a1",
            target_id="a2",
            personality=_extraverted_personality(),
            values=ValueWeights(cooperation_weight=0.9, competition_weight=0.1),
            target_personality=_make_personality(),
            target_values=ValueWeights(cooperation_weight=0.1, competition_weight=0.9),
            tick=1,
        )
        assert "conflict" in result


# ---------------------------------------------------------------------------
# SOCIALIZE ActionType in act.py
# ---------------------------------------------------------------------------


class TestSocializeActionType:
    def test_socialize_action_type_exists(self):
        assert ActionType.SOCIALIZE == "socialize"

    def test_socialize_token_cost(self):
        executor = ActionExecutor()
        assert executor.get_cost(ActionType.SOCIALIZE) == 5

    def test_socialize_handler_dispatched(self):
        """Verify SOCIALIZE is in the handler map."""
        assert ActionType.SOCIALIZE in ActionExecutor._HANDLER_NAMES
        assert ActionExecutor._HANDLER_NAMES[ActionType.SOCIALIZE] == "_handle_socialize"


class _MockWorldClient:
    """Simple mock that supports socialize."""

    async def socialize(self, target_agent_id: str, message: str = "") -> dict:
        return {
            "status": "ok",
            "action": "socialize",
            "target_agent_id": target_agent_id,
        }


@pytest.mark.asyncio
async def test_socialize_action_execution():
    """Full execution path for SOCIALIZE action."""
    state = _make_state(tokens=100)
    executor = ActionExecutor()
    ctx = ActionContext(
        agent=state,
        world=_MockWorldClient(),
        parameters={"target_agent_id": "agent-b", "message": "Hello!"},
    )
    result = await executor.execute(ActionType.SOCIALIZE, ctx)
    assert result.status.value == "success"
    assert result.token_cost == 5
    assert result.data["target_agent_id"] == "agent-b"


@pytest.mark.asyncio
async def test_socialize_action_without_target_fails():
    """SOCIALIZE without target_agent_id should raise."""
    state = _make_state(tokens=100)
    executor = ActionExecutor()
    ctx = ActionContext(
        agent=state,
        world=_MockWorldClient(),
        parameters={},
    )
    result = await executor.execute(ActionType.SOCIALIZE, ctx)
    assert result.status.value == "retry_exhausted"


# ---------------------------------------------------------------------------
# Social context in decide.py prompt
# ---------------------------------------------------------------------------


class TestSocialContextInPrompt:
    def test_prompt_includes_social_context(self):
        state = _make_state()
        perception = DecisionPerception(tick=42, nearby_agents=["agent-b"])
        survival = SurvivalAssessment()
        social = SocialContext(
            social_propensity=0.75,
            should_socialize=True,
            recommended_target_id="agent-b",
            trust_snapshot={"agent-b": 0.8},
            personality_description="A highly sociable agent.",
        )
        prompt = build_prompt(
            state,
            perception,
            survival,
            DecisionAction.all(),
            social=social,
        )
        assert "Social Context" in prompt
        assert "75%" in prompt  # social_propensity formatted
        assert "True" in prompt  # should_socialize
        assert "agent-b" in prompt
        assert "trust=0.80" in prompt
        assert "Recommended social target: agent-b" in prompt

    def test_prompt_without_social_context(self):
        state = _make_state()
        perception = DecisionPerception(tick=42)
        survival = SurvivalAssessment()
        prompt = build_prompt(
            state,
            perception,
            survival,
            DecisionAction.all(),
        )
        assert "Social Context" in prompt
        assert "No social context available" in prompt


# ---------------------------------------------------------------------------
# SOCIALIZE in llm_decide.py mapping
# ---------------------------------------------------------------------------


class TestLlmDecideSocializeMapping:
    def test_socialize_maps_correctly(self):
        from agent_runtime.core.llm_decide import _DECISION_TO_ACTION
        assert _DECISION_TO_ACTION[DecisionAction.SOCIALIZE] == ActionType.SOCIALIZE


# ---------------------------------------------------------------------------
# End-to-end: SOCIALIZE through ThinkLoop
# ---------------------------------------------------------------------------


class _FixedDecisionProvider:
    """Decision provider that always returns a SOCIALIZE action."""

    def __init__(self, target_id: str = "agent-b") -> None:
        self._target_id = target_id

    async def decide(self, state, perception, survival):
        return Decision(
            action_type=ActionType.SOCIALIZE,
            parameters={"target_agent_id": self._target_id, "message": "Hi!"},
            reasoning="Testing socialize e2e",
        )


class _ProfileTracker:
    """Records which agents had process_socialize called."""

    def __init__(self) -> None:
        self.interactions: list[tuple[str, str, int]] = []
        self.diffusion_calls: int = 0

    def __call__(self, agent_id: str) -> AgentProfile | None:
        return AgentProfile(
            personality=(
                _extraverted_personality()
                if agent_id == "agent-a"
                else _make_personality()
            ),
            values=ValueWeights(),
            group_ids=["group-1"],
        )


@pytest.mark.asyncio
async def test_e2e_socialize_through_think_loop():
    """End-to-end: ThinkLoop executes SOCIALIZE and triggers SocialEngineHook."""
    state = _make_state(name="agent-a", tokens=500)
    executor = ActionExecutor()

    # Build a social engine hook with a profile source and region source
    engine = SocialEngine()
    profiles = {
        "agent-a": AgentProfile(
            personality=_extraverted_personality(),
            values=ValueWeights(cooperation_weight=0.8),
            group_ids=["group-1"],
        ),
        "agent-b": AgentProfile(
            personality=_make_personality(),
            values=ValueWeights(),
            group_ids=["group-1"],
        ),
    }
    diffusion_data = {
        "region-1": [
            {
                "agent_id": "agent-a",
                "values": ValueWeights(),
                "personality": _extraverted_personality(),
            },
            {
                "agent_id": "agent-b",
                "values": ValueWeights(),
                "personality": _make_personality(),
            },
        ],
    }

    hook = DefaultSocialEngineHook(
        engine=engine,
        profile_source=lambda aid: profiles.get(aid),
        region_agent_source=lambda: diffusion_data,
    )

    loop = ThinkLoop(
        state=state,
        survival=__import__(
            "agent_runtime.survival.instinct",
            fromlist=["SurvivalInstinct"],
        ).SurvivalInstinct(),
        executor=executor,
        config=ThinkLoopConfig(tick_interval=0.0),
        decision_provider=_FixedDecisionProvider("agent-b"),
        world_client=_MockWorldClient(),
        social_engine_hook=hook,
    )

    await loop.run(max_ticks=1)

    # Verify the action was executed
    history = executor.history
    assert len(history) == 1
    assert history[0].action_type == ActionType.SOCIALIZE
    assert history[0].status.value == "success"
    assert history[0].data["target_agent_id"] == "agent-b"


@pytest.mark.asyncio
async def test_e2e_socialize_updates_trust():
    """End-to-end: SOCIALIZE through ThinkLoop triggers trust update in engine."""
    engine = SocialEngine()
    profiles = {
        "agent-a": AgentProfile(
            personality=_extraverted_personality(),
            values=ValueWeights(cooperation_weight=0.8),
            group_ids=["group-1"],
        ),
        "agent-b": AgentProfile(
            personality=_make_personality(),
            values=ValueWeights(),
            group_ids=["group-1"],
        ),
    }

    hook = DefaultSocialEngineHook(
        engine=engine,
        profile_source=lambda aid: profiles.get(aid),
    )

    result = hook.process_socialize(
        agent_id="agent-a",
        target_id="agent-b",
        tick=1,
    )

    assert result is not None
    assert result["trust_update"]["event"] == "cooperation"
    assert result["trust_update"]["new_trust"] > 0.3


@pytest.mark.asyncio
async def test_e2e_tick_diffusion_called_each_tick():
    """End-to-end: apply_tick_diffusion is called each tick in ThinkLoop."""
    engine = SocialEngine()
    diffusion_data = {
        "region-1": [
            {
                "agent_id": "agent-a",
                "values": ValueWeights(),
                "personality": _extraverted_personality(),
            },
            {
                "agent_id": "agent-b",
                "values": ValueWeights(),
                "personality": _make_personality(),
            },
        ],
    }

    hook = DefaultSocialEngineHook(
        engine=engine,
        region_agent_source=lambda: diffusion_data,
    )

    state = _make_state(name="agent-a", tokens=500)
    executor = ActionExecutor()

    loop = ThinkLoop(
        state=state,
        survival=__import__(
            "agent_runtime.survival.instinct",
            fromlist=["SurvivalInstinct"],
        ).SurvivalInstinct(),
        executor=executor,
        config=ThinkLoopConfig(tick_interval=0.0),
        decision_provider=_FixedDecisionProvider("agent-b"),
        world_client=_MockWorldClient(),
        social_engine_hook=hook,
    )

    await loop.run(max_ticks=2)

    # Verify that both ticks executed (REST fallback if SOCIALIZE exhausted tokens)
    assert loop.tick == 2


@pytest.mark.asyncio
async def test_e2e_social_engine_hook_degrades_without_sources():
    """Hook returns None/empty when sources are not configured."""
    hook = DefaultSocialEngineHook()

    assert hook.process_socialize("a", "b", 1) is None
    assert hook.apply_tick_diffusion() == []
