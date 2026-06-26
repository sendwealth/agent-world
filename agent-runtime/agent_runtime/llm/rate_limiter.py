"""Cross-process global rate limiter using a file-lock based token bucket.

This module provides a process-safe token-bucket rate limiter that works
even when multiple agent processes (each with their own event loop) are
running simultaneously.  It uses an OS-level advisory lock on a
well-known file path so that acquiring a token is always serialized
across processes.

Usage::

    limiter = RateLimiter(path="/tmp/agent_world_llm_limiter.lock",
                          rate=2.0, burst=4)
    async with limiter:
        await llm_provider.chat(messages)

Configuration (all via environment variables)::

    LLM_RATE_LIMIT_RATE       — tokens/sec  (default 2.0)
    LLM_RATE_LIMIT_BURST      — bucket size (default 4)
    LLM_RATE_LIMIT_PATH — lock file path
        (default $HOME/.cache/agent-world/llm_rate_limit.lock)
"""

from __future__ import annotations

import asyncio
import fcntl
import io
import json
import logging
import os
import time
from pathlib import Path

logger = logging.getLogger(__name__)

# ---------------------------------------------------------------------------
# Environment-variable defaults
# ---------------------------------------------------------------------------

_ENV_RATE = float(os.environ.get("LLM_RATE_LIMIT_RATE", "2.0"))
_ENV_BURST = int(os.environ.get("LLM_RATE_LIMIT_BURST", "4"))
_ENV_PATH = os.environ.get(
    "LLM_RATE_LIMIT_PATH",
    str(Path.home() / ".cache" / "agent-world" / "llm_rate_limit.lock"),
)

# ---------------------------------------------------------------------------
# Internal state file format
# ---------------------------------------------------------------------------

# JSON schema stored in the lock file:
# {"tokens": float, "last_refill": epoch_seconds}


def _default_rate() -> float:
    return _ENV_RATE


def _default_burst() -> int:
    return _ENV_BURST


def _default_lock_path() -> str:
    return _ENV_PATH


# ---------------------------------------------------------------------------
# RateLimiter — file-lock backed token bucket
# ---------------------------------------------------------------------------


class RateLimiter:
    """Process-safe token-bucket rate limiter using OS file locks.

    Attributes:
        rate: Refill rate (tokens per second).
        burst: Maximum bucket capacity (burst size).
        lock_path: Path to the shared lock file.
    """

    def __init__(
        self,
        *,
        rate: float | None = None,
        burst: int | None = None,
        path: str | None = None,
    ) -> None:
        self._rate = rate if rate is not None else _default_rate()
        self._burst = burst if burst is not None else _default_burst()
        self._lock_path = path if path is not None else _default_lock_path()
        self._lock_fd: io.TextIOWrapper | None = None
        self._initialized = False

    # -- lifecycle ----------------------------------------------------------

    def ensure_initialized(self) -> None:
        """Create the lock-file directory and open the fd (idempotent)."""
        if self._initialized:
            return
        lock_dir = os.path.dirname(os.path.abspath(self._lock_path))
        os.makedirs(lock_dir, exist_ok=True)
        # Open in append mode so the file always exists
        self._lock_fd = open(self._lock_path, "a")
        self._initialized = True

    async def acquire(self) -> None:
        """Acquire a token, blocking until one is available."""
        self.ensure_initialized()
        loop = asyncio.get_event_loop()
        await loop.run_in_executor(None, self._blocking_acquire)

    def _blocking_acquire(self) -> None:
        """Synchronous acquire with file-lock serialization."""
        if self._lock_fd is None:
            return

        now = time.monotonic()
        while True:
            # Lock the file (blocks across processes)
            fcntl.flock(self._lock_fd.fileno(), fcntl.LOCK_EX)
            try:
                self._try_refill_and_consume(now)
            finally:
                # Keep the lock only for the duration of the read+write
                fcntl.flock(self._lock_fd.fileno(), fcntl.LOCK_UN)
            if self._wait_remaining <= 0:
                return
            # Sleep outside the lock so other processes aren't blocked
            time.sleep(self._wait_remaining)

    def _try_refill_and_consume(self, now: float) -> None:
        """Read state, refill tokens, consume one if available."""
        state = self._read_state()
        elapsed = max(0, now - state["_now"])
        state["_now"] = now
        state["tokens"] = min(self._burst, state["tokens"] + elapsed * self._rate)

        if state["tokens"] >= 1.0:
            state["tokens"] -= 1.0
            self._wait_remaining = 0
        else:
            # Calculate wait until 1 token is available
            deficit = 1.0 - state["tokens"]
            self._wait_remaining = deficit / self._rate if self._rate > 0 else 0

        self._write_state(state)

    # -- state persistence --------------------------------------------------

    _STATE_KEYS = {"tokens", "_now"}

    def _read_state(self) -> dict:
        if self._lock_fd is None:
            return {"tokens": float(self._burst), "_now": time.monotonic()}
        try:
            self._lock_fd.seek(0)
            text = self._lock_fd.read().strip()
            data = json.loads(text) if text else {}
        except (json.JSONDecodeError, OSError):
            data = {}
        return {
            "tokens": data.get("tokens", float(self._burst)),
            "_now": data.get("_now", time.monotonic()),
        }

    def _write_state(self, state: dict) -> None:
        if self._lock_fd is None:
            return
        payload = json.dumps({
            "tokens": round(state["tokens"], 6),
            "_now": round(state["_now"], 6),
        })
        self._lock_fd.seek(0)
        self._lock_fd.write(payload)
        self._lock_fd.truncate()

    # -- context-manager protocol -------------------------------------------

    async def __aenter__(self) -> "RateLimiter":
        await self.acquire()
        return self

    async def __aexit__(self, *exc: object) -> None:
        pass

    def __del__(self) -> None:
        try:
            if self._lock_fd is not None:
                self._lock_fd.close()
        except Exception:
            pass


# ---------------------------------------------------------------------------
# Singleton (module-level) for convenient import
# ---------------------------------------------------------------------------

_default_limiter: RateLimiter | None = None


def default_rate_limiter() -> RateLimiter:
    """Return the globally-configured :class:`RateLimiter` singleton."""
    global _default_limiter
    if _default_limiter is None:
        _default_limiter = RateLimiter()
    return _default_limiter
