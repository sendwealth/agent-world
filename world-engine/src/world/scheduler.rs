//! Scheduler — drives the world forward at a configurable tick interval.
//!
//! The [`Scheduler`] wraps a [`SharedWorldState`] and calls `tick()` on it
//! at a regular interval derived from `genesis.yaml` (default: 1 second).
//! It uses `tokio::time::interval` for precise, non-drifting timing and
//! supports graceful shutdown via a cancellation token.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use super::state::WorldState;

/// Shared, thread-safe handle to the world state.
pub type SharedWorldState = Arc<Mutex<WorldState>>;

/// Drives periodic tick execution on a [`WorldState`].
pub struct Scheduler {
    /// Duration between ticks.
    interval: Duration,
    /// The world state being advanced.
    state: SharedWorldState,
    /// Token used to request graceful shutdown.
    cancel: CancellationToken,
}

impl Scheduler {
    /// Create a new scheduler.
    ///
    /// * `interval` — time between ticks (e.g. `Duration::from_millis(1000)`)
    /// * `state` — shared world state to tick
    pub fn new(interval: Duration, state: SharedWorldState) -> Self {
        Self {
            interval,
            state,
            cancel: CancellationToken::new(),
        }
    }

    /// Returns a clone of the cancellation token so external code can
    /// request shutdown.
    pub fn cancel_token(&self) -> CancellationToken {
        self.cancel.clone()
    }

    /// Run the tick loop until cancelled.
    ///
    /// This method is designed to be spawned on a tokio task:
    /// ```ignore
    /// let handle = tokio::spawn(scheduler.run());
    /// // ... later ...
    /// scheduler.cancel_token().cancel();
    /// handle.await?;
    /// ```
    pub async fn run(self) {
        let mut ticker = tokio::time::interval(self.interval);
        // The first tick fires immediately; skip it so we wait for the
        // configured interval before the first real tick.
        ticker.tick().await;

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    let mut state = self.state.lock().await;
                    state.tick();
                }
                _ = self.cancel.cancelled() => {
                    break;
                }
            }
        }
    }

    /// Convenience: run `n` ticks back-to-back (no delay).
    ///
    /// Useful for tests and simulations that don't need real-time pacing.
    pub async fn run_n_ticks(state: &SharedWorldState, n: u64) {
        for _ in 0..n {
            let mut s = state.lock().await;
            s.tick();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::subsystem::SubsystemRegistry;
    use crate::world::state::EventBus;

    use uuid::Uuid;
    use std::collections::HashMap;
    use crate::economy::token_burn::AgentRecord;
    use crate::world::enums::AgentPhase;

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
            tasks_completed: 0,
            tasks_attempted: 0,
            },
        )
    }

    #[tokio::test]
    async fn run_n_ticks_advances_state() {
        let event_bus = Arc::new(EventBus::new(256));
        let registry = SubsystemRegistry::new();
        let agents = vec![make_agent(AgentPhase::Adult, 1000)];

        let state = Arc::new(Mutex::new(
            WorldState::new(event_bus, registry, agents)
        ));

        Scheduler::run_n_ticks(&state, 10).await;

        let s = state.lock().await;
        assert_eq!(s.current_tick(), 10);
    }

    #[tokio::test]
    async fn scheduler_stops_on_cancel() {
        let event_bus = Arc::new(EventBus::new(256));
        let registry = SubsystemRegistry::new();
        let agents = vec![make_agent(AgentPhase::Adult, 1000)];

        let state = Arc::new(Mutex::new(
            WorldState::new(event_bus, registry, agents)
        ));

        let scheduler = Scheduler::new(Duration::from_millis(10), state.clone());
        let cancel = scheduler.cancel_token();

        let handle = tokio::spawn(scheduler.run());

        // Let it run a few ticks
        tokio::time::sleep(Duration::from_millis(50)).await;
        cancel.cancel();

        handle.await.unwrap();

        let s = state.lock().await;
        assert!(s.current_tick() > 0);
    }
}
