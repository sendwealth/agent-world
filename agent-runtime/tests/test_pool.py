"""Unit tests for AgentProcessManager (pool.py).

All subprocess calls are mocked — no real processes are spawned.
"""

from __future__ import annotations

import signal
import subprocess
from pathlib import Path
from typing import Optional
from unittest.mock import MagicMock, patch

import pytest

from agent_runtime.pool import (
    _DEFAULT_CHECK_INTERVAL,
    AgentProcessManager,
    AgentStatus,
    AutoRestartPolicy,
    DeathEvent,
    ManagedProcess,
)

# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------


@pytest.fixture
def tmp_base(tmp_path: Path) -> Path:
    """Provide a clean temporary base directory."""
    return tmp_path / "pool_test"


@pytest.fixture
def manager(tmp_base: Path) -> AgentProcessManager:
    """Create an AgentProcessManager with a temp base directory."""
    return AgentProcessManager(
        base_dir=tmp_base,
        check_interval=0.1,
        python_executable="/usr/bin/echo",
    )


def _mock_popen(pid: int = 12345, returncode: Optional[int] = None) -> MagicMock:
    """Create a mock Popen object."""
    proc = MagicMock()
    proc.pid = pid
    proc.returncode = returncode
    proc.poll.return_value = returncode
    proc.wait.return_value = returncode
    return proc


# ---------------------------------------------------------------------------
# Initialisation
# ---------------------------------------------------------------------------


class TestInit:
    def test_creates_directories(self, tmp_base: Path) -> None:
        AgentProcessManager(base_dir=tmp_base)
        assert (tmp_base / "logs").is_dir()
        assert (tmp_base / "data").is_dir()

    def test_default_base_dir(self) -> None:
        mgr = AgentProcessManager()
        assert mgr._base_dir.name == ".agent_pool"

    def test_default_values(self, tmp_base: Path) -> None:
        mgr = AgentProcessManager(base_dir=tmp_base)
        assert mgr._check_interval == _DEFAULT_CHECK_INTERVAL
        assert mgr._auto_restart == AutoRestartPolicy.NEVER
        assert mgr._max_restart_count == 5


# ---------------------------------------------------------------------------
# spawn
# ---------------------------------------------------------------------------


class TestSpawn:
    @patch("agent_runtime.pool.subprocess.Popen")
    def test_spawn_creates_process(self, mock_popen_cls, manager: AgentProcessManager) -> None:
        mock_proc = _mock_popen()
        mock_popen_cls.return_value = mock_proc

        mp = manager.spawn("alice", {"world_url": "http://localhost:3000"})

        assert mp.name == "alice"
        assert mp.status == AgentStatus.RUNNING
        assert mp.process is mock_proc
        assert mp.started_at > 0
        mock_popen_cls.assert_called_once()

    @patch("agent_runtime.pool.subprocess.Popen")
    def test_spawn_creates_data_dir(self, mock_popen_cls, manager: AgentProcessManager) -> None:
        mock_popen_cls.return_value = _mock_popen()

        mp = manager.spawn("bob")

        assert mp.data_dir.is_dir()
        assert mp.data_dir.name == "bob"

    @patch("agent_runtime.pool.subprocess.Popen")
    def test_spawn_duplicate_name_raises(
        self, mock_popen_cls, manager: AgentProcessManager
    ) -> None:
        mock_popen_cls.return_value = _mock_popen()

        manager.spawn("alice")
        with pytest.raises(ValueError, match="already running"):
            manager.spawn("alice")

    @patch("agent_runtime.pool.subprocess.Popen")
    def test_spawn_after_kill_allows_respawn(
        self, mock_popen_cls, manager: AgentProcessManager
    ) -> None:
        mock_popen_cls.return_value = _mock_popen()

        manager.spawn("alice")
        # Simulate process death so status is no longer RUNNING
        manager._processes["alice"].status = AgentStatus.STOPPED
        manager._processes["alice"].process = None

        # Should not raise
        manager.spawn("alice")

    @patch("agent_runtime.pool.subprocess.Popen")
    def test_spawn_failure_raises_runtime_error(
        self, mock_popen_cls, manager: AgentProcessManager
    ) -> None:
        mock_popen_cls.side_effect = OSError("exec failed")

        with pytest.raises(RuntimeError, match="Failed to start agent"):
            manager.spawn("broken")

    @patch("agent_runtime.pool.subprocess.Popen")
    def test_spawn_builds_correct_command(
        self, mock_popen_cls, manager: AgentProcessManager
    ) -> None:
        mock_popen_cls.return_value = _mock_popen()

        manager.spawn("test-agent", {
            "world_url": "http://world:3000",
            "llm_provider": "ollama",
            "mock_llm": True,
            "max_ticks": 100,
            "health_port": 8080,
            "verbose": True,
        })

        args, kwargs = mock_popen_cls.call_args
        cmd = args[0]
        assert "--name" in cmd
        assert "test-agent" in cmd
        assert "--world-url" in cmd
        assert "http://world:3000" in cmd
        assert "--llm-provider" in cmd
        assert "--mock-llm" in cmd
        assert "--max-ticks" in cmd
        assert "100" in cmd
        assert "--health-port" in cmd
        assert "8080" in cmd
        assert "--verbose" in cmd

    @patch("agent_runtime.pool.subprocess.Popen")
    def test_spawn_sets_env_with_data_dir(
        self, mock_popen_cls, manager: AgentProcessManager
    ) -> None:
        mock_popen_cls.return_value = _mock_popen()

        manager.spawn("env-test")

        _, kwargs = mock_popen_cls.call_args
        env = kwargs["env"]
        assert "AGENT_DATA_DIR" in env
        assert "env-test" in env["AGENT_DATA_DIR"]


