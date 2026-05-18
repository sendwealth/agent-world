use std::convert::Infallible;
use std::sync::Arc;

use axum::{
    Json,
    Router,
    extract::{Path, State},
    http::StatusCode,
    response::{
        sse::{Event as SseEvent, KeepAlive, Sse},
        IntoResponse,
    },
    routing::{delete, get, post},
};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tower_http::cors::CorsLayer;
use uuid::Uuid;

use crate::economy::task::{TaskBoard, Task};
use crate::wal::WAL;
use crate::world::state::SharedEventBus;

// ── Shared State ──────────────────────────────────────────

pub type SharedTaskBoard = Arc<Mutex<TaskBoard>>;
pub type SharedWAL = Arc<Mutex<WAL>>;

/// Combined state for the API with WAL support.
#[derive(Clone)]
pub struct AppState {
    pub board: SharedTaskBoard,
    pub wal: SharedWAL,
    pub event_bus: Option<SharedEventBus>,
}

pub fn create_router(board: SharedTaskBoard) -> Router {
    Router::new()
        .route("/tasks", post(create_task))
        .route("/tasks", get(list_tasks))
        .route("/tasks/:id", get(get_task))
        .route("/tasks/:id/claim", post(claim_task))
        .route("/tasks/:id/start", post(start_task))
        .route("/tasks/:id/submit", post(submit_task))
        .route("/tasks/:id/review", post(review_task))
        .route("/tasks/:id/complete", post(complete_task))
        .route("/tasks/:id/expire", post(expire_task))
        .route("/tasks/:id", delete(delete_task))
        .with_state(board)
}

pub fn create_router_with_wal(board: SharedTaskBoard, wal: SharedWAL, event_bus: SharedEventBus) -> Router {
    let state = AppState { board, wal, event_bus: Some(event_bus) };
    Router::new()
        // Task routes
        .route("/tasks", post(create_task_with_wal))
        .route("/tasks", get(list_tasks_with_wal))
        .route("/tasks/:id", get(get_task_with_wal))
        .route("/tasks/:id/claim", post(claim_task_with_wal))
        .route("/tasks/:id/start", post(start_task_with_wal))
        .route("/tasks/:id/submit", post(submit_task_with_wal))
        .route("/tasks/:id/review", post(review_task_with_wal))
        .route("/tasks/:id/complete", post(complete_task_with_wal))
        .route("/tasks/:id/expire", post(expire_task_with_wal))
        .route("/tasks/:id", delete(delete_task_with_wal))
        // WAL routes
        .route("/wal/stats", get(wal_stats))
        .route("/wal/snapshot", post(wal_snapshot))
        .route("/wal/verify", get(wal_verify))
        // SSE endpoint
        .route("/api/v1/world/events", get(world_events_sse))
        .with_state(state)
        .layer(CorsLayer::permissive())
}

// ── Request Types ─────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreateTaskRequest {
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub reward: u64,
    pub publisher_id: String,
    pub expires_at: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct ClaimTaskRequest {
    pub assignee_id: String,
}

#[derive(Debug, Deserialize)]
pub struct SubmitTaskRequest {
    pub result: String,
}

#[derive(Debug, Deserialize)]
pub struct ReviewTaskRequest {
    pub approved: bool,
    pub reviewer_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct ListTasksQuery {
    pub status: Option<String>,
    pub publisher_id: Option<String>,
    pub assignee_id: Option<String>,
}

impl Default for ListTasksQuery {
    fn default() -> Self {
        Self {
            status: None,
            publisher_id: None,
            assignee_id: None,
        }
    }
}

// ── Response Types ────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct TaskResponse {
    pub id: String,
    pub title: String,
    pub description: String,
    pub status: String,
    pub reward: u64,
    pub escrow_held: bool,
    pub publisher_id: String,
    pub assignee_id: Option<String>,
    pub result: Option<String>,
    pub expires_at: Option<u64>,
    pub created_tick: u64,
}

impl From<&Task> for TaskResponse {
    fn from(task: &Task) -> Self {
        TaskResponse {
            id: task.id.to_string(),
            title: task.title.clone(),
            description: task.description.clone(),
            status: task.status.to_string(),
            reward: task.reward,
            escrow_held: task.escrow_held,
            publisher_id: task.publisher_id.clone(),
            assignee_id: task.assignee_id.clone(),
            result: task.result.clone(),
            expires_at: task.expires_at,
            created_tick: task.created_tick,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

// ── Handlers ──────────────────────────────────────────────

async fn create_task(
    State(board): State<SharedTaskBoard>,
    Json(body): Json<CreateTaskRequest>,
) -> impl IntoResponse {
    if body.title.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "title is required".into() })).into_response();
    }
    if body.publisher_id.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "publisher_id is required".into() })).into_response();
    }

    let mut board = board.lock().await;
    match board.create_task(
        body.title,
        body.description,
        body.reward,
        body.publisher_id,
        0, // created_tick — would come from world clock in production
        body.expires_at,
    ) {
        Ok(id) => {
            let task = board.get(id).unwrap();
            (StatusCode::CREATED, Json(TaskResponse::from(task))).into_response()
        }
        Err(e) => {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: e.to_string() })).into_response()
        }
    }
}

