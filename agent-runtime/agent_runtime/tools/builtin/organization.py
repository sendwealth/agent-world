"""Built-in tool: Organization.

Allows agents to create, join, leave, dissolve organizations and query
organization details.
"""

from __future__ import annotations

from typing import Any, Dict, Optional

from ..base import ToolParameters, ToolResult, ToolStatus
from .world_engine_base import WorldEngineTool


class OrganizationParams(ToolParameters):
    """Parameters for the organization tool."""

    action: str  # create, list, get, join, leave, dissolve, distribution
    org_id: Optional[str] = None
    name: Optional[str] = None
    org_type: Optional[str] = None  # company, guild, alliance, university
    description: Optional[str] = None
    charter: Optional[str] = None
    agent_id: Optional[str] = None


class OrganizationTool(WorldEngineTool):
    """Interact with the organization subsystem.

    Actions:
    - create: Create a new organization.
    - list: List all organizations.
    - get: Get organization details.
    - join: Join an organization.
    - leave: Leave an organization.
    - dissolve: Dissolve an organization.
    - distribution: Calculate profit distribution for an org.
    """

    @property
    def name(self) -> str:
        return "organization"

    @property
    def description(self) -> str:
        return "Manage organizations: create, join, leave, dissolve, query details"

    @property
    def category(self) -> str:
        return "organization"

    @property
    def timeout(self) -> float:
        return 15.0

    @property
    def parameters_schema(self) -> type[ToolParameters]:
        return OrganizationParams

    @property
    def _valid_actions(self) -> set[str]:
        return {"create", "list", "get", "join", "leave", "dissolve", "distribution"}

    async def execute_async(self, params: ToolParameters) -> ToolResult:
        assert isinstance(params, OrganizationParams)

        action = params.action
        param_dict = params.model_dump(exclude_none=True, exclude={"action"})

        validation = self._validate_action(action)
        if validation is not None:
            return validation

        if self._sandbox:
            return self._sandbox_response(action, param_dict)

        try:
            if action == "create":
                return await self._create(params)
            elif action == "list":
                return await self._list(params)
            elif action == "get":
                return await self._get(params)
            elif action == "join":
                return await self._join(params)
            elif action == "leave":
                return await self._leave(params)
            elif action == "dissolve":
                return await self._dissolve(params)
            elif action == "distribution":
                return await self._distribution(params)
            else:
                return self._make_error_result(f"Unknown organization action: {action}")
        except Exception as exc:
            return self._make_error_result(str(exc))

    async def _create(self, params: OrganizationParams) -> ToolResult:
        body: Dict[str, Any] = {}
        if params.name:
            body["name"] = params.name
        if params.org_type:
            body["org_type"] = params.org_type
        if params.description:
            body["description"] = params.description
        if params.charter:
            body["charter"] = params.charter
        if params.agent_id:
            body["founder_id"] = params.agent_id

        data = await self._post("/orgs", json=body)
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _list(self, params: OrganizationParams) -> ToolResult:
        data = await self._get("/orgs")
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _get(self, params: OrganizationParams) -> ToolResult:
        data = await self._get(f"/orgs/{params.org_id}")
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _join(self, params: OrganizationParams) -> ToolResult:
        body: Dict[str, Any] = {}
        if params.agent_id:
            body["agent_id"] = params.agent_id
        data = await self._post(f"/orgs/{params.org_id}/join", json=body)
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _leave(self, params: OrganizationParams) -> ToolResult:
        body: Dict[str, Any] = {}
        if params.agent_id:
            body["agent_id"] = params.agent_id
        data = await self._post(f"/orgs/{params.org_id}/leave", json=body)
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _dissolve(self, params: OrganizationParams) -> ToolResult:
        data = await self._post(f"/orgs/{params.org_id}/dissolve", json={})
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _distribution(self, params: OrganizationParams) -> ToolResult:
        data = await self._post(f"/orgs/{params.org_id}/distribution", json={})
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)
