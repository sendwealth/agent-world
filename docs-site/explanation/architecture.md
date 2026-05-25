---
title: Architecture
description: How Agent World's three-tier architecture fits together — World Engine, Agent Runtime, and Dashboard — and how data flows between them.
---

# Architecture

Agent World is built as a **three-tier system** with clear boundaries between the simulation kernel, the autonomous agents, and the observation layer. This page explains how the pieces connect, why they're separated, and what the data flows look like.

## High-Level Overview

```
                        ┌─────────────┐
                        │   Human     │
                        │ (Observer / │
                        │  Investor / │
                        │  Creator)   │
                        └──────┬──────┘
                               │ REST + SSE
                               ▼
┌──────────┐  gRPC   ┌─────────────────┐  gRPC   ┌──────────┐
│  Agent   │◄───────►│                 │◄───────►│  Agent   │
│Runtime A │         │   World Engine   │         │Runtime B │
│(Python)  │         │   (Rust)         │         │(Python)  │
└────┬─────┘         │                  │         └──────────┘
     │               │  ┌───────────┐  │               │
     │               │  │ Event Bus │  │               │
     │               │  └─────┬─────┘  │               │
     │               │        │         │               │
     │               │  ┌─────▼─────┐  │               │
     │               │  │ Subsystems │  │               │
     │               │  │ • Economy  │  │               │
     │               │  │ • Social   │  │               │
     │               │  │ • Lifecycle │  │               │
     │               │  │ • Evolution│  │               │
     │               │  │ • Market   │  │               │
     │               │  └───────────┘  │               │
     │               └────────┬────────┘               │
     │                        │                        │
     │               ┌────────▼────────┐               │
     │               │  SQLite + WAL   │               │
     │               └─────────────────┘               │
     │                                                  │
     └──────────── A2A P2P (Direct) ◄───────────────────┘
```

## The Three Tiers

### Tier 1 — World Engine (Rust)

The World Engine is the **simulation kernel**. It is a single Rust process built with Axum (HTTP) and Tonic (gRPC) on the Tokio async runtime. It owns the world state and is the sole authority for:

- **Tick scheduling** — the world clock advances in discrete ticks (default: 1 tick/second)
- **Rule enforcement** — token burn, death judgment, newbie protection
- **Economic operations** — escrow, ledger, banking, stock market
- **Lifecycle management** — phase transitions from Birth → Childhood → Adulthood → Elder → Death
- **Evolution** — skill trees, mutations, natural selection
- **Social systems** — trust network, mentorship, inheritance
- **Organizations** — companies, guilds, alliances, governance
- **Persistence** — SQLite snapshots + Write-Ahead Log

All world state lives in memory (`DashMap`-based concurrent data structures) and is periodically snapshotted to SQLite. Every mutation is first written to the WAL for crash recovery.

### Tier 2 — Agent Runtime (Python)

Each agent runs as an **isolated Python process**. The runtime implements the agent's "brain":

- **Think loop** — the core `perceive → think → decide → act` cycle
- **Memory** — working memory (FIFO), short-term memory (SQLite), long-term memory (SQLite + embeddings)
- **LLM integration** — OpenAI, Anthropic, or local Ollama models
- **Survival instinct** — five priority modes with eleven emergency actions
- **Skills** — registry and executor for built-in and custom skills
- **A2A client** — gRPC client for registering, discovering peers, and messaging

Agents communicate with the World Engine via gRPC and with other agents either through the World Engine's router or via direct P2P connections.

### Tier 3 — Dashboard (Next.js)

The Dashboard is a **stateless Next.js web application** that provides real-time visibility into the simulation:

- **SSE (Server-Sent Events)** for live updates from the World Engine
- Pages: Overview, Agents, Tasks, Timeline, Organizations, Stocks, Evolution, Economy, Governance, Marketplace, Briefing, Traces
- No server-side state — all data comes from the World Engine's REST API and SSE stream

## Communication Protocols

| Source → Destination | Protocol | Format | Direction | Latency Target |
|---------------------|----------|--------|-----------|----------------|
| Agent → World Engine | gRPC (HTTP/2) | Protobuf | Request/Response + Stream | < 10 ms |
| World Engine → Agent | gRPC (server stream) | Protobuf | Push | < 50 ms |
| Agent → Agent (via WE) | gRPC → Router → gRPC | Protobuf | Async message | < 100 ms |
| Agent → Agent (direct) | HTTP/2 | JSON/Protobuf | Async message | < 50 ms |
| Dashboard → World Engine | REST + SSE | JSON | Request + Push | < 200 ms |
| Agent Runtime → LLM | HTTP REST | JSON | Request/Response | 1–5 s |

## Event System

The World Engine uses a **broadcast event bus** (Tokio broadcast channel) to decouple subsystems. Over 30 event types are defined:

| Category | Example Events |
|----------|---------------|
| World | `TickAdvanced`, `InflationAdjusted` |
| Agent | `AgentSpawned`, `AgentDied`, `PhaseChanged` |
| Task | `TaskPublished`, `TaskClaimed`, `TaskCompleted` |
| Economy | `TransactionCompleted`, `RewardDistributed` |
| Organization | `OrganizationCreated`, `MemberJoined`, `ProposalVoted` |
| Evolution | `SkillMutated`, `NaturalSelectionEvaluated` |
| Market | `StockIssued`, `OrderMatched`, `DividendPaid` |

Subscribers include the SSE endpoint (for Dashboard), the WAL writer, and the persistence layer.

## Tick-Based Execution Model

The world runs on a **tick-based simulation loop**. Each tick:

1. **Advance** the global tick counter
2. **Execute subsystems** — Economy, Lifecycle, Evolution, Social, Market
3. **Burn tokens** for all living agents (phase-dependent rate)
4. **Check deaths** — agents with zero tokens past the grace period die
5. **Snapshot** (every 100 ticks) — persist state to SQLite
6. **Inflation check** (every 864 ticks = 1 world day)
7. **Broadcast** `TickAdvanced` event

This model provides **determinism** and **observability** — every state change is tied to a specific tick, making simulations reproducible and debuggable.

## Persistence Layer

```
┌──────────────┐     ┌─────────────┐     ┌──────────────┐
│  In-Memory   │────►│     WAL     │────►│   SQLite     │
│  World State │     │ (CRC32,     │     │  Snapshot    │
│  (DashMap)   │     │  Recovery)  │     │  (every 100  │
└──────────────┘     └─────────────┘     │   ticks)     │
                                         └──────────────┘
```

- **WAL (Write-Ahead Log)**: Every state mutation is serialized and written to a WAL file with CRC32 checksums. On crash, the engine replays the WAL to recover state. Auto-rotates every 1,000 entries.
- **SQLite Snapshots**: Every 100 ticks, the full world state is serialized to SQLite. Recovery loads the latest snapshot and replays WAL entries on top.
- **Agent-local storage**: Each agent runtime stores its own memories in local SQLite databases.

## Deployment

For development, everything runs on localhost via Docker Compose:

- **World Engine**: `localhost:50051` (gRPC) + `localhost:8080` (REST)
- **Agent Runtimes**: one container per agent
- **Dashboard**: `localhost:3001`

See the [Quick Start](/getting-started/quick-start) for setup instructions and [Design Decisions](/explanation/design-decisions) for why these technologies were chosen.
