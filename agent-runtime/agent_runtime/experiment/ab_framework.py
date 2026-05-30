"""A/B comparison experiment framework.

Runs two experiment configurations side-by-side (parallel or sequential)
with isolated world instances, then compares results across key metrics:
agent survival rate, organization formation, economic indicators,
social network clustering, and emergence event counts.

Supports two modes:
1. **Local mode**: Uses placeholder data with real statistical tests (default)
2. **World Engine mode**: Connects to the Rust World Engine API for live experiments
"""

from __future__ import annotations

import asyncio
import logging
from dataclasses import dataclass, field
from typing import Any

from agent_runtime.experiment.config import ExperimentConfig
from agent_runtime.experiment.report import ExperimentResult
from agent_runtime.experiment.reproducibility import ReproducibilityManager
from agent_runtime.experiment.statistics import (
    compare_metrics,
)

logger = logging.getLogger(__name__)


@dataclass
class ComparisonReport:
    """Result of comparing two experiment runs.

    Attributes:
        metrics_diff: Absolute differences for each metric (A - B).
        statistical_significance: p-values from Welch's t-test for each metric.
        effect_sizes: Cohen's d effect sizes for each metric.
        test_results: Full test result details.
        recommendation: Which config to adopt (A, B, or "inconclusive").
        summary: Human-readable summary of the comparison.
    """

    metrics_diff: dict[str, float] = field(default_factory=dict)
    statistical_significance: dict[str, float] = field(default_factory=dict)
    effect_sizes: dict[str, float] = field(default_factory=dict)
    test_results: dict[str, Any] = field(default_factory=dict)
    recommendation: str = ""
    summary: str = ""


@dataclass
class ABResult:
    """Container for both sides of an A/B experiment."""

    result_a: ExperimentResult
    result_b: ExperimentResult
    comparison: ComparisonReport


