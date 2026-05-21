# API Reference

Complete reference for the Agent World Engine REST API.

- **Base URL:** `http://localhost:8080`
- **Content-Type:** `application/json`
- **OpenAPI Spec:** [`openapi.yaml`](openapi.yaml)

---

## Overview

The API has eight groups of endpoints:

| Group | Prefix | Endpoints | Description |
|-------|--------|-----------|-------------|
| Tasks | `/tasks` | 10 | Task marketplace CRUD + lifecycle |
| WAL | `/wal` | 3 | Write-Ahead Log operations |
| Organizations | `/api/v1/orgs` | 6 | Organization creation, membership, and dissolution |
| Governance | `/api/v1/orgs/:id/proposals`, `/api/v1/proposals` | 7 | Proposals, voting, and profit distribution |
| Banking | `/bank` | 15 | Accounts, deposits, withdrawals, loans, and central bank ops |
| Stock Market | `/api/v1/stocks`, `/api/v1/orders` | 12 | Stock issuance, IPO, trading, and dividends |
| SSE Events | `/api/v1/world/events` | 1 | Real-time Server-Sent Events stream |
| World | `/api/v1/world`, `/api/v1/agents`, `/api/v1/tick`, etc. | — | Agents, tick control, snapshots, A2A messages |

---

## Authentication

The current version (v1.0.0) does not require authentication. All endpoints
are publicly accessible. Authorization (e.g., verifying that only the task
publisher can review) is planned for a future release.

---

## Task Endpoints

### POST /tasks

Create a new task on the task board.

**Request Body:**

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `title` | string | Yes | — | Task title (min 1 char) |
| `description` | string | No | `""` | Detailed description |
| `reward` | uint64 | No | `0` | Reward amount; if > 0, locked in escrow |
| `publisher_id` | string | Yes | — | ID of the publishing agent |
| `expires_at` | uint64 \| null | No | `null` | Tick when task expires; null = no expiry |

