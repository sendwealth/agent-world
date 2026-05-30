"""Tests for new context modules: budget.py, processors.py, and ContextEngine.

Covers:
- TokenBudget with safety_margin
- PipelineConfig with safety_margin
- KeywordMatcher scoring
- TimeDecayCalculator decay
- RelevanceScorer combined scoring
- ContextProcessor ranking
- ContextEngine.build_context()
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any

import pytest

from agent_runtime.context.budget import PipelineConfig, TokenBudget
from agent_runtime.context.engine import (
    ContextEngine,
    ContextEnginePipeline,
    ContextItem,
    ContextPriority,
    ContextSource,
)
from agent_runtime.context.processors import (
    ContextProcessor,
    KeywordMatcher,
    RelevanceScore,
    RelevanceScorer,
    TimeDecayCalculator,
)

# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class FakeState:
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


def _make_item(
    content: str,
    *,
    source: ContextSource = ContextSource.PERCEPTION,
    priority: ContextPriority = ContextPriority.P1_MISSION,
    token_estimate: int = 0,
    protected: bool = False,
    metadata: dict[str, Any] | None = None,
) -> ContextItem:
    return ContextItem(
        content=content,
        source=source,
        priority=priority,
        token_estimate=token_estimate,
        protected=protected,
        metadata=metadata or {},
    )


# ===========================================================================
# Test: TokenBudget with safety_margin
# ===========================================================================


class TestTokenBudgetSafetyMargin:
    def test_default_safety_margin_is_zero(self) -> None:
        budget = TokenBudget(max_tokens=1000)
        assert budget.safety_margin == 0

    def test_safety_margin_reduces_effective_budget(self) -> None:
        budget = TokenBudget(max_tokens=200, safety_margin=100)
        items = [
            _make_item("a", token_estimate=80),
            _make_item("b", token_estimate=80),
        ]
        result, trimmed, _ = budget.allocate(items)
        # effective budget is 200 - 100 = 100
        # Only first item (80 tokens) fits in 100
        assert trimmed == 1
        assert len(result) == 1

    def test_safety_margin_with_protected_items(self) -> None:
        budget = TokenBudget(max_tokens=200, safety_margin=50)
        items = [
            _make_item("protected", source=ContextSource.SURVIVAL,
                        priority=ContextPriority.P0_SURVIVAL, protected=True,
                        token_estimate=50),
            _make_item("regular", token_estimate=60),
            _make_item("extra", token_estimate=60),
        ]
        result, trimmed, overflow = budget.allocate(items)
        # effective = 200 - 50 = 150
        # protected uses 50, remaining = 100
        # One regular item (60) fits, second (60) would push to 120 > 100
        assert len([i for i in result if i.protected]) == 1
        assert trimmed == 1

    def test_full_safety_margin_blocks_all_regular(self) -> None:
        budget = TokenBudget(max_tokens=50, safety_margin=50)
        items = [
            _make_item("protected", source=ContextSource.SURVIVAL,
                        priority=ContextPriority.P0_SURVIVAL, protected=True,
                        token_estimate=10),
            _make_item("regular", token_estimate=5),
        ]
        result, trimmed, overflow = budget.allocate(items)
        # effective = 50 - 50 = 0
        # Protected (10) > 0 → overflow
        assert overflow is True
        assert len([i for i in result if i.protected]) == 1
        assert trimmed == 1


class TestPipelineConfigSafetyMargin:
    def test_default_safety_margin_is_100(self) -> None:
        config = PipelineConfig()
        assert config.safety_margin == 100

    def test_custom_safety_margin(self) -> None:
        config = PipelineConfig(max_tokens=2000, safety_margin=200)
        assert config.max_tokens == 2000
        assert config.safety_margin == 200

    def test_env_override_does_not_affect_safety_margin(
        self, monkeypatch: pytest.MonkeyPatch,
    ) -> None:
        monkeypatch.setenv("CONTEXT_MAX_TOKENS", "2048")
        config = PipelineConfig(safety_margin=50)
        assert config.max_tokens == 2048
        assert config.safety_margin == 50


# ===========================================================================
# Test: KeywordMatcher
# ===========================================================================


class TestKeywordMatcher:
    def test_perfect_overlap(self) -> None:
        score = KeywordMatcher.score("trade resources", "trade resources for gold")
        assert score == 1.0

    def test_partial_overlap(self) -> None:
        score = KeywordMatcher.score("trade gold", "trade resources for silver")
        assert 0.0 < score < 1.0
        assert score == 0.5  # "trade" matches, "gold" doesn't

    def test_no_overlap(self) -> None:
        score = KeywordMatcher.score("build house", "trade resources for gold")
        assert score == 0.0

    def test_empty_query(self) -> None:
        score = KeywordMatcher.score("", "some content")
        assert score == 0.0

    def test_empty_content(self) -> None:
        score = KeywordMatcher.score("query", "")
        assert score == 0.0

    def test_case_insensitive(self) -> None:
        score = KeywordMatcher.score("TRADE", "trade resources")
        assert score == 1.0


# ===========================================================================
# Test: TimeDecayCalculator
# ===========================================================================


class TestTimeDecayCalculator:
    def test_recent_item_has_high_decay(self) -> None:
        calc = TimeDecayCalculator(half_life_ticks=500)
        decay = calc.decay(current_tick=100, item_tick=100)
        assert decay == 1.0

    def test_old_item_has_low_decay(self) -> None:
        calc = TimeDecayCalculator(half_life_ticks=500)
        decay = calc.decay(current_tick=10000, item_tick=0)
        assert decay < 0.01

    def test_half_life(self) -> None:
        calc = TimeDecayCalculator(half_life_ticks=500)
        decay = calc.decay(current_tick=500, item_tick=0)
        assert abs(decay - 0.5) < 0.01

    def test_negative_elapsed_clamps_to_zero(self) -> None:
        calc = TimeDecayCalculator(half_life_ticks=500)
        decay = calc.decay(current_tick=0, item_tick=100)
        assert decay == 1.0

    def test_invalid_half_life_raises(self) -> None:
        with pytest.raises(ValueError):
            TimeDecayCalculator(half_life_ticks=0)
        with pytest.raises(ValueError):
            TimeDecayCalculator(half_life_ticks=-1)


# ===========================================================================
# Test: RelevanceScorer
# ===========================================================================


class TestRelevanceScorer:
    def test_score_with_all_components(self) -> None:
        scorer = RelevanceScorer()
        result = scorer.score(
            query="trade gold",
            content="trade gold for resources",
            current_tick=100,
            item_tick=100,
            importance=0.9,
        )
        assert isinstance(result, RelevanceScore)
        assert result.keyword > 0
        assert result.time_decay == 1.0
        assert result.importance == 0.9
        assert result.total > 0

    def test_score_older_items_have_lower_total(self) -> None:
        scorer = RelevanceScorer()
        fresh = scorer.score("trade", "trade resources", 100, 100, 0.5)
        stale = scorer.score("trade", "trade resources", 1000, 0, 0.5)
        assert fresh.total > stale.total

    def test_score_higher_importance_increases_total(self) -> None:
        scorer = RelevanceScorer()
        low = scorer.score("query", "content", 100, 100, 0.1)
        high = scorer.score("query", "content", 100, 100, 0.9)
        assert high.total > low.total

    def test_score_keyword_match_dominates(self) -> None:
        scorer = RelevanceScorer(keyword_weight=0.8, time_decay_weight=0.1, importance_weight=0.1)
        match = scorer.score("trade", "trade resources", 100, 100, 0.0)
        no_match = scorer.score("trade", "build houses", 100, 100, 1.0)
        # Even with 0 importance, keyword match should score higher
        assert match.keyword > no_match.keyword


class TestRelevanceScorerRankItems:
    def test_rank_items_by_relevance(self) -> None:
        scorer = RelevanceScorer()
        items = [
            _make_item("build a house", metadata={"importance": 0.5}),
            _make_item("trade gold for resources", metadata={"importance": 0.5}),
            _make_item("explore the map", metadata={"importance": 0.5}),
        ]
        ranked = scorer.rank_items(items, query="trade gold", current_tick=100)
        assert ranked[0][0].content == "trade gold for resources"

    def test_rank_empty_list(self) -> None:
        scorer = RelevanceScorer()
        ranked = scorer.rank_items([], query="test", current_tick=100)
        assert ranked == []


# ===========================================================================
# Test: ContextProcessor
# ===========================================================================


class TestContextProcessor:
    def test_process_reorders_by_relevance(self) -> None:
        processor = ContextProcessor()
        items = [
            _make_item("build a house"),
            _make_item("trade gold"),
            _make_item("explore"),
        ]
        result = processor.process(items, query="trade gold", current_tick=100)
        # "trade gold" should be ranked first among regular items
        regular = [i for i in result if not i.protected]
        assert regular[0].content == "trade gold"

    def test_process_keeps_protected_first(self) -> None:
        processor = ContextProcessor()
        items = [
            _make_item("regular task"),
            _make_item("SURVIVAL!", source=ContextSource.SURVIVAL,
                        priority=ContextPriority.P0_SURVIVAL, protected=True),
            _make_item("another task"),
        ]
        result = processor.process(items, query="task", current_tick=100)
        assert result[0].protected is True
        assert result[0].content == "SURVIVAL!"

    def test_process_empty_list(self) -> None:
        processor = ContextProcessor()
        result = processor.process([], query="test", current_tick=100)
        assert result == []

    def test_process_all_protected(self) -> None:
        processor = ContextProcessor()
        items = [
            _make_item("p1", source=ContextSource.SURVIVAL,
                        priority=ContextPriority.P0_SURVIVAL, protected=True),
            _make_item("p2", source=ContextSource.SURVIVAL,
                        priority=ContextPriority.P0_SURVIVAL, protected=True),
        ]
        result = processor.process(items, query="test", current_tick=100)
        assert len(result) == 2
        assert all(i.protected for i in result)


# ===========================================================================
# Test: ContextEngine
# ===========================================================================


class TestContextEngine:
    def test_build_context_returns_string(self) -> None:
        engine = ContextEngine(token_budget=2000)
        state = FakeState(phase=FakePhase())
        context = engine.build_context(agent_state=state)
        assert isinstance(context, str)

    def test_build_context_with_all_inputs(self) -> None:
        engine = ContextEngine(token_budget=4000)
        state = FakeState(phase=FakePhase(), skills={"trading": type("S", (), {"level": 3})()})

        @dataclass(frozen=True)
        class FakePerception:
            messages: list = None
            token_ratio: float = 0.5
            market_state: dict = None
            health: float = 100.0
            tick: int = 42

        perc = FakePerception(
            messages=[{"type": "INFORM", "payload": {"content": "Hello"}, "trust_score": 0.8}],
            market_state={"price": 42},
        )

        context = engine.build_context(
            agent_state=state,
            perception=perc,
            survival=None,
        )
        assert isinstance(context, str)
        assert len(context) > 0

    def test_build_context_respects_token_budget(self) -> None:
        engine = ContextEngine(token_budget=200)
        state = FakeState(phase=FakePhase())

        # Create lots of messages to overflow the budget
        @dataclass(frozen=True)
        class BigPerception:
            messages: list = None
            token_ratio: float = 0.5
            market_state: dict = None
            health: float = 100.0
            tick: int = 1

        msgs = [
            {
                "type": "INFORM",
                "payload": {"content": f"Message {i} " + "x" * 100},
                "trust_score": 0.5,
            }
            for i in range(50)
        ]
        perc = BigPerception(messages=msgs)

        context = engine.build_context(agent_state=state, perception=perc)
        # Budget is 200, safety margin is 100, so effective is 100
        # Context should be limited
        estimated_tokens = len(context) / 4
        assert estimated_tokens <= 250  # some slack for protected items

    def test_build_context_empty_inputs(self) -> None:
        engine = ContextEngine(token_budget=2000)
        context = engine.build_context()
        assert context == ""

    def test_build_context_with_working_memory(self) -> None:
        engine = ContextEngine(token_budget=4000)

        class FakeWorkingMemory:
            def read_all(self) -> list:
                return [type("Entry", (), {"content": "remembered something"})()]

        context = engine.build_context(working_memory=FakeWorkingMemory())
        assert isinstance(context, str)

    def test_default_token_budget_is_2000(self) -> None:
        engine = ContextEngine()
        assert engine.pipeline.config.max_tokens == 2000

    def test_pipeline_property_returns_pipeline(self) -> None:
        engine = ContextEngine(token_budget=1000)
        assert isinstance(engine.pipeline, ContextEnginePipeline)

    def test_build_context_with_pipeline_injection(self) -> None:
        config = PipelineConfig(max_tokens=500, safety_margin=50)
        pipeline = ContextEnginePipeline(config=config)
        engine = ContextEngine(pipeline=pipeline)
        state = FakeState(phase=FakePhase())
        context = engine.build_context(agent_state=state)
        assert isinstance(context, str)


# ===========================================================================
# Test: Pipeline with safety_margin
# ===========================================================================


class TestPipelineWithSafetyMargin:
    def test_pipeline_uses_safety_margin(self) -> None:
        config = PipelineConfig(max_tokens=200, safety_margin=100)
        pipeline = ContextEnginePipeline(config=config)

        # The pipeline's budget should have the safety margin set
        assert pipeline._budget.safety_margin == 100
        assert pipeline._budget.max_tokens == 200

    def test_pipeline_trims_within_effective_budget(self) -> None:
        config = PipelineConfig(max_tokens=300, safety_margin=100)
        pipeline = ContextEnginePipeline(config=config)

        # Create many messages
        msgs = [
            {"type": "INFORM", "payload": {"content": f"Msg {i} " + "x" * 50}, "trust_score": 0.5}
            for i in range(30)
        ]

        @dataclass(frozen=True)
        class FakePerception:
            messages: list = None
            token_ratio: float = 0.5
            market_state: dict = None
            health: float = 100.0
            tick: int = 1

        result = pipeline.run(perception=FakePerception(messages=msgs))
        # Effective budget is 300 - 100 = 200
        # Should be within 200 + protected items
        assert result.stats.final_token_count <= 250  # some slack


# ===========================================================================
# Test: Public API — new symbols importable
# ===========================================================================


class TestNewPublicAPI:
    def test_context_engine_importable(self) -> None:
        from agent_runtime.context import ContextEngine
        assert ContextEngine is not None

    def test_processors_importable(self) -> None:
        from agent_runtime.context import (
            ContextProcessor,
            KeywordMatcher,
            RelevanceScore,
            RelevanceScorer,
            TimeDecayCalculator,
        )
        symbols = [ContextProcessor, KeywordMatcher, RelevanceScore,
                    RelevanceScorer, TimeDecayCalculator]
        assert len(symbols) == 5
        assert len(set(id(s) for s in symbols)) == 5

    def test_budget_importable_from_budget_module(self) -> None:
        from agent_runtime.context.budget import PipelineConfig, TokenBudget
        assert TokenBudget is not None
        assert PipelineConfig is not None
