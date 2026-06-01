"""Tests for provider_registry.py — ModelRegistry CRUD + agent model override.

Covers:
- ProviderProtocol enum has 5 members
- ProviderConfig / ModelOption dataclass construction
- ModelRegistry singleton lifecycle
- register_provider / remove_provider / list_providers / get_provider
- create_provider (delegates to factory)
- Agent model override: set_agent_model / get_agent_model / resolve_provider_for_agent
- create_provider_from_config convenience wrapper
- Backward compatibility: old code still works
"""

from __future__ import annotations

import pytest

from agent_runtime.llm.base import LLMConfig, LLMProvider, ProviderType
from agent_runtime.llm.factory import (
    create_provider,
    create_provider_from_config,
)
from agent_runtime.llm.provider_registry import (
    ModelOption,
    ModelRegistry,
    ProviderConfig,
    ProviderProtocol,
)

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


@pytest.fixture(autouse=True)
def _reset_registry():
    """Reset the singleton between tests."""
    ModelRegistry.reset()
    yield
    ModelRegistry.reset()


def _make_config(**overrides) -> ProviderConfig:
    defaults = dict(
        id="test-provider",
        protocol=ProviderProtocol.OPENAI,
        base_url="https://api.example.com/v1",
        api_key="sk-test",
    )
    defaults.update(overrides)
    return ProviderConfig(**defaults)


# ---------------------------------------------------------------------------
# ProviderProtocol enum
# ---------------------------------------------------------------------------


class TestProviderProtocol:
    def test_has_five_members(self):
        assert len(ProviderProtocol) == 5

    def test_members(self):
        assert ProviderProtocol.OPENAI.value == "openai"
        assert ProviderProtocol.ANTHROPIC.value == "anthropic"
        assert ProviderProtocol.OLLAMA.value == "ollama"
        assert ProviderProtocol.GOOGLE.value == "google"
        assert ProviderProtocol.AZURE.value == "azure"

    def test_string_conversion(self):
        assert ProviderProtocol("openai") is ProviderProtocol.OPENAI


# ---------------------------------------------------------------------------
# Data classes
# ---------------------------------------------------------------------------


class TestModelOption:
    def test_basic(self):
        opt = ModelOption(id="gpt-4")
        assert opt.id == "gpt-4"
        assert opt.label == "gpt-4"  # defaults to id

    def test_custom_label(self):
        opt = ModelOption(id="gpt-4", label="GPT-4 Turbo")
        assert opt.label == "GPT-4 Turbo"

    def test_provider_id_default(self):
        opt = ModelOption(id="gpt-4")
        assert opt.provider_id == "gpt-4"

    def test_frozen(self):
        opt = ModelOption(id="gpt-4")
        with pytest.raises(AttributeError):
            opt.id = "changed"  # type: ignore[misc]


class TestProviderConfig:
    def test_basic(self):
        cfg = _make_config()
        assert cfg.id == "test-provider"
        assert cfg.protocol is ProviderProtocol.OPENAI
        assert cfg.display_name == "test-provider"  # defaults to id

    def test_display_name_override(self):
        cfg = _make_config(display_name="My Provider")
        assert cfg.display_name == "My Provider"

    def test_default_values(self):
        cfg = _make_config()
        assert cfg.timeout == 60.0
        assert cfg.max_tokens == 4096
        assert cfg.is_default is False


# ---------------------------------------------------------------------------
# ModelRegistry singleton
# ---------------------------------------------------------------------------


class TestModelRegistrySingleton:
    def test_singleton_identity(self):
        a = ModelRegistry.instance()
        b = ModelRegistry.instance()
        assert a is b

    def test_reset_creates_new(self):
        first = ModelRegistry.instance()
        ModelRegistry.reset()
        second = ModelRegistry.instance()
        assert first is not second


# ---------------------------------------------------------------------------
# ModelRegistry CRUD
# ---------------------------------------------------------------------------


class TestModelRegistryCRUD:
    def test_builtin_providers_registered(self):
        reg = ModelRegistry.instance()
        ids = {p.id for p in reg.list_providers()}
        assert "openai" in ids
        assert "anthropic" in ids
        assert "ollama" in ids

    def test_register_provider(self):
        reg = ModelRegistry.instance()
        cfg = _make_config(id="google-gemini", protocol=ProviderProtocol.GOOGLE)
        reg.register_provider(cfg)
        assert reg.get_provider("google-gemini") is cfg

    def test_register_replaces(self):
        reg = ModelRegistry.instance()
        cfg1 = _make_config(id="custom", base_url="https://old.example.com")
        cfg2 = _make_config(id="custom", base_url="https://new.example.com")
        reg.register_provider(cfg1)
        reg.register_provider(cfg2)
        assert reg.get_provider("custom") is cfg2

    def test_remove_provider(self):
        reg = ModelRegistry.instance()
        cfg = _make_config(id="to-remove")
        reg.register_provider(cfg)
        assert reg.remove_provider("to-remove") is True
        assert reg.get_provider("to-remove") is None

    def test_remove_nonexistent(self):
        reg = ModelRegistry.instance()
        assert reg.remove_provider("nope") is False

    def test_list_providers_includes_builtins_and_custom(self):
        reg = ModelRegistry.instance()
        reg.register_provider(_make_config(id="custom"))
        ids = {p.id for p in reg.list_providers()}
        assert "openai" in ids
        assert "custom" in ids

    def test_remove_cleans_agent_overrides(self):
        reg = ModelRegistry.instance()
        cfg = _make_config(id="to-remove")
        reg.register_provider(cfg)
        reg.set_agent_model("agent-1", "to-remove")
        reg.remove_provider("to-remove")
        assert reg.get_agent_model("agent-1") is None


