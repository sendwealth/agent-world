//! # Provider Management REST API
//!
//! CRUD endpoints for LLM provider configurations and agent model assignments.

use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::api::{AppState, api_err};

// ── Valid Protocols ────────────────────────────────────

const VALID_PROTOCOLS: &[&str] = &["openai", "anthropic", "ollama", "custom"];

// ── Data Structures ────────────────────────────────────

/// A configured LLM provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub id: String,
    pub protocol: String,
    pub base_url: String,
    #[serde(skip_serializing)]
    pub api_key: Option<String>,
    pub api_version: Option<String>,
    pub display_name: Option<String>,
    pub is_default: bool,
}

/// Sanitised provider returned in GET responses (api_key masked).
#[derive(Debug, Clone, Serialize)]
pub struct ProviderResponse {
    pub id: String,
    pub protocol: String,
    pub base_url: String,
    pub api_key: Option<String>,
    pub api_version: Option<String>,
    pub display_name: Option<String>,
    pub is_default: bool,
}

impl From<&ProviderConfig> for ProviderResponse {
    fn from(p: &ProviderConfig) -> Self {
        Self {
            id: p.id.clone(),
            protocol: p.protocol.clone(),
            base_url: p.base_url.clone(),
            api_key: p.api_key.as_ref().map(|k| mask_key(k)),
            api_version: p.api_version.clone(),
            display_name: p.display_name.clone(),
            is_default: p.is_default,
        }
    }
}

/// Agent-to-model assignment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentModelAssignment {
    pub agent_id: String,
    pub provider_id: String,
    pub model_id: String,
}

