"""Tests for the skills system: SkillRegistry, built-in skills, SkillExecutor, and XP.

Covers: registration, querying, upgrading, skill execution, XP calculation,
and experience accumulation across all four built-in skills.
"""

from __future__ import annotations

import pytest

from agent_runtime.models.skill import Skill
from agent_runtime.skills import (
    BUILTIN_SKILLS,
    CODING_SKILL,
    RESEARCH_SKILL,
    TEACHING_SKILL,
    TRADING_SKILL,
    SkillDefinition,
    SkillExecutionResult,
    SkillExecutor,
    SkillRegistry,
    XPReward,
    create_registry_with_builtins,
)

# ============================================================
# Helpers
# ============================================================


def _make_registry() -> SkillRegistry:
    """Create a registry with all built-in skills."""
    return create_registry_with_builtins()


# ============================================================
# SkillDefinition tests
# ============================================================


class TestSkillDefinition:
    def test_create_minimal(self):
        defn = SkillDefinition(name="test_skill")
        assert defn.name == "test_skill"
        assert defn.description == ""
        assert defn.max_level == 10
        assert defn.execute_fn is None
        assert defn.category == "general"

    def test_create_full(self):
        def fn(skills, **kw):
            return {"ok": True}
        defn = SkillDefinition(
            name="custom",
            description="A custom skill",
            max_level=20,
            execute_fn=fn,
            category="special",
        )
        assert defn.name == "custom"
        assert defn.description == "A custom skill"
        assert defn.max_level == 20
        assert defn.execute_fn is fn
        assert defn.category == "special"

    def test_frozen(self):
        defn = SkillDefinition(name="test")
        with pytest.raises(AttributeError):
            defn.name = "changed"


# ============================================================
# SkillRegistry tests
# ============================================================


class TestSkillRegistryRegister:
    def test_register_single(self):
        registry = SkillRegistry()
        defn = SkillDefinition(name="mining")
        registry.register(defn)
        assert registry.has("mining")
        assert registry.count == 1

    def test_register_duplicate_raises(self):
        registry = SkillRegistry()
        registry.register(SkillDefinition(name="mining"))
        with pytest.raises(ValueError, match="already registered"):
            registry.register(SkillDefinition(name="mining"))

    def test_register_multiple(self):
        registry = SkillRegistry()
        for s in BUILTIN_SKILLS:
            registry.register(s)
        assert registry.count == len(BUILTIN_SKILLS)


class TestSkillRegistryUnregister:
    def test_unregister_existing(self):
        registry = SkillRegistry()
        registry.register(SkillDefinition(name="mining"))
        removed = registry.unregister("mining")
        assert removed.name == "mining"
        assert not registry.has("mining")

    def test_unregister_nonexistent_raises(self):
        registry = SkillRegistry()
        with pytest.raises(KeyError, match="not registered"):
            registry.unregister("nothing")


class TestSkillRegistryUpgrade:
    def test_upgrade_existing(self):
        registry = SkillRegistry()
        registry.register(SkillDefinition(name="mining", description="v1"))
        registry.upgrade(SkillDefinition(name="mining", description="v2"))
        assert registry.get("mining").description == "v2"

    def test_upgrade_nonexistent_raises(self):
        registry = SkillRegistry()
        with pytest.raises(KeyError, match="not registered"):
            registry.upgrade(SkillDefinition(name="mining"))


