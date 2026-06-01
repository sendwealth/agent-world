"""Tests for provider connection testing and model discovery.

All HTTP calls are mocked via ``httpx.MockTransport`` so no real network
traffic occurs.
"""

from __future__ import annotations

from typing import Any

import httpx
import pytest

from agent_runtime.llm.connection_test import (
    ConnectionTestKind,
    ProviderConfig,
    discover_provider_models,
    validate_base_url,
)
from agent_runtime.llm.connection_test import (
    test_provider_connection as do_test_connection,
)

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def _make_config(
    protocol: str = "ollama",
    base_url: str = "http://localhost:11434",
    api_key: str | None = None,
    **kw: Any,
) -> ProviderConfig:
    return ProviderConfig(protocol=protocol, base_url=base_url, api_key=api_key, **kw)


def _json_response(
    status_code: int = 200,
    body: dict | list | None = None,
) -> httpx.Response:
    """Build a fake httpx.Response with a JSON body."""
    return httpx.Response(
        status_code=status_code,
        json=body or {},
        request=httpx.Request("POST", "http://test/"),
    )


def _openai_chat_response(content: str = "ok") -> dict:
    return {
        "id": "chatcmpl-test",
        "object": "chat.completion",
        "model": "gpt-4",
        "choices": [
            {
                "index": 0,
                "message": {"role": "assistant", "content": content},
                "finish_reason": "stop",
            }
        ],
        "usage": {"prompt_tokens": 5, "completion_tokens": 2, "total_tokens": 7},
    }


def _anthropic_chat_response(content: str = "ok") -> dict:
    return {
        "id": "msg_test",
        "type": "message",
        "role": "assistant",
        "model": "claude-3-sonnet",
        "content": [{"type": "text", "text": content}],
        "stop_reason": "end_turn",
        "usage": {"input_tokens": 5, "output_tokens": 2},
    }


def _ollama_chat_response(content: str = "ok") -> dict:
    return {
        "model": "minicpm5-1b",
        "message": {"role": "assistant", "content": content},
        "done": True,
    }


def _google_chat_response(content: str = "ok") -> dict:
    return {
        "candidates": [{"content": {"parts": [{"text": content}]}}],
    }


# ---------------------------------------------------------------------------
# Base-URL validation
# ---------------------------------------------------------------------------


class TestValidateBaseUrl:
    def test_localhost_allowed(self) -> None:
        assert validate_base_url("http://localhost:11434") is None
        assert validate_base_url("http://127.0.0.1:11434") is None

    def test_localhost_disallowed(self) -> None:
        assert validate_base_url("http://127.0.0.1:11434", allow_localhost=False) is not None

    def test_rfc1918_rejected(self) -> None:
        assert validate_base_url("http://10.0.0.1/v1") is not None
        assert validate_base_url("http://172.16.0.1/v1") is not None
        assert validate_base_url("http://192.168.1.1/v1") is not None

    def test_link_local_rejected(self) -> None:
        assert validate_base_url("http://169.254.0.1/v1") is not None

    def test_public_ip_ok(self) -> None:
        assert validate_base_url("http://1.2.3.4/v1") is None

    def test_hostname_ok(self) -> None:
        assert validate_base_url("https://api.openai.com/v1") is None

    def test_missing_hostname(self) -> None:
        assert validate_base_url("http://") is not None

    def test_invalid_url(self) -> None:
        # urlparse can parse almost anything; test a clearly bad one
        result = validate_base_url("")
        # empty string → hostname is None → "Missing hostname"
        assert result is not None


# ---------------------------------------------------------------------------
# Connection tests — per protocol
# ---------------------------------------------------------------------------


class TestOllamaConnection:
    @pytest.mark.asyncio
    async def test_success(self, monkeypatch: pytest.MonkeyPatch) -> None:
        config = _make_config("ollama", "http://localhost:11434")
        mock_resp = _json_response(200, _ollama_chat_response("ok"))

        async def _mock_post(client_self: Any, url: str, **kw: Any) -> httpx.Response:
            return mock_resp

        monkeypatch.setattr(httpx.AsyncClient, "post", _mock_post)
        result = await do_test_connection(config, "minicpm5-1b")
        assert result.ok is True
        assert result.kind == ConnectionTestKind.SUCCESS
        assert result.model == "minicpm5-1b"
        assert "ok" in result.sample.lower()

    @pytest.mark.asyncio
    async def test_model_not_found(self, monkeypatch: pytest.MonkeyPatch) -> None:
        config = _make_config("ollama", "http://localhost:11434")
        mock_resp = _json_response(404, {"error": "model not found"})

        async def _mock_post(client_self: Any, url: str, **kw: Any) -> httpx.Response:
            return mock_resp

        monkeypatch.setattr(httpx.AsyncClient, "post", _mock_post)
        result = await do_test_connection(config, "nonexistent-model")
        assert result.ok is False
        assert result.kind == ConnectionTestKind.NOT_FOUND_MODEL


