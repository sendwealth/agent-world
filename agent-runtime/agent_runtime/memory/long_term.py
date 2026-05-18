"""Long-term memory — SQLite-backed persistent memory for learned experiences.

Stores high-importance memories that survive across sessions. Unlike
ShortTermMemory which has a small capacity and auto-evicts old entries,
LongTermMemory is designed to hold learned experiences, strategy insights,
and reflection outcomes with rich metadata.

Retrieval supports:
- **Keyword search**: case-insensitive LIKE matching on content.
- **Category filter**: filter by memory category (strategy, experience, insight).
- **Importance ranking**: results ordered by importance score.

Token cost model:
- ``store``: free (writing is a local operation).
- ``search`` / ``recall``: **3 Tokens** per query (cheaper than short-term
  since long-term is less time-sensitive).
"""

from __future__ import annotations

import json
import logging
import sqlite3
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Protocol, runtime_checkable

logger = logging.getLogger(__name__)

# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------

_DEFAULT_MAX_ENTRIES: int = 500
_TOKEN_COST_PER_QUERY: int = 3

_SCHEMA = """\
CREATE TABLE IF NOT EXISTS long_term_memories (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    content     TEXT    NOT NULL,
    category    TEXT    NOT NULL DEFAULT 'experience',
    importance  REAL    NOT NULL DEFAULT 0.7,
    source      TEXT    DEFAULT NULL,
    created_tick INTEGER NOT NULL DEFAULT 0,
    metadata    TEXT    DEFAULT NULL
);
CREATE INDEX IF NOT EXISTS idx_ltm_category ON long_term_memories(category);
CREATE INDEX IF NOT EXISTS idx_ltm_importance ON long_term_memories(importance);
"""

# ---------------------------------------------------------------------------
# Data types
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class LongTermMemoryEntry:
    """A single entry in long-term memory."""

    id: int
    content: str
    category: str  # "strategy", "experience", "insight"
    importance: float
    source: str | None = None  # "reflection", "observation", "learning"
    created_tick: int = 0
    metadata: dict[str, Any] = field(default_factory=dict)

    def __str__(self) -> str:
        return (
            f"[{self.id}] {self.content} "
            f"(cat={self.category}, imp={self.importance:.2f}, tick={self.created_tick})"
        )


# ---------------------------------------------------------------------------
# Protocol
# ---------------------------------------------------------------------------


@runtime_checkable
class LongTermMemoryProtocol(Protocol):
    """Minimal interface that consumers depend on."""

    def store(
        self,
        content: str,
        *,
        category: str = "experience",
        importance: float = 0.7,
        source: str | None = None,
        tick: int = 0,
        metadata: dict[str, Any] | None = None,
    ) -> LongTermMemoryEntry: ...

    def search(
        self, query: str, *, top_k: int = 5, category: str | None = None, tick: int = 0
    ) -> list[LongTermMemoryEntry]: ...

    def recall(
        self, query: str, *, top_k: int = 5, category: str | None = None, tick: int = 0
    ) -> list[LongTermMemoryEntry]: ...


# ---------------------------------------------------------------------------
# LongTermMemory
# ---------------------------------------------------------------------------


