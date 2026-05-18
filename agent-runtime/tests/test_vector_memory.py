"""Tests for vector-backed long-term memory (agent_runtime.memory.vector_memory).

Covers:
- VectorMemoryEntry creation and string representation
- VectorMemory creation and defaults
- Store entries with embeddings (experience, lesson, fact)
- Semantic retrieval by cosine similarity (top-k)
- Forgetting curve decay (30-day 50% weight)
- Memory type filtering
- Eviction when max_entries exceeded
- Delete and clear
- Token cost property
- Edge cases: empty search, exact match, etc.
"""

from __future__ import annotations

import math
from datetime import datetime, timedelta, timezone

import pytest

from agent_runtime.memory.embedding import HashEmbeddingProvider
from agent_runtime.memory.vector_memory import (
    MemoryType,
    VectorMemory,
    VectorMemoryEntry,
    _blob_to_floats,
    _floats_to_blob,
    _now_iso,
)


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def make_vector_memory(**kwargs):
    """Create a VectorMemory with in-memory DB and small max for tests."""
    defaults = {"db_path": ":memory:", "max_entries": 50}
    defaults.update(kwargs)
    return VectorMemory(**defaults)


def _cosine_sim(a: list[float], b: list[float]) -> float:
    dot = sum(x * y for x, y in zip(a, b))
    na = math.sqrt(sum(x * x for x in a))
    nb = math.sqrt(sum(x * x for x in b))
    if na == 0 or nb == 0:
        return 0.0
    return dot / (na * nb)


# ---------------------------------------------------------------------------
# VectorMemoryEntry
# ---------------------------------------------------------------------------


class TestVectorMemoryEntry:
    def test_str_representation(self):
        entry = VectorMemoryEntry(
            id=1, content="test", memory_type="experience",
            importance=0.8, access_count=3,
        )
        s = str(entry)
        assert "test" in s
        assert "experience" in s
        assert "3x" in s

    def test_frozen(self):
        entry = VectorMemoryEntry(
            id=1, content="test", memory_type="experience", importance=0.5,
        )
        with pytest.raises(AttributeError):
            entry.content = "changed"  # type: ignore[misc]

    def test_default_metadata(self):
        entry = VectorMemoryEntry(
            id=1, content="test", memory_type="experience", importance=0.5,
        )
        assert entry.metadata == {}

    def test_custom_metadata(self):
        entry = VectorMemoryEntry(
            id=1, content="test", memory_type="experience", importance=0.5,
            metadata={"key": "value"},
        )
        assert entry.metadata == {"key": "value"}


# ---------------------------------------------------------------------------
# MemoryType enum
# ---------------------------------------------------------------------------


class TestMemoryType:
    def test_values(self):
        assert MemoryType.EXPERIENCE.value == "experience"
        assert MemoryType.LESSON.value == "lesson"
        assert MemoryType.FACT.value == "fact"

    def test_from_string(self):
        assert MemoryType("experience") == MemoryType.EXPERIENCE
        assert MemoryType("lesson") == MemoryType.LESSON
        assert MemoryType("fact") == MemoryType.FACT


# ---------------------------------------------------------------------------
# Serialization helpers
# ---------------------------------------------------------------------------


class TestSerialization:
    def test_floats_to_blob_roundtrip(self):
        floats = [1.0, -2.5, 3.14, 0.0]
        blob = _floats_to_blob(floats)
        result = _blob_to_floats(blob)
        assert len(result) == 4
        assert abs(result[0] - 1.0) < 1e-5
        assert abs(result[1] - (-2.5)) < 1e-5
        assert abs(result[2] - 3.14) < 1e-5
        assert abs(result[3] - 0.0) < 1e-5

    def test_floats_to_blob_size(self):
        floats = [1.0] * 128
        blob = _floats_to_blob(floats)
        # 4 bytes per float
        assert len(blob) == 128 * 4


# ---------------------------------------------------------------------------
# VectorMemory — creation
# ---------------------------------------------------------------------------


