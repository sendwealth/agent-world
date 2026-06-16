//! REST API routes for cross-world trade routes (Phase 5.6).
//!
//! Endpoints:
//! - `POST   /federation/trades/offer`     — create a trade offer
//! - `POST   /federation/trades/:id/accept` — accept a pending offer
//! - `POST   /federation/trades/:id/execute` — execute an accepted trade atomically
//! - `GET    /federation/trades`            — list trades (optional `?world_id=`, `?status=`)
//! - `GET    /federation/trades/:id`        — get a single trade
//! - `DELETE /federation/trades/:id`        — cancel a pending/accepted trade
//! - `POST   /federation/trades/sweep-expired` — sweep expired offers
//! - `GET    /federation/trades/stats`      — trade statistics

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::*,
    Json,
};
use serde::Deserialize;

use crate::api::{api_err, api_ok, AppState};
use crate::federation::trade::{CrossWorldTradeManager, TradeItem, TradeStatus};

// ── REST DTOs ─────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct RestCreateOffer {
    pub initiator_world_id: String,
    pub receiver_world_id: String,
    pub offering: TradeItem,
    pub wanting: TradeItem,
    #[serde(default)]
    pub expires_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RestAcceptOffer {
    pub accepter_world_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RestCancelTrade {
    pub cancelled_by: String,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RestListTradesQuery {
    #[serde(default)]
    pub world_id: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub limit: Option<u32>,
    #[serde(default)]
    pub offset: Option<u32>,
}

// ── Helpers ───────────────────────────────────────────────

fn parse_status(s: &str) -> Option<TradeStatus> {
    match s {
        "pending" => Some(TradeStatus::Pending),
        "accepted" => Some(TradeStatus::Accepted),
        "escrow_locked" => Some(TradeStatus::EscrowLocked),
        "completed" => Some(TradeStatus::Completed),
        "cancelled" => Some(TradeStatus::Cancelled),
        "expired" => Some(TradeStatus::Expired),
        _ => None,
    }
}

/// Extract the trade manager or return a 503 response.
fn manager_or_503(
    state: &AppState,
) -> Result<&std::sync::Arc<tokio::sync::Mutex<CrossWorldTradeManager>>, Box<axum::response::Response>> {
    state.trade_manager.as_ref().ok_or_else(|| {
        Box::new(api_err(
            StatusCode::SERVICE_UNAVAILABLE,
            "cross-world trade manager not configured",
        ))
    })
}

// ── Handlers ──────────────────────────────────────────────

/// POST /api/v1/federation/trades/offer
pub async fn create_trade_offer(
    State(state): State<AppState>,
    Json(body): Json<RestCreateOffer>,
) -> impl IntoResponse {
    let manager = match manager_or_503(&state) {
        Ok(m) => m.clone(),
        Err(resp) => return *resp,
    };
    let mgr = manager.lock().await;
    match mgr
        .create_offer(
            body.initiator_world_id,
            body.receiver_world_id,
            body.offering,
            body.wanting,
            body.expires_at,
        )
        .await
    {
        Ok(offer) => api_ok(&offer),
        Err(e) => api_err(StatusCode::BAD_REQUEST, e),
    }
}

/// POST /api/v1/federation/trades/:id/accept
pub async fn accept_trade(
    State(state): State<AppState>,
    Path(trade_id): Path<String>,
    Json(body): Json<RestAcceptOffer>,
) -> impl IntoResponse {
    let manager = match manager_or_503(&state) {
        Ok(m) => m.clone(),
        Err(resp) => return *resp,
    };
    let mgr = manager.lock().await;
    match mgr.accept_offer(&trade_id, &body.accepter_world_id).await {
        Ok(offer) => api_ok(&offer),
        Err(e) => api_err(StatusCode::BAD_REQUEST, e),
    }
}

/// POST /api/v1/federation/trades/:id/execute
pub async fn execute_trade(
    State(state): State<AppState>,
    Path(trade_id): Path<String>,
) -> impl IntoResponse {
    let manager = match manager_or_503(&state) {
        Ok(m) => m.clone(),
        Err(resp) => return *resp,
    };
    let mgr = manager.lock().await;
    match mgr.execute_trade(&trade_id).await {
        Ok(offer) => api_ok(&offer),
        Err(e) => api_err(StatusCode::BAD_REQUEST, e),
    }
}

/// GET /api/v1/federation/trades
pub async fn list_trades(
    State(state): State<AppState>,
    Query(q): Query<RestListTradesQuery>,
) -> impl IntoResponse {
    let manager = match manager_or_503(&state) {
        Ok(m) => m.clone(),
        Err(resp) => return *resp,
    };
    let status_filter = q.status.as_deref().and_then(parse_status);
    let mgr = manager.lock().await;
    let results = mgr
        .list_offers(
            q.world_id.as_deref(),
            status_filter,
            q.limit.unwrap_or(20),
            q.offset.unwrap_or(0),
        )
        .await;
    api_ok(&results)
}

/// GET /api/v1/federation/trades/:id
pub async fn get_trade(
    State(state): State<AppState>,
    Path(trade_id): Path<String>,
) -> impl IntoResponse {
    let manager = match manager_or_503(&state) {
        Ok(m) => m.clone(),
        Err(resp) => return *resp,
    };
    let mgr = manager.lock().await;
    match mgr.get_trade(&trade_id).await {
        Some(offer) => api_ok(&offer),
        None => api_err(StatusCode::NOT_FOUND, "trade not found"),
    }
}

/// DELETE /api/v1/federation/trades/:id
pub async fn cancel_trade(
    State(state): State<AppState>,
    Path(trade_id): Path<String>,
    Json(body): Json<RestCancelTrade>,
) -> impl IntoResponse {
    let manager = match manager_or_503(&state) {
        Ok(m) => m.clone(),
        Err(resp) => return *resp,
    };
    let mgr = manager.lock().await;
    match mgr
        .cancel_trade(&trade_id, &body.cancelled_by, body.reason)
        .await
    {
        Ok(offer) => api_ok(&offer),
        Err(e) => api_err(StatusCode::BAD_REQUEST, e),
    }
}

/// POST /api/v1/federation/trades/sweep-expired
pub async fn sweep_expired_trades(State(state): State<AppState>) -> impl IntoResponse {
    let manager = match manager_or_503(&state) {
        Ok(m) => m.clone(),
        Err(resp) => return *resp,
    };
    let mgr = manager.lock().await;
    let expired = mgr.sweep_expired().await;
    api_ok(serde_json::json!({ "expired_count": expired.len(), "trade_ids": expired }))
}

/// GET /api/v1/federation/trades/stats
pub async fn trade_stats(State(state): State<AppState>) -> impl IntoResponse {
    let manager = match manager_or_503(&state) {
        Ok(m) => m.clone(),
        Err(resp) => return *resp,
    };
    let mgr = manager.lock().await;
    let stats = mgr.stats().await;
    api_ok(&stats)
}

/// Build the federation-trade sub-router.
pub fn federation_trade_routes() -> axum::Router<AppState> {
    axum::Router::new()
        .route("/federation/trades/offer", post(create_trade_offer))
        .route("/federation/trades/sweep-expired", post(sweep_expired_trades))
        .route("/federation/trades/stats", get(trade_stats))
        .route("/federation/trades/:id/accept", post(accept_trade))
        .route("/federation/trades/:id/execute", post(execute_trade))
        .route("/federation/trades/:id", get(get_trade))
        .route("/federation/trades/:id", delete(cancel_trade))
        .route("/federation/trades", get(list_trades))
}
