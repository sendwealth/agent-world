"""Tests for action feedback loop: RESTWorldClient routing, GRPCWorldClient
exception propagation, ActionExecutor return-value checking, and
_NoOpWorldClient elimination.

Verifies:
1. Every RESTWorldClient action method sends the correct HTTP request
   (endpoint, method, payload) to the World Engine.
2. RESTWorldClient never silently returns {"status": "standalone"} for
   real action types — errors are raised.
3. HTTP errors (4xx/5xx) are re-raised so ThinkLoop can track them as
   real failures (not masked as success).
4. GRPCWorldClient raises exceptions instead of returning {"status": "error"}.
5. ActionExecutor checks return values for {"status": "error"} and raises.
6. _NoOpWorldClient is not used when a real world_client is provided
   to ThinkLoop (connected mode).
7. _NoOpWorldClient is only used as fallback when world_client=None
   (standalone mode).
"""

from __future__ import annotations

from typing import Any
from unittest.mock import AsyncMock, MagicMock, patch

import httpx
import pytest

from agent_runtime.__main__ import RESTWorldClient
from agent_runtime.core.act import (
    ActionContext,
    ActionExecutor,
    ActionStatus,
    ActionType,
)
from agent_runtime.core.think_loop import Decision, ThinkLoop, ThinkLoopConfig
from agent_runtime.models.agent_state import AgentState
from agent_runtime.survival.instinct import SurvivalInstinct

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def _make_client() -> RESTWorldClient:
    return RESTWorldClient("http://localhost:3000", agent_id="test-agent-123")


# ---------------------------------------------------------------------------
# Test: RESTWorldClient action routing
# ---------------------------------------------------------------------------


class TestRESTWorldClientRouting:
    """Verify each action method routes to the correct endpoint and payload."""

    @pytest.mark.asyncio
    async def test_move_routes_to_action_endpoint(self) -> None:
        client = _make_client()
        with patch.object(client, "_request", new_callable=AsyncMock) as mock:
            mock.return_value = {"action": "move", "success": True}
            await client.move("north")
            mock.assert_called_once_with(
                "POST",
                "/api/v1/agents/test-agent-123/action",
                json={"action": "move", "params": {"direction": "north"}},
            )

    @pytest.mark.asyncio
    async def test_gather_routes_to_action_endpoint(self) -> None:
        client = _make_client()
        with patch.object(client, "_request", new_callable=AsyncMock) as mock:
            mock.return_value = {"action": "gather", "success": True}
            await client.gather("wood")
            mock.assert_called_once_with(
                "POST",
                "/api/v1/agents/test-agent-123/action",
                json={"action": "gather", "params": {"resource_type": "wood"}},
            )

    @pytest.mark.asyncio
    async def test_build_routes_to_action_endpoint(self) -> None:
        client = _make_client()
        with patch.object(client, "_request", new_callable=AsyncMock) as mock:
            mock.return_value = {"action": "build", "success": True}
            await client.build("house", durability=100)
            mock.assert_called_once_with(
                "POST",
                "/api/v1/agents/test-agent-123/action",
                json={
                    "action": "build",
                    "params": {"structure_type": "house", "durability": 100},
                },
            )

    @pytest.mark.asyncio
    async def test_explore_routes_to_action_endpoint(self) -> None:
        client = _make_client()
        with patch.object(client, "_request", new_callable=AsyncMock) as mock:
            mock.return_value = {"action": "explore", "success": True}
            await client.explore({"capabilities": ["gather"]})
            mock.assert_called_once_with(
                "POST",
                "/api/v1/agents/test-agent-123/action",
                json={"action": "explore", "params": {"capabilities": ["gather"]}},
            )

    @pytest.mark.asyncio
    async def test_claim_task_routes_to_action_endpoint(self) -> None:
        client = _make_client()
        with patch.object(client, "_request", new_callable=AsyncMock) as mock:
            mock.return_value = {"action": "claim_task", "success": True}
            await client.claim_task("task-42")
            mock.assert_called_once_with(
                "POST",
                "/api/v1/agents/test-agent-123/action",
                json={"action": "claim_task", "params": {"task_id": "task-42"}},
            )

    @pytest.mark.asyncio
    async def test_submit_task_routes_to_action_endpoint(self) -> None:
        client = _make_client()
        with patch.object(client, "_request", new_callable=AsyncMock) as mock:
            mock.return_value = {"action": "submit_task", "success": True}
            await client.submit_task("task-42", {"output": "done"})
            mock.assert_called_once_with(
                "POST",
                "/api/v1/agents/test-agent-123/action",
                json={
                    "action": "submit_task",
                    "params": {"task_id": "task-42", "result": {"output": "done"}},
                },
            )

    @pytest.mark.asyncio
    async def test_propose_deal_routes_as_trade(self) -> None:
        client = _make_client()
        proposal = {"item": "wood", "price": 10}
        with patch.object(client, "_request", new_callable=AsyncMock) as mock:
            mock.return_value = {"action": "trade", "success": True}
            await client.propose_deal(proposal)
            mock.assert_called_once_with(
                "POST",
                "/api/v1/agents/test-agent-123/action",
                json={"action": "trade", "params": proposal},
            )

    @pytest.mark.asyncio
    async def test_teach_skill_routes_as_communicate(self) -> None:
        client = _make_client()
        with patch.object(client, "_request", new_callable=AsyncMock) as mock:
            mock.return_value = {"action": "communicate", "success": True}
            await client.teach_skill("agent-456", "coding", 3)
            mock.assert_called_once_with(
                "POST",
                "/api/v1/agents/test-agent-123/action",
                json={
                    "action": "communicate",
                    "params": {
                        "target_agent_id": "agent-456",
                        "skill_name": "coding",
                        "level": 3,
                    },
                },
            )

    @pytest.mark.asyncio
    async def test_send_message_routes_to_messages_endpoint(self) -> None:
        client = _make_client()
        payload = {"to": "agent-456", "text": "hello"}
        with patch.object(client, "_request", new_callable=AsyncMock) as mock:
            mock.return_value = {"status": "ok"}
            await client.send_message(payload)
            mock.assert_called_once_with(
                "POST",
                "/api/v1/messages",
                json=payload,
            )

    @pytest.mark.asyncio
    async def test_submit_action_routes_correctly(self) -> None:
        client = _make_client()
        with patch.object(client, "_request", new_callable=AsyncMock) as mock:
            mock.return_value = {"action": "rest", "success": True}
            await client.submit_action("rest", {})
            mock.assert_called_once_with(
                "POST",
                "/api/v1/agents/test-agent-123/action",
                json={"action": "rest", "params": {}},
            )


