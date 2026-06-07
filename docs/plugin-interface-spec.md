# Plugin Interface Specification

> **Version**: 1.0.0
> **Status**: Active
> **Related**: [SEN-312](https://github.com/sendwealth/agent-world/issues/312) (WASM Runtime), [SEN-314](https://github.com/sendwealth/agent-world/issues/314) (Skill Marketplace), [SEN-316](https://github.com/sendwealth/agent-world/issues/316) (Example Plugins)

This document defines the standard plugin interface for Agent World's skill system. Third-party developers implement this interface to create plugins that run inside the WASM sandbox.

---

## 1. Overview

Plugins are WASM modules that implement the `SkillPlugin` trait. The World Engine loads them into a wasmtime sandbox and invokes their methods during simulation ticks. Plugins can:

- Inspect and modify agent state (read-only by default, explicit mutation via `ActionResponse`)
- React to world events
- Estimate their own token cost before execution
- Declare which built-in skills they depend on

### Lifecycle

```
                  ┌──────────┐
                  │  Loaded  │  WASM module compiled & instantiated
                  └────┬─────┘
                       │
                  ┌────▼─────┐
            ┌─────│   Init   │  Plugin provides metadata, validates config
            │     └────┬─────┘
            │          │
            │     ┌────▼──────┐
            │     │ Register  │  Engine registers capabilities & skill deps
            │     └────┬──────┘
            │          │
            │     ┌────▼──────┐
            │  ┌──│  Execute  │◄── Called each tick (or on event trigger)
            │  │  └────┬──────┘
            │  │       │
            │  │  ┌────▼────────────┐
            │  │  │ Cost Estimate   │◄── Pre-flight: should the engine run this?
            │  │  └────┬────────────┘
            │  │       │
            │  │       └────── (loop back to Execute)
            │  │
            │  │  ┌──────────────┐
            │  └──│   Shutdown   │  Graceful teardown, resource cleanup
            │     └──────────────┘
            │
            └── On error: Engine logs, emits PluginError event, continues
```

**Phase transitions**:

| Phase | Trigger | Description |
|-------|---------|-------------|
| Loaded → Init | Module instantiation | One-time setup |
| Init → Register | Init succeeds | Engine registers the plugin |
| Register → Execute | Tick fires / event matches | Normal operation loop |
| Execute → Cost Estimate | Before each execution | Budget check |
| * → Shutdown | Engine stopping or plugin unload | Cleanup |

---

## 2. Rust Trait Definition

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ─── Core Types ───────────────────────────────────────────────────────────

/// Unique identifier for a plugin, namespaced by author.
/// Format: `"<author>/<plugin-name>"` (e.g. `"agentworld/code-reviewer"`).
pub type PluginId = String;

/// Semantic version string (MAJOR.MINOR.PATCH).
pub type SemVer = String;

/// Identifier for a built-in or registered skill.
pub type SkillId = String;

/// Unique identifier for an agent in the simulation.
pub type AgentId = String;

// ─── Metadata ─────────────────────────────────────────────────────────────

/// Plugin metadata returned from `init()`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    /// Unique plugin identifier (e.g. `"agentworld/code-reviewer"`).
    pub id: PluginId,
    /// Human-readable name.
    pub name: String,
    /// Plugin version (semver).
    pub version: SemVer,
    /// One-line description.
    pub description: String,
    /// Author name or organization.
    pub author: String,
    /// Minimum compatible engine API version.
    pub min_engine_version: SemVer,
    /// List of skill IDs this plugin depends on.
    pub required_skills: Vec<SkillId>,
    /// Optional configuration schema (JSON Schema draft-07).
    pub config_schema: Option<String>,
    /// Tags for marketplace discovery.
    pub tags: Vec<String>,
}

// ─── Context Types ────────────────────────────────────────────────────────

/// Read-only snapshot of the world state, provided to the plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldContext {
    /// Current simulation tick.
    pub tick: u64,
    /// Agent executing the skill (if applicable).
    pub agent: Option<AgentSnapshot>,
    /// All visible agents (depends on permissions).
    pub visible_agents: Vec<AgentSnapshot>,
    /// World-level global key-value state.
    pub globals: HashMap<String, String>,
    /// Events emitted since last tick.
    pub recent_events: Vec<String>,
}

