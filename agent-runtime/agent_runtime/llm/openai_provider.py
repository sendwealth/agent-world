"""OpenAI LLM provider (GPT-4, GPT-3.5, etc.).

Uses the OpenAI Chat Completions API via httpx.
"""

from __future__ import annotations

import asyncio
import json
import logging
import os
from collections.abc import AsyncIterator

import httpx

from .base import (
    LLMConfig,
    LLMError,
    LLMMessage,
    LLMProvider,
    LLMResponse,
    LLMStreamChunk,
    TokenUsage,
)

logger = logging.getLogger(__name__)

_DEFAULT_BASE_URL = "https://api.openai.com/v1"

# Rate-limit retry configuration (readable via environment variables)
_RATE_LIMIT_MAX_RETRIES = int(os.environ.get("LLM_MAX_RETRIES", "3"))
_RATE_LIMIT_MIN_BACKOFF = float(os.environ.get("LLM_MIN_BACKOFF_SECONDS", "1.0"))
_RATE_LIMIT_MAX_BACKOFF = float(os.environ.get("LLM_MAX_BACKOFF_SECONDS", "60.0"))
_RATE_LIMIT_JITTER_FACTOR = float(os.environ.get("LLM_JITTER_FACTOR", "0.25"))


class OpenAIProvider(LLMProvider):
    """OpenAI Chat Completions provider.

    Expects ``config.api_key`` to be set.

    Handles 429 Too Many Requests with exponential-backoff retry so
    that transient rate-limit errors from the LLM gateway do not crash
    an agent tick.
    """

    def __init__(self, config: LLMConfig) -> None:
        super().__init__(config)
        self._base_url = (config.base_url or _DEFAULT_BASE_URL).rstrip("/")
        # Per-provider retry budget (allow overrides per-provider for mixed setups)
        self._max_retries = _RATE_LIMIT_MAX_RETRIES

    # ------------------------------------------------------------------
    # chat (non-streaming)
    # ------------------------------------------------------------------

    async def chat(
        self,
        messages: list[LLMMessage],
        *,
        max_tokens: int | None = None,
        temperature: float | None = None,
    ) -> LLMResponse:
        payload = self._build_payload(
            messages,
            stream=False,
            max_tokens=max_tokens,
            temperature=temperature,
        )
        last_exc: Exception | None = None
        for attempt in range(self._max_retries + 1):
            try:
                resp = await self._client.post(
                    f"{self._base_url}/chat/completions",
                    headers=self._headers(),
                    json=payload,
                )
                if resp.status_code == 429:
                    delay = self._backoff_delay(attempt)
                    logger.warning(
                        "Agent %s: rate-limited (429) on attempt %d/%d — "
                        "retrying in %.1fs",
                        self._config.model,
                        attempt + 1,
                        self._max_retries + 1,
                        delay,
                    )
                    await asyncio.sleep(delay)
                    continue
                resp.raise_for_status()
                data = resp.json()
                return self._parse_response(data)
            except httpx.HTTPError as exc:
                last_exc = exc
                break  # non-429 HTTP error — do not retry
            except Exception as exc:
                last_exc = exc
                break  # unexpected error — do not retry

        raise LLMError(
            f"OpenAI request failed after {self._max_retries + 1} attempts: {last_exc}",
            provider="openai",
            model=self._config.model,
        ) from last_exc

    # ------------------------------------------------------------------
    # chat_stream (streaming)
    # ------------------------------------------------------------------

    async def chat_stream(
        self,
        messages: list[LLMMessage],
        *,
        max_tokens: int | None = None,
        temperature: float | None = None,
    ) -> AsyncIterator[LLMStreamChunk]:
        payload = self._build_payload(
            messages,
            stream=True,
            max_tokens=max_tokens,
            temperature=temperature,
        )
        for attempt in range(self._max_retries + 1):
            try:
                async with self._client.stream(
                    "POST",
                    f"{self._base_url}/chat/completions",
                    headers=self._headers(),
                    json=payload,
                ) as resp:
                    if resp.status_code == 429:
                        delay = self._backoff_delay(attempt)
                        logger.warning(
                            "Agent %s: rate-limited (429) on stream attempt %d/%d — "
                            "retrying in %.1fs",
                            self._config.model,
                            attempt + 1,
                            self._max_retries + 1,
                            delay,
                        )
                        await asyncio.sleep(delay)
                        continue
                    resp.raise_for_status()
                    async for line in resp.aiter_lines():
                        chunk = self._parse_sse_line(line)
                        if chunk is not None:
                            yield chunk
                return  # stream completed successfully
            except httpx.HTTPError:
                if attempt < self._max_retries:
                    continue  # will retry
                raise

    # ------------------------------------------------------------------
    # Rate-limit retry
    # ------------------------------------------------------------------

    def _backoff_delay(self, attempt: int) -> float:
        """Compute exponential backoff delay with jitter for a given retry attempt."""
        import random

        exponential = _RATE_LIMIT_MIN_BACKOFF * (2 ** attempt)
        clamped = min(exponential, _RATE_LIMIT_MAX_BACKOFF)
        # Add jitter so concurrent agents don't thunder-peck together
        jitter = clamped * _RATE_LIMIT_JITTER_FACTOR * random.uniform(-1, 1)
        return max(0.0, jitter)

    # ------------------------------------------------------------------
    # Internal helpers
    # ------------------------------------------------------------------

    def _headers(self) -> dict[str, str]:
        headers: dict[str, str] = {
            "Content-Type": "application/json",
        }
        if self._config.api_key:
            headers["Authorization"] = f"Bearer {self._config.api_key}"
        return headers

    def _build_payload(
        self,
        messages: list[LLMMessage],
        *,
        stream: bool = False,
        max_tokens: int | None = None,
        temperature: float | None = None,
    ) -> dict:
        return {
            "model": self._config.model,
            "messages": [{"role": m.role, "content": m.content} for m in messages],
            "max_tokens": max_tokens if max_tokens is not None else self._config.max_tokens,
            "temperature": temperature
            if temperature is not None
            else (self._config.temperature if self._config.temperature is not None else 0.7),
            "stream": stream,
        }

    @staticmethod
    def _parse_response(data: dict) -> LLMResponse:
        choice = data["choices"][0]
        usage_data = data.get("usage", {})
        usage = TokenUsage(
            prompt_tokens=usage_data.get("prompt_tokens", 0),
            completion_tokens=usage_data.get("completion_tokens", 0),
            total_tokens=usage_data.get("total_tokens", 0),
        )
        return LLMResponse(
            content=choice["message"]["content"],
            model=data["model"],
            usage=usage,
            finish_reason=choice.get("finish_reason"),
        )

    @staticmethod
    def _parse_sse_line(line: str) -> LLMStreamChunk | None:
        """Parse a single SSE line from the OpenAI streaming response."""
        if not line.startswith("data: "):
            return None
        payload = line[len("data: ") :]
        if payload.strip() == "[DONE]":
            return None
        try:
            data = json.loads(payload)
        except json.JSONDecodeError:
            return None
        choices = data.get("choices", [])
        if not choices:
            return None
        delta = choices[0].get("delta", {})
        content = delta.get("content", "")
        if not content:
            return None
        return LLMStreamChunk(
            content=content,
            model=data.get("model", ""),
            finish_reason=choices[0].get("finish_reason"),
        )