class TestSkillRegistryQuery:
    def setup_method(self):
        self.registry = _make_registry()

    def test_get_existing(self):
        defn = self.registry.get("coding")
        assert defn.name == "coding"

    def test_get_nonexistent_raises(self):
        with pytest.raises(KeyError, match="not registered"):
            self.registry.get("nonexistent")

    def test_has(self):
        assert self.registry.has("coding")
        assert not self.registry.has("nonexistent")

    def test_list_skills_all(self):
        skills = self.registry.list_skills()
        names = [s.name for s in skills]
        assert "coding" in names
        assert "trading" in names
        assert "research" in names
        assert "teaching" in names
        assert len(skills) == len(BUILTIN_SKILLS)

    def test_list_skills_sorted_by_name(self):
        skills = self.registry.list_skills()
        names = [s.name for s in skills]
        assert names == sorted(names)

    def test_list_skills_by_category(self):
        skills = self.registry.list_skills(category="technical")
        assert len(skills) == 1
        assert skills[0].name == "coding"

    def test_list_skills_empty_category(self):
        skills = self.registry.list_skills(category="nonexistent")
        assert skills == []

    def test_categories(self):
        cats = self.registry.categories()
        assert "technical" in cats
        assert "economic" in cats
        assert "knowledge" in cats
        assert "social" in cats

    def test_count(self):
        assert self.registry.count == len(BUILTIN_SKILLS)


class TestSkillRegistryCreateSkill:
    def setup_method(self):
        self.registry = _make_registry()

    def test_create_skill_default_level(self):
        skill = self.registry.create_skill("coding")
        assert isinstance(skill, Skill)
        assert skill.name == "coding"
        assert skill.level == 1
        assert skill.max_level == 10
        assert skill.experience == 0

    def test_create_skill_custom_level(self):
        skill = self.registry.create_skill("trading", level=5)
        assert skill.level == 5

    def test_create_skill_nonexistent_raises(self):
        with pytest.raises(KeyError):
            self.registry.create_skill("nonexistent")


class TestCreateRegistryWithBuiltins:
    def test_all_builtins_registered(self):
        registry = create_registry_with_builtins()
        for skill_def in BUILTIN_SKILLS:
            assert registry.has(skill_def.name)

    def test_builtin_count(self):
        assert len(BUILTIN_SKILLS) == 5


# ============================================================
# Built-in skill execution tests
# ============================================================


class TestCodingSkill:
    def test_definition(self):
        assert CODING_SKILL.name == "coding"
        assert CODING_SKILL.category == "technical"
        assert CODING_SKILL.execute_fn is not None

    def test_execute_basic(self):
        skills = {"coding": Skill(name="coding", level=1)}
        result = CODING_SKILL.execute_fn(skills, task="hello world", language="python")
        assert result["skill"] == "coding"
        assert result["success"] is True
        assert result["language"] == "python"
        assert result["level_used"] == 1

    def test_execute_high_level(self):
        skills = {"coding": Skill(name="coding", level=9)}
        result = CODING_SKILL.execute_fn(skills, task="optimize")
        assert result["level_used"] == 9
        assert "advanced" in result["capability"]

    def test_execute_no_skill(self):
        result = CODING_SKILL.execute_fn({}, task="test")
        assert result["success"] is False
        assert result["level_used"] == 0


class TestTradingSkill:
    def test_definition(self):
        assert TRADING_SKILL.name == "trading"
        assert TRADING_SKILL.category == "economic"

    def test_execute_buy(self):
        skills = {"trading": Skill(name="trading", level=5)}
        result = TRADING_SKILL.execute_fn(skills, action="buy", item="gold", quantity=10)
        assert result["skill"] == "trading"
        assert result["action"] == "buy"
        assert result["item"] == "gold"
        assert result["quantity"] == 10
        assert result["success"] is True
        assert result["profit_margin"] == 0.25

    def test_execute_high_level_strategy(self):
        skills = {"trading": Skill(name="trading", level=9)}
        result = TRADING_SKILL.execute_fn(skills)
        assert result["strategy"] == "algorithmic trading"

    def test_profit_margin_capped(self):
        skills = {"trading": Skill(name="trading", level=10)}
        result = TRADING_SKILL.execute_fn(skills)
        assert result["profit_margin"] == 0.50


