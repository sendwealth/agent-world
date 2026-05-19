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
    public_key_b64: str | None = None,
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

    # Attach Ed25519 public key if available
    if public_key_b64 is not None:
        payload["public_key"] = public_key_b64

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


async def deregister_agent(
    agent_id: str,
    world_url: str,
    *,
    timeout: float = 5.0,
) -> bool:
    """Deregister the agent from the World Engine REST API.

    Non-fatal: errors are logged but do not propagate.
    """
    try:
        import httpx
    except ImportError:
        logger.info("httpx not available, skipping agent deregistration")
        return False

    url = f"{world_url.rstrip('/')}/agents/{agent_id}"
    logger.info("Deregistering agent %s from World Engine", agent_id)

    try:
        async with httpx.AsyncClient(timeout=timeout) as client:
            resp = await client.delete(url)
            if resp.status_code in (200, 204):
                logger.info("Agent deregistered successfully")
                return True
            logger.warning(
                "World Engine returned %d on deregister: %s",
                resp.status_code,
                resp.text[:200] if resp.text else "(empty)",
            )
            return False
    except httpx.ConnectError:
        logger.warning(
            "World Engine unreachable during deregister — already standalone",
        )
        return False
    except Exception:
        logger.exception("Failed to deregister from World Engine")
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
    shutdown_reason: str = ""

    @property
    def duration_s(self) -> float:
        return self.end_time - self.start_time

    def to_dict(self) -> dict[str, Any]:
        d: dict[str, Any] = {
            "agent_name": self.agent_name,
            "agent_id": self.agent_id,
            "ticks": self.ticks,
            "errors": self.errors,
            "duration_s": round(self.duration_s, 2),
        }
        if self.shutdown_reason:
            d["shutdown_reason"] = self.shutdown_reason
        return d


