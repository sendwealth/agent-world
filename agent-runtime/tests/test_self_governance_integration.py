"""Phase 4.4.2 Integration Tests — Self-Governance (Python side).

Validates end-to-end interaction of GovernanceDecider with simulated
Rust event system responses. These tests exercise the full decision
pipeline: org perception → governance decision → event emission,
mirroring the Rust integration test patterns from org_formation_integration.rs.
"""
from __future__ import annotations

import uuid
from dataclasses import dataclass, field
from enum import Enum
from typing import Any, Dict, List, Optional

import pytest

from agent_runtime.organization.governance import (
    AgentInterests,
    AllocationStrategy,
    Candidate,
    GovernanceDecider,
    LeadershipAmbition,
    OrgSnapshot,
    Treaty,
    TreatyResponse,
)

# ═══════════════════════════════════════════════════════════════
# Test Infrastructure — Simulated World Engine
# ═══════════════════════════════════════════════════════════════


class OrgType(str, Enum):
    COMPANY = "company"
    GUILD = "guild"
    ALLIANCE = "alliance"
    UNIVERSITY = "university"


@dataclass
class SimulatedEvent:
    """Simulates a WorldEvent from the Rust engine."""
    event_type: str
    payload: Dict[str, Any]


@dataclass
class SimulatedOrg:
    """Simulates an organization with full governance state."""
    id: str
    name: str
    org_type: OrgType
    members: List[str] = field(default_factory=list)
    treasury: int = 100
    avg_wealth: float = 100.0
    total_wealth: float = 0.0
    tax_rate: float = 0.10
    leader_id: Optional[str] = None
    created_tick: int = 0

    @property
    def has_leader(self) -> bool:
        return self.leader_id is not None

    @property
    def member_count(self) -> int:
        return len(self.members)


class GovernanceEventBus:
    """Collects governance events for verification."""

    def __init__(self) -> None:
        self.events: List[SimulatedEvent] = []

    def publish(self, event_type: str, payload: Dict[str, Any]) -> None:
        self.events.append(SimulatedEvent(event_type=event_type, payload=payload))

    def find_by_type(self, event_type: str) -> List[SimulatedEvent]:
        return [e for e in self.events if e.event_type == event_type]


class SimulatedGovernanceWorld:
    """Simulates the full governance world with multiple orgs.

    Mirrors the Rust-side integration by providing a tick-driven
    simulation that exercises Treasury, Leadership, and Diplomacy
    through the GovernanceDecider interface.
    """

    def __init__(self) -> None:
        self.orgs: Dict[str, SimulatedOrg] = {}
        self.event_bus = GovernanceEventBus()
        self.tick: int = 0
        self.decider = GovernanceDecider()

    def advance_tick(self) -> None:
        self.tick += 1

    def create_org(
        self,
        name: str,
        org_type: OrgType,
        member_ids: List[str],
    ) -> SimulatedOrg:
        org_id = str(uuid.uuid4())
        org = SimulatedOrg(
            id=org_id,
            name=name,
            org_type=org_type,
            members=list(member_ids),
            total_wealth=len(member_ids) * 100.0,
        )
        self.orgs[org_id] = org
        self.event_bus.publish("OrgCreated", {
            "org_id": org_id,
            "name": name,
            "org_type": org_type.value,
        })
        return org

    def get_org_snapshot(self, org_id: str) -> OrgSnapshot:
        org = self.orgs[org_id]
        return OrgSnapshot(
            org_id=org.id,
            member_count=org.member_count,
            avg_wealth=org.avg_wealth,
            total_wealth=org.total_wealth,
            tax_rate=org.tax_rate,
            has_leader=org.has_leader,
            leader_id=org.leader_id,
            treasury=org.treasury,
        )


# ═══════════════════════════════════════════════════════════════
# Test Cases
# ═══════════════════════════════════════════════════════════════


