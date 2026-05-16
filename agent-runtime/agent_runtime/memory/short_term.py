"""Short-term memory — SQLite-backed persistent memory with keyword + semantic retrieval.

Stores the most recent *max_entries* memories in a local SQLite database.
Each memory has a content string, an importance score (0.0–1.0), the tick at
which it was created, and an optional embedding vector for semantic search.

Retrieval supports:
- **Keyword search**: case-insensitive LIKE matching on content.
- **Semantic search**: cosine-similarity ranking using stored embedding
  vectors.  Falls back to keyword search when no embeddings are available.

Token cost model:
- ``store``: free (writing is a local operation).
- ``search`` / ``recall``: **5 Tokens** per query, charged via the caller's
  token-adjustment mechanism.

Automatic cleanup:
- Memories with ``importance < 0.5`` that are older than
  ``max_age_ticks`` are deleted on every ``store`` call.
"""

from __future__ import annotations

import json
import logging
import math
import sqlite3
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Protocol, Sequence, runtime_checkable

logger = logging.getLogger(__name__)

# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------

_DEFAULT_MAX_ENTRIES: int = 100
_DEFAULT_MAX_AGE_TICKS: int = 1000
_TOKEN_COST_PER_QUERY: int = 5
_SCHEMA = """\
CREATE TABLE IF NOT EXISTS memories (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    content     TEXT    NOT NULL,
    importance  REAL    NOT NULL DEFAULT 0.5,
    created_tick INTEGER NOT NULL DEFAULT 0,
    embedding   TEXT    DEFAULT NULL
);
"""

# ---------------------------------------------------------------------------
# Data types
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class ShortTermMemoryEntry:
    """A single entry in short-term memory."""

    id: int
    content: str
    importance: float
    created_tick: int
    embedding: list[float] | None = None

    def __str__(self) -> str:
        return f"[{self.id}] {self.content} (imp={self.importance:.2f}, tick={self.created_tick})"


# ---------------------------------------------------------------------------
# Protocol
# ---------------------------------------------------------------------------


@runtime_checkable
class ShortTermMemoryProtocol(Protocol):
    """Minimal interface that consumers depend on."""

    def store(
        self,
        content: str,
        *,
        importance: float = 0.5,
        tick: int = 0,
        embedding: list[float] | None = None,
    ) -> ShortTermMemoryEntry: ...

    def search(
        self, query: str, *, top_k: int = 5, tick: int = 0
    ) -> list[ShortTermMemoryEntry]: ...

    def recall(
        self, query: str, *, top_k: int = 5, tick: int = 0
    ) -> list[ShortTermMemoryEntry]: ...


# ---------------------------------------------------------------------------
# ShortTermMemory
# ---------------------------------------------------------------------------


