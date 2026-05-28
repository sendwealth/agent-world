//! # Authentication & Role-Based Access Control
//!
//! API key authentication, human-user identity, and capability-based RBAC.
//!
//! Key types: AuthStore, SharedAuthStore, HumanUser, Claims,
//!            HumanRole, Capability, AuthUser, RequireAuth, OptionalAuth
//! Depends on: config
//!
pub mod extractors;
pub mod roles;
pub mod store;

pub use extractors::{AuthUser, OptionalAuth, RequireAuth};
pub use roles::{Capability, HumanRole};
pub use store::{AuthStore, Claims, HumanUser, SharedAuthStore};