class TestResearchSkill:
    def test_definition(self):
        assert RESEARCH_SKILL.name == "research"
        assert RESEARCH_SKILL.category == "knowledge"

    def test_execute_basic(self):
        skills = {"research": Skill(name="research", level=3)}
        result = RESEARCH_SKILL.execute_fn(skills, topic="AI safety", depth="medium")
        assert result["skill"] == "research"
        assert result["topic"] == "AI safety"
        assert result["success"] is True
        assert abs(result["confidence"] - 0.30) < 1e-9

    def test_high_level_methodology(self):
        skills = {"research": Skill(name="research", level=9)}
        result = RESEARCH_SKILL.execute_fn(skills)
        assert result["methodology"] == "original research and synthesis"


class TestTeachingSkill:
    def test_definition(self):
        assert TEACHING_SKILL.name == "teaching"
        assert TEACHING_SKILL.category == "social"

    def test_execute_basic(self):
        skills = {"teaching": Skill(name="teaching", level=5)}
        result = TEACHING_SKILL.execute_fn(
            skills,
            subject="coding",
            target_skill="coding",
            target_level=3,
        )
        assert result["skill"] == "teaching"
        assert result["subject"] == "coding"
        assert result["target_skill"] == "coding"
        assert result["success"] is True
        assert result["effectiveness"] == 0.50

    def test_high_level_method(self):
        skills = {"teaching": Skill(name="teaching", level=9)}
        result = TEACHING_SKILL.execute_fn(skills)
        assert result["method"] == "experiential learning design"


# ============================================================
# SkillExecutor tests
# ============================================================


class TestSkillExecutorXP:
    def test_xp_use_only(self):
        """A skill execution that doesn't succeed earns only USE XP (10)."""
        registry = _make_registry()
        executor = SkillExecutor(registry)
        agent_skills = {"coding": Skill(name="coding", level=1)}

        # Override execute_fn to return failure
        registry.upgrade(
            SkillDefinition(
                name="coding",
                description=CODING_SKILL.description,
                max_level=CODING_SKILL.max_level,
                execute_fn=lambda skills, **kw: {"success": False},
                category=CODING_SKILL.category,
            )
        )

        result = executor.execute("coding", agent_skills)
        assert result.xp_earned == XPReward.USE.value  # 10
        assert result.xp_breakdown == {"use": 10}

    def test_xp_use_plus_success(self):
        """A successful skill execution earns USE + SUCCESS XP (10 + 30 = 40)."""
        registry = _make_registry()
        executor = SkillExecutor(registry)
        agent_skills = {"coding": Skill(name="coding", level=1)}

        result = executor.execute("coding", agent_skills, task="test")
        assert result.xp_earned == 40
        assert result.xp_breakdown["use"] == 10
        assert result.xp_breakdown["success"] == 30

    def test_xp_use_plus_success_plus_teaching(self):
        """A successful teaching execution earns USE + SUCCESS + TEACHING (10 + 30 + 50 = 90)."""
        registry = _make_registry()
        executor = SkillExecutor(registry)
        agent_skills = {"teaching": Skill(name="teaching", level=1)}

        result = executor.execute("teaching", agent_skills, subject="coding")
        assert result.xp_earned == 90
        assert result.xp_breakdown["use"] == 10
        assert result.xp_breakdown["success"] == 30
        assert result.xp_breakdown["teaching"] == 50

    def test_xp_teaching_without_success(self):
        """Teaching that fails still earns USE + TEACHING (10 + 50 = 60)."""
        registry = _make_registry()
        executor = SkillExecutor(registry)
        agent_skills = {"teaching": Skill(name="teaching", level=1)}

        registry.upgrade(
            SkillDefinition(
                name="teaching",
                description=TEACHING_SKILL.description,
                max_level=TEACHING_SKILL.max_level,
                execute_fn=lambda skills, **kw: {"success": False},
                category=TEACHING_SKILL.category,
            )
        )

        result = executor.execute("teaching", agent_skills)
        assert result.xp_earned == 60  # 10 use + 50 teaching
        assert result.xp_breakdown["teaching"] == 50


