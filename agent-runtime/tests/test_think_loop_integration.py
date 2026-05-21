"""Tests for Phase A Think Loop E2E integration.

Covers:
- GRPCPerceptionProvider wiring in CLI (WorldConnection)
- ActionType MOVE, GATHER, BUILD extensions
- Perception → Decision data flow fix (_perception_to_decision)
- Heartbeat RPC integration in ThinkLoop
- GRPCWorldClient move/gather/build methods
"""

from __future__ import annotations

from typing import Any
from unittest.mock import AsyncMock, MagicMock

import pytest

from agent_runtime.__main__ import (
    RESTWorldClient,
    WorldConnection,
    _A2AHeartbeatAdapter,
)
from agent_runtime.core.act import (
    ActionContext,
    ActionExecutor,
    ActionType,
)
from agent_runtime.core.decide import DecisionAction
from agent_runtime.core.llm_decide import (
    _DECISION_TO_ACTION,
    _UNMAPPABLE_ACTIONS,
    _perception_to_decision,
)
from agent_runtime.core.think_loop import (
    Perception,
    ThinkLoop,
    ThinkLoopConfig,
)
from agent_runtime.models.agent_state import AgentState
from agent_runtime.survival.instinct import SurvivalInstinct

# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------


class MockWorldClient:
    """Mock world client for testing action handlers."""

    def __init__(self) -> None:
        self.calls: list[dict[str, Any]] = []

    async def send_message(self, payload: dict[str, Any]) -> dict[str, Any]:
        self.calls.append(("send_message", payload))
        return {"status": "ok"}

    async def claim_task(self, task_id: str) -> dict[str, Any]:
        self.calls.append(("claim_task", task_id))
        return {"status": "ok", "task_id": task_id}

    async def submit_task(
        self, task_id: str, result: dict[str, Any]
    ) -> dict[str, Any]:
        self.calls.append(("submit_task", task_id))
        return {"status": "ok", "task_id": task_id}

    async def propose_deal(self, proposal: dict[str, Any]) -> dict[str, Any]:
        self.calls.append(("propose_deal", proposal))
        return {"status": "ok"}

    async def teach_skill(
        self, target_agent_id: str, skill_name: str, level: int
    ) -> dict[str, Any]:
        self.calls.append(("teach_skill", target_agent_id))
        return {"status": "ok"}

    async def explore(self, parameters: dict[str, Any]) -> dict[str, Any]:
        self.calls.append(("explore", parameters))
        return {"status": "ok", "agents": []}

    async def move(self, direction: str) -> dict[str, Any]:
        self.calls.append(("move", direction))
        return {"status": "ok", "direction": direction}

    async def gather(self, resource_type: str) -> dict[str, Any]:
        self.calls.append(("gather", resource_type))
        return {"status": "ok", "resource_type": resource_type}

    async def build(self, structure_type: str, **kwargs: Any) -> dict[str, Any]:
        self.calls.append(("build", structure_type))
        return {"status": "ok", "structure_type": structure_type}


@pytest.fixture
def agent_state() -> AgentState:
    return AgentState(name="TestAgent", tokens=500, max_tokens=1000, health=80.0)


@pytest.fixture
def mock_world() -> MockWorldClient:
    return MockWorldClient()


@pytest.fixture
def executor() -> ActionExecutor:
    return ActionExecutor()


# ---------------------------------------------------------------------------
# Test: ActionType MOVE, GATHER, BUILD
# ---------------------------------------------------------------------------


class TestNewActionTypes:
    def test_move_enum_exists(self) -> None:
        assert ActionType.MOVE == "move"

    def test_gather_enum_exists(self) -> None:
        assert ActionType.GATHER == "gather"

    def test_build_enum_exists(self) -> None:
        assert ActionType.BUILD == "build"

    def test_move_token_cost(self) -> None:
        executor = ActionExecutor()
        assert executor.get_cost(ActionType.MOVE) == 12

    def test_gather_token_cost(self) -> None:
        executor = ActionExecutor()
        assert executor.get_cost(ActionType.GATHER) == 8

    def test_build_token_cost(self) -> None:
        executor = ActionExecutor()
        assert executor.get_cost(ActionType.BUILD) == 20

    def test_can_afford_move(self) -> None:
        executor = ActionExecutor()
        state = AgentState(name="Rich", tokens=100, max_tokens=200)
        assert executor.can_afford(ActionType.MOVE, state)

    def test_cannot_afford_build(self) -> None:
        executor = ActionExecutor()
        state = AgentState(name="Poor", tokens=10, max_tokens=200)
        assert not executor.can_afford(ActionType.BUILD, state)


