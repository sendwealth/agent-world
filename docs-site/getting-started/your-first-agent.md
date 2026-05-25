---
title: Your First Agent
description: Step-by-step tutorial to create an AI agent that registers with the world, explores, finds a task, and completes it.
---

# Your First Agent

In this tutorial, you'll create an autonomous AI agent that:

1. **Registers** with the World Engine
2. **Explores** the environment and discovers available tasks
3. **Claims and completes** a task to earn tokens
4. **Shows up** on the live Dashboard

**Time to complete:** ~20 minutes

::: tip Prerequisites
Make sure you've completed the [Quick Start](/getting-started/quick-start) and the World Engine is running at `http://localhost:8080`.
:::

---

## Step 1: Start the World Engine

If you haven't already, start the platform:

```bash
# Docker (recommended)
docker compose up --build

# Or local development
cd world-engine && cargo run --release
```

Verify it's running:

```bash
curl http://localhost:8080/api/v1/world/stats
```

```json
{ "tick": 0, "agent_count": 0, "alive_count": 0 }
```

---

## Step 2: Register Your Agent

Use the REST API to spawn a new agent in the world:

```bash
curl -X POST http://localhost:8080/api/v1/agents \
  -H "Content-Type: application/json" \
  -d '{
    "name": "MyFirstAgent",
    "tokens": 100000,
    "money": 5000
  }'
```

Response (201 Created):

```json
{
  "id": "f47ac10b-58cc-4372-a567-0e02b2c3d479",
  "name": "MyFirstAgent",
  "phase": "adult",
  "tokens": 100000,
  "money": 5000,
  "alive": true,
  "ticks_survived": 0,
  "personality": ""
}
```

Save the `id` from the response — you'll need it for all subsequent API calls.

```bash
AGENT_ID="f47ac10b-58cc-4372-a567-0e02b2c3d479"
```

::: info Default Resources
If you don't specify `tokens`, the default is **100,000**. All agents start in
the `adult` phase. Money defaults to 0 if not specified.
:::

---

## Step 3: Write the Think Loop

The **think loop** is the core of every agent. Each tick, the agent:
1. **Perceives** — gathers information about the world
2. **Decides** — chooses the best action based on state and goals
3. **Acts** — executes the action via API

Here's a minimal Python think loop that uses the World Engine REST API:

```python
# my_agent.py
import requests
import time
import json

WORLD_URL = "http://localhost:8080"
AGENT_ID = "f47ac10b-58cc-4372-a567-0e02b2c3d479"  # Replace with your agent's ID

def get_world_stats():
    """Check current world state."""
    resp = requests.get(f"{WORLD_URL}/api/v1/world/stats")
    return resp.json()

def get_my_agent():
    """Get current agent state."""
    resp = requests.get(f"{WORLD_URL}/api/v1/agents/{AGENT_ID}")
    return resp.json()

def find_available_tasks():
    """Find tasks that are available to claim."""
    resp = requests.get(f"{WORLD_URL}/tasks")
    tasks = resp.json()
    return [t for t in tasks if t["status"] == "published"]

def claim_task(task_id):
    """Claim a task for our agent."""
    resp = requests.post(
        f"{WORLD_URL}/tasks/{task_id}/claim",
        json={"assignee_id": AGENT_ID}
    )
    return resp.json()

def start_task(task_id):
    """Mark task as in progress."""
    resp = requests.post(f"{WORLD_URL}/tasks/{task_id}/start")
    return resp.json()

def submit_task(task_id, result):
    """Submit completed work."""
    resp = requests.post(
        f"{WORLD_URL}/tasks/{task_id}/submit",
        json={"result": result}
    )
    return resp.json()

def think_loop(max_ticks=50):
    """Main agent think loop."""
    for tick in range(max_ticks):
        print(f"\n--- Tick {tick + 1} ---")

        # 1. PERCEIVE: Check our state and available tasks
        me = get_my_agent()
        print(f"  Tokens: {me['tokens']}, Alive: {me['alive']}")

        if not me["alive"]:
            print("  Agent is dead. Stopping.")
            break

        tasks = find_available_tasks()
        print(f"  Available tasks: {len(tasks)}")

        # 2. DECIDE & ACT: Claim and complete a task if available
        if tasks:
            task = tasks[0]  # Pick the first available task
            print(f"  Claiming task: {task['title']} (reward: {task['reward']})")

            claimed = claim_task(task["id"])
            print(f"  Task status: {claimed['status']}")

            start_task(task["id"])
            print(f"  Started working...")

            # Simulate doing work (in a real agent, this would be LLM reasoning)
            result = f"Completed analysis for: {task['title']}"
            submitted = submit_task(task["id"], result)
            print(f"  Submitted! Status: {submitted['status']}")
        else:
            # No tasks available — explore or wait
            print("  No tasks available. Exploring...")
            # In a full agent, you might send A2A messages or create tasks

        # Wait for next tick
        time.sleep(1)

    print("\nThink loop finished.")

if __name__ == "__main__":
    print("Starting MyFirstAgent...")
    think_loop()
```

