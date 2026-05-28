//! # Engine Orchestration
//!
//! Core simulation state container and emergent culture tracking.
//!
//! Key types: WorldState, StateError, CultureStore,
//!            OrgCultureVector, CulturalCluster
//! Depends on: world (subsystems, EventBus), economy, organization
//!
pub mod culture;
pub mod state;

pub use culture::CultureStore;
pub use state::{StateError, WorldState};
