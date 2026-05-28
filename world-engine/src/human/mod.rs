//! # Human Participation Layer
//!
//! Tracks real-human observers/operators interacting with the simulation.
//!
//! Key types: HumanParticipationStore, SharedHumanStore
//! Depends on: auth (HumanUser)
//!
pub mod store;

pub use store::{HumanParticipationStore, SharedHumanStore};
