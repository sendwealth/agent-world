"""Built-in tool: Reputation.

Allows agents to query reputation scores, rankings, and system config.
"""

from __future__ import annotations

from typing import Any

from ..base import ToolParameters, ToolResult, ToolStatus
from .world_engine_base import WorldEngineTool


class ReputationParams(ToolParameters):
    """Parameters for the reputation tool."""

    action: str  # get_score, rankings, low_reputation, config
    agent_id: str | None = None
    limit: int | None = None


class ReputationTool(WorldEngineTool):
    """Interact with the reputation subsystem.

    Actions:
    - get_score: Get an agent's reputation score.
    - rankings: Get reputation rankings.
    - low_reputation: Get agents with low reputation.
    - config: Get reputation system config.
    """

    @property
    def name(self) -> str:
        return "reputation"

    @property
    def description(self) -> str:
        return "Query reputation scores, rankings, and system configuration"

    @property
    def category(self) -> str:
        return "social"

    @property
    def timeout(self) -> float:
        return 10.0

    @property
    def parameters_schema(self) -> type[ToolParameters]:
        return ReputationParams

    @property
    def _valid_actions(self) -> set[str]:
        return {"get_score", "rankings", "low_reputation", "config"}

    async def execute_async(self, params: ToolParameters) -> ToolResult:
        assert isinstance(params, ReputationParams)

        action = params.action
        param_dict = params.model_dump(exclude_none=True, exclude={"action"})

        validation = self._validate_action(action)
        if validation is not None:
            return validation

        if self._sandbox:
            return self._sandbox_response(action, param_dict)

        try:
            if action == "get_score":
                data = await self._get(f"/reputation/{params.agent_id}")
                return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)
            elif action == "rankings":
                query: dict[str, Any] = {}
                if params.limit is not None:
                    query["limit"] = params.limit
                data = await self._get("/reputation/rankings", params=query)
                return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)
            elif action == "low_reputation":
                data = await self._get("/reputation/low-reputation")
                return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)
            elif action == "config":
                data = await self._get("/reputation/config")
                return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)
            else:
                return self._make_error_result(f"Unknown reputation action: {action}")
        except Exception as exc:
            return self._make_error_result(str(exc))
