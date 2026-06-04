"""Comprehensive tests for the analyze module."""

from __future__ import annotations

import math

import pytest

from agent_world_sdk.analyze import AnalyzeModule


# -- Fixtures ---------------------------------------------------------------

@pytest.fixture
def analyze():
    return AnalyzeModule()


@pytest.fixture
def sample_agents():
    return [
        {"id": "a1", "name": "Alice", "phase": "Adult", "money": 100, "tokens": 50,
         "alive": True, "skills": {"coding": 5, "trading": 3}},
        {"id": "a2", "name": "Bob", "phase": "Adult", "money": 200, "tokens": 80,
         "alive": True, "skills": {"coding": 2, "research": 7}},
        {"id": "a3", "name": "Carol", "phase": "Elder", "money": 50, "tokens": 20,
         "alive": True, "skills": {"teaching": 8}},
        {"id": "a4", "name": "Dave", "phase": "Childhood", "money": 300, "tokens": 150,
         "alive": False, "skills": {"mining": 1}},
        {"id": "a5", "name": "Eve", "phase": "Adult", "money": 150, "tokens": 60,
         "alive": True, "skills": {"coding": 4, "trading": 6}},
    ]


@pytest.fixture
def sample_history():
    """Tick-by-tick history with clear trends."""
    return [
        {"tick": 0, "total_money": 1000, "total_tokens": 500, "gini_coefficient": 0.3},
        {"tick": 1, "total_money": 1050, "total_tokens": 520, "gini_coefficient": 0.32},
        {"tick": 2, "total_money": 1100, "total_tokens": 540, "gini_coefficient": 0.35},
        {"tick": 3, "total_money": 1150, "total_tokens": 560, "gini_coefficient": 0.38},
        {"tick": 4, "total_money": 1200, "total_tokens": 580, "gini_coefficient": 0.40},
        {"tick": 5, "total_money": 1250, "total_tokens": 600, "gini_coefficient": 0.42},
        {"tick": 6, "total_money": 1300, "total_tokens": 620, "gini_coefficient": 0.45},
        {"tick": 7, "total_money": 1350, "total_tokens": 640, "gini_coefficient": 0.47},
        {"tick": 8, "total_money": 1400, "total_tokens": 660, "gini_coefficient": 0.50},
        {"tick": 9, "total_money": 1450, "total_tokens": 680, "gini_coefficient": 0.52},
    ]


@pytest.fixture
def sample_edges():
    return [
        {"source": "a1", "target": "a2", "weight": 3.0},
        {"source": "a2", "target": "a3", "weight": 1.0},
        {"source": "a1", "target": "a3", "weight": 2.0},
        {"source": "a3", "target": "a4", "weight": 1.5},
    ]


@pytest.fixture
def sample_events():
    return [
        {"agent_id": "a1", "tick": 0, "action": "trade", "phase": "Adult"},
        {"agent_id": "a1", "tick": 1, "action": "build", "phase": "Adult"},
        {"agent_id": "a1", "tick": 5, "action": "trade", "phase": "Elder"},
        {"agent_id": "a2", "tick": 0, "action": "research", "phase": "Adult"},
        {"agent_id": "a2", "tick": 2, "action": "communicate", "phase": "Adult"},
        {"agent_id": "a2", "tick": 3, "action": "research", "phase": "Adult",
         "context": {"topic": "economics"}, "outcome": "success"},
    ]


# =========================================================================
# Descriptive Statistics
# =========================================================================

