"""E2E test: Agent action → World Engine state change (feedback loop).

Validates the #1 blocker from Phase 4.7: agent decisions are reliably
committed to the World Engine and the state change is observable.

Test strategy:
  1. Register an agent via RESTWorldClient
  2. Execute actions (gather, move, build) through RESTWorldClient
  3. Read the agent status back from the World Engine REST API
  4. Assert that World Engine state (money, position) changed as expected
  5. Verify error propagation when action execution fails

This test uses the live World Engine subprocess (managed by conftest.py)
and the RESTWorldClient class from ``__main__.py``.
"""

from __future__ import annotations

import json
import urllib.request
import urllib.error
from typing import Any

import pytest

from agent_runtime.__main__ import RESTWorldClient


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def _fetch_json(url: str, timeout: float = 5.0) -> dict[str, Any]:
    """GET *url* and return parsed JSON; raises on failure."""
    req = urllib.request.Request(url, method="GET")
    req.add_header("Accept", "application/json")
    with urllib.request.urlopen(req, timeout=timeout) as resp:
        return json.loads(resp.read().decode())


def _register_agent(
    engine_port: int, name: str = "feedback-test-agent"
) -> dict[str, Any]:
    """Register an external agent via the World Engine REST API."""
    url = f"http://localhost:{engine_port}/api/v1/agents/register"
    payload = json.dumps({
        "name": name,
        "capabilities": ["gather", "move", "build"],
        "config": {},
    }).encode()
    req = urllib.request.Request(url, data=payload, method="POST")
    req.add_header("Content-Type", "application/json")
    with urllib.request.urlopen(req, timeout=5.0) as resp:
        return json.loads(resp.read().decode())


def _deregister_agent(engine_port: int, agent_id: str) -> None:
    """Remove an agent from the World Engine (cleanup)."""
    url = f"http://localhost:{engine_port}/api/v1/agents/{agent_id}"
    req = urllib.request.Request(url, method="DELETE")
    try:
        with urllib.request.urlopen(req, timeout=5.0):
            pass
    except urllib.error.HTTPError:
        pass  # agent may already be gone


def _get_agent_status(
    engine_port: int, agent_id: str
) -> dict[str, Any]:
    """Get agent status from the World Engine."""
    return _fetch_json(
        f"http://localhost:{engine_port}/api/v1/agents/{agent_id}/status"
    )


# ---------------------------------------------------------------------------
# Test: Action → World Engine state change
# ---------------------------------------------------------------------------