class TestOpenAIConnection:
    @pytest.mark.asyncio
    async def test_success(self, monkeypatch: pytest.MonkeyPatch) -> None:
        config = _make_config("openai", "https://api.openai.com", "sk-test")
        mock_resp = _json_response(200, _openai_chat_response("ok"))

        async def _mock_post(client_self: Any, url: str, **kw: Any) -> httpx.Response:
            return mock_resp

        monkeypatch.setattr(httpx.AsyncClient, "post", _mock_post)
        result = await do_test_connection(config, "gpt-4")
        assert result.ok is True
        assert result.kind == ConnectionTestKind.SUCCESS

    @pytest.mark.asyncio
    async def test_auth_failed(self, monkeypatch: pytest.MonkeyPatch) -> None:
        config = _make_config("openai", "https://api.openai.com", "bad-key")
        mock_resp = _json_response(401, {"error": "invalid api key"})

        async def _mock_post(client_self: Any, url: str, **kw: Any) -> httpx.Response:
            return mock_resp

        monkeypatch.setattr(httpx.AsyncClient, "post", _mock_post)
        result = await do_test_connection(config, "gpt-4")
        assert result.ok is False
        assert result.kind == ConnectionTestKind.AUTH_FAILED


class TestAnthropicConnection:
    @pytest.mark.asyncio
    async def test_success(self, monkeypatch: pytest.MonkeyPatch) -> None:
        config = _make_config("anthropic", "https://api.anthropic.com", "sk-ant-test")
        mock_resp = _json_response(200, _anthropic_chat_response("ok"))

        async def _mock_post(client_self: Any, url: str, **kw: Any) -> httpx.Response:
            return mock_resp

        monkeypatch.setattr(httpx.AsyncClient, "post", _mock_post)
        result = await do_test_connection(config, "claude-3-sonnet")
        assert result.ok is True
        assert result.kind == ConnectionTestKind.SUCCESS


class TestGoogleConnection:
    @pytest.mark.asyncio
    async def test_success(self, monkeypatch: pytest.MonkeyPatch) -> None:
        config = _make_config("google", "https://generativelanguage.googleapis.com", "test-key")
        mock_resp = _json_response(200, _google_chat_response("ok"))

        async def _mock_post(client_self: Any, url: str, **kw: Any) -> httpx.Response:
            return mock_resp

        monkeypatch.setattr(httpx.AsyncClient, "post", _mock_post)
        result = await do_test_connection(config, "gemini-pro")
        assert result.ok is True
        assert result.kind == ConnectionTestKind.SUCCESS


class TestAzureConnection:
    @pytest.mark.asyncio
    async def test_success(self, monkeypatch: pytest.MonkeyPatch) -> None:
        config = _make_config(
            "azure",
            "https://my-resource.openai.azure.com",
            "azure-key",
            api_version="2024-02-15-preview",
        )
        mock_resp = _json_response(200, _openai_chat_response("ok"))

        async def _mock_post(client_self: Any, url: str, **kw: Any) -> httpx.Response:
            return mock_resp

        monkeypatch.setattr(httpx.AsyncClient, "post", _mock_post)
        result = await do_test_connection(config, "my-deployment")
        assert result.ok is True
        assert result.kind == ConnectionTestKind.SUCCESS


# ---------------------------------------------------------------------------
# Error-classification tests
# ---------------------------------------------------------------------------


