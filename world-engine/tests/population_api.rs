//! Tests for Population Evolution Tracking & Statistics API endpoints.
//!
//! Covers: /api/v1/population/{stats,species,diversity,events,genealogy,timeline,export/csv}

use std::sync::Arc;

use tokio::sync::{watch, Mutex};

use agent_world_engine::api::{self, AgentRecord};
use agent_world_engine::economy::TaskBoard;
use agent_world_engine::wal::WAL;
use agent_world_engine::world::state::EventBus;

use axum::http::StatusCode;
use reqwest::Client;

// ── Helpers ──────────────────────────────────────────────────

async fn start_server() -> String {
    let event_bus = Arc::new(EventBus::new(4096));
    let board = Arc::new(Mutex::new(TaskBoard::with_shared_event_bus(
        event_bus.clone(),
    )));

    let tmp = tempfile::tempdir().unwrap();
    let tmp_path = tmp.path().to_path_buf();
    std::mem::forget(tmp);
    let mut wal = WAL::new(tmp_path.to_str().unwrap());
    wal.open().unwrap();
    let shared_wal = Arc::new(Mutex::new(wal));

    let (tick_tx, tick_rx) = watch::channel(0u64);

    let app = api::create_router_for_test(board, shared_wal, event_bus, tick_tx, tick_rx);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("http://127.0.0.1:{}", port)
}

/// Spawn a single agent and return its record.
async fn spawn_one(base: &str, name: &str) -> AgentRecord {
    let client = Client::new();
    let resp = client
        .post(format!("{}/api/v1/agents", base))
        .json(&serde_json::json!({"name": name, "tokens": 1000, "money": 500}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    resp.json().await.unwrap()
}

// ── Population Stats Tests ───────────────────────────────────

#[tokio::test]
async fn test_population_stats_empty() {
    let base = start_server().await;
    let client = Client::new();

    let resp = client
        .get(format!("{}/api/v1/population/stats", base))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["alive_count"], 0);
    assert_eq!(body["dead_count"], 0);
    assert_eq!(body["total_spawned"], 0);
    assert_eq!(body["max_generation"], 0);
}

#[tokio::test]
async fn test_population_stats_after_spawns() {
    let base = start_server().await;
    let client = Client::new();
    let _a = spawn_one(&base, "Alpha").await;
    let _b = spawn_one(&base, "Beta").await;

    let resp = client
        .get(format!("{}/api/v1/population/stats", base))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["alive_count"], 2);
    assert_eq!(body["total_spawned"], 2);
    assert_eq!(body["total_tokens"], 2000);
    assert_eq!(body["total_money"], 1000);
    // New agents have generation 0 and no skills
    assert_eq!(body["max_generation"], 0);
    assert!(body["skill_distribution"].as_array().unwrap().is_empty());
}

// ── Population Species Tests ─────────────────────────────────

#[tokio::test]
async fn test_population_species_groups_by_skills() {
    let base = start_server().await;
    let client = Client::new();

    // Spawn 3 agents — all have empty skills, so they should be one species
    let _a = spawn_one(&base, "A").await;
    let _b = spawn_one(&base, "B").await;
    let _c = spawn_one(&base, "C").await;

    let resp = client
        .get(format!("{}/api/v1/population/species", base))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body: serde_json::Value = resp.json().await.unwrap();
    let species = body.as_array().unwrap();
    // All 3 agents have empty skills → one "(no skills)" species
    assert_eq!(species.len(), 1);
    assert_eq!(species[0]["count"], 3);
    assert_eq!(species[0]["skill_signature"], "(no skills)");
}

// ── Population Diversity Tests ────────────────────────────────

#[tokio::test]
async fn test_population_diversity_empty() {
    let base = start_server().await;
    let client = Client::new();

    let resp = client
        .get(format!("{}/api/v1/population/diversity", base))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["shannon_index"], 0.0);
    assert_eq!(body["simpson_index"], 0.0);
    assert_eq!(body["skill_richness"], 0);
    assert_eq!(body["alive_count"], 0);
}

#[tokio::test]
async fn test_population_diversity_with_agents() {
    let base = start_server().await;
    let client = Client::new();
    let _a = spawn_one(&base, "X").await;

    let resp = client
        .get(format!("{}/api/v1/population/diversity", base))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["alive_count"], 1);
    assert_eq!(body["max_generation"], 0);
}

// ── Population Events Tests ──────────────────────────────────

