"""A/B comparison experiment framework.

Runs two experiment configurations side-by-side (parallel or sequential)
with isolated world instances, then compares results across key metrics:
agent survival rate, organization formation, economic indicators,
social network clustering, and emergence event counts.
"""

from __future__ import annotations

import asyncio
import logging
import math
from dataclasses import dataclass, field
from typing import Any

from agent_runtime.experiment.config import ExperimentConfig
from agent_runtime.experiment.report import ExperimentResult
from agent_runtime.experiment.reproducibility import ReproducibilityManager

logger = logging.getLogger(__name__)


@dataclass
class ComparisonReport:
    """Result of comparing two experiment runs.

    Attributes:
        metrics_diff: Absolute differences for each metric (A - B).
        statistical_significance: Approximate p-values for each metric.
        recommendation: Which config to adopt (A, B, or "inconclusive").
        summary: Human-readable summary of the comparison.
    """

    metrics_diff: dict[str, float] = field(default_factory=dict)
    statistical_significance: dict[str, float] = field(default_factory=dict)
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

    Usage::

        ab = ABExperiment(config_a, config_b)
        result = await ab.run_parallel()
        print(result.comparison.summary)

    Args:
        config_a: First experiment configuration (the "control").
        config_b: Second experiment configuration (the "treatment").
        seed_base: Base seed for reproducibility. Each world gets a
            deterministic child seed derived from this base.
    """

    def __init__(
        self,
        config_a: ExperimentConfig,
        config_b: ExperimentConfig,
        seed_base: int = 42,
    ) -> None:
        self.config_a = config_a
        self.config_b = config_b
        self.seed_base = seed_base
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

        # Run both experiments concurrently
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

        result_a = await self._run_single(config_a_seeded)
        result_b = await self._run_single(config_b_seeded)

        comparison = self.compare_results(result_a, result_b)
        return ABResult(result_a=result_a, result_b=result_b, comparison=comparison)

    def compare_results(
        self,
        result_a: ExperimentResult,
        result_b: ExperimentResult,
    ) -> ComparisonReport:
        """Compare two experiment results and produce a comparison report.

        Compares across multiple dimensions:
        - Agent survival rate
        - Organization count
        - Economic indicators (trading volume, Gini coefficient)
        - Social network clustering coefficient
        - Emergence event count

        Args:
            result_a: Result from the control (A) experiment.
            result_b: Result from the treatment (B) experiment.

        Returns:
            ComparisonReport with differences and recommendations.
        """
        metrics_a = self._extract_metrics(result_a)
        metrics_b = self._extract_metrics(result_b)

        metrics_diff: dict[str, float] = {}
        for key in metrics_a:
            if key in metrics_b:
                metrics_diff[key] = metrics_a[key] - metrics_b[key]

        # Approximate statistical significance
        # (In a real implementation, this would use proper statistical tests
        # across multiple runs. Here we provide a simplified heuristic.)
        significance: dict[str, float] = {}
        for key, diff in metrics_diff.items():
            magnitude = abs(diff)
            # Heuristic: p-value decreases with magnitude
            pval = max(0.0, min(1.0, math.exp(-magnitude * 5)))
            significance[key] = round(pval, 4)

        # Generate recommendation
        significant_a = sum(
            1 for k, p in significance.items() if p < 0.05 and metrics_diff[k] > 0
        )
        significant_b = sum(
            1 for k, p in significance.items() if p < 0.05 and metrics_diff[k] < 0
        )

        if significant_a > significant_b:
            recommendation = f"Config A ({result_a.experiment_id})"
        elif significant_b > significant_a:
            recommendation = f"Config B ({result_b.experiment_id})"
        else:
            recommendation = "Inconclusive — no significant difference detected"

        # Build summary
        summary_parts: list[str] = []
        summary_parts.append(
            f"Comparing {result_a.experiment_id} (A) vs {result_b.experiment_id} (B):"
        )
        for key, diff in metrics_diff.items():
            direction = "higher" if diff > 0 else "lower"
            summary_parts.append(
                f"  - {key}: A is {abs(diff):.4f} {direction} than B"
            )
        summary_parts.append(f"Recommendation: {recommendation}")
        summary = "\n".join(summary_parts)

        return ComparisonReport(
            metrics_diff=metrics_diff,
            statistical_significance=significance,
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

        In a full implementation, this would:
        1. Create a WorldEngine instance with the config
        2. Populate agents using the seeded RNG
        3. Run the simulation for duration_ticks
        4. Collect metrics at snapshot_interval
        5. Return the result

        For now, this provides a scaffold that records the config
        and returns a placeholder result.
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
        # In production, this would drive the actual WorldEngine
        await asyncio.sleep(0.01)  # Yield to event loop

        # Collect placeholder metrics
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

    @staticmethod
    def _extract_metrics(result: ExperimentResult) -> dict[str, float]:
        """Extract aggregate metrics from an experiment result."""
        timeline = result.metrics_timeline
        if not timeline:
            return {}

        # Average across all timeline entries
        keys_to_avg = [
            "survival_rate",
            "organization_count",
            "trade_volume",
            "gini_coefficient",
            "clustering_coefficient",
        ]

        metrics: dict[str, float] = {}
        for key in keys_to_avg:
            values = [entry[key] for entry in timeline if key in entry]
            if values:
                metrics[key] = sum(values) / len(values)

        # Add emergence event count
        metrics["emergence_event_count"] = float(len(result.emergence_events))

        return metrics
