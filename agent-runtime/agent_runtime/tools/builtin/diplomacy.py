"""Built-in tool: Diplomacy.

Allows agents to manage cross-world diplomatic relations — register worlds,
propose/accept/reject treaties, impose sanctions, declare war, propose peace.
"""

from __future__ import annotations

from typing import Any, Dict, Optional

from ..base import ToolParameters, ToolResult, ToolStatus
from .world_engine_base import WorldEngineTool


class DiplomacyParams(ToolParameters):
    """Parameters for the diplomacy tool."""

    action: str
    # World management
    world_name: Optional[str] = None
    world_url: Optional[str] = None
    world_id: Optional[str] = None
    world_type: Optional[str] = None
    # Diplomatic relations
    from_world_id: Optional[str] = None
    to_world_id: Optional[str] = None
    relation_type: Optional[str] = None
    # Treaties
    # non_aggression, trade_pact, military_alliance, research_exchange,
    # cultural_exchange
    treaty_type: Optional[str] = None
    treaty_id: Optional[str] = None
    terms: Optional[str] = None
    proposer_world_id: Optional[str] = None
    # Sanctions
    target_world_id: Optional[str] = None
    sanction_type: Optional[str] = None
    reason: Optional[str] = None
    sanction_id: Optional[str] = None
    # War & peace
    aggressor_world_id: Optional[str] = None
    defender_world_id: Optional[str] = None
    peace_id: Optional[str] = None
    peace_terms: Optional[str] = None


