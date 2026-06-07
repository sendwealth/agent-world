//! Integration tests for the Task API (api_tasks module) - SEN-660 P0.
//!
//! Covers:
//! - Create task (happy path + validation errors)
//! - List tasks (empty + after creation)
//! - Get task by ID (happy path + not found + invalid UUID)
//! - Task lifecycle: claim -> start -> submit -> review -> complete
//! - Task expiry and deletion
//! - Invalid state transitions (error paths)

use std::sync::Arc;
use tokio::sync::Mutex;

use agent_world_engine::api::{build_full_router, AppState, TestOverrides};
use agent_world_engine::economy::task::TaskBoard;
use agent_world_engine::wal::WAL;

use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use serde_json::json;
use tower::ServiceExt;

fn test_app() -> Router {
    let board = Arc::new(Mutex::new(TaskBoard::new()));
    let wal = Arc::new(Mutex::new(WAL::new("./data/test-wal")));
    let state = AppState::new(board, wal, TestOverrides::default());
    build_full_router(state)
}

async fn send(app: &Router, req: Request<Body>) -> (StatusCode, serde_json::Value) {
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let body_bytes = axum::body::to_bytes(resp.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let val: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap_or(json!(null));
    (status, val)
}

fn json_post(uri: &str, body: serde_json::Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap()
}

fn get_req(uri: &str) -> Request<Body> {
    Request::builder().uri(uri).body(Body::empty()).unwrap()
}

fn delete_req(uri: &str) -> Request<Body> {
    Request::builder()
        .method("DELETE")
        .uri(uri)
        .body(Body::empty())
        .unwrap()
}

#[tokio::test]
async fn test_create_task_happy_path() {
    let app = test_app();
    let body = json!({
        "title": "Gather wood",
        "description": "Collect 10 wood",
        "reward": 50,
        "publisher_id": "agent-1",
    });
    let (status, resp) = send(&app, json_post("/api/v1/tasks", body)).await;
    assert_eq!(status, StatusCode::CREATED, "resp: {resp:?}");
    assert_eq!(resp["title"], "Gather wood");
    assert_eq!(resp["reward"], 50);
    assert_eq!(resp["status"], "published");
    assert_eq!(resp["publisher_id"], "agent-1");
    assert!(resp["id"].as_str().unwrap().len() > 0);
}

#[tokio::test]
async fn test_create_task_empty_title() {
    let app = test_app();
    let body = json!({"title": "", "publisher_id": "agent-1"});
    let (status, _) = send(&app, json_post("/api/v1/tasks", body)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_create_task_empty_publisher_id() {
    let app = test_app();
    let body = json!({"title": "Test task", "publisher_id": ""});
    let (status, _) = send(&app, json_post("/api/v1/tasks", body)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_list_tasks_empty() {
    let app = test_app();
    let (status, resp) = send(&app, get_req("/api/v1/tasks")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(resp.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_list_tasks_after_create() {
    let app = test_app();
    let body = json!({"title": "Task A", "publisher_id": "p1", "reward": 10});
    send(&app, json_post("/api/v1/tasks", body)).await;
    let (status, resp) = send(&app, get_req("/api/v1/tasks")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(resp.as_array().unwrap().len(), 1);
    assert_eq!(resp[0]["title"], "Task A");
}

#[tokio::test]
async fn test_get_task_happy_path() {
    let app = test_app();
    let body = json!({"title": "Read task", "publisher_id": "p1", "reward": 20});
    let (_, cr) = send(&app, json_post("/api/v1/tasks", body)).await;
    let task_id = cr["id"].as_str().unwrap();
    let (status, resp) = send(&app, get_req(&format!("/api/v1/tasks/{task_id}"))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(resp["title"], "Read task");
}

#[tokio::test]
async fn test_get_task_not_found() {
    let app = test_app();
    let fake_id = uuid::Uuid::new_v4().to_string();
    let (status, _) = send(&app, get_req(&format!("/api/v1/tasks/{fake_id}"))).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_get_task_invalid_uuid() {
    let app = test_app();
    let (status, _) = send(&app, get_req("/api/v1/tasks/not-a-uuid")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_task_full_lifecycle_approved() {
    let app = test_app();
    let body = json!({"title": "Build house", "publisher_id": "builder-1", "reward": 100});
    let (_, cr) = send(&app, json_post("/api/v1/tasks", body)).await;
    let tid = cr["id"].as_str().unwrap();
    assert_eq!(cr["status"], "published");

    // claim
    let (s, r) = send(&app, json_post(&format!("/api/v1/tasks/{tid}/claim"), json!({"assignee_id": "worker-1"}))).await;
    assert_eq!(s, StatusCode::OK, "claim: {r:?}");
    assert_eq!(r["status"], "claimed");

    // start
    let req = Request::builder().method("POST").uri(&format!("/api/v1/tasks/{tid}/start")).body(Body::empty()).unwrap();
    let (s, r) = send(&app, req).await;
    assert_eq!(s, StatusCode::OK, "start: {r:?}");
    assert_eq!(r["status"], "in_progress");

    // submit
    let (s, r) = send(&app, json_post(&format!("/api/v1/tasks/{tid}/submit"), json!({"result": "Done"}))).await;
    assert_eq!(s, StatusCode::OK, "submit: {r:?}");
    assert_eq!(r["status"], "submitted");

    // review approve
    let (s, r) = send(&app, json_post(&format!("/api/v1/tasks/{tid}/review"), json!({"approved": true, "reviewer_id": "builder-1"}))).await;
    assert_eq!(s, StatusCode::OK, "review: {r:?}");
    assert_eq!(r["status"], "reviewed");
}

#[tokio::test]
async fn test_task_review_rejection() {
    let app = test_app();
    let body = json!({"title": "Cook", "publisher_id": "chef-1", "reward": 30});
    let (_, cr) = send(&app, json_post("/api/v1/tasks", body)).await;
    let tid = cr["id"].as_str().unwrap();

    send(&app, json_post(&format!("/api/v1/tasks/{tid}/claim"), json!({"assignee_id": "cook-1"}))).await;
    let req = Request::builder().method("POST").uri(&format!("/api/v1/tasks/{tid}/start")).body(Body::empty()).unwrap();
    send(&app, req).await;
    send(&app, json_post(&format!("/api/v1/tasks/{tid}/submit"), json!({"result": "Burnt"}))).await;

    let (s, r) = send(&app, json_post(&format!("/api/v1/tasks/{tid}/review"), json!({"approved": false, "reviewer_id": "chef-1"}))).await;
    assert_eq!(s, StatusCode::OK, "reject: {r:?}");
    assert_eq!(r["status"], "in_progress");
}

#[tokio::test]
async fn test_claim_nonexistent_task() {
    let app = test_app();
    let fake = uuid::Uuid::new_v4().to_string();
    let (s, _) = send(&app, json_post(&format!("/api/v1/tasks/{fake}/claim"), json!({"assignee_id": "w"}))).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_submit_without_start() {
    let app = test_app();
    let body = json!({"title": "Skip", "publisher_id": "p1", "reward": 10});
    let (_, cr) = send(&app, json_post("/api/v1/tasks", body)).await;
    let tid = cr["id"].as_str().unwrap();
    let (s, _) = send(&app, json_post(&format!("/api/v1/tasks/{tid}/submit"), json!({"result": "x"}))).await;
    assert_eq!(s, StatusCode::CONFLICT);
}

#[tokio::test]
async fn test_complete_open_task() {
    let app = test_app();
    let body = json!({"title": "Too early", "publisher_id": "p1", "reward": 10});
    let (_, cr) = send(&app, json_post("/api/v1/tasks", body)).await;
    let tid = cr["id"].as_str().unwrap();
    let req = Request::builder().method("POST").uri(&format!("/api/v1/tasks/{tid}/complete")).body(Body::empty()).unwrap();
    let (s, _) = send(&app, req).await;
    assert_eq!(s, StatusCode::CONFLICT);
}

#[tokio::test]
async fn test_expire_task() {
    let app = test_app();
    let body = json!({"title": "Expire", "publisher_id": "p1", "reward": 5});
    let (_, cr) = send(&app, json_post("/api/v1/tasks", body)).await;
    let tid = cr["id"].as_str().unwrap();
    let req = Request::builder().method("POST").uri(&format!("/api/v1/tasks/{tid}/expire")).body(Body::empty()).unwrap();
    let (s, r) = send(&app, req).await;
    assert_eq!(s, StatusCode::OK, "expire: {r:?}");
    assert_eq!(r["status"], "expired");
}

#[tokio::test]
async fn test_delete_task() {
    let app = test_app();
    let body = json!({"title": "Del", "publisher_id": "p1", "reward": 1});
    let (_, cr) = send(&app, json_post("/api/v1/tasks", body)).await;
    let tid = cr["id"].as_str().unwrap();
    let (s, _) = send(&app, delete_req(&format!("/api/v1/tasks/{tid}"))).await;
    assert_eq!(s, StatusCode::NO_CONTENT);
    let (s, _) = send(&app, get_req(&format!("/api/v1/tasks/{tid}"))).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_delete_nonexistent_task() {
    let app = test_app();
    let fake = uuid::Uuid::new_v4().to_string();
    let (s, _) = send(&app, delete_req(&format!("/api/v1/tasks/{fake}"))).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
}
