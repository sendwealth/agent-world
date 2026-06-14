"""Tests for the think loop (think_loop.py).

Covers:
- ThinkLoopConfig creation and defaults
- Perception and Decision data classes
- DefaultPerceptionProvider: produces valid Perception from state
- MockDecisionProvider: picks affordable random actions
- DefaultReflectionProvider: no-op
- ThinkLoop: single tick execution
- ThinkLoop: full 100-tick stability run
- ThinkLoop: error recovery (exception in perception, decision, act)
- ThinkLoop: survival bypass (PANIC/URGENT skips LLM decision)
- ThinkLoop: stop() gracefully halts the loop
- ThinkLoop: max_ticks limit
- ThinkLoop: consecutive error limit
- ThinkLoop: reflect called every N ticks
- Custom providers can be injected
- _NoOpWorldClient returns valid results
"""

from __future__ import annotations

import pytest

from agent_runtime.core.act import (
    ActionExecutor,
    ActionType,
)
from agent_runtime.core.think_loop import (
    Decision,
    DefaultPerceptionProvider,
    DefaultReflectionProvider,
    MockDecisionProvider,
    Perception,
    ThinkLoop,
    ThinkLoopConfig,
    _NoOpWorldClient,
)
from agent_runtime.models.agent_state import AgentState
from agent_runtime.models.enums import AgentPhase
from agent_runtime.survival.instinct import (
    SurvivalAction,
    SurvivalInstinct,
    SurvivalMode,
)

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def make_state(
    tokens: int = 500,
    max_tokens: int = 1000,
    *,
    name: str = "TestAgent",
) -> AgentState:
    """Create a test AgentState with reasonable defaults."""
    return AgentState(
        name=name,
        tokens=tokens,
        max_tokens=max_tokens,
        money=50.0,
        health=100.0,
        phase=AgentPhase.ADULT,
    )


# ---------------------------------------------------------------------------
# ThinkLoopConfig
# ---------------------------------------------------------------------------


class TestThinkLoopConfig:
    def test_defaults(self):
        cfg = ThinkLoopConfig()
        assert cfg.tick_interval == 1.0
        assert cfg.max_ticks == 0
        assert cfg.reflect_interval == 10
        assert cfg.error_backoff == 5.0
        assert cfg.max_consecutive_errors == 0

    def test_custom(self):
        cfg = ThinkLoopConfig(
            tick_interval=0.01,
            max_ticks=50,
            reflect_interval=5,
            error_backoff=0.1,
            max_consecutive_errors=10,
        )
        assert cfg.tick_interval == 0.01
        assert cfg.max_ticks == 50
        assert cfg.reflect_interval == 5
        assert cfg.error_backoff == 0.1
        assert cfg.max_consecutive_errors == 10


# ---------------------------------------------------------------------------
# Perception
# ---------------------------------------------------------------------------


class TestPerception:
    def test_defaults(self):
        p = Perception()
        assert p.messages == []
        assert p.token_balance == 0
        assert p.token_ratio == 0.0
        assert p.market_state == {}
        assert p.active_task is None
        assert p.health == 100.0
        assert p.tick == 0

    def test_custom(self):
        p = Perception(
            messages=[{"from": "bob", "text": "hi"}],
            token_balance=500,
            token_ratio=0.5,
            tick=42,
        )
        assert len(p.messages) == 1
        assert p.tick == 42

    def test_frozen(self):
        p = Perception()
        with pytest.raises(AttributeError):
            p.tick = 99  # type: ignore[misc]


# ---------------------------------------------------------------------------
# Decision
# ---------------------------------------------------------------------------


class TestDecision:
    def test_defaults(self):
        d = Decision(action_type=ActionType.REST)
        assert d.action_type == ActionType.REST
        assert d.parameters == {}
        assert d.reasoning == ""

    def test_frozen(self):
        d = Decision(action_type=ActionType.REST)
        with pytest.raises(AttributeError):
            d.reasoning = "x"  # type: ignore[misc]


# ---------------------------------------------------------------------------
# DefaultPerceptionProvider
# ---------------------------------------------------------------------------


