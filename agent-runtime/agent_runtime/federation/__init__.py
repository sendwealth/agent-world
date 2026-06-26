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

import asyncio
import logging
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

import httpx
import yaml

logger = logging.getLogger(__name__)


class FederationClient:
    """HTTP client for the World Engine federation and migration endpoints."""

    def __init__(self, base_url: str = "http://localhost:8080") -> None:
        self.base_url = base_url.rstrip("/")
        self._client = httpx.Client(timeout=30.0)

    def _url(self, path: str) -> str:
        return f"{self.base_url}{path}"

    def _unwrap(self, resp: httpx.Response) -> dict[str, Any]:
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
        json: dict[str, Any] | None = None,
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
        capabilities: list[str] | None = None,
        max_agents: int = 100,
        labels: dict[str, str] | None = None,
    ) -> dict[str, Any]:
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

    def list_worlds(self) -> list[dict[str, Any]]:
        result = self._safe_request("GET", "/api/v1/federation/worlds", default=[])
        return result if isinstance(result, list) else []

    def get_world(self, world_id: str) -> dict[str, Any]:
        return self._safe_request("GET", f"/api/v1/federation/worlds/{world_id}")

    def deregister_world(self, world_id: str) -> dict[str, Any]:
        return self._safe_request("DELETE", f"/api/v1/federation/worlds/{world_id}")

    def heartbeat(self, world_id: str, metrics: dict[str, Any] | None = None) -> dict[str, Any]:
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
        skills: dict[str, int] | None = None,
        public_key: str = "",
    ) -> dict[str, Any]:
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
        rejection_reason: str | None = None,
    ) -> dict[str, Any]:
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

    def execute_migration(self, migration_id: str) -> dict[str, Any]:
        return self._safe_request("POST", f"/api/v1/migration/{migration_id}/execute")

    def cancel_migration(
        self,
        migration_id: str,
        cancelled_by: str,
        reason: str | None = None,
    ) -> dict[str, Any]:
        return self._safe_request(
            "POST",
            f"/api/v1/migration/{migration_id}/cancel",
            json={"cancelled_by": cancelled_by, "reason": reason},
        )

    def get_migration_status(self, migration_id: str) -> dict[str, Any]:
        return self._safe_request("GET", f"/api/v1/migration/{migration_id}")

    def list_migrations(
        self,
        world_id: str | None = None,
        inbound: bool = True,
        status_filter: str | None = None,
        limit: int = 10,
        offset: int = 0,
    ) -> list[dict[str, Any]]:
        body: dict[str, Any] = {
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

    def get_migration_policy(self) -> dict[str, Any]:
        return self._safe_request("GET", "/api/v1/migration/policy")

    def update_migration_policy(self, **kwargs: Any) -> dict[str, Any]:
        return self._safe_request("PUT", "/api/v1/migration/policy", json=kwargs)

    def get_migration_stats(self) -> dict[str, Any]:
        return self._safe_request("GET", "/api/v1/migration/stats")

    def get_agent_immigration_status(self, agent_id: str) -> dict[str, Any]:
        return self._safe_request("GET", f"/api/v1/agents/{agent_id}/immigration-status")


# ── Phase 2: Federation sync hook for the think loop ───────────────────────
#
# FederationClient is an HTTP client, but nothing in the agent runtime
# previously called it automatically — it was only reachable indirectly via
# the LLM-driven `diplomacy` tool.  FederationSync wires the client into the
# think loop so that, once federation is enabled (genesis.yaml
# ``federation.enabled: true``), each agent periodically discovers peer worlds
# through its local World Engine.  When federation is disabled (the Phase 1
# default) the hook is simply not constructed, so behaviour is unchanged.


@dataclass
class FederationSyncConfig:
    """Configuration for periodic federation peer discovery.

    Attributes:
        enabled: When False, sync is a no-op (Phase 1 default).
        world_id: Identifier of the local world.  Used to exclude our own
            world from the discovered peer set.
        bootstrap_peers: Seed peer URLs from genesis.yaml
            ``federation.bootstrap_peers``.  Surfaced for diagnostics; actual
            discovery happens through the World Engine registry the client
            talks to (the engine is seeded with these peers at startup).
        sync_interval_ticks: Run discovery every N think-loop ticks.
            Federation gossip is comparatively slow, so the default
            (50 ticks ~ 50s at 1 tick/s) avoids hammering the registry.
    """

    enabled: bool = False
    world_id: str = ""
    bootstrap_peers: list[str] = field(default_factory=list)
    sync_interval_ticks: int = 50


class FederationSync:
    """Think-loop hook that periodically discovers federation peer worlds.

    Implements the ``FederationHook`` protocol expected by ``ThinkLoop``.
    The blocking ``list_worlds`` HTTP call is offloaded to a thread so it
    never stalls the async event loop.  All failures are swallowed —
    federation must never break the perceive->decide->act cycle.
    """

    def __init__(
        self,
        client: FederationClient,
        config: FederationSyncConfig,
    ) -> None:
        self._client = client
        self._config = config
        self._discovered_peers: list[dict[str, Any]] = []
        self._last_sync_tick: int = -1

    @property
    def config(self) -> FederationSyncConfig:
        return self._config

    @property
    def discovered_peers(self) -> list[dict[str, Any]]:
        """Most recently discovered peer worlds (empty until first sync)."""
        return list(self._discovered_peers)

    @property
    def last_sync_tick(self) -> int:
        """Tick of the last successful peer-discovery run (-1 if never)."""
        return self._last_sync_tick

    async def sync(self, tick: int) -> None:
        """Run peer discovery on the configured interval.

        Skipped (no-op) when disabled or off-interval.  Never raises.
        """
        if not self._config.enabled:
            return
        interval = (
            self._config.sync_interval_ticks
            if self._config.sync_interval_ticks > 0
            else 1
        )
        if tick % interval != 0:
            return

        try:
            # list_worlds() is a blocking httpx call — offload to a thread
            # so we don't stall the think-loop's event loop.
            peers = await asyncio.to_thread(self._client.list_worlds)
        except Exception:
            logger.debug("Federation sync at tick %d failed", tick, exc_info=True)
            return

        own_id = self._config.world_id
        self._discovered_peers = [
            w for w in peers if isinstance(w, dict) and w.get("world_id") != own_id
        ]
        self._last_sync_tick = tick
        logger.info(
            "Federation sync (tick %d): discovered %d peer world(s)",
            tick,
            len(self._discovered_peers),
        )

    def close(self) -> None:
        """Release the underlying HTTP client."""
        try:
            self._client._client.close()  # noqa: SLF001
        except Exception:
            pass


def load_federation_config_from_genesis(path: str | Path) -> FederationSyncConfig:
    """Read the ``federation`` section of a genesis.yaml file.

    This is the bridge that makes ``genesis.yaml``'s
    ``federation.bootstrap_peers`` (and the ``federation.enabled`` flag)
    actually take effect on the agent side: the genesis file is the single
    source of truth for whether federation is on and which seed peers to use.

    A missing file or missing ``federation`` section yields a *disabled*
    config, so callers can point this at any genesis path without guarding.
    Federation is considered enabled when ``federation.enabled`` is true **or**
    ``bootstrap_peers`` is non-empty (peers imply intent to federate).
    """
    p = Path(path)
    if not p.exists():
        return FederationSyncConfig()
    try:
        with open(p) as f:
            data = yaml.safe_load(f)
    except Exception:
        logger.warning(
            "Failed to parse genesis federation config at %s", p, exc_info=True
        )
        return FederationSyncConfig()
    if not isinstance(data, dict):
        return FederationSyncConfig()

    fed = data.get("federation") or {}
    if not isinstance(fed, dict):
        fed = {}
    peers_raw = fed.get("bootstrap_peers") or []
    if not isinstance(peers_raw, list):
        peers_raw = []
    peers = [str(x) for x in peers_raw]
    enabled = bool(fed.get("enabled")) or len(peers) > 0
    return FederationSyncConfig(
        enabled=enabled,
        world_id=str(fed.get("world_id", "")),
        bootstrap_peers=peers,
        sync_interval_ticks=int(fed.get("sync_interval_ticks", 50)),
    )


def build_federation_sync(
    base_url: str,
    genesis_path: str | Path | None = None,
) -> FederationSync | None:
    """Construct a FederationSync hook, or None when federation is disabled.

    Convenience wrapper for ``run_agent``: resolve the genesis path (explicit
    arg -> ``AGENT_WORLD_GENESIS`` env var -> default ``config/genesis.yaml``),
    load the federation config, and build a client + hook.  Returns None when
    federation is disabled so the think loop runs unchanged (Phase 1 default).
    """
    import os

    if genesis_path is None:
        genesis_path = os.environ.get("AGENT_WORLD_GENESIS", "config/genesis.yaml")
    cfg = load_federation_config_from_genesis(genesis_path)
    if not cfg.enabled:
        return None
    client = FederationClient(base_url=base_url)
    return FederationSync(client=client, config=cfg)
