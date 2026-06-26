"""TraceCollector — hooks into ThinkLoop to capture TickSnapshots.

Instead of requiring a pre-built Hook system (SEN-153), the TraceCollector
instruments the ThinkLoop directly by wrapping the think_once cycle.  It
captures data at each phase boundary:

  before_sense → after_sense (Perception)
  after_survive (SurvivalAction)
  after_decide (Decision)
  after_act (ActionResult)

This approach is non-invasive: it wraps _think_once without modifying
the ThinkLoop internals.
"""

from __future__ import annotations

import asyncio
import logging
import time
from datetime import UTC, datetime
from typing import TYPE_CHECKING, Any
from uuid import UUID

from agent_runtime.tracing.models import PhaseSnapshot, TickSnapshot, TracePhase
from agent_runtime.tracing.store import TraceStore

if TYPE_CHECKING:
    from agent_runtime.tracing.pusher import TracePusher

logger = logging.getLogger(__name__)


class TraceCollector:
    """Collects tick snapshots from a ThinkLoop and persists them.

    Usage::

        store = TraceStore(db_path="traces.db")
        collector = TraceCollector(agent_id=agent.state.id, store=store)

        # Option 1: Install as wrapper on ThinkLoop
        collector.install(think_loop)

        # Option 2: Use manually in your own loop
        collector.on_tick_start(tick=1)
        collector.on_phase_end(TracePhase.SENSE, input_data=..., output_data=...)
        collector.on_tick_end()

    Args:
        agent_id: UUID of the agent being traced.
        store: TraceStore instance for persistence.
        enabled: Whether tracing is active (can be toggled at runtime).
    """

    def __init__(
        self,
        agent_id: UUID,
        store: TraceStore,
        *,
        enabled: bool = True,
        pusher: TracePusher | None = None,
    ) -> None:
        self.agent_id = agent_id
        self._store = store
        self.enabled = enabled
        self._pusher = pusher

        # Current tick being collected
        self._current: TickSnapshot | None = None
        self._tick_start_time: float = 0.0
        self._phase_start_time: float = 0.0

        # Reference to the original _think_once for unwrapping
        self._original_think_once: Any = None

    # ------------------------------------------------------------------
    # Manual collection API
    # ------------------------------------------------------------------

    def on_tick_start(self, tick: int) -> None:
        """Start collecting a new tick snapshot."""
        if not self.enabled:
            return
        now_iso = datetime.now(UTC).isoformat()
        self._tick_start_time = time.monotonic()
        self._current = TickSnapshot(
            agent_id=self.agent_id,
            tick=tick,
            started_at=now_iso,
        )

    def on_phase_start(self, phase: TracePhase) -> None:
        """Mark the start of a phase (for timing)."""
        if not self.enabled:
            return
        self._phase_start_time = time.monotonic()

    def on_phase_end(
        self,
        phase: TracePhase,
        *,
        input_data: dict[str, Any] | None = None,
        output_data: dict[str, Any] | None = None,
        error: str | None = None,
    ) -> None:
        """Record the end of a phase.

        Automatically computes duration from the last on_phase_start call.
        """
        if not self.enabled or self._current is None:
            return

        duration_ms = 0.0
        if self._phase_start_time > 0:
            duration_ms = (time.monotonic() - self._phase_start_time) * 1000
            self._phase_start_time = 0.0

        snapshot = PhaseSnapshot(
            phase=phase,
            input_data=input_data or {},
            output_data=output_data or {},
            duration_ms=duration_ms,
            error=error,
        )
        self._current.phases.append(snapshot)

    def on_tick_end(self) -> TickSnapshot | None:
        """Finalize and persist the current tick snapshot.

        Returns the saved TickSnapshot, or None if tracing is disabled.
        """
        if not self.enabled or self._current is None:
            return None

        now_iso = datetime.now(UTC).isoformat()
        self._current.finished_at = now_iso
        self._current.total_duration_ms = (
            (time.monotonic() - self._tick_start_time) * 1000
        )

        try:
            self._store.save(self._current)
        except Exception:
            logger.exception(
                "Failed to save trace for agent=%s tick=%d",
                self.agent_id,
                self._current.tick,
            )

        # Push to World Engine (fire-and-forget)
        if self._pusher is not None:
            snapshot = self._current
            try:
                loop = asyncio.get_running_loop()
                loop.create_task(self._pusher.push(snapshot))
            except RuntimeError:
                # No running event loop — skip push
                pass

        snapshot = self._current
        self._current = None
        return snapshot

    # ------------------------------------------------------------------
    # ThinkLoop integration (wrapper pattern)
    # ------------------------------------------------------------------

    def install(self, loop: Any) -> None:
        """Install the collector on a ThinkLoop by wrapping _think_once.

        This patches loop._think_once to inject tracing around each phase.
        The original method is preserved and can be restored with uninstall().
        """
        if self._original_think_once is not None:
            logger.warning("TraceCollector already installed on a loop")
            return

        self._original_think_once = loop._think_once
        collector = self

        async def _traced_think_once() -> None:
            tick = loop._tick + 1  # tick hasn't been incremented yet
            collector.on_tick_start(tick)

            try:
                # --- Sense phase ---
                collector.on_phase_start(TracePhase.SENSE)
                perception = await loop._perception.perceive(loop.state, tick)
                sense_output = {
                    "token_balance": perception.token_balance,
                    "token_ratio": perception.token_ratio,
                    "health": perception.health,
                    "tick": perception.tick,
                    "messages_count": len(perception.messages),
                    "active_task": perception.active_task,
                }
                collector.on_phase_end(
                    TracePhase.SENSE,
                    input_data={"tick": tick},
                    output_data=sense_output,
                )

                # --- Survive phase ---
                collector.on_phase_start(TracePhase.SURVIVE)
                survival_action = loop.survival.assess(loop.state)
                survive_output = {
                    "mode": survival_action.mode.value,
                    "token_ratio": survival_action.token_ratio,
                    "actions_count": len(survival_action.actions),
                }
                collector.on_phase_end(
                    TracePhase.SURVIVE,
                    input_data={"token_ratio": survival_action.token_ratio},
                    output_data=survive_output,
                )

                # Now increment tick (matching original _think_once behavior)
                loop._tick = tick

                if survival_action.mode.value in ("panic", "urgent"):
                    logger.warning(
                        "Tick %d: survival mode=%s — executing emergency actions",
                        tick,
                        survival_action.mode.value,
                    )
                    a2a = loop._world_client if loop._world_client is not None else None
                    await loop.survival.execute(survival_action, loop.state, a2a_client=a2a)
                    collector.on_tick_end()
                    return

                # --- Decide phase ---
                collector.on_phase_start(TracePhase.DECIDE)
                decision = await loop._decision.decide(
                    loop.state, perception, survival_action
                )
                decide_output = {
                    "action_type": decision.action_type.value,
                    "reasoning": decision.reasoning[:500] if decision.reasoning else "",
                    "has_parameters": bool(decision.parameters),
                }
                collector.on_phase_end(
                    TracePhase.DECIDE,
                    input_data={"survival_mode": survival_action.mode.value},
                    output_data=decide_output,
                )

                # --- Act phase ---
                collector.on_phase_start(TracePhase.ACT)
                await loop._act(decision)

                # Extract act result from executor history
                act_output: dict[str, Any] = {
                    "action_type": decision.action_type.value,
                }
                if loop.executor.history:
                    last_result = loop.executor.history[-1]
                    act_output["status"] = last_result.status.value
                    act_output["token_cost"] = last_result.token_cost
                    act_output["elapsed_ms"] = last_result.elapsed_ms
                    if last_result.error:
                        act_output["error"] = last_result.error

                collector.on_phase_end(
                    TracePhase.ACT,
                    input_data={"action_type": decision.action_type.value},
                    output_data=act_output,
                    error=act_output.get("error"),
                )

                # --- Reflect (periodic) ---
                if (
                    loop.config.reflect_interval > 0
                    and tick % loop.config.reflect_interval == 0
                ):
                    await loop._reflection.reflect(loop.state, tick)

            except Exception as exc:
                # Record the error in the current phase if possible
                collector.on_phase_end(
                    TracePhase.ACT,
                    error=str(exc),
                )
                raise

            finally:
                collector.on_tick_end()

        loop._think_once = _traced_think_once

    def uninstall(self, loop: Any) -> None:
        """Restore the original _think_once method by removing the wrapper."""
        if self._original_think_once is not None:
            # Delete the instance attribute so the class method is used
            if hasattr(loop, "_think_once"):
                del loop._think_once
            self._original_think_once = None

    # ------------------------------------------------------------------
    # Control
    # ------------------------------------------------------------------

    def enable(self) -> None:
        """Enable tracing."""
        self.enabled = True

    def disable(self) -> None:
        """Disable tracing (current tick is discarded if in progress)."""
        self.enabled = False
        self._current = None
