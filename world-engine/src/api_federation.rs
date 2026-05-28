use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::*,
    Json,
};
use uuid::Uuid;

use crate::api::AppState;
use crate::federation::migration::{AgentSnapshot, MigrationStatus as MigStatus};
use crate::federation::registry::{WorldEndpoint, WorldEntry, WorldMetrics};
use crate::federation::service::{RestMigrationReview, RestMigrationSubmit, RestWorldRegister};

/// Helper: wrap success response in { data, error: null, request_id } format.
pub fn api_ok(data: impl serde::Serialize) -> axum::response::Response {
    let request_id = Uuid::new_v4().to_string();
    Json(serde_json::json!({
        "data": data,
        "error": null,
        "request_id": request_id,
    }))
    .into_response()
}

/// Helper: wrap error response in { data: null, error, request_id } format.
pub fn api_err(status: StatusCode, error: impl Into<String>) -> axum::response::Response {
    let request_id = Uuid::new_v4().to_string();
    (
        status,
        Json(serde_json::json!({
            "data": null,
            "error": error.into(),
            "request_id": request_id,
        })),
    )
        .into_response()
}

/// POST /api/v1/federation/worlds — Register a new world.
#[allow(dead_code)]
pub async fn federation_register_world(
    State(state): State<AppState>,
    Json(body): Json<RestWorldRegister>,
) -> impl IntoResponse {
    let registry = match &state.federation_registry {
        Some(r) => r.clone(),
        None => {
            return api_err(
                StatusCode::SERVICE_UNAVAILABLE,
                "federation registry not configured",
            )
        }
    };
    let entry = WorldEntry {
        world_id: body.world_id,
        name: body.name,
        description: body.description,
        endpoint: WorldEndpoint {
            host: body.host,
            grpc_port: body.grpc_port,
            http_port: body.http_port,
        },
        status: crate::federation::WorldStatus::Online,
        capabilities: body.capabilities,
        max_agents: body.max_agents,
        current_agents: 0,
        labels: body.labels,
        metrics: WorldMetrics::default(),
        registered_at: chrono::Utc::now().to_rfc3339(),
        last_heartbeat: chrono::Utc::now().to_rfc3339(),
    };
    let reg = registry.lock().await;
    match reg.register(entry).await {
        Ok(is_new) => api_ok(serde_json::json!({ "registered": true, "is_new": is_new })),
        Err(e) => api_err(StatusCode::BAD_REQUEST, e),
    }
}

/// GET /api/v1/federation/worlds — List all registered worlds.
#[allow(dead_code)]
pub async fn federation_list_worlds(State(state): State<AppState>) -> impl IntoResponse {
    let registry = match &state.federation_registry {
        Some(r) => r.clone(),
        None => {
            return api_err(
                StatusCode::SERVICE_UNAVAILABLE,
                "federation registry not configured",
            )
        }
    };
    let reg = registry.lock().await;
    let worlds = reg.list_all().await;
    api_ok(&worlds)
}

/// GET /api/v1/federation/worlds/:world_id — Get a specific world.
#[allow(dead_code)]
pub async fn federation_get_world(
    State(state): State<AppState>,
    Path(world_id): Path<String>,
) -> impl IntoResponse {
    let registry = match &state.federation_registry {
        Some(r) => r.clone(),
        None => {
            return api_err(
                StatusCode::SERVICE_UNAVAILABLE,
                "federation registry not configured",
            )
        }
    };
    let reg = registry.lock().await;
    match reg.get_world(&world_id).await {
        Some(entry) => api_ok(&entry),
        None => api_err(StatusCode::NOT_FOUND, "world not found"),
    }
}

/// DELETE /api/v1/federation/worlds/:world_id — Deregister a world.
#[allow(dead_code)]
pub async fn federation_deregister_world(
    State(state): State<AppState>,
    Path(world_id): Path<String>,
) -> impl IntoResponse {
    let registry = match &state.federation_registry {
        Some(r) => r.clone(),
        None => {
            return api_err(
                StatusCode::SERVICE_UNAVAILABLE,
                "federation registry not configured",
            )
        }
    };
    let reg = registry.lock().await;
    let removed = reg.deregister(&world_id).await;
    api_ok(serde_json::json!({ "removed": removed }))
}

