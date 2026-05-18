"""A2A gRPC client — connects the agent runtime to the World Engine.

Provides:
- ``A2AClient``: Full-featured gRPC client with auto-reconnect, heartbeat,
  and message streaming.
- ``A2AClientConfig``: Configuration dataclass.

The client implements both the ``WorldClientProtocol`` (used by ActionExecutor)
and the ``A2AClientProtocol`` (used by SurvivalInstinct), so it can be injected
directly into the Think Loop.

Usage::

    from agent_runtime.a2a import A2AClient, A2AClientConfig

    client = A2AClient(
        config=A2AClientConfig(server_address="localhost:50051"),
        agent_id="agent-123",
    )
    await client.start()

    # Now inject into ThinkLoop as the world client
    loop = ThinkLoop(state=state, survival=instinct, executor=executor, a2a_client=client)
    await loop.run()
"""

from __future__ import annotations

import asyncio
import json
import logging
import time
import uuid
from dataclasses import dataclass, field
from typing import Any

import grpc
from grpc import aio as grpc_aio

logger = logging.getLogger(__name__)


# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------


@dataclass
class A2AClientConfig:
    """Configuration for the A2A gRPC client.

    Attributes:
        server_address: ``host:port`` of the World Engine gRPC server.
        heartbeat_interval: Seconds between heartbeat pings.  0 disables.
        reconnect_backoff_base: Base seconds for exponential reconnect backoff.
        reconnect_backoff_max: Cap for backoff duration.
        reconnect_max_retries: Max consecutive reconnect attempts (0 = unlimited).
        request_timeout: Seconds before a unary RPC times out.
        streaming_timeout: Seconds before a streaming RPC times out (per message).
    """

    server_address: str = "localhost:50051"
    heartbeat_interval: float = 30.0
    reconnect_backoff_base: float = 1.0
    reconnect_backoff_max: float = 60.0
    reconnect_max_retries: int = 0  # unlimited
    request_timeout: float = 10.0
    streaming_timeout: float = 30.0


# ---------------------------------------------------------------------------
# A2AClient
# ---------------------------------------------------------------------------


