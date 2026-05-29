"""End-to-end tests for the autonomous self-legislation cycle.

Validates the full cycle: election → analysis → proposal → voting → enactment → rule execution.
Also verifies governance metrics collection and leader election influence on legislation.
"""

from __future__ import annotations

import uuid
from typing import Any

import pytest

from agent_runtime.organization.governance_analysis import (
    GovernanceAnalyzer,
    GovernanceEventData,
    OrgGovernanceSnapshot,
    StabilityReport,
)
from agent_runtime.organization.rule_proposal import (
    RuleCategory,
    RuleCondition,
    RuleEffect,
    RuleProposal,
    RuleProposalEngine,
)
from agent_runtime.organization.self_legislation import (
    InMemoryRuleEngineClient,
    LegislationCycle,
    LegislationStatus,
    SelfLegislationCycleEngine,
)


# ═══════════════════════════════════════════════════════════════
# Test Helpers
# ═══════════════════════════════════════════════════════════════


def _make_org_snapshot(
    org_id: str = "org-1",
    member_count: int = 5,
    total_elections: int = 1,
    total_leadership_changes: int = 1,
    total_tax_collected: int = 500,
    total_tax_events: int = 5,
    treaties_signed: int = 2,
    treaties_broken: int = 0,
) -> OrgGovernanceSnapshot:
    """Create a governance snapshot for testing."""
    return OrgGovernanceSnapshot(
        org_id=org_id,
        member_count=member_count,
        total_elections=total_elections,
        total_leadership_changes=total_leadership_changes,
        total_tax_collected=total_tax_collected,
        total_tax_events=total_tax_events,
        treaties_signed=treaties_signed,
        treaties_broken=treaties_broken,
    )


def _make_events(org_id: str = "org-1", count: int = 5) -> list[GovernanceEventData]:
    """Create sample governance events."""
    events: list[GovernanceEventData] = []
    for i in range(count):
        events.append(
            GovernanceEventData(
                event_type="election" if i % 3 == 0 else "tax_collected",
                org_id=org_id,
                tick=i * 10,
                details={"amount": 50 + i * 10, "agent_id": f"agent-{i}"},
            )
        )
    return events


def _inequality_context(
    members: int = 5,
    low_resources: int = 10,
    high_resources: int = 10000,
) -> dict[str, Any]:
    """Create a context that triggers inequality-based legislation (Gini > 0.4)."""
    member_list: list[dict[str, int]] = []
    for i in range(members):
        # First ~half have very low resources, rest have very high → high Gini
        resources = low_resources if i < (members + 1) // 2 else high_resources
        member_list.append({"id": f"agent-{i}", "resources": resources})
    return {
        "members": member_list,
        "recent_attacks": 0,
        "economic_output": 100,
        "expected_output": 100,
        "rival_org_expansion": False,
    }


def _safety_context(recent_attacks: int = 10) -> dict[str, Any]:
    """Create a context that triggers safety-based legislation."""
    return {
        "members": [
            {"id": "a1", "resources": 100},
            {"id": "a2", "resources": 100},
        ],
        "recent_attacks": recent_attacks,
    }


# ═══════════════════════════════════════════════════════════════
# Test: Full Legislation Cycle
# ═══════════════════════════════════════════════════════════════