class TestDescriptiveStats:
    def test_basic_stats(self, analyze):
        result = analyze.descriptive_stats([1, 2, 3, 4, 5])
        assert result["count"] == 5
        assert result["mean"] == 3.0
        assert result["median"] == 3.0
        assert result["min"] == 1
        assert result["max"] == 5
        assert result["range"] == 4
        assert result["std_dev"] > 0

    def test_empty(self, analyze):
        result = analyze.descriptive_stats([])
        assert result["count"] == 0
        assert result["mean"] == 0.0

    def test_single_value(self, analyze):
        result = analyze.descriptive_stats([42])
        assert result["count"] == 1
        assert result["mean"] == 42
        assert result["variance"] == 0.0
        assert result["std_dev"] == 0.0

    def test_quartiles(self, analyze):
        # 1,2,3,4,5,6,7,8,9,10
        result = analyze.descriptive_stats(list(range(1, 11)))
        assert result["q1"] == pytest.approx(3.25, abs=0.1)
        assert result["q3"] == pytest.approx(7.75, abs=0.1)
        assert result["iqr"] > 0

    def test_skewness(self, analyze):
        # Right-skewed data
        result = analyze.descriptive_stats([1, 1, 1, 1, 100])
        assert result["skewness"] > 0

    def test_mode(self, analyze):
        result = analyze.descriptive_stats([1, 2, 2, 3, 4])
        assert result["mode"] == 2


class TestFrequencyDistribution:
    def test_categorical(self, analyze):
        result = analyze.frequency_distribution(["a", "b", "a", "c", "a"])
        assert result["counts"]["a"] == 3
        assert result["total"] == 5
        assert result["unique_count"] == 3

    def test_binned(self, analyze):
        result = analyze.frequency_distribution(
            [1, 2, 3, 4, 5, 6, 7, 8, 9, 10], bins=5
        )
        assert result["total"] == 10
        assert len(result.get("bin_edges", [])) == 6  # 5 bins = 6 edges

    def test_empty(self, analyze):
        result = analyze.frequency_distribution([])
        assert result["total"] == 0


class TestGroupStatistics:
    def test_basic(self, analyze):
        data = [
            {"group": "A", "value": 10},
            {"group": "A", "value": 20},
            {"group": "B", "value": 30},
            {"group": "B", "value": 40},
        ]
        result = analyze.group_statistics(data, "group", "value")
        assert "A" in result
        assert "B" in result
        assert result["A"]["mean"] == 15.0
        assert result["B"]["mean"] == 35.0

    def test_empty(self, analyze):
        result = analyze.group_statistics([], "group", "value")
        assert result == {}


# =========================================================================
# Correlation Analysis
# =========================================================================

class TestCorrelation:
    def test_perfect_positive(self, analyze):
        x = [1, 2, 3, 4, 5]
        y = [2, 4, 6, 8, 10]
        r = analyze.pearson_correlation(x, y)
        assert r == pytest.approx(1.0, abs=0.001)

    def test_perfect_negative(self, analyze):
        x = [1, 2, 3, 4, 5]
        y = [10, 8, 6, 4, 2]
        r = analyze.pearson_correlation(x, y)
        assert r == pytest.approx(-1.0, abs=0.001)

    def test_no_correlation_short(self, analyze):
        r = analyze.pearson_correlation([1], [2])
        assert r == 0.0

    def test_spearman(self, analyze):
        x = [1, 2, 3, 4, 5]
        y = [5, 4, 3, 2, 1]
        r = analyze.spearman_correlation(x, y)
        assert r == pytest.approx(-1.0, abs=0.001)

    def test_correlation_matrix(self, analyze):
        data = [
            {"a": 1, "b": 2, "c": 3},
            {"a": 2, "b": 4, "c": 6},
            {"a": 3, "b": 6, "c": 9},
        ]
        result = analyze.correlation_matrix(data, ["a", "b", "c"])
        assert "a" in result
        assert result["a"]["b"] == pytest.approx(1.0, abs=0.001)

    def test_behavior_outcome(self, analyze):
        agents = [
            {"activity": 10, "wealth": 100},
            {"activity": 20, "wealth": 200},
            {"activity": 30, "wealth": 300},
        ]
        result = analyze.behavior_outcome_correlation(agents, "activity", "wealth")
        assert result["pearson"] == pytest.approx(1.0, abs=0.001)
        assert "strong" in result["interpretation"]


