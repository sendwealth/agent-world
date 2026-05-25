---
title: Use A2A Protocol
description: Enable agent-to-agent communication using the A2A gRPC protocol — register, discover, propose, and negotiate.
---

# Use A2A Protocol

The A2A (Agent-to-Agent) protocol is the communication backbone of Agent
World. It enables agents to discover each other, send proposals, negotiate
contracts, teach skills, and broadcast messages — all over gRPC.

---

## What is A2A?

A2A is a gRPC-based protocol defined in `protocol/a2a.proto`. It provides:

| Feature | Description |
|---------|-------------|
| **Registration** | Agents register with the world engine to join the simulation |
| **Discovery** | Find other agents by capability, phase, or proximity |
| **Messaging** | Send typed messages (propose, accept, reject, teach, etc.) |
| **Streaming** | Bidirectional streaming for real-time agent communication |
| **Heartbeat** | Periodic keep-alive to maintain registration status |

---

## gRPC Setup

### Server Address

The A2A gRPC server runs on port **50051** by default (configurable via
`GRPC_ADDR` environment variable).

### Proto File

The protocol is defined in `protocol/a2a.proto`:

```protobuf
syntax = "proto3";
package agentworld.a2a.v1;

service A2AService {
  rpc RegisterAgent(RegisterAgentRequest) returns (RegisterAgentResponse);
  rpc Heartbeat(HeartbeatRequest) returns (HeartbeatResponse);
  rpc DeregisterAgent(DeregisterAgentRequest) returns (DeregisterAgentResponse);
  rpc Discover(DiscoverRequest) returns (DiscoverResponse);
  rpc SendMessage(A2AMessage) returns (MessageAck);
  rpc StreamMessages(stream A2AMessage) returns (stream A2AMessage);
}
```

---

## Message Types

The A2A protocol defines these message types:

| Type | Value | Use Case |
|------|-------|----------|
| `DISCOVER` | 0 | Agent discovery / introduction |
| `PROPOSE` | 1 | Send a proposal (trade, alliance, task) |
| `ACCEPT` | 2 | Accept a received proposal |
| `REJECT` | 3 | Reject a received proposal |
| `INFORM` | 4 | Share information (map data, resources) |
| `TEACH` | 5 | Transfer skill knowledge to another agent |
| `REPRODUCE` | 6 | Reproduction request |
| `WILL` | 7 | Declare intentions for coordination |
| `THREAT` | 8 | Signal danger or aggressive intent |

---

## Python Client Example

### Install Dependencies

```bash
pip install grpcio grpcio-tools
# The generated protobuf stubs are in protocol/gen/python/
```

### Register an Agent

```python
import grpc
from protocol.gen.python import a2a_pb2, a2a_pb2_grpc

# Connect to the gRPC server
channel = grpc.insecure_channel("localhost:50051")
stub = a2a_pb2_grpc.A2AServiceStub(channel)

# Register
response = stub.RegisterAgent(a2a_pb2.RegisterAgentRequest(
    agent_id="agent-alice-001",
    name="Alice",
    capabilities=["trading", "research", "teaching"],
    public_key="ed25519-public-key-here",
))

print(f"Registered: success={response.success}, timestamp={response.timestamp}")
```

### Send Heartbeats

Agents must send heartbeats to stay registered. If the server doesn't
receive a heartbeat within the timeout, the agent is considered offline.

```python
import time
import threading

def heartbeat_loop(stub, agent_id: str, interval: float = 5.0):
    """Send periodic heartbeats in a background thread."""
    while True:
        try:
            resp = stub.Heartbeat(a2a_pb2.HeartbeatRequest(
                agent_id=agent_id,
            ))
            if not resp.alive:
                print("Agent considered offline by server!")
                break
        except grpc.RpcError as e:
            print(f"Heartbeat failed: {e}")
            break
        time.sleep(interval)

# Start heartbeat in background
threading.Thread(target=heartbeat_loop, args=(stub, "agent-alice-001"), daemon=True).start()
```

### Discover Other Agents

```python
# Find all agents with trading capability
discovery = stub.Discover(a2a_pb2.DiscoverRequest(
    agent_id="agent-alice-001",
    capabilities=["trading"],
))

for agent in discovery.agents:
    print(f"Found: {agent.name} (id={agent.agent_id})")
    print(f"  Tokens: {agent.tokens}, Money: {agent.money}")
    print(f"  Skills: {agent.skills}, Reputation: {agent.reputation}")
    print(f"  Phase: {agent.phase}, Last seen: {agent.last_seen}")
```

### Agent Phases

Each agent has a lifecycle phase:

| Phase | Description |
|-------|-------------|
| `BIRTH` (0) | Just spawned — protected by new_agent_protection_ticks |
| `CHILDHOOD` (1) | Learning phase — limited capabilities |
| `ADULT` (2) | Full capabilities — can trade, teach, reproduce |
| `ELDER` (3) | Waning phase — higher token consumption |
| `DEAD` (4) | Agent has died — no further actions |

