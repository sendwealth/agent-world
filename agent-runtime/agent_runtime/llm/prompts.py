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

from dataclasses import dataclass, field
from typing import Any, Protocol

logger = __import__("logging").getLogger(__name__)


# ---------------------------------------------------------------------------
# Protocols (decoupled from concrete agent types)
# ---------------------------------------------------------------------------


class _HasState(Protocol):
    """Minimal state interface the prompt template needs."""

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


class _HasPerception(Protocol):
    @property
    def tick(self) -> int: ...

    @property
    def nearby_agents(self) -> list[str]: ...

    @property
    def available_tasks(self) -> list[str]: ...

    @property
    def visible_resources(self) -> list[str]: ...

    @property
    def recent_events(self) -> list[str]: ...


class _HasSurvival(Protocol):
    @property
    def ticks_until_depletion(self) -> int: ...

    @property
    def in_danger(self) -> bool: ...

    @property
    def survival_score(self) -> int: ...

    @property
    def recommendation(self) -> str: ...


class _HasAction(Protocol):
    @property
    def value(self) -> str: ...

    def token_cost(self) -> int: ...


# ---------------------------------------------------------------------------
# Default prompt template (migrated from core.decide)
# ---------------------------------------------------------------------------


DEFAULT_TEMPLATE = """\
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


# A survival-focused variant — emphasises resource conservation
SURVIVAL_TEMPLATE = """\
You are {name}, an agent in CRITICAL survival mode. Resource conservation is your TOP priority.

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
                health, reputation, skills attributes).
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

        return self.template.format(
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
