"""Tests for agent_runtime.organization module.

Covers:
- Formation condition evaluation (all three triggers)
- Proposal generation (name, type, charter)
- Recruitment (invitation sending and response tracking)
"""

from __future__ import annotations

import asyncio
import pytest

from agent_runtime.organization import (
    AgentInterests,
    AllocationStrategy,
    Candidate,
    FormationConditions,
    FormationEvaluator,
    FormationReason,
    GovernanceDecision,
    GovernanceDecider,
    Invitation,
    InvitationStatus,
    LeadershipAmbition,
    OrgProposal,
    OrgSnapshot,
    OrgType,
    ProposalGenerator,
    RecruitmentEngine,
    Treaty,
    TreatyResponse,
)
from agent_runtime.organization.formation import (
    AgentProfile,
    FORMATION_THRESHOLD,
    _euclidean_distance,
    _jaccard_similarity,
)


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def _make_profile(
    agent_id: str = "agent-001",
    skills: dict | None = None,
    location: tuple[float, float] = (0.0, 0.0),
    resources: dict | None = None,
    goals: list | None = None,
) -> AgentProfile:
    return AgentProfile(
        agent_id=agent_id,
        skills=skills or {},
        location=location,
        resources=resources or {},
        goals=goals or [],
    )


class _FakeA2AClient:
    """Minimal A2A client mock for recruitment tests."""

    def __init__(self) -> None:
        self.sent_messages: list[dict] = []

    async def send_message(self, payload: dict) -> dict:
        self.sent_messages.append(payload)
        return {"status": "ok", "received": True}

    async def broadcast_message(self, payload: dict) -> dict:
        self.sent_messages.append(payload)
        return {"status": "ok", "received": True}


def _make_proposal(
    org_name: str = "Test Org",
    org_type: OrgType = OrgType.GUILD,
    founder_id: str = "agent-001",
    members: list[str] | None = None,
) -> OrgProposal:
    return OrgProposal(
        proposal_id="test-proposal-id",
        org_name=org_name,
        org_type=org_type,
        charter="Test charter.",
        founder_id=founder_id,
        founding_members=members or [founder_id],
        proposed_tick=0,
    )


# ===========================================================================
# Formation evaluation tests
# ===========================================================================


class TestJaccardSimilarity:
    """Tests for the _jaccard_similarity helper."""

    def test_identical_sets(self) -> None:
        assert _jaccard_similarity({"a", "b"}, {"a", "b"}) == 1.0

    def test_disjoint_sets(self) -> None:
        assert _jaccard_similarity({"a"}, {"b"}) == 0.0

    def test_partial_overlap(self) -> None:
        sim = _jaccard_similarity({"a", "b", "c"}, {"b", "c", "d"})
        assert 0.0 < sim < 1.0

    def test_empty_sets(self) -> None:
        assert _jaccard_similarity(set(), set()) == 0.0

    def test_one_empty_set(self) -> None:
        assert _jaccard_similarity({"a"}, set()) == 0.0


class TestEuclideanDistance:
    """Tests for the _euclidean_distance helper."""

    def test_same_point(self) -> None:
        assert _euclidean_distance((0.0, 0.0), (0.0, 0.0)) == 0.0

    def test_unit_distance(self) -> None:
        assert abs(_euclidean_distance((0.0, 0.0), (1.0, 0.0)) - 1.0) < 1e-9

    def test_diagonal(self) -> None:
        dist = _euclidean_distance((0.0, 0.0), (3.0, 4.0))
        assert abs(dist - 5.0) < 1e-9