/// Snapshot of a single agent's public state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSnapshot {
    pub id: AgentId,
    pub name: String,
    pub phase: String,
    pub money: u64,
    pub tokens: u64,
    pub reputation: f64,
    pub skills: HashMap<String, u64>,
    pub alive: bool,
    pub age: u64,
}

/// The action context passed to `execute()`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionContext {
    /// World context (read-only snapshot).
    pub world: WorldContext,
    /// Skill-specific input parameters (from the agent's decision).
    pub params: HashMap<String, String>,
    /// Plugin-specific configuration.
    pub config: HashMap<String, String>,
}

// ─── Response Types ───────────────────────────────────────────────────────

/// Result of `execute()`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionResult {
    /// Whether the action succeeded.
    pub success: bool,
    /// Human-readable result message.
    pub message: String,
    /// State mutations the plugin requests the engine to apply.
    pub mutations: Vec<StateMutation>,
    /// Events the plugin wants to emit.
    pub events: Vec<String>,
    /// Additional data to return to the agent.
    pub data: HashMap<String, String>,
    /// Actual token cost consumed (may differ from estimate).
    pub tokens_consumed: u64,
}

/// A state mutation requested by the plugin.
/// The engine validates and applies these; plugins cannot directly mutate state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateMutation {
    /// The kind of mutation.
    pub kind: MutationKind,
    /// Target agent ID (if applicable).
    pub target_agent: Option<AgentId>,
    /// Field name to mutate.
    pub field: String,
    /// New value (string-encoded).
    pub value: String,
}

/// Kinds of state mutations a plugin can request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MutationKind {
    /// Add tokens to an agent's balance.
    CreditTokens,
    /// Subtract tokens from an agent's balance.
    DebitTokens,
    /// Add money to an agent.
    CreditMoney,
    /// Subtract money from an agent.
    DebitMoney,
    /// Update a skill level.
    SetSkill,
    /// Modify reputation.
    AdjustReputation,
    /// Set a world-level key-value pair.
    SetGlobal,
    /// Emit a custom event.
    EmitEvent,
}

/// Token cost estimate, returned from `cost_estimate()`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenCost {
    /// Estimated token consumption.
    pub estimated: u64,
    /// Confidence level (0.0–1.0).
    pub confidence: f64,
    /// Human-readable cost breakdown.
    pub breakdown: Option<String>,
}

// ─── Error Handling ───────────────────────────────────────────────────────

/// Errors a plugin can return.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PluginError {
    /// Initialization failed.
    InitFailed { reason: String },
    /// Execution failed.
    ExecutionFailed { reason: String },
    /// Missing required configuration.
    ConfigError { key: String, message: String },
    /// Missing required skill dependency.
    MissingSkill { skill_id: SkillId },
    /// Cost estimation failed.
    CostEstimateFailed { reason: String },
    /// Plugin state is invalid for the requested operation.
    InvalidState { expected: String, actual: String },
    /// Custom error.
    Custom { code: String, message: String },
}

// ─── The Trait ────────────────────────────────────────────────────────────

/// The core plugin interface. All WASM skill plugins must implement this trait.
///
/// # WASM Compatibility
///
/// All types use `serde` with JSON serialization for WASM ABI boundary.
/// Methods return `Result<T, PluginError>` which the runtime unwraps.
///
/// # Thread Safety
///
/// The runtime invokes plugin methods sequentially per instance.
/// No concurrent access guarantees are needed within a single plugin instance.
pub trait SkillPlugin {
    /// Return plugin metadata. Called once after WASM instantiation.
    ///
    /// Use this to validate configuration and prepare internal state.
    fn init(config: HashMap<String, String>) -> Result<PluginInfo, PluginError>;

    /// Return the list of skill IDs this plugin provides.
    /// Called after `init()` succeeds. The engine registers these
    /// skills in the world's skill tree.
    fn register() -> Vec<SkillId>;

    /// Execute the plugin's core logic.
    ///
    /// Receives an `ActionContext` with world state and parameters.
    /// Returns an `ActionResult` with success status and any requested
    /// state mutations.
    ///
    /// # Panics
    ///
    /// Panics are caught by the WASM runtime and converted to
    /// `PluginError::ExecutionFailed`.
    fn execute(ctx: ActionContext) -> Result<ActionResult, PluginError>;

    /// Estimate the token cost of execution *before* calling `execute()`.
    ///
    /// The engine uses this to decide whether to proceed based on the
    /// agent's remaining token budget. Return a cost of 0 for free actions.
    fn cost_estimate(ctx: &ActionContext) -> Result<TokenCost, PluginError>;

