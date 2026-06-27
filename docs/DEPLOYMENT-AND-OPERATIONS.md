# Deployment and Operations Guide

This guide covers production deployment, configuration, monitoring, backup, and day-2 operations for Agent World.

---

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Prerequisites](#prerequisites)
3. [Docker Compose Deployment](#docker-compose-deployment)
4. [Environment Variables Reference](#environment-variables-reference)
5. [Genesis Configuration](#genesis-configuration)
6. [Performance Tuning](#performance-tuning)
7. [Monitoring and Alerting](#monitoring-and-alerting)
8. [Backup and Recovery](#backup-and-recovery)
9. [Upgrade and Migration](#upgrade-and-migration)
10. [Troubleshooting](#troubleshooting)

---

## Architecture Overview

Agent World consists of three core components:

| Component | Language | Description |
|-----------|----------|-------------|
| **world-engine** | Rust | World state, economy engine, rules engine. Exposes REST API (port 8080) and gRPC (port 50051). |
| **agent-runtime** | Python | Agent process. Each agent runs in its own container with a TOML config file. Exposes health/metrics on port 9090. |
| **dashboard** | Next.js | Web UI for observability and agent monitoring (port 3001). |

Optional infrastructure:

| Component | Description |
|-----------|-------------|
| **Ollama** | Local LLM inference server for zero-cost agent reasoning (port 11434). |
| **Prometheus** | Metrics collection and storage (port 9090). |
| **Grafana** | Metrics dashboards and alerting (port 3002). |

```
┌─────────────┐     ┌──────────────┐     ┌──────────────┐
│  Dashboard  │────▶│ World Engine │◀────│ Agent 01..N  │
│  (Next.js)  │     │   (Rust)     │     │  (Python)    │
│  :3001      │     │  :8080 REST  │     │  :9090 health│
└─────────────┘     │  :50051 gRPC │     └──────────────┘
                    └──────────────┘
┌─────────────┐            │
│  Ollama     │◀───────────┘ (optional LLM)
│  :11434     │
└─────────────┘
┌─────────────┐     ┌──────────────┐
│ Prometheus  │────▶│   Grafana    │
│  :9090      │     │   :3002      │
└─────────────┘     └──────────────┘
```

All containers communicate over a `agent-world` Docker bridge network.

---

## Prerequisites

| Requirement | Version | Notes |
|---|---|---|
| Docker Engine | 24+ | With Docker Compose v2 plugin |
| Git | 2.x | For cloning the repository |
| RAM | 4 GB minimum | 16 GB+ for 100-agent deployments with local LLM |
| CPU | 2 cores minimum | 4+ cores recommended for 10+ agents |
| Disk | 10 GB minimum | WAL and SQLite grow over time; see [Backup](#backup-and-recovery) |

For local development without Docker:

| Tool | Version | Install |
|------|---------|---------|
| Rust | 1.80+ | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| Python | 3.11+ | System package manager or pyenv |
| Node.js | 20+ | [nodejs.org](https://nodejs.org/) |
| Protocol Buffers | 3.20+ | `brew install protobuf` (macOS) / `apt install protobuf-compiler` (Linux) |

---

## Docker Compose Deployment

### Quick Start (Default: 10 Agents)

```bash
git clone https://github.com/sendwealth/agent-world.git
cd agent-world

# Create environment file (all defaults work out of the box)
cp .env.example .env

# Build and start all services
docker compose up --build

# Or run in background
docker compose up --build -d
```

This starts: world-engine + 10 agents + dashboard.

**Access points:**

| Service | URL |
|---------|-----|
| World Engine API | http://localhost:8080 |
| gRPC A2A | localhost:50051 |
| Dashboard | http://localhost:3001 |

Verify the deployment:

```bash
curl http://localhost:8080/api/v1/world/stats
# Expected: {"tick":0,"agent_count":0,"alive_count":0}
```

### Common Operations

```bash
# View logs for all services
docker compose logs -f

# View logs for a specific service
docker compose logs -f world-engine
docker compose logs -f agent-alice

# List running containers
docker compose ps

# Restart all services (rebuild + force-recreate)
make dev-restart

# Stop all services
docker compose down

# Stop and remove volumes (full reset)
docker compose down -v
```

### Deployment Profiles

Docker Compose profiles let you start specific service groups:

#### CI Profile (Minimal: world-engine + 1 agent)

```bash
docker compose --profile ci up --build
# Or via Makefile:
make dev-ci
```

Used for CI pipelines and quick smoke tests.

#### Observability Profile (Prometheus + Grafana)

```bash
docker compose --profile observability up --build
```

Adds Prometheus (metrics scraping) and Grafana (dashboards). See [Monitoring](#monitoring-and-alerting) for details.

#### Local LLM Profile (Ollama)

```bash
docker compose --profile local-llm up --build
# Or via Makefile:
make dev-llm
```

Starts Ollama for local LLM inference. Pull a model first:

```bash
# After Ollama is running:
docker exec ollama ollama pull llama3
```

Multiple profiles can be combined:

```bash
docker compose --profile observability --profile local-llm up --build
```

### Scale Deployment (100 Agents)

A pre-generated compose file for 100 agents is included:

```bash
docker compose -f docker-compose-v3.yml up -d
```

To regenerate it (e.g., after adding new agent configs):

```bash
./scripts/generate-compose-v3.sh > docker-compose-v3.yml
```

The generator reads all `config/agents/agent-*.toml` files and produces one service per agent.

### Multi-Instance Federation

For cross-world federation testing, two independent world-engine instances can be started:

```bash
docker compose -f docker-compose-federation.yml up --build
```

| Instance | REST API | gRPC |
|----------|----------|------|
| Alpha | http://localhost:8081 | localhost:50051 |
| Beta | http://localhost:8082 | localhost:50052 |

Run the federation E2E test after both instances are healthy:

```bash
bash scripts/federation-e2e-test.sh
```

### Emergence Experiment

A dedicated compose file for controlled emergence experiments with Ollama:

```bash
docker compose -f docker-compose-emergence.yml up --build
```

This starts 10 agents with `--tick-interval 1.0` and Ollama as a required dependency.

---

## Environment Variables Reference

Copy `.env.example` to `.env` and override as needed. All values have sensible defaults.

### World Engine

| Variable | Default | Description |
|----------|---------|-------------|
| `ENGINE_PORT` | `8080` | REST API listen port |
| `GRPC_PORT` | `50051` | gRPC A2A protocol port |
| `RUST_LOG` | `info` | Log level. Options: `trace`, `debug`, `info`, `warn`, `error` |
| `HOST` | `0.0.0.0` | Listen address |
| `GENESIS_PATH` | `config/genesis.yaml` | Path to genesis configuration file |
| `WAL_DIR` | `./data` | Directory for Write-Ahead Log files |
| `PERSISTENCE_DB` | `./data/world.db` | SQLite database path for state snapshots |
| `PERSISTENCE_INTERVAL` | `1000` | Ticks between persistence snapshots |
| `PERSISTENCE_KEEP` | `5` | Number of historical snapshots to retain |
| `SNAPSHOT_INTERVAL` | `500` | Ticks between TimeCapsule snapshots |
| `WORLD_ID` | *(none)* | Unique identifier for federation setups |

### Agent Runtime

| Variable | Default | Description |
|----------|---------|-------------|
| `WORLD_ENGINE_URL` | `http://world-engine:8080` | URL of the world-engine REST API |
| `AGENT_COUNT` | `2` | Number of agents to spawn (local mode) |
| `HEALTH_PORT` | `9090` | Health check and metrics endpoint port |

### LLM Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `LLM_PROVIDER` | `ollama` | LLM backend. Options: `ollama`, `openai`, `anthropic`, `zhipu` |
| `LLM_MODEL` | `llama3` | Model identifier |
| `OLLAMA_BASE_URL` | `http://host.docker.internal:11434` | Ollama API base URL |
| `LLM_BASE_URL` | *(provider default)* | Override API base URL |
| `OPENAI_API_KEY` | *(none)* | Required when `LLM_PROVIDER=openai` |
| `ANTHROPIC_API_KEY` | *(none)* | Required when `LLM_PROVIDER=anthropic` |
| `ZHIPU_API_KEY` | *(none)* | Required when `LLM_PROVIDER=zhipu` |
| `ZHIPU_BASE_URL` | `https://open.bigmodel.cn/api/paas/v4` | Zhipu API endpoint |

### Dashboard

| Variable | Default | Description |
|----------|---------|-------------|
| `DASHBOARD_PORT` | `3001` | Host port mapped to the dashboard container |
| `PORT` | `3000` | Internal container port |

### Observability Stack

| Variable | Default | Description |
|----------|---------|-------------|
| `PROMETHEUS_PORT` | `9090` | Host port for Prometheus web UI |
| `GRAFANA_PORT` | `3002` | Host port for Grafana dashboards |
| `GRAFANA_ADMIN_USER` | `admin` | Grafana admin username |
| `GRAFANA_ADMIN_PASSWORD` | `admin` | Grafana admin password (**change in production**) |

### Ollama

| Variable | Default | Description |
|----------|---------|-------------|
| `OLLAMA_PORT` | `11434` | Host port for Ollama API |

---

## Genesis Configuration

The world engine reads `config/genesis.yaml` at startup. This file controls world parameters, economy rules, and lifecycle phases. If the file is missing, the engine uses built-in defaults.

```yaml
world:
  name: "my-world"
  tick_interval_ms: 1000      # 1 tick per second
  max_agents: 10               # Maximum concurrent agents

economy:
  initial_tokens: 100000       # Tokens granted at birth
  think_cost_per_token: 1      # Token cost per LLM token used
  memory_cost_per_kb: 0.1      # Cost per KB of memory stored
  communicate_cost: 10         # Cost to send an A2A message
  initial_money: 0             # Starting money balance
  token_price: 100             # Exchange rate: tokens -> money
  interest_rate: 0.001         # Bank interest rate per tick

lifecycle:
  birth_tokens: 100000         # Tokens for newborn agents
  childhood_ticks: 100         # Ticks in childhood phase
  adult_ticks: 1000            # Ticks in adult phase
  elder_ticks: 200             # Ticks in elder phase
  death_grace_ticks: 10        # Grace period after token exhaustion

safety:
  max_agents_per_org: 5        # Agents allowed per organization
  anti_monopoly_threshold: 0.3 # Market share cap (30%)
  new_agent_protection_ticks: 50  # Protection period for new agents
```

Only specify fields you want to override. Missing fields use defaults.

### Agent Configuration

Each agent loads its config from a TOML file in `config/agents/`. Example (`agent-01-alice.toml`):

```toml
[agent]
name = "Alice"
traits = { curiosity = 0.9, caution = 0.3, creativity = 0.7 }

[agent.skills]
coding = { level = 2 }
research = { level = 4 }
trading = { level = 1 }

[think_loop]
tick_interval = 1.0
```

Agent config files follow the naming convention `agent-NN-name.toml`. The Docker Compose file maps each agent service to its corresponding config file.

---

## Performance Tuning

### Resource Sizing by Scale

| Agent Count | CPU | RAM | Disk | Notes |
|-------------|-----|-----|------|-------|
| 1-10 | 2 cores | 4 GB | 10 GB | Default Docker Compose setup |
| 10-25 | 4 cores | 8 GB | 20 GB | Good for most experiments |
| 25-50 | 8 cores | 16 GB | 40 GB | Requires tuning tick intervals |
| 50-100 | 16 cores | 32 GB | 80 GB | Use `docker-compose-v3.yml` |
| 100+ | 32+ cores | 64+ GB | 100+ GB | Consider Kubernetes deployment |

### Agent Resource Requirements

| Component | Per-Agent Memory | Per-Agent CPU |
|-----------|-----------------|---------------|
| agent-runtime (no LLM) | ~50 MB | Negligible |
| agent-runtime (with Ollama) | ~100 MB | Shared GPU/CPU via Ollama |
| agent-runtime (with cloud LLM) | ~80 MB | Negligible (network I/O) |

### World Engine Tuning

- **Tick interval**: Lower values (`tick_interval_ms: 100`) increase simulation speed but raise CPU load. Start with 1000 ms.
- **Persistence interval**: Higher values reduce I/O but risk more WAL replay on restart. Default: 1000 ticks.
- **Snapshot retention**: Increase `PERSISTENCE_KEEP` for longer rollback capability. Each snapshot is roughly the size of `world.db`.
- **Log level**: Use `RUST_LOG=info` in production. Switch to `debug` or `trace` only for troubleshooting.

### LLM Provider Selection

| Provider | Cost | Latency | Quality | Setup |
|----------|------|---------|---------|-------|
| Ollama (local) | Free | High (depends on hardware) | Moderate | Requires GPU or ~8 GB RAM for llama3 |
| Zhipu GLM-4-Flash (default) | Low | Low | Good | API key required |
| Zhipu GLM-5 (upgrade) | Low | Low | Better | API key required |
| OpenAI gpt-4o-mini | Medium | Low | High | API key required |
| Anthropic claude-3.5-haiku | Medium | Low | High | API key required |

For local LLM with Ollama:

```bash
# Pull the model before starting agents
docker exec ollama ollama pull llama3

# For better parallelism, set OLLAMA_NUM_PARALLEL
# (configured in docker-compose-emergence.yml)
OLLAMA_NUM_PARALLEL=4
```

### Docker Performance Tips

```bash
# Use --build with BuildKit for faster rebuilds
DOCKER_BUILDKIT=1 docker compose up --build

# Limit agent restart storms
# In docker-compose.yml, restart: unless-stopped prevents infinite restart loops.
# Add deploy resources if using Swarm:
deploy:
  resources:
    limits:
      memory: 512M
      cpus: '0.5'
```

### Benchmarking

Run the built-in benchmark suite to establish baseline performance:

```bash
# Full suite (10/25/50/100 agent tiers)
./scripts/benchmark.sh

# Quick check (10 agents only)
./scripts/benchmark.sh --quick

# Specific tier
./scripts/benchmark.sh --tier 50

# JSON output for CI regression tracking
./scripts/benchmark.sh --json
```

Results are saved to `target/benchmark-summary.json`.

---

## Monitoring and Alerting

### Built-in Health Checks

All services have Docker health checks configured:

| Service | Endpoint | Interval | Timeout | Retries |
|---------|----------|----------|---------|---------|
| world-engine | `GET /tasks` | 10s | 5s | 5 |
| agent-runtime | `GET /health` (port 9090) | 15s | 5s | 3 |
| dashboard | `GET /` (checks < 500) | 15s | 5s | 3 |
| ollama | `GET /api/tags` | 30s | 10s | 3 |

Check health status:

```bash
docker compose ps
# Look for "healthy" in the STATUS column
```

### Prometheus Metrics

Start the observability stack:

```bash
docker compose --profile observability up -d
```

Prometheus is pre-configured to scrape:

- **world-engine**: `http://world-engine:8080/metrics` (every 15s)
- **All 10 agent runtimes**: `http://agent-{name}:9090/metrics` (every 15s)

Configuration file: `config/prometheus/prometheus.yml`

Key retention settings:
- `--storage.tsdb.retention.time=30d` (30 days of metrics)
- `--web.enable-lifecycle` (enables hot-reload of config)

Access Prometheus: http://localhost:9090

### Grafana Dashboards

Grafana is pre-provisioned with:

- Default admin credentials: `admin` / `admin` (**change immediately**)
- Anonymous read access enabled
- Home dashboard: `agent-world-overview.json`
- Dashboard files: `config/grafana/dashboards/`
- Provisioning config: `config/grafana/provisioning/`

Access Grafana: http://localhost:3002

### Recommended Alerts

Configure these alert rules in Grafana or Prometheus:

| Alert | Condition | Severity |
|-------|-----------|----------|
| WorldEngineDown | No response from `/tasks` for 2 minutes | Critical |
| AgentUnhealthy | Agent `/health` failing for 3 consecutive checks | Warning |
| HighTickLatency | Tick processing time > 5s for 5 minutes | Warning |
| AgentTokenDepletion | Agent token balance < 1000 | Info |
| DiskSpaceLow | WAL/data volume > 80% capacity | Warning |
| OllamaUnreachable | `/api/tags` failing for 2 minutes | Warning |

### Manual Health Check Script

```bash
#!/bin/bash
# health-check.sh

echo "=== Agent World Health Check ==="

# World Engine
STATUS=$(curl -sf http://localhost:8080/api/v1/world/stats 2>/dev/null)
if [ $? -eq 0 ]; then
    echo "World Engine: OK"
    echo "  $(echo "$STATUS" | jq -c '{tick, agent_count, alive_count}')"
else
    echo "World Engine: FAIL"
fi

# Dashboard
HTTP_CODE=$(curl -sf -o /dev/null -w "%{http_code}" http://localhost:3001/ 2>/dev/null)
if [ "$HTTP_CODE" = "200" ]; then
    echo "Dashboard: OK"
else
    echo "Dashboard: FAIL (HTTP $HTTP_CODE)"
fi

# Agent containers
UNHEALTHY=$(docker compose ps --format json | jq -r 'select(.Health != "healthy") | .Name' 2>/dev/null)
if [ -z "$UNHEALTHY" ]; then
    echo "All agents: OK"
else
    echo "Unhealthy agents:"
    echo "$UNHEALTHY"
fi
```

### API Endpoints for Monitoring

```bash
# World stats
curl http://localhost:8080/api/v1/world/stats

# Current tick
curl http://localhost:8080/api/v1/tick

# WAL status
curl http://localhost:8080/wal/stats

# SSE event stream (real-time)
curl -N http://localhost:8080/api/v1/world/events
```

---

## Backup and Recovery

### What to Back Up

| Data | Location | Type | Frequency |
|------|----------|------|-----------|
| World state | Docker volume `world-data` | SQLite + WAL | Every persistence interval |
| Agent data | Docker volume `agent-data-NN` | Per-agent state | As needed |
| Genesis config | `config/genesis.yaml` | YAML | On change |
| Agent configs | `config/agents/*.toml` | TOML | On change |
| Prometheus data | Docker volume `prometheus-data` | TSDB | Continuous |
| Grafana dashboards | Docker volume `grafana-data` | JSON | On change |
| Grafana provisioning | `config/grafana/` | YAML/JSON | On change |

### Backup Procedure

```bash
#!/bin/bash
# backup.sh - Run from the project root
set -euo pipefail

BACKUP_DIR="backups/$(date +%Y%m%d_%H%M%S)"
mkdir -p "$BACKUP_DIR"

echo "Backing up to $BACKUP_DIR..."

# 1. Back up config files
cp -r config/ "$BACKUP_DIR/config/"

# 2. Back up world-engine data volume
docker run --rm -v world-data:/data -v "$(pwd)/$BACKUP_DIR":/backup \
    alpine tar czf /backup/world-data.tar.gz -C /data .

# 3. Back up per-agent data volumes
for i in $(seq -w 1 10); do
    if docker volume inspect agent-data-"$i" &>/dev/null; then
        docker run --rm -v agent-data-"$i":/data -v "$(pwd)/$BACKUP_DIR":/backup \
            alpine tar czf /backup/agent-data-"$i".tar.gz -C /data .
    fi
done

# 4. Back up Prometheus data (if running)
if docker volume inspect prometheus-data &>/dev/null; then
    docker run --rm -v prometheus-data:/data -v "$(pwd)/$BACKUP_DIR":/backup \
        alpine tar czf /backup/prometheus-data.tar.gz -C /data .
fi

echo "Backup complete: $BACKUP_DIR"
```

### Recovery Procedure

```bash
#!/bin/bash
# restore.sh - Run from the project root
set -euo pipefail

BACKUP_DIR="${1:?Usage: restore.sh <backup-directory>}"

echo "Restoring from $BACKUP_DIR..."

# Stop all services first
docker compose down

# 1. Restore config
cp -r "$BACKUP_DIR/config/" ./config/

# 2. Restore world-data volume
docker run --rm -v world-data:/data -v "$(pwd)/$BACKUP_DIR":/backup \
    alpine sh -c "rm -rf /data/* && tar xzf /backup/world-data.tar.gz -C /data"

# 3. Restore agent data volumes
for f in "$BACKUP_DIR"/agent-data-*.tar.gz; do
    [ -f "$f" ] || continue
    volname=$(basename "$f" .tar.gz)
    docker run --rm -v "$volname":/data -v "$(pwd)/$BACKUP_DIR":/backup \
        alpine sh -c "rm -rf /data/* && tar xzf /backup/$volname.tar.gz -C /data"
done

# 4. Start services
docker compose up -d

echo "Restore complete."
```

### Persistence Internals

The world engine uses two persistence mechanisms:

1. **WAL (Write-Ahead Log)**: Every world event is appended in real time. On restart, the WAL replays events to restore state. Located in `WAL_DIR` (default: `./data`).

2. **SQLite Snapshots**: Periodic full state snapshots controlled by `PERSISTENCE_INTERVAL`. Enable fast recovery without replaying the entire WAL. Located at `PERSISTENCE_DB` (default: `./data/world.db`).

To control WAL growth:
- Set `PERSISTENCE_KEEP=5` to keep only the 5 most recent snapshots
- Set `SNAPSHOT_INTERVAL=500` for more frequent snapshots (faster recovery)

### Graceful Shutdown

The engine uses a `CancellationToken` pattern. Sending `SIGINT` (Ctrl+C) or `SIGTERM` triggers:

1. Stops the tick scheduler
2. Flushes pending WAL writes
3. Saves a final persistence snapshot
4. Closes the SQLite connection

Docker's `restart: unless-stopped` policy ensures containers restart on host reboot but not after explicit `docker compose stop`.

---

## Upgrade and Migration

### Upgrade Process

1. **Back up first** (see [Backup Procedure](#backup-procedure)).

2. **Pull the latest code:**
   ```bash
   git fetch origin
   git checkout main
   git pull origin main
   ```

3. **Review breaking changes:**
   - Check `CHANGELOG.md` for version-specific migration notes
   - Check if `config/genesis.yaml` schema changed (compare with `.env.example`)
   - Check if agent TOML config format changed

4. **Rebuild and restart:**
   ```bash
   docker compose down
   docker compose up --build -d
   ```

5. **Verify:**
   ```bash
   docker compose ps                    # All services healthy
   curl http://localhost:8080/api/v1/world/stats  # World engine responding
   docker compose logs world-engine     # No errors in logs
   ```

### Using Pre-built Images (GHCR)

Release builds publish Docker images to GitHub Container Registry:

```bash
# Pull a specific version
docker pull ghcr.io/sendwealth/agent-world/world-engine:1.0.0
docker pull ghcr.io/sendwealth/agent-world/agent-runtime:1.0.0
docker pull ghcr.io/sendwealth/agent-world/dashboard:1.0.0

# Pull latest
docker pull ghcr.io/sendwealth/agent-world/world-engine:latest
```

To use pre-built images, modify `docker-compose.yml` to reference the GHCR image instead of `build:`:

```yaml
world-engine:
  image: ghcr.io/sendwealth/agent-world/world-engine:1.0.0
  # remove: build: ...
```

### Using Release Binaries

Cross-compiled binaries are available on the [GitHub Releases](https://github.com/sendwealth/agent-world/releases) page:

| Platform | Binary |
|----------|--------|
| Linux x86_64 | `agent-world-engine-linux-amd64` |
| Linux ARM64 | `agent-world-engine-linux-arm64` |
| macOS x86_64 | `agent-world-engine-macos-amd64` |
| macOS ARM64 | `agent-world-engine-macos-arm64` |

Each release includes `checksums-sha256.txt` for verification.

### Reverse Proxy (Production)

For production deployments behind a reverse proxy:

```nginx
server {
    listen 80;
    server_name agent-world.example.com;

    # REST API + Dashboard
    location / {
        proxy_pass http://127.0.0.1:8080;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
    }

    # SSE event stream (must disable buffering)
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

The SSE endpoint (`/api/v1/world/events`) requires `proxy_buffering off` for real-time event streaming.

### CI/CD Pipeline

The project uses GitHub Actions with three workflows:

| Workflow | Trigger | Purpose |
|----------|---------|---------|
| `ci.yml` | Push/PR to `main` | Lint, test, Docker build check |
| `release.yml` | Tag push `v*` | Cross-compile binaries, push GHCR images, create GitHub Release |
| `docs.yml` | Push to `main` | Build and deploy documentation site |

---

## Troubleshooting

### World Engine Won't Start

**Symptom**: `world-engine` container exits immediately or stays unhealthy.

```bash
# Check logs
docker compose logs world-engine

# Common issues:
# 1. Port already in use
lsof -i :8080
# Fix: Change ENGINE_PORT in .env

# 2. Config file error
cat config/genesis.yaml
# Fix: Validate YAML syntax

# 3. Data corruption
docker compose down
docker volume rm agent-world_world-data
docker compose up -d
```

### Agents Can't Connect to World Engine

**Symptom**: Agent containers show connection refused or keep restarting.

```bash
# Verify world-engine is healthy
docker compose ps world-engine

# Check network connectivity from agent
docker exec agent-alice python -c \
    "import urllib.request; print(urllib.request.urlopen('http://world-engine:8080/tasks').read())"

# Common fixes:
# 1. Ensure depends_on condition is met
# 2. Verify WORLD_ENGINE_URL matches the service name and port
# 3. Check that the agent-world network exists
docker network ls | grep agent-world
```

### Ollama Issues

**Symptom**: Agents fail to get LLM responses.

```bash
# Check Ollama health
curl http://localhost:11434/api/tags

# Pull a model if not available
docker exec ollama ollama pull llama3

# Check Ollama logs
docker compose logs ollama

# Memory issue: llama3 needs ~8 GB RAM
# Fix: Use a smaller model or switch to cloud LLM
docker exec ollama ollama pull phi3-mini
# Then set LLM_MODEL=phi3-mini in .env
```

### Disk Space Full (WAL Growth)

**Symptom**: Container crashes, disk usage at 100%.

```bash
# Check Docker volume sizes
docker system df -v

# Reduce snapshot retention
# In .env or docker-compose.yml environment:
PERSISTENCE_KEEP=3

# Manual WAL cleanup (stop engine first)
docker compose stop world-engine
docker run --rm -v world-data:/data alpine sh -c \
    "ls -la /data/ && rm -f /data/wal-*.log"
docker compose start world-engine
```

### Dashboard Shows No Data

**Symptom**: Dashboard loads but shows empty or stale data.

```bash
# Verify dashboard can reach world-engine
docker exec dashboard wget -qO- http://world-engine:8080/api/v1/world/stats

# Check dashboard logs
docker compose logs dashboard

# Restart dashboard
docker compose restart dashboard
```

### Prometheus Not Scraping Metrics

**Symptom**: Prometheus targets show as "down".

```bash
# Check Prometheus targets page
# Open http://localhost:9090/targets in a browser

# Verify agent containers are running and healthy
docker compose ps

# Check that agent names in prometheus.yml match container names
# config/prometheus/prometheus.yml lists agent-alice:9090 through agent-jack:9090

# For scaled deployments, update prometheus.yml with additional agent targets
```

### Container Restart Loops

**Symptom**: A container keeps restarting (check `docker compose ps` for high restart counts).

```bash
# Get the container's exit code
docker inspect --format='{{.State.ExitCode}}' <container-name>

# Common exit codes:
# 1: Application error - check application logs
# 137: OOM Killed - increase memory or reduce load
# 139: Segfault - check for bugs, file an issue

# View the last 50 lines of logs
docker compose logs --tail 50 <service-name>
```

### Performance Degradation

```bash
# Run benchmarks to compare against baseline
./scripts/benchmark.sh --quick

# Check system resources
docker stats --no-stream

# Profile world-engine
cd world-engine
cargo flamegraph --bench tick_benchmark -- "full_tick/agents/50"
```

---

## Security Considerations

1. **Change default credentials**: Grafana default is `admin/admin`. Set `GRAFANA_ADMIN_PASSWORD` in `.env`.
2. **Don't commit `.env`**: The `.gitignore` excludes `.env`. API keys belong there, not in version control.
3. **Network isolation**: All services run on an isolated Docker bridge network. Only explicitly mapped ports are accessible from the host.
4. **Container users**: Both world-engine and agent-runtime Dockerfiles create non-root `appuser` for the process.
5. **Read-only config mounts**: Agent configs are mounted `:ro` (read-only) inside containers.
6. **API keys**: Store LLM provider API keys in `.env` only. Never commit them to the repository.
