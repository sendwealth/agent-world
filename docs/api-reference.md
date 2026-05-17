# API Reference

Complete reference for the Agent World Engine REST API.

- **Base URL:** `http://localhost:3000`
- **Content-Type:** `application/json`
- **OpenAPI Spec:** [`openapi.yaml`](openapi.yaml)

---

## Overview

The API has two groups of endpoints:

| Group | Prefix | Endpoints | Description |
|-------|--------|-----------|-------------|
| Tasks | `/tasks` | 10 | Task marketplace CRUD + lifecycle |
| WAL | `/wal` | 3 | Write-Ahead Log operations |

---

## Authentication

The current version (v0.1.0) does not require authentication. All endpoints
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
curl -X POST http://localhost:3000/tasks \
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
curl http://localhost:3000/tasks
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
| 201 | Created | `POST /tasks` |
| 204 | No Content | `DELETE /tasks/{id}` |
| 400 | Bad Request | Invalid UUID, missing fields |
| 403 | Forbidden | Review by non-publisher |
| 404 | Not Found | Task doesn't exist |
| 409 | Conflict | Invalid state transition |
| 500 | Internal Error | Unexpected server errors |

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