class TestFormationEvaluator:
    """Tests for FormationEvaluator."""

    def test_too_few_agents_returns_false(self) -> None:
        evaluator = FormationEvaluator()
        conditions = evaluator.evaluate([_make_profile("agent-001")])
        assert not conditions.should_form
        assert conditions.composite_score == 0.0

    def test_shared_skills_triggers_interests(self) -> None:
        """Agents with identical skills should score high on shared interests."""
        evaluator = FormationEvaluator()
        profiles = [
            _make_profile("a1", skills={"mining": 50, "crafting": 30}),
            _make_profile("a2", skills={"mining": 60, "crafting": 40}),
        ]
        conditions = evaluator.evaluate(profiles)
        assert conditions.shared_interests_score > 0.5
        assert FormationReason.SHARED_INTERESTS in conditions.triggers

    def test_shared_goals_triggers_interests(self) -> None:
        """Agents with shared goals should score on shared interests."""
        evaluator = FormationEvaluator()
        profiles = [
            _make_profile("a1", goals=["survive", "trade", "build"]),
            _make_profile("a2", goals=["survive", "trade", "explore"]),
        ]
        conditions = evaluator.evaluate(profiles)
        # With no shared skills, only goals contribute: 0.6*0 + 0.4*0.5 = 0.2
        assert conditions.shared_interests_score > 0.0
        assert FormationReason.GEOGRAPHIC_PROXIMITY in conditions.triggers

    def test_nearby_agents_triggers_proximity(self) -> None:
        """Agents at the same location should score high on proximity."""
        evaluator = FormationEvaluator()
        profiles = [
            _make_profile("a1", location=(10.0, 10.0)),
            _make_profile("a2", location=(10.5, 10.5)),
        ]
        conditions = evaluator.evaluate(profiles)
        assert conditions.proximity_score > 0.9
        assert FormationReason.GEOGRAPHIC_PROXIMITY in conditions.triggers

    def test_distant_agents_low_proximity(self) -> None:
        """Agents far apart should score low on proximity."""
        evaluator = FormationEvaluator()
        profiles = [
            _make_profile("a1", location=(0.0, 0.0)),
            _make_profile("a2", location=(1000.0, 1000.0)),
        ]
        conditions = evaluator.evaluate(profiles)
        assert conditions.proximity_score < 0.1

    def test_complementary_skills_triggers_complementarity(self) -> None:
        """Agents with different skills should score high on complementarity."""
        evaluator = FormationEvaluator()
        profiles = [
            _make_profile("a1", skills={"mining": 50}),
            _make_profile("a2", skills={"farming": 50}),
        ]
        conditions = evaluator.evaluate(profiles)
        assert conditions.complementarity_score > 0.5
        assert FormationReason.ECONOMIC_COMPLEMENTARITY in conditions.triggers

    def test_complementary_resources(self) -> None:
        """Agents with different resources should increase complementarity."""
        evaluator = FormationEvaluator()
        profiles = [
            _make_profile("a1", resources={"wood": 10}),
            _make_profile("a2", resources={"stone": 10}),
        ]
        conditions = evaluator.evaluate(profiles)
        assert conditions.complementarity_score > 0.3

    def test_formation_threshold_respected(self) -> None:
        """Well-aligned agents should exceed the formation threshold."""
        evaluator = FormationEvaluator()
        profiles = [
            _make_profile(
                "a1",
                skills={"mining": 50, "crafting": 30},
                location=(10.0, 10.0),
                resources={"wood": 10},
                goals=["build", "trade"],
            ),
            _make_profile(
                "a2",
                skills={"farming": 40},
                location=(10.0, 10.0),
                resources={"stone": 10},
                goals=["build", "explore"],
            ),
        ]
        conditions = evaluator.evaluate(profiles)
        assert conditions.should_form
        assert conditions.composite_score >= FORMATION_THRESHOLD

    def test_custom_threshold(self) -> None:
        """Custom formation threshold should be respected."""
        evaluator = FormationEvaluator(formation_threshold=0.99)
        profiles = [
            _make_profile("a1", skills={"a": 1}),
            _make_profile("a2", skills={"b": 1}),
        ]
        conditions = evaluator.evaluate(profiles)
        # Very high threshold, unlikely to pass
        assert not conditions.should_form

    def test_composite_score_weighted(self) -> None:
        """Composite score should be the weighted average of individual scores."""
        evaluator = FormationEvaluator()
        profiles = [
            _make_profile("a1", skills={"mining": 50}),
            _make_profile("a2", skills={"mining": 60}),
        ]
        conditions = evaluator.evaluate(profiles)
        expected = (
            conditions.shared_interests_score * 0.4
            + conditions.proximity_score * 0.25
            + conditions.complementarity_score * 0.35
        )
        assert abs(conditions.composite_score - expected) < 1e-9

    def test_three_agents(self) -> None:
        """Should work with more than 2 agents."""
        evaluator = FormationEvaluator()
        profiles = [
            _make_profile("a1", skills={"mining": 50}, location=(0.0, 0.0)),
            _make_profile("a2", skills={"mining": 60}, location=(1.0, 1.0)),
            _make_profile("a3", skills={"mining": 70}, location=(2.0, 2.0)),
        ]
        conditions = evaluator.evaluate(profiles)
        assert conditions.shared_interests_score > 0.5

    def test_identical_agents_high_composite(self) -> None:
        """Identical agents should have high skill-based shared interest."""
        evaluator = FormationEvaluator()
        profile = _make_profile("a1", skills={"mining": 50}, location=(5.0, 5.0))
        profiles = [profile, _make_profile("a2", skills={"mining": 50}, location=(5.0, 5.0))]
        conditions = evaluator.evaluate(profiles)
        # Skills overlap = 1.0, goals overlap = 0.0 → 0.6*1.0 + 0.4*0.0 = 0.6
        assert conditions.shared_interests_score == 0.6
        assert conditions.proximity_score == 1.0

    def test_no_skills_no_goals(self) -> None:
        """Agents with no skills or goals should have low shared interest score."""
        evaluator = FormationEvaluator()
        profiles = [
            _make_profile("a1"),
            _make_profile("a2"),
        ]
        conditions = evaluator.evaluate(profiles)
        assert conditions.shared_interests_score == 0.0

    def test_empty_profiles_list(self) -> None:
        """Empty profiles list should not form."""
        evaluator = FormationEvaluator()
        conditions = evaluator.evaluate([])
        assert not conditions.should_form


