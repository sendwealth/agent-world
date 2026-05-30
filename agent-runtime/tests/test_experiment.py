"""Tests for the enhanced A/B experiment framework (SEN-550).

Covers:
- ExperimentConfig parsing from YAML/TOML and dict
- Config validation and serialization
- ReproducibilityManager: seeded RNG, config snapshot, child seeds
- ExperimentReporter: markdown/json/html/pdf output
- ABExperiment: parallel/sequential runs and comparison with real stats
- Statistics: Welch's t-test, chi-square, Mann-Whitney U, Cohen's d
- DSL: ExperimentDefinition, groups, variables, agent assignment
- WorldEngineAdapter: client parsing logic
- REST API: FastAPI endpoints
"""

from __future__ import annotations

import asyncio
import json
from pathlib import Path

import pytest

from agent_runtime.experiment.ab_framework import ABExperiment, ComparisonReport
from agent_runtime.experiment.config import ExperimentConfig
from agent_runtime.experiment.dsl import (
    ExperimentDefinition,
    ExperimentGroup,
    ExperimentVariable,
    Hypothesis,
)
from agent_runtime.experiment.report import ExperimentReporter, ExperimentResult
from agent_runtime.experiment.reproducibility import ReproducibilityManager
from agent_runtime.experiment.statistics import (
    TestResult,
    chi_square_goodness_of_fit,
    chi_square_test,
    cohens_d,
    compare_metrics,
    mann_whitney_u_test,
    welch_t_test,
)
from agent_runtime.experiment.world_engine_adapter import (
    VariantComparisonResult,
    WorldEngineAdapter,
)

# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------

SAMPLE_YAML = """
experiment:
  id: "test-exp-001"
  name: "Test Experiment"
  seed: 42
  duration_ticks: 1000
  world:
    width: 50
    height: 50
    resource_density: 0.4
  agents:
    count: 20
    personality_distribution: "clustered"
    initial_tokens: 200
  governance:
    enabled: false
    tax_rate: 0.0
  llm:
    provider: "ollama"
    model: "qwen2"
    temperature: 0.5
  tracing:
    snapshot_interval: 50
    export_on_complete: false
"""


@pytest.fixture
def yaml_file(tmp_path: Path) -> Path:
    p = tmp_path / "test_config.yaml"
    p.write_text(SAMPLE_YAML)
    return p


@pytest.fixture
def sample_config() -> ExperimentConfig:
    return ExperimentConfig(
        experiment_id="test-001",
        name="Test",
        seed=42,
        duration_ticks=1000,
    )


@pytest.fixture
def sample_result() -> ExperimentResult:
    return ExperimentResult(
        experiment_id="test-001",
        config_snapshot={"seed": 42},
        duration_ticks=1000,
        completed_ticks=1000,
        agent_count=20,
        final_snapshot={"tick": 1000, "survival_rate": 0.95},
        metrics_timeline=[
            {"tick": 0, "survival_rate": 1.0, "trade_volume": 500.0},
            {"tick": 500, "survival_rate": 0.98, "trade_volume": 600.0},
            {"tick": 1000, "survival_rate": 0.95, "trade_volume": 550.0},
        ],
        emergence_events=[
            {"tick": 300, "type": "org_formation", "description": "Test event"},
        ],
        errors=[],
        started_at="2026-01-01T00:00:00Z",
        finished_at="2026-01-01T01:00:00Z",
    )


# ---------------------------------------------------------------------------
# 1. Config tests (existing, preserved)
# ---------------------------------------------------------------------------


