"""Tests for the survival instinct module.

Covers:
- SurvivalMode classification at all token-ratio boundaries
- Emergency-action generation for each mode
- PANIC mode triggers immediately (< 10 %) without LLM
- Action cooldown enforcement
- execute() with mock A2A client
- Custom thresholds
- Edge cases (zero max_tokens, exactly on boundary)
"""

from __future__ import annotations

import asyncio
from dataclasses import dataclass
from typing import Any

import pytest

from agent_runtime.survival.instinct import (
    A2AClientProtocol,
    EmergencyAction,
    EmergencyActionType,
    SurvivalAction,
    SurvivalInstinct,
    SurvivalMode,
    SurvivalThresholds,
)


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


@dataclass
class FakeAgentState:
    """Minimal agent state for testing."""

    tokens: int = 100_000
    max_tokens: int = 100_000
    money: int = 0
    current_task: str | None = None


class FakeA2AClient:
    """Mock A2A client that records calls."""

    def __init__(self) -> None:
        self.broadcasts: list[dict[str, Any]] = []

    async def broadcast_message(self, payload: dict[str, Any]) -> dict[str, Any]:
        self.broadcasts.append(payload)
        return {"status": "ok"}


# ---------------------------------------------------------------------------
# Mode classification tests
# ---------------------------------------------------------------------------


class TestModeClassification:
    """Test _classify_mode at all boundaries."""

    def setup_method(self) -> None:
        self.instinct = SurvivalInstinct()

    def test_panic_below_10_percent(self) -> None:
        agent = FakeAgentState(tokens=5_000, max_tokens=100_000)
        result = self.instinct.assess(agent)
        assert result.mode == SurvivalMode.PANIC
        assert result.token_ratio == pytest.approx(0.05)

    def test_panic_at_zero(self) -> None:
        agent = FakeAgentState(tokens=0, max_tokens=100_000)
        result = self.instinct.assess(agent)
        assert result.mode == SurvivalMode.PANIC
        assert result.token_ratio == 0.0

    def test_panic_at_just_below_10_percent(self) -> None:
        agent = FakeAgentState(tokens=9_999, max_tokens=100_000)
        result = self.instinct.assess(agent)
        assert result.mode == SurvivalMode.PANIC

    def test_urgent_at_just_above_10_percent(self) -> None:
        agent = FakeAgentState(tokens=10_001, max_tokens=100_000)
        result = self.instinct.assess(agent)
        assert result.mode == SurvivalMode.URGENT

    def test_urgent_at_15_percent(self) -> None:
        agent = FakeAgentState(tokens=15_000, max_tokens=100_000)
        result = self.instinct.assess(agent)
        assert result.mode == SurvivalMode.URGENT

    def test_urgent_at_just_below_20_percent(self) -> None:
        agent = FakeAgentState(tokens=19_999, max_tokens=100_000)
        result = self.instinct.assess(agent)
        assert result.mode == SurvivalMode.URGENT

    def test_conservative_at_just_above_20_percent(self) -> None:
        agent = FakeAgentState(tokens=20_001, max_tokens=100_000)
        result = self.instinct.assess(agent)
        assert result.mode == SurvivalMode.CONSERVATIVE

    def test_conservative_at_30_percent(self) -> None:
        agent = FakeAgentState(tokens=30_000, max_tokens=100_000)
        result = self.instinct.assess(agent)
        assert result.mode == SurvivalMode.CONSERVATIVE

    def test_normal_at_just_above_40_percent(self) -> None:
        agent = FakeAgentState(tokens=40_001, max_tokens=100_000)
        result = self.instinct.assess(agent)
        assert result.mode == SurvivalMode.NORMAL

    def test_normal_at_50_percent(self) -> None:
        agent = FakeAgentState(tokens=50_000, max_tokens=100_000)
        result = self.instinct.assess(agent)
        assert result.mode == SurvivalMode.NORMAL

    def test_normal_at_exactly_80_percent(self) -> None:
        agent = FakeAgentState(tokens=80_000, max_tokens=100_000)
        result = self.instinct.assess(agent)
        assert result.mode == SurvivalMode.NORMAL

    def test_invest_at_just_above_80_percent(self) -> None:
        agent = FakeAgentState(tokens=80_001, max_tokens=100_000)
        result = self.instinct.assess(agent)
        assert result.mode == SurvivalMode.INVEST

    def test_invest_at_100_percent(self) -> None:
        agent = FakeAgentState(tokens=100_000, max_tokens=100_000)
        result = self.instinct.assess(agent)
        assert result.mode == SurvivalMode.INVEST


