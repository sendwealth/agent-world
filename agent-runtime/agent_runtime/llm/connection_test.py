"""Provider connection testing and model discovery.

Sends a minimal chat request to verify connectivity, authentication, and
model availability.  Also exposes a ``discover_provider_models`` helper that
lists available models for each supported protocol.

This module defines its own ``ProviderConfig`` dataclass so it can be used
independently of the T1 protocol registry.
"""

from __future__ import annotations

import ipaddress
import logging
import time
from dataclasses import dataclass
from enum import Enum
from typing import Any
from urllib.parse import urlparse

import httpx

logger = logging.getLogger(__name__)

# ---------------------------------------------------------------------------
# Data types
# ---------------------------------------------------------------------------


class ConnectionTestKind(str, Enum):
    """Classification of a connection test result."""

    SUCCESS = "success"
    AUTH_FAILED = "auth_failed"
    TIMEOUT = "timeout"
    NOT_FOUND_MODEL = "not_found_model"
    INVALID_BASE_URL = "invalid_base_url"
    RATE_LIMITED = "rate_limited"
    UPSTREAM_UNAVAILABLE = "upstream_unavailable"
    UNKNOWN = "unknown"


@dataclass(frozen=True)
class ConnectionTestResult:
    """Result of a provider connection test."""

    ok: bool
    kind: ConnectionTestKind
    latency_ms: float
    model: str
    sample: str = ""
    detail: str = ""


@dataclass(frozen=True)
class ModelOption:
    """A single model discovered from a provider."""

    id: str


@dataclass
class ProviderConfig:
    """Minimal configuration needed to connect to an LLM provider.

    Independent of ``LLMConfig`` so this module can be used before the
    full provider registry is available.
    """

    protocol: str  # "openai" | "anthropic" | "ollama" | "google" | "azure"
    base_url: str
    api_key: str | None = None
    timeout: float = 15.0
    # Azure-specific
    api_version: str | None = None  # e.g. "2024-02-15-preview"


# ---------------------------------------------------------------------------
# Base-URL validation
# ---------------------------------------------------------------------------

# Private-network ranges we reject (SSRF protection).
_PRIVATE_NETWORKS = [
    ipaddress.ip_network("10.0.0.0/8"),
    ipaddress.ip_network("172.16.0.0/12"),
    ipaddress.ip_network("192.168.0.0/16"),
    ipaddress.ip_network("169.254.0.0/16"),  # link-local
    ipaddress.ip_network("127.0.0.0/8"),  # loopback — handled separately
]


def validate_base_url(base_url: str, *, allow_localhost: bool = True) -> str | None:
    """Return an error string if *base_url* is unsafe, else ``None``.

    Rejects RFC 1918 / link-local addresses.  Localhost (``127.0.0.1``,
    ``::1``, ``localhost``) is allowed when *allow_localhost* is True so
    that local providers like Ollama work out of the box.
    """
    try:
        parsed = urlparse(base_url)
        hostname = parsed.hostname
        if not hostname:
            return "Missing hostname in base_url"
    except Exception as exc:
        return f"Cannot parse base_url: {exc}"

    # Resolve hostname to IP and check against private ranges.
    try:
        addr = ipaddress.ip_address(hostname)
    except ValueError:
        # Not an IP literal (e.g. "api.openai.com") — fine.
        return None

    for net in _PRIVATE_NETWORKS:
        if addr in net:
            # Loopback is a special case: allowed for local providers.
            if addr.is_loopback:
                return None if allow_localhost else "Loopback address not allowed"
            return f"Private/reserved IP rejected: {addr} ({net})"

    return None


# ---------------------------------------------------------------------------
# Connection test
# ---------------------------------------------------------------------------

_TEST_MESSAGES = [
    {"role": "system", "content": "Reply with only: ok"},
    {"role": "user", "content": "Say ok"},
]

_TEST_MAX_TOKENS = 10


