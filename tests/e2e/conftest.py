"""
E2E integration test fixtures.

Manages subprocess lifecycles for World Engine, Agent Runtime, and Dashboard
using pytest fixtures with parametrized ports and timeouts.

Each fixture starts a process, waits for a health check, and tears it down
after the test scope ends.
"""

from __future__ import annotations

import os
import signal
import subprocess
import time
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

DEFAULT_ENGINE_PORT = int(os.environ.get("E2E_ENGINE_PORT", "3000"))
DEFAULT_GRPC_PORT = int(os.environ.get("E2E_GRPC_PORT", "50051"))
DEFAULT_DASHBOARD_PORT = int(os.environ.get("E2E_DASHBOARD_PORT", "3001"))
DEFAULT_AGENT_HEALTH_PORT = int(os.environ.get("E2E_AGENT_HEALTH_PORT", "9090"))
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
        "WAL_DIR": str(ROOT / "data" / "e2e_test"),
    })

    proc = subprocess.Popen(
        ["cargo", "run", "--release"],
        cwd=str(WORLD_ENGINE_DIR),
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

    proc = subprocess.Popen(
        [
            "python", "-m", "agent_runtime", "spawn",
            "--name", agent_name,
            "--world-url", f"http://localhost:{engine_port}",
            "--no-llm",
            "--health-port", str(health_port),
        ],
        cwd=str(AGENT_RUNTIME_DIR),
        env=os.environ.copy(),
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
