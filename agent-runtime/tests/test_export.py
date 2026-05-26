"""Tests for agent_runtime.export module."""

import json

from agent_runtime.export.behavior_log import BehaviorEntry, BehaviorLogExporter
from agent_runtime.export.economy_export import EconomyExporter, compute_gini
from agent_runtime.export.network_export import NetworkExporter

# ── BehaviorLogExporter Tests ──

class TestBehaviorLogExporter:
    """Test behavior log export functionality."""

    def test_behavior_entry_creation(self):
        entry = BehaviorEntry(
            agent_id="agent-01",
            tick=5,
            phase="act",
            action="gather",
            input_data={"location": "forest"},
            output_data={"gathered": 10},
            duration_ms=150.0,
        )
        assert entry.agent_id == "agent-01"
        assert entry.tick == 5
        assert entry.error is None

    def test_behavior_entry_with_error(self):
        entry = BehaviorEntry(
            agent_id="agent-02",
            tick=3,
            phase="decide",
            action="trade",
            input_data={},
            output_data={},
            duration_ms=50.0,
            error="timeout",
        )
        assert entry.error == "timeout"

    def test_csv_format_basic(self):
        """Test CSV output format is Pandas-compatible."""
        exporter = BehaviorLogExporter.__new__(BehaviorLogExporter)
        entries = [
            BehaviorEntry("a1", 1, "act", "gather", {"x": 1}, {"y": 2}, 100.0),
            BehaviorEntry("a2", 2, "sense", "scan", {}, {}, 50.0, "timeout"),
        ]
        csv_output = exporter._entries_to_csv(entries)
        lines = csv_output.strip().split("\n")
        assert lines[0] == "agent_id,tick,phase,action,duration_ms,error,input_data,output_data"
        assert len(lines) == 3  # header + 2 rows

    def test_json_format_basic(self):
        """Test JSON output format."""
        exporter = BehaviorLogExporter.__new__(BehaviorLogExporter)
        entries = [
            BehaviorEntry("a1", 1, "act", "gather", {"x": 1}, {"y": 2}, 100.0),
        ]
        json_output = exporter._entries_to_json(entries)
        data = json.loads(json_output)
        assert len(data) == 1
        assert data[0]["agent_id"] == "a1"
        assert data[0]["tick"] == 1


# ── NetworkExporter Tests ──

class TestNetworkExporter:
    """Test network graph export functionality."""

    def _make_mock_graph(self):
        """Create a mock InteractionGraph with test data."""
        from agent_runtime.tracing.interaction_graph import InteractionGraph
        graph = InteractionGraph()
        graph.add_interaction("a1", "a2", "trade", 1)
        graph.add_interaction("a2", "a3", "chat", 2)
        graph.add_interaction("a1", "a3", "trade", 3)
        return graph

    def test_export_json_basic(self):
        graph = self._make_mock_graph()
        exporter = NetworkExporter(graph)
        result = exporter.export_json()
        assert "nodes" in result
        assert "edges" in result
        assert "summary" in result
        assert result["summary"]["node_count"] == 3
        assert result["summary"]["edge_count"] == 3

    def test_export_json_with_tick_range(self):
        graph = self._make_mock_graph()
        exporter = NetworkExporter(graph)
        result = exporter.export_json(tick_range=(1, 2))
        assert result["summary"]["edge_count"] == 2  # Only ticks 1 and 2

    def test_export_graphml(self):
        graph = self._make_mock_graph()
        exporter = NetworkExporter(graph)
        graphml = exporter.export_graphml()
        assert '<?xml version="1.0"' in graphml
        assert '<graphml' in graphml
        assert '<node id=' in graphml
        assert '<edge source=' in graphml
        assert 'interaction_type' in graphml

    def test_export_adjacency_matrix(self):
        graph = self._make_mock_graph()
        exporter = NetworkExporter(graph)
        matrix = exporter.export_adjacency_matrix(tick=1)
        # Only one interaction at tick 1: a1 -> a2
        # Nodes sorted: a1, a2 (only nodes with interactions at tick 1)
        # matrix[0][1] should be 1.0 (a1 -> a2)
        assert len(matrix) == 2
        assert matrix[0][1] == 1.0

    def test_export_adjacency_matrix_no_interactions(self):
        graph = self._make_mock_graph()
        exporter = NetworkExporter(graph)
        matrix = exporter.export_adjacency_matrix(tick=99)
        assert matrix == []

    def test_export_json_string(self):
        graph = self._make_mock_graph()
        exporter = NetworkExporter(graph)
        result = exporter.export_json_string()
        data = json.loads(result)
        assert "nodes" in data


# ── EconomyExporter Tests ──

class TestEconomyExporter:
    """Test economy metrics export functionality."""

    def test_compute_gini_equal_distribution(self):
        """Gini should be 0 for perfectly equal distribution."""
        result = compute_gini([100.0, 100.0, 100.0])
        assert result == 0.0

    def test_compute_gini_maximal_inequality(self):
        """Gini should approach 1.0 for maximal inequality."""
        result = compute_gini([0.0, 0.0, 0.0, 100.0])
        assert result > 0.5  # Should be significantly unequal

    def test_compute_gini_empty(self):
        assert compute_gini([]) == 0.0
        assert compute_gini([50.0]) == 0.0

    def test_compute_gini_all_zeros(self):
        assert compute_gini([0.0, 0.0, 0.0]) == 0.0

    def test_add_tick_data(self):
        exporter = EconomyExporter()
        agents = [
            {"money": 100, "tokens": 500, "alive": True},
            {"money": 200, "tokens": 300, "alive": True},
            {"money": 50, "tokens": 100, "alive": False},
        ]
        exporter.add_tick_data(tick=0, agents=agents, task_count=5)

        assert len(exporter._data_points) == 1
        dp = exporter._data_points[0]
        assert dp.total_money == 350
        assert dp.total_tokens == 900
        assert dp.alive_count == 2
        assert dp.agent_count == 3
        assert dp.task_count == 5

    def test_export_json(self):
        exporter = EconomyExporter()
        exporter.add_tick_data(tick=0, agents=[{"money": 100, "tokens": 50}])
        exporter.add_tick_data(tick=1, agents=[{"money": 200, "tokens": 100}])

        result = json.loads(exporter.export_json())
        assert len(result) == 2
        assert result[0]["tick"] == 0
        assert result[1]["tick"] == 1

    def test_export_csv(self):
        exporter = EconomyExporter()
        exporter.add_tick_data(tick=0, agents=[{"money": 100}])

        csv_output = exporter.export_csv()
        lines = csv_output.strip().split("\n")
        assert "tick,total_money,total_tokens" in lines[0]
        assert len(lines) == 2

    def test_get_summary(self):
        exporter = EconomyExporter()
        exporter.add_tick_data(tick=0, agents=[{"money": 100}])
        exporter.add_tick_data(tick=1, agents=[{"money": 200}])

        summary = exporter.get_summary()
        assert summary["total_ticks"] == 2
        assert summary["tick_range"] == [0, 1]

    def test_get_summary_empty(self):
        exporter = EconomyExporter()
        summary = exporter.get_summary()
        assert summary["total_ticks"] == 0

    def test_clear(self):
        exporter = EconomyExporter()
        exporter.add_tick_data(tick=0, agents=[{"money": 100}])
        exporter.clear()
        assert len(exporter._data_points) == 0
