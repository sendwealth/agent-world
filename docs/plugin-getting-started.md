# Plugin Development Guide

> **Audience**: External developers who want to extend Agent World with custom plugins.
> **Related**: [Plugin Interface Spec](./plugin-interface-spec.md) · [Public Plugin API](./public-plugin-api.md)

This guide walks you through writing, testing, and deploying a plugin for Agent World's plugin system.

---

## What Are Plugins?

Agent World plugins are **WASM modules** that implement the `SkillPlugin` interface. They let you:

- **Add custom skills** - new agent capabilities (e.g., "send emote", "analyze economy")
- **React to world events** - respond to ticks, agent spawns, transactions
- **Modify agent state** - request token transfers, skill changes, reputation adjustments (validated by the engine)
- **Emit events** - broadcast custom events that other plugins and agents can observe

Plugins run inside a **WASM sandbox** with no filesystem, network, or environment access. All state changes go through a request/validation pipeline - plugins never directly mutate world state.

---

## Two Plugin Interfaces

The plugin system has two layers:

| Layer | Language | Purpose | Entry Point |
|-------|----------|---------|-------------|
| **WASM Skill Plugin** | Python, Rust, or any WASM-compatible language | External developer plugins | `SkillPlugin` trait |
| **Engine Hook Plugin** | Rust only | Built-in engine extensions | Hook traits (OnTickStart, OnAgentAction, etc.) |

**This guide covers WASM Skill Plugins** - the interface for third-party developers. Engine hooks are for internal use only.

---

## Quick Start (Python)

### 1. Install the SDK

```bash
pip install agent-world-plugin-sdk
```

### 2. Create Your Plugin

```python
from agent_world_plugin_sdk import (
    SkillPlugin, PluginInfo, ActionContext, ActionResult,
    TokenCost, PluginError
)


class MyPlugin(SkillPlugin):
    @classmethod
    def init(cls, config):
        return PluginInfo(
            id="my-name/hello-plugin",
            name="Hello Plugin",
            version="0.1.0",
            description="Greets agents with a configurable message",
            author="Your Name",
            min_engine_version="1.0.0",
            required_skills=[],
            tags=["example", "tutorial"],
        )

    @classmethod
    def register(cls):
        return ["hello"]

    @classmethod
    def execute(cls, ctx: ActionContext) -> ActionResult:
        agent_name = ctx.world.agent.name if ctx.world.agent else "stranger"
        greeting = ctx.config.get("greeting", "Hello")
        return ActionResult(
            success=True,
            message=f"{greeting}, {agent_name}!",
            mutations=[],
            events=[],
            data={"greeting": f"{greeting}_{agent_name}"},
            tokens_consumed=10,
        )

    @classmethod
    def cost_estimate(cls, ctx: ActionContext) -> TokenCost:
        return TokenCost(estimated=10, confidence=0.95)

    @classmethod
    def shutdown(cls):
        pass

    @classmethod
    def on_event(cls, event, ctx):
        return None
```

### 3. Add a Manifest (`skills.yaml`)

```yaml
apiVersion: v1
plugin:
  id: "my-name/hello-plugin"
  name: "Hello Plugin"
  version: "0.1.0"
  description: "Greets agents with a configurable message"
  author: "Your Name"
  min_engine_version: "1.0.0"
  tags: [example, tutorial]
  skills:
    - id: "hello"
      name: "Hello Skill"
      description: "Sends a greeting to an agent"
  required_skills: []
  config:
    type: object
    properties:
      greeting:
        type: string
        default: "Hello"
    required: []
  subscribe: []
  resources:
    max_memory_mb: 8
    max_execution_time_s: 10
```

### 4. Test Locally

```python
from agent_world_plugin_sdk import ActionContext, WorldContext

ctx = ActionContext(world=WorldContext(tick=1), params={}, config={"greeting": "Hi"})
result = MyPlugin.execute(ctx)
assert result.success
```

### 5. Deploy via REST API

```bash
curl -X POST http://localhost:8080/api/v1/plugins/register \
  -H "Authorization: Bearer YOUR_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"id":"my-name/hello-plugin","name":"Hello Plugin","version":"0.1.0",
       "description":"Greets agents","author":"Your Name","priority":100,
       "permissions":["read_world_state","read_agents"]}'

# Then load WASM binary and initialize via sandbox endpoints
```

---

## Quick Start (Rust)

```bash
cargo new --lib my-rust-plugin && cd my-rust-plugin
# Add serde deps, implement WASM exports (init/register/execute/cost_estimate/shutdown)
rustup target add wasm32-unknown-unknown
cargo build --target wasm32-unknown-unknown --release
```

See `plugins/data-analysis-plugin/` for a complete working Rust plugin.

---

## Core Concepts

### Plugin Lifecycle

```
Loaded -> Init -> Register -> Execute (loop) -> Shutdown
```

