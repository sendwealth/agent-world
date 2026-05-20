"""
E2E integration test fixtures.

Manages subprocess lifecycles for World Engine, Agent Runtime, and Dashboard
using pytest fixtures with parametrized ports and timeouts.

Each fixture starts a process, waits for a health check, and tears it down
after the test scope ends.
"""

from __future__ import annotations

import json
import os
import signal
import subprocess
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Generator, Optional

import pytest
import urllib.request
import urllib.error


# ── Project root resolution ─────────────────────────────────────

ROOT = Path(__file__).resolve().parent.parent.parent  # agent-world/

WORLD_ENGINE_DIR = ROOT / "world-engine"
AGENT_RUNTIME_DIR = ROOT / "agent-runtime"
DASHBOARD_DIR = ROOT / "dashboard"
CONFIG_DIR = ROOT / "config"


# ── Default configuration ───────────────────────────────────────

DEFAULT_ENGINE_PORT = int(os.environ.get("E2E_ENGINE_PORT", "18080"))
DEFAULT_GRPC_PORT = int(os.environ.get("E2E_GRPC_PORT", "50052"))
DEFAULT_DASHBOARD_PORT = int(os.environ.get("E2E_DASHBOARD_PORT", "13001"))
DEFAULT_AGENT_HEALTH_PORT = int(os.environ.get("E2E_AGENT_HEALTH_PORT", "19090"))
DEFAULT_STARTUP_TIMEOUT = int(os.environ.get("E2E_STARTUP_TIMEOUT", "30"))
DEFAULT_HEALTH_INTERVAL = 0.5  # seconds between health polls


# ── Helpers ─────────────────────────────────────────────────────

def wait_for_health(
    url: str,
    timeout: float = DEFAULT_STARTUP_TIMEOUT,
    interval: float = DEFAULT_HEALTH_INTERVAL,
) -> bool:
    """Poll *url* until it returns HTTP 200 or *timeout* expires.

    Returns True if healthy within timeout, False otherwise.
    """
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        try:
            req = urllib.request.Request(url, method="GET")
            with urllib.request.urlopen(req, timeout=2) as resp:
                if resp.status == 200:
                    return True
        except (urllib.error.URLError, ConnectionError, OSError):
            pass
        time.sleep(interval)
    return False


def terminate_process(proc: subprocess.Popen, timeout: float = 5.0) -> None:
    """Gracefully terminate a subprocess, then kill if needed."""
    if proc.poll() is not None:
        return
    try:
        proc.terminate()
        try:
            proc.wait(timeout=timeout)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait(timeout=2)
    except OSError:
        pass


def kill_port(port: int) -> None:
    """Kill any process listening on *port* to avoid stale-process conflicts."""
    import subprocess as _sp
    try:
        pids = _sp.check_output(
            ["lsof", "-t", "-i", f":{port}"], stderr=_sp.DEVNULL
        ).decode()
        for pid in pids.split():
            _sp.run(["kill", pid], stderr=_sp.DEVNULL)
    except (_sp.CalledProcessError, FileNotFoundError):
        pass  # nothing on that port — all good


# ── Fixture: World Engine ───────────────────────────────────────

@pytest.fixture(scope="session")
def engine_port() -> int:
    return DEFAULT_ENGINE_PORT


@pytest.fixture(scope="session")
def grpc_port() -> int:
    return DEFAULT_GRPC_PORT


@pytest.fixture(scope="session")
def startup_timeout() -> float:
    return float(DEFAULT_STARTUP_TIMEOUT)


@pytest.fixture(scope="session")
def world_engine_process(
    engine_port: int,
    grpc_port: int,
    startup_timeout: float,
) -> Generator[subprocess.Popen, None, None]:
    """Start the World Engine (Rust binary) as a subprocess.

    Uses the genesis config from ``config/genesis.yaml``.
    The process is torn down when the session ends.
    """
    env = os.environ.copy()
    env.update({
        "PORT": str(engine_port),
        "GRPC_ADDR": f"0.0.0.0:{grpc_port}",
        "RUST_LOG": "warn",
        "GENESIS_PATH": str(CONFIG_DIR / "genesis.yaml"),
        # Isolate data to avoid conflicting with dev runs
        "WAL_DIR": "/tmp/agent-world-e2e-test/wal",
    })

    # Kill any stale process on our ports before starting
    kill_port(engine_port)
    kill_port(grpc_port)

    ENGINE_BIN = WORLD_ENGINE_DIR / "target" / "debug" / "agent-world-engine"
    proc = subprocess.Popen(
        [str(ENGINE_BIN)],
        cwd=str(ROOT),
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )

    health_url = f"http://localhost:{engine_port}/api/v1/world/stats"
    healthy = wait_for_health(health_url, timeout=startup_timeout)

    if not healthy:
        terminate_process(proc)
        stdout = proc.stdout.read().decode(errors="replace") if proc.stdout else ""
        stderr = proc.stderr.read().decode(errors="replace") if proc.stderr else ""
        pytest.fail(
            f"World Engine did not become healthy within {startup_timeout}s.\n"
            f"stdout: {stdout[-500:]}\nstderr: {stderr[-500:]}"
        )

    yield proc

    terminate_process(proc)


