"""Long-term memory — vector DB-backed persistent memory with semantic retrieval.

Stores high-value memories (experiences, lessons, facts) in a vector database
for long-term retention and semantic retrieval.  Built on ChromaDB with an
in-memory fallback when ChromaDB is unavailable.

Memory types:
- **experience**: Things the agent has done or observed (events, outcomes).
- **lesson**: Learned rules or heuristics (what worked, what didn't).
- **fact**: Verified knowledge about the world (properties, relationships).

Memory decay:
- Each memory has an ``importance`` score (0.0–1.0) that decays over time
  according to an exponential decay function.  The ``effective_importance``
  at any tick is ``importance * decay_factor^elapsed_ticks`` where
  ``decay_factor`` is configurable (default 0.999, meaning ~0.5 after 693
  ticks, ~0.37 after 1000 ticks).
- Memories below ``decay_threshold`` (default 0.05) are eligible for cleanup.

Token cost model:
- ``store``: free (writing is a local operation).
- ``search`` / ``recall``: **10 Tokens** per query.

Usage::

    from agent_runtime.memory.embedding import HashEmbeddingProvider
    from agent_runtime.memory.long_term import LongTermMemory

    provider = HashEmbeddingProvider(dimension=128)
    mem = LongTermMemory(embedding_provider=provider)
    mem.store("learned that trading at dawn yields better prices", memory_type="lesson")
    results = mem.search("trading prices", top_k=5)
"""

from __future__ import annotations

import json
import logging
import math
import sqlite3
import time
import uuid
from dataclasses import dataclass
from enum import Enum
from pathlib import Path
from typing import Any, Protocol, runtime_checkable

from agent_runtime.memory.embedding import (
    EmbeddingProviderProtocol,
    HashEmbeddingProvider,
)

logger = logging.getLogger(__name__)

# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------

_DEFAULT_MAX_ENTRIES: int = 10_000
_DEFAULT_DECAY_FACTOR: float = 0.999
_DEFAULT_DECAY_THRESHOLD: float = 0.05
_TOKEN_COST_PER_QUERY: int = 10
_COLLECTION_NAME_PREFIX = "long_term_memory"

# SQLite schema for metadata storage (ChromaDB stores vectors)
_METADATA_SCHEMA = """\
CREATE TABLE IF NOT EXISTS memory_metadata (
    id          TEXT PRIMARY KEY,
    content     TEXT    NOT NULL,
    memory_type TEXT    NOT NULL DEFAULT 'experience',
    importance  REAL    NOT NULL DEFAULT 0.5,
    created_tick INTEGER NOT NULL DEFAULT 0,
    access_count INTEGER NOT NULL DEFAULT 0,
    last_access_tick INTEGER NOT NULL DEFAULT 0,
    tags        TEXT    DEFAULT NULL,
    source      TEXT    DEFAULT NULL,
    created_at  REAL    NOT NULL DEFAULT 0
);
"""


# ---------------------------------------------------------------------------
# Memory types
# ---------------------------------------------------------------------------


class MemoryType(str, Enum):
    """Types of long-term memories."""

    EXPERIENCE = "experience"
    LESSON = "lesson"
    FACT = "fact"


# ---------------------------------------------------------------------------
# Data types
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class LongTermMemoryEntry:
    """A single entry in long-term memory."""

    id: str
    content: str
    memory_type: str
    importance: float
    created_tick: int
    access_count: int
    last_access_tick: int
    tags: list[str] | None = None
    source: str | None = None
    distance: float | None = None

    def __str__(self) -> str:
        tag_str = f" tags={self.tags}" if self.tags else ""
        return (
            f"[{self.id[:8]}] ({self.memory_type}) {self.content} "
            f"(imp={self.importance:.2f}, tick={self.created_tick}{tag_str})"
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
        memory_type: str = "experience",
        importance: float = 0.5,
        tick: int = 0,
        tags: list[str] | None = None,
        source: str | None = None,
    ) -> LongTermMemoryEntry: ...

    def search(
        self,
        query: str,
        *,
        top_k: int = 5,
        tick: int = 0,
        memory_type: str | None = None,
    ) -> list[LongTermMemoryEntry]: ...

    def recall(
        self,
        query: str,
        *,
        top_k: int = 5,
        tick: int = 0,
        memory_type: str | None = None,
        min_importance: float = 0.0,
    ) -> list[LongTermMemoryEntry]: ...

    def delete(self, memory_id: str) -> bool: ...

    def count(self) -> int: ...


