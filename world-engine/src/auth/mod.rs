//! # Authentication & Role-Based Access Control
//!
//! API key authentication, human-user identity, and capability-based RBAC.
//!
//! Key types: AuthStore, SharedAuthStore, HumanUser, Claims,
//!            HumanRole, Capability, AuthUser, RequireAuth, OptionalAuth
//! Depends on: config
//!
pub mod roles;
pub mod store;
pub mod extractors;

pub use roles::{HumanRole, Capability};
pub use store::{AuthStore, SharedAuthStore, HumanUser, Claims};
pub use extractors::{AuthUser, RequireAuth, OptionalAuth};
