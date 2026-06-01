"""Emotion engine — transforms events into emotional states.

The EmotionEngine maintains an agent's emotional state and updates it in response
to game events. It accounts for:

1. **Event-emotion mapping**: each event type produces a baseline emotional response.
2. **Personality modulation**: neuroticism amplifies negative reactions, extraversion
   amplifies positive reactions.
3. **Temporal decay**: emotions naturally regress toward a personality-derived baseline
   over time.

Also provides ``ThinkLoopEmotionHook``, an adapter that implements the ThinkLoop's
``EmotionHook`` protocol, bridging action results to emotion updates.

Usage::

    from agent_runtime.emotion.engine import EmotionEngine
    from agent_runtime.models.personality import PersonalityVector

    personality = PersonalityVector(neuroticism=0.7, extraversion=0.6)
    engine = EmotionEngine(personality=personality)

    # Event-driven update
    state = engine.update("earned_money", {"amount": 50})
    print(state.primary_emotion)  # EmotionType.HAPPY
    print(state.valence)          # ~0.3

    # Time-based decay
    state = engine.decay(ticks_elapsed=5)
    print(state.valence)          # closer to baseline

    # LLM context injection
    print(engine.get_mood_description())  # natural-language description
"""

from __future__ import annotations

import logging
import math
from dataclasses import dataclass
from typing import Any

from agent_runtime.emotion.mood import EmotionalState, EmotionType, _clamp_unit, _clamp_valence
from agent_runtime.models.personality import PersonalityVector

logger = logging.getLogger(__name__)


# ---------------------------------------------------------------------------
# Event → emotion response mapping
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class EmotionResponse:
    """Baseline emotional response to an event type.

    Attributes:
        valence_delta: Change in pleasure (-1 to +1).
        arousal_delta: Change in activation (0 to +1).
        dominance_delta: Change in control feeling (-1 to +1).
        primary_emotion: The emotion this event tends to elicit.
        secondary_emotion: Optional secondary emotion.
        intensity_base: Base intensity of this response (0 to 1).
    """

    valence_delta: float
    arousal_delta: float
    dominance_delta: float
    primary_emotion: EmotionType
    secondary_emotion: EmotionType | None = None
    intensity_base: float = 0.5


# Mapping of event types to their baseline emotional responses.
# These are calibrated to produce moderate shifts that personality can amplify.
_EVENT_RESPONSES: dict[str, EmotionResponse] = {
    "earned_money": EmotionResponse(
        valence_delta=0.3,
        arousal_delta=0.2,
        dominance_delta=0.1,
        primary_emotion=EmotionType.HAPPY,
        intensity_base=0.5,
    ),
    "attacked": EmotionResponse(
        valence_delta=-0.4,
        arousal_delta=0.5,
        dominance_delta=-0.3,
        primary_emotion=EmotionType.FEARFUL,
        secondary_emotion=EmotionType.ANGRY,
        intensity_base=0.7,
    ),
    "social_success": EmotionResponse(
        valence_delta=0.2,
        arousal_delta=0.15,
        dominance_delta=0.1,
        primary_emotion=EmotionType.HAPPY,
        intensity_base=0.4,
    ),
    "resource_loss": EmotionResponse(
        valence_delta=-0.2,
        arousal_delta=0.15,
        dominance_delta=-0.1,
        primary_emotion=EmotionType.ANXIOUS,
        intensity_base=0.4,
    ),
    "oracle_received": EmotionResponse(
        valence_delta=0.1,
        arousal_delta=0.25,
        dominance_delta=0.05,
        primary_emotion=EmotionType.SURPRISED,
        secondary_emotion=EmotionType.CALM,
        intensity_base=0.3,
    ),
    "bounty_completed": EmotionResponse(
        valence_delta=0.35,
        arousal_delta=0.3,
        dominance_delta=0.2,
        primary_emotion=EmotionType.HAPPY,
        secondary_emotion=EmotionType.SURPRISED,
        intensity_base=0.6,
    ),
    "trade_success": EmotionResponse(
        valence_delta=0.25,
        arousal_delta=0.1,
        dominance_delta=0.15,
        primary_emotion=EmotionType.HAPPY,
        intensity_base=0.45,
    ),
    "trade_failure": EmotionResponse(
        valence_delta=-0.15,
        arousal_delta=0.1,
        dominance_delta=-0.1,
        primary_emotion=EmotionType.DISGUSTED,
        secondary_emotion=EmotionType.SAD,
        intensity_base=0.35,
    ),
    "cooperation": EmotionResponse(
        valence_delta=0.2,
        arousal_delta=0.1,
        dominance_delta=0.1,
        primary_emotion=EmotionType.HAPPY,
        intensity_base=0.4,
    ),
    "betrayal": EmotionResponse(
        valence_delta=-0.35,
        arousal_delta=0.4,
        dominance_delta=-0.25,
        primary_emotion=EmotionType.ANGRY,
        secondary_emotion=EmotionType.SAD,
        intensity_base=0.65,
    ),
    "exploration_success": EmotionResponse(
        valence_delta=0.2,
        arousal_delta=0.2,
        dominance_delta=0.1,
        primary_emotion=EmotionType.SURPRISED,
        secondary_emotion=EmotionType.HAPPY,
        intensity_base=0.45,
    ),
    "death_witness": EmotionResponse(
        valence_delta=-0.3,
        arousal_delta=0.3,
        dominance_delta=-0.2,
        primary_emotion=EmotionType.SAD,
        secondary_emotion=EmotionType.FEARFUL,
        intensity_base=0.6,
    ),
    "survival_crisis": EmotionResponse(
        valence_delta=-0.25,
        arousal_delta=0.45,
        dominance_delta=-0.3,
        primary_emotion=EmotionType.ANXIOUS,
        secondary_emotion=EmotionType.FEARFUL,
        intensity_base=0.7,
    ),
    "rest": EmotionResponse(
        valence_delta=0.05,
        arousal_delta=-0.1,
        dominance_delta=0.0,
        primary_emotion=EmotionType.CALM,
        intensity_base=0.1,
    ),
}

