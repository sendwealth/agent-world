"""Tests for the Context Engine Pipeline.

Covers:
- Single-source collection
- Multi-source priority ordering
- Token budget trimming (100 messages)
- Survival information is never filtered
"""

from __future__ import annotations

import os
from dataclasses import dataclass
from typing import Any

import pytest

from agent_runtime.context.engine import (
    ContextEnginePipeline,
    ContextItem,
    ContextPriority,
    ContextSource,
    DefaultMemorySource,
    DefaultPerceptionSource,
    DefaultStateSource,
    DefaultSurvivalSource,
    MemorySource,
    MessageFilter,
    PerceptionSource,
    PipelineConfig,
    PipelineResult,
    PipelineStats,
    StateSource,
    SurvivalSource,
    TokenBudget,
)


# ---------------------------------------------------------------------------
# Fixtures — lightweight fakes matching the real dataclass shapes
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class FakePerception:
    """Mimics ``core/think_loop.Perception``."""

    messages: list[dict[str, Any]] | None = None
    token_balance: int = 100
    token_ratio: float = 0.5
    market_state: dict[str, Any] | None = None
    active_task: str | None = None
    health: float = 100.0
    tick: int = 42


@dataclass(frozen=True)
class FakeSurvivalAction:
    """Mimics ``survival/instinct.SurvivalAction``."""

    mode: Any = None
    token_ratio: float = 0.5
    actions: list[Any] | None = None


@dataclass(frozen=True)
class FakeEmergencyAction:
    reason: str = "test action"


class FakeMode:
    value = "normal"


@dataclass(frozen=True)
class FakeState:
    """Mimics ``models/agent_state.AgentState``."""

    name: str = "TestAgent"
    health: float = 100.0
    tokens: int = 500
    max_tokens: int = 1000
    money: float = 50.0
    reputation: float = 10.0
    phase: Any = None
    skills: dict[str, Any] | None = None


class FakePhase:
    value = "adult"


@dataclass(frozen=True)
class FakeRecalledMemory:
    content: str
    memory_type: str = "experience"
    relevance: float = 0.8
    importance: float = 0.5


# ---------------------------------------------------------------------------
# Helper
# ---------------------------------------------------------------------------


def _make_items(n: int, priority: ContextPriority, source: ContextSource) -> list[ContextItem]:
    return [
        ContextItem(
            content=f"Item {i} " + "x" * 20,
            source=source,
            priority=priority,
        )
        for i in range(n)
    ]


# ===========================================================================
# Test: Single-source collection
# ===========================================================================


class TestPerceptionSource:
    def test_collect_messages(self) -> None:
        perc = FakePerception(
            messages=[
                {"type": "INFORM", "payload": {"content": "Hello"}, "trust_score": 0.8},
                {"type": "PROPOSE", "payload": {"content": "Deal?"}, "trust_score": 0.5},
            ]
        )
        src = DefaultPerceptionSource()
        items = src.collect(perc)
        assert len(items) >= 2
        social_items = [i for i in items if i.priority == ContextPriority.P2_SOCIAL]
        task_items = [i for i in items if i.priority == ContextPriority.P1_MISSION]
        assert len(social_items) >= 1
        assert len(task_items) >= 1

    def test_collect_market(self) -> None:
        perc = FakePerception(market_state={"price": 42})
        src = DefaultPerceptionSource()
        items = src.collect(perc)
        market_items = [i for i in items if "Market state" in i.content]
        assert len(market_items) == 1

    def test_collect_tick_summary(self) -> None:
        perc = FakePerception(tick=10, health=90.0, token_ratio=0.6)
        src = DefaultPerceptionSource()
        items = src.collect(perc)
        tick_items = [i for i in items if "Tick 10" in i.content]
        assert len(tick_items) == 1


class TestSurvivalSource:
    def test_collect_survival(self) -> None:
        surv = FakeSurvivalAction(
            mode=FakeMode(),
            token_ratio=0.15,
            actions=[FakeEmergencyAction(reason="seek income")],
        )
        src = DefaultSurvivalSource()
        items = src.collect(surv)
        assert len(items) == 2  # mode summary + 1 action
        assert all(i.source == ContextSource.SURVIVAL for i in items)
        assert all(i.priority == ContextPriority.P0_SURVIVAL for i in items)
        assert all(i.protected for i in items)

    def test_collect_no_actions(self) -> None:
        surv = FakeSurvivalAction(mode=FakeMode(), token_ratio=0.5, actions=[])
        src = DefaultSurvivalSource()
        items = src.collect(surv)
        assert len(items) == 1


