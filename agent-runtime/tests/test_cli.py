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
    RESTPerceptionProvider,
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
        assert state.tokens == 100_000
        assert state.max_tokens == 200_000

    def test_spawn_with_traits(self) -> None:
        cfg = AgentSpawnConfig(
            name="CuriousBot",
            traits={"curiosity": 0.9, "aggression": 0.1},
        )
        state = spawn_agent(cfg)
        # Traits are merged into the personality dict alongside
        # structured personality/values/preferences data.
        assert state.personality["curiosity"] == 0.9
        assert state.personality["aggression"] == 0.1
        assert "big_five" in state.personality
        assert "values" in state.personality

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
        # World Engine is unreachable, so it returns None but doesn't crash
        result = await register_agent(
            state,
            "http://localhost:1",
            public_key_b64="dGVzdHB1YmxpY2tleQ",
            timeout=0.1,
            max_retries=1,
        )
        assert result is None  # Unreachable but no crash

    @pytest.mark.asyncio
    async def test_register_retries_on_connect_error(self) -> None:
        """register_agent retries on connection errors before giving up."""
        state = AgentState(name="RetryTest", tokens=500, max_tokens=1000)
        result = await register_agent(
            state,
            "http://localhost:1",
            timeout=0.1,
            max_retries=2,
            retry_delay=0.05,
        )
        assert result is None  # Exhausted retries, no crash


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
    async def test_send_message_returns_success(self) -> None:
        client = RESTWorldClient("http://localhost:3000", agent_id="test-agent")
        with patch.object(client, "_request", new_callable=AsyncMock) as mock_req:
            mock_req.return_value = {"status": "ok"}
            result = await client.send_message({"text": "hello"})
            assert result["status"] == "ok"

    @pytest.mark.asyncio
    async def test_claim_task_returns_success(self) -> None:
        client = RESTWorldClient("http://localhost:3000", agent_id="test-agent")
        with patch.object(client, "_request", new_callable=AsyncMock) as mock_req:
            mock_req.return_value = {"status": "ok", "task_id": "task-123"}
            result = await client.claim_task("task-123")
            assert result["status"] == "ok"

    @pytest.mark.asyncio
    async def test_explore_returns_success(self) -> None:
        client = RESTWorldClient("http://localhost:3000", agent_id="test-agent")
        with patch.object(client, "_request", new_callable=AsyncMock) as mock_req:
            mock_req.return_value = {"status": "ok", "agents": []}
            result = await client.explore({})
            assert result["status"] == "ok"


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
        assert isinstance(conn.perception_provider, RESTPerceptionProvider)
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

    def test_world_url_from_env_var(self) -> None:
        """WORLD_ENGINE_URL env var is used when --world-url is not provided."""
        import os

        parser = build_parser()
        args = parser.parse_args(["spawn"])
        with patch.dict(os.environ, {"WORLD_ENGINE_URL": "http://world-engine:8080"}):
            config = build_config_from_args(args)
        assert config.world.engine_url == "http://world-engine:8080"

    def test_world_url_cli_overrides_env_var(self) -> None:
        """CLI --world-url takes precedence over WORLD_ENGINE_URL env var."""
        import os

        parser = build_parser()
        args = parser.parse_args(["spawn", "--world-url", "http://cli-url:9090"])
        with patch.dict(os.environ, {"WORLD_ENGINE_URL": "http://env-url:8080"}):
            config = build_config_from_args(args)
        assert config.world.engine_url == "http://cli-url:9090"


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


# ---------------------------------------------------------------------------
# Pool sub-command
# ---------------------------------------------------------------------------


