"""Low-level A2A gRPC client with retry logic and bidirectional streaming.

Provides two communication modes:
    - **Synchronous**: ``SendMessage`` (unary) and ``Discover`` (unary).
    - **Streaming**: ``StreamMessages`` (bidirectional) for background
      message receive and send.

Network errors trigger automatic retry with exponential backoff + jitter.
The streaming connection auto-reconnects on disconnect.
"""

from __future__ import annotations

import asyncio
import logging
import random
from typing import TYPE_CHECKING, AsyncIterator

import grpc
from protocol.gen.python import a2a_pb2, a2a_pb2_grpc

from .config import A2AClientConfig
from .message import build_a2a_message

if TYPE_CHECKING:
    from agent_runtime.core.message_queue import MessageQueue

logger = logging.getLogger(__name__)


class A2AClient:
    """Low-level gRPC client for A2A communication.

    Usage::

        config = A2AClientConfig(server_address="localhost:50051", agent_id="alice")
        client = A2AClient(config)
        await client.connect()

        # Synchronous send
        ack = await client.send_message(to_agent="bob", message_type=a2a_pb2.INFORM,
                                         payload={"text": "hello"})

        # Streaming
        await client.start_streaming()
        async for msg in client.incoming_messages():
            ...
        await client.stop_streaming()

        await client.close()

    Or as an async context manager::

        async with A2AClient(config) as client:
            ack = await client.send_message(to_agent="bob", ...)
    """

    def __init__(self, config: A2AClientConfig) -> None:
        self._config = config
        self._channel: grpc.aio.Channel | None = None
        self._stub: a2a_pb2_grpc.A2AServiceStub | None = None
        self._incoming_queue: asyncio.Queue[a2a_pb2.A2AMessage] | None = None
        self._stream_task: asyncio.Task[None] | None = None
        self._send_queue: asyncio.Queue[a2a_pb2.A2AMessage] | None = None
        self._streaming = False
        # World message (Oracle/Bounty) streaming
        self._consume_task: asyncio.Task[None] | None = None
        self._consuming = False
        self._message_queue: MessageQueue | None = None

    def _get_incoming_queue(self) -> asyncio.Queue[a2a_pb2.A2AMessage]:
        if self._incoming_queue is None:
            self._incoming_queue = asyncio.Queue()
        return self._incoming_queue

    def _get_send_queue(self) -> asyncio.Queue[a2a_pb2.A2AMessage]:
        if self._send_queue is None:
            self._send_queue = asyncio.Queue()
        return self._send_queue

    # ------------------------------------------------------------------
    # Connection lifecycle
    # ------------------------------------------------------------------

    async def connect(self) -> None:
        """Open a gRPC channel, verify readiness, and create the service stub.

        Raises:
            ConnectionError: If the channel does not become ready within the
                configured timeout.
        """
        self._channel = grpc.aio.insecure_channel(self._config.server_address)
        self._stub = a2a_pb2_grpc.A2AServiceStub(self._channel)

        # Verify channel connectivity before returning so callers get a fast
        # error instead of waiting for the first RPC to time out.
        # Use the native async channel_ready() coroutine instead of
        # grpc.channel_ready_future() which creates a _ChannelReadyFuture
        # whose __del__ calls subscribe/unsubscribe on grpc.aio.Channel,
        # raising PytestUnraisableExceptionWarning on GC.
        try:
            await asyncio.wait_for(
                self._channel.channel_ready(),  # type: ignore[union-attr]
                timeout=self._config.timeout,
            )
        except asyncio.TimeoutError:
            await self._channel.close()
            self._channel = None
            self._stub = None
            raise ConnectionError(
                f"gRPC channel to {self._config.server_address} did not become "
                f"ready within {self._config.timeout}s"
            ) from None

        logger.info("A2A client connected to %s", self._config.server_address)

    async def close(self) -> None:
        """Stop streaming (if active) and close the gRPC channel."""
        await self.stop_streaming()
        await self.stop_consuming()
        if self._channel is not None:
            await self._channel.close()
            self._channel = None
            self._stub = None
        logger.info("A2A client closed")

    # ------------------------------------------------------------------
    # Context manager support
    # ------------------------------------------------------------------

    async def __aenter__(self) -> "A2AClient":
        """Async context manager entry — open the gRPC channel."""
        await self.connect()
        return self

    async def __aexit__(self, exc_type, exc_val, exc_tb) -> None:  # type: ignore[override]
        """Async context manager exit — close the gRPC channel."""
        await self.close()

    @property
    def connected(self) -> bool:
        """Return True if the gRPC channel is open."""
        return self._channel is not None

    # ------------------------------------------------------------------
    # Synchronous RPCs (with retry)
    # ------------------------------------------------------------------

    async def send_message(
        self,
        *,
        to_agent: str = "",
        message_type: int = a2a_pb2.INFORM,
        payload: dict | None = None,
    ) -> a2a_pb2.MessageAck:
        """Send a message via the unary SendMessage RPC (with retry).

        Args:
            to_agent: Recipient agent ID (empty = broadcast).
            message_type: MessageType enum value.
            payload: Dict payload to JSON-encode.

        Returns:
            MessageAck from the server.
        """
        msg = build_a2a_message(
            from_agent=self._config.agent_id,
            to_agent=to_agent,
            message_type=message_type,
            payload=payload,
        )
        return await self._retry_rpc(
            lambda: self._stub.SendMessage(  # type: ignore[union-attr]
                msg, timeout=self._config.timeout
            )
        )

    async def discover(
        self,
        capabilities: list[str] | None = None,
    ) -> a2a_pb2.DiscoverResponse:
        """Discover other agents via the Discover RPC (with retry).

        Args:
            capabilities: Optional capability filter list.

        Returns:
            DiscoverResponse containing found AgentInfo entries.
        """
        request = a2a_pb2.DiscoverRequest(
            agent_id=self._config.agent_id,
            capabilities=capabilities or [],
        )
        return await self._retry_rpc(
            lambda: self._stub.Discover(  # type: ignore[union-attr]
                request, timeout=self._config.timeout
            )
        )

    async def heartbeat(self) -> a2a_pb2.HeartbeatResponse:
        """Send a heartbeat to the server (with retry).

        Returns:
            HeartbeatResponse with server_time for tick synchronization.
        """
        request = a2a_pb2.HeartbeatRequest(
            agent_id=self._config.agent_id,
        )
        return await self._retry_rpc(
            lambda: self._stub.Heartbeat(  # type: ignore[union-attr]
                request, timeout=self._config.timeout
            )
        )

    # ------------------------------------------------------------------
    # Bidirectional streaming
    # ------------------------------------------------------------------

    async def start_streaming(self) -> None:
        """Start the bidirectional StreamMessages RPC in the background.

        Incoming messages are placed into an internal queue that can be
        read via ``incoming_messages()``.  If streaming is already active
        this is a no-op.
        """
        if self._streaming:
            return
        self._streaming = True
        self._stream_task = asyncio.create_task(self._stream_loop())

    async def stop_streaming(self) -> None:
        """Stop the background streaming task."""
        self._streaming = False
        if self._stream_task is not None:
            self._stream_task.cancel()
            try:
                await self._stream_task
            except asyncio.CancelledError:
                pass
            self._stream_task = None

    async def incoming_messages(self) -> AsyncIterator[a2a_pb2.A2AMessage]:
        """Yield incoming messages from the streaming queue."""
        while self._streaming or not self._get_incoming_queue().empty():
            try:
                msg = await asyncio.wait_for(
                    self._get_incoming_queue().get(), timeout=1.0
                )
                yield msg
            except asyncio.TimeoutError:
                continue

    def drain_incoming(self) -> list[a2a_pb2.A2AMessage]:
        """Drain and return all currently queued incoming messages.

        Returns a list of all messages that have been received via the
        streaming connection since the last drain.  Returns an empty list
        if no messages are available or streaming is not active.
        """
        messages: list[a2a_pb2.A2AMessage] = []
        while True:
            try:
                messages.append(self._get_incoming_queue().get_nowait())
            except asyncio.QueueEmpty:
                break
        return messages

    async def stream_send(self, msg: a2a_pb2.A2AMessage) -> None:
        """Queue a message for sending on the streaming connection.

        Raises:
            RuntimeError: If streaming is not active.
        """
        if not self._streaming:
            raise RuntimeError("Streaming is not active — call start_streaming() first")
        await self._get_send_queue().put(msg)

    @property
    def streaming(self) -> bool:
        """Return True if the bidirectional stream is active."""
        return self._streaming

    # ------------------------------------------------------------------
    # World message streaming (Oracle/Bounty)
    # ------------------------------------------------------------------

    async def start_consuming(self, message_queue: MessageQueue) -> None:
        """Start the ConsumeMessages server-streaming RPC in the background.

        Incoming WorldMessage items (Oracle, Bounty) are converted and
        pushed into the provided ``MessageQueue`` for the ThinkLoop
        perceive phase to read.

        If already consuming, this is a no-op.

        Args:
            message_queue: The MessageQueue to push world messages into.
        """
        if self._consuming:
            return
        self._message_queue = message_queue
        self._consuming = True
        self._consume_task = asyncio.create_task(self._consume_loop())

    async def stop_consuming(self) -> None:
        """Stop the background ConsumeMessages streaming task."""
        self._consuming = False
        if self._consume_task is not None:
            self._consume_task.cancel()
            try:
                await self._consume_task
            except asyncio.CancelledError:
                pass
            self._consume_task = None

    @property
    def consuming(self) -> bool:
        """Return True if the ConsumeMessages stream is active."""
        return self._consuming

    @property
    def message_queue(self) -> MessageQueue | None:
        """Return the MessageQueue if consuming is active."""
        return self._message_queue

    # ------------------------------------------------------------------
    # Internal streaming loop
    # ------------------------------------------------------------------

    async def _stream_loop(self) -> None:
        """Background task that maintains the bidirectional stream."""
        while self._streaming:
            try:
                await self._run_stream()
            except Exception:
                logger.exception("Stream error, reconnecting in %.1fs",
                                 self._config.stream_reconnect_delay)
                await asyncio.sleep(self._config.stream_reconnect_delay)

    async def _run_stream(self) -> None:
        """One iteration of the bidirectional streaming connection."""
        if self._stub is None:
            raise RuntimeError("Cannot stream — call connect() first")

        async def request_iter() -> AsyncIterator[a2a_pb2.A2AMessage]:
            while self._streaming:
                try:
                    msg = await asyncio.wait_for(
                        self._get_send_queue().get(), timeout=1.0
                    )
                    yield msg
                except asyncio.TimeoutError:
                    continue

        call = self._stub.StreamMessages(request_iter())
        async for response in call:
            if not self._streaming:
                break
            await self._get_incoming_queue().put(response)

    async def _consume_loop(self) -> None:
        """Background task that maintains the ConsumeMessages stream."""
        while self._consuming:
            try:
                await self._run_consume()
            except Exception:
                logger.exception(
                    "ConsumeMessages stream error, reconnecting in %.1fs",
                    self._config.stream_reconnect_delay,
                )
                await asyncio.sleep(self._config.stream_reconnect_delay)

    async def _run_consume(self) -> None:
        """One iteration of the ConsumeMessages streaming connection."""
        if self._stub is None:
            raise RuntimeError("Cannot consume — call connect() first")
        if self._message_queue is None:
            raise RuntimeError("No MessageQueue set — call start_consuming() first")

        request = a2a_pb2.ConsumeMessagesRequest(
            agent_id=self._config.agent_id,
        )

        call = self._stub.ConsumeMessages(request)
        async for world_msg in call:
            if not self._consuming:
                break
            self._message_queue.enqueue_world_message(world_msg)

    # ------------------------------------------------------------------
    # Retry logic
    # ------------------------------------------------------------------

    async def _retry_rpc(self, rpc_fn):
        """Execute an RPC call with exponential backoff retry.

        Retries on transient errors (UNAVAILABLE, DEADLINE_EXCEEDED,
        RESOURCE_EXHAUSTED) up to max_retries times.
        """
        policy = self._config.retry_policy
        last_exc: Exception | None = None

        for attempt in range(policy.max_retries + 1):
            try:
                return await rpc_fn()
            except grpc.aio.AioRpcError as exc:
                last_exc = exc
                code_name = exc.code().name if exc.code() else "UNKNOWN"
                if code_name not in policy.retryable_codes:
                    raise
                if attempt >= policy.max_retries:
                    raise
                delay = self._compute_backoff(attempt)
                logger.warning(
                    "RPC retry %d/%d (code=%s, delay=%.2fs)",
                    attempt + 1,
                    policy.max_retries,
                    code_name,
                    delay,
                )
                await asyncio.sleep(delay)

        # Should not reach here, but just in case
        raise last_exc  # type: ignore[misc]

    def _compute_backoff(self, attempt: int) -> float:
        """Compute exponential backoff with jitter for retry attempt."""
        policy = self._config.retry_policy
        delay = min(
            policy.base_delay * (2 ** attempt),
            policy.max_delay,
        )
        jitter_amount = delay * policy.jitter * random.random()
        return min(delay + jitter_amount, policy.max_delay)
