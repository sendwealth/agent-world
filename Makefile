.PHONY: help setup dev dev-llm dev-detach dev-down dev-logs dev-ps dev-restart \
       dev-ci dev-ci-down test lint fmt proto clean build run demo demo-json demo-death \
       bench stress test-e2e-integration screenshots screenshots-install

help: ## Show this help
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-20s\033[0m %s\n", $$1, $$2}'

# ── Setup ────────────────────────────────────────────────

setup: ## Install all dependencies
	@echo "Setting up development environment..."
	$(MAKE) setup-rust
	$(MAKE) setup-python
	$(MAKE) setup-dashboard
	$(MAKE) proto
	@echo "Setup complete!"

setup-rust:
	@echo "Rust dependencies..."
	cd world-engine && cargo fetch

setup-python:
	@echo "Python dependencies..."
	cd agent-runtime && uv pip install -e ".[dev]"

setup-dashboard:
	@echo "Dashboard dependencies..."
	cd dashboard && npm install

# ── Development (Docker Compose v2) ─────────────────────
# make dev         → all services (world-engine + 10 agents + dashboard)
# make dev-llm     → same + local Ollama LLM container

dev: ## Start all services with Docker Compose (world-engine + 10 agents + dashboard)
	@test -f .env || cp .env.example .env
	docker compose up --build

dev-llm: ## Start all services + local Ollama LLM
	@test -f .env || cp .env.example .env
	docker compose --profile local-llm up --build

dev-detach: ## Start all services in background
	@test -f .env || cp .env.example .env
	docker compose up --build -d

dev-down: ## Stop all Docker Compose services
	docker compose --profile local-llm --profile ci down

# ── CI profile (minimal: world-engine + 1 agent) ─────────
# make dev-ci         → build & run CI profile (world-engine + ci-agent)
# make dev-ci-down    → stop CI profile

dev-ci: ## Start CI profile (world-engine + 1 agent only)
	@test -f .env || cp .env.example .env
	docker compose --profile ci up --build

dev-ci-detach: ## Start CI profile in background
	@test -f .env || cp .env.example .env
	docker compose --profile ci up --build -d

dev-ci-down: ## Stop CI profile
	docker compose --profile ci down

dev-logs: ## Tail Docker Compose logs
	docker compose logs -f

dev-ps: ## List running Docker Compose services
	docker compose ps

dev-restart: ## Restart all services (rebuild + restart)
	@test -f .env || cp .env.example .env
	docker compose up --build -d --force-recreate

run-engine: ## Start world engine locally (no Docker)
	cd world-engine && cargo run --release

run-agents: ## Spawn and run agents locally
	cd agent-runtime && python -m agent_runtime spawn --count 2

run-dashboard: ## Start dashboard locally
	cd dashboard && npm run dev

# ── Testing ──────────────────────────────────────────────

test: ## Run all tests
	@echo "Running all tests..."
	$(MAKE) test-rust
	$(MAKE) test-python
	@echo "All tests passed!"

test-rust: ## Run Rust tests
	cd world-engine && cargo test

test-python: ## Run Python tests
	cd agent-runtime && pytest -v

test-integration: ## Run integration tests
	cd world-engine && cargo test --test e2e_full_flow

test-e2e: ## Run end-to-end tests
	cd world-engine && cargo test --test e2e_full_flow

test-e2e-integration: ## Run Python E2E integration tests (subprocess-based)
	pytest tests/e2e/ -v --timeout=60

bench: ## Run benchmark tests (P3-7: 100 agents × 2000 ticks)
	cd world-engine && cargo test --test benchmark_100_agents -- --nocapture

stress: ## Run stress tests (100 agents concurrent)
	cd world-engine && cargo test --test stress_100_agents -- --nocapture

demo: ## Run E2E demo: 2 agents survive 1000 ticks with trading, tasks, death
	python3 scripts/e2e_demo.py

demo-json: ## Run E2E demo with JSON metrics output
	python3 scripts/e2e_demo.py --json

demo-death: ## Run death scenario (agent with 30 tokens)
	python3 scripts/e2e_demo.py --death-scenario

# ── Screenshots ──────────────────────────────────────────

screenshots-install: ## Install Playwright + Chromium for screenshot automation
	cd scripts/screenshots && npm install && npx playwright install chromium

screenshots: ## Capture dashboard screenshots (requires running dashboard)
	@cd scripts/screenshots && DASHBOARD_URL=$${DASHBOARD_URL:-http://localhost:3000} node capture.mjs --out $$(pwd)/../../docs/screenshots $${SCREENSHOTS_ARGS:-}

# ── Code Quality ─────────────────────────────────────────

lint: ## Run all linters
	$(MAKE) lint-rust
	$(MAKE) lint-python

lint-rust:
	cd world-engine && cargo clippy -- -D warnings

lint-python:
	cd agent-runtime && ruff check . && mypy .

fmt: ## Format all code
	$(MAKE) fmt-rust
	$(MAKE) fmt-python

fmt-rust:
	cd world-engine && cargo fmt

fmt-python:
	cd agent-runtime && ruff format .

# ── Build ────────────────────────────────────────────────

proto: ## Generate protobuf code
	@echo "Generating protobuf code..."
	mkdir -p protocol/gen/python protocol/gen/rust
	protoc --proto_path=protocol \
		--python_out=protocol/gen/python \
		--grpc_python_out=protocol/gen/python \
		--rust_out=protocol/gen/rust \
		--tonic_out=protocol/gen/rust \
		protocol/*.proto || echo "protoc not found. Install: brew install protobuf"
	@# Fix generated grpc imports: protoc emits bare `import X_pb2` which breaks
	@# when the _pb2 module lives inside a package. Patch to use absolute import.
	@for f in protocol/gen/python/*_pb2_grpc.py; do \
		sed -i.bak 's/^import \(.*_pb2\) as \(.*\)$$/from protocol.gen.python import \1 as \2/' "$$f" && rm -f "$$f.bak"; \
	done

build: ## Build all components
	cd world-engine && cargo build --release

# ── Clean ────────────────────────────────────────────────

clean: ## Clean all build artifacts
	cd world-engine && cargo clean
	cd agent-runtime && rm -rf __pycache__ .pytest_cache .mypy_cache
	cd dashboard && rm -rf node_modules .next
	rm -rf protocol/gen/
	@echo "Clean!"
