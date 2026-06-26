"""Anthropic LLM provider (Claude).

Uses the Anthropic Messages API via httpx.
"""

from __future__ import annotations

import asyncio
import json
import logging
import os
from collections.abc import AsyncIterator
from typing import Any

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

_DEFAULT_BASE_URL = "https://api.anthropic.com/v1"
_ANTHROPIC_VERSION = "2023-06-01"

# Rate-limit retry configuration (same env vars as OpenAI provider)
_ANTHROPIC_MAX_RETRIES = int(os.environ.get("LLM_MAX_RETRIES", "3"))


class AnthropicProvider(LLMProvider):
    """Anthropic Messages API provider.

    Expects ``config.api_key`` to be set.
    """

    def __init__(self, config: LLMConfig) -> None:
        super().__init__(config)
        self._base_url = (config.base_url or _DEFAULT_BASE_URL).rstrip("/")
        self._max_retries = _ANTHROPIC_MAX_RETRIES

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
        system_msg, chat_messages = self._split_system(messages)
        payload = self._build_payload(
            chat_messages,
            system=system_msg,
            stream=False,
            max_tokens=max_tokens,
            temperature=temperature,
        )
        last_exc: Exception | None = None
        for attempt in range(self._max_retries + 1):
            try:
                resp = await self._client.post(
                    f"{self._base_url}/messages",
                    headers=self._headers(),
                    json=payload,
                )
                if resp.status_code == 429:
                    delay = self._anthropic_backoff(attempt)
                    logger.warning(
                        "Anthropic rate-limited (429) on attempt %d/%d — "
                        "retrying in %.1fs",
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
                break
            except Exception as exc:
                last_exc = exc
                break

        raise LLMError(
            f"Anthropic request failed after {self._max_retries + 1} attempts: {last_exc}",
            provider="anthropic",
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
        system_msg, chat_messages = self._split_system(messages)
        payload = self._build_payload(
            chat_messages,
            system=system_msg,
            stream=True,
            max_tokens=max_tokens,
            temperature=temperature,
        )
        try:
            async with self._client.stream(
                "POST",
                f"{self._base_url}/messages",
                headers=self._headers(),
                json=payload,
            ) as resp:
                resp.raise_for_status()
                model = ""
                async for line in resp.aiter_lines():
                    chunk = self._parse_sse_line(line)
                    if chunk is not None:
                        model = chunk.model or model
                        if chunk.content or chunk.finish_reason is not None:
                            yield LLMStreamChunk(
                                content=chunk.content,
                                model=model,
                                finish_reason=chunk.finish_reason,
                            )
        except httpx.HTTPError as exc:
            raise LLMError(
                f"Anthropic stream failed: {exc}",
                provider="anthropic",
                model=self._config.model,
            ) from exc

    # ------------------------------------------------------------------
    # Internal helpers
    # ------------------------------------------------------------------

    @staticmethod
    def _anthropic_backoff(attempt: int) -> float:
        """Compute exponential backoff with jitter for 429 retries."""
        import random

        min_backoff_str = os.environ.get("LLM_MIN_BACKOFF_SECONDS", "1.0")
        max_backoff_str = os.environ.get("LLM_MAX_BACKOFF_SECONDS", "60.0")
        jitter_factor_str = os.environ.get("LLM_JITTER_FACTOR", "0.25")
        min_backoff: float = float(min_backoff_str)
        max_backoff: float = float(max_backoff_str)
        jitter_factor: float = float(jitter_factor_str)
        exponential = min_backoff * (2 ** attempt)
        clamped = min(exponential, max_backoff)
        jitter: float = clamped * jitter_factor * float(random.uniform(-1, 1))
        return max(0.0, jitter)

    def _headers(self) -> dict[str, str]:
        headers: dict[str, str] = {
            "Content-Type": "application/json",
            "anthropic-version": _ANTHROPIC_VERSION,
        }
        if self._config.api_key:
            headers["x-api-key"] = self._config.api_key
        return headers

    def _build_payload(
        self,
        messages: list[LLMMessage],
        *,
        system: str | None = None,
        stream: bool = False,
        max_tokens: int | None = None,
        temperature: float | None = None,
    ) -> dict[str, Any]:
        payload: dict[str, Any] = {
            "model": self._config.model,
            "messages": [{"role": m.role, "content": m.content} for m in messages],
            "max_tokens": max_tokens if max_tokens is not None else self._config.max_tokens,
            "stream": stream,
        }
        if temperature is not None:
            payload["temperature"] = temperature
        elif self._config.temperature is not None:
            payload["temperature"] = self._config.temperature
        if system:
            payload["system"] = system
        return payload

    @staticmethod
    def _split_system(
        messages: list[LLMMessage],
    ) -> tuple[str | None, list[LLMMessage]]:
        """Extract system messages; Anthropic uses a separate ``system`` field."""
        system_parts: list[str] = []
        chat_msgs: list[LLMMessage] = []
        for msg in messages:
            if msg.role == "system":
                system_parts.append(msg.content)
            else:
                chat_msgs.append(msg)
        system = "\n\n".join(system_parts) if system_parts else None
        return system, chat_msgs

    @staticmethod
    def _parse_response(data: dict[str, Any]) -> LLMResponse:
        content_blocks = data.get("content", [])
        text = "".join(block["text"] for block in content_blocks if block.get("type") == "text")
        usage_data = data.get("usage", {})
        usage = TokenUsage(
            prompt_tokens=usage_data.get("input_tokens", 0),
            completion_tokens=usage_data.get("output_tokens", 0),
            total_tokens=(usage_data.get("input_tokens", 0) + usage_data.get("output_tokens", 0)),
        )
        return LLMResponse(
            content=text,
            model=data.get("model", ""),
            usage=usage,
            finish_reason=data.get("stop_reason"),
        )

    @staticmethod
    def _parse_sse_line(line: str) -> LLMStreamChunk | None:
        """Parse a single SSE line from the Anthropic streaming response."""
        if not line.startswith("data: "):
            return None
        payload = line[len("data: ") :]
        try:
            data = json.loads(payload)
        except json.JSONDecodeError:
            return None

        event_type = data.get("type", "")

        if event_type == "content_block_delta":
            delta = data.get("delta", {})
            if delta.get("type") == "text_delta":
                text = delta.get("text", "")
                if not text:
                    return None
                return LLMStreamChunk(
                    content=text,
                    model="",  # Model is in the message_start event
                    finish_reason=None,
                )

        if event_type == "message_start":
            msg = data.get("message", {})
            return LLMStreamChunk(
                content="",
                model=msg.get("model", ""),
                finish_reason=None,
            )

        if event_type == "message_delta":
            delta = data.get("delta", {})
            return LLMStreamChunk(
                content="",
                model="",
                finish_reason=delta.get("stop_reason"),
            )

        return None
