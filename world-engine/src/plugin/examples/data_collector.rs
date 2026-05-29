//! Example: Data Collector Plugin
//!
//! A plugin that collects world statistics every N ticks for analytics.
//! Demonstrates `OnTickEnd` and `OnEvent` hooks with read-only permissions.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crate::plugin::{
    EventContext, HookResult, OnEvent, OnTickEnd, PluginHooks, PluginManager, PluginMetadata,
    Permission, PermissionSet, TickContext,
};

/// Atomic counters for thread-safe statistics.
#[derive(Default)]
pub struct Stats {
    pub total_ticks_seen: AtomicU64,
    pub total_events_seen: AtomicU64,
    pub transactions_observed: AtomicU64,
    pub agents_spawned: AtomicU64,
}

/// A plugin that collects world statistics for analytics.
///
/// Configuration (via genesis.yaml):
/// ```yaml
/// plugins:
///   data-collector:
///     log_interval: 100   # Log stats every 100 ticks
/// ```
pub struct DataCollectorPlugin {
    /// Log statistics every N ticks.
    pub log_interval: u64,
    /// Shared statistics counters.
    pub stats: Arc<Stats>,
}

impl DataCollectorPlugin {
    pub fn new(log_interval: u64) -> Self {
        Self {
            log_interval,
            stats: Arc::new(Stats::default()),
        }
    }

    /// Default: log every 100 ticks.
    pub fn default_plugin() -> Self {
        Self::new(100)
    }

    /// Register this plugin with the given manager.
    pub fn register(self, manager: &mut PluginManager) -> Result<(), crate::plugin::PluginError> {
        let metadata = PluginMetadata {
            id: "com.agent-world.data-collector".to_string(),
            name: "Data Collector Plugin".to_string(),
            version: "1.0.0".to_string(),
            description: "Collects world statistics every N ticks for analytics.".to_string(),
            author: "Agent World Team".to_string(),
            priority: 200, // Run after most other plugins
        };

        let permissions = PermissionSet::from_iter([
            Permission::ReadWorldState,
            Permission::ReadAgents,
            Permission::ReadEvents,
        ]);

        let hooks = PluginHooks {
            on_tick_end: Some(Box::new(self.clone())),
            on_event: Some(Box::new(self)),
            ..Default::default()
        };

        manager.register_plugin(metadata, permissions, hooks)
    }
}

impl Clone for DataCollectorPlugin {
    fn clone(&self) -> Self {
        Self {
            log_interval: self.log_interval,
            stats: self.stats.clone(),
        }
    }
}

impl OnTickEnd for DataCollectorPlugin {
    fn on_tick_end(&self, ctx: &TickContext) -> HookResult {
        self.stats.total_ticks_seen.fetch_add(1, Ordering::Relaxed);

        if ctx.tick > 0 && ctx.tick % self.log_interval == 0 {
            tracing::info!(
                "[DataCollector] Tick {}: living={}, total={}, events={}, transactions={}, spawns={}",
                ctx.tick,
                ctx.world.living_agents,
                ctx.world.total_agents,
                self.stats.total_events_seen.load(Ordering::Relaxed),
                self.stats.transactions_observed.load(Ordering::Relaxed),
                self.stats.agents_spawned.load(Ordering::Relaxed),
            );
        }

        HookResult::Continue
    }
}

impl OnEvent for DataCollectorPlugin {
    fn on_event(&self, ctx: &EventContext) -> HookResult {
        self.stats.total_events_seen.fetch_add(1, Ordering::Relaxed);

        match &ctx.event {
            crate::world::event::WorldEvent::TransactionCompleted { .. } => {
                self.stats.transactions_observed.fetch_add(1, Ordering::Relaxed);
            }
            crate::world::event::WorldEvent::AgentSpawned { .. } => {
                self.stats.agents_spawned.fetch_add(1, Ordering::Relaxed);
            }
            _ => {}
        }

        HookResult::Continue
    }
}
