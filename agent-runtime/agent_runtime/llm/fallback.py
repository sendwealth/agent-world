"""Fallback chain provider — tries primary LLM, falls back to alternatives.

Implements :class:`ModelFallback`, a wrapper around :class:`LLMProvider`
that tries a primary provider first and, on failure, walks through an
ordered list of fallback providers.  If all providers fail, the caller
receives the last exception (the decision engine will then use a random
fallback).

This module also provides :class:`FallbackChainProvider`, an ``LLMProvider``
subclass that transparently wraps the fallback chain so it can be used
anywhere a single ``LLMProvider`` is expected.
"""

from __future__ import annotations

import logging
from dataclasses import dataclass, field
from typing import AsyncIterator

from .base import (
    LLMError,
    LLMMessage,
    LLMProvider,
    LLMResponse,
    LLMStreamChunk,
)

logger = logging.getLogger(__name__)


# ---------------------------------------------------------------------------
# ModelFallback — ordered provider list
# ---------------------------------------------------------------------------


@dataclass
class ModelFallback:
    """Ordered fallback chain for LLM providers.

    ``primary`` is tried first.  On any exception (``LLMError`` or
    otherwise), each entry in ``fallbacks`` is tried in order.  If all
    fail, ``None`` is returned and the caller should use a random decision.
    """

    primary: LLMProvider
    fallbacks: list[LLMProvider] = field(default_factory=list)

    @property
    def all_providers(self) -> list[LLMProvider]:
        """Return the full chain: primary + fallbacks."""
        return [self.primary] + list(self.fallbacks)


# ---------------------------------------------------------------------------
# FallbackChainProvider — LLMProvider-compatible wrapper
# ---------------------------------------------------------------------------


class FallbackChainProvider(LLMProvider):
    """LLMProvider that tries providers in order with fallback.

    Can be used anywhere a single ``LLMProvider`` is expected:
    ``DecisionEngine(provider=fallback_provider)``.

    On ``chat()``, tries each provider in the chain.  Returns the first
    successful response.  If all fail, raises the last ``LLMError``
    (or wraps a generic exception in ``LLMError``).

    Logging:
      - Emits ``Fallback triggered`` at WARNING level when falling back.
      - Emits ``All providers failed`` at ERROR level when the chain is exhausted.
    """

    def __init__(self, chain: ModelFallback) -> None:
        # LLMProvider.__init__ expects an LLMConfig; use the primary's config
        super().__init__(chain.primary._config)
        self._chain = chain

    @property
    def chain(self) -> ModelFallback:
        """The underlying fallback chain."""
        return self._chain

    async def chat(
        self,
        messages: list[LLMMessage],
        *,
        max_tokens: int | None = None,
        temperature: float | None = None,
    ) -> LLMResponse:
        """Try each provider in the chain, return the first success."""
        last_error: Exception | None = None

        for i, provider in enumerate(self._chain.all_providers):
            provider_label = (
                "primary"
                if provider is self._chain.primary
                else f"fallback[{i - 1}]"
            )
            try:
                response = await provider.chat(
                    messages,
                    max_tokens=max_tokens,
                    temperature=temperature,
                )
                if i > 0:
                    logger.info(
                        "Fallback succeeded: %s → %s (provider=%s, model=%s)",
                        provider_label,
                        "success",
                        provider._config.provider.value,
                        provider._config.model,
                        extra={
                            "event": "fallback_triggered",
                            "fallback_index": i - 1,
                            "provider": provider._config.provider.value,
                            "model": provider._config.model,
                        },
                    )
                return response
            except Exception as exc:
                last_error = exc
                if i < len(self._chain.all_providers) - 1:
                    next_label = (
                        "fallback[0]"
                        if i == 0
                        else f"fallback[{i}]"
                    )
                    logger.warning(
                        "Fallback triggered: %s failed (%s: %s), trying %s",
                        provider_label,
                        type(exc).__name__,
                        exc,
                        next_label,
                        extra={
                            "event": "fallback_triggered",
                            "failed_provider": provider._config.provider.value,
                            "failed_model": provider._config.model,
                        },
                    )
                else:
                    logger.error(
                        "All providers failed in fallback chain "
                        "(primary + %d fallbacks): %s: %s",
                        len(self._chain.fallbacks),
                        type(exc).__name__,
                        exc,
                        extra={
                            "event": "fallback_exhausted",
                            "total_providers": len(self._chain.all_providers),
                        },
                    )

        # All providers failed — raise the last error
        if last_error is not None:
            if isinstance(last_error, LLMError):
                raise last_error
            # Wrap in LLMError for uniform handling upstream
            raise LLMError(
                f"All providers failed: {last_error}",
                provider="fallback_chain",
                model="unknown",
            ) from last_error

        # Should not reach here, but guard anyway
        raise LLMError(
            "No providers in fallback chain",
            provider="fallback_chain",
            model="unknown",
        )

    def chat_stream(
        self,
        messages: list[LLMMessage],
        *,
        max_tokens: int | None = None,
        temperature: float | None = None,
    ) -> AsyncIterator[LLMStreamChunk]:
        """Streaming not supported in fallback chain — use chat() instead."""
        raise NotImplementedError(
            "FallbackChainProvider does not support streaming. "
            "Use chat() for non-streaming responses."
        )

    async def close(self) -> None:
        """Close all providers in the chain."""
        for provider in self._chain.all_providers:
            try:
                await provider.close()
            except Exception:
                logger.debug(
                    "Error closing provider %s (non-fatal)",
                    provider._config.provider.value,
                    exc_info=True,
                )
