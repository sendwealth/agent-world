"""SkillExecutor — executes skills and manages experience point rewards.

XP rules (per the task spec):
  - USE:   +10 XP  — any skill execution
  - SUCCESS: +30 XP — the execution returned success=True
  - TEACHING: +50 XP — the skill was "teaching"

Multiple bonuses stack, e.g. a successful teaching action earns 10 + 30 + 50 = 90 XP.
"""

from __future__ import annotations

import logging
from dataclasses import dataclass, field
from enum import Enum
from typing import Any, Dict, Optional

from ..models.skill import Skill
from .registry import SkillRegistry

logger = logging.getLogger(__name__)


# ---------------------------------------------------------------------------
# XP constants
# ---------------------------------------------------------------------------


class XPReward(Enum):
    """Fixed XP amounts for different skill actions."""

    USE = 10
    SUCCESS = 30
    TEACHING = 50


# ---------------------------------------------------------------------------
# Execution result
# ---------------------------------------------------------------------------


@dataclass
class SkillExecutionResult:
    """Result of executing a skill.

    Attributes:
        skill_name: Name of the executed skill.
        output: The raw return value from the skill's execute_fn.
        xp_earned: Total XP awarded for this execution.
        xp_breakdown: Detailed breakdown of XP sources.
        leveled_up: Whether the skill leveled up from this execution.
    """

    skill_name: str
    output: Any
    xp_earned: int = 0
    xp_breakdown: Dict[str, int] = field(default_factory=dict)
    leveled_up: bool = False


# ---------------------------------------------------------------------------
# SkillExecutor
# ---------------------------------------------------------------------------


class SkillExecutor:
    """Executes registered skills and awards experience points.

    Usage::

        registry = SkillRegistry()
        # ... register skills ...
        executor = SkillExecutor(registry)
        agent_skills = {"coding": Skill(name="coding", level=3)}
        result = executor.execute("coding", agent_skills, task="build API")
        print(result.xp_earned)       # e.g. 40 (10 use + 30 success)
        print(result.leveled_up)      # whether coding leveled up
    """

    def __init__(self, registry: Optional[SkillRegistry] = None) -> None:
        self._registry = registry or SkillRegistry()

    @property
    def registry(self) -> SkillRegistry:
        return self._registry

    def execute(
        self,
        skill_name: str,
        agent_skills: Dict[str, Skill],
        **kwargs: Any,
    ) -> SkillExecutionResult:
        """Execute a skill by name.

        Args:
            skill_name: Name of the registered skill to execute.
            agent_skills: The agent's current skill dict (name -> Skill).
            **kwargs: Passed through to the skill's execute_fn.

        Returns:
            A SkillExecutionResult with output, XP earned, and level-up status.

        Raises:
            KeyError: If the skill is not registered.
            ValueError: If the skill definition has no execute_fn.
        """
        defn = self._registry.get(skill_name)

        if defn.execute_fn is None:
            raise ValueError(f"Skill '{skill_name}' has no execute function")

        # Execute the skill
        output = defn.execute_fn(agent_skills, **kwargs)

        # Calculate XP
        xp_breakdown: Dict[str, int] = {}
        xp_breakdown["use"] = XPReward.USE.value

        # Check if the result signals success
        is_success = isinstance(output, dict) and output.get("success", False)
        if is_success:
            xp_breakdown["success"] = XPReward.SUCCESS.value

        # Teaching bonus
        if skill_name == "teaching":
            xp_breakdown["teaching"] = XPReward.TEACHING.value

        total_xp = sum(xp_breakdown.values())

        # Apply XP to the agent's skill instance
        leveled_up = False
        skill = agent_skills.get(skill_name)
        if skill is not None:
            leveled_up = skill.add_experience(total_xp)
            logger.info(
                "Skill '%s' earned %d XP (total: %d, level: %d)%s",
                skill_name, total_xp, skill.experience, skill.level,
                " — LEVELED UP!" if leveled_up else "",
            )

        return SkillExecutionResult(
            skill_name=skill_name,
            output=output,
            xp_earned=total_xp,
            xp_breakdown=xp_breakdown,
            leveled_up=leveled_up,
        )

    def calculate_xp(
        self,
        skill_name: str,
        success: bool = False,
    ) -> int:
        """Calculate XP for a hypothetical skill execution without applying it.

        Useful for previews or planning.
        """
        total = XPReward.USE.value
        if success:
            total += XPReward.SUCCESS.value
        if skill_name == "teaching":
            total += XPReward.TEACHING.value
        return total
