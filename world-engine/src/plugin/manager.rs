//! Plugin manager — central coordinator for the plugin system.
//!
//! The `PluginManager` owns the registry, enforces permissions,
//! dispatches hooks, and bridges plugins into the tick loop.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::Mutex;
use uuid::Uuid;

use super::context::{
    AgentActionContext, AgentSpawnContext, EventContext, PluginContext, ShutdownContext,
    StartupContext, TickContext, TransactionContext, WorldSnapshot,
};
use super::error::PluginError;
use super::hooks::HookResult;
use super::metadata::{PluginMetadata, PluginState};
use super::permission::{Permission, PermissionSet};
use super::registry::{PluginEntry, PluginHooks, PluginInfo, PluginRegistry};
use crate::world::event::WorldEvent;
use crate::world::state::SharedEventBus;
use crate::world::subsystem::Subsystem;
use crate::economy::token_burn::AgentRecord;

/// Shared plugin manager behind `Arc<Mutex<>>`.
pub type SharedPluginManager = Arc<Mutex<PluginManager>>;

/// The central plugin manager.
///
/// Owns the plugin registry and provides methods for:
/// - Registering and unregistering plugins
/// - Dispatching hooks at each extension point
/// - Bridging plugins into the Subsystem tick pipeline
/// - Querying plugin state for the API
pub struct PluginManager {
    registry: PluginRegistry,
    event_bus: SharedEventBus,
    /// Per-plugin configuration loaded from genesis.yaml.
    plugin_configs: HashMap<String, HashMap<String, serde_yaml::Value>>,
}

impl PluginManager {
    /// Create a new plugin manager.
    pub fn new(event_bus: SharedEventBus) -> Self {
        Self {
            registry: PluginRegistry::new(),
            event_bus,
            plugin_configs: HashMap::new(),
        }
    }

    /// Load plugin configs from genesis config section.
    pub fn set_plugin_configs(&mut self, configs: HashMap<String, HashMap<String, serde_yaml::Value>>) {
        self.plugin_configs = configs;
    }

    // ── Registration ──────────────────────────────────────────

    /// Register a plugin with the given metadata, permissions, and hooks.
    pub fn register_plugin(
        &mut self,
        metadata: PluginMetadata,
        permissions: PermissionSet,
        hooks: PluginHooks,
    ) -> Result<(), PluginError> {
        let entry = PluginEntry {
            metadata,
            state: PluginState::Registered,
            permissions,
            hooks,
        };
        self.registry.register(entry)
    }

    /// Initialize all registered plugins.
    ///
    /// This calls `on_load` equivalent logic (the PluginContext is provided
    /// here) and transitions plugins from `Registered` to `Active`.
    pub fn initialize_plugins(&mut self) -> Result<Vec<String>, PluginError> {
        let errors = Vec::new();
        let ids: Vec<String> = self.registry.plugin_ids();
        let plugin_count = self.registry.len();

        for id in ids {
            let config = self.plugin_configs.get(&id).cloned().unwrap_or_default();
            let ctx = PluginContext {
                config,
                event_bus: self.event_bus.clone(),
            };

            if let Some(entry) = self.registry.get_mut(&id) {
                // Plugins that implement OnStartup get their startup hook called
                if entry.hooks.on_startup.is_some() {
                    let startup_ctx = StartupContext {
                        plugin_count,
                        initial_tick: 0,
                    };
                    if let Some(ref hook) = entry.hooks.on_startup {
                        hook.on_startup(&startup_ctx);
                    }
                }
                entry.state = PluginState::Active;
            }
            let _ = ctx; // consumed by startup hook
        }

        Ok(errors)
    }

    /// Unregister a plugin by ID.
    pub fn unregister_plugin(&mut self, id: &str) -> Result<PluginEntry, PluginError> {
        self.registry.remove(id).ok_or_else(|| PluginError::NotFound(id.to_string()))
    }

    /// Disable a plugin (keeps it registered but inactive).
    pub fn disable_plugin(&mut self, id: &str) -> Result<(), PluginError> {
        let entry = self.registry.get_mut(id).ok_or_else(|| PluginError::NotFound(id.to_string()))?;
        entry.state = PluginState::Disabled;
        Ok(())
    }