# ---------------------------------------------------------------------------
# ChromaDB-backed Long-term Memory
# ---------------------------------------------------------------------------


class LongTermMemory:
    """Vector DB-backed long-term memory with semantic retrieval and decay.

    Uses ChromaDB for vector storage and similarity search, with a SQLite
    sidecar for metadata that ChromaDB doesn't natively handle well
    (decay tracking, access counts, type filtering).

    Falls back to an in-memory implementation when ChromaDB is unavailable.

    Parameters
    ----------
    embedding_provider : EmbeddingProviderProtocol | None
        Provider for generating embeddings.  Defaults to
        ``HashEmbeddingProvider(dimension=128)``.
    db_path : str | Path
        Path for the SQLite metadata database.  Use ``":memory:"`` for
        in-memory storage.
    persist_dir : str | Path | None
        Directory for ChromaDB persistence.  If None, ChromaDB runs in-memory.
    max_entries : int
        Maximum number of entries.  Default 10,000.
    decay_factor : float
        Exponential decay rate per tick.  Default 0.999.
    decay_threshold : float
        Importance threshold below which memories are cleaned up.  Default 0.05.
    """

    def __init__(
        self,
        embedding_provider: EmbeddingProviderProtocol | None = None,
        db_path: str | Path = ":memory:",
        persist_dir: str | Path | None = None,
        max_entries: int = _DEFAULT_MAX_ENTRIES,
        decay_factor: float = _DEFAULT_DECAY_FACTOR,
        decay_threshold: float = _DEFAULT_DECAY_THRESHOLD,
    ) -> None:
        if max_entries <= 0:
            raise ValueError("max_entries must be a positive integer")
        if not 0.0 < decay_factor <= 1.0:
            raise ValueError("decay_factor must be in (0.0, 1.0]")
        if not 0.0 <= decay_threshold < 1.0:
            raise ValueError("decay_threshold must be in [0.0, 1.0)")

        self._provider = embedding_provider or HashEmbeddingProvider()
        self._max_entries = max_entries
        self._decay_factor = decay_factor
        self._decay_threshold = decay_threshold
        self._collection_name = f"{_COLLECTION_NAME_PREFIX}_{self._provider.dimension}"

        # SQLite metadata store
        self._db_path = str(db_path)
        self._conn = sqlite3.connect(self._db_path)
        self._conn.execute("PRAGMA journal_mode=WAL")
        self._conn.execute(_METADATA_SCHEMA)
        self._conn.commit()

        # ChromaDB vector store
        self._chroma = None
        self._collection = None
        self._use_fallback = False
        self._fallback_store: dict[str, dict[str, Any]] = {}

        try:
            import chromadb

            if persist_dir:
                self._chroma = chromadb.PersistentClient(path=str(persist_dir))
            else:
                self._chroma = chromadb.Client()

            self._collection = self._chroma.get_or_create_collection(
                name=self._collection_name,
                metadata={"hnsw:space": "cosine"},
            )
            logger.debug("ChromaDB vector store initialized (persist_dir=%s)", persist_dir)
        except ImportError:
            logger.warning(
                "ChromaDB not available, falling back to in-memory vector search"
            )
            self._use_fallback = True
        except Exception as exc:
            logger.warning("ChromaDB initialization failed: %s, using fallback", exc)
            self._use_fallback = True

    # ------------------------------------------------------------------
    # Core API
    # ------------------------------------------------------------------

    def store(
        self,
        content: str,
        *,
        memory_type: str = "experience",
        importance: float = 0.5,
        tick: int = 0,
        tags: list[str] | None = None,
        source: str | None = None,
    ) -> LongTermMemoryEntry:
        """Store a new long-term memory.

        Parameters
        ----------
        content : str
            The memory text to store.
        memory_type : str
            One of ``experience``, ``lesson``, ``fact``.
        importance : float
            Importance score between 0.0 and 1.0.
        tick : int
            The simulation tick at which this memory was created.
        tags : list[str] | None
            Optional tags for categorization.
        source : str | None
            Optional source identifier (e.g., agent name, event type).

        Returns
        -------
        LongTermMemoryEntry
            The newly created entry.
        """
        memory_id = str(uuid.uuid4())
        importance = max(0.0, min(1.0, importance))

        # Validate memory_type
        if memory_type not in ("experience", "lesson", "fact"):
            raise ValueError(
                f"memory_type must be 'experience', 'lesson', or 'fact', got {memory_type!r}"
            )

        # Generate embedding
        embedding = self._provider.embed(content)

        # Evict if at capacity
        if self._count() >= self._max_entries:
            self._evict_one(tick)

        # Store in ChromaDB (or fallback)
        if self._use_fallback:
            self._fallback_store[memory_id] = {
                "embedding": embedding,
                "content": content,
            }
        else:
            self._collection.add(
                ids=[memory_id],
                embeddings=[embedding],
                documents=[content],
                metadatas=[{"memory_type": memory_type, "importance": importance}],
            )

        # Store metadata in SQLite
        tags_json = json.dumps(tags) if tags else None
        self._conn.execute(
            "INSERT INTO memory_metadata "
            "(id, content, memory_type, importance, created_tick, access_count, "
            "last_access_tick, tags, source, created_at) "
            "VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            (
                memory_id,
                content,
                memory_type,
                importance,
                tick,
                0,
                tick,
                tags_json,
                source,
                time.time(),
            ),
        )
        self._conn.commit()

        entry = LongTermMemoryEntry(
            id=memory_id,
            content=content,
            memory_type=memory_type,
            importance=importance,
            created_tick=tick,
            access_count=0,
            last_access_tick=tick,
            tags=tags,
            source=source,
        )
        logger.debug(
            "Stored long-term memory: %s (type=%s, imp=%.2f, tick=%d)",
            content[:50],
            memory_type,
            importance,
            tick,
        )
        return entry

    def search(
        self,
        query: str,
        *,
        top_k: int = 5,
        tick: int = 0,
        memory_type: str | None = None,
        _track_access: bool = True,
    ) -> list[LongTermMemoryEntry]:
        """Search memories by semantic similarity.

        Costs **10 Tokens** per query.

        Parameters
        ----------
        query : str
            Search query text.
        top_k : int
            Maximum number of results to return.
        tick : int
            Current tick (used for decay computation).
        memory_type : str | None
            Filter to a specific memory type (experience/lesson/fact).

        Returns
        -------
        list[LongTermMemoryEntry]
            Top-k matching entries ordered by relevance.
        """
        query_embedding = self._provider.embed(query)

        if self._use_fallback:
            return self._search_fallback(
                query_embedding, top_k, tick, memory_type, _track_access
            )

        # ChromaDB query
        where_filter = None
        if memory_type:
            where_filter = {"memory_type": memory_type}

        results = self._collection.query(
            query_embeddings=[query_embedding],
            n_results=min(top_k * 2, self._max_entries),  # fetch extra for decay filtering
            where=where_filter,
            include=["documents", "metadatas", "distances"],
        )

        if not results or not results["ids"] or not results["ids"][0]:
            return []

        # Build entries from ChromaDB results
        entries: list[LongTermMemoryEntry] = []
        for i, mid in enumerate(results["ids"][0]):
            # Look up full metadata from SQLite
            row = self._conn.execute(
                "SELECT id, content, memory_type, importance, created_tick, "
                "access_count, last_access_tick, tags, source "
                "FROM memory_metadata WHERE id = ?",
                (mid,),
            ).fetchone()
            if row is None:
                continue

            entry = self._row_to_entry(row)
            distance = results["distances"][0][i] if results["distances"] else None

            # Compute effective importance with decay
            effective = self._compute_effective_importance(entry, tick)
            if effective < self._decay_threshold:
                continue

            entries.append(
                LongTermMemoryEntry(
                    id=entry.id,
                    content=entry.content,
                    memory_type=entry.memory_type,
                    importance=effective,
                    created_tick=entry.created_tick,
                    access_count=entry.access_count,
                    last_access_tick=entry.last_access_tick,
                    tags=entry.tags,
                    source=entry.source,
                    distance=distance,
                )
            )

        # Update access counts
        if _track_access:
            self._increment_access([e.id for e in entries[:top_k]], tick)

        return entries[:top_k]

    def recall(
        self,
        query: str,
        *,
        top_k: int = 5,
        tick: int = 0,
        memory_type: str | None = None,
        min_importance: float = 0.0,
    ) -> list[LongTermMemoryEntry]:
        """Retrieve memories using semantic search with decay-adjusted importance.

        Costs **10 Tokens** per query.

        Like ``search`` but additionally:
        - Filters results by minimum effective importance.
        - Boosts frequently accessed memories.
        - Applies decay-aware ranking.

        Parameters
        ----------
        query : str
            Search query text.
        top_k : int
            Maximum number of results to return.
        tick : int
            Current tick (used for decay computation).
        memory_type : str | None
            Filter to a specific memory type.
        min_importance : float
            Minimum effective importance for results.  Default 0.0.

        Returns
        -------
        list[LongTermMemoryEntry]
            Top-k matching entries ordered by decay-adjusted relevance.
        """
        results = self.search(
            query, top_k=top_k * 2, tick=tick, memory_type=memory_type,
            _track_access=False,
        )

        # Filter by minimum importance
        filtered = [e for e in results if e.importance >= min_importance]

        # Re-rank by decay-adjusted score: distance + importance + access bonus
        def rank_score(entry: LongTermMemoryEntry) -> float:
            # Distance is cosine distance (lower is better), invert for scoring
            dist_score = 1.0 - (entry.distance or 0.5)
            # Importance (already decay-adjusted)
            imp_score = entry.importance
            # Access frequency bonus (diminishing returns)
            access_bonus = math.log1p(entry.access_count) * 0.05
            return dist_score + imp_score + access_bonus

        filtered.sort(key=rank_score, reverse=True)
        final = filtered[:top_k]
        self._increment_access([e.id for e in final], tick)
        return final

    def delete(self, memory_id: str) -> bool:
        """Delete a memory entry by ID.

        Returns True if a row was deleted, False otherwise.
        """
        # Delete from vector store
        if self._use_fallback:
            self._fallback_store.pop(memory_id, None)
        else:
            try:
                self._collection.delete(ids=[memory_id])
            except Exception:
                pass

        # Delete from metadata
        cursor = self._conn.execute(
            "DELETE FROM memory_metadata WHERE id = ?", (memory_id,)
        )
        self._conn.commit()
        return cursor.rowcount > 0

    def clear(self) -> None:
        """Remove all entries."""
        # Clear vector store
        if self._use_fallback:
            self._fallback_store.clear()
        else:
            try:
                self._chroma.delete_collection(self._collection_name)
                self._collection = self._chroma.get_or_create_collection(
                    name=self._collection_name,
                    metadata={"hnsw:space": "cosine"},
                )
            except Exception:
                self._fallback_store.clear()
                self._use_fallback = True

        # Clear metadata
        self._conn.execute("DELETE FROM memory_metadata")
        self._conn.commit()
        logger.debug("Cleared long-term memory")

    def decay(self, current_tick: int) -> int:
        """Run memory decay: remove memories below the decay threshold.

        Parameters
        ----------
        current_tick : int
            The current simulation tick.

        Returns
        -------
        int
            Number of memories removed.
        """
        # Get all memories
        rows = self._conn.execute(
            "SELECT id, importance, created_tick FROM memory_metadata"
        ).fetchall()

        ids_to_remove: list[str] = []
        for row in rows:
            mid, importance, created_tick = row
            elapsed = current_tick - created_tick
            effective = importance * (self._decay_factor ** elapsed)
            if effective < self._decay_threshold:
                ids_to_remove.append(mid)

        for mid in ids_to_remove:
            self.delete(mid)

        if ids_to_remove:
            logger.debug(
                "Decay removed %d memories at tick %d", len(ids_to_remove), current_tick
            )
        return len(ids_to_remove)

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
    def decay_factor(self) -> float:
        return self._decay_factor

    @property
    def decay_threshold(self) -> float:
        return self._decay_threshold

    @property
    def uses_chroma(self) -> bool:
        """Whether ChromaDB is being used (vs. in-memory fallback)."""
        return not self._use_fallback

    def count(self) -> int:
        """Return the current number of stored memories."""
        return self._count()

    # ------------------------------------------------------------------
    # Dunder helpers
    # ------------------------------------------------------------------

    def __len__(self) -> int:
        return self._count()

    def __repr__(self) -> str:
        backend = "chromadb" if not self._use_fallback else "fallback"
        return (
            f"LongTermMemory(size={self._count()}/{self._max_entries}, "
            f"backend={backend}, decay={self._decay_factor})"
        )

    # ------------------------------------------------------------------
    # Private helpers
    # ------------------------------------------------------------------

    def _count(self) -> int:
        row = self._conn.execute("SELECT COUNT(*) FROM memory_metadata").fetchone()
        return row[0] if row else 0

    def _compute_effective_importance(
        self, entry: LongTermMemoryEntry, current_tick: int
    ) -> float:
        """Compute importance after exponential decay."""
        elapsed = current_tick - entry.created_tick
        effective = entry.importance * (self._decay_factor ** elapsed)
        return max(0.0, effective)

    def _increment_access(self, ids: list[str], tick: int) -> None:
        """Increment access count and update last_access_tick."""
        if not ids:
            return
        for mid in ids:
            self._conn.execute(
                "UPDATE memory_metadata SET access_count = access_count + 1, "
                "last_access_tick = ? WHERE id = ?",
                (tick, mid),
            )
        self._conn.commit()

    def _evict_one(self, current_tick: int) -> None:
        """Evict the lowest effective-importance memory."""
        rows = self._conn.execute(
            "SELECT id, importance, created_tick FROM memory_metadata"
        ).fetchall()

        if not rows:
            return

        # Find the memory with the lowest effective importance
        worst_id: str | None = None
        worst_score = float("inf")
        for mid, importance, created_tick in rows:
            elapsed = current_tick - created_tick
            effective = importance * (self._decay_factor ** elapsed)
            if effective < worst_score:
                worst_score = effective
                worst_id = mid

        if worst_id:
            self.delete(worst_id)

    def _search_fallback(
        self,
        query_embedding: list[float],
        top_k: int,
        tick: int,
        memory_type: str | None,
        _track_access: bool = True,
    ) -> list[LongTermMemoryEntry]:
        """In-memory fallback search using cosine similarity."""
        rows = self._conn.execute(
            "SELECT id, content, memory_type, importance, created_tick, "
            "access_count, last_access_tick, tags, source "
            "FROM memory_metadata"
        ).fetchall()

        if not rows:
            return []

        scored: list[tuple[float, LongTermMemoryEntry]] = []
        for row in rows:
            entry = self._row_to_entry(row)

            # Filter by memory_type
            if memory_type and entry.memory_type != memory_type:
                continue

            # Check decay
            effective = self._compute_effective_importance(entry, tick)
            if effective < self._decay_threshold:
                continue

            # Compute cosine similarity with stored embedding
            stored_data = self._fallback_store.get(entry.id)
            if stored_data and "embedding" in stored_data:
                sim = _cosine_similarity(query_embedding, stored_data["embedding"])
            else:
                # Re-embed content if embedding not in fallback store
                embedding = self._provider.embed(entry.content)
                sim = _cosine_similarity(query_embedding, embedding)

            distance = 1.0 - sim
            scored.append(
                (
                    distance,
                    LongTermMemoryEntry(
                        id=entry.id,
                        content=entry.content,
                        memory_type=entry.memory_type,
                        importance=effective,
                        created_tick=entry.created_tick,
                        access_count=entry.access_count,
                        last_access_tick=entry.last_access_tick,
                        tags=entry.tags,
                        source=entry.source,
                        distance=distance,
                    ),
                )
            )

        scored.sort(key=lambda x: x[0])
        entries = [e for _, e in scored[:top_k]]

        # Update access counts
        if _track_access:
            self._increment_access([e.id for e in entries], tick)

        return entries

    @staticmethod
    def _row_to_entry(row: tuple[Any, ...]) -> LongTermMemoryEntry:
        """Convert a database row to a LongTermMemoryEntry."""
        (
            id_,
            content,
            memory_type,
            importance,
            created_tick,
            access_count,
            last_access_tick,
            tags_str,
            source,
        ) = row
        tags: list[str] | None = None
        if tags_str:
            tags = json.loads(tags_str)
        return LongTermMemoryEntry(
            id=id_,
            content=content,
            memory_type=memory_type,
            importance=importance,
            created_tick=created_tick,
            access_count=access_count,
            last_access_tick=last_access_tick,
            tags=tags,
            source=source,
        )

    # ------------------------------------------------------------------
    # Lifecycle
    # ------------------------------------------------------------------

    def close(self) -> None:
        """Close database connections."""
        self._conn.close()

    def __enter__(self) -> LongTermMemory:
        return self

    def __exit__(self, *args: Any) -> None:
        self.close()

    def __del__(self) -> None:
        try:
            self._conn.close()
        except Exception:
            pass


# ---------------------------------------------------------------------------
# Utility functions
# ---------------------------------------------------------------------------


def _cosine_similarity(a: list[float], b: list[float]) -> float:
    """Compute cosine similarity between two vectors."""
    if len(a) != len(b):
        raise ValueError(f"Vector dimensions don't match: {len(a)} vs {len(b)}")
    dot = sum(x * y for x, y in zip(a, b))
    norm_a = math.sqrt(sum(x * x for x in a))
    norm_b = math.sqrt(sum(x * x for x in b))
    if norm_a == 0.0 or norm_b == 0.0:
        return 0.0
    return dot / (norm_a * norm_b)
