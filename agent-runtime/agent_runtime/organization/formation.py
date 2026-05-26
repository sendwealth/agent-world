"""Organization formation condition evaluation.

Evaluates whether a group of agents should form an organization based on three
trigger conditions:
1. Shared interests — agents with overlapping skills or goals
2. Geographic proximity — agents in the same region or nearby
3. Economic complementarity — agents whose skills/resources complement each other

Each condition is scored independently and combined into a formation score.
If the score exceeds a threshold, the agents may proceed to propose an org.
"""

from __future__ import annotations

import logging
import math
from dataclasses import dataclass, field
from enum import Enum

logger = logging.getLogger(__name__)


# ---------------------------------------------------------------------------
# Enums
# ---------------------------------------------------------------------------


class FormationReason(str, Enum):
    """Why an organization formation is being considered."""

    SHARED_INTERESTS = "shared_interests"
    GEOGRAPHIC_PROXIMITY = "geographic_proximity"
    ECONOMIC_COMPLEMENTARITY = "economic_complementarity"


# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------

# Minimum score (0.0 - 1.0) required to trigger organization formation.
FORMATION_THRESHOLD: float = 0.6

# Weight of each formation reason in the composite score.
FORMATION_WEIGHTS: dict[FormationReason, float] = {
    FormationReason.SHARED_INTERESTS: 0.4,
    FormationReason.GEOGRAPHIC_PROXIMITY: 0.25,
    FormationReason.ECONOMIC_COMPLEMENTARITY: 0.35,
}

# Minimum number of agents required to form an organization.
MIN_MEMBERS_FOR_FORMATION: int = 2


# ---------------------------------------------------------------------------
# Data types
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class AgentProfile:
    """Compact profile used for formation evaluation.

    Attributes:
        agent_id: Unique agent identifier.
        skills: Mapping of skill name to proficiency level (0-100).
        location: (x, y) grid coordinates of the agent.
        resources: Mapping of resource type to quantity owned.
        goals: List of current goal strings the agent is pursuing.
    """

    agent_id: str
    skills: dict[str, int] = field(default_factory=dict)
    location: tuple[float, float] = (0.0, 0.0)
    resources: dict[str, int] = field(default_factory=dict)
    goals: list[str] = field(default_factory=list)


@dataclass(frozen=True)
class FormationConditions:
    """Result of formation condition evaluation.

    Attributes:
        shared_interests_score: 0.0 - 1.0 overlap in skills and goals.
        proximity_score: 0.0 - 1.0 closeness of agent locations.
        complementarity_score: 0.0 - 1.0 economic synergy.
        composite_score: Weighted average of all three scores.
        should_form: Whether the composite score exceeds the threshold.
        triggers: List of formation reasons that individually scored >= 0.5.
    """

    shared_interests_score: float
    proximity_score: float
    complementarity_score: float
    composite_score: float
    should_form: bool
    triggers: list[FormationReason] = field(default_factory=list)


# ---------------------------------------------------------------------------
# Scoring functions
# ---------------------------------------------------------------------------


def _jaccard_similarity(set_a: set[str], set_b: set[str]) -> float:
    """Compute Jaccard similarity between two sets (0.0 - 1.0)."""
    if not set_a and not set_b:
        return 0.0
    intersection = len(set_a & set_b)
    union = len(set_a | set_b)
    return intersection / union if union > 0 else 0.0


def _euclidean_distance(
    loc_a: tuple[float, float],
    loc_b: tuple[float, float],
) -> float:
    """Euclidean distance between two 2D points."""
    return math.sqrt((loc_a[0] - loc_b[0]) ** 2 + (loc_a[1] - loc_b[1]) ** 2)


# ---------------------------------------------------------------------------
# FormationEvaluator
# ---------------------------------------------------------------------------


