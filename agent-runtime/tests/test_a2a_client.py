"""Tests for the A2A gRPC client and Think Loop integration.

Covers:
- A2AClientConfig creation and defaults
- A2AClient: construction, start/stop lifecycle
- A2AClient: WorldClientProtocol methods (send_message, claim_task, etc.)
- A2AClient: broadcast_message (A2AClientProtocol)
- A2AClient: get_unread_messages
- A2AClient: auto-reconnect with exponential backoff
- GRpcPerceptionProvider: fetches messages via client
- GRpcPerceptionProvider: graceful fallback on failure
- ThinkLoop with A2AClient: full cycle integration
- ThinkLoop with A2AClient: survival bypass uses client
"""

from __future__ import annotations

import asyncio
from typing import Any
from unittest.mock import AsyncMock, patch

import pytest

from agent_runtime.a2a.client import A2AClient, A2AClientConfig
from agent_runtime.a2a.perception import GRpcPerceptionProvider
from agent_runtime.core.act import ActionExecutor, ActionType
from agent_runtime.core.think_loop import (
    Decision,
    Perception,
    ThinkLoop,
    ThinkLoopConfig,
)
from agent_runtime.models.agent_state import AgentState
from agent_runtime.survival.instinct import (
    SurvivalAction,
    SurvivalInstinct,
    SurvivalMode,
)


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def make_state(
    tokens: int = 500,
    max_tokens: int = 1000,
    *,
    name: str = "TestAgent",
) -> AgentState:
    return AgentState(
        name=name,
        tokens=tokens,
        max_tokens=max_tokens,
        money=50.0,
        health=100.0,
    )


# ---------------------------------------------------------------------------
# A2AClientConfig
# ---------------------------------------------------------------------------


class TestA2AClientConfig:
    def test_defaults(self):
        cfg = A2AClientConfig()
        assert cfg.server_address == "localhost:50051"
        assert cfg.heartbeat_interval == 30.0
        assert cfg.reconnect_backoff_base == 1.0
        assert cfg.reconnect_backoff_max == 60.0
        assert cfg.reconnect_max_retries == 0
        assert cfg.request_timeout == 10.0
        assert cfg.streaming_timeout == 30.0

    def test_custom(self):
        cfg = A2AClientConfig(
            server_address="engine:50051",
            heartbeat_interval=15.0,
            reconnect_backoff_base=0.5,
            reconnect_max_retries=5,
        )
        assert cfg.server_address == "engine:50051"
        assert cfg.heartbeat_interval == 15.0
        assert cfg.reconnect_backoff_base == 0.5
        assert cfg.reconnect_max_retries == 5


# ---------------------------------------------------------------------------
# A2AClient — construction & properties
# ---------------------------------------------------------------------------


class TestA2AClientConstruction:
    def test_default_config(self):
        client = A2AClient(agent_id="agent-1")
        assert client.agent_id == "agent-1"
        assert client.config.server_address == "localhost:50051"
        assert client.connected is False

    def test_custom_config(self):
        cfg = A2AClientConfig(server_address="engine:1234")
        client = A2AClient(config=cfg, agent_id="agent-2")
        assert client.config.server_address == "engine:1234"


# ---------------------------------------------------------------------------
# A2AClient — start / stop lifecycle
# ---------------------------------------------------------------------------


class TestA2AClientLifecycle:
    @pytest.mark.asyncio
    async def test_start_and_stop(self):
        """Client should start and stop without error."""
        client = A2AClient(
            config=A2AClientConfig(
                heartbeat_interval=0,  # disable heartbeat for test
            ),
            agent_id="test-agent",
        )

        # Patch the connection to avoid needing a real server
        with patch.object(client, "_connect", new_callable=AsyncMock):
            await client.start()
            assert client._running is True

        await client.stop()
        assert client._running is False
        assert client.connected is False

    @pytest.mark.asyncio
    async def test_stop_cancels_background_tasks(self):
        """Stop should cancel stream and heartbeat tasks."""
        client = A2AClient(
            config=A2AClientConfig(heartbeat_interval=0.01),
            agent_id="test-agent",
        )

        with patch.object(client, "_connect", new_callable=AsyncMock):
            await client.start()
            assert client._stream_task is not None
            assert client._heartbeat_task is not None

        await client.stop()
        assert client._stream_task.done()
        assert client._heartbeat_task.done()


# ---------------------------------------------------------------------------
# A2AClient — WorldClientProtocol methods
# ---------------------------------------------------------------------------