    /// Graceful shutdown. Called when the engine is stopping or
    /// unloading the plugin. Use this to release resources.
    ///
    /// Default implementation is a no-op.
    fn shutdown() {}

    /// Optional: Handle a world event. Called when an event matching
    /// the plugin's subscriptions fires.
    ///
    /// Default implementation does nothing.
    fn on_event(_event: &str, _ctx: &WorldContext) -> Option<ActionResult> {
        None
    }
}
```

---

## 3. Python Equivalent Interface

Python plugins compile to WASM via [ComponentizePy](https://github.com/bytecodealliance/componentize-py) or run in a Python-side shim. The interface is structurally identical:

```python
"""Agent World Plugin SDK — Python Interface."""
from __future__ import annotations

from dataclasses import dataclass, field
from typing import Dict, List, Optional
from enum import Enum


# ─── Core Types ────────────────────────────────────────────────────────

PluginId = str
SemVer = str
SkillId = str
AgentId = str


# ─── Data Classes ──────────────────────────────────────────────────────

@dataclass
class PluginInfo:
    """Metadata returned from init()."""
    id: PluginId
    name: str
    version: SemVer
    description: str
    author: str
    min_engine_version: SemVer
    required_skills: List[SkillId] = field(default_factory=list)
    config_schema: Optional[str] = None
    tags: List[str] = field(default_factory=list)


@dataclass
class AgentSnapshot:
    """Read-only snapshot of an agent."""
    id: AgentId
    name: str
    phase: str
    money: int
    tokens: int
    reputation: float
    skills: Dict[str, int] = field(default_factory=dict)
    alive: bool = True
    age: int = 0


@dataclass
class WorldContext:
    """Read-only world state snapshot."""
    tick: int
    agent: Optional[AgentSnapshot] = None
    visible_agents: List[AgentSnapshot] = field(default_factory=list)
    globals: Dict[str, str] = field(default_factory=dict)
    recent_events: List[str] = field(default_factory=list)


@dataclass
class ActionContext:
    """Full context for plugin execution."""
    world: WorldContext
    params: Dict[str, str] = field(default_factory=dict)
    config: Dict[str, str] = field(default_factory=dict)


class MutationKind(Enum):
    CREDIT_TOKENS = "credit_tokens"
    DEBIT_TOKENS = "debit_tokens"
    CREDIT_MONEY = "credit_money"
    DEBIT_MONEY = "debit_money"
    SET_SKILL = "set_skill"
    ADJUST_REPUTATION = "adjust_reputation"
    SET_GLOBAL = "set_global"
    EMIT_EVENT = "emit_event"


@dataclass
class StateMutation:
    """A state mutation requested by the plugin."""
    kind: MutationKind
    target_agent: Optional[AgentId] = None
    field: str = ""
    value: str = ""


@dataclass
class ActionResult:
    """Result of execute()."""
    success: bool
    message: str
    mutations: List[StateMutation] = field(default_factory=list)
    events: List[str] = field(default_factory=list)
    data: Dict[str, str] = field(default_factory=dict)
    tokens_consumed: int = 0


@dataclass
class TokenCost:
    """Token cost estimate."""
    estimated: int
    confidence: float = 1.0
    breakdown: Optional[str] = None


class PluginError(Exception):
    """Base plugin error."""
    def __init__(self, code: str = "custom", message: str = ""):
        self.code = code
        self.message = message
        super().__init__(f"[{code}] {message}")


# ─── Abstract Base ─────────────────────────────────────────────────────

from abc import ABC, abstractmethod


class SkillPlugin(ABC):
    """The core plugin interface. All skill plugins must subclass this."""

    @classmethod
    @abstractmethod
    def init(cls, config: Dict[str, str]) -> PluginInfo:
        """Return plugin metadata. Called once after loading."""
        ...

    @classmethod
    @abstractmethod
    def register(cls) -> List[SkillId]:
        """Return the skill IDs this plugin provides."""
        ...

    @classmethod
    @abstractmethod
    def execute(cls, ctx: ActionContext) -> ActionResult:
        """Execute the plugin's core logic."""
        ...

    @classmethod
    @abstractmethod
    def cost_estimate(cls, ctx: ActionContext) -> TokenCost:
        """Estimate token cost before execution."""
        ...

    @classmethod
    def shutdown(cls) -> None:
        """Graceful shutdown. Override if cleanup is needed."""
        pass

    @classmethod
    def on_event(cls, event: str, ctx: WorldContext) -> Optional[ActionResult]:
        """Handle a world event. Override to react to events."""
        return None
