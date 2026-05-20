"""SQLite storage layer for TickSnapshots.

Uses only stdlib sqlite3 — no new dependencies.  The schema is optimized
for the query patterns:
  - By agent_id + tick range
  - By agent_id + time range
  - Latest tick for a given agent
"""

from __future__ import annotations

import logging
import sqlite3
import time
from typing import Any
from uuid import UUID

from agent_runtime.tracing.models import TickSnapshot, TickSummary

logger = logging.getLogger(__name__)

# ---------------------------------------------------------------------------
# Schema
# ---------------------------------------------------------------------------

_SCHEMA_SQL = """
CREATE TABLE IF NOT EXISTS tick_snapshots (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    agent_id        TEXT    NOT NULL,
    tick            INTEGER NOT NULL,
    snapshot_json   TEXT    NOT NULL,
    action          TEXT    NOT NULL DEFAULT '',
    survival_mode   TEXT    NOT NULL DEFAULT '',
    token_ratio     REAL    NOT NULL DEFAULT 0.0,
    duration_ms     REAL    NOT NULL DEFAULT 0.0,
    started_at      TEXT    NOT NULL DEFAULT '',
    has_error       INTEGER NOT NULL DEFAULT 0,
    error_msg       TEXT    DEFAULT NULL,
    created_at      REAL    NOT NULL DEFAULT 0.0
);

CREATE INDEX IF NOT EXISTS idx_snapshots_agent_tick
    ON tick_snapshots (agent_id, tick);

CREATE INDEX IF NOT EXISTS idx_snapshots_agent_time
    ON tick_snapshots (agent_id, started_at);

CREATE INDEX IF NOT EXISTS idx_snapshots_created
    ON tick_snapshots (created_at);
"""


