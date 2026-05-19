"""Tests for AgentState data model, enums, Skill, and lifecycle abilities.

Covers: creation, validation, serialization, state mutations, lifecycle helpers,
phase abilities, and World Engine sync.
"""

import json
from uuid import UUID

import pytest
from pydantic import ValidationError

from agent_runtime.models import (
    AgentPhase,
    AgentState,
    DeathReason,
    Skill,
    SurvivalMode,
    get_phase_abilities,
    is_alive,
    is_terminal,
)

# ============================================================
# Enum tests
# ============================================================


class TestAgentPhase:
    def test_all_phases_defined(self):
        expected = {
            "birth",
            "childhood",
            "adult",
            "elder",
            "dying",
            "dead",
        }
        assert {p.value for p in AgentPhase} == expected

    def test_phase_is_string_enum(self):
        assert isinstance(AgentPhase.ADULT, str)
        assert AgentPhase.ADULT == "adult"

    def test_phase_from_value(self):
        assert AgentPhase("childhood") is AgentPhase.CHILDHOOD

    def test_phase_ordering(self):
        """Phases should follow lifecycle order."""
        phases = [p.value for p in AgentPhase]
        expected_order = ["birth", "childhood", "adult", "elder", "dying", "dead"]
        assert phases == expected_order


class TestDeathReason:
    def test_all_reasons_defined(self):
        expected = {
            "token_depleted",
            "human_terminated",
            "vote_evicted",
            "natural_death",
        }
        assert {r.value for r in DeathReason} == expected

    def test_death_reason_is_string_enum(self):
        assert isinstance(DeathReason.TOKEN_DEPLETED, str)


class TestSurvivalMode:
    def test_all_modes_defined(self):
        expected = {"conservation", "adaptation", "expansion", "crisis"}
        assert {m.value for m in SurvivalMode} == expected

    def test_mode_is_string_enum(self):
        assert isinstance(SurvivalMode.CRISIS, str)


# ============================================================
# PhaseAbilities tests
# ============================================================


class TestPhaseAbilities:
    def test_birth_abilities(self):
        ab = get_phase_abilities(AgentPhase.BIRTH)
        assert ab.skill_efficiency == 0.0
        assert ab.can_learn is True
        assert ab.can_take_tasks is False
        assert ab.can_trade is False
        assert ab.can_communicate is True

    def test_childhood_abilities(self):
        ab = get_phase_abilities(AgentPhase.CHILDHOOD)
        assert abs(ab.skill_efficiency - 0.3) < 1e-9
        assert ab.can_learn is True
        assert ab.can_take_tasks is True
        assert ab.can_trade is False
        assert ab.can_teach is False

    def test_adult_abilities(self):
        ab = get_phase_abilities(AgentPhase.ADULT)
        assert ab.skill_efficiency == 1.0
        assert ab.can_learn is True
        assert ab.can_take_tasks is True
        assert ab.can_trade is True
        assert ab.can_teach is True
        assert ab.can_write_will is True

    def test_elder_abilities(self):
        ab = get_phase_abilities(AgentPhase.ELDER)
        assert abs(ab.skill_efficiency - 0.6) < 1e-9
        assert ab.can_learn is True
        assert ab.can_take_tasks is True
        assert ab.can_trade is True
        assert ab.can_teach is True
        assert ab.can_write_will is True

    def test_dying_abilities(self):
        ab = get_phase_abilities(AgentPhase.DYING)
        assert abs(ab.skill_efficiency - 0.1) < 1e-9
        assert ab.can_learn is False
        assert ab.can_take_tasks is False
        assert ab.can_trade is False
        assert ab.can_write_will is True
        assert ab.can_communicate is True

    def test_dead_abilities(self):
        ab = get_phase_abilities(AgentPhase.DEAD)
        assert ab.skill_efficiency == 0.0
        assert ab.can_learn is False
        assert ab.can_take_tasks is False
        assert ab.can_trade is False
        assert ab.can_teach is False
        assert ab.can_write_will is False
        assert ab.can_communicate is False


