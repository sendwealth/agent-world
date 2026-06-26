"""Intergroup trust system — 'us vs them' trust differentiation.

Agents develop different trust levels toward in-group and out-group members.
Trust is shaped by direct interaction history and group-level events.
"""

from __future__ import annotations

from enum import StrEnum

from pydantic import BaseModel, Field

# Minimum out-group trust floor — agents never completely distrust outsiders.
MIN_OUT_GROUP_TRUST = 0.1

# Default in-group trust baseline.
DEFAULT_IN_GROUP_TRUST = 0.7

# Default out-group trust baseline.
DEFAULT_OUT_GROUP_TRUST = 0.3

# Trust adjustment per positive/negative event.
TRUST_EVENT_DELTA = 0.05


class InterGroupEventType(StrEnum):
    """Types of events that affect intergroup trust."""

    TRADE_SUCCESS = "trade_success"
    TRADE_FAILURE = "trade_failure"
    CONFLICT = "conflict"
    COOPERATION = "cooperation"
    BETRAYAL = "betrayal"
    ALLIANCE_FORMED = "alliance_formed"
    ALLIANCE_BROKEN = "alliance_broken"


class InterGroupEvent(BaseModel):
    """An event that affects trust between groups."""

    event_type: InterGroupEventType
    source_group: str
    target_group: str
    # Intensity > 1.0 represents amplified events (e.g. major betrayals or
    # high-stakes alliances) that have outsized impact on trust adjustment.
    intensity: float = Field(default=1.0, ge=0.0, le=2.0)
    tick: int = 0


class TrustRecord(BaseModel):
    """Trust record between two groups."""

    source_group: str
    target_group: str
    trust_value: float = Field(default=DEFAULT_OUT_GROUP_TRUST, ge=MIN_OUT_GROUP_TRUST, le=1.0)
    interaction_count: int = 0
    last_event_tick: int = 0


