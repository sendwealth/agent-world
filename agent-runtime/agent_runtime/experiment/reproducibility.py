"""Reproducibility manager — seeded RNG and config snapshots.

Ensures experiments produce deterministic results by:
1. Capturing a full config snapshot (including defaults) at experiment start
2. Providing a seeded random number generator for deterministic decisions
3. Verifying that two runs with the same seed + config produce identical results
"""

from __future__ import annotations

import copy
import hashlib
import json
import random as stdlib_random
from dataclasses import dataclass, field
from typing import Any

from agent_runtime.experiment.config import ExperimentConfig


@dataclass
class ConfigSnapshot:
    """Immutable snapshot of an experiment's full configuration.

    Includes a content hash for quick equality checks.
    """

    config_dict: dict[str, Any]
    content_hash: str
    seed: int

    @classmethod
    def from_config(cls, config: ExperimentConfig) -> ConfigSnapshot:
        """Create a snapshot from an ExperimentConfig."""
        config_dict = config.to_dict()
        content = json.dumps(config_dict, sort_keys=True, ensure_ascii=False)
        content_hash = hashlib.sha256(content.encode()).hexdigest()
        return cls(
            config_dict=config_dict,
            content_hash=content_hash,
            seed=config.seed,
        )

    def to_dict(self) -> dict[str, Any]:
        """Serialize for storage."""
        return {
            "config": self.config_dict,
            "content_hash": self.content_hash,
            "seed": self.seed,
        }


class ReproducibilityManager:
    """Ensures experiment reproducibility via seeded RNG and config snapshots.

    Usage::

        manager = ReproducibilityManager(config)
        snapshot = manager.snapshot_config()

        # Deterministic random numbers
        value = manager.random.random()
        items = manager.random.sample(range(100), 10)

        # Verify two runs match
        assert manager.verify_reproducibility(result_a, result_b)
    """

    def __init__(self, config: ExperimentConfig) -> None:
        self.seed = config.seed
        self.random = stdlib_random.Random(config.seed)
        self._config = config
        self._snapshot: ConfigSnapshot | None = None

    def snapshot_config(self) -> ConfigSnapshot:
        """Export the full running configuration (including all defaults).

        The snapshot includes a SHA-256 content hash for quick equality
        comparison. The result is cached after the first call.

        Returns:
            ConfigSnapshot with full config dict and content hash.
        """
        if self._snapshot is None:
            self._snapshot = ConfigSnapshot.from_config(self._config)
        return self._snapshot

    def verify_reproducibility(
        self,
        run_a: dict[str, Any],
        run_b: dict[str, Any],
    ) -> bool:
        """Verify two runs produced the same results.

        Compares key metrics from both runs. Two experiments with the same
        seed + config should produce identical results if the simulation
        is deterministic.

        Args:
            run_a: First run's result dict (from ExperimentResult.to_dict()).
            run_b: Second run's result dict.

        Returns:
            True if the results match, False otherwise.
        """
        # Compare config snapshots first (quick rejection)
        config_a = run_a.get("config_snapshot", {})
        config_b = run_b.get("config_snapshot", {})
        if config_a != config_b:
            return False

        # Compare key metrics
        metrics_a = run_a.get("metrics_timeline", [])
        metrics_b = run_b.get("metrics_timeline", [])
        if metrics_a != metrics_b:
            return False

        # Compare final snapshot
        final_a = run_a.get("final_snapshot", {})
        final_b = run_b.get("final_snapshot", {})
        if final_a != final_b:
            return False

        return True

    def reset_rng(self) -> None:
        """Reset the RNG to the original seed (for re-running experiments)."""
        self.random = stdlib_random.Random(self.seed)

    def derive_child_seed(self, index: int) -> int:
        """Derive a deterministic child seed for parallel world instances.

        Used by A/B experiments to give each world a unique but
        deterministic seed derived from the parent.

        Args:
            index: Child index (0, 1, 2, ...).

        Returns:
            A deterministic seed value.
        """
        base = f"{self.seed}:{index}"
        return int(hashlib.sha256(base.encode()).hexdigest(), 16) % (2**31)
