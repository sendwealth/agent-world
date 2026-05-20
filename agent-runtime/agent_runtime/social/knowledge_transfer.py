"""Knowledge transfer — experience and skill transmission between agents.

Implements two transmission channels:
  - teach_lesson: an agent imparts an experience (with outcome) to another agent,
    whose values and personality are updated based on how receptive they are.
  - transfer_skill: partial skill level transfer, gated by the teacher's proficiency
    and the student's existing level.
"""

from __future__ import annotations

import logging
from typing import Any, Dict, List, Optional

from agent_runtime.core.experience import Experience
from agent_runtime.models.personality import PersonalityVector
from agent_runtime.models.skill import Skill
from agent_runtime.models.values import ValueWeights

logger = logging.getLogger(__name__)

# Maximum fraction of skill level that can be transferred in one session.
MAX_SKILL_TRANSFER_RATIO = 0.3

# Learning efficiency modifier from openness personality dimension.
# Higher openness → faster learning from others.
OPENNESS_WEIGHT = 0.4


class KnowledgeTransfer:
    """Manages direct knowledge/experience transmission between agents."""

    def teach_lesson(
        self,
        teacher_values: ValueWeights,
        student_personality: PersonalityVector,
        student_values: ValueWeights,
        experience: Experience,
    ) -> Dict[str, Any]:
        """Teacher imparts an experience lesson to a student.

        The student's values are updated via update_from_experience, scaled by
        their openness (receptiveness to new ideas). The student also receives
        a small personality micro-shift based on the lesson.

        Args:
            teacher_values: The teacher's current values (used to gauge teaching quality).
            student_personality: The student's personality (openness affects learning rate).
            student_values: The student's values (mutated in place).
            experience: The experience being taught.

        Returns:
            Dict with keys: learned (bool), learning_efficiency (float),
            value_changes (dict), personality_shift (dict).
        """
        # Learning efficiency based on student's openness
        learning_efficiency = (
            OPENNESS_WEIGHT + (1.0 - OPENNESS_WEIGHT) * student_personality.openness
        )

        # Apply the experience to student's values, scaled by learning efficiency
        original = student_values.to_storage_dict()
        student_values.update_from_experience(experience.event_type, experience.outcome)

        # Scale the actual changes by learning efficiency
        updated = student_values.to_storage_dict()
        value_changes: Dict[str, float] = {}
        for dim in ValueWeights._dimension_names():
            delta = updated[dim] - original[dim]
            if abs(delta) > 1e-9:
                scaled_delta = delta * learning_efficiency
                new_val = max(0.0, min(1.0, original[dim] + scaled_delta))
                object.__setattr__(student_values, dim, new_val)
                value_changes[dim] = scaled_delta

        # Small personality shift: teaching tends to increase social orientation
        personality_shift: Dict[str, float] = {}
        if experience.outcome > 0:
            shift = 0.01 * learning_efficiency
            new_soc = min(1.0, student_personality.social_orientation + shift)
            personality_shift["social_orientation"] = new_soc - student_personality.social_orientation
            object.__setattr__(student_personality, "social_orientation", new_soc)

        learned = len(value_changes) > 0 or experience.outcome != 0

        return {
            "learned": learned,
            "learning_efficiency": learning_efficiency,
            "value_changes": value_changes,
            "personality_shift": personality_shift,
        }

    def transfer_skill(
        self,
        teacher_skill: Skill,
        student_skills: Dict[str, Skill],
        student_personality: PersonalityVector,
    ) -> float:
        """Transfer skill knowledge from teacher to student.

        The student gains experience proportional to:
        - Teacher's current level (more skilled teachers teach more)
        - MAX_SKILL_TRANSFER_RATIO (cap per session)
        - Student's openness (receptiveness)
        - Diminishing returns if student is already close to teacher's level

        Args:
            teacher_skill: The skill being taught.
            student_skills: The student's skill dict (mutated in place).
            student_personality: Student personality (openness gates transfer).

        Returns:
            Effective skill points transferred.
        """
        if teacher_skill.level < 2:
            return 0.0

        transferable_level = teacher_skill.level * MAX_SKILL_TRANSFER_RATIO
        openness_factor = (
            OPENNESS_WEIGHT + (1.0 - OPENNESS_WEIGHT) * student_personality.openness
        )
        effective_xp = int(transferable_level * 10 * openness_factor)  # 10 xp per level

        if effective_xp <= 0:
            return 0.0

        skill_name = teacher_skill.name
        if skill_name in student_skills:
            student_skills[skill_name].add_experience(effective_xp)
        else:
            new_skill = Skill(
                name=skill_name,
                max_level=teacher_skill.max_level,
                level=1,
                experience=0,
                next_level_exp=100,
            )
            new_skill.add_experience(effective_xp)
            student_skills[skill_name] = new_skill

        logger.debug(
            "Skill transfer: %s → student gained %d xp (openness=%.2f)",
            skill_name,
            effective_xp,
            student_personality.openness,
        )
        return float(effective_xp)
