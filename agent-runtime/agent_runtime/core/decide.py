"""Decision engine — LLM-driven decision making for agent thinking cycle.

This module implements the core decision engine that:
1. Builds a Decision Prompt from agent state (skills, perception, survival assessment)
2. Calls an LLM provider and parses JSON responses
3. Validates decisions against token budgets and action constraints
4. Falls back to random decisions when the LLM fails

Usage::

    from agent_runtime.core.decide import DecisionEngine, DecisionAction
    from agent_runtime.llm.base import LLMConfig, LLMMessage, LLMProvider

    engine = DecisionEngine(provider=my_llm_provider)
    decision = await engine.decide(state, perception, survival)
    # decision.action -> DecisionAction
    # decision.parameters -> dict
    # decision.reasoning -> str
    # decision.confidence -> int (0-100)
"""

from __future__ import annotations

import json
import logging
import random
import re
from dataclasses import dataclass, field
from enum import Enum
from typing import Any, Protocol

from agent_runtime.llm.base import LLMMessage, LLMProvider

logger = logging.getLogger(__name__)


# ---------------------------------------------------------------------------
# Action types — 10 actions with token costs
# ---------------------------------------------------------------------------


class DecisionAction(str, Enum):
    """All 10 possible actions an agent can choose in a single tick.

    Token costs are aligned with the issue spec and genesis.yaml.
    """

    RESPOND_MESSAGE = "respond_message"  # 5 tokens
    CLAIM_TASK = "claim_task"  # 10 tokens
    REST = "rest"  # 0 tokens (free)
    EXPLORE = "explore"  # 15 tokens
    TRADE = "trade"  # 10 tokens
    PRACTICE_SKILL = "practice_skill"  # 8 tokens
    MOVE = "move"  # 12 tokens
    GATHER = "gather"  # 8 tokens
    BUILD = "build"  # 20 tokens
    SOCIALIZE = "socialize"  # 5 tokens

    @classmethod
    def all(cls) -> list[DecisionAction]:
        """Return all available action variants."""
        return list(cls)

    def token_cost(self) -> int:
        """Return the token cost for this action."""
        return _TOKEN_COSTS[self]


# Token cost table per the issue spec
_TOKEN_COSTS: dict[DecisionAction, int] = {
    DecisionAction.RESPOND_MESSAGE: 5,
    DecisionAction.CLAIM_TASK: 10,
    DecisionAction.REST: 0,
    DecisionAction.EXPLORE: 15,
    DecisionAction.TRADE: 10,
    DecisionAction.PRACTICE_SKILL: 8,
    DecisionAction.MOVE: 12,
    DecisionAction.GATHER: 8,
    DecisionAction.BUILD: 20,
    DecisionAction.SOCIALIZE: 5,
}


def _action_from_name(name: str) -> DecisionAction | None:
    """Look up a DecisionAction by its snake_case value string."""
    try:
        return DecisionAction(name)
    except ValueError:
        return None


# ---------------------------------------------------------------------------
# Perception & Survival context for decisions
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class DecisionPerception:
    """What the agent perceives about its environment at decision time."""

    tick: int = 0
    nearby_agents: list[str] = field(default_factory=list)
    available_tasks: list[str] = field(default_factory=list)
    visible_resources: list[str] = field(default_factory=list)
    recent_events: list[str] = field(default_factory=list)


@dataclass(frozen=True)
class SurvivalAssessment:
    """Survival assessment passed to the decision engine."""

    ticks_until_depletion: int = 0
    in_danger: bool = False
    survival_score: int = 100  # 0-100
    recommendation: str = "Agent is stable"


# ---------------------------------------------------------------------------
# Agent state protocol (decoupled from concrete AgentState)
# ---------------------------------------------------------------------------


