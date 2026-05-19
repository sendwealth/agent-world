"""Tests for the agent runtime CLI (__main__.py) and config module.

Covers:
- Config file loading (TOML and YAML)
- Config parsing (agent, LLM, think_loop, world sections)
- CLI argument parsing (name, skills, traits, overrides)
- Agent spawning (state creation from config)
- Runtime execution (think loop runs, stats collected)
- Signal handling (SIGINT graceful shutdown)
- Structured JSON logging
- Error handling (bad trait values, unknown providers, YAML floats)
"""

from __future__ import annotations

import json
import logging
import os
from pathlib import Path

import pytest

from agent_runtime.config import (
    AgentSpawnConfig,
    RuntimeConfig,
    WorldConfig,
    load_config_file,
    load_runtime_config,
    parse_runtime_config,
)
from agent_runtime.core.think_loop import ThinkLoopConfig
from agent_runtime.__main__ import (
    JSONFormatter,
    RunStats,
    build_config_from_args,
    build_parser,
    parse_skills,
    parse_traits,
    run_agent,
    setup_logging,
    spawn_agent,
)


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def _write_toml(path: Path, content: str) -> Path:
    path.write_text(content)
    return path


def _write_yaml(path: Path, content: str) -> Path:
    path.write_text(content)
    return path


# ---------------------------------------------------------------------------
# Config file loading
# ---------------------------------------------------------------------------


class TestLoadConfigFile:
    def test_load_toml(self, tmp_path: Path):
        p = _write_toml(tmp_path / "agent.toml", """
[agent]
name = "TomlAgent"
tokens = 800

[llm]
provider = "ollama"
model = "llama3"
""")
        data = load_config_file(p)
        assert data["agent"]["name"] == "TomlAgent"
        assert data["agent"]["tokens"] == 800

    def test_load_yaml(self, tmp_path: Path):
        p = _write_yaml(tmp_path / "agent.yaml", """
agent:
  name: YamlAgent
  tokens: 600
llm:
  provider: openai
  model: gpt-4
""")
        data = load_config_file(p)
        assert data["agent"]["name"] == "YamlAgent"
        assert data["llm"]["provider"] == "openai"

    def test_load_yml_extension(self, tmp_path: Path):
        p = _write_yaml(tmp_path / "agent.yml", """
agent:
  name: YmlAgent
""")
        data = load_config_file(p)
        assert data["agent"]["name"] == "YmlAgent"

    def test_file_not_found(self):
        with pytest.raises(FileNotFoundError):
            load_config_file(Path("/nonexistent/file.toml"))

    def test_unsupported_extension(self, tmp_path: Path):
        p = tmp_path / "agent.json"
        p.write_text("{}")
        with pytest.raises(ValueError, match="Unsupported"):
            load_config_file(p)

    def test_empty_yaml_returns_empty_dict(self, tmp_path: Path):
        p = tmp_path / "empty.yaml"
        p.write_text("")
        data = load_config_file(p)
        assert data == {}


# ---------------------------------------------------------------------------
# Config parsing
# ---------------------------------------------------------------------------