class TestPoolParser:
    def test_pool_defaults(self) -> None:
        parser = build_parser()
        args = parser.parse_args(["pool"])
        assert args.command == "pool"
        assert args.count == 1
        assert args.max_restart == 3
        assert args.health_interval == 10.0
        assert args.api_port == 9090
        assert args.config_dir is None

    def test_pool_with_count(self) -> None:
        parser = build_parser()
        args = parser.parse_args(["pool", "--count", "10"])
        assert args.count == 10

    def test_pool_inherits_spawn_args(self) -> None:
        parser = build_parser()
        args = parser.parse_args([
            "pool",
            "--count", "5",
            "--world-url", "http://localhost:8080",
            "--llm-provider", "zhipu",
            "--llm-model", "glm-5",
            "--no-llm",
        ])
        assert args.count == 5
        assert args.world_url == "http://localhost:8080"
        assert args.llm_provider == "zhipu"
        assert args.llm_model == "glm-5"
        assert args.no_llm is True

    def test_pool_all_options(self) -> None:
        parser = build_parser()
        args = parser.parse_args([
            "pool",
            "--count", "3",
            "--config-dir", "/tmp/agents",
            "--max-restart", "5",
            "--health-interval", "20",
            "--api-port", "8888",
            "--world-url", "http://engine:3000",
            "--skills", "coding,trading",
            "--max-ticks", "100",
        ])
        assert args.count == 3
        assert str(args.config_dir) == "/tmp/agents"
        assert args.max_restart == 5
        assert args.health_interval == 20.0
        assert args.api_port == 8888
        assert args.world_url == "http://engine:3000"
        assert args.skills == "coding,trading"
        assert args.max_ticks == 100


class TestBuildPoolSpawnArgs:
    def test_extracts_world_url(self) -> None:
        from agent_runtime.__main__ import _build_pool_spawn_args
        parser = build_parser()
        args = parser.parse_args(["pool", "--world-url", "http://localhost:8080"])
        result = _build_pool_spawn_args(args)
        assert "--world-url" in result
        assert "http://localhost:8080" in result

    def test_extracts_no_llm(self) -> None:
        from agent_runtime.__main__ import _build_pool_spawn_args
        parser = build_parser()
        args = parser.parse_args(["pool", "--no-llm"])
        result = _build_pool_spawn_args(args)
        assert "--no-llm" in result

    def test_empty_when_no_spawn_args(self) -> None:
        from agent_runtime.__main__ import _build_pool_spawn_args
        parser = build_parser()
        args = parser.parse_args(["pool"])
        result = _build_pool_spawn_args(args)
        assert result == []


class TestAgentPool:
    def test_build_from_count(self) -> None:
        from agent_runtime.__main__ import AgentPool
        pool = AgentPool(count=5)
        agents = pool._build_from_count()
        assert len(agents) == 5
        assert agents[0].name == "Agent-1"
        assert agents[4].name == "Agent-5"

    def test_build_from_count_default(self) -> None:
        from agent_runtime.__main__ import AgentPool
        pool = AgentPool(count=1)
        agents = pool._build_from_count()
        assert len(agents) == 1
        assert agents[0].name == "Agent-1"

    def test_build_from_config_dir_missing(self, tmp_path) -> None:
        from agent_runtime.__main__ import AgentPool
        pool = AgentPool(config_dir=tmp_path / "nonexistent")
        agents = pool._build_from_config_dir()
        assert agents == []

    def test_build_from_config_dir_with_toml(self, tmp_path) -> None:
        from agent_runtime.__main__ import AgentPool
        (tmp_path / "alice.toml").write_text("[agent]\nname = 'Alice'\n")
        (tmp_path / "bob.toml").write_text("[agent]\nname = 'Bob'\n")
        pool = AgentPool(config_dir=tmp_path)
        agents = pool._build_from_config_dir()
        assert len(agents) == 2
        assert agents[0].name == "alice"
        assert agents[1].name == "bob"

    @pytest.mark.asyncio
    async def test_pool_run_3_agents(self) -> None:
        """Integration test: pool launches 3 agents, each completes, pool exits."""
        from agent_runtime.__main__ import AgentPool
        pool = AgentPool(
            count=3,
            max_restart=0,
            health_interval=1,
            api_port=0,
            spawn_args=["--no-llm", "--max-ticks", "2"],
        )
        result = await pool.run()
        assert len(result["agents"]) == 3
        assert all(a["status"] in ("stopped", "crashed") for a in result["agents"])
        assert result["duration_s"] > 0

    @pytest.mark.asyncio
    async def test_pool_auto_naming(self) -> None:
        from agent_runtime.__main__ import AgentPool
        pool = AgentPool(
            count=3,
            max_restart=0,
            health_interval=1,
            api_port=0,
            spawn_args=["--no-llm", "--max-ticks", "1"],
        )
        result = await pool.run()
        names = [a["name"] for a in result["agents"]]
        assert names == ["Agent-1", "Agent-2", "Agent-3"]

    @pytest.mark.asyncio
    async def test_pool_request_shutdown(self) -> None:
        """Test that request_shutdown() causes pool to exit."""
        from agent_runtime.__main__ import AgentPool
        pool = AgentPool(
            count=1,
            max_restart=0,
            health_interval=0.2,
            api_port=0,
            spawn_args=["--no-llm", "--max-ticks", "9999"],
        )

        async def shutdown_after(delay: float) -> None:
            await asyncio.sleep(delay)
            pool.request_shutdown()

        asyncio.get_running_loop().create_task(shutdown_after(2.0))
        result = await pool.run()
        assert len(result["agents"]) == 1
        # Agent was either stopped (terminated by pool shutdown) or running
        assert result["agents"][0]["status"] in ("stopped", "running", "crashed")


