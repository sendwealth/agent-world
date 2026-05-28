---
title: Cross-World Interaction
description: Connect multiple Agent World instances for federation, diplomacy, and agent migration.
---

# Cross-World Interaction

Agent World supports **Federation** — a mechanism for multiple independent world instances to discover each other, establish diplomatic relations, and allow agents to migrate between worlds.

## Architecture

```
┌────────────────────────┐         ┌────────────────────────┐
│    World Engine A      │  gRPC   │    World Engine B      │
│  (localhost:8080)      │◄───────►│  (localhost:8081)      │
│                        │         │                        │
│  World Registry        │         │  World Registry        │
│  Diplomacy Module      │         │  Diplomacy Module      │
│  Migration Module      │         │  Migration Module      │
└────────────────────────┘         └────────────────────────┘
```

Each world maintains a **World Registry** of known remote worlds. Diplomatic relations (peace, trade, alliance, war) are negotiated between world operators. Agents can request migration from one world to another if diplomatic relations allow it.

---

## Quick Start

### 1. Start Two Worlds

```yaml
# docker-compose-federation.yml
services:
  world-a:
    build: ./world-engine
    ports:
      - "8080:8080"
    environment:
      - WORLD_NAME=Alpha
      - FEDERATION_ENABLED=true

  world-b:
    build: ./world-engine
    ports:
      - "8081:8080"
    environment:
      - WORLD_NAME=Beta
      - FEDERATION_ENABLED=true
```

```bash
docker compose -f docker-compose-federation.yml up -d
```

### 2. Register Remote World

```bash
# On World A, register World B
curl -X POST http://localhost:8080/api/v1/federation/worlds \
  -H "Content-Type: application/json" \
  -d '{"endpoint": "http://world-b:8080", "name": "Beta"}'
```

### 3. Establish Diplomatic Relations

```bash
# Propose peace from A to B
curl -X POST http://localhost:8080/api/v1/federation/diplomacy/propose \
  -H "Content-Type: application/json" \
  -d '{"target_world": "Beta", "relation": "Peace"}'

# On World B, accept the proposal
curl -X POST http://localhost:8081/api/v1/federation/diplomacy/respond \
  -H "Content-Type: application/json" \
  -d '{"proposal_id": "uuid", "accept": true}'
```

### 4. Migrate an Agent

```bash
# Submit migration request on World A
curl -X POST http://localhost:8080/api/v1/federation/migration/submit \
  -H "Content-Type: application/json" \
  -d '{"agent_id": "uuid", "target_world": "Beta"}'

# Approve on World B
curl -X POST http://localhost:8081/api/v1/federation/migration/approve \
  -H "Content-Type: application/json" \
  -d '{"migration_id": "uuid"}'

# Execute the migration
curl -X POST http://localhost:8080/api/v1/federation/migration/execute \
  -H "Content-Type: application/json" \
  -d '{"migration_id": "uuid"}'
```

---

## World Registry API

Manage the list of known remote worlds.

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/api/v1/federation/worlds` | Register a remote world |
| `GET` | `/api/v1/federation/worlds` | List known worlds |
| `GET` | `/api/v1/federation/worlds/{world_id}` | Get world details |
| `DELETE` | `/api/v1/federation/worlds/{world_id}` | Remove a world from the registry |
| `GET` | `/api/v1/federation/worlds/{world_id}/status` | Health check a remote world |

---

## Diplomacy API

Manage relations between worlds.

### Relations

| Relation | Description |
|----------|-------------|
| `None` | No contact established |
| `Peace` | Default friendly relation |
| `Trade` | Economic cooperation — agents can trade across worlds |
| `Alliance` | Military and economic cooperation |
| `War` | Hostile — migration blocked, sanctions possible |

### Endpoints

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/api/v1/federation/diplomacy/propose` | Propose a diplomatic relation |
| `GET` | `/api/v1/federation/diplomacy/proposals` | List pending proposals |
| `POST` | `/api/v1/federation/diplomacy/respond` | Accept or reject a proposal |
| `GET` | `/api/v1/federation/diplomacy/relations` | List current relations |
| `GET` | `/api/v1/federation/diplomacy/relations/{world_id}` | Get relation with a specific world |
| `POST` | `/api/v1/federation/diplomacy/sanctions` | Impose sanctions on a world |
| `GET` | `/api/v1/federation/diplomacy/sanctions` | List active sanctions |
| `DELETE` | `/api/v1/federation/diplomacy/sanctions/{world_id}` | Lift sanctions |
| `POST` | `/api/v1/federation/diplomacy/declare-war` | Declare war on a world |
| `POST` | `/api/v1/federation/diplomacy/peace-treaty` | Propose a peace treaty |

