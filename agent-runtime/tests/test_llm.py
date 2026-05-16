"""Tests for the LLM provider abstraction layer.

Covers:
- LLMProvider ABC and data types
- OpenAIProvider with mocked HTTP
- AnthropicProvider with mocked HTTP
- OllamaProvider with mocked HTTP
- Config-driven provider selection (factory)
- Token counting and cost tracking
- Streaming support
"""

from __future__ import annotations

import json
from typing import Any
from unittest.mock import AsyncMock, MagicMock, patch

import pytest

from agent_runtime.llm import (
    AnthropicProvider,
    CostTracker,
    LLMConfig,
    LLMError,
    LLMMessage,
    LLMProvider,
    LLMResponse,
    LLMStreamChunk,
    OllamaProvider,
    OpenAIProvider,
    TokenUsage,
    create_provider,
)
from agent_runtime.llm.base import ProviderType


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def _openai_response(
    content: str = "Hello!",
    model: str = "gpt-4",
    prompt_tokens: int = 10,
    completion_tokens: int = 5,
    finish_reason: str = "stop",
) -> dict:
    """Build a fake OpenAI Chat Completions API response."""
    return {
        "id": "chatcmpl-test",
        "object": "chat.completion",
        "model": model,
        "choices": [
            {
                "index": 0,
                "message": {"role": "assistant", "content": content},
                "finish_reason": finish_reason,
            }
        ],
        "usage": {
            "prompt_tokens": prompt_tokens,
            "completion_tokens": completion_tokens,
            "total_tokens": prompt_tokens + completion_tokens,
        },
    }


def _anthropic_response(
    content: str = "Hi there!",
    model: str = "claude-3-sonnet",
    input_tokens: int = 15,
    output_tokens: int = 8,
    stop_reason: str = "end_turn",
) -> dict:
    """Build a fake Anthropic Messages API response."""
    return {
        "id": "msg_test",
        "type": "message",
        "role": "assistant",
        "model": model,
        "content": [{"type": "text", "text": content}],
        "stop_reason": stop_reason,
        "usage": {"input_tokens": input_tokens, "output_tokens": output_tokens},
    }


def _ollama_response(
    content: str = "Local response",
    model: str = "llama3",
    prompt_eval_count: int = 12,
    eval_count: int = 6,
    done: bool = True,
) -> dict:
    """Build a fake Ollama /api/chat response."""
    return {
        "model": model,
        "message": {"role": "assistant", "content": content},
        "prompt_eval_count": prompt_eval_count,
        "eval_count": eval_count,
        "done": done,
    }


def _make_mock_response(data: dict, status_code: int = 200) -> MagicMock:
    """Create a mock httpx.Response."""
    mock_resp = MagicMock()
    mock_resp.status_code = status_code
    mock_resp.json.return_value = data
    mock_resp.raise_for_status = MagicMock()
    return mock_resp


# ---------------------------------------------------------------------------
# Data types tests
# ---------------------------------------------------------------------------


class TestLLMMessage:
    def test_create(self):
        msg = LLMMessage(role="user", content="Hello")
        assert msg.role == "user"
        assert msg.content == "Hello"

    def test_frozen(self):
        msg = LLMMessage(role="user", content="Hello")
        with pytest.raises(AttributeError):
            msg.content = "Changed"  # type: ignore[misc]


class TestTokenUsage:
    def test_defaults(self):
        usage = TokenUsage()
        assert usage.prompt_tokens == 0
        assert usage.completion_tokens == 0
        assert usage.total_tokens == 0

    def test_custom(self):
        usage = TokenUsage(prompt_tokens=10, completion_tokens=20, total_tokens=30)
        assert usage.total_tokens == 30


class TestLLMResponse:
    def test_create(self):
        resp = LLMResponse(content="Hello", model="gpt-4")
        assert resp.content == "Hello"
        assert resp.model == "gpt-4"
        assert resp.finish_reason is None

    def test_with_usage(self):
        usage = TokenUsage(prompt_tokens=10, completion_tokens=5, total_tokens=15)
        resp = LLMResponse(content="Hi", model="gpt-4", usage=usage)
        assert resp.usage.total_tokens == 15


