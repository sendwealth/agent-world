#!/usr/bin/env bash
# ── Federation Multi-Instance E2E Test ──────────────────────────
#
# Prerequisites:
#   - Two World Engine instances running (see docker-compose-federation.yml)
#   - curl, jq installed
#
# Usage:
#   bash scripts/federation-e2e-test.sh
#
# Environment:
#   ALPHA_URL  — base URL for World Engine Alpha (default: http://localhost:8081)
#   BETA_URL   — base URL for World Engine Beta  (default: http://localhost:8082)
#
# ────────────────────────────────────────────────────────────────
set -euo pipefail

ALPHA_URL="${ALPHA_URL:-http://localhost:8081}"
BETA_URL="${BETA_URL:-http://localhost:8082}"

PASS=0
FAIL=0

# ── Helpers ─────────────────────────────────────────────────────

log()  { printf "\033[1;36m[INFO]\033[0m  %s\n" "$*"; }
ok()   { printf "\033[1;32m[PASS]\033[0m  %s\n" "$*"; PASS=$((PASS+1)); }
fail() { printf "\033[1;31m[FAIL]\033[0m  %s\n" "$*"; FAIL=$((FAIL+1)); }

assert_status() {
  local label="$1" expected="$2" actual="$3"
  if [ "$actual" = "$expected" ]; then
    ok "$label → HTTP $actual"
  else
    fail "$label → expected HTTP $expected, got HTTP $actual"
  fi
}

assert_field() {
  local label="$1" expected="$2" actual="$3"
  if [ "$actual" = "$expected" ]; then
    ok "$label → $actual"
  else
    fail "$label → expected '$expected', got '$actual'"
  fi
}

post() {
  local url="$1" body="$2"
  curl -s -w "\n%{http_code}" -X POST "$url" \
    -H "Content-Type: application/json" \
    -d "$body"
}

get() {
  local url="$1"
  curl -s -w "\n%{http_code}" "$url"
}

del() {
  local url="$1"
  curl -s -w "\n%{http_code}" -X DELETE "$url"
}

# response_body <full_response> — strips the last line (status code)
response_body() { sed '$d' <<< "$1"; }
# response_code <full_response> — returns the last line (status code)
response_code() { tail -1 <<< "$1"; }

# ── Wait for instances ──────────────────────────────────────────

wait_for() {
  local url="$1" name="$2" max=30 i=0
  log "Waiting for $name at $url ..."
  while [ $i -lt $max ]; do
    if curl -sf "$url/tasks" > /dev/null 2>&1; then
      ok "$name is up"
      return 0
    fi
    sleep 2
    i=$((i+1))
  done
  fail "$name did not become healthy within 60s"
  return 1
}

log "════════════════════════════════════════════════════════════"
log " Federation E2E Test — Multi-Instance"
log " Alpha: $ALPHA_URL"
log " Beta:  $BETA_URL"
log "════════════════════════════════════════════════════════════"

wait_for "$ALPHA_URL" "World-Alpha"
wait_for "$BETA_URL"  "World-Beta"

# ──────────────────────────────────────────────────────────────────
# 1. Cross-Registration: Each instance registers the other
# ──────────────────────────────────────────────────────────────────

log ""
log "── 1. Cross-Registration ────────────────────────────────"

# Alpha registers Beta as a foreign world
resp=$(post "$ALPHA_URL/api/v1/federation/worlds" '{
  "id": "world-beta",
  "name": "Beta World",
  "endpoint": "http://world-engine-beta:8080",
  "tick": 0
}')
assert_status "Alpha registers Beta" "201" "$(response_code <<< "$resp")"

# Beta registers Alpha as a foreign world
resp=$(post "$BETA_URL/api/v1/federation/worlds" '{
  "id": "world-alpha",
  "name": "Alpha World",
  "endpoint": "http://world-engine-alpha:8080",
  "tick": 0
}')
assert_status "Beta registers Alpha" "201" "$(response_code <<< "$resp")"

# Verify Alpha sees Beta in its world list
resp=$(get "$ALPHA_URL/api/v1/federation/worlds")
assert_status "Alpha lists worlds" "200" "$(response_code <<< "$resp")"
count=$(response_body <<< "$resp" | jq 'length')
if [ "$count" -ge 1 ]; then
  ok "Alpha sees ≥1 foreign world"
else
  fail "Alpha sees 0 foreign worlds"
fi

# ──────────────────────────────────────────────────────────────────
# 2. Heartbeat Exchange
# ──────────────────────────────────────────────────────────────────

log ""
log "── 2. Heartbeat Exchange ────────────────────────────────"

