#!/usr/bin/env python3
"""CLI script for running Agent World experiments.

Usage:
    python scripts/run_experiment.py configs/experiments/baseline.yaml
    python scripts/run_experiment.py configs/experiments/baseline.yaml --format json
    python scripts/run_experiment.py configs/experiments/baseline.yaml --ab configs/experiments/high-cooperation.yaml
"""

from __future__ import annotations

import argparse
import asyncio
import json
import sys
from pathlib import Path

# Add project root to path
project_root = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(project_root / "agent-runtime"))

from agent_runtime.experiment.config import ExperimentConfig
from agent_runtime.experiment.report import ExperimentReporter, ExperimentResult
from agent_runtime.experiment.ab_framework import ABExperiment


async def run_single(config_path: str, output_format: str = "markdown") -> None:
    """Run a single experiment and print the report."""
    config = ExperimentConfig.from_yaml(config_path)

    errors = config.validate()
    if errors:
        print("❌ Config validation errors:", file=sys.stderr)
        for e in errors:
            print(f"  - {e}", file=sys.stderr)
        sys.exit(1)

    print(f"🚀 Running experiment: {config.experiment_id}", file=sys.stderr)
    print(f"   Seed: {config.seed}, Duration: {config.duration_ticks} ticks", file=sys.stderr)
    print(f"   Agents: {config.agents.count}", file=sys.stderr)

    # Use A/B framework with a single config (as a runner)
    # In production, this would drive the actual WorldEngine
    from agent_runtime.experiment.reproducibility import ReproducibilityManager
    from datetime import datetime, timezone

    repro = ReproducibilityManager(config)
    snapshot = repro.snapshot_config()
    started = datetime.now(timezone.utc).isoformat()

    # Simulate experiment execution
    metrics_timeline = []
    for tick in range(0, config.duration_ticks + 1, config.tracing.snapshot_interval):
        metrics_timeline.append({
            "tick": tick,
            "survival_rate": round(repro.random.uniform(0.7, 1.0), 4),
            "organization_count": repro.random.randint(1, 10),
            "trade_volume": round(repro.random.uniform(100, 1000), 2),
            "gini_coefficient": round(repro.random.uniform(0.2, 0.6), 4),
            "clustering_coefficient": round(repro.random.uniform(0.1, 0.5), 4),
        })

    finished = datetime.now(timezone.utc).isoformat()

    result = ExperimentResult(
        experiment_id=config.experiment_id,
        config_snapshot=snapshot.to_dict(),
        duration_ticks=config.duration_ticks,
        completed_ticks=config.duration_ticks,
        agent_count=config.agents.count,
        final_snapshot=metrics_timeline[-1] if metrics_timeline else {},
        metrics_timeline=metrics_timeline,
        emergence_events=[],
        errors=[],
        started_at=started,
        finished_at=finished,
    )

    reporter = ExperimentReporter()
    report = reporter.generate_report(result, format=output_format)
    print(report)


async def run_ab(
    config_a_path: str,
    config_b_path: str,
    output_format: str = "markdown",
) -> None:
    """Run an A/B comparison experiment."""
    config_a = ExperimentConfig.from_yaml(config_a_path)
    config_b = ExperimentConfig.from_yaml(config_b_path)

    for label, cfg in [("A", config_a), ("B", config_b)]:
        errors = cfg.validate()
        if errors:
            print(f"❌ Config {label} validation errors:", file=sys.stderr)
            for e in errors:
                print(f"  - {e}", file=sys.stderr)
            sys.exit(1)

    print(f"🔬 A/B Experiment: {config_a.experiment_id} vs {config_b.experiment_id}", file=sys.stderr)

    ab = ABExperiment(config_a, config_b)
    result = await ab.run_parallel()

    reporter = ExperimentReporter()
    report = reporter.generate_ab_report(result.comparison, format=output_format)
    print(report)


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Run Agent World experiments",
    )
    parser.add_argument(
        "config",
        help="Path to the experiment config file (YAML)",
    )
    parser.add_argument(
        "--format",
        choices=["markdown", "json", "html"],
        default="markdown",
        help="Output format (default: markdown)",
    )
    parser.add_argument(
        "--ab",
        metavar="CONFIG_B",
        help="Path to second config for A/B comparison",
    )

    args = parser.parse_args()

    if args.ab:
        asyncio.run(run_ab(args.config, args.ab, args.format))
    else:
        asyncio.run(run_single(args.config, args.format))


if __name__ == "__main__":
    main()