# =========================================================================
# Emergence Pattern Detection
# =========================================================================

class TestPhaseTransitions:
    def test_detect_transition(self, analyze):
        # Build series with a very clear jump at tick 15
        ts = [{"tick": i, "value": 10.0} for i in range(50)]
        # Insert a massive sudden jump at tick 15
        for i in range(15, 50):
            ts[i]["value"] = 1000.0
        result = analyze.detect_phase_transitions(ts, "value", window=5, threshold=2.0)
        assert len(result) > 0
        # The transition should be near tick 15
        ticks = [t["tick"] for t in result]
        assert any(14 <= t <= 16 for t in ticks)

    def test_no_transition(self, analyze):
        ts = [{"tick": i, "value": float(i)} for i in range(30)]
        result = analyze.detect_phase_transitions(ts, "value", window=5, threshold=5.0)
        # Linear growth shouldn't trigger z-score threshold
        assert len(result) == 0 or len(result) < 3

    def test_too_short(self, analyze):
        ts = [{"tick": i, "value": 1.0} for i in range(5)]
        result = analyze.detect_phase_transitions(ts, "value", window=10)
        assert result == []


class TestClustering:
    def test_basic_clustering(self, analyze):
        agents = [
            {"id": f"a{i}", "money": m, "tokens": t}
            for i, (m, t) in enumerate([
                (10, 10), (15, 12), (12, 14),  # Cluster 1
                (100, 100), (110, 105), (95, 98),  # Cluster 2
                (500, 500), (510, 490), (505, 505),  # Cluster 3
            ])
        ]
        result = analyze.detect_clustering(agents, ["money", "tokens"], k=3)
        assert len(result["clusters"]) == 3
        assert len(result["centroids"]) == 3
        assert sum(result["cluster_sizes"]) == 9

    def test_too_few_agents(self, analyze):
        agents = [{"id": "a1", "money": 10}]
        result = analyze.detect_clustering(agents, ["money"], k=3)
        assert result["clusters"] == []

    def test_missing_fields(self, analyze):
        agents = [{"id": "a1"}, {"id": "a2"}]
        result = analyze.detect_clustering(agents, ["money"], k=2)
        assert result["clusters"] == []


class TestEmergentPatterns:
    def test_detect_spike(self, analyze):
        ts = [{"tick": i, "money": 100.0, "tokens": 50.0} for i in range(50)]
        # Simultaneous massive spike at tick 35
        ts[35]["money"] = 5000.0
        ts[35]["tokens"] = 2500.0
        result = analyze.detect_emergent_patterns(
            ts, ["money", "tokens"], window=10, variance_threshold=2.0
        )
        assert len(result["emergence_events"]) > 0
        ticks = [e["tick"] for e in result["emergence_events"]]
        assert 35 in ticks

    def test_no_spikes(self, analyze):
        ts = [{"tick": i, "money": float(i * 10), "tokens": float(i * 5)} for i in range(50)]
        result = analyze.detect_emergent_patterns(
            ts, ["money", "tokens"], window=10, variance_threshold=5.0
        )
        # Linear growth should have no emergence events
        assert len(result["emergence_events"]) == 0


class TestPowerLaw:
    def test_power_law_distribution(self, analyze):
        # Zipf-like distribution: rank 1 has 100, rank 2 has 50, etc.
        values = [100.0 / (i + 1) for i in range(100)]
        result = analyze.detect_power_law(values)
        assert result["is_power_law"] is True
        assert result["exponent"] < -0.5
        assert result["r_squared"] > 0.8

    def test_uniform_not_power_law(self, analyze):
        values = [50.0] * 100
        result = analyze.detect_power_law(values)
        assert result["is_power_law"] is False

    def test_too_few(self, analyze):
        result = analyze.detect_power_law([1, 2, 3])
        assert result["is_power_law"] is False


