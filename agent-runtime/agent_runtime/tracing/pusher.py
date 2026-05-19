"""TracePusher — pushes TickSnapshot data to World Engine via HTTP.

After each tick, the collector saves to local SQLite.  The pusher
additionally POSTs the trace data to the World Engine's trace API
so the Dashboard can query it.
"""

from __future__ import annotations

import logging
from typing import Any
from uuid import UUID

import httpx

from agent_runtime.tracing.models import TickSnapshot

logger = logging.getLogger(__name__)


class TracePusher:
    """Pushes tick traces to the World Engine via HTTP.

    Args:
        world_engine_url: Base URL of the World Engine (e.g., "http://127.0.0.1:3000").
        enabled: Whether pushing is active.
    """

    def __init__(
        self,
        world_engine_url: str = "http://127.0.0.1:3000",
        *,
        enabled: bool = True,
    ) -> None:
        self._url = world_engine_url.rstrip("/")
        self.enabled = enabled
        self._client: httpx.AsyncClient | None = None

    async def _get_client(self) -> httpx.AsyncClient:
        if self._client is None or self._client.is_closed:
            self._client = httpx.AsyncClient()
        return self._client

    async def push(self, snapshot: TickSnapshot) -> bool:
        """Push a TickSnapshot to the World Engine.

        Returns True if successful, False otherwise.
        Errors are logged but not raised — push failures should not
        block the agent's think loop.
        """
        if not self.enabled:
            return False

        try:
            client = await self._get_client()
            url = f"{self._url}/api/v1/agents/{snapshot.agent_id}/traces"
            payload = snapshot.to_dict()
            resp = await client.post(url, json=payload)
            if resp.status_code == 200:
                return True
            else:
                logger.warning(
                    "Trace push failed: status=%d body=%s",
                    resp.status_code,
                    resp.text[:200],
                )
                return False
        except Exception:
            logger.debug("Trace push error (world engine unavailable?)", exc_info=True)
            return False

    async def close(self) -> None:
        """Close the HTTP client."""
        if self._client and not self._client.is_closed:
            await self._client.aclose()
            self._client = None
