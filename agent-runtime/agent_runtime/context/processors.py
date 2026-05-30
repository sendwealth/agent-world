"""Context processors — relevance scoring, keyword matching, and time decay.

Provides the second stage of the three-stage context pipeline:
    Stage 1 (collect)  → gather raw context items from sources
    Stage 2 (rank)     → score and sort items by relevance  ← this module
    Stage 3 (truncate) → trim to token budget

Public symbols (re-exported via ``context/__init__.py``):
    RelevanceScorer, KeywordMatcher, TimeDecayCalculator, ContextProcessor
"""

from __future__ import annotations

import logging
from dataclasses import dataclass
from typing import Any

logger = logging.getLogger(__name__)


# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------

_HALF_LIFE_TICKS: float = 500.0  # Ebbinghaus-inspired half-life
_KEYWORD_WEIGHT: float = 0.6
_TIME_DECAY_WEIGHT: float = 0.3
_IMPORTANCE_WEIGHT: float = 0.1


# ---------------------------------------------------------------------------
# Keyword matcher
# ---------------------------------------------------------------------------


class KeywordMatcher:
    """Score items by keyword overlap between query and item content.

    Uses simple word-set intersection normalised by query length.
    This is intentionally lightweight — no vector DB, no embedding model.
    """

    @staticmethod
    def score(query: str, content: str) -> float:
        """Return a 0–1 overlap score between *query* and *content*.

        The score is ``|query_words ∩ content_words| / |query_words|``.
        Returns 0.0 for empty queries.
        """
        if not query or not content:
            return 0.0
        query_words = set(query.lower().split())
        if not query_words:
            return 0.0
        content_words = set(content.lower().split())
        overlap = len(query_words & content_words)
        return overlap / len(query_words)


# ---------------------------------------------------------------------------
# Time-decay calculator
# ---------------------------------------------------------------------------


class TimeDecayCalculator:
    """Compute an exponential time-decay factor for context items.

    Uses a half-life model: ``decay_factor = 2^(-elapsed / half_life)``.
    """

    def __init__(self, half_life_ticks: float = _HALF_LIFE_TICKS) -> None:
        if half_life_ticks <= 0:
            raise ValueError("half_life_ticks must be positive")
        self._half_life = half_life_ticks

    def decay(self, current_tick: int, item_tick: int) -> float:
        """Return the decay factor in (0, 1] for the given tick delta.

        Newer items (small delta) score close to 1.0; old items approach 0.0.
        """
        elapsed = max(0, current_tick - item_tick)
        return 2.0 ** (-elapsed / self._half_life)


# ---------------------------------------------------------------------------
# Relevance scorer
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class RelevanceScore:
    """Breakdown of a single item's relevance score."""

    keyword: float = 0.0
    time_decay: float = 1.0
    importance: float = 0.0
    total: float = 0.0


class RelevanceScorer:
    """Combined relevance scorer: keyword + time decay + importance.

    Weights default to:
        keyword  = 0.6
        decay    = 0.3
        importance = 0.1

    These can be overridden at construction time.
    """

    def __init__(
        self,
        *,
        keyword_weight: float = _KEYWORD_WEIGHT,
        time_decay_weight: float = _TIME_DECAY_WEIGHT,
        importance_weight: float = _IMPORTANCE_WEIGHT,
        half_life_ticks: float = _HALF_LIFE_TICKS,
    ) -> None:
        self._kw_weight = keyword_weight
        self._td_weight = time_decay_weight
        self._imp_weight = importance_weight
        self._matcher = KeywordMatcher()
        self._decay = TimeDecayCalculator(half_life_ticks=half_life_ticks)

    def score(
        self,
        query: str,
        content: str,
        current_tick: int,
        item_tick: int,
        importance: float = 0.5,
    ) -> RelevanceScore:
        """Compute a full relevance score for a single item.

        Parameters
        ----------
        query : str
            The search query (e.g. current decision context).
        content : str
            The item content to score against.
        current_tick : int
            The current simulation tick.
        item_tick : int
            The tick when the item was created / observed.
        importance : float
            The item's importance score (0–1).

        Returns
        -------
        RelevanceScore
            Score breakdown with a ``total`` field.
        """
        kw = self._matcher.score(query, content)
        td = self._decay.decay(current_tick, item_tick)
        imp = max(0.0, min(1.0, importance))

        total = self._kw_weight * kw + self._td_weight * td + self._imp_weight * imp
        return RelevanceScore(keyword=kw, time_decay=td, importance=imp, total=total)

    def rank_items(
        self,
        items: list,
        query: str,
        current_tick: int,
    ) -> list[tuple[Any, RelevanceScore]]:
        """Score and rank a list of context items by relevance.

        Parameters
        ----------
        items :
            Iterable of objects with ``content``, ``token_estimate`` and
            optional ``metadata`` attributes.
        query : str
            Search query string.
        current_tick : int
            Current tick for time-decay calculation.

        Returns
        -------
        list[tuple[item, RelevanceScore]]
            Items sorted by relevance (highest first).
        """
        scored: list[tuple[Any, RelevanceScore]] = []
        for item in items:
            content = getattr(item, "content", str(item))
            item_tick = getattr(item, "metadata", {}).get("tick", current_tick)
            importance = getattr(item, "metadata", {}).get("importance", 0.5)
            s = self.score(query, content, current_tick, item_tick, importance)
            scored.append((item, s))

        scored.sort(key=lambda pair: pair[1].total, reverse=True)
        return scored


# ---------------------------------------------------------------------------
# Context processor (Stage 2 orchestrator)
# ---------------------------------------------------------------------------


class ContextProcessor:
    """Three-stage context processing pipeline stage: filter → rank → truncate.

    This processor is the *relevance ranking* stage of the overall
    ``ContextEnginePipeline``.  It:

    1. Scores every non-protected item using ``RelevanceScorer``.
    2. Sorts items by relevance (protected items keep their position).
    3. Returns the reordered list ready for budget truncation.

    Usage::

        processor = ContextProcessor()
        ranked = processor.process(items, query="decide action", current_tick=42)
    """

    def __init__(self, scorer: RelevanceScorer | None = None) -> None:
        self._scorer = scorer or RelevanceScorer()

    def process(
        self,
        items: list,
        query: str,
        current_tick: int,
    ) -> list:
        """Score, rank, and reorder items by relevance.

        Protected items are moved to the front (preserving their relative
        order) and are not re-scored.  Non-protected items are sorted by
        descending relevance score.

        Parameters
        ----------
        items :
            List of ``ContextItem``-compatible objects.
        query : str
            Query string for keyword matching.
        current_tick : int
            Current tick for time-decay.

        Returns
        -------
        list
            Reordered items.
        """
        protected: list = []
        regular: list = []

        for item in items:
            if item.protected:
                protected.append(item)
            else:
                regular.append(item)

        if not regular:
            return protected

        ranked = self._scorer.rank_items(regular, query, current_tick)
        ranked_items = [item for item, _score in ranked]

        return protected + ranked_items
