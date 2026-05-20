"""Social interaction modules — knowledge transfer, imitation, cultural diffusion,
and group identity (org culture, regional clustering, intergroup dynamics).

Phase 4.3.2: Knowledge transfer, imitation, cultural diffusion.
Phase 4.3.3: Organization culture, regional clustering, trust, and conflict/fusion.
"""

from agent_runtime.social.cultural_diffusion import CulturalDiffusion
from agent_runtime.social.imitation import ImitationEngine
from agent_runtime.social.knowledge_transfer import KnowledgeTransfer

from .cultural_conflict import (
    CONFLICT_THRESHOLD,
    MAX_FUSION_DELTA,
    AgentInteraction,
    ConflictReport,
    CulturalConflictAndFusion,
)
from .intergroup_trust import (
    DEFAULT_IN_GROUP_TRUST,
    DEFAULT_OUT_GROUP_TRUST,
    MIN_OUT_GROUP_TRUST,
    InterGroupEvent,
    InterGroupEventType,
    IntergroupTrust,
)
from .org_culture import (
    MAX_CULTURE_PRESSURE_PER_TICK,
    NATURAL_DRIFT_RATE,
    CultureVector,
    OrgCultureSystem,
)
from .regional_culture import Cluster, RegionalCulture

__all__ = [
    # Phase 4.3.2 — cultural transmission
    "CulturalDiffusion",
    "ImitationEngine",
    "KnowledgeTransfer",
    # Phase 4.3.3 — group identity
    "CultureVector",
    "OrgCultureSystem",
    "MAX_CULTURE_PRESSURE_PER_TICK",
    "NATURAL_DRIFT_RATE",
    "Cluster",
    "RegionalCulture",
    "InterGroupEvent",
    "InterGroupEventType",
    "IntergroupTrust",
    "MIN_OUT_GROUP_TRUST",
    "DEFAULT_IN_GROUP_TRUST",
    "DEFAULT_OUT_GROUP_TRUST",
    "AgentInteraction",
    "ConflictReport",
    "CulturalConflictAndFusion",
    "CONFLICT_THRESHOLD",
    "MAX_FUSION_DELTA",
]
