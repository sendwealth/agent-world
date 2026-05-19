"""Memory recall integration — bridges vector memory with the think loop decide phase.

Provides a ``MemoryRecall`` component that:
1. Receives the agent's current context (state + perception).
2. Queries the vector memory for relevant past experiences, lessons, and facts.
3. Formats the recalled memories as context for the decision engine.

This is called during the think loop's ``decide`` step so that past
experience informs current decisions.
"""

from __future__ import annotations

import logging
from dataclasses import dataclass
from typing import Any, Protocol, runtime_checkable

logger = logging.getLogger(__name__)

# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------

_DEFAULT_MAX_MEMORIES: int = 5
_DEFAULT_MIN_RELEVANCE: float = 0.3


# ---------------------------------------------------------------------------
# Data types
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class RecalledMemory:
    """A memory recalled for decision context."""

    content: str
    memory_type: str  # "experience", "lesson", "fact"
    relevance: float  # Similarity/decay score
    importance: float

    def __str__(self) -> str:
        return f"[{self.memory_type}] {self.content} (relevance={self.relevance:.2f})"


# ---------------------------------------------------------------------------
# Protocol for the vector memory dependency
# ---------------------------------------------------------------------------


@runtime_checkable
class VectorMemoryProvider(Protocol):
    """Minimal interface for vector memory recall."""

    def recall_with_decay(
        self,
        query: str,
        *,
        top_k: int = 5,
        memory_type: str | None = None,
    ) -> list[tuple[Any, float]]: ...


# ---------------------------------------------------------------------------
# MemoryRecall configuration
# ---------------------------------------------------------------------------


@dataclass
class MemoryRecallConfig:
    """Configuration for memory recall during decisions.

    Attributes:
        max_memories: Maximum number of memories to recall per query.
        min_relevance: Minimum relevance score to include a memory.
        include_experiences: Whether to recall experience-type memories.
        include_lessons: Whether to recall lesson-type memories.
        include_facts: Whether to recall fact-type memories.
    """

    max_memories: int = _DEFAULT_MAX_MEMORIES
    min_relevance: float = _DEFAULT_MIN_RELEVANCE
    include_experiences: bool = True
    include_lessons: bool = True
    include_facts: bool = True


# ---------------------------------------------------------------------------
# MemoryRecall
# ---------------------------------------------------------------------------


class MemoryRecall:
    """Bridges vector memory with the think loop decision phase.

    During each decide step, this component:
    1. Builds a query from the current agent state and perception.
    2. Recalls relevant memories from vector storage (with decay).
    3. Formats them as context for the decision engine.

    Usage::

        from agent_runtime.memory.memory_recall import MemoryRecall
        from agent_runtime.memory.vector_memory import VectorMemory

        vector_mem = VectorMemory()
        recall = MemoryRecall(vector_memory=vector_mem)

        # In the decide step:
        context = recall.build_context(
            query="deciding whether to trade resources",
            situation="low on tokens, need to earn",
        )
        # context is a formatted string to include in the decision prompt
    """

    def __init__(
        self,
        vector_memory: VectorMemoryProvider,
        *,
        config: MemoryRecallConfig | None = None,
    ) -> None:
        self._vm = vector_memory
        self._config = config or MemoryRecallConfig()

    def recall_for_decision(
        self,
        query: str,
        *,
        situation: str = "",
    ) -> list[RecalledMemory]:
        """Recall relevant memories for a decision.

        Parameters
        ----------
        query : str
            The decision context to search for related memories.
        situation : str
            Additional situational context to append to the query.

        Returns
        -------
        list[RecalledMemory]
            Recalled memories sorted by relevance.
        """
        full_query = f"{query} {situation}".strip() if situation else query
        recalled: list[RecalledMemory] = []

        # Recall from each enabled memory type
        types_to_search: list[str] = []
        if self._config.include_experiences:
            types_to_search.append("experience")
        if self._config.include_lessons:
            types_to_search.append("lesson")
        if self._config.include_facts:
            types_to_search.append("fact")

        total_budget = self._config.max_memories
        per_type_budget = max(1, total_budget // len(types_to_search)) if types_to_search else 0

        for mt in types_to_search:
            try:
                results = self._vm.recall_with_decay(
                    full_query,
                    top_k=per_type_budget,
                    memory_type=mt,
                )
                for entry, score in results:
                    if score >= self._config.min_relevance:
                        recalled.append(RecalledMemory(
                            content=entry.content,
                            memory_type=entry.memory_type,
                            relevance=score,
                            importance=entry.importance,
                        ))
            except Exception:
                logger.warning("Failed to recall %s memories", mt, exc_info=True)

        # Sort by relevance and trim to budget
        recalled.sort(key=lambda m: m.relevance, reverse=True)
        return recalled[:self._config.max_memories]

    def build_context(
        self,
        query: str,
        *,
        situation: str = "",
    ) -> str:
        """Build a formatted context string from recalled memories.

        This is designed to be injected into the decision prompt.

        Parameters
        ----------
        query : str
            The decision context.
        situation : str
            Additional situational context.

        Returns
        -------
        str
            Formatted memory context string for the prompt, or empty
            string if no relevant memories are found.
        """
        memories = self.recall_for_decision(query, situation=situation)
        if not memories:
            return ""

        lines = ["## Relevant Past Memories"]
        for m in memories:
            lines.append(
                f"  - [{m.memory_type}] {m.content} "
                f"(relevance: {m.relevance:.2f}, importance: {m.importance:.2f})"
            )
        return "\n".join(lines)

    @property
    def config(self) -> MemoryRecallConfig:
        return self._config
