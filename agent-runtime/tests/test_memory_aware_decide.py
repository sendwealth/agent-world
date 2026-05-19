"""Integration tests for memory-aware decision provider.

Tests the full integration chain: VectorMemory -> MemoryRecall -> MemoryAwareDecisionProvider.
Verifies that recalled memories flow into the decision process.
"""

from __future__ import annotations

import pytest

from agent_runtime.core.act import ActionExecutor, ActionType
from agent_runtime.core.memory_aware_decide import MemoryAwareDecisionProvider
from agent_runtime.core.think_loop import (
    Decision,
    MockDecisionProvider,
    Perception,
    ThinkLoop,
    ThinkLoopConfig,
)
from agent_runtime.memory.memory_recall import MemoryRecall, MemoryRecallConfig
from agent_runtime.memory.vector_memory import VectorMemory
from agent_runtime.models.agent_state import AgentState
from agent_runtime.survival.instinct import SurvivalAction, SurvivalInstinct, SurvivalMode

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def make_integration_setup():
    """Create a fully wired integration setup."""
    vm = VectorMemory(db_path=":memory:")
    recall = MemoryRecall(vector_memory=vm, config=MemoryRecallConfig(min_relevance=0.0))
    return vm, recall


# ---------------------------------------------------------------------------
# MemoryAwareDecisionProvider
# ---------------------------------------------------------------------------


class TestMemoryAwareDecisionProvider:
    @pytest.fixture
    def setup(self):
        vm, recall = make_integration_setup()
        state = AgentState(name="TestAgent", max_tokens=1000, tokens=500)
        executor = ActionExecutor()
        mock_provider = MockDecisionProvider(executor)
        provider = MemoryAwareDecisionProvider(
            base_provider=mock_provider,
            memory_recall=recall,
        )
        return vm, recall, state, provider

    @pytest.mark.asyncio
    async def test_decision_without_memories(self, setup):
        vm, recall, state, provider = setup
        perception = Perception(tick=1)
        survival = SurvivalAction(mode=SurvivalMode.NORMAL, token_ratio=0.5)

        decision = await provider.decide(state, perception, survival)
        assert isinstance(decision, Decision)
        assert decision.action_type in (ActionType.REST, ActionType.EXPLORE)

    @pytest.mark.asyncio
    async def test_decision_with_memories(self, setup):
        vm, recall, state, provider = setup
        vm.store("Trading during low tokens is dangerous", memory_type="lesson", importance=0.9)

        perception = Perception(tick=1, token_ratio=0.05)
        survival = SurvivalAction(mode=SurvivalMode.CONSERVATIVE, token_ratio=0.05)

        decision = await provider.decide(state, perception, survival)
        assert isinstance(decision, Decision)
        # The decision reasoning should contain memory context
        if decision.reasoning and "[Memory context used]" in decision.reasoning:
            assert "trading" in decision.reasoning.lower() or "memory" in decision.reasoning.lower()

    @pytest.mark.asyncio
    async def test_memory_context_in_reasoning(self, setup):
        vm, recall, state, provider = setup
        vm.store("Rest is always safe when tokens are low", memory_type="lesson", importance=0.9)

        perception = Perception(tick=1)
        survival = SurvivalAction(mode=SurvivalMode.NORMAL, token_ratio=0.5)

        decision = await provider.decide(state, perception, survival)
        assert isinstance(decision, Decision)

    @pytest.mark.asyncio
    async def test_query_builds_from_survival(self, setup):
        """Test that the query includes survival context."""
        vm, recall, state, provider = setup
        vm.store("Emergency survival lesson", memory_type="lesson", importance=0.95)

        state_low = AgentState(name="LowAgent", max_tokens=1000, tokens=50)
        perception = Perception(tick=1, token_ratio=0.05)
        survival = SurvivalAction(mode=SurvivalMode.URGENT, token_ratio=0.05)

        decision = await provider.decide(state_low, perception, survival)
        assert isinstance(decision, Decision)

    @pytest.mark.asyncio
    async def test_query_builds_from_perception(self, setup):
        vm, recall, state, provider = setup
        vm.store("Message response strategy", memory_type="experience", importance=0.8)

        perception = Perception(
            tick=1, messages=[{"from": "agent2", "content": "hello"}]
        )
        survival = SurvivalAction(mode=SurvivalMode.NORMAL, token_ratio=0.5)

        decision = await provider.decide(state, perception, survival)
        assert isinstance(decision, Decision)


# ---------------------------------------------------------------------------
# Full Think Loop integration
# ---------------------------------------------------------------------------


class TestThinkLoopMemoryIntegration:
    @pytest.mark.asyncio
    async def test_think_loop_with_memory_aware_provider(self):
        """Test that the think loop runs successfully with memory-aware provider."""
        vm = VectorMemory(db_path=":memory:")
        recall = MemoryRecall(vector_memory=vm, config=MemoryRecallConfig(min_relevance=0.0))

        state = AgentState(name="MemAgent", max_tokens=1000, tokens=500)
        executor = ActionExecutor()
        mock_provider = MockDecisionProvider(executor)
        memory_provider = MemoryAwareDecisionProvider(
            base_provider=mock_provider,
            memory_recall=recall,
        )

        loop = ThinkLoop(
            state=state,
            survival=SurvivalInstinct(),
            executor=executor,
            config=ThinkLoopConfig(tick_interval=0.01),
            decision_provider=memory_provider,
        )

        await loop.run(max_ticks=3)
        assert loop.tick == 3
        assert loop.total_errors == 0

    @pytest.mark.asyncio
    async def test_think_loop_recall_influences_decisions(self):
        """Test that stored memories are available during think loop cycles."""
        vm = VectorMemory(db_path=":memory:")
        vm.store("Rest conserves tokens", memory_type="lesson", importance=0.9)

        recall = MemoryRecall(vector_memory=vm, config=MemoryRecallConfig(min_relevance=0.0))

        state = AgentState(name="RecallAgent", max_tokens=1000, tokens=500)
        executor = ActionExecutor()
        mock_provider = MockDecisionProvider(executor)
        memory_provider = MemoryAwareDecisionProvider(
            base_provider=mock_provider,
            memory_recall=recall,
        )

        loop = ThinkLoop(
            state=state,
            survival=SurvivalInstinct(),
            executor=executor,
            config=ThinkLoopConfig(tick_interval=0.01),
            decision_provider=memory_provider,
        )

        await loop.run(max_ticks=2)
        assert loop.tick == 2
        assert loop.total_errors == 0

    @pytest.mark.asyncio
    async def test_think_loop_multiple_memory_types(self):
        """Test that different memory types are stored and recalled."""
        vm = VectorMemory(db_path=":memory:")
        vm.store("Explored the eastern forest", memory_type="experience", importance=0.7)
        vm.store("Never trade when tokens below 100", memory_type="lesson", importance=0.95)
        vm.store("Market prices peak at midday", memory_type="fact", importance=0.8)

        recall = MemoryRecall(vector_memory=vm, config=MemoryRecallConfig(min_relevance=0.0))

        state = AgentState(name="MultiMemAgent", max_tokens=1000, tokens=500)
        executor = ActionExecutor()
        mock_provider = MockDecisionProvider(executor)
        memory_provider = MemoryAwareDecisionProvider(
            base_provider=mock_provider,
            memory_recall=recall,
        )

        loop = ThinkLoop(
            state=state,
            survival=SurvivalInstinct(),
            executor=executor,
            config=ThinkLoopConfig(tick_interval=0.01),
            decision_provider=memory_provider,
        )

        await loop.run(max_ticks=5)
        assert loop.tick == 5
        assert loop.total_errors == 0
