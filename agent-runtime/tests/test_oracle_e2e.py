"""End-to-end tests for Oracle → Agent integration.

Covers the full flow:
  1. Human sends Oracle → stored in MessageQueue
  2. Agent ThinkLoop Perceive phase drains Oracle from queue
  3. Decision layer receives Oracle context (pending_oracles in DecisionPerception)
  4. Agent decides RESPOND_ORACLE with response parameters
  5. ActionExecutor dispatches respond_oracle to world client
  6. Oracle response is recorded

Also covers:
  - DecisionPerception carries pending_oracles / pending_bounties fields
  - DecisionEngine prompt includes Oracle section when oracles are pending
  - _perception_to_decision extracts Oracle messages from perception.messages
  - _DECISION_TO_ACTION mapping includes RESPOND_ORACLE
  - _NoOpWorldClient supports respond_to_oracle
  - Full ThinkLoop tick with Oracle in perception → correct action dispatched
"""

from __future__ import annotations

from typing import Any

import pytest

from agent_runtime.actions.oracle_responder import (
    OracleResponder,
    OracleResponseStrategy,
)
from agent_runtime.core.act import (
    ActionContext,
    ActionExecutor,
    ActionStatus,
    ActionType,
)
from agent_runtime.core.decide import (
    DecisionAction,
    DecisionPerception,
    SurvivalAssessment,
    build_prompt,
)
from agent_runtime.core.llm_decide import (
    _DECISION_TO_ACTION,
    _perception_to_decision,
)
from agent_runtime.core.message_queue import MessageQueue, OracleMessage
from agent_runtime.core.message_queue import OracleType as MQOracleType
from agent_runtime.core.think_loop import (
    Decision,
    Perception,
    ThinkLoop,
    ThinkLoopConfig,
    _NoOpWorldClient,
)
from agent_runtime.models.agent_state import AgentState
from agent_runtime.models.enums import AgentPhase
from agent_runtime.survival.instinct import (
    SurvivalAction,
    SurvivalInstinct,
)

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def make_state(tokens: int = 500, max_tokens: int = 1000) -> AgentState:
    """Create a test AgentState."""
    return AgentState(
        name="TestAgent",
        tokens=tokens,
        max_tokens=max_tokens,
        money=50.0,
        health=100.0,
        phase=AgentPhase.ADULT,
    )


def make_oracle_perception(
    oracle_id: str = "oracle-1",
    oracle_type: str = "guidance",
    content: str = "Focus on building shelters.",
    human_id: str = "human-42",
) -> Perception:
    """Create a Perception containing an Oracle message."""
    return Perception(
        messages=[
            {
                "kind": "oracle",
                "id": "msg-1",
                "oracle_id": oracle_id,
                "type": oracle_type,
                "content": content,
                "from_human": True,
                "human_id": human_id,
            }
        ],
        token_balance=500,
        token_ratio=0.5,
        market_state={},
        active_task=None,
        health=100.0,
        tick=10,
    )


# ---------------------------------------------------------------------------
# Test: DecisionPerception carries Oracle data
# ---------------------------------------------------------------------------


class TestDecisionPerceptionOracles:
    """Verify that Oracle messages survive the Perception → DecisionPerception bridge."""

    def test_oracle_extracted_from_messages(self):
        perception = make_oracle_perception()
        dec = _perception_to_decision(perception)
        assert len(dec.pending_oracles) == 1
        assert dec.pending_oracles[0]["oracle_id"] == "oracle-1"
        assert dec.pending_oracles[0]["type"] == "guidance"
        assert dec.pending_oracles[0]["content"] == "Focus on building shelters."

    def test_bounty_extracted_from_messages(self):
        perception = Perception(
            messages=[
                {
                    "kind": "bounty",
                    "id": "msg-2",
                    "bounty_id": "b-1",
                    "title": "Gather 50 wood",
                    "description": "Collect wood from the forest.",
                    "reward": 100,
                    "human_id": "human-1",
                }
            ],
            tick=5,
        )
        dec = _perception_to_decision(perception)
        assert len(dec.pending_bounties) == 1
        assert dec.pending_bounties[0]["bounty_id"] == "b-1"
        assert dec.pending_bounties[0]["reward"] == 100

    def test_oracle_not_in_recent_events(self):
        """Oracle messages should be in pending_oracles, not recent_events."""
        perception = make_oracle_perception()
        dec = _perception_to_decision(perception)
        assert len(dec.recent_events) == 0

    def test_regular_messages_still_in_events(self):
        perception = Perception(
            messages=[
                {
                    "type": "INFORM",
                    "from_agent": "agent-2",
                    "payload": {"text": "Hello!"},
                },
                {
                    "kind": "oracle",
                    "oracle_id": "o-1",
                    "type": "guidance",
                    "content": "Be wise.",
                },
            ],
            tick=5,
        )
        dec = _perception_to_decision(perception)
        assert len(dec.recent_events) == 1
        assert "[agent-2] Hello!" in dec.recent_events
        assert len(dec.pending_oracles) == 1

    def test_empty_perception_no_oracles(self):
        perception = Perception(messages=[], tick=1)
        dec = _perception_to_decision(perception)
        assert dec.pending_oracles == []
        assert dec.pending_bounties == []


