"""LLM Mock for E2E testing — context-triggered preset decision responses.

Replaces the ``--no-llm`` random/idle behaviour with deterministic,
context-aware decisions so tests can verify specific Agent behaviour
branches (foraging, socialising, resting, etc.) without a real LLM.

Inspired by LobeChat's ``LLMMockManager`` — register trigger/action
pairs and the mock returns a matching action based on the current
agent context (state + perception + survival).

Usage::

    from tests.e2e.mocks.mock_llm import AgentMockLLM

    mock = AgentMockLLM()
    mock.set_response(
        trigger=ConditionTrigger(lambda ctx: ctx.token_ratio < 0.2),
        action_type=ActionType.GATHER,
        reasoning="Hungry — gathering resources",
    )
    mock.set_response(
        trigger=KeywordTrigger("nearby"),
        action_type=ActionType.SEND_MESSAGE,
        reasoning="Social agent nearby — socialising",
    )
    mock.set_default(ActionType.REST)

    # Wire into ThinkLoop
    loop = ThinkLoop(
        state=state, survival=instinct, executor=executor,
        decision_provider=mock,
    )
"""

from __future__ import annotations

import logging
import os
from dataclasses import dataclass, field
from typing import Any, Callable

from agent_runtime.core.act import ActionType
from agent_runtime.core.think_loop import Decision, Perception
from agent_runtime.models.agent_state import AgentState
from agent_runtime.survival.instinct import SurvivalAction

logger = logging.getLogger(__name__)


# ---------------------------------------------------------------------------
# Trigger types
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class MockContext:
    """Bundle of everything a trigger can inspect.

    Provided to every trigger callable so it can make decisions based
    on agent state, perception data, or survival assessment.
    """

    state: AgentState
    perception: Perception
    survival: SurvivalAction

    # Convenience properties
    @property
    def token_ratio(self) -> float:
        """Agent's token ratio (tokens / max_tokens)."""
        max_t = getattr(self.state, "max_tokens", None)
        if max_t and max_t > 0:
            return self.state.tokens / max_t
        return 0.0

    @property
    def health(self) -> float:
        """Agent's current health."""
        return self.state.health

    @property
    def nearby_agents(self) -> list[Any]:
        """Nearby agents from perception market_state."""
        return self.perception.market_state.get("nearby_agents", [])

    @property
    def has_nearby_agents(self) -> bool:
        """Whether there are nearby agents."""
        return len(self.nearby_agents) > 0

    @property
    def survival_mode(self) -> str:
        """Current survival mode as string."""
        return self.survival.mode.value

    @property
    def tick(self) -> int:
        """Current tick from perception."""
        return self.perception.tick

    @property
    def messages(self) -> list[dict[str, Any]]:
        """Pending messages."""
        return self.perception.messages


class KeywordTrigger:
    """Matches when the serialised context contains **any** of the keywords.

    Serialises the context (state name, messages, market_state, nearby
    agents, events, etc.) to a lower-case string and checks for each
    keyword.

    Args:
        keywords: One or more strings to search for.
        case_sensitive: If False (default), matching is case-insensitive.
    """

    def __init__(
        self,
        *keywords: str,
        case_sensitive: bool = False,
    ) -> None:
        if not keywords:
            raise ValueError("KeywordTrigger requires at least one keyword")
        self._keywords = keywords
        self._case_sensitive = case_sensitive

    def __call__(self, ctx: MockContext) -> bool:
        blob = self._serialize(ctx)
        if not self._case_sensitive:
            blob = blob.lower()
        for kw in self._keywords:
            target = kw if self._case_sensitive else kw.lower()
            if target in blob:
                return True
        return False

    def _serialize(self, ctx: MockContext) -> str:
        """Flatten the context into a searchable string."""
        parts: list[str] = [
            ctx.state.name,
            str(ctx.state.phase),
            ctx.survival_mode,
            ctx.survival.recommendation if hasattr(ctx.survival, "recommendation") else "",
        ]
        # Messages
        for msg in ctx.messages:
            parts.append(str(msg))
        # Market state
        for key, val in ctx.perception.market_state.items():
            parts.append(f"{key}={val}")
        # Nearby agents
        for agent_info in ctx.nearby_agents:
            parts.append(str(agent_info))
        return " ".join(parts)

    def __repr__(self) -> str:
        return f"KeywordTrigger({', '.join(repr(k) for k in self._keywords)})"


class ConditionTrigger:
    """Matches when a callable predicate returns True.

    The callable receives a :class:`MockContext` and must return a bool.

    Example::

        # Token ratio < 20% (starving)
        ConditionTrigger(lambda ctx: ctx.token_ratio < 0.2)

        # Health below 30
        ConditionTrigger(lambda ctx: ctx.health < 30)

        # Nearby agents present
        ConditionTrigger(lambda ctx: ctx.has_nearby_agents)
    """

    def __init__(self, predicate: Callable[[MockContext], bool]) -> None:
        self._predicate = predicate

    def __call__(self, ctx: MockContext) -> bool:
        return self._predicate(ctx)

    def __repr__(self) -> str:
        return f"ConditionTrigger({self._predicate!r})"


