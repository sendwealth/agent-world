"""Tests for the action executor (act.py).

Covers:
- ActionType enum: all types defined
- ActionStatus enum: all statuses defined
- ActionResult: creation, immutability
- Token costs: default costs, custom costs, can_afford
- execute() for every action type with mock world client
- Token deduction on success and on failure
- Insufficient tokens returns INSUFFICIENT_TOKENS without deduction
- Retry logic: success on retry, exhaustion
- Action history recording and clearing
- Missing required parameters raise ValueError
- REST action costs zero tokens and needs no world interaction
- Concurrency safety (sequential calls)
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any

import pytest

from agent_runtime.core.act import (
    ActionContext,
    ActionExecutor,
    ActionResult,
    ActionStatus,
    ActionType,
)

# ---------------------------------------------------------------------------
# Helpers: Fake agent state and world client
# ---------------------------------------------------------------------------


@dataclass
class FakeAgentState:
    """Minimal agent state for testing."""

    tokens: int = 1000

    def adjust_tokens(self, delta: int) -> None:
        new_balance = self.tokens + delta
        if new_balance < 0:
            raise ValueError(
                f"Cannot reduce tokens below 0 (current: {self.tokens}, delta: {delta})"
            )
        self.tokens = new_balance


class FakeWorldClient:
    """Mock world client that records calls and returns configurable results."""

    def __init__(self, *, should_fail: bool = False, fail_times: int = 0) -> None:
        self.calls: list[tuple[str, dict[str, Any]]] = []
        self._should_fail = should_fail
        self._fail_times = fail_times
        self._call_count = 0

    async def send_message(self, payload: dict[str, Any]) -> dict[str, Any]:
        self._call_count += 1
        self.calls.append(("send_message", payload))
        if self._should_fail or self._call_count <= self._fail_times:
            raise ConnectionError("A2A network unreachable")
        return {"status": "sent", "message_id": "msg-123"}

    async def claim_task(self, task_id: str) -> dict[str, Any]:
        self._call_count += 1
        self.calls.append(("claim_task", {"task_id": task_id}))
        if self._should_fail or self._call_count <= self._fail_times:
            raise RuntimeError("Task already claimed")
        return {"status": "claimed", "task_id": task_id}

    async def submit_task(self, task_id: str, result: dict[str, Any]) -> dict[str, Any]:
        self._call_count += 1
        self.calls.append(("submit_task", {"task_id": task_id, "result": result}))
        if self._should_fail or self._call_count <= self._fail_times:
            raise RuntimeError("Task submission rejected")
        return {"status": "accepted", "reward": 50}

    async def propose_deal(self, proposal: dict[str, Any]) -> dict[str, Any]:
        self._call_count += 1
        self.calls.append(("propose_deal", proposal))
        if self._should_fail or self._call_count <= self._fail_times:
            raise ConnectionError("Deal proposal failed")
        return {"status": "proposed", "deal_id": "deal-456"}

    async def teach_skill(
        self, target_agent_id: str, skill_name: str, level: int
    ) -> dict[str, Any]:
        self._call_count += 1
        self.calls.append(
            ("teach_skill", {"target": target_agent_id, "skill": skill_name, "level": level})
        )
        if self._should_fail or self._call_count <= self._fail_times:
            raise RuntimeError("Teaching failed")
        return {"status": "taught", "skill": skill_name}

    async def explore(self, parameters: dict[str, Any]) -> dict[str, Any]:
        self._call_count += 1
        self.calls.append(("explore", parameters))
        if self._should_fail or self._call_count <= self._fail_times:
            raise RuntimeError("Exploration blocked")
        return {"status": "explored", "discoveries": ["a mine", "a market"]}


# ---------------------------------------------------------------------------
# Enum tests
# ---------------------------------------------------------------------------


class TestActionType:
    def test_all_types_defined(self) -> None:
        expected = {
            "send_message",
            "claim_task",
            "submit_task",
            "propose_deal",
            "teach_skill",
            "practice_skill",
            "rest",
            "explore",
            "move",
            "gather",
            "build",
            "socialize",
            "form_org",
            "join_org",
            "propose_rule",
            "vote_rule",
            "respond_oracle",
            "check_bounties",
            "accept_bounty",
            "complete_bounty",
        }
        assert {t.value for t in ActionType} == expected

    def test_is_string_enum(self) -> None:
        assert isinstance(ActionType.SEND_MESSAGE, str)
        assert ActionType.SEND_MESSAGE == "send_message"


class TestActionStatus:
    def test_all_statuses_defined(self) -> None:
        expected = {
            "success",
            "failed",
            "insufficient_tokens",
            "skipped",
            "retry_exhausted",
            "blocked_by_intervention",
        }
        assert {s.value for s in ActionStatus} == expected

    def test_is_string_enum(self) -> None:
        assert isinstance(ActionStatus.SUCCESS, str)


# ---------------------------------------------------------------------------
# ActionResult tests
# ---------------------------------------------------------------------------


class TestActionResult:
    def test_create_minimal(self) -> None:
        r = ActionResult(
            action_type=ActionType.SEND_MESSAGE,
            status=ActionStatus.SUCCESS,
        )
        assert r.action_type == ActionType.SEND_MESSAGE
        assert r.status == ActionStatus.SUCCESS
        assert r.token_cost == 0
        assert r.data == {}
        assert r.error is None
        assert r.attempts == 1
        assert r.elapsed_ms == 0.0

    def test_create_full(self) -> None:
        r = ActionResult(
            action_type=ActionType.CLAIM_TASK,
            status=ActionStatus.FAILED,
            token_cost=5,
            data={"task_id": "t1"},
            error="Already claimed",
            attempts=3,
            elapsed_ms=12.5,
            timestamp=1700000000.0,
        )
        assert r.token_cost == 5
        assert r.error == "Already claimed"
        assert r.attempts == 3

    def test_frozen(self) -> None:
        r = ActionResult(
            action_type=ActionType.REST,
            status=ActionStatus.SUCCESS,
        )
        with pytest.raises(AttributeError):
            r.status = ActionStatus.FAILED  # type: ignore[misc]


# ---------------------------------------------------------------------------
# Token cost tests
# ---------------------------------------------------------------------------


class TestTokenCosts:
    def test_default_costs(self) -> None:
        executor = ActionExecutor()
        assert executor.get_cost(ActionType.SEND_MESSAGE) == 10
        assert executor.get_cost(ActionType.CLAIM_TASK) == 5
        assert executor.get_cost(ActionType.SUBMIT_TASK) == 8
        assert executor.get_cost(ActionType.PROPOSE_DEAL) == 10
        assert executor.get_cost(ActionType.TEACH_SKILL) == 15
        assert executor.get_cost(ActionType.REST) == 0
        assert executor.get_cost(ActionType.EXPLORE) == 3

    def test_custom_costs_override(self) -> None:
        executor = ActionExecutor(token_costs={ActionType.SEND_MESSAGE: 20})
        assert executor.get_cost(ActionType.SEND_MESSAGE) == 20
        # Others unchanged
        assert executor.get_cost(ActionType.CLAIM_TASK) == 5

    def test_custom_costs_add_new(self) -> None:
        """Custom costs dict is merged over defaults."""
        executor = ActionExecutor(token_costs={ActionType.EXPLORE: 10})
        assert executor.get_cost(ActionType.EXPLORE) == 10
        assert executor.get_cost(ActionType.SEND_MESSAGE) == 10  # default

    def test_can_afford_sufficient_tokens(self) -> None:
        executor = ActionExecutor()
        agent = FakeAgentState(tokens=100)
        assert executor.can_afford(ActionType.SEND_MESSAGE, agent) is True

    def test_can_afford_insufficient_tokens(self) -> None:
        executor = ActionExecutor()
        agent = FakeAgentState(tokens=5)
        assert executor.can_afford(ActionType.SEND_MESSAGE, agent) is False

    def test_can_afford_exact_amount(self) -> None:
        executor = ActionExecutor()
        agent = FakeAgentState(tokens=10)
        assert executor.can_afford(ActionType.SEND_MESSAGE, agent) is True

    def test_can_afford_zero_cost(self) -> None:
        executor = ActionExecutor()
        agent = FakeAgentState(tokens=0)
        assert executor.can_afford(ActionType.REST, agent) is True

    def test_can_afford_one_short(self) -> None:
        executor = ActionExecutor()
        agent = FakeAgentState(tokens=9)
        assert executor.can_afford(ActionType.SEND_MESSAGE, agent) is False


# ---------------------------------------------------------------------------
# Execute: success paths for each action type
# ---------------------------------------------------------------------------


class TestExecuteSendMessage:
    @pytest.mark.asyncio
    async def test_success(self) -> None:
        executor = ActionExecutor()
        agent = FakeAgentState(tokens=100)
        world = FakeWorldClient()
        ctx = ActionContext(
            agent=agent,
            world=world,
            parameters={"payload": {"type": "INFORM", "content": "Hello!"}},
        )

        result = await executor.execute(ActionType.SEND_MESSAGE, ctx)

        assert result.status == ActionStatus.SUCCESS
        assert result.action_type == ActionType.SEND_MESSAGE
        assert result.token_cost == 10
        assert agent.tokens == 90
        assert world.calls[0][0] == "send_message"
        assert result.data["status"] == "sent"

    @pytest.mark.asyncio
    async def test_insufficient_tokens(self) -> None:
        executor = ActionExecutor()
        agent = FakeAgentState(tokens=5)
        world = FakeWorldClient()
        ctx = ActionContext(agent=agent, world=world, parameters={"payload": {}})

        result = await executor.execute(ActionType.SEND_MESSAGE, ctx)

        assert result.status == ActionStatus.INSUFFICIENT_TOKENS
        assert result.token_cost == 0
        assert agent.tokens == 5  # unchanged
        assert len(world.calls) == 0


class TestExecuteClaimTask:
    @pytest.mark.asyncio
    async def test_success(self) -> None:
        executor = ActionExecutor()
        agent = FakeAgentState(tokens=100)
        world = FakeWorldClient()
        ctx = ActionContext(
            agent=agent,
            world=world,
            parameters={"task_id": "task-001"},
        )

        result = await executor.execute(ActionType.CLAIM_TASK, ctx)

        assert result.status == ActionStatus.SUCCESS
        assert result.token_cost == 5
        assert agent.tokens == 95
        assert result.data["task_id"] == "task-001"

    @pytest.mark.asyncio
    async def test_missing_task_id_degrades_gracefully(self) -> None:
        """No task_id → graceful no_tasks_available result, not retry-exhausted."""
        executor = ActionExecutor(max_retries=3, retry_delay=0.0)
        agent = FakeAgentState(tokens=100)
        world = FakeWorldClient()
        ctx = ActionContext(agent=agent, world=world, parameters={})

        result = await executor.execute(ActionType.CLAIM_TASK, ctx)

        assert result.status == ActionStatus.SUCCESS
        assert result.attempts == 1  # no retries on graceful degradation
        assert result.data["action"] == "claim_task"
        assert result.data["status"] == "no_tasks_available"
        # World client must NOT have been called
        assert len(world.calls) == 0


class TestExecuteSubmitTask:
    @pytest.mark.asyncio
    async def test_success(self) -> None:
        executor = ActionExecutor()
        agent = FakeAgentState(tokens=100)
        world = FakeWorldClient()
        ctx = ActionContext(
            agent=agent,
            world=world,
            parameters={"task_id": "task-001", "result": {"output": "done"}},
        )

        result = await executor.execute(ActionType.SUBMIT_TASK, ctx)

        assert result.status == ActionStatus.SUCCESS
        assert result.token_cost == 8
        assert agent.tokens == 92
        assert result.data["reward"] == 50

    @pytest.mark.asyncio
    async def test_missing_task_id(self) -> None:
        executor = ActionExecutor()
        agent = FakeAgentState(tokens=100)
        world = FakeWorldClient()
        ctx = ActionContext(agent=agent, world=world, parameters={"result": {}})

        result = await executor.execute(ActionType.SUBMIT_TASK, ctx)
        assert result.status == ActionStatus.RETRY_EXHAUSTED


class TestExecuteProposeDeal:
    @pytest.mark.asyncio
    async def test_success(self) -> None:
        executor = ActionExecutor()
        agent = FakeAgentState(tokens=100)
        world = FakeWorldClient()
        ctx = ActionContext(
            agent=agent,
            world=world,
            parameters={"proposal": {"action": "trade", "amount": 100}},
        )

        result = await executor.execute(ActionType.PROPOSE_DEAL, ctx)

        assert result.status == ActionStatus.SUCCESS
        assert result.token_cost == 10
        assert agent.tokens == 90
        assert result.data["deal_id"] == "deal-456"


class TestExecuteTeachSkill:
    @pytest.mark.asyncio
    async def test_success(self) -> None:
        executor = ActionExecutor()
        agent = FakeAgentState(tokens=100)
        world = FakeWorldClient()
        ctx = ActionContext(
            agent=agent,
            world=world,
            parameters={
                "target_agent_id": "agent-002",
                "skill_name": "coding",
                "level": 5,
            },
        )

        result = await executor.execute(ActionType.TEACH_SKILL, ctx)

        assert result.status == ActionStatus.SUCCESS
        assert result.token_cost == 15
        assert agent.tokens == 85
        assert result.data["skill"] == "coding"

    @pytest.mark.asyncio
    async def test_missing_target_agent_id(self) -> None:
        executor = ActionExecutor()
        agent = FakeAgentState(tokens=100)
        world = FakeWorldClient()
        ctx = ActionContext(
            agent=agent,
            world=world,
            parameters={"skill_name": "coding"},
        )

        result = await executor.execute(ActionType.TEACH_SKILL, ctx)
        assert result.status == ActionStatus.RETRY_EXHAUSTED
        assert "target_agent_id" in (result.error or "")

    @pytest.mark.asyncio
    async def test_missing_skill_name(self) -> None:
        executor = ActionExecutor()
        agent = FakeAgentState(tokens=100)
        world = FakeWorldClient()
        ctx = ActionContext(
            agent=agent,
            world=world,
            parameters={"target_agent_id": "agent-002"},
        )

        result = await executor.execute(ActionType.TEACH_SKILL, ctx)
        assert result.status == ActionStatus.RETRY_EXHAUSTED
        assert "skill_name" in (result.error or "")


class TestExecutePracticeSkill:
    @pytest.mark.asyncio
    async def test_success_with_skill_name(self) -> None:
        executor = ActionExecutor()
        agent = FakeAgentState(tokens=100)
        world = FakeWorldClient()
        ctx = ActionContext(
            agent=agent,
            world=world,
            parameters={"skill_name": "foraging"},
        )

        result = await executor.execute(ActionType.PRACTICE_SKILL, ctx)

        assert result.status == ActionStatus.SUCCESS
        assert result.token_cost == 8
        assert agent.tokens == 92
        assert result.data["action"] == "practice_skill"
        assert result.data["status"] == "practiced"
        assert result.data["skill"] == "foraging"
        # Self-practice needs no world interaction
        assert len(world.calls) == 0

    @pytest.mark.asyncio
    async def test_success_without_skill_name(self) -> None:
        """No skill_name is fine — practice_skill needs no required params."""
        executor = ActionExecutor()
        agent = FakeAgentState(tokens=100)
        world = FakeWorldClient()
        ctx = ActionContext(agent=agent, world=world, parameters={})

        result = await executor.execute(ActionType.PRACTICE_SKILL, ctx)

        assert result.status == ActionStatus.SUCCESS
        assert result.token_cost == 8
        assert result.data["status"] == "practiced"
        assert len(world.calls) == 0

    @pytest.mark.asyncio
    async def test_no_target_required(self) -> None:
        """practice_skill must NOT behave like teach_skill (which needs a target).

        Regression guard for the original bug where PRACTICE_SKILL was mapped
        to TEACH_SKILL, causing a 'teach_skill requires target_agent_id' error.
        """
        executor = ActionExecutor()
        agent = FakeAgentState(tokens=100)
        world = FakeWorldClient()
        ctx = ActionContext(
            agent=agent,
            world=world,
            parameters={},  # deliberately empty — no target, no skill
        )

        result = await executor.execute(ActionType.PRACTICE_SKILL, ctx)

        assert result.status == ActionStatus.SUCCESS
        assert result.attempts == 1
        assert result.error is None


class TestExecuteRest:
    @pytest.mark.asyncio
    async def test_success(self) -> None:
        executor = ActionExecutor()
        agent = FakeAgentState(tokens=100)
        world = FakeWorldClient()
        ctx = ActionContext(agent=agent, world=world)

        result = await executor.execute(ActionType.REST, ctx)

        assert result.status == ActionStatus.SUCCESS
        assert result.token_cost == 0
        assert agent.tokens == 100  # unchanged
        assert result.data["action"] == "rest"
        # No world client calls
        assert len(world.calls) == 0

    @pytest.mark.asyncio
    async def test_rest_with_zero_tokens(self) -> None:
        """REST should succeed even with zero tokens (cost is 0)."""
        executor = ActionExecutor()
        agent = FakeAgentState(tokens=0)
        world = FakeWorldClient()
        ctx = ActionContext(agent=agent, world=world)

        result = await executor.execute(ActionType.REST, ctx)

        assert result.status == ActionStatus.SUCCESS
        assert result.token_cost == 0
        assert agent.tokens == 0


class TestExecuteExplore:
    @pytest.mark.asyncio
    async def test_success(self) -> None:
        executor = ActionExecutor()
        agent = FakeAgentState(tokens=100)
        world = FakeWorldClient()
        ctx = ActionContext(agent=agent, world=world, parameters={"explore_params": {"radius": 5}})

        result = await executor.execute(ActionType.EXPLORE, ctx)

        assert result.status == ActionStatus.SUCCESS
        assert result.token_cost == 3
        assert agent.tokens == 97
        assert "discoveries" in result.data

    @pytest.mark.asyncio
    async def test_explore_default_params(self) -> None:
        executor = ActionExecutor()
        agent = FakeAgentState(tokens=100)
        world = FakeWorldClient()
        ctx = ActionContext(agent=agent, world=world, parameters={})

        result = await executor.execute(ActionType.EXPLORE, ctx)

        assert result.status == ActionStatus.SUCCESS
        # explore handler uses {} as default when no explore_params key
        assert world.calls[0][0] == "explore"


# ---------------------------------------------------------------------------
# Token deduction tests
# ---------------------------------------------------------------------------


class TestTokenDeduction:
    @pytest.mark.asyncio
    async def test_tokens_deducted_on_success(self) -> None:
        executor = ActionExecutor()
        agent = FakeAgentState(tokens=50)
        world = FakeWorldClient()
        ctx = ActionContext(
            agent=agent,
            world=world,
            parameters={"task_id": "t1"},
        )

        await executor.execute(ActionType.CLAIM_TASK, ctx)
        assert agent.tokens == 45  # 50 - 5

    @pytest.mark.asyncio
    async def test_tokens_deducted_once_on_retry_success(self) -> None:
        """Tokens should be deducted only once even across retries."""
        executor = ActionExecutor(max_retries=3, retry_delay=0.0)
        agent = FakeAgentState(tokens=50)
        world = FakeWorldClient(fail_times=2)  # fail first 2, succeed on 3rd
        ctx = ActionContext(
            agent=agent,
            world=world,
            parameters={"task_id": "t1"},
        )

        result = await executor.execute(ActionType.CLAIM_TASK, ctx)
        assert result.status == ActionStatus.SUCCESS
        assert result.attempts == 3
        assert agent.tokens == 45  # deducted only once: 50 - 5

    @pytest.mark.asyncio
    async def test_tokens_deducted_on_retry_exhausted(self) -> None:
        """Tokens are deducted even when all retries fail."""
        executor = ActionExecutor(max_retries=2, retry_delay=0.0)
        agent = FakeAgentState(tokens=50)
        world = FakeWorldClient(should_fail=True)
        ctx = ActionContext(
            agent=agent,
            world=world,
            parameters={"task_id": "t1"},
        )

        result = await executor.execute(ActionType.CLAIM_TASK, ctx)
        assert result.status == ActionStatus.RETRY_EXHAUSTED
        assert result.token_cost == 5
        assert agent.tokens == 45  # still deducted

    @pytest.mark.asyncio
    async def test_no_deduction_on_insufficient_tokens(self) -> None:
        executor = ActionExecutor()
        agent = FakeAgentState(tokens=3)
        world = FakeWorldClient()
        ctx = ActionContext(
            agent=agent,
            world=world,
            parameters={"task_id": "t1"},
        )

        result = await executor.execute(ActionType.CLAIM_TASK, ctx)
        assert result.status == ActionStatus.INSUFFICIENT_TOKENS
        assert result.token_cost == 0
        assert agent.tokens == 3


# ---------------------------------------------------------------------------
# Retry logic
# ---------------------------------------------------------------------------


class TestRetryLogic:
    @pytest.mark.asyncio
    async def test_success_on_first_try(self) -> None:
        executor = ActionExecutor(max_retries=3, retry_delay=0.0)
        agent = FakeAgentState(tokens=100)
        world = FakeWorldClient()
        ctx = ActionContext(
            agent=agent,
            world=world,
            parameters={"task_id": "t1"},
        )

        result = await executor.execute(ActionType.CLAIM_TASK, ctx)
        assert result.status == ActionStatus.SUCCESS
        assert result.attempts == 1

    @pytest.mark.asyncio
    async def test_success_on_second_try(self) -> None:
        executor = ActionExecutor(max_retries=3, retry_delay=0.0)
        agent = FakeAgentState(tokens=100)
        world = FakeWorldClient(fail_times=1)
        ctx = ActionContext(
            agent=agent,
            world=world,
            parameters={"task_id": "t1"},
        )

        result = await executor.execute(ActionType.CLAIM_TASK, ctx)
        assert result.status == ActionStatus.SUCCESS
        assert result.attempts == 2

    @pytest.mark.asyncio
    async def test_success_on_third_try(self) -> None:
        executor = ActionExecutor(max_retries=3, retry_delay=0.0)
        agent = FakeAgentState(tokens=100)
        world = FakeWorldClient(fail_times=2)
        ctx = ActionContext(
            agent=agent,
            world=world,
            parameters={"task_id": "t1"},
        )

        result = await executor.execute(ActionType.CLAIM_TASK, ctx)
        assert result.status == ActionStatus.SUCCESS
        assert result.attempts == 3

    @pytest.mark.asyncio
    async def test_exhausted_after_max_retries(self) -> None:
        executor = ActionExecutor(max_retries=3, retry_delay=0.0)
        agent = FakeAgentState(tokens=100)
        world = FakeWorldClient(should_fail=True)
        ctx = ActionContext(
            agent=agent,
            world=world,
            parameters={"task_id": "t1"},
        )

        result = await executor.execute(ActionType.CLAIM_TASK, ctx)
        assert result.status == ActionStatus.RETRY_EXHAUSTED
        assert result.attempts == 3
        assert result.error is not None

    @pytest.mark.asyncio
    async def test_single_attempt_with_max_retries_1(self) -> None:
        executor = ActionExecutor(max_retries=1, retry_delay=0.0)
        agent = FakeAgentState(tokens=100)
        world = FakeWorldClient(should_fail=True)
        ctx = ActionContext(
            agent=agent,
            world=world,
            parameters={"task_id": "t1"},
        )

        result = await executor.execute(ActionType.CLAIM_TASK, ctx)
        assert result.status == ActionStatus.RETRY_EXHAUSTED
        assert result.attempts == 1

    @pytest.mark.asyncio
    async def test_error_message_preserved(self) -> None:
        executor = ActionExecutor(max_retries=1, retry_delay=0.0)
        agent = FakeAgentState(tokens=100)
        world = FakeWorldClient(should_fail=True)
        ctx = ActionContext(
            agent=agent,
            world=world,
            parameters={"task_id": "t1"},
        )

        result = await executor.execute(ActionType.CLAIM_TASK, ctx)
        assert "already claimed" in (result.error or "").lower()


# ---------------------------------------------------------------------------
# Action history
# ---------------------------------------------------------------------------


class TestActionHistory:
    @pytest.mark.asyncio
    async def test_history_records_success(self) -> None:
        executor = ActionExecutor()
        agent = FakeAgentState(tokens=100)
        world = FakeWorldClient()
        ctx = ActionContext(agent=agent, world=world)

        await executor.execute(ActionType.REST, ctx)
        history = executor.history
        assert len(history) == 1
        assert history[0].status == ActionStatus.SUCCESS
        assert history[0].action_type == ActionType.REST

    @pytest.mark.asyncio
    async def test_history_records_insufficient_tokens(self) -> None:
        executor = ActionExecutor()
        # Use 1 token: above IC-04 low-water mark (0) so intervention checker
        # passes, but below SEND_MESSAGE cost (10) → INSUFFICIENT_TOKENS.
        agent = FakeAgentState(tokens=1)
        world = FakeWorldClient()
        ctx = ActionContext(agent=agent, world=world)

        await executor.execute(ActionType.SEND_MESSAGE, ctx)
        history = executor.history
        assert len(history) == 1
        # InterventionChecker (IC-04) blocks before the regular token check
        assert history[0].status in (
            ActionStatus.INSUFFICIENT_TOKENS,
            ActionStatus.BLOCKED_BY_INTERVENTION,
        )

    @pytest.mark.asyncio
    async def test_history_accumulates(self) -> None:
        executor = ActionExecutor()
        agent = FakeAgentState(tokens=100)
        world = FakeWorldClient()
        ctx = ActionContext(agent=agent, world=world)

        await executor.execute(ActionType.REST, ctx)
        await executor.execute(ActionType.REST, ctx)
        await executor.execute(ActionType.REST, ctx)

        assert len(executor.history) == 3

    @pytest.mark.asyncio
    async def test_history_is_a_copy(self) -> None:
        """Returned history list should be a copy, not a reference."""
        executor = ActionExecutor()
        h1 = executor.history
        h2 = executor.history
        assert h1 is not h2

    @pytest.mark.asyncio
    async def test_clear_history(self) -> None:
        executor = ActionExecutor()
        agent = FakeAgentState(tokens=100)
        world = FakeWorldClient()
        ctx = ActionContext(agent=agent, world=world)

        await executor.execute(ActionType.REST, ctx)
        assert len(executor.history) == 1

        executor.clear_history()
        assert len(executor.history) == 0

    @pytest.mark.asyncio
    async def test_history_records_retry_exhausted(self) -> None:
        executor = ActionExecutor(max_retries=1, retry_delay=0.0)
        agent = FakeAgentState(tokens=100)
        world = FakeWorldClient(should_fail=True)
        ctx = ActionContext(
            agent=agent,
            world=world,
            parameters={"task_id": "t1"},
        )

        await executor.execute(ActionType.CLAIM_TASK, ctx)
        history = executor.history
        assert len(history) == 1
        assert history[0].status == ActionStatus.RETRY_EXHAUSTED
        assert history[0].attempts == 1


# ---------------------------------------------------------------------------
# Edge cases
# ---------------------------------------------------------------------------


class TestEdgeCases:
    @pytest.mark.asyncio
    async def test_elapsed_ms_is_positive(self) -> None:
        executor = ActionExecutor()
        agent = FakeAgentState(tokens=100)
        world = FakeWorldClient()
        ctx = ActionContext(agent=agent, world=world)

        result = await executor.execute(ActionType.REST, ctx)
        assert result.elapsed_ms >= 0.0

    @pytest.mark.asyncio
    async def test_timestamp_is_set(self) -> None:
        executor = ActionExecutor()
        agent = FakeAgentState(tokens=100)
        world = FakeWorldClient()
        ctx = ActionContext(agent=agent, world=world)

        result = await executor.execute(ActionType.REST, ctx)
        assert result.timestamp > 0

    @pytest.mark.asyncio
    async def test_execute_all_types(self) -> None:
        """Verify every action type can execute successfully."""
        executor = ActionExecutor()
        agent = FakeAgentState(tokens=1000)
        world = FakeWorldClient()

        actions_and_params = [
            (ActionType.SEND_MESSAGE, {"payload": {"msg": "hi"}}),
            (ActionType.CLAIM_TASK, {"task_id": "t1"}),
            (ActionType.SUBMIT_TASK, {"task_id": "t1", "result": {"done": True}}),
            (ActionType.PROPOSE_DEAL, {"proposal": {"deal": "trade"}}),
            (
                ActionType.TEACH_SKILL,
                {
                    "target_agent_id": "a2",
                    "skill_name": "coding",
                    "level": 3,
                },
            ),
            (ActionType.REST, {}),
            (ActionType.EXPLORE, {"explore_params": {"radius": 1}}),
        ]

        for action_type, params in actions_and_params:
            ctx = ActionContext(agent=agent, world=world, parameters=params)
            result = await executor.execute(action_type, ctx)
            assert result.status == ActionStatus.SUCCESS, (
                f"{action_type.value} failed: {result.error}"
            )

        # All 7 actions succeeded
        assert len(executor.history) == 7
        # Check total tokens deducted
        expected_cost = 10 + 5 + 8 + 10 + 15 + 0 + 3  # = 51
        assert agent.tokens == 1000 - expected_cost

    @pytest.mark.asyncio
    async def test_custom_cost_changes_affordability(self) -> None:
        executor = ActionExecutor(token_costs={ActionType.REST: 999})
        agent = FakeAgentState(tokens=100)
        world = FakeWorldClient()
        ctx = ActionContext(agent=agent, world=world)

        result = await executor.execute(ActionType.REST, ctx)
        assert result.status == ActionStatus.INSUFFICIENT_TOKENS

    @pytest.mark.asyncio
    async def test_result_data_contains_world_response(self) -> None:
        """Verify that the action result includes the world client response."""
        executor = ActionExecutor()
        agent = FakeAgentState(tokens=100)
        world = FakeWorldClient()
        ctx = ActionContext(
            agent=agent,
            world=world,
            parameters={"task_id": "t1"},
        )

        result = await executor.execute(ActionType.CLAIM_TASK, ctx)
        assert result.data["status"] == "claimed"
        assert result.data["task_id"] == "t1"

    @pytest.mark.asyncio
    async def test_sequential_executions_independent(self) -> None:
        """Each execution should be independent."""
        executor = ActionExecutor()
        # Use 26 tokens so after 3 successful actions (5+10+10=25),
        # 1 token remains — above IC-04 low-water mark (0) so the
        # intervention checker does not intercept, but below SEND_MESSAGE
        # cost (10) → INSUFFICIENT_TOKENS.
        agent = FakeAgentState(tokens=26)
        world = FakeWorldClient()

        # First: claim task (cost 5, remaining 21)
        ctx1 = ActionContext(
            agent=agent,
            world=world,
            parameters={"task_id": "t1"},
        )
        r1 = await executor.execute(ActionType.CLAIM_TASK, ctx1)
        assert r1.status == ActionStatus.SUCCESS

        # Second: send message (cost 10, remaining 11)
        ctx2 = ActionContext(
            agent=agent,
            world=world,
            parameters={"payload": {"msg": "hi"}},
        )
        r2 = await executor.execute(ActionType.SEND_MESSAGE, ctx2)
        assert r2.status == ActionStatus.SUCCESS

        # Third: send message again (cost 10, remaining 1)
        ctx3 = ActionContext(agent=agent, world=world, parameters={"payload": {}})
        r3 = await executor.execute(ActionType.SEND_MESSAGE, ctx3)
        assert r3.status == ActionStatus.SUCCESS

        # Fourth: not enough tokens for another send_message (1 < 10)
        # InterventionChecker (IC-04) blocks before the regular token check
        ctx4 = ActionContext(agent=agent, world=world, parameters={"payload": {}})
        r4 = await executor.execute(ActionType.SEND_MESSAGE, ctx4)
        assert r4.status in (
            ActionStatus.INSUFFICIENT_TOKENS,
            ActionStatus.BLOCKED_BY_INTERVENTION,
        )

        assert agent.tokens == 1  # 26 - 5 - 10 - 10
