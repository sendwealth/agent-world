"""Tests for the rule proposal engine and rule evolution tracker."""

from __future__ import annotations

from agent_runtime.core.act import _DEFAULT_TOKEN_COSTS, ActionType
from agent_runtime.core.decide import _TOKEN_COSTS, DecisionAction
from agent_runtime.organization.rule_evolution import (
    RuleEvolutionTracker,
    RuleLifecycleEventType,
)
from agent_runtime.organization.rule_proposal import (
    RuleCategory,
    RuleCondition,
    RuleEffect,
    RuleProposal,
    RuleProposalEngine,
    _gini_coefficient,
)

# ===================================================================
# DecisionAction / ActionType registration tests
# ===================================================================


class TestDecisionActionRegistration:
    """Verify PROPOSE_RULE and VOTE_RULE are registered correctly."""

    def test_propose_rule_action_exists(self) -> None:
        assert DecisionAction.PROPOSE_RULE == "propose_rule"

    def test_vote_rule_action_exists(self) -> None:
        assert DecisionAction.VOTE_RULE == "vote_rule"

    def test_propose_rule_token_cost(self) -> None:
        assert _TOKEN_COSTS[DecisionAction.PROPOSE_RULE] == 15

    def test_vote_rule_token_cost(self) -> None:
        assert _TOKEN_COSTS[DecisionAction.VOTE_RULE] == 5

    def test_action_count_is_14(self) -> None:
        """There should now be 19 actions (12 original + 6 new + teach_skill)."""
        assert len(DecisionAction) == 19


class TestActionTypeRegistration:
    """Verify PROPOSE_RULE and VOTE_RULE in ActionType (act.py)."""

    def test_propose_rule_action_type(self) -> None:
        assert ActionType.PROPOSE_RULE == "propose_rule"

    def test_vote_rule_action_type(self) -> None:
        assert ActionType.VOTE_RULE == "vote_rule"

    def test_propose_rule_execution_cost(self) -> None:
        assert _DEFAULT_TOKEN_COSTS[ActionType.PROPOSE_RULE] == 15

    def test_vote_rule_execution_cost(self) -> None:
        assert _DEFAULT_TOKEN_COSTS[ActionType.VOTE_RULE] == 5


# ===================================================================
# RuleProposalEngine tests
# ===================================================================


class TestShouldProposeRule:
    """Test the trigger condition evaluation."""

    def test_no_trigger_when_stable(self) -> None:
        engine = RuleProposalEngine()
        context = {
            "members": [
                {"id": "a1", "resources": 100},
                {"id": "a2", "resources": 110},
            ],
            "recent_attacks": 0,
            "economic_output": 100,
            "expected_output": 100,
            "rival_org_expansion": False,
        }
        should, reason = engine.should_propose_rule("a1", "org-1", context)
        assert should is False
        assert "no triggering" in reason.lower()

    def test_trigger_on_inequality(self) -> None:
        engine = RuleProposalEngine()
        context = {
            "members": [
                {"id": "a1", "resources": 10},
                {"id": "a2", "resources": 1000},
                {"id": "a3", "resources": 5000},
            ],
            "recent_attacks": 0,
            "economic_output": 100,
            "expected_output": 100,
        }
        should, reason = engine.should_propose_rule("a1", "org-1", context)
        assert should is True
        assert "inequality" in reason.lower()

    def test_trigger_on_safety_incidents(self) -> None:
        engine = RuleProposalEngine(safety_threshold=0.2)
        context = {
            "members": [
                {"id": "a1", "resources": 100},
                {"id": "a2", "resources": 100},
            ],
            "recent_attacks": 5,
        }
        should, reason = engine.should_propose_rule("a1", "org-1", context)
        assert should is True
        assert "safety" in reason.lower()

    def test_trigger_on_economic_inefficiency(self) -> None:
        engine = RuleProposalEngine()
        context = {
            "members": [
                {"id": "a1", "resources": 100},
            ],
            "economic_output": 30,
            "expected_output": 100,
        }
        should, reason = engine.should_propose_rule("a1", "org-1", context)
        assert should is True
        assert "efficiency" in reason.lower()

    def test_trigger_on_external_threat(self) -> None:
        engine = RuleProposalEngine()
        context = {
            "members": [{"id": "a1", "resources": 100}],
            "rival_org_expansion": True,
        }
        should, reason = engine.should_propose_rule("a1", "org-1", context)
        assert should is True
        assert "threat" in reason.lower()


