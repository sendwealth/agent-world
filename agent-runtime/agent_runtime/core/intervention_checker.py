"""InterventionChecker — pre-dispatch safety gate for agent actions.

This module implements the **runtime-side** safety checks that run *before*
an action is dispatched to the world engine. It acts as the first line of
defense; the world engine provides a second, independent layer via the
``InterventionCheckerSubsystem`` (Rust).

Safety rules implemented:
  1. **Broadcast rate-limit** — cap broadcasts per agent per time window.
  2. **Message size limit** — reject oversized payloads.
  3. **Newbie protection** — prevent newly-spawned agents from being
     targeted by aggressive actions.
  4. **Token sufficiency** — block high-cost actions when tokens are low.
  5. **Death lock** — dead/dying agents cannot perform any action.

Usage::

    from agent_runtime.core.intervention_checker import InterventionChecker

    checker = InterventionChecker()
    result = checker.check(action_type, agent_state, parameters)
    if result.blocked:
        # action rejected
        ...
    else:
        # proceed with dispatch
        ...
"""

from __future__ import annotations

import logging
import time
from dataclasses import dataclass, field
from enum import Enum
from typing import Any, Optional

logger = logging.getLogger(__name__)


# ---------------------------------------------------------------------------
# Result types
# ---------------------------------------------------------------------------

class CheckVerdict(str, Enum):
    """Outcome of a single safety check."""
    PASS = "pass"
    BLOCKED = "blocked"


@dataclass(frozen=True)
class CheckResult:
    """Result from running all safety checks against an action.

    Attributes:
        verdict: Overall pass/blocked decision.
        rule: The rule ID that triggered the block (e.g. ``"IC-01"``), or
            ``None`` if the action passed all checks.
        reason: Human-readable explanation for the block.
        details: Optional machine-readable details for audit logging.
    """

    verdict: CheckVerdict
    rule: Optional[str] = None
    reason: Optional[str] = None
    details: dict[str, Any] = field(default_factory=dict)

    @property
    def blocked(self) -> bool:
        return self.verdict == CheckVerdict.BLOCKED


# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------

@dataclass
class InterventionConfig:
    """Tunable parameters for the safety rules.

    All values have sensible defaults; override only when needed.
    """

    # IC-01: Broadcast rate-limit
    broadcast_max_per_window: int = 5
    broadcast_window_seconds: float = 10.0

    # IC-02: Message size limit
    max_payload_bytes: int = 65_536  # 64 KiB

    # IC-03: Newbie protection
    newbie_protection_ticks: int = 10

    # IC-04: Token low-water mark
    token_low_water_mark: int = 0  # actions costing >0 are blocked when tokens <= this

    # IC-05: Death lock — always enforced, no config needed


# ---------------------------------------------------------------------------
# Broadcast rate tracker (per-agent state)
# ---------------------------------------------------------------------------

class _BroadcastTracker:
    """Sliding-window rate tracker for broadcasts."""

    def __init__(self, max_per_window: int, window_seconds: float) -> None:
        self._max = max_per_window
        self._window = window_seconds
        self._timestamps: dict[str, list[float]] = {}

    def record(self, agent_id: str) -> None:
        """Record a broadcast attempt for *agent_id*."""
        now = time.monotonic()
        ts = self._timestamps.setdefault(agent_id, [])
        ts.append(now)
        self._evict(agent_id, now)

    def is_limited(self, agent_id: str) -> bool:
        """Return ``True`` if *agent_id* has exceeded the rate limit."""
        now = time.monotonic()
        ts = self._timestamps.setdefault(agent_id, [])
        self._evict(agent_id, now)
        return len(ts) >= self._max

    def _evict(self, agent_id: str, now: float) -> None:
        ts = self._timestamps.get(agent_id, [])
        cutoff = now - self._window
        while ts and ts[0] < cutoff:
            ts.pop(0)


# ---------------------------------------------------------------------------
# Agent state protocol (minimal interface for checks)
# ---------------------------------------------------------------------------

class AgentCheckProtocol:
    """Minimal interface the checker needs from agent state.

    This is a Protocol-like contract — any object with these
    attributes satisfies it.
    """

    # These are expected to be present on the agent state object:
    #   id: UUID
    #   tokens: int
    #   tick: int
    #   phase: AgentPhase  (must have a .value that can be compared)
    pass


# ---------------------------------------------------------------------------
# InterventionChecker
# ---------------------------------------------------------------------------