async def test_provider_connection(
    config: ProviderConfig,
    model: str,
) -> ConnectionTestResult:
    """Test connectivity to a provider by sending a minimal chat request."""
    # 1. Validate base URL first.
    url_err = validate_base_url(config.base_url)
    if url_err:
        return ConnectionTestResult(
            ok=False,
            kind=ConnectionTestKind.INVALID_BASE_URL,
            latency_ms=0,
            model=model,
            detail=url_err,
        )

    protocol = config.protocol.lower()
    handler = _CONNECTION_HANDLERS.get(protocol)
    if handler is None:
        return ConnectionTestResult(
            ok=False,
            kind=ConnectionTestKind.UNKNOWN,
            latency_ms=0,
            model=model,
            detail=f"Unsupported protocol: {config.protocol!r}",
        )

    start = time.monotonic()
    try:
        return await handler(config, model, start)
    except Exception as exc:
        elapsed_ms = (time.monotonic() - start) * 1000
        return ConnectionTestResult(
            ok=False,
            kind=ConnectionTestKind.UNKNOWN,
            latency_ms=elapsed_ms,
            model=model,
            detail=str(exc),
        )


# ---------------------------------------------------------------------------
# Shared HTTP helper for chat-style protocols
# ---------------------------------------------------------------------------


def _classify_response(
    resp: httpx.Response,
    model: str,
    elapsed_ms: float,
    protocol: str,
) -> ConnectionTestResult:
    """Turn an httpx response into a ConnectionTestResult."""
    if resp.status_code == 200:
        try:
            data = resp.json()
        except Exception:
            return ConnectionTestResult(
                ok=True,
                kind=ConnectionTestKind.SUCCESS,
                latency_ms=elapsed_ms,
                model=model,
                detail="(non-JSON response)",
            )
        sample = _extract_sample(protocol, data)
        return ConnectionTestResult(
            ok=True,
            kind=ConnectionTestKind.SUCCESS,
            latency_ms=elapsed_ms,
            model=model,
            sample=sample,
        )

    # Error classification by status code.
    if resp.status_code in (401, 403):
        return ConnectionTestResult(
            ok=False,
            kind=ConnectionTestKind.AUTH_FAILED,
            latency_ms=elapsed_ms,
            model=model,
            detail=f"HTTP {resp.status_code}: {resp.text[:200]}",
        )
    if resp.status_code == 404:
        return ConnectionTestResult(
            ok=False,
            kind=ConnectionTestKind.NOT_FOUND_MODEL,
            latency_ms=elapsed_ms,
            model=model,
            detail=f"HTTP 404: {resp.text[:200]}",
        )
    if resp.status_code == 429:
        return ConnectionTestResult(
            ok=False,
            kind=ConnectionTestKind.RATE_LIMITED,
            latency_ms=elapsed_ms,
            model=model,
            detail=f"HTTP 429: {resp.text[:200]}",
        )
    if resp.status_code >= 500:
        return ConnectionTestResult(
            ok=False,
            kind=ConnectionTestKind.UPSTREAM_UNAVAILABLE,
            latency_ms=elapsed_ms,
            model=model,
            detail=f"HTTP {resp.status_code}: {resp.text[:200]}",
        )
    return ConnectionTestResult(
        ok=False,
        kind=ConnectionTestKind.UNKNOWN,
        latency_ms=elapsed_ms,
        model=model,
        detail=f"HTTP {resp.status_code}: {resp.text[:200]}",
    )


def _extract_sample(protocol: str, data: dict) -> str:
    """Extract the text sample from a successful response body."""
    if protocol == "anthropic":
        return "".join(
            b["text"] for b in data.get("content", []) if b.get("type") == "text"
        )
    if protocol == "ollama":
        return data.get("message", {}).get("content", "")
    # OpenAI / Azure share the same shape
    choices = data.get("choices", [])
    if choices:
        return choices[0].get("message", {}).get("content", "")
    return ""