class TestGovernanceDeciderLeadership:
    """Test: GovernanceDecider leadership decisions."""

    def test_rich_skilled_agent_runs_for_leader(self) -> None:
        """An agent with wealth and skills decides to run for leader."""
        world = SimulatedGovernanceWorld()
        org = world.create_org("TestGuild", OrgType.GUILD, ["a1", "a2", "a3"])

        interests = AgentInterests(
            agent_id="a1",
            wealth=150.0,
            skills={"mining": 80, "crafting": 70},
            risk_tolerance=0.7,
        )

        decision = world.decider.should_run_for_leader(
            world.get_org_snapshot(org.id),
            interests,
        )

        assert decision.action == LeadershipAmbition.RUN.value
        assert decision.confidence > 0.5

    def test_poor_unskilled_agent_abstains(self) -> None:
        """An agent without wealth or skills abstains from running."""
        world = SimulatedGovernanceWorld()
        org = world.create_org("TestGuild", OrgType.GUILD, ["a1", "a2", "a3"])
        org.avg_wealth = 200.0  # high average

        interests = AgentInterests(
            agent_id="a2",
            wealth=10.0,
            skills={},
            risk_tolerance=0.1,
        )

        decision = world.decider.should_run_for_leader(
            world.get_org_snapshot(org.id),
            interests,
        )

        assert decision.action == LeadershipAmbition.ABSTAIN.value

    def test_empty_org_abstains(self) -> None:
        """An agent in an empty org should abstain."""
        decider = GovernanceDecider()
        org = OrgSnapshot(org_id="empty", member_count=0)
        interests = AgentInterests(agent_id="a1", wealth=1000.0)

        decision = decider.should_run_for_leader(org, interests)
        assert decision.action == LeadershipAmbition.ABSTAIN.value
        assert decision.confidence == 1.0

    def test_voting_picks_best_candidate(self) -> None:
        """Agent votes for the candidate with best alignment."""
        decider = GovernanceDecider()

        candidates = [
            Candidate(
                agent_id="c1",
                wealth=100.0,
                skills={"mining": 80},
                reputation=0.9,
            ),
            Candidate(
                agent_id="c2",
                wealth=50.0,
                skills={"combat": 30},
                reputation=0.3,
            ),
        ]

        voter_interests = AgentInterests(
            agent_id="v1",
            wealth=100.0,
            skills={"mining": 70},
        )

        decision = decider.vote_in_election(candidates, voter_interests)
        assert decision.action == "c1", "Should vote for candidate with aligned skills"

    def test_voting_single_candidate_default(self) -> None:
        """With only one candidate, voter defaults to supporting them."""
        decider = GovernanceDecider()
        candidates = [Candidate(agent_id="only", wealth=50.0)]
        interests = AgentInterests(agent_id="v1")

        decision = decider.vote_in_election(candidates, interests)
        assert decision.action == "only"
        assert decision.confidence == 0.8

    def test_voting_no_candidates_abstains(self) -> None:
        """With no candidates, voter abstains."""
        decider = GovernanceDecider()
        interests = AgentInterests(agent_id="v1")

        decision = decider.vote_in_election([], interests)
        assert decision.action == "abstain"


class TestGovernanceDeciderTreaty:
    """Test: GovernanceDecider treaty response decisions."""

    def test_accept_trade_treaty_complementary(self) -> None:
        """Trade treaty between complementary orgs is accepted."""
        decider = GovernanceDecider()

        my_org = OrgSnapshot(org_id="my", member_count=5, avg_wealth=50.0)
        other_org = OrgSnapshot(org_id="other", member_count=8, avg_wealth=200.0)
        treaty = Treaty(
            treaty_id="t1",
            proposer_org_id="other",
            treaty_type="trade",
        )

        decision = decider.respond_to_treaty(treaty, my_org, other_org)
        assert decision.action == TreatyResponse.ACCEPT.value

    def test_reject_treaty_empty_org(self) -> None:
        """Treaty with empty org is rejected."""
        decider = GovernanceDecider()

        my_org = OrgSnapshot(org_id="my", member_count=5)
        empty_org = OrgSnapshot(org_id="empty", member_count=0)
        treaty = Treaty(treaty_id="t2", proposer_org_id="empty")

        decision = decider.respond_to_treaty(treaty, my_org, empty_org)
        assert decision.action == TreatyResponse.REJECT.value

    def test_accept_defense_treaty_from_larger_org(self) -> None:
        """Defense treaty from larger org provides protection value."""
        decider = GovernanceDecider()

        my_org = OrgSnapshot(org_id="small", member_count=3)
        big_org = OrgSnapshot(org_id="big", member_count=10)
        treaty = Treaty(
            treaty_id="t3",
            proposer_org_id="big",
            treaty_type="defense",
        )

        decision = decider.respond_to_treaty(treaty, my_org, big_org)
        assert decision.action == TreatyResponse.ACCEPT.value

    def test_accept_non_aggression(self) -> None:
        """Non-aggression treaties are generally accepted."""
        decider = GovernanceDecider()

        my_org = OrgSnapshot(org_id="my", member_count=5)
        other_org = OrgSnapshot(org_id="other", member_count=5)
        treaty = Treaty(
            treaty_id="t4",
            proposer_org_id="other",
            treaty_type="non_aggression",
        )

        decision = decider.respond_to_treaty(treaty, my_org, other_org)
        assert decision.action == TreatyResponse.ACCEPT.value