class TestA2AClientMethods:
    @pytest.mark.asyncio
    async def test_send_message(self):
        client = A2AClient(agent_id="agent-1")
        result = await client.send_message({"to_agent": "agent-2", "content": "hi"})
        assert result["status"] == "ok"

    @pytest.mark.asyncio
    async def test_claim_task(self):
        client = A2AClient(agent_id="agent-1")
        result = await client.claim_task("task-123")
        assert result["status"] == "ok"

    @pytest.mark.asyncio
    async def test_submit_task(self):
        client = A2AClient(agent_id="agent-1")
        result = await client.submit_task("task-123", {"output": "done"})
        assert result["status"] == "ok"

    @pytest.mark.asyncio
    async def test_propose_deal(self):
        client = A2AClient(agent_id="agent-1")
        result = await client.propose_deal({"terms": "50 tokens"})
        assert result["status"] == "ok"

    @pytest.mark.asyncio
    async def test_teach_skill(self):
        client = A2AClient(agent_id="agent-1")
        result = await client.teach_skill("agent-2", "python", 3)
        assert result["status"] == "ok"

    @pytest.mark.asyncio
    async def test_explore(self):
        client = A2AClient(agent_id="agent-1")
        result = await client.explore({"region": "north"})
        assert result["status"] == "ok"


# ---------------------------------------------------------------------------
# A2AClient — broadcast_message (A2AClientProtocol)
# ---------------------------------------------------------------------------


class TestA2AClientBroadcast:
    @pytest.mark.asyncio
    async def test_broadcast_message(self):
        client = A2AClient(agent_id="agent-1")
        result = await client.broadcast_message({
            "type": "INFORM",
            "payload": {"content": "SOS"},
        })
        assert result["status"] == "ok"


# ---------------------------------------------------------------------------
# A2AClient — get_unread_messages
# ---------------------------------------------------------------------------


class TestA2AClientMessages:
    @pytest.mark.asyncio
    async def test_get_unread_messages_empty(self):
        client = A2AClient(agent_id="agent-1")
        messages = await client.get_unread_messages()
        assert messages == []

    @pytest.mark.asyncio
    async def test_get_unread_messages_drains_queue(self):
        client = A2AClient(agent_id="agent-1")
        # Manually push messages into the queue
        await client._message_queue.put({"from": "bob", "text": "hello"})
        await client._message_queue.put({"from": "alice", "text": "hi"})

        messages = await client.get_unread_messages()
        assert len(messages) == 2
        assert messages[0]["from"] == "bob"
        assert messages[1]["from"] == "alice"

        # Queue should now be empty
        messages2 = await client.get_unread_messages()
        assert messages2 == []


# ---------------------------------------------------------------------------
# A2AClient — RPC retry and error handling
# ---------------------------------------------------------------------------


class TestA2AClientRetry:
    @pytest.mark.asyncio
    async def test_rpc_returns_error_after_retries(self):
        """When _call_stub always fails, rpc returns error status."""
        client = A2AClient(
            config=A2AClientConfig(reconnect_backoff_base=0.001),
            agent_id="agent-1",
        )

        async def _failing_stub(method: str, message: dict) -> dict[str, Any]:
            raise OSError("connection refused")

        with patch.object(client, "_call_stub", side_effect=_failing_stub):
            with patch.object(client, "_ensure_connected", new_callable=AsyncMock):
                result = await client._rpc_with_retry("TestRPC", {}, max_retries=2)

        assert result["status"] == "error"
        assert "TestRPC" in result["error"]
        assert "2 attempts" in result["error"]


# ---------------------------------------------------------------------------
# GRpcPerceptionProvider
# ---------------------------------------------------------------------------


class TestGRpcPerceptionProvider:
    @pytest.mark.asyncio
    async def test_perceive_with_messages(self):
        """Provider should fetch messages via the A2A client."""
        client = A2AClient(agent_id="agent-1")
        await client._message_queue.put({"from": "bob", "content": "hello"})

        provider = GRpcPerceptionProvider(client)
        state = make_state(tokens=500, max_tokens=1000)
        perception = await provider.perceive(state, tick=5)

        assert len(perception.messages) == 1
        assert perception.messages[0]["from"] == "bob"
        assert perception.token_balance == 500
        assert perception.token_ratio == 0.5
        assert perception.tick == 5

    @pytest.mark.asyncio
    async def test_perceive_empty_messages(self):
        """Provider should work fine with no messages."""
        client = A2AClient(agent_id="agent-1")
        provider = GRpcPerceptionProvider(client)
        state = make_state()
        perception = await provider.perceive(state, tick=1)

        assert perception.messages == []
        assert perception.token_balance == 500

    @pytest.mark.asyncio
    async def test_perceive_graceful_fallback(self):
        """If get_unread_messages raises, provider should return empty messages."""
        client = A2AClient(agent_id="agent-1")

        async def _broken():
            raise RuntimeError("gRPC failure")

        with patch.object(client, "get_unread_messages", side_effect=_broken):
            provider = GRpcPerceptionProvider(client)
            state = make_state()
            perception = await provider.perceive(state, tick=1)

        assert perception.messages == []
        assert perception.token_balance == 500


# ---------------------------------------------------------------------------
# ThinkLoop — A2AClient integration
# ---------------------------------------------------------------------------


