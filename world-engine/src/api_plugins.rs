//! Plugin management API endpoints.
//!
//! REST routes for listing, enabling, disabling, and querying plugins.

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::api::{api_err, api_ok, AppState};
use crate::plugin::{PluginInfo, PluginManager, PluginMetadata, PermissionSet, WasmSandbox};

// ── Response types ─────────────────────────────────────────────

#[derive(Debug, Serialize)]
struct PluginListResponse {
    plugins: Vec<PluginInfo>,
    total: usize,
    active: usize,
}

#[derive(Debug, Deserialize)]
pub struct RegisterPluginRequest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    #[serde(default = "default_priority")]
    pub priority: u32,
    pub permissions: Vec<String>,
}

fn default_priority() -> u32 {
    100
}

#[derive(Debug, Serialize)]
struct PluginActionResponse {
    id: String,
    action: String,
    success: bool,
    message: String,
}

// ── Handlers ───────────────────────────────────────────────────

/// GET /api/v1/plugins — list all registered plugins.
pub async fn list_plugins(State(state): State<AppState>) -> axum::response::Response {
    let mgr = match get_manager(&state) {
        Some(m) => m,
        None => return api_err(StatusCode::NOT_FOUND, "plugin system not initialized"),
    };
    let mgr = mgr.lock().await;
    let plugins = mgr.list_plugins();
    let total = mgr.total_count();
    let active = mgr.active_count();
    api_ok(PluginListResponse {
        plugins,
        total,
        active,
    })
}

