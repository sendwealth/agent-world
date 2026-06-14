"""Tests for federation think-loop integration (SEN-712).

Covers:
- FederationSyncConfig defaults
- FederationSync peer discovery via a stub client
- sync_interval_ticks gating (runs only on interval boundaries)
- self-world exclusion from discovered peers
- graceful degradation when list_worlds raises
- disabled config is a no-op
- load_federation_config_from_genesis parses bootstrap_peers / enabled
- build_federation_sync returns None when disabled, a hook when enabled
- ThinkLoop invokes the federation hook each tick
"""

from __future__ import annotations

from typing import Any

from agent_runtime.core.act import ActionExecutor
from agent_runtime.core.think_loop import ThinkLoop, ThinkLoopConfig
from agent_runtime.federation import (
    FederationClient,
    FederationSync,
    FederationSyncConfig,
    build_federation_sync,
    load_federation_config_from_genesis,
)
from agent_runtime.models.agent_state import AgentState
from agent_runtime.survival.instinct import SurvivalInstinct

# ---------------------------------------------------------------------------
# Stubs
# ---------------------------------------------------------------------------


class StubFederationClient:
    """Drop-in for FederationClient.list_worlds with controllable behaviour."""

    def __init__(self, worlds: list[dict[str, Any]] | None = None, raises: bool = False):
        self._worlds = worlds if worlds is not None else []
        self._raises = raises
        self.list_calls = 0

    def list_worlds(self) -> list[dict[str, Any]]:
        self.list_calls += 1
        if self._raises:
            raise RuntimeError("simulated registry outage")
        return list(self._worlds)


def _enabled_cfg(**overrides: Any) -> FederationSyncConfig:
    base: dict[str, Any] = {
        "enabled": True,
        "world_id": "agent-world-v1",
        "bootstrap_peers": [],
        "sync_interval_ticks": 1,
    }
    base.update(overrides)
    return FederationSyncConfig(**base)


# ---------------------------------------------------------------------------
# FederationSyncConfig
# ---------------------------------------------------------------------------


def test_default_config_is_disabled():
    """Phase 1 default must be disabled — no behaviour change."""
    cfg = FederationSyncConfig()
    assert cfg.enabled is False
    assert cfg.bootstrap_peers == []
    assert cfg.sync_interval_ticks == 50


# ---------------------------------------------------------------------------
# Peer discovery
# ---------------------------------------------------------------------------


async def test_sync_discovers_peers():
    client = StubFederationClient(
        worlds=[
            {"world_id": "agent-world-v1", "name": "self"},
            {"world_id": "world-b", "name": "Peer B"},
            {"world_id": "world-c", "name": "Peer C"},
        ]
    )
    sync = FederationSync(client=client, config=_enabled_cfg(sync_interval_ticks=1))

    await sync.sync(tick=1)

    ids = [w["world_id"] for w in sync.discovered_peers]
    assert ids == ["world-b", "world-c"], "own world must be excluded"
    assert sync.last_sync_tick == 1


async def test_sync_respects_interval():
    client = StubFederationClient(worlds=[{"world_id": "w2"}])
    sync = FederationSync(client=client, config=_enabled_cfg(sync_interval_ticks=5))

    # Ticks 1-4 are off-interval → no discovery.
    for tick in range(1, 5):
        await sync.sync(tick=tick)
    assert client.list_calls == 0
    assert sync.last_sync_tick == -1

    # Tick 5 is on-interval.
    await sync.sync(tick=5)
    assert client.list_calls == 1
    assert sync.last_sync_tick == 5
    assert len(sync.discovered_peers) == 1


async def test_sync_disabled_is_noop():
    client = StubFederationClient(worlds=[{"world_id": "w2"}])
    cfg = _enabled_cfg(enabled=False)
    sync = FederationSync(client=client, config=cfg)

    await sync.sync(tick=1)

    assert client.list_calls == 0
    assert sync.discovered_peers == []
    assert sync.last_sync_tick == -1


async def test_sync_swallows_client_errors():
    """A failing registry must never propagate into the think loop."""
    client = StubFederationClient(raises=True)
    sync = FederationSync(client=client, config=_enabled_cfg(sync_interval_ticks=1))

    # Should not raise.
    await sync.sync(tick=1)

    assert client.list_calls == 1
    assert sync.last_sync_tick == -1, "failed sync must not update last_sync_tick"
    assert sync.discovered_peers == []


# ---------------------------------------------------------------------------
# Genesis loader
# ---------------------------------------------------------------------------


def test_genesis_loader_enabled_flag(tmp_path):
    genesis = tmp_path / "genesis.yaml"
    genesis.write_text(
        "federation:\n"
        "  enabled: true\n"
        '  world_id: "my-world"\n'
        "  bootstrap_peers:\n"
        "    - http://peer-a:8080\n"
        "    - http://peer-b:8080\n"
        "  sync_interval_ticks: 25\n"
    )
    cfg = load_federation_config_from_genesis(genesis)
    assert cfg.enabled is True
    assert cfg.world_id == "my-world"
    assert cfg.bootstrap_peers == ["http://peer-a:8080", "http://peer-b:8080"]
    assert cfg.sync_interval_ticks == 25


