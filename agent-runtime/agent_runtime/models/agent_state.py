from __future__ import annotations

import json
from typing import Any, Dict, List, Optional
from uuid import UUID, uuid4

from pydantic import BaseModel, ConfigDict, Field, field_validator

from .enums import AgentPhase, SurvivalMode
from .skill import Skill


class AgentState(BaseModel):
    """Core state model for an agent in the simulation.

    Tracks identity, resources, skills, personality, and provides
    synchronization with the World Engine.
    """

    model_config = ConfigDict(use_enum_values=False)

    id: UUID = Field(default_factory=uuid4, description="Unique agent identifier")
    name: str = Field(..., min_length=1, description="Agent display name")
    phase: AgentPhase = Field(
        default=AgentPhase.INITIALIZATION, description="Current lifecycle phase"
    )
    survival_mode: SurvivalMode = Field(
        default=SurvivalMode.CONSERVATION, description="Current survival strategy"
    )
    tokens: int = Field(default=100, ge=0, description="Token resource balance")
    money: float = Field(default=50.0, ge=0.0, description="Monetary balance")
    health: float = Field(
        default=100.0, ge=0.0, le=100.0, description="Health percentage (0-100)"
    )
    reputation: float = Field(
        default=0.0, ge=-100.0, le=100.0, description="Reputation score (-100 to 100)"
    )
    skills: Dict[str, Skill] = Field(
        default_factory=dict, description="Skill name -> Skill mapping"
    )
    personality: Dict[str, Any] = Field(
        default_factory=dict, description="Flexible personality traits"
    )
    world_sync_version: int = Field(
        default=0, description="Monotonic version counter for World Engine sync"
    )

    @field_validator("skills", mode="before")
    @classmethod
    def normalize_skills(cls, v):
        """Accept both list and dict input for skills."""
        if isinstance(v, list):
            return {s.name if isinstance(s, Skill) else s["name"]: s for s in v}
        return v

    # --- State mutation helpers ---

    def add_skill(self, skill: Skill) -> None:
        """Add or replace a skill."""
        self.skills[skill.name] = skill
        self._bump_version()

    def remove_skill(self, skill_name: str) -> Optional[Skill]:
        """Remove a skill by name. Returns the removed skill or None."""
        removed = self.skills.pop(skill_name, None)
        if removed is not None:
            self._bump_version()
        return removed

    def adjust_tokens(self, delta: int) -> None:
        """Adjust token balance (positive or negative)."""
        new_balance = self.tokens + delta
        if new_balance < 0:
            raise ValueError(
                f"Cannot reduce tokens below 0 (current: {self.tokens}, delta: {delta})"
            )
        self.tokens = new_balance
        self._bump_version()

    def adjust_money(self, delta: float) -> None:
        """Adjust money balance (positive or negative)."""
        new_balance = self.money + delta
        if new_balance < 0:
            raise ValueError(
                f"Cannot reduce money below 0 (current: {self.money}, delta: {delta})"
            )
        self.money = round(new_balance, 2)
        self._bump_version()

    def adjust_health(self, delta: float) -> None:
        """Adjust health, clamped to [0, 100]."""
        self.health = max(0.0, min(100.0, self.health + delta))
        self._bump_version()

    def adjust_reputation(self, delta: float) -> None:
        """Adjust reputation, clamped to [-100, 100]."""
        self.reputation = max(-100.0, min(100.0, self.reputation + delta))
        self._bump_version()

    def transition_phase(self, new_phase: AgentPhase) -> None:
        """Transition to a new lifecycle phase."""
        self.phase = new_phase
        self._bump_version()

    def set_survival_mode(self, mode: SurvivalMode) -> None:
        """Change the survival strategy."""
        self.survival_mode = mode
        self._bump_version()

    # --- World Engine sync ---

    def to_sync_payload(self) -> Dict[str, Any]:
        """Serialize state for sending to the World Engine.

        Returns a JSON-serializable dict with a sync version stamp.
        """
        payload = self.model_dump()
        payload["id"] = str(payload["id"])
        payload["phase"] = self.phase.value
        payload["survival_mode"] = self.survival_mode.value
        payload["skills"] = {
            name: skill.model_dump() for name, skill in self.skills.items()
        }
        return payload

    @classmethod
    def from_sync_payload(cls, data: Dict[str, Any]) -> AgentState:
        """Deserialize state received from the World Engine.

        Validates all fields and bumps the sync version.
        """
        return cls(**data)

    def apply_sync(self, remote: Dict[str, Any]) -> None:
        """Apply a remote state update from the World Engine.

        Uses last-writer-wins semantics: if the remote version is newer,
        all fields are overwritten.
        """
        remote_version = remote.get("world_sync_version", 0)
        if remote_version <= self.world_sync_version:
            return  # Local state is at least as fresh

        updated = AgentState.from_sync_payload(remote)
        for field_name in self.__class__.model_fields.keys():
            if field_name == "world_sync_version":
                continue
            setattr(self, field_name, getattr(updated, field_name))
        self.world_sync_version = remote_version

    # --- Serialization helpers ---

    def to_json(self) -> str:
        """Serialize to JSON string."""
        return self.model_dump_json()

    @classmethod
    def from_json(cls, data: str) -> AgentState:
        """Deserialize from JSON string."""
        return cls.model_validate_json(data)

    def _bump_version(self) -> None:
        """Increment the sync version counter on every state change."""
        self.world_sync_version += 1
