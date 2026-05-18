use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use uuid::Uuid;

use super::event::WorldEvent;
use super::state::EventBus;

// ── Agent Status ──────────────────────────────────────────

/// Online status of an agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    Online,
    Offline,
}

impl std::fmt::Display for AgentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentStatus::Online => write!(f, "online"),
            AgentStatus::Offline => write!(f, "offline"),
        }
    }
}

// ── Agent Profile ─────────────────────────────────────────

/// Full profile of a registered agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentProfile {
    pub agent_id: String,
    pub name: String,
    /// Character traits / personality descriptors.
    #[serde(default)]
    pub traits: Vec<String>,
    /// Declared skills the agent possesses.
    #[serde(default)]
    pub skills: Vec<String>,
    pub status: AgentStatus,
    /// Epoch-millis timestamp of the last heartbeat.
    pub last_heartbeat_at: u64,
    /// Epoch-millis timestamp of initial registration.
    pub registered_at: u64,
}

// ── Errors ────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum DiscoveryError {
    #[error("agent already registered: {0}")]
    AlreadyRegistered(String),
    #[error("agent not found: {0}")]
    NotFound(String),
    #[error("name is required")]
    NameRequired,
}

// ── Agent Registry ────────────────────────────────────────

/// In-memory registry of agents with automatic discovery support.
///
/// Agents register themselves, send periodic heartbeats to stay "online",
/// and are marked "offline" (or removed) after a configurable timeout.
pub struct AgentRegistry {
    agents: HashMap<String, AgentProfile>,
    event_bus: Option<EventBus>,
    heartbeat_timeout: Duration,
}

/// How long an agent can go without a heartbeat before being marked offline.
const DEFAULT_HEARTBEAT_TIMEOUT_SECS: u64 = 30;

impl AgentRegistry {
    /// Create a new, empty registry.
    pub fn new() -> Self {
        Self {
            agents: HashMap::new(),
            event_bus: None,
            heartbeat_timeout: Duration::from_secs(DEFAULT_HEARTBEAT_TIMEOUT_SECS),
        }
    }

    /// Create a registry wired to an EventBus for publishing discovery events.
    pub fn with_event_bus(event_bus: EventBus) -> Self {
        Self {
            agents: HashMap::new(),
            event_bus: Some(event_bus),
            heartbeat_timeout: Duration::from_secs(DEFAULT_HEARTBEAT_TIMEOUT_SECS),
        }
    }

    /// Set a custom heartbeat timeout (how long before an agent is considered offline).
    pub fn with_heartbeat_timeout(mut self, timeout: Duration) -> Self {
        self.heartbeat_timeout = timeout;
        self
    }

    // ── Registration ──────────────────────────────────────

    /// Register a new agent. Returns the generated agent ID.
    ///
    /// Publishes an `AgentRegistered` event when successful.
    pub fn register(
        &mut self,
        name: String,
        traits: Vec<String>,
        skills: Vec<String>,
    ) -> Result<String, DiscoveryError> {
        if name.is_empty() {
            return Err(DiscoveryError::NameRequired);
        }

        let agent_id = Uuid::new_v4().to_string();
        let now = current_epoch_millis();

        let profile = AgentProfile {
            agent_id: agent_id.clone(),
            name,
            traits,
            skills,
            status: AgentStatus::Online,
            last_heartbeat_at: now,
            registered_at: now,
        };

        self.agents.insert(agent_id.clone(), profile);

        if let Some(ref bus) = self.event_bus {
            bus.emit(WorldEvent::AgentRegistered {
                agent_id: agent_id.clone(),
                name: self.agents.get(&agent_id).unwrap().name.clone(),
            });
        }

        Ok(agent_id)
    }

    /// Register an agent with a pre-assigned ID (used for genesis / recovery).
    pub fn register_with_id(
        &mut self,
        agent_id: String,
        name: String,
        traits: Vec<String>,
        skills: Vec<String>,
    ) -> Result<(), DiscoveryError> {
        if name.is_empty() {
            return Err(DiscoveryError::NameRequired);
        }
        if self.agents.contains_key(&agent_id) {
            return Err(DiscoveryError::AlreadyRegistered(agent_id));
        }

        let now = current_epoch_millis();

        let profile = AgentProfile {
            agent_id: agent_id.clone(),
            name: name.clone(),
            traits,
            skills,
            status: AgentStatus::Online,
            last_heartbeat_at: now,
            registered_at: now,
        };

        self.agents.insert(agent_id.clone(), profile);

        if let Some(ref bus) = self.event_bus {
            bus.emit(WorldEvent::AgentRegistered {
                agent_id,
                name,
            });
        }

        Ok(())
    }

