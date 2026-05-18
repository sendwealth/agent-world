//! SSE Endpoint Integration Tests.
//!
//! Tests the GET /world/events SSE endpoint:
//! - Basic SSE connection and event delivery
//! - Event filtering by type
//! - Event filtering by agent_id
//! - Multi-client subscription
//! - Invalid event type returns 400
//! - Valid filter with no matching events

use std::sync::Arc;

use tokio::sync::Mutex;

use agent_world_engine::economy::task::TaskBoard;
use agent_world_engine::wal::WAL;
use agent_world_engine::world::event::WorldEvent;
use agent_world_engine::world::state::EventBus;

fn build_app() -> (
    Arc<EventBus>,
    Arc<Mutex<TaskBoard>>,
    axum::Router,
) {
    let dir = tempfile::TempDir::new().unwrap();
    let event_bus = Arc::new(EventBus::new(256));
    let board = Arc::new(Mutex::new(TaskBoard::with_event_bus((*event_bus).clone())));
    let wal = Arc::new(Mutex::new(WAL::new(dir.path())));
    let (tick_tx, tick_rx) = tokio::sync::watch::channel(0u64);
    let app = agent_world_engine::api::create_router_for_test(
        board.clone(),
        wal,
        event_bus.clone(),
        tick_tx,
        tick_rx,
    );
    (event_bus, board, app)
}

/// Helper: start the server on a random port, return (port, EventBus).
async fn start_server() -> (u16, Arc<EventBus>) {
    let (event_bus, _, app) = build_app();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // Give the server a moment to start
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    (port, event_bus)
}

/// Connect to SSE and collect data events for a bounded duration.
/// Returns parsed JSON strings from `data: ` lines.
async fn collect_sse_events(port: u16, query: &str, event_bus: Arc<EventBus>, events_to_emit: Vec<WorldEvent>) -> Vec<String> {
    let url = if query.is_empty() {
        format!("http://127.0.0.1:{}/api/v1/world/events", port)
    } else {
        format!("http://127.0.0.1:{}/api/v1/world/events?{}", port, query)
    };

    let client = reqwest::Client::new();

    // Spawn a task that collects SSE lines using chunk-based streaming
    let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(100);

    let collect_handle = tokio::spawn(async move {
        let resp = client.get(&url).send().await.unwrap();
        let mut stream = resp.bytes_stream();
        use futures::StreamExt;

        while let Some(chunk) = stream.next().await {
            if let Ok(bytes) = chunk {
                let text = String::from_utf8_lossy(&bytes);
                for line in text.lines() {
                    if line.starts_with("data: ") {
                        let _ = tx.send(line[6..].to_string()).await;
                    }
                }
            }
        }
    });

    // Wait for the connection to be established
    tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;

    // Emit the events
    for event in &events_to_emit {
        event_bus.emit(event.clone());
    }

    // Wait for events to propagate
    tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;

    // Abort the SSE connection
    collect_handle.abort();

    // Collect all received events
    let mut result = Vec::new();
    while let Ok(evt) = rx.try_recv() {
        result.push(evt);
    }
    result
}

#[tokio::test]
async fn sse_connection_receives_events() {
    let (port, event_bus) = start_server().await;

    let events = collect_sse_events(
        port,
        "",
        event_bus,
        vec![
            WorldEvent::TickAdvanced { tick: 42 },
            WorldEvent::AgentSpawned {
                agent_id: "a1".into(),
                name: "Alice".into(),
            },
        ],
    )
    .await;

    assert!(events.len() >= 2, "Expected at least 2 events, got {}", events.len());

    let tick: serde_json::Value = serde_json::from_str(&events[0]).unwrap();
    assert_eq!(tick["type"], "tick_advanced");
    assert_eq!(tick["payload"]["tick"], 42);
}

