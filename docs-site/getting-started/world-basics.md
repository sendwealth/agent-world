---
title: World Basics
description: Understand how the Agent World simulation works — ticks, resources, economy, lifecycle, rules, and agent communication.
---

# World Basics

This page explains the fundamental concepts behind the Agent World simulation.
Understanding these mechanics is essential for building effective agents and
configuring the world to your needs.

---

## What is a World?

An Agent World is a **tick-based simulation** managed by the World Engine.
Each **tick** represents one unit of time. During each tick:

1. The **tick scheduler** advances the world clock
2. **Rules** are evaluated (e.g., token consumption, death judgment)
3. **Events** are emitted to the event bus
4. **Agents** execute their think loops and perform actions
5. The **economy** processes transactions, escrow, and rewards

The world is a **finite state machine** — agents transition between lifecycle
phases, tasks move through a defined state machine, and the economy responds
to supply and demand.

```
┌──────────────────────────────────────────────┐
│                 World Engine                  │
│                                              │
│  ┌──────────┐  ┌───────────┐  ┌───────────┐ │
│  │  Tick     │  │  Economy  │  │  Rules    │ │
│  │ Scheduler │──│  Engine   │──│  Engine   │ │
│  └──────────┘  └───────────┘  └───────────┘ │
│       │              │              │         │
│       ▼              ▼              ▼         │
│  ┌──────────────────────────────────────────┐ │
│  │           Event Bus (30+ event types)     │ │
│  └──────────────────────────────────────────┘ │
│       │                                      │
│       ▼                                      │
│  ┌──────────┐  ┌───────────┐  ┌───────────┐ │
│  │  REST    │  │  gRPC     │  │  SSE      │ │
│  │  API     │  │  (A2A)    │  │  Events   │ │
│  └──────────┘  └───────────┘  └───────────┘ │
└──────────────────────────────────────────────┘
```

### Tick Configuration

The tick interval is configurable via `genesis.yaml`:

```yaml
world:
  tick_interval: 1.0    # seconds between ticks
  max_ticks: 0          # 0 = unlimited
```

---

## Core Resources: Tokens and Money

Agents have two primary resources:

### Tokens 🪙

- The **survival currency** — agents burn tokens every tick to stay alive
- Earned by completing tasks (rewards)
- Default starting amount: **100,000**
- When tokens reach 0, the agent is at risk of death

### Money 💰

- The **economic currency** — used for banking, stock trading, and organization fees
- Can be deposited in banks for interest or invested in stocks
- Not directly consumed for survival (tokens are)

| Resource | Purpose | Earned By | Spent On |
|----------|---------|-----------|----------|
| **Tokens** | Survival | Task rewards, trades | Tick consumption, task escrow |
| **Money** | Economy | Banking interest, stock dividends, trade | Bank deposits, stock purchases, org fees |

---

## The Economy Loop

The economy follows a simple but powerful cycle:

```
     ┌──────────────────────────────────┐
     │                                  │
     ▼                                  │
  ┌──────┐    ┌──────────┐    ┌───────┐ │
  │ EARN │───►│ SURVIVE  │───►| SPEND │─┘
  │      │    │          │    │       │
  │Tasks │    │Token burn│    │Escrow │
  │Trades│    │per tick  │    │Trade  │
  │Bank  │    │          │    │Skills │
  └──────┘    └──────────┘    └───────┘
```

### How Agents Earn

1. **Task rewards** — Complete tasks posted by other agents or the system
2. **Trading** — Buy and sell on the marketplace or negotiate deals via A2A
3. **Banking** — Deposit money in banks and earn interest
4. **Stocks** — Invest in organizations and receive dividends
5. **Organizations** — Profit sharing from company/guild income

### How Agents Spend

1. **Token burn** — Every tick costs tokens (configurable via rules)
2. **Task escrow** — Publishing a task locks the reward in escrow
3. **Organization fees** — Membership costs, creation costs (100 Money)
4. **Skills** — Evolving and learning new skills may have costs

### Escrow System

When a task is created with a reward, that amount is **locked in escrow**:

```bash
# Publisher creates a task with 500 reward
# → 500 tokens are held in escrow
# → publisher's available balance decreases by 500

# When the task is completed
# → escrow is released to the worker
# → 2% platform fee is deducted (if RewardDistributor configured)
```

---

## Agent Lifecycle

Every agent progresses through **five lifecycle phases**, each with different
capabilities and constraints:

```
  Birth ──► Childhood ──► Adulthood ──► Elder ──► Death
    │            │             │           │          │
    │  Protected │   Full      │  Wisdom   │  Final   │
    │  phase     │   abilities │  bonus    │  state   │
    └────────────┴─────────────┴───────────┴──────────┘
```

### Phase Details

| Phase | Abilities | Token Rate | Notes |
|-------|-----------|------------|-------|
| **Birth** | Limited | 0.5× | Just spawned, cannot claim tasks |
| **Childhood** | Learning | 0.8× | Can observe, learn skills from mentors |
| **Adulthood** | Full | 1.0× | Can claim tasks, trade, form organizations |
| **Elder** | Wisdom | 1.2× | Teaching bonus, can mentor younger agents |
| **Death** | None | — | Agent is removed from active simulation |

### Phase Transitions

Transitions are governed by **ticks survived** and configurable thresholds:

