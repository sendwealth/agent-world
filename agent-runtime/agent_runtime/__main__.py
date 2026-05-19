"""Agent Runtime CLI — ``python -m agent_runtime``.

Complete P2.5 integration: CLI argument parsing, World Engine connection
(gRPC with REST fallback), agent registration, full ThinkLoop wiring
with memory-aware decisions, A2A messaging, reflection, and survival
instinct.

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
    WorldConfig,
    load_runtime_config,
    parse_runtime_config,
)
from agent_runtime.env_loader import load_dotenv
from agent_runtime.core.act import ActionExecutor
from agent_runtime.core.think_loop import ThinkLoop, ThinkLoopConfig
from agent_runtime.llm.base import LLMConfig, ProviderType
from agent_runtime.models.agent_state import AgentState
from agent_runtime.survival.instinct import SurvivalInstinct

logger = logging.getLogger(__name__)


# ---------------------------------------------------------------------------
# Structured JSON log formatter
# ---------------------------------------------------------------------------


class JSONFormatter(logging.Formatter):
    """Emit log records as single-line JSON objects."""

    def format(self, record: logging.LogRecord) -> str:
        entry: dict[str, Any] = {
            "ts": time.strftime("%Y-%m-%dT%H:%M:%S", time.gmtime(record.created))
            + f".{int(record.created % 1 * 1_000_000):06d}",
            "level": record.levelname,
            "logger": record.name,
            "msg": record.getMessage(),
        }

        for key in ("agent", "tick", "action", "duration_s", "event"):
            val = getattr(record, key, None)
            if val is not None:
                entry[key] = val

        if record.exc_info and record.exc_info[1] is not None:
            entry["error"] = str(record.exc_info[1])

        return json.dumps(entry, default=str, ensure_ascii=False)


def setup_logging(verbose: bool = False, json_output: bool = True) -> None:
    """Configure structured JSON logging for the runtime."""
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
# REST fallback client (module-level, used when gRPC is unavailable)
# ---------------------------------------------------------------------------


class RESTWorldClient:
    """REST-based fallback World Client for when gRPC is unavailable.

    All methods log warnings because the REST API is not yet implemented
    on the server side. This is a placeholder that allows the agent to
    run in standalone mode.
    """

    def __init__(self, base_url: str) -> None:
        self._base_url = base_url.rstrip("/")

    async def _request(self, method: str, path: str, **kwargs: Any) -> dict[str, Any]:
        logger.warning(
            "REST fallback: %s %s (not implemented, running standalone)",
            method, path,
        )
        return {"status": "standalone", "method": method, "path": path}

    async def send_message(self, payload: dict[str, Any]) -> dict[str, Any]:
        return await self._request("POST", "/messages", json=payload)

    async def claim_task(self, task_id: str) -> dict[str, Any]:
        return await self._request("POST", f"/tasks/{task_id}/claim")

    async def submit_task(
        self, task_id: str, result: dict[str, Any]
    ) -> dict[str, Any]:
        return await self._request("POST", f"/tasks/{task_id}/submit", json=result)

    async def propose_deal(self, proposal: dict[str, Any]) -> dict[str, Any]:
        return await self._request("POST", "/deals", json=proposal)

    async def teach_skill(
        self, target_agent_id: str, skill_name: str, level: int
    ) -> dict[str, Any]:
        return await self._request(
            "POST", f"/agents/{target_agent_id}/skills/{skill_name}",
            json={"level": level},
        )

    async def explore(self, parameters: dict[str, Any]) -> dict[str, Any]:
        return await self._request("GET", "/explore", params=parameters)

    async def broadcast_message(
        self, payload: dict[str, object]
    ) -> dict[str, object]:
        return await self._request("POST", "/broadcast", json=payload)  # type: ignore[return-value]


# ---------------------------------------------------------------------------
# World Engine connection (gRPC with REST fallback)
# ---------------------------------------------------------------------------


async def connect_world_engine(
    grpc_address: str,
    rest_url: str,
    agent_id: str,
) -> Any | None:
    """Connect to the World Engine via gRPC, falling back to REST.

    Tries gRPC first (preferred).  If the gRPC server is unreachable,
    creates a REST fallback client so the agent can still run.

    Returns:
        A world client (GRPCWorldClient, RESTWorldClient), or None
        if neither connection method works.
    """
    # Try gRPC first
    try:
        from agent_runtime.a2a.client import A2AClient
        from agent_runtime.a2a.config import A2AClientConfig
        from agent_runtime.a2a.world_client import GRPCWorldClient

        config = A2AClientConfig(
            server_address=grpc_address,
            agent_id=agent_id,
        )
        client = A2AClient(config)
        await client.connect()
        world_client = GRPCWorldClient(client)
        logger.info(
            "Connected to World Engine via gRPC at %s",
            grpc_address,
            extra={"agent": agent_id, "event": "grpc_connected"},
        )
        return world_client
    except ImportError:
        logger.info("gRPC dependencies not available, using REST fallback")
    except Exception:
        logger.warning(
            "Could not connect to World Engine via gRPC at %s — falling back to REST",
            grpc_address,
        )

    # REST fallback
    rest_client = RESTWorldClient(rest_url)
    logger.info(
        "Using REST fallback for World Engine at %s",
        rest_url,
        extra={"agent": agent_id, "event": "rest_fallback"},
    )
    return rest_client


async def register_agent(
    state: AgentState,
    world_url: str,
    *,
    timeout: float = 5.0,
) -> bool:
    """Attempt to register the agent with the World Engine REST API.

    Non-fatal: if the World Engine is unreachable the agent runs in
    standalone mode.
    """
    try:
        import httpx
    except ImportError:
        logger.info("httpx not available, skipping agent registration")
        return False

    url = f"{world_url.rstrip('/')}/agents"
    payload = state.to_sync_payload()

    logger.info(
        "Registering agent %s (%s) with World Engine at %s",
        state.name, state.id, url,
    )

    try:
        async with httpx.AsyncClient(timeout=timeout) as client:
            resp = await client.post(url, json=payload)
            if resp.status_code in (200, 201):
                logger.info("Agent registered successfully")
                return True
            logger.warning(
                "World Engine returned %d: %s",
                resp.status_code,
                resp.text[:200] if resp.text else "(empty)",
            )
            return False
    except httpx.ConnectError:
        logger.warning(
            "World Engine unreachable at %s — running in standalone mode",
            world_url,
        )
        return False
    except Exception:
        logger.exception("Failed to register with World Engine")
        return False


# ---------------------------------------------------------------------------
# Agent spawner
# ---------------------------------------------------------------------------


def spawn_agent(config: AgentSpawnConfig) -> AgentState:
    """Create an AgentState from spawn configuration."""
    state = AgentState(
        name=config.name,
        tokens=config.tokens,
        max_tokens=config.max_tokens,
        money=config.money,
        health=config.health,
        personality=config.traits,
    )

    for skill_name, level in config.skills.items():
        from agent_runtime.models.skill import Skill
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


async def run_agent(config: RuntimeConfig) -> RunStats:
    """Spawn an agent and run its think loop until signalled to stop."""
    state = spawn_agent(config.agent)
    stats = RunStats(
        agent_name=state.name,
        agent_id=str(state.id),
    )

    # Set up core components
    survival = SurvivalInstinct()
    executor = ActionExecutor()

    # Build decision provider (memory-aware if vector memory available)
    decision_provider = _build_decision_provider(config, executor)

    # Connect to World Engine (gRPC preferred, REST fallback)
    grpc_address = _extract_grpc_address(config.world.engine_url)
    world_client = await connect_world_engine(
        grpc_address=grpc_address,
        rest_url=config.world.engine_url,
        agent_id=str(state.id),
    )

    # Attempt registration
    await register_agent(state, config.world.engine_url)

    # Build ThinkLoop with all providers wired in via constructor
    think_loop = ThinkLoop(
        state=state,
        survival=survival,
        executor=executor,
        config=config.think_loop,
        decision_provider=decision_provider,
        world_client=world_client,
    )

    # Graceful shutdown on SIGINT
    loop = asyncio.get_running_loop()
    shutdown_event = asyncio.Event()

    def _signal_handler() -> None:
        logger.info(
            "SIGINT received — shutting down gracefully",
            extra={"agent": state.name, "event": "shutdown_signal"},
        )
        think_loop.stop()
        shutdown_event.set()

    loop.add_signal_handler(signal.SIGINT, _signal_handler)

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
        try:
            loop.remove_signal_handler(signal.SIGINT)
        except (ValueError, OSError) as exc:
            logger.warning("Failed to remove signal handler: %s", exc)

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


def _build_decision_provider(
    config: RuntimeConfig, executor: ActionExecutor
) -> Any | None:
    """Build the best available decision provider.

    Priority:
      1. Memory-aware provider wrapping LLMDecisionProvider (if LLM + memory deps available)
      2. LLMDecisionProvider (if LLM config available, no memory)
      3. Memory-aware provider wrapping MockDecisionProvider (if memory deps available, no LLM)
      4. None (ThinkLoop falls back to MockDecisionProvider)
    """
    # Build the LLM-backed decision provider if config is available
    llm_provider = _create_llm_decision_provider(config)

    # Try to wrap with memory-aware provider
    try:
        from agent_runtime.core.memory_aware_decide import MemoryAwareDecisionProvider
        from agent_runtime.memory.vector_memory import VectorMemory
        from agent_runtime.memory.memory_recall import MemoryRecall

        vector_memory = VectorMemory()
        memory_recall = MemoryRecall(vector_memory=vector_memory)

        if llm_provider is not None:
            logger.info("Using MemoryAware + LLM decision provider")
            return MemoryAwareDecisionProvider(
                base_provider=llm_provider,
                memory_recall=memory_recall,
            )
        else:
            from agent_runtime.core.think_loop import MockDecisionProvider

            logger.info("Using MemoryAware + Mock decision provider (no LLM config)")
            return MemoryAwareDecisionProvider(
                base_provider=MockDecisionProvider(executor),
                memory_recall=memory_recall,
            )
    except Exception:
        logger.info("Memory-aware decision provider not available")

    # Without memory, use LLM provider directly
    if llm_provider is not None:
        logger.info("Using LLM decision provider (no memory layer)")
        return llm_provider

    logger.info("No LLM configured, falling back to mock decision provider")
    return None


def _create_llm_decision_provider(config: RuntimeConfig) -> Any | None:
    """Create an LLMDecisionProvider from config, or None if LLM is not configured."""
    if config.llm is None:
        return None

    try:
        from agent_runtime.llm.factory import create_provider
        from agent_runtime.core.llm_decide import LLMDecisionProvider

        llm = create_provider(config.llm)
        logger.info(
            "LLM provider created: provider=%s model=%s base_url=%s",
            config.llm.provider.value,
            config.llm.model,
            config.llm.base_url or "(default)",
        )
        return LLMDecisionProvider(llm_provider=llm)
    except Exception:
        logger.warning(
            "Failed to create LLM provider (provider=%s model=%s), will use fallback",
            config.llm.provider.value if config.llm else "none",
            config.llm.model if config.llm else "none",
            exc_info=True,
        )
        return None


def _extract_grpc_address(engine_url: str) -> str:
    """Convert an HTTP REST URL to a gRPC address (host:port).

    ``http://localhost:3000`` → ``localhost:3000``
    ``https://engine.example.com:443`` → ``engine.example.com:443``
    """
    url = engine_url.replace("https://", "").replace("http://", "")
    # Default gRPC port is 50051; strip REST port and use gRPC port
    if ":" in url:
        host = url.split(":")[0]
    else:
        host = url
    return f"{host}:50051"


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
        "--llm-provider", choices=["openai", "anthropic", "ollama", "zhipu"], default=None,
        help="LLM provider (default: ollama; zhipu maps to OpenAI-compatible GLM-5 API)",
    )
    spawn_parser.add_argument(
        "--llm-model", default=None,
        help="LLM model name (default: llama3)",
    )
    spawn_parser.add_argument(
        "--llm-base-url", default=None,
        help="LLM API base URL",
    )
    spawn_parser.add_argument(
        "--no-llm", action="store_true",
        help="Disable LLM and use mock random decisions",
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
                logger.error("Invalid trait value for %r: %r (expected number)", key, val)
                raise SystemExit(1)
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
            skills[name] = 1
    return skills


def build_config_from_args(args: argparse.Namespace) -> RuntimeConfig:
    """Build a RuntimeConfig from CLI arguments, optionally merging with a config file."""
    if args.config is not None:
        config = load_runtime_config(args.config)
    else:
        config = RuntimeConfig()

    # CLI overrides for agent — modify config in-place, no redundant copies
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

    # LLM configuration: CLI args > environment variables > default (Ollama)
    _apply_llm_config(config, args)

    return config


def _apply_llm_config(config: RuntimeConfig, args: argparse.Namespace) -> None:
    """Apply LLM configuration from CLI args, environment variables, or defaults.

    Priority order (highest wins):
      1. --no-llm flag (disables LLM entirely)
      2. CLI flags (--llm-provider, --llm-model, --llm-base-url)
      3. Environment variables (LLM_PROVIDER, LLM_MODEL, LLM_BASE_URL, OLLAMA_BASE_URL)
      4. Existing config file value
      5. Default: Ollama with llama3 (zero-cost mode)
    """
    import os

    # --no-llm explicitly disables LLM
    if getattr(args, "no_llm", False):
        config.llm = None
        logger.info("LLM disabled via --no-llm flag")
        return

    # Determine provider: CLI > env > existing > default(ollama)
    provider_str = (
        args.llm_provider
        or os.environ.get("LLM_PROVIDER")
        or (config.llm.provider.value if config.llm else None)
        or "ollama"
    )

    # Handle provider aliases (zhipu → openai with zhipu base URL)
    zhipu_mode = False
    if provider_str == "zhipu":
        zhipu_mode = True
        provider_str = "openai"

    # Determine model: CLI > env > existing > default(llama3)
    model = (
        args.llm_model
        or os.environ.get("LLM_MODEL")
        or (config.llm.model if config.llm else None)
        or ("glm-5" if zhipu_mode else "llama3")
    )

    # Determine base_url: CLI > env > existing > provider-specific defaults
    base_url = (
        args.llm_base_url
        or os.environ.get("LLM_BASE_URL")
        or (config.llm.base_url if config.llm else None)
    )
    # Zhipu default base URL
    if base_url is None and zhipu_mode:
        base_url = os.environ.get("ZHIPU_BASE_URL", "https://open.bigmodel.cn/api/paas/v4")
    # Ollama-specific env var fallback
    if base_url is None and provider_str == "ollama":
        base_url = os.environ.get("OLLAMA_BASE_URL")

    # Load API key from environment
    api_key = (
        os.environ.get("LLM_API_KEY")
        or os.environ.get(f"{provider_str.upper()}_API_KEY")
        or (config.llm.api_key if config.llm else None)
    )
    # Zhipu-specific API key env var
    if api_key is None and zhipu_mode:
        api_key = os.environ.get("ZHIPU_API_KEY")

    config.llm = LLMConfig(
        provider=ProviderType(provider_str),
        model=model,
        api_key=api_key,
        base_url=base_url,
        timeout=config.llm.timeout if config.llm else 60.0,
        max_tokens=config.llm.max_tokens if config.llm else 4096,
        temperature=config.llm.temperature if config.llm else None,
    )


# ---------------------------------------------------------------------------
# Entry point
# ---------------------------------------------------------------------------


def main() -> None:
    """CLI entry point — parse args and run."""
    # Load .env file early, before any config reading
    load_dotenv()

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
        stats = asyncio.run(run_agent(config))
        print(json.dumps(stats.to_dict(), indent=2))
    else:
        parser.print_help()
        sys.exit(1)


if __name__ == "__main__":
    main()
