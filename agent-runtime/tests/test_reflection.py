"""Comprehensive tests for the reflection layer.

Covers:
- ActionTypeStats computation
- ReflectionConfig defaults and customization
- ReflectionLayer.should_reflect() tick logic
- ReflectionLayer.reflect() full cycle
- StrategyRegistry update from reflection
- LongTermMemory store/query lifecycle
- Integration: reflection → strategy → memory pipeline
- Edge cases: empty history, single action, all failures, all successes
"""

from __future__ import annotations

import asyncio
import tempfile
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import pytest

from agent_runtime.reflection import (
    ActionStatus,
    ActionTypeStats,
    LongTermMemory,
    MemoryEntry,
    ReflectionConfig,
    ReflectionLayer,
    ReflectionResult,
    StrategyPreference,
    StrategyRegistry,
)
from agent_runtime.reflection.memory import MemoryCategory


# ---------------------------------------------------------------------------
# Fixtures and fakes
# ---------------------------------------------------------------------------

@dataclass
class FakeActionOutcome:
    """Minimal action outcome for testing."""

    action_type: str
    status: str
    token_cost: int = 5


@pytest.fixture
def memory() -> LongTermMemory:
    """Create an in-memory LongTermMemory."""
    return LongTermMemory(":memory:")


@pytest.fixture
def strategy() -> StrategyRegistry:
    """Create a StrategyRegistry without disk persistence."""
    return StrategyRegistry()


@pytest.fixture
def config() -> ReflectionConfig:
    """Create a ReflectionConfig with interval=10."""
    return ReflectionConfig(interval=10)


@pytest.fixture
def layer(strategy: StrategyRegistry, memory: LongTermMemory, config: ReflectionConfig) -> ReflectionLayer:
    """Create a ReflectionLayer with default config."""
    return ReflectionLayer(strategy, memory, config=config)


def make_actions(
    *specs: tuple[str, str, int],
) -> list[FakeActionOutcome]:
    """Create a list of FakeActionOutcome from (action_type, status, token_cost) tuples."""
    return [FakeActionOutcome(at, st, cost) for at, st, cost in specs]


# ===========================================================================
# ActionTypeStats tests
# ===========================================================================

class TestActionTypeStats:
    def test_empty_stats(self) -> None:
        stats = ActionTypeStats(action_type="test")
        assert stats.total == 0
        assert stats.success_rate == 0.0
        assert stats.token_efficiency == 0.0

    def test_success_rate_calculation(self) -> None:
        stats = ActionTypeStats(action_type="send_message", total=10, successes=7, failures=3)
        assert stats.success_rate == pytest.approx(0.7)

    def test_success_rate_zero_total(self) -> None:
        stats = ActionTypeStats(action_type="rest", total=0, successes=0, failures=0)
        assert stats.success_rate == 0.0

    def test_token_efficiency(self) -> None:
        stats = ActionTypeStats(
            action_type="claim_task", total=5, successes=5,
            tokens_spent=25, rewards=50.0,
        )
        assert stats.token_efficiency == pytest.approx(2.0)

    def test_token_efficiency_zero_tokens(self) -> None:
        stats = ActionTypeStats(action_type="rest", total=3, successes=3, tokens_spent=0)
        assert stats.token_efficiency == 0.0


# ===========================================================================
# ReflectionConfig tests
# ===========================================================================

class TestReflectionConfig:
    def test_defaults(self) -> None:
        config = ReflectionConfig()
        assert config.interval == 10
        assert config.min_actions_for_reflection == 1
        assert config.importance_threshold == 0.6
        assert config.decay_factor == 0.95

    def test_custom_interval(self) -> None:
        config = ReflectionConfig(interval=5)
        assert config.interval == 5


# ===========================================================================
# ReflectionResult tests
# ===========================================================================

