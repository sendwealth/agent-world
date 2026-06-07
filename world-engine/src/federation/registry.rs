//! World Registry — centralized registry for discovering and tracking world instances.
//!
//! Phase A: in-process centralized registry. Phase B will add Gossip-based discovery.

use std::collections::HashMap;
use std::sync::Arc;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::world::state::EventBus;

// ── World Status ──────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorldStatus {
    Online,
    Offline,
    Draining,
    Maintenance,
}

impl std::fmt::Display for WorldStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorldStatus::Online => write!(f, "online"),
            WorldStatus::Offline => write!(f, "offline"),
            WorldStatus::Draining => write!(f, "draining"),
            WorldStatus::Maintenance => write!(f, "maintenance"),
        }
    }
}

// ── World Endpoint ────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldEndpoint {
    pub host: String,
    pub grpc_port: u32,
    pub http_port: u32,
}

// ── World Metrics ─────────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorldMetrics {
    pub total_ticks: u64,
    pub alive_agents: u32,
    pub avg_reputation: f64,
    pub total_tokens: u64,
    pub total_money: u64,
}

// ── World Entry ───────────────────────────────────────────

/// A registered world instance in the federation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldEntry {
    pub world_id: String,
    pub name: String,
    pub description: String,
    pub endpoint: WorldEndpoint,
    pub status: WorldStatus,
    pub capabilities: Vec<String>,
    pub max_agents: u32,
    pub current_agents: u32,
    pub labels: HashMap<String, String>,
    pub metrics: WorldMetrics,
    pub registered_at: String,
    pub last_heartbeat: String,
}

// ── World Registry ────────────────────────────────────────

/// Centralized world registry — tracks all known world instances.
#[derive(Clone)]
pub struct WorldRegistry {
    worlds: Arc<RwLock<HashMap<String, WorldEntry>>>,
    event_bus: Arc<EventBus>,
    heartbeat_timeout_secs: u64,
}

impl WorldRegistry {
    pub fn new(event_bus: Arc<EventBus>) -> Self {
        Self {
            worlds: Arc::new(RwLock::new(HashMap::new())),
            event_bus,
            heartbeat_timeout_secs: 90,
        }
    }

    pub fn with_heartbeat_timeout(mut self, secs: u64) -> Self {
        self.heartbeat_timeout_secs = secs;
        self
    }

    /// Register a new world instance.
    pub async fn register(&self, entry: WorldEntry) -> Result<bool, String> {
        let world_id = entry.world_id.clone();
        let name = entry.name.clone();
        let endpoint = format!("{}:{}", entry.endpoint.host, entry.endpoint.grpc_port);
        let mut worlds = self.worlds.write().await;

        let is_new = !worlds.contains_key(&world_id);
        worlds.insert(world_id.clone(), entry);

        drop(worlds);

        if is_new {
            self.event_bus.emit(crate::world::event::WorldEvent::ForeignWorldDiscovered {
                world_id,
                name,
                endpoint,
            });
        }

        Ok(is_new)
    }

    /// Deregister a world instance.
    pub async fn deregister(&self, world_id: &str) -> bool {
        let mut worlds = self.worlds.write().await;
        let removed = worlds.remove(world_id);
        let was_present = removed.is_some();
        let name = removed.map(|e| e.name).unwrap_or_default();
        drop(worlds);

        if was_present {
            self.event_bus.emit(crate::world::event::WorldEvent::ForeignWorldDeregistered {
                world_id: world_id.to_string(),
                name,
            });
        }

        was_present
    }

    /// Record a heartbeat for a world.
    pub async fn heartbeat(&self, world_id: &str, metrics: WorldMetrics) -> bool {
        let mut worlds = self.worlds.write().await;
        if let Some(entry) = worlds.get_mut(world_id) {
            entry.last_heartbeat = Utc::now().to_rfc3339();
            entry.metrics = metrics;
            entry.status = WorldStatus::Online;
            true
        } else {
            false
        }
    }

    /// Discover worlds matching optional filters.
    pub async fn discover(
        &self,
        capability_filters: &[String],
        label_filters: &[String],
        status_filters: &[WorldStatus],
    ) -> Vec<WorldEntry> {
        let worlds = self.worlds.read().await;
        worlds
            .values()
            .filter(|w| {
                // Status filter
                if !status_filters.is_empty() && !status_filters.contains(&w.status) {
                    return false;
                }
                // Capability filter — world must have ALL requested capabilities
                if !capability_filters.is_empty() {
                    let has_all = capability_filters
                        .iter()
                        .all(|cap| w.capabilities.contains(cap));
                    if !has_all {
                        return false;
                    }
                }
                // Label filter — world must have ALL requested labels
                if !label_filters.is_empty() {
                    let has_all = label_filters
                        .iter()
                        .all(|label| w.labels.contains_key(label));
                    if !has_all {
                        return false;
                    }
                }
                true
            })
            .cloned()
            .collect()
    }

