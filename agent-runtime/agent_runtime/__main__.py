"""Agent Runtime CLI — ``python -m agent_runtime``.

Clean entry point. All implementation has been moved into dedicated modules:

- ``agent_runtime.cli`` — argument parsing, config building, logging
- ``agent_runtime.a2a.rest_world_client`` — REST-based world client
- ``agent_runtime.perception.rest_provider`` — REST perception provider
- ``agent_runtime.runtime.bootstrap`` — World Engine connection, registration
- ``agent_runtime.runtime.agent_loop`` — spawn, run_agent, decision providers
- ``agent_runtime.runtime.health_check`` — health check HTTP server
- ``agent_runtime.runtime.pool`` — multi-agent pool management

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
    python -m agent_runtime spawn --name Alice --world-url http://localhost:8080
"""

from __future__ import annotations

import asyncio
import json
import sys

from agent_runtime import __version__
from agent_runtime.a2a.rest_world_client import RESTWorldClient
from agent_runtime.cli import (
    _apply_preset_defaults,
    _extract_grpc_address,
    _get_health_port,
    _has_world_arg,
    _rewrite_world_to_world_url,
    build_config_from_args,
    build_parser,
    parse_skills,
    parse_traits,
    setup_logging,
)
from agent_runtime.env_loader import load_dotenv
from agent_runtime.perception.rest_provider import RESTPerceptionProvider
from agent_runtime.runtime.agent_loop import (
    RunStats,
    _A2AHeartbeatAdapter,
    _init_data_dir,
    _load_agent_state_from_dir,
    _save_agent_state_to_dir,
    run_agent,
    spawn_agent,
)
from agent_runtime.runtime.bootstrap import (
    WorldConnection,
    connect_world_engine,
    deregister_agent,
    register_agent,
)
from agent_runtime.runtime.health_check import HealthCheckServer
from agent_runtime.runtime.pool import (
    AgentPool,
    PoolAgentInfo,
    _build_pool_spawn_args,
    _run_publish,
    run_pool,
)

# Backward-compat re-exports (tests and other code import these from __main__)
__all__ = [
    "__version__",
    "RESTPerceptionProvider",
    "RESTWorldClient",
    "AgentPool",
    "HealthCheckServer",
    "PoolAgentInfo",
    "RunStats",
    "WorldConnection",
    "_A2AHeartbeatAdapter",
    "_apply_preset_defaults",
    "_build_pool_spawn_args",
    "_extract_grpc_address",
    "_get_health_port",
    "_has_world_arg",
    "_init_data_dir",
    "_load_agent_state_from_dir",
    "_run_publish",
    "_save_agent_state_to_dir",
    "_rewrite_world_to_world_url",
    "build_config_from_args",
    "build_parser",
    "connect_world_engine",
    "deregister_agent",
    "parse_skills",
    "parse_traits",
    "register_agent",
    "run_agent",
    "run_pool",
    "setup_logging",
    "spawn_agent",
]


def main() -> None:
    """CLI entry point — parse args and run."""
    load_dotenv()

    parser = build_parser()
    args = parser.parse_args()

    # Auto-default to 'spawn' when no subcommand but --world is given
    if args.command is None:
        if _has_world_arg(sys.argv[1:]):
            rewritten = _rewrite_world_to_world_url(sys.argv[1:])
            args = parser.parse_args(["spawn"] + rewritten)
        else:
            parser.print_help()
            sys.exit(1)

    setup_logging(verbose=args.verbose, json_output=not args.log_text)

    if args.command == "spawn":
        config = build_config_from_args(args)
        stats = asyncio.run(run_agent(config))
        print(json.dumps(stats.to_dict(), indent=2))
    elif args.command == "pool":
        result = asyncio.run(run_pool(args))
        print(json.dumps(result, indent=2))
    elif args.command == "publish":
        _run_publish(args)
    else:
        parser.print_help()
        sys.exit(1)


if __name__ == "__main__":
    main()
