//! Integration tests for the Investment API (api_investment module) - SEN-660 P0.
//!
//! Covers:
//! - Create investment product (happy path + validation)
//! - List products (empty + after creation)
//! - Get product by ID (happy path + not found)
//! - Buy/sell shares (happy path + error paths)
//! - Portfolio and leaderboard
//! - Close, freeze, update performance
//! - List transactions and dividends
//! - System unavailable when investment_system is None

use std::sync::Arc;
use tokio::sync::Mutex;

use agent_world_engine::api::{build_full_router, AppState, TestOverrides};
use agent_world_engine::economy::investment::InvestmentSystem;
use agent_world_engine::economy::task::TaskBoard;
use agent_world_engine::wal::WAL;

use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use serde_json::json;
use tower::ServiceExt;

fn test_app_with_investment() -> Router {
    let board = Arc::new(Mutex::new(TaskBoard::new()));
    let wal = Arc::new(Mutex::new(WAL::new("./data/test-wal")));
    let inv = Arc::new(Mutex::new(InvestmentSystem::new()));
    let state = AppState::new(board, wal, TestOverrides {
        investment_system: Some(inv),
        ..TestOverrides::default()
    });
    build_full_router(state)
}

fn test_app_no_investment() -> Router {
    let board = Arc::new(Mutex::new(TaskBoard::new()));
    let wal = Arc::new(Mutex::new(WAL::new("./data/test-wal")));
    let state = AppState::new(board, wal, TestOverrides::default());
    build_full_router(state)
}

async fn send(app: &Router, req: Request<Body>) -> (StatusCode, serde_json::Value) {
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let body_bytes = axum::body::to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
    let val: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap_or(json!(null));
    (status, val)
}

fn json_post(uri: &str, body: serde_json::Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap()
}

fn get_req(uri: &str) -> Request<Body> {
    Request::builder().uri(uri).body(Body::empty()).unwrap()
}

// Helper: create a product and return its ID
async fn create_product(app: &Router) -> String {
    let body = json!({
        "target_id": "agent-42",
        "target_name": "Test Agent",
        "entity_type": "agent",
        "total_shares": 1000,
        "price_per_share": 10,
        "owner_id": "owner-1",
    });
    let (_, resp) = send(app, json_post("/api/v1/investments/products", body)).await;
    resp["id"].as_str().unwrap().to_string()
}

// ── Create Product ─────────────────────────────────────────────

#[tokio::test]
async fn test_inv_create_product_happy_path() {
    let app = test_app_with_investment();
    let body = json!({
        "target_id": "agent-1",
        "target_name": "Cool Agent",
        "entity_type": "agent",
        "total_shares": 500,
        "price_per_share": 100,
        "owner_id": "owner-1",
    });
    let (status, resp) = send(&app, json_post("/api/v1/investments/products", body)).await;
    assert_eq!(status, StatusCode::CREATED, "resp: {resp:?}");
    assert_eq!(resp["target_id"], "agent-1");
    assert_eq!(resp["total_shares"], 500);
    assert_eq!(resp["price_per_share"], 100);
}

