"""Memory subsystem — in-memory, persistent, and vector DB-backed memory layers.

Three memory tiers:
- WorkingMemory: In-memory FIFO cache for recent interactions (free reads).
- ShortTermMemory: SQLite-backed medium-term memory with keyword search.
- LongTermMemory: Vector DB-backed long-term memory with semantic retrieval
  and decay.  Supports experience, lesson, and fact memory types.
"""

from agent_runtime.memory.embedding import (
    EmbeddingProviderProtocol,
    HashEmbeddingProvider,
    OpenAIEmbeddingProvider,
)
from agent_runtime.memory.long_term import (
    LongTermMemory,
    LongTermMemoryEntry,
    LongTermMemoryProtocol,
    MemoryType,
)
from agent_runtime.memory.short_term import (
    ShortTermMemory,
    ShortTermMemoryEntry,
    ShortTermMemoryProtocol,
)
from agent_runtime.memory.working_memory import (
    MemoryEntry,
    WorkingMemory,
    WorkingMemoryProtocol,
)

__all__ = [
    "EmbeddingProviderProtocol",
    "HashEmbeddingProvider",
    "LongTermMemory",
    "LongTermMemoryEntry",
    "LongTermMemoryProtocol",
    "MemoryEntry",
    "MemoryType",
    "OpenAIEmbeddingProvider",
    "ShortTermMemory",
    "ShortTermMemoryEntry",
    "ShortTermMemoryProtocol",
    "WorkingMemory",
    "WorkingMemoryProtocol",
]