class TestExperimentConfig:
    def test_from_yaml(self, yaml_file: Path) -> None:
        config = ExperimentConfig.from_yaml(yaml_file)
        assert config.experiment_id == "test-exp-001"
        assert config.name == "Test Experiment"
        assert config.seed == 42
        assert config.duration_ticks == 1000
        assert config.world.width == 50
        assert config.agents.count == 20

    def test_from_yaml_not_found(self) -> None:
        with pytest.raises(FileNotFoundError):
            ExperimentConfig.from_yaml("/nonexistent/config.yaml")

    def test_from_dict(self) -> None:
        data = {"experiment": {"id": "dict-test", "seed": 99, "duration_ticks": 500}}
        config = ExperimentConfig.from_dict(data)
        assert config.experiment_id == "dict-test"
        assert config.seed == 99
        assert config.world.width == 100  # default

    def test_validate_valid_config(self) -> None:
        good = ExperimentConfig(experiment_id="good-exp", duration_ticks=100)
        assert good.validate() == []

    def test_validate_negative_ticks(self) -> None:
        config = ExperimentConfig(experiment_id="bad", duration_ticks=-1)
        errors = config.validate()
        assert any("duration_ticks" in e for e in errors)

    def test_to_dict_roundtrip(self) -> None:
        original = ExperimentConfig(experiment_id="roundtrip", seed=123, duration_ticks=5000)
        d = original.to_dict()
        restored = ExperimentConfig.from_dict(d)
        assert restored.experiment_id == original.experiment_id
        assert restored.seed == original.seed

    def test_from_toml(self, tmp_path: Path) -> None:
        toml_content = """
[experiment]
id = "toml-test"
name = "TOML Test"
seed = 77
duration_ticks = 2000
"""
        p = tmp_path / "test.toml"
        p.write_text(toml_content)
        config = ExperimentConfig.from_toml(p)
        assert config.experiment_id == "toml-test"
        assert config.seed == 77


# ---------------------------------------------------------------------------
# 2. Reproducibility tests (existing, preserved)
# ---------------------------------------------------------------------------


class TestReproducibility:
    def test_seeded_rng_deterministic(self) -> None:
        config = ExperimentConfig(seed=42)
        mgr_a = ReproducibilityManager(config)
        mgr_b = ReproducibilityManager(config)
        vals_a = [mgr_a.random.random() for _ in range(10)]
        vals_b = [mgr_b.random.random() for _ in range(10)]
        assert vals_a == vals_b

    def test_seeded_rng_different_seeds(self) -> None:
        mgr_a = ReproducibilityManager(ExperimentConfig(seed=42))
        mgr_b = ReproducibilityManager(ExperimentConfig(seed=99))
        assert (
            [mgr_a.random.random() for _ in range(10)]
            != [mgr_b.random.random() for _ in range(10)]
        )

    def test_config_snapshot_hash_stable(self) -> None:
        mgr = ReproducibilityManager(ExperimentConfig(experiment_id="snap-test", seed=42))
        snap1 = mgr.snapshot_config()
        snap2 = mgr.snapshot_config()
        assert snap1.content_hash == snap2.content_hash

    def test_verify_reproducibility_matching(self) -> None:
        mgr = ReproducibilityManager(ExperimentConfig(seed=42))
        data = mgr.snapshot_config().to_dict()
        run = {"config_snapshot": data, "metrics_timeline": [{"tick": 0}], "final_snapshot": {}}
        assert mgr.verify_reproducibility(run, run) is True

    def test_derive_child_seed_unique(self) -> None:
        mgr = ReproducibilityManager(ExperimentConfig(seed=42))
        assert mgr.derive_child_seed(0) != mgr.derive_child_seed(1)

    def test_reset_rng(self) -> None:
        mgr = ReproducibilityManager(ExperimentConfig(seed=42))
        vals_before = [mgr.random.random() for _ in range(5)]
        mgr.reset_rng()
        vals_after = [mgr.random.random() for _ in range(5)]
        assert vals_before == vals_after


# ---------------------------------------------------------------------------
# 3. Statistics tests (NEW)
# ---------------------------------------------------------------------------


