use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::*,
    Json,
};
use serde::Deserialize;

use crate::api::AppState;
use crate::world::event::TrustInteractionType;

// -- Request Types --

#[derive(Debug, Deserialize)]
pub struct RecordInteractionRequest {
    pub from_agent: String,
    pub to_agent: String,
    pub interaction: TrustInteractionType,
    pub tick: u64,
}

// -- API Handlers --

pub async fn trust_record_interaction(
    State(state): State<AppState>,
    Json(req): Json<RecordInteractionRequest>,
) -> impl IntoResponse {
    let Some(ref net) = state.trust_network else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "trust network not available"})),
        );
    };
    let mut sys = net.lock().await;
    let score = sys.record_interaction(
        &req.from_agent,
        &req.to_agent,
        req.interaction,
        req.tick,
    );
    (StatusCode::OK, Json(serde_json::json!({"trust_score": score})))
}

pub async fn trust_get(
    State(state): State<AppState>,
    Path((from, to)): Path<(String, String)>,
) -> impl IntoResponse {
    let Some(ref net) = state.trust_network else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "trust network not available"})),
        );
    };
    let sys = net.lock().await;
    let score = sys.get_trust(&from, &to);
    (StatusCode::OK, Json(serde_json::json!({"from": from, "to": to, "trust_score": score})))
}

pub async fn trust_relationships(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
) -> impl IntoResponse {
    let Some(ref net) = state.trust_network else {
        return (StatusCode::OK, Json(serde_json::json!([])));
    };
    let sys = net.lock().await;
    (StatusCode::OK, Json(serde_json::json!(sys.get_agent_relationships(&agent_id))))
}

pub async fn trust_allies(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
) -> impl IntoResponse {
    let Some(ref net) = state.trust_network else {
        return (StatusCode::OK, Json(serde_json::json!([])));
    };
    let sys = net.lock().await;
    (StatusCode::OK, Json(serde_json::json!(sys.get_allies(&agent_id))))
}

pub async fn trust_enemies(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
) -> impl IntoResponse {
    let Some(ref net) = state.trust_network else {
        return (StatusCode::OK, Json(serde_json::json!([])));
    };
    let sys = net.lock().await;
    (StatusCode::OK, Json(serde_json::json!(sys.get_enemies(&agent_id))))
}

pub async fn trust_stats(State(state): State<AppState>) -> impl IntoResponse {
    let Some(ref net) = state.trust_network else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "trust network not available"})),
        );
    };
    let sys = net.lock().await;
    (StatusCode::OK, Json(serde_json::json!({
        "edge_count": sys.edge_count(),
        "total_interactions": sys.total_interactions(),
    })))
}

// -- Routes --

pub fn trust_routes() -> axum::Router<AppState> {
    axum::Router::new()
        .route("/api/v1/trust/interact", post(trust_record_interaction))
        .route("/api/v1/trust/:from/:to", get(trust_get))
        .route("/api/v1/trust/relationships/:agent_id", get(trust_relationships))
        .route("/api/v1/trust/allies/:agent_id", get(trust_allies))
        .route("/api/v1/trust/enemies/:agent_id", get(trust_enemies))
        .route("/api/v1/trust/stats", get(trust_stats))
}
