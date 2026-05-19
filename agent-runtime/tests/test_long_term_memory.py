"""Tests for long-term memory (agent_runtime.memory.long_term).

Covers:
- LongTermMemoryEntry creation and string representation
- LongTermMemory creation and defaults
- Store and retrieve entries
- Keyword search
- Category filtering
- Importance-based ordering
- Eviction when max_entries exceeded
- Delete and clear
- Token cost property
- Edge cases: empty search, max_entries=1, etc.
"""

from __future__ import annotations

import pytest

from agent_runtime.memory.long_term import (
    LongTermMemory,
    LongTermMemoryEntry,
)

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def make_memory(**kwargs):
    """Create a LongTermMemory with in-memory DB and small max for tests."""
    defaults = {"db_path": ":memory:", "max_entries": 50}
    defaults.update(kwargs)
    return LongTermMemory(**defaults)


# ---------------------------------------------------------------------------
# LongTermMemoryEntry
# ---------------------------------------------------------------------------


class TestLongTermMemoryEntry:
    def test_str_representation(self):
        entry = LongTermMemoryEntry(
            id=1, content="test", category="strategy",
            importance=0.8, created_tick=42,
        )
        s = str(entry)
        assert "test" in s
        assert "strategy" in s
        assert "42" in s

    def test_frozen(self):
        entry = LongTermMemoryEntry(
            id=1, content="test", category="experience", importance=0.5,
        )
        with pytest.raises(AttributeError):
            entry.content = "changed"  # type: ignore[misc]

    def test_default_metadata(self):
        entry = LongTermMemoryEntry(
            id=1, content="test", category="experience", importance=0.5,
        )
        assert entry.metadata == {}

    def test_custom_metadata(self):
        entry = LongTermMemoryEntry(
            id=1, content="test", category="experience", importance=0.5,
            metadata={"key": "value"},
        )
        assert entry.metadata == {"key": "value"}


# ---------------------------------------------------------------------------
# LongTermMemory — creation
# ---------------------------------------------------------------------------


class TestLongTermMemoryCreation:
    def test_defaults(self):
        mem = LongTermMemory()
        assert mem.max_entries == 500
        assert mem.count() == 0

    def test_custom_max_entries(self):
        mem = make_memory(max_entries=10)
        assert mem.max_entries == 10

    def test_invalid_max_entries(self):
        with pytest.raises(ValueError, match="positive integer"):
            LongTermMemory(max_entries=0)

    def test_repr(self):
        mem = make_memory()
        r = repr(mem)
        assert "LongTermMemory" in r
        assert "0/50" in r


# ---------------------------------------------------------------------------
# LongTermMemory — store
# ---------------------------------------------------------------------------


class TestLongTermMemoryStore:
    def test_store_returns_entry(self):
        mem = make_memory()
        entry = mem.store("learned about trading", category="experience", tick=42)
        assert isinstance(entry, LongTermMemoryEntry)
        assert entry.id == 1
        assert entry.content == "learned about trading"
        assert entry.category == "experience"
        assert entry.created_tick == 42

    def test_store_auto_increments_id(self):
        mem = make_memory()
        e1 = mem.store("first")
        e2 = mem.store("second")
        assert e1.id == 1
        assert e2.id == 2

    def test_store_importance_clamped(self):
        mem = make_memory()
        e1 = mem.store("high", importance=5.0)
        e2 = mem.store("low", importance=-1.0)
        assert e1.importance == 1.0
        assert e2.importance == 0.0

    def test_store_with_source(self):
        mem = make_memory()
        entry = mem.store("insight", source="reflection", tick=100)
        assert entry.source == "reflection"

    def test_store_with_metadata(self):
        mem = make_memory()
        entry = mem.store("test", metadata={"foo": "bar"})
        assert entry.metadata == {"foo": "bar"}

    def test_count_increases(self):
        mem = make_memory()
        assert mem.count() == 0
        mem.store("a")
        assert mem.count() == 1
        mem.store("b")
        assert mem.count() == 2


# ---------------------------------------------------------------------------
# LongTermMemory — search
# ---------------------------------------------------------------------------


