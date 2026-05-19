"""Tests for the agent tracing system.

Covers:
- TickSnapshot / PhaseSnapshot data models: creation, serialization, deserialization
- TraceStore: SQLite CRUD, tick range queries, time range queries, batch writes
- TraceCollector: manual collection API, ThinkLoop integration
- TraceQueryService: Dashboard query API
- Acceptance: 100-tick trace with full query verification
- Performance: 10 agents × 1000 ticks under 100ms per query
"""

from __future__ import annotations

import asyncio
import json
import time
from datetime import datetime, timezone
from uuid import UUID, uuid4

import pytest

from agent_runtime.core.act import ActionExecutor, ActionType
from agent_runtime.core.think_loop import (
    Decision,
    Perception,
    ThinkLoop,
    ThinkLoopConfig,
)
from agent_runtime.models.agent_state import AgentState
from agent_runtime.survival.instinct import (
    SurvivalAction,
    SurvivalInstinct,
    SurvivalMode,
)
from agent_runtime.tracing import (
    PhaseSnapshot,
    TickSnapshot,
    TickSummary,
    TraceCollector,
    TraceQuery,
    TraceQueryService,
    TraceStore,
)
from agent_runtime.tracing.models import TracePhase


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def make_state(
    tokens: int = 500,
    max_tokens: int = 1000,
    *,
    name: str = "TracedAgent",
) -> AgentState:
    """Create a test AgentState with reasonable defaults."""
    return AgentState(
        name=name,
        tokens=tokens,
        max_tokens=max_tokens,
        money=50.0,
        health=100.0,
    )


def make_snapshot(
    agent_id: UUID | None = None,
    tick: int = 1,
    *,
    action: str = "rest",
    survival_mode: str = "normal",
    token_ratio: float = 0.5,
) -> TickSnapshot:
    """Create a test TickSnapshot with all phases populated."""
    agent_id = agent_id or uuid4()
    now_iso = datetime.now(timezone.utc).isoformat()
    return TickSnapshot(
        agent_id=agent_id,
        tick=tick,
        phases=[
            PhaseSnapshot(
                phase=TracePhase.SENSE,
                input_data={"tick": tick},
                output_data={"token_ratio": token_ratio, "health": 100.0},
                duration_ms=1.5,
            ),
            PhaseSnapshot(
                phase=TracePhase.SURVIVE,
                input_data={"token_ratio": token_ratio},
                output_data={"mode": survival_mode, "token_ratio": token_ratio},
                duration_ms=0.3,
            ),
            PhaseSnapshot(
                phase=TracePhase.DECIDE,
                input_data={"survival_mode": survival_mode},
                output_data={"action_type": action, "reasoning": "test"},
                duration_ms=50.0,
            ),
            PhaseSnapshot(
                phase=TracePhase.ACT,
                input_data={"action_type": action},
                output_data={"action_type": action, "status": "success"},
                duration_ms=5.0,
            ),
        ],
        started_at=now_iso,
        finished_at=now_iso,
        total_duration_ms=56.8,
    )


# ---------------------------------------------------------------------------
# PhaseSnapshot
# ---------------------------------------------------------------------------


class TestPhaseSnapshot:
    def test_defaults(self):
        p = PhaseSnapshot(phase=TracePhase.SENSE)
        assert p.phase == TracePhase.SENSE
        assert p.input_data == {}
        assert p.output_data == {}
        assert p.duration_ms == 0.0
        assert p.error is None

    def test_frozen(self):
        p = PhaseSnapshot(phase=TracePhase.SENSE)
        with pytest.raises(AttributeError):
            p.phase = TracePhase.DECIDE  # type: ignore[misc]

    def test_with_data(self):
        p = PhaseSnapshot(
            phase=TracePhase.ACT,
            input_data={"action": "explore"},
            output_data={"status": "success"},
            duration_ms=10.5,
            error=None,
        )
        assert p.input_data["action"] == "explore"
        assert p.duration_ms == 10.5


# ---------------------------------------------------------------------------
# TickSnapshot
# ---------------------------------------------------------------------------


