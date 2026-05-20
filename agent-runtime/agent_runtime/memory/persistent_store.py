"""Unified persistent storage for agent state and memories.

Wraps existing LongTermMemory and VectorMemory to provide cross-restart
persistence for agent runtime state. The store uses SQLite as the backing
database, keeping all data in a single file per agent.
"""

from __future__ import annotations

import json
import sqlite3
from pathlib import Path
from typing import Any, Dict, List, Optional
from uuid import UUID

from agent_runtime.memory.long_term import LongTermMemory, LongTermMemoryEntry
from agent_runtime.memory.vector_memory import VectorMemory, VectorMemoryEntry
from agent_runtime.models.agent_state import AgentState


class PersistentMemoryStore:
    """Agent-level persistent storage that survives process restarts.

    Manages:
    - AgentState serialization / deserialization (full state snapshot)
    - Wraps LongTermMemory for persistent long-term storage
    - Wraps VectorMemory for persistent vector-backed memory
    """

    def __init__(
        self,
        agent_id: str,
        db_dir: str = "./data/agents",
        vector_db_dir: str = "./data/agents",
    ) -> None:
        self.agent_id = agent_id
        self.db_dir = Path(db_dir)
        self.db_dir.mkdir(parents=True, exist_ok=True)
        self._state_db_path = self.db_dir / f"{agent_id}_state.db"

        self._long_term = LongTermMemory(
            db_path=str(self.db_dir / f"{agent_id}_ltm.db"),
        )
        self._vector_memory = VectorMemory(
            db_path=str(Path(vector_db_dir) / f"{agent_id}_vm.db"),
        )
        self._init_state_db()

    def _init_state_db(self) -> None:
        conn = sqlite3.connect(str(self._state_db_path))
        conn.execute("""
            CREATE TABLE IF NOT EXISTS agent_state (
                id          TEXT PRIMARY KEY,
                state_json  TEXT NOT NULL,
                updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
            )
        """)
        conn.commit()
        conn.close()

    # ── AgentState persistence ──────────────────────────────

    def save_agent_state(self, state: AgentState) -> None:
        """Save a complete AgentState snapshot to SQLite."""
        state_json = state.to_json()
        conn = sqlite3.connect(str(self._state_db_path))
        conn.execute(
            "INSERT OR REPLACE INTO agent_state (id, state_json) VALUES (?1, ?2)",
            (self.agent_id, state_json),
        )
        conn.commit()
        conn.close()

    def load_agent_state(self) -> Optional[AgentState]:
        """Load the most recent AgentState snapshot.

        Returns None if no state has been saved yet.
        """
        conn = sqlite3.connect(str(self._state_db_path))
        row = conn.execute(
            "SELECT state_json FROM agent_state WHERE id = ?1",
            (self.agent_id,),
        ).fetchone()
        conn.close()

        if row is None:
            return None
        return AgentState.from_json(row[0])

    # ── Long-term memory delegation ─────────────────────────

    def save_memory(
        self,
        content: str,
        *,
        category: str = "experience",
        importance: float = 0.7,
        source: str | None = None,
        tick: int = 0,
        metadata: dict[str, Any] | None = None,
    ) -> LongTermMemoryEntry:
        """Store a memory entry via the underlying LongTermMemory."""
        return self._long_term.store(
            content,
            category=category,
            importance=importance,
            source=source,
            tick=tick,
            metadata=metadata,
        )

    def search_memories(
        self,
        query: str,
        *,
        top_k: int = 5,
        category: str | None = None,
        tick: int = 0,
    ) -> list[LongTermMemoryEntry]:
        """Search long-term memories by keyword."""
        return self._long_term.search(query, top_k=top_k, category=category, tick=tick)

    def get_recent_memories(
        self,
        *,
        top_k: int = 10,
        category: str | None = None,
    ) -> list[LongTermMemoryEntry]:
        """Get recent long-term memories."""
        return self._long_term.get_recent(top_k=top_k, category=category)

    # ── Vector memory delegation ────────────────────────────

    def save_vector_memory(
        self,
        content: str,
        *,
        memory_type: str = "experience",
        importance: float = 0.7,
        source: str | None = None,
        metadata: dict[str, Any] | None = None,
    ) -> VectorMemoryEntry:
        """Store a vector-backed memory entry."""
        return self._vector_memory.store(
            content,
            memory_type=memory_type,
            importance=importance,
            source=source,
            metadata=metadata,
        )

    def search_vector_memories(
        self,
        query: str,
        *,
        top_k: int = 5,
        memory_type: str | None = None,
        min_relevance: float = 0.0,
    ) -> list[tuple[VectorMemoryEntry, float]]:
        """Search vector memories by semantic similarity."""
        return self._vector_memory.recall(
            query,
            top_k=top_k,
            memory_type=memory_type,
            min_relevance=min_relevance,
        )

    # ── Backup / export ─────────────────────────────────────

    def backup_to_file(self, path: str) -> None:
        """Export all persistent data to a JSON file."""
        data: Dict[str, Any] = {"agent_id": self.agent_id}

        state = self.load_agent_state()
        data["agent_state"] = json.loads(state.to_json()) if state else None

        # Export long-term memories
        recent = self._long_term.get_recent(top_k=self._long_term.max_entries)
        data["long_term_memories"] = [
            {
                "content": m.content,
                "category": m.category,
                "importance": m.importance,
                "source": m.source,
                "created_tick": m.created_tick,
                "metadata": m.metadata,
            }
            for m in recent
        ]

        Path(path).parent.mkdir(parents=True, exist_ok=True)
        with open(path, "w", encoding="utf-8") as f:
            json.dump(data, f, ensure_ascii=False, indent=2)

    # ── Lifecycle ───────────────────────────────────────────

    def close(self) -> None:
        """Close all underlying database connections."""
        self._long_term.close()
        self._vector_memory.close()

    @property
    def long_term_memory(self) -> LongTermMemory:
        return self._long_term

    @property
    def vector_memory(self) -> VectorMemory:
        return self._vector_memory
