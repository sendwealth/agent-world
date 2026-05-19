"""TraceQuery — high-level query API for Dashboard integration.

Provides a clean query interface over the TraceStore for the Dashboard
to consume.  Returns serializable dicts ready for JSON API responses.
"""

from __future__ import annotations

import logging
from dataclasses import dataclass, field
from typing import Any
from uuid import UUID

from agent_runtime.tracing.models import TickSnapshot, TickSummary
from agent_runtime.tracing.store import TraceStore

logger = logging.getLogger(__name__)


@dataclass
class TraceQuery:
    """Query parameters for trace lookups.

    All fields are optional — set only what you need.

    Attributes:
        agent_id: Filter by agent UUID.
        start_tick: Start of tick range (inclusive).
        end_tick: End of tick range (inclusive).
        start_time: Start of time range (ISO 8601).
        end_time: End of time range (ISO 8601).
        limit: Maximum results to return.
        offset: Pagination offset.
    """

    agent_id: UUID | None = None
    start_tick: int | None = None
    end_tick: int | None = None
    start_time: str | None = None
    end_time: str | None = None
    limit: int = 100
    offset: int = 0


class TraceQueryService:
    """High-level query service for trace data.

    Wraps TraceStore to provide Dashboard-friendly query methods
    that return serializable results.

    Args:
        store: The TraceStore to query against.
    """

    def __init__(self, store: TraceStore) -> None:
        self._store = store

    def get_tick(self, agent_id: UUID, tick: int) -> dict[str, Any] | None:
        """Get a single tick's full trace.

        Returns a dict with the full TickSnapshot data, or None if not found.
        """
        snapshot = self._store.get_snapshot(agent_id, tick)
        if snapshot is None:
            return None
        return snapshot.to_dict()

    def get_tick_range(
        self,
        agent_id: UUID,
        start_tick: int,
        end_tick: int,
    ) -> list[dict[str, Any]]:
        """Get full traces for a tick range.

        Returns a list of TickSnapshot dicts ordered by tick ascending.
        """
        snapshots = self._store.get_snapshots_by_tick_range(
            agent_id, start_tick, end_tick
        )
        return [s.to_dict() for s in snapshots]

    def get_time_range(
        self,
        agent_id: UUID,
        start_time: str,
        end_time: str,
    ) -> list[dict[str, Any]]:
        """Get full traces for a time range (ISO 8601 strings)."""
        snapshots = self._store.get_snapshots_by_time_range(
            agent_id, start_time, end_time
        )
        return [s.to_dict() for s in snapshots]

    def get_timeline(
        self,
        agent_id: UUID,
        *,
        limit: int = 100,
        offset: int = 0,
    ) -> list[dict[str, Any]]:
        """Get lightweight tick summaries for a timeline view.

        Returns summaries ordered by tick descending (newest first).
        """
        summaries = self._store.get_summaries(
            agent_id, limit=limit, offset=offset
        )
        return [
            {
                "agent_id": str(s.agent_id),
                "tick": s.tick,
                "action": s.action,
                "survival_mode": s.survival_mode,
                "token_ratio": s.token_ratio,
                "duration_ms": s.duration_ms,
                "started_at": s.started_at,
                "error": s.error,
            }
            for s in summaries
        ]

    def get_agent_stats(self, agent_id: UUID) -> dict[str, Any]:
        """Get aggregate statistics for an agent's traces."""
        total_ticks = self._store.count_ticks(agent_id)
        latest_tick = self._store.get_latest_tick(agent_id)

        return {
            "agent_id": str(agent_id),
            "total_ticks": total_ticks,
            "latest_tick": latest_tick,
        }

    def list_agents(self) -> list[dict[str, Any]]:
        """List all agents that have trace data."""
        agent_ids = self._store.get_all_agent_ids()
        result = []
        for aid in agent_ids:
            stats = self.get_agent_stats(aid)
            result.append(stats)
        return result

    def query(self, q: TraceQuery) -> list[dict[str, Any]]:
        """Execute a flexible query.

        Combines tick range, time range, and agent filters.
        Falls back to timeline if no range is specified.
        """
        if q.agent_id is None:
            # No agent filter — return all agents' stats
            return self.list_agents()

        if q.start_tick is not None and q.end_tick is not None:
            return self.get_tick_range(q.agent_id, q.start_tick, q.end_tick)

        if q.start_time is not None and q.end_time is not None:
            return self.get_time_range(q.agent_id, q.start_time, q.end_time)

        # Default: return timeline
        return self.get_timeline(
            q.agent_id, limit=q.limit, offset=q.offset
        )
