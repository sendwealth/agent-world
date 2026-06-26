"""Health check HTTP server.

Provides ``HealthCheckServer`` — a lightweight HTTP server exposing
``GET /health`` and ``POST /api/v1/runtime/swap-model``.
"""

from __future__ import annotations

import asyncio
import json
import logging
import time
from typing import Any

logger = logging.getLogger(__name__)


class HealthCheckServer:
    """Lightweight HTTP health check server using asyncio.

    Exposes:

    - ``GET /health`` — JSON with agent status.
    - ``POST /api/v1/runtime/swap-model`` — Hot-swap the agent's LLM model at runtime.

    Runs alongside the ThinkLoop.
    """

    def __init__(
        self,
        agent_name: str,
        think_loop: Any,
        port: int = 9090,
    ) -> None:
        self._agent_name = agent_name
        self._think_loop = think_loop
        self._port = port
        self._start_time = time.monotonic()
        self._server: asyncio.Server | None = None

    async def start(self) -> None:
        """Start the health check HTTP server."""
        try:
            self._server = await asyncio.start_server(
                self._handle_request,
                host="0.0.0.0",
                port=self._port,
            )
        except OSError:
            logger.warning(
                "Health check server: port %d unavailable, skipping",
                self._port,
            )
            return
        logger.info(
            "Health check server listening on 0.0.0.0:%d",
            self._port,
            extra={"event": "health_server_started", "port": self._port},
        )
        try:
            if self._server is not None:
                await self._server.serve_forever()
        except asyncio.CancelledError:
            pass  # Graceful shutdown via stop()

    async def stop(self) -> None:
        """Stop the health check server."""
        if self._server is not None:
            self._server.close()
            await self._server.wait_closed()
            logger.info("Health check server stopped")

    async def _handle_request(
        self,
        reader: asyncio.StreamReader,
        writer: asyncio.StreamWriter,
    ) -> None:
        """Handle a single HTTP request."""
        try:
            # Read the request line
            request_line = await asyncio.wait_for(reader.readline(), timeout=5.0)
            request_str = request_line.decode("ascii", errors="replace").strip()

            # Read remaining headers
            content_length = 0
            body_buf = b""
            for _ in range(64):
                line = await asyncio.wait_for(reader.readline(), timeout=2.0)
                if line in (b"\r\n", b"\n", b""):
                    break
                line_str = line.decode("ascii", errors="replace").strip().lower()
                if line_str.startswith("content-length:"):
                    try:
                        content_length = int(line_str.split(":", 1)[1].strip())
                    except ValueError:
                        pass
            else:
                writer.write(b"HTTP/1.1 400 Bad Request\r\nConnection: close\r\n\r\n")
                await writer.drain()
                return

            # Read body if Content-Length is set
            if content_length > 0:
                body_buf = await asyncio.wait_for(reader.readexactly(content_length), timeout=5.0)

            # Route the request
            parts = request_str.split()
            method = parts[0] if parts else ""
            path = parts[1].split("?")[0] if len(parts) >= 2 else ""

            if method == "GET" and path == "/health":
                uptime = time.monotonic() - self._start_time
                body = json.dumps(
                    {
                        "status": "running" if self._think_loop.running else "stopped",
                        "agent": self._agent_name,
                        "tick": self._think_loop.tick,
                        "uptime_s": round(uptime, 1),
                    }
                )
                response = (
                    "HTTP/1.1 200 OK\r\n"
                    "Content-Type: application/json\r\n"
                    f"Content-Length: {len(body)}\r\n"
                    "Connection: close\r\n"
                    "\r\n"
                    f"{body}"
                )

            elif method == "POST" and path == "/api/v1/runtime/swap-model":
                response = self._handle_swap_model(body_buf)

            else:
                response = "HTTP/1.1 404 Not Found\r\nConnection: close\r\n\r\n"

            writer.write(response.encode("ascii"))
            await writer.drain()
        except Exception:
            logger.debug("Health check request error", exc_info=True)
        finally:
            writer.close()
            try:
                await writer.wait_closed()
            except Exception:
                pass

    def _handle_swap_model(self, body_buf: bytes) -> str:
        """Handle POST /api/v1/runtime/swap-model."""
        try:
            from agent_runtime.llm.provider_registry import ModelRegistry

            payload = json.loads(body_buf) if body_buf else {}
        except json.JSONDecodeError:
            body = json.dumps({"error": "Invalid JSON body"})
            return (
                "HTTP/1.1 400 Bad Request\r\n"
                "Content-Type: application/json\r\n"
                f"Content-Length: {len(body)}\r\n"
                "Connection: close\r\n"
                "\r\n"
                f"{body}"
            )

        agent_id = payload.get("agent_id")
        provider_id = payload.get("provider_id")
        model = payload.get("model")

        if not agent_id or not provider_id or not model:
            body = json.dumps(
                {
                    "error": "Missing required fields: agent_id, provider_id, model",
                }
            )
            return (
                "HTTP/1.1 400 Bad Request\r\n"
                "Content-Type: application/json\r\n"
                f"Content-Length: {len(body)}\r\n"
                "Connection: close\r\n"
                "\r\n"
                f"{body}"
            )

        reg = ModelRegistry.instance()
        try:
            reg.hot_swap_model(agent_id, provider_id, model)
        except KeyError as exc:
            body = json.dumps({"error": str(exc)})
            return (
                "HTTP/1.1 404 Not Found\r\n"
                "Content-Type: application/json\r\n"
                f"Content-Length: {len(body)}\r\n"
                "Connection: close\r\n"
                "\r\n"
                f"{body}"
            )

        body = json.dumps(
            {
                "status": "ok",
                "agent_id": agent_id,
                "provider_id": provider_id,
                "model": model,
                "tick": self._think_loop.tick,
            }
        )
        return (
            "HTTP/1.1 200 OK\r\n"
            "Content-Type: application/json\r\n"
            f"Content-Length: {len(body)}\r\n"
            "Connection: close\r\n"
            "\r\n"
            f"{body}"
        )
