"""LLM provider abstraction layer.

Provides a unified interface for calling different LLM providers:
- OpenAI (GPT-4, GPT-3.5, etc.)
- Anthropic (Claude)
- Ollama (local models, zero cost)

Usage::

    from agent_runtime.llm import create_provider, LLMConfig

    config = LLMConfig(provider="openai", model="gpt-4", api_key="sk-...")
    provider = create_provider(config)
    response = await provider.chat([{"role": "user", "content": "Hello!"}])
"""

from agent_runtime.llm.base import (
    LLMConfig,
    LLMError,
    LLMMessage,
    LLMProvider,
    LLMResponse,
    LLMStreamChunk,
    TokenUsage,
)
from agent_runtime.llm.cost import CostTracker
from agent_runtime.llm.factory import create_provider
from agent_runtime.llm.openai_provider import OpenAIProvider
from agent_runtime.llm.anthropic_provider import AnthropicProvider
from agent_runtime.llm.ollama_provider import OllamaProvider

__all__ = [
    "AnthropicProvider",
    "CostTracker",
    "LLMConfig",
    "LLMError",
    "LLMMessage",
    "LLMProvider",
    "LLMResponse",
    "LLMStreamChunk",
    "OllamaProvider",
    "OpenAIProvider",
    "TokenUsage",
    "create_provider",
]
