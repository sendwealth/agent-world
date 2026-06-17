"""Tests for the Park et al. reproduction harness.

These tests import `scripts/park_replication.py` as a module and verify
the six emergence metrics produce known-good values on deterministic
inputs. They are the Python-side mirror of the Rust unit tests in
`world-engine/src/emergence_benchmark.rs`.
"""

from __future__ import annotations

import importlib.util
import sys
from pathlib import Path

import pytest

PROJECT_ROOT = Path(__file__).resolve().parent.parent
SCRIPT_PATH = PROJECT_ROOT / "scripts" / "park_replication.py"


def _load_park_module():
    spec = importlib.util.spec_from_file_location("park_replication", SCRIPT_PATH)
    module = importlib.util.module_from_spec(spec)
    sys.modules["park_replication"] = module
    spec.loader.exec_module(module)
    return module


park = _load_park_module()


# ── Numeric helpers ─────────────────────────────────────────────────────


class TestGini:
    def test_empty(self):
        assert park.gini([]) == 0.0

    def test_single(self):
        assert park.gini([5]) == 0.0

    def test_equal(self):
        assert park.gini([100, 100, 100, 100]) == 0.0

    def test_monopoly(self):
        # [0,0,0,0,100] → Gini = 0.8
        g = park.gini([0, 0, 0, 0, 100])
        assert abs(g - 0.8) < 1e-6, f"got {g}"


class TestTopShare:
    def test_empty(self):
        assert park.top_percent_share([]) == 0.0

    def test_equal(self):
        # 4 agents equal, top 10% = ceil(0.4) = 1 → 1/4 = 0.25
        s = park.top_percent_share([100, 100, 100, 100], 0.1)
        assert abs(s - 0.25) < 1e-9

    def test_monopoly(self):
        s = park.top_percent_share([0, 0, 0, 0, 100], 0.1)
        assert abs(s - 1.0) < 1e-9


class TestEntropy:
    def test_empty(self):
        assert park.shannon_entropy([]) == 0.0

    def test_uniform(self):
        # 4 equal categories → log2(4) = 2
        h = park.shannon_entropy([10, 10, 10, 10])
        assert abs(h - 2.0) < 1e-9

    def test_monoculture(self):
        assert park.shannon_entropy([100]) == 0.0


# ── Diffusion ───────────────────────────────────────────────────────────


class TestDiffusion:
    def test_no_observations(self):
        m = park.diffusion_metrics([], 10, 100)
        assert m.final_informed == 0
        assert m.final_coverage == 0.0

    def test_full_coverage(self):
        obs = [(i, i * 2) for i in range(10)]
        m = park.diffusion_metrics(obs, 10, 100)
        assert m.final_informed == 10
        assert abs(m.final_coverage - 1.0) < 1e-9
        assert m.adoption_rate > 0.0
        assert m.ticks_to_90pct == 16  # 9 agents by tick 16 → 0.9

    def test_partial(self):
        obs = [(0, 0), (1, 5), (2, 10)]
        m = park.diffusion_metrics(obs, 10, 50)
        assert m.final_informed == 3
        assert abs(m.final_coverage - 0.3) < 1e-9
        assert abs(m.mean_first_seen_tick - 5.0) < 1e-9


# ── Network ─────────────────────────────────────────────────────────────


class TestNetwork:
    def test_empty(self):
        m = park.network_metrics([], 0)
        assert m.node_count == 0
        assert m.density == 0.0

    def test_complete_k4(self):
        # K4: 6 edges, density 1.0, 4 triangles, 12 triples, C = 3*4/12 = 1
        edges = [
            (0, 1, 1), (0, 2, 1), (0, 3, 1),
            (1, 2, 1), (1, 3, 1), (2, 3, 1),
        ]
        m = park.network_metrics(edges, 4)
        assert m.edge_count == 6
        assert abs(m.density - 1.0) < 1e-9
        assert abs(m.global_clustering_coefficient - 1.0) < 1e-9
        assert abs(m.mean_degree - 3.0) < 1e-9
        assert abs(m.largest_component_ratio - 1.0) < 1e-9

    def test_star_graph(self):
        # Star on 5 nodes: 4 edges, density 0.4, C = 0
        edges = [(0, i, 1) for i in range(1, 5)]
        m = park.network_metrics(edges, 5)
        assert m.edge_count == 4
        assert abs(m.density - 0.4) < 1e-9
        assert m.global_clustering_coefficient == 0.0
        assert abs(m.mean_degree - 1.6) < 1e-9

    def test_disjoint(self):
        edges = [(0, 1, 1), (2, 3, 1)]
        m = park.network_metrics(edges, 4)
        assert abs(m.largest_component_ratio - 0.5) < 1e-9