class IntergroupTrust:
    """'Us vs them': different trust levels for in-group vs out-group agents.

    Agent→group trust and group→group trust are stored in separate dicts to
    avoid key-space collisions when agent_id and group_id strings overlap.

    In-group trust defaults high; out-group trust defaults low but bounded below
    by MIN_OUT_GROUP_TRUST.
    """

    def __init__(self) -> None:
        # (agent_id, group_id) -> TrustRecord  — individual agent↔group trust
        self._agent_group_trust: dict[tuple[str, str], TrustRecord] = {}
        # (source_group, target_group) -> TrustRecord  — group↔group trust
        self._group_group_trust: dict[tuple[str, str], TrustRecord] = {}
        # agent_id -> set of group_ids the agent belongs to
        self._agent_groups: dict[str, set[str]] = {}

    # ── In-group trust ──

    def compute_in_group_trust(self, agent_id: str, group_id: str) -> float:
        """Compute trust level for an agent toward its own group.

        In-group trust is always >= DEFAULT_IN_GROUP_TRUST and can increase
        with positive interactions but never exceeds 1.0.

        Args:
            agent_id: The agent computing trust.
            group_id: The agent's own group.

        Returns:
            Trust value in [0.7, 1.0].
        """
        record = self._agent_group_trust.get((agent_id, group_id))
        if record:
            return record.trust_value

        # Register agent-group membership
        self._register_membership(agent_id, group_id)

        # Create a default in-group trust record
        record = TrustRecord(
            source_group=agent_id,
            target_group=group_id,
            trust_value=DEFAULT_IN_GROUP_TRUST,
        )
        self._agent_group_trust[(agent_id, group_id)] = record
        return DEFAULT_IN_GROUP_TRUST

    # ── Out-group trust ──

    def compute_out_group_trust(
        self,
        agent_id: str,
        other_group_id: str,
    ) -> float:
        """Compute trust level for an agent toward a different group.

        Out-group trust starts at DEFAULT_OUT_GROUP_TRUST and is bounded
        below by MIN_OUT_GROUP_TRUST. It can increase through positive
        interactions and decrease through negative ones.

        Args:
            agent_id: The agent computing trust.
            other_group_id: The other group to evaluate trust toward.

        Returns:
            Trust value in [MIN_OUT_GROUP_TRUST, 1.0].
        """
        key = (agent_id, other_group_id)
        record = self._agent_group_trust.get(key)
        if record:
            return record.trust_value

        # Default out-group trust
        record = TrustRecord(
            source_group=agent_id,
            target_group=other_group_id,
            trust_value=DEFAULT_OUT_GROUP_TRUST,
        )
        self._agent_group_trust[key] = record
        return DEFAULT_OUT_GROUP_TRUST

    # ── Event-driven trust updates ──

    def update_trust_from_event(self, event: InterGroupEvent) -> None:
        """Update trust between two groups based on an inter-group event.

        Positive events (cooperation, trade success) increase trust.
        Negative events (conflict, betrayal) decrease trust.
        Out-group trust is floored at MIN_OUT_GROUP_TRUST.

        Args:
            event: The inter-group event triggering trust changes.
        """
        delta = self._event_delta(event.event_type, event.intensity)

        # Update group-to-group trust
        self._adjust_group_trust(event.source_group, event.target_group, delta)
        # Reciprocal: also affect the reverse direction (slightly less)
        self._adjust_group_trust(event.target_group, event.source_group, delta * 0.7)

        # Mirror updates into agent→group trust when the source/target is an
        # agent that already has a record.  Without this, get_trust() returns
        # the stale agent→group record and ignores the updated group→group one.
        self._adjust_agent_group_trust(event.source_group, event.target_group, delta)
        self._adjust_agent_group_trust(event.target_group, event.source_group, delta * 0.7)

    # ── Query helpers ──

    def get_trust(self, source: str, target: str) -> float:
        """Get the current trust value from source to target.

        Checks agent→group trust first, then falls back to group→group trust.
        """
        record = self._agent_group_trust.get((source, target))
        if record:
            return record.trust_value
        record = self._group_group_trust.get((source, target))
        return record.trust_value if record else DEFAULT_OUT_GROUP_TRUST

    def get_all_group_trust(self, group_id: str) -> dict[str, float]:
        """Get trust levels from one group toward all known groups.

        Combines records from both agent→group and group→group trust stores.
        """
        result: dict[str, float] = {}
        for (src, tgt), record in self._agent_group_trust.items():
            if src == group_id:
                result[tgt] = record.trust_value
        for (src, tgt), record in self._group_group_trust.items():
            if src == group_id:
                result[tgt] = record.trust_value
        return result

    def get_agent_groups(self, agent_id: str) -> set[str]:
        """Get all groups an agent belongs to."""
        return self._agent_groups.get(agent_id, set())

    def register_membership(self, agent_id: str, group_id: str) -> None:
        """Register an agent as a member of a group."""
        self._register_membership(agent_id, group_id)

    # ── Internals ──

    def _register_membership(self, agent_id: str, group_id: str) -> None:
        if agent_id not in self._agent_groups:
            self._agent_groups[agent_id] = set()
        self._agent_groups[agent_id].add(group_id)

    def _adjust_group_trust(
        self,
        source: str,
        target: str,
        delta: float,
    ) -> None:
        """Adjust group-to-group trust from source toward target by delta."""
        key = (source, target)
        record = self._group_group_trust.get(key)

        if record is None:
            record = TrustRecord(
                source_group=source,
                target_group=target,
                trust_value=DEFAULT_OUT_GROUP_TRUST,
            )
            self._group_group_trust[key] = record

        new_val = record.trust_value + delta

        # Check if source belongs to target's group (in-group)
        groups = self._agent_groups.get(source, set())
        if target in groups:
            new_val = max(DEFAULT_IN_GROUP_TRUST, min(1.0, new_val))
        else:
            new_val = max(MIN_OUT_GROUP_TRUST, min(1.0, new_val))

        record.trust_value = new_val
        record.interaction_count += 1

    def _adjust_agent_group_trust(
        self,
        source: str,
        target: str,
        delta: float,
    ) -> None:
        """Adjust agent→group trust if a record already exists.

        This ensures that event-driven updates are visible via get_trust(),
        which checks the agent→group store first.
        """
        key = (source, target)
        record = self._agent_group_trust.get(key)
        if record is None:
            return  # No existing record — nothing to update

        new_val = record.trust_value + delta

        # Apply the same in-group / out-group bounds as _adjust_group_trust
        groups = self._agent_groups.get(source, set())
        if target in groups:
            new_val = max(DEFAULT_IN_GROUP_TRUST, min(1.0, new_val))
        else:
            new_val = max(MIN_OUT_GROUP_TRUST, min(1.0, new_val))

        record.trust_value = new_val
        record.interaction_count += 1

    @staticmethod
    def _event_delta(event_type: InterGroupEventType, intensity: float) -> float:
        """Compute trust delta from an event type."""
        positive = {
            InterGroupEventType.TRADE_SUCCESS,
            InterGroupEventType.COOPERATION,
            InterGroupEventType.ALLIANCE_FORMED,
        }
        negative = {
            InterGroupEventType.TRADE_FAILURE,
            InterGroupEventType.CONFLICT,
            InterGroupEventType.BETRAYAL,
            InterGroupEventType.ALLIANCE_BROKEN,
        }

        if event_type in positive:
            return TRUST_EVENT_DELTA * intensity
        elif event_type in negative:
            return -TRUST_EVENT_DELTA * intensity
        return 0.0
