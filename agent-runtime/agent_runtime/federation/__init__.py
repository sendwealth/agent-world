"""Federation client — interact with the World Registry and Migration APIs.

All methods degrade gracefully on HTTP errors (404/500/etc.):
they log a warning and return a safe empty result instead of raising.
Callers that need to distinguish success from failure can check
the ``error`` key in the returned dict.

Backend support status (``world-engine/src/api_federation.rs``):
    All routes are implemented under ``/api/v1/federation/*``,
    ``/api/v1/migration/*``, and ``/api/v1/agents/*/immigration-status``.
    If a route is temporarily unavailable, the client degrades gracefully.
"""

from __future__ import annotations

import logging
from typing import Any, Dict, List, Optional

import httpx

logger = logging.getLogger(__name__)


class FederationClient:
    """HTTP client for the World Engine federation and migration endpoints."""

    def __init__(self, base_url: str = "http://localhost:8080") -> None:
        self.base_url = base_url.rstrip("/")
        self._client = httpx.Client(timeout=30.0)

    def _url(self, path: str) -> str:
        return f"{self.base_url}{path}"

    def _unwrap(self, resp: httpx.Response) -> Dict[str, Any]:
        """Parse the { data, error, request_id } envelope."""
        resp.raise_for_status()
        body = resp.json()
        if body.get("error"):
            raise RuntimeError(f"[{body.get('request_id')}] {body['error']}")
        return body.get("data", body)

    def _safe_request(
        self,
        method: str,
        path: str,
        *,
        json: Optional[Dict[str, Any]] = None,
        default: Any = None,
    ) -> Any:
        """Execute an HTTP request with graceful error handling.

        On HTTP errors (404/500/connection failures), logs a warning and
        returns *default* instead of raising.
        """
        try:
            if method == "GET":
                resp = self._client.get(self._url(path))
            elif method == "POST":
                resp = self._client.post(self._url(path), json=json)
            elif method == "PUT":
                resp = self._client.put(self._url(path), json=json)
            elif method == "DELETE":
                resp = self._client.delete(self._url(path))
            else:
                raise ValueError(f"Unsupported HTTP method: {method}")
            return self._unwrap(resp)
        except httpx.HTTPStatusError as exc:
            logger.warning(
                "Federation %s %s failed: %s — returning default",
                method,
                path,
                exc.response.status_code,
            )
            return default if default is not None else {"error": str(exc)}
        except (httpx.HTTPError, RuntimeError) as exc:
            logger.warning("Federation %s %s failed: %s", method, path, exc)
            return default if default is not None else {"error": str(exc)}

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
        return self._safe_request(
            "POST",
            "/api/v1/federation/worlds",
            json={
                "world_id": world_id,
                "name": name,
                "host": host,
                "grpc_port": grpc_port,
                "http_port": http_port,
                "description": description,
                "capabilities": capabilities or [],
                "max_agents": max_agents,
                "labels": labels or {},
            },
        )

    def list_worlds(self) -> List[Dict[str, Any]]:
        result = self._safe_request("GET", "/api/v1/federation/worlds", default=[])
        return result if isinstance(result, list) else []

    def get_world(self, world_id: str) -> Dict[str, Any]:
        return self._safe_request("GET", f"/api/v1/federation/worlds/{world_id}")

    def deregister_world(self, world_id: str) -> Dict[str, Any]:
        return self._safe_request("DELETE", f"/api/v1/federation/worlds/{world_id}")

    def heartbeat(self, world_id: str, metrics: Optional[Dict[str, Any]] = None) -> Dict[str, Any]:
        return self._safe_request(
            "POST",
            f"/api/v1/federation/worlds/{world_id}/heartbeat",
            json=metrics or {},
        )

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
        return self._safe_request(
            "POST",
            "/api/v1/migration/submit",
            json={
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
            },
        )

    def review_migration(
        self,
        migration_id: str,
        approved: bool,
        reviewer_world_id: str,
        rejection_reason: Optional[str] = None,
    ) -> Dict[str, Any]:
        return self._safe_request(
            "POST",
            f"/api/v1/migration/{migration_id}/review",
            json={
                "migration_id": migration_id,
                "approved": approved,
                "reviewer_world_id": reviewer_world_id,
                "rejection_reason": rejection_reason,
            },
        )

    def execute_migration(self, migration_id: str) -> Dict[str, Any]:
        return self._safe_request("POST", f"/api/v1/migration/{migration_id}/execute")

    def cancel_migration(
        self,
        migration_id: str,
        cancelled_by: str,
        reason: Optional[str] = None,
    ) -> Dict[str, Any]:
        return self._safe_request(
            "POST",
            f"/api/v1/migration/{migration_id}/cancel",
            json={"cancelled_by": cancelled_by, "reason": reason},
        )

    def get_migration_status(self, migration_id: str) -> Dict[str, Any]:
        return self._safe_request("GET", f"/api/v1/migration/{migration_id}")

    def list_migrations(
        self,
        world_id: Optional[str] = None,
        inbound: bool = True,
        status_filter: Optional[str] = None,
        limit: int = 10,
        offset: int = 0,
    ) -> List[Dict[str, Any]]:
        body: Dict[str, Any] = {
            "inbound": inbound,
            "limit": limit,
            "offset": offset,
        }
        if world_id:
            body["world_id"] = world_id
        if status_filter:
            body["status_filter"] = status_filter
        result = self._safe_request(
            "POST", "/api/v1/migration/list", json=body, default=[]
        )
        return result if isinstance(result, list) else []

    def get_migration_policy(self) -> Dict[str, Any]:
        return self._safe_request("GET", "/api/v1/migration/policy")

    def update_migration_policy(self, **kwargs: Any) -> Dict[str, Any]:
        return self._safe_request("PUT", "/api/v1/migration/policy", json=kwargs)

    def get_migration_stats(self) -> Dict[str, Any]:
        return self._safe_request("GET", "/api/v1/migration/stats")

    def get_agent_immigration_status(self, agent_id: str) -> Dict[str, Any]:
        return self._safe_request("GET", f"/api/v1/agents/{agent_id}/immigration-status")
