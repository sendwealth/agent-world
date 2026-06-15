//! Human-as-Agent (Phase 5.5) E2E Integration Tests.
//!
//! Tests the full flow:
//! - POST /api/v1/human/incarnate — register as agent
//! - POST /api/v1/human/agents/:id/action — submit actions
//! - Action queue drain + execution
//! - Timeout fallback to AI decision
//! - Survival rules: token burn, death, newbie protection
//! - SSE events endpoint

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tokio::sync::{watch, Mutex};
use tower::ServiceExt;

use agent_world_engine::api::create_router_for_test;
use agent_world_engine::economy::task::TaskBoard;
use agent_world_engine::human::{HumanActionQueue, HumanActionType};
use agent_world_engine::wal::WAL;
use agent_world_engine::world::state::EventBus;

// ── Helpers ──────────────────────────────────────────────────

/// Build a test router with a human action queue accessible via AppState.
fn create_test_app() -> axum::Router {
    let board = Arc::new(Mutex::new(TaskBoard::new()));
    let dir = tempfile::TempDir::new().unwrap();
    let wal = Arc::new(Mutex::new(WAL::new(dir.path())));
    std::mem::forget(dir);
    let event_bus = EventBus::new(256);
    let (tx, rx) = watch::channel(0u64);
    create_router_for_test(board, wal, Arc::new(event_bus), tx, rx)
}

#[allow(dead_code)]
async fn body_json(body: Body) -> Value {
    let bytes = body.collect().await.unwrap().to_bytes();
    String::from_utf8(bytes.to_vec())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| json!({}))
}