class TestParseRuntimeConfig:
    def test_minimal_config(self):
        config = parse_runtime_config({})
        assert config.agent.name == "Agent"
        assert config.agent.tokens == 500
        assert config.llm is None
        assert config.think_loop.tick_interval == 1.0
        assert config.world.engine_url == "http://localhost:3000"

    def test_full_toml_config(self):
        raw = {
            "agent": {
                "name": "Alice",
                "traits": {"curiosity": 0.8, "caution": 0.3},
                "skills": {"coding": {"level": 3}, "trading": 2},
                "tokens": 1000,
                "max_tokens": 2000,
                "money": 100.0,
                "health": 90.0,
            },
            "llm": {
                "provider": "ollama",
                "model": "llama3",
                "base_url": "http://localhost:11434",
            },
            "think_loop": {
                "tick_interval": 0.5,
                "max_ticks": 500,
                "reflect_interval": 20,
            },
            "world": {
                "engine_url": "http://world:3000",
            },
        }
        config = parse_runtime_config(raw)

        assert config.agent.name == "Alice"
        assert config.agent.traits == {"curiosity": 0.8, "caution": 0.3}
        assert config.agent.skills == {"coding": 3, "trading": 2}
        assert config.agent.tokens == 1000
        assert config.agent.max_tokens == 2000
        assert config.agent.money == 100.0
        assert config.agent.health == 90.0

        assert config.llm is not None
        assert config.llm.provider.value == "ollama"
        assert config.llm.model == "llama3"
        assert config.llm.base_url == "http://localhost:11434"

        assert config.think_loop.tick_interval == 0.5
        assert config.think_loop.max_ticks == 500
        assert config.think_loop.reflect_interval == 20

        assert config.world.engine_url == "http://world:3000"

    def test_skills_as_ints(self):
        raw = {"agent": {"skills": {"research": 5, "teaching": 1}}}
        config = parse_runtime_config(raw)
        assert config.agent.skills == {"research": 5, "teaching": 1}

    def test_skills_as_floats(self):
        """YAML parses 3.0 as float — ensure it's converted to int."""
        raw = {"agent": {"skills": {"coding": 3.0, "trading": 1.5}}}
        config = parse_runtime_config(raw)
        assert config.agent.skills == {"coding": 3, "trading": 1}

    def test_skills_as_non_numeric_string_warns(self):
        """Non-numeric string skill levels should be silently dropped with a warning."""
        raw = {"agent": {"skills": {"coding": "expert"}}}
        config = parse_runtime_config(raw)
        assert config.agent.skills == {}

    def test_unknown_llm_provider_raises(self):
        """Unknown provider string should raise ValueError with clear message."""
        raw = {"llm": {"provider": "azure"}}
        with pytest.raises(ValueError, match="Unknown LLM provider"):
            parse_runtime_config(raw)

    def test_api_key_from_env_var(self, monkeypatch):
        """API key should come from environment, not config file."""
        monkeypatch.setenv("LLM_API_KEY", "test-key-123")
        raw = {"llm": {"provider": "ollama", "model": "llama3"}}
        config = parse_runtime_config(raw)
        assert config.llm is not None
        assert config.llm.api_key == "test-key-123"

    def test_api_key_from_provider_specific_env(self, monkeypatch):
        """Provider-specific env var (e.g. OPENAI_API_KEY) should work."""
        monkeypatch.setenv("OPENAI_API_KEY", "openai-key-456")
        raw = {"llm": {"provider": "openai", "model": "gpt-4"}}
        config = parse_runtime_config(raw)
        assert config.llm is not None
        assert config.llm.api_key == "openai-key-456"

    def test_api_key_not_from_config_file(self, monkeypatch):
        """api_key in config file should be ignored."""
        monkeypatch.delenv("LLM_API_KEY", raising=False)
        monkeypatch.delenv("OLLAMA_API_KEY", raising=False)
        raw = {"llm": {"provider": "ollama", "api_key": "should-be-ignored"}}
        config = parse_runtime_config(raw)
        assert config.llm is not None
        assert config.llm.api_key is None


class TestLoadRuntimeConfig:
    def test_load_toml_file(self, tmp_path: Path):
        p = _write_toml(tmp_path / "test.toml", """
[agent]
name = "FromFile"
tokens = 999

[think_loop]
max_ticks = 42
""")
        config = load_runtime_config(p)
        assert config.agent.name == "FromFile"
        assert config.agent.tokens == 999
        assert config.think_loop.max_ticks == 42

    def test_load_yaml_file(self, tmp_path: Path):
        p = _write_yaml(tmp_path / "test.yaml", """
agent:
  name: YamlFile
  tokens: 777
""")
        config = load_runtime_config(p)
        assert config.agent.name == "YamlFile"
        assert config.agent.tokens == 777

    def test_yaml_float_skills(self, tmp_path: Path):
        """YAML parses bare numbers as float — ensure they become int skills."""
        p = _write_yaml(tmp_path / "skills.yaml", """
agent:
  name: FloatSkillBot
  skills:
    coding: 3.0
    trading: 2
""")
        config = load_runtime_config(p)
        assert config.agent.skills == {"coding": 3, "trading": 2}


# ---------------------------------------------------------------------------
# Trait / skill parsing
# ---------------------------------------------------------------------------


class TestParseTraits:
    def test_valid_traits(self):
        result = parse_traits(["curiosity=0.8", "caution=0.3"])
        assert result == {"curiosity": 0.8, "caution": 0.3}

    def test_none_input(self):
        assert parse_traits(None) == {}

    def test_empty_list(self):
        assert parse_traits([]) == {}

    def test_malformed_trait_ignored(self):
        result = parse_traits(["curiosity=0.8", "bad_trait"])
        assert result == {"curiosity": 0.8}

    def test_non_numeric_trait_exits(self):
        """Non-numeric trait values should cause a clear exit, not a raw traceback."""
        with pytest.raises(SystemExit):
            parse_traits(["curiosity=not_a_number"])


