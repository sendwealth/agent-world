"""Experiment configuration parser — YAML/TOML support with validation.

Loads experiment definitions from YAML or TOML files, validates required
fields, applies defaults, and produces an immutable ExperimentConfig.

Supported file formats (auto-detected by extension):
    - ``.yaml`` / ``.yml`` → parsed with ``pyyaml``
    - ``.toml`` → parsed with ``tomllib`` (stdlib, Python 3.11+)
"""

from __future__ import annotations

import logging
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

import yaml

logger = logging.getLogger(__name__)


# ---------------------------------------------------------------------------
# Sub-config dataclasses
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class WorldExperimentConfig:
    """World generation parameters for an experiment."""

    width: int = 100
    height: int = 100
    resource_density: float = 0.3


@dataclass(frozen=True)
class AgentsExperimentConfig:
    """Agent population parameters."""

    count: int = 50
    personality_distribution: str = "random"  # random | clustered | uniform
    initial_tokens: int = 100


@dataclass(frozen=True)
class GovernanceExperimentConfig:
    """Governance / tax parameters."""

    enabled: bool = True
    tax_rate: float = 0.1


@dataclass(frozen=True)
class LLMExperimentConfig:
    """LLM provider settings for the experiment."""

    provider: str = "ollama"
    model: str = "qwen2"
    temperature: float = 0.7


@dataclass(frozen=True)
class TracingExperimentConfig:
    """Tracing / snapshot settings."""

    snapshot_interval: int = 100
    export_on_complete: bool = True