# Alpha sends heartbeat for itself (via WorldRegistry)
# Note: the diplomacy routes use FederationEngine, registry uses WorldRegistry —
# so heartbeat via the federation routes targets WorldRegistry.
resp=$(post "$ALPHA_URL/api/v1/federation/worlds/world-alpha/heartbeat" '{
  "total_ticks": 100, "alive_agents": 5, "avg_reputation": 4.5,
  "total_tokens": 10000, "total_money": 5000
}')
code=$(response_code <<< "$resp")
assert_status "Alpha heartbeat" "200" "$code"

# ──────────────────────────────────────────────────────────────────
# 3. Diplomatic Relations: Establish on both sides
# ──────────────────────────────────────────────────────────────────

log ""
log "── 3. Establish Diplomatic Relations ────────────────────"

# Alpha establishes relations with Beta
resp=$(post "$ALPHA_URL/api/v1/federation/establish-relations" '{
  "world_id": "world-beta",
  "tick": 10
}')
assert_status "Alpha establishes relations with Beta" "200" "$(response_code <<< "$resp")"

# Beta establishes relations with Alpha
resp=$(post "$BETA_URL/api/v1/federation/establish-relations" '{
  "world_id": "world-alpha",
  "tick": 10
}')
assert_status "Beta establishes relations with Alpha" "200" "$(response_code <<< "$resp")"

# ──────────────────────────────────────────────────────────────────
# 4. Treaty Lifecycle: Propose → Accept → Verify
# ──────────────────────────────────────────────────────────────────

log ""
log "── 4. Treaty Lifecycle ──────────────────────────────────"

# Alpha proposes a trade treaty with Beta
resp=$(post "$ALPHA_URL/api/v1/federation/treaties" '{
  "world_id": "world-beta",
  "treaty_type": "trade_pact",
  "terms": "free movement of goods and agents",
  "tick": 20,
  "duration_ticks": 1000
}')
assert_status "Alpha proposes trade treaty" "201" "$(response_code <<< "$resp")"
treaty_id=$(response_body <<< "$resp" | jq -r '.id // empty')

if [ -n "$treaty_id" ]; then
  ok "Got treaty ID: $treaty_id"

  # Verify treaty exists
  resp=$(get "$ALPHA_URL/api/v1/federation/treaties/$treaty_id")
  assert_status "Get treaty on Alpha" "200" "$(response_code <<< "$resp")"
  status=$(response_body <<< "$resp" | jq -r '.status // empty')
  assert_field "Treaty status" "proposed" "$status"

  # Accept the treaty on Alpha (simulating Beta's acceptance)
  resp=$(post "$ALPHA_URL/api/v1/federation/treaties/$treaty_id/accept" '{"tick": 25}')
  assert_status "Accept treaty" "200" "$(response_code <<< "$resp")"

  # Verify active status
  resp=$(get "$ALPHA_URL/api/v1/federation/treaties/$treaty_id")
  status=$(response_body <<< "$resp" | jq -r '.status // empty')
  assert_field "Treaty now active" "active" "$status"
else
  fail "Could not extract treaty_id"
fi

# ──────────────────────────────────────────────────────────────────
# 5. Agent Migration: Submit → Review → Execute
# ──────────────────────────────────────────────────────────────────

log ""
log "── 5. Agent Migration (Alpha → Beta) ────────────────────"

# Submit migration on Alpha (agent migrates from Alpha to Beta)
resp=$(post "$ALPHA_URL/api/v1/migration/submit" '{
  "agent_id": "agent-explorer-1",
  "source_world_id": "world-alpha",
  "target_world_id": "world-beta",
  "name": "Explorer One",
  "phase": "explorer",
  "tokens": 100000,
  "money": 5000,
  "reputation": 5.0,
  "skills": {"mining": 10, "navigation": 8},
  "public_key": "pk-explorer-1"
}')
code=$(response_code <<< "$resp")
assert_status "Submit migration on Alpha" "200" "$code"