class DiplomacyTool(WorldEngineTool):
    """Interact with the cross-world diplomacy subsystem.

    Actions:
    - register_world: Register a new world.
    - list_worlds: List all registered worlds.
    - get_world: Get world details.
    - deregister_world: Remove a world.
    - establish_relations: Establish diplomatic relations between worlds.
    - propose_treaty: Propose a treaty.
    - list_treaties: List treaties.
    - get_treaty: Get treaty details.
    - accept_treaty: Accept a proposed treaty.
    - reject_treaty: Reject a proposed treaty.
    - break_treaty: Break an active treaty.
    - impose_sanctions: Impose sanctions on a world.
    - lift_sanctions: Lift sanctions.
    - sever_ties: Sever diplomatic ties.
    - declare_war: Declare war on another world.
    - propose_peace: Propose peace.
    - accept_peace: Accept a peace proposal.
    - summary: Get diplomacy summary.
    """

    @property
    def name(self) -> str:
        return "diplomacy"

    @property
    def description(self) -> str:
        return "Manage cross-world diplomacy: treaties, sanctions, war, peace, relations"

    @property
    def category(self) -> str:
        return "diplomacy"

    @property
    def timeout(self) -> float:
        return 15.0

    @property
    def parameters_schema(self) -> type[ToolParameters]:
        return DiplomacyParams

    @property
    def _valid_actions(self) -> set[str]:
        return {
            "register_world", "list_worlds", "get_world", "deregister_world",
            "establish_relations", "propose_treaty", "list_treaties",
            "get_treaty", "accept_treaty", "reject_treaty", "break_treaty",
            "impose_sanctions", "lift_sanctions", "sever_ties",
            "declare_war", "propose_peace", "accept_peace", "summary",
        }

    async def execute_async(self, params: ToolParameters) -> ToolResult:
        assert isinstance(params, DiplomacyParams)

        action = params.action
        param_dict = params.model_dump(exclude_none=True, exclude={"action"})

        validation = self._validate_action(action)
        if validation is not None:
            return validation

        if self._sandbox:
            return self._sandbox_response(action, param_dict)

        try:
            handlers = {
                "register_world": self._register_world,
                "list_worlds": self._list_worlds,
                "get_world": self._get_world,
                "deregister_world": self._deregister_world,
                "establish_relations": self._establish_relations,
                "propose_treaty": self._propose_treaty,
                "list_treaties": self._list_treaties,
                "get_treaty": self._get_treaty,
                "accept_treaty": self._accept_treaty,
                "reject_treaty": self._reject_treaty,
                "break_treaty": self._break_treaty,
                "impose_sanctions": self._impose_sanctions,
                "lift_sanctions": self._lift_sanctions,
                "sever_ties": self._sever_ties,
                "declare_war": self._declare_war,
                "propose_peace": self._propose_peace,
                "accept_peace": self._accept_peace,
                "summary": self._summary,
            }
            handler = handlers.get(action)
            if handler is None:
                return self._make_error_result(f"Unknown diplomacy action: {action}")
            return await handler(params)
        except Exception as exc:
            return self._make_error_result(str(exc))

    async def _register_world(self, p: DiplomacyParams) -> ToolResult:
        body: Dict[str, Any] = {}
        if p.world_name:
            body["name"] = p.world_name
        if p.world_url:
            body["url"] = p.world_url
        if p.world_type:
            body["world_type"] = p.world_type
        data = await self._post("/federation/worlds", json=body)
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _list_worlds(self, p: DiplomacyParams) -> ToolResult:
        data = await self._get("/federation/worlds")
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _get_world(self, p: DiplomacyParams) -> ToolResult:
        data = await self._get(f"/federation/worlds/{p.world_id}")
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _deregister_world(self, p: DiplomacyParams) -> ToolResult:
        data = await self._delete(f"/federation/worlds/{p.world_id}")
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _establish_relations(self, p: DiplomacyParams) -> ToolResult:
        body: Dict[str, Any] = {}
        if p.from_world_id:
            body["from_world_id"] = p.from_world_id
        if p.to_world_id:
            body["to_world_id"] = p.to_world_id
        if p.relation_type:
            body["relation_type"] = p.relation_type
        data = await self._post("/federation/establish-relations", json=body)
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _propose_treaty(self, p: DiplomacyParams) -> ToolResult:
        body: Dict[str, Any] = {}
        if p.treaty_type:
            body["treaty_type"] = p.treaty_type
        if p.from_world_id:
            body["proposer_world_id"] = p.from_world_id
        if p.to_world_id:
            body["target_world_id"] = p.to_world_id
        if p.terms:
            body["terms"] = p.terms
        data = await self._post("/federation/treaties", json=body)
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _list_treaties(self, p: DiplomacyParams) -> ToolResult:
        data = await self._get("/federation/treaties")
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _get_treaty(self, p: DiplomacyParams) -> ToolResult:
        data = await self._get(f"/federation/treaties/{p.treaty_id}")
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _accept_treaty(self, p: DiplomacyParams) -> ToolResult:
        data = await self._post(f"/federation/treaties/{p.treaty_id}/accept", json={})
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _reject_treaty(self, p: DiplomacyParams) -> ToolResult:
        data = await self._post(f"/federation/treaties/{p.treaty_id}/reject", json={})
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _break_treaty(self, p: DiplomacyParams) -> ToolResult:
        data = await self._post(f"/federation/treaties/{p.treaty_id}/break", json={})
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _impose_sanctions(self, p: DiplomacyParams) -> ToolResult:
        body: Dict[str, Any] = {}
        if p.from_world_id:
            body["imposer_world_id"] = p.from_world_id
        if p.target_world_id:
            body["target_world_id"] = p.target_world_id
        if p.sanction_type:
            body["sanction_type"] = p.sanction_type
        if p.reason:
            body["reason"] = p.reason
        data = await self._post("/federation/sanctions", json=body)
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _lift_sanctions(self, p: DiplomacyParams) -> ToolResult:
        data = await self._post(f"/federation/sanctions/{p.sanction_id}/lift", json={})
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _sever_ties(self, p: DiplomacyParams) -> ToolResult:
        body: Dict[str, Any] = {}
        if p.from_world_id:
            body["world_id"] = p.from_world_id
        if p.to_world_id:
            body["target_world_id"] = p.to_world_id
        data = await self._post("/federation/sever-ties", json=body)
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _declare_war(self, p: DiplomacyParams) -> ToolResult:
        body: Dict[str, Any] = {}
        if p.aggressor_world_id:
            body["aggressor_world_id"] = p.aggressor_world_id
        if p.defender_world_id:
            body["defender_world_id"] = p.defender_world_id
        if p.reason:
            body["reason"] = p.reason
        data = await self._post("/federation/declare-war", json=body)
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _propose_peace(self, p: DiplomacyParams) -> ToolResult:
        body: Dict[str, Any] = {}
        if p.from_world_id:
            body["proposer_world_id"] = p.from_world_id
        if p.to_world_id:
            body["target_world_id"] = p.to_world_id
        if p.peace_terms:
            body["terms"] = p.peace_terms
        data = await self._post("/federation/propose-peace", json=body)
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _accept_peace(self, p: DiplomacyParams) -> ToolResult:
        data = await self._post(f"/federation/accept-peace/{p.peace_id}", json={})
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _summary(self, p: DiplomacyParams) -> ToolResult:
        data = await self._get("/federation/summary")
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)
