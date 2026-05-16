# Changelog

All notable changes to Agent World will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Skill registry with 4 built-in skills (Explore, Trade, Rest, Communicate) (`world-engine/src/skills/`)
- ed25519 crypto: signing, verification, nonce replay prevention, key registry (`world-engine/src/crypto/`)
- Short-term memory module (`agent-runtime/agent_runtime/memory/short_term.py`)
- Timeline dashboard page (`dashboard/src/app/timeline/`)

### Fixed
- Dockerfile Rust toolchain upgraded to 1.85 for edition2024 support
- Missing HashMap import in rules test module

---

## [0.1.0] - 2026-05-16

Phase 1 (Island) initial release -- core subsystems with tests, not yet integrated end-to-end.

### Added

**World Engine (Rust)**
- Token burn engine with configurable phase multipliers and skill maintenance costs (`economy/token_burn.rs`)
- Escrow manager with full lifecycle: create, claim, complete, refund, dispute, resolve, freeze, expiry (`economy/escrow.rs`)
- Reward distributor with 2% platform fee, XP awards, and reputation changes (`economy/reward.rs`)
- Task marketplace with escrow integration (`economy/task.rs`)
- Event system with 24 typed event variants and JSON serialization (`world/event.rs`)
- EventBus using tokio::sync::broadcast with filtered subscriptions (`world/state.rs`)
- Currency, AgentPhase, DeathReason enums (`world/enums.rs`)
- Axum REST API scaffold with health endpoint (`api.rs`)
- Genesis YAML configuration loader (`main.rs`)
- Placeholder modules for lifecycle and rules engine
- Comprehensive unit tests for all economy modules (token burn, escrow, reward, task, events)

**Agent Runtime (Python)**
- Think loop with configurable perception/decision/reflection providers (`core/think_loop.py`)
- Decision engine with LLM-driven 10-action prompt template, JSON parsing, validation, fallback (`core/decide.py`)
- Action executor with 7 action types, retry logic, and ActionResult recording (`core/act.py`)
- Survival instinct module with 5 modes and 11 emergency actions (`survival/instinct.py`)
- WorkingMemory -- in-memory FIFO cache with decay and configurable capacity (`memory/working_memory.py`)
- AgentState Pydantic model with mutation helpers and world sync (`models/agent_state.py`)
- Skill dataclass with XP thresholds and level-up logic (`models/skill.py`)
- LLM provider abstraction: OpenAI, Anthropic, Ollama implementations (`llm/`)
- LLM cost tracking per provider and model (`llm/cost.py`)
- Provider factory (`llm/factory.py`)
- Unit tests for all modules

**Dashboard (Next.js)**
- Next.js 15 + React 19 + Tailwind CSS 4 + TypeScript project scaffold
- World overview page with StatCards (`app/page.tsx`)
- Agent list page (`app/agents/page.tsx`)
- Agent detail page (`app/agents/[id]/page.tsx`)
- Task list page (`app/tasks/page.tsx`)
- EventStream component for real-time event display
- Leaderboard component for agent rankings
- StatCards and StatCard components
- Sidebar navigation component
- SSE hook for live data (`hooks/useWorldState.ts`)
- REST API client (`lib/api.ts`)
- TypeScript type definitions (`lib/world.ts`)

**Infrastructure**
- GitHub Actions CI: Rust (clippy + test), Python (ruff + pytest), Dashboard (lint + type-check + build), Docker build check
- Dockerfiles for world-engine and agent-runtime
- Makefile with setup, dev, test, lint, fmt, proto, and build targets
- Setup script (`scripts/setup.sh`)

**Configuration**
- Genesis configuration (`config/genesis.yaml`) -- tick interval, economy, lifecycle, A2A, survival, market, safety
- World rules (`config/world-rules.yaml`) -- 31 rules covering survival, economic, social, and safety

**Protocol**
- A2A protobuf definition (`protocol/a2a.proto`) -- Discover, SendMessage, StreamMessages RPCs with ed25519 signatures

**Documentation**
- Product requirements document (`docs/DESIGN.md`)
- Architecture design document (`docs/ARCHITECTURE.md`)
- Development roadmap (`docs/ROADMAP.md`)
- Contributing guidelines (`CONTRIBUTING.md`)
- Code of Conduct (`CODE_OF_CONDUCT.md`)
- Security policy (`SECURITY.md`)
- MIT License (`LICENSE`)

### Not Yet Implemented
- Tick scheduler (world cannot advance)
- gRPC server / A2A message router
- Agent CLI entry point (no spawn mechanism)
- Lifecycle state machine (birth, aging, death transitions)
- Rules engine
- Social subsystem
- Evolution subsystem
- Market subsystem (knowledge, tools)
- Persistence (SQLite, snapshots, WAL)
- SSE endpoint for dashboard
- End-to-end integration

---

[Unreleased]: https://github.com/sendwealth/agent-world/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/sendwealth/agent-world/releases/tag/v0.1.0
