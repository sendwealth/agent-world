"""Tests for agent_runtime.sdk.client.AgentWorldClient.

All HTTP calls are mocked via unittest.mock – no live server required.
"""

from __future__ import annotations

import json
from unittest.mock import MagicMock, patch

import httpx
import pytest

from agent_runtime.sdk.client import API_PREFIX, AgentWorldClient

BASE_URL = "http://localhost:3000"


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def _mock_response(status_code: int = 200, data: dict | None = None) -> MagicMock:
    """Return a MagicMock that behaves like an httpx.Response."""
    resp = MagicMock(spec=httpx.Response)
    resp.status_code = status_code
    resp.json.return_value = data or {}
    if status_code >= 400:
        resp.raise_for_status.side_effect = httpx.HTTPStatusError(
            message=f"HTTP {status_code}",
            request=MagicMock(),
            response=resp,
        )
    else:
        resp.raise_for_status.return_value = None
    return resp


# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------

@pytest.fixture()
def mock_httpx_client():
    """Patch httpx.Client and yield the mock instance for assertions."""
    with patch("agent_runtime.sdk.client.httpx.Client") as MockClient:
        mock_instance = MagicMock()
        MockClient.return_value = mock_instance
        yield mock_instance


@pytest.fixture()
def client(mock_httpx_client):
    """Return an AgentWorldClient whose internal httpx.Client is mocked."""
    return AgentWorldClient(BASE_URL)


# ---------------------------------------------------------------------------
# Tests
# ---------------------------------------------------------------------------

class TestRegister:
    """Tests for AgentWorldClient.register()."""

    def test_register_success(self, client, mock_httpx_client):
        """register() should POST to /agents/register and return parsed JSON."""
        expected = {"agent_id": "agent-001", "api_key": "key-abc"}
        mock_httpx_client.post.return_value = _mock_response(data=expected)

        result = client.register("test-agent", capabilities=["move", "gather"])

        mock_httpx_client.post.assert_called_once_with(
            f"{API_PREFIX}/agents/register",
            json={"name": "test-agent", "capabilities": ["move", "gather"]},
        )
        assert result == expected

    def test_register_sets_agent_id_and_api_key(self, client, mock_httpx_client):
        """register() should store agent_id and api_key as instance state."""
        mock_httpx_client.post.return_value = _mock_response(
            data={"agent_id": "agent-42", "api_key": "secret"}
        )

        assert client.agent_id is None
        assert client.api_key is None

        client.register("stateful-agent", capabilities=["explore"])

        assert client.agent_id == "agent-42"
        assert client.api_key == "secret"


class TestStatus:
    """Tests for AgentWorldClient.status()."""

    def test_status(self, client, mock_httpx_client):
        """status() should GET /agents/{id}/status and return the payload."""
        expected = {
            "agent_id": "agent-001",
            "name": "test-agent",
            "capabilities": ["move"],
            "alive": True,
            "tokens": 100,
        }
        mock_httpx_client.get.return_value = _mock_response(data=expected)

        # Register first so the internal _agent_id is set.
        mock_httpx_client.post.return_value = _mock_response(
            data={"agent_id": "agent-001", "api_key": "key"}
        )
        client.register("test-agent")

        result = client.status()

        mock_httpx_client.get.assert_called_once_with(
            f"{API_PREFIX}/agents/agent-001/status"
        )
        assert result["agent_id"] == "agent-001"
        assert result["alive"] is True

    def test_status_with_explicit_agent_id(self, client, mock_httpx_client):
        """status(agent_id=...) should use the supplied ID, not the stored one."""
        expected = {"agent_id": "other-agent", "name": "other", "alive": False}
        mock_httpx_client.get.return_value = _mock_response(data=expected)

        result = client.status("other-agent")

        mock_httpx_client.get.assert_called_once_with(
            f"{API_PREFIX}/agents/other-agent/status"
        )
        assert result["agent_id"] == "other-agent"


class TestDeregister:
    """Tests for AgentWorldClient.deregister()."""

    def test_deregister_clears_state(self, client, mock_httpx_client):
        """deregister() should clear agent_id and api_key when deregistering self."""
        mock_httpx_client.post.return_value = _mock_response(
            data={"agent_id": "agent-99", "api_key": "k"}
        )
        mock_httpx_client.delete.return_value = _mock_response(
            data={"deregistered": True}
        )

        client.register("to-remove")
        assert client.agent_id == "agent-99"

        result = client.deregister()

        assert result["deregistered"] is True
        assert client.agent_id is None
        assert client.api_key is None