class ABExperiment:
    """A/B comparison experiment framework.

    Runs two configurations against each other with isolated world instances.
    The two configs should differ in exactly one dimension for clean
    comparison, though any difference is technically supported.

    Usage (local mode)::

        ab = ABExperiment(config_a, config_b)
        result = await ab.run_parallel()
        print(result.comparison.summary)

    Usage (World Engine mode)::

        ab = ABExperiment(config_a, config_b, world_engine_url="http://localhost:3000")
        result = await ab.run_parallel()
        print(result.comparison.summary)

    Args:
        config_a: First experiment configuration (the "control").
        config_b: Second experiment configuration (the "treatment").
        seed_base: Base seed for reproducibility. Each world gets a
            deterministic child seed derived from this base.
        world_engine_url: Optional URL for the World Engine API.
            If provided, experiments will run against the live engine.
    """

    def __init__(
        self,
        config_a: ExperimentConfig,
        config_b: ExperimentConfig,
        seed_base: int = 42,
        world_engine_url: str | None = None,
    ) -> None:
        self.config_a = config_a
        self.config_b = config_b
        self.seed_base = seed_base
        self.world_engine_url = world_engine_url
        self._manager = ReproducibilityManager(
            ExperimentConfig(seed=seed_base)
        )

    async def run_parallel(self) -> ABResult:
        """Run both experiments in parallel with isolated world instances.

        Each world gets a unique deterministic seed derived from seed_base.

        Returns:
            ABResult with both results and the comparison report.
        """
        seed_a = self._manager.derive_child_seed(0)
        seed_b = self._manager.derive_child_seed(1)

        config_a_seeded = self._reseed(self.config_a, seed_a)
        config_b_seeded = self._reseed(self.config_b, seed_b)

        if self.world_engine_url:
            result = await self._run_via_world_engine(
                config_a_seeded, config_b_seeded
            )
            return result

        # Run both experiments concurrently (local mode)
        result_a, result_b = await asyncio.gather(
            self._run_single(config_a_seeded),
            self._run_single(config_b_seeded),
        )

        comparison = self.compare_results(result_a, result_b)
        return ABResult(result_a=result_a, result_b=result_b, comparison=comparison)

    async def run_sequential(self) -> ABResult:
        """Run both experiments sequentially.

        Useful when parallel execution would exceed resource limits.

        Returns:
            ABResult with both results and the comparison report.
        """
        seed_a = self._manager.derive_child_seed(0)
        seed_b = self._manager.derive_child_seed(1)

        config_a_seeded = self._reseed(self.config_a, seed_a)
        config_b_seeded = self._reseed(self.config_b, seed_b)

        if self.world_engine_url:
            return await self._run_via_world_engine(
                config_a_seeded, config_b_seeded
            )

        result_a = await self._run_single(config_a_seeded)
        result_b = await self._run_single(config_b_seeded)

        comparison = self.compare_results(result_a, result_b)
        return ABResult(result_a=result_a, result_b=result_b, comparison=comparison)

    def compare_results(
        self,
        result_a: ExperimentResult,
        result_b: ExperimentResult,
        alpha: float = 0.05,
    ) -> ComparisonReport:
        """Compare two experiment results with real statistical tests.

        Performs Welch's t-test on each metric, computes effect sizes,
        and generates a recommendation based on significance and effect.

        Compared dimensions:
        - Agent survival rate
        - Organization count
        - Economic indicators (trading volume, Gini coefficient)
        - Social network clustering coefficient
        - Emergence event count

        Args:
            result_a: Result from the control (A) experiment.
            result_b: Result from the treatment (B) experiment.
            alpha: Significance level for statistical tests.

        Returns:
            ComparisonReport with differences, p-values, effect sizes, and recommendation.
        """
        metrics_a_timeline = self._extract_timeline_metrics(result_a)
        metrics_b_timeline = self._extract_timeline_metrics(result_b)

        # Compute metric differences (means)
        mean_a = self._compute_means(metrics_a_timeline)
        mean_b = self._compute_means(metrics_b_timeline)

        metrics_diff: dict[str, float] = {}
        for key in mean_a:
            if key in mean_b:
                metrics_diff[key] = mean_a[key] - mean_b[key]

        # Run statistical tests on timeline data
        test_results_dict = compare_metrics(
            metrics_a_timeline, metrics_b_timeline, alpha=alpha
        )

        # Extract p-values and effect sizes
        significance: dict[str, float] = {}
        effect_sizes: dict[str, float] = {}
        significant_metrics: list[str] = []
        for metric_name, result in test_results_dict.items():
            test_data = result["test"]
            significance[metric_name] = test_data["p_value"]
            effect_sizes[metric_name] = result["effect_size"]
            if result["test"]["significant"]:
                significant_metrics.append(metric_name)

        # Generate recommendation based on significant metrics
        recommendation = self._generate_recommendation(
            result_a, result_b, metrics_diff, significance, effect_sizes
        )

        # Build summary
        summary = self._build_summary(
            result_a, result_b, metrics_diff, test_results_dict, recommendation
        )

        return ComparisonReport(
            metrics_diff=metrics_diff,
            statistical_significance=significance,
            effect_sizes=effect_sizes,
            test_results={k: v["test"] for k, v in test_results_dict.items()},
            recommendation=recommendation,
            summary=summary,
        )

    # -------------------------------------------------------------------
    # Internal helpers
    # -------------------------------------------------------------------

    @staticmethod
    def _reseed(config: ExperimentConfig, new_seed: int) -> ExperimentConfig:
        """Create a copy of config with a different seed."""
        return ExperimentConfig(
            experiment_id=config.experiment_id,
            name=config.name,
            seed=new_seed,
            duration_ticks=config.duration_ticks,
            world=config.world,
            agents=config.agents,
            governance=config.governance,
            llm=config.llm,
            tracing=config.tracing,
        )

    async def _run_single(self, config: ExperimentConfig) -> ExperimentResult:
        """Run a single experiment with the given config.

        In local mode, generates deterministic placeholder data using seeded RNG.
        In World Engine mode, this method is not called directly.
        """
        from datetime import datetime, timezone

        repro_mgr = ReproducibilityManager(config)
        snapshot = repro_mgr.snapshot_config()

        started = datetime.now(timezone.utc).isoformat()

        logger.info(
            "Starting experiment %s (seed=%d, ticks=%d)",
            config.experiment_id,
            config.seed,
            config.duration_ticks,
        )

        # Simulate experiment execution
        await asyncio.sleep(0.01)  # Yield to event loop

        # Collect deterministic placeholder metrics
        metrics_timeline: list[dict[str, Any]] = []
        for tick in range(0, config.duration_ticks + 1, config.tracing.snapshot_interval):
            metrics_timeline.append({
                "tick": tick,
                "survival_rate": round(repro_mgr.random.uniform(0.7, 1.0), 4),
                "organization_count": repro_mgr.random.randint(1, 10),
                "trade_volume": round(repro_mgr.random.uniform(100, 1000), 2),
                "gini_coefficient": round(repro_mgr.random.uniform(0.2, 0.6), 4),
                "clustering_coefficient": round(repro_mgr.random.uniform(0.1, 0.5), 4),
            })

        emergence_events: list[dict[str, Any]] = []
        if repro_mgr.random.random() > 0.3:
            emergence_events.append({
                "tick": repro_mgr.random.randint(100, config.duration_ticks),
                "type": "organization_formation",
                "description": "Agents自发形成合作组织",
            })

        finished = datetime.now(timezone.utc).isoformat()

        return ExperimentResult(
            experiment_id=config.experiment_id,
            config_snapshot=snapshot.to_dict(),
            duration_ticks=config.duration_ticks,
            completed_ticks=config.duration_ticks,
            agent_count=config.agents.count,
            final_snapshot=metrics_timeline[-1] if metrics_timeline else {},
            metrics_timeline=metrics_timeline,
            emergence_events=emergence_events,
            errors=[],
            started_at=started,
            finished_at=finished,
        )

    async def _run_via_world_engine(
        self,
        config_a: ExperimentConfig,
        config_b: ExperimentConfig,
    ) -> ABResult:
        """Run both experiments via the World Engine API."""
        from agent_runtime.experiment.world_engine_adapter import WorldEngineAdapter

        async with WorldEngineAdapter(self.world_engine_url) as adapter:
            # Create experiment with two variants
            exp_id = await adapter.create_experiment(
                name=f"{config_a.name} vs {config_b.name}",
                variants=[
                    {
                        "name": "control",
                        "parameters": self._config_to_variant_params(config_a),
                    },
                    {
                        "name": "treatment",
                        "parameters": self._config_to_variant_params(config_b),
                    },
                ],
            )

            await adapter.start_experiment(exp_id)

            # Capture initial snapshots
            await adapter.capture_snapshot(exp_id, "control")
            await adapter.capture_snapshot(exp_id, "treatment")

            # Stop experiment
            await adapter.stop_experiment(exp_id)

            # Get full experiment data
            exp_data = await adapter.get_experiment(exp_id)

            # Build results from World Engine data
            result_a = self._build_result_from_we(
                config_a, exp_data, "control"
            )
            result_b = self._build_result_from_we(
                config_b, exp_data, "treatment"
            )

            # Compare using World Engine's built-in t-test
            try:
                we_comparison = await adapter.compare_variants(
                    exp_id, "control", "treatment"
                )
                comparison = self._comparison_from_we(we_comparison, result_a, result_b)
            except Exception:
                # Fall back to Python-side comparison
                comparison = self.compare_results(result_a, result_b)

            return ABResult(
                result_a=result_a,
                result_b=result_b,
                comparison=comparison,
            )

    @staticmethod
    def _config_to_variant_params(config: ExperimentConfig) -> dict[str, str]:
        """Convert ExperimentConfig to variant parameters dict."""
        params: dict[str, str] = {}
        params["initial_tokens"] = str(config.agents.initial_tokens)
        params["agent_count"] = str(config.agents.count)
        params["tax_rate"] = str(config.governance.tax_rate)
        params["governance_enabled"] = str(config.governance.enabled).lower()
        params["resource_density"] = str(config.world.resource_density)
        params["temperature"] = str(config.llm.temperature)
        if config.llm.model:
            params["model"] = config.llm.model
        return params

    @staticmethod
    def _build_result_from_we(
        config: ExperimentConfig,
        exp_data: dict[str, Any],
        variant_name: str,
    ) -> ExperimentResult:
        """Build an ExperimentResult from World Engine response data."""

        variants = exp_data.get("variants", [])
        variant = next(
            (v for v in variants if v.get("config", {}).get("name") == variant_name),
            None,
        )

        if not variant:
            return ExperimentResult(
                experiment_id=config.experiment_id,
                config_snapshot={},
                duration_ticks=config.duration_ticks,
                completed_ticks=0,
                agent_count=config.agents.count,
                errors=[f"Variant '{variant_name}' not found in World Engine data"],
            )

        snapshots = variant.get("snapshots", [])
        metrics_timeline = [
            {
                "tick": s.get("tick", 0),
                "agent_count": s.get("agent_count", 0),
                "alive_count": s.get("alive_count", 0),
                "survival_rate": (
                    s.get("alive_count", 0) / max(1, s.get("agent_count", 1))
                ),
                "total_tokens": s.get("total_tokens", 0),
                "total_money": s.get("total_money", 0),
                "gini_coefficient": s.get("gini_coefficient"),
                "org_count": s.get("org_count", 0),
            }
            for s in snapshots
        ]

        return ExperimentResult(
            experiment_id=config.experiment_id,
            config_snapshot=config.to_dict(),
            duration_ticks=config.duration_ticks,
            completed_ticks=(
                snapshots[-1].get("tick", 0) if snapshots else 0
            ),
            agent_count=config.agents.count,
            final_snapshot=metrics_timeline[-1] if metrics_timeline else {},
            metrics_timeline=metrics_timeline,
            emergence_events=[],
            errors=[],
            started_at=exp_data.get("started_at", ""),
            finished_at=exp_data.get("stopped_at", ""),
        )

    def _comparison_from_we(
        self,
        we_comparison: Any,
        result_a: ExperimentResult,
        result_b: ExperimentResult,
    ) -> ComparisonReport:
        """Convert World Engine comparison to ComparisonReport."""
        from agent_runtime.experiment.world_engine_adapter import VariantComparisonResult

        if isinstance(we_comparison, VariantComparisonResult):
            metrics_diff = {}
            significance = {}
            effect_sizes = {}

            for m in we_comparison.metrics:
                metrics_diff[m.metric_name] = m.delta
                significance[m.metric_name] = m.p_value or 1.0
                # Compute effect size from available data
                if m.variant_a_mean != 0:
                    effect_sizes[m.metric_name] = abs(m.delta / abs(m.variant_a_mean))
                else:
                    effect_sizes[m.metric_name] = 0.0

            summary = f"Comparing {we_comparison.variant_a} vs {we_comparison.variant_a}:\n"
            for m in we_comparison.metrics:
                sig = "✅" if m.significant else ""
                summary += f"  - {m.metric_name}: delta={m.delta:+.4f} p={m.p_value:.4f} {sig}\n"
            if we_comparison.recommendation:
                summary += f"\nRecommendation: {we_comparison.recommendation}"

            return ComparisonReport(
                metrics_diff=metrics_diff,
                statistical_significance=significance,
                effect_sizes=effect_sizes,
                recommendation=we_comparison.recommendation or "",
                summary=summary,
            )

        return self.compare_results(result_a, result_b)

    @staticmethod
    def _extract_timeline_metrics(
        result: ExperimentResult,
    ) -> dict[str, list[float]]:
        """Extract per-metric value lists from the timeline for statistical testing."""
        timeline = result.metrics_timeline
        if not timeline:
            return {}

        metrics: dict[str, list[float]] = {}
        float_keys = [
            "survival_rate",
            "organization_count",
            "trade_volume",
            "gini_coefficient",
            "clustering_coefficient",
        ]

        for key in float_keys:
            values = []
            for entry in timeline:
                if key in entry and isinstance(entry[key], (int, float)):
                    values.append(float(entry[key]))
            if values:
                metrics[key] = values

        # Emergence events as count
        metrics["emergence_event_count"] = [float(len(result.emergence_events))]

        return metrics

    @staticmethod
    def _compute_means(
        metrics: dict[str, list[float]],
    ) -> dict[str, float]:
        """Compute mean for each metric."""
        return {
            key: sum(values) / len(values) if values else 0.0
            for key, values in metrics.items()
        }

    @staticmethod
    def _generate_recommendation(
        result_a: ExperimentResult,
        result_b: ExperimentResult,
        metrics_diff: dict[str, float],
        significance: dict[str, float],
        effect_sizes: dict[str, float],
    ) -> str:
        """Generate a recommendation based on test results."""
        significant_a = sum(
            1
            for k, p in significance.items()
            if p < 0.05 and metrics_diff.get(k, 0) > 0
        )
        significant_b = sum(
            1
            for k, p in significance.items()
            if p < 0.05 and metrics_diff.get(k, 0) < 0
        )

        if significant_a == 0 and significant_b == 0:
            return (
                "Inconclusive — no statistically significant differences "
                "detected (p >= 0.05). Consider collecting more data."
            )

        if significant_a > significant_b:
            return (
                f"Config A ({result_a.experiment_id}) — significantly better in "
                f"{significant_a} metric(s) (p < 0.05)"
            )
        elif significant_b > significant_a:
            return (
                f"Config B ({result_b.experiment_id}) — significantly better in "
                f"{significant_b} metric(s) (p < 0.05)"
            )
        else:
            return (
                "Mixed results — both configs show significant advantages in "
                "different metrics. Decision depends on priority."
            )

    @staticmethod
    def _build_summary(
        result_a: ExperimentResult,
        result_b: ExperimentResult,
        metrics_diff: dict[str, float],
        test_results: dict[str, Any],
        recommendation: str,
    ) -> str:
        """Build human-readable comparison summary."""
        parts: list[str] = []
        parts.append(
            f"Comparing {result_a.experiment_id} (A) vs {result_b.experiment_id} (B):"
        )
        parts.append("")

        for key, diff in metrics_diff.items():
            direction = "higher" if diff > 0 else "lower"
            test = test_results.get(key, {})
            p_val = test.get("test", {}).get("p_value", "N/A")
            effect = test.get("effect_size", "N/A")
            sig_marker = "✅" if test.get("test", {}).get("significant") else ""

            parts.append(
                f"  - {key}: A is {abs(diff):.4f} {direction} than B "
                f"(p={p_val}, d={effect}) {sig_marker}"
            )

        parts.append("")
        parts.append(f"Recommendation: {recommendation}")

        return "\n".join(parts)
