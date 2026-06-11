//! Integration tests for the Escrow API (api_escrow module) - SEN-660 P0.
//!
//! Covers:
//! - Create escrow (happy path + invalid inputs)
//! - List escrows (empty + after creation)
//! - Get escrow by ID (happy path + not found + invalid UUID)
//! - Escrow lifecycle: claim -> complete / refund
//! - Dispute + resolve dispute
//! - Set balance for escrow operations
//! - System unavailable when escrow_manager is None

use std::sync::Arc;
use tokio::sync::Mutex;

use agent_world_engine::api::{build_full_router, AppState, TestOverrides};
use agent_world_engine::economy::escrow::EscrowManager;
use agent_world_engine::economy::task::TaskBoard;
use agent_world_engine::wal::WAL;

use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use serde_json::json;
use tower::ServiceExt;

fn test_app_with_escrow() -> Router {
    let board = Arc::new(Mutex::new(TaskBoard::new()));
    let wal = Arc::new(Mutex::new(WAL::new("./data/test-wal")));
    let escrow = Arc::new(Mutex::new(EscrowManager::new()));
    let state = AppState::new(board, wal, TestOverrides {
        escrow_manager: Some(escrow),
        ..TestOverrides::default()
    });
    build_full_router(state)
}

fn test_app_no_escrow() -> Router {
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

fn post_empty(uri: &str) -> Request<Body> {
    Request::builder().method("POST").uri(uri).body(Body::empty()).unwrap()
}

// ── Create Escrow ──────────────────────────────────────────────

#[tokio::test]
async fn test_escrow_create_happy_path() {
    let app = test_app_with_escrow();

    // Set balance for publisher first
    let bal_body = json!({"agent": "publisher-1", "amount": 1000});
    send(&app, json_post("/api/v1/escrow/balance", bal_body)).await;

    let body = json!({
        "publisher": "publisher-1",
        "reward": 100,
        "deposit": 50,
        "currency": "token",
        "created_tick": 10,
    });
    let (status, resp) = send(&app, json_post("/api/v1/escrow", body)).await;
    assert_eq!(status, StatusCode::CREATED, "create resp: {resp:?}");
    assert!(!resp["escrow_id"].as_str().unwrap().is_empty());
}

#[tokio::test]
async fn test_escrow_create_no_balance() {
    let app = test_app_with_escrow();
    let body = json!({
        "publisher": "broke-agent",
        "reward": 100,
        "deposit": 50,
        "currency": "token",
        "created_tick": 10,
    });
    let (status, resp) = send(&app, json_post("/api/v1/escrow", body)).await;
    // Should fail because no balance set
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "resp: {resp:?}");
}

#[tokio::test]
async fn test_escrow_create_without_system() {
    let app = test_app_no_escrow();
    let body = json!({"publisher": "p1", "reward": 10, "deposit": 5, "currency": "token", "created_tick": 0});
    let (status, _) = send(&app, json_post("/api/v1/escrow", body)).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
}

// ── List Escrows ───────────────────────────────────────────────