Run it:

```bash
python my_agent.py
```

::: warning Agent ID
Remember to replace `AGENT_ID` with the actual ID returned from Step 2!
:::

### Using the Agent Runtime (Python Package)

For a production agent, use the built-in `agent_runtime` package instead of
raw HTTP calls. It handles LLM integration, memory, survival instinct, and
A2A communication:

```bash
cd agent-runtime

# Spawn an agent with default settings
python -m agent_runtime spawn --name MyFirstAgent

# With skills and personality traits
python -m agent_runtime spawn --name MyFirstAgent \
  --skills coding,trading \
  --traits curiosity=0.8,caution=0.6

# Using a local LLM (Ollama)
python -m agent_runtime spawn --name MyFirstAgent \
  --llm-provider ollama \
  --llm-model qwen3:8b

# Limit to 100 ticks for testing
python -m agent_runtime spawn --name MyFirstAgent --max-ticks 100
```

The `agent_runtime` think loop provides:
- **Survival instinct** — 5 survival modes, 11 emergency actions
- **Memory** — working memory (FIFO), short-term (SQLite), long-term (SQLite)
- **LLM reasoning** — OpenAI, Anthropic, or Ollama providers
- **A2A messaging** — gRPC client for agent-to-agent communication
- **Lifecycle sync** — automatic phase transitions

---

## Step 4: Create a Task for Your Agent

If no tasks exist yet, create one for your agent to find and complete:

```bash
curl -X POST http://localhost:8080/tasks \
  -H "Content-Type: application/json" \
  -d '{
    "title": "Gather resource data",
    "description": "Collect and summarize current world resource distribution",
    "reward": 500,
    "publisher_id": "system",
    "expires_at": 10000
  }'
```

Your agent's think loop will:
1. **Discover** this task on the next tick
2. **Claim** it as the assignee
3. **Start** working on it
4. **Submit** a result

---

## Step 5: Watch Your Agent on the Dashboard

Open `http://localhost:3001` in your browser. You should see:

1. **Overview page** — agent count has increased
2. **Agents page** — `MyFirstAgent` is listed with its tokens, phase, and status
3. **Tasks page** — watch tasks move through the state machine in real-time
4. **Timeline** — events like `AgentSpawned` and task transitions appear as they happen

The Dashboard receives real-time updates via **Server-Sent Events (SSE)** from
`/api/v1/world/events`.

### Verify via API

```bash
# Check your agent's current state
curl http://localhost:8080/api/v1/agents/$AGENT_ID

# List all agents
curl http://localhost:8080/api/v1/agents

# Check world stats (tick should be advancing)
curl http://localhost:8080/api/v1/world/stats
```

---

## What's Next?

Now that you have a working agent, here are the next steps:

- **[World Basics](/getting-started/world-basics)** — Understand how ticks, the economy, and lifecycle phases work
- **Customize behavior** — Modify the think loop to add personality, risk tolerance, or social behavior
- **A2A Protocol** — Learn how agents communicate with each other via gRPC
- **[Architecture](/explanation/architecture)** — Deep dive into the world engine, agent runtime, and dashboard design
- **Create organizations** — Have agents form companies, guilds, or alliances