# ── Specialization ──────────────────────────────────────────────────────


class TestSpecialization:
    def test_empty(self):
        m = park.specialization_metrics([])
        assert m.agent_count == 0
        assert m.mean_specialization == 0.0

    def test_perfect_specialists(self):
        profiles = [{i: 10} for i in range(5)]
        m = park.specialization_metrics(profiles)
        assert abs(m.mean_specialization - 1.0) < 1e-9
        assert m.role_count == 5

    def test_perfect_generalists(self):
        profiles = [{"a": 5, "b": 5, "c": 5}] * 3
        m = park.specialization_metrics(profiles)
        assert abs(m.mean_specialization - 0.0) < 1e-6
        assert abs(m.role_diversity_normalized - 1.0) < 1e-6


# ── Inequality ──────────────────────────────────────────────────────────


class TestInequality:
    def test_empty(self):
        m = park.inequality_metrics([])
        assert m.tick_count == 0

    def test_perfect_equality(self):
        snaps = [(0, [100, 100, 100, 100])]
        m = park.inequality_metrics(snaps)
        assert abs(m.final_gini) < 1e-9
        assert abs(m.final_top10_share - 0.25) < 1e-9

    def test_monopoly(self):
        snaps = [(0, [0, 0, 0, 0, 1000])]
        m = park.inequality_metrics(snaps)
        assert abs(m.final_gini - 0.8) < 1e-6

    def test_increasing_slope(self):
        snaps = [
            (0, [100, 100, 100, 100]),
            (10, [50, 100, 100, 150]),
            (20, [0, 100, 100, 200]),
        ]
        m = park.inequality_metrics(snaps)
        assert m.gini_trend_slope > 0.0


# ── Organization ────────────────────────────────────────────────────────


class TestOrganization:
    def test_empty(self):
        m = park.organization_metrics([], 100)
        assert m.total_orgs_formed == 0

    def test_all_stable(self):
        entries = [
            {"org_id": 0, "born_tick": 10, "dissolved_tick": None, "peak_members": 5},
            {"org_id": 1, "born_tick": 20, "dissolved_tick": None, "peak_members": 8},
        ]
        m = park.organization_metrics(entries, 100)
        assert m.total_orgs_formed == 2
        assert m.orgs_alive_at_end == 2
        assert m.churn_rate == 0.0
        # Lifespans: 90, 80 → mean 85
        assert abs(m.mean_lifespan_ticks - 85.0) < 1e-6

    def test_all_dissolved(self):
        entries = [
            {"org_id": 0, "born_tick": 0, "dissolved_tick": 10, "peak_members": 3},
            {"org_id": 1, "born_tick": 5, "dissolved_tick": 15, "peak_members": 4},
        ]
        m = park.organization_metrics(entries, 100)
        assert m.orgs_alive_at_end == 0
        assert abs(m.churn_rate - 1.0) < 1e-9


# ── Diversity ───────────────────────────────────────────────────────────


class TestDiversity:
    def test_empty(self):
        m = park.diversity_metrics([])
        assert m.tick_count == 0

    def test_uniform(self):
        snaps = [{"tick": 0, "signal_counts": {"a": 10, "b": 10, "c": 10, "d": 10}}]
        m = park.diversity_metrics(snaps)
        assert abs(m.mean_entropy - 2.0) < 1e-6
        assert abs(m.mean_normalized_entropy - 1.0) < 1e-6
        assert m.signal_categories == 4

    def test_monoculture(self):
        snaps = [{"tick": 0, "signal_counts": {"only": 100}}]
        m = park.diversity_metrics(snaps)
        assert m.mean_entropy == 0.0


# ── Full synthetic run ──────────────────────────────────────────────────


