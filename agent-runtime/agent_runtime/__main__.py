"""Agent Runtime CLI entry point.

Usage::

    python -m agent_runtime --name Alpha
    python -m agent_runtime --name Alpha --config agent.yaml --seed 500
"""

from __future__ import annotations

import argparse
import asyncio
import logging
import signal
import sys
from pathlib import Path
from typing import Any

import yaml

from agent_runtime import __version__
from agent_runtime.core.act import ActionExecutor
from agent_runtime.core.think_loop import ThinkLoop, ThinkLoopConfig
from agent_runtime.crypto.keys import generate_key_pair
from agent_runtime.models.agent_state import AgentState
from agent_runtime.models.enums import AgentPhase
from agent_runtime.survival.instinct import SurvivalInstinct

logger = logging.getLogger("agent_runtime")

# Default gRPC server address for the World Engine.
DEFAULT_GRPC_ADDR = "localhost:50051"


# ---------------------------------------------------------------------------
# Configuration loading
# ---------------------------------------------------------------------------


def load_config(config_path: str | None) -> dict[str, Any]:
    """Load agent configuration from a YAML file.

    Returns an empty dict when *config_path* is ``None``.
    """
    if config_path is None:
        return {}
    path = Path(config_path)
    if not path.exists():
        logger.warning("Config file not found: %s — using defaults", config_path)
        return {}
    with open(path) as fh:
        data = yaml.safe_load(fh)
    return data if isinstance(data, dict) else {}


# ---------------------------------------------------------------------------
# gRPC connection (placeholder)
# ---------------------------------------------------------------------------


async def connect_world_engine(address: str) -> None:
    """Connect to the World Engine gRPC server.

    Performs a basic connectivity check using gRPC async channel.
    Non-fatal — if the server is unreachable the agent runs in
    standalone mode.
    """
    import grpc
    import grpc.aio

    logger.info("Connecting to World Engine at %s …", address)
    try:
        channel = grpc.aio.insecure_channel(address)
        # Use a short timeout; if unavailable we log and continue.
        await asyncio.wait_for(channel.channel_ready(), timeout=3)
        logger.info("Connected to World Engine at %s", address)
    except (asyncio.TimeoutError, grpc.aio.AioRpcError):
        logger.warning(
            "Could not reach World Engine at %s — running in standalone mode",
            address,
        )
    except Exception:
        logger.warning(
            "Error connecting to World Engine at %s — running in standalone mode",
            address,
        )


# ---------------------------------------------------------------------------
# Agent bootstrap
# ---------------------------------------------------------------------------


def build_agent_state(
    name: str,
    seed_tokens: int | None,
    config: dict[str, Any],
) -> AgentState:
    """Construct an :class:`AgentState` from CLI args + config file."""
    economy_cfg = config.get("economy", {})
    lifecycle_cfg = config.get("lifecycle", {})

    # Determine initial token count (CLI flag > config > genesis default).
    default_tokens = lifecycle_cfg.get(
        "birth_tokens",
        economy_cfg.get("initial_tokens", 1000),
    )
    tokens = seed_tokens if seed_tokens is not None else default_tokens

    # Generate a key pair for the agent identity.
    key_pair = generate_key_pair()

    state = AgentState(
        name=name,
        tokens=tokens,
        max_tokens=config.get("max_tokens", 100_000),
        phase=AgentPhase.INITIALIZATION,
    )

    logger.info(
        "Agent spawned: name=%s id=%s tokens=%d max_tokens=%d",
        state.name,
        state.id,
        state.tokens,
        state.max_tokens,
    )
    logger.debug("Agent public key: %s", key_pair.public_key_b64())
    return state


# ---------------------------------------------------------------------------
# CLI argument parser
# ---------------------------------------------------------------------------


def build_parser() -> argparse.ArgumentParser:
    """Build the argument parser for the agent CLI."""
    parser = argparse.ArgumentParser(
        prog="agent_runtime",
        description="Agent World — Agent Runtime CLI",
    )
    parser.add_argument(
        "--name",
        required=True,
        help="Name of the agent to spawn (e.g. Alpha, Beta).",
    )
    parser.add_argument(
        "--config",
        default=None,
        help="Path to a YAML configuration file.",
    )
    parser.add_argument(
        "--seed",
        type=int,
        default=None,
        help="Initial token count for the agent.",
    )
    parser.add_argument(
        "--server",
        default=DEFAULT_GRPC_ADDR,
        help=f"World Engine gRPC server address (default: {DEFAULT_GRPC_ADDR}).",
    )
    parser.add_argument(
        "--tick-interval",
        type=float,
        default=1.0,
        help="Seconds between think-loop ticks (default: 1.0).",
    )
    parser.add_argument(
        "--max-ticks",
        type=int,
        default=0,
        help="Maximum number of ticks (0 = unlimited).",
    )
    parser.add_argument(
        "--log-level",
        default="INFO",
        choices=["DEBUG", "INFO", "WARNING", "ERROR", "CRITICAL"],
        help="Logging level (default: INFO).",
    )
    parser.add_argument(
        "--version",
        action="version",
        version=f"%(prog)s {__version__}",
    )
    return parser


# ---------------------------------------------------------------------------
# Main entry point
# ---------------------------------------------------------------------------


async def async_main(argv: list[str] | None = None) -> None:
    """Async entry point — parse args, bootstrap, run think loop."""
    parser = build_parser()
    args = parser.parse_args(argv)

    # Configure logging.
    logging.basicConfig(
        level=getattr(logging, args.log_level),
        format="%(asctime)s %(levelname)-8s %(name)s  %(message)s",
        stream=sys.stderr,
    )

    logger.info("Agent Runtime v%s starting…", __version__)

    # Load optional config file.
    config = load_config(args.config)

    # Build agent state.
    state = build_agent_state(args.name, args.seed, config)

    # Connect to World Engine (non-fatal if unavailable).
    await connect_world_engine(args.server)

    # Assemble the think loop components.
    survival = SurvivalInstinct()
    executor = ActionExecutor()
    loop_config = ThinkLoopConfig(
        tick_interval=args.tick_interval,
        max_ticks=args.max_ticks,
    )
    think_loop = ThinkLoop(
        state=state,
        survival=survival,
        executor=executor,
        config=loop_config,
    )

    # Set up graceful shutdown via SIGTERM / SIGINT.
    loop = asyncio.get_running_loop()
    stop_event = asyncio.Event()

    def _signal_handler() -> None:
        logger.info("Received stop signal — shutting down gracefully…")
        think_loop.stop()
        stop_event.set()

    for sig in (signal.SIGTERM, signal.SIGINT):
        loop.add_signal_handler(sig, _signal_handler)

    logger.info(
        "Starting think loop: name=%s tick_interval=%.2fs max_ticks=%s",
        args.name,
        args.tick_interval,
        args.max_ticks or "unlimited",
    )

    # Run the think loop.
    try:
        await think_loop.run()
    except Exception:
        logger.exception("Think loop terminated with an error")
        raise
    finally:
        logger.info("Agent %s shut down.", args.name)


def main() -> None:
    """Synchronous entry point for ``python -m agent_runtime``."""
    try:
        asyncio.run(async_main())
    except KeyboardInterrupt:
        pass


if __name__ == "__main__":
    main()