class TestRESTWorldClientErrorHandling:
    """Verify error propagation: all errors raise so ThinkLoop can track them."""

    @pytest.mark.asyncio
    async def test_connect_error_raises(self) -> None:
        """When World Engine is unreachable, raise so ActionExecutor retry kicks in."""
        client = _make_client()
        with patch("httpx.AsyncClient.request", side_effect=httpx.ConnectError("refused")):
            with pytest.raises(httpx.ConnectError):
                await client.move("north")

    @pytest.mark.asyncio
    async def test_http_404_raises(self) -> None:
        """A 404 (wrong agent_id) must raise, not return silently."""
        client = _make_client()
        response = MagicMock()
        response.status_code = 404
        response.text = "agent not found"
        exc = httpx.HTTPStatusError(
            "404", request=MagicMock(), response=response
        )
        with patch("httpx.AsyncClient.request", side_effect=exc):
            with pytest.raises(httpx.HTTPStatusError):
                await client.move("north")

    @pytest.mark.asyncio
    async def test_http_500_raises(self) -> None:
        """A 500 (server bug) must raise, not return silently."""
        client = _make_client()
        response = MagicMock()
        response.status_code = 500
        response.text = "internal error"
        exc = httpx.HTTPStatusError(
            "500", request=MagicMock(), response=response
        )
        with patch("httpx.AsyncClient.request", side_effect=exc):
            with pytest.raises(httpx.HTTPStatusError):
                await client.gather("food")

    @pytest.mark.asyncio
    async def test_http_400_raises(self) -> None:
        """A 400 (bad request / unknown action) must raise."""
        client = _make_client()
        response = MagicMock()
        response.status_code = 400
        response.text = "unknown action 'fly'"
        exc = httpx.HTTPStatusError(
            "400", request=MagicMock(), response=response
        )
        with patch("httpx.AsyncClient.request", side_effect=exc):
            with pytest.raises(httpx.HTTPStatusError):
                await client.submit_action("fly", {})


