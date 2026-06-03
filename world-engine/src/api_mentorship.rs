use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::*,
    Json,
};
use serde::Deserialize;

use crate::api::AppState;

// -- Request Types --

#[derive(Debug, Deserialize)]
pub struct EstablishMentorshipRequest {
    pub mentor_id: String,
    pub apprentice_id: String,
    pub skill_name: String,
    pub mentor_skill_level: u32,
    pub tick: u64,
}

// -- API Handlers --

pub async fn mentorship_establish(
    State(state): State<AppState>,
    Json(req): Json<EstablishMentorshipRequest>,
) -> impl IntoResponse {
    let Some(ref sys) = state.mentorship_system else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "mentorship system not available"})),
        );
    };
    let mut mgr = sys.lock().await;
    match mgr.establish(
        &req.mentor_id,
        &req.apprentice_id,
        &req.skill_name,
        req.mentor_skill_level,
        req.tick,
    ) {
        Ok(id) => (
            StatusCode::CREATED,
            Json(serde_json::json!({"session_id": id.to_string()})),
        ),
        Err(e) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(serde_json::json!({"error": e})),
        ),
    }
}

pub async fn mentorship_mentor_sessions(
    State(state): State<AppState>,
    Path(mentor_id): Path<String>,
) -> impl IntoResponse {
    let Some(ref sys) = state.mentorship_system else {
        return (StatusCode::OK, Json(serde_json::json!([])));
    };
    let mgr = sys.lock().await;
    (StatusCode::OK, Json(serde_json::json!(mgr.mentor_active_sessions(&mentor_id))))
}

pub async fn mentorship_apprentice_sessions(
    State(state): State<AppState>,
    Path(apprentice_id): Path<String>,
) -> impl IntoResponse {
    let Some(ref sys) = state.mentorship_system else {
        return (StatusCode::OK, Json(serde_json::json!([])));
    };
    let mgr = sys.lock().await;
    (StatusCode::OK, Json(serde_json::json!(mgr.apprentice_sessions(&apprentice_id))))
}

pub async fn mentorship_stats(State(state): State<AppState>) -> impl IntoResponse {
    let Some(ref sys) = state.mentorship_system else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "mentorship system not available"})),
        );
    };
    let mgr = sys.lock().await;
    (StatusCode::OK, Json(serde_json::json!({
        "session_count": mgr.session_count(),
        "completed_count": mgr.completed_count(),
        "active_count": mgr.active_count(),
    })))
}

// -- Routes --

pub fn mentorship_routes() -> axum::Router<AppState> {
    axum::Router::new()
        .route("/api/v1/mentorship/establish", post(mentorship_establish))
        .route("/api/v1/mentorship/mentor/:mentor_id", get(mentorship_mentor_sessions))
        .route("/api/v1/mentorship/apprentice/:apprentice_id", get(mentorship_apprentice_sessions))
        .route("/api/v1/mentorship/stats", get(mentorship_stats))
}