# =========================================================================
# Statistical Significance Testing
# =========================================================================

class TestTTest:
    def test_significantly_different(self, analyze):
        a = [10, 12, 14, 16, 18, 20, 22, 24, 26, 28]
        b = [30, 32, 34, 36, 38, 40, 42, 44, 46, 48]
        result = analyze.t_test(a, b)
        assert result["significant_at_005"] is True
        assert result["t_statistic"] < 0  # a < b
        assert result["p_value"] < 0.05

    def test_not_significant(self, analyze):
        a = [10, 11, 12, 13, 14]
        b = [11, 12, 13, 14, 15]
        result = analyze.t_test(a, b)
        # These overlap heavily
        assert result["p_value"] > 0.01

    def test_empty_groups(self, analyze):
        result = analyze.t_test([], [])
        assert result["significant_at_005"] is False

    def test_single_element(self, analyze):
        result = analyze.t_test([1], [2])
        assert result["significant_at_005"] is False


class TestMannWhitney:
    def test_clear_separation(self, analyze):
        a = [1, 2, 3, 4, 5]
        b = [10, 11, 12, 13, 14]
        result = analyze.mann_whitney_u(a, b)
        assert result["significant_at_005"] is True
        assert result["z_score"] != 0

    def test_overlapping(self, analyze):
        a = [1, 3, 5, 7, 9]
        b = [2, 4, 6, 8, 10]
        result = analyze.mann_whitney_u(a, b)
        # These interleave, should not be significant
        assert result["p_value"] > 0.01

    def test_empty(self, analyze):
        result = analyze.mann_whitney_u([], [1, 2, 3])
        assert result["u_statistic"] == 0


class TestChiSquared:
    def test_uniform_not_significant(self, analyze):
        observed = [50, 50, 50, 50]
        result = analyze.chi_squared_test(observed)
        assert result["chi2_statistic"] == pytest.approx(0.0, abs=0.01)
        assert result["significant_at_005"] is False

    def test_skewed_significant(self, analyze):
        observed = [100, 1, 1, 1]
        result = analyze.chi_squared_test(observed)
        assert result["chi2_statistic"] > 10
        assert result["significant_at_005"] is True

    def test_with_expected(self, analyze):
        observed = [10, 20, 30]
        expected = [20, 20, 20]
        result = analyze.chi_squared_test(observed, expected)
        assert result["chi2_statistic"] > 0

    def test_empty(self, analyze):
        result = analyze.chi_squared_test([])
        assert result["chi2_statistic"] == 0.0


# =========================================================================
# Cultural Analysis
# =========================================================================

class TestCulturalDiversity:
    def test_diverse(self, analyze):
        data = [
            {"phase": "Adult"}, {"phase": "Elder"}, {"phase": "Child"},
            {"phase": "Adult"}, {"phase": "Elder"},
        ]
        result = analyze.cultural_diversity(data)
        assert result["unique_phases"] == 3
        assert result["shannon_entropy"] > 0
        assert result["simpson_index"] > 0

    def test_uniform(self, analyze):
        data = [{"phase": "Adult"}] * 10
        result = analyze.cultural_diversity(data)
        assert result["shannon_entropy"] == 0.0
        assert result["unique_phases"] == 1

    def test_empty(self, analyze):
        result = analyze.cultural_diversity([])
        assert result["shannon_entropy"] == 0.0


class TestCulturalEvolution:
    def test_evolution(self, analyze):
        snapshots = [
            {"tick": 0, "phase": "A"},
            {"tick": 0, "phase": "A"},
            {"tick": 0, "phase": "B"},
            {"tick": 1, "phase": "A"},
            {"tick": 1, "phase": "B"},
            {"tick": 1, "phase": "B"},
        ]
        result = analyze.cultural_evolution(snapshots)
        assert len(result["entropy_trajectory"]) == 2
        assert len(result["diversity_trajectory"]) == 2

    def test_empty(self, analyze):
        result = analyze.cultural_evolution([])
        assert result["entropy_trajectory"] == []


