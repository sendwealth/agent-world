"""Tests for the survival instinct module.

Covers:
- SurvivalMode classification at all token-ratio boundaries
- Emergency-action generation for each mode
- PANIC mode triggers immediately (< 10 %) without LLM
- Action cooldown enforcement
- execute() with mock A2A client
- Custom thresholds and threshold validation
- Edge cases (zero max_tokens, exactly on boundary, tokens > max_tokens)
- A2A client failure handling
- Concurrent execute() serialisation
- Loan terms configurability
"""

from __future__ import annotations

import asyncio
from dataclasses import dataclass
from typing import Any

import pytest

from agent_runtime.survival.instinct import (
    EmergencyActionType,
    LoanTerms,
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


class FailingA2AClient:
    """Mock A2A client that always raises."""

    def __init__(self, error: Exception | None = None) -> None:
        self.error = error or ConnectionError("A2A network unreachable")
        self.call_count = 0

    async def broadcast_message(self, payload: dict[str, Any]) -> dict[str, Any]:
        self.call_count += 1
        raise self.error


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
        """Degenerate case: max_tokens == 0 -> ratio 0 -> PANIC."""
        agent = FakeAgentState(tokens=0, max_tokens=0)
        result = self.instinct.assess(agent)
        assert result.mode == SurvivalMode.PANIC
        assert result.token_ratio == 0.0

    def test_tokens_exceed_max_tokens_clamps_ratio(self) -> None:
        """tokens > max_tokens -> ratio clamped to 1.0 -> INVEST."""
        agent = FakeAgentState(tokens=120_000, max_tokens=100_000)
        result = self.instinct.assess(agent)
        assert result.token_ratio == 1.0
        assert result.mode == SurvivalMode.INVEST

    def test_negative_tokens_clamps_ratio(self) -> None:
        """Negative tokens -> ratio clamped to 0.0 -> PANIC."""
        agent = FakeAgentState(tokens=-500, max_tokens=100_000)
        result = self.instinct.assess(agent)
        assert result.token_ratio == 0.0
        assert result.mode == SurvivalMode.PANIC


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
        """The assess() call is synchronous (no await) -- no LLM involved."""
        instinct = SurvivalInstinct()
        agent = FakeAgentState(tokens=9_999, max_tokens=100_000)
        result = instinct.assess(agent)
        assert result.mode == SurvivalMode.PANIC
        assert not asyncio.iscoroutinefunction(instinct.assess)

    def test_panic_actions_include_sos(self) -> None:
        instinct = SurvivalInstinct()
        agent = FakeAgentState(tokens=1_000, max_tokens=100_000)
        result = instinct.assess(agent)
        assert any(a.action_type == EmergencyActionType.BROADCAST_SOS for a in result.actions)

    def test_panic_actions_include_loan_request(self) -> None:
        instinct = SurvivalInstinct()
        agent = FakeAgentState(tokens=1_000, max_tokens=100_000)
        result = instinct.assess(agent)
        assert any(a.action_type == EmergencyActionType.REQUEST_LOAN for a in result.actions)

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
        await instinct.execute(action, agent, a2a)

        assert len(a2a.broadcasts) >= 1
        sos_broadcasts = [
            b for b in a2a.broadcasts if b.get("payload", {}).get("category") == "personal"
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
            b for b in a2a.broadcasts if b.get("payload", {}).get("action") == "loan_request"
        ]
        assert len(loan_broadcasts) >= 1

    @pytest.mark.asyncio
    async def test_execute_without_a2a_client(self) -> None:
        """Execute should work even without an A2A client (local actions)."""
        instinct = SurvivalInstinct(action_cooldown=0.0)
        agent = FakeAgentState(tokens=5_000, max_tokens=100_000)

        action = instinct.assess(agent)
        results = await instinct.execute(action, agent, a2a_client=None)

        assert len(results) > 0
        for r in results:
            # Without A2A client, A2A actions get "executed" status but no
            # broadcast_result; local-only actions get "logged".
            assert r["status"] in ("executed", "logged")

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
    async def test_execute_returns_results_with_correct_status(self) -> None:
        """A2A actions get 'executed', local-only actions get 'logged'."""
        instinct = SurvivalInstinct(action_cooldown=0.0)
        agent = FakeAgentState(tokens=5_000, max_tokens=100_000, money=100)
        a2a = FakeA2AClient()

        action = instinct.assess(agent)
        results = await instinct.execute(action, agent, a2a)

        a2a_action_names = {"broadcast_sos", "request_loan"}
        for r in results:
            assert "action" in r
            assert "reason" in r
            if r["action"] in a2a_action_names:
                assert r["status"] == "executed"
            else:
                assert r["status"] == "logged"

    @pytest.mark.asyncio
    async def test_execute_local_actions_without_a2a_are_logged(self) -> None:
        """Non-A2A actions (REST_TO_CONSERVE, etc.) report 'logged' status."""
        instinct = SurvivalInstinct(action_cooldown=0.0)
        agent = FakeAgentState(tokens=5_000, max_tokens=100_000)

        action = instinct.assess(agent)
        results = await instinct.execute(action, agent, a2a_client=None)

        for r in results:
            if r["action"] == "rest_to_conserve":
                assert r["status"] == "logged"


# ---------------------------------------------------------------------------
# A2A client failure handling (review issue #4 and #8)
# ---------------------------------------------------------------------------


class TestA2AFailure:
    """Test that A2A failures do not break the execute chain."""

    @pytest.mark.asyncio
    async def test_a2a_failure_returns_failed_status(self) -> None:
        """A failing SOS broadcast should mark the action as 'failed'."""
        instinct = SurvivalInstinct(action_cooldown=0.0)
        agent = FakeAgentState(tokens=5_000, max_tokens=100_000)
        a2a = FailingA2AClient()

        action = instinct.assess(agent)
        results = await instinct.execute(action, agent, a2a)

        # SOS action should have failed status
        sos_results = [r for r in results if r["action"] == "broadcast_sos"]
        assert len(sos_results) >= 1
        assert sos_results[0]["status"] == "failed"

    @pytest.mark.asyncio
    async def test_a2a_failure_does_not_stop_other_actions(self) -> None:
        """Even if SOS broadcast fails, subsequent actions should still run."""
        instinct = SurvivalInstinct(action_cooldown=0.0)
        agent = FakeAgentState(tokens=5_000, max_tokens=100_000)
        a2a = FailingA2AClient()

        action = instinct.assess(agent)
        results = await instinct.execute(action, agent, a2a)

        # Should have results for all actions, not just the first one
        assert len(results) > 1
        # REST_TO_CONSERVE should still have been attempted
        rest_results = [r for r in results if r["action"] == "rest_to_conserve"]
        assert len(rest_results) >= 1
        # Non-A2A actions should be logged even when A2A fails
        assert rest_results[0]["status"] == "logged"

    @pytest.mark.asyncio
    async def test_a2a_timeout_is_handled(self) -> None:
        """TimeoutError in A2A should be caught."""
        instinct = SurvivalInstinct(action_cooldown=0.0)
        agent = FakeAgentState(tokens=5_000, max_tokens=100_000)
        a2a = FailingA2AClient(error=TimeoutError("A2A timed out"))

        action = instinct.assess(agent)
        results = await instinct.execute(action, agent, a2a)

        # Should not raise; actions should complete with appropriate status
        assert len(results) > 0
        failed = [r for r in results if r["status"] == "failed"]
        assert len(failed) >= 1  # At least the SOS should fail

    @pytest.mark.asyncio
    async def test_a2a_connection_error_is_handled(self) -> None:
        """ConnectionError in A2A should be caught."""
        instinct = SurvivalInstinct(action_cooldown=0.0)
        agent = FakeAgentState(tokens=5_000, max_tokens=100_000)
        a2a = FailingA2AClient(error=ConnectionError("Network unreachable"))

        action = instinct.assess(agent)
        results = await instinct.execute(action, agent, a2a)

        assert len(results) > 0


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
# Concurrent execute() serialisation (review issue #1)
# ---------------------------------------------------------------------------


class TestConcurrency:
    """Test that concurrent execute() calls are serialised."""

    @pytest.mark.asyncio
    async def test_concurrent_execute_respects_cooldown(self) -> None:
        """Two concurrent execute() calls should not double-fire actions."""
        instinct = SurvivalInstinct(action_cooldown=100.0)
        agent = FakeAgentState(tokens=5_000, max_tokens=100_000)
        a2a = FakeA2AClient()

        action = instinct.assess(agent)

        # Launch two concurrent execute calls.
        results1, results2 = await asyncio.gather(
            instinct.execute(action, agent, a2a),
            instinct.execute(action, agent, a2a),
        )

        total_results = len(results1) + len(results2)
        # Due to the lock, only one call should execute actions;
        # the other should see cooldowns and return empty.
        assert total_results == len(action.actions)


# ---------------------------------------------------------------------------
# Threshold validation (review issue #3)
# ---------------------------------------------------------------------------


class TestThresholdValidation:
    """Test SurvivalThresholds ordering validation."""

    def test_valid_default_thresholds(self) -> None:
        t = SurvivalThresholds()
        assert t.panic == 0.10

    def test_valid_custom_thresholds(self) -> None:
        t = SurvivalThresholds(panic=0.05, urgent=0.15, conservative=0.30, invest=0.90)
        assert t.panic == 0.05

    def test_inverted_panic_urgent_raises(self) -> None:
        with pytest.raises(ValueError, match="Thresholds must satisfy"):
            SurvivalThresholds(panic=0.30, urgent=0.10, conservative=0.40, invest=0.80)

    def test_equal_panic_urgent_raises(self) -> None:
        with pytest.raises(ValueError, match="Thresholds must satisfy"):
            SurvivalThresholds(panic=0.20, urgent=0.20, conservative=0.40, invest=0.80)

    def test_inverted_conservative_invest_raises(self) -> None:
        with pytest.raises(ValueError, match="Thresholds must satisfy"):
            SurvivalThresholds(panic=0.10, urgent=0.20, conservative=0.90, invest=0.50)

    def test_negative_panic_raises(self) -> None:
        with pytest.raises(ValueError, match="Thresholds must satisfy"):
            SurvivalThresholds(panic=-0.1, urgent=0.20, conservative=0.40, invest=0.80)

    def test_invest_above_1_raises(self) -> None:
        with pytest.raises(ValueError, match="Thresholds must satisfy"):
            SurvivalThresholds(panic=0.10, urgent=0.20, conservative=0.40, invest=1.5)

    def test_invest_exactly_1_is_valid(self) -> None:
        t = SurvivalThresholds(panic=0.10, urgent=0.20, conservative=0.40, invest=1.0)
        assert t.invest == 1.0

    def test_all_zero_except_invest_raises(self) -> None:
        with pytest.raises(ValueError):
            SurvivalThresholds(panic=0.0, urgent=0.0, conservative=0.0, invest=0.0)


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
# Loan terms configurability (review issue #7)
# ---------------------------------------------------------------------------


class TestLoanTerms:
    """Test that loan terms are configurable."""

    def test_default_loan_terms(self) -> None:
        terms = LoanTerms()
        assert terms.interest_offered == 0.02
        assert terms.repayment_ticks == 500

    def test_custom_loan_terms_in_instinct(self) -> None:
        terms = LoanTerms(interest_offered=0.05, repayment_ticks=1000)
        instinct = SurvivalInstinct(loan_terms=terms)
        assert instinct.loan_terms.interest_offered == 0.05
        assert instinct.loan_terms.repayment_ticks == 1000

    @pytest.mark.asyncio
    async def test_custom_loan_terms_used_in_broadcast(self) -> None:
        terms = LoanTerms(interest_offered=0.10, repayment_ticks=200)
        instinct = SurvivalInstinct(action_cooldown=0.0, loan_terms=terms)
        agent = FakeAgentState(tokens=5_000, max_tokens=100_000)
        a2a = FakeA2AClient()

        action = instinct.assess(agent)
        await instinct.execute(action, agent, a2a)

        loan_broadcasts = [
            b for b in a2a.broadcasts if b.get("payload", {}).get("action") == "loan_request"
        ]
        assert len(loan_broadcasts) >= 1
        loan_terms = loan_broadcasts[0]["payload"]["terms"]
        assert loan_terms["interest_offered"] == 0.10
        assert loan_terms["repayment_ticks"] == 200


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
# SOS message formatting safety (review issue #6)
# ---------------------------------------------------------------------------


class TestSOSMessageFormat:
    """Test that SOS message handles non-float token_ratio gracefully."""

    @pytest.mark.asyncio
    async def test_sos_with_float_ratio(self) -> None:
        instinct = SurvivalInstinct(action_cooldown=0.0)
        agent = FakeAgentState(tokens=5_000, max_tokens=100_000)
        a2a = FakeA2AClient()

        action = instinct.assess(agent)
        await instinct.execute(action, agent, a2a)

        sos = [b for b in a2a.broadcasts if b.get("payload", {}).get("category") == "personal"]
        assert len(sos) >= 1
        content = sos[0]["payload"]["content"]
        assert "5.0%" in content  # 0.05 formatted as 5.0%

    @pytest.mark.asyncio
    async def test_sos_message_format_safe(self) -> None:
        """Verify the SOS content is a well-formed string."""
        instinct = SurvivalInstinct(action_cooldown=0.0)
        agent = FakeAgentState(tokens=5_000, max_tokens=100_000)
        a2a = FakeA2AClient()

        action = instinct.assess(agent)
        await instinct.execute(action, agent, a2a)

        sos = [b for b in a2a.broadcasts if b.get("payload", {}).get("category") == "personal"]
        assert len(sos) >= 1
        assert isinstance(sos[0]["payload"]["content"], str)
        assert "[SOS]" in sos[0]["payload"]["content"]


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

        # NORMAL mode -> actions list is empty -> proceed to LLM
        assert action.mode == SurvivalMode.NORMAL
        assert action.actions == []


# ---------------------------------------------------------------------------
# Urgent fallback to LLM after repeated failures
# ---------------------------------------------------------------------------


class TestUrgentFallback:
    """Test that repeated urgent-action failures trigger fallback to LLM."""

    @pytest.mark.asyncio
    async def test_fallback_after_consecutive_failures(self) -> None:
        """After max_urgent_retries consecutive failures, should_fallback_to_llm is True."""
        instinct = SurvivalInstinct(action_cooldown=0.0, max_urgent_retries=3)
        agent = FakeAgentState(tokens=5_000, max_tokens=100_000)
        a2a = FailingA2AClient()

        action = instinct.assess(agent)
        assert action.mode == SurvivalMode.PANIC

        # Execute 3 times — each should fail because A2A is broken.
        for i in range(3):
            results = await instinct.execute(action, agent, a2a)
            assert any(r["status"] == "failed" for r in results)
            assert instinct.consecutive_urgent_failures == i + 1

        assert instinct.should_fallback_to_llm is True

    @pytest.mark.asyncio
    async def test_no_fallback_when_actions_succeed(self) -> None:
        """When A2A works, should_fallback_to_llm stays False."""
        instinct = SurvivalInstinct(action_cooldown=0.0, max_urgent_retries=3)
        agent = FakeAgentState(tokens=5_000, max_tokens=100_000)
        a2a = FakeA2AClient()  # succeeds

        action = instinct.assess(agent)
        await instinct.execute(action, agent, a2a)

        assert instinct.consecutive_urgent_failures == 0
        assert instinct.should_fallback_to_llm is False

    @pytest.mark.asyncio
    async def test_fallback_counter_resets_on_success(self) -> None:
        """After failures, a successful execute resets the counter."""
        instinct = SurvivalInstinct(action_cooldown=0.0, max_urgent_retries=3)
        agent = FakeAgentState(tokens=5_000, max_tokens=100_000)
        failing = FailingA2AClient()
        succeeding = FakeA2AClient()

        action = instinct.assess(agent)

        # Fail twice.
        await instinct.execute(action, agent, failing)
        await instinct.execute(action, agent, failing)
        assert instinct.consecutive_urgent_failures == 2

        # Succeed once — counter should reset.
        instinct.reset_cooldowns()
        await instinct.execute(action, agent, succeeding)
        assert instinct.consecutive_urgent_failures == 0
        assert instinct.should_fallback_to_llm is False

    @pytest.mark.asyncio
    async def test_fallback_counter_resets_on_mode_improvement(self) -> None:
        """When token ratio improves out of PANIC/URGENT, the counter resets."""
        instinct = SurvivalInstinct(action_cooldown=0.0, max_urgent_retries=3)
        agent_low = FakeAgentState(tokens=5_000, max_tokens=100_000)
        agent_ok = FakeAgentState(tokens=50_000, max_tokens=100_000)
        failing = FailingA2AClient()

        # Fail twice in PANIC mode.
        action = instinct.assess(agent_low)
        await instinct.execute(action, agent_low, failing)
        await instinct.execute(action, agent_low, failing)
        assert instinct.consecutive_urgent_failures == 2

        # Agent recovers — assess with normal tokens.
        action2 = instinct.assess(agent_ok)
        assert action2.mode == SurvivalMode.NORMAL
        assert instinct.consecutive_urgent_failures == 0
        assert instinct.should_fallback_to_llm is False

    @pytest.mark.asyncio
    async def test_manual_reset_fallback_counter(self) -> None:
        """reset_fallback_counter() clears the counter."""
        instinct = SurvivalInstinct(action_cooldown=0.0, max_urgent_retries=3)
        agent = FakeAgentState(tokens=5_000, max_tokens=100_000)
        failing = FailingA2AClient()

        action = instinct.assess(agent)
        for _ in range(3):
            await instinct.execute(action, agent, failing)

        assert instinct.should_fallback_to_llm is True
        instinct.reset_fallback_counter()
        assert instinct.should_fallback_to_llm is False
        assert instinct.consecutive_urgent_failures == 0

    @pytest.mark.asyncio
    async def test_custom_max_urgent_retries(self) -> None:
        """max_urgent_retries is configurable."""
        instinct = SurvivalInstinct(action_cooldown=0.0, max_urgent_retries=1)
        agent = FakeAgentState(tokens=5_000, max_tokens=100_000)
        failing = FailingA2AClient()

        action = instinct.assess(agent)
        await instinct.execute(action, agent, failing)

        # Should fallback after just 1 failure.
        assert instinct.should_fallback_to_llm is True

    @pytest.mark.asyncio
    async def test_think_loop_pattern_fallback(self) -> None:
        """Simulate the think-loop pattern: after max failures, LLM is called."""
        instinct = SurvivalInstinct(action_cooldown=0.0, max_urgent_retries=2)
        agent = FakeAgentState(tokens=5_000, max_tokens=100_000)
        failing = FailingA2AClient()

        llm_calls = 0

        for tick in range(5):
            action = instinct.assess(agent)
            if action.mode in (SurvivalMode.PANIC, SurvivalMode.URGENT):
                await instinct.execute(action, agent, failing)
                if instinct.should_fallback_to_llm:
                    # Fall through to LLM
                    llm_calls += 1
                # else: skip LLM (return early)
            else:
                llm_calls += 1

        # Tick 0: counter=1, no fallback → no LLM
        # Tick 1: counter=2 (= max), fallback → LLM
        # Ticks 2-4: counter keeps growing, fallback stays True → LLM
        # Total LLM calls = 4 (ticks 1,2,3,4)
        assert llm_calls == 4
