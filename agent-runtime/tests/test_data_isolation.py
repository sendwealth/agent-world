"""Tests for agent data directory isolation (SEN-497).

Validates:
  1. Each agent gets an independent data directory under ``data/{name}/``
  2. Each directory contains: memory.db, skills.json, trace.db
  3. Pool spawn auto-creates data directories with data files
  4. ``--data-dir`` is passed to child processes
  5. AgentState persists to each agent's directory
  6. Kill agent A, confirm agent B data is unaffected
"""

from __future__ import annotations

import json
import os
import sqlite3
import sys
import tempfile
from pathlib import Path
from unittest.mock import MagicMock, patch

import pytest

# ---------------------------------------------------------------------------
# Unit tests: _init_data_dir (in __main__.py)
# ---------------------------------------------------------------------------


def test_init_data_dir_creates_files(tmp_path: Path) -> None:
    """_init_data_dir creates memory.db, skills.json, trace.db, agent_state.json."""
    # We need to import the function from __main__ which has special import handling
    from agent_runtime.models.agent_state import AgentState
    from agent_runtime.models.skill import Skill

    state = AgentState(name="TestAgent")
    state.add_skill(Skill(name="coding", level=3))

    # Import _init_data_dir from __main__
    from agent_runtime.__main__ import _init_data_dir

    data_dir = tmp_path / "data" / "testagent"
    _init_data_dir(data_dir, state)

    assert data_dir.is_dir()
    assert (data_dir / "memory.db").exists()
    assert (data_dir / "skills.json").exists()
    assert (data_dir / "trace.db").exists()
    assert (data_dir / "agent_state.json").exists()


def test_init_data_dir_memory_db_is_sqlite(tmp_path: Path) -> None:
    """memory.db is a valid SQLite database with a memories table."""
    from agent_runtime.__main__ import _init_data_dir
    from agent_runtime.models.agent_state import AgentState

    state = AgentState(name="Agent1")
    data_dir = tmp_path / "data" / "agent1"
    _init_data_dir(data_dir, state)

    conn = sqlite3.connect(str(data_dir / "memory.db"))
    # Should not raise
    conn.execute("SELECT * FROM memories")
    conn.close()


def test_init_data_dir_trace_db_is_sqlite(tmp_path: Path) -> None:
    """trace.db is a valid SQLite database with a tick_snapshots table."""
    from agent_runtime.__main__ import _init_data_dir
    from agent_runtime.models.agent_state import AgentState

    state = AgentState(name="Agent1")
    data_dir = tmp_path / "data" / "agent1"
    _init_data_dir(data_dir, state)

    conn = sqlite3.connect(str(data_dir / "trace.db"))
    conn.execute("SELECT * FROM tick_snapshots")
    conn.close()


def test_init_data_dir_skills_json_content(tmp_path: Path) -> None:
    """skills.json contains the agent's skills serialised as JSON."""
    from agent_runtime.__main__ import _init_data_dir
    from agent_runtime.models.agent_state import AgentState
    from agent_runtime.models.skill import Skill

    state = AgentState(name="Agent1")
    state.add_skill(Skill(name="coding", level=3))
    state.add_skill(Skill(name="trading", level=1))

    data_dir = tmp_path / "data" / "agent1"
    _init_data_dir(data_dir, state)

    skills = json.loads((data_dir / "skills.json").read_text())
    assert "coding" in skills
    assert "trading" in skills
    assert skills["coding"]["level"] == 3


def test_init_data_dir_agent_state_json(tmp_path: Path) -> None:
    """agent_state.json is a valid serialised AgentState."""
    from agent_runtime.__main__ import _init_data_dir, _load_agent_state_from_dir
    from agent_runtime.models.agent_state import AgentState

    state = AgentState(name="Agent1", tokens=500)
    data_dir = tmp_path / "data" / "agent1"
    _init_data_dir(data_dir, state)

    loaded = _load_agent_state_from_dir(data_dir)
    assert loaded is not None
    assert loaded.name == "Agent1"
    assert loaded.tokens == 500


def test_init_data_dir_idempotent(tmp_path: Path) -> None:
    """Calling _init_data_dir twice does not corrupt existing data."""
    from agent_runtime.__main__ import _init_data_dir
    from agent_runtime.models.agent_state import AgentState

    state = AgentState(name="Agent1")
    data_dir = tmp_path / "data" / "agent1"

    _init_data_dir(data_dir, state)

    # Write something to memory.db
    conn = sqlite3.connect(str(data_dir / "memory.db"))
    conn.execute("INSERT INTO memories (tick, content, created_at) VALUES (1, 'test', 0.0)")
    conn.commit()
    conn.close()

    # Re-init should not wipe existing data
    _init_data_dir(data_dir, state)

    conn = sqlite3.connect(str(data_dir / "memory.db"))
    rows = conn.execute("SELECT * FROM memories").fetchall()
    conn.close()
    assert len(rows) == 1
    assert rows[0][2] == "test"


