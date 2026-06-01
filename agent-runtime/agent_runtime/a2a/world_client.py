"""GRPCWorldClient — implements WorldClientProtocol for the ACT phase.

Translates high-level action calls (claim_task, submit_task, etc.) into
A2A protobuf messages and sends them via the low-level A2AClient.

Also satisfies A2AClientProtocol (broadcast_message) so the same client
can be injected into SurvivalInstinct.
"""

from __future__ import annotations

import logging
from typing import Any

from protocol.gen.python import a2a_pb2

from .client import A2AClient

logger = logging.getLogger(__name__)


class GRPCWorldClient:
    """gRPC-backed WorldClient for the ACT phase of the Think Loop.

    Implements both ``WorldClientProtocol`` (from ``core.act``) and
    ``A2AClientProtocol`` (from ``survival.instinct``).

    Errors are raised (not swallowed) so that ``ActionExecutor``'s retry
    logic can function correctly.  Each method logs the error and
    re-raises the exception so the executor records ``RETRY_EXHAUSTED``
    instead of a false success.

    Usage::

        a2a = A2AClient(config)
        await a2a.connect()
        world = GRPCWorldClient(a2a)

        # Via WorldClientProtocol (used by ActionExecutor)
        result = await world.send_message({"text": "hello"})
        result = await world.claim_task("task-123")

        # Via A2AClientProtocol (used by SurvivalInstinct)
        result = await world.broadcast_message({"type": "SOS"})
    """

    def __init__(self, a2a_client: A2AClient) -> None:
        self._client = a2a_client

    # ------------------------------------------------------------------
    # WorldClientProtocol methods (ACT phase)
    # ------------------------------------------------------------------

    async def send_message(self, payload: dict[str, Any]) -> dict[str, Any]:
        """Send a generic A2A message to another agent."""
        to_agent = payload.get("to_agent", "")
        msg_type = _payload_type_to_proto(payload.get("type", "INFORM"))
        try:
            ack = await self._client.send_message(
                to_agent=to_agent,
                message_type=msg_type,
                payload=payload.get("payload", payload),
            )
            return {"status": "ok", "received": ack.received}
        except Exception:
            logger.exception("send_message failed")
            raise

    async def claim_task(self, task_id: str) -> dict[str, Any]:
        """Claim an available task — sent as a PROPOSE message."""
        try:
            ack = await self._client.send_message(
                message_type=a2a_pb2.PROPOSE,
                payload={"action": "claim_task", "task_id": task_id},
            )
            return {"status": "ok", "task_id": task_id, "received": ack.received}
        except Exception:
            logger.exception("claim_task failed")
            raise

    async def submit_task(
        self, task_id: str, result: dict[str, Any]
    ) -> dict[str, Any]:
        """Submit completed task work — sent as an INFORM message."""
        try:
            ack = await self._client.send_message(
                message_type=a2a_pb2.INFORM,
                payload={
                    "action": "submit_task",
                    "task_id": task_id,
                    "result": result,
                },
            )
            return {"status": "ok", "task_id": task_id, "received": ack.received}
        except Exception:
            logger.exception("submit_task failed")
            raise

    async def propose_deal(self, proposal: dict[str, Any]) -> dict[str, Any]:
        """Propose a deal/contract — sent as a PROPOSE message."""
        try:
            to_agent = proposal.get("target_agent_id", "")
            ack = await self._client.send_message(
                to_agent=to_agent,
                message_type=a2a_pb2.PROPOSE,
                payload={"action": "propose_deal", "proposal": proposal},
            )
            return {"status": "ok", "received": ack.received}
        except Exception:
            logger.exception("propose_deal failed")
            raise

    async def teach_skill(
        self, target_agent_id: str, skill_name: str, level: int
    ) -> dict[str, Any]:
        """Teach a skill to another agent — sent as a TEACH message."""
        try:
            ack = await self._client.send_message(
                to_agent=target_agent_id,
                message_type=a2a_pb2.TEACH,
                payload={
                    "action": "teach_skill",
                    "skill_name": skill_name,
                    "level": level,
                },
            )
            return {
                "status": "ok",
                "target": target_agent_id,
                "skill": skill_name,
                "received": ack.received,
            }
        except Exception:
            logger.exception("teach_skill failed")
            raise

    async def explore(self, parameters: dict[str, Any]) -> dict[str, Any]:
        """Explore the world via Discover RPC."""
        try:
            response = await self._client.discover(
                capabilities=parameters.get("capabilities", []),
            )
            agents = [
                {
                    "agent_id": a.agent_id,
                    "name": a.name,
                    "tokens": a.tokens,
                    "money": a.money,
                    "skills": list(a.skills),
                    "reputation": a.reputation,
                    "phase": a.phase,
                }
                for a in response.agents
            ]
            return {"status": "ok", "agents": agents}
        except Exception:
            logger.exception("explore failed")
            raise

    async def move(self, direction: str) -> dict[str, Any]:
        """Move the agent in a direction — sent as a WILL message."""
        try:
            ack = await self._client.send_message(
                message_type=a2a_pb2.WILL,
                payload={"action": "move", "direction": direction},
            )
            return {"status": "ok", "direction": direction, "received": ack.received}
        except Exception:
            logger.exception("move failed")
            raise

    async def gather(self, resource_type: str) -> dict[str, Any]:
        """Gather a resource — sent as a WILL message."""
        try:
            ack = await self._client.send_message(
                message_type=a2a_pb2.WILL,
                payload={"action": "gather", "resource_type": resource_type},
            )
            return {"status": "ok", "resource_type": resource_type, "received": ack.received}
        except Exception:
            logger.exception("gather failed")
            raise

    async def build(self, structure_type: str, **kwargs: Any) -> dict[str, Any]:
        """Build a structure — sent as a WILL message."""
        try:
            ack = await self._client.send_message(
                message_type=a2a_pb2.WILL,
                payload={"action": "build", "structure_type": structure_type, **kwargs},
            )
            return {"status": "ok", "structure_type": structure_type, "received": ack.received}
        except Exception:
            logger.exception("build failed")
            raise

    # ------------------------------------------------------------------
    # Oracle & Bounty methods (Human Participation integration)
    # ------------------------------------------------------------------

    async def respond_to_oracle(self, oracle_id: str, response: str) -> dict[str, Any]:
        """Respond to an Oracle — sent as an INFORM message to World Engine."""
        try:
            ack = await self._client.send_message(
                message_type=a2a_pb2.INFORM,
                payload={
                    "action": "respond_oracle",
                    "oracle_id": oracle_id,
                    "response": response,
                },
            )
            return {"status": "ok", "oracle_id": oracle_id, "received": ack.received}
        except Exception:
            logger.exception("respond_to_oracle failed")
            raise

    async def check_bounties(self) -> dict[str, Any]:
        """Check available bounties — sent as a DISCOVER message."""
        try:
            response = await self._client.discover(capabilities=["bounties"])
            return {
                "status": "ok",
                "bounties": [
                    {"id": a.agent_id, "name": a.name}
                    for a in response.agents
                ],
            }
        except Exception:
            logger.exception("check_bounties failed")
            raise

    async def claim_bounty(self, bounty_id: str) -> dict[str, Any]:
        """Claim a bounty — sent as a PROPOSE message to World Engine."""
        try:
            ack = await self._client.send_message(
                message_type=a2a_pb2.PROPOSE,
                payload={
                    "action": "claim_bounty",
                    "bounty_id": bounty_id,
                },
            )
            return {"status": "ok", "bounty_id": bounty_id, "received": ack.received}
        except Exception:
            logger.exception("claim_bounty failed")
            raise

    async def complete_bounty(self, bounty_id: str, result: str) -> dict[str, Any]:
        """Complete a bounty — sent as an INFORM message to World Engine."""
        try:
            ack = await self._client.send_message(
                message_type=a2a_pb2.INFORM,
                payload={
                    "action": "complete_bounty",
                    "bounty_id": bounty_id,
                    "result": result,
                },
            )
            return {"status": "ok", "bounty_id": bounty_id, "received": ack.received}
        except Exception:
            logger.exception("complete_bounty failed")
            raise

    # ------------------------------------------------------------------
    # A2AClientProtocol method (SurvivalInstinct integration)
    # ------------------------------------------------------------------

    async def broadcast_message(
        self, payload: dict[str, object]
    ) -> dict[str, object]:
        """Broadcast a message to all agents (empty to_agent).

        Satisfies ``A2AClientProtocol`` from ``survival.instinct``.
        """
        msg_type = _payload_type_to_proto(
            str(payload.get("type", "INFORM"))
        )
        try:
            ack = await self._client.send_message(
                to_agent="",
                message_type=msg_type,
                payload=payload.get("payload", {}),
            )
            return {"status": "ok", "received": ack.received}
        except Exception:
            logger.exception("broadcast_message failed")
            raise


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

_PROTO_TYPE_MAP: dict[str, int] = {
    "DISCOVER": a2a_pb2.DISCOVER,
    "PROPOSE": a2a_pb2.PROPOSE,
    "ACCEPT": a2a_pb2.ACCEPT,
    "REJECT": a2a_pb2.REJECT,
    "INFORM": a2a_pb2.INFORM,
    "TEACH": a2a_pb2.TEACH,
    "REPRODUCE": a2a_pb2.REPRODUCE,
    "WILL": a2a_pb2.WILL,
    "THREAT": a2a_pb2.THREAT,
}


def _payload_type_to_proto(type_str: str) -> int:
    """Convert a string message type to the protobuf enum value."""
    return _PROTO_TYPE_MAP.get(type_str.upper(), a2a_pb2.INFORM)