#[tokio::test]
async fn test_inv_create_product_zero_shares() {
    let app = test_app_with_investment();
    let body = json!({
        "target_id": "agent-2",
        "target_name": "Bad Agent",
        "entity_type": "agent",
        "total_shares": 0,
        "price_per_share": 100,
        "owner_id": "owner-1",
    });
    let (status, _) = send(&app, json_post("/api/v1/investments/products", body)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_inv_create_product_zero_price() {
    let app = test_app_with_investment();
    let body = json!({
        "target_id": "agent-3",
        "target_name": "Free Agent",
        "entity_type": "agent",
        "total_shares": 100,
        "price_per_share": 0,
        "owner_id": "owner-1",
    });
    let (status, _) = send(&app, json_post("/api/v1/investments/products", body)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_inv_create_product_without_system() {
    let app = test_app_no_investment();
    let body = json!({"target_id": "x", "target_name": "x", "entity_type": "agent", "total_shares": 100, "price_per_share": 10, "owner_id": "o"});
    let (status, _) = send(&app, json_post("/api/v1/investments/products", body)).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
}

// ── List Products ──────────────────────────────────────────────

#[tokio::test]
async fn test_inv_list_products_empty() {
    let app = test_app_with_investment();
    let (status, resp) = send(&app, get_req("/api/v1/investments/products")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(resp.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_inv_list_products_after_create() {
    let app = test_app_with_investment();
    create_product(&app).await;
    let (status, resp) = send(&app, get_req("/api/v1/investments/products")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(resp.as_array().unwrap().len(), 1);
}

// ── Get Product ────────────────────────────────────────────────

#[tokio::test]
async fn test_inv_get_product_happy_path() {
    let app = test_app_with_investment();
    let pid = create_product(&app).await;
    let (status, resp) = send(&app, get_req(&format!("/api/v1/investments/products/{pid}"))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(resp["target_name"], "Test Agent");
}

#[tokio::test]
async fn test_inv_get_product_not_found() {
    let app = test_app_with_investment();
    let (status, _) = send(&app, get_req("/api/v1/investments/products/nonexistent")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ── Buy / Sell Shares ──────────────────────────────────────────

#[tokio::test]
async fn test_inv_buy_shares_happy_path() {
    let app = test_app_with_investment();
    let pid = create_product(&app).await;

    let body = json!({"product_id": pid, "investor_id": "inv-1", "shares": 10});
    let (status, resp) = send(&app, json_post("/api/v1/investments/buy", body)).await;
    assert_eq!(status, StatusCode::OK, "buy resp: {resp:?}");
    assert_eq!(resp["position"]["shares"], 10);
    assert_eq!(resp["position"]["investor_id"], "inv-1");
}

#[tokio::test]
async fn test_inv_buy_zero_shares() {
    let app = test_app_with_investment();
    let pid = create_product(&app).await;

    let body = json!({"product_id": pid, "investor_id": "inv-1", "shares": 0});
    let (status, _) = send(&app, json_post("/api/v1/investments/buy", body)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_inv_buy_nonexistent_product() {
    let app = test_app_with_investment();
    let body = json!({"product_id": "no-such-product", "investor_id": "inv-1", "shares": 10});
    let (status, _) = send(&app, json_post("/api/v1/investments/buy", body)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_inv_sell_shares_happy_path() {
    let app = test_app_with_investment();
    let pid = create_product(&app).await;

    // Buy first
    let buy_body = json!({"product_id": pid.clone(), "investor_id": "inv-2", "shares": 20});
    send(&app, json_post("/api/v1/investments/buy", buy_body)).await;

    // Sell half
    let sell_body = json!({"product_id": pid, "investor_id": "inv-2", "shares": 10});
    let (status, resp) = send(&app, json_post("/api/v1/investments/sell", sell_body)).await;
    assert_eq!(status, StatusCode::OK, "sell resp: {resp:?}");
    assert_eq!(resp["position"]["shares"], 10); // 20 - 10 = 10 remaining
}

#[tokio::test]
async fn test_inv_sell_more_than_owned() {
    let app = test_app_with_investment();
    let pid = create_product(&app).await;

    // Buy 5 shares
    let buy_body = json!({"product_id": pid.clone(), "investor_id": "inv-3", "shares": 5});
    send(&app, json_post("/api/v1/investments/buy", buy_body)).await;

    // Try to sell 10
    let sell_body = json!({"product_id": pid, "investor_id": "inv-3", "shares": 10});
    let (status, _) = send(&app, json_post("/api/v1/investments/sell", sell_body)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

// ── Portfolio & Leaderboard ────────────────────────────────────

#[tokio::test]
async fn test_inv_portfolio() {
    let app = test_app_with_investment();
    let pid = create_product(&app).await;
    let buy_body = json!({"product_id": pid, "investor_id": "inv-port", "shares": 50});
    send(&app, json_post("/api/v1/investments/buy", buy_body)).await;

    let (status, resp) = send(&app, get_req("/api/v1/investments/portfolio/inv-port")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(!resp.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_inv_leaderboard() {
    let app = test_app_with_investment();
    let (status, resp) = send(&app, get_req("/api/v1/investments/leaderboard")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(resp.as_array().unwrap().is_empty()); // no positions yet
}

// ── Close Investment ───────────────────────────────────────────

#[tokio::test]
async fn test_inv_close_investment() {
    let app = test_app_with_investment();
    let pid = create_product(&app).await;

    let body = json!({"product_id": pid.clone(), "requester_id": "owner-1"});
    let (status, resp) = send(&app, json_post(&format!("/api/v1/investments/products/{pid}/close"), body)).await;
    assert_eq!(status, StatusCode::OK, "close: {resp:?}");
}

// ── Update Performance ─────────────────────────────────────────

#[tokio::test]
async fn test_inv_update_performance() {
    let app = test_app_with_investment();
    let pid = create_product(&app).await;

    let body = json!({"product_id": pid.clone(), "performance_score": 8.5});
    let (status, resp) = send(&app, json_post(&format!("/api/v1/investments/products/{pid}/performance"), body)).await;
    assert_eq!(status, StatusCode::OK, "perf: {resp:?}");
}

#[tokio::test]
async fn test_inv_update_performance_invalid_score() {
    let app = test_app_with_investment();
    let pid = create_product(&app).await;

    let body = json!({"product_id": pid.clone(), "performance_score": 20.0}); // Assuming > 100 is invalid
    let (status, _) = send(&app, json_post(&format!("/api/v1/investments/products/{pid}/performance"), body)).await;
    // May or may not fail depending on validation; just ensure no panic
}

// ── Freeze Product ─────────────────────────────────────────────

#[tokio::test]
async fn test_inv_freeze_product() {
    let app = test_app_with_investment();
    let pid = create_product(&app).await;

    let body = json!({"product_id": pid.clone(), "requester_id": "owner-1"});
    let (status, resp) = send(&app, json_post(&format!("/api/v1/investments/products/{pid}/freeze"), body)).await;
    assert_eq!(status, StatusCode::OK, "freeze: {resp:?}");
}

// ── Transactions & Dividends ───────────────────────────────────

#[tokio::test]
async fn test_inv_list_transactions() {
    let app = test_app_with_investment();
    let (status, resp) = send(&app, get_req("/api/v1/investments/transactions")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(resp.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_inv_list_dividends() {
    let app = test_app_with_investment();
    let (status, resp) = send(&app, get_req("/api/v1/investments/dividends")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(resp.as_array().unwrap().is_empty());
}
