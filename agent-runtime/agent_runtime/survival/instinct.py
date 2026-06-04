"""Survival instinct — LLM-bypassing bottom-layer survival logic.

Evaluates agent token balance and immediately drives behaviour without
waiting for an LLM decision cycle.  The think-loop calls
``SurvivalInstinct.assess()`` *before* the LLM decide step; if the
returned ``SurvivalAction`` is PANIC or URGENT the normal LLM path is
skipped entirely and the emergency actions are executed directly.

Thresholds (from DESIGN.md s4.4 + issue spec):
    PANIC        < 10 %   — broadcast SOS, request loan, cancel tasks
    URGENT       < 20 %   — seek cheapest income, reject costly tasks
    CONSERVATIVE < 40 %   — accept only profitable tasks, limit spending
    NORMAL     40 - 80 %  — regular LLM-driven behaviour
    INVEST       > 80 %   — invest surplus, teach, share knowledge
"""

from __future__ import annotations

import asyncio
import logging
import time
from dataclasses import dataclass, field
from enum import Enum
from typing import Protocol

logger = logging.getLogger(__name__)


# ---------------------------------------------------------------------------
# Enums & data types
# ---------------------------------------------------------------------------


class SurvivalMode(Enum):
    """Five survival modes ordered by urgency (lowest value = most urgent)."""

    PANIC = "panic"  # Token < 10 %
    URGENT = "urgent"  # Token < 20 %
    CONSERVATIVE = "conservative"  # Token < 40 %
    NORMAL = "normal"  # Token 40-80 %
    INVEST = "invest"  # Token > 80 %


class EmergencyActionType(Enum):
    """Concrete actions the survival instinct can emit.

    These are executed *without* going through the LLM.
    """

    BROADCAST_SOS = "broadcast_sos"  # Broadcast a distress signal
    REQUEST_LOAN = "request_loan"  # Request a loan from peers/bank
    CANCEL_ALL_TASKS = "cancel_all_tasks"  # Cancel in-progress tasks
    SEEK_CHEAPEST_INCOME = "seek_cheapest_income"  # Accept any cheap task
    REJECT_COSTLY_TASKS = "reject_costly_tasks"  # Refuse high-cost tasks
    EXCHANGE_MONEY_TO_TOKENS = "exchange_money_to_tokens"  # Convert Money → Token
    LIMIT_SPENDING = "limit_spending"  # Reduce non-essential spending
    REST_TO_CONSERVE = "rest_to_conserve"  # Skip tick to save tokens
    INVEST_SURPLUS = "invest_surplus"  # Invest excess tokens
    TEACH_FOR_INCOME = "teach_for_income"  # Offer to teach for money
    SHARE_KNOWLEDGE = "share_knowledge"  # Publish knowledge for profit


@dataclass(frozen=True)
class EmergencyAction:
    """A single emergency action produced by the survival instinct."""

    action_type: EmergencyActionType
    priority: int  # lower = higher priority
    reason: str
    parameters: dict[str, object] = field(default_factory=dict)


@dataclass(frozen=True)
class SurvivalAction:
    """Result returned by ``SurvivalInstinct.assess()``."""

    mode: SurvivalMode
    token_ratio: float
    actions: list[EmergencyAction] = field(default_factory=list)


@dataclass(frozen=True)
class SurvivalThresholds:
    """Configurable token-ratio thresholds for each survival mode.

    Ratios are expressed as fractions of ``max_tokens`` (0.0 - 1.0).

    Raises:
        ValueError: If thresholds are not in strictly increasing order
            (panic < urgent < conservative < invest).
    """

    panic: float = 0.10  # < 10 %
    urgent: float = 0.20  # < 20 %
    conservative: float = 0.40  # < 40 %
    invest: float = 0.80  # > 80 %

    def __post_init__(self) -> None:
        if not (0.0 <= self.panic < self.urgent < self.conservative < self.invest <= 1.0):
            raise ValueError(
                f"Thresholds must satisfy "
                f"0.0 <= panic({self.panic}) < urgent({self.urgent}) "
                f"< conservative({self.conservative}) < invest({self.invest}) <= 1.0"
            )


@dataclass(frozen=True)
class LoanTerms:
    """Configurable terms for emergency loan requests."""

    interest_offered: float = 0.02
    repayment_ticks: int = 500


# ---------------------------------------------------------------------------
# Agent state protocol
# ---------------------------------------------------------------------------


