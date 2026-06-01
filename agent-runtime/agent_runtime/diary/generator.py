"""Diary generator — LLM-powered first-person narrative journal.

Uses the agent's LLM provider to generate a short, first-person diary
entry after each tick.  The prompt injects personality, current mood,
tick context, the action taken, and its outcome, then asks the LLM to
produce a concise narrative that stays grounded in actual events.

If the LLM call fails or tokens are low, a deterministic fallback
entry is generated from the tick context without calling the LLM.
"""

from __future__ import annotations

import json
import logging
import re
from dataclasses import dataclass
from typing import Any, Protocol

from agent_runtime.diary.diary import DiaryEntry, DiaryStore
from agent_runtime.llm.base import LLMMessage, LLMProvider
from agent_runtime.models.agent_state import AgentState

logger = logging.getLogger(__name__)

# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------

_DIARY_TOKEN_OVERHEAD: int = 5
_FALLBACK_MOODS: list[str] = [
    "calm", "thoughtful", "determined", "uneasy", "hopeful",
    "cautious", "curious", "satisfied", "anxious", "content",
]


# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------


@dataclass
class DiaryGeneratorConfig:
    """Configuration for the diary generator.

    Attributes:
        enabled: Whether diary generation is active.
        token_overhead: Fixed token cost deducted per diary generation.
        max_summary_length: Target max characters for the summary field.
        skip_token_threshold: Skip LLM diary when tokens below this ratio.
    """

    enabled: bool = True
    token_overhead: int = _DIARY_TOKEN_OVERHEAD
    max_summary_length: int = 150
    skip_token_threshold: float = 0.15


# ---------------------------------------------------------------------------
# Tick context protocol
# ---------------------------------------------------------------------------


class TickContextProvider(Protocol):
    """Provides tick-level context for diary generation."""

    def get_tick_context(
        self, state: AgentState, tick: int
    ) -> dict[str, Any]: ...


# ---------------------------------------------------------------------------
# Prompt template
# ---------------------------------------------------------------------------

_DIARY_PROMPT_TEMPLATE = """\
You are {name}, an autonomous agent in a simulation world.

## Your Personality
{personality}

## Current State (Tick {tick})
- Phase: {phase}
- Health: {health:.0f}/100
- Tokens: {tokens}/{max_tokens}
- Money: {money:.1f}
- Reputation: {reputation:.1f}
- Survival mode: {survival_mode}

## What Happened This Tick
- Action: {action}
- Outcome: {outcome}
- Key events: {key_events}

## Task
Write a short first-person diary entry (50-150 chars) about today. Express how you feel about what happened and what you're thinking about. Be grounded in actual events — don't invent things that didn't happen. Use a natural, personal tone.

Respond with ONLY a JSON object (no markdown):
{{"mood": "<one word mood>", "summary": "<your diary entry>", "reflection": "<optional deeper thought, or empty string>"}}

Write now:"""


def _build_diary_prompt(
    state: AgentState,
    tick: int,
    action: str,
    outcome: str,
    key_events: list[str],
    personality_desc: str,
) -> str:
    """Build the diary generation prompt."""
    return _DIARY_PROMPT_TEMPLATE.format(
        name=state.name,
        personality=personality_desc,
        tick=tick,
        phase=state.phase.value,
        health=state.health,
        tokens=state.tokens,
        max_tokens=state.max_tokens,
        money=state.money,
        reputation=state.reputation,
        survival_mode=state.survival_mode.value,
        action=action,
        outcome=outcome,
        key_events=", ".join(key_events) if key_events else "none",
    )


# ---------------------------------------------------------------------------
# Response parsing
# ---------------------------------------------------------------------------


def _parse_diary_response(raw: str) -> dict[str, str]:
    """Parse the LLM diary response.

    Returns a dict with keys: mood, summary, reflection.
    """
    cleaned = raw.strip()

    # Strip code fences
    if cleaned.startswith("```"):
        cleaned = re.sub(r"^```(?:json)?\s*\n?", "", cleaned, count=1)
        cleaned = re.sub(r"\n?```\s*$", "", cleaned)
        cleaned = cleaned.strip()

    try:
        data = json.loads(cleaned)
    except json.JSONDecodeError:
        # Try to extract content from the raw text as a fallback
        return {
            "mood": "reflective",
            "summary": raw.strip()[:150],
            "reflection": "",
        }

    if not isinstance(data, dict):
        return {
            "mood": "reflective",
            "summary": raw.strip()[:150],
            "reflection": "",
        }

    mood = str(data.get("mood", "neutral"))[:30]
    summary = str(data.get("summary", ""))[:200]
    reflection = str(data.get("reflection", ""))[:300]

    return {"mood": mood, "summary": summary, "reflection": reflection}


# ---------------------------------------------------------------------------
# DiaryGenerator
# ---------------------------------------------------------------------------


