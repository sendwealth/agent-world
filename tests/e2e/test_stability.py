"""
10-Agent E2E stability tests.

Validates that 10 agents can simultaneously connect to the World Engine,
process 100 ticks, and remain alive throughout — proving the system handles
multi-agent workloads without crashes or resource leaks.
"""

from __future__ import annotations

import json
import subprocess
import time
import urllib.request
import urllib.error

import pytest


class TestStability10Agents:
    """10-Agent stability: run 100 ticks, verify all agents alive."""

    def test_10_agents_100_ticks(
        self,
        world_engine_process: subprocess.Popen,
        engine_port: int,
        multi_agent_processes: list[tuple[subprocess.Popen, int]],
    ) -> None:
        agents = multi_agent_processes
        assert len(agents) == 10, f"Expected 10 agents, got {len(agents)}"

        # 1. Verify World Engine reports >= 10 registered agents
        agents_url = f"http://localhost:{engine_port}/api/v1/agents"
        with urllib.request.urlopen(agents_url, timeout=5) as resp:
            assert resp.status == 200
            body = json.loads(resp.read())
            agent_list = body if isinstance(body, list) else body.get("agents", [])
            assert len(agent_list) >= 10, (
                f"Expected >= 10 agents registered, got {len(agent_list)}"
            )

        # 2. Advance 100 ticks via POST /api/v1/tick
        tick_url = f"http://localhost:{engine_port}/api/v1/tick"
        req = urllib.request.Request(
            tick_url,
            data=json.dumps({"count": 100}).encode(),
            headers={"Content-Type": "application/json"},
            method="POST",
        )
        with urllib.request.urlopen(req, timeout=30) as resp:
            assert resp.status == 200

        # 3. Wait for agents to process the ticks
        time.sleep(2)

        # 4. Verify tick has advanced to >= 100
        with urllib.request.urlopen(tick_url, timeout=5) as resp:
            assert resp.status == 200
            tick_body = json.loads(resp.read())
            current_tick = tick_body.get("tick", tick_body.get("current_tick", 0))
            assert current_tick >= 100, (
                f"Expected tick >= 100, got {current_tick}"
            )

        # 5. Verify all 10 agent processes are still alive
        for i, (proc, port) in enumerate(agents):
            assert proc.poll() is None, (
                f"Agent {i} (port {port}) has exited with code {proc.returncode}"
            )

        # 6. Verify /api/v1/world/stats reports correct agent counts
        stats_url = f"http://localhost:{engine_port}/api/v1/world/stats"
        with urllib.request.urlopen(stats_url, timeout=5) as resp:
            assert resp.status == 200
            stats = json.loads(resp.read())
            agent_count = stats.get("agentCount", stats.get("agent_count", stats.get("agents", 0)))
            alive_count = stats.get("aliveCount", stats.get("alive_count", stats.get("alive", 0)))
            assert agent_count >= 10, (
                f"Expected agent_count >= 10, got {agent_count}"
            )
            assert alive_count >= 10, (
                f"Expected alive_count >= 10, got {alive_count}"
            )

        # 7. Verify each agent's /health endpoint reports tick > 0
        for i, (proc, health_port) in enumerate(agents):
            health_url = f"http://localhost:{health_port}/health"
            with urllib.request.urlopen(health_url, timeout=5) as resp:
                assert resp.status == 200
                health = json.loads(resp.read())
                agent_tick = health.get("tick", health.get("ticks", 0))
                assert agent_tick > 0, (
                    f"Agent {i} (port {health_port}) reports tick={agent_tick}, expected > 0"
                )