async def _do_chat_test(
    base_url: str,
    path: str,
    headers: dict[str, str],
    payload: dict,
    model: str,
    start: float,
    extract_content: Any | None = None,
) -> ConnectionTestResult:
    """POST to a chat endpoint and classify the response."""
    url = f"{base_url}{path}"
    try:
        async with httpx.AsyncClient(timeout=15.0) as client:
            resp = await client.post(url, headers=headers, json=payload)
            elapsed_ms = (time.monotonic() - start) * 1000
            return _classify_response(
                resp, model, elapsed_ms, _protocol_from_path(path)
            )
    except httpx.TimeoutException:
        elapsed_ms = (time.monotonic() - start) * 1000
        return ConnectionTestResult(
            ok=False,
            kind=ConnectionTestKind.TIMEOUT,
            latency_ms=elapsed_ms,
            model=model,
            detail="Connection timed out",
        )
    except httpx.ConnectError:
        elapsed_ms = (time.monotonic() - start) * 1000
        return ConnectionTestResult(
            ok=False,
            kind=ConnectionTestKind.UPSTREAM_UNAVAILABLE,
            latency_ms=elapsed_ms,
            model=model,
            detail="Cannot connect to upstream",
        )


def _protocol_from_path(path: str) -> str:
    """Best-effort protocol name from the URL path."""
    if "/v1/messages" in path:
        return "anthropic"
    if "/api/chat" in path:
        return "ollama"
    if "/openai/deployments/" in path:
        return "azure"
    return "openai"


# ---------------------------------------------------------------------------
# Per-protocol connection-test strategies
# ---------------------------------------------------------------------------


async def _test_openai(
    config: ProviderConfig,
    model: str,
    start: float,
) -> ConnectionTestResult:
    base = config.base_url.rstrip("/")
    headers: dict[str, str] = {"Content-Type": "application/json"}
    if config.api_key:
        headers["Authorization"] = f"Bearer {config.api_key}"

    payload = {
        "model": model,
        "messages": _TEST_MESSAGES,
        "max_tokens": _TEST_MAX_TOKENS,
    }
    return await _do_chat_test(
        base_url=base,
        path="/v1/chat/completions",
        headers=headers,
        payload=payload,
        model=model,
        start=start,
    )


async def _test_anthropic(
    config: ProviderConfig,
    model: str,
    start: float,
) -> ConnectionTestResult:
    base = config.base_url.rstrip("/")
    headers: dict[str, str] = {
        "Content-Type": "application/json",
        "anthropic-version": "2023-06-01",
    }
    if config.api_key:
        headers["x-api-key"] = config.api_key

    payload = {
        "model": model,
        "max_tokens": _TEST_MAX_TOKENS,
        "messages": [{"role": "user", "content": "Say ok"}],
        "system": "Reply with only: ok",
    }
    return await _do_chat_test(
        base_url=base,
        path="/v1/messages",
        headers=headers,
        payload=payload,
        model=model,
        start=start,
    )


async def _test_ollama(
    config: ProviderConfig,
    model: str,
    start: float,
) -> ConnectionTestResult:
    base = config.base_url.rstrip("/")
    payload = {
        "model": model,
        "messages": _TEST_MESSAGES,
        "stream": False,
        "options": {"num_predict": _TEST_MAX_TOKENS},
    }
    return await _do_chat_test(
        base_url=base,
        path="/api/chat",
        headers={},
        payload=payload,
        model=model,
        start=start,
    )


async def _test_google(
    config: ProviderConfig,
    model: str,
    start: float,
) -> ConnectionTestResult:
    base = config.base_url.rstrip("/")
    # Google passes the API key in the URL.
    sep = "&" if "?" in base else "?"
    url = f"{base}/v1beta/models/{model}:generateContent{sep}key={config.api_key}"
    payload = {
        "contents": [{"parts": [{"text": "Say ok"}]}],
        "systemInstruction": {"parts": [{"text": "Reply with only: ok"}]},
        "generationConfig": {"maxOutputTokens": _TEST_MAX_TOKENS},
    }
    try:
        async with httpx.AsyncClient(timeout=config.timeout) as client:
            resp = await client.post(url, json=payload)
            elapsed_ms = (time.monotonic() - start) * 1000
            return _classify_response(resp, model, elapsed_ms, "google")
    except httpx.TimeoutException:
        elapsed_ms = (time.monotonic() - start) * 1000
        return ConnectionTestResult(
            ok=False,
            kind=ConnectionTestKind.TIMEOUT,
            latency_ms=elapsed_ms,
            model=model,
            detail="Connection timed out",
        )
    except httpx.ConnectError:
        elapsed_ms = (time.monotonic() - start) * 1000
        return ConnectionTestResult(
            ok=False,
            kind=ConnectionTestKind.UPSTREAM_UNAVAILABLE,
            latency_ms=elapsed_ms,
            model=model,
            detail="Cannot connect to upstream",
        )


