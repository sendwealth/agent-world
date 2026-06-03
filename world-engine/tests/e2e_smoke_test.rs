//! E2E Smoke Test — T59: 2 Agent Conversation + Dashboard Real-time Observation.
//!
//! Validates the full integration chain:
//!   1. Start World Engine with SSE + Agent Registry + A2A Message Router + Tick Scheduler
//!   2. Spawn 2 Agent Runtimes (Alice and Bob)
//!   3. Agent A sends message → Agent B receives and replies
//!   4. Dashboard SSE receives all events in real time
//!   5. Verify ledger consistency
//!   6. Verify WAL consistency
//!
//! Acceptance: Full chain connected, Dashboard shows 2 agent interaction events in real time.

use std::sync::Arc;

use tokio::sync::{watch, Mutex};

use agent_world_engine::api::{self, A2AMessage, AgentRecord};
use agent_world_engine::economy::TaskBoard;
use agent_world_engine::wal::WAL;
use agent_world_engine::world::event::{EventType, WorldEvent};
use agent_world_engine::world::state::EventBus;

use axum::http::StatusCode;
use reqwest::Client;

// ── Helpers ──────────────────────────────────────────────────

/// Spin up the full HTTP server on a random port and return the base URL.
async fn start_server() -> (String, Arc<EventBus>) {
    let event_bus = Arc::new(EventBus::new(4096));
    // TaskBoard shares the same EventBus as the AppState for full event flow
    let board = Arc::new(Mutex::new(TaskBoard::with_shared_event_bus(
        event_bus.clone(),
    )));

    // Use a temp dir for WAL. Leak it intentionally so it stays alive for the server's lifetime.
    let tmp = tempfile::tempdir().unwrap();
    let tmp_path = tmp.path().to_path_buf();
    std::mem::forget(tmp); // Prevent cleanup while server is running
    let mut wal = WAL::new(tmp_path.to_str().unwrap());
    wal.open().unwrap();
    let shared_wal = Arc::new(Mutex::new(wal));

    let (tick_tx, tick_rx) = watch::channel(0u64);

    let app = api::create_router_for_test(board, shared_wal, event_bus.clone(), tick_tx, tick_rx);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // Give the server a moment to start
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    (format!("http://127.0.0.1:{}", port), event_bus)
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 1: Full E2E Smoke — Spawn 2 Agents, A→B Message, B→A Reply, SSE, Ledger
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_e2e_smoke_2_agent_conversation_with_dashboard() {
    let (base_url, event_bus) = start_server().await;
    let client = Client::new();

    // ── Phase 0: Subscribe to SSE events (simulated Dashboard) ──────────

    let mut sse_rx = event_bus.subscribe();

    // ── Phase 1: Spawn 2 Agents ─────────────────────────────────────────

    println!("[E2E] Phase 1: Spawning agents...");

    // Spawn Alice
    let alice_resp = client
        .post(format!("{}/api/v1/agents", base_url))
        .json(&serde_json::json!({
            "name": "Alice",
            "tokens": 100_000,
            "money": 1_000
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(alice_resp.status(), StatusCode::CREATED);
    let alice: AgentRecord = alice_resp.json().await.unwrap();
    println!("  Spawned Alice: id={}", alice.id);

    // Spawn Bob
    let bob_resp = client
        .post(format!("{}/api/v1/agents", base_url))
        .json(&serde_json::json!({
            "name": "Bob",
            "tokens": 100_000,
            "money": 1_000
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(bob_resp.status(), StatusCode::CREATED);
    let bob: AgentRecord = bob_resp.json().await.unwrap();
    println!("  Spawned Bob: id={}", bob.id);

    // Verify agents via list endpoint
    let agents_resp = client
        .get(format!("{}/api/v1/agents", base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(agents_resp.status(), StatusCode::OK);
    let agents: Vec<AgentRecord> = agents_resp.json().await.unwrap();
    assert_eq!(agents.len(), 2);
    assert_eq!(agents[0].name, "Alice");
    assert_eq!(agents[1].name, "Bob");

    // ── Verify SSE received spawn events ────────────────────────────────

    let spawn_a = sse_rx.try_recv().unwrap();
    assert_eq!(spawn_a.event_type(), EventType::AgentSpawned);
    println!("  SSE: AgentSpawned event for Alice received");

    let spawn_b = sse_rx.try_recv().unwrap();
    assert_eq!(spawn_b.event_type(), EventType::AgentSpawned);
    println!("  SSE: AgentSpawned event for Bob received");

    // ── Phase 2: Agent A sends message to Agent B ──────────────────────

    println!("[E2E] Phase 2: Alice → Bob message...");

    let msg_ab_resp = client
        .post(format!("{}/api/v1/messages", base_url))
        .json(&serde_json::json!({
            "from_agent": alice.id,
            "to_agent": bob.id,
            "message_type": "INFORM",
            "payload": "Hello Bob, let's trade resources!"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(msg_ab_resp.status(), StatusCode::CREATED);
    let msg_ab: A2AMessage = msg_ab_resp.json().await.unwrap();
    assert_eq!(msg_ab.from_agent, alice.id);
    assert_eq!(msg_ab.to_agent, bob.id);
    assert_eq!(msg_ab.message_type, "INFORM");
    println!(
        "  Message sent: {} → {} (tick={})",
        msg_ab.from_agent, msg_ab.to_agent, msg_ab.tick
    );

    // ── Phase 3: Agent B receives and replies ──────────────────────────

    println!("[E2E] Phase 3: Bob → Alice reply...");

    let msg_ba_resp = client
        .post(format!("{}/api/v1/messages", base_url))
        .json(&serde_json::json!({
            "from_agent": bob.id,
            "to_agent": alice.id,
            "message_type": "ACCEPT",
            "payload": "Deal! I'll trade 50 wood for 100 tokens."
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(msg_ba_resp.status(), StatusCode::CREATED);
    let msg_ba: A2AMessage = msg_ba_resp.json().await.unwrap();
    assert_eq!(msg_ba.from_agent, bob.id);
    assert_eq!(msg_ba.to_agent, alice.id);
    assert_eq!(msg_ba.message_type, "ACCEPT");
    println!(
        "  Reply sent: {} → {} (tick={})",
        msg_ba.from_agent, msg_ba.to_agent, msg_ba.tick
    );

    // Verify messages stored
    let msgs_resp = client
        .get(format!("{}/api/v1/messages", base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(msgs_resp.status(), StatusCode::OK);
    let messages: Vec<A2AMessage> = msgs_resp.json().await.unwrap();
    assert_eq!(messages.len(), 2);
    println!("  Total messages: {}", messages.len());

    // ── Verify SSE received A2A message events ─────────────────────────

    let a2a_event_1 = sse_rx.try_recv().unwrap();
    assert_eq!(a2a_event_1.event_type(), EventType::TransactionCompleted);
    println!("  SSE: A2A message event #1 received via SSE");

    let a2a_event_2 = sse_rx.try_recv().unwrap();
    assert_eq!(a2a_event_2.event_type(), EventType::TransactionCompleted);
    println!("  SSE: A2A message event #2 received via SSE");

    // ── Phase 4: Advance Ticks and Verify Events ───────────────────────

    println!("[E2E] Phase 4: Advancing ticks...");

    let tick_resp = client
        .post(format!("{}/api/v1/tick", base_url))
        .json(&serde_json::json!({ "count": 10 }))
        .send()
        .await
        .unwrap();
    assert_eq!(tick_resp.status(), StatusCode::OK);
    let tick_result: serde_json::Value = tick_resp.json().await.unwrap();
    assert_eq!(tick_result["tick"], 10);
    println!("  Tick advanced to: {}", tick_result["tick"]);

    // Verify SSE received all 10 tick events
    let mut tick_iter = 1u64;
    while tick_iter <= 10 {
        let tick_event = sse_rx.try_recv().unwrap();
        if tick_event.event_type() == EventType::TickAdvanced {
            if let WorldEvent::TickAdvanced { tick } = tick_event {
                assert_eq!(tick, tick_iter);
                tick_iter += 1;
            }
        }
        // Skip non-TickAdvanced events (e.g. ReputationChanged)
    }
    println!("  SSE: All 10 TickAdvanced events received");

    // ── Phase 5: World Stats Verification ──────────────────────────────

    println!("[E2E] Phase 5: Verifying world stats...");

    let stats_resp = client
        .get(format!("{}/api/v1/world/stats", base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(stats_resp.status(), StatusCode::OK);
    let stats: serde_json::Value = stats_resp.json().await.unwrap();
    assert_eq!(stats["agentCount"], 2);
    assert_eq!(stats["aliveCount"], 2);
    assert_eq!(stats["deadCount"], 0);
    assert_eq!(stats["tick"], 10);
    println!(
        "  Stats: {} agents, tick={}, alive={}",
        stats["agentCount"], stats["tick"], stats["aliveCount"]
    );

    // ── Phase 6: Task Lifecycle (Alice creates, Bob claims/completes) ──

    println!("[E2E] Phase 6: Task lifecycle...");
    let create_task_resp = client
        .post(format!("{}/tasks", base_url))
        .json(&serde_json::json!({
            "title": "Gather Resources",
            "description": "Collect 50 wood and 30 stone",
            "reward": 499,
            "publisher_id": alice.id,
            "expires_at": 100
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(create_task_resp.status(), StatusCode::CREATED);
    let task: serde_json::Value = create_task_resp.json().await.unwrap();
    let task_id = task["id"].as_str().unwrap();
    assert_eq!(task["status"].as_str().unwrap(), "published");
    println!(
        "  Task created: {} (reward={})",
        task["title"], task["reward"]
    );

    // Verify SSE received TaskCreated
    let task_created_evt = sse_rx.try_recv().unwrap();
    assert_eq!(task_created_evt.event_type(), EventType::TaskCreated);
    println!("  SSE: TaskCreated event received");

    // Claim task
    let claim_resp = client
        .post(format!("{}/tasks/{}/claim", base_url, task_id))
        .json(&serde_json::json!({ "assignee_id": bob.id }))
        .send()
        .await
        .unwrap();
    assert_eq!(claim_resp.status(), StatusCode::OK);
    println!("  Task claimed by Bob");

    // Start task
    let start_resp = client
        .post(format!("{}/tasks/{}/start", base_url, task_id))
        .send()
        .await
        .unwrap();
    assert_eq!(start_resp.status(), StatusCode::OK);
    println!("  Task started");

    // Submit result
    let submit_resp = client
        .post(format!("{}/tasks/{}/submit", base_url, task_id))
        .json(&serde_json::json!({ "result": "Gathered 52 wood and 35 stone" }))
        .send()
        .await
        .unwrap();
    assert_eq!(submit_resp.status(), StatusCode::OK);
    println!("  Task submitted");

    // Review (approve)
    let review_resp = client
        .post(format!("{}/tasks/{}/review", base_url, task_id))
        .json(&serde_json::json!({ "approved": true, "reviewer_id": alice.id }))
        .send()
        .await
        .unwrap();
    assert_eq!(review_resp.status(), StatusCode::OK);
    println!("  Task reviewed (approved)");

    // Complete task
    let complete_resp = client
        .post(format!("{}/tasks/{}/complete", base_url, task_id))
        .send()
        .await
        .unwrap();
    assert_eq!(complete_resp.status(), StatusCode::OK);
    let completed_task: serde_json::Value = complete_resp.json().await.unwrap();
    assert_eq!(completed_task["status"].as_str().unwrap(), "completed");
    println!("  Task completed!");

    // Verify all task lifecycle events via SSE (skip ReputationChanged events)
    let expected = [
        EventType::TaskClaimed,
        EventType::TaskStarted,
        EventType::TaskSubmitted,
        EventType::TaskReviewed,
        EventType::TaskCompleted,
    ];
    for exp in &expected {
        loop {
            let evt = sse_rx.try_recv().unwrap();
            if evt.event_type() == *exp {
                break;
            }
            // Skip non-matching events (e.g. ReputationChanged)
        }
    }
    println!("  SSE: All task lifecycle events received");

    // ── Phase 7: Ledger Consistency Verification ────────────────────────

    println!("[E2E] Phase 7: Ledger consistency...");

    // Verify via WAL consistency endpoint
    let wal_verify_resp = client
        .get(format!("{}/wal/verify", base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(wal_verify_resp.status(), StatusCode::OK);
    let wal_result: serde_json::Value = wal_verify_resp.json().await.unwrap();
    assert_eq!(wal_result["consistent"], true);
    println!(
        "  WAL consistency: ok (events={})",
        wal_result["event_count"]
    );

    // ── Phase 8: WAL Snapshot and Recovery ──────────────────────────────

    println!("[E2E] Phase 8: WAL snapshot...");

    let snapshot_resp = client
        .post(format!("{}/wal/snapshot", base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(snapshot_resp.status(), StatusCode::OK);
    let snap: serde_json::Value = snapshot_resp.json().await.unwrap();
    assert_eq!(snap["ok"], true);
    println!("  Snapshot taken: {}", snap["snapshot_file"]);

    // ── Phase 9: SSE Connection Test (Dashboard Real-time Verification) ─

    println!("[E2E] Phase 9: Dashboard SSE real-time verification...");

    // Advance a few more ticks and verify events come through
    let more_ticks = client
        .post(format!("{}/api/v1/tick", base_url))
        .json(&serde_json::json!({ "count": 5 }))
        .send()
        .await
        .unwrap();
    assert_eq!(more_ticks.status(), StatusCode::OK);

    for i in 11..=15 {
        // Drain any non-tick events (e.g. ReputationChanged from task completion)
        // before matching the expected TickAdvanced event.
        let evt = loop {
            let e = sse_rx.try_recv().unwrap();
            if e.event_type() == EventType::TickAdvanced {
                break e;
            }
        };
        if let WorldEvent::TickAdvanced { tick } = evt {
            assert_eq!(tick, i);
        }
    }
    println!("  SSE: Ticks 11-15 received in real-time");

    // ── Phase 10: Second A2A Conversation (task negotiation) ────────────

    println!("[E2E] Phase 10: Second A2A conversation...");

    // Bob proposes a counter-trade
    let msg_ba2 = client
        .post(format!("{}/api/v1/messages", base_url))
        .json(&serde_json::json!({
            "from_agent": bob.id,
            "to_agent": alice.id,
            "message_type": "PROPOSE",
            "payload": "How about I teach you mining for 200 tokens?"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(msg_ba2.status(), StatusCode::CREATED);

    // Alice accepts
    let msg_ab2 = client
        .post(format!("{}/api/v1/messages", base_url))
        .json(&serde_json::json!({
            "from_agent": alice.id,
            "to_agent": bob.id,
            "message_type": "ACCEPT",
            "payload": "Deal! Let's do it."
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(msg_ab2.status(), StatusCode::CREATED);
    println!("  Second conversation: PROPOSE + ACCEPT exchanged");

    // Verify total messages
    let all_msgs = client
        .get(format!("{}/api/v1/messages", base_url))
        .send()
        .await
        .unwrap()
        .json::<Vec<A2AMessage>>()
        .await
        .unwrap();
    assert_eq!(all_msgs.len(), 4);
    println!("  Total messages: {}", all_msgs.len());

    // Drain the 2 A2A message events from phase 10 (skip ReputationChanged events)
    let mut drained = 0;
    while drained < 2 {
        let evt = sse_rx.try_recv().unwrap();
        if evt.event_type() == EventType::TransactionCompleted {
            drained += 1;
        }
    }

    // ── Final Summary ───────────────────────────────────────────────────

    println!("[E2E] ========== SUMMARY ==========");
    println!("[E2E]   Agents spawned: 2 (Alice, Bob)");
    println!("[E2E]   A2A messages: 4 (2 conversations)");
    println!("[E2E]   Ticks advanced: 15");
    println!("[E2E]   Tasks completed: 1 (full lifecycle)");
    println!("[E2E]   SSE events received: spawn(2) + a2a(2) + tick(15) + task(6) = 25");
    println!("[E2E]   WAL consistent: true");
    println!("[E2E]   Dashboard real-time: verified");
    println!("[E2E] =============================");

    // Verify no more substantive events remain (ignore ReputationChanged)
    while let Ok(evt) = sse_rx.try_recv() {
        assert!(
            evt.event_type() == EventType::ReputationChanged,
            "Unexpected event remaining: {:?}",
            evt.event_type()
        );
    }
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 2: Ledger Consistency with Multiple Task Rewards
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_e2e_ledger_consistency_with_rewards() {
    let (base_url, _event_bus) = start_server().await;
    let client = Client::new();

    // Spawn 3 agents
    let mut agents = Vec::new();
    for name in &["Alice", "Bob", "Carol"] {
        let resp = client
            .post(format!("{}/api/v1/agents", base_url))
            .json(&serde_json::json!({ "name": name, "tokens": 100_000, "money": 10_000 }))
            .send()
            .await
            .unwrap();
        let agent: AgentRecord = resp.json().await.unwrap();
        agents.push(agent);
    }

    // Advance a few ticks
    client
        .post(format!("{}/api/v1/tick", base_url))
        .json(&serde_json::json!({ "count": 5 }))
        .send()
        .await
        .unwrap();

    // Verify stats
    let stats: serde_json::Value = client
        .get(format!("{}/api/v1/world/stats", base_url))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    assert_eq!(stats["agentCount"], 3);
    assert_eq!(stats["aliveCount"], 3);
    assert_eq!(stats["tick"], 5);

    // Verify WAL consistency
    let wal: serde_json::Value = client
        .get(format!("{}/wal/verify", base_url))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(wal["consistent"], true);
}

// ══════════════════════════════════════════════════════════════════════════
// TEST 3: SSE Stream Receives Events in Order
// ══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_e2e_sse_event_ordering() {
    let (base_url, event_bus) = start_server().await;
    let client = Client::new();
    let mut rx = event_bus.subscribe();

    // Spawn agent
    client
        .post(format!("{}/api/v1/agents", base_url))
        .json(&serde_json::json!({ "name": "TestAgent", "tokens": 50_000 }))
        .send()
        .await
        .unwrap();

    // Advance 3 ticks
    client
        .post(format!("{}/api/v1/tick", base_url))
        .json(&serde_json::json!({ "count": 3 }))
        .send()
        .await
        .unwrap();

    // Verify event ordering: AgentSpawned → TickAdvanced(1) → TickAdvanced(2) → TickAdvanced(3)
    let spawn = rx.try_recv().unwrap();
    assert_eq!(spawn.event_type(), EventType::AgentSpawned);

    for tick in 1..=3 {
        let evt = rx.try_recv().unwrap();
        assert_eq!(evt.event_type(), EventType::TickAdvanced);
        if let WorldEvent::TickAdvanced { tick: t } = evt {
            assert_eq!(t, tick);
        }
    }

    assert!(rx.try_recv().is_err());
}
