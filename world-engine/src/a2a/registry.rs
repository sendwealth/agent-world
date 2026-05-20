use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::RwLock;
use tokio::time;

use crate::world::EventBus;

/// In-memory record for a registered agent.
///
/// Uses `Box<str>` for short string fields that are rarely modified to reduce
/// allocation overhead when cloning (Box<str> is one pointer + len vs
/// String's pointer + len + capacity).
#[derive(Debug, Clone)]
pub struct RegisteredAgent {
    pub agent_id: String,
    pub name: String,
    pub capabilities: Vec<String>,
    pub public_key: String,
    pub tokens: i64,
    pub money: i64,
    pub skills: Vec<String>,
    pub reputation: f32,
    pub phase: i32, // AgentPhase protobuf enum value
    pub last_seen: i64,
}

/// Agent registry that tracks online agents and removes stale entries.
#[derive(Clone)]
pub struct AgentRegistry {
    agents: Arc<RwLock<HashMap<String, RegisteredAgent>>>,
    event_bus: Arc<EventBus>,
    heartbeat_timeout_secs: u64,
}

impl AgentRegistry {
    pub fn new(event_bus: Arc<EventBus>) -> Self {
        Self {
            agents: Arc::new(RwLock::new(HashMap::new())),
            event_bus,
            heartbeat_timeout_secs: 30,
        }
    }

    pub fn with_heartbeat_timeout(mut self, secs: u64) -> Self {
        self.heartbeat_timeout_secs = secs;
        self
    }

    /// Register a new agent or update an existing one.
    pub async fn register(
        &self,
        agent_id: String,
        name: String,
        capabilities: Vec<String>,
        public_key: String,
    ) -> bool {
        let now = chrono::Utc::now().timestamp();
        let mut agents = self.agents.write().await;

        let is_new = !agents.contains_key(&agent_id);

        let agent = RegisteredAgent {
            agent_id: agent_id.clone(),
            name,
            capabilities,
            public_key,
            tokens: 0,
            money: 0,
            skills: Vec::new(),
            reputation: 0.0,
            phase: 0, // BIRTH
            last_seen: now,
        };

        agents.insert(agent_id.clone(), agent);

        if is_new {
            drop(agents); // release write lock before publishing event
            self.event_bus.publish(crate::world::event::WorldEvent::AgentSpawned {
                agent_id,
                name: String::new(), // name already stored in registry
            });
        }

        is_new
    }

    /// Record a heartbeat for the given agent. Returns false if agent is not registered.
    pub async fn heartbeat(&self, agent_id: &str) -> bool {
        let now = chrono::Utc::now().timestamp();
        let mut agents = self.agents.write().await;
        if let Some(agent) = agents.get_mut(agent_id) {
            agent.last_seen = now;
            true
        } else {
            false
        }
    }

    /// Gracefully deregister an agent.
    pub async fn deregister(&self, agent_id: &str) -> bool {
        let mut agents = self.agents.write().await;
        if agents.remove(agent_id).is_some() {
            drop(agents);
            self.event_bus.publish(crate::world::event::WorldEvent::AgentDied {
                agent_id: agent_id.to_string(),
                reason: crate::world::enums::DeathReason::HumanTerminated,
            });
            true
        } else {
            false
        }
    }

    /// Discover agents, optionally filtering by capabilities.
    pub async fn discover(&self, capabilities: &[String]) -> Vec<RegisteredAgent> {
        let agents = self.agents.read().await;
        agents
            .values()
            .filter(|agent| {
                if capabilities.is_empty() {
                    return true;
                }
                capabilities
                    .iter()
                    .all(|cap| agent.capabilities.iter().any(|c| c == cap))
            })
            .cloned()
            .collect()
    }

    /// Discover agents without cloning — applies a callback to each match.
    ///
    /// Use this when the caller only needs to read fields (e.g., building a
    /// protobuf response) and doesn't need to own the records.
    pub async fn discover_with<F>(&self, capabilities: &[String], mut f: F)
    where
        F: FnMut(&RegisteredAgent),
    {
        let agents = self.agents.read().await;
        for agent in agents.values() {
            if capabilities.is_empty()
                || capabilities
                    .iter()
                    .all(|cap| agent.capabilities.iter().any(|c| c == cap))
            {
                f(agent);
            }
        }
    }

