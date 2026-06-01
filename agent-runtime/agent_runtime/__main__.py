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
from agent_runtime.env_loader import load_dotenv
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
    """REST-based World Client using httpx.AsyncClient.

    Routes each method to the correct World Engine REST endpoint:

    - gather / move / explore / build / claim_task / submit_task
      → ``POST /api/v1/agents/{agent_id}/action``  (unified action endpoint)
    - propose_deal  → ``submit_action("trade", ...)``
    - teach_skill   → ``submit_action("communicate", ...)``
    - send_message  → ``POST /api/v1/messages``
    - form_org      → ``POST /api/v1/orgs``
    - join_org      → ``POST /api/v1/orgs/{org_id}/join``
    - broadcast     → standalone (no World Engine endpoint)
    """

    def __init__(self, base_url: str, agent_id: str) -> None:
        self._base_url = base_url.rstrip("/")
        self._agent_id = agent_id

    async def _request(self, method: str, path: str, **kwargs: Any) -> dict[str, Any]:
        """Send an HTTP request to the World Engine.

        All errors — including connection failures — are raised so the
        ``ActionExecutor`` retry logic and ThinkLoop error tracking can
        function correctly.  The agent cannot silently "succeed" while
        the World Engine is unreachable.

        If the user wants the agent to run without a World Engine, they
        should not provide ``--world-url``; the ``_NoOpWorldClient``
        (``world_client=None``) handles that case explicitly.
        """
        import httpx

        url = f"{self._base_url}{path}"
        try:
            async with httpx.AsyncClient(timeout=5.0, trust_env=False) as client:
                resp = await client.request(method, url, **kwargs)
                resp.raise_for_status()
                return resp.json()
        except httpx.ConnectError:
            logger.warning("World Engine unreachable at %s", url)
            raise
        except httpx.HTTPStatusError as exc:
            logger.warning(
                "World Engine returned %d for %s %s: %s",
                exc.response.status_code, method, path,
                exc.response.text[:200] if exc.response.text else "(empty)",
            )
            raise
        except Exception:
            logger.warning("World Engine request failed: %s %s", method, path, exc_info=True)
            raise

    async def submit_action(
        self, action: str, params: dict[str, Any]
    ) -> dict[str, Any]:
        """Submit an action via the unified ``POST /api/v1/agents/{id}/action``."""
        return await self._request(
            "POST",
            f"/api/v1/agents/{self._agent_id}/action",
            json={"action": action, "params": params},
        )

    async def send_message(self, payload: dict[str, Any]) -> dict[str, Any]:
        return await self._request("POST", "/api/v1/messages", json=payload)

    async def claim_task(self, task_id: str) -> dict[str, Any]:
        return await self.submit_action("claim_task", {"task_id": task_id})

    async def submit_task(
        self, task_id: str, result: dict[str, Any]
    ) -> dict[str, Any]:
        return await self.submit_action("submit_task", {"task_id": task_id, "result": result})

    async def propose_deal(self, proposal: dict[str, Any]) -> dict[str, Any]:
        return await self.submit_action("trade", proposal)

    async def teach_skill(
        self, target_agent_id: str, skill_name: str, level: int
    ) -> dict[str, Any]:
        return await self.submit_action("communicate", {
            "target_agent_id": target_agent_id,
            "skill_name": skill_name,
            "level": level,
        })

    async def explore(self, parameters: dict[str, Any]) -> dict[str, Any]:
        return await self.submit_action("explore", parameters)

    async def move(self, direction: str) -> dict[str, Any]:
        return await self.submit_action("move", {"direction": direction})

    async def gather(self, resource_type: str) -> dict[str, Any]:
        return await self.submit_action("gather", {"resource_type": resource_type})

    async def build(self, structure_type: str, **kwargs: Any) -> dict[str, Any]:
        return await self.submit_action(
            "build", {"structure_type": structure_type, **kwargs},
        )

    async def broadcast_message(
        self, payload: dict[str, object]
    ) -> dict[str, object]:
        """No World Engine broadcast endpoint — raises to indicate unsupported."""
        logger.warning("broadcast_message: no World Engine endpoint for broadcast")
        raise NotImplementedError("broadcast_message is not supported via REST World Engine")  # type: ignore[return-value]

    async def form_org(self, org_data: dict[str, Any]) -> dict[str, Any]:
        return await self._request("POST", "/api/v1/orgs", json=org_data)

    async def join_org(self, org_id: str, member_data: dict[str, Any]) -> dict[str, Any]:
        return await self._request(
            "POST", f"/api/v1/orgs/{org_id}/join", json=member_data,
        )


# ---------------------------------------------------------------------------
# World Engine connection (gRPC with REST fallback)
# ---------------------------------------------------------------------------


@dataclass
class WorldConnection:
    """Holds the world client and optional perception provider from a gRPC connection."""

    world_client: Any
    perception_provider: Any | None = None
    a2a_client: Any | None = None


async def connect_world_engine(
    grpc_address: str,
    rest_url: str,
    agent_id: str,
) -> WorldConnection:
    """Connect to the World Engine via gRPC, falling back to REST.

    Tries gRPC first (preferred).  If the gRPC server is unreachable,
    creates a REST fallback client so the agent can still run.

    Returns:
        A WorldConnection containing the world client and, when gRPC
        is available, a GRPCPerceptionProvider and the underlying
        A2AClient for streaming.
    """
    # Try gRPC first
    try:
        import asyncio

        from agent_runtime.a2a.client import A2AClient
        from agent_runtime.a2a.config import A2AClientConfig
        from agent_runtime.a2a.perception import GRPCPerceptionProvider
        from agent_runtime.a2a.world_client import GRPCWorldClient

        config = A2AClientConfig(
            server_address=grpc_address,
            agent_id=agent_id,
        )
        client = A2AClient(config)
        await client.connect()

        # Verify the channel is actually reachable before committing to gRPC.
        # Use the native async channel_ready() coroutine instead of
        # grpc.channel_ready_future() which requires a synchronous Channel
        # (grpc.aio.Channel lacks subscribe/unsubscribe, causing AttributeError
        # in _ChannelReadyFuture.__del__).
        try:
            await asyncio.wait_for(
                client._channel.channel_ready(),  # type: ignore[union-attr]
                timeout=2.0,
            )
        except Exception:
            await client.close()
            raise ConnectionError(f"gRPC channel not ready: {grpc_address}")

        world_client = GRPCWorldClient(client)
        perception_provider = GRPCPerceptionProvider(client)
        logger.info(
            "Connected to World Engine via gRPC at %s",
            grpc_address,
            extra={"agent": agent_id, "event": "grpc_connected"},
        )
        return WorldConnection(
            world_client=world_client,
            perception_provider=perception_provider,
            a2a_client=client,
        )
    except ImportError:
        logger.info("gRPC dependencies not available, using REST fallback")
    except Exception:
        logger.warning(
            "Could not connect to World Engine via gRPC at %s — falling back to REST",
            grpc_address,
        )

    # REST fallback
    rest_client = RESTWorldClient(rest_url, agent_id=agent_id)
    logger.info(
        "Using REST fallback for World Engine at %s",
        rest_url,
        extra={"agent": agent_id, "event": "rest_fallback"},
    )
    return WorldConnection(world_client=rest_client)


