"""Anthropic LLM provider (Claude).

Uses the Anthropic Messages API via httpx.
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

_DEFAULT_BASE_URL = "https://api.anthropic.com/v1"
_ANTHROPIC_VERSION = "2023-06-01"


class AnthropicProvider(LLMProvider):
    """Anthropic Messages API provider.

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
        system_msg, chat_messages = self._split_system(messages)
        payload = self._build_payload(
            chat_messages,
            system=system_msg,
            stream=False,
            max_tokens=max_tokens,
            temperature=temperature,
        )
        resp = await self._client.post(
            f"{self._base_url}/messages",
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
        system_msg, chat_messages = self._split_system(messages)
        payload = self._build_payload(
            chat_messages,
            system=system_msg,
            stream=True,
            max_tokens=max_tokens,
            temperature=temperature,
        )
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
                    yield chunk

    # ------------------------------------------------------------------
    # Internal helpers
    # ------------------------------------------------------------------

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
    ) -> dict:
        payload: dict = {
            "model": self._config.model,
            "messages": [{"role": m.role, "content": m.content} for m in messages],
            "max_tokens": max_tokens or self._config.max_tokens,
            "stream": stream,
        }
        if temperature is not None:
            payload["temperature"] = temperature
        elif self._config.temperature != 0.7:
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
    def _parse_response(data: dict) -> LLMResponse:
        content_blocks = data.get("content", [])
        text = "".join(
            block["text"] for block in content_blocks if block.get("type") == "text"
        )
        usage_data = data.get("usage", {})
        usage = TokenUsage(
            prompt_tokens=usage_data.get("input_tokens", 0),
            completion_tokens=usage_data.get("output_tokens", 0),
            total_tokens=(
                usage_data.get("input_tokens", 0) + usage_data.get("output_tokens", 0)
            ),
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
        payload = line[len("data: "):]
        try:
            data = json.loads(payload)
        except json.JSONDecodeError:
            return None

        event_type = data.get("type", "")

        if event_type == "content_block_delta":
            delta = data.get("delta", {})
            if delta.get("type") == "text_delta":
                return LLMStreamChunk(
                    content=delta.get("text", ""),
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
