"""Tests for the working memory module.

Covers:
- Construction with default and custom capacity
- Invalid capacity validation
- Store, read_all, read_recent
- FIFO eviction (acceptance criterion: 10 items auto-evict oldest)
- Importance marking and eviction priority
- mark_important / mark_unimportant (frozen dataclass replacement)
- clear
- Properties (size, capacity, is_full)
- Dunder methods (__len__, __contains__, __repr__)
- Edge cases (capacity=1, empty string, duplicate content)
"""

from __future__ import annotations

import pytest

from agent_runtime.memory.working_memory import MemoryEntry, WorkingMemory

# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------


@pytest.fixture
def mem() -> WorkingMemory:
    """Return a WorkingMemory with default capacity (10)."""
    return WorkingMemory()


@pytest.fixture
def small_mem() -> WorkingMemory:
    """Return a WorkingMemory with capacity 3 for easier eviction tests."""
    return WorkingMemory(capacity=3)


# ---------------------------------------------------------------------------
# Construction
# ---------------------------------------------------------------------------


class TestConstruction:
    def test_default_capacity(self, mem: WorkingMemory) -> None:
        assert mem.capacity == 10
        assert mem.size == 0

    def test_custom_capacity(self) -> None:
        m = WorkingMemory(capacity=5)
        assert m.capacity == 5

    def test_zero_capacity_raises(self) -> None:
        with pytest.raises(ValueError, match="positive integer"):
            WorkingMemory(capacity=0)

    def test_negative_capacity_raises(self) -> None:
        with pytest.raises(ValueError, match="positive integer"):
            WorkingMemory(capacity=-3)


# ---------------------------------------------------------------------------
# Store & Read
# ---------------------------------------------------------------------------


class TestStoreAndRead:
    def test_store_returns_entry(self, mem: WorkingMemory) -> None:
        entry = mem.store("hello")
        assert isinstance(entry, MemoryEntry)
        assert entry.content == "hello"
        assert entry.important is False

    def test_store_with_importance(self, mem: WorkingMemory) -> None:
        entry = mem.store("critical", important=True)
        assert entry.important is True

    def test_store_with_metadata(self, mem: WorkingMemory) -> None:
        entry = mem.store("data", metadata={"source": "test"})
        assert entry.metadata == {"source": "test"}

    def test_read_all_empty(self, mem: WorkingMemory) -> None:
        assert mem.read_all() == []

    def test_read_all_returns_newest_first(self, mem: WorkingMemory) -> None:
        mem.store("first")
        mem.store("second")
        mem.store("third")
        contents = [e.content for e in mem.read_all()]
        assert contents == ["third", "second", "first"]

    def test_read_recent(self, mem: WorkingMemory) -> None:
        for i in range(8):
            mem.store(f"item-{i}")
        recent = mem.read_recent(3)
        contents = [e.content for e in recent]
        assert contents == ["item-7", "item-6", "item-5"]

    def test_read_recent_n_larger_than_size(self, mem: WorkingMemory) -> None:
        mem.store("only")
        recent = mem.read_recent(10)
        assert len(recent) == 1
        assert recent[0].content == "only"


# ---------------------------------------------------------------------------
# FIFO Eviction (Acceptance Criterion)
# ---------------------------------------------------------------------------


class TestFIFOEviction:
    """Core acceptance test: storing 10 items auto-evicts the oldest."""

    def test_at_capacity_no_eviction(self, mem: WorkingMemory) -> None:
        for i in range(10):
            mem.store(f"item-{i}")
        assert mem.size == 10
        assert "item-0" in mem
        assert "item-9" in mem

    def test_over_capacity_evicts_oldest(self, mem: WorkingMemory) -> None:
        """Acceptance criterion: after 10 items, adding one more evicts oldest."""
        for i in range(10):
            mem.store(f"item-{i}")
        # Now add the 11th entry — should evict item-0
        mem.store("item-10")
        assert mem.size == 10
        assert "item-0" not in mem
        assert "item-1" in mem
        assert "item-10" in mem

    def test_multiple_evictions(self, mem: WorkingMemory) -> None:
        for i in range(15):
            mem.store(f"item-{i}")
        assert mem.size == 10
        # Items 0–4 should be gone, items 5–14 should remain
        for i in range(5):
            assert f"item-{i}" not in mem
        for i in range(5, 15):
            assert f"item-{i}" in mem

    def test_small_memory_eviction(self, small_mem: WorkingMemory) -> None:
        small_mem.store("a")
        small_mem.store("b")
        small_mem.store("c")
        assert small_mem.size == 3
        small_mem.store("d")
        assert small_mem.size == 3
        assert "a" not in small_mem
        assert "b" in small_mem
        assert "c" in small_mem
        assert "d" in small_mem


