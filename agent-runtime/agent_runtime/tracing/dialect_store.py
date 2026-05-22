"""SQLite storage for dialect divergence analysis results.

Persists DivergenceReport snapshots (distance matrix + dialect regions)
so they can be queried by the API layer and visualized on the dashboard.

Uses stdlib sqlite3 — no new dependencies.
"""

from __future__ import annotations

import json
import logging
import sqlite3
import time
from typing import Any, Optional

logger = logging.getLogger(__name__)

# ---------------------------------------------------------------------------
# Schema
# ---------------------------------------------------------------------------

_SCHEMA_SQL = """
CREATE TABLE IF NOT EXISTS dialect_reports (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    tick            INTEGER NOT NULL,
    grouping_method TEXT    NOT NULL DEFAULT 'region',
    avg_inter_distance  REAL NOT NULL DEFAULT 0.0,
    avg_intra_distance  REAL NOT NULL DEFAULT 0.0,
    divergence_index    REAL NOT NULL DEFAULT 0.0,
    matrix_json     TEXT    NOT NULL DEFAULT '{}',
    regions_json    TEXT    NOT NULL DEFAULT '[]',
    created_at      REAL    NOT NULL DEFAULT 0.0
);

CREATE INDEX IF NOT EXISTS idx_dialect_tick
    ON dialect_reports (tick);

CREATE INDEX IF NOT EXISTS idx_dialect_method
    ON dialect_reports (grouping_method);
"""


