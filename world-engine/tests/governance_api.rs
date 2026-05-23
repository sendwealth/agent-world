//! Governance API Endpoint Integration Tests.
//!
//! Tests the 4 governance endpoints:
//! - GET /api/v1/governance/summary
//! - GET /api/v1/governance/orgs/:org_id
//! - GET /api/v1/governance/orgs/:org_id/timeline
//! - GET /api/v1/governance/comparison

use std::sync::Arc;

use tokio::sync::Mutex;

use agent_world_engine::api::AppState;
use agent_world_engine::economy::task::TaskBoard;
use agent_world_engine::organization::governance_metrics::GovernanceMetricsCollector;
use agent_world_engine::wal::WAL;
use agent_world_engine::world::event::WorldEvent;
use agent_world_engine::world::state::EventBus;

fn build_app() -> (Arc<EventBus>, Arc<Mutex<GovernanceMetricsCollector>>, axum::Router) {
    let dir = tempfile::TempDir::new().unwrap();
    let event_bus = Arc::new(EventBus::new(256));
    let board = Arc::new(Mutex::new(TaskBoard::with_event_bus((*event_bus).clone())));
    let wal = Arc::new(Mutex::new(WAL::new(dir.path())));
    let (tick_tx, tick_rx) = tokio::sync::watch::channel(0u64);

    // Create collector from the event bus — it spawns a background subscription task
    let collector = GovernanceMetricsCollector::new(&event_bus);
    let metrics = Arc::new(Mutex::new(collector));

    let state = AppState {
        board,
        wal,
        event_bus: event_bus.clone(),
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
        external_agents: Arc::new(Mutex::new(std::collections::HashMap::new())),
        governance_metrics: Some(metrics.clone()),
        building_manager: Arc::new(Mutex::new(agent_world_engine::world::map::building::BuildingManager::new())),
        federation_registry: None,
        migration_manager: None,
    };

    let app = agent_world_engine::api::build_full_router(state);
    (event_bus, metrics, app)
}

async fn start_server() -> (u16, Arc<EventBus>, Arc<Mutex<GovernanceMetricsCollector>>) {
    let (event_bus, metrics, app) = build_app();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // Give the server and background collector time to start
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    (port, event_bus, metrics)
}

fn sample_events() -> Vec<WorldEvent> {
    vec![
        WorldEvent::TaxCollected {
            org_id: "00000000-0000-0000-0000-000000000001".into(),
            payer_id: "a1".into(),
            tax_kind: "IncomeTax".into(),
            rate: 0.10,
            gross_amount: 1000,
            tax_amount: 100,
            tick: 10,
        },
        WorldEvent::TaxCollected {
            org_id: "00000000-0000-0000-0000-000000000001".into(),
            payer_id: "a2".into(),
            tax_kind: "IncomeTax".into(),
            rate: 0.10,
            gross_amount: 500,
            tax_amount: 50,
            tick: 15,
        },
        WorldEvent::TreasuryDistributed {
            org_id: "00000000-0000-0000-0000-000000000001".into(),
            strategy: "equal".into(),
            total_amount: 100,
            allocations: vec![("a1".into(), 50), ("a2".into(), 50)],
            tick: 20,
        },
        WorldEvent::TreatySigned {
            treaty_id: "t-1".into(),
            org_a: "00000000-0000-0000-0000-000000000001".into(),
            org_b: "00000000-0000-0000-0000-000000000002".into(),
        },
    ]
}

#[tokio::test]
async fn governance_summary_returns_world_summary() {
    let (port, event_bus, _metrics) = start_server().await;

    // Emit events — the background collector will pick them up
    for event in sample_events() {
        event_bus.emit(event);
    }
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://127.0.0.1:{}/api/v1/governance/summary", port))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), reqwest::StatusCode::OK);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["total_orgs"], 2);
    assert_eq!(body["total_tax_collected"], 150);
    assert_eq!(body["total_treaties"], 2); // both orgs counted
}

