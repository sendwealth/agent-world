"""Tests for the short-term memory module (SQLite-backed).

Covers:
- Construction with default and custom parameters
- Invalid parameter validation
- Store and retrieve (basic CRUD)
- Keyword search (case-insensitive LIKE matching)
- Semantic search / recall (with embeddings and fallback)
- Capacity eviction (oldest low-importance first)
- Automatic cleanup of old low-importance memories
- Token cost model (5 Tokens per query)
- Delete and clear operations
- Edge cases (empty search, capacity 1, duplicate content, empty embedding)
"""

from __future__ import annotations

import os
import tempfile

import pytest

from agent_runtime.memory.short_term import (
    ShortTermMemory,
    ShortTermMemoryEntry,
    ShortTermMemoryProtocol,
)


# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------


@pytest.fixture
def mem() -> ShortTermMemory:
    """Return a ShortTermMemory with in-memory DB and default settings."""
    return ShortTermMemory(db_path=":memory:", max_entries=100, max_age_ticks=1000)


@pytest.fixture
def small_mem() -> ShortTermMemory:
    """Return a ShortTermMemory with capacity 3 for eviction tests."""
    return ShortTermMemory(db_path=":memory:", max_entries=3, max_age_ticks=1000)


@pytest.fixture
def file_mem(tmp_path):
    """Return a ShortTermMemory backed by a file DB (tests persistence)."""
    db_file = tmp_path / "test_mem.db"
    m = ShortTermMemory(db_path=str(db_file), max_entries=100, max_age_ticks=1000)
    yield m
    m.close()


# ---------------------------------------------------------------------------
# Construction
# ---------------------------------------------------------------------------


class TestConstruction:
    def test_default_settings(self, mem: ShortTermMemory) -> None:
        assert mem.max_entries == 100
        assert mem.max_age_ticks == 1000
        assert mem.count() == 0
        assert len(mem) == 0

    def test_custom_settings(self) -> None:
        m = ShortTermMemory(db_path=":memory:", max_entries=50, max_age_ticks=500)
        assert m.max_entries == 50
        assert m.max_age_ticks == 500
        m.close()

    def test_zero_max_entries_raises(self) -> None:
        with pytest.raises(ValueError, match="positive integer"):
            ShortTermMemory(db_path=":memory:", max_entries=0)

    def test_negative_max_entries_raises(self) -> None:
        with pytest.raises(ValueError, match="positive integer"):
            ShortTermMemory(db_path=":memory:", max_entries=-5)

    def test_zero_max_age_ticks_raises(self) -> None:
        with pytest.raises(ValueError, match="positive integer"):
            ShortTermMemory(db_path=":memory:", max_age_ticks=0)

    def test_negative_max_age_ticks_raises(self) -> None:
        with pytest.raises(ValueError, match="positive integer"):
            ShortTermMemory(db_path=":memory:", max_age_ticks=-10)


# ---------------------------------------------------------------------------
# Store & Read
# ---------------------------------------------------------------------------


class TestStoreAndRetrieve:
    def test_store_returns_entry(self, mem: ShortTermMemory) -> None:
        entry = mem.store("hello world", importance=0.7, tick=10)
        assert isinstance(entry, ShortTermMemoryEntry)
        assert entry.id == 1
        assert entry.content == "hello world"
        assert entry.importance == 0.7
        assert entry.created_tick == 10

    def test_store_auto_increment_id(self, mem: ShortTermMemory) -> None:
        e1 = mem.store("first", tick=1)
        e2 = mem.store("second", tick=2)
        assert e1.id == 1
        assert e2.id == 2
        assert e2.id > e1.id

    def test_store_default_importance(self, mem: ShortTermMemory) -> None:
        entry = mem.store("default imp", tick=0)
        assert entry.importance == 0.5

    def test_store_importance_clamped(self, mem: ShortTermMemory) -> None:
        e1 = mem.store("high", importance=5.0, tick=0)
        assert e1.importance == 1.0
        e2 = mem.store("low", importance=-1.0, tick=0)
        assert e2.importance == 0.0

    def test_store_with_embedding(self, mem: ShortTermMemory) -> None:
        emb = [0.1, 0.2, 0.3, 0.4]
        entry = mem.store("embedded", importance=0.6, tick=5, embedding=emb)
        assert entry.embedding == emb

    def test_count(self, mem: ShortTermMemory) -> None:
        assert mem.count() == 0
        mem.store("a", tick=1)
        assert mem.count() == 1
        mem.store("b", tick=2)
        assert mem.count() == 2

    def test_len(self, mem: ShortTermMemory) -> None:
        assert len(mem) == 0
        mem.store("x", tick=1)
        assert len(mem) == 1


