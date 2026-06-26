"""Built-in tool: Escrow.

Allows agents to create, claim, complete, dispute, and resolve escrow
contracts for secure transactions.
"""

from __future__ import annotations

from typing import Any

from ..base import ToolParameters, ToolResult, ToolStatus
from .world_engine_base import WorldEngineTool


class EscrowParams(ToolParameters):
    """Parameters for the escrow tool."""

    action: str
    escrow_id: str | None = None
    agent_id: str | None = None
    # Create
    counterparty_id: str | None = None
    amount: float | None = None
    description: str | None = None
    timeout_ticks: int | None = None
    # Dispute/resolve
    reason: str | None = None
    resolution: str | None = None
    refund_amount: float | None = None
    # Admin
    balance: float | None = None


class EscrowTool(WorldEngineTool):
    """Interact with the escrow subsystem.

    Actions:
    - create: Create an escrow.
    - list: List escrows.
    - get: Get escrow details.
    - claim: Claim escrow.
    - complete: Complete escrow.
    - refund: Refund escrow.
    - dispute: Open a dispute.
    - resolve: Resolve a dispute.
    - set_balance: Set agent balance (admin).
    """

    @property
    def name(self) -> str:
        return "escrow"

    @property
    def description(self) -> str:
        return "Manage escrow contracts: create, claim, complete, dispute, resolve"

    @property
    def category(self) -> str:
        return "economy"

    @property
    def timeout(self) -> float:
        return 15.0

    @property
    def parameters_schema(self) -> type[ToolParameters]:
        return EscrowParams

    @property
    def _valid_actions(self) -> set[str]:
        return {
            "create", "list", "get", "claim", "complete",
            "refund", "dispute", "resolve", "set_balance",
        }

    async def execute_async(self, params: ToolParameters) -> ToolResult:
        assert isinstance(params, EscrowParams)

        action = params.action
        param_dict = params.model_dump(exclude_none=True, exclude={"action"})

        validation = self._validate_action(action)
        if validation is not None:
            return validation

        if self._sandbox:
            return self._sandbox_response(action, param_dict)

        try:
            handlers = {
                "create": self._create,
                "list": self._list,
                "get": self._get_escrow,
                "claim": self._claim,
                "complete": self._complete,
                "refund": self._refund,
                "dispute": self._dispute,
                "resolve": self._resolve,
                "set_balance": self._set_balance,
            }
            handler = handlers.get(action)
            if handler is None:
                return self._make_error_result(f"Unknown escrow action: {action}")
            return await handler(params)
        except Exception as exc:
            return self._make_error_result(str(exc))

    async def _create(self, p: EscrowParams) -> ToolResult:
        body: dict[str, Any] = {}
        if p.agent_id:
            body["creator_id"] = p.agent_id
        if p.counterparty_id:
            body["counterparty_id"] = p.counterparty_id
        if p.amount is not None:
            body["amount"] = p.amount
        if p.description:
            body["description"] = p.description
        if p.timeout_ticks is not None:
            body["timeout_ticks"] = p.timeout_ticks
        data = await self._post("/escrow", json=body)
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _list(self, p: EscrowParams) -> ToolResult:
        data = await self._get("/escrow")
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _get_escrow(self, p: EscrowParams) -> ToolResult:
        data = await self._get(f"/escrow/{p.escrow_id}")
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _claim(self, p: EscrowParams) -> ToolResult:
        body: dict[str, Any] = {}
        if p.agent_id:
            body["claimer_id"] = p.agent_id
        data = await self._post(f"/escrow/{p.escrow_id}/claim", json=body)
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _complete(self, p: EscrowParams) -> ToolResult:
        data = await self._post(f"/escrow/{p.escrow_id}/complete", json={})
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _refund(self, p: EscrowParams) -> ToolResult:
        data = await self._post(f"/escrow/{p.escrow_id}/refund", json={})
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _dispute(self, p: EscrowParams) -> ToolResult:
        body: dict[str, Any] = {}
        if p.agent_id:
            body["disputer_id"] = p.agent_id
        if p.reason:
            body["reason"] = p.reason
        data = await self._post(f"/escrow/{p.escrow_id}/dispute", json=body)
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _resolve(self, p: EscrowParams) -> ToolResult:
        body: dict[str, Any] = {}
        if p.resolution:
            body["resolution"] = p.resolution
        if p.refund_amount is not None:
            body["refund_amount"] = p.refund_amount
        data = await self._post(f"/escrow/{p.escrow_id}/resolve", json=body)
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _set_balance(self, p: EscrowParams) -> ToolResult:
        body: dict[str, Any] = {}
        if p.agent_id:
            body["agent_id"] = p.agent_id
        if p.balance is not None:
            body["balance"] = p.balance
        data = await self._post("/escrow/balance", json=body)
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)
