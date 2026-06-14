"""Action layer — translates decisions into concrete actions.

The ``ActionExecutor`` is the "Act" step in the Perceive → Decide → Act loop.
It validates that the agent can afford each action (token cost check), executes
it, records the result, and handles failures with configurable retry logic.

Supported action types:
    - ``send_message``     — Send an A2A message to another agent
    - ``claim_task``       — Claim an available task from the market
    - ``submit_task``      — Submit completed work for a claimed task
    - ``propose_deal``     — Propose a deal/contract to another agent
    - ``teach_skill``      — Teach a skill to another agent (costs tokens)
    - ``practice_skill``   — Practice a skill by yourself (no target needed)
    - ``rest``             — Skip the tick to conserve tokens (no cost)
    - ``explore``          — Explore the world for opportunities

Usage::

    executor = ActionExecutor()
    result = await executor.execute(action, context)
    if result.status == ActionStatus.SUCCESS:
        ...
"""

from __future__ import annotations

import logging
import time
from dataclasses import dataclass, field
from enum import Enum
from typing import Any, Protocol

from agent_runtime.core.intervention_checker import (
    InterventionChecker,
    InterventionConfig,
)

logger = logging.getLogger(__name__)


# ---------------------------------------------------------------------------
# Enums & data types
# ---------------------------------------------------------------------------


class ActionType(str, Enum):
    """All supported action types."""

    SEND_MESSAGE = "send_message"
    CLAIM_TASK = "claim_task"
    SUBMIT_TASK = "submit_task"
    PROPOSE_DEAL = "propose_deal"
    TEACH_SKILL = "teach_skill"
    PRACTICE_SKILL = "practice_skill"
    REST = "rest"
    EXPLORE = "explore"
    MOVE = "move"
    GATHER = "gather"
    BUILD = "build"
    SOCIALIZE = "socialize"
    FORM_ORG = "form_org"
    JOIN_ORG = "join_org"
    PROPOSE_RULE = "propose_rule"
    VOTE_RULE = "vote_rule"
    RESPOND_ORACLE = "respond_oracle"
    CHECK_BOUNTIES = "check_bounties"
    ACCEPT_BOUNTY = "accept_bounty"
    COMPLETE_BOUNTY = "complete_bounty"


class ActionStatus(str, Enum):
    """Outcome status of an action execution."""

    SUCCESS = "success"
    FAILED = "failed"
    INSUFFICIENT_TOKENS = "insufficient_tokens"
    SKIPPED = "skipped"
    RETRY_EXHAUSTED = "retry_exhausted"
    BLOCKED_BY_INTERVENTION = "blocked_by_intervention"


@dataclass(frozen=True)
class ActionResult:
    """Record of a single action execution attempt.

    Attributes:
        action_type: The type of action that was executed.
        status: The outcome status.
        token_cost: The number of tokens consumed (0 if not executed).
        data: Arbitrary result payload from the action handler.
        error: Error message if the action failed.
        attempts: Number of attempts made (including retries).
        elapsed_ms: Wall-clock time spent on the action in milliseconds.
        timestamp: Monotonic timestamp when the action started.
    """

    action_type: ActionType
    status: ActionStatus
    token_cost: int = 0
    data: dict[str, Any] = field(default_factory=dict)
    error: str | None = None
    attempts: int = 1
    elapsed_ms: float = 0.0
    timestamp: float = 0.0


# ---------------------------------------------------------------------------
# Token cost table
# ---------------------------------------------------------------------------

