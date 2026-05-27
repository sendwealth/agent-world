//! Snapshot data structures for the world state snapshot system.
//!
//! Defines the complete snapshot model: agent state, world metadata,
//! and incremental diff format for efficient storage.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::economy::token_burn::{AgentRecord, SkillRecord};
use crate::world::enums::AgentPhase;

/// Configuration for the snapshot subsystem.
/// Maps to the `snapshot` section in genesis.yaml.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SnapshotConfig {
    /// Number of ticks between automatic snapshots. 0 = disabled.
    #[serde(default = "default_interval_ticks")]
    pub interval_ticks: u64,
    /// Zstd compression level (1-22, higher = better compression but slower).
    #[serde(default = "default_compression_level")]
    pub compression_level: i32,
    /// Maximum number of snapshots to keep on disk. 0 = unlimited.
    #[serde(default = "default_max_snapshots")]
    pub max_snapshots: usize,
    /// Event types that trigger an extra (non-periodic) snapshot.
    #[serde(default)]
    pub trigger_event_types: Vec<String>,
}

impl Default for SnapshotConfig {
    fn default() -> Self {
        Self {
            interval_ticks: default_interval_ticks(),
            compression_level: default_compression_level(),
            max_snapshots: default_max_snapshots(),
            trigger_event_types: Vec::new(),
        }
    }
}

fn default_interval_ticks() -> u64 { 100 }
fn default_compression_level() -> i32 { 3 }
fn default_max_snapshots() -> usize { 100 }

// ═══════════════════════════════════════════════════════════════════════════
// Full Snapshot
// ═══════════════════════════════════════════════════════════════════════════

/// Complete snapshot of world state at a point in time.
///
/// This is the primary snapshot type — a self-contained representation
/// of the entire world that can be used for recovery or replay.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldSnapshot {
    /// The tick at which this snapshot was taken.
    pub tick: u64,
    /// Wall-clock timestamp (Unix epoch seconds).
    pub timestamp: i64,
    /// All agents at this tick.
    pub agents: Vec<AgentSnapshot>,
    /// Hash of the full snapshot JSON (for integrity verification).
    pub content_hash: String,
}

/// Snapshot of a single agent within a world snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentSnapshot {
    /// Agent UUID.
    pub id: Uuid,
    /// Agent display name.
    pub name: String,
    /// Current lifecycle phase.
    pub phase: AgentPhase,
    /// Current token balance.
    pub tokens: u64,
    /// Spawn tick of this agent.
    pub spawn_tick: u64,
    /// Current skills.
    pub skills: HashMap<String, SkillRecord>,
    /// Personality vector (opaque JSON string).
    #[serde(default)]
    pub personality: String,
    /// Number of tasks this agent has completed successfully.
    #[serde(default)]
    pub tasks_completed: u32,
    /// Number of tasks this agent has attempted (claimed or started).
    #[serde(default)]
    pub tasks_attempted: u32,
}

impl AgentSnapshot {
    /// Create from a live AgentRecord.
    pub fn from_record(id: Uuid, spawn_tick: u64, record: &AgentRecord) -> Self {
        Self {
            id,
            name: record.name.clone(),
            phase: record.phase,
            tokens: record.tokens,
            spawn_tick,
            skills: record.skills.clone(),
            personality: record.personality.clone(),
            tasks_completed: record.tasks_completed,
            tasks_attempted: record.tasks_attempted,
        }
    }

    /// Convert back to an AgentRecord + spawn_tick.
    pub fn to_record(&self) -> (Uuid, u64, AgentRecord) {
        (
            self.id,
            self.spawn_tick,
            AgentRecord {
                id: self.id,
                name: self.name.clone(),
                phase: self.phase,
                tokens: self.tokens,
                skills: self.skills.clone(),
                personality: self.personality.clone(),
                tasks_attempted: self.tasks_attempted,
                tasks_completed: self.tasks_completed,
            },
        )
    }
}

