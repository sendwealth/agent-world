"""Built-in tool: Trust.

Allows agents to record trust interactions, query trust scores, and
discover allies and enemies.
"""

from __future__ import annotations

from typing import Any

from ..base import ToolParameters, ToolResult, ToolStatus
from .world_engine_base import WorldEngineTool


class TrustParams(ToolParameters):
    """Parameters for the trust tool."""

    action: str  # interact, get_score, relationships, allies, enemies, stats
    from_agent_id: str | None = None
    to_agent_id: str | None = None
    agent_id: str | None = None
    interaction_type: str | None = None
    value: float | None = None
    context: str | None = None


class TrustTool(WorldEngineTool):
    """Interact with the trust network subsystem.

    Actions:
    - interact: Record a trust interaction between agents.
    - get_score: Get trust score between two agents.
    - relationships: List trust relationships for an agent.
    - allies: Get trusted allies for an agent.
    - enemies: Get enemies for an agent.
    - stats: Get trust network statistics.
    """

    @property
    def name(self) -> str:
        return "trust"

    @property
    def description(self) -> str:
        return "Manage trust network: record interactions, query scores, find allies/enemies"

    @property
    def category(self) -> str:
        return "social"

    @property
    def timeout(self) -> float:
        return 10.0

    @property
    def parameters_schema(self) -> type[ToolParameters]:
        return TrustParams

    @property
    def _valid_actions(self) -> set[str]:
        return {"interact", "get_score", "relationships", "allies", "enemies", "stats"}

    async def execute_async(self, params: ToolParameters) -> ToolResult:
        assert isinstance(params, TrustParams)

        action = params.action
        param_dict = params.model_dump(exclude_none=True, exclude={"action"})

        validation = self._validate_action(action)
        if validation is not None:
            return validation

        if self._sandbox:
            return self._sandbox_response(action, param_dict)

        try:
            if action == "interact":
                body: dict[str, Any] = {}
                if params.from_agent_id:
                    body["from_agent_id"] = params.from_agent_id
                if params.to_agent_id:
                    body["to_agent_id"] = params.to_agent_id
                if params.interaction_type:
                    body["interaction_type"] = params.interaction_type
                if params.value is not None:
                    body["value"] = params.value
                if params.context:
                    body["context"] = params.context
                data = await self._post("/trust/interact", json=body)
                return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

            elif action == "get_score":
                data = await self._get(f"/trust/{params.from_agent_id}/{params.to_agent_id}")
                return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

            elif action == "relationships":
                data = await self._get(f"/trust/relationships/{params.agent_id}")
                return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

            elif action == "allies":
                data = await self._get(f"/trust/allies/{params.agent_id}")
                return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

            elif action == "enemies":
                data = await self._get(f"/trust/enemies/{params.agent_id}")
                return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

            elif action == "stats":
                data = await self._get("/trust/stats")
                return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

            else:
                return self._make_error_result(f"Unknown trust action: {action}")
        except Exception as exc:
            return self._make_error_result(str(exc))
