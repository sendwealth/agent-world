"""Organization module — formation, proposal, and recruitment for agent organizations.

Agents can spontaneously decide to form organizations based on:
- Shared interests (common skills, goals)
- Geographic proximity (nearby agents)
- Economic complementarity (skill synergy, resource exchange potential)
"""

from .formation import (
    FormationConditions,
    FormationEvaluator,
    FormationReason,
)
from .proposal import (
    OrgProposal,
    OrgType,
    ProposalGenerator,
)
from .recruitment import (
    Invitation,
    InvitationStatus,
    RecruitmentEngine,
)

__all__ = [
    # formation
    "FormationConditions",
    "FormationEvaluator",
    "FormationReason",
    # proposal
    "OrgProposal",
    "OrgType",
    "ProposalGenerator",
    # recruitment
    "Invitation",
    "InvitationStatus",
    "RecruitmentEngine",
]
