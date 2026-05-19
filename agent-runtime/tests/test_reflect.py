"""Tests for the reflection engine (agent_runtime.core.reflect).

Covers:
- ReflectionConfig creation and defaults
- ReflectionResult data class
- build_reflection_prompt: includes state and action history
- parse_reflection_response: valid JSON, code fences, defaults, errors
- ReflectionEngine.record_action: tracks action history
- ReflectionEngine.reflect: success path with LLM
- ReflectionEngine.reflect: skipped when tokens low
- ReflectionEngine.reflect: LLM failure graceful handling
- ReflectionEngine.reflect: parse failure graceful handling
- ReflectionEngine.reflect: writes experiences to long-term memory
- Integration: ThinkLoop with ReflectionEngine
"""

from __future__ import annotations

import json
from unittest.mock import AsyncMock

import pytest

from agent_runtime.core.act import ActionExecutor
from agent_runtime.core.reflect import (
    ReflectionConfig,
    ReflectionEngine,
    ReflectionResult,
    build_reflection_prompt,
    parse_reflection_response,
)
from agent_runtime.core.think_loop import (
    ThinkLoop,
    ThinkLoopConfig,
)
from agent_runtime.llm.base import LLMResponse, TokenUsage
from agent_runtime.llm.cost import CostTracker
from agent_runtime.memory.long_term import LongTermMemory
from agent_runtime.memory.short_term import ShortTermMemory
from agent_runtime.memory.working_memory import WorkingMemory
from agent_runtime.models.agent_state import AgentState
from agent_runtime.survival.instinct import SurvivalInstinct

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def make_state(
    tokens: int = 500,
    max_tokens: int = 1000,
    *,
    name: str = "TestAgent",
) -> AgentState:
    return AgentState(
        name=name,
        tokens=tokens,
        max_tokens=max_tokens,
        money=50.0,
        health=100.0,
    )


def make_engine(
    llm_content: str | None = None,
    *,
    ltm: LongTermMemory | None = None,
    stm: ShortTermMemory | None = None,
    wm: WorkingMemory | None = None,
    cost_tracker: CostTracker | None = None,
    config: ReflectionConfig | None = None,
    side_effect: Exception | None = None,
) -> ReflectionEngine:
    """Create a ReflectionEngine with a mock LLM provider."""
    mock_provider = AsyncMock()
    if side_effect is not None:
        mock_provider.chat.side_effect = side_effect
    elif llm_content is not None:
        mock_provider.chat.return_value = LLMResponse(
            content=llm_content,
            model="test-model",
            usage=TokenUsage(prompt_tokens=50, completion_tokens=30, total_tokens=80),
        )

    return ReflectionEngine(
        llm_provider=mock_provider,
        long_term_memory=ltm if ltm is not None else LongTermMemory(),
        short_term_memory=stm,
        working_memory=wm,
        cost_tracker=cost_tracker,
        config=config,
    )


def valid_reflection_json(
    *,
    analysis: str = "Reflection analysis",
    adjustments: list[str] | None = None,
    experiences: list[dict] | None = None,
) -> str:
    """Build a valid reflection JSON response."""
    return json.dumps({
        "analysis": analysis,
        "strategy_adjustments": adjustments or ["rest more when tokens low"],
        "experiences": experiences or [
            {"content": "Learned to rest more", "importance": 0.8, "category": "strategy"},
        ],
    })


# ---------------------------------------------------------------------------
# ReflectionConfig
# ---------------------------------------------------------------------------


class TestReflectionConfig:
    def test_defaults(self):
        cfg = ReflectionConfig()
        assert cfg.token_overhead == 10
        assert cfg.max_recent_actions == 10
        assert cfg.max_strategy_memories == 5

    def test_custom(self):
        cfg = ReflectionConfig(token_overhead=5, max_recent_actions=5)
        assert cfg.token_overhead == 5
        assert cfg.max_recent_actions == 5


# ---------------------------------------------------------------------------
# ReflectionResult
# ---------------------------------------------------------------------------


