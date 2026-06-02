"""Built-in skill: Tool Marketplace.

The tool_marketplace skill enables an agent to interact with the world-engine's
Tool Marketplace API — list tools, browse/search, purchase, rent, rate, and
manage their own tool listings. Higher levels unlock advanced marketplace
operations (renting, bulk operations, analytics).
"""

from __future__ import annotations

import logging
from dataclasses import dataclass
from enum import Enum
from typing import Any, Dict, List, Optional

import httpx

from ..models.skill import Skill
from .registry import SkillDefinition

logger = logging.getLogger(__name__)

# ---------------------------------------------------------------------------
# Marketplace API client
# ---------------------------------------------------------------------------


class ToolCategory(str, Enum):
    computation = "computation"
    communication = "communication"
    analysis = "analysis"
    storage = "storage"
    automation = "automation"
    defense = "defense"
    production = "production"
    utility = "utility"


class ListingMode(str, Enum):
    sale = "sale"
    rent = "rent"
    both = "both"


class ToolMarketplaceClient:
    """Async client for the world-engine Tool Marketplace REST API.

    Usage::

        client = ToolMarketplaceClient("http://localhost:3000")
        tools = await client.search_tools(category="analysis")
        record = await client.purchase_tool(tool_id, "agent-1")
    """

    def __init__(self, base_url: str, timeout: float = 10.0) -> None:
        self._base_url = base_url.rstrip("/")
        self._timeout = timeout

    def _url(self, path: str) -> str:
        return f"{self._base_url}{path}"

    async def _get(self, path: str, params: Optional[Dict[str, Any]] = None) -> Dict[str, Any]:
        async with httpx.AsyncClient(timeout=self._timeout) as client:
            resp = await client.get(self._url(path), params=params)
            resp.raise_for_status()
            return resp.json()

    async def _post(self, path: str, json: Optional[Dict[str, Any]] = None) -> Dict[str, Any]:
        async with httpx.AsyncClient(timeout=self._timeout) as client:
            resp = await client.post(self._url(path), json=json)
            resp.raise_for_status()
            return resp.json()

    async def _put(self, path: str, json: Optional[Dict[str, Any]] = None) -> Dict[str, Any]:
        async with httpx.AsyncClient(timeout=self._timeout) as client:
            resp = await client.put(self._url(path), json=json)
            resp.raise_for_status()
            return resp.json()

    # -- Tool CRUD --

    async def list_tool(
        self,
        name: str,
        description: str,
        category: str,
        owner_id: str,
        purchase_price: int,
        rental_price_per_tick: int,
        currency: str,
        listing_mode: str,
        tags: Optional[List[str]] = None,
        created_tick: int = 0,
    ) -> Dict[str, Any]:
        payload = {
            "name": name,
            "description": description,
            "category": category,
            "owner_id": owner_id,
            "purchase_price": purchase_price,
            "rental_price_per_tick": rental_price_per_tick,
            "currency": currency,
            "listing_mode": listing_mode,
            "tags": tags or [],
            "created_tick": created_tick,
        }
        return await self._post("/api/v1/tool-marketplace/tools", json=payload)

    async def search_tools(self, **params: Any) -> List[Dict[str, Any]]:
        result = await self._get("/api/v1/tool-marketplace/tools", params=params)
        return result.get("data", []) if isinstance(result, dict) else result

    async def get_tool(self, tool_id: str) -> Dict[str, Any]:
        return await self._get(f"/api/v1/tool-marketplace/tools/{tool_id}")

    async def update_tool(
        self,
        tool_id: str,
        owner_id: str,
        purchase_price: Optional[int] = None,
        rental_price_per_tick: Optional[int] = None,
        status: Optional[str] = None,
        tags: Optional[List[str]] = None,
    ) -> Dict[str, Any]:
        payload: Dict[str, Any] = {"owner_id": owner_id}
        if purchase_price is not None:
            payload["purchase_price"] = purchase_price
        if rental_price_per_tick is not None:
            payload["rental_price_per_tick"] = rental_price_per_tick
        if status is not None:
            payload["status"] = status
        if tags is not None:
            payload["tags"] = tags
        return await self._put(f"/api/v1/tool-marketplace/tools/{tool_id}", json=payload)

    async def delist_tool(self, tool_id: str, owner_id: str) -> Dict[str, Any]:
        return await self._post(
            f"/api/v1/tool-marketplace/tools/{tool_id}/delist",
            json={"owner_id": owner_id},
        )

    # -- Purchase / Rent --

    async def purchase_tool(self, tool_id: str, buyer_id: str, tick: int = 0) -> Dict[str, Any]:
        return await self._post(
            f"/api/v1/tool-marketplace/tools/{tool_id}/purchase",
            json={"buyer_id": buyer_id, "tick": tick},
        )

    async def rent_tool(
        self,
        tool_id: str,
        renter_id: str,
        duration_ticks: int,
        current_tick: int = 0,
    ) -> Dict[str, Any]:
        return await self._post(
            f"/api/v1/tool-marketplace/tools/{tool_id}/rent",
            json={
                "renter_id": renter_id,
                "duration_ticks": duration_ticks,
                "current_tick": current_tick,
            },
        )

    async def cancel_rental(self, rental_id: str, renter_id: str) -> Dict[str, Any]:
        return await self._post(
            f"/api/v1/tool-marketplace/rentals/{rental_id}/cancel",
            json={"renter_id": renter_id},
        )

    async def active_rentals(self, agent_id: str) -> List[Dict[str, Any]]:
        result = await self._get(f"/api/v1/tool-marketplace/rentals/active/{agent_id}")
        return result.get("data", []) if isinstance(result, dict) else result

    # -- Ratings --

    async def rate_tool(
        self,
        tool_id: str,
        rater_id: str,
        score: int,
        review: Optional[str] = None,
        tick: int = 0,
    ) -> Dict[str, Any]:
        payload: Dict[str, Any] = {"rater_id": rater_id, "score": score, "tick": tick}
        if review is not None:
            payload["review"] = review
        return await self._post(
            f"/api/v1/tool-marketplace/tools/{tool_id}/rate",
            json=payload,
        )

    async def tool_ratings(self, tool_id: str) -> List[Dict[str, Any]]:
        result = await self._get(f"/api/v1/tool-marketplace/tools/{tool_id}/ratings")
        return result.get("data", []) if isinstance(result, dict) else result

    # -- Ownership --

    async def check_ownership(self, tool_id: str, agent_id: str) -> Dict[str, Any]:
        return await self._get(f"/api/v1/tool-marketplace/tools/{tool_id}/ownership/{agent_id}")

    # -- Balance --

    async def get_balance(self, agent_id: str) -> int:
        result = await self._get(f"/api/v1/tool-marketplace/balance/{agent_id}")
        data = result.get("data", result)
        return data.get("balance", 0)

    async def set_balance(self, agent_id: str, amount: int) -> Dict[str, Any]:
        return await self._post(
            "/api/v1/tool-marketplace/balance",
            json={"agent_id": agent_id, "amount": amount},
        )

    # -- Purchases --

    async def tool_purchases(self, tool_id: str) -> List[Dict[str, Any]]:
        result = await self._get(f"/api/v1/tool-marketplace/tools/{tool_id}/purchases")
        return result.get("data", []) if isinstance(result, dict) else result


