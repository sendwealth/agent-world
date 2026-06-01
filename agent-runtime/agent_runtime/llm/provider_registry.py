"""Dynamic provider registry and model routing.

Implements :class:`ModelRegistry` — a process-level singleton that manages
provider registration, per-agent model overrides, and provider instantiation.

This is the foundation of the T1 multi-model support system (SEN-571).
All subsequent tasks (T3-T8) depend on the ``ProviderConfig`` /
``ModelOption`` / ``ModelRegistry`` abstractions defined here.
"""

from __future__ import annotations

import logging
import threading
from dataclasses import dataclass
from enum import Enum

from .base import LLMConfig, LLMProvider, ProviderType
from .factory import create_provider

logger = logging.getLogger(__name__)

# ---------------------------------------------------------------------------
# Enums
# ---------------------------------------------------------------------------


class ProviderProtocol(str, Enum):
    """Supported LLM wire protocols.

    Each value corresponds to a concrete HTTP chat-completion dialect.
    """

    OPENAI = "openai"
    ANTHROPIC = "anthropic"
    OLLAMA = "ollama"
    GOOGLE = "google"
    AZURE = "azure"


# Backward-compatible alias so that ``ProviderType`` still resolves.
# ``ProviderType`` is also kept in ``base.py`` as its own definition for
# import-path stability; the alias below is a secondary pathway.
ProviderType = ProviderType  # re-export from original module — intentional


# ---------------------------------------------------------------------------
# Data classes
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class ModelOption:
    """A model offered by a registered provider."""

    id: str
    label: str = ""
    provider_id: str = ""

    def __post_init__(self) -> None:
        # Frozen dataclass — use object.__setattr__ for derived defaults.
        if not self.label:
            object.__setattr__(self, "label", self.id)
        if not self.provider_id:
            object.__setattr__(self, "provider_id", self.id.rsplit("/", 1)[0])


@dataclass
class ProviderConfig:
    """Full configuration for a single LLM provider entry in the registry."""

    id: str
    protocol: ProviderProtocol
    base_url: str
    api_key: str | None = None
    api_version: str | None = None
    display_name: str = ""
    is_default: bool = False
    timeout: float = 60.0
    max_tokens: int = 4096

    def __post_init__(self) -> None:
        if not self.display_name:
            self.display_name = self.id


# ---------------------------------------------------------------------------
# Protocol → ProviderType mapping
# ---------------------------------------------------------------------------

# Not every protocol has a concrete LLMProvider subclass yet.
# GOOGLE and AZURE fall back to the OpenAI-compatible transport which
# works for most OpenAI-compatible endpoints (e.g. Azure OpenAI).
_PROTOCOL_TO_PROVIDER_TYPE: dict[ProviderProtocol, ProviderType] = {
    ProviderProtocol.OPENAI: ProviderType.OPENAI,
    ProviderProtocol.ANTHROPIC: ProviderType.ANTHROPIC,
    ProviderProtocol.OLLAMA: ProviderType.OLLAMA,
    # Azure and Google use the OpenAI-compatible transport layer for now.
    ProviderProtocol.GOOGLE: ProviderType.OPENAI,
    ProviderProtocol.AZURE: ProviderType.OPENAI,
}


# ---------------------------------------------------------------------------
# ModelRegistry
# ---------------------------------------------------------------------------