class TestStateSource:
    def test_collect_state(self) -> None:
        state = FakeState(phase=FakePhase(), skills={"trading": type("S", (), {"level": 3})()})
        src = DefaultStateSource()
        items = src.collect(state)
        assert len(items) == 2  # state summary + skills
        state_items = [i for i in items if i.source == ContextSource.STATE]
        assert len(state_items) == 2

    def test_critical_health_is_survival_priority(self) -> None:
        state = FakeState(health=20.0, tokens=500, max_tokens=1000, phase=FakePhase())
        src = DefaultStateSource()
        items = src.collect(state)
        summary = [i for i in items if "Agent state" in i.content][0]
        assert summary.priority == ContextPriority.P0_SURVIVAL

    def test_critical_tokens_is_survival_priority(self) -> None:
        state = FakeState(health=80.0, tokens=100, max_tokens=1000, phase=FakePhase())
        src = DefaultStateSource()
        items = src.collect(state)
        summary = [i for i in items if "Agent state" in i.content][0]
        assert summary.priority == ContextPriority.P0_SURVIVAL


class TestMemorySource:
    def test_collect_memories(self) -> None:
        mems = [
            FakeRecalledMemory(content="Learned trading", relevance=0.9),
            FakeRecalledMemory(content="Met Alice", memory_type="fact", relevance=0.6),
        ]
        src = DefaultMemorySource()
        items = src.collect(mems)
        assert len(items) == 2
        assert all(i.source == ContextSource.MEMORY for i in items)

    def test_collect_empty(self) -> None:
        src = DefaultMemorySource()
        items = src.collect([])
        assert items == []


# ===========================================================================
# Test: Multi-source priority ordering
# ===========================================================================


class TestMultiSourcePriority:
    def test_pipeline_collects_from_all_sources(self) -> None:
        pipeline = ContextEnginePipeline()
        result = pipeline.run(
            perception=FakePerception(tick=1, messages=[{"type": "INFORM", "payload": {"content": "Hi"}, "trust_score": 0.5}]),
            survival=FakeSurvivalAction(mode=FakeMode(), token_ratio=0.5, actions=[]),
            state=FakeState(phase=FakePhase()),
            memory=[FakeRecalledMemory(content="past event")],
        )
        sources_seen = {i.source for i in result.items}
        assert ContextSource.PERCEPTION in sources_seen
        assert ContextSource.SURVIVAL in sources_seen
        assert ContextSource.STATE in sources_seen
        assert ContextSource.MEMORY in sources_seen

    def test_survival_items_come_first(self) -> None:
        pipeline = ContextEnginePipeline(config=PipelineConfig(max_tokens=100000))
        result = pipeline.run(
            perception=FakePerception(tick=1),
            survival=FakeSurvivalAction(mode=FakeMode(), token_ratio=0.5, actions=[]),
            state=FakeState(phase=FakePhase()),
            memory=[FakeRecalledMemory(content="memory")],
        )
        # Protected P0 items should appear before non-P0
        priorities = [i.priority for i in result.items if i.protected]
        non_protected = [i.priority for i in result.items if not i.protected]
        if priorities and non_protected:
            assert max(priorities) <= min(non_protected)


# ===========================================================================
# Test: MessageFilter
# ===========================================================================


class TestMessageFilter:
    def test_survival_items_are_protected(self) -> None:
        items = [
            ContextItem(content="low health", source=ContextSource.SURVIVAL,
                        priority=ContextPriority.P0_SURVIVAL),
            ContextItem(content="casual chat", source=ContextSource.PERCEPTION,
                        priority=ContextPriority.P2_SOCIAL,
                        metadata={"trust_score": 0.5}),
        ]
        mf = MessageFilter()
        filtered = mf.filter(items)
        survival_item = filtered[0]
        assert survival_item.protected is True

    def test_critical_health_is_protected(self) -> None:
        items = [
            ContextItem(
                content="health check",
                source=ContextSource.STATE,
                priority=ContextPriority.P1_MISSION,
                metadata={"health": 10.0},
            ),
        ]
        mf = MessageFilter()
        filtered = mf.filter(items)
        assert filtered[0].protected is True

    def test_critical_token_ratio_is_protected(self) -> None:
        items = [
            ContextItem(
                content="token check",
                source=ContextSource.STATE,
                priority=ContextPriority.P1_MISSION,
                metadata={"token_ratio": 0.05},
            ),
        ]
        mf = MessageFilter()
        filtered = mf.filter(items)
        assert filtered[0].protected is True

    def test_social_sorted_by_trust_score(self) -> None:
        items = [
            ContextItem(content="low trust", source=ContextSource.PERCEPTION,
                        priority=ContextPriority.P2_SOCIAL,
                        metadata={"trust_score": 0.1}),
            ContextItem(content="high trust", source=ContextSource.PERCEPTION,
                        priority=ContextPriority.P2_SOCIAL,
                        metadata={"trust_score": 0.9}),
        ]
        mf = MessageFilter()
        filtered = mf.filter(items)
        social = [i for i in filtered if i.priority == ContextPriority.P2_SOCIAL]
        assert social[0].metadata["trust_score"] >= social[1].metadata["trust_score"]


