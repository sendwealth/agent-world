"""Tests for the experiment framework (Phase 4.5.2).

Covers:
- ExperimentConfig parsing from YAML/TOML and dict
- Config validation
- Config serialization (to_dict)
- ReproducibilityManager: seeded RNG, config snapshot, child seeds
- ExperimentReporter: markdown/json/html output
- ABExperiment: parallel/sequential runs and comparison
"""

from __future__ import annotations

import asyncio
import json
from pathlib import Path

import pytest

from agent_runtime.experiment.ab_framework import ABExperiment, ComparisonReport
from agent_runtime.experiment.config import ExperimentConfig
from agent_runtime.experiment.report import ExperimentReporter, ExperimentResult
from agent_runtime.experiment.reproducibility import ReproducibilityManager

# ---------------------------------------------------------------------------
# Helpers
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
            {"tick": 0, "survival_rate": 1.0},
            {"tick": 500, "survival_rate": 0.98},
            {"tick": 1000, "survival_rate": 0.95},
        ],
        emergence_events=[
            {"tick": 300, "type": "org_formation", "description": "Test event"},
        ],
        errors=[],
        started_at="2026-01-01T00:00:00Z",
        finished_at="2026-01-01T01:00:00Z",
    )


# ---------------------------------------------------------------------------
# 1. Config parsing tests
# ---------------------------------------------------------------------------


class TestExperimentConfig:
    def test_from_yaml(self, yaml_file: Path) -> None:
        config = ExperimentConfig.from_yaml(yaml_file)
        assert config.experiment_id == "test-exp-001"
        assert config.name == "Test Experiment"
        assert config.seed == 42
        assert config.duration_ticks == 1000
        assert config.world.width == 50
        assert config.world.height == 50
        assert config.world.resource_density == 0.4
        assert config.agents.count == 20
        assert config.agents.personality_distribution == "clustered"
        assert config.agents.initial_tokens == 200
        assert config.governance.enabled is False
        assert config.governance.tax_rate == 0.0
        assert config.tracing.snapshot_interval == 50

    def test_from_yaml_not_found(self) -> None:
        with pytest.raises(FileNotFoundError):
            ExperimentConfig.from_yaml("/nonexistent/config.yaml")

    def test_from_dict(self) -> None:
        data = {
            "experiment": {
                "id": "dict-test",
                "seed": 99,
                "duration_ticks": 500,
            }
        }
        config = ExperimentConfig.from_dict(data)
        assert config.experiment_id == "dict-test"
        assert config.seed == 99
        # Defaults should apply
        assert config.world.width == 100
        assert config.agents.count == 50

    def test_from_dict_defaults_only(self) -> None:
        config = ExperimentConfig.from_dict({})
        assert config.experiment_id == "unnamed"
        assert config.seed == 42
        assert config.duration_ticks == 10000

    def test_validate_valid_config(self, sample_config: ExperimentConfig) -> None:
        _errors = sample_config.validate()
        # "unnamed" id is valid as long as it's not empty (but "unnamed" triggers a warning)
        # Actually it does trigger - let's use a proper config
        good = ExperimentConfig(experiment_id="good-exp", duration_ticks=100)
        assert good.validate() == []

    def test_validate_negative_ticks(self) -> None:
        config = ExperimentConfig(experiment_id="bad", duration_ticks=-1)
        errors = config.validate()
        assert any("duration_ticks" in e for e in errors)

    def test_validate_bad_resource_density(self) -> None:
        config = ExperimentConfig(
            experiment_id="bad",
            world={"resource_density": 1.5} if False else None,  # type: ignore
        )
        # Let's construct properly
        from agent_runtime.experiment.config import WorldExperimentConfig
        bad_world = WorldExperimentConfig(resource_density=1.5)
        config = ExperimentConfig(experiment_id="bad", world=bad_world)
        errors = config.validate()
        assert any("resource_density" in e for e in errors)

    def test_validate_bad_personality_distribution(self) -> None:
        from agent_runtime.experiment.config import AgentsExperimentConfig
        bad_agents = AgentsExperimentConfig(personality_distribution="invalid")
        config = ExperimentConfig(experiment_id="bad", agents=bad_agents)
        errors = config.validate()
        assert any("personality_distribution" in e for e in errors)

    def test_to_dict_roundtrip(self) -> None:
        original = ExperimentConfig(
            experiment_id="roundtrip",
            name="Round Trip",
            seed=123,
            duration_ticks=5000,
        )
        d = original.to_dict()
        assert d["experiment_id"] == "roundtrip"
        assert d["seed"] == 123
        assert isinstance(d["world"], dict)
        assert isinstance(d["agents"], dict)

        # Reconstruct and verify key fields
        restored = ExperimentConfig.from_dict(d)
        assert restored.experiment_id == original.experiment_id
        assert restored.seed == original.seed
        assert restored.duration_ticks == original.duration_ticks

    def test_from_toml(self, tmp_path: Path) -> None:
        toml_content = """
[experiment]
id = "toml-test"
name = "TOML Test"
seed = 77
duration_ticks = 2000

[experiment.world]
width = 60
height = 60

[experiment.agents]
count = 15
"""
        p = tmp_path / "test.toml"
        p.write_text(toml_content)
        config = ExperimentConfig.from_toml(p)
        assert config.experiment_id == "toml-test"
        assert config.seed == 77
        assert config.world.width == 60
        assert config.agents.count == 15