def test_save_and_load_agent_state_roundtrip(tmp_path: Path) -> None:
    """save_to_dir / load_from_dir round-trip preserves state."""
    from agent_runtime.__main__ import _save_agent_state_to_dir, _load_agent_state_from_dir
    from agent_runtime.models.agent_state import AgentState

    state = AgentState(name="RoundTrip", tokens=123, money=45.6, health=78.9)
    data_dir = tmp_path / "data" / "rt"
    data_dir.mkdir(parents=True)

    _save_agent_state_to_dir(data_dir, state)

    loaded = _load_agent_state_from_dir(data_dir)
    assert loaded is not None
    assert loaded.name == state.name
    assert loaded.tokens == state.tokens
    assert loaded.money == state.money
    assert loaded.health == state.health


def test_load_agent_state_missing_dir(tmp_path: Path) -> None:
    """Loading from a non-existent directory returns None."""
    from agent_runtime.__main__ import _load_agent_state_from_dir

    result = _load_agent_state_from_dir(tmp_path / "nonexistent")
    assert result is None


# ---------------------------------------------------------------------------
# Unit tests: pool.py _init_data_files
# ---------------------------------------------------------------------------


def test_pool_init_data_files(tmp_path: Path) -> None:
    """AgentProcessManager._init_data_files creates the three standard files."""
    from agent_runtime.pool import AgentProcessManager

    mgr = AgentProcessManager(base_dir=tmp_path)
    data_dir = tmp_path / "data" / "test_agent"
    data_dir.mkdir(parents=True)

    mgr._init_data_files(data_dir, "test_agent")

    assert (data_dir / "memory.db").exists()
    assert (data_dir / "skills.json").exists()
    assert (data_dir / "trace.db").exists()


def test_pool_init_data_files_idempotent(tmp_path: Path) -> None:
    """_init_data_files is safe to call twice (preserves existing data)."""
    from agent_runtime.pool import AgentProcessManager

    mgr = AgentProcessManager(base_dir=tmp_path)
    data_dir = tmp_path / "data" / "test_agent"
    data_dir.mkdir(parents=True)

    mgr._init_data_files(data_dir, "test_agent")

    # Insert data into memory.db
    conn = sqlite3.connect(str(data_dir / "memory.db"))
    conn.execute("INSERT INTO memories (tick, content, created_at) VALUES (1, 'hello', 0.0)")
    conn.commit()
    conn.close()

    # Re-init should not destroy data
    mgr._init_data_files(data_dir, "test_agent")

    conn = sqlite3.connect(str(data_dir / "memory.db"))
    rows = conn.execute("SELECT * FROM memories").fetchall()
    conn.close()
    assert len(rows) == 1


# ---------------------------------------------------------------------------
# Unit tests: pool.py _build_command includes --data-dir
# ---------------------------------------------------------------------------


def test_pool_build_command_includes_data_dir(tmp_path: Path) -> None:
    """_build_command includes --data-dir pointing to the agent's data directory."""
    from agent_runtime.pool import AgentProcessManager

    mgr = AgentProcessManager(base_dir=tmp_path)
    data_dir = tmp_path / "data" / "alice"

    cmd = mgr._build_command("alice", {}, data_dir)

    assert "--data-dir" in cmd
    idx = cmd.index("--data-dir")
    assert cmd[idx + 1] == str(data_dir)


# ---------------------------------------------------------------------------
# Integration: 2 agents with independent data directories
# ---------------------------------------------------------------------------


def test_two_agents_independent_data_dirs(tmp_path: Path) -> None:
    """Spawning 2 agents creates 2 independent data directories."""
    from agent_runtime.__main__ import _init_data_dir
    from agent_runtime.models.agent_state import AgentState
    from agent_runtime.models.skill import Skill

    # Create two agents with different data
    alice_state = AgentState(name="Alice", tokens=100)
    alice_state.add_skill(Skill(name="coding", level=5))

    bob_state = AgentState(name="Bob", tokens=200)
    bob_state.add_skill(Skill(name="trading", level=2))

    alice_dir = tmp_path / "data" / "alice"
    bob_dir = tmp_path / "data" / "bob"

    _init_data_dir(alice_dir, alice_state)
    _init_data_dir(bob_dir, bob_state)

    # Verify independent skills.json
    alice_skills = json.loads((alice_dir / "skills.json").read_text())
    bob_skills = json.loads((bob_dir / "skills.json").read_text())

    assert "coding" in alice_skills
    assert "coding" not in bob_skills
    assert "trading" in bob_skills
    assert "trading" not in alice_skills

    # Verify independent memory.db (each has its own file)
    assert alice_dir / "memory.db" != bob_dir / "memory.db"

    # Write to Alice's memory.db
    conn_a = sqlite3.connect(str(alice_dir / "memory.db"))
    conn_a.execute("INSERT INTO memories (tick, content, created_at) VALUES (1, 'alice-memory', 0.0)")
    conn_a.commit()
    conn_a.close()

    # Write to Bob's memory.db
    conn_b = sqlite3.connect(str(bob_dir / "memory.db"))
    conn_b.execute("INSERT INTO memories (tick, content, created_at) VALUES (1, 'bob-memory', 0.0)")
    conn_b.commit()
    conn_b.close()

    # Verify no cross-contamination
    conn_a = sqlite3.connect(str(alice_dir / "memory.db"))
    alice_rows = conn_a.execute("SELECT content FROM memories").fetchall()
    conn_a.close()
    assert len(alice_rows) == 1
    assert alice_rows[0][0] == "alice-memory"

    conn_b = sqlite3.connect(str(bob_dir / "memory.db"))
    bob_rows = conn_b.execute("SELECT content FROM memories").fetchall()
    conn_b.close()
    assert len(bob_rows) == 1
    assert bob_rows[0][0] == "bob-memory"


