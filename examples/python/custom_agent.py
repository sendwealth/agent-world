#!/usr/bin/env python3
"""
Agent World — Custom Agent Example (Python)

A complete, runnable example demonstrating how to:
  1. Register an agent with the simulation
  2. Read perception (observe the world)
  3. Execute actions (move, gather, rest, etc.)
  4. Check agent status
  5. Deregister on shutdown

Run with:
    python custom_agent.py

Requires the agent_runtime SDK package:
    pip install agent-runtime
"""

import os
import sys
import time
import signal
import logging

from agent_runtime.sdk import AgentWorldClient

# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------

# API key — can also be set via the AGENT_WORLD_API_KEY environment variable.
API_KEY = os.environ.get("AGENT_WORLD_API_KEY", "your-api-key-here")

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
# Decision-making helpers
# ---------------------------------------------------------------------------

def decide_action(perception: dict) -> dict:
    """
    Very simple rule-based decision loop.

    Priorities:
      1. Rest if energy is low (< 20).
      2. Gather if resources are present on the current tile.
      3. Explore / move in a random direction otherwise.
    """
    # Extract useful data from perception (defensive defaults).
    center = perception.get("center", {"x": 0, "y": 0})
    tiles = perception.get("tiles", [])

    # We need the agent's energy — fetch it from the status field if present,
    # or just assume full energy if not available in perception.
    # (In practice you'd call client.get_status() separately.)

    # 1) Rest if energy is critically low.
    energy = perception.get("energy", 100)
    if energy < 20:
        return {"type": "rest", "params": {}}

    # 2) Gather if there are resources on the current tile.
    current_tile = next(
        (t for t in tiles if t["x"] == center["x"] and t["y"] == center["y"]),
        None,
    )
    if current_tile and current_tile.get("resources"):
        resource_name = next(iter(current_tile["resources"]))
        return {
            "type": "gather",
            "params": {"resource": resource_name},
        }

    # 3) Move — cycle through cardinal directions.
    directions = ["north", "east", "south", "west"]
    direction = directions[tick_count % len(directions)]
    return {"type": "move", "params": {"direction": direction}}


# ---------------------------------------------------------------------------
# Main loop
# ---------------------------------------------------------------------------

def main():
    global agent_id, client, tick_count

    tick_count = 0

    # ------------------------------------------------------------------
    # Step 1 — Initialise the SDK client
    # ------------------------------------------------------------------
    log.info("Connecting to Agent World at %s", BASE_URL)
    client = AgentWorldClient(base_url=BASE_URL, api_key=API_KEY)

    # ------------------------------------------------------------------
    # Step 2 — Register the agent
    # ------------------------------------------------------------------
    log.info("Registering agent …")
    registration = client.register(
        name="PyScout",
        kind="explorer",
        metadata={
            "language": "python",
            "owner": "example",
            "version": "1.0.0",
        },
    )
    agent_id = registration["id"]
    log.info("Registered as %s (id=%s)", registration["name"], agent_id)

    try:
        # --------------------------------------------------------------
        # Step 3 — Main simulation loop
        # --------------------------------------------------------------
        while running:
            if MAX_TICKS > 0 and tick_count >= MAX_TICKS:
                log.info("Reached MAX_TICKS=%d, stopping.", MAX_TICKS)
                break

            tick_count += 1
            log.info("--- Tick %d ---", tick_count)

            # 3a. Observe the world (perception).
            perception = client.get_perception(agent_id, radius=3)
            log.info(
                "Perception: %d tiles, %d nearby agents",
                len(perception.get("tiles", [])),
                len(perception.get("nearby_agents", [])),
            )

            # 3b. Decide what to do next.
            action = decide_action(perception)
            log.info("Action: %s %s", action["type"], action.get("params", {}))

            # 3c. Execute the action.
            result = client.execute_action(agent_id, action)
            log.info("Result: %s — %s", result.get("result"), result.get("message"))

            # 3d. (Optional) Check status every 5 ticks.
            if tick_count % 5 == 0:
                status = client.get_status(agent_id)
                log.info(
                    "Status — energy: %s, position: %s, inventory: %s",
                    status.get("energy"),
                    status.get("position"),
                    status.get("inventory"),
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


if __name__ == "__main__":
    main()