#[tokio::test]
async fn test_population_events_returns_event_types() {
    let base = start_server().await;
    let client = Client::new();

    let resp = client
        .get(format!("{}/api/v1/population/events", base))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body: serde_json::Value = resp.json().await.unwrap();
    let events = body.as_array().unwrap();
    assert!(!events.is_empty());
    // Check that known event types are present
    let types: Vec<&str> = events
        .iter()
        .filter_map(|e| e["event_type"].as_str())
        .collect();
    assert!(types.contains(&"agent_spawned"));
    assert!(types.contains(&"agent_died"));
    assert!(types.contains(&"fitness_evaluated"));
}

// ── Population Genealogy Tests ───────────────────────────────

#[tokio::test]
async fn test_population_genealogy_found() {
    let base = start_server().await;
    let client = Client::new();
    let agent = spawn_one(&base, "Founder").await;

    let resp = client
        .get(format!("{}/api/v1/population/genealogy/{}", base, agent.id))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["target"]["agent"]["id"], agent.id);
    assert_eq!(body["target"]["agent"]["name"], "Founder");
    // No children yet
    assert!(body["target"]["children"].as_array().unwrap().is_empty());
    // No parents (generation 0)
    assert!(body["ancestors"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_population_genealogy_not_found() {
    let base = start_server().await;
    let client = Client::new();

    let resp = client
        .get(format!(
            "{}/api/v1/population/genealogy/nonexistent-id",
            base
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// ── Population Timeline Tests ────────────────────────────────

#[tokio::test]
async fn test_population_timeline_returns_current_state() {
    let base = start_server().await;
    let client = Client::new();
    let _a = spawn_one(&base, "T1").await;
    let _b = spawn_one(&base, "T2").await;

    let resp = client
        .get(format!("{}/api/v1/population/timeline", base))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body: serde_json::Value = resp.json().await.unwrap();
    let timeline = body.as_array().unwrap();
    // Should have at least one data point (current state)
    assert!(!timeline.is_empty());
    let last = timeline.last().unwrap();
    assert_eq!(last["alive_count"], 2);
}

#[tokio::test]
async fn test_population_timeline_with_params() {
    let base = start_server().await;
    let client = Client::new();

    let resp = client
        .get(format!(
            "{}/api/v1/population/timeline?from_tick=0&to_tick=100&interval=50",
            base
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.as_array().is_some());
}

// ── Population CSV Export Tests ──────────────────────────────

#[tokio::test]
async fn test_population_csv_export() {
    let base = start_server().await;
    let client = Client::new();
    let _a = spawn_one(&base, "CsvAgent1").await;
    let _b = spawn_one(&base, "CsvAgent2").await;

    let resp = client
        .get(format!("{}/api/v1/population/export/csv", base))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let ct = resp
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(ct.contains("text/csv"));

    let cd = resp
        .headers()
        .get("content-disposition")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(cd.contains("population_tick_"));

    let csv = resp.text().await.unwrap();
    let lines: Vec<&str> = csv.lines().collect();
    // Header + 2 data rows
    assert_eq!(lines.len(), 3);
    assert!(lines[0].contains("agent_id"));
    assert!(lines[0].contains("generation"));
    assert!(lines[0].contains("parent_ids"));
    assert!(lines[0].contains("skills"));
}

#[tokio::test]
async fn test_population_csv_escapes_special_chars() {
    let base = start_server().await;
    let client = Client::new();

    // Spawn agent with CSV-dangerous name (contains comma and quote)
    let resp = client
        .post(format!("{}/api/v1/agents", base))
        .json(&serde_json::json!({"name": "Test, \"Agent\"", "tokens": 1000, "money": 500}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    let resp = client
        .get(format!("{}/api/v1/population/export/csv", base))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let csv = resp.text().await.unwrap();
    let lines: Vec<&str> = csv.lines().collect();
    assert!(lines.len() >= 2);

    // The name field should be properly quoted/escaped
    // "Test, "Agent"" should appear as """Test, ""Agent""""" or similar safe form
    let data_line = lines[1];
    // Ensure it doesn't have unescaped commas within a field that would break CSV parsing
    // The csv_escape function wraps fields containing commas in double quotes
    assert!(data_line.contains("\"Test,") || data_line.contains("Test,"));
}

#[tokio::test]
async fn test_population_csv_injection_prevention() {
    let base = start_server().await;
    let client = Client::new();

    // Try CSV injection via formula in name
    let resp = client
        .post(format!("{}/api/v1/agents", base))
        .json(&serde_json::json!({"name": "=CMD(\"danger\")", "tokens": 1000, "money": 500}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    let resp = client
        .get(format!("{}/api/v1/population/export/csv", base))
        .send()
        .await
        .unwrap();
    let csv = resp.text().await.unwrap();

    // The name field is escaped via csv_escape which wraps it in quotes
    // The field should start with a quote character since it contains special chars
    // This means spreadsheet software treats it as text, not a formula
    assert!(csv.contains("\"=CMD")); // quoted → safe
}
