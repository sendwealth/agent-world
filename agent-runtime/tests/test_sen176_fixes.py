"""Tests for Phase 4.1-B (SEN-176) fixes: P0 + P1 code review items.

P0: switch_model() TOCTOU race — asyncio.Lock protection
P1: DecisionLogStore thread safety + lifecycle (context manager + lock)
P1: get_neighbors() O(V*E) → reverse adjacency map
P1: BFS list.pop(0) O(n) → collections.deque
"""

import asyncio
import json

import pytest

from agent_runtime.llm.base import LLMConfig, ProviderType
from agent_runtime.llm.cost import CostTracker
from agent_runtime.llm.decision_log import DecisionLog, DecisionLogStore
from agent_runtime.llm.ollama_provider import OllamaHealthStatus, OllamaProvider
from agent_runtime.tracing.emergence_metrics import EmergenceMetrics
from agent_runtime.tracing.interaction_graph import InteractionGraph

# ---------------------------------------------------------------------------
# P0: switch_model() TOCTOU race protection
# ---------------------------------------------------------------------------


class TestSwitchModelLock:
    """Verify that switch_model serializes config mutations."""

    def _make_config(self, model: str = "llama3") -> LLMConfig:
        return LLMConfig(provider=ProviderType.OLLAMA, model=model)

    @pytest.mark.asyncio
    async def test_switch_model_returns_old_model(self):
        provider = OllamaProvider(self._make_config("llama3"))
        old = await provider.switch_model("qwen2")
        assert old == "llama3"
        assert provider.active_model == "qwen2"
        await provider.close()

    @pytest.mark.asyncio
    async def test_switch_model_concurrent_no_loss(self):
        """Two concurrent switch_model calls should not silently lose one."""
        provider = OllamaProvider(self._make_config("llama3"))

        async def switch_to(model: str) -> str:
            return await provider.switch_model(model)

        results = await asyncio.gather(switch_to("qwen2"), switch_to("glm-4"))
        # Both should return the original model or the intermediate model
        assert set(results) == {"llama3", "qwen2"}
        # Final model should be one of the two requested
        assert provider.active_model in {"qwen2", "glm-4"}
        await provider.close()

    @pytest.mark.asyncio
    async def test_config_preserved_after_switch(self):
        """switch_model should preserve all other config fields."""
        config = self._make_config("llama3")
        config = LLMConfig(
            provider=ProviderType.OLLAMA,
            model="llama3",
            api_key=None,
            base_url="http://custom:11434",
            timeout=30.0,
            max_tokens=2048,
            temperature=0.7,
        )
        provider = OllamaProvider(config)
        await provider.switch_model("qwen2")
        assert provider.config.base_url == "http://custom:11434"
        assert provider.config.timeout == 30.0
        assert provider.config.max_tokens == 2048
        assert provider.config.temperature == 0.7
        await provider.close()


# ---------------------------------------------------------------------------
# P1: DecisionLogStore — context manager + append lock
# ---------------------------------------------------------------------------


class TestDecisionLogStore:
    def _make_log(self, agent_id: str = "a1", tick: int = 1) -> DecisionLog:
        return DecisionLog(
            agent_id=agent_id,
            tick=tick,
            timestamp="2025-01-01T00:00:00+00:00",
            prompt="test prompt",
            response_raw="test response",
            action_chosen="move",
            reasoning="reasoning",
            confidence=80,
            llm_model="llama3",
            latency_ms=100.0,
        )

    @pytest.mark.asyncio
    async def test_context_manager_creates_and_closes_file(self, tmp_path):
        path = tmp_path / "test.jsonl"
        async with DecisionLogStore(path) as store:
            await store.append(self._make_log())
            assert store.count == 1
        # After exiting, file should be closed and readable
        lines = path.read_text().strip().split("\n")
        assert len(lines) == 1
        data = json.loads(lines[0])
        assert data["agent_id"] == "a1"

    @pytest.mark.asyncio
    async def test_context_manager_no_path_is_memory_only(self):
        async with DecisionLogStore() as store:
            await store.append(self._make_log())
            assert store.count == 1

    @pytest.mark.asyncio
    async def test_concurrent_appends_serialized(self):
        """Many concurrent appends should not corrupt the list."""
        logs = [self._make_log(agent_id=f"a{i}", tick=i) for i in range(100)]

        async with DecisionLogStore() as store:
            await asyncio.gather(*(store.append(entry) for entry in logs))
            assert store.count == 100
            assert len(store) == 100

    @pytest.mark.asyncio
    async def test_query_by_agent(self):
        async with DecisionLogStore() as store:
            await store.append(self._make_log(agent_id="a1", tick=1))
            await store.append(self._make_log(agent_id="a2", tick=2))
            await store.append(self._make_log(agent_id="a1", tick=3))
            assert len(store.query(agent_id="a1")) == 2
            assert len(store.query(agent_id="a2")) == 1

    @pytest.mark.asyncio
    async def test_query_by_tick_range(self):
        async with DecisionLogStore() as store:
            for i in range(10):
                await store.append(self._make_log(tick=i))
            assert len(store.query(tick_min=3, tick_max=7)) == 5

    @pytest.mark.asyncio
    async def test_jsonl_persistence(self, tmp_path):
        path = tmp_path / "log.jsonl"
        async with DecisionLogStore(path) as store:
            await store.append(self._make_log(agent_id="x", tick=1))
            await store.append(self._make_log(agent_id="y", tick=2))
        lines = path.read_text().strip().split("\n")
        assert len(lines) == 2
        assert json.loads(lines[0])["agent_id"] == "x"
        assert json.loads(lines[1])["agent_id"] == "y"


