//! # A2A (Agent-to-Agent) Communication Protocol
//!
//! Inter-world agent discovery, messaging, and federation via gRPC.
//!
//! Key types: ConnectionPool, FederationEngine, AgentRegistry,
//!            RegisteredAgent, MessageRouter, A2aServiceImpl,
//!            ForeignWorld, CrossWorldTreaty
//! Depends on: world (WorldState, EventBus)
//!
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