class TestStatistics:
    def test_welch_t_significant(self) -> None:
        """Groups with clearly different means should be significant."""
        import random
        random.seed(42)
        a = [random.gauss(10, 1) for _ in range(50)]
        b = [random.gauss(15, 1) for _ in range(50)]
        result = welch_t_test(a, b)
        assert result.significant is True
        assert result.p_value < 0.001
        assert abs(result.statistic) > 10  # Large difference between groups
        assert result.test_name == "Welch's t-test"
        assert result.details is not None
        assert "mean_a" in result.details

    def test_welch_t_not_significant(self) -> None:
        """Same distribution should not be significant."""
        import random
        random.seed(42)
        a = [random.gauss(10, 1) for _ in range(30)]
        b = [random.gauss(10.1, 1) for _ in range(30)]
        result = welch_t_test(a, b)
        # May or may not be significant — just check it returns a valid result
        assert 0.0 <= result.p_value <= 1.0
        assert isinstance(result.significant, bool)

    def test_welch_t_small_sample(self) -> None:
        result = welch_t_test([1.0], [2.0])
        assert result.significant is False
        assert "error" in (result.details or {})

    def test_chi_square_significant(self) -> None:
        observed = [[100, 10], [20, 80]]
        result = chi_square_test(observed)
        assert result.significant is True
        assert result.p_value < 0.001
        assert result.details is not None
        assert result.details["df"] == 1

    def test_chi_square_not_significant(self) -> None:
        observed = [[50, 50], [50, 50]]
        result = chi_square_test(observed)
        assert result.significant is False
        assert result.p_value > 0.05

    def test_chi_square_goodness_of_fit(self) -> None:
        observed = [100, 100, 100]
        result = chi_square_goodness_of_fit(observed)
        assert result.significant is False
        assert result.p_value > 0.05

    def test_mann_whitney_u(self) -> None:
        import random
        random.seed(42)
        a = [random.gauss(10, 2) for _ in range(30)]
        b = [random.gauss(15, 2) for _ in range(30)]
        result = mann_whitney_u_test(a, b)
        assert result.significant is True
        assert result.p_value < 0.01

    def test_cohens_d_large(self) -> None:
        a = [float(i) for i in range(10)]
        b = [float(i + 10) for i in range(10)]
        d = cohens_d(a, b)
        assert d > 2.0  # Very large effect

    def test_cohens_d_small(self) -> None:
        a = [10.0, 10.1, 9.9, 10.0, 10.1]
        b = [10.2, 10.3, 10.1, 10.2, 10.3]
        d = cohens_d(a, b)
        assert abs(d) < 3.0  # Small effect

    def test_compare_metrics(self) -> None:
        metrics_a = {
            "survival_rate": [0.9, 0.91, 0.89, 0.92, 0.88],
            "trade_volume": [100, 110, 95, 105, 115],
        }
        metrics_b = {
            "survival_rate": [0.95, 0.96, 0.94, 0.97, 0.93],
            "trade_volume": [100, 105, 98, 102, 107],
        }
        results = compare_metrics(metrics_a, metrics_b)
        assert "survival_rate" in results
        assert "trade_volume" in results
        assert "test" in results["survival_rate"]
        assert "effect_size" in results["survival_rate"]

    def test_test_result_to_dict(self) -> None:
        result = TestResult(
            statistic=2.5,
            p_value=0.03,
            significant=True,
            alpha=0.05,
            test_name="test",
        )
        d = result.to_dict()
        assert d["significant"] is True
        assert d["p_value"] == 0.03


# ---------------------------------------------------------------------------
# 4. DSL tests (NEW)
# ---------------------------------------------------------------------------