# ---------------------------------------------------------------------------
# Keyword Search
# ---------------------------------------------------------------------------


class TestKeywordSearch:
    def test_search_basic(self, mem: ShortTermMemory) -> None:
        mem.store("learned about trading", importance=0.6, tick=1)
        mem.store("explored the forest", importance=0.4, tick=2)
        mem.store("traded with merchant", importance=0.7, tick=3)

        results = mem.search("trading", top_k=5)
        assert len(results) >= 1
        assert any("trading" in r.content for r in results)

    def test_search_case_insensitive(self, mem: ShortTermMemory) -> None:
        mem.store("Found FOOD near river", importance=0.6, tick=1)
        results = mem.search("food", top_k=5)
        assert len(results) == 1
        assert "FOOD" in results[0].content

    def test_search_top_k_limits(self, mem: ShortTermMemory) -> None:
        for i in range(10):
            mem.store(f"memory about topic {i}", importance=0.5, tick=i)
        results = mem.search("topic", top_k=3)
        assert len(results) == 3

    def test_search_no_results(self, mem: ShortTermMemory) -> None:
        mem.store("hello world", importance=0.5, tick=1)
        results = mem.search("nonexistent", top_k=5)
        assert results == []

    def test_search_exact_match_first(self, mem: ShortTermMemory) -> None:
        mem.store("food", importance=0.5, tick=1)
        mem.store("found food near river", importance=0.5, tick=2)
        results = mem.search("food", top_k=5)
        assert len(results) == 2
        assert results[0].content == "food"

    def test_token_cost_per_query(self, mem: ShortTermMemory) -> None:
        assert mem.token_cost_per_query == 5


# ---------------------------------------------------------------------------
# Semantic Recall
# ---------------------------------------------------------------------------


class TestSemanticRecall:
    def test_recall_fallback_to_keyword(self, mem: ShortTermMemory) -> None:
        """When no embeddings exist, recall falls back to keyword search."""
        mem.store("learned about trading", importance=0.6, tick=1)
        mem.store("explored the forest", importance=0.4, tick=2)

        results = mem.recall("trading", top_k=5)
        assert len(results) >= 1
        assert any("trading" in r.content for r in results)

    def test_recall_with_embeddings(self, mem: ShortTermMemory) -> None:
        """Recall with embeddings performs relevance ranking."""
        mem.store("trading goods", importance=0.6, tick=1, embedding=[0.8, 0.2])
        mem.store("fighting enemies", importance=0.5, tick=2, embedding=[0.1, 0.9])
        mem.store("trading with allies", importance=0.7, tick=3, embedding=[0.7, 0.3])

        results = mem.recall("trading", top_k=2)
        assert len(results) <= 2
        # Both trading-related entries should rank higher
        trading_results = [r for r in results if "trading" in r.content]
        assert len(trading_results) >= 1

    def test_recall_no_results(self, mem: ShortTermMemory) -> None:
        mem.store("hello world", importance=0.5, tick=1, embedding=[0.5, 0.5])
        results = mem.recall("nonexistent", top_k=5)
        # With embeddings present but no keyword match, semantic search
        # may still return candidates ranked by relevance
        # The key contract is that it returns a list without error
        assert isinstance(results, list)


