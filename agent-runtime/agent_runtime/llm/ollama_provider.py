"""Ollama LLM provider (local models, zero cost).

Uses the Ollama API via httpx.  Ollama exposes an OpenAI-compatible
``/v1/chat/completions`` endpoint, but we use the native
``/api/chat`` endpoint for richer streaming control.
"""

from __future__ import annotations

import json
import logging
from typing import AsyncIterator

from .base import (
    LLMConfig,
    LLMMessage,
    LLMProvider,
    LLMResponse,
    LLMStreamChunk,
    TokenUsage,
)

logger = logging.getLogger(__name__)

_DEFAULT_BASE_URL = "http://localhost:11434"


class OllamaProvider(LLMProvider):
    """Ollama local model provider.

    No API key required.  Expects Ollama to be running locally.
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
            f"{self._base_url}/api/chat",
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
            f"{self._base_url}/api/chat",
            json=payload,
        ) as resp:
            resp.raise_for_status()
            async for line in resp.aiter_lines():
                chunk = self._parse_stream_line(line)
                if chunk is not None:
                    yield chunk

    # ------------------------------------------------------------------
    # Internal helpers
    # ------------------------------------------------------------------

    def _build_payload(
        self,
        messages: list[LLMMessage],
        *,
        stream: bool = False,
        max_tokens: int | None = None,
        temperature: float | None = None,
    ) -> dict:
        options: dict = {}
        if max_tokens is not None:
            options["num_predict"] = max_tokens
        elif self._config.max_tokens != 4096:
            options["num_predict"] = self._config.max_tokens
        if temperature is not None:
            options["temperature"] = temperature
        elif self._config.temperature != 0.7:
            options["temperature"] = self._config.temperature

        payload: dict = {
            "model": self._config.model,
            "messages": [{"role": m.role, "content": m.content} for m in messages],
            "stream": stream,
        }
        if options:
            payload["options"] = options
        return payload

    @staticmethod
    def _parse_response(data: dict) -> LLMResponse:
        content = data.get("message", {}).get("content", "")
        eval_count = data.get("eval_count", 0) or 0
        prompt_eval_count = data.get("prompt_eval_count", 0) or 0
        usage = TokenUsage(
            prompt_tokens=prompt_eval_count,
            completion_tokens=eval_count,
            total_tokens=prompt_eval_count + eval_count,
        )
        return LLMResponse(
            content=content,
            model=data.get("model", ""),
            usage=usage,
            finish_reason="stop" if data.get("done", False) else None,
        )

    @staticmethod
    def _parse_stream_line(line: str) -> LLMStreamChunk | None:
        """Parse a single JSON line from the Ollama streaming response."""
        if not line.strip():
            return None
        try:
            data = json.loads(line)
        except json.JSONDecodeError:
            return None
        content = data.get("message", {}).get("content", "")
        done = data.get("done", False)
        return LLMStreamChunk(
            content=content,
            model=data.get("model", ""),
            finish_reason="stop" if done else None,
        )
