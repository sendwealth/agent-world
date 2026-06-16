//! Federation module — cross-world Registry + Migration subsystem.
//!
//! Provides:
//! - `WorldRegistry` — tracks remote world instances (Phase A: centralized)
//! - `MigrationManager` — handles agent migration between worlds
//! - gRPC services for both WorldRegistryService and MigrationService

pub mod migration;
pub mod registry;
pub mod service;
pub mod trade;

pub use migration::{
    AgentSnapshot, MigrationApplication, MigrationManager, MigrationPolicy, MigrationRecord,
    MigrationStats, MigrationStatus, MigrationType,
};
pub use registry::{WorldEntry, WorldRegistry, WorldStatus};
pub use trade::{
    CrossWorldTrade, CrossWorldTradeManager, CrossWorldTradeOffer, TradeItem, TradeStats,
    TradeStatus,
};