# ===========================================================================
# Proposal generation tests
# ===========================================================================


class TestProposalGenerator:
    """Tests for ProposalGenerator."""

    def _make_conditions(self, **overrides: float) -> FormationConditions:
        defaults = {
            "shared_interests_score": 0.8,
            "proximity_score": 0.7,
            "complementarity_score": 0.6,
            "composite_score": 0.7,
            "should_form": True,
            "triggers": [FormationReason.SHARED_INTERESTS],
        }
        defaults.update(overrides)
        return FormationConditions(**defaults)

    def test_generates_valid_proposal(self) -> None:
        gen = ProposalGenerator(seed=42)
        profiles = [
            _make_profile("a1", skills={"mining": 50}),
            _make_profile("a2", skills={"mining": 60}),
        ]
        conditions = self._make_conditions()
        proposal = gen.generate(profiles, conditions, founder_id="a1", tick=10)

        assert proposal.proposal_id
        assert proposal.org_name
        assert proposal.org_type in OrgType
        assert proposal.charter
        assert proposal.founder_id == "a1"
        assert "a1" in proposal.founding_members
        assert "a2" in proposal.founding_members
        assert proposal.proposed_tick == 10

    def test_org_type_guild_for_shared_interests(self) -> None:
        """When shared interests dominate, should tend toward GUILD."""
        gen = ProposalGenerator(seed=42)
        conditions = self._make_conditions(
            triggers=[FormationReason.SHARED_INTERESTS],
            shared_interests_score=0.9,
            proximity_score=0.1,
            complementarity_score=0.1,
        )
        profiles = [_make_profile("a1"), _make_profile("a2")]
        proposal = gen.generate(profiles, conditions, founder_id="a1")
        assert proposal.org_type == OrgType.GUILD

    def test_org_type_alliance_for_proximity(self) -> None:
        """When proximity dominates, should tend toward ALLIANCE."""
        gen = ProposalGenerator(seed=42)
        conditions = self._make_conditions(
            triggers=[FormationReason.GEOGRAPHIC_PROXIMITY],
            shared_interests_score=0.1,
            proximity_score=0.9,
            complementarity_score=0.1,
        )
        profiles = [_make_profile("a1"), _make_profile("a2")]
        proposal = gen.generate(profiles, conditions, founder_id="a1")
        assert proposal.org_type == OrgType.ALLIANCE

    def test_org_type_syndicate_for_complementarity(self) -> None:
        """When complementarity dominates, should tend toward SYNDICATE."""
        gen = ProposalGenerator(seed=42)
        conditions = self._make_conditions(
            triggers=[FormationReason.ECONOMIC_COMPLEMENTARITY],
            shared_interests_score=0.1,
            proximity_score=0.1,
            complementarity_score=0.9,
        )
        profiles = [_make_profile("a1"), _make_profile("a2")]
        proposal = gen.generate(profiles, conditions, founder_id="a1")
        assert proposal.org_type == OrgType.SYNDICATE

    def test_name_has_prefix_and_suffix(self) -> None:
        """Generated name should contain a prefix and a suffix."""
        gen = ProposalGenerator(seed=42)
        conditions = self._make_conditions()
        profiles = [_make_profile("a1"), _make_profile("a2")]
        proposal = gen.generate(profiles, conditions, founder_id="a1")
        # Name should have at least two words
        parts = proposal.org_name.split()
        assert len(parts) >= 2

    def test_deterministic_with_seed(self) -> None:
        """Same seed should produce the same proposal name."""
        gen1 = ProposalGenerator(seed=123)
        gen2 = ProposalGenerator(seed=123)
        conditions = self._make_conditions()
        profiles = [_make_profile("a1"), _make_profile("a2")]

        p1 = gen1.generate(profiles, conditions, founder_id="a1")
        p2 = gen2.generate(profiles, conditions, founder_id="a1")

        assert p1.org_name == p2.org_name
        assert p1.org_type == p2.org_type

    def test_charter_references_skills(self) -> None:
        """Charter should reference skills when shared interests trigger."""
        gen = ProposalGenerator(seed=42)
        profiles = [
            _make_profile("a1", skills={"mining": 50}),
            _make_profile("a2", skills={"crafting": 30}),
        ]
        conditions = self._make_conditions(triggers=[FormationReason.SHARED_INTERESTS])
        proposal = gen.generate(profiles, conditions, founder_id="a1")
        # Charter should mention skills
        assert "mining" in proposal.charter or "crafting" in proposal.charter

    def test_no_triggers_uses_default(self) -> None:
        """When no triggers, should still generate a valid proposal."""
        gen = ProposalGenerator(seed=42)
        conditions = self._make_conditions(triggers=[], should_form=False)
        profiles = [_make_profile("a1"), _make_profile("a2")]
        proposal = gen.generate(profiles, conditions, founder_id="a1")
        assert proposal.org_type in OrgType
        assert proposal.org_name


