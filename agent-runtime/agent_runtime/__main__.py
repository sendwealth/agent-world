"""Agent Runtime CLI — ``python -m agent_runtime``.

Usage::

    # Spawn a single agent with defaults
    python -m agent_runtime spawn --name Alice

    # Spawn with skills and traits
    python -m agent_runtime spawn --name Bob --skills coding,trading --traits curiosity=0.8

    # Use a config file
    python -m agent_runtime spawn --config agent.toml

    # Limit ticks for testing
    python -m agent_runtime spawn --name TestAgent --max-ticks 100

    # Connect to a specific world engine
    python -m agent_runtime spawn --name Alice --world-url http://localhost:3000
"""

from __future__ import annotations

import argparse
import asyncio
import json
import logging
import os
import signal
import sys
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from agent_runtime import __version__
from agent_runtime.config import (
    AgentSpawnConfig,
    RuntimeConfig,
    load_runtime_config,
)
from agent_runtime.core.act import ActionExecutor
from agent_runtime.core.think_loop import ThinkLoop
from agent_runtime.llm.base import LLMConfig, ProviderType
from agent_runtime.models.agent_state import AgentState
from agent_runtime.models.skill import Skill
from agent_runtime.survival.instinct import SurvivalInstinct

logger = logging.getLogger(__name__)


# ---------------------------------------------------------------------------
# Structured JSON log formatter
# ---------------------------------------------------------------------------


class JSONFormatter(logging.Formatter):
    """Emit log records as single-line JSON objects.

    Each record becomes::

        {
          "ts": "2024-01-15T10:30:00.123456",
          "level": "INFO",
          "logger": "agent_runtime.core.think_loop",
          "msg": "ThinkLoop started",
          "agent": "Alice",         // present when bound
          "tick": 42,               // present when bound
          ...extra fields...
        }
    """

    def format(self, record: logging.LogRecord) -> str:
        entry: dict[str, Any] = {
            "ts": time.strftime("%Y-%m-%dT%H:%M:%S", time.gmtime(record.created))
            + f".{int(record.created % 1 * 1_000_000):06d}",
            "level": record.levelname,
            "logger": record.name,
            "msg": record.getMessage(),
        }

        # Attach structured extras if present
        for key in ("agent", "tick", "action", "duration_s", "event"):
            val = getattr(record, key, None)
            if val is not None:
                entry[key] = val

        if record.exc_info and record.exc_info[1] is not None:
            entry["error"] = str(record.exc_info[1])

        return json.dumps(entry, default=str, ensure_ascii=False)


def setup_logging(verbose: bool = False, json_output: bool = True) -> None:
    """Configure structured JSON logging for the runtime.

    Args:
        verbose: Set to True for DEBUG level, False for INFO.
        json_output: Use JSON formatter (default True).
    """
    level = logging.DEBUG if verbose else logging.INFO
    handler = logging.StreamHandler(sys.stderr)
    handler.setLevel(level)

    if json_output:
        handler.setFormatter(JSONFormatter())
    else:
        handler.setFormatter(
            logging.Formatter("%(asctime)s [%(levelname)s] %(name)s: %(message)s")
        )

    root = logging.getLogger("agent_runtime")
    root.setLevel(level)
    root.handlers.clear()
    root.addHandler(handler)


# ---------------------------------------------------------------------------
# Agent spawner
# ---------------------------------------------------------------------------


def spawn_agent(config: AgentSpawnConfig) -> AgentState:
    """Create an AgentState from spawn configuration.

    Initializes agent state with the specified name, traits as personality,
    and skills from the config.
    """
    state = AgentState(
        name=config.name,
        tokens=config.tokens,
        max_tokens=config.max_tokens,
        money=config.money,
        health=config.health,
        personality=config.traits,
    )

    # Register initial skills
    for skill_name, level in config.skills.items():
        state.add_skill(Skill(name=skill_name, level=level))

    logger.info(
        "Agent spawned",
        extra={
            "agent": config.name,
            "event": "agent_spawned",
            "tokens": config.tokens,
            "skills": list(config.skills.keys()),
            "traits": config.traits,
        },
    )
    return state


# ---------------------------------------------------------------------------
# Main runtime
# ---------------------------------------------------------------------------


@dataclass
class RunStats:
    """Statistics collected during a run."""

    agent_name: str
    agent_id: str
    ticks: int = 0
    errors: int = 0
    start_time: float = 0.0
    end_time: float = 0.0

    @property
    def duration_s(self) -> float:
        return self.end_time - self.start_time

    def to_dict(self) -> dict[str, Any]:
        return {
            "agent_name": self.agent_name,
            "agent_id": self.agent_id,
            "ticks": self.ticks,
            "errors": self.errors,
            "duration_s": round(self.duration_s, 2),
        }


