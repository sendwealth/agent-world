"""Bounty Hunter -- bounty evaluation, acceptance, and completion logic.

Provides the strategic layer for bounty-related actions. The BountyHunter
evaluates whether a bounty matches the agent's skills and resources,
generates an execution plan upon acceptance, and submits a result upon
completion.

Evaluation scoring is based on:
1. Skill match -- Does the bounty description contain keywords matching the agent's skills?
2. Resource feasibility -- Can the agent afford the token cost?
3. Reputation gate -- Does the agent have enough reputation?
"""

from __future__ import annotations

import logging
import re
from dataclasses import dataclass, field
from enum import StrEnum
from typing import Any

logger = logging.getLogger(__name__)

_HIGH_VALUE_REWARD_THRESHOLD = 500
_HIGH_VALUE_REPUTATION_MIN = 10.0

_SKILL_MATCH_WEIGHT = 0.4
_RESOURCE_WEIGHT = 0.3
_REPUTATION_WEIGHT = 0.3

_MIN_ACCEPTANCE_SCORE = 0.4


class BountyDecision(StrEnum):
    """Decision outcome for bounty evaluation."""

    ACCEPT = "accept"
    DECLINE_LOW_SCORE = "decline_low_score"
    DECLINE_INSUFFICIENT_TOKENS = "decline_insufficient_tokens"
    DECLINE_LOW_REPUTATION = "decline_low_reputation"


@dataclass(frozen=True)
class BountyEvaluation:
    """Result of evaluating a bounty against an agent's capabilities."""

    bounty_id: str
    decision: BountyDecision
    score: float = 0.0
    skill_match: float = 0.0
    resource_feasible: bool = True
    reputation_sufficient: bool = True
    reason: str = ""
    execution_plan: list[str] = field(default_factory=list)

    @property
    def should_accept(self) -> bool:
        """Whether the evaluation recommends accepting the bounty."""
        return self.decision == BountyDecision.ACCEPT


@dataclass(frozen=True)
class BountyCompletion:
    """Result of completing a bounty."""

    bounty_id: str
    result_text: str
    success: bool = True


_SKILL_KEYWORDS: dict[str, list[str]] = {
    "gather": ["gather", "collect", "harvest", "resource", "wood", "stone", "food"],
    "build": ["build", "construct", "create", "structure", "house", "wall", "tower"],
    "explore": ["explore", "discover", "map", "scout", "survey", "navigate"],
    "trade": ["trade", "exchange", "buy", "sell", "market", "deal", "commerce"],
    "fight": ["fight", "combat", "battle", "attack", "defend", "warrior"],
    "heal": ["heal", "cure", "medicine", "health", "recover"],
    "craft": ["craft", "forge", "smith", "enchant", "make", "tool"],
    "farm": ["farm", "plant", "grow", "crop", "agriculture"],
    "mine": ["mine", "dig", "ore", "mineral", "excavate"],
    "fish": ["fish", "catch", "sea", "river", "water"],
}


class BountyHunter:
    """Evaluates and manages bounty acceptance and completion.

    Stateless strategy module that the agent's action system calls
    before dispatching ACCEPT_BOUNTY / COMPLETE_BOUNTY via ActionExecutor.
    """

    def evaluate(
        self,
        bounty: dict[str, Any],
        agent_skills: dict[str, Any],
        agent_tokens: int,
        agent_reputation: float,
        *,
        acceptance_cost: int = 10,
    ) -> BountyEvaluation:
        """Evaluate whether an agent should accept a bounty."""
        bounty_id = bounty.get("id", "unknown")
        title = bounty.get("title", "")
        description = bounty.get("description", "")
        reward = bounty.get("reward", 0)

        skill_match = self._compute_skill_match(title, description, agent_skills)
        resource_feasible = agent_tokens >= acceptance_cost
        reputation_sufficient = True
        if reward >= _HIGH_VALUE_REWARD_THRESHOLD:
            reputation_sufficient = agent_reputation >= _HIGH_VALUE_REPUTATION_MIN

        score = (
            _SKILL_MATCH_WEIGHT * skill_match
            + _RESOURCE_WEIGHT * (1.0 if resource_feasible else 0.0)
            + _REPUTATION_WEIGHT * (1.0 if reputation_sufficient else 0.0)
        )

        if not resource_feasible:
            decision = BountyDecision.DECLINE_INSUFFICIENT_TOKENS
            reason = f"Insufficient tokens: need {acceptance_cost}, have {agent_tokens}"
        elif not reputation_sufficient:
            decision = BountyDecision.DECLINE_LOW_REPUTATION
            reason = (
                f"Reputation too low for high-value bounty: "
                f"need {_HIGH_VALUE_REPUTATION_MIN}, have {agent_reputation:.1f}"
            )
        elif score < _MIN_ACCEPTANCE_SCORE:
            decision = BountyDecision.DECLINE_LOW_SCORE
            reason = f"Low match score ({score:.2f} < {_MIN_ACCEPTANCE_SCORE}): skills do not align"
        else:
            decision = BountyDecision.ACCEPT
            reason = f"Good match (score {score:.2f}): skill_match={skill_match:.2f}"

        execution_plan = []
        if decision == BountyDecision.ACCEPT:
            execution_plan = self._generate_plan(title, description, agent_skills)

        return BountyEvaluation(
            bounty_id=bounty_id,
            decision=decision,
            score=score,
            skill_match=skill_match,
            resource_feasible=resource_feasible,
            reputation_sufficient=reputation_sufficient,
            reason=reason,
            execution_plan=execution_plan,
        )

    def create_completion_result(
        self,
        bounty_id: str,
        bounty_title: str,
        execution_summary: str,
    ) -> BountyCompletion:
        """Create a completion result for a bounty."""
        result_text = f"Completed bounty '{bounty_title}': {execution_summary}"
        return BountyCompletion(bounty_id=bounty_id, result_text=result_text, success=True)

    def _compute_skill_match(
        self,
        title: str,
        description: str,
        agent_skills: dict[str, Any],
    ) -> float:
        """Compute how well the agent's skills match the bounty."""
        if not agent_skills:
            return 0.0

        text = f"{title} {description}".lower()
        words = set(re.findall(r"[a-z]{3,}", text))
        skill_names = {name.lower() for name in agent_skills.keys()}

        expanded: set[str] = set()
        for skill_name in skill_names:
            expanded.add(skill_name)
            if skill_name in _SKILL_KEYWORDS:
                expanded.update(_SKILL_KEYWORDS[skill_name])

        if not expanded:
            matched = skill_names & words
            return len(matched) / max(len(skill_names), 1)

        overlap = expanded & words
        if not overlap:
            matched = skill_names & words
            if matched:
                return len(matched) / max(len(skill_names), 1)
            return 0.0

        return min(len(overlap) / max(len(expanded), 1) * 2, 1.0)

    def _generate_plan(
        self,
        title: str,
        description: str,
        agent_skills: dict[str, Any],
    ) -> list[str]:
        """Generate a simple execution plan for an accepted bounty."""
        plan = [
            f"Accept bounty: {title}",
            "Assess required resources and plan approach",
        ]
        for skill_name in agent_skills:
            plan.append(f"Utilize {skill_name} skill to progress")
        plan.extend([
            "Execute plan and monitor progress",
            "Submit completion result",
        ])
        return plan