# ---------------------------------------------------------------------------
# kill
# ---------------------------------------------------------------------------


class TestKill:
    @patch("agent_runtime.pool.os.killpg")
    @patch("agent_runtime.pool.subprocess.Popen")
    def test_kill_sends_sigterm(
        self, mock_popen_cls, mock_killpg, manager: AgentProcessManager
    ) -> None:
        mock_proc = _mock_popen()
        mock_proc.wait.return_value = 0
        mock_proc.returncode = 0
        mock_popen_cls.return_value = mock_proc

        manager.spawn("alice")
        manager.kill("alice")

        mock_killpg.assert_called_once_with(mock_proc.pid, signal.SIGTERM)
        assert manager._processes["alice"].status == AgentStatus.STOPPED

    @patch("agent_runtime.pool.os.killpg")
    @patch("agent_runtime.pool.subprocess.Popen")
    def test_kill_unknown_raises_keyerror(
        self, mock_popen_cls, mock_killpg, manager: AgentProcessManager
    ) -> None:
        mock_popen_cls.return_value = _mock_popen()
        with pytest.raises(KeyError, match="not found"):
            manager.kill("nonexistent")

    @patch("agent_runtime.pool.os.killpg")
    @patch("agent_runtime.pool.subprocess.Popen")
    def test_kill_escalates_to_sigkill_on_timeout(
        self, mock_popen_cls, mock_killpg, manager: AgentProcessManager
    ) -> None:
        mock_proc = _mock_popen()
        # First wait (SIGTERM) times out, second wait (SIGKILL) succeeds
        mock_proc.wait.side_effect = [subprocess.TimeoutExpired(cmd="x", timeout=5), None]
        mock_proc.returncode = -9
        mock_popen_cls.return_value = mock_proc

        manager.spawn("stubborn")
        manager.kill("stubborn")

        # SIGTERM then SIGKILL
        assert mock_killpg.call_count == 2
        mock_killpg.assert_any_call(mock_proc.pid, signal.SIGTERM)
        mock_killpg.assert_any_call(mock_proc.pid, signal.SIGKILL)

    @patch("agent_runtime.pool.os.killpg")
    @patch("agent_runtime.pool.subprocess.Popen")
    def test_kill_handles_already_dead_process(
        self, mock_popen_cls, mock_killpg, manager: AgentProcessManager
    ) -> None:
        mock_proc = _mock_popen()
        mock_proc.wait.return_value = 0
        mock_proc.returncode = 0
        mock_popen_cls.return_value = mock_proc

        manager.spawn("dying")
        # Process is already dead
        mock_killpg.side_effect = ProcessLookupError("no such process")
        manager.kill("dying")

        assert manager._processes["dying"].status == AgentStatus.STOPPED


