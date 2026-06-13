"""LLM-driven decision provider — bridges DecisionEngine to ThinkLoop.

This module adapts the standalone :class:`DecisionEngine` (from ``decide.py``)
to the :class:`DecisionProvider` protocol expected by :class:`ThinkLoop`.
It translates between the two type systems:

- ``Perception`` → ``DecisionPerception``
- ``SurvivalAction`` → ``SurvivalAssessment``
- ``DecisionAction`` → ``ActionType`` (via a mapping table)

On any LLM failure it falls back to a simple random choice.

Usage::

    from agent_runtime.core.llm_decide import LLMDecisionProvider
    from agent_runtime.llm import create_provider, LLMConfig

    llm = create_provider(LLMConfig(provider="ollama", model="minicpm5:1b"))
    provider = LLMDecisionProvider(llm_provider=llm)

    # Use in ThinkLoop
    loop = ThinkLoop(..., decision_provider=provider)
"""

from __future__ import annotations

import logging
import random
from typing import Any

from agent_runtime.core.act import ActionType
from agent_runtime.core.decide import (
    DecisionAction,
    DecisionEngine,
    DecisionPerception,
    SocialContextProvider,
    SurvivalAssessment,
)
from agent_runtime.core.think_loop import Decision, Perception
from agent_runtime.llm.base import LLMProvider
from agent_runtime.models.agent_state import AgentState
from agent_runtime.survival.instinct import SurvivalAction

logger = logging.getLogger(__name__)


# ---------------------------------------------------------------------------
# DecisionAction → ActionType mapping
# ---------------------------------------------------------------------------

_DECISION_TO_ACTION: dict[DecisionAction, ActionType] = {
    DecisionAction.RESPOND_MESSAGE: ActionType.SEND_MESSAGE,
    DecisionAction.CLAIM_TASK: ActionType.CLAIM_TASK,
    DecisionAction.REST: ActionType.REST,
    DecisionAction.EXPLORE: ActionType.EXPLORE,
    DecisionAction.TRADE: ActionType.PROPOSE_DEAL,
    DecisionAction.PRACTICE_SKILL: ActionType.TEACH_SKILL,
    DecisionAction.SOCIALIZE: ActionType.SOCIALIZE,
    DecisionAction.MOVE: ActionType.MOVE,
    DecisionAction.GATHER: ActionType.GATHER,
    DecisionAction.BUILD: ActionType.BUILD,
    DecisionAction.RESPOND_ORACLE: ActionType.RESPOND_ORACLE,
    DecisionAction.CHECK_BOUNTIES: ActionType.CHECK_BOUNTIES,
    DecisionAction.ACCEPT_BOUNTY: ActionType.ACCEPT_BOUNTY,
    DecisionAction.COMPLETE_BOUNTY: ActionType.COMPLETE_BOUNTY,
}

# DecisionActions without a direct ActionType counterpart — mapped to REST
_UNMAPPABLE_ACTIONS: frozenset[DecisionAction] = frozenset()


# ---------------------------------------------------------------------------
# LLMDecisionProvider
# ---------------------------------------------------------------------------


class LLMDecisionProvider:
    """Decision provider backed by a real LLM.

    Uses :class:`DecisionEngine` internally and translates its output
    to the ``ThinkLoop`` ``DecisionProvider`` protocol.  Falls back to
    a random affordable action on any LLM error.
    """

    def __init__(
        self,
        llm_provider: LLMProvider,
        *,
        fallback_actions: list[ActionType] | None = None,
        social_provider: SocialContextProvider | None = None,
    ) -> None:
        self._engine = DecisionEngine(
            provider=llm_provider,
            social_provider=social_provider,
        )
        self._fallback_actions = fallback_actions or [ActionType.REST, ActionType.EXPLORE]

    async def decide(
        self,
        state: AgentState,
        perception: Perception,
        survival: SurvivalAction,
    ) -> Decision:
        """Produce a decision via LLM, falling back to random on failure."""
        # Translate types
        dec_perception = _perception_to_decision(perception)
        dec_survival = _survival_to_assessment(survival)

        try:
            result = await self._engine.decide(state, dec_perception, dec_survival)
            action_type = _map_decision_action(result.action)

            # If the DecisionAction mapped to an unexecutable action,
            # include the original reasoning but note the remap.
            reasoning = result.reasoning
            if result.action in _UNMAPPABLE_ACTIONS:
                reasoning = (
                    f"{reasoning} [Remapped from {result.action.value} to rest"
                    f" — action not yet executable]"
                )

            return Decision(
                action_type=action_type,
                parameters=result.parameters,
                reasoning=reasoning,
            )
        except Exception:
            logger.warning(
                "LLM decision failed for agent %s, falling back to random",
                state.id,
                exc_info=True,
            )
            return _random_fallback(state, self._fallback_actions)