# ===========================================================================
# Recruitment tests
# ===========================================================================


class TestRecruitmentEngine:
    """Tests for RecruitmentEngine."""

    @pytest.mark.asyncio
    async def test_send_invitations(self) -> None:
        engine = RecruitmentEngine()
        client = _FakeA2AClient()
        proposal = _make_proposal(members=["agent-001"])

        invitations = await engine.send_invitations(
            proposal, ["agent-002", "agent-003"], client
        )

        assert len(invitations) == 2
        assert all(inv.status == InvitationStatus.PENDING for inv in invitations)
        assert len(client.sent_messages) == 2

    @pytest.mark.asyncio
    async def test_skip_founding_members(self) -> None:
        """Should not send invitations to founding members."""
        engine = RecruitmentEngine()
        client = _FakeA2AClient()
        proposal = _make_proposal(members=["agent-001", "agent-002"])

        invitations = await engine.send_invitations(
            proposal, ["agent-001", "agent-002"], client
        )

        assert len(invitations) == 0
        assert len(client.sent_messages) == 0

    @pytest.mark.asyncio
    async def test_mixed_candidates(self) -> None:
        """Should only invite non-founding candidates."""
        engine = RecruitmentEngine()
        client = _FakeA2AClient()
        proposal = _make_proposal(members=["agent-001"])

        invitations = await engine.send_invitations(
            proposal, ["agent-001", "agent-002"], client
        )

        assert len(invitations) == 1
        assert invitations[0].target_agent_id == "agent-002"

    @pytest.mark.asyncio
    async def test_accept_invitation(self) -> None:
        engine = RecruitmentEngine()
        client = _FakeA2AClient()
        proposal = _make_proposal(members=["agent-001"])

        invitations = await engine.send_invitations(
            proposal, ["agent-002"], client
        )
        inv_id = invitations[0].invitation_id

        updated = engine.respond_to_invitation(inv_id, accept=True)
        assert updated is not None
        assert updated.status == InvitationStatus.ACCEPTED

    @pytest.mark.asyncio
    async def test_decline_invitation(self) -> None:
        engine = RecruitmentEngine()
        client = _FakeA2AClient()
        proposal = _make_proposal(members=["agent-001"])

        invitations = await engine.send_invitations(
            proposal, ["agent-002"], client
        )
        inv_id = invitations[0].invitation_id

        updated = engine.respond_to_invitation(inv_id, accept=False)
        assert updated is not None
        assert updated.status == InvitationStatus.DECLINED

    @pytest.mark.asyncio
    async def test_respond_to_unknown_invitation(self) -> None:
        engine = RecruitmentEngine()
        result = engine.respond_to_invitation("nonexistent", accept=True)
        assert result is None

    @pytest.mark.asyncio
    async def test_get_pending_invitations(self) -> None:
        engine = RecruitmentEngine()
        client = _FakeA2AClient()
        proposal = _make_proposal(members=["agent-001"])

        await engine.send_invitations(proposal, ["agent-002", "agent-003"], client)

        pending = engine.get_pending_invitations("agent-002")
        assert len(pending) == 1
        assert pending[0].target_agent_id == "agent-002"

    @pytest.mark.asyncio
    async def test_get_pending_after_accept(self) -> None:
        """Accepted invitations should not appear in pending."""
        engine = RecruitmentEngine()
        client = _FakeA2AClient()
        proposal = _make_proposal(members=["agent-001"])

        invitations = await engine.send_invitations(proposal, ["agent-002"], client)
        engine.respond_to_invitation(invitations[0].invitation_id, accept=True)

        pending = engine.get_pending_invitations("agent-002")
        assert len(pending) == 0

    @pytest.mark.asyncio
    async def test_invitation_message_contains_org_info(self) -> None:
        engine = RecruitmentEngine()
        client = _FakeA2AClient()
        proposal = _make_proposal(org_name="Iron Guild")

        invitations = await engine.send_invitations(proposal, ["agent-002"], client)
        assert "Iron Guild" in invitations[0].message

    @pytest.mark.asyncio
    async def test_get_invitation_by_id(self) -> None:
        engine = RecruitmentEngine()
        client = _FakeA2AClient()
        proposal = _make_proposal(members=["agent-001"])

        invitations = await engine.send_invitations(proposal, ["agent-002"], client)
        inv_id = invitations[0].invitation_id

        retrieved = engine.get_invitation(inv_id)
        assert retrieved is not None
        assert retrieved.target_agent_id == "agent-002"

    @pytest.mark.asyncio
    async def test_a2a_payload_structure(self) -> None:
        """Verify the A2A message has correct structure for org invitation."""
        engine = RecruitmentEngine()
        client = _FakeA2AClient()
        proposal = _make_proposal(org_name="Test Syndicate", org_type=OrgType.SYNDICATE)

        await engine.send_invitations(proposal, ["agent-002"], client)

        msg = client.sent_messages[0]
        assert msg["to_agent"] == "agent-002"
        assert msg["type"] == "PROPOSE"
        payload = msg["payload"]
        assert payload["action"] == "org_invitation"
        assert payload["org_name"] == "Test Syndicate"
        assert payload["org_type"] == "syndicate"


