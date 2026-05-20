//! Time Capsule — periodic world snapshots for analysis and replay.
//!
//! Generates a world snapshot every `N` ticks containing:
//! - Total population / active agent count
//! - GDP (total tokens in circulation)
//! - Gini coefficient of token distribution
//! - Skill distribution TOP 5
//! - Key event timeline (deaths, large transactions, alliance formation)
//!
//! Snapshots are stored in SQLite for efficient querying and export.

use std::collections::HashMap;
use std::sync::Arc;

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::economy::token_burn::AgentRecord;
use crate::world::enums::AgentPhase;
use crate::world::event::WorldEvent;

// ═══════════════════════════════════════════════════════════════════════════
// Snapshot Data Types
// ═══════════════════════════════════════════════════════════════════════════

/// A single world snapshot at a point in time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldSnapshotData {
    /// Tick at which this snapshot was taken.
    pub tick: u64,
    /// Timestamp (epoch seconds) when snapshot was taken.
    pub timestamp: i64,
    /// Total number of agents ever spawned.
    pub total_population: u64,
    /// Number of living agents.
    pub active_agents: u64,
    /// Sum of all living agents' tokens (GDP proxy).
    pub gdp: u64,
    /// Gini coefficient of token distribution (0.0 = perfect equality, 1.0 = perfect inequality).
    pub gini_coefficient: f64,
    /// Top 5 skills by number of agents possessing them.
    pub skill_distribution_top5: Vec<SkillCount>,
    /// Key events since the last snapshot.
    pub key_events: Vec<KeyEvent>,
}

/// A skill and how many agents have it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillCount {
    pub skill_name: String,
    pub agent_count: u64,
    pub avg_level: f64,
}

/// A key event for the timeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyEvent {
    pub tick: u64,
    pub event_type: String,
    pub agent_id: Option<String>,
    pub description: String,
}

// ═══════════════════════════════════════════════════════════════════════════
// Gini Coefficient
// ═══════════════════════════════════════════════════════════════════════════

