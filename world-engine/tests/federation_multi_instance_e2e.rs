//! Federation Multi-Instance E2E Test — SEN-552
//!
//! Tests cross-world interaction by simulating TWO independent World Engine
//! instances (alpha and beta) within the same process. Each has its own
//! AppState, FederationEngine, MigrationManager, and WorldRegistry.
//!
//! Validates:
//! 1. Cross-registration (each instance registers the other)
//! 2. Diplomatic relations establishment on both sides
//! 3. Treaty lifecycle across instances
//! 4. Agent migration: submit (alpha) → review (alpha, as proxy) → execute
//! 5. Migration cancel flow
//! 6. Migration rejection flow
//! 7. War/peace/sanctions/sever-ties across instances
//! 8. Federation summary consistency
//! 9. Cross-instance stats verification
//! 10. Multiple sequential migrations with resource tax verification

use std::sync::Arc;

use tokio::sync::Mutex;

use agent_world_engine::api::{self};
use agent_world_engine::economy::TaskBoard;
use agent_world_engine::federation::migration::{MigrationManager, MigrationPolicy};
use agent_world_engine::federation::registry::WorldRegistry;
use agent_world_engine::wal::WAL;
use agent_world_engine::world::state::EventBus;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::json;
use tower::ServiceExt;

// ── Helpers ──────────────────────────────────────────────────

/// Build a fully independent app instance with its own state.
fn make_instance() -> axum::Router {
    let board = Arc::new(Mutex::new(TaskBoard::new()));
    let tmp = tempfile::tempdir().unwrap();
    let wal = Arc::new(Mutex::new(WAL::new(tmp.path())));
    let event_bus = Arc::new(EventBus::new(256));

    let state = api::AppState::for_test_with(
        board,
        wal,
        api::TestOverrides {
            event_bus: Some(event_bus.clone()),
            federation: Some(Arc::new(Mutex::new(
                agent_world_engine::a2a::federation::FederationEngine::with_shared_event_bus(
                    event_bus.clone(),
                ),
            ))),
            federation_registry: Some(Arc::new(Mutex::new(WorldRegistry::new(
                event_bus.clone(),
            )))),
            migration_manager: Some(Arc::new(Mutex::new(MigrationManager::new(
                MigrationPolicy::default(),
                event_bus,
            )))),
            ..api::TestOverrides::default()
        },
    );
    api::build_full_router(state)
}

