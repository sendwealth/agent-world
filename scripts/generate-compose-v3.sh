#!/usr/bin/env bash
# ────────────────────────────────────────────────────────────────
# generate-compose-v3.sh
#
# Generates a Docker Compose v3 YAML file (docker-compose-v3.yml)
# with 100 agent services (agent-01 through agent-100), plus
# world-engine, dashboard, and ollama infrastructure services.
#
# Usage:
#   ./scripts/generate-compose-v3.sh > docker-compose-v3.yml
#
# The script reads agent names from config/agents/*.toml filenames
# and produces valid Docker Compose v3 YAML on stdout.
# ────────────────────────────────────────────────────────────────
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
AGENTS_DIR="$PROJECT_ROOT/config/agents"

# Emit the file header
cat <<'HEADER'
# ── Agent World — Docker Compose v3 (100 Agents) ────────────────
#
# One-command startup: docker compose -f docker-compose-v3.yml up -d
#
# Services:
#   world-engine   — Rust world state / economy / rules engine
#   agent-01..100  — 100 Python agent-runtime instances
#   dashboard      — Next.js observability UI
#   ollama         — Optional local LLM (only started with profile: local-llm)
#
# Environment: copy .env.example -> .env and adjust
# ────────────────────────────────────────────────────────────────

services:
  # ── World Engine (Rust) ──────────────────────────────────────
  world-engine:
    build:
      context: ./world-engine
    container_name: world-engine
    ports:
      - "${ENGINE_PORT:-8080}:${ENGINE_PORT:-8080}"
      - "${GRPC_PORT:-50051}:${GRPC_PORT:-50051}"
    volumes:
      - world-data:/app/data
      - ./config:/app/config:ro
    environment:
      - HOST=0.0.0.0
      - PORT=${ENGINE_PORT:-8080}
      - GRPC_ADDR=0.0.0.0:${GRPC_PORT:-50051}
      - RUST_LOG=${RUST_LOG:-info}
      - GENESIS_PATH=config/genesis.yaml
    restart: unless-stopped
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:${ENGINE_PORT:-8080}/tasks"]
      interval: 10s
      timeout: 5s
      retries: 5
      start_period: 30s
    networks:
      - agent-world
HEADER

# Emit 100 agent services
echo ""
echo "  # ── Agent Runtimes (Python) ──────────────────────────────────"
echo "  # 100 agents, each with a unique config file from config/agents/"
echo ""

# Iterate over config files sorted by number
for config_file in $(ls "$AGENTS_DIR"/agent-*.toml 2>/dev/null | sort -t'-' -k2 -n); do
    basename_no_ext="$(basename "$config_file" .toml)"
    # Extract the two-digit number and the agent name
    # Format: agent-NN-name
    agent_num="$(echo "$basename_no_ext" | sed 's/^agent-//; s/-[^-]*$//')"
    agent_name="$(echo "$basename_no_ext" | sed 's/^agent-[0-9]*-//')"
    service_name="agent-${agent_num}"

    cat <<AGENT_BLOCK
  ${service_name}:
    build:
      context: ./agent-runtime
    container_name: agent-${agent_name}
    depends_on:
      world-engine:
        condition: service_healthy
    volumes:
      - ./config/agents:/app/agent-configs:ro
      - agent-data:/app/data
    environment:
      - WORLD_ENGINE_URL=http://world-engine:\${ENGINE_PORT:-8080}
    command: ["spawn", "--config", "/app/agent-configs/${basename_no_ext}.toml", "--world-url", "http://world-engine:\${ENGINE_PORT:-8080}"]
    restart: unless-stopped
    networks:
      - agent-world

AGENT_BLOCK
done

# Emit dashboard service
cat <<'DASHBOARD'
  # ── Dashboard (Next.js) ──────────────────────────────────────
  dashboard:
    build:
      context: ./dashboard
      args:
        WORLD_ENGINE_URL: http://world-engine:${ENGINE_PORT:-8080}
    container_name: dashboard
    ports:
      - "${DASHBOARD_PORT:-3001}:3000"
    depends_on:
      world-engine:
        condition: service_healthy
    environment:
      - WORLD_ENGINE_URL=http://world-engine:${ENGINE_PORT:-8080}
      - PORT=3000
    restart: unless-stopped
    networks:
      - agent-world

DASHBOARD

# Emit ollama service
cat <<'OLLAMA'
  # ── Ollama (optional local LLM) ─────────────────────────────
  # Started only with: docker compose --profile local-llm up
  ollama:
    image: ollama/ollama:latest
    container_name: ollama
    ports:
      - "${OLLAMA_PORT:-11434}:11434"
    volumes:
      - ollama-data:/root/.ollama
    restart: unless-stopped
    profiles:
      - local-llm
    networks:
      - agent-world
OLLAMA

# Emit volumes and networks
cat <<'FOOTER'

volumes:
  world-data:
  agent-data:
  ollama-data:

networks:
  agent-world:
    driver: bridge
FOOTER
