"""Agent Process Manager — subprocess-based multi-agent lifecycle management.

Manages a pool of agent processes, each running as an independent subprocess
with its own data directory and log file.

Usage::

    manager = AgentProcessManager(base_dir="/tmp/agent-world")

    # Spawn agents
    manager.spawn("alice", {"config": "config/agents/agent-01.toml"})
    manager.spawn("bob", {"config": "config/agents/agent-02.toml"})

    # Lifecycle
    manager.health_check()          # check all processes
    manager.restart("alice")        # kill + respawn, data preserved
    manager.kill("bob")             # graceful termination

    # Main loop (blocking)
    manager.run()                   # periodic health checks + optional auto-restart

    # Or run with a signal handler
    manager.run(signals={signal.SIGTERM})
"""

from __future__ import annotations

import logging
import os
import signal
import subprocess
import sys
import threading
import time
from dataclasses import dataclass, field
from enum import Enum
from pathlib import Path
from typing import Any

logger = logging.getLogger(__name__)

# Seconds to wait after SIGTERM before escalating to SIGKILL
_GRACEFUL_SHUTDOWN_TIMEOUT = 5

# Default interval (seconds) between health-check sweeps in run()
_DEFAULT_CHECK_INTERVAL = 10


class AutoRestartPolicy(Enum):
    """Controls auto-restart behaviour when a managed process dies."""

    NEVER = "never"
    ON_FAILURE = "on_failure"
    ALWAYS = "always"


class AgentStatus(Enum):
    """Lifecycle status for a managed agent process."""

    STARTING = "starting"
    RUNNING = "running"
    STOPPING = "stopping"
    STOPPED = "stopped"
    FAILED = "failed"


@dataclass
class ManagedProcess:
    """Tracks the state of a single managed agent process."""

    name: str
    config: dict[str, Any]
    process: subprocess.Popen | None = None
    data_dir: Path = field(default_factory=Path)
    status: AgentStatus = AgentStatus.STOPPED
    exit_code: int | None = None
    restart_count: int = 0
    last_health_check: float = 0.0
    started_at: float = 0.0
    log_file: Any = None


@dataclass
class DeathEvent:
    """Record of a process death detected during health check."""

    agent_name: str
    exit_code: int | None
    timestamp: float