class TestTickSnapshot:
    def test_creation(self):
        aid = uuid4()
        snap = TickSnapshot(agent_id=aid, tick=1)
        assert snap.agent_id == aid
        assert snap.tick == 1
        assert snap.phases == []

    def test_get_phase(self):
        snap = make_snapshot()
        sense = snap.get_phase(TracePhase.SENSE)
        assert sense is not None
        assert sense.output_data["token_ratio"] == 0.5

        # Non-existent phase (returns None when no match)
        assert snap.get_phase(TracePhase.ACT) is not None
        # get_phase returns None when the phase is not found in the list
        # (can't use invalid enum value, so we test with a phase that exists
        # but may not be in every snapshot)
        empty_snap = TickSnapshot(agent_id=uuid4(), tick=1)
        assert empty_snap.get_phase(TracePhase.SENSE) is None

    def test_serialization_roundtrip(self):
        snap = make_snapshot()
        data = snap.to_dict()
        assert data["tick"] == 1
        assert len(data["phases"]) == 4
        assert data["phases"][0]["phase"] == "sense"

        restored = TickSnapshot.from_dict(data)
        assert restored.agent_id == snap.agent_id
        assert restored.tick == snap.tick
        assert len(restored.phases) == len(snap.phases)
        assert restored.phases[0].phase == TracePhase.SENSE

    def test_json_roundtrip(self):
        snap = make_snapshot()
        json_str = snap.to_json()
        parsed = json.loads(json_str)
        assert parsed["tick"] == 1

        restored = TickSnapshot.from_json(json_str)
        assert restored.agent_id == snap.agent_id
        assert restored.tick == snap.tick

    def test_serialization_preserves_all_fields(self):
        snap = make_snapshot()
        snap.started_at = "2025-01-01T00:00:00+00:00"
        snap.finished_at = "2025-01-01T00:00:01+00:00"
        snap.total_duration_ms = 1000.0

        restored = TickSnapshot.from_json(snap.to_json())
        assert restored.started_at == snap.started_at
        assert restored.finished_at == snap.finished_at
        assert restored.total_duration_ms == snap.total_duration_ms

    def test_phase_with_error(self):
        snap = TickSnapshot(
            agent_id=uuid4(),
            tick=5,
            phases=[
                PhaseSnapshot(
                    phase=TracePhase.SENSE,
                    output_data={},
                    error="perception timeout",
                ),
            ],
        )
        restored = TickSnapshot.from_json(snap.to_json())
        assert restored.phases[0].error == "perception timeout"


# ---------------------------------------------------------------------------
# TraceStore
# ---------------------------------------------------------------------------


