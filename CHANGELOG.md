# Changelog

All notable changes to Agent World will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-05-16

First public release — Phase 1 (Island) MVP foundations.

### Added

**World Engine (Rust)**
- Core simulation engine with agent lifecycle management
- Economy system: token ledger, money supply, task rewards
- Task Board CRUD with state machine (Open → Assigned → Done/Failed)
- Event system for world state mutations
- Reward distribution for completed tasks
- HTTP API (`/agents`, `/tasks`, `/events`, `/economy`)
- Integration and E2E test suites

**Agent Runtime (Python)**
- Think loop: Perceive → Decide → Act cycle
- Action executor with support for multiple action types
- Decision engine with configurable strategies
- LLM provider abstraction layer (OpenAI, Anthropic, Ollama)
- Cost tracking per provider and model
- WorkingMemory — in-memory FIFO cache with configurable capacity
- SurvivalInstinct module for resource-aware agent behavior
- AgentState data model with lifecycle fields

**Dashboard (Next.js)**
- Observatory UI with real-time world state polling
- Agent list and detail views
- Task board view
- Event stream component
- Leaderboard component
- Stat cards overview
- Sidebar navigation

**Infrastructure**
- GitHub Actions CI: Rust (clippy + test), Python (ruff + pytest), Dashboard (lint + type-check + build), Docker build check
- Dockerfiles for world-engine and agent-runtime
- Makefile with setup, dev, test, lint, fmt, and build targets
- VERSION file for release tracking

**Documentation**
- Comprehensive product design document (PRD)
- Comprehensive architecture design document (ADD)
- Architecture Decision Records (ADR-001, ADR-002, ADR-003)
- Contributing guidelines, Code of Conduct, Security policy
- MIT License

---

## [Unreleased]

[Unreleased]: https://github.com/sendwealth/agent-world/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/sendwealth/agent-world/releases/tag/v0.1.0
