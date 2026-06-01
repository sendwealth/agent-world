"""Tests for Oracle Responder, Bounty Hunter, and new Action handlers.

Covers:
- OracleResponder: strategy selection, LLM fallback, response generation
- BountyHunter: skill matching, evaluation scoring, acceptance/decline decisions
- ActionExecutor: RESPOND_ORACLE, CHECK_BOUNTIES, ACCEPT_BOUNTY, COMPLETE_BOUNTY dispatch
"""

from __future__ import annotations

from unittest.mock import AsyncMock

import pytest

from agent_runtime.actions.bounty_hunter import (
    BountyDecision,
    BountyHunter,
)
from agent_runtime.actions.oracle_responder import (
    OracleResponder,
    OracleResponseStrategy,
    OracleType,
)
from agent_runtime.core.act import (
    ActionContext,
    ActionExecutor,
    ActionStatus,
    ActionType,
)

# ============================================================================
# OracleResponder Tests
# ============================================================================


class TestOracleResponder:
    """Tests for OracleResponder."""

    @pytest.mark.asyncio
    async def test_guidance_strategy_with_fallback(self):
        responder = OracleResponder(llm_provider=None)
        result = await responder.respond("o-1", "guidance", "Focus on gathering.", "Alice")
        assert result.oracle_id == "o-1"
        assert result.strategy == OracleResponseStrategy.ACKNOWLEDGE_GUIDANCE
        assert result.oracle_type == OracleType.GUIDANCE
        assert not result.used_llm

    @pytest.mark.asyncio
    async def test_warning_strategy(self):
        responder = OracleResponder()
        result = await responder.respond("o-2", "warning", "Danger ahead!", "Bob")
        assert result.strategy == OracleResponseStrategy.HEED_WARNING
        assert result.oracle_type == OracleType.WARNING

    @pytest.mark.asyncio
    async def test_blessing_strategy(self):
        responder = OracleResponder()
        result = await responder.respond("o-3", "blessing", "Blessed!", "Charlie")
        assert result.strategy == OracleResponseStrategy.EXPRESS_GRATITUDE

    @pytest.mark.asyncio
    async def test_curse_strategy(self):
        responder = OracleResponder()
        result = await responder.respond("o-4", "curse", "Cursed!", "Dave")
        assert result.strategy == OracleResponseStrategy.SHOW_RESILIENCE

    @pytest.mark.asyncio
    async def test_unknown_type_defaults_to_guidance(self):
        responder = OracleResponder()
        result = await responder.respond("o-5", "unknown_type", "Hello", "Eve")
        assert result.strategy == OracleResponseStrategy.ACKNOWLEDGE_GUIDANCE
        assert result.oracle_type == OracleType.GUIDANCE

    @pytest.mark.asyncio
    async def test_llm_generation(self):
        mock_llm = AsyncMock()
        mock_llm.generate = AsyncMock(return_value="I appreciate your wise words.")
        responder = OracleResponder(llm_provider=mock_llm)
        result = await responder.respond("o-6", "guidance", "Build houses.", "Frank")
        assert result.used_llm
        assert "appreciate" in result.response

    @pytest.mark.asyncio
    async def test_llm_failure_uses_fallback(self):
        mock_llm = AsyncMock()
        mock_llm.generate = AsyncMock(side_effect=RuntimeError("LLM unavailable"))
        responder = OracleResponder(llm_provider=mock_llm)
        result = await responder.respond("o-7", "warning", "Be careful!", "Grace")
        assert not result.used_llm
        assert result.strategy == OracleResponseStrategy.HEED_WARNING

    def test_get_strategy(self):
        responder = OracleResponder()
        assert responder.get_strategy("guidance") == OracleResponseStrategy.ACKNOWLEDGE_GUIDANCE
        assert responder.get_strategy("warning") == OracleResponseStrategy.HEED_WARNING
        assert responder.get_strategy("blessing") == OracleResponseStrategy.EXPRESS_GRATITUDE
        assert responder.get_strategy("curse") == OracleResponseStrategy.SHOW_RESILIENCE
        assert responder.get_strategy("unknown") == OracleResponseStrategy.ACKNOWLEDGE_GUIDANCE


# ============================================================================
# BountyHunter Tests
# ============================================================================


