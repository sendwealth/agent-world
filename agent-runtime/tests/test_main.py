"""Tests for the __main__.py CLI entry point."""

from __future__ import annotations

import pytest

from agent_runtime.__main__ import (
    async_main,
    build_agent_state,
    build_parser,
    load_config,
)
from agent_runtime.models.enums import AgentPhase

# ---------------------------------------------------------------------------
# Parser tests
# ---------------------------------------------------------------------------


class TestBuildParser:
    def test_name_required(self) -> None:
        parser = build_parser()
        with pytest.raises(SystemExit):
            parser.parse_args([])

    def test_name_provided(self) -> None:
        parser = build_parser()
        args = parser.parse_args(["--name", "Alpha"])
        assert args.name == "Alpha"

    def test_all_options(self) -> None:
        parser = build_parser()
        args = parser.parse_args([
            "--name", "Alpha",
            "--config", "agent.yaml",
            "--seed", "500",
            "--server", "localhost:9090",
            "--tick-interval", "0.5",
            "--max-ticks", "100",
            "--log-level", "DEBUG",
        ])
        assert args.name == "Alpha"
        assert args.config == "agent.yaml"
        assert args.seed == 500
        assert args.server == "localhost:9090"
        assert args.tick_interval == 0.5
        assert args.max_ticks == 100
        assert args.log_level == "DEBUG"

    def test_defaults(self) -> None:
        parser = build_parser()
        args = parser.parse_args(["--name", "Test"])
        assert args.config is None
        assert args.seed is None
        assert args.server == "localhost:50051"
        assert args.tick_interval == 1.0
        assert args.max_ticks == 0
        assert args.log_level == "INFO"


# ---------------------------------------------------------------------------
# Config loading tests
# ---------------------------------------------------------------------------


class TestLoadConfig:
    def test_none_returns_empty(self, tmp_path: object) -> None:
        assert load_config(None) == {}

    def test_missing_file_returns_empty(self) -> None:
        assert load_config("/nonexistent/path.yaml") == {}

    def test_valid_yaml(self, tmp_path: object) -> None:
        import pathlib

        p = pathlib.Path(str(tmp_path)) / "config.yaml"
        p.write_text("economy:\n  initial_tokens: 2000\nmax_tokens: 5000\n")
        result = load_config(str(p))
        assert result == {"economy": {"initial_tokens": 2000}, "max_tokens": 5000}

    def test_empty_file(self, tmp_path: object) -> None:
        import pathlib

        p = pathlib.Path(str(tmp_path)) / "empty.yaml"
        p.write_text("")
        result = load_config(str(p))
        assert result == {}


# ---------------------------------------------------------------------------
# Agent state bootstrap tests
# ---------------------------------------------------------------------------


class TestBuildAgentState:
    def test_basic_creation(self) -> None:
        state = build_agent_state("Alpha", None, {})
        assert state.name == "Alpha"
        assert state.phase == AgentPhase.INITIALIZATION

    def test_seed_overrides_config(self) -> None:
        config = {"lifecycle": {"birth_tokens": 2000}, "economy": {"initial_tokens": 1000}}
        state = build_agent_state("Alpha", 500, config)
        assert state.tokens == 500

    def test_config_tokens_when_no_seed(self) -> None:
        config = {"lifecycle": {"birth_tokens": 2000}}
        state = build_agent_state("Alpha", None, config)
        assert state.tokens == 2000

    def test_default_tokens(self) -> None:
        state = build_agent_state("Alpha", None, {})
        assert state.tokens == 1000

    def test_max_tokens_from_config(self) -> None:
        config = {"max_tokens": 5000}
        state = build_agent_state("Alpha", None, config)
        assert state.max_tokens == 5000

    def test_default_max_tokens(self) -> None:
        state = build_agent_state("Alpha", None, {})
        assert state.max_tokens == 100_000


# ---------------------------------------------------------------------------
# async_main integration tests
# ---------------------------------------------------------------------------


class TestAsyncMain:
    @pytest.mark.asyncio
    async def test_runs_with_max_ticks(self) -> None:
        """Smoke test: the agent runs and exits after max_ticks."""
        await async_main([
            "--name", "TestAgent",
            "--seed", "50000",
            "--max-ticks", "3",
            "--tick-interval", "0.05",
            "--log-level", "WARNING",
        ])

    @pytest.mark.asyncio
    async def test_missing_name_exits(self) -> None:
        """Missing --name should cause SystemExit."""
        with pytest.raises(SystemExit):
            await async_main([])