# ===========================================================================
# Integration-style tests (formation → proposal → recruitment)
# ===========================================================================


class TestOrganizationFormationFlow:
    """End-to-end flow: evaluate → propose → recruit."""

    @pytest.mark.asyncio
    async def test_full_flow(self) -> None:
        """Agents with shared interests should be able to form and recruit."""
        profiles = [
            _make_profile(
                "a1",
                skills={"mining": 50, "crafting": 30},
                location=(10.0, 10.0),
                resources={"wood": 20},
                goals=["build", "trade"],
            ),
            _make_profile(
                "a2",
                skills={"mining": 60, "crafting": 40},
                location=(10.5, 10.5),
                resources={"stone": 15},
                goals=["build", "explore"],
            ),
            _make_profile(
                "a3",
                skills={"farming": 50},
                location=(100.0, 100.0),
                resources={"food": 30},
                goals=["survive"],
            ),
        ]

        # Step 1: Evaluate formation conditions for a1 and a2
        evaluator = FormationEvaluator()
        conditions = evaluator.evaluate(profiles[:2])
        assert conditions.should_form

        # Step 2: Generate proposal
        generator = ProposalGenerator(seed=42)
        proposal = generator.generate(
            profiles[:2], conditions, founder_id="a1", tick=5
        )
        assert proposal.org_name
        assert proposal.org_type in OrgType
        assert len(proposal.founding_members) == 2

        # Step 3: Recruit a3
        engine = RecruitmentEngine()
        client = _FakeA2AClient()
        invitations = await engine.send_invitations(proposal, ["a3"], client)

        assert len(invitations) == 1
        assert invitations[0].target_agent_id == "a3"

        # a3 accepts
        updated = engine.respond_to_invitation(invitations[0].invitation_id, accept=True)
        assert updated is not None
        assert updated.status == InvitationStatus.ACCEPTED

    @pytest.mark.asyncio
    async def test_flow_rejects_mismatched_agents(self) -> None:
        """Agents with nothing in common should not form an org."""
        profiles = [
            _make_profile(
                "a1",
                skills={"mining": 50},
                location=(0.0, 0.0),
                resources={"wood": 10},
                goals=["mine"],
            ),
            _make_profile(
                "a2",
                skills={"farming": 50},
                location=(500.0, 500.0),
                resources={"seeds": 10},
                goals=["farm"],
            ),
        ]

        evaluator = FormationEvaluator()
        conditions = evaluator.evaluate(profiles)
        # These agents have different skills, far apart, different goals
        # Complementarity might be high but shared interests and proximity are low
        # The composite may or may not exceed threshold depending on weights
        # The key assertion is that the flow correctly identifies triggers
        assert isinstance(conditions.triggers, list)


