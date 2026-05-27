//! Snapshot engine — non-blocking snapshot creation orchestrated by tick events.
//!
//! The engine listens for tick completions and event triggers via channels.
//! Snapshot creation runs on a dedicated tokio task so it never blocks the
//! tick processing thread. The flow is:
//!
//! 1. After each tick, the scheduler sends a `SnapshotRequest` via a channel.
//! 2. The engine task receives the request and decides whether to snapshot.
//! 3. If a snapshot is needed, it creates a full or incremental snapshot.
//! 4. The snapshot is stored via the storage backend.

use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::{mpsc, Mutex};
use uuid::Uuid;

use crate::economy::token_burn::AgentRecord;
use crate::world::event::WorldEvent;

use super::storage::SnapshotStorage;
use super::types::{SnapshotConfig, SnapshotDelta, WorldSnapshot};

/// Request sent from the tick loop to the snapshot engine.
#[derive(Debug)]
pub enum SnapshotRequest {
    /// A tick has completed; decide whether to snapshot.
    TickCompleted {
        tick: u64,
        agents: Vec<(Uuid, u64, AgentRecord)>,
        events: Vec<WorldEvent>,
    },
    /// Shut down the engine.
    Shutdown,
}

/// Handle to interact with the snapshot engine.
#[derive(Clone)]
pub struct SnapshotEngineHandle {
    sender: mpsc::Sender<SnapshotRequest>,
}

impl SnapshotEngineHandle {
    /// Send a tick completion notification.
    pub async fn notify_tick(
        &self,
        tick: u64,
        agents: Vec<(Uuid, u64, AgentRecord)>,
        events: Vec<WorldEvent>,
    ) {
        let _ = self.sender.send(SnapshotRequest::TickCompleted {
            tick,
            agents,
            events,
        }).await;
    }

    /// Request engine shutdown.
    pub async fn shutdown(&self) {
        let _ = self.sender.send(SnapshotRequest::Shutdown).await;
    }
}

/// The snapshot engine running as a background task.
pub struct SnapshotEngine {
    config: SnapshotConfig,
    storage: SnapshotStorage,
    receiver: mpsc::Receiver<SnapshotRequest>,
    /// The tick at which the last full snapshot was taken.
    last_full_tick: u64,
    /// The last full snapshot (used for delta computation).
    last_full_snapshot: Option<WorldSnapshot>,
}

impl SnapshotEngine {
    /// Create and spawn the snapshot engine as a background tokio task.
    ///
    /// Returns a handle for sending requests and a shared storage reference
    /// for queries. The engine runs an async event loop that processes
    /// snapshot requests from the channel, so snapshot creation never blocks
    /// the tick loop.
    ///
    /// Note: The engine owns its own `SnapshotStorage` instance backed by the
    /// same directory. Queries through the returned `Arc<Mutex<SnapshotStorage>>`
    /// read from disk, so they always see the latest snapshots.
    pub fn spawn(
        config: SnapshotConfig,
        snapshot_dir: PathBuf,
        channel_capacity: usize,
    ) -> anyhow::Result<(SnapshotEngineHandle, Arc<Mutex<SnapshotStorage>>)> {
        // Create storage for the spawned engine task
        let engine_storage = SnapshotStorage::new(snapshot_dir.clone(), config.clone())?;
        // Create a separate storage instance for the caller's read queries,
        // backed by the same directory
        let query_storage = SnapshotStorage::new(snapshot_dir, config.clone())?;

        let (sender, receiver) = mpsc::channel(channel_capacity);

        let storage_arc = Arc::new(Mutex::new(query_storage));

        let mut engine = SnapshotEngine {
            config,
            storage: engine_storage,
            receiver,
            last_full_tick: 0,
            last_full_snapshot: None,
        };

        tokio::spawn(async move {
            engine.run().await;
        });

        let handle = SnapshotEngineHandle { sender };
        Ok((handle, storage_arc))
    }

    /// Create a manual engine for testing (no spawn, caller drives the loop).
    pub fn create_manual(
        config: SnapshotConfig,
        storage: SnapshotStorage,
    ) -> (SnapshotEngineHandle, SnapshotEngine) {
        let (sender, receiver) = mpsc::channel(256);
        let handle = SnapshotEngineHandle { sender };

        let engine = SnapshotEngine {
            config,
            storage,
            receiver,
            last_full_tick: 0,
            last_full_snapshot: None,
        };

        (handle, engine)
    }