def test_kill_agent_preserves_others_data(tmp_path: Path) -> None:
    """Simulate killing agent A and verify agent B's data is intact."""
    from agent_runtime.__main__ import _init_data_dir, _save_agent_state_to_dir, _load_agent_state_from_dir
    from agent_runtime.models.agent_state import AgentState

    alice_state = AgentState(name="Alice", tokens=100)
    bob_state = AgentState(name="Bob", tokens=200)

    alice_dir = tmp_path / "data" / "alice"
    bob_dir = tmp_path / "data" / "bob"

    _init_data_dir(alice_dir, alice_state)
    _init_data_dir(bob_dir, bob_state)

    # Write data to both
    conn_a = sqlite3.connect(str(alice_dir / "memory.db"))
    conn_a.execute("INSERT INTO memories (tick, content, created_at) VALUES (1, 'alice-data', 0.0)")
    conn_a.commit()
    conn_a.close()

    conn_b = sqlite3.connect(str(bob_dir / "memory.db"))
    conn_b.execute("INSERT INTO memories (tick, content, created_at) VALUES (1, 'bob-data', 0.0)")
    conn_b.commit()
    conn_b.close()

    # "Kill" agent A: delete alice's data directory (simulating worst case)
    import shutil
    shutil.rmtree(alice_dir)

    # Verify agent B's data is still intact
    assert not alice_dir.exists()
    assert bob_dir.exists()

    conn_b = sqlite3.connect(str(bob_dir / "memory.db"))
    bob_rows = conn_b.execute("SELECT content FROM memories").fetchall()
    conn_b.close()
    assert len(bob_rows) == 1
    assert bob_rows[0][0] == "bob-data"

    loaded_bob = _load_agent_state_from_dir(bob_dir)
    assert loaded_bob is not None
    assert loaded_bob.name == "Bob"
    assert loaded_bob.tokens == 200


# ---------------------------------------------------------------------------
# CLI argument tests
# ---------------------------------------------------------------------------


def test_data_dir_cli_arg_parsed() -> None:
    """--data-dir is parsed correctly from CLI arguments."""
    from agent_runtime.__main__ import build_parser

    parser = build_parser()
    args = parser.parse_args(["spawn", "--name", "Test", "--data-dir", "/tmp/test_data"])
    assert args.data_dir == Path("/tmp/test_data")


def test_data_dir_falls_back_to_env_var(tmp_path: Path) -> None:
    """build_config_from_args falls back to AGENT_DATA_DIR env var."""
    from agent_runtime.__main__ import build_config_from_args, build_parser

    parser = build_parser()
    args = parser.parse_args(["spawn", "--name", "Test", "--no-llm"])

    env_dir = tmp_path / "env_data"
    env_dir.mkdir()

    with patch.dict(os.environ, {"AGENT_DATA_DIR": str(env_dir)}):
        config = build_config_from_args(args)
        assert config.data_dir == env_dir


def test_data_dir_in_pool_spawn_args() -> None:
    """_build_pool_spawn_args forwards --data-dir when set."""
    from agent_runtime.__main__ import build_parser, _build_pool_spawn_args

    parser = build_parser()
    args = parser.parse_args([
        "pool", "--count", "2",
        "--data-dir", "/tmp/pool_data",
        "--no-llm",
    ])

    spawn_args = _build_pool_spawn_args(args)
    assert "--data-dir" in spawn_args
    assert "/tmp/pool_data" in spawn_args


# ---------------------------------------------------------------------------
# RuntimeConfig data_dir field test
# ---------------------------------------------------------------------------


def test_runtime_config_has_data_dir() -> None:
    """RuntimeConfig has a data_dir field defaulting to None."""
    from agent_runtime.config import RuntimeConfig

    config = RuntimeConfig()
    assert config.data_dir is None

    config.data_dir = Path("/tmp/test")
    assert config.data_dir == Path("/tmp/test")
