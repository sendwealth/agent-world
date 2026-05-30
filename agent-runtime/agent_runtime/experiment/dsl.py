"""Experiment configuration DSL — define experiment groups, variables, and hypotheses.

Provides a declarative DSL for defining A/B experiments:
- **ExperimentDefinition**: Top-level experiment with groups, variables, and hypotheses
- **ExperimentGroup**: Named group (control/treatment) with variable overrides
- **ExperimentVariable**: A single variable being tested (with name, default, and optional range)
- **Hypothesis**: Formal hypothesis statement with direction (two-sided, greater, less)

Usage::

    from agent_runtime.experiment.dsl import (
        ExperimentDefinition, ExperimentGroup, ExperimentVariable, Hypothesis,
    )

    exp = ExperimentDefinition(
        name="High Cooperation Test",
        description="Test effect of high initial tokens on cooperation",
        variables=[
            ExperimentVariable("initial_tokens", default=100, type_hint="int"),
            ExperimentVariable("tax_rate", default=0.1, type_hint="float"),
        ],
        groups=[
            ExperimentGroup("control", variables={"initial_tokens": 100, "tax_rate": 0.1}),
            ExperimentGroup("treatment", variables={"initial_tokens": 500, "tax_rate": 0.1}),
        ],
        hypothesis=Hypothesis(
            null="Initial tokens have no effect on survival rate",
            alternative="Higher initial tokens increase survival rate",
            direction="greater",
            metric="survival_rate",
            alpha=0.05,
        ),
        agent_count=50,
        duration_ticks=10000,
    )

    # Validate and convert to config
    errors = exp.validate()
    assert not errors, errors

    # Generate ExperimentConfig for each group
    configs = exp.to_configs(base_seed=42)
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any, Literal

import yaml

from agent_runtime.experiment.config import ExperimentConfig

# ---------------------------------------------------------------------------
# Core DSL types
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class ExperimentVariable:
    """A single experimental variable being tested.

    Attributes:
        name: Variable name (e.g., "initial_tokens").
        default: Default value for the control group.
        type_hint: Type hint ("int", "float", "bool", "str").
        description: Human-readable description.
        min_value: Optional minimum value (for numeric types).
        max_value: Optional maximum value (for numeric types).
    """

    name: str
    default: Any = None
    type_hint: str = "str"
    description: str = ""
    min_value: float | None = None
    max_value: float | None = None

    def validate_value(self, value: Any) -> list[str]:
        """Validate a value against this variable's constraints."""
        errors: list[str] = []

        if self.type_hint == "int":
            if not isinstance(value, int) or isinstance(value, bool):
                errors.append(f"Variable '{self.name}': expected int, got {type(value).__name__}")
        elif self.type_hint == "float":
            if not isinstance(value, (int, float)) or isinstance(value, bool):
                errors.append(f"Variable '{self.name}': expected float, got {type(value).__name__}")
        elif self.type_hint == "bool":
            if not isinstance(value, bool):
                errors.append(f"Variable '{self.name}': expected bool, got {type(value).__name__}")

        if self.min_value is not None and isinstance(value, (int, float)):
            if value < self.min_value:
                errors.append(
                    f"Variable '{self.name}': value {value} < min {self.min_value}"
                )
        if self.max_value is not None and isinstance(value, (int, float)):
            if value > self.max_value:
                errors.append(
                    f"Variable '{self.name}': value {value} > max {self.max_value}"
                )

        return errors


@dataclass(frozen=True)
class ExperimentGroup:
    """A named group in an A/B experiment.

    Attributes:
        name: Group name (e.g., "control", "treatment_a").
        variables: Dict of variable overrides for this group.
        description: Human-readable description.
        agent_ratio: Proportion of agents to assign to this group (0.0-1.0).
            If not set, agents are split equally across groups.
    """

    name: str
    variables: dict[str, Any] = field(default_factory=dict)
    description: str = ""
    agent_ratio: float | None = None

    def is_control(self) -> bool:
        """Check if this group is the control group."""
        return self.name.lower() in ("control", "baseline", "default", "a")