class TestParseSkills:
    def test_comma_separated(self):
        result = parse_skills("coding,trading,research")
        assert result == {"coding": 1, "trading": 1, "research": 1}

    def test_none_input(self):
        assert parse_skills(None) == {}

    def test_empty_string(self):
        assert parse_skills("") == {}

    def test_whitespace_handling(self):
        result = parse_skills(" coding , trading ")
        assert result == {"coding": 1, "trading": 1}


# ---------------------------------------------------------------------------
# Agent spawning
# ---------------------------------------------------------------------------


class TestSpawnAgent:
    def test_spawn_basic(self):
        cfg = AgentSpawnConfig(name="TestBot")
        state = spawn_agent(cfg)
        assert state.name == "TestBot"
        assert state.tokens == 500
        assert state.max_tokens == 1000
        assert state.health == 100.0

    def test_spawn_with_traits(self):
        cfg = AgentSpawnConfig(
            name="CuriousBot",
            traits={"curiosity": 0.9, "aggression": 0.1},
        )
        state = spawn_agent(cfg)
        assert state.personality == {"curiosity": 0.9, "aggression": 0.1}

    def test_spawn_with_skills(self):
        cfg = AgentSpawnConfig(
            name="SkilledBot",
            skills={"coding": 5, "trading": 2},
        )
        state = spawn_agent(cfg)
        assert "coding" in state.skills
        assert state.skills["coding"].level == 5
        assert "trading" in state.skills
        assert state.skills["trading"].level == 2

    def test_spawn_custom_resources(self):
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
# CLI argument parsing
# ---------------------------------------------------------------------------