# ===========================================================================
# Test: TokenBudget
# ===========================================================================


class TestTokenBudget:
    def test_trim_removes_low_priority(self) -> None:
        budget = TokenBudget(max_tokens=50)
        items = [
            ContextItem(content="P0 survival", source=ContextSource.SURVIVAL,
                        priority=ContextPriority.P0_SURVIVAL, protected=True,
                        token_estimate=10),
            ContextItem(content="P3 explore", source=ContextSource.PERCEPTION,
                        priority=ContextPriority.P3_EXPLORATION,
                        token_estimate=30),
            ContextItem(content="P1 mission", source=ContextSource.STATE,
                        priority=ContextPriority.P1_MISSION,
                        token_estimate=20),
        ]
        result, trimmed = budget.allocate(items)
        assert trimmed == 1
        # P0 (protected) + P1 (fits) kept; P3 dropped
        assert any("P0" in i.content for i in result)
        assert any("P1" in i.content for i in result)

    def test_protected_items_always_kept(self) -> None:
        budget = TokenBudget(max_tokens=10)
        items = [
            ContextItem(content="protected", source=ContextSource.SURVIVAL,
                        priority=ContextPriority.P0_SURVIVAL, protected=True,
                        token_estimate=8),
            ContextItem(content="extra", source=ContextSource.PERCEPTION,
                        priority=ContextPriority.P1_MISSION,
                        token_estimate=5),
        ]
        result, _ = budget.allocate(items)
        assert any(i.content == "protected" for i in result)
        assert not any(i.content == "extra" for i in result)

    def test_no_trim_when_under_budget(self) -> None:
        budget = TokenBudget(max_tokens=1000)
        items = [
            ContextItem(content="a", source=ContextSource.STATE,
                        priority=ContextPriority.P1_MISSION, token_estimate=10),
            ContextItem(content="b", source=ContextSource.MEMORY,
                        priority=ContextPriority.P1_MISSION, token_estimate=10),
        ]
        result, trimmed = budget.allocate(items)
        assert len(result) == 2
        assert trimmed == 0


# ===========================================================================
# Test: Token budget trimming with 100 messages
# ===========================================================================


class TestBudgetTrimming100Messages:
    """Simulate 100 messages and verify output is within budget."""

    def test_100_social_messages_trimmed(self) -> None:
        budget = TokenBudget(max_tokens=500)
        items = _make_items(100, ContextPriority.P2_SOCIAL, ContextSource.PERCEPTION)
        result, trimmed = budget.allocate(items)
        total_tokens = sum(i.token_estimate for i in result)
        assert total_tokens <= 500
        assert trimmed > 0

    def test_100_messages_pipeline(self) -> None:
        pipeline = ContextEnginePipeline(config=PipelineConfig(max_tokens=500))
        msgs = [
            {"type": "INFORM", "payload": {"content": f"Message {i} " + "x" * 40}, "trust_score": 0.5}
            for i in range(100)
        ]
        result = pipeline.run(
            perception=FakePerception(messages=msgs, tick=1),
            survival=FakeSurvivalAction(mode=FakeMode(), token_ratio=0.5, actions=[]),
            state=FakeState(phase=FakePhase()),
        )
        assert result.stats.final_token_count <= 500
        assert result.stats.total_items_collected >= 100
        assert result.stats.items_trimmed > 0


# ===========================================================================
# Test: Survival information is never filtered
# ===========================================================================


