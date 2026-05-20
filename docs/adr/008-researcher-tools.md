# ADR-008: Phase 4.5 Researcher Tools Architecture

**Status**: Accepted  
**Date**: 2026-05-20  
**Decider**: 架构师

## Context

Phase 4.5 exposes Agent World's internal capabilities to external researchers via REST API, Python SDK, and data export tools. The existing API (`api.rs`, ~2500 lines monolithic) has no authentication, no experiment framework, and no researcher-specific endpoints.

Key constraints from issue spec:
1. API layer based on existing Axum HTTP service — no new frameworks
2. SDK is pure Python, thin wrapper — no business logic in SDK
3. Simple API Key auth — no OAuth
4. Data export formats compatible with mainstream tools (pandas, Gephi, etc.)

## Decision

### 1. API Module Structure — Split into Domain Modules

The monolithic `api.rs` will be refactored into `api/` directory with separate modules:

```
world-engine/src/api/
├── mod.rs          — AppState, Shared types, router composition
├── tasks.rs        — Existing task handlers (migrated)
├── wal.rs          — Existing WAL handlers (migrated)
├── world.rs        — Existing world/agent/tick handlers (migrated)
├── org.rs          — Existing org/governance handlers (migrated)
├── banking.rs      — Existing banking handlers (migrated)
├── stocks.rs       — Existing stock market handlers (migrated)
├── traces.rs       — Existing trace handlers (migrated)
├── auth.rs         — NEW: API Key authentication middleware
├── research.rs     — NEW: Researcher query endpoints (4.5.1)
├── experiment.rs   — NEW: Experiment management (4.5.2)
└── export.rs       — NEW: Data export endpoints (4.5.4)
```

**Rationale**: The 2500-line monolith is unmaintainable. New researcher features should live in their own modules. Full migration of existing handlers is out of scope for 4.5 — we extract only the new modules and include them via `mod api` in the existing structure. The existing `api.rs` remains as-is; new modules are added alongside via `#[path]` or a new `api/` directory structure.

**Revised approach**: Keep `api.rs` intact. Add new files as `api_auth.rs`, `api_research.rs`, `api_experiment.rs`, `api_export.rs` in `world-engine/src/`. Register them as modules in `lib.rs`. Mount new routes under `/api/v2/` prefix to avoid collision with existing endpoints. This is the least-invasive approach.

### 2. Authentication — Axum Middleware Layer

```rust
// api_auth.rs — API Key authentication as Axum middleware
// - Keys stored in-memory HashMap<String, ApiKeyInfo>
// - Header: X-API-Key: <key>
// - Rate limiting: token bucket per key (60 req/min default)
// - Routes under /api/v2/* require auth; /api/v1/* unchanged
```

**Key design**:
- `ApiKeyStore` — in-memory, loaded from env var `API_KEYS` (comma-separated) or file
- Axum middleware layer that extracts `X-API-Key` header, validates, and applies rate limit
- Response headers: `X-RateLimit-Limit`, `X-RateLimit-Remaining`, `X-RateLimit-Reset`
- `AppState` extended with `api_key_store: Option<SharedApiKeyStore>`

### 3. Experiment Framework — In-Process State Machine

```rust
// api_experiment.rs — Experiment state machine
// States: Created → Running ⇄ Paused → Stopped
// Each experiment: isolated WorldState clone? No — too expensive.
// Approach: experiment is a "recording session" that captures tick ranges + config.
```

**Experiment model**:
- `Experiment` struct: id, name, config (agent count, tick limit, LLM config), status, tick range, created_at
- Experiment doesn't create an isolated world. Instead, it "observes" the running world for a configured tick range, recording all state changes.
- `POST /api/v2/experiments` → creates observation window
- `POST /api/v2/experiments/{id}/start` → begins recording from current tick
- `POST /api/v2/experiments/{id}/stop` → ends recording
- `POST /api/v2/experiments/{id}/inject` → injects external event into world (uses existing intervention subsystem)
- Results: query snapshot store for the experiment's tick range

**Why not isolated worlds**: Cloning WorldState (1000+ agents with complex state) is prohibitively expensive. Researchers observe the real simulation.

### 4. Research API — Read-Only Views over Existing State

All endpoints under `/api/v2/` (authenticated):
- `GET /api/v2/world/state` — aggregated world state (tick, agent count, org count, resource distribution)
- `GET /api/v2/agents/{id}` — deep agent profile (personality, values, memories, relationships)
- `GET /api/v2/world/history?from_tick=&to_tick=` — historical snapshot range query
- `GET /api/v2/world/events/stream` — SSE (reuses existing EventBus subscription)
- `GET /api/v2/metrics/emergence` — computed emergence metrics (cultural diversity, org formation, economic indicators)

### 5. Python SDK — Thin HTTP Client

```
sdk/
├── pyproject.toml
├── agent_world_sdk/
│   ├── __init__.py
│   ├── client.py        — AgentWorldClient(url, api_key)
│   ├── models.py        — Pydantic response models
│   ├── world.py         — client.world.state(), client.world.history()
│   ├── agents.py        — client.agents.list(), client.agents.get(id)
│   ├── experiments.py   — client.experiments.create(config), exp.run(), exp.results()
│   └── analyze.py       — analyze.cultural_diversity(data), analyze.trust_network(data)
└── examples/
    └── quickstart.ipynb — Jupyter notebook demo
```

**Principle**: SDK is purely `httpx` + `pydantic`. Zero business logic. Each method = one HTTP call + response parsing.

### 6. Data Export Formats

| Format | Endpoint | Tool Compatibility |
|--------|----------|-------------------|
| JSON | `GET /api/v2/export/snapshots?format=json` | Generic |
| CSV | `GET /api/v2/export/snapshots?format=csv` | pandas, R |
| GraphML | `GET /api/v2/export/graph?format=graphml` | Gephi, Cytoscape |
| JSON Graph | `GET /api/v2/export/graph?format=json` | NetworkX, D3 |

## Execution Order

```
Phase A (parallel):
  ├── [BE] api_auth.rs — API Key auth middleware + rate limiting
  ├── [BE] api_research.rs — Research query endpoints
  └── [Python] SDK scaffold — pyproject.toml, client.py, models.py

Phase B (depends on A):
  ├── [BE] api_experiment.rs — Experiment management (needs auth)
  ├── [BE] api_export.rs — Data export endpoints
  └── [Python] SDK modules — world.py, agents.py, experiments.py

Phase C (depends on B):
  ├── [DOC] API reference update — OpenAPI + docs/api-reference.md
  ├── [Python] SDK examples — Jupyter notebook
  └── [BE] Integration tests
```

## Risks

| Risk | Mitigation |
|------|-----------|
| Experiment "inject" could destabilize running world | Use existing intervention subsystem which has safety checks |
| Rate limiting in-memory is lost on restart | Acceptable for research tool; keys are stateless |
| Existing `api.rs` monolith makes module extraction tricky | Don't extract existing code; add new modules alongside |
| Emergence metrics computation could be slow | Cache results, compute on-demand with TTL |
