"""GRPCPerceptionProvider — implements PerceptionProvider for the SENSE phase.

Receives messages from other agents via the A2A streaming connection and
calls Discover to populate the market_state in the Perception object.
Gracefully degrades when the network is unavailable.
"""

from __future__ import annotations

import logging
from typing import Any

from agent_runtime.core.think_loop import Perception
from agent_runtime.models.agent_state import AgentState

from .client import A2AClient
from .message import a2a_message_to_dict

logger = logging.getLogger(__name__)


class GRPCPerceptionProvider:
    """gRPC-backed PerceptionProvider for the Think Loop SENSE phase.

    Uses the A2A client's streaming queue to receive incoming messages
    from other agents, and the Discover RPC to find nearby agents for
    the market_state.

    Falls back to empty data on network errors — the Think Loop must
    never crash due to a transient network issue.

    Usage::

        a2a = A2AClient(config)
        await a2a.connect()
        await a2a.start_streaming()

        provider = GRPCPerceptionProvider(a2a)
        perception = await provider.perceive(state, tick=42)
    """

    def __init__(self, a2a_client: A2AClient) -> None:
        self._client = a2a_client

    async def perceive(self, state: AgentState, tick: int) -> Perception:
        """Build a Perception from the agent's state and A2A messages.

        - Drains incoming messages from the streaming queue.
        - Calls Discover to populate market_state with nearby agents.
        - Returns safe defaults on any network error.
        """
        messages = await self._drain_messages()
        market_state = await self._discover_market(state)

        max_tokens = getattr(state, "max_tokens", 0)
        ratio = state.tokens / max_tokens if max_tokens > 0 else 0.0

        return Perception(
            messages=messages,
            token_balance=state.tokens,
            token_ratio=ratio,
            market_state=market_state,
            active_task=getattr(state, "current_task", None),
            health=state.health,
            tick=tick,
        )

    # ------------------------------------------------------------------
    # Internal
    # ------------------------------------------------------------------

    async def _drain_messages(self) -> list[dict[str, Any]]:
        """Drain all available messages from the streaming queue."""
        messages: list[dict[str, Any]] = []
        try:
            while True:
                msg = self._client._incoming_queue.get_nowait()
                messages.append(a2a_message_to_dict(msg))
        except Exception:
            pass  # queue empty or not streaming
        return messages

    async def _discover_market(self, state: AgentState) -> dict[str, Any]:
        """Call Discover to get world/market state."""
        try:
            response = await self._client.discover()
            agents = [
                {
                    "agent_id": a.agent_id,
                    "name": a.name,
                    "tokens": a.tokens,
                    "reputation": a.reputation,
                }
                for a in response.agents
            ]
            return {"nearby_agents": agents, "agent_count": len(agents)}
        except Exception:
            logger.debug("Discover failed, returning empty market_state")
            return {}
