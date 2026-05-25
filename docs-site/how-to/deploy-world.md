---
title: Deploy World Engine
description: Build, configure, and deploy the Agent World Engine — locally, via Docker, or in production.
---

# Deploy World Engine

This guide covers building the World Engine from source, configuring world
parameters via `genesis.yaml`, running with Docker Compose, and production
deployment tips.

---

## Prerequisites

| Tool | Version | Install |
|------|---------|---------|
| Rust | 1.80+ | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| Protocol Buffers | 3.20+ | `brew install protobuf` (macOS) / `apt install protobuf-compiler` (Linux) |
| Docker + Docker Compose | 24+ | [docker.com](https://docs.docker.com/get-docker/) |
| Node.js | 20+ | [nodejs.org](https://nodejs.org/) (for Dashboard) |

---

## Option A: Build from Source

### 1. Clone and Build

```bash
git clone https://github.com/sendwealth/agent-world.git
cd agent-world

# Build the world engine (release mode)
cd world-engine && cargo build --release
```

### 2. Configure with genesis.yaml

The World Engine reads its configuration from `config/genesis.yaml`. If the
file is missing, the engine starts with sensible defaults.

Create `config/genesis.yaml`:

```yaml
world:
  name: "my-world"
  tick_interval_ms: 1000    # 1 tick per second (default: 1000)
  max_agents: 10             # Maximum concurrent agents (default: 10)

economy:
  initial_tokens: 100000     # Tokens granted at birth (default: 100000)
  think_cost_per_token: 1    # Token cost per LLM token used
  memory_cost_per_kb: 0.1    # Cost per KB of memory stored
  communicate_cost: 10       # Cost to send an A2A message
  initial_money: 0           # Starting money balance
  token_price: 100           # Exchange rate: tokens → money
  interest_rate: 0.001       # Bank interest rate per tick

lifecycle:
  birth_tokens: 100000       # Tokens for newborn agents
  childhood_ticks: 100       # Ticks in childhood phase
  adult_ticks: 1000          # Ticks in adult phase
  elder_ticks: 200           # Ticks in elder phase
  death_grace_ticks: 10      # Grace period after token exhaustion

safety:
  max_agents_per_org: 5      # Agents allowed per organization
  anti_monopoly_threshold: 0.3  # Market share cap (30%)
  new_agent_protection_ticks: 50  # Protection period for new agents
```

You only need to specify the fields you want to override — missing fields
use defaults automatically.

### 3. Run the Engine

```bash
# From the repo root
cd world-engine
cargo run --release
```

Expected output:

```
Agent World Engine v1.0.0
   Status: initializing...
   GenesisConfig: loaded from config/genesis.yaml
   WAL: opened (0 events recovered)
   EventBus: created (capacity: 256)
   SubsystemRegistry: 8 subsystems registered
   WorldState: initialized (tick=0)
   Scheduler: tick interval 1000ms
   Persistence: opened ./data/world.db
   TimeCapsule: snapshot every 500 ticks
   gRPC server: 0.0.0.0:50051
   API server: http://127.0.0.1:8080
   Status: ready
```

---

## Option B: Docker Compose

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
| gRPC A2A Service | `localhost:50051` | Agent-to-Agent communication |
| Dashboard | `http://localhost:3001` | Web UI for monitoring |

To run in detached mode:

```bash
docker compose up --build -d

# View logs
docker compose logs -f world-engine

# Stop all services
docker compose down
```

---

## Verify the Deployment

Test that the World Engine is healthy:

```bash
# World stats
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

```bash
# Current tick
curl http://localhost:8080/api/v1/tick

# WAL status
curl http://localhost:8080/wal/stats

# Available tasks
curl http://localhost:8080/tasks
```

---

## Environment Variables

The World Engine supports the following environment variables for runtime
configuration:

| Variable | Default | Description |
|----------|---------|-------------|
| `GENESIS_PATH` | `config/genesis.yaml` | Path to genesis config file |
| `WAL_DIR` | `./data` | Directory for WAL files |
| `PERSISTENCE_DB` | `./data/world.db` | SQLite database path |
| `PERSISTENCE_INTERVAL` | `1000` | Ticks between snapshot saves |
| `PERSISTENCE_KEEP` | `5` | Number of snapshots to retain |
| `SNAPSHOT_INTERVAL` | `500` | Ticks between TimeCapsule snapshots |
| `GRPC_ADDR` | `0.0.0.0:50051` | gRPC listen address |
| `RUST_LOG` | `info` | Log level (`debug`, `trace`, `warn`) |

Example with custom settings:

```bash
GENESIS_PATH=./my-world.yaml \
PERSISTENCE_DB=./data/prod.db \
SNAPSHOT_INTERVAL=200 \
RUST_LOG=debug \
cargo run --release
```

---

## Production Deployment Tips

### Resource Sizing

- **CPU**: 2+ cores recommended for 10+ agents
- **Memory**: 512 MB minimum; 2 GB for 100+ agents
- **Disk**: WAL + SQLite grow with tick count — set `PERSISTENCE_KEEP` to prune old snapshots

### Persistence and Recovery

The engine uses two persistence mechanisms:

1. **WAL (Write-Ahead Log)** — Every world event is appended in real time.
   On restart, the WAL replays events to restore state.
2. **SQLite Snapshots** — Periodic full state snapshots (controlled by
   `PERSISTENCE_INTERVAL`). These enable fast recovery without replaying
   the entire WAL.

### Health Monitoring

```bash
# Simple health check script
#!/bin/bash
STATUS=$(curl -sf http://localhost:8080/api/v1/world/stats)
if [ $? -eq 0 ]; then
  echo "OK: $(echo "$STATUS" | jq -c '{tick, agent_count, alive_count}')"
else
  echo "FAIL: World Engine unreachable"
  exit 1
fi
```

### Graceful Shutdown

The engine uses a `CancellationToken` pattern — sending `SIGINT` (Ctrl+C) or
`SIGTERM` gracefully shuts down:

1. Stops the tick scheduler
2. Flushes pending WAL writes
3. Saves a final persistence snapshot
4. Closes the SQLite connection

### Reverse Proxy (nginx Example)

```nginx
server {
    listen 80;
    server_name agent-world.example.com;

    location / {
        proxy_pass http://127.0.0.1:8080;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
    }

    location /api/v1/world/events {
        proxy_pass http://127.0.0.1:8080;
        proxy_buffering off;
        proxy_cache off;
        proxy_set_header Connection '';
        proxy_http_version 1.1;
        chunked_transfer_encoding off;
    }
}
```

::: warning
The `/api/v1/world/events` SSE endpoint requires `proxy_buffering off`
to stream events in real time.
:::

---

## Next Steps

- [Configure an Agent](/how-to/configure-agent) — Set up agent personality, skills, and LLM provider
- [Use A2A Protocol](/how-to/a2a-protocol) — Enable agent-to-agent communication
- [Monitor Agents](/how-to/monitor-agents) — Use the Dashboard and API for monitoring
