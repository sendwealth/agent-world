"""Agent runtime configuration — TOML / YAML config file support.

Loads agent configuration from a TOML or YAML file and merges it with
CLI overrides.  The config drives agent spawning (name, traits, skills,
LLM provider settings, think-loop parameters, and world-engine connection).

Supported file formats (auto-detected by extension):
    - ``.toml``  → parsed with ``tomllib`` (stdlib, Python 3.11+)
    - ``.yaml`` / ``.yml`` → parsed with ``pyyaml``

Config file example (TOML)::

    [agent]
    name = "Alice"
    traits = { curiosity = 0.8, caution = 0.3 }

    [agent.skills]
    coding = { level = 3 }
    trading = { level = 1 }

    [llm]
    provider = "ollama"
    model = "minicpm5:1b"
    base_url = "http://localhost:11434"

    [think_loop]
    tick_interval = 1.0
    max_ticks = 0
    reflect_interval = 10

    [world]
    engine_url = "http://localhost:3000"

Config file example (YAML)::

    agent:
      name: Alice
      traits:
        curiosity: 0.8
        caution: 0.3
      skills:
        coding:
          level: 3
        trading:
          level: 1
    llm:
      provider: ollama
      model: minicpm5:1b
      base_url: http://localhost:11434
    think_loop:
      tick_interval: 1.0
      max_ticks: 0
      reflect_interval: 10
    world:
      engine_url: http://localhost:3000
"""

from __future__ import annotations

import logging
import os
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

import yaml

from agent_runtime import DEFAULT_INITIAL_TOKENS, DEFAULT_MAX_TOKENS
from agent_runtime.core.think_loop import ThinkLoopConfig
from agent_runtime.llm.base import LLMConfig, ProviderType
from agent_runtime.llm.queue import QueueConfig as LLMQueueConfig

logger = logging.getLogger(__name__)

# Default World Engine URL (single source of truth)
# Can be overridden via WORLD_ENGINE_URL env var or --world-url CLI flag.
_DEFAULT_ENGINE_URL = "http://localhost:3000"


def _resolve_engine_url() -> str:
    """Resolve the World Engine URL from environment or default.

    Priority:
      1. ``WORLD_ENGINE_URL`` env var
      2. ``_DEFAULT_ENGINE_URL`` (``http://localhost:3000``)
    """
    return os.environ.get("WORLD_ENGINE_URL", _DEFAULT_ENGINE_URL)


# ---------------------------------------------------------------------------
# Data classes
# ---------------------------------------------------------------------------


@dataclass
class WorldConfig:
    """Connection settings for the World Engine.

    ``engine_url`` defaults to the ``WORLD_ENGINE_URL`` environment variable
    when set, otherwise ``http://localhost:3000``.
    """

    engine_url: str = field(default_factory=_resolve_engine_url)


@dataclass
class IdentityConfig:
    """Rich identity information for an agent.

    Attributes:
        display_name: Human-friendly name (may differ from internal name).
        bio: Short biography (1-2 sentences).
        backstory: Extended backstory for LLM prompt injection.
        alignment: Moral alignment (e.g. "lawful-good", "chaotic-neutral").
        archetype: Character archetype (e.g. "explorer", "merchant", "scholar").
        mbti: MBTI personality type (e.g. "INTJ", "ENFP").
    """

    display_name: str = ""
    bio: str = ""
    backstory: str = ""
    alignment: str = ""
    archetype: str = ""
    mbti: str = ""


@dataclass
class PersonalityConfig:
    """Big Five personality dimensions plus survival specialization.

    All values are floats in [0, 1].
    """

    openness: float = 0.5
    conscientiousness: float = 0.5
    extraversion: float = 0.5
    agreeableness: float = 0.5
    neuroticism: float = 0.5
    risk_tolerance: float = 0.5
    social_orientation: float = 0.5
    greed: float = 0.5