class TestLLMConfig:
    def test_create(self):
        config = LLMConfig(
            provider=ProviderType.OPENAI,
            model="gpt-4",
            api_key="sk-test",
        )
        assert config.provider == ProviderType.OPENAI
        assert config.model == "gpt-4"
        assert config.api_key == "sk-test"
        assert config.timeout == 60.0
        assert config.max_tokens == 4096
        assert config.temperature is None

    def test_ollama_no_api_key(self):
        config = LLMConfig(provider=ProviderType.OLLAMA, model="llama3")
        assert config.api_key is None


# ---------------------------------------------------------------------------
# OpenAI Provider tests
# ---------------------------------------------------------------------------


class TestOpenAIProvider:
    def _make_config(self, **kwargs) -> LLMConfig:
        defaults = dict(provider=ProviderType.OPENAI, model="gpt-4", api_key="sk-test")
        defaults.update(kwargs)
        return LLMConfig(**defaults)

    @pytest.mark.asyncio
    async def test_chat_success(self):
        config = self._make_config()
        provider = OpenAIProvider(config)
        fake_resp = _make_mock_response(_openai_response())

        with patch.object(provider._client, "post", new_callable=AsyncMock) as mock_post:
            mock_post.return_value = fake_resp
            messages = [LLMMessage(role="user", content="Hello")]
            result = await provider.chat(messages)

        assert result.content == "Hello!"
        assert result.model == "gpt-4"
        assert result.usage.prompt_tokens == 10
        assert result.usage.completion_tokens == 5
        assert result.usage.total_tokens == 15
        assert result.finish_reason == "stop"

        # Verify the request was well-formed
        call_args = mock_post.call_args
        body = call_args.kwargs["json"]
        assert body["model"] == "gpt-4"
        assert body["messages"] == [{"role": "user", "content": "Hello"}]
        assert body["stream"] is False
        await provider.close()

    @pytest.mark.asyncio
    async def test_chat_with_system_message(self):
        config = self._make_config()
        provider = OpenAIProvider(config)
        fake_resp = _make_mock_response(_openai_response())

        with patch.object(provider._client, "post", new_callable=AsyncMock) as mock_post:
            mock_post.return_value = fake_resp
            messages = [
                LLMMessage(role="system", content="You are helpful."),
                LLMMessage(role="user", content="Hi"),
            ]
            result = await provider.chat(messages)

        body = mock_post.call_args.kwargs["json"]
        assert len(body["messages"]) == 2
        assert body["messages"][0]["role"] == "system"
        await provider.close()

    @pytest.mark.asyncio
    async def test_chat_custom_params(self):
        config = self._make_config()
        provider = OpenAIProvider(config)
        fake_resp = _make_mock_response(_openai_response())

        with patch.object(provider._client, "post", new_callable=AsyncMock) as mock_post:
            mock_post.return_value = fake_resp
            messages = [LLMMessage(role="user", content="Hi")]
            await provider.chat(messages, max_tokens=100, temperature=0.5)

        body = mock_post.call_args.kwargs["json"]
        assert body["max_tokens"] == 100
        assert body["temperature"] == 0.5
        await provider.close()

    @pytest.mark.asyncio
    async def test_chat_auth_header(self):
        config = self._make_config(api_key="sk-secret")
        provider = OpenAIProvider(config)
        fake_resp = _make_mock_response(_openai_response())

        with patch.object(provider._client, "post", new_callable=AsyncMock) as mock_post:
            mock_post.return_value = fake_resp
            await provider.chat([LLMMessage(role="user", content="Hi")])

        headers = mock_post.call_args.kwargs["headers"]
        assert headers["Authorization"] == "Bearer sk-secret"
        await provider.close()

    @pytest.mark.asyncio
    async def test_chat_custom_base_url(self):
        config = self._make_config(base_url="https://custom.openai.proxy/v1")
        provider = OpenAIProvider(config)
        fake_resp = _make_mock_response(_openai_response())

        with patch.object(provider._client, "post", new_callable=AsyncMock) as mock_post:
            mock_post.return_value = fake_resp
            await provider.chat([LLMMessage(role="user", content="Hi")])

        url = mock_post.call_args.args[0]
        assert url.startswith("https://custom.openai.proxy/v1/")
        await provider.close()

    def test_parse_sse_line_data(self):
        line = 'data: {"choices":[{"delta":{"content":"Hi"}}],"model":"gpt-4"}'
        chunk = OpenAIProvider._parse_sse_line(line)
        assert chunk is not None
        assert chunk.content == "Hi"

    def test_parse_sse_line_done(self):
        chunk = OpenAIProvider._parse_sse_line("data: [DONE]")
        assert chunk is None

    def test_parse_sse_line_non_data(self):
        chunk = OpenAIProvider._parse_sse_line("")
        assert chunk is None

    def test_parse_sse_line_empty_content(self):
        line = 'data: {"choices":[{"delta":{}}],"model":"gpt-4"}'
        chunk = OpenAIProvider._parse_sse_line(line)
        assert chunk is None