# ── Fixture: Agent Process ──────────────────────────────────────

@pytest.fixture()
def agent_process(
    world_engine_process: subprocess.Popen,
    engine_port: int,
    startup_timeout: float,
    request,
) -> Generator[subprocess.Popen, None, None]:
    """Start a single Agent Runtime process connected to the World Engine.

    Uses ``--no-llm`` for deterministic testing.  The agent is torn down
    after each test.
    """
    health_port = DEFAULT_AGENT_HEALTH_PORT + hash(request.node.name) % 100
    agent_name = f"e2e-test-{request.node.name}"

    env = os.environ.copy()
    # Pass gRPC port so the agent can attempt gRPC before REST fallback
    env["GRPC_PORT"] = str(DEFAULT_GRPC_PORT)

    proc = subprocess.Popen(
        [
            "python", "-m", "agent_runtime", "spawn",
            "--name", agent_name,
            "--world-url", f"http://localhost:{engine_port}",
            "--no-llm",
            "--health-port", str(health_port),
        ],
        cwd=str(AGENT_RUNTIME_DIR),
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )

    health_url = f"http://localhost:{health_port}/health"
    healthy = wait_for_health(health_url, timeout=startup_timeout)

    if not healthy:
        terminate_process(proc)
        stdout = proc.stdout.read().decode(errors="replace") if proc.stdout else ""
        stderr = proc.stderr.read().decode(errors="replace") if proc.stderr else ""
        pytest.fail(
            f"Agent '{agent_name}' did not become healthy within {startup_timeout}s.\n"
            f"stdout: {stdout[-500:]}\nstderr: {stderr[-500:]}"
        )

    yield proc

    terminate_process(proc)


# ── Fixture: Dashboard Process ──────────────────────────────────

@pytest.fixture(scope="session")
def dashboard_port() -> int:
    return DEFAULT_DASHBOARD_PORT


@pytest.fixture(scope="session")
def dashboard_process(
    dashboard_port: int,
    startup_timeout: float,
) -> Generator[subprocess.Popen, None, None]:
    """Start the Next.js Dashboard dev server.

    Skipped if ``npm`` is not available or dashboard deps are missing.
    """
    if not (DASHBOARD_DIR / "node_modules").exists():
        pytest.skip("Dashboard node_modules not installed; run `cd dashboard && npm install`")

    env = os.environ.copy()
    env["PORT"] = str(dashboard_port)

    proc = subprocess.Popen(
        ["npm", "run", "dev"],
        cwd=str(DASHBOARD_DIR),
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )

    health_url = f"http://localhost:{dashboard_port}/"
    healthy = wait_for_health(health_url, timeout=startup_timeout)

    if not healthy:
        terminate_process(proc)
        pytest.fail(f"Dashboard did not become healthy within {startup_timeout}s")

    yield proc

    terminate_process(proc)


# ── Fixture: Multi-Agent Processes (10 agents for stability tests) ─

@pytest.fixture(scope="session")
def multi_agent_processes(
    world_engine_process: subprocess.Popen,
    engine_port: int,
    startup_timeout: float,
) -> Generator[list[tuple[subprocess.Popen, int]], None, None]:
    """Start 10 Agent Runtime processes connected to the World Engine.

    Each agent runs with ``--no-llm`` for deterministic testing and
    ``--max-ticks 0`` for unlimited execution.  Health ports are offset
    by 200 from the default to avoid clashing with the single-agent fixture.

    Yields a list of ``(Popen, health_port)`` tuples.
    """
    NUM_AGENTS = 10
    agents: list[tuple[subprocess.Popen, int]] = []

    for i in range(NUM_AGENTS):
        health_port = DEFAULT_AGENT_HEALTH_PORT + 200 + i
        agent_name = f"agent-{i}"

        env = os.environ.copy()
        env["GRPC_PORT"] = str(DEFAULT_GRPC_PORT)

        proc = subprocess.Popen(
            [
                "python", "-m", "agent_runtime", "spawn",
                "--name", agent_name,
                "--world-url", f"http://localhost:{engine_port}",
                "--no-llm",
                "--max-ticks", "0",
                "--health-port", str(health_port),
            ],
            cwd=str(AGENT_RUNTIME_DIR),
            env=env,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )

        health_url = f"http://localhost:{health_port}/health"
        healthy = wait_for_health(health_url, timeout=startup_timeout)

        if not healthy:
            # Tear down everything started so far
            for p, _ in reversed(agents):
                terminate_process(p)
            terminate_process(proc)
            stdout = proc.stdout.read().decode(errors="replace") if proc.stdout else ""
            stderr = proc.stderr.read().decode(errors="replace") if proc.stderr else ""
            pytest.fail(
                f"Agent '{agent_name}' (port {health_port}) did not become healthy "
                f"within {startup_timeout}s.\n"
                f"stdout: {stdout[-500:]}\nstderr: {stderr[-500:]}"
            )

        agents.append((proc, health_port))

    yield agents

    # Teardown in reverse order
    for proc, _ in reversed(agents):
        terminate_process(proc)


