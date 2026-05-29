"""Experiment framework for Agent World — configuration-driven experiments.

Provides:
- ExperimentConfig: YAML/TOML config parsing with validation
- ABExperiment: A/B comparison framework (parallel/sequential + World Engine integration)
- ReproducibilityManager: Seeded RNG + config snapshots
- ExperimentReporter: Auto-generated reports (Markdown/JSON/HTML/PDF)
- ExperimentResult: Standardized result dataclass
- ExperimentDefinition: DSL for defining experiment groups, variables, hypotheses
- WorldEngineAdapter: Client for the Rust World Engine A/B API
- Statistical tests: Welch's t-test, chi-square, Mann-Whitney U, Cohen's d
- REST API: FastAPI endpoints for managing experiments
"""

from agent_runtime.experiment.ab_framework import ABExperiment, ABResult, ComparisonReport
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

__all__ = [
    # Core framework
    "ABExperiment",
    "ABResult",
    "ComparisonReport",
    "ExperimentConfig",
    "ExperimentReporter",
    "ExperimentResult",
    "ReproducibilityManager",
    # DSL
    "ExperimentDefinition",
    "ExperimentGroup",
    "ExperimentVariable",
    "Hypothesis",
    # Statistics
    "TestResult",
    "chi_square_goodness_of_fit",
    "chi_square_test",
    "cohens_d",
    "compare_metrics",
    "mann_whitney_u_test",
    "welch_t_test",
]