/// POST /api/v1/federation/worlds/:world_id/heartbeat — Record a heartbeat.
pub async fn federation_heartbeat(
    State(state): State<AppState>,
    Path(world_id): Path<String>,
    Json(metrics): Json<WorldMetrics>,
) -> impl IntoResponse {
    let registry = match &state.federation_registry {
        Some(r) => r.clone(),
        None => {
            return api_err(
                StatusCode::SERVICE_UNAVAILABLE,
                "federation registry not configured",
            )
        }
    };
    let reg = registry.lock().await;
    let ok = reg.heartbeat(&world_id, metrics).await;
    if ok {
        api_ok(serde_json::json!({ "ok": true }))
    } else {
        api_err(StatusCode::NOT_FOUND, "world not found")
    }
}

/// POST /api/v1/migration/submit — Submit a migration application.
pub async fn migration_submit(
    State(state): State<AppState>,
    Json(body): Json<RestMigrationSubmit>,
) -> impl IntoResponse {
    let manager = match &state.migration_manager {
        Some(m) => m.clone(),
        None => {
            return api_err(
                StatusCode::SERVICE_UNAVAILABLE,
                "migration manager not configured",
            )
        }
    };
    let snapshot = AgentSnapshot {
        agent_id: body.agent_id,
        name: body.name,
        phase: body.phase,
        tokens: body.tokens,
        money: body.money,
        reputation: body.reputation,
        skills: body.skills,
        metadata: std::collections::HashMap::new(),
        source_world_id: body.source_world_id,
        memory_data: Vec::new(),
        public_key: body.public_key,
    };
    let mgr = manager.lock().await;
    match mgr.submit(snapshot, body.target_world_id).await {
        Ok(app) => api_ok(&app),
        Err(e) => api_err(StatusCode::BAD_REQUEST, e),
    }
}

/// POST /api/v1/migration/:migration_id/review — Review a migration.
pub async fn migration_review(
    State(state): State<AppState>,
    Path(migration_id): Path<String>,
    Json(body): Json<RestMigrationReview>,
) -> impl IntoResponse {
    let manager = match &state.migration_manager {
        Some(m) => m.clone(),
        None => {
            return api_err(
                StatusCode::SERVICE_UNAVAILABLE,
                "migration manager not configured",
            )
        }
    };
    let mgr = manager.lock().await;
    match mgr
        .review(
            &migration_id,
            body.approved,
            &body.reviewer_world_id,
            body.rejection_reason,
        )
        .await
    {
        Ok(app) => api_ok(&app),
        Err(e) => api_err(StatusCode::BAD_REQUEST, e),
    }
}

/// POST /api/v1/migration/:migration_id/execute — Execute a migration.
pub async fn migration_execute(
    State(state): State<AppState>,
    Path(migration_id): Path<String>,
) -> impl IntoResponse {
    let manager = match &state.migration_manager {
        Some(m) => m.clone(),
        None => {
            return api_err(
                StatusCode::SERVICE_UNAVAILABLE,
                "migration manager not configured",
            )
        }
    };
    let mgr = manager.lock().await;
    match mgr.execute_standalone(&migration_id).await {
        Ok(app) => api_ok(&app),
        Err(e) => api_err(StatusCode::BAD_REQUEST, e),
    }
}

/// POST /api/v1/migration/:migration_id/cancel — Cancel a migration.
pub async fn migration_cancel(
    State(state): State<AppState>,
    Path(migration_id): Path<String>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let manager = match &state.migration_manager {
        Some(m) => m.clone(),
        None => {
            return api_err(
                StatusCode::SERVICE_UNAVAILABLE,
                "migration manager not configured",
            )
        }
    };
    let cancelled_by = body
        .get("cancelled_by")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let reason = body
        .get("reason")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let mgr = manager.lock().await;
    match mgr.cancel(&migration_id, &cancelled_by, reason).await {
        Ok(app) => api_ok(&app),
        Err(e) => api_err(StatusCode::BAD_REQUEST, e),
    }
}

/// GET /api/v1/migration/:migration_id — Get migration status.
pub async fn migration_get_status(
    State(state): State<AppState>,
    Path(migration_id): Path<String>,
) -> impl IntoResponse {
    let manager = match &state.migration_manager {
        Some(m) => m.clone(),
        None => {
            return api_err(
                StatusCode::SERVICE_UNAVAILABLE,
                "migration manager not configured",
            )
        }
    };
    let mgr = manager.lock().await;
    match mgr.get(&migration_id).await {
        Some(app) => api_ok(&app),
        None => api_err(StatusCode::NOT_FOUND, "migration not found"),
    }
}