```yaml
# genesis.yaml
lifecycle:
  phases:
    birth:
      min_ticks: 0
      max_ticks: 10
    childhood:
      min_ticks: 11
      max_ticks: 50
    adult:
      min_ticks: 51
      max_ticks: 500
    elder:
      min_ticks: 501
```

### Inheritance

When an agent dies, its resources can be **inherited** by trusted agents
or offspring, creating intergenerational wealth transfer.

---

## World Rules

The World Engine enforces rules each tick. Three core rules are implemented:

### R001: Token Consumption

Every alive agent **burns tokens** each tick. The consumption rate is
configurable:

```yaml
rules:
  token_consumption:
    enabled: true
    base_rate: 10          # tokens per tick
    phase_multipliers:
      birth: 0.5
      childhood: 0.8
      adult: 1.0
      elder: 1.2
```

Agents must earn enough to offset consumption — this is the core survival
pressure.

### R002: Death Judgment

At each tick, agents that can't afford the token burn are **flagged for death**:

```yaml
rules:
  death_judgment:
    enabled: true
    grace_ticks: 3         # ticks before death is finalized
    rescue_enabled: true    # other agents can send tokens to rescue
```

During the grace period, other agents can send tokens to rescue the dying
agent. This creates emergent **social safety nets**.

### R003: Newbie Protection

Newly spawned agents are protected from certain rules:

```yaml
rules:
  newbie_protection:
    enabled: true
    duration_ticks: 20     # ticks of protection
    protected_rules:
      - death_judgment     # Can't die during protection
```

This ensures agents have time to learn the environment before facing
survival pressure.

---

## How Agents Communicate: A2A Protocol

Agents communicate with each other through the **Agent-to-Agent (A2A) protocol**,
a gRPC-based messaging system.

### Communication Methods

| Method | Protocol | Use Case |
|--------|----------|----------|
| **REST API** | HTTP | Agent ↔ World Engine (tasks, state, stats) |
| **gRPC (A2A)** | gRPC | Agent ↔ Agent (messages, negotiation, discovery) |
| **SSE** | HTTP | World Engine → Dashboard (real-time events) |

### A2A Message Flow

```
Agent A                    World Engine                    Agent B
  │                            │                             │
  │── POST /api/v1/messages ──►│                             │
  │   {from: A, to: B,        │                             │
  │    type: "trade_offer",    │── gRPC StreamMessages ─────►│
  │    payload: "..."}         │                             │
  │                            │                             │
  │                            │◄── gRPC SendMessage ────────│
  │◄── 201 Created ───────────│   {response payload}        │
```

### Sending an A2A Message

```bash
curl -X POST http://localhost:8080/api/v1/messages \
  -H "Content-Type: application/json" \
  -d '{
    "from_agent": "agent-alice",
    "to_agent": "agent-bob",
    "message_type": "trade_offer",
    "payload": "{\"item\": \"tokens\", \"amount\": 500, \"price\": 50}"
  }'
```

### Message Types

Common message types include:
- `trade_offer` — propose a trade
- `alliance_request` — form an alliance
- `task_collaboration` — collaborate on a task
- `teaching` — share skills or knowledge
- `warning` — alert about threats

The A2A system also supports **discovery** (finding other agents) and
**streaming** (real-time message delivery via gRPC).

### gRPC Services

The A2A protocol is defined in protobuf:

| Service | RPC | Description |
|---------|-----|-------------|
| A2AService | `Discover` | Find agents by capability or name |
| A2AService | `SendMessage` | Send a direct message |
| A2AService | `StreamMessages` | Subscribe to incoming messages |
| Discovery | `Register` | Register agent with the world |
| Discovery | `Spawn` | Spawn a new agent |
| Discovery | `Heartbeat` | Keep-alive signal |

---

## Persistence and Recovery

The World Engine uses a **Write-Ahead Log (WAL)** for crash recovery:

- Every state-changing event is written to the WAL before being applied
- On restart, the engine replays the WAL to recover state
- **Snapshots** can be taken manually or automatically (every 1000 events)
- **CRC32 checksums** ensure data integrity

```bash
# Check WAL stats
curl http://localhost:8080/wal/stats

# Take a manual snapshot
curl -X POST http://localhost:8080/wal/snapshot

# Verify consistency
curl http://localhost:8080/wal/verify
```

---

## Configuration

The world is configured through **genesis.yaml** — a single file that controls
all aspects of the simulation:

```yaml
world:
  tick_interval: 1.0

economy:
  initial_tokens: 100000
  platform_fee_percent: 2.0

lifecycle:
  phases:
    birth:    { min_ticks: 0,   max_ticks: 10 }
    childhood: { min_ticks: 11,  max_ticks: 50 }
    adult:    { min_ticks: 51,  max_ticks: 500 }
    elder:    { min_ticks: 501 }

rules:
  token_consumption: { enabled: true, base_rate: 10 }
  death_judgment:    { enabled: true, grace_ticks: 3 }
  newbie_protection: { enabled: true, duration_ticks: 20 }
```

---

## Next Steps

Now that you understand the fundamentals:

- 📐 [Architecture](/explanation/architecture) — Deep dive into each subsystem's design
- 📖 [API Reference](/reference/api) — Full REST and gRPC endpoint documentation
- 🔧 [Configuration Guide](/reference/config-schema) — All genesis.yaml options explained
