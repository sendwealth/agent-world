//! Federation module — cross-world Registry + Migration subsystem.
//!
//! Provides:
//! - `WorldRegistry` — tracks remote world instances (Phase A: centralized)
//! - `MigrationManager` — handles agent migration between worlds
//! - gRPC services for both WorldRegistryService and MigrationService

pub mod registry;
pub mod migration;
pub mod service;

pub use registry::{WorldRegistry, WorldEntry, WorldStatus};
pub use migration::{
    MigrationManager, MigrationApplication, MigrationPolicy, MigrationStatus,
    AgentSnapshot, MigrationRecord, MigrationType, MigrationStats,
};
