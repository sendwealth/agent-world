"""Async LLM request queue with concurrency control and priority scheduling.

Manages concurrent access to LLM providers so that multiple agents can
share a bounded pool of LLM connections without blocking the world tick.

Key design:
- ``asyncio.Semaphore`` limits concurrent LLM requests
- Priority queue: survival > social > explore > default
- Per-request timeout with fallback on timeout
- Graceful start/stop lifecycle

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
import time
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
            call returns a fallback response instead of raising.
        fallback_on_timeout: Whether to return a fallback response on timeout
            (if False, the timeout raises ``asyncio.TimeoutError``).
    """

    max_concurrency: int = 2
    timeout_seconds: float = 30.0
    fallback_on_timeout: bool = True


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

_FALLBACK_RESPONSE = LLMResponse(
    content='{"action": "rest", "parameters": {}, "reasoning": "LLM queue fallback", "confidence": 0}',
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

    Higher-priority requests are dequeued first (using ``asyncio.PriorityQueue``).
    """

    def __init__(self, provider: LLMProvider, config: QueueConfig | None = None) -> None:
        self._provider = provider
        self._config = config or QueueConfig()
        self._semaphore = asyncio.Semaphore(self._config.max_concurrency)
        self._queue: asyncio.PriorityQueue[_QueueEntry] = asyncio.PriorityQueue()
        self._stats = QueueStats()
        self._running = False
        self._entry_counter = 0  # monotonic tiebreaker for FIFO within same priority

    # ------------------------------------------------------------------
    # Lifecycle
    # ------------------------------------------------------------------

    async def start(self) -> None:
        """Start the queue processor."""
        self._running = True
        logger.info(
            "LLMQueue started: max_concurrency=%d timeout=%.1fs",
            self._config.max_concurrency,
            self._config.timeout_seconds,
        )

    async def stop(self) -> None:
        """Stop the queue and cancel in-flight requests."""
        self._running = False
        logger.info(
            "LLMQueue stopped: stats=%s",
            self._stats.__dict__,
        )

    # ------------------------------------------------------------------
    # Public API
    # ------------------------------------------------------------------

    async def enqueue(self, request: LLMRequest) -> LLMResponse:
        """Submit a request and wait for the result.

        The call blocks (await) until:
        1. A semaphore slot is available, AND
        2. The LLM provider returns a response (or times out).

        For non-blocking behaviour, use ``AsyncDecisionProvider`` which
        decouples the tick from this call.
        """
        if not self._running:
            await self.start()

        self._stats.total_requests += 1
        priority = _resolve_priority(request.priority)

        try:
            async with self._semaphore:
                self._stats.active_requests += 1
                try:
                    response = await asyncio.wait_for(
                        self._provider.chat(
                            request.messages,
                            max_tokens=request.max_tokens,
                            temperature=request.temperature,
                        ),
                        timeout=self._config.timeout_seconds,
                    )
                    self._stats.completed_requests += 1
                    return response
                except asyncio.TimeoutError:
                    self._stats.timed_out_requests += 1
                    logger.warning(
                        "LLM request timed out (%.1fs) priority=%s",
                        self._config.timeout_seconds,
                        request.priority,
                    )
                    if self._config.fallback_on_timeout:
                        return _FALLBACK_RESPONSE
                    raise
                except Exception:
                    self._stats.failed_requests += 1
                    raise
                finally:
                    self._stats.active_requests -= 1
        except asyncio.TimeoutError:
            # Re-raise if fallback_on_timeout is False
            raise
        except Exception:
            # If acquiring semaphore fails for any reason, return fallback
            if self._config.fallback_on_timeout:
                return _FALLBACK_RESPONSE
            raise

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
