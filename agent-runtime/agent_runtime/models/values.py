"""Agent value weights — dynamic behavior preference weights shaped by experience.

Values are floats in [0, 1] that influence decision-making. They evolve slowly
through experience events and naturally decay to prevent extreme polarization.
"""

from __future__ import annotations

from typing import Dict

from pydantic import BaseModel, Field


def _clamp(v: float) -> float:
    return max(0.0, min(1.0, v))


# Maximum single-event adjustment to prevent wild swings.
MAX_ADJUSTMENT = 0.05


class ValueWeights(BaseModel):
    """Agent value system — behavior preference weights adjusted by experience."""

    survival_priority: float = Field(
        default=0.5, ge=0.0, le=1.0,
        description="Survival priority (auto-rises when tokens are low)",
    )
    cooperation_weight: float = Field(
        default=0.5, ge=0.0, le=1.0,
        description="Cooperation preference (rises after successful cooperation)",
    )
    competition_weight: float = Field(
        default=0.5, ge=0.0, le=1.0,
        description="Competition preference (rises after betrayal)",
    )
    exploration_drive: float = Field(
        default=0.5, ge=0.0, le=1.0,
        description="Exploration drive (rises in new environments, falls in familiar ones)",
    )
    tradition_adherence: float = Field(
        default=0.3, ge=0.0, le=1.0,
        description="Tradition adherence (influenced by group culture)",
    )
    innovation_tendency: float = Field(
        default=0.3, ge=0.0, le=1.0,
        description="Innovation tendency (rises after successful exploration)",
    )

    # ── Experience-driven updates ──

    def update_from_experience(self, event_type: str, outcome: float) -> None:
        """Adjust value weights based on an experience event.

        Args:
            event_type: One of 'trade', 'cooperation', 'betrayal',
                'exploration', 'death_witness', 'survival_crisis'.
            outcome: Event outcome in [-1.0, +1.0].
                Positive = good outcome, negative = bad outcome.
        """
        delta = outcome * MAX_ADJUSTMENT  # scale by outcome (can be negative)

        if event_type == "cooperation":
            if outcome > 0:
                self.cooperation_weight = _clamp(self.cooperation_weight + delta)
                self.competition_weight = _clamp(self.competition_weight - delta * 0.5)
            else:
                self.cooperation_weight = _clamp(self.cooperation_weight - delta)

        elif event_type == "betrayal":
            if outcome < 0:
                self.competition_weight = _clamp(self.competition_weight + abs(delta))
                self.cooperation_weight = _clamp(self.cooperation_weight - abs(delta) * 0.5)
            else:
                # Agent betrayed someone else and gained — may feel emboldened
                self.competition_weight = _clamp(self.competition_weight + delta * 0.5)

        elif event_type == "trade":
            if outcome > 0:
                self.cooperation_weight = _clamp(self.cooperation_weight + delta * 0.3)
            else:
                self.competition_weight = _clamp(self.competition_weight + abs(delta) * 0.3)

        elif event_type == "exploration":
            if outcome > 0:
                self.exploration_drive = _clamp(self.exploration_drive + delta)
                self.innovation_tendency = _clamp(self.innovation_tendency + delta * 0.5)
            else:
                self.exploration_drive = _clamp(self.exploration_drive - delta * 0.3)

        elif event_type == "death_witness":
            # Witnessing death increases survival priority
            self.survival_priority = _clamp(
                self.survival_priority + abs(delta)
            )

        elif event_type == "survival_crisis":
            self.survival_priority = _clamp(
                self.survival_priority + abs(delta)
            )
            self.exploration_drive = _clamp(self.exploration_drive - abs(delta) * 0.3)

    # ── Natural decay ──

    def decay(self, rate: float = 0.01) -> None:
        """Apply natural decay toward the neutral midpoint (0.5) for most weights.

        Survival_priority is exempt — it should only change through events.
        Tradition_adherence decays toward 0 (traditions fade without reinforcement).

        Args:
            rate: Decay rate per tick (small, e.g. 0.01).
        """
        for dim in self._adjustable_dimension_names():
            current = getattr(self, dim)
            # Decay toward 0.5 midpoint
            midpoint = 0.5
            diff = current - midpoint
            new_val = current - diff * rate
            object.__setattr__(self, dim, _clamp(new_val))

    # ── Serialization ──

    def to_storage_dict(self) -> Dict[str, float]:
        """Export as a plain dict for JSON storage/transmission."""
        return {d: getattr(self, d) for d in self._dimension_names()}

    @classmethod
    def from_storage_dict(cls, data: Dict[str, float]) -> ValueWeights:
        """Restore from a plain dict (tolerant of missing keys)."""
        names = cls._dimension_names_set()
        return cls(**{k: v for k, v in data.items() if k in names})

    # ── LLM summary ──

    def to_prompt_summary(self) -> str:
        """Generate a natural-language summary of current values for LLM prompts."""
        parts: list[str] = []

        if self.survival_priority > 0.7:
            parts.append("survival is your top priority right now")
        if self.cooperation_weight > 0.7:
            parts.append("you strongly prefer working with others")
        elif self.competition_weight > 0.7:
            parts.append("you prefer competing over cooperating")
        if self.exploration_drive > 0.7:
            parts.append("you are highly driven to explore the unknown")
        if self.tradition_adherence > 0.7:
            parts.append("you respect established traditions and norms")
        if self.innovation_tendency > 0.7:
            parts.append("you are drawn to trying new approaches")

        if not parts:
            return "Your values are balanced with no strong bias."
        return "Current value tendencies: " + "; ".join(parts) + "."

    # ── Internals ──

    @classmethod
    def _dimension_names(cls) -> list[str]:
        return [
            "survival_priority",
            "cooperation_weight",
            "competition_weight",
            "exploration_drive",
            "tradition_adherence",
            "innovation_tendency",
        ]

    @classmethod
    def _dimension_names_set(cls) -> set[str]:
        return set(cls._dimension_names())

    @classmethod
    def _adjustable_dimension_names(cls) -> list[str]:
        """Dimensions that decay toward midpoint. Survival_priority is excluded."""
        return [
            "cooperation_weight",
            "competition_weight",
            "exploration_drive",
            "tradition_adherence",
            "innovation_tendency",
        ]
