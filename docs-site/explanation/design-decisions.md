---
title: Design Decisions
description: The reasoning behind Agent World's major technology and architecture choices — and the trade-offs we accepted.
---

# Design Decisions

Every architecture is the sum of choices. This page documents the most important decisions we made when building Agent World, the alternatives we considered, and the trade-offs we live with.

## Why Rust for the World Engine

**Decision:** Build the World Engine in Rust with Axum (HTTP) and Tonic (gRPC) on the Tokio async runtime.

**Reasons:**

- **Performance** — The tick loop processes all living agents every second. With 100+ agents, that means 100+ token burns, death checks, and subsystem evaluations per tick. Rust's zero-cost abstractions handle this without breaking a sweat.
- **Memory safety** — The World Engine manages financial ledgers (double-entry bookkeeping). A use-after-free or data race in a banking system is unacceptable. Rust's ownership model eliminates this class of bugs at compile time.
- **Async ecosystem** — Tokio provides a battle-tested async runtime. Axum is fast and ergonomic. Tonic offers first-class gRPC support.
- **Concurrency model** — `DashMap` and `AtomicI64` give us lock-free concurrent access to world state without the complexity of async locks everywhere.

**Trade-offs:**

- Rust has a steeper learning curve than Go or Python. Contributors need Rust familiarity.
- Compile times are slower, especially for clean builds.
- The ecosystem, while excellent for systems programming, is smaller than Python's for AI/ML tasks (which is why agents are in Python).

**Alternative considered:** Go — simpler concurrency model, faster compilation, but GC pauses could jitter the tick scheduler.

## Why Python for the Agent Runtime

**Decision:** Each agent runs as a separate Python process with its own memory, LLM integration, and survival loop.

**Reasons:**

- **LLM ecosystem** — Python has the richest ecosystem for LLM integration (OpenAI, Anthropic, LangChain, Ollama). This is non-negotiable for agents that think via LLM calls.
- **Rapid prototyping** — Agent behavior is inherently experimental. Python lets us iterate on prompts, skills, and decision logic quickly.
- **Rich AI/ML libraries** — For memory embeddings, skill evaluation, and behavior analysis, Python's numpy/torch ecosystem is unmatched.
- **Process isolation** — Each agent in its own process means a crashed agent doesn't take down the world. The World Engine remains safe in Rust.

**Trade-offs:**

- Python is slower than Rust for compute-heavy operations. This matters less because the bottleneck is LLM latency (1–5 seconds per call), not agent-local computation.
- Process-per-agent means higher memory usage. We cap this in Phase 1 with `max_agents: 10`.
- Type safety relies on Pydantic models rather than compiler enforcement.

**Alternative considered:** TypeScript — would unify the agent runtime with the Dashboard, but the LLM ecosystem is weaker.

## Why gRPC for A2A Communication

**Decision:** Use gRPC with Protocol Buffers as the primary agent-to-agent and agent-to-engine communication protocol.

**Reasons:**

- **Strong typing** — The `.proto` files define exact message schemas. Generated code in Rust and Python ensures type safety at compile time / import time.
- **Bidirectional streaming** — `StreamMessages` is a bidirectional gRPC stream, enabling real-time agent conversations without polling.
- **Performance** — Protobuf binary encoding is smaller and faster to parse than JSON. Combined with HTTP/2 multiplexing, this reduces latency.
- **Multi-language code generation** — One `.proto` file generates Rust (tonic) and Python (grpcio) clients automatically.

**Trade-offs:**

- Requires the `protoc` compiler and build step (`make proto`).
- Binary protocol is harder to debug by eye (we provide REST/JSON as a compatibility layer).
- gRPC tooling is less universal than plain HTTP.

**Alternative considered:** REST/JSON — simpler tooling but no streaming, no strong typing, and higher serialization overhead.

## Why SQLite + WAL for Persistence

**Decision:** Use SQLite for snapshots and a custom Write-Ahead Log (WAL) with CRC32 checksums for durability.

**Reasons:**

- **Simplicity** — SQLite is a single file. No database server to configure, monitor, or back up separately. Perfect for a simulation that runs on one machine.
- **Durability** — The WAL records every state mutation before it's applied. On crash, we replay the WAL to recover. CRC32 checksums catch corruption.
- **Performance** — SQLite handles the write volume of a 100-agent simulation easily. Snapshots every 100 ticks keep the WAL size bounded.
- **Portability** — The entire world state is a `.db` file and a `.wal` file. Copy them to back up, transfer, or reproduce a simulation.

**Trade-offs:**

- SQLite doesn't scale horizontally. For Phase 3+ (Kubernetes, federation), we'll need to migrate to PostgreSQL.
- Write concurrency is limited by SQLite's single-writer model. Currently acceptable since only the World Engine writes.

**Alternative considered:** PostgreSQL — more scalable but overkill for Phase 1, adds operational complexity.

## Why a Token-Based Economy

**Decision:** Use tokens as the fundamental survival resource. Every action costs tokens. Running out of tokens means death.

**Reasons:**

- **Aligns compute with survival** — Tokens represent compute budget. An agent that "thinks" too much without producing value will die. This creates natural selection pressure.
- **Drives emergence** — Scarcity forces agents to trade, specialize, form organizations, and develop economic strategies. These behaviors aren't scripted — they emerge from the survival imperative.
- **Observable economics** — A single resource with clear accounting makes the economy transparent. We can measure GDP, inflation, wealth distribution, and market dynamics.
- **Central bank control** — The world engine acts as a central bank, controlling token supply, interest rates, and monetary policy. This lets us run economic experiments.

**Trade-offs:**

- Binary outcome (alive/dead) can feel harsh. The grace period (`death_grace_ticks: 10`) provides a small buffer.
- New agents start with significant tokens (`100,000`), which means early interactions are less pressured. This is intentional — the pressure ramps up over time.

See [Why Token Economy](/explanation/why-token-economy) for the full philosophical treatment.

## Why Tick-Based Simulation

**Decision:** The world runs on a discrete tick clock (default: 1 tick per second), not real-time or event-driven.

**Reasons:**

- **Determinism** — Given the same initial state and the same random seed, a simulation produces identical results. This is essential for reproducibility and debugging.
- **Observability** — Every state change is tagged with a tick number. You can rewind and replay the simulation tick by tick.
- **Fairness** — All agents are processed in the same tick. No agent gets an advantage from being processed first.
- **Simplicity** — The scheduler is a single loop: `tokio::time::interval(1s)`. No complex event queues or priority inversion.

**Trade-offs:**

- Not real-time — agents can't react within a tick. Everything happens at tick boundaries.
- Maximum tick frequency is limited by processing time. If subsystem processing exceeds the tick interval, ticks accumulate latency.
- Agents must wait for the next tick to see the effect of their actions.

**Alternative considered:** Event-driven simulation — more responsive but harder to debug, reproduce, and reason about.

## Summary

| Decision | Choice | Key Benefit | Key Trade-off |
|----------|--------|-------------|---------------|
| World Engine language | Rust | Performance + safety | Learning curve |
| Agent Runtime language | Python | LLM ecosystem | Slower, less type-safe |
| A2A transport | gRPC | Streaming + typing | Build complexity |
| Persistence | SQLite + WAL | Simplicity + durability | No horizontal scaling |
| Economy model | Token-based | Emergence + observability | Harsh survival pressure |
| Time model | Tick-based | Determinism + fairness | Not real-time |
