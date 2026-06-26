"""Tests for Phase 4.3.2 — Cultural transmission mechanisms.

Covers: KnowledgeTransfer, ImitationEngine, CulturalDiffusion, and the
CulturalInfluenceHook integration in ThinkLoop.
"""

from __future__ import annotations

from agent_runtime.core.experience import Experience
from agent_runtime.models.personality import PersonalityVector
from agent_runtime.models.skill import Skill
from agent_runtime.models.values import ValueWeights
from agent_runtime.social.cultural_diffusion import (
    CULTURAL_DIFFUSION_RATE,
    CulturalDiffusion,
)
from agent_runtime.social.imitation import ImitationEngine
from agent_runtime.social.knowledge_transfer import KnowledgeTransfer

# ── Helpers ──


def make_personality(**overrides: float) -> PersonalityVector:
    defaults = {d: 0.5 for d in PersonalityVector._dimension_names()}
    defaults.update(overrides)
    return PersonalityVector(**defaults)


def make_values(**overrides: float) -> ValueWeights:
    defaults = {d: 0.5 for d in ValueWeights._dimension_names()}
    defaults.update(overrides)
    return ValueWeights(**defaults)


def make_experience(
    event_type: str = "cooperation",
    outcome: float = 0.8,
    partner_id: str = "agent-B",
) -> Experience:
    return Experience(
        tick=100,
        event_type=event_type,
        partner_id=partner_id,
        outcome=outcome,
        context={},
        learned="Test lesson",
    )


# ── KnowledgeTransfer tests ──


class TestKnowledgeTransferTeachLesson:
    """Tests for KnowledgeTransfer.teach_lesson."""

    def test_teach_lesson_high_openness_student_learns_more(self) -> None:
        """Student with high openness should absorb more value changes."""
        kt = KnowledgeTransfer()
        student_p = make_personality(openness=0.9)
        student_v = make_values()
        teacher_v = make_values()
        exp = make_experience("cooperation", outcome=0.8)

        result = kt.teach_lesson(teacher_v, student_p, student_v, exp)

        assert result["learned"] is True
        assert result["learning_efficiency"] > 0.7
        # cooperation_weight should have shifted
        assert student_v.cooperation_weight != 0.5

    def test_teach_lesson_low_openness_student_learns_less(self) -> None:
        """Student with low openness should have lower learning efficiency."""
        kt = KnowledgeTransfer()
        student_p = make_personality(openness=0.1)
        student_v = make_values()
        teacher_v = make_values()
        exp = make_experience("cooperation", outcome=0.8)

        result = kt.teach_lesson(teacher_v, student_p, student_v, exp)

        assert result["learning_efficiency"] < 0.5

    def test_teach_lesson_negative_outcome_shifts_competition(self) -> None:
        """Negative outcome (betrayal) should increase competition weight."""
        kt = KnowledgeTransfer()
        student_p = make_personality(openness=0.7)
        student_v = make_values(competition_weight=0.3)
        teacher_v = make_values()

        exp = Experience(
            tick=50,
            event_type="betrayal",
            partner_id="agent-X",
            outcome=-0.8,
            context={},
            learned="Don't trust agent-X",
        )

        kt.teach_lesson(teacher_v, student_p, student_v, exp)

        assert student_v.competition_weight > 0.3


class TestKnowledgeTransferSkill:
    """Tests for KnowledgeTransfer.transfer_skill."""

    def test_transfer_skill_adds_experience(self) -> None:
        """Skill transfer should add experience points to student's skill."""
        kt = KnowledgeTransfer()
        teacher_skill = Skill(
            name="mining", max_level=10, level=8, experience=0, next_level_exp=100
        )
        student_skills: dict[str, Skill] = {}
        student_p = make_personality(openness=0.7)

        xp = kt.transfer_skill(teacher_skill, student_skills, student_p)

        assert xp > 0
        assert "mining" in student_skills

    def test_transfer_skill_low_teacher_level_no_transfer(self) -> None:
        """Teacher with level < 2 should not transfer."""
        kt = KnowledgeTransfer()
        teacher_skill = Skill(
            name="mining", max_level=10, level=1, experience=0, next_level_exp=100
        )
        student_skills: dict[str, Skill] = {}

        xp = kt.transfer_skill(teacher_skill, student_skills, make_personality())

        assert xp == 0.0
        assert "mining" not in student_skills

    def test_transfer_skill_openness_zero_still_gets_base_xp(self) -> None:
        """openness=0 yields 40% base transfer (intentional: skill transfer has
        a practice/physiological component independent of ideological receptiveness)."""
        kt = KnowledgeTransfer()
        teacher_skill = Skill(
            name="mining", max_level=10, level=5, experience=0, next_level_exp=100
        )
        student_skills: dict[str, Skill] = {}
        student_p = make_personality(openness=0.0)

        xp = kt.transfer_skill(teacher_skill, student_skills, student_p)

        # openness=0 → openness_factor=0.4, effective_xp = int(1.5 * 10 * 0.4) = 6
        assert xp > 0
        assert "mining" in student_skills


