# E2E Integration Tests

Cross-process integration tests that start World Engine, Agent Runtime, and Dashboard as real subprocesses and verify they work together.

## Prerequisites

```bash
# Rust toolchain (for World Engine)
cargo --version

# Python with dev dependencies
cd agent-runtime && uv pip install -e ".[dev]"

# Dashboard dependencies (only needed for Dashboard tests)
cd dashboard && npm install
```

## Running

```bash
# Run all E2E integration tests
make test-e2e-integration

# Or directly with pytest
pytest tests/e2e/ -v --timeout=60

# Run a single test file
pytest tests/e2e/test_smoke.py -v

# Run a specific test
pytest tests/e2e/test_smoke.py::TestWorldEngine::test_engine_http_health -v
```

## Configuration

All ports and timeouts are parametrized via environment variables:

| Variable | Default | Description |
|---|---|---|
| `E2E_ENGINE_PORT` | `3000` | World Engine HTTP port |
| `E2E_GRPC_PORT` | `50051` | World Engine gRPC port |
| `E2E_DASHBOARD_PORT` | `3001` | Dashboard dev server port |
| `E2E_AGENT_HEALTH_PORT` | `9090` | Base health check port for agents |
| `E2E_STARTUP_TIMEOUT` | `30` | Max seconds to wait for a service to start |

Example with custom ports:

```bash
E2E_ENGINE_PORT=8080 E2E_STARTUP_TIMEOUT=60 pytest tests/e2e/ -v
```

## Architecture

```
tests/e2e/
├── conftest.py      # Fixtures: world_engine_process, agent_process, dashboard_process
├── test_smoke.py    # Smoke tests: startup, health, registration
└── README.md        # This file
```

### Fixtures

- **`world_engine_process`** (session-scoped) — Starts `cargo run --release` in `world-engine/`, waits for HTTP health on `/api/v1/world/stats`.
- **`agent_process`** (function-scoped) — Runs `python -m agent_runtime spawn --no-llm`, waits for `/health` endpoint.
- **`dashboard_process`** (session-scoped) — Runs `npm run dev` in `dashboard/`, waits for HTTP 200 on `/`.

Each fixture handles its own process lifecycle: startup with health polling and graceful teardown with SIGTERM → SIGKILL fallback.

## Constraints

- Tests use `subprocess` (no Docker).
- All ports and timeouts are parametrized.
- Each test self-manages process lifecycle via fixtures.
- Single test timeout: < 60s.
