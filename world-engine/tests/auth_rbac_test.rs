//! Integration tests for the auth/RBAC system (SEN-482).
//!
//! Covers:
//! - Registration + login flow
//! - JWT token validation
//! - Role-based access control (key role operations)
//! - Identity spoofing prevention (body.human_id ignored)
//! - Unauthenticated access rejection
//! - human_stats deadlock fix verification
//! - Password security (unique salts, min length)

use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::Mutex;
use tokio::sync::watch;

use agent_world_engine::api::{AppState, build_full_router};
use agent_world_engine::auth::{AuthStore, HumanRole, Capability};
use agent_world_engine::human::store::HumanParticipationStore;
use agent_world_engine::economy::task::TaskBoard;
use agent_world_engine::wal::WAL;
use agent_world_engine::world::state::EventBus;
use agent_world_engine::world::map::building::BuildingManager;

use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use serde_json::json;
use tower::ServiceExt;

fn test_app() -> Router {
    let board = Arc::new(Mutex::new(TaskBoard::new()));
    let wal = Arc::new(Mutex::new(WAL::new("./data/test-wal")));
    let (tick_tx, tick_rx) = watch::channel(0u64);
    let auth_store = Arc::new(Mutex::new(AuthStore::new("test-jwt-secret")));

    let state = AppState {
        board,
        wal,
        event_bus: Arc::new(EventBus::new(256)),
        agents: Arc::new(Mutex::new(Vec::new())),
        messages: Arc::new(Mutex::new(Vec::new())),
        tick_tx,
        tick_rx,
        snapshot_store: None,
        marketplace: None,
        reputation_system: None,
        org_store: None,
        stock_market: None,
        governance: None,
        banking_system: None,
        trace_store: None,
        external_agents: Arc::new(Mutex::new(HashMap::new())),
        governance_metrics: None,
        building_manager: Arc::new(Mutex::new(BuildingManager::new())),
        human_store: Arc::new(Mutex::new(HumanParticipationStore::new())),
        auth_store,
        investment_system: None,
        rule_engine: None,
        federation: None,
        federation_registry: None,
        migration_manager: None,
        api_key_store: None,
        experiment_store: Arc::new(Mutex::new(Vec::new())),
    };

    build_full_router(state)
}

async fn register_and_login(app: &Router, username: &str, role: &str) -> (String, String) {
    // Register
    let body = json!({"username": username, "password": "password123", "role": role});
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/register")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED, "Register {} failed", username);

    // Login
    let body = json!({"username": username, "password": "password123"});
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/login")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "Login {} failed", username);

    let body_bytes = axum::body::to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
    let resp_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    let user_id = resp_json["user"]["id"].as_str().unwrap().to_string();
    let token = resp_json["token"].as_str().unwrap().to_string();
    (user_id, token)
}

// ── Auth Flow Tests ────────────────────────────────────────────

#[tokio::test]
async fn test_register_and_login() {
    let app = test_app();
    let (user_id, token) = register_and_login(&app, "alice", "investor").await;
    assert!(!user_id.is_empty());
    assert!(!token.is_empty());
}

#[tokio::test]
async fn test_register_duplicate_fails() {
    let app = test_app();
    register_and_login(&app, "bob", "observer").await;

    let body = json!({"username": "bob", "password": "password123", "role": "investor"});
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/register")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_login_wrong_password() {
    let app = test_app();
    register_and_login(&app, "charlie", "observer").await;

    let body = json!({"username": "charlie", "password": "wrongpassword"});
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/login")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_auth_me_requires_token() {
    let app = test_app();
    let req = Request::builder()
        .uri("/api/v1/auth/me")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_auth_me_with_valid_token() {
    let app = test_app();
    let (_, token) = register_and_login(&app, "dave", "creator").await;

    let req = Request::builder()
        .uri("/api/v1/auth/me")
        .header("authorization", format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_invalid_token_rejected() {
    let app = test_app();
    let req = Request::builder()
        .uri("/api/v1/auth/me")
        .header("authorization", "Bearer totally.invalid.token")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

// ── RBAC Tests ─────────────────────────────────────────────────

#[tokio::test]
async fn test_rbac_invest_requires_investor_role() {
    let app = test_app();
    let (_, observer_token) = register_and_login(&app, "obs1", "observer").await;
    let (_, investor_token) = register_and_login(&app, "inv1", "investor").await;

    // Observer tries to invest → 403
    let body = json!({"human_id": "fake", "agent_id": "agent-1", "amount": 100});
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/human/portfolio/invest")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {}", observer_token))
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // Investor tries to invest → 404 (agent doesn't exist) but NOT 403
    let body = json!({"human_id": "fake", "agent_id": "agent-1", "amount": 100});
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/human/portfolio/invest")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {}", investor_token))
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND); // Agent not found, but auth passed
}