class TestGenerateRuleProposal:
    """Test rule proposal generation."""

    def test_generate_tax_rule(self) -> None:
        engine = RuleProposalEngine()
        proposal = engine.generate_rule_proposal(
            "agent-1",
            "org-1",
            "resource inequality detected among members",
            {"avg_member_resources": 100},
        )
        assert proposal.proposer_id == "agent-1"
        assert proposal.org_id == "org-1"
        assert proposal.rule_type == RuleCategory.TAX
        assert len(proposal.conditions) > 0
        assert len(proposal.effects) > 0
        assert "Tax" in proposal.title

    def test_generate_behavior_rule(self) -> None:
        engine = RuleProposalEngine()
        proposal = engine.generate_rule_proposal(
            "agent-1",
            "org-1",
            "safety concern: too many attacks",
            {},
        )
        assert proposal.rule_type == RuleCategory.BEHAVIOR

    def test_generate_trade_rule(self) -> None:
        engine = RuleProposalEngine()
        proposal = engine.generate_rule_proposal(
            "agent-1",
            "org-1",
            "trade efficiency is low",
            {"min_trade_volume": 10},
        )
        assert proposal.rule_type == RuleCategory.TRADE

    def test_generate_diplomacy_rule(self) -> None:
        engine = RuleProposalEngine()
        proposal = engine.generate_rule_proposal(
            "agent-1",
            "org-1",
            "rival organization threatening borders",
            {},
        )
        assert proposal.rule_type == RuleCategory.DIPLOMACY

    def test_generate_custom_rule(self) -> None:
        engine = RuleProposalEngine()
        proposal = engine.generate_rule_proposal(
            "agent-1",
            "org-1",
            "something unusual is happening",
            {},
        )
        assert proposal.rule_type == RuleCategory.CUSTOM


class TestCampaignForRule:
    """Test campaign message generation."""

    def test_campaign_message_content(self) -> None:
        engine = RuleProposalEngine()
        proposal = RuleProposal(
            proposal_id="r1",
            proposer_id="a1",
            org_id="org-1",
            title="Tax Rule",
            description="Charge extra tax on wealthy agents",
            rule_type=RuleCategory.TAX,
            conditions=(
                RuleCondition(field="agent.resources", operator=">", value=200),
            ),
            effects=(
                RuleEffect(target="agent.tax_bonus", action="set", value=0.1),
            ),
        )
        msg = engine.campaign_for_rule("a1", proposal)
        assert "Tax Rule" in msg
        assert "agent.resources" in msg
        assert "vote" in msg.lower()


class TestGiniCoefficient:
    """Test the Gini coefficient helper."""

    def test_perfect_equality(self) -> None:
        assert _gini_coefficient([100, 100, 100]) == 0.0

    def test_perfect_inequality(self) -> None:
        gini = _gini_coefficient([0, 0, 100])
        assert gini > 0.5

    def test_empty(self) -> None:
        assert _gini_coefficient([]) == 0.0

    def test_single_value(self) -> None:
        assert _gini_coefficient([42]) == 0.0


# ===================================================================
# RuleEvolutionTracker tests
# ===================================================================