class TestLifecycleHelpers:
    def test_is_alive(self):
        assert is_alive(AgentPhase.BIRTH) is True
        assert is_alive(AgentPhase.CHILDHOOD) is True
        assert is_alive(AgentPhase.ADULT) is True
        assert is_alive(AgentPhase.ELDER) is True
        assert is_alive(AgentPhase.DYING) is True
        assert is_alive(AgentPhase.DEAD) is False

    def test_is_terminal(self):
        assert is_terminal(AgentPhase.DEAD) is True
        assert is_terminal(AgentPhase.ADULT) is False
        assert is_terminal(AgentPhase.DYING) is False


# ============================================================
# Skill tests
# ============================================================


class TestSkill:
    def test_create_skill_defaults(self):
        s = Skill(name="coding")
        assert s.name == "coding"
        assert s.level == 1
        assert s.experience == 0
        assert s.max_level == 10
        assert s.next_level_exp == 100

    def test_create_skill_custom(self):
        s = Skill(name="combat", level=5, experience=50, max_level=20, next_level_exp=200)
        assert s.level == 5
        assert s.max_level == 20

    def test_skill_name_required(self):
        with pytest.raises(ValidationError):
            Skill()

    def test_skill_name_not_empty(self):
        with pytest.raises(ValidationError):
            Skill(name="")

    def test_level_cannot_exceed_max(self):
        with pytest.raises(ValidationError):
            Skill(name="test", level=11, max_level=10)

    def test_level_below_custom_max_level(self):
        """level < max_level but level > default max_level (10) should succeed."""
        s = Skill(name="test", level=15, max_level=20)
        assert s.level == 15
        assert s.max_level == 20

    def test_negative_level_rejected(self):
        with pytest.raises(ValidationError):
            Skill(name="test", level=0)

    def test_add_experience_no_level_up(self):
        s = Skill(name="coding", level=1, experience=0, next_level_exp=100)
        result = s.add_experience(50)
        assert result is False
        assert s.experience == 50
        assert s.level == 1

    def test_add_experience_single_level_up(self):
        s = Skill(name="coding", level=1, experience=0, next_level_exp=100)
        result = s.add_experience(100)
        assert result is True
        assert s.level == 2
        assert s.experience == 0
        assert s.next_level_exp == 150  # 100 * 1.5

    def test_add_experience_multi_level_up(self):
        s = Skill(name="coding", level=1, experience=0, next_level_exp=100)
        result = s.add_experience(250)  # 100 for level 2, 150 for level 3
        assert result is True
        assert s.level == 3
        assert s.experience == 0
        assert s.next_level_exp == 225  # 150 * 1.5

    def test_experience_at_max_level(self):
        s = Skill(name="coding", level=10, max_level=10, next_level_exp=100)
        result = s.add_experience(500)
        assert result is False
        assert s.level == 10

    def test_skill_serialization(self):
        s = Skill(name="coding", level=3, experience=50, max_level=10, next_level_exp=100)
        data = s.model_dump()
        assert data == {
            "name": "coding",
            "max_level": 10,
            "level": 3,
            "experience": 50,
            "next_level_exp": 100,
        }

    def test_skill_json_roundtrip(self):
        s = Skill(name="combat", level=7, experience=30, max_level=15, next_level_exp=200)
        json_str = s.model_dump_json()
        restored = Skill.model_validate_json(json_str)
        assert restored == s


# ============================================================
# AgentState tests
# ============================================================


