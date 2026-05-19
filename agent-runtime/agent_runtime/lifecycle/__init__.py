"""Lifecycle sync — keeps Agent Runtime state in sync with World Engine.

The World Engine owns the ground truth for lifecycle state (phase, spawn tick,
death reason).  The Agent Runtime periodically syncs via A2A/world-client
and applies updates to its local AgentState.

This module provides:
    - ``LifecycleSyncService``: polls World Engine each tick and applies phase updates.
    - ``LifecycleTransitionGuard``: checks phase abilities before allowing actions.
    - ``DeathHandler``: handles graceful shutdown when the agent enters Dying/Dead.
"""

from __future__ import annotations

import logging
from dataclasses import dataclass, field
from typing import Any, Protocol

from agent_runtime.models.agent_state import AgentState
from agent_runtime.models.enums import AgentPhase, DeathReason
from agent_runtime.models.phase_abilities import (
    PhaseAbilities,
    get_phase_abilities,
    is_terminal,
)
from agent_runtime.models.phase_abilities import (
    is_alive as is_alive,
)

logger = logging.getLogger(__name__)


# ---------------------------------------------------------------------------
# Protocols
# ---------------------------------------------------------------------------


class WorldStateProvider(Protocol):
    """Provides current lifecycle state from the World Engine."""

    async def get_agent_state(self, agent_id: str) -> dict[str, Any] | None: ...


# ---------------------------------------------------------------------------
# Transition result
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class LifecycleEvent:
    """An event emitted when a lifecycle transition occurs."""

    agent_id: str
    old_phase: AgentPhase
    new_phase: AgentPhase
    reason: str = ""
    death_reason: DeathReason | None = None


# ---------------------------------------------------------------------------
# Lifecycle sync service
# ---------------------------------------------------------------------------


class LifecycleSyncService:
    """Syncs agent lifecycle state from World Engine each tick.

    Usage::

        sync = LifecycleSyncService(world_provider)
        events = await sync.sync(state)
        for event in events:
            logger.info("Phase changed: %s -> %s", event.old_phase, event.new_phase)
    """

    def __init__(self, world_provider: WorldStateProvider | None = None) -> None:
        self._world_provider = world_provider
        self._last_synced_phase: AgentPhase | None = None

    async def sync(self, state: AgentState) -> list[LifecycleEvent]:
        """Synchronise lifecycle state from the World Engine.

        Returns a (possibly empty) list of lifecycle events.
        """
        events: list[LifecycleEvent] = []

        if self._world_provider is None:
            return events

        remote = await self._world_provider.get_agent_state(str(state.id))
        if remote is None:
            return events

        remote_version = remote.get("world_sync_version", 0)
        if remote_version <= state.world_sync_version:
            return events  # Already up to date

        # Extract phase from remote
        remote_phase_str = remote.get("phase", "")
        try:
            remote_phase = AgentPhase(remote_phase_str)
        except ValueError:
            logger.warning(
                "Unknown phase '%s' from World Engine, ignoring", remote_phase_str
            )
            return events

        old_phase = state.phase
        if remote_phase != old_phase:
            # Phase changed — record event
            death_reason = None
            reason = f"World Engine transition: {old_phase.value} -> {remote_phase.value}"

            if remote_phase == AgentPhase.DYING:
                reason_str = remote.get("death_reason", "")
                try:
                    death_reason = DeathReason(reason_str)
                except ValueError:
                    death_reason = None
                reason = f"Agent entering Dying phase: {reason_str}"

            elif remote_phase == AgentPhase.DEAD:
                reason_str = remote.get("death_reason", "")
                try:
                    death_reason = DeathReason(reason_str)
                except ValueError:
                    death_reason = None
                reason = f"Agent died: {reason_str}"

            event = LifecycleEvent(
                agent_id=str(state.id),
                old_phase=old_phase,
                new_phase=remote_phase,
                reason=reason,
                death_reason=death_reason,
            )
            events.append(event)

            # Apply the sync
            state.apply_sync(remote)
            self._last_synced_phase = remote_phase

            logger.info(
                "Lifecycle sync: phase %s -> %s (version %d -> %d)",
                old_phase.value,
                remote_phase.value,
                state.world_sync_version - 1,
                remote_version,
            )
        else:
            # No phase change, but apply other state updates
            state.apply_sync(remote)

        return events


# ---------------------------------------------------------------------------
# Transition guard — checks phase abilities before actions
# ---------------------------------------------------------------------------


