"""Diary data model and SQLite storage layer.

Each ``DiaryEntry`` captures a single tick's subjective experience in the
agent's own voice.  Entries are persisted via ``DiaryStore`` in a local
SQLite database (WAL mode) so they survive agent restarts.

Query patterns optimised:
  - By agent_id + tick range (timeline view)
  - By agent_id + days (recent diary)
  - By agent_id + keyword full-text search
"""

from __future__ import annotations

import json
import logging
import sqlite3
from dataclasses import dataclass, field
from datetime import UTC, datetime
from pathlib import Path
from typing import Any
from uuid import UUID

logger = logging.getLogger(__name__)

# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------

_DEFAULT_MAX_ENTRIES: int = 2000

_SCHEMA = """\
CREATE TABLE IF NOT EXISTS diary_entries (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    agent_id    TEXT    NOT NULL,
    tick        INTEGER NOT NULL,
    phase       TEXT    NOT NULL DEFAULT '',
    mood        TEXT    NOT NULL DEFAULT 'neutral',
    summary     TEXT    NOT NULL,
    key_events  TEXT    NOT NULL DEFAULT '[]',
    decisions   TEXT    NOT NULL DEFAULT '[]',
    reflection  TEXT    NOT NULL DEFAULT '',
    created_at  TEXT    NOT NULL DEFAULT '',
    created_ts  REAL    NOT NULL DEFAULT 0.0
);
CREATE INDEX IF NOT EXISTS idx_diary_agent_tick
    ON diary_entries (agent_id, tick);
CREATE INDEX IF NOT EXISTS idx_diary_agent_ts
    ON diary_entries (agent_id, created_ts);
"""

# FTS virtual table for keyword search
_SCHEMA_FTS = """\
CREATE VIRTUAL TABLE IF NOT EXISTS diary_entries_fts
    USING fts5(summary, key_events, decisions, reflection,
               content=diary_entries, content_rowid=id);
"""

# Triggers to keep FTS in sync with the main table
_SCHEMA_TRIGGERS = (  # noqa: E501
    "CREATE TRIGGER IF NOT EXISTS diary_ai AFTER INSERT ON diary_entries BEGIN\n"
    "    INSERT INTO diary_entries_fts(rowid, summary, key_events,"
    " decisions, reflection)\n"
    "        VALUES (new.id, new.summary, new.key_events,"
    " new.decisions, new.reflection);\n"
    "END;\n"
    "CREATE TRIGGER IF NOT EXISTS diary_ad AFTER DELETE ON diary_entries BEGIN\n"
    "    INSERT INTO diary_entries_fts("
    "diary_entries_fts, rowid, summary, key_events,"
    " decisions, reflection)\n"
    "        VALUES ('delete', old.id, old.summary,"
    " old.key_events, old.decisions, old.reflection);\n"
    "END;\n"
)


# ---------------------------------------------------------------------------
# Data types
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class DiaryEntry:
    """A single diary entry for one tick.

    Attributes:
        id: Auto-generated row ID.
        agent_id: UUID of the agent who wrote this entry.
        tick: Simulation tick number.
        phase: Agent's lifecycle phase at the time.
        mood: Emotional tone of the entry (e.g. "hopeful", "anxious").
        summary: First-person narrative summary (50-150 chars target).
        key_events: List of notable events that happened this tick.
        decisions: List of decisions the agent made.
        reflection: Optional personal reflection beyond the summary.
        created_at: ISO 8601 timestamp of entry creation.
    """

    id: int = 0
    agent_id: str = ""
    tick: int = 0
    phase: str = ""
    mood: str = "neutral"
    summary: str = ""
    key_events: list[str] = field(default_factory=list)
    decisions: list[str] = field(default_factory=list)
    reflection: str = ""
    created_at: str = ""

    def to_dict(self) -> dict[str, Any]:
        """Serialize to a plain dict (API-ready)."""
        return {
            "id": self.id,
            "agent_id": self.agent_id,
            "tick": self.tick,
            "phase": self.phase,
            "mood": self.mood,
            "summary": self.summary,
            "key_events": self.key_events,
            "decisions": self.decisions,
            "reflection": self.reflection,
            "created_at": self.created_at,
        }


# ---------------------------------------------------------------------------
# DiaryStore
# ---------------------------------------------------------------------------