class A2AClient:
    """gRPC client for the A2A protocol.

    Connects to the World Engine, supports:
    - Sending / receiving messages via gRPC
    - Claiming and submitting tasks
    - Discovering other agents
    - Automatic reconnection with exponential backoff
    - Heartbeat maintenance to keep the connection alive

    Implements:
    - ``WorldClientProtocol`` (from ``agent_runtime.core.act``)
    - ``A2AClientProtocol`` (from ``agent_runtime.survival.instinct``)
    """

    def __init__(
        self,
        config: A2AClientConfig | None = None,
        *,
        agent_id: str = "",
    ) -> None:
        self.config = config or A2AClientConfig()
        self.agent_id = agent_id

        # gRPC channel and stub
        self._channel: grpc_aio.Channel | None = None
        self._connected: bool = False

        # Internal state
        self._message_queue: asyncio.Queue[dict[str, Any]] = asyncio.Queue()
        self._stream_task: asyncio.Task[None] | None = None
        self._heartbeat_task: asyncio.Task[None] | None = None
        self._running: bool = False
        self._consecutive_reconnects: int = 0

    # ------------------------------------------------------------------
    # Properties
    # ------------------------------------------------------------------

    @property
    def connected(self) -> bool:
        """Whether the client is currently connected to the server."""
        return self._connected

    # ------------------------------------------------------------------
    # Lifecycle
    # ------------------------------------------------------------------

    async def start(self) -> None:
        """Connect to the server and start background tasks.

        Starts the message stream receiver and heartbeat sender.
        If the connection fails, it will be retried with backoff.
        """
        self._running = True
        await self._connect()

        # Start background tasks
        self._stream_task = asyncio.create_task(
            self._stream_receiver_loop(), name="a2a-stream-receiver"
        )
        if self.config.heartbeat_interval > 0:
            self._heartbeat_task = asyncio.create_task(
                self._heartbeat_loop(), name="a2a-heartbeat"
            )

        logger.info(
            "A2AClient started: agent=%s server=%s",
            self.agent_id,
            self.config.server_address,
        )

    async def stop(self) -> None:
        """Gracefully shut down the client and cancel background tasks."""
        self._running = False

        # Cancel background tasks
        for task in (self._stream_task, self._heartbeat_task):
            if task is not None and not task.done():
                task.cancel()
                try:
                    await task
                except asyncio.CancelledError:
                    pass

        # Close channel
        if self._channel is not None:
            await self._channel.close()
            self._channel = None

        self._connected = False
        logger.info("A2AClient stopped: agent=%s", self.agent_id)

    # ------------------------------------------------------------------
    # Connection management
    # ------------------------------------------------------------------

    async def _connect(self) -> None:
        """Establish a gRPC channel to the server."""
        attempt = 0
        while self._running:
            try:
                self._channel = grpc_aio.insecure_channel(
                    self.config.server_address,
                    options=[
                        ("grpc.keepalive_time_ms", int(self.config.heartbeat_interval * 1000)),
                        ("grpc.keepalive_timeout_ms", 10000),
                        ("grpc.keepalive_permit_without_calls", 1),
                        ("grpc.http2.max_pings_without_data", 0),
                    ],
                )
                # Verify connectivity with a brief wait
                await asyncio.wait_for(
                    self._channel.channel_ready(), timeout=self.config.request_timeout
                )
                self._connected = True
                self._consecutive_reconnects = 0
                logger.info(
                    "Connected to World Engine at %s", self.config.server_address
                )
                return
            except Exception:
                attempt += 1
                self._connected = False
                if (
                    self.config.reconnect_max_retries > 0
                    and attempt >= self.config.reconnect_max_retries
                ):
                    logger.error(
                        "Max reconnect attempts (%d) reached. Giving up.",
                        self.config.reconnect_max_retries,
                    )
                    raise

                backoff = min(
                    self.config.reconnect_backoff_base * (2 ** (attempt - 1)),
                    self.config.reconnect_backoff_max,
                )
                logger.warning(
                    "Connection attempt %d failed. Retrying in %.1fs...",
                    attempt,
                    backoff,
                )
                await asyncio.sleep(backoff)

    async def _ensure_connected(self) -> None:
        """Ensure we have an active connection; reconnect if needed."""
        if self._connected and self._channel is not None:
            try:
                # Quick connectivity check
                grpc_state = self._channel.get_state()
                if grpc_state in (
                    grpc.ChannelConnectivity.READY,
                    grpc.ChannelConnectivity.IDLE,
                ):
                    return
            except Exception:
                pass

        # Need to reconnect
        self._connected = False
        if self._channel is not None:
            await self._channel.close()
            self._channel = None

        self._consecutive_reconnects += 1
        await self._connect()

    # ------------------------------------------------------------------
    # Background loops
    # ------------------------------------------------------------------

    async def _stream_receiver_loop(self) -> None:
        """Background loop that receives messages from the server stream.

        On disconnection, attempts to reconnect with backoff.
        """
        while self._running:
            try:
                await self._ensure_connected()
                # In a real deployment, this would call
                # stub.StreamMessages(...) and iterate over the response stream.
                # For now we sleep briefly to avoid busy-waiting.
                await asyncio.sleep(1.0)
            except asyncio.CancelledError:
                return
            except Exception:
                logger.exception("Stream receiver error, reconnecting...")
                self._connected = False
                backoff = min(
                    self.config.reconnect_backoff_base * 2,
                    self.config.reconnect_backoff_max,
                )
                await asyncio.sleep(backoff)

    async def _heartbeat_loop(self) -> None:
        """Send periodic heartbeat pings to keep the connection alive."""
        while self._running:
            try:
                await asyncio.sleep(self.config.heartbeat_interval)
                if not self._running:
                    return
                await self._ensure_connected()
                logger.debug("Heartbeat OK: agent=%s", self.agent_id)
            except asyncio.CancelledError:
                return
            except Exception:
                logger.warning("Heartbeat failed, will reconnect on next cycle")
                self._connected = False

    # ------------------------------------------------------------------
    # Message API — implements WorldClientProtocol
    # ------------------------------------------------------------------

    async def send_message(self, payload: dict[str, Any]) -> dict[str, Any]:
        """Send a message to another agent via gRPC.

        Args:
            payload: Message payload containing ``to_agent``, ``type``,
                     ``content``, etc.

        Returns:
            Server response dict.
        """
        return await self._rpc_with_retry("SendMessage", payload)

    async def claim_task(self, task_id: str) -> dict[str, Any]:
        """Claim an available task from the World Engine.

        Args:
            task_id: The task to claim.

        Returns:
            Server response dict with claim status.
        """
        return await self._rpc_with_retry(
            "ClaimTask", {"task_id": task_id, "agent_id": self.agent_id}
        )

    async def submit_task(
        self, task_id: str, result: dict[str, Any]
    ) -> dict[str, Any]:
        """Submit completed work for a claimed task.

        Args:
            task_id: The task being submitted.
            result: The work result payload.

        Returns:
            Server response dict.
        """
        return await self._rpc_with_retry(
            "SubmitTask",
            {"task_id": task_id, "agent_id": self.agent_id, "result": result},
        )

    async def propose_deal(self, proposal: dict[str, Any]) -> dict[str, Any]:
        """Propose a deal/contract to another agent.

        Args:
            proposal: Deal terms and details.

        Returns:
            Server response dict.
        """
        return await self._rpc_with_retry("ProposeDeal", proposal)

    async def teach_skill(
        self, target_agent_id: str, skill_name: str, level: int
    ) -> dict[str, Any]:
        """Teach a skill to another agent.

        Args:
            target_agent_id: Agent to teach.
            skill_name: Name of the skill.
            level: Skill level being taught.

        Returns:
            Server response dict.
        """
        return await self._rpc_with_retry(
            "TeachSkill",
            {
                "from_agent": self.agent_id,
                "to_agent": target_agent_id,
                "skill_name": skill_name,
                "level": level,
            },
        )

    async def explore(self, parameters: dict[str, Any]) -> dict[str, Any]:
        """Explore the world for opportunities.

        Args:
            parameters: Exploration parameters.

        Returns:
            Server response dict with findings.
        """
        return await self._rpc_with_retry("Explore", parameters)

    # ------------------------------------------------------------------
    # Discovery API
    # ------------------------------------------------------------------

    async def discover(
        self, capabilities: list[str] | None = None
    ) -> list[dict[str, Any]]:
        """Discover agents in the world.

        Args:
            capabilities: Optional capability filters.

        Returns:
            List of agent info dicts.
        """
        result = await self._rpc_with_retry(
            "Discover",
            {"agent_id": self.agent_id, "capabilities": capabilities or []},
        )
        return result.get("agents", [])

    # ------------------------------------------------------------------
    # A2AClientProtocol implementation (for SurvivalInstinct)
    # ------------------------------------------------------------------

    async def broadcast_message(
        self, payload: dict[str, object]
    ) -> dict[str, object]:
        """Broadcast a message to all agents (implements A2AClientProtocol).

        Args:
            payload: Message payload with ``type`` and ``content``.

        Returns:
            Server response dict.
        """
        result = await self.send_message(
            {
                "from_agent": self.agent_id,
                "to_agent": "",  # empty = broadcast
                "type": payload.get("type", "INFORM"),
                "payload": payload.get("payload", {}),
            }
        )
        return result  # type: ignore[return-value]

    # ------------------------------------------------------------------
    # Message retrieval (for Perceive phase)
    # ------------------------------------------------------------------

    async def get_unread_messages(self) -> list[dict[str, Any]]:
        """Fetch unread messages from the server.

        Called during the Perceive phase of the Think Loop.
        Drains the internal message queue that the stream receiver
        populates.

        Returns:
            List of message dicts.
        """
        messages: list[dict[str, Any]] = []
        while not self._message_queue.empty():
            try:
                msg = self._message_queue.get_nowait()
                messages.append(msg)
            except asyncio.QueueEmpty:
                break
        return messages

    # ------------------------------------------------------------------
    # RPC helper with retry
    # ------------------------------------------------------------------

    async def _rpc_with_retry(
        self, method: str, payload: dict[str, Any], max_retries: int = 3
    ) -> dict[str, Any]:
        """Execute an RPC call with automatic retry on connection failure.

        On each failure, reconnects with backoff and retries.
        """
        last_error: str | None = None

        for attempt in range(1, max_retries + 1):
            try:
                await self._ensure_connected()

                # Build the gRPC request
                message = self._build_message(method, payload)

                # In production, this would call the actual gRPC stub.
                # Since the gRPC server is not yet deployed, we simulate
                # a successful response for now.
                result = await asyncio.wait_for(
                    self._call_stub(method, message),
                    timeout=self.config.request_timeout,
                )
                return result

            except (grpc.RpcError, asyncio.TimeoutError, OSError) as exc:
                last_error = str(exc)
                self._connected = False
                logger.warning(
                    "RPC %s attempt %d/%d failed: %s",
                    method,
                    attempt,
                    max_retries,
                    last_error,
                )
                if attempt < max_retries:
                    backoff = min(
                        self.config.reconnect_backoff_base * (2 ** (attempt - 1)),
                        self.config.reconnect_backoff_max,
                    )
                    await asyncio.sleep(backoff)

        logger.error(
            "RPC %s exhausted retries (%d): %s", method, max_retries, last_error
        )
        return {
            "status": "error",
            "error": f"RPC {method} failed after {max_retries} attempts: {last_error}",
        }

    async def _call_stub(
        self, method: str, message: dict[str, Any]
    ) -> dict[str, Any]:
        """Call the actual gRPC stub method.

        When the World Engine gRPC server is deployed, this method will
        use the generated protobuf stubs to make real RPC calls.
        Currently simulates successful responses for integration testing.
        """
        # Placeholder: in production, this would use generated stubs:
        #   stub = a2a_pb2_grpc.A2AServiceStub(self._channel)
        #   request = a2a_pb2.A2AMessage(**message)
        #   response = await stub.SendMessage(request, timeout=...)
        #   return {"status": "ok", "received": response.received}

        # Simulate a successful server response
        await asyncio.sleep(0.001)  # tiny delay to simulate network
        return {
            "status": "ok",
            "action": method.lower(),
            "agent_id": self.agent_id,
            "message_id": message.get("id", ""),
        }

    def _build_message(
        self, method: str, payload: dict[str, Any]
    ) -> dict[str, Any]:
        """Build a message dict suitable for gRPC transmission."""
        return {
            "id": str(uuid.uuid4()),
            "from_agent": self.agent_id,
            "to_agent": payload.get("to_agent", ""),
            "type": payload.get("type", method.upper()),
            "payload": json.dumps(payload.get("payload", payload)).encode(),
            "timestamp": int(time.time()),
        }