class TestBountyHunter:
    """Tests for BountyHunter."""

    def test_accept_good_match(self):
        hunter = BountyHunter()
        result = hunter.evaluate(
            bounty={
                "id": "b-1",
                "title": "Gather 50 wood",
                "reward": 100,
                "description": "Collect wood from forest.",
            },
            agent_skills={"gather": 3, "build": 1},
            agent_tokens=50,
            agent_reputation=5.0,
        )
        assert result.should_accept
        assert result.decision == BountyDecision.ACCEPT
        assert result.skill_match > 0
        assert result.resource_feasible
        assert len(result.execution_plan) > 0

    def test_decline_insufficient_tokens(self):
        hunter = BountyHunter()
        result = hunter.evaluate(
            bounty={
                "id": "b-2",
                "title": "Build tower",
                "reward": 200,
                "description": "Build stone tower.",
            },
            agent_skills={"build": 5},
            agent_tokens=3,
            agent_reputation=5.0,
            acceptance_cost=10,
        )
        assert not result.should_accept
        assert result.decision == BountyDecision.DECLINE_INSUFFICIENT_TOKENS
        assert not result.resource_feasible

    def test_decline_low_reputation_high_value(self):
        hunter = BountyHunter()
        result = hunter.evaluate(
            bounty={
                "id": "b-3",
                "title": "Defeat dragon",
                "reward": 1000,
                "description": "Slay the dragon.",
            },
            agent_skills={"fight": 10},
            agent_tokens=100,
            agent_reputation=3.0,
        )
        assert not result.should_accept
        assert result.decision == BountyDecision.DECLINE_LOW_REPUTATION

    def test_accept_high_reputation_high_value(self):
        hunter = BountyHunter()
        result = hunter.evaluate(
            bounty={
                "id": "b-4",
                "title": "Defeat dragon",
                "reward": 1000,
                "description": "Slay the dragon.",
            },
            agent_skills={"fight": 10},
            agent_tokens=100,
            agent_reputation=15.0,
        )
        assert result.should_accept
        assert result.reputation_sufficient

    def test_no_skills_zero_match(self):
        """Agent with no skills: skill_match=0 but may accept on resources/reputation."""
        hunter = BountyHunter()
        result = hunter.evaluate(
            bounty={
                "id": "b-5",
                "title": "Explore caves",
                "reward": 100,
                "description": "Map cave system.",
            },
            agent_skills={},
            agent_tokens=100,
            agent_reputation=5.0,
        )
        # skill_match is 0, but resource + reputation give 0.6 > threshold 0.4
        assert result.skill_match == 0.0
        # If we want to ensure decline, use low tokens or low reputation
        result2 = hunter.evaluate(
            bounty={
                "id": "b-5",
                "title": "Explore caves",
                "reward": 100,
                "description": "Map cave system.",
            },
            agent_skills={},
            agent_tokens=0,
            agent_reputation=0.0,
        )
        assert not result2.should_accept

    def test_create_completion_result(self):
        hunter = BountyHunter()
        result = hunter.create_completion_result("b-1", "Gather 50 wood", "Collected 52 wood units")
        assert result.bounty_id == "b-1"
        assert result.success
        assert "Gather 50 wood" in result.result_text

    def test_execution_plan_generated(self):
        hunter = BountyHunter()
        result = hunter.evaluate(
            bounty={
                "id": "b-7",
                "title": "Gather wood",
                "reward": 100,
                "description": "Collect wood.",
            },
            agent_skills={"gather": 3},
            agent_tokens=50,
            agent_reputation=5.0,
        )
        assert result.should_accept
        assert any("gather" in step.lower() for step in result.execution_plan)


# ============================================================================
# ActionExecutor Tests (Oracle & Bounty actions)
# ============================================================================


class _MockAgentState:
    def __init__(self, tokens: int = 100):
        self._tokens = tokens

    @property
    def tokens(self) -> int:
        return self._tokens

    def adjust_tokens(self, delta: int) -> None:
        self._tokens += delta


class _MockWorldClient:
    def __init__(self):
        self.respond_to_oracle = AsyncMock(return_value={
            "status": "ok", "oracle_id": "o-1", "received": True
        })
        self.check_bounties = AsyncMock(return_value={
            "status": "ok", "bounties": [{"id": "b-1", "title": "Gather wood", "reward": 100}]
        })
        self.claim_bounty = AsyncMock(return_value={
            "status": "ok", "bounty_id": "b-1", "received": True
        })
        self.complete_bounty = AsyncMock(return_value={
            "status": "ok", "bounty_id": "b-1", "received": True
        })


