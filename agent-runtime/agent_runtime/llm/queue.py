"""Async LLM request queue with concurrency control and priority scheduling.

Manages concurrent access to LLM providers so that multiple agents can
share a bounded pool of LLM connections without blocking the world tick.

Key design:
- ``asyncio.PriorityQueue`` orders requests by priority (survival > social > explore > default)
- Background worker drains the queue through a semaphore-bounded provider
- Per-request timeout with fallback on timeout
- Graceful start/stop lifecycle with proper cleanup

Usage::

    from agent_runtime.llm.queue import LLMQueue, QueueConfig, LLMRequest

    queue = LLMQueue(provider=my_llm, config=QueueConfig(max_concurrency=2))
    await queue.start()

    request = LLMRequest(messages=[...], priority="survival")
    response = await queue.enqueue(request)

    await queue.stop()
"""

from __future__ import annotations

import asyncio
import logging
from dataclasses import dataclass, field
from enum import IntEnum
from typing import Any

from agent_runtime.llm.base import LLMMessage, LLMProvider, LLMResponse

logger = logging.getLogger(__name__)


# ---------------------------------------------------------------------------
# Priority levels (higher = scheduled sooner)
# ---------------------------------------------------------------------------


class Priority(IntEnum):
    """Numeric priority for LLM requests."""

    DEFAULT = 0
    EXPLORE = 1
    SOCIAL = 2
    SURVIVAL = 3


# Map string category → Priority
_PRIORITY_MAP: dict[str, Priority] = {
    "survival": Priority.SURVIVAL,
    "social": Priority.SOCIAL,
    "explore": Priority.EXPLORE,
    "default": Priority.DEFAULT,
}


def _resolve_priority(raw: str) -> Priority:
    """Resolve a priority string to a Priority enum value."""
    return _PRIORITY_MAP.get(raw.lower(), Priority.DEFAULT)


# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------


@dataclass
class QueueConfig:
    """Configuration for the LLM request queue.

    Attributes:
        max_concurrency: Maximum number of concurrent LLM requests.
        timeout_seconds: Per-request timeout.  If exceeded, the enqueue
            call raises ``asyncio.TimeoutError`` so that the caller
            (typically DecisionEngine) can apply its own validated fallback.
    """

    max_concurrency: int = 2
    timeout_seconds: float = 120.0


# ---------------------------------------------------------------------------
# Request / Response types
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class LLMRequest:
    """A single LLM request submitted to the queue.

    Attributes:
        messages: Conversation messages to send.
        priority: Scheduling priority (survival > social > explore > default).
        metadata: Optional dict for tracing / logging.
        max_tokens: Override max tokens for this request.
        temperature: Override temperature for this request.
    """

    messages: list[LLMMessage]
    priority: str = "default"
    metadata: dict[str, Any] = field(default_factory=dict)
    max_tokens: int | None = None
    temperature: float | None = None


@dataclass
class QueueStats:
    """Runtime statistics for the LLM queue."""

    total_requests: int = 0
    completed_requests: int = 0
    failed_requests: int = 0
    timed_out_requests: int = 0
    active_requests: int = 0


# ---------------------------------------------------------------------------
# Fallback response
# ---------------------------------------------------------------------------

# JSON schema: {"action": str, "parameters": {}, "reasoning": str, "confidence": int}
# Used only when the queue is stopped with pending requests still enqueued.
# Normal timeouts and errors propagate so DecisionEngine.fallback_decision() can
# pick an action validated against available_actions.
_FALLBACK_RESPONSE = LLMResponse(
    content=(
        '{"action": "rest", "parameters": {},'
        ' "reasoning": "LLM queue shutdown fallback", "confidence": 0}'
    ),
    model="fallback",
)


# ---------------------------------------------------------------------------
# LLMQueue
# ---------------------------------------------------------------------------


