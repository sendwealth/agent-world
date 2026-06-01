"""Model preset store — load and query built-in provider/model configurations.

The presets file (``config/model-presets.yaml``) ships with the project and
defines well-known providers (Ollama, Zhipu, DeepSeek, OpenRouter) and their
recommended models.  The CLI uses ``--preset <provider_id>`` to auto-fill
connection details so users only need to supply an API key when required.

Usage::

    from agent_runtime.presets import load_presets, get_provider_preset, resolve_preset

    # Load all presets (cached after first call)
    presets = load_presets()

    # Look up a single provider preset
    zhipu = get_provider_preset("zhipu")
    print(zhipu["base_url"])  # https://open.bigmodel.cn/api/paas/v4

    # Resolve --preset + --model into LLMConfig kwargs
    cfg = resolve_preset("zhipu", "glm-5")
    # cfg = {"provider": "openai", "model": "glm-5", "base_url": "..."}

Data classes are deliberately avoided here — the preset data is read-only
dict-from-YAML, and converting every entry to a dataclass adds boilerplate
with no practical benefit for a config-loading code path.
"""

from __future__ import annotations

import logging
from pathlib import Path
from typing import Any

import yaml

logger = logging.getLogger(__name__)

# Default path: <repo_root>/config/model-presets.yaml
_REPO_ROOT = Path(__file__).resolve().parent.parent.parent
_DEFAULT_PRESETS_PATH = _REPO_ROOT / "config" / "model-presets.yaml"

# Module-level cache — presets are static, load once.
_cache: dict[str, Any] | None = None


# ---------------------------------------------------------------------------
# Loading
# ---------------------------------------------------------------------------


def load_presets(path: Path | None = None) -> dict[str, Any]:
    """Load the presets YAML file and return the parsed dict.

    Results are cached after the first successful call.  Pass a *path*
    explicitly to bypass the cache (used in tests).

    Raises:
        FileNotFoundError: If the presets file does not exist.
        ValueError: If the YAML content is not a dict.
    """
    global _cache
    if path is None and _cache is not None:
        return _cache

    resolved = path or _DEFAULT_PRESETS_PATH
    if not resolved.exists():
        raise FileNotFoundError(f"Presets file not found: {resolved}")

    with open(resolved) as f:
        data = yaml.safe_load(f)

    if not isinstance(data, dict):
        raise ValueError(f"Presets file must contain a YAML mapping, got {type(data).__name__}")

    if path is None:
        _cache = data

    return data


def reload_presets(path: Path | None = None) -> dict[str, Any]:
    """Force-reload the presets file (clears cache first)."""
    global _cache
    _cache = None
    return load_presets(path)


# ---------------------------------------------------------------------------
# Query helpers
# ---------------------------------------------------------------------------


def get_provider_preset(provider_id: str, path: Path | None = None) -> dict[str, Any]:
    """Return the preset dict for a single provider.

    Raises:
        ValueError: If *provider_id* is not found in the presets file.
    """
    presets = load_presets(path)
    providers: dict[str, Any] = presets.get("providers", {})
    entry = providers.get(provider_id)
    if entry is None:
        available = ", ".join(sorted(providers.keys())) or "(none)"
        raise ValueError(
            f"Unknown provider preset {provider_id!r}. Available: {available}"
        )
    return entry


def list_provider_presets(path: Path | None = None) -> list[dict[str, Any]]:
    """Return all provider presets as a list of dicts."""
    presets = load_presets(path)
    return list(presets.get("providers", {}).values())


def list_models_for_provider(provider_id: str, path: Path | None = None) -> list[dict[str, Any]]:
    """Return all model presets belonging to *provider_id*."""
    presets = load_presets(path)
    models: list[dict[str, Any]] = presets.get("models", [])
    return [m for m in models if m.get("provider") == provider_id]


def list_all_model_presets(path: Path | None = None) -> list[dict[str, Any]]:
    """Return every model preset from the presets file."""
    presets = load_presets(path)
    return list(presets.get("models", []))


def get_model_preset(model_id: str, path: Path | None = None) -> dict[str, Any] | None:
    """Look up a model preset by *model_id*.  Returns ``None`` if not found."""
    presets = load_presets(path)
    for m in presets.get("models", []):
        if m.get("id") == model_id:
            return m
    return None


# ---------------------------------------------------------------------------
# Resolve --preset + --model into config values
# ---------------------------------------------------------------------------


def resolve_preset(
    preset: str,
    model: str | None = None,
    path: Path | None = None,
) -> dict[str, Any]:
    """Resolve a ``--preset`` (and optional ``--model``) into LLM config values.

    Returns a dict with keys:
        - ``provider`` (str): The protocol to use (e.g. ``"openai"``).
        - ``model`` (str): The model name (from *model* arg or the first model
          for the provider in the presets file).
        - ``base_url`` (str): The API base URL.
        - ``api_key_env`` (str | None): Environment variable name for the API key.
        - ``api_key_required`` (bool): Whether an API key is required.

    Raises:
        ValueError: If *preset* is not found in the presets file.
    """
    provider_cfg = get_provider_preset(preset, path)

    resolved_model = model
    if resolved_model is None:
        # Pick the first model listed for this provider
        models = list_models_for_provider(preset, path)
        if models:
            resolved_model = models[0]["id"]
        else:
            resolved_model = provider_cfg.get("id", "")

    return {
        "provider": provider_cfg.get("protocol", "openai"),
        "model": resolved_model,
        "base_url": provider_cfg.get("base_url", ""),
        "api_key_env": provider_cfg.get("api_key_env"),
        "api_key_required": provider_cfg.get("api_key_required", False),
    }
