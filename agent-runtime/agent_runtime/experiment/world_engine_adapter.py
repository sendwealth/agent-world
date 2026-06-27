"""World Engine API adapter — connects Python experiment framework to the Rust World Engine.

Provides a high-level client that wraps the World Engine's A/B experiment REST API
(``/api/v2/experiments/ab/*``), enabling the Python framework to:
- Create A/B experiments on the World Engine
- Start/stop experiments and capture snapshots
- Retrieve variant data and comparison results
- Export results as CSV

Usage::

    from agent_runtime.experiment.world_engine_adapter import WorldEngineAdapter

    adapter = WorldEngineAdapter("http://localhost:8080")
    exp_id = await adapter.create_experiment(
        name="Token Test",
        variants=[
            {"name": "control", "parameters": {"initial_tokens": "100"}},
            {"name": "treatment", "parameters": {"initial_tokens": "500"}},
        ],
    )
    await adapter.start_experiment(exp_id)
    # ... run simulation ...
    await adapter.capture_snapshot(exp_id, "control")
    await adapter.capture_snapshot(exp_id, "treatment")
    results = await adapter.compare_variants(exp_id, "control", "treatment")
    await adapter.stop_experiment(exp_id)
"""

from __future__ import annotations

import logging
from dataclasses import dataclass, field
from typing import Any

import httpx

logger = logging.getLogger(__name__)

# Default API prefix for A/B experiments
AB_API_PREFIX = "/api/v2/experiments/ab"

# Default timeout in seconds
DEFAULT_TIMEOUT = 30.0


@dataclass
class VariantSnapshot:
    """Snapshot of a variant's state at a specific tick.

    Mirrors the Rust-side VariantSnapshot struct.
    """

    tick: int
    agent_count: int
    alive_count: int
    total_tokens: int
    total_money: int
    gini_coefficient: float | None
    org_count: int
    timestamp: str = ""


@dataclass
class MetricComparison:
    """Comparison of a single metric between two variants."""

    metric_name: str
    variant_a_mean: float
    variant_b_mean: float
    delta: float
    delta_percent: float | None
    p_value: float | None
    significant: bool | None


@dataclass
class VariantComparisonResult:
    """Full comparison result between two variants."""

    variant_a: str
    variant_b: str
    metrics: list[MetricComparison] = field(default_factory=list)
    recommendation: str | None = None

    def to_dict(self) -> dict[str, Any]:
        return {
            "variant_a": self.variant_a,
            "variant_b": self.variant_b,
            "metrics": [
                {
                    "metric_name": m.metric_name,
                    "variant_a_mean": m.variant_a_mean,
                    "variant_b_mean": m.variant_b_mean,
                    "delta": m.delta,
                    "delta_percent": m.delta_percent,
                    "p_value": m.p_value,
                    "significant": m.significant,
                }
                for m in self.metrics
            ],
            "recommendation": self.recommendation,
        }


