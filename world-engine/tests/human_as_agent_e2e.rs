//! Human-as-Agent (Phase 5.5) E2E Integration Tests.
//!
//! Tests the full flow:
//! - POST /api/v1/human/incarnate — register as agent
//! - POST /api/v1/human/agents/:id/action — submit actions
//! - Action queue drain + execution
//! - Timeout fallback to AI decision
//! - Survival rules: token burn, death, newbie protection
//! - SSE events endpoint
//!
//! After the dual-implementation unification (SEN-725), all tests use
//! the single `human_agent::HumanActionQueue` + `HumanAgentRegistry`.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tokio::sync::{watch, Mutex};
use tower::ServiceExt;

use agent_world_engine::api::create_router_for_test;
use agent_world_engine::economy::task::TaskBoard;
use agent_world_engine::human_agent::{
    HumanActionQueue, HumanActionType, HumanAgent, HumanAgentRegistry, QueuedAction,
};
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

/// Helper: create a QueuedAction for a given agent + action verb.
fn queued(agent_id: &str, action: &str, tick: u64) -> QueuedAction {
    QueuedAction {
        id: String::new(), // auto-assigned by enqueue
        agent_id: agent_id.to_string(),
        action: action.to_string(),
        params: json!({}),
        enqueued_tick: tick,
        applied: false,
    }
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 1: HumanActionQueue — enqueue, drain
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_action_queue_enqueue_drain() {
    let queue = HumanActionQueue::shared();
    let agent_id = "agent-1";

    queue.lock().await.enqueue(queued(agent_id, "rest", 1)).unwrap();
    queue
        .lock()
        .await
        .enqueue(queued(agent_id, "explore", 2))
        .unwrap();

    assert_eq!(queue.lock().await.pending_count(), 2);

    let drained = queue.lock().await.drain_for_agent(agent_id);
    assert_eq!(drained.len(), 2);
    assert_eq!(drained[0].action, "rest");
    assert_eq!(drained[1].action, "explore");

    assert_eq!(queue.lock().await.pending_count(), 0);
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 2: HumanAgentRegistry — incarnation + timeout bookkeeping
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_registry_timeout_bookkeeping() {
    let registry = HumanAgentRegistry::shared();
    let agent_id = "a1";

    // Register two agents at tick 0
    registry
        .lock()
        .await
        .register(HumanAgent {
            agent_id: agent_id.to_string(),
            human_id: "u1".into(),
            name: "Alice".into(),
            initial_tokens: 1000,
            initial_money: 0,
            spawned_tick: 0,
            last_action_tick: 0,
            alive: true,
            metadata: json!({}),
        })
        .unwrap();
    registry
        .lock()
        .await
        .register(HumanAgent {
            agent_id: "a2".into(),
            human_id: "u2".into(),
            name: "Bob".into(),
            initial_tokens: 1000,
            initial_money: 0,
            spawned_tick: 0,
            last_action_tick: 0,
            alive: true,
            metadata: json!({}),
        })
        .unwrap();

    // a1 submits an action at tick 1 → touch_action
    registry.lock().await.touch_action("a1", 1);

    // At tick 4: a1 idle for 3, a2 idle for 4
    let idle: Vec<String> = registry
        .lock()
        .await
        .iter_alive()
        .filter(|a| 4u64.saturating_sub(a.last_action_tick) >= 3)
        .map(|a| a.agent_id.clone())
        .collect();
    assert!(idle.contains(&"a1".to_string()));
    assert!(idle.contains(&"a2".to_string()));

    // At tick 3: a1 idle for 2, a2 idle for 3
    let idle: Vec<String> = registry
        .lock()
        .await
        .iter_alive()
        .filter(|a| 3u64.saturating_sub(a.last_action_tick) >= 3)
        .map(|a| a.agent_id.clone())
        .collect();
    assert!(!idle.contains(&"a1".to_string()));
    assert!(idle.contains(&"a2".to_string()));
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 3: Action token costs are correct
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
// TEST 4: Action type from_str_lossy round-trip
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_action_type_from_str() {
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
    assert_eq!(
        HumanActionType::from_str_lossy("gather"),
        Some(HumanActionType::Gather)
    );

    assert!(HumanActionType::from_str_lossy("fly").is_none());
    assert!(HumanActionType::from_str_lossy("").is_none());
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 5: Incarnate endpoint (without auth — expects 400 for missing auth)
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
    assert!(
        resp.status() == StatusCode::UNAUTHORIZED
            || resp.status() == StatusCode::BAD_REQUEST
            || resp.status() == StatusCode::FORBIDDEN,
        "expected auth failure, got {}",
        resp.status()
    );
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 6: Submit action with invalid action type returns 400
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_submit_action_invalid_type() {
    let app = create_test_app();

    let req = make_request(
        Method::POST,
        "/api/v1/human/agents/fake-id/action",
        Some(json!({"action": "invalid_action"})),
    );
    let resp = app.oneshot(req).await.unwrap();
    assert_ne!(resp.status(), StatusCode::OK);
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 7: Full lifecycle via unified queue + registry
//
// Simulates: incarnate → submit action → drain → survival rules
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_full_lifecycle_via_unified_queue() {
    let queue = HumanActionQueue::shared();
    let registry = HumanAgentRegistry::shared();
    let agent_id = "agent-1";

    // Step 1: Incarnate (register in registry)
    registry
        .lock()
        .await
        .register(HumanAgent {
            agent_id: agent_id.to_string(),
            human_id: "user-1".into(),
            name: "Alice".into(),
            initial_tokens: 100_000,
            initial_money: 0,
            spawned_tick: 0,
            last_action_tick: 0,
            alive: true,
            metadata: json!({}),
        })
        .unwrap();

    let agent = registry.lock().await.get_by_agent(agent_id).cloned().unwrap();
    assert_eq!(agent.initial_tokens, 100_000);

    // Step 2: Submit a Rest action
    queue.lock().await.enqueue(queued(agent_id, "rest", 1)).unwrap();
    assert_eq!(queue.lock().await.pending_count(), 1);

    // Step 3: Drain at tick 1 (simulates tick execution)
    let actions = queue.lock().await.drain_for_agent(agent_id);
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].action, "rest");

    // Touch action in registry
    registry.lock().await.touch_action(agent_id, 1);

    let agent = registry.lock().await.get_by_agent(agent_id).cloned().unwrap();
    assert_eq!(agent.last_action_tick, 1);

    // Step 4: Simulate timeout (no actions for several ticks)
    let current_tick = 10u64;
    let idle = current_tick.saturating_sub(agent.last_action_tick);
    assert!(idle >= 5, "agent should be idle after 9 ticks");

    // Step 5: Mark dead (simulates death)
    registry.lock().await.mark_dead(agent_id);
    let agent = registry.lock().await.get_by_agent(agent_id).cloned().unwrap();
    assert!(!agent.alive);
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 8: Concurrent action submission safety
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_concurrent_action_submission() {
    let queue = Arc::new(HumanActionQueue::shared());
    let agent_id = "agent-1".to_string();

    // Spawn 10 concurrent tasks that each enqueue an action
    let mut handles = Vec::new();
    for i in 0..10 {
        let q = queue.clone();
        let aid = agent_id.clone();
        handles.push(tokio::spawn(async move {
            q.lock()
                .await
                .enqueue(QueuedAction {
                    id: String::new(),
                    agent_id: aid,
                    action: "explore".to_string(),
                    params: json!({"seq": i}),
                    enqueued_tick: 1,
                    applied: false,
                })
                .unwrap();
        }));
    }

    for handle in handles {
        handle.await.unwrap();
    }

    assert_eq!(queue.lock().await.pending_count(), 10);

    let drained = queue.lock().await.drain_all();
    let total: usize = drained.values().map(|v| v.len()).sum();
    assert_eq!(total, 10);

    for action in drained.values().flatten() {
        assert_eq!(action.agent_id, "agent-1");
    }
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 9: Multiple human agents coexist
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_multiple_human_agents() {
    let queue = HumanActionQueue::shared();
    let registry = HumanAgentRegistry::shared();

    // Register 3 human agents
    for i in 0..3u32 {
        registry
            .lock()
            .await
            .register(HumanAgent {
                agent_id: format!("agent-{i}"),
                human_id: format!("user-{i}"),
                name: format!("Player{i}"),
                initial_tokens: 100_000,
                initial_money: 0,
                spawned_tick: 0,
                last_action_tick: 0,
                alive: true,
                metadata: json!({}),
            })
            .unwrap();
    }

    let alive_count = registry.lock().await.iter_alive().count();
    assert_eq!(alive_count, 3);

    // Each submits a different action
    queue.lock().await.enqueue(queued("agent-0", "rest", 1)).unwrap();
    queue.lock().await.enqueue(queued("agent-1", "trade", 1)).unwrap();
    queue.lock().await.enqueue(queued("agent-2", "explore", 1)).unwrap();

    let drained = queue.lock().await.drain_all();
    assert_eq!(drained.len(), 3); // 3 agents

    // Touch each agent
    for i in 0..3 {
        registry.lock().await.touch_action(&format!("agent-{i}"), 1);
    }

    // Verify each agent's state was updated
    for i in 0..3 {
        let agent = registry
            .lock()
            .await
            .get_by_agent(&format!("agent-{i}"))
            .cloned()
            .unwrap();
        assert_eq!(agent.last_action_tick, 1);
    }
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 10: World events SSE endpoint exists at /world/:id/events
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_world_events_sse_with_id_route() {
    let app = create_test_app();
    let req = make_request(Method::GET, "/api/v1/world/test-world/events", None);
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 11: HumanAgentConfig defaults
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
// TEST 12: GenesisConfig includes human_agent section
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_genesis_config_human_agent_section() {
    use agent_world_engine::config::GenesisConfig;

    let config = GenesisConfig::default();
    assert_eq!(config.human_agent.initial_tokens, 100_000);
    assert_eq!(config.human_agent.timeout_ticks, 3);

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
// TEST 13: Full incarnate → action → drain → token changes
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_incarnate_action_drain_token_flow() {
    use agent_world_engine::human_agent::{action_token_cost, action_token_income};

    let queue = HumanActionQueue::shared();

    // Simulate incarnate by enqueuing actions and computing token changes
    // using the free functions that the subsystem itself uses.
    let mut current_tokens: u64 = 1000;

    // Tick 1: Explore (cost: 3, income: 2)
    queue.lock().await.enqueue(queued("agent-1", "explore", 1)).unwrap();
    let actions = queue.lock().await.drain_for_agent("agent-1");
    for action in &actions {
        current_tokens = current_tokens
            .saturating_sub(action_token_cost(&action.action))
            .saturating_add(action_token_income(&action.action));
    }
    // Explore: 1000 - 3 + 2 = 999
    assert_eq!(current_tokens, 999);

    // Tick 2: Rest (cost: 0, income: 5)
    queue.lock().await.enqueue(queued("agent-1", "rest", 2)).unwrap();
    let actions = queue.lock().await.drain_for_agent("agent-1");
    for action in &actions {
        current_tokens = current_tokens
            .saturating_sub(action_token_cost(&action.action))
            .saturating_add(action_token_income(&action.action));
    }
    // Rest: 999 - 0 + 5 = 1004
    assert_eq!(current_tokens, 1004);

    // Tick 3: Build (cost: 20, income: 5)
    queue.lock().await.enqueue(queued("agent-1", "build", 3)).unwrap();
    let actions = queue.lock().await.drain_for_agent("agent-1");
    for action in &actions {
        current_tokens = current_tokens
            .saturating_sub(action_token_cost(&action.action))
            .saturating_add(action_token_income(&action.action));
    }
    // Build: 1004 - 20 + 5 = 989
    assert_eq!(current_tokens, 989);
}