class TestReflectionResult:
    def test_auto_timestamp(self) -> None:
        result = ReflectionResult(
            tick=10,
            total_actions_evaluated=5,
            overall_success_rate=0.8,
            overall_token_efficiency=0.5,
            action_stats=[],
            strategy_changes=[],
            memories_stored=0,
            top_actions=[],
        )
        assert result.reflected_at > 0.0

    def test_explicit_timestamp(self) -> None:
        result = ReflectionResult(
            tick=10,
            total_actions_evaluated=5,
            overall_success_rate=0.8,
            overall_token_efficiency=0.5,
            action_stats=[],
            strategy_changes=[],
            memories_stored=0,
            top_actions=[],
            reflected_at=12345.0,
        )
        assert result.reflected_at == 12345.0


# ===========================================================================
# ReflectionLayer.should_reflect tests
# ===========================================================================

class TestShouldReflect:
    def test_first_reflection_at_interval(self, layer: ReflectionLayer) -> None:
        assert layer.should_reflect(10) is True

    def test_before_interval(self, layer: ReflectionLayer) -> None:
        assert layer.should_reflect(9) is False
        assert layer.should_reflect(5) is False
        assert layer.should_reflect(0) is False

    def test_after_reflection_resets(self, layer: ReflectionLayer) -> None:
        actions = make_actions(("rest", "success", 0))
        layer.reflect(10, actions)
        assert layer.should_reflect(15) is False
        assert layer.should_reflect(19) is False
        assert layer.should_reflect(20) is True

    def test_disabled_interval(self, strategy: StrategyRegistry, memory: LongTermMemory) -> None:
        config = ReflectionConfig(interval=0)
        layer = ReflectionLayer(strategy, memory, config=config)
        assert layer.should_reflect(10) is False
        assert layer.should_reflect(100) is False

    def test_negative_interval(self, strategy: StrategyRegistry, memory: LongTermMemory) -> None:
        config = ReflectionConfig(interval=-1)
        layer = ReflectionLayer(strategy, memory, config=config)
        assert layer.should_reflect(10) is False


# ===========================================================================
# ReflectionLayer.reflect tests
# ===========================================================================

class TestReflect:
    def test_basic_reflection(self, layer: ReflectionLayer) -> None:
        actions = make_actions(
            ("send_message", "success", 10),
            ("claim_task", "success", 5),
            ("explore", "failed", 3),
        )
        result = layer.reflect(10, actions)

        assert result is not None
        assert result.tick == 10
        assert result.total_actions_evaluated == 3
        assert result.overall_success_rate == pytest.approx(2 / 3)
        assert len(result.action_stats) == 3  # 3 distinct action types

    def test_skip_when_not_time(self, layer: ReflectionLayer) -> None:
        actions = make_actions(("rest", "success", 0))
        result = layer.reflect(5, actions)
        assert result is None

    def test_skip_when_no_actions(self, layer: ReflectionLayer) -> None:
        result = layer.reflect(10, [])
        assert result is None

    def test_min_actions_threshold(
        self, strategy: StrategyRegistry, memory: LongTermMemory,
    ) -> None:
        config = ReflectionConfig(interval=10, min_actions_for_reflection=3)
        layer = ReflectionLayer(strategy, memory, config=config)

        # Only 2 actions — below threshold
        actions = make_actions(("rest", "success", 0), ("explore", "success", 3))
        result = layer.reflect(10, actions)
        assert result is None

        # 3 actions — at threshold
        actions.append(FakeActionOutcome("claim_task", "success", 5))
        result = layer.reflect(10, actions)
        assert result is not None

    def test_all_failures(self, layer: ReflectionLayer) -> None:
        actions = make_actions(
            ("send_message", "failed", 10),
            ("send_message", "failed", 10),
            ("send_message", "failed", 10),
        )
        result = layer.reflect(10, actions)

        assert result is not None
        assert result.overall_success_rate == 0.0
        assert len(result.action_stats) == 1
        assert result.action_stats[0].success_rate == 0.0

    def test_all_successes(self, layer: ReflectionLayer) -> None:
        actions = make_actions(
            ("rest", "success", 0),
            ("rest", "success", 0),
            ("rest", "success", 0),
        )
        result = layer.reflect(10, actions)

        assert result is not None
        assert result.overall_success_rate == 1.0

    def test_reflection_count_increments(self, layer: ReflectionLayer) -> None:
        actions = make_actions(("rest", "success", 0))

        assert layer.reflection_count == 0
        layer.reflect(10, actions)
        assert layer.reflection_count == 1
        layer.reflect(20, actions)
        assert layer.reflection_count == 2

    def test_last_reflection_tick_updates(self, layer: ReflectionLayer) -> None:
        actions = make_actions(("rest", "success", 0))

        assert layer.last_reflection_tick == 0
        layer.reflect(10, actions)
        assert layer.last_reflection_tick == 10
        layer.reflect(20, actions)
        assert layer.last_reflection_tick == 20

    def test_multiple_reflections_in_sequence(self, layer: ReflectionLayer) -> None:
        actions = make_actions(("rest", "success", 0))

        for tick in [10, 20, 30, 40, 50]:
            result = layer.reflect(tick, actions)
            assert result is not None
            assert result.tick == tick

        assert layer.reflection_count == 5

    def test_async_reflect(self, layer: ReflectionLayer) -> None:
        actions = make_actions(("rest", "success", 0))
        result = asyncio.run(layer.reflect_async(10, actions))
        assert result is not None
        assert result.tick == 10