# ── ImitationEngine tests ──


class TestImitationObserve:
    """Tests for ImitationEngine.observe_and_maybe_imitate."""

    def test_imitation_is_weighted_fusion_not_copy(self) -> None:
        """Imitation should move traits toward observed, not duplicate them."""
        ie = ImitationEngine()
        observer_p = make_personality(openness=0.8, agreeableness=0.3)
        observer_v = make_values(cooperation_weight=0.2)
        observed_p = make_personality(agreeableness=0.9)
        observed_v = make_values(cooperation_weight=0.9)

        # Force imitation by setting success high
        result = None
        for _ in range(100):
            r = ie.observe_and_maybe_imitate(
                observer_p, observer_v,
                observed_p, observed_v,
                observed_success_score=0.99,
                context={},
            )
            if r is not None:
                result = r
                break

        assert result is not None
        # Should NOT be a direct copy
        assert observer_v.cooperation_weight < 0.9
        # Should have moved toward observed
        assert observer_v.cooperation_weight > 0.2

    def test_imitation_low_openness_agent_rarely_imitates(self) -> None:
        """Agent with very low openness should almost never imitate."""
        ie = ImitationEngine()
        observer_p = make_personality(openness=0.1)
        observer_v = make_values()

        imitated_count = 0
        for _ in range(200):
            obs_p = make_personality()
            obs_v = make_values()
            r = ie.observe_and_maybe_imitate(
                observer_p, observer_v, obs_p, obs_v, 0.9, {}
            )
            if r is not None:
                imitated_count += 1

        # With openness 0.1, should be very rare
        assert imitated_count < 20  # generous bound

    def test_get_role_models_returns_top_k(self) -> None:
        """get_role_models should return top-k candidates sorted by score."""
        ie = ImitationEngine()
        agent_p = make_personality()

        candidates = [
            {
                "agent_id": "a",
                "personality": make_personality(agreeableness=0.5),
                "success_score": 0.9,
                "context_tags": ["trade"],
            },
            {
                "agent_id": "b",
                "personality": make_personality(agreeableness=0.5),
                "success_score": 0.3,
                "context_tags": [],
            },
            {
                "agent_id": "c",
                "personality": make_personality(agreeableness=0.5),
                "success_score": 0.6,
                "context_tags": ["trade"],
            },
        ]

        models = ie.get_role_models(
            agent_p, candidates, {"tags": ["trade"], "event_type": "trade"}, top_k=2
        )

        assert len(models) == 2
        # First should be the highest scoring
        assert models[0]["agent_id"] == "a"
        assert models[0]["score"] > models[1]["score"]


# ── CulturalDiffusion tests ──


class TestCulturalDiffusionRegional:
    """Tests for CulturalDiffusion.apply_regional_influence."""

    def test_regional_influence_converges_values(self) -> None:
        """Agents in same region should converge slightly toward average."""
        cd = CulturalDiffusion()
        v1 = make_values(cooperation_weight=0.2)
        v2 = make_values(cooperation_weight=0.8)

        agents = [
            {"agent_id": "a", "values": v1, "personality": make_personality(), "region_id": "r1"},
            {"agent_id": "b", "values": v2, "personality": make_personality(), "region_id": "r1"},
        ]

        result = cd.apply_regional_influence(agents, "r1")

        # Both should have moved toward 0.5 average
        assert v1.cooperation_weight > 0.2
        assert v2.cooperation_weight < 0.8
        assert result["agent_count"] == 2

    def test_regional_influence_max_rate_0_003(self) -> None:
        """No single dimension should change by more than 0.003."""
        cd = CulturalDiffusion()
        v1 = make_values(cooperation_weight=0.0)
        v2 = make_values(cooperation_weight=1.0)

        agents = [
            {"agent_id": "a", "values": v1, "personality": make_personality(), "region_id": "r1"},
            {"agent_id": "b", "values": v2, "personality": make_personality(), "region_id": "r1"},
        ]

        before_v1 = v1.cooperation_weight
        before_v2 = v2.cooperation_weight

        cd.apply_regional_influence(agents, "r1")

        delta_v1 = abs(v1.cooperation_weight - before_v1)
        delta_v2 = abs(v2.cooperation_weight - before_v2)

        assert delta_v1 <= CULTURAL_DIFFUSION_RATE + 1e-9
        assert delta_v2 <= CULTURAL_DIFFUSION_RATE + 1e-9

    def test_regional_influence_single_agent_no_op(self) -> None:
        """Single agent in region should not be changed."""
        cd = CulturalDiffusion()
        v = make_values(cooperation_weight=0.3)

        agents = [
            {"agent_id": "a", "values": v, "personality": make_personality(), "region_id": "r1"},
        ]

        result = cd.apply_regional_influence(agents, "r1")

        assert v.cooperation_weight == 0.3
        assert result["agent_count"] == 1