class WorldEngineAdapter:
    """Async client for the World Engine A/B Experiment API.

    Provides high-level methods that map to the Rust World Engine's
    ``/api/v2/experiments/ab/*`` endpoints.

    Args:
        base_url: Root URL of the World Engine (e.g., ``http://localhost:8080``).
        timeout: HTTP request timeout in seconds.
    """

    def __init__(self, base_url: str, *, timeout: float = DEFAULT_TIMEOUT) -> None:
        self.base_url = base_url.rstrip("/")
        self.timeout = timeout
        self._client = httpx.AsyncClient(
            base_url=self.base_url,
            timeout=timeout,
        )

    # -------------------------------------------------------------------
    # Experiment lifecycle
    # -------------------------------------------------------------------

    async def create_experiment(
        self,
        name: str,
        variants: list[dict[str, Any]],
        description: str = "",
    ) -> str:
        """Create a new A/B experiment on the World Engine.

        Args:
            name: Experiment name.
            variants: List of variant configs with ``name`` and ``parameters``.
            description: Optional description.

        Returns:
            The experiment ID (UUID string).
        """
        body = {
            "name": name,
            "description": description,
            "variants": variants,
        }
        resp = await self._client.post(f"{AB_API_PREFIX}", json=body)
        resp.raise_for_status()
        data = resp.json()
        experiment_id = data["experiment_id"]
        logger.info("Created A/B experiment: %s (id=%s)", name, experiment_id)
        return experiment_id

    async def start_experiment(self, experiment_id: str) -> dict[str, Any]:
        """Start an A/B experiment.

        Args:
            experiment_id: The experiment UUID.

        Returns:
            Response data with status and start_tick.
        """
        resp = await self._client.post(f"{AB_API_PREFIX}/{experiment_id}/start")
        resp.raise_for_status()
        logger.info("Started experiment %s", experiment_id)
        return resp.json()

    async def stop_experiment(self, experiment_id: str) -> dict[str, Any]:
        """Stop a running A/B experiment.

        Args:
            experiment_id: The experiment UUID.

        Returns:
            Response data with status and end_tick.
        """
        resp = await self._client.post(f"{AB_API_PREFIX}/{experiment_id}/stop")
        resp.raise_for_status()
        logger.info("Stopped experiment %s", experiment_id)
        return resp.json()

    async def get_experiment(self, experiment_id: str) -> dict[str, Any]:
        """Get full experiment details.

        Args:
            experiment_id: The experiment UUID.

        Returns:
            Full experiment data including all variant snapshots.
        """
        resp = await self._client.get(f"{AB_API_PREFIX}/{experiment_id}")
        resp.raise_for_status()
        return resp.json()

    async def list_experiments(self) -> list[dict[str, Any]]:
        """List all A/B experiments.

        Returns:
            List of experiment summaries.
        """
        resp = await self._client.get(f"{AB_API_PREFIX}")
        resp.raise_for_status()
        return resp.json()

    # -------------------------------------------------------------------
    # Snapshot & comparison
    # -------------------------------------------------------------------

    async def capture_snapshot(
        self, experiment_id: str, variant_name: str
    ) -> dict[str, Any]:
        """Capture a snapshot for a specific variant.

        Records the current world state as a data point for the variant.

        Args:
            experiment_id: The experiment UUID.
            variant_name: Name of the variant to snapshot.

        Returns:
            Snapshot data with tick, agent_count, etc.
        """
        body = {"variant_name": variant_name}
        resp = await self._client.post(
            f"{AB_API_PREFIX}/{experiment_id}/snapshot", json=body
        )
        resp.raise_for_status()
        data = resp.json()
        logger.info(
            "Captured snapshot for %s at tick %s",
            variant_name,
            data.get("tick", "?"),
        )
        return data

    async def compare_variants(
        self,
        experiment_id: str,
        variant_a: str,
        variant_b: str,
    ) -> VariantComparisonResult:
        """Compare two variants using statistical tests.

        The World Engine performs Welch's t-test on all metrics.

        Args:
            experiment_id: The experiment UUID.
            variant_a: Name of variant A (typically "control").
            variant_b: Name of variant B (typically "treatment").

        Returns:
            VariantComparisonResult with metrics and recommendations.
        """
        params = {"variant_a": variant_a, "variant_b": variant_b}
        resp = await self._client.get(
            f"{AB_API_PREFIX}/{experiment_id}/compare", params=params
        )
        resp.raise_for_status()
        data = resp.json()
        return self._parse_comparison(data)

    async def export_csv(self, experiment_id: str) -> str:
        """Export experiment results as CSV.

        Args:
            experiment_id: The experiment UUID.

        Returns:
            CSV string with per-variant snapshot data.
        """
        resp = await self._client.get(f"{AB_API_PREFIX}/{experiment_id}/export")
        resp.raise_for_status()
        return resp.text

    # -------------------------------------------------------------------
    # Convenience: run full experiment cycle
    # -------------------------------------------------------------------

    async def run_experiment(
        self,
        name: str,
        variants: list[dict[str, Any]],
        snapshot_interval_ticks: int = 100,
        total_ticks: int = 1000,
        description: str = "",
    ) -> VariantComparisonResult:
        """Run a complete A/B experiment cycle.

        Creates the experiment, starts it, captures periodic snapshots,
        stops it, and returns the comparison results.

        Args:
            name: Experiment name.
            variants: Variant configurations.
            snapshot_interval_ticks: How often to capture snapshots (in ticks).
            total_ticks: Total ticks to run (approximate).
            description: Optional description.

        Returns:
            VariantComparisonResult with statistical comparison.
        """
        exp_id = await self.create_experiment(name, variants, description)
        await self.start_experiment(exp_id)

        try:
            # Capture snapshots periodically
            num_snapshots = max(1, total_ticks // snapshot_interval_ticks)
            for _ in range(num_snapshots):
                for variant in variants:
                    try:
                        await self.capture_snapshot(exp_id, variant["name"])
                    except httpx.HTTPStatusError as e:
                        logger.warning(
                            "Snapshot failed for %s: %s", variant["name"], e
                        )
        finally:
            await self.stop_experiment(exp_id)

        # Compare first two variants
        if len(variants) >= 2:
            return await self.compare_variants(
                exp_id, variants[0]["name"], variants[1]["name"]
            )

        raise ValueError("Need at least 2 variants for comparison")

    # -------------------------------------------------------------------
    # Lifecycle
    # -------------------------------------------------------------------

    async def close(self) -> None:
        """Close the underlying HTTP client."""
        await self._client.aclose()

    async def __aenter__(self) -> WorldEngineAdapter:
        return self

    async def __aexit__(self, *exc: Any) -> None:
        await self.close()

    # -------------------------------------------------------------------
    # Internal helpers
    # -------------------------------------------------------------------

    @staticmethod
    def _parse_comparison(data: dict[str, Any]) -> VariantComparisonResult:
        """Parse comparison response from the World Engine."""
        metrics = [
            MetricComparison(
                metric_name=m.get("metric_name", ""),
                variant_a_mean=m.get("variant_a_mean", 0.0),
                variant_b_mean=m.get("variant_b_mean", 0.0),
                delta=m.get("delta", 0.0),
                delta_percent=m.get("delta_percent"),
                p_value=m.get("p_value"),
                significant=m.get("significant"),
            )
            for m in data.get("metrics", [])
        ]
        return VariantComparisonResult(
            variant_a=data.get("variant_a", ""),
            variant_b=data.get("variant_b", ""),
            metrics=metrics,
            recommendation=data.get("recommendation"),
        )

    @staticmethod
    def parse_variant_data(raw: dict[str, Any]) -> list[VariantSnapshot]:
        """Parse variant snapshot data from the World Engine response."""
        snapshots: list[VariantSnapshot] = []
        for snap in raw.get("snapshots", []):
            snapshots.append(
                VariantSnapshot(
                    tick=snap.get("tick", 0),
                    agent_count=snap.get("agent_count", 0),
                    alive_count=snap.get("alive_count", 0),
                    total_tokens=snap.get("total_tokens", 0),
                    total_money=snap.get("total_money", 0),
                    gini_coefficient=snap.get("gini_coefficient"),
                    org_count=snap.get("org_count", 0),
                    timestamp=snap.get("timestamp", ""),
                )
            )
        return snapshots