# ---------------------------------------------------------------------------
# restart
# ---------------------------------------------------------------------------


class TestRestart:
    @patch("agent_runtime.pool.os.killpg")
    @patch("agent_runtime.pool.subprocess.Popen")
    def test_restart_kill_and_respawn(
        self, mock_popen_cls, mock_killpg, manager: AgentProcessManager
    ) -> None:
        old_proc = _mock_popen(pid=100)
        new_proc = _mock_popen(pid=200)
        mock_popen_cls.side_effect = [old_proc, new_proc]

        manager.spawn("alice")
        mp = manager.restart("alice")

        assert mp.process is new_proc
        assert mp.restart_count == 1
        assert mp.status == AgentStatus.RUNNING
        # Old process was killed
        mock_killpg.assert_called_once_with(old_proc.pid, signal.SIGTERM)

    @patch("agent_runtime.pool.os.killpg")
    @patch("agent_runtime.pool.subprocess.Popen")
    def test_restart_preserves_data_dir(
        self, mock_popen_cls, mock_killpg, manager: AgentProcessManager
    ) -> None:
        mock_popen_cls.return_value = _mock_popen()
        mock_popen_cls.return_value.wait.return_value = 0
        mock_popen_cls.return_value.returncode = 0

        first = manager.spawn("alice")
        original_data_dir = first.data_dir

        # Need a new mock for the restart
        mock_popen_cls.return_value = _mock_popen(pid=999)
        mock_popen_cls.return_value.wait.return_value = 0
        mock_popen_cls.return_value.returncode = 0

        second = manager.restart("alice")
        assert second.data_dir == original_data_dir

    @patch("agent_runtime.pool.subprocess.Popen")
    def test_restart_unknown_raises_keyerror(
        self, mock_popen_cls, manager: AgentProcessManager
    ) -> None:
        with pytest.raises(KeyError, match="not found"):
            manager.restart("nonexistent")

    @patch("agent_runtime.pool.os.killpg")
    @patch("agent_runtime.pool.subprocess.Popen")
    def test_restart_increments_restart_count(
        self, mock_popen_cls, mock_killpg, manager: AgentProcessManager
    ) -> None:
        procs = [_mock_popen(pid=i) for i in range(5)]
        for p in procs:
            p.wait.return_value = 0
            p.returncode = 0
        mock_popen_cls.side_effect = procs

        manager.spawn("alice")
        _mp1 = manager.restart("alice")
        _mp2 = manager.restart("alice")
        mp3 = manager.restart("alice")

        assert mp3.restart_count == 3


# ---------------------------------------------------------------------------
# health_check
# ---------------------------------------------------------------------------


class TestHealthCheck:
    @patch("agent_runtime.pool.subprocess.Popen")
    def test_healthy_process(self, mock_popen_cls, manager: AgentProcessManager) -> None:
        mock_proc = _mock_popen(returncode=None)
        mock_proc.poll.return_value = None
        mock_popen_cls.return_value = mock_proc

        manager.spawn("healthy")
        deaths = manager.health_check()

        assert deaths == []
        assert manager._processes["healthy"].status == AgentStatus.RUNNING

    @patch("agent_runtime.pool.subprocess.Popen")
    def test_detects_dead_process(self, mock_popen_cls, manager: AgentProcessManager) -> None:
        mock_proc = _mock_popen(returncode=None)
        mock_proc.poll.return_value = None
        mock_popen_cls.return_value = mock_proc

        manager.spawn("doomed")

        # Now simulate death
        mock_proc.poll.return_value = 1
        deaths = manager.health_check()

        assert len(deaths) == 1
        assert deaths[0].agent_name == "doomed"
        assert deaths[0].exit_code == 1
        assert manager._processes["doomed"].status == AgentStatus.FAILED
        assert manager._processes["doomed"].exit_code == 1

    @patch("agent_runtime.pool.subprocess.Popen")
    def test_death_log_accumulates(self, mock_popen_cls, manager: AgentProcessManager) -> None:
        mock_proc = _mock_popen(returncode=None)
        mock_proc.poll.return_value = None
        mock_popen_cls.return_value = mock_proc

        manager.spawn("a1")
        mock_proc.poll.return_value = 0
        deaths1 = manager.health_check()
        assert len(deaths1) == 1

        # Spawn second
        mock_popen_cls.return_value = _mock_popen()
        mock_popen_cls.return_value.poll.return_value = None
        manager.spawn("a2")
        mock_popen_cls.return_value.poll.return_value = 137
        deaths2 = manager.health_check()
        assert len(deaths2) == 1

        assert len(manager.death_log) == 2

    @patch("agent_runtime.pool.subprocess.Popen")
    def test_skips_non_running_processes(
        self, mock_popen_cls, manager: AgentProcessManager
    ) -> None:
        mock_popen_cls.return_value = _mock_popen()

        manager.spawn("stopped")
        manager._processes["stopped"].status = AgentStatus.STOPPED

        deaths = manager.health_check()
        assert deaths == []