class TestThinkLoopWithA2AClient:
    @pytest.mark.asyncio
    async def test_single_tick_with_a2a_client(self):
        """ThinkLoop should work with a real A2A client injected."""
        state = make_state(tokens=5000, max_tokens=10000)
        client = A2AClient(agent_id=str(state.id))
        loop = ThinkLoop(
            state=state,
            survival=SurvivalInstinct(),
            executor=ActionExecutor(),
            config=ThinkLoopConfig(tick_interval=0.0),
            a2a_client=client,
        )
        await loop.run(max_ticks=1)
        assert loop.tick == 1
        assert loop.total_errors == 0

    @pytest.mark.asyncio
    async def test_ten_ticks_with_a2a_client(self):
        """ThinkLoop should run stably for 10 ticks with A2AClient."""
        state = make_state(tokens=5000, max_tokens=10000)
        client = A2AClient(agent_id=str(state.id))
        loop = ThinkLoop(
            state=state,
            survival=SurvivalInstinct(),
            executor=ActionExecutor(),
            config=ThinkLoopConfig(tick_interval=0.0),
            a2a_client=client,
        )
        await loop.run(max_ticks=10)
        assert loop.tick == 10
        assert loop.total_errors == 0

    @pytest.mark.asyncio
    async def test_with_grpc_perception_provider(self):
        """ThinkLoop with GRpcPerceptionProvider should perceive messages."""
        state = make_state(tokens=5000, max_tokens=10000)
        client = A2AClient(agent_id=str(state.id))
        await client._message_queue.put({"from": "bob", "content": "hello"})

        provider = GRpcPerceptionProvider(client)
        loop = ThinkLoop(
            state=state,
            survival=SurvivalInstinct(),
            executor=ActionExecutor(),
            config=ThinkLoopConfig(tick_interval=0.0),
            a2a_client=client,
            perception_provider=provider,
        )
        await loop.run(max_ticks=5)
        assert loop.tick == 5
        assert loop.total_errors == 0

    @pytest.mark.asyncio
    async def test_survival_bypass_with_a2a_client(self):
        """PANIC mode should use A2A client for emergency actions."""
        state = make_state(tokens=5, max_tokens=100)
        client = A2AClient(agent_id=str(state.id))

        # Track if broadcast was called
        broadcast_calls: list[dict] = []
        original_broadcast = client.broadcast_message

        async def tracking_broadcast(payload: dict) -> dict[str, Any]:
            broadcast_calls.append(payload)
            return await original_broadcast(payload)

        with patch.object(client, "broadcast_message", side_effect=tracking_broadcast):
            loop = ThinkLoop(
                state=state,
                survival=SurvivalInstinct(),
                executor=ActionExecutor(),
                config=ThinkLoopConfig(tick_interval=0.0),
                a2a_client=client,
            )
            await loop.run(max_ticks=3)

        assert loop.tick == 3
        # Broadcast should have been called for SOS (survival cooldown means
        # it fires once per cooldown period)
        assert len(broadcast_calls) >= 1

    @pytest.mark.asyncio
    async def test_act_phase_uses_a2a_client(self):
        """Act phase should route through the A2A client."""
        state = make_state(tokens=5000, max_tokens=10000)
        client = A2AClient(agent_id=str(state.id))

        # Track send_message calls
        send_calls: list[dict] = []

        class AlwaysSendMessage:
            async def decide(self, state_ref, perception, survival):
                return Decision(
                    action_type=ActionType.SEND_MESSAGE,
                    parameters={"payload": {"to_agent": "bob", "content": "hello"}},
                )

        original_send = client.send_message

        async def tracking_send(payload: dict) -> dict[str, Any]:
            send_calls.append(payload)
            return await original_send(payload)

        with patch.object(client, "send_message", side_effect=tracking_send):
            loop = ThinkLoop(
                state=state,
                survival=SurvivalInstinct(),
                executor=ActionExecutor(),
                config=ThinkLoopConfig(tick_interval=0.0),
                a2a_client=client,
                decision_provider=AlwaysSendMessage(),
            )
            await loop.run(max_ticks=3)

        assert loop.tick == 3
        assert len(send_calls) == 3
        assert send_calls[0]["to_agent"] == "bob"


# ---------------------------------------------------------------------------
# ThinkLoop — backward compatibility (no A2A client)
# ---------------------------------------------------------------------------


class TestThinkLoopBackwardCompat:
    @pytest.mark.asyncio
    async def test_no_a2a_client_uses_noop(self):
        """Without A2A client, ThinkLoop should still work with _NoOpWorldClient."""
        state = make_state(tokens=5000, max_tokens=10000)
        loop = ThinkLoop(
            state=state,
            survival=SurvivalInstinct(),
            executor=ActionExecutor(),
            config=ThinkLoopConfig(tick_interval=0.0),
        )
        await loop.run(max_ticks=5)
        assert loop.tick == 5
        assert loop.total_errors == 0

    @pytest.mark.asyncio
    async def test_100_ticks_no_a2a_client(self):
        """Stability test: 100 ticks without A2A client still works."""
        state = make_state(tokens=10000, max_tokens=20000)
        loop = ThinkLoop(
            state=state,
            survival=SurvivalInstinct(),
            executor=ActionExecutor(),
            config=ThinkLoopConfig(tick_interval=0.0),
        )
        await loop.run(max_ticks=100)
        assert loop.tick == 100
        assert loop.total_errors == 0
