"""Vector-backed long-term memory — SQLite + numpy cosine similarity with decay.

Stores agent experiences, lessons, and facts with embedding vectors for
semantic retrieval.  Uses plain SQLite for storage and numpy for vector
operations — zero deployment cost, no external vector DB required.

Retrieval features:
- **Semantic search**: top-k nearest neighbours by cosine similarity.
- **Forgetting curve**: memories not accessed for 30 days ( configurable )
  have their weight decayed by 50% so they rank lower in results.
- **Memory types**: ``experience``, ``lesson``, ``fact`` — each stored with
  its own category tag for filtered retrieval.

Embedding is pluggable via ``EmbeddingProviderProtocol`` so the default
``HashEmbeddingProvider`` works for testing while ``SentenceTransformer``
can be swapped in for production.

Token cost model:
- ``store``: free (writing is a local operation).
- ``recall``: **3 Tokens** per query (same as LongTermMemory).
"""

from __future__ import annotations

import json
import logging
import sqlite3
from dataclasses import dataclass, field
from datetime import datetime, timezone
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

_DEFAULT_MAX_ENTRIES: int = 1000
_TOKEN_COST_PER_QUERY: int = 3
_DEFAULT_DECAY_DAYS: float = 30.0  # days until weight decays by 50%
_DECAY_HALF_LIFE: float = 30.0  # Ebbinghaus-style half-life in days

_SCHEMA = """\
CREATE TABLE IF NOT EXISTS vector_memories (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    content         TEXT    NOT NULL,
    memory_type     TEXT    NOT NULL DEFAULT 'experience',
    importance      REAL    NOT NULL DEFAULT 0.7,
    embedding       BLOB    NOT NULL,
    created_at      TEXT    NOT NULL,
    last_accessed   TEXT    NOT NULL,
    access_count    INTEGER NOT NULL DEFAULT 0,
    source          TEXT    DEFAULT NULL,
    metadata        TEXT    DEFAULT NULL
);
CREATE INDEX IF NOT EXISTS idx_vm_type ON vector_memories(memory_type);
CREATE INDEX IF NOT EXISTS idx_vm_importance ON vector_memories(importance);
"""


# ---------------------------------------------------------------------------
# Enums
# ---------------------------------------------------------------------------


class MemoryType(str, Enum):
    """Types of long-term vector memories."""

    EXPERIENCE = "experience"  # Things the agent has done / observed
    LESSON = "lesson"  # Lessons learned (what to do / avoid)
    FACT = "fact"  # Factual knowledge about the world


# ---------------------------------------------------------------------------
# Data types
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class VectorMemoryEntry:
    """A single entry in vector-backed memory."""

    id: int
    content: str
    memory_type: str  # "experience", "lesson", "fact"
    importance: float
    embedding: list[float] | None = None
    created_at: str = ""
    last_accessed: str = ""
    access_count: int = 0
    source: str | None = None
    metadata: dict[str, Any] = field(default_factory=dict)

    def __str__(self) -> str:
        return (
            f"[{self.id}] {self.content} "
            f"(type={self.memory_type}, imp={self.importance:.2f}, "
            f"accessed={self.access_count}x)"
        )


# ---------------------------------------------------------------------------
# Protocol
# ---------------------------------------------------------------------------


@runtime_checkable
class VectorMemoryProtocol(Protocol):
    """Minimal interface that consumers depend on."""

    def store(
        self,
        content: str,
        *,
        memory_type: str = "experience",
        importance: float = 0.7,
        source: str | None = None,
        metadata: dict[str, Any] | None = None,
    ) -> VectorMemoryEntry: ...

    def recall(
        self,
        query: str,
        *,
        top_k: int = 5,
        memory_type: str | None = None,
        min_relevance: float = 0.0,
    ) -> list[tuple[VectorMemoryEntry, float]]: ...

    def recall_with_decay(
        self,
        query: str,
        *,
        top_k: int = 5,
        memory_type: str | None = None,
    ) -> list[tuple[VectorMemoryEntry, float]]: ...


# ---------------------------------------------------------------------------
# VectorMemory
# ---------------------------------------------------------------------------


