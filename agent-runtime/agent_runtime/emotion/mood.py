"""Emotional state model — PAD-based affective state with discrete emotion labels.

Represents an agent's emotional state using a simplified PAD (Pleasure-Arousal-Dominance)
model with discrete primary/secondary emotion labels. Designed for integration with
the agent's personality vector and think-loop cycle.

Emotional dimensions:
    valence:   -1.0 (very negative) → +1.0 (very positive)
    arousal:    0.0 (calm/sleepy)  →  1.0 (highly activated)
    dominance:  0.0 (submissive)   →  1.0 (dominant/in-control)
"""

from __future__ import annotations

from enum import StrEnum
from typing import Any

from pydantic import BaseModel, Field, model_validator


class EmotionType(StrEnum):
    """Discrete emotion categories mapped from PAD space."""

    HAPPY = "happy"
    SAD = "sad"
    ANGRY = "angry"
    FEARFUL = "fearful"
    SURPRISED = "surprised"
    DISGUSTED = "disgusted"
    CALM = "calm"
    ANXIOUS = "anxious"


def _clamp_valence(v: float) -> float:
    return max(-1.0, min(1.0, v))


def _clamp_unit(v: float) -> float:
    return max(0.0, min(1.0, v))


class EmotionalState(BaseModel):
    """Agent emotional state using PAD dimensions + discrete labels.

    Attributes:
        valence: Pleasure dimension, -1.0 (negative) to +1.0 (positive).
        arousal: Activation level, 0.0 (calm) to 1.0 (highly aroused).
        dominance: Control feeling, 0.0 (submissive) to 1.0 (dominant).
        primary_emotion: Dominant discrete emotion label.
        secondary_emotion: Secondary emotion, if mixed.
        triggers: List of event types that contributed to this state.
        intensity: Overall intensity of the emotional state, 0.0 to 1.0.
    """

    valence: float = Field(
        default=0.0, ge=-1.0, le=1.0,
        description="Pleasure: -1.0 (very negative) to +1.0 (very positive)",
    )
    arousal: float = Field(
        default=0.3, ge=0.0, le=1.0,
        description="Arousal: 0.0 (calm) to 1.0 (highly activated)",
    )
    dominance: float = Field(
        default=0.5, ge=0.0, le=1.0,
        description="Dominance: 0.0 (submissive) to 1.0 (in-control)",
    )
    primary_emotion: EmotionType = Field(
        default=EmotionType.CALM,
        description="Dominant discrete emotion",
    )
    secondary_emotion: EmotionType | None = Field(
        default=None,
        description="Secondary emotion if mixed",
    )
    triggers: list[str] = Field(
        default_factory=list,
        description="Event types that contributed to this state",
    )
    intensity: float = Field(
        default=0.0, ge=0.0, le=1.0,
        description="Overall emotional intensity",
    )

    @model_validator(mode="after")
    def _clamp_all(self) -> EmotionalState:
        """Ensure all dimensions are within bounds after loading."""
        val = _clamp_valence(self.valence)
        if val != self.valence:
            object.__setattr__(self, "valence", val)
        aro = _clamp_unit(self.arousal)
        if aro != self.arousal:
            object.__setattr__(self, "arousal", aro)
        dom = _clamp_unit(self.dominance)
        if dom != self.dominance:
            object.__setattr__(self, "dominance", dom)
        itn = _clamp_unit(self.intensity)
        if itn != self.intensity:
            object.__setattr__(self, "intensity", itn)
        return self

    def to_prompt_description(self) -> str:
        """Generate a natural-language description of the emotional state for LLM prompts.

        Returns a concise description suitable for injecting into the decision prompt,
        helping the LLM make mood-appropriate decisions.
        """
        if self.intensity < 0.05:
            return "You feel emotionally neutral and balanced."

        parts: list[str] = []

        # Primary emotion description
        emotion_desc = _EMOTION_DESCRIPTIONS.get(self.primary_emotion, "neutral")
        parts.append(f"You feel {emotion_desc}")

        # Add intensity qualifier
        if self.intensity > 0.7:
            parts.append("intensely")
        elif self.intensity > 0.4:
            parts.append("moderately")

        # Secondary emotion
        if self.secondary_emotion is not None and self.secondary_emotion != self.primary_emotion:
            sec_desc = _EMOTION_DESCRIPTIONS.get(self.secondary_emotion, "neutral")
            parts.append(f"with undertones of {sec_desc}")

        # Behavioral tendency
        tendency = self._get_behavioral_tendency()
        if tendency:
            parts.append(tendency)

        base = " ".join(parts) + "."

        # Add trigger context if available (last 3)
        if self.triggers:
            recent = self.triggers[-3:]
            base += f" Recent emotional triggers: {', '.join(recent)}."

        return base

    def _get_behavioral_tendency(self) -> str:
        """Infer a behavioral tendency from the current emotional state."""
        if self.valence > 0.3 and self.arousal > 0.5:
            return "You are inclined toward bold, decisive action"
        if self.valence < -0.3 and self.arousal > 0.5:
            return "You are inclined toward cautious, defensive behavior"
        if self.valence < -0.3 and self.arousal < 0.3:
            return "You feel withdrawn and prefer to avoid risk"
        if self.valence > 0.3 and self.arousal < 0.3:
            return "You feel content and satisfied"
        return ""

    def to_storage_dict(self) -> dict[str, Any]:
        """Export as a plain dict for JSON storage/transmission."""
        return {
            "valence": self.valence,
            "arousal": self.arousal,
            "dominance": self.dominance,
            "primary_emotion": self.primary_emotion.value,
            "secondary_emotion": self.secondary_emotion.value if self.secondary_emotion else None,
            "triggers": list(self.triggers),
            "intensity": self.intensity,
        }

    @classmethod
    def from_storage_dict(cls, data: dict[str, Any]) -> EmotionalState:
        """Restore from a plain dict (tolerant of missing keys)."""
        if isinstance(data.get("primary_emotion"), str):
            data["primary_emotion"] = EmotionType(data["primary_emotion"])
        if isinstance(data.get("secondary_emotion"), str):
            data["secondary_emotion"] = EmotionType(data["secondary_emotion"])
        return cls(**{k: v for k, v in data.items() if k in cls.model_fields})

    @classmethod
    def neutral(cls) -> EmotionalState:
        """Return a calm, neutral emotional baseline."""
        return cls(
            valence=0.0,
            arousal=0.3,
            dominance=0.5,
            primary_emotion=EmotionType.CALM,
            intensity=0.0,
        )


# Emotion → natural language description
_EMOTION_DESCRIPTIONS: dict[EmotionType, str] = {
    EmotionType.HAPPY: "happy and optimistic",
    EmotionType.SAD: "sad and melancholic",
    EmotionType.ANGRY: "angry and frustrated",
    EmotionType.FEARFUL: "fearful and on edge",
    EmotionType.SURPRISED: "surprised and startled",
    EmotionType.DISGUSTED: "disgusted and repulsed",
    EmotionType.CALM: "calm and composed",
    EmotionType.ANXIOUS: "anxious and uneasy",
}
