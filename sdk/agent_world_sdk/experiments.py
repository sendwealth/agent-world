"""Experiment management sub-module."""

from __future__ import annotations

from typing import Any


class Experiment:
    """Handle to a running or completed experiment."""

    def __init__(self, client: Any, experiment_id: str) -> None:
        self._client = client
        self.id = experiment_id

    def start(self) -> dict:
        """Start the experiment."""
        return self._client._post(f"/api/v2/experiments/{self.id}/start")

    def stop(self) -> dict:
        """Stop the experiment."""
        return self._client._post(f"/api/v2/experiments/{self.id}/stop")

    def pause(self) -> dict:
        """Pause the experiment."""
        return self._client._post(f"/api/v2/experiments/{self.id}/pause")

    def resume(self) -> dict:
        """Resume a paused experiment."""
        return self._client._post(f"/api/v2/experiments/{self.id}/resume")

    def inject(
        self,
        injection_type: str,
        *,
        agent_id: str | None = None,
        payload: Any = None,
    ) -> dict:
        """Inject an event or attribute modification."""
        body: dict[str, Any] = {"injection_type": injection_type}
        if agent_id is not None:
            body["agent_id"] = agent_id
        if payload is not None:
            body["payload"] = payload
        return self._client._post(
            f"/api/v2/experiments/{self.id}/inject", json=body
        )

    def results(self) -> dict:
        """Retrieve full experiment results."""
        return self._client._get(f"/api/v2/experiments/{self.id}/results")


class ExperimentsModule:
    """Create and manage experiments."""

    def __init__(self, client: Any) -> None:
        self._client = client

    def create(
        self,
        *,
        agent_count: int | None = None,
        target_ticks: int | None = None,
        llm_model: str | None = None,
        llm_temperature: float | None = None,
        description: str = "",
    ) -> Experiment:
        """Create a new experiment and return an Experiment handle."""
        body: dict[str, Any] = {"description": description}
        if agent_count is not None:
            body["agent_count"] = agent_count
        if target_ticks is not None:
            body["target_ticks"] = target_ticks
        if llm_model is not None:
            body["llm_model"] = llm_model
        if llm_temperature is not None:
            body["llm_temperature"] = llm_temperature

        result = self._client._post("/api/v2/experiments", json=body)
        return Experiment(self._client, result["experiment_id"])

    def list(self) -> list[dict]:
        """List all experiments (summaries)."""
        return self._client._get("/api/v2/experiments")
