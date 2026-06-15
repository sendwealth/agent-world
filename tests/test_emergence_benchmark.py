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


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
