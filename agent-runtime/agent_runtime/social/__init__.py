"""Social module — group identity, cultural clustering, and intergroup dynamics.

Phase 4.3.3: Organization culture, regional clustering, trust, and conflict/fusion.
"""

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
    # org_culture
    "CultureVector",
    "OrgCultureSystem",
    "MAX_CULTURE_PRESSURE_PER_TICK",
    "NATURAL_DRIFT_RATE",
    # regional_culture
    "Cluster",
    "RegionalCulture",
    # intergroup_trust
    "InterGroupEvent",
    "InterGroupEventType",
    "IntergroupTrust",
    "MIN_OUT_GROUP_TRUST",
    "DEFAULT_IN_GROUP_TRUST",
    "DEFAULT_OUT_GROUP_TRUST",
    # cultural_conflict
    "AgentInteraction",
    "ConflictReport",
    "CulturalConflictAndFusion",
    "CONFLICT_THRESHOLD",
    "MAX_FUSION_DELTA",
]