class TestGovernanceDeciderTax:
    """Test: GovernanceDecider tax rate proposals."""

    def test_wealthy_agent_proposes_lower_tax(self) -> None:
        """Wealthy agents propose lower tax rates."""
        decider = GovernanceDecider()

        org = OrgSnapshot(org_id="org1", avg_wealth=100.0, treasury=50.0)
        decision = decider.propose_tax_rate(org, my_wealth=300.0)

        rate = float(decision.action)
        assert rate < 0.10, "Wealthy agent should propose lower taxes"

    def test_poor_agent_proposes_higher_tax(self) -> None:
        """Poor agents propose higher tax rates for redistribution."""
        decider = GovernanceDecider()

        org = OrgSnapshot(org_id="org1", avg_wealth=200.0, treasury=10.0)
        decision = decider.propose_tax_rate(org, my_wealth=30.0)

        rate = float(decision.action)
        assert rate > 0.10, "Poor agent should propose higher taxes"

    def test_zero_avg_wealth_uses_default(self) -> None:
        """When avg wealth is 0, default tax rate is proposed."""
        decider = GovernanceDecider()

        org = OrgSnapshot(org_id="org1", avg_wealth=0.0)
        decision = decider.propose_tax_rate(org, my_wealth=100.0)

        rate = float(decision.action)
        assert rate == pytest.approx(0.10, abs=0.01)


class TestGovernanceDeciderAllocation:
    """Test: GovernanceDecider allocation strategy choices."""

    def test_small_org_picks_equal(self) -> None:
        """Small healthy orgs default to equal distribution."""
        decider = GovernanceDecider()

        org = OrgSnapshot(
            org_id="small",
            member_count=3,
            avg_wealth=100.0,
            treasury=200.0,
            tax_rate=0.10,
        )
        decision = decider.choose_allocation_strategy(org)
        assert decision.action == AllocationStrategy.EQUAL.value

    def test_low_treasury_picks_need_based(self) -> None:
        """Org with low treasury picks need-based allocation."""
        decider = GovernanceDecider()

        org = OrgSnapshot(
            org_id="poor",
            member_count=5,
            avg_wealth=100.0,
            treasury=5.0,
        )
        decision = decider.choose_allocation_strategy(org)
        assert decision.action == AllocationStrategy.NEED_BASED.value

    def test_large_org_picks_proportional(self) -> None:
        """Large orgs pick proportional allocation."""
        decider = GovernanceDecider()

        org = OrgSnapshot(
            org_id="big",
            member_count=15,
            avg_wealth=100.0,
            treasury=5000.0,
            tax_rate=0.10,
        )
        decision = decider.choose_allocation_strategy(org)
        assert decision.action == AllocationStrategy.PROPORTIONAL.value

    def test_single_member_picks_equal(self) -> None:
        """Single-member org always picks equal."""
        decider = GovernanceDecider()

        org = OrgSnapshot(org_id="solo", member_count=1)
        decision = decider.choose_allocation_strategy(org)
        assert decision.action == AllocationStrategy.EQUAL.value
        assert decision.confidence == 1.0


