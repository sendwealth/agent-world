"""Social module — personality, cultural transmission, group identity, and intergroup dynamics.

Phase 4.3: Personality vectors, knowledge transfer, cultural diffusion, organization culture,
regional clustering, trust, conflict/fusion, and language emergence.
"""

from .comm_analyzer import CommunicationAnalyzer, DialectReport, MessagePattern
from .cultural_conflict import (
    CONFLICT_THRESHOLD,
    MAX_FUSION_DELTA,
    AgentInteraction,
    ConflictReport,
    CulturalConflictAndFusion,
)
from .cultural_diffusion import CulturalDiffusion
from .dialect_divergence import (
    DialectDivergenceAnalyzer,
    DialectDistanceMatrix,
    DialectRegion,
    DivergenceReport,
)
from .imitation import ImitationEngine
from .intergroup_trust import (
    DEFAULT_IN_GROUP_TRUST,
    DEFAULT_OUT_GROUP_TRUST,
    MIN_OUT_GROUP_TRUST,
    InterGroupEvent,
    InterGroupEventType,
    IntergroupTrust,
)
from .jargon_detector import JargonDetector, JargonTerm
from .knowledge_transfer import KnowledgeTransfer
from .language_experiment import EfficiencyMetrics, LanguageExperiment, VocabConstraint
from .org_culture import (
    MAX_CULTURE_PRESSURE_PER_TICK,
    NATURAL_DRIFT_RATE,
    CultureVector,
    OrgCultureSystem,
)
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
    "MAX_CULTURE_PRESSURE_PER_TICK",
    "NATURAL_DRIFT_RATE",
    # regional_culture (Phase 4.3.3)
    "Cluster",
    "RegionalCulture",
    # intergroup_trust (Phase 4.3.3)
    "InterGroupEvent",
    "InterGroupEventType",
    "IntergroupTrust",
    "MIN_OUT_GROUP_TRUST",
    "DEFAULT_IN_GROUP_TRUST",
    "DEFAULT_OUT_GROUP_TRUST",
    # cultural_conflict (Phase 4.3.3)
    "AgentInteraction",
    "ConflictReport",
    "CulturalConflictAndFusion",
    "CONFLICT_THRESHOLD",
    "MAX_FUSION_DELTA",
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
    # dialect_divergence (Phase 4.8)
    "DialectDivergenceAnalyzer",
    "DialectDistanceMatrix",
    "DialectRegion",
    "DivergenceReport",
]
