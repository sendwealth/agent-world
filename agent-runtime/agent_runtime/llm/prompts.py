"""Structured prompt templates for agent LLM decisions.

Provides a :class:`PromptTemplate` system that renders the decision
prompt from agent state, perception, and survival data.  Different
agents can use different prompt variants (e.g. survival-focused vs
exploration-focused) by selecting a different template name.

The default template is migrated from ``core.decide._DECISION_PROMPT_TEMPLATE``
to centralise all prompt logic in the ``llm`` package.

Usage::

    from agent_runtime.llm.prompts import PromptTemplate, DEFAULT_TEMPLATE

    tpl = PromptTemplate(DEFAULT_TEMPLATE)
    prompt = tpl.render(state, perception, survival, actions)
"""

from __future__ import annotations

import logging
from dataclasses import dataclass, field
from typing import Any

logger = logging.getLogger(__name__)


# ---------------------------------------------------------------------------
# Default prompt template (migrated from core.decide)
# ---------------------------------------------------------------------------


DEFAULT_TEMPLATE = """\
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


# A survival-focused variant — emphasises resource conservation
SURVIVAL_TEMPLATE = """\
You are {name}, an agent in CRITICAL survival mode. Resource conservation is your TOP priority.
{identity_section}
## Agent Identity
- Name: {name} | ID: {id} | Phase: {phase}

## Critical State
- Health: {health:.0f}/100 | Tokens: {tokens} | Money: {money:.1f} | Reputation: {reputation:.1f}

## Survival Alert
- Ticks until depletion: {ticks_until_depletion}
- In danger: {in_danger}
- Survival score: {survival_score}/100
- Recommendation: {recommendation}

## Available Actions (prioritise low-cost)
{actions_section}

## Response Format
{{"action": "<action_name>", "parameters": {{"key": "value"}}, "reasoning": "<why>", \
"confidence": <0-100>}}

Choose the SAFEST action now:"""


# ---------------------------------------------------------------------------
# Template registry
# ---------------------------------------------------------------------------

_TEMPLATES: dict[str, str] = {
    "default": DEFAULT_TEMPLATE,
    "survival": SURVIVAL_TEMPLATE,
}


def register_template(name: str, template: str) -> None:
    """Register a custom prompt template.

    Args:
        name: Template name (used in config / code to select the template).
        template: A Python format string with the same placeholders as
            the default template.
    """
    _TEMPLATES[name] = template


def get_template(name: str = "default") -> str:
    """Retrieve a prompt template by name.

    Returns the default template if the name is not found.
    """
    return _TEMPLATES.get(name, DEFAULT_TEMPLATE)


# ---------------------------------------------------------------------------
# PromptTemplate class
# ---------------------------------------------------------------------------


@dataclass
class PromptTemplate:
    """Structured prompt template for agent LLM decisions.

    Wraps a format string and provides a ``render`` method that fills in
    agent state, perception, survival, and action data.

    Expected interface for parameters passed to ``render()``:

    - ``state``: Must have ``name``, ``id``, ``phase`` (str or enum with
      ``.value``), ``tokens`` (int), ``money`` (float), ``health`` (float),
      ``reputation`` (float), ``skills`` (dict or sequence).
    - ``perception``: Must have ``tick`` (int), ``nearby_agents`` (list[str]),
      ``available_tasks`` (list[str]), ``visible_resources`` (list[str]),
      ``recent_events`` (list[str]).
    - ``survival``: Must have ``ticks_until_depletion`` (int), ``in_danger``
      (bool), ``survival_score`` (int), ``recommendation`` (str).
    - ``actions``: List of objects with ``.value`` (str) and ``.token_cost()``
      (returns int).

    Attributes:
        template: The format string to use.
        name: Human-readable name for logging.
    """

    template: str = field(default_factory=lambda: DEFAULT_TEMPLATE)
    name: str = "default"

    def render(
        self,
        state: Any,
        perception: Any,
        survival: Any,
        actions: list[Any],
    ) -> str:
        """Render the prompt template with concrete data.

        Args:
            state: Agent state (must have name, id, phase, tokens, money,
                health, reputation, skills, personality attributes).
            perception: Perception data (tick, nearby_agents, available_tasks,
                visible_resources, recent_events).
            survival: Survival assessment (ticks_until_depletion, in_danger,
                survival_score, recommendation).
            actions: List of available actions (each must have .value and
                .token_cost()).

        Returns:
            The rendered prompt string ready to send to the LLM.
        """
        skills_section = _format_skills(state.skills)

        nearby = (
            ", ".join(perception.nearby_agents) if perception.nearby_agents else "None"
        )

        tasks_section = (
            "\n".join(f"  - {t}" for t in perception.available_tasks)
            if perception.available_tasks
            else "  None available"
        )

        resources = (
            ", ".join(perception.visible_resources)
            if perception.visible_resources
            else "None visible"
        )

        events_section = (
            "\n".join(f"  - {e}" for e in perception.recent_events)
            if perception.recent_events
            else "  No recent events"
        )

        actions_section = "\n".join(
            f"  - {a.value} (cost: {a.token_cost()} tokens)" for a in actions
        )

        phase_value = (
            state.phase.value if hasattr(state.phase, "value") else str(state.phase)
        )

        reputation_note = (
            "You CAN claim high-value tasks."
            if state.reputation >= 10.0
            else (
                "You CANNOT claim high-value tasks (reward >= 500)"
                " — build reputation by completing smaller tasks first."
            )
        )

        # Build identity section from personality dict
        identity_section = _build_identity_section(state)
        communication_section = _build_communication_section(state)

        return self.template.format(
            name=state.name,
            id=state.id,
            phase=phase_value,
            health=state.health,
            tokens=state.tokens,
            money=state.money,
            reputation=state.reputation,
            identity_section=identity_section,
            communication_section=communication_section,
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
# Helpers
# ---------------------------------------------------------------------------


def _format_skills(skills: Any) -> str:
    """Format the skills section of the prompt."""
    if skills and len(skills) > 0:
        if isinstance(skills, dict):
            return "\n".join(
                f"  - {name}: level {s.level}" for name, s in skills.items()
            )
        return "  (skills present but not in dict form)"
    return "  No skills learned yet."


def _build_identity_section(state: Any) -> str:
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


def _build_communication_section(state: Any) -> str:
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