    /// Re-enable a disabled plugin.
    pub fn enable_plugin(&mut self, id: &str) -> Result<(), PluginError> {
        let entry = self.registry.get_mut(id).ok_or_else(|| PluginError::NotFound(id.to_string()))?;
        if entry.state != PluginState::Disabled {
            return Err(PluginError::InvalidState {
                plugin_id: id.to_string(),
                current: entry.state.to_string(),
                required: "disabled".to_string(),
            });
        }
        entry.state = PluginState::Active;
        Ok(())
    }

    // ── Query ─────────────────────────────────────────────────

    /// List all plugins with their metadata.
    pub fn list_plugins(&self) -> Vec<PluginInfo> {
        self.registry.list_plugins()
    }

    /// Get info for a specific plugin.
    pub fn get_plugin(&self, id: &str) -> Option<PluginInfo> {
        self.registry.get(id).map(PluginInfo::from)
    }

    /// Number of active plugins.
    pub fn active_count(&self) -> usize {
        self.registry.iter().filter(|e| e.state == PluginState::Active).count()
    }

    /// Total number of registered plugins.
    pub fn total_count(&self) -> usize {
        self.registry.len()
    }

    // ── Hook Dispatch ─────────────────────────────────────────

    /// Dispatch `on_tick_start` hooks to all active plugins sorted by priority.
    pub fn dispatch_tick_start(&self, ctx: &TickContext) -> Vec<(String, HookResult)> {
        let mut results = Vec::new();
        for entry in self.registry.sorted_by_priority() {
            if entry.state != PluginState::Active {
                continue;
            }
            if !entry.permissions.has(Permission::ReadWorldState) {
                continue;
            }
            if let Some(ref hook) = entry.hooks.on_tick_start {
                let result = hook.on_tick_start(ctx);
                results.push((entry.metadata.id.clone(), result.clone()));
                if result == HookResult::SkipRemaining {
                    break;
                }
            }
        }
        results
    }

    /// Dispatch `on_tick_end` hooks to all active plugins sorted by priority.
    pub fn dispatch_tick_end(&self, ctx: &TickContext) -> Vec<(String, HookResult)> {
        let mut results = Vec::new();
        for entry in self.registry.sorted_by_priority() {
            if entry.state != PluginState::Active {
                continue;
            }
            if !entry.permissions.has(Permission::ReadWorldState) {
                continue;
            }
            if let Some(ref hook) = entry.hooks.on_tick_end {
                let result = hook.on_tick_end(ctx);
                results.push((entry.metadata.id.clone(), result.clone()));
                if result == HookResult::SkipRemaining {
                    break;
                }
            }
        }
        results
    }

    /// Dispatch `on_agent_action` hooks. Returns true if the action is allowed.
    pub fn dispatch_agent_action(&self, ctx: &AgentActionContext) -> (bool, Vec<String>) {
        let mut allowed = true;
        let mut block_reasons = Vec::new();
        for entry in self.registry.sorted_by_priority() {
            if entry.state != PluginState::Active {
                continue;
            }
            if !entry.permissions.has(Permission::InterceptActions) {
                continue;
            }
            if let Some(ref hook) = entry.hooks.on_agent_action {
                match hook.on_agent_action(ctx) {
                    HookResult::Block(reason) => {
                        allowed = false;
                        block_reasons.push(format!("{}: {}", entry.metadata.id, reason));
                    }
                    HookResult::SkipRemaining => break,
                    HookResult::Continue => {}
                }
            }
        }
        (allowed, block_reasons)
    }

    /// Dispatch `on_transaction` hooks. Returns true if the transaction is allowed.
    pub fn dispatch_transaction(&self, ctx: &TransactionContext) -> (bool, Vec<String>) {
        let mut allowed = true;
        let mut block_reasons = Vec::new();
        for entry in self.registry.sorted_by_priority() {
            if entry.state != PluginState::Active {
                continue;
            }
            if !entry.permissions.has(Permission::InterceptTransactions) {
                continue;
            }
            if let Some(ref hook) = entry.hooks.on_transaction {
                match hook.on_transaction(ctx) {
                    HookResult::Block(reason) => {
                        allowed = false;
                        block_reasons.push(format!("{}: {}", entry.metadata.id, reason));
                    }
                    HookResult::SkipRemaining => break,
                    HookResult::Continue => {}
                }
            }
        }
        (allowed, block_reasons)
    }