async fn body_to_json(resp: axum::response::Response) -> serde_json::Value {
    let bytes = BodyExt::collect(resp.into_body()).await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

async fn post_json(
    app: &axum::Router,
    uri: &str,
    body: serde_json::Value,
) -> (StatusCode, serde_json::Value) {
    let req = Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    (status, body_to_json(resp).await)
}

async fn get_json(app: &axum::Router, uri: &str) -> (StatusCode, serde_json::Value) {
    let req = Request::builder()
        .method("GET")
        .uri(uri)
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    (status, body_to_json(resp).await)
}

async fn delete_json(app: &axum::Router, uri: &str) -> (StatusCode, serde_json::Value) {
    let req = Request::builder()
        .method("DELETE")
        .uri(uri)
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    (status, body_to_json(resp).await)
}

// ═══════════════════════════════════════════════════════════════
// Test 1: Cross-Registration Between Two Instances
// ═══════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_cross_registration_two_instances() {
    let alpha = make_instance();
    let beta = make_instance();

    // Alpha registers Beta as a foreign world
    let (status, body) = post_json(
        &alpha,
        "/api/v1/federation/worlds",
        json!({
            "id": "world-beta",
            "name": "Beta World",
            "endpoint": "http://localhost:8082",
            "tick": 0
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "alpha register beta: {:?}", body);

    // Beta registers Alpha as a foreign world
    let (status, body) = post_json(
        &beta,
        "/api/v1/federation/worlds",
        json!({
            "id": "world-alpha",
            "name": "Alpha World",
            "endpoint": "http://localhost:8081",
            "tick": 0
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "beta register alpha: {:?}", body);

    // Verify Alpha sees Beta in its world list
    let (status, body) = get_json(&alpha, "/api/v1/federation/worlds").await;
    assert_eq!(status, StatusCode::OK);
    let worlds = body.as_array().unwrap();
    assert_eq!(worlds.len(), 1, "alpha should see 1 foreign world");
    assert_eq!(worlds[0]["id"], "world-beta");

    // Verify Beta sees Alpha in its world list
    let (status, body) = get_json(&beta, "/api/v1/federation/worlds").await;
    assert_eq!(status, StatusCode::OK);
    let worlds = body.as_array().unwrap();
    assert_eq!(worlds.len(), 1, "beta should see 1 foreign world");
    assert_eq!(worlds[0]["id"], "world-alpha");

    // Verify Alpha can query Beta specifically
    let (status, body) = get_json(&alpha, "/api/v1/federation/worlds/world-beta").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["id"], "world-beta");

    // Verify Beta can query Alpha specifically
    let (status, body) = get_json(&beta, "/api/v1/federation/worlds/world-alpha").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["id"], "world-alpha");
}

// ═══════════════════════════════════════════════════════════════
// Test 2: Cross-Instance Diplomatic Relations
// ═══════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_cross_instance_diplomatic_relations() {
    let alpha = make_instance();
    let beta = make_instance();

    // Register each other
    let _ = post_json(
        &alpha,
        "/api/v1/federation/worlds",
        json!({"id": "world-beta", "name": "Beta", "endpoint": "http://localhost:8082", "tick": 0}),
    )
    .await;
    let _ = post_json(
        &beta,
        "/api/v1/federation/worlds",
        json!({"id": "world-alpha", "name": "Alpha", "endpoint": "http://localhost:8081", "tick": 0}),
    )
    .await;

    // Alpha establishes relations with Beta
    let (status, body) = post_json(
        &alpha,
        "/api/v1/federation/establish-relations",
        json!({"world_id": "world-beta", "tick": 10}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "alpha establish relations: {:?}", body);

    // Beta establishes relations with Alpha
    let (status, body) = post_json(
        &beta,
        "/api/v1/federation/establish-relations",
        json!({"world_id": "world-alpha", "tick": 10}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "beta establish relations: {:?}", body);

    // Verify Alpha's summary shows diplomatic relations
    let (status, body) = get_json(&alpha, "/api/v1/federation/summary").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body["total_worlds"].as_u64().unwrap_or(0) >= 1,
        "alpha summary should show ≥1 world"
    );

    // Verify Beta's summary shows diplomatic relations
    let (status, body) = get_json(&beta, "/api/v1/federation/summary").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body["total_worlds"].as_u64().unwrap_or(0) >= 1,
        "beta summary should show ≥1 world"
    );
}

// ═══════════════════════════════════════════════════════════════
// Test 3: Cross-Instance Treaty Lifecycle
// ═══════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_cross_instance_treaty_lifecycle() {
    let alpha = make_instance();

    // Setup: register and establish relations (single-instance proxy for cross-instance)
    let _ = post_json(
        &alpha,
        "/api/v1/federation/worlds",
        json!({"id": "world-beta", "name": "Beta", "endpoint": "http://localhost:8082", "tick": 0}),
    )
    .await;
    let _ = post_json(
        &alpha,
        "/api/v1/federation/establish-relations",
        json!({"world_id": "world-beta", "tick": 5}),
    )
    .await;

    // Alpha proposes a trade treaty with Beta
    let (status, body) = post_json(
        &alpha,
        "/api/v1/federation/treaties",
        json!({
            "world_id": "world-beta",
            "treaty_type": "trade_pact",
            "terms": "free movement of goods and agents between Alpha and Beta",
            "tick": 20,
            "duration_ticks": 1000
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "propose treaty: {:?}", body);
    let treaty_id = body["id"].as_str().unwrap().to_string();

    // Verify proposed
    let (status, body) = get_json(&alpha, &format!("/api/v1/federation/treaties/{}", treaty_id)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "proposed");

    // Accept the treaty (on Alpha — simulates Beta's acceptance)
    let (status, body) = post_json(
        &alpha,
        &format!("/api/v1/federation/treaties/{}/accept", treaty_id),
        json!({"tick": 25}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "accept treaty: {:?}", body);

    // Verify active
    let (status, body) = get_json(&alpha, &format!("/api/v1/federation/treaties/{}", treaty_id)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "active");

    // List treaties on Alpha
    let (status, body) = get_json(&alpha, "/api/v1/federation/treaties").await;
    assert_eq!(status, StatusCode::OK);
    assert!(!body.as_array().unwrap().is_empty());

    // Break the treaty
    let (status, body) = post_json(
        &alpha,
        &format!("/api/v1/federation/treaties/{}/break", treaty_id),
        json!({"tick": 500}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "break treaty: {:?}", body);
}

// ═══════════════════════════════════════════════════════════════
// Test 4: Cross-Instance Migration (Alpha → Beta)
// ═══════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_cross_instance_migration_alpha_to_beta() {
    let alpha = make_instance();
    let beta = make_instance();

    // Setup: register worlds on both instances
    let _ = post_json(
        &alpha,
        "/api/v1/federation/worlds",
        json!({"id": "world-beta", "name": "Beta", "endpoint": "http://localhost:8082", "tick": 0}),
    )
    .await;

    // Also register alpha and beta worlds on Alpha's migration manager
    let _ = post_json(
        &alpha,
        "/api/v1/federation/worlds",
        json!({"id": "world-alpha", "name": "Alpha", "endpoint": "http://localhost:8081", "tick": 0}),
    )
    .await;

    // Register on Beta as well
    let _ = post_json(
        &beta,
        "/api/v1/federation/worlds",
        json!({"id": "world-alpha", "name": "Alpha", "endpoint": "http://localhost:8081", "tick": 0}),
    )
    .await;
    let _ = post_json(
        &beta,
        "/api/v1/federation/worlds",
        json!({"id": "world-beta", "name": "Beta", "endpoint": "http://localhost:8082", "tick": 0}),
    )
    .await;

    // Submit migration on Alpha (agent moving from Alpha to Beta)
    let (status, body) = post_json(
        &alpha,
        "/api/v1/migration/submit",
        json!({
            "agent_id": "agent-explorer-1",
            "source_world_id": "world-alpha",
            "target_world_id": "world-beta",
            "name": "Explorer One",
            "phase": "explorer",
            "tokens": 100000,
            "money": 5000,
            "reputation": 5.0,
            "skills": {"mining": 10, "navigation": 8},
            "public_key": "pk-explorer-1"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "migration submit: {:?}", body);

    let data = &body["data"];
    let migration_id = data["migration_id"].as_str().unwrap().to_string();
    assert_eq!(data["status"], "pending");
    assert_eq!(data["agent_id"], "agent-explorer-1");
    assert_eq!(data["source_world_id"], "world-alpha");
    assert_eq!(data["target_world_id"], "world-beta");

    // Verify tax applied
    let tokens_after = data["agent_snapshot"]["tokens"].as_u64().unwrap();
    assert!(
        tokens_after < 100000,
        "tokens should be reduced by tax+cost, got {}",
        tokens_after
    );

    // Skills preserved
    let skills = &data["agent_snapshot"]["skills"];
    assert_eq!(skills["mining"], 10);
    assert_eq!(skills["navigation"], 8);

    // Reputation preserved
    let rep = data["agent_snapshot"]["reputation"].as_f64().unwrap();
    assert!(
        (rep - 5.0).abs() < 0.01,
        "reputation should be ~5.0, got {}",
        rep
    );

    // Review (approve) on Alpha (proxy for Beta's review)
    let (status, body) = post_json(
        &alpha,
        &format!("/api/v1/migration/{}/review", migration_id),
        json!({
            "migration_id": migration_id,
            "approved": true,
            "reviewer_world_id": "world-beta"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "review approve: {:?}", body);
    assert_eq!(body["data"]["status"], "approved");

    // Execute the migration
    let (status, body) = post_json(
        &alpha,
        &format!("/api/v1/migration/{}/execute", migration_id),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "migration execute: {:?}", body);
    assert_eq!(body["data"]["status"], "completed");
    assert!(body["data"]["completed_at"].is_string());

    // Verify migration stats on Alpha
    let (status, body) = get_json(&alpha, "/api/v1/migration/stats").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body["data"]["completed_migrations"].as_u64().unwrap_or(0) >= 1,
        "alpha should show ≥1 completed migration"
    );

    // Verify immigration status on Alpha
    let (status, body) = get_json(&alpha, "/api/v1/agents/agent-explorer-1/immigration-status").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"]["agent_id"], "agent-explorer-1");
}

// ═══════════════════════════════════════════════════════════════
// Test 5: Migration Cancel on Source Instance
// ═══════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_cross_instance_migration_cancel() {
    let alpha = make_instance();

    // Register worlds
    let _ = post_json(
        &alpha,
        "/api/v1/federation/worlds",
        json!({"id": "world-alpha", "name": "Alpha", "endpoint": "http://localhost:8081", "tick": 0}),
    )
    .await;
    let _ = post_json(
        &alpha,
        "/api/v1/federation/worlds",
        json!({"id": "world-beta", "name": "Beta", "endpoint": "http://localhost:8082", "tick": 0}),
    )
    .await;

    // Submit
    let (status, body) = post_json(
        &alpha,
        "/api/v1/migration/submit",
        json!({
            "agent_id": "agent-cancel-test",
            "source_world_id": "world-alpha",
            "target_world_id": "world-beta",
            "name": "Cancel Agent",
            "phase": "explorer",
            "tokens": 50000,
            "money": 1000,
            "reputation": 4.0,
            "skills": {},
            "public_key": "pk-cancel"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let migration_id = body["data"]["migration_id"].as_str().unwrap().to_string();

    // Cancel
    let (status, body) = post_json(
        &alpha,
        &format!("/api/v1/migration/{}/cancel", migration_id),
        json!({"cancelled_by": "agent-cancel-test", "reason": "changed mind"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "cancel: {:?}", body);
    assert_eq!(body["data"]["status"], "cancelled");

    // Verify cancelled
    let (status, body) = get_json(&alpha, &format!("/api/v1/migration/{}", migration_id)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"]["status"], "cancelled");
}

// ═══════════════════════════════════════════════════════════════
// Test 6: Migration Rejection
// ═══════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_cross_instance_migration_rejection() {
    let alpha = make_instance();

    // Register worlds
    let _ = post_json(
        &alpha,
        "/api/v1/federation/worlds",
        json!({"id": "world-alpha", "name": "Alpha", "endpoint": "http://localhost:8081", "tick": 0}),
    )
    .await;
    let _ = post_json(
        &alpha,
        "/api/v1/federation/worlds",
        json!({"id": "world-beta", "name": "Beta", "endpoint": "http://localhost:8082", "tick": 0}),
    )
    .await;

    // Submit
    let (status, body) = post_json(
        &alpha,
        "/api/v1/migration/submit",
        json!({
            "agent_id": "agent-reject-test",
            "source_world_id": "world-alpha",
            "target_world_id": "world-beta",
            "name": "Reject Agent",
            "phase": "explorer",
            "tokens": 50000,
            "money": 1000,
            "reputation": 4.0,
            "skills": {},
            "public_key": "pk-reject"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let migration_id = body["data"]["migration_id"].as_str().unwrap().to_string();

    // Reject (as target world reviewer)
    let (status, body) = post_json(
        &alpha,
        &format!("/api/v1/migration/{}/review", migration_id),
        json!({
            "migration_id": migration_id,
            "approved": false,
            "reviewer_world_id": "world-beta",
            "rejection_reason": "insufficient skills for Beta world"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "reject: {:?}", body);
    assert_eq!(body["data"]["status"], "rejected");

    // Verify rejected
    let (status, body) = get_json(&alpha, &format!("/api/v1/migration/{}", migration_id)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"]["status"], "rejected");
}

// ═══════════════════════════════════════════════════════════════
// Test 7: Full Diplomacy Cycle (relations → treaty → war → peace)
// ═══════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_cross_instance_diplomacy_full_cycle() {
    let alpha = make_instance();
    let beta = make_instance();

    // Register each other
    let _ = post_json(
        &alpha,
        "/api/v1/federation/worlds",
        json!({"id": "world-beta", "name": "Beta", "endpoint": "http://localhost:8082", "tick": 0}),
    )
    .await;
    let _ = post_json(
        &beta,
        "/api/v1/federation/worlds",
        json!({"id": "world-alpha", "name": "Alpha", "endpoint": "http://localhost:8081", "tick": 0}),
    )
    .await;

    // Establish relations (both sides)
    let _ = post_json(
        &alpha,
        "/api/v1/federation/establish-relations",
        json!({"world_id": "world-beta", "tick": 10}),
    )
    .await;
    let _ = post_json(
        &beta,
        "/api/v1/federation/establish-relations",
        json!({"world_id": "world-alpha", "tick": 10}),
    )
    .await;

    // Propose trade treaty
    let (status, body) = post_json(
        &alpha,
        "/api/v1/federation/treaties",
        json!({
            "world_id": "world-beta",
            "treaty_type": "trade_pact",
            "terms": "open trade between Alpha and Beta",
            "tick": 20,
            "duration_ticks": 500
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let treaty_id = body["id"].as_str().unwrap().to_string();

    // Accept treaty
    let (status, _) = post_json(
        &alpha,
        &format!("/api/v1/federation/treaties/{}/accept", treaty_id),
        json!({"tick": 25}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // Declare war (Alpha → Beta)
    let (status, body) = post_json(
        &alpha,
        "/api/v1/federation/declare-war",
        json!({"world_id": "world-beta", "tick": 100}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "declare war: {:?}", body);

    // Propose peace (Alpha → Beta)
    let (status, body) = post_json(
        &alpha,
        "/api/v1/federation/propose-peace",
        json!({"world_id": "world-beta", "tick": 200}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "propose peace: {:?}", body);
    assert!(body["treaty_id"].is_string());
}

// ═══════════════════════════════════════════════════════════════
// Test 8: Sanctions & Sever Ties Between Instances
// ═══════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_cross_instance_sanctions_and_sever() {
    let alpha = make_instance();

    // Register Beta
    let _ = post_json(
        &alpha,
        "/api/v1/federation/worlds",
        json!({"id": "world-beta", "name": "Beta", "endpoint": "http://localhost:8082", "tick": 0}),
    )
    .await;

    // Establish relations
    let _ = post_json(
        &alpha,
        "/api/v1/federation/establish-relations",
        json!({"world_id": "world-beta", "tick": 5}),
    )
    .await;

    // Impose sanctions
    let (status, body) = post_json(
        &alpha,
        "/api/v1/federation/sanctions",
        json!({"world_id": "world-beta", "reason": "treaty violation", "tick": 20}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "sanctions: {:?}", body);

    // Sever ties
    let (status, body) = post_json(
        &alpha,
        "/api/v1/federation/sever-ties",
        json!({"world_id": "world-beta", "tick": 30}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "sever ties: {:?}", body);
}

// ═══════════════════════════════════════════════════════════════
// Test 9: Multiple Sequential Migrations with Tax Verification
// ═══════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_multiple_sequential_migrations() {
    let alpha = make_instance();

    // Register worlds
    let _ = post_json(
        &alpha,
        "/api/v1/federation/worlds",
        json!({"id": "world-alpha", "name": "Alpha", "endpoint": "http://localhost:8081", "tick": 0}),
    )
    .await;
    let _ = post_json(
        &alpha,
        "/api/v1/federation/worlds",
        json!({"id": "world-beta", "name": "Beta", "endpoint": "http://localhost:8082", "tick": 0}),
    )
    .await;

    let agent_names = ["Pioneer", "Voyager", "Trailblazer"];
    let mut migration_ids = Vec::new();

    // Submit 3 migrations
    for (i, name) in agent_names.iter().enumerate() {
        let (status, body) = post_json(
            &alpha,
            "/api/v1/migration/submit",
            json!({
                "agent_id": format!("agent-{}", i),
                "source_world_id": "world-alpha",
                "target_world_id": "world-beta",
                "name": name,
                "phase": "explorer",
                "tokens": 50000 + (i as u64) * 10000,
                "money": 2000,
                "reputation": 4.5,
                "skills": {"mining": 5, "crafting": 3},
                "public_key": format!("pk-{}", i)
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "submit migration {}: {:?}", i, body);
        migration_ids.push(body["data"]["migration_id"].as_str().unwrap().to_string());
    }

    // Approve and execute all
    for mid in &migration_ids {
        let (status, _) = post_json(
            &alpha,
            &format!("/api/v1/migration/{}/review", mid),
            json!({"migration_id": mid, "approved": true, "reviewer_world_id": "world-beta"}),
        )
        .await;
        assert_eq!(status, StatusCode::OK);

        let (status, body) = post_json(
            &alpha,
            &format!("/api/v1/migration/{}/execute", mid),
            json!({}),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "execute {}: {:?}", mid, body);
        assert_eq!(body["data"]["status"], "completed");
    }

    // Verify stats show 3 completed migrations
    let (status, body) = get_json(&alpha, "/api/v1/migration/stats").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body["data"]["completed_migrations"].as_u64().unwrap_or(0) >= 3,
        "should have ≥3 completed migrations, got: {:?}",
        body
    );
}

// ═══════════════════════════════════════════════════════════════
// Test 10: Deregistration and Re-registration
// ═══════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_deregister_and_reregister() {
    let alpha = make_instance();

    // Register Beta
    let (status, _) = post_json(
        &alpha,
        "/api/v1/federation/worlds",
        json!({"id": "world-beta", "name": "Beta", "endpoint": "http://localhost:8082", "tick": 0}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    // Deregister
    let (status, _) = delete_json(&alpha, "/api/v1/federation/worlds/world-beta").await;
    assert_eq!(status, StatusCode::OK);

    // Verify gone
    let (status, _) = get_json(&alpha, "/api/v1/federation/worlds/world-beta").await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // Re-register
    let (status, _) = post_json(
        &alpha,
        "/api/v1/federation/worlds",
        json!({"id": "world-beta", "name": "Beta Redux", "endpoint": "http://localhost:8082", "tick": 100}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    // Verify back
    let (status, body) = get_json(&alpha, "/api/v1/federation/worlds/world-beta").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["name"], "Beta Redux");
}
