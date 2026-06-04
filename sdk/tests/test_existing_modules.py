"""Tests for social, economic, and behavior modules."""

from __future__ import annotations

import pytest

from agent_world_sdk.social import SocialModule
from agent_world_sdk.economic import EconomicModule
from agent_world_sdk.behavior import BehaviorModule


# =========================================================================
# SocialModule Tests
# =========================================================================

class TestDegreeCentrality:
    def test_basic(self):
        edges = [
            {"source": "a", "target": "b"},
            {"source": "b", "target": "c"},
            {"source": "a", "target": "c"},
        ]
        result = SocialModule.degree_centrality(edges, directed=False)
        assert "a" in result
        assert result["a"]["degree"] > 0

    def test_directed(self):
        edges = [
            {"source": "a", "target": "b"},
            {"source": "a", "target": "c"},
        ]
        result = SocialModule.degree_centrality(edges, directed=True)
        assert result["a"]["out_degree"] == 2
        assert result["b"]["in_degree"] == 1

    def test_empty(self):
        result = SocialModule.degree_centrality([])
        assert result == {}


class TestBetweennessCentrality:
    def test_basic(self):
        edges = [
            {"source": "a", "target": "b"},
            {"source": "b", "target": "c"},
            {"source": "c", "target": "d"},
        ]
        result = SocialModule.betweenness_centrality(edges, directed=False)
        # b and c should have higher betweenness than a and d
        assert result["b"] > result["a"]
        assert result["c"] > result["d"]

    def test_single_edge(self):
        edges = [{"source": "a", "target": "b"}]
        result = SocialModule.betweenness_centrality(edges)
        assert "a" in result

    def test_empty(self):
        result = SocialModule.betweenness_centrality([])
        assert result == {}


class TestConnectedComponents:
    def test_two_components(self):
        edges = [
            {"source": "a", "target": "b"},
            {"source": "b", "target": "c"},
            {"source": "d", "target": "e"},
        ]
        result = SocialModule.connected_components(edges)
        assert len(result) == 2
        assert len(result[0]) == 3  # a, b, c
        assert len(result[1]) == 2  # d, e

    def test_isolated_nodes(self):
        edges = [{"source": "a", "target": "b"}]
        nodes = [{"id": "a"}, {"id": "b"}, {"id": "c"}]
        result = SocialModule.connected_components(edges, nodes)
        assert len(result) == 2

    def test_empty(self):
        result = SocialModule.connected_components([])
        assert result == []


class TestCommunitySummary:
    def test_basic(self):
        edges = [
            {"source": "a", "target": "b"},
            {"source": "b", "target": "c"},
            {"source": "d", "target": "e"},
        ]
        mod = SocialModule()
        result = mod.community_summary(edges)
        assert result["component_count"] == 2
        assert result["largest_component_size"] == 3


class TestInteractionMatrix:
    def test_basic(self):
        edges = [
            {"source": "a", "target": "b", "weight": 2.0},
            {"source": "a", "target": "b", "weight": 3.0},
            {"source": "b", "target": "c", "weight": 1.0},
        ]
        result = SocialModule.interaction_matrix(edges)
        assert result["a"]["b"] == 5.0
        assert result["b"]["c"] == 1.0

    def test_empty(self):
        result = SocialModule.interaction_matrix([])
        assert result == {}


class TestTopInteractors:
    def test_basic(self):
        edges = [
            {"source": "a", "target": "b", "weight": 5.0},
            {"source": "a", "target": "c", "weight": 3.0},
            {"source": "b", "target": "c", "weight": 1.0},
        ]
        result = SocialModule.top_interactors(edges, top_n=2)
        assert len(result) == 2
        assert result[0]["agent_id"] == "a"
        assert result[0]["total_weight"] == 8.0


# =========================================================================
# EconomicModule Tests
# =========================================================================

class TestGini:
    def test_perfect_equality(self):
        g = EconomicModule.gini([100, 100, 100, 100])
        assert g == 0.0

    def test_maximal_inequality(self):
        g = EconomicModule.gini([0, 0, 0, 100])
        assert g > 0.5

    def test_empty(self):
        assert EconomicModule.gini([]) == 0.0

    def test_single(self):
        assert EconomicModule.gini([42]) == 0.0


class TestTopPercentShare:
    def test_top_10(self):
        vals = list(range(1, 101))  # 1 to 100
        share = EconomicModule.top_percent_share(vals, 0.1)
        # Top 10 of 100 = top 10 values (91-100)
        total = sum(range(1, 101))
        top_10_sum = sum(range(91, 101))
        assert share == pytest.approx(top_10_sum / total, abs=0.01)

    def test_empty(self):
        assert EconomicModule.top_percent_share([]) == 0.0