class AgentStateProtocol(Protocol):
    """Minimal interface the decision engine needs from agent state."""

    @property
    def name(self) -> str: ...

    @property
    def id(self) -> str: ...

    @property
    def phase(self) -> Any: ...

    @property
    def tokens(self) -> int: ...

    @property
    def money(self) -> float: ...

    @property
    def health(self) -> float: ...

    @property
    def reputation(self) -> float: ...

    @property
    def skills(self) -> Any: ...


# ---------------------------------------------------------------------------
# Decision output
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class Decision:
    """A validated decision produced by the decision engine."""

    action: DecisionAction
    parameters: dict[str, Any] = field(default_factory=dict)
    reasoning: str = ""
    confidence: int = 50  # 0-100


# ---------------------------------------------------------------------------
# Errors
# ---------------------------------------------------------------------------


class DecisionError(Exception):
    """Base error for the decision engine."""


class LlmCallError(DecisionError):
    """LLM provider call failed."""


class JsonParseError(DecisionError):
    """Failed to parse LLM response as valid decision JSON."""


class ValidationError(DecisionError):
    """Decision failed validation checks."""


# ---------------------------------------------------------------------------
# Prompt template
# ---------------------------------------------------------------------------

_DECISION_PROMPT_TEMPLATE = """\
You are {name}, an autonomous agent in a simulated world. Analyze your current \
state and choose the best action.

## Agent Identity
- Name: {name}
- ID: {id}
- Phase: {phase}

## Current State
- Health: {health:.0f}/100
- Tokens: {tokens}
- Money: {money:.1f}
- Reputation: {reputation:.1f}

## Skills
{skills_section}

## Perception (Tick {tick})
- Nearby agents: {nearby}
- Available tasks:
{tasks_section}
- Visible resources: {resources}
- Recent events:
{events_section}

## Survival Assessment
- Ticks until token depletion: {ticks_until_depletion}
- In danger: {in_danger}
- Survival score: {survival_score}/100
- Recommendation: {recommendation}

## Available Actions
{actions_section}

## Token Budget
You have {tokens} tokens remaining. Each action has a cost listed above.
Do NOT choose an action you cannot afford.

## Reputation Constraints
Your reputation is {reputation:.1f}. High-value tasks (reward >= 500) require reputation >= 10.0.
{reputation_note}

## Response Format
Respond with ONLY a JSON object (no markdown, no explanation outside JSON):
{{"action": "<action_name>", "parameters": {{"key": "value"}}, "reasoning": "<why>", \
"confidence": <0-100>}}

Choose the best action now:"""


def build_prompt(
    state: AgentStateProtocol,
    perception: DecisionPerception,
    survival: SurvivalAssessment,
    available_actions: list[DecisionAction],
) -> str:
    """Build the decision prompt from agent state, perception, and survival data.

    The prompt is structured to guide the LLM to return valid JSON:
    1. Agent identity & current state
    2. Skills assessment
    3. Perception (nearby agents, tasks, resources, events)
    4. Survival assessment
    5. Available actions with costs
    6. Response format instructions
    """
    # Skills
    skills = state.skills
    if skills and len(skills) > 0:
        if isinstance(skills, dict):
            skills_section = "\n".join(f"  - {name}: level {s.level}" for name, s in skills.items())
        else:
            skills_section = "  (skills present but not in dict form)"
    else:
        skills_section = "  No skills learned yet."

    # Nearby agents
    nearby = ", ".join(perception.nearby_agents) if perception.nearby_agents else "None"

    # Tasks
    tasks_section = (
        "\n".join(f"  - {t}" for t in perception.available_tasks)
        if perception.available_tasks
        else "  None available"
    )

    # Resources
    resources = (
        ", ".join(perception.visible_resources) if perception.visible_resources else "None visible"
    )

    # Events
    events_section = (
        "\n".join(f"  - {e}" for e in perception.recent_events)
        if perception.recent_events
        else "  No recent events"
    )

    # Actions with costs
    actions_section = "\n".join(
        f"  - {a.value} (cost: {a.token_cost()} tokens)" for a in available_actions
    )

    phase_value = state.phase.value if hasattr(state.phase, "value") else str(state.phase)

    # Reputation constraint note
    reputation_note = (
        "You CAN claim high-value tasks."
        if state.reputation >= 10.0
        else "You CANNOT claim high-value tasks (reward >= 500) — build reputation by completing smaller tasks first."
    )

    return _DECISION_PROMPT_TEMPLATE.format(
        name=state.name,
        id=state.id,
        phase=phase_value,
        health=state.health,
        tokens=state.tokens,
        money=state.money,
        reputation=state.reputation,
        skills_section=skills_section,
        tick=perception.tick,
        nearby=nearby,
        tasks_section=tasks_section,
        resources=resources,
        events_section=events_section,
        ticks_until_depletion=survival.ticks_until_depletion,
        in_danger=survival.in_danger,
        survival_score=survival.survival_score,
        recommendation=survival.recommendation,
        actions_section=actions_section,
        reputation_note=reputation_note,
    )


