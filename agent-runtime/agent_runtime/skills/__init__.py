"""Skills registry — pluggable capabilities that an agent can learn and execute.

Provides:
- SkillDefinition: Describes a skill template.
- SkillRegistry: Central registry for skill definitions (register / query / upgrade).
- SkillExecutor: Executes skills and awards experience points.
- Built-in skills: coding, trading, research, teaching.
"""

from .registry import SkillDefinition, SkillRegistry
from .executor import SkillExecutor, SkillExecutionResult, XPReward
from .coding import CODING_SKILL
from .trading import TRADING_SKILL
from .research import RESEARCH_SKILL
from .teaching import TEACHING_SKILL

# Convenience: all built-in skill definitions as a list.
BUILTIN_SKILLS = [CODING_SKILL, TRADING_SKILL, RESEARCH_SKILL, TEACHING_SKILL]


def create_registry_with_builtins() -> SkillRegistry:
    """Create a SkillRegistry pre-loaded with all built-in skills."""
    registry = SkillRegistry()
    for skill_def in BUILTIN_SKILLS:
        registry.register(skill_def)
    return registry


__all__ = [
    "SkillDefinition",
    "SkillRegistry",
    "SkillExecutor",
    "SkillExecutionResult",
    "XPReward",
    "CODING_SKILL",
    "TRADING_SKILL",
    "RESEARCH_SKILL",
    "TEACHING_SKILL",
    "BUILTIN_SKILLS",
    "create_registry_with_builtins",
]