# ---------------------------------------------------------------------------
# P1: InteractionGraph — reverse adjacency map + deque BFS
# ---------------------------------------------------------------------------


class TestInteractionGraphReverseAdjacency:
    def test_get_neighbors_includes_incoming(self):
        """get_neighbors should return both outgoing and incoming neighbors."""
        g = InteractionGraph()
        g.add_interaction("a", "b", "trade", 1)
        g.add_interaction("c", "a", "trade", 2)
        neighbors = g.get_neighbors("a")
        assert neighbors == {"b", "c"}

    def test_get_neighbors_empty(self):
        g = InteractionGraph()
        g.add_interaction("a", "b", "trade", 1)
        assert g.get_neighbors("c") == set()

    def test_get_neighbors_bidirectional(self):
        g = InteractionGraph()
        g.add_interaction("a", "b", "trade", 1)
        g.add_interaction("b", "a", "gift", 2)
        neighbors_a = g.get_neighbors("a")
        neighbors_b = g.get_neighbors("b")
        assert neighbors_a == {"b"}
        assert neighbors_b == {"a"}


class TestInteractionGraphBFS:
    def test_get_clusters_single(self):
        g = InteractionGraph()
        g.add_interaction("a", "b", "trade", 1)
        g.add_interaction("b", "c", "trade", 2)
        g.add_interaction("d", "e", "trade", 3)
        clusters = g.get_clusters()
        assert len(clusters) == 2
        sizes = sorted(len(c) for c in clusters)
        assert sizes == [2, 3]

    def test_get_clusters_isolated_node(self):
        g = InteractionGraph()
        g._nodes.add("lonely")
        g.add_interaction("a", "b", "trade", 1)
        clusters = g.get_clusters()
        assert len(clusters) == 2

    def test_get_clusters_empty_graph(self):
        g = InteractionGraph()
        assert g.get_clusters() == []

    def test_large_graph_performance(self):
        """Build a chain of 1000 nodes, verify clusters is one component."""
        g = InteractionGraph()
        for i in range(999):
            g.add_interaction(f"n{i}", f"n{i+1}", "link", i)
        clusters = g.get_clusters()
        assert len(clusters) == 1
        assert len(clusters[0]) == 1000


class TestInteractionGraphExport:
    def test_export_dot(self):
        g = InteractionGraph()
        g.add_interaction("a", "b", "trade", 1)
        dot = g.export_dot()
        assert "digraph" in dot
        assert '"a" -> "b"' in dot

    def test_export_dot_escapes_quotes(self):
        g = InteractionGraph()
        g.add_interaction('agent"a', 'agent"b', "trade", 1)
        dot = g.export_dot()
        assert r'\"' in dot or '\\"' in dot

    def test_export_json(self):
        g = InteractionGraph()
        g.add_interaction("a", "b", "trade", 1)
        data = g.export_json()
        assert "nodes" in data
        assert "edges" in data
        assert "clusters" in data
        assert "summary" in data
        assert data["summary"]["total_nodes"] == 2
        assert data["summary"]["total_edges"] == 1


# ---------------------------------------------------------------------------
# EmergenceMetrics — basic functionality
# ---------------------------------------------------------------------------