#[tokio::test]
async fn test_rbac_bounty_requires_task_publisher() {
    let app = test_app();
    let (_, observer_token) = register_and_login(&app, "obs2", "observer").await;
    let (_, tasker_token) = register_and_login(&app, "tasker1", "task_publisher").await;

    // Observer tries to create bounty → 403
    let body = json!({"human_id": "fake", "title": "Test Bounty", "reward": 100});
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/human/bounties")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {}", observer_token))
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // TaskPublisher creates bounty → 201
    let body = json!({"human_id": "fake", "title": "Test Bounty", "reward": 100});
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/human/bounties")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {}", tasker_token))
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn test_rbac_creator_has_all_permissions() {
    let app = test_app();
    let (_, creator_token) = register_and_login(&app, "creator1", "creator").await;

    // Creator can create bounty (requires TaskPublisher capability)
    let body = json!({"human_id": "fake", "title": "Creator Bounty", "reward": 500});
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/human/bounties")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {}", creator_token))
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
}

// ── Identity Spoofing Prevention ───────────────────────────────

#[tokio::test]
async fn test_identity_spoofing_prevented_in_oracle() {
    let app = test_app();
    let (real_user_id, token) = register_and_login(&app, "real_user", "observer").await;

    // Attacker tries to send oracle with different human_id
    let body = json!({
        "human_id": "victim-user-id",
        "oracle_type": "guidance",
        "target_agent_id": "agent-1",
        "content": "malicious oracle"
    });
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/human/oracles")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {}", token))
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Verify the oracle used the authenticated user's ID, not the spoofed one
    let body_bytes = axum::body::to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
    let oracle: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_ne!(oracle["human_id"].as_str().unwrap(), "victim-user-id");
    assert_eq!(oracle["human_id"].as_str().unwrap(), real_user_id);
}

// ── Unauthenticated Access ─────────────────────────────────────

#[tokio::test]
async fn test_human_send_oracle_requires_auth() {
    let app = test_app();
    let body = json!({
        "human_id": "anyone",
        "oracle_type": "guidance",
        "target_agent_id": "agent-1",
        "content": "anonymous oracle"
    });
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/human/oracles")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_human_claim_agent_requires_auth() {
    let app = test_app();
    let body = json!({"human_id": "anyone", "agent_id": "agent-1"});
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/human/agents/claim")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_human_stats_no_deadlock() {
    let app = test_app();
    // human_stats previously deadlocked due to double Mutex lock.
    // This test verifies it returns successfully.
    let req = Request::builder()
        .uri("/api/v1/human/stats")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

// ── List Users (Creator only) ──────────────────────────────────

#[tokio::test]
async fn test_list_users_requires_creator() {
    let app = test_app();
    let (_, observer_token) = register_and_login(&app, "obs3", "observer").await;
    let (_, creator_token) = register_and_login(&app, "creator2", "creator").await;

    // Observer → 403
    let req = Request::builder()
        .uri("/api/v1/auth/users")
        .header("authorization", format!("Bearer {}", observer_token))
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // Creator → 200
    let req = Request::builder()
        .uri("/api/v1/auth/users")
        .header("authorization", format!("Bearer {}", creator_token))
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

// ── Password Security (unit tests via AuthStore directly) ──────

#[tokio::test]
async fn test_password_has_unique_salts() {
    let mut store = AuthStore::new("test-secret");
    let u1 = store.register("salt_test_1", "same_pass1", HumanRole::Observer).unwrap();
    let u2 = store.register("salt_test_2", "same_pass1", HumanRole::Observer).unwrap();
    assert_ne!(u1.password_hash, u2.password_hash, "Same password should produce different hashes");
}

#[tokio::test]
async fn test_password_min_length() {
    let mut store = AuthStore::new("test-secret");
    let result = store.register("short_pw_user", "abc", HumanRole::Observer);
    assert!(result.is_err(), "Short password should be rejected");
}

// ── RBAC Capability Mapping Unit Tests ─────────────────────────

#[test]
fn test_observer_has_minimal_capabilities() {
    let caps = HumanRole::Observer.capabilities();
    assert!(caps.contains(&Capability::ViewWorld));
    assert!(!caps.contains(&Capability::Invest));
    assert!(!caps.contains(&Capability::PublishTasks));
}

#[test]
fn test_creator_has_all_capabilities() {
    let caps = HumanRole::Creator.capabilities();
    assert_eq!(caps.len(), 9, "Creator should have all 9 capabilities");
}

#[test]
fn test_investor_has_invest_capability() {
    let caps = HumanRole::Investor.capabilities();
    assert!(caps.contains(&Capability::Invest));
    assert!(caps.contains(&Capability::ViewWorld));
}