# Type alias — any callable that accepts a MockContext and returns bool
TriggerFn = Callable[[MockContext], bool]


# ---------------------------------------------------------------------------
# Registered response entry
# ---------------------------------------------------------------------------


@dataclass
class _RegisteredResponse:
    """Internal bookkeeping for a trigger → action mapping."""

    trigger: TriggerFn
    action_type: ActionType
    reasoning: str = ""
    parameters: dict[str, Any] = field(default_factory=dict)
    priority: int = 0  # higher = checked first


# ---------------------------------------------------------------------------
# AgentMockLLM — the main mock decision provider
# ---------------------------------------------------------------------------


class AgentMockLLM:
    """Mock decision provider with context-triggered preset responses.

    Implements the ``DecisionProvider`` protocol expected by
    :class:`ThinkLoop`.  Instead of calling a real LLM, it matches
    the current agent context against registered triggers and returns
    the first matching preset action.

    Usage::

        mock = AgentMockLLM()
        # High-priority: starving → gather
        mock.set_response(
            trigger=ConditionTrigger(lambda ctx: ctx.token_ratio < 0.2),
            action_type=ActionType.GATHER,
            reasoning="Starving — gathering resources",
            priority=10,
        )
        # Low-priority: nearby agents → socialise
        mock.set_response(
            trigger=ConditionTrigger(lambda ctx: ctx.has_nearby_agents),
            action_type=ActionType.SEND_MESSAGE,
            reasoning="Social agent nearby — saying hello",
            priority=5,
        )
        # Default fallback
        mock.set_default(ActionType.REST)

    The mock also tracks call history for test assertions::

        # After running the think loop
        assert mock.call_count >= 1
        last = mock.last_decision
        assert last.action_type == ActionType.GATHER
    """

    def __init__(self, *, default_action: ActionType = ActionType.REST) -> None:
        self._responses: list[_RegisteredResponse] = []
        self._default_action = default_action
        self._default_reasoning = "Mock default: no trigger matched"

        # Call history for assertions
        self.call_count: int = 0
        self.last_decision: Decision | None = None
        self.decision_history: list[Decision] = []

    # ------------------------------------------------------------------
    # Registration API
    # ------------------------------------------------------------------

    def set_response(
        self,
        *,
        trigger: TriggerFn,
        action_type: ActionType,
        reasoning: str = "",
        parameters: dict[str, Any] | None = None,
        priority: int = 0,
    ) -> None:
        """Register a trigger → action mapping.

        Args:
            trigger: A callable (``KeywordTrigger``, ``ConditionTrigger``,
                or custom lambda) that returns True when this response
                should fire.
            action_type: The ``ActionType`` to return when the trigger matches.
            reasoning: Explanation string attached to the decision.
            parameters: Optional action parameters.
            priority: Higher-priority triggers are checked first.
                Ties are broken by registration order (first registered wins).
        """
        entry = _RegisteredResponse(
            trigger=trigger,
            action_type=action_type,
            reasoning=reasoning,
            parameters=parameters or {},
            priority=priority,
        )
        self._responses.append(entry)
        # Keep sorted: highest priority first, stable within same priority
        self._responses.sort(key=lambda r: r.priority, reverse=True)
        logger.debug(
            "Registered mock response: trigger=%r → action=%s priority=%d",
            trigger,
            action_type.value,
            priority,
        )

    def set_default(
        self,
        action_type: ActionType,
        reasoning: str = "Mock default: no trigger matched",
    ) -> None:
        """Set the default action when no trigger matches."""
        self._default_action = action_type
        self._default_reasoning = reasoning

    # ------------------------------------------------------------------
    # DecisionProvider protocol
    # ------------------------------------------------------------------

    async def decide(
        self,
        state: AgentState,
        perception: Perception,
        survival: SurvivalAction,
    ) -> Decision:
        """Produce a decision by matching context against registered triggers.

        Checks triggers in priority order (highest first).  Returns the
        first matching action.  Falls back to the default if no trigger
        matches.
        """
        self.call_count += 1
        ctx = MockContext(
            state=state,
            perception=perception,
            survival=survival,
        )

        for entry in self._responses:
            try:
                if entry.trigger(ctx):
                    decision = Decision(
                        action_type=entry.action_type,
                        parameters=entry.parameters,
                        reasoning=entry.reasoning,
                    )
                    self.last_decision = decision
                    self.decision_history.append(decision)
                    logger.debug(
                        "Mock matched trigger → %s (reason: %s)",
                        entry.action_type.value,
                        entry.reasoning,
                    )
                    return decision
            except Exception:
                # Trigger errors should not crash the think loop
                logger.warning(
                    "Mock trigger %r raised an error — skipping",
                    entry.trigger,
                    exc_info=True,
                )

        # No trigger matched — return default
        decision = Decision(
            action_type=self._default_action,
            reasoning=self._default_reasoning,
        )
        self.last_decision = decision
        self.decision_history.append(decision)
        logger.debug("Mock no trigger matched → default %s", self._default_action.value)
        return decision

    # ------------------------------------------------------------------
    # Test helpers
    # ------------------------------------------------------------------

    def reset_history(self) -> None:
        """Clear all call history (useful between test phases)."""
        self.call_count = 0
        self.last_decision = None
        self.decision_history.clear()

    @property
    def registered_count(self) -> int:
        """Number of registered trigger/response pairs."""
        return len(self._responses)


