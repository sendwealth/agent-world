//! # Human Participation Layer
//!
//! Tracks real-human observers/operators interacting with the simulation.
//! All data is persisted to SQLite for crash recovery.
//!
//! Key types: HumanParticipationStore, SharedHumanStore
//! Depends on: auth (HumanUser), persistence (SQLite)
//!
pub mod store;

pub use store::{HumanParticipationStore, SharedHumanStore, RechargeRequest, RechargeEntry};