class TestTraceStore:
    def test_save_and_get(self):
        store = TraceStore(":memory:")
        agent_id = uuid4()
        snap = make_snapshot(agent_id=agent_id, tick=1)

        store.save(snap)

        result = store.get_snapshot(agent_id, 1)
        assert result is not None
        assert result.tick == 1
        assert len(result.phases) == 4

    def test_get_nonexistent(self):
        store = TraceStore(":memory:")
        result = store.get_snapshot(uuid4(), 999)
        assert result is None

    def test_tick_range_query(self):
        store = TraceStore(":memory:")
        agent_id = uuid4()

        for i in range(1, 11):
            store.save(make_snapshot(agent_id=agent_id, tick=i))

        results = store.get_snapshots_by_tick_range(agent_id, 3, 7)
        assert len(results) == 5
        assert [s.tick for s in results] == [3, 4, 5, 6, 7]

    def test_tick_range_empty(self):
        store = TraceStore(":memory:")
        results = store.get_snapshots_by_tick_range(uuid4(), 1, 10)
        assert results == []

    def test_time_range_query(self):
        store = TraceStore(":memory:")
        agent_id = uuid4()

        snap1 = make_snapshot(agent_id=agent_id, tick=1)
        snap1.started_at = "2025-06-01T10:00:00+00:00"
        snap2 = make_snapshot(agent_id=agent_id, tick=2)
        snap2.started_at = "2025-06-01T11:00:00+00:00"
        snap3 = make_snapshot(agent_id=agent_id, tick=3)
        snap3.started_at = "2025-06-01T12:00:00+00:00"

        store.save(snap1)
        store.save(snap2)
        store.save(snap3)

        results = store.get_snapshots_by_time_range(
            agent_id,
            "2025-06-01T10:30:00+00:00",
            "2025-06-01T11:30:00+00:00",
        )
        assert len(results) == 1
        assert results[0].tick == 2

    def test_latest_tick(self):
        store = TraceStore(":memory:")
        agent_id = uuid4()

        for i in range(1, 6):
            store.save(make_snapshot(agent_id=agent_id, tick=i))

        assert store.get_latest_tick(agent_id) == 5

    def test_latest_tick_no_data(self):
        store = TraceStore(":memory:")
        assert store.get_latest_tick(uuid4()) is None

    def test_summaries(self):
        store = TraceStore(":memory:")
        agent_id = uuid4()

        for i in range(1, 6):
            store.save(make_snapshot(agent_id=agent_id, tick=i, action="rest"))

        summaries = store.get_summaries(agent_id, limit=3)
        assert len(summaries) == 3
        # Ordered by tick DESC
        assert summaries[0].tick == 5
        assert summaries[0].action == "rest"

    def test_summaries_with_offset(self):
        store = TraceStore(":memory:")
        agent_id = uuid4()

        for i in range(1, 6):
            store.save(make_snapshot(agent_id=agent_id, tick=i))

        summaries = store.get_summaries(agent_id, limit=2, offset=2)
        assert len(summaries) == 2
        assert summaries[0].tick == 3

    def test_all_agent_ids(self):
        store = TraceStore(":memory:")
        id1 = uuid4()
        id2 = uuid4()

        store.save(make_snapshot(agent_id=id1, tick=1))
        store.save(make_snapshot(agent_id=id2, tick=1))

        agent_ids = store.get_all_agent_ids()
        assert set(agent_ids) == {id1, id2}

    def test_count_ticks(self):
        store = TraceStore(":memory:")
        agent_id = uuid4()

        for i in range(1, 11):
            store.save(make_snapshot(agent_id=agent_id, tick=i))

        assert store.count_ticks(agent_id) == 10

    def test_batch_save(self):
        store = TraceStore(":memory:")
        agent_id = uuid4()

        snapshots = [make_snapshot(agent_id=agent_id, tick=i) for i in range(1, 51)]
        store.save_batch(snapshots)

        assert store.count_ticks(agent_id) == 50
        assert store.get_latest_tick(agent_id) == 50

    def test_delete_agent_traces(self):
        store = TraceStore(":memory:")
        id1 = uuid4()
        id2 = uuid4()

        store.save(make_snapshot(agent_id=id1, tick=1))
        store.save(make_snapshot(agent_id=id2, tick=1))

        deleted = store.delete_agent_traces(id1)
        assert deleted == 1
        assert store.get_snapshot(id1, 1) is None
        assert store.get_snapshot(id2, 1) is not None

    def test_context_manager(self):
        with TraceStore(":memory:") as store:
            store.save(make_snapshot(tick=1))
            assert store.count_ticks(make_snapshot(tick=1).agent_id) >= 0


# ---------------------------------------------------------------------------
# TraceCollector — Manual API
# ---------------------------------------------------------------------------