async def register_agent(
    state: AgentState,
    world_url: str,
    *,
    public_key_b64: str | None = None,
    timeout: float = 5.0,
) -> str | None:
    """Register the agent with the World Engine as an *external* agent.

    Uses the ``POST /api/v1/agents/register`` endpoint which stores the
    agent in the World Engine's ``external_agents`` map — the same map
    that ``POST /api/v1/agents/:id/action`` looks up.

    Returns the World Engine-assigned ``agent_id`` on success, or ``None``
    on failure (in which case the agent runs in standalone mode).
    """
    try:
        import httpx
    except ImportError:
        logger.info("httpx not available, skipping agent registration")
        return None

    url = f"{world_url.rstrip('/')}/api/v1/agents/register"

    # Build payload matching World Engine's RegisterAgentRequest:
    #   { name: String, capabilities: Vec<String>, config: Value }
    capabilities = [s.name for s in state.skills.values()]
    config: dict[str, Any] = {}
    if public_key_b64 is not None:
        config["public_key"] = public_key_b64

    payload: dict[str, Any] = {
        "name": state.name,
        "capabilities": capabilities,
        "config": config,
    }

    logger.info(
        "Registering agent %s (%s) with World Engine at %s",
        state.name, state.id, url,
    )

    try:
        async with httpx.AsyncClient(timeout=timeout, trust_env=False) as client:
            resp = await client.post(url, json=payload)
            if resp.status_code in (200, 201):
                body = resp.json()
                world_agent_id = body.get("agent_id")
                logger.info(
                    "Agent registered successfully (world_id=%s)",
                    world_agent_id,
                    extra={"agent": state.name, "event": "registered"},
                )
                return world_agent_id
            logger.warning(
                "World Engine returned %d: %s",
                resp.status_code,
                resp.text[:200] if resp.text else "(empty)",
            )
            return None
    except httpx.ConnectError:
        logger.warning(
            "World Engine unreachable at %s — running in standalone mode",
            world_url,
        )
        return None
    except Exception:
        logger.exception("Failed to register with World Engine")
        return None


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

    url = f"{world_url.rstrip('/')}/api/v1/agents/{agent_id}"
    logger.info("Deregistering agent %s from World Engine", agent_id)

    try:
        async with httpx.AsyncClient(timeout=timeout, trust_env=False) as client:
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
# Data directory isolation
# ---------------------------------------------------------------------------


def _init_data_dir(data_dir: Path, state: AgentState) -> None:
    """Initialize an agent's isolated data directory.

    Creates the directory (if needed) and seeds the standard data files:
      - ``memory.db``  : SQLite database for agent memory (touched to reserve)
      - ``skills.json``: JSON snapshot of the agent's skills
      - ``trace.db``   : SQLite database for tick traces (touched to reserve)
      - ``agent_state.json``: Serialized AgentState for crash recovery

    Args:
        data_dir: Per-agent data directory (e.g. ``data/alice/``).
        state: The agent's current state (serialized to ``agent_state.json``).
    """
    data_dir.mkdir(parents=True, exist_ok=True)

    # Reserve memory.db (SQLite) — create empty DB if not present
    memory_db = data_dir / "memory.db"
    if not memory_db.exists():
        import sqlite3
        conn = sqlite3.connect(str(memory_db))
        conn.execute(
            "CREATE TABLE IF NOT EXISTS memories ("
            " id INTEGER PRIMARY KEY AUTOINCREMENT,"
            " tick INTEGER NOT NULL,"
            " content TEXT NOT NULL,"
            " created_at REAL NOT NULL"
            ")"
        )
        conn.commit()
        conn.close()

    # Write skills.json
    skills_json = data_dir / "skills.json"
    skills_data = {
        name: skill.model_dump() for name, skill in state.skills.items()
    }
    skills_json.write_text(
        json.dumps(skills_data, indent=2, default=str, ensure_ascii=False)
    )

    # Reserve trace.db (SQLite) — create empty DB if not present
    trace_db = data_dir / "trace.db"
    if not trace_db.exists():
        import sqlite3
        conn = sqlite3.connect(str(trace_db))
        conn.execute(
            "CREATE TABLE IF NOT EXISTS tick_snapshots ("
            " id INTEGER PRIMARY KEY AUTOINCREMENT,"
            " agent_id TEXT NOT NULL,"
            " tick INTEGER NOT NULL,"
            " snapshot_json TEXT NOT NULL,"
            " created_at REAL NOT NULL"
            ")"
        )
        conn.commit()
        conn.close()

    # Persist AgentState for crash recovery
    _save_agent_state_to_dir(data_dir, state)

    logger.info(
        "Data directory initialised  dir=%s"
        "  files=[memory.db, skills.json, trace.db, agent_state.json]",
        data_dir,
        extra={"agent": state.name, "event": "data_dir_initialised"},
    )


def _save_agent_state_to_dir(data_dir: Path, state: AgentState) -> None:
    """Persist an AgentState snapshot to the agent's data directory.

    Writes ``agent_state.json`` with the full serialised state.
    This file can be loaded on restart to resume the agent.
    """
    state_path = data_dir / "agent_state.json"
    state_path.write_text(state.to_json())
    logger.debug(
        "AgentState saved to %s",
        state_path,
        extra={"agent": state.name},
    )