impl WorldSnapshot {
    /// Build a snapshot from the live world state components.
    pub fn from_world_state(
        tick: u64,
        agents: &[(Uuid, u64, AgentRecord)],
    ) -> Self {
        let agent_snapshots: Vec<AgentSnapshot> = agents
            .iter()
            .map(|(id, spawn_tick, record)| {
                AgentSnapshot::from_record(*id, *spawn_tick, record)
            })
            .collect();

        let mut snapshot = Self {
            tick,
            timestamp: chrono::Utc::now().timestamp(),
            agents: agent_snapshots,
            content_hash: String::new(),
        };

        // Compute content hash from the snapshot without the hash field itself
        snapshot.content_hash = snapshot.compute_hash();
        snapshot
    }

    /// Compute a deterministic hash of the snapshot contents.
    fn compute_hash(&self) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        self.tick.hash(&mut hasher);
        for agent in &self.agents {
            agent.id.hash(&mut hasher);
            agent.name.hash(&mut hasher);
            agent.phase.hash(&mut hasher);
            agent.tokens.hash(&mut hasher);
            agent.spawn_tick.hash(&mut hasher);
            agent.tasks_completed.hash(&mut hasher);
            agent.tasks_attempted.hash(&mut hasher);
            // Hash skills in a deterministic order
            let mut skills: Vec<_> = agent.skills.iter().collect();
            skills.sort_by_key(|(k, _)| *k);
            for (name, skill) in skills {
                name.hash(&mut hasher);
                skill.level.hash(&mut hasher);
                skill.experience.to_bits().hash(&mut hasher);
            }
        }
        format!("{:016x}", hasher.finish())
    }

    /// Verify the snapshot integrity.
    pub fn verify_hash(&self) -> bool {
        self.content_hash == self.compute_hash()
    }

    /// Get living agents (non-Dead).
    pub fn living_agents(&self) -> Vec<&AgentSnapshot> {
        self.agents
            .iter()
            .filter(|a| a.phase != AgentPhase::Dead)
            .collect()
    }

    /// Total tokens in circulation (including dead agents).
    pub fn total_tokens(&self) -> u64 {
        self.agents.iter().map(|a| a.tokens).sum()
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Incremental Snapshot (Delta)
// ═══════════════════════════════════════════════════════════════════════════

/// Incremental diff between two snapshots.
///
/// Only stores agents that were added, removed, or changed between
/// the previous and current tick. This drastically reduces storage
/// when the world changes incrementally.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotDelta {
    /// Tick this delta applies to.
    pub tick: u64,
    /// Wall-clock timestamp.
    pub timestamp: i64,
    /// Agents added since the previous snapshot (full data).
    pub agents_added: Vec<AgentSnapshot>,
    /// Agents removed (died or despawned) since the previous snapshot.
    pub agents_removed: Vec<Uuid>,
    /// Agents whose state changed since the previous snapshot (full new data).
    pub agents_changed: Vec<AgentSnapshot>,
    /// Hash of the full reconstructed state at this tick (for verification).
    pub content_hash: String,
}

impl SnapshotDelta {
    /// Compute the delta between a previous full snapshot and the current state.
    pub fn diff(
        prev: &WorldSnapshot,
        current_tick: u64,
        current_agents: &[(Uuid, u64, AgentRecord)],
    ) -> Self {
        let timestamp = chrono::Utc::now().timestamp();

        // Index previous agents by ID
        let prev_map: HashMap<Uuid, &AgentSnapshot> = prev.agents
            .iter()
            .map(|a| (a.id, a))
            .collect();

        // Index current agents by ID
        let curr_ids: std::collections::HashSet<Uuid> = current_agents
            .iter()
            .map(|(id, _, _)| *id)
            .collect();

        let mut agents_added = Vec::new();
        let mut agents_removed = Vec::new();
        let mut agents_changed = Vec::new();

        // Find added and changed agents
        for (id, spawn_tick, record) in current_agents {
            match prev_map.get(id) {
                None => {
                    // New agent
                    agents_added.push(AgentSnapshot::from_record(*id, *spawn_tick, record));
                }
                Some(prev_agent) => {
                    // Check if changed
                    if agent_changed(prev_agent, record) {
                        agents_changed.push(AgentSnapshot::from_record(*id, *spawn_tick, record));
                    }
                }
            }
        }

        // Find removed agents
        for prev_agent in &prev.agents {
            if !curr_ids.contains(&prev_agent.id) {
                agents_removed.push(prev_agent.id);
            }
        }

        // Reconstruct the full state to compute the hash
        let full = Self::reconstruct_full(prev, &agents_added, &agents_removed, &agents_changed);
        let content_hash = full.compute_hash();

        Self {
            tick: current_tick,
            timestamp,
            agents_added,
            agents_removed,
            agents_changed,
            content_hash,
        }
    }