async fn list_tasks(
    State(board): State<SharedTaskBoard>,
) -> impl IntoResponse {
    let board = board.lock().await;
    let tasks: Vec<TaskResponse> = board.list().into_iter().map(TaskResponse::from).collect();
    Json(tasks).into_response()
}

async fn get_task(
    State(board): State<SharedTaskBoard>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let Ok(uuid) = Uuid::parse_str(&id) else {
        return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "invalid task id".into() })).into_response();
    };

    let board = board.lock().await;
    match board.get(uuid) {
        Some(task) => Json(TaskResponse::from(task)).into_response(),
        None => (StatusCode::NOT_FOUND, Json(ErrorResponse { error: "task not found".into() })).into_response(),
    }
}

async fn claim_task(
    State(board): State<SharedTaskBoard>,
    Path(id): Path<String>,
    Json(body): Json<ClaimTaskRequest>,
) -> impl IntoResponse {
    let Ok(uuid) = Uuid::parse_str(&id) else {
        return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "invalid task id".into() })).into_response();
    };

    let mut board = board.lock().await;
    match board.claim_task(uuid, body.assignee_id) {
        Ok(()) => {
            let task = board.get(uuid).unwrap();
            Json(TaskResponse::from(task)).into_response()
        }
        Err(e) => {
            let status = match &e {
                crate::economy::task::TaskError::InvalidTransition { .. } => StatusCode::CONFLICT,
                crate::economy::task::TaskError::NotFound(_) => StatusCode::NOT_FOUND,
                _ => StatusCode::BAD_REQUEST,
            };
            (status, Json(ErrorResponse { error: e.to_string() })).into_response()
        }
    }
}

async fn start_task(
    State(board): State<SharedTaskBoard>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let Ok(uuid) = Uuid::parse_str(&id) else {
        return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "invalid task id".into() })).into_response();
    };

    let mut board = board.lock().await;
    match board.start_task(uuid) {
        Ok(()) => {
            let task = board.get(uuid).unwrap();
            Json(TaskResponse::from(task)).into_response()
        }
        Err(e) => {
            let status = match &e {
                crate::economy::task::TaskError::InvalidTransition { .. } => StatusCode::CONFLICT,
                crate::economy::task::TaskError::NotFound(_) => StatusCode::NOT_FOUND,
                _ => StatusCode::BAD_REQUEST,
            };
            (status, Json(ErrorResponse { error: e.to_string() })).into_response()
        }
    }
}

async fn submit_task(
    State(board): State<SharedTaskBoard>,
    Path(id): Path<String>,
    Json(body): Json<SubmitTaskRequest>,
) -> impl IntoResponse {
    let Ok(uuid) = Uuid::parse_str(&id) else {
        return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "invalid task id".into() })).into_response();
    };

    let mut board = board.lock().await;
    match board.submit_result(uuid, body.result) {
        Ok(()) => {
            let task = board.get(uuid).unwrap();
            Json(TaskResponse::from(task)).into_response()
        }
        Err(e) => {
            let status = match &e {
                crate::economy::task::TaskError::InvalidTransition { .. } => StatusCode::CONFLICT,
                crate::economy::task::TaskError::NotFound(_) => StatusCode::NOT_FOUND,
                crate::economy::task::TaskError::ResultRequired => StatusCode::BAD_REQUEST,
                _ => StatusCode::BAD_REQUEST,
            };
            (status, Json(ErrorResponse { error: e.to_string() })).into_response()
        }
    }
}

async fn review_task(
    State(board): State<SharedTaskBoard>,
    Path(id): Path<String>,
    Json(body): Json<ReviewTaskRequest>,
) -> impl IntoResponse {
    let Ok(uuid) = Uuid::parse_str(&id) else {
        return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "invalid task id".into() })).into_response();
    };

    let mut board = board.lock().await;
    match board.review_task(uuid, &body.reviewer_id, body.approved) {
        Ok(()) => {
            let task = board.get(uuid).unwrap();
            Json(TaskResponse::from(task)).into_response()
        }
        Err(e) => {
            let status = match &e {
                crate::economy::task::TaskError::InvalidTransition { .. } => StatusCode::CONFLICT,
                crate::economy::task::TaskError::NotFound(_) => StatusCode::NOT_FOUND,
                crate::economy::task::TaskError::NotPublisher { .. } => StatusCode::FORBIDDEN,
                _ => StatusCode::BAD_REQUEST,
            };
            (status, Json(ErrorResponse { error: e.to_string() })).into_response()
        }
    }
}