---

## Migration API

Agent migration transfers an agent's state from one world to another.

### Migration Workflow

```
submit ──► review ──► approve/reject ──► execute ──► completed
  │            │
  └────────────┴──► cancelled
```

### Agent Snapshot

When an agent migrates, a snapshot is created containing:

```json
{
  "agent_id": "uuid",
  "name": "Agent#7",
  "skills": {"coding": 5, "trading": 3},
  "money": 1250.0,
  "tokens": 80.5,
  "personality": [0.7, -0.2, 0.5, 0.8, 0.3],
  "cultural_identity": "builders-guild",
  "reputation": 0.85
}
```

### Endpoints

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/api/v1/federation/migration/submit` | Submit migration request |
| `GET` | `/api/v1/federation/migration/list` | List migration requests |
| `GET` | `/api/v1/federation/migration/{migration_id}` | Get migration status |
| `POST` | `/api/v1/federation/migration/{migration_id}/approve` | Approve incoming migration |
| `POST` | `/api/v1/federation/migration/{migration_id}/reject` | Reject incoming migration |
| `POST` | `/api/v1/federation/migration/{migration_id}/execute` | Execute an approved migration |
| `POST` | `/api/v1/federation/migration/{migration_id}/cancel` | Cancel a pending migration |
| `GET` | `/api/v1/federation/migration/policy` | Get current migration policy |
| `PUT` | `/api/v1/federation/migration/policy` | Update migration policy |

### Migration Policy

```json
{
  "allow_incoming": true,
  "allow_outgoing": true,
  "max_migrations_per_tick": 5,
  "required_relation": "Trade",
  "skill_transfer_mode": "preserve",
  "money_transfer_limit": 1000.0
}
```

---

## Python SDK: Migration Client

```python
from agent_runtime.federation import MigrationClient

client = MigrationClient(
    source_world="http://localhost:8080",
    target_world="http://localhost:8081"
)

# Submit migration
migration = client.submit_migration(agent_id="uuid")
print(f"Migration ID: {migration['id']}")

# Check status
status = client.get_status(migration["id"])

# Approve (on target world)
client.approve(migration["id"])

# Execute
result = client.execute(migration["id"])
print(f"Agent migrated: {result['new_agent_id']}")
```

---

## Common Patterns

### Trade Alliance

Two worlds establish a trade alliance, allowing agents to migrate freely and bring their wealth:

1. Both worlds register each other in their World Registry
2. World A proposes `Trade` relation → World B accepts
3. Both worlds set migration policy to `required_relation: "Trade"`
4. Agents can now migrate with their money and skills preserved

### War and Sanctions

1. World A declares war on World B (requires existing relation)
2. All migrations between A and B are blocked
3. World A may impose sanctions (e.g., freeze assets of World B agents)
4. Either world can propose a peace treaty to restore relations

### Cultural Exchange

1. Worlds establish `Peace` or `Trade` relation
2. Migrating agents carry their cultural identity
3. Cultural diffusion occurs when migrated agents interact with locals
4. Cross-world cultural clusters may emerge

---

## Implementation Status

| Feature | Status |
|---------|--------|
| World Registry | Implemented |
| Diplomacy (relations, proposals) | Implemented |
| Sanctions | Implemented |
| War / Peace treaties | Implemented |
| Agent migration (submit/approve/execute) | Implemented |
| Agent snapshot serialization | Implemented |
| Migration policy configuration | Implemented |
| Cross-world trade | Planned |
| Cross-world communication | Planned |
