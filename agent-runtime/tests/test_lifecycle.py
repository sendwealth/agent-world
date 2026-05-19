"""Tests for lifecycle sync service, death handler, and E2E lifecycle flows.

Covers the lifecycle module (agent_runtime/lifecycle/) and its integration
with ThinkLoop and AgentState.
"""

import asyncio

import pytest

from agent_runtime.core.act import ActionContext, ActionExecutor, ActionType
from agent_runtime.core.think_loop import Decision, ThinkLoop, ThinkLoopConfig
from agent_runtime.lifecycle import (
    DeathHandler,
    DeathHandlerConfig,
    LifecycleEvent,
    LifecycleSyncService,
    LifecycleTransitionGuard,
)
from agent_runtime.models.agent_state import AgentState
from agent_runtime.models.enums import AgentPhase, DeathReason
from agent_runtime.models.phase_abilities import (
    PhaseAbilities,
    get_phase_abilities,
    is_alive,
    is_terminal,
)
from agent_runtime.survival.instinct import SurvivalInstinct


# ---------------------------------------------------------------------------
# PhaseAbilities tests (comprehensive)
# ---------------------------------------------------------------------------


class TestGetPhaseAbilitiesComprehensive:
    """Exhaustive tests for get_phase_abilities covering all phases."""

    def test_every_phase_has_defined_abilities(self):
        """Every AgentPhase must return a valid PhaseAbilities."""
        for phase in AgentPhase:
            ab = get_phase_abilities(phase)
            assert isinstance(ab, PhaseAbilities)
            assert 0.0 <= ab.skill_efficiency <= 1.0

    def test_only_adult_and_elder_can_trade(self):
        for phase in AgentPhase:
            ab = get_phase_abilities(phase)
            if phase in (AgentPhase.ADULT, AgentPhase.ELDER):
                assert ab.can_trade is True
            else:
                assert ab.can_trade is False

    def test_only_adult_elder_dying_can_write_will(self):
        for phase in AgentPhase:
            ab = get_phase_abilities(phase)
            if phase in (AgentPhase.ADULT, AgentPhase.ELDER, AgentPhase.DYING):
                assert ab.can_write_will is True
            else:
                assert ab.can_write_will is False

    def test_dead_cannot_communicate(self):
        ab = get_phase_abilities(AgentPhase.DEAD)
        assert ab.can_communicate is False

    def test_dying_can_communicate_but_not_act(self):
        ab = get_phase_abilities(AgentPhase.DYING)
        assert ab.can_communicate is True
        assert ab.can_take_tasks is False
        assert ab.can_trade is False
        assert ab.can_learn is False


# ---------------------------------------------------------------------------
# LifecycleSyncService tests
# ---------------------------------------------------------------------------


class MockWorldProvider:
    """Mock world state provider for testing."""

    def __init__(self, state_data=None):
        self._state = state_data

    async def get_agent_state(self, agent_id):
        return self._state


