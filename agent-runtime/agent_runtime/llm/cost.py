"""Token counting and cost tracking for LLM usage.

Provides a :class:`CostTracker` that accumulates token usage and
computes estimated costs per provider/model.
"""

from __future__ import annotations

import asyncio
import logging
from dataclasses import dataclass, field
from typing import Dict

from .base import LLMResponse, ProviderType

logger = logging.getLogger(__name__)

# ---------------------------------------------------------------------------
# Pricing table (USD per 1K tokens)
# ---------------------------------------------------------------------------

# These are approximate prices as of early 2025.  Update as needed.
_PRICING: dict[str, dict[str, float]] = {
    # OpenAI
    "gpt-4": {"prompt": 0.03, "completion": 0.06},
    "gpt-4-turbo": {"prompt": 0.01, "completion": 0.03},
    "gpt-4o": {"prompt": 0.005, "completion": 0.015},
    "gpt-4o-mini": {"prompt": 0.00015, "completion": 0.0006},
    "gpt-3.5-turbo": {"prompt": 0.0005, "completion": 0.0015},
    # Anthropic
    "claude-3-opus": {"prompt": 0.015, "completion": 0.075},
    "claude-3-sonnet": {"prompt": 0.003, "completion": 0.015},
    "claude-3-haiku": {"prompt": 0.00025, "completion": 0.00125},
    "claude-3.5-sonnet": {"prompt": 0.003, "completion": 0.015},
    "claude-3.5-haiku": {"prompt": 0.0008, "completion": 0.004},
}

# Default/fallback pricing when model is not in the table
_DEFAULT_PRICING: dict[str, float] = {"prompt": 0.005, "completion": 0.015}


def _get_pricing(model: str) -> dict[str, float]:
    """Look up pricing for a model, with fallback."""
    # Try exact match first
    if model in _PRICING:
        return _PRICING[model]
    # Try prefix match (e.g., "gpt-4-0613" → "gpt-4"), longest prefix first
    for prefix in sorted(_PRICING, key=len, reverse=True):
        if model.startswith(prefix):
            return _PRICING[prefix]
    return _DEFAULT_PRICING


# ---------------------------------------------------------------------------
# UsageRecord & CostTracker
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class UsageRecord:
    """A single recorded LLM call with token usage and cost."""

    model: str
    prompt_tokens: int
    completion_tokens: int
    total_tokens: int
    cost_usd: float


@dataclass
class CostTracker:
    """Accumulates token usage and estimated cost across LLM calls.

    Thread-safe and async-safe via an internal lock.
    """

    _records: list[UsageRecord] = field(default_factory=list)
    _lock: asyncio.Lock = field(default_factory=asyncio.Lock, init=False, repr=False)

    async def record(self, response: LLMResponse) -> UsageRecord:
        """Record a completed LLM call and return the usage record.

        Computes cost based on the built-in pricing table.
        """
        pricing = _get_pricing(response.model)
        cost = (
            response.usage.prompt_tokens * pricing["prompt"] / 1000.0
            + response.usage.completion_tokens * pricing["completion"] / 1000.0
        )
        rec = UsageRecord(
            model=response.model,
            prompt_tokens=response.usage.prompt_tokens,
            completion_tokens=response.usage.completion_tokens,
            total_tokens=response.usage.total_tokens,
            cost_usd=cost,
        )
        async with self._lock:
            self._records.append(rec)
        logger.debug(
            "LLM usage: model=%s tokens=%d cost=$%.6f",
            rec.model,
            rec.total_tokens,
            rec.cost_usd,
        )
        return rec

    @property
    def total_prompt_tokens(self) -> int:
        return sum(r.prompt_tokens for r in self._records)

    @property
    def total_completion_tokens(self) -> int:
        return sum(r.completion_tokens for r in self._records)

    @property
    def total_tokens(self) -> int:
        return sum(r.total_tokens for r in self._records)

    @property
    def total_cost_usd(self) -> float:
        return sum(r.cost_usd for r in self._records)

    def summary(self) -> Dict[str, float | int]:
        """Return a summary dict of total usage and cost."""
        return {
            "calls": len(self._records),
            "total_prompt_tokens": self.total_prompt_tokens,
            "total_completion_tokens": self.total_completion_tokens,
            "total_tokens": self.total_tokens,
            "total_cost_usd": round(self.total_cost_usd, 6),
        }

    def by_model(self) -> Dict[str, Dict[str, float | int]]:
        """Break down usage and cost by model."""
        result: Dict[str, Dict[str, float | int]] = {}
        for rec in self._records:
            if rec.model not in result:
                result[rec.model] = {
                    "calls": 0,
                    "prompt_tokens": 0,
                    "completion_tokens": 0,
                    "total_tokens": 0,
                    "cost_usd": 0.0,
                }
            entry = result[rec.model]
            entry["calls"] += 1  # type: ignore[operator]
            entry["prompt_tokens"] += rec.prompt_tokens  # type: ignore[operator]
            entry["completion_tokens"] += rec.completion_tokens  # type: ignore[operator]
            entry["total_tokens"] += rec.total_tokens  # type: ignore[operator]
            entry["cost_usd"] = round(entry["cost_usd"] + rec.cost_usd, 6)  # type: ignore[operator]
        return result

    async def reset(self) -> None:
        """Clear all recorded usage."""
        async with self._lock:
            self._records.clear()
