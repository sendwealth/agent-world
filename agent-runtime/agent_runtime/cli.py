"""CLI argument parsing and configuration building.

Provides ``build_parser``, ``build_config_from_args``, ``parse_traits``,
``parse_skills``, ``setup_logging``, and related helpers.
"""

from __future__ import annotations

import argparse
import json
import logging
import os
import sys
import time
from pathlib import Path
from typing import Any

from agent_runtime import __version__
from agent_runtime.config import (
    RuntimeConfig,
    load_runtime_config,
)
from agent_runtime.llm.base import LLMConfig, ProviderType

logger = logging.getLogger(__name__)


class JSONFormatter(logging.Formatter):
    """Emit log records as single-line JSON objects."""

    def format(self, record: logging.LogRecord) -> str:
        entry: dict[str, Any] = {
            "ts": time.strftime("%Y-%m-%dT%H:%M:%S", time.gmtime(record.created))
            + f".{int(record.created % 1 * 1_000_000):06d}",
            "level": record.levelname,
            "logger": record.name,
            "msg": record.getMessage(),
        }

        for key in ("agent", "tick", "action", "duration_s", "event"):
            val = getattr(record, key, None)
            if val is not None:
                entry[key] = val

        if record.exc_info and record.exc_info[1] is not None:
            entry["error"] = str(record.exc_info[1])

        return json.dumps(entry, default=str, ensure_ascii=False)


def setup_logging(verbose: bool = False, json_output: bool = True) -> None:
    """Configure structured JSON logging for the runtime."""
    level = logging.DEBUG if verbose else logging.INFO
    handler = logging.StreamHandler(sys.stderr)
    handler.setLevel(level)

    if json_output:
        handler.setFormatter(JSONFormatter())
    else:
        handler.setFormatter(logging.Formatter("%(asctime)s [%(levelname)s] %(name)s: %(message)s"))

    root = logging.getLogger("agent_runtime")
    root.setLevel(level)
    root.handlers.clear()
    root.addHandler(handler)


# ---------------------------------------------------------------------------
# CLI argument parsing
# ---------------------------------------------------------------------------


def _add_spawn_args(parser: argparse.ArgumentParser) -> None:
    """Add spawn-related arguments to a sub-parser (shared by spawn and pool)."""
    parser.add_argument("--name", default=None, help="Agent name (default: Agent)")
    parser.add_argument(
        "--config",
        type=Path,
        default=None,
        help="Path to TOML or YAML config file",
    )
    parser.add_argument(
        "--skills",
        default=None,
        help="Comma-separated skill names (e.g. coding,trading,research)",
    )
    parser.add_argument(
        "--traits",
        nargs="*",
        default=None,
        help="Personality traits as key=value pairs (e.g. curiosity=0.8 caution=0.3)",
    )
    parser.add_argument(
        "--tokens",
        type=int,
        default=None,
        help="Initial token balance",
    )
    parser.add_argument(
        "--max-tokens",
        type=int,
        default=None,
        help="Maximum token capacity",
    )
    parser.add_argument(
        "--max-ticks",
        type=int,
        default=None,
        help="Maximum ticks to run (0 = unlimited)",
    )
    parser.add_argument(
        "--tick-interval",
        type=float,
        default=None,
        help="Seconds between ticks",
    )
    parser.add_argument(
        "--world-url",
        default=None,
        help="World Engine URL (default: http://localhost:8080)",
    )
    parser.add_argument(
        "--llm-provider",
        choices=["openai", "anthropic", "ollama", "zhipu", "google", "azure"],
        default=None,
        help=(
            "LLM provider (default: ollama; "
            "zhipu maps to OpenAI-compatible GLM-5 API; "
            "google/azure via registry)"
        ),
    )
    parser.add_argument(
        "--llm-model",
        default=None,
        help="LLM model name (default: glm-4-flash)",
    )
    parser.add_argument(
        "--llm-base-url",
        default=None,
        help="LLM API base URL",
    )
    parser.add_argument(
        "--no-llm",
        action="store_true",
        help="Disable LLM and use mock random decisions",
    )
    parser.add_argument(
        "--mock-llm",
        default=None,
        help=(
            "Use preset LLM mock for deterministic decisions. "
            "Options: hungry_gather, social_nearby, survival. "
            "Can also be set via MOCK_LLM_PRESET env var."
        ),
    )
    parser.add_argument(
        "--health-port",
        type=int,
        default=None,
        help="Health check HTTP port (default: 9090, env: HEALTH_PORT)",
    )
    parser.add_argument(
        "--data-dir",
        type=Path,
        default=None,
        help="Agent data directory for isolated storage (memory.db, skills.json, trace.db)",
    )
    parser.add_argument(
        "--preset",
        default=None,
        help=(
            "Provider preset name (e.g. zhipu, deepseek, ollama-local, openrouter). "
            "Auto-fills --llm-provider, --llm-base-url, and --llm-model. "
            "Explicit CLI flags override preset values."
        ),
    )