class TestTraceCollectorManual:
    def test_full_tick_lifecycle(self):
        store = TraceStore(":memory:")
        agent_id = uuid4()
        collector = TraceCollector(agent_id=agent_id, store=store)

        # Simulate a full tick
        collector.on_tick_start(tick=1)
        collector.on_phase_start(TracePhase.SENSE)
        collector.on_phase_end(
            TracePhase.SENSE,
            input_data={"tick": 1},
            output_data={"token_ratio": 0.5, "health": 100.0},
        )
        collector.on_phase_start(TracePhase.SURVIVE)
        collector.on_phase_end(
            TracePhase.SURVIVE,
            output_data={"mode": "normal"},
        )
        collector.on_phase_start(TracePhase.DECIDE)
        collector.on_phase_end(
            TracePhase.DECIDE,
            output_data={"action_type": "rest", "reasoning": "tired"},
        )
        collector.on_phase_start(TracePhase.ACT)
        collector.on_phase_end(
            TracePhase.ACT,
            output_data={"action_type": "rest", "status": "success"},
        )
        result = collector.on_tick_end()

        assert result is not None
        assert result.tick == 1
        assert len(result.phases) == 4
        assert result.total_duration_ms > 0

        # Verify persisted
        saved = store.get_snapshot(agent_id, 1)
        assert saved is not None
        assert len(saved.phases) == 4

    def test_disabled_collector(self):
        store = TraceStore(":memory:")
        agent_id = uuid4()
        collector = TraceCollector(agent_id=agent_id, store=store, enabled=False)

        collector.on_tick_start(tick=1)
        collector.on_phase_end(TracePhase.SENSE, output_data={"test": True})
        result = collector.on_tick_end()

        assert result is None
        assert store.count_ticks(agent_id) == 0

    def test_toggle_enable_disable(self):
        store = TraceStore(":memory:")
        agent_id = uuid4()
        collector = TraceCollector(agent_id=agent_id, store=store)

        # Enabled
        collector.on_tick_start(tick=1)
        collector.on_phase_end(TracePhase.SENSE, output_data={})
        collector.on_tick_end()
        assert store.count_ticks(agent_id) == 1

        # Disable
        collector.disable()
        collector.on_tick_start(tick=2)
        collector.on_phase_end(TracePhase.SENSE, output_data={})
        collector.on_tick_end()
        assert store.count_ticks(agent_id) == 1  # No new save

        # Re-enable
        collector.enable()
        collector.on_tick_start(tick=3)
        collector.on_phase_end(TracePhase.SENSE, output_data={})
        collector.on_tick_end()
        assert store.count_ticks(agent_id) == 2

    def test_phase_with_error(self):
        store = TraceStore(":memory:")
        agent_id = uuid4()
        collector = TraceCollector(agent_id=agent_id, store=store)

        collector.on_tick_start(tick=1)
        collector.on_phase_start(TracePhase.SENSE)
        collector.on_phase_end(
            TracePhase.SENSE,
            output_data={},
            error="perception timeout",
        )
        result = collector.on_tick_end()

        assert result is not None
        assert result.phases[0].error == "perception timeout"


# ---------------------------------------------------------------------------
# TraceCollector — ThinkLoop Integration
# ---------------------------------------------------------------------------


class TestTraceCollectorThinkLoop:
    @pytest.mark.asyncio
    async def test_single_tick_with_tracing(self):
        store = TraceStore(":memory:")
        agent_id = uuid4()
        state = AgentState(
            id=agent_id,
            name="TracedAgent",
            tokens=500,
            max_tokens=1000,
        )
        collector = TraceCollector(agent_id=agent_id, store=store)

        loop = ThinkLoop(
            state=state,
            survival=SurvivalInstinct(),
            executor=ActionExecutor(),
            config=ThinkLoopConfig(tick_interval=0.0),
        )
        collector.install(loop)

        await loop.run(max_ticks=1)
        assert loop.tick == 1

        # Verify snapshot was saved
        snapshot = store.get_snapshot(agent_id, 1)
        assert snapshot is not None
        assert len(snapshot.phases) >= 2  # At least SENSE + SURVIVE
        assert snapshot.total_duration_ms > 0

        collector.uninstall(loop)

    @pytest.mark.asyncio
    async def test_ten_ticks_with_tracing(self):
        store = TraceStore(":memory:")
        agent_id = uuid4()
        state = AgentState(
            id=agent_id,
            name="TracedAgent",
            tokens=5000,
            max_tokens=10000,
        )
        collector = TraceCollector(agent_id=agent_id, store=store)

        loop = ThinkLoop(
            state=state,
            survival=SurvivalInstinct(),
            executor=ActionExecutor(),
            config=ThinkLoopConfig(tick_interval=0.0),
        )
        collector.install(loop)

        await loop.run(max_ticks=10)

        assert loop.tick == 10
        assert store.count_ticks(agent_id) == 10
        assert store.get_latest_tick(agent_id) == 10

        # Verify tick range query
        snapshots = store.get_snapshots_by_tick_range(agent_id, 3, 7)
        assert len(snapshots) == 5

        collector.uninstall(loop)

    @pytest.mark.asyncio
    async def test_install_uninstall_restores_original(self):
        store = TraceStore(":memory:")
        agent_id = uuid4()
        state = AgentState(
            id=agent_id,
            name="TracedAgent",
            tokens=500,
            max_tokens=1000,
        )
        collector = TraceCollector(agent_id=agent_id, store=store)

        loop = ThinkLoop(
            state=state,
            survival=SurvivalInstinct(),
            executor=ActionExecutor(),
            config=ThinkLoopConfig(tick_interval=0.0),
        )

        # Save the original unbound method
        original_method = ThinkLoop._think_once
        collector.install(loop)
        # After install, the instance attribute should differ from the class method
        assert loop.__dict__.get("_think_once") is not None

        collector.uninstall(loop)
        # After uninstall, instance attribute is removed, falls back to class
        assert "_think_once" not in loop.__dict__