class TestConnectionErrors:
    @pytest.mark.asyncio
    async def test_timeout(self, monkeypatch: pytest.MonkeyPatch) -> None:
        config = _make_config("ollama", "http://localhost:11434")

        async def _mock_post(client_self: Any, url: str, **kw: Any) -> httpx.Response:
            raise httpx.TimeoutException("timed out")

        monkeypatch.setattr(httpx.AsyncClient, "post", _mock_post)
        result = await do_test_connection(config, "minicpm5-1b")
        assert result.ok is False
        assert result.kind == ConnectionTestKind.TIMEOUT

    @pytest.mark.asyncio
    async def test_connect_error(self, monkeypatch: pytest.MonkeyPatch) -> None:
        config = _make_config("ollama", "http://localhost:11434")

        async def _mock_post(client_self: Any, url: str, **kw: Any) -> httpx.Response:
            raise httpx.ConnectError("refused")

        monkeypatch.setattr(httpx.AsyncClient, "post", _mock_post)
        result = await do_test_connection(config, "minicpm5-1b")
        assert result.ok is False
        assert result.kind == ConnectionTestKind.UPSTREAM_UNAVAILABLE

    @pytest.mark.asyncio
    async def test_rate_limited(self, monkeypatch: pytest.MonkeyPatch) -> None:
        config = _make_config("openai", "https://api.openai.com", "sk-test")
        mock_resp = _json_response(429, {"error": "rate limited"})

        async def _mock_post(client_self: Any, url: str, **kw: Any) -> httpx.Response:
            return mock_resp

        monkeypatch.setattr(httpx.AsyncClient, "post", _mock_post)
        result = await do_test_connection(config, "gpt-4")
        assert result.ok is False
        assert result.kind == ConnectionTestKind.RATE_LIMITED

    @pytest.mark.asyncio
    async def test_server_error(self, monkeypatch: pytest.MonkeyPatch) -> None:
        config = _make_config("openai", "https://api.openai.com", "sk-test")
        mock_resp = _json_response(503, {"error": "service unavailable"})

        async def _mock_post(client_self: Any, url: str, **kw: Any) -> httpx.Response:
            return mock_resp

        monkeypatch.setattr(httpx.AsyncClient, "post", _mock_post)
        result = await do_test_connection(config, "gpt-4")
        assert result.ok is False
        assert result.kind == ConnectionTestKind.UPSTREAM_UNAVAILABLE

    @pytest.mark.asyncio
    async def test_invalid_base_url(self) -> None:
        config = _make_config("openai", "http://10.0.0.1/v1")
        result = await do_test_connection(config, "gpt-4")
        assert result.ok is False
        assert result.kind == ConnectionTestKind.INVALID_BASE_URL

    @pytest.mark.asyncio
    async def test_unsupported_protocol(self) -> None:
        config = _make_config("unknown", "https://example.com")
        result = await do_test_connection(config, "model")
        assert result.ok is False
        assert result.kind == ConnectionTestKind.UNKNOWN


# ---------------------------------------------------------------------------
# Model discovery tests
# ---------------------------------------------------------------------------