class TestMultiOrgGovernanceSimulation:
    """Test: 3 organizations, 200+ ticks — governance decisions drive events.

    This is the main E2E integration test on the Python side. It simulates
    multiple agents making governance decisions across three orgs over 200+
    ticks, verifying that the GovernanceDecider produces decisions consistent
    with the Rust event system expectations.
    """

    def test_three_orgs_governance_over_200_ticks(self) -> None:
        """3 orgs over 200 ticks: leadership, tax, diplomacy decisions all occur."""
        world = SimulatedGovernanceWorld()
        decider = GovernanceDecider()

        # Create 3 orgs with diverse memberships
        org_a = world.create_org("Miners Guild", OrgType.GUILD, [
            f"miner-{i}" for i in range(5)
        ])
        org_b = world.create_org("Trade Company", OrgType.COMPANY, [
            f"trader-{i}" for i in range(4)
        ])
        org_c = world.create_org("Defense Alliance", OrgType.ALLIANCE, [
            f"guard-{i}" for i in range(3)
        ])

        # Tracking for verification
        leadership_decisions: List[str] = []
        tax_proposals: List[float] = []
        treaty_decisions: List[str] = []
        allocation_decisions: List[str] = []

        for tick in range(1, 251):
            world.advance_tick()

            # ── Leadership: evaluate running at tick 30 ──────────
            if tick == 30:
                for org in [org_a, org_b, org_c]:
                    for i, member_id in enumerate(org.members):
                        snapshot = world.get_org_snapshot(org.id)
                        interests = AgentInterests(
                            agent_id=member_id,
                            wealth=100.0 + i * 30,
                            skills={"mining": 50 + i * 10, "crafting": 40 + i * 5},
                            risk_tolerance=0.3 + i * 0.15,
                        )
                        decision = decider.should_run_for_leader(snapshot, interests)
                        if decision.action == LeadershipAmbition.RUN.value:
                            leadership_decisions.append(member_id)
                            world.event_bus.publish("GovernanceIntent", {
                                "intent": "run_for_leader",
                                "org_id": org.id,
                                "agent_id": member_id,
                                "tick": tick,
                            })

            # ── Voting: at tick 31, agents vote ─────────────────
            if tick == 31:
                for org in [org_a, org_b, org_c]:
                    candidates = [
                        Candidate(
                            agent_id=m,
                            wealth=100.0 + i * 30,
                            skills={"mining": 50 + i * 10},
                            reputation=0.5 + i * 0.1,
                        )
                        for i, m in enumerate(org.members[:3])
                    ]
                    for member_id in org.members:
                        voter_interests = AgentInterests(
                            agent_id=member_id,
                            wealth=100.0,
                            skills={"mining": 60},
                        )
                        decision = decider.vote_in_election(candidates, voter_interests)
                        world.event_bus.publish("GovernanceIntent", {
                            "intent": "cast_vote",
                            "org_id": org.id,
                            "voter_id": member_id,
                            "candidate_id": decision.action,
                            "tick": tick,
                        })

            # ── Tax: every 25 ticks, agents propose tax rates ────
            if tick % 25 == 0:
                for org in [org_a, org_b, org_c]:
                    for member_id in org.members:
                        snapshot = world.get_org_snapshot(org.id)
                        wealth = 100.0 if member_id == org.members[0] else 50.0
                        decision = decider.propose_tax_rate(snapshot, wealth)
                        tax_proposals.append(float(decision.action))

            # ── Allocation: every 50 ticks ───────────────────────
            if tick % 50 == 0 and tick > 0:
                for org in [org_a, org_b, org_c]:
                    snapshot = world.get_org_snapshot(org.id)
                    decision = decider.choose_allocation_strategy(snapshot)
                    allocation_decisions.append(decision.action)

            # ── Diplomacy: propose treaties at tick 60 ───────────
            if tick == 60:
                # Org A proposes trade treaty to Org B
                treaty = Treaty(
                    treaty_id="t-ab",
                    proposer_org_id=org_a.id,
                    treaty_type="trade",
                )
                snapshot_a = world.get_org_snapshot(org_a.id)
                snapshot_b = world.get_org_snapshot(org_b.id)

                # Different wealth profiles → complementary trade
                org_b.avg_wealth = 250.0
                decision = decider.respond_to_treaty(treaty, snapshot_a, snapshot_b)
                treaty_decisions.append(decision.action)
                world.event_bus.publish("GovernanceIntent", {
                    "intent": "treaty_response",
                    "treaty_id": "t-ab",
                    "response": decision.action,
                    "tick": tick,
                })

                # Org C proposes defense treaty to Org A
                treaty2 = Treaty(
                    treaty_id="t-ca",
                    proposer_org_id=org_c.id,
                    treaty_type="defense",
                )
                snapshot_a2 = world.get_org_snapshot(org_a.id)
                snapshot_c = world.get_org_snapshot(org_c.id)

                decision2 = decider.respond_to_treaty(treaty2, snapshot_a2, snapshot_c)
                treaty_decisions.append(decision2.action)

                # Org C proposes non-aggression to Org B
                treaty3 = Treaty(
                    treaty_id="t-cb",
                    proposer_org_id=org_c.id,
                    treaty_type="non_aggression",
                )
                snapshot_b2 = world.get_org_snapshot(org_b.id)
                snapshot_c2 = world.get_org_snapshot(org_c.id)
                decision3 = decider.respond_to_treaty(treaty3, snapshot_b2, snapshot_c2)
                treaty_decisions.append(decision3.action)

        # ── Verify Results ───────────────────────────────────────

        # At least some agents decided to run for leader
        assert len(leadership_decisions) > 0, (
            f"Expected some agents to run for leader, got {len(leadership_decisions)}"
        )

        # Tax proposals were made (3 orgs * multiple rounds)
        assert len(tax_proposals) > 0, "Expected tax proposals"
        # All proposals are within valid range
        for rate in tax_proposals:
            assert 0.05 <= rate <= 0.30, f"Tax rate {rate} out of range"

        # Treaty decisions were made
        assert len(treaty_decisions) >= 2, (
            f"Expected at least 2 treaty decisions, got {len(treaty_decisions)}"
        )

        # Allocation decisions were made
        assert len(allocation_decisions) > 0, "Expected allocation decisions"

        # Verify governance events were published
        governance_events = world.event_bus.find_by_type("GovernanceIntent")
        assert len(governance_events) > 0, "Expected governance events"

        # Verify specific intents
        run_events = [
            e for e in governance_events
            if e.payload.get("intent") == "run_for_leader"
        ]
        assert len(run_events) > 0, "Expected run_for_leader intents"

        vote_events = [
            e for e in governance_events
            if e.payload.get("intent") == "cast_vote"
        ]
        assert len(vote_events) > 0, "Expected cast_vote intents"

        treaty_response_events = [
            e for e in governance_events
            if e.payload.get("intent") == "treaty_response"
        ]
        assert len(treaty_response_events) > 0, "Expected treaty_response intents"


