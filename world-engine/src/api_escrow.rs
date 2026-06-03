use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::*,
    Json,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::api::AppState;
use crate::world::enums::Currency;

// -- Request Types --

#[derive(Debug, Deserialize)]
pub struct CreateEscrowRequest {
    pub publisher: String,
    pub reward: u64,
    pub deposit: u64,
    pub currency: Currency,
    pub created_tick: u64,
    pub expires_at: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct ClaimEscrowRequest {
    pub claimant: String,
}

#[derive(Debug, Deserialize)]
pub struct ResolveDisputeRequest {
    pub award_to_claimant: bool,
}

// -- API Handlers --

pub async fn escrow_create(
    State(state): State<AppState>,
    Json(req): Json<CreateEscrowRequest>,
) -> impl IntoResponse {
    let Some(ref mgr) = state.escrow_manager else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "escrow system not available"})),
        );
    };
    let mut sys = mgr.lock().await;
    match sys.create_escrow(
        &req.publisher, req.reward, req.deposit, req.currency,
        req.created_tick, req.expires_at,
    ) {
        Ok(id) => (
            StatusCode::CREATED,
            Json(serde_json::json!({"escrow_id": id.to_string()})),
        ),
        Err(e) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

pub async fn escrow_list(State(state): State<AppState>) -> impl IntoResponse {
    let Some(ref mgr) = state.escrow_manager else {
        return (StatusCode::OK, Json(serde_json::json!([])));
    };
    let sys = mgr.lock().await;
    (StatusCode::OK, Json(serde_json::json!(sys.list())))
}

pub async fn escrow_get(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let Some(ref mgr) = state.escrow_manager else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "escrow system not available"})),
        );
    };
    let sys = mgr.lock().await;
    match id.parse::<Uuid>() {
        Ok(uuid) => match sys.get(uuid) {
            Some(record) => (StatusCode::OK, Json(serde_json::json!(record))),
            None => (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "escrow not found"})),
            ),
        },
        Err(_) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "invalid escrow id"})),
        ),
    }
}

pub async fn escrow_claim(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<ClaimEscrowRequest>,
) -> impl IntoResponse {
    let Some(ref mgr) = state.escrow_manager else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "escrow system not available"})),
        );
    };
    let mut sys = mgr.lock().await;
    match id.parse::<Uuid>() {
        Ok(uuid) => match sys.claim_escrow(uuid, &req.claimant) {
            Ok(()) => (StatusCode::OK, Json(serde_json::json!({"status": "claimed"}))),
            Err(e) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(serde_json::json!({"error": e.to_string()})),
            ),
        },
        Err(_) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "invalid escrow id"})),
        ),
    }
}

pub async fn escrow_complete(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let Some(ref mgr) = state.escrow_manager else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "escrow system not available"})),
        );
    };
    let mut sys = mgr.lock().await;
    match id.parse::<Uuid>() {
        Ok(uuid) => match sys.complete_escrow(uuid) {
            Ok(()) => (StatusCode::OK, Json(serde_json::json!({"status": "completed"}))),
            Err(e) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(serde_json::json!({"error": e.to_string()})),
            ),
        },
        Err(_) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "invalid escrow id"})),
        ),
    }
}

pub async fn escrow_refund(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let Some(ref mgr) = state.escrow_manager else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "escrow system not available"})),
        );
    };
    let mut sys = mgr.lock().await;
    match id.parse::<Uuid>() {
        Ok(uuid) => match sys.refund_escrow(uuid) {
            Ok(()) => (StatusCode::OK, Json(serde_json::json!({"status": "refunded"}))),
            Err(e) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(serde_json::json!({"error": e.to_string()})),
            ),
        },
        Err(_) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "invalid escrow id"})),
        ),
    }
}

pub async fn escrow_dispute(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let Some(ref mgr) = state.escrow_manager else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "escrow system not available"})),
        );
    };
    let mut sys = mgr.lock().await;
    match id.parse::<Uuid>() {
        Ok(uuid) => match sys.dispute_escrow(uuid, "api dispute") {
            Ok(()) => (StatusCode::OK, Json(serde_json::json!({"status": "disputed"}))),
            Err(e) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(serde_json::json!({"error": e.to_string()})),
            ),
        },
        Err(_) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "invalid escrow id"})),
        ),
    }
}

pub async fn escrow_resolve_dispute(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<ResolveDisputeRequest>,
) -> impl IntoResponse {
    let Some(ref mgr) = state.escrow_manager else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "escrow system not available"})),
        );
    };
    let mut sys = mgr.lock().await;
    match id.parse::<Uuid>() {
        Ok(uuid) => match sys.resolve_dispute(uuid, req.award_to_claimant) {
            Ok(()) => (StatusCode::OK, Json(serde_json::json!({"status": "resolved"}))),
            Err(e) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(serde_json::json!({"error": e.to_string()})),
            ),
        },
        Err(_) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "invalid escrow id"})),
        ),
    }
}

pub async fn escrow_set_balance(
    State(state): State<AppState>,
    Json(req): Json<serde_json::Value>,
) -> impl IntoResponse {
    let Some(ref mgr) = state.escrow_manager else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "escrow system not available"})),
        );
    };
    let agent = req.get("agent").and_then(|v| v.as_str()).unwrap_or("");
    let amount = req.get("amount").and_then(|v| v.as_u64()).unwrap_or(0);
    let mut sys = mgr.lock().await;
    sys.set_balance(agent, amount);
    (StatusCode::OK, Json(serde_json::json!({"status": "balance_set"})))
}

// -- Routes --

pub fn escrow_routes() -> axum::Router<AppState> {
    axum::Router::new()
        .route("/api/v1/escrow", post(escrow_create))
        .route("/api/v1/escrow", get(escrow_list))
        .route("/api/v1/escrow/:id", get(escrow_get))
        .route("/api/v1/escrow/:id/claim", post(escrow_claim))
        .route("/api/v1/escrow/:id/complete", post(escrow_complete))
        .route("/api/v1/escrow/:id/refund", post(escrow_refund))
        .route("/api/v1/escrow/:id/dispute", post(escrow_dispute))
        .route("/api/v1/escrow/:id/resolve", post(escrow_resolve_dispute))
        .route("/api/v1/escrow/balance", post(escrow_set_balance))
}