def build_parser() -> argparse.ArgumentParser:
    """Build the argument parser for the CLI."""
    parser = argparse.ArgumentParser(
        prog="agent_runtime",
        description="Agent World — Agent Runtime CLI. Spawn and run AI agents.",
    )
    parser.add_argument("--version", action="version", version=f"%(prog)s {__version__}")
    parser.add_argument("-v", "--verbose", action="store_true", help="Enable debug logging")
    parser.add_argument(
        "--log-text",
        action="store_true",
        help="Use human-readable log format instead of JSON (default: JSON)",
    )

    # Top-level --world shortcut (alias for spawn --world-url)
    parser.add_argument(
        "--world",
        default=None,
        dest="world",
        help="World Engine URL — shorthand that implies 'spawn' (e.g. --world http://localhost:8080)",
    )

    sub = parser.add_subparsers(dest="command", help="Available commands")

    # -- spawn --
    spawn_parser = sub.add_parser("spawn", help="Spawn and run a single agent")
    _add_spawn_args(spawn_parser)

    # -- pool --
    pool_parser = sub.add_parser("pool", help="Spawn and manage a pool of agents")
    pool_parser.add_argument(
        "--count",
        type=int,
        default=1,
        help="Number of agents to launch with auto-naming (Agent-1..N, default: 1)",
    )
    pool_parser.add_argument(
        "--config-dir",
        type=Path,
        default=None,
        help="Directory of .toml agent configs (one file per agent)",
    )
    pool_parser.add_argument(
        "--max-restart",
        type=int,
        default=3,
        help="Max restart attempts per crashed agent (default: 3)",
    )
    pool_parser.add_argument(
        "--health-interval",
        type=float,
        default=10.0,
        help="Health check interval in seconds (default: 10)",
    )
    pool_parser.add_argument(
        "--api-port",
        type=int,
        default=9090,
        help="Pool API HTTP port (default: 9090)",
    )
    # Reuse all spawn args
    _add_spawn_args(pool_parser)

    # -- publish --
    publish_parser = sub.add_parser(
        "publish",
        help="Package and publish an experiment to Zenodo/Dataverse (DOI)",
    )
    publish_parser.add_argument(
        "experiment_dir",
        type=Path,
        help="Path to experiment directory or report.json / reference.json",
    )
    publish_parser.add_argument(
        "--backend",
        choices=["zenodo", "dataverse"],
        default="zenodo",
        help="Publishing backend (default: zenodo)",
    )
    publish_parser.add_argument(
        "--env-file",
        type=Path,
        default=None,
        help="Path to .env file to load before publishing",
    )
    publish_parser.add_argument(
        "--production",
        action="store_true",
        help="Use production Zenodo/Dataverse instead of sandbox",
    )
    publish_parser.add_argument(
        "--package-only",
        action="store_true",
        help="Only create the dataset ZIP — skip upload",
    )
    publish_parser.add_argument(
        "--output",
        type=Path,
        default=None,
        help="Output path for the dataset ZIP (default: <dir>/dataset.zip)",
    )
    publish_parser.add_argument(
        "--title",
        default=None,
        help="Dataset title override",
    )
    publish_parser.add_argument(
        "--description",
        default=None,
        help="Dataset description override",
    )

    return parser


def parse_traits(trait_args: list[str] | None) -> dict[str, float]:
    """Parse trait arguments like ['curiosity=0.8', 'caution=0.3']."""
    if not trait_args:
        return {}
    traits: dict[str, float] = {}
    for item in trait_args:
        if "=" in item:
            key, val = item.split("=", 1)
            try:
                traits[key.strip()] = float(val.strip())
            except ValueError as exc:
                logger.error("Invalid trait value for %r: %r (expected number)", key, val)
                raise SystemExit(1) from exc
        else:
            logger.warning("Ignoring malformed trait: %r (expected key=value)", item)
    return traits


