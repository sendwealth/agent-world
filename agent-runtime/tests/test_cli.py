"""Tests for the __main__.py CLI entry point.

Covers:
- Parser building and argument validation
- Trait and skill parsing
- Agent spawning from config
- Runtime execution (think loop runs, stats collected)
- RESTWorldClient fallback
- World Engine connection
- Config building from CLI args
- Key generation integration
- Graceful shutdown (shutdown_reason field)
- Health check server
- CLI shortcut (--world defaults to spawn)
"""

from __future__ import annotations

import asyncio
import socket
from unittest.mock import AsyncMock, patch

import pytest

from agent_runtime.__main__ import (
    HealthCheckServer,
    RESTWorldClient,
    WorldConnection,
    _get_health_port,
    _has_world_arg,
    _rewrite_world_to_world_url,
    build_config_from_args,
    build_parser,
    connect_world_engine,
    deregister_agent,
    parse_skills,
    parse_traits,
    register_agent,
    run_agent,
    spawn_agent,
)
from agent_runtime.config import AgentSpawnConfig, RuntimeConfig
from agent_runtime.core.act import ActionExecutor
from agent_runtime.core.think_loop import ThinkLoop, ThinkLoopConfig
from agent_runtime.models.agent_state import AgentState
from agent_runtime.survival.instinct import SurvivalInstinct

# ---------------------------------------------------------------------------
# Parser tests
# ---------------------------------------------------------------------------


class TestBuildParser:
    def test_no_command_returns_none(self) -> None:
        parser = build_parser()
        args = parser.parse_args([])
        assert args.command is None

    def test_spawn_with_name(self) -> None:
        parser = build_parser()
        args = parser.parse_args(["spawn", "--name", "Alpha"])
        assert args.command == "spawn"
        assert args.name == "Alpha"

    def test_all_options(self) -> None:
        parser = build_parser()
        args = parser.parse_args([
            "spawn",
            "--name", "Alpha",
            "--skills", "coding,trading",
            "--traits", "curiosity=0.8",
            "--tokens", "2000",
            "--max-tokens", "5000",
            "--max-ticks", "100",
            "--tick-interval", "0.5",
            "--world-url", "http://engine:3000",
            "--llm-provider", "ollama",
            "--llm-model", "llama3",
        ])
        assert args.name == "Alpha"
        assert args.skills == "coding,trading"
        assert args.traits == ["curiosity=0.8"]
        assert args.tokens == 2000
        assert args.max_tokens == 5000
        assert args.max_ticks == 100
        assert args.tick_interval == 0.5
        assert args.world_url == "http://engine:3000"
        assert args.llm_provider == "ollama"
        assert args.llm_model == "llama3"

    def test_health_port_option(self) -> None:
        parser = build_parser()
        args = parser.parse_args(["spawn", "--health-port", "8888"])
        assert args.health_port == 8888

    def test_top_level_world_arg(self) -> None:
        parser = build_parser()
        args = parser.parse_args(["--world", "http://localhost:8080"])
        assert args.world == "http://localhost:8080"
        assert args.command is None


# ---------------------------------------------------------------------------
# Trait / skill parsing
# ---------------------------------------------------------------------------


class TestParseTraits:
    def test_valid_traits(self) -> None:
        result = parse_traits(["curiosity=0.8", "caution=0.3"])
        assert result == {"curiosity": 0.8, "caution": 0.3}

    def test_none_input(self) -> None:
        assert parse_traits(None) == {}

    def test_empty_list(self) -> None:
        assert parse_traits([]) == {}

    def test_malformed_trait_ignored(self) -> None:
        result = parse_traits(["curiosity=0.8", "bad_trait"])
        assert result == {"curiosity": 0.8}

    def test_non_numeric_trait_exits(self) -> None:
        with pytest.raises(SystemExit):
            parse_traits(["curiosity=not_a_number"])


class TestParseSkills:
    def test_comma_separated(self) -> None:
        result = parse_skills("coding,trading,research")
        assert result == {"coding": 1, "trading": 1, "research": 1}

    def test_none_input(self) -> None:
        assert parse_skills(None) == {}

    def test_empty_string(self) -> None:
        assert parse_skills("") == {}

    def test_whitespace_handling(self) -> None:
        result = parse_skills(" coding , trading ")
        assert result == {"coding": 1, "trading": 1}


