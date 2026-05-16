"""SkillRegistry — register, query, and upgrade skills for agents.

The registry holds SkillDefinition entries that describe available skills.
Agents acquire skills (as Skill instances) by registering them from the
registry into their own skill set.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any, Callable, Dict, List, Optional

from ..models.skill import Skill


# ---------------------------------------------------------------------------
# SkillDefinition
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class SkillDefinition:
    """Describes a skill template that can be registered in the SkillRegistry.

    Attributes:
        name: Unique skill identifier (e.g. "coding", "trading").
        description: Human-readable description.
        max_level: Maximum achievable level (default 10).
        execute_fn: Callable that performs the skill action.
            Receives (agent_skills: Dict[str, Skill], **kwargs) and returns Any.
        category: Optional grouping label.
    """

    name: str
    description: str = ""
    max_level: int = 10
    execute_fn: Optional[Callable[..., Any]] = None
    category: str = "general"


# ---------------------------------------------------------------------------
# SkillRegistry
# ---------------------------------------------------------------------------


class SkillRegistry:
    """Central registry for skill definitions.

    Usage::

        registry = SkillRegistry()
        registry.register(coding_def)
        defs = registry.list_skills()
        defn = registry.get("coding")
        skill = registry.create_skill("coding")
    """

    def __init__(self) -> None:
        self._definitions: Dict[str, SkillDefinition] = {}

    # -- Register / Unregister --

    def register(self, definition: SkillDefinition) -> None:
        """Register a new skill definition.

        Raises:
            ValueError: If a skill with the same name is already registered.
        """
        if definition.name in self._definitions:
            raise ValueError(f"Skill '{definition.name}' is already registered")
        self._definitions[definition.name] = definition

    def unregister(self, name: str) -> SkillDefinition:
        """Remove a skill definition from the registry.

        Raises:
            KeyError: If the skill is not found.
        """
        if name not in self._definitions:
            raise KeyError(f"Skill '{name}' is not registered")
        return self._definitions.pop(name)

    def upgrade(self, definition: SkillDefinition) -> None:
        """Replace an existing skill definition with an updated version.

        Raises:
            KeyError: If the skill is not currently registered.
        """
        if definition.name not in self._definitions:
            raise KeyError(f"Skill '{definition.name}' is not registered")
        self._definitions[definition.name] = definition

    # -- Query --

    def get(self, name: str) -> SkillDefinition:
        """Get a skill definition by name.

        Raises:
            KeyError: If not found.
        """
        if name not in self._definitions:
            raise KeyError(f"Skill '{name}' is not registered")
        return self._definitions[name]

    def has(self, name: str) -> bool:
        """Check whether a skill is registered."""
        return name in self._definitions

    def list_skills(self, category: Optional[str] = None) -> List[SkillDefinition]:
        """Return all registered definitions, optionally filtered by category."""
        defs = list(self._definitions.values())
        if category is not None:
            defs = [d for d in defs if d.category == category]
        return sorted(defs, key=lambda d: d.name)

    def categories(self) -> List[str]:
        """Return unique category names."""
        return sorted({d.category for d in self._definitions.values()})

    # -- Factory --

    def create_skill(self, name: str, level: int = 1) -> Skill:
        """Create a Skill instance from a registered definition.

        Args:
            name: Skill definition name.
            level: Starting level (default 1).

        Returns:
            A new Skill instance with the definition's max_level.
        """
        defn = self.get(name)
        return Skill(
            name=defn.name,
            max_level=defn.max_level,
            level=level,
        )

    @property
    def count(self) -> int:
        """Number of registered skill definitions."""
        return len(self._definitions)