# ---------------------------------------------------------------------------
# Skill execute function
# ---------------------------------------------------------------------------


def _execute_tool_marketplace(agent_skills: Dict[str, Skill], **kwargs: Any) -> Dict[str, Any]:
    """Execute a tool marketplace operation (synchronous wrapper).

    This is the execute_fn registered in the SkillDefinition.
    The actual HTTP calls should be made via ToolMarketplaceClient in
    async agent loops. This synchronous stub validates the action and
    returns a description of what would happen.

    Kwargs:
        action: One of "list", "search", "purchase", "rent", "rate",
                "delist", "update", "cancel_rental".
        tool_name: Name for listing (for "list" action).
        category: Tool category (for "list" action).
        tool_id: UUID of the tool (for most actions).
        price: Purchase price (for "list" action).
        rental_price: Rental price per tick (for "list" action).
        score: Rating score 1-5 (for "rate" action).
        duration_ticks: Rental duration (for "rent" action).

    Returns:
        Dict with success status, action details, and level_used.
    """
    tm_skill = agent_skills.get("tool_marketplace")
    level = tm_skill.level if tm_skill else 0

    action = kwargs.get("action", "search")
    available_actions = {"search", "list", "purchase"}
    if level >= 2:
        available_actions.add("rent")
    if level >= 3:
        available_actions.add("rate")
    if level >= 4:
        available_actions.add("update")
        available_actions.add("cancel_rental")
    if level >= 5:
        available_actions.add("delist")

    if action not in available_actions:
        return {
            "skill": "tool_marketplace",
            "action": action,
            "success": False,
            "error": f"action '{action}' requires tool_marketplace level {({'rent': 2, 'rate': 3, 'update': 4, 'cancel_rental': 4, 'delist': 5}.get(action, 1))}",
            "level_used": level,
        }

    return {
        "skill": "tool_marketplace",
        "action": action,
        "success": True,
        "level_used": level,
        "kwargs": {k: v for k, v in kwargs.items() if k != "action"},
    }


TOOL_MARKETPLACE_SKILL = SkillDefinition(
    name="tool_marketplace",
    description="Ability to list, browse, purchase, rent, and rate tools on the Tool Marketplace",
    max_level=10,
    execute_fn=_execute_tool_marketplace,
    category="economic",
)