#[tokio::test]
async fn sse_filter_by_type() {
    let (port, event_bus) = start_server().await;

    let events = collect_sse_events(
        port,
        "types=agent_died,agent_rescued",
        event_bus,
        vec![
            WorldEvent::TickAdvanced { tick: 1 },
            WorldEvent::TickAdvanced { tick: 2 },
            WorldEvent::AgentDied {
                agent_id: "a1".into(),
                reason: agent_world_engine::world::enums::DeathReason::TokenDepleted,
            },
            WorldEvent::TickAdvanced { tick: 3 },
            WorldEvent::AgentRescued {
                agent_id: "a2".into(),
            },
        ],
    )
    .await;

    assert_eq!(events.len(), 2, "Expected exactly 2 filtered events, got {}", events.len());

    let evt1: serde_json::Value = serde_json::from_str(&events[0]).unwrap();
    assert_eq!(evt1["type"], "agent_died");

    let evt2: serde_json::Value = serde_json::from_str(&events[1]).unwrap();
    assert_eq!(evt2["type"], "agent_rescued");
}

#[tokio::test]
async fn sse_filter_by_agent_id() {
    let (port, event_bus) = start_server().await;

    use agent_world_engine::world::enums::AgentPhase;

    let events = collect_sse_events(
        port,
        "agent_id=agent-001",
        event_bus,
        vec![
            WorldEvent::AgentSpawned {
                agent_id: "agent-001".into(),
                name: "Alice".into(),
            },
            WorldEvent::AgentSpawned {
                agent_id: "agent-002".into(),
                name: "Bob".into(),
            },
            WorldEvent::PhaseChanged {
                agent_id: "agent-001".into(),
                old_phase: AgentPhase::Childhood,
                new_phase: AgentPhase::Adult,
            },
            WorldEvent::TickAdvanced { tick: 1 },
        ],
    )
    .await;

    assert_eq!(events.len(), 2, "Expected exactly 2 agent-filtered events, got {}", events.len());

    let evt1: serde_json::Value = serde_json::from_str(&events[0]).unwrap();
    assert_eq!(evt1["type"], "agent_spawned");

    let evt2: serde_json::Value = serde_json::from_str(&events[1]).unwrap();
    assert_eq!(evt2["type"], "phase_changed");
}

#[tokio::test]
async fn sse_multi_client() {
    let (port, event_bus) = start_server().await;

    // Start 3 SSE clients
    let mut rx_channels = Vec::new();
    for _ in 0..3 {
        let (tx, rx) = tokio::sync::mpsc::channel::<String>(100);
        rx_channels.push(rx);

        let client = reqwest::Client::new();
        let url = format!("http://127.0.0.1:{}/api/v1/world/events", port);

        tokio::spawn(async move {
            let resp = client.get(&url).send().await.unwrap();
            let mut stream = resp.bytes_stream();
            use futures::StreamExt;

            while let Some(chunk) = stream.next().await {
                if let Ok(bytes) = chunk {
                    let text = String::from_utf8_lossy(&bytes);
                    for line in text.lines() {
                        if line.starts_with("data: ") {
                            let _ = tx.send(line[6..].to_string()).await;
                        }
                    }
                }
            }
        });
    }

    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // Emit a single event
    event_bus.emit(WorldEvent::TickAdvanced { tick: 99 });

    tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;

    // All 3 clients should receive the event
    for (i, rx) in rx_channels.iter_mut().enumerate() {
        let mut events = Vec::new();
        while let Ok(evt) = rx.try_recv() {
            events.push(evt);
        }
        assert!(
            events.len() >= 1,
            "Client {} should receive at least 1 event, got {}",
            i,
            events.len()
        );
    }
}

#[tokio::test]
async fn sse_invalid_event_type_returns_400() {
    let (port, _) = start_server().await;
    let client = reqwest::Client::new();

    let resp = client
        .get(&format!(
            "http://127.0.0.1:{}/api/v1/world/events?types=tick_advanced,invalid_type",
            port
        ))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), reqwest::StatusCode::BAD_REQUEST);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body["error"].as_str().unwrap().contains("unknown event type"));
}

#[tokio::test]
async fn sse_valid_filter_with_no_matching_events() {
    let (port, event_bus) = start_server().await;

    let events = collect_sse_events(
        port,
        "types=task_completed",
        event_bus,
        vec![
            WorldEvent::TickAdvanced { tick: 1 },
            WorldEvent::TickAdvanced { tick: 2 },
        ],
    )
    .await;

    assert!(
        events.is_empty(),
        "Expected 0 events for non-matching filter, got {}",
        events.len()
    );
}
