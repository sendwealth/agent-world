"""Agent behavior log exporter for researcher analysis."""

from __future__ import annotations

import csv
import io
import json
from dataclasses import dataclass
from typing import TYPE_CHECKING

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


class BehaviorLogExporter:
    """Export agent decision behavior logs from trace data.

    Supports filtering by agent, tick range, and event type.
    Outputs JSON or CSV formats compatible with analysis tools.
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
                phase_value = (phase.phase.value
                    if hasattr(phase.phase, "value") else str(phase.phase))

                # Apply event type filter
                if self._event_type_filter and phase_value not in self._event_type_filter:
                    continue

                action = ""
                output = phase.output_data
                if isinstance(output, dict):
                    action = output.get("action_type", output.get("action", ""))
                elif isinstance(output, str):
                    try:
                        parsed = json.loads(output)
                        action = parsed.get("action_type", parsed.get("action", ""))
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
        writer.writerow([
            "agent_id", "tick", "phase",
            "action", "duration_ms", "error",
            "input_data", "output_data",
        ])
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