async def run_agent(config: RuntimeConfig) -> RunStats:
    """Spawn an agent and run its think loop until signalled to stop."""
    state = spawn_agent(config.agent)
    stats = RunStats(
        agent_name=state.name,
        agent_id=str(state.id),
    )

    # Generate Ed25519 key pair for this agent
    public_key_b64: str | None = None
    try:
        from agent_runtime.crypto.keys import generate_key_pair

        key_pair = generate_key_pair()
        public_key_b64 = key_pair.public_key_b64()
        logger.info(
            "Generated Ed25519 key pair for agent %s (pub=%s...)",
            state.name,
            public_key_b64[:12],
            extra={"agent": state.name, "event": "key_generated"},
        )
    except ImportError:
        logger.info("crypto.keys not available, skipping key generation")
    except Exception:
        logger.warning("Failed to generate key pair", exc_info=True)

    # Set up core components
    survival = SurvivalInstinct()
    executor = ActionExecutor()

    # Build decision provider (memory-aware if vector memory available)
    decision_provider, vector_memory = _build_decision_provider_with_memory(config, executor)

    # Connect to World Engine (gRPC preferred, REST fallback)
    grpc_address = _extract_grpc_address(config.world.engine_url)
    world_client = await connect_world_engine(
        grpc_address=grpc_address,
        rest_url=config.world.engine_url,
        agent_id=str(state.id),
    )

    # Attempt registration (with public key)
    await register_agent(
        state,
        config.world.engine_url,
        public_key_b64=public_key_b64,
    )

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

    # Start health check HTTP server
    health_port = _get_health_port(config)
    health_server = HealthCheckServer(
        agent_name=state.name,
        think_loop=think_loop,
        port=health_port,
    )

    logger.info(
        "Starting agent runtime",
        extra={
            "agent": state.name,
            "event": "runtime_start",
            "config": {
                "tick_interval": config.think_loop.tick_interval,
                "max_ticks": config.think_loop.max_ticks or "unlimited",
                "world_url": config.world.engine_url,
                "health_port": health_port,
            },
        },
    )

    stats.start_time = time.monotonic()

    try:
        # Run think loop and health server concurrently
        think_task = asyncio.create_task(think_loop.run())
        health_task = asyncio.create_task(health_server.start())

        # Wait for the think loop to finish
        await think_task

        # Stop health server
        await health_server.stop()
        await health_task
    finally:
        stats.end_time = time.monotonic()
        stats.ticks = think_loop.tick
        stats.errors = think_loop.total_errors
        stats.shutdown_reason = "sigint" if shutdown_event.is_set() else "completed"
        try:
            loop.remove_signal_handler(signal.SIGINT)
        except (ValueError, OSError) as exc:
            logger.warning("Failed to remove signal handler: %s", exc)

        # Graceful shutdown: save memory if available
        if vector_memory is not None:
            try:
                vector_memory.close()
                logger.info(
                    "Vector memory closed (persisted to disk)",
                    extra={"agent": state.name, "event": "memory_saved"},
                )
            except Exception:
                logger.warning("Failed to close vector memory", exc_info=True)

        # Graceful shutdown: deregister from World Engine
        await deregister_agent(str(state.id), config.world.engine_url)

    logger.info(
        "Agent runtime stopped",
        extra={
            "agent": state.name,
            "event": "runtime_stop",
            "ticks": stats.ticks,
            "errors": stats.errors,
            "duration_s": round(stats.duration_s, 2),
            "shutdown_reason": stats.shutdown_reason,
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
    provider, _ = _build_decision_provider_with_memory(config, executor)
    return provider


def _build_decision_provider_with_memory(
    config: RuntimeConfig, executor: ActionExecutor
) -> tuple[Any | None, Any | None]:
    """Build the best available decision provider and return (provider, vector_memory).

    Returns a tuple of (decision_provider, vector_memory) where
    vector_memory may be None if memory deps are unavailable.
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
            return (
                MemoryAwareDecisionProvider(
                    base_provider=llm_provider,
                    memory_recall=memory_recall,
                ),
                vector_memory,
            )
        else:
            from agent_runtime.core.think_loop import MockDecisionProvider

            logger.info("Using MemoryAware + Mock decision provider (no LLM config)")
            return (
                MemoryAwareDecisionProvider(
                    base_provider=MockDecisionProvider(executor),
                    memory_recall=memory_recall,
                ),
                vector_memory,
            )
    except Exception:
        logger.info("Memory-aware decision provider not available")

    # Without memory, use LLM provider directly
    if llm_provider is not None:
        logger.info("Using LLM decision provider (no memory layer)")
        return llm_provider, None

    logger.info("No LLM configured, falling back to mock decision provider")
    return None, None


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


# ---------------------------------------------------------------------------
# Health check HTTP server
# ---------------------------------------------------------------------------


class HealthCheckServer:
    """Lightweight HTTP health check server using asyncio.

    Exposes ``GET /health`` returning JSON with agent status.
    Runs alongside the ThinkLoop.
    """

    def __init__(
        self,
        agent_name: str,
        think_loop: ThinkLoop,
        port: int = 9090,
    ) -> None:
        self._agent_name = agent_name
        self._think_loop = think_loop
        self._port = port
        self._start_time = time.monotonic()
        self._server: asyncio.Server | None = None

    async def start(self) -> None:
        """Start the health check HTTP server."""
        try:
            self._server = await asyncio.start_server(
                self._handle_request,
                host="0.0.0.0",
                port=self._port,
            )
        except OSError:
            logger.warning(
                "Health check server: port %d unavailable, skipping",
                self._port,
            )
            return
        logger.info(
            "Health check server listening on 0.0.0.0:%d",
            self._port,
            extra={"event": "health_server_started", "port": self._port},
        )
        # Keep running until stop() closes the server
        if self._server is not None:
            await self._server.serve_forever()

    async def stop(self) -> None:
        """Stop the health check server."""
        if self._server is not None:
            self._server.close()
            await self._server.wait_closed()
            logger.info("Health check server stopped")

    async def _handle_request(
        self,
        reader: asyncio.StreamReader,
        writer: asyncio.StreamWriter,
    ) -> None:
        """Handle a single HTTP request."""
        try:
            # Read the request line (we only care about the first line)
            request_line = await asyncio.wait_for(reader.readline(), timeout=5.0)
            request_str = request_line.decode("ascii", errors="replace").strip()

            # Drain remaining headers
            while True:
                line = await asyncio.wait_for(reader.readline(), timeout=2.0)
                if line in (b"\r\n", b"\n", b""):
                    break

            # Only respond to GET /health
            if request_str.startswith("GET /health"):
                uptime = time.monotonic() - self._start_time
                body = json.dumps({
                    "status": "running" if self._think_loop.running else "stopped",
                    "agent": self._agent_name,
                    "tick": self._think_loop.tick,
                    "uptime_s": round(uptime, 1),
                })
                response = (
                    "HTTP/1.1 200 OK\r\n"
                    "Content-Type: application/json\r\n"
                    f"Content-Length: {len(body)}\r\n"
                    "Connection: close\r\n"
                    "\r\n"
                    f"{body}"
                )
            else:
                response = "HTTP/1.1 404 Not Found\r\nConnection: close\r\n\r\n"

            writer.write(response.encode("ascii"))
            await writer.drain()
        except Exception:
            logger.debug("Health check request error", exc_info=True)
        finally:
            writer.close()
            try:
                await writer.wait_closed()
            except Exception:
                pass


def _get_health_port(config: RuntimeConfig) -> int:
    """Determine the health check port from env or config default."""
    import os

    env_port = os.environ.get("HEALTH_PORT")
    if env_port:
        try:
            return int(env_port)
        except ValueError:
            pass
    return config.health_port


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

    # Top-level --world shortcut (alias for spawn --world-url)
    parser.add_argument(
        "--world", default=None, dest="world",
        help="World Engine URL — shorthand that implies 'spawn' (e.g. --world http://localhost:8080)",
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
    spawn_parser.add_argument(
        "--health-port", type=int, default=None,
        help="Health check HTTP port (default: 9090, env: HEALTH_PORT)",
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

    # CLI overrides for world -- support both --world-url and top-level --world
    world_url = args.world_url or getattr(args, "world", None)
    if world_url is not None:
        config.world.engine_url = world_url

    # Health check port
    if getattr(args, "health_port", None) is not None:
        config.health_port = args.health_port  # type: ignore[attr-defined]

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

    # Auto-default to 'spawn' when no subcommand but --world is given
    if args.command is None:
        if _has_world_arg(sys.argv[1:]):
            # Rewrite --world to --world-url and inject 'spawn' subcommand
            rewritten = _rewrite_world_to_world_url(sys.argv[1:])
            args = parser.parse_args(["spawn"] + rewritten)
        else:
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


def _has_world_arg(argv: list[str]) -> bool:
    """Check if --world or --world-url is present in the argument list."""
    for arg in argv:
        if arg in ("--world", "--world-url"):
            return True
        if arg.startswith("--world=") or arg.startswith("--world-url="):
            return True
    return False


def _rewrite_world_to_world_url(argv: list[str]) -> list[str]:
    """Replace top-level --world with spawn's --world-url for re-parsing."""
    result: list[str] = []
    i = 0
    while i < len(argv):
        if argv[i] == "--world":
            result.append("--world-url")
            i += 1
            if i < len(argv):
                result.append(argv[i])
                i += 1
        elif argv[i].startswith("--world="):
            result.append("--world-url=" + argv[i].split("=", 1)[1])
            i += 1
        else:
            result.append(argv[i])
            i += 1
    return result


if __name__ == "__main__":
    main()