def _load_agent_state_from_dir(data_dir: Path) -> AgentState | None:
    """Load a previously saved AgentState from a data directory.

    Returns None if no saved state exists.
    """
    state_path = data_dir / "agent_state.json"
    if not state_path.exists():
        return None
    try:
        return AgentState.from_json(state_path.read_text())
    except Exception:
        logger.warning("Failed to load AgentState from %s", state_path, exc_info=True)
        return None


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


class _A2AHeartbeatAdapter:
    """Adapts an A2AClient to the HeartbeatProvider protocol."""

    def __init__(self, a2a_client: Any) -> None:
        self._client = a2a_client

    async def heartbeat(self) -> int:
        """Send heartbeat and return server tick."""
        response = await self._client.heartbeat()
        return response.server_time


async def run_agent(config: RuntimeConfig) -> RunStats:
    """Spawn an agent and run its think loop until signalled to stop."""
    state = spawn_agent(config.agent)
    stats = RunStats(
        agent_name=state.name,
        agent_id=str(state.id),
    )

    # Initialize data directory isolation if configured
    data_dir = config.data_dir
    if data_dir is not None:
        _init_data_dir(data_dir, state)

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

    # Start the LLM queue if wired into the decision provider chain.
    # The queue may be on the outer provider or on base_provider (if wrapped
    # by MemoryAwareDecisionProvider).
    llm_queue = _find_llm_queue(decision_provider)
    if llm_queue is not None:
        try:
            await llm_queue.start()
        except Exception:
            logger.warning("Failed to start LLMQueue", exc_info=True)

    # Connect to World Engine (gRPC preferred, REST fallback)
    grpc_address = _extract_grpc_address(config.world.engine_url)
    conn = await connect_world_engine(
        grpc_address=grpc_address,
        rest_url=config.world.engine_url,
        agent_id=str(state.id),
    )
    world_client = conn.world_client
    perception_provider = conn.perception_provider
    a2a_client = conn.a2a_client

    # Protect the entire post-connection lifecycle so that a2a_client is
    # always closed even if start_streaming / register_agent / ThinkLoop
    # construction raises before the think_task try/finally is reached.
    try:
        # Start streaming for perception if gRPC is connected
        if a2a_client is not None:
            try:
                await a2a_client.start_streaming()
                logger.info(
                    "A2A streaming started",
                    extra={"agent": state.name, "event": "streaming_started"},
                )
            except Exception:
                logger.warning("Failed to start A2A streaming, perception will be limited")

            # Start ConsumeMessages stream for Oracle/Bounty delivery
            try:
                from agent_runtime.core.message_queue import MessageQueue

                msg_queue = MessageQueue()
                await a2a_client.start_consuming(msg_queue)
                logger.info(
                    "World message consuming started (Oracle/Bounty)",
                    extra={"agent": state.name, "event": "consuming_started"},
                )
            except Exception:
                logger.warning(
                    "Failed to start world message consuming, Oracle/Bounty delivery disabled"
                )

        # Attempt registration (with public key).
        # If the World Engine returns its own agent_id, update the
        # RESTWorldClient so subsequent action calls use the correct ID.
        world_agent_id = await register_agent(
            state,
            config.world.engine_url,
            public_key_b64=public_key_b64,
        )
        if world_agent_id is not None:
            # Update the REST client to use the World Engine-assigned ID
            if isinstance(world_client, RESTWorldClient):
                world_client._agent_id = world_agent_id
            # Also update stats so deregister uses the right ID
            stats.agent_id = world_agent_id

        # Build heartbeat provider if A2A client is available
        heartbeat_provider: Any | None = None
        if a2a_client is not None and config.think_loop.heartbeat_enabled:
            heartbeat_provider = _A2AHeartbeatAdapter(a2a_client)

        # Build ThinkLoop with all providers wired in via constructor
        # Create social context provider to wire social/cultural modules into decisions
        social_context_provider = _create_social_context_provider(state)

        think_loop = ThinkLoop(
            state=state,
            survival=survival,
            executor=executor,
            config=config.think_loop,
            perception_provider=perception_provider,
            decision_provider=decision_provider,
            world_client=world_client,
            heartbeat_provider=heartbeat_provider,
            social_context_provider=social_context_provider,
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

        think_task = asyncio.create_task(think_loop.run())
        health_task = asyncio.create_task(health_server.start())

        try:
            # Wait for the think loop to finish
            await think_task
        finally:
            # Ensure health server is stopped before deregistering, even if
            # think_task raised an exception.
            await health_server.stop()
            health_task.cancel()
            try:
                await health_task
            except asyncio.CancelledError:
                pass
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

            # Graceful shutdown: persist AgentState to data directory
            if data_dir is not None:
                try:
                    _save_agent_state_to_dir(data_dir, state)
                    logger.info(
                        "AgentState persisted to data directory",
                        extra={"agent": state.name, "event": "state_persisted"},
                    )
                except Exception:
                    logger.warning("Failed to persist AgentState", exc_info=True)

            # Graceful shutdown: stop LLM queue and async decision provider
            llm_queue = _find_llm_queue(decision_provider)
            if llm_queue is not None:
                try:
                    await llm_queue.stop()
                    logger.info(
                        "LLMQueue stopped",
                        extra={"agent": state.name, "event": "llm_queue_stopped"},
                    )
                except Exception:
                    logger.warning("Failed to stop LLMQueue", exc_info=True)
            if decision_provider is not None and hasattr(decision_provider, "stop"):
                try:
                    await decision_provider.stop()
                    logger.info(
                        "AsyncDecisionProvider stopped",
                        extra={"agent": state.name, "event": "async_decide_stopped"},
                    )
                except Exception:
                    logger.warning("Failed to stop AsyncDecisionProvider", exc_info=True)

            # Graceful shutdown: deregister from World Engine
            # Use stats.agent_id which tracks the World Engine-assigned ID
            # after registration (falls back to state.id for standalone).
            await deregister_agent(stats.agent_id, config.world.engine_url)
    finally:
        # Close A2A connection if active — this runs regardless of which
        # stage threw an exception (including before think_task starts).
        if a2a_client is not None:
            try:
                await a2a_client.close()
                logger.info(
                    "A2A client closed",
                    extra={"agent": state.name, "event": "a2a_closed"},
                )
            except Exception:
                logger.warning("Failed to close A2A client", exc_info=True)

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


def _create_mock_decision_provider(preset: str) -> Any | None:
    """Create an AgentMockLLM decision provider from a preset name.

    Returns None if the preset name is unrecognised.
    """
    try:
        import sys
        from pathlib import Path

        # Ensure tests/ is importable so tests.e2e.mocks can be found
        project_root = Path(__file__).resolve().parent.parent.parent
        tests_dir = project_root / "tests"
        tests_str = str(tests_dir)
        if tests_str not in sys.path:
            sys.path.insert(0, tests_str)

        from e2e.mocks.mock_llm import (  # noqa: F811
            hungry_gather_mock,
            social_nearby_mock,
            survival_behaviour_mock,
        )

        factories = {
            "hungry_gather": hungry_gather_mock,
            "social_nearby": social_nearby_mock,
            "survival": survival_behaviour_mock,
        }
        factory = factories.get(preset.lower().strip())
        if factory is not None:
            return factory()
    except Exception:
        logger.warning(
            "Failed to create mock LLM provider for preset=%r",
            preset,
            exc_info=True,
        )
    return None


def _build_decision_provider_with_memory(
    config: RuntimeConfig, executor: ActionExecutor
) -> tuple[Any | None, Any | None]:
    """Build the best available decision provider and return (provider, vector_memory).

    Returns a tuple of (decision_provider, vector_memory) where
    vector_memory may be None if memory deps are unavailable.
    """
    # ── Mock LLM preset (highest priority) ──
    if config.mock_llm_preset:
        provider = _create_mock_decision_provider(config.mock_llm_preset)
        if provider is not None:
            logger.info("Using mock LLM preset: %s", config.mock_llm_preset)
            return provider, None

    # Build the LLM-backed decision provider if config is available
    llm_provider = _create_llm_decision_provider(config)

    # Try to wrap with memory-aware provider
    try:
        from agent_runtime.core.memory_aware_decide import MemoryAwareDecisionProvider
        from agent_runtime.memory.memory_recall import MemoryRecall
        from agent_runtime.memory.vector_memory import VectorMemory

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


def _find_llm_queue(provider: Any) -> Any | None:
    """Walk the decision provider chain to find the LLMQueue, if wired in."""
    seen: set[int] = set()
    current = provider
    while current is not None:
        obj_id = id(current)
        if obj_id in seen:
            break
        seen.add(obj_id)
        q = getattr(current, "_queue", None)
        if q is not None:
            return q
        # Try each known wrapper attribute in turn so we don't skip layers
        for attr in ("base_provider", "_inner"):
            next_obj = getattr(current, attr, None)
            if next_obj is not None:
                current = next_obj
                break
        else:
            # No known attribute found — dead end
            break
    return None


def _create_llm_decision_provider(config: RuntimeConfig) -> Any | None:
    """Create an LLMDecisionProvider from config, or None if LLM is not configured.

    When LLM is configured, the provider is wired through:
      1. LLMQueue — concurrency control with priority scheduling
      2. AsyncDecisionProvider — non-blocking decisions that don't stall ticks

    The LLMQueue is stored on the returned provider (``._queue``) so that
    ``run_agent`` can ``stop()`` it during shutdown.
    """
    if config.llm is None:
        return None

    try:
        from agent_runtime.core.async_decide import AsyncDecisionProvider
        from agent_runtime.core.llm_decide import LLMDecisionProvider
        from agent_runtime.llm.factory import create_provider
        from agent_runtime.llm.queue import LLMQueue

        llm = create_provider(config.llm)
        logger.info(
            "LLM provider created: provider=%s model=%s base_url=%s",
            config.llm.provider.value,
            config.llm.model,
            config.llm.base_url or "(default)",
        )

        # Create the concurrency-controlled queue
        queue = LLMQueue(provider=llm, config=config.llm_queue)

        # Wrap with async decision provider so LLM latency doesn't block ticks.
        # The inner LLMDecisionProvider talks through the queue (not the raw
        # provider) so all LLM calls are routed through priority scheduling
        # and concurrency control.
        inner_provider = LLMDecisionProvider(llm_provider=queue)
        async_provider = AsyncDecisionProvider(inner=inner_provider)

        # Attach the queue so run_agent can stop it during shutdown
        async_provider._queue = queue

        logger.info(
            "Using AsyncDecisionProvider + LLMQueue "
            "(max_concurrency=%d, tick-decoupled from LLM latency)",
            config.llm_queue.max_concurrency,
        )
        return async_provider
    except Exception:
        logger.warning(
            "Failed to create LLM provider (provider=%s model=%s), will use fallback",
            config.llm.provider.value if config.llm else "none",
            config.llm.model if config.llm else "none",
            exc_info=True,
        )
        return None


def _create_social_context_provider(state: AgentState) -> Any:
    """Create a DefaultSocialContextProvider wired to the agent's state.

    The provider bridges social/ modules (trust, cultural diffusion, imitation,
    language emergence) into the decision prompt. It reads the agent's
    personality and values from AgentState to build social context each tick.
    """
    try:
        from agent_runtime.models.personality import PersonalityVector
        from agent_runtime.models.values import ValueWeights
        from agent_runtime.social.provider import (
            AgentProfile,
            DefaultSocialContextProvider,
        )

        agent_id = str(state.id)

        def _profile_source(aid: str) -> AgentProfile | None:
            """Resolve agent profile from AgentState.personality dict."""
            if aid != agent_id:
                return None
            personality = PersonalityVector.from_storage_dict(state.personality)
            values = ValueWeights()
            return AgentProfile(
                personality=personality,
                values=values,
                group_ids=[],
            )

        provider = DefaultSocialContextProvider(profile_source=_profile_source)
        logger.info("SocialContextProvider created for agent %s", state.name)
        return provider
    except Exception:
        logger.debug(
            "SocialContextProvider creation failed (non-fatal), "
            "social context will not be available",
            exc_info=True,
        )
        return None


# ---------------------------------------------------------------------------
# Health check HTTP server
# ---------------------------------------------------------------------------


class HealthCheckServer:
    """Lightweight HTTP health check server using asyncio.

    Exposes:

    - ``GET /health`` — JSON with agent status.
    - ``POST /api/v1/runtime/swap-model`` — Hot-swap the agent's LLM model at runtime.

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
        try:
            if self._server is not None:
                await self._server.serve_forever()
        except asyncio.CancelledError:
            pass  # Graceful shutdown via stop()

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

            # Read remaining headers and collect body
            content_length = 0
            body_buf = b""
            for _ in range(64):
                line = await asyncio.wait_for(reader.readline(), timeout=2.0)
                if line in (b"\r\n", b"\n", b""):
                    break
                # Parse Content-Length header
                line_str = line.decode("ascii", errors="replace").strip().lower()
                if line_str.startswith("content-length:"):
                    try:
                        content_length = int(line_str.split(":", 1)[1].strip())
                    except ValueError:
                        pass
            else:
                # Too many headers — close connection
                writer.write(b"HTTP/1.1 400 Bad Request\r\nConnection: close\r\n\r\n")
                await writer.drain()
                return

            # Read body if Content-Length is set
            if content_length > 0:
                body_buf = await asyncio.wait_for(
                    reader.readexactly(content_length), timeout=5.0
                )

            # Route the request
            parts = request_str.split()
            method = parts[0] if parts else ""
            path = parts[1].split("?")[0] if len(parts) >= 2 else ""

            if method == "GET" and path == "/health":
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

            elif method == "POST" and path == "/api/v1/runtime/swap-model":
                response = self._handle_swap_model(body_buf)

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

    def _handle_swap_model(self, body_buf: bytes) -> str:
        """Handle POST /api/v1/runtime/swap-model.

        Expected JSON body: ``{"agent_id": "...", "provider_id": "...", "model": "..."}``
        """
        from agent_runtime.llm.provider_registry import ModelRegistry

        try:
            payload = json.loads(body_buf) if body_buf else {}
        except json.JSONDecodeError:
            body = json.dumps({"error": "Invalid JSON body"})
            return (
                "HTTP/1.1 400 Bad Request\r\n"
                "Content-Type: application/json\r\n"
                f"Content-Length: {len(body)}\r\n"
                "Connection: close\r\n"
                "\r\n"
                f"{body}"
            )

        agent_id = payload.get("agent_id")
        provider_id = payload.get("provider_id")
        model = payload.get("model")

        if not agent_id or not provider_id or not model:
            body = json.dumps({
                "error": "Missing required fields: agent_id, provider_id, model",
            })
            return (
                "HTTP/1.1 400 Bad Request\r\n"
                "Content-Type: application/json\r\n"
                f"Content-Length: {len(body)}\r\n"
                "Connection: close\r\n"
                "\r\n"
                f"{body}"
            )

        reg = ModelRegistry.instance()
        try:
            reg.hot_swap_model(agent_id, provider_id, model)
        except KeyError as exc:
            body = json.dumps({"error": str(exc)})
            return (
                "HTTP/1.1 404 Not Found\r\n"
                "Content-Type: application/json\r\n"
                f"Content-Length: {len(body)}\r\n"
                "Connection: close\r\n"
                "\r\n"
                f"{body}"
            )

        body = json.dumps({
            "status": "ok",
            "agent_id": agent_id,
            "provider_id": provider_id,
            "model": model,
            "tick": self._think_loop.tick,
        })
        return (
            "HTTP/1.1 200 OK\r\n"
            "Content-Type: application/json\r\n"
            f"Content-Length: {len(body)}\r\n"
            "Connection: close\r\n"
            "\r\n"
            f"{body}"
        )


def _get_health_port(config: RuntimeConfig) -> int:
    """Determine the health check port from env or config default."""
    env_port = os.environ.get("HEALTH_PORT")
    if env_port:
        try:
            return int(env_port)
        except ValueError:
            pass
    return config.health_port


def _extract_grpc_address(engine_url: str) -> str:
    """Convert an HTTP REST URL to a gRPC address (host:port).

    ``http://localhost:3000`` → ``localhost:50051``
    ``https://engine.example.com:443`` → ``engine.example.com:50051``

    The gRPC port can be overridden with the ``GRPC_PORT`` environment variable.
    """
    url = engine_url.replace("https://", "").replace("http://", "")
    # Extract host from REST URL
    if ":" in url:
        host = url.split(":")[0]
    else:
        host = url
    # Use GRPC_PORT env var if set, otherwise default to 50051
    grpc_port = os.environ.get("GRPC_PORT", "50051")
    return f"{host}:{grpc_port}"


# ---------------------------------------------------------------------------
# CLI argument parsing
# ---------------------------------------------------------------------------


def _add_spawn_args(parser: argparse.ArgumentParser) -> None:
    """Add spawn-related arguments to a sub-parser (shared by spawn and pool)."""
    parser.add_argument(
        "--name", default=None, help="Agent name (default: Agent)"
    )
    parser.add_argument(
        "--config", type=Path, default=None,
        help="Path to TOML or YAML config file",
    )
    parser.add_argument(
        "--skills", default=None,
        help="Comma-separated skill names (e.g. coding,trading,research)",
    )
    parser.add_argument(
        "--traits", nargs="*", default=None,
        help="Personality traits as key=value pairs (e.g. curiosity=0.8 caution=0.3)",
    )
    parser.add_argument(
        "--tokens", type=int, default=None,
        help="Initial token balance",
    )
    parser.add_argument(
        "--max-tokens", type=int, default=None,
        help="Maximum token capacity",
    )
    parser.add_argument(
        "--max-ticks", type=int, default=None,
        help="Maximum ticks to run (0 = unlimited)",
    )
    parser.add_argument(
        "--tick-interval", type=float, default=None,
        help="Seconds between ticks",
    )
    parser.add_argument(
        "--world-url", default=None,
        help="World Engine URL (default: http://localhost:3000)",
    )
    parser.add_argument(
        "--llm-provider",
        choices=["openai", "anthropic", "ollama", "zhipu", "google", "azure"],
        default=None,
        help=(
            "LLM provider (default: ollama; "
            "zhipu maps to OpenAI-compatible GLM-5 API; "
            "google/azure via registry)"
        ),
    )
    parser.add_argument(
        "--llm-model", default=None,
        help="LLM model name (default: qwen3:8b)",
    )
    parser.add_argument(
        "--llm-base-url", default=None,
        help="LLM API base URL",
    )
    parser.add_argument(
        "--no-llm", action="store_true",
        help="Disable LLM and use mock random decisions",
    )
    parser.add_argument(
        "--mock-llm", default=None,
        help=(
            "Use preset LLM mock for deterministic decisions. "
            "Options: hungry_gather, social_nearby, survival. "
            "Can also be set via MOCK_LLM_PRESET env var."
        ),
    )
    parser.add_argument(
        "--health-port", type=int, default=None,
        help="Health check HTTP port (default: 9090, env: HEALTH_PORT)",
    )
    parser.add_argument(
        "--data-dir", type=Path, default=None,
        help="Agent data directory for isolated storage (memory.db, skills.json, trace.db)",
    )
    parser.add_argument(
        "--preset", default=None,
        help=(
            "Provider preset name (e.g. zhipu, deepseek, ollama-local, openrouter). "
            "Auto-fills --llm-provider, --llm-base-url, and --llm-model. "
            "Explicit CLI flags override preset values."
        ),
    )


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
    _add_spawn_args(spawn_parser)

    # -- pool --
    pool_parser = sub.add_parser(
        "pool", help="Spawn and manage a pool of agents"
    )
    pool_parser.add_argument(
        "--count", type=int, default=1,
        help="Number of agents to launch with auto-naming (Agent-1..N, default: 1)",
    )
    pool_parser.add_argument(
        "--config-dir", type=Path, default=None,
        help="Directory of .toml agent configs (one file per agent)",
    )
    pool_parser.add_argument(
        "--max-restart", type=int, default=3,
        help="Max restart attempts per crashed agent (default: 3)",
    )
    pool_parser.add_argument(
        "--health-interval", type=float, default=10.0,
        help="Health check interval in seconds (default: 10)",
    )
    pool_parser.add_argument(
        "--api-port", type=int, default=9090,
        help="Pool API HTTP port (default: 9090)",
    )
    # Reuse all spawn args
    _add_spawn_args(pool_parser)

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


def _apply_preset_defaults(args: argparse.Namespace) -> None:
    """Fill in LLM args from a provider preset if --preset is given.

    Only sets values that the user did *not* explicitly pass on the command
    line, so ``--llm-model foo --preset zhipu`` keeps the user's model.
    """
    preset_name = getattr(args, "preset", None)
    if not preset_name:
        return

    from agent_runtime.presets import get_provider_preset, list_models_for_provider

    try:
        provider_cfg = get_provider_preset(preset_name)
    except ValueError as exc:
        logger.error("%s", exc)
        raise SystemExit(1)

    # Map preset protocol to CLI --llm-provider choices
    protocol = provider_cfg.get("protocol", "openai")
    # zhipu is a special alias in the CLI
    if preset_name == "zhipu":
        protocol = "zhipu"

    if args.llm_provider is None:
        args.llm_provider = protocol
    if args.llm_base_url is None:
        args.llm_base_url = provider_cfg.get("base_url")
    if args.llm_model is None:
        # Pick the first model listed for this provider
        models = list_models_for_provider(preset_name)
        if models:
            args.llm_model = models[0]["id"]

    logger.info(
        "Applied preset %r: provider=%s base_url=%s model=%s",
        preset_name,
        args.llm_provider,
        args.llm_base_url,
        args.llm_model,
    )


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

    # Data directory for agent isolation
    data_dir = getattr(args, "data_dir", None)
    if data_dir is None:
        # Fall back to AGENT_DATA_DIR env var (set by pool.py subprocess manager)
        env_data_dir = os.environ.get("AGENT_DATA_DIR")
        if env_data_dir:
            data_dir = Path(env_data_dir)
    if data_dir is not None:
        config.data_dir = Path(data_dir)
        config.data_dir.mkdir(parents=True, exist_ok=True)

    # Apply --preset as defaults (explicit CLI flags override preset values)
    _apply_preset_defaults(args)

    # LLM configuration: CLI args > preset > environment variables > default (Ollama)
    _apply_llm_config(config, args)

    # Mock LLM preset: --mock-llm > MOCK_LLM_PRESET env var
    mock_llm_preset = getattr(args, "mock_llm", None) or os.environ.get("MOCK_LLM_PRESET")
    if mock_llm_preset:
        config.mock_llm_preset = mock_llm_preset
        # Disable real LLM when using mock
        config.llm = None
        logger.info("Using mock LLM preset: %s", mock_llm_preset)

    return config


def _apply_llm_config(config: RuntimeConfig, args: argparse.Namespace) -> None:
    """Apply LLM configuration from CLI args, environment variables, or defaults.

    Priority order (highest wins):
      1. --no-llm flag (disables LLM entirely)
      2. CLI flags (--llm-provider, --llm-model, --llm-base-url)
      3. Environment variables (LLM_PROVIDER, LLM_MODEL, LLM_BASE_URL, OLLAMA_BASE_URL)
      4. Existing config file value
      5. Default: Ollama with qwen3:8b (zero-cost mode)
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

    # Map new protocols to their ProviderType equivalent for backward compat.
    # google/azure use the OpenAI-compatible transport layer for now.
    _new_protocol_map: dict[str, str] = {
        "google": "openai",
        "azure": "openai",
    }
    if provider_str in _new_protocol_map:
        provider_str = _new_protocol_map[provider_str]

    # Determine model: CLI > env > existing > default(qwen3:8b)
    model = (
        args.llm_model
        or os.environ.get("LLM_MODEL")
        or (config.llm.model if config.llm else None)
        or ("glm-5" if zhipu_mode else "qwen3:8b")
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
# Agent Pool — spawn and manage N agents in subprocesses
# ---------------------------------------------------------------------------


@dataclass
class PoolAgentInfo:
    """Tracks a single agent in the pool."""

    index: int
    name: str
    process: asyncio.subprocess.Process | None = None
    restarts: int = 0
    status: str = "pending"  # pending | running | crashed | stopped


class AgentPool:
    """Manages a pool of agent subprocesses with health monitoring.

    Each agent runs as ``python -m agent_runtime spawn --name <name> ...``
    in its own subprocess, inheriting the pool's LLM/world configuration.
    Crashed agents are automatically restarted up to ``max_restart`` times.
    """

    def __init__(
        self,
        *,
        count: int = 1,
        config_dir: Path | None = None,
        max_restart: int = 3,
        health_interval: float = 10.0,
        api_port: int = 9090,
        spawn_args: list[str] | None = None,
    ) -> None:
        self._count = count
        self._config_dir = config_dir
        self._max_restart = max_restart
        self._health_interval = health_interval
        self._api_port = api_port
        self._spawn_args = spawn_args or []
        self._agents: list[PoolAgentInfo] = []
        self._shutdown = asyncio.Event()
        self._api_server: asyncio.Server | None = None

    async def run(self) -> dict[str, Any]:
        """Start all agents and monitor until shutdown."""
        start_time = time.monotonic()

        # Build agent list from --config-dir or --count
        if self._config_dir is not None:
            self._agents = self._build_from_config_dir()
        else:
            self._agents = self._build_from_count()

        if not self._agents:
            logger.warning("No agents to start in pool")
            return {"agents": [], "duration_s": 0.0}

        logger.info(
            "Starting agent pool with %d agents",
            len(self._agents),
            extra={"event": "pool_start"},
        )

        # Launch all agents
        for agent in self._agents:
            await self._start_agent(agent)

        # Start the Pool API server
        api_task = asyncio.create_task(self._start_api_server())

        # Health monitor loop
        try:
            while not self._shutdown.is_set():
                try:
                    await asyncio.wait_for(
                        self._shutdown.wait(), timeout=self._health_interval
                    )
                except asyncio.TimeoutError:
                    pass
                # Check health and restart crashed agents
                await self._health_check()
                # Auto-shutdown when all agents are stopped/crashed
                if all(a.status in ("stopped", "crashed") for a in self._agents):
                    logger.info("All agents finished — pool shutting down")
                    break
        except asyncio.CancelledError:
            pass
        finally:
            # Stop all agents
            await self._stop_all()
            if self._api_server is not None:
                self._api_server.close()
                await self._api_server.wait_closed()
            api_task.cancel()
            try:
                await api_task
            except (asyncio.CancelledError, Exception):
                pass

        duration = time.monotonic() - start_time
        result = {
            "agents": [
                {
                    "name": a.name,
                    "status": a.status,
                    "restarts": a.restarts,
                }
                for a in self._agents
            ],
            "duration_s": round(duration, 2),
        }
        logger.info("Pool stopped: %s", result, extra={"event": "pool_stop"})
        return result

    def request_shutdown(self) -> None:
        """Signal the pool to shut down gracefully."""
        self._shutdown.set()

    def _build_from_count(self) -> list[PoolAgentInfo]:
        """Build agent list from --count with auto-naming (Agent-1..N)."""
        return [
            PoolAgentInfo(index=i, name=f"Agent-{i + 1}")
            for i in range(self._count)
        ]

    def _build_from_config_dir(self) -> list[PoolAgentInfo]:
        """Build agent list from .toml files in --config-dir."""
        agents: list[PoolAgentInfo] = []
        if not self._config_dir or not self._config_dir.is_dir():
            logger.warning("Config directory not found: %s", self._config_dir)
            return agents
        for i, path in enumerate(sorted(self._config_dir.glob("*.toml"))):
            agents.append(
                PoolAgentInfo(
                    index=i,
                    name=path.stem,
                )
            )
        return agents

    async def _start_agent(self, agent: PoolAgentInfo) -> None:
        """Start a single agent subprocess."""
        cmd = [sys.executable, "-m", "agent_runtime", "spawn"]

        # If using config-dir, pass the specific config file
        if self._config_dir is not None:
            config_file = self._config_dir / f"{agent.name}.toml"
            if config_file.exists():
                cmd.extend(["--config", str(config_file)])

        # Always set the name (overrides config file name)
        cmd.extend(["--name", agent.name])

        # Per-agent data directory isolation
        data_dir = Path("data") / agent.name
        data_dir.mkdir(parents=True, exist_ok=True)
        cmd.extend(["--data-dir", str(data_dir)])

        # Append shared spawn args (world-url, llm settings, etc.)
        cmd.extend(self._spawn_args)

        try:
            process = await asyncio.create_subprocess_exec(
                *cmd,
                stdout=asyncio.subprocess.PIPE,
                stderr=asyncio.subprocess.PIPE,
            )
            agent.process = process
            agent.status = "running"
            logger.info(
                "Started agent %s (pid=%d)",
                agent.name,
                process.pid,
                extra={"agent": agent.name, "event": "pool_agent_started"},
            )
        except Exception:
            agent.status = "crashed"
            logger.exception(
                "Failed to start agent %s",
                agent.name,
                extra={"agent": agent.name, "event": "pool_agent_start_failed"},
            )

    async def _stop_agent(self, agent: PoolAgentInfo) -> None:
        """Stop a single agent subprocess gracefully."""
        if agent.process is None or agent.process.returncode is not None:
            return
        try:
            agent.process.terminate()
            try:
                await asyncio.wait_for(agent.process.wait(), timeout=5.0)
            except asyncio.TimeoutError:
                agent.process.kill()
                await agent.process.wait()
            agent.status = "stopped"
            logger.info(
                "Stopped agent %s",
                agent.name,
                extra={"agent": agent.name, "event": "pool_agent_stopped"},
            )
        except Exception:
            logger.warning(
                "Error stopping agent %s",
                agent.name,
                exc_info=True,
                extra={"agent": agent.name},
            )

    async def _stop_all(self) -> None:
        """Stop all running agents."""
        for agent in self._agents:
            await self._stop_agent(agent)

    async def _health_check(self) -> None:
        """Check agent health and restart crashed agents."""
        for agent in self._agents:
            if agent.process is None:
                continue
            if agent.process.returncode is not None:
                # Clean exit — mark stopped, don't restart
                if agent.process.returncode == 0:
                    agent.status = "stopped"
                    continue
                # Non-zero exit — crashed, attempt restart
                if agent.restarts < self._max_restart:
                    logger.info(
                        "Agent %s crashed (exit=%d), restarting (%d/%d)",
                        agent.name,
                        agent.process.returncode,
                        agent.restarts + 1,
                        self._max_restart,
                        extra={
                            "agent": agent.name,
                            "event": "pool_agent_restart",
                        },
                    )
                    agent.restarts += 1
                    await self._start_agent(agent)
                else:
                    agent.status = "crashed"
                    logger.warning(
                        "Agent %s exceeded max restarts (%d)",
                        agent.name,
                        self._max_restart,
                        extra={
                            "agent": agent.name,
                            "event": "pool_agent_max_restarts",
                        },
                    )

    def _build_spawn_args(self) -> list[str]:
        """Return the shared spawn args list."""
        return list(self._spawn_args)

    # -- Pool API server --

    async def _start_api_server(self) -> None:
        """Start a lightweight HTTP server for pool status."""
        try:
            self._api_server = await asyncio.start_server(
                self._handle_api_request,
                host="0.0.0.0",
                port=self._api_port,
            )
        except OSError:
            logger.warning(
                "Pool API: port %d unavailable, skipping", self._api_port
            )
            return
        logger.info(
            "Pool API listening on 0.0.0.0:%d",
            self._api_port,
            extra={"event": "pool_api_started", "port": self._api_port},
        )
        if self._api_server is not None:
            try:
                await self._api_server.serve_forever()
            except asyncio.CancelledError:
                pass

    async def _handle_api_request(
        self,
        reader: asyncio.StreamReader,
        writer: asyncio.StreamWriter,
    ) -> None:
        """Handle Pool API HTTP requests."""
        try:
            request_line = await asyncio.wait_for(reader.readline(), timeout=5.0)
            request_str = request_line.decode("ascii", errors="replace").strip()
            # Drain headers
            for _ in range(64):
                line = await asyncio.wait_for(reader.readline(), timeout=2.0)
                if line in (b"\r\n", b"\n", b""):
                    break

            parts = request_str.split()
            path = parts[1].split("?")[0] if len(parts) >= 2 else ""

            if parts and parts[0] == "GET" and path == "/health":
                body = json.dumps(self._pool_status())
            elif parts and parts[0] == "GET" and path == "/agents":
                body = json.dumps(self._agent_list())
            else:
                writer.write(b"HTTP/1.1 404 Not Found\r\nConnection: close\r\n\r\n")
                await writer.drain()
                return

            response = (
                "HTTP/1.1 200 OK\r\n"
                "Content-Type: application/json\r\n"
                f"Content-Length: {len(body)}\r\n"
                "Connection: close\r\n"
                "\r\n"
                f"{body}"
            )
            writer.write(response.encode("utf-8"))
            await writer.drain()
        except Exception:
            logger.debug("Pool API request error", exc_info=True)
        finally:
            writer.close()
            try:
                await writer.wait_closed()
            except Exception:
                pass

    def _pool_status(self) -> dict[str, Any]:
        """Return pool health summary."""
        running = sum(1 for a in self._agents if a.status == "running")
        return {
            "status": "running" if not self._shutdown.is_set() else "stopping",
            "total": len(self._agents),
            "running": running,
            "crashed": sum(1 for a in self._agents if a.status == "crashed"),
        }

    def _agent_list(self) -> list[dict[str, Any]]:
        """Return per-agent status list."""
        return [
            {
                "name": a.name,
                "status": a.status,
                "restarts": a.restarts,
                "pid": a.process.pid if a.process and a.process.returncode is None else None,
            }
            for a in self._agents
        ]


def _build_pool_spawn_args(args: argparse.Namespace) -> list[str]:
    """Extract the shared spawn flags from parsed pool args into a CLI list.

    These are passed as extra arguments to each ``spawn`` subprocess so the
    pool agents share the same LLM / world / tick configuration.
    """
    parts: list[str] = []
    if getattr(args, "world_url", None):
        parts.extend(["--world-url", args.world_url])
    if getattr(args, "llm_provider", None):
        parts.extend(["--llm-provider", args.llm_provider])
    if getattr(args, "llm_model", None):
        parts.extend(["--llm-model", args.llm_model])
    if getattr(args, "llm_base_url", None):
        parts.extend(["--llm-base-url", args.llm_base_url])
    if getattr(args, "no_llm", False):
        parts.append("--no-llm")
    if getattr(args, "mock_llm", None):
        parts.extend(["--mock-llm", args.mock_llm])
    if getattr(args, "skills", None):
        parts.extend(["--skills", args.skills])
    if getattr(args, "traits", None):
        parts.extend(["--traits", *args.traits])
    if getattr(args, "tokens", None) is not None:
        parts.extend(["--tokens", str(args.tokens)])
    if getattr(args, "max_tokens", None) is not None:
        parts.extend(["--max-tokens", str(args.max_tokens)])
    if getattr(args, "max_ticks", None) is not None:
        parts.extend(["--max-ticks", str(args.max_ticks)])
    if getattr(args, "tick_interval", None) is not None:
        parts.extend(["--tick-interval", str(args.tick_interval)])
    if getattr(args, "health_port", None) is not None:
        parts.extend(["--health-port", str(args.health_port)])
    if getattr(args, "data_dir", None) is not None:
        parts.extend(["--data-dir", str(args.data_dir)])
    if getattr(args, "preset", None):
        parts.extend(["--preset", args.preset])
    return parts


async def run_pool(args: argparse.Namespace) -> dict[str, Any]:
    """Run an AgentPool from parsed CLI args."""
    spawn_args = _build_pool_spawn_args(args)

    pool = AgentPool(
        count=getattr(args, "count", 1),
        config_dir=getattr(args, "config_dir", None),
        max_restart=getattr(args, "max_restart", 3),
        health_interval=getattr(args, "health_interval", 10.0),
        api_port=getattr(args, "api_port", 9090),
        spawn_args=spawn_args,
    )

    # Graceful shutdown on SIGINT
    loop = asyncio.get_running_loop()

    def _signal_handler() -> None:
        logger.info("Pool received SIGINT — shutting down", extra={"event": "pool_shutdown_signal"})
        pool.request_shutdown()

    loop.add_signal_handler(signal.SIGINT, _signal_handler)

    try:
        result = await pool.run()
    finally:
        try:
            loop.remove_signal_handler(signal.SIGINT)
        except (ValueError, OSError):
            pass

    return result


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
    elif args.command == "pool":
        result = asyncio.run(run_pool(args))
        print(json.dumps(result, indent=2))
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
