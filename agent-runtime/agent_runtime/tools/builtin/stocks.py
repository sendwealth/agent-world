"""Built-in tool: Stocks.

Allows agents to manage stock listings, IPOs, buy/sell orders, dividends.
"""

from __future__ import annotations

from typing import Any, Dict, Optional

from ..base import ToolParameters, ToolResult, ToolStatus
from .world_engine_base import WorldEngineTool


class StocksParams(ToolParameters):
    """Parameters for the stocks tool."""

    action: str
    stock_id: Optional[str] = None
    order_id: Optional[str] = None
    org_id: Optional[str] = None
    agent_id: Optional[str] = None
    # Stock listing
    symbol: Optional[str] = None
    total_shares: Optional[int] = None
    initial_price: Optional[float] = None
    # Orders
    order_type: Optional[str] = None  # market, limit
    quantity: Optional[int] = None
    price: Optional[float] = None
    side: Optional[str] = None  # buy, sell
    # Dividends
    dividend_per_share: Optional[float] = None


class StocksTool(WorldEngineTool):
    """Interact with the stock market subsystem.

    Actions:
    - list_stocks: List all stock listings.
    - issue_shares: Issue new shares.
    - get_stock: Get stock listing details.
    - perform_ipo: Perform IPO.
    - distribute_dividend: Distribute dividend.
    - list_orders: List stock orders.
    - buy_order: Place a buy order.
    - sell_order: Place a sell order.
    - get_order: Get order details.
    - cancel_order: Cancel an order.
    """

    @property
    def name(self) -> str:
        return "stocks"

    @property
    def description(self) -> str:
        return "Manage stocks: listings, IPOs, buy/sell orders, dividends"

    @property
    def category(self) -> str:
        return "economy"

    @property
    def timeout(self) -> float:
        return 15.0

    @property
    def parameters_schema(self) -> type[ToolParameters]:
        return StocksParams

    @property
    def _valid_actions(self) -> set[str]:
        return {
            "list_stocks", "issue_shares", "get_stock", "perform_ipo",
            "distribute_dividend", "list_orders", "buy_order",
            "sell_order", "get_order", "cancel_order",
        }

    async def execute_async(self, params: ToolParameters) -> ToolResult:
        assert isinstance(params, StocksParams)

        action = params.action
        param_dict = params.model_dump(exclude_none=True, exclude={"action"})

        validation = self._validate_action(action)
        if validation is not None:
            return validation

        if self._sandbox:
            return self._sandbox_response(action, param_dict)

        try:
            handlers = {
                "list_stocks": self._list_stocks,
                "issue_shares": self._issue_shares,
                "get_stock": self._get_stock,
                "perform_ipo": self._perform_ipo,
                "distribute_dividend": self._distribute_dividend,
                "list_orders": self._list_orders,
                "buy_order": self._buy_order,
                "sell_order": self._sell_order,
                "get_order": self._get_order,
                "cancel_order": self._cancel_order,
            }
            handler = handlers.get(action)
            if handler is None:
                return self._make_error_result(f"Unknown stocks action: {action}")
            return await handler(params)
        except Exception as exc:
            return self._make_error_result(str(exc))

    async def _list_stocks(self, p: StocksParams) -> ToolResult:
        data = await self._get("/stocks")
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _issue_shares(self, p: StocksParams) -> ToolResult:
        body: Dict[str, Any] = {}
        if p.org_id:
            body["org_id"] = p.org_id
        if p.symbol:
            body["symbol"] = p.symbol
        if p.total_shares is not None:
            body["total_shares"] = p.total_shares
        if p.initial_price is not None:
            body["initial_price"] = p.initial_price
        data = await self._post("/stocks", json=body)
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _get_stock(self, p: StocksParams) -> ToolResult:
        data = await self._get(f"/stocks/{p.stock_id}")
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _perform_ipo(self, p: StocksParams) -> ToolResult:
        data = await self._post(f"/stocks/{p.stock_id}/ipo", json={})
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _distribute_dividend(self, p: StocksParams) -> ToolResult:
        body: Dict[str, Any] = {}
        if p.dividend_per_share is not None:
            body["dividend_per_share"] = p.dividend_per_share
        data = await self._post(f"/stocks/{p.stock_id}/dividend", json=body)
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _list_orders(self, p: StocksParams) -> ToolResult:
        data = await self._get("/orders")
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _buy_order(self, p: StocksParams) -> ToolResult:
        body: Dict[str, Any] = {}
        if p.stock_id:
            body["stock_id"] = p.stock_id
        if p.agent_id:
            body["agent_id"] = p.agent_id
        if p.quantity is not None:
            body["quantity"] = p.quantity
        if p.order_type:
            body["order_type"] = p.order_type
        if p.price is not None:
            body["price"] = p.price
        data = await self._post("/orders/buy", json=body)
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _sell_order(self, p: StocksParams) -> ToolResult:
        body: Dict[str, Any] = {}
        if p.stock_id:
            body["stock_id"] = p.stock_id
        if p.agent_id:
            body["agent_id"] = p.agent_id
        if p.quantity is not None:
            body["quantity"] = p.quantity
        if p.order_type:
            body["order_type"] = p.order_type
        if p.price is not None:
            body["price"] = p.price
        data = await self._post("/orders/sell", json=body)
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _get_order(self, p: StocksParams) -> ToolResult:
        data = await self._get(f"/orders/{p.order_id}")
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _cancel_order(self, p: StocksParams) -> ToolResult:
        data = await self._post(f"/orders/{p.order_id}/cancel", json={})
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)
