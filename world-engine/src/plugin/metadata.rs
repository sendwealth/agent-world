//! Plugin metadata and state tracking.

use serde::{Deserialize, Serialize};

/// Static metadata describing a plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMetadata {
    /// Unique plugin identifier (e.g. "com.example.tax-plugin").
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Plugin version (semver recommended).
    pub version: String,
    /// Brief description of what the plugin does.
    pub description: String,
    /// Author or organization.
    pub author: String,
    /// Ordered priority for hook execution (lower = runs first).
    /// Default: 100.
    pub priority: u32,
}

/// Runtime state of a registered plugin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginState {
    /// Registered but not yet initialized.
    Registered,
    /// Successfully loaded and active.
    Active,
    /// Temporarily disabled.
    Disabled,
    /// Failed during initialization or execution.
    Error,
    /// Gracefully shut down.
    Unloaded,
}

impl std::fmt::Display for PluginState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Registered => write!(f, "registered"),
            Self::Active => write!(f, "active"),
            Self::Disabled => write!(f, "disabled"),
            Self::Error => write!(f, "error"),
            Self::Unloaded => write!(f, "unloaded"),
        }
    }
}