# Decay rate per tick — how quickly emotion regresses toward baseline.
# Lower values = longer-lasting emotions.
_DECAY_RATE = 0.08

# Maximum number of trigger events to retain in the state.
_MAX_TRIGGERS = 10


class EmotionEngine:
    """Maintains and updates an agent's emotional state.

    The engine applies event-driven updates, personality modulation, and
    temporal decay. It is designed to be called once per tick from the
    ThinkLoop.

    Usage::

        engine = EmotionEngine(personality=personality_vector)
        state = engine.update("earned_money", {"amount": 50})
        desc = engine.get_mood_description()
    """

    def __init__(
        self,
        personality: PersonalityVector,
        *,
        initial_state: EmotionalState | None = None,
        decay_rate: float = _DECAY_RATE,
    ) -> None:
        self._personality = personality
        self._state = initial_state or self.get_baseline()
        self._decay_rate = decay_rate

    @property
    def state(self) -> EmotionalState:
        """Current emotional state (read-only snapshot)."""
        return self._state

    @property
    def personality(self) -> PersonalityVector:
        """Agent personality vector (affects emotion modulation)."""
        return self._personality

    def get_baseline(self) -> EmotionalState:
        """Compute the baseline emotional state from personality traits.

        Personality → baseline mapping:
        - neuroticism: baseline arousal ↑, baseline valence ↓
        - extraversion: baseline valence ↑
        - agreeableness: baseline calm dominance ↑
        - risk_tolerance: baseline dominance ↑
        """
        p = self._personality

        baseline_valence = 0.1 * (p.extraversion - p.neuroticism)
        baseline_arousal = 0.2 + 0.3 * p.neuroticism
        baseline_dominance = 0.3 + 0.3 * p.risk_tolerance + 0.2 * p.agreeableness

        # Determine baseline emotion type
        if baseline_valence > 0.1:
            baseline_emotion = EmotionType.CALM
        elif baseline_valence < -0.1:
            baseline_emotion = EmotionType.ANXIOUS
        else:
            baseline_emotion = EmotionType.CALM

        return EmotionalState(
            valence=baseline_valence,
            arousal=baseline_arousal,
            dominance=baseline_dominance,
            primary_emotion=baseline_emotion,
            intensity=0.0,
        )

    def update(
        self,
        event: str,
        context: dict[str, Any] | None = None,
    ) -> EmotionalState:
        """Update emotional state based on a game event.

        Applies:
        1. Baseline emotional response from event-emotion mapping.
        2. Personality modulation (neuroticism / extraversion scaling).
        3. Context intensity adjustment (optional).

        Args:
            event: Event type string (e.g. "earned_money", "attacked").
            context: Optional event context for intensity adjustments.

        Returns:
            The updated EmotionalState.
        """
        response = _EVENT_RESPONSES.get(event)

        if response is None:
            # Unknown event — small perturbation toward curiosity/surprise
            logger.debug("Unknown emotion event: %s", event)
            response = EmotionResponse(
                valence_delta=0.0,
                arousal_delta=0.05,
                dominance_delta=0.0,
                primary_emotion=EmotionType.SURPRISED,
                intensity_base=0.1,
            )

        # Personality modulation factors
        valence_scale = self._compute_valence_scale(response.valence_delta)
        arousal_scale = self._compute_arousal_scale()
        intensity_scale = self._compute_intensity_scale(response.valence_delta)

        # Apply scaled deltas
        new_valence = _clamp_valence(
            self._state.valence + response.valence_delta * valence_scale
        )
        new_arousal = _clamp_unit(
            self._state.arousal + response.arousal_delta * arousal_scale
        )
        new_dominance = _clamp_unit(
            self._state.dominance + response.dominance_delta
        )

        # Compute intensity
        context_multiplier = self._context_intensity(context)
        new_intensity = _clamp_unit(
            response.intensity_base * intensity_scale * context_multiplier
        )

        # Update triggers
        new_triggers = list(self._state.triggers[-(_MAX_TRIGGERS - 1):]) + [event]

        # Determine emotions
        primary = response.primary_emotion
        secondary = response.secondary_emotion

        # If valence flipped significantly, adjust primary
        if new_valence > 0.2 and primary not in (EmotionType.HAPPY, EmotionType.SURPRISED):
            secondary = primary
            primary = EmotionType.HAPPY
        elif new_valence < -0.2 and primary in (EmotionType.HAPPY,):
            secondary = primary
            primary = EmotionType.SAD

        self._state = EmotionalState(
            valence=new_valence,
            arousal=new_arousal,
            dominance=new_dominance,
            primary_emotion=primary,
            secondary_emotion=secondary,
            triggers=new_triggers,
            intensity=new_intensity,
        )

        logger.debug(
            "Emotion update: event=%s valence=%.2f arousal=%.2f emotion=%s intensity=%.2f",
            event,
            self._state.valence,
            self._state.arousal,
            self._state.primary_emotion.value,
            self._state.intensity,
        )

        return self._state

    def decay(self, ticks_elapsed: int = 1) -> EmotionalState:
        """Apply temporal decay — emotion regresses toward baseline.

        Each tick, the current state moves a fraction toward the baseline.
        The decay follows exponential decay: remaining = (1 - rate) ^ ticks.

        Args:
            ticks_elapsed: Number of ticks to decay over.

        Returns:
            The decayed EmotionalState.
        """
        if ticks_elapsed <= 0:
            return self._state

        baseline = self.get_baseline()
        factor = (1.0 - self._decay_rate) ** ticks_elapsed

        new_valence = _clamp_valence(
            baseline.valence + (self._state.valence - baseline.valence) * factor
        )
        new_arousal = _clamp_unit(
            baseline.arousal + (self._state.arousal - baseline.arousal) * factor
        )
        new_dominance = _clamp_unit(
            baseline.dominance + (self._state.dominance - baseline.dominance) * factor
        )
        new_intensity = _clamp_unit(self._state.intensity * factor)

        # Determine if emotion label should revert
        primary = self._state.primary_emotion
        if new_intensity < 0.1:
            primary = baseline.primary_emotion

        self._state = EmotionalState(
            valence=new_valence,
            arousal=new_arousal,
            dominance=new_dominance,
            primary_emotion=primary,
            secondary_emotion=self._state.secondary_emotion if new_intensity > 0.15 else None,
            triggers=self._state.triggers,
            intensity=new_intensity,
        )

        return self._state

    def get_mood_description(self) -> str:
        """Natural-language description of the current mood for LLM prompts."""
        return self._state.to_prompt_description()

    def to_prompt_description(self) -> str:
        """Alias for get_mood_description — matches the pattern used by PersonalityVector."""
        return self.get_mood_description()

    # ------------------------------------------------------------------
    # Personality modulation helpers
    # ------------------------------------------------------------------

    def _compute_valence_scale(self, valence_delta: float) -> float:
        """Personality-based scaling of valence changes.

        - neuroticism amplifies negative valence shifts.
        - extraversion amplifies positive valence shifts.
        """
        scale = 1.0
        if valence_delta < 0:
            # Negative event: neuroticism amplifies
            scale *= 1.0 + 0.5 * self._personality.neuroticism
        else:
            # Positive event: extraversion amplifies
            scale *= 1.0 + 0.3 * self._personality.extraversion
        return scale

    def _compute_arousal_scale(self) -> float:
        """Personality-based scaling of arousal changes.

        neuroticism amplifies arousal (more reactive).
        """
        return 1.0 + 0.3 * self._personality.neuroticism

    def _compute_intensity_scale(self, valence_delta: float) -> float:
        """Personality-based scaling of overall intensity."""
        if valence_delta < 0:
            return 1.0 + 0.4 * self._personality.neuroticism
        return 1.0 + 0.2 * self._personality.extraversion

    @staticmethod
    def _context_intensity(context: dict[str, Any] | None) -> float:
        """Derive an intensity multiplier from event context.

        Looks for magnitude-related fields in the context dict.
        """
        if not context:
            return 1.0

        # Scale by amount if present
        amount = context.get("amount") or context.get("magnitude")
        if amount is not None:
            try:
                amount = float(amount)
                # Sigmoid-like scaling: small amounts → ~0.5x, large amounts → ~1.5x
                return max(0.3, min(1.5, 0.5 + 0.5 * math.tanh(amount / 100.0)))
            except (TypeError, ValueError):
                pass

        # Scale by severity if present
        severity = context.get("severity")
        if severity is not None:
            try:
                severity = float(severity)
                return max(0.3, min(1.5, 0.3 + 0.7 * _clamp_unit(severity)))
            except (TypeError, ValueError):
                pass

        return 1.0

    # ------------------------------------------------------------------
    # Serialization
    # ------------------------------------------------------------------

    def get_state_dict(self) -> dict[str, Any]:
        """Export current state for storage or dashboard."""
        return self._state.to_storage_dict()

    def load_state_dict(self, data: dict[str, Any]) -> None:
        """Restore emotional state from a stored dict."""
        self._state = EmotionalState.from_storage_dict(data)


