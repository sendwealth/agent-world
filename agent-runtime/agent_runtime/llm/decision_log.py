"""Decision logging for LLM-driven agent actions.

Records the complete prompt/response/action triple for each agent decision,
persisted to JSONL files for offline analysis.
"""

from __future__ import annotations

import asyncio
import io
import json
import logging
from collections.abc import Iterator
from dataclasses import asdict, dataclass
from datetime import UTC, datetime
from pathlib import Path

logger = logging.getLogger(__name__)


def _now_iso() -> str:
    """Return the current UTC time as an ISO 8601 string."""
    return datetime.now(UTC).isoformat()


@dataclass
class DecisionLog:
    """A single agent decision record — the prompt/response/action triple."""

    agent_id: str
    tick: int
    timestamp: str
    prompt: str
    response_raw: str
    action_chosen: str
    reasoning: str
    confidence: int
    llm_model: str
    latency_ms: float
    fallback: bool = False

    def to_dict(self) -> dict:
        """Serialize to a plain dict."""
        return asdict(self)


class DecisionLogStore:
    """In-memory + JSONL-file decision log store.

    Thread-safe for concurrent ``append`` calls via an internal
    ``asyncio.Lock``.  Supports context-manager usage to ensure the
    file handle is properly closed::

        async with DecisionLogStore() as store:
            await store.append(log)

    If no ``path`` is given the store operates in memory only.
    """

    def __init__(self, path: str | Path | None = None) -> None:
        self._logs: list[DecisionLog] = []
        self._path = Path(path) if path else None
        self._fh: io.IOBase | None = None  # TextIO when open
        self._lock: asyncio.Lock | None = None

    def _get_lock(self) -> asyncio.Lock:
        if self._lock is None:
            self._lock = asyncio.Lock()
        return self._lock

    # -- Context manager --------------------------------------------------

    async def __aenter__(self) -> DecisionLogStore:
        if self._path is not None:
            self._path.parent.mkdir(parents=True, exist_ok=True)
            self._fh = open(self._path, "a", encoding="utf-8")  # noqa: ASYNC230,SIM115
        return self

    async def __aexit__(self, *exc: object) -> None:
        self.close()

    # -- Public API -------------------------------------------------------

    async def append(self, log: DecisionLog) -> None:
        """Add a decision log entry (in-memory + optional JSONL flush)."""
        async with self._get_lock():
            self._logs.append(log)
            self._write_line(log.to_dict())

    def query(
        self,
        *,
        agent_id: str | None = None,
        tick_min: int | None = None,
        tick_max: int | None = None,
    ) -> list[DecisionLog]:
        """Return logs matching the given filters."""
        result = self._logs
        if agent_id is not None:
            result = [entry for entry in result if entry.agent_id == agent_id]
        if tick_min is not None:
            result = [entry for entry in result if entry.tick >= tick_min]
        if tick_max is not None:
            result = [entry for entry in result if entry.tick <= tick_max]
        return list(result)

    @property
    def count(self) -> int:
        """Number of stored log entries."""
        return len(self._logs)

    def __len__(self) -> int:
        return len(self._logs)

    def __iter__(self) -> Iterator[DecisionLog]:
        return iter(self._logs)

    def close(self) -> None:
        """Flush and close the JSONL file handle if open."""
        if self._fh is not None:
            try:
                self._fh.flush()
                self._fh.close()
            except OSError:
                logger.warning("Failed to close decision log file", exc_info=True)
            finally:
                self._fh = None

    # -- Internal ---------------------------------------------------------

    def _write_line(self, data: dict) -> None:
        """Append a JSON line to the JSONL file (if open)."""
        if self._fh is None:
            return
        try:
            self._fh.write(json.dumps(data, ensure_ascii=False) + "\n")
            self._fh.flush()
        except OSError:
            logger.warning("Failed to write decision log line", exc_info=True)