# Costs are aligned with genesis.yaml:
#   think_cost_per_token: 1, communicate_cost: 10
_DEFAULT_TOKEN_COSTS: dict[ActionType, int] = {
    ActionType.SEND_MESSAGE: 10,
    ActionType.CLAIM_TASK: 5,
    ActionType.SUBMIT_TASK: 8,
    ActionType.PROPOSE_DEAL: 10,
    ActionType.TEACH_SKILL: 15,
    ActionType.PRACTICE_SKILL: 8,
    ActionType.REST: 0,
    ActionType.EXPLORE: 3,
    ActionType.MOVE: 12,
    ActionType.GATHER: 8,
    ActionType.BUILD: 20,
    ActionType.SOCIALIZE: 5,
    ActionType.FORM_ORG: 25,
    ActionType.JOIN_ORG: 10,
    ActionType.PROPOSE_RULE: 15,
    ActionType.VOTE_RULE: 5,
    ActionType.RESPOND_ORACLE: 3,
    ActionType.CHECK_BOUNTIES: 2,
    ActionType.ACCEPT_BOUNTY: 10,
    ActionType.COMPLETE_BOUNTY: 8,
}


# ---------------------------------------------------------------------------
# Protocols (dependency injection)
# ---------------------------------------------------------------------------


class AgentStateProtocol(Protocol):
    """Minimal interface ActionExecutor needs from agent state."""

    @property
    def tokens(self) -> int: ...

    def adjust_tokens(self, delta: int) -> None: ...


class WorldClientProtocol(Protocol):
    """Interface for interacting with the world (A2A, market, etc.)."""

    async def send_message(self, payload: dict[str, Any]) -> dict[str, Any]: ...

    async def claim_task(self, task_id: str) -> dict[str, Any]: ...

    async def submit_task(self, task_id: str, result: dict[str, Any]) -> dict[str, Any]: ...

    async def propose_deal(self, proposal: dict[str, Any]) -> dict[str, Any]: ...

    async def teach_skill(
        self, target_agent_id: str, skill_name: str, level: int
    ) -> dict[str, Any]: ...

    async def explore(self, parameters: dict[str, Any]) -> dict[str, Any]: ...

    async def practice_skill(self, skill_name: str) -> dict[str, Any]: ...

    async def move(self, direction: str) -> dict[str, Any]: ...

    async def gather(self, resource_type: str) -> dict[str, Any]: ...

    async def build(self, structure_type: str, **kwargs: Any) -> dict[str, Any]: ...

    async def socialize(self, target_agent_id: str, message: str = "") -> dict[str, Any]: ...

    async def respond_to_oracle(self, oracle_id: str, response: str) -> dict[str, Any]: ...

    async def check_bounties(self) -> dict[str, Any]: ...

    async def claim_bounty(self, bounty_id: str) -> dict[str, Any]: ...

    async def complete_bounty(self, bounty_id: str, result: str) -> dict[str, Any]: ...


@dataclass
class ActionContext:
    """Bundles everything an action handler needs.

    Attributes:
        agent: The agent's state (for token checks and deduction).
        world: Client for interacting with the outside world.
        parameters: Action-specific parameters.
    """

    agent: AgentStateProtocol
    world: WorldClientProtocol
    parameters: dict[str, Any] = field(default_factory=dict)


# ---------------------------------------------------------------------------
# ActionExecutor
# ---------------------------------------------------------------------------

# Default retry settings
_DEFAULT_MAX_RETRIES: int = 3
_DEFAULT_RETRY_DELAY_S: float = 0.1


