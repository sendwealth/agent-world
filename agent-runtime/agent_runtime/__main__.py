"""Agent Runtime entry point.

Usage: python -m agent_runtime

Connects to the World Engine and starts a think loop for one agent.
"""

from __future__ import annotations

import asyncio
import logging
import os
import sys

from agent_runtime.core.act import ActionExecutor
from agent_runtime.core.think_loop import ThinkLoop, ThinkLoopConfig
from agent_runtime.models.agent_state import AgentState
from agent_runtime.survival.instinct import SurvivalInstinct

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [%(levelname)s] %(name)s: %(message)s",
)
logger = logging.getLogger(__name__)


async def main() -> None:
    world_engine_url = os.environ.get("WORLD_ENGINE_URL", "http://localhost:8080")
    agent_name = os.environ.get("AGENT_NAME", "Agent-1")
    tick_interval = float(os.environ.get("TICK_INTERVAL", "1.0"))

    logger.info("Agent Runtime starting")
    logger.info("  World Engine: %s", world_engine_url)
    logger.info("  Agent name: %s", agent_name)

    state = AgentState(name=agent_name, max_tokens=1000, tokens=500)
    survival = SurvivalInstinct()
    executor = ActionExecutor()

    loop = ThinkLoop(
        state=state,
        survival=survival,
        executor=executor,
        config=ThinkLoopConfig(tick_interval=tick_interval),
    )

    logger.info("Starting think loop...")
    await loop.run()


if __name__ == "__main__":
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        logger.info("Agent Runtime stopped")
        sys.exit(0)