class TestAgentStateCreation:
    def test_create_minimal(self):
        agent = AgentState(name="TestAgent")
        assert agent.name == "TestAgent"
        assert isinstance(agent.id, UUID)
        assert agent.phase == AgentPhase.BIRTH
        assert agent.survival_mode == SurvivalMode.CONSERVATION
        assert agent.tokens == 100
        assert agent.money == 50.0
        assert agent.health == 100.0
        assert agent.reputation == 0.0
        assert agent.skills == {}
        assert agent.personality == {}
        assert agent.world_sync_version == 0
        assert agent.spawn_tick == 0
        assert agent.death_reason is None

    def test_create_with_all_fields(self):
        agent = AgentState(
            name="FullAgent",
            phase=AgentPhase.ADULT,
            survival_mode=SurvivalMode.ADAPTATION,
            tokens=500,
            money=1000.0,
            health=80.0,
            reputation=50.0,
            skills={"coding": Skill(name="coding", level=5)},
            personality={"boldness": 0.8, "caution": 0.3},
        )
        assert agent.phase == AgentPhase.ADULT
        assert "coding" in agent.skills

    def test_name_required(self):
        with pytest.raises(ValidationError):
            AgentState()

    def test_name_not_empty(self):
        with pytest.raises(ValidationError):
            AgentState(name="")

    def test_health_bounds(self):
        with pytest.raises(ValidationError):
            AgentState(name="test", health=101.0)
        with pytest.raises(ValidationError):
            AgentState(name="test", health=-1.0)

    def test_reputation_bounds(self):
        with pytest.raises(ValidationError):
            AgentState(name="test", reputation=101.0)
        with pytest.raises(ValidationError):
            AgentState(name="test", reputation=-101.0)

    def test_negative_tokens_rejected(self):
        with pytest.raises(ValidationError):
            AgentState(name="test", tokens=-1)

    def test_negative_money_rejected(self):
        with pytest.raises(ValidationError):
            AgentState(name="test", money=-1.0)

    def test_skills_from_list(self):
        """Skills can be passed as a list and are normalized to a dict."""
        agent = AgentState(
            name="test",
            skills=[Skill(name="coding", level=3), Skill(name="combat", level=1)],
        )
        assert len(agent.skills) == 2
        assert agent.skills["coding"].level == 3

    def test_death_reason_field(self):
        agent = AgentState(
            name="DeadAgent",
            phase=AgentPhase.DEAD,
            death_reason=DeathReason.TOKEN_DEPLETED,
        )
        assert agent.death_reason == DeathReason.TOKEN_DEPLETED


class TestAgentStateMutations:
    def setup_method(self):
        self.agent = AgentState(name="TestAgent")

    def test_add_skill(self):
        skill = Skill(name="mining", level=2)
        self.agent.add_skill(skill)
        assert "mining" in self.agent.skills
        assert self.agent.skills["mining"].level == 2
        assert self.agent.world_sync_version == 1

    def test_remove_skill(self):
        self.agent.add_skill(Skill(name="mining", level=2))
        removed = self.agent.remove_skill("mining")
        assert removed is not None
        assert removed.name == "mining"
        assert "mining" not in self.agent.skills

    def test_remove_nonexistent_skill(self):
        removed = self.agent.remove_skill("nothing")
        assert removed is None

    def test_adjust_tokens_positive(self):
        self.agent.adjust_tokens(50)
        assert self.agent.tokens == 150

    def test_adjust_tokens_negative(self):
        self.agent.adjust_tokens(-30)
        assert self.agent.tokens == 70

    def test_adjust_tokens_cannot_go_negative(self):
        with pytest.raises(ValueError, match="Cannot reduce tokens below 0"):
            self.agent.adjust_tokens(-200)

    def test_adjust_money_positive(self):
        self.agent.adjust_money(25.50)
        assert self.agent.money == 75.50

    def test_adjust_money_negative(self):
        self.agent.adjust_money(-20.0)
        assert self.agent.money == 30.0

    def test_adjust_money_cannot_go_negative(self):
        with pytest.raises(ValueError, match="Cannot reduce money below 0"):
            self.agent.adjust_money(-100.0)

    def test_adjust_health_positive(self):
        self.agent.health = 50.0
        self.agent.adjust_health(20.0)
        assert self.agent.health == 70.0

    def test_adjust_health_clamped_at_100(self):
        self.agent.health = 90.0
        self.agent.adjust_health(20.0)
        assert self.agent.health == 100.0

    def test_adjust_health_clamped_at_0(self):
        self.agent.adjust_health(-200.0)
        assert self.agent.health == 0.0

    def test_adjust_reputation_positive(self):
        self.agent.adjust_reputation(30.0)
        assert self.agent.reputation == 30.0

    def test_adjust_reputation_clamped_at_100(self):
        self.agent.adjust_reputation(150.0)
        assert self.agent.reputation == 100.0

    def test_adjust_reputation_clamped_at_neg100(self):
        self.agent.adjust_reputation(-150.0)
        assert self.agent.reputation == -100.0

    def test_transition_phase(self):
        self.agent.transition_phase(AgentPhase.CHILDHOOD)
        assert self.agent.phase == AgentPhase.CHILDHOOD

    def test_set_survival_mode(self):
        self.agent.set_survival_mode(SurvivalMode.CRISIS)
        assert self.agent.survival_mode == SurvivalMode.CRISIS

    def test_version_increments_on_mutation(self):
        initial_version = self.agent.world_sync_version
        self.agent.adjust_tokens(10)
        assert self.agent.world_sync_version == initial_version + 1
        self.agent.adjust_health(-5)
        assert self.agent.world_sync_version == initial_version + 2