class TestWealthDistribution:
    def test_basic(self):
        agents = [
            {"money": 100, "tokens": 50, "alive": True},
            {"money": 200, "tokens": 100, "alive": True},
            {"money": 50, "tokens": 25, "alive": False},
        ]
        mod = EconomicModule()
        result = mod.wealth_distribution(agents)
        assert result["alive_count"] == 2
        assert result["total_money"] == 300
        assert result["money_gini"] >= 0

    def test_empty(self):
        mod = EconomicModule()
        result = mod.wealth_distribution([])
        assert result["alive_count"] == 0


class TestPriceTrend:
    def test_increasing(self):
        history = [{"tick": i, "total_tokens": 100 + i * 10} for i in range(10)]
        mod = EconomicModule()
        result = mod.price_trend(history, "total_tokens")
        assert result["change_pct"] > 0
        assert result["slope"] > 0

    def test_short_history(self):
        mod = EconomicModule()
        result = mod.price_trend([{"tick": 0, "val": 1}], "val")
        assert result["change_pct"] == 0.0


class TestInflationRate:
    def test_positive_inflation(self):
        history = [
            {"tick": 0, "total_money": 1000},
            {"tick": 1, "total_money": 1100},
            {"tick": 2, "total_money": 1200},
        ]
        mod = EconomicModule()
        result = mod.inflation_rate(history)
        assert result["cumulative"] > 0
        assert len(result["per_interval"]) == 2

    def test_short_history(self):
        mod = EconomicModule()
        result = mod.inflation_rate([{"tick": 0, "total_money": 100}])
        assert result["cumulative"] == 0.0


# =========================================================================
# BehaviorModule Tests
# =========================================================================

class TestSurvivalStats:
    def test_basic(self):
        agents = [
            {"alive": True, "ticks_survived": 100, "phase": "Adult"},
            {"alive": True, "ticks_survived": 200, "phase": "Adult"},
            {"alive": False, "ticks_survived": 50, "phase": "Elder"},
        ]
        result = BehaviorModule.survival_stats(agents)
        assert result["total"] == 3
        assert result["alive_count"] == 2
        assert result["dead_count"] == 1
        assert result["survival_rate"] == pytest.approx(2 / 3, abs=0.01)

    def test_empty(self):
        result = BehaviorModule.survival_stats([])
        assert result["total"] == 0


class TestActivityProfile:
    def test_basic(self):
        log = [
            {"agent_id": "a1", "event_type": "trade"},
            {"agent_id": "a1", "event_type": "trade"},
            {"agent_id": "a1", "event_type": "build"},
            {"agent_id": "a2", "event_type": "research"},
        ]
        result = BehaviorModule.activity_profile(log)
        assert "a1" in result["profiles"]
        assert result["profiles"]["a1"]["dominant_action"] == "trade"
        assert len(result["top_agents"]) <= 10

    def test_empty(self):
        result = BehaviorModule.activity_profile([])
        assert result["profiles"] == {}


class TestStrategyClassification:
    def test_trader(self):
        log = [
            {"agent_id": "a1", "event_type": "trade"},
            {"agent_id": "a1", "event_type": "trade"},
            {"agent_id": "a1", "event_type": "trade"},
            {"agent_id": "a1", "event_type": "other"},
        ]
        result = BehaviorModule.strategy_classification(log)
        assert result["agent_strategies"]["a1"] == "trader"

    def test_mixed(self):
        log = [
            {"agent_id": "a1", "event_type": "trade"},
            {"agent_id": "a1", "event_type": "build"},
            {"agent_id": "a1", "event_type": "research"},
            {"agent_id": "a1", "event_type": "message"},
        ]
        result = BehaviorModule.strategy_classification(log)
        assert result["agent_strategies"]["a1"] == "mixed"


class TestActivityOverTicks:
    def test_basic(self):
        log = [
            {"tick": 0, "event_type": "trade"},
            {"tick": 1, "event_type": "trade"},
            {"tick": 15, "event_type": "build"},
            {"tick": 16, "event_type": "build"},
        ]
        result = BehaviorModule.activity_over_ticks(log, tick_bucket_size=10)
        assert len(result) == 2  # [0-9] and [10-19]

    def test_empty(self):
        result = BehaviorModule.activity_over_ticks([])
        assert result == []