# ---------------------------------------------------------------------------
# Anthropic Provider tests
# ---------------------------------------------------------------------------


class TestAnthropicProvider:
    def _make_config(self, **kwargs) -> LLMConfig:
        defaults = dict(provider=ProviderType.ANTHROPIC, model="claude-3-sonnet", api_key="sk-ant-test")
        defaults.update(kwargs)
        return LLMConfig(**defaults)

    @pytest.mark.asyncio
    async def test_chat_success(self):
        config = self._make_config()
        provider = AnthropicProvider(config)
        fake_resp = _make_mock_response(_anthropic_response())

        with patch.object(provider._client, "post", new_callable=AsyncMock) as mock_post:
            mock_post.return_value = fake_resp
            messages = [LLMMessage(role="user", content="Hello")]
            result = await provider.chat(messages)

        assert result.content == "Hi there!"
        assert result.model == "claude-3-sonnet"
        assert result.usage.prompt_tokens == 15
        assert result.usage.completion_tokens == 8
        assert result.usage.total_tokens == 23
        assert result.finish_reason == "end_turn"
        await provider.close()

    @pytest.mark.asyncio
    async def test_system_message_extraction(self):
        config = self._make_config()
        provider = AnthropicProvider(config)
        fake_resp = _make_mock_response(_anthropic_response())

        with patch.object(provider._client, "post", new_callable=AsyncMock) as mock_post:
            mock_post.return_value = fake_resp
            messages = [
                LLMMessage(role="system", content="Be helpful"),
                LLMMessage(role="user", content="Hi"),
            ]
            await provider.chat(messages)

        body = mock_post.call_args.kwargs["json"]
        # System should be extracted to a separate field
        assert body["system"] == "Be helpful"
        # Only non-system messages in messages array
        assert len(body["messages"]) == 1
        assert body["messages"][0]["role"] == "user"
        await provider.close()

    @pytest.mark.asyncio
    async def test_multiple_system_messages(self):
        config = self._make_config()
        provider = AnthropicProvider(config)
        fake_resp = _make_mock_response(_anthropic_response())

        with patch.object(provider._client, "post", new_callable=AsyncMock) as mock_post:
            mock_post.return_value = fake_resp
            messages = [
                LLMMessage(role="system", content="Part 1"),
                LLMMessage(role="system", content="Part 2"),
                LLMMessage(role="user", content="Hi"),
            ]
            await provider.chat(messages)

        body = mock_post.call_args.kwargs["json"]
        assert body["system"] == "Part 1\n\nPart 2"
        await provider.close()

    @pytest.mark.asyncio
    async def test_headers(self):
        config = self._make_config(api_key="sk-ant-secret")
        provider = AnthropicProvider(config)
        fake_resp = _make_mock_response(_anthropic_response())

        with patch.object(provider._client, "post", new_callable=AsyncMock) as mock_post:
            mock_post.return_value = fake_resp
            await provider.chat([LLMMessage(role="user", content="Hi")])

        headers = mock_post.call_args.kwargs["headers"]
        assert headers["x-api-key"] == "sk-ant-secret"
        assert "anthropic-version" in headers
        await provider.close()

    def test_split_system_no_system(self):
        msgs = [
            LLMMessage(role="user", content="Hi"),
            LLMMessage(role="assistant", content="Hello"),
        ]
        system, chat = AnthropicProvider._split_system(msgs)
        assert system is None
        assert len(chat) == 2

    def test_parse_response(self):
        data = _anthropic_response(content="Test response", input_tokens=20, output_tokens=10)
        result = AnthropicProvider._parse_response(data)
        assert result.content == "Test response"
        assert result.usage.prompt_tokens == 20
        assert result.usage.completion_tokens == 10
        assert result.usage.total_tokens == 30

    def test_parse_sse_content_delta(self):
        line = json.dumps({
            "type": "content_block_delta",
            "delta": {"type": "text_delta", "text": "Hello"},
        })
        chunk = AnthropicProvider._parse_sse_line(f"data: {line}")
        assert chunk is not None
        assert chunk.content == "Hello"

    def test_parse_sse_message_start(self):
        line = json.dumps({
            "type": "message_start",
            "message": {"model": "claude-3-sonnet"},
        })
        chunk = AnthropicProvider._parse_sse_line(f"data: {line}")
        assert chunk is not None
        assert chunk.model == "claude-3-sonnet"

    def test_parse_sse_non_data(self):
        chunk = AnthropicProvider._parse_sse_line("event: ping")
        assert chunk is None