# ---------------------------------------------------------------------------
# JSON response parsing
# ---------------------------------------------------------------------------


def strip_code_fences(text: str) -> str:
    """Strip markdown code fences from LLM output."""
    trimmed = text.strip()
    if not trimmed.startswith("```"):
        return trimmed

    # Strip opening ```json or ```
    without_start = re.sub(r"^```(?:json)?\s*\n?", "", trimmed, count=1)

    # Strip closing ```
    without_end = re.sub(r"\n?```\s*$", "", without_start)

    return without_end.strip()


def parse_llm_response(raw: str) -> dict[str, Any]:
    """Parse the raw LLM response string into a dict.

    Handles:
    - Markdown code fences (```json ... ```)
    - JSON extraction from surrounding text

    Returns:
        Parsed dict with at least 'action' key.

    Raises:
        JsonParseError: If the response cannot be parsed as valid JSON
            or the action field is missing/invalid.
    """
    cleaned = strip_code_fences(raw)

    try:
        data = json.loads(cleaned)
    except json.JSONDecodeError as e:
        raise JsonParseError(f"Failed to parse LLM response as JSON: {e}") from e

    if not isinstance(data, dict) or "action" not in data:
        raise JsonParseError("LLM response must be a JSON object with an 'action' field")

    # Validate action name
    action_name = data["action"]
    action = _action_from_name(action_name)
    if action is None:
        raise JsonParseError(f"Unknown action: {action_name}")

    # Ensure parameters is a dict
    if "parameters" not in data:
        data["parameters"] = {}
    elif not isinstance(data["parameters"], dict):
        data["parameters"] = {}

    # Ensure reasoning is a string
    if "reasoning" not in data:
        data["reasoning"] = ""
    elif not isinstance(data["reasoning"], str):
        data["reasoning"] = str(data["reasoning"])

    # Clamp confidence to 0-100
    confidence = data.get("confidence", 50)
    try:
        confidence = int(confidence)
    except (TypeError, ValueError):
        confidence = 50
    data["confidence"] = max(0, min(100, confidence))

    return data


# ---------------------------------------------------------------------------
# Decision validation
# ---------------------------------------------------------------------------


def validate_decision(
    decision: Decision,
    state: AgentStateProtocol,
    available_actions: list[DecisionAction],
) -> None:
    """Validate a parsed decision against the agent's current state.

    Checks:
    - Dead agents cannot act
    - Action must be in the available set
    - Agent must have enough tokens for the action
    - Confidence must be 0-100

    Raises:
        ValidationError: If any check fails.
    """
    # Dead agents cannot act
    phase_value = state.phase.value if hasattr(state.phase, "value") else str(state.phase)
    if phase_value == "dead":
        raise ValidationError("Agent is dead and cannot act")

    # Action must be available
    if decision.action not in available_actions:
        available_names = [a.value for a in available_actions]
        raise ValidationError(
            f"Action '{decision.action.value}' not available (available: {available_names})"
        )

    # Token budget check
    cost = decision.action.token_cost()
    if cost > state.tokens:
        raise ValidationError(
            f"Insufficient tokens: action '{decision.action.value}' "
            f"costs {cost}, agent has {state.tokens}"
        )

    # Confidence range
    if not 0 <= decision.confidence <= 100:
        raise ValidationError(f"Invalid confidence: {decision.confidence} (must be 0-100)")


