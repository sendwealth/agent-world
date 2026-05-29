#!/usr/bin/env python3
"""
Agent World — Custom Agent Example (Python)

A complete, runnable example demonstrating how to:
  1. Register an agent with the World Engine
  2. Read perception (observe the world)
  3. Execute actions (move, gather, rest, etc.)
  4. Check agent status
  5. Deregister on shutdown

Run with:
    python custom_agent.py

Requires the agent_runtime SDK package:
    pip install -e ./agent-runtime
"""

import os
import sys
import time
import signal
import logging
import random

from agent_runtime.sdk import AgentWorldClient

# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------

# Base URL of the Agent World server.
BASE_URL = os.environ.get("AGENT_WORLD_BASE_URL", "http://localhost:3000")

# How many simulation ticks to run before exiting (0 = run forever).
MAX_TICKS = int(os.environ.get("MAX_TICKS", "10"))

# Seconds to sleep between ticks.
TICK_INTERVAL = float(os.environ.get("TICK_INTERVAL", "1.0"))

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [%(levelname)s] %(message)s",
)
log = logging.getLogger("custom_agent")

# ---------------------------------------------------------------------------
# Globals for graceful shutdown
# ---------------------------------------------------------------------------
agent_id: str | None = None
client: AgentWorldClient | None = None
running = True


def handle_signal(signum, frame):
    """Catch SIGINT / SIGTERM so we can deregister cleanly."""
    global running
    log.info("Received shutdown signal, cleaning up …")
    running = False


signal.signal(signal.SIGINT, handle_signal)
signal.signal(signal.SIGTERM, handle_signal)


# ---------------------------------------------------------------------------
# Decision-making helper
# ---------------------------------------------------------------------------

def decide_action(perception: dict) -> tuple[str, dict]:
    """
    Simple rule-based decision loop (Perceive-Decide-Act).

    Priorities:
      1. Gather if resources are nearby.
      2. Move in a random direction otherwise.
    """
    resources = perception.get("nearby_resources", [])

    # 1) Gather if resources are present.
    if resources:
        return "gather", {}

    # 2) Move in a random direction.
    directions = ["north", "south", "east", "west"]
    return "move", {"direction": random.choice(directions)}


# ---------------------------------------------------------------------------
# Main loop
# ---------------------------------------------------------------------------

def main():
    global agent_id, client

    # ------------------------------------------------------------------
    # Step 1 — Initialise the SDK client
    # ------------------------------------------------------------------
    log.info("Connecting to Agent World at %s", BASE_URL)
    client = AgentWorldClient(base_url=BASE_URL)

    # ------------------------------------------------------------------
    # Step 2 — Register the agent
    # ------------------------------------------------------------------
    log.info("Registering agent …")
    registration = client.register(
        name="PyScout",
        capabilities=["move", "gather", "explore", "rest"],
    )
    agent_id = registration["agent_id"]
    log.info("Registered as %s (id=%s)", registration["name"], agent_id)

    try:
        # --------------------------------------------------------------
        # Step 3 — Main simulation loop (Perceive → Decide → Act)
        # --------------------------------------------------------------
        tick_count = 0
        while running:
            if MAX_TICKS > 0 and tick_count >= MAX_TICKS:
                log.info("Reached MAX_TICKS=%d, stopping.", MAX_TICKS)
                break

            tick_count += 1
            log.info("--- Tick %d ---", tick_count)

            # 3a. Observe the world (perception).
            perception = client.perception(agent_id)
            log.info(
                "Perception: position=%s, %d nearby agents, %d resources",
                perception.get("position"),
                len(perception.get("nearby_agents", [])),
                len(perception.get("nearby_resources", [])),
            )

            # 3b. Decide what to do next.
            action, params = decide_action(perception)
            log.info("Action: %s %s", action, params)

            # 3c. Execute the action.
            result = client.action(agent_id, action, params)
            log.info(
                "Result: action=%s success=%s tick=%s",
                result.get("action"),
                result.get("success"),
                result.get("tick"),
            )

            # 3d. (Optional) Check status every 5 ticks.
            if tick_count % 5 == 0:
                status = client.status(agent_id)
                log.info(
                    "Status — alive=%s, money=%s, position=%s, phase=%s",
                    status.get("alive"),
                    status.get("money"),
                    status.get("position"),
                    status.get("phase"),
                )

            # Pause between ticks.
            time.sleep(TICK_INTERVAL)

    finally:
        # --------------------------------------------------------------
        # Step 4 — Deregister the agent on exit
        # --------------------------------------------------------------
        if agent_id:
            log.info("Deregistering agent %s …", agent_id)
            try:
                client.deregister(agent_id)
                log.info("Agent deregistered.")
            except Exception as exc:
                log.error("Failed to deregister: %s", exc)
        client.close()


if __name__ == "__main__":
    main()
