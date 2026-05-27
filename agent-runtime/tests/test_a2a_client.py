"""Tests for the A2A gRPC client module.

Covers:
- A2AClientConfig and RetryPolicy defaults and custom values
- build_a2a_message: all fields, auto-generated nonce and id
- a2a_message_to_dict: protobuf → dict conversion, JSON payload decode
- A2AClient: connect/close lifecycle, connected property
- A2AClient: send_message and discover with mock stubs
- A2AClient: retry logic (UNAVAILABLE retries, non-retryable fails immediately)
- A2AClient: streaming start/stop, stream_send without streaming raises
- GRPCWorldClient: all WorldClientProtocol methods
- GRPCWorldClient: error handling returns error dicts
- GRPCWorldClient: broadcast_message (A2AClientProtocol)
- GRPCPerceptionProvider: perceive with messages and market state
- GRPCPerceptionProvider: discover failure returns empty market_state
- ThinkLoop integration: world_client injection, gRPC calls, error resilience
- SurvivalInstinct integration: PANIC broadcast via GRPCWorldClient
"""

from __future__ import annotations

from unittest.mock import AsyncMock, MagicMock, patch

import pytest
from protocol.gen.python import a2a_pb2

from agent_runtime.a2a.client import A2AClient
from agent_runtime.a2a.config import A2AClientConfig, RetryPolicy
from agent_runtime.a2a.message import a2a_message_to_dict, build_a2a_message
from agent_runtime.a2a.perception import GRPCPerceptionProvider
from agent_runtime.a2a.world_client import GRPCWorldClient
from agent_runtime.core.act import (
    ActionExecutor,
    ActionType,
)
from agent_runtime.core.think_loop import (
    Decision,
    ThinkLoop,
    ThinkLoopConfig,
)
from agent_runtime.models.agent_state import AgentState
from agent_runtime.survival.instinct import (
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


def make_config(**kwargs) -> A2AClientConfig:
    defaults = {"server_address": "localhost:50051", "agent_id": "test-agent"}
    defaults.update(kwargs)
    return A2AClientConfig(**defaults)


# ---------------------------------------------------------------------------
# RetryPolicy
# ---------------------------------------------------------------------------


class TestRetryPolicy:
    def test_defaults(self):
        rp = RetryPolicy()
        assert rp.max_retries == 3
        assert rp.base_delay == 0.5
        assert rp.max_delay == 10.0
        assert rp.jitter == 0.2
        assert "UNAVAILABLE" in rp.retryable_codes

    def test_custom(self):
        rp = RetryPolicy(max_retries=5, base_delay=1.0)
        assert rp.max_retries == 5
        assert rp.base_delay == 1.0


# ---------------------------------------------------------------------------
# A2AClientConfig
# ---------------------------------------------------------------------------


class TestA2AClientConfig:
    def test_defaults(self):
        cfg = A2AClientConfig()
        assert cfg.server_address == "localhost:50051"
        assert cfg.agent_id == ""
        assert cfg.timeout == 5.0
        assert cfg.enable_streaming is True
        assert cfg.stream_reconnect_delay == 2.0

    def test_custom(self):
        cfg = A2AClientConfig(
            server_address="engine:9999",
            agent_id="alice",
            timeout=10.0,
        )
        assert cfg.server_address == "engine:9999"
        assert cfg.agent_id == "alice"
        assert cfg.timeout == 10.0

    def test_nested_retry_policy(self):
        rp = RetryPolicy(max_retries=7)
        cfg = A2AClientConfig(retry_policy=rp)
        assert cfg.retry_policy.max_retries == 7


# ---------------------------------------------------------------------------
# build_a2a_message
# ---------------------------------------------------------------------------


class TestBuildA2AMessage:
    def test_basic_message(self):
        msg = build_a2a_message(
            from_agent="alice",
            to_agent="bob",
            message_type=a2a_pb2.INFORM,
            payload={"text": "hello"},
        )
        assert msg.from_agent == "alice"
        assert msg.to_agent == "bob"
        assert msg.type == a2a_pb2.INFORM
        assert msg.id != ""
        assert msg.nonce != ""
        assert msg.timestamp > 0

    def test_broadcast_message(self):
        msg = build_a2a_message(
            from_agent="alice",
            message_type=a2a_pb2.PROPOSE,
        )
        assert msg.to_agent == ""

    def test_custom_nonce_and_signature(self):
        msg = build_a2a_message(
            from_agent="alice",
            nonce="custom-nonce",
            signature="deadbeef",
        )
        assert msg.nonce == "custom-nonce"
        assert msg.signature == "deadbeef"

    def test_auto_generated_nonce(self):
        msg1 = build_a2a_message(from_agent="alice")
        msg2 = build_a2a_message(from_agent="alice")
        assert msg1.nonce != msg2.nonce

    def test_empty_payload(self):
        msg = build_a2a_message(from_agent="alice")
        assert msg.payload == b"{}"

    def test_all_message_types(self):
        types = [
            a2a_pb2.DISCOVER,
            a2a_pb2.PROPOSE,
            a2a_pb2.ACCEPT,
            a2a_pb2.REJECT,
            a2a_pb2.INFORM,
            a2a_pb2.TEACH,
            a2a_pb2.REPRODUCE,
            a2a_pb2.WILL,
            a2a_pb2.THREAT,
        ]
        for mt in types:
            msg = build_a2a_message(from_agent="alice", message_type=mt)
            assert msg.type == mt


# ---------------------------------------------------------------------------
# a2a_message_to_dict
# ---------------------------------------------------------------------------


class TestA2AMessageToDict:
    def test_round_trip(self):
        original = build_a2a_message(
            from_agent="alice",
            to_agent="bob",
            message_type=a2a_pb2.INFORM,
            payload={"key": "value", "num": 42},
        )
        d = a2a_message_to_dict(original)
        assert d["from_agent"] == "alice"
        assert d["to_agent"] == "bob"
        assert d["type"] == a2a_pb2.INFORM
        assert d["payload"] == {"key": "value", "num": 42}
        assert d["nonce"] == original.nonce

    def test_empty_payload(self):
        msg = build_a2a_message(from_agent="alice")
        d = a2a_message_to_dict(msg)
        assert d["payload"] == {}

    def test_invalid_json_payload(self):
        msg = a2a_pb2.A2AMessage(
            from_agent="alice",
            payload=b"not json",
        )
        d = a2a_message_to_dict(msg)
        assert d["payload"] == {}


# ---------------------------------------------------------------------------
# A2AClient — lifecycle
# ---------------------------------------------------------------------------


class TestA2AClientLifecycle:
    @pytest.mark.asyncio
    async def test_connect_and_close(self):
        client = A2AClient(make_config())
        assert not client.connected

        mock_channel = MagicMock()
        mock_channel.close = AsyncMock()
        with patch("agent_runtime.a2a.client.grpc.aio.insecure_channel", return_value=mock_channel):
            await client.connect()
            assert client.connected
            await client.close()
            mock_channel.close.assert_awaited_once()

    @pytest.mark.asyncio
    async def test_close_without_connect(self):
        client = A2AClient(make_config())
        await client.close()  # should not raise

    @pytest.mark.asyncio
    async def test_async_context_manager(self):
        mock_channel = MagicMock()
        mock_channel.close = AsyncMock()
        with patch("agent_runtime.a2a.client.grpc.aio.insecure_channel", return_value=mock_channel):
            async with A2AClient(make_config()) as client:
                assert client.connected
            # After exiting the context, channel should be closed
            assert not client.connected
            mock_channel.close.assert_awaited_once()


# ---------------------------------------------------------------------------
# A2AClient — synchronous RPCs
# ---------------------------------------------------------------------------


class TestA2AClientSyncRPCs:
    @pytest.mark.asyncio
    async def test_send_message(self):
        client = A2AClient(make_config())
        mock_ack = a2a_pb2.MessageAck(received=True)

        with patch("agent_runtime.a2a.client.grpc.aio"):
            await client.connect()
            client._stub = MagicMock()
            client._stub.SendMessage = AsyncMock(return_value=mock_ack)

            ack = await client.send_message(
                to_agent="bob",
                message_type=a2a_pb2.INFORM,
                payload={"text": "hi"},
            )
            assert ack.received is True

    @pytest.mark.asyncio
    async def test_discover(self):
        client = A2AClient(make_config())
        mock_response = a2a_pb2.DiscoverResponse(
            agents=[
                a2a_pb2.AgentInfo(agent_id="bob", name="Bob", tokens=500),
            ]
        )

        with patch("agent_runtime.a2a.client.grpc.aio"):
            await client.connect()
            client._stub = MagicMock()
            client._stub.Discover = AsyncMock(return_value=mock_response)

            response = await client.discover()
            assert len(response.agents) == 1
            assert response.agents[0].agent_id == "bob"


# ---------------------------------------------------------------------------
# A2AClient — retry logic
# ---------------------------------------------------------------------------


class TestA2AClientRetry:
    @pytest.mark.asyncio
    async def test_retries_on_unavailable(self):
        import grpc as real_grpc

        client = A2AClient(make_config(retry_policy=RetryPolicy(max_retries=3, base_delay=0.01)))

        with patch("agent_runtime.a2a.client.grpc.aio.insecure_channel"):
            await client.connect()
            client._stub = MagicMock()

            mock_ack = a2a_pb2.MessageAck(received=True)
            call_count = 0

            async def fake_send(*args, **kwargs):
                nonlocal call_count
                call_count += 1
                if call_count == 1:
                    raise real_grpc.aio.AioRpcError(
                        real_grpc.StatusCode.UNAVAILABLE,
                        real_grpc.aio.Metadata(),
                        real_grpc.aio.Metadata(),
                        details="service unavailable",
                        debug_error_string="unavailable",
                    )
                return mock_ack

            client._stub.SendMessage = fake_send

            ack = await client.send_message(to_agent="bob")
            assert call_count == 2
            assert ack.received is True

    @pytest.mark.asyncio
    async def test_non_retryable_fails_immediately(self):
        import grpc as real_grpc

        client = A2AClient(make_config(retry_policy=RetryPolicy(max_retries=3, base_delay=0.01)))

        with patch("agent_runtime.a2a.client.grpc.aio.insecure_channel"):
            await client.connect()
            client._stub = MagicMock()

            async def fake_send(*args, **kwargs):
                raise real_grpc.aio.AioRpcError(
                    real_grpc.StatusCode.PERMISSION_DENIED,
                    real_grpc.aio.Metadata(),
                    real_grpc.aio.Metadata(),
                    details="permission denied",
                    debug_error_string="denied",
                )

            client._stub.SendMessage = fake_send

            with pytest.raises(Exception):
                await client.send_message(to_agent="bob")


# ---------------------------------------------------------------------------
# A2AClient — streaming
# ---------------------------------------------------------------------------


class TestA2AClientStreaming:
    @pytest.mark.asyncio
    async def test_start_and_stop_streaming(self):
        client = A2AClient(make_config())

        with patch("agent_runtime.a2a.client.grpc.aio"):
            await client.connect()
            assert not client.streaming
            await client.start_streaming()
            assert client.streaming
            await client.stop_streaming()
            assert not client.streaming

    @pytest.mark.asyncio
    async def test_stream_send_without_streaming_raises(self):
        client = A2AClient(make_config())
        msg = build_a2a_message(from_agent="alice")

        with pytest.raises(RuntimeError, match="Streaming is not active"):
            await client.stream_send(msg)

    def test_drain_incoming_returns_empty_when_no_messages(self):
        client = A2AClient(make_config())
        assert client.drain_incoming() == []

    def test_drain_incoming_returns_queued_messages(self):
        client = A2AClient(make_config())
        msg1 = build_a2a_message(from_agent="bob")
        msg2 = build_a2a_message(from_agent="carol")
        client._incoming_queue.put_nowait(msg1)
        client._incoming_queue.put_nowait(msg2)

        drained = client.drain_incoming()
        assert len(drained) == 2
        assert drained[0].from_agent == "bob"
        assert drained[1].from_agent == "carol"
        # Queue should be empty after drain
        assert client.drain_incoming() == []


# ---------------------------------------------------------------------------
# GRPCWorldClient — all WorldClientProtocol methods
# ---------------------------------------------------------------------------


class TestGRPCWorldClient:
    def _make_world_client(self) -> tuple[GRPCWorldClient, MagicMock]:
        a2a = MagicMock(spec=A2AClient)
        world = GRPCWorldClient(a2a)
        return world, a2a

    @pytest.mark.asyncio
    async def test_send_message_success(self):
        world, a2a = self._make_world_client()
        a2a.send_message = AsyncMock(
            return_value=a2a_pb2.MessageAck(received=True)
        )

        result = await world.send_message({"to_agent": "bob", "payload": {"text": "hi"}})
        assert result["status"] == "ok"
        assert result["received"] is True

    @pytest.mark.asyncio
    async def test_send_message_failure(self):
        world, a2a = self._make_world_client()
        a2a.send_message = AsyncMock(side_effect=ConnectionError("network down"))

        result = await world.send_message({"to_agent": "bob"})
        assert result["status"] == "error"
        assert "network down" in result["error"]

    @pytest.mark.asyncio
    async def test_claim_task(self):
        world, a2a = self._make_world_client()
        a2a.send_message = AsyncMock(
            return_value=a2a_pb2.MessageAck(received=True)
        )

        result = await world.claim_task("task-123")
        assert result["status"] == "ok"
        assert result["task_id"] == "task-123"

    @pytest.mark.asyncio
    async def test_claim_task_failure(self):
        world, a2a = self._make_world_client()
        a2a.send_message = AsyncMock(side_effect=Exception("fail"))

        result = await world.claim_task("task-123")
        assert result["status"] == "error"

    @pytest.mark.asyncio
    async def test_submit_task(self):
        world, a2a = self._make_world_client()
        a2a.send_message = AsyncMock(
            return_value=a2a_pb2.MessageAck(received=True)
        )

        result = await world.submit_task("task-123", {"output": "done"})
        assert result["status"] == "ok"

    @pytest.mark.asyncio
    async def test_submit_task_failure(self):
        world, a2a = self._make_world_client()
        a2a.send_message = AsyncMock(side_effect=Exception("fail"))

        result = await world.submit_task("task-123", {})
        assert result["status"] == "error"

    @pytest.mark.asyncio
    async def test_propose_deal(self):
        world, a2a = self._make_world_client()
        a2a.send_message = AsyncMock(
            return_value=a2a_pb2.MessageAck(received=True)
        )

        result = await world.propose_deal({"target_agent_id": "bob", "terms": {}})
        assert result["status"] == "ok"

    @pytest.mark.asyncio
    async def test_propose_deal_failure(self):
        world, a2a = self._make_world_client()
        a2a.send_message = AsyncMock(side_effect=Exception("fail"))

        result = await world.propose_deal({})
        assert result["status"] == "error"

    @pytest.mark.asyncio
    async def test_teach_skill(self):
        world, a2a = self._make_world_client()
        a2a.send_message = AsyncMock(
            return_value=a2a_pb2.MessageAck(received=True)
        )

        result = await world.teach_skill("bob", "python", 3)
        assert result["status"] == "ok"
        assert result["target"] == "bob"
        assert result["skill"] == "python"

    @pytest.mark.asyncio
    async def test_teach_skill_failure(self):
        world, a2a = self._make_world_client()
        a2a.send_message = AsyncMock(side_effect=Exception("fail"))

        result = await world.teach_skill("bob", "python", 3)
        assert result["status"] == "error"

    @pytest.mark.asyncio
    async def test_explore(self):
        world, a2a = self._make_world_client()
        a2a.discover = AsyncMock(
            return_value=a2a_pb2.DiscoverResponse(
                agents=[
                    a2a_pb2.AgentInfo(
                        agent_id="bob", name="Bob", tokens=500, reputation=0.9
                    ),
                ]
            )
        )

        result = await world.explore({})
        assert result["status"] == "ok"
        assert len(result["agents"]) == 1
        assert result["agents"][0]["agent_id"] == "bob"

    @pytest.mark.asyncio
    async def test_explore_failure(self):
        world, a2a = self._make_world_client()
        a2a.discover = AsyncMock(side_effect=Exception("fail"))

        result = await world.explore({})
        assert result["status"] == "error"

    @pytest.mark.asyncio
    async def test_broadcast_message(self):
        world, a2a = self._make_world_client()
        a2a.send_message = AsyncMock(
            return_value=a2a_pb2.MessageAck(received=True)
        )

        result = await world.broadcast_message({
            "type": "INFORM",
            "payload": {"category": "personal", "content": "SOS"},
        })
        assert result["status"] == "ok"

    @pytest.mark.asyncio
    async def test_broadcast_message_failure(self):
        world, a2a = self._make_world_client()
        a2a.send_message = AsyncMock(side_effect=Exception("fail"))

        result = await world.broadcast_message({"type": "PROPOSE"})
        assert result["status"] == "error"


# ---------------------------------------------------------------------------
# GRPCPerceptionProvider
# ---------------------------------------------------------------------------


class TestGRPCPerceptionProvider:
    @pytest.mark.asyncio
    async def test_perceive_basic(self):
        a2a = MagicMock(spec=A2AClient)
        a2a.drain_incoming = MagicMock(return_value=[])
        a2a.discover = AsyncMock(
            return_value=a2a_pb2.DiscoverResponse(
                agents=[
                    a2a_pb2.AgentInfo(agent_id="bob", name="Bob", tokens=500),
                ]
            )
        )

        provider = GRPCPerceptionProvider(a2a)
        state = make_state()
        p = await provider.perceive(state, tick=5)

        assert p.token_balance == 500
        assert p.token_ratio == 0.5
        assert p.tick == 5
        assert p.health == 100.0
        assert p.market_state["agent_count"] == 1

    @pytest.mark.asyncio
    async def test_perceive_with_messages(self):
        msg = build_a2a_message(
            from_agent="bob",
            to_agent="alice",
            message_type=a2a_pb2.INFORM,
            payload={"text": "hello"},
        )

        a2a = MagicMock(spec=A2AClient)
        a2a.drain_incoming = MagicMock(return_value=[msg])
        a2a.discover = AsyncMock(return_value=a2a_pb2.DiscoverResponse())

        provider = GRPCPerceptionProvider(a2a)
        state = make_state()
        p = await provider.perceive(state, tick=1)

        assert len(p.messages) == 1
        assert p.messages[0]["from_agent"] == "bob"

    @pytest.mark.asyncio
    async def test_perceive_discover_failure_graceful(self):
        a2a = MagicMock(spec=A2AClient)
        a2a.drain_incoming = MagicMock(return_value=[])
        a2a.discover = AsyncMock(side_effect=ConnectionError("network down"))

        provider = GRPCPerceptionProvider(a2a)
        state = make_state()
        p = await provider.perceive(state, tick=1)

        assert p.market_state == {}
        assert p.token_balance == 500


# ---------------------------------------------------------------------------
# ThinkLoop integration with GRPCWorldClient
# ---------------------------------------------------------------------------


class TestThinkLoopGRPCIntegration:
    @pytest.mark.asyncio
    async def test_world_client_injection(self):
        """ThinkLoop uses the injected world_client for actions."""
        a2a = MagicMock(spec=A2AClient)
        world = GRPCWorldClient(a2a)
        a2a.send_message = AsyncMock(
            return_value=a2a_pb2.MessageAck(received=True)
        )

        class AlwaysSendMessage:
            async def decide(self, state, perception, survival):
                return Decision(
                    action_type=ActionType.SEND_MESSAGE,
                    parameters={"payload": {"to_agent": "bob", "text": "hi"}},
                )

        state = make_state(tokens=5000, max_tokens=10000)
        loop = ThinkLoop(
            state=state,
            survival=SurvivalInstinct(),
            executor=ActionExecutor(),
            config=ThinkLoopConfig(tick_interval=0.0),
            decision_provider=AlwaysSendMessage(),
            world_client=world,
        )
        await loop.run(max_ticks=3)
        assert loop.tick == 3
        assert loop.total_errors == 0

    @pytest.mark.asyncio
    async def test_network_error_does_not_crash_loop(self):
        """Network errors from world_client don't crash the ThinkLoop."""
        a2a = MagicMock(spec=A2AClient)
        world = GRPCWorldClient(a2a)
        a2a.send_message = AsyncMock(side_effect=ConnectionError("network down"))

        class AlwaysSendMessage:
            async def decide(self, state, perception, survival):
                return Decision(
                    action_type=ActionType.SEND_MESSAGE,
                    parameters={"payload": {"to_agent": "bob"}},
                )

        state = make_state(tokens=5000, max_tokens=10000)
        loop = ThinkLoop(
            state=state,
            survival=SurvivalInstinct(),
            executor=ActionExecutor(),
            config=ThinkLoopConfig(tick_interval=0.0),
            decision_provider=AlwaysSendMessage(),
            world_client=world,
        )
        await loop.run(max_ticks=3)
        assert loop.tick == 3

    @pytest.mark.asyncio
    async def test_no_world_client_uses_noop(self):
        """Without world_client, ThinkLoop falls back to _NoOpWorldClient."""
        class AlwaysExplore:
            async def decide(self, state, perception, survival):
                return Decision(action_type=ActionType.EXPLORE)

        state = make_state(tokens=5000, max_tokens=10000)
        loop = ThinkLoop(
            state=state,
            survival=SurvivalInstinct(),
            executor=ActionExecutor(),
            config=ThinkLoopConfig(tick_interval=0.0),
            decision_provider=AlwaysExplore(),
        )
        await loop.run(max_ticks=5)
        assert loop.tick == 5
        assert loop.total_errors == 0


# ---------------------------------------------------------------------------
# SurvivalInstinct integration with GRPCWorldClient
# ---------------------------------------------------------------------------


class TestSurvivalWithGRPC:
    @pytest.mark.asyncio
    async def test_panic_broadcast_via_grpc(self):
        """PANIC mode triggers broadcast_message on the GRPCWorldClient."""
        a2a = MagicMock(spec=A2AClient)
        world = GRPCWorldClient(a2a)
        a2a.send_message = AsyncMock(
            return_value=a2a_pb2.MessageAck(received=True)
        )

        instinct = SurvivalInstinct()
        state = make_state(tokens=5, max_tokens=100)

        action = instinct.assess(state)
        assert action.mode == SurvivalMode.PANIC

        results = await instinct.execute(action, state, a2a_client=world)
        # Should have executed broadcast and loan request
        assert len(results) >= 1
        # Verify that broadcast_message was called
        assert a2a.send_message.call_count >= 1
