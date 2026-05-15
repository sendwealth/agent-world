"""OpenAI LLM provider (GPT-4, GPT-3.5, etc.).

Uses the OpenAI Chat Completions API via httpx.
"""

from __future__ import annotations

import json
import logging
from typing import AsyncIterator

import httpx

from .base import (
    LLMConfig,
    LLMMessage,
    LLMProvider,
    LLMResponse,
    LLMStreamChunk,
    TokenUsage,
)

logger = logging.getLogger(__name__)

_DEFAULT_BASE_URL = "https://api.openai.com/v1"


class OpenAIProvider(LLMProvider):
    """OpenAI Chat Completions provider.

    Expects ``config.api_key`` to be set.
    """

    def __init__(self, config: LLMConfig) -> None:
        super().__init__(config)
        self._base_url = (config.base_url or _DEFAULT_BASE_URL).rstrip("/")

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
            messages, stream=False, max_tokens=max_tokens, temperature=temperature,
        )
        resp = await self._client.post(
            f"{self._base_url}/chat/completions",
            headers=self._headers(),
            json=payload,
        )
        resp.raise_for_status()
        data = resp.json()
        return self._parse_response(data)

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
            messages, stream=True, max_tokens=max_tokens, temperature=temperature,
        )
        async with self._client.stream(
            "POST",
            f"{self._base_url}/chat/completions",
            headers=self._headers(),
            json=payload,
        ) as resp:
            resp.raise_for_status()
            async for line in resp.aiter_lines():
                chunk = self._parse_sse_line(line)
                if chunk is not None:
                    yield chunk

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
            "max_tokens": max_tokens or self._config.max_tokens,
            "temperature": temperature if temperature is not None else self._config.temperature,
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
        payload = line[len("data: "):]
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