def _register_signal_handler(loop: asyncio.AbstractEventLoop, callback: Any) -> None:
    """Register a SIGINT handler, with a cross-platform fallback.

    ``loop.add_signal_handler`` is Unix-only. On Windows we fall back to
    ``signal.signal()`` which works but runs the callback synchronously.
    """
    try:
        loop.add_signal_handler(signal.SIGINT, callback)
    except NotImplementedError:
        # Windows fallback
        signal.signal(signal.SIGINT, lambda *_: callback())


def _unregister_signal_handler(loop: asyncio.AbstractEventLoop) -> None:
    """Remove the SIGINT handler registered by ``_register_signal_handler``."""
    try:
        loop.remove_signal_handler(signal.SIGINT)
    except (NotImplementedError, OSError):
        # Windows: restore default handler
        signal.signal(signal.SIGINT, signal.SIG_DFL)


async def run_agent(config: RuntimeConfig) -> RunStats:
    """Spawn an agent and run its think loop until signalled to stop.

    This is the main runtime entry point.  It:
    1. Creates the agent state from config.
    2. Wires up survival instinct and action executor.
    3. Runs the observe-think-decide-act loop.
    4. Handles SIGINT for graceful shutdown.
    """
    state = spawn_agent(config.agent)
    stats = RunStats(
        agent_name=state.name,
        agent_id=str(state.id),
    )

    # Set up components
    survival = SurvivalInstinct()
    executor = ActionExecutor()

    think_loop = ThinkLoop(
        state=state,
        survival=survival,
        executor=executor,
        config=config.think_loop,
    )

    # Graceful shutdown on SIGINT
    loop = asyncio.get_running_loop()

    def _signal_handler() -> None:
        logger.info(
            "SIGINT received — shutting down gracefully",
            extra={"agent": state.name, "event": "shutdown_signal"},
        )
        think_loop.stop()

    _register_signal_handler(loop, _signal_handler)

    logger.info(
        "Starting agent runtime",
        extra={
            "agent": state.name,
            "event": "runtime_start",
            "config": {
                "tick_interval": config.think_loop.tick_interval,
                "max_ticks": config.think_loop.max_ticks or "unlimited",
                "world_url": config.world.engine_url,
            },
        },
    )

    stats.start_time = time.monotonic()

    try:
        await think_loop.run()
    finally:
        stats.end_time = time.monotonic()
        stats.ticks = think_loop.tick
        stats.errors = think_loop.total_errors
        _unregister_signal_handler(loop)

    logger.info(
        "Agent runtime stopped",
        extra={
            "agent": state.name,
            "event": "runtime_stop",
            "ticks": stats.ticks,
            "errors": stats.errors,
            "duration_s": round(stats.duration_s, 2),
        },
    )

    return stats


# ---------------------------------------------------------------------------
# CLI argument parsing
# ---------------------------------------------------------------------------


def build_parser() -> argparse.ArgumentParser:
    """Build the argument parser for the CLI."""
    parser = argparse.ArgumentParser(
        prog="agent_runtime",
        description="Agent World — Agent Runtime CLI. Spawn and run AI agents.",
    )
    parser.add_argument(
        "--version", action="version", version=f"%(prog)s {__version__}"
    )
    parser.add_argument(
        "-v", "--verbose", action="store_true", help="Enable debug logging"
    )
    parser.add_argument(
        "--log-text", action="store_true",
        help="Use human-readable log format instead of JSON (default: JSON)",
    )

    sub = parser.add_subparsers(dest="command", help="Available commands")

    # -- spawn --
    spawn_parser = sub.add_parser(
        "spawn", help="Spawn and run a single agent"
    )
    spawn_parser.add_argument(
        "--name", default=None, help="Agent name (default: Agent)"
    )
    spawn_parser.add_argument(
        "--config", type=Path, default=None,
        help="Path to TOML or YAML config file",
    )
    spawn_parser.add_argument(
        "--skills", default=None,
        help="Comma-separated skill names (e.g. coding,trading,research)",
    )
    spawn_parser.add_argument(
        "--traits", nargs="*", default=None,
        help="Personality traits as key=value pairs (e.g. curiosity=0.8 caution=0.3)",
    )
    spawn_parser.add_argument(
        "--tokens", type=int, default=None,
        help="Initial token balance",
    )
    spawn_parser.add_argument(
        "--max-tokens", type=int, default=None,
        help="Maximum token capacity",
    )
    spawn_parser.add_argument(
        "--max-ticks", type=int, default=None,
        help="Maximum ticks to run (0 = unlimited)",
    )
    spawn_parser.add_argument(
        "--tick-interval", type=float, default=None,
        help="Seconds between ticks",
    )
    spawn_parser.add_argument(
        "--world-url", default=None,
        help="World Engine URL (default: http://localhost:3000)",
    )
    spawn_parser.add_argument(
        "--llm-provider", choices=["openai", "anthropic", "ollama"], default=None,
        help="LLM provider",
    )
    spawn_parser.add_argument(
        "--llm-model", default=None,
        help="LLM model name",
    )
    spawn_parser.add_argument(
        "--llm-base-url", default=None,
        help="LLM API base URL",
    )

    return parser