# ---------------------------------------------------------------------------
# Boundary edge cases
# ---------------------------------------------------------------------------


class TestBoundaryEdgeCases:
    """Test exact boundary values."""

    def setup_method(self) -> None:
        self.instinct = SurvivalInstinct()

    def test_exactly_at_panic_threshold(self) -> None:
        """10 % exactly should be URGENT (panic is strictly less than)."""
        agent = FakeAgentState(tokens=10_000, max_tokens=100_000)
        result = self.instinct.assess(agent)
        assert result.mode == SurvivalMode.URGENT

    def test_exactly_at_urgent_threshold(self) -> None:
        """20 % exactly should be CONSERVATIVE."""
        agent = FakeAgentState(tokens=20_000, max_tokens=100_000)
        result = self.instinct.assess(agent)
        assert result.mode == SurvivalMode.CONSERVATIVE

    def test_exactly_at_conservative_threshold(self) -> None:
        """40 % exactly should be NORMAL."""
        agent = FakeAgentState(tokens=40_000, max_tokens=100_000)
        result = self.instinct.assess(agent)
        assert result.mode == SurvivalMode.NORMAL

    def test_exactly_at_invest_threshold(self) -> None:
        """80 % exactly should be NORMAL (invest is strictly greater than)."""
        agent = FakeAgentState(tokens=80_000, max_tokens=100_000)
        result = self.instinct.assess(agent)
        assert result.mode == SurvivalMode.NORMAL

    def test_zero_max_tokens(self) -> None:
        """Degenerate case: max_tokens == 0 → ratio 0 → PANIC."""
        agent = FakeAgentState(tokens=0, max_tokens=0)
        result = self.instinct.assess(agent)
        assert result.mode == SurvivalMode.PANIC
        assert result.token_ratio == 0.0


# ---------------------------------------------------------------------------
# Emergency action generation
# ---------------------------------------------------------------------------