    /// Dispatch `on_agent_spawn` hooks.
    pub fn dispatch_agent_spawn(&self, ctx: &AgentSpawnContext) -> Vec<(String, HookResult)> {
        let mut results = Vec::new();
        for entry in self.registry.sorted_by_priority() {
            if entry.state != PluginState::Active {
                continue;
            }
            if !entry.permissions.has(Permission::ReadAgents) {
                continue;
            }
            if let Some(ref hook) = entry.hooks.on_agent_spawn {
                let result = hook.on_agent_spawn(ctx);
                results.push((entry.metadata.id.clone(), result.clone()));
                if result == HookResult::SkipRemaining {
                    break;
                }
            }
        }
        results
    }

    /// Dispatch `on_event` hooks to all active plugins.
    pub fn dispatch_event(&self, ctx: &EventContext) {
        for entry in self.registry.sorted_by_priority() {
            if entry.state != PluginState::Active {
                continue;
            }
            if !entry.permissions.has(Permission::ReadEvents) {
                continue;
            }
            if let Some(ref hook) = entry.hooks.on_event {
                let _ = hook.on_event(ctx);
            }
        }
    }

    /// Dispatch `on_shutdown` hooks to all plugins.
    pub fn dispatch_shutdown(&self, ctx: &ShutdownContext) {
        for entry in self.registry.sorted_by_priority() {
            if let Some(ref hook) = entry.hooks.on_shutdown {
                hook.on_shutdown(ctx);
            }
        }
    }

    /// Get a reference to the event bus.
    pub fn event_bus(&self) -> &SharedEventBus {
        &self.event_bus
    }
}

// ── PluginSubsystemBridge ─────────────────────────────────────

/// Wraps the plugin manager into a `Subsystem` so it participates in
/// the tick loop, dispatching `on_tick_start` and `on_tick_end` hooks.
pub struct PluginSubsystemBridge {
    manager: SharedPluginManager,
}

impl PluginSubsystemBridge {
    pub fn new(manager: SharedPluginManager) -> Self {
        Self { manager }
    }
}

impl Subsystem for PluginSubsystemBridge {
    fn name(&self) -> &str {
        "plugin_bridge"
    }

    fn on_tick(
        &self,
        tick: u64,
        agents: &mut [(Uuid, u64, AgentRecord)],
    ) -> Vec<WorldEvent> {
        // NOTE: This method is called from the sync subsystem tick loop, which
        // itself runs inside a tokio async context (Scheduler::run). We cannot
        // use `blocking_lock()` here because it panics when called from a tokio
        // worker thread. Use `try_lock()` instead — if the lock is contended
        // (e.g. an API handler is querying plugins), skip this tick's hooks.
        let mgr = match self.manager.try_lock() {
            Ok(guard) => guard,
            Err(_) => {
                tracing::warn!(
                    "PluginSubsystemBridge: skipped tick {} — plugin manager lock contended",
                    tick
                );
                return Vec::new();
            }
        };

        let living = agents
            .iter()
            .filter(|(_, _, a)| a.phase != crate::world::enums::AgentPhase::Dead)
            .count();

        let tick_ctx = TickContext {
            tick,
            world: WorldSnapshot {
                tick,
                living_agents: living,
                total_agents: agents.len(),
            },
        };

        // Dispatch tick_start hooks
        let start_results = mgr.dispatch_tick_start(&tick_ctx);
        for (plugin_id, result) in &start_results {
            if let HookResult::Block(reason) = result {
                tracing::warn!(
                    "Plugin {} blocked tick_start at tick {}: {}",
                    plugin_id,
                    tick,
                    reason
                );
            }
        }

        // Dispatch tick_end hooks
        let end_results = mgr.dispatch_tick_end(&tick_ctx);
        let _ = end_results;

        // Plugins don't directly emit events through the subsystem bridge.
        // They use the EventBus directly (if they have EmitEvents permission).
        Vec::new()
    }
}