class LLMQueue:
    """Async LLM request queue with concurrency control and priority scheduling.

    Wraps an :class:`LLMProvider` and serialises access through an
    ``asyncio.Semaphore`` so that at most ``max_concurrency`` requests
    are in-flight at once.

    A background worker drains a ``PriorityQueue`` in priority order,
    dispatching each request through the semaphore-bounded provider.
    Higher-priority requests (survival > social > explore > default) are
    always processed first.
    """

    def __init__(self, provider: LLMProvider, config: QueueConfig | None = None) -> None:
        self._provider = provider
        self._config = config or QueueConfig()
        self._semaphore = asyncio.Semaphore(self._config.max_concurrency)
        self._queue: asyncio.PriorityQueue[_QueueEntry] = asyncio.PriorityQueue()
        self._stats = QueueStats()
        self._running = False
        self._entry_counter = 0
        self._worker_task: asyncio.Task[None] | None = None
        self._active_count = 0
        self._dispatch_tasks: set[asyncio.Task[None]] = set()

    # ------------------------------------------------------------------
    # Lifecycle
    # ------------------------------------------------------------------

    async def start(self) -> None:
        """Start the queue processor (background worker)."""
        self._running = True
        self._worker_task = asyncio.get_running_loop().create_task(self._worker())
        logger.info(
            "LLMQueue started: max_concurrency=%d timeout=%.1fs",
            self._config.max_concurrency,
            self._config.timeout_seconds,
        )

    async def stop(self) -> None:
        """Stop the queue, cancel the worker, and cancel in-flight requests."""
        self._running = False

        # Cancel the background worker
        if self._worker_task is not None and not self._worker_task.done():
            self._worker_task.cancel()
            try:
                await self._worker_task
            except asyncio.CancelledError:
                pass
            self._worker_task = None

        # Cancel in-flight dispatch tasks
        for t in list(self._dispatch_tasks):
            if not t.done():
                t.cancel()
        # Wait for dispatch tasks to finish
        if self._dispatch_tasks:
            await asyncio.gather(*self._dispatch_tasks, return_exceptions=True)
        self._dispatch_tasks.clear()

        # Drain remaining queue entries and resolve them with fallback
        while not self._queue.empty():
            try:
                entry = self._queue.get_nowait()
                if not entry.future.done():
                    entry.future.set_result(_FALLBACK_RESPONSE)
            except asyncio.QueueEmpty:
                break

        logger.info(
            "LLMQueue stopped: stats=%s",
            self._stats.__dict__,
        )

    # ------------------------------------------------------------------
    # Background worker — drains the priority queue
    # ------------------------------------------------------------------

    async def _worker(self) -> None:
        """Background worker that drains the priority queue.

        Pulls entries in priority order and dispatches them through
        the semaphore-bounded provider.
        """
        while self._running:
            try:
                entry = await asyncio.wait_for(
                    self._queue.get(), timeout=1.0,
                )
            except asyncio.TimeoutError:
                # No items in the queue — loop back and check _running
                continue
            except asyncio.CancelledError:
                break

            if entry.future.done():
                # Already cancelled or resolved — skip
                continue

            # Dispatch the request (does not await the LLM call here —
            # we fire-and-forget so the worker can process the next entry)
            t = asyncio.get_running_loop().create_task(
                self._dispatch_entry(entry),
            )
            self._dispatch_tasks.add(t)
            t.add_done_callback(self._dispatch_tasks.discard)

    async def _dispatch_entry(self, entry: _QueueEntry) -> None:
        """Dispatch a single queue entry through the semaphore to the provider."""
        if entry.future.done():
            return

        try:
            # Wrap the entire semaphore+LLM call with a timeout.
            # Use asyncio.wait_for for Python 3.9 compatibility.
            await asyncio.wait_for(
                self._execute_with_semaphore(entry),
                timeout=self._config.timeout_seconds,
            )
        except asyncio.TimeoutError:
            # Timeout covers both semaphore acquisition and LLM call.
            # Propagate so the caller (DecisionEngine) can use its own
            # fallback_decision() which validates against available_actions.
            self._stats.timed_out_requests += 1
            logger.warning(
                "LLM queue: request timed out (%.1fs) priority=%s",
                self._config.timeout_seconds,
                entry.request.priority,
            )
            if not entry.future.done():
                entry.future.set_exception(asyncio.TimeoutError())
        except asyncio.CancelledError:
            # Queue is shutting down — resolve with fallback
            if not entry.future.done():
                entry.future.set_result(_FALLBACK_RESPONSE)
            raise

    async def _execute_with_semaphore(self, entry: _QueueEntry) -> None:
        """Acquire semaphore and execute the LLM call."""
        async with self._semaphore:
            self._active_count += 1
            self._stats.active_requests = self._active_count
            try:
                response = await self._provider.chat(
                    entry.request.messages,
                    max_tokens=entry.request.max_tokens,
                    temperature=entry.request.temperature,
                )
                self._stats.completed_requests += 1
                if not entry.future.done():
                    entry.future.set_result(response)
            except Exception as exc:
                self._stats.failed_requests += 1
                if not entry.future.done():
                    entry.future.set_exception(exc)
            finally:
                self._active_count -= 1
                self._stats.active_requests = self._active_count

    # ------------------------------------------------------------------
    # Public API
    # ------------------------------------------------------------------

    async def enqueue(self, request: LLMRequest) -> LLMResponse:
        """Submit a request and wait for the result.

        The call blocks (await) until the priority queue dispatches
        the request through the semaphore and the LLM provider returns
        a response (or times out).

        Higher-priority requests are dispatched first.  Within the same
        priority level, requests are processed in FIFO order.

        For non-blocking behaviour, use ``AsyncDecisionProvider`` which
        decouples the tick from this call.
        """
        if not self._running:
            await self.start()

        self._stats.total_requests += 1
        priority = _resolve_priority(request.priority)

        # Create a future for the caller to await
        loop = asyncio.get_running_loop()
        future: asyncio.Future[LLMResponse] = loop.create_future()

        # Build the queue entry with sort key = (-priority, counter)
        # Negate priority so higher values sort first (min-heap)
        entry = _QueueEntry(
            sort_key=(-priority, self._entry_counter),
            request=request,
            future=future,
        )
        self._entry_counter += 1

        await self._queue.put(entry)

        # Wait for the result
        return await future

    async def chat(
        self,
        messages: list[LLMMessage],
        *,
        max_tokens: int | None = None,
        temperature: float | None = None,
    ) -> LLMResponse:
        """LLMProvider-compatible chat interface — enqueues the request.

        This allows ``LLMQueue`` to be passed directly to components that
        expect an ``LLMProvider`` (e.g. ``LLMDecisionProvider``), routing
        their calls through the priority queue and concurrency control.
        """
        request = LLMRequest(
            messages=messages,
            priority="default",
            max_tokens=max_tokens,
            temperature=temperature,
        )
        return await self.enqueue(request)

    def stats(self) -> QueueStats:
        """Return a snapshot of queue statistics."""
        return QueueStats(
            total_requests=self._stats.total_requests,
            completed_requests=self._stats.completed_requests,
            failed_requests=self._stats.failed_requests,
            timed_out_requests=self._stats.timed_out_requests,
            active_requests=self._stats.active_requests,
        )


# ---------------------------------------------------------------------------
# Internal: priority queue entry
# ---------------------------------------------------------------------------


@dataclass(order=True)
class _QueueEntry:
    """Ordered queue entry — sorts by (priority_neg, counter).

    We negate priority so that higher-priority values are dequeued first
    (PriorityQueue is a min-heap).
    """

    sort_key: tuple[int, int] = field(compare=True)
    request: LLMRequest = field(compare=False)
    future: asyncio.Future[LLMResponse] = field(compare=False)
