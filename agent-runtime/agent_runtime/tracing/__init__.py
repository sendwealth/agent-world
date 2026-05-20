"""Agent Tracing — TickSnapshot capture, storage, and query.

Records every tick's Perceive → Decide → Act cycle as a TickSnapshot,
persisted to SQLite for Dashboard replay and analysis.

Usage::

    from agent_runtime.tracing import TraceCollector, TraceStore

    store = TraceStore(db_path="traces.db")
    collector = TraceCollector(agent_id=agent.state.id, store=store)

    # Register hooks on the ThinkLoop (or call manually)
    loop = ThinkLoop(...)
    collector.install(loop)
"""

from agent_runtime.tracing.collector import TraceCollector
from agent_runtime.tracing.emergence_metrics import EmergenceMetrics
from agent_runtime.tracing.interaction_graph import Interaction, InteractionGraph
from agent_runtime.tracing.models import (
    PhaseSnapshot,
    TickSnapshot,
    TickSummary,
)
from agent_runtime.tracing.pusher import TracePusher
from agent_runtime.tracing.query import TraceQuery, TraceQueryService
from agent_runtime.tracing.store import TraceStore

__all__ = [
    "EmergenceMetrics",
    "Interaction",
    "InteractionGraph",
    "PhaseSnapshot",
    "TickSnapshot",
    "TickSummary",
    "TraceCollector",
    "TracePusher",
    "TraceQuery",
    "TraceQueryService",
    "TraceStore",
]