# ---------------------------------------------------------------------------
# 2. Reproducibility tests
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
        config_a = ExperimentConfig(seed=42)
        config_b = ExperimentConfig(seed=99)
        mgr_a = ReproducibilityManager(config_a)
        mgr_b = ReproducibilityManager(config_b)
        vals_a = [mgr_a.random.random() for _ in range(10)]
        vals_b = [mgr_b.random.random() for _ in range(10)]
        assert vals_a != vals_b

    def test_config_snapshot_hash_stable(self) -> None:
        config = ExperimentConfig(experiment_id="snap-test", seed=42)
        mgr = ReproducibilityManager(config)
        snap1 = mgr.snapshot_config()
        snap2 = mgr.snapshot_config()
        assert snap1.content_hash == snap2.content_hash

    def test_config_snapshot_captures_all_fields(self) -> None:
        config = ExperimentConfig(
            experiment_id="full-snap",
            seed=42,
            duration_ticks=500,
        )
        mgr = ReproducibilityManager(config)
        snap = mgr.snapshot_config()
        d = snap.to_dict()
        assert d["seed"] == 42
        assert "config" in d
        assert "content_hash" in d
        assert d["config"]["experiment_id"] == "full-snap"

    def test_verify_reproducibility_matching(self) -> None:
        config = ExperimentConfig(seed=42)
        mgr = ReproducibilityManager(config)
        snapshot = mgr.snapshot_config()
        data = snapshot.to_dict()
        timeline = [{"tick": 0, "val": 1.0}]
        run_a = {
            "config_snapshot": data,
            "metrics_timeline": timeline,
            "final_snapshot": {"tick": 0},
        }
        run_b = {
            "config_snapshot": data,
            "metrics_timeline": timeline,
            "final_snapshot": {"tick": 0},
        }
        assert mgr.verify_reproducibility(run_a, run_b) is True

    def test_verify_reproducibility_mismatch(self) -> None:
        config = ExperimentConfig(seed=42)
        mgr = ReproducibilityManager(config)
        snapshot = mgr.snapshot_config()
        data = snapshot.to_dict()
        run_a = {
            "config_snapshot": data,
            "metrics_timeline": [{"tick": 0, "val": 1.0}],
            "final_snapshot": {"tick": 0},
        }
        run_b = {
            "config_snapshot": data,
            "metrics_timeline": [{"tick": 0, "val": 2.0}],
            "final_snapshot": {"tick": 0},
        }
        assert mgr.verify_reproducibility(run_a, run_b) is False

    def test_derive_child_seed_deterministic(self) -> None:
        config = ExperimentConfig(seed=42)
        mgr = ReproducibilityManager(config)
        child_a = mgr.derive_child_seed(0)
        child_b = mgr.derive_child_seed(0)
        assert child_a == child_b

    def test_derive_child_seed_unique(self) -> None:
        config = ExperimentConfig(seed=42)
        mgr = ReproducibilityManager(config)
        child_0 = mgr.derive_child_seed(0)
        child_1 = mgr.derive_child_seed(1)
        assert child_0 != child_1

    def test_reset_rng(self) -> None:
        config = ExperimentConfig(seed=42)
        mgr = ReproducibilityManager(config)
        vals_before = [mgr.random.random() for _ in range(5)]
        mgr.reset_rng()
        vals_after = [mgr.random.random() for _ in range(5)]
        assert vals_before == vals_after