    /// Run the engine loop (called by the spawned task or manually for tests).
    pub async fn run(&mut self) {
        while let Some(request) = self.receiver.recv().await {
            match request {
                SnapshotRequest::TickCompleted { tick, agents, events } => {
                    self.process_tick(tick, &agents, &events);
                }
                SnapshotRequest::Shutdown => {
                    break;
                }
            }
        }
    }

    /// Process a single pending request (non-blocking test helper).
    ///
    /// Returns false if no request was pending.
    pub fn try_process_one(&mut self) -> bool {
        match self.receiver.try_recv() {
            Ok(request) => {
                match request {
                    SnapshotRequest::TickCompleted { tick, agents, events } => {
                        self.process_tick(tick, &agents, &events);
                    }
                    SnapshotRequest::Shutdown => {}
                }
                true
            }
            Err(_) => false,
        }
    }

    /// Process all pending requests (drain the channel).
    pub fn process_all_pending(&mut self) {
        while self.try_process_one() {}
    }

    /// Process a single tick completion.
    fn process_tick(
        &mut self,
        tick: u64,
        agents: &[(Uuid, u64, AgentRecord)],
        events: &[WorldEvent],
    ) {
        let should_snapshot = self.should_snapshot(tick, events);
        if !should_snapshot {
            return;
        }

        let is_full = self.last_full_snapshot.is_none()
            || (tick - self.last_full_tick) >= self.config.interval_ticks * 10;

        if is_full {
            self.take_full_snapshot(tick, agents);
        } else {
            self.take_delta_snapshot(tick, agents);
        }
    }

    /// Determine if a snapshot should be taken at this tick.
    fn should_snapshot(&self, tick: u64, events: &[WorldEvent]) -> bool {
        // Check interval-based trigger
        if tick.is_multiple_of(self.config.interval_ticks) {
            return true;
        }

        // Check event-based triggers
        for event in events {
            let event_type_str = format!("{:?}", event.event_type());
            // Convert CamelCase to snake_case for comparison
            let snake_type = camel_to_snake(&event_type_str);
            if self.config.trigger_event_types.contains(&snake_type)
                || self.config.trigger_event_types.contains(&event_type_str)
            {
                return true;
            }
        }

        false
    }

    /// Take a full snapshot.
    fn take_full_snapshot(
        &mut self,
        tick: u64,
        agents: &[(Uuid, u64, AgentRecord)],
    ) {
        let snapshot = WorldSnapshot::from_world_state(tick, agents);

        match self.storage.save_full(&snapshot) {
            Ok(record) => {
                let _ = record; // used for logging in real impl
                self.last_full_tick = tick;
                self.last_full_snapshot = Some(snapshot);
            }
            Err(e) => {
                eprintln!("[SnapshotEngine] Failed to save full snapshot at tick {}: {}", tick, e);
            }
        }
    }

    /// Take an incremental delta snapshot.
    fn take_delta_snapshot(
        &mut self,
        tick: u64,
        agents: &[(Uuid, u64, AgentRecord)],
    ) {
        if let Some(ref prev) = self.last_full_snapshot {
            let delta = SnapshotDelta::diff(prev, tick, agents);

            if delta.is_empty() {
                return; // Skip empty deltas
            }

            match self.storage.save_delta(&delta) {
                Ok(_) => {}
                Err(e) => {
                    eprintln!(
                        "[SnapshotEngine] Failed to save delta at tick {}: {}",
                        tick, e
                    );
                }
            }
        } else {
            // No previous full snapshot — take a full one instead
            self.take_full_snapshot(tick, agents);
        }
    }
}

