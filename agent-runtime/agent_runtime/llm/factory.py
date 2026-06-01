"""Factory function for config-driven provider selection."""

from __future__ import annotations

from typing import TYPE_CHECKING

from .anthropic_provider import AnthropicProvider
from .base import LLMConfig, LLMProvider, ProviderType
from .ollama_provider import OllamaProvider
from .openai_provider import OpenAIProvider

if TYPE_CHECKING:
    from .provider_registry import ProviderConfig as RegistryProviderConfig

_PROVIDERS: dict[ProviderType, type[LLMProvider]] = {
    ProviderType.OPENAI: OpenAIProvider,
    ProviderType.ANTHROPIC: AnthropicProvider,
    ProviderType.OLLAMA: OllamaProvider,
}


def create_provider(config: LLMConfig) -> LLMProvider:
    """Create an LLM provider from a configuration object.

    The ``config.provider`` field determines which provider is instantiated:

    - ``ProviderType.OPENAI`` → :class:`OpenAIProvider`
    - ``ProviderType.ANTHROPIC`` → :class:`AnthropicProvider`
    - ``ProviderType.OLLAMA`` → :class:`OllamaProvider`

    Raises:
        ValueError: If the provider type is not recognized.
    """
    provider_cls = _PROVIDERS.get(config.provider)
    if provider_cls is None:
        raise ValueError(
            f"Unknown provider: {config.provider!r}. Supported: {list(_PROVIDERS.keys())}"
        )
    return provider_cls(config)


def create_provider_from_config(config: RegistryProviderConfig) -> LLMProvider:
    """Create an LLM provider from a :class:`ProviderConfig` (registry entry).

    Converts the registry-level ``ProviderConfig`` into an ``LLMConfig`` and
    delegates to :func:`create_provider`.  This is the preferred entry point
    when working with the new provider registry (T1 / SEN-571).
    """
    from .provider_registry import _PROTOCOL_TO_PROVIDER_TYPE

    provider_type = _PROTOCOL_TO_PROVIDER_TYPE.get(config.protocol)
    if provider_type is None:
        raise ValueError(
            f"No ProviderType mapping for protocol {config.protocol!r}. "
            f"Supported: {list(_PROTOCOL_TO_PROVIDER_TYPE.keys())}"
        )
    llm_config = LLMConfig(
        provider=provider_type,
        model=config.display_name,
        api_key=config.api_key,
        base_url=config.base_url,
        timeout=config.timeout,
        max_tokens=config.max_tokens,
    )
    return create_provider(llm_config)