# ---------------------------------------------------------------------------
# TraceQueryService
# ---------------------------------------------------------------------------


class TestTraceQueryService:
    def _setup_store_with_data(self) -> tuple[TraceStore, UUID]:
        store = TraceStore(":memory:")
        agent_id = uuid4()

        for i in range(1, 21):
            store.save(
                make_snapshot(
                    agent_id=agent_id,
                    tick=i,
                    action="rest" if i % 2 == 0 else "explore",
                    survival_mode="normal",
                    token_ratio=0.5,
                )
            )
        return store, agent_id

    def test_get_tick(self):
        store, agent_id = self._setup_store_with_data()
        svc = TraceQueryService(store)

        result = svc.get_tick(agent_id, 5)
        assert result is not None
        assert result["tick"] == 5
        assert len(result["phases"]) == 4

    def test_get_tick_not_found(self):
        store, _ = self._setup_store_with_data()
        svc = TraceQueryService(store)
        assert svc.get_tick(uuid4(), 999) is None

    def test_get_tick_range(self):
        store, agent_id = self._setup_store_with_data()
        svc = TraceQueryService(store)

        results = svc.get_tick_range(agent_id, 5, 10)
        assert len(results) == 6
        assert results[0]["tick"] == 5
        assert results[-1]["tick"] == 10

    def test_get_timeline(self):
        store, agent_id = self._setup_store_with_data()
        svc = TraceQueryService(store)

        timeline = svc.get_timeline(agent_id, limit=5)
        assert len(timeline) == 5
        # Ordered by tick DESC
        assert timeline[0]["tick"] == 20
        assert "action" in timeline[0]
        assert "survival_mode" in timeline[0]

    def test_get_timeline_with_offset(self):
        store, agent_id = self._setup_store_with_data()
        svc = TraceQueryService(store)

        timeline = svc.get_timeline(agent_id, limit=5, offset=5)
        assert len(timeline) == 5
        assert timeline[0]["tick"] == 15

    def test_get_agent_stats(self):
        store, agent_id = self._setup_store_with_data()
        svc = TraceQueryService(store)

        stats = svc.get_agent_stats(agent_id)
        assert stats["agent_id"] == str(agent_id)
        assert stats["total_ticks"] == 20
        assert stats["latest_tick"] == 20

    def test_list_agents(self):
        store, agent_id = self._setup_store_with_data()
        # Add a second agent
        id2 = uuid4()
        store.save(make_snapshot(agent_id=id2, tick=1))

        svc = TraceQueryService(store)
        agents = svc.list_agents()
        assert len(agents) == 2
        agent_ids = {a["agent_id"] for a in agents}
        assert str(agent_id) in agent_ids
        assert str(id2) in agent_ids

    def test_flexible_query_tick_range(self):
        store, agent_id = self._setup_store_with_data()
        svc = TraceQueryService(store)

        q = TraceQuery(agent_id=agent_id, start_tick=5, end_tick=10)
        results = svc.query(q)
        assert len(results) == 6

    def test_flexible_query_no_agent(self):
        store, _ = self._setup_store_with_data()
        svc = TraceQueryService(store)

        q = TraceQuery()
        results = svc.query(q)
        assert len(results) == 1  # One agent in the store

    def test_flexible_query_default_timeline(self):
        store, agent_id = self._setup_store_with_data()
        svc = TraceQueryService(store)

        q = TraceQuery(agent_id=agent_id, limit=3)
        results = svc.query(q)
        assert len(results) == 3


# ---------------------------------------------------------------------------
# Acceptance Tests
# ---------------------------------------------------------------------------