#[tokio::test]
async fn test_escrow_list_empty() {
    let app = test_app_with_escrow();
    let (status, resp) = send(&app, get_req("/api/v1/escrow")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(resp.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_escrow_list_after_create() {
    let app = test_app_with_escrow();
    let bal_body = json!({"agent": "p1", "amount": 500});
    send(&app, json_post("/api/v1/escrow/balance", bal_body)).await;
    let body = json!({"publisher": "p1", "reward": 50, "deposit": 25, "currency": "token", "created_tick": 0});
    send(&app, json_post("/api/v1/escrow", body)).await;

    let (status, resp) = send(&app, get_req("/api/v1/escrow")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(!resp.as_array().unwrap().is_empty());
}

// ── Get Escrow ─────────────────────────────────────────────────

#[tokio::test]
async fn test_escrow_get_happy_path() {
    let app = test_app_with_escrow();
    let bal_body = json!({"agent": "p1", "amount": 500});
    send(&app, json_post("/api/v1/escrow/balance", bal_body)).await;
    let body = json!({"publisher": "p1", "reward": 50, "deposit": 25, "currency": "money", "created_tick": 5});
    let (_, cr) = send(&app, json_post("/api/v1/escrow", body)).await;
    let escrow_id = cr["escrow_id"].as_str().unwrap();

    let (status, resp) = send(&app, get_req(&format!("/api/v1/escrow/{escrow_id}"))).await;
    assert_eq!(status, StatusCode::OK, "get resp: {resp:?}");
}

#[tokio::test]
async fn test_escrow_get_not_found() {
    let app = test_app_with_escrow();
    let fake = uuid::Uuid::new_v4().to_string();
    let (status, _) = send(&app, get_req(&format!("/api/v1/escrow/{fake}"))).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_escrow_get_invalid_uuid() {
    let app = test_app_with_escrow();
    let (status, _) = send(&app, get_req("/api/v1/escrow/bad-uuid")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

// ── Claim + Complete Escrow ────────────────────────────────────

#[tokio::test]
async fn test_escrow_claim_and_complete() {
    let app = test_app_with_escrow();

    // Setup publisher balance
    send(&app, json_post("/api/v1/escrow/balance", json!({"agent": "pub-1", "amount": 500}))).await;
    send(&app, json_post("/api/v1/escrow/balance", json!({"agent": "claim-1", "amount": 200}))).await;

    let body = json!({"publisher": "pub-1", "reward": 100, "deposit": 20, "currency": "token", "created_tick": 1});
    let (_, cr) = send(&app, json_post("/api/v1/escrow", body)).await;
    let eid = cr["escrow_id"].as_str().unwrap();

    // Claim
    let (status, resp) = send(&app, json_post(&format!("/api/v1/escrow/{eid}/claim"), json!({"claimant": "claim-1"}))).await;
    assert_eq!(status, StatusCode::OK, "claim: {resp:?}");
    assert_eq!(resp["status"], "claimed");

    // Complete
    let (status, resp) = send(&app, post_empty(&format!("/api/v1/escrow/{eid}/complete"))).await;
    assert_eq!(status, StatusCode::OK, "complete: {resp:?}");
    assert_eq!(resp["status"], "completed");
}

// ── Refund Escrow ──────────────────────────────────────────────

#[tokio::test]
async fn test_escrow_refund() {
    let app = test_app_with_escrow();

    send(&app, json_post("/api/v1/escrow/balance", json!({"agent": "pub-2", "amount": 500}))).await;

    let body = json!({"publisher": "pub-2", "reward": 100, "deposit": 20, "currency": "token", "created_tick": 1});
    let (_, cr) = send(&app, json_post("/api/v1/escrow", body)).await;
    let eid = cr["escrow_id"].as_str().unwrap();

    let (status, resp) = send(&app, post_empty(&format!("/api/v1/escrow/{eid}/refund"))).await;
    assert_eq!(status, StatusCode::OK, "refund: {resp:?}");
    assert_eq!(resp["status"], "refunded");
}

// ── Dispute + Resolve ──────────────────────────────────────────

#[tokio::test]
async fn test_escrow_dispute_and_resolve() {
    let app = test_app_with_escrow();

    send(&app, json_post("/api/v1/escrow/balance", json!({"agent": "pub-3", "amount": 500}))).await;
    send(&app, json_post("/api/v1/escrow/balance", json!({"agent": "claim-3", "amount": 200}))).await;

    let body = json!({"publisher": "pub-3", "reward": 100, "deposit": 20, "currency": "token", "created_tick": 1});
    let (_, cr) = send(&app, json_post("/api/v1/escrow", body)).await;
    let eid = cr["escrow_id"].as_str().unwrap();

    // Claim first
    send(&app, json_post(&format!("/api/v1/escrow/{eid}/claim"), json!({"claimant": "claim-3"}))).await;

    // Dispute
    let (status, resp) = send(&app, post_empty(&format!("/api/v1/escrow/{eid}/dispute"))).await;
    assert_eq!(status, StatusCode::OK, "dispute: {resp:?}");
    assert_eq!(resp["status"], "disputed");

    // Resolve - award to claimant
    let (status, resp) = send(&app, json_post(&format!("/api/v1/escrow/{eid}/resolve"), json!({"award_to_claimant": true}))).await;
    assert_eq!(status, StatusCode::OK, "resolve: {resp:?}");
    assert_eq!(resp["status"], "resolved");
}

// ── Set Balance ────────────────────────────────────────────────

#[tokio::test]
async fn test_escrow_set_balance() {
    let app = test_app_with_escrow();
    let body = json!({"agent": "test-agent", "amount": 9999});
    let (status, resp) = send(&app, json_post("/api/v1/escrow/balance", body)).await;
    assert_eq!(status, StatusCode::OK, "set_balance: {resp:?}");
    assert_eq!(resp["status"], "balance_set");
}

#[tokio::test]
async fn test_escrow_set_balance_without_system() {
    let app = test_app_no_escrow();
    let body = json!({"agent": "a", "amount": 100});
    let (status, _) = send(&app, json_post("/api/v1/escrow/balance", body)).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
}

// ── Operations on invalid UUID ─────────────────────────────────

#[tokio::test]
async fn test_escrow_claim_invalid_uuid() {
    let app = test_app_with_escrow();
    let (status, _) = send(&app, json_post("/api/v1/escrow/not-uuid/claim", json!({"claimant": "c"}))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_escrow_complete_invalid_uuid() {
    let app = test_app_with_escrow();
    let (status, _) = send(&app, post_empty("/api/v1/escrow/not-uuid/complete")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
