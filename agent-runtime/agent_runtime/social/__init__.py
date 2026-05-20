"""Social module — group identity, cultural clustering, and intergroup dynamics.

Phase 4.3.3: Organization culture, regional clustering, trust, and conflict/fusion.
"""

from .cultural_conflict import (
    AgentInteraction,
    ConflictReport,
    CulturalConflictAndFusion,
)
from .intergroup_trust import (
    InterGroupEvent,
    InterGroupEventType,
    IntergroupTrust,
)
from .org_culture import CultureVector, OrgCultureSystem
from .regional_culture import Cluster, RegionalCulture

__all__ = [
    # org_culture
    "CultureVector",
    "OrgCultureSystem",
    # regional_culture
    "Cluster",
    "RegionalCulture",
    # intergroup_trust
    "InterGroupEvent",
    "InterGroupEventType",
    "IntergroupTrust",
    # cultural_conflict
    "AgentInteraction",
    "ConflictReport",
    "CulturalConflictAndFusion",
]
