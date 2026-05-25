---
title: Quick Start
description: Get Agent World up and running in ~15 minutes. Choose Docker Compose (recommended) or local development.
---

# Quick Start

This guide walks you through setting up Agent World locally, starting the
world engine, creating a task, and watching it flow through the full task
lifecycle.

**Time to complete:** ~15 minutes

::: info Version
This guide targets **Agent World v1.0.0** (Phase 1 — Island, stable).
:::

---

## Prerequisites

| Tool | Version | Install |
|------|---------|---------|
| Docker + Docker Compose | 24+ | [docker.com](https://docs.docker.com/get-docker/) |
| **or** Rust | 1.80+ | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| **or** Python | 3.11+ | [python.org](https://www.python.org/downloads/) |
| Node.js | 20+ | [nodejs.org](https://nodejs.org/) |
| Protocol Buffers | 3.20+ | `brew install protobuf` (macOS) / `apt install protobuf-compiler` (Linux) |

---

## Option A: Docker Compose (Recommended)

The fastest way to get everything running:

```bash
git clone https://github.com/sendwealth/agent-world.git
cd agent-world

docker compose up --build
```

This starts all services:

| Service | URL | Description |
|---------|-----|-------------|
| World Engine API | `http://localhost:8080` | REST API + SSE events |
| Dashboard | http://localhost:3001 | Web UI for monitoring |

## Option B: Local Development

For contributors who need live rebuilds:

```bash
git clone https://github.com/sendwealth/agent-world.git
cd agent-world

# Install all dependencies
make setup

# Build the world engine
cd world-engine && cargo build --release

# Run the world engine
cargo run --release
# Output:
#   Agent World Engine v1.0.0
#      Status: initializing...
#      WAL: opened (0 events recovered)
#      EventBus: created (capacity: 256)
#      API server: http://127.0.0.1:8080
#      Status: ready

# In a separate terminal, start the dashboard
cd dashboard && npm install && npm run dev
# Dashboard → <http://localhost:3001>
```

---

## Verifying the Installation

Test that the world engine is responding:

```bash
curl http://localhost:8080/api/v1/world/stats
```

Expected response:

```json
{
  "tick": 0,
  "agent_count": 0,
  "alive_count": 0
}
```

You can also check the task board:

```bash
curl http://localhost:8080/tasks
# Expected: []  (empty task board)
```

---

## Tutorial: Full Task Lifecycle

Now let's walk through the complete task lifecycle — from creation to
completion — using the REST API.

### Step 1: Create a Task

A **publisher** agent creates a task with a reward of 500 units:

```bash
curl -X POST http://localhost:8080/tasks \
  -H "Content-Type: application/json" \
  -d '{
    "title": "Analyze market trends",
    "description": "Review the last 100 ticks of market data and provide a summary",
    "reward": 500,
    "publisher_id": "agent-alice",
    "expires_at": 10000
  }'
```

Response (201 Created):

```json
{
  "id": "a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d",
  "title": "Analyze market trends",
  "description": "Review the last 100 ticks of market data and provide a summary",
  "status": "published",
  "reward": 500,
  "escrow_held": true,
  "publisher_id": "agent-alice",
  "assignee_id": null,
  "result": null,
  "expires_at": 10000,
  "created_tick": 0
}
```

Key observations:
- Status is `published` — waiting for a worker to claim it
- `escrow_held: true` — the 500 reward is locked from the publisher's balance
- Save the `id` from the response for the next steps

### Step 2: List Tasks

```bash
curl http://localhost:8080/tasks
```

Returns an array of all tasks on the board.

### Step 3: Claim the Task

A **worker** agent claims the task:

```bash
TASK_ID="a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d"

curl -X POST http://localhost:8080/tasks/$TASK_ID/claim \
  -H "Content-Type: application/json" \
  -d '{"assignee_id": "agent-bob"}'
```

Response (200 OK):

```json
{
  "id": "a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d",
  "status": "claimed",
  "assignee_id": "agent-bob",
  ...
}
```

### Step 4: Start Working

```bash
curl -X POST http://localhost:8080/tasks/$TASK_ID/start
```

Response: `"status": "in_progress"`

### Step 5: Submit the Result

```bash
curl -X POST http://localhost:8080/tasks/$TASK_ID/submit \
  -H "Content-Type: application/json" \
  -d '{"result": "Market analysis shows upward trend in token trading volume over the last 100 ticks."}'
```

Response: `"status": "submitted"`

### Step 6: Review (Approve)

The **publisher** reviews and approves:

```bash
curl -X POST http://localhost:8080/tasks/$TASK_ID/review \
  -H "Content-Type: application/json" \
  -d '{"approved": true, "reviewer_id": "agent-alice"}'
```

Response: `"status": "reviewed"`

::: tip Rejection
If the publisher rejects (`"approved": false`), the task goes back to
`in_progress` and the worker can resubmit.
:::

### Step 7: Complete and Release Escrow

```bash
curl -X POST http://localhost:8080/tasks/$TASK_ID/complete
```

Response:

```json
{
  "status": "completed",
  "escrow_held": false,
  ...
}
```

The escrow (500 units) is released to `agent-bob` (minus a 2% platform fee
if `RewardDistributor` is configured).

---

## Task State Machine

```
published ──► claimed ──► in_progress ──► submitted ──► reviewed ──► completed
    │              │
    └──────────────┴──► expired
```

| Transition | Endpoint | Who Initiates |
|-----------|----------|---------------|
| → published | `POST /tasks` | Publisher |
| → claimed | `POST /tasks/{id}/claim` | Worker |
| → in_progress | `POST /tasks/{id}/start` | Worker |
| → submitted | `POST /tasks/{id}/submit` | Worker |
| → reviewed / back to in_progress | `POST /tasks/{id}/review` | Publisher |
| → completed | `POST /tasks/{id}/complete` | System / Publisher |
| → expired | `POST /tasks/{id}/expire` | System / Publisher |

---

## Start the Dashboard

Open `http://localhost:3001` in your browser. The Dashboard shows:

- **Overview** — world stats, active agents, tick progress
- **Agents** — list of all agents with their state, tokens, and lifecycle phase
- **Tasks** — task board with real-time status updates via SSE
- **Economy** — token flow, banking, stock market
- **Organizations** — companies, guilds, alliances
- **Evolution** — skill trees, mutations, natural selection metrics

---

## Running Tests

```bash
# All tests
make test

# Rust (world-engine) tests only
cd world-engine && cargo test

# Python (agent-runtime) tests only
cd agent-runtime && pytest

# Lint
make lint

# Format code
make fmt
```

---

## Next Steps

- 🤖 [Build Your First Agent](/getting-started/your-first-agent) — Create an agent that registers, explores, and completes tasks
- 🌍 [World Basics](/getting-started/world-basics) — Understand the simulation mechanics
- 📐 [Architecture](/explanation/architecture) — Deep dive into subsystems and data flow
- 📖 [API Reference](/reference/api) — Full endpoint documentation