class TestLifecycleSyncService:
    @pytest.mark.asyncio
    async def test_no_events_when_no_provider(self):
        state = AgentState(name="TestAgent", phase=AgentPhase.BIRTH)
        sync = LifecycleSyncService(world_provider=None)
        events = await sync.sync(state)
        assert events == []

    @pytest.mark.asyncio
    async def test_no_events_when_remote_is_older(self):
        state = AgentState(name="TestAgent", phase=AgentPhase.ADULT)
        state.world_sync_version = 10

        remote = {
            "phase": "elder",
            "tokens": 500,
            "money": 50.0,
            "health": 100.0,
            "reputation": 0.0,
            "survival_mode": "conservation",
            "skills": {},
            "personality": {},
            "world_sync_version": 5,  # Older
        }
        provider = MockWorldProvider(remote)
        sync = LifecycleSyncService(world_provider=provider)
        events = await sync.sync(state)
        assert events == []
        assert state.phase == AgentPhase.ADULT  # Unchanged

    @pytest.mark.asyncio
    async def test_phase_change_event(self):
        state = AgentState(name="TestAgent", phase=AgentPhase.CHILDHOOD)
        state.world_sync_version = 0

        remote = {
            "name": "TestAgent",
            "phase": "adult",
            "tokens": 500,
            "money": 50.0,
            "health": 100.0,
            "reputation": 0.0,
            "survival_mode": "conservation",
            "skills": {},
            "personality": {},
            "world_sync_version": 5,
        }
        provider = MockWorldProvider(remote)
        sync = LifecycleSyncService(world_provider=provider)
        events = await sync.sync(state)

        assert len(events) == 1
        assert events[0].old_phase == AgentPhase.CHILDHOOD
        assert events[0].new_phase == AgentPhase.ADULT
        assert state.phase == AgentPhase.ADULT

    @pytest.mark.asyncio
    async def test_dying_event_with_death_reason(self):
        state = AgentState(name="TestAgent", phase=AgentPhase.ADULT)
        state.world_sync_version = 0

        remote = {
            "name": "TestAgent",
            "phase": "dying",
            "death_reason": "token_depleted",
            "tokens": 0,
            "money": 50.0,
            "health": 100.0,
            "reputation": 0.0,
            "survival_mode": "conservation",
            "skills": {},
            "personality": {},
            "world_sync_version": 10,
        }
        provider = MockWorldProvider(remote)
        sync = LifecycleSyncService(world_provider=provider)
        events = await sync.sync(state)

        assert len(events) == 1
        assert events[0].new_phase == AgentPhase.DYING
        assert events[0].death_reason == DeathReason.TOKEN_DEPLETED

    @pytest.mark.asyncio
    async def test_dead_event(self):
        state = AgentState(name="TestAgent", phase=AgentPhase.DYING)
        state.world_sync_version = 5

        remote = {
            "name": "TestAgent",
            "phase": "dead",
            "death_reason": "natural_death",
            "tokens": 0,
            "money": 50.0,
            "health": 0.0,
            "reputation": 0.0,
            "survival_mode": "conservation",
            "skills": {},
            "personality": {},
            "world_sync_version": 15,
        }
        provider = MockWorldProvider(remote)
        sync = LifecycleSyncService(world_provider=provider)
        events = await sync.sync(state)

        assert len(events) == 1
        assert events[0].new_phase == AgentPhase.DEAD
        assert events[0].death_reason == DeathReason.NATURAL_DEATH
        assert state.phase == AgentPhase.DEAD


# ---------------------------------------------------------------------------
# LifecycleTransitionGuard tests
# ---------------------------------------------------------------------------


class TestLifecycleTransitionGuard:
    def test_adult_can_claim_task(self):
        guard = LifecycleTransitionGuard()
        state = AgentState(name="TestAgent", phase=AgentPhase.ADULT)
        assert guard.can_execute_action(state, "claim_task") is True

    def test_birth_cannot_claim_task(self):
        guard = LifecycleTransitionGuard()
        state = AgentState(name="TestAgent", phase=AgentPhase.BIRTH)
        assert guard.can_execute_action(state, "claim_task") is False

    def test_childhood_cannot_trade(self):
        guard = LifecycleTransitionGuard()
        state = AgentState(name="TestAgent", phase=AgentPhase.CHILDHOOD)
        assert guard.can_execute_action(state, "propose_deal") is False

    def test_dead_cannot_do_anything(self):
        guard = LifecycleTransitionGuard()
        state = AgentState(name="TestAgent", phase=AgentPhase.DEAD)
        assert guard.can_execute_action(state, "claim_task") is False
        assert guard.can_execute_action(state, "send_message") is False
        assert guard.can_execute_action(state, "rest") is False

    def test_dying_can_communicate(self):
        guard = LifecycleTransitionGuard()
        state = AgentState(name="TestAgent", phase=AgentPhase.DYING)
        assert guard.can_execute_action(state, "send_message") is True
        assert guard.can_execute_action(state, "claim_task") is False

    def test_check_returns_phase_abilities(self):
        guard = LifecycleTransitionGuard()
        state = AgentState(name="TestAgent", phase=AgentPhase.ADULT)
        abilities = guard.check(state)
        assert isinstance(abilities, PhaseAbilities)
        assert abilities.can_trade is True


# ---------------------------------------------------------------------------
# DeathHandler tests
# ---------------------------------------------------------------------------


