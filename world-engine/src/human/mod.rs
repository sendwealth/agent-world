//! # Human Participation Layer
//!
//! Tracks real-human observers/operators interacting with the simulation.
//! All data is persisted to SQLite for crash recovery.
//!
//! Key types: HumanParticipationStore, SharedHumanStore
//! Depends on: auth (HumanUser), persistence (SQLite)
//!
//! The HumanActionQueue / HumanAgentSubsystem types live in [`crate::human_agent`].
//! This module re-exports [`HumanActionType`] for API handlers under `/human/*`
//! that need typed action validation.
//!
pub mod store;

// Re-export the unified types from `human_agent` so callers that
// historically reached them via `crate::human::*` keep working.
pub use crate::human_agent::{
    HumanActionQueue, HumanActionType, QueuedAction, SharedHumanActionQueue,
};
pub use store::{HumanParticipationStore, SharedHumanStore, RechargeEntry, RechargeRequest};