/// GET /api/v1/plugins/:id — get a specific plugin's info.
pub async fn get_plugin(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> axum::response::Response {
    let mgr = match get_manager(&state) {
        Some(m) => m,
        None => return api_err(StatusCode::NOT_FOUND, "plugin system not initialized"),
    };
    let mgr = mgr.lock().await;
    match mgr.get_plugin(&id) {
        Some(info) => api_ok(info),
        None => api_err(StatusCode::NOT_FOUND, format!("plugin not found: {}", id)),
    }
}

/// POST /api/v1/plugins/:id/enable — enable a disabled plugin.
pub async fn enable_plugin(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> axum::response::Response {
    let mgr = match get_manager(&state) {
        Some(m) => m,
        None => return api_err(StatusCode::NOT_FOUND, "plugin system not initialized"),
    };
    let mut mgr = mgr.lock().await;
    match mgr.enable_plugin(&id) {
        Ok(()) => api_ok(PluginActionResponse {
            id,
            action: "enable".to_string(),
            success: true,
            message: "plugin enabled".to_string(),
        }),
        Err(e) => api_err(
            StatusCode::BAD_REQUEST,
            format!("failed to enable plugin: {}", e),
        ),
    }
}

/// POST /api/v1/plugins/:id/disable — disable an active plugin.
pub async fn disable_plugin(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> axum::response::Response {
    let mgr = match get_manager(&state) {
        Some(m) => m,
        None => return api_err(StatusCode::NOT_FOUND, "plugin system not initialized"),
    };
    let mut mgr = mgr.lock().await;
    match mgr.disable_plugin(&id) {
        Ok(()) => api_ok(PluginActionResponse {
            id,
            action: "disable".to_string(),
            success: true,
            message: "plugin disabled".to_string(),
        }),
        Err(e) => api_err(
            StatusCode::BAD_REQUEST,
            format!("failed to disable plugin: {}", e),
        ),
    }
}

/// POST /api/v1/plugins/:id/unload — unload and remove a plugin.
pub async fn unload_plugin(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> axum::response::Response {
    let mgr = match get_manager(&state) {
        Some(m) => m,
        None => return api_err(StatusCode::NOT_FOUND, "plugin system not initialized"),
    };
    let mut mgr = mgr.lock().await;
    match mgr.unregister_plugin(&id) {
        Ok(_) => api_ok(PluginActionResponse {
            id,
            action: "unload".to_string(),
            success: true,
            message: "plugin unloaded".to_string(),
        }),
        Err(e) => api_err(
            StatusCode::BAD_REQUEST,
            format!("failed to unload plugin: {}", e),
        ),
    }
}

/// GET /api/v1/plugins/stats — plugin system statistics.
pub async fn plugin_stats(State(state): State<AppState>) -> axum::response::Response {
    let mgr = match get_manager(&state) {
        Some(m) => m,
        None => return api_err(StatusCode::NOT_FOUND, "plugin system not initialized"),
    };
    let mgr = mgr.lock().await;
    api_ok(serde_json::json!({
        "total_plugins": mgr.total_count(),
        "active_plugins": mgr.active_count(),
    }))
}

// ── Router ─────────────────────────────────────────────────────

/// Build the plugin API sub-router.
pub fn plugin_routes() -> Router<AppState> {
    Router::new()
        .route("/plugins", get(list_plugins))
        .route("/plugins/stats", get(plugin_stats))
        .route("/plugins/register", post(register_plugin))
        .route("/plugins/:id", get(get_plugin))
        .route("/plugins/:id/enable", post(enable_plugin))
        .route("/plugins/:id/disable", post(disable_plugin))
        .route("/plugins/:id/unload", post(unload_plugin))
        .route("/plugins/sandbox", get(sandbox_list))
        .route("/plugins/sandbox/load", post(sandbox_load))
        .route("/plugins/sandbox/:id/init", post(sandbox_init))
        .route("/plugins/sandbox/:id/execute", post(sandbox_execute))
        .route("/plugins/sandbox/:id/shutdown", post(sandbox_shutdown))
}

// ── Public Registration ──────────────────────────────────────────

/// POST /api/v1/plugins/register — Register a new third-party plugin.
///
/// This endpoint accepts plugin metadata and returns a registration
/// confirmation. The plugin can then be loaded into the WASM sandbox.
pub async fn register_plugin(
    State(_state): State<AppState>,
    axum::Json(req): axum::Json<RegisterPluginRequest>,
) -> axum::response::Response {
    // Validate required fields
    if req.id.is_empty() || req.name.is_empty() {
        return api_err(StatusCode::BAD_REQUEST, "id and name are required");
    }

    // Build permissions from request
    let mut perms = PermissionSet::new();
    for p in &req.permissions {
        match p.as_str() {
            "read_agents" => perms.grant(crate::plugin::Permission::ReadAgents),
            "read_world_state" => perms.grant(crate::plugin::Permission::ReadWorldState),
            "read_events" => perms.grant(crate::plugin::Permission::ReadEvents),
            "write_agent_tokens" => perms.grant(crate::plugin::Permission::WriteAgentTokens),
            "write_agent_phase" => perms.grant(crate::plugin::Permission::WriteAgentPhase),
            "write_agent_skills" => perms.grant(crate::plugin::Permission::WriteAgentSkills),
            "emit_events" => perms.grant(crate::plugin::Permission::EmitEvents),
            "intercept_actions" => perms.grant(crate::plugin::Permission::InterceptActions),
            "intercept_transactions" => perms.grant(crate::plugin::Permission::InterceptTransactions),
            "tick_subsystem" => perms.grant(crate::plugin::Permission::TickSubsystem),
            "admin_access" => perms.grant(crate::plugin::Permission::AdminAccess),
            _ => {} // ignore unknown permissions
        }
    }

    let metadata = PluginMetadata {
        id: req.id.clone(),
        name: req.name.clone(),
        version: req.version.clone(),
        description: req.description.clone(),
        author: req.author.clone(),
        priority: req.priority,
    };

    // Store in a global registry for later WASM loading
    // For now, return the registration info
    api_ok(serde_json::json!({
        "id": metadata.id,
        "name": metadata.name,
        "version": metadata.version,
        "status": "registered",
        "permissions": req.permissions,
        "message": "Plugin registered. Upload WASM binary to /api/v1/plugins/sandbox/load to activate."
    }))
}

// ── WASM Sandbox Endpoints ──────────────────────────────────────

#[derive(Debug, Serialize)]
struct SandboxListResponse {
    plugins: Vec<serde_json::Value>,
    total: usize,
    active: usize,
}

/// GET /api/v1/plugins/sandbox — list WASM sandbox plugins.
pub async fn sandbox_list(State(_state): State<AppState>) -> axum::response::Response {
    // In production, this would query the SharedWasmSandbox from AppState
    api_ok(SandboxListResponse {
        plugins: Vec::new(),
        total: 0,
        active: 0,
    })
}

#[derive(Debug, Deserialize)]
#[allow(dead_code, private_interfaces)]
pub struct SandboxLoadRequest {
    plugin_id: String,
    wasm_base64: String,
}

/// POST /api/v1/plugins/sandbox/load — load a WASM plugin into sandbox.
pub async fn sandbox_load(
    State(_state): State<AppState>,
    axum::Json(req): axum::Json<SandboxLoadRequest>,
) -> axum::response::Response {
    use base64::Engine;
    let wasm_bytes = match base64::engine::general_purpose::STANDARD.decode(&req.wasm_base64) {
        Ok(b) => b,
        Err(e) => return api_err(StatusCode::BAD_REQUEST, format!("Invalid base64: {}", e)),
    };

    let mut sandbox = WasmSandbox::default_sandbox();
    let metadata = PluginMetadata {
        id: req.plugin_id.clone(),
        name: req.plugin_id.clone(),
        version: "1.0.0".to_string(),
        description: "Uploaded plugin".to_string(),
        author: "external".to_string(),
        priority: 100,
    };

    match sandbox.load_plugin(&req.plugin_id, wasm_bytes, metadata, PermissionSet::default(), std::collections::HashMap::new()) {
        Ok(()) => api_ok(serde_json::json!({
            "id": req.plugin_id,
            "status": "loaded",
            "message": "Plugin loaded into WASM sandbox. POST to /init to initialize."
        })),
        Err(e) => api_err(StatusCode::BAD_REQUEST, format!("Load failed: {}", e)),
    }
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct SandboxIdRequest {
    context: Option<serde_json::Value>,
}

/// POST /api/v1/plugins/sandbox/:id/init — initialize a loaded WASM plugin.
pub async fn sandbox_init(
    State(_state): State<AppState>,
    Path(id): Path<String>,
) -> axum::response::Response {
    let mut sandbox = WasmSandbox::default_sandbox();
    match sandbox.initialize_plugin(&id) {
        Ok(info) => api_ok(serde_json::json!({
            "id": info.id, "phase": format!("{:?}", info.phase), "name": info.name
        })),
        Err(e) => api_err(StatusCode::BAD_REQUEST, format!("Init failed: {}", e)),
    }
}

/// POST /api/v1/plugins/sandbox/:id/execute — execute a WASM plugin.
pub async fn sandbox_execute(
    State(_state): State<AppState>,
    Path(id): Path<String>,
    axum::Json(body): axum::Json<Option<serde_json::Value>>,
) -> axum::response::Response {
    let ctx = body.map(|v| v.to_string()).unwrap_or_default();
    let mut sandbox = WasmSandbox::default_sandbox();
    let result = sandbox.execute(&id, &ctx);
    api_ok(serde_json::json!({
        "success": result.success,
        "payload": result.payload,
        "error": result.error,
        "execution_time_ms": result.execution_time_ms,
    }))
}

/// POST /api/v1/plugins/sandbox/:id/shutdown — shutdown a WASM plugin.
pub async fn sandbox_shutdown(
    State(_state): State<AppState>,
    Path(id): Path<String>,
) -> axum::response::Response {
    let mut sandbox = WasmSandbox::default_sandbox();
    match sandbox.shutdown_plugin(&id) {
        Ok(()) => api_ok(serde_json::json!({"id": id, "status": "shutdown"})),
        Err(e) => api_err(StatusCode::BAD_REQUEST, format!("Shutdown failed: {}", e)),
    }
}

// ── Helpers ────────────────────────────────────────────────────

fn get_manager(state: &AppState) -> Option<Arc<Mutex<PluginManager>>> {
    state.plugin_manager.clone()
}
