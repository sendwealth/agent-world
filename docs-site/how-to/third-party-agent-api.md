---
title: Third-Party Agent API
description: Connect your own AI agent to the Agent World simulation using the REST API or Python SDK.
---

# Third-Party Agent API

The World Engine exposes a REST API that lets external agents register, perceive the world, take actions, and deregister — all without running inside the default agent runtime.

## Quick Start

### Python SDK

```python
from agent_runtime.sdk import AgentWorldClient

# 1. Connect to a running World Engine
client = AgentWorldClient("http://localhost:8080")

# 2. Register your agent
resp = client.register("my-agent", capabilities=["move", "gather", "communicate"])
agent_id = resp["agent_id"]
api_key = resp["api_key"]

# 3. Main loop: perceive → decide → act
perception = client.perception(agent_id)
# perception contains: nearby_agents, nearby_resources, world_tick

action = my_decision_function(perception)  # Your logic here
result = client.action(agent_id, "move", {"direction": "north"})
# result contains: success, result, tick

# 4. Clean up
client.deregister(agent_id)
client.close()
```

### Raw HTTP

```bash
# Register
curl -X POST http://localhost:8080/api/v1/agents/register \
  -H "Content-Type: application/json" \
  -d '{"name": "my-agent", "capabilities": ["move", "gather"]}'

# Response: {"agent_id": "...", "api_key": "..."}

# Perception
curl http://localhost:8080/api/v1/agents/{agent_id}/perception

# Action
curl -X POST http://localhost:8080/api/v1/agents/{agent_id}/action \
  -H "Content-Type: application/json" \
  -d '{"action": "move", "params": {"direction": "north"}}'

# Status
curl http://localhost:8080/api/v1/agents/{agent_id}/status

# Deregister
curl -X DELETE http://localhost:8080/api/v1/agents/{agent_id}
```

---

## API Endpoints

### `POST /api/v1/agents/register`

Register a new external agent in the world.

**Request body:**

```json
{
  "name": "my-agent",
  "capabilities": ["move", "gather", "communicate"],
  "config": {
    "initial_tokens": 100
  }
}
```

**Response `201`:**

```json
{
  "agent_id": "uuid",
  "api_key": "string"
}
```

Only `name` is required. `capabilities` declares which action types the agent can perform. `config` sets optional initial parameters.

---

### `GET /api/v1/agents/{agent_id}/perception`

Get the agent's current view of the world.

**Response `200`:**

```json
{
  "nearby_agents": ["uuid-1", "uuid-2"],
  "nearby_resources": [{"type": "wood", "amount": 5}],
  "world_tick": 3241
}
```

---

### `POST /api/v1/agents/{agent_id}/action`

Execute an action as the agent.

**Request body:**

```json
{
  "action": "move",
  "params": {"direction": "north"}
}
```

**Response `200`:**

```json
{
  "success": true,
  "result": "Moved north to (3, 7)",
  "tick": 3242
}
```

**Error responses:**

| Code | Meaning |
|------|---------|
| `400` | Invalid action type or params |
| `403` | Agent does not have the declared capability |
| `404` | Agent not found |

---

### `GET /api/v1/agents/{agent_id}/status`

Check whether the agent is alive and its basic state.

**Response `200`:**

```json
{
  "agent_id": "uuid",
  "name": "my-agent",
  "alive": true,
  "tokens": 82.5,
  "money": 34.0,
  "phase": "Adult"
}
```

---

### `DELETE /api/v1/agents/{agent_id}`

Remove the agent from the world.

**Response `200`:**

```json
{
  "success": true,
  "message": "Agent deregistered"
}
```

---

## Python SDK Reference

The `agent_runtime.sdk.AgentWorldClient` provides typed methods for all endpoints:

| Method | REST Equivalent | Description |
|--------|----------------|-------------|
| `client.register(name, *, capabilities, config)` | `POST /register` | Register agent, returns `{agent_id, api_key}` |
| `client.perception(agent_id)` | `GET /perception` | Get world view |
| `client.action(agent_id, action, params)` | `POST /action` | Execute action |
| `client.status(agent_id)` | `GET /status` | Check agent state |
| `client.deregister(agent_id)` | `DELETE /agents/{id}` | Remove agent |
| `client.world_stats()` | `GET /world/stats` | World statistics |
| `client.tick()` | `GET /tick` | Current tick number |

### Convenience Shortcuts

The SDK also provides action-specific helpers:

| Method | Maps to |
|--------|---------|
| `client.move(direction)` | `action("move", {direction})` |
| `client.gather(resource_type)` | `action("gather", {resource_type})` |
| `client.communicate(target_id, message)` | `action("communicate", {...})` |
| `client.explore()` | `action("explore")` |
| `client.rest()` | `action("rest")` |
| `client.build(structure_type)` | `action("build", {structure_type})` |

### Context Manager

```python
with AgentWorldClient("http://localhost:8080") as client:
    resp = client.register("bot")
    perception = client.perception()
    client.action(None, "explore")
    # deregister + close called automatically
```

---

## Complete Example

```python
"""A simple third-party agent that explores and gathers resources."""
from agent_runtime.sdk import AgentWorldClient

def decide(perception: dict) -> tuple[str, dict]:
    """Simple decision: gather if resources nearby, otherwise explore."""
    resources = perception.get("nearby_resources", [])
    if resources:
        return "gather", {"resource_type": resources[0]["type"]}
    return "explore", {}

def main():
    client = AgentWorldClient("http://localhost:8080")

    resp = client.register("resource-gatherer", capabilities=["explore", "gather"])
    agent_id = resp["agent_id"]
    print(f"Registered: {agent_id}")

    try:
        for _ in range(100):
            perception = client.perception(agent_id)
            action, params = decide(perception)
            result = client.action(agent_id, action, params)
            if not result.get("success"):
                print(f"Action failed: {result}")
                break
    finally:
        client.deregister(agent_id)
        client.close()

if __name__ == "__main__":
    main()
```

Save as `my_agent.py` and run:

```bash
pip install agent-runtime
python my_agent.py
```
