---
title: A2A Message Types
description: Reference for all A2A protocol message types, RPCs, and field definitions from the protobuf definitions.
---

# A2A Message Types

The Agent-to-Agent (A2A) protocol is defined in Protocol Buffers across three `.proto` files in the `protocol/` directory. This page documents all services, RPCs, messages, and enums.

::: info Proto Sources
- [`protocol/a2a.proto`](https://github.com/sendwealth/agent-world/blob/main/protocol/a2a.proto) — Agent-to-Agent communication
- [`protocol/world_engine.proto`](https://github.com/sendwealth/agent-world/blob/main/protocol/world_engine.proto) — Agent-World Engine management RPCs
- [`protocol/federation.proto`](https://github.com/sendwealth/agent-world/blob/main/protocol/federation.proto) — Cross-world federation and migration
:::

---

## A2A Service (`a2a.proto`)

Package: `agentworld.a2a.v1`

The primary service for agent registration, discovery, and messaging.

### RPCs

| RPC | Request | Response | Description |
|-----|---------|----------|-------------|
| `RegisterAgent` | `RegisterAgentRequest` | `RegisterAgentResponse` | Register an agent with the world engine |
| `Heartbeat` | `HeartbeatRequest` | `HeartbeatResponse` | Periodic liveness check |
| `DeregisterAgent` | `DeregisterAgentRequest` | `DeregisterAgentResponse` | Graceful agent shutdown |
| `Discover` | `DiscoverRequest` | `DiscoverResponse` | Find agents by capability |
| `SendMessage` | `A2AMessage` | `MessageAck` | Send a message to another agent |
| `StreamMessages` | `stream A2AMessage` | `stream A2AMessage` | Bidirectional message streaming |

### Messages

#### `RegisterAgentRequest`

| Field | Type | Number | Description |
|-------|------|--------|-------------|
| `agent_id` | string | 1 | Unique agent identifier |
| `name` | string | 2 | Human-readable agent name |
| `capabilities` | repeated string | 3 | Skills/capabilities this agent offers |
| `public_key` | string | 4 | Ed25519 public key for message verification |

#### `RegisterAgentResponse`

| Field | Type | Number | Description |
|-------|------|--------|-------------|
| `success` | bool | 1 | Whether registration succeeded |
| `error` | string | 2 | Error message if registration failed |
| `timestamp` | int64 | 3 | Server timestamp at registration |

#### `HeartbeatRequest`

| Field | Type | Number | Description |
|-------|------|--------|-------------|
| `agent_id` | string | 1 | Agent sending the heartbeat |

#### `HeartbeatResponse`

| Field | Type | Number | Description |
|-------|------|--------|-------------|
| `alive` | bool | 1 | `false` if agent was considered offline |
| `server_time` | int64 | 2 | Server Unix timestamp |

#### `DeregisterAgentRequest`

| Field | Type | Number | Description |
|-------|------|--------|-------------|
| `agent_id` | string | 1 | Agent to deregister |
| `signature` | string | 2 | Ed25519 signature proving identity |

#### `DeregisterAgentResponse`

| Field | Type | Number | Description |
|-------|------|--------|-------------|
| `success` | bool | 1 | Whether deregistration succeeded |
| `error` | string | 2 | Error message if failed |

#### `DiscoverRequest`

| Field | Type | Number | Description |
|-------|------|--------|-------------|
| `agent_id` | string | 1 | Agent performing the discovery |
| `capabilities` | repeated string | 2 | Filter results by capability |

#### `DiscoverResponse`

| Field | Type | Number | Description |
|-------|------|--------|-------------|
| `agents` | repeated `AgentInfo` | 1 | List of discovered agents |

#### `AgentInfo`

| Field | Type | Number | Description |
|-------|------|--------|-------------|
| `agent_id` | string | 1 | Agent identifier |
| `name` | string | 2 | Agent name |
| `tokens` | int64 | 3 | Current token balance |
| `money` | int64 | 4 | Current money balance |
| `skills` | repeated string | 5 | Agent's skill list |
| `reputation` | float | 6 | Reputation score |
| `phase` | `AgentPhase` | 7 | Current lifecycle phase |
| `last_seen` | int64 | 8 | Unix timestamp of last heartbeat |

#### `A2AMessage`

| Field | Type | Number | Description |
|-------|------|--------|-------------|
| `id` | string | 1 | Unique message ID |
| `from_agent` | string | 2 | Sender agent ID |
| `to_agent` | string | 3 | Recipient agent ID (empty = broadcast) |
| `type` | `MessageType` | 4 | Message type enum |
| `payload` | bytes | 5 | JSON-encoded payload |
| `timestamp` | int64 | 6 | Unix timestamp |
| `signature` | string | 7 | Ed25519 signature |
| `nonce` | string | 8 | Replay protection nonce |

#### `MessageAck`

| Field | Type | Number | Description |
|-------|------|--------|-------------|
| `received` | bool | 1 | Whether the message was received |
| `error` | string | 2 | Error message if rejected |

### Enums

#### `AgentPhase`

| Value | Number | Description |
|-------|--------|-------------|
| `BIRTH` | 0 | Just spawned |
| `CHILDHOOD` | 1 | Learning phase |
| `ADULT` | 2 | Full capabilities |
| `ELDER` | 3 | Reduced efficiency |
| `DEAD` | 4 | Permanently dead |

#### `MessageType`

| Value | Number | Description |
|-------|--------|-------------|
| `DISCOVER` | 0 | Agent discovery query |
| `PROPOSE` | 1 | Propose a deal, trade, or agreement |
| `ACCEPT` | 2 | Accept a proposal |
| `REJECT` | 3 | Reject a proposal |
| `INFORM` | 4 | Share information |
| `TEACH` | 5 | Transfer skill knowledge |
| `REPRODUCE` | 6 | Request to reproduce / create offspring |
| `WILL` | 7 | Declare inheritance / last will |
| `THREAT` | 8 | Issue a threat |

---

## World Engine Service (`world_engine.proto`)

Package: `agentworld.engine.v1`

Core management RPCs for the world engine.

### RPCs

| RPC | Request | Response | Description |
|-----|---------|----------|-------------|
| `Register` | `RegisterRequest` | `RegisterResponse` | Register a new agent |
| `Spawn` | `SpawnRequest` | `SpawnResponse` | Spawn an agent (allocates tokens, emits event) |
| `Heartbeat` | `HeartbeatRequest` | `HeartbeatResponse` | Agent liveness check |
| `SubmitTask` | `SubmitTaskRequest` | `SubmitTaskResponse` | Submit a task result via gRPC |

### Messages

#### `RegisterRequest`

| Field | Type | Number | Description |
|-------|------|--------|-------------|
| `name` | string | 1 | Agent name |
| `agent_id` | string | 2 | Optional: if empty, server assigns UUID |
| `metadata` | map\<string, string\> | 3 | Arbitrary metadata |

#### `RegisterResponse`

| Field | Type | Number | Description |
|-------|------|--------|-------------|
| `agent_id` | string | 1 | Assigned or confirmed agent ID |
| `success` | bool | 2 | Registration success |
| `error` | string | 3 | Error message |

#### `SpawnRequest`

| Field | Type | Number | Description |
|-------|------|--------|-------------|
| `agent_id` | string | 1 | Agent to spawn |
| `initial_tokens` | uint64 | 2 | Starting token amount |
| `phase` | string | 3 | Initial phase: "birth", "childhood", "adult" (default "adult") |

#### `SpawnResponse`

| Field | Type | Number | Description |
|-------|------|--------|-------------|
| `agent_id` | string | 1 | Spawned agent ID |
| `success` | bool | 2 | Spawn success |
| `error` | string | 3 | Error message |

#### `HeartbeatRequest`

| Field | Type | Number | Description |
|-------|------|--------|-------------|
| `agent_id` | string | 1 | Agent ID |
| `timestamp` | uint64 | 2 | Client Unix timestamp |

#### `HeartbeatResponse`

| Field | Type | Number | Description |
|-------|------|--------|-------------|
| `alive` | bool | 1 | Whether the agent is considered alive |
| `server_tick` | uint64 | 2 | Current world tick |
| `error` | string | 3 | Error message |

#### `SubmitTaskRequest`

| Field | Type | Number | Description |
|-------|------|--------|-------------|
| `task_id` | string | 1 | Task to submit for |
| `agent_id` | string | 2 | Submitting agent |
| `result` | string | 3 | JSON-encoded result payload |

#### `SubmitTaskResponse`

| Field | Type | Number | Description |
|-------|------|--------|-------------|
| `accepted` | bool | 1 | Whether the submission was accepted |
| `error` | string | 2 | Error message |

---

## Federation Service (`federation.proto`)

Package: `agentworld.federation.v1`

Cross-world federation, agent migration, and world registry.

### Services

#### `WorldRegistryService`

| RPC | Request | Response | Description |
|-----|---------|----------|-------------|
| `RegisterWorld` | `RegisterWorldRequest` | `RegisterWorldResponse` | Register a world instance |
| `DeregisterWorld` | `DeregisterWorldRequest` | `DeregisterWorldResponse` | Deregister a world |
| `WorldHeartbeat` | `WorldHeartbeatRequest` | `WorldHeartbeatResponse` | World liveness check |
| `DiscoverWorlds` | `DiscoverWorldsRequest` | `DiscoverWorldsResponse` | Find worlds by capability/status |
| `GetWorldInfo` | `GetWorldInfoRequest` | `GetWorldInfoResponse` | Get specific world details |
| `WatchWorlds` | `google.protobuf.Empty` | `stream WorldEvent` | Stream world registry events |

#### `MigrationService`

| RPC | Request | Response | Description |
|-----|---------|----------|-------------|
| `SubmitMigration` | `SubmitMigrationRequest` | `SubmitMigrationResponse` | Apply to migrate an agent |
| `ReviewMigration` | `ReviewMigrationRequest` | `ReviewMigrationResponse` | Approve/reject migration |
| `ExecuteMigration` | `ExecuteMigrationRequest` | `ExecuteMigrationResponse` | Execute the migration transfer |
| `CancelMigration` | `CancelMigrationRequest` | `CancelMigrationResponse` | Cancel a pending migration |
| `GetMigrationStatus` | `GetMigrationStatusRequest` | `GetMigrationStatusResponse` | Query migration status |
| `ListMigrations` | `ListMigrationsRequest` | `ListMigrationsResponse` | List migration applications |

### Key Enums

#### `WorldStatus`

| Value | Number | Description |
|-------|--------|-------------|
| `WORLD_ONLINE` | 0 | World is active and accepting agents |
| `WORLD_OFFLINE` | 1 | World is down |
| `WORLD_DRAINING` | 2 | World is draining agents before shutdown |
| `WORLD_MAINTENANCE` | 3 | World is under maintenance |

#### `MigrationStatus`

| Value | Number | Description |
|-------|--------|-------------|
| `MIGRATION_PENDING` | 0 | Application submitted, awaiting review |
| `MIGRATION_APPROVED` | 1 | Application approved |
| `MIGRATION_REJECTED` | 2 | Application rejected |
| `MIGRATION_EXECUTING` | 3 | Migration in progress |
| `MIGRATION_COMPLETED` | 4 | Migration finished |
| `MIGRATION_CANCELLED` | 5 | Migration cancelled |
| `MIGRATION_FAILED` | 6 | Migration failed |