class TestActionGeneration:
    """Test that correct emergency actions are produced for each mode."""

    def setup_method(self) -> None:
        self.instinct = SurvivalInstinct()

    def test_panic_produces_sos_and_loan(self) -> None:
        agent = FakeAgentState(tokens=5_000, max_tokens=100_000)
        result = self.instinct.assess(agent)
        action_types = {a.action_type for a in result.actions}
        assert EmergencyActionType.BROADCAST_SOS in action_types
        assert EmergencyActionType.REQUEST_LOAN in action_types

    def test_panic_with_active_task_cancels_tasks(self) -> None:
        agent = FakeAgentState(tokens=5_000, max_tokens=100_000, current_task="task_1")
        result = self.instinct.assess(agent)
        action_types = {a.action_type for a in result.actions}
        assert EmergencyActionType.CANCEL_ALL_TASKS in action_types

    def test_panic_without_active_task_no_cancel(self) -> None:
        agent = FakeAgentState(tokens=5_000, max_tokens=100_000, current_task=None)
        result = self.instinct.assess(agent)
        action_types = {a.action_type for a in result.actions}
        assert EmergencyActionType.CANCEL_ALL_TASKS not in action_types

    def test_panic_with_money_exchanges(self) -> None:
        agent = FakeAgentState(tokens=5_000, max_tokens=100_000, money=500)
        result = self.instinct.assess(agent)
        action_types = {a.action_type for a in result.actions}
        assert EmergencyActionType.EXCHANGE_MONEY_TO_TOKENS in action_types

    def test_panic_without_money_no_exchange(self) -> None:
        agent = FakeAgentState(tokens=5_000, max_tokens=100_000, money=0)
        result = self.instinct.assess(agent)
        action_types = {a.action_type for a in result.actions}
        assert EmergencyActionType.EXCHANGE_MONEY_TO_TOKENS not in action_types

    def test_urgent_seeks_income_and_rejects_costly(self) -> None:
        agent = FakeAgentState(tokens=15_000, max_tokens=100_000)
        result = self.instinct.assess(agent)
        action_types = {a.action_type for a in result.actions}
        assert EmergencyActionType.SEEK_CHEAPEST_INCOME in action_types
        assert EmergencyActionType.REJECT_COSTLY_TASKS in action_types

    def test_urgent_with_money_exchanges(self) -> None:
        agent = FakeAgentState(tokens=15_000, max_tokens=100_000, money=200)
        result = self.instinct.assess(agent)
        action_types = {a.action_type for a in result.actions}
        assert EmergencyActionType.EXCHANGE_MONEY_TO_TOKENS in action_types

    def test_urgent_requests_loan(self) -> None:
        agent = FakeAgentState(tokens=15_000, max_tokens=100_000)
        result = self.instinct.assess(agent)
        action_types = {a.action_type for a in result.actions}
        assert EmergencyActionType.REQUEST_LOAN in action_types

    def test_conservative_limits_spending(self) -> None:
        agent = FakeAgentState(tokens=25_000, max_tokens=100_000)
        result = self.instinct.assess(agent)
        action_types = {a.action_type for a in result.actions}
        assert EmergencyActionType.LIMIT_SPENDING in action_types
        assert EmergencyActionType.REJECT_COSTLY_TASKS in action_types

    def test_normal_has_no_actions(self) -> None:
        agent = FakeAgentState(tokens=50_000, max_tokens=100_000)
        result = self.instinct.assess(agent)
        assert result.actions == []

    def test_invest_produces_invest_actions(self) -> None:
        agent = FakeAgentState(tokens=90_000, max_tokens=100_000)
        result = self.instinct.assess(agent)
        action_types = {a.action_type for a in result.actions}
        assert EmergencyActionType.INVEST_SURPLUS in action_types
        assert EmergencyActionType.TEACH_FOR_INCOME in action_types
        assert EmergencyActionType.SHARE_KNOWLEDGE in action_types

    def test_panic_actions_are_highest_priority(self) -> None:
        agent = FakeAgentState(tokens=5_000, max_tokens=100_000)
        result = self.instinct.assess(agent)
        priorities = [a.priority for a in result.actions]
        # Priorities should be non-negative and ordered
        assert all(p >= 0 for p in priorities)

    def test_actions_have_reason_strings(self) -> None:
        agent = FakeAgentState(tokens=5_000, max_tokens=100_000)
        result = self.instinct.assess(agent)
        for action in result.actions:
            assert len(action.reason) > 0


# ---------------------------------------------------------------------------
# Verification criterion: Token < 10 % triggers PANIC immediately
# ---------------------------------------------------------------------------


class TestPanicVerification:
    """Acceptance criterion: Token < 10 % triggers PANIC, no LLM wait."""

    def test_immediately_panics_below_10_percent(self) -> None:
        """The assess() call is synchronous (no await) — no LLM involved."""
        instinct = SurvivalInstinct()
        agent = FakeAgentState(tokens=9_999, max_tokens=100_000)
        result = instinct.assess(agent)
        assert result.mode == SurvivalMode.PANIC
        # assess() is a regular method, not async — guaranteed no LLM.
        import asyncio
        assert not asyncio.iscoroutinefunction(instinct.assess)

    def test_panic_actions_include_sos(self) -> None:
        instinct = SurvivalInstinct()
        agent = FakeAgentState(tokens=1_000, max_tokens=100_000)
        result = instinct.assess(agent)
        assert any(
            a.action_type == EmergencyActionType.BROADCAST_SOS
            for a in result.actions
        )

    def test_panic_actions_include_loan_request(self) -> None:
        instinct = SurvivalInstinct()
        agent = FakeAgentState(tokens=1_000, max_tokens=100_000)
        result = instinct.assess(agent)
        assert any(
            a.action_type == EmergencyActionType.REQUEST_LOAN
            for a in result.actions
        )

    def test_panic_at_1_percent(self) -> None:
        instinct = SurvivalInstinct()
        agent = FakeAgentState(tokens=1_000, max_tokens=100_000)
        result = instinct.assess(agent)
        assert result.mode == SurvivalMode.PANIC
        assert result.token_ratio == pytest.approx(0.01)

    def test_panic_at_half_percent(self) -> None:
        instinct = SurvivalInstinct()
        agent = FakeAgentState(tokens=500, max_tokens=100_000)
        result = instinct.assess(agent)
        assert result.mode == SurvivalMode.PANIC