class TestVectorMemoryCreation:
    def test_defaults(self):
        vm = VectorMemory()
        assert vm.max_entries == 1000
        assert vm.count() == 0
        assert vm.token_cost_per_query == 3

    def test_custom_max_entries(self):
        vm = make_vector_memory(max_entries=10)
        assert vm.max_entries == 10

    def test_invalid_max_entries(self):
        with pytest.raises(ValueError, match="positive integer"):
            VectorMemory(max_entries=0)

    def test_invalid_decay_days(self):
        with pytest.raises(ValueError, match="positive"):
            VectorMemory(decay_days=0)

    def test_custom_embedding_provider(self):
        provider = HashEmbeddingProvider(dimension=64)
        vm = VectorMemory(embedding_provider=provider)
        assert vm.dimension == 64

    def test_repr(self):
        vm = make_vector_memory()
        r = repr(vm)
        assert "VectorMemory" in r
        assert "0/50" in r


# ---------------------------------------------------------------------------
# VectorMemory — store
# ---------------------------------------------------------------------------


class TestVectorMemoryStore:
    def test_store_returns_entry(self):
        vm = make_vector_memory()
        entry = vm.store("learned about trading", memory_type="experience")
        assert isinstance(entry, VectorMemoryEntry)
        assert entry.id == 1
        assert entry.content == "learned about trading"
        assert entry.memory_type == "experience"

    def test_store_auto_increments_id(self):
        vm = make_vector_memory()
        e1 = vm.store("first")
        e2 = vm.store("second")
        assert e1.id == 1
        assert e2.id == 2

    def test_store_importance_clamped(self):
        vm = make_vector_memory()
        e1 = vm.store("high", importance=5.0)
        e2 = vm.store("low", importance=-1.0)
        assert e1.importance == 1.0
        assert e2.importance == 0.0

    def test_store_with_source(self):
        vm = make_vector_memory()
        entry = vm.store("insight", source="reflection")
        assert entry.source == "reflection"

    def test_store_with_metadata(self):
        vm = make_vector_memory()
        entry = vm.store("test", metadata={"foo": "bar"})
        assert entry.metadata == {"foo": "bar"}

    def test_store_generates_embedding(self):
        vm = make_vector_memory()
        entry = vm.store("test content")
        assert entry.embedding is not None
        assert len(entry.embedding) == vm.dimension

    def test_store_sets_timestamps(self):
        vm = make_vector_memory()
        entry = vm.store("test")
        assert entry.created_at != ""
        assert entry.last_accessed != ""
        assert entry.access_count == 0

    def test_count_increases(self):
        vm = make_vector_memory()
        assert vm.count() == 0
        vm.store("a")
        assert vm.count() == 1
        vm.store("b")
        assert vm.count() == 2

    def test_store_all_types(self):
        vm = make_vector_memory()
        e = vm.store("exp", memory_type="experience")
        assert e.memory_type == "experience"
        l = vm.store("les", memory_type="lesson")
        assert l.memory_type == "lesson"
        f = vm.store("fac", memory_type="fact")
        assert f.memory_type == "fact"


# ---------------------------------------------------------------------------
# VectorMemory — recall (semantic search)
# ---------------------------------------------------------------------------


class TestVectorMemoryRecall:
    def test_recall_finds_similar(self):
        vm = make_vector_memory()
        vm.store("Trading during a crisis is very risky", memory_type="lesson", importance=0.9)
        vm.store("The agent explored the forest and found berries", memory_type="experience")
        vm.store("Trading post opens at dawn", memory_type="fact")

        results = vm.recall("trading risk", top_k=2)
        assert len(results) >= 1
        # The first result should be about trading risk
        entry, score = results[0]
        assert "trading" in entry.content.lower() or "crisis" in entry.content.lower()
        assert score > 0.0

    def test_recall_exact_match_high_similarity(self):
        vm = make_vector_memory()
        vm.store("avoid trading during crisis", memory_type="lesson")
        results = vm.recall("avoid trading during crisis", top_k=1)
        assert len(results) == 1
        _, score = results[0]
        assert score > 0.9  # Should be very similar

    def test_recall_top_k_limit(self):
        vm = make_vector_memory()
        for i in range(10):
            vm.store(f"memory item {i}", importance=0.5)
        results = vm.recall("memory", top_k=3)
        assert len(results) <= 3

    def test_recall_empty_db(self):
        vm = make_vector_memory()
        results = vm.recall("anything", top_k=5)
        assert results == []

    def test_recall_with_type_filter(self):
        vm = make_vector_memory()
        vm.store("trading strategy", memory_type="experience")
        vm.store("never trade in panic", memory_type="lesson")
        vm.store("market opens at 9am", memory_type="fact")

        results = vm.recall("trading", top_k=5, memory_type="lesson")
        assert len(results) >= 1
        for entry, _ in results:
            assert entry.memory_type == "lesson"

    def test_recall_type_filter_no_match(self):
        vm = make_vector_memory()
        vm.store("trading is fun", memory_type="experience")
        results = vm.recall("trading", top_k=5, memory_type="fact")
        assert results == []

    def test_recall_returns_tuples_with_scores(self):
        vm = make_vector_memory()
        vm.store("test memory")
        results = vm.recall("test", top_k=5)
        for entry, score in results:
            assert isinstance(entry, VectorMemoryEntry)
            assert isinstance(score, float)
            assert 0.0 <= score <= 1.0

    def test_recall_min_relevance(self):
        vm = make_vector_memory()
        vm.store("completely unrelated xyz", importance=0.1)
        results = vm.recall("something very different abc", top_k=5, min_relevance=0.99)
        # Should filter out low-relevance results
        assert all(score >= 0.99 for _, score in results)