class TestSurvivalNotFiltered:
    def test_survival_items_survive_trim(self) -> None:
        """Even with a tiny budget, P0 protected items remain."""
        pipeline = ContextEnginePipeline(config=PipelineConfig(max_tokens=30))
        result = pipeline.run(
            perception=FakePerception(tick=1),
            survival=FakeSurvivalAction(
                mode=FakeMode(), token_ratio=0.05,
                actions=[FakeEmergencyAction(reason="SOS!")],
            ),
            state=FakeState(phase=FakePhase()),
            memory=_make_items(50, ContextPriority.P3_EXPLORATION, ContextSource.MEMORY),
        )
        survival_items = [i for i in result.items if i.source == ContextSource.SURVIVAL]
        assert len(survival_items) >= 1
        assert all(i.protected for i in survival_items)

    def test_critical_health_not_filtered(self) -> None:
        pipeline = ContextEnginePipeline(config=PipelineConfig(max_tokens=30))
        result = pipeline.run(
            perception=FakePerception(tick=1, health=10.0, token_ratio=0.1),
            state=FakeState(health=10.0, tokens=50, max_tokens=1000, phase=FakePhase()),
            memory=_make_items(50, ContextPriority.P3_EXPLORATION, ContextSource.MEMORY),
        )
        # The critical health item should be present
        health_items = [
            i for i in result.items
            if i.metadata.get("health") is not None and i.metadata["health"] < 30
        ]
        assert len(health_items) >= 1


# ===========================================================================
# Test: PipelineConfig
# ===========================================================================


class TestPipelineConfig:
    def test_default_max_tokens(self) -> None:
        config = PipelineConfig()
        assert config.max_tokens == 4096

    def test_env_override(self, monkeypatch: pytest.MonkeyPatch) -> None:
        monkeypatch.setenv("CONTEXT_MAX_TOKENS", "2048")
        config = PipelineConfig()
        assert config.max_tokens == 2048

    def test_invalid_env_uses_default(self, monkeypatch: pytest.MonkeyPatch) -> None:
        monkeypatch.setenv("CONTEXT_MAX_TOKENS", "not_a_number")
        config = PipelineConfig()
        assert config.max_tokens == 4096


# ===========================================================================
# Test: PipelineStats
# ===========================================================================


class TestPipelineStats:
    def test_stats_populated(self) -> None:
        pipeline = ContextEnginePipeline()
        result = pipeline.run(
            perception=FakePerception(tick=1),
            state=FakeState(phase=FakePhase()),
        )
        s = result.stats
        assert s.total_items_collected > 0
        assert s.total_tokens_collected > 0
        assert s.final_token_count > 0
        assert s.final_token_count <= pipeline.config.max_tokens


# ===========================================================================
# Test: ContextItem auto token estimation
# ===========================================================================


class TestContextItem:
    def test_auto_token_estimate(self) -> None:
        item = ContextItem(
            content="Hello world, this is a test message.",
            source=ContextSource.PERCEPTION,
            priority=ContextPriority.P1_MISSION,
        )
        assert item.token_estimate > 0

    def test_explicit_token_estimate(self) -> None:
        item = ContextItem(
            content="hello",
            source=ContextSource.PERCEPTION,
            priority=ContextPriority.P1_MISSION,
            token_estimate=42,
        )
        assert item.token_estimate == 42


# ===========================================================================
# Test: PipelineResult
# ===========================================================================


class TestPipelineResult:
    def test_formatted_context(self) -> None:
        pipeline = ContextEnginePipeline(config=PipelineConfig(max_tokens=10000))
        result = pipeline.run(
            perception=FakePerception(tick=1),
            survival=FakeSurvivalAction(mode=FakeMode(), token_ratio=0.5, actions=[]),
            state=FakeState(phase=FakePhase()),
        )
        assert isinstance(result.formatted_context, str)
        assert len(result.formatted_context) > 0
        # Items are separated by double newline
        assert "\n\n" in result.formatted_context or len(result.items) == 1


# ===========================================================================
# Test: Public API (all 13 symbols exported)
# ===========================================================================


class TestPublicAPI:
    def test_all_13_symbols_importable(self) -> None:
        from agent_runtime.context import (
            ContextEnginePipeline,
            ContextItem,
            ContextPriority,
            ContextSource,
            MemorySource,
            MessageFilter,
            PerceptionSource,
            PipelineConfig,
            PipelineResult,
            PipelineStats,
            StateSource,
            SurvivalSource,
            TokenBudget,
        )
        # Verify they are all distinct symbols
        symbols = [
            ContextEnginePipeline, ContextItem, ContextPriority, ContextSource,
            MemorySource, MessageFilter, PerceptionSource, PipelineConfig,
            PipelineResult, PipelineStats, StateSource, SurvivalSource, TokenBudget,
        ]
        assert len(symbols) == 13
        assert len(set(id(s) for s in symbols)) == 13