# ===========================================================================
# StrategyPreference tests
# ===========================================================================

class TestStrategyPreference:
    def test_defaults(self) -> None:
        pref = StrategyPreference(action_type="test")
        assert pref.weight == 1.0
        assert pref.success_count == 0
        assert pref.failure_count == 0
        assert pref.success_rate == 0.5  # neutral prior
        assert pref.token_efficiency == 0.0

    def test_record_success(self) -> None:
        pref = StrategyPreference(action_type="test")
        pref.record_success(10, 5.0)
        assert pref.success_count == 1
        assert pref.total_tokens_spent == 10
        assert pref.total_rewards == 5.0
        assert pref.success_rate == 1.0

    def test_record_failure(self) -> None:
        pref = StrategyPreference(action_type="test")
        pref.record_failure(10)
        assert pref.failure_count == 1
        assert pref.total_tokens_spent == 10
        assert pref.success_rate == 0.0

    def test_adjusted_weight_bounds(self) -> None:
        pref = StrategyPreference(action_type="test", weight=0.1)
        assert pref.adjusted_weight >= 0.2  # clamped lower bound

        pref2 = StrategyPreference(action_type="test2", weight=10.0)
        assert pref2.adjusted_weight <= 2.0  # clamped upper bound

    def test_decay(self) -> None:
        pref = StrategyPreference(action_type="test", weight=2.0)
        pref.decay(0.9)
        assert pref.weight == pytest.approx(1.8)

    def test_token_efficiency_with_spending(self) -> None:
        pref = StrategyPreference(action_type="test")
        pref.record_success(10, 20.0)
        assert pref.token_efficiency == pytest.approx(2.0)


# ===========================================================================
# StrategyRegistry tests
# ===========================================================================