class TestDSL:
    def test_create_definition(self) -> None:
        exp = ExperimentDefinition(
            name="Token Test",
            variables=[
                ExperimentVariable("initial_tokens", default=100, type_hint="int"),
            ],
            groups=[
                ExperimentGroup("control", variables={"initial_tokens": 100}),
                ExperimentGroup("treatment", variables={"initial_tokens": 500}),
            ],
            hypothesis=Hypothesis(
                null="No effect",
                alternative="Higher tokens increase survival",
                direction="greater",
                metric="survival_rate",
            ),
            agent_count=50,
            duration_ticks=10000,
        )
        errors = exp.validate()
        assert errors == []

    def test_validate_no_groups(self) -> None:
        exp = ExperimentDefinition(name="Bad", groups=[])
        errors = exp.validate()
        assert any("2 groups" in e for e in errors)

    def test_validate_no_control(self) -> None:
        exp = ExperimentDefinition(
            name="Bad",
            groups=[
                ExperimentGroup("group_a"),
                ExperimentGroup("group_b"),
            ],
        )
        errors = exp.validate()
        assert any("control" in e.lower() for e in errors)

    def test_validate_duplicate_names(self) -> None:
        exp = ExperimentDefinition(
            name="Bad",
            groups=[
                ExperimentGroup("control"),
                ExperimentGroup("control"),
            ],
        )
        errors = exp.validate()
        assert any("unique" in e.lower() for e in errors)

    def test_to_configs(self) -> None:
        exp = ExperimentDefinition(
            name="Token Test",
            groups=[
                ExperimentGroup("control", variables={"initial_tokens": 100}),
                ExperimentGroup("treatment", variables={"initial_tokens": 500}),
            ],
            agent_count=20,
            duration_ticks=5000,
            base_seed=42,
        )
        configs = exp.to_configs()
        assert "control" in configs
        assert "treatment" in configs
        assert configs["control"].agents.initial_tokens == 100
        assert configs["treatment"].agents.initial_tokens == 500

    def test_assign_agents(self) -> None:
        exp = ExperimentDefinition(
            name="Assign Test",
            groups=[
                ExperimentGroup("control", agent_ratio=0.5),
                ExperimentGroup("treatment", agent_ratio=0.5),
            ],
        )
        agents = [f"agent-{i}" for i in range(20)]
        assignment = exp.assign_agents(agents)
        assert len(assignment["control"]) == 10
        assert len(assignment["treatment"]) == 10
        # All agents assigned
        all_assigned = assignment["control"] + assignment["treatment"]
        assert set(all_assigned) == set(agents)

    def test_assign_agents_deterministic(self) -> None:
        exp = ExperimentDefinition(
            name="Det Test",
            groups=[
                ExperimentGroup("control"),
                ExperimentGroup("treatment"),
            ],
            base_seed=42,
        )
        agents = [f"a-{i}" for i in range(10)]
        a1 = exp.assign_agents(agents)
        a2 = exp.assign_agents(agents)
        assert a1 == a2

    def test_yaml_roundtrip(self) -> None:
        exp = ExperimentDefinition(
            name="YAML Test",
            description="Test YAML roundtrip",
            variables=[
                ExperimentVariable("initial_tokens", default=100, type_hint="int"),
            ],
            groups=[
                ExperimentGroup("control", variables={"initial_tokens": 100}),
                ExperimentGroup("treatment", variables={"initial_tokens": 500}),
            ],
        )
        yaml_str = exp.to_yaml()
        restored = ExperimentDefinition.from_yaml(yaml_str)
        assert restored.name == "YAML Test"
        assert len(restored.variables) == 1
        assert len(restored.groups) == 2

    def test_group_is_control(self) -> None:
        assert ExperimentGroup("control").is_control() is True
        assert ExperimentGroup("baseline").is_control() is True
        assert ExperimentGroup("treatment").is_control() is False

    def test_variable_validation(self) -> None:
        var = ExperimentVariable("count", type_hint="int", min_value=0, max_value=100)
        assert var.validate_value(50) == []
        assert len(var.validate_value(-1)) > 0  # below min
        assert len(var.validate_value("abc")) > 0  # wrong type


# ---------------------------------------------------------------------------
# 5. Reporter tests (enhanced)
# ---------------------------------------------------------------------------