class TestFullLegislationCycle:
    """E2E: Agent proposes rule → members vote → rule enacted and activated."""

    def test_elected_leader_proposes_inequality_rule(self) -> None:
        """An elected leader identifies inequality and proposes a tax rule."""
        engine = SelfLegislationCycleEngine(quorum=3)
        org_id = "org-1"
        leader_id = "leader-agent"

        snapshot = _make_org_snapshot(org_id=org_id)
        events = _make_events(org_id)
        context = _inequality_context()

        cycle = engine.start_cycle(
            leader_id=leader_id,
            org_id=org_id,
            org_snapshot=snapshot,
            events=events,
            context=context,
        )

        assert cycle is not None, "Leader should propose a rule when inequality is detected"
        assert cycle.status == LegislationStatus.CAMPAIGNING
        assert cycle.leader_id == leader_id
        assert cycle.org_id == org_id
        assert cycle.proposal is not None
        assert cycle.proposal.rule_type == RuleCategory.TAX
        assert "inequality" in cycle.trigger_reason.lower()

    def test_members_vote_and_rule_is_enacted(self) -> None:
        """Members vote on a proposal, it meets quorum, and is enacted."""
        engine = SelfLegislationCycleEngine(quorum=3)
        client = InMemoryRuleEngineClient()

        org_id = "org-1"
        leader_id = "leader-agent"

        snapshot = _make_org_snapshot(org_id=org_id)
        events = _make_events(org_id)
        context = _inequality_context()

        # Start the cycle
        cycle = engine.start_cycle(
            leader_id=leader_id,
            org_id=org_id,
            org_snapshot=snapshot,
            events=events,
            context=context,
        )
        assert cycle is not None

        # Members vote — 3 for, 1 against (meets quorum=3 and majority)
        cycle = engine.cast_vote(cycle, "member-1", support=True)
        cycle = engine.cast_vote(cycle, "member-2", support=True)
        cycle = engine.cast_vote(cycle, "member-3", support=True)
        cycle = engine.cast_vote(cycle, "member-4", support=False)

        assert cycle.status == LegislationStatus.VOTING
        assert cycle.votes_for == 3
        assert cycle.votes_against == 1
        assert len(cycle.voters) == 4

        # Finalize
        result = engine.finalize_cycle(cycle, client)

        assert result.status == LegislationStatus.ENACTED
        assert result.enacted_rule_id is not None
        assert result.votes_for == 3
        assert result.votes_against == 1

    def test_rule_is_active_in_engine_after_enactment(self) -> None:
        """After enactment, the rule exists in the engine as active."""
        engine = SelfLegislationCycleEngine(quorum=3)
        client = InMemoryRuleEngineClient()

        context = _inequality_context()

        cycle = engine.start_cycle(
            leader_id="leader-1",
            org_id="org-1",
            org_snapshot=_make_org_snapshot(),
            events=_make_events(),
            context=context,
        )
        assert cycle is not None

        # Vote to pass
        cycle = engine.cast_vote(cycle, "v1", True)
        cycle = engine.cast_vote(cycle, "v2", True)
        cycle = engine.cast_vote(cycle, "v3", True)

        result = engine.finalize_cycle(cycle, client)

        assert result.status == LegislationStatus.ENACTED
        rule_id = result.enacted_rule_id
        assert rule_id is not None

        # Verify the rule is in the engine and active
        rule = client._rules[rule_id]
        assert rule["status"] == "active"

    def test_rejected_when_majority_opposes(self) -> None:
        """Rule is rejected when more members vote against than for."""
        engine = SelfLegislationCycleEngine(quorum=3)
        client = InMemoryRuleEngineClient()

        context = _inequality_context()

        cycle = engine.start_cycle(
            leader_id="leader-1",
            org_id="org-1",
            org_snapshot=_make_org_snapshot(),
            events=_make_events(),
            context=context,
        )
        assert cycle is not None

        # Vote against: 1 for, 3 against
        cycle = engine.cast_vote(cycle, "v1", True)
        cycle = engine.cast_vote(cycle, "v2", False)
        cycle = engine.cast_vote(cycle, "v3", False)
        cycle = engine.cast_vote(cycle, "v4", False)

        result = engine.finalize_cycle(cycle, client)

        assert result.status == LegislationStatus.REJECTED
        assert result.enacted_rule_id is None

    def test_rejected_when_quorum_not_met(self) -> None:
        """Rule is rejected when quorum is not reached."""
        engine = SelfLegislationCycleEngine(quorum=5)
        client = InMemoryRuleEngineClient()

        context = _inequality_context()

        cycle = engine.start_cycle(
            leader_id="leader-1",
            org_id="org-1",
            org_snapshot=_make_org_snapshot(),
            events=_make_events(),
            context=context,
        )
        assert cycle is not None

        # Only 2 votes (quorum=5 not met), both for
        cycle = engine.cast_vote(cycle, "v1", True)
        cycle = engine.cast_vote(cycle, "v2", True)

        result = engine.finalize_cycle(cycle, client)

        # Even though 2-for and 0-against, quorum not met → rejected
        assert result.status == LegislationStatus.REJECTED

    def test_no_legislation_when_stable(self) -> None:
        """No legislation is proposed when the org is stable."""
        engine = SelfLegislationCycleEngine()
        context = {
            "members": [
                {"id": "a1", "resources": 100},
                {"id": "a2", "resources": 110},
            ],
            "recent_attacks": 0,
            "economic_output": 100,
            "expected_output": 100,
            "rival_org_expansion": False,
        }

        cycle = engine.start_cycle(
            leader_id="leader-1",
            org_id="org-1",
            org_snapshot=_make_org_snapshot(),
            events=_make_events(),
            context=context,
        )

        assert cycle is None, "No legislation should be proposed in a stable org"