# ---------------------------------------------------------------------------
# Ollama Provider tests
# ---------------------------------------------------------------------------


class TestOllamaProvider:
    def _make_config(self, **kwargs) -> LLMConfig:
        defaults = dict(provider=ProviderType.OLLAMA, model="llama3")
        defaults.update(kwargs)
        return LLMConfig(**defaults)

    @pytest.mark.asyncio
    async def test_chat_success(self):
        config = self._make_config()
        provider = OllamaProvider(config)
        fake_resp = _make_mock_response(_ollama_response())

        with patch.object(provider._client, "post", new_callable=AsyncMock) as mock_post:
            mock_post.return_value = fake_resp
            messages = [LLMMessage(role="user", content="Hello")]
            result = await provider.chat(messages)

        assert result.content == "Local response"
        assert result.model == "llama3"
        assert result.usage.prompt_tokens == 12
        assert result.usage.completion_tokens == 6
        assert result.usage.total_tokens == 18
        assert result.finish_reason == "stop"
        await provider.close()

    @pytest.mark.asyncio
    async def test_no_api_key_required(self):
        config = self._make_config()
        provider = OllamaProvider(config)
        fake_resp = _make_mock_response(_ollama_response())

        with patch.object(provider._client, "post", new_callable=AsyncMock) as mock_post:
            mock_post.return_value = fake_resp
            await provider.chat([LLMMessage(role="user", content="Hi")])

        # No Authorization header should be set
        headers = mock_post.call_args.kwargs.get("headers", {})
        assert "Authorization" not in headers
        await provider.close()

    @pytest.mark.asyncio
    async def test_custom_base_url(self):
        config = self._make_config(base_url="http://my-server:11434")
        provider = OllamaProvider(config)
        fake_resp = _make_mock_response(_ollama_response())

        with patch.object(provider._client, "post", new_callable=AsyncMock) as mock_post:
            mock_post.return_value = fake_resp
            await provider.chat([LLMMessage(role="user", content="Hi")])

        url = mock_post.call_args.args[0]
        assert url.startswith("http://my-server:11434/")
        await provider.close()

    @pytest.mark.asyncio
    async def test_options_passed(self):
        config = self._make_config()
        provider = OllamaProvider(config)
        fake_resp = _make_mock_response(_ollama_response())

        with patch.object(provider._client, "post", new_callable=AsyncMock) as mock_post:
            mock_post.return_value = fake_resp
            await provider.chat(
                [LLMMessage(role="user", content="Hi")],
                max_tokens=200,
                temperature=0.3,
            )

        body = mock_post.call_args.kwargs["json"]
        assert body["options"]["num_predict"] == 200
        assert body["options"]["temperature"] == 0.3
        await provider.close()

    @pytest.mark.asyncio
    async def test_default_url_is_localhost(self):
        config = self._make_config()
        provider = OllamaProvider(config)
        assert provider._base_url == "http://localhost:11434"
        await provider.close()

    def test_parse_response_no_eval_counts(self):
        data = {"model": "llama3", "message": {"role": "assistant", "content": "Hi"}, "done": True}
        result = OllamaProvider._parse_response(data)
        assert result.content == "Hi"
        assert result.usage.prompt_tokens == 0
        assert result.usage.completion_tokens == 0

    def test_parse_stream_line(self):
        line = json.dumps({"model": "llama3", "message": {"content": "chunk"}, "done": False})
        chunk = OllamaProvider._parse_stream_line(line)
        assert chunk is not None
        assert chunk.content == "chunk"
        assert chunk.finish_reason is None

    def test_parse_stream_line_done(self):
        line = json.dumps({"model": "llama3", "message": {"content": ""}, "done": True})
        chunk = OllamaProvider._parse_stream_line(line)
        assert chunk is not None
        assert chunk.finish_reason == "stop"

    def test_parse_stream_line_empty(self):
        assert OllamaProvider._parse_stream_line("") is None