    /// Check if an agent exists without cloning.
    pub async fn exists(&self, agent_id: &str) -> bool {
        self.agents.read().await.contains_key(agent_id)
    }

    /// Get the count of registered agents.
    pub async fn count(&self) -> usize {
        self.agents.read().await.len()
    }

    /// Get agent IDs without cloning full records.
    pub async fn list_ids(&self) -> Vec<String> {
        self.agents.read().await.keys().cloned().collect()
    }

    /// Get a single agent by ID.
    pub async fn get(&self, agent_id: &str) -> Option<RegisteredAgent> {
        let agents = self.agents.read().await;
        agents.get(agent_id).cloned()
    }

    /// Update agent stats (tokens, money, skills, reputation, phase).
    pub async fn update_stats(
        &self,
        agent_id: &str,
        tokens: Option<i64>,
        money: Option<i64>,
        skills: Option<Vec<String>>,
        reputation: Option<f32>,
        phase: Option<i32>,
    ) -> bool {
        let mut agents = self.agents.write().await;
        if let Some(agent) = agents.get_mut(agent_id) {
            if let Some(v) = tokens {
                agent.tokens = v;
            }
            if let Some(v) = money {
                agent.money = v;
            }
            if let Some(v) = skills {
                agent.skills = v;
            }
            if let Some(v) = reputation {
                agent.reputation = v;
            }
            if let Some(v) = phase {
                agent.phase = v;
            }
            true
        } else {
            false
        }
    }