# ---------------------------------------------------------------------------
# run / shutdown / signal handling
# ---------------------------------------------------------------------------


class TestRunLoop:
    @patch("agent_runtime.pool.subprocess.Popen")
    def test_run_exits_on_shutdown(self, mock_popen_cls, manager: AgentProcessManager) -> None:
        mock_popen_cls.return_value = _mock_popen()

        # Trigger shutdown immediately
        def immediate_shutdown(*args, **kwargs):
            manager._shutdown_event.set()
            return MagicMock()

        with patch.object(manager, "health_check", side_effect=immediate_shutdown):
            manager.run(signals=set())  # no signal handlers in test

    @patch("agent_runtime.pool.subprocess.Popen")
    def test_run_calls_shutdown_all(
        self, mock_popen_cls, manager: AgentProcessManager
    ) -> None:
        mock_popen_cls.return_value = _mock_popen()

        with patch.object(manager, "_shutdown_all") as mock_shutdown_all:
            manager._shutdown_event.set()
            manager.run(signals=set())
            mock_shutdown_all.assert_called_once()

    @patch("agent_runtime.pool.os.killpg")
    @patch("agent_runtime.pool.subprocess.Popen")
    def test_shutdown_all_kills_running_processes(
        self, mock_popen_cls, mock_killpg, manager: AgentProcessManager
    ) -> None:
        mock_proc = _mock_popen()
        mock_proc.wait.return_value = 0
        mock_proc.returncode = 0
        mock_popen_cls.return_value = mock_proc

        manager.spawn("a1")
        manager.spawn("a2")

        manager._shutdown_all()

        assert mock_killpg.call_count == 2

    def test_shutdown_sets_event(self, manager: AgentProcessManager) -> None:
        assert not manager._shutdown_event.is_set()
        manager.shutdown()
        assert manager._shutdown_event.is_set()


# ---------------------------------------------------------------------------
# Auto-restart
# ---------------------------------------------------------------------------