class TestDefaultPerceptionProvider:
    @pytest.mark.asyncio
    async def test_perceive(self):
        state = make_state(tokens=500, max_tokens=1000)
        provider = DefaultPerceptionProvider()
        p = await provider.perceive(state, tick=5)
        assert p.token_balance == 500
        assert p.token_ratio == 0.5
        assert p.health == 100.0
        assert p.tick == 5
        assert p.messages == []

    @pytest.mark.asyncio
    async def test_perceive_zero_max_tokens(self):
        state = make_state(tokens=500, max_tokens=1000)
        # Manually set max_tokens to 0 to test division guard
        state.max_tokens = 0
        provider = DefaultPerceptionProvider()
        p = await provider.perceive(state, tick=1)
        assert p.token_ratio == 0.0


# ---------------------------------------------------------------------------
# MockDecisionProvider
# ---------------------------------------------------------------------------


class TestMockDecisionProvider:
    @pytest.mark.asyncio
    async def test_decide_returns_affordable_action(self):
        state = make_state(tokens=500, max_tokens=1000)
        executor = ActionExecutor()
        provider = MockDecisionProvider(executor)
        perception = Perception(token_balance=500, tick=1)
        survival = SurvivalAction(mode=SurvivalMode.NORMAL, token_ratio=0.5)

        decision = await provider.decide(state, perception, survival)
        assert decision.action_type in (
            ActionType.REST, ActionType.EXPLORE,
            ActionType.GATHER, ActionType.MOVE,
        )
        assert decision.reasoning != ""

    @pytest.mark.asyncio
    async def test_decide_no_affordable_actions_rests(self):
        state = make_state(tokens=0, max_tokens=1000)
        executor = ActionExecutor()
        provider = MockDecisionProvider(executor)
        perception = Perception(token_balance=0, tick=1)
        survival = SurvivalAction(mode=SurvivalMode.PANIC, token_ratio=0.0)

        decision = await provider.decide(state, perception, survival)
        # REST costs 0, so it should always be affordable
        assert decision.action_type == ActionType.REST


# ---------------------------------------------------------------------------
# DefaultReflectionProvider
# ---------------------------------------------------------------------------


class TestDefaultReflectionProvider:
    @pytest.mark.asyncio
    async def test_reflect_no_error(self):
        state = make_state()
        provider = DefaultReflectionProvider()
        # Should not raise
        await provider.reflect(state, tick=10)


# ---------------------------------------------------------------------------
# _NoOpWorldClient
# ---------------------------------------------------------------------------


class TestNoOpWorldClient:
    @pytest.mark.asyncio
    async def test_all_methods_return_ok(self):
        client = _NoOpWorldClient()
        assert (await client.send_message({}))["status"] == "ok"
        assert (await client.claim_task("t1"))["status"] == "ok"
        assert (await client.submit_task("t1", {}))["status"] == "ok"
        assert (await client.propose_deal({}))["status"] == "ok"
        assert (await client.teach_skill("a1", "python", 1))["status"] == "ok"
        assert (await client.explore({}))["status"] == "ok"


# ---------------------------------------------------------------------------
# ThinkLoop — basic
# ---------------------------------------------------------------------------


class TestThinkLoopBasic:
    def test_initial_state(self):
        state = make_state()
        loop = ThinkLoop(
            state=state,
            survival=SurvivalInstinct(),
            executor=ActionExecutor(),
            config=ThinkLoopConfig(),
        )
        assert loop.tick == 0
        assert loop.running is False
        assert loop.total_errors == 0

    @pytest.mark.asyncio
    async def test_single_tick(self):
        state = make_state(tokens=500, max_tokens=1000)
        loop = ThinkLoop(
            state=state,
            survival=SurvivalInstinct(),
            executor=ActionExecutor(),
            config=ThinkLoopConfig(tick_interval=0.0),
        )
        await loop.run(max_ticks=1)
        assert loop.tick == 1
        assert loop.total_errors == 0

    @pytest.mark.asyncio
    async def test_ten_ticks(self):
        state = make_state(tokens=5000, max_tokens=10000)
        loop = ThinkLoop(
            state=state,
            survival=SurvivalInstinct(),
            executor=ActionExecutor(),
            config=ThinkLoopConfig(tick_interval=0.0),
        )
        await loop.run(max_ticks=10)
        assert loop.tick == 10
        assert loop.total_errors == 0