/// Calculate the Gini coefficient of a distribution of values.
///
/// Returns 0.0 for empty or single-element distributions (perfect equality).
pub fn calculate_gini(values: &[u64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    if values.len() == 1 {
        return 0.0;
    }

    let mut sorted: Vec<u64> = values.to_vec();
    sorted.sort();

    let n = sorted.len() as f64;
    let sum: u64 = sorted.iter().sum();
    if sum == 0 {
        return 0.0;
    }

    let mean = sum as f64 / n;
    let mut numerator = 0.0;
    for &val in sorted.iter() {
        for &val2 in sorted.iter() {
            numerator += (val as f64 - val2 as f64).abs();
        }
    }

    numerator / (2.0 * n * n * mean)
}

// ═══════════════════════════════════════════════════════════════════════════
// Snapshot Store (SQLite)
// ═══════════════════════════════════════════════════════════════════════════

/// SQLite-backed store for world snapshots.
pub struct SnapshotStore {
    conn: Connection,
}

impl SnapshotStore {
    /// Create a new snapshot store, initializing the database.
    pub fn new(path: &str) -> Result<Self, rusqlite::Error> {
        let conn = Connection::open(path)?;
        let store = Self { conn };
        store.init_schema()?;
        Ok(store)
    }

    /// Create an in-memory store (for testing).
    pub fn new_in_memory() -> Result<Self, rusqlite::Error> {
        let conn = Connection::open_in_memory()?;
        let store = Self { conn };
        store.init_schema()?;
        Ok(store)
    }

    fn init_schema(&self) -> Result<(), rusqlite::Error> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS snapshots (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                tick        INTEGER NOT NULL,
                timestamp   INTEGER NOT NULL,
                total_population INTEGER NOT NULL,
                active_agents    INTEGER NOT NULL,
                gdp              INTEGER NOT NULL,
                gini_coefficient REAL NOT NULL,
                snapshot_data    TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_snapshots_tick ON snapshots(tick);
            ",
        )?;
        Ok(())
    }

    /// Save a snapshot to the database.
    pub fn save(&self, snapshot: &WorldSnapshotData) -> Result<i64, rusqlite::Error> {
        let json = serde_json::to_string(snapshot).unwrap();
        self.conn.execute(
            "INSERT INTO snapshots (tick, timestamp, total_population, active_agents, gdp, gini_coefficient, snapshot_data)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                snapshot.tick,
                snapshot.timestamp,
                snapshot.total_population,
                snapshot.active_agents,
                snapshot.gdp,
                snapshot.gini_coefficient,
                json,
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Get a snapshot by tick number.
    pub fn get_by_tick(&self, tick: u64) -> Result<Option<WorldSnapshotData>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT snapshot_data FROM snapshots WHERE tick = ?1 ORDER BY id DESC LIMIT 1",
        )?;
        let mut rows = stmt.query(params![tick])?;
        match rows.next()? {
            Some(row) => {
                let json: String = row.get(0)?;
                let snapshot: WorldSnapshotData = serde_json::from_str(&json).unwrap();
                Ok(Some(snapshot))
            }
            None => Ok(None),
        }
    }

    /// List snapshots in a tick range, ordered by tick.
    pub fn list(&self, from_tick: Option<u64>, to_tick: Option<u64>, limit: Option<u64>) -> Result<Vec<WorldSnapshotData>, rusqlite::Error> {
        let mut sql = "SELECT snapshot_data FROM snapshots WHERE 1=1".to_string();
        let mut param_idx = 1;

        if from_tick.is_some() {
            sql.push_str(&format!(" AND tick >= ?{}", param_idx));
            param_idx += 1;
        }
        if to_tick.is_some() {
            sql.push_str(&format!(" AND tick <= ?{}", param_idx));
            param_idx += 1;
        }
        sql.push_str(" ORDER BY tick ASC");
        if limit.is_some() {
            sql.push_str(&format!(" LIMIT ?{}", param_idx));
        }

        let mut stmt = self.conn.prepare(&sql)?;

        // Build dynamic parameter list
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        if let Some(from) = from_tick {
            param_values.push(Box::new(from));
        }
        if let Some(to) = to_tick {
            param_values.push(Box::new(to));
        }
        if let Some(lim) = limit {
            param_values.push(Box::new(lim));
        }

        let param_refs: Vec<&dyn rusqlite::types::ToSql> = param_values.iter().map(|p| p.as_ref()).collect();
        let mut rows = stmt.query(param_refs.as_slice())?;

        let mut results = Vec::new();
        while let Some(row) = rows.next()? {
            let json: String = row.get(0)?;
            let snapshot: WorldSnapshotData = serde_json::from_str(&json).unwrap();
            results.push(snapshot);
        }
        Ok(results)
    }

    /// Get the latest snapshot.
    pub fn latest(&self) -> Result<Option<WorldSnapshotData>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT snapshot_data FROM snapshots ORDER BY tick DESC LIMIT 1",
        )?;
        let mut rows = stmt.query([])?;
        match rows.next()? {
            Some(row) => {
                let json: String = row.get(0)?;
                let snapshot: WorldSnapshotData = serde_json::from_str(&json).unwrap();
                Ok(Some(snapshot))
            }
            None => Ok(None),
        }
    }

    /// Get count of stored snapshots.
    pub fn count(&self) -> Result<u64, rusqlite::Error> {
        let count: u64 = self.conn.query_row(
            "SELECT COUNT(*) FROM snapshots",
            [],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    /// Export all snapshots as JSON string.
    pub fn export_json(&self) -> Result<String, rusqlite::Error> {
        let snapshots = self.list(None, None, None)?;
        Ok(serde_json::to_string_pretty(&snapshots).unwrap())
    }

    /// Export all snapshots as CSV string.
    pub fn export_csv(&self) -> Result<String, rusqlite::Error> {
        let snapshots = self.list(None, None, None)?;
        let mut csv = String::from("tick,timestamp,total_population,active_agents,gdp,gini_coefficient,skill_distribution_top5,key_events_count\n");
        for snap in &snapshots {
            let skills_json = serde_json::to_string(&snap.skill_distribution_top5).unwrap();
            csv.push_str(&format!(
                "{},{},{},{},{},{:.4},{},{}\n",
                snap.tick,
                snap.timestamp,
                snap.total_population,
                snap.active_agents,
                snap.gdp,
                snap.gini_coefficient,
                skills_json,
                snap.key_events.len(),
            ));
        }
        Ok(csv)
    }
}

/// Thread-safe shared snapshot store.
pub type SharedSnapshotStore = Arc<Mutex<SnapshotStore>>;

// ═══════════════════════════════════════════════════════════════════════════
// Snapshot Builder
// ═══════════════════════════════════════════════════════════════════════════

/// Build a snapshot from the current agent state and event buffer.
pub fn build_snapshot(
    tick: u64,
    agents: &[(Uuid, u64, AgentRecord)],
    key_events: &[KeyEvent],
) -> WorldSnapshotData {
    let total_population = agents.len() as u64;
    let active_agents = agents.iter()
        .filter(|(_, _, a)| a.phase != AgentPhase::Dead)
        .count() as u64;

    let token_values: Vec<u64> = agents.iter()
        .filter(|(_, _, a)| a.phase != AgentPhase::Dead)
        .map(|(_, _, a)| a.tokens)
        .collect();
    let gdp = token_values.iter().sum();

    let gini_coefficient = calculate_gini(&token_values);

    // Calculate skill distribution
    let mut skill_map: HashMap<String, (u64, f64)> = HashMap::new();
    for (_, _, agent) in agents.iter() {
        if agent.phase == AgentPhase::Dead {
            continue;
        }
        for (name, skill) in &agent.skills {
            let entry = skill_map.entry(name.clone()).or_insert((0, 0.0));
            entry.0 += 1;
            entry.1 += skill.level as f64;
        }
    }

    let mut skill_counts: Vec<SkillCount> = skill_map.into_iter()
        .map(|(name, (count, total_level))| SkillCount {
            skill_name: name,
            agent_count: count,
            avg_level: if count > 0 { total_level / count as f64 } else { 0.0 },
        })
        .collect();
    skill_counts.sort_by_key(|b| std::cmp::Reverse(b.agent_count));
    let skill_distribution_top5 = skill_counts.into_iter().take(5).collect();

    let timestamp = chrono::Utc::now().timestamp();

    WorldSnapshotData {
        tick,
        timestamp,
        total_population,
        active_agents,
        gdp,
        gini_coefficient,
        skill_distribution_top5,
        key_events: key_events.to_vec(),
    }
}

/// Extract key events from a list of world events.
pub fn extract_key_events(events: &[WorldEvent]) -> Vec<KeyEvent> {
    let mut key_events = Vec::new();

    for event in events {
        match event {
            WorldEvent::AgentDied { agent_id, reason } => {
                key_events.push(KeyEvent {
                    tick: 0, // Will be filled in by caller
                    event_type: "agent_died".to_string(),
                    agent_id: Some(agent_id.clone()),
                    description: format!("Agent died: {:?}", reason),
                });
            }
            WorldEvent::TransactionCompleted { from, to, amount, .. } if *amount >= 100 => {
                key_events.push(KeyEvent {
                    tick: 0,
                    event_type: "large_transaction".to_string(),
                    agent_id: Some(from.clone()),
                    description: format!("Large transaction: {} from {} to {}", amount, from, to),
                });
            }
            WorldEvent::AgentSpawned { agent_id, name } => {
                key_events.push(KeyEvent {
                    tick: 0,
                    event_type: "agent_spawned".to_string(),
                    agent_id: Some(agent_id.clone()),
                    description: format!("New agent spawned: {}", name),
                });
            }
            WorldEvent::PhaseChanged { agent_id, old_phase, new_phase } => {
                key_events.push(KeyEvent {
                    tick: 0,
                    event_type: "phase_changed".to_string(),
                    agent_id: Some(agent_id.clone()),
                    description: format!("Phase: {:?} -> {:?}", old_phase, new_phase),
                });
            }
            WorldEvent::TaskCompleted { task_id } => {
                key_events.push(KeyEvent {
                    tick: 0,
                    event_type: "task_completed".to_string(),
                    agent_id: None,
                    description: format!("Task completed: {}", task_id),
                });
            }
            _ => {}
        }
    }

    key_events
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::economy::token_burn::SkillRecord;
    use std::collections::HashMap;

    fn make_agent(phase: AgentPhase, tokens: u64) -> (Uuid, u64, AgentRecord) {
        (
            Uuid::new_v4(),
            0,
            AgentRecord {
                id: Uuid::new_v4(),
                name: "test".to_string(),
                phase,
                tokens,
                skills: HashMap::new(),
                personality: String::new(),
            },
        )
    }

    fn make_agent_with_skills(
        phase: AgentPhase,
        tokens: u64,
        skills: Vec<(&str, u32)>,
    ) -> (Uuid, u64, AgentRecord) {
        (
            Uuid::new_v4(),
            0,
            AgentRecord {
                id: Uuid::new_v4(),
                name: "test".to_string(),
                phase,
                tokens,
                skills: skills.into_iter().map(|(n, l)| {
                    (n.to_string(), SkillRecord {
                        name: n.to_string(),
                        level: l,
                        experience: 0.0,
                    })
                }).collect(),
                personality: String::new(),
            },
        )
    }

    #[test]
    fn test_gini_perfect_equality() {
        let values = vec![100, 100, 100, 100];
        let gini = calculate_gini(&values);
        assert!((gini - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_gini_perfect_inequality() {
        let values = vec![0, 0, 0, 100];
        let gini = calculate_gini(&values);
        assert!(gini > 0.5, "gini should be high for unequal distribution, got {}", gini);
    }

    #[test]
    fn test_gini_empty() {
        assert_eq!(calculate_gini(&[]), 0.0);
        assert_eq!(calculate_gini(&[42]), 0.0);
    }

    #[test]
    fn test_snapshot_store_save_and_retrieve() {
        let store = SnapshotStore::new_in_memory().unwrap();
        let snapshot = WorldSnapshotData {
            tick: 1000,
            timestamp: 1700000000,
            total_population: 10,
            active_agents: 8,
            gdp: 5000,
            gini_coefficient: 0.35,
            skill_distribution_top5: vec![SkillCount {
                skill_name: "mining".to_string(),
                agent_count: 5,
                avg_level: 3.2,
            }],
            key_events: vec![KeyEvent {
                tick: 950,
                event_type: "agent_died".to_string(),
                agent_id: Some("agent-1".to_string()),
                description: "Agent died: TokenDepleted".to_string(),
            }],
        };

        store.save(&snapshot).unwrap();

        let retrieved = store.get_by_tick(1000).unwrap().unwrap();
        assert_eq!(retrieved.tick, 1000);
        assert_eq!(retrieved.total_population, 10);
        assert_eq!(retrieved.active_agents, 8);
        assert_eq!(retrieved.gdp, 5000);
        assert!((retrieved.gini_coefficient - 0.35).abs() < 0.001);
        assert_eq!(retrieved.skill_distribution_top5.len(), 1);
        assert_eq!(retrieved.key_events.len(), 1);
    }

    #[test]
    fn test_snapshot_store_list_with_range() {
        let store = SnapshotStore::new_in_memory().unwrap();
        for tick in [1000, 2000, 3000, 4000] {
            let snap = WorldSnapshotData {
                tick,
                timestamp: 1700000000 + tick as i64,
                total_population: 10,
                active_agents: 8,
                gdp: 5000,
                gini_coefficient: 0.3,
                skill_distribution_top5: vec![],
                key_events: vec![],
            };
            store.save(&snap).unwrap();
        }

        let all = store.list(None, None, None).unwrap();
        assert_eq!(all.len(), 4);

        let range = store.list(Some(1500), Some(3500), None).unwrap();
        assert_eq!(range.len(), 2);
        assert_eq!(range[0].tick, 2000);
        assert_eq!(range[1].tick, 3000);

        let limited = store.list(None, None, Some(2)).unwrap();
        assert_eq!(limited.len(), 2);
    }

    #[test]
    fn test_snapshot_store_latest() {
        let store = SnapshotStore::new_in_memory().unwrap();
        assert!(store.latest().unwrap().is_none());

        for tick in [1000, 2000, 3000] {
            let snap = WorldSnapshotData {
                tick,
                timestamp: 1700000000,
                total_population: 10,
                active_agents: 8,
                gdp: 5000,
                gini_coefficient: 0.3,
                skill_distribution_top5: vec![],
                key_events: vec![],
            };
            store.save(&snap).unwrap();
        }

        let latest = store.latest().unwrap().unwrap();
        assert_eq!(latest.tick, 3000);
    }

    #[test]
    fn test_snapshot_store_export_json() {
        let store = SnapshotStore::new_in_memory().unwrap();
        let snap = WorldSnapshotData {
            tick: 1000,
            timestamp: 1700000000,
            total_population: 5,
            active_agents: 4,
            gdp: 2000,
            gini_coefficient: 0.2,
            skill_distribution_top5: vec![],
            key_events: vec![],
        };
        store.save(&snap).unwrap();

        let json = store.export_json().unwrap();
        assert!(json.contains("\"tick\": 1000"));
        assert!(json.contains("\"gdp\": 2000"));
    }

    #[test]
    fn test_snapshot_store_export_csv() {
        let store = SnapshotStore::new_in_memory().unwrap();
        let snap = WorldSnapshotData {
            tick: 1000,
            timestamp: 1700000000,
            total_population: 5,
            active_agents: 4,
            gdp: 2000,
            gini_coefficient: 0.2,
            skill_distribution_top5: vec![],
            key_events: vec![],
        };
        store.save(&snap).unwrap();

        let csv = store.export_csv().unwrap();
        assert!(csv.starts_with("tick,timestamp"));
        assert!(csv.contains("1000,"));
    }

    #[test]
    fn test_build_snapshot() {
        let agents = vec![
            make_agent(AgentPhase::Adult, 500),
            make_agent(AgentPhase::Adult, 300),
            make_agent(AgentPhase::Dead, 0),
        ];

        let snapshot = build_snapshot(1000, &agents, &[]);
        assert_eq!(snapshot.total_population, 3);
        assert_eq!(snapshot.active_agents, 2);
        assert_eq!(snapshot.gdp, 800);
    }

    #[test]
    fn test_build_snapshot_with_skills() {
        let agents = vec![
            make_agent_with_skills(AgentPhase::Adult, 500, vec![("mining", 3), ("trading", 2)]),
            make_agent_with_skills(AgentPhase::Adult, 300, vec![("mining", 5)]),
        ];

        let snapshot = build_snapshot(1000, &agents, &[]);
        assert_eq!(snapshot.skill_distribution_top5.len(), 2);
        assert_eq!(snapshot.skill_distribution_top5[0].skill_name, "mining");
        assert_eq!(snapshot.skill_distribution_top5[0].agent_count, 2);
    }

    #[test]
    fn test_extract_key_events() {
        use crate::world::enums::{Currency, DeathReason};

        let events = vec![
            WorldEvent::AgentDied {
                agent_id: "a1".to_string(),
                reason: DeathReason::TokenDepleted,
            },
            WorldEvent::TransactionCompleted {
                from: "a1".to_string(),
                to: "a2".to_string(),
                amount: 500,
                currency: Currency::Token,
            },
            WorldEvent::TransactionCompleted {
                from: "a1".to_string(),
                to: "a2".to_string(),
                amount: 10,
                currency: Currency::Token,
            },
            WorldEvent::TickAdvanced { tick: 1 },
        ];

        let key_events = extract_key_events(&events);
        assert_eq!(key_events.len(), 2); // death + large transaction (10 amount is filtered out)
        assert_eq!(key_events[0].event_type, "agent_died");
        assert_eq!(key_events[1].event_type, "large_transaction");
    }

    #[test]
    fn test_snapshot_store_count() {
        let store = SnapshotStore::new_in_memory().unwrap();
        assert_eq!(store.count().unwrap(), 0);

        for tick in [1000, 2000] {
            let snap = WorldSnapshotData {
                tick,
                timestamp: 1700000000,
                total_population: 10,
                active_agents: 8,
                gdp: 5000,
                gini_coefficient: 0.3,
                skill_distribution_top5: vec![],
                key_events: vec![],
            };
            store.save(&snap).unwrap();
        }

        assert_eq!(store.count().unwrap(), 2);
    }
}
