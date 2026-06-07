"""Shared base for tools that call the World Engine REST API.

All world-engine tools accept a ``base_url`` pointing at the world-engine
instance and use ``httpx`` for HTTP communication.  In sandbox mode (default)
tools return simulated responses without making network calls.
"""

from __future__ import annotations

import logging
import time
from typing import Any, Dict, Optional

import httpx

from ..base import Tool, ToolParameters, ToolResult, ToolStatus

logger = logging.getLogger(__name__)

API_PREFIX = "/api/v1"


class WorldEngineTool(Tool):
    """Abstract base for world-engine API tools.

    Subclasses inherit:
    - ``base_url`` / ``sandbox`` configuration
    - HTTP helpers (``_get``, ``_post``, ``_delete``, ``_put``)
    - Sandbox response generation via ``_sandbox_response``

    Subclasses must still implement:
    - ``name``, ``description``, ``category``, ``parameters_schema``
    - ``execute`` or ``execute_async``
    """

    def __init__(
        self,
        *,
        base_url: str = "http://localhost:3000",
        sandbox: bool = True,
        timeout_seconds: float = 10.0,
    ) -> None:
        super().__init__()
        self._base_url = base_url.rstrip("/")
        self._sandbox = sandbox
        self._timeout_seconds = timeout_seconds
        self._client: Optional[httpx.Client] = None
        self._async_client: Optional[httpx.AsyncClient] = None

    # ------------------------------------------------------------------
    # HTTP helpers
    # ------------------------------------------------------------------

    def _get_client(self) -> httpx.Client:
        if self._client is None:
            self._client = httpx.Client(
                base_url=self._base_url,
                timeout=self._timeout_seconds,
            )
        return self._client

    def _get_async_client(self) -> httpx.AsyncClient:
        if self._async_client is None:
            self._async_client = httpx.AsyncClient(
                base_url=self._base_url,
                timeout=self._timeout_seconds,
            )
        return self._async_client

    async def _get(self, path: str, *, params: Optional[Dict[str, Any]] = None) -> Dict[str, Any]:
        client = self._get_async_client()
        resp = await client.get(f"{API_PREFIX}{path}", params=params)
        resp.raise_for_status()
        return resp.json()

    async def _post(self, path: str, *, json: Optional[Dict[str, Any]] = None) -> Dict[str, Any]:
        client = self._get_async_client()
        resp = await client.post(f"{API_PREFIX}{path}", json=json)
        resp.raise_for_status()
        return resp.json()

    async def _put(self, path: str, *, json: Optional[Dict[str, Any]] = None) -> Dict[str, Any]:
        client = self._get_async_client()
        resp = await client.put(f"{API_PREFIX}{path}", json=json)
        resp.raise_for_status()
        return resp.json()

    async def _delete(self, path: str) -> Dict[str, Any]:
        client = self._get_async_client()
        resp = await client.delete(f"{API_PREFIX}{path}")
        resp.raise_for_status()
        return resp.json()

    # ------------------------------------------------------------------
    # Sandbox helpers
    # ------------------------------------------------------------------

    def _sandbox_response(
        self,
        action: str,
        params: Dict[str, Any],
        output: Optional[Dict[str, Any]] = None,
    ) -> ToolResult:
        """Generate a simulated response in sandbox mode."""
        return ToolResult(
            tool_name=self.name,
            status=ToolStatus.SUCCESS,
            output=output or {
                "sandbox": True,
                "action": action,
                "message": f"Simulated {action} response",
                "params": params,
            },
            metadata={"sandbox": True},
        )

    def _make_error_result(self, error: str) -> ToolResult:
        return ToolResult(
            tool_name=self.name,
            status=ToolStatus.ERROR,
            error=error,
        )

    @property
    def _valid_actions(self) -> set[str]:
        """Return the set of valid action names. Subclasses should override."""
        return set()

    def _validate_action(self, action: str) -> ToolResult | None:
        """Check if action is valid. Returns error ToolResult if invalid, None if valid."""
        valid = self._valid_actions
        if valid and action not in valid:
            return self._make_error_result(
                f"Unknown {self.name} action: {action}"
            )
        return None

    async def execute_async(self, params: ToolParameters) -> ToolResult:
        """Default async execution — delegates to sync execute()."""
        return await super().execute_async(params)
