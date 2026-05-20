"""Tests for PersistentMemoryStore — agent state and memory persistence."""

import json
import os
import tempfile
from uuid import uuid4

import pytest

from agent_runtime.memory.persistent_store import PersistentMemoryStore
from agent_runtime.models.agent_state import AgentState
from agent_runtime.models.enums import AgentPhase
from agent_runtime.models.skill import Skill


@pytest.fixture
def tmp_dir():
    with tempfile.TemporaryDirectory() as d:
        yield d


@pytest.fixture
def store(tmp_dir):
    agent_id = str(uuid4())
    s = PersistentMemoryStore(agent_id=agent_id, db_dir=tmp_dir, vector_db_dir=tmp_dir)
    yield s
    s.close()


def _make_state(name: str = "TestAgent", tokens: int = 100) -> AgentState:
    return AgentState(name=name, tokens=tokens, phase=AgentPhase.ADULT)


class TestAgentStatePersistence:
    def test_save_and_load_roundtrip(self, store):
        state = _make_state("Alice", 500)
        state.reputation = 42.0
        state.spawn_tick = 10
        store.save_agent_state(state)

        loaded = store.load_agent_state()
        assert loaded is not None
        assert loaded.name == "Alice"
        assert loaded.tokens == 500
        assert loaded.phase == AgentPhase.ADULT
        assert loaded.reputation == 42.0
        assert loaded.spawn_tick == 10

    def test_load_without_save_returns_none(self, store):
        loaded = store.load_agent_state()
        assert loaded is None

    def test_save_overwrites_previous(self, store):
        state1 = _make_state("V1", 100)
        store.save_agent_state(state1)

        state2 = _make_state("V2", 200)
        store.save_agent_state(state2)

        loaded = store.load_agent_state()
        assert loaded is not None
        assert loaded.name == "V2"
        assert loaded.tokens == 200

    def test_persistence_with_skills(self, store):
        state = _make_state("SkilledAgent")
        state.add_skill(Skill(name="mining", level=5, experience=300, max_level=10))
        state.add_skill(Skill(name="trading", level=3, experience=150, max_level=10))
        store.save_agent_state(state)

        loaded = store.load_agent_state()
        assert loaded is not None
        assert len(loaded.skills) == 2
        assert loaded.skills["mining"].level == 5
        assert loaded.skills["trading"].experience == 150

    def test_persistence_with_personality(self, store):
        state = _make_state("PersonalityAgent")
        state.personality = {"curiosity": 0.8, "caution": 0.3, "ambition": 0.9}
        store.save_agent_state(state)

        loaded = store.load_agent_state()
        assert loaded is not None
        assert loaded.personality["curiosity"] == 0.8
        assert loaded.personality["ambition"] == 0.9


class TestMemoryPersistence:
    def test_save_and_search_long_term(self, store):
        store.save_memory("Learned to mine gold", category="experience", importance=0.9, tick=100)
        store.save_memory("Trading strategy: buy low", category="strategy", importance=0.8, tick=200)

        results = store.search_memories("mine", top_k=5)
        assert len(results) >= 1
        assert any("mine" in r.content for r in results)

    def test_get_recent_memories(self, store):
        store.save_memory("First memory", tick=1)
        store.save_memory("Second memory", tick=2)
        store.save_memory("Third memory", tick=3)

        recent = store.get_recent_memories(top_k=2)
        assert len(recent) == 2

    def test_vector_memory_persistence(self, store):
        entry = store.save_vector_memory("Discovered a new trade route", memory_type="experience")
        assert entry.content == "Discovered a new trade route"

        results = store.search_vector_memories("trade", top_k=5)
        assert len(results) >= 1
        assert results[0][0].content == "Discovered a new trade route"


class TestBackup:
    def test_backup_creates_valid_json(self, store, tmp_dir):
        state = _make_state("BackupAgent", 1000)
        store.save_agent_state(state)
        store.save_memory("Important lesson", category="lesson")

        backup_path = os.path.join(tmp_dir, "backup.json")
        store.backup_to_file(backup_path)

        with open(backup_path) as f:
            data = json.load(f)

        assert data["agent_id"] == store.agent_id
        assert data["agent_state"]["name"] == "BackupAgent"
        assert len(data["long_term_memories"]) >= 1
