"""A2A message builder and converter — protobuf <-> dict translation.

Builds A2AMessage protobuf objects from Python dicts with proper
signing and nonce fields, and converts received protobufs back to
plain dicts for consumption by the rest of the runtime.
"""

from __future__ import annotations

import json
import time
import uuid
from typing import Any

from protocol.gen.python import a2a_pb2


def build_a2a_message(
    *,
    from_agent: str,
    to_agent: str = "",
    message_type: int = a2a_pb2.INFORM,
    payload: dict[str, Any] | None = None,
    signature: str = "",
    nonce: str = "",
) -> a2a_pb2.A2AMessage:
    """Build an A2AMessage protobuf from keyword arguments.

    Args:
        from_agent: Sender agent ID.
        to_agent: Recipient agent ID (empty string = broadcast).
        message_type: One of the MessageType enum values from a2a_pb2.
        payload: Arbitrary dict to encode as JSON bytes.
        signature: Ed25519 hex-encoded signature (set after signing).
        nonce: Replay-protection nonce (auto-generated if empty).

    Returns:
        A populated A2AMessage protobuf.
    """
    payload_bytes = json.dumps(payload or {}, separators=(",", ":")).encode("utf-8")
    if not nonce:
        nonce = uuid.uuid4().hex

    return a2a_pb2.A2AMessage(
        id=uuid.uuid4().hex[:16],
        from_agent=from_agent,
        to_agent=to_agent,
        type=message_type,
        payload=payload_bytes,
        timestamp=int(time.time()),
        signature=signature,
        nonce=nonce,
    )


def a2a_message_to_dict(msg: a2a_pb2.A2AMessage) -> dict[str, Any]:
    """Convert an A2AMessage protobuf to a plain dict.

    Decodes the JSON payload bytes back into a Python dict.
    """
    try:
        payload = json.loads(msg.payload.decode("utf-8")) if msg.payload else {}
    except (json.JSONDecodeError, UnicodeDecodeError):
        payload = {}

    return {
        "id": msg.id,
        "from_agent": msg.from_agent,
        "to_agent": msg.to_agent,
        "type": msg.type,
        "payload": payload,
        "timestamp": msg.timestamp,
        "signature": msg.signature,
        "nonce": msg.nonce,
    }