class TestDuplicateVoting:
    """Test that duplicate votes are properly handled."""

    def test_duplicate_vote_ignored(self) -> None:
        """Duplicate votes from the same voter are ignored."""
        engine = SelfLegislationCycleEngine(quorum=3)
        client = InMemoryRuleEngineClient()

        context = _inequality_context()

        cycle = engine.start_cycle(
            leader_id="leader-1",
            org_id="org-1",
            org_snapshot=_make_org_snapshot(),
            events=_make_events(),
            context=context,
        )
        assert cycle is not None

        cycle = engine.cast_vote(cycle, "v1", True)
        cycle = engine.cast_vote(cycle, "v1", True)  # duplicate

        assert cycle.votes_for == 1
        assert len(cycle.voters) == 1


class TestRunFullCycle:
    """Test the run_full_cycle convenience method."""

    def test_run_full_cycle_success(self) -> None:
        """Full cycle completes in one call with member votes."""
        engine = SelfLegislationCycleEngine(quorum=3)
        client = InMemoryRuleEngineClient()

        member_votes = {
            "member-1": True,
            "member-2": True,
            "member-3": True,
            "member-4": False,
        }

        result = engine.run_full_cycle(
            leader_id="leader-1",
            org_id="org-1",
            org_snapshot=_make_org_snapshot(),
            events=_make_events(),
            context=_inequality_context(),
            member_votes=member_votes,
            client=client,
        )

        assert result is not None
        assert result.status == LegislationStatus.ENACTED
        assert result.enacted_rule_id is not None

    def test_run_full_cycle_no_legislation(self) -> None:
        """Full cycle returns None when no legislation is needed."""
        engine = SelfLegislationCycleEngine(quorum=3)
        client = InMemoryRuleEngineClient()

        stable_context = {
            "members": [
                {"id": "a1", "resources": 100},
                {"id": "a2", "resources": 110},
            ],
            "recent_attacks": 0,
            "economic_output": 100,
            "expected_output": 100,
        }

        result = engine.run_full_cycle(
            leader_id="leader-1",
            org_id="org-1",
            org_snapshot=_make_org_snapshot(),
            events=_make_events(),
            context=stable_context,
            member_votes={"m1": True, "m2": True, "m3": True},
            client=client,
        )

        assert result is None


# ═══════════════════════════════════════════════════════════════
# Test: Governance Metrics Collection
# ═══════════════════════════════════════════════════════════════


