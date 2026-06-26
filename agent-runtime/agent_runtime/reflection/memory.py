"""Long-term memory — SQLite-backed persistence for key decisions and lessons.

Stores reflection outcomes, critical decisions, and learned lessons
so the agent can carry knowledge across sessions.
"""

from __future__ import annotations

import json
import logging
import sqlite3
import time
from dataclasses import dataclass, field
from enum import StrEnum
from pathlib import Path
from typing import Any

logger = logging.getLogger(__name__)


class MemoryCategory(StrEnum):
    """Categories of long-term memory entries."""

    DECISION = "decision"
    LESSON = "lesson"
    REFLECTION = "reflection"
    STRATEGY_CHANGE = "strategy_change"
    MILESTONE = "milestone"


@dataclass
class MemoryEntry:
    """A single long-term memory entry."""

    category: MemoryCategory
    content: str
    tick: int
    importance: float = 0.5  # 0.0 to 1.0
    metadata: dict[str, Any] = field(default_factory=dict)
    created_at: float = 0.0
    id: int | None = None

    def __post_init__(self) -> None:
        if self.created_at == 0.0:
            self.created_at = time.time()
        self.importance = max(0.0, min(1.0, self.importance))


class LongTermMemory:
    """SQLite-backed long-term memory for agent reflections.

    Supports the context manager protocol for safe resource cleanup::

        with LongTermMemory(db_path) as mem:
            mem.store(entry)

    Stores decisions, lessons, and strategy changes with importance
    scoring and tick-based retrieval.
    """

    def __init__(self, db_path: Path | str = ":memory:") -> None:
        self._db_path = str(db_path)
        self._conn: sqlite3.Connection | None = None
        self._connect()
        self._init_schema()

    # ------------------------------------------------------------------
    # Context manager protocol
    # ------------------------------------------------------------------

    def __enter__(self) -> LongTermMemory:
        return self

    def __exit__(self, exc_type: Any, exc_val: Any, exc_tb: Any) -> None:
        self.close()

    # ------------------------------------------------------------------
    # Internal helpers
    # ------------------------------------------------------------------

    def _require_conn(self) -> sqlite3.Connection:
        """Return the connection or raise RuntimeError if closed."""
        if self._conn is None:
            raise RuntimeError("LongTermMemory connection is closed")
        return self._conn

    def _connect(self) -> None:
        """Create database connection."""
        self._conn = sqlite3.connect(self._db_path, check_same_thread=False)
        self._conn.row_factory = sqlite3.Row
        self._conn.execute("PRAGMA journal_mode=WAL")

    def _init_schema(self) -> None:
        """Initialize database schema."""
        conn = self._require_conn()
        conn.executescript(
            """
            CREATE TABLE IF NOT EXISTS memories (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                category TEXT NOT NULL,
                content TEXT NOT NULL,
                tick INTEGER NOT NULL,
                importance REAL NOT NULL DEFAULT 0.5,
                metadata TEXT NOT NULL DEFAULT '{}',
                created_at REAL NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_memories_category ON memories(category);
            CREATE INDEX IF NOT EXISTS idx_memories_tick ON memories(tick);
            CREATE INDEX IF NOT EXISTS idx_memories_importance ON memories(importance DESC);
            """
        )
        conn.commit()

    # ------------------------------------------------------------------
    # Write operations
    # ------------------------------------------------------------------

    def store(self, entry: MemoryEntry) -> int:
        """Store a memory entry. Returns the row ID."""
        conn = self._require_conn()
        cursor = conn.execute(
            """
            INSERT INTO memories (category, content, tick, importance, metadata, created_at)
            VALUES (?, ?, ?, ?, ?, ?)
            """,
            (
                entry.category.value,
                entry.content,
                entry.tick,
                entry.importance,
                json.dumps(entry.metadata, ensure_ascii=False),
                entry.created_at,
            ),
        )
        conn.commit()
        row_id = cursor.lastrowid
        logger.debug(
            "Stored memory [%s] tick=%d importance=%.2f: %s",
            entry.category.value,
            entry.tick,
            entry.importance,
            entry.content[:80],
        )
        return row_id  # type: ignore[return-value]

    def store_batch(self, entries: list[MemoryEntry]) -> list[int]:
        """Store multiple memory entries in a single transaction. Returns list of row IDs."""
        conn = self._require_conn()
        ids: list[int] = []
        try:
            conn.execute("BEGIN")
            for entry in entries:
                cursor = conn.execute(
                    """
                    INSERT INTO memories (category, content, tick, importance, metadata, created_at)
                    VALUES (?, ?, ?, ?, ?, ?)
                    """,
                    (
                        entry.category.value,
                        entry.content,
                        entry.tick,
                        entry.importance,
                        json.dumps(entry.metadata, ensure_ascii=False),
                        entry.created_at,
                    ),
                )
                ids.append(cursor.lastrowid)  # type: ignore[arg-type]
            conn.commit()
        except Exception:
            conn.rollback()
            raise
        return ids

    # ------------------------------------------------------------------
    # Read operations
    # ------------------------------------------------------------------

    def query(
        self,
        *,
        category: MemoryCategory | None = None,
        since_tick: int | None = None,
        min_importance: float = 0.0,
        limit: int = 100,
    ) -> list[MemoryEntry]:
        """Query memories with optional filters."""
        conn = self._require_conn()
        conditions: list[str] = []
        params: list[Any] = []

        if category is not None:
            conditions.append("category = ?")
            params.append(category.value)
        if since_tick is not None:
            conditions.append("tick >= ?")
            params.append(since_tick)
        if min_importance > 0.0:
            conditions.append("importance >= ?")
            params.append(min_importance)

        where = " AND ".join(conditions) if conditions else "1=1"
        query = f"""
            SELECT id, category, content, tick, importance, metadata, created_at
            FROM memories
            WHERE {where}
            ORDER BY importance DESC, created_at DESC
            LIMIT ?
        """
        params.append(limit)

        rows = conn.execute(query, params).fetchall()
        return [self._row_to_entry(row) for row in rows]

    def get_recent(self, limit: int = 10) -> list[MemoryEntry]:
        """Get the most recent memories."""
        conn = self._require_conn()
        rows = conn.execute(
            """
            SELECT id, category, content, tick, importance, metadata, created_at
            FROM memories
            ORDER BY created_at DESC
            LIMIT ?
            """,
            (limit,),
        ).fetchall()
        return [self._row_to_entry(row) for row in rows]

    def get_important(self, limit: int = 10, min_importance: float = 0.7) -> list[MemoryEntry]:
        """Get the most important memories."""
        return self.query(min_importance=min_importance, limit=limit)

    def count(self, category: MemoryCategory | None = None) -> int:
        """Count total memories, optionally filtered by category."""
        conn = self._require_conn()
        if category is not None:
            row = conn.execute(
                "SELECT COUNT(*) FROM memories WHERE category = ?",
                (category.value,),
            ).fetchone()
        else:
            row = conn.execute("SELECT COUNT(*) FROM memories").fetchone()
        return row[0]  # type: ignore[index]

    def delete_before_tick(self, tick: int) -> int:
        """Delete all memories before a given tick. Returns count deleted."""
        conn = self._require_conn()
        cursor = conn.execute(
            "DELETE FROM memories WHERE tick < ?", (tick,)
        )
        conn.commit()
        return cursor.rowcount  # type: ignore[return-value]

    # ------------------------------------------------------------------
    # Lifecycle
    # ------------------------------------------------------------------

    def close(self) -> None:
        """Close the database connection."""
        if self._conn is not None:
            self._conn.close()
            self._conn = None

    @staticmethod
    def _row_to_entry(row: sqlite3.Row) -> MemoryEntry:
        """Convert a database row to a MemoryEntry."""
        return MemoryEntry(
            id=row["id"],
            category=MemoryCategory(row["category"]),
            content=row["content"],
            tick=row["tick"],
            importance=row["importance"],
            metadata=json.loads(row["metadata"]),
            created_at=row["created_at"],
        )
