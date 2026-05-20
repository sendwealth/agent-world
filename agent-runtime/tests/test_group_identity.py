"""Unit tests for Phase 4.3.3 — Group Identity modules.

Covers:
- OrgCultureSystem (org culture computation, pressure, drift)
- RegionalCulture (clustering, boundaries)
- IntergroupTrust (in/out-group, events)
- CulturalConflictAndFusion (conflict detection, fusion, diversity index)
"""

import pytest

from agent_runtime.models.personality import PersonalityVector
from agent_runtime.models.values import ValueWeights
from agent_runtime.social.org_culture import OrgCultureSystem, CultureVector, MAX_CULTURE_PRESSURE_PER_TICK
from agent_runtime.social.regional_culture import RegionalCulture, Cluster
from agent_runtime.social.intergroup_trust import (
    IntergroupTrust,
    InterGroupEvent,
    InterGroupEventType,
    MIN_OUT_GROUP_TRUST,
    DEFAULT_IN_GROUP_TRUST,
    DEFAULT_OUT_GROUP_TRUST,
)
from agent_runtime.social.cultural_conflict import (
    CulturalConflictAndFusion,
    AgentInteraction,
    ConflictReport,
    CONFLICT_THRESHOLD,
    MAX_FUSION_DELTA,
)


# ── Helpers ──

def _make_pv(**overrides) -> PersonalityVector:
    defaults = {d: 0.5 for d in PersonalityVector._dimension_names()}
    defaults.update(overrides)
    return PersonalityVector(**defaults)


def _make_vw(**overrides) -> ValueWeights:
    defaults = {d: 0.5 for d in ValueWeights._dimension_names()}
    defaults.update(overrides)
    return ValueWeights(**defaults)


# ══════════════════════════════════════════════════════════════
# 1. OrgCultureSystem tests
# ══════════════════════════════════════════════════════════════


class TestOrgCultureSystem:
    """Tests for OrgCultureSystem."""

    def test_compute_org_culture_empty(self):
        """Empty member list returns default CultureVector."""
        system = OrgCultureSystem()
        culture = system.compute_org_culture("org_1", [])
        assert culture.cooperation_norm == 0.5

    def test_compute_org_culture_averages_members(self):
        """Org culture is the average of member value weights."""
        system = OrgCultureSystem()
        members = [
            _make_vw(cooperation_weight=0.8, competition_weight=0.2),
            _make_vw(cooperation_weight=0.4, competition_weight=0.6),
        ]
        culture = system.compute_org_culture("org_1", members)
        assert abs(culture.cooperation_norm - 0.6) < 1e-9
        assert abs(culture.competition_norm - 0.4) < 1e-9

    def test_culture_pressure_bounded(self):
        """Culture pressure per tick never exceeds MAX_CULTURE_PRESSURE_PER_TICK."""
        system = OrgCultureSystem()
        system.compute_org_culture("org_1", [_make_vw(cooperation_weight=0.9)])

        agent_values = _make_vw(cooperation_weight=0.1)
        result = system.apply_culture_pressure(agent_values, "org_1")

        for dim, delta in result["adjustments"].items():
            assert abs(delta) <= MAX_CULTURE_PRESSURE_PER_TICK + 1e-9

    def test_culture_pressure_moves_toward_org(self):
        """Agent values move toward the org culture after pressure."""
        system = OrgCultureSystem()
        system.compute_org_culture("org_1", [_make_vw(cooperation_weight=0.9)])

        agent_values = _make_vw(cooperation_weight=0.1)
        initial = agent_values.cooperation_weight
        result = system.apply_culture_pressure(agent_values, "org_1")

        assert result["updated_values"].cooperation_weight > initial

    def test_culture_drift_small(self):
        """Culture drift produces small random changes."""
        system = OrgCultureSystem()
        system.compute_org_culture("org_1", [_make_vw()])

        culture_before = system.get_org_culture("org_1")
        result = system.culture_drift("org_1", tick=100)
        culture_after = system.get_org_culture("org_1")

        # Drift should be very small
        if culture_before and culture_after:
            dist = culture_before.distance(culture_after)
            assert dist < 0.01  # very small drift per tick


# ══════════════════════════════════════════════════════════════
# 2. RegionalCulture tests
# ══════════════════════════════════════════════════════════════


