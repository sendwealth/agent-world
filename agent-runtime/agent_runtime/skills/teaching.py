"""Built-in skill: Teaching.

The teaching skill represents an agent's ability to transfer knowledge
to other agents. Higher levels unlock more effective teaching methods
and faster knowledge transfer.
"""

from __future__ import annotations

from typing import Any, Dict

from ..models.skill import Skill
from .registry import SkillDefinition


def _execute_teaching(agent_skills: Dict[str, Skill], **kwargs: Any) -> Dict[str, Any]:
    """Execute a teaching task.

    Kwargs:
        subject: The subject or skill being taught.
        target_skill: The target skill name to teach (optional).
        target_level: Desired level for the learner (optional).

    Returns:
        Dict with success status, teaching method, and effectiveness score.
    """
    teaching_skill = agent_skills.get("teaching")
    level = teaching_skill.level if teaching_skill else 0

    subject = kwargs.get("subject", "general knowledge")
    target_skill = kwargs.get("target_skill")
    target_level = kwargs.get("target_level", 1)

    method = "direct instruction"
    if level >= 3:
        method = "guided practice"
    if level >= 5:
        method = "socratic method"
    if level >= 7:
        method = "personalized curriculum"
    if level >= 9:
        method = "experiential learning design"

    effectiveness = min(level * 0.10, 1.0)
    success = level >= 1

    return {
        "skill": "teaching",
        "subject": subject,
        "target_skill": target_skill,
        "target_level": target_level,
        "method": method,
        "effectiveness": effectiveness,
        "success": success,
        "level_used": level,
    }


TEACHING_SKILL = SkillDefinition(
    name="teaching",
    description="Ability to transfer knowledge and skills to other agents effectively",
    max_level=10,
    execute_fn=_execute_teaching,
    category="social",
)
