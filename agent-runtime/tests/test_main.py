"""Tests for the __main__.py CLI entry point.

Covers:
- Parser building and argument validation
- Trait and skill parsing
- Agent spawning from config
- Runtime execution (think loop runs, stats collected)
- RESTWorldClient endpoint mapping
- World Engine connection
- Config building from CLI args
"""

from __future__ import annotations

from unittest.mock import AsyncMock, MagicMock, patch

import pytest

from agent_runtime.__main__ import (
    RESTPerceptionProvider,
    RESTWorldClient,
    WorldConnection,
    build_config_from_args,
    build_parser,
    connect_world_engine,
    parse_skills,
    parse_traits,
    run_agent,
    spawn_agent,
)
from agent_runtime.config import AgentSpawnConfig, RuntimeConfig
from agent_runtime.core.think_loop import ThinkLoopConfig

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


# ---------------------------------------------------------------------------
# RESTWorldClient endpoint mapping
# ---------------------------------------------------------------------------


def _make_client() -> RESTWorldClient:
    """Create a RESTWorldClient with a fake URL and agent ID."""
    return RESTWorldClient("http://localhost:3000", agent_id="agent-42")


class TestRESTWorldClientEndpoints:
    """Verify each method hits the correct World Engine REST endpoint."""

    @pytest.mark.asyncio
    async def test_send_message_hits_api_v1_messages(self) -> None:
        client = _make_client()
        with patch.object(client, "_request", new_callable=AsyncMock) as mock_req:
            mock_req.return_value = {"status": "ok"}
            await client.send_message({
                "from_agent": "a", "to_agent": "b",
                "message_type": "text", "payload": "hi",
            })
            mock_req.assert_awaited_once_with(
                "POST", "/api/v1/messages",
                json={"from_agent": "a", "to_agent": "b", "message_type": "text", "payload": "hi"},
            )

    @pytest.mark.asyncio
    async def test_claim_task_uses_submit_action(self) -> None:
        client = _make_client()
        with patch.object(client, "submit_action", new_callable=AsyncMock) as mock:
            mock.return_value = {"status": "ok"}
            await client.claim_task("task-99")
            mock.assert_awaited_once_with("claim_task", {"task_id": "task-99"})

    @pytest.mark.asyncio
    async def test_submit_task_uses_submit_action(self) -> None:
        client = _make_client()
        with patch.object(client, "submit_action", new_callable=AsyncMock) as mock:
            mock.return_value = {"status": "ok"}
            await client.submit_task("task-99", {"answer": 42})
            mock.assert_awaited_once_with(
                "submit_task",
                {"task_id": "task-99", "result": {"answer": 42}},
            )

    @pytest.mark.asyncio
    async def test_propose_deal_uses_trade_action(self) -> None:
        """propose_deal → submit_action('trade', ...) — not POST /deals."""
        client = _make_client()
        with patch.object(client, "submit_action", new_callable=AsyncMock) as mock:
            mock.return_value = {"status": "ok"}
            await client.propose_deal({"resource": "wood", "amount": 10})
            mock.assert_awaited_once_with("trade", {"resource": "wood", "amount": 10})

    @pytest.mark.asyncio
    async def test_teach_skill_uses_communicate_action(self) -> None:
        """teach_skill → submit_action('communicate', ...) — not POST /agents/{id}/skills."""
        client = _make_client()
        with patch.object(client, "submit_action", new_callable=AsyncMock) as mock:
            mock.return_value = {"status": "ok"}
            await client.teach_skill("agent-bob", "fishing", 3)
            mock.assert_awaited_once_with("communicate", {
                "target_agent_id": "agent-bob",
                "skill_name": "fishing",
                "level": 3,
            })

    @pytest.mark.asyncio
    async def test_explore_uses_submit_action(self) -> None:
        client = _make_client()
        with patch.object(client, "submit_action", new_callable=AsyncMock) as mock:
            mock.return_value = {"status": "ok"}
            await client.explore({"radius": 5})
            mock.assert_awaited_once_with("explore", {"radius": 5})

    @pytest.mark.asyncio
    async def test_move_uses_submit_action(self) -> None:
        client = _make_client()
        with patch.object(client, "submit_action", new_callable=AsyncMock) as mock:
            mock.return_value = {"status": "ok"}
            await client.move("north")
            mock.assert_awaited_once_with("move", {"direction": "north"})

    @pytest.mark.asyncio
    async def test_gather_uses_submit_action(self) -> None:
        client = _make_client()
        with patch.object(client, "submit_action", new_callable=AsyncMock) as mock:
            mock.return_value = {"status": "ok"}
            await client.gather("wood")
            mock.assert_awaited_once_with("gather", {"resource_type": "wood"})

    @pytest.mark.asyncio
    async def test_build_uses_submit_action(self) -> None:
        client = _make_client()
        with patch.object(client, "submit_action", new_callable=AsyncMock) as mock:
            mock.return_value = {"status": "ok"}
            await client.build("house", location=(0, 1))
            mock.assert_awaited_once_with("build", {"structure_type": "house", "location": (0, 1)})

    @pytest.mark.asyncio
    async def test_submit_action_hits_correct_endpoint(self) -> None:
        """submit_action → POST /api/v1/agents/{agent_id}/action."""
        client = _make_client()
        with patch.object(client, "_request", new_callable=AsyncMock) as mock_req:
            mock_req.return_value = {"status": "ok"}
            await client.submit_action("gather", {"resource_type": "wood"})
            mock_req.assert_awaited_once_with(
                "POST",
                "/api/v1/agents/agent-42/action",
                json={"action": "gather", "params": {"resource_type": "wood"}},
            )

    @pytest.mark.asyncio
    async def test_form_org_hits_api_v1_orgs(self) -> None:
        client = _make_client()
        with patch.object(client, "_request", new_callable=AsyncMock) as mock_req:
            mock_req.return_value = {"status": "ok"}
            await client.form_org({"name": "guild"})
            mock_req.assert_awaited_once_with("POST", "/api/v1/orgs", json={"name": "guild"})

    @pytest.mark.asyncio
    async def test_join_org_hits_api_v1_orgs_join(self) -> None:
        client = _make_client()
        with patch.object(client, "_request", new_callable=AsyncMock) as mock_req:
            mock_req.return_value = {"status": "ok"}
            await client.join_org("org-7", {"agent_id": "agent-42"})
            mock_req.assert_awaited_once_with(
                "POST", "/api/v1/orgs/org-7/join",
                json={"agent_id": "agent-42"},
            )

    @pytest.mark.asyncio
    async def test_broadcast_message_fallback(self) -> None:
        """broadcast_message POSTs to /api/v1/messages; returns fallback dict on error."""
        client = _make_client()
        result = await client.broadcast_message({"text": "hello all"})
        # World Engine is unreachable in tests — should return no_endpoint fallback
        assert result["status"] == "no_endpoint"

    @pytest.mark.asyncio
    async def test_send_message_stringifies_dict_payload(self) -> None:
        """World Engine expects payload as a JSON string, not a map (SEN-718)."""
        import json as _json
        client = _make_client()
        with patch.object(client, "_request", new_callable=AsyncMock) as mock_req:
            mock_req.return_value = {"status": "ok"}
            await client.send_message({
                "from_agent": "a", "to_agent": "",
                "message_type": "INFORM",
                "payload": {"content": "SOS"},
            })
            _args, kwargs = mock_req.call_args
            assert kwargs["json"]["payload"] == _json.dumps({"content": "SOS"}, separators=(",", ":"))

    @pytest.mark.asyncio
    async def test_send_message_keeps_string_payload(self) -> None:
        """A pre-stringified payload should pass through unchanged."""
        client = _make_client()
        with patch.object(client, "_request", new_callable=AsyncMock) as mock_req:
            mock_req.return_value = {"status": "ok"}
            await client.send_message({
                "from_agent": "a", "to_agent": "b",
                "message_type": "INFORM", "payload": "raw-string",
            })
            _args, kwargs = mock_req.call_args
            assert kwargs["json"]["payload"] == "raw-string"

    @pytest.mark.asyncio
    async def test_broadcast_message_stringifies_dict_payload(self) -> None:
        """broadcast_message must JSON-encode the inner payload dict (SEN-718)."""
        import json as _json
        client = _make_client()
        with patch.object(client, "_request", new_callable=AsyncMock) as mock_req:
            mock_req.return_value = {"status": "ok"}
            await client.broadcast_message({
                "type": "INFORM",
                "payload": {"category": "personal", "content": "[SOS] help"},
            })
            _args, kwargs = mock_req.call_args
            sent_body = kwargs["json"]
            assert sent_body["to_agent"] == ""
            assert sent_body["from_agent"] == "agent-42"
            assert sent_body["message_type"] == "INFORM"
            # payload must be a JSON string, not a dict
            assert isinstance(sent_body["payload"], str)
            assert _json.loads(sent_body["payload"]) == {
                "category": "personal", "content": "[SOS] help",
            }