# ---------------------------------------------------------------------------
# Test: DecisionEngine prompt includes Oracle section
# ---------------------------------------------------------------------------


class TestOracleInPrompt:
    """Verify that the LLM prompt includes pending Oracle data."""

    def test_prompt_contains_oracle(self):
        state = make_state()
        perception = DecisionPerception(
            tick=10,
            pending_oracles=[
                {
                    "oracle_id": "oracle-1",
                    "type": "guidance",
                    "content": "Build houses for shelter.",
                    "human_id": "human-42",
                }
            ],
        )
        survival = SurvivalAssessment()
        available = [DecisionAction.REST, DecisionAction.RESPOND_ORACLE]
        prompt = build_prompt(state, perception, survival, available)
        assert "oracle-1" in prompt
        assert "guidance" in prompt
        assert "Build houses for shelter." in prompt
        assert "human-42" in prompt
        assert "Pending Oracles" in prompt

    def test_prompt_no_oracles_message(self):
        state = make_state()
        perception = DecisionPerception(tick=1)
        survival = SurvivalAssessment()
        available = [DecisionAction.REST]
        prompt = build_prompt(state, perception, survival, available)
        assert "No pending oracles." in prompt

    def test_prompt_contains_bounty(self):
        state = make_state()
        perception = DecisionPerception(
            tick=10,
            pending_bounties=[
                {
                    "bounty_id": "b-1",
                    "title": "Gather 50 wood",
                    "reward": 100,
                    "description": "Collect wood from the forest.",
                }
            ],
        )
        survival = SurvivalAssessment()
        available = [DecisionAction.REST, DecisionAction.ACCEPT_BOUNTY]
        prompt = build_prompt(state, perception, survival, available)
        assert "b-1" in prompt
        assert "Gather 50 wood" in prompt
        assert "100" in prompt
        assert "Pending Bounties" in prompt


# ---------------------------------------------------------------------------
# Test: _DECISION_TO_ACTION mapping includes Oracle/Bounty
# ---------------------------------------------------------------------------


class TestDecisionActionMapping:
    """Verify that DecisionAction → ActionType mapping covers Oracle/Bounty."""

    def test_respond_oracle_mapped(self):
        assert _DECISION_TO_ACTION[DecisionAction.RESPOND_ORACLE] == ActionType.RESPOND_ORACLE

    def test_check_bounties_mapped(self):
        assert _DECISION_TO_ACTION[DecisionAction.CHECK_BOUNTIES] == ActionType.CHECK_BOUNTIES

    def test_accept_bounty_mapped(self):
        assert _DECISION_TO_ACTION[DecisionAction.ACCEPT_BOUNTY] == ActionType.ACCEPT_BOUNTY

    def test_complete_bounty_mapped(self):
        assert _DECISION_TO_ACTION[DecisionAction.COMPLETE_BOUNTY] == ActionType.COMPLETE_BOUNTY


# ---------------------------------------------------------------------------
# Test: _NoOpWorldClient supports Oracle/Bounty
# ---------------------------------------------------------------------------