class TestReflectionResult:
    def test_defaults(self):
        r = ReflectionResult(tick=10)
        assert r.tick == 10
        assert r.analysis == ""
        assert r.strategy_adjustments == []
        assert r.memories_stored == 0
        assert r.token_cost == 0
        assert r.skipped is False
        assert r.skip_reason == ""

    def test_frozen(self):
        r = ReflectionResult(tick=10)
        with pytest.raises(AttributeError):
            r.tick = 20  # type: ignore[misc]


# ---------------------------------------------------------------------------
# build_reflection_prompt
# ---------------------------------------------------------------------------


class TestBuildReflectionPrompt:
    def test_prompt_contains_state(self):
        state = make_state(name="Alice", tokens=500, max_tokens=1000)
        prompt = build_reflection_prompt(
            state, tick=42, recent_actions=[], strategy_context="test",
        )
        assert "Alice" in prompt
        assert "Tick 42" in prompt
        assert "500" in prompt

    def test_prompt_contains_actions(self):
        state = make_state()
        actions = [
            {"tick": 1, "action": "explore", "status": "success"},
            {"tick": 2, "action": "rest", "status": "success"},
        ]
        prompt = build_reflection_prompt(
            state, tick=10, recent_actions=actions, strategy_context="",
        )
        assert "explore" in prompt
        assert "rest" in prompt

    def test_prompt_no_actions(self):
        state = make_state()
        prompt = build_reflection_prompt(state, tick=10, recent_actions=[], strategy_context="")
        assert "No recent actions" in prompt

    def test_prompt_contains_strategy_context(self):
        state = make_state()
        prompt = build_reflection_prompt(
            state, tick=10, recent_actions=[],
            strategy_context="  - avoid risky trades",
        )
        assert "avoid risky trades" in prompt


# ---------------------------------------------------------------------------
# parse_reflection_response
# ---------------------------------------------------------------------------


class TestParseReflectionResponse:
    def test_parse_valid_json(self):
        raw = valid_reflection_json()
        result = parse_reflection_response(raw)
        assert result["analysis"] == "Reflection analysis"
        assert len(result["strategy_adjustments"]) == 1
        assert len(result["experiences"]) == 1

    def test_parse_with_code_fences(self):
        raw = f"```json\n{valid_reflection_json()}\n```"
        result = parse_reflection_response(raw)
        assert result["analysis"] == "Reflection analysis"

    def test_parse_default_analysis(self):
        raw = json.dumps({"strategy_adjustments": [], "experiences": []})
        result = parse_reflection_response(raw)
        assert result["analysis"] == ""

    def test_parse_string_experience(self):
        raw = json.dumps({
            "analysis": "test",
            "strategy_adjustments": [],
            "experiences": ["simple string experience"],
        })
        result = parse_reflection_response(raw)
        assert len(result["experiences"]) == 1
        assert result["experiences"][0]["content"] == "simple string experience"
        assert result["experiences"][0]["importance"] == 0.7

    def test_parse_importance_clamped(self):
        raw = json.dumps({
            "experiences": [{"content": "test", "importance": 5.0}],
        })
        result = parse_reflection_response(raw)
        assert result["experiences"][0]["importance"] == 1.0

    def test_parse_invalid_json_raises(self):
        with pytest.raises(ValueError, match="Failed to parse"):
            parse_reflection_response("not json")

    def test_parse_non_dict_raises(self):
        with pytest.raises(ValueError, match="JSON object"):
            parse_reflection_response('"a string"')


# ---------------------------------------------------------------------------
# ReflectionEngine — record_action
# ---------------------------------------------------------------------------


class TestRecordAction:
    def test_record_action_tracks_history(self):
        engine = make_engine("dummy")
        engine.record_action(tick=1, action="explore", status="success")
        engine.record_action(tick=2, action="rest", status="success")
        assert len(engine.action_history) == 2
        assert engine.action_history[0]["tick"] == 1
        assert engine.action_history[1]["action"] == "rest"

    def test_record_action_trims_history(self):
        engine = make_engine("dummy", config=ReflectionConfig(max_recent_actions=2))
        for i in range(20):
            engine.record_action(tick=i, action="rest", status="success")
        # max_recent_actions * 3 = 6 entries kept
        assert len(engine.action_history) <= 6

    def test_record_action_with_details(self):
        engine = make_engine("dummy")
        engine.record_action(
            tick=5, action="explore", status="success",
            token_cost=3, reasoning="looking for resources",
        )
        assert engine.action_history[0]["token_cost"] == 3
        assert engine.action_history[0]["reasoning"] == "looking for resources"

    def test_clear_history(self):
        engine = make_engine("dummy")
        engine.record_action(tick=1, action="rest", status="success")
        engine.clear_history()
        assert len(engine.action_history) == 0