class ActionExecutor:
    """Translates decisions into concrete actions with token cost management.

    For each action the executor:
      1. Checks if the agent has enough tokens (pre-flight check).
      2. Deducts the token cost.
      3. Executes the action via the world client.
      4. Records the result.
      5. On failure, retries up to ``max_retries`` times (with the token
         cost deducted only once for the whole sequence).

    Usage::

        executor = ActionExecutor()
        ctx = ActionContext(agent=agent_state, world=world_client, parameters={...})
        result = await executor.execute(ActionType.SEND_MESSAGE, ctx)
    """

    def __init__(
        self,
        *,
        token_costs: dict[ActionType, int] | None = None,
        max_retries: int = _DEFAULT_MAX_RETRIES,
        retry_delay: float = _DEFAULT_RETRY_DELAY_S,
        intervention_config: InterventionConfig | None = None,
    ) -> None:
        self._token_costs = {**_DEFAULT_TOKEN_COSTS, **(token_costs or {})}
        self._max_retries = max_retries
        self._retry_delay = retry_delay
        self._history: list[ActionResult] = []
        self._intervention = InterventionChecker(intervention_config)

    # ------------------------------------------------------------------
    # Public API
    # ------------------------------------------------------------------

    def get_cost(self, action_type: ActionType) -> int:
        """Return the token cost for a given action type."""
        return self._token_costs.get(action_type, 0)

    def can_afford(self, action_type: ActionType, agent: AgentStateProtocol) -> bool:
        """Check if the agent can afford an action without executing it."""
        return agent.tokens >= self.get_cost(action_type)

    async def execute(self, action_type: ActionType, context: ActionContext) -> ActionResult:
        """Execute an action with token checking, intervention check, retry, and result recording.

        Returns an :class:`ActionResult` describing the outcome.
        """
        cost = self.get_cost(action_type)
        start_time = time.monotonic()
        start_ts = time.time()

        # 0. InterventionChecker — pre-dispatch safety gate
        intervention_result = self._intervention.check(
            action_type=action_type.value,
            agent_state=context.agent,
            parameters=context.parameters,
        )
        if intervention_result.blocked:
            result = ActionResult(
                action_type=action_type,
                status=ActionStatus.BLOCKED_BY_INTERVENTION,
                token_cost=0,
                error=(
                    f"[{intervention_result.rule}] "
                    f"{intervention_result.reason}"
                ),
                data={
                    "intervention_rule": intervention_result.rule,
                    "intervention_details": intervention_result.details,
                },
                timestamp=start_ts,
            )
            self._record(result)
            return result

        # 1. Pre-flight token check
        if not self.can_afford(action_type, context.agent):
            result = ActionResult(
                action_type=action_type,
                status=ActionStatus.INSUFFICIENT_TOKENS,
                token_cost=0,
                error=f"Need {cost} tokens, have {context.agent.tokens}",
                timestamp=start_ts,
            )
            self._record(result)
            return result

        # 2. Deduct tokens once for the entire attempt sequence
        context.agent.adjust_tokens(-cost)

        # 3. Execute with retry
        last_error: str | None = None
        attempts = 0
        result: ActionResult | None = None

        for attempt in range(1, self._max_retries + 1):
            attempts = attempt
            try:
                data = await self._dispatch(action_type, context)
                # Guard against world clients that return {"status": "error", ...}
                # instead of raising.  Treat it as a dispatch failure so the
                # retry logic kicks in and the final result is RETRY_EXHAUSTED
                # rather than a false SUCCESS.
                if isinstance(data, dict) and data.get("status") == "error":
                    raise RuntimeError(
                        data.get("error", "world client returned error status")
                    )
                elapsed = (time.monotonic() - start_time) * 1000
                result = ActionResult(
                    action_type=action_type,
                    status=ActionStatus.SUCCESS,
                    token_cost=cost,
                    data=data,
                    attempts=attempts,
                    elapsed_ms=elapsed,
                    timestamp=start_ts,
                )
                self._record(result)
                return result
            except Exception as exc:
                last_error = str(exc)
                logger.warning(
                    "Action %s attempt %d/%d failed: %s",
                    action_type.value,
                    attempt,
                    self._max_retries,
                    last_error,
                )
                if attempt < self._max_retries:
                    import asyncio

                    await asyncio.sleep(self._retry_delay)

        # All retries exhausted
        elapsed = (time.monotonic() - start_time) * 1000
        result = ActionResult(
            action_type=action_type,
            status=ActionStatus.RETRY_EXHAUSTED,
            token_cost=cost,
            error=last_error,
            attempts=attempts,
            elapsed_ms=elapsed,
            timestamp=start_ts,
        )
        self._record(result)
        return result

    @property
    def history(self) -> list[ActionResult]:
        """Read-only access to the action execution history."""
        return list(self._history)

    def clear_history(self) -> None:
        """Clear the recorded action history."""
        self._history.clear()

    # ------------------------------------------------------------------
    # Action dispatch
    # ------------------------------------------------------------------

    # Handler name lookup table
    _HANDLER_NAMES: dict[ActionType, str] = {
        ActionType.SEND_MESSAGE: "_handle_send_message",
        ActionType.CLAIM_TASK: "_handle_claim_task",
        ActionType.SUBMIT_TASK: "_handle_submit_task",
        ActionType.PROPOSE_DEAL: "_handle_propose_deal",
        ActionType.TEACH_SKILL: "_handle_teach_skill",
        ActionType.PRACTICE_SKILL: "_handle_practice_skill",
        ActionType.REST: "_handle_rest",
        ActionType.EXPLORE: "_handle_explore",
        ActionType.MOVE: "_handle_move",
        ActionType.GATHER: "_handle_gather",
        ActionType.BUILD: "_handle_build",
        ActionType.SOCIALIZE: "_handle_socialize",
        ActionType.FORM_ORG: "_handle_form_org",
        ActionType.JOIN_ORG: "_handle_join_org",
        ActionType.PROPOSE_RULE: "_handle_propose_rule",
        ActionType.VOTE_RULE: "_handle_vote_rule",
        ActionType.RESPOND_ORACLE: "_handle_respond_oracle",
        ActionType.CHECK_BOUNTIES: "_handle_check_bounties",
        ActionType.ACCEPT_BOUNTY: "_handle_accept_bounty",
        ActionType.COMPLETE_BOUNTY: "_handle_complete_bounty",
    }

    async def _dispatch(self, action_type: ActionType, context: ActionContext) -> dict[str, Any]:
        """Route an action type to its handler."""
        handler_name = self._HANDLER_NAMES.get(action_type)
        if handler_name is None:
            raise ValueError(f"Unknown action type: {action_type}")
        handler = getattr(self, handler_name)
        return await handler(context)

    # ------------------------------------------------------------------
    # Individual action handlers (instance methods)
    # ------------------------------------------------------------------

    async def _handle_send_message(self, context: ActionContext) -> dict[str, Any]:
        """Send a message to another agent via A2A.

        Injects ``from_agent``, ``to_agent``, ``message_type``, and ``payload``
        if the LLM omitted them, since the World Engine requires all four fields.
        ``payload`` must be a JSON string, not a dict.
        """
        import json

        payload = dict(context.parameters.get("payload", context.parameters))
        # Ensure required fields
        payload.setdefault("from_agent", str(context.agent.id))
        payload.setdefault("to_agent", context.parameters.get("target_agent_id", ""))
        payload.setdefault("message_type", payload.pop("type", "INFORM"))
        # World Engine expects payload as a JSON string
        if "payload" not in payload:
            payload["payload"] = json.dumps(
                {"text": context.parameters.get("message", "")}
            )
        elif isinstance(payload["payload"], (dict, list)):
            payload["payload"] = json.dumps(payload["payload"])
        return await context.world.send_message(payload)

    async def _handle_claim_task(self, context: ActionContext) -> dict[str, Any]:
        """Claim an available task from the market.

        Degrades gracefully when no ``task_id`` is present (e.g. the LLM chose
        ``claim_task`` but the perception carried no available tasks, or every
        candidate was claimed between perceive and act). Instead of raising —
        which burns all retries — it returns a ``no_tasks_available`` result so
        the caller can move on to the next tick.
        """
        task_id = context.parameters.get("task_id", "")
        if not task_id:
            logger.warning(
                "claim_task executed without a task_id — no task selected"
            )
            return {"action": "claim_task", "status": "no_tasks_available"}
        return await context.world.claim_task(task_id)

    async def _handle_submit_task(self, context: ActionContext) -> dict[str, Any]:
        """Submit completed work for a claimed task."""
        task_id = context.parameters.get("task_id", "")
        task_result = context.parameters.get("result", {})
        if not task_id:
            raise ValueError("submit_task requires 'task_id' parameter")
        return await context.world.submit_task(task_id, task_result)

    async def _handle_propose_deal(self, context: ActionContext) -> dict[str, Any]:
        """Propose a deal/contract to another agent.

        Builds a minimal proposal if the LLM omitted details.
        """
        proposal = context.parameters.get("proposal", {})
        if not proposal:
            proposal = {
                "from_agent": str(context.agent.id),
                "deal_type": "trade",
                "offering": context.parameters.get("offering", "tokens"),
                "offering_amount": context.parameters.get("offering_amount", 10),
                "requesting": context.parameters.get("requesting", "food"),
                "requesting_amount": context.parameters.get("requesting_amount", 5),
            }
        proposal.setdefault("from_agent", str(context.agent.id))
        return await context.world.propose_deal(proposal)

    async def _handle_teach_skill(self, context: ActionContext) -> dict[str, Any]:
        """Teach a skill to another agent."""
        target_id = context.parameters.get("target_agent_id", "")
        skill_name = context.parameters.get("skill_name", "")
        level = context.parameters.get("level", 1)
        if not target_id or not skill_name:
            raise ValueError("teach_skill requires 'target_agent_id' and 'skill_name'")
        return await context.world.teach_skill(target_id, skill_name, level)

    async def _handle_practice_skill(self, context: ActionContext) -> dict[str, Any]:
        """Practice a skill by yourself — improves proficiency, no target needed.

        Unlike ``teach_skill``, self-practice does not require a target agent.
        Sends a ``practice_skill`` intent to the World Engine via the world
        client so the engine can record the attempt and update the agent's
        skill proficiency server-side.
        """
        skill_name = context.parameters.get("skill_name", "")
        return await context.world.practice_skill(skill_name)

    async def _handle_rest(self, context: ActionContext) -> dict[str, Any]:
        """Rest — skip the tick to conserve tokens. No world interaction."""
        return {"action": "rest", "message": "Resting to conserve tokens."}

    async def _handle_explore(self, context: ActionContext) -> dict[str, Any]:
        """Explore the world for opportunities."""
        params = context.parameters.get("explore_params", {})
        return await context.world.explore(params)

    async def _handle_move(self, context: ActionContext) -> dict[str, Any]:
        """Move the agent in a direction via the world client.

        Defaults to a random direction if the LLM omitted it (common when
        the anti-repetition system forces a MOVE action).
        """
        import random

        direction = context.parameters.get("direction", "")
        if not direction:
            direction = random.choice(["north", "south", "east", "west"])
        return await context.world.move(direction)

    async def _handle_gather(self, context: ActionContext) -> dict[str, Any]:
        """Gather a resource from the agent's location.

        Defaults to ``food`` if the LLM omitted ``resource_type`` (common
        when the anti-repetition system forces a GATHER action).
        """
        resource_type = context.parameters.get("resource_type", "food")
        return await context.world.gather(resource_type)

    async def _handle_build(self, context: ActionContext) -> dict[str, Any]:
        """Build a structure at the agent's location."""
        structure_type = context.parameters.get("structure_type", "")
        if not structure_type:
            raise ValueError("build requires 'structure_type' parameter")
        return await context.world.build(
            structure_type,
            **{
                k: v
                for k, v in context.parameters.items()
                if k not in ("structure_type",)
            },
        )

    async def _handle_socialize(self, context: ActionContext) -> dict[str, Any]:
        """Socialize with nearby agents — triggers trust and cultural computations.

        Calls the dedicated ``socialize()`` method on the world client, which
        sends a SOCIALIZE intent to the World Engine for distribution to nearby
        agents.  The action carries ``target_agent_id`` and an optional
        ``message`` that downstream social modules consume.
        """
        target_agent_id = context.parameters.get("target_agent_id", "")
        if not target_agent_id:
            raise ValueError("socialize requires 'target_agent_id' parameter")
        message = context.parameters.get("message", "")
        return await context.world.socialize(target_agent_id, message)
    async def _handle_form_org(self, context: ActionContext) -> dict[str, Any]:
        """Form a new organization — sent as a WILL message to World Engine."""
        org_name = context.parameters.get("org_name", "")
        org_type = context.parameters.get("org_type", "")
        charter = context.parameters.get("charter", "")
        founding_members = context.parameters.get("founding_members", [])
        if not org_name:
            raise ValueError("form_org requires 'org_name' parameter")
        return await context.world.send_message(
            {
                "type": "WILL",
                "payload": {
                    "action": "form_org",
                    "org_name": org_name,
                    "org_type": org_type,
                    "charter": charter,
                    "founding_members": founding_members,
                },
            }
        )

    async def _handle_join_org(self, context: ActionContext) -> dict[str, Any]:
        """Join an existing organization — sent as a PROPOSE message to World Engine."""
        org_id = context.parameters.get("org_id", "")
        if not org_id:
            raise ValueError("join_org requires 'org_id' parameter")
        return await context.world.send_message(
            {
                "type": "PROPOSE",
                "payload": {
                    "action": "join_org",
                    "org_id": org_id,
                },
            }
        )

    async def _handle_propose_rule(self, context: ActionContext) -> dict[str, Any]:
        """Propose a new soft rule — sent as a WILL message to World Engine."""
        org_id = context.parameters.get("org_id", "")
        title = context.parameters.get("title", "")
        description = context.parameters.get("description", "")
        rule_type = context.parameters.get("rule_type", "custom")
        conditions = context.parameters.get("conditions", [])
        effects = context.parameters.get("effects", [])
        if not org_id:
            raise ValueError("propose_rule requires 'org_id' parameter")
        if not title:
            raise ValueError("propose_rule requires 'title' parameter")
        return await context.world.send_message(
            {
                "type": "WILL",
                "payload": {
                    "action": "propose_rule",
                    "org_id": org_id,
                    "title": title,
                    "description": description,
                    "rule_type": rule_type,
                    "conditions": conditions,
                    "effects": effects,
                },
            }
        )

    async def _handle_vote_rule(self, context: ActionContext) -> dict[str, Any]:
        """Vote on a proposed soft rule — sent as a PROPOSE message to World Engine."""
        rule_id = context.parameters.get("rule_id", "")
        support = context.parameters.get("support", True)
        if not rule_id:
            raise ValueError("vote_rule requires 'rule_id' parameter")
        return await context.world.send_message(
            {
                "type": "PROPOSE",
                "payload": {
                    "action": "vote_rule",
                    "rule_id": rule_id,
                    "support": support,
                },
            }
        )

    async def _handle_respond_oracle(self, context: ActionContext) -> dict[str, Any]:
        """Respond to an Oracle message from a human."""
        oracle_id = context.parameters.get("oracle_id", "")
        response = context.parameters.get("response", "")
        if not oracle_id:
            raise ValueError("respond_oracle requires 'oracle_id' parameter")
        if not response:
            raise ValueError("respond_oracle requires 'response' parameter")
        return await context.world.respond_to_oracle(oracle_id, response)

    async def _handle_check_bounties(self, context: ActionContext) -> dict[str, Any]:
        """Check available bounties from the World Engine."""
        return await context.world.check_bounties()

    async def _handle_accept_bounty(self, context: ActionContext) -> dict[str, Any]:
        """Accept (claim) a bounty by ID."""
        bounty_id = context.parameters.get("bounty_id", "")
        if not bounty_id:
            raise ValueError("accept_bounty requires 'bounty_id' parameter")
        return await context.world.claim_bounty(bounty_id)

    async def _handle_complete_bounty(self, context: ActionContext) -> dict[str, Any]:
        """Complete a bounty by ID with a result."""
        bounty_id = context.parameters.get("bounty_id", "")
        result_text = context.parameters.get("result", "")
        if not bounty_id:
            raise ValueError("complete_bounty requires 'bounty_id' parameter")
        if not result_text:
            raise ValueError("complete_bounty requires 'result' parameter")
        return await context.world.complete_bounty(bounty_id, result_text)

    # ------------------------------------------------------------------
    # Internal helpers
    # ------------------------------------------------------------------

    def _record(self, result: ActionResult) -> None:
        """Append an ActionResult to the history log."""
        self._history.append(result)
        logger.debug(
            "Action recorded: type=%s status=%s cost=%d attempts=%d",
            result.action_type.value,
            result.status.value,
            result.token_cost,
            result.attempts,
        )
