//! SQLite-backed state persistence.

use std::path::Path;
use std::sync::Mutex;

use rusqlite::{params, Connection};

use super::{SerializableAgentRecord, SerializableWorldState, StatePersistence};

const SCHEMA_SQL: &str = include_str!("schema.sql");

/// SQLite-based world state persistence.
///
/// The inner `Connection` is wrapped in a `Mutex` so that `SqlitePersistence`
/// is `Send + Sync` and can be shared across threads via `Arc`.
pub struct SqlitePersistence {
    conn: Mutex<Connection>,
}

impl SqlitePersistence {
    /// Open (or create) the persistence database at `path`.
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        conn.execute_batch(SCHEMA_SQL)?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Open an in-memory database (for testing).
    pub fn open_in_memory() -> anyhow::Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch(SCHEMA_SQL)?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }
}

impl StatePersistence for SqlitePersistence {
    fn save_snapshot(&self, state: &SerializableWorldState) -> anyhow::Result<()> {
        let conn = self.conn.lock().map_err(|e| anyhow::anyhow!("lock poisoned: {}", e))?;
        let tx = conn.unchecked_transaction()?;

        // Insert or update snapshot row, returning the id
        let snapshot_id: i64 = tx.query_row(
            "INSERT INTO snapshots (tick, agent_count) VALUES (?1, ?2) \
             ON CONFLICT(tick) DO UPDATE SET agent_count = excluded.agent_count \
             RETURNING id",
            params![state.tick as i64, state.agents.len() as i64],
            |row| row.get(0),
        )?;

        // Delete old agent data for this tick (handles re-save)
        tx.execute(
            "DELETE FROM agents WHERE snapshot_id = ?1",
            params![snapshot_id],
        )?;

        // Insert agents
        for entry in &state.agents {
            let skills_json = serde_json::to_string(&entry.record.skills)?;
            tx.execute(
                "INSERT INTO agents (id, name, phase, tokens, spawn_tick, skills_json, snapshot_id) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    entry.agent_id.to_string(),
                    entry.record.name,
                    serde_json::to_string(&entry.record.phase)?,
                    entry.record.tokens as i64,
                    entry.spawn_tick as i64,
                    skills_json,
                    snapshot_id,
                ],
            )?;
        }

        // Insert economy ledger entry
        let total_tokens: i64 = state.agents.iter().map(|a| a.record.tokens as i64).sum();
        let living = state
            .agents
            .iter()
            .filter(|a| a.record.phase != crate::world::enums::AgentPhase::Dead)
            .count() as i64;
        tx.execute(
            "INSERT INTO economy_ledger (snapshot_id, total_tokens, total_agents, living_agents) \
             VALUES (?1, ?2, ?3, ?4)",
            params![
                snapshot_id,
                total_tokens,
                state.agents.len() as i64,
                living,
            ],
        )?;