class TestNewActionHandlers:
    @pytest.mark.asyncio
    async def test_move_handler(
        self, executor: ActionExecutor, mock_world: MockWorldClient
    ) -> None:
        state = AgentState(name="Mover", tokens=500, max_tokens=1000)
        context = ActionContext(
            agent=state, world=mock_world, parameters={"direction": "north"}
        )
        result = await executor.execute(ActionType.MOVE, context)
        assert result.status.value == "success"
        assert result.token_cost == 12
        assert mock_world.calls[-1] == ("move", "north")

    @pytest.mark.asyncio
    async def test_move_requires_direction(
        self, executor: ActionExecutor, mock_world: MockWorldClient
    ) -> None:
        state = AgentState(name="Mover", tokens=500, max_tokens=1000)
        context = ActionContext(
            agent=state, world=mock_world, parameters={}
        )
        result = await executor.execute(ActionType.MOVE, context)
        assert result.status.value == "retry_exhausted"

    @pytest.mark.asyncio
    async def test_gather_handler(
        self, executor: ActionExecutor, mock_world: MockWorldClient
    ) -> None:
        state = AgentState(name="Gatherer", tokens=500, max_tokens=1000)
        context = ActionContext(
            agent=state,
            world=mock_world,
            parameters={"resource_type": "wood"},
        )
        result = await executor.execute(ActionType.GATHER, context)
        assert result.status.value == "success"
        assert result.token_cost == 8
        assert mock_world.calls[-1] == ("gather", "wood")

    @pytest.mark.asyncio
    async def test_gather_requires_resource_type(
        self, executor: ActionExecutor, mock_world: MockWorldClient
    ) -> None:
        state = AgentState(name="Gatherer", tokens=500, max_tokens=1000)
        context = ActionContext(
            agent=state, world=mock_world, parameters={}
        )
        result = await executor.execute(ActionType.GATHER, context)
        assert result.status.value == "retry_exhausted"

    @pytest.mark.asyncio
    async def test_build_handler(
        self, executor: ActionExecutor, mock_world: MockWorldClient
    ) -> None:
        state = AgentState(name="Builder", tokens=500, max_tokens=1000)
        context = ActionContext(
            agent=state,
            world=mock_world,
            parameters={"structure_type": "house"},
        )
        result = await executor.execute(ActionType.BUILD, context)
        assert result.status.value == "success"
        assert result.token_cost == 20
        assert mock_world.calls[-1] == ("build", "house")

    @pytest.mark.asyncio
    async def test_build_requires_structure_type(
        self, executor: ActionExecutor, mock_world: MockWorldClient
    ) -> None:
        state = AgentState(name="Builder", tokens=500, max_tokens=1000)
        context = ActionContext(
            agent=state, world=mock_world, parameters={}
        )
        result = await executor.execute(ActionType.BUILD, context)
        assert result.status.value == "retry_exhausted"


# ---------------------------------------------------------------------------
# Test: DecisionAction → ActionType mapping
# ---------------------------------------------------------------------------


class TestDecisionActionMapping:
    def test_move_mapped(self) -> None:
        assert _DECISION_TO_ACTION[DecisionAction.MOVE] == ActionType.MOVE

    def test_gather_mapped(self) -> None:
        assert _DECISION_TO_ACTION[DecisionAction.GATHER] == ActionType.GATHER

    def test_build_mapped(self) -> None:
        assert _DECISION_TO_ACTION[DecisionAction.BUILD] == ActionType.BUILD

    def test_no_unmappable_actions(self) -> None:
        assert len(_UNMAPPABLE_ACTIONS) == 0


# ---------------------------------------------------------------------------
# Test: Perception → Decision data flow
# ---------------------------------------------------------------------------