# ---------------------------------------------------------------------------
# Type conversion helpers
# ---------------------------------------------------------------------------


def _perception_to_decision(perception: Perception) -> DecisionPerception:
    """Convert ThinkLoop Perception to DecisionEngine DecisionPerception.

    Extracts structured data from the Perception's market_state and
    messages fields to populate the decision perception with real data.
    """
    # Extract nearby_agents from market_state
    market = perception.market_state
    nearby_agents_raw: list[Any] = market.get("nearby_agents", [])
    nearby_agents: list[str] = []
    for agent_info in nearby_agents_raw:
        if isinstance(agent_info, dict):
            name = agent_info.get("name") or agent_info.get("agent_id") or agent_info.get("id") or "unknown"
            nearby_agents.append(str(name))
        elif isinstance(agent_info, str):
            nearby_agents.append(agent_info)

    # Extract available tasks — market_state may also contain tasks
    available_tasks: list[str] = []
    if perception.active_task:
        available_tasks.append(perception.active_task)
    for task in market.get("available_tasks", []):
        task_str = task if isinstance(task, str) else str(task)
        if task_str not in available_tasks:
            available_tasks.append(task_str)

    # Extract visible_resources from market_state
    visible_resources_raw: list[Any] = market.get("visible_resources", [])
    visible_resources: list[str] = [
        r if isinstance(r, str) else str(r) for r in visible_resources_raw
    ]

    # Extract recent_events from messages (excluding Oracle/Bounty which are
    # tracked separately)
    recent_events: list[str] = []
    pending_oracles: list[dict[str, Any]] = []
    pending_bounties: list[dict[str, Any]] = []

    for msg in perception.messages:
        if isinstance(msg, dict):
            kind = msg.get("kind", "")
            if kind == "oracle":
                pending_oracles.append(msg)
                continue
            if kind == "bounty":
                pending_bounties.append(msg)
                continue

            msg_type = msg.get("type", "event")
            payload = msg.get("payload", {})
            if isinstance(payload, dict):
                summary = payload.get("text") or payload.get("action") or msg_type
            else:
                summary = msg_type
            from_agent = msg.get("from_agent", "")
            if from_agent:
                recent_events.append(f"[{from_agent}] {summary}")
            else:
                recent_events.append(str(summary))
        elif isinstance(msg, str):
            recent_events.append(msg)

    return DecisionPerception(
        tick=perception.tick,
        nearby_agents=nearby_agents,
        available_tasks=available_tasks,
        visible_resources=visible_resources,
        recent_events=recent_events,
        pending_oracles=pending_oracles,
        pending_bounties=pending_bounties,
    )


def _survival_to_assessment(survival: SurvivalAction) -> SurvivalAssessment:
    """Convert SurvivalAction to SurvivalAssessment."""
    # Estimate ticks until depletion based on mode
    ticks_map = {
        "panic": 10,
        "urgent": 50,
        "conservative": 100,
        "normal": 500,
        "invest": 1000,
    }
    mode_str = survival.mode.value
    ticks = ticks_map.get(mode_str, 500)

    return SurvivalAssessment(
        ticks_until_depletion=ticks,
        in_danger=mode_str in ("panic", "urgent"),
        survival_score=max(0, min(100, int(survival.token_ratio * 100))),
        recommendation=f"Survival mode: {mode_str}, token ratio: {survival.token_ratio:.1%}",
    )


def _map_decision_action(action: DecisionAction) -> ActionType:
    """Map a DecisionAction to an ActionType.

    Returns the mapped ActionType if available, otherwise REST.
    """
    mapped = _DECISION_TO_ACTION.get(action)
    if mapped is not None:
        return mapped
    return ActionType.REST


def _random_fallback(
    state: AgentState,
    actions: list[ActionType],
) -> Decision:
    """Return a random affordable action as a fallback."""
    # REST costs 0 so it's always affordable
    affordable = [a for a in actions if a == ActionType.REST or state.tokens > 0]
    if not affordable:
        affordable = [ActionType.REST]

    chosen = random.choice(affordable)
    return Decision(
        action_type=chosen,
        reasoning="Fallback: random decision due to LLM failure",
    )
