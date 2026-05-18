//! Tick scheduler — drives automatic world progression.
//!
//! The scheduler holds a shared `WorldState` and advances it by one tick
//! at a configurable interval. It runs as a background tokio task and
//! supports graceful shutdown.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::watch;
use tokio::time::{self, MissedTickBehavior};

use super::engine::{WorldState, TickResult};

// ═══════════════════════════════════════════════════════════════════════════
// Scheduler
// ═══════════════════════════════════════════════════════════════════════════

/// Configuration for the tick scheduler.
#[derive(Debug, Clone)]
pub struct SchedulerConfig {
    /// Duration between ticks.
    pub tick_interval: Duration,
}

impl SchedulerConfig {
    /// Create with a tick interval in milliseconds.
    pub fn from_millis(ms: u64) -> Self {
        Self {
            tick_interval: Duration::from_millis(ms),
        }
    }

    /// Create with a tick interval in seconds.
    pub fn from_secs(secs: u64) -> Self {
        Self {
            tick_interval: Duration::from_secs(secs),
        }
    }
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        // Default: 1 second per tick (matches genesis.yaml tick_interval_ms: 1000)
        Self {
            tick_interval: Duration::from_millis(1000),
        }
    }
}

/// The tick scheduler drives the world forward automatically.
///
/// It holds an `Arc<WorldState>` and spawns a background tokio task that
/// calls `world.tick()` at the configured interval. The scheduler can be
/// stopped via a shutdown signal or by dropping the handle.
pub struct Scheduler {
    world: Arc<WorldState>,
    config: SchedulerConfig,
    shutdown_tx: Option<watch::Sender<bool>>,
    handle: Option<tokio::task::JoinHandle<()>>,
}

impl Scheduler {
    /// Create a new scheduler for the given world state.
    pub fn new(world: Arc<WorldState>, config: SchedulerConfig) -> Self {
        Self {
            world,
            config,
            shutdown_tx: None,
            handle: None,
        }
    }

    /// Start the scheduler. Returns `true` if started, `false` if already running.
    pub fn start(&mut self) -> bool {
        if self.handle.is_some() {
            return false;
        }

        let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
        self.shutdown_tx = Some(shutdown_tx);

        let world = self.world.clone();
        let interval = self.config.tick_interval;

        let handle = tokio::spawn(async move {
            let mut ticker = time::interval(interval);
            ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

            loop {
                // Check for shutdown signal
                if *shutdown_rx.borrow() {
                    break;
                }

                tokio::select! {
                    _ = ticker.tick() => {
                        let result = world.tick().await;
                        if !result.all_subsystems_ok() {
                            for sr in &result.subsystem_results {
                                if !sr.success {
                                    eprintln!(
                                        "[Scheduler] tick {}: subsystem '{}' failed: {}",
                                        result.tick,
                                        sr.subsystem_id,
                                        sr.error.as_deref().unwrap_or("unknown")
                                    );
                                }
                            }
                        }

                        // Log dead agents
                        if !result.dead_agents.is_empty() {
                            eprintln!(
                                "[Scheduler] tick {}: {} agent(s) died: {}",
                                result.tick,
                                result.dead_agents.len(),
                                result.dead_agents.join(", ")
                            );
                        }
                    }
                    _ = shutdown_rx.changed() => {
                        break;
                    }
                }
            }

            println!("[Scheduler] Stopped at tick {}", world.current_tick());
        });

        self.handle = Some(handle);
        true
    }

    /// Stop the scheduler gracefully.
    pub async fn stop(&mut self) {
        if let Some(tx) = &self.shutdown_tx {
            let _ = tx.send(true);
        }
        if let Some(handle) = self.handle.take() {
            let _ = handle.await;
        }
        self.shutdown_tx = None;
    }

    /// Check if the scheduler is currently running.
    pub fn is_running(&self) -> bool {
        self.handle.is_some()
    }

    /// Get a reference to the world state.
    pub fn world(&self) -> &Arc<WorldState> {
        &self.world
    }

    /// Manually trigger a single tick (does not require the scheduler to be running).
    pub async fn tick_once(&self) -> TickResult {
        self.world.tick().await
    }
}

impl Drop for Scheduler {
    fn drop(&mut self) {
        if let Some(tx) = &self.shutdown_tx {
            let _ = tx.send(true);
        }
        if let Some(handle) = self.handle.take() {
            handle.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::enums::AgentPhase;

    #[tokio::test]
    async fn test_scheduler_config_default() {
        let config = SchedulerConfig::default();
        assert_eq!(config.tick_interval, Duration::from_millis(1000));
    }

    #[tokio::test]
    async fn test_scheduler_config_from_millis() {
        let config = SchedulerConfig::from_millis(500);
        assert_eq!(config.tick_interval, Duration::from_millis(500));
    }

    #[tokio::test]
    async fn test_scheduler_start_stop() {
        let world = Arc::new(WorldState::with_defaults());
        let config = SchedulerConfig::from_millis(50);
        let mut scheduler = Scheduler::new(world.clone(), config);

        assert!(!scheduler.is_running());
        assert!(scheduler.start());
        assert!(scheduler.is_running());

        // Second start should return false
        assert!(!scheduler.start());

        // Let it run a few ticks
        tokio::time::sleep(Duration::from_millis(200)).await;

        assert!(world.current_tick() > 0);

        scheduler.stop().await;
        assert!(!scheduler.is_running());
    }

    #[tokio::test]
    async fn test_scheduler_tick_once() {
        let world = Arc::new(WorldState::with_defaults());
        let config = SchedulerConfig::default();
        let scheduler = Scheduler::new(world.clone(), config);

        let id = world.spawn_agent_with_phase("Alice", 100, AgentPhase::Adult).await;

        let result = scheduler.tick_once().await;
        assert_eq!(result.tick, 1);

        let agent = world.get_agent(&id).await.unwrap();
        assert_eq!(agent.tokens, 90);
    }

    #[tokio::test]
    async fn test_scheduler_auto_advances_world() {
        let world = Arc::new(WorldState::with_defaults());
        world.spawn_agent_with_phase("Alice", 100_000, AgentPhase::Adult).await;

        let config = SchedulerConfig::from_millis(10);
        let mut scheduler = Scheduler::new(world.clone(), config);
        scheduler.start();

        // Let it run for ~200ms → ~20 ticks
        tokio::time::sleep(Duration::from_millis(200)).await;
        scheduler.stop().await;

        let tick = world.current_tick();
        assert!(tick >= 5, "Expected at least 5 ticks, got {}", tick);
    }

    #[tokio::test]
    async fn test_scheduler_graceful_shutdown() {
        let world = Arc::new(WorldState::with_defaults());
        let config = SchedulerConfig::from_millis(10);
        let mut scheduler = Scheduler::new(world.clone(), config);
        scheduler.start();

        // Let it tick for a bit
        tokio::time::sleep(Duration::from_millis(100)).await;
        scheduler.stop().await;

        let tick_at_stop = world.current_tick();
        assert!(tick_at_stop > 0, "Should have ticked at least once");

        // Wait and verify ticks stopped — the count should remain stable
        tokio::time::sleep(Duration::from_millis(100)).await;
        let tick_after = world.current_tick();
        assert_eq!(tick_at_stop, tick_after,
            "Ticks should stop after shutdown: tick_at_stop={}, tick_after={}", tick_at_stop, tick_after);
    }
}
