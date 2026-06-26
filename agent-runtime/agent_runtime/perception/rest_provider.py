"""REST perception provider.

Uses ``GET /api/v1/agents/{id}/perception`` to get nearby agents,
resources, position, and the current world tick, then builds a
``Perception`` object that the ThinkLoop can consume.
"""

from __future__ import annotations

import logging
from typing import Any

from agent_runtime.a2a.rest_world_client import RESTWorldClient
from agent_runtime.core.think_loop import Perception
from agent_runtime.models.agent_state import AgentState

logger = logging.getLogger(__name__)


class RESTPerceptionProvider:
    """Fetches perception data from the World Engine REST API."""

    def __init__(self, rest_client: RESTWorldClient) -> None:
        self._client = rest_client

    async def perceive(self, state: AgentState, tick: int) -> Any:
        """Fetch world state from the World Engine and build a Perception."""
        try:
            data = await self._client.get_perception()
        except Exception:
            logger.debug(
                "Failed to fetch perception from World Engine, using local state",
                exc_info=True,
            )
            # Fallback to local-only perception
            max_tokens = getattr(state, "max_tokens", None)
            ratio = (state.tokens / max_tokens) if max_tokens and max_tokens > 0 else 0.0
            return Perception(
                messages=[],
                token_balance=state.tokens,
                token_ratio=ratio,
                market_state={},
                active_task=None,
                health=state.health,
                tick=tick,
            )

        # Extract world state
        nearby_agents = data.get("nearby_agents", [])
        nearby_resources = data.get("nearby_resources", [])
        position = data.get("position", {})
        world_tick = data.get("world_tick", tick)

        # Build market_state from perception data
        market_state: dict[str, Any] = {
            "nearby_agents": nearby_agents,
            "nearby_resources": nearby_resources,
            "position": position,
            "world_tick": world_tick,
        }

        max_tokens = getattr(state, "max_tokens", None)
        ratio = (state.tokens / max_tokens) if max_tokens and max_tokens > 0 else 0.0

        return Perception(
            messages=[],
            token_balance=state.tokens,
            token_ratio=ratio,
            market_state=market_state,
            active_task=None,
            health=state.health,
            tick=tick,
            server_tick=world_tick,
        )