class LongTermMemory:
    """SQLite-backed persistent long-term memory.

    Parameters
    ----------
    db_path : str | Path
        Path to the SQLite database file.  Use ``":memory:"`` for an
        in-memory database (useful for testing).
    max_entries : int
        Maximum number of entries to retain.  When ``store`` is called and
        the table already holds *max_entries* rows, the lowest-importance
        oldest entry is evicted first.

    Usage::

        mem = LongTermMemory(db_path="agent_ltm.db", max_entries=500)
        mem.store("avoid trading in crisis mode", category="strategy",
                  importance=0.9, source="reflection", tick=100)
        results = mem.search("trading", top_k=3)
    """

    def __init__(
        self,
        db_path: str | Path = ":memory:",
        max_entries: int = _DEFAULT_MAX_ENTRIES,
    ) -> None:
        if max_entries <= 0:
            raise ValueError("max_entries must be a positive integer")

        self._db_path = str(db_path)
        self._max_entries = max_entries
        self._conn = sqlite3.connect(self._db_path)
        self._conn.execute("PRAGMA journal_mode=WAL")
        self._conn.executescript(_SCHEMA)
        self._conn.commit()

    # ------------------------------------------------------------------
    # Core API
    # ------------------------------------------------------------------

    def store(
        self,
        content: str,
        *,
        category: str = "experience",
        importance: float = 0.7,
        source: str | None = None,
        tick: int = 0,
        metadata: dict[str, Any] | None = None,
    ) -> LongTermMemoryEntry:
        """Store a new long-term memory entry.

        Parameters
        ----------
        content : str
            The memory text to store.
        category : str
            Category of memory: "strategy", "experience", or "insight".
        importance : float
            Importance score between 0.0 and 1.0.
        source : str | None
            Origin of this memory: "reflection", "observation", "learning".
        tick : int
            The simulation tick at which this memory was created.
        metadata : dict | None
            Optional arbitrary metadata.

        Returns
        -------
        LongTermMemoryEntry
            The newly created entry (with its auto-generated ``id``).
        """
        importance = max(0.0, min(1.0, importance))
        meta_json: str | None = json.dumps(metadata) if metadata is not None else None

        cursor = self._conn.execute(
            "INSERT INTO long_term_memories "
            "(content, category, importance, source, created_tick, metadata) "
            "VALUES (?, ?, ?, ?, ?, ?)",
            (content, category, importance, source, tick, meta_json),
        )
        self._conn.commit()

        if self._count() > self._max_entries:
            self._evict_one()

        entry = LongTermMemoryEntry(
            id=cursor.lastrowid,
            content=content,
            category=category,
            importance=importance,
            source=source,
            created_tick=tick,
            metadata=metadata or {},
        )
        logger.debug(
            "Stored long-term memory: %s (cat=%s, imp=%.2f, tick=%d)",
            content[:50],
            category,
            importance,
            tick,
        )
        return entry

    def search(
        self, query: str, *, top_k: int = 5, category: str | None = None, tick: int = 0
    ) -> list[LongTermMemoryEntry]:
        """Search memories by keyword (case-insensitive LIKE match).

        Costs **3 Tokens** per query.

        Parameters
        ----------
        query : str
            Keyword or phrase to search for.
        top_k : int
            Maximum number of results.
        category : str | None
            Filter by category if specified.
        tick : int
            Current tick (unused by keyword search).

        Returns
        -------
        list[LongTermMemoryEntry]
            Matching entries, ordered by importance (desc) then recency.
        """
        if category is not None:
            rows = self._conn.execute(
                "SELECT id, content, category, importance, source, created_tick, metadata "
                "FROM long_term_memories WHERE content LIKE ? AND category = ? "
                "ORDER BY importance DESC, created_tick DESC LIMIT ?",
                (f"%{query}%", category, top_k),
            ).fetchall()
        else:
            rows = self._conn.execute(
                "SELECT id, content, category, importance, source, created_tick, metadata "
                "FROM long_term_memories WHERE content LIKE ? "
                "ORDER BY importance DESC, created_tick DESC LIMIT ?",
                (f"%{query}%", top_k),
            ).fetchall()
        return [self._row_to_entry(r) for r in rows]

    def recall(
        self, query: str, *, top_k: int = 5, category: str | None = None, tick: int = 0
    ) -> list[LongTermMemoryEntry]:
        """Retrieve memories — alias for search with same behavior.

        Costs **3 Tokens** per query.
        """
        return self.search(query, top_k=top_k, category=category, tick=tick)

    def get_recent(
        self, *, top_k: int = 10, category: str | None = None
    ) -> list[LongTermMemoryEntry]:
        """Get recent memories, optionally filtered by category."""
        if category is not None:
            rows = self._conn.execute(
                "SELECT id, content, category, importance, source, created_tick, metadata "
                "FROM long_term_memories WHERE category = ? "
                "ORDER BY created_tick DESC LIMIT ?",
                (category, top_k),
            ).fetchall()
        else:
            rows = self._conn.execute(
                "SELECT id, content, category, importance, source, created_tick, metadata "
                "FROM long_term_memories "
                "ORDER BY created_tick DESC LIMIT ?",
                (top_k,),
            ).fetchall()
        return [self._row_to_entry(r) for r in rows]

    def delete(self, memory_id: int) -> bool:
        """Delete a memory entry by ID."""
        cursor = self._conn.execute(
            "DELETE FROM long_term_memories WHERE id = ?", (memory_id,)
        )
        self._conn.commit()
        return cursor.rowcount > 0

    def clear(self) -> None:
        """Remove all entries."""
        self._conn.execute("DELETE FROM long_term_memories")
        self._conn.commit()
        logger.debug("Cleared long-term memory")

    # ------------------------------------------------------------------
    # Properties
    # ------------------------------------------------------------------

    @property
    def token_cost_per_query(self) -> int:
        return _TOKEN_COST_PER_QUERY

    @property
    def max_entries(self) -> int:
        return self._max_entries

    def count(self) -> int:
        return self._count()

    # ------------------------------------------------------------------
    # Dunder helpers
    # ------------------------------------------------------------------

    def __len__(self) -> int:
        return self._count()

    def __repr__(self) -> str:
        return f"LongTermMemory(size={self._count()}/{self._max_entries}, db={self._db_path!r})"

    # ------------------------------------------------------------------
    # Private helpers
    # ------------------------------------------------------------------

    def _count(self) -> int:
        row = self._conn.execute("SELECT COUNT(*) FROM long_term_memories").fetchone()
        return row[0] if row else 0

    def _evict_one(self) -> None:
        """Evict the lowest-importance oldest entry."""
        cursor = self._conn.execute(
            "SELECT id FROM long_term_memories "
            "ORDER BY importance ASC, created_tick ASC LIMIT 1"
        )
        row = cursor.fetchone()
        if row is not None:
            self._conn.execute("DELETE FROM long_term_memories WHERE id = ?", (row[0],))
            self._conn.commit()

    @staticmethod
    def _row_to_entry(row: tuple[Any, ...]) -> LongTermMemoryEntry:
        """Convert a database row to a LongTermMemoryEntry."""
        id_, content, category, importance, source, created_tick, meta_json = row
        metadata: dict[str, Any] = {}
        if meta_json is not None:
            try:
                metadata = json.loads(meta_json)
            except (json.JSONDecodeError, TypeError):
                metadata = {}
        return LongTermMemoryEntry(
            id=id_,
            content=content,
            category=category,
            importance=importance,
            source=source,
            created_tick=created_tick,
            metadata=metadata,
        )

    # ------------------------------------------------------------------
    # Lifecycle
    # ------------------------------------------------------------------

    def close(self) -> None:
        """Close the database connection."""
        self._conn.close()

    def __del__(self) -> None:
        try:
            self._conn.close()
        except Exception:
            pass