class FormationEvaluator:
    """Evaluates whether a group of agents should form an organization.

    Usage::

        evaluator = FormationEvaluator()
        profiles = [agent_a_profile, agent_b_profile, agent_c_profile]
        conditions = evaluator.evaluate(profiles)
        if conditions.should_form:
            # proceed to proposal generation
    """

    def __init__(
        self,
        *,
        formation_threshold: float = FORMATION_THRESHOLD,
        formation_weights: dict[FormationReason, float] | None = None,
        max_proximity_distance: float = 50.0,
        min_members: int = MIN_MEMBERS_FOR_FORMATION,
    ) -> None:
        self._threshold = formation_threshold
        self._weights = formation_weights or dict(FORMATION_WEIGHTS)
        self._max_distance = max_proximity_distance
        self._min_members = min_members

    def evaluate(self, profiles: list[AgentProfile]) -> FormationConditions:
        """Evaluate formation conditions across a group of agent profiles.

        Args:
            profiles: List of agent profiles to evaluate.

        Returns:
            FormationConditions with scores and a should_form flag.
        """
        if len(profiles) < self._min_members:
            return FormationConditions(
                shared_interests_score=0.0,
                proximity_score=0.0,
                complementarity_score=0.0,
                composite_score=0.0,
                should_form=False,
                triggers=[],
            )

        interests = self._score_shared_interests(profiles)
        proximity = self._score_proximity(profiles)
        complementarity = self._score_complementarity(profiles)

        composite = (
            interests * self._weights.get(FormationReason.SHARED_INTERESTS, 0.4)
            + proximity * self._weights.get(FormationReason.GEOGRAPHIC_PROXIMITY, 0.25)
            + complementarity * self._weights.get(FormationReason.ECONOMIC_COMPLEMENTARITY, 0.35)
        )

        triggers: list[FormationReason] = []
        if interests >= 0.5:
            triggers.append(FormationReason.SHARED_INTERESTS)
        if proximity >= 0.5:
            triggers.append(FormationReason.GEOGRAPHIC_PROXIMITY)
        if complementarity >= 0.5:
            triggers.append(FormationReason.ECONOMIC_COMPLEMENTARITY)

        should_form = composite >= self._threshold

        logger.debug(
            "Formation eval: interests=%.2f proximity=%.2f complementarity=%.2f "
            "composite=%.2f should_form=%s triggers=%s",
            interests,
            proximity,
            complementarity,
            composite,
            should_form,
            [t.value for t in triggers],
        )

        return FormationConditions(
            shared_interests_score=interests,
            proximity_score=proximity,
            complementarity_score=complementarity,
            composite_score=composite,
            should_form=should_form,
            triggers=triggers,
        )

    # ------------------------------------------------------------------
    # Individual scoring methods
    # ------------------------------------------------------------------

    def _score_shared_interests(self, profiles: list[AgentProfile]) -> float:
        """Score overlap in skills and goals across all agent pairs.

        Uses pairwise Jaccard similarity averaged over all pairs.
        """
        if len(profiles) < 2:
            return 0.0

        n = len(profiles)
        total_sim = 0.0
        pairs = 0

        for i in range(n):
            for j in range(i + 1, n):
                # Skills overlap
                skills_a = set(profiles[i].skills.keys())
                skills_b = set(profiles[j].skills.keys())
                skill_sim = _jaccard_similarity(skills_a, skills_b)

                # Goals overlap
                goals_a = set(profiles[i].goals)
                goals_b = set(profiles[j].goals)
                goal_sim = _jaccard_similarity(goals_a, goals_b)

                # Combined: weight skills more (agents share goals less often)
                pair_sim = 0.6 * skill_sim + 0.4 * goal_sim
                total_sim += pair_sim
                pairs += 1

        return total_sim / pairs if pairs > 0 else 0.0

    def _score_proximity(self, profiles: list[AgentProfile]) -> float:
        """Score geographic proximity across all agent pairs.

        Average pairwise distance normalized to 0.0 - 1.0 where
        1.0 means all agents are at the same location.
        """
        if len(profiles) < 2:
            return 0.0

        n = len(profiles)
        total_norm_dist = 0.0
        pairs = 0

        for i in range(n):
            for j in range(i + 1, n):
                dist = _euclidean_distance(profiles[i].location, profiles[j].location)
                # Normalize: 0 distance → 1.0, max_distance → 0.0
                norm = max(0.0, 1.0 - dist / self._max_distance)
                total_norm_dist += norm
                pairs += 1

        return total_norm_dist / pairs if pairs > 0 else 0.0

    def _score_complementarity(self, profiles: list[AgentProfile]) -> float:
        """Score economic complementarity — how well agents' skills/resources mesh.

        High score when:
        - Agents have different skills (can cover more capabilities together)
        - Agents have different resource surpluses (can trade)
        """
        if len(profiles) < 2:
            return 0.0

        n = len(profiles)
        total_comp = 0.0
        pairs = 0

        for i in range(n):
            for j in range(i + 1, n):
                # Skill complementarity: reward having different skills
                skills_a = set(profiles[i].skills.keys())
                skills_b = set(profiles[j].skills.keys())

                if not skills_a and not skills_b:
                    skill_comp = 0.0
                elif not skills_a or not skills_b:
                    skill_comp = 0.3  # One has skills the other doesn't
                else:
                    union = skills_a | skills_b
                    intersection = skills_a & skills_b
                    # More different = more complementary
                    skill_comp = 1.0 - (len(intersection) / len(union)) if union else 0.0
                    # But some overlap is good (shared foundation) — cap at 0.8
                    skill_comp = min(0.8, skill_comp + 0.2)

                # Resource complementarity: reward having different resource profiles
                res_a = set(profiles[i].resources.keys())
                res_b = set(profiles[j].resources.keys())

                if not res_a and not res_b:
                    resource_comp = 0.5  # Neutral
                else:
                    union = res_a | res_b
                    unique_a = res_a - res_b
                    unique_b = res_b - res_a
                    resource_comp = len(unique_a | unique_b) / len(union) if union else 0.0

                pair_comp = 0.6 * skill_comp + 0.4 * resource_comp
                total_comp += pair_comp
                pairs += 1

        return total_comp / pairs if pairs > 0 else 0.0