async def _test_azure(
    config: ProviderConfig,
    model: str,
    start: float,
) -> ConnectionTestResult:
    base = config.base_url.rstrip("/")
    api_version = config.api_version or "2024-02-15-preview"
    path = f"/openai/deployments/{model}/chat/completions?api-version={api_version}"
    headers: dict[str, str] = {"Content-Type": "application/json"}
    if config.api_key:
        headers["api-key"] = config.api_key

    payload = {
        "messages": _TEST_MESSAGES,
        "max_tokens": _TEST_MAX_TOKENS,
    }
    return await _do_chat_test(
        base_url=base,
        path=path,
        headers=headers,
        payload=payload,
        model=model,
        start=start,
    )


_CONNECTION_HANDLERS: dict[str, Any] = {
    "openai": _test_openai,
    "anthropic": _test_anthropic,
    "ollama": _test_ollama,
    "google": _test_google,
    "azure": _test_azure,
}


# ---------------------------------------------------------------------------
# Model discovery
# ---------------------------------------------------------------------------


async def discover_provider_models(config: ProviderConfig) -> list[ModelOption]:
    """List available models for the given provider.

    Azure does not support dynamic discovery — returns an empty list.
    """
    url_err = validate_base_url(config.base_url)
    if url_err:
        return []

    protocol = config.protocol.lower()
    handler = _DISCOVERY_HANDLERS.get(protocol)
    if handler is None:
        return []
    try:
        return await handler(config)
    except Exception:
        logger.debug("Model discovery failed for %s", protocol, exc_info=True)
        return []


async def _discover_openai(config: ProviderConfig) -> list[ModelOption]:
    base = config.base_url.rstrip("/")
    headers: dict[str, str] = {}
    if config.api_key:
        headers["Authorization"] = f"Bearer {config.api_key}"
    async with httpx.AsyncClient(timeout=config.timeout) as client:
        resp = await client.get(f"{base}/v1/models", headers=headers)
        resp.raise_for_status()
        data = resp.json()
    return [ModelOption(id=m["id"]) for m in data.get("data", [])]


async def _discover_anthropic(config: ProviderConfig) -> list[ModelOption]:
    base = config.base_url.rstrip("/")
    headers: dict[str, str] = {}
    if config.api_key:
        headers["x-api-key"] = config.api_key
    async with httpx.AsyncClient(timeout=config.timeout) as client:
        resp = await client.get(
            f"{base}/v1/models?limit=1000", headers=headers
        )
        resp.raise_for_status()
        data = resp.json()
    return [ModelOption(id=m["id"]) for m in data.get("data", [])]


async def _discover_ollama(config: ProviderConfig) -> list[ModelOption]:
    base = config.base_url.rstrip("/")
    async with httpx.AsyncClient(timeout=config.timeout) as client:
        resp = await client.get(f"{base}/api/tags")
        resp.raise_for_status()
        data = resp.json()
    return [ModelOption(id=m["name"]) for m in data.get("models", [])]


async def _discover_google(config: ProviderConfig) -> list[ModelOption]:
    base = config.base_url.rstrip("/")
    sep = "&" if "?" in base else "?"
    url = f"{base}/v1beta/models{sep}key={config.api_key}"
    async with httpx.AsyncClient(timeout=config.timeout) as client:
        resp = await client.get(url)
        resp.raise_for_status()
        data = resp.json()
    return [ModelOption(id=m["name"]) for m in data.get("models", [])]


async def _discover_azure(config: ProviderConfig) -> list[ModelOption]:
    """Azure does not support dynamic model discovery."""
    return []


_DISCOVERY_HANDLERS: dict[str, Any] = {
    "openai": _discover_openai,
    "anthropic": _discover_anthropic,
    "ollama": _discover_ollama,
    "google": _discover_google,
    "azure": _discover_azure,
}