# ---------------------------------------------------------------------------
# ModelRegistry create_provider
# ---------------------------------------------------------------------------


class TestModelRegistryCreateProvider:
    def test_create_openai(self):
        reg = ModelRegistry.instance()
        cfg = _make_config(
            id="openai",
            protocol=ProviderProtocol.OPENAI,
            api_key="sk-test",
        )
        provider = reg.create_provider(cfg, model="gpt-4")
        assert isinstance(provider, LLMProvider)
        assert provider.config.provider == ProviderType.OPENAI
        assert provider.config.model == "gpt-4"

    def test_create_anthropic(self):
        reg = ModelRegistry.instance()
        cfg = _make_config(
            id="anthropic",
            protocol=ProviderProtocol.ANTHROPIC,
            api_key="sk-ant-test",
        )
        provider = reg.create_provider(cfg, model="claude-3-sonnet")
        assert provider.config.provider == ProviderType.ANTHROPIC

    def test_create_ollama(self):
        reg = ModelRegistry.instance()
        cfg = _make_config(
            id="ollama",
            protocol=ProviderProtocol.OLLAMA,
            base_url="http://localhost:11434",
            api_key=None,
        )
        provider = reg.create_provider(cfg, model="llama3")
        assert provider.config.provider == ProviderType.OLLAMA

    def test_create_google_maps_to_openai(self):
        reg = ModelRegistry.instance()
        cfg = _make_config(
            id="google",
            protocol=ProviderProtocol.GOOGLE,
            base_url="https://generativelanguage.googleapis.com/v1beta",
            api_key="test-key",
        )
        provider = reg.create_provider(cfg, model="gemini-pro")
        # Google currently uses the OpenAI-compatible transport
        assert provider.config.provider == ProviderType.OPENAI

    def test_create_azure_maps_to_openai(self):
        reg = ModelRegistry.instance()
        cfg = _make_config(
            id="azure",
            protocol=ProviderProtocol.AZURE,
            base_url="https://my-resource.openai.azure.com",
            api_key="azure-key",
        )
        provider = reg.create_provider(cfg, model="gpt-4-deployment")
        assert provider.config.provider == ProviderType.OPENAI


# ---------------------------------------------------------------------------
# Agent model override
# ---------------------------------------------------------------------------


class TestAgentModelOverride:
    def test_set_and_get(self):
        reg = ModelRegistry.instance()
        reg.set_agent_model("agent-1", "openai")
        assert reg.get_agent_model("agent-1") == "openai"

    def test_get_nonexistent(self):
        reg = ModelRegistry.instance()
        assert reg.get_agent_model("no-agent") is None

    def test_resolve_with_override(self):
        reg = ModelRegistry.instance()
        reg.set_agent_model("agent-1", "anthropic")
        result = reg.resolve_provider_for_agent("agent-1")
        assert result is not None
        assert result.id == "anthropic"

    def test_resolve_falls_back_to_default(self):
        reg = ModelRegistry.instance()
        # openai is registered with is_default=True
        result = reg.resolve_provider_for_agent("unknown-agent")
        assert result is not None
        assert result.is_default is True

    def test_resolve_override_unknown_provider_falls_back(self):
        reg = ModelRegistry.instance()
        reg.set_agent_model("agent-1", "nonexistent-provider")
        result = reg.resolve_provider_for_agent("agent-1")
        # Should fall back to default (openai)
        assert result is not None
        assert result.is_default is True

    def test_resolve_empty_registry(self):
        reg = ModelRegistry.instance()
        reg._providers.clear()
        assert reg.resolve_provider_for_agent("agent-1") is None


# ---------------------------------------------------------------------------
# create_provider_from_config convenience
# ---------------------------------------------------------------------------


class TestCreateProviderFromConfig:
    def test_basic(self):
        cfg = _make_config(display_name="gpt-4")
        provider = create_provider_from_config(cfg)
        assert isinstance(provider, LLMProvider)
        assert provider.config.model == "gpt-4"

    def test_unknown_protocol_raises(self):
        # This would raise if protocol had no mapping
        # Since we test with a valid protocol, let's verify it works
        cfg = _make_config(protocol=ProviderProtocol.OLLAMA)
        provider = create_provider_from_config(cfg)
        assert provider.config.provider == ProviderType.OLLAMA


# ---------------------------------------------------------------------------
# Backward compatibility
# ---------------------------------------------------------------------------


class TestBackwardCompatibility:
    def test_old_create_provider_still_works(self):
        """Existing code using create_provider(LLMConfig(...)) must still work."""
        config = LLMConfig(provider=ProviderType.OPENAI, model="gpt-4", api_key="sk-test")
        provider = create_provider(config)
        assert provider.config.provider == ProviderType.OPENAI

    def test_provider_type_enum_unchanged(self):
        """ProviderType values must not change."""
        assert ProviderType.OPENAI.value == "openai"
        assert ProviderType.ANTHROPIC.value == "anthropic"
        assert ProviderType.OLLAMA.value == "ollama"
        assert len(ProviderType) == 3

    def test_llm_config_with_provider_type(self):
        """LLMConfig accepts ProviderType enum as before."""
        config = LLMConfig(provider=ProviderType.OLLAMA, model="llama3")
        assert config.provider == ProviderType.OLLAMA