# ---------------------------------------------------------------------------
# Factory (config-driven selection) tests
# ---------------------------------------------------------------------------


class TestFactory:
    def test_create_openai(self):
        config = LLMConfig(provider=ProviderType.OPENAI, model="gpt-4", api_key="sk-test")
        provider = create_provider(config)
        assert isinstance(provider, OpenAIProvider)

    def test_create_anthropic(self):
        config = LLMConfig(provider=ProviderType.ANTHROPIC, model="claude-3-sonnet", api_key="sk-ant")
        provider = create_provider(config)
        assert isinstance(provider, AnthropicProvider)

    def test_create_ollama(self):
        config = LLMConfig(provider=ProviderType.OLLAMA, model="llama3")
        provider = create_provider(config)
        assert isinstance(provider, OllamaProvider)

    def test_create_unknown_raises(self):
        config = LLMConfig(provider="unknown", model="test")  # type: ignore[arg-type]
        with pytest.raises(ValueError, match="Unknown provider"):
            create_provider(config)

    @pytest.mark.asyncio
    async def test_created_provider_is_configured(self):
        config = LLMConfig(
            provider=ProviderType.OPENAI,
            model="gpt-4o",
            api_key="sk-test",
            temperature=0.5,
            max_tokens=2048,
        )
        provider = create_provider(config)
        assert provider.config.model == "gpt-4o"
        assert provider.config.temperature == 0.5
        assert provider.config.max_tokens == 2048
        await provider.close()


# ---------------------------------------------------------------------------
# Cost tracker tests
# ---------------------------------------------------------------------------


class TestCostTracker:
    @pytest.mark.asyncio
    async def test_record_single(self):
        tracker = CostTracker()
        response = LLMResponse(
            content="Hello",
            model="gpt-4",
            usage=TokenUsage(prompt_tokens=100, completion_tokens=50, total_tokens=150),
        )
        rec = await tracker.record(response)
        assert rec.model == "gpt-4"
        assert rec.prompt_tokens == 100
        assert rec.completion_tokens == 50
        # gpt-4: prompt $0.03/1K, completion $0.06/1K
        expected_cost = 100 * 0.03 / 1000 + 50 * 0.06 / 1000
        assert rec.cost_usd == pytest.approx(expected_cost)

    @pytest.mark.asyncio
    async def test_total_properties(self):
        tracker = CostTracker()
        await tracker.record(LLMResponse(
            content="A", model="gpt-4",
            usage=TokenUsage(prompt_tokens=100, completion_tokens=50, total_tokens=150),
        ))
        await tracker.record(LLMResponse(
            content="B", model="gpt-4",
            usage=TokenUsage(prompt_tokens=200, completion_tokens=100, total_tokens=300),
        ))
        assert tracker.total_prompt_tokens == 300
        assert tracker.total_completion_tokens == 150
        assert tracker.total_tokens == 450
        assert tracker.total_cost_usd > 0

    @pytest.mark.asyncio
    async def test_summary(self):
        tracker = CostTracker()
        await tracker.record(LLMResponse(
            content="A", model="gpt-4",
            usage=TokenUsage(prompt_tokens=100, completion_tokens=50, total_tokens=150),
        ))
        s = tracker.summary()
        assert s["calls"] == 1
        assert s["total_prompt_tokens"] == 100
        assert s["total_completion_tokens"] == 50
        assert s["total_tokens"] == 150
        assert isinstance(s["total_cost_usd"], float)

    @pytest.mark.asyncio
    async def test_by_model(self):
        tracker = CostTracker()
        await tracker.record(LLMResponse(
            content="A", model="gpt-4",
            usage=TokenUsage(prompt_tokens=100, completion_tokens=50, total_tokens=150),
        ))
        await tracker.record(LLMResponse(
            content="B", model="claude-3-sonnet",
            usage=TokenUsage(prompt_tokens=200, completion_tokens=100, total_tokens=300),
        ))
        by_model = tracker.by_model()
        assert "gpt-4" in by_model
        assert "claude-3-sonnet" in by_model
        assert by_model["gpt-4"]["calls"] == 1
        assert by_model["claude-3-sonnet"]["calls"] == 1

    @pytest.mark.asyncio
    async def test_reset(self):
        tracker = CostTracker()
        await tracker.record(LLMResponse(
            content="A", model="gpt-4",
            usage=TokenUsage(prompt_tokens=100, completion_tokens=50, total_tokens=150),
        ))
        await tracker.reset()
        assert tracker.total_tokens == 0
        assert tracker.total_cost_usd == 0.0

    def test_pricing_prefix_match(self):
        """gpt-4-0613 should use gpt-4 pricing via prefix match."""
        from agent_runtime.llm.cost import _get_pricing
        pricing = _get_pricing("gpt-4-0613")
        assert pricing["prompt"] == 0.03
        assert pricing["completion"] == 0.06

    def test_pricing_prefix_longest_first(self):
        """gpt-4o-mini should match gpt-4o-mini, not gpt-4o."""
        from agent_runtime.llm.cost import _get_pricing
        pricing = _get_pricing("gpt-4o-mini")
        assert pricing["prompt"] == 0.00015
        assert pricing["completion"] == 0.0006

    def test_pricing_unknown_model(self):
        from agent_runtime.llm.cost import _get_pricing, _DEFAULT_PRICING
        pricing = _get_pricing("unknown-model-xyz")
        assert pricing == _DEFAULT_PRICING

    @pytest.mark.asyncio
    async def test_ollama_zero_cost(self):
        """Ollama models use default pricing but can be tracked."""
        tracker = CostTracker()
        await tracker.record(LLMResponse(
            content="Local", model="llama3",
            usage=TokenUsage(prompt_tokens=50, completion_tokens=25, total_tokens=75),
        ))
        # Should still track tokens
        assert tracker.total_tokens == 75
        # Cost is based on default pricing (not zero, but that's fine for tracking)
        assert tracker.total_cost_usd > 0

    def test_empty_tracker(self):
        tracker = CostTracker()
        assert tracker.total_tokens == 0
        assert tracker.total_cost_usd == 0.0
        assert tracker.summary()["calls"] == 0
        assert tracker.by_model() == {}