# ── Failure Diagnostics Collection ──────────────────────────────

REPORTS_DIR = ROOT / "reports" / "failures"


def _fetch_json(url: str, timeout: float = 3.0) -> Optional[dict]:
    """Fetch JSON from *url*, returning parsed dict or None on failure."""
    try:
        req = urllib.request.Request(url, method="GET")
        req.add_header("Accept", "application/json")
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            return json.loads(resp.read().decode())
    except Exception:
        return None


def _read_stderr(proc: subprocess.Popen, tail: int = 2000) -> str:
    """Read up to *tail* chars of stderr from a subprocess (non-blocking best-effort)."""
    try:
        if proc.stderr is None:
            return ""
        # Don't block — read whatever is available
        import select
        import fcntl

        fd = proc.stderr.fileno()
        fl = fcntl.fcntl(fd, fcntl.F_GETFL)
        fcntl.fcntl(fd, fcntl.F_SETFL, fl | os.O_NONBLOCK)
        try:
            data = proc.stderr.read()
            if data:
                text = data.decode(errors="replace")
                return text[-tail:] if len(text) > tail else text
        except (IOError, OSError):
            pass
        finally:
            fcntl.fcntl(fd, fcntl.F_SETFL, fl)
    except Exception:
        pass
    return ""


def _get_fixture_safe(request, name: str):
    """Get a fixture value by name, returning None if not available."""
    if name not in request.fixturenames:
        return None
    try:
        return request.getfixturevalue(name)
    except Exception:
        return None


@pytest.fixture(autouse=True)
def collect_failure_diagnostics(request):
    """On test failure, collect diagnostic context and write to ``reports/failures/``.

    Inspired by LobeChat's After-hook pattern: screenshot + HTML + console errors.
    We collect engine/agent stderr, World stats, Tick info, and Agent list.

    Gracefully degrades when engine/agent fixtures are not active (e.g. unit tests).
    """
    yield  # Run the test first

    # Only collect on failure
    if not hasattr(request.node, "rep_call") or not request.node.rep_call.failed:
        return

    engine_port_val = _get_fixture_safe(request, "engine_port")
    world_engine_proc = _get_fixture_safe(request, "world_engine_process")

    # If no engine is running, there's nothing to diagnose
    if engine_port_val is None or world_engine_proc is None:
        return

    test_name = request.node.nodeid.replace("::", "_").replace("/", "_").replace(" ", "_")
    timestamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    report_file = REPORTS_DIR / f"{test_name}.{timestamp}.json"

    diagnostics: dict = {
        "timestamp": datetime.now(timezone.utc).isoformat(),
        "test_name": request.node.nodeid,
        "engine_port": engine_port_val,
    }

    # 1. World Engine stderr log
    diagnostics["engine_stderr"] = _read_stderr(world_engine_proc)

    # 2. Agent Runtime stderr — best-effort from active agent fixtures
    agent_proc = _get_fixture_safe(request, "agent_process")
    if agent_proc is not None:
        diagnostics["agent_stderr"] = _read_stderr(agent_proc)
    else:
        multi_agents = _get_fixture_safe(request, "multi_agent_processes")
        if multi_agents:
            diagnostics["multi_agent_stderr"] = {
                f"agent-{i}": _read_stderr(p)
                for i, (p, _) in enumerate(multi_agents)
            }

    # 3. World state
    world_stats = _fetch_json(f"http://localhost:{engine_port_val}/api/v1/world/stats")
    diagnostics["world_stats"] = world_stats

    # 4. Tick information
    tick_info = _fetch_json(f"http://localhost:{engine_port_val}/api/v1/tick")
    diagnostics["tick_info"] = tick_info

    # 5. Agent list and status
    agents_list = _fetch_json(f"http://localhost:{engine_port_val}/api/v1/agents")
    diagnostics["agents"] = agents_list

    # Write report
    REPORTS_DIR.mkdir(parents=True, exist_ok=True)
    report_file.write_text(json.dumps(diagnostics, indent=2, ensure_ascii=False, default=str))
    print(f"\n[Failure Diagnostics] Written to {report_file}")


@pytest.hookimpl(tryfirst=True, hookwrapper=True)
def pytest_runtest_makereport(item, call):
    """Hook to store test results on the item for the autouse fixture to read."""
    outcome = yield
    rep = outcome.get_result()
    setattr(item, f"rep_{rep.when}", rep)
