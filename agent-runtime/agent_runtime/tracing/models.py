"""TickSnapshot data models for agent tracing.

A TickSnapshot captures the complete decision trajectory of one tick:
  - Perception phase (what the agent sensed)
  - Think phase (what the agent considered)
  - Act phase (what the agent did)

Each phase is recorded as a PhaseSnapshot with input/output pairs.
"""

from __future__ import annotations

import json
from dataclasses import dataclass, field
from enum import Enum
from typing import Any
from uuid import UUID


class TracePhase(str, Enum):
    """Phases in the think loop that are traced."""

    SENSE = "sense"
    SURVIVE = "survive"
    DECIDE = "decide"
    ACT = "act"


@dataclass(frozen=True)
class PhaseSnapshot:
    """Snapshot of a single phase within a tick.

    Attributes:
        phase: Which phase this snapshot captures.
        input_data: What went into this phase (serialized to JSON-safe dict).
        output_data: What came out of this phase.
        duration_ms: Wall-clock time spent in this phase.
        error: Error message if the phase failed, None otherwise.
    """

    phase: TracePhase
    input_data: dict[str, Any] = field(default_factory=dict)
    output_data: dict[str, Any] = field(default_factory=dict)
    duration_ms: float = 0.0
    error: str | None = None


@dataclass
class TickSnapshot:
    """Complete trace of one Perceive → Decide → Act tick.

    Attributes:
        agent_id: UUID of the agent.
        tick: Tick number (1-indexed).
        phases: Ordered list of phase snapshots for this tick.
        started_at: ISO 8601 timestamp when the tick started.
        finished_at: ISO 8601 timestamp when the tick finished.
        total_duration_ms: Total wall-clock time for the entire tick.
    """

    agent_id: UUID
    tick: int
    phases: list[PhaseSnapshot] = field(default_factory=list)
    started_at: str = ""
    finished_at: str = ""
    total_duration_ms: float = 0.0

    def get_phase(self, phase: TracePhase) -> PhaseSnapshot | None:
        """Get the snapshot for a specific phase, if recorded."""
        for p in self.phases:
            if p.phase == phase:
                return p
        return None

    def to_dict(self) -> dict[str, Any]:
        """Serialize to a JSON-safe dict for storage."""
        return {
            "agent_id": str(self.agent_id),
            "tick": self.tick,
            "phases": [
                {
                    "phase": p.phase.value,
                    "input_data": p.input_data,
                    "output_data": p.output_data,
                    "duration_ms": p.duration_ms,
                    "error": p.error,
                }
                for p in self.phases
            ],
            "started_at": self.started_at,
            "finished_at": self.finished_at,
            "total_duration_ms": self.total_duration_ms,
        }

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> TickSnapshot:
        """Deserialize from a dict (e.g., from SQLite)."""
        phases = [
            PhaseSnapshot(
                phase=TracePhase(p["phase"]),
                input_data=p.get("input_data", {}),
                output_data=p.get("output_data", {}),
                duration_ms=p.get("duration_ms", 0.0),
                error=p.get("error"),
            )
            for p in data.get("phases", [])
        ]
        return cls(
            agent_id=UUID(data["agent_id"]),
            tick=data["tick"],
            phases=phases,
            started_at=data.get("started_at", ""),
            finished_at=data.get("finished_at", ""),
            total_duration_ms=data.get("total_duration_ms", 0.0),
        )

    def to_json(self) -> str:
        """Serialize to JSON string."""
        return json.dumps(self.to_dict(), ensure_ascii=False)

    @classmethod
    def from_json(cls, data: str) -> TickSnapshot:
        """Deserialize from JSON string."""
        return cls.from_dict(json.loads(data))


@dataclass(frozen=True)
class TickSummary:
    """Lightweight summary of a tick for list views.

    Used when the full phase data isn't needed (e.g., timeline view).
    """

    agent_id: UUID
    tick: int
    action: str
    survival_mode: str
    token_ratio: float
    duration_ms: float
    started_at: str
    error: str | None = None
