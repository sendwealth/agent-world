"""World, Agents, and Events sub-modules."""

from __future__ import annotations

from typing import Any, Iterator


class WorldModule:
    """Access world state and history."""

    def __init__(self, client: Any) -> None:
        self._client = client

    def state(self) -> dict:
        """Get aggregated world state."""
        return self._client._get("/api/v2/world/state")

    def history(
        self,
        *,
        from_tick: int | None = None,
        to_tick: int | None = None,
        limit: int | None = None,
    ) -> list[dict]:
        """Query historical snapshots."""
        params: dict[str, Any] = {}
        if from_tick is not None:
            params["from_tick"] = from_tick
        if to_tick is not None:
            params["to_tick"] = to_tick
        if limit is not None:
            params["limit"] = limit
        return self._client._get("/api/v2/world/history", params=params)


class AgentsModule:
    """Agent lookup and listing."""

    def __init__(self, client: Any) -> None:
        self._client = client

    def list(self) -> list[dict]:
        """List all agents via the v2 endpoint (auth-protected)."""
        return self._client._get("/api/v2/agents")

    def profile(self, agent_id: str) -> dict:
        """Get deep agent profile."""
        return self._client._get(f"/api/v2/agents/{agent_id}/profile")


class EventsModule:
    """SSE event stream access."""

    def __init__(self, client: Any) -> None:
        self._client = client

    def stream(
        self,
        *,
        types: str | None = None,
        agent_id: str | None = None,
    ) -> Iterator[dict]:
        """Yield SSE events as dicts.

        This uses httpx streaming to read the SSE endpoint.
        """
        import json as _json

        params: dict[str, str] = {}
        if types:
            params["types"] = types
        if agent_id:
            params["agent_id"] = agent_id

        with self._client._http.stream(
            "GET", "/api/v2/world/events/stream", params=params
        ) as resp:
            for line in resp.iter_lines():
                line = line.strip()
                if not line or line.startswith(":"):
                    continue
                if line.startswith("data:"):
                    data = line[len("data:"):].strip()
                    try:
                        yield _json.loads(data)
                    except _json.JSONDecodeError:
                        yield {"raw": data}
