"""Built-in tool: Legislation.

Allows agents to manage legislation cycles — start cycles, submit rules,
vote, tally, and query legislation effects.
"""

from __future__ import annotations

from typing import Any, Dict, Optional

from ..base import ToolParameters, ToolResult, ToolStatus
from .world_engine_base import WorldEngineTool


class LegislationParams(ToolParameters):
    """Parameters for the legislation tool."""

    action: str
    org_id: Optional[str] = None
    cycle_id: Optional[str] = None
    # Cycle management
    leader_id: Optional[str] = None
    # Rules
    rule_type: Optional[str] = None  # tax, trade, behavior, diplomacy, custom
    rule_name: Optional[str] = None
    rule_description: Optional[str] = None
    rule_parameters: Optional[str] = None  # JSON string
    proposer_id: Optional[str] = None
    # Voting
    voter_id: Optional[str] = None
    vote: Optional[str] = None  # for, against, abstain
    # Repeal
    rule_id: Optional[str] = None
    repeal_reason: Optional[str] = None
    # Filters
    status: Optional[str] = None
    include_completed: Optional[bool] = None


class LegislationTool(WorldEngineTool):
    """Interact with the legislation cycle subsystem.

    Actions:
    - start_cycle: Start a new legislation cycle (triggers election).
    - start_cycle_with_leader: Start cycle with a pre-elected leader.
    - full_cycle: Run full legislation cycle in one call.
    - list_active: List active legislation cycles.
    - list_completed: List completed legislation cycles.
    - get_cycle: Get current cycle status for an org.
    - get_rules: Get candidate rules for a cycle.
    - submit_rule: Submit a candidate rule.
    - start_voting: Start voting phase.
    - cast_vote: Cast a vote on legislation.
    - tally: Tally votes and enact passed rules.
    - effects: Evaluate cycle effects.
    - repeal: Submit repeal proposal.
    """

    @property
    def name(self) -> str:
        return "legislation"

    @property
    def description(self) -> str:
        return "Manage legislation cycles: submit rules, vote, tally, repeal laws"

    @property
    def category(self) -> str:
        return "governance"

    @property
    def timeout(self) -> float:
        return 15.0

    @property
    def parameters_schema(self) -> type[ToolParameters]:
        return LegislationParams

    @property
    def _valid_actions(self) -> set[str]:
        return {
            "start_cycle", "start_cycle_with_leader", "full_cycle",
            "list_active", "list_completed", "get_cycle",
            "get_rules", "submit_rule", "start_voting",
            "cast_vote", "tally", "effects", "repeal",
        }

    async def execute_async(self, params: ToolParameters) -> ToolResult:
        assert isinstance(params, LegislationParams)

        action = params.action
        param_dict = params.model_dump(exclude_none=True, exclude={"action"})

        validation = self._validate_action(action)
        if validation is not None:
            return validation

        if self._sandbox:
            return self._sandbox_response(action, param_dict)

        try:
            handlers = {
                "start_cycle": self._start_cycle,
                "start_cycle_with_leader": self._start_cycle_with_leader,
                "full_cycle": self._full_cycle,
                "list_active": self._list_active,
                "list_completed": self._list_completed,
                "get_cycle": self._get_cycle,
                "get_rules": self._get_rules,
                "submit_rule": self._submit_rule,
                "start_voting": self._start_voting,
                "cast_vote": self._cast_vote,
                "tally": self._tally,
                "effects": self._effects,
                "repeal": self._repeal,
            }
            handler = handlers.get(action)
            if handler is None:
                return self._make_error_result(f"Unknown legislation action: {action}")
            return await handler(params)
        except Exception as exc:
            return self._make_error_result(str(exc))

    async def _start_cycle(self, p: LegislationParams) -> ToolResult:
        body: Dict[str, Any] = {}
        if p.org_id:
            body["org_id"] = p.org_id
        data = await self._post("/legislation/cycles", json=body)
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _start_cycle_with_leader(self, p: LegislationParams) -> ToolResult:
        body: Dict[str, Any] = {}
        if p.org_id:
            body["org_id"] = p.org_id
        if p.leader_id:
            body["leader_id"] = p.leader_id
        data = await self._post("/legislation/cycles/with-leader", json=body)
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _full_cycle(self, p: LegislationParams) -> ToolResult:
        body: Dict[str, Any] = {}
        if p.org_id:
            body["org_id"] = p.org_id
        data = await self._post("/legislation/cycles/full", json=body)
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _list_active(self, p: LegislationParams) -> ToolResult:
        data = await self._get("/legislation/cycles/active")
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _list_completed(self, p: LegislationParams) -> ToolResult:
        data = await self._get("/legislation/cycles/completed")
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _get_cycle(self, p: LegislationParams) -> ToolResult:
        data = await self._get(f"/legislation/cycles/{p.org_id}")
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _get_rules(self, p: LegislationParams) -> ToolResult:
        data = await self._get(f"/legislation/cycles/{p.org_id}/rules")
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _submit_rule(self, p: LegislationParams) -> ToolResult:
        body: Dict[str, Any] = {}
        if p.rule_type:
            body["rule_type"] = p.rule_type
        if p.rule_name:
            body["name"] = p.rule_name
        if p.rule_description:
            body["description"] = p.rule_description
        if p.rule_parameters:
            body["parameters"] = p.rule_parameters
        if p.proposer_id:
            body["proposer_id"] = p.proposer_id
        data = await self._post(f"/legislation/cycles/{p.org_id}/rules", json=body)
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _start_voting(self, p: LegislationParams) -> ToolResult:
        data = await self._post(f"/legislation/cycles/{p.org_id}/voting", json={})
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _cast_vote(self, p: LegislationParams) -> ToolResult:
        body: Dict[str, Any] = {}
        if p.voter_id:
            body["voter_id"] = p.voter_id
        if p.vote:
            body["vote"] = p.vote
        if p.rule_id:
            body["rule_id"] = p.rule_id
        data = await self._post(f"/legislation/cycles/{p.org_id}/vote", json=body)
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _tally(self, p: LegislationParams) -> ToolResult:
        data = await self._post(f"/legislation/cycles/{p.org_id}/tally", json={})
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _effects(self, p: LegislationParams) -> ToolResult:
        data = await self._get(f"/legislation/cycles/{p.org_id}/effects")
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _repeal(self, p: LegislationParams) -> ToolResult:
        body: Dict[str, Any] = {}
        if p.rule_id:
            body["rule_id"] = p.rule_id
        if p.repeal_reason:
            body["reason"] = p.repeal_reason
        if p.proposer_id:
            body["proposer_id"] = p.proposer_id
        data = await self._post(f"/legislation/cycles/{p.org_id}/repeal", json=body)
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)
