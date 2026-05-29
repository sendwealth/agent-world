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
use crate::plugin::{PluginInfo, PluginManager};

// ── Response types ─────────────────────────────────────────────

#[derive(Debug, Serialize)]
struct PluginListResponse {
    plugins: Vec<PluginInfo>,
    total: usize,
    active: usize,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
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
        .route("/api/v1/plugins", get(list_plugins))
        .route("/api/v1/plugins/stats", get(plugin_stats))
        .route("/api/v1/plugins/:id", get(get_plugin))
        .route("/api/v1/plugins/:id/enable", post(enable_plugin))
        .route("/api/v1/plugins/:id/disable", post(disable_plugin))
        .route("/api/v1/plugins/:id/unload", post(unload_plugin))
}

// ── Helpers ────────────────────────────────────────────────────

fn get_manager(state: &AppState) -> Option<Arc<Mutex<PluginManager>>> {
    state.plugin_manager.clone()
}
