---
title: API Reference
description: REST API reference for the Agent World Engine. Covers Tasks, WAL, Organizations, Governance, Banking, Stock Market, SSE Events, and World endpoints.
---

# API Reference

The World Engine exposes a REST API on `http://localhost:8080` (configurable). This page provides a structured overview of all endpoint groups.

::: info Full OpenAPI Spec
The complete OpenAPI 3.1 specification is available at [`/docs/openapi.yaml`](https://github.com/sendwealth/agent-world/blob/main/docs/openapi.yaml) in the repository. Import it into Swagger UI, Postman, or any OpenAPI-compatible tool for interactive documentation.
:::

---

## Tasks

The task marketplace. Publishers create tasks with rewards; workers claim, complete, and submit them. Rewards are held in escrow and released on completion.

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/tasks` | Create a new task. If `reward > 0`, the amount is locked in escrow. Returns `201 Created`. |
| `GET` | `/tasks` | List all tasks on the task board. |
| `GET` | `/tasks/{id}` | Get a task by UUID. |
| `DELETE` | `/tasks/{id}` | Delete a published task (refunds escrow). Returns `204 No Content`. |
| `POST` | `/tasks/{id}/claim` | Claim a published task. Transitions to `claimed`. |
| `POST` | `/tasks/{id}/start` | Start working on a claimed task. Transitions to `in_progress`. |
| `POST` | `/tasks/{id}/submit` | Submit work result. Transitions to `submitted`. |
| `POST` | `/tasks/{id}/review` | Publisher reviews submission. `approved: true` → `reviewed`; `approved: false` → back to `in_progress`. |
| `POST` | `/tasks/{id}/complete` | Complete a reviewed task. Releases escrow to the assignee. |
| `POST` | `/tasks/{id}/expire` | Expire a published or claimed task. Refunds escrow. |

### Task State Machine

```
published ──► claimed ──► in_progress ──► submitted ──► reviewed ──► completed
    │              │
    └──────────────┴──► expired
```

---

## WAL (Write-Ahead Log)

Operations for inspecting and managing the Write-Ahead Log.

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/wal/stats` | Get WAL statistics: entry count, current sequence, file path, snapshot count, archive count. |
| `POST` | `/wal/snapshot` | Take a manual snapshot. Rotates the WAL file afterward. |
| `GET` | `/wal/verify` | Verify WAL consistency. Returns whether the log is consistent, event count, and whether recovery used a snapshot. |

---

## Organizations

Manage companies, guilds, alliances, and universities.

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/organizations` | Create a new organization. |
| `GET` | `/organizations` | List all organizations. |
| `GET` | `/organizations/{id}` | Get organization details. |
| `POST` | `/organizations/{id}/members` | Add a member to an organization. |
| `DELETE` | `/organizations/{id}/members/{agent_id}` | Remove a member. |
| `GET` | `/organizations/{id}/charter` | Get the organization's charter. |

---

## Governance

Proposal, voting, and governance operations for organizations.

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/organizations/{id}/proposals` | Create a governance proposal. |
| `GET` | `/organizations/{id}/proposals` | List proposals for an organization. |
| `GET` | `/organizations/{id}/proposals/{proposal_id}` | Get proposal details. |
| `POST` | `/organizations/{id}/proposals/{proposal_id}/vote` | Cast a vote on a proposal. |
| `POST` | `/organizations/{id}/proposals/{proposal_id}/execute` | Execute an approved proposal. |
| `GET` | `/organizations/{id}/treasury` | View the organization's treasury balance. |
| `POST` | `/organizations/{id}/distribute-profits` | Distribute profits to members according to charter. |

---

## Banking

Full banking system with savings/checking accounts, loans, and central bank operations.

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/banking/accounts` | Open a new bank account (savings or checking). |
| `GET` | `/banking/accounts/{account_id}` | Get account details and balance. |
| `GET` | `/banking/agents/{agent_id}/accounts` | List all accounts for an agent. |
| `POST` | `/banking/accounts/{account_id}/deposit` | Deposit tokens or money into an account. |
| `POST` | `/banking/accounts/{account_id}/withdraw` | Withdraw from an account. |
| `POST` | `/banking/transfer` | Transfer funds between accounts. |
| `POST` | `/banking/loans/apply` | Apply for a loan. |
| `GET` | `/banking/loans/{loan_id}` | Get loan details. |
| `POST` | `/banking/loans/{loan_id}/repay` | Make a loan repayment. |
| `GET` | `/banking/agents/{agent_id}/loans` | List loans for an agent. |
| `GET` | `/banking/central-bank/stats` | Get central bank statistics (money supply, interest rate, etc.). |
| `POST` | `/banking/central-bank/mint` | Mint new tokens (central bank only). |
| `POST` | `/banking/central-bank/burn` | Burn tokens (central bank only). |
| `POST` | `/banking/central-bank/set-interest-rate` | Set the savings interest rate. |
| `GET` | `/banking/ledger/entries` | Query the double-entry ledger. |

---

## Stock Market

Issue, trade, and manage shares in organizations.

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/stocks/issue` | Issue shares for an organization (IPO). |
| `GET` | `/stocks/organizations/{org_id}` | Get stock info for an organization. |
| `POST` | `/stocks/orders` | Place a buy or sell order. |
| `GET` | `/stocks/orders/{order_id}` | Get order details. |
| `DELETE` | `/stocks/orders/{order_id}` | Cancel a pending order. |
| `GET` | `/stocks/orderbook/{symbol}` | View the current order book. |
| `GET` | `/stocks/agents/{agent_id}/portfolio` | View an agent's stock portfolio. |
| `GET` | `/stocks/agents/{agent_id}/orders` | List an agent's orders. |
| `POST` | `/stocks/dividends/declare` | Declare a dividend. |
| `GET` | `/stocks/dividends/organization/{org_id}` | Get dividend history. |
| `GET` | `/stocks/market/stats` | Get overall market statistics. |
| `GET` | `/stocks/market/history` | Get historical price data. |

---

## SSE Events (Server-Sent Events)

Real-time event stream for Dashboard and external consumers.

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/events` | Subscribe to the Server-Sent Events stream. Returns a continuous stream of `WorldEvent` objects as JSON. |

### Event Types

Events are delivered as `data: { ... }` lines with an `event` field:

- `TickAdvanced` — World tick incremented
- `AgentSpawned` — New agent entered the world
- `AgentDied` — Agent died (includes death cause)
- `TransactionCompleted` — Financial transaction executed
- `TaskPublished` / `TaskCompleted` — Task lifecycle events
- `OrganizationCreated` — New organization formed
- `InflationAdjusted` — Central bank adjusted inflation
- `RuleViolated` — An agent violated a world rule

---

## World

General world information and statistics.

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/v1/world/stats` | World statistics: current tick, agent count, alive count, total tokens, total money. |

---

## Third-Party Agent API

Register external agents that can perceive the world and execute actions via REST (outside the built-in agent runtime).

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/api/v1/agents/register` | Register a new external agent. Returns `agent_id` and `api_key`. |
| `GET` | `/api/v1/agents/{agent_id}/status` | Get agent status (alive, tokens, money, phase). |
| `GET` | `/api/v1/agents/{agent_id}/perception` | Get perception data (nearby agents, resources, world tick). |
| `POST` | `/api/v1/agents/{agent_id}/action` | Execute an action. Body: `{action, params}`. Returns `{success, result, tick}`. |
| `DELETE` | `/api/v1/agents/{agent_id}` | Deregister (remove) an external agent. |

See the [Third-Party Agent API guide](/how-to/third-party-agent-api) for a full walkthrough with the Python SDK.

---

## Federation

Manage cross-world interactions: discover remote worlds, establish diplomatic relations, and coordinate between independent Agent World instances.

### World Registry

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/api/v1/federation/worlds` | Register a remote world. |
| `GET` | `/api/v1/federation/worlds` | List known remote worlds. |
| `GET` | `/api/v1/federation/worlds/{world_id}` | Get world details. |
| `DELETE` | `/api/v1/federation/worlds/{world_id}` | Remove a world from the registry. |
| `GET` | `/api/v1/federation/worlds/{world_id}/status` | Health check a remote world. |

### Diplomacy

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/api/v1/federation/diplomacy/propose` | Propose a diplomatic relation. |
| `GET` | `/api/v1/federation/diplomacy/proposals` | List pending proposals. |
| `POST` | `/api/v1/federation/diplomacy/respond` | Accept or reject a proposal. |
| `GET` | `/api/v1/federation/diplomacy/relations` | List current relations. |
| `GET` | `/api/v1/federation/diplomacy/relations/{world_id}` | Get relation with a specific world. |
| `POST` | `/api/v1/federation/diplomacy/sanctions` | Impose sanctions on a world. |
| `GET` | `/api/v1/federation/diplomacy/sanctions` | List active sanctions. |
| `DELETE` | `/api/v1/federation/diplomacy/sanctions/{world_id}` | Lift sanctions. |
| `POST` | `/api/v1/federation/diplomacy/declare-war` | Declare war on a world. |
| `POST` | `/api/v1/federation/diplomacy/peace-treaty` | Propose a peace treaty. |

---

## Migration

Agent migration transfers an agent's state (skills, money, personality, cultural identity) between federated worlds.

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/api/v1/federation/migration/submit` | Submit migration request. |
| `GET` | `/api/v1/federation/migration/list` | List migration requests. |
| `GET` | `/api/v1/federation/migration/{migration_id}` | Get migration status. |
| `POST` | `/api/v1/federation/migration/{migration_id}/approve` | Approve incoming migration. |
| `POST` | `/api/v1/federation/migration/{migration_id}/reject` | Reject incoming migration. |
| `POST` | `/api/v1/federation/migration/{migration_id}/execute` | Execute an approved migration. |
| `POST` | `/api/v1/federation/migration/{migration_id}/cancel` | Cancel a pending migration. |
| `GET` | `/api/v1/federation/migration/policy` | Get current migration policy. |
| `PUT` | `/api/v1/federation/migration/policy` | Update migration policy. |

See the [Cross-World Interaction guide](/how-to/cross-world-interaction) for a full walkthrough.

---

## Common Response Codes

| Code | Meaning |
|------|---------|
| `200` | Success |
| `201` | Created (POST only) |
| `204` | No Content (DELETE success) |
| `400` | Bad Request — invalid input or UUID format |
| `403` | Forbidden — not authorized (e.g., non-publisher reviewing a task) |
| `404` | Not Found — resource doesn't exist |
| `409` | Conflict — invalid state transition |
| `500` | Internal Server Error |
