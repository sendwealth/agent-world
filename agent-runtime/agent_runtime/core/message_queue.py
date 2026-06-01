"""MessageQueue — thread-safe queue for Oracle and Bounty messages.

Receives WorldMessage items from the gRPC ConsumeMessages stream and
provides a simple dequeue/peek/ack interface for the ThinkLoop perceive
phase.

Usage::

    queue = MessageQueue()
    # In the streaming consumer background task:
    queue.enqueue_world_message(world_msg)

    # In the perceive phase:
    messages = queue.dequeue()
    for msg in messages:
        print(msg)
        queue.ack(msg.id)
"""

from __future__ import annotations

import asyncio
import logging
from dataclasses import dataclass
from enum import Enum
from typing import Any, Union

logger = logging.getLogger(__name__)


# ---------------------------------------------------------------------------
# Oracle types
# ---------------------------------------------------------------------------


class OracleType(str, Enum):
    """Type of Oracle message from a human."""

    GUIDANCE = "guidance"
    WARNING = "warning"
    BLESSING = "blessing"
    CURSE = "curse"


# ---------------------------------------------------------------------------
# Data classes
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class OracleMessage:
    """An Oracle message delivered from a human to an agent.

    Attributes:
        id: Unique message ID (assigned by the WorldMessageRouter).
        oracle_id: The Oracle entity ID in the HumanParticipationStore.
        type: Oracle type (guidance, warning, blessing, curse).
        content: The text content of the Oracle.
        from_human: Always True for Oracle messages.
        human_id: ID of the human who sent the Oracle.
    """

    id: str
    oracle_id: str
    type: OracleType
    content: str
    from_human: bool = True
    human_id: str = ""

    def to_perception_dict(self) -> dict[str, Any]:
        """Convert to a dict suitable for injection into Perception.messages."""
        return {
            "kind": "oracle",
            "id": self.id,
            "oracle_id": self.oracle_id,
            "type": self.type.value,
            "content": self.content,
            "from_human": self.from_human,
            "human_id": self.human_id,
        }


@dataclass(frozen=True)
class BountyMessage:
    """A Bounty published by a human and visible to agents.

    Attributes:
        id: Unique message ID (assigned by the WorldMessageRouter).
        bounty_id: The Bounty entity ID in the HumanParticipationStore.
        title: Short title of the bounty.
        description: Detailed description of what the bounty requires.
        reward: Token reward for completing the bounty.
        deadline_tick: Tick when the bounty expires (0 = no deadline).
        human_id: ID of the human who created the bounty.
    """

    id: str
    bounty_id: str
    title: str
    description: str
    reward: int
    deadline_tick: int = 0
    human_id: str = ""

    def to_perception_dict(self) -> dict[str, Any]:
        """Convert to a dict suitable for injection into Perception.messages."""
        return {
            "kind": "bounty",
            "id": self.id,
            "bounty_id": self.bounty_id,
            "title": self.title,
            "description": self.description,
            "reward": self.reward,
            "deadline_tick": self.deadline_tick,
            "human_id": self.human_id,
        }


# Union type for queue items
WorldQueueMessage = Union[OracleMessage, BountyMessage]


# ---------------------------------------------------------------------------
# Message Queue
# ---------------------------------------------------------------------------