@dataclass(frozen=True)
class Hypothesis:
    """Formal hypothesis statement for the experiment.

    Attributes:
        null: Null hypothesis statement (H0).
        alternative: Alternative hypothesis statement (H1).
        direction: Test direction — "two-sided", "greater", or "less".
        metric: Primary metric to test (e.g., "survival_rate").
        alpha: Significance level (default 0.05).
        min_sample_size: Minimum sample size per group for adequate power.
    """

    null: str = ""
    alternative: str = ""
    direction: Literal["two-sided", "greater", "less"] = "two-sided"
    metric: str = ""
    alpha: float = 0.05
    min_sample_size: int = 30


@dataclass
class ExperimentDefinition:
    """Top-level experiment definition using the DSL.

    Attributes:
        name: Experiment name.
        description: Human-readable description.
        variables: List of variables being tested.
        groups: List of experiment groups (control + treatment(s)).
        hypothesis: Formal hypothesis (optional).
        agent_count: Total number of agents.
        duration_ticks: Simulation duration in ticks.
        base_seed: Base random seed for reproducibility.
        world_config: Optional world configuration overrides.
        llm_config: Optional LLM configuration overrides.
    """

    name: str = ""
    description: str = ""
    variables: list[ExperimentVariable] = field(default_factory=list)
    groups: list[ExperimentGroup] = field(default_factory=list)
    hypothesis: Hypothesis | None = None
    agent_count: int = 50
    duration_ticks: int = 10000
    base_seed: int = 42
    world_config: dict[str, Any] = field(default_factory=dict)
    llm_config: dict[str, Any] = field(default_factory=dict)

    # -------------------------------------------------------------------
    # Validation
    # -------------------------------------------------------------------

    def validate(self) -> list[str]:
        """Validate the experiment definition.

        Returns:
            List of error strings (empty if valid).
        """
        errors: list[str] = []

        if not self.name:
            errors.append("Experiment name is required")

        if len(self.groups) < 2:
            errors.append("At least 2 groups (control + treatment) are required")

        # Check for control group
        has_control = any(g.is_control() for g in self.groups)
        if not has_control and len(self.groups) >= 2:
            errors.append(
                "No control group found. Name one group 'control' or 'baseline'."
            )

        # Check unique group names
        names = [g.name for g in self.groups]
        if len(names) != len(set(names)):
            errors.append("Group names must be unique")

        # Check agent ratios sum to ~1.0
        ratios = [g.agent_ratio for g in self.groups if g.agent_ratio is not None]
        if ratios:
            total = sum(ratios)
            if abs(total - 1.0) > 0.01:
                errors.append(
                    f"Agent ratios sum to {total:.2f}, expected 1.0"
                )

        # Validate variable values in each group
        var_map = {v.name: v for v in self.variables}
        for group in self.groups:
            for var_name, value in group.variables.items():
                if var_name in var_map:
                    errs = var_map[var_name].validate_value(value)
                    errors.extend(errs)
                # Unknown variables are allowed (may be custom parameters)

        # Validate hypothesis
        if self.hypothesis:
            if self.hypothesis.alpha <= 0 or self.hypothesis.alpha >= 1:
                errors.append("Hypothesis alpha must be between 0 and 1")
            if self.hypothesis.direction not in ("two-sided", "greater", "less"):
                errors.append(
                    f"Invalid hypothesis direction: {self.hypothesis.direction}"
                )

        if self.agent_count <= 0:
            errors.append("agent_count must be > 0")

        if self.duration_ticks <= 0:
            errors.append("duration_ticks must be > 0")

        return errors

    # -------------------------------------------------------------------
    # Agent assignment
    # -------------------------------------------------------------------

    def assign_agents(self, agent_ids: list[str]) -> dict[str, list[str]]:
        """Randomly assign agents to experiment groups.

        Uses deterministic seeding based on base_seed for reproducibility.

        Args:
            agent_ids: List of agent IDs to assign.

        Returns:
            Dict of group_name -> list of agent IDs.
        """
        import random

        rng = random.Random(self.base_seed)
        shuffled = list(agent_ids)
        rng.shuffle(shuffled)

        n = len(shuffled)
        assignment: dict[str, list[str]] = {g.name: [] for g in self.groups}

        # Calculate group sizes
        ratios = self._get_ratios()
        boundaries = []
        cumsum = 0.0
        for ratio in ratios:
            cumsum += ratio
            boundaries.append(int(cumsum * n))

        # Assign agents to groups
        start = 0
        for i, group in enumerate(self.groups):
            end = boundaries[i]
            assignment[group.name] = shuffled[start:end]
            start = end

        # Handle remainder (due to rounding)
        if start < n:
            last_group = self.groups[-1].name
            assignment[last_group].extend(shuffled[start:])

        return assignment

    # -------------------------------------------------------------------
    # Convert to ExperimentConfig
    # -------------------------------------------------------------------

    def to_configs(self, base_seed: int | None = None) -> dict[str, ExperimentConfig]:
        """Generate ExperimentConfig for each group.

        Maps DSL variables to ExperimentConfig fields. Supported mappings:
        - initial_tokens -> agents.initial_tokens
        - agent_count -> agents.count
        - tax_rate -> governance.tax_rate
        - governance_enabled -> governance.enabled
        - resource_density -> world.resource_density
        - world_width -> world.width
        - world_height -> world.height
        - temperature -> llm.temperature
        - model -> llm.model

        Args:
            base_seed: Override base seed (uses self.base_seed if None).

        Returns:
            Dict of group_name -> ExperimentConfig.
        """
        seed = base_seed if base_seed is not None else self.base_seed
        configs: dict[str, ExperimentConfig] = {}

        for i, group in enumerate(self.groups):
            group_seed = self._derive_group_seed(seed, i)
            overrides = dict(group.variables)

            # Map DSL variables to config fields
            config_data: dict[str, Any] = {
                "experiment_id": f"{self.name.lower().replace(' ', '-')}-{group.name}",
                "name": f"{self.name} [{group.name}]",
                "seed": group_seed,
                "duration_ticks": self.duration_ticks,
                "agents": {
                    "count": overrides.pop("agent_count", self.agent_count),
                    "initial_tokens": overrides.pop("initial_tokens", 100),
                },
                "governance": {
                    "enabled": overrides.pop("governance_enabled", True),
                    "tax_rate": overrides.pop("tax_rate", 0.1),
                },
                "world": dict(self.world_config),
                "llm": dict(self.llm_config),
            }

            # Apply remaining world overrides
            for key in ("resource_density", "width", "height"):
                if key in overrides:
                    config_data["world"][key] = overrides.pop(key)

            # Apply LLM overrides
            for key in ("temperature", "model", "provider"):
                if key in overrides:
                    config_data["llm"][key] = overrides.pop(key)

            # Any remaining overrides go into a custom metadata bag
            if overrides:
                config_data["custom_parameters"] = overrides

            configs[group.name] = ExperimentConfig.from_dict(config_data)

        return configs

    # -------------------------------------------------------------------
    # YAML serialization
    # -------------------------------------------------------------------

    def to_yaml(self) -> str:
        """Serialize the experiment definition to YAML."""
        data = {
            "name": self.name,
            "description": self.description,
            "agent_count": self.agent_count,
            "duration_ticks": self.duration_ticks,
            "base_seed": self.base_seed,
            "variables": [
                {
                    "name": v.name,
                    "default": v.default,
                    "type": v.type_hint,
                    "description": v.description,
                    **({"min": v.min_value} if v.min_value is not None else {}),
                    **({"max": v.max_value} if v.max_value is not None else {}),
                }
                for v in self.variables
            ],
            "groups": [
                {
                    "name": g.name,
                    "variables": g.variables,
                    "description": g.description,
                    **({"agent_ratio": g.agent_ratio} if g.agent_ratio is not None else {}),
                }
                for g in self.groups
            ],
        }
        if self.hypothesis:
            data["hypothesis"] = {
                "null": self.hypothesis.null,
                "alternative": self.hypothesis.alternative,
                "direction": self.hypothesis.direction,
                "metric": self.hypothesis.metric,
                "alpha": self.hypothesis.alpha,
                "min_sample_size": self.hypothesis.min_sample_size,
            }
        if self.world_config:
            data["world_config"] = self.world_config
        if self.llm_config:
            data["llm_config"] = self.llm_config

        return yaml.dump(data, default_flow_style=False, allow_unicode=True, sort_keys=False)

    @classmethod
    def from_yaml(cls, yaml_str: str) -> ExperimentDefinition:
        """Parse an experiment definition from YAML string."""
        raw = yaml.safe_load(yaml_str)
        return cls.from_dict(raw)

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> ExperimentDefinition:
        """Parse an experiment definition from a dict."""
        variables = [
            ExperimentVariable(
                name=v["name"],
                default=v.get("default"),
                type_hint=v.get("type", "str"),
                description=v.get("description", ""),
                min_value=v.get("min"),
                max_value=v.get("max"),
            )
            for v in data.get("variables", [])
        ]

        groups = [
            ExperimentGroup(
                name=g["name"],
                variables=g.get("variables", {}),
                description=g.get("description", ""),
                agent_ratio=g.get("agent_ratio"),
            )
            for g in data.get("groups", [])
        ]

        hyp_data = data.get("hypothesis")
        hypothesis = (
            Hypothesis(
                null=hyp_data.get("null", ""),
                alternative=hyp_data.get("alternative", ""),
                direction=hyp_data.get("direction", "two-sided"),
                metric=hyp_data.get("metric", ""),
                alpha=hyp_data.get("alpha", 0.05),
                min_sample_size=hyp_data.get("min_sample_size", 30),
            )
            if hyp_data
            else None
        )

        return cls(
            name=data.get("name", ""),
            description=data.get("description", ""),
            variables=variables,
            groups=groups,
            hypothesis=hypothesis,
            agent_count=data.get("agent_count", 50),
            duration_ticks=data.get("duration_ticks", 10000),
            base_seed=data.get("base_seed", 42),
            world_config=data.get("world_config", {}),
            llm_config=data.get("llm_config", {}),
        )

    # -------------------------------------------------------------------
    # Internal helpers
    # -------------------------------------------------------------------

    def _get_ratios(self) -> list[float]:
        """Get agent ratios for each group, defaulting to equal split."""
        n = len(self.groups)
        if n == 0:
            return []

        has_explicit = any(g.agent_ratio is not None for g in self.groups)
        if has_explicit:
            # Fill in missing ratios
            explicit_count = sum(1 for g in self.groups if g.agent_ratio is not None)
            explicit_sum = sum(g.agent_ratio for g in self.groups if g.agent_ratio is not None)
            remaining = max(0.0, 1.0 - explicit_sum)
            implicit_count = n - explicit_count
            default_ratio = remaining / implicit_count if implicit_count > 0 else 0.0

            return [
                g.agent_ratio if g.agent_ratio is not None else default_ratio
                for g in self.groups
            ]
        else:
            # Equal split
            return [1.0 / n] * n

    @staticmethod
    def _derive_group_seed(base_seed: int, group_index: int) -> int:
        """Derive a deterministic seed for each group."""
        import hashlib
        key = f"{base_seed}:group:{group_index}"
        return int(hashlib.sha256(key.encode()).hexdigest(), 16) % (2**31)
