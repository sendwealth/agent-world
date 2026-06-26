"""World Engine connection and agent registration.

Provides ``connect_world_engine`` (gRPC with REST fallback),
``register_agent``, and ``deregister_agent``.
"""

from __future__ import annotations

import asyncio
import logging
from dataclasses import dataclass
from typing import Any

from agent_runtime.a2a.rest_world_client import RESTWorldClient
from agent_runtime.models.agent_state import AgentState
from agent_runtime.perception.rest_provider import RESTPerceptionProvider

logger = logging.getLogger(__name__)


@dataclass
class WorldConnection:
    """Holds the world client and optional perception provider from a connection."""

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
        except Exception as exc:
            await client.close()
            raise ConnectionError(f"gRPC channel not ready: {grpc_address}") from exc

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
    rest_perception = RESTPerceptionProvider(rest_client)
    logger.info(
        "Using REST fallback for World Engine at %s",
        rest_url,
        extra={"agent": agent_id, "event": "rest_fallback"},
    )
    return WorldConnection(
        world_client=rest_client,
        perception_provider=rest_perception,
    )


async def register_agent(
    state: AgentState,
    world_url: str,
    *,
    public_key_b64: str | None = None,
    timeout: float = 5.0,  # noqa: ASYNC109
    max_retries: int = 3,
    retry_delay: float = 2.0,
) -> str | None:
    """Register the agent with the World Engine as an *external* agent.

    Uses the ``POST /api/v1/agents/register`` endpoint which stores the
    agent in the World Engine's ``external_agents`` map — the same map
    that ``POST /api/v1/agents/:id/action`` looks up.

    Retries up to ``max_retries`` times with ``retry_delay`` second delay
    between attempts to handle transient startup ordering in Docker Compose.

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
        state.name,
        state.id,
        url,
    )

    last_exc: Exception | None = None
    for attempt in range(1, max_retries + 1):
        try:
            async with httpx.AsyncClient(timeout=timeout, trust_env=False) as client:
                resp = await client.post(url, json=payload)
                if resp.status_code in (200, 201):
                    body = resp.json()
                    world_agent_id = body.get("agent_id")
                    logger.info(
                        "Agent registered successfully (world_id=%s) on attempt %d",
                        world_agent_id,
                        attempt,
                        extra={"agent": state.name, "event": "registered"},
                    )
                    return world_agent_id
                logger.warning(
                    "World Engine returned %d on attempt %d: %s",
                    resp.status_code,
                    attempt,
                    resp.text[:200] if resp.text else "(empty)",
                )
                # Non-connection errors (4xx/5xx) — don't retry
                if resp.status_code < 500:
                    return None
        except httpx.ConnectError as exc:
            last_exc = exc
            logger.warning(
                "World Engine unreachable at %s (attempt %d/%d) — %s",
                world_url,
                attempt,
                max_retries,
                exc,
            )
        except Exception as exc:
            last_exc = exc
            logger.warning(
                "Registration attempt %d/%d failed: %s",
                attempt,
                max_retries,
                exc,
                exc_info=True,
            )

        # Wait before retrying (except on last attempt)
        if attempt < max_retries:
            logger.info(
                "Retrying registration in %.1fs...",
                retry_delay,
                extra={"agent": state.name},
            )
            await asyncio.sleep(retry_delay)

    logger.error(
        "Failed to register with World Engine after %d attempts — "
        "running in standalone mode (last error: %s)",
        max_retries,
        last_exc,
    )
    return None


async def deregister_agent(
    agent_id: str,
    world_url: str,
    *,
    timeout: float = 5.0,  # noqa: ASYNC109
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