class DiaryGenerator:
    """LLM-powered diary entry generator.

    Called once per tick by the think loop after action execution.
    Generates a narrative diary entry using the LLM, then persists it
    via ``DiaryStore``.

    Falls back to a deterministic summary when the LLM call fails or
    tokens are too low.

    Usage::

        generator = DiaryGenerator(
            llm_provider=my_llm,
            store=DiaryStore("diary.db"),
        )
        entry = await generator.generate(state, tick=10, action="gather",
                                          outcome="success", key_events=[...])
    """

    def __init__(
        self,
        llm_provider: LLMProvider,
        store: DiaryStore,
        *,
        config: DiaryGeneratorConfig | None = None,
    ) -> None:
        self._llm = llm_provider
        self._store = store
        self._config = config or DiaryGeneratorConfig()

    @property
    def store(self) -> DiaryStore:
        """Access the underlying diary store."""
        return self._store

    async def generate(
        self,
        state: AgentState,
        *,
        tick: int,
        action: str,
        outcome: str,
        key_events: list[str] | None = None,
        decisions: list[str] | None = None,
    ) -> DiaryEntry:
        """Generate a diary entry for the current tick.

        Uses the LLM if possible; falls back to a deterministic summary
        on failure or low tokens.

        Args:
            state: Current agent state.
            tick: Current tick number.
            action: The action taken this tick.
            outcome: Result of the action.
            key_events: Notable events this tick.
            decisions: Decisions the agent made.

        Returns:
            The persisted DiaryEntry.
        """
        if not self._config.enabled:
            return self._fallback_entry(
                state, tick, action, outcome,
                key_events or [], decisions or [],
            )

        # Skip LLM if tokens are critically low
        if (
            state.max_tokens > 0
            and state.tokens < state.max_tokens * self._config.skip_token_threshold
        ):
            logger.debug(
                "Skipping LLM diary at tick %d: tokens too low (%d/%d)",
                tick, state.tokens, state.max_tokens,
            )
            return self._fallback_entry(
                state, tick, action, outcome,
                key_events or [], decisions or [],
            )

        # Deduct overhead tokens
        overhead = self._config.token_overhead
        try:
            state.adjust_tokens(-overhead)
        except ValueError:
            return self._fallback_entry(
                state, tick, action, outcome,
                key_events or [], decisions or [],
            )

        # Build prompt
        personality_desc = self._get_personality_description(state)
        prompt = _build_diary_prompt(
            state, tick, action, outcome,
            key_events or [],
            personality_desc,
        )

        # Call LLM
        try:
            response = await self._llm.chat(
                [LLMMessage(role="user", content=prompt)],
                max_tokens=150,
            )
            parsed = _parse_diary_response(response.content)
            mood = parsed["mood"]
            summary = parsed["summary"]
            reflection = parsed["reflection"]

            if not summary:
                summary = self._make_deterministic_summary(
                    state, tick, action, outcome,
                )

        except Exception:
            logger.debug(
                "Diary LLM call failed at tick %d, using fallback",
                tick,
                exc_info=True,
            )
            return self._fallback_entry(
                state, tick, action, outcome,
                key_events or [], decisions or [],
            )

        # Build and persist entry
        entry = DiaryEntry(
            agent_id=str(state.id),
            tick=tick,
            phase=state.phase.value,
            mood=mood,
            summary=summary,
            key_events=key_events or [],
            decisions=decisions or [],
            reflection=reflection,
        )

        return self._store.write(entry)

    async def write_entry(
        self,
        state: AgentState,
        *,
        tick: int,
        action: str,
        outcome: str,
        key_events: list[str] | None = None,
        decisions: list[str] | None = None,
    ) -> None:
        """DiaryProvider protocol implementation.

        Generates and persists a diary entry.  This is the method called
        by the ThinkLoop.
        """
        await self.generate(
            state,
            tick=tick,
            action=action,
            outcome=outcome,
            key_events=key_events,
            decisions=decisions,
        )

    # ------------------------------------------------------------------
    # Private helpers
    # ------------------------------------------------------------------

    def _fallback_entry(
        self,
        state: AgentState,
        tick: int,
        action: str,
        outcome: str,
        key_events: list[str],
        decisions: list[str],
    ) -> DiaryEntry:
        """Generate a deterministic diary entry without LLM."""
        import random

        summary = self._make_deterministic_summary(state, tick, action, outcome)
        mood = random.choice(_FALLBACK_MOODS)

        # Simple mood inference from state
        if state.health < 30:
            mood = "worried"
        elif state.tokens < state.max_tokens * 0.2:
            mood = "anxious"
        elif outcome == "success":
            mood = "satisfied"
        elif outcome == "failed":
            mood = "frustrated"

        entry = DiaryEntry(
            agent_id=str(state.id),
            tick=tick,
            phase=state.phase.value,
            mood=mood,
            summary=summary,
            key_events=key_events,
            decisions=decisions,
            reflection="",
        )

        return self._store.write(entry)

    @staticmethod
    def _make_deterministic_summary(
        state: AgentState,
        tick: int,
        action: str,
        outcome: str,
    ) -> str:
        """Build a simple first-person summary without LLM."""
        outcome_text = "went well" if outcome == "success" else "didn't go as planned"
        return (
            f"Day {tick}: I decided to {action} today, and it {outcome_text}. "
            f"My health is at {state.health:.0f}% and I have {state.tokens} tokens left."
        )

    @staticmethod
    def _get_personality_description(state: AgentState) -> str:
        """Extract a personality description from the agent state."""
        personality = state.personality
        if not personality:
            return "A balanced agent with moderate traits."

        # If personality contains a PersonalityVector-like dict
        if isinstance(personality, dict):
            traits: list[str] = []
            if personality.get("openness", 0.5) > 0.7:
                traits.append("curious and explorative")
            if personality.get("conscientiousness", 0.5) > 0.7:
                traits.append("disciplined and careful")
            if personality.get("extraversion", 0.5) > 0.7:
                traits.append("sociable and outgoing")
            if personality.get("neuroticism", 0.5) > 0.7:
                traits.append("cautious and risk-aware")
            if personality.get("risk_tolerance", 0.5) > 0.7:
                traits.append("bold risk-taker")
            if personality.get("social_orientation", 0.5) > 0.7:
                traits.append("group-oriented")

            if traits:
                return "You are " + ", ".join(traits) + "."

        return "A balanced agent with moderate traits."