class TestRESTWorldClientFormOrgJoinOrg:
    """Verify form_org and join_org route to their own endpoints."""

    @pytest.mark.asyncio
    async def test_form_org_routes_to_orgs_endpoint(self) -> None:
        client = _make_client()
        org_data = {"org_name": "Builders", "org_type": "guild"}
        with patch.object(client, "_request", new_callable=AsyncMock) as mock:
            mock.return_value = {"org_id": "org-1"}
            await client.form_org(org_data)
            mock.assert_called_once_with(
                "POST", "/api/v1/orgs", json=org_data
            )

    @pytest.mark.asyncio
    async def test_join_org_routes_to_join_endpoint(self) -> None:
        client = _make_client()
        member_data = {"role": "member"}
        with patch.object(client, "_request", new_callable=AsyncMock) as mock:
            mock.return_value = {"status": "ok"}
            await client.join_org("org-1", member_data)
            mock.assert_called_once_with(
                "POST", "/api/v1/orgs/org-1/join", json=member_data
            )


# ---------------------------------------------------------------------------
# Test: _NoOpWorldClient elimination in connected mode
# ---------------------------------------------------------------------------


class TrackedWorldClient:
    """A real world client that records whether it was actually called."""

    def __init__(self) -> None:
        self.calls: list[tuple[str, dict[str, Any]]] = []

    async def send_message(self, payload: dict[str, Any]) -> dict[str, Any]:
        self.calls.append(("send_message", payload))
        return {"status": "ok"}

    async def claim_task(self, task_id: str) -> dict[str, Any]:
        self.calls.append(("claim_task", {"task_id": task_id}))
        return {"status": "ok", "task_id": task_id}

    async def submit_task(
        self, task_id: str, result: dict[str, Any]
    ) -> dict[str, Any]:
        self.calls.append(("submit_task", {"task_id": task_id}))
        return {"status": "ok", "task_id": task_id}

    async def propose_deal(self, proposal: dict[str, Any]) -> dict[str, Any]:
        self.calls.append(("propose_deal", proposal))
        return {"status": "ok"}

    async def teach_skill(
        self, target_agent_id: str, skill_name: str, level: int
    ) -> dict[str, Any]:
        self.calls.append(("teach_skill", {"target": target_agent_id}))
        return {"status": "ok"}

    async def explore(self, parameters: dict[str, Any]) -> dict[str, Any]:
        self.calls.append(("explore", parameters))
        return {"status": "ok", "agents": []}

    async def move(self, direction: str) -> dict[str, Any]:
        self.calls.append(("move", {"direction": direction}))
        return {"status": "ok", "direction": direction}

    async def gather(self, resource_type: str) -> dict[str, Any]:
        self.calls.append(("gather", {"resource_type": resource_type}))
        return {"status": "ok", "resource_type": resource_type}

    async def build(self, structure_type: str, **kwargs: Any) -> dict[str, Any]:
        self.calls.append(("build", {"structure_type": structure_type}))
        return {"status": "ok", "structure_type": structure_type}