    /// Get a specific world by ID.
    pub async fn get_world(&self, world_id: &str) -> Option<WorldEntry> {
        let worlds = self.worlds.read().await;
        worlds.get(world_id).cloned()
    }

    /// List all registered worlds.
    pub async fn list_all(&self) -> Vec<WorldEntry> {
        let worlds = self.worlds.read().await;
        worlds.values().cloned().collect()
    }

    /// Count registered worlds.
    pub async fn count(&self) -> usize {
        let worlds = self.worlds.read().await;
        worlds.len()
    }

    /// Check liveness of all registered worlds and mark stale ones as offline.
    pub async fn check_liveness(&self) {
        let timeout = self.heartbeat_timeout_secs;
        let now = Utc::now();

        let mut worlds = self.worlds.write().await;
        for entry in worlds.values_mut() {
            if entry.status == WorldStatus::Online {
                if let Ok(last_hb) = chrono::DateTime::parse_from_rfc3339(&entry.last_heartbeat) {
                    let elapsed = (now - last_hb.with_timezone(&Utc)).num_seconds();
                    if elapsed > timeout as i64 {
                        entry.status = WorldStatus::Offline;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_entry(id: &str, name: &str) -> WorldEntry {
        WorldEntry {
            world_id: id.to_string(),
            name: name.to_string(),
            description: format!("Test world {}", name),
            endpoint: WorldEndpoint {
                host: "localhost".into(),
                grpc_port: 50051,
                http_port: 8080,
            },
            status: WorldStatus::Online,
            capabilities: vec!["trade".into(), "migration".into()],
            max_agents: 100,
            current_agents: 10,
            labels: HashMap::from([("region".into(), "us-west".into())]),
            metrics: WorldMetrics::default(),
            registered_at: Utc::now().to_rfc3339(),
            last_heartbeat: Utc::now().to_rfc3339(),
        }
    }

    #[tokio::test]
    async fn test_register_and_discover() {
        let event_bus = Arc::new(EventBus::new(64));
        let registry = WorldRegistry::new(event_bus);

        let entry = test_entry("world-1", "Alpha");
        let is_new = registry.register(entry).await.unwrap();
        assert!(is_new);

        let worlds = registry.discover(&[], &[], &[]).await;
        assert_eq!(worlds.len(), 1);
        assert_eq!(worlds[0].world_id, "world-1");
    }

    #[tokio::test]
    async fn test_discover_with_capability_filter() {
        let event_bus = Arc::new(EventBus::new(64));
        let registry = WorldRegistry::new(event_bus);

        let mut entry1 = test_entry("world-1", "Alpha");
        entry1.capabilities = vec!["trade".into(), "migration".into()];

        let mut entry2 = test_entry("world-2", "Beta");
        entry2.capabilities = vec!["trade".into()];

        registry.register(entry1).await.unwrap();
        registry.register(entry2).await.unwrap();

        let migration_worlds = registry.discover(&["migration".into()], &[], &[]).await;
        assert_eq!(migration_worlds.len(), 1);
        assert_eq!(migration_worlds[0].world_id, "world-1");
    }

    #[tokio::test]
    async fn test_deregister() {
        let event_bus = Arc::new(EventBus::new(64));
        let registry = WorldRegistry::new(event_bus);

        let entry = test_entry("world-1", "Alpha");
        registry.register(entry).await.unwrap();

        assert!(registry.deregister("world-1").await);
        assert!(!registry.deregister("world-1").await);
        assert_eq!(registry.count().await, 0);
    }

    #[tokio::test]
    async fn test_heartbeat() {
        let event_bus = Arc::new(EventBus::new(64));
        let registry = WorldRegistry::new(event_bus);

        let entry = test_entry("world-1", "Alpha");
        registry.register(entry).await.unwrap();

        let metrics = WorldMetrics {
            total_ticks: 100,
            alive_agents: 42,
            ..Default::default()
        };

        assert!(registry.heartbeat("world-1", metrics.clone()).await);
        assert!(!registry.heartbeat("unknown", metrics).await);

        let world = registry.get_world("world-1").await.unwrap();
        assert_eq!(world.metrics.alive_agents, 42);
    }
}