def parse_traits(trait_args: list[str] | None) -> dict[str, float]:
    """Parse trait arguments like ['curiosity=0.8', 'caution=0.3']."""
    if not trait_args:
        return {}
    traits: dict[str, float] = {}
    for item in trait_args:
        if "=" in item:
            key, val = item.split("=", 1)
            try:
                traits[key.strip()] = float(val.strip())
            except ValueError:
                logger.error(
                    "Invalid trait value for %r: %r (expected a number)",
                    key.strip(), val,
                )
                sys.exit(1)
        else:
            logger.warning("Ignoring malformed trait: %r (expected key=value)", item)
    return traits


def parse_skills(skill_str: str | None) -> dict[str, int]:
    """Parse a comma-separated skill string like 'coding,trading'."""
    if not skill_str:
        return {}
    skills: dict[str, int] = {}
    for name in skill_str.split(","):
        name = name.strip()
        if name:
            skills[name] = 1  # Default level 1
    return skills


def build_config_from_args(args: argparse.Namespace) -> RuntimeConfig:
    """Build a RuntimeConfig from CLI arguments, optionally merging with a config file."""
    # Start from config file if provided
    if args.config is not None:
        config = load_runtime_config(args.config)
    else:
        config = RuntimeConfig()

    # CLI overrides for agent
    if args.name is not None:
        config.agent.name = args.name
    if args.skills is not None:
        config.agent.skills.update(parse_skills(args.skills))
    if args.traits is not None:
        config.agent.traits.update(parse_traits(args.traits))
    if args.tokens is not None:
        config.agent.tokens = args.tokens
    if args.max_tokens is not None:
        config.agent.max_tokens = args.max_tokens

    # CLI overrides for think loop
    if args.max_ticks is not None:
        config.think_loop.max_ticks = args.max_ticks
    if args.tick_interval is not None:
        config.think_loop.tick_interval = args.tick_interval

    # CLI overrides for world
    if args.world_url is not None:
        config.world.engine_url = args.world_url

    # CLI overrides for LLM
    if args.llm_provider is not None or args.llm_model is not None:
        if config.llm is None:
            config.llm = LLMConfig(
                provider=ProviderType(args.llm_provider or "ollama"),
                model=args.llm_model or "llama3",
            )
        else:
            if args.llm_provider is not None:
                config.llm.provider = ProviderType(args.llm_provider)
            if args.llm_model is not None:
                config.llm.model = args.llm_model
        if args.llm_base_url is not None and config.llm is not None:
            config.llm.base_url = args.llm_base_url

    return config


# ---------------------------------------------------------------------------
# Entry point
# ---------------------------------------------------------------------------


def main() -> None:
    """CLI entry point — parse args and run."""
    parser = build_parser()
    args = parser.parse_args()

    if args.command is None:
        parser.print_help()
        sys.exit(1)

    setup_logging(verbose=args.verbose, json_output=not args.log_text)

    logger.info(
        "Agent Runtime CLI starting",
        extra={"version": __version__, "command": args.command},
    )

    if args.command == "spawn":
        config = build_config_from_args(args)
        try:
            stats = asyncio.run(run_agent(config))
        except KeyboardInterrupt:
            # Graceful handling: user pressed Ctrl+C during asyncio.run()
            # which cancels the coroutine — stats summary is skipped, that's OK.
            logger.info(
                "Agent runtime interrupted by user",
                extra={"event": "keyboard_interrupt"},
            )
            sys.exit(0)
        # Print final summary to stdout (always human-readable)
        print(json.dumps(stats.to_dict(), indent=2))
    else:
        parser.print_help()
        sys.exit(1)


if __name__ == "__main__":
    main()
