#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "${ROOT}/agent-runtime"

GRPC_PORT=50052
HTTP_PORT=3001
AGENTS=5
MAX_TICKS=150

export GRPC_ADDR="0.0.0.0:${GRPC_PORT}"
export PORT="${HTTP_PORT}"

echo "=== E2E Smoke Test — Phase 4.4.1 ==="
echo "Starting World Engine on gRPC=:${GRPC_PORT} HTTP=:${HTTP_PORT}"

# Start world engine in background
WE_LOG=$(mktemp)
"${ROOT}/world-engine/target/release/agent-world-engine" > "${WE_LOG}" 2>&1 &
WE_PID=$!
echo "World Engine PID=${WE_PID}"

cleanup() {
    echo "Cleaning up..."
    kill ${WE_PID} 2>/dev/null || true
    for pid in "${AGENT_PIDS[@]-}"; do kill "$pid" 2>/dev/null || true; done
    rm -f "${WE_LOG}"
}
trap cleanup EXIT

# Wait for world engine to be ready
for i in $(seq 1 30); do
    if curl -sf "http://localhost:${HTTP_PORT}/api/v1/world/stats" > /dev/null 2>&1; then
        echo "World Engine ready after ${i}s"
        break
    fi
    sleep 1
done

if ! curl -sf "http://localhost:${HTTP_PORT}/api/v1/world/stats" > /dev/null 2>&1; then
    echo "ERROR: World Engine failed to start"
    cat "${WE_LOG}"
    exit 1
fi

echo ""
echo "Spawning ${AGENTS} agents (--no-llm, max-ticks=${MAX_TICKS})..."

AGENT_PIDS=()
for i in $(seq 1 "${AGENTS}"); do
    LOG="/tmp/e2e_agent_${i}.log"
    GRPC_PORT=${GRPC_PORT} python -m agent_runtime spawn \
        --name "Agent-${i}" \
        --no-llm \
        --max-ticks "${MAX_TICKS}" \
        --world-url "http://localhost:${HTTP_PORT}" \
        --health-port "$((9091 + i))" \
        > "${LOG}" 2>&1 &
    AGENT_PIDS+=($!)
    echo "  Agent-${i} launched (PID=$!)"
    sleep 2  # stagger agent starts
done

echo ""
echo "Waiting for agents to run..."

# Wait up to 180s for agents to finish
WAITED=0
while [ "${WAITED}" -lt 180 ]; do
    STATS=$(curl -sf "http://localhost:${HTTP_PORT}/api/v1/world/stats" 2>/dev/null || echo '{}')
    TICK=$(echo "${STATS}" | python3 -c "import sys,json; print(json.load(sys.stdin).get('tick',0))" 2>/dev/null || echo "0")
    AGENT_COUNT=$(echo "${STATS}" | python3 -c "import sys,json; print(json.load(sys.stdin).get('agent_count',0))" 2>/dev/null || echo "0")
    ALIVE=$(echo "${STATS}" | python3 -c "import sys,json; print(json.load(sys.stdin).get('alive_count',0))" 2>/dev/null || echo "0")
    
    echo "  [${WAITED}s] tick=${TICK} agents=${AGENT_COUNT} alive=${ALIVE}"
    
    # Check if all agents are done
    ALL_DONE=true
    for pid in "${AGENT_PIDS[@]}"; do
        if kill -0 "${pid}" 2>/dev/null; then
            ALL_DONE=false
            break
        fi
    done
    
    if [ "${ALL_DONE}" = true ]; then
        echo "All agents finished."
        break
    fi
    
    if [ "${TICK}" -ge 100 ]; then
        echo "100+ ticks reached."
        # Give agents a few more seconds to finish
        sleep 5
        break
    fi
    
    sleep 10
    WAITED=$((WAITED + 10))
done

# Final stats
echo ""
echo "=== Final World Stats ==="
STATS=$(curl -sf "http://localhost:${HTTP_PORT}/api/v1/world/stats")
echo "${STATS}" | python3 -m json.tool

echo ""
echo "=== Agent Details ==="
curl -sf "http://localhost:${HTTP_PORT}/api/v1/agents" | python3 -m json.tool

# Check organization events
echo ""
echo "=== Checking for Organization Events ==="
if curl -sf "http://localhost:${HTTP_PORT}/api/v1/world/events" > /tmp/e2e_events.json 2>&1; then
    python3 -c "
import json
events = json.load(open('/tmp/e2e_events.json'))
org_events = [e for e in events if 'organization' in str(e).lower() or 'org' in str(e).lower()]
print(f'Total events: {len(events)}')
print(f'Organization-related events: {len(org_events)}')
for e in org_events[:10]:
    print(f'  - {e}')
if not org_events:
    print('  (No organization events found — expected for random-decision --no-llm mode with few ticks)')
" 2>&1 || echo "(Could not parse events)"
else
    echo "(Events endpoint not available)"
fi

echo ""
echo "=== Smoke Test Complete ==="