        tx.commit()?;
        Ok(())
    }

    fn load_latest_snapshot(&self) -> anyhow::Result<Option<SerializableWorldState>> {
        let conn = self.conn.lock().map_err(|e| anyhow::anyhow!("lock poisoned: {}", e))?;

        let mut stmt = conn.prepare(
            "SELECT id, tick FROM snapshots ORDER BY tick DESC LIMIT 1",
        )?;

        let result = stmt.query_row([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
        });

        let (snapshot_id, tick) = match result {
            Ok(r) => r,
            Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
            Err(e) => return Err(e.into()),
        };

        // Load agents for this snapshot
        let mut agents = Vec::new();
        let mut agent_stmt = conn.prepare(
            "SELECT id, name, phase, tokens, spawn_tick, skills_json \
             FROM agents WHERE snapshot_id = ?1",
        )?;
        let mut agent_rows = agent_stmt.query(params![snapshot_id])?;

        while let Some(row) = agent_rows.next()? {
            let id_str: String = row.get(0)?;
            let name: String = row.get(1)?;
            let phase_str: String = row.get(2)?;
            let tokens: i64 = row.get(3)?;
            let spawn_tick: i64 = row.get(4)?;
            let skills_json: String = row.get(5)?;

            let agent_id = id_str.parse::<uuid::Uuid>()?;
            let phase: crate::world::enums::AgentPhase = serde_json::from_str(&phase_str)?;
            let skills: std::collections::HashMap<String, crate::economy::token_burn::SkillRecord> =
                serde_json::from_str(&skills_json)?;

            agents.push(super::SerializableAgentEntry {
                agent_id,
                spawn_tick: spawn_tick as u64,
                record: SerializableAgentRecord {
                    id: agent_id,
                    name,
                    phase,
                    tokens: tokens as u64,
                    skills,
                },
            });
        }

        Ok(Some(SerializableWorldState {
            tick: tick as u64,
            agents,
            timestamp: chrono::Utc::now().timestamp(),
        }))
    }

    fn prune_snapshots(&self, keep: usize) -> anyhow::Result<()> {
        let conn = self.conn.lock().map_err(|e| anyhow::anyhow!("lock poisoned: {}", e))?;
        conn.execute(
            "DELETE FROM snapshots WHERE id NOT IN \
             (SELECT id FROM snapshots ORDER BY tick DESC LIMIT ?1)",
            params![keep as i64],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::economy::token_burn::AgentRecord;
    use crate::persistence::{SerializableAgentEntry, SerializableAgentRecord, StatePersistence};
    use crate::world::enums::AgentPhase;
    use std::collections::HashMap;
    use uuid::Uuid;

    fn make_agent(name: &str, tokens: u64, phase: AgentPhase) -> (Uuid, u64, AgentRecord) {
        let id = Uuid::new_v4();
        (
            id,
            0,
            AgentRecord {
                id,
                name: name.to_string(),
                phase,
                tokens,
                skills: HashMap::new(),
            },
        )
    }

    #[test]
    fn sqlite_save_and_load_roundtrip() {
        let db = SqlitePersistence::open_in_memory().unwrap();
        let agents = vec![
            make_agent("Alice", 1000, AgentPhase::Adult),
            make_agent("Bob", 500, AgentPhase::Childhood),
        ];
        let state = SerializableWorldState::from_world_state(100, &agents);

        db.save_snapshot(&state).unwrap();

        let loaded = db.load_latest_snapshot().unwrap().unwrap();
        assert_eq!(loaded.tick, 100);
        assert_eq!(loaded.agents.len(), 2);

        let names: Vec<&str> = loaded.agents.iter().map(|a| a.record.name.as_str()).collect();
        assert!(names.contains(&"Alice"));
        assert!(names.contains(&"Bob"));
    }

    #[test]
    fn sqlite_load_empty_returns_none() {
        let db = SqlitePersistence::open_in_memory().unwrap();
        let result = db.load_latest_snapshot().unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn sqlite_latest_snapshot_wins() {
        let db = SqlitePersistence::open_in_memory().unwrap();

        // Save snapshot at tick 50
        let s1 = SerializableWorldState::from_world_state(50, &[]);
        db.save_snapshot(&s1).unwrap();

        // Save snapshot at tick 200
        let s2 = SerializableWorldState::from_world_state(200, &[]);
        db.save_snapshot(&s2).unwrap();

        let loaded = db.load_latest_snapshot().unwrap().unwrap();
        assert_eq!(loaded.tick, 200);
    }

    #[test]
    fn sqlite_prune_old_snapshots() {
        let db = SqlitePersistence::open_in_memory().unwrap();

        for tick in [10, 20, 30, 40, 50] {
            let s = SerializableWorldState::from_world_state(tick, &[]);
            db.save_snapshot(&s).unwrap();
        }

        db.prune_snapshots(2).unwrap();

        let loaded = db.load_latest_snapshot().unwrap().unwrap();
        assert_eq!(loaded.tick, 50);

        // Verify we only kept 2 (the latest should be tick 50 and 40)
        let conn = db.conn.lock().unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM snapshots", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn sqlite_agent_with_skills_roundtrip() {
        let db = SqlitePersistence::open_in_memory().unwrap();
        let mut skills = HashMap::new();
        skills.insert(
            "mining".to_string(),
            crate::economy::token_burn::SkillRecord {
                name: "mining".to_string(),
                level: 7,
                experience: 450.0,
            },
        );
        let id = Uuid::new_v4();
        let agents = vec![(
            id,
            5u64,
            AgentRecord {
                id,
                name: "Miner".to_string(),
                phase: AgentPhase::Adult,
                tokens: 3000,
                skills,
            },
        )];
        let state = SerializableWorldState::from_world_state(99, &agents);
        db.save_snapshot(&state).unwrap();

        let loaded = db.load_latest_snapshot().unwrap().unwrap();
        assert_eq!(loaded.agents.len(), 1);
        let agent = &loaded.agents[0];
        assert_eq!(agent.record.name, "Miner");
        assert_eq!(agent.spawn_tick, 5);
        assert_eq!(agent.record.skills.len(), 1);
        assert_eq!(agent.record.skills["mining"].level, 7);
    }

    #[test]
    fn sqlite_overwrite_same_tick() {
        let db = SqlitePersistence::open_in_memory().unwrap();

        let a1 = make_agent("V1", 100, AgentPhase::Adult);
        let s1 = SerializableWorldState::from_world_state(50, &[a1]);
        db.save_snapshot(&s1).unwrap();

        let a2 = make_agent("V2", 200, AgentPhase::Adult);
        let s2 = SerializableWorldState::from_world_state(50, &[a2]);
        db.save_snapshot(&s2).unwrap();

        let loaded = db.load_latest_snapshot().unwrap().unwrap();
        assert_eq!(loaded.agents.len(), 1);
        assert_eq!(loaded.agents[0].record.name, "V2");
        assert_eq!(loaded.agents[0].record.tokens, 200);
    }

    #[test]
    fn sqlite_dead_agents_preserved() {
        let db = SqlitePersistence::open_in_memory().unwrap();
        let agents = vec![
            make_agent("Alive", 100, AgentPhase::Adult),
            make_agent("Dead", 0, AgentPhase::Dead),
        ];
        let state = SerializableWorldState::from_world_state(500, &agents);
        db.save_snapshot(&state).unwrap();

        let loaded = db.load_latest_snapshot().unwrap().unwrap();
        assert_eq!(loaded.agents.len(), 2);
        let dead = loaded.agents.iter().find(|a| a.record.name == "Dead").unwrap();
        assert_eq!(dead.record.phase, AgentPhase::Dead);
    }

    #[test]
    fn sqlite_prune_cascades_to_agents_and_ledger() {
        let db = SqlitePersistence::open_in_memory().unwrap();

        // Create snapshots at tick 10, 20, 30 with agents
        let agent = make_agent("Persist", 100, AgentPhase::Adult);
        for tick in [10u64, 20, 30] {
            let s = SerializableWorldState::from_world_state(tick, &[agent.clone()]);
            db.save_snapshot(&s).unwrap();
        }

        // Verify we have 3 snapshots, 3 agent rows (same agent, 3 snapshots), 3 ledger rows
        let conn = db.conn.lock().unwrap();
        let snap_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM snapshots", [], |row| row.get(0))
            .unwrap();
        assert_eq!(snap_count, 3);
        let agent_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM agents", [], |row| row.get(0))
            .unwrap();
        assert_eq!(agent_count, 3);
        let ledger_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM economy_ledger", [], |row| row.get(0))
            .unwrap();
        assert_eq!(ledger_count, 3);
        drop(conn);

        // Prune to keep only 1 snapshot
        db.prune_snapshots(1).unwrap();

        // Verify CASCADE deleted agents and ledger for pruned snapshots
        let conn = db.conn.lock().unwrap();
        let snap_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM snapshots", [], |row| row.get(0))
            .unwrap();
        assert_eq!(snap_count, 1);
        let agent_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM agents", [], |row| row.get(0))
            .unwrap();
        assert_eq!(agent_count, 1);
        let ledger_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM economy_ledger", [], |row| row.get(0))
            .unwrap();
        assert_eq!(ledger_count, 1);
    }

    #[test]
    fn sqlite_same_agent_across_snapshots() {
        let db = SqlitePersistence::open_in_memory().unwrap();

        // Same agent ID across two snapshots (composite PK allows this)
        let id = Uuid::new_v4();

        // Snapshot at tick 10 — agent is Childhood
        let agent_v1 = (
            id,
            0u64,
            AgentRecord {
                id,
                name: "Evolver".to_string(),
                phase: AgentPhase::Childhood,
                tokens: 100,
                skills: HashMap::new(),
            },
        );
        let s1 = SerializableWorldState::from_world_state(10, &[agent_v1]);
        db.save_snapshot(&s1).unwrap();

        // Snapshot at tick 20 — same agent ID, now Adult
        let agent_v2 = (
            id,
            0u64,
            AgentRecord {
                id,
                name: "Evolver".to_string(),
                phase: AgentPhase::Adult,
                tokens: 500,
                skills: HashMap::new(),
            },
        );
        let s2 = SerializableWorldState::from_world_state(20, &[agent_v2]);
        db.save_snapshot(&s2).unwrap();

        // Load each snapshot and verify the agent appears with correct phase
        let conn = db.conn.lock().unwrap();
        let phase_at_10: String = conn
            .query_row(
                "SELECT a.phase FROM agents a JOIN snapshots s ON a.snapshot_id = s.id WHERE s.tick = 10",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let phase_at_20: String = conn
            .query_row(
                "SELECT a.phase FROM agents a JOIN snapshots s ON a.snapshot_id = s.id WHERE s.tick = 20",
                [],
                |row| row.get(0),
            )
            .unwrap();
        drop(conn);

        // Phase is JSON-serialized as lowercase (e.g. "\"childhood\"")
        assert!(phase_at_10.contains("childhood"));
        assert!(phase_at_20.contains("adult"));
    }

    #[test]
    fn sqlite_on_conflict_returning_id_consistent() {
        let db = SqlitePersistence::open_in_memory().unwrap();

        // First insert
        let s1 = SerializableWorldState::from_world_state(42, &[]);
        db.save_snapshot(&s1).unwrap();
        let conn = db.conn.lock().unwrap();
        let id_v1: i64 = conn
            .query_row("SELECT id FROM snapshots WHERE tick = 42", [], |row| row.get(0))
            .unwrap();
        drop(conn);

        // Upsert same tick — ON CONFLICT DO UPDATE should return same id
        let s2 = SerializableWorldState::from_world_state(42, &[]);
        db.save_snapshot(&s2).unwrap();
        let conn = db.conn.lock().unwrap();
        let id_v2: i64 = conn
            .query_row("SELECT id FROM snapshots WHERE tick = 42", [], |row| row.get(0))
            .unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM snapshots WHERE tick = 42", [], |row| row.get(0))
            .unwrap();
        drop(conn);

        assert_eq!(id_v1, id_v2, "RETURNING id should be same on update path");
        assert_eq!(count, 1, "Only one row should exist after upsert");
    }
}