# ---------------------------------------------------------------------------
# ThinkLoop — 100-tick stability test (acceptance criteria)
# ---------------------------------------------------------------------------


class TestThinkLoopStability:
    @pytest.mark.asyncio
    async def test_100_ticks_no_crash(self):
        """Acceptance test: agent runs 100 ticks without crashing."""
        state = make_state(tokens=10000, max_tokens=20000)
        loop = ThinkLoop(
            state=state,
            survival=SurvivalInstinct(),
            executor=ActionExecutor(),
            config=ThinkLoopConfig(tick_interval=0.0),
        )
        await loop.run(max_ticks=100)
        assert loop.tick == 100
        assert loop.running is False
        assert loop.total_errors == 0

    @pytest.mark.asyncio
    async def test_100_ticks_with_real_interval(self):
        """100 ticks with a small interval to simulate real timing."""
        state = make_state(tokens=10000, max_tokens=20000)
        loop = ThinkLoop(
            state=state,
            survival=SurvivalInstinct(),
            executor=ActionExecutor(),
            config=ThinkLoopConfig(tick_interval=0.001),
        )
        await loop.run(max_ticks=100)
        assert loop.tick == 100
        assert loop.total_errors == 0

    @pytest.mark.asyncio
    async def test_100_ticks_survival_panic_recovery(self):
        """Agent that starts low on tokens but can still run 100 ticks."""
        # Start with moderate tokens — survival mode should kick in
        state = make_state(tokens=50, max_tokens=1000)
        loop = ThinkLoop(
            state=state,
            survival=SurvivalInstinct(),
            executor=ActionExecutor(),
            config=ThinkLoopConfig(tick_interval=0.0),
        )
        await loop.run(max_ticks=100)
        assert loop.tick == 100
        assert loop.total_errors == 0


# ---------------------------------------------------------------------------
# ThinkLoop — error recovery
# ---------------------------------------------------------------------------


class TestThinkLoopErrorRecovery:
    @pytest.mark.asyncio
    async def test_error_in_perception_continues(self):
        """If perception raises, the loop should log and continue."""

        class BrokenPerception:
            call_count = 0

            async def perceive(self, state, tick):
                self.call_count += 1
                if self.call_count <= 3:
                    raise RuntimeError("perception failed")
                return Perception(tick=tick)

        state = make_state(tokens=5000, max_tokens=10000)
        broken = BrokenPerception()
        loop = ThinkLoop(
            state=state,
            survival=SurvivalInstinct(),
            executor=ActionExecutor(),
            config=ThinkLoopConfig(
                tick_interval=0.0,
                error_backoff=0.0,
            ),
            perception_provider=broken,
        )
        await loop.run(max_ticks=10)
        assert loop.tick == 10
        assert loop.total_errors == 3  # First 3 ticks errored

    @pytest.mark.asyncio
    async def test_error_in_decision_continues(self):
        """If decision raises, the loop should log and continue."""

        class BrokenDecision:
            call_count = 0

            async def decide(self, state, perception, survival):
                self.call_count += 1
                if self.call_count <= 2:
                    raise RuntimeError("decision failed")
                return Decision(action_type=ActionType.REST)

        state = make_state(tokens=5000, max_tokens=10000)
        broken = BrokenDecision()
        loop = ThinkLoop(
            state=state,
            survival=SurvivalInstinct(),
            executor=ActionExecutor(),
            config=ThinkLoopConfig(
                tick_interval=0.0,
                error_backoff=0.0,
            ),
            decision_provider=broken,
        )
        await loop.run(max_ticks=10)
        assert loop.tick == 10
        assert loop.total_errors == 2

    @pytest.mark.asyncio
    async def test_consecutive_error_limit(self):
        """Loop stops after max_consecutive_errors."""

        class AlwaysBroken:
            async def perceive(self, state, tick):
                raise RuntimeError("always broken")

        state = make_state()
        loop = ThinkLoop(
            state=state,
            survival=SurvivalInstinct(),
            executor=ActionExecutor(),
            config=ThinkLoopConfig(
                tick_interval=0.0,
                error_backoff=0.0,
                max_consecutive_errors=5,
            ),
            perception_provider=AlwaysBroken(),
        )
        await loop.run(max_ticks=100)
        assert loop.tick == 5
        assert loop.total_errors == 5
        assert loop.running is False