fn make_request(method: Method, uri: &str, body: Option<Value>) -> Request<Body> {
    let body = body.map(|v| v.to_string()).unwrap_or_default();
    Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body))
        .unwrap()
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 1: HumanActionQueue unit — enqueue, drain, timeout
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_action_queue_enqueue_drain() {
    use agent_world_engine::human::HumanAction;

    let queue = HumanActionQueue::new();
    queue
        .register_agent("agent-1", "user-1", "Alice", 0, 100_000, 5)
        .await;

    // Enqueue two actions
    queue
        .enqueue(HumanAction {
            agent_id: "agent-1".into(),
            action_type: HumanActionType::Rest,
            params: json!({}),
            submitted_tick: 1,
        })
        .await;
    queue
        .enqueue(HumanAction {
            agent_id: "agent-1".into(),
            action_type: HumanActionType::Explore,
            params: json!({"direction": "north"}),
            submitted_tick: 1,
        })
        .await;

    assert_eq!(queue.pending_count().await, 2);

    let drained = queue.drain(1).await;
    assert_eq!(drained.len(), 2);
    assert_eq!(drained[0].action_type, HumanActionType::Rest);
    assert_eq!(drained[1].action_type, HumanActionType::Explore);

    assert_eq!(queue.pending_count().await, 0);
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 2: HumanActionQueue — timeout detection
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_action_queue_timeout_detection() {
    let queue = HumanActionQueue::new();
    queue
        .register_agent("a1", "u1", "Alice", 0, 1000, 0)
        .await;
    queue
        .register_agent("a2", "u2", "Bob", 0, 1000, 0)
        .await;

    // a1 submits an action at tick 1
    queue
        .enqueue(agent_world_engine::human::HumanAction {
            agent_id: "a1".into(),
            action_type: HumanActionType::Rest,
            params: json!({}),
            submitted_tick: 1,
        })
        .await;
    queue.drain(1).await;

    // At tick 4, a1 has been idle for 3 ticks, a2 for 4 ticks
    let timed_out = queue.check_timeouts(4, 3).await;
    assert!(timed_out.contains(&"a1".to_string()));
    assert!(timed_out.contains(&"a2".to_string()));
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 3: HumanActionQueue — newbie protection disabled after first action
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_newbie_protection_disabled_after_first_action() {
    let queue = HumanActionQueue::new();
    queue
        .register_agent("a1", "u1", "Alice", 0, 1000, 10)
        .await;

    let state = queue.get_agent_state("a1").await.unwrap();
    assert!(state.newbie_protection);

    queue
        .enqueue(agent_world_engine::human::HumanAction {
            agent_id: "a1".into(),
            action_type: HumanActionType::Rest,
            params: json!({}),
            submitted_tick: 1,
        })
        .await;
    queue.drain(1).await;

    let state = queue.get_agent_state("a1").await.unwrap();
    assert!(!state.newbie_protection);
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 4: Action token costs are correct
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_action_token_costs() {
    assert_eq!(HumanActionType::Rest.token_cost(), 0);
    assert_eq!(HumanActionType::Rest.token_income(), 5);
    assert_eq!(HumanActionType::Communicate.token_cost(), 10);
    assert_eq!(HumanActionType::Explore.token_cost(), 3);
    assert_eq!(HumanActionType::Build.token_cost(), 20);
    assert_eq!(HumanActionType::Trade.token_cost(), 10);
    assert_eq!(HumanActionType::Move.token_cost(), 12);

    assert_eq!(HumanActionType::Explore.token_income(), 2);
    assert_eq!(HumanActionType::Gather.token_income(), 3);
    assert_eq!(HumanActionType::Build.token_income(), 5);
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 5: Action type from_str_lossy round-trip
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_action_type_from_str() {
    // Valid types
    assert_eq!(
        HumanActionType::from_str_lossy("rest"),
        Some(HumanActionType::Rest)
    );
    assert_eq!(
        HumanActionType::from_str_lossy("communicate"),
        Some(HumanActionType::Communicate)
    );
    assert_eq!(
        HumanActionType::from_str_lossy("trade"),
        Some(HumanActionType::Trade)
    );
    assert_eq!(
        HumanActionType::from_str_lossy("explore"),
        Some(HumanActionType::Explore)
    );
    assert_eq!(
        HumanActionType::from_str_lossy("move"),
        Some(HumanActionType::Move)
    );

    // Invalid type
    assert!(HumanActionType::from_str_lossy("fly").is_none());
    assert!(HumanActionType::from_str_lossy("").is_none());
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 6: Incarnate endpoint (without auth — expects 400 for missing auth)
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_incarnate_without_auth_returns_400_or_401() {
    let app = create_test_app();

    let req = make_request(
        Method::POST,
        "/api/v1/human/incarnate",
        Some(json!({"name": "Player1"})),
    );
    let resp = app.oneshot(req).await.unwrap();
    // Without auth middleware configured, RequireAuth will reject
    assert!(
        resp.status() == StatusCode::UNAUTHORIZED
            || resp.status() == StatusCode::BAD_REQUEST
            || resp.status() == StatusCode::FORBIDDEN,
        "expected auth failure, got {}",
        resp.status()
    );
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 7: Submit action with invalid action type returns 400
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_submit_action_invalid_type() {
    let app = create_test_app();

    // Even without auth, we can test the routing + validation:
    // With RequireAuth, this will fail at auth before reaching the handler.
    // But if auth is not configured in test, it will reach the handler and
    // fail at agent lookup.
    let req = make_request(
        Method::POST,
        "/api/v1/human/agents/fake-id/action",
        Some(json!({"action": "invalid_action"})),
    );
    let resp = app.oneshot(req).await.unwrap();
    // Either auth failure or agent not found — both are non-200
    assert_ne!(resp.status(), StatusCode::OK);
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 8: Direct action queue + agent lifecycle simulation
//
// Simulates: incarnate → submit action → drain → survival rules
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_full_lifecycle_via_action_queue() {
    use agent_world_engine::human::HumanAction;

    let queue = HumanActionQueue::new();

    // Step 1: Incarnate
    queue
        .register_agent("agent-1", "user-1", "Alice", 0, 100_000, 5)
        .await;
    let state = queue.get_agent_state("agent-1").await.unwrap();
    assert_eq!(state.initial_tokens, 100_000);
    assert!(state.newbie_protection);

    // Step 2: Submit a Rest action
    queue
        .enqueue(HumanAction {
            agent_id: "agent-1".into(),
            action_type: HumanActionType::Rest,
            params: json!({}),
            submitted_tick: 1,
        })
        .await;
    assert_eq!(queue.pending_count().await, 1);

    // Step 3: Drain at tick 1 (simulates tick execution)
    let actions = queue.drain(1).await;
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].action_type, HumanActionType::Rest);

    // Verify state updated
    let state = queue.get_agent_state("agent-1").await.unwrap();
    assert_eq!(state.last_action_tick, 1);
    assert!(!state.newbie_protection); // Disabled after first action

    // Step 4: Simulate timeout (no actions for 3 ticks)
    let timed_out = queue.check_timeouts(4, 3).await;
    assert!(timed_out.contains(&"agent-1".to_string()));

    // Step 5: Apply AI fallback (Rest) and touch the agent
    queue.touch_agent("agent-1", 4).await;

    // Now check timeout again — should not be timed out
    let timed_out = queue.check_timeouts(5, 3).await;
    assert!(!timed_out.contains(&"agent-1".to_string()));

    // Step 6: Remove agent (simulates death)
    queue.remove_agent("agent-1").await;
    assert!(!queue.is_human_agent("agent-1").await);
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 9: Concurrent action submission safety
//
// Multiple actions from the same agent should all be enqueued without
// data races.
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_concurrent_action_submission() {
    use agent_world_engine::human::HumanAction;

    let queue = Arc::new(HumanActionQueue::new());
    queue
        .register_agent("agent-1", "user-1", "Alice", 0, 100_000, 0)
        .await;

    // Spawn 10 concurrent tasks that each enqueue an action
    let mut handles = Vec::new();
    for i in 0..10 {
        let q = queue.clone();
        handles.push(tokio::spawn(async move {
            q.enqueue(HumanAction {
                agent_id: "agent-1".into(),
                action_type: HumanActionType::Explore,
                params: json!({"seq": i}),
                submitted_tick: 1,
            })
            .await;
        }));
    }

    for handle in handles {
        handle.await.unwrap();
    }

    assert_eq!(queue.pending_count().await, 10);

    let drained = queue.drain(1).await;
    assert_eq!(drained.len(), 10);

    // All should be for the same agent
    for action in &drained {
        assert_eq!(action.agent_id, "agent-1");
    }
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 10: Multiple human agents coexist
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_multiple_human_agents() {
    use agent_world_engine::human::HumanAction;

    let queue = HumanActionQueue::new();

    // Register 3 human agents
    for i in 0..3 {
        queue
            .register_agent(
                &format!("agent-{i}"),
                &format!("user-{i}"),
                &format!("Player{i}"),
                0,
                100_000,
                5,
            )
            .await;
    }

    let agents = queue.list_agents().await;
    assert_eq!(agents.len(), 3);

    // Each submits a different action
    queue
        .enqueue(HumanAction {
            agent_id: "agent-0".into(),
            action_type: HumanActionType::Rest,
            params: json!({}),
            submitted_tick: 1,
        })
        .await;
    queue
        .enqueue(HumanAction {
            agent_id: "agent-1".into(),
            action_type: HumanActionType::Trade,
            params: json!({"target": "agent-2"}),
            submitted_tick: 1,
        })
        .await;
    queue
        .enqueue(HumanAction {
            agent_id: "agent-2".into(),
            action_type: HumanActionType::Explore,
            params: json!({}),
            submitted_tick: 1,
        })
        .await;

    let drained = queue.drain(1).await;
    assert_eq!(drained.len(), 3);

    // Verify each agent's state was updated
    for i in 0..3 {
        let state = queue.get_agent_state(&format!("agent-{i}")).await.unwrap();
        assert_eq!(state.last_action_tick, 1);
        assert!(!state.newbie_protection); // Disabled after first action
    }
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 11: World events SSE endpoint exists at /world/:id/events
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_world_events_sse_with_id_route() {
    let app = create_test_app();

    // The route /api/v1/world/test-world-id/events should be accepted
    // (same handler as /world/events)
    let req = make_request(Method::GET, "/api/v1/world/test-world/events", None);
    let resp = app.oneshot(req).await.unwrap();

    // Should return 200 (SSE stream starts successfully)
    // In test mode without a running server, the response will be the SSE stream
    assert_eq!(resp.status(), StatusCode::OK);
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 12: HumanAgentConfig defaults
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_human_agent_config_defaults() {
    use agent_world_engine::config::HumanAgentConfig;

    let config = HumanAgentConfig::default();
    assert_eq!(config.initial_tokens, 100_000);
    assert_eq!(config.newbie_protection_ticks, 5);
    assert_eq!(config.timeout_ticks, 3);
    assert_eq!(config.token_burn_per_tick, 50);
    assert_eq!(config.death_token_threshold, 0);
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 13: GenesisConfig includes human_agent section
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_genesis_config_human_agent_section() {
    use agent_world_engine::config::GenesisConfig;

    let config = GenesisConfig::default();
    assert_eq!(config.human_agent.initial_tokens, 100_000);
    assert_eq!(config.human_agent.timeout_ticks, 3);

    // Test YAML deserialization with custom values
    let yaml = r#"
human_agent:
  initial_tokens: 50000
  newbie_protection_ticks: 10
  timeout_ticks: 5
  token_burn_per_tick: 25
  death_token_threshold: 100
"#;
    let config: GenesisConfig = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(config.human_agent.initial_tokens, 50000);
    assert_eq!(config.human_agent.newbie_protection_ticks, 10);
    assert_eq!(config.human_agent.timeout_ticks, 5);
    assert_eq!(config.human_agent.token_burn_per_tick, 25);
    assert_eq!(config.human_agent.death_token_threshold, 100);
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 14: Full incarnate → action → drain → token changes
//
// Simulates the entire tick cycle with token tracking.
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_incarnate_action_drain_token_flow() {
    use agent_world_engine::human::HumanAction;

    let queue = HumanActionQueue::new();

    // Incarnate with 1000 tokens
    queue
        .register_agent("agent-1", "user-1", "Alice", 0, 1000, 5)
        .await;

    let mut current_tokens: u64 = 1000;

    // Tick 1: Submit Explore action (cost: 3, income: 2)
    queue
        .enqueue(HumanAction {
            agent_id: "agent-1".into(),
            action_type: HumanActionType::Explore,
            params: json!({}),
            submitted_tick: 1,
        })
        .await;

    // Drain and apply
    let actions = queue.drain(1).await;
    for action in &actions {
        current_tokens = current_tokens
            .saturating_sub(action.action_type.token_cost())
            .saturating_add(action.action_type.token_income());
    }
    // Explore: 1000 - 3 + 2 = 999
    assert_eq!(current_tokens, 999);

    // Tick 2: Submit Rest action (cost: 0, income: 5)
    queue
        .enqueue(HumanAction {
            agent_id: "agent-1".into(),
            action_type: HumanActionType::Rest,
            params: json!({}),
            submitted_tick: 2,
        })
        .await;

    let actions = queue.drain(2).await;
    for action in &actions {
        current_tokens = current_tokens
            .saturating_sub(action.action_type.token_cost())
            .saturating_add(action.action_type.token_income());
    }
    // Rest: 999 - 0 + 5 = 1004
    assert_eq!(current_tokens, 1004);

    // Tick 3: Submit Build action (cost: 20, income: 5)
    queue
        .enqueue(HumanAction {
            agent_id: "agent-1".into(),
            action_type: HumanActionType::Build,
            params: json!({}),
            submitted_tick: 3,
        })
        .await;

    let actions = queue.drain(3).await;
    for action in &actions {
        current_tokens = current_tokens
            .saturating_sub(action.action_type.token_cost())
            .saturating_add(action.action_type.token_income());
    }
    // Build: 1004 - 20 + 5 = 989
    assert_eq!(current_tokens, 989);
}