class TestDeathHandler:
    def test_enter_dying_creates_will(self):
        handler = DeathHandler()
        state = AgentState(name="DyingAgent", phase=AgentPhase.ADULT, tokens=500)
        result = handler.on_enter_dying(state, tick=100, death_reason=DeathReason.TOKEN_DEPLETED)

        assert result["phase"] == "dying"
        assert result["will_created"] is True
        assert result["will"]["total_tokens"] == 500
        assert result["will"]["auto_created"] is True

    def test_enter_dying_no_auto_will(self):
        config = DeathHandlerConfig(auto_create_will=False)
        handler = DeathHandler(config=config)
        state = AgentState(name="DyingAgent", phase=AgentPhase.ADULT, tokens=500)
        result = handler.on_enter_dying(state, tick=100)

        assert result["will_created"] is False

    def test_enter_dead_records_cleanup(self):
        handler = DeathHandler()
        state = AgentState(name="DeadAgent", phase=AgentPhase.DYING, tokens=50)
        result = handler.on_enter_dead(state, tick=120, death_reason=DeathReason.TOKEN_DEPLETED)

        assert result["phase"] == "dead"
        assert result["tokens_remaining"] == 50
        assert result["death_reason"] == "token_depleted"

    def test_should_stop_when_dead(self):
        handler = DeathHandler()
        state = AgentState(name="DeadAgent", phase=AgentPhase.DEAD)
        assert handler.should_stop(state, current_tick=200) is True

    def test_should_not_stop_when_adult(self):
        handler = DeathHandler()
        state = AgentState(name="AdultAgent", phase=AgentPhase.ADULT)
        assert handler.should_stop(state, current_tick=100) is False

    def test_should_stop_after_dying_grace_period(self):
        config = DeathHandlerConfig(dying_grace_ticks=5)
        handler = DeathHandler(config=config)
        state = AgentState(name="DyingAgent", phase=AgentPhase.DYING)

        # Trigger dying at tick 100
        handler.on_enter_dying(state, tick=100)

        # Still within grace period
        assert handler.should_stop(state, current_tick=104) is False

        # Grace period expired
        assert handler.should_stop(state, current_tick=105) is True

    def test_auto_will_includes_skills(self):
        handler = DeathHandler()
        from agent_runtime.models.skill import Skill

        state = AgentState(
            name="SkilledAgent",
            phase=AgentPhase.ADULT,
            tokens=1000,
            skills={
                "mining": Skill(name="mining", level=5),
                "trading": Skill(name="trading", level=3),
            },
        )
        result = handler.on_enter_dying(state, tick=50)
        will = result["will"]

        assert will["skills"]["mining"] == 5
        assert will["skills"]["trading"] == 3


# ---------------------------------------------------------------------------
# E2E Lifecycle flow tests
# ---------------------------------------------------------------------------


