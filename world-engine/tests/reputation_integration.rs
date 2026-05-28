//! Reputation System Integration Tests — SEN-517
//!
//! Validates the reputation system wired into AppState:
//!   1. Reputation API endpoints return 200 when system is configured
//!   2. Reputation API endpoints return 503 when system is not configured
//!   3. Agent reputation increases on task completion
//!   4. Agent reputation decreases on task expiry (breach)
//!   5. Publisher reputation decreases on published task expiry
//!   6. High-value task claim is blocked for low-reputation agents
//!   7. High-value task claim is allowed for high-reputation agents
//!   8. Reputation changes affect marketplace behavior

use std::sync::Arc;

use tokio::sync::Mutex;

use agent_world_engine::api::{AppState, TestOverrides};
use agent_world_engine::economy::reputation::{ReputationConfig, ReputationSystem};
use agent_world_engine::economy::task::TaskBoard;
use agent_world_engine::wal::WAL;
use agent_world_engine::world::state::EventBus;

// ══════════════════════════════════════════════════════════════════════════
// Helpers
// ══════════════════════════════════════════════════════════════════════════

#[allow(clippy::type_complexity)]
fn build_app_with_reputation() -> (
    Arc<EventBus>,
    Arc<Mutex<TaskBoard>>,
    Arc<Mutex<ReputationSystem>>,
    axum::Router,
) {
    let dir = tempfile::TempDir::new().unwrap();
    let event_bus = Arc::new(EventBus::new(256));
    let board = Arc::new(Mutex::new(TaskBoard::with_event_bus((*event_bus).clone())));
    let wal = Arc::new(Mutex::new(WAL::new(dir.path())));

    let reputation_system = Arc::new(Mutex::new(ReputationSystem::with_event_bus(
        ReputationConfig::default(),
        event_bus.as_ref().clone(),
    )));

    let state = AppState::for_test_with(board.clone(), wal, TestOverrides {
        event_bus: Some(event_bus.clone()),
        reputation_system: Some(reputation_system.clone()),
        ..TestOverrides::default()
    });

    let app = agent_world_engine::api::build_full_router(state);
    (event_bus, board, reputation_system, app)
}

fn build_app_without_reputation() -> axum::Router {
    let dir = tempfile::TempDir::new().unwrap();
    let event_bus = Arc::new(EventBus::new(256));
    let board = Arc::new(Mutex::new(TaskBoard::with_event_bus((*event_bus).clone())));
    let wal = Arc::new(Mutex::new(WAL::new(dir.path())));

    let state = AppState::for_test_with(board, wal, TestOverrides {
        event_bus: Some(event_bus),
        ..TestOverrides::default()
    });

    agent_world_engine::api::build_full_router(state)
}

async fn start_server_with_reputation() -> (u16, Arc<EventBus>, Arc<Mutex<TaskBoard>>, Arc<Mutex<ReputationSystem>>) {
    let (event_bus, board, rep, app) = build_app_with_reputation();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    (port, event_bus, board, rep)
}

async fn start_server_without_reputation() -> u16 {
    let app = build_app_without_reputation();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    port
}

/// Create a task via the API and return its ID.
async fn create_task(port: u16, title: &str, reward: u64, publisher: &str, expires_at: Option<u64>) -> String {
    let mut body = serde_json::json!({
        "title": title,
        "description": format!("Desc for {}", title),
        "reward": reward,
        "publisher_id": publisher,
    });
    if let Some(exp) = expires_at {
        body["expires_at"] = serde_json::json!(exp);
    }

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://127.0.0.1:{}/tasks", port))
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    let json: serde_json::Value = resp.json().await.unwrap();
    json["id"].as_str().unwrap().to_string()
}