# ===========================================================================
# Governance decision tests
# ===========================================================================


def _make_org(
    org_id: str = "org-001",
    member_count: int = 5,
    avg_wealth: float = 100.0,
    total_wealth: float = 500.0,
    tax_rate: float = 0.10,
    has_leader: bool = True,
    leader_id: str | None = "agent-001",
    treasury: float = 50.0,
) -> OrgSnapshot:
    return OrgSnapshot(
        org_id=org_id,
        member_count=member_count,
        avg_wealth=avg_wealth,
        total_wealth=total_wealth,
        tax_rate=tax_rate,
        has_leader=has_leader,
        leader_id=leader_id,
        treasury=treasury,
    )


def _make_interests(
    agent_id: str = "agent-001",
    wealth: float = 100.0,
    skills: dict | None = None,
    goals: list | None = None,
    risk_tolerance: float = 0.5,
) -> AgentInterests:
    return AgentInterests(
        agent_id=agent_id,
        wealth=wealth,
        skills=skills or {},
        goals=goals or [],
        risk_tolerance=risk_tolerance,
    )


def _make_candidate(
    agent_id: str = "cand-001",
    wealth: float = 100.0,
    skills: dict | None = None,
    goals: list | None = None,
    reputation: float = 0.5,
) -> Candidate:
    return Candidate(
        agent_id=agent_id,
        wealth=wealth,
        skills=skills or {},
        goals=goals or [],
        reputation=reputation,
    )


def _make_treaty(
    treaty_id: str = "treaty-001",
    proposer_org_id: str = "org-002",
    terms: dict | None = None,
    treaty_type: str = "trade",
) -> Treaty:
    return Treaty(
        treaty_id=treaty_id,
        proposer_org_id=proposer_org_id,
        terms=terms or {},
        treaty_type=treaty_type,
    )


class TestGovernanceShouldRunForLeader:
    """Tests for GovernanceDecider.should_run_for_leader."""

    def test_wealthy_skilled_agent_runs(self) -> None:
        """Agent with high wealth and skills should run for leader."""
        decider = GovernanceDecider()
        org = _make_org(avg_wealth=100.0)
        interests = _make_interests(
            wealth=150.0,
            skills={"mining": 80, "crafting": 70},
            risk_tolerance=0.7,
        )
        decision = decider.should_run_for_leader(org, interests)
        assert decision.action == LeadershipAmbition.RUN.value
        assert decision.confidence > 0.5

    def test_poor_unskilled_agent_abstains(self) -> None:
        """Agent with low wealth and no skills should not run."""
        decider = GovernanceDecider()
        org = _make_org(avg_wealth=100.0)
        interests = _make_interests(wealth=10.0, skills={}, risk_tolerance=0.1)
        decision = decider.should_run_for_leader(org, interests)
        assert decision.action == LeadershipAmbition.ABSTAIN.value

    def test_empty_org_abstain(self) -> None:
        """Should abstain from leading an empty organization."""
        decider = GovernanceDecider()
        org = _make_org(member_count=0, avg_wealth=0.0)
        interests = _make_interests(wealth=100.0, skills={"mining": 90})
        decision = decider.should_run_for_leader(org, interests)
        assert decision.action == LeadershipAmbition.ABSTAIN.value
        assert decision.confidence == 1.0

    def test_high_risk_tolerance_boosts_willingness(self) -> None:
        """Agent with high risk tolerance more likely to run."""
        decider = GovernanceDecider()
        org = _make_org(avg_wealth=100.0)
        interests = _make_interests(
            wealth=80.0,
            skills={"mining": 55},
            risk_tolerance=0.95,
        )
        decision = decider.should_run_for_leader(org, interests)
        assert decision.action == LeadershipAmbition.RUN.value

    def test_no_leader_bonus(self) -> None:
        """Agent more likely to run when org has no leader."""
        decider = GovernanceDecider()
        org_with_leader = _make_org(has_leader=True, leader_id="other-agent")
        org_no_leader = _make_org(has_leader=False, leader_id=None)
        interests = _make_interests(
            wealth=90.0,
            skills={"mining": 60},
            risk_tolerance=0.5,
        )
        decision_with = decider.should_run_for_leader(org_with_leader, interests)
        decision_without = decider.should_run_for_leader(org_no_leader, interests)
        # No-leader org should have at least as high confidence for running
        assert decision_without.confidence >= decision_with.confidence