class AgentProcessManager:
    """Manages a pool of agent subprocesses.

    Each agent runs as ``python -m agent_runtime spawn --name <name> ...``
    inside its own subprocess, with stdout/stderr redirected to a per-agent
    log file under ``<base_dir>/logs/<name>.log``.  Data directories are
    isolated under ``<base_dir>/data/<name>/``.

    Thread-safety: the manager uses an internal lock to serialize mutations
    (spawn/kill/restart).  The main ``run()`` loop is blocking and should be
    called from the main thread (it installs signal handlers).
    """

    def __init__(
        self,
        base_dir: str | Path | None = None,
        check_interval: float = _DEFAULT_CHECK_INTERVAL,
        auto_restart: AutoRestartPolicy = AutoRestartPolicy.NEVER,
        python_executable: str | None = None,
        max_restart_count: int = 5,
    ) -> None:
        self._base_dir = Path(base_dir) if base_dir else Path.cwd() / ".agent_pool"
        self._check_interval = check_interval
        self._auto_restart = auto_restart
        self._python = python_executable or sys.executable
        self._max_restart_count = max_restart_count

        self._processes: dict[str, ManagedProcess] = {}
        self._death_log: list[DeathEvent] = []
        self._lock = threading.Lock()
        self._shutdown_event = threading.Event()

        (self._base_dir / "logs").mkdir(parents=True, exist_ok=True)
        (self._base_dir / "data").mkdir(parents=True, exist_ok=True)

        logger.info(
            "AgentProcessManager initialised  base_dir=%s  check_interval=%s"
            "  auto_restart=%s",
            self._base_dir, self._check_interval, self._auto_restart.value,
        )

    # ------------------------------------------------------------------
    # Public API
    # ------------------------------------------------------------------

    @property
    def processes(self) -> dict[str, ManagedProcess]:
        """Snapshot of all managed processes (shallow copy)."""
        with self._lock:
            return dict(self._processes)

    @property
    def death_log(self) -> list[DeathEvent]:
        """Return a copy of the death event log."""
        with self._lock:
            return list(self._death_log)

    def spawn(self, name: str, config: dict[str, Any] | None = None) -> ManagedProcess:
        """Start an agent subprocess.

        Args:
            name: Unique identifier for this agent (used as directory and log name).
            config: Arbitrary configuration forwarded to the agent process.
                    Supports keys like ``config``, ``world_url``, ``llm_provider``,
                    ``mock_llm``, ``max_ticks``, etc.

        Returns:
            ManagedProcess tracking object.

        Raises:
            ValueError: If an agent with this name is already running.
            RuntimeError: If the subprocess fails to start.
        """
        config = config or {}

        with self._lock:
            if name in self._processes:
                existing = self._processes[name]
                if existing.status in (AgentStatus.RUNNING, AgentStatus.STARTING):
                    raise ValueError(
                        f"Agent {name!r} is already running"
                        f" (status={existing.status.value})"
                    )

            data_dir = self._base_dir / "data" / name
            data_dir.mkdir(parents=True, exist_ok=True)
            log_path = self._base_dir / "logs" / f"{name}.log"

            # Initialize data files for the agent
            self._init_data_files(data_dir, name)

            proc, log_fh = self._start_process(name, config, data_dir, log_path)

            mp = ManagedProcess(
                name=name,
                config=config,
                process=proc,
                data_dir=data_dir,
                status=AgentStatus.RUNNING,
                started_at=time.monotonic(),
                last_health_check=time.monotonic(),
                log_file=log_fh,
            )
            self._processes[name] = mp

            logger.info(
                "Spawned agent %s  pid=%d  data_dir=%s  log=%s",
                name, proc.pid, data_dir, log_path,
            )
            return mp

    def kill(self, name: str) -> None:
        """Gracefully terminate an agent process.

        Sends SIGTERM, waits up to 5 seconds, then escalates to SIGKILL.

        Raises:
            KeyError: If the agent name is not known.
        """
        with self._lock:
            mp = self._processes.get(name)
            if mp is None:
                raise KeyError(f"Agent {name!r} not found")
            self._kill_locked(mp)

    def restart(self, name: str) -> ManagedProcess:
        """Kill and respawn an agent, preserving its data directory.

        Raises:
            KeyError: If the agent name is not known.
        """
        with self._lock:
            mp = self._processes.get(name)
            if mp is None:
                raise KeyError(f"Agent {name!r} not found")

            saved_config = mp.config
            saved_restart_count = mp.restart_count
            data_dir = mp.data_dir

            self._kill_locked(mp)

            log_path = self._base_dir / "logs" / f"{name}.log"
            proc, log_fh = self._start_process(name, saved_config, data_dir, log_path)

            new_mp = ManagedProcess(
                name=name,
                config=saved_config,
                process=proc,
                data_dir=data_dir,
                status=AgentStatus.RUNNING,
                restart_count=saved_restart_count + 1,
                started_at=time.monotonic(),
                last_health_check=time.monotonic(),
                log_file=log_fh,
            )
            self._processes[name] = new_mp

            logger.info(
                "Restarted agent %s  pid=%d  restart_count=%d",
                name, proc.pid, new_mp.restart_count,
            )
            return new_mp

    def health_check(self) -> list[DeathEvent]:
        """Check all managed processes and record any that have died.

        Returns:
            List of DeathEvent for any processes that died since last check.
        """
        deaths: list[DeathEvent] = []
        now = time.monotonic()

        with self._lock:
            for name, mp in list(self._processes.items()):
                if mp.status != AgentStatus.RUNNING or mp.process is None:
                    continue

                returncode = mp.process.poll()
                mp.last_health_check = now

                if returncode is not None:
                    mp.status = AgentStatus.FAILED
                    mp.exit_code = returncode
                    event = DeathEvent(
                        agent_name=name,
                        exit_code=returncode,
                        timestamp=now,
                    )
                    self._death_log.append(event)
                    deaths.append(event)
                    logger.warning(
                        "Agent %s died  exit_code=%s  pid=%d",
                        name, returncode, mp.process.pid,
                    )

        if deaths:
            logger.info("Health check: %d dead process(es) detected", len(deaths))

        return deaths

    def run(self, signals: set[int] | None = None) -> None:
        """Main loop: periodic health checks with optional auto-restart.

        Blocks until a shutdown signal is received or ``shutdown()`` is called.
        Installs signal handlers for SIGTERM (and any additional signals
        provided) that trigger graceful shutdown of all managed processes.

        Args:
            signals: OS signals that should trigger graceful shutdown.
                     Defaults to ``{signal.SIGTERM}`` if not provided.
        """
        if signals is None:
            signals = {signal.SIGTERM}

        original_handlers: dict[int, Any] = {}

        def _handle_shutdown(signum: int, frame: Any) -> None:
            logger.info("Received signal %d — initiating graceful shutdown", signum)
            self._shutdown_event.set()

        for sig in signals:
            original_handlers[sig] = signal.signal(sig, _handle_shutdown)

        try:
            logger.info(
                "Main loop started  check_interval=%ss  auto_restart=%s",
                self._check_interval, self._auto_restart.value,
            )
            while not self._shutdown_event.is_set():
                deaths = self.health_check()

                if deaths and self._auto_restart != AutoRestartPolicy.NEVER:
                    self._handle_auto_restart(deaths)

                self._shutdown_event.wait(self._check_interval)

        finally:
            for sig, handler in original_handlers.items():
                signal.signal(sig, handler)

            self._shutdown_all()
            logger.info("Main loop exited")

    def shutdown(self) -> None:
        """Signal the main loop to stop and gracefully terminate all processes."""
        self._shutdown_event.set()

    # ------------------------------------------------------------------
    # Internal helpers
    # ------------------------------------------------------------------

    def _build_command(
        self, name: str, config: dict[str, Any], data_dir: Path,
    ) -> list[str]:
        """Build the subprocess command for spawning an agent."""
        cmd = [
            self._python, "-m", "agent_runtime", "spawn",
            "--name", name,
            "--data-dir", str(data_dir),
        ]

        if config.get("config"):
            cmd.extend(["--config", str(config["config"])])
        if config.get("world_url"):
            cmd.extend(["--world-url", str(config["world_url"])])
        if config.get("llm_provider"):
            cmd.extend(["--llm-provider", str(config["llm_provider"])])
        if config.get("mock_llm"):
            cmd.append("--mock-llm")
        if config.get("max_ticks"):
            cmd.extend(["--max-ticks", str(config["max_ticks"])])
        if config.get("health_port"):
            cmd.extend(["--health-port", str(config["health_port"])])
        if config.get("verbose"):
            cmd.append("--verbose")

        return cmd

    def _get_env(self, data_dir: Path) -> dict[str, str]:
        """Build the environment dict for a subprocess."""
        return {**os.environ, "AGENT_DATA_DIR": str(data_dir)}

    def _init_data_files(self, data_dir: Path, agent_name: str) -> None:
        """Seed the standard data files for an agent if they don't exist yet.

        Creates:
          - ``memory.db``  : empty SQLite database for agent memories
          - ``skills.json``: empty JSON object (populated by subprocess)
          - ``trace.db``   : empty SQLite database for tick traces

        Idempotent — skips files that already exist.
        """
        # memory.db
        memory_db = data_dir / "memory.db"
        if not memory_db.exists():
            import sqlite3
            conn = sqlite3.connect(str(memory_db))
            conn.execute(
                "CREATE TABLE IF NOT EXISTS memories ("
                " id INTEGER PRIMARY KEY AUTOINCREMENT,"
                " tick INTEGER NOT NULL,"
                " content TEXT NOT NULL,"
                " created_at REAL NOT NULL"
                ")"
            )
            conn.commit()
            conn.close()

        # skills.json
        skills_json = data_dir / "skills.json"
        if not skills_json.exists():
            skills_json.write_text("{}")

        # trace.db
        trace_db = data_dir / "trace.db"
        if not trace_db.exists():
            import sqlite3
            conn = sqlite3.connect(str(trace_db))
            conn.execute(
                "CREATE TABLE IF NOT EXISTS tick_snapshots ("
                " id INTEGER PRIMARY KEY AUTOINCREMENT,"
                " agent_id TEXT NOT NULL,"
                " tick INTEGER NOT NULL,"
                " snapshot_json TEXT NOT NULL,"
                " created_at REAL NOT NULL"
                ")"
            )
            conn.commit()
            conn.close()

        logger.debug(
            "Data files initialised for %s in %s", agent_name, data_dir,
        )

    def _start_process(
        self, name: str, config: dict[str, Any], data_dir: Path, log_path: Path,
    ) -> tuple[subprocess.Popen, Any]:
        """Start a single agent subprocess.

        Returns a tuple of (Popen, log_file_handle).

        Raises:
            RuntimeError: If the subprocess cannot be launched.
        """
        cmd = self._build_command(name, config, data_dir)
        env = self._get_env(data_dir)
        log_file = open(log_path, "a")

        try:
            proc = subprocess.Popen(
                cmd,
                stdout=log_file,
                stderr=log_file,
                env=env,
                start_new_session=True,
            )
            return proc, log_file
        except Exception:
            log_file.close()
            raise RuntimeError(f"Failed to start agent {name!r}: {cmd}") from None

    def _kill_locked(self, mp: ManagedProcess) -> None:
        """Terminate a ManagedProcess (caller must hold self._lock)."""
        if mp.process is None or mp.status == AgentStatus.STOPPED:
            return

        mp.status = AgentStatus.STOPPING
        proc = mp.process

        try:
            os.killpg(proc.pid, signal.SIGTERM)
        except ProcessLookupError:
            pass

        try:
            proc.wait(timeout=_GRACEFUL_SHUTDOWN_TIMEOUT)
        except subprocess.TimeoutExpired:
            logger.warning(
                "Agent %s did not exit within %ds — sending SIGKILL",
                mp.name, _GRACEFUL_SHUTDOWN_TIMEOUT,
            )
            try:
                os.killpg(proc.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            try:
                proc.wait(timeout=2)
            except subprocess.TimeoutExpired:
                logger.error("Agent %s refused to die after SIGKILL", mp.name)

        mp.exit_code = proc.returncode
        mp.status = AgentStatus.STOPPED

        # Close the log file handle to prevent FD leak
        if mp.log_file is not None:
            try:
                mp.log_file.close()
            except Exception:
                pass
            mp.log_file = None

        logger.info("Agent %s stopped  exit_code=%s", mp.name, mp.exit_code)

    def _shutdown_all(self) -> None:
        """Gracefully terminate all managed processes."""
        with self._lock:
            names = list(self._processes.keys())
        logger.info("Shutting down %d agent(s)", len(names))
        for name in names:
            try:
                with self._lock:
                    mp = self._processes.get(name)
                    if mp and mp.status in (AgentStatus.RUNNING, AgentStatus.STARTING):
                        self._kill_locked(mp)
            except Exception:
                logger.exception("Error shutting down agent %s", name)

    def _handle_auto_restart(self, deaths: list[DeathEvent]) -> None:
        """Attempt auto-restart for dead processes according to policy."""
        for event in deaths:
            should_restart = False
            with self._lock:
                mp = self._processes.get(event.agent_name)
                if mp is None:
                    continue

                if mp.restart_count >= self._max_restart_count:
                    logger.warning(
                        "Agent %s exceeded max restart count (%d) — not restarting",
                        event.agent_name, self._max_restart_count,
                    )
                    continue

                should_restart = (
                    self._auto_restart == AutoRestartPolicy.ALWAYS
                    or (
                        self._auto_restart == AutoRestartPolicy.ON_FAILURE
                        and event.exit_code is not None
                        and event.exit_code != 0
                    )
                )

            if should_restart:
                try:
                    self.restart(event.agent_name)
                except Exception:
                    logger.exception(
                        "Auto-restart failed for agent %s", event.agent_name,
                    )
