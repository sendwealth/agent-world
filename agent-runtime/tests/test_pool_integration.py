"""Integration tests for AgentProcessManager (pool.py).

These tests spawn REAL subprocesses using mock scripts (sleep, echo, etc.)
to validate process isolation, lifecycle, and log redirection end-to-end.

No mock subprocess objects are used — only real OS processes.
"""

from __future__ import annotations

import os
import signal
import sys
import time
from pathlib import Path
from typing import Optional

import pytest

from agent_runtime.pool import (
    AgentProcessManager,
    AgentStatus,
    AutoRestartPolicy,
    _GRACEFUL_SHUTDOWN_TIMEOUT,
)


# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------


@pytest.fixture
def tmp_base(tmp_path: Path) -> Path:
    return tmp_path / "pool_integration"


@pytest.fixture
def manager(tmp_base: Path) -> AgentProcessManager:
    """Create a manager that uses the current Python interpreter.

    We use a mock script that just sleeps (simulating a long-running agent).
    """
    return AgentProcessManager(
        base_dir=tmp_base,
        check_interval=0.2,
        python_executable=sys.executable,
    )


@pytest.fixture
def mock_script(tmp_path: Path) -> Path:
    """Write a simple mock agent script that sleeps for a configurable duration."""
    script = tmp_path / "mock_agent.py"
    script.write_text(
        "import sys, time, signal\n"
        "signal.signal(signal.SIGTERM, lambda *_: sys.exit(0))\n"
        "duration = int(sys.argv[1]) if len(sys.argv) > 1 else 60\n"
        "print(f'MOCK_AGENT started, will run for {duration}s', flush=True)\n"
        "try:\n"
        "    time.sleep(duration)\n"
        "except KeyboardInterrupt:\n"
        "    pass\n"
        "print('MOCK_AGENT exited normally', flush=True)\n"
    )
    return script


# ---------------------------------------------------------------------------
# Integration: spawn real processes
# ---------------------------------------------------------------------------


class TestSpawnRealProcesses:
    def test_spawn_3_agents_and_verify_isolation(
        self, manager: AgentProcessManager, mock_script: Path, tmp_base: Path
    ) -> None:
        """Spawn 3 independent agent processes, verify each has its own
        data directory and log file, and that processes are running."""
        # We spawn using sys.executable + mock_script instead of agent_runtime
        # Patch _build_command to use our mock script
        original_build = manager._build_command

        def mock_build(name, config, data_dir):
            return [sys.executable, str(mock_script), "60"]

        manager._build_command = mock_build

        names = ["agent-alpha", "agent-beta", "agent-gamma"]
        for name in names:
            mp = manager.spawn(name)
            assert mp.status == AgentStatus.RUNNING
            assert mp.process is not None
            assert mp.process.poll() is None  # still running

        # Verify data directories
        for name in names:
            data_dir = tmp_base / "data" / name
            assert data_dir.is_dir(), f"Data dir for {name} missing"

        # Verify log files exist
        for name in names:
            log_file = tmp_base / "logs" / f"{name}.log"
            assert log_file.is_file(), f"Log file for {name} missing"

        # Verify processes are independent (different PIDs)
        pids = [manager._processes[n].process.pid for n in names]
        assert len(set(pids)) == 3, f"PIDs not unique: {pids}"

        # Health check — all should be alive
        deaths = manager.health_check()
        assert deaths == [], f"Unexpected deaths: {deaths}"

        # Cleanup
        for name in names:
            manager.kill(name)

        # Verify all stopped
        for name in names:
            assert manager._processes[name].status == AgentStatus.STOPPED

    def test_spawn_logs_stdout_to_file(
        self, manager: AgentProcessManager, mock_script: Path, tmp_base: Path
    ) -> None:
        """Verify that agent stdout is redirected to the log file."""
        def mock_build(name, config, data_dir):
            return [sys.executable, str(mock_script), "5"]

        manager._build_command = mock_build

        manager.spawn("logger")

        # Wait for the process to write output
        time.sleep(0.5)

        log_file = tmp_base / "logs" / "logger.log"
        assert log_file.exists()
        content = log_file.read_text()
        assert "MOCK_AGENT started" in content

        manager.kill("logger")


# ---------------------------------------------------------------------------
# Integration: kill with real processes
# ---------------------------------------------------------------------------


class TestKillRealProcesses:
    def test_kill_terminates_gracefully(
        self, manager: AgentProcessManager, mock_script: Path
    ) -> None:
        """Kill a running agent — it should exit and status should be STOPPED."""
        def mock_build(name, config, data_dir):
            return [sys.executable, str(mock_script), "60"]

        manager._build_command = mock_build
        manager.spawn("target")

        # Verify running
        assert manager._processes["target"].process.poll() is None

        manager.kill("target")

        # Verify stopped — exit code may be 0 (clean) or -15 (SIGTERM on group)
        assert manager._processes["target"].status == AgentStatus.STOPPED
        assert manager._processes["target"].exit_code is not None

    def test_kill_removes_process_from_os(
        self, manager: AgentProcessManager, mock_script: Path
    ) -> None:
        """After kill, the OS process should no longer exist."""
        def mock_build(name, config, data_dir):
            return [sys.executable, str(mock_script), "60"]

        manager._build_command = mock_build
        manager.spawn("target")
        pid = manager._processes["target"].process.pid

        manager.kill("target")

        # Process should be gone
        import subprocess
        result = subprocess.run(
            ["kill", "-0", str(pid)], capture_output=True,
        )
        assert result.returncode != 0, f"Process {pid} still alive after kill"


