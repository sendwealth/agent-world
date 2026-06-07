"""Built-in tool: Governance.

Allows agents to participate in organizational governance — create proposals,
vote on proposals, tally votes, and query governance metrics.
"""

from __future__ import annotations

from typing import Any, Dict, List, Optional

from ..base import ToolParameters, ToolResult, ToolStatus
from .world_engine_base import WorldEngineTool


class GovernanceParams(ToolParameters):
    """Parameters for the governance tool."""

    # create_proposal, vote, start_voting, tally, cancel_proposal,
    # add_argument, list_proposals, get_proposal, summary, org_metrics,
    # timeline, comparison, list_legislation
    action: str
    org_id: Optional[str] = None
    proposal_id: Optional[str] = None
    # amend_charter, accept_member, expel_member, dissolve_org,
    # change_profit_sharing
    proposal_type: Optional[str] = None
    title: Optional[str] = None
    description: Optional[str] = None
    vote: Optional[str] = None  # for, against, abstain
    argument: Optional[str] = None
    argument_side: Optional[str] = None  # for, against
    proposer_id: Optional[str] = None
    voter_id: Optional[str] = None
    status: Optional[str] = None  # filter by proposal status
    org_ids: Optional[List[str]] = None  # for comparison action
    rule_type: Optional[str] = None  # for legislation filter


class GovernanceTool(WorldEngineTool):
    """Interact with the governance subsystem — proposals, voting, metrics.

    Actions:
    - create_proposal: Create a governance proposal for an org.
    - vote: Cast a vote on a proposal.
    - start_voting: Start the voting phase on a proposal.
    - tally: Tally votes on a proposal.
    - cancel_proposal: Cancel a proposal.
    - add_argument: Add a debate argument to a proposal.
    - list_proposals: List proposals for an org.
    - get_proposal: Get details of a single proposal.
    - summary: Get world governance summary metrics.
    - org_metrics: Get governance metrics for a specific org.
    - timeline: Get governance event timeline for an org.
    - comparison: Compare governance metrics across multiple orgs.
    - list_legislation: Query legislation history for an org.
    """

    @property
    def name(self) -> str:
        return "governance"

    @property
    def description(self) -> str:
        return (
            "Participate in organizational governance:"
            " create proposals, vote, tally, query metrics"
        )

    @property
    def category(self) -> str:
        return "governance"

    @property
    def timeout(self) -> float:
        return 15.0

    @property
    def parameters_schema(self) -> type[ToolParameters]:
        return GovernanceParams

    @property
    def _valid_actions(self) -> set[str]:
        return {
            "create_proposal", "vote", "start_voting", "tally",
            "cancel_proposal", "add_argument", "list_proposals",
            "get_proposal", "summary", "org_metrics", "timeline",
            "comparison", "list_legislation",
        }

    async def execute_async(self, params: ToolParameters) -> ToolResult:
        assert isinstance(params, GovernanceParams)

        action = params.action
        param_dict = params.model_dump(exclude_none=True, exclude={"action"})

        validation = self._validate_action(action)
        if validation is not None:
            return validation

        if self._sandbox:
            return self._sandbox_response(action, param_dict)

        try:
            if action == "create_proposal":
                return await self._create_proposal(params)
            elif action == "vote":
                return await self._vote(params)
            elif action == "start_voting":
                return await self._start_voting(params)
            elif action == "tally":
                return await self._tally(params)
            elif action == "cancel_proposal":
                return await self._cancel_proposal(params)
            elif action == "add_argument":
                return await self._add_argument(params)
            elif action == "list_proposals":
                return await self._list_proposals(params)
            elif action == "get_proposal":
                return await self._get_proposal(params)
            elif action == "summary":
                return await self._summary(params)
            elif action == "org_metrics":
                return await self._org_metrics(params)
            elif action == "timeline":
                return await self._timeline(params)
            elif action == "comparison":
                return await self._comparison(params)
            elif action == "list_legislation":
                return await self._list_legislation(params)
            else:
                return self._make_error_result(f"Unknown governance action: {action}")
        except Exception as exc:
            return self._make_error_result(str(exc))

    async def _create_proposal(self, params: GovernanceParams) -> ToolResult:
        body: Dict[str, Any] = {}
        if params.proposal_type:
            body["proposal_type"] = params.proposal_type
        if params.title:
            body["title"] = params.title
        if params.description:
            body["description"] = params.description
        if params.proposer_id:
            body["proposer_id"] = params.proposer_id

        data = await self._post(f"/orgs/{params.org_id}/proposals", json=body)
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _vote(self, params: GovernanceParams) -> ToolResult:
        body: Dict[str, Any] = {}
        if params.vote:
            body["vote"] = params.vote
        if params.voter_id:
            body["voter_id"] = params.voter_id

        data = await self._post(f"/proposals/{params.proposal_id}/vote", json=body)
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _start_voting(self, params: GovernanceParams) -> ToolResult:
        data = await self._post(f"/proposals/{params.proposal_id}/start-voting", json={})
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _tally(self, params: GovernanceParams) -> ToolResult:
        data = await self._post(f"/proposals/{params.proposal_id}/tally", json={})
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _cancel_proposal(self, params: GovernanceParams) -> ToolResult:
        data = await self._post(f"/proposals/{params.proposal_id}/cancel", json={})
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _add_argument(self, params: GovernanceParams) -> ToolResult:
        body: Dict[str, Any] = {}
        if params.argument:
            body["argument"] = params.argument
        if params.argument_side:
            body["side"] = params.argument_side
        if params.proposer_id:
            body["author_id"] = params.proposer_id

        data = await self._post(f"/proposals/{params.proposal_id}/arguments", json=body)
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _list_proposals(self, params: GovernanceParams) -> ToolResult:
        data = await self._get(f"/orgs/{params.org_id}/proposals")
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _get_proposal(self, params: GovernanceParams) -> ToolResult:
        data = await self._get(f"/proposals/{params.proposal_id}")
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _summary(self, params: GovernanceParams) -> ToolResult:
        data = await self._get("/governance/summary")
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _org_metrics(self, params: GovernanceParams) -> ToolResult:
        data = await self._get(f"/governance/orgs/{params.org_id}")
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _timeline(self, params: GovernanceParams) -> ToolResult:
        data = await self._get(f"/governance/orgs/{params.org_id}/timeline")
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _comparison(self, params: GovernanceParams) -> ToolResult:
        query_params: Dict[str, Any] = {}
        if params.org_ids:
            query_params["org_ids"] = ",".join(params.org_ids)
        data = await self._get("/governance/comparison", params=query_params)
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _list_legislation(self, params: GovernanceParams) -> ToolResult:
        query_params: Dict[str, Any] = {}
        if params.status:
            query_params["status"] = params.status
        if params.rule_type:
            query_params["rule_type"] = params.rule_type
        data = await self._get(f"/governance/orgs/{params.org_id}/legislation", params=query_params)
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)
