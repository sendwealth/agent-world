"""Built-in tool: HTTP Request.

Allows agents to make HTTP requests to external APIs. Supports the common
HTTP methods, configurable headers, and timeout.

This is a sandboxed simulation for the agent-world environment — it does
NOT make real network requests by default. Set ``sandbox=False`` in the
tool constructor to enable real HTTP calls.
"""

from __future__ import annotations

import json
import logging
import time
from typing import Any, Dict, Optional

import httpx

from ..base import Tool, ToolParameters, ToolResult, ToolStatus

logger = logging.getLogger(__name__)


# ---------------------------------------------------------------------------
# Parameter schema
# ---------------------------------------------------------------------------


class HttpRequestParams(ToolParameters):
    """Parameters for the HTTP request tool."""

    url: str
    method: str = "GET"
    headers: Dict[str, str] = {}
    body: Optional[str] = None
    timeout_seconds: float = 10.0
    follow_redirects: bool = True


# ---------------------------------------------------------------------------
# Tool implementation
# ---------------------------------------------------------------------------


class HttpRequestTool(Tool):
    """Make HTTP requests to external services.

    Supports GET, POST, PUT, PATCH, DELETE methods with configurable
    headers, body, and timeout. Returns a structured response with
    status code, headers, and parsed body.

    In sandbox mode (default), returns simulated responses.
    """

    def __init__(self, *, sandbox: bool = True) -> None:
        super().__init__()
        self._sandbox = sandbox

    @property
    def name(self) -> str:
        return "http_request"

    @property
    def description(self) -> str:
        return "Make HTTP requests to external APIs (GET, POST, PUT, DELETE)"

    @property
    def category(self) -> str:
        return "network"

    @property
    def timeout(self) -> float:
        return 15.0

    @property
    def parameters_schema(self) -> type[ToolParameters]:
        return HttpRequestParams

    async def execute_async(self, params: ToolParameters) -> ToolResult:
        assert isinstance(params, HttpRequestParams)

        method = params.method.upper()
        valid_methods = {"GET", "POST", "PUT", "PATCH", "DELETE", "HEAD"}
        if method not in valid_methods:
            return ToolResult(
                tool_name=self.name,
                status=ToolStatus.ERROR,
                error=f"Invalid HTTP method: {method}",
            )

        if self._sandbox:
            return self._sandbox_response(params)

        # Real HTTP request
        start = time.monotonic()
        try:
            async with httpx.AsyncClient(
                timeout=params.timeout_seconds,
                follow_redirects=params.follow_redirects,
            ) as client:
                request_fn = {
                    "GET": client.get,
                    "POST": client.post,
                    "PUT": client.put,
                    "PATCH": client.patch,
                    "DELETE": client.delete,
                    "HEAD": client.head,
                }[method]

                kwargs: Dict[str, Any] = {"headers": params.headers}
                if params.body and method in {"POST", "PUT", "PATCH"}:
                    kwargs["content"] = params.body

                response = await request_fn(params.url, **kwargs)  # type: ignore[operator]
                elapsed = (time.monotonic() - start) * 1000

                # Try to parse JSON, fall back to text
                try:
                    body = response.json()
                except (json.JSONDecodeError, ValueError):
                    body = response.text

                return ToolResult(
                    tool_name=self.name,
                    status=ToolStatus.SUCCESS if response.is_success else ToolStatus.ERROR,
                    output={
                        "status_code": response.status_code,
                        "headers": dict(response.headers),
                        "body": body,
                    },
                    metadata={"latency_ms": round(elapsed, 1)},
                )
        except httpx.TimeoutException:
            return ToolResult(
                tool_name=self.name,
                status=ToolStatus.TIMEOUT,
                error=f"Request timed out after {params.timeout_seconds}s",
            )
        except httpx.RequestError as exc:
            return ToolResult(
                tool_name=self.name,
                status=ToolStatus.ERROR,
                error=f"Request failed: {exc}",
            )

    def _sandbox_response(self, params: HttpRequestParams) -> ToolResult:
        """Generate a simulated response in sandbox mode."""
        return ToolResult(
            tool_name=self.name,
            status=ToolStatus.SUCCESS,
            output={
                "status_code": 200,
                "headers": {"content-type": "application/json"},
                "body": {
                    "sandbox": True,
                    "message": f"Simulated {params.method} response for {params.url}",
                },
            },
            metadata={"sandbox": True},
        )
