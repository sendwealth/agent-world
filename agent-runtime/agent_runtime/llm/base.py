"""Base types and abstract interface for LLM providers."""

from __future__ import annotations

from abc import ABC, abstractmethod
from collections.abc import AsyncIterator
from dataclasses import dataclass, field
from enum import StrEnum

import httpx

# ---------------------------------------------------------------------------
# Domain exceptions
# ---------------------------------------------------------------------------


class LLMError(Exception):
    """Domain-level exception for LLM provider failures.

    Wraps underlying HTTP or transport errors with a stable public API.
    """

    def __init__(self, message: str, *, provider: str, model: str) -> None:
        super().__init__(message)
        self.provider = provider
        self.model = model


# ---------------------------------------------------------------------------
# Data types
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class LLMMessage:
    """A single message in a conversation."""

    role: str  # "system", "user", "assistant"
    content: str


@dataclass(frozen=True)
class TokenUsage:
    """Token usage statistics returned by the provider."""

    prompt_tokens: int = 0
    completion_tokens: int = 0
    total_tokens: int = 0


@dataclass(frozen=True)
class LLMResponse:
    """Complete (non-streaming) response from an LLM provider."""

    content: str
    model: str
    usage: TokenUsage = field(default_factory=TokenUsage)
    finish_reason: str | None = None


@dataclass(frozen=True)
class LLMStreamChunk:
    """A single chunk from a streaming response."""

    content: str
    model: str
    finish_reason: str | None = None


class ProviderType(StrEnum):
    """Supported LLM provider types."""

    OPENAI = "openai"
    ANTHROPIC = "anthropic"
    OLLAMA = "ollama"


@dataclass
class LLMConfig:
    """Configuration for instantiating an LLM provider.

    Drives provider selection via ``create_provider(config)``.
    """

    provider: ProviderType
    model: str
    api_key: str | None = None
    base_url: str | None = None
    timeout: float = 60.0
    max_tokens: int = 4096
    temperature: float | None = None


# ---------------------------------------------------------------------------
# Abstract base class
# ---------------------------------------------------------------------------


class LLMProvider(ABC):
    """Abstract base class for all LLM providers.

    Subclasses must implement:
    - ``chat`` for complete (non-streaming) responses
    - ``chat_stream`` for streaming responses
    """

    def __init__(self, config: LLMConfig) -> None:
        self._config = config
        self._client = httpx.AsyncClient(
            timeout=httpx.Timeout(config.timeout, connect=10.0),
        )

    @property
    def config(self) -> LLMConfig:
        return self._config

    @abstractmethod
    async def chat(
        self,
        messages: list[LLMMessage],
        *,
        max_tokens: int | None = None,
        temperature: float | None = None,
    ) -> LLMResponse:
        """Send messages and return a complete response."""
        ...

    @abstractmethod
    def chat_stream(
        self,
        messages: list[LLMMessage],
        *,
        max_tokens: int | None = None,
        temperature: float | None = None,
    ) -> AsyncIterator[LLMStreamChunk]:
        """Send messages and yield response chunks."""
        ...

    async def close(self) -> None:
        """Close the underlying HTTP client."""
        await self._client.aclose()

    async def __aenter__(self) -> LLMProvider:
        return self

    async def __aexit__(self, *exc: object) -> None:
        await self.close()