class TestNoOpWorldClientOracleBounty:
    """Verify _NoOpWorldClient returns valid responses for Oracle/Bounty actions."""

    @pytest.mark.asyncio
    async def test_respond_to_oracle(self):
        client = _NoOpWorldClient()
        result = await client.respond_to_oracle("oracle-1", "Thank you!")
        assert result["status"] == "ok"
        assert result["oracle_id"] == "oracle-1"

    @pytest.mark.asyncio
    async def test_check_bounties(self):
        client = _NoOpWorldClient()
        result = await client.check_bounties()
        assert result["status"] == "ok"
        assert "bounties" in result

    @pytest.mark.asyncio
    async def test_claim_bounty(self):
        client = _NoOpWorldClient()
        result = await client.claim_bounty("b-1")
        assert result["status"] == "ok"
        assert result["bounty_id"] == "b-1"

    @pytest.mark.asyncio
    async def test_complete_bounty(self):
        client = _NoOpWorldClient()
        result = await client.complete_bounty("b-1", "Done!")
        assert result["status"] == "ok"
        assert result["bounty_id"] == "b-1"


# ---------------------------------------------------------------------------
# Test: Full E2E flow — Oracle perception → action execution
# ---------------------------------------------------------------------------


class _OracleDecisionProvider:
    """A mock DecisionProvider that always chooses RESPOND_ORACLE when oracles are pending."""

    async def decide(
        self,
        state: AgentState,
        perception: Perception,
        survival: SurvivalAction,
    ) -> Decision:
        # Check if there are oracle messages in perception
        for msg in perception.messages:
            if isinstance(msg, dict) and msg.get("kind") == "oracle":
                return Decision(
                    action_type=ActionType.RESPOND_ORACLE,
                    parameters={
                        "oracle_id": msg["oracle_id"],
                        "response": "Thank you for your guidance. I will follow this advice.",
                    },
                    reasoning="Responding to Oracle from human.",
                )
        return Decision(action_type=ActionType.REST, reasoning="No oracle pending.")


class _OraclePerceptionProvider:
    """A perception provider that injects an Oracle message."""

    def __init__(self, oracle_msg: dict[str, Any]) -> None:
        self._oracle = oracle_msg

    async def perceive(self, state: AgentState, tick: int) -> Perception:
        max_tokens = getattr(state, "max_tokens", 1000)
        ratio = state.tokens / max_tokens if max_tokens > 0 else 0.0
        return Perception(
            messages=[self._oracle],
            token_balance=state.tokens,
            token_ratio=ratio,
            market_state={},
            active_task=None,
            health=state.health,
            tick=tick,
        )


class _TrackingWorldClient(_NoOpWorldClient):
    """World client that tracks method calls for assertion."""

    def __init__(self) -> None:
        super().__init__()
        self.responded_oracles: list[tuple[str, str]] = []

    async def respond_to_oracle(self, oracle_id: str, response: str) -> dict[str, Any]:
        self.responded_oracles.append((oracle_id, response))
        return {"status": "ok", "action": "respond_oracle", "oracle_id": oracle_id}

    async def broadcast_message(self, payload: dict[str, object]) -> dict[str, object]:
        return {"status": "ok", "action": "broadcast"}


