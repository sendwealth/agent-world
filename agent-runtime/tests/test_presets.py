"""Tests for the model preset store (agent_runtime.presets).

Covers:
- YAML loading and caching
- Provider preset lookups
- Model preset queries (list, filter, single)
- resolve_preset() mapping to LLM config values
- Error handling for missing / invalid presets
"""

from __future__ import annotations

from pathlib import Path

import pytest

from agent_runtime.presets import (
    get_model_preset,
    get_provider_preset,
    list_all_model_presets,
    list_models_for_provider,
    reload_presets,
    resolve_preset,
)

# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------

SAMPLE_PRESETS_YAML = """\
providers:
  ollama-local:
    id: ollama-local
    protocol: ollama
    base_url: "http://localhost:11434"
    display_name: "Ollama (Local)"
    api_key_required: false
  zhipu:
    id: zhipu
    protocol: openai
    base_url: "https://open.bigmodel.cn/api/paas/v4"
    display_name: "Zhipu AI"
    api_key_required: true
    api_key_env: "ZHIPU_API_KEY"

models:
  - id: qwen3:8b
    provider: ollama-local
    label: "Qwen3 8B"
    ctx: 32768
    recommended_for: ["决策", "通用"]
  - id: glm-5
    provider: zhipu
    label: "GLM-5"
    ctx: 128000
    recommended_for: ["决策", "推理"]
"""


@pytest.fixture
def presets_file(tmp_path: Path) -> Path:
    """Write a sample presets YAML and return its path."""
    p = tmp_path / "model-presets.yaml"
    p.write_text(SAMPLE_PRESETS_YAML)
    return p


# ---------------------------------------------------------------------------
# Loading
# ---------------------------------------------------------------------------


class TestLoadPresets:
    def test_loads_valid_yaml(self, presets_file: Path) -> None:
        data = reload_presets(presets_file)
        assert "providers" in data
        assert "models" in data

    def test_caches_by_default(self, presets_file: Path) -> None:
        reload_presets(presets_file)
        # Second call without path should return cached data
        from agent_runtime.presets import load_presets

        data = load_presets()
        assert "providers" in data

    def test_reload_clears_cache(self, presets_file: Path) -> None:
        # Load with explicit path (doesn't populate cache)
        reload_presets(presets_file)
        from agent_runtime.presets import _cache

        # Cache is not set when loading with explicit path
        assert _cache is None

    def test_missing_file_raises(self, tmp_path: Path) -> None:
        with pytest.raises(FileNotFoundError):
            reload_presets(tmp_path / "nonexistent.yaml")

    def test_invalid_content_raises(self, tmp_path: Path) -> None:
        p = tmp_path / "bad.yaml"
        p.write_text("- just\n- a\n- list\n")
        with pytest.raises(ValueError, match="YAML mapping"):
            reload_presets(p)


# ---------------------------------------------------------------------------
# Provider presets
# ---------------------------------------------------------------------------


class TestGetProviderPreset:
    def test_returns_existing_provider(self, presets_file: Path) -> None:
        reload_presets(presets_file)
        cfg = get_provider_preset("zhipu")
        assert cfg["base_url"] == "https://open.bigmodel.cn/api/paas/v4"
        assert cfg["api_key_required"] is True

    def test_raises_for_unknown_provider(self, presets_file: Path) -> None:
        reload_presets(presets_file)
        with pytest.raises(ValueError, match="Unknown provider preset"):
            get_provider_preset("nonexistent")

    def test_error_message_lists_available(self, presets_file: Path) -> None:
        reload_presets(presets_file)
        with pytest.raises(ValueError, match="ollama-local.*zhipu|zhipu.*ollama-local"):
            get_provider_preset("bad_name")


class TestListProviderPresets:
    def test_returns_all_providers(self, presets_file: Path) -> None:
        reload_presets(presets_file)
        from agent_runtime.presets import list_provider_presets

        result = list_provider_presets()
        ids = [p["id"] for p in result]
        assert "ollama-local" in ids
        assert "zhipu" in ids


# ---------------------------------------------------------------------------
# Model presets
# ---------------------------------------------------------------------------


class TestListModelPresets:
    def test_filters_by_provider(self, presets_file: Path) -> None:
        reload_presets(presets_file)
        models = list_models_for_provider("zhipu")
        assert len(models) == 1
        assert models[0]["id"] == "glm-5"

    def test_returns_empty_for_unknown_provider(self, presets_file: Path) -> None:
        reload_presets(presets_file)
        models = list_models_for_provider("unknown")
        assert models == []

    def test_list_all(self, presets_file: Path) -> None:
        reload_presets(presets_file)
        all_models = list_all_model_presets(presets_file)
        assert len(all_models) == 2


class TestGetModelPreset:
    def test_returns_matching_model(self, presets_file: Path) -> None:
        reload_presets(presets_file)
        m = get_model_preset("glm-5")
        assert m is not None
        assert m["provider"] == "zhipu"

    def test_returns_none_for_missing(self, presets_file: Path) -> None:
        reload_presets(presets_file)
        m = get_model_preset("does-not-exist")
        assert m is None


# ---------------------------------------------------------------------------
# resolve_preset
# ---------------------------------------------------------------------------


class TestResolvePreset:
    def test_resolves_zhipu_with_model(self, presets_file: Path) -> None:
        reload_presets(presets_file)
        result = resolve_preset("zhipu", "glm-5")
        assert result["provider"] == "openai"
        assert result["model"] == "glm-5"
        assert result["base_url"] == "https://open.bigmodel.cn/api/paas/v4"
        assert result["api_key_required"] is True
        assert result["api_key_env"] == "ZHIPU_API_KEY"

    def test_resolves_ollama_default_model(self, presets_file: Path) -> None:
        reload_presets(presets_file)
        result = resolve_preset("ollama-local")
        assert result["provider"] == "ollama"
        assert result["model"] == "qwen3:8b"
        assert result["api_key_required"] is False

    def test_raises_for_unknown_preset(self, presets_file: Path) -> None:
        reload_presets(presets_file)
        with pytest.raises(ValueError, match="Unknown provider preset"):
            resolve_preset("nonexistent")

    def test_explicit_model_overrides_default(self, presets_file: Path) -> None:
        reload_presets(presets_file)
        result = resolve_preset("ollama-local", "llama3:custom")
        assert result["model"] == "llama3:custom"