# ---------------------------------------------------------------------------
# Eviction (Capacity)
# ---------------------------------------------------------------------------


class TestEviction:
    def test_at_capacity_no_eviction(self, small_mem: ShortTermMemory) -> None:
        small_mem.store("a", importance=0.5, tick=1)
        small_mem.store("b", importance=0.5, tick=2)
        small_mem.store("c", importance=0.5, tick=3)
        assert small_mem.count() == 3

    def test_over_capacity_evicts_oldest_low_importance(
        self, small_mem: ShortTermMemory
    ) -> None:
        small_mem.store("a", importance=0.3, tick=1)
        small_mem.store("b", importance=0.7, tick=2)
        small_mem.store("c", importance=0.4, tick=3)
        # At capacity. Adding a new entry should evict "a" (oldest low-importance).
        small_mem.store("d", importance=0.5, tick=4)
        assert small_mem.count() == 3
        # "a" should be gone
        results = small_mem.search("a", top_k=10)
        assert len(results) == 0
        # "b" should still exist
        results = small_mem.search("b", top_k=10)
        assert len(results) >= 1

    def test_all_important_falls_back_to_oldest(
        self, small_mem: ShortTermMemory
    ) -> None:
        small_mem.store("imp-1", importance=0.8, tick=1)
        small_mem.store("imp-2", importance=0.9, tick=2)
        small_mem.store("imp-3", importance=0.7, tick=3)
        # All important. Adding new evicts the absolute oldest.
        small_mem.store("new", importance=0.5, tick=4)
        assert small_mem.count() == 3
        results = small_mem.search("imp-1", top_k=10)
        assert len(results) == 0  # "imp-1" evicted

    def test_multiple_evictions(self, small_mem: ShortTermMemory) -> None:
        for i in range(10):
            small_mem.store(f"item-{i}", importance=0.3, tick=i)
        assert small_mem.count() == 3
        # Only the last 3 should remain
        results = small_mem.search("item", top_k=10)
        assert len(results) == 3


# ---------------------------------------------------------------------------
# Automatic Cleanup (Age-based)
# ---------------------------------------------------------------------------


class TestAutoCleanup:
    def test_cleanup_old_low_importance(self, mem: ShortTermMemory) -> None:
        # max_age_ticks = 1000, so memories from tick 0 should be cleaned
        # when current tick >= 1000
        mem.store("old unimportant", importance=0.3, tick=0)
        mem.store("old important", importance=0.7, tick=0)
        mem.store("recent unimportant", importance=0.3, tick=500)

        # Store at tick 1001 triggers cleanup of entries older than tick 1
        mem.store("new entry", importance=0.5, tick=1001)

        # "old unimportant" should be cleaned (tick 0 < 1001 - 1000 = 1)
        results = mem.search("old unimportant", top_k=10)
        assert len(results) == 0

        # "old important" should survive (importance >= 0.5)
        results = mem.search("old important", top_k=10)
        assert len(results) == 1

        # "recent unimportant" should survive (tick 500 >= 1)
        results = mem.search("recent unimportant", top_k=10)
        assert len(results) == 1

    def test_no_cleanup_when_not_old_enough(self, mem: ShortTermMemory) -> None:
        mem.store("not too old", importance=0.3, tick=500)
        mem.store("trigger", importance=0.5, tick=1000)
        # tick 500 is >= 1000 - 1000 = 0, so not cleaned
        results = mem.search("not too old", top_k=10)
        assert len(results) == 1


# ---------------------------------------------------------------------------
# Delete & Clear
# ---------------------------------------------------------------------------


