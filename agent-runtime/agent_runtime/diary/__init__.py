"""Agent diary system — first-person narrative journal for each tick.

Generates, stores, and retrieves narrative diary entries that capture
the agent's subjective experience of each simulation tick.  The diary
reflects personality, mood, and the actual events that occurred, giving
observers a window into the agent's inner life.
"""

from agent_runtime.diary.diary import DiaryEntry, DiaryStore
from agent_runtime.diary.generator import DiaryGenerator, DiaryGeneratorConfig

__all__ = [
    "DiaryEntry",
    "DiaryStore",
    "DiaryGenerator",
    "DiaryGeneratorConfig",
]
