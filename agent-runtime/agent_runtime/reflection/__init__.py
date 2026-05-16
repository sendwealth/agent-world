"""Reflection layer — periodic self-assessment and strategy adjustment.

Triggered every 10 ticks (configurable). Evaluates recent action outcomes,
computes success rate and token efficiency per action type, updates strategy
preferences, and writes key decisions and lessons to long-term memory.
"""

from agent_runtime.reflection.reflection import (
    ActionStatus,
    ActionTypeStats,
    ReflectionConfig,
    ReflectionLayer,
    ReflectionResult,
)
from agent_runtime.reflection.strategy import StrategyPreference, StrategyRegistry
from agent_runtime.reflection.memory import LongTermMemory, MemoryEntry

__all__ = [
    "ActionStatus",
    "ActionTypeStats",
    "LongTermMemory",
    "MemoryEntry",
    "ReflectionConfig",
    "ReflectionLayer",
    "ReflectionResult",
    "StrategyPreference",
    "StrategyRegistry",
]
