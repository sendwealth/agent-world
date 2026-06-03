use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::*,
    Json,
};
use serde::Deserialize;

use crate::api::AppState;
use crate::economy::inheritance::Beneficiary;

// -- Request Types --

#[derive(Debug, Deserialize)]
pub struct CreateWillRequest {
    pub testator_id: String,
    pub beneficiaries: Vec<Beneficiary>,
    pub tick: u64,
}

// -- API Handlers --

pub async fn inheritance_create_will(
    State(state): State<AppState>,
    Json(req): Json<CreateWillRequest>,
) -> impl IntoResponse {
    let Some(ref sys) = state.inheritance_system else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "inheritance system not available"})),
        );
    };
    let mut mgr = sys.lock().await;
    match mgr.create_will(&req.testator_id, req.beneficiaries, req.tick) {
        Ok(id) => (
            StatusCode::CREATED,
            Json(serde_json::json!({"will_id": id.to_string()})),
        ),
        Err(e) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(serde_json::json!({"error": e})),
        ),
    }
}

pub async fn inheritance_get_will(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
) -> impl IntoResponse {
    let Some(ref sys) = state.inheritance_system else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "inheritance system not available"})),
        );
    };
    let mgr = sys.lock().await;
    match mgr.get_will(&agent_id) {
        Some(will) => (StatusCode::OK, Json(serde_json::json!(will))),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "no will found for this agent"})),
        ),
    }
}

pub async fn inheritance_has_will(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
) -> impl IntoResponse {
    let Some(ref sys) = state.inheritance_system else {
        return (StatusCode::OK, Json(serde_json::json!({"has_will": false})));
    };
    let mgr = sys.lock().await;
    (StatusCode::OK, Json(serde_json::json!({"has_will": mgr.has_will(&agent_id)})))
}

pub async fn inheritance_stats(State(state): State<AppState>) -> impl IntoResponse {
    let Some(ref sys) = state.inheritance_system else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "inheritance system not available"})),
        );
    };
    let mgr = sys.lock().await;
    (StatusCode::OK, Json(serde_json::json!({
        "will_count": mgr.will_count(),
    })))
}

// -- Routes --

pub fn inheritance_routes() -> axum::Router<AppState> {
    axum::Router::new()
        .route("/api/v1/inheritance/will", post(inheritance_create_will))
        .route("/api/v1/inheritance/will/:agent_id", get(inheritance_get_will))
        .route("/api/v1/inheritance/has_will/:agent_id", get(inheritance_has_will))
        .route("/api/v1/inheritance/stats", get(inheritance_stats))
}