# ---------------------------------------------------------------------------
# Agent spawning
# ---------------------------------------------------------------------------


class TestSpawnAgent:
    def test_spawn_basic(self) -> None:
        cfg = AgentSpawnConfig(name="TestBot")
        state = spawn_agent(cfg)
        assert state.name == "TestBot"
        assert state.tokens == 500
        assert state.max_tokens == 1000

    def test_spawn_with_traits(self) -> None:
        cfg = AgentSpawnConfig(
            name="CuriousBot",
            traits={"curiosity": 0.9, "aggression": 0.1},
        )
        state = spawn_agent(cfg)
        assert state.personality == {"curiosity": 0.9, "aggression": 0.1}

    def test_spawn_with_skills(self) -> None:
        cfg = AgentSpawnConfig(
            name="SkilledBot",
            skills={"coding": 5, "trading": 2},
        )
        state = spawn_agent(cfg)
        assert "coding" in state.skills
        assert state.skills["coding"].level == 5

    def test_spawn_custom_resources(self) -> None:
        cfg = AgentSpawnConfig(
            name="RichBot",
            tokens=5000,
            max_tokens=10000,
            money=200.0,
            health=80.0,
        )
        state = spawn_agent(cfg)
        assert state.tokens == 5000
        assert state.max_tokens == 10000
        assert state.money == 200.0
        assert state.health == 80.0


# ---------------------------------------------------------------------------
# Runtime execution
# ---------------------------------------------------------------------------


class TestRunAgent:
    @pytest.mark.asyncio
    async def test_run_agent_completes(self) -> None:
        config = RuntimeConfig(
            agent=AgentSpawnConfig(name="TestRunner", tokens=5000, max_tokens=10000),
            think_loop=ThinkLoopConfig(tick_interval=0.0, max_ticks=5),
        )
        stats = await run_agent(config)
        assert stats.agent_name == "TestRunner"
        assert stats.ticks == 5
        assert stats.errors == 0
        assert stats.duration_s > 0
        assert stats.agent_id

    @pytest.mark.asyncio
    async def test_run_agent_shutdown_reason_completed(self) -> None:
        config = RuntimeConfig(
            agent=AgentSpawnConfig(name="ShutdownTest"),
            think_loop=ThinkLoopConfig(tick_interval=0.0, max_ticks=3),
        )
        stats = await run_agent(config)
        assert stats.shutdown_reason == "completed"

    @pytest.mark.asyncio
    async def test_run_agent_stats_include_shutdown_reason(self) -> None:
        config = RuntimeConfig(
            agent=AgentSpawnConfig(name="StatsShutdown"),
            think_loop=ThinkLoopConfig(tick_interval=0.0, max_ticks=2),
        )
        stats = await run_agent(config)
        d = stats.to_dict()
        assert "shutdown_reason" in d
        assert d["shutdown_reason"] == "completed"


# ---------------------------------------------------------------------------
# Key generation integration
# ---------------------------------------------------------------------------


class TestKeyGeneration:
    @pytest.mark.asyncio
    async def test_register_with_public_key(self) -> None:
        """register_agent accepts public_key_b64 without error."""
        state = AgentState(name="KeyTest", tokens=500, max_tokens=1000)
        # World Engine is unreachable, so it returns False but doesn't crash
        result = await register_agent(
            state,
            "http://localhost:1",
            public_key_b64="dGVzdHB1YmxpY2tleQ",
            timeout=0.1,
        )
        assert result is False or result is None  # Unreachable but no crash


# ---------------------------------------------------------------------------
# Deregistration
# ---------------------------------------------------------------------------


class TestDeregisterAgent:
    @pytest.mark.asyncio
    async def test_deregister_unreachable(self) -> None:
        """deregister_agent handles unreachable server gracefully."""
        result = await deregister_agent(
            "test-agent-id",
            "http://localhost:1",
            timeout=0.1,
        )
        assert result is False  # Unreachable but no crash


# ---------------------------------------------------------------------------
# RESTWorldClient fallback
# ---------------------------------------------------------------------------