class TestGovernanceMetricsCollection:
    """Verify governance analysis metrics are correctly collected during legislation."""

    def test_stability_report_generated_during_cycle(self) -> None:
        """A stability report is generated when starting a legislation cycle."""
        engine = SelfLegislationCycleEngine(quorum=3)

        snapshot = _make_org_snapshot(
            total_elections=3,
            total_leadership_changes=2,
            treaties_signed=5,
            treaties_broken=1,
        )
        events = _make_events(count=10)

        cycle = engine.start_cycle(
            leader_id="leader-1",
            org_id="org-1",
            org_snapshot=snapshot,
            events=events,
            context=_inequality_context(),
        )
        assert cycle is not None
        assert cycle.stability_report is not None
        # The stability report should reflect the org's governance state
        assert 0.0 <= cycle.stability_report.stability_score <= 1.0

    def test_campaign_message_generated(self) -> None:
        """A campaign message is generated when the cycle starts."""
        engine = SelfLegislationCycleEngine(quorum=3)

        cycle = engine.start_cycle(
            leader_id="leader-1",
            org_id="org-1",
            org_snapshot=_make_org_snapshot(),
            events=_make_events(),
            context=_inequality_context(),
        )
        assert cycle is not None

        campaign = engine.last_campaign_message
        assert len(campaign) > 0
        assert cycle.proposal is not None
        assert cycle.proposal.title in campaign

    def test_metrics_reflect_legislation_outcome(self) -> None:
        """After legislation, the InMemoryRuleEngineClient tracks the rule correctly."""
        engine = SelfLegislationCycleEngine(quorum=3)
        client = InMemoryRuleEngineClient()

        result = engine.run_full_cycle(
            leader_id="leader-1",
            org_id="org-1",
            org_snapshot=_make_org_snapshot(),
            events=_make_events(),
            context=_inequality_context(),
            member_votes={"m1": True, "m2": True, "m3": True},
            client=client,
        )

        assert result is not None
        assert result.status == LegislationStatus.ENACTED

        rule_id = result.enacted_rule_id
        assert rule_id is not None

        # Verify the client has exactly one active rule
        active_rules = [r for r in client._rules.values() if r["status"] == "active"]
        assert len(active_rules) == 1
        assert active_rules[0]["proposer_id"] == "leader-1"
        assert active_rules[0]["org_id"] == "org-1"

        # Verify votes were recorded
        votes = client._votes.get(rule_id, {})
        assert len(votes) == 3  # 3 members voted

    def test_multiple_cycles_tracked_separately(self) -> None:
        """Multiple legislation cycles for different orgs are tracked independently."""
        engine = SelfLegislationCycleEngine(quorum=2)
        client = InMemoryRuleEngineClient()

        # Org-1: inequality-based rule
        result1 = engine.run_full_cycle(
            leader_id="leader-1",
            org_id="org-1",
            org_snapshot=_make_org_snapshot(org_id="org-1"),
            events=_make_events(org_id="org-1"),
            context=_inequality_context(),
            member_votes={"m1": True, "m2": True},
            client=client,
        )

        # Org-2: safety-based rule
        result2 = engine.run_full_cycle(
            leader_id="leader-2",
            org_id="org-2",
            org_snapshot=_make_org_snapshot(org_id="org-2"),
            events=_make_events(org_id="org-2"),
            context=_safety_context(recent_attacks=15),
            member_votes={"m3": True, "m4": True},
            client=client,
        )

        assert result1 is not None
        assert result2 is not None
        assert result1.org_id == "org-1"
        assert result2.org_id == "org-2"
        assert result1.enacted_rule_id != result2.enacted_rule_id

        # Two active rules in the engine
        active_rules = [r for r in client._rules.values() if r["status"] == "active"]
        assert len(active_rules) == 2


# ═══════════════════════════════════════════════════════════════
# Test: Leader Election Influence on Legislation
# ═══════════════════════════════════════════════════════════════


