"""Factory function for config-driven provider selection."""

from __future__ import annotations

from .anthropic_provider import AnthropicProvider
from .base import LLMConfig, LLMProvider, ProviderType
from .ollama_provider import OllamaProvider
from .openai_provider import OpenAIProvider

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