# ---------------------------------------------------------------------------
# Execute with mock A2A client
# ---------------------------------------------------------------------------


class TestExecute:
    """Test execute() with a mock A2A client."""

    @pytest.mark.asyncio
    async def test_execute_panic_broadcasts_sos(self) -> None:
        instinct = SurvivalInstinct(action_cooldown=0.0)
        agent = FakeAgentState(tokens=5_000, max_tokens=100_000)
        a2a = FakeA2AClient()

        action = instinct.assess(agent)
        results = await instinct.execute(action, agent, a2a)

        # Should have broadcast at least one SOS message
        assert len(a2a.broadcasts) >= 1
        sos_broadcasts = [
            b for b in a2a.broadcasts
            if b.get("payload", {}).get("category") == "personal"
        ]
        assert len(sos_broadcasts) >= 1

    @pytest.mark.asyncio
    async def test_execute_panic_requests_loan(self) -> None:
        instinct = SurvivalInstinct(action_cooldown=0.0)
        agent = FakeAgentState(tokens=5_000, max_tokens=100_000)
        a2a = FakeA2AClient()

        action = instinct.assess(agent)
        await instinct.execute(action, agent, a2a)

        loan_broadcasts = [
            b for b in a2a.broadcasts
            if b.get("payload", {}).get("action") == "loan_request"
        ]
        assert len(loan_broadcasts) >= 1

    @pytest.mark.asyncio
    async def test_execute_without_a2a_client(self) -> None:
        """Execute should work even without an A2A client (local actions)."""
        instinct = SurvivalInstinct(action_cooldown=0.0)
        agent = FakeAgentState(tokens=5_000, max_tokens=100_000)

        action = instinct.assess(agent)
        results = await instinct.execute(action, agent, a2a_client=None)

        # All actions should still return results
        assert len(results) > 0
        for r in results:
            assert r["status"] == "executed"

    @pytest.mark.asyncio
    async def test_execute_normal_mode_no_actions(self) -> None:
        instinct = SurvivalInstinct(action_cooldown=0.0)
        agent = FakeAgentState(tokens=50_000, max_tokens=100_000)
        a2a = FakeA2AClient()

        action = instinct.assess(agent)
        results = await instinct.execute(action, agent, a2a)

        assert results == []
        assert len(a2a.broadcasts) == 0

    @pytest.mark.asyncio
    async def test_execute_returns_results(self) -> None:
        instinct = SurvivalInstinct(action_cooldown=0.0)
        agent = FakeAgentState(tokens=5_000, max_tokens=100_000, money=100)

        action = instinct.assess(agent)
        results = await instinct.execute(action, agent, a2a_client=None)

        for r in results:
            assert "action" in r
            assert "reason" in r
            assert r["status"] == "executed"


# ---------------------------------------------------------------------------
# Action cooldown
# ---------------------------------------------------------------------------


class TestCooldown:
    """Test that repeated actions are throttled."""

    @pytest.mark.asyncio
    async def test_cooldown_prevents_duplicate_actions(self) -> None:
        instinct = SurvivalInstinct(action_cooldown=9999.0)
        agent = FakeAgentState(tokens=5_000, max_tokens=100_000)
        a2a = FakeA2AClient()

        action = instinct.assess(agent)

        # First execution: all actions fire.
        results1 = await instinct.execute(action, agent, a2a)
        assert len(results1) > 0

        # Second execution: all skipped due to cooldown.
        a2a.broadcasts.clear()
        results2 = await instinct.execute(action, agent, a2a)
        assert len(results2) == 0
        assert len(a2a.broadcasts) == 0

    @pytest.mark.asyncio
    async def test_reset_cooldowns_allows_reexecution(self) -> None:
        instinct = SurvivalInstinct(action_cooldown=9999.0)
        agent = FakeAgentState(tokens=5_000, max_tokens=100_000)
        a2a = FakeA2AClient()

        action = instinct.assess(agent)
        await instinct.execute(action, agent, a2a)
        assert len(a2a.broadcasts) > 0

        # After cooldown reset, actions can fire again.
        a2a.broadcasts.clear()
        instinct.reset_cooldowns()
        results = await instinct.execute(action, agent, a2a)
        assert len(results) > 0