class DialectStore:
    """SQLite-backed store for dialect divergence reports.

    Thread-safe via a single connection with check_same_thread=False
    and explicit lock for writes.

    Args:
        db_path: Path to the SQLite database file.
            Use ``:memory:`` for in-memory testing.
    """

    def __init__(self, db_path: str = "dialect.db") -> None:
        self._db_path = db_path
        self._conn = sqlite3.connect(db_path, check_same_thread=False)
        self._conn.execute("PRAGMA journal_mode=WAL")
        self._conn.execute("PRAGMA synchronous=NORMAL")
        self._conn.row_factory = sqlite3.Row
        self._init_schema()

    def _init_schema(self) -> None:
        self._conn.executescript(_SCHEMA_SQL)
        self._conn.commit()

    # ------------------------------------------------------------------
    # Write
    # ------------------------------------------------------------------

    def save_report(self, report_data: dict[str, Any]) -> int:
        """Persist a DivergenceReport (as dict) to SQLite.

        Args:
            report_data: Dict with keys: tick, grouping_method,
                avg_inter_group_distance, avg_intra_group_distance,
                divergence_index, matrix (DialectDistanceMatrix dict),
                regions (list of DialectRegion dicts).

        Returns:
            Row ID of the inserted report.
        """
        matrix_data = report_data.get("matrix", {})
        regions_data = report_data.get("regions", [])

        cursor = self._conn.execute(
            """
            INSERT INTO dialect_reports
                (tick, grouping_method, avg_inter_distance, avg_intra_distance,
                 divergence_index, matrix_json, regions_json, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (
                report_data.get("tick", 0),
                report_data.get("grouping_method", "region"),
                report_data.get("avg_inter_group_distance", 0.0),
                report_data.get("avg_intra_group_distance", 0.0),
                report_data.get("divergence_index", 0.0),
                json.dumps(matrix_data),
                json.dumps(regions_data),
                time.time(),
            ),
        )
        self._conn.commit()
        return cursor.lastrowid  # type: ignore[return-value]

    # ------------------------------------------------------------------
    # Read
    # ------------------------------------------------------------------

    def get_latest_report(self) -> Optional[dict[str, Any]]:
        """Get the most recent dialect report."""
        row = self._conn.execute(
            """
            SELECT * FROM dialect_reports
            ORDER BY tick DESC
            LIMIT 1
            """
        ).fetchone()
        if row is None:
            return None
        return self._row_to_dict(row)

    def get_report_by_tick(self, tick: int) -> Optional[dict[str, Any]]:
        """Get a dialect report by tick number."""
        row = self._conn.execute(
            "SELECT * FROM dialect_reports WHERE tick = ?",
            (tick,),
        ).fetchone()
        if row is None:
            return None
        return self._row_to_dict(row)

    def get_distance_matrix(self, tick: Optional[int] = None) -> dict[str, Any]:
        """Get the dialect distance matrix for a specific tick.

        If tick is None, returns the latest matrix.

        Returns:
            Dict with: tick, group_ids, distances (flat list), method.
        """
        report = (
            self.get_report_by_tick(tick) if tick is not None
            else self.get_latest_report()
        )
        if report is None:
            return {"tick": 0, "group_ids": [], "distances": [], "method": "cosine"}

        matrix = report.get("matrix", {})
        group_ids = matrix.get("group_ids", [])
        distances = matrix.get("distances", {})

        # Flatten the nested distances dict.
        flat: list[dict[str, Any]] = []
        seen: set[tuple[str, str]] = set()
        for a, targets in distances.items():
            for b, d in targets.items():
                key = (min(a, b), max(a, b))
                if key not in seen:
                    seen.add(key)
                    flat.append({"source": a, "target": b, "distance": d})

        return {
            "tick": report["tick"],
            "group_ids": group_ids,
            "distances": flat,
            "method": matrix.get("method", "cosine"),
        }

    def get_dialect_regions(self, tick: Optional[int] = None) -> dict[str, Any]:
        """Get dialect regions for a specific tick.

        If tick is None, returns the latest regions.

        Returns:
            Dict with: tick, regions (list of region dicts).
        """
        report = (
            self.get_report_by_tick(tick) if tick is not None
            else self.get_latest_report()
        )
        if report is None:
            return {"tick": 0, "regions": []}

        return {
            "tick": report["tick"],
            "regions": report.get("regions", []),
        }

    def get_divergence_timeline(
        self,
        limit: int = 100,
        offset: int = 0,
    ) -> list[dict[str, Any]]:
        """Get a timeline of divergence metrics across ticks."""
        rows = self._conn.execute(
            """
            SELECT tick, grouping_method, avg_inter_distance,
                   avg_intra_distance, divergence_index, created_at
            FROM dialect_reports
            ORDER BY tick ASC
            LIMIT ? OFFSET ?
            """,
            (limit, offset),
        ).fetchall()
        return [
            {
                "tick": r["tick"],
                "grouping_method": r["grouping_method"],
                "avg_inter_distance": r["avg_inter_distance"],
                "avg_intra_distance": r["avg_intra_distance"],
                "divergence_index": r["divergence_index"],
                "created_at": r["created_at"],
            }
            for r in rows
        ]

    # ------------------------------------------------------------------
    # Maintenance
    # ------------------------------------------------------------------

    def delete_reports_before_tick(self, tick: int) -> int:
        """Delete all reports before a given tick. Returns count of deleted rows."""
        cursor = self._conn.execute(
            "DELETE FROM dialect_reports WHERE tick < ?",
            (tick,),
        )
        self._conn.commit()
        return cursor.rowcount

    def close(self) -> None:
        self._conn.close()

    def __enter__(self) -> DialectStore:
        return self

    def __exit__(self, *args: Any) -> None:
        self.close()

    # ------------------------------------------------------------------
    # Internals
    # ------------------------------------------------------------------

    @staticmethod
    def _row_to_dict(row: sqlite3.Row) -> dict[str, Any]:
        return {
            "id": row["id"],
            "tick": row["tick"],
            "grouping_method": row["grouping_method"],
            "avg_inter_group_distance": row["avg_inter_distance"],
            "avg_intra_group_distance": row["avg_intra_distance"],
            "divergence_index": row["divergence_index"],
            "matrix": json.loads(row["matrix_json"]),
            "regions": json.loads(row["regions_json"]),
            "created_at": row["created_at"],
        }