class TestRESTWorldClientFallback:
    """Verify error handling when World Engine is unreachable."""

    @pytest.mark.asyncio
    async def test_send_message_connect_error(self) -> None:
        client = _make_client()
        import httpx
        with patch("httpx.AsyncClient") as mock_client_cls:
            mock_instance = AsyncMock()
            mock_client_cls.return_value.__aenter__ = AsyncMock(return_value=mock_instance)
            mock_client_cls.return_value.__aexit__ = AsyncMock(return_value=False)
            mock_instance.request = AsyncMock(side_effect=httpx.ConnectError("refused"))
            with pytest.raises(httpx.ConnectError):
                await client.send_message({"text": "hi"})

    @pytest.mark.asyncio
    async def test_submit_action_http_error(self) -> None:
        client = _make_client()
        import httpx
        with patch("httpx.AsyncClient") as mock_client_cls:
            mock_instance = AsyncMock()
            mock_client_cls.return_value.__aenter__ = AsyncMock(return_value=mock_instance)
            mock_client_cls.return_value.__aexit__ = AsyncMock(return_value=False)
            mock_response = MagicMock()
            mock_response.status_code = 500
            mock_response.text = "internal error"
            mock_response.raise_for_status = MagicMock(
                side_effect=httpx.HTTPStatusError(
                    "500",
                    request=MagicMock(),
                    response=mock_response,
                )
            )
            mock_instance.request = AsyncMock(return_value=mock_response)
            with pytest.raises(httpx.HTTPStatusError):
                await client.submit_action("gather", {"resource_type": "wood"})


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