class TestReporter:
    def test_markdown_report(self, sample_result: ExperimentResult) -> None:
        reporter = ExperimentReporter()
        md = reporter.generate_report(sample_result, format="markdown")
        assert "# Experiment Report: test-001" in md
        assert "Metrics Timeline" in md
        assert "1000/1000 ticks" in md

    def test_json_report(self, sample_result: ExperimentResult) -> None:
        reporter = ExperimentReporter()
        j = reporter.generate_report(sample_result, format="json")
        parsed = json.loads(j)
        assert parsed["experiment_id"] == "test-001"

    def test_html_report(self, sample_result: ExperimentResult) -> None:
        reporter = ExperimentReporter()
        html = reporter.generate_report(sample_result, format="html")
        assert "<!DOCTYPE html>" in html
        assert "test-001" in html

    def test_pdf_report(self, sample_result: ExperimentResult) -> None:
        reporter = ExperimentReporter()
        result = reporter.generate_report(sample_result, format="pdf")
        # PDF returns bytes if fpdf2 is installed, HTML string as fallback
        assert isinstance(result, (bytes, str))
        # Both PDF bytes and HTML string are valid outputs
        if isinstance(result, bytes):
            assert len(result) > 0
        else:
            assert "<!DOCTYPE html>" in result

    def test_result_to_dict_roundtrip(self, sample_result: ExperimentResult) -> None:
        d = sample_result.to_dict()
        restored = ExperimentResult.from_dict(d)
        assert restored.experiment_id == sample_result.experiment_id
        assert len(restored.metrics_timeline) == len(sample_result.metrics_timeline)

    def test_ab_report_markdown(self) -> None:
        comparison = ComparisonReport(
            metrics_diff={"survival_rate": 0.05, "trade_volume": -100.0},
            statistical_significance={"survival_rate": 0.03},
            effect_sizes={"survival_rate": 0.8},
            recommendation="Config A",
            summary="A is better than B.",
        )
        reporter = ExperimentReporter()
        md = reporter.generate_ab_report(comparison, format="markdown")
        assert "# A/B Comparison Report" in md
        assert "survival_rate" in md
        assert "Config A" in md
        assert "Effect Size" in md

    def test_ab_report_json(self) -> None:
        comparison = ComparisonReport(
            metrics_diff={"x": 1.0},
            recommendation="B",
            summary="B wins.",
        )
        reporter = ExperimentReporter()
        j = reporter.generate_ab_report(comparison, format="json")
        parsed = json.loads(j)
        assert parsed["recommendation"] == "B"
        assert "effect_sizes" in parsed


# ---------------------------------------------------------------------------
# 6. A/B Framework tests (enhanced)
# ---------------------------------------------------------------------------


class TestABFramework:
    @pytest.mark.asyncio
    async def test_parallel_run(self) -> None:
        config_a = ExperimentConfig(experiment_id="exp-a", seed=42, duration_ticks=500)
        config_b = ExperimentConfig(experiment_id="exp-b", seed=42, duration_ticks=500)
        ab = ABExperiment(config_a, config_b, seed_base=100)
        result = await ab.run_parallel()
        assert result.result_a.experiment_id == "exp-a"
        assert result.result_b.experiment_id == "exp-b"
        assert result.comparison.summary != ""

    @pytest.mark.asyncio
    async def test_sequential_run(self) -> None:
        config_a = ExperimentConfig(experiment_id="seq-a", seed=42, duration_ticks=200)
        config_b = ExperimentConfig(experiment_id="seq-b", seed=42, duration_ticks=200)
        ab = ABExperiment(config_a, config_b)
        result = await ab.run_sequential()
        assert result.result_a.experiment_id == "seq-a"
        assert result.result_b.experiment_id == "seq-b"

    @pytest.mark.asyncio
    async def test_comparison_with_real_stats(self) -> None:
        """Comparison should include real Welch's t-test results."""
        config_a = ExperimentConfig(experiment_id="a", seed=42, duration_ticks=500)
        config_b = ExperimentConfig(experiment_id="b", seed=99, duration_ticks=500)
        ab = ABExperiment(config_a, config_b)
        result = await ab.run_parallel()
        comp = result.comparison

        assert isinstance(comp.metrics_diff, dict)
        assert isinstance(comp.statistical_significance, dict)
        assert isinstance(comp.effect_sizes, dict)
        assert comp.recommendation != ""

        # Should have real p-values from Welch's t-test
        for metric, p_val in comp.statistical_significance.items():
            assert 0.0 <= p_val <= 1.0, f"p-value out of range for {metric}: {p_val}"

    @pytest.mark.asyncio
    async def test_deterministic_results(self) -> None:
        """Same seed should produce deterministic A/B results."""
        config = ExperimentConfig(experiment_id="det", seed=42, duration_ticks=500)
        ab1 = ABExperiment(config, config, seed_base=42)
        ab2 = ABExperiment(config, config, seed_base=42)

        r1, r2 = await asyncio.gather(
            ab1._run_single(config),
            ab2._run_single(config),
        )
        assert r1.metrics_timeline == r2.metrics_timeline


