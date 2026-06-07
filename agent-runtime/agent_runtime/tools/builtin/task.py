"""Built-in tool: Task.

Allows agents to create, claim, start, submit, review, and manage tasks
in the world-engine task system.
"""

from __future__ import annotations

from typing import Any, Dict, List, Optional

from ..base import ToolParameters, ToolResult, ToolStatus
from .world_engine_base import WorldEngineTool


class TaskParams(ToolParameters):
    """Parameters for the task tool."""

    action: str  # create, list, get, claim, start, submit, review, complete, expire, delete, list_coordination, join_coordination, contribute_coordination, complete_coordination, cancel_coordination
    task_id: Optional[str] = None
    coordination_task_id: Optional[str] = None
    title: Optional[str] = None
    description: Optional[str] = None
    reward: Optional[float] = None
    deadline_tick: Optional[int] = None
    creator_id: Optional[str] = None
    claimer_id: Optional[str] = None
    result_data: Optional[str] = None
    reviewer_id: Optional[str] = None
    approved: Optional[bool] = None
    rating: Optional[int] = None
    contribution_data: Optional[str] = None
    contributor_id: Optional[str] = None
    status: Optional[str] = None


class TaskTool(WorldEngineTool):
    """Interact with the task system — create, claim, submit, review tasks.

    Actions:
    - create: Create a new task.
    - list: List all tasks.
    - get: Get task details by ID.
    - claim: Claim a task.
    - start: Start working on a task.
    - submit: Submit task result.
    - review: Review (approve/reject) a task.
    - complete: Complete a task (with reputation update).
    - expire: Expire a task (with reputation penalty).
    - delete: Delete a task.
    - list_coordination: List coordination tasks.
    - join_coordination: Join a coordination task.
    - contribute_coordination: Submit contribution to a coordination task.
    - complete_coordination: Complete a coordination task.
    - cancel_coordination: Cancel a coordination task.
    """

    @property
    def name(self) -> str:
        return "task"

    @property
    def description(self) -> str:
        return "Manage tasks: create, claim, submit, review, complete tasks and coordination tasks"

    @property
    def category(self) -> str:
        return "task"

    @property
    def timeout(self) -> float:
        return 15.0

    @property
    def parameters_schema(self) -> type[ToolParameters]:
        return TaskParams

    @property
    def _valid_actions(self) -> set[str]:
        return {
            "create", "list", "get", "claim", "start", "submit",
            "review", "complete", "expire", "delete",
            "list_coordination", "join_coordination",
            "contribute_coordination", "complete_coordination",
            "cancel_coordination",
        }

    async def execute_async(self, params: ToolParameters) -> ToolResult:
        assert isinstance(params, TaskParams)

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
            elif action == "claim":
                return await self._claim(params)
            elif action == "start":
                return await self._start(params)
            elif action == "submit":
                return await self._submit(params)
            elif action == "review":
                return await self._review(params)
            elif action == "complete":
                return await self._complete(params)
            elif action == "expire":
                return await self._expire(params)
            elif action == "delete":
                return await self._delete(params)
            elif action == "list_coordination":
                return await self._list_coordination(params)
            elif action == "join_coordination":
                return await self._join_coordination(params)
            elif action == "contribute_coordination":
                return await self._contribute_coordination(params)
            elif action == "complete_coordination":
                return await self._complete_coordination(params)
            elif action == "cancel_coordination":
                return await self._cancel_coordination(params)
            else:
                return self._make_error_result(f"Unknown task action: {action}")
        except Exception as exc:
            return self._make_error_result(str(exc))

    async def _create(self, params: TaskParams) -> ToolResult:
        body: Dict[str, Any] = {}
        if params.title:
            body["title"] = params.title
        if params.description:
            body["description"] = params.description
        if params.reward is not None:
            body["reward"] = params.reward
        if params.deadline_tick is not None:
            body["deadline_tick"] = params.deadline_tick
        if params.creator_id:
            body["creator_id"] = params.creator_id

        data = await self._post("/tasks", json=body)
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _list(self, params: TaskParams) -> ToolResult:
        query: Dict[str, Any] = {}
        if params.status:
            query["status"] = params.status
        data = await self._get("/tasks", params=query)
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _get(self, params: TaskParams) -> ToolResult:
        data = await self._get(f"/tasks/{params.task_id}")
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _claim(self, params: TaskParams) -> ToolResult:
        body: Dict[str, Any] = {}
        if params.claimer_id:
            body["agent_id"] = params.claimer_id
        data = await self._post(f"/tasks/{params.task_id}/claim", json=body)
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _start(self, params: TaskParams) -> ToolResult:
        data = await self._post(f"/tasks/{params.task_id}/start", json={})
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _submit(self, params: TaskParams) -> ToolResult:
        body: Dict[str, Any] = {}
        if params.result_data:
            body["result"] = params.result_data
        data = await self._post(f"/tasks/{params.task_id}/submit", json=body)
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _review(self, params: TaskParams) -> ToolResult:
        body: Dict[str, Any] = {}
        if params.approved is not None:
            body["approved"] = params.approved
        if params.reviewer_id:
            body["reviewer_id"] = params.reviewer_id
        if params.rating is not None:
            body["rating"] = params.rating
        data = await self._post(f"/tasks/{params.task_id}/review", json=body)
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _complete(self, params: TaskParams) -> ToolResult:
        data = await self._post(f"/tasks/{params.task_id}/complete", json={})
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _expire(self, params: TaskParams) -> ToolResult:
        data = await self._post(f"/tasks/{params.task_id}/expire", json={})
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _delete(self, params: TaskParams) -> ToolResult:
        data = await self._delete(f"/tasks/{params.task_id}")
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _list_coordination(self, params: TaskParams) -> ToolResult:
        data = await self._get("/coordination-tasks")
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _join_coordination(self, params: TaskParams) -> ToolResult:
        body: Dict[str, Any] = {}
        if params.claimer_id:
            body["agent_id"] = params.claimer_id
        data = await self._post(f"/coordination-tasks/{params.coordination_task_id}/join", json=body)
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _contribute_coordination(self, params: TaskParams) -> ToolResult:
        body: Dict[str, Any] = {}
        if params.contribution_data:
            body["contribution"] = params.contribution_data
        if params.contributor_id:
            body["agent_id"] = params.contributor_id
        data = await self._post(f"/coordination-tasks/{params.coordination_task_id}/contribute", json=body)
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _complete_coordination(self, params: TaskParams) -> ToolResult:
        data = await self._post(f"/coordination-tasks/{params.coordination_task_id}/complete", json={})
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _cancel_coordination(self, params: TaskParams) -> ToolResult:
        data = await self._post(f"/coordination-tasks/{params.coordination_task_id}/cancel", json={})
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)
