"""Organization culture system — org-level culture vectors that influence members.

When agents join an organization, they experience gradual cultural pressure
toward the org's collective values.  The org culture is the weighted average
of all members' value weights, and it evolves naturally over time.
"""

from __future__ import annotations

import math
import random
from typing import Any, Dict, List, Optional

from pydantic import BaseModel, Field

from agent_runtime.models.personality import PersonalityVector
from agent_runtime.models.values import ValueWeights


# Maximum cultural pressure per tick (0.1% = 0.001).
MAX_CULTURE_PRESSURE_PER_TICK = 0.001

# Natural drift rate per tick (org culture slowly shifts toward
# the aggregate personality of its members).
NATURAL_DRIFT_RATE = 0.0005


class CultureVector(BaseModel):
    """Aggregate culture vector representing an organization's shared values.

    Each dimension mirrors ValueWeights but represents the org-level consensus.
    """

    cooperation_norm: float = Field(default=0.5, ge=0.0, le=1.0)
    competition_norm: float = Field(default=0.5, ge=0.0, le=1.0)
    exploration_norm: float = Field(default=0.5, ge=0.0, le=1.0)
    tradition_strength: float = Field(default=0.3, ge=0.0, le=1.0)
    innovation_norm: float = Field(default=0.3, ge=0.0, le=1.0)

    def distance(self, other: "CultureVector") -> float:
        """Euclidean distance to another culture vector."""
        dims = [
            "cooperation_norm",
            "competition_norm",
            "exploration_norm",
            "tradition_strength",
            "innovation_norm",
        ]
        return math.sqrt(sum((getattr(self, d) - getattr(other, d)) ** 2 for d in dims))

    def to_storage_dict(self) -> Dict[str, float]:
        return {
            "cooperation_norm": self.cooperation_norm,
            "competition_norm": self.competition_norm,
            "exploration_norm": self.exploration_norm,
            "tradition_strength": self.tradition_strength,
            "innovation_norm": self.innovation_norm,
        }


class OrgCultureSystem:
    """Organization culture: members' values are gradually influenced by the org norm.

    The system tracks one CultureVector per org, computed as the weighted average
    of member values.  New members experience gradual cultural pressure each tick.
    """

    def __init__(self) -> None:
        # org_id -> CultureVector
        self._org_cultures: Dict[str, CultureVector] = {}
        # org_id -> {agent_id: tenure_ticks}
        self._members: Dict[str, Dict[str, int]] = {}

    # ── Org culture computation ──

    def compute_org_culture(
        self,
        org_id: str,
        member_values: List[ValueWeights],
    ) -> CultureVector:
        """Compute organization culture vector as weighted average of member values.

        Args:
            org_id: Organization identifier.
            member_values: List of ValueWeights from all current members.

        Returns:
            The aggregated CultureVector for the organization.
        """
        if not member_values:
            return CultureVector()

        n = len(member_values)
        culture = CultureVector(
            cooperation_norm=sum(v.cooperation_weight for v in member_values) / n,
            competition_norm=sum(v.competition_weight for v in member_values) / n,
            exploration_norm=sum(v.exploration_drive for v in member_values) / n,
            tradition_strength=sum(v.tradition_adherence for v in member_values) / n,
            innovation_norm=sum(v.innovation_tendency for v in member_values) / n,
        )
        self._org_cultures[org_id] = culture
        return culture

    # ── Cultural pressure ──

    def apply_culture_pressure(
        self,
        agent_values: ValueWeights,
        org_id: str,
    ) -> Dict[str, Any]:
        """Apply gradual cultural pressure on an agent toward the org culture.

        Pressure is bounded by MAX_CULTURE_PRESSURE_PER_TICK (0.1%).
        Only adjusts dimensions where the agent deviates from the org norm.

        Args:
            agent_values: The agent's current ValueWeights.
            org_id: The organization the agent belongs to.

        Returns:
            Dict with 'adjustments' (per-dimension deltas), 'org_id', and
            'updated_values' (a new ValueWeights instance with the adjustments applied).
        """
        culture = self._org_cultures.get(org_id)
        if culture is None:
            return {"adjustments": {}, "org_id": org_id, "updated_values": agent_values}

        adjustments: Dict[str, float] = {}
        updates: Dict[str, float] = {}
        mapping = {
            "cooperation_weight": "cooperation_norm",
            "competition_weight": "competition_norm",
            "exploration_drive": "exploration_norm",
            "tradition_adherence": "tradition_strength",
            "innovation_tendency": "innovation_norm",
        }

        for agent_dim, culture_dim in mapping.items():
            current = getattr(agent_values, agent_dim)
            target = getattr(culture, culture_dim)
            diff = target - current

            # Scale by max pressure, preserving direction
            delta = max(-MAX_CULTURE_PRESSURE_PER_TICK, min(MAX_CULTURE_PRESSURE_PER_TICK, diff))

            if abs(delta) > 1e-9:
                updates[agent_dim] = max(0.0, min(1.0, current + delta))
                adjustments[agent_dim] = delta

        if updates:
            updated = agent_values.model_copy(update=updates)
        else:
            updated = agent_values

        return {"adjustments": adjustments, "org_id": org_id, "updated_values": updated}

    # ── Natural culture drift ──

    def culture_drift(self, org_id: str, tick: int) -> Dict[str, Any]:
        """Apply small random drift to org culture (simulates natural evolution).

        Drift is very small (NATURAL_DRIFT_RATE) and random in direction.

        Args:
            org_id: Organization identifier.
            tick: Current world tick.

        Returns:
            Dict with 'drift' (per-dimension changes), 'org_id', and 'tick'.
            The stored CultureVector is replaced with a new drifted instance.
        """
        culture = self._org_cultures.get(org_id)
        if culture is None:
            return {"drift": {}, "org_id": org_id, "tick": tick}

        dims = [
            "cooperation_norm",
            "competition_norm",
            "exploration_norm",
            "tradition_strength",
            "innovation_norm",
        ]

        drift: Dict[str, float] = {}
        updates: Dict[str, float] = {}
        for dim in dims:
            current = getattr(culture, dim)
            noise = random.uniform(-NATURAL_DRIFT_RATE, NATURAL_DRIFT_RATE)
            updates[dim] = max(0.0, min(1.0, current + noise))
            drift[dim] = noise

        drifted = culture.model_copy(update=updates)
        self._org_cultures[org_id] = drifted

        return {"drift": drift, "org_id": org_id, "tick": tick}

    # ── Accessors ──

    def get_org_culture(self, org_id: str) -> Optional[CultureVector]:
        """Get the stored culture vector for an organization."""
        return self._org_cultures.get(org_id)

    def register_member(self, org_id: str, agent_id: str) -> None:
        """Register an agent as a member of an organization."""
        if org_id not in self._members:
            self._members[org_id] = {}
        self._members[org_id][agent_id] = 0

    def increment_tenure(self, org_id: str, agent_id: str) -> None:
        """Increment the tenure counter for a member."""
        members = self._members.get(org_id)
        if members and agent_id in members:
            members[agent_id] += 1

    def get_tenure(self, org_id: str, agent_id: str) -> int:
        """Get the tenure (in ticks) of an agent in an organization."""
        members = self._members.get(org_id)
        if members and agent_id in members:
            return members[agent_id]
        return 0