class TestAutoRestart:
    @patch("agent_runtime.pool.os.killpg")
    @patch("agent_runtime.pool.subprocess.Popen")
    def test_never_policy_no_restart(
        self, mock_popen_cls, mock_killpg, tmp_base: Path
    ) -> None:
        mgr = AgentProcessManager(
            base_dir=tmp_base, auto_restart=AutoRestartPolicy.NEVER,
        )
        mock_proc = _mock_popen()
        mock_proc.poll.return_value = 1
        mock_popen_cls.return_value = mock_proc

        mgr.spawn("doomed")
        deaths = mgr.health_check()

        # Even though ON_FAILURE is the logic, NEVER means no restart
        mgr._handle_auto_restart(deaths)
        assert mgr._processes["doomed"].status == AgentStatus.FAILED

    @patch("agent_runtime.pool.os.killpg")
    @patch("agent_runtime.pool.subprocess.Popen")
    def test_on_failure_restarts_nonzero_exit(
        self, mock_popen_cls, mock_killpg, tmp_base: Path
    ) -> None:
        mgr = AgentProcessManager(
            base_dir=tmp_base, auto_restart=AutoRestartPolicy.ON_FAILURE,
        )
        old_proc = _mock_popen()
        old_proc.poll.return_value = 1
        old_proc.wait.return_value = 0
        old_proc.returncode = 0
        new_proc = _mock_popen(pid=999)
        mock_popen_cls.side_effect = [old_proc, new_proc]

        mgr.spawn("crashy")
        deaths = mgr.health_check()

        mgr._handle_auto_restart(deaths)
        assert mgr._processes["crashy"].status == AgentStatus.RUNNING
        assert mgr._processes["crashy"].restart_count == 1

    @patch("agent_runtime.pool.subprocess.Popen")
    def test_on_failure_ignores_zero_exit(
        self, mock_popen_cls, tmp_base: Path
    ) -> None:
        mgr = AgentProcessManager(
            base_dir=tmp_base, auto_restart=AutoRestartPolicy.ON_FAILURE,
        )
        mock_proc = _mock_popen()
        mock_proc.poll.return_value = 0
        mock_popen_cls.return_value = mock_proc

        mgr.spawn("clean_exit")
        deaths = mgr.health_check()

        mgr._handle_auto_restart(deaths)
        # No restart — process exited cleanly
        assert mgr._processes["clean_exit"].status == AgentStatus.FAILED
        assert mgr._processes["clean_exit"].restart_count == 0

    @patch("agent_runtime.pool.os.killpg")
    @patch("agent_runtime.pool.subprocess.Popen")
    def test_always_restarts_on_zero_exit(
        self, mock_popen_cls, mock_killpg, tmp_base: Path
    ) -> None:
        mgr = AgentProcessManager(
            base_dir=tmp_base, auto_restart=AutoRestartPolicy.ALWAYS,
        )
        old_proc = _mock_popen()
        old_proc.poll.return_value = 0
        old_proc.wait.return_value = 0
        old_proc.returncode = 0
        new_proc = _mock_popen(pid=888)
        mock_popen_cls.side_effect = [old_proc, new_proc]

        mgr.spawn("graceful")
        deaths = mgr.health_check()

        mgr._handle_auto_restart(deaths)
        assert mgr._processes["graceful"].restart_count == 1

    @patch("agent_runtime.pool.os.killpg")
    @patch("agent_runtime.pool.subprocess.Popen")
    def test_max_restart_count_respected(
        self, mock_popen_cls, mock_killpg, tmp_base: Path
    ) -> None:
        mgr = AgentProcessManager(
            base_dir=tmp_base,
            auto_restart=AutoRestartPolicy.ALWAYS,
            max_restart_count=2,
        )

        # Create enough mock procs for initial spawn + 2 restarts + 1 more death
        procs = [_mock_popen(pid=i) for i in range(5)]
        for p in procs:
            p.poll.return_value = 0
            p.wait.return_value = 0
            p.returncode = 0
        mock_popen_cls.side_effect = procs

        mgr.spawn("flaky")
        mgr._processes["flaky"].process.poll.return_value = 0
        deaths1 = mgr.health_check()
        mgr._handle_auto_restart(deaths1)  # restart 1

        mgr._processes["flaky"].process.poll.return_value = 0
        deaths2 = mgr.health_check()
        mgr._handle_auto_restart(deaths2)  # restart 2

        # Third death — should NOT restart (exceeded max)
        mgr._processes["flaky"].process.poll.return_value = 0
        deaths3 = mgr.health_check()
        mgr._handle_auto_restart(deaths3)

        assert mgr._processes["flaky"].restart_count == 2
        assert mgr._processes["flaky"].status == AgentStatus.FAILED


# ---------------------------------------------------------------------------
# _build_command
# ---------------------------------------------------------------------------


class TestBuildCommand:
    def test_basic_command(self, manager: AgentProcessManager) -> None:
        cmd = manager._build_command("test", {}, Path("/data/test"))
        assert cmd == [
            "/usr/bin/echo", "-m", "agent_runtime", "spawn",
            "--name", "test",
            "--data-dir", "/data/test",
        ]

    def test_command_with_config_file(self, manager: AgentProcessManager) -> None:
        cmd = manager._build_command(
            "test", {"config": "/etc/agent.toml"}, Path("/data/test"),
        )
        assert "--config" in cmd
        assert "/etc/agent.toml" in cmd

    def test_command_skips_empty_config(self, manager: AgentProcessManager) -> None:
        cmd = manager._build_command("test", {"config": None}, Path("/data/test"))
        assert "--config" not in cmd