/// POST /api/v1/migration/list — List migrations with optional filters.
pub async fn migration_list(
    State(state): State<AppState>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let manager = match &state.migration_manager {
        Some(m) => m.clone(),
        None => {
            return api_err(
                StatusCode::SERVICE_UNAVAILABLE,
                "migration manager not configured",
            )
        }
    };
    let world_id: Option<String> = body
        .get("world_id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let inbound = body
        .get("inbound")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let status_filter: Option<MigStatus> = body
        .get("status_filter")
        .and_then(|v| v.as_str())
        .and_then(|s| match s {
            "pending" => Some(MigStatus::Pending),
            "approved" => Some(MigStatus::Approved),
            "rejected" => Some(MigStatus::Rejected),
            "executing" => Some(MigStatus::Executing),
            "completed" => Some(MigStatus::Completed),
            "cancelled" => Some(MigStatus::Cancelled),
            "failed" => Some(MigStatus::Failed),
            _ => None,
        });
    let limit = body.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as u32;
    let offset = body.get("offset").and_then(|v| v.as_u64()).unwrap_or(0) as u32;

    let mgr = manager.lock().await;
    let world_id_ref = world_id.as_deref();
    let results = mgr
        .list(world_id_ref, inbound, status_filter, limit, offset)
        .await;
    api_ok(&results)
}

/// GET /api/v1/migration/policy — Get the current migration policy.
pub async fn migration_get_policy(State(state): State<AppState>) -> impl IntoResponse {
    let manager = match &state.migration_manager {
        Some(m) => m.clone(),
        None => {
            return api_err(
                StatusCode::SERVICE_UNAVAILABLE,
                "migration manager not configured",
            )
        }
    };
    let mgr = manager.lock().await;
    let policy = mgr.get_policy().await;
    let rest_policy = crate::federation::service::RestMigrationPolicy::from(policy);
    api_ok(&rest_policy)
}

/// PUT /api/v1/migration/policy — Update migration policy.
pub async fn migration_update_policy(
    State(state): State<AppState>,
    Json(body): Json<crate::federation::service::RestMigrationPolicy>,
) -> impl IntoResponse {
    let manager = match &state.migration_manager {
        Some(m) => m.clone(),
        None => {
            return api_err(
                StatusCode::SERVICE_UNAVAILABLE,
                "migration manager not configured",
            )
        }
    };
    let mgr = manager.lock().await;
    let policy: crate::federation::MigrationPolicy = body.into();
    mgr.set_policy(policy).await;
    let updated = mgr.get_policy().await;
    let rest_policy = crate::federation::service::RestMigrationPolicy::from(updated);
    api_ok(&rest_policy)
}

/// GET /api/v1/migration/stats — Get migration statistics.
pub async fn migration_stats(State(state): State<AppState>) -> impl IntoResponse {
    let manager = match &state.migration_manager {
        Some(m) => m.clone(),
        None => {
            return api_err(
                StatusCode::SERVICE_UNAVAILABLE,
                "migration manager not configured",
            )
        }
    };
    let mgr = manager.lock().await;
    let stats = mgr.stats().await;
    api_ok(&stats)
}

/// GET /api/v1/agents/:id/immigration-status — Get agent immigration status.
pub async fn agent_immigration_status(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
) -> impl IntoResponse {
    let manager = match &state.migration_manager {
        Some(m) => m.clone(),
        None => {
            return api_err(
                StatusCode::SERVICE_UNAVAILABLE,
                "migration manager not configured",
            )
        }
    };
    let mgr = manager.lock().await;
    match mgr.get_agent_status(&agent_id).await {
        Some(app) => api_ok(&app),
        None => api_ok(
            serde_json::json!({ "agent_id": agent_id, "status": "none", "message": "no migration applications found" }),
        ),
    }
}

/// Federation + migration REST routes.
/// Note: federation/* diplomacy routes are registered in api_diplomacy::diplomacy_routes().
pub fn federation_routes() -> axum::Router<AppState> {
    axum::Router::new()
        .route(
            "/api/v1/federation/worlds/:world_id/heartbeat",
            post(federation_heartbeat),
        )
        .route("/api/v1/migration/submit", post(migration_submit))
        .route(
            "/api/v1/migration/:migration_id/review",
            post(migration_review),
        )
        .route(
            "/api/v1/migration/:migration_id/execute",
            post(migration_execute),
        )
        .route(
            "/api/v1/migration/:migration_id/cancel",
            post(migration_cancel),
        )
        .route("/api/v1/migration/:migration_id", get(migration_get_status))
        .route("/api/v1/migration/list", post(migration_list))
        .route("/api/v1/migration/policy", get(migration_get_policy))
        .route("/api/v1/migration/policy", put(migration_update_policy))
        .route("/api/v1/migration/stats", get(migration_stats))
        .route(
            "/api/v1/agents/:id/immigration-status",
            get(agent_immigration_status),
        )
}
