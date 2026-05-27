"""Experiment framework for Agent World — configuration-driven experiments.

Provides:
- ExperimentConfig: YAML/TOML config parsing with validation
- ABExperiment: A/B comparison framework (parallel/sequential)
- ReproducibilityManager: Seeded RNG + config snapshots
- ExperimentReporter: Auto-generated reports (Markdown/JSON/HTML)
- ExperimentResult: Standardized result dataclass
"""

from agent_runtime.experiment.config import ExperimentConfig
from agent_runtime.experiment.reproducibility import ReproducibilityManager
from agent_runtime.experiment.report import ExperimentReporter, ExperimentResult
from agent_runtime.experiment.ab_framework import ABExperiment, ComparisonReport

__all__ = [
    "ABExperiment",
    "ComparisonReport",
    "ExperimentConfig",
    "ExperimentReporter",
    "ExperimentResult",
    "ReproducibilityManager",
]