class TestStrategyRegistry:
    def test_get_creates_default(self, strategy: StrategyRegistry) -> None:
        pref = strategy.get("send_message")
        assert pref.action_type == "send_message"
        assert pref.weight == 1.0

    def test_get_returns_same_instance(self, strategy: StrategyRegistry) -> None:
        pref1 = strategy.get("test")
        pref2 = strategy.get("test")
        assert pref1 is pref2

    def test_update_from_reflection_success(self, strategy: StrategyRegistry) -> None:
        pref = strategy.update_from_reflection("claim_task", success=True, tokens_spent=5, reward=1.0)
        assert pref.success_count == 1
        assert pref.weight > 1.0  # weight should increase on success

    def test_update_from_reflection_failure(self, strategy: StrategyRegistry) -> None:
        pref = strategy.update_from_reflection("explore", success=False, tokens_spent=3)
        assert pref.failure_count == 1
        assert pref.weight < 1.0  # weight should decrease on failure

    def test_global_decay(self, strategy: StrategyRegistry) -> None:
        strategy.get("a").weight = 1.0
        strategy.get("b").weight = 2.0
        strategy.apply_global_decay()
        assert strategy.get("a").weight == pytest.approx(0.95)
        assert strategy.get("b").weight == pytest.approx(1.9)

    def test_top_actions(self, strategy: StrategyRegistry) -> None:
        # Boost "claim_task" above "explore"
        for _ in range(5):
            strategy.update_from_reflection("claim_task", success=True, tokens_spent=5, reward=1.0)
        for _ in range(5):
            strategy.update_from_reflection("explore", success=False, tokens_spent=3)

        top = strategy.top_actions(2)
        assert len(top) == 2
        assert top[0][0] == "claim_task"  # highest weight

    def test_summary(self, strategy: StrategyRegistry) -> None:
        strategy.update_from_reflection("rest", success=True, tokens_spent=0, reward=1.0)
        summary = strategy.summary()
        assert "rest" in summary
        assert summary["rest"]["success_count"] == 1

    def test_save_and_load(self, strategy: StrategyRegistry) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            path = Path(tmpdir) / "strategy.json"
            strategy.update_from_reflection("test_action", success=True, tokens_spent=5, reward=1.0)
            strategy.save(path)

            # Load into new registry
            loaded = StrategyRegistry(storage_path=path)
            pref = loaded.get("test_action")
            assert pref.success_count == 1
            assert pref.total_tokens_spent == 5

    def test_load_missing_file(self) -> None:
        registry = StrategyRegistry(storage_path=Path("/nonexistent/path.json"))
        assert len(registry.all_preferences()) == 0


# ===========================================================================
# LongTermMemory tests
# ===========================================================================