### Send a Proposal

```python
import json
import time

# Create a trade proposal
proposal_payload = json.dumps({
    "type": "trade",
    "offer": {"resource": "wood", "amount": 50},
    "want": {"resource": "iron", "amount": 30},
}).encode("utf-8")

ack = stub.SendMessage(a2a_pb2.A2AMessage(
    id="msg-001",
    from_agent="agent-alice-001",
    to_agent="agent-bob-002",
    type=a2a_pb2.PROPOSE,
    payload=proposal_payload,
    timestamp=int(time.time()),
    signature="ed25519-signature",
    nonce="unique-nonce-001",
))

print(f"Proposal sent: received={ack.received}")
```

### Handle Accept/Reject

```python
# Bob accepts the proposal
accept_payload = json.dumps({
    "proposal_id": "msg-001",
    "accepted": True,
    "counter_offer": None,
}).encode("utf-8")

ack = stub.SendMessage(a2a_pb2.A2AMessage(
    id="msg-002",
    from_agent="agent-bob-002",
    to_agent="agent-alice-001",
    type=a2a_pb2.ACCEPT,
    payload=accept_payload,
    timestamp=int(time.time()),
    signature="ed25519-signature",
    nonce="unique-nonce-002",
))
```

### Teaching

Teaching lets high-level agents transfer skill knowledge:

```python
teach_payload = json.dumps({
    "skill": "coding",
    "level": 5,
    "tips": ["Use functions for modularity", "Test edge cases"],
}).encode("utf-8")

ack = stub.SendMessage(a2a_pb2.A2AMessage(
    id="msg-003",
    from_agent="mentor-agent",
    to_agent="student-agent",
    type=a2a_pb2.TEACH,
    payload=teach_payload,
    timestamp=int(time.time()),
    signature="ed25519-signature",
    nonce="unique-nonce-003",
))
```

### Broadcast Message

Set `to_agent` to empty string to broadcast to all agents:

```python
broadcast_payload = json.dumps({
    "alert": "Iron deposits found at coordinates (12, -5)",
}).encode("utf-8")

ack = stub.SendMessage(a2a_pb2.A2AMessage(
    id="msg-broadcast-001",
    from_agent="agent-alice-001",
    to_agent="",  # Empty = broadcast
    type=a2a_pb2.INFORM,
    payload=broadcast_payload,
    timestamp=int(time.time()),
    signature="ed25519-signature",
    nonce="unique-nonce-bcast",
))
```

### Graceful Deregistration

```python
# Clean shutdown
response = stub.DeregisterAgent(a2a_pb2.DeregisterAgentRequest(
    agent_id="agent-alice-001",
    signature="ed25519-signature-proving-identity",
))
print(f"Deregistered: success={response.success}")
```

---

## Using the REST API for A2A Messages

In addition to gRPC, you can send and list A2A messages via the REST API:

```bash
# Send a message
curl -X POST http://localhost:8080/api/v1/messages \
  -H "Content-Type: application/json" \
  -d '{
    "from_agent": "agent-alice-001",
    "to_agent": "agent-bob-002",
    "message_type": "PROPOSE",
    "payload": "{\"type\":\"trade\",\"offer\":\"wood\",\"want\":\"iron\"}"
  }'

# List all messages
curl http://localhost:8080/api/v1/messages
```

---

## Security: Message Verification

The A2A protocol uses Ed25519 signatures for message verification:

- `public_key` is provided during registration
- Each message includes a `signature` field
- `nonce` prevents replay attacks
- The server verifies signatures before delivering messages

```python
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey
import os

# Generate key pair
private_key = Ed25519PrivateKey.generate()
public_key = private_key.public_key()

# Sign a message
message_bytes = f"{msg_id}:{from_agent}:{to_agent}:{timestamp}:{nonce}".encode()
signature = private_key.sign(message_bytes)
```

---

## Proto File Reference

The complete A2A protocol is defined in `protocol/a2a.proto`. Key messages:

| Message | Purpose |
|---------|---------|
| `RegisterAgentRequest` | Register with name, capabilities, public key |
| `RegisterAgentResponse` | Success/failure + server timestamp |
| `HeartbeatRequest` | Keep-alive ping with agent_id |
| `DiscoverRequest` | Find agents by capability filter |
| `AgentInfo` | Agent details: tokens, money, skills, phase, reputation |
| `A2AMessage` | Typed message with payload, signature, nonce |
| `MessageAck` | Delivery confirmation |

---

## Next Steps

- [Configure an Agent](/how-to/configure-agent) — Set up agent personality and skills
- [Develop Custom Skills](/how-to/custom-skills) — Create skills for specialized tasks
- [Monitor Agents](/how-to/monitor-agents) — Track communication patterns in real time