# =========================================================================
# Network Analysis
# =========================================================================

class TestTrustNetwork:
    def test_basic(self, analyze, sample_edges):
        result = analyze.trust_network(sample_edges)
        assert result["node_count"] == 4
        assert result["edge_count"] == 4
        assert result["avg_degree"] > 0
        assert 0 < result["density"] < 1

    def test_empty(self, analyze):
        result = analyze.trust_network([])
        assert result["node_count"] == 0


# =========================================================================
# Agent Behavior Trajectory
# =========================================================================

class TestAgentTrajectory:
    def test_basic_trajectory(self, analyze, sample_events):
        result = analyze.agent_trajectory(sample_events, "a1")
        assert result["agent_id"] == "a1"
        assert len(result["action_sequence"]) == 3
        assert result["total_events"] == 3
        assert len(result["phase_transitions"]) == 1  # Adult -> Elder

    def test_no_events(self, analyze):
        result = analyze.agent_trajectory([], "unknown")
        assert result["total_events"] == 0

    def test_decision_tree(self, analyze, sample_events):
        result = analyze.decision_tree(sample_events, "a2")
        assert result["agent_id"] == "a2"
        assert result["depth"] == 3
        # Last event has context and outcome
        assert "context" in result["nodes"][2]
        assert "outcome" in result["nodes"][2]


# =========================================================================
# Economic Time Series
# =========================================================================

class TestEconomicTimeSeries:
    def test_basic(self, analyze, sample_history):
        result = analyze.economic_time_series(sample_history)
        assert "total_money" in result["series"]
        assert result["tick_count"] == 10
        assert result["series"]["total_money"]["change_pct"] > 0

    def test_custom_fields(self, analyze, sample_history):
        result = analyze.economic_time_series(sample_history, fields=["total_money"])
        assert "total_money" in result["series"]
        assert "total_tokens" not in result["series"]

    def test_empty(self, analyze):
        result = analyze.economic_time_series([])
        assert result["series"] == {}


# =========================================================================
# World Summary
# =========================================================================

class TestWorldSummary:
    def test_basic(self, analyze, sample_agents):
        result = analyze.world_summary(sample_agents)
        assert result["population"] == 5
        assert result["alive_count"] == 4
        assert result["dead_count"] == 1
        assert result["survival_rate"] == 0.8
        assert "wealth" in result
        assert "tokens" in result

    def test_with_history(self, analyze, sample_agents, sample_history):
        result = analyze.world_summary(sample_agents, history=sample_history)
        assert "economic_trend" in result

    def test_empty(self, analyze):
        result = analyze.world_summary([])
        assert result["population"] == 0


# =========================================================================
# Module-level Helper Tests
# =========================================================================

class TestHelpers:
    def test_linear_regression(self):
        from agent_world_sdk.analyze import _linear_regression
        xs = [1, 2, 3, 4, 5]
        ys = [2, 4, 6, 8, 10]
        slope, intercept, r_sq = _linear_regression(xs, ys)
        assert slope == pytest.approx(2.0, abs=0.01)
        assert intercept == pytest.approx(0.0, abs=0.01)
        assert r_sq == pytest.approx(1.0, abs=0.01)

    def test_rank(self):
        from agent_world_sdk.analyze import _rank
        ranks = _rank([3, 1, 4, 1, 5])
        assert ranks == pytest.approx([3.0, 1.5, 4.0, 1.5, 5.0])

    def test_normal_cdf(self):
        from agent_world_sdk.analyze import _normal_cdf
        assert _normal_cdf(0) == pytest.approx(0.5, abs=0.01)
        assert _normal_cdf(1.96) > 0.97
        assert _normal_cdf(-1.96) < 0.03