class TestPerceptionToDecision:
    def test_extracts_nearby_agents(self) -> None:
        p = Perception(
            tick=1,
            market_state={
                "nearby_agents": [
                    {"agent_id": "a1", "name": "Alice", "tokens": 100},
                    {"agent_id": "a2", "name": "Bob", "tokens": 200},
                ],
            },
        )
        result = _perception_to_decision(p)
        assert "Alice" in result.nearby_agents
        assert "Bob" in result.nearby_agents

    def test_extracts_nearby_agents_by_id_when_no_name(self) -> None:
        p = Perception(
            tick=1,
            market_state={
                "nearby_agents": [{"agent_id": "a1"}],
            },
        )
        result = _perception_to_decision(p)
        assert "a1" in result.nearby_agents

    def test_extracts_visible_resources(self) -> None:
        p = Perception(
            tick=1,
            market_state={"visible_resources": ["wood", "stone", "gold"]},
        )
        result = _perception_to_decision(p)
        assert result.visible_resources == ["wood", "stone", "gold"]

    def test_extracts_recent_events_from_messages(self) -> None:
        p = Perception(
            tick=1,
            messages=[
                {
                    "from_agent": "Alice",
                    "type": "INFORM",
                    "payload": {"text": "Hello"},
                },
                {
                    "from_agent": "Bob",
                    "type": "PROPOSE",
                    "payload": {"action": "trade"},
                },
            ],
        )
        result = _perception_to_decision(p)
        assert len(result.recent_events) == 2
        assert "[Alice] Hello" in result.recent_events
        assert "[Bob] trade" in result.recent_events

    def test_extracts_events_without_from_agent(self) -> None:
        p = Perception(
            tick=1,
            messages=[
                {
                    "type": "INFORM",
                    "payload": {"text": "World event"},
                },
            ],
        )
        result = _perception_to_decision(p)
        assert "World event" in result.recent_events

    def test_extracts_available_tasks_from_market_state(self) -> None:
        p = Perception(
            tick=1,
            market_state={"available_tasks": ["task-1", "task-2"]},
        )
        result = _perception_to_decision(p)
        assert "task-1" in result.available_tasks
        assert "task-2" in result.available_tasks

    def test_active_task_included_in_available_tasks(self) -> None:
        p = Perception(
            tick=1,
            active_task="my-task",
        )
        result = _perception_to_decision(p)
        assert "my-task" in result.available_tasks

    def test_empty_perception_gives_empty_lists(self) -> None:
        p = Perception(tick=1)
        result = _perception_to_decision(p)
        assert result.nearby_agents == []
        assert result.available_tasks == []
        assert result.visible_resources == []
        assert result.recent_events == []

    def test_string_messages_handled(self) -> None:
        p = Perception(
            tick=1,
            messages=["simple message"],
        )
        result = _perception_to_decision(p)
        assert "simple message" in result.recent_events


# ---------------------------------------------------------------------------
# Test: WorldConnection dataclass
# ---------------------------------------------------------------------------


class TestWorldConnection:
    def test_default_values(self) -> None:
        conn = WorldConnection(world_client="fake")
        assert conn.world_client == "fake"
        assert conn.perception_provider is None
        assert conn.a2a_client is None

    def test_with_all_fields(self) -> None:
        conn = WorldConnection(
            world_client="world",
            perception_provider="perception",
            a2a_client="a2a",
        )
        assert conn.world_client == "world"
        assert conn.perception_provider == "perception"
        assert conn.a2a_client == "a2a"


class TestRESTWorldClientNewActions:
    @pytest.mark.asyncio
    async def test_move_returns_standalone(self) -> None:
        client = RESTWorldClient("http://localhost:3000", agent_id="test-agent")
        result = await client.move("north")
        assert result["status"] == "standalone"

    @pytest.mark.asyncio
    async def test_gather_returns_standalone(self) -> None:
        client = RESTWorldClient("http://localhost:3000", agent_id="test-agent")
        result = await client.gather("wood")
        assert result["status"] == "standalone"

    @pytest.mark.asyncio
    async def test_build_returns_standalone(self) -> None:
        client = RESTWorldClient("http://localhost:3000", agent_id="test-agent")
        result = await client.build("house")
        assert result["status"] == "standalone"


# ---------------------------------------------------------------------------
# Test: Heartbeat integration in ThinkLoop
# ---------------------------------------------------------------------------


class MockHeartbeatProvider:
    """Mock heartbeat provider for testing."""

    def __init__(self, server_tick: int = 100) -> None:
        self._server_tick = server_tick
        self.call_count = 0

    async def heartbeat(self) -> int:
        self.call_count += 1
        return self._server_tick


