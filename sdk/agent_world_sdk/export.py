"""Data export sub-module."""

from __future__ import annotations

from typing import Any


class ExportModule:
    """Export world data in various formats."""

    def __init__(self, client: Any) -> None:
        self._client = client

    def world(self, *, format: str = "json") -> Any:
        """Export world state snapshot.

        Returns JSON dict for format='json', raw CSV string for format='csv'.
        """
        if format == "json":
            return self._client._get("/api/v2/export/world", params={"format": "json"})
        # For CSV, get the raw text response.
        resp = self._client._get_raw("/api/v2/export/world", params={"format": "csv"})
        resp.raise_for_status()
        return resp.text

    def graph(self, *, format: str = "json") -> Any:
        """Export agent interaction graph.

        Returns JSON for format='json', GraphML XML string for format='graphml'.
        """
        if format == "json":
            return self._client._get(
                "/api/v2/export/agents/graph", params={"format": "json"}
            )
        resp = self._client._get_raw(
            "/api/v2/export/agents/graph", params={"format": "graphml"}
        )
        resp.raise_for_status()
        return resp.text

    def timeseries(self, *, format: str = "csv") -> Any:
        """Export emergence metrics time series.

        Returns CSV string for format='csv', JSON list for format='json'.
        """
        if format == "json":
            return self._client._get(
                "/api/v2/export/metrics/timeseries", params={"format": "json"}
            )
        resp = self._client._get_raw(
            "/api/v2/export/metrics/timeseries", params={"format": "csv"}
        )
        resp.raise_for_status()
        return resp.text