# ---------------------------------------------------------------------------
# VectorMemory — recall_with_decay (forgetting curve)
# ---------------------------------------------------------------------------


class TestVectorMemoryRecallWithDecay:
    def test_recent_memories_ranked_high(self):
        vm = make_vector_memory()
        vm.store("recent important lesson", memory_type="lesson", importance=0.9)
        results = vm.recall_with_decay("important lesson", top_k=1)
        assert len(results) == 1
        entry, score = results[0]
        assert "important lesson" in entry.content
        assert score > 0.0

    def test_decay_reduces_old_memory_score(self):
        vm = make_vector_memory()

        # Store a memory
        vm.store("old lesson about trading", memory_type="lesson", importance=0.9)

        # Manually update last_accessed to simulate 60 days ago
        old_date = (datetime.now(timezone.utc) - timedelta(days=60)).isoformat()
        vm._conn.execute(
            "UPDATE vector_memories SET last_accessed = ? WHERE id = 1",
            (old_date,),
        )
        vm._conn.commit()

        # Store a similar recent memory
        vm.store("recent lesson about trading", memory_type="lesson", importance=0.9)

        results = vm.recall_with_decay("trading lesson", top_k=2)
        assert len(results) == 2

        # Recent memory should rank higher
        recent_entry = [e for e, s in results if "recent" in e.content]
        old_entry = [e for e, s in results if "old" in e.content]
        if recent_entry and old_entry:
            recent_score = next(s for e, s in results if "recent" in e.content)
            old_score = next(s for e, s in results if "old" in e.content)
            assert recent_score > old_score

    def test_recall_with_decay_updates_access_stats(self):
        vm = make_vector_memory()
        vm.store("lesson to be accessed")
        initial_count = vm._conn.execute(
            "SELECT access_count FROM vector_memories WHERE id = 1"
        ).fetchone()[0]
        assert initial_count == 0

        vm.recall_with_decay("lesson", top_k=1)
        updated_count = vm._conn.execute(
            "SELECT access_count FROM vector_memories WHERE id = 1"
        ).fetchone()[0]
        assert updated_count == 1

    def test_recall_with_decay_type_filter(self):
        vm = make_vector_memory()
        vm.store("experience trading", memory_type="experience")
        vm.store("lesson about trading", memory_type="lesson")

        results = vm.recall_with_decay("trading", top_k=5, memory_type="lesson")
        for entry, _ in results:
            assert entry.memory_type == "lesson"


# ---------------------------------------------------------------------------
# Forgetting curve decay computation
# ---------------------------------------------------------------------------


