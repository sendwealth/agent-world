"""gRPC-based perception provider for the Think Loop.

Fetches world state via the A2A gRPC client during the Perceive phase.
Replaces ``DefaultPerceptionProvider`` which only reads local state.
"""

from __future__ import annotations

import logging
from typing import TYPE_CHECKING

from agent_runtime.core.think_loop import Perception

if TYPE_CHECKING:
    from agent_runtime.a2a.client import A2AClient
    from agent_runtime.models.agent_state import AgentState

logger = logging.getLogger(__name__)


class GRpcPerceptionProvider:
    """Perception provider backed by the A2A gRPC client.

    During each tick's Perceive phase, this provider:
    1. Fetches unread messages from the World Engine via gRPC.
    2. Reads the agent's local token balance and health.
    3. Assembles a ``Perception`` snapshot for the Decide phase.

    If the gRPC call fails, falls back gracefully to an empty perception
    so the Think Loop can continue operating.
    """

    def __init__(self, client: A2AClient) -> None:
        self._client = client

    async def perceive(self, state: AgentState, tick: int) -> Perception:
        """Build a Perception from gRPC-fetched world state.

        Args:
            state: Current agent state.
            tick: Current tick number.

        Returns:
            A ``Perception`` with real messages and world data when
            the gRPC connection is available, or a best-effort
            fallback otherwise.
        """
        # Fetch unread messages via gRPC
        messages = await self._safe_fetch_messages()

        # Compute token ratio from local state
        max_tokens = getattr(state, "max_tokens", None)
        if max_tokens and max_tokens > 0:
            ratio = state.tokens / max_tokens
        else:
            ratio = 0.0

        # Check for active task
        active_task = getattr(state, "current_task", None)

        return Perception(
            messages=messages,
            token_balance=state.tokens,
            token_ratio=ratio,
            market_state={},
            active_task=active_task,
            health=state.health,
            tick=tick,
        )

    async def _safe_fetch_messages(self) -> list[dict]:
        """Fetch messages with error handling.

        Returns an empty list on any failure so the think loop
        can continue operating in degraded mode.
        """
        try:
            return await self._client.get_unread_messages()
        except Exception:
            logger.debug("Failed to fetch messages via gRPC", exc_info=True)
            return []
