"""Agent lifecycle, runtime, and spawn helpers.

Provides ``spawn_agent``, ``run_agent``, data-directory helpers,
``RunStats``, ``_A2AHeartbeatAdapter``, and all decision-provider helpers.
"""

from __future__ import annotations

import asyncio
import json
import logging
import signal
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from agent_runtime.a2a.rest_world_client import RESTWorldClient
from agent_runtime.config import AgentSpawnConfig, RuntimeConfig
from agent_runtime.core.act import ActionExecutor
from agent_runtime.core.llm_decide import LLMDecisionProvider
from agent_runtime.core.think_loop import ThinkLoop
from agent_runtime.llm.factory import create_provider
from agent_runtime.llm.queue import LLMQueue
from agent_runtime.models.agent_state import AgentState
from agent_runtime.survival.instinct import SurvivalInstinct

logger = logging.getLogger(__name__)


def spawn_agent(config: AgentSpawnConfig) -> AgentState:
    """Create an AgentState from spawn configuration.

    Merges extended identity data (backstory, alignment, communication_style,
    personality vector, values, preferences) into the AgentState personality
    dict so it can be consumed by the prompt and decision layers.
    """
    # Build enriched personality dict: start with legacy traits, then merge
    # structured sections under namespaced keys.
    personality: dict[str, Any] = dict(config.traits)

    # Identity
    identity = config.identity
    if identity.display_name:
        personality["display_name"] = identity.display_name
    if identity.bio:
        personality["bio"] = identity.bio
    if identity.backstory:
        personality["backstory"] = identity.backstory
    if identity.alignment:
        personality["alignment"] = identity.alignment
    if identity.archetype:
        personality["archetype"] = identity.archetype
    if identity.mbti:
        personality["mbti"] = identity.mbti

    # Personality vector (Big Five + survival)
    personality["big_five"] = {
        "openness": config.personality.openness,
        "conscientiousness": config.personality.conscientiousness,
        "extraversion": config.personality.extraversion,
        "agreeableness": config.personality.agreeableness,
        "neuroticism": config.personality.neuroticism,
        "risk_tolerance": config.personality.risk_tolerance,
        "social_orientation": config.personality.social_orientation,
        "greed": config.personality.greed,
    }

    # Values
    personality["values"] = {
        "survival": config.values.survival,
        "knowledge": config.values.knowledge,
        "wealth": config.values.wealth,
        "social": config.values.social,
        "freedom": config.values.freedom,
        "power": config.values.power,
    }

    # Preferences
    prefs = config.preferences
    if prefs.preferred_actions:
        personality["preferred_actions"] = prefs.preferred_actions
    if prefs.avoided_actions:
        personality["avoided_actions"] = prefs.avoided_actions
    if prefs.social_style:
        personality["social_style"] = prefs.social_style
    if prefs.communication_style:
        personality["communication_style"] = prefs.communication_style

    # Questions
    if config.questions:
        personality["questions"] = config.questions

    from agent_runtime.models.enums import AgentPhase
    from agent_runtime.models.skill import Skill

    state = AgentState(
        name=config.name,
        tokens=config.tokens,
        max_tokens=config.max_tokens,
        money=config.money,
        health=config.health,
        personality=personality,
        phase=AgentPhase.ADULT,  # External agents skip BIRTH/CHILDHOOD
    )

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
# Data directory isolation
# ---------------------------------------------------------------------------


def _init_data_dir(data_dir: Path, state: AgentState) -> None:
    """Initialize an agent's isolated data directory."""
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
    skills_data = {name: skill.model_dump() for name, skill in state.skills.items()}
    skills_json.write_text(json.dumps(skills_data, indent=2, default=str, ensure_ascii=False))

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
    """Persist an AgentState snapshot to the agent's data directory."""
    state_path = data_dir / "agent_state.json"
    state_path.write_text(state.to_json())
    logger.debug(
        "AgentState saved to %s",
        state_path,
        extra={"agent": state.name},
    )


def _load_agent_state_from_dir(data_dir: Path) -> AgentState | None:
    """Load a previously saved AgentState from a data directory."""
    state_path = data_dir / "agent_state.json"
    if not state_path.exists():
        return None
    try:
        return AgentState.from_json(state_path.read_text())
    except Exception:
        logger.warning("Failed to load AgentState from %s", state_path, exc_info=True)
        return None


