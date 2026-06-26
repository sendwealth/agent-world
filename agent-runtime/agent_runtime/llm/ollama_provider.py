"""Ollama LLM provider (local models, zero cost).

Uses the Ollama API via httpx.  Ollama exposes an OpenAI-compatible
``/v1/chat/completions`` endpoint, but we use the native
``/api/chat`` endpoint for richer streaming control.
"""

from __future__ import annotations

import asyncio
import json
import logging
import os
from dataclasses import dataclass
from typing import Any, AsyncIterator

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

_DEFAULT_BASE_URL = "http://localhost:11434"


@dataclass(frozen=True)
class OllamaModelInfo:
    """Information about a loaded Ollama model."""

    name: str
    size: int = 0
    digest: str = ""


@dataclass(frozen=True)
class OllamaHealthStatus:
    """Health check result for the Ollama service."""

    healthy: bool
    loaded_models: list[str]
    num_parallel: int = 1


class OllamaProvider(LLMProvider):
    """Ollama local model provider.

    No API key required.  Expects Ollama to be running locally.
    Supports runtime model switching and concurrent health checks.
    """

    def __init__(self, config: LLMConfig) -> None:
        super().__init__(config)
        self._base_url = (config.base_url or _DEFAULT_BASE_URL).rstrip("/")
        self._num_parallel = int(os.environ.get("OLLAMA_NUM_PARALLEL", "1"))
        self._config_lock: asyncio.Lock | None = None

        # Override the client with fine-grained timeouts for long text generation
        self._client = httpx.AsyncClient(
            timeout=httpx.Timeout(
                connect=10.0,
                read=config.timeout,
                write=30.0,
                pool=10.0,
            ),
        )

    def _get_config_lock(self) -> asyncio.Lock:
        if self._config_lock is None:
            self._config_lock = asyncio.Lock()
        return self._config_lock

    @property
    def active_model(self) -> str:
        """Return the currently configured model name."""
        return self._config.model

    @property
    def num_parallel(self) -> int:
        """Return the configured parallel request count."""
        return self._num_parallel

    async def check_health(self) -> OllamaHealthStatus:
        """Query the Ollama ``/api/ps`` endpoint for service health.

        Returns loaded model names and the ``OLLAMA_NUM_PARALLEL`` config.
        """
        try:
            resp = await self._client.get(f"{self._base_url}/api/ps")
            resp.raise_for_status()
            data = resp.json()
            loaded_models = [
                m.get("name", "") for m in data.get("models", []) if m.get("name")
            ]
            return OllamaHealthStatus(
                healthy=True,
                loaded_models=loaded_models,
                num_parallel=self._num_parallel,
            )
        except httpx.HTTPError:
            return OllamaHealthStatus(
                healthy=False,
                loaded_models=[],
                num_parallel=self._num_parallel,
            )

    async def switch_model(self, new_model: str) -> str:
        """Switch the active model at runtime.

        Protected by an ``asyncio.Lock`` to prevent TOCTOU races between
        concurrent ``switch_model`` or ``chat`` calls.

        Rebuilds the httpx.AsyncClient so that timeout / connection pool
        settings are consistent with the new configuration.

        Returns the previous model name.
        """
        async with self._get_config_lock():
            old_model = self._config.model
            new_config = LLMConfig(
                provider=self._config.provider,
                model=new_model,
                api_key=self._config.api_key,
                base_url=self._config.base_url,
                timeout=self._config.timeout,
                max_tokens=self._config.max_tokens,
                temperature=self._config.temperature,
            )
            self._config = new_config

            # Rebuild the client with the new config
            old_client = self._client
            self._client = httpx.AsyncClient(
                timeout=httpx.Timeout(
                    connect=10.0,
                    read=new_config.timeout,
                    write=30.0,
                    pool=10.0,
                ),
            )
            await old_client.aclose()

        return old_model

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
        # Snapshot config under lock to avoid TOCTOU with switch_model
        async with self._get_config_lock():
            payload = self._build_payload(
                messages,
                stream=False,
                max_tokens=max_tokens,
                temperature=temperature,
            )
            current_model = self._config.model
        try:
            resp = await self._client.post(
                f"{self._base_url}/api/chat",
                json=payload,
            )
            resp.raise_for_status()
        except httpx.HTTPError as exc:
            raise LLMError(
                f"Ollama request failed: {exc}",
                provider="ollama",
                model=current_model,
            ) from exc
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
        # Snapshot config under lock to avoid TOCTOU with switch_model
        async with self._get_config_lock():
            payload = self._build_payload(
                messages,
                stream=True,
                max_tokens=max_tokens,
                temperature=temperature,
            )
            current_model = self._config.model
        try:
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
        except httpx.HTTPError as exc:
            raise LLMError(
                f"Ollama stream failed: {exc}",
                provider="ollama",
                model=current_model,
            ) from exc

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
    ) -> dict[str, Any]:
        options: dict[str, Any] = {}
        if max_tokens is not None:
            options["num_predict"] = max_tokens
        else:
            options["num_predict"] = self._config.max_tokens

        # Cap num_predict for small/thinking models to prevent slow responses
        # (e.g. minicpm5-1b emits long <think/> blocks by default)
        cap = int(os.environ.get("OLLAMA_MAX_PREDICT_CAP", "512"))
        if options["num_predict"] > cap:
            options["num_predict"] = cap

        if temperature is not None:
            options["temperature"] = temperature
        elif self._config.temperature is not None:
            options["temperature"] = self._config.temperature

        payload: dict[str, Any] = {
            "model": self._config.model,
            "messages": [{"role": m.role, "content": m.content} for m in messages],
            "stream": stream,
        }
        if options:
            payload["options"] = options
        return payload

    @staticmethod
    def _parse_response(data: dict[str, Any]) -> LLMResponse:
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