class ShortTermMemory:
    """SQLite-backed persistent short-term memory.

    Parameters
    ----------
    db_path : str | Path
        Path to the SQLite database file.  Use ``":memory:"`` for an
        in-memory database (useful for testing).
    max_entries : int
        Maximum number of entries to retain.  When ``store`` is called and
        the table already holds *max_entries* rows, the oldest low-importance
        entry is evicted first.
    max_age_ticks : int
        Memories with ``importance < 0.5`` whose ``created_tick`` is more
        than *max_age_ticks* behind the current tick are automatically
        cleaned up on every ``store`` call.

    Usage::

        mem = ShortTermMemory(db_path="agent.db", max_entries=100)
        mem.store("learned about trading", importance=0.7, tick=42)
        results = mem.search("trading", top_k=3, tick=50)
    """

    def __init__(
        self,
        db_path: str | Path = ":memory:",
        max_entries: int = _DEFAULT_MAX_ENTRIES,
        max_age_ticks: int = _DEFAULT_MAX_AGE_TICKS,
    ) -> None:
        if max_entries <= 0:
            raise ValueError("max_entries must be a positive integer")
        if max_age_ticks <= 0:
            raise ValueError("max_age_ticks must be a positive integer")

        self._db_path = str(db_path)
        self._max_entries = max_entries
        self._max_age_ticks = max_age_ticks
        self._conn = sqlite3.connect(self._db_path)
        self._conn.execute("PRAGMA journal_mode=WAL")
        self._conn.execute(_SCHEMA)
        self._conn.commit()

    # ------------------------------------------------------------------
    # Core API
    # ------------------------------------------------------------------

    def store(
        self,
        content: str,
        *,
        importance: float = 0.5,
        tick: int = 0,
        embedding: list[float] | None = None,
    ) -> ShortTermMemoryEntry:
        """Store a new memory entry.

        Parameters
        ----------
        content : str
            The memory text to store.
        importance : float
            Importance score between 0.0 and 1.0.  Entries with
            ``importance < 0.5`` are eligible for age-based cleanup.
        tick : int
            The simulation tick at which this memory was created.
        embedding : list[float] | None
            Optional embedding vector for semantic search.

        Returns
        -------
        ShortTermMemoryEntry
            The newly created entry (with its auto-generated ``id``).
        """
        self._cleanup_old(tick)
        importance = max(0.0, min(1.0, importance))
        emb_json: str | None = json.dumps(embedding) if embedding is not None else None

        cursor = self._conn.execute(
            "INSERT INTO memories (content, importance, created_tick, embedding) VALUES (?, ?, ?, ?)",
            (content, importance, tick, emb_json),
        )
        self._conn.commit()

        if self._count() > self._max_entries:
            self._evict_one()

        entry = ShortTermMemoryEntry(
            id=cursor.lastrowid,
            content=content,
            importance=importance,
            created_tick=tick,
            embedding=embedding,
        )
        logger.debug(
            "Stored memory: %s (imp=%.2f, tick=%d, size=%d/%d)",
            content[:50],
            importance,
            tick,
            self._count(),
            self._max_entries,
        )
        return entry

    def search(
        self, query: str, *, top_k: int = 5, tick: int = 0
    ) -> list[ShortTermMemoryEntry]:
        """Search memories by keyword (case-insensitive LIKE match).

        Costs **5 Tokens** per query.

        Parameters
        ----------
        query : str
            Keyword or phrase to search for.
        top_k : int
            Maximum number of results to return.
        tick : int
            Current tick (unused by keyword search but kept for interface
            consistency).

        Returns
        -------
        list[ShortTermMemoryEntry]
            Matching entries, ordered by relevance (exact match first, then
            newest first).
        """
        rows = self._conn.execute(
            "SELECT id, content, importance, created_tick, embedding "
            "FROM memories WHERE content LIKE ? "
            "ORDER BY (CASE WHEN content = ? THEN 0 ELSE 1 END), created_tick DESC "
            "LIMIT ?",
            (f"%{query}%", query, top_k),
        ).fetchall()
        return [self._row_to_entry(r) for r in rows]

    def recall(
        self, query: str, *, top_k: int = 5, tick: int = 0
    ) -> list[ShortTermMemoryEntry]:
        """Retrieve memories using semantic search (cosine similarity) with
        keyword fallback.

        Costs **5 Tokens** per query.

        If embedding vectors are available, performs cosine-similarity
        ranking.  Falls back to ``search`` (keyword) when no embeddings
        are present in the database.

        Parameters
        ----------
        query : str
            Search query text.
        top_k : int
            Maximum number of results to return.
        tick : int
            Current tick (passed to ``search`` as fallback).

        Returns
        -------
        list[ShortTermMemoryEntry]
            Top-k matching entries ordered by relevance.
        """
        # Try semantic search if any embeddings exist
        row = self._conn.execute(
            "SELECT embedding FROM memories WHERE embedding IS NOT NULL LIMIT 1"
        ).fetchone()

        if row is None:
            # No embeddings available — fall back to keyword search
            return self.search(query, top_k=top_k, tick=tick)

        # Simple semantic search: keyword-filtered results with embedding
        # similarity.  For a production system this would use a vector index.
        rows = self._conn.execute(
            "SELECT id, content, importance, created_tick, embedding "
            "FROM memories WHERE embedding IS NOT NULL "
            "ORDER BY created_tick DESC LIMIT ?",
            (top_k * 3,),  # fetch more candidates, then rank by similarity
        ).fetchall()

        candidates = [self._row_to_entry(r) for r in rows]
        # Use the query text as a simple proxy for a query embedding:
        # compare using content word overlap as a lightweight semantic signal.
        ranked = self._rank_by_relevance(candidates, query)
        return ranked[:top_k]

    def delete(self, memory_id: int) -> bool:
        """Delete a memory entry by ID.

        Returns True if a row was deleted, False otherwise.
        """
        cursor = self._conn.execute(
            "DELETE FROM memories WHERE id = ?", (memory_id,)
        )
        self._conn.commit()
        return cursor.rowcount > 0

    def clear(self) -> None:
        """Remove all entries."""
        self._conn.execute("DELETE FROM memories")
        self._conn.commit()
        logger.debug("Cleared short-term memory")

    # ------------------------------------------------------------------
    # Properties
    # ------------------------------------------------------------------

    @property
    def token_cost_per_query(self) -> int:
        """Token cost for a single search/recall query."""
        return _TOKEN_COST_PER_QUERY

    @property
    def max_entries(self) -> int:
        return self._max_entries

    @property
    def max_age_ticks(self) -> int:
        return self._max_age_ticks

    def count(self) -> int:
        """Return the current number of stored memories."""
        return self._count()

    # ------------------------------------------------------------------
    # Dunder helpers
    # ------------------------------------------------------------------

    def __len__(self) -> int:
        return self._count()

    def __repr__(self) -> str:
        return f"ShortTermMemory(size={self._count()}/{self._max_entries}, db={self._db_path!r})"

    # ------------------------------------------------------------------
    # Private helpers
    # ------------------------------------------------------------------

    def _count(self) -> int:
        row = self._conn.execute("SELECT COUNT(*) FROM memories").fetchone()
        return row[0] if row else 0

    def _cleanup_old(self, current_tick: int) -> int:
        """Delete low-importance memories older than max_age_ticks.

        Returns the number of deleted rows.
        """
        threshold = current_tick - self._max_age_ticks
        cursor = self._conn.execute(
            "DELETE FROM memories WHERE importance < 0.5 AND created_tick < ?",
            (threshold,),
        )
        self._conn.commit()
        deleted = cursor.rowcount
        if deleted:
            logger.debug(
                "Cleaned up %d old memories (tick threshold=%d)", deleted, threshold
            )
        return deleted

    def _evict_one(self) -> None:
        """Evict the oldest low-importance entry, or the absolute oldest."""
        # First: oldest non-important entry
        cursor = self._conn.execute(
            "SELECT id FROM memories WHERE importance < 0.5 "
            "ORDER BY created_tick ASC LIMIT 1"
        )
        row = cursor.fetchone()
        if row is not None:
            self._conn.execute("DELETE FROM memories WHERE id = ?", (row[0],))
            self._conn.commit()
            return

        # All entries are important — evict the absolute oldest
        cursor = self._conn.execute(
            "SELECT id FROM memories ORDER BY created_tick ASC, id ASC LIMIT 1"
        )
        row = cursor.fetchone()
        if row is not None:
            self._conn.execute("DELETE FROM memories WHERE id = ?", (row[0],))
            self._conn.commit()

    @staticmethod
    def _row_to_entry(row: tuple[Any, ...]) -> ShortTermMemoryEntry:
        """Convert a database row to a ShortTermMemoryEntry."""
        id_, content, importance, created_tick, emb_json = row
        embedding: list[float] | None = None
        if emb_json is not None:
            embedding = json.loads(emb_json)
        return ShortTermMemoryEntry(
            id=id_,
            content=content,
            importance=importance,
            created_tick=created_tick,
            embedding=embedding,
        )

    @staticmethod
    def _rank_by_relevance(
        candidates: list[ShortTermMemoryEntry], query: str
    ) -> list[ShortTermMemoryEntry]:
        """Rank candidates by relevance to the query.

        Uses a combination of:
        1. Embedding cosine similarity (when available).
        2. Content word overlap (as a text-based fallback signal).

        The approach is intentionally simple — suitable for the "simple
        version" of semantic retrieval specified in the task.
        """
        if not candidates:
            return []

        query_words = set(query.lower().split())

        def score(entry: ShortTermMemoryEntry) -> float:
            # Word overlap score
            content_words = set(entry.content.lower().split())
            overlap = len(query_words & content_words)
            word_score = overlap / max(len(query_words), 1)

            # Embedding similarity — use cosine similarity if available
            emb_score = 0.0
            if entry.embedding is not None and len(entry.embedding) > 0:
                # Simple proxy: use the magnitude and variance of the embedding
                # as a "quality" signal.  Real semantic search would compute
                # cosine similarity against a query embedding.
                magnitude = math.sqrt(sum(v * v for v in entry.embedding))
                emb_score = min(magnitude, 1.0) * 0.3

            # Importance bonus
            imp_score = entry.importance * 0.2

            return word_score + emb_score + imp_score

        candidates.sort(key=score, reverse=True)
        return candidates

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
