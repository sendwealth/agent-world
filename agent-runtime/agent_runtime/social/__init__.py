"""Social module — personality, cultural transmission, group identity, and intergroup dynamics.

Phase 4.3: Personality vectors, knowledge transfer, cultural diffusion, organization culture,
regional clustering, trust, conflict/fusion, and language emergence.
"""

from .comm_analyzer import CommunicationAnalyzer, DialectReport, MessagePattern
from .cultural_conflict import (
    AgentInteraction,
    ConflictReport,
    CulturalConflictAndFusion,
)
from .cultural_diffusion import CulturalDiffusion
from .imitation import ImitationEngine
from .intergroup_trust import (
    InterGroupEvent,
    InterGroupEventType,
    IntergroupTrust,
)
from .jargon_detector import JargonDetector, JargonTerm
from .knowledge_transfer import KnowledgeTransfer
from .language_experiment import EfficiencyMetrics, LanguageExperiment, VocabConstraint
from .org_culture import CultureVector, OrgCultureSystem
from .regional_culture import Cluster, RegionalCulture

__all__ = [
    # cultural_diffusion (Phase 4.3.2)
    "CulturalDiffusion",
    # imitation (Phase 4.3.2)
    "ImitationEngine",
    # knowledge_transfer (Phase 4.3.2)
    "KnowledgeTransfer",
    # org_culture (Phase 4.3.3)
    "CultureVector",
    "OrgCultureSystem",
    # regional_culture (Phase 4.3.3)
    "Cluster",
    "RegionalCulture",
    # intergroup_trust (Phase 4.3.3)
    "InterGroupEvent",
    "InterGroupEventType",
    "IntergroupTrust",
    # cultural_conflict (Phase 4.3.3)
    "AgentInteraction",
    "ConflictReport",
    "CulturalConflictAndFusion",
    # comm_analyzer (Phase 4.3.4)
    "CommunicationAnalyzer",
    "DialectReport",
    "MessagePattern",
    # jargon_detector (Phase 4.3.4)
    "JargonDetector",
    "JargonTerm",
    # language_experiment (Phase 4.3.4)
    "EfficiencyMetrics",
    "LanguageExperiment",
    "VocabConstraint",
]