@dataclass
class ValuesConfig:
    """Value weights that influence agent decision-making priorities.

    All values are floats in [0, 1].
    """

    survival: float = 0.5
    knowledge: float = 0.5
    wealth: float = 0.5
    social: float = 0.5
    freedom: float = 0.5
    power: float = 0.5


@dataclass
class PreferencesConfig:
    """Agent behavioral preferences."""

    preferred_actions: list[str] = field(default_factory=list)
    avoided_actions: list[str] = field(default_factory=list)
    social_style: str = ""
    communication_style: str = ""


@dataclass
class AgentSpawnConfig:
    """Configuration for spawning a single agent.

    Attributes:
        name: Agent display name.
        traits: Personality trait scores (e.g. {"curiosity": 0.8}).
        skills: Skill names with optional starting levels (e.g. {"coding": 3}).
        tokens: Initial token balance.
        max_tokens: Maximum token capacity.
        money: Initial money balance.
        health: Initial health (0-100).
        identity: Rich identity information (backstory, alignment, etc.).
        personality: Big Five + survival personality vector.
        values: Value weights for decision-making.
        preferences: Behavioral preferences.
        questions: Personalized Q&A pairs for agent characterization.
    """

    name: str = "Agent"
    traits: dict[str, float] = field(default_factory=dict)
    skills: dict[str, int] = field(default_factory=dict)
    tokens: int = DEFAULT_INITIAL_TOKENS
    max_tokens: int = DEFAULT_MAX_TOKENS
    money: float = 50.0
    health: float = 100.0
    identity: IdentityConfig = field(default_factory=IdentityConfig)
    personality: PersonalityConfig = field(default_factory=PersonalityConfig)
    values: ValuesConfig = field(default_factory=ValuesConfig)
    preferences: PreferencesConfig = field(default_factory=PreferencesConfig)
    questions: list[dict[str, str]] = field(default_factory=list)


@dataclass
class RuntimeConfig:
    """Top-level configuration for the agent runtime.

    Aggregates all sub-configs needed to spawn and run an agent.
    """

    agent: AgentSpawnConfig = field(default_factory=AgentSpawnConfig)
    llm: LLMConfig | None = None
    llm_queue: LLMQueueConfig = field(default_factory=LLMQueueConfig)
    think_loop: ThinkLoopConfig = field(default_factory=ThinkLoopConfig)
    world: WorldConfig = field(default_factory=WorldConfig)
    health_port: int = 9090
    mock_llm_preset: str | None = None
    data_dir: Path | None = None


# ---------------------------------------------------------------------------
# Config file loading
# ---------------------------------------------------------------------------


def load_config_file(path: Path) -> dict[str, Any]:
    """Load a TOML or YAML config file and return the parsed dict.

    Raises:
        FileNotFoundError: If the file does not exist.
        ValueError: If the file extension is not .toml, .yaml, or .yml.
    """
    if not path.exists():
        raise FileNotFoundError(f"Config file not found: {path}")

    suffix = path.suffix.lower()
    if suffix == ".toml":
        try:
            import tomllib
        except ModuleNotFoundError:
            try:
                import tomli as tomllib  # type: ignore[no-redef]
            except ModuleNotFoundError:
                raise ImportError(
                    "TOML support requires Python 3.11+ (tomllib) or the 'tomli' package"
                )
        with open(path, "rb") as f:
            return tomllib.load(f)
    elif suffix in (".yaml", ".yml"):
        with open(path) as f:
            data = yaml.safe_load(f)
            return data if isinstance(data, dict) else {}
    else:
        raise ValueError(
            f"Unsupported config file extension: {suffix!r}. "
            f"Use .toml, .yaml, or .yml"
        )


