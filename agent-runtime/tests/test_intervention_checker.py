"""Tests for the InterventionChecker — pre-dispatch safety gate."""

import time
from unittest.mock import MagicMock

import pytest

from agent_runtime.core.intervention_checker import (
    CheckVerdict,
    InterventionChecker,
    InterventionConfig,
)


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


class MockAgentState:
    """Minimal mock that satisfies the checker's attribute access."""

    def __init__(
        self,
        agent_id: str = "test-agent",
        tokens: int = 100,
        tick: int = 50,
        phase: str = "Adult",
    ):
        self.id = agent_id
        self.tokens = tokens
        self.tick = tick
        self.phase = MagicMock(value=phase)


# ---------------------------------------------------------------------------
# IC-05: Death lock
# ---------------------------------------------------------------------------


class TestDeathLock:
    def test_dead_agent_blocked(self):
        checker = InterventionChecker()
        agent = MockAgentState(phase="Dead")
        result = checker.check("send_message", agent)
        assert result.blocked
        assert result.rule == "IC-05"

    def test_dying_agent_blocked(self):
        checker = InterventionChecker()
        agent = MockAgentState(phase="Dying")
        result = checker.check("send_message", agent)
        assert result.blocked
        assert result.rule == "IC-05"

    def test_alive_agent_passes(self):
        checker = InterventionChecker()
        agent = MockAgentState(phase="Adult")
        result = checker.check("send_message", agent)
        assert not result.blocked


# ---------------------------------------------------------------------------
# IC-01: Broadcast rate-limit
# ---------------------------------------------------------------------------


class TestBroadcastRateLimit:
    def test_allows_under_limit(self):
        config = InterventionConfig(broadcast_max_per_window=3, broadcast_window_seconds=1.0)
        checker = InterventionChecker(config)
        agent = MockAgentState()

        for _ in range(3):
            result = checker.check("send_message", agent, {"payload": {}})
            assert not result.blocked

    def test_blocks_over_limit(self):
        config = InterventionConfig(broadcast_max_per_window=2, broadcast_window_seconds=10.0)
        checker = InterventionChecker(config)
        agent = MockAgentState()

        # First two pass
        checker.check("send_message", agent, {"payload": {}})
        checker.check("send_message", agent, {"payload": {}})
        # Third is blocked
        result = checker.check("send_message", agent, {"payload": {}})
        assert result.blocked
        assert result.rule == "IC-01"

    def test_targeted_message_not_rate_limited(self):
        config = InterventionConfig(broadcast_max_per_window=1, broadcast_window_seconds=10.0)
        checker = InterventionChecker(config)
        agent = MockAgentState()

        # Targeted message (has to_agent)
        result = checker.check(
            "send_message", agent, {"payload": {"to_agent": "other"}, "to_agent": "other"}
        )
        assert not result.blocked
        # Second targeted message should also pass
        result = checker.check(
            "send_message", agent, {"payload": {"to_agent": "other"}, "to_agent": "other"}
        )
        assert not result.blocked


# ---------------------------------------------------------------------------
# IC-02: Message size limit
# ---------------------------------------------------------------------------


class TestMessageSizeLimit:
    def test_allows_normal_payload(self):
        checker = InterventionChecker()
        agent = MockAgentState()
        result = checker.check("send_message", agent, {"payload": {"text": "hello"}})
        assert not result.blocked

    def test_blocks_oversized_payload(self):
        config = InterventionConfig(max_payload_bytes=100)
        checker = InterventionChecker(config)
        agent = MockAgentState()
        big_payload = {"data": "x" * 200}
        result = checker.check("send_message", agent, {"payload": big_payload})
        assert result.blocked
        assert result.rule == "IC-02"

    def test_non_message_actions_skip_size_check(self):
        config = InterventionConfig(max_payload_bytes=10)
        checker = InterventionChecker(config)
        agent = MockAgentState()
        result = checker.check("rest", agent, {"payload": {"data": "x" * 200}})
        assert not result.blocked


# ---------------------------------------------------------------------------
# IC-03: Newbie protection
# ---------------------------------------------------------------------------


class TestNewbieProtection:
    def test_newbie_blocked_from_aggressive_actions(self):
        config = InterventionConfig(newbie_protection_ticks=10)
        checker = InterventionChecker(config)
        agent = MockAgentState(tick=3)
        result = checker.check("attack", agent, {"target_agent_id": "victim"})
        assert result.blocked
        assert result.rule == "IC-03"

    def test_newbie_can_do_non_aggressive_actions(self):
        config = InterventionConfig(newbie_protection_ticks=10)
        checker = InterventionChecker(config)
        agent = MockAgentState(tick=3)
        result = checker.check("explore", agent)
        assert not result.blocked

    def test_mature_agent_can_attack(self):
        config = InterventionConfig(newbie_protection_ticks=10)
        checker = InterventionChecker(config)
        agent = MockAgentState(tick=15)
        result = checker.check("attack", agent, {"target_agent_id": "victim"})
        assert not result.blocked


# ---------------------------------------------------------------------------
# IC-04: Token sufficiency
# ---------------------------------------------------------------------------


class TestTokenSufficiency:
    def test_zero_tokens_blocks_actions(self):
        config = InterventionConfig(token_low_water_mark=0)
        checker = InterventionChecker(config)
        agent = MockAgentState(tokens=0)
        result = checker.check("explore", agent)
        assert result.blocked
        assert result.rule == "IC-04"

    def test_zero_tokens_allows_rest(self):
        config = InterventionConfig(token_low_water_mark=0)
        checker = InterventionChecker(config)
        agent = MockAgentState(tokens=0)
        result = checker.check("rest", agent)
        assert not result.blocked

    def test_positive_tokens_allowed(self):
        checker = InterventionChecker()
        agent = MockAgentState(tokens=50)
        result = checker.check("explore", agent)
        assert not result.blocked


# ---------------------------------------------------------------------------
# Audit log
# ---------------------------------------------------------------------------


class TestAuditLog:
    def test_blocked_actions_are_logged(self):
        checker = InterventionChecker()
        agent = MockAgentState(phase="Dead")
        assert len(checker.audit_log) == 0
        checker.check("send_message", agent)
        assert len(checker.audit_log) == 1
        ts, agent_id, action, rule, reason = checker.audit_log[0]
        assert agent_id == "test-agent"
        assert action == "send_message"
        assert rule == "IC-05"

    def test_clear_audit_log(self):
        checker = InterventionChecker()
        agent = MockAgentState(phase="Dead")
        checker.check("send_message", agent)
        assert len(checker.audit_log) == 1
        checker.clear_audit_log()
        assert len(checker.audit_log) == 0


# ---------------------------------------------------------------------------
# Check order priority
# ---------------------------------------------------------------------------


class TestCheckOrder:
    """IC-05 (death lock) must be checked before all other rules."""

    def test_dead_agent_gets_ic05_not_ic04(self):
        """A dead agent with 0 tokens should get IC-05, not IC-04."""
        checker = InterventionChecker()
        agent = MockAgentState(phase="Dead", tokens=0)
        result = checker.check("send_message", agent)
        assert result.blocked
        assert result.rule == "IC-05"

    def test_dead_agent_skips_broadcast_check(self):
        """A dead agent should not even trigger broadcast rate-limit."""
        config = InterventionConfig(broadcast_max_per_window=0)
        checker = InterventionChecker(config)
        agent = MockAgentState(phase="Dead")
        result = checker.check("send_message", agent, {"payload": {}})
        assert result.rule == "IC-05"  # Not IC-01