# ---------------------------------------------------------------------------
# Importance marking
# ---------------------------------------------------------------------------


class TestImportance:
    def test_important_entries_evicted_last(self, small_mem: WorkingMemory) -> None:
        small_mem.store("important-1", important=True)
        small_mem.store("normal-1")
        small_mem.store("normal-2")
        # At capacity. Adding a new entry should evict "normal-1" (oldest non-important).
        small_mem.store("new")
        assert "important-1" in small_mem
        assert "normal-1" not in small_mem
        assert "new" in small_mem

    def test_all_important_falls_back_to_fifo(self, small_mem: WorkingMemory) -> None:
        small_mem.store("imp-1", important=True)
        small_mem.store("imp-2", important=True)
        small_mem.store("imp-3", important=True)
        # All important. Adding new entry evicts the absolute oldest.
        small_mem.store("new")
        assert "imp-1" not in small_mem  # oldest evicted even though important
        assert "imp-2" in small_mem
        assert "new" in small_mem

    def test_mark_important_after_store(self, small_mem: WorkingMemory) -> None:
        small_mem.store("a")
        small_mem.mark_important(0)  # index 0 = oldest = "a"
        small_mem.store("b")
        small_mem.store("c")
        # "a" is important; "b" should be evicted first
        small_mem.store("d")
        assert "a" in small_mem
        assert "b" not in small_mem

    def test_mark_unimportant(self, small_mem: WorkingMemory) -> None:
        small_mem.store("a", important=True)
        small_mem.mark_unimportant(0)
        entry = small_mem.read_all()[-1]  # oldest is last in read_all
        assert entry.important is False

    def test_importance_str_display(self) -> None:
        entry = MemoryEntry(content="test", important=True)
        assert "[!]" in str(entry)
        entry2 = MemoryEntry(content="test", important=False)
        assert "[!]" not in str(entry2)


# ---------------------------------------------------------------------------
# Properties & Dunders
# ---------------------------------------------------------------------------


class TestProperties:
    def test_is_full(self, small_mem: WorkingMemory) -> None:
        assert not small_mem.is_full
        small_mem.store("a")
        small_mem.store("b")
        assert not small_mem.is_full
        small_mem.store("c")
        assert small_mem.is_full

    def test_len(self, mem: WorkingMemory) -> None:
        assert len(mem) == 0
        mem.store("x")
        assert len(mem) == 1

    def test_contains(self, mem: WorkingMemory) -> None:
        mem.store("hello")
        assert "hello" in mem
        assert "world" not in mem

    def test_repr(self, mem: WorkingMemory) -> None:
        assert repr(mem) == "WorkingMemory(size=0/10)"
        mem.store("x")
        assert repr(mem) == "WorkingMemory(size=1/10)"


# ---------------------------------------------------------------------------
# Clear
# ---------------------------------------------------------------------------


class TestClear:
    def test_clear_empties_memory(self, mem: WorkingMemory) -> None:
        for i in range(5):
            mem.store(f"item-{i}")
        assert mem.size == 5
        mem.clear()
        assert mem.size == 0
        assert mem.read_all() == []

    def test_clear_then_store(self, mem: WorkingMemory) -> None:
        for i in range(10):
            mem.store(f"old-{i}")
        mem.clear()
        mem.store("new")
        assert mem.size == 1
        assert "new" in mem


# ---------------------------------------------------------------------------
# Edge cases
# ---------------------------------------------------------------------------


class TestEdgeCases:
    def test_capacity_one(self) -> None:
        m = WorkingMemory(capacity=1)
        m.store("only")
        assert m.size == 1
        m.store("replacement")
        assert m.size == 1
        assert "only" not in m
        assert "replacement" in m

    def test_store_empty_string(self, mem: WorkingMemory) -> None:
        entry = mem.store("")
        assert entry.content == ""
        assert "" in mem

    def test_store_duplicate_content(self, mem: WorkingMemory) -> None:
        mem.store("dup")
        mem.store("dup")
        assert len(mem) == 2

    def test_metadata_default_empty(self, mem: WorkingMemory) -> None:
        entry = mem.store("no-meta")
        assert entry.metadata == {}

    def test_timestamp_auto_set(self, mem: WorkingMemory) -> None:
        entry = mem.store("timed")
        assert entry.timestamp > 0