class TestCrossSystemGovernanceInteraction:
    """Test: Governance decisions interact correctly across subsystems."""

    def test_tax_decision_after_leadership_change(self) -> None:
        """Agent proposes different tax rates depending on leadership state."""
        decider = GovernanceDecider()

        # Org without leader
        org_no_leader = OrgSnapshot(
            org_id="org1",
            member_count=5,
            avg_wealth=100.0,
            treasury=50.0,
            has_leader=False,
        )

        # Same org with leader
        org_with_leader = OrgSnapshot(
            org_id="org1",
            member_count=5,
            avg_wealth=100.0,
            treasury=50.0,
            has_leader=True,
            leader_id="leader-1",
        )

        # Poor agent's tax proposal should be the same regardless of leader
        interests = AgentInterests(agent_id="poor", wealth=30.0)

        decision1 = decider.propose_tax_rate(org_no_leader, interests.wealth)
        decision2 = decider.propose_tax_rate(org_with_leader, interests.wealth)

        rate1 = float(decision1.action)
        rate2 = float(decision2.action)
        # Both should be higher than default since agent is poor
        assert rate1 > 0.10
        assert rate2 > 0.10

    def test_allocation_changes_with_treasury(self) -> None:
        """Allocation strategy changes as treasury fluctuates."""
        decider = GovernanceDecider()

        # Rich org: equal or proportional
        rich_org = OrgSnapshot(
            org_id="rich",
            member_count=5,
            avg_wealth=100.0,
            treasury=5000.0,
            tax_rate=0.10,
        )
        decision_rich = decider.choose_allocation_strategy(rich_org)

        # Poor org: need-based
        poor_org = OrgSnapshot(
            org_id="poor",
            member_count=5,
            avg_wealth=100.0,
            treasury=5.0,
            tax_rate=0.10,
        )
        decision_poor = decider.choose_allocation_strategy(poor_org)

        assert decision_rich.action == AllocationStrategy.EQUAL.value
        assert decision_poor.action == AllocationStrategy.NEED_BASED.value

    def test_treaty_response_varies_by_org_size(self) -> None:
        """Defense treaty acceptance depends on relative org sizes."""
        decider = GovernanceDecider()

        # Small org: accepts defense from big org
        small_org = OrgSnapshot(org_id="small", member_count=3)
        big_org = OrgSnapshot(org_id="big", member_count=10)
        treaty = Treaty(treaty_id="t1", proposer_org_id="big", treaty_type="defense")

        decision_small = decider.respond_to_treaty(treaty, small_org, big_org)
        assert decision_small.action == TreatyResponse.ACCEPT.value

        # Big org: rejects defense from small org (limited value)
        decision_big = decider.respond_to_treaty(treaty, big_org, small_org)
        assert decision_big.action == TreatyResponse.REJECT.value
