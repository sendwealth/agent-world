"""Memory subsystem — in-memory, persistent, and vector DB-backed memory layers.

Memory tiers:
- WorkingMemory: In-memory FIFO cache for recent interactions (free reads).
- ShortTermMemory: SQLite-backed medium-term memory with keyword search.
- LongTermMemory: SQLite-backed long-term memory with keyword search.
- VectorMemory: Vector-backed long-term memory with semantic retrieval,
  forgetting curve decay, and experience/lesson/fact types.
"""

from agent_runtime.memory.embedding import (
    EmbeddingProviderProtocol,
    HashEmbeddingProvider,
    OpenAIEmbeddingProvider,
    SentenceTransformerEmbeddingProvider,
)
from agent_runtime.memory.long_term import (
    LongTermMemory,
    LongTermMemoryEntry,
    LongTermMemoryProtocol,
)
from agent_runtime.memory.short_term import (
    ShortTermMemory,
    ShortTermMemoryEntry,
    ShortTermMemoryProtocol,
)
from agent_runtime.memory.vector_memory import (
    MemoryType,
    VectorMemory,
    VectorMemoryEntry,
    VectorMemoryProtocol,
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
    "SentenceTransformerEmbeddingProvider",
    "ShortTermMemory",
    "ShortTermMemoryEntry",
    "ShortTermMemoryProtocol",
    "VectorMemory",
    "VectorMemoryEntry",
    "VectorMemoryProtocol",
    "WorkingMemory",
    "WorkingMemoryProtocol",
]