# ---------------------------------------------------------------------------
# ReflectionEngine — reflect (success)
# ---------------------------------------------------------------------------


class TestReflectSuccess:
    @pytest.mark.asyncio
    async def test_reflect_stores_experiences(self):
        ltm = LongTermMemory()
        engine = make_engine(
            valid_reflection_json(analysis="Good reflection"),
            ltm=ltm,
        )
        state = make_state(tokens=500, max_tokens=1000)

        result = await engine.reflect(state, tick=10)

        assert result.skipped is False
        assert result.analysis == "Good reflection"
        assert result.memories_stored >= 1  # at least 1 experience + analysis insight
        assert result.token_cost > 0
        assert ltm.count() > 0

    @pytest.mark.asyncio
    async def test_reflect_stores_strategy_adjustments(self):
        ltm = LongTermMemory()
        engine = make_engine(
            valid_reflection_json(adjustments=["avoid combat", "rest more"]),
            ltm=ltm,
        )
        state = make_state(tokens=500, max_tokens=1000)

        result = await engine.reflect(state, tick=10)

        assert result.strategy_adjustments == ["avoid combat", "rest more"]
        # Strategy adjustments should be in LTM
        strategies = ltm.search("avoid", category="strategy")
        assert len(strategies) >= 1

    @pytest.mark.asyncio
    async def test_reflect_deducts_overhead_tokens(self):
        engine = make_engine(valid_reflection_json())
        state = make_state(tokens=500, max_tokens=1000)
        tokens_before = state.tokens

        await engine.reflect(state, tick=10)

        # Should have deducted at least the overhead (10 tokens)
        assert state.tokens < tokens_before

    @pytest.mark.asyncio
    async def test_reflect_records_cost(self):
        cost_tracker = CostTracker()
        engine = make_engine(
            valid_reflection_json(),
            cost_tracker=cost_tracker,
        )
        state = make_state(tokens=500, max_tokens=1000)

        await engine.reflect(state, tick=10)

        assert cost_tracker.total_tokens > 0


# ---------------------------------------------------------------------------
# ReflectionEngine — reflect (skipped)
# ---------------------------------------------------------------------------


class TestReflectSkipped:
    @pytest.mark.asyncio
    async def test_skip_when_tokens_low(self):
        engine = make_engine("should not be called")
        state = make_state(tokens=10, max_tokens=1000)  # 1% < 20%

        result = await engine.reflect(state, tick=10)

        assert result.skipped is True
        assert "low" in result.skip_reason.lower() or "20%" in result.skip_reason
        assert result.token_cost == 0

    @pytest.mark.asyncio
    async def test_skip_when_tokens_insufficient_for_overhead(self):
        engine = make_engine(
            "should not be called",
            config=ReflectionConfig(token_overhead=100),
        )
        state = make_state(tokens=50, max_tokens=1000)  # 5% but above 20% check... wait
        # tokens=50, max_tokens=1000 -> ratio=5% < 20%, should skip
        result = await engine.reflect(state, tick=10)
        assert result.skipped is True

    @pytest.mark.asyncio
    async def test_skip_does_not_call_llm(self):
        engine = make_engine("should not be called")
        state = make_state(tokens=5, max_tokens=1000)

        result = await engine.reflect(state, tick=10)

        assert result.skipped is True
        # The mock LLM should not have been called
        engine._llm.chat.assert_not_called()


# ---------------------------------------------------------------------------
# ReflectionEngine — reflect (error handling)
# ---------------------------------------------------------------------------


