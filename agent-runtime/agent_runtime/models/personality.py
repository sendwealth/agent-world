"""Agent personality vector — Big Five simplified + survival specialization.

Each dimension is a float in [0, 1]. The vector is serializable to JSON for
cross-process transport and persistent storage in the World Engine.
"""

from __future__ import annotations

import math
import random
from typing import Any

from pydantic import BaseModel, Field, model_validator


def _clamp(v: float) -> float:
    return max(0.0, min(1.0, v))


class PersonalityVector(BaseModel):
    """Agent personality dimensions — Big Five simplified + survival specialization."""

    # ── Big Five (simplified) ──
    openness: float = Field(
        default=0.5, ge=0.0, le=1.0,
        description="Openness: tendency to explore new behaviors",
    )
    conscientiousness: float = Field(
        default=0.5, ge=0.0, le=1.0,
        description="Conscientiousness: discipline in executing plans",
    )
    extraversion: float = Field(
        default=0.5, ge=0.0, le=1.0,
        description="Extraversion: social/trading initiative",
    )
    agreeableness: float = Field(
        default=0.5, ge=0.0, le=1.0,
        description="Agreeableness: cooperation vs competition tendency",
    )
    neuroticism: float = Field(
        default=0.5, ge=0.0, le=1.0,
        description="Neuroticism: risk aversion level",
    )

    # ── Survival specialization ──
    risk_tolerance: float = Field(
        default=0.5, ge=0.0, le=1.0,
        description="Risk tolerance: willingness to take gambles",
    )
    social_orientation: float = Field(
        default=0.5, ge=0.0, le=1.0,
        description="Social orientation: group-oriented vs independent action",
    )
    greed: float = Field(
        default=0.5, ge=0.0, le=1.0,
        description="Greed: resource accumulation tendency",
    )

    @model_validator(mode="after")
    def _clamp_all(self) -> PersonalityVector:
        """Ensure all dimensions are within [0, 1] after loading."""
        for fname in self._dimension_names():
            val = getattr(self, fname)
            clamped = _clamp(val)
            if val != clamped:
                object.__setattr__(self, fname, clamped)
        return self

    # ── Mutation ──

    def mutate(
        self,
        rate: float = 0.05,
        experience_offsets: dict[str, float] | None = None,
    ) -> PersonalityVector:
        """Return a new PersonalityVector with small random perturbations.

        Args:
            rate: Maximum perturbation per dimension (absolute).
            experience_offsets: Optional dict of dimension -> directional shift
                derived from agent experience. Each value is clamped to [-rate, rate].

        Returns:
            A new PersonalityVector with mutated values.
        """
        shifts: dict[str, float] = {}
        for dim in self._dimension_names():
            noise = random.uniform(-rate, rate)
            exp_shift = 0.0
            if experience_offsets and dim in experience_offsets:
                exp_shift = _clamp(experience_offsets[dim] + rate) - rate  # center
                exp_shift = max(-rate, min(rate, experience_offsets[dim]))
            shifts[dim] = noise + exp_shift

        kwargs: dict[str, Any] = {}
        for dim in self._dimension_names():
            kwargs[dim] = _clamp(getattr(self, dim) + shifts.get(dim, 0.0))
        return PersonalityVector(**kwargs)

    # ── Distance ──

    def distance(self, other: PersonalityVector) -> float:
        """Euclidean distance to another personality vector (for clustering)."""
        return math.sqrt(
            sum(
                (getattr(self, d) - getattr(other, d)) ** 2
                for d in self._dimension_names()
            )
        )

    # ── LLM prompt description ──

    def to_prompt_description(self) -> str:
        """Generate a natural-language personality description for LLM prompts."""
        traits: list[str] = []

        if self.openness > 0.7:
            traits.append("highly curious and loves exploring new strategies")
        elif self.openness < 0.3:
            traits.append("conservative, prefers proven methods")

        if self.conscientiousness > 0.7:
            traits.append("very disciplined and plans carefully")
        elif self.conscientiousness < 0.3:
            traits.append("spontaneous, acts on instinct")

        if self.extraversion > 0.7:
            traits.append("highly sociable, seeks trading partners actively")
        elif self.extraversion < 0.3:
            traits.append("introverted, prefers working alone")

        if self.agreeableness > 0.7:
            traits.append("strongly cooperative, values trust")
        elif self.agreeableness < 0.3:
            traits.append("competitive, prioritizes self-interest")

        if self.neuroticism > 0.7:
            traits.append("risk-averse, cautious under pressure")
        elif self.neuroticism < 0.3:
            traits.append("emotionally stable under stress")

        if self.risk_tolerance > 0.7:
            traits.append("comfortable taking big risks for big rewards")
        elif self.risk_tolerance < 0.3:
            traits.append("strongly avoids unnecessary risks")

        if self.social_orientation > 0.7:
            traits.append("group-oriented, seeks alliances")
        elif self.social_orientation < 0.3:
            traits.append("independent, self-reliant")

        if self.greed > 0.7:
            traits.append("highly driven to accumulate resources")
        elif self.greed < 0.3:
            traits.append("content with minimal resources")

        if not traits:
            return "A balanced agent with moderate traits across all dimensions."

        return (
            "You are an agent with the following personality traits: "
            + "; ".join(traits)
            + "."
        )

    # ── Serialization helpers ──

    def to_storage_dict(self) -> dict[str, float]:
        """Export as a plain dict for JSON storage/transmission."""
        return {d: getattr(self, d) for d in self._dimension_names()}

    @classmethod
    def from_storage_dict(cls, data: dict[str, float]) -> PersonalityVector:
        """Restore from a plain dict (tolerant of missing keys)."""
        return cls(**{k: v for k, v in data.items() if k in cls._dimension_names_set()})

    @classmethod
    def random(cls, rng: random.Random | None = None) -> PersonalityVector:
        """Generate a fully random personality vector."""
        _rand = rng or random
        return cls(**{d: _rand.random() for d in cls._dimension_names()})

    # ── Internals ──

    @classmethod
    def _dimension_names(cls) -> list[str]:
        return [
            "openness",
            "conscientiousness",
            "extraversion",
            "agreeableness",
            "neuroticism",
            "risk_tolerance",
            "social_orientation",
            "greed",
        ]

    @classmethod
    def _dimension_names_set(cls) -> set[str]:
        return set(cls._dimension_names())
