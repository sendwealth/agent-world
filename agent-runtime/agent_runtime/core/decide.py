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

from agent_runtime.context.engine import ContextEnginePipeline, PipelineResult
from agent_runtime.llm.base import LLMMessage, LLMProvider

logger = logging.getLogger(__name__)


# ---------------------------------------------------------------------------
# Social context (optional, injected from social.engine)
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class SocialContext:
    """Social context injected into the decision prompt.

    Produced by :class:`agent_runtime.social.engine.SocialEngine.build_context`.
    """

    social_propensity: float = 0.5
    should_socialize: bool = False
    recommended_target_id: str = ""
    trust_snapshot: dict[str, float] = field(default_factory=dict)
    personality_description: str = ""


# ---------------------------------------------------------------------------
# Action types — 10 actions with token costs
# ---------------------------------------------------------------------------


class DecisionAction(str, Enum):
    """All possible actions an agent can choose in a single tick.

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
    FORM_ORG = "form_org"  # 25 tokens
    JOIN_ORG = "join_org"  # 10 tokens
    PROPOSE_RULE = "propose_rule"  # 15 tokens
    VOTE_RULE = "vote_rule"  # 5 tokens
    RESPOND_ORACLE = "respond_oracle"  # 3 tokens
    CHECK_BOUNTIES = "check_bounties"  # 2 tokens
    ACCEPT_BOUNTY = "accept_bounty"  # 10 tokens
    COMPLETE_BOUNTY = "complete_bounty"  # 8 tokens

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
    DecisionAction.FORM_ORG: 25,
    DecisionAction.JOIN_ORG: 10,
    DecisionAction.PROPOSE_RULE: 15,
    DecisionAction.VOTE_RULE: 5,
    DecisionAction.RESPOND_ORACLE: 3,
    DecisionAction.CHECK_BOUNTIES: 2,
    DecisionAction.ACCEPT_BOUNTY: 10,
    DecisionAction.COMPLETE_BOUNTY: 8,
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
{identity_section}
## Agent Identity
- Name: {name}
- ID: {id}
- Phase: {phase}

## Current State
- Health: {health:.0f}/100
- Tokens: {tokens}
- Money: {money:.1f}
- Reputation: {reputation:.1f}
{communication_section}
## Personality
{personality_description}

## Current Mood
{mood_description}

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

## Social Context
- Social propensity: {social_propensity:.0%}
- Should socialize: {should_socialize}
{social_targets_section}

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


def _build_identity_section(state: AgentStateProtocol) -> str:
    """Build backstory / alignment / archetype section from agent personality.

    Returns an empty string if no identity data is present (backward compatible).
    """
    personality = getattr(state, "personality", None)
    if not personality or not isinstance(personality, dict):
        return ""

    parts: list[str] = []

    backstory = personality.get("backstory", "")
    if backstory:
        parts.append(f"\n## Backstory\n{backstory}")

    alignment = personality.get("alignment", "")
    if alignment:
        parts.append(f"\n## Alignment\n{alignment}")

    archetype = personality.get("archetype", "")
    if archetype:
        parts.append(f"\n## Archetype\n{archetype}")

    bio = personality.get("bio", "")
    if bio:
        parts.append(f"\n## Bio\n{bio}")

    return "\n".join(parts)


def _build_communication_section(state: AgentStateProtocol) -> str:
    """Build communication style section from agent personality.

    Returns an empty string if no communication_style is configured.
    """
    personality = getattr(state, "personality", None)
    if not personality or not isinstance(personality, dict):
        return ""

    comm_style = personality.get("communication_style", "")
    if not comm_style:
        return ""

    return f"\n## Communication Style\n{comm_style}\n"


def build_prompt(
    state: AgentStateProtocol,
    perception: DecisionPerception,
    survival: SurvivalAssessment,
    available_actions: list[DecisionAction],
    social: SocialContext | None = None,
    mood_description: str | None = None,
) -> str:
    """Build the decision prompt from agent state, perception, and survival data.

    The prompt is structured to guide the LLM to return valid JSON:
    1. Agent identity & current state
    2. Personality description (from social context)
    3. Current mood (from emotion engine)
    4. Skills assessment
    5. Perception (nearby agents, tasks, resources, events)
    6. Survival assessment
    7. Social context (propensity, trust, recommended targets)
    8. Available actions with costs
    9. Response format instructions
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
        else (
            "You CANNOT claim high-value tasks (reward >= 500)"
            " — build reputation by completing smaller tasks first."
        )
    )

    # Social context fields
    personality_description = ""
    social_propensity = 0.0
    should_socialize = False
    social_targets_section = "  No social context available."

    if social is not None:
        personality_description = social.personality_description or "No personality data."
        social_propensity = social.social_propensity
        should_socialize = social.should_socialize

        if social.trust_snapshot:
            trust_lines = [
                f"  - {aid}: trust={t:.2f}"
                for aid, t in social.trust_snapshot.items()
            ]
            social_targets_section = "\n".join(trust_lines)
        else:
            social_targets_section = "  No nearby agents to socialize with."

        if social.recommended_target_id:
            social_targets_section += (
                f"\n  Recommended social target: {social.recommended_target_id}"
            )

    # Mood description
    mood_desc = mood_description or "No mood data available."

    # Build identity section from agent personality dict
    identity_section = _build_identity_section(state)
    communication_section = _build_communication_section(state)

    return _DECISION_PROMPT_TEMPLATE.format(
        name=state.name,
        id=state.id,
        phase=phase_value,
        health=state.health,
        tokens=state.tokens,
        money=state.money,
        reputation=state.reputation,
        identity_section=identity_section,
        communication_section=communication_section,
        personality_description=personality_description,
        mood_description=mood_desc,
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
        social_propensity=social_propensity,
        should_socialize=should_socialize,
        social_targets_section=social_targets_section,
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


class SocialContextProvider(Protocol):
    """Provides social context for the decision engine."""

    def build_social_context(
        self,
        agent_id: str,
        tick: int,
    ) -> SocialContext | None: ...


class EmotionContextProvider(Protocol):
    """Provides emotion context (mood description) for the decision engine."""

    def get_mood_description(self) -> str: ...


class DecisionEngine:
    """Core decision engine that drives agent behavior via LLM.

    Supports optional fallback providers: if the primary LLM call fails,
    each fallback is tried in order.  If all providers fail, a random
    decision is returned (via ``fallback_decision()``).

    Usage::

        engine = DecisionEngine(provider=my_llm)
        decision = await engine.decide(state, perception, survival)

    With fallbacks::

        from agent_runtime.llm.fallback import ModelFallback, FallbackChainProvider

        chain = ModelFallback(primary=primary_llm, fallbacks=[backup_llm])
        engine = DecisionEngine(provider=FallbackChainProvider(chain))
    """

    def __init__(
        self,
        provider: LLMProvider,
        *,
        pipeline: ContextEnginePipeline | None = None,
        social_provider: SocialContextProvider | None = None,
        emotion_provider: EmotionContextProvider | None = None,
        fallback_providers: list[LLMProvider] | None = None,
    ) -> None:
        self._provider = provider
        self._pipeline = pipeline
        self._social_provider = social_provider
        self._emotion_provider = emotion_provider
        self._fallback_providers = fallback_providers or []

    async def decide(
        self,
        state: AgentStateProtocol,
        perception: DecisionPerception,
        survival: SurvivalAssessment,
    ) -> Decision:
        """Generate a decision for the given agent context.

        This is the main entry point. It:
        1. Computes available actions from agent state
        2. Builds the prompt (with optional social context)
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

        # Build social context if provider is available
        social: SocialContext | None = None
        if self._social_provider is not None:
            try:
                social = self._social_provider.build_social_context(
                    agent_id=state.id,
                    tick=perception.tick,
                )
            except Exception:
                logger.debug(
                    "Social context build failed for agent %s (non-fatal)",
                    state.id,
                    exc_info=True,
                )

        # Get mood description if emotion provider is available
        mood_description: str | None = None
        if self._emotion_provider is not None:
            try:
                mood_description = self._emotion_provider.get_mood_description()
            except Exception:
                logger.debug(
                    "Emotion context build failed for agent %s (non-fatal)",
                    state.id,
                    exc_info=True,
                )

        try:
            decision = await self._try_decide(
                state, perception, survival, available, social=social,
                mood_description=mood_description,
            )
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
        *,
        social: SocialContext | None = None,
        mood_description: str | None = None,
    ) -> Decision:
        """Attempt to generate a validated decision via the LLM.

        If the primary provider fails, tries each fallback provider in
        order.  Raises ``LlmCallError`` only when all providers fail.
        """
        if self._pipeline is not None:
            pipeline_result: PipelineResult = self._pipeline.run(
                perception=perception,
                survival=survival,
                state=state,
            )
            prompt = pipeline_result.formatted_context
        else:
            prompt = build_prompt(
                state, perception, survival, available_actions, social=social,
                mood_description=mood_description,
            )

        # Build the provider chain: primary + fallbacks
        providers = [self._provider] + self._fallback_providers
        messages = [LLMMessage(role="user", content=prompt)]

        last_error: Exception | None = None
        for i, provider in enumerate(providers):
            label = "primary" if i == 0 else f"fallback[{i - 1}]"
            try:
                response = await provider.chat(messages)
                # Parse and validate the response
                raw = response.content
                parsed = parse_llm_response(raw)
                decision = Decision(
                    action=DecisionAction(parsed["action"]),
                    parameters=parsed["parameters"],
                    reasoning=parsed["reasoning"],
                    confidence=parsed["confidence"],
                )
                validate_decision(decision, state, available_actions)

                if i > 0:
                    logger.info(
                        "Fallback triggered: %s succeeded for agent %s "
                        "(provider=%s, model=%s)",
                        label,
                        state.id,
                        provider._config.provider.value,
                        provider._config.model,
                        extra={
                            "event": "fallback_triggered",
                            "agent": state.id,
                            "fallback_index": i - 1,
                            "provider": provider._config.provider.value,
                            "model": provider._config.model,
                        },
                    )

                return decision
            except Exception as exc:
                last_error = exc
                if i < len(providers) - 1:
                    next_label = "fallback[0]" if i == 0 else f"fallback[{i}]"
                    logger.warning(
                        "Fallback triggered: %s failed for agent %s (%s: %s), "
                        "trying %s",
                        label,
                        state.id,
                        type(exc).__name__,
                        exc,
                        next_label,
                        extra={
                            "event": "fallback_triggered",
                            "agent": state.id,
                            "failed_provider": provider._config.provider.value
                            if hasattr(provider, "_config")
                            else "unknown",
                        },
                    )

        # All providers failed
        if last_error is not None:
            raise LlmCallError(
                f"All providers failed (primary + {len(self._fallback_providers)} "
                f"fallbacks): {last_error}"
            ) from last_error

        raise LlmCallError("No providers available")
