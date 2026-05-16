"""Working memory — in-memory FIFO cache for recent interactions.

Stores the most recent *capacity* interactions in a deque with automatic
FIFO eviction.  Reads are free (no token consumption).  Supports
importance marking so that critical entries are evicted last.

The think-loop stores each perception/decision/action in the working
memory so the agent can recall recent context without paying LLM token
costs.  When the deque is full, the oldest non-important entry is
evicted first; if all entries are important, the absolute oldest is
removed as a fallback.
"""

from __future__ import annotations

import logging
import time
from collections import deque
from dataclasses import dataclass, field
from typing import Any, Protocol

logger = logging.getLogger(__name__)


# ---------------------------------------------------------------------------
# Data types
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class MemoryEntry:
    """A single entry in working memory."""

    content: str
    timestamp: float = field(default_factory=time.time)
    important: bool = False
    metadata: dict[str, Any] = field(default_factory=dict)

    def __str__(self) -> str:
        flag = " [!]" if self.important else ""
        return f"{self.content}{flag}"


# ---------------------------------------------------------------------------
# Protocol for agent state (dependency injection)
# ---------------------------------------------------------------------------


class WorkingMemoryProtocol(Protocol):
    """Minimal interface that consumers depend on."""

    def store(
        self, content: str, *, important: bool = False, metadata: dict[str, Any] | None = None
    ) -> MemoryEntry: ...

    def read_all(self) -> list[MemoryEntry]: ...

    def read_recent(self, n: int = 5) -> list[MemoryEntry]: ...


# ---------------------------------------------------------------------------
# WorkingMemory
# ---------------------------------------------------------------------------

_DEFAULT_CAPACITY: int = 10


class WorkingMemory:
    """In-memory FIFO cache for recent interactions.

    Parameters
    ----------
    capacity : int
        Maximum number of entries to retain.  When ``store`` is called
        and the deque is already at capacity, the oldest **non-important**
        entry is evicted first.  If all entries are marked important the
        absolute oldest entry is evicted regardless.

    Usage::

        memory = WorkingMemory(capacity=10)
        memory.store("perceived food nearby")
        memory.store("decided to move north", important=True)
        recent = memory.read_recent(5)
    """

    def __init__(self, capacity: int = _DEFAULT_CAPACITY) -> None:
        if capacity <= 0:
            raise ValueError("capacity must be a positive integer")
        self._capacity = capacity
        self._entries: deque[MemoryEntry] = deque()

    # ------------------------------------------------------------------
    # Core API
    # ------------------------------------------------------------------

    def store(
        self,
        content: str,
        *,
        important: bool = False,
        metadata: dict[str, Any] | None = None,
    ) -> MemoryEntry:
        """Store a new interaction entry.

        If the deque is at capacity the oldest non-important entry is
        evicted; if all entries are important the absolute oldest is
        evicted instead.

        Parameters
        ----------
        content : str
            The interaction text to store.
        important : bool
            Mark this entry as important.  Important entries are
            evicted **last** when the memory is full.
        metadata : dict | None
            Optional arbitrary metadata attached to the entry.

        Returns
        -------
        MemoryEntry
            The newly created entry.
        """
        entry = MemoryEntry(
            content=content,
            important=important,
            metadata=metadata or {},
        )

        if len(self._entries) >= self._capacity:
            evicted = self._evict_one()
            if evicted is not None:
                logger.debug("Evicted entry: %s", evicted.content)

        self._entries.append(entry)
        logger.debug(
            "Stored entry: %s (important=%s, size=%d/%d)",
            content[:50],
            important,
            len(self._entries),
            self._capacity,
        )
        return entry

    def read_all(self) -> list[MemoryEntry]:
        """Return all stored entries (newest first).

        This is a **free** read — no token consumption.
        """
        return list(reversed(self._entries))

    def read_recent(self, n: int = 5) -> list[MemoryEntry]:
        """Return the *n* most recent entries (newest first).

        Free read — no token consumption.
        """
        entries = list(reversed(self._entries))
        return entries[:n]

    def mark_important(self, index: int) -> MemoryEntry:
        """Mark the entry at *index* (0 = oldest) as important.

        Because entries are stored in a ``deque`` we look up by positional
        index.
        """
        entry = self._entries[index]
        # frozen=True prevents mutation; create a new entry.
        new_entry = MemoryEntry(
            content=entry.content,
            timestamp=entry.timestamp,
            important=True,
            metadata=entry.metadata,
        )
        self._entries[index] = new_entry
        logger.debug("Marked entry at index %d as important: %s", index, entry.content[:50])
        return new_entry

    def mark_unimportant(self, index: int) -> MemoryEntry:
        """Remove the important flag from the entry at *index*."""
        entry = self._entries[index]
        new_entry = MemoryEntry(
            content=entry.content,
            timestamp=entry.timestamp,
            important=False,
            metadata=entry.metadata,
        )
        self._entries[index] = new_entry
        return new_entry

    def clear(self) -> None:
        """Remove all entries."""
        self._entries.clear()
        logger.debug("Cleared working memory")

    # ------------------------------------------------------------------
    # Properties
    # ------------------------------------------------------------------

    @property
    def capacity(self) -> int:
        return self._capacity

    @property
    def size(self) -> int:
        return len(self._entries)

    @property
    def is_full(self) -> bool:
        return len(self._entries) >= self._capacity

    # ------------------------------------------------------------------
    # Dunder helpers
    # ------------------------------------------------------------------

    def __len__(self) -> int:
        return len(self._entries)

    def __contains__(self, content: str) -> bool:
        return any(e.content == content for e in self._entries)

    def __repr__(self) -> str:
        return f"WorkingMemory(size={self.size}/{self.capacity})"

    # ------------------------------------------------------------------
    # Private helpers
    # ------------------------------------------------------------------

    def _evict_one(self) -> MemoryEntry | None:
        """Evict the oldest non-important entry, or the absolute oldest.

        Returns the evicted entry, or ``None`` if the deque was empty.
        """
        if not self._entries:
            return None

        # First pass: remove oldest non-important entry.
        for i, entry in enumerate(self._entries):
            if not entry.important:
                evicted = self._entries[i]
                del self._entries[i]
                return evicted

        # All entries are important — evict the absolute oldest.
        evicted = self._entries.popleft()
        return evicted