def _parse_llm_config(data: dict[str, Any]) -> LLMConfig | None:
    """Parse the ``[llm]`` section into an LLMConfig.

    The API key is loaded from the environment variable ``LLM_API_KEY``
    (or ``OPENAI_API_KEY`` / ``ANTHROPIC_API_KEY`` as provider-specific
    fallbacks). Config file ``api_key`` entries are ignored to prevent
    accidental plaintext secret exposure.
    """
    if not data:
        return None

    provider_str = data.get("provider", "ollama").lower()

    # Map new protocols to their ProviderType equivalent.
    _new_protocol_map: dict[str, str] = {
        "google": "openai",
        "azure": "openai",
        "zhipu": "openai",
    }
    mapped = _new_protocol_map.get(provider_str)
    effective_str = mapped if mapped else provider_str

    try:
        provider = ProviderType(effective_str)
    except ValueError:
        valid = ", ".join(p.value for p in ProviderType)
        raise ValueError(
            f"Unknown LLM provider {provider_str!r} in config. Valid options: {valid}"
        )

    # Load API key from environment, never from config file
    api_key = (
        os.environ.get("LLM_API_KEY")
        or os.environ.get(f"{provider_str.upper()}_API_KEY")
        or None
    )

    return LLMConfig(
        provider=provider,
        model=data.get("model", "minicpm5:1b"),
        api_key=api_key,
        base_url=data.get("base_url"),
        timeout=data.get("timeout", 60.0),
        max_tokens=data.get("max_tokens", 4096),
        temperature=data.get("temperature"),
    )


def _parse_think_loop_config(data: dict[str, Any]) -> ThinkLoopConfig:
    """Parse the ``[think_loop]`` section into a ThinkLoopConfig.

    Default tick_interval is raised from 1.0 to 5.0 seconds to stay within
    typical LLM gateway rate limits (e.g. GLM-4-Flash ~2-3 req/s).  Agents
    that explicitly set ``tick_interval`` in their config file keep their
    configured value.
    """
    return ThinkLoopConfig(
        tick_interval=data.get("tick_interval", 5.0),
        max_ticks=data.get("max_ticks", 0),
        reflect_interval=data.get("reflect_interval", 10),
        error_backoff=data.get("error_backoff", 5.0),
        max_consecutive_errors=data.get("max_consecutive_errors", 0),
    )


def _parse_world_config(data: dict[str, Any]) -> WorldConfig:
    """Parse the ``[world]`` section.

    Uses ``WORLD_ENGINE_URL`` env var as fallback when ``engine_url``
    is not specified in the config file.
    """
    return WorldConfig(
        engine_url=data.get("engine_url", _resolve_engine_url()),
    )


def _parse_llm_queue_config(data: dict[str, Any]) -> LLMQueueConfig:
    """Parse the ``[llm_queue]`` section into a QueueConfig."""
    return LLMQueueConfig(
        max_concurrency=data.get("max_concurrency", 2),
        timeout_seconds=data.get("timeout_seconds", 120.0),
    )


def _parse_identity_config(data: dict[str, Any]) -> IdentityConfig:
    """Parse the ``[agent.identity]`` section."""
    if not data:
        return IdentityConfig()
    return IdentityConfig(
        display_name=data.get("display_name", ""),
        bio=data.get("bio", ""),
        backstory=data.get("backstory", ""),
        alignment=data.get("alignment", ""),
        archetype=data.get("archetype", ""),
        mbti=data.get("mbti", ""),
    )


def _parse_personality_config(data: dict[str, Any]) -> PersonalityConfig:
    """Parse the ``[agent.personality]`` section (Big Five + survival)."""
    if not data:
        return PersonalityConfig()
    return PersonalityConfig(
        openness=data.get("openness", 0.5),
        conscientiousness=data.get("conscientiousness", 0.5),
        extraversion=data.get("extraversion", 0.5),
        agreeableness=data.get("agreeableness", 0.5),
        neuroticism=data.get("neuroticism", 0.5),
        risk_tolerance=data.get("risk_tolerance", 0.5),
        social_orientation=data.get("social_orientation", 0.5),
        greed=data.get("greed", 0.5),
    )


def _parse_values_config(data: dict[str, Any]) -> ValuesConfig:
    """Parse the ``[agent.values]`` section."""
    if not data:
        return ValuesConfig()
    return ValuesConfig(
        survival=data.get("survival", 0.5),
        knowledge=data.get("knowledge", 0.5),
        wealth=data.get("wealth", 0.5),
        social=data.get("social", 0.5),
        freedom=data.get("freedom", 0.5),
        power=data.get("power", 0.5),
    )


