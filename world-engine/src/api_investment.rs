use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::*,
};
use serde::Deserialize;

use crate::api::AppState;
use crate::economy::investment::{
    InvestmentSystem, InvestmentError,
    CreateProductRequest, BuySharesRequest, SellSharesRequest,
    CloseInvestmentRequest, DistributeReturnsRequest,
    UpdatePerformanceRequest, FreezeProductRequest,
    ListTransactionsQuery,
};

// ── Investment System API Handlers ────────────────────────

pub fn inv_error_to_status(e: &InvestmentError) -> StatusCode {
    match e {
        InvestmentError::ProductNotFound(_)
        | InvestmentError::PositionNotFound(_) => StatusCode::NOT_FOUND,
        InvestmentError::Unauthorized(_) => StatusCode::FORBIDDEN,
        InvestmentError::InvalidShareCount
        | InvestmentError::InvalidPrice
        | InvestmentError::InvalidTotalShares
        | InvestmentError::InvalidPerformanceScore => StatusCode::BAD_REQUEST,
        InvestmentError::DuplicateIdempotencyKey(_) => StatusCode::CONFLICT,
        _ => StatusCode::UNPROCESSABLE_ENTITY,
    }
}

pub async fn inv_create_product(
    State(state): State<AppState>,
    Json(req): Json<CreateProductRequest>,
) -> impl IntoResponse {
    let Some(ref inv) = state.investment_system else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error": "investment system not available"})));
    };
    let mut sys = inv.lock().await;
    match sys.create_product(req) {
        Ok(product) => (StatusCode::CREATED, Json(serde_json::json!(product))),
        Err(e) => (inv_error_to_status(&e), Json(serde_json::json!({"error": e.to_string()}))),
    }
}

pub async fn inv_list_products(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let Some(ref inv) = state.investment_system else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!([])));
    };
    let sys = inv.lock().await;
    (StatusCode::OK, Json(serde_json::json!(sys.list_products())))
}

pub async fn inv_get_product(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let Some(ref inv) = state.investment_system else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error": "investment system not available"})));
    };
    let sys = inv.lock().await;
    match sys.get_product(&id) {
        Some(p) => (StatusCode::OK, Json(serde_json::json!(p))),
        None => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "product not found"}))),
    }
}

pub async fn inv_buy_shares(
    State(state): State<AppState>,
    Json(req): Json<BuySharesRequest>,
) -> impl IntoResponse {
    let Some(ref inv) = state.investment_system else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error": "investment system not available"})));
    };
    let mut sys = inv.lock().await;
    match sys.buy_shares(req) {
        Ok((pos, tx)) => (StatusCode::OK, Json(serde_json::json!({"position": pos, "transaction": tx}))),
        Err(e) => (inv_error_to_status(&e), Json(serde_json::json!({"error": e.to_string()}))),
    }
}

pub async fn inv_sell_shares(
    State(state): State<AppState>,
    Json(req): Json<SellSharesRequest>,
) -> impl IntoResponse {
    let Some(ref inv) = state.investment_system else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error": "investment system not available"})));
    };
    let mut sys = inv.lock().await;
    match sys.sell_shares(req) {
        Ok((pos, tx)) => (StatusCode::OK, Json(serde_json::json!({"position": pos, "transaction": tx}))),
        Err(e) => (inv_error_to_status(&e), Json(serde_json::json!({"error": e.to_string()}))),
    }
}

pub async fn inv_get_portfolio(
    State(state): State<AppState>,
    Path(investor_id): Path<String>,
) -> impl IntoResponse {
    let Some(ref inv) = state.investment_system else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!([])));
    };
    let sys = inv.lock().await;
    (StatusCode::OK, Json(serde_json::json!(sys.get_portfolio(&investor_id))))
}

pub async fn inv_leaderboard(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let Some(ref inv) = state.investment_system else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!([])));
    };
    let sys = inv.lock().await;
    (StatusCode::OK, Json(serde_json::json!(sys.get_leaderboard())))
}

