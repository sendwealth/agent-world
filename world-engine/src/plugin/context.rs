//! Plugin context types.
//!
//! Contexts provide plugins with controlled, read-only or scoped access
//! to engine state during hook invocations.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::world::event::WorldEvent;
use crate::world::state::SharedEventBus;

/// Context provided to plugins during initialization.
pub struct PluginContext {
    /// Plugin-specific configuration from genesis.yaml.
    pub config: HashMap<String, serde_yaml::Value>,
    /// Shared reference to the event bus (requires `ReadEvents` or `EmitEvents`).
    pub event_bus: SharedEventBus,
}

/// Read-only snapshot of world state provided during hooks.
#[derive(Debug, Clone)]
pub struct WorldSnapshot {
    /// Current tick number.
    pub tick: u64,
    /// Number of living agents.
    pub living_agents: usize,
    /// Total agents (including dead).
    pub total_agents: usize,
}

/// Context provided during transaction hooks.
#[derive(Debug, Clone)]
pub struct TransactionContext {
    /// Sender agent ID.
    pub from: String,
    /// Receiver agent ID.
    pub to: String,
    /// Transaction amount.
    pub amount: u64,
    /// Currency type as string.
    pub currency: String,
    /// Optional description or reason for the transaction.
    pub reason: Option<String>,
}

/// Context provided during agent action hooks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentActionContext {
    /// Agent performing the action.
    pub agent_id: String,
    /// Action type (move, gather, trade, etc.).
    pub action: String,
    /// Action parameters as JSON.
    pub params: serde_json::Value,
}

/// Context provided during agent spawn hooks.
#[derive(Debug, Clone)]
pub struct AgentSpawnContext {
    /// New agent's ID.
    pub agent_id: Uuid,
    /// Agent name.
    pub name: String,
    /// Initial token balance.
    pub initial_tokens: u64,
}

/// Context provided during tick hooks.
#[derive(Debug, Clone)]
pub struct TickContext {
    /// Current tick number.
    pub tick: u64,
    /// Snapshot of world state at the start of this tick.
    pub world: WorldSnapshot,
}

/// Event context for OnEvent hooks.
#[derive(Debug, Clone)]
pub struct EventContext {
    /// The event that was emitted.
    pub event: WorldEvent,
    /// Tick when the event was generated.
    pub tick: u64,
}

/// Startup context provided once after all plugins are loaded.
pub struct StartupContext {
    /// Total number of active plugins.
    pub plugin_count: usize,
    /// Initial world tick.
    pub initial_tick: u64,
}

/// Shutdown context provided before the engine stops.
pub struct ShutdownContext {
    /// Final tick number.
    pub final_tick: u64,
}
