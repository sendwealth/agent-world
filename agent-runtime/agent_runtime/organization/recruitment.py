"""Organization recruitment — sending invitations to potential members.

Handles the process of inviting agents to join a proposed organization
and tracking invitation state (pending, accepted, declined).
"""

from __future__ import annotations

import logging
import uuid
from dataclasses import dataclass
from enum import Enum
from typing import Any, Protocol

from .proposal import OrgProposal

logger = logging.getLogger(__name__)


# ---------------------------------------------------------------------------
# Enums
# ---------------------------------------------------------------------------


class InvitationStatus(str, Enum):
    """Status of a recruitment invitation."""

    PENDING = "pending"
    ACCEPTED = "accepted"
    DECLINED = "declined"
    EXPIRED = "expired"


# ---------------------------------------------------------------------------
# Protocol for A2A message sending
# ---------------------------------------------------------------------------


class A2AClientProtocol(Protocol):
    """Minimal interface for sending invitation messages via A2A."""

    async def send_message(self, payload: dict[str, Any]) -> dict[str, Any]: ...

    async def broadcast_message(self, payload: dict[str, Any]) -> dict[str, Any]: ...


# ---------------------------------------------------------------------------
# Data types
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class Invitation:
    """A recruitment invitation sent to a potential organization member.

    Attributes:
        invitation_id: Unique identifier.
        proposal: The organization proposal this invitation is for.
        target_agent_id: Agent being invited.
        status: Current status of the invitation.
        message: The invitation message sent to the agent.
    """

    invitation_id: str
    proposal: OrgProposal
    target_agent_id: str
    status: InvitationStatus = InvitationStatus.PENDING
    message: str = ""


# ---------------------------------------------------------------------------
# RecruitmentEngine
# ---------------------------------------------------------------------------


class RecruitmentEngine:
    """Manages the recruitment process for organization proposals.

    Sends invitations to potential members via A2A and tracks responses.

    Usage::

        engine = RecruitmentEngine()
        invitations = await engine.send_invitations(
            proposal=my_proposal,
            candidate_ids=["agent-002", "agent-003"],
            a2a_client=world_client,
        )
    """

    def __init__(self) -> None:
        self._invitations: dict[str, Invitation] = {}

    async def send_invitations(
        self,
        proposal: OrgProposal,
        candidate_ids: list[str],
        a2a_client: A2AClientProtocol,
    ) -> list[Invitation]:
        """Send recruitment invitations to candidate agents.

        Only sends invitations to agents not already in the founding members list.

        Args:
            proposal: The org proposal to invite candidates to.
            candidate_ids: Agent IDs to invite.
            a2a_client: A2A client for sending messages.

        Returns:
            List of created Invitation objects.
        """
        invitations: list[Invitation] = []
        founding_set = set(proposal.founding_members)

        for agent_id in candidate_ids:
            if agent_id in founding_set:
                logger.debug(
                    "Skipping invitation to %s — already a founding member.",
                    agent_id,
                )
                continue

            invitation = self._create_invitation(proposal, agent_id)
            self._invitations[invitation.invitation_id] = invitation

            # Send invitation via A2A
            try:
                await a2a_client.send_message(
                    {
                        "to_agent": agent_id,
                        "type": "PROPOSE",
                        "payload": {
                            "action": "org_invitation",
                            "org_name": proposal.org_name,
                            "org_type": proposal.org_type.value,
                            "charter": proposal.charter,
                            "founder_id": proposal.founder_id,
                            "proposal_id": proposal.proposal_id,
                        },
                    }
                )
                logger.info(
                    "Sent org invitation to %s for %s",
                    agent_id,
                    proposal.org_name,
                )
            except Exception:
                logger.exception(
                    "Failed to send org invitation to %s",
                    agent_id,
                )

            invitations.append(invitation)

        return invitations

    def respond_to_invitation(
        self,
        invitation_id: str,
        accept: bool,
    ) -> Invitation | None:
        """Record an agent's response to an invitation.

        Args:
            invitation_id: The invitation being responded to.
            accept: Whether the agent accepts the invitation.

        Returns:
            Updated Invitation, or None if invitation_id not found.
        """
        invitation = self._invitations.get(invitation_id)
        if invitation is None:
            logger.warning("Invitation %s not found.", invitation_id)
            return None

        new_status = InvitationStatus.ACCEPTED if accept else InvitationStatus.DECLINED
        updated = Invitation(
            invitation_id=invitation.invitation_id,
            proposal=invitation.proposal,
            target_agent_id=invitation.target_agent_id,
            status=new_status,
            message=invitation.message,
        )
        self._invitations[invitation_id] = updated
        return updated

    def get_invitation(self, invitation_id: str) -> Invitation | None:
        """Retrieve an invitation by ID."""
        return self._invitations.get(invitation_id)

    def get_pending_invitations(self, agent_id: str) -> list[Invitation]:
        """Get all pending invitations targeting a specific agent."""
        return [
            inv
            for inv in self._invitations.values()
            if inv.target_agent_id == agent_id
            and inv.status == InvitationStatus.PENDING
        ]

    # ------------------------------------------------------------------
    # Internal helpers
    # ------------------------------------------------------------------

    @staticmethod
    def _create_invitation(proposal: OrgProposal, target_agent_id: str) -> Invitation:
        """Create an Invitation object with a generated message."""
        message = (
            f"You are invited to join '{proposal.org_name}' "
            f"({proposal.org_type.value}). "
            f"Charter: {proposal.charter}"
        )
        return Invitation(
            invitation_id=f"inv-{uuid.uuid4().hex[:12]}",
            proposal=proposal,
            target_agent_id=target_agent_id,
            status=InvitationStatus.PENDING,
            message=message,
        )