class TestLongTermMemory:
    def test_store_and_query(self, memory: LongTermMemory) -> None:
        entry = MemoryEntry(
            category=MemoryCategory.REFLECTION,
            content="Test reflection",
            tick=10,
        )
        entry_id = memory.store(entry)
        assert entry_id > 0

        results = memory.query()
        assert len(results) == 1
        assert results[0].content == "Test reflection"
        assert results[0].id == entry_id

    def test_query_by_category(self, memory: LongTermMemory) -> None:
        memory.store(MemoryEntry(category=MemoryCategory.DECISION, content="d1", tick=5))
        memory.store(MemoryEntry(category=MemoryCategory.LESSON, content="l1", tick=10))
        memory.store(MemoryEntry(category=MemoryCategory.REFLECTION, content="r1", tick=15))

        decisions = memory.query(category=MemoryCategory.DECISION)
        assert len(decisions) == 1
        assert decisions[0].content == "d1"

    def test_query_by_tick(self, memory: LongTermMemory) -> None:
        memory.store(MemoryEntry(category=MemoryCategory.LESSON, content="old", tick=5))
        memory.store(MemoryEntry(category=MemoryCategory.LESSON, content="new", tick=20))

        recent = memory.query(since_tick=10)
        assert len(recent) == 1
        assert recent[0].content == "new"

    def test_query_by_importance(self, memory: LongTermMemory) -> None:
        memory.store(MemoryEntry(category=MemoryCategory.DECISION, content="low", tick=1, importance=0.3))
        memory.store(MemoryEntry(category=MemoryCategory.DECISION, content="high", tick=2, importance=0.9))

        important = memory.query(min_importance=0.7)
        assert len(important) == 1
        assert important[0].content == "high"

    def test_get_recent(self, memory: LongTermMemory) -> None:
        for i in range(5):
            memory.store(MemoryEntry(category=MemoryCategory.LESSON, content=f"lesson_{i}", tick=i))

        recent = memory.get_recent(limit=3)
        assert len(recent) == 3
        # Most recent first
        assert recent[0].content == "lesson_4"

    def test_get_important(self, memory: LongTermMemory) -> None:
        memory.store(MemoryEntry(category=MemoryCategory.DECISION, content="critical", tick=1, importance=0.95))
        memory.store(MemoryEntry(category=MemoryCategory.DECISION, content="minor", tick=2, importance=0.4))

        important = memory.get_important(limit=5, min_importance=0.7)
        assert len(important) == 1
        assert important[0].content == "critical"

    def test_count(self, memory: LongTermMemory) -> None:
        memory.store(MemoryEntry(category=MemoryCategory.DECISION, content="d1", tick=1))
        memory.store(MemoryEntry(category=MemoryCategory.DECISION, content="d2", tick=2))
        memory.store(MemoryEntry(category=MemoryCategory.LESSON, content="l1", tick=3))

        assert memory.count() == 3
        assert memory.count(category=MemoryCategory.DECISION) == 2
        assert memory.count(category=MemoryCategory.LESSON) == 1

    def test_delete_before_tick(self, memory: LongTermMemory) -> None:
        memory.store(MemoryEntry(category=MemoryCategory.DECISION, content="old", tick=5))
        memory.store(MemoryEntry(category=MemoryCategory.DECISION, content="keep", tick=15))

        deleted = memory.delete_before_tick(10)
        assert deleted == 1
        assert memory.count() == 1

    def test_store_batch(self, memory: LongTermMemory) -> None:
        entries = [
            MemoryEntry(category=MemoryCategory.LESSON, content=f"lesson_{i}", tick=i)
            for i in range(5)
        ]
        ids = memory.store_batch(entries)
        assert len(ids) == 5
        assert all(id_ > 0 for id_ in ids)

    def test_importance_clamping(self) -> None:
        entry = MemoryEntry(
            category=MemoryCategory.LESSON, content="test", tick=1, importance=2.0,
        )
        assert entry.importance == 1.0

        entry2 = MemoryEntry(
            category=MemoryCategory.LESSON, content="test", tick=1, importance=-1.0,
        )
        assert entry2.importance == 0.0

    def test_metadata_round_trip(self, memory: LongTermMemory) -> None:
        entry = MemoryEntry(
            category=MemoryCategory.STRATEGY_CHANGE,
            content="Adjusted weights",
            tick=10,
            importance=0.8,
            metadata={"action": "claim_task", "old_weight": 1.0, "new_weight": 1.5},
        )
        memory.store(entry)

        results = memory.query(category=MemoryCategory.STRATEGY_CHANGE)
        assert len(results) == 1
        assert results[0].metadata["action"] == "claim_task"
        assert results[0].metadata["new_weight"] == 1.5

    def test_persistence_to_file(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            db_path = Path(tmpdir) / "test.db"
            mem1 = LongTermMemory(db_path)
            mem1.store(MemoryEntry(category=MemoryCategory.LESSON, content="persist me", tick=1))
            mem1.close()

            mem2 = LongTermMemory(db_path)
            results = mem2.query()
            assert len(results) == 1
            assert results[0].content == "persist me"
            mem2.close()


# ===========================================================================
# Integration: Reflection → Strategy → Memory pipeline
# ===========================================================================

class TestIntegration:
    def test_full_reflection_pipeline(
        self, layer: ReflectionLayer, strategy: StrategyRegistry, memory: LongTermMemory,
    ) -> None:
        """Test the complete flow: actions → reflect → strategy update → memory write."""
        actions = make_actions(
            ("claim_task", "success", 5),
            ("claim_task", "success", 5),
            ("claim_task", "failed", 5),
            ("explore", "success", 3),
            ("send_message", "failed", 10),
        )

        result = layer.reflect(10, actions)

        assert result is not None
        assert result.total_actions_evaluated == 5

        # Check strategy was updated
        claim_pref = strategy.get("claim_task")
        assert claim_pref.success_count >= 2

        # Check memory was written
        assert memory.count() > 0
        reflections = memory.query(category=MemoryCategory.REFLECTION)
        assert len(reflections) >= 1

    def test_multiple_reflection_cycles(
        self, layer: ReflectionLayer, strategy: StrategyRegistry, memory: LongTermMemory,
    ) -> None:
        """Test that multiple reflection cycles accumulate correctly."""
        for tick in [10, 20, 30]:
            actions = make_actions(
                ("rest", "success", 0),
                ("explore", "success", 3),
            )
            result = layer.reflect(tick, actions)
            assert result is not None

        assert layer.reflection_count == 3
        assert memory.count(category=MemoryCategory.REFLECTION) == 3

        # Strategy should have accumulated data
        explore_pref = strategy.get("explore")
        assert explore_pref.success_count >= 3

    def test_strategy_decay_over_time(
        self, strategy: StrategyRegistry, memory: LongTermMemory,
    ) -> None:
        """Test that strategy preferences decay across reflections."""
        config = ReflectionConfig(interval=10, decay_factor=0.9)
        layer = ReflectionLayer(strategy, memory, config=config)

        # First reflection: boost
        actions = make_actions(("claim_task", "success", 5))
        layer.reflect(10, actions)
        weight_after_first = strategy.get("claim_task").weight

        # Second reflection with no actions for claim_task — decay
        layer.reflect(20, make_actions(("explore", "success", 3)))
        weight_after_second = strategy.get("claim_task").weight

        # Weight should have decayed
        assert weight_after_second < weight_after_first

    def test_low_success_rate_triggers_lesson(
        self, strategy: StrategyRegistry, memory: LongTermMemory,
    ) -> None:
        """Test that action types with low success rates generate lessons."""
        config = ReflectionConfig(interval=10, importance_threshold=0.0)
        layer = ReflectionLayer(strategy, memory, config=config)

        # 5 actions, 1 success (20% success rate — below 30% threshold)
        actions = make_actions(
            ("send_message", "success", 10),
            ("send_message", "failed", 10),
            ("send_message", "failed", 10),
            ("send_message", "failed", 10),
            ("send_message", "failed", 10),
        )
        layer.reflect(10, actions)

        lessons = memory.query(category=MemoryCategory.LESSON)
        assert any("send_message" in l.content and "low success rate" in l.content for l in lessons)

    def test_strategy_change_recorded(
        self, strategy: StrategyRegistry, memory: LongTermMemory,
    ) -> None:
        """Test that significant strategy changes are recorded as memories."""
        config = ReflectionConfig(interval=10, importance_threshold=0.0)
        layer = ReflectionLayer(strategy, memory, config=config)

        # Create a big weight shift
        actions = make_actions(
            ("explore", "success", 3),
            ("explore", "success", 3),
            ("explore", "success", 3),
            ("explore", "success", 3),
            ("explore", "success", 3),
        )
        layer.reflect(10, actions)

        changes = memory.query(category=MemoryCategory.STRATEGY_CHANGE)
        assert len(changes) > 0

    def test_reflect_with_mixed_action_types(
        self, layer: ReflectionLayer, strategy: StrategyRegistry,
    ) -> None:
        """Test reflection with many different action types."""
        actions = make_actions(
            ("send_message", "success", 10),
            ("claim_task", "success", 5),
            ("submit_task", "failed", 8),
            ("propose_deal", "success", 10),
            ("teach_skill", "failed", 15),
            ("rest", "success", 0),
            ("explore", "success", 3),
        )
        result = layer.reflect(10, actions)

        assert result is not None
        assert len(result.action_stats) == 7
        assert result.overall_success_rate == pytest.approx(5 / 7)

    def test_top_actions_populated(
        self, layer: ReflectionLayer,
    ) -> None:
        """Test that reflection result includes top actions."""
        actions = make_actions(
            ("claim_task", "success", 5),
            ("claim_task", "success", 5),
            ("explore", "failed", 3),
        )
        result = layer.reflect(10, actions)

        assert result is not None
        assert len(result.top_actions) > 0
        # claim_task should be ranked higher than explore
        action_names = [a[0] for a in result.top_actions]
        if "explore" in action_names and "claim_task" in action_names:
            assert action_names.index("claim_task") < action_names.index("explore")

    def test_window_size_limiting(
        self, strategy: StrategyRegistry, memory: LongTermMemory,
    ) -> None:
        """Test that window_size limits which actions are evaluated."""
        config = ReflectionConfig(interval=10, window_size=3)
        layer = ReflectionLayer(strategy, memory, config=config)

        # 10 actions, but window_size=3 should only evaluate last 3
        actions = make_actions(
            ("rest", "success", 0),  # skipped
            ("rest", "success", 0),  # skipped
            ("rest", "success", 0),  # skipped
            ("explore", "failed", 3),  # evaluated
            ("explore", "failed", 3),  # evaluated
            ("explore", "success", 3),  # evaluated
        )
        result = layer.reflect(10, actions)

        assert result is not None
        # Only 3 actions evaluated (the last 3 explore)
        assert result.total_actions_evaluated == 3

    def test_importance_threshold_filters_memories(
        self, strategy: StrategyRegistry, memory: LongTermMemory,
    ) -> None:
        """Test that only memories above the importance threshold are stored."""
        config = ReflectionConfig(interval=10, importance_threshold=0.9)
        layer = ReflectionLayer(strategy, memory, config=config)

        actions = make_actions(("rest", "success", 0))
        layer.reflect(10, actions)

        # With threshold 0.9, most memories should be filtered out
        # The reflection summary has importance 0.4 + rate * 0.4, so for 100% success = 0.8
        # which is below 0.9 threshold
        # Strategy changes have importance 0.7, also below 0.9
        # So we expect 0 memories stored
        count = memory.count()
        assert count == 0

    def test_max_memory_per_reflection(
        self, strategy: StrategyRegistry, memory: LongTermMemory,
    ) -> None:
        """Test that max_memory_per_reflection limits stored entries."""
        config = ReflectionConfig(
            interval=10,
            importance_threshold=0.0,  # store everything
            max_memory_per_reflection=2,
        )
        layer = ReflectionLayer(strategy, memory, config=config)

        # Create many action types that will generate multiple memory entries
        actions = make_actions(
            ("send_message", "failed", 10),
            ("send_message", "failed", 10),
            ("send_message", "failed", 10),
            ("explore", "success", 3),
            ("explore", "success", 3),
            ("explore", "success", 3),
        )
        layer.reflect(10, actions)

        # Should have at most 2 memories stored
        assert memory.count() <= 2


# ===========================================================================
# MemoryCategory tests
# ===========================================================================

class TestMemoryCategory:
    def test_all_categories(self) -> None:
        assert MemoryCategory.DECISION.value == "decision"
        assert MemoryCategory.LESSON.value == "lesson"
        assert MemoryCategory.REFLECTION.value == "reflection"
        assert MemoryCategory.STRATEGY_CHANGE.value == "strategy_change"
        assert MemoryCategory.MILESTONE.value == "milestone"

    def test_string_comparison(self) -> None:
        assert MemoryCategory.REFLECTION == "reflection"
        assert MemoryCategory.LESSON == "lesson"


# ===========================================================================
# MemoryEntry tests
# ===========================================================================

class TestMemoryEntry:
    def test_auto_created_at(self) -> None:
        entry = MemoryEntry(category=MemoryCategory.DECISION, content="test", tick=1)
        assert entry.created_at > 0.0

    def test_importance_clamped_high(self) -> None:
        entry = MemoryEntry(
            category=MemoryCategory.DECISION, content="test", tick=1, importance=5.0,
        )
        assert entry.importance == 1.0

    def test_importance_clamped_low(self) -> None:
        entry = MemoryEntry(
            category=MemoryCategory.DECISION, content="test", tick=1, importance=-1.0,
        )
        assert entry.importance == 0.0

    def test_default_metadata(self) -> None:
        entry = MemoryEntry(category=MemoryCategory.DECISION, content="test", tick=1)
        assert entry.metadata == {}
