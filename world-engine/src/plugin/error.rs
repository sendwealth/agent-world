//! Plugin error types.

use std::fmt;

/// Errors that can occur during plugin operations.
#[derive(Debug)]
pub enum PluginError {
    /// A plugin with the same ID is already registered.
    AlreadyRegistered(String),
    /// No plugin found with the given ID.
    NotFound(String),
    /// The plugin lacks the required permission.
    PermissionDenied {
        plugin_id: String,
        required: String,
    },
    /// The plugin failed during initialization.
    InitFailed(String),
    /// The plugin returned an error during hook execution.
    HookFailed(String),
    /// Invalid plugin configuration.
    InvalidConfig(String),
    /// The plugin is in an invalid state for this operation.
    InvalidState {
        plugin_id: String,
        current: String,
        required: String,
    },
}

impl fmt::Display for PluginError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyRegistered(id) => write!(f, "plugin already registered: {}", id),
            Self::NotFound(id) => write!(f, "plugin not found: {}", id),
            Self::PermissionDenied {
                plugin_id,
                required,
            } => write!(
                f,
                "plugin '{}' lacks permission: {}",
                plugin_id, required
            ),
            Self::InitFailed(msg) => write!(f, "plugin init failed: {}", msg),
            Self::HookFailed(msg) => write!(f, "plugin hook failed: {}", msg),
            Self::InvalidConfig(msg) => write!(f, "invalid plugin config: {}", msg),
            Self::InvalidState {
                plugin_id,
                current,
                required,
            } => write!(
                f,
                "plugin '{}' is in state '{}', required: '{}'",
                plugin_id, current, required
            ),
        }
    }
}

impl std::error::Error for PluginError {}
