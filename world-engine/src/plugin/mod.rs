//! # Plugin System
//!
//! Third-party extension API for the World Engine.
//!
//! The plugin system allows external code to extend the engine through:
//! - **Hooks**: Pre/post interceptors for ticks, actions, trades, and events
//! - **Subsystems**: Full tick-participating components (same as built-in subsystems)
//! - **Event handlers**: Reactive handlers that respond to specific event types
//!
//! ## Architecture
//!
//! ```text
//! PluginManager
//! ├── PluginRegistry          — metadata + instance storage
//! ├── HookDispatcher          — ordered hook invocation
//! ├── PermissionGuard         — capability enforcement
//! └── PluginSubsystemBridge   — wraps plugins as Subsystem trait objects
//! ```
//!
//! ## Plugin Lifecycle
//!
//! 1. **Register** — plugin declares metadata, permissions, and hooks
//! 2. **Initialize** — `on_load()` called with PluginContext (config, event bus access)
//! 3. **Active** — hooks fire on registered points; subsystem runs each tick
//! 4. **Unload** — `on_unload()` called for cleanup
//!
//! ## Example
//!
//! ```rust,ignore
//! use agent_world_engine::plugin::*;
//!
//! struct TaxPlugin { rate: f64 }
//!
//! impl Plugin for TaxPlugin {
//!     fn metadata(&self) -> PluginMetadata { /* ... */ }
//!     fn on_load(&mut self, ctx: &PluginContext) -> Result<(), PluginError> { Ok(()) }
//!     fn on_unload(&mut self) {}
//! }
//!
//! impl OnTransaction for TaxPlugin {
//!     fn on_transaction(&self, txn: &TransactionContext) -> HookResult {
//!         // Apply tax logic
//!         HookResult::Continue
//!     }
//! }
//! ```

mod context;
mod error;
pub mod examples;
mod hooks;
mod manager;
mod metadata;
mod permission;
mod registry;
pub mod sandbox;

pub use context::{
    AgentActionContext, AgentSpawnContext, EventContext, PluginContext, ShutdownContext,
    StartupContext, TickContext, TransactionContext, WorldSnapshot,
};
pub use error::PluginError;
pub use hooks::{
    HookResult, OnAgentAction, OnAgentSpawn, OnEvent, OnShutdown, OnStartup, OnTickEnd,
    OnTickStart, OnTransaction,
};
pub use manager::{PluginManager, PluginSubsystemBridge, SharedPluginManager};
pub use metadata::{PluginMetadata, PluginState};
pub use permission::{Permission, PermissionSet};
pub use registry::{PluginHooks, PluginInfo, PluginRegistry};
pub use sandbox::{
    SandboxConfig, SharedWasmSandbox, WasmExecutionResult, WasmPluginInfo, WasmPluginPhase,
    WasmSandbox,
};