def parse_skills(skill_str: str | None) -> dict[str, int]:
    """Parse a comma-separated skill string like 'coding,trading'."""
    if not skill_str:
        return {}
    skills: dict[str, int] = {}
    for name in skill_str.split(","):
        name = name.strip()
        if name:
            skills[name] = 1
    return skills


def _apply_preset_defaults(args: argparse.Namespace) -> None:
    """Fill in LLM args from a provider preset if --preset is given.

    Only sets values that the user did *not* explicitly pass on the command
    line, so ``--llm-model foo --preset zhipu`` keeps the user's model.
    """
    preset_name = getattr(args, "preset", None)
    if not preset_name:
        return

    from agent_runtime.presets import get_provider_preset, list_models_for_provider

    try:
        provider_cfg = get_provider_preset(preset_name)
    except ValueError as exc:
        logger.error("%s", exc)
        raise SystemExit(1) from exc

    # Map preset protocol to CLI --llm-provider choices
    protocol = provider_cfg.get("protocol", "openai")
    # zhipu is a special alias in the CLI
    if preset_name == "zhipu":
        protocol = "zhipu"

    if args.llm_provider is None:
        args.llm_provider = protocol
    if args.llm_base_url is None:
        args.llm_base_url = provider_cfg.get("base_url")
    if args.llm_model is None:
        # Pick the first model listed for this provider
        models = list_models_for_provider(preset_name)
        if models:
            args.llm_model = models[0]["id"]

    logger.info(
        "Applied preset %r: provider=%s base_url=%s model=%s",
        preset_name,
        args.llm_provider,
        args.llm_base_url,
        args.llm_model,
    )


def build_config_from_args(args: argparse.Namespace) -> RuntimeConfig:
    """Build a RuntimeConfig from CLI arguments, optionally merging with a config file."""
    if args.config is not None:
        config = load_runtime_config(args.config)
    else:
        config = RuntimeConfig()

    # CLI overrides for agent — modify config in-place, no redundant copies
    if args.name is not None:
        config.agent.name = args.name
    if args.skills is not None:
        config.agent.skills.update(parse_skills(args.skills))
    if args.traits is not None:
        config.agent.traits.update(parse_traits(args.traits))
    if args.tokens is not None:
        config.agent.tokens = args.tokens
    if args.max_tokens is not None:
        config.agent.max_tokens = args.max_tokens

    # CLI overrides for think loop
    if args.max_ticks is not None:
        config.think_loop.max_ticks = args.max_ticks
    if args.tick_interval is not None:
        config.think_loop.tick_interval = args.tick_interval

    # CLI overrides for world -- support --world-url, top-level --world,
    # and WORLD_ENGINE_URL env var.
    # Priority: CLI flag > env var > config file > default
    world_url = args.world_url or getattr(args, "world", None)
    if world_url is None:
        world_url = os.environ.get("WORLD_ENGINE_URL")
    if world_url is not None:
        config.world.engine_url = world_url

    # Health check port
    if getattr(args, "health_port", None) is not None:
        config.health_port = args.health_port  # type: ignore[attr-defined]

    # Data directory for agent isolation
    data_dir = getattr(args, "data_dir", None)
    if data_dir is None:
        # Fall back to AGENT_DATA_DIR env var (set by pool.py subprocess manager)
        env_data_dir = os.environ.get("AGENT_DATA_DIR")
        if env_data_dir:
            data_dir = Path(env_data_dir)
    if data_dir is not None:
        config.data_dir = Path(data_dir)
        config.data_dir.mkdir(parents=True, exist_ok=True)

    # Apply --preset as defaults (explicit CLI flags override preset values)
    _apply_preset_defaults(args)

    # LLM configuration: CLI args > preset > environment variables > default (Ollama)
    _apply_llm_config(config, args)

    # Mock LLM preset: --mock-llm > MOCK_LLM_PRESET env var
    mock_llm_preset = getattr(args, "mock_llm", None) or os.environ.get("MOCK_LLM_PRESET")
    if mock_llm_preset:
        config.mock_llm_preset = mock_llm_preset
        # Disable real LLM when using mock
        config.llm = None
        logger.info("Using mock LLM preset: %s", mock_llm_preset)

    return config