# ---------------------------------------------------------------------------
# Fallback strategy
# ---------------------------------------------------------------------------


def fallback_decision(
    state: AgentStateProtocol,
    available_actions: list[DecisionAction],
) -> Decision:
    """Generate a random fallback decision when the LLM fails.

    Picks a random affordable action from the available actions.
    If no action is affordable, defaults to REST (which is always free).
    """
    affordable = [a for a in available_actions if a.token_cost() <= state.tokens]

    if not affordable:
        # REST is always free, use as ultimate fallback
        return Decision(
            action=DecisionAction.REST,
            reasoning="Fallback: no affordable actions, resting",
            confidence=0,
        )

    chosen = random.choice(affordable)
    return Decision(
        action=chosen,
        reasoning="Fallback: random decision due to LLM failure",
        confidence=0,
    )


# ---------------------------------------------------------------------------
# Available actions helper
# ---------------------------------------------------------------------------


def get_available_actions(
    state: AgentStateProtocol,
    *,
    dead_phase: str = "dead",
) -> list[DecisionAction]:
    """Compute the list of actions the agent can take this tick.

    Filters by token affordability and removes all actions for dead agents.
    """
    phase_value = state.phase.value if hasattr(state.phase, "value") else str(state.phase)
    if phase_value == dead_phase:
        return []

    return [a for a in DecisionAction.all() if a.token_cost() <= state.tokens]


# ---------------------------------------------------------------------------
# Decision Engine
# ---------------------------------------------------------------------------


class DecisionEngine:
    """Core decision engine that drives agent behavior via LLM.

    Usage::

        engine = DecisionEngine(provider=my_llm)
        decision = await engine.decide(state, perception, survival)
    """

    def __init__(self, provider: LLMProvider) -> None:
        self._provider = provider

    async def decide(
        self,
        state: AgentStateProtocol,
        perception: DecisionPerception,
        survival: SurvivalAssessment,
    ) -> Decision:
        """Generate a decision for the given agent context.

        This is the main entry point. It:
        1. Computes available actions from agent state
        2. Builds the prompt
        3. Calls the LLM
        4. Parses the JSON response
        5. Validates the decision
        6. Falls back to a random decision on any failure
        """
        available = get_available_actions(state)

        # No actions available (e.g. dead agent)
        if not available:
            return Decision(
                action=DecisionAction.REST,
                reasoning="No available actions",
                confidence=0,
            )

        try:
            decision = await self._try_decide(state, perception, survival, available)
            logger.info(
                "Agent %s decided: %s (confidence: %d)",
                state.id,
                decision.action.value,
                decision.confidence,
            )
            return decision
        except DecisionError as e:
            logger.warning(
                "Agent %s LLM decision failed (%s), falling back to random",
                state.id,
                e,
            )
            return fallback_decision(state, available)

    async def _try_decide(
        self,
        state: AgentStateProtocol,
        perception: DecisionPerception,
        survival: SurvivalAssessment,
        available_actions: list[DecisionAction],
    ) -> Decision:
        """Attempt to generate a validated decision via the LLM."""
        prompt = build_prompt(state, perception, survival, available_actions)

        # Call LLM provider
        try:
            response = await self._provider.chat([LLMMessage(role="user", content=prompt)])
        except Exception as e:
            raise LlmCallError(f"LLM request failed: {e}") from e

        raw = response.content

        # Parse JSON
        parsed = parse_llm_response(raw)

        # Build Decision object
        decision = Decision(
            action=DecisionAction(parsed["action"]),
            parameters=parsed["parameters"],
            reasoning=parsed["reasoning"],
            confidence=parsed["confidence"],
        )

        # Validate
        validate_decision(decision, state, available_actions)

        return decision