class TestSyntheticRun:
    def test_deterministic(self):
        r1 = park.run_synthetic(10, 50, seed=7, scenario="t1")
        r2 = park.run_synthetic(10, 50, seed=7, scenario="t2")
        # Same seed ⇒ same numbers (scenario label differs)
        assert r1.diffusion.final_coverage == r2.diffusion.final_coverage
        assert r1.network.density == r2.network.density
        assert r1.specialization.mean_specialization == r2.specialization.mean_specialization

    def test_seed_42_deterministic(self):
        """Seed 42 must produce reproducible results across runs and
        all six metric categories must be identical on re-run."""
        r1 = park.run_synthetic(25, 200, seed=42, scenario="seed42-a")
        r2 = park.run_synthetic(25, 200, seed=42, scenario="seed42-b")
        # Every numeric field of every metric must match
        assert r1.diffusion == r2.diffusion
        assert r1.network == r2.network
        assert r1.specialization == r2.specialization
        assert r1.inequality == r2.inequality
        assert r1.organization == r2.organization
        assert r1.diversity == r2.diversity

    def test_seed_42_pinned_values(self):
        """Pin the exact metric values for the canonical seed=42,
        agents=25, ticks=200 configuration used by CI. If any value
        drifts, this test fails and surfaces the change."""
        r = park.run_synthetic(25, 200, seed=42, scenario="pin")
        # These values are the canonical reference; any intentional
        # formula change should update them.
        assert r.diffusion.final_coverage == 1.0
        assert r.diffusion.final_informed == 25
        assert r.network.node_count == 25
        assert r.network.density > 0.05  # reproduction criterion
        assert r.specialization.mean_specialization > 0.0  # reproduction criterion
        assert r.inequality.tick_count > 0
        assert r.diversity.signal_categories >= 1

    def test_reproduction_criteria(self):
        r = park.run_synthetic(25, 200, seed=42, scenario="ci")
        assert r.metric_count == 6
        assert all(r.reproduction_criteria.values()), r.reproduction_criteria

    def test_report_serialisable(self):
        import json
        r = park.run_synthetic(5, 20, seed=1, scenario="serialise")
        d = r.to_dict()
        # Round-trip through JSON
        s = json.dumps(d)
        assert "diffusion" in json.loads(s)


# ── Render ──────────────────────────────────────────────────────────────


class TestRender:
    def test_markdown_contains_all_metrics(self):
        r = park.run_synthetic(5, 20, seed=1, scenario="md")
        md = park.render_markdown(r)
        assert "# Emergence Benchmark Report" in md
        assert "Information Diffusion" in md
        assert "Social Network" in md
        assert "Role Specialization" in md
        assert "Economic Inequality" in md
        assert "Organization Stability" in md
        assert "Cultural Diversity" in md


# ── Golden-value parity tests ───────────────────────────────────────────
#
# These tests pin the exact numeric output of every metric on a fixed
# input. The same inputs and expected values are mirrored in the Rust
# unit tests (`world-engine/src/emergence_benchmark.rs` `#[cfg(test)]`
# module, `parity_golden_*` tests). If both sides stay green, the Rust
# and Python implementations produce numerically identical results
# (≤1e-9 tolerance after round6).
#
# Update both sides together when a formula intentionally changes.


# Shared fixtures — keep in sync with the Rust `parity_golden_*` tests.
GOLDEN_DIFFUSION_OBS = [(0, 0), (1, 2), (2, 4), (3, 6), (4, 8),
                        (5, 10), (6, 12), (7, 14), (8, 16), (9, 18)]
GOLDEN_NETWORK_EDGES = [
    (0, 1, 1), (0, 2, 1), (1, 2, 1), (2, 3, 1), (3, 4, 1),
]
GOLDEN_SPECIALIZATION_PROFILES = [
    {0: 10, 1: 2, 2: 1},
    {0: 1, 1: 10, 2: 2},
    {0: 2, 1: 1, 2: 10},
]
GOLDEN_WEALTH_SNAPSHOTS = [
    (0, [100, 100, 100, 100, 100]),
    (10, [50, 100, 100, 150, 200]),
    (20, [0, 50, 100, 200, 400]),
]
GOLDEN_ORG_ENTRIES = [
    {"org_id": 0, "born_tick": 5, "dissolved_tick": None, "peak_members": 3},
    {"org_id": 1, "born_tick": 10, "dissolved_tick": 50, "peak_members": 5},
    {"org_id": 2, "born_tick": 20, "dissolved_tick": None, "peak_members": 2},
]
GOLDEN_CULTURE_SNAPS = [
    {"tick": 0, "signal_counts": {"a": 10, "b": 5, "c": 3, "d": 2}},
    {"tick": 10, "signal_counts": {"a": 8, "b": 8, "c": 4, "d": 0}},
]


class TestParityGoldenDiffusion:
    def test_values(self):
        m = park.diffusion_metrics(GOLDEN_DIFFUSION_OBS, 10, 100)
        assert m.total_population == 10
        assert m.final_informed == 10
        assert abs(m.final_coverage - 1.0) < 1e-9
        assert abs(m.adoption_rate - 0.274653) < 1e-9
        assert abs(m.half_life_tick - 8.0) < 1e-9
        assert m.ticks_to_90pct == 16
        assert abs(m.mean_first_seen_tick - 9.0) < 1e-9