class AgentStateProtocol(Protocol):
    """Minimal interface the survival instinct needs from agent state.

    Using a Protocol keeps the module decoupled from the concrete
    ``AgentState`` dataclass which may evolve independently.
    """

    @property
    def tokens(self) -> int: ...

    @property
    def max_tokens(self) -> int: ...

    @property
    def money(self) -> int: ...

    @property
    def current_task(self) -> str | None: ...


# ---------------------------------------------------------------------------
# A2A actions that require network communication
# ---------------------------------------------------------------------------

# Actions that have real A2A side-effects.
_A2A_ACTIONS: frozenset[EmergencyActionType] = frozenset(
    {
        EmergencyActionType.BROADCAST_SOS,
        EmergencyActionType.REQUEST_LOAN,
    }
)


# ---------------------------------------------------------------------------
# SurvivalInstinct
# ---------------------------------------------------------------------------

# Minimum interval (seconds) between identical emergency actions to avoid
# flooding the network with repeated SOS / loan requests.
_ACTION_COOLDOWN: float = 5.0


class SurvivalInstinct:
    """LLM-bypassing survival logic.

    Usage::

        instinct = SurvivalInstinct()
        action = instinct.assess(agent_state)
        if action.mode in (SurvivalMode.PANIC, SurvivalMode.URGENT):
            await instinct.execute(action, agent, a2a_client)
            return  # skip normal LLM decision
    """

    def __init__(
        self,
        thresholds: SurvivalThresholds | None = None,
        *,
        action_cooldown: float = _ACTION_COOLDOWN,
        loan_terms: LoanTerms | None = None,
    ) -> None:
        self.thresholds = thresholds or SurvivalThresholds()
        self._last_action_time: dict[EmergencyActionType, float | None] = {}
        self._action_cooldown = action_cooldown
        self.loan_terms = loan_terms or LoanTerms()
        self._lock: asyncio.Lock | None = None

    def _get_lock(self) -> asyncio.Lock:
        """Lazily create the asyncio.Lock on first use.

        Creating an ``asyncio.Lock`` in ``__init__`` binds it to the
        current event loop.  If no loop is running (e.g. during import or
        in a synchronous test), this raises on Python 3.9.  Deferring
        creation to first use avoids the issue entirely.
        """
        if self._lock is None:
            self._lock = asyncio.Lock()
        return self._lock

    # ------------------------------------------------------------------
    # Core assessment (synchronous — no LLM, no I/O)
    # ------------------------------------------------------------------

    def assess(self, agent: AgentStateProtocol) -> SurvivalAction:
        """Evaluate the agent's survival status and return actions.

        This method is **pure** — it performs no I/O and makes no LLM
        calls.  It reads the agent's token balance and computes the
        appropriate survival mode and a list of emergency actions.

        The think-loop should call this *before* the LLM decide step.
        If the returned mode is PANIC or URGENT, the loop should skip
        the LLM and call ``execute()`` directly.
        """
        if agent.max_tokens <= 0:
            # Degenerate case: avoid division by zero.
            ratio = 0.0
        else:
            ratio = agent.tokens / agent.max_tokens

        if ratio > 1.0:
            logger.warning(
                "Token ratio exceeds 1.0 (%.2f): tokens=%d max_tokens=%d. Clamping to 1.0.",
                ratio,
                agent.tokens,
                agent.max_tokens,
            )
            ratio = 1.0
        elif ratio < 0.0:
            logger.warning(
                "Negative token ratio (%.2f): tokens=%d max_tokens=%d. Clamping to 0.0.",
                ratio,
                agent.tokens,
                agent.max_tokens,
            )
            ratio = 0.0

        mode = self._classify_mode(ratio)
        actions = self._generate_actions(mode, ratio, agent)

        logger.debug(
            "Survival assessment: mode=%s ratio=%.2f actions=%d",
            mode.value,
            ratio,
            len(actions),
        )

        return SurvivalAction(mode=mode, token_ratio=ratio, actions=actions)

    # ------------------------------------------------------------------
    # Emergency action execution (still bypasses LLM)
    # ------------------------------------------------------------------

    async def execute(
        self,
        action: SurvivalAction,
        agent: AgentStateProtocol,
        a2a_client: A2AClientProtocol | None = None,
    ) -> list[dict[str, object]]:
        """Execute emergency actions directly (no LLM involved).

        Returns a list of result dicts, one per executed action, so the
        think-loop can log outcomes.

        This method is serialised with an ``asyncio.Lock`` so that
        concurrent calls do not bypass the cooldown check.
        """
        async with self._get_lock():
            results: list[dict[str, object]] = []
            now = time.monotonic()

            for ema in action.actions:
                # Enforce cooldown to prevent flooding.
                last = self._last_action_time.get(ema.action_type)
                if last is not None and now - last < self._action_cooldown:
                    logger.debug("Skipping %s (cooldown)", ema.action_type.value)
                    continue

                result = await self._execute_single(ema, agent, a2a_client)
                self._last_action_time[ema.action_type] = now
                results.append(result)

            return results

    # ------------------------------------------------------------------
    # Internal helpers
    # ------------------------------------------------------------------

    def _classify_mode(self, ratio: float) -> SurvivalMode:
        """Map a token ratio to a survival mode."""
        t = self.thresholds
        if ratio < t.panic:
            return SurvivalMode.PANIC
        if ratio < t.urgent:
            return SurvivalMode.URGENT
        if ratio < t.conservative:
            return SurvivalMode.CONSERVATIVE
        if ratio > t.invest:
            return SurvivalMode.INVEST
        return SurvivalMode.NORMAL

    def _generate_actions(
        self,
        mode: SurvivalMode,
        ratio: float,
        agent: AgentStateProtocol,
    ) -> list[EmergencyAction]:
        """Produce emergency actions for the given mode."""
        actions: list[EmergencyAction] = []

        if mode == SurvivalMode.PANIC:
            # Token < 10 % — maximum urgency.
            actions.append(
                EmergencyAction(
                    action_type=EmergencyActionType.BROADCAST_SOS,
                    priority=0,
                    reason=f"Token ratio critical: {ratio:.1%}. Broadcasting SOS.",
                    parameters={"token_ratio": ratio},
                )
            )
            actions.append(
                EmergencyAction(
                    action_type=EmergencyActionType.REQUEST_LOAN,
                    priority=1,
                    reason=f"Token ratio critical: {ratio:.1%}. Requesting loan.",
                    parameters={"amount_needed": int(agent.max_tokens * self.thresholds.urgent)},
                )
            )
            if agent.current_task is not None:
                actions.append(
                    EmergencyAction(
                        action_type=EmergencyActionType.CANCEL_ALL_TASKS,
                        priority=2,
                        reason="Cancelling tasks to conserve tokens in PANIC mode.",
                    )
                )
            if agent.money > 0:
                actions.append(
                    EmergencyAction(
                        action_type=EmergencyActionType.EXCHANGE_MONEY_TO_TOKENS,
                        priority=1,
                        reason=f"Exchanging {agent.money} Money for tokens in PANIC mode.",
                        parameters={"money_amount": agent.money},
                    )
                )
            actions.append(
                EmergencyAction(
                    action_type=EmergencyActionType.REST_TO_CONSERVE,
                    priority=3,
                    reason="Resting to minimise token consumption.",
                )
            )

        elif mode == SurvivalMode.URGENT:
            # Token < 20 % — seek income urgently.
            actions.append(
                EmergencyAction(
                    action_type=EmergencyActionType.SEEK_CHEAPEST_INCOME,
                    priority=0,
                    reason=f"Token ratio urgent: {ratio:.1%}. Seeking cheapest income.",
                )
            )
            actions.append(
                EmergencyAction(
                    action_type=EmergencyActionType.REJECT_COSTLY_TASKS,
                    priority=1,
                    reason="Rejecting costly tasks to preserve remaining tokens.",
                )
            )
            if agent.money > 0:
                actions.append(
                    EmergencyAction(
                        action_type=EmergencyActionType.EXCHANGE_MONEY_TO_TOKENS,
                        priority=0,
                        reason=f"Exchanging {agent.money} Money for tokens in URGENT mode.",
                        parameters={"money_amount": agent.money},
                    )
                )
            actions.append(
                EmergencyAction(
                    action_type=EmergencyActionType.REQUEST_LOAN,
                    priority=2,
                    reason=f"Token ratio low: {ratio:.1%}. Requesting loan.",
                    parameters={
                        "amount_needed": int(agent.max_tokens * self.thresholds.conservative)
                    },
                )
            )

        elif mode == SurvivalMode.CONSERVATIVE:
            # Token < 40 % — limit spending.
            actions.append(
                EmergencyAction(
                    action_type=EmergencyActionType.LIMIT_SPENDING,
                    priority=0,
                    reason=f"Token ratio conservative: {ratio:.1%}. Limiting spending.",
                )
            )
            actions.append(
                EmergencyAction(
                    action_type=EmergencyActionType.REJECT_COSTLY_TASKS,
                    priority=1,
                    reason="Only accepting profitable tasks in CONSERVATIVE mode.",
                )
            )

        elif mode == SurvivalMode.INVEST:
            # Token > 80 % — invest surplus.
            actions.append(
                EmergencyAction(
                    action_type=EmergencyActionType.INVEST_SURPLUS,
                    priority=0,
                    reason=f"Token ratio high: {ratio:.1%}. Investing surplus.",
                )
            )
            actions.append(
                EmergencyAction(
                    action_type=EmergencyActionType.TEACH_FOR_INCOME,
                    priority=1,
                    reason="Surplus tokens — offering to teach for additional income.",
                )
            )
            actions.append(
                EmergencyAction(
                    action_type=EmergencyActionType.SHARE_KNOWLEDGE,
                    priority=2,
                    reason="Surplus tokens — publishing knowledge for profit.",
                )
            )

        # NORMAL mode: no emergency actions — let the LLM decide.

        return actions

    async def _execute_single(
        self,
        ema: EmergencyAction,
        agent: AgentStateProtocol,
        a2a_client: A2AClientProtocol | None,
    ) -> dict[str, object]:
        """Execute one emergency action.

        For now this logs the action and, if an A2A client is available,
        sends appropriate messages.  The action is performed *without*
        consulting the LLM.

        Non-A2A actions are logged but have no side-effect yet; their
        status is set to ``"logged"`` rather than ``"executed"``.
        """
        logger.info(
            "Survival action: %s — %s",
            ema.action_type.value,
            ema.reason,
        )

        # Determine status based on whether this is a real A2A action.
        status = "logged" if ema.action_type not in _A2A_ACTIONS else "executed"
        result: dict[str, object] = {
            "action": ema.action_type.value,
            "reason": ema.reason,
            "status": status,
        }

        # Actions that require A2A communication.
        if a2a_client is not None:
            try:
                if ema.action_type == EmergencyActionType.BROADCAST_SOS:
                    token_ratio = ema.parameters.get("token_ratio", 0)
                    # Safely format the ratio — handle non-float values.
                    try:
                        ratio_str = f"{float(token_ratio):.1%}"
                    except (TypeError, ValueError):
                        ratio_str = "unknown"

                    msg_result = await a2a_client.broadcast_message(
                        {
                            "type": "INFORM",
                            "payload": {
                                "category": "personal",
                                "content": (
                                    f"[SOS] I am critically low on tokens "
                                    f"({ratio_str}). Please help!"
                                ),
                                "confidence": 1.0,
                                "source": "direct",
                            },
                        }
                    )
                    result["broadcast_result"] = msg_result

                elif ema.action_type == EmergencyActionType.REQUEST_LOAN:
                    amount_needed = ema.parameters.get("amount_needed", 0)
                    msg_result = await a2a_client.broadcast_message(
                        {
                            "type": "PROPOSE",
                            "payload": {
                                "action": "loan_request",
                                "terms": {
                                    "amount_needed": amount_needed,
                                    "interest_offered": self.loan_terms.interest_offered,
                                    "repayment_ticks": self.loan_terms.repayment_ticks,
                                },
                            },
                        }
                    )
                    result["loan_request_result"] = msg_result

            except Exception:
                logger.exception(
                    "Failed to execute A2A action %s",
                    ema.action_type.value,
                )
                result["status"] = "failed"

        return result

    # ------------------------------------------------------------------
    # Reset (useful for testing)
    # ------------------------------------------------------------------

    def reset_cooldowns(self) -> None:
        """Clear all action cooldown timers."""
        self._last_action_time.clear()


# ---------------------------------------------------------------------------
# A2A client protocol (for dependency injection)
# ---------------------------------------------------------------------------


class A2AClientProtocol(Protocol):
    """Minimal interface for sending messages.

    In production this will be the real ``A2AClient``; in tests it can
    be replaced with a mock.
    """

    async def broadcast_message(self, payload: dict[str, object]) -> dict[str, object]: ...
