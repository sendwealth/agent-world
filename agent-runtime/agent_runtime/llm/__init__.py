"""LLM provider abstraction layer.

Provides a unified interface for calling different LLM providers:
- OpenAI (GPT-4, GPT-3.5, etc.)
- Anthropic (Claude)
- Ollama (local models, zero cost)

Also provides async request queueing and prompt template system for
multi-agent LLM-driven decision making.

Usage::

    from agent_runtime.llm import create_provider, LLMConfig

    config = LLMConfig(provider="openai", model="gpt-4", api_key="sk-...")
    provider = create_provider(config)
    response = await provider.chat([{"role": "user", "content": "Hello!"}])
"""

from agent_runtime.llm.anthropic_provider import AnthropicProvider
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
from agent_runtime.llm.decision_log import DecisionLog, DecisionLogStore
from agent_runtime.llm.factory import create_provider, create_provider_from_config
from agent_runtime.llm.ollama_provider import (
    OllamaHealthStatus,
    OllamaModelInfo,
    OllamaProvider,
)
from agent_runtime.llm.openai_provider import OpenAIProvider
from agent_runtime.llm.prompts import (
    DEFAULT_TEMPLATE,
    SURVIVAL_TEMPLATE,
    PromptTemplate,
    get_template,
    register_template,
)
from agent_runtime.llm.provider_registry import (
    ModelOption,
    ModelRegistry,
    ProviderConfig,
    ProviderProtocol,
)
from agent_runtime.llm.queue import (
    LLMQueue,
    LLMRequest,
    Priority,
    QueueConfig,
    QueueStats,
)
from agent_runtime.llm.rate_limiter import RateLimiter, default_rate_limiter

__all__ = [
    "AnthropicProvider",
    "CostTracker",
    "DecisionLog",
    "DecisionLogStore",
    "DEFAULT_TEMPLATE",
    "LLMConfig",
    "LLMError",
    "LLMMessage",
    "LLMProvider",
    "LLMQueue",
    "LLMRequest",
    "LLMResponse",
    "LLMStreamChunk",
    "ModelOption",
    "ModelRegistry",
    "OllamaHealthStatus",
    "OllamaModelInfo",
    "OllamaProvider",
    "OpenAIProvider",
    "Priority",
    "PromptTemplate",
    "ProviderConfig",
    "ProviderProtocol",
    "QueueConfig",
    "QueueStats",
    "RateLimiter",
    "SURVIVAL_TEMPLATE",
    "TokenUsage",
    "create_provider",
    "create_provider_from_config",
    "default_rate_limiter",
    "get_template",
    "register_template",
]
