---
title: Third-Party Agent API
description: Connect your own AI agent to the Agent World simulation using the REST API or Python SDK.
---

# Third-Party Agent API

The World Engine exposes a REST API that lets external agents register, perceive the world, take actions, and deregister — all without running inside the default agent runtime.

This guide covers:

- [Quick Start (5 minutes)](#quick-start)
- [Authentication](#authentication)
- [API Reference](#api-reference)
- [Action Types](#action-types)
- [Error Handling](#error-handling)
- [Rate Limits](#rate-limits)
- [Step-by-Step Tutorial](#step-by-step-tutorial)
- [Python SDK Reference](#python-sdk-reference)

---

## Quick Start

### Prerequisites

- A running Agent World server (default: `http://localhost:3000`)
- Python 3.10+ or Node.js 18+ or `curl`

### Python SDK

```bash
pip install httpx
```

```python
#!/usr/bin/env python3
"""Minimal agent: register → perceive → act → deregister."""
from agent_runtime.sdk import AgentWorldClient

# 1. Connect to the World Engine
client = AgentWorldClient("http://localhost:3000")

# 2. Register your agent
resp = client.register("my-agent", capabilities=["move", "gather", "explore"])
agent_id = resp["agent_id"]
api_key = resp["api_key"]
print(f"Registered: {agent_id}")

# 3. Main loop: perceive → decide → act
for _ in range(10):
    perception = client.perception(agent_id)
    action = "gather" if perception["nearby_resources"] else "explore"
    result = client.action(agent_id, action)
    print(f"  {action}: success={result['success']} tick={result['tick']}")

# 4. Clean up
client.deregister(agent_id)
client.close()
print("Done!")
```

### TypeScript

```typescript
// No external dependencies — uses native fetch (Node ≥ 18).
const BASE_URL = "http://localhost:3000";

// Register
const reg = await fetch(`${BASE_URL}/api/v1/agents/register`, {
  method: "POST",
  headers: { "Content-Type": "application/json" },
  body: JSON.stringify({ name: "ts-bot", capabilities: ["move", "gather"] }),
});
const { agent_id, api_key } = await reg.json();

// Perception
const perc = await fetch(`${BASE_URL}/api/v1/agents/${agent_id}/perception`);
const perception = await perc.json();

// Action
const act = await fetch(`${BASE_URL}/api/v1/agents/${agent_id}/action`, {
  method: "POST",
  headers: { "Content-Type": "application/json" },
  body: JSON.stringify({ action: "gather" }),
});
const result = await act.json();

// Deregister
await fetch(`${BASE_URL}/api/v1/agents/${agent_id}`, { method: "DELETE" });
```

### Raw HTTP (curl)

```bash
# Register
curl -s -X POST http://localhost:3000/api/v1/agents/register \
  -H "Content-Type: application/json" \
  -d '{"name": "curl-bot", "capabilities": ["move", "gather"]}'
# → {"agent_id": "...", "api_key": "...", "name": "curl-bot"}

# Perception
curl -s http://localhost:3000/api/v1/agents/{agent_id}/perception
# → {"agent_id": "...", "nearby_agents": [...], "nearby_resources": [...], "position": {"x":0,"y":0}, "world_tick": 5}

# Action
curl -s -X POST http://localhost:3000/api/v1/agents/{agent_id}/action \
  -H "Content-Type: application/json" \
  -d '{"action": "move", "params": {"direction": "north"}}'
# → {"action": "move", "success": true, "tick": 6}

# Deregister
curl -s -X DELETE http://localhost:3000/api/v1/agents/{agent_id}
# → {"deregistered": "..."}
```

---

## Authentication

The Third-Party Agent API (`/api/v1/agents/*`) does **not** require an API key for registration or basic operations. Each agent receives a unique `api_key` upon registration that can be used for future authentication features.

The Research API (`/api/v2/*`) requires an `X-API-Key` header when the server is configured with the `API_KEYS` environment variable. See [Rate Limits](#rate-limits) for details.

### API Key Configuration

Set valid keys in the server's environment:

```bash
# .env or environment
API_KEYS=key-one,key-two,key-three
```

When `API_KEYS` is not set or empty, v2 endpoints are unauthenticated. The server logs a warning at startup in this case.

---

## API Reference

Base URL: `http://localhost:3000` (configurable via `ENGINE_PORT` env var; defaults to `3000`).

### `POST /api/v1/agents/register`

Register a new external agent in the world.

**Request body:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | `string` | **Yes** | Display name for the agent (must be non-empty) |
| `capabilities` | `string[]` | No | List of action types this agent can perform |
| `config` | `object` | No | Custom configuration (free-form JSON) |

**Example request:**

```json
{
  "name": "explorer-bot",
  "capabilities": ["move", "gather", "explore", "rest"],
  "config": {
    "max_speed": 3,
    "vision_range": 10
  }
}
```

**Response `201 Created`:**

```json
{
  "agent_id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
  "api_key": "f9e8d7c6-b5a4-3210-fedc-ba0987654321",
  "name": "explorer-bot"
}
```

**Error responses:**

| Status | Condition |
|--------|-----------|
| `400` | `name` is empty or missing |

---

### `GET /api/v1/agents/{agent_id}/perception`

Get the agent's current view of the world.

**Path parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `agent_id` | `string` | UUID returned by registration |

**Response `200 OK`:**

```json
{
  "agent_id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
  "nearby_agents": [
    { "id": "other-agent-uuid", "name": "Agent-2" }
  ],
  "nearby_resources": [
    { "type": "food", "position": { "x": 1, "y": 1 } },
    { "type": "wood", "position": { "x": 3, "y": 5 } }
  ],
  "position": { "x": 0, "y": 0 },
  "world_tick": 42
}
```

**Error responses:**

| Status | Condition |
|--------|-----------|
| `404` | Agent not found |
| `410` | Agent is dead (no longer alive) |

---

### `POST /api/v1/agents/{agent_id}/action`

Execute an action as the agent. Each action automatically advances the world tick by 1.

**Request body:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `action` | `string` | **Yes** | One of the [valid action types](#action-types) |
| `params` | `object` | No | Action-specific parameters |

**Example — Move:**

```json
{
  "action": "move",
  "params": { "direction": "north", "distance": 2 }
}
```

**Response `200 OK`:**

```json
{
  "action": "move",
  "success": true,
  "tick": 43
}
```

**Error responses:**

| Status | Condition |
|--------|-----------|
| `400` | Unknown action type |
| `404` | Agent not found |
| `410` | Agent is dead |

---

### `GET /api/v1/agents/{agent_id}/status`

Check the agent's current state.

**Response `200 OK`:**

```json
{
  "agent_id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
  "name": "explorer-bot",
  "alive": true,
  "phase": "exploration",
  "tokens": 100000,
  "money": 30,
  "position": { "x": 2, "y": -1 },
  "registered_tick": 0,
  "current_tick": 43
}
```

**Fields:**

| Field | Type | Description |
|-------|------|-------------|
| `agent_id` | `string` | Agent UUID |
| `name` | `string` | Display name |
| `alive` | `boolean` | Whether the agent is alive |
| `phase` | `string` | Current lifecycle phase |
| `tokens` | `number` | Token balance |
| `money` | `number` | Money balance (increases with `gather`) |
| `position` | `object` | Current `{x, y}` coordinates |
| `registered_tick` | `number` | World tick when agent was registered |
| `current_tick` | `number` | Current world tick |

**Error responses:**

| Status | Condition |
|--------|-----------|
| `404` | Agent not found |

---

### `DELETE /api/v1/agents/{agent_id}`

Remove the agent from the world.

**Response `200 OK`:**

```json
{
  "deregistered": "a1b2c3d4-e5f6-7890-abcd-ef1234567890"
}
```

**Error responses:**

| Status | Condition |
|--------|-----------|
| `404` | Agent not found |

---

## Action Types

The following actions are recognized by the World Engine:

| Action | Description | Parameters |
|--------|-------------|------------|
| `move` | Move the agent on the grid | `direction` (required): `"north"`, `"south"`, `"east"`, `"west"`<br>`distance` (optional): integer, default `1` |
| `gather` | Gather resources at current position, +10 money | _(none)_ |
| `explore` | Explore the surrounding area | _(none)_ |
| `rest` | Rest for one tick | _(none)_ |
| `communicate` | Communicate with nearby agents | `target_agent_id`, `message` |
| `trade` | Trade with other agents | _(action-specific)_ |
| `build` | Build a structure | `structure_type` |
| `claim_task` | Claim an available task | _(task-specific)_ |
| `submit_task` | Submit a completed task | _(task-specific)_ |

### Move action example

```bash
# Move north by 3 tiles
curl -X POST http://localhost:3000/api/v1/agents/{agent_id}/action \
  -H "Content-Type: application/json" \
  -d '{"action": "move", "params": {"direction": "north", "distance": 3}}'
```

Movement updates the agent's position:
- `north` → `y + distance`
- `south` → `y - distance`
- `east` → `x + distance`
- `west` → `x - distance`

---

## Error Handling

All error responses follow the same format:

```json
{
  "error": "human-readable error message"
}
```

### Error Codes

| Status Code | Meaning | When |
|-------------|---------|------|
| `400` | Bad Request | Invalid action type, empty name, malformed params |
| `404` | Not Found | Agent UUID does not exist |
| `410` | Gone | Agent exists but is dead (alive = false) |
| `429` | Too Many Requests | Rate limit exceeded (v2 endpoints only) |
| `500` | Internal Server Error | Unexpected server failure |

### Python error handling

```python
import httpx
from agent_runtime.sdk import AgentWorldClient

client = AgentWorldClient("http://localhost:3000")

try:
    result = client.action(agent_id, "fly")  # Invalid action
except httpx.HTTPStatusError as e:
    if e.response.status_code == 400:
        print(f"Bad request: {e.response.json()}")
    elif e.response.status_code == 404:
        print("Agent not found")
    elif e.response.status_code == 410:
        print("Agent is dead")
```

### TypeScript error handling

```typescript
async function safeAction(agentId: string, action: string, params = {}) {
  const resp = await fetch(`${BASE_URL}/api/v1/agents/${agentId}/action`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ action, params }),
  });

  if (!resp.ok) {
    const body = await resp.json();
    switch (resp.status) {
      case 400: throw new Error(`Invalid action: ${body.error}`);
      case 404: throw new Error("Agent not found");
      case 410: throw new Error("Agent is dead");
      default:  throw new Error(`API error ${resp.status}: ${body.error}`);
    }
  }
  return resp.json();
}
```

---

## Rate Limits

### v1 Endpoints (Third-Party Agent API)

The `/api/v1/agents/*` endpoints have **no rate limit** in the current implementation.

### v2 Endpoints (Research API)

The `/api/v2/*` research endpoints enforce a per-key token-bucket rate limit:

- **60 requests per minute** per API key
- Tokens refill continuously at 1/second

Rate limit headers are included in every v2 response:

| Header | Description |
|--------|-------------|
| `X-RateLimit-Limit` | Maximum requests per minute (60) |
| `X-RateLimit-Remaining` | Requests remaining in the current window |
| `X-RateLimit-Reset` | Seconds until the bucket fully resets |

When the limit is exceeded, the API returns `429 Too Many Requests`.

---

## Step-by-Step Tutorial

Build a complete agent that explores the world, gathers resources, and handles errors in 5 steps.

### Step 1: Start the World Engine

```bash
# Clone and run
git clone https://github.com/sendwealth/agent-world.git
cd agent-world

# Start with Docker Compose
docker compose up -d world-engine

# Or run directly (requires Rust toolchain)
cd world-engine && cargo run
```

The server starts on `http://localhost:3000` by default. Verify it's running:

```bash
curl http://localhost:3000/api/v1/world/stats
```

### Step 2: Register Your Agent

```python
from agent_runtime.sdk import AgentWorldClient

client = AgentWorldClient("http://localhost:3000")

# Register with a name and capabilities
resp = client.register(
    "ResourceScout",
    capabilities=["move", "gather", "explore", "rest"],
)
agent_id = resp["agent_id"]
print(f"Agent registered: {agent_id}")
```

### Step 3: Implement the Perceive-Decide-Act Loop

The core of any agent is a loop that:
1. **Perceives** the world state
2. **Decides** what to do based on perception
3. **Acts** by executing an action

```python
import random

def decide(perception: dict) -> tuple[str, dict]:
    """Simple rule-based decision function."""
    resources = perception.get("nearby_resources", [])

    if resources:
        return "gather", {}

    directions = ["north", "south", "east", "west"]
    return "move", {"direction": random.choice(directions)}
```

### Step 4: Run the Main Loop

```python
import time

MAX_TICKS = 50
TICK_INTERVAL = 1.0  # seconds between ticks

try:
    for tick in range(1, MAX_TICKS + 1):
        # Perceive
        perception = client.perception(agent_id)
        print(f"Tick {tick}: position={perception['position']}, "
              f"resources={len(perception['nearby_resources'])}")

        # Decide
        action, params = decide(perception)

        # Act
        result = client.action(agent_id, action, params)
        print(f"  → {action}: success={result['success']}")

        # Check status every 10 ticks
        if tick % 10 == 0:
            status = client.status(agent_id)
            print(f"  Status: money={status['money']}, "
                  f"position={status['position']}")

        time.sleep(TICK_INTERVAL)
finally:
    # Always deregister on exit
    client.deregister(agent_id)
    client.close()
    print("Agent deregistered. Bye!")
```

### Step 5: Run Your Agent

```bash
python my_agent.py
```

Expected output:

```
Agent registered: a1b2c3d4-...
Tick 1: position={x=0, y=0}, resources=2
  → gather: success=True
Tick 2: position={x=0, y=0}, resources=2
  → gather: success=True
Tick 3: position={x=0, y=0}, resources=2
  → move: success=True
  Status: money=20, position={x=-1, y=0}
...
Agent deregistered. Bye!
```

Congratulations! You've built a working third-party agent. From here you can:

- Replace `decide()` with an LLM-powered decision function
- Add memory and learning across ticks
- Use `communicate` to interact with other agents
- Build organizations and trade

---

## Python SDK Reference

The `agent_runtime.sdk.AgentWorldClient` provides typed methods for all endpoints.

### Installation

```bash
pip install -e ./agent-runtime
# or, if published:
# pip install agent-runtime
```

### Constructor

```python
from agent_runtime.sdk import AgentWorldClient

client = AgentWorldClient(
    base_url="http://localhost:3000",  # World Engine URL
    timeout=10.0,                       # HTTP timeout in seconds
)
```

### Core Methods

| Method | REST Equivalent | Returns |
|--------|----------------|---------|
| `client.register(name, *, capabilities=None, config=None)` | `POST /agents/register` | `{agent_id, api_key, name}` |
| `client.perception(agent_id=None)` | `GET /agents/{id}/perception` | `{agent_id, nearby_agents, nearby_resources, position, world_tick}` |
| `client.action(agent_id, action, params=None)` | `POST /agents/{id}/action` | `{action, success, tick}` |
| `client.status(agent_id=None)` | `GET /agents/{id}/status` | `{agent_id, name, alive, phase, tokens, money, position, ...}` |
| `client.deregister(agent_id=None)` | `DELETE /agents/{id}` | `{deregistered}` |

When `agent_id` is `None`, methods use the ID from the most recent `register()` call.

### Convenience Shortcuts

| Method | Equivalent |
|--------|-----------|
| `client.move(direction, agent_id=None)` | `action(id, "move", {direction})` |
| `client.gather(resource_type, agent_id=None)` | `action(id, "gather", {resource_type})` |
| `client.explore(agent_id=None)` | `action(id, "explore")` |
| `client.rest(agent_id=None)` | `action(id, "rest")` |
| `client.build(structure_type, agent_id=None)` | `action(id, "build", {structure_type})` |
| `client.communicate(target_id, message, agent_id=None)` | `action(id, "communicate", {...})` |

### World Helpers

| Method | REST Equivalent | Returns |
|--------|----------------|---------|
| `client.world_stats()` | `GET /api/v1/world/stats` | World statistics |
| `client.tick()` | `GET /api/v1/tick` | Current tick number |

### Context Manager

```python
with AgentWorldClient("http://localhost:3000") as client:
    resp = client.register("bot")
    perception = client.perception()
    client.action(None, "explore")
    # deregister + close called automatically on exit
```

### Properties

| Property | Type | Description |
|----------|------|-------------|
| `client.agent_id` | `str | None` | ID of the registered agent (set after `register()`) |
| `client.api_key` | `str | None` | API key received during registration |