/// Simple CamelCase to snake_case conversion.
fn camel_to_snake(s: &str) -> String {
    let mut result = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() {
            if i > 0 {
                result.push('_');
            }
            result.push(c.to_ascii_lowercase());
        } else {
            result.push(c);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::economy::token_burn::AgentRecord;
    use crate::world::enums::{AgentPhase, DeathReason};
    use std::collections::HashMap;

    fn make_agent(name: &str, tokens: u64) -> (Uuid, u64, AgentRecord) {
        (
            Uuid::new_v4(),
            0,
            AgentRecord {
                id: Uuid::new_v4(),
                name: name.to_string(),
                phase: AgentPhase::Adult,
                tokens,
                skills: HashMap::new(),
                personality: String::new(),
                tasks_completed: 0,
                tasks_attempted: 0,
            },
        )
    }

    fn make_config() -> SnapshotConfig {
        SnapshotConfig {
            interval_ticks: 10,
            compression_level: 3,
            max_snapshots: 0,
            trigger_event_types: vec!["agent_died".to_string()],
        }
    }

    #[tokio::test]
    async fn engine_takes_periodic_snapshots() {
        let dir = tempfile::tempdir().unwrap();
        let config = make_config();
        let storage = SnapshotStorage::new(dir.path().to_path_buf(), config.clone()).unwrap();
        let (handle, mut engine) = SnapshotEngine::create_manual(config, storage);

        let agents = vec![make_agent("Alice", 1000)];

        // Tick 1-9: no snapshot (interval is 10)
        for tick in 1..10 {
            handle.notify_tick(tick, agents.clone(), vec![]).await;
        }
        engine.process_all_pending();
        assert_eq!(engine.storage.count(), 0);

        // Tick 10: snapshot
        handle.notify_tick(10, agents.clone(), vec![]).await;
        engine.process_all_pending();
        assert_eq!(engine.storage.count(), 1);
    }

    #[tokio::test]
    async fn engine_takes_event_triggered_snapshot() {
        let dir = tempfile::tempdir().unwrap();
        let config = make_config();
        let storage = SnapshotStorage::new(dir.path().to_path_buf(), config.clone()).unwrap();
        let (handle, mut engine) = SnapshotEngine::create_manual(config, storage);

        let agents = vec![make_agent("Alice", 1000)];

        // Tick 5 with an AgentDied event (trigger is configured)
        let events = vec![WorldEvent::AgentDied {
            agent_id: "agent-001".to_string(),
            reason: DeathReason::TokenDepleted,
        }];

        handle.notify_tick(5, agents.clone(), events).await;
        engine.process_all_pending();
        assert_eq!(engine.storage.count(), 1);
    }

    #[tokio::test]
    async fn engine_full_then_delta() {
        let dir = tempfile::tempdir().unwrap();
        let config = make_config();
        let storage = SnapshotStorage::new(dir.path().to_path_buf(), config.clone()).unwrap();
        let (handle, mut engine) = SnapshotEngine::create_manual(config, storage);

        let agents = vec![make_agent("Alice", 1000)];

        // Tick 10: full snapshot
        handle.notify_tick(10, agents.clone(), vec![]).await;
        engine.process_all_pending();

        // Tick 20: delta (agents changed)
        let mut agents_changed = agents.clone();
        agents_changed[0].2.tokens = 900;
        handle.notify_tick(20, agents_changed, vec![]).await;
        engine.process_all_pending();

        assert_eq!(engine.storage.count(), 2);

        // Verify reconstruction works
        let reconstructed = engine.storage.reconstruct_at(20).unwrap();
        assert_eq!(reconstructed.agents.len(), 1);
        assert_eq!(reconstructed.agents[0].tokens, 900);
    }

    #[tokio::test]
    async fn spawn_actually_creates_snapshots() {
        let dir = tempfile::tempdir().unwrap();
        let config = make_config();
        let (handle, storage) = SnapshotEngine::spawn(
            config,
            dir.path().to_path_buf(),
            256,
        ).unwrap();

        let agents = vec![make_agent("Alice", 1000)];

        // Send tick 10 (should trigger periodic snapshot)
        handle.notify_tick(10, agents.clone(), vec![]).await;

        // Give the background task time to process
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Query via the shared storage — rebuild index first to pick up new files
        {
            let mut s = storage.lock().await;
            s.rebuild_index_public().unwrap();
            assert_eq!(s.count(), 1);
        }

        handle.shutdown().await;
    }

    #[test]
    fn camel_to_snake_conversion() {
        assert_eq!(camel_to_snake("AgentDied"), "agent_died");
        assert_eq!(camel_to_snake("TickAdvanced"), "tick_advanced");
        assert_eq!(camel_to_snake("BalanceChanged"), "balance_changed");
    }
}