class VectorMemory:
    """SQLite + numpy vector-backed long-term memory with forgetting curve.

    Parameters
    ----------
    db_path : str | Path
        Path to the SQLite database file.  Use ``":memory:"`` for an
        in-memory database (useful for testing).
    embedding_provider : EmbeddingProviderProtocol | None
        Provider for generating embedding vectors.  Defaults to
        ``HashEmbeddingProvider(dimension=128)``.
    max_entries : int
        Maximum number of entries to retain.  When ``store`` is called and
        the table already holds *max_entries* rows, the lowest-importance
        oldest entry is evicted first.
    decay_days : float
        Number of days after which an unaccessed memory's weight decays by
        50% (Ebbinghaus forgetting curve).  Default 30 days.

    Usage::

        from agent_runtime.memory.vector_memory import VectorMemory

        vm = VectorMemory()
        vm.store("Trading during crisis is risky", memory_type="lesson",
                 importance=0.9)
        results = vm.recall("trading risk", top_k=3)
    """

    def __init__(
        self,
        db_path: str | Path = ":memory:",
        embedding_provider: EmbeddingProviderProtocol | None = None,
        max_entries: int = _DEFAULT_MAX_ENTRIES,
        decay_days: float = _DEFAULT_DECAY_DAYS,
    ) -> None:
        if max_entries <= 0:
            raise ValueError("max_entries must be a positive integer")
        if decay_days <= 0:
            raise ValueError("decay_days must be positive")

        self._db_path = str(db_path)
        self._max_entries = max_entries
        self._decay_days = decay_days
        self._provider = embedding_provider or HashEmbeddingProvider(dimension=128)
        self._dimension = self._provider.dimension

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
        memory_type: str = "experience",
        importance: float = 0.7,
        source: str | None = None,
        metadata: dict[str, Any] | None = None,
    ) -> VectorMemoryEntry:
        """Store a new vector memory entry.

        Parameters
        ----------
        content : str
            The memory text to store.
        memory_type : str
            Type of memory: "experience", "lesson", or "fact".
        importance : float
            Importance score between 0.0 and 1.0.
        source : str | None
            Origin of this memory.
        metadata : dict | None
            Optional arbitrary metadata.

        Returns
        -------
        VectorMemoryEntry
            The newly created entry.
        """
        importance = max(0.0, min(1.0, importance))
        embedding = self._provider.embed(content)
        emb_blob = _floats_to_blob(embedding)
        now = _now_iso()
        meta_json: str | None = json.dumps(metadata) if metadata is not None else None

        cursor = self._conn.execute(
            "INSERT INTO vector_memories "
            "(content, memory_type, importance, embedding, created_at, "
            "last_accessed, access_count, source, metadata) "
            "VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            (content, memory_type, importance, emb_blob, now, now, 0, source, meta_json),
        )
        self._conn.commit()

        if self._count() > self._max_entries:
            self._evict_one()

        entry = VectorMemoryEntry(
            id=cursor.lastrowid,
            content=content,
            memory_type=memory_type,
            importance=importance,
            embedding=embedding,
            created_at=now,
            last_accessed=now,
            access_count=0,
            source=source,
            metadata=metadata or {},
        )
        logger.debug(
            "Stored vector memory: %s (type=%s, imp=%.2f)",
            content[:50],
            memory_type,
            importance,
        )
        return entry

    def recall(
        self,
        query: str,
        *,
        top_k: int = 5,
        memory_type: str | None = None,
        min_relevance: float = 0.0,
    ) -> list[tuple[VectorMemoryEntry, float]]:
        """Retrieve memories by semantic similarity (cosine similarity).

        Costs **3 Tokens** per query.

        Parameters
        ----------
        query : str
            The search query text.
        top_k : int
            Maximum number of results to return.
        memory_type : str | None
            Filter by memory type if specified.
        min_relevance : float
            Minimum relevance score (0.0-1.0) to include in results.

        Returns
        -------
        list[tuple[VectorMemoryEntry, float]]
            Matching entries with their similarity scores, ordered by
            relevance descending.
        """
        query_embedding = self._provider.embed(query)
        return self._search_by_similarity(
            query_embedding, top_k=top_k, memory_type=memory_type,
            min_relevance=min_relevance,
        )

    def recall_with_decay(
        self,
        query: str,
        *,
        top_k: int = 5,
        memory_type: str | None = None,
    ) -> list[tuple[VectorMemoryEntry, float]]:
        """Retrieve memories with forgetting-curve decay applied.

        Like ``recall`` but applies an Ebbinghaus-style forgetting curve:
        memories not accessed for 30 days have their effective score
        reduced by 50%.  The decay is continuous (exponential), not a
        step function.

        Also updates ``last_accessed`` and ``access_count`` for returned
        entries, reinforcing frequently accessed memories.

        Parameters
        ----------
        query : str
            The search query text.
        top_k : int
            Maximum number of results to return.
        memory_type : str | None
            Filter by memory type if specified.

        Returns
        -------
        list[tuple[VectorMemoryEntry, float]]
            Matching entries with decay-adjusted scores.
        """
        query_embedding = self._provider.embed(query)

        # Fetch more candidates than needed, then re-rank with decay
        candidates = self._search_by_similarity(
            query_embedding, top_k=top_k * 3, memory_type=memory_type,
            min_relevance=0.0,
        )

        # Apply forgetting-curve decay and re-rank
        decayed: list[tuple[VectorMemoryEntry, float]] = []
        for entry, similarity in candidates:
            decay_factor = self._compute_decay(entry.last_accessed)
            adjusted_score = similarity * decay_factor * entry.importance
            decayed.append((entry, adjusted_score))

        decayed.sort(key=lambda x: x[1], reverse=True)
        results = decayed[:top_k]

        # Update access stats for returned entries
        for entry, _ in results:
            self._touch(entry.id)

        return results

    def delete(self, memory_id: int) -> bool:
        """Delete a memory entry by ID."""
        cursor = self._conn.execute(
            "DELETE FROM vector_memories WHERE id = ?", (memory_id,)
        )
        self._conn.commit()
        return cursor.rowcount > 0

    def clear(self) -> None:
        """Remove all entries."""
        self._conn.execute("DELETE FROM vector_memories")
        self._conn.commit()
        logger.debug("Cleared vector memory")

    # ------------------------------------------------------------------
    # Properties
    # ------------------------------------------------------------------

    @property
    def token_cost_per_query(self) -> int:
        return _TOKEN_COST_PER_QUERY

    @property
    def max_entries(self) -> int:
        return self._max_entries

    @property
    def decay_days(self) -> float:
        return self._decay_days

    @property
    def dimension(self) -> int:
        return self._dimension

    def count(self) -> int:
        return self._count()

    # ------------------------------------------------------------------
    # Dunder helpers
    # ------------------------------------------------------------------

    def __len__(self) -> int:
        return self._count()

    def __repr__(self) -> str:
        return (
            f"VectorMemory(size={self._count()}/{self._max_entries}, "
            f"dim={self._dimension}, db={self._db_path!r})"
        )

    # ------------------------------------------------------------------
    # Private helpers
    # ------------------------------------------------------------------

    def _count(self) -> int:
        row = self._conn.execute("SELECT COUNT(*) FROM vector_memories").fetchone()
        return row[0] if row else 0

    def _evict_one(self) -> None:
        """Evict the lowest-importance oldest entry."""
        cursor = self._conn.execute(
            "SELECT id FROM vector_memories "
            "ORDER BY importance ASC, created_at ASC LIMIT 1"
        )
        row = cursor.fetchone()
        if row is not None:
            self._conn.execute("DELETE FROM vector_memories WHERE id = ?", (row[0],))
            self._conn.commit()

    def _touch(self, memory_id: int) -> None:
        """Update last_accessed and increment access_count."""
        now = _now_iso()
        self._conn.execute(
            "UPDATE vector_memories SET last_accessed = ?, access_count = access_count + 1 "
            "WHERE id = ?",
            (now, memory_id),
        )
        self._conn.commit()

    def _search_by_similarity(
        self,
        query_embedding: list[float],
        *,
        top_k: int = 5,
        memory_type: str | None = None,
        min_relevance: float = 0.0,
    ) -> list[tuple[VectorMemoryEntry, float]]:
        """Search for similar memories using vectorized cosine similarity.

        Loads all matching embeddings as a single numpy matrix and computes
        cosine similarity against the query vector in one vectorized operation,
        avoiding the per-row Python loop that was the previous bottleneck.
        """
        import numpy as np

        if memory_type is not None:
            rows = self._conn.execute(
                "SELECT id, content, memory_type, importance, embedding, "
                "created_at, last_accessed, access_count, source, metadata "
                "FROM vector_memories WHERE memory_type = ?",
                (memory_type,),
            ).fetchall()
        else:
            rows = self._conn.execute(
                "SELECT id, content, memory_type, importance, embedding, "
                "created_at, last_accessed, access_count, source, metadata "
                "FROM vector_memories",
            ).fetchall()

        if not rows:
            return []

        query_vec = np.array(query_embedding, dtype=np.float32)
        query_norm = np.linalg.norm(query_vec)
        if query_norm == 0:
            return []

        # Vectorized approach: build a matrix of all embeddings
        # and compute cosine similarity in one operation
        n_rows = len(rows)
        dim = len(query_embedding)
        emb_matrix = np.zeros((n_rows, dim), dtype=np.float32)
        valid_mask = np.ones(n_rows, dtype=bool)

        for i, row in enumerate(rows):
            emb_blob = row[4]  # embedding column
            if emb_blob is not None:
                emb = _blob_to_floats(emb_blob)
                if len(emb) == dim:
                    emb_matrix[i] = emb
                else:
                    valid_mask[i] = False
            else:
                valid_mask[i] = False

        # Compute cosine similarities for all valid rows at once
        norms = np.linalg.norm(emb_matrix, axis=1)
        nonzero_mask = norms > 0
        valid_mask &= nonzero_mask

        if not valid_mask.any():
            return []

        # Normalize and compute dot products
        valid_indices = np.where(valid_mask)[0]
        valid_norms = norms[valid_indices]
        valid_embs = emb_matrix[valid_indices]

        # cosine_sim = dot(query, emb) / (norm_query * norm_emb)
        cosine_sims = np.dot(valid_embs, query_vec) / (query_norm * valid_norms)
        # Clamp to [0, 1]
        cosine_sims = np.clip(cosine_sims, 0.0, 1.0)

        # Filter by min_relevance
        if min_relevance > 0:
            relevance_mask = cosine_sims >= min_relevance
            filtered_indices = valid_indices[relevance_mask]
            filtered_sims = cosine_sims[relevance_mask]
        else:
            filtered_indices = valid_indices
            filtered_sims = cosine_sims

        # Sort by similarity descending and take top_k
        if len(filtered_sims) > top_k:
            top_idx = np.argpartition(filtered_sims, -top_k)[-top_k:]
            top_idx = top_idx[np.argsort(filtered_sims[top_idx])[::-1]]
        else:
            top_idx = np.argsort(filtered_sims)[::-1]

        results: list[tuple[VectorMemoryEntry, float]] = []
        for idx in top_idx:
            row_idx = filtered_indices[idx]
            entry = self._row_to_entry(rows[row_idx])
            results.append((entry, float(filtered_sims[idx])))

        return results

    @staticmethod
    def _row_to_entry(row: tuple[Any, ...]) -> VectorMemoryEntry:
        """Convert a database row to a VectorMemoryEntry."""
        (id_, content, memory_type, importance, emb_blob,
         created_at, last_accessed, access_count, source, meta_json) = row
        embedding: list[float] | None = None
        if emb_blob is not None:
            embedding = _blob_to_floats(emb_blob)
        metadata: dict[str, Any] = {}
        if meta_json is not None:
            try:
                metadata = json.loads(meta_json)
            except (json.JSONDecodeError, TypeError):
                metadata = {}
        return VectorMemoryEntry(
            id=id_,
            content=content,
            memory_type=memory_type,
            importance=importance,
            embedding=embedding,
            created_at=created_at,
            last_accessed=last_accessed,
            access_count=access_count,
            source=source,
            metadata=metadata,
        )

    def _compute_decay(self, last_accessed: str) -> float:
        """Compute the forgetting curve decay factor.

        Uses Ebbinghaus-style exponential decay:
            decay_factor = 2 ** (-days_since_access / half_life)

        A memory accessed today has decay_factor = 1.0.
        A memory not accessed for 30 days has decay_factor ≈ 0.5.
        """
        try:
            last_dt = datetime.fromisoformat(last_accessed)
            if last_dt.tzinfo is None:
                last_dt = last_dt.replace(tzinfo=timezone.utc)
        except (ValueError, TypeError):
            return 1.0

        now_dt = datetime.now(timezone.utc)
        days_since = (now_dt - last_dt).total_seconds() / 86400.0

        if days_since <= 0:
            return 1.0

        # Exponential decay: half-life = decay_days
        decay_factor = 2.0 ** (-days_since / self._decay_days)
        return max(0.0, min(1.0, decay_factor))

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


# ---------------------------------------------------------------------------
# Serialization helpers
# ---------------------------------------------------------------------------


def _floats_to_blob(floats: list[float]) -> bytes:
    """Serialize a list of floats to a compact binary blob.

    Uses struct packing for efficient storage (4 bytes per float).
    """
    import struct

    return struct.pack(f"<{len(floats)}f", *floats)


def _blob_to_floats(blob: bytes) -> list[float]:
    """Deserialize a binary blob back to a list of floats."""
    import struct

    count = len(blob) // 4
    return list(struct.unpack(f"<{count}f", blob))


def _now_iso() -> str:
    """Return the current UTC time as an ISO 8601 string."""
    return datetime.now(timezone.utc).isoformat()