# ---------------------------------------------------------------------------
# --preset CLI argument
# ---------------------------------------------------------------------------


class TestPresetArg:
    def test_parsing_preset_arg(self) -> None:
        parser = build_parser()
        args = parser.parse_args(["spawn", "--preset", "zhipu"])
        assert args.preset == "zhipu"

    def test_parsing_preset_with_model(self) -> None:
        parser = build_parser()
        args = parser.parse_args(["spawn", "--preset", "zhipu", "--llm-model", "glm-5"])
        assert args.preset == "zhipu"
        assert args.llm_model == "glm-5"

    def test_parsing_preset_with_pool(self) -> None:
        parser = build_parser()
        args = parser.parse_args(["pool", "--count", "3", "--preset", "deepseek"])
        assert args.preset == "deepseek"
        assert args.count == 3

    def test_invalid_preset_exits(self, tmp_path) -> None:
        """Invalid preset name causes SystemExit with available list."""
        import argparse

        from agent_runtime.__main__ import _apply_preset_defaults

        # Write a minimal presets file for testing
        presets_path = tmp_path / "model-presets.yaml"
        presets_path.write_text(
            "providers:\n"
            "  zhipu:\n"
            "    id: zhipu\n"
            "    protocol: openai\n"
            "    base_url: 'https://example.com'\n"
            "models: []\n"
        )

        from agent_runtime.presets import reload_presets
        reload_presets(presets_path)

        args = argparse.Namespace(
            preset="nonexistent",
            llm_provider=None,
            llm_base_url=None,
            llm_model=None,
        )
        with pytest.raises(SystemExit):
            _apply_preset_defaults(args)

    def test_valid_preset_fills_args(self, tmp_path) -> None:
        """Valid preset fills in missing LLM args."""
        import argparse

        from agent_runtime.__main__ import _apply_preset_defaults

        presets_path = tmp_path / "model-presets.yaml"
        presets_path.write_text(
            "providers:\n"
            "  zhipu:\n"
            "    id: zhipu\n"
            "    protocol: openai\n"
            "    base_url: 'https://open.bigmodel.cn/api/paas/v4'\n"
            "    api_key_required: true\n"
            "    api_key_env: ZHIPU_API_KEY\n"
            "models:\n"
            "  - id: glm-5\n"
            "    provider: zhipu\n"
            "    label: GLM-5\n"
        )

        from agent_runtime.presets import reload_presets
        reload_presets(presets_path)

        args = argparse.Namespace(
            preset="zhipu",
            llm_provider=None,
            llm_base_url=None,
            llm_model=None,
        )
        _apply_preset_defaults(args)

        assert args.llm_provider == "zhipu"
        assert args.llm_base_url == "https://open.bigmodel.cn/api/paas/v4"
        assert args.llm_model == "glm-5"

    def test_preset_does_not_override_explicit_cli(self, tmp_path) -> None:
        """Explicit CLI flags take precedence over preset values."""
        import argparse

        from agent_runtime.__main__ import _apply_preset_defaults

        presets_path = tmp_path / "model-presets.yaml"
        presets_path.write_text(
            "providers:\n"
            "  zhipu:\n"
            "    id: zhipu\n"
            "    protocol: openai\n"
            "    base_url: 'https://open.bigmodel.cn/api/paas/v4'\n"
            "models:\n"
            "  - id: glm-5\n"
            "    provider: zhipu\n"
        )

        from agent_runtime.presets import reload_presets
        reload_presets(presets_path)

        args = argparse.Namespace(
            preset="zhipu",
            llm_provider="ollama",
            llm_base_url="http://custom:1234",
            llm_model="custom-model",
        )
        _apply_preset_defaults(args)

        # Explicit values should NOT be overwritten
        assert args.llm_provider == "ollama"
        assert args.llm_base_url == "http://custom:1234"
        assert args.llm_model == "custom-model"

    def test_pool_spawn_args_includes_preset(self) -> None:
        """--preset is forwarded in pool spawn args."""
        from agent_runtime.__main__ import _build_pool_spawn_args

        parser = build_parser()
        args = parser.parse_args(["pool", "--preset", "zhipu"])
        result = _build_pool_spawn_args(args)
        assert "--preset" in result
        assert "zhipu" in result

    def test_build_config_with_preset(self, tmp_path, monkeypatch) -> None:
        """End-to-end: build_config_from_args with --preset resolves correctly."""
        presets_path = tmp_path / "model-presets.yaml"
        presets_path.write_text(
            "providers:\n"
            "  ollama-local:\n"
            "    id: ollama-local\n"
            "    protocol: ollama\n"
            "    base_url: 'http://localhost:11434'\n"
            "    api_key_required: false\n"
            "models:\n"
            "  - id: qwen3:8b\n"
            "    provider: ollama-local\n"
        )

        from agent_runtime.presets import reload_presets
        reload_presets(presets_path)

        parser = build_parser()
        args = parser.parse_args(["spawn", "--preset", "ollama-local"])
        config = build_config_from_args(args)

        assert config.llm is not None
        assert config.llm.provider.value == "ollama"
        assert config.llm.model == "qwen3:8b"
        assert config.llm.base_url == "http://localhost:11434"


