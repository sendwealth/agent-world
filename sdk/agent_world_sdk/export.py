"""Data export sub-module — world state, graph, time series, and research formats.

Supports:
- JSON / CSV world state export
- GraphML social network export
- Economic time series export
- Agent behavior trajectory export
- Cultural evolution data export
"""

from __future__ import annotations

from typing import Any


class ExportModule:
    """Export world data in various formats."""

    def __init__(self, client: Any) -> None:
        self._client = client

    # -- World state ---------------------------------------------------------

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

    # -- Social network graph ------------------------------------------------

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

    # -- Time series ---------------------------------------------------------

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

    # -- Behavior log --------------------------------------------------------

    def behavior_log(
        self,
        agent_id: str | None = None,
        *,
        tick_start: int | None = None,
        tick_end: int | None = None,
        format: str = "json",
    ) -> Any:
        """Export agent behavior logs.

        Returns JSON dict or CSV string depending on *format*.
        """
        params: dict[str, Any] = {"format": format}
        if agent_id:
            params["agent_id"] = agent_id
        if tick_start is not None:
            params["tick_start"] = tick_start
        if tick_end is not None:
            params["tick_end"] = tick_end

        if format == "json":
            return self._client._get("/api/v2/behavior-log", params=params)
        resp = self._client._get_raw("/api/v2/behavior-log", params=params)
        resp.raise_for_status()
        return resp.text

    # -- Network graph (full analysis) ---------------------------------------

    def network_graph(
        self,
        *,
        tick_start: int | None = None,
        tick_end: int | None = None,
    ) -> Any:
        """Export full network analysis with communities and centrality."""
        params: dict[str, Any] = {}
        if tick_start is not None:
            params["tick_start"] = tick_start
        if tick_end is not None:
            params["tick_end"] = tick_end
        return self._client._get("/api/v2/network-graph", params=params)

    # -- Economic time series ------------------------------------------------

    def economic_timeseries(
        self,
        *,
        tick_start: int | None = None,
        tick_end: int | None = None,
        format: str = "json",
    ) -> Any:
        """Export economic time series (GDP, Gini, money supply, prices).

        Returns JSON dict or CSV string depending on *format*.
        """
        params: dict[str, Any] = {"format": format}
        if tick_start is not None:
            params["tick_start"] = tick_start
        if tick_end is not None:
            params["tick_end"] = tick_end
        if format == "json":
            return self._client._get("/api/v2/export/economic/timeseries", params=params)
        resp = self._client._get_raw("/api/v2/export/economic/timeseries", params=params)
        resp.raise_for_status()
        return resp.text

    # -- Cultural evolution --------------------------------------------------

    def cultural_data(
        self,
        *,
        tick_start: int | None = None,
        tick_end: int | None = None,
        format: str = "json",
    ) -> Any:
        """Export cultural evolution data (language changes, value shifts).

        Returns JSON dict or CSV string depending on *format*.
        """
        params: dict[str, Any] = {"format": format}
        if tick_start is not None:
            params["tick_start"] = tick_start
        if tick_end is not None:
            params["tick_end"] = tick_end
        if format == "json":
            return self._client._get("/api/v2/export/cultural", params=params)
        resp = self._client._get_raw("/api/v2/export/cultural", params=params)
        resp.raise_for_status()
        return resp.text