class TestE2ELifecycleFlow:
    """End-to-end tests for lifecycle progression through the ThinkLoop."""

    @pytest.mark.asyncio
    async def test_dead_agent_stops_loop(self):
        """An agent in Dead phase should stop the think loop immediately."""
        state = AgentState(name="DeadAgent", phase=AgentPhase.DEAD, tokens=0)

        loop = ThinkLoop(
            state=state,
            survival=SurvivalInstinct(),
            executor=ActionExecutor(),
            config=ThinkLoopConfig(tick_interval=0.0),
        )
        await loop.run(max_ticks=10)
        # Should stop after first tick (dead check)
        assert loop.tick <= 2  # at most 1-2 ticks before dead check stops it

    @pytest.mark.asyncio
    async def test_birth_phase_blocks_explore(self):
        """In Birth phase, explore should be blocked and fall back to rest."""
        state = AgentState(
            name="BabyAgent",
            phase=AgentPhase.BIRTH,
            tokens=5000,
            max_tokens=10000,
        )

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
        await loop.run(max_ticks=5)

        # EXPLORE costs 3 tokens but is blocked in Birth, so falls back to REST (cost 0)
        assert state.tokens == 5000
        assert loop.total_errors == 0

    @pytest.mark.asyncio
    async def test_childhood_cannot_trade(self):
        """Childhood agents cannot trade — propose_deal should be blocked."""
        state = AgentState(
            name="ChildAgent",
            phase=AgentPhase.CHILDHOOD,
            tokens=5000,
            max_tokens=10000,
        )

        class AlwaysTrade:
            async def decide(self, state_ref, perception, survival):
                return Decision(action_type=ActionType.PROPOSE_DEAL)

        loop = ThinkLoop(
            state=state,
            survival=SurvivalInstinct(),
            executor=ActionExecutor(),
            config=ThinkLoopConfig(tick_interval=0.0),
            decision_provider=AlwaysTrade(),
        )
        await loop.run(max_ticks=5)

        # PROPOSE_DEAL costs 10 but is blocked in Childhood, falls to REST (cost 0)
        assert state.tokens == 5000
        assert loop.total_errors == 0

    @pytest.mark.asyncio
    async def test_adult_can_do_everything(self):
        """Adult agents have full abilities — explore should work."""
        state = AgentState(
            name="AdultAgent",
            phase=AgentPhase.ADULT,
            tokens=5000,
            max_tokens=10000,
        )

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

        # EXPLORE costs 3 tokens per tick, 10 ticks = 30 tokens
        assert state.tokens == 5000 - 30
        assert loop.total_errors == 0

    @pytest.mark.asyncio
    async def test_lifecycle_progression_sync(self):
        """Simulate full lifecycle: Birth → Childhood → Adult → Elder → Dying → Dead."""
        state = AgentState(name="LifeAgent", phase=AgentPhase.BIRTH, tokens=10000)
        state.world_sync_version = 0

        # Simulate World Engine driving phase transitions via sync
        transitions = [
            (1, "childhood"),
            (101, "adult"),
            (1101, "elder"),
            (1250, "dying", "token_depleted"),
            (1260, "dead", "token_depleted"),
        ]

        for transition_data in transitions:
            tick = transition_data[0]
            phase_str = transition_data[1]
            death_reason = transition_data[2] if len(transition_data) > 2 else None

            remote = {
                "name": "LifeAgent",
                "phase": phase_str,
                "tokens": state.tokens,
                "money": state.money,
                "health": state.health,
                "reputation": state.reputation,
                "survival_mode": "conservation",
                "skills": {},
                "personality": {},
                "world_sync_version": state.world_sync_version + 1,
            }
            if death_reason:
                remote["death_reason"] = death_reason

            provider = MockWorldProvider(remote)
            sync = LifecycleSyncService(world_provider=provider)
            events = await sync.sync(state)

            # Each transition should produce an event
            assert len(events) == 1, f"Expected event for {phase_str} at tick {tick}"
            assert events[0].new_phase == AgentPhase(phase_str)

        # Final state should be Dead
        assert state.phase == AgentPhase.DEAD

    @pytest.mark.asyncio
    async def test_death_handler_with_think_loop(self):
        """Death handler should integrate with ThinkLoop to stop on death."""
        state = AgentState(name="MortalAgent", phase=AgentPhase.ADULT, tokens=10000)

        class KillAtTick5:
            """After tick 5, transition to Dead."""

            def __init__(self):
                self.handler = DeathHandler()
                self.killed = False

            async def decide(self, state_ref, perception, survival):
                if perception.tick >= 5 and not self.killed:
                    state_ref.transition_phase(AgentPhase.DYING)
                    self.handler.on_enter_dying(
                        state_ref, tick=perception.tick,
                        death_reason=DeathReason.TOKEN_DEPLETED,
                    )
                    self.killed = True
                return Decision(action_type=ActionType.REST)

        loop = ThinkLoop(
            state=state,
            survival=SurvivalInstinct(),
            executor=ActionExecutor(),
            config=ThinkLoopConfig(tick_interval=0.0),
            decision_provider=KillAtTick5(),
        )
        await loop.run(max_ticks=20)

        # Should have run at least 5 ticks and agent should be Dying
        assert state.phase == AgentPhase.DYING


# ---------------------------------------------------------------------------
# Will creation tests
# ---------------------------------------------------------------------------


class TestWillCreation:
    """Tests for will creation during the Dying phase."""

    def test_will_includes_all_assets(self):
        from agent_runtime.models.skill import Skill

        handler = DeathHandler()
        state = AgentState(
            name="WealthyAgent",
            phase=AgentPhase.ADULT,
            tokens=5000,
            money=200.0,
            skills={
                "mining": Skill(name="mining", level=8),
                "crafting": Skill(name="crafting", level=4),
            },
        )
        result = handler.on_enter_dying(state, tick=500)
        will = result["will"]

        assert will["testator_name"] == "WealthyAgent"
        assert will["total_tokens"] == 5000
        assert will["skills"]["mining"] == 8
        assert will["skills"]["crafting"] == 4
        assert will["distribution"] == "equal"
        assert will["auto_created"] is True

    def test_will_not_created_when_disabled(self):
        config = DeathHandlerConfig(auto_create_will=False)
        handler = DeathHandler(config=config)
        state = AgentState(name="NoWillAgent", phase=AgentPhase.ADULT, tokens=100)
        result = handler.on_enter_dying(state, tick=50)

        assert result["will_created"] is False
        assert "will" not in result