# ---------------------------------------------------------------------------
# Factory helpers for common presets
# ---------------------------------------------------------------------------


def hungry_gather_mock() -> AgentMockLLM:
    """Pre-built mock: starving agents gather, others rest.

    Trigger priority:
      1. token_ratio < 0.2 → GATHER ("Starving — gathering resources")
      default → REST
    """
    mock = AgentMockLLM()
    mock.set_response(
        trigger=ConditionTrigger(lambda ctx: ctx.token_ratio < 0.2),
        action_type=ActionType.GATHER,
        reasoning="Starving — gathering resources",
        priority=10,
    )
    return mock


def social_nearby_mock() -> AgentMockLLM:
    """Pre-built mock: agents with neighbours socialise, others explore.

    Trigger priority:
      1. has_nearby_agents → SEND_MESSAGE ("Social agent nearby — socialising")
      default → EXPLORE
    """
    mock = AgentMockLLM(default_action=ActionType.EXPLORE)
    mock.set_response(
        trigger=ConditionTrigger(lambda ctx: ctx.has_nearby_agents),
        action_type=ActionType.SEND_MESSAGE,
        reasoning="Social agent nearby — socialising",
        priority=10,
    )
    return mock


def survival_behaviour_mock() -> AgentMockLLM:
    """Pre-built mock: full survival-driven behaviour branches.

    Trigger priority (highest first):
      1. panic/urgent survival → GATHER ("Emergency — gathering")
      2. token_ratio < 0.4    → GATHER ("Conservative — gathering")
      3. has_nearby_agents     → SEND_MESSAGE ("Social — greeting")
      4. Keyword "task"        → CLAIM_TASK ("Task available — claiming")
      default → REST
    """
    mock = AgentMockLLM()

    # 1. Emergency survival
    mock.set_response(
        trigger=ConditionTrigger(
            lambda ctx: ctx.survival_mode in ("panic", "urgent")
        ),
        action_type=ActionType.GATHER,
        reasoning="Emergency — gathering resources",
        priority=30,
    )

    # 2. Conservative gathering
    mock.set_response(
        trigger=ConditionTrigger(lambda ctx: ctx.token_ratio < 0.4),
        action_type=ActionType.GATHER,
        reasoning="Conservative — gathering resources",
        priority=20,
    )

    # 3. Social behaviour
    mock.set_response(
        trigger=ConditionTrigger(lambda ctx: ctx.has_nearby_agents),
        action_type=ActionType.SEND_MESSAGE,
        reasoning="Social — greeting nearby agent",
        priority=10,
    )

    # 4. Task claiming
    mock.set_response(
        trigger=KeywordTrigger("task"),
        action_type=ActionType.CLAIM_TASK,
        reasoning="Task available — claiming",
        priority=5,
    )

    return mock


# ---------------------------------------------------------------------------
# CLI / env-var integration helper
# ---------------------------------------------------------------------------

def create_mock_from_env() -> AgentMockLLM | None:
    """Build an ``AgentMockLLM`` from ``MOCK_LLM_PRESET`` env var.

    Recognised values:
      - ``hungry_gather`` → :func:`hungry_gather_mock`
      - ``social_nearby`` → :func:`social_nearby_mock`
      - ``survival``      → :func:`survival_behaviour_mock`
      - ``true`` / ``1``  → :func:`survival_behaviour_mock` (default preset)

    Returns None if the env var is not set.
    """
    preset = os.environ.get("MOCK_LLM_PRESET", "").strip().lower()
    if not preset:
        return None

    factories = {
        "hungry_gather": hungry_gather_mock,
        "social_nearby": social_nearby_mock,
        "survival": survival_behaviour_mock,
    }

    factory = factories.get(preset)
    if factory is None:
        if preset in ("true", "1"):
            factory = survival_behaviour_mock
        else:
            logger.warning(
                "Unknown MOCK_LLM_PRESET=%r, using 'survival' default", preset
            )
            factory = survival_behaviour_mock

    return factory()