migration_id=$(response_body <<< "$resp" | jq -r '.data.migration_id // empty')
if [ -n "$migration_id" ]; then
  ok "Got migration ID: $migration_id"

  # Verify pending status
  resp=$(get "$ALPHA_URL/api/v1/migration/$migration_id")
  status=$(response_body <<< "$resp" | jq -r '.data.status // empty')
  assert_field "Migration status" "pending" "$status"

  # Review (approve) on Alpha (acting as target world reviewer)
  resp=$(post "$ALPHA_URL/api/v1/migration/$migration_id/review" "{
    \"migration_id\": \"$migration_id\",
    \"approved\": true,
    \"reviewer_world_id\": \"world-beta\"
  }")
  assert_status "Approve migration" "200" "$(response_code <<< "$resp")"
  status=$(response_body <<< "$resp" | jq -r '.data.status // empty')
  assert_field "Migration after review" "approved" "$status"

  # Execute the migration
  resp=$(post "$ALPHA_URL/api/v1/migration/$migration_id/execute" '{}')
  assert_status "Execute migration" "200" "$(response_code <<< "$resp")"
  status=$(response_body <<< "$resp" | jq -r '.data.status // empty')
  assert_field "Migration completed" "completed" "$status"

  # Verify completed_at is set
  completed_at=$(response_body <<< "$resp" | jq -r '.data.completed_at // empty')
  if [ -n "$completed_at" ]; then
    ok "completed_at timestamp present"
  else
    fail "completed_at timestamp missing"
  fi
else
  fail "Could not extract migration_id"
fi

# ──────────────────────────────────────────────────────────────────
# 6. Migration Stats & Policy
# ──────────────────────────────────────────────────────────────────

log ""
log "── 6. Migration Stats & Policy ──────────────────────────"

resp=$(get "$ALPHA_URL/api/v1/migration/stats")
assert_status "Get migration stats" "200" "$(response_code <<< "$resp")"
completed=$(response_body <<< "$resp" | jq -r '.data.completed_migrations // 0')
if [ "$completed" -ge 1 ]; then
  ok "Stats show ≥1 completed migration"
else
  fail "Stats show 0 completed migrations"
fi

resp=$(get "$ALPHA_URL/api/v1/migration/policy")
assert_status "Get migration policy" "200" "$(response_code <<< "$resp")"

# ──────────────────────────────────────────────────────────────────
# 7. Federation Summary
# ──────────────────────────────────────────────────────────────────

log ""
log "── 7. Federation Summary ────────────────────────────────"

resp=$(get "$ALPHA_URL/api/v1/federation/summary")
assert_status "Get federation summary" "200" "$(response_code <<< "$resp")"
total=$(response_body <<< "$resp" | jq -r '.total_worlds // 0')
if [ "$total" -ge 2 ]; then
  ok "Summary shows ≥2 worlds"
else
  fail "Summary shows <2 worlds (got $total)"
fi

# ──────────────────────────────────────────────────────────────────
# 8. War & Peace Cycle
# ──────────────────────────────────────────────────────────────────

log ""
log "── 8. War & Peace ───────────────────────────────────────"

resp=$(post "$ALPHA_URL/api/v1/federation/declare-war" '{
  "world_id": "world-beta",
  "tick": 100
}')
assert_status "Alpha declares war on Beta" "200" "$(response_code <<< "$resp")"

resp=$(post "$ALPHA_URL/api/v1/federation/propose-peace" '{
  "world_id": "world-beta",
  "tick": 200
}')
assert_status "Alpha proposes peace" "201" "$(response_code <<< "$resp")"

# ──────────────────────────────────────────────────────────────────
# 9. Sanctions & Sever Ties
# ──────────────────────────────────────────────────────────────────

log ""
log "── 9. Sanctions & Sever Ties ────────────────────────────"

# Re-establish relations first
resp=$(post "$ALPHA_URL/api/v1/federation/establish-relations" '{
  "world_id": "world-beta",
  "tick": 250
}')
assert_status "Re-establish relations" "200" "$(response_code <<< "$resp")"

resp=$(post "$ALPHA_URL/api/v1/federation/sanctions" '{
  "world_id": "world-beta",
  "reason": "treaty violation test",
  "tick": 260
}')
assert_status "Impose sanctions on Beta" "200" "$(response_code <<< "$resp")"

resp=$(post "$ALPHA_URL/api/v1/federation/sever-ties" '{
  "world_id": "world-beta",
  "tick": 300
}')
assert_status "Sever ties with Beta" "200" "$(response_code <<< "$resp")"

# ──────────────────────────────────────────────────────────────────
# 10. Deregistration
# ──────────────────────────────────────────────────────────────────

log ""
log "── 10. Deregistration ──────────────────────────────────"

resp=$(del "$ALPHA_URL/api/v1/federation/worlds/world-beta")
assert_status "Deregister Beta from Alpha" "200" "$(response_code <<< "$resp")"

# ── Summary ──────────────────────────────────────────────────────

log ""
log "════════════════════════════════════════════════════════════"
log " Results:  $PASS passed,  $FAIL failed"
log "════════════════════════════════════════════════════════════"

if [ "$FAIL" -gt 0 ]; then
  exit 1
fi
exit 0