class LifecycleTransitionGuard:
    """Validates that an agent can perform an action given its current phase.

    Usage::

        guard = LifecycleTransitionGuard()
        abilities = guard.check(state)
        if not abilities.can_trade:
            raise RuntimeError("Cannot trade in current phase")
    """

    def check(self, state: AgentState) -> PhaseAbilities:
        """Return the abilities for the agent's current phase."""
        return get_phase_abilities(state.phase)

    def can_execute_action(self, state: AgentState, action_type: str) -> bool:
        """Check if the agent can execute a specific action type.

        Maps action types to their required phase abilities.
        """
        abilities = get_phase_abilities(state.phase)

        # Dead agents can do nothing
        if is_terminal(state.phase):
            return False

        action_ability_map: dict[str, str] = {
            "claim_task": "can_take_tasks",
            "submit_task": "can_take_tasks",
            "propose_deal": "can_trade",
            "teach_skill": "can_teach",
            "send_message": "can_communicate",
            "explore": "can_take_tasks",
        }

        required_ability = action_ability_map.get(action_type)
        if required_ability is None:
            # Unknown actions are allowed (e.g., rest)
            return True

        return getattr(abilities, required_ability, False)


# ---------------------------------------------------------------------------
# Death handler — graceful shutdown on death
# ---------------------------------------------------------------------------


@dataclass
class DeathHandlerConfig:
    """Configuration for death handling."""

    # Whether to create a will automatically when entering Dying phase
    auto_create_will: bool = True
    # Maximum ticks to wait in Dying phase before force-exiting
    dying_grace_ticks: int = 10


class DeathHandler:
    """Handles agent death — will creation and graceful shutdown.

    When the agent enters the Dying phase:
    1. Automatically creates a will (if auto_create_will is enabled)
    2. Emits lifecycle events for dashboard visibility
    3. After grace ticks, signals the think loop to stop

    When the agent enters Dead phase:
    1. Performs final state cleanup
    2. Signals the think loop to stop immediately
    """

    def __init__(self, config: DeathHandlerConfig | None = None) -> None:
        self.config = config or DeathHandlerConfig()
        self._dying_start_tick: int | None = None
        self._will_created: bool = False
        self._callbacks: list[Any] = field(default_factory=list)

    def on_enter_dying(
        self,
        state: AgentState,
        tick: int,
        death_reason: DeathReason | None = None,
    ) -> dict[str, Any]:
        """Called when agent enters Dying phase.

        Returns a dict with will data and lifecycle info.
        """
        self._dying_start_tick = tick
        self._will_created = False

        abilities = get_phase_abilities(AgentPhase.DYING)

        result: dict[str, Any] = {
            "phase": "dying",
            "tick": tick,
            "death_reason": death_reason.value if death_reason else None,
            "can_write_will": abilities.can_write_will,
            "can_communicate": abilities.can_communicate,
            "will_created": False,
        }

        # Auto-create will if possible
        if self.config.auto_create_will and abilities.can_write_will:
            will_data = self._create_auto_will(state)
            result["will_created"] = True
            result["will"] = will_data
            self._will_created = True

        logger.info(
            "Agent %s entering Dying phase at tick %d (reason: %s, will: %s)",
            state.name,
            tick,
            death_reason.value if death_reason else "unknown",
            self._will_created,
        )

        return result

    def on_enter_dead(
        self,
        state: AgentState,
        tick: int,
        death_reason: DeathReason | None = None,
    ) -> dict[str, Any]:
        """Called when agent enters Dead phase.

        Performs final cleanup. The think loop should stop after this.
        """
        result: dict[str, Any] = {
            "phase": "dead",
            "tick": tick,
            "death_reason": death_reason.value if death_reason else None,
            "tokens_remaining": state.tokens,
            "skills_count": len(state.skills),
        }

        logger.info(
            "Agent %s died at tick %d (reason: %s, tokens: %d, skills: %d)",
            state.name,
            tick,
            death_reason.value if death_reason else "unknown",
            state.tokens,
            len(state.skills),
        )

        return result

    def should_stop(self, state: AgentState, current_tick: int) -> bool:
        """Check if the think loop should stop due to death."""
        if state.phase == AgentPhase.DEAD:
            return True

        if state.phase == AgentPhase.DYING and self._dying_start_tick is not None:
            ticks_in_dying = current_tick - self._dying_start_tick
            if ticks_in_dying >= self.config.dying_grace_ticks:
                logger.info(
                    "Dying grace period expired (%d ticks), forcing stop",
                    ticks_in_dying,
                )
                return True

        return False

    def _create_auto_will(self, state: AgentState) -> dict[str, Any]:
        """Create an automatic will distributing assets equally.

        In a real implementation, this would use the World Engine's
        InheritanceSystem. For now, we create a local will spec.
        """
        return {
            "testator_id": str(state.id),
            "testator_name": state.name,
            "total_tokens": state.tokens,
            "skills": {name: skill.level for name, skill in state.skills.items()},
            "distribution": "equal",  # Equal split among known contacts
            "auto_created": True,
        }
