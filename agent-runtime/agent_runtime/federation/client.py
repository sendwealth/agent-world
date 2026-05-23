"""Federation client for cross-world communication and agent migration.

Provides a Python client for interacting with the World Engine federation
and migration REST API endpoints.
"""

from __future__ import annotations

from typing import Any, Dict, List, Optional

import httpx


class FederationClient:
    """Client for the World Engine federation and migration REST API.

    Args:
        base_url: Base URL of the World Engine API (e.g. ``http://localhost:3000``).
    """

    def __init__(self, base_url: str) -> None:
        self._base_url = base_url.rstrip("/")
        self._client = httpx.Client(timeout=30.0)

    # ── Federation (World Registry) ────────────────────────

    def register_world(
        self,
        *,
        world_id: str,
        name: str,
        description: str = "",
        host: str,
        grpc_port: int,
        http_port: int,
        capabilities: Optional[List[str]] = None,
        max_agents: int = 100,
        labels: Optional[Dict[str, str]] = None,
    ) -> httpx.Response:
        """Register a new world with the federation registry.

        POST ``/api/v1/federation/worlds``
        """
        payload: Dict[str, Any] = {
            "world_id": world_id,
            "name": name,
            "description": description,
            "host": host,
            "grpc_port": grpc_port,
            "http_port": http_port,
            "capabilities": capabilities or [],
            "max_agents": max_agents,
            "labels": labels or {},
        }
        return self._client.post(f"{self._base_url}/api/v1/federation/worlds", json=payload)

    def list_worlds(self) -> httpx.Response:
        """List all registered worlds.

        GET ``/api/v1/federation/worlds``
        """
        return self._client.get(f"{self._base_url}/api/v1/federation/worlds")

    def get_world(self, world_id: str) -> httpx.Response:
        """Get a specific world by ID.

        GET ``/api/v1/federation/worlds/{world_id}``
        """
        return self._client.get(f"{self._base_url}/api/v1/federation/worlds/{world_id}")

    def deregister_world(self, world_id: str) -> httpx.Response:
        """Deregister (remove) a world from the federation registry.

        DELETE ``/api/v1/federation/worlds/{world_id}``
        """
        return self._client.delete(f"{self._base_url}/api/v1/federation/worlds/{world_id}")

    def heartbeat(
        self, world_id: str, metrics: Optional[Dict[str, Any]] = None
    ) -> httpx.Response:
        """Send a heartbeat for a registered world.

        POST ``/api/v1/federation/worlds/{world_id}/heartbeat``
        """
        return self._client.post(
            f"{self._base_url}/api/v1/federation/worlds/{world_id}/heartbeat",
            json=metrics or {},
        )

    # ── Migration ──────────────────────────────────────────

    def submit_migration(
        self,
        agent_snapshot: Dict[str, Any],
        target_world_id: str,
    ) -> httpx.Response:
        """Submit a migration application for an agent.

        POST ``/api/v1/migration/submit``

        Args:
            agent_snapshot: Snapshot of the agent to migrate (agent_id, name,
                phase, tokens, money, reputation, skills, source_world_id, etc.).
            target_world_id: ID of the destination world.
        """
        payload = {
            **agent_snapshot,
            "target_world_id": target_world_id,
        }
        return self._client.post(f"{self._base_url}/api/v1/migration/submit", json=payload)

    def review_migration(
        self,
        migration_id: str,
        *,
        approved: bool,
        reviewer_world_id: str,
        rejection_reason: Optional[str] = None,
    ) -> httpx.Response:
        """Review (approve or reject) a pending migration.

        POST ``/api/v1/migration/{migration_id}/review``
        """
        payload: Dict[str, Any] = {
            "approved": approved,
            "reviewer_world_id": reviewer_world_id,
        }
        if rejection_reason is not None:
            payload["rejection_reason"] = rejection_reason
        return self._client.post(
            f"{self._base_url}/api/v1/migration/{migration_id}/review", json=payload
        )

    def execute_migration(self, migration_id: str) -> httpx.Response:
        """Execute an approved migration.

        POST ``/api/v1/migration/{migration_id}/execute``
        """
        return self._client.post(
            f"{self._base_url}/api/v1/migration/{migration_id}/execute"
        )

    def cancel_migration(
        self,
        migration_id: str,
        *,
        cancelled_by: str,
        reason: Optional[str] = None,
    ) -> httpx.Response:
        """Cancel a migration.

        POST ``/api/v1/migration/{migration_id}/cancel``
        """
        payload: Dict[str, Any] = {"cancelled_by": cancelled_by}
        if reason is not None:
            payload["reason"] = reason
        return self._client.post(
            f"{self._base_url}/api/v1/migration/{migration_id}/cancel", json=payload
        )

    def get_migration_status(self, migration_id: str) -> httpx.Response:
        """Get the status of a specific migration.

        GET ``/api/v1/migration/{migration_id}``
        """
        return self._client.get(
            f"{self._base_url}/api/v1/migration/{migration_id}"
        )

    def list_migrations(
        self,
        *,
        world_id: Optional[str] = None,
        inbound: bool = True,
        status_filter: Optional[str] = None,
        limit: int = 10,
        offset: int = 0,
    ) -> httpx.Response:
        """List migrations with optional filters.

        POST ``/api/v1/migration/list``
        """
        payload: Dict[str, Any] = {
            "world_id": world_id,
            "inbound": inbound,
            "status_filter": status_filter,
            "limit": limit,
            "offset": offset,
        }
        return self._client.post(f"{self._base_url}/api/v1/migration/list", json=payload)

    def get_migration_policy(self) -> httpx.Response:
        """Get the current migration policy.

        GET ``/api/v1/migration/policy``
        """
        return self._client.get(f"{self._base_url}/api/v1/migration/policy")

    # ── Lifecycle ──────────────────────────────────────────

    def close(self) -> None:
        """Close the underlying HTTP client."""
        self._client.close()

    def __enter__(self) -> "FederationClient":
        return self

    def __exit__(self, *args: Any) -> None:
        self.close()