class TestForgettingCurve:
    def test_recent_access_no_decay(self):
        """Just-accessed memory should have decay factor ≈ 1.0."""
        vm = make_vector_memory()
        now = _now_iso()
        factor = vm._compute_decay(now)
        assert factor > 0.99

    def test_30_day_decay(self):
        """Memory accessed 30 days ago should decay to ~0.5."""
        vm = make_vector_memory(decay_days=30.0)
        old_date = (datetime.now(timezone.utc) - timedelta(days=30)).isoformat()
        factor = vm._compute_decay(old_date)
        assert abs(factor - 0.5) < 0.05  # Should be approximately 0.5

    def test_60_day_decay(self):
        """Memory accessed 60 days ago should decay to ~0.25."""
        vm = make_vector_memory(decay_days=30.0)
        old_date = (datetime.now(timezone.utc) - timedelta(days=60)).isoformat()
        factor = vm._compute_decay(old_date)
        assert abs(factor - 0.25) < 0.05  # Should be approximately 0.25

    def test_future_access_no_decay(self):
        """Future-dated access should result in 1.0."""
        vm = make_vector_memory()
        future = (datetime.now(timezone.utc) + timedelta(days=10)).isoformat()
        factor = vm._compute_decay(future)
        assert factor == 1.0

    def test_invalid_date_no_decay(self):
        """Invalid date string should default to 1.0."""
        vm = make_vector_memory()
        factor = vm._compute_decay("not-a-date")
        assert factor == 1.0

    def test_custom_decay_days(self):
        """Custom decay_days should change the half-life."""
        vm = make_vector_memory(decay_days=10.0)
        old_date = (datetime.now(timezone.utc) - timedelta(days=10)).isoformat()
        factor = vm._compute_decay(old_date)
        assert abs(factor - 0.5) < 0.05  # 10 days = half-life for decay_days=10

    def test_decay_continuous(self):
        """Decay should be continuous, not a step function."""
        vm = make_vector_memory(decay_days=30.0)
        factor_15 = vm._compute_decay(
            (datetime.now(timezone.utc) - timedelta(days=15)).isoformat()
        )
        factor_30 = vm._compute_decay(
            (datetime.now(timezone.utc) - timedelta(days=30)).isoformat()
        )
        factor_45 = vm._compute_decay(
            (datetime.now(timezone.utc) - timedelta(days=45)).isoformat()
        )
        # 15-day factor should be between 30-day (0.5) and 1.0
        assert factor_30 < factor_15 < 1.0
        assert factor_45 < factor_30


# ---------------------------------------------------------------------------
# VectorMemory — eviction
# ---------------------------------------------------------------------------


class TestVectorMemoryEviction:
    def test_evicts_lowest_importance(self):
        vm = make_vector_memory(max_entries=3)
        vm.store("high importance", importance=0.9)
        vm.store("low importance", importance=0.2)
        vm.store("mid importance", importance=0.5)
        # This should trigger eviction of "low importance"
        vm.store("newest entry", importance=0.7)
        assert vm.count() == 3
        # Check "low importance" was evicted
        results = vm.recall("low importance", top_k=10)
        contents = [e.content for e, s in results]
        assert "low importance" not in contents

    def test_eviction_keeps_high_importance(self):
        vm = make_vector_memory(max_entries=2)
        vm.store("important", importance=0.9)
        vm.store("less important", importance=0.3)
        vm.store("medium", importance=0.6)
        assert vm.count() == 2


# ---------------------------------------------------------------------------
# VectorMemory — delete & clear
# ---------------------------------------------------------------------------


class TestVectorMemoryDeleteClear:
    def test_delete_existing(self):
        vm = make_vector_memory()
        entry = vm.store("to delete")
        assert vm.delete(entry.id) is True
        assert vm.count() == 0

    def test_delete_nonexistent(self):
        vm = make_vector_memory()
        assert vm.delete(999) is False

    def test_clear(self):
        vm = make_vector_memory()
        vm.store("a")
        vm.store("b")
        vm.clear()
        assert vm.count() == 0


# ---------------------------------------------------------------------------
# VectorMemory — token cost
# ---------------------------------------------------------------------------


class TestVectorMemoryTokenCost:
    def test_token_cost_per_query(self):
        vm = make_vector_memory()
        assert vm.token_cost_per_query == 3


# ---------------------------------------------------------------------------
# VectorMemory — len
# ---------------------------------------------------------------------------


class TestVectorMemoryLen:
    def test_len(self):
        vm = make_vector_memory()
        assert len(vm) == 0
        vm.store("a")
        assert len(vm) == 1


# ---------------------------------------------------------------------------
# VectorMemory — protocol compliance
# ---------------------------------------------------------------------------


class TestVectorMemoryProtocol:
    def test_satisfies_protocol(self):
        from agent_runtime.memory.vector_memory import VectorMemoryProtocol
        vm = VectorMemory()
        assert isinstance(vm, VectorMemoryProtocol)


# ---------------------------------------------------------------------------
# VectorMemory — close
# ---------------------------------------------------------------------------


class TestVectorMemoryClose:
    def test_close(self):
        vm = VectorMemory()
        vm.store("test")
        vm.close()
        # After close, operations should fail
        with pytest.raises(Exception):
            vm.count()
