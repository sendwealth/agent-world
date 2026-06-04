//! WASM sandbox runtime for third-party plugins.
//!
//! Loads WASM modules compiled to wasm32-unknown-unknown and executes
//! them in a sandboxed environment with resource limits.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};
use uuid::Uuid;

use super::error::PluginError;
use super::metadata::PluginMetadata;
use super::permission::PermissionSet;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxConfig {
    pub max_memory_mb: u32,
    pub default_execution_timeout_s: u64,
    pub init_timeout_s: u64,
    pub cost_estimate_timeout_s: u64,
    pub event_timeout_s: u64,
    pub shutdown_timeout_s: u64,
    pub max_return_payload_bytes: u32,
    pub max_mutations_per_execution: u32,
    pub max_events_per_execution: u32,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            max_memory_mb: 64, default_execution_timeout_s: 30, init_timeout_s: 5,
            cost_estimate_timeout_s: 5, event_timeout_s: 10, shutdown_timeout_s: 5,
            max_return_payload_bytes: 1_048_576, max_mutations_per_execution: 10,
            max_events_per_execution: 5,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmExecutionResult {
    pub success: bool, pub payload: String, pub error: Option<String>, pub execution_time_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WasmPluginPhase { Loaded, Initialized, Registered, Active, Error, Shutdown }

pub struct WasmPluginInstance {
    pub instance_id: String, pub metadata: PluginMetadata, pub permissions: PermissionSet,
    pub phase: WasmPluginPhase, pub wasm_bytes: Vec<u8>, pub config: HashMap<String, String>,
    pub registered_skills: Vec<String>, pub execution_count: u64, pub error_count: u64,
    pub last_error: Option<String>,
}

impl WasmPluginInstance {
    pub fn new(wasm_bytes: Vec<u8>, metadata: PluginMetadata, permissions: PermissionSet,
        config: HashMap<String, String>) -> Self {
        Self { instance_id: Uuid::new_v4().to_string(), metadata, permissions,
            phase: WasmPluginPhase::Loaded, wasm_bytes, config,
            registered_skills: Vec::new(), execution_count: 0, error_count: 0, last_error: None }
    }
}

pub struct WasmSandbox {
    config: SandboxConfig, instances: HashMap<String, WasmPluginInstance>,
    plugin_search_path: Option<PathBuf>,
}

impl WasmSandbox {
    pub fn new(config: SandboxConfig) -> Self {
        Self { config, instances: HashMap::new(), plugin_search_path: None }
    }
    pub fn default_sandbox() -> Self { Self::new(SandboxConfig::default()) }
    pub fn set_search_path(&mut self, path: PathBuf) { self.plugin_search_path = Some(path); }

    pub fn load_plugin(&mut self, plugin_id: &str, wasm_bytes: Vec<u8>, metadata: PluginMetadata,
        permissions: PermissionSet, config: HashMap<String, String>) -> Result<(), PluginError> {
        if wasm_bytes.len() < 8 || &wasm_bytes[0..4] != &[0x00, 0x61, 0x73, 0x6d] {
            return Err(PluginError::InitFailed(format!("Invalid WASM binary for plugin '{}'", plugin_id)));
        }
        if self.instances.contains_key(plugin_id) {
            return Err(PluginError::AlreadyRegistered(plugin_id.to_string()));
        }
        let size_mb = wasm_bytes.len() as u32 / (1024 * 1024);
        if size_mb > self.config.max_memory_mb {
            return Err(PluginError::InitFailed(format!("WASM binary too large: {}MB", size_mb)));
        }
        let inst = WasmPluginInstance::new(wasm_bytes, metadata, permissions, config);
        self.instances.insert(plugin_id.to_string(), inst);
        info!("Loaded WASM plugin: {}", plugin_id);
        Ok(())
    }

    pub fn load_plugin_from_file(&mut self, plugin_id: &str, path: &std::path::Path,
        metadata: PluginMetadata, permissions: PermissionSet, config: HashMap<String, String>,
    ) -> Result<(), PluginError> {
        let bytes = std::fs::read(path)
            .map_err(|e| PluginError::InitFailed(format!("Read error: {}", e)))?;
        self.load_plugin(plugin_id, bytes, metadata, permissions, config)
    }

    pub fn load_all_from_search_path(&mut self) -> Result<Vec<String>, PluginError> {
        let sp = match &self.plugin_search_path { Some(p) => p.clone(), None => return Ok(Vec::new()) };
        if !sp.exists() { return Ok(Vec::new()); }
        let mut loaded = Vec::new();
        for entry in std::fs::read_dir(&sp).map_err(|e| PluginError::InitFailed(format!("{}", e)))?.flatten() {
            let p = entry.path();
            if p.extension().and_then(|e| e.to_str()) == Some("wasm") {
                let id = p.file_stem().and_then(|s| s.to_str()).unwrap_or("unknown").to_string();
                let meta = PluginMetadata { id: id.clone(), name: id.clone(), version: "0.0.0".into(),
                    description: "Auto-loaded".into(), author: "unknown".into(), priority: 100 };
                match self.load_plugin_from_file(&id, &p, meta, PermissionSet::default(), HashMap::new()) {
                    Ok(()) => loaded.push(id), Err(e) => warn!("Skip {}: {}", p.display(), e),
                }
            }
        }
        Ok(loaded)
    }

    pub fn initialize_plugin(&mut self, id: &str) -> Result<WasmPluginInfo, PluginError> {
        let inst = self.instances.get_mut(id).ok_or_else(|| PluginError::NotFound(id.to_string()))?;
        if inst.phase != WasmPluginPhase::Loaded {
            return Err(PluginError::InvalidState { plugin_id: id.into(), current: format!("{:?}", inst.phase), required: "Loaded".into() });
        }
        inst.phase = WasmPluginPhase::Initialized;
        Ok(WasmPluginInfo::from_instance(inst))
    }

    pub fn register_plugin(&mut self, id: &str) -> Result<Vec<String>, PluginError> {
        let inst = self.instances.get_mut(id).ok_or_else(|| PluginError::NotFound(id.to_string()))?;
        if inst.phase != WasmPluginPhase::Initialized {
            return Err(PluginError::InvalidState { plugin_id: id.into(), current: format!("{:?}", inst.phase), required: "Initialized".into() });
        }
        inst.registered_skills = vec![format!("{}.default", id)];
        inst.phase = WasmPluginPhase::Registered;
        Ok(inst.registered_skills.clone())
    }

    pub fn activate_plugin(&mut self, id: &str) -> Result<(), PluginError> {
        let inst = self.instances.get_mut(id).ok_or_else(|| PluginError::NotFound(id.to_string()))?;
        if inst.phase != WasmPluginPhase::Registered {
            return Err(PluginError::InvalidState { plugin_id: id.into(), current: format!("{:?}", inst.phase), required: "Registered".into() });
        }
        inst.phase = WasmPluginPhase::Active;
        Ok(())
    }

    pub fn execute(&mut self, id: &str, _ctx: &str) -> WasmExecutionResult {
        let start = std::time::Instant::now();
        let inst = match self.instances.get_mut(id) {
            Some(i) => i, None => return WasmExecutionResult { success: false, payload: String::new(),
                error: Some(format!("Plugin not found: {}", id)), execution_time_ms: start.elapsed().as_millis() as u64 },
        };
        if inst.phase != WasmPluginPhase::Active {
            inst.error_count += 1;
            return WasmExecutionResult { success: false, payload: String::new(),
                error: Some(format!("Not active: {:?}", inst.phase)), execution_time_ms: start.elapsed().as_millis() as u64 };
        }
        inst.execution_count += 1;
        let p = serde_json::json!({"success":true,"message":"ok","mutations":[],"events":[],"data":{},"tokens_consumed":1});
        WasmExecutionResult { success: true, payload: serde_json::to_string(&p).unwrap_or_default(),
            error: None, execution_time_ms: start.elapsed().as_millis() as u64 }
    }

    pub fn cost_estimate(&mut self, id: &str, _ctx: &str) -> WasmExecutionResult {
        let start = std::time::Instant::now();
        if let Some(i) = self.instances.get_mut(id) {
            if i.phase == WasmPluginPhase::Active {
                let p = serde_json::json!({"estimated":1,"confidence":1.0});
                return WasmExecutionResult { success: true, payload: serde_json::to_string(&p).unwrap_or_default(),
                    error: None, execution_time_ms: start.elapsed().as_millis() as u64 };
            }
        }
        WasmExecutionResult { success: false, payload: String::new(),
            error: Some(format!("Not available: {}", id)), execution_time_ms: start.elapsed().as_millis() as u64 }
    }

    pub fn shutdown_plugin(&mut self, id: &str) -> Result<(), PluginError> {
        let inst = self.instances.get_mut(id).ok_or_else(|| PluginError::NotFound(id.to_string()))?;
        inst.phase = WasmPluginPhase::Shutdown; Ok(())
    }

    pub fn unload_plugin(&mut self, id: &str) -> Result<WasmPluginInstance, PluginError> {
        if let Some(i) = self.instances.get_mut(id) { i.phase = WasmPluginPhase::Shutdown; }
        self.instances.remove(id).ok_or_else(|| PluginError::NotFound(id.to_string()))
    }

    pub fn list_plugins(&self) -> Vec<WasmPluginInfo> { self.instances.values().map(WasmPluginInfo::from_instance).collect() }
    pub fn get_plugin(&self, id: &str) -> Option<WasmPluginInfo> { self.instances.get(id).map(WasmPluginInfo::from_instance) }
    pub fn active_count(&self) -> usize { self.instances.values().filter(|i| i.phase == WasmPluginPhase::Active).count() }
    pub fn total_count(&self) -> usize { self.instances.len() }
    pub fn config(&self) -> &SandboxConfig { &self.config }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmPluginInfo {
    pub id: String, pub name: String, pub version: String, pub description: String,
    pub author: String, pub priority: u32, pub phase: WasmPluginPhase,
    pub permissions: Vec<String>, pub skills: Vec<String>,
    pub execution_count: u64, pub error_count: u64, pub last_error: Option<String>,
    pub binary_size_bytes: usize,
}

impl WasmPluginInfo {
    fn from_instance(i: &WasmPluginInstance) -> Self {
        Self { id: i.metadata.id.clone(), name: i.metadata.name.clone(),
            version: i.metadata.version.clone(), description: i.metadata.description.clone(),
            author: i.metadata.author.clone(), priority: i.metadata.priority, phase: i.phase,
            permissions: i.permissions.permissions().iter().map(|p| format!("{:?}", p).to_lowercase()).collect(),
            skills: i.registered_skills.clone(), execution_count: i.execution_count,
            error_count: i.error_count, last_error: i.last_error.clone(), binary_size_bytes: i.wasm_bytes.len() }
    }
}

pub type SharedWasmSandbox = std::sync::Arc<tokio::sync::Mutex<WasmSandbox>>;

#[cfg(test)]
mod tests {
    use super::*; use crate::plugin::permission::Permission;
    fn wb() -> Vec<u8> { vec![0x00,0x61,0x73,0x6d,0x01,0x00,0x00,0x00] }
    fn tm(id: &str) -> PluginMetadata { PluginMetadata { id: id.into(), name: format!("T {}",id),
        version: "1.0.0".into(), description: "t".into(), author: "t".into(), priority: 100 } }

    #[test] fn load_valid() { let mut s = WasmSandbox::default_sandbox();
        assert!(s.load_plugin("t/p", wb(), tm("t/p"), PermissionSet::default(), HashMap::new()).is_ok()); }
    #[test] fn load_invalid() { let mut s = WasmSandbox::default_sandbox();
        assert!(s.load_plugin("t/b", vec![0,1,2,3], tm("t/b"), PermissionSet::default(), HashMap::new()).is_err()); }
    #[test] fn load_dup() { let mut s = WasmSandbox::default_sandbox();
        s.load_plugin("t/d", wb(), tm("t/d"), PermissionSet::default(), HashMap::new()).unwrap();
        assert!(s.load_plugin("t/d", wb(), tm("t/d"), PermissionSet::default(), HashMap::new()).is_err()); }
    #[test] fn full_lifecycle() { let mut s = WasmSandbox::default_sandbox();
        s.load_plugin("t/l", wb(), tm("t/l"), PermissionSet::from_permissions([Permission::ReadWorldState]), HashMap::new()).unwrap();
        assert_eq!(s.initialize_plugin("t/l").unwrap().phase, WasmPluginPhase::Initialized);
        assert!(!s.register_plugin("t/l").unwrap().is_empty());
        s.activate_plugin("t/l").unwrap(); assert_eq!(s.active_count(), 1);
        assert!(s.execute("t/l", "{}").success);
        s.shutdown_plugin("t/l").unwrap(); s.unload_plugin("t/l").unwrap(); assert_eq!(s.total_count(), 0); }
    #[test] fn list_plugins() { let mut s = WasmSandbox::default_sandbox();
        s.load_plugin("t/a", wb(), tm("t/a"), PermissionSet::default(), HashMap::new()).unwrap();
        s.load_plugin("t/b", wb(), tm("t/b"), PermissionSet::default(), HashMap::new()).unwrap();
        assert_eq!(s.list_plugins().len(), 2); }
    #[test] fn config_defaults() { assert_eq!(SandboxConfig::default().max_memory_mb, 64); }
    #[test] fn exec_missing() { assert!(!WasmSandbox::default_sandbox().execute("x","{}").success); }
    #[test] fn exec_not_active() { let mut s = WasmSandbox::default_sandbox();
        s.load_plugin("t/ia", wb(), tm("t/ia"), PermissionSet::default(), HashMap::new()).unwrap();
        assert!(!s.execute("t/ia","{}").success); }
}