    /// Apply this delta to a previous snapshot to get the full current state.
    pub fn apply(&self, prev: &WorldSnapshot) -> WorldSnapshot {
        Self::reconstruct_full(
            prev,
            &self.agents_added,
            &self.agents_removed,
            &self.agents_changed,
        )
    }

    fn reconstruct_full(
        prev: &WorldSnapshot,
        added: &[AgentSnapshot],
        removed: &[Uuid],
        changed: &[AgentSnapshot],
    ) -> WorldSnapshot {
        let removed_set: std::collections::HashSet<Uuid> = removed.iter().copied().collect();
        let changed_map: HashMap<Uuid, &AgentSnapshot> = changed
            .iter()
            .map(|a| (a.id, a))
            .collect();

        let mut agents: Vec<AgentSnapshot> = prev.agents.iter()
            .filter(|a| !removed_set.contains(&a.id))
            .map(|a| {
                match changed_map.get(&a.id) {
                    Some(updated) => (*updated).clone(),
                    None => a.clone(),
                }
            })
            .chain(added.iter().cloned())
            .collect();

        // Sort by ID for deterministic ordering
        agents.sort_by_key(|a| a.id);

        // Use tick from delta if available, otherwise from prev
        let tick = prev.tick; // Caller sets correct tick
        let mut snapshot = WorldSnapshot {
            tick,
            timestamp: chrono::Utc::now().timestamp(),
            agents,
            content_hash: String::new(),
        };
        snapshot.content_hash = snapshot.compute_hash();
        snapshot
    }

    /// Check if this delta is effectively empty (no changes).
    pub fn is_empty(&self) -> bool {
        self.agents_added.is_empty()
            && self.agents_removed.is_empty()
            && self.agents_changed.is_empty()
    }
}

/// Check if an agent's state has changed from a snapshot.
fn agent_changed(snapshot: &AgentSnapshot, record: &AgentRecord) -> bool {
    snapshot.name != record.name
        || snapshot.phase != record.phase
        || snapshot.tokens != record.tokens
        || snapshot.skills != record.skills
        || snapshot.personality != record.personality
        || snapshot.tasks_completed != record.tasks_completed
        || snapshot.tasks_attempted != record.tasks_attempted
}

// ═══════════════════════════════════════════════════════════════════════════
// Storage Record
// ═══════════════════════════════════════════════════════════════════════════

/// On-disk storage record for a snapshot (full or delta).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotRecord {
    /// Unique record ID.
    pub id: Uuid,
    /// Tick this snapshot represents.
    pub tick: u64,
    /// Whether this is a full snapshot or a delta.
    pub kind: SnapshotKind,
    /// Compressed data (zstd-compressed JSON).
    pub compressed_data: Vec<u8>,
    /// Uncompressed size in bytes.
    pub uncompressed_size: usize,
    /// Compressed size in bytes.
    pub compressed_size: usize,
    /// Wall-clock timestamp.
    pub timestamp: i64,
    /// Content hash of the full state at this tick.
    pub content_hash: String,
}

