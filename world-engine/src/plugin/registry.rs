//! Plugin registry — storage and lookup for registered plugins.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::error::PluginError;
use super::hooks::*;
use super::metadata::{PluginMetadata, PluginState};
use super::permission::PermissionSet;

/// Type-erased hook set stored for a registered plugin.
/// Each field is `Some` if the plugin implements the corresponding trait.
#[derive(Default)]
pub struct PluginHooks {
    pub on_tick_start: Option<Box<dyn OnTickStart>>,
    pub on_tick_end: Option<Box<dyn OnTickEnd>>,
    pub on_agent_action: Option<Box<dyn OnAgentAction>>,
    pub on_transaction: Option<Box<dyn OnTransaction>>,
    pub on_agent_spawn: Option<Box<dyn OnAgentSpawn>>,
    pub on_event: Option<Box<dyn OnEvent>>,
    pub on_startup: Option<Box<dyn OnStartup>>,
    pub on_shutdown: Option<Box<dyn OnShutdown>>,
}

/// Entry for a single registered plugin.
pub struct PluginEntry {
    pub metadata: PluginMetadata,
    pub state: PluginState,
    pub permissions: PermissionSet,
    pub hooks: PluginHooks,
}

/// Serializable plugin info for API responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub priority: u32,
    pub state: PluginState,
    pub permissions: Vec<String>,
    pub hooks: Vec<String>,
}

impl From<&PluginEntry> for PluginInfo {
    fn from(entry: &PluginEntry) -> Self {
        let mut hooks = Vec::new();
        if entry.hooks.on_tick_start.is_some() {
            hooks.push("on_tick_start".to_string());
        }
        if entry.hooks.on_tick_end.is_some() {
            hooks.push("on_tick_end".to_string());
        }
        if entry.hooks.on_agent_action.is_some() {
            hooks.push("on_agent_action".to_string());
        }
        if entry.hooks.on_transaction.is_some() {
            hooks.push("on_transaction".to_string());
        }
        if entry.hooks.on_agent_spawn.is_some() {
            hooks.push("on_agent_spawn".to_string());
        }
        if entry.hooks.on_event.is_some() {
            hooks.push("on_event".to_string());
        }
        if entry.hooks.on_startup.is_some() {
            hooks.push("on_startup".to_string());
        }
        if entry.hooks.on_shutdown.is_some() {
            hooks.push("on_shutdown".to_string());
        }

        Self {
            id: entry.metadata.id.clone(),
            name: entry.metadata.name.clone(),
            version: entry.metadata.version.clone(),
            description: entry.metadata.description.clone(),
            author: entry.metadata.author.clone(),
            priority: entry.metadata.priority,
            state: entry.state,
            permissions: entry
                .permissions
                .permissions()
                .iter()
                .map(|p| format!("{:?}", p).to_lowercase())
                .collect::<Vec<String>>(),
            hooks,
        }
    }
}

/// Registry of all plugins, keyed by ID.
pub struct PluginRegistry {
    entries: HashMap<String, PluginEntry>,
}

impl PluginRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Register a new plugin entry.
    pub fn register(&mut self, entry: PluginEntry) -> Result<(), PluginError> {
        let id = entry.metadata.id.clone();
        if self.entries.contains_key(&id) {
            return Err(PluginError::AlreadyRegistered(id));
        }
        self.entries.insert(id, entry);
        Ok(())
    }

    /// Look up a plugin by ID.
    pub fn get(&self, id: &str) -> Option<&PluginEntry> {
        self.entries.get(id)
    }

    /// Look up a plugin by ID (mutable).
    pub fn get_mut(&mut self, id: &str) -> Option<&mut PluginEntry> {
        self.entries.get_mut(id)
    }

    /// Remove a plugin by ID.
    pub fn remove(&mut self, id: &str) -> Option<PluginEntry> {
        self.entries.remove(id)
    }

    /// List all plugin IDs.
    pub fn plugin_ids(&self) -> Vec<String> {
        self.entries.keys().cloned().collect()
    }

    /// Get info for all registered plugins.
    pub fn list_plugins(&self) -> Vec<PluginInfo> {
        self.entries.values().map(PluginInfo::from).collect()
    }

    /// Get all entries sorted by priority (ascending).
    pub fn sorted_by_priority(&self) -> Vec<&PluginEntry> {
        let mut entries: Vec<_> = self.entries.values().collect();
        entries.sort_by_key(|e| e.metadata.priority);
        entries
    }

    /// Get all entries sorted by priority (ascending), mutable.
    pub fn sorted_by_priority_mut(&mut self) -> Vec<&mut PluginEntry> {
        let mut entries: Vec<_> = self.entries.values_mut().collect();
        entries.sort_by_key(|e| e.metadata.priority);
        entries
    }

    /// Number of registered plugins.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Iterate over all entries.
    pub fn iter(&self) -> impl Iterator<Item = &PluginEntry> {
        self.entries.values()
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}