class TestActionExecutorOracleBounty:
    """Tests for new Oracle/Bounty action handlers."""

    @pytest.mark.asyncio
    async def test_respond_oracle_success(self):
        executor = ActionExecutor()
        agent = _MockAgentState(tokens=100)
        world = _MockWorldClient()
        ctx = ActionContext(
            agent=agent, world=world,
            parameters={"oracle_id": "o-1", "response": "Thank you!"},
        )
        result = await executor.execute(ActionType.RESPOND_ORACLE, ctx)
        assert result.status == ActionStatus.SUCCESS
        assert result.token_cost == 3
        world.respond_to_oracle.assert_called_once_with("o-1", "Thank you!")

    @pytest.mark.asyncio
    async def test_respond_oracle_missing_params(self):
        executor = ActionExecutor()
        agent = _MockAgentState(tokens=100)
        world = _MockWorldClient()
        ctx = ActionContext(agent=agent, world=world, parameters={"response": "hello"})
        result = await executor.execute(ActionType.RESPOND_ORACLE, ctx)
        assert result.status == ActionStatus.RETRY_EXHAUSTED

    @pytest.mark.asyncio
    async def test_check_bounties_success(self):
        executor = ActionExecutor()
        agent = _MockAgentState(tokens=100)
        world = _MockWorldClient()
        ctx = ActionContext(agent=agent, world=world, parameters={})
        result = await executor.execute(ActionType.CHECK_BOUNTIES, ctx)
        assert result.status == ActionStatus.SUCCESS
        assert result.token_cost == 2
        world.check_bounties.assert_called_once()

    @pytest.mark.asyncio
    async def test_accept_bounty_success(self):
        executor = ActionExecutor()
        agent = _MockAgentState(tokens=100)
        world = _MockWorldClient()
        ctx = ActionContext(agent=agent, world=world, parameters={"bounty_id": "b-1"})
        result = await executor.execute(ActionType.ACCEPT_BOUNTY, ctx)
        assert result.status == ActionStatus.SUCCESS
        assert result.token_cost == 10
        world.claim_bounty.assert_called_once_with("b-1")

    @pytest.mark.asyncio
    async def test_accept_bounty_missing_bounty_id(self):
        executor = ActionExecutor()
        agent = _MockAgentState(tokens=100)
        world = _MockWorldClient()
        ctx = ActionContext(agent=agent, world=world, parameters={})
        result = await executor.execute(ActionType.ACCEPT_BOUNTY, ctx)
        assert result.status == ActionStatus.RETRY_EXHAUSTED

    @pytest.mark.asyncio
    async def test_complete_bounty_success(self):
        executor = ActionExecutor()
        agent = _MockAgentState(tokens=100)
        world = _MockWorldClient()
        ctx = ActionContext(
            agent=agent, world=world,
            parameters={"bounty_id": "b-1", "result": "Collected 50 wood"},
        )
        result = await executor.execute(ActionType.COMPLETE_BOUNTY, ctx)
        assert result.status == ActionStatus.SUCCESS
        assert result.token_cost == 8
        world.complete_bounty.assert_called_once_with("b-1", "Collected 50 wood")

    @pytest.mark.asyncio
    async def test_complete_bounty_missing_result(self):
        executor = ActionExecutor()
        agent = _MockAgentState(tokens=100)
        world = _MockWorldClient()
        ctx = ActionContext(agent=agent, world=world, parameters={"bounty_id": "b-1"})
        result = await executor.execute(ActionType.COMPLETE_BOUNTY, ctx)
        assert result.status == ActionStatus.RETRY_EXHAUSTED

    @pytest.mark.asyncio
    async def test_insufficient_tokens(self):
        executor = ActionExecutor()
        agent = _MockAgentState(tokens=2)
        world = _MockWorldClient()
        ctx = ActionContext(agent=agent, world=world, parameters={"bounty_id": "b-1"})
        result = await executor.execute(ActionType.ACCEPT_BOUNTY, ctx)
        assert result.status == ActionStatus.INSUFFICIENT_TOKENS
        assert result.token_cost == 0

    @pytest.mark.asyncio
    async def test_token_deduction(self):
        executor = ActionExecutor()
        agent = _MockAgentState(tokens=100)
        world = _MockWorldClient()
        ctx = ActionContext(
            agent=agent, world=world,
            parameters={"oracle_id": "o-1", "response": "ok"},
        )
        await executor.execute(ActionType.RESPOND_ORACLE, ctx)
        assert agent.tokens == 97