# ---------------------------------------------------------------------------
# 3. Reporter tests
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
        assert parsed["completed_ticks"] == 1000
        assert len(parsed["metrics_timeline"]) == 3

    def test_html_report(self, sample_result: ExperimentResult) -> None:
        reporter = ExperimentReporter()
        html = reporter.generate_report(sample_result, format="html")
        assert "<!DOCTYPE html>" in html
        assert "test-001" in html

    def test_result_to_dict_roundtrip(self, sample_result: ExperimentResult) -> None:
        d = sample_result.to_dict()
        restored = ExperimentResult.from_dict(d)
        assert restored.experiment_id == sample_result.experiment_id
        assert restored.completed_ticks == sample_result.completed_ticks
        assert len(restored.metrics_timeline) == len(sample_result.metrics_timeline)

    def test_ab_report_markdown(self) -> None:
        comparison = ComparisonReport(
            metrics_diff={"survival_rate": 0.05, "trade_volume": -100.0},
            statistical_significance={"survival_rate": 0.03},
            recommendation="Config A",
            summary="A is better than B.",
        )
        reporter = ExperimentReporter()
        md = reporter.generate_ab_report(comparison, format="markdown")
        assert "# A/B Comparison Report" in md
        assert "survival_rate" in md
        assert "Config A" in md

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


# ---------------------------------------------------------------------------
# 4. A/B Framework tests
# ---------------------------------------------------------------------------


class TestABFramework:
    @pytest.mark.asyncio
    async def test_parallel_run(self) -> None:
        config_a = ExperimentConfig(
            experiment_id="exp-a", seed=42, duration_ticks=500,
        )
        config_b = ExperimentConfig(
            experiment_id="exp-b", seed=42, duration_ticks=500,
        )
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
    async def test_comparison_metrics(self) -> None:
        config_a = ExperimentConfig(experiment_id="a", seed=42, duration_ticks=500)
        config_b = ExperimentConfig(experiment_id="b", seed=99, duration_ticks=500)
        ab = ABExperiment(config_a, config_b)
        result = await ab.run_parallel()
        comp = result.comparison
        assert isinstance(comp.metrics_diff, dict)
        assert isinstance(comp.statistical_significance, dict)
        assert comp.recommendation != ""

    @pytest.mark.asyncio
    async def test_compare_results_deterministic(self) -> None:
        """Same seed should produce deterministic A/B results."""
        config = ExperimentConfig(experiment_id="det", seed=42, duration_ticks=500)
        ab1 = ABExperiment(config, config, seed_base=42)
        ab2 = ABExperiment(config, config, seed_base=42)

        # Compare results manually (since _run_single is async)
        r1, r2 = await asyncio.gather(
            ab1._run_single(config),
            ab2._run_single(config),
        )
        # Results should be identical (same seed, same config)
        assert r1.metrics_timeline == r2.metrics_timeline