```

---

## 4. Error Handling Specification

### 4.1 Error Propagation

```
Plugin Error → WASM Runtime (trap) → Engine catches → Logs + PluginError event → Simulation continues
```

Key principles:

1. **Plugins never crash the engine.** All errors are caught at the WASM boundary.
2. **Errors are events.** The engine emits a `PluginError` event that other subsystems can observe.
3. **Failures are billed.** If a plugin's `execute()` fails, the agent still pays the estimated token cost (or a minimum cost).
4. **Retry is the engine's choice.** The engine may retry transient failures up to a configurable limit.

### 4.2 Error Categories

| Category | Code | Severity | Action |
|----------|------|----------|--------|
| Initialization | `init_failed` | Fatal for plugin | Skip plugin, log warning |
| Execution | `execution_failed` | Recoverable | Charge tokens, continue |
| Configuration | `config_error` | Fatal for plugin | Skip plugin, notify admin |
| Missing Dependency | `missing_skill` | Fatal for plugin | Skip plugin, log dependency |
| Cost Estimate | `cost_estimate_failed` | Recoverable | Use max budget, proceed |
| Invalid State | `invalid_state` | Recoverable | Charge tokens, continue |
| Custom | Any string | Plugin-defined | Logged as-is |

### 4.3 Timeout Handling

The WASM runtime enforces execution timeouts:

| Method | Default Timeout | Configurable |
|--------|----------------|--------------|
| `init()` | 5 seconds | Yes |
| `execute()` | 30 seconds | Yes |
| `cost_estimate()` | 5 seconds | Yes |
| `on_event()` | 10 seconds | Yes |
| `shutdown()` | 5 seconds | Yes |

Exceeding a timeout results in a `PluginError::ExecutionFailed { reason: "timeout" }`.

---

## 5. Version Management

### 5.1 Version Scheme

Plugins follow [Semantic Versioning 2.0](https://semver.org/):

```
MAJOR.MINOR.PATCH[-PRERELEASE]
```

- **MAJOR**: Breaking interface changes (new required methods, changed signatures)
- **MINOR**: New optional features (new default methods, new mutation kinds)
- **PATCH**: Bug fixes, no interface changes
- **PRERELEASE**: `-alpha.N`, `-beta.N`, `-rc.N` for pre-release versions

### 5.2 Compatibility Check

The engine checks compatibility at load time:

1. Plugin declares `min_engine_version` in `PluginInfo`.
2. Engine compares against its current API version.
3. If engine version < `min_engine_version`, the plugin is rejected with a descriptive error.
4. Plugin API version follows the engine version (not independent).

### 5.3 Evolution Strategy

| Change Type | Plugin Action | Engine Action |
|-------------|--------------|---------------|
| New optional method (with default) | No change needed | Call default if not implemented |
| New required method | Bump MAJOR | Require new version |
| New `MutationKind` variant | Ignore unknown | Reject unknown mutations gracefully |
| New field on existing struct | Ignore unknown | Deserialize with `#[non_exhaustive]` |
| Deprecated method | Mark `#[deprecated]` | Still call, log warning |

---

## 6. WASM ABI Contract

### 6.1 Serialization

All data crossing the WASM boundary uses **JSON** (via serde). The engine:

1. Serializes input parameters to JSON.
2. Writes JSON to shared memory.
3. Invokes the exported WASM function.
4. Reads JSON output from shared memory.
5. Deserializes into Rust types.

### 6.2 Exports

The WASM module must export these functions:

| Export Name | Signature | Description |
|-------------|-----------|-------------|
| `init` | `(ptr: *const u8, len: usize) -> usize` | Config JSON in, `PluginInfo` JSON out |
| `register` | `() -> usize` | Returns `Vec<SkillId>` JSON |
| `execute` | `(ptr: *const u8, len: usize) -> usize` | `ActionContext` JSON in, `ActionResult` JSON out |
| `cost_estimate` | `(ptr: *const u8, len: usize) -> usize` | `ActionContext` JSON in, `TokenCost` JSON out |
| `shutdown` | `() -> ()` | No I/O |
| `on_event` | `(ptr: *const u8, len: usize) -> usize` | Event JSON + WorldContext JSON in, optional `ActionResult` JSON out |

