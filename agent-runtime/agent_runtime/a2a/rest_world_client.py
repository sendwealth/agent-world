"""REST-based World Client.

Routes each method to the correct World Engine REST endpoint:

- gather / move / explore / build / claim_task / submit_task
  → ``POST /api/v1/agents/{agent_id}/action``  (unified action endpoint)
- propose_deal  → ``submit_action("trade", ...)``
- teach_skill   → ``submit_action("communicate", ...)``
- send_message  → ``POST /api/v1/messages``
- form_org      → ``POST /api/v1/orgs``
- join_org      → ``POST /api/v1/orgs/{org_id}/join``
- broadcast     → standalone (no World Engine endpoint)
"""

from __future__ import annotations

import json
import logging
from typing import Any

logger = logging.getLogger(__name__)


class RESTWorldClient:
    """REST-based World Client using httpx.AsyncClient."""

    def __init__(self, base_url: str, agent_id: str) -> None:
        self._base_url = base_url.rstrip("/")
        self._agent_id = agent_id

    async def _request(self, method: str, path: str, **kwargs: Any) -> dict[str, Any]:
        """Send an HTTP request to the World Engine.

        All errors — including connection failures — are raised so the
        ``ActionExecutor`` retry logic and ThinkLoop error tracking can
        function correctly.  The agent cannot silently "succeed" while
        the World Engine is unreachable.

        If the user wants the agent to run without a World Engine, they
        should not provide ``--world-url``; the ``_NoOpWorldClient``
        (``world_client=None``) handles that case explicitly.
        """
        import httpx

        url = f"{self._base_url}{path}"
        try:
            async with httpx.AsyncClient(timeout=5.0, trust_env=False) as client:
                resp = await client.request(method, url, **kwargs)
                resp.raise_for_status()
                return resp.json()
        except httpx.ConnectError:
            logger.warning("World Engine unreachable at %s", url)
            raise
        except httpx.HTTPStatusError as exc:
            logger.warning(
                "World Engine returned %d for %s %s: %s",
                exc.response.status_code,
                method,
                path,
                exc.response.text[:200] if exc.response.text else "(empty)",
            )
            raise
        except Exception:
            logger.warning("World Engine request failed: %s %s", method, path, exc_info=True)
            raise

    async def submit_action(self, action: str, params: dict[str, Any]) -> dict[str, Any]:
        """Submit an action via the unified ``POST /api/v1/agents/{id}/action``."""
        return await self._request(
            "POST",
            f"/api/v1/agents/{self._agent_id}/action",
            json={"action": action, "params": params},
        )

    @staticmethod
    def _stringify_payload(body: dict[str, Any]) -> dict[str, Any]:
        """Ensure the ``payload`` field is a JSON string, as the World Engine expects.

        World Engine's ``SendMessageRequest.payload`` is typed as ``String``
        (``api_world.rs``), so sending a JSON object/map here results in a 422
        deserialization error.  ``act.py:_handle_send_message`` already does
        this conversion; this helper applies the same rule at the REST boundary
        so every caller (including survival ``broadcast_message``) is safe.
        """
        inner = body.get("payload")
        if isinstance(inner, (dict, list)):
            body = {**body, "payload": json.dumps(inner, separators=(",", ":"))}
        return body

    async def send_message(self, payload: dict[str, Any]) -> dict[str, Any]:
        return await self._request(
            "POST", "/api/v1/messages", json=self._stringify_payload(payload)
        )

    async def claim_task(self, task_id: str) -> dict[str, Any]:
        return await self.submit_action("claim_task", {"task_id": task_id})

    async def submit_task(self, task_id: str, result: dict[str, Any]) -> dict[str, Any]:
        return await self.submit_action("submit_task", {"task_id": task_id, "result": result})

    async def propose_deal(self, proposal: dict[str, Any]) -> dict[str, Any]:
        return await self.submit_action("trade", proposal)

    async def teach_skill(
        self, target_agent_id: str, skill_name: str, level: int
    ) -> dict[str, Any]:
        return await self.submit_action(
            "communicate",
            {
                "target_agent_id": target_agent_id,
                "skill_name": skill_name,
                "level": level,
            },
        )

    async def explore(self, parameters: dict[str, Any]) -> dict[str, Any]:
        return await self.submit_action("explore", parameters)

    async def socialize(self, target_agent_id: str, message: str = "") -> dict[str, Any]:
        return await self.submit_action(
            "socialize",
            {
                "target_agent_id": target_agent_id,
                "message": message,
            },
        )

    async def move(self, direction: str) -> dict[str, Any]:
        return await self.submit_action("move", {"direction": direction})

    async def gather(self, resource_type: str) -> dict[str, Any]:
        return await self.submit_action("gather", {"resource_type": resource_type})

    async def build(self, structure_type: str, **kwargs: Any) -> dict[str, Any]:
        return await self.submit_action(
            "build",
            {"structure_type": structure_type, **kwargs},
        )

    async def get_perception(self) -> dict[str, Any]:
        """Fetch perception data from the World Engine.

        Uses ``GET /api/v1/agents/{id}/perception`` which returns
        nearby agents, resources, position, and the current world tick.
        """
        return await self._request(
            "GET",
            f"/api/v1/agents/{self._agent_id}/perception",
        )

    async def get_status(self) -> dict[str, Any]:
        """Fetch the agent's status from the World Engine.

        Uses ``GET /api/v1/agents/{id}/status`` which returns
        alive, phase, tokens, money, position, etc.
        """
        return await self._request(
            "GET",
            f"/api/v1/agents/{self._agent_id}/status",
        )

    async def broadcast_message(self, payload: dict[str, object]) -> dict[str, object]:
        """Broadcast a message to all agents via the World Engine REST API.

        Posts to ``POST /api/v1/messages`` with an empty ``to_agent`` field
        to indicate a broadcast.  Falls back to a no-op log if the endpoint
        is unavailable, so emergency actions don't crash the think loop.
        """
        try:
            inner_payload = payload.get("payload", {})
            if isinstance(inner_payload, (dict, list)):
                inner_payload = json.dumps(inner_payload, separators=(",", ":"))
            return await self._request(
                "POST",
                "/api/v1/messages",
                json={
                    "from_agent": self._agent_id,
                    "to_agent": "",  # empty = broadcast
                    "message_type": payload.get("type", "INFORM"),
                    "payload": inner_payload,
                },
            )
        except Exception:
            logger.debug("broadcast_message: POST /api/v1/messages failed (non-fatal)")
            return {"status": "no_endpoint", "broadcast": False}

    async def form_org(self, org_data: dict[str, Any]) -> dict[str, Any]:
        return await self._request("POST", "/api/v1/orgs", json=org_data)

    async def join_org(self, org_id: str, member_data: dict[str, Any]) -> dict[str, Any]:
        return await self._request(
            "POST",
            f"/api/v1/orgs/{org_id}/join",
            json=member_data,
        )

    async def propose_rule(
        self, org_id: str, rule_data: dict[str, Any]
    ) -> dict[str, Any]:
        return await self._request(
            "POST",
            f"/api/v1/legislation/cycles/{org_id}/rules",
            json=rule_data,
        )

    async def vote_rule(
        self, rule_id: str, vote_data: dict[str, Any]
    ) -> dict[str, Any]:
        return await self._request(
            "POST",
            f"/api/v1/rules/dsl/rules/{rule_id}/vote",
            json=vote_data,
        )

    async def practice_skill(self, skill_name: str) -> dict[str, Any]:
        return await self.submit_action("practice_skill", {"skill_name": skill_name})

    async def respond_to_oracle(self, oracle_id: str, response: str) -> dict[str, Any]:
        return await self.submit_action(
            "respond_oracle", {"oracle_id": oracle_id, "response": response}
        )

    async def check_bounties(self) -> dict[str, Any]:
        return await self.submit_action("check_bounties", {})

    async def claim_bounty(self, bounty_id: str) -> dict[str, Any]:
        return await self.submit_action("accept_bounty", {"bounty_id": bounty_id})

    async def complete_bounty(self, bounty_id: str, result: str) -> dict[str, Any]:
        return await self.submit_action(
            "complete_bounty", {"bounty_id": bounty_id, "result": result}
        )