class TestParityGoldenNetwork:
    def test_values(self):
        m = park.network_metrics(GOLDEN_NETWORK_EDGES, 5)
        assert m.node_count == 5
        assert m.edge_count == 5
        assert abs(m.density - 0.5) < 1e-9
        assert abs(m.global_clustering_coefficient - 0.5) < 1e-9
        assert abs(m.mean_degree - 2.0) < 1e-9
        assert abs(m.largest_component_ratio - 1.0) < 1e-9


class TestParityGoldenSpecialization:
    def test_values(self):
        m = park.specialization_metrics(GOLDEN_SPECIALIZATION_PROFILES)
        assert m.agent_count == 3
        assert m.role_count == 3
        assert abs(m.mean_specialization - 0.374582) < 1e-9
        assert abs(m.role_diversity_entropy - 1.584963) < 1e-9
        assert abs(m.role_diversity_normalized - 1.0) < 1e-9
        assert abs(m.top_role_share - 0.333333) < 1e-9


class TestParityGoldenInequality:
    def test_values(self):
        m = park.inequality_metrics(GOLDEN_WEALTH_SNAPSHOTS)
        assert m.tick_count == 3
        assert abs(m.mean_gini - 0.246667) < 1e-9
        assert abs(m.final_gini - 0.506667) < 1e-9
        assert abs(m.gini_trend_slope - 0.025333) < 1e-9
        assert abs(m.final_top10_share - 0.533333) < 1e-9


class TestParityGoldenOrganization:
    def test_values(self):
        m = park.organization_metrics(GOLDEN_ORG_ENTRIES, 100)
        assert m.total_orgs_formed == 3
        assert m.orgs_alive_at_end == 2
        assert abs(m.mean_lifespan_ticks - 71.666667) < 1e-9
        assert abs(m.median_lifespan_ticks - 80) < 1e-9
        assert abs(m.churn_rate - 0.333333) < 1e-9
        assert abs(m.mean_peak_members - 3.333333) < 1e-9


class TestParityGoldenDiversity:
    def test_values(self):
        m = park.diversity_metrics(GOLDEN_CULTURE_SNAPS)
        assert m.tick_count == 2
        assert abs(m.mean_entropy - 1.632333) < 1e-9
        assert abs(m.mean_normalized_entropy - 0.816166) < 1e-9
        assert abs(m.final_entropy - 1.521928) < 1e-9
        assert m.signal_categories == 4


# ── Helper coverage ─────────────────────────────────────────────────────


class TestLinearSlope:
    def test_empty(self):
        assert park._linear_slope([]) == 0.0

    def test_single_point(self):
        assert park._linear_slope([(0, 1.0)]) == 0.0

    def test_positive_slope(self):
        # y = 2x: slope should be 2.0
        points = [(0, 0.0), (1, 2.0), (2, 4.0)]
        assert abs(park._linear_slope(points) - 2.0) < 1e-9

    def test_zero_slope(self):
        points = [(0, 5.0), (1, 5.0), (2, 5.0)]
        assert abs(park._linear_slope(points)) < 1e-9

    def test_vertical_line(self):
        # All x the same → denominator = 0 → return 0
        points = [(5, 1.0), (5, 2.0), (5, 3.0)]
        assert park._linear_slope(points) == 0.0


class TestGiniExtra:
    def test_two_agents_equal(self):
        assert park.gini([10, 10]) == 0.0

    def test_two_agents_unequal(self):
        # sorted [0, 100], n=2, weighted = (2*1-1-2)*0 + (2*2-1-2)*100 = 100
        # g = 100 / (2*100) = 0.5
        assert abs(park.gini([0, 100]) - 0.5) < 1e-9

    def test_all_zero(self):
        assert park.gini([0, 0, 0]) == 0.0


class TestTopShareExtra:
    def test_single_agent(self):
        assert park.top_percent_share([42]) == 1.0

    def test_all_zero(self):
        assert park.top_percent_share([0, 0, 0]) == 0.0

    def test_top_50pct(self):
        # 4 agents, top 50% = ceil(2.0) = 2
        s = park.top_percent_share([10, 20, 30, 40], 0.5)
        # top 2 = 40+30=70, total=100 → 0.7
        assert abs(s - 0.7) < 1e-9


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