# ---------------------------------------------------------------------------
# Integration: restart with real processes
# ---------------------------------------------------------------------------


class TestRestartRealProcesses:
    def test_restart_preserves_data_dir(
        self, manager: AgentProcessManager, mock_script: Path, tmp_base: Path
    ) -> None:
        """Restart should kill old process, start new one, keep data dir."""
        def mock_build(name, config, data_dir):
            return [sys.executable, str(mock_script), "60"]

        manager._build_command = mock_build

        mp1 = manager.spawn("resilient")
        original_data_dir = mp1.data_dir
        old_pid = mp1.process.pid

        # Write a file to data dir to prove preservation
        test_file = original_data_dir / "state.json"
        test_file.write_text('{"tick": 42}')

        mp2 = manager.restart("resilient")

        # Data dir preserved
        assert mp2.data_dir == original_data_dir
        assert test_file.exists()
        assert test_file.read_text() == '{"tick": 42}'

        # New process started (different PID)
        assert mp2.process.pid != old_pid
        assert mp2.restart_count == 1
        assert mp2.status == AgentStatus.RUNNING

        # Old process is dead
        import subprocess
        result = subprocess.run(
            ["kill", "-0", str(old_pid)], capture_output=True,
        )
        assert result.returncode != 0

        manager.kill("resilient")


# ---------------------------------------------------------------------------
# Integration: health_check with real processes
# ---------------------------------------------------------------------------


class TestHealthCheckRealProcesses:
    def test_detects_real_process_death(
        self, manager: AgentProcessManager, tmp_base: Path
    ) -> None:
        """Spawn a process that exits quickly, verify health_check detects it."""
        # Use a script that exits immediately
        short_script = tmp_base / "short_agent.py"
        short_script.write_text(
            "import sys; print('quick exit', flush=True); sys.exit(42)\n"
        )

        def mock_build(name, config, data_dir):
            return [sys.executable, str(short_script)]

        manager._build_command = mock_build

        manager.spawn("short-lived")

        # Wait for the process to exit
        time.sleep(0.5)

        deaths = manager.health_check()
        assert len(deaths) == 1
        assert deaths[0].agent_name == "short-lived"
        assert deaths[0].exit_code == 42
        assert manager._processes["short-lived"].status == AgentStatus.FAILED


# ---------------------------------------------------------------------------
# Integration: shutdown_all with real processes
# ---------------------------------------------------------------------------


class TestShutdownAll:
    def test_shutdown_all_kills_multiple_processes(
        self, manager: AgentProcessManager, mock_script: Path
    ) -> None:
        """_shutdown_all should terminate all running agent processes."""
        def mock_build(name, config, data_dir):
            return [sys.executable, str(mock_script), "60"]

        manager._build_command = mock_build

        manager.spawn("a1")
        manager.spawn("a2")
        manager.spawn("a3")

        # All running
        for name in ("a1", "a2", "a3"):
            assert manager._processes[name].process.poll() is None

        manager._shutdown_all()

        for name in ("a1", "a2", "a3"):
            assert manager._processes[name].status == AgentStatus.STOPPED

    def test_shutdown_method_stops_run_loop(
        self, manager: AgentProcessManager, mock_script: Path
    ) -> None:
        """Calling shutdown() should cause run() to exit."""
        import threading

        def mock_build(name, config, data_dir):
            return [sys.executable, str(mock_script), "60"]

        manager._build_command = mock_build
        manager.spawn("loop-agent")

        # Run in a thread, trigger shutdown shortly after
        run_thread = threading.Thread(
            target=manager.run, kwargs={"signals": set()},
        )
        run_thread.start()

        time.sleep(0.3)
        manager.shutdown()
        run_thread.join(timeout=3.0)

        assert not run_thread.is_alive()


# ---------------------------------------------------------------------------
# Integration: auto-restart with real processes
# ---------------------------------------------------------------------------


class TestAutoRestartReal:
    def test_auto_restart_on_failure(
        self, mock_script: Path, tmp_base: Path
    ) -> None:
        """Auto-restart ON_FAILURE should respawn a crashed agent."""
        mgr = AgentProcessManager(
            base_dir=tmp_base,
            check_interval=0.2,
            auto_restart=AutoRestartPolicy.ON_FAILURE,
            python_executable=sys.executable,
        )

        crash_script = tmp_base / "crash_agent.py"
        crash_script.write_text(
            "import sys; print('crash', flush=True); sys.exit(1)\n"
        )

        def mock_build(name, config, data_dir):
            return [sys.executable, str(crash_script)]

        mgr._build_command = mock_build

        mgr.spawn("crashy")

        # Wait for crash + auto-restart cycle
        time.sleep(1.0)
        mgr.health_check()
        mgr._handle_auto_restart(mgr.death_log)

        # Should have been restarted
        assert mgr._processes["crashy"].restart_count >= 1

        mgr._shutdown_all()