class TestAcceptance:
    @pytest.mark.asyncio
    async def test_100_tick_trace_queryable(self):
        """Acceptance: Agent runs 100 ticks, every tick queryable via API."""
        store = TraceStore(":memory:")
        agent_id = uuid4()
        state = AgentState(
            id=agent_id,
            name="AcceptanceAgent",
            tokens=10000,
            max_tokens=20000,
        )
        collector = TraceCollector(agent_id=agent_id, store=store)

        loop = ThinkLoop(
            state=state,
            survival=SurvivalInstinct(),
            executor=ActionExecutor(),
            config=ThinkLoopConfig(tick_interval=0.0),
        )
        collector.install(loop)
        await loop.run(max_ticks=100)
        collector.uninstall(loop)

        assert loop.tick == 100

        # Query any tick via API
        svc = TraceQueryService(store)
        for tick_num in [1, 25, 50, 75, 100]:
            result = svc.get_tick(agent_id, tick_num)
            assert result is not None, f"Tick {tick_num} not found"
            assert result["tick"] == tick_num
            assert len(result["phases"]) >= 2  # At least sense + survive

        # Timeline should show all 100 ticks
        timeline = svc.get_timeline(agent_id, limit=100)
        assert len(timeline) == 100

        # Stats
        stats = svc.get_agent_stats(agent_id)
        assert stats["total_ticks"] == 100
        assert stats["latest_tick"] == 100

    @pytest.mark.asyncio
    async def test_10_agents_1000_ticks_performance(self):
        """Acceptance: 10 agents × 1000 ticks, query < 100ms."""
        store = TraceStore(":memory:")
        agent_ids = [uuid4() for _ in range(10)]

        # Insert data for 10 agents × 1000 ticks
        for agent_id in agent_ids:
            snapshots = [
                make_snapshot(
                    agent_id=agent_id,
                    tick=t,
                    action="rest" if t % 2 == 0 else "explore",
                    token_ratio=0.5 + (t % 10) * 0.01,
                )
                for t in range(1, 1001)
            ]
            store.save_batch(snapshots)

        # Total should be 10,000
        for agent_id in agent_ids:
            assert store.count_ticks(agent_id) == 1000

        # Query performance test: tick range query
        svc = TraceQueryService(store)
        test_agent = agent_ids[0]

        start = time.monotonic()
        for _ in range(10):
            result = svc.get_tick_range(test_agent, 100, 500)
            assert len(result) == 401
        elapsed = (time.monotonic() - start) / 10 * 1000  # avg ms

        assert elapsed < 100, f"Query took {elapsed:.1f}ms (limit: 100ms)"

        # Single tick query performance
        start = time.monotonic()
        for _ in range(100):
            result = svc.get_tick(test_agent, 500)
            assert result is not None
        elapsed = (time.monotonic() - start) / 100 * 1000

        assert elapsed < 100, f"Single query took {elapsed:.1f}ms (limit: 100ms)"

    @pytest.mark.asyncio
    async def test_realtime_sense_think_act_display(self):
        """Acceptance: Dashboard can display sense→think→act for current tick."""
        store = TraceStore(":memory:")
        agent_id = uuid4()
        state = AgentState(
            id=agent_id,
            name="DashboardAgent",
            tokens=5000,
            max_tokens=10000,
        )
        collector = TraceCollector(agent_id=agent_id, store=store)

        loop = ThinkLoop(
            state=state,
            survival=SurvivalInstinct(),
            executor=ActionExecutor(),
            config=ThinkLoopConfig(tick_interval=0.0),
        )
        collector.install(loop)
        await loop.run(max_ticks=10)
        collector.uninstall(loop)

        # Dashboard queries latest tick
        svc = TraceQueryService(store)
        latest_tick = store.get_latest_tick(agent_id)
        assert latest_tick == 10

        result = svc.get_tick(agent_id, latest_tick)
        assert result is not None

        # Verify sense→think→act phases present
        phases = {p["phase"] for p in result["phases"]}
        assert "sense" in phases
        assert "survive" in phases
        assert "decide" in phases or "act" in phases

        # Verify sense output has perception data
        sense_phase = next(
            (p for p in result["phases"] if p["phase"] == "sense"), None
        )
        if sense_phase:
            assert "token_ratio" in sense_phase["output_data"]
            assert "health" in sense_phase["output_data"]
