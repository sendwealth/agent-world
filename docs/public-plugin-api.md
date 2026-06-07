# Public Plugin API Documentation

> **Version**: 1.0.0
> **Status**: Active
> **Base URL**: `/api/v1/plugins`

> **Getting Started**: See [Plugin Development Guide](./plugin-getting-started.md) for a step-by-step tutorial.
This document describes the public REST API for third-party plugin registration, management, and execution.

---

## Overview

The Plugin API enables external developers to:

1. **Register** plugins with metadata and permissions
2. **Load** WASM binaries into the sandbox
3. **Manage** plugin lifecycle (initialize, activate, disable, shutdown)
4. **Execute** plugins in an isolated WASM sandbox
5. **Query** plugin status and statistics

---

## Authentication

All endpoints require a valid API key via the `Authorization: Bearer <key>` header.

---

## Endpoints

### List Plugins

```
GET /api/v1/plugins
```

Returns all registered plugins.

**Response**:
```json
{
  "plugins": [
    {
      "id": "com.example.my-plugin",
      "name": "My Plugin",
      "version": "1.0.0",
      "description": "...",
      "author": "developer",
      "priority": 100,
      "state": "active",
      "permissions": ["read_world_state"],
      "hooks": ["on_tick_start"]
    }
  ],
  "total": 1,
  "active": 1
}
```

### Get Plugin

```
GET /api/v1/plugins/:id
```

Returns details for a specific plugin.

### Register Plugin

```
POST /api/v1/plugins/register
```

Register a new third-party plugin with metadata and requested permissions.

**Request**:
```json
{
  "id": "com.example.my-plugin",
  "name": "My Plugin",
  "version": "1.0.0",
  "description": "A custom plugin",
  "author": "developer@example.com",
  "priority": 100,
  "permissions": ["read_world_state", "read_agents", "emit_events"]
}
```

**Response**:
```json
{
  "id": "com.example.my-plugin",
  "name": "My Plugin",
  "version": "1.0.0",
  "status": "registered",
  "permissions": ["read_world_state", "read_agents", "emit_events"],
  "message": "Plugin registered. Upload WASM binary to /api/v1/plugins/sandbox/load to activate."
}
```

### Enable Plugin

```
POST /api/v1/plugins/:id/enable
```

### Disable Plugin

```
POST /api/v1/plugins/:id/disable
```

### Unload Plugin

```
POST /api/v1/plugins/:id/unload
```

### Plugin Statistics

```
GET /api/v1/plugins/stats
```

**Response**:
```json
{
  "total_plugins": 3,
  "active_plugins": 2
}
```

---

## WASM Sandbox Endpoints

### List Sandbox Plugins

```
GET /api/v1/plugins/sandbox
```

### Load WASM Binary

```
POST /api/v1/plugins/sandbox/load
```

**Request**:
```json
{
  "plugin_id": "com.example.my-plugin",
  "wasm_base64": "<base64-encoded WASM binary>"
}
```

### Initialize Plugin

```
POST /api/v1/plugins/sandbox/:id/init
```

### Execute Plugin

```
POST /api/v1/plugins/sandbox/:id/execute
```

**Request** (optional body):
```json
{
  "world": {"tick": 42},
  "agent": {"id": "agent-001", "name": "Alice"},
  "params": {}
}
```

### Shutdown Plugin

```
POST /api/v1/plugins/sandbox/:id/shutdown
```

---

## Permissions

| Permission | Description |
|-----------|-------------|
| `read_agents` | Read agent state (name, tokens, phase, skills) |
| `read_world_state` | Read world tick and config |
| `read_events` | Subscribe to events (read-only) |
| `write_agent_tokens` | Modify agent tokens |
| `write_agent_phase` | Modify agent phase |
| `write_agent_skills` | Modify agent skills |
| `emit_events` | Emit custom events |
| `intercept_actions` | Intercept and potentially block agent actions |
| `intercept_transactions` | Intercept and modify transactions |
| `tick_subsystem` | Register as a tick subsystem |
| `admin_access` | Access other subsystems' state |

---

## SDK Packages

### Python SDK

```bash
pip install agent-world-plugin-sdk
```

```python
from agent_world_plugin_sdk import SkillPlugin, PluginInfo, ActionContext, ActionResult

class MyPlugin(SkillPlugin):
    @classmethod
    def init(cls, config):
        return PluginInfo(id="my/plugin", name="My Plugin", ...)
    
    @classmethod
    def execute(cls, ctx: ActionContext) -> ActionResult:
        return ActionResult(success=True, message="Hello!")
```

### TypeScript SDK

```bash
npm install @agent-world/plugin-sdk
```

```typescript
import { SkillPlugin, PluginInfo, ActionContext, ActionResult } from '@agent-world/plugin-sdk';

class MyPlugin extends SkillPlugin {
    init(config: Record<string, string>): PluginInfo { ... }
    execute(ctx: ActionContext): ActionResult { ... }
}
```

---

## Example Plugins

See `plugins/` directory for complete examples:

1. **Custom Action Plugin** (`plugins/custom-action-plugin/`) — Python plugin for custom agent emote actions
2. **Data Analysis Plugin** (`plugins/data-analysis-plugin/`) — Rust plugin for economy analysis with Gini coefficient

---

## Security Model

1. **Sandbox Isolation**: WASM plugins run with no filesystem, network, or environment access
2. **Capability-Based**: Plugins can only access data granted by their permission set
3. **Resource Limits**: Memory (64 MB), execution time (30s), payload size (1 MB)
4. **Mutation Validation**: All state change requests are validated before application
5. **Budget Enforcement**: Token costs are checked before execution via `cost_estimate()`