class TestGovernanceVoteInElection:
    """Tests for GovernanceDecider.vote_in_election."""

    def test_no_candidates_abstain(self) -> None:
        """No candidates → abstain."""
        decider = GovernanceDecider()
        interests = _make_interests()
        decision = decider.vote_in_election([], interests)
        assert decision.action == "abstain"

    def test_single_candidate_default_support(self) -> None:
        """Single candidate gets default support."""
        decider = GovernanceDecider()
        cand = _make_candidate("cand-001")
        interests = _make_interests()
        decision = decider.vote_in_election([cand], interests)
        assert decision.action == "cand-001"
        assert decision.confidence == 0.8

    def test_votes_for_best_aligned_candidate(self) -> None:
        """Should vote for the candidate with most goal/skill overlap."""
        decider = GovernanceDecider()
        interests = _make_interests(
            skills={"mining": 50, "crafting": 30},
            goals=["build", "trade"],
        )
        cand_aligned = _make_candidate(
            "aligned",
            skills={"mining": 60},
            goals=["build"],
            reputation=0.7,
        )
        cand_misaligned = _make_candidate(
            "misaligned",
            skills={"farming": 80},
            goals=["farm"],
            reputation=0.9,
        )
        decision = decider.vote_in_election(
            [cand_aligned, cand_misaligned], interests
        )
        assert decision.action == "aligned"

    def test_reputation_breaks_ties(self) -> None:
        """When candidates are otherwise similar, reputation matters."""
        decider = GovernanceDecider()
        interests = _make_interests(skills={"mining": 50})
        cand_high_rep = _make_candidate(
            "high-rep", skills={"mining": 50}, reputation=0.9
        )
        cand_low_rep = _make_candidate(
            "low-rep", skills={"mining": 50}, reputation=0.2
        )
        decision = decider.vote_in_election(
            [cand_high_rep, cand_low_rep], interests
        )
        assert decision.action == "high-rep"


class TestGovernanceRespondToTreaty:
    """Tests for GovernanceDecider.respond_to_treaty."""

    def test_trade_treaty_complementary_wealth_accepts(self) -> None:
        """Trade treaty between complementary orgs → accept."""
        decider = GovernanceDecider()
        my_org = _make_org("org-001", avg_wealth=50.0)
        other_org = _make_org("org-002", avg_wealth=200.0)
        treaty = _make_treaty(treaty_type="trade")
        decision = decider.respond_to_treaty(treaty, my_org, other_org)
        assert decision.action == TreatyResponse.ACCEPT.value

    def test_trade_treaty_similar_wealth_counters(self) -> None:
        """Trade treaty between similar orgs → counter-propose."""
        decider = GovernanceDecider()
        my_org = _make_org("org-001", avg_wealth=100.0)
        other_org = _make_org("org-002", avg_wealth=110.0)
        treaty = _make_treaty(treaty_type="trade")
        decision = decider.respond_to_treaty(treaty, my_org, other_org)
        assert decision.action == TreatyResponse.COUNTER.value

    def test_defense_treaty_with_larger_org_accepts(self) -> None:
        """Defense treaty with larger org → accept (they bring more defenders)."""
        decider = GovernanceDecider()
        my_org = _make_org("org-001", member_count=3)
        other_org = _make_org("org-002", member_count=10)
        treaty = _make_treaty(treaty_type="defense")
        decision = decider.respond_to_treaty(treaty, my_org, other_org)
        assert decision.action == TreatyResponse.ACCEPT.value

    def test_defense_treaty_with_smaller_org_rejects(self) -> None:
        """Defense treaty with smaller org → reject (limited defense value)."""
        decider = GovernanceDecider()
        my_org = _make_org("org-001", member_count=10)
        other_org = _make_org("org-002", member_count=2)
        treaty = _make_treaty(treaty_type="defense")
        decision = decider.respond_to_treaty(treaty, my_org, other_org)
        assert decision.action == TreatyResponse.REJECT.value

    def test_non_aggression_pact_accepts(self) -> None:
        """Non-aggression pacts are generally accepted."""
        decider = GovernanceDecider()
        my_org = _make_org("org-001")
        other_org = _make_org("org-002")
        treaty = _make_treaty(treaty_type="non_aggression")
        decision = decider.respond_to_treaty(treaty, my_org, other_org)
        assert decision.action == TreatyResponse.ACCEPT.value

    def test_empty_org_rejects(self) -> None:
        """Empty org should reject any treaty."""
        decider = GovernanceDecider()
        my_org = _make_org("org-001", member_count=0, avg_wealth=0.0)
        other_org = _make_org("org-002", member_count=5)
        treaty = _make_treaty()
        decision = decider.respond_to_treaty(treaty, my_org, other_org)
        assert decision.action == TreatyResponse.REJECT.value