### 6.3 Memory

- **Maximum linear memory**: 64 MB (configurable)
- **Maximum table entries**: 1 (no indirect calls)
- **No WASI**: Plugins run in a pure sandbox without filesystem, network, or environment access.

### 6.4 Resource Limits

| Resource | Default Limit | Configurable |
|----------|--------------|--------------|
| Linear memory | 64 MB | Yes |
| Execution time (execute) | 30s | Yes |
| Execution time (init) | 5s | Yes |
| Return payload size | 1 MB | Yes |
| Mutations per execution | 10 | Yes |
| Events per execution | 5 | Yes |

---

## 7. Configuration

### 7.1 Plugin Manifest (`skills.yaml`)

Every plugin ships with a `skills.yaml` manifest:

```yaml
# skills.yaml — Plugin manifest
apiVersion: v1

plugin:
  id: "example/hello-world"
  name: "Hello World"
  version: "0.1.0"
  description: "A minimal example plugin"
  author: "Agent World Team"
  min_engine_version: "1.0.0"
  tags:
    - example
    - tutorial

  # Skills provided by this plugin
  skills:
    - id: "hello"
      name: "Hello Skill"
      description: "Greets the agent"

  # Skill dependencies (must exist in the world)
  required_skills: []

  # Configuration schema (JSON Schema draft-07)
  config:
    type: object
    properties:
      greeting:
        type: string
        default: "Hello"
        description: "The greeting word to use"
    required: []

  # Event subscriptions (optional)
  subscribe:
    - "agent_spawned"
    - "tick_advanced"

  # Resource requirements
  resources:
    max_memory_mb: 16
    max_execution_time_s: 10
```

### 7.2 Engine Configuration

The engine's config (in `world-rules.yaml` or equivalent):

```yaml
plugins:
  # Directory to load plugin WASM files from
  search_path: "./plugins"

  # Global settings
  max_memory_mb: 64
  default_execution_timeout_s: 30

  # Per-plugin overrides (by plugin ID)
  overrides:
    "example/heavy-compute":
      max_memory_mb: 128
      max_execution_timeout_s: 60
```

---

## 8. Security Model

1. **Sandbox isolation**: Plugins run in wasmtime with no access to host filesystem, network, or environment variables.
2. **Capability-based API**: Plugins can only access the data passed in `ActionContext`. No direct state mutation — all changes go through `StateMutation` requests validated by the engine.
3. **Budget enforcement**: The engine checks `cost_estimate()` against the agent's remaining token budget before executing.
4. **Timeout enforcement**: Execution is terminated if it exceeds the configured timeout.
5. **Mutation validation**: All `StateMutation` requests are validated by the engine's rule system before application.

---

## Appendix A: Full Type Reference

| Type | Fields | Description |
|------|--------|-------------|
| `PluginInfo` | id, name, version, description, author, min_engine_version, required_skills, config_schema, tags | Plugin metadata |
| `WorldContext` | tick, agent, visible_agents, globals, recent_events | Read-only world snapshot |
| `AgentSnapshot` | id, name, phase, money, tokens, reputation, skills, alive, age | Agent state snapshot |
| `ActionContext` | world, params, config | Full execution context |
| `ActionResult` | success, message, mutations, events, data, tokens_consumed | Execution result |
| `StateMutation` | kind, target_agent, field, value | State change request |
| `MutationKind` | CREDIT_TOKENS, DEBIT_TOKENS, CREDIT_MONEY, DEBIT_MONEY, SET_SKILL, ADJUST_REPUTATION, SET_GLOBAL, EMIT_EVENT | Mutation type enum |
| `TokenCost` | estimated, confidence, breakdown | Cost estimate |
| `PluginError` | InitFailed, ExecutionFailed, ConfigError, MissingSkill, CostEstimateFailed, InvalidState, Custom | Error enum |

## Appendix B: WASM Compilation Targets

| Language | Toolchain | Target | Status |
|----------|-----------|--------|--------|
| Rust | `cargo build --target wasm32-unknown-unknown` | `wasm32-unknown-unknown` | ✅ Primary |
| Python | ComponentizePy | WASM Component | 🔜 Planned |
| AssemblyScript | `asc` | `wasm32-unknown-unknown` | 🔜 Planned |
| Go | TinyGo | `wasm32-unknown-unknown` | 🔜 Planned |