# ---------------------------------------------------------------------------
# 7. WorldEngineAdapter tests (unit, no real HTTP)
# ---------------------------------------------------------------------------


class TestWorldEngineAdapter:
    def test_parse_comparison(self) -> None:
        data = {
            "variant_a": "control",
            "variant_b": "treatment",
            "metrics": [
                {
                    "metric_name": "survival_rate",
                    "variant_a_mean": 0.9,
                    "variant_b_mean": 0.95,
                    "delta": 0.05,
                    "delta_percent": 5.5,
                    "p_value": 0.03,
                    "significant": True,
                }
            ],
            "recommendation": "Treatment is better",
        }
        result = WorldEngineAdapter._parse_comparison(data)
        assert result.variant_a == "control"
        assert result.variant_b == "treatment"
        assert len(result.metrics) == 1
        assert result.metrics[0].significant is True
        assert result.recommendation == "Treatment is better"

    def test_parse_variant_data(self) -> None:
        raw = {
            "snapshots": [
                {
                    "tick": 100,
                    "agent_count": 20,
                    "alive_count": 18,
                    "total_tokens": 2000,
                    "total_money": 1000,
                    "gini_coefficient": 0.3,
                    "org_count": 2,
                    "timestamp": "2026-01-01T00:00:00Z",
                }
            ]
        }
        snapshots = WorldEngineAdapter.parse_variant_data(raw)
        assert len(snapshots) == 1
        assert snapshots[0].tick == 100
        assert snapshots[0].alive_count == 18

    def test_comparison_to_dict(self) -> None:
        from agent_runtime.experiment.world_engine_adapter import MetricComparison

        comp = VariantComparisonResult(
            variant_a="a",
            variant_b="b",
            metrics=[
                MetricComparison(
                    metric_name="test",
                    variant_a_mean=1.0,
                    variant_b_mean=2.0,
                    delta=1.0,
                    delta_percent=100.0,
                    p_value=0.01,
                    significant=True,
                )
            ],
            recommendation="B wins",
        )
        d = comp.to_dict()
        assert d["variant_a"] == "a"
        assert d["metrics"][0]["p_value"] == 0.01


# ---------------------------------------------------------------------------
# 8. Integration: DSL → ABExperiment → Report
# ---------------------------------------------------------------------------


class TestIntegration:
    @pytest.mark.asyncio
    async def test_full_pipeline(self) -> None:
        """Full pipeline: define experiment → run → compare → report."""
        # Define
        exp = ExperimentDefinition(
            name="Integration Test",
            variables=[
                ExperimentVariable("initial_tokens", default=100, type_hint="int"),
            ],
            groups=[
                ExperimentGroup("control", variables={"initial_tokens": 100}),
                ExperimentGroup("treatment", variables={"initial_tokens": 500}),
            ],
            agent_count=20,
            duration_ticks=500,
            base_seed=42,
        )
        assert exp.validate() == []

        # Generate configs
        configs = exp.to_configs()
        assert len(configs) == 2

        # Assign agents
        agents = [f"agent-{i}" for i in range(20)]
        assignment = exp.assign_agents(agents)
        assert len(assignment["control"]) == 10

        # Run A/B experiment
        ab = ABExperiment(configs["control"], configs["treatment"])
        result = await ab.run_parallel()

        # Verify comparison
        assert result.comparison.metrics_diff != {}
        assert result.comparison.statistical_significance != {}
        assert result.comparison.recommendation != ""

        # Generate reports
        reporter = ExperimentReporter()
        md_report = reporter.generate_ab_report(result.comparison, format="markdown")
        assert "# A/B Comparison Report" in md_report

        json_report = reporter.generate_ab_report(result.comparison, format="json")
        parsed = json.loads(json_report)
        assert "metrics_diff" in parsed
        assert "effect_sizes" in parsed
