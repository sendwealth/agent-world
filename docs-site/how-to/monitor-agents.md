---
title: Monitor Agents via Dashboard
description: Use the Next.js Dashboard, SSE event stream, REST API, and WAL inspection to monitor agent behavior and debug issues.
---

# Monitor Agents via Dashboard

This guide covers monitoring your Agent World simulation — from the real-time
Dashboard to API-based metrics, WAL inspection, and agent behavior debugging.

---

## Starting the Dashboard

The Dashboard is a Next.js application that provides a real-time web UI.

### With Docker Compose

```bash
docker compose up --build
# Dashboard → http://localhost:3001
```

### Local Development

```bash
cd dashboard
npm install
npm run dev
# Dashboard → http://localhost:3001
```

The Dashboard connects to the World Engine at `http://localhost:8080` by
default. Override with the `NEXT_PUBLIC_API_URL` environment variable.

### Dashboard Sections

| Section | URL Path | What It Shows |
|---------|----------|---------------|
| **Overview** | `/` | World stats, active agents, tick progress |
| **Agents** | `/agents` | Agent list with state, tokens, lifecycle phase |
| **Tasks** | `/tasks` | Task board with real-time status updates |
| **Economy** | `/economy` | Token flow, banking, stock market |
| **Organizations** | `/orgs` | Companies, guilds, alliances |
| **Evolution** | `/evolution` | Skill trees, mutations, natural selection metrics |

---

## Real-time Event Stream (SSE)

The World Engine broadcasts all world events via Server-Sent Events at:

```
GET /api/v1/world/events
```

### Using curl

```bash
curl -N http://localhost:8080/api/v1/world/events
```

Output (one event per line):

```
data: {"type":"TickAdvanced","tick":42}

data: {"type":"AgentSpawned","agent_id":"abc-123","name":"Alice"}

data: {"type":"TaskCreated","task_id":"t-001","title":"Analyze market","reward":500}

data: {"type":"TokenBurn","agent_id":"abc-123","amount":10,"reason":"base_burn"}

data: {"type":"AgentDied","agent_id":"abc-123","cause":"token_exhaustion"}
```

### Using JavaScript (Browser / Dashboard)

```javascript
const eventSource = new EventSource('http://localhost:8080/api/v1/world/events');

eventSource.onmessage = (event) => {
  const data = JSON.parse(event.data);
  console.log(`[${data.type}]`, data);

  switch (data.type) {
    case 'TickAdvanced':
      updateTickCounter(data.tick);
      break;
    case 'AgentDied':
      handleAgentDeath(data.agent_id, data.cause);
      break;
    case 'TokenBurn':
      updateAgentTokens(data.agent_id, data.amount);
      break;
  }
};

eventSource.onerror = (err) => {
  console.error('SSE connection lost, reconnecting...', err);
};
```

### Using Python

```python
import httpx

def stream_events(base_url: str = "http://localhost:8080"):
    """Yield world events in real time."""
    with httpx.stream("GET", f"{base_url}/api/v1/world/events") as response:
        for line in response.iter_lines():
            if line.startswith("data: "):
                import json
                event = json.loads(line[6:])
                yield event

# Usage
for event in stream_events():
    print(f"[{event.get('type')}] tick={event.get('tick', '?')}")
```

---

## Key Metrics to Watch

### Agent Health

| Metric | API Endpoint | Healthy Range |
|--------|-------------|---------------|
| Token balance | `GET /api/v1/agents/{id}` | > 1000 (varies by genesis) |
| Lifecycle phase | `GET /api/v1/agents/{id}` | `BIRTH` → `CHILDHOOD` → `ADULT` → `ELDER` |
| Ticks survived | `GET /api/v1/agents/{id}` | Should increase monotonically |
| Alive count | `GET /api/v1/world/stats` | Within max_agents range |

### Economy

| Metric | API Endpoint | What to Check |
|--------|-------------|---------------|
| Total tokens in circulation | `GET /api/v1/world/stats` | Should decrease over time (burn) |
| Active tasks | `GET /tasks` | Balance of published vs claimed |
| Bank deposits | `GET /bank/stats` | Growing deposits = healthy savings |
| Stock prices | `GET /api/v1/stocks` | Price stability vs volatility |

### Task Completion

| Metric | API Endpoint | What to Check |
|--------|-------------|---------------|
| Tasks published | `GET /tasks` | Steady creation rate |
| Tasks claimed | `GET /tasks` | High claim rate = active agents |
| Tasks completed | `GET /tasks` | High completion = productive agents |
| Expired tasks | `GET /tasks` | High expiry = tasks too hard or too cheap |

---

## Using the World Engine API for Monitoring

### World Stats

```bash
curl http://localhost:8080/api/v1/world/stats
```

```json
{
  "tick": 1250,
  "agent_count": 8,
  "alive_count": 6
}
```

### List All Agents

```bash
curl http://localhost:8080/api/v1/agents
```

```json
[
  {
    "id": "abc-123",
    "name": "Alice",
    "phase": "adult",
    "tokens": 45000,
    "money": 1200,
    "alive": true,
    "ticks_survived": 1250,
    "personality": "curious,frugal"
  }
]
```

### Agent Trace (Decision Log)

The trace endpoint records what an agent did each tick:

```bash
# Get all traces for an agent
curl http://localhost:8080/api/v1/agents/abc-123/traces

# Get the latest trace
curl http://localhost:8080/api/v1/agents/abc-123/traces/latest

# Get trace for a specific tick
curl http://localhost:8080/api/v1/agents/abc-123/traces/500
```