class TestSkillExecutorExecution:
    def test_execute_coding(self):
        registry = _make_registry()
        executor = SkillExecutor(registry)
        agent_skills = {"coding": Skill(name="coding", level=3)}

        result = executor.execute("coding", agent_skills, task="build API", language="rust")
        assert result.skill_name == "coding"
        assert result.output["success"] is True
        assert result.output["language"] == "rust"

    def test_execute_trading(self):
        registry = _make_registry()
        executor = SkillExecutor(registry)
        agent_skills = {"trading": Skill(name="trading", level=5)}

        result = executor.execute("trading", agent_skills, action="buy", item="ore")
        assert result.skill_name == "trading"
        assert result.output["success"] is True

    def test_execute_unregistered_raises(self):
        registry = SkillRegistry()
        executor = SkillExecutor(registry)
        with pytest.raises(KeyError):
            executor.execute("nonexistent", {})

    def test_execute_no_execute_fn_raises(self):
        registry = SkillRegistry()
        registry.register(SkillDefinition(name="empty"))
        executor = SkillExecutor(registry)
        with pytest.raises(ValueError, match="no execute function"):
            executor.execute("empty", {})


class TestSkillExecutorLevelUp:
    def test_xp_accumulates(self):
        """XP is correctly applied to the agent's skill instance."""
        registry = _make_registry()
        executor = SkillExecutor(registry)
        agent_skills = {"coding": Skill(name="coding", level=1, next_level_exp=100)}

        executor.execute("coding", agent_skills, task="test")
        assert agent_skills["coding"].experience == 40  # 10 use + 30 success

    def test_level_up_on_threshold(self):
        """Skill levels up when XP exceeds next_level_exp."""
        registry = _make_registry()
        executor = SkillExecutor(registry)
        # Set next_level_exp low so 40 XP triggers level-up
        agent_skills = {"coding": Skill(name="coding", level=1, next_level_exp=30)}

        result = executor.execute("coding", agent_skills, task="test")
        assert result.leveled_up is True
        assert agent_skills["coding"].level == 2

    def test_no_level_up_below_threshold(self):
        registry = _make_registry()
        executor = SkillExecutor(registry)
        agent_skills = {"coding": Skill(name="coding", level=1, next_level_exp=100)}

        result = executor.execute("coding", agent_skills, task="test")
        assert result.leveled_up is False
        assert agent_skills["coding"].level == 1

    def test_no_skill_instance_still_works(self):
        """Execution works even if agent doesn't have the Skill instance yet.

        Without a skill instance, the execute_fn gets level=0 and returns
        success=False, so only USE XP (10) is awarded. No crash, no level-up.
        """
        registry = _make_registry()
        executor = SkillExecutor(registry)
        # No "coding" key in agent_skills
        result = executor.execute("coding", {})
        assert result.xp_earned == 10  # USE only (no skill instance → level 0 → success=False)
        assert result.leveled_up is False

    def test_multiple_executions_accumulate(self):
        """Multiple executions correctly accumulate XP."""
        registry = _make_registry()
        executor = SkillExecutor(registry)
        agent_skills = {"coding": Skill(name="coding", level=1, next_level_exp=100)}

        # 3 executions * 40 XP = 120 XP → 100 for level 2, 20 overflow
        for _ in range(3):
            executor.execute("coding", agent_skills, task="test")

        assert agent_skills["coding"].level == 2
        assert agent_skills["coding"].experience == 20


class TestSkillExecutorCalculateXP:
    def test_use_only(self):
        executor = SkillExecutor()
        assert executor.calculate_xp("coding", success=False) == 10

    def test_use_plus_success(self):
        executor = SkillExecutor()
        assert executor.calculate_xp("coding", success=True) == 40

    def test_teaching_use_only(self):
        executor = SkillExecutor()
        assert executor.calculate_xp("teaching", success=False) == 60  # 10 + 50

    def test_teaching_all(self):
        executor = SkillExecutor()
        assert executor.calculate_xp("teaching", success=True) == 90  # 10 + 30 + 50


class TestXPRewardConstants:
    def test_values(self):
        assert XPReward.USE.value == 10
        assert XPReward.SUCCESS.value == 30
        assert XPReward.TEACHING.value == 50