1. **Loaded** - WASM module compiled and instantiated.
2. **Init** - `init(config)` returns `PluginInfo`.
3. **Register** - `register()` returns skill IDs.
4. **Execute** - `execute(ctx)` called each tick; `cost_estimate(ctx)` for budget check.
5. **Shutdown** - `shutdown()` on engine stop.

### Hooks System

| Hook | Trigger | Can Block? | Required Permission |
|------|---------|-----------|---------------------|
| `OnTickStart` | Before each tick | No | - |
| `OnTickEnd` | After each tick | No | - |
| `OnAgentAction` | Agent action | Yes | `InterceptActions` |
| `OnTransaction` | Agent trade | Yes | `InterceptTransactions` |
| `OnAgentSpawn` | New agent | No | `ReadAgents` |
| `OnEvent` | World event | No | `ReadEvents` |
| `OnStartup` | Engine start | No | - |
| `OnShutdown` | Engine stop | No | - |

### Permission Model

| Permission | Category | Description |
|-----------|----------|-------------|
| `ReadAgents` | Read | Read agent state |
| `ReadWorldState` | Read | Read world tick and config |
| `ReadEvents` | Read | Subscribe to events |
| `WriteAgentTokens` | Write | Modify agent tokens |
| `WriteAgentPhase` | Write | Modify agent phase |
| `WriteAgentSkills` | Write | Modify agent skills |
| `EmitEvents` | Write | Emit custom events |
| `InterceptActions` | Action | Intercept agent actions |
| `InterceptTransactions` | Action | Intercept transactions |
| `TickSubsystem` | Action | Register as tick subsystem |
| `AdminAccess` | Admin | Access subsystem state |

### State Mutations

Plugins return `StateMutation` requests (validated by engine):

| MutationKind | Effect |
|-------------|--------|
| `CREDIT_TOKENS` | Add tokens |
| `DEBIT_TOKENS` | Subtract tokens |
| `CREDIT_MONEY` | Add money |
| `DEBIT_MONEY` | Subtract money |
| `SET_SKILL` | Set skill level |
| `ADJUST_REPUTATION` | Modify reputation |
| `SET_GLOBAL` | Set world KV pair |
| `EMIT_EVENT` | Emit event |

### Resource Limits

| Resource | Default | Configurable |
|----------|---------|-------------|
| Linear memory | 64 MB | Yes |
| Execution time | 30s | Yes |
| Init timeout | 5s | Yes |
| Payload size | 1 MB | Yes |
| Mutations/execution | 10 | Yes |
| Events/execution | 5 | Yes |

### Error Handling

- Plugins **never crash the engine**.
- Failures are **billed** (agent pays estimated cost).
- Timeouts enforced -> `ExecutionFailed`.

---

## Complete Examples

- **Python**: `plugins/custom-action-plugin/` - Custom emote with event subscription, state mutations
- **Rust**: `plugins/data-analysis-plugin/` - Economy analysis (GDP, Gini), 20+ tests

---

## REST API Reference

**Base URL**: `http://localhost:8080/api/v1/plugins`

### Plugin Management

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/plugins` | List all plugins |
| GET | `/plugins/stats` | Plugin statistics |
| POST | `/plugins/register` | Register plugin |
| GET | `/plugins/:id` | Get plugin details |
| POST | `/plugins/:id/enable` | Enable plugin |
| POST | `/plugins/:id/disable` | Disable plugin |
| POST | `/plugins/:id/unload` | Unload plugin |

### WASM Sandbox

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/plugins/sandbox` | List sandbox plugins |
| POST | `/plugins/sandbox/load` | Load WASM binary |
| POST | `/plugins/sandbox/:id/init` | Initialize plugin |
| POST | `/plugins/sandbox/:id/execute` | Execute plugin |
| POST | `/plugins/sandbox/:id/shutdown` | Shutdown plugin |

---

## SDK Reference (Python)

```bash
pip install agent-world-plugin-sdk
```

Types: `SkillPlugin`, `PluginInfo`, `WorldContext`, `AgentSnapshot`, `ActionContext`, `ActionResult`, `StateMutation`, `MutationKind`, `TokenCost`, `PluginError`, `PluginClient`

---

## Security Model

1. **Sandbox isolation** - No filesystem/network/env access.
2. **Capability-based** - Permission-gated access.
3. **Resource limits** - Memory, time, payload caps.
4. **Mutation validation** - Engine validates all changes.
5. **Budget enforcement** - Cost check before execution.

---

## Implementation Status

| Component | Status |
|-----------|--------|
| SkillPlugin trait | Complete |
| Python SDK | Complete |
| Permission system | Complete |
| Hook dispatch | Complete |
| REST API (12 endpoints) | Complete (management), In Progress (sandbox) |
| WASM sandbox runtime | In Progress |
| Example plugins | Complete |

---

## Further Reading

- [Plugin Interface Specification](./plugin-interface-spec.md) - Types, ABI, versioning
- [Public Plugin API](./public-plugin-api.md) - REST API reference
- [Architecture](./ARCHITECTURE.md) - System design
- `plugins/` - Example plugins
- `sdk/plugin-sdk-python/` - Python SDK source