/// Drive a task through the full lifecycle (create → claim → start → submit → review → complete).
async fn complete_task_via_api(port: u16, reward: u64, publisher: &str, worker: &str) -> String {
    let task_id = create_task(port, "Test Task", reward, publisher, None).await;

    let client = reqwest::Client::new();

    // Claim
    let resp = client
        .post(format!("http://127.0.0.1:{}/tasks/{}/claim", port, task_id))
        .json(&serde_json::json!({ "assignee_id": worker }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Start
    let resp = client
        .post(format!("http://127.0.0.1:{}/tasks/{}/start", port, task_id))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Submit
    let resp = client
        .post(format!("http://127.0.0.1:{}/tasks/{}/submit", port, task_id))
        .json(&serde_json::json!({ "result": "done" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Review
    let resp = client
        .post(format!("http://127.0.0.1:{}/tasks/{}/review", port, task_id))
        .json(&serde_json::json!({ "approved": true, "reviewer_id": publisher }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Complete
    let resp = client
        .post(format!("http://127.0.0.1:{}/tasks/{}/complete", port, task_id))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    task_id
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 1: Reputation API endpoints return 200 when configured
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_reputation_api_returns_200_when_configured() {
    let (port, _, _, _) = start_server_with_reputation().await;
    let client = reqwest::Client::new();

    // GET /api/v1/reputation/:agent_id
    let resp = client
        .get(format!("http://127.0.0.1:{}/api/v1/reputation/agent-1", port))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(json["data"]["agent_id"].as_str(), Some("agent-1"));
    assert_eq!(json["data"]["reputation"].as_f64(), Some(0.0));

    // GET /api/v1/reputation/rankings
    let resp = client
        .get(format!("http://127.0.0.1:{}/api/v1/reputation/rankings", port))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // GET /api/v1/reputation/low-reputation
    let resp = client
        .get(format!("http://127.0.0.1:{}/api/v1/reputation/low-reputation", port))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // GET /api/v1/reputation/config
    let resp = client
        .get(format!("http://127.0.0.1:{}/api/v1/reputation/config", port))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(json["data"]["high_value_threshold"].as_u64(), Some(500));
    assert_eq!(json["data"]["on_time_bonus"].as_f64(), Some(1.0));
    assert_eq!(json["data"]["breach_penalty"].as_f64(), Some(5.0));
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 2: Reputation API endpoints return 503 when not configured
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_reputation_api_returns_503_when_not_configured() {
    let port = start_server_without_reputation().await;
    let client = reqwest::Client::new();

    let resp = client
        .get(format!("http://127.0.0.1:{}/api/v1/reputation/agent-1", port))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 503);

    let resp = client
        .get(format!("http://127.0.0.1:{}/api/v1/reputation/rankings", port))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 503);

    let resp = client
        .get(format!("http://127.0.0.1:{}/api/v1/reputation/config", port))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 503);
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 3: Reputation increases on task completion
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_reputation_increases_on_task_completion() {
    let (port, _, _, rep) = start_server_with_reputation().await;

    // Complete a task as worker-1
    let _task_id = complete_task_via_api(port, 100, "publisher", "worker-1").await;

    // Verify reputation increased
    let rep = rep.lock().await;
    let score = rep.get_reputation("worker-1");
    assert_eq!(score, 1.0, "reputation should increase by on_time_bonus (1.0) after completing a task");
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 4: Reputation decreases on claimed task expiry (breach)
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_reputation_decreases_on_task_breach() {
    let (port, _, _, rep) = start_server_with_reputation().await;

    // Create and claim a task, then expire it
    let task_id = create_task(port, "Breach Task", 100, "publisher", None).await;

    let client = reqwest::Client::new();

    // Claim the task
    let resp = client
        .post(format!("http://127.0.0.1:{}/tasks/{}/claim", port, task_id))
        .json(&serde_json::json!({ "assignee_id": "worker-1" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Expire the claimed task
    let resp = client
        .post(format!("http://127.0.0.1:{}/tasks/{}/expire", port, task_id))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Verify breach penalty applied
    let rep = rep.lock().await;
    let score = rep.get_reputation("worker-1");
    assert_eq!(score, -5.0, "reputation should decrease by breach_penalty (-5.0) when claimed task expires");
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 5: Publisher reputation decreases on published task expiry
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_reputation_decreases_on_published_task_expiry() {
    let (port, _, _, rep) = start_server_with_reputation().await;

    // Create a task and expire it without claiming
    let task_id = create_task(port, "Expire Task", 100, "publisher-1", None).await;

    let client = reqwest::Client::new();

    // Expire the published task (no assignee)
    let resp = client
        .post(format!("http://127.0.0.1:{}/tasks/{}/expire", port, task_id))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Verify publisher penalty applied
    let rep = rep.lock().await;
    let score = rep.get_reputation("publisher-1");
    assert_eq!(score, -2.0, "publisher reputation should decrease by expiry_penalty (-2.0) when published task expires");
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 6: High-value task claim blocked for low-reputation agent
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_high_value_claim_blocked_for_low_reputation() {
    let (port, _, _, _rep) = start_server_with_reputation().await;

    // Create a high-value task (reward >= 500, the default high_value_threshold)
    let task_id = create_task(port, "High Value Task", 500, "publisher", None).await;

    // Worker has 0 reputation (below min_reputation_for_high_value = 10.0)
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://127.0.0.1:{}/tasks/{}/claim", port, task_id))
        .json(&serde_json::json!({ "assignee_id": "newbie" }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 403, "low-reputation agent should be forbidden from claiming high-value tasks");

    // Verify the error message
    let json: serde_json::Value = resp.json().await.unwrap();
    let error = json["error"].as_str().unwrap();
    assert!(error.contains("reputation too low"), "error should mention low reputation, got: {}", error);
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 7: High-value task claim allowed for high-reputation agent
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_high_value_claim_allowed_for_high_reputation() {
    let (port, _, _, rep) = start_server_with_reputation().await;

    // Build up reputation for worker-1 by completing 15 tasks
    {
        let mut rep = rep.lock().await;
        rep.set_reputation("worker-1", 15.0, 1);
    }

    // Create a high-value task
    let task_id = create_task(port, "High Value Task", 500, "publisher", None).await;

    // worker-1 should be allowed to claim
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://127.0.0.1:{}/tasks/{}/claim", port, task_id))
        .json(&serde_json::json!({ "assignee_id": "worker-1" }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200, "high-reputation agent should be allowed to claim high-value tasks");
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 8: Reputation accumulates across multiple task completions
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_reputation_accumulates_across_tasks() {
    let (port, _, _, rep) = start_server_with_reputation().await;

    // Complete 3 tasks as worker-1
    complete_task_via_api(port, 100, "publisher", "worker-1").await;
    complete_task_via_api(port, 200, "publisher", "worker-1").await;
    complete_task_via_api(port, 300, "publisher", "worker-1").await;

    // Verify accumulated reputation
    let rep = rep.lock().await;
    let score = rep.get_reputation("worker-1");
    assert_eq!(score, 3.0, "reputation should accumulate 1.0 per task completion");
    drop(rep);

    // Verify via API
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://127.0.0.1:{}/api/v1/reputation/worker-1", port))
        .send()
        .await
        .unwrap();
    let json: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(json["data"]["reputation"].as_f64(), Some(3.0));
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 9: Reputation rankings reflect completed tasks
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_reputation_rankings_reflect_completed_tasks() {
    let (port, _, _, _) = start_server_with_reputation().await;

    // Complete tasks for different agents
    complete_task_via_api(port, 100, "publisher", "worker-a").await;
    complete_task_via_api(port, 100, "publisher", "worker-b").await;
    complete_task_via_api(port, 100, "publisher", "worker-b").await;

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://127.0.0.1:{}/api/v1/reputation/rankings", port))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let json: serde_json::Value = resp.json().await.unwrap();
    let rankings = json["data"].as_array().unwrap();
    assert_eq!(rankings.len(), 2);

    // worker-b should be ranked higher (2 completions = 2.0 reputation)
    assert_eq!(rankings[0]["agent_id"].as_str(), Some("worker-b"));
    assert_eq!(rankings[0]["reputation"].as_f64(), Some(2.0));
    assert_eq!(rankings[0]["rank"].as_u64(), Some(1));

    // worker-a should be second (1 completion = 1.0 reputation)
    assert_eq!(rankings[1]["agent_id"].as_str(), Some("worker-a"));
    assert_eq!(rankings[1]["reputation"].as_f64(), Some(1.0));
    assert_eq!(rankings[1]["rank"].as_u64(), Some(2));
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 10: Low-reputation agents endpoint filters correctly
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_low_reputation_endpoint_filters_correctly() {
    let (port, _, _, rep) = start_server_with_reputation().await;

    // Set some agent reputations
    {
        let mut rep = rep.lock().await;
        rep.set_reputation("good-agent", 50.0, 1);
        rep.set_reputation("bad-agent", -20.0, 1);
        rep.set_reputation("ok-agent", 5.0, 1);
    }

    let client = reqwest::Client::new();

    // Get agents below 0.0 (default threshold is -10.0, so use custom)
    let resp = client
        .get(format!("http://127.0.0.1:{}/api/v1/reputation/low-reputation?threshold=0.0", port))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let json: serde_json::Value = resp.json().await.unwrap();
    let agents = json["data"].as_array().unwrap();
    assert_eq!(agents.len(), 1);
    // The one below 0 is bad-agent
    assert!(agents[0].as_array().unwrap()[0].as_str() == Some("bad-agent"));
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 11: Low-value task claim always allowed regardless of reputation
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_low_value_task_claim_always_allowed() {
    let (port, _, _, _) = start_server_with_reputation().await;

    // Create a low-value task (reward < 500)
    let task_id = create_task(port, "Low Value Task", 100, "publisher", None).await;

    // Any agent should be able to claim it, even with 0 reputation
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://127.0.0.1:{}/tasks/{}/claim", port, task_id))
        .json(&serde_json::json!({ "assignee_id": "newbie" }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200, "low-value task should be claimable regardless of reputation");
}