class TestAction:
    """Tests for AgentWorldClient.action() and convenience shortcuts."""

    def test_action_move(self, client, mock_httpx_client):
        """action() should POST to /agents/{id}/action with the right payload."""
        expected = {"success": True, "result": "moved north", "tick": 5}
        mock_httpx_client.post.return_value = _mock_response(data=expected)

        # Register first.
        mock_httpx_client.post.side_effect = [
            _mock_response(data={"agent_id": "a1", "api_key": "k"}),
            _mock_response(data=expected),
        ]

        client.register("mover", capabilities=["move"])
        result = client.action(None, "move", {"direction": "north"})

        assert result["success"] is True
        assert result["tick"] == 5

    def test_convenience_methods(self, client, mock_httpx_client):
        """move(), gather(), explore() should call action() with correct args."""
        # Register first so _agent_id is set.
        mock_httpx_client.post.return_value = _mock_response(
            data={"agent_id": "a1", "api_key": "k"}
        )
        client.register("convenience-agent")

        # Reset the mock so we can inspect individual calls.
        mock_httpx_client.post.reset_mock()

        action_response = {"success": True, "result": "ok", "tick": 1}
        mock_httpx_client.post.return_value = _mock_response(data=action_response)

        # --- move ---
        client.move("north")
        mock_httpx_client.post.assert_called_with(
            f"{API_PREFIX}/agents/a1/action",
            json={"action": "move", "params": {"direction": "north"}},
        )

        # --- gather ---
        mock_httpx_client.post.reset_mock()
        client.gather("wood")
        mock_httpx_client.post.assert_called_with(
            f"{API_PREFIX}/agents/a1/action",
            json={"action": "gather", "params": {"resource_type": "wood"}},
        )

        # --- explore ---
        mock_httpx_client.post.reset_mock()
        client.explore()
        mock_httpx_client.post.assert_called_with(
            f"{API_PREFIX}/agents/a1/action",
            json={"action": "explore"},
        )

        # --- rest ---
        mock_httpx_client.post.reset_mock()
        client.rest()
        mock_httpx_client.post.assert_called_with(
            f"{API_PREFIX}/agents/a1/action",
            json={"action": "rest"},
        )

        # --- build ---
        mock_httpx_client.post.reset_mock()
        client.build("wall")
        mock_httpx_client.post.assert_called_with(
            f"{API_PREFIX}/agents/a1/action",
            json={"action": "build", "params": {"structure_type": "wall"}},
        )

        # --- communicate ---
        mock_httpx_client.post.reset_mock()
        client.communicate("agent-b", "hello")
        mock_httpx_client.post.assert_called_with(
            f"{API_PREFIX}/agents/a1/action",
            json={
                "action": "communicate",
                "params": {"target_agent_id": "agent-b", "message": "hello"},
            },
        )


class TestPerception:
    """Tests for AgentWorldClient.perception()."""

    def test_perception(self, client, mock_httpx_client):
        """perception() should GET /agents/{id}/perception."""
        expected = {
            "nearby_agents": [],
            "nearby_resources": [{"type": "wood", "distance": 2}],
            "world_tick": 10,
        }
        mock_httpx_client.get.return_value = _mock_response(data=expected)

        # Register so we have an agent_id.
        mock_httpx_client.post.return_value = _mock_response(
            data={"agent_id": "a1", "api_key": "k"}
        )
        client.register("perceiver")

        result = client.perception()

        mock_httpx_client.get.assert_called_once_with(
            f"{API_PREFIX}/agents/a1/perception"
        )
        assert result["world_tick"] == 10
        assert len(result["nearby_resources"]) == 1


class TestRequireAgentId:
    """Tests for _require_agent_id() guarding."""

    def test_require_agent_id_raises_before_register(self, client, mock_httpx_client):
        """Methods that need an agent_id should raise RuntimeError before register."""
        with pytest.raises(RuntimeError, match="No agent registered"):
            client.status()

        with pytest.raises(RuntimeError, match="No agent registered"):
            client.perception()

        with pytest.raises(RuntimeError, match="No agent registered"):
            client.action(None, "move", {"direction": "north"})

        with pytest.raises(RuntimeError, match="No agent registered"):
            client.move("north")


class TestContextManager:
    """Tests for context-manager protocol."""

    def test_context_manager(self, mock_httpx_client):
        """Using AgentWorldClient as a context manager should close on exit."""
        with AgentWorldClient(BASE_URL) as client:
            assert isinstance(client, AgentWorldClient)
            assert client.base_url == BASE_URL

        mock_httpx_client.close.assert_called_once()

    def test_context_manager_closes_on_exception(self, mock_httpx_client):
        """close() should be called even if an exception occurs inside the block."""
        with pytest.raises(ValueError):
            with AgentWorldClient(BASE_URL) as client:
                raise ValueError("boom")

        mock_httpx_client.close.assert_called_once()


class TestHttpErrors:
    """Tests for HTTP error handling."""

    def test_http_error_raises(self, client, mock_httpx_client):
        """Non-2xx responses should raise httpx.HTTPStatusError."""
        mock_httpx_client.get.return_value = _mock_response(
            status_code=404, data={"detail": "not found"}
        )

        with pytest.raises(httpx.HTTPStatusError) as exc_info:
            client.status("nonexistent-agent")

        assert exc_info.value.response.status_code == 404


class TestWorldHelpers:
    """Tests for world-level helpers."""

    def test_world_stats(self, client, mock_httpx_client):
        expected = {"total_agents": 5, "tick": 42}
        mock_httpx_client.get.return_value = _mock_response(data=expected)

        result = client.world_stats()

        mock_httpx_client.get.assert_called_once_with(f"{API_PREFIX}/world/stats")
        assert result == expected

    def test_tick(self, client, mock_httpx_client):
        expected = {"tick": 42}
        mock_httpx_client.get.return_value = _mock_response(data=expected)

        result = client.tick()

        mock_httpx_client.get.assert_called_once_with(f"{API_PREFIX}/tick")
        assert result["tick"] == 42


class TestInit:
    """Tests for __init__ behaviour."""

    def test_base_url_trailing_slash_stripped(self, mock_httpx_client):
        """Trailing slash in base_url should be removed."""
        client = AgentWorldClient("http://localhost:3000/")
        assert client.base_url == "http://localhost:3000"

    def test_timeout_default(self, mock_httpx_client):
        """Default timeout should be 10.0 seconds."""
        client = AgentWorldClient(BASE_URL)
        assert client.timeout == 10.0

    def test_custom_timeout(self, mock_httpx_client):
        """Custom timeout should be forwarded to httpx.Client."""
        client = AgentWorldClient(BASE_URL, timeout=30.0)
        assert client.timeout == 30.0