class TestAgentStateLifecycleHelpers:
    """Tests for the lifecycle helper methods on AgentState."""

    def test_get_phase_abilities(self):
        agent = AgentState(name="TestAgent", phase=AgentPhase.ADULT)
        ab = agent.get_phase_abilities()
        assert ab.can_trade is True
        assert ab.can_write_will is True

    def test_is_alive_method(self):
        agent = AgentState(name="TestAgent", phase=AgentPhase.ADULT)
        assert agent.is_alive() is True

        agent.transition_phase(AgentPhase.DEAD)
        assert agent.is_alive() is False

    def test_is_dead_method(self):
        agent = AgentState(name="TestAgent", phase=AgentPhase.ADULT)
        assert agent.is_dead() is False

        agent.transition_phase(AgentPhase.DEAD)
        assert agent.is_dead() is True

    def test_can_perform_adult(self):
        agent = AgentState(name="TestAgent", phase=AgentPhase.ADULT)
        assert agent.can_perform("claim_task") is True
        assert agent.can_perform("send_message") is True
        assert agent.can_perform("propose_deal") is True
        assert agent.can_perform("teach_skill") is True
        assert agent.can_perform("rest") is True

    def test_can_perform_childhood(self):
        agent = AgentState(name="TestAgent", phase=AgentPhase.CHILDHOOD)
        assert agent.can_perform("claim_task") is True
        assert agent.can_perform("send_message") is True
        assert agent.can_perform("propose_deal") is False  # can_trade=False
        assert agent.can_perform("teach_skill") is False   # can_teach=False

    def test_can_perform_dying(self):
        agent = AgentState(name="TestAgent", phase=AgentPhase.DYING)
        assert agent.can_perform("claim_task") is False
        assert agent.can_perform("send_message") is True  # can still communicate
        assert agent.can_perform("propose_deal") is False

    def test_can_perform_dead(self):
        agent = AgentState(name="TestAgent", phase=AgentPhase.DEAD)
        assert agent.can_perform("claim_task") is False
        assert agent.can_perform("send_message") is False
        assert agent.can_perform("rest") is False


