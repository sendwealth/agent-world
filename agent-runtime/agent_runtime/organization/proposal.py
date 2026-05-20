"""Organization proposal generation.

Generates organization proposals with name, type, and charter when
formation conditions are met. Proposals include enough detail for
potential members to evaluate and decide whether to join.
"""

from __future__ import annotations

import logging
import random
import uuid
from dataclasses import dataclass, field
from enum import Enum
from typing import Any

from .formation import AgentProfile, FormationConditions, FormationReason

logger = logging.getLogger(__name__)


# ---------------------------------------------------------------------------
# Enums
# ---------------------------------------------------------------------------


class OrgType(str, Enum):
    """Types of organizations agents can form."""

    GUILD = "guild"  # Skill-based collaboration
    COOPERATIVE = "cooperative"  # Resource sharing
    ALLIANCE = "alliance"  # Mutual defense / mutual benefit
    SYNDICATE = "syndicate"  # Economic specialization
    COLLECTIVE = "collective"  # General-purpose community


# ---------------------------------------------------------------------------
# Name / charter generation pools
# ---------------------------------------------------------------------------

# Prefixes for org names — provide variety and flavor.
_NAME_PREFIXES: list[str] = [
    "Iron", "Silver", "Golden", "Crimson", "Azure", "Emerald",
    "Ancient", "Eternal", "Radiant", "Shadow", "Crystal", "Stellar",
    "Prime", "United", "Noble", "Sage", "Wild", "Swift",
    "Dawn", "Dusk", "Storm", "Frost", "Flame", "Stone",
]

# Suffixes based on org type.
_NAME_SUFFIXES: dict[OrgType, list[str]] = {
    OrgType.GUILD: ["Guild", "Brotherhood", "Order", "Society", "Circle"],
    OrgType.COOPERATIVE: ["Cooperative", "Union", "Collective", "Exchange", "Network"],
    OrgType.ALLIANCE: ["Alliance", "Pact", "Coalition", "League", "Front"],
    OrgType.SYNDICATE: ["Syndicate", "Consortium", "Trust", "Cartel", "Venture"],
    OrgType.COLLECTIVE: ["Collective", "Community", "Assembly", "Council", "Fellowship"],
}

# Charter templates keyed by formation reason.
_CHARTER_TEMPLATES: dict[FormationReason, list[str]] = {
    FormationReason.SHARED_INTERESTS: [
        "United by common expertise in {skills}, we pledge to advance our craft.",
        "Drawn together by shared goals, we commit to mutual growth and learning.",
        "Our combined knowledge in {skills} forms the foundation of this bond.",
    ],
    FormationReason.GEOGRAPHIC_PROXIMITY: [
        "Neighbors in a shared region, we band together for mutual prosperity.",
        "Bound by proximity, we forge this alliance to strengthen our community.",
        "Together in this land, we shall build a lasting foundation.",
    ],
    FormationReason.ECONOMIC_COMPLEMENTARITY: [
        "Our diverse skills complement each other — together we achieve more.",
        "By pooling our distinct resources, we create economic strength for all.",
        "Specialization and trade are the pillars of our shared prosperity.",
    ],
}


# ---------------------------------------------------------------------------
# Data types
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class OrgProposal:
    """A complete organization proposal ready for recruitment.

    Attributes:
        proposal_id: Unique identifier for this proposal.
        org_name: Generated organization name.
        org_type: Type of organization.
        charter: Draft charter / mission statement.
        founder_id: Agent ID of the founding member.
        founding_members: Agent IDs of initial members (including founder).
        formation_conditions: The conditions that triggered this proposal.
        proposed_tick: World tick when the proposal was created.
    """

    proposal_id: str
    org_name: str
    org_type: OrgType
    charter: str
    founder_id: str
    founding_members: list[str] = field(default_factory=list)
    formation_conditions: FormationConditions | None = None
    proposed_tick: int = 0


# ---------------------------------------------------------------------------
# ProposalGenerator
# ---------------------------------------------------------------------------