class TestRegionalCulture:
    """Tests for RegionalCulture."""

    def test_compute_regional_culture_empty(self):
        """Empty region returns agent_count=0."""
        rc = RegionalCulture()
        result = rc.compute_regional_culture("region_1", [])
        assert result["agent_count"] == 0

    def test_compute_regional_culture_averages(self):
        """Regional culture aggregates personality and values."""
        rc = RegionalCulture()
        agents = [
            {"personality": _make_pv(openness=0.8), "values": _make_vw(cooperation_weight=0.7)},
            {"personality": _make_pv(openness=0.4), "values": _make_vw(cooperation_weight=0.3)},
        ]
        result = rc.compute_regional_culture("region_1", agents)
        assert result["agent_count"] == 2
        assert abs(result["aggregate_personality"]["openness"] - 0.6) < 1e-9

    def test_detect_clusters_fewer_agents_than_k(self):
        """When agents < n_clusters, each agent is its own cluster."""
        rc = RegionalCulture(n_clusters=5)
        agents = [
            {"id": "a1", "personality": _make_pv(), "values": _make_vw()},
            {"id": "a2", "personality": _make_pv(), "values": _make_vw()},
        ]
        clusters = rc.detect_cultural_clusters(agents)
        assert len(clusters) == 2
        assert clusters[0].agent_ids == ["a1"]
        assert clusters[1].agent_ids == ["a2"]

    def test_detect_clusters_groups_similar_agents(self):
        """Clustering groups similar agents together."""
        rc = RegionalCulture(n_clusters=2, max_iterations=50)
        agents = [
            {"id": f"a{i}", "personality": _make_pv(openness=0.1 if i < 5 else 0.9), "values": _make_vw()}
            for i in range(10)
        ]
        clusters = rc.detect_cultural_clusters(agents)

        assert len(clusters) == 2
        total_agents = sum(len(c.agent_ids) for c in clusters)
        assert total_agents == 10

    def test_cluster_boundary_computation(self):
        """Boundary between clusters reports correct distances."""
        rc = RegionalCulture()
        c_a = Cluster(
            cluster_id="ca",
            center_personality=_make_pv(openness=0.2),
            center_values=_make_vw(cooperation_weight=0.8, competition_weight=0.2),
            agent_ids=["a1"],
        )
        c_b = Cluster(
            cluster_id="cb",
            center_personality=_make_pv(openness=0.8),
            center_values=_make_vw(cooperation_weight=0.2, competition_weight=0.8),
            agent_ids=["a2"],
        )
        boundary = rc.get_cluster_boundary(c_a, c_b)
        assert boundary["cluster_a"] == "ca"
        assert boundary["cluster_b"] == "cb"
        assert boundary["personality_distance"] > 0
        assert len(boundary["top_differences"]) <= 3


# ══════════════════════════════════════════════════════════════
# 3. IntergroupTrust tests
# ══════════════════════════════════════════════════════════════


class TestIntergroupTrust:
    """Tests for IntergroupTrust."""

    def test_in_group_trust_default(self):
        """In-group trust defaults to DEFAULT_IN_GROUP_TRUST."""
        trust = IntergroupTrust()
        val = trust.compute_in_group_trust("agent_1", "group_a")
        assert val == DEFAULT_IN_GROUP_TRUST

    def test_out_group_trust_default(self):
        """Out-group trust defaults to DEFAULT_OUT_GROUP_TRUST."""
        trust = IntergroupTrust()
        val = trust.compute_out_group_trust("agent_1", "group_b")
        assert val == DEFAULT_OUT_GROUP_TRUST

    def test_out_group_trust_minimum(self):
        """Out-group trust never falls below MIN_OUT_GROUP_TRUST."""
        trust = IntergroupTrust()
        trust.register_membership("agent_1", "group_a")

        # Apply many negative events
        for _ in range(100):
            event = InterGroupEvent(
                event_type=InterGroupEventType.BETRAYAL,
                source_group="group_a",
                target_group="group_b",
                intensity=2.0,
            )
            trust.update_trust_from_event(event)

        val = trust.compute_out_group_trust("agent_1", "group_b")
        assert val >= MIN_OUT_GROUP_TRUST

    def test_positive_event_increases_trust(self):
        """Cooperation events increase inter-group trust."""
        trust = IntergroupTrust()

        before = trust.get_trust("group_a", "group_b")
        event = InterGroupEvent(
            event_type=InterGroupEventType.COOPERATION,
            source_group="group_a",
            target_group="group_b",
        )
        trust.update_trust_from_event(event)
        after = trust.get_trust("group_a", "group_b")

        assert after > before

    def test_negative_event_decreases_trust(self):
        """Conflict events decrease inter-group trust (but floor at MIN_OUT_GROUP_TRUST)."""
        trust = IntergroupTrust()

        # Set initial trust higher
        event_up = InterGroupEvent(
            event_type=InterGroupEventType.COOPERATION,
            source_group="group_a",
            target_group="group_b",
            intensity=2.0,
        )
        for _ in range(20):
            trust.update_trust_from_event(event_up)

        before = trust.get_trust("group_a", "group_b")
        event_down = InterGroupEvent(
            event_type=InterGroupEventType.CONFLICT,
            source_group="group_a",
            target_group="group_b",
        )
        trust.update_trust_from_event(event_down)
        after = trust.get_trust("group_a", "group_b")

        assert after < before

    def test_membership_tracking(self):
        """Agent group memberships are tracked correctly."""
        trust = IntergroupTrust()
        trust.register_membership("agent_1", "group_a")
        trust.register_membership("agent_1", "group_b")

        groups = trust.get_agent_groups("agent_1")
        assert groups == {"group_a", "group_b"}