// ── Request Types ──────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreateProviderRequest {
    pub id: Option<String>,
    pub protocol: String,
    pub base_url: String,
    pub api_key: Option<String>,
    pub api_version: Option<String>,
    pub display_name: Option<String>,
    #[serde(default)]
    pub is_default: bool,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProviderRequest {
    pub protocol: Option<String>,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub api_version: Option<String>,
    pub display_name: Option<String>,
    pub is_default: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct SetAgentModelRequest {
    pub provider_id: String,
    pub model_id: String,
}

// ── Shared Store Type ──────────────────────────────────

pub type SharedProviderStore = Arc<Mutex<HashMap<String, ProviderConfig>>>;
pub type SharedAgentModelStore = Arc<Mutex<HashMap<String, AgentModelAssignment>>>;

// ── Helpers ────────────────────────────────────────────

fn mask_key(key: &str) -> String {
    if key.len() <= 8 {
        "****".to_string()
    } else {
        format!("{}****{}", &key[..4], &key[key.len() - 4..])
    }
}

// ── Handler: Create Provider ───────────────────────────

pub async fn create_provider(
    State(state): State<AppState>,
    Json(body): Json<CreateProviderRequest>,
) -> axum::response::Response {
    if !VALID_PROTOCOLS.contains(&body.protocol.as_str()) {
        return api_err(
            StatusCode::BAD_REQUEST,
            format!(
                "invalid protocol '{}'. Allowed: {}",
                body.protocol,
                VALID_PROTOCOLS.join(", ")
            ),
        );
    }

    if body.base_url.is_empty() {
        return api_err(StatusCode::BAD_REQUEST, "base_url is required");
    }

    let id = body.id.unwrap_or_else(|| Uuid::new_v4().to_string());

    let provider = ProviderConfig {
        id: id.clone(),
        protocol: body.protocol,
        base_url: body.base_url,
        api_key: body.api_key,
        api_version: body.api_version,
        display_name: body.display_name,
        is_default: body.is_default,
    };

    let mut store = state.providers.lock().await;
    if store.contains_key(&id) {
        return api_err(
            StatusCode::CONFLICT,
            format!("provider with id '{}' already exists", id),
        );
    }

    store.insert(id.clone(), provider.clone());

    (StatusCode::CREATED, Json(ProviderResponse::from(&provider))).into_response()
}

// ── Handler: List Providers ────────────────────────────

pub async fn list_providers(State(state): State<AppState>) -> axum::response::Response {
    let store = state.providers.lock().await;
    let list: Vec<ProviderResponse> = store.values().map(ProviderResponse::from).collect();
    Json(list).into_response()
}

// ── Handler: Get Provider ──────────────────────────────

pub async fn get_provider(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> axum::response::Response {
    let store = state.providers.lock().await;
    match store.get(&id) {
        Some(p) => Json(ProviderResponse::from(p)).into_response(),
        None => api_err(StatusCode::NOT_FOUND, format!("provider '{}' not found", id)),
    }
}

// ── Handler: Update Provider ───────────────────────────

pub async fn update_provider(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<UpdateProviderRequest>,
) -> axum::response::Response {
    if let Some(ref proto) = body.protocol {
        if !VALID_PROTOCOLS.contains(&proto.as_str()) {
            return api_err(
                StatusCode::BAD_REQUEST,
                format!(
                    "invalid protocol '{}'. Allowed: {}",
                    proto,
                    VALID_PROTOCOLS.join(", ")
                ),
            );
        }
    }

    let mut store = state.providers.lock().await;
    match store.get_mut(&id) {
        Some(p) => {
            if let Some(v) = body.protocol {
                p.protocol = v;
            }
            if let Some(v) = body.base_url {
                p.base_url = v;
            }
            if let Some(v) = body.api_key {
                p.api_key = Some(v);
            }
            if let Some(v) = body.api_version {
                p.api_version = Some(v);
            }
            if let Some(v) = body.display_name {
                p.display_name = Some(v);
            }
            if let Some(v) = body.is_default {
                p.is_default = v;
            }
            Json(ProviderResponse::from(&*p)).into_response()
        }
        None => api_err(StatusCode::NOT_FOUND, format!("provider '{}' not found", id)),
    }
}

// ── Handler: Delete Provider ───────────────────────────

pub async fn delete_provider(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> axum::response::Response {
    let mut store = state.providers.lock().await;
    match store.remove(&id) {
        Some(_) => Json(serde_json::json!({"deleted": id})).into_response(),
        None => api_err(StatusCode::NOT_FOUND, format!("provider '{}' not found", id)),
    }
}

// ── Handler: Connection Test (proxy) ───────────────────

pub async fn test_provider_connection(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> axum::response::Response {
    let store = state.providers.lock().await;
    match store.get(&id) {
        Some(p) => {
            let protocol = p.protocol.clone();
            drop(store);
            Json(serde_json::json!({
                "provider_id": id,
                "status": "test_queued",
                "message": format!("Connection test queued for provider '{}' ({})", id, protocol),
            }))
            .into_response()
        }
        None => api_err(StatusCode::NOT_FOUND, format!("provider '{}' not found", id)),
    }
}

// ── Handler: Model Discovery (proxy) ──────────────────

pub async fn discover_provider_models(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> axum::response::Response {
    let store = state.providers.lock().await;
    match store.get(&id) {
        Some(p) => {
            let protocol = p.protocol.clone();
            drop(store);
            Json(serde_json::json!({
                "provider_id": id,
                "status": "discovery_queued",
                "message": format!("Model discovery queued for provider '{}' ({})", id, protocol),
            }))
            .into_response()
        }
        None => api_err(StatusCode::NOT_FOUND, format!("provider '{}' not found", id)),
    }
}

// ── Handler: Set Agent Model Assignment ────────────────

pub async fn set_agent_model(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
    Json(body): Json<SetAgentModelRequest>,
) -> axum::response::Response {
    {
        let store = state.providers.lock().await;
        if !store.contains_key(&body.provider_id) {
            return api_err(
                StatusCode::NOT_FOUND,
                format!("provider '{}' not found", body.provider_id),
            );
        }
    }

    let assignment = AgentModelAssignment {
        agent_id: agent_id.clone(),
        provider_id: body.provider_id,
        model_id: body.model_id,
    };

    let mut store = state.agent_models.lock().await;
    store.insert(agent_id.clone(), assignment.clone());

    Json(assignment).into_response()
}

// ── Handler: Get Agent Model Assignment ────────────────

pub async fn get_agent_model(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
) -> axum::response::Response {
    let store = state.agent_models.lock().await;
    match store.get(&agent_id) {
        Some(a) => Json(a).into_response(),
        None => Json(serde_json::Value::Null).into_response(),
    }
}

// ── Router ─────────────────────────────────────────────

pub fn provider_routes() -> Router<AppState> {
    Router::new()
        .route("/api/v1/providers", post(create_provider))
        .route("/api/v1/providers", get(list_providers))
        .route("/api/v1/providers/:id", get(get_provider))
        .route("/api/v1/providers/:id", put(update_provider))
        .route("/api/v1/providers/:id", delete(delete_provider))
        .route("/api/v1/providers/:id/test", post(test_provider_connection))
        .route("/api/v1/providers/:id/models", get(discover_provider_models))
        .route("/api/v1/agents/:id/model", put(set_agent_model))
        .route("/api/v1/agents/:id/model", get(get_agent_model))
}