class TestNoOpWorldClientElimination:
    """Verify that in connected mode, the real world client is used
    (not _NoOpWorldClient)."""

    @pytest.mark.asyncio
    async def test_connected_mode_uses_real_client(self) -> None:
        """When world_client is provided, ThinkLoop must use it, not _NoOp."""
        state = AgentState(name="ConnectedAgent", tokens=500, max_tokens=1000)
        survival = SurvivalInstinct()
        executor = ActionExecutor()
        tracked = TrackedWorldClient()

        class MoveProvider:
            async def decide(self, *args: Any, **kwargs: Any) -> Decision:
                return Decision(
                    action_type=ActionType.MOVE,
                    parameters={"direction": "north"},
                    reasoning="test move",
                )

        loop = ThinkLoop(
            state=state,
            survival=survival,
            executor=executor,
            config=ThinkLoopConfig(tick_interval=0.0, max_ticks=1),
            world_client=tracked,
            decision_provider=MoveProvider(),
        )
        await loop.run()

        # The real client should have been called
        assert len(tracked.calls) > 0, (
            "Expected the real world client to be called, but it wasn't. "
            "_NoOpWorldClient may have been used instead."
        )
        assert tracked.calls[0][0] == "move"
        assert tracked.calls[0][1] == {"direction": "north"}

    @pytest.mark.asyncio
    async def test_standalone_mode_uses_noop(self) -> None:
        """When world_client is None, ThinkLoop uses _NoOpWorldClient
        (agent runs but actions have no effect)."""
        state = AgentState(name="StandaloneAgent", tokens=500, max_tokens=1000)
        survival = SurvivalInstinct()
        executor = ActionExecutor()

        class GatherProvider:
            async def decide(self, *args: Any, **kwargs: Any) -> Decision:
                return Decision(
                    action_type=ActionType.GATHER,
                    parameters={"resource_type": "wood"},
                    reasoning="test gather",
                )

        loop = ThinkLoop(
            state=state,
            survival=survival,
            executor=executor,
            config=ThinkLoopConfig(tick_interval=0.0, max_ticks=1),
            world_client=None,  # No client → _NoOpWorldClient
            decision_provider=GatherProvider(),
        )
        await loop.run()

        # Should complete without errors (NoOp is fine for standalone)
        assert loop.tick == 1
        assert loop.total_errors == 0

        # But action history shows the action was executed
        history = executor.history
        assert len(history) > 0
        assert history[-1].action_type == ActionType.GATHER
        assert history[-1].status == ActionStatus.SUCCESS

    @pytest.mark.asyncio
    async def test_connected_mode_gather_reaches_client(self) -> None:
        """Gather in connected mode reaches the real world client."""
        state = AgentState(name="GatherAgent", tokens=500, max_tokens=1000)
        survival = SurvivalInstinct()
        executor = ActionExecutor()
        tracked = TrackedWorldClient()

        class GatherProvider:
            async def decide(self, *args: Any, **kwargs: Any) -> Decision:
                return Decision(
                    action_type=ActionType.GATHER,
                    parameters={"resource_type": "food"},
                    reasoning="hungry",
                )

        loop = ThinkLoop(
            state=state,
            survival=survival,
            executor=executor,
            config=ThinkLoopConfig(tick_interval=0.0, max_ticks=1),
            world_client=tracked,
            decision_provider=GatherProvider(),
        )
        await loop.run()

        assert tracked.calls[0][0] == "gather"
        assert tracked.calls[0][1] == {"resource_type": "food"}

    @pytest.mark.asyncio
    async def test_connected_mode_action_failure_is_recorded(self) -> None:
        """When the real world client raises, ThinkLoop records the failure."""
        state = AgentState(name="FailAgent", tokens=500, max_tokens=1000)
        survival = SurvivalInstinct()
        executor = ActionExecutor()

        class FailingClient:
            async def move(self, direction: str) -> dict[str, Any]:
                raise ConnectionError("World Engine unreachable")

        class MoveProvider:
            async def decide(self, *args: Any, **kwargs: Any) -> Decision:
                return Decision(
                    action_type=ActionType.MOVE,
                    parameters={"direction": "north"},
                    reasoning="test",
                )

        loop = ThinkLoop(
            state=state,
            survival=survival,
            executor=executor,
            config=ThinkLoopConfig(tick_interval=0.0, max_ticks=1),
            world_client=FailingClient(),
            decision_provider=MoveProvider(),
        )
        await loop.run()

        # Action should have been recorded as failed (retry_exhausted)
        history = executor.history
        assert len(history) > 0
        assert history[-1].status == ActionStatus.RETRY_EXHAUSTED
        assert "World Engine unreachable" in (history[-1].error or "")


# ---------------------------------------------------------------------------
# Test: GRPCWorldClient raises on error (no swallowing)
# ---------------------------------------------------------------------------