class ProposalGenerator:
    """Generates organization proposals from formation conditions.

    Usage::

        generator = ProposalGenerator()
        proposal = generator.generate(
            profiles=[profile_a, profile_b],
            conditions=formation_conditions,
            founder_id="agent-001",
            tick=42,
        )
    """

    def __init__(self, *, seed: int | None = None) -> None:
        self._rng = random.Random(seed)

    def generate(
        self,
        profiles: list[AgentProfile],
        conditions: FormationConditions,
        founder_id: str,
        tick: int = 0,
    ) -> OrgProposal:
        """Generate an organization proposal.

        Args:
            profiles: Agent profiles of founding members.
            conditions: The evaluated formation conditions.
            founder_id: ID of the agent initiating the formation.
            tick: Current world tick.

        Returns:
            An OrgProposal with generated name, type, charter, and members.
        """
        org_type = self._determine_org_type(conditions)
        org_name = self._generate_name(org_type)
        charter = self._generate_charter(conditions, profiles)
        member_ids = [p.agent_id for p in profiles]

        return OrgProposal(
            proposal_id=str(uuid.uuid4()),
            org_name=org_name,
            org_type=org_type,
            charter=charter,
            founder_id=founder_id,
            founding_members=member_ids,
            formation_conditions=conditions,
            proposed_tick=tick,
        )

    # ------------------------------------------------------------------
    # Internal helpers
    # ------------------------------------------------------------------

    def _determine_org_type(self, conditions: FormationConditions) -> OrgType:
        """Determine org type based on which formation triggers are strongest."""
        scores: dict[OrgType, float] = {
            OrgType.GUILD: 0.0,
            OrgType.COOPERATIVE: 0.0,
            OrgType.ALLIANCE: 0.0,
            OrgType.SYNDICATE: 0.0,
            OrgType.COLLECTIVE: 0.0,
        }

        # Map triggers to org types
        if FormationReason.SHARED_INTERESTS in conditions.triggers:
            scores[OrgType.GUILD] += conditions.shared_interests_score
            scores[OrgType.COLLECTIVE] += conditions.shared_interests_score * 0.5

        if FormationReason.GEOGRAPHIC_PROXIMITY in conditions.triggers:
            scores[OrgType.ALLIANCE] += conditions.proximity_score
            scores[OrgType.COLLECTIVE] += conditions.proximity_score * 0.5

        if FormationReason.ECONOMIC_COMPLEMENTARITY in conditions.triggers:
            scores[OrgType.SYNDICATE] += conditions.complementarity_score
            scores[OrgType.COOPERATIVE] += conditions.complementarity_score * 0.7

        # Pick the highest-scoring type; break ties randomly
        max_score = max(scores.values())
        if max_score == 0.0:
            return self._rng.choice(list(OrgType))

        top_types = [t for t, s in scores.items() if abs(s - max_score) < 1e-9]
        return self._rng.choice(top_types)

    def _generate_name(self, org_type: OrgType) -> str:
        """Generate a random organization name."""
        prefix = self._rng.choice(_NAME_PREFIXES)
        suffix = self._rng.choice(_NAME_SUFFIXES.get(org_type, ["Organization"]))
        return f"{prefix} {suffix}"

    def _generate_charter(
        self,
        conditions: FormationConditions,
        profiles: list[AgentProfile],
    ) -> str:
        """Generate a draft charter/mission statement."""
        # Pick the strongest trigger for the charter theme
        primary_trigger = (
            conditions.triggers[0] if conditions.triggers
            else FormationReason.SHARED_INTERESTS
        )

        template = self._rng.choice(
            _CHARTER_TEMPLATES.get(primary_trigger, _CHARTER_TEMPLATES[FormationReason.SHARED_INTERESTS])
        )

        # Collect shared skills for template formatting
        all_skills: set[str] = set()
        for p in profiles:
            all_skills.update(p.skills.keys())
        skills_str = ", ".join(sorted(all_skills)[:3]) if all_skills else "various arts"

        charter_body = template.format(skills=skills_str)

        # Append org-type-specific goals
        type_purpose = {
            OrgType.GUILD: "Advancing mastery through shared knowledge.",
            OrgType.COOPERATIVE: "Pooling resources for mutual benefit.",
            OrgType.ALLIANCE: "Standing together for mutual security and growth.",
            OrgType.SYNDICATE: "Maximizing economic efficiency through specialization.",
            OrgType.COLLECTIVE: "Building a strong community through cooperation.",
        }

        return f"{charter_body} {type_purpose.get(conditions.triggers[0] if conditions.triggers else None, type_purpose[OrgType.COLLECTIVE])}" if conditions.triggers else f"{charter_body} {type_purpose[OrgType.COLLECTIVE]}"