# ---------------------------------------------------------------------------
# RunStats
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


# ---------------------------------------------------------------------------
# Heartbeat adapter
# ---------------------------------------------------------------------------


class _A2AHeartbeatAdapter:
    """Adapts an A2AClient to the HeartbeatProvider protocol."""

    def __init__(self, a2a_client: Any) -> None:
        self._client = a2a_client

    async def heartbeat(self) -> int:
        """Send heartbeat and return server tick."""
        response = await self._client.heartbeat()
        return response.server_time


# ---------------------------------------------------------------------------
# Decision provider helpers
# ---------------------------------------------------------------------------


def _build_decision_provider(config: RuntimeConfig, executor: ActionExecutor) -> Any | None:
    """Build the best available decision provider."""
    provider, _ = _build_decision_provider_with_memory(config, executor)
    return provider


def _create_mock_decision_provider(preset: str) -> Any | None:
    """Create an AgentMockLLM decision provider from a preset name."""
    try:
        import sys

        # Ensure tests/ is importable so tests.e2e.mocks can be found
        project_root = Path(__file__).resolve().parent.parent.parent
        tests_dir = project_root / "tests"
        tests_str = str(tests_dir)
        if tests_str not in sys.path:
            sys.path.insert(0, tests_str)

        from e2e.mocks.mock_llm import (
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
    """Build the best available decision provider and return (provider, vector_memory)."""
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
        # Try each known wrapper attribute in turn
        for attr in ("base_provider", "_inner"):
            next_obj = getattr(current, attr, None)
            if next_obj is not None:
                current = next_obj
                break
        else:
            break
    return None


def _create_llm_decision_provider(config: RuntimeConfig) -> Any | None:
    """Create an LLMDecisionProvider from config, or None if LLM is not configured."""
    if config.llm is None:
        return None

    try:
        from agent_runtime.core.async_decide import AsyncDecisionProvider

        llm = create_provider(config.llm)
        logger.info(
            "LLM provider created: provider=%s model=%s base_url=%s",
            config.llm.provider.value,
            config.llm.model,
            config.llm.base_url or "(default)",
        )

        # Create the concurrency-controlled queue
        queue = LLMQueue(provider=llm, config=config.llm_queue)

        # Wrap with async decision provider
        inner_provider = LLMDecisionProvider(llm_provider=queue)  # type: ignore[arg-type]
        async_provider = AsyncDecisionProvider(inner=inner_provider)

        # Attach the queue so run_agent can stop it during shutdown
        setattr(async_provider, "_queue", queue)  # type: ignore[attr-defined]

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


def _create_social_context_provider(
    state: AgentState,
) -> tuple[Any, list[dict[str, Any]]] | None:
    """Create a DefaultSocialContextProvider wired to the agent's state."""
    try:
        from agent_runtime.models.personality import PersonalityVector
        from agent_runtime.models.values import ValueWeights
        from agent_runtime.social.provider import (
            AgentProfile,
            DefaultSocialContextProvider,
        )

        agent_id = str(state.id)

        def _profile_source(aid: str) -> AgentProfile | None:
            if aid != agent_id:
                return None
            personality = PersonalityVector.from_storage_dict(state.personality)
            values = ValueWeights()
            return AgentProfile(
                personality=personality,
                values=values,
                group_ids=[],
            )

        # Mutable cache updated by the think loop after each perception.
        nearby_cache: list[dict[str, Any]] = []

        def _nearby_source(
            aid: str,
            tick: int,  # noqa: ARG001
        ) -> list[dict[str, Any]]:
            return list(nearby_cache)

        provider = DefaultSocialContextProvider(
            profile_source=_profile_source,  # type: ignore[arg-type]
            nearby_source=_nearby_source,  # type: ignore[arg-type]
        )
        logger.info("SocialContextProvider created for agent %s", state.name)
        return provider, nearby_cache
    except Exception:
        logger.debug(
            "SocialContextProvider creation failed (non-fatal), "
            "social context will not be available",
            exc_info=True,
        )
        return None


# ---------------------------------------------------------------------------
# run_agent
# ---------------------------------------------------------------------------


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
    llm_queue = _find_llm_queue(decision_provider)
    if llm_queue is not None:
        try:
            await llm_queue.start()
        except Exception:
            logger.warning("Failed to start LLMQueue", exc_info=True)

    # Connect to World Engine (gRPC preferred, REST fallback)
    from agent_runtime.cli import _extract_grpc_address as _extract_grpc
    from agent_runtime.cli import _get_health_port as _get_health

    grpc_address = _extract_grpc(config.world.engine_url)
    from agent_runtime.runtime.bootstrap import (
        connect_world_engine,
        register_agent,
    )

    conn = await connect_world_engine(
        grpc_address=grpc_address,
        rest_url=config.world.engine_url,
        agent_id=str(state.id),
    )
    world_client = conn.world_client
    perception_provider = conn.perception_provider
    a2a_client = conn.a2a_client

    # Protect the entire post-connection lifecycle
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
            logger.info(
                "World Engine connection established (world_agent_id=%s, "
                "perception=%s, world_client=%s)",
                world_agent_id,
                type(perception_provider).__name__ if perception_provider else "None",
                type(world_client).__name__,
                extra={"agent": state.name, "event": "world_connected"},
            )

        # Build heartbeat provider if A2A client is available
        heartbeat_provider: Any | None = None
        if a2a_client is not None and config.think_loop.heartbeat_enabled:
            heartbeat_provider = _A2AHeartbeatAdapter(a2a_client)

        # Build social context provider
        social_context_result = _create_social_context_provider(state)
        social_context_provider: Any | None = None
        _social_nearby_cache: list[dict[str, Any]] = []
        if social_context_result is not None:
            social_context_provider, _social_nearby_cache = social_context_result

        # Build emotion hook
        emotion_hook: Any | None = None
        try:
            from agent_runtime.emotion.engine import EmotionEngine, ThinkLoopEmotionHook
            from agent_runtime.models.personality import PersonalityVector

            personality_data = getattr(state, "personality", None) or {}
            personality = PersonalityVector(
                openness=float(personality_data.get("openness", 0.5)),
                conscientiousness=float(personality_data.get("conscientiousness", 0.5)),
                extraversion=float(personality_data.get("extraversion", 0.5)),
                agreeableness=float(personality_data.get("agreeableness", 0.5)),
                neuroticism=float(personality_data.get("neuroticism", 0.5)),
                risk_tolerance=float(personality_data.get("risk_tolerance", 0.5)),
                social_orientation=float(personality_data.get("social_orientation", 0.5)),
                greed=float(personality_data.get("greed", 0.5)),
            )
            emotion_engine = EmotionEngine(personality=personality)
            emotion_hook = ThinkLoopEmotionHook(emotion_engine)
            logger.info(
                "EmotionEngine wired (personality: O=%.1f E=%.1f N=%.1f)",
                personality.openness,
                personality.extraversion,
                personality.neuroticism,
            )
        except Exception:
            logger.debug("EmotionEngine setup failed (non-fatal)", exc_info=True)

        # Build federation sync hook (optional — Phase 2; disabled by default).
        federation_hook: Any | None = None
        try:
            from agent_runtime.federation import build_federation_sync

            federation_hook = build_federation_sync(config.world.engine_url)
            if federation_hook is not None:
                logger.info(
                    "Federation sync enabled (world_id=%s, bootstrap_peers=%d)",
                    federation_hook.config.world_id,
                    len(federation_hook.config.bootstrap_peers),
                )
        except Exception:
            logger.debug("Federation sync setup failed (non-fatal)", exc_info=True)

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
            social_nearby_cache=_social_nearby_cache,
            emotion_hook=emotion_hook,
            federation_hook=federation_hook,
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
        from agent_runtime.runtime.health_check import HealthCheckServer

        health_port = _get_health(config)
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
            await think_task
        finally:
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
            from agent_runtime.runtime.bootstrap import deregister_agent

            await deregister_agent(stats.agent_id, config.world.engine_url)
    finally:
        # Close A2A connection if active
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