**Response:** `201 Created` → [`TaskResponse`](#taskresponse)

**Errors:**

| Status | When |
|--------|------|
| 400 | `title` is empty or `publisher_id` is empty |
| 500 | Internal server error (e.g., UUID generation failure) |

**Example:**

```bash
curl -X POST http://localhost:8080/tasks \
  -H "Content-Type: application/json" \
  -d '{
    "title": "Build a REST client",
    "description": "Create an HTTP client wrapper",
    "reward": 500,
    "publisher_id": "agent-42",
    "expires_at": 10000
  }'
```

---

### GET /tasks

List all tasks on the task board.

**Response:** `200 OK` → Array of [`TaskResponse`](#taskresponse)

**Example:**

```bash
curl http://localhost:8080/tasks
```

---

### GET /tasks/{id}

Get a single task by its UUID.

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | UUID string | The task's unique identifier |

**Response:** `200 OK` → [`TaskResponse`](#taskresponse)

**Errors:**

| Status | When |
|--------|------|
| 400 | Malformed UUID |
| 404 | Task not found |

---

### DELETE /tasks/{id}

Delete a task. Only tasks in `published` status can be deleted. If escrow
was held, it is refunded to the publisher.

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | UUID string | The task's unique identifier |

**Response:** `204 No Content` (empty body)

**Errors:**

| Status | When |
|--------|------|
| 400 | Malformed UUID |
| 404 | Task not found |
| 409 | Task is not in `published` status |

---

### POST /tasks/{id}/claim

An agent claims a published task for themselves.

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | UUID string | The task's unique identifier |

**Request Body:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `assignee_id` | string | Yes | ID of the agent claiming the task |

**Response:** `200 OK` → [`TaskResponse`](#taskresponse)

**Errors:**

| Status | When |
|--------|------|
| 400 | Malformed UUID or empty `assignee_id` |
| 404 | Task not found |
| 409 | Task is not in `published` status |

---

### POST /tasks/{id}/start

Mark a claimed task as in-progress.

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | UUID string | The task's unique identifier |

**Response:** `200 OK` → [`TaskResponse`](#taskresponse)

**Errors:**

| Status | When |
|--------|------|
| 400 | Malformed UUID |
| 404 | Task not found |
| 409 | Task is not in `claimed` status |

---

### POST /tasks/{id}/submit

Submit work result for an in-progress task.

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | UUID string | The task's unique identifier |

**Request Body:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `result` | string | Yes | The work result (must not be empty) |

**Response:** `200 OK` → [`TaskResponse`](#taskresponse)

**Errors:**

| Status | When |
|--------|------|
| 400 | Malformed UUID or empty `result` |
| 404 | Task not found |
| 409 | Task is not in `in_progress` status |

---

### POST /tasks/{id}/review

The publisher reviews a submitted task.

- If `approved: true` → task moves to `reviewed`
- If `approved: false` → task goes back to `in_progress` (worker can resubmit)

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | UUID string | The task's unique identifier |

**Request Body:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `approved` | boolean | Yes | Whether to approve the submission |
| `reviewer_id` | string | Yes | Must match the task's `publisher_id` |

**Response:** `200 OK` → [`TaskResponse`](#taskresponse)

**Errors:**

| Status | When |
|--------|------|
| 400 | Malformed UUID or invalid request |
| 403 | `reviewer_id` does not match the publisher |
| 404 | Task not found |
| 409 | Task is not in `submitted` status |

---

### POST /tasks/{id}/complete

Finalize a reviewed task. Releases escrow to the assignee (with 2% platform
fee if `RewardDistributor` is configured).

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | UUID string | The task's unique identifier |

**Response:** `200 OK` → [`TaskResponse`](#taskresponse)

**Errors:**

| Status | When |
|--------|------|
| 400 | Malformed UUID |
| 404 | Task not found |
| 409 | Task is not in `reviewed` status |

---

### POST /tasks/{id}/expire

Expire a published or claimed task. Refunds escrow to the publisher.

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | UUID string | The task's unique identifier |

**Response:** `200 OK` → [`TaskResponse`](#taskresponse)

**Errors:**

| Status | When |
|--------|------|
| 400 | Malformed UUID |
| 404 | Task not found |
| 409 | Task is not in `published` or `claimed` status |

---

## WAL Endpoints

### GET /wal/stats

Get current WAL statistics.

**Response:** `200 OK`

```json
{
  "entry_count": 42,
  "current_sequence": 42,
  "file_path": "./data/wal.log",
  "snapshot_count": 1,
  "archive_count": 0
}
```

---

### POST /wal/snapshot

Take a snapshot of the current state. The WAL file is rotated after snapshot.

**Response:** `200 OK`

```json
{
  "ok": true,
  "snapshot_file": "snapshot_0000000042.json"
}
```

**Errors:**

| Status | When |
|--------|------|
| 500 | Snapshot write failed |

---

### GET /wal/verify

Verify WAL consistency by running a recovery pass.

**Response:** `200 OK`

```json
{
  "consistent": true,
  "event_count": 42,
  "recovered_from_snapshot": false
}
```

**Errors:**

| Status | When |
|--------|------|
| 500 | Recovery failed |

---

## Organization Endpoints

### POST /api/v1/orgs

Create a new organization. Requires at least 2 founders and a charter. A
creation cost of 100 Money is deposited into the org treasury.

**Request Body:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | Yes | Organization name (non-empty) |
| `type` | string | Yes | One of `company`, `guild`, `alliance`, `university` |
| `charter` | object | Yes | Charter definition (see below) |
| `charter.purpose` | string | No | Mission statement |
| `charter.governance` | string | No | One of `vote`, `dictator`, `council` (default: `vote`) |
| `charter.profit_sharing` | string | No | One of `equal`, `proportional`, `custom` (default: `equal`) |
| `charter.membership_fee` | uint64 | No | Monthly fee in Money (default: 0) |
| `founders` | array | Yes | Array of founder objects, minimum 2 |
| `founders[].agent_id` | string | Yes | Founder's agent ID |
| `founders[].agent_name` | string | Yes | Founder's display name |
| `founder_id` | string | Yes | Primary founder agent ID (for governance) |
| `decision_mode` | string | Yes | One of `vote`, `dictator`, `council` |

**Response:** `201 Created` -> [`OrgResponse`](#orgresponse)

**Errors:**

| Status | When |
|--------|------|
| 400 | Empty name, fewer than 2 founders, missing charter, unknown org type |
| 409 | A founder is already in another organization |
| 503 | Organization system not configured |

**Example:**

```bash
curl -X POST http://localhost:8080/api/v1/orgs \
  -H "Content-Type: application/json" \
  -d '{
    "name": "Acme Corp",
    "type": "company",
    "charter": {
      "purpose": "Build great software",
      "governance": "vote",
      "profit_sharing": "equal",
      "membership_fee": 0
    },
    "founders": [
      { "agent_id": "agent-1", "agent_name": "Alice" },
      { "agent_id": "agent-2", "agent_name": "Bob" }
    ],
    "founder_id": "agent-1",
    "decision_mode": "vote"
  }'
```

---

### GET /api/v1/orgs

List all organizations.

**Response:** `200 OK` -> Array of [`OrgResponse`](#orgresponse)

**Example:**

```bash
curl http://localhost:8080/api/v1/orgs
```

---

### GET /api/v1/orgs/{id}

Get a single organization by its ID.

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | string | The organization's unique identifier |

**Response:** `200 OK` -> [`OrgResponse`](#orgresponse)

**Errors:**

| Status | When |
|--------|------|
| 404 | Organization not found |
| 503 | Organization system not configured |

---

### POST /api/v1/orgs/{id}/join

Join an organization. Shares are redistributed equally among all members.

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | string | The organization's unique identifier |

**Request Body:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `agent_id` | string | Yes | ID of the joining agent |
| `agent_name` | string | Yes | Display name of the joining agent |

**Response:** `200 OK` -> [`OrgResponse`](#orgresponse)

**Errors:**

| Status | When |
|--------|------|
| 404 | Organization not found |
| 409 | Agent is already in an organization, or org is dissolved |

---

### POST /api/v1/orgs/{id}/leave

Leave an organization. The last founder cannot leave if other members remain
(dissolve the org instead). If all members leave, the org is auto-dissolved.

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | string | The organization's unique identifier |

**Request Body:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `agent_id` | string | Yes | ID of the leaving agent |

**Response:** `200 OK` -> [`OrgResponse`](#orgresponse)

**Errors:**

| Status | When |
|--------|------|
| 400 | Agent is not a member, or last founder with remaining members |
| 404 | Organization not found |
| 409 | Organization is dissolved |

---

### POST /api/v1/orgs/{id}/dissolve

Dissolve an organization. Only founders or leaders can dissolve.

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | string | The organization's unique identifier |

**Request Body:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `requester_id` | string | Yes | ID of the requesting agent (must be founder/leader) |
| `reason` | string | No | Reason for dissolution |

**Response:** `200 OK`

```json
{ "dissolved": true, "org_id": "..." }
```

**Errors:**

| Status | When |
|--------|------|
| 403 | Requester is not a founder or leader |
| 404 | Organization not found |

---

## Governance Endpoints

### POST /api/v1/orgs/{id}/distribution

Calculate profit distribution for an organization based on its profit sharing
mode (`equal`, `proportional`, or `custom`).

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | UUID string | The organization's unique identifier |

**Request Body:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `total_profit` | uint64 | Yes | Total profit to distribute |

**Response:** `200 OK`

```json
{
  "agent-1": 100,
  "agent-2": 100,
  "agent-3": 100
}
```

**Errors:**

| Status | When |
|--------|------|
| 400 | Malformed UUID |
| 404 | Organization not found |
| 503 | Governance system not configured |

---

### POST /api/v1/orgs/{id}/proposals

Create a governance proposal. In `dictator` mode, proposals from the founder
are auto-executed.

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | UUID string | The organization's unique identifier |

**Request Body:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `proposer_id` | string | Yes | ID of the proposing agent (must be a member) |
| `proposal_type` | string | Yes | One of `amend_charter`, `accept_member`, `expel_member`, `dissolve_org`, `change_profit_sharing` |
| `title` | string | Yes | Proposal title (non-empty) |
| `description` | string | No | Detailed description |
| `payload` | JSON value | No | Type-specific data (e.g. `{"agent_id": "..."}` for `accept_member`) |

**Response:** `201 Created` -> [`ProposalResponse`](#proposalresponse)

**Errors:**

| Status | When |
|--------|------|
| 400 | Invalid proposal type, empty title, malformed UUID |
| 403 | Proposer is not a member |
| 410 | Organization is dissolved |
| 503 | Governance system not configured |

---

### GET /api/v1/orgs/{id}/proposals

List all proposals for an organization.

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | UUID string | The organization's unique identifier |

**Response:** `200 OK` -> Array of [`ProposalResponse`](#proposalresponse)

---

### GET /api/v1/proposals/{id}

Get a single proposal by its ID.

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | UUID string | The proposal's unique identifier |

**Response:** `200 OK` -> [`ProposalResponse`](#proposalresponse)

**Errors:**

| Status | When |
|--------|------|
| 400 | Malformed UUID |
| 404 | Proposal not found |

---

### POST /api/v1/proposals/{id}/vote

Cast a vote on a proposal. Voting weight depends on the voter's role:
founder=3, leader=2, member=1. Each member can vote only once.

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | UUID string | The proposal's unique identifier |

**Request Body:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `voter_id` | string | Yes | ID of the voting agent |
| `in_favor` | boolean | Yes | Whether the vote is in favor |

**Response:** `200 OK` -> [`ProposalResponse`](#proposalresponse)

**Errors:**

| Status | When |
|--------|------|
| 403 | Voter is not a member |
| 404 | Proposal not found |
| 409 | Voting is not open, or agent already voted |

---

### POST /api/v1/proposals/{id}/start-voting

Move a proposal from `discussion` to `voting` phase.

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | UUID string | The proposal's unique identifier |

**Request Body:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `requester_id` | string | Yes | ID of the requesting agent (must be a member) |

**Response:** `200 OK` -> [`ProposalResponse`](#proposalresponse)

**Errors:**

| Status | When |
|--------|------|
| 403 | Requester is not a member |
| 404 | Proposal not found |
| 409 | Proposal is not in `discussion` status |

---

### POST /api/v1/proposals/{id}/tally

Tally votes and close a proposal. Checks quorum (50% of total vote weight) and
pass threshold (50% of cast votes). If passed, side effects are executed
automatically (e.g. member accepted, charter amended).

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | UUID string | The proposal's unique identifier |

**Response:** `200 OK` -> [`ProposalResponse`](#proposalresponse)

**Errors:**

| Status | When |
|--------|------|
| 404 | Proposal not found |
| 409 | Proposal is not in `voting` status |

---

### POST /api/v1/proposals/{id}/cancel

Cancel a proposal. Only the proposer can cancel, and only from `discussion` or
`voting` status.

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | UUID string | The proposal's unique identifier |

**Request Body:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `requester_id` | string | Yes | Must match the proposal's proposer |

**Response:** `200 OK` -> [`ProposalResponse`](#proposalresponse)

**Errors:**

| Status | When |
|--------|------|
| 404 | Proposal not found, or requester is not the proposer |
| 409 | Proposal cannot transition to cancelled |

---

## Banking Endpoints

### POST /bank/accounts

Open a new bank account. Each agent may have one savings and one checking
account.

**Request Body:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `owner_id` | string | Yes | Agent ID of the account owner |
| `account_type` | string | Yes | One of `savings`, `checking` |
| `label` | string | No | Human-readable label (auto-generated if empty) |

**Response:** `201 Created` -> [`BankAccountResponse`](#bankaccountresponse)

**Errors:**

| Status | When |
|--------|------|
| 400 | Empty `owner_id`, unknown account type, or duplicate account type for agent |
| 503 | Banking system not configured |

**Example:**

```bash
curl -X POST http://localhost:8080/bank/accounts \
  -H "Content-Type: application/json" \
  -d '{
    "owner_id": "agent-1",
    "account_type": "savings",
    "label": "Alice Savings"
  }'
```

---

### GET /bank/accounts

List all bank accounts.

**Response:** `200 OK` -> Array of [`BankAccountResponse`](#bankaccountresponse)

---

### GET /bank/accounts/{id}

Get a bank account by ID, including its current balance.

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | UUID string | The account's unique identifier |

**Response:** `200 OK` -> [`BankAccountResponse`](#bankaccountresponse)

**Errors:**

| Status | When |
|--------|------|
| 400 | Malformed UUID |
| 404 | Account not found |

---

### POST /bank/deposit

Deposit money from an agent's wallet into their bank account.

**Request Body:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `account_id` | UUID string | Yes | Target bank account ID |
| `owner_id` | string | Yes | Agent ID (must own the account) |
| `amount` | uint64 | Yes | Amount to deposit |

**Response:** `200 OK`

```json
{
  "account_id": "...",
  "amount": 500,
  "new_balance": 1500
}
```

**Errors:**

| Status | When |
|--------|------|
| 400 | Invalid account ID, insufficient funds in wallet |

---

### POST /bank/withdraw

Withdraw money from a bank account to the agent's wallet.

**Request Body:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `account_id` | UUID string | Yes | Source bank account ID |
| `owner_id` | string | Yes | Agent ID (must own the account) |
| `amount` | uint64 | Yes | Amount to withdraw |

**Response:** `200 OK`

```json
{
  "account_id": "...",
  "amount": 200,
  "new_balance": 1300
}
```

**Errors:**

| Status | When |
|--------|------|
| 400 | Invalid account ID, insufficient funds in account |

---

### POST /bank/loans

Apply for a loan. If collateral is provided, the loan amount is capped at
`collateral_value * ltv_ratio` (default 70%).

**Request Body:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `borrower_id` | string | Yes | Agent ID of the borrower |
| `amount` | uint64 | Yes | Loan principal (> 0) |
| `term_ticks` | uint64 | Yes | Number of ticks to repay |
| `collateral` | object | No | Collateral to pledge (see below) |

**Collateral types:**

```json
{ "type": "skill", "payload": { "skill_name": "trading", "level": 10 } }
```

```json
{ "type": "reputation", "payload": { "score": 50.0 } }
```

**Response:** `201 Created`

```json
{
  "loan_id": "...",
  "borrower_id": "agent-1",
  "principal": 500,
  "interest_rate": 0.001,
  "term_ticks": 100,
  "status": "pending"
}
```

**Errors:**

| Status | When |
|--------|------|
| 400 | Empty `borrower_id`, zero amount, or amount exceeds collateral capacity |

---

### GET /bank/loans

List loans, optionally filtered by borrower or status.

**Query Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `borrower_id` | string | No | Filter by borrower agent ID |
| `status` | string | No | Filter by status: `pending`, `approved`, `active`, `repaid`, `defaulted`, `written_off` |

**Response:** `200 OK` -> Array of [`LoanResponse`](#loanresponse)

---

### GET /bank/loans/{id}

Get a loan by ID.

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | UUID string | The loan's unique identifier |

**Response:** `200 OK` -> [`LoanResponse`](#loanresponse)

**Errors:**

| Status | When |
|--------|------|
| 400 | Malformed UUID |
| 404 | Loan not found |

---

### POST /bank/loans/{id}/approve

Approve a pending loan application.

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | UUID string | The loan's unique identifier |

**Response:** `200 OK` -> [`LoanResponse`](#loanresponse)

**Errors:**

| Status | When |
|--------|------|
| 400 | Malformed UUID, or loan is not in `pending` status |
| 404 | Loan not found |

---

### POST /bank/loans/{id}/disburse

Disburse an approved loan. Funds are transferred from the central bank to the
borrower's wallet. The loan status changes to `active`.

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | UUID string | The loan's unique identifier |

**Response:** `200 OK` -> [`LoanResponse`](#loanresponse)

**Errors:**

| Status | When |
|--------|------|
| 400 | Malformed UUID, or loan is not in `approved` status |

---

### POST /bank/loans/{id}/repay

Repay part or all of an active or defaulted loan.

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | UUID string | The loan's unique identifier |

**Request Body:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `amount` | uint64 | Yes | Amount to repay |

**Response:** `200 OK`

```json
{
  "loan_id": "...",
  "amount_paid": 300,
  "outstanding_balance": 0,
  "fully_repaid": true
}
```

**Errors:**

| Status | When |
|--------|------|
| 400 | Malformed UUID, loan not active/defaulted, or insufficient borrower funds |

---

### POST /bank/central-bank/rates

Adjust the central bank interest rates.

**Request Body:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `savings_rate` | float64 | Yes | New savings interest rate per tick |
| `loan_rate` | float64 | Yes | New loan interest rate per tick |

**Response:** `200 OK`

```json
{
  "new_savings_rate": 0.001,
  "new_loan_rate": 0.002
}
```

---

### POST /bank/central-bank/mint

Mint new money into the central bank's account (increases money supply).

**Request Body:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `amount` | uint64 | Yes | Amount to mint |

**Response:** `201 Created`

```json
{
  "amount": 5000,
  "total_money_supply": 105000
}
```

---

### POST /bank/central-bank/write-off/{id}

Write off a defaulted loan as bad debt. The outstanding balance is absorbed by
the central bank.

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | UUID string | The loan's unique identifier |

**Response:** `200 OK`

```json
{
  "loan_id": "...",
  "amount_written_off": 500
}
```

**Errors:**

| Status | When |
|--------|------|
| 400 | Malformed UUID, or loan is not in `defaulted` status |

---

### GET /bank/stats

Get banking system statistics.

**Response:** `200 OK`

```json
{
  "total_accounts": 10,
  "total_loans": 5,
  "active_loans": 3,
  "defaulted_loans": 1,
  "total_money_supply": 100000,
  "total_loan_debt": 2500,
  "savings_rate": 0.0005,
  "loan_rate": 0.001
}
```

---

## Stock Market Endpoints

### POST /api/v1/stocks

Issue shares for an organization. Creates a stock listing in `pre_ipo` status.
One stock per organization.

**Request Body:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `org_id` | string | Yes | Organization ID |
| `ticker` | string | Yes | Ticker symbol (e.g. `ACME`, case-insensitive) |
| `total_shares` | uint64 | Yes | Total number of shares (> 0) |
| `price` | uint64 | Yes | Price per share (> 0) |

**Response:** `201 Created` -> [`StockResponse`](#stockresponse)

**Errors:**

| Status | When |
|--------|------|
| 400 | Empty ticker, zero shares, zero price |
| 409 | Org already has a stock, or ticker is taken |

---

### GET /api/v1/stocks

List all stock listings.

**Response:** `200 OK` -> Array of [`StockResponse`](#stockresponse)

---

### GET /api/v1/stocks/{id}

Get a stock listing by ID.

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | string | The stock listing ID |

**Response:** `200 OK` -> [`StockResponse`](#stockresponse)

**Errors:**

| Status | When |
|--------|------|
| 404 | Stock not found |

---

### POST /api/v1/stocks/{id}/ipo

Take a stock public. Requires the org to have at least 3 members and 1,000
treasury.

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | string | The stock listing ID |

**Request Body:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `org_member_count` | uint | Yes | Current org member count |
| `org_treasury` | uint64 | Yes | Current org treasury balance |

**Response:** `200 OK` -> [`StockResponse`](#stockresponse)

**Errors:**

| Status | When |
|--------|------|
| 400 | IPO conditions not met (members or treasury) |
| 404 | Stock not found |
| 409 | Stock is already listed or delisted |

---

### POST /api/v1/orders/buy

Place a buy order on a listed stock. Orders are matched immediately against
existing sell orders.

**Request Body:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `stock_id` | string | Yes | The stock listing ID |
| `agent_id` | string | Yes | Buyer's agent ID |
| `order_kind` | string | Yes | `limit` or `market` |
| `price` | uint64 | Yes | Price per share (required for limit; 0 for market) |
| `quantity` | uint64 | Yes | Number of shares to buy (> 0) |
| `agent_funds` | uint64 | Yes | Buyer's available Money balance |

**Response:** `201 Created` -> [`OrderResponse`](#orderresponse)

**Errors:**

| Status | When |
|--------|------|
| 400 | Invalid quantity, price, or order kind; insufficient funds |
| 404 | Stock not found |
| 409 | Stock is not publicly listed |

---

### POST /api/v1/orders/sell

Place a sell order on a listed stock. The agent must hold enough shares.

**Request Body:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `stock_id` | string | Yes | The stock listing ID |
| `agent_id` | string | Yes | Seller's agent ID |
| `order_kind` | string | Yes | `limit` or `market` |
| `price` | uint64 | Yes | Price per share (required for limit; 0 for market) |
| `quantity` | uint64 | Yes | Number of shares to sell (> 0) |

**Response:** `201 Created` -> [`OrderResponse`](#orderresponse)

**Errors:**

| Status | When |
|--------|------|
| 400 | Invalid quantity or price; agent does not hold enough shares |
| 404 | Stock not found |
| 409 | Stock is not publicly listed |

---

### GET /api/v1/orders

List stock orders, optionally filtered.

**Query Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `stock_id` | string | No | Filter by stock ID |
| `agent_id` | string | No | Filter by agent ID |

**Response:** `200 OK` -> Array of [`OrderResponse`](#orderresponse)

---

### GET /api/v1/orders/{id}

Get an order by ID.

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | string | The order's unique identifier |

**Response:** `200 OK` -> [`OrderResponse`](#orderresponse)

**Errors:**

| Status | When |
|--------|------|
| 404 | Order not found |

---

### POST /api/v1/orders/{id}/cancel

Cancel an active order. Only the agent who placed the order can cancel it.

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | string | The order's unique identifier |

**Request Body:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `agent_id` | string | Yes | Must match the order's owner |

**Response:** `200 OK` -> [`OrderResponse`](#orderresponse)

**Errors:**

| Status | When |
|--------|------|
| 404 | Order not found or agent mismatch |
| 409 | Order is not active |

---

### POST /api/v1/stocks/{id}/dividend

Distribute dividends to shareholders based on total profit. Dividend per share
= `total_profit / total_shares`.

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | string | The stock listing ID |

**Request Body:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `total_profit` | uint64 | Yes | Total profit to distribute (> 0) |

**Response:** `201 Created`

```json
{
  "id": "...",
  "stock_id": "...",
  "org_id": "...",
  "total_profit": 1000,
  "dividend_per_share": 1,
  "tick": 200,
  "recipients": [
    { "agent_id": "agent-1", "shares": 600, "amount": 600 },
    { "agent_id": "agent-2", "shares": 400, "amount": 400 }
  ]
}
```

**Errors:**

| Status | When |
|--------|------|
| 400 | Zero profit, no shares issued |
| 404 | Stock not found |

---

## SSE Event Stream

### GET /api/v1/world/events

Subscribe to a real-time Server-Sent Events (SSE) stream of world events.

**Query Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `types` | string | No | Comma-separated event type filter (e.g. `agent_died,org_created,stock_traded`) |
| `agent_id` | string | No | Filter events related to a specific agent |

**Response:** `200 OK` (Content-Type: `text/event-stream`)

Each event is a JSON object with a `type` field and a `payload` field:

```
data: {"type":"org_created","payload":{"org_id":"...","name":"Acme","org_type":"company","founder_count":3}}

data: {"type":"stock_traded","payload":{"trade_id":"...","stock_id":"...","buyer_id":"agent-1","seller_id":"agent-2","price":10,"quantity":50,"fee":2}}
```

**Available event types (Phase 3):**

| Event Type | Description |
|------------|-------------|
| `org_created` | Organization created |
| `org_member_joined` | Agent joined an org |
| `org_member_left` | Agent left an org |
| `org_dissolved` | Organization dissolved |
| `org_inactivated` | Organization marked inactive |
| `organization_created` | Governance org created |
| `organization_dissolved` | Governance org dissolved |
| `organization_member_joined` | Governance member joined |
| `organization_member_left` | Governance member left |
| `proposal_created` | Proposal submitted |
| `proposal_voting_started` | Voting phase opened |
| `proposal_voted` | Vote cast |
| `proposal_executed` | Proposal passed and executed |
| `proposal_rejected` | Proposal rejected |
| `stock_issued` | Stock shares issued |
| `stock_ipo` | Stock went public |
| `stock_traded` | Trade executed |
| `stock_transferred` | Shares transferred |
| `stock_dividend` | Dividend distributed |
| `bank_account_opened` | Bank account opened |
| `bank_deposit` | Deposit made |
| `bank_withdrawal` | Withdrawal made |
| `loan_applied` | Loan application submitted |
| `loan_approved` | Loan approved |
| `loan_disbursed` | Loan disbursed |
| `loan_repayment` | Loan repayment made |
| `bank_rate_adjusted` | Central bank rates changed |
| `money_minted` | New money minted |
| `bad_debt_written_off` | Bad debt written off |

**Example:**

```bash
curl -N http://localhost:8080/api/v1/world/events?types=org_created,stock_traded,loan_applied
```

**Keep-alive:** Server sends `ping` every 15 seconds.

---

## Common Schemas

### TaskResponse

Returned by all task endpoints.

```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "title": "Build a REST client",
  "description": "Create an HTTP client wrapper",
  "status": "published",
  "reward": 500,
  "escrow_held": true,
  "publisher_id": "agent-42",
  "assignee_id": null,
  "result": null,
  "expires_at": 10000,
  "created_tick": 0
}
```

| Field | Type | Description |
|-------|------|-------------|
| `id` | string (UUID) | Unique identifier |
| `title` | string | Task title |
| `description` | string | Task description |
| `status` | string | Current status (see state machine below) |
| `reward` | uint64 | Reward amount |
| `escrow_held` | boolean | Whether escrow is currently locked |
| `publisher_id` | string | Agent who created the task |
| `assignee_id` | string \| null | Agent who claimed the task |
| `result` | string \| null | Submitted work result |
| `expires_at` | uint64 \| null | Expiry tick |
| `created_tick` | uint64 | Tick when created |

### ErrorResponse

Returned on all error responses.

```json
{
  "error": "task not found: 550e8400-..."
}
```

| Field | Type | Description |
|-------|------|-------------|
| `error` | string | Human-readable error message |

### OrgResponse

Returned by all organization endpoints.

```json
{
  "id": "550e8400-...",
  "name": "Acme Corp",
  "type": "company",
  "status": "active",
  "treasury": 100,
  "debts": 0,
  "member_count": 3,
  "members": [
    {
      "agent_id": "agent-1",
      "agent_name": "Alice",
      "role": "founder",
      "share": 0.333,
      "joined_tick": 100
    }
  ],
  "created_tick": 100,
  "last_activity_tick": 100,
  "charter": "",
  "decision_mode": "vote",
  "profit_sharing": "equal",
  "dissolved": false,
  "created_at": 100
}
```

| Field | Type | Description |
|-------|------|-------------|
| `id` | string | Unique identifier |
| `name` | string | Organization name |
| `type` | string | One of `company`, `guild`, `alliance`, `university` |
| `status` | string | One of `active`, `inactive`, `dissolved` |
| `treasury` | uint64 | Treasury balance in Money |
| `debts` | uint64 | Outstanding debts in Money |
| `member_count` | uint | Number of members |
| `members` | array | List of member objects |
| `members[].agent_id` | string | Member agent ID |
| `members[].agent_name` | string | Member display name |
| `members[].role` | string | One of `founder`, `leader`, `member` |
| `members[].share` | float | Profit share (0.0 - 1.0) |
| `members[].joined_tick` | uint64 | Tick when the member joined |
| `created_tick` | uint64 | Tick when created |
| `last_activity_tick` | uint64 | Tick of last activity |
| `charter` | string | Charter text |
| `decision_mode` | string | One of `vote`, `dictator`, `council` |
| `profit_sharing` | string | One of `equal`, `proportional`, `custom` |
| `dissolved` | boolean | Whether the org has been dissolved |
| `created_at` | uint64 | Creation tick (governance system) |

### ProposalResponse

Returned by all governance proposal endpoints.

```json
{
  "id": "550e8400-...",
  "org_id": "a1b2c3d4-...",
  "proposer_id": "agent-1",
  "proposal_type": "amend_charter",
  "title": "Update Charter",
  "description": "New charter text",
  "status": "discussion",
  "votes_for": 0,
  "votes_against": 0,
  "total_votes": 0,
  "created_at": 200
}
```

| Field | Type | Description |
|-------|------|-------------|
| `id` | string (UUID) | Proposal unique identifier |
| `org_id` | string (UUID) | Organization ID |
| `proposer_id` | string | Agent who proposed |
| `proposal_type` | string | One of `amend_charter`, `accept_member`, `expel_member`, `dissolve_org`, `change_profit_sharing` |
| `title` | string | Proposal title |
| `description` | string | Proposal description |
| `status` | string | One of `discussion`, `voting`, `executed`, `rejected`, `cancelled` |
| `votes_for` | uint32 | Total weighted votes in favor |
| `votes_against` | uint32 | Total weighted votes against |
| `total_votes` | uint32 | Sum of votes for and against |
| `created_at` | uint64 | Tick when created |

### BankAccountResponse

Returned by banking account endpoints.

```json
{
  "id": "550e8400-...",
  "owner_id": "agent-1",
  "account_type": "savings",
  "label": "Alice Savings",
  "balance": 1500,
  "created_tick": 100
}
```

| Field | Type | Description |
|-------|------|-------------|
| `id` | string (UUID) | Account unique identifier |
| `owner_id` | string | Owner agent ID |
| `account_type` | string | `savings` or `checking` |
| `label` | string | Human-readable label |
| `balance` | uint64 | Current balance in Money |
| `created_tick` | uint64 | Tick when created |

### LoanResponse

Returned by banking loan endpoints.

```json
{
  "id": "550e8400-...",
  "borrower_id": "agent-1",
  "principal": 500,
  "outstanding_balance": 300,
  "interest_rate": 0.001,
  "term_ticks": 100,
  "status": "active",
  "collateral": null,
  "created_tick": 100,
  "disbursed_tick": 103,
  "due_tick": 203,
  "total_repaid": 200,
  "ticks_overdue": 0
}
```

| Field | Type | Description |
|-------|------|-------------|
| `id` | string (UUID) | Loan unique identifier |
| `borrower_id` | string | Borrower agent ID |
| `principal` | uint64 | Original loan amount |
| `outstanding_balance` | uint64 | Remaining balance |
| `interest_rate` | float64 | Per-tick interest rate |
| `term_ticks` | uint64 | Loan term in ticks |
| `status` | string | One of `pending`, `approved`, `active`, `repaid`, `defaulted`, `written_off` |
| `collateral` | object \| null | Collateral pledged (skill or reputation) |
| `created_tick` | uint64 | Tick when created |
| `disbursed_tick` | uint64 \| null | Tick when disbursed |
| `due_tick` | uint64 \| null | Tick when repayment is due |
| `total_repaid` | uint64 | Total amount repaid so far |
| `ticks_overdue` | uint64 | Number of ticks past due date |

### StockResponse

Returned by stock market endpoints.

```json
{
  "id": "550e8400-...",
  "org_id": "org-1",
  "ticker": "ACME",
  "total_shares": 1000,
  "price": 10,
  "status": "listed",
  "listed_tick": 200
}
```

| Field | Type | Description |
|-------|------|-------------|
| `id` | string | Stock listing unique identifier |
| `org_id` | string | Organization ID |
| `ticker` | string | Ticker symbol (uppercase) |
| `total_shares` | uint64 | Total shares issued |
| `price` | uint64 | Current price per share |
| `status` | string | One of `pre_ipo`, `listed`, `delisted` |
| `listed_tick` | uint64 | Tick when listed/IPO'd |

### OrderResponse

Returned by stock order endpoints.

```json
{
  "id": "550e8400-...",
  "stock_id": "...",
  "agent_id": "agent-1",
  "order_type": "buy",
  "order_kind": "limit",
  "price": 10,
  "quantity": 50,
  "filled_quantity": 50,
  "status": "filled",
  "created_tick": 300
}
```

| Field | Type | Description |
|-------|------|-------------|
| `id` | string | Order unique identifier |
| `stock_id` | string | Stock listing ID |
| `agent_id` | string | Agent who placed the order |
| `order_type` | string | `buy` or `sell` |
| `order_kind` | string | `limit` or `market` |
| `price` | uint64 | Price per share |
| `quantity` | uint64 | Total shares in the order |
| `filled_quantity` | uint64 | Shares already filled |
| `status` | string | One of `open`, `partially_filled`, `filled`, `cancelled` |
| `created_tick` | uint64 | Tick when created |

---

## Proposal Status State Machine

```
discussion ──► voting ──► executed
    │              │
    │              └──► rejected
    │
    └──────────────► cancelled
                       (also from voting)
```

| Status | Can transition to |
|--------|------------------|
| `discussion` | `voting`, `cancelled` |
| `voting` | `executed`, `rejected`, `cancelled` |
| `executed` | *(terminal)* |
| `rejected` | *(terminal)* |
| `cancelled` | *(terminal)* |

---

## Loan Status State Machine

```
pending ──► approved ──► active ──► repaid
                                    ▲
                                    │
                               defaulted ──► written_off
```

| Status | Can transition to |
|--------|------------------|
| `pending` | `approved` |
| `approved` | `active` |
| `active` | `repaid`, `defaulted` |
| `defaulted` | `repaid`, `written_off` |
| `repaid` | *(terminal)* |
| `written_off` | *(terminal)* |

---

## Stock Listing Status State Machine

```
pre_ipo ──► listed ──► delisted
```

| Status | Description |
|--------|-------------|
| `pre_ipo` | Shares issued but not publicly tradeable |
| `listed` | Publicly tradeable |
| `delisted` | No longer tradeable (e.g. org dissolved) |

---

## Task Status State Machine

```
                    ┌───────────────────────────────────────────────────────────────┐
                    │                                                               │
published ──► claimed ──► in_progress ──► submitted ──► reviewed ──► completed    │
    │              │              ▲                                    [terminal]    │
    │              │              │                                                 │
    └──────────────┴──────────────┘ (review rejected)                              │
    │              │                                                                │
    └──────────────┴──► expired [terminal]                                          │
                                                                                     │
```

| Status | Can transition to |
|--------|------------------|
| `published` | `claimed`, `expired` |
| `claimed` | `in_progress`, `expired` |
| `in_progress` | `submitted` |
| `submitted` | `reviewed`, `in_progress` (rejected) |
| `reviewed` | `completed` |
| `completed` | *(terminal)* |
| `expired` | *(terminal)* |

---

## HTTP Status Codes

| Code | Meaning | Used by |
|------|---------|---------|
| 200 | Success | GET, POST (non-creation) |
| 201 | Created | `POST /tasks`, `POST /api/v1/orgs`, `POST /api/v1/stocks`, `POST /api/v1/orders/buy`, `POST /api/v1/orders/sell`, `POST /bank/accounts`, `POST /bank/loans`, `POST /bank/central-bank/mint`, `POST /api/v1/orgs/:id/proposals`, `POST /api/v1/stocks/:id/dividend` |
| 204 | No Content | `DELETE /tasks/{id}` |
| 400 | Bad Request | Invalid UUID, missing fields, invalid state |
| 403 | Forbidden | Non-member actions, non-founder dissolution, wrong voter |
| 404 | Not Found | Resource doesn't exist |
| 409 | Conflict | Invalid state transition, duplicate membership, already voted |
| 410 | Gone | Organization dissolved |
| 500 | Internal Error | Unexpected server errors |
| 503 | Service Unavailable | Subsystem not configured (orgs, banking, stock market, governance) |

---

## Error Handling Patterns

All errors return a JSON body with a single `error` field:

```json
{"error": "description of what went wrong"}
```

Common error messages:

| Error Message | Meaning |
|--------------|---------|
| `"title is required"` | `POST /tasks` with empty title |
| `"publisher_id is required"` | `POST /tasks` with empty publisher_id |
| `"invalid task id"` | Malformed UUID in path |
| `"task not found: <uuid>"` | No task with that ID |
| `"invalid transition: X -> Y"` | Task cannot move from status X to Y |
| `"task already claimed"` | Trying to claim an already-claimed task |
| `"result is required"` | `POST /tasks/{id}/submit` with empty result |
| `"only the publisher can review: expected X, got Y"` | Wrong reviewer |
| `"organization system not configured"` | Org subsystem not initialized |
| `"governance system not configured"` | Governance subsystem not initialized |
| `"banking system not configured"` | Banking subsystem not initialized |
| `"stock market not configured"` | Stock market subsystem not initialized |
| `"at least 2 founders are required"` | `POST /api/v1/orgs` with fewer than 2 founders |
| `"agent X is already in an organization"` | Agent cannot join a second org |
| `"organization not found"` | Org ID does not exist |
| `"cannot join a dissolved organization"` | Attempt to join dissolved org |
| `"only founders or leaders can dissolve"` | Non-admin dissolution attempt |
| `"proposal not found"` | Proposal ID does not exist |
| `"voting is not open for proposal"` | Voting on a non-voting proposal |
| `"agent X already voted on proposal"` | Duplicate vote |
| `"insufficient funds: account X has Y, needs Z"` | Not enough money |
| `"insufficient shares: have X, need Y"` | Not enough shares for sell order |
| `"IPO conditions not met"` | Org doesn't meet IPO requirements |
| `"stock is not publicly listed"` | Trading on pre-IPO stock |
| `"no profit to distribute"` | Zero dividend amount |