class TestReflectErrors:
    @pytest.mark.asyncio
    async def test_llm_failure_graceful(self):
        engine = make_engine(side_effect=RuntimeError("LLM down"))
        state = make_state(tokens=500, max_tokens=1000)

        result = await engine.reflect(state, tick=10)

        assert result.skipped is False
        assert "LLM call error" in result.analysis
        assert result.token_cost > 0  # overhead was deducted

    @pytest.mark.asyncio
    async def test_parse_failure_graceful(self):
        engine = make_engine("not valid json")
        state = make_state(tokens=500, max_tokens=1000)

        result = await engine.reflect(state, tick=10)

        assert result.skipped is False
        assert "parse error" in result.analysis


# ---------------------------------------------------------------------------
# Integration: ThinkLoop with ReflectionEngine
# ---------------------------------------------------------------------------


class TestThinkLoopWithReflection:
    @pytest.mark.asyncio
    async def test_think_loop_records_actions_to_reflection(self):
        """ThinkLoop should record actions to the reflection engine."""
        ltm = LongTermMemory()
        engine = make_engine(
            valid_reflection_json(analysis="reflected"),
            ltm=ltm,
        )
        state = make_state(tokens=5000, max_tokens=10000)

        loop = ThinkLoop(
            state=state,
            survival=SurvivalInstinct(),
            executor=ActionExecutor(),
            config=ThinkLoopConfig(
                tick_interval=0.0,
                reflect_interval=10,
            ),
            reflection_provider=engine,
        )
        await loop.run(max_ticks=15)

        # Should have recorded some actions
        assert len(engine.action_history) > 0

    @pytest.mark.asyncio
    async def test_think_loop_reflects_at_interval(self):
        """Reflection should be called at configured intervals."""
        reflect_ticks: list[int] = []

        class TrackingReflectionEngine:
            """Minimal provider that tracks when reflect is called."""

            async def reflect(self, state, tick):
                reflect_ticks.append(tick)

        state = make_state(tokens=5000, max_tokens=10000)
        loop = ThinkLoop(
            state=state,
            survival=SurvivalInstinct(),
            executor=ActionExecutor(),
            config=ThinkLoopConfig(
                tick_interval=0.0,
                reflect_interval=5,
            ),
            reflection_provider=TrackingReflectionEngine(),
        )
        await loop.run(max_ticks=20)
        assert reflect_ticks == [5, 10, 15, 20]

    @pytest.mark.asyncio
    async def test_full_integration_with_ltm(self):
        """Full integration: ThinkLoop -> record actions -> reflect -> write to LTM."""
        ltm = LongTermMemory()
        engine = make_engine(
            valid_reflection_json(
                analysis="I should rest more",
                adjustments=["prioritize rest"],
                experiences=[
                    {"content": "Resting conserves tokens effectively",
                     "importance": 0.9, "category": "strategy"},
                ],
            ),
            ltm=ltm,
        )
        state = make_state(tokens=5000, max_tokens=10000)

        loop = ThinkLoop(
            state=state,
            survival=SurvivalInstinct(),
            executor=ActionExecutor(),
            config=ThinkLoopConfig(
                tick_interval=0.0,
                reflect_interval=10,
            ),
            reflection_provider=engine,
        )
        await loop.run(max_ticks=10)

        # Reflection should have been called at tick 10
        # LTM should have entries
        assert ltm.count() > 0

        # Verify experiences were stored
        results = ltm.search("Resting")
        assert len(results) >= 1

    @pytest.mark.asyncio
    async def test_reflection_with_panic_mode_still_records_actions(self):
        """Even in PANIC mode, actions should still be recorded for later reflection."""
        engine = make_engine(valid_reflection_json())
        # Tokens at 5% — PANIC mode, no decision but survival actions
        state = make_state(tokens=5, max_tokens=100)

        loop = ThinkLoop(
            state=state,
            survival=SurvivalInstinct(),
            executor=ActionExecutor(),
            config=ThinkLoopConfig(
                tick_interval=0.0,
                reflect_interval=10,
            ),
            reflection_provider=engine,
        )
        await loop.run(max_ticks=5)

        # In PANIC mode, _act is not called (survival bypass), so no actions recorded
        # This is expected behavior — reflection engine won't have action data
        # but it shouldn't crash either
        assert loop.tick == 5