class FailingHeartbeatProvider:
    """Heartbeat provider that always fails."""

    async def heartbeat(self) -> int:
        raise ConnectionError("Server unreachable")


class TestHeartbeatIntegration:
    @pytest.mark.asyncio
    async def test_heartbeat_called_each_tick(self) -> None:
        state = AgentState(name="HeartbeatTest", tokens=500, max_tokens=1000)
        survival = SurvivalInstinct()
        executor = ActionExecutor()
        heartbeat = MockHeartbeatProvider(server_tick=42)

        loop = ThinkLoop(
            state=state,
            survival=survival,
            executor=executor,
            config=ThinkLoopConfig(
                tick_interval=0.0, max_ticks=3, heartbeat_enabled=True
            ),
            heartbeat_provider=heartbeat,
        )
        await loop.run()
        assert heartbeat.call_count == 3
        assert loop.server_tick == 42

    @pytest.mark.asyncio
    async def test_heartbeat_not_called_when_disabled(self) -> None:
        state = AgentState(name="NoHeartbeat", tokens=500, max_tokens=1000)
        survival = SurvivalInstinct()
        executor = ActionExecutor()
        heartbeat = MockHeartbeatProvider()

        loop = ThinkLoop(
            state=state,
            survival=survival,
            executor=executor,
            config=ThinkLoopConfig(
                tick_interval=0.0, max_ticks=3, heartbeat_enabled=False
            ),
            heartbeat_provider=heartbeat,
        )
        await loop.run()
        assert heartbeat.call_count == 0

    @pytest.mark.asyncio
    async def test_heartbeat_failure_non_fatal(self) -> None:
        state = AgentState(name="FailHeartbeat", tokens=500, max_tokens=1000)
        survival = SurvivalInstinct()
        executor = ActionExecutor()
        heartbeat = FailingHeartbeatProvider()

        loop = ThinkLoop(
            state=state,
            survival=survival,
            executor=executor,
            config=ThinkLoopConfig(
                tick_interval=0.0, max_ticks=3, heartbeat_enabled=True
            ),
            heartbeat_provider=heartbeat,
        )
        await loop.run()
        # Should complete despite heartbeat failures
        assert loop.tick == 3
        assert loop.total_errors == 0  # Heartbeat failures don't count as errors

    @pytest.mark.asyncio
    async def test_heartbeat_without_provider(self) -> None:
        state = AgentState(name="NoProvider", tokens=500, max_tokens=1000)
        survival = SurvivalInstinct()
        executor = ActionExecutor()

        loop = ThinkLoop(
            state=state,
            survival=survival,
            executor=executor,
            config=ThinkLoopConfig(
                tick_interval=0.0, max_ticks=2, heartbeat_enabled=True
            ),
            heartbeat_provider=None,
        )
        await loop.run()
        assert loop.tick == 2


# ---------------------------------------------------------------------------
# Test: _A2AHeartbeatAdapter
# ---------------------------------------------------------------------------


class TestA2AHeartbeatAdapter:
    @pytest.mark.asyncio
    async def test_adapter_returns_server_time(self) -> None:
        mock_client = MagicMock()
        mock_response = MagicMock()
        mock_response.server_time = 12345
        mock_client.heartbeat = AsyncMock(return_value=mock_response)

        adapter = _A2AHeartbeatAdapter(mock_client)
        result = await adapter.heartbeat()
        assert result == 12345
        mock_client.heartbeat.assert_called_once()


# ---------------------------------------------------------------------------
# Test: Perception server_tick field
# ---------------------------------------------------------------------------


class TestPerceptionServerTick:
    def test_default_server_tick(self) -> None:
        p = Perception(tick=1)
        assert p.server_tick == 0

    def test_custom_server_tick(self) -> None:
        p = Perception(tick=1, server_tick=42)
        assert p.server_tick == 42


# ---------------------------------------------------------------------------
# Test: ThinkLoopConfig heartbeat_enabled field
# ---------------------------------------------------------------------------


class TestThinkLoopConfigHeartbeat:
    def test_default_disabled(self) -> None:
        config = ThinkLoopConfig()
        assert config.heartbeat_enabled is False

    def test_can_enable(self) -> None:
        config = ThinkLoopConfig(heartbeat_enabled=True)
        assert config.heartbeat_enabled is True