class TestLeaderElectionInfluence:
    """Verify that the elected leader is the one who proposes and drives legislation."""

    def test_leader_is_the_proposer(self) -> None:
        """The elected leader's ID is recorded as the proposer in the legislation cycle."""
        engine = SelfLegislationCycleEngine(quorum=3)

        cycle = engine.start_cycle(
            leader_id="elected-leader-42",
            org_id="org-1",
            org_snapshot=_make_org_snapshot(),
            events=_make_events(),
            context=_inequality_context(),
        )

        assert cycle is not None
        assert cycle.leader_id == "elected-leader-42"
        assert cycle.proposal is not None
        assert cycle.proposal.proposer_id == "elected-leader-42"

    def test_different_leaders_propose_different_rules(self) -> None:
        """Different elected leaders can propose rules for the same org."""
        engine = SelfLegislationCycleEngine(quorum=2)
        client = InMemoryRuleEngineClient()

        # First leader proposes
        result1 = engine.run_full_cycle(
            leader_id="leader-alpha",
            org_id="org-1",
            org_snapshot=_make_org_snapshot(),
            events=_make_events(),
            context=_inequality_context(),
            member_votes={"m1": True, "m2": True},
            client=client,
        )
        assert result1 is not None
        assert result1.leader_id == "leader-alpha"

        # New leader proposes after election
        result2 = engine.run_full_cycle(
            leader_id="leader-beta",
            org_id="org-1",
            org_snapshot=_make_org_snapshot(),
            events=_make_events(),
            context=_safety_context(recent_attacks=20),
            member_votes={"m1": True, "m2": True},
            client=client,
        )
        assert result2 is not None
        assert result2.leader_id == "leader-beta"

        # Both rules should be for the same org but from different leaders
        assert result1.org_id == result2.org_id
        assert result1.enacted_rule_id != result2.enacted_rule_id

    def test_leader_proposal_reflects_org_governance_state(self) -> None:
        """The leader's proposal is influenced by the org's governance snapshot."""
        engine = SelfLegislationCycleEngine(quorum=3)

        # Stable org with no attacks
        stable_snapshot = _make_org_snapshot(
            total_elections=1,
            total_leadership_changes=0,
            treaties_signed=10,
            treaties_broken=0,
        )
        # Turbulent org with many changes
        turbulent_snapshot = _make_org_snapshot(
            total_elections=10,
            total_leadership_changes=8,
            treaties_signed=3,
            treaties_broken=5,
        )

        stable_events = [
            GovernanceEventData(
                event_type="election", org_id="org-1", tick=i, details={}
            )
            for i in range(2)
        ]
        turbulent_events = [
            GovernanceEventData(
                event_type="election" if i % 2 == 0 else "leader_changed",
                org_id="org-1",
                tick=i,
                details={},
            )
            for i in range(20)
        ]

        cycle_stable = engine.start_cycle(
            leader_id="leader-1",
            org_id="org-1",
            org_snapshot=stable_snapshot,
            events=stable_events,
            context=_inequality_context(),
        )
        cycle_turbulent = engine.start_cycle(
            leader_id="leader-1",
            org_id="org-1",
            org_snapshot=turbulent_snapshot,
            events=turbulent_events,
            context=_inequality_context(),
        )

        # Both should produce legislation (inequality trigger), but
        # the stability reports should differ
        if cycle_stable is not None and cycle_turbulent is not None:
            assert cycle_stable.stability_report is not None
            assert cycle_turbulent.stability_report is not None
            # Turbulent org should have lower stability
            assert (
                cycle_turbulent.stability_report.stability_score
                <= cycle_stable.stability_report.stability_score
            )


# ═══════════════════════════════════════════════════════════════
# Test: Rule Execution After Enactment
# ═══════════════════════════════════════════════════════════════


class TestRuleExecutionAfterEnactment:
    """Verify that enacted rules take effect in the simulation."""

    def test_enacted_rule_conditions_and_effects_are_valid(self) -> None:
        """The enacted rule has properly structured conditions and effects."""
        engine = SelfLegislationCycleEngine(quorum=3)
        client = InMemoryRuleEngineClient()

        result = engine.run_full_cycle(
            leader_id="leader-1",
            org_id="org-1",
            org_snapshot=_make_org_snapshot(),
            events=_make_events(),
            context=_inequality_context(),
            member_votes={"m1": True, "m2": True, "m3": True},
            client=client,
        )

        assert result is not None
        assert result.status == LegislationStatus.ENACTED
        assert result.proposal is not None

        # Verify the proposal has valid conditions and effects
        proposal = result.proposal
        assert len(proposal.conditions) > 0
        assert len(proposal.effects) > 0

        for condition in proposal.conditions:
            assert condition.field
            assert condition.operator in (">", "<", "==", ">=", "<=", "contains")
            assert condition.value is not None

        for effect in proposal.effects:
            assert effect.target
            assert effect.action in ("set", "add", "subtract", "multiply", "block_action")

    def test_rule_submitted_to_engine_matches_proposal(self) -> None:
        """The rule stored in the engine matches the original proposal."""
        engine = SelfLegislationCycleEngine(quorum=3)
        client = InMemoryRuleEngineClient()

        result = engine.run_full_cycle(
            leader_id="leader-1",
            org_id="org-1",
            org_snapshot=_make_org_snapshot(),
            events=_make_events(),
            context=_inequality_context(),
            member_votes={"m1": True, "m2": True, "m3": True},
            client=client,
        )

        assert result is not None
        rule_id = result.enacted_rule_id
        assert rule_id is not None

        rule = client._rules[rule_id]
        assert rule["proposer_id"] == result.proposal.proposer_id
        assert rule["org_id"] == result.proposal.org_id
        assert rule["title"] == result.proposal.title
        assert rule["rule_type"] == result.proposal.rule_type.value
        assert rule["status"] == "active"