# ---------------------------------------------------------------------------
# ThinkLoop — survival bypass
# ---------------------------------------------------------------------------


class TestThinkLoopSurvivalBypass:
    @pytest.mark.asyncio
    async def test_panic_mode_skips_decision(self):
        """PANIC initially bypasses the LLM, then falls back after repeated failures."""
        decision_calls = 0

        class CountingDecision:
            async def decide(self, state, perception, survival):
                nonlocal decision_calls
                decision_calls += 1
                return Decision(action_type=ActionType.REST)

        # Tokens at 5% — should trigger PANIC
        state = make_state(tokens=5, max_tokens=100)
        counting = CountingDecision()
        loop = ThinkLoop(
            state=state,
            survival=SurvivalInstinct(),
            executor=ActionExecutor(),
            config=ThinkLoopConfig(tick_interval=0.0),
            decision_provider=counting,
        )
        await loop.run(max_ticks=5)
        assert loop.tick == 5
        # PANIC mode now always falls through to normal LLM decision-making
        # after emergency actions fire (fire-and-forget).  So every tick
        # should invoke the decision provider.
        assert decision_calls == 5

    @pytest.mark.asyncio
    async def test_urgent_mode_skips_decision(self):
        """URGENT initially bypasses the LLM, then falls back after repeated failures."""
        decision_calls = 0

        class CountingDecision:
            async def decide(self, state, perception, survival):
                nonlocal decision_calls
                decision_calls += 1
                return Decision(action_type=ActionType.REST)

        # Tokens at 15% — URGENT
        state = make_state(tokens=15, max_tokens=100)
        counting = CountingDecision()
        loop = ThinkLoop(
            state=state,
            survival=SurvivalInstinct(),
            executor=ActionExecutor(),
            config=ThinkLoopConfig(tick_interval=0.0),
            decision_provider=counting,
        )
        await loop.run(max_ticks=5)
        assert loop.tick == 5
        # URGENT mode now always falls through to normal LLM decision-making
        # after emergency actions fire (fire-and-forget).  So every tick
        # should invoke the decision provider.
        assert decision_calls == 5

    @pytest.mark.asyncio
    async def test_normal_mode_calls_decision(self):
        """NORMAL mode (40-80% tokens) should call the decision provider."""
        decision_calls = 0

        class CountingDecision:
            async def decide(self, state, perception, survival):
                nonlocal decision_calls
                decision_calls += 1
                return Decision(action_type=ActionType.REST)

        # Tokens at 60% — NORMAL
        state = make_state(tokens=600, max_tokens=1000)
        counting = CountingDecision()
        loop = ThinkLoop(
            state=state,
            survival=SurvivalInstinct(),
            executor=ActionExecutor(),
            config=ThinkLoopConfig(tick_interval=0.0),
            decision_provider=counting,
        )
        await loop.run(max_ticks=5)
        assert loop.tick == 5
        assert decision_calls == 5


# ---------------------------------------------------------------------------
# ThinkLoop — stop
# ---------------------------------------------------------------------------


class TestThinkLoopStop:
    @pytest.mark.asyncio
    async def test_stop_halts_loop(self):
        """stop() should gracefully stop the loop."""
        state = make_state(tokens=5000, max_tokens=10000)

        class StoppingPerception:
            async def perceive(self, state_ref, tick):
                if tick >= 5:
                    loop.stop()
                return Perception(tick=tick)

        loop = ThinkLoop(
            state=state,
            survival=SurvivalInstinct(),
            executor=ActionExecutor(),
            config=ThinkLoopConfig(tick_interval=0.0),
            perception_provider=StoppingPerception(),
        )
        await loop.run()
        assert loop.tick >= 5
        assert loop.running is False


# ---------------------------------------------------------------------------
# ThinkLoop — reflection
# ---------------------------------------------------------------------------