class TestRESTWorldClient:
    @pytest.mark.asyncio
    async def test_send_message_returns_standalone(self) -> None:
        client = RESTWorldClient("http://localhost:3000", agent_id="test-agent")
        with patch.object(client, "_request", new_callable=AsyncMock) as mock_req:
            mock_req.return_value = {"status": "standalone"}
            result = await client.send_message({"text": "hello"})
            assert result["status"] == "standalone"

    @pytest.mark.asyncio
    async def test_claim_task_returns_standalone(self) -> None:
        client = RESTWorldClient("http://localhost:3000", agent_id="test-agent")
        with patch.object(client, "_request", new_callable=AsyncMock) as mock_req:
            mock_req.return_value = {"status": "standalone"}
            result = await client.claim_task("task-123")
            assert result["status"] == "standalone"

    @pytest.mark.asyncio
    async def test_explore_returns_standalone(self) -> None:
        client = RESTWorldClient("http://localhost:3000", agent_id="test-agent")
        with patch.object(client, "_request", new_callable=AsyncMock) as mock_req:
            mock_req.return_value = {"status": "standalone"}
            result = await client.explore({})
            assert result["status"] == "standalone"


# ---------------------------------------------------------------------------
# World Engine connection
# ---------------------------------------------------------------------------


class TestConnectWorldEngine:
    @pytest.mark.asyncio
    async def test_rest_fallback_when_grpc_unavailable(self) -> None:
        conn = await connect_world_engine(
            grpc_address="nonexistent:50051",
            rest_url="http://localhost:3000",
            agent_id="test-agent",
        )
        assert isinstance(conn, WorldConnection)
        assert isinstance(conn.world_client, RESTWorldClient)
        assert conn.perception_provider is None
        assert conn.a2a_client is None


# ---------------------------------------------------------------------------
# Build config from args
# ---------------------------------------------------------------------------


class TestBuildConfigFromArgs:
    def test_name_override(self) -> None:
        parser = build_parser()
        args = parser.parse_args(["spawn", "--name", "Override"])
        config = build_config_from_args(args)
        assert config.agent.name == "Override"

    def test_default_name_when_none_provided(self) -> None:
        parser = build_parser()
        args = parser.parse_args(["spawn"])
        config = build_config_from_args(args)
        assert config.agent.name == "Agent"

    def test_skills_from_cli(self) -> None:
        parser = build_parser()
        args = parser.parse_args(["spawn", "--skills", "coding,research"])
        config = build_config_from_args(args)
        assert "coding" in config.agent.skills
        assert "research" in config.agent.skills

    def test_health_port_from_cli(self) -> None:
        parser = build_parser()
        args = parser.parse_args(["spawn", "--health-port", "7777"])
        config = build_config_from_args(args)
        assert config.health_port == 7777

    def test_world_url_from_spawn_subcommand(self) -> None:
        parser = build_parser()
        args = parser.parse_args(["spawn", "--world-url", "http://engine:4000"])
        config = build_config_from_args(args)
        assert config.world.engine_url == "http://engine:4000"


# ---------------------------------------------------------------------------
# CLI shortcut: --world defaults to spawn
# ---------------------------------------------------------------------------


class TestCLIShortcut:
    def test_has_world_arg_detects_world(self) -> None:
        assert _has_world_arg(["--world", "http://localhost:8080"]) is True

    def test_has_world_arg_detects_world_equals(self) -> None:
        assert _has_world_arg(["--world=http://localhost:8080"]) is True

    def test_has_world_arg_detects_world_url(self) -> None:
        assert _has_world_arg(["--world-url", "http://localhost:8080"]) is True

    def test_has_world_arg_false_when_absent(self) -> None:
        assert _has_world_arg(["--name", "Alice"]) is False

    def test_rewrite_world_to_world_url(self) -> None:
        result = _rewrite_world_to_world_url(
            ["--world", "http://localhost:8080", "--name", "Test"]
        )
        assert result == ["--world-url", "http://localhost:8080", "--name", "Test"]

    def test_rewrite_world_equals_form(self) -> None:
        result = _rewrite_world_to_world_url(
            ["--world=http://host:9090", "--name", "Test"]
        )
        assert result == ["--world-url=http://host:9090", "--name", "Test"]

    def test_rewrite_preserves_other_args(self) -> None:
        result = _rewrite_world_to_world_url(
            ["--name", "Alice", "--max-ticks", "10"]
        )
        assert result == ["--name", "Alice", "--max-ticks", "10"]


