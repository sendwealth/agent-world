//! Persistence layer — structured SQLite snapshots of world state.
//!
//! This module provides a snapshot-based persistence mechanism layered on top
//! of the existing WAL (event-level) system. The SQLite layer stores structured
//! world state for fast recovery, while the WAL handles incremental event
//! replay between snapshots.

pub mod sqlite;
pub use sqlite::SqlitePersistence;

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::economy::token_burn::{AgentRecord, SkillRecord};
use crate::world::enums::AgentPhase;

/// Intermediate serializable representation of world state.
///
/// `WorldState` contains `SharedEventBus` (Arc<EventBus>) and
/// `SubsystemRegistry` (Vec<Box<dyn Subsystem>>) which are not serializable.
/// This type captures only the persistent data: tick counter and agent roster.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializableWorldState {
    pub tick: u64,
    pub agents: Vec<SerializableAgentEntry>,
    pub timestamp: i64,
}

/// A single agent entry suitable for persistence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializableAgentEntry {
    pub agent_id: Uuid,
    pub spawn_tick: u64,
    pub record: SerializableAgentRecord,
}

/// Serializable version of AgentRecord.
///
/// Mirrors `crate::economy::token_burn::AgentRecord` but is an independent
/// type so the persistence layer doesn't couple to economy internals.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializableAgentRecord {
    pub id: Uuid,
    pub name: String,
    pub phase: AgentPhase,
    pub tokens: u64,
    pub skills: HashMap<String, SkillRecord>,
}

impl SerializableAgentRecord {
    pub fn from_agent_record(r: &AgentRecord) -> Self {
        Self {
            id: r.id,
            name: r.name.clone(),
            phase: r.phase,
            tokens: r.tokens,
            skills: r.skills.clone(),
        }
    }

    pub fn to_agent_record(&self) -> AgentRecord {
        AgentRecord {
            id: self.id,
            name: self.name.clone(),
            phase: self.phase,
            tokens: self.tokens,
            skills: self.skills.clone(),
        }
    }
}

impl SerializableWorldState {
    /// Build a serializable snapshot from the live world state components.
    pub fn from_world_state(
        tick: u64,
        agents: &[(Uuid, u64, AgentRecord)],
    ) -> Self {
        Self {
            tick,
            agents: agents
                .iter()
                .map(|(id, spawn_tick, record)| SerializableAgentEntry {
                    agent_id: *id,
                    spawn_tick: *spawn_tick,
                    record: SerializableAgentRecord::from_agent_record(record),
                })
                .collect(),
            timestamp: chrono::Utc::now().timestamp(),
        }
    }

    /// Convert back to the live world state components.
    pub fn to_world_state_parts(&self) -> (u64, Vec<(Uuid, u64, AgentRecord)>) {
        let agents = self
            .agents
            .iter()
            .map(|entry| {
                (
                    entry.agent_id,
                    entry.spawn_tick,
                    entry.record.to_agent_record(),
                )
            })
            .collect();
        (self.tick, agents)
    }
}

/// Trait for world state persistence backends.
pub trait StatePersistence: Send + Sync {
    /// Save a complete world state snapshot.
    fn save_snapshot(&self, state: &SerializableWorldState) -> anyhow::Result<()>;

    /// Load the most recent snapshot, if any.
    fn load_latest_snapshot(&self) -> anyhow::Result<Option<SerializableWorldState>>;

    /// Delete snapshots older than the given tick, keeping at most `keep` recent ones.
    fn prune_snapshots(&self, keep: usize) -> anyhow::Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_test_agent(name: &str, tokens: u64) -> (Uuid, u64, AgentRecord) {
        (
            Uuid::new_v4(),
            0,
            AgentRecord {
                id: Uuid::new_v4(),
                name: name.to_string(),
                phase: AgentPhase::Adult,
                tokens,
                skills: HashMap::new(),
            },
        )
    }

    #[test]
    fn serializable_world_state_roundtrip() {
        let agents = vec![
            make_test_agent("Alice", 1000),
            make_test_agent("Bob", 500),
        ];
        let original = SerializableWorldState::from_world_state(42, &agents);

        let json = serde_json::to_string(&original).unwrap();
        let restored: SerializableWorldState = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.tick, 42);
        assert_eq!(restored.agents.len(), 2);
        assert_eq!(restored.agents[0].record.name, "Alice");
        assert_eq!(restored.agents[1].record.tokens, 500);
    }

    #[test]
    fn serializable_world_state_to_world_parts() {
        let agents = vec![
            make_test_agent("Carol", 800),
        ];
        let snapshot = SerializableWorldState::from_world_state(10, &agents);
        let (tick, restored_agents) = snapshot.to_world_state_parts();

        assert_eq!(tick, 10);
        assert_eq!(restored_agents.len(), 1);
        assert_eq!(restored_agents[0].2.name, "Carol");
        assert_eq!(restored_agents[0].2.tokens, 800);
    }

    #[test]
    fn serializable_world_state_empty_agents() {
        let snapshot = SerializableWorldState::from_world_state(0, &[]);
        let json = serde_json::to_string(&snapshot).unwrap();
        let restored: SerializableWorldState = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.tick, 0);
        assert!(restored.agents.is_empty());
    }

    #[test]
    fn serializable_agent_record_with_skills() {
        let mut skills = HashMap::new();
        skills.insert(
            "mining".to_string(),
            SkillRecord {
                name: "mining".to_string(),
                level: 5,
                experience: 250.0,
            },
        );
        let record = AgentRecord {
            id: Uuid::new_v4(),
            name: "SkilledAgent".to_string(),
            phase: AgentPhase::Elder,
            tokens: 9999,
            skills,
        };

        let serializable = SerializableAgentRecord::from_agent_record(&record);
        let json = serde_json::to_string(&serializable).unwrap();
        let restored: SerializableAgentRecord = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.skills.len(), 1);
        assert_eq!(restored.skills["mining"].level, 5);
        assert_eq!(restored.phase, AgentPhase::Elder);
    }
}
