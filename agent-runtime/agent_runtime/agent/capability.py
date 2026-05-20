"""Agent capability declaration standard.

Standardized capability declarations for external agents connecting to
the Agent World platform.  Every external agent must declare its
capabilities at registration time so the World Engine can validate
actions and provide appropriate perception data.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any


# ── Valid action types (aligned with Rust ALLOWED_ACTIONS) ──────────

VALID_ACTIONS: frozenset[str] = frozenset({
    "move",
    "gather",
    "trade",
    "communicate",
    "explore",
    "rest",
    "build",
    "claim_task",
    "submit_task",
})


@dataclass
class AgentCapability:
    """Standardized agent capability declaration.

    Attributes:
        name: Human-readable capability name (e.g. "navigation").
        version: Semver string for the capability version.
        actions: List of action types this capability enables.
        perception_range: How far the agent can perceive (in tiles).
        max_actions_per_tick: Maximum actions the agent can perform per tick.
    """

    name: str
    version: str = "1.0.0"
    actions: list[str] = field(default_factory=list)
    perception_range: int = 3
    max_actions_per_tick: int = 1

    # ── Factory methods ──────────────────────────────────────────

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> AgentCapability:
        """Create from a dictionary (e.g. parsed JSON)."""
        return cls(
            name=data.get("name", ""),
            version=data.get("version", "1.0.0"),
            actions=data.get("actions", []),
            perception_range=data.get("perception_range", 3),
            max_actions_per_tick=data.get("max_actions_per_tick", 1),
        )

    # ── Validation ───────────────────────────────────────────────

    def validate(self) -> list[str]:
        """Validate the capability declaration.

        Returns:
            List of error messages.  Empty list means valid.
        """
        errors: list[str] = []

        if not self.name:
            errors.append("capability name is required")

        if not self.actions:
            errors.append("at least one action must be declared")

        for action in self.actions:
            if action not in VALID_ACTIONS:
                errors.append(
                    f"unknown action '{action}'; valid: {sorted(VALID_ACTIONS)}"
                )

        if self.perception_range < 1:
            errors.append("perception_range must be >= 1")

        if self.perception_range > 10:
            errors.append("perception_range must be <= 10")

        if self.max_actions_per_tick < 1:
            errors.append("max_actions_per_tick must be >= 1")

        if self.max_actions_per_tick > 10:
            errors.append("max_actions_per_tick must be <= 10")

        return errors

    # ── Serialization ────────────────────────────────────────────

    def to_dict(self) -> dict[str, Any]:
        """Serialize to a JSON-friendly dictionary."""
        return {
            "name": self.name,
            "version": self.version,
            "actions": list(self.actions),
            "perception_range": self.perception_range,
            "max_actions_per_tick": self.max_actions_per_tick,
        }

    def to_prompt_description(self) -> str:
        """Generate a capability description for LLM prompts.

        This description can be injected into the agent's system prompt
        so the LLM knows what actions are available.
        """
        actions_list = ", ".join(self.actions)
        return (
            f"Capability: {self.name} (v{self.version})\n"
            f"  Available actions: {actions_list}\n"
            f"  Perception range: {self.perception_range} tiles\n"
            f"  Max actions per tick: {self.max_actions_per_tick}"
        )

    def to_registration_actions(self) -> list[str]:
        """Return the actions list suitable for the registration API."""
        return list(self.actions)
