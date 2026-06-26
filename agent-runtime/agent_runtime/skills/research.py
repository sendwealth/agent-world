"""Built-in skill: Research.

The research skill represents an agent's ability to gather information,
analyze data, and produce insights. Higher levels unlock deeper analysis
and faster information retrieval.
"""

from __future__ import annotations

from typing import Any

from ..models.skill import Skill
from .registry import SkillDefinition


def _execute_research(agent_skills: dict[str, Skill], **kwargs: Any) -> dict[str, Any]:
    """Execute a research task.

    Kwargs:
        topic: Research topic or question.
        depth: Desired analysis depth ("shallow", "medium", "deep").

    Returns:
        Dict with success status, findings summary, and confidence score.
    """
    research_skill = agent_skills.get("research")
    level = research_skill.level if research_skill else 0

    topic = kwargs.get("topic", "general inquiry")
    depth = kwargs.get("depth", "medium")

    methodology = "basic search"
    if level >= 3:
        methodology = "structured literature review"
    if level >= 5:
        methodology = "comparative analysis"
    if level >= 7:
        methodology = "multi-source triangulation"
    if level >= 9:
        methodology = "original research and synthesis"

    confidence = min(level * 0.10, 1.0)
    success = level >= 1

    return {
        "skill": "research",
        "topic": topic,
        "depth": depth,
        "methodology": methodology,
        "confidence": confidence,
        "success": success,
        "level_used": level,
    }


RESEARCH_SKILL = SkillDefinition(
    name="research",
    description="Ability to gather information, analyze data, and produce actionable insights",
    max_level=10,
    execute_fn=_execute_research,
    category="knowledge",
)
