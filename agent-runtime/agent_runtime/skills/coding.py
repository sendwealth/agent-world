"""Built-in skill: Coding.

The coding skill represents an agent's ability to write, analyze, and
debug code. Higher levels unlock more complex tasks and faster execution.
"""

from __future__ import annotations

from typing import Any, Dict

from ..models.skill import Skill
from .registry import SkillDefinition


def _execute_coding(agent_skills: Dict[str, Skill], **kwargs: Any) -> Dict[str, Any]:
    """Execute a coding task.

    Kwargs:
        task: Description of the coding task.
        language: Target programming language (optional).

    Returns:
        Dict with success status, skill_level used, and result description.
    """
    coding_skill = agent_skills.get("coding")
    level = coding_skill.level if coding_skill else 0

    task = kwargs.get("task", "generic coding task")
    language = kwargs.get("language", "python")

    complexity_thresholds = {
        1: "simple scripts",
        3: "moderate programs with functions",
        5: "complex applications with classes",
        7: "system-level programming",
        9: "advanced algorithms and optimization",
    }

    capability = "basic scripting"
    for threshold, desc in sorted(complexity_thresholds.items()):
        if level >= threshold:
            capability = desc

    success = level >= 1
    quality = min(level / 10.0, 1.0)

    return {
        "skill": "coding",
        "task": task,
        "language": language,
        "capability": capability,
        "quality": quality,
        "success": success,
        "level_used": level,
    }


CODING_SKILL = SkillDefinition(
    name="coding",
    description="Ability to write, analyze, and debug code across programming languages",
    max_level=10,
    execute_fn=_execute_coding,
    category="technical",
)