class TestDiscoverModels:
    @pytest.mark.asyncio
    async def test_ollama_discovery(self, monkeypatch: pytest.MonkeyPatch) -> None:
        config = _make_config("ollama", "http://localhost:11434")
        body = {
            "models": [
                {"name": "minicpm5-1b", "size": 656000000, "digest": "abc123"},
                {"name": "llama3:8b", "size": 4700000000, "digest": "def456"},
            ]
        }
        mock_resp = _json_response(200, body)

        async def _mock_get(client_self: Any, url: str, **kw: Any) -> httpx.Response:
            return mock_resp

        monkeypatch.setattr(httpx.AsyncClient, "get", _mock_get)
        models = await discover_provider_models(config)
        ids = [m.id for m in models]
        assert "minicpm5-1b" in ids
        assert "llama3:8b" in ids

    @pytest.mark.asyncio
    async def test_openai_discovery(self, monkeypatch: pytest.MonkeyPatch) -> None:
        config = _make_config("openai", "https://api.openai.com", "sk-test")
        body = {
            "data": [
                {"id": "gpt-4", "object": "model"},
                {"id": "gpt-3.5-turbo", "object": "model"},
            ]
        }
        mock_resp = _json_response(200, body)

        async def _mock_get(client_self: Any, url: str, **kw: Any) -> httpx.Response:
            return mock_resp

        monkeypatch.setattr(httpx.AsyncClient, "get", _mock_get)
        models = await discover_provider_models(config)
        ids = [m.id for m in models]
        assert "gpt-4" in ids
        assert "gpt-3.5-turbo" in ids

    @pytest.mark.asyncio
    async def test_anthropic_discovery(self, monkeypatch: pytest.MonkeyPatch) -> None:
        config = _make_config("anthropic", "https://api.anthropic.com", "sk-ant-test")
        body = {
            "data": [
                {"id": "claude-3-sonnet", "display_name": "Claude 3 Sonnet"},
            ]
        }
        mock_resp = _json_response(200, body)

        async def _mock_get(client_self: Any, url: str, **kw: Any) -> httpx.Response:
            return mock_resp

        monkeypatch.setattr(httpx.AsyncClient, "get", _mock_get)
        models = await discover_provider_models(config)
        ids = [m.id for m in models]
        assert "claude-3-sonnet" in ids

    @pytest.mark.asyncio
    async def test_google_discovery(self, monkeypatch: pytest.MonkeyPatch) -> None:
        config = _make_config("google", "https://generativelanguage.googleapis.com", "test-key")
        body = {
            "models": [
                {"name": "models/gemini-pro"},
                {"name": "models/gemini-1.5-flash"},
            ]
        }
        mock_resp = _json_response(200, body)

        async def _mock_get(client_self: Any, url: str, **kw: Any) -> httpx.Response:
            return mock_resp

        monkeypatch.setattr(httpx.AsyncClient, "get", _mock_get)
        models = await discover_provider_models(config)
        ids = [m.id for m in models]
        assert "models/gemini-pro" in ids

    @pytest.mark.asyncio
    async def test_azure_returns_empty(self) -> None:
        config = _make_config("azure", "https://my-resource.openai.azure.com", "azure-key")
        models = await discover_provider_models(config)
        assert models == []

    @pytest.mark.asyncio
    async def test_discovery_invalid_base_url(self) -> None:
        config = _make_config("openai", "http://10.0.0.1/v1", "sk-test")
        models = await discover_provider_models(config)
        assert models == []

    @pytest.mark.asyncio
    async def test_discovery_http_error(self, monkeypatch: pytest.MonkeyPatch) -> None:
        config = _make_config("ollama", "http://localhost:11434")

        async def _mock_get(client_self: Any, url: str, **kw: Any) -> httpx.Response:
            raise httpx.ConnectError("refused")

        monkeypatch.setattr(httpx.AsyncClient, "get", _mock_get)
        models = await discover_provider_models(config)
        assert models == []

    @pytest.mark.asyncio
    async def test_unsupported_protocol_discovery(self) -> None:
        config = _make_config("unknown", "https://example.com")
        models = await discover_provider_models(config)
        assert models == []


# ---------------------------------------------------------------------------
# Google connection-test error paths (dedicated since Google has its own
# HTTP logic rather than using the shared _do_chat_test helper)
# ---------------------------------------------------------------------------


class TestGoogleConnectionErrors:
    @pytest.mark.asyncio
    async def test_timeout(self, monkeypatch: pytest.MonkeyPatch) -> None:
        config = _make_config("google", "https://generativelanguage.googleapis.com", "k")

        async def _mock_post(client_self: Any, url: str, **kw: Any) -> httpx.Response:
            raise httpx.TimeoutException("timed out")

        monkeypatch.setattr(httpx.AsyncClient, "post", _mock_post)
        result = await do_test_connection(config, "gemini-pro")
        assert result.ok is False
        assert result.kind == ConnectionTestKind.TIMEOUT

    @pytest.mark.asyncio
    async def test_connect_error(self, monkeypatch: pytest.MonkeyPatch) -> None:
        config = _make_config("google", "https://generativelanguage.googleapis.com", "k")

        async def _mock_post(client_self: Any, url: str, **kw: Any) -> httpx.Response:
            raise httpx.ConnectError("refused")

        monkeypatch.setattr(httpx.AsyncClient, "post", _mock_post)
        result = await do_test_connection(config, "gemini-pro")
        assert result.ok is False
        assert result.kind == ConnectionTestKind.UPSTREAM_UNAVAILABLE

    @pytest.mark.asyncio
    async def test_auth_failed(self, monkeypatch: pytest.MonkeyPatch) -> None:
        config = _make_config("google", "https://generativelanguage.googleapis.com", "bad")
        mock_resp = _json_response(403, {"error": "forbidden"})

        async def _mock_post(client_self: Any, url: str, **kw: Any) -> httpx.Response:
            return mock_resp

        monkeypatch.setattr(httpx.AsyncClient, "post", _mock_post)
        result = await do_test_connection(config, "gemini-pro")
        assert result.ok is False
        assert result.kind == ConnectionTestKind.AUTH_FAILED