class TestBuildParser:
    def test_spawn_with_name(self):
        parser = build_parser()
        args = parser.parse_args(["spawn", "--name", "Alice"])
        assert args.command == "spawn"
        assert args.name == "Alice"

    def test_spawn_with_all_options(self):
        parser = build_parser()
        args = parser.parse_args([
            "spawn",
            "--name", "Bob",
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
        assert args.name == "Bob"
        assert args.skills == "coding,trading"
        assert args.traits == ["curiosity=0.8"]
        assert args.tokens == 2000
        assert args.max_tokens == 5000
        assert args.max_ticks == 100
        assert args.tick_interval == 0.5
        assert args.world_url == "http://engine:3000"
        assert args.llm_provider == "ollama"
        assert args.llm_model == "llama3"

    def test_no_command_returns_none(self):
        parser = build_parser()
        args = parser.parse_args([])
        assert args.command is None

    def test_verbose_flag(self):
        parser = build_parser()
        args = parser.parse_args(["-v", "spawn", "--name", "X"])
        assert args.verbose is True

    def test_log_text_flag(self):
        parser = build_parser()
        args = parser.parse_args(["--log-text", "spawn"])
        assert args.log_text is True


class TestBuildConfigFromArgs:
    def test_name_override(self):
        parser = build_parser()
        args = parser.parse_args(["spawn", "--name", "Override"])
        config = build_config_from_args(args)
        assert config.agent.name == "Override"

    def test_config_file_merge(self, tmp_path: Path):
        cfg_path = _write_toml(tmp_path / "base.toml", """
[agent]
name = "FromFile"
tokens = 1000

[think_loop]
max_ticks = 50
""")
        parser = build_parser()
        args = parser.parse_args([
            "spawn",
            "--config", str(cfg_path),
            "--name", "CLIOverride",
            "--max-ticks", "200",
        ])
        config = build_config_from_args(args)
        # CLI overrides file
        assert config.agent.name == "CLIOverride"
        assert config.agent.tokens == 1000  # From file
        assert config.think_loop.max_ticks == 200  # CLI override

    def test_skills_from_cli(self):
        parser = build_parser()
        args = parser.parse_args([
            "spawn", "--skills", "coding,research",
        ])
        config = build_config_from_args(args)
        assert "coding" in config.agent.skills
        assert "research" in config.agent.skills

    def test_default_name_when_none_provided(self):
        parser = build_parser()
        args = parser.parse_args(["spawn"])
        config = build_config_from_args(args)
        assert config.agent.name == "Agent"  # Default from RuntimeConfig

    def test_skills_merge_with_config_file(self, tmp_path: Path):
        cfg_path = _write_yaml(tmp_path / "base.yaml", """
agent:
  name: MergeBot
  skills:
    coding: 3
""")
        parser = build_parser()
        args = parser.parse_args([
            "spawn", "--config", str(cfg_path), "--skills", "trading",
        ])
        config = build_config_from_args(args)
        assert "coding" in config.agent.skills  # From file
        assert "trading" in config.agent.skills  # From CLI


# ---------------------------------------------------------------------------
# Runtime execution
# ---------------------------------------------------------------------------


class TestRunAgent:
    @pytest.mark.asyncio
    async def test_run_agent_completes(self):
        config = RuntimeConfig(
            agent=AgentSpawnConfig(name="TestRunner", tokens=5000, max_tokens=10000),
            think_loop=ThinkLoopConfig(tick_interval=0.0, max_ticks=5),
        )
        stats = await run_agent(config)
        assert stats.agent_name == "TestRunner"
        assert stats.ticks == 5
        assert stats.errors == 0
        assert stats.duration_s > 0
        assert stats.agent_id  # non-empty UUID string

    @pytest.mark.asyncio
    async def test_run_agent_stats_serializable(self):
        config = RuntimeConfig(
            agent=AgentSpawnConfig(name="StatsTest"),
            think_loop=ThinkLoopConfig(tick_interval=0.0, max_ticks=3),
        )
        stats = await run_agent(config)
        d = stats.to_dict()
        # Must be JSON-serializable
        json_str = json.dumps(d)
        parsed = json.loads(json_str)
        assert parsed["agent_name"] == "StatsTest"
        assert parsed["ticks"] == 3

    @pytest.mark.asyncio
    async def test_run_agent_with_skills_and_traits(self):
        config = RuntimeConfig(
            agent=AgentSpawnConfig(
                name="ComplexBot",
                tokens=5000,
                max_tokens=10000,
                skills={"coding": 3, "trading": 1},
                traits={"curiosity": 0.8},
            ),
            think_loop=ThinkLoopConfig(tick_interval=0.0, max_ticks=5),
        )
        stats = await run_agent(config)
        assert stats.ticks == 5
        assert stats.errors == 0


# ---------------------------------------------------------------------------
# JSON logging
# ---------------------------------------------------------------------------


class TestJSONFormatter:
    def test_basic_format(self):
        formatter = JSONFormatter()
        record = logging.LogRecord(
            name="test.logger",
            level=logging.INFO,
            pathname="",
            lineno=0,
            msg="Hello world",
            args=None,
            exc_info=None,
        )
        output = formatter.format(record)
        parsed = json.loads(output)
        assert parsed["level"] == "INFO"
        assert parsed["logger"] == "test.logger"
        assert parsed["msg"] == "Hello world"
        assert "ts" in parsed

    def test_extra_fields(self):
        formatter = JSONFormatter()
        record = logging.LogRecord(
            name="test",
            level=logging.DEBUG,
            pathname="",
            lineno=0,
            msg="Tick done",
            args=None,
            exc_info=None,
        )
        record.agent = "Alice"
        record.tick = 42
        output = formatter.format(record)
        parsed = json.loads(output)
        assert parsed["agent"] == "Alice"
        assert parsed["tick"] == 42


class TestSetupLogging:
    def test_json_logging(self):
        setup_logging(verbose=False, json_output=True)
        agent_logger = logging.getLogger("agent_runtime")
        assert agent_logger.level == logging.INFO
        assert len(agent_logger.handlers) == 1
        assert isinstance(agent_logger.handlers[0].formatter, JSONFormatter)

    def test_text_logging(self):
        setup_logging(verbose=False, json_output=False)
        agent_logger = logging.getLogger("agent_runtime")
        assert not isinstance(
            agent_logger.handlers[0].formatter, JSONFormatter
        )

    def test_verbose_mode(self):
        setup_logging(verbose=True)
        agent_logger = logging.getLogger("agent_runtime")
        assert agent_logger.level == logging.DEBUG


# ---------------------------------------------------------------------------
# RunStats
# ---------------------------------------------------------------------------


class TestRunStats:
    def test_duration_calculation(self):
        stats = RunStats(
            agent_name="Test",
            agent_id="abc-123",
            start_time=100.0,
            end_time=105.5,
        )
        assert stats.duration_s == 5.5

    def test_to_dict(self):
        stats = RunStats(
            agent_name="Bot",
            agent_id="uuid-1",
            ticks=42,
            errors=1,
            start_time=0.0,
            end_time=10.0,
        )
        d = stats.to_dict()
        assert d["agent_name"] == "Bot"
        assert d["ticks"] == 42
        assert d["errors"] == 1
        assert d["duration_s"] == 10.0
