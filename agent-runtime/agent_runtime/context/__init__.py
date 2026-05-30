"""Context Engine Pipeline — token-budgeted, priority-driven context assembly.

Collects context from multiple sources (perception, survival, state, memory),
assigns priorities, applies message filtering, and trims to a token budget
before delivering a structured ``PipelineResult`` to the decision engine.

This replaces the hardcoded ``build_prompt()`` in ``core/decide.py``.
"""

from .budget import PipelineConfig, TokenBudget
from .engine import (
    ContextEngine,
    ContextEnginePipeline,
    ContextItem,
    ContextPriority,
    ContextSource,
    MemorySource,
    MessageFilter,
    PerceptionSource,
    PipelineResult,
    PipelineStats,
    StateSource,
    SurvivalSource,
)
from .processors import (
    ContextProcessor,
    KeywordMatcher,
    RelevanceScore,
    RelevanceScorer,
    TimeDecayCalculator,
)

__all__ = [
    # Core
    "ContextEngine",
    "ContextEnginePipeline",
    "ContextItem",
    "ContextPriority",
    "ContextSource",
    # Budget
    "PipelineConfig",
    "TokenBudget",
    # Result
    "PipelineResult",
    "PipelineStats",
    # Sources
    "MemorySource",
    "PerceptionSource",
    "StateSource",
    "SurvivalSource",
    # Filter
    "MessageFilter",
    # Processors
    "ContextProcessor",
    "KeywordMatcher",
    "RelevanceScorer",
    "RelevanceScore",
    "TimeDecayCalculator",
]