def _apply_llm_config(config: RuntimeConfig, args: argparse.Namespace) -> None:
    """Apply LLM configuration from CLI args, environment variables, or defaults.

    Priority order (highest wins):
      1. --no-llm flag (disables LLM entirely)
      2. CLI flags (--llm-provider, --llm-model, --llm-base-url)
      3. Environment variables (LLM_PROVIDER, LLM_MODEL, LLM_BASE_URL, OLLAMA_BASE_URL)
      4. Existing config file value
      5. Default: Ollama with glm-4-flash (zero-cost mode)
    """
    # --no-llm explicitly disables LLM
    if getattr(args, "no_llm", False):
        config.llm = None
        logger.info("LLM disabled via --no-llm flag")
        return

    # Determine provider: CLI > env > existing > default(ollama)
    provider_str = (
        args.llm_provider
        or os.environ.get("LLM_PROVIDER")
        or (config.llm.provider.value if config.llm else None)
        or "ollama"
    )

    # Handle provider aliases (zhipu → openai with zhipu base URL)
    zhipu_mode = False
    if provider_str == "zhipu":
        zhipu_mode = True
        provider_str = "openai"

    # Map new protocols to their ProviderType equivalent for backward compat.
    # google/azure use the OpenAI-compatible transport layer for now.
    _new_protocol_map: dict[str, str] = {
        "google": "openai",
        "azure": "openai",
    }
    if provider_str in _new_protocol_map:
        provider_str = _new_protocol_map[provider_str]

    # Determine model: CLI > env > existing > default(glm-4-flash)
    model = (
        args.llm_model
        or os.environ.get("LLM_MODEL")
        or (config.llm.model if config.llm else None)
        or ("glm-5" if zhipu_mode else "glm-4-flash")
    )

    # Determine base_url: CLI > env > existing > provider-specific defaults
    base_url = (
        args.llm_base_url
        or os.environ.get("LLM_BASE_URL")
        or (config.llm.base_url if config.llm else None)
    )
    # Zhipu default base URL
    if base_url is None and zhipu_mode:
        base_url = os.environ.get("ZHIPU_BASE_URL", "https://open.bigmodel.cn/api/paas/v4")
    # Ollama-specific env var fallback
    if base_url is None and provider_str == "ollama":
        base_url = os.environ.get("OLLAMA_BASE_URL")

    # Load API key from environment
    api_key = (
        os.environ.get("LLM_API_KEY")
        or os.environ.get(f"{provider_str.upper()}_API_KEY")
        or (config.llm.api_key if config.llm else None)
    )
    # Zhipu-specific API key env var
    if api_key is None and zhipu_mode:
        api_key = os.environ.get("ZHIPU_API_KEY")

    config.llm = LLMConfig(
        provider=ProviderType(provider_str),
        model=model,
        api_key=api_key,
        base_url=base_url,
        timeout=config.llm.timeout if config.llm else 60.0,
        max_tokens=config.llm.max_tokens if config.llm else 4096,
        temperature=config.llm.temperature if config.llm else None,
    )


# ---------------------------------------------------------------------------
# CLI utility functions
# ---------------------------------------------------------------------------


def _get_health_port(config: RuntimeConfig) -> int:
    """Determine the health check port from env or config default."""
    env_port = os.environ.get("HEALTH_PORT")
    if env_port:
        try:
            return int(env_port)
        except ValueError:
            pass
    return config.health_port


def _extract_grpc_address(engine_url: str) -> str:
    """Convert an HTTP REST URL to a gRPC address (host:port).

    ``http://localhost:8080`` → ``localhost:50051``
    ``https://engine.example.com:443`` → ``engine.example.com:50051``

    The gRPC port can be overridden with the ``GRPC_PORT`` environment variable.
    """
    url = engine_url.replace("https://", "").replace("http://", "")
    # Extract host from REST URL
    if ":" in url:
        host = url.split(":")[0]
    else:
        host = url
    # Use GRPC_PORT env var if set, otherwise default to 50051
    grpc_port = os.environ.get("GRPC_PORT", "50051")
    return f"{host}:{grpc_port}"


def _has_world_arg(argv: list[str]) -> bool:
    """Check if --world or --world-url is present in the argument list."""
    for arg in argv:
        if arg in ("--world", "--world-url"):
            return True
        if arg.startswith("--world=") or arg.startswith("--world-url="):
            return True
    return False


def _rewrite_world_to_world_url(argv: list[str]) -> list[str]:
    """Replace top-level --world with spawn's --world-url for re-parsing."""
    result: list[str] = []
    i = 0
    while i < len(argv):
        if argv[i] == "--world":
            result.append("--world-url")
            i += 1
            if i < len(argv):
                result.append(argv[i])
                i += 1
        elif argv[i].startswith("--world="):
            result.append("--world-url=" + argv[i].split("=", 1)[1])
            i += 1
        else:
            result.append(argv[i])
            i += 1
    return result