class DiaryStore:
    """SQLite-backed persistent storage for agent diary entries.

    Parameters
    ----------
    db_path : str | Path
        Path to the SQLite database file.  Use ``":memory:"`` for an
        in-memory database (useful for testing).
    max_entries : int
        Maximum number of entries to retain per agent.  When exceeded,
        the oldest entries are evicted first.

    Usage::

        store = DiaryStore(db_path="agent_diary.db")
        entry = DiaryEntry(agent_id="...", tick=10, summary="Today I...")
        store.write(entry)
        recent = store.read(agent_id, days=7)
        matches = store.search(agent_id, "trading")
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
        self._conn.execute("PRAGMA synchronous=NORMAL")
        self._conn.row_factory = sqlite3.Row
        self._init_schema()

    def _init_schema(self) -> None:
        """Create tables and indexes if they don't exist."""
        self._conn.executescript(_SCHEMA)
        # FTS is optional — some SQLite builds don't include FTS5
        try:
            self._conn.executescript(_SCHEMA_FTS)
            self._conn.executescript(_SCHEMA_TRIGGERS)
            self._conn.commit()
        except sqlite3.OperationalError:
            logger.debug("FTS5 not available — keyword search will use LIKE fallback")
            self._conn.commit()

    # ------------------------------------------------------------------
    # Write
    # ------------------------------------------------------------------

    def write(self, entry: DiaryEntry) -> DiaryEntry:
        """Persist a diary entry.

        Returns the entry with its auto-generated ``id`` and ``created_at``.
        """
        now = datetime.now(UTC)
        created_at = now.isoformat()
        created_ts = now.timestamp()

        cursor = self._conn.execute(
            "INSERT INTO diary_entries "
            "(agent_id, tick, phase, mood, summary, key_events, decisions, "
            "reflection, created_at, created_ts) "
            "VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            (
                entry.agent_id,
                entry.tick,
                entry.phase,
                entry.mood,
                entry.summary,
                json.dumps(entry.key_events, ensure_ascii=False),
                json.dumps(entry.decisions, ensure_ascii=False),
                entry.reflection,
                created_at,
                created_ts,
            ),
        )
        self._conn.commit()

        new_entry = DiaryEntry(
            id=cursor.lastrowid or 0,
            agent_id=entry.agent_id,
            tick=entry.tick,
            phase=entry.phase,
            mood=entry.mood,
            summary=entry.summary,
            key_events=entry.key_events,
            decisions=entry.decisions,
            reflection=entry.reflection,
            created_at=created_at,
        )

        # Evict oldest entries if over limit
        self._evict_if_needed(entry.agent_id)

        logger.debug(
            "Stored diary entry: tick=%d agent=%s mood=%s",
            entry.tick,
            entry.agent_id[:8],
            entry.mood,
        )
        return new_entry

    # ------------------------------------------------------------------
    # Read
    # ------------------------------------------------------------------

    def read(self, agent_id: str | UUID, *, days: int = 7) -> list[DiaryEntry]:
        """Get diary entries from the last N days.

        Args:
            agent_id: The agent UUID.
            days: Number of days to look back.

        Returns:
            List of entries ordered by tick ASC (oldest first).
        """
        cutoff_ts = (
            datetime.now(UTC).timestamp() - days * 86400
        )
        rows = self._conn.execute(
            "SELECT id, agent_id, tick, phase, mood, summary, key_events, "
            "decisions, reflection, created_at "
            "FROM diary_entries "
            "WHERE agent_id = ? AND created_ts >= ? "
            "ORDER BY tick ASC",
            (str(agent_id), cutoff_ts),
        ).fetchall()
        return [self._row_to_entry(r) for r in rows]

    def read_by_ticks(
        self,
        agent_id: str | UUID,
        start_tick: int,
        end_tick: int,
    ) -> list[DiaryEntry]:
        """Get diary entries for a tick range (inclusive)."""
        rows = self._conn.execute(
            "SELECT id, agent_id, tick, phase, mood, summary, key_events, "
            "decisions, reflection, created_at "
            "FROM diary_entries "
            "WHERE agent_id = ? AND tick >= ? AND tick <= ? "
            "ORDER BY tick ASC",
            (str(agent_id), start_tick, end_tick),
        ).fetchall()
        return [self._row_to_entry(r) for r in rows]

    def get_latest(self, agent_id: str | UUID) -> DiaryEntry | None:
        """Get the most recent diary entry for an agent."""
        row = self._conn.execute(
            "SELECT id, agent_id, tick, phase, mood, summary, key_events, "
            "decisions, reflection, created_at "
            "FROM diary_entries WHERE agent_id = ? "
            "ORDER BY tick DESC LIMIT 1",
            (str(agent_id),),
        ).fetchone()
        if row is None:
            return None
        return self._row_to_entry(row)

    # ------------------------------------------------------------------
    # Search
    # ------------------------------------------------------------------

    def search(
        self,
        agent_id: str | UUID,
        keyword: str,
        *,
        limit: int = 20,
    ) -> list[DiaryEntry]:
        """Search diary entries by keyword.

        Uses FTS5 if available, otherwise falls back to LIKE.

        Args:
            agent_id: The agent UUID.
            keyword: Search term.
            limit: Maximum results.

        Returns:
            Matching entries ordered by tick DESC.
        """
        aid = str(agent_id)

        # Try FTS5 first
        try:
            rows = self._conn.execute(
                "SELECT d.id, d.agent_id, d.tick, d.phase, d.mood, d.summary, "
                "d.key_events, d.decisions, d.reflection, d.created_at "
                "FROM diary_entries d "
                "JOIN diary_entries_fts f ON d.id = f.rowid "
                "WHERE f.diary_entries_fts MATCH ? AND d.agent_id = ? "
                "ORDER BY d.tick DESC LIMIT ?",
                (keyword, aid, limit),
            ).fetchall()
            return [self._row_to_entry(r) for r in rows]
        except sqlite3.OperationalError:
            pass

        # Fallback: LIKE search
        rows = self._conn.execute(
            "SELECT id, agent_id, tick, phase, mood, summary, key_events, "
            "decisions, reflection, created_at "
            "FROM diary_entries "
            "WHERE agent_id = ? AND "
            "(summary LIKE ? OR key_events LIKE ? OR reflection LIKE ?) "
            "ORDER BY tick DESC LIMIT ?",
            (aid, f"%{keyword}%", f"%{keyword}%", f"%{keyword}%", limit),
        ).fetchall()
        return [self._row_to_entry(r) for r in rows]

    # ------------------------------------------------------------------
    # Properties
    # ------------------------------------------------------------------

    @property
    def max_entries(self) -> int:
        return self._max_entries

    def count(self, agent_id: str | UUID) -> int:
        """Count diary entries for an agent."""
        row = self._conn.execute(
            "SELECT COUNT(*) FROM diary_entries WHERE agent_id = ?",
            (str(agent_id),),
        ).fetchone()
        return row[0] if row else 0

    # ------------------------------------------------------------------
    # Lifecycle
    # ------------------------------------------------------------------

    def close(self) -> None:
        """Close the database connection."""
        self._conn.close()

    def __enter__(self) -> DiaryStore:
        return self

    def __exit__(self, *args: Any) -> None:
        self.close()

    # ------------------------------------------------------------------
    # Private helpers
    # ------------------------------------------------------------------

    def _evict_if_needed(self, agent_id: str) -> None:
        """Evict oldest entries for an agent if over max_entries."""
        cnt = self.count(agent_id)
        if cnt <= self._max_entries:
            return
        excess = cnt - self._max_entries
        self._conn.execute(
            "DELETE FROM diary_entries WHERE agent_id = ? "
            "AND id IN ("
            "  SELECT id FROM diary_entries WHERE agent_id = ? "
            "  ORDER BY tick ASC LIMIT ?"
            ")",
            (agent_id, agent_id, excess),
        )
        self._conn.commit()

    @staticmethod
    def _row_to_entry(row: sqlite3.Row) -> DiaryEntry:
        """Convert a database row to a DiaryEntry."""
        key_events: list[str] = []
        if row["key_events"]:
            try:
                key_events = json.loads(row["key_events"])
            except (json.JSONDecodeError, TypeError):
                key_events = []

        decisions: list[str] = []
        if row["decisions"]:
            try:
                decisions = json.loads(row["decisions"])
            except (json.JSONDecodeError, TypeError):
                decisions = []

        return DiaryEntry(
            id=row["id"],
            agent_id=row["agent_id"],
            tick=row["tick"],
            phase=row["phase"],
            mood=row["mood"],
            summary=row["summary"],
            key_events=key_events,
            decisions=decisions,
            reflection=row["reflection"] or "",
            created_at=row["created_at"] or "",
        )
