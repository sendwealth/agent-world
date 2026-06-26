"""Agent behavior log exporter for researcher analysis.

Supports:
- Per-agent trace export with tick range filtering
- Batch export of multiple agents
- Paginated iteration for large datasets
- JSON and CSV output formats
- Batch summary statistics
"""

from __future__ import annotations

import csv
import io
import json
from dataclasses import dataclass
from typing import TYPE_CHECKING, Any, Iterator

if TYPE_CHECKING:
    from agent_runtime.tracing.store import TraceStore


@dataclass
class BehaviorEntry:
    """Single agent behavior record."""
    agent_id: str
    tick: int
    phase: str
    action: str
    input_data: dict
    output_data: dict
    duration_ms: float
    error: str | None = None


@dataclass
class BatchPage:
    """A single page of paginated behavior entries."""
    entries: list[BehaviorEntry]
    page: int
    page_size: int
    total_entries: int
    has_next: bool


class BehaviorLogExporter:
    """Export agent decision behavior logs from trace data.

    Supports filtering by agent, tick range, and event type.
    Outputs JSON or CSV formats compatible with analysis tools.
    Provides paginated iteration for large datasets.
    """

    def __init__(self, trace_store: TraceStore) -> None:
        self._store = trace_store
        self._agent_filter: list[str] | None = None
        self._tick_range: tuple[int, int] | None = None
        self._event_type_filter: list[str] | None = None

    def filter_agents(self, agent_ids: list[str]) -> BehaviorLogExporter:
        """Filter to specific agents."""
        self._agent_filter = agent_ids
        return self

    def filter_tick_range(self, start: int, end: int) -> BehaviorLogExporter:
        """Filter to a tick range (inclusive)."""
        self._tick_range = (start, end)
        return self

    def filter_by_event_type(self, event_types: list[str]) -> BehaviorLogExporter:
        """Filter by event/phase types (sense, survive, decide, act)."""
        self._event_type_filter = event_types
        return self

    def _collect_entries(self) -> list[BehaviorEntry]:
        """Collect behavior entries from the trace store with applied filters."""
        entries: list[BehaviorEntry] = []

        # Get snapshots from the trace store
        snapshots = self._store.list_all()

        for snap in snapshots:
            # Apply agent filter
            agent_id = str(snap.agent_id)
            if self._agent_filter and agent_id not in self._agent_filter:
                continue

            # Apply tick range filter
            if self._tick_range:
                start, end = self._tick_range
                if snap.tick < start or snap.tick > end:
                    continue

            # Extract entries from each phase
            for phase in snap.phases:
                # phase.phase is a TracePhase enum; compare using .value
                phase_value = (
                    phase.phase.value
                    if hasattr(phase.phase, "value")
                    else str(phase.phase)
                )

                # Apply event type filter
                if self._event_type_filter and phase_value not in self._event_type_filter:
                    continue

                action = ""
                output = phase.output_data
                if isinstance(output, dict):
                    action = output.get("action_type", "") or ""
                    if not action:
                        action = output.get("action", "") or ""
                elif isinstance(output, str):
                    try:
                        parsed = json.loads(output)
                        action = parsed.get("action_type", "") or ""
                        if not action:
                            action = parsed.get("action", "") or ""
                    except (json.JSONDecodeError, AttributeError):
                        pass

                input_data = phase.input_data
                if not isinstance(input_data, dict):
                    input_data = {"raw": str(input_data)} if input_data else {}

                output_data = phase.output_data
                if not isinstance(output_data, dict):
                    output_data = {"raw": str(output_data)} if output_data else {}

                entries.append(BehaviorEntry(
                    agent_id=agent_id,
                    tick=snap.tick,
                    phase=phase_value,
                    action=action,
                    input_data=input_data,
                    output_data=output_data,
                    duration_ms=phase.duration_ms,
                    error=phase.error,
                ))

        return entries

    # ── Paginated Export ──────────────────────────────────────

    def iter_pages(self, page_size: int = 100) -> Iterator[BatchPage]:
        """Iterate over behavior entries in pages.

        Args:
            page_size: Number of entries per page.

        Yields:
            BatchPage objects containing entries and pagination metadata.
        """
        all_entries = self._collect_entries()
        total = len(all_entries)
        total_pages = (total + page_size - 1) // page_size if total > 0 else 1

        for page_num in range(total_pages):
            start_idx = page_num * page_size
            end_idx = min(start_idx + page_size, total)
            page_entries = all_entries[start_idx:end_idx]

            yield BatchPage(
                entries=page_entries,
                page=page_num,
                page_size=page_size,
                total_entries=total,
                has_next=(page_num + 1) < total_pages,
            )

    def get_page(self, page: int, page_size: int = 100) -> BatchPage:
        """Get a specific page of behavior entries.

        Args:
            page: Zero-indexed page number.
            page_size: Number of entries per page.

        Returns:
            BatchPage with entries and pagination metadata.
        """
        all_entries = self._collect_entries()
        total = len(all_entries)
        start_idx = page * page_size
        end_idx = min(start_idx + page_size, total)

        total_pages = (total + page_size - 1) // page_size if total > 0 else 1

        return BatchPage(
            entries=all_entries[start_idx:end_idx],
            page=page,
            page_size=page_size,
            total_entries=total,
            has_next=(page + 1) < total_pages,
        )

    # ── Single Agent Export ───────────────────────────────────

    def export_agent_trace(self, agent_id: str, tick_range: tuple[int, int],
                           format: str = "json") -> str:
        """Export a single agent's complete decision trace.

        Args:
            agent_id: The agent to export.
            tick_range: (start_tick, end_tick) inclusive.
            format: "json" or "csv".

        Returns:
            Serialized export data as string.
        """
        self._agent_filter = [agent_id]
        self._tick_range = tick_range
        entries = self._collect_entries()

        if format == "csv":
            return self._entries_to_csv(entries)
        return self._entries_to_json(entries)

    # ── Batch Export ──────────────────────────────────────────

    def export_batch_traces(self, agent_ids: list[str],
                           tick_range: tuple[int, int]) -> dict:
        """Batch export multiple agent traces.

        Returns:
            Dict mapping agent_id -> list of behavior entries.
        """
        result: dict[str, list[dict]] = {}

        for aid in agent_ids:
            self._agent_filter = [aid]
            self._tick_range = tick_range
            entries = self._collect_entries()
            result[aid] = [
                {
                    "tick": e.tick,
                    "phase": e.phase,
                    "action": e.action,
                    "duration_ms": e.duration_ms,
                    "error": e.error,
                }
                for e in entries
            ]

        return result

    def export_batch_summary(self, agent_ids: list[str] | None = None,
                             tick_range: tuple[int, int] | None = None) -> dict[str, Any]:
        """Export summary statistics for a batch of agents.

        Args:
            agent_ids: Agents to include. None for all agents.
            tick_range: Optional tick range filter.

        Returns:
            Dict with per-agent and aggregate statistics.
        """
        if agent_ids is not None:
            self._agent_filter = agent_ids
        if tick_range is not None:
            self._tick_range = tick_range

        entries = self._collect_entries()

        # Group by agent
        agent_entries: dict[str, list[BehaviorEntry]] = {}
        for entry in entries:
            agent_entries.setdefault(entry.agent_id, []).append(entry)

        per_agent: dict[str, dict[str, Any]] = {}
        total_errors = 0
        total_duration = 0.0
        phase_counts: dict[str, int] = {}

        for aid, agent_list in agent_entries.items():
            errors = sum(1 for e in agent_list if e.error)
            durations = [e.duration_ms for e in agent_list]

            per_agent[aid] = {
                "entry_count": len(agent_list),
                "error_count": errors,
                "avg_duration_ms": sum(durations) / len(durations) if durations else 0.0,
                "max_duration_ms": max(durations) if durations else 0.0,
                "min_duration_ms": min(durations) if durations else 0.0,
                "ticks_covered": len(set(e.tick for e in agent_list)),
            }

            total_errors += errors
            total_duration += sum(durations)

            for e in agent_list:
                phase_counts[e.phase] = phase_counts.get(e.phase, 0) + 1

        return {
            "total_entries": len(entries),
            "total_agents": len(agent_entries),
            "total_errors": total_errors,
            "avg_duration_ms": total_duration / len(entries) if entries else 0.0,
            "phase_distribution": phase_counts,
            "per_agent": per_agent,
        }

    def export_all_traces(self, format: str = "json",
                          tick_range: tuple[int, int] | None = None) -> str:
        """Export all agent traces in a single output.

        Args:
            format: "json" or "csv".
            tick_range: Optional tick range filter.

        Returns:
            Serialized export data as string.
        """
        if tick_range is not None:
            self._tick_range = tick_range
        self._agent_filter = None
        entries = self._collect_entries()

        if format == "csv":
            return self._entries_to_csv(entries)
        return self._entries_to_json(entries)

    # ── Serialization ────────────────────────────────────────

    def _entries_to_json(self, entries: list[BehaviorEntry]) -> str:
        """Serialize entries to JSON."""
        data = [
            {
                "agent_id": e.agent_id,
                "tick": e.tick,
                "phase": e.phase,
                "action": e.action,
                "input_data": e.input_data,
                "output_data": e.output_data,
                "duration_ms": e.duration_ms,
                "error": e.error,
            }
            for e in entries
        ]
        return json.dumps(data, indent=2, ensure_ascii=False)

    def _entries_to_csv(self, entries: list[BehaviorEntry]) -> str:
        """Serialize entries to CSV (Pandas-compatible)."""
        output = io.StringIO()
        writer = csv.writer(output, lineterminator="\n")
        writer.writerow(
            [
                "agent_id", "tick", "phase", "action", "duration_ms",
                "error", "input_data", "output_data",
            ]
        )
        for e in entries:
            writer.writerow([
                e.agent_id,
                e.tick,
                e.phase,
                e.action,
                e.duration_ms,
                e.error or "",
                json.dumps(e.input_data, ensure_ascii=False),
                json.dumps(e.output_data, ensure_ascii=False),
            ])
        return output.getvalue()