class TestAgentStateSync:
    def test_to_sync_payload(self):
        agent = AgentState(
            name="SyncAgent",
            phase=AgentPhase.ADULT,
            skills={"coding": Skill(name="coding", level=3)},
        )
        payload = agent.to_sync_payload()
        assert isinstance(payload["id"], str)
        assert payload["name"] == "SyncAgent"
        assert payload["phase"] == "adult"
        assert "coding" in payload["skills"]
        assert payload["world_sync_version"] >= 0

    def test_from_sync_payload(self):
        payload = {
            "name": "RemoteAgent",
            "phase": "elder",
            "survival_mode": "expansion",
            "tokens": 300,
            "money": 200.0,
            "health": 90.0,
            "reputation": 25.0,
            "skills": {"mining": {"name": "mining", "level": 4}},
            "personality": {"boldness": 0.7},
        }
        agent = AgentState.from_sync_payload(payload)
        assert agent.name == "RemoteAgent"
        assert agent.phase == AgentPhase.ELDER
        assert agent.survival_mode == SurvivalMode.EXPANSION
        assert agent.skills["mining"].level == 4

    def test_apply_sync_newer_version(self):
        agent = AgentState(name="LocalAgent", tokens=100)
        agent.world_sync_version = 5

        remote_payload = {
            "id": str(agent.id),
            "name": "LocalAgent",
            "tokens": 500,
            "money": 50.0,
            "health": 100.0,
            "reputation": 0.0,
            "phase": "adult",
            "survival_mode": "conservation",
            "skills": {},
            "personality": {},
            "world_sync_version": 10,
        }
        agent.apply_sync(remote_payload)
        assert agent.tokens == 500
        assert agent.world_sync_version == 10

    def test_apply_sync_older_version_ignored(self):
        agent = AgentState(name="LocalAgent", tokens=200)
        agent.world_sync_version = 10

        remote_payload = {
            "name": "LocalAgent",
            "tokens": 999,
            "money": 50.0,
            "health": 100.0,
            "reputation": 0.0,
            "phase": "birth",
            "survival_mode": "conservation",
            "skills": {},
            "personality": {},
            "world_sync_version": 5,
        }
        agent.apply_sync(remote_payload)
        assert agent.tokens == 200  # Unchanged

    def test_apply_sync_syncs_all_fields(self):
        """Verify apply_sync overwrites every defined field, including personality."""
        agent = AgentState(name="LocalAgent", personality={"old_trait": 1.0})
        agent.world_sync_version = 0

        remote_payload = {
            "id": str(agent.id),
            "name": "RemoteName",
            "tokens": 999,
            "money": 10.0,
            "health": 50.0,
            "reputation": 75.0,
            "phase": "elder",
            "survival_mode": "crisis",
            "skills": {"crafting": {"name": "crafting", "level": 8}},
            "personality": {"new_trait": 0.5},
            "world_sync_version": 1,
        }
        agent.apply_sync(remote_payload)
        assert agent.name == "RemoteName"
        assert agent.personality == {"new_trait": 0.5}
        assert "crafting" in agent.skills


class TestAgentStateSerialization:
    def test_json_roundtrip(self):
        agent = AgentState(
            name="RoundTrip",
            phase=AgentPhase.ADULT,
            tokens=250,
            money=75.5,
            health=85.0,
            reputation=40.0,
            skills={"coding": Skill(name="coding", level=5)},
            personality={"boldness": 0.9},
        )
        json_str = agent.to_json()
        restored = AgentState.from_json(json_str)
        assert restored.name == "RoundTrip"
        assert restored.phase == AgentPhase.ADULT
        assert restored.tokens == 250
        assert restored.money == 75.5
        assert restored.health == 85.0
        assert "coding" in restored.skills
        assert restored.skills["coding"].level == 5

    def test_dict_export(self):
        agent = AgentState(name="DictTest", tokens=42)
        data = agent.model_dump()
        assert isinstance(data["id"], UUID)
        assert data["name"] == "DictTest"
        assert data["tokens"] == 42

    def test_to_sync_payload_json_serializable(self):
        agent = AgentState(
            name="JsonTest",
            skills={"mining": Skill(name="mining", level=2)},
        )
        payload = agent.to_sync_payload()
        # Should not raise
        json_str = json.dumps(payload)
        assert "mining" in json_str