# ---------------------------------------------------------------------------
# Top-level config
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class ExperimentConfig:
    """Complete experiment configuration, loaded from YAML or TOML.

    Use ``ExperimentConfig.from_yaml()`` or ``ExperimentConfig.from_toml()``
    to parse a config file.  Call ``validate()`` to check for errors.
    """

    experiment_id: str = "unnamed"
    name: str = "Unnamed Experiment"
    seed: int = 42
    duration_ticks: int = 10000
    world: WorldExperimentConfig = field(default_factory=WorldExperimentConfig)
    agents: AgentsExperimentConfig = field(default_factory=AgentsExperimentConfig)
    governance: GovernanceExperimentConfig = field(default_factory=GovernanceExperimentConfig)
    llm: LLMExperimentConfig = field(default_factory=LLMExperimentConfig)
    tracing: TracingExperimentConfig = field(default_factory=TracingExperimentConfig)

    # -----------------------------------------------------------------------
    # Constructors
    # -----------------------------------------------------------------------

    @classmethod
    def from_yaml(cls, path: str | Path) -> ExperimentConfig:
        """Parse an experiment config from a YAML file.

        Args:
            path: Path to the YAML file.

        Returns:
            Fully constructed ExperimentConfig.

        Raises:
            FileNotFoundError: If the file does not exist.
            ValueError: If the file contains invalid data.
        """
        p = Path(path)
        if not p.exists():
            raise FileNotFoundError(f"Config file not found: {p}")

        with open(p) as f:
            raw = yaml.safe_load(f)

        if not isinstance(raw, dict):
            raise ValueError(f"Expected a YAML mapping, got {type(raw).__name__}")

        return cls._from_dict(raw)

    @classmethod
    def from_toml(cls, path: str | Path) -> ExperimentConfig:
        """Parse an experiment config from a TOML file.

        Args:
            path: Path to the TOML file.

        Returns:
            Fully constructed ExperimentConfig.

        Raises:
            FileNotFoundError: If the file does not exist.
            ValueError: If the file contains invalid data.
            ImportError: If TOML support is unavailable.
        """
        p = Path(path)
        if not p.exists():
            raise FileNotFoundError(f"Config file not found: {p}")

        try:
            import tomllib
        except ModuleNotFoundError:
            try:
                import tomli as tomllib  # type: ignore[no-redef]
            except ModuleNotFoundError:
                raise ImportError(
                    "TOML support requires Python 3.11+ (tomllib) or the 'tomli' package"
                )

        with open(p, "rb") as f:
            raw = tomllib.load(f)

        return cls._from_dict(raw)

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> ExperimentConfig:
        """Construct from a raw dict (e.g., for testing or programmatic use).

        Args:
            data: Dict with experiment configuration.

        Returns:
            ExperimentConfig instance.
        """
        return cls._from_dict(data)

    # -----------------------------------------------------------------------
    # Validation
    # -----------------------------------------------------------------------

    def validate(self) -> list[str]:
        """Validate the config and return a list of error messages.

        Returns:
            Empty list if valid, otherwise human-readable error strings.
        """
        errors: list[str] = []

        if not self.experiment_id or self.experiment_id == "unnamed":
            errors.append("experiment_id must be a non-empty, meaningful identifier")

        if self.duration_ticks <= 0:
            errors.append("duration_ticks must be > 0")

        if self.seed < 0:
            errors.append("seed must be >= 0")

        if self.agents.count <= 0:
            errors.append("agents.count must be > 0")

        if self.agents.count > 1000:
            errors.append("agents.count > 1000 may cause performance issues")

        if self.world.width <= 0 or self.world.height <= 0:
            errors.append("world dimensions must be > 0")

        if not 0.0 <= self.world.resource_density <= 1.0:
            errors.append("world.resource_density must be between 0.0 and 1.0")

        if self.agents.personality_distribution not in ("random", "clustered", "uniform"):
            errors.append(
                f"agents.personality_distribution must be random|clustered|uniform, "
                f"got {self.agents.personality_distribution!r}"
            )

        if not 0.0 <= self.governance.tax_rate <= 1.0:
            errors.append("governance.tax_rate must be between 0.0 and 1.0")

        if self.tracing.snapshot_interval <= 0:
            errors.append("tracing.snapshot_interval must be > 0")

        if self.agents.initial_tokens < 0:
            errors.append("agents.initial_tokens must be >= 0")

        return errors

    # -----------------------------------------------------------------------
    # Serialization
    # -----------------------------------------------------------------------

    def to_dict(self) -> dict[str, Any]:
        """Serialize to a JSON-safe dict (for archiving and reproducibility)."""
        return {
            "experiment_id": self.experiment_id,
            "name": self.name,
            "seed": self.seed,
            "duration_ticks": self.duration_ticks,
            "world": {
                "width": self.world.width,
                "height": self.world.height,
                "resource_density": self.world.resource_density,
            },
            "agents": {
                "count": self.agents.count,
                "personality_distribution": self.agents.personality_distribution,
                "initial_tokens": self.agents.initial_tokens,
            },
            "governance": {
                "enabled": self.governance.enabled,
                "tax_rate": self.governance.tax_rate,
            },
            "llm": {
                "provider": self.llm.provider,
                "model": self.llm.model,
                "temperature": self.llm.temperature,
            },
            "tracing": {
                "snapshot_interval": self.tracing.snapshot_interval,
                "export_on_complete": self.tracing.export_on_complete,
            },
        }

    # -----------------------------------------------------------------------
    # Internal helpers
    # -----------------------------------------------------------------------

    @classmethod
    def _from_dict(cls, raw: dict[str, Any]) -> ExperimentConfig:
        """Internal: parse from a raw dict with defaults.

        Supports both wrapped format (``{experiment: {id: ...}}``) and flat
        format (``{experiment_id: ..., seed: ...}``) — the latter is what
        ``to_dict()`` produces.
        """
        # Detect wrapped vs flat format
        if "experiment" in raw and isinstance(raw["experiment"], dict):
            exp = raw["experiment"]
        elif "experiment_id" in raw:
            # Flat format (from to_dict)
            exp = raw
        else:
            exp = raw.get("experiment", raw)

        return cls(
            experiment_id=exp.get("id", exp.get("experiment_id", "unnamed")),
            name=exp.get("name", "Unnamed Experiment"),
            seed=exp.get("seed", 42),
            duration_ticks=exp.get("duration_ticks", 10000),
            world=cls._parse_world(exp.get("world", {})),
            agents=cls._parse_agents(exp.get("agents", {})),
            governance=cls._parse_governance(exp.get("governance", {})),
            llm=cls._parse_llm(exp.get("llm", {})),
            tracing=cls._parse_tracing(exp.get("tracing", {})),
        )

    @staticmethod
    def _parse_world(data: dict[str, Any]) -> WorldExperimentConfig:
        return WorldExperimentConfig(
            width=data.get("width", 100),
            height=data.get("height", 100),
            resource_density=data.get("resource_density", 0.3),
        )

    @staticmethod
    def _parse_agents(data: dict[str, Any]) -> AgentsExperimentConfig:
        return AgentsExperimentConfig(
            count=data.get("count", 50),
            personality_distribution=data.get("personality_distribution", "random"),
            initial_tokens=data.get("initial_tokens", 100),
        )

    @staticmethod
    def _parse_governance(data: dict[str, Any]) -> GovernanceExperimentConfig:
        return GovernanceExperimentConfig(
            enabled=data.get("enabled", True),
            tax_rate=data.get("tax_rate", 0.1),
        )

    @staticmethod
    def _parse_llm(data: dict[str, Any]) -> LLMExperimentConfig:
        return LLMExperimentConfig(
            provider=data.get("provider", "ollama"),
            model=data.get("model", "qwen2"),
            temperature=data.get("temperature", 0.7),
        )

    @staticmethod
    def _parse_tracing(data: dict[str, Any]) -> TracingExperimentConfig:
        return TracingExperimentConfig(
            snapshot_interval=data.get("snapshot_interval", 100),
            export_on_complete=data.get("export_on_complete", True),
        )