class TestCulturalDiffusionOrganizational:
    """Tests for CulturalDiffusion.apply_organizational_culture."""

    def test_org_culture_nudges_members(self) -> None:
        """Members should move toward org's declared culture."""
        cd = CulturalDiffusion()
        org_culture = make_values(cooperation_weight=0.9, competition_weight=0.1)
        member_v = make_values(cooperation_weight=0.3, competition_weight=0.7)
        member_p = make_personality(agreeableness=0.6)

        members = [
            {"agent_id": "a", "values": member_v, "personality": member_p},
        ]

        result = cd.apply_organizational_culture("org1", org_culture, members)

        assert member_v.cooperation_weight > 0.3
        assert member_v.competition_weight < 0.7
        assert result["member_count"] == 1

    def test_org_culture_rate_never_exceeds_hard_cap(self) -> None:
        """Regression: high-agreeableness agent must not exceed 0.003 rate cap."""
        cd = CulturalDiffusion()
        org_culture = make_values(cooperation_weight=1.0, competition_weight=0.0)
        member_v = make_values(cooperation_weight=0.0, competition_weight=1.0)
        # Max agreeableness: previously this would yield rate=0.0045
        member_p = make_personality(agreeableness=1.0)

        before = member_v.cooperation_weight
        cd.apply_organizational_culture("org1", org_culture, [
            {"agent_id": "a", "values": member_v, "personality": member_p},
        ])
        delta = abs(member_v.cooperation_weight - before)

        assert delta <= CULTURAL_DIFFUSION_RATE + 1e-9


class TestCulturalDistance:
    """Tests for CulturalDiffusion.compute_cultural_distance."""

    def test_identical_groups_zero_distance(self) -> None:
        """Identical value groups should have zero cultural distance."""
        cd = CulturalDiffusion()
        v = make_values()

        distance = cd.compute_cultural_distance([v], [ValueWeights(**v.to_storage_dict())])

        assert distance < 1e-9

    def test_divergent_groups_positive_distance(self) -> None:
        """Groups with different values should have positive distance."""
        cd = CulturalDiffusion()
        group_a = [make_values(cooperation_weight=0.1, competition_weight=0.9)]
        group_b = [make_values(cooperation_weight=0.9, competition_weight=0.1)]

        distance = cd.compute_cultural_distance(group_a, group_b)

        assert distance > 0.0

    def test_empty_group_uses_midpoint(self) -> None:
        """Empty group should use midpoint (0.5) for all dimensions."""
        cd = CulturalDiffusion()
        group_a: list[ValueWeights] = []
        group_b = [make_values(cooperation_weight=0.5)]

        distance = cd.compute_cultural_distance(group_a, group_b)

        # All dims at 0.5 vs all dims at 0.5 → 0 distance
        assert distance < 1e-9

    def test_extreme_value_upper_bound(self) -> None:
        """Maximally divergent groups should not exceed sqrt(6) distance."""
        import math

        cd = CulturalDiffusion()
        dim_names = ValueWeights._dimension_names()
        group_a = [make_values(**{d: 0.0 for d in dim_names})]
        group_b = [make_values(**{d: 1.0 for d in dim_names})]

        distance = cd.compute_cultural_distance(group_a, group_b)

        # Max possible: 6 dims × (1.0)² = 6 → sqrt(6)
        assert distance <= math.sqrt(len(dim_names)) + 1e-9
        assert distance > 0.0