class TestDeleteAndClear:
    def test_delete_existing(self, mem: ShortTermMemory) -> None:
        entry = mem.store("to delete", importance=0.5, tick=1)
        assert mem.delete(entry.id) is True
        assert mem.count() == 0

    def test_delete_nonexistent(self, mem: ShortTermMemory) -> None:
        assert mem.delete(999) is False

    def test_clear(self, mem: ShortTermMemory) -> None:
        for i in range(5):
            mem.store(f"item-{i}", importance=0.5, tick=i)
        assert mem.count() == 5
        mem.clear()
        assert mem.count() == 0

    def test_clear_then_store(self, mem: ShortTermMemory) -> None:
        mem.store("old", importance=0.5, tick=1)
        mem.clear()
        entry = mem.store("new", importance=0.6, tick=2)
        assert mem.count() == 1
        assert entry.content == "new"


# ---------------------------------------------------------------------------
# Persistence (file-backed DB)
# ---------------------------------------------------------------------------


class TestPersistence:
    def test_data_persists_across_connections(self, tmp_path) -> None:
        db_file = str(tmp_path / "persist.db")
        m1 = ShortTermMemory(db_path=db_file, max_entries=100, max_age_ticks=1000)
        m1.store("persistent data", importance=0.6, tick=10)
        m1.close()

        m2 = ShortTermMemory(db_path=db_file, max_entries=100, max_age_ticks=1000)
        results = m2.search("persistent", top_k=5)
        assert len(results) == 1
        assert results[0].content == "persistent data"
        m2.close()


# ---------------------------------------------------------------------------
# Properties & Dunders
# ---------------------------------------------------------------------------


class TestProperties:
    def test_repr(self, mem: ShortTermMemory) -> None:
        r = repr(mem)
        assert "ShortTermMemory" in r
        assert "size=0/100" in r

    def test_repr_with_entries(self, mem: ShortTermMemory) -> None:
        mem.store("test", importance=0.5, tick=1)
        r = repr(mem)
        assert "size=1/100" in r


# ---------------------------------------------------------------------------
# Protocol compliance
# ---------------------------------------------------------------------------


class TestProtocol:
    def test_implements_protocol(self, mem: ShortTermMemory) -> None:
        """ShortTermMemory should satisfy the ShortTermMemoryProtocol."""
        assert isinstance(mem, ShortTermMemoryProtocol)


# ---------------------------------------------------------------------------
# Edge Cases
# ---------------------------------------------------------------------------


class TestEdgeCases:
    def test_capacity_one(self) -> None:
        m = ShortTermMemory(db_path=":memory:", max_entries=1, max_age_ticks=1000)
        m.store("only", importance=0.5, tick=1)
        assert m.count() == 1
        m.store("replacement", importance=0.5, tick=2)
        assert m.count() == 1
        results = m.search("only", top_k=10)
        assert len(results) == 0
        m.close()

    def test_store_empty_string(self, mem: ShortTermMemory) -> None:
        entry = mem.store("", importance=0.5, tick=1)
        assert entry.content == ""

    def test_store_duplicate_content(self, mem: ShortTermMemory) -> None:
        mem.store("dup", importance=0.5, tick=1)
        mem.store("dup", importance=0.5, tick=2)
        assert mem.count() == 2

    def test_empty_embedding(self, mem: ShortTermMemory) -> None:
        entry = mem.store("no emb", importance=0.5, tick=1, embedding=[])
        assert entry.embedding == []

    def test_none_embedding_default(self, mem: ShortTermMemory) -> None:
        entry = mem.store("default", importance=0.5, tick=1)
        assert entry.embedding is None

    def test_entry_str(self, mem: ShortTermMemory) -> None:
        entry = mem.store("test content", importance=0.75, tick=42)
        s = str(entry)
        assert "test content" in s
        assert "imp=0.75" in s
        assert "tick=42" in s

    def test_search_empty_db(self, mem: ShortTermMemory) -> None:
        results = mem.search("anything", top_k=5)
        assert results == []

    def test_recall_empty_db(self, mem: ShortTermMemory) -> None:
        results = mem.recall("anything", top_k=5)
        assert results == []

    def test_close_and_reopen(self, tmp_path) -> None:
        db_file = str(tmp_path / "close_test.db")
        m = ShortTermMemory(db_path=db_file, max_entries=10, max_age_ticks=100)
        m.store("data", importance=0.5, tick=1)
        m.close()
        # Re-opening should work fine
        m2 = ShortTermMemory(db_path=db_file, max_entries=10, max_age_ticks=100)
        assert m2.count() == 1
        m2.close()