/// Whether a snapshot record is a full snapshot or an incremental delta.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotKind {
    Full,
    Delta,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_agent(name: &str, tokens: u64, phase: AgentPhase) -> (Uuid, u64, AgentRecord) {
        make_agent_with_tasks(name, tokens, phase, 0, 0)
    }

    fn make_agent_with_tasks(
        name: &str,
        tokens: u64,
        phase: AgentPhase,
        tasks_completed: u32,
        tasks_attempted: u32,
    ) -> (Uuid, u64, AgentRecord) {
        (
            Uuid::new_v4(),
            0,
            AgentRecord {
                id: Uuid::new_v4(),
                name: name.to_string(),
                phase,
                tokens,
                skills: HashMap::new(),
                personality: String::new(),
                tasks_completed,
                tasks_attempted,
            },
        )
    }

    #[test]
    fn snapshot_from_world_state() {
        let agents = vec![
            make_agent("Alice", 1000, AgentPhase::Adult),
            make_agent("Bob", 500, AgentPhase::Childhood),
        ];
        let snapshot = WorldSnapshot::from_world_state(42, &agents);
        assert_eq!(snapshot.tick, 42);
        assert_eq!(snapshot.agents.len(), 2);
        assert!(!snapshot.content_hash.is_empty());
    }

    #[test]
    fn snapshot_verify_hash() {
        let agents = vec![make_agent("Alice", 1000, AgentPhase::Adult)];
        let snapshot = WorldSnapshot::from_world_state(1, &agents);
        assert!(snapshot.verify_hash());

        // Tamper with the snapshot
        let mut tampered = snapshot.clone();
        tampered.tick = 999;
        assert!(!tampered.verify_hash());
    }

    #[test]
    fn snapshot_living_agents() {
        let agents = vec![
            make_agent("Alice", 1000, AgentPhase::Adult),
            make_agent("DeadBob", 0, AgentPhase::Dead),
        ];
        let snapshot = WorldSnapshot::from_world_state(1, &agents);
        assert_eq!(snapshot.living_agents().len(), 1);
    }

    #[test]
    fn snapshot_total_tokens() {
        let agents = vec![
            make_agent("Alice", 1000, AgentPhase::Adult),
            make_agent("Bob", 500, AgentPhase::Adult),
        ];
        let snapshot = WorldSnapshot::from_world_state(1, &agents);
        assert_eq!(snapshot.total_tokens(), 1500);
    }

    #[test]
    fn agent_snapshot_roundtrip() {
        let (id, spawn_tick, record) = make_agent("Alice", 1000, AgentPhase::Adult);
        let snapshot = AgentSnapshot::from_record(id, spawn_tick, &record);
        let (rid, rspawn, rrecord) = snapshot.to_record();
        assert_eq!(rid, id);
        assert_eq!(rspawn, spawn_tick);
        assert_eq!(rrecord.name, "Alice");
        assert_eq!(rrecord.tokens, 1000);
        assert_eq!(rrecord.tasks_completed, 0);
        assert_eq!(rrecord.tasks_attempted, 0);
    }

    #[test]
    fn agent_snapshot_roundtrip_preserves_tasks() {
        let (id, spawn_tick, record) =
            make_agent_with_tasks("Bob", 500, AgentPhase::Adult, 7, 12);
        let snapshot = AgentSnapshot::from_record(id, spawn_tick, &record);
        let (rid, rspawn, rrecord) = snapshot.to_record();
        assert_eq!(rid, id);
        assert_eq!(rspawn, spawn_tick);
        assert_eq!(rrecord.tasks_completed, 7);
        assert_eq!(rrecord.tasks_attempted, 12);
    }

    #[test]
    fn delta_detect_tasks_change() {
        let (id1, _, rec1) =
            make_agent_with_tasks("Alice", 1000, AgentPhase::Adult, 5, 10);
        let agents_t1 = vec![(id1, 0u64, rec1.clone())];
        let snapshot_t1 = WorldSnapshot::from_world_state(1, &agents_t1);

        // Agent completes more tasks
        let rec2 = AgentRecord {
            tasks_completed: 6,
            tasks_attempted: 11,
            ..rec1.clone()
        };
        let agents_t2 = vec![(id1, 0u64, rec2)];

        let delta = SnapshotDelta::diff(&snapshot_t1, 2, &agents_t2);
        assert_eq!(delta.agents_changed.len(), 1);
        assert_eq!(delta.agents_changed[0].tasks_completed, 6);
        assert_eq!(delta.agents_changed[0].tasks_attempted, 11);
    }

    #[test]
    fn delta_diff_no_changes() {
        let agents = vec![make_agent("Alice", 1000, AgentPhase::Adult)];
        let snapshot = WorldSnapshot::from_world_state(1, &agents);

        let delta = SnapshotDelta::diff(&snapshot, 2, &agents);
        assert!(delta.is_empty());
    }

    #[test]
    fn delta_diff_with_changes() {
        let (id1, _, mut rec1) = make_agent("Alice", 1000, AgentPhase::Adult);
        let agents_t1 = vec![(id1, 0u64, rec1.clone())];
        let snapshot_t1 = WorldSnapshot::from_world_state(1, &agents_t1);

        // Change Alice's tokens
        rec1.tokens = 900;
        let agents_t2 = vec![(id1, 0u64, rec1.clone())];

        let delta = SnapshotDelta::diff(&snapshot_t1, 2, &agents_t2);
        assert!(delta.agents_added.is_empty());
        assert!(delta.agents_removed.is_empty());
        assert_eq!(delta.agents_changed.len(), 1);
        assert_eq!(delta.agents_changed[0].tokens, 900);
    }

    #[test]
    fn delta_diff_with_addition() {
        let agent1 = make_agent("Alice", 1000, AgentPhase::Adult);
        let snapshot_t1 = WorldSnapshot::from_world_state(1, std::slice::from_ref(&agent1));

        let agent2 = make_agent("Bob", 500, AgentPhase::Childhood);
        let agents_t2 = vec![agent1, agent2];

        let delta = SnapshotDelta::diff(&snapshot_t1, 2, &agents_t2);
        assert_eq!(delta.agents_added.len(), 1);
        assert_eq!(delta.agents_added[0].name, "Bob");
    }

    #[test]
    fn delta_diff_with_removal() {
        let agent1 = make_agent("Alice", 1000, AgentPhase::Adult);
        let agent2 = make_agent("Bob", 500, AgentPhase::Adult);
        let agents_t1 = vec![agent1.clone(), agent2.clone()];
        let snapshot_t1 = WorldSnapshot::from_world_state(1, &agents_t1);

        // Only Alice remains
        let agents_t2 = vec![agent1];

        let delta = SnapshotDelta::diff(&snapshot_t1, 2, &agents_t2);
        assert_eq!(delta.agents_removed.len(), 1);
    }

    #[test]
    fn delta_apply_roundtrip() {
        let agent1 = make_agent("Alice", 1000, AgentPhase::Adult);
        let agent2 = make_agent("Bob", 500, AgentPhase::Adult);
        let agents_t1 = vec![agent1.clone(), agent2.clone()];
        let snapshot_t1 = WorldSnapshot::from_world_state(1, &agents_t1);

        // Change Alice's tokens, remove Bob, add Carol
        let mut rec1_changed = agent1.2.clone();
        rec1_changed.tokens = 800;
        let agent3 = make_agent("Carol", 300, AgentPhase::Childhood);
        let agents_t2 = vec![(agent1.0, 0u64, rec1_changed), agent3];

        let delta = SnapshotDelta::diff(&snapshot_t1, 2, &agents_t2);
        let reconstructed = delta.apply(&snapshot_t1);

        assert_eq!(reconstructed.tick, 1); // base tick from prev
        assert_eq!(reconstructed.agents.len(), 2);

        // Find Alice in reconstructed
        let alice = reconstructed.agents.iter().find(|a| a.name == "Alice").unwrap();
        assert_eq!(alice.tokens, 800);

        // Carol should be there
        let carol = reconstructed.agents.iter().find(|a| a.name == "Carol").unwrap();
        assert_eq!(carol.tokens, 300);

        // Bob should be gone
        let bob = reconstructed.agents.iter().find(|a| a.name == "Bob");
        assert!(bob.is_none());
    }

    #[test]
    fn snapshot_config_default() {
        let config = SnapshotConfig::default();
        assert_eq!(config.interval_ticks, 100);
        assert_eq!(config.compression_level, 3);
        assert_eq!(config.max_snapshots, 100);
    }
}
