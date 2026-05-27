//! Tests for Phase 4.5.1 Data Export API endpoints.
//!
//! Validates snapshot export (JSON/CSV), economy export, and custom query API.

use std::sync::Arc;

use tokio::sync::{Mutex, watch};

use agent_world_engine::api::{self, AgentRecord};
use agent_world_engine::economy::TaskBoard;
use agent_world_engine::wal::WAL;
use agent_world_engine::world::state::EventBus;

use axum::http::StatusCode;
use reqwest::Client;

// ── Helpers ──────────────────────────────────────────────────

async fn start_server() -> String {
    let event_bus = Arc::new(EventBus::new(4096));
    let board = Arc::new(Mutex::new(TaskBoard::with_shared_event_bus(event_bus.clone())));

    let tmp = tempfile::tempdir().unwrap();
    let tmp_path = tmp.path().to_path_buf();
    std::mem::forget(tmp);
    let mut wal = WAL::new(tmp_path.to_str().unwrap());
    wal.open().unwrap();
    let shared_wal = Arc::new(Mutex::new(wal));

    let (tick_tx, tick_rx) = watch::channel(0u64);

    let app = api::create_router_for_test(
        board,
        shared_wal,
        event_bus,
        tick_tx,
        tick_rx,
    );

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("http://127.0.0.1:{}", port)
}

async fn seed_agents(base: &str) -> Vec<AgentRecord> {
    let client = Client::new();
    let mut agents = Vec::new();

    let resp = client.post(format!("{}/api/v1/agents", base))
        .json(&serde_json::json!({"name": "Alice", "tokens": 1000, "money": 500}))
        .send().await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let alice: AgentRecord = resp.json().await.unwrap();
    agents.push(alice);

    let resp = client.post(format!("{}/api/v1/agents", base))
        .json(&serde_json::json!({"name": "Bob", "tokens": 800, "money": 300}))
        .send().await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let bob: AgentRecord = resp.json().await.unwrap();
    agents.push(bob);

    agents
}

// ── Snapshot Export Tests ─────────────────────────────────────

#[tokio::test]
async fn test_export_snapshot_json() {
    let base = start_server().await;
    let client = Client::new();
    let _ = seed_agents(&base).await;

    let resp = client.get(format!("{}/api/v1/export/snapshot?format=json", base))
        .send().await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["tick"], 0);
    assert_eq!(body["agents"].as_array().unwrap().len(), 2);
    assert_eq!(body["task_count"], 0);
    assert!(!body["exported_at"].as_str().unwrap().is_empty());


    let agent_names: Vec<&str> = body["agents"].as_array().unwrap().iter()
        .map(|a| a["name"].as_str().unwrap())
        .collect();
    assert!(agent_names.contains(&"Alice"));
    assert!(agent_names.contains(&"Bob"));
}

#[tokio::test]
async fn test_export_snapshot_csv() {
    let base = start_server().await;
    let client = Client::new();
    let _ = seed_agents(&base).await;

    let resp = client.get(format!("{}/api/v1/export/snapshot?format=csv", base))
        .send().await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let content_type = resp.headers().get("content-type").unwrap().to_str().unwrap();
    assert!(content_type.contains("text/csv"));

    let body = resp.text().await.unwrap();
    let lines: Vec<&str> = body.trim().lines().collect();
    assert!(lines[0].contains("id,name,phase"));
    assert_eq!(lines.len(), 3);
}

#[tokio::test]
async fn test_export_snapshot_default_format_is_json() {
    let base = start_server().await;
    let client = Client::new();

    let resp = client.get(format!("{}/api/v1/export/snapshot", base))
        .send().await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let content_type = resp.headers().get("content-type").unwrap().to_str().unwrap();
    assert!(content_type.contains("application/json"));
}

#[tokio::test]
async fn test_export_snapshot_by_tick() {
    let base = start_server().await;
    let client = Client::new();
    let _ = seed_agents(&base).await;

    let resp = client.get(format!("{}/api/v1/export/snapshot/5?format=json", base))
        .send().await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["tick"], 5);
}

#[tokio::test]
async fn test_export_snapshot_by_tick_csv() {
    let base = start_server().await;
    let client = Client::new();
    let _ = seed_agents(&base).await;

    let resp = client.get(format!("{}/api/v1/export/snapshot/10?format=csv", base))
        .send().await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let content_type = resp.headers().get("content-type").unwrap().to_str().unwrap();
    assert!(content_type.contains("text/csv"));
}

// ── Economy Export Tests ──────────────────────────────────────

#[tokio::test]
async fn test_export_economy_json() {
    let base = start_server().await;
    let client = Client::new();
    let _ = seed_agents(&base).await;

    let resp = client.get(format!("{}/api/v1/export/economy?format=json", base))
        .send().await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.get("tick").is_some() || body.is_array() || body.get("total_money").is_some());
}

#[tokio::test]
async fn test_export_economy_csv() {
    let base = start_server().await;
    let client = Client::new();
    let _ = seed_agents(&base).await;

    let resp = client.get(format!("{}/api/v1/export/economy?format=csv", base))
        .send().await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let content_type = resp.headers().get("content-type").unwrap().to_str().unwrap();
    assert!(content_type.contains("text/csv"));
}

#[tokio::test]
async fn test_export_economy_with_tick_range() {
    let base = start_server().await;
    let client = Client::new();
    let _ = seed_agents(&base).await;

    let resp = client.get(format!("{}/api/v1/export/economy?from_tick=0&to_tick=100&format=json", base))
        .send().await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

// ── Custom Query Tests ────────────────────────────────────────

#[tokio::test]
async fn test_export_query_basic() {
    let base = start_server().await;
    let client = Client::new();
    let agents = seed_agents(&base).await;

    let resp = client.post(format!("{}/api/v1/export/query", base))
        .json(&serde_json::json!({
            "filters": {
                "agent_ids": [agents[0].id],
                "tick_range": [0, 100],
                "format": "json"
            }
        }))
        .send().await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.is_object());
}

#[tokio::test]
async fn test_export_query_missing_tick_range() {
    let base = start_server().await;
    let client = Client::new();

    let resp = client.post(format!("{}/api/v1/export/query", base))
        .json(&serde_json::json!({
            "filters": {
                "format": "json"
            }
        }))
        .send().await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_export_query_with_event_types() {
    let base = start_server().await;
    let client = Client::new();
    let agents = seed_agents(&base).await;

    let resp = client.post(format!("{}/api/v1/export/query", base))
        .json(&serde_json::json!({
            "filters": {
                "agent_ids": [agents[0].id, agents[1].id],
                "event_types": ["act", "decide"],
                "tick_range": [0, 50],
                "format": "json"
            }
        }))
        .send().await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_export_query_csv_format() {
    let base = start_server().await;
    let client = Client::new();
    let agents = seed_agents(&base).await;

    let resp = client.post(format!("{}/api/v1/export/query", base))
        .json(&serde_json::json!({
            "filters": {
                "agent_ids": [agents[0].id],
                "tick_range": [0, 10],
                "format": "csv"
            }
        }))
        .send().await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_export_query_all_agents_no_filter() {
    let base = start_server().await;
    let client = Client::new();
    let _ = seed_agents(&base).await;

    let resp = client.post(format!("{}/api/v1/export/query", base))
        .json(&serde_json::json!({
            "filters": {
                "tick_range": [0, 10],
                "format": "json"
            }
        }))
        .send().await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}