class TestEmergenceMetrics:
    def _make_tick(self, tick_num, agents):
        return {"tick": tick_num, "agents": agents}

    def test_empty_metrics(self):
        m = EmergenceMetrics()
        assert m.compute_diversity_index() == 0.0
        assert m.compute_cooperation_rate() == 0.0
        assert m.compute_survival_rate() == 0.0

    def test_diversity_index_uniform(self):
        m = EmergenceMetrics()
        m.record_tick(self._make_tick(1, [
            {"id": "a1", "action": "move", "alive": True},
            {"id": "a2", "action": "trade", "alive": True},
        ]))
        assert m.compute_diversity_index() == pytest.approx(1.0, abs=1e-6)

    def test_diversity_index_single_action(self):
        m = EmergenceMetrics()
        m.record_tick(self._make_tick(1, [
            {"id": "a1", "action": "move", "alive": True},
            {"id": "a2", "action": "move", "alive": True},
        ]))
        assert m.compute_diversity_index() == 0.0

    def test_cooperation_rate(self):
        m = EmergenceMetrics()
        m.record_tick(self._make_tick(1, [
            {"id": "a1", "action": "move", "alive": True, "interactions": [{"to": "a2"}]},
        ]))
        m.record_tick(self._make_tick(2, [
            {"id": "a1", "action": "move", "alive": True},
        ]))
        assert m.compute_cooperation_rate() == 0.5

    def test_survival_rate(self):
        m = EmergenceMetrics()
        m.record_tick(self._make_tick(1, [
            {"id": "a1", "action": "move", "alive": True},
            {"id": "a2", "action": "rest", "alive": False},
        ]))
        assert m.compute_survival_rate() == 0.5

    def test_export_json(self):
        m = EmergenceMetrics()
        m.record_tick(self._make_tick(1, [
            {"id": "a1", "action": "move", "alive": True},
        ]))
        data = m.export_json()
        assert "total_ticks" in data
        assert "diversity_index" in data
        assert data["total_ticks"] == 1

    def test_large_simulation(self):
        m = EmergenceMetrics()
        for t in range(100):
            agents = []
            for i in range(10):
                agents.append({
                    "id": f"a{i}",
                    "action": ["move", "trade", "rest", "gather"][i % 4],
                    "alive": i < 8,
                    # Even ticks: a0..a4 have interactions; odd ticks: none
                    "interactions": [{"to": f"a{(i+1)%10}"}] if t % 2 == 0 and i < 5 else [],
                })
            m.record_tick(self._make_tick(t, agents))
        assert m.compute_survival_rate() == 0.8
        assert m.compute_cooperation_rate() == 0.5
        assert 0.0 < m.compute_diversity_index() <= 1.0


# ---------------------------------------------------------------------------
# CostTracker — by_agent + by_time_range
# ---------------------------------------------------------------------------


class TestCostTrackerAgentAndTime:
    @pytest.mark.asyncio
    async def test_by_agent(self):
        from agent_runtime.llm.base import LLMResponse, TokenUsage

        tracker = CostTracker()
        resp1 = LLMResponse(content="hi", model="llama3", usage=TokenUsage(100, 50, 150))
        resp2 = LLMResponse(content="hi", model="llama3", usage=TokenUsage(200, 100, 300))
        await tracker.record(resp1, agent_id="a1")
        await tracker.record(resp2, agent_id="a2")
        result = tracker.by_agent()
        assert result["a1"]["calls"] == 1
        assert result["a2"]["calls"] == 1
        assert result["a1"]["total_tokens"] == 150
        assert result["a2"]["total_tokens"] == 300

    @pytest.mark.asyncio
    async def test_by_time_range(self):
        from agent_runtime.llm.base import LLMResponse, TokenUsage

        tracker = CostTracker()
        resp1 = LLMResponse(content="hi", model="llama3", usage=TokenUsage(100, 50, 150))
        resp2 = LLMResponse(content="hi", model="llama3", usage=TokenUsage(200, 100, 300))
        await tracker.record(resp1, agent_id="a1")
        await tracker.record(resp2, agent_id="a2")
        records = tracker._records
        records[0].timestamp
        records[1].timestamp
        # Query full range
        result = tracker.by_time_range("2000-01-01T00:00:00+00:00", "2099-12-31T23:59:59+00:00")
        assert result["calls"] == 2
        assert result["total_tokens"] == 450
        # Query empty range
        result_empty = tracker.by_time_range(
            "2099-01-01T00:00:00+00:00", "2099-12-31T23:59:59+00:00"
        )
        assert result_empty["calls"] == 0


# ---------------------------------------------------------------------------
# OllamaProvider — check_health
# ---------------------------------------------------------------------------


class TestOllamaHealthCheck:
    def test_health_status_dataclass(self):
        status = OllamaHealthStatus(healthy=True, loaded_models=["llama3"], num_parallel=4)
        assert status.healthy is True
        assert status.loaded_models == ["llama3"]
        assert status.num_parallel == 4

    def test_num_parallel_from_env(self, monkeypatch):
        monkeypatch.setenv("OLLAMA_NUM_PARALLEL", "4")
        config = LLMConfig(provider=ProviderType.OLLAMA, model="llama3")
        provider = OllamaProvider(config)
        assert provider.num_parallel == 4

    def test_num_parallel_default(self):
        config = LLMConfig(provider=ProviderType.OLLAMA, model="llama3")
        provider = OllamaProvider(config)
        assert provider.num_parallel == 1