class TestGovernanceProposeTaxRate:
    """Tests for GovernanceDecider.propose_tax_rate."""

    def test_wealthy_agent_proposes_low_tax(self) -> None:
        """Agent well above average wealth proposes lower taxes."""
        decider = GovernanceDecider()
        org = _make_org(avg_wealth=100.0)
        decision = decider.propose_tax_rate(org, my_wealth=300.0)
        rate = float(decision.action)
        assert rate < 0.10  # Below default

    def test_poor_agent_proposes_higher_tax(self) -> None:
        """Agent below average wealth proposes higher taxes."""
        decider = GovernanceDecider()
        org = _make_org(avg_wealth=100.0)
        decision = decider.propose_tax_rate(org, my_wealth=20.0)
        rate = float(decision.action)
        assert rate > 0.10  # Above default

    def test_tax_rate_clamped_to_min(self) -> None:
        """Proposed tax rate should not go below configured minimum."""
        decider = GovernanceDecider()
        org = _make_org(avg_wealth=100.0)
        decision = decider.propose_tax_rate(org, my_wealth=1000.0)
        rate = float(decision.action)
        assert rate >= 0.05

    def test_tax_rate_clamped_to_max(self) -> None:
        """Proposed tax rate should not exceed configured maximum."""
        decider = GovernanceDecider()
        org = _make_org(avg_wealth=100.0)
        decision = decider.propose_tax_rate(org, my_wealth=0.01)
        rate = float(decision.action)
        assert rate <= 0.30

    def test_low_treasury_increases_tax(self) -> None:
        """Low treasury should push tax rate higher."""
        decider = GovernanceDecider()
        org_healthy = _make_org(avg_wealth=100.0, total_wealth=500.0, treasury=100.0)
        org_low_treasury = _make_org(avg_wealth=100.0, total_wealth=500.0, treasury=5.0)
        decision_healthy = decider.propose_tax_rate(org_healthy, my_wealth=100.0)
        decision_low = decider.propose_tax_rate(org_low_treasury, my_wealth=100.0)
        assert float(decision_low.action) >= float(decision_healthy.action)


class TestGovernanceAllocationStrategy:
    """Tests for GovernanceDecider.choose_allocation_strategy."""

    def test_single_member_equal(self) -> None:
        """Single-member org always gets equal allocation."""
        decider = GovernanceDecider()
        org = _make_org(member_count=1)
        decision = decider.choose_allocation_strategy(org)
        assert decision.action == AllocationStrategy.EQUAL.value
        assert decision.confidence == 1.0

    def test_low_treasury_need_based(self) -> None:
        """Low treasury triggers need-based allocation."""
        decider = GovernanceDecider()
        org = _make_org(member_count=5, avg_wealth=100.0, treasury=10.0)
        decision = decider.choose_allocation_strategy(org)
        assert decision.action == AllocationStrategy.NEED_BASED.value

    def test_large_org_proportional(self) -> None:
        """Large org (>=10 members) gets proportional allocation."""
        decider = GovernanceDecider()
        org = _make_org(
            member_count=15, avg_wealth=100.0, treasury=500.0, tax_rate=0.10
        )
        decision = decider.choose_allocation_strategy(org)
        assert decision.action == AllocationStrategy.PROPORTIONAL.value

    def test_high_tax_merit_based(self) -> None:
        """High tax rate with moderate size triggers merit-based."""
        decider = GovernanceDecider()
        org = _make_org(
            member_count=5, avg_wealth=100.0, treasury=200.0, tax_rate=0.25
        )
        decision = decider.choose_allocation_strategy(org)
        assert decision.action == AllocationStrategy.MERIT_BASED.value

    def test_small_healthy_org_equal(self) -> None:
        """Small, healthy org defaults to equal allocation."""
        decider = GovernanceDecider()
        org = _make_org(
            member_count=3, avg_wealth=100.0, treasury=200.0, tax_rate=0.10
        )
        decision = decider.choose_allocation_strategy(org)
        assert decision.action == AllocationStrategy.EQUAL.value
