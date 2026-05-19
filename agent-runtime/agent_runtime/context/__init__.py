"""Context Engine Pipeline — token-budgeted, priority-driven context assembly.

Collects context from multiple sources (perception, survival, state, memory),
assigns priorities, applies message filtering, and trims to a token budget
before delivering a structured ``PipelineResult`` to the decision engine.

This will eventually replace the hardcoded ``build_prompt()`` in
``core/decide.py``, but for now lives as an independent module.
"""

from .engine import (
    ContextEnginePipeline,
    ContextItem,
    ContextPriority,
    ContextSource,
    MemorySource,
    MessageFilter,
    PerceptionSource,
    PipelineConfig,
    PipelineResult,
    PipelineStats,
    StateSource,
    SurvivalSource,
    TokenBudget,
)

__all__ = [
    "ContextEnginePipeline",
    "ContextItem",
    "ContextPriority",
    "ContextSource",
    "MemorySource",
    "MessageFilter",
    "PerceptionSource",
    "PipelineConfig",
    "PipelineResult",
    "PipelineStats",
    "StateSource",
    "SurvivalSource",
    "TokenBudget",
]