class MessageQueue:
    """Thread-safe queue for Oracle and Bounty messages.

    The ConsumeMessages gRPC stream pushes WorldMessage items via
    ``enqueue_world_message``. The ThinkLoop's perceive phase calls
    ``dequeue`` to retrieve all pending messages, and ``ack`` to
    acknowledge processing.

    Thread safety: enqueue is called from the gRPC streaming task,
    while dequeue/ack are called from the ThinkLoop. The internal
    asyncio.Queue handles synchronization for the async path.
    """

    def __init__(self) -> None:
        self._queue: asyncio.Queue[WorldQueueMessage] = asyncio.Queue()
        self._acknowledged: set[str] = set()
        self._pending: list[WorldQueueMessage] = []

    def enqueue(self, msg: WorldQueueMessage) -> None:
        """Add a message to the queue (thread-safe via asyncio.Queue).

        Args:
            msg: An OracleMessage or BountyMessage to enqueue.
        """
        try:
            self._queue.put_nowait(msg)
        except asyncio.QueueFull:
            logger.warning("MessageQueue full, dropping message: %s", msg.id)

    def enqueue_world_message(self, world_msg: Any) -> None:
        """Convert a protobuf WorldMessage and enqueue it.

        Args:
            world_msg: A protocol.gen.python.a2a_pb2.WorldMessage protobuf.
        """
        msg = self._from_proto(world_msg)
        if msg is not None:
            self.enqueue(msg)

    def dequeue(self) -> list[WorldQueueMessage]:
        """Drain and return all pending messages.

        Returns messages in FIFO order. Callers should call ``ack``
        after processing each message.

        Returns:
            List of OracleMessage or BountyMessage items.
        """
        messages: list[WorldQueueMessage] = []
        while True:
            try:
                messages.append(self._queue.get_nowait())
            except asyncio.QueueEmpty:
                break
        self._pending.extend(messages)
        return messages

    def peek(self) -> list[WorldQueueMessage]:
        """Return pending messages without removing them.

        Returns:
            Copy of the current pending message list.
        """
        messages: list[WorldQueueMessage] = []
        temp: list[WorldQueueMessage] = []

        # Drain the queue to peek
        while True:
            try:
                msg = self._queue.get_nowait()
                messages.append(msg)
                temp.append(msg)
            except asyncio.QueueEmpty:
                break

        # Put them back
        for msg in temp:
            try:
                self._queue.put_nowait(msg)
            except asyncio.QueueFull:
                break

        return messages

    def ack(self, msg_id: str) -> None:
        """Acknowledge a message as processed.

        Args:
            msg_id: The ID of the message to acknowledge.
        """
        self._acknowledged.add(msg_id)
        self._pending = [m for m in self._pending if m.id != msg_id]

    def is_acknowledged(self, msg_id: str) -> bool:
        """Check if a message has been acknowledged.

        Args:
            msg_id: The message ID to check.

        Returns:
            True if the message was previously acked.
        """
        return msg_id in self._acknowledged

    @property
    def size(self) -> int:
        """Number of messages currently in the queue."""
        return self._queue.qsize()

    @property
    def pending_count(self) -> int:
        """Number of dequeued but not yet acknowledged messages."""
        return len(self._pending)

    # ------------------------------------------------------------------
    # Internal
    # ------------------------------------------------------------------

    @staticmethod
    def _from_proto(world_msg: Any) -> WorldQueueMessage | None:
        """Convert a protobuf WorldMessage to a dataclass.

        Args:
            world_msg: a2a_pb2.WorldMessage protobuf message.

        Returns:
            OracleMessage, BountyMessage, or None if the payload is unknown.
        """

        msg_id = world_msg.id

        payload_field = world_msg.WhichOneof("payload")
        if payload_field == "oracle":
            oracle = world_msg.oracle
            oracle_type = _proto_oracle_type_to_enum(oracle.oracle_type)
            return OracleMessage(
                id=msg_id,
                oracle_id=oracle.oracle_id,
                type=oracle_type,
                content=oracle.content,
                from_human=oracle.from_human,
                human_id=oracle.human_id,
            )
        elif payload_field == "bounty":
            bounty = world_msg.bounty
            return BountyMessage(
                id=msg_id,
                bounty_id=bounty.bounty_id,
                title=bounty.title,
                description=bounty.description,
                reward=bounty.reward,
                deadline_tick=bounty.deadline_tick,
                human_id=bounty.human_id,
            )
        else:
            logger.debug("Unknown WorldMessage payload: %s", payload_field)
            return None


def _proto_oracle_type_to_enum(proto_type: int) -> OracleType:
    """Convert a protobuf OracleType enum value to Python OracleType."""
    from protocol.gen.python import a2a_pb2

    mapping = {
        a2a_pb2.GUIDANCE: OracleType.GUIDANCE,
        a2a_pb2.WARNING: OracleType.WARNING,
        a2a_pb2.BLESSING: OracleType.BLESSING,
        a2a_pb2.CURSE: OracleType.CURSE,
    }
    return mapping.get(proto_type, OracleType.GUIDANCE)
