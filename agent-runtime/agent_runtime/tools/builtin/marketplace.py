"""Built-in tool: Marketplace.

Allows agents to publish, search, purchase, rate knowledge listings in
the marketplace, and manage token balances.
"""

from __future__ import annotations

from typing import Any

from ..base import ToolParameters, ToolResult, ToolStatus
from .world_engine_base import WorldEngineTool


class MarketplaceParams(ToolParameters):
    """Parameters for the marketplace tool."""

    action: str
    listing_id: str | None = None
    agent_id: str | None = None
    # Listing
    title: str | None = None
    description: str | None = None
    listing_type: str | None = None
    price: float | None = None
    tags: list[str] | None = None
    content_ref: str | None = None
    # Search
    query: str | None = None
    category: str | None = None
    min_price: float | None = None
    max_price: float | None = None
    # Rating
    rating: int | None = None
    review: str | None = None
    # Transfer
    to_agent_id: str | None = None
    transfer_amount: float | None = None
    # Balance
    balance: float | None = None


class MarketplaceTool(WorldEngineTool):
    """Interact with the knowledge marketplace subsystem.

    Actions:
    - publish: Publish a knowledge listing.
    - search: Search/list knowledge listings.
    - get_listing: Get a single listing.
    - update_listing: Update a listing.
    - delist: Delist a listing.
    - purchase: Purchase a listing.
    - rate: Rate a purchased listing.
    - list_ratings: List ratings for a listing.
    - get_balance: Get agent balance.
    - set_balance: Set agent balance (admin).
    - transfer: Transfer tokens between agents.
    """

    @property
    def name(self) -> str:
        return "marketplace"

    @property
    def description(self) -> str:
        return "Manage marketplace: publish, search, purchase knowledge listings, manage tokens"

    @property
    def category(self) -> str:
        return "economy"

    @property
    def timeout(self) -> float:
        return 15.0

    @property
    def parameters_schema(self) -> type[ToolParameters]:
        return MarketplaceParams

    @property
    def _valid_actions(self) -> set[str]:
        return {
            "publish", "search", "get_listing", "update_listing",
            "delist", "purchase", "rate", "list_ratings",
            "get_balance", "set_balance", "transfer",
        }

    async def execute_async(self, params: ToolParameters) -> ToolResult:
        assert isinstance(params, MarketplaceParams)

        action = params.action
        param_dict = params.model_dump(exclude_none=True, exclude={"action"})

        validation = self._validate_action(action)
        if validation is not None:
            return validation

        if self._sandbox:
            return self._sandbox_response(action, param_dict)

        try:
            handlers = {
                "publish": self._publish,
                "search": self._search,
                "get_listing": self._get_listing,
                "update_listing": self._update_listing,
                "delist": self._delist,
                "purchase": self._purchase,
                "rate": self._rate,
                "list_ratings": self._list_ratings,
                "get_balance": self._get_balance,
                "set_balance": self._set_balance,
                "transfer": self._transfer,
            }
            handler = handlers.get(action)
            if handler is None:
                return self._make_error_result(f"Unknown marketplace action: {action}")
            return await handler(params)
        except Exception as exc:
            return self._make_error_result(str(exc))

    async def _publish(self, p: MarketplaceParams) -> ToolResult:
        body: dict[str, Any] = {}
        if p.title:
            body["title"] = p.title
        if p.description:
            body["description"] = p.description
        if p.listing_type:
            body["listing_type"] = p.listing_type
        if p.price is not None:
            body["price"] = p.price
        if p.tags:
            body["tags"] = p.tags
        if p.content_ref:
            body["content_ref"] = p.content_ref
        if p.agent_id:
            body["seller_id"] = p.agent_id
        data = await self._post("/marketplace/listings", json=body)
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _search(self, p: MarketplaceParams) -> ToolResult:
        query: dict[str, Any] = {}
        if p.query:
            query["q"] = p.query
        if p.category:
            query["category"] = p.category
        if p.listing_type:
            query["type"] = p.listing_type
        if p.min_price is not None:
            query["min_price"] = p.min_price
        if p.max_price is not None:
            query["max_price"] = p.max_price
        data = await self._get("/marketplace/listings", params=query)
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _get_listing(self, p: MarketplaceParams) -> ToolResult:
        data = await self._get(f"/marketplace/listings/{p.listing_id}")
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _update_listing(self, p: MarketplaceParams) -> ToolResult:
        body: dict[str, Any] = {}
        if p.title:
            body["title"] = p.title
        if p.description:
            body["description"] = p.description
        if p.price is not None:
            body["price"] = p.price
        if p.tags:
            body["tags"] = p.tags
        data = await self._put(f"/marketplace/listings/{p.listing_id}", json=body)
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _delist(self, p: MarketplaceParams) -> ToolResult:
        data = await self._post(f"/marketplace/listings/{p.listing_id}/delist", json={})
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _purchase(self, p: MarketplaceParams) -> ToolResult:
        body: dict[str, Any] = {}
        if p.agent_id:
            body["buyer_id"] = p.agent_id
        data = await self._post(f"/marketplace/listings/{p.listing_id}/purchase", json=body)
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _rate(self, p: MarketplaceParams) -> ToolResult:
        body: dict[str, Any] = {}
        if p.rating is not None:
            body["rating"] = p.rating
        if p.review:
            body["review"] = p.review
        if p.agent_id:
            body["rater_id"] = p.agent_id
        data = await self._post(f"/marketplace/listings/{p.listing_id}/rate", json=body)
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _list_ratings(self, p: MarketplaceParams) -> ToolResult:
        data = await self._get(f"/marketplace/listings/{p.listing_id}/ratings")
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _get_balance(self, p: MarketplaceParams) -> ToolResult:
        data = await self._get(f"/marketplace/balance/{p.agent_id}")
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _set_balance(self, p: MarketplaceParams) -> ToolResult:
        body: dict[str, Any] = {}
        if p.agent_id:
            body["agent_id"] = p.agent_id
        if p.balance is not None:
            body["balance"] = p.balance
        data = await self._post("/marketplace/balance", json=body)
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)

    async def _transfer(self, p: MarketplaceParams) -> ToolResult:
        body: dict[str, Any] = {}
        if p.agent_id:
            body["from_agent_id"] = p.agent_id
        if p.to_agent_id:
            body["to_agent_id"] = p.to_agent_id
        if p.transfer_amount is not None:
            body["amount"] = p.transfer_amount
        data = await self._post("/marketplace/transfer", json=body)
        return ToolResult(tool_name=self.name, status=ToolStatus.SUCCESS, output=data)