class TestActionFeedbackLoop:
    """Verify that RESTWorldClient actions cause observable state changes
    in the World Engine."""

    @pytest.fixture()
    def registered_agent(
        self, engine_port: int
    ) -> dict[str, Any]:
        """Register an agent and yield its details; deregister on cleanup."""
        data = _register_agent(engine_port)
        yield data
        _deregister_agent(engine_port, data["agent_id"])

    @pytest.mark.asyncio
    async def test_gather_increases_money(
        self, engine_port: int, registered_agent: dict[str, Any]
    ) -> None:
        """Agent executes gather → World Engine agent.money increases."""
        agent_id = registered_agent["agent_id"]
        client = RESTWorldClient(
            f"http://localhost:{engine_port}", agent_id=agent_id
        )

        # Baseline: check money before gather
        before = _get_agent_status(engine_port, agent_id)
        money_before = before["money"]

        # Execute gather
        result = await client.gather("food")
        assert result.get("success") is True or result.get("status") == "ok", (
            f"Expected action success, got: {result}"
        )

        # Verify: money should have increased by 10 (World Engine gather effect)
        after = _get_agent_status(engine_port, agent_id)
        assert after["money"] == money_before + 10, (
            f"Expected money={money_before + 10}, got money={after['money']}"
        )

    @pytest.mark.asyncio
    async def test_move_updates_position(
        self, engine_port: int, registered_agent: dict[str, Any]
    ) -> None:
        """Agent executes move(north) → World Engine agent.position.y increases."""
        agent_id = registered_agent["agent_id"]
        client = RESTWorldClient(
            f"http://localhost:{engine_port}", agent_id=agent_id
        )

        # Baseline: check position before move
        before = _get_agent_status(engine_port, agent_id)
        y_before = before["position"]["y"]

        # Execute move north
        result = await client.move("north")
        assert result.get("success") is True or result.get("status") == "ok", (
            f"Expected action success, got: {result}"
        )

        # Verify: y should have increased by 1
        after = _get_agent_status(engine_port, agent_id)
        assert after["position"]["y"] == y_before + 1, (
            f"Expected position.y={y_before + 1}, got position.y={after['position']['y']}"
        )

    @pytest.mark.asyncio
    async def test_move_east_updates_x(
        self, engine_port: int, registered_agent: dict[str, Any]
    ) -> None:
        """Agent executes move(east) → World Engine agent.position.x increases."""
        agent_id = registered_agent["agent_id"]
        client = RESTWorldClient(
            f"http://localhost:{engine_port}", agent_id=agent_id
        )

        before = _get_agent_status(engine_port, agent_id)
        x_before = before["position"]["x"]

        result = await client.move("east")
        assert result.get("success") is True or result.get("status") == "ok"

        after = _get_agent_status(engine_port, agent_id)
        assert after["position"]["x"] == x_before + 1

    @pytest.mark.asyncio
    async def test_sequential_gather_accumulates(
        self, engine_port: int, registered_agent: dict[str, Any]
    ) -> None:
        """Multiple gather calls accumulate money correctly."""
        agent_id = registered_agent["agent_id"]
        client = RESTWorldClient(
            f"http://localhost:{engine_port}", agent_id=agent_id
        )

        before = _get_agent_status(engine_port, agent_id)
        money_before = before["money"]

        # Gather three times
        for _ in range(3):
            result = await client.gather("wood")
            assert result.get("success") is True or result.get("status") == "ok"

        # Should have +30 money (3 × 10)
        after = _get_agent_status(engine_port, agent_id)
        assert after["money"] == money_before + 30

    @pytest.mark.asyncio
    async def test_action_on_unknown_agent_raises(
        self, engine_port: int
    ) -> None:
        """Action on a non-existent agent should raise (404), not silently succeed."""
        client = RESTWorldClient(
            f"http://localhost:{engine_port}",
            agent_id="nonexistent-agent-00000000",
        )

        with pytest.raises(Exception) as exc_info:
            await client.gather("food")
        # Should be an HTTP status error, not a silent {"status": "standalone"}
        error_msg = str(exc_info.value).lower()
        assert "404" in error_msg or "not found" in error_msg, (
            f"Expected 404 error for unknown agent, got: {exc_info.value}"
        )

    @pytest.mark.asyncio
    async def test_invalid_action_returns_error(
        self, engine_port: int, registered_agent: dict[str, Any]
    ) -> None:
        """Submitting an unknown action type returns an error from World Engine."""
        agent_id = registered_agent["agent_id"]
        client = RESTWorldClient(
            f"http://localhost:{engine_port}", agent_id=agent_id
        )

        with pytest.raises(Exception) as exc_info:
            await client.submit_action("fly_to_moon", {})
        error_msg = str(exc_info.value).lower()
        assert "400" in error_msg or "unknown" in error_msg, (
            f"Expected 400/bad request for invalid action, got: {exc_info.value}"
        )

    @pytest.mark.asyncio
    async def test_deregistered_agent_action_fails(
        self, engine_port: int
    ) -> None:
        """After deregistering, actions on the agent should fail."""
        # Register
        data = _register_agent(engine_port, name="ephemeral-agent")
        agent_id = data["agent_id"]
        client = RESTWorldClient(
            f"http://localhost:{engine_port}", agent_id=agent_id
        )

        # Verify it works first
        result = await client.gather("food")
        assert result.get("success") is True or result.get("status") == "ok"

        # Deregister
        _deregister_agent(engine_port, agent_id)

        # Now actions should fail with 404
        with pytest.raises(Exception) as exc_info:
            await client.gather("food")
        error_msg = str(exc_info.value).lower()
        assert "404" in error_msg or "not found" in error_msg

    @pytest.mark.asyncio
    async def test_tick_advances_on_action(
        self, engine_port: int, registered_agent: dict[str, Any]
    ) -> None:
        """Each action should advance the World Engine tick."""
        agent_id = registered_agent["agent_id"]
        client = RESTWorldClient(
            f"http://localhost:{engine_port}", agent_id=agent_id
        )

        before = _get_agent_status(engine_port, agent_id)
        tick_before = before["current_tick"]

        await client.move("north")

        after = _get_agent_status(engine_port, agent_id)
        assert after["current_tick"] > tick_before, (
            f"Expected tick to advance from {tick_before}, "
            f"got tick={after['current_tick']}"
        )

    @pytest.mark.asyncio
    async def test_all_world_client_actions_route_correctly(
        self, engine_port: int, registered_agent: dict[str, Any]
    ) -> None:
        """Verify every action method on RESTWorldClient hits the World Engine."""
        agent_id = registered_agent["agent_id"]
        client = RESTWorldClient(
            f"http://localhost:{engine_port}", agent_id=agent_id
        )

        # These should all succeed (the action reaches the World Engine)
        # gather → money increases
        r = await client.gather("stone")
        assert r.get("success") is True or r.get("status") == "ok"

        # move → position changes
        r = await client.move("south")
        assert r.get("success") is True or r.get("status") == "ok"

        # explore → accepted
        r = await client.explore({"capabilities": ["gather"]})
        assert r.get("success") is True or r.get("status") == "ok"

        # build → accepted
        r = await client.build("house")
        assert r.get("success") is True or r.get("status") == "ok"

        # claim_task → accepted (World Engine accepts it)
        r = await client.claim_task("task-001")
        assert r.get("success") is True or r.get("status") == "ok"

        # submit_task → accepted
        r = await client.submit_task("task-001", {"result": "done"})
        assert r.get("success") is True or r.get("status") == "ok"

        # propose_deal → routed as "trade" action
        r = await client.propose_deal({"item": "wood", "price": 10})
        assert r.get("success") is True or r.get("status") == "ok"

        # teach_skill → routed as "communicate" action
        r = await client.teach_skill("other-agent", "coding", 2)
        assert r.get("success") is True or r.get("status") == "ok"

        # rest → accepted
        r = await client.submit_action("rest", {})
        assert r.get("success") is True or r.get("status") == "ok"