class ModelRegistry:
    """Process-level singleton registry for LLM providers and model routing.

    Thread-safe.  Use :meth:`instance()` to obtain the singleton.
    """

    _instance: ModelRegistry | None = None
    _lock: threading.Lock = threading.Lock()

    def __init__(self) -> None:
        self._providers: dict[str, ProviderConfig] = {}
        self._provider_type_map: dict[str, type[LLMProvider]] = {}
        self._agent_models: dict[str, str] = {}  # agent_id → provider_id
        self._agent_models_version: int = 0  # bumped on every hot_swap
        self._agent_model_overrides: dict[str, tuple[str, str]] = {}  # agent_id → (provider_id, model)
        # Register the three built-in providers so that existing code
        # ``create_provider(LLMConfig(provider=ProviderType.OPENAI, ...))``
        # keeps working out of the box.
        self._register_builtins()

    @classmethod
    def instance(cls) -> ModelRegistry:
        """Return the global singleton, creating it on first call."""
        if cls._instance is None:
            with cls._lock:
                if cls._instance is None:
                    cls._instance = cls()
        return cls._instance

    @classmethod
    def reset(cls) -> None:
        """Reset the singleton (for testing only)."""
        with cls._lock:
            cls._instance = None

    # -- Built-in registrations -------------------------------------------

    def _register_builtins(self) -> None:
        """Register the three providers that ship with the runtime."""
        from .anthropic_provider import AnthropicProvider
        from .ollama_provider import OllamaProvider
        from .openai_provider import OpenAIProvider

        self._provider_type_map = {
            ProviderType.OPENAI.value: OpenAIProvider,
            ProviderType.ANTHROPIC.value: AnthropicProvider,
            ProviderType.OLLAMA.value: OllamaProvider,
        }

        builtins: list[ProviderConfig] = [
            ProviderConfig(
                id="openai",
                protocol=ProviderProtocol.OPENAI,
                base_url="https://api.openai.com/v1",
                display_name="OpenAI",
                is_default=True,
            ),
            ProviderConfig(
                id="anthropic",
                protocol=ProviderProtocol.ANTHROPIC,
                base_url="https://api.anthropic.com/v1",
                display_name="Anthropic",
            ),
            ProviderConfig(
                id="ollama",
                protocol=ProviderProtocol.OLLAMA,
                base_url="http://localhost:11434",
                display_name="Ollama",
            ),
        ]
        for cfg in builtins:
            self._providers[cfg.id] = cfg

    # -- CRUD operations -------------------------------------------------

    def register_provider(self, config: ProviderConfig) -> None:
        """Register (or replace) a provider configuration.

        Args:
            config: Full provider configuration including ``id``, ``protocol``,
                ``base_url``, etc.
        """
        self._providers[config.id] = config
        logger.info("Registered provider %s (protocol=%s)", config.id, config.protocol.value)

    def remove_provider(self, provider_id: str) -> bool:
        """Remove a provider by ID.  Returns ``True`` if it existed."""
        removed = self._providers.pop(provider_id, None) is not None
        if removed:
            # Clean up agent overrides pointing to this provider.
            self._agent_models = {
                aid: pid
                for aid, pid in self._agent_models.items()
                if pid != provider_id
            }
            self._agent_model_overrides = {
                aid: (pid, m)
                for aid, (pid, m) in self._agent_model_overrides.items()
                if pid != provider_id
            }
            logger.info("Removed provider %s", provider_id)
        return removed

    def list_providers(self) -> list[ProviderConfig]:
        """Return all registered provider configurations."""
        return list(self._providers.values())

    def get_provider(self, provider_id: str) -> ProviderConfig | None:
        """Look up a single provider by ID."""
        return self._providers.get(provider_id)

    # -- Provider instantiation ------------------------------------------

    def create_provider(
        self,
        config: ProviderConfig,
        model: str,
        *,
        api_key: str | None = None,
        temperature: float | None = None,
    ) -> LLMProvider:
        """Create an :class:`LLMProvider` from a registry config.

        The provider type is derived from ``config.protocol``.  For protocols
        without a dedicated provider class (GOOGLE, AZURE) the OpenAI-
        compatible transport is used.
        """
        resolved = api_key or config.api_key
        llm_config = LLMConfig(
            provider=self._protocol_to_provider_type(config.protocol),
            model=model,
            api_key=resolved,
            base_url=config.base_url,
            timeout=config.timeout,
            max_tokens=config.max_tokens,
            temperature=temperature,
        )
        return create_provider(llm_config)

    def create_provider_from_config(self, config: ProviderConfig) -> LLMProvider:
        """Create a provider using the config's own defaults for model etc.

        This is a convenience wrapper when the caller already has a fully
        populated :class:`ProviderConfig` and doesn't need to override model
        or API key.
        """
        return self.create_provider(config, model=config.display_name)

    # -- Agent → model routing -------------------------------------------

    def set_agent_model(self, agent_id: str, provider_id: str) -> None:
        """Override the model for a specific agent."""
        self._agent_models[agent_id] = provider_id

    def get_agent_model(self, agent_id: str) -> str | None:
        """Return the provider ID override for an agent, or ``None``."""
        return self._agent_models.get(agent_id)

    def resolve_provider_for_agent(self, agent_id: str) -> ProviderConfig | None:
        """Resolve the effective provider for *agent_id*.

        Order of precedence:
        1. Per-agent override (``set_agent_model``)
        2. The provider marked ``is_default=True``
        3. The first registered provider
        """
        override = self._agent_models.get(agent_id)
        if override and override in self._providers:
            return self._providers[override]

        # Fall back to the default provider.
        for cfg in self._providers.values():
            if cfg.is_default:
                return cfg

        # Last resort: first provider.
        return next(iter(self._providers.values())) if self._providers else None

    # -- Hot-swap support ------------------------------------------------

    def hot_swap_model(self, agent_id: str, provider_id: str, model: str) -> None:
        """Hot-swap the model for a running agent.

        Updates both the agent-to-provider routing and the model override.
        ThinkLoop checks ``get_agent_models_version()`` at the start of each
        tick; if the version changed, it re-creates the LLMProvider.

        Thread-safe: the GIL protects dict assignments for single-key updates.

        Args:
            agent_id: The agent to retarget.
            provider_id: Target provider ID (must be registered).
            model: Model name for the new provider.

        Raises:
            KeyError: If ``provider_id`` is not registered.
        """
        if provider_id not in self._providers:
            raise KeyError(
                f"Provider {provider_id!r} not registered. "
                f"Available: {list(self._providers.keys())}"
            )
        self._agent_models[agent_id] = provider_id
        self._agent_model_overrides[agent_id] = (provider_id, model)
        self._agent_models_version += 1
        logger.info(
            "Model hot-swap: agent=%s provider=%s model=%s (version=%d)",
            agent_id,
            provider_id,
            model,
            self._agent_models_version,
            extra={
                "event": "model_switched",
                "agent": agent_id,
                "provider": provider_id,
                "model": model,
            },
        )

    def get_agent_models_version(self) -> int:
        """Return the current version counter for agent-model overrides.

        Callers compare this value across ticks to detect hot-swaps.
        """
        return self._agent_models_version

    def get_agent_model_override(self, agent_id: str) -> tuple[str, str] | None:
        """Return the (provider_id, model) override for an agent, or None."""
        return self._agent_model_overrides.get(agent_id)

    # -- Internal helpers ------------------------------------------------

    @staticmethod
    def _protocol_to_provider_type(protocol: ProviderProtocol) -> ProviderType:
        """Map a :class:`ProviderProtocol` to a :class:`ProviderType`."""
        pt = _PROTOCOL_TO_PROVIDER_TYPE.get(protocol)
        if pt is None:
            raise ValueError(
                f"No ProviderType mapping for protocol {protocol!r}. "
                f"Supported: {list(_PROTOCOL_TO_PROVIDER_TYPE.keys())}"
            )
        return pt