class TestOracleE2EFlow:
    """End-to-end test: Oracle → Perceive → Decide → Act → Response recorded."""

    @pytest.mark.asyncio
    async def test_full_oracle_flow(self):
        """Simulate: Human sends Oracle → Agent perceives → decides to respond → acts."""
        state = make_state(tokens=500, max_tokens=1000)
        survival = SurvivalInstinct()
        executor = ActionExecutor()

        oracle_msg = {
            "kind": "oracle",
            "id": "msg-1",
            "oracle_id": "oracle-e2e",
            "type": "guidance",
            "content": "Focus on building shelters for the community.",
            "from_human": True,
            "human_id": "human-42",
        }

        world_client = _TrackingWorldClient()

        loop = ThinkLoop(
            state=state,
            survival=survival,
            executor=executor,
            config=ThinkLoopConfig(tick_interval=0),
            perception_provider=_OraclePerceptionProvider(oracle_msg),
            decision_provider=_OracleDecisionProvider(),
            world_client=world_client,
        )

        await loop.run(max_ticks=1)

        # Verify the Oracle was responded to
        assert len(world_client.responded_oracles) == 1
        oracle_id, response = world_client.responded_oracles[0]
        assert oracle_id == "oracle-e2e"
        lower = response.lower()
        assert "guidance" in lower or "Thank you" in response or "advice" in lower
        # Verify token was deducted (RESPOND_ORACLE costs 3)
        assert state.tokens == 497  # 500 - 3

    @pytest.mark.asyncio
    async def test_oracle_perception_with_message_queue(self):
        """Test that MessageQueue correctly delivers Oracle to perception dict."""
        queue = MessageQueue()
        oracle_msg = OracleMessage(
            id="msg-1",
            oracle_id="oracle-mq",
            type=MQOracleType.GUIDANCE,
            content="Be careful with resources.",
            from_human=True,
            human_id="human-1",
        )
        queue.enqueue(oracle_msg)

        msgs = queue.dequeue()
        assert len(msgs) == 1
        assert isinstance(msgs[0], OracleMessage)
        assert msgs[0].oracle_id == "oracle-mq"

        # Convert to perception dict
        perception_dict = msgs[0].to_perception_dict()
        assert perception_dict["kind"] == "oracle"
        assert perception_dict["oracle_id"] == "oracle-mq"
        assert perception_dict["type"] == "guidance"
        assert perception_dict["content"] == "Be careful with resources."

        queue.ack(msgs[0].id)

    @pytest.mark.asyncio
    async def test_oracle_action_through_executor(self):
        """Verify the ActionExecutor correctly dispatches RESPOND_ORACLE."""
        executor = ActionExecutor()
        agent_state = make_state(tokens=100)
        world = _TrackingWorldClient()

        ctx = ActionContext(
            agent=agent_state,
            world=world,
            parameters={
                "oracle_id": "oracle-exec",
                "response": "I acknowledge your warning.",
            },
        )
        result = await executor.execute(ActionType.RESPOND_ORACLE, ctx)

        assert result.status == ActionStatus.SUCCESS
        assert result.token_cost == 3
        assert len(world.responded_oracles) == 1
        assert world.responded_oracles[0] == ("oracle-exec", "I acknowledge your warning.")

    @pytest.mark.asyncio
    async def test_oracle_responder_integration(self):
        """Verify OracleResponder produces a response that can be sent via the world client."""
        responder = OracleResponder(llm_provider=None)
        result = await responder.respond(
            oracle_id="oracle-resp",
            oracle_type="warning",
            content="A storm is approaching!",
            agent_name="TestAgent",
        )

        assert result.oracle_id == "oracle-resp"
        assert result.strategy == OracleResponseStrategy.HEED_WARNING
        assert result.response  # Non-empty response
        assert not result.used_llm

        # Now dispatch the response through the executor
        executor = ActionExecutor()
        state = make_state(tokens=100)
        world = _TrackingWorldClient()

        ctx = ActionContext(
            agent=state,
            world=world,
            parameters={"oracle_id": result.oracle_id, "response": result.response},
        )
        action_result = await executor.execute(ActionType.RESPOND_ORACLE, ctx)

        assert action_result.status == ActionStatus.SUCCESS
        assert len(world.responded_oracles) == 1
        assert world.responded_oracles[0][0] == "oracle-resp"

    @pytest.mark.asyncio
    async def test_no_oracle_rests(self):
        """When no Oracle is pending, agent should not try to respond."""
        state = make_state(tokens=100)
        survival = SurvivalInstinct()
        executor = ActionExecutor()

        # Perception provider that returns NO messages
        class EmptyPerceptionProvider:
            async def perceive(self, state: AgentState, tick: int) -> Perception:
                return Perception(
                    messages=[],
                    tick=tick,
                    token_balance=state.tokens,
                    health=state.health,
                )

        world_client = _TrackingWorldClient()

        loop = ThinkLoop(
            state=state,
            survival=survival,
            executor=executor,
            config=ThinkLoopConfig(tick_interval=0),
            perception_provider=EmptyPerceptionProvider(),
            decision_provider=_OracleDecisionProvider(),  # Only responds to oracles
            world_client=world_client,
        )

        await loop.run(max_ticks=1)

        # No Oracle was responded to
        assert len(world_client.responded_oracles) == 0
