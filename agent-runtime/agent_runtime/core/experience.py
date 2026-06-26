"""Agent experience recording and accumulation.

The ExperienceAccumulator bridges experiences (events) to personality
mutations and value-weight adjustments. It maintains a bounded history
and supports relevance-based retrieval for LLM context injection.
"""

from __future__ import annotations

from typing import Any

from pydantic import BaseModel, Field

from agent_runtime.models.personality import PersonalityVector
from agent_runtime.models.values import ValueWeights


class Experience(BaseModel):
    """A single agent experience record."""

    tick: int = Field(..., ge=0, description="World tick when the event occurred")
    event_type: str = Field(
        ...,
        description="Event category: trade | cooperation | betrayal | exploration | death_witness",
    )
    partner_id: str | None = Field(
        default=None, description="ID of the other agent involved (if any)"
    )
    outcome: float = Field(
        ..., ge=-1.0, le=1.0,
        description="Event outcome: -1.0 (severe loss) → +1.0 (huge gain)",
    )
    context: dict[str, Any] = Field(
        default_factory=dict,
        description="Event context (resource state, environment, etc.)",
    )
    learned: str = Field(
        default="",
        description="Agent's natural-language summary of the experience",
    )


# Valid event types for experience-driven updates.
_VALID_EVENT_TYPES = {
    "trade",
    "cooperation",
    "betrayal",
    "exploration",
    "death_witness",
    "survival_crisis",
}

# Default maximum number of experiences to retain.
DEFAULT_MAX_HISTORY = 200


class ExperienceAccumulator:
    """Accumulates experiences and translates them into personality/value changes.

    The accumulator maintains a bounded history and applies small adjustments
    to the agent's personality vector and value weights on each recorded event.
    """

    def __init__(
        self,
        personality: PersonalityVector,
        values: ValueWeights,
        max_history: int = DEFAULT_MAX_HISTORY,
    ) -> None:
        self.personality = personality
        self.values = values
        self._history: list[Experience] = []
        self._max_history = max_history

    # ── Recording ──

    def record(self, experience: Experience) -> None:
        """Record an experience and trigger personality/value micro-adjustments.

        Steps:
        1. Append to bounded history.
        2. Update value weights from event.
        3. Compute a small personality mutation offset based on event outcome.
        """
        self._history.append(experience)
        if len(self._history) > self._max_history:
            self._history = self._history[-self._max_history :]

        # Update values
        self.values.update_from_experience(experience.event_type, experience.outcome)

        # Compute personality offsets from outcome
        offsets = self._compute_personality_offsets(experience)
        self.personality = self.personality.mutate(
            rate=0.02,
            experience_offsets=offsets,
        )

    # ── Retrieval ──

    def get_relevant_experiences(
        self,
        current_context: dict[str, Any],
        top_k: int = 5,
    ) -> list[Experience]:
        """Retrieve the most relevant historical experiences for the current context.

        Relevance is computed as a simple heuristic:
        - Same event_type → +2 score
        - Similar outcome direction (both positive or both negative) → +1
        - Recency bonus proportional to position in history
        - Same partner_id → +1

        Returns up to `top_k` experiences sorted by relevance descending.
        """
        if not self._history:
            return []

        ctx_event_type = current_context.get("event_type", "")
        ctx_outcome = current_context.get("outcome", 0.0)
        ctx_partner = current_context.get("partner_id")

        scored: list[tuple[float, Experience]] = []
        total = len(self._history)
        for idx, exp in enumerate(self._history):
            score = 0.0

            # Event type match
            if exp.event_type == ctx_event_type:
                score += 2.0

            # Outcome direction match
            if (exp.outcome >= 0) == (ctx_outcome >= 0):
                score += 1.0

            # Partner match
            if ctx_partner and exp.partner_id == ctx_partner:
                score += 1.0

            # Recency bonus (newer = higher)
            recency = (idx + 1) / total
            score += recency

            scored.append((score, exp))

        scored.sort(key=lambda pair: pair[0], reverse=True)
        return [exp for _, exp in scored[:top_k]]

    # ── Snapshot ──

    def get_personality_snapshot(self) -> dict[str, Any]:
        """Export current personality state for Dashboard display."""
        return {
            "personality": self.personality.to_storage_dict(),
            "values": self.values.to_storage_dict(),
            "experience_count": len(self._history),
            "prompt_description": self.personality.to_prompt_description(),
            "value_summary": self.values.to_prompt_summary(),
        }

    # ── History access ──

    @property
    def history(self) -> list[Experience]:
        """Read-only access to the full experience history."""
        return list(self._history)

    @property
    def experience_count(self) -> int:
        return len(self._history)

    # ── Internals ──

    def _compute_personality_offsets(
        self, experience: Experience
    ) -> dict[str, float]:
        """Compute directional personality shifts based on an experience.

        Returns a dict of dimension -> small offset (typically ±0.02).
        """
        offsets: dict[str, float] = {}
        outcome = experience.outcome

        if experience.event_type == "exploration":
            # Successful exploration → more openness; failure → less
            offsets["openness"] = 0.02 if outcome > 0 else -0.02

        elif experience.event_type == "cooperation":
            # Good cooperation → more agreeableness and social orientation
            if outcome > 0:
                offsets["agreeableness"] = 0.02
                offsets["social_orientation"] = 0.01
            else:
                offsets["agreeableness"] = -0.02

        elif experience.event_type == "betrayal":
            # Betrayal → less agreeableness, more neuroticism
            offsets["agreeableness"] = -0.03
            offsets["neuroticism"] = 0.02
            offsets["risk_tolerance"] = -0.02

        elif experience.event_type == "trade":
            # Successful trading → more extraversion
            if outcome > 0:
                offsets["extraversion"] = 0.01
            else:
                offsets["greed"] = -0.01

        elif experience.event_type == "death_witness":
            # Witnessing death → more neuroticism, less risk tolerance
            offsets["neuroticism"] = 0.03
            offsets["risk_tolerance"] = -0.03

        elif experience.event_type == "survival_crisis":
            offsets["neuroticism"] = 0.02
            offsets["greed"] = 0.02  # scarcity → more hoarding instinct

        return offsets
