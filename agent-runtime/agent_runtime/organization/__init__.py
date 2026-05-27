"""Organization module — formation, proposal, recruitment, and governance for agent organizations.

Agents can spontaneously decide to form organizations based on:
- Shared interests (common skills, goals)
- Geographic proximity (nearby agents)
- Economic complementarity (skill synergy, resource exchange potential)

Once formed, agents use the governance module to make leadership, treaty,
tax, and allocation decisions.
"""

from .formation import (
    FormationConditions,
    FormationEvaluator,
    FormationReason,
)
from .governance import (
    AgentInterests,
    AllocationStrategy,
    Candidate,
    GovernanceDecider,
    GovernanceDecision,
    LeadershipAmbition,
    OrgSnapshot,
    Treaty,
    TreatyResponse,
)
from .governance_analysis import (
    ExportFormat,
    GovernanceAnalyzer,
    GovernanceComparison,
    GovernanceEventData,
    LeadershipPrediction,
    OrgGovernanceSnapshot,
    StabilityLevel,
    StabilityReport,
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
from .rule_proposal import (
    RuleProposalEngine,
    RuleProposal,
    RuleCondition,
    RuleEffect,
    RuleCategory,
)
from .rule_evolution import (
    RuleEvolutionTracker,
    RuleLifecycleEvent,
    RuleLifecycleEventType,
    RuleStats,
)

__all__ = [
    # formation
    "FormationConditions",
    "FormationEvaluator",
    "FormationReason",
    # governance
    "AgentInterests",
    "AllocationStrategy",
    "Candidate",
    "GovernanceDecision",
    "GovernanceDecider",
    "LeadershipAmbition",
    "OrgSnapshot",
    "Treaty",
    "TreatyResponse",
    # governance_analysis
    "ExportFormat",
    "GovernanceAnalyzer",
    "GovernanceComparison",
    "GovernanceEventData",
    "LeadershipPrediction",
    "OrgGovernanceSnapshot",
    "StabilityLevel",
    "StabilityReport",
    # proposal
    "OrgProposal",
    "OrgType",
    "ProposalGenerator",
    # recruitment
    "Invitation",
    "InvitationStatus",
    "RecruitmentEngine",
    # rule proposal
    "RuleProposalEngine",
    "RuleProposal",
    "RuleCondition",
    "RuleEffect",
    "RuleCategory",
    # rule evolution
    "RuleEvolutionTracker",
    "RuleLifecycleEvent",
    "RuleLifecycleEventType",
    "RuleStats",
]
