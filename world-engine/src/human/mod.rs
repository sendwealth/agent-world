//! # Human Participation Layer
//!
//! Tracks real-human observers/operators interacting with the simulation.
//! All data is persisted to SQLite for crash recovery.
//!
//! Key types: HumanParticipationStore, SharedHumanStore
//! Depends on: auth (HumanUser), persistence (SQLite)
//!
pub mod action_queue;
pub mod store;

pub use action_queue::{
    HumanAction, HumanActionQueue, HumanActionType, HumanAgentState, SharedHumanActionQueue,
};
pub use store::{HumanParticipationStore, SharedHumanStore, RechargeEntry, RechargeRequest};
