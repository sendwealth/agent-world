"""Token budget controller for the context engine pipeline.

Provides:
- ``TokenBudget`` — priority-aware token budget manager with a configurable
  safety margin so the LLM prompt never overshoots the model's context window.
- ``PipelineConfig`` — pipeline-level configuration including the token cap
  and environment-variable override.

Both are re-exported via ``context/__init__.py``.
"""

from __future__ import annotations

import logging
import os
from dataclasses import dataclass

logger = logging.getLogger(__name__)

# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------

_DEFAULT_MAX_TOKENS: int = 4096
_SAFETY_MARGIN: int = 100  # tokens reserved as headroom


# ---------------------------------------------------------------------------
# TokenBudget
# ---------------------------------------------------------------------------


@dataclass
class TokenBudget:
    """Token budget manager with a built-in safety margin.

    Protected items (survival-critical information) are **never** trimmed
    and are **not** constrained by ``max_tokens``.  When protected items
    alone exceed ``max_tokens``, the ``protected_overflow`` flag in
    ``PipelineStats`` is set so the caller can react accordingly.

    Attributes:
        max_tokens: Hard cap on total tokens for regular (non-protected)
            items.  The effective budget used for regular items is
            ``max_tokens - safety_margin`` so there is always headroom
            for response generation.
        safety_margin: Number of tokens reserved as headroom.  Defaults
            to 0 for backward compatibility.  The ``PipelineConfig``
            defaults to 100 and passes the margin through to the budget
            allocator.
    """

    max_tokens: int = _DEFAULT_MAX_TOKENS
    safety_margin: int = 0

    def allocate(
        self, items: list,
    ) -> tuple[list, int, bool]:
        """Trim items to fit within the token budget (minus safety margin).

        Protected items are always kept.  Remaining items are sorted by
        priority (ascending — P0 first) and included until the effective
        budget is exhausted.

        Parameters
        ----------
        items :
            A list of ``ContextItem``-compatible objects that have
            ``protected``, ``priority``, and ``token_estimate`` attributes.

        Returns
        -------
        tuple[list, int, bool]
            ``(kept items, trimmed count, protected_overflow flag)``.
        """
        protected: list = []
        regular: list = []

        for item in items:
            if item.protected:
                protected.append(item)
            else:
                regular.append(item)

        # Effective budget after subtracting safety margin
        effective_budget = max(0, self.max_tokens - self.safety_margin)

        # Tokens used by protected items
        protected_tokens = sum(i.token_estimate for i in protected)
        overflow = protected_tokens > effective_budget
        if overflow:
            logger.warning(
                "Protected items exceed effective budget: %d > %d (max=%d, margin=%d)",
                protected_tokens,
                effective_budget,
                self.max_tokens,
                self.safety_margin,
            )
        remaining_budget = max(0, effective_budget - protected_tokens)

        # Sort regular items by priority (lower = higher priority)
        regular.sort(key=lambda i: i.priority)

        accepted: list = []
        used = 0
        for item in regular:
            if used + item.token_estimate <= remaining_budget:
                accepted.append(item)
                used += item.token_estimate

        trimmed_count = len(regular) - len(accepted)
        return protected + accepted, trimmed_count, overflow


# ---------------------------------------------------------------------------
# PipelineConfig
# ---------------------------------------------------------------------------


@dataclass
class PipelineConfig:
    """Configuration for the context engine pipeline.

    Attributes:
        max_tokens: Token budget cap (overridden by ``CONTEXT_MAX_TOKENS``
            environment variable).  Protected items are not constrained by
            this cap — they are always kept regardless of budget.  When
            protected items alone exceed ``max_tokens``,
            ``PipelineStats.protected_overflow`` is set to ``True``.
        safety_margin: Tokens reserved as headroom so the prompt never
            overshoots the model's context window.  Defaults to 100.
    """

    max_tokens: int = _DEFAULT_MAX_TOKENS
    safety_margin: int = _SAFETY_MARGIN

    def __post_init__(self) -> None:
        env_val = os.environ.get("CONTEXT_MAX_TOKENS")
        if env_val is not None:
            try:
                self.max_tokens = int(env_val)
            except ValueError:
                logger.warning(
                    "Invalid CONTEXT_MAX_TOKENS=%r, using default %d",
                    env_val,
                    self.max_tokens,
                )