# ---------------------------------------------------------------------------
# Health check server
# ---------------------------------------------------------------------------


def _find_free_port() -> int:
    """Find a free TCP port for testing."""
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


class TestHealthCheckServer:
    @pytest.mark.asyncio
    async def test_health_endpoint_returns_json(self) -> None:
        port = _find_free_port()
        state = AgentState(name="HealthBot", tokens=500, max_tokens=1000)
        think_loop = ThinkLoop(
            state=state,
            survival=SurvivalInstinct(),
            executor=ActionExecutor(),
            config=ThinkLoopConfig(tick_interval=0.0, max_ticks=0),
        )
        server = HealthCheckServer(
            agent_name="HealthBot",
            think_loop=think_loop,
            port=port,
        )

        task = asyncio.create_task(server.start())
        try:
            # Wait for server to be ready
            await asyncio.sleep(0.3)

            # Query using raw TCP to avoid httpx proxy interference
            reader, writer = await asyncio.open_connection("127.0.0.1", port)
            writer.write(b"GET /health HTTP/1.1\r\nHost: localhost\r\n\r\n")
            await writer.drain()

            response_data = b""
            while True:
                chunk = await asyncio.wait_for(reader.read(4096), timeout=2.0)
                if not chunk:
                    break
                response_data += chunk
                if b"\r\n\r\n" in response_data:
                    break

            writer.close()
            try:
                await writer.wait_closed()
            except Exception:
                pass

            response_str = response_data.decode("utf-8", errors="replace")
            assert "200 OK" in response_str
            assert "HealthBot" in response_str
            assert "uptime_s" in response_str
        finally:
            await server.stop()
            task.cancel()
            try:
                await task
            except (asyncio.CancelledError, Exception):
                pass

    @pytest.mark.asyncio
    async def test_health_404_for_unknown_path(self) -> None:
        port = _find_free_port()
        state = AgentState(name="PathBot", tokens=500, max_tokens=1000)
        think_loop = ThinkLoop(
            state=state,
            survival=SurvivalInstinct(),
            executor=ActionExecutor(),
            config=ThinkLoopConfig(tick_interval=0.0, max_ticks=0),
        )
        server = HealthCheckServer(
            agent_name="PathBot",
            think_loop=think_loop,
            port=port,
        )

        task = asyncio.create_task(server.start())
        try:
            await asyncio.sleep(0.3)

            reader, writer = await asyncio.open_connection("127.0.0.1", port)
            writer.write(b"GET /unknown HTTP/1.1\r\nHost: localhost\r\n\r\n")
            await writer.drain()

            response_data = b""
            while True:
                chunk = await asyncio.wait_for(reader.read(4096), timeout=2.0)
                if not chunk:
                    break
                response_data += chunk
                if b"\r\n\r\n" in response_data:
                    break

            writer.close()
            try:
                await writer.wait_closed()
            except Exception:
                pass

            response_str = response_data.decode("utf-8", errors="replace")
            assert "404" in response_str
        finally:
            await server.stop()
            task.cancel()
            try:
                await task
            except (asyncio.CancelledError, Exception):
                pass


# ---------------------------------------------------------------------------
# Health port config
# ---------------------------------------------------------------------------


class TestGetHealthPort:
    def test_default_port(self) -> None:
        config = RuntimeConfig()
        assert _get_health_port(config) == 9090

    def test_config_port(self) -> None:
        config = RuntimeConfig(health_port=8888)
        assert _get_health_port(config) == 8888

    def test_env_override(self, monkeypatch: pytest.MonkeyPatch) -> None:
        monkeypatch.setenv("HEALTH_PORT", "7777")
        config = RuntimeConfig(health_port=8888)
        assert _get_health_port(config) == 7777

    def test_invalid_env_falls_back_to_config(self, monkeypatch: pytest.MonkeyPatch) -> None:
        monkeypatch.setenv("HEALTH_PORT", "not_a_number")
        config = RuntimeConfig(health_port=8888)
        assert _get_health_port(config) == 8888