class TestGRPCWorldClientRaises:
    """Verify GRPCWorldClient raises exceptions instead of returning
    {"status": "error"} dicts, so ActionExecutor retry logic works."""

    @pytest.mark.asyncio
    async def test_move_raises_on_error(self) -> None:
        from agent_runtime.a2a.world_client import GRPCWorldClient

        mock_a2a = MagicMock()
        mock_a2a.send_message = AsyncMock(side_effect=ConnectionError("grpc down"))
        client = GRPCWorldClient(mock_a2a)

        with pytest.raises(ConnectionError, match="grpc down"):
            await client.move("north")

    @pytest.mark.asyncio
    async def test_gather_raises_on_error(self) -> None:
        from agent_runtime.a2a.world_client import GRPCWorldClient

        mock_a2a = MagicMock()
        mock_a2a.send_message = AsyncMock(side_effect=RuntimeError("server error"))
        client = GRPCWorldClient(mock_a2a)

        with pytest.raises(RuntimeError, match="server error"):
            await client.gather("wood")

    @pytest.mark.asyncio
    async def test_explore_raises_on_error(self) -> None:
        from agent_runtime.a2a.world_client import GRPCWorldClient

        mock_a2a = MagicMock()
        mock_a2a.discover = AsyncMock(side_effect=ConnectionError("unreachable"))
        client = GRPCWorldClient(mock_a2a)

        with pytest.raises(ConnectionError):
            await client.explore({})

    @pytest.mark.asyncio
    async def test_broadcast_message_raises_on_error(self) -> None:
        from agent_runtime.a2a.world_client import GRPCWorldClient

        mock_a2a = MagicMock()
        mock_a2a.send_message = AsyncMock(side_effect=Exception("fail"))
        client = GRPCWorldClient(mock_a2a)

        with pytest.raises(Exception, match="fail"):
            await client.broadcast_message({"type": "SOS"})


# ---------------------------------------------------------------------------
# Test: ActionExecutor checks return value for {"status": "error"}
# ---------------------------------------------------------------------------


class TestActionExecutorErrorStatusCheck:
    """Verify ActionExecutor treats {"status": "error"} returns as failures."""

    @pytest.mark.asyncio
    async def test_error_status_return_triggers_retry_exhausted(self) -> None:
        """A world client returning {"status": "error"} should cause RETRY_EXHAUSTED."""

        class ErrorReturningClient:
            async def move(self, direction: str) -> dict[str, Any]:
                return {"status": "error", "error": "something went wrong"}

        executor = ActionExecutor(max_retries=2, retry_delay=0.0)
        state = AgentState(name="TestAgent", tokens=500, max_tokens=1000)
        context = ActionContext(
            agent=state,
            world=ErrorReturningClient(),
            parameters={"direction": "north"},
        )
        result = await executor.execute(ActionType.MOVE, context)

        assert result.status == ActionStatus.RETRY_EXHAUSTED
        assert "something went wrong" in (result.error or "")

    @pytest.mark.asyncio
    async def test_ok_status_return_succeeds(self) -> None:
        """A world client returning {"status": "ok"} should succeed."""

        class OkClient:
            async def gather(self, resource_type: str) -> dict[str, Any]:
                return {"status": "ok", "resource_type": resource_type}

        executor = ActionExecutor()
        state = AgentState(name="TestAgent", tokens=500, max_tokens=1000)
        context = ActionContext(
            agent=state,
            world=OkClient(),
            parameters={"resource_type": "wood"},
        )
        result = await executor.execute(ActionType.GATHER, context)

        assert result.status == ActionStatus.SUCCESS

    @pytest.mark.asyncio
    async def test_mixed_status_with_retry(self) -> None:
        """Error return then success should succeed on retry."""

        class FlakyClient:
            def __init__(self) -> None:
                self._call_count = 0

            async def move(self, direction: str) -> dict[str, Any]:
                self._call_count += 1
                if self._call_count == 1:
                    return {"status": "error", "error": "transient"}
                return {"status": "ok", "direction": direction}

        executor = ActionExecutor(max_retries=3, retry_delay=0.0)
        state = AgentState(name="TestAgent", tokens=500, max_tokens=1000)
        context = ActionContext(
            agent=state,
            world=FlakyClient(),
            parameters={"direction": "north"},
        )
        result = await executor.execute(ActionType.MOVE, context)

        assert result.status == ActionStatus.SUCCESS
        assert result.attempts == 2
