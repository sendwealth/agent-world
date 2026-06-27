"""Python SDK client for the Agent World Third-Party Agent API.

Usage::

    from agent_runtime.sdk import AgentWorldClient

    client = AgentWorldClient("http://localhost:8080")
    resp = client.register("my-agent", capabilities=["move", "gather"])
    print(resp["agent_id"], resp["api_key"])

    perception = client.perception(resp["agent_id"])
    result = client.action(resp["agent_id"], "move", {"direction": "north"})
"""

from __future__ import annotations

import logging
from typing import Any

import httpx

logger = logging.getLogger(__name__)

# Default API version prefix.
API_PREFIX = "/api/v1"


class AgentWorldClient:
    """High-level client for the Agent World Third-Party Agent API.

    Attributes:
        base_url: Root URL of the World Engine (e.g. ``http://localhost:8080``).
        timeout: HTTP request timeout in seconds.
    """

    def __init__(self, base_url: str, *, timeout: float = 10.0) -> None:
        self.base_url = base_url.rstrip("/")
        self.timeout = timeout
        self._client = httpx.Client(base_url=self.base_url, timeout=timeout)
        self._api_key: str | None = None
        self._agent_id: str | None = None

    # ── Properties ───────────────────────────────────────────────

    @property
    def agent_id(self) -> str | None:
        """ID of the currently registered agent (set after ``register``)."""
        return self._agent_id

    @property
    def api_key(self) -> str | None:
        """API key for the currently registered agent."""
        return self._api_key

    # ── Core API methods ─────────────────────────────────────────

    def register(
        self,
        name: str,
        *,
        capabilities: list[str] | None = None,
        config: dict[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Register a new external agent.

        Args:
            name: Agent display name.
            capabilities: List of action types this agent can perform.
            config: Optional configuration (initial_tokens, etc.).

        Returns:
            Dict with ``agent_id`` and ``api_key``.
        """
        body: dict[str, Any] = {"name": name}
        if capabilities:
            body["capabilities"] = capabilities
        if config:
            body["config"] = config

        resp = self._client.post(f"{API_PREFIX}/agents/register", json=body)
        resp.raise_for_status()
        data = resp.json()

        self._agent_id = data["agent_id"]
        self._api_key = data["api_key"]

        logger.info("Registered agent %s (id=%s)", name, self._agent_id)
        return data

    def status(self, agent_id: str | None = None) -> dict[str, Any]:
        """Get the status of an external agent.

        Args:
            agent_id: Agent ID (defaults to the registered agent).

        Returns:
            Dict with agent status fields.
        """
        aid = agent_id or self._require_agent_id()
        resp = self._client.get(f"{API_PREFIX}/agents/{aid}/status")
        resp.raise_for_status()
        return resp.json()

    def deregister(self, agent_id: str | None = None) -> dict[str, Any]:
        """Deregister (remove) an external agent from the world.

        Args:
            agent_id: Agent ID (defaults to the registered agent).

        Returns:
            Confirmation dict.
        """
        aid = agent_id or self._require_agent_id()
        resp = self._client.delete(f"{API_PREFIX}/agents/{aid}")
        resp.raise_for_status()

        if aid == self._agent_id:
            self._agent_id = None
            self._api_key = None

        return resp.json()

    def action(
        self,
        agent_id: str | None,
        action: str,
        params: dict[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Execute an action as an external agent.

        Args:
            agent_id: Agent ID (defaults to the registered agent).
            action: Action type (e.g. ``move``, ``gather``).
            params: Action-specific parameters.

        Returns:
            Dict with ``success``, ``result``, and ``tick``.
        """
        aid = agent_id or self._require_agent_id()
        body: dict[str, Any] = {"action": action}
        if params:
            body["params"] = params

        resp = self._client.post(
            f"{API_PREFIX}/agents/{aid}/action", json=body
        )
        resp.raise_for_status()
        return resp.json()

    def perception(self, agent_id: str | None = None) -> dict[str, Any]:
        """Get perception data for an external agent.

        Args:
            agent_id: Agent ID (defaults to the registered agent).

        Returns:
            Dict with ``nearby_agents``, ``nearby_resources``, ``world_tick``.
        """
        aid = agent_id or self._require_agent_id()
        resp = self._client.get(f"{API_PREFIX}/agents/{aid}/perception")
        resp.raise_for_status()
        return resp.json()

    # ── Convenience shortcuts ────────────────────────────────────

    def move(self, direction: str, agent_id: str | None = None) -> dict[str, Any]:
        """Shortcut: move the agent in a direction."""
        return self.action(agent_id, "move", {"direction": direction})

    def gather(
        self, resource_type: str, agent_id: str | None = None
    ) -> dict[str, Any]:
        """Shortcut: gather a resource."""
        return self.action(agent_id, "gather", {"resource_type": resource_type})

    def communicate(
        self,
        target_agent_id: str,
        message: str,
        agent_id: str | None = None,
    ) -> dict[str, Any]:
        """Shortcut: send a message to another agent."""
        return self.action(
            agent_id,
            "communicate",
            {"target_agent_id": target_agent_id, "message": message},
        )

    def explore(self, agent_id: str | None = None) -> dict[str, Any]:
        """Shortcut: explore surroundings."""
        return self.action(agent_id, "explore")

    def rest(self, agent_id: str | None = None) -> dict[str, Any]:
        """Shortcut: rest for one tick."""
        return self.action(agent_id, "rest")

    def build(
        self, structure_type: str, agent_id: str | None = None
    ) -> dict[str, Any]:
        """Shortcut: build a structure."""
        return self.action(agent_id, "build", {"structure_type": structure_type})

    # ── World-level helpers ──────────────────────────────────────

    def world_stats(self) -> dict[str, Any]:
        """Get world statistics."""
        resp = self._client.get(f"{API_PREFIX}/world/stats")
        resp.raise_for_status()
        return resp.json()

    def tick(self) -> dict[str, Any]:
        """Get the current world tick."""
        resp = self._client.get(f"{API_PREFIX}/tick")
        resp.raise_for_status()
        return resp.json()

    # ── Lifecycle ────────────────────────────────────────────────

    def close(self) -> None:
        """Close the underlying HTTP client."""
        self._client.close()

    def __enter__(self) -> AgentWorldClient:
        return self

    def __exit__(self, *exc: Any) -> None:
        self.close()

    # ── Internal helpers ─────────────────────────────────────────

    def _require_agent_id(self) -> str:
        if self._agent_id is None:
            raise RuntimeError(
                "No agent registered yet. Call register() first."
            )
        return self._agent_id
