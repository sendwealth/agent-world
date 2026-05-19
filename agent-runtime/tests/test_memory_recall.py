"""Tests for memory recall integration (agent_runtime.memory.memory_recall).

Covers:
- MemoryRecall creation and defaults
- recall_for_decision with various memory types
- build_context formatting
- Configuration options
- Error handling for failing vector memory
"""

from __future__ import annotations

from agent_runtime.memory.memory_recall import (
    MemoryRecall,
    MemoryRecallConfig,
    RecalledMemory,
)
from agent_runtime.memory.vector_memory import VectorMemory

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def make_recall_with_memory(**kwargs):
    """Create a MemoryRecall with a real VectorMemory backend."""
    vm = VectorMemory(db_path=":memory:")
    config = MemoryRecallConfig(**kwargs) if kwargs else None
    recall = MemoryRecall(vector_memory=vm, config=config)
    return recall, vm


# ---------------------------------------------------------------------------
# RecalledMemory
# ---------------------------------------------------------------------------


class TestRecalledMemory:
    def test_str(self):
        m = RecalledMemory(
            content="test", memory_type="lesson", relevance=0.8, importance=0.7
        )
        s = str(m)
        assert "lesson" in s
        assert "test" in s
        assert "0.80" in s


# ---------------------------------------------------------------------------
# MemoryRecallConfig
# ---------------------------------------------------------------------------


class TestMemoryRecallConfig:
    def test_defaults(self):
        config = MemoryRecallConfig()
        assert config.max_memories == 5
        assert config.min_relevance == 0.3
        assert config.include_experiences is True
        assert config.include_lessons is True
        assert config.include_facts is True

    def test_custom(self):
        config = MemoryRecallConfig(
            max_memories=10, min_relevance=0.5, include_facts=False,
        )
        assert config.max_memories == 10
        assert config.include_facts is False


# ---------------------------------------------------------------------------
# MemoryRecall — recall_for_decision
# ---------------------------------------------------------------------------


class TestMemoryRecallForDecision:
    def test_recall_from_empty_memory(self):
        recall, vm = make_recall_with_memory()
        results = recall.recall_for_decision("trading")
        assert results == []

    def test_recall_finds_relevant_memories(self):
        recall, vm = make_recall_with_memory()
        vm.store("Trading during crisis is risky", memory_type="lesson", importance=0.9)
        vm.store("The forest has berries", memory_type="experience", importance=0.5)

        results = recall.recall_for_decision("trading risk")
        assert len(results) >= 1
        # Should find the trading-related memory
        contents = [m.content for m in results]
        assert any("trading" in c.lower() for c in contents)

    def test_recall_returns_recalled_memory_objects(self):
        recall, vm = make_recall_with_memory()
        vm.store("test memory", memory_type="experience", importance=0.8)

        results = recall.recall_for_decision("test")
        for m in results:
            assert isinstance(m, RecalledMemory)
            assert m.content
            assert m.memory_type in ("experience", "lesson", "fact")

    def test_recall_type_filtering(self):
        config = MemoryRecallConfig(
            include_experiences=False, include_lessons=True, include_facts=False,
        )
        recall, vm = make_recall_with_memory()
        recall._config = config

        vm.store("experience memory", memory_type="experience", importance=0.9)
        vm.store("lesson memory", memory_type="lesson", importance=0.9)

        results = recall.recall_for_decision("memory")
        for m in results:
            assert m.memory_type == "lesson"

    def test_recall_respects_max_memories(self):
        config = MemoryRecallConfig(max_memories=2)
        recall, vm = make_recall_with_memory()
        recall._config = config

        for i in range(10):
            vm.store(f"memory item {i}", importance=0.7)

        results = recall.recall_for_decision("memory")
        assert len(results) <= 2

    def test_recall_with_situation_appended(self):
        recall, vm = make_recall_with_memory()
        vm.store("low tokens survival lesson", memory_type="lesson", importance=0.9)

        results = recall.recall_for_decision(
            "survival", situation="low on tokens"
        )
        assert len(results) >= 1


# ---------------------------------------------------------------------------
# MemoryRecall — build_context
# ---------------------------------------------------------------------------


class TestMemoryRecallBuildContext:
    def test_empty_context(self):
        recall, vm = make_recall_with_memory()
        context = recall.build_context("anything")
        assert context == ""

    def test_context_formatting(self):
        recall, vm = make_recall_with_memory()
        vm.store("Trading is risky", memory_type="lesson", importance=0.9)

        context = recall.build_context("trading risk")
        assert "Relevant Past Memories" in context
        assert "lesson" in context
        assert "Trading is risky" in context
        assert "relevance" in context.lower()

    def test_context_multiple_memories(self):
        recall, vm = make_recall_with_memory(min_relevance=0.0)
        vm.store("lesson one about memory", memory_type="lesson", importance=0.8)
        vm.store("fact two about memory", memory_type="fact", importance=0.7)

        context = recall.build_context("memory lesson fact")
        # At least one memory should be found
        assert context != ""
        assert "Relevant Past Memories" in context


# ---------------------------------------------------------------------------
# MemoryRecall — error handling
# ---------------------------------------------------------------------------


class TestMemoryRecallErrorHandling:
    def test_handles_failing_vector_memory(self):
        """Should not crash if vector memory raises an exception."""

        class FailingVectorMemory:
            def recall_with_decay(self, query, *, top_k=5, memory_type=None):
                raise RuntimeError("Vector memory is down")

        recall = MemoryRecall(vector_memory=FailingVectorMemory())
        # Should return empty list, not raise
        results = recall.recall_for_decision("test")
        assert results == []

    def test_build_context_handles_failure(self):
        class FailingVectorMemory:
            def recall_with_decay(self, query, *, top_k=5, memory_type=None):
                raise RuntimeError("down")

        recall = MemoryRecall(vector_memory=FailingVectorMemory())
        context = recall.build_context("test")
        assert context == ""