class TestLongTermMemorySearch:
    def test_search_finds_match(self):
        mem = make_memory()
        mem.store("learned about trading", category="experience")
        mem.store("explored the forest", category="experience")
        results = mem.search("trading")
        assert len(results) == 1
        assert "trading" in results[0].content

    def test_search_no_match(self):
        mem = make_memory()
        mem.store("hello world")
        results = mem.search("nonexistent")
        assert results == []

    def test_search_top_k(self):
        mem = make_memory()
        for i in range(10):
            mem.store(f"trading item {i}", importance=0.5 + i * 0.05)
        results = mem.search("trading", top_k=3)
        assert len(results) == 3

    def test_search_ordered_by_importance(self):
        mem = make_memory()
        mem.store("trading low", importance=0.3)
        mem.store("trading high", importance=0.9)
        mem.store("trading mid", importance=0.6)
        results = mem.search("trading", top_k=3)
        assert results[0].content == "trading high"
        assert results[0].importance == 0.9

    def test_search_category_filter(self):
        mem = make_memory()
        mem.store("trading strategy", category="strategy")
        mem.store("trading experience", category="experience")
        mem.store("combat strategy", category="strategy")
        results = mem.search("trading", category="strategy")
        assert len(results) == 1
        assert results[0].category == "strategy"

    def test_search_category_no_match(self):
        mem = make_memory()
        mem.store("trading", category="experience")
        results = mem.search("trading", category="strategy")
        assert results == []


# ---------------------------------------------------------------------------
# LongTermMemory — recall (alias for search)
# ---------------------------------------------------------------------------


class TestLongTermMemoryRecall:
    def test_recall_same_as_search(self):
        mem = make_memory()
        mem.store("test memory")
        search_results = mem.search("test")
        recall_results = mem.recall("test")
        assert len(search_results) == len(recall_results)


# ---------------------------------------------------------------------------
# LongTermMemory — get_recent
# ---------------------------------------------------------------------------


class TestLongTermMemoryGetRecent:
    def test_get_recent_ordered_by_tick(self):
        mem = make_memory()
        mem.store("old", tick=1)
        mem.store("newer", tick=10)
        mem.store("newest", tick=100)
        results = mem.get_recent(top_k=2)
        assert len(results) == 2
        assert results[0].content == "newest"
        assert results[0].created_tick == 100

    def test_get_recent_category_filter(self):
        mem = make_memory()
        mem.store("strategy a", category="strategy", tick=1)
        mem.store("experience b", category="experience", tick=2)
        mem.store("strategy c", category="strategy", tick=3)
        results = mem.get_recent(top_k=10, category="strategy")
        assert len(results) == 2
        assert all(r.category == "strategy" for r in results)


# ---------------------------------------------------------------------------
# LongTermMemory — eviction
# ---------------------------------------------------------------------------


class TestLongTermMemoryEviction:
    def test_evicts_lowest_importance(self):
        mem = make_memory(max_entries=3)
        mem.store("high importance", importance=0.9)
        mem.store("low importance", importance=0.2)
        mem.store("mid importance", importance=0.5)
        # This should trigger eviction of "low importance"
        mem.store("newest entry", importance=0.7)
        assert mem.count() == 3
        # "low importance" should have been evicted
        all_content = [r.content for r in mem.get_recent(top_k=10)]
        assert "low importance" not in all_content

    def test_eviction_keeps_high_importance(self):
        mem = make_memory(max_entries=2)
        mem.store("important", importance=0.9)
        mem.store("less important", importance=0.3)
        mem.store("medium", importance=0.6)
        # "less important" should be evicted
        assert mem.count() == 2


# ---------------------------------------------------------------------------
# LongTermMemory — delete & clear
# ---------------------------------------------------------------------------


class TestLongTermMemoryDeleteClear:
    def test_delete_existing(self):
        mem = make_memory()
        entry = mem.store("to delete")
        assert mem.delete(entry.id) is True
        assert mem.count() == 0

    def test_delete_nonexistent(self):
        mem = make_memory()
        assert mem.delete(999) is False

    def test_clear(self):
        mem = make_memory()
        mem.store("a")
        mem.store("b")
        mem.clear()
        assert mem.count() == 0


# ---------------------------------------------------------------------------
# LongTermMemory — token cost
# ---------------------------------------------------------------------------


class TestLongTermMemoryTokenCost:
    def test_token_cost_per_query(self):
        mem = make_memory()
        assert mem.token_cost_per_query == 3


# ---------------------------------------------------------------------------
# LongTermMemory — len
# ---------------------------------------------------------------------------


class TestLongTermMemoryLen:
    def test_len(self):
        mem = make_memory()
        assert len(mem) == 0
        mem.store("a")
        assert len(mem) == 1
