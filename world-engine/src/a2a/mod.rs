pub mod client_pool;
pub mod federation;
pub mod registry;
pub mod router;
pub mod service;

pub use client_pool::ConnectionPool;
pub use federation::{
    FederationEngine, FederationError, FederationSummary,
    ForeignWorld, DiplomaticStatus,
    CrossWorldTreaty, CrossWorldTreatyType, CrossWorldTreatyStatus,
};
pub use registry::{AgentRegistry, RegisteredAgent};
pub use router::MessageRouter;
pub use service::A2aServiceImpl;