# ============================================================
# SkillExecutionResult tests
# ============================================================


class TestSkillExecutionResult:
    def test_defaults(self):
        result = SkillExecutionResult(skill_name="test", output={"ok": True})
        assert result.skill_name == "test"
        assert result.output == {"ok": True}
        assert result.xp_earned == 0
        assert result.xp_breakdown == {}
        assert result.leveled_up is False


# ============================================================
# Integration tests
# ============================================================


class TestIntegration:
    def test_full_workflow(self):
        """Test the complete workflow: register, create, execute, accumulate XP."""
        registry = _make_registry()
        executor = SkillExecutor(registry)

        # Create skill instances for an agent
        agent_skills = {
            "coding": registry.create_skill("coding"),
            "trading": registry.create_skill("trading"),
            "research": registry.create_skill("research"),
            "teaching": registry.create_skill("teaching"),
        }

        # Execute coding skill
        result = executor.execute("coding", agent_skills, task="write function")
        assert result.output["success"] is True
        assert agent_skills["coding"].experience > 0

        # Execute teaching skill — should get bonus XP
        result = executor.execute("teaching", agent_skills, subject="coding")
        assert result.xp_earned == 90
        assert agent_skills["teaching"].experience == 90

    def test_custom_skill_registration(self):
        """Register a custom skill and execute it."""
        registry = SkillRegistry()

        def execute_mining(skills, **kwargs):
            skill = skills.get("mining")
            level = skill.level if skill else 0
            return {
                "ore_found": level >= 1,
                "success": level >= 1,
                "level_used": level,
            }

        registry.register(
            SkillDefinition(
                name="mining",
                description="Extract resources from the environment",
                max_level=20,
                execute_fn=execute_mining,
                category="gathering",
            )
        )

        executor = SkillExecutor(registry)
        agent_skills = {"mining": Skill(name="mining", level=5, max_level=20)}
        result = executor.execute("mining", agent_skills)

        assert result.skill_name == "mining"
        assert result.output["ore_found"] is True
        assert result.xp_earned == 40  # use + success

    def test_upgrade_skill_preserves_data(self):
        """Upgrading a skill definition doesn't affect existing agent skills."""
        registry = SkillRegistry()
        registry.register(
            SkillDefinition(
                name="crafting",
                description="v1",
                max_level=10,
            )
        )

        agent_skills = {"crafting": registry.create_skill("crafting", level=5)}
        assert agent_skills["crafting"].level == 5

        # Upgrade the definition
        registry.upgrade(
            SkillDefinition(
                name="crafting",
                description="v2",
                max_level=15,
            )
        )

        # Existing skill instance is unchanged
        assert agent_skills["crafting"].level == 5
        assert agent_skills["crafting"].max_level == 10

        # New skill created after upgrade uses new max_level
        new_skill = registry.create_skill("crafting")
        assert new_skill.max_level == 15

    def test_multiple_xp_types_stack(self):
        """Teaching success gives the maximum XP: USE + SUCCESS + TEACHING."""
        registry = _make_registry()
        executor = SkillExecutor(registry)
        agent_skills = {"teaching": Skill(name="teaching", level=1)}

        result = executor.execute("teaching", agent_skills, subject="coding")
        assert result.xp_breakdown == {"use": 10, "success": 30, "teaching": 50}
        assert result.xp_earned == 90
        assert agent_skills["teaching"].experience == 90

    def test_all_builtin_skills_execute(self):
        """Verify all 4 built-in skills can execute successfully."""
        registry = _make_registry()
        executor = SkillExecutor(registry)

        for skill_def in BUILTIN_SKILLS:
            agent_skills = {skill_def.name: Skill(name=skill_def.name, level=3)}
            result = executor.execute(skill_def.name, agent_skills)
            assert result.output["success"] is True, f"{skill_def.name} did not succeed"
            assert result.xp_earned >= XPReward.USE.value, f"{skill_def.name} got no XP"
