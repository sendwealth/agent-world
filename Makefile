.PHONY: help setup dev test lint fmt proto clean build run

help: ## Show this help
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-20s\033[0m %s\n", $$1, $$2}'

# ── Setup ────────────────────────────────────────────────

setup: ## Install all dependencies
	@echo "🔧 Setting up development environment..."
	$(MAKE) setup-rust
	$(MAKE) setup-python
	$(MAKE) setup-dashboard
	$(MAKE) proto
	@echo "✅ Setup complete!"

setup-rust:
	@echo "🦀 Rust dependencies..."
	cd world-engine && cargo fetch

setup-python:
	@echo "🐍 Python dependencies..."
	cd agent-runtime && uv pip install -e ".[dev]"

setup-dashboard:
	@echo "🖥️  Dashboard dependencies..."
	cd dashboard && npm install

# ── Development ──────────────────────────────────────────

dev: ## Start all services with Docker Compose
	@test -f .env || cp .env.example .env
	docker compose up --build

dev-detach: ## Start all services in background
	@test -f .env || cp .env.example .env
	docker compose up --build -d

dev-down: ## Stop all Docker Compose services
	docker compose down

dev-logs: ## Tail Docker Compose logs
	docker compose logs -f

run-engine: ## Start world engine
	cd world-engine && cargo run --release

run-agents: ## Spawn and run agents
	cd agent-runtime && python -m agent_runtime spawn --count 2

run-dashboard: ## Start dashboard
	cd dashboard && npm run dev

# ── Testing ──────────────────────────────────────────────

test: ## Run all tests
	@echo "🧪 Running all tests..."
	$(MAKE) test-rust
	$(MAKE) test-python
	@echo "✅ All tests passed!"

test-rust: ## Run Rust tests
	cd world-engine && cargo test

test-python: ## Run Python tests
	cd agent-runtime && pytest -v

test-integration: ## Run integration tests
	cd world-engine && cargo test --test e2e_full_flow

test-e2e: ## Run end-to-end tests
	cd world-engine && cargo test --test e2e_full_flow

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
	@echo "📦 Generating protobuf code..."
	mkdir -p protocol/gen/python protocol/gen/rust
	protoc --proto_path=protocol \
		--python_out=protocol/gen/python \
		--grpc_python_out=protocol/gen/python \
		--rust_out=protocol/gen/rust \
		--tonic_out=protocol/gen/rust \
		protocol/*.proto || echo "⚠️  protoc not found. Install: brew install protobuf"

build: ## Build all components
	cd world-engine && cargo build --release

# ── Clean ────────────────────────────────────────────────

clean: ## Clean all build artifacts
	cd world-engine && cargo clean
	cd agent-runtime && rm -rf __pycache__ .pytest_cache .mypy_cache
	cd dashboard && rm -rf node_modules .next
	rm -rf protocol/gen/
	@echo "🧹 Clean!"