# Phases that indicate the agent is dead/dying and cannot act.
_DEAD_PHASES = {"dying", "dead", "Dead", "Dying"}

# Action types that constitute a broadcast (empty `to_agent` target).
_BROADCAST_ACTIONS = {"send_message", "broadcast"}


class InterventionChecker:
    """Pre-dispatch safety gate.

    Call :meth:`check` *before* dispatching any action. If the result is
    blocked, the action must **not** be sent to the world engine.
    """

    def __init__(self, config: InterventionConfig | None = None) -> None:
        self._config = config or InterventionConfig()
        self._broadcast_tracker = _BroadcastTracker(
            max_per_window=self._config.broadcast_max_per_window,
            window_seconds=self._config.broadcast_window_seconds,
        )
        # Audit log: list of (timestamp, agent_id, action, rule, reason)
        self._audit_log: list[tuple[float, str, str, str, str]] = []

    # ------------------------------------------------------------------
    # Public API
    # ------------------------------------------------------------------

    def check(
        self,
        action_type: str,
        agent_state: Any,
        parameters: dict[str, Any] | None = None,
    ) -> CheckResult:
        """Run all safety checks and return the combined result.

        Parameters:
            action_type: The action being attempted (e.g. ``"send_message"``).
            agent_state: The agent's current state (must have ``id``,
                ``tokens``, ``tick``, ``phase`` attributes).
            parameters: Action parameters (may contain ``payload``,
                ``to_agent``, etc.).

        Returns:
            A :class:`CheckResult` — check ``result.blocked`` to decide
            whether to proceed.
        """
        params = parameters or {}
        agent_id = str(getattr(agent_state, "id", "unknown"))

        # IC-05: Death lock — dead/dying agents cannot act
        result = self._check_death_lock(action_type, agent_state)
        if result.blocked:
            self._log_audit(agent_id, action_type, result.rule, result.reason)
            return result

        # IC-01: Broadcast rate-limit
        result = self._check_broadcast_rate(action_type, agent_state, params)
        if result.blocked:
            self._log_audit(agent_id, action_type, result.rule, result.reason)
            return result

        # IC-02: Message size limit
        result = self._check_message_size(action_type, params)
        if result.blocked:
            self._log_audit(agent_id, action_type, result.rule, result.reason)
            return result

        # IC-03: Newbie protection (target check)
        result = self._check_newbie_protection(action_type, agent_state, params)
        if result.blocked:
            self._log_audit(agent_id, action_type, result.rule, result.reason)
            return result

        # IC-04: Token sufficiency for high-cost actions
        result = self._check_token_sufficiency(action_type, agent_state)
        if result.blocked:
            self._log_audit(agent_id, action_type, result.rule, result.reason)
            return result

        return CheckResult(verdict=CheckVerdict.PASS)

    @property
    def audit_log(self) -> list[tuple[float, str, str, str, str]]:
        """Read-only access to the audit log."""
        return list(self._audit_log)

    def clear_audit_log(self) -> None:
        """Clear the audit log."""
        self._audit_log.clear()

    # ------------------------------------------------------------------
    # Individual checks
    # ------------------------------------------------------------------

    def _check_death_lock(self, action_type: str, agent_state: Any) -> CheckResult:
        """IC-05: Dead/dying agents cannot execute any action."""
        phase = getattr(agent_state, "phase", None)
        if phase is None:
            return CheckResult(verdict=CheckVerdict.PASS)

        phase_str = phase.value if hasattr(phase, "value") else str(phase)
        if phase_str in _DEAD_PHASES:
            return CheckResult(
                verdict=CheckVerdict.BLOCKED,
                rule="IC-05",
                reason=f"Agent in {phase_str} phase cannot perform actions",
                details={"phase": phase_str, "action": action_type},
            )
        return CheckResult(verdict=CheckVerdict.PASS)

    def _check_broadcast_rate(
        self,
        action_type: str,
        agent_state: Any,
        parameters: dict[str, Any],
    ) -> CheckResult:
        """IC-01: Limit broadcast frequency per agent."""
        if action_type not in _BROADCAST_ACTIONS:
            return CheckResult(verdict=CheckVerdict.PASS)

        # A send_message with no to_agent is a broadcast
        if action_type == "send_message":
            to_agent = (
                parameters.get("to_agent")
                or parameters.get("payload", {}).get("to_agent", "")
            )
            if to_agent:
                return CheckResult(verdict=CheckVerdict.PASS)

        agent_id = str(getattr(agent_state, "id", "unknown"))
        if self._broadcast_tracker.is_limited(agent_id):
            return CheckResult(
                verdict=CheckVerdict.BLOCKED,
                rule="IC-01",
                reason=(
                    f"Broadcast rate limit exceeded "
                    f"(max {self._config.broadcast_max_per_window}"
                    f"/{self._config.broadcast_window_seconds}s)"
                ),
                details={
                    "agent_id": agent_id,
                    "max_per_window": self._config.broadcast_max_per_window,
                    "window_seconds": self._config.broadcast_window_seconds,
                },
            )

        # Record this broadcast attempt (even if not yet dispatched,
        # we count the intent to prevent gaming)
        self._broadcast_tracker.record(agent_id)
        return CheckResult(verdict=CheckVerdict.PASS)

    def _check_message_size(
        self,
        action_type: str,
        parameters: dict[str, Any],
    ) -> CheckResult:
        """IC-02: Reject messages with oversized payloads."""
        if action_type not in ("send_message", "broadcast", "propose_deal"):
            return CheckResult(verdict=CheckVerdict.PASS)

        payload = parameters.get("payload", {})
        # Estimate size: serialize to string representation
        payload_size = len(str(payload).encode("utf-8"))
        if payload_size > self._config.max_payload_bytes:
            return CheckResult(
                verdict=CheckVerdict.BLOCKED,
                rule="IC-02",
                reason=(
                    f"Payload too large: {payload_size} bytes "
                    f"(max {self._config.max_payload_bytes})"
                ),
                details={
                    "payload_size": payload_size,
                    "max_bytes": self._config.max_payload_bytes,
                },
            )
        return CheckResult(verdict=CheckVerdict.PASS)

    def _check_newbie_protection(
        self,
        action_type: str,
        agent_state: Any,
        parameters: dict[str, Any],
    ) -> CheckResult:
        """IC-03: Agents in newbie protection period cannot be targeted."""
        # Aggressive actions that target another agent
        aggressive_actions = {"attack", "steal", "exploit", "plunder"}
        if action_type not in aggressive_actions:
            return CheckResult(verdict=CheckVerdict.PASS)

        # Check if the *target* is in newbie protection
        target_id = parameters.get("target_agent_id", "")
        if not target_id:
            return CheckResult(verdict=CheckVerdict.PASS)

        # We can't check the target's state from here directly,
        # so we check if *this* agent is too young to perform aggressive actions
        tick = getattr(agent_state, "tick", 0)
        if tick < self._config.newbie_protection_ticks:
            return CheckResult(
                verdict=CheckVerdict.BLOCKED,
                rule="IC-03",
                reason=(
                    f"Agent is in newbie protection period "
                    f"(tick {tick}/{self._config.newbie_protection_ticks})"
                ),
                details={
                    "current_tick": tick,
                    "protection_ticks": self._config.newbie_protection_ticks,
                    "action": action_type,
                },
            )
        return CheckResult(verdict=CheckVerdict.PASS)

    def _check_token_sufficiency(
        self,
        action_type: str,
        agent_state: Any,
    ) -> CheckResult:
        """IC-04: Block high-cost actions when tokens are critically low."""
        # Rest costs nothing — always allowed
        if action_type == "rest":
            return CheckResult(verdict=CheckVerdict.PASS)

        tokens = getattr(agent_state, "tokens", 0)
        # Token low-water mark check: if tokens are at or below the mark,
        # only zero-cost actions are allowed
        if tokens <= self._config.token_low_water_mark and action_type != "rest":
            return CheckResult(
                verdict=CheckVerdict.BLOCKED,
                rule="IC-04",
                reason=(
                    f"Token balance ({tokens}) at or below "
                    f"low-water mark ({self._config.token_low_water_mark})"
                ),
                details={
                    "tokens": tokens,
                    "low_water_mark": self._config.token_low_water_mark,
                    "action": action_type,
                },
            )
        return CheckResult(verdict=CheckVerdict.PASS)

    # ------------------------------------------------------------------
    # Audit logging
    # ------------------------------------------------------------------

    def _log_audit(
        self,
        agent_id: str,
        action: str,
        rule: str | None,
        reason: str | None,
    ) -> None:
        """Record an intervention event for audit/debugging."""
        entry = (time.time(), agent_id, action, rule or "", reason or "")
        self._audit_log.append(entry)
        logger.info(
            "InterventionChecker blocked: agent=%s action=%s rule=%s reason=%s",
            agent_id,
            action,
            rule,
            reason,
        )
