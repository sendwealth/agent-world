//! Provider REST API Integration Tests
//!
//! Covers CRUD for providers, connection test / model discovery proxy,
//! and agent model assignment endpoints.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tokio::sync::Mutex;

use agent_world_engine::api::{AppState, TestOverrides, build_full_router};
use agent_world_engine::economy::task::TaskBoard;
use agent_world_engine::wal::WAL;

// ── Helpers ──────────────────────────────────────────────────

fn create_test_state() -> AppState {
    let board = Arc::new(Mutex::new(TaskBoard::new()));
    let dir = tempfile::TempDir::new().unwrap();
    let wal = Arc::new(Mutex::new(WAL::new(dir.path())));
    std::mem::forget(dir);
    AppState::new(board, wal, TestOverrides::default())
}

async fn body_json(body: Body) -> Value {
    let bytes = body.collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
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

/// Send a request using a fresh router built from the shared state.
async fn send(state: &AppState, req: Request<Body>) -> (StatusCode, Value) {
    use tower::ServiceExt;
    let router = build_full_router(state.clone());
    let resp = router.oneshot(req).await.unwrap();
    let status = resp.status();
    let body = body_json(resp.into_body()).await;
    (status, body)
}

/// Create a test provider.
async fn create_provider(state: &AppState, id: &str) -> Value {
    let req = make_request(
        Method::POST,
        "/api/v1/providers",
        Some(json!({
            "id": id,
            "protocol": "openai",
            "base_url": "https://api.openai.com/v1",
            "api_key": "sk-1234567890abcdef",
            "display_name": "OpenAI",
            "is_default": true
        })),
    );
    let (status, body) = send(state, req).await;
    assert_eq!(status, StatusCode::CREATED, "create should return 201, got {}: {:?}", status, body);
    body
}

// ── Tests ────────────────────────────────────────────────────

#[tokio::test]
async fn test_create_provider_returns_201() {
    let state = create_test_state();
    let result = create_provider(&state, "openai-1").await;

    assert_eq!(result["id"], "openai-1");
    assert_eq!(result["protocol"], "openai");
    assert_eq!(result["base_url"], "https://api.openai.com/v1");
    assert_eq!(result["display_name"], "OpenAI");
    assert_eq!(result["is_default"], true);
    assert_ne!(result["api_key"], "sk-1234567890abcdef");
    assert!(result["api_key"].as_str().unwrap().contains("****"));
}

#[tokio::test]
async fn test_create_provider_auto_generates_id() {
    let state = create_test_state();
    let req = make_request(
        Method::POST,
        "/api/v1/providers",
        Some(json!({
            "protocol": "anthropic",
            "base_url": "https://api.anthropic.com"
        })),
    );
    let (status, result) = send(&state, req).await;
    assert_eq!(status, StatusCode::CREATED);
    assert!(!result["id"].as_str().unwrap().is_empty());
    assert_eq!(result["protocol"], "anthropic");
}

#[tokio::test]
async fn test_create_provider_duplicate_id_returns_409() {
    let state = create_test_state();
    create_provider(&state, "dup-id").await;

    let req = make_request(
        Method::POST,
        "/api/v1/providers",
        Some(json!({
            "id": "dup-id",
            "protocol": "openai",
            "base_url": "https://api.openai.com/v1"
        })),
    );
    let (status, _) = send(&state, req).await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn test_create_provider_invalid_protocol_returns_400() {
    let state = create_test_state();
    let req = make_request(
        Method::POST,
        "/api/v1/providers",
        Some(json!({
            "protocol": "invalid_proto",
            "base_url": "https://example.com"
        })),
    );
    let (status, _) = send(&state, req).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_create_provider_empty_base_url_returns_400() {
    let state = create_test_state();
    let req = make_request(
        Method::POST,
        "/api/v1/providers",
        Some(json!({
            "protocol": "openai",
            "base_url": ""
        })),
    );
    let (status, _) = send(&state, req).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_list_providers_returns_all() {
    let state = create_test_state();
    create_provider(&state, "p1").await;
    create_provider(&state, "p2").await;

    let req = make_request(Method::GET, "/api/v1/providers", None);
    let (status, list) = send(&state, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list.as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn test_get_provider_returns_masked_key() {
    let state = create_test_state();
    create_provider(&state, "masked").await;

    let req = make_request(Method::GET, "/api/v1/providers/masked", None);
    let (status, result) = send(&state, req).await;
    assert_eq!(status, StatusCode::OK);
    let key = result["api_key"].as_str().unwrap();
    assert!(key.contains("****"));
    assert!(!key.contains("sk-1234567890abcdef"));
}

#[tokio::test]
async fn test_get_provider_not_found_returns_404() {
    let state = create_test_state();
    let req = make_request(Method::GET, "/api/v1/providers/nonexistent", None);
    let (status, _) = send(&state, req).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_update_provider_modifies_fields() {
    let state = create_test_state();
    create_provider(&state, "upd").await;

    let req = make_request(
        Method::PUT,
        "/api/v1/providers/upd",
        Some(json!({
            "base_url": "https://new-url.com",
            "is_default": false
        })),
    );
    let (status, result) = send(&state, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(result["base_url"], "https://new-url.com");
    assert_eq!(result["is_default"], false);
}

#[tokio::test]
async fn test_update_provider_invalid_protocol_returns_400() {
    let state = create_test_state();
    create_provider(&state, "upd-bad").await;

    let req = make_request(
        Method::PUT,
        "/api/v1/providers/upd-bad",
        Some(json!({ "protocol": "bad_proto" })),
    );
    let (status, _) = send(&state, req).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_update_provider_not_found_returns_404() {
    let state = create_test_state();
    let req = make_request(
        Method::PUT,
        "/api/v1/providers/nope",
        Some(json!({ "base_url": "https://x.com" })),
    );
    let (status, _) = send(&state, req).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_delete_provider_removes_it() {
    let state = create_test_state();
    create_provider(&state, "del-me").await;

    let req = make_request(Method::DELETE, "/api/v1/providers/del-me", None);
    let (status, _) = send(&state, req).await;
    assert_eq!(status, StatusCode::OK);

    let req = make_request(Method::GET, "/api/v1/providers/del-me", None);
    let (status, _) = send(&state, req).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_delete_provider_not_found_returns_404() {
    let state = create_test_state();
    let req = make_request(Method::DELETE, "/api/v1/providers/nope", None);
    let (status, _) = send(&state, req).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_connection_test_proxy() {
    let state = create_test_state();
    create_provider(&state, "test-conn").await;

    let req = make_request(Method::POST, "/api/v1/providers/test-conn/test", None);
    let (status, result) = send(&state, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(result["status"], "test_queued");
}

#[tokio::test]
async fn test_model_discovery_proxy() {
    let state = create_test_state();
    create_provider(&state, "disc").await;

    let req = make_request(Method::GET, "/api/v1/providers/disc/models", None);
    let (status, result) = send(&state, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(result["status"], "discovery_queued");
}

#[tokio::test]
async fn test_set_agent_model_assignment() {
    let state = create_test_state();
    create_provider(&state, "prov-a").await;

    let req = make_request(
        Method::PUT,
        "/api/v1/agents/agent-1/model",
        Some(json!({
            "provider_id": "prov-a",
            "model_id": "gpt-4"
        })),
    );
    let (status, result) = send(&state, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(result["agent_id"], "agent-1");
    assert_eq!(result["provider_id"], "prov-a");
    assert_eq!(result["model_id"], "gpt-4");
}

#[tokio::test]
async fn test_get_agent_model_returns_assignment() {
    let state = create_test_state();
    create_provider(&state, "prov-b").await;

    let req = make_request(
        Method::PUT,
        "/api/v1/agents/agent-2/model",
        Some(json!({
            "provider_id": "prov-b",
            "model_id": "claude-3"
        })),
    );
    let (status, _) = send(&state, req).await;
    assert_eq!(status, StatusCode::OK);

    let req = make_request(Method::GET, "/api/v1/agents/agent-2/model", None);
    let (status, result) = send(&state, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(result["provider_id"], "prov-b");
    assert_eq!(result["model_id"], "claude-3");
}

#[tokio::test]
async fn test_get_agent_model_unassigned_returns_null() {
    let state = create_test_state();
    let req = make_request(Method::GET, "/api/v1/agents/unknown/model", None);
    let (status, result) = send(&state, req).await;
    assert_eq!(status, StatusCode::OK);
    assert!(result.is_null());
}

#[tokio::test]
async fn test_set_agent_model_invalid_provider_returns_404() {
    let state = create_test_state();
    let req = make_request(
        Method::PUT,
        "/api/v1/agents/agent-x/model",
        Some(json!({
            "provider_id": "nonexistent",
            "model_id": "test"
        })),
    );
    let (status, _) = send(&state, req).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_full_crud_lifecycle() {
    let state = create_test_state();

    // CREATE
    let req = make_request(
        Method::POST,
        "/api/v1/providers",
        Some(json!({
            "id": "lifecycle",
            "protocol": "ollama",
            "base_url": "http://localhost:11434",
            "api_key": "short",
            "display_name": "Local Ollama"
        })),
    );
    let (status, _) = send(&state, req).await;
    assert_eq!(status, StatusCode::CREATED);

    // READ
    let req = make_request(Method::GET, "/api/v1/providers/lifecycle", None);
    let (status, p) = send(&state, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(p["protocol"], "ollama");
    assert_eq!(p["api_key"], "****"); // short key fully masked

    // UPDATE
    let req = make_request(
        Method::PUT,
        "/api/v1/providers/lifecycle",
        Some(json!({
            "base_url": "http://localhost:11435",
            "display_name": "Ollama Updated"
        })),
    );
    let (status, p) = send(&state, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(p["base_url"], "http://localhost:11435");
    assert_eq!(p["display_name"], "Ollama Updated");

    // DELETE
    let req = make_request(Method::DELETE, "/api/v1/providers/lifecycle", None);
    let (status, _) = send(&state, req).await;
    assert_eq!(status, StatusCode::OK);

    // Verify deleted
    let req = make_request(Method::GET, "/api/v1/providers/lifecycle", None);
    let (status, _) = send(&state, req).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
