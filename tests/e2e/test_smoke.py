"""
Smoke tests for the Agent World E2E integration.

Validates:
1. World Engine starts and exposes its HTTP/gRPC endpoints
2. An Agent can connect to the World Engine and register
3. Dashboard dev server becomes accessible

Each test manages its own process lifecycle through fixtures in conftest.py.
All tests are designed to complete within 60 seconds.
"""

from __future__ import annotations

import json
import subprocess
import urllib.request
import urllib.error



# ── World Engine smoke tests ────────────────────────────────────


class TestWorldEngine:
    """Verify World Engine process starts and responds."""

    def test_engine_http_health(self, world_engine_process: subprocess.Popen, engine_port: int) -> None:
        """World Engine should expose HTTP API on the configured port."""
        url = f"http://localhost:{engine_port}/api/v1/world/stats"
        with urllib.request.urlopen(url, timeout=5) as resp:
            assert resp.status == 200
            body = json.loads(resp.read())
            assert isinstance(body, dict)

    def test_engine_agents_endpoint(self, world_engine_process: subprocess.Popen, engine_port: int) -> None:
        """GET /api/v1/agents should return a list (may be empty)."""
        url = f"http://localhost:{engine_port}/api/v1/agents"
        with urllib.request.urlopen(url, timeout=5) as resp:
            assert resp.status == 200
            body = json.loads(resp.read())
            assert isinstance(body, (list, dict))

    def test_engine_tick_endpoint(self, world_engine_process: subprocess.Popen, engine_port: int) -> None:
        """GET /api/v1/tick should return current tick info."""
        url = f"http://localhost:{engine_port}/api/v1/tick"
        with urllib.request.urlopen(url, timeout=5) as resp:
            assert resp.status == 200


# ── Agent registration smoke tests ──────────────────────────────


class TestAgentRegistration:
    """Verify an Agent can start, connect, and register with World Engine."""

    def test_agent_health_endpoint(self, agent_process: subprocess.Popen) -> None:
        """Agent should expose /health endpoint."""
        # Port is derived from the fixture (hash-based); we verify process is alive
        assert agent_process.poll() is None, "Agent process should still be running"

    def test_agent_registered_in_engine(
        self,
        world_engine_process: subprocess.Popen,
        agent_process: subprocess.Popen,
        engine_port: int,
    ) -> None:
        """After agent connects, it should appear in World Engine's agent list."""
        # Allow a brief moment for registration to propagate
        import time
        time.sleep(1)

        url = f"http://localhost:{engine_port}/api/v1/agents"
        with urllib.request.urlopen(url, timeout=5) as resp:
            assert resp.status == 200
            body = json.loads(resp.read())
            agents = body if isinstance(body, list) else body.get("agents", [])
            assert len(agents) >= 1, f"Expected at least 1 registered agent, got {len(agents)}"


# ── Dashboard smoke tests ───────────────────────────────────────


class TestDashboard:
    """Verify Dashboard dev server starts and serves pages."""

    def test_dashboard_homepage(
        self,
        world_engine_process: subprocess.Popen,
        dashboard_process: subprocess.Popen,
        dashboard_port: int,
    ) -> None:
        """Dashboard should serve HTML on its configured port."""
        url = f"http://localhost:{dashboard_port}/"
        with urllib.request.urlopen(url, timeout=10) as resp:
            assert resp.status == 200
            content_type = resp.headers.get("Content-Type", "")
            assert "text/html" in content_type


# ── Combined smoke test ─────────────────────────────────────────


class TestSmokeFullStack:
    """Full-stack smoke: Engine + Agent + Dashboard all running."""

    def test_full_stack_responsive(
        self,
        world_engine_process: subprocess.Popen,
        agent_process: subprocess.Popen,
        dashboard_process: subprocess.Popen,
        engine_port: int,
        dashboard_port: int,
    ) -> None:
        """All three services should be alive simultaneously."""
        # World Engine
        engine_url = f"http://localhost:{engine_port}/api/v1/world/stats"
        with urllib.request.urlopen(engine_url, timeout=5) as resp:
            assert resp.status == 200

        # Dashboard
        dash_url = f"http://localhost:{dashboard_port}/"
        with urllib.request.urlopen(dash_url, timeout=5) as resp:
            assert resp.status == 200

        # Agent process alive
        assert agent_process.poll() is None