def _parse_preferences_config(data: dict[str, Any]) -> PreferencesConfig:
    """Parse the ``[agent.preferences]`` section."""
    if not data:
        return PreferencesConfig()
    return PreferencesConfig(
        preferred_actions=data.get("preferred_actions", []),
        avoided_actions=data.get("avoided_actions", []),
        social_style=data.get("social_style", ""),
        communication_style=data.get("communication_style", ""),
    )


def _parse_questions(data: list[Any]) -> list[dict[str, str]]:
    """Parse the ``[[agent.questions]]`` array of tables."""
    if not data:
        return []
    result: list[dict[str, str]] = []
    for item in data:
        if isinstance(item, dict) and "question" in item:
            result.append({
                "question": str(item["question"]),
                "answer": str(item.get("answer", "")),
            })
    return result


def parse_runtime_config(raw: dict[str, Any]) -> RuntimeConfig:
    """Parse a raw dict (from config file) into a RuntimeConfig.

    Supports both TOML and YAML key structures.  New identity, personality,
    values, preferences, and questions sections are fully optional — configs
    without them work unchanged (backward compatible).
    """
    agent_raw = raw.get("agent", {})
    skills_raw = agent_raw.get("skills", {})

    # Parse skills: accept {"coding": {"level": 3}} or {"coding": 3} or {"coding": 3.0}
    skills: dict[str, int] = {}
    for name, val in skills_raw.items():
        if isinstance(val, dict):
            skills[name] = val.get("level", 1)
        elif isinstance(val, int):
            skills[name] = val
        elif isinstance(val, float):
            skills[name] = int(val)
        elif isinstance(val, str):
            try:
                skills[name] = int(val)
            except ValueError:
                logger.warning(
                    "Ignoring non-numeric skill level for %r: %r", name, val
                )

    # Parse extended identity sections (all optional)
    identity_cfg = _parse_identity_config(agent_raw.get("identity", {}))
    personality_cfg = _parse_personality_config(agent_raw.get("personality", {}))
    values_cfg = _parse_values_config(agent_raw.get("values", {}))
    preferences_cfg = _parse_preferences_config(agent_raw.get("preferences", {}))
    questions_cfg = _parse_questions(agent_raw.get("questions", []))

    agent_cfg = AgentSpawnConfig(
        name=agent_raw.get("name", "Agent"),
        traits=agent_raw.get("traits", {}),
        skills=skills,
        tokens=agent_raw.get("tokens", DEFAULT_INITIAL_TOKENS),
        max_tokens=agent_raw.get("max_tokens", DEFAULT_MAX_TOKENS),
        money=agent_raw.get("money", 50.0),
        health=agent_raw.get("health", 100.0),
        identity=identity_cfg,
        personality=personality_cfg,
        values=values_cfg,
        preferences=preferences_cfg,
        questions=questions_cfg,
    )

    llm_cfg = _parse_llm_config(raw.get("llm", {}))
    llm_queue_cfg = _parse_llm_queue_config(raw.get("llm_queue", {}))
    think_loop_cfg = _parse_think_loop_config(raw.get("think_loop", {}))
    world_cfg = _parse_world_config(raw.get("world", {}))

    return RuntimeConfig(
        agent=agent_cfg,
        llm=llm_cfg,
        llm_queue=llm_queue_cfg,
        think_loop=think_loop_cfg,
        world=world_cfg,
    )


def load_runtime_config(path: Path) -> RuntimeConfig:
    """Load a config file and parse it into a RuntimeConfig.

    This is the main entry point: give it a path, get back a fully
    validated RuntimeConfig.
    """
    raw = load_config_file(path)
    config = parse_runtime_config(raw)
    logger.info("Loaded config from %s: agent=%s", path, config.agent.name)
    return config