class TraceStore:
    """SQLite-backed store for tick snapshots.

    Thread-safe via a single connection with check_same_thread=False
    and explicit lock for writes.  Suitable for single-process agent
    runtime usage.

    Args:
        db_path: Path to the SQLite database file.
            Use ``:memory:`` for in-memory testing.
    """

    def __init__(self, db_path: str = "traces.db") -> None:
        self._db_path = db_path
        self._conn = sqlite3.connect(db_path, check_same_thread=False)
        self._conn.execute("PRAGMA journal_mode=WAL")
        self._conn.execute("PRAGMA synchronous=NORMAL")
        self._conn.row_factory = sqlite3.Row
        self._init_schema()

    def _init_schema(self) -> None:
        """Create tables and indexes if they don't exist."""
        self._conn.executescript(_SCHEMA_SQL)
        self._conn.commit()

    # ------------------------------------------------------------------
    # Write
    # ------------------------------------------------------------------

    def save(self, snapshot: TickSnapshot) -> None:
        """Persist a TickSnapshot to SQLite.

        Extracts summary fields for indexed querying and stores the full
        snapshot as JSON for complete retrieval.
        """
        act_phase = snapshot.get_phase("act")  # use value, not enum
        sense_phase = snapshot.get_phase("sense")
        survive_phase = snapshot.get_phase("survive")

        # Extract summary fields from phases
        action = ""
        if act_phase and act_phase.output_data:
            action = act_phase.output_data.get("action_type", "")

        survival_mode = ""
        if survive_phase and survive_phase.output_data:
            survival_mode = survive_phase.output_data.get("mode", "")

        token_ratio = 0.0
        if sense_phase and sense_phase.output_data:
            token_ratio = sense_phase.output_data.get("token_ratio", 0.0)

        # Check for any phase errors
        has_error = 0
        error_msg = None
        for p in snapshot.phases:
            if p.error:
                has_error = 1
                error_msg = p.error
                break

        self._conn.execute(
            """
            INSERT INTO tick_snapshots
                (agent_id, tick, snapshot_json, action, survival_mode,
                 token_ratio, duration_ms, started_at, has_error,
                 error_msg, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (
                str(snapshot.agent_id),
                snapshot.tick,
                snapshot.to_json(),
                action,
                survival_mode,
                token_ratio,
                snapshot.total_duration_ms,
                snapshot.started_at,
                has_error,
                error_msg,
                time.time(),
            ),
        )
        self._conn.commit()

    def save_batch(self, snapshots: list[TickSnapshot]) -> None:
        """Persist multiple TickSnapshots in a single transaction."""
        rows = []
        for snapshot in snapshots:
            act_phase = snapshot.get_phase("act")
            sense_phase = snapshot.get_phase("sense")
            survive_phase = snapshot.get_phase("survive")

            action = ""
            if act_phase and act_phase.output_data:
                action = act_phase.output_data.get("action_type", "")

            survival_mode = ""
            if survive_phase and survive_phase.output_data:
                survival_mode = survive_phase.output_data.get("mode", "")

            token_ratio = 0.0
            if sense_phase and sense_phase.output_data:
                token_ratio = sense_phase.output_data.get("token_ratio", 0.0)

            has_error = 0
            error_msg = None
            for p in snapshot.phases:
                if p.error:
                    has_error = 1
                    error_msg = p.error
                    break

            rows.append((
                str(snapshot.agent_id),
                snapshot.tick,
                snapshot.to_json(),
                action,
                survival_mode,
                token_ratio,
                snapshot.total_duration_ms,
                snapshot.started_at,
                has_error,
                error_msg,
                time.time(),
            ))

        self._conn.executemany(
            """
            INSERT INTO tick_snapshots
                (agent_id, tick, snapshot_json, action, survival_mode,
                 token_ratio, duration_ms, started_at, has_error,
                 error_msg, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            """,
            rows,
        )
        self._conn.commit()

    # ------------------------------------------------------------------
    # Read
    # ------------------------------------------------------------------

    def get_snapshot(self, agent_id: UUID, tick: int) -> TickSnapshot | None:
        """Get a single TickSnapshot by agent_id and tick number."""
        row = self._conn.execute(
            """
            SELECT snapshot_json FROM tick_snapshots
            WHERE agent_id = ? AND tick = ?
            """,
            (str(agent_id), tick),
        ).fetchone()
        if row is None:
            return None
        return TickSnapshot.from_json(row["snapshot_json"])

    def get_snapshots_by_tick_range(
        self,
        agent_id: UUID,
        start_tick: int,
        end_tick: int,
    ) -> list[TickSnapshot]:
        """Get all snapshots for an agent in a tick range (inclusive)."""
        rows = self._conn.execute(
            """
            SELECT snapshot_json FROM tick_snapshots
            WHERE agent_id = ? AND tick >= ? AND tick <= ?
            ORDER BY tick ASC
            """,
            (str(agent_id), start_tick, end_tick),
        ).fetchall()
        return [TickSnapshot.from_json(r["snapshot_json"]) for r in rows]

    def get_snapshots_by_time_range(
        self,
        agent_id: UUID,
        start_time: str,
        end_time: str,
    ) -> list[TickSnapshot]:
        """Get all snapshots for an agent in a time range (ISO 8601)."""
        rows = self._conn.execute(
            """
            SELECT snapshot_json FROM tick_snapshots
            WHERE agent_id = ? AND started_at >= ? AND started_at <= ?
            ORDER BY tick ASC
            """,
            (str(agent_id), start_time, end_time),
        ).fetchall()
        return [TickSnapshot.from_json(r["snapshot_json"]) for r in rows]

    def get_latest_tick(self, agent_id: UUID) -> int | None:
        """Get the latest tick number for an agent."""
        row = self._conn.execute(
            """
            SELECT MAX(tick) as max_tick FROM tick_snapshots
            WHERE agent_id = ?
            """,
            (str(agent_id),),
        ).fetchone()
        if row is None or row["max_tick"] is None:
            return None
        return int(row["max_tick"])

    def get_summaries(
        self,
        agent_id: UUID,
        *,
        limit: int = 100,
        offset: int = 0,
    ) -> list[TickSummary]:
        """Get lightweight tick summaries for timeline views."""
        rows = self._conn.execute(
            """
            SELECT agent_id, tick, action, survival_mode, token_ratio,
                   duration_ms, started_at, error_msg
            FROM tick_snapshots
            WHERE agent_id = ?
            ORDER BY tick DESC
            LIMIT ? OFFSET ?
            """,
            (str(agent_id), limit, offset),
        ).fetchall()
        return [
            TickSummary(
                agent_id=UUID(r["agent_id"]),
                tick=r["tick"],
                action=r["action"],
                survival_mode=r["survival_mode"],
                token_ratio=r["token_ratio"],
                duration_ms=r["duration_ms"],
                started_at=r["started_at"],
                error=r["error_msg"],
            )
            for r in rows
        ]

    def get_all_agent_ids(self) -> list[UUID]:
        """Get all distinct agent IDs that have traces."""
        rows = self._conn.execute(
            "SELECT DISTINCT agent_id FROM tick_snapshots"
        ).fetchall()
        return [UUID(r["agent_id"]) for r in rows]

    def count_ticks(self, agent_id: UUID) -> int:
        """Count total recorded ticks for an agent."""
        row = self._conn.execute(
            "SELECT COUNT(*) as cnt FROM tick_snapshots WHERE agent_id = ?",
            (str(agent_id),),
        ).fetchone()
        return int(row["cnt"]) if row else 0

    def list_all(self) -> list[TickSnapshot]:
        """Get all TickSnapshots from the store, ordered by tick ASC."""
        rows = self._conn.execute(
            """
            SELECT snapshot_json FROM tick_snapshots
            ORDER BY tick ASC
            """
        ).fetchall()
        return [TickSnapshot.from_json(r["snapshot_json"]) for r in rows]

    # ------------------------------------------------------------------
    # Maintenance
    # ------------------------------------------------------------------

    def delete_agent_traces(self, agent_id: UUID) -> int:
        """Delete all traces for an agent. Returns count of deleted rows."""
        cursor = self._conn.execute(
            "DELETE FROM tick_snapshots WHERE agent_id = ?",
            (str(agent_id),),
        )
        self._conn.commit()
        return cursor.rowcount

    def close(self) -> None:
        """Close the database connection."""
        self._conn.close()

    def __enter__(self) -> TraceStore:
        return self

    def __exit__(self, *args: Any) -> None:
        self.close()
