"""Built-in tool: Investment.

Allows agents to create investment products, buy/sell shares, manage portfolios,
and query investment metrics.
"""

from __future__ import annotations

from typing import Any

from ..base import ToolParameters, ToolResult, ToolStatus
from .world_engine_base import WorldEngineTool


class InvestmentParams(ToolParameters):
    """Parameters for the investment tool."""

    action: str
    product_id: str | None = None
    product_name: str | None = None
    product_type: str | None = None
    description: str | None = None
    min_investment: float | None = None
    max_investment: float | None = None
    expected_return: float | None = None
    risk_level: str | None = None
    duration_ticks: int | None = None
    investor_id: str | None = None
    amount: float | None = None
    shares: float | None = None
    performance_score: float | None = None
    status: str | None = None


class InvestmentTool(WorldEngineTool):
    """Interact with the investment subsystem.

    Actions:
    - create_product: Create a new investment product.
    - list_products: List investment products.
    - get_product: Get product details.
    - buy: Buy investment shares.
    - sell: Sell investment shares.
    - portfolio: Get investor portfolio.
    - leaderboard: Get investment leaderboard.
    - close_product: Close an investment product.
    - distribute_returns: Distribute returns for a product.
    - update_performance: Update performance score for a product.
    - freeze_product: Freeze an investment product.
    - list_transactions: List investment transactions.
    - list_dividends: List dividends.
    """

    @property
    def name(self) -> str:
        return "investment"

    @property
    def description(self) -> str:
        return "Manage investments: create products, buy/sell shares, portfolio, leaderboard"

    @property
    def category(self) -> str:
        return "economy"

    @property
    def timeout(self) -> float:
        return 15.0

    @property
    def parameters_schema(self) -> type[ToolParameters]:
        return InvestmentParams

    @property
    def _valid_actions(self) -> set[str]:
        return {
            "create_product", "list_products", "get_product",
            "buy", "sell", "portfolio", "leaderboard",
            "close_product", "distribute_returns", "update_performance",
            "freeze_product", "list_transactions", "list_dividends",
        }

    async def execute_async(self, params: ToolParameters) -> ToolResult:
        assert isinstance(params, InvestmentParams)

        action = params.action
        param_dict = params.model_dump(exclude_none=True, exclude={"action"})

        validation = self._validate_action(action)
        if validation is not None:
            return validation

        if self._sandbox:
            return self._sandbox_response(action, param_dict)

        try:
            handlers = {
                "create_product": self._create_product,
                "list_products": self._list_products,
                "get_product": self._get_product,
                "buy": self._buy,
                "sell": self._sell,
                "portfolio": self._portfolio,
                "leaderboard": self._leaderboard,
                "close_product": self._close_product,
                "distribute_returns": self._distribute_returns,
                "update_performance": self._update_performance,
                "freeze_product": self._freeze_product,
                "list_transactions": self._list_transactions,
                "list_dividends": self._list_dividends,
            }
            handler = handlers.get(action)
            if handler is None:
                return self._make_error_result(f"Unknown investment action: {action}")
            return await handler(params)
        except Exception as exc:
            return self._make_error_result(str(exc))

    async def _create_product(self, p: InvestmentParams) -> ToolResult:
        body: dict[str, Any] = {}
        if p.product_name:
            body["name"] = p.product_name
        if p.product_type:
            body["product_type"] = p.product_type
        if p.description:
            body["description"] = p.description
        if p.min_investment is not None:
            body["min_investment"] = p.min_investment
        if p.max_investment is not None:
            body["max_investment"] = p.max_investment
        if p.expected_return is not None:
            body["expected_return"] = p.expected_return
        if p.risk_level:
            body["risk_level"] = p.risk_level
        if p.duration_ticks is not None:
            body["duration_ticks"] = p.duration_ticks
        data = await self._post("/investments/products", json=body)
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _list_products(self, p: InvestmentParams) -> ToolResult:
        data = await self._get("/investments/products")
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _get_product(self, p: InvestmentParams) -> ToolResult:
        data = await self._get(f"/investments/products/{p.product_id}")
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _buy(self, p: InvestmentParams) -> ToolResult:
        body: dict[str, Any] = {}
        if p.product_id:
            body["product_id"] = p.product_id
        if p.investor_id:
            body["investor_id"] = p.investor_id
        if p.amount is not None:
            body["amount"] = p.amount
        if p.shares is not None:
            body["shares"] = p.shares
        data = await self._post("/investments/buy", json=body)
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _sell(self, p: InvestmentParams) -> ToolResult:
        body: dict[str, Any] = {}
        if p.product_id:
            body["product_id"] = p.product_id
        if p.investor_id:
            body["investor_id"] = p.investor_id
        if p.shares is not None:
            body["shares"] = p.shares
        data = await self._post("/investments/sell", json=body)
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _portfolio(self, p: InvestmentParams) -> ToolResult:
        data = await self._get(f"/investments/portfolio/{p.investor_id}")
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _leaderboard(self, p: InvestmentParams) -> ToolResult:
        data = await self._get("/investments/leaderboard")
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _close_product(self, p: InvestmentParams) -> ToolResult:
        data = await self._post(f"/investments/products/{p.product_id}/close", json={})
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _distribute_returns(self, p: InvestmentParams) -> ToolResult:
        data = await self._post(f"/investments/products/{p.product_id}/distribute-returns", json={})
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _update_performance(self, p: InvestmentParams) -> ToolResult:
        body: dict[str, Any] = {}
        if p.performance_score is not None:
            body["performance_score"] = p.performance_score
        data = await self._post(f"/investments/products/{p.product_id}/performance", json=body)
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _freeze_product(self, p: InvestmentParams) -> ToolResult:
        data = await self._post(f"/investments/products/{p.product_id}/freeze", json={})
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _list_transactions(self, p: InvestmentParams) -> ToolResult:
        data = await self._get("/investments/transactions")
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _list_dividends(self, p: InvestmentParams) -> ToolResult:
        data = await self._get("/investments/dividends")
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)