#[tokio::test]
async fn governance_org_metrics_returns_org_detail() {
    let (port, event_bus, _metrics) = start_server().await;

    for event in sample_events() {
        event_bus.emit(event);
    }
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let client = reqwest::Client::new();
    let resp = client
        .get(format!(
            "http://127.0.0.1:{}/api/v1/governance/orgs/00000000-0000-0000-0000-000000000001",
            port
        ))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), reqwest::StatusCode::OK);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["total_tax_collected"], 150);
    assert_eq!(body["tax_collection_count"], 2);
}

#[tokio::test]
async fn governance_org_metrics_404_for_unknown() {
    let (port, _event_bus, _metrics) = start_server().await;

    let client = reqwest::Client::new();
    let resp = client
        .get(format!(
            "http://127.0.0.1:{}/api/v1/governance/orgs/ffffffff-ffff-ffff-ffff-ffffffffffff",
            port
        ))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), reqwest::StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn governance_timeline_returns_filtered_events() {
    let (port, event_bus, _metrics) = start_server().await;

    for event in sample_events() {
        event_bus.emit(event);
    }
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let client = reqwest::Client::new();
    let resp = client
        .get(format!(
            "http://127.0.0.1:{}/api/v1/governance/orgs/00000000-0000-0000-0000-000000000001/timeline?from_tick=0&to_tick=100",
            port
        ))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), reqwest::StatusCode::OK);

    let body: serde_json::Value = resp.json().await.unwrap();
    let events = body.as_array().unwrap();
    // Should have tax events + treasury distributed + treaty signed
    assert!(!events.is_empty());
}

#[tokio::test]
async fn governance_timeline_tick_range_filter() {
    let (port, event_bus, _metrics) = start_server().await;

    for event in sample_events() {
        event_bus.emit(event);
    }
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let client = reqwest::Client::new();
    let resp = client
        .get(format!(
            "http://127.0.0.1:{}/api/v1/governance/orgs/00000000-0000-0000-0000-000000000001/timeline?from_tick=12&to_tick=18",
            port
        ))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), reqwest::StatusCode::OK);

    let body: serde_json::Value = resp.json().await.unwrap();
    let events = body.as_array().unwrap();
    // Only the tax event at tick 15 should match [12, 18]
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["tick"], 15);
}

#[tokio::test]
async fn governance_comparison_returns_multiple_orgs() {
    let (port, event_bus, _metrics) = start_server().await;

    for event in sample_events() {
        event_bus.emit(event);
    }
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let client = reqwest::Client::new();
    let resp = client
        .get(format!(
            "http://127.0.0.1:{}/api/v1/governance/comparison?org_ids=00000000-0000-0000-0000-000000000001,00000000-0000-0000-0000-000000000002",
            port
        ))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), reqwest::StatusCode::OK);

    let body: serde_json::Value = resp.json().await.unwrap();
    let orgs = body.as_array().unwrap();
    assert_eq!(orgs.len(), 2);
}

#[tokio::test]
async fn governance_comparison_400_without_org_ids() {
    let (port, _event_bus, _metrics) = start_server().await;

    let client = reqwest::Client::new();
    let resp = client
        .get(format!(
            "http://127.0.0.1:{}/api/v1/governance/comparison",
            port
        ))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), reqwest::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn governance_endpoints_503_when_not_configured() {
    let dir = tempfile::TempDir::new().unwrap();
    let event_bus = Arc::new(EventBus::new(256));
    let board = Arc::new(Mutex::new(TaskBoard::with_event_bus((*event_bus).clone())));
    let wal = Arc::new(Mutex::new(WAL::new(dir.path())));
    let (tick_tx, tick_rx) = tokio::sync::watch::channel(0u64);

    let state = AppState {
        board,
        wal,
        event_bus: event_bus.clone(),
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
        external_agents: Arc::new(Mutex::new(std::collections::HashMap::new())),
        governance_metrics: None,
        building_manager: Arc::new(Mutex::new(agent_world_engine::world::map::building::BuildingManager::new())),
        federation_registry: None,
        migration_manager: None,
    };
    let app = agent_world_engine::api::build_full_router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://127.0.0.1:{}/api/v1/governance/summary", port))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), reqwest::StatusCode::SERVICE_UNAVAILABLE);
}