    /// Remove (deregister) an agent from the registry.
    ///
    /// Publishes a `AgentDeregistered` event when successful.
    pub fn deregister(&mut self, agent_id: &str) -> Result<AgentProfile, DiscoveryError> {
        let profile = self
            .agents
            .remove(agent_id)
            .ok_or_else(|| DiscoveryError::NotFound(agent_id.to_string()))?;

        if let Some(ref bus) = self.event_bus {
            bus.emit(WorldEvent::AgentDeregistered {
                agent_id: agent_id.to_string(),
                name: profile.name.clone(),
            });
        }

        Ok(profile)
    }

    // ── Heartbeat ─────────────────────────────────────────

    /// Record a heartbeat for an agent, keeping it online.
    ///
    /// Publishes a `AgentHeartbeat` event.
    pub fn heartbeat(&mut self, agent_id: &str) -> Result<(), DiscoveryError> {
        let profile = self
            .agents
            .get_mut(agent_id)
            .ok_or_else(|| DiscoveryError::NotFound(agent_id.to_string()))?;

        profile.status = AgentStatus::Online;
        profile.last_heartbeat_at = current_epoch_millis();

        if let Some(ref bus) = self.event_bus {
            bus.emit(WorldEvent::AgentHeartbeat {
                agent_id: agent_id.to_string(),
                timestamp: profile.last_heartbeat_at,
            });
        }

        Ok(())
    }

    /// Mark all agents whose heartbeat has exceeded the timeout as offline.
    ///
    /// Should be called periodically (e.g. every few seconds) by a background
    /// task in the server.
    pub fn expire_stale_agents(&mut self) -> Vec<String> {
        let now = current_epoch_millis();
        let timeout_ms = self.heartbeat_timeout.as_millis() as u64;
        let mut expired = Vec::new();

        for profile in self.agents.values_mut() {
            if profile.status == AgentStatus::Online
                && now.saturating_sub(profile.last_heartbeat_at) > timeout_ms
            {
                profile.status = AgentStatus::Offline;
                expired.push(profile.agent_id.clone());
            }
        }

        expired
    }

    // ── Queries ───────────────────────────────────────────

    /// Get a single agent's profile by ID.
    pub fn get(&self, agent_id: &str) -> Option<&AgentProfile> {
        self.agents.get(agent_id)
    }

    /// List all registered agents, optionally filtered.
    pub fn list(&self) -> Vec<&AgentProfile> {
        self.agents.values().collect()
    }

    /// List agents filtered by online/offline status.
    pub fn list_by_status(&self, status: AgentStatus) -> Vec<&AgentProfile> {
        self.agents
            .values()
            .filter(|p| p.status == status)
            .collect()
    }

    /// Search for agents that have a specific skill.
    pub fn find_by_skill(&self, skill: &str) -> Vec<&AgentProfile> {
        let skill_lower = skill.to_lowercase();
        self.agents
            .values()
            .filter(|p| {
                p.skills
                    .iter()
                    .any(|s| s.to_lowercase() == skill_lower)
            })
            .collect()
    }

    /// Search for agents matching any of the given skills.
    pub fn find_by_skills(&self, skills: &[String]) -> Vec<&AgentProfile> {
        let skills_lower: Vec<String> = skills.iter().map(|s| s.to_lowercase()).collect();
        self.agents
            .values()
            .filter(|p| {
                p.skills
                    .iter()
                    .any(|s| skills_lower.contains(&s.to_lowercase()))
            })
            .collect()
    }

    /// Total number of registered agents.
    pub fn count(&self) -> usize {
        self.agents.len()
    }

    /// Number of online agents.
    pub fn count_online(&self) -> usize {
        self.agents
            .values()
            .filter(|p| p.status == AgentStatus::Online)
            .count()
    }

    /// Update an agent's profile fields (name, traits, skills).
    pub fn update_profile(
        &mut self,
        agent_id: &str,
        name: Option<String>,
        traits: Option<Vec<String>>,
        skills: Option<Vec<String>>,
    ) -> Result<(), DiscoveryError> {
        let profile = self
            .agents
            .get_mut(agent_id)
            .ok_or_else(|| DiscoveryError::NotFound(agent_id.to_string()))?;

        if let Some(n) = name {
            if n.is_empty() {
                return Err(DiscoveryError::NameRequired);
            }
            profile.name = n;
        }
        if let Some(t) = traits {
            profile.traits = t;
        }
        if let Some(s) = skills {
            profile.skills = s;
        }

        Ok(())
    }
}