class TestThinkLoopReflection:
    @pytest.mark.asyncio
    async def test_reflect_called_at_interval(self):
        """Reflect is called every reflect_interval ticks."""
        reflect_ticks: list[int] = []

        class TrackingReflection:
            async def reflect(self, state, tick):
                reflect_ticks.append(tick)

        state = make_state(tokens=5000, max_tokens=10000)
        loop = ThinkLoop(
            state=state,
            survival=SurvivalInstinct(),
            executor=ActionExecutor(),
            config=ThinkLoopConfig(
                tick_interval=0.0,
                reflect_interval=10,
            ),
            reflection_provider=TrackingReflection(),
        )
        await loop.run(max_ticks=50)
        assert reflect_ticks == [10, 20, 30, 40, 50]

    @pytest.mark.asyncio
    async def test_reflect_not_called_when_zero(self):
        """Setting reflect_interval to 0 disables reflection."""
        reflect_ticks: list[int] = []

        class TrackingReflection:
            async def reflect(self, state, tick):
                reflect_ticks.append(tick)

        state = make_state(tokens=5000, max_tokens=10000)
        loop = ThinkLoop(
            state=state,
            survival=SurvivalInstinct(),
            executor=ActionExecutor(),
            config=ThinkLoopConfig(
                tick_interval=0.0,
                reflect_interval=0,
            ),
            reflection_provider=TrackingReflection(),
        )
        await loop.run(max_ticks=50)
        assert reflect_ticks == []


# ---------------------------------------------------------------------------
# ThinkLoop — custom providers
# ---------------------------------------------------------------------------


class TestThinkLoopCustomProviders:
    @pytest.mark.asyncio
    async def test_custom_perception_provider(self):
        custom_perceptions: list[Perception] = []

        class CustomPerception:
            async def perceive(self, state, tick):
                p = Perception(
                    tick=tick,
                    token_balance=state.tokens,
                    token_ratio=0.99,
                    messages=[{"custom": True}],
                )
                custom_perceptions.append(p)
                return p

        state = make_state(tokens=5000, max_tokens=10000)
        loop = ThinkLoop(
            state=state,
            survival=SurvivalInstinct(),
            executor=ActionExecutor(),
            config=ThinkLoopConfig(tick_interval=0.0),
            perception_provider=CustomPerception(),
        )
        await loop.run(max_ticks=3)
        assert len(custom_perceptions) == 3
        assert all(p.messages == [{"custom": True}] for p in custom_perceptions)

    @pytest.mark.asyncio
    async def test_custom_decision_provider(self):
        decisions: list[Decision] = []

        class AlwaysRest:
            async def decide(self, state, perception, survival):
                d = Decision(
                    action_type=ActionType.REST,
                    reasoning="always resting",
                )
                decisions.append(d)
                return d

        state = make_state(tokens=5000, max_tokens=10000)
        loop = ThinkLoop(
            state=state,
            survival=SurvivalInstinct(),
            executor=ActionExecutor(),
            config=ThinkLoopConfig(tick_interval=0.0),
            decision_provider=AlwaysRest(),
        )
        await loop.run(max_ticks=5)
        assert len(decisions) == 5
        assert all(d.action_type == ActionType.REST for d in decisions)


# ---------------------------------------------------------------------------
# ThinkLoop — action execution integration
# ---------------------------------------------------------------------------


class TestThinkLoopActionExecution:
    @pytest.mark.asyncio
    async def test_tokens_deducted_after_actions(self):
        """Tokens should be deducted when actions are executed."""
        initial_tokens = 5000
        state = make_state(tokens=initial_tokens, max_tokens=10000)

        class AlwaysExplore:
            async def decide(self, state_ref, perception, survival):
                return Decision(action_type=ActionType.EXPLORE)

        loop = ThinkLoop(
            state=state,
            survival=SurvivalInstinct(),
            executor=ActionExecutor(),
            config=ThinkLoopConfig(tick_interval=0.0),
            decision_provider=AlwaysExplore(),
        )
        await loop.run(max_ticks=10)
        # EXPLORE costs 3 tokens per tick
        assert state.tokens < initial_tokens
        assert loop.total_errors == 0