# ══════════════════════════════════════════════════════════════
# 4. CulturalConflictAndFusion tests
# ══════════════════════════════════════════════════════════════


class TestCulturalConflictAndFusion:
    """Tests for CulturalConflictAndFusion."""

    def test_no_conflict_for_similar_agents(self):
        """Similar agents produce no conflict."""
        ccf = CulturalConflictAndFusion()
        interaction = AgentInteraction(
            agent_a_id="a1",
            agent_b_id="a2",
            agent_a_values=_make_vw(cooperation_weight=0.5),
            agent_b_values=_make_vw(cooperation_weight=0.5),
        )
        report = ccf.detect_cultural_conflict(interaction)
        assert report is None

    def test_conflict_detected_for_different_agents(self):
        """Agents with large value differences produce a conflict report."""
        ccf = CulturalConflictAndFusion()
        interaction = AgentInteraction(
            agent_a_id="a1",
            agent_b_id="a2",
            agent_a_values=_make_vw(cooperation_weight=0.9, competition_weight=0.1),
            agent_b_values=_make_vw(cooperation_weight=0.1, competition_weight=0.9),
        )
        report = ccf.detect_cultural_conflict(interaction)
        assert report is not None
        assert report.conflict_score > 0
        assert len(report.conflicting_dimensions) > 0

    def test_fusion_blends_border_agents(self):
        """Fusion effect moves border agents toward neighbor averages."""
        ccf = CulturalConflictAndFusion()
        agent_values = _make_vw(cooperation_weight=0.2)
        neighbors = [_make_vw(cooperation_weight=0.8)]

        result = ccf.apply_fusion_effect([
            {"id": "a1", "values": agent_values, "neighbor_values": neighbors},
        ])

        assert result["affected_agents"] == 1
        updated = result["updated_values"]["a1"]
        assert updated.cooperation_weight > 0.2
        assert updated.cooperation_weight <= 0.2 + MAX_FUSION_DELTA + 1e-9

    def test_diversity_index_homogeneous(self):
        """Identical agents produce diversity index = 0."""
        ccf = CulturalConflictAndFusion()
        agents = [{"values": _make_vw()} for _ in range(5)]
        idx = ccf.compute_cultural_diversity_index(agents)
        assert idx == 0.0

    def test_diversity_index_bounded(self):
        """Diversity index is always in [0, 1]."""
        ccf = CulturalConflictAndFusion()
        agents = [
            {"values": _make_vw(**{d: 0.0 if i % 2 == 0 else 1.0 for d in ValueWeights._dimension_names()})}
            for i in range(10)
        ]
        idx = ccf.compute_cultural_diversity_index(agents)
        assert 0.0 <= idx <= 1.0
        assert idx > 0  # diverse agents should have positive index

    def test_conflict_history_tracking(self):
        """Conflicts are tracked in history."""
        ccf = CulturalConflictAndFusion()
        interaction = AgentInteraction(
            agent_a_id="a1",
            agent_b_id="a2",
            agent_a_values=_make_vw(cooperation_weight=0.9),
            agent_b_values=_make_vw(cooperation_weight=0.1),
        )
        ccf.detect_cultural_conflict(interaction)
        assert len(ccf.conflict_history) == 1
        assert ccf.conflict_history[0].agent_a_id == "a1"

    def test_get_conflicts_for_agent(self):
        """Can query conflicts involving a specific agent."""
        ccf = CulturalConflictAndFusion()
        interaction = AgentInteraction(
            agent_a_id="a1",
            agent_b_id="a2",
            agent_a_values=_make_vw(cooperation_weight=0.9),
            agent_b_values=_make_vw(cooperation_weight=0.1),
        )
        ccf.detect_cultural_conflict(interaction)

        a1_conflicts = ccf.get_conflicts_for_agent("a1")
        assert len(a1_conflicts) == 1

        a3_conflicts = ccf.get_conflicts_for_agent("a3")
        assert len(a3_conflicts) == 0