# ---------------------------------------------------------------------------
# Data classes
# ---------------------------------------------------------------------------


class TestDataClasses:
    def test_death_event(self) -> None:
        event = DeathEvent(agent_name="a", exit_code=1, timestamp=1.0)
        assert event.agent_name == "a"
        assert event.exit_code == 1

    def test_managed_process_defaults(self) -> None:
        mp = ManagedProcess(name="x", config={})
        assert mp.status == AgentStatus.STOPPED
        assert mp.restart_count == 0
        assert mp.process is None

    def test_agent_status_values(self) -> None:
        assert AgentStatus.RUNNING.value == "running"
        assert AgentStatus.FAILED.value == "failed"
        assert AgentStatus.STOPPED.value == "stopped"

    def test_auto_restart_policy_values(self) -> None:
        assert AutoRestartPolicy.NEVER.value == "never"
        assert AutoRestartPolicy.ON_FAILURE.value == "on_failure"
        assert AutoRestartPolicy.ALWAYS.value == "always"


# ---------------------------------------------------------------------------
# File handle leak regression tests
# ---------------------------------------------------------------------------


class TestFileHandleLeak:
    @patch("agent_runtime.pool.os.killpg")
    @patch("agent_runtime.pool.subprocess.Popen")
    def test_no_file_handle_leak_after_10_restarts(
        self, mock_popen_cls, mock_killpg, manager: AgentProcessManager
    ) -> None:
        """Verify that restarting an agent 10 times does not leak file handles.

        Each restart creates a new log file handle in _start_process().
        _kill_locked() must close the previous handle. If it doesn't,
        the number of open file descriptors will grow with each restart.
        """
        # Build 11 mock procs (1 spawn + 10 restarts) with distinct log files
        procs = [_mock_popen(pid=i) for i in range(11)]
        for p in procs:
            p.wait.return_value = 0
            p.returncode = 0
        mock_popen_cls.side_effect = procs

        # Collect all log file handles that _start_process creates
        log_handles: list = []

        original_open = open

        def _tracking_open(path, *args, **kwargs):
            fh = original_open(path, *args, **kwargs)
            log_handles.append(fh)
            return fh

        with patch("agent_runtime.pool.open", side_effect=_tracking_open):
            mp = manager.spawn("leaky")

            for i in range(10):
                mp = manager.restart("leaky")

        # We should have 11 log handles total (1 spawn + 10 restarts)
        assert len(log_handles) == 11

        # All handles except the last should be closed by _kill_locked
        closed_count = sum(1 for fh in log_handles[:-1] if fh.closed)
        assert closed_count == 10, (
            f"Expected 10 closed file handles, got {closed_count}"
        )

        # The last handle should still be open (current process)
        assert not log_handles[-1].closed, "Current log file should still be open"

        # Cleanup
        for fh in log_handles:
            if not fh.closed:
                fh.close()

    @patch("agent_runtime.pool.os.killpg")
    @patch("agent_runtime.pool.subprocess.Popen")
    def test_kill_closes_log_file_handle(
        self, mock_popen_cls, mock_killpg, manager: AgentProcessManager
    ) -> None:
        """Verify that kill() closes the associated log file handle."""
        mock_proc = _mock_popen()
        mock_proc.wait.return_value = 0
        mock_proc.returncode = 0
        mock_popen_cls.return_value = mock_proc

        log_handles: list = []
        original_open = open

        def _tracking_open(path, *args, **kwargs):
            fh = original_open(path, *args, **kwargs)
            log_handles.append(fh)
            return fh

        with patch("agent_runtime.pool.open", side_effect=_tracking_open):
            manager.spawn("test-agent")

        assert len(log_handles) == 1
        assert not log_handles[0].closed

        manager.kill("test-agent")

        assert log_handles[0].closed, "Log file should be closed after kill()"