class TestRuleLifecycleTracking:
    """Test rule lifecycle event recording and querying."""

    def test_record_creation_and_activation(self) -> None:
        tracker = RuleEvolutionTracker()
        tracker.record_rule_creation("r1", "org-1", "Test Rule", 10)
        tracker.record_rule_activation("r1", "org-1", 15)

        stats = tracker.get_rule_stats("r1")
        assert stats is not None
        assert stats.status == "active"
        assert stats.created_tick == 10

    def test_track_lifecycle_timeline(self) -> None:
        tracker = RuleEvolutionTracker()
        tracker.record_rule_creation("r1", "org-1", "Rule 1", 10)
        tracker.record_rule_activation("r1", "org-1", 20)
        tracker.record_rule_trigger("r1", "org-1", 30)

        timeline = tracker.track_rule_lifecycle("org-1")
        assert len(timeline) == 3
        assert timeline[0].event_type == RuleLifecycleEventType.PROPOSED
        assert timeline[1].event_type == RuleLifecycleEventType.ACTIVATED
        assert timeline[2].event_type == RuleLifecycleEventType.TRIGGERED

    def test_trigger_count(self) -> None:
        tracker = RuleEvolutionTracker()
        tracker.record_rule_creation("r1", "org-1", "Rule", 10)
        tracker.record_rule_trigger("r1", "org-1", 20)
        tracker.record_rule_trigger("r1", "org-1", 30)
        tracker.record_rule_trigger("r1", "org-1", 40)

        stats = tracker.get_rule_stats("r1")
        assert stats is not None
        assert stats.trigger_count == 3
        assert stats.last_triggered_tick == 40


class TestDetectStaleRules:
    """Test stale rule detection."""

    def test_detect_never_triggered_rule(self) -> None:
        tracker = RuleEvolutionTracker(stale_threshold_ticks=100)
        tracker.record_rule_creation("r1", "org-1", "Old Rule", 0)
        tracker.record_rule_activation("r1", "org-1", 10)

        stale = tracker.detect_stale_rules("org-1", current_tick=200)
        assert "r1" in stale

    def test_active_rule_not_stale(self) -> None:
        tracker = RuleEvolutionTracker(stale_threshold_ticks=100)
        tracker.record_rule_creation("r1", "org-1", "Recent Rule", 150)
        tracker.record_rule_activation("r1", "org-1", 160)

        stale = tracker.detect_stale_rules("org-1", current_tick=200)
        assert "r1" not in stale

    def test_recently_triggered_not_stale(self) -> None:
        tracker = RuleEvolutionTracker(stale_threshold_ticks=100)
        tracker.record_rule_creation("r1", "org-1", "Rule", 0)
        tracker.record_rule_activation("r1", "org-1", 10)
        tracker.record_rule_trigger("r1", "org-1", 180)

        stale = tracker.detect_stale_rules("org-1", current_tick=200)
        assert "r1" not in stale


class TestSuggestRuleReform:
    """Test reform suggestion logic."""

    def test_suggest_rules_when_none_exist(self) -> None:
        tracker = RuleEvolutionTracker()
        suggestions = tracker.suggest_rule_reform("org-1", {})
        assert len(suggestions) > 0
        assert any("no active" in s.lower() for s in suggestions)

    def test_suggest_repeal_stale_rules(self) -> None:
        tracker = RuleEvolutionTracker()
        tracker.record_rule_creation("r1", "org-1", "Stale Rule", 0)
        tracker.record_rule_activation("r1", "org-1", 10)
        # Never triggered

        suggestions = tracker.suggest_rule_reform("org-1", {})
        assert any("never triggered" in s.lower() for s in suggestions)

    def test_suggest_trade_rules_for_low_economy(self) -> None:
        tracker = RuleEvolutionTracker()
        tracker.record_rule_creation("r1", "org-1", "Behavior Rule", 0)
        tracker.record_rule_activation("r1", "org-1", 10)
        tracker.record_rule_trigger("r1", "org-1", 50)

        suggestions = tracker.suggest_rule_reform(
            "org-1",
            {"economic_output": 30, "expected_output": 100},
        )
        assert any("trade" in s.lower() for s in suggestions)