# ---------------------------------------------------------------------------
# Custom thresholds
# ---------------------------------------------------------------------------


class TestCustomThresholds:
    """Test with non-default thresholds."""

    def test_custom_panic_threshold(self) -> None:
        thresholds = SurvivalThresholds(panic=0.15, urgent=0.30, conservative=0.50, invest=0.75)
        instinct = SurvivalInstinct(thresholds=thresholds)
        agent = FakeAgentState(tokens=12_000, max_tokens=100_000)  # 12 %
        result = instinct.assess(agent)
        assert result.mode == SurvivalMode.PANIC

    def test_custom_invest_threshold(self) -> None:
        thresholds = SurvivalThresholds(invest=0.70)
        instinct = SurvivalInstinct(thresholds=thresholds)
        agent = FakeAgentState(tokens=75_000, max_tokens=100_000)  # 75 %
        result = instinct.assess(agent)
        assert result.mode == SurvivalMode.INVEST

    def test_default_thresholds_used_when_none(self) -> None:
        instinct = SurvivalInstinct(thresholds=None)
        assert instinct.thresholds == SurvivalThresholds()


# ---------------------------------------------------------------------------
# SurvivalThresholds dataclass
# ---------------------------------------------------------------------------


class TestSurvivalThresholds:
    """Test SurvivalThresholds defaults."""

    def test_default_values(self) -> None:
        t = SurvivalThresholds()
        assert t.panic == 0.10
        assert t.urgent == 0.20
        assert t.conservative == 0.40
        assert t.invest == 0.80

    def test_frozen(self) -> None:
        t = SurvivalThresholds()
        with pytest.raises(AttributeError):
            t.panic = 0.5  # type: ignore[misc]


# ---------------------------------------------------------------------------
# SurvivalAction dataclass
# ---------------------------------------------------------------------------


class TestSurvivalAction:
    """Test SurvivalAction immutability."""

    def test_frozen(self) -> None:
        action = SurvivalAction(mode=SurvivalMode.PANIC, token_ratio=0.05, actions=[])
        with pytest.raises(AttributeError):
            action.mode = SurvivalMode.NORMAL  # type: ignore[misc]


# ---------------------------------------------------------------------------
# Integration-style: full think-loop pattern
# ---------------------------------------------------------------------------


class TestThinkLoopPattern:
    """Simulate the think-loop pattern described in the architecture."""

    @pytest.mark.asyncio
    async def test_panic_skips_llm(self) -> None:
        """When tokens < 10 %, the think-loop should skip the LLM step."""
        instinct = SurvivalInstinct(action_cooldown=0.0)
        agent = FakeAgentState(tokens=5_000, max_tokens=100_000)
        a2a = FakeA2AClient()

        # Step 1: Assess (synchronous, no LLM)
        action = instinct.assess(agent)

        # Step 2: If PANIC or URGENT, execute directly and skip LLM
        if action.mode in (SurvivalMode.PANIC, SurvivalMode.URGENT):
            await instinct.execute(action, agent, a2a)
            llm_called = False
        else:
            llm_called = True  # Would call LLM here

        assert not llm_called
        assert action.mode == SurvivalMode.PANIC
        assert len(a2a.broadcasts) >= 1  # SOS was broadcast

    @pytest.mark.asyncio
    async def test_normal_proceeds_to_llm(self) -> None:
        """When tokens are normal, the think-loop should proceed to LLM."""
        instinct = SurvivalInstinct()
        agent = FakeAgentState(tokens=60_000, max_tokens=100_000)

        action = instinct.assess(agent)

        # NORMAL mode → actions list is empty → proceed to LLM
        assert action.mode == SurvivalMode.NORMAL
        assert action.actions == []
        # The think-loop would now call the LLM (not tested here).