/// Shared reference-counted registry.
pub type SharedAgentRegistry = Arc<Mutex<AgentRegistry>>;

// ── Helpers ───────────────────────────────────────────────

fn current_epoch_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("time went backwards")
        .as_millis() as u64
}

// ── Tests ─────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_agent_basic() {
        let mut registry = AgentRegistry::new();
        let id = registry
            .register("Alice".into(), vec!["brave".into()], vec!["mining".into()])
            .unwrap();

        let profile = registry.get(&id).unwrap();
        assert_eq!(profile.name, "Alice");
        assert_eq!(profile.traits, vec!["brave"]);
        assert_eq!(profile.skills, vec!["mining"]);
        assert_eq!(profile.status, AgentStatus::Online);
    }

    #[test]
    fn register_rejects_empty_name() {
        let mut registry = AgentRegistry::new();
        let result = registry.register("".into(), vec![], vec![]);
        assert!(matches!(result, Err(DiscoveryError::NameRequired)));
    }

    #[test]
    fn register_with_id_rejects_duplicate() {
        let mut registry = AgentRegistry::new();
        registry
            .register_with_id("a1".into(), "Alice".into(), vec![], vec![])
            .unwrap();
        let result =
            registry.register_with_id("a1".into(), "Alice".into(), vec![], vec![]);
        assert!(matches!(result, Err(DiscoveryError::AlreadyRegistered(_))));
    }

    #[test]
    fn deregister_agent() {
        let mut registry = AgentRegistry::new();
        let id = registry.register("Bob".into(), vec![], vec![]).unwrap();
        let removed = registry.deregister(&id).unwrap();
        assert_eq!(removed.name, "Bob");
        assert!(registry.get(&id).is_none());
    }

    #[test]
    fn deregister_not_found() {
        let mut registry = AgentRegistry::new();
        let result = registry.deregister("nonexistent");
        assert!(matches!(result, Err(DiscoveryError::NotFound(_))));
    }

    #[test]
    fn heartbeat_keeps_agent_online() {
        let mut registry = AgentRegistry::new();
        let id = registry.register("Charlie".into(), vec![], vec![]).unwrap();

        // Manually age the heartbeat
        {
            let profile = registry.agents.get_mut(&id).unwrap();
            profile.last_heartbeat_at = 0; // long ago
            profile.status = AgentStatus::Offline;
        }

        registry.heartbeat(&id).unwrap();
        let profile = registry.get(&id).unwrap();
        assert_eq!(profile.status, AgentStatus::Online);
        assert!(profile.last_heartbeat_at > 0);
    }

    #[test]
    fn heartbeat_not_found() {
        let mut registry = AgentRegistry::new();
        let result = registry.heartbeat("nonexistent");
        assert!(matches!(result, Err(DiscoveryError::NotFound(_))));
    }

    #[test]
    fn expire_stale_agents() {
        let mut registry =
            AgentRegistry::new().with_heartbeat_timeout(Duration::from_millis(50));

        let id1 = registry.register("Fresh".into(), vec![], vec![]).unwrap();
        let id2 = registry.register("Stale".into(), vec![], vec![]).unwrap();

        // Age id2's heartbeat
        {
            let profile = registry.agents.get_mut(&id2).unwrap();
            profile.last_heartbeat_at = 0;
        }

        let expired = registry.expire_stale_agents();
        assert_eq!(expired, vec![id2.clone()]);

        assert_eq!(registry.get(&id1).unwrap().status, AgentStatus::Online);
        assert_eq!(registry.get(&id2).unwrap().status, AgentStatus::Offline);
    }

    #[test]
    fn find_by_skill_case_insensitive() {
        let mut registry = AgentRegistry::new();
        registry
            .register("Alice".into(), vec![], vec!["Mining".into()])
            .unwrap();
        registry
            .register("Bob".into(), vec![], vec!["Fishing".into()])
            .unwrap();
        registry
            .register("Carol".into(), vec![], vec!["mining".into(), "crafting".into()])
            .unwrap();

        let miners = registry.find_by_skill("mining");
        assert_eq!(miners.len(), 2);
        let names: Vec<&str> = miners.iter().map(|p| p.name.as_str()).collect();
        assert!(names.contains(&"Alice"));
        assert!(names.contains(&"Carol"));
    }

    #[test]
    fn find_by_skills_multiple() {
        let mut registry = AgentRegistry::new();
        registry
            .register("Alice".into(), vec![], vec!["mining".into()])
            .unwrap();
        registry
            .register("Bob".into(), vec![], vec!["fishing".into()])
            .unwrap();
        registry
            .register("Carol".into(), vec![], vec!["crafting".into()])
            .unwrap();

        let results =
            registry.find_by_skills(&["mining".into(), "fishing".into()]);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn list_by_status() {
        let mut registry =
            AgentRegistry::new().with_heartbeat_timeout(Duration::from_millis(50));

        let id1 = registry.register("Online".into(), vec![], vec![]).unwrap();
        let id2 = registry.register("ToBeOffline".into(), vec![], vec![]).unwrap();

        // Age id2
        {
            let profile = registry.agents.get_mut(&id2).unwrap();
            profile.last_heartbeat_at = 0;
        }
        registry.expire_stale_agents();

        let online = registry.list_by_status(AgentStatus::Online);
        let offline = registry.list_by_status(AgentStatus::Offline);
        assert_eq!(online.len(), 1);
        assert_eq!(offline.len(), 1);
    }

    #[test]
    fn count_and_count_online() {
        let mut registry =
            AgentRegistry::new().with_heartbeat_timeout(Duration::from_millis(50));

        registry.register("A".into(), vec![], vec![]).unwrap();
        let id2 = registry.register("B".into(), vec![], vec![]).unwrap();
        registry.register("C".into(), vec![], vec![]).unwrap();

        // Age id2
        {
            let profile = registry.agents.get_mut(&id2).unwrap();
            profile.last_heartbeat_at = 0;
        }
        registry.expire_stale_agents();

        assert_eq!(registry.count(), 3);
        assert_eq!(registry.count_online(), 2);
    }

    #[test]
    fn update_profile() {
        let mut registry = AgentRegistry::new();
        let id = registry
            .register("Alice".into(), vec!["brave".into()], vec!["mining".into()])
            .unwrap();

        registry
            .update_profile(
                &id,
                Some("Alice Updated".into()),
                Some(vec!["cautious".into()]),
                Some(vec!["fishing".into(), "crafting".into()]),
            )
            .unwrap();

        let profile = registry.get(&id).unwrap();
        assert_eq!(profile.name, "Alice Updated");
        assert_eq!(profile.traits, vec!["cautious"]);
        assert_eq!(profile.skills, vec!["fishing", "crafting"]);
    }

    #[test]
    fn update_profile_rejects_empty_name() {
        let mut registry = AgentRegistry::new();
        let id = registry.register("Alice".into(), vec![], vec![]).unwrap();

        let result = registry.update_profile(&id, Some("".into()), None, None);
        assert!(matches!(result, Err(DiscoveryError::NameRequired)));
    }

    #[test]
    fn register_emits_event() {
        let bus = EventBus::new(64);
        let mut rx = bus.subscribe();
        let mut registry = AgentRegistry::with_event_bus(bus);

        let id = registry.register("Eve".into(), vec![], vec![]).unwrap();

        let event = rx.try_recv().unwrap();
        match event {
            WorldEvent::AgentRegistered { agent_id, name } => {
                assert_eq!(agent_id, id);
                assert_eq!(name, "Eve");
            }
            other => panic!("expected AgentRegistered, got {:?}", other),
        }
    }

    #[test]
    fn deregister_emits_event() {
        let bus = EventBus::new(64);
        let mut rx = bus.subscribe();
        let mut registry = AgentRegistry::with_event_bus(bus);

        let id = registry.register("Eve".into(), vec![], vec![]).unwrap();
        // consume the AgentRegistered event
        let _ = rx.try_recv();

        registry.deregister(&id).unwrap();

        let event = rx.try_recv().unwrap();
        match event {
            WorldEvent::AgentDeregistered { agent_id, name } => {
                assert_eq!(agent_id, id);
                assert_eq!(name, "Eve");
            }
            other => panic!("expected AgentDeregistered, got {:?}", other),
        }
    }

    #[test]
    fn heartbeat_emits_event() {
        let bus = EventBus::new(64);
        let mut rx = bus.subscribe();
        let mut registry = AgentRegistry::with_event_bus(bus);

        let id = registry.register("Eve".into(), vec![], vec![]).unwrap();
        // consume the AgentRegistered event
        let _ = rx.try_recv();

        registry.heartbeat(&id).unwrap();

        let event = rx.try_recv().unwrap();
        match event {
            WorldEvent::AgentHeartbeat { agent_id, timestamp } => {
                assert_eq!(agent_id, id);
                assert!(timestamp > 0);
            }
            other => panic!("expected AgentHeartbeat, got {:?}", other),
        }
    }
}