async fn complete_task(
    State(board): State<SharedTaskBoard>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let Ok(uuid) = Uuid::parse_str(&id) else {
        return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "invalid task id".into() })).into_response();
    };

    let mut board = board.lock().await;
    match board.complete_task(uuid, 0) {
        Ok(_) => {
            let task = board.get(uuid).unwrap();
            Json(TaskResponse::from(task)).into_response()
        }
        Err(e) => {
            let status = match &e {
                crate::economy::task::TaskError::InvalidTransition { .. } => StatusCode::CONFLICT,
                crate::economy::task::TaskError::NotFound(_) => StatusCode::NOT_FOUND,
                _ => StatusCode::BAD_REQUEST,
            };
            (status, Json(ErrorResponse { error: e.to_string() })).into_response()
        }
    }
}

async fn expire_task(
    State(board): State<SharedTaskBoard>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let Ok(uuid) = Uuid::parse_str(&id) else {
        return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "invalid task id".into() })).into_response();
    };

    let mut board = board.lock().await;
    match board.expire_task(uuid) {
        Ok(()) => {
            let task = board.get(uuid).unwrap();
            Json(TaskResponse::from(task)).into_response()
        }
        Err(e) => {
            let status = match &e {
                crate::economy::task::TaskError::InvalidTransition { .. } => StatusCode::CONFLICT,
                crate::economy::task::TaskError::NotFound(_) => StatusCode::NOT_FOUND,
                _ => StatusCode::BAD_REQUEST,
            };
            (status, Json(ErrorResponse { error: e.to_string() })).into_response()
        }
    }
}

async fn delete_task(
    State(board): State<SharedTaskBoard>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let Ok(uuid) = Uuid::parse_str(&id) else {
        return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "invalid task id".into() })).into_response();
    };

    let mut board = board.lock().await;
    match board.delete_task(uuid) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => {
            let status = match &e {
                crate::economy::task::TaskError::InvalidTransition { .. } => StatusCode::CONFLICT,
                crate::economy::task::TaskError::NotFound(_) => StatusCode::NOT_FOUND,
                _ => StatusCode::BAD_REQUEST,
            };
            (status, Json(ErrorResponse { error: e.to_string() })).into_response()
        }
    }
}

// ── Task Handlers (with WAL state) ────────────────────────

async fn create_task_with_wal(
    State(state): State<AppState>,
    Json(body): Json<CreateTaskRequest>,
) -> impl IntoResponse {
    create_task(State(state.board), Json(body)).await
}

async fn list_tasks_with_wal(
    State(state): State<AppState>,
) -> impl IntoResponse {
    list_tasks(State(state.board)).await
}

async fn get_task_with_wal(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    get_task(State(state.board), Path(id)).await
}

async fn claim_task_with_wal(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<ClaimTaskRequest>,
) -> impl IntoResponse {
    claim_task(State(state.board), Path(id), Json(body)).await
}

async fn start_task_with_wal(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    start_task(State(state.board), Path(id)).await
}

async fn submit_task_with_wal(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<SubmitTaskRequest>,
) -> impl IntoResponse {
    submit_task(State(state.board), Path(id), Json(body)).await
}

async fn review_task_with_wal(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<ReviewTaskRequest>,
) -> impl IntoResponse {
    review_task(State(state.board), Path(id), Json(body)).await
}

async fn complete_task_with_wal(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    complete_task(State(state.board), Path(id)).await
}

async fn expire_task_with_wal(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    expire_task(State(state.board), Path(id)).await
}

async fn delete_task_with_wal(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    delete_task(State(state.board), Path(id)).await
}

// ── WAL Handlers ──────────────────────────────────────────

async fn wal_stats(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let wal = state.wal.lock().await;
    Json(wal.stats())
}

async fn wal_snapshot(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let mut wal = state.wal.lock().await;
    match wal.take_snapshot(&[], 0) {
        Ok(snapshot_file) => Json(serde_json::json!({ "ok": true, "snapshot_file": snapshot_file })).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: e.to_string() })).into_response(),
    }
}

async fn wal_verify(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let mut wal = state.wal.lock().await;
    let result = wal.recover();
    match result {
        Ok(recovery) => Json(serde_json::json!({
            "consistent": !recovery.corrupted_records,
            "event_count": recovery.event_counter,
            "recovered_from_snapshot": recovery.recovered_from_snapshot,
        })).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: e.to_string() })).into_response(),
    }
}

// ── SSE Handler ──────────────────────────────────────────

async fn world_events_sse(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let event_bus = match state.event_bus {
        Some(ref bus) => bus.clone(),
        None => {
            return (StatusCode::SERVICE_UNAVAILABLE, Json(ErrorResponse {
                error: "event bus not available".into(),
            })).into_response();
        }
    };

    let stream = async_stream::stream! {
        let mut rx = event_bus.subscribe();
        loop {
            match rx.recv().await {
                Ok(event) => {
                    let json = event.to_json();
                    yield Ok::<_, Infallible>(SseEvent::default().data(json));
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!("SSE client lagged {} events, skipping", n);
                    continue;
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    break;
                }
            }
        }
    };

    let sse = Sse::new(stream).keep_alive(KeepAlive::default());
    sse.into_response()
}