pub async fn inv_close_investment(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<CloseInvestmentRequest>,
) -> impl IntoResponse {
    let Some(ref inv) = state.investment_system else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error": "investment system not available"})));
    };
    let mut sys = inv.lock().await;
    let mut req = req;
    req.product_id = id;
    match sys.close_investment(req) {
        Ok(product) => (StatusCode::OK, Json(serde_json::json!(product))),
        Err(e) => (inv_error_to_status(&e), Json(serde_json::json!({"error": e.to_string()}))),
    }
}

pub async fn inv_distribute_returns(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<DistributeReturnsRequest>,
) -> impl IntoResponse {
    let Some(ref inv) = state.investment_system else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error": "investment system not available"})));
    };
    let mut sys = inv.lock().await;
    let mut req = req;
    req.product_id = id;
    match sys.distribute_returns(req) {
        Ok(dist) => (StatusCode::OK, Json(serde_json::json!(dist))),
        Err(e) => (inv_error_to_status(&e), Json(serde_json::json!({"error": e.to_string()}))),
    }
}

pub async fn inv_update_performance(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<UpdatePerformanceRequest>,
) -> impl IntoResponse {
    let Some(ref inv) = state.investment_system else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error": "investment system not available"})));
    };
    let mut sys = inv.lock().await;
    match sys.update_performance(&id, req.performance_score) {
        Ok(product) => (StatusCode::OK, Json(serde_json::json!(product))),
        Err(e) => (inv_error_to_status(&e), Json(serde_json::json!({"error": e.to_string()}))),
    }
}

pub async fn inv_freeze_product(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<FreezeProductRequest>,
) -> impl IntoResponse {
    let Some(ref inv) = state.investment_system else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error": "investment system not available"})));
    };
    let mut sys = inv.lock().await;
    let mut req = req;
    req.product_id = id;
    match sys.freeze_product(req) {
        Ok(product) => (StatusCode::OK, Json(serde_json::json!(product))),
        Err(e) => (inv_error_to_status(&e), Json(serde_json::json!({"error": e.to_string()}))),
    }
}

pub async fn inv_list_transactions(
    State(state): State<AppState>,
    Query(query): Query<ListTransactionsQuery>,
) -> impl IntoResponse {
    let Some(ref inv) = state.investment_system else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!([])));
    };
    let sys = inv.lock().await;
    (StatusCode::OK, Json(serde_json::json!(sys.list_transactions(&query))))
}

pub async fn inv_list_dividends(
    State(state): State<AppState>,
    Query(query): Query<serde_json::Value>,
) -> impl IntoResponse {
    let product_id = query.get("product_id").and_then(|v| v.as_str()).map(|s| s.to_string());
    let Some(ref inv) = state.investment_system else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!([])));
    };
    let sys = inv.lock().await;
    (StatusCode::OK, Json(serde_json::json!(sys.list_dividends(product_id.as_deref()))))
}

/// Investment system routes.
pub fn investment_routes() -> axum::Router<AppState> {
    axum::Router::new()
        .route("/api/v1/investments/products", post(inv_create_product))
        .route("/api/v1/investments/products", get(inv_list_products))
        .route("/api/v1/investments/products/:id", get(inv_get_product))
        .route("/api/v1/investments/buy", post(inv_buy_shares))
        .route("/api/v1/investments/sell", post(inv_sell_shares))
        .route("/api/v1/investments/portfolio/:investor_id", get(inv_get_portfolio))
        .route("/api/v1/investments/leaderboard", get(inv_leaderboard))
        .route("/api/v1/investments/products/:id/close", post(inv_close_investment))
        .route("/api/v1/investments/products/:id/distribute-returns", post(inv_distribute_returns))
        .route("/api/v1/investments/products/:id/performance", post(inv_update_performance))
        .route("/api/v1/investments/products/:id/freeze", post(inv_freeze_product))
        .route("/api/v1/investments/transactions", get(inv_list_transactions))
        .route("/api/v1/investments/dividends", get(inv_list_dividends))
}