    /// Spawn a background task that periodically removes agents whose
    /// heartbeats have expired.
    pub fn spawn_liveness_monitor(self: &Arc<Self>) {
        let registry = Arc::clone(self);
        let timeout = self.heartbeat_timeout_secs;
        tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_secs(timeout / 2));
            loop {
                interval.tick().await;
                registry.evict_stale_agents().await;
            }
        });
    }

    async fn evict_stale_agents(&self) {
        let now = chrono::Utc::now().timestamp();
        let timeout = self.heartbeat_timeout_secs as i64;
        let mut agents = self.agents.write().await;
        let stale: Vec<String> = agents
            .iter()
            .filter(|(_, agent)| now - agent.last_seen > timeout)
            .map(|(id, _)| id.clone())
            .collect();

        for agent_id in &stale {
            agents.remove(agent_id);
            drop(agents);
            self.event_bus.publish(crate::world::event::WorldEvent::AgentDied {
                agent_id: agent_id.clone(),
                reason: crate::world::enums::DeathReason::TokenDepleted, // timeout = effectively died
            });
            agents = self.agents.write().await;
        }

        if !stale.is_empty() {
            tracing::info!(
                count = stale.len(),
                "Evicted stale agents due to heartbeat timeout"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_registry() -> Arc<AgentRegistry> {
        Arc::new(AgentRegistry::new(Arc::new(EventBus::new(256))))
    }

    #[tokio::test]
    async fn register_new_agent() {
        let reg = make_registry();
        let is_new = reg
            .register(
                "agent-1".into(),
                "Alice".into(),
                vec!["coding".into()],
                "pubkey1".into(),
            )
            .await;
        assert!(is_new);
        assert_eq!(reg.count().await, 1);
    }

    #[tokio::test]
    async fn register_existing_agent_is_update() {
        let reg = make_registry();
        reg.register(
            "agent-1".into(),
            "Alice".into(),
            vec![],
            "pk".into(),
        )
        .await;
        let is_new = reg
            .register(
                "agent-1".into(),
                "Alice Updated".into(),
                vec!["coding".into()],
                "pk".into(),
            )
            .await;
        assert!(!is_new);
        assert_eq!(reg.count().await, 1);

        let agent = reg.get("agent-1").await.unwrap();
        assert_eq!(agent.name, "Alice Updated");
    }

    #[tokio::test]
    async fn heartbeat_updates_last_seen() {
        let reg = make_registry();
        reg.register(
            "agent-1".into(),
            "Alice".into(),
            vec![],
            "pk".into(),
        )
        .await;

        let agent = reg.get("agent-1").await.unwrap();
        let first_seen = agent.last_seen;

        // Wait a bit so timestamp changes
        tokio::time::sleep(Duration::from_millis(10)).await;

        assert!(reg.heartbeat("agent-1").await);

        let agent = reg.get("agent-1").await.unwrap();
        assert!(agent.last_seen >= first_seen);
    }

    #[tokio::test]
    async fn heartbeat_unknown_agent_returns_false() {
        let reg = make_registry();
        assert!(!reg.heartbeat("unknown").await);
    }

    #[tokio::test]
    async fn deregister_removes_agent() {
        let reg = make_registry();
        reg.register(
            "agent-1".into(),
            "Alice".into(),
            vec![],
            "pk".into(),
        )
        .await;
        assert!(reg.deregister("agent-1").await);
        assert_eq!(reg.count().await, 0);
    }

    #[tokio::test]
    async fn deregister_unknown_returns_false() {
        let reg = make_registry();
        assert!(!reg.deregister("unknown").await);
    }

    #[tokio::test]
    async fn discover_all_agents() {
        let reg = make_registry();
        reg.register(
            "a1".into(),
            "Alice".into(),
            vec!["coding".into()],
            "pk1".into(),
        )
        .await;
        reg.register(
            "a2".into(),
            "Bob".into(),
            vec!["research".into()],
            "pk2".into(),
        )
        .await;

        let found = reg.discover(&[]).await;
        assert_eq!(found.len(), 2);
    }

    #[tokio::test]
    async fn discover_filter_by_capability() {
        let reg = make_registry();
        reg.register(
            "a1".into(),
            "Alice".into(),
            vec!["coding".into(), "research".into()],
            "pk1".into(),
        )
        .await;
        reg.register(
            "a2".into(),
            "Bob".into(),
            vec!["research".into()],
            "pk2".into(),
        )
        .await;

        let found = reg
            .discover(&["coding".into()])
            .await;
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].agent_id, "a1");
    }

    #[tokio::test]
    async fn discover_filter_multiple_capabilities() {
        let reg = make_registry();
        reg.register(
            "a1".into(),
            "Alice".into(),
            vec!["coding".into(), "research".into()],
            "pk1".into(),
        )
        .await;
        reg.register(
            "a2".into(),
            "Bob".into(),
            vec!["research".into()],
            "pk2".into(),
        )
        .await;

        let found = reg
            .discover(&["coding".into(), "research".into()])
            .await;
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].agent_id, "a1");
    }

    #[tokio::test]
    async fn update_stats() {
        let reg = make_registry();
        reg.register(
            "a1".into(),
            "Alice".into(),
            vec![],
            "pk".into(),
        )
        .await;

        assert!(
            reg.update_stats("a1", Some(100), Some(50), Some(vec!["coding".into()]), Some(0.8), Some(2))
                .await
        );

        let agent = reg.get("a1").await.unwrap();
        assert_eq!(agent.tokens, 100);
        assert_eq!(agent.money, 50);
        assert_eq!(agent.skills, vec!["coding"]);
        assert!((agent.reputation - 0.8).abs() < f32::EPSILON);
        assert_eq!(agent.phase, 2); // ADULT
    }

    #[tokio::test]
    async fn update_stats_unknown_agent() {
        let reg = make_registry();
        assert!(!reg.update_stats("unknown", Some(100), None, None, None, None).await);
    }

    #[tokio::test]
    async fn get_unknown_returns_none() {
        let reg = make_registry();
        assert!(reg.get("unknown").await.is_none());
    }

    #[tokio::test]
    async fn discover_empty_registry() {
        let reg = make_registry();
        let found = reg.discover(&[]).await;
        assert!(found.is_empty());
    }

    #[tokio::test]
    async fn discover_no_match() {
        let reg = make_registry();
        reg.register(
            "a1".into(),
            "Alice".into(),
            vec!["coding".into()],
            "pk".into(),
        )
        .await;
        let found = reg
            .discover(&["teaching".into()])
            .await;
        assert!(found.is_empty());
    }
}