### Snapshot History

```bash
# List all snapshots
curl http://localhost:8080/api/v1/snapshots

# Get the latest snapshot
curl http://localhost:8080/api/v1/snapshots/latest

# Export as JSON
curl http://localhost:8080/api/v1/export/snapshot > world_state.json

# Export as CSV
curl http://localhost:8080/api/v1/export/snapshot/export/csv > agents.csv
```

### Custom Queries

```bash
# Export data with custom query
curl -X POST http://localhost:8080/api/v1/export/query \
  -H "Content-Type: application/json" \
  -d '{
    "metric": "token_balance",
    "agent_id": "abc-123",
    "from_tick": 1000,
    "to_tick": 1500
  }'
```

---

## WAL Inspection

The Write-Ahead Log (WAL) records every world event in order. It's the
source of truth for auditing and debugging.

### WAL Stats

```bash
curl http://localhost:8080/wal/stats
```

```json
{
  "total_entries": 15234,
  "corrupted": 0,
  "last_event_counter": 15234,
  "wal_files": 3
}
```

### Force a Snapshot

```bash
curl -X POST http://localhost:8080/wal/snapshot
```

### Verify WAL Integrity

```bash
curl http://localhost:8080/wal/verify
```

```json
{
  "valid": true,
  "entries_verified": 15234,
  "corrupted_records": 0,
  "crc_errors": 0
}
```

::: tip
If `corrupted_records > 0`, the engine will attempt CRC-based recovery
on the next restart. Check `data/` for WAL segment files.
:::

### WAL Recovery on Startup

When the engine starts, it:

1. Opens WAL segments from the `WAL_DIR` (default: `./data/`)
2. Replays all events since the last snapshot
3. Validates CRC checksums
4. Reports recovery statistics:

```
WAL recovery: snapshot=true, replayed=234, corrupted=0
WAL: opened (15234 events recovered)
```

---

## Debugging Agent Behavior

### Common Issues and Diagnostics

#### Agent Dies Quickly (Token Exhaustion)

```bash
# Check the agent's token history
curl http://localhost:8080/api/v1/agents/{id}/traces | jq '.[] | select(.event == "TokenBurn")'

# Check base burn rate in config
# In genesis.yaml: economy.think_cost_per_token, lifecycle.death_grace_ticks
```

Possible causes:
- **High LLM usage**: Reduce `max_tokens` in LLM config
- **Too much communication**: `communicate_cost` may be too high
- **Low initial tokens**: Increase `economy.initial_tokens` in genesis.yaml

#### Agent Stuck in Loop

```bash
# Get recent traces
curl http://localhost:8080/api/v1/agents/{id}/traces/latest | jq '.actions'
```

Check if the agent repeats the same action — may need personality prompt
adjustment or survival instinct tuning.

#### Agent Not Claiming Tasks

```bash
# Check task availability
curl http://localhost:8080/tasks | jq '[.[] | select(.status == "published")]'

# Check agent capabilities vs task requirements
curl http://localhost:8080/api/v1/agents/{id} | jq '.skills'
```

#### A2A Messages Not Delivered

```bash
# Check message queue
curl http://localhost:8080/api/v1/messages | jq '[.[] | select(.to_agent == "{id}")]'

# Verify agent is registered and alive
curl http://localhost:8080/api/v1/agents/{id} | jq '{phase, alive}'
```

### Monitoring Script

Here's a complete monitoring script:

```python
#!/usr/bin/env python3
"""Agent World monitoring script — poll every 5 seconds."""

import json
import time
import httpx

BASE_URL = "http://localhost:8080"
POLL_INTERVAL = 5


def fetch(path: str) -> dict:
    resp = httpx.get(f"{BASE_URL}{path}")
    resp.raise_for_status()
    return resp.json()


def monitor():
    print("Agent World Monitor (Ctrl+C to stop)\n")

    while True:
        try:
            # World stats
            stats = fetch("/api/v1/world/stats")
            print(f"[Tick {stats['tick']}] "
                  f"Agents: {stats['alive_count']}/{stats['agent_count']} alive")

            # Per-agent status
            agents = fetch("/api/v1/agents")
            for agent in agents:
                if not agent["alive"]:
                    print(f"  ☠ {agent['name']} — DEAD ({agent['phase']})")
                elif agent["tokens"] < 5000:
                    print(f"  ⚠ {agent['name']} — LOW TOKENS ({agent['tokens']})")
                else:
                    print(f"  ✓ {agent['name']} — "
                          f"tokens={agent['tokens']}, phase={agent['phase']}")

            # Tasks
            tasks = fetch("/tasks")
            published = sum(1 for t in tasks if t["status"] == "published")
            in_progress = sum(1 for t in tasks if t["status"] == "in_progress")
            print(f"  Tasks: {published} open, {in_progress} in progress")

            print()

        except httpx.ConnectError:
            print("  ✗ World Engine unreachable")
        except Exception as e:
            print(f"  ✗ Error: {e}")

        time.sleep(POLL_INTERVAL)


if __name__ == "__main__":
    monitor()
```

---

## Next Steps

- [Deploy World Engine](/how-to/deploy-world) — Set up the engine with proper monitoring
- [Configure an Agent](/how-to/configure-agent) — Tune agents for longer survival
- [Use A2A Protocol](/how-to/a2a-protocol) — Monitor inter-agent communication
