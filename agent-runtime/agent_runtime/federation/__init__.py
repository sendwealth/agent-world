"""Federation client — interact with the World Registry and Migration APIs."""

from __future__ import annotations

import json
from typing import Any, Dict, List, Optional

import httpx


class FederationClient:
    """HTTP client for the World Engine federation endpoints."""

    def __init__(self, base_url: str = "http://localhost:8080") -> None:
        self.base_url = base_url.rstrip("/")
        self._client = httpx.Client(timeout=30.0)

    def _url(self, path: str) -> str:
        return f"{self.base_url}{path}"

    # ── World Registry ──────────────────────────────────────

    def register_world(
        self,
        world_id: str,
        name: str,
        host: str,
        grpc_port: int = 50051,
        http_port: int = 8080,
        description: str = "",
        capabilities: Optional[List[str]] = None,
        max_agents: int = 100,
        labels: Optional[Dict[str, str]] = None,
    ) -> Dict[str, Any]:
        payload = {
            "world_id": world_id,
            "name": name,
            "host": host,
            "grpc_port": grpc_port,
            "http_port": http_port,
            "description": description,
            "capabilities": capabilities or [],
            "max_agents": max_agents,
            "labels": labels or {},
        }
        resp = self._client.post(self._url("/api/v1/federation/worlds"), json=payload)
        resp.raise_for_status()
        return resp.json()

    def list_worlds(self) -> List[Dict[str, Any]]:
        resp = self._client.get(self._url("/api/v1/federation/worlds"))
        resp.raise_for_status()
        return resp.json()

    def get_world(self, world_id: str) -> Dict[str, Any]:
        resp = self._client.get(self._url(f"/api/v1/federation/worlds/{world_id}"))
        resp.raise_for_status()
        return resp.json()

    def deregister_world(self, world_id: str) -> Dict[str, Any]:
        resp = self._client.delete(self._url(f"/api/v1/federation/worlds/{world_id}"))
        resp.raise_for_status()
        return resp.json()

    def heartbeat(self, world_id: str, metrics: Optional[Dict[str, Any]] = None) -> Dict[str, Any]:
        resp = self._client.post(
            self._url(f"/api/v1/federation/worlds/{world_id}/heartbeat"),
            json={"metrics": metrics or {}},
        )
        resp.raise_for_status()
        return resp.json()

    # ── Migration ────────────────────────────────────────────

    def submit_migration(
        self,
        agent_id: str,
        source_world_id: str,
        target_world_id: str,
        name: str = "",
        phase: str = "adult",
        tokens: int = 0,
        money: int = 0,
        reputation: float = 0.0,
        skills: Optional[Dict[str, int]] = None,
        public_key: str = "",
    ) -> Dict[str, Any]:
        payload = {
            "agent_id": agent_id,
            "source_world_id": source_world_id,
            "target_world_id": target_world_id,
            "name": name,
            "phase": phase,
            "tokens": tokens,
            "money": money,
            "reputation": reputation,
            "skills": skills or {},
            "public_key": public_key,
        }
        resp = self._client.post(self._url("/api/v1/migration/submit"), json=payload)
        resp.raise_for_status()
        return resp.json()

    def review_migration(
        self,
        migration_id: str,
        approved: bool,
        reviewer_world_id: str,
        rejection_reason: Optional[str] = None,
    ) -> Dict[str, Any]:
        payload = {
            "migration_id": migration_id,
            "approved": approved,
            "reviewer_world_id": reviewer_world_id,
            "rejection_reason": rejection_reason,
        }
        resp = self._client.post(
            self._url(f"/api/v1/migration/{migration_id}/review"), json=payload
        )
        resp.raise_for_status()
        return resp.json()

    def execute_migration(self, migration_id: str) -> Dict[str, Any]:
        resp = self._client.post(
            self._url(f"/api/v1/migration/{migration_id}/execute")
        )
        resp.raise_for_status()
        return resp.json()

    def cancel_migration(
        self,
        migration_id: str,
        cancelled_by: str,
        reason: Optional[str] = None,
    ) -> Dict[str, Any]:
        payload = {
            "cancelled_by": cancelled_by,
            "reason": reason,
        }
        resp = self._client.post(
            self._url(f"/api/v1/migration/{migration_id}/cancel"), json=payload
        )
        resp.raise_for_status()
        return resp.json()

    def get_migration_status(self, migration_id: str) -> Dict[str, Any]:
        resp = self._client.get(
            self._url(f"/api/v1/migration/{migration_id}")
        )
        resp.raise_for_status()
        return resp.json()

    def list_migrations(
        self,
        world_id: Optional[str] = None,
        inbound: bool = True,
        status_filter: Optional[str] = None,
        limit: int = 10,
        offset: int = 0,
    ) -> Dict[str, Any]:
        payload = {
            "world_id": world_id,
            "inbound": inbound,
            "status_filter": status_filter,
            "limit": limit,
            "offset": offset,
        }
        resp = self._client.post(self._url("/api/v1/migration/list"), json=payload)
        resp.raise_for_status()
        return resp.json()

    def get_migration_policy(self) -> Dict[str, Any]:
        resp = self._client.get(self._url("/api/v1/migration/policy"))
        resp.raise_for_status()
        return resp.json()

    def close(self) -> None:
        self._client.close()

    def __enter__(self) -> "FederationClient":
        return self

    def __exit__(self, *args: Any) -> None:
        self.close()