# ---------------------------------------------------------------------------
# _extract_grpc_address
# ---------------------------------------------------------------------------


class TestExtractGrpcAddress:
    """Tests for converting REST URLs to gRPC addresses."""

    def test_http_with_port(self) -> None:
        from agent_runtime.cli import _extract_grpc_address

        assert _extract_grpc_address("http://localhost:8080") == "localhost:50051"

    def test_https_with_port(self) -> None:
        from agent_runtime.cli import _extract_grpc_address

        assert (
            _extract_grpc_address("https://engine.example.com:443")
            == "engine.example.com:50051"
        )

    def test_http_no_port(self) -> None:
        from agent_runtime.cli import _extract_grpc_address

        assert _extract_grpc_address("http://localhost") == "localhost:50051"

    def test_https_no_port(self) -> None:
        from agent_runtime.cli import _extract_grpc_address

        assert (
            _extract_grpc_address("https://engine.example.com")
            == "engine.example.com:50051"
        )

    def test_grpc_port_env_override(self, monkeypatch: pytest.MonkeyPatch) -> None:
        from agent_runtime.cli import _extract_grpc_address

        monkeypatch.setenv("GRPC_PORT", "9090")
        assert _extract_grpc_address("http://localhost:8080") == "localhost:9090"

    def test_url_with_path(self) -> None:
        from agent_runtime.cli import _extract_grpc_address

        assert (
            _extract_grpc_address("http://engine.example.com:8080/api/v1")
            == "engine.example.com:50051"
        )

    def test_url_with_query_params(self) -> None:
        from agent_runtime.cli import _extract_grpc_address

        assert (
            _extract_grpc_address("http://engine.example.com:8080?key=value")
            == "engine.example.com:50051"
        )

    def test_ipv6_address(self) -> None:
        from agent_runtime.cli import _extract_grpc_address

        assert _extract_grpc_address("http://[::1]:8080") == "[::1]:50051"

    def test_ipv6_address_no_port(self) -> None:
        from agent_runtime.cli import _extract_grpc_address

        assert _extract_grpc_address("http://[::1]") == "[::1]:50051"

    def test_ipv6_full_address(self) -> None:
        from agent_runtime.cli import _extract_grpc_address

        assert (
            _extract_grpc_address("http://[2001:db8::1]:8080")
            == "[2001:db8::1]:50051"
        )

    def test_plain_hostname_no_scheme(self) -> None:
        """Bare hostname without scheme still works as fallback."""
        from agent_runtime.cli import _extract_grpc_address

        assert _extract_grpc_address("localhost") == "localhost:50051"