# ═══════════════════════════════════════════════════════════════
# Test: Multi-Tick Simulation
# ═══════════════════════════════════════════════════════════════


class TestMultiTickLegislationSimulation:
    """Simulate a multi-tick scenario where agents legislate across time.

    This test mirrors the governance simulation from test_self_governance_integration.py
    but adds the self-legislation layer on top.
    """

    def test_legislation_over_100_ticks(self) -> None:
        """Simulate legislation events over 100 ticks with voting cycles."""
        engine = SelfLegislationCycleEngine(quorum=3)
        client = InMemoryRuleEngineClient()

        org_id = "org-1"
        leader_id = "leader-agent"
        members = ["member-1", "member-2", "member-3", "member-4", "member-5"]

        enacted_cycles: list[LegislationCycle] = []
        rejected_cycles: list[LegislationCycle] = []

        for tick in range(0, 100, 10):
            # Vary the context to sometimes trigger legislation
            if tick % 20 == 0:
                # Inequality trigger
                context = _inequality_context()
            elif tick % 20 == 10:
                # Safety trigger
                context = _safety_context(recent_attacks=10 + tick // 10)
            else:
                continue

            snapshot = _make_org_snapshot(org_id=org_id)
            events = _make_events(org_id, count=tick // 10 + 1)

            cycle = engine.start_cycle(
                leader_id=leader_id,
                org_id=org_id,
                org_snapshot=snapshot,
                events=events,
                context=context,
            )

            if cycle is None:
                continue

            # Members vote — alternate voting patterns
            for i, member in enumerate(members):
                support = i < 3  # First 3 support, last 2 oppose
                cycle = engine.cast_vote(cycle, member, support)

            result = engine.finalize_cycle(cycle, client)

            if result.status == LegislationStatus.ENACTED:
                enacted_cycles.append(result)
            elif result.status == LegislationStatus.REJECTED:
                rejected_cycles.append(result)

        # Verify legislation happened across the simulation
        total_cycles = len(enacted_cycles) + len(rejected_cycles)
        assert total_cycles > 0, "Expected legislation cycles to occur over 100 ticks"

        # Most should pass since we have 3-for/2-against pattern
        assert len(enacted_cycles) > 0, "Expected some rules to be enacted"

        # Verify all enacted rules are in the engine
        active_rules = [r for r in client._rules.values() if r["status"] == "active"]
        assert len(active_rules) == len(enacted_cycles)

        # Verify all enacted rules belong to the org
        for rule in active_rules:
            assert rule["org_id"] == org_id

    def test_three_orgs_legislation_over_50_ticks(self) -> None:
        """Three orgs legislate independently over 50 ticks."""
        engine = SelfLegislationCycleEngine(quorum=2)
        client = InMemoryRuleEngineClient()

        orgs = {
            "org-miners": {
                "leader": "miner-leader",
                "members": ["m1", "m2", "m3"],
                "context": _inequality_context(members=3),
            },
            "org-traders": {
                "leader": "trader-leader",
                "members": ["t1", "t2", "t3"],
                "context": _safety_context(recent_attacks=12),
            },
            "org-guards": {
                "leader": "guard-leader",
                "members": ["g1", "g2", "g3"],
                "context": {
                    "members": [
                        {"id": "g1", "resources": 100},
                        {"id": "g2", "resources": 100},
                    ],
                    "rival_org_expansion": True,
                },
            },
        }

        enacted_per_org: dict[str, int] = {oid: 0 for oid in orgs}

        for tick in range(0, 50, 10):
            for org_id, org_data in orgs.items():
                snapshot = _make_org_snapshot(org_id=org_id)
                events = _make_events(org_id, count=3)

                cycle = engine.start_cycle(
                    leader_id=org_data["leader"],
                    org_id=org_id,
                    org_snapshot=snapshot,
                    events=events,
                    context=org_data["context"],
                )
                if cycle is None:
                    continue

                for member in org_data["members"]:
                    cycle = engine.cast_vote(cycle, member, True)

                result = engine.finalize_cycle(cycle, client)
                if result.status == LegislationStatus.ENACTED:
                    enacted_per_org[org_id] += 1

        # At least some orgs should have enacted rules
        total_enacted = sum(enacted_per_org.values())
        assert total_enacted > 0, "Expected at least one org to enact rules"

        # Verify rules in engine belong to correct orgs
        active_rules = [r for r in client._rules.values() if r["status"] == "active"]
        assert len(active_rules) == total_enacted

        org_ids_in_rules = {r["org_id"] for r in active_rules}
        assert len(org_ids_in_rules) > 0


# ═══════════════════════════════════════════════════════════════
# Test: LegislationHistory Tracking
# ═══════════════════════════════════════════════════════════════


class TestLegislationHistoryTracking:
    """Test that legislation cycle history is properly maintained."""

    def test_cycle_tracks_trigger_reason(self) -> None:
        """Each legislation cycle records why it was triggered."""
        engine = SelfLegislationCycleEngine(quorum=3)

        # Test inequality trigger
        cycle = engine.start_cycle(
            leader_id="leader-1",
            org_id="org-1",
            org_snapshot=_make_org_snapshot(),
            events=_make_events(),
            context=_inequality_context(),
        )
        assert cycle is not None
        assert "inequality" in cycle.trigger_reason.lower()

    def test_cycle_tracks_safety_trigger(self) -> None:
        """Safety-based trigger reason is recorded."""
        engine = SelfLegislationCycleEngine(quorum=3, majority_ratio=0.5)

        cycle = engine.start_cycle(
            leader_id="leader-1",
            org_id="org-1",
            org_snapshot=_make_org_snapshot(),
            events=_make_events(),
            context=_safety_context(recent_attacks=15),
        )
        assert cycle is not None
        assert "safety" in cycle.trigger_reason.lower()

    def test_cycle_preserves_stability_report(self) -> None:
        """The governance stability report is preserved in the cycle."""
        engine = SelfLegislationCycleEngine(quorum=3)

        snapshot = _make_org_snapshot(
            total_elections=5,
            total_leadership_changes=3,
            treaties_signed=8,
            treaties_broken=2,
        )

        cycle = engine.start_cycle(
            leader_id="leader-1",
            org_id="org-1",
            org_snapshot=snapshot,
            events=_make_events(count=20),
            context=_inequality_context(),
        )
        assert cycle is not None
        assert cycle.stability_report is not None
        report = cycle.stability_report
        assert 0.0 <= report.stability_score <= 1.0

    def test_enacted_cycle_records_rule_id(self) -> None:
        """An enacted cycle records the rule ID from the engine."""
        engine = SelfLegislationCycleEngine(quorum=3)
        client = InMemoryRuleEngineClient()

        result = engine.run_full_cycle(
            leader_id="leader-1",
            org_id="org-1",
            org_snapshot=_make_org_snapshot(),
            events=_make_events(),
            context=_inequality_context(),
            member_votes={"m1": True, "m2": True, "m3": True},
            client=client,
        )

        assert result is not None
        assert result.status == LegislationStatus.ENACTED
        assert result.enacted_rule_id is not None
        assert result.enacted_rule_id.startswith("rule-")

    def test_rejected_cycle_has_no_rule_id(self) -> None:
        """A rejected cycle has no rule ID."""
        engine = SelfLegislationCycleEngine(quorum=5)
        client = InMemoryRuleEngineClient()

        result = engine.run_full_cycle(
            leader_id="leader-1",
            org_id="org-1",
            org_snapshot=_make_org_snapshot(),
            events=_make_events(),
            context=_inequality_context(),
            member_votes={"m1": True, "m2": True},  # only 2 votes, quorum=5
            client=client,
        )

        assert result is not None
        assert result.status == LegislationStatus.REJECTED
        assert result.enacted_rule_id is None