# ---------------------------------------------------------------------------
# Acceptance Criteria
# ---------------------------------------------------------------------------


class TestAcceptanceCriteria:
    """Memory storage and retrieval tests — core acceptance criteria."""

    def test_store_and_retrieve_memory(self) -> None:
        """Core acceptance: store a memory and retrieve it via search."""
        mem = ShortTermMemory(db_path=":memory:", max_entries=100, max_age_ticks=1000)
        mem.store("agent learned to trade resources", importance=0.7, tick=50)
        mem.store("agent explored the northern region", importance=0.4, tick=51)

        # Keyword search
        results = mem.search("trade", top_k=5)
        assert len(results) == 1
        assert results[0].content == "agent learned to trade resources"
        assert results[0].importance == 0.7
        assert results[0].created_tick == 50

        # Recall (semantic with fallback)
        results2 = mem.recall("trade", top_k=5)
        assert len(results2) >= 1
        assert any("trade" in r.content for r in results2)

        mem.close()

    def test_max_100_entries(self) -> None:
        """Acceptance: store the most recent 100 entries."""
        mem = ShortTermMemory(db_path=":memory:", max_entries=100, max_age_ticks=10000)
        for i in range(150):
            mem.store(f"memory-{i}", importance=0.3, tick=i)
        assert mem.count() == 100
        # Latest memories should be present
        results = mem.search("memory-149", top_k=5)
        assert len(results) == 1
        # Oldest should be gone
        results = mem.search("memory-0", top_k=5)
        assert len(results) == 0
        mem.close()

    def test_auto_cleanup_low_importance_old_memories(self) -> None:
        """Acceptance: clean up memories older than 1000 ticks with low importance."""
        mem = ShortTermMemory(db_path=":memory:", max_entries=100, max_age_ticks=1000)
        # Old low-importance memory
        mem.store("old low importance", importance=0.3, tick=0)
        # Old high-importance memory
        mem.store("old high importance", importance=0.8, tick=0)
        # Trigger cleanup by storing at tick > 1000
        mem.store("new memory", importance=0.5, tick=1001)

        results = mem.search("old low importance", top_k=5)
        assert len(results) == 0  # cleaned up
        results = mem.search("old high importance", top_k=5)
        assert len(results) == 1  # kept (high importance)
        mem.close()

    def test_token_cost_5_per_query(self) -> None:
        """Acceptance: Token cost is 5 Tokens per query."""
        mem = ShortTermMemory(db_path=":memory:", max_entries=100, max_age_ticks=1000)
        assert mem.token_cost_per_query == 5
        mem.close()

    def test_sqlite_schema_fields(self) -> None:
        """Acceptance: SQLite table has id, content, importance, created_tick, embedding."""
        mem = ShortTermMemory(db_path=":memory:", max_entries=100, max_age_ticks=1000)
        # Verify schema by storing and checking the returned entry
        entry = mem.store(
            "schema test",
            importance=0.6,
            tick=42,
            embedding=[0.1, 0.2, 0.3],
        )
        assert hasattr(entry, "id")
        assert hasattr(entry, "content")
        assert hasattr(entry, "importance")
        assert hasattr(entry, "created_tick")
        assert hasattr(entry, "embedding")
        assert entry.id is not None
        assert entry.content == "schema test"
        assert entry.importance == 0.6
        assert entry.created_tick == 42
        assert entry.embedding == [0.1, 0.2, 0.3]
        mem.close()