def test_genesis_loader_peers_imply_enabled(tmp_path):
    """Non-empty bootstrap_peers should enable federation even without a flag."""
    genesis = tmp_path / "genesis.yaml"
    genesis.write_text(
        "federation:\n  bootstrap_peers:\n    - http://peer:8080\n"
    )
    cfg = load_federation_config_from_genesis(genesis)
    assert cfg.enabled is True
    assert cfg.bootstrap_peers == ["http://peer:8080"]


def test_genesis_loader_disabled_default(tmp_path):
    genesis = tmp_path / "genesis.yaml"
    genesis.write_text("federation:\n  enabled: false\n  bootstrap_peers: []\n")
    cfg = load_federation_config_from_genesis(genesis)
    assert cfg.enabled is False


def test_genesis_loader_missing_file_is_disabled(tmp_path):
    cfg = load_federation_config_from_genesis(tmp_path / "nope.yaml")
    assert cfg.enabled is False


def test_genesis_loader_missing_section_is_disabled(tmp_path):
    genesis = tmp_path / "genesis.yaml"
    genesis.write_text("world:\n  name: x\n")
    cfg = load_federation_config_from_genesis(genesis)
    assert cfg.enabled is False


# ---------------------------------------------------------------------------
# build_federation_sync
# ---------------------------------------------------------------------------


def test_build_returns_none_when_disabled(tmp_path, monkeypatch):
    genesis = tmp_path / "genesis.yaml"
    genesis.write_text("federation:\n  enabled: false\n  bootstrap_peers: []\n")
    monkeypatch.setenv("AGENT_WORLD_GENESIS", str(genesis))
    assert build_federation_sync("http://localhost:8080") is None


def test_build_returns_hook_when_enabled(tmp_path, monkeypatch):
    genesis = tmp_path / "genesis.yaml"
    genesis.write_text(
        "federation:\n  enabled: true\n  world_id: 'w1'\n"
        "  bootstrap_peers:\n    - http://peer:8080\n"
    )
    monkeypatch.setenv("AGENT_WORLD_GENESIS", str(genesis))
    hook = build_federation_sync("http://localhost:8080")
    assert hook is not None
    assert isinstance(hook, FederationSync)
    assert isinstance(hook._client, FederationClient)
    assert hook.config.world_id == "w1"
    assert hook.config.bootstrap_peers == ["http://peer:8080"]
    hook.close()


# ---------------------------------------------------------------------------
# ThinkLoop integration
# ---------------------------------------------------------------------------


class RecordingFederationHook:
    """Minimal FederationHook implementation that records every sync call."""

    def __init__(self) -> None:
        self.ticks: list[int] = []

    async def sync(self, tick: int) -> None:
        self.ticks.append(tick)


def _make_state() -> AgentState:
    return AgentState(name="FedAgent", max_tokens=1000, tokens=500)


async def test_think_loop_calls_federation_hook_each_tick():
    hook = RecordingFederationHook()
    loop = ThinkLoop(
        state=_make_state(),
        survival=SurvivalInstinct(),
        executor=ActionExecutor(),
        config=ThinkLoopConfig(tick_interval=0.0, max_ticks=3),
        federation_hook=hook,
    )
    await loop.run()
    # The hook is invoked once per tick (step 0c), before the cycle gates
    # that might stop the loop. 3 ticks → 3 calls.
    assert hook.ticks == [1, 2, 3], f"expected sync on ticks 1-3, got {hook.ticks}"
    assert loop.tick == 3


async def test_think_loop_without_hook_runs_unchanged():
    """Omitting federation_hook must not change loop behaviour (Phase 1)."""
    loop = ThinkLoop(
        state=_make_state(),
        survival=SurvivalInstinct(),
        executor=ActionExecutor(),
        config=ThinkLoopConfig(tick_interval=0.0, max_ticks=2),
    )
    await loop.run()
    assert loop.tick == 2
    assert loop.total_errors == 0


async def test_think_loop_hook_failure_is_non_fatal():
    """A raising hook must not count as a think-loop error."""

    class BoomHook:
        async def sync(self, tick: int) -> None:
            raise RuntimeError("boom")

    loop = ThinkLoop(
        state=_make_state(),
        survival=SurvivalInstinct(),
        executor=ActionExecutor(),
        config=ThinkLoopConfig(tick_interval=0.0, max_ticks=2),
        federation_hook=BoomHook(),
    )
    await loop.run()
    assert loop.tick == 2
    assert loop.total_errors == 0, "hook failure must be swallowed"
