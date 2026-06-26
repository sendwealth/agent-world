"""Built-in tool: Bank.

Allows agents to manage bank accounts, deposits, withdrawals, loans, and
central bank operations.
"""

from __future__ import annotations

from typing import Any

from ..base import ToolParameters, ToolResult, ToolStatus
from .world_engine_base import WorldEngineTool


class BankParams(ToolParameters):
    """Parameters for the bank tool."""

    action: str
    account_id: str | None = None
    agent_id: str | None = None
    account_type: str | None = None  # savings, checking
    amount: float | None = None
    # Loans
    loan_id: str | None = None
    loan_amount: float | None = None
    loan_purpose: str | None = None
    term_ticks: int | None = None
    # Central bank
    savings_rate: float | None = None
    loan_rate: float | None = None
    mint_amount: float | None = None


class BankTool(WorldEngineTool):
    """Interact with the banking subsystem.

    Actions:
    - open_account: Open a bank account.
    - list_accounts: List all bank accounts.
    - get_account: Get account details.
    - deposit: Deposit funds.
    - withdraw: Withdraw funds.
    - apply_loan: Apply for a loan.
    - list_loans: List loans.
    - get_loan: Get loan details.
    - approve_loan: Approve a loan.
    - disburse_loan: Disburse a loan.
    - repay_loan: Repay a loan.
    - set_rates: Adjust savings/loan interest rates (central bank).
    - mint: Mint new money (central bank).
    - write_off: Write off bad debt (central bank).
    - stats: Get banking system statistics.
    """

    @property
    def name(self) -> str:
        return "bank"

    @property
    def description(self) -> str:
        return "Manage banking: accounts, deposits, withdrawals, loans, central bank operations"

    @property
    def category(self) -> str:
        return "economy"

    @property
    def timeout(self) -> float:
        return 15.0

    @property
    def parameters_schema(self) -> type[ToolParameters]:
        return BankParams

    @property
    def _valid_actions(self) -> set[str]:
        return {
            "open_account", "list_accounts", "get_account",
            "deposit", "withdraw", "apply_loan", "list_loans",
            "get_loan", "approve_loan", "disburse_loan", "repay_loan",
            "set_rates", "mint", "write_off", "stats",
        }

    async def execute_async(self, params: ToolParameters) -> ToolResult:
        assert isinstance(params, BankParams)

        action = params.action
        param_dict = params.model_dump(exclude_none=True, exclude={"action"})

        validation = self._validate_action(action)
        if validation is not None:
            return validation

        if self._sandbox:
            return self._sandbox_response(action, param_dict)

        try:
            handlers = {
                "open_account": self._open_account,
                "list_accounts": self._list_accounts,
                "get_account": self._get_account,
                "deposit": self._deposit,
                "withdraw": self._withdraw,
                "apply_loan": self._apply_loan,
                "list_loans": self._list_loans,
                "get_loan": self._get_loan,
                "approve_loan": self._approve_loan,
                "disburse_loan": self._disburse_loan,
                "repay_loan": self._repay_loan,
                "set_rates": self._set_rates,
                "mint": self._mint,
                "write_off": self._write_off,
                "stats": self._stats,
            }
            handler = handlers.get(action)
            if handler is None:
                return self._make_error_result(f"Unknown bank action: {action}")
            return await handler(params)
        except Exception as exc:
            return self._make_error_result(str(exc))

    async def _open_account(self, p: BankParams) -> ToolResult:
        body: dict[str, Any] = {}
        if p.agent_id:
            body["agent_id"] = p.agent_id
        if p.account_type:
            body["account_type"] = p.account_type
        data = await self._post("/bank/accounts", json=body)
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _list_accounts(self, p: BankParams) -> ToolResult:
        data = await self._get("/bank/accounts")
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _get_account(self, p: BankParams) -> ToolResult:
        data = await self._get(f"/bank/accounts/{p.account_id}")
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _deposit(self, p: BankParams) -> ToolResult:
        body: dict[str, Any] = {}
        if p.account_id:
            body["account_id"] = p.account_id
        if p.amount is not None:
            body["amount"] = p.amount
        data = await self._post("/bank/deposit", json=body)
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _withdraw(self, p: BankParams) -> ToolResult:
        body: dict[str, Any] = {}
        if p.account_id:
            body["account_id"] = p.account_id
        if p.amount is not None:
            body["amount"] = p.amount
        data = await self._post("/bank/withdraw", json=body)
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _apply_loan(self, p: BankParams) -> ToolResult:
        body: dict[str, Any] = {}
        if p.agent_id:
            body["agent_id"] = p.agent_id
        if p.loan_amount is not None:
            body["amount"] = p.loan_amount
        if p.loan_purpose:
            body["purpose"] = p.loan_purpose
        if p.term_ticks is not None:
            body["term_ticks"] = p.term_ticks
        data = await self._post("/bank/loans", json=body)
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _list_loans(self, p: BankParams) -> ToolResult:
        data = await self._get("/bank/loans")
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _get_loan(self, p: BankParams) -> ToolResult:
        data = await self._get(f"/bank/loans/{p.loan_id}")
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _approve_loan(self, p: BankParams) -> ToolResult:
        data = await self._post(f"/bank/loans/{p.loan_id}/approve", json={})
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _disburse_loan(self, p: BankParams) -> ToolResult:
        data = await self._post(f"/bank/loans/{p.loan_id}/disburse", json={})
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _repay_loan(self, p: BankParams) -> ToolResult:
        body: dict[str, Any] = {}
        if p.amount is not None:
            body["amount"] = p.amount
        data = await self._post(f"/bank/loans/{p.loan_id}/repay", json=body)
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _set_rates(self, p: BankParams) -> ToolResult:
        body: dict[str, Any] = {}
        if p.savings_rate is not None:
            body["savings_rate"] = p.savings_rate
        if p.loan_rate is not None:
            body["loan_rate"] = p.loan_rate
        data = await self._post("/bank/central-bank/rates", json=body)
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _mint(self, p: BankParams) -> ToolResult:
        body: dict[str, Any] = {}
        if p.mint_amount is not None:
            body["amount"] = p.mint_amount
        data = await self._post("/bank/central-bank/mint", json=body)
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _write_off(self, p: BankParams) -> ToolResult:
        data = await self._post(f"/bank/central-bank/write-off/{p.loan_id}", json={})
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _stats(self, p: BankParams) -> ToolResult:
        data = await self._get("/bank/stats")
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)
