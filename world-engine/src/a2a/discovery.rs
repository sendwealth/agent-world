use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Information about a registered agent in the world.
#[derive(Debug, Clone)]
pub struct AgentRecord {
    pub agent_id: String,
    pub name: String,
    pub tokens: i64,
    pub money: i64,
    pub skills: Vec<String>,
    pub reputation: f32,
    pub phase: String,
}

/// Thread-safe registry of agents in the world.
/// Supports registration, deregistration, and discovery queries.
#[derive(Debug, Clone)]
pub struct AgentRegistry {
    agents: Arc<RwLock<HashMap<String, AgentRecord>>>,
}

impl AgentRegistry {
    pub fn new() -> Self {
        Self {
            agents: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a new agent or update an existing one.
    pub async fn register(&self, record: AgentRecord) {
        let mut agents = self.agents.write().await;
        agents.insert(record.agent_id.clone(), record);
    }

    /// Remove an agent from the registry.
    pub async fn deregister(&self, agent_id: &str) -> Option<AgentRecord> {
        let mut agents = self.agents.write().await;
        agents.remove(agent_id)
    }

    /// Look up a single agent by ID.
    pub async fn get(&self, agent_id: &str) -> Option<AgentRecord> {
        let agents = self.agents.read().await;
        agents.get(agent_id).cloned()
    }

    /// Discover agents, optionally filtering by capabilities.
    /// If `capabilities` is empty, returns all registered agents.
    pub async fn discover(&self, capabilities: &[String]) -> Vec<AgentRecord> {
        let agents = self.agents.read().await;
        let records: Vec<AgentRecord> = agents.values().cloned().collect();

        if capabilities.is_empty() {
            return records;
        }

        records
            .into_iter()
            .filter(|agent| {
                capabilities
                    .iter()
                    .any(|cap| agent.skills.iter().any(|s| s == cap))
            })
            .collect()
    }

    /// List all registered agent IDs.
    pub async fn list_ids(&self) -> Vec<String> {
        let agents = self.agents.read().await;
        agents.keys().cloned().collect()
    }

    /// Update an agent's phase.
    pub async fn update_phase(&self, agent_id: &str, phase: &str) -> bool {
        let mut agents = self.agents.write().await;
        if let Some(agent) = agents.get_mut(agent_id) {
            agent.phase = phase.to_string();
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn register_and_discover() {
        let registry = AgentRegistry::new();
        let agent = AgentRecord {
            agent_id: "agent-001".into(),
            name: "Alice".into(),
            tokens: 1000,
            money: 500,
            skills: vec!["coding".into(), "trading".into()],
            reputation: 4.5,
            phase: "adult".into(),
        };
        registry.register(agent).await;

        let results = registry.discover(&[]).await;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].agent_id, "agent-001");
    }

    #[tokio::test]
    async fn discover_filter_by_capability() {
        let registry = AgentRegistry::new();
        registry
            .register(AgentRecord {
                agent_id: "a1".into(),
                name: "Alice".into(),
                tokens: 100,
                money: 0,
                skills: vec!["coding".into()],
                reputation: 0.0,
                phase: "adult".into(),
            })
            .await;
        registry
            .register(AgentRecord {
                agent_id: "a2".into(),
                name: "Bob".into(),
                tokens: 200,
                money: 0,
                skills: vec!["trading".into()],
                reputation: 0.0,
                phase: "adult".into(),
            })
            .await;

        let coders = registry.discover(&["coding".into()]).await;
        assert_eq!(coders.len(), 1);
        assert_eq!(coders[0].agent_id, "a1");
    }

    #[tokio::test]
    async fn deregister_agent() {
        let registry = AgentRegistry::new();
        registry
            .register(AgentRecord {
                agent_id: "a1".into(),
                name: "Alice".into(),
                tokens: 0,
                money: 0,
                skills: vec![],
                reputation: 0.0,
                phase: "birth".into(),
            })
            .await;
        assert!(registry.deregister("a1").await.is_some());
        assert!(registry.get("a1").await.is_none());
    }
}