# ---------------------------------------------------------------------------
# ThinkLoop adapter
# ---------------------------------------------------------------------------

# Mapping from ThinkLoop ActionType values to emotion event types.
# This bridges the action execution layer to the emotion engine.
_ACTION_TO_EMOTION_EVENT: dict[str, str] = {
    "gather": "earned_money",
    "claim_task": "earned_money",
    "explore": "exploration_success",
    "trade": "trade_success",
    "propose_deal": "trade_success",
    "socialize": "social_success",
    "send_message": "social_success",
    "rest": "rest",
    "build": "bounty_completed",
    "move": "exploration_success",
}


class ThinkLoopEmotionHook:
    """Adapter that connects EmotionEngine to the ThinkLoop's EmotionHook protocol.

    Translates action results into emotion events and applies decay.

    Usage::

        from agent_runtime.emotion.engine import EmotionEngine, ThinkLoopEmotionHook

        engine = EmotionEngine(personality=personality)
        hook = ThinkLoopEmotionHook(engine)

        loop = ThinkLoop(..., emotion_hook=hook)
    """

    def __init__(self, engine: EmotionEngine) -> None:
        self._engine = engine

    @property
    def engine(self) -> EmotionEngine:
        """Access the underlying EmotionEngine."""
        return self._engine

    def update_from_action(
        self,
        action_type: str,
        status: str,
        context: dict[str, Any] | None,
    ) -> None:
        """Translate an action result into an emotion event.

        Maps action types to emotion events and only triggers updates
        for successful actions (failures are logged but don't shift emotion
        strongly to avoid negative feedback loops).
        """
        if status == "success":
            event = _ACTION_TO_EMOTION_EVENT.get(action_type)
            if event:
                self._engine.update(event, context)
            else:
                # Unknown successful action — mild positive surprise
                self._engine.update(action_type, context)
        elif status in ("failed", "error"):
            # Failed actions produce mild negative response
            self._engine.update("resource_loss", context)

    def decay(self, ticks_elapsed: int) -> None:
        """Apply temporal emotion decay."""
        self._engine.decay(ticks_elapsed)

    def get_mood_description(self) -> str:
        """Get natural-language mood description."""
        return self._engine.get_mood_description()
