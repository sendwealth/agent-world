//! Plugin permission system.
//!
//! Each plugin declares a set of permissions it needs. The engine
//! enforces these at runtime — a plugin cannot access data or
//! perform operations beyond its granted permissions.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Capabilities a plugin can request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    // ── Read permissions ──
    /// Read agent state (name, tokens, phase, skills).
    ReadAgents,
    /// Read world tick and config.
    ReadWorldState,
    /// Subscribe to events (read-only).
    ReadEvents,

    // ── Write permissions ──
    /// Modify agent tokens (deduct, add).
    WriteAgentTokens,
    /// Modify agent phase (e.g., force phase transitions).
    WriteAgentPhase,
    /// Modify agent skills.
    WriteAgentSkills,
    /// Emit custom events onto the event bus.
    EmitEvents,

    // ── Action permissions ──
    /// Intercept and potentially block agent actions.
    InterceptActions,
    /// Intercept and potentially modify transactions.
    InterceptTransactions,
    /// Register as a tick subsystem (runs every tick).
    TickSubsystem,

    // ── Admin permissions ──
    /// Access other subsystems' state.
    AdminAccess,
}

/// A set of granted permissions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionSet {
    permissions: HashSet<Permission>,
}

impl PermissionSet {
    /// Create an empty permission set.
    pub fn new() -> Self {
        Self {
            permissions: HashSet::new(),
        }
    }

    /// Create a permission set from an iterator of permissions.
    pub fn from_iter<I: IntoIterator<Item = Permission>>(iter: I) -> Self {
        Self {
            permissions: iter.into_iter().collect(),
        }
    }

    /// Grant a permission.
    pub fn grant(&mut self, perm: Permission) {
        self.permissions.insert(perm);
    }

    /// Revoke a permission.
    pub fn revoke(&mut self, perm: Permission) {
        self.permissions.remove(&perm);
    }

    /// Check if a specific permission is granted.
    pub fn has(&self, perm: Permission) -> bool {
        self.permissions.contains(&perm)
    }

    /// Check if all specified permissions are granted.
    pub fn has_all(&self, perms: &[Permission]) -> bool {
        perms.iter().all(|p| self.permissions.contains(p))
    }

    /// Read-only access to the underlying set.
    pub fn permissions(&self) -> &HashSet<Permission> {
        &self.permissions
    }

    /// Number of granted permissions.
    pub fn len(&self) -> usize {
        self.permissions.len()
    }

    /// Whether no permissions are granted.
    pub fn is_empty(&self) -> bool {
        self.permissions.is_empty()
    }
}

impl Default for PermissionSet {
    fn default() -> Self {
        Self::new()
    }
}