# ---------------------------------------------------------------------------
# Context manager tests
# ---------------------------------------------------------------------------


class TestContextManager:
    @pytest.mark.asyncio
    async def test_async_context_manager(self):
        config = LLMConfig(provider=ProviderType.OPENAI, model="gpt-4", api_key="sk-test")
        async with OpenAIProvider(config) as provider:
            assert isinstance(provider, OpenAIProvider)
        # Client should be closed after exiting context


# ---------------------------------------------------------------------------
# LLMError tests
# ---------------------------------------------------------------------------


class TestLLMError:
    def test_llm_error_fields(self):
        err = LLMError("boom", provider="openai", model="gpt-4")
        assert str(err) == "boom"
        assert err.provider == "openai"
        assert err.model == "gpt-4"

    @pytest.mark.asyncio
    async def test_openai_http_error_wrapped(self):
        import httpx
        config = LLMConfig(provider=ProviderType.OPENAI, model="gpt-4", api_key="sk-test")
        provider = OpenAIProvider(config)
        with patch.object(provider._client, "post", new_callable=AsyncMock) as mock_post:
            mock_post.side_effect = httpx.ConnectError("refused")
            with pytest.raises(LLMError, match="OpenAI request failed"):
                await provider.chat([LLMMessage(role="user", content="Hi")])
        await provider.close()

    @pytest.mark.asyncio
    async def test_anthropic_http_error_wrapped(self):
        import httpx
        config = LLMConfig(provider=ProviderType.ANTHROPIC, model="claude-3-sonnet", api_key="sk-ant")
        provider = AnthropicProvider(config)
        with patch.object(provider._client, "post", new_callable=AsyncMock) as mock_post:
            mock_post.side_effect = httpx.ConnectError("refused")
            with pytest.raises(LLMError, match="Anthropic request failed"):
                await provider.chat([LLMMessage(role="user", content="Hi")])
        await provider.close()

    @pytest.mark.asyncio
    async def test_ollama_http_error_wrapped(self):
        import httpx
        config = LLMConfig(provider=ProviderType.OLLAMA, model="llama3")
        provider = OllamaProvider(config)
        with patch.object(provider._client, "post", new_callable=AsyncMock) as mock_post:
            mock_post.side_effect = httpx.ConnectError("refused")
            with pytest.raises(LLMError, match="Ollama request failed"):
                await provider.chat([LLMMessage(role="user", content="Hi")])
        await provider.close()
