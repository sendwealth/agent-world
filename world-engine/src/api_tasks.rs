use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::api::{AppState, ErrorResponse, SharedTaskBoard};
use crate::economy::task::Task;

// ── Task Request Types ─────────────────────────────────────

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

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ListTasksQuery {
    pub status: Option<String>,
    pub publisher_id: Option<String>,
    pub assignee_id: Option<String>,
}

// ── Task Response Types ─────────────────────────────────────

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

// ── Task Handlers (no WAL) ────────────────────────────────

pub async fn create_task(
    State(board): State<SharedTaskBoard>,
    Json(body): Json<CreateTaskRequest>,
) -> impl IntoResponse {
    if body.title.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "title is required".into(),
            }),
        )
            .into_response();
    }
    if body.publisher_id.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "publisher_id is required".into(),
            }),
        )
            .into_response();
    }

    let mut board = board.lock().await;
    match board.create_task(
        body.title,
        body.description,
        body.reward,
        body.publisher_id,
        0,
        body.expires_at,
    ) {
        Ok(id) => {
            let task = match board.get(id) {
                Some(t) => t,
                None => return (StatusCode::NOT_FOUND, Json(ErrorResponse { error: "task not found".into() })).into_response(),
            };
            (StatusCode::CREATED, Json(TaskResponse::from(task))).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
            .into_response(),
    }
}

pub async fn list_tasks(State(board): State<SharedTaskBoard>) -> impl IntoResponse {
    let board = board.lock().await;
    let tasks: Vec<TaskResponse> = board.list().into_iter().map(TaskResponse::from).collect();
    Json(tasks).into_response()
}

pub async fn get_task(
    State(board): State<SharedTaskBoard>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let Ok(uuid) = Uuid::parse_str(&id) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "invalid task id".into(),
            }),
        )
            .into_response();
    };

    let board = board.lock().await;
    match board.get(uuid) {
        Some(task) => Json(TaskResponse::from(task)).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "task not found".into(),
            }),
        )
            .into_response(),
    }
}

pub async fn claim_task(
    State(board): State<SharedTaskBoard>,
    Path(id): Path<String>,
    Json(body): Json<ClaimTaskRequest>,
) -> impl IntoResponse {
    let Ok(uuid) = Uuid::parse_str(&id) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "invalid task id".into(),
            }),
        )
            .into_response();
    };

    let mut board = board.lock().await;
    match board.claim_task(uuid, body.assignee_id) {
        Ok(()) => {
            let task = match board.get(uuid) {
                Some(t) => t,
                None => return (StatusCode::NOT_FOUND, Json(ErrorResponse { error: "task not found".into() })).into_response(),
            };
            Json(TaskResponse::from(task)).into_response()
        }
        Err(e) => {
            let status = match &e {
                crate::economy::task::TaskError::InvalidTransition { .. } => StatusCode::CONFLICT,
                crate::economy::task::TaskError::NotFound(_) => StatusCode::NOT_FOUND,
                _ => StatusCode::BAD_REQUEST,
            };
            (
                status,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
                .into_response()
        }
    }
}

pub async fn start_task(
    State(board): State<SharedTaskBoard>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let Ok(uuid) = Uuid::parse_str(&id) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "invalid task id".into(),
            }),
        )
            .into_response();
    };

    let mut board = board.lock().await;
    match board.start_task(uuid) {
        Ok(()) => {
            let task = match board.get(uuid) {
                Some(t) => t,
                None => return (StatusCode::NOT_FOUND, Json(ErrorResponse { error: "task not found".into() })).into_response(),
            };
            Json(TaskResponse::from(task)).into_response()
        }
        Err(e) => {
            let status = match &e {
                crate::economy::task::TaskError::InvalidTransition { .. } => StatusCode::CONFLICT,
                crate::economy::task::TaskError::NotFound(_) => StatusCode::NOT_FOUND,
                _ => StatusCode::BAD_REQUEST,
            };
            (
                status,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
                .into_response()
        }
    }
}

pub async fn submit_task(
    State(board): State<SharedTaskBoard>,
    Path(id): Path<String>,
    Json(body): Json<SubmitTaskRequest>,
) -> impl IntoResponse {
    let Ok(uuid) = Uuid::parse_str(&id) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "invalid task id".into(),
            }),
        )
            .into_response();
    };

    let mut board = board.lock().await;
    match board.submit_result(uuid, body.result) {
        Ok(()) => {
            let task = match board.get(uuid) {
                Some(t) => t,
                None => return (StatusCode::NOT_FOUND, Json(ErrorResponse { error: "task not found".into() })).into_response(),
            };
            Json(TaskResponse::from(task)).into_response()
        }
        Err(e) => {
            let status = match &e {
                crate::economy::task::TaskError::InvalidTransition { .. } => StatusCode::CONFLICT,
                crate::economy::task::TaskError::NotFound(_) => StatusCode::NOT_FOUND,
                crate::economy::task::TaskError::ResultRequired => StatusCode::BAD_REQUEST,
                _ => StatusCode::BAD_REQUEST,
            };
            (
                status,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
                .into_response()
        }
    }
}

pub async fn review_task(
    State(board): State<SharedTaskBoard>,
    Path(id): Path<String>,
    Json(body): Json<ReviewTaskRequest>,
) -> impl IntoResponse {
    let Ok(uuid) = Uuid::parse_str(&id) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "invalid task id".into(),
            }),
        )
            .into_response();
    };

    let mut board = board.lock().await;
    match board.review_task(uuid, &body.reviewer_id, body.approved) {
        Ok(()) => {
            let task = match board.get(uuid) {
                Some(t) => t,
                None => return (StatusCode::NOT_FOUND, Json(ErrorResponse { error: "task not found".into() })).into_response(),
            };
            Json(TaskResponse::from(task)).into_response()
        }
        Err(e) => {
            let status = match &e {
                crate::economy::task::TaskError::InvalidTransition { .. } => StatusCode::CONFLICT,
                crate::economy::task::TaskError::NotFound(_) => StatusCode::NOT_FOUND,
                crate::economy::task::TaskError::NotPublisher { .. } => StatusCode::FORBIDDEN,
                _ => StatusCode::BAD_REQUEST,
            };
            (
                status,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
                .into_response()
        }
    }
}

pub async fn complete_task(
    State(board): State<SharedTaskBoard>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let Ok(uuid) = Uuid::parse_str(&id) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "invalid task id".into(),
            }),
        )
            .into_response();
    };

    let mut board = board.lock().await;
    match board.complete_task(uuid, 0) {
        Ok(_) => {
            let task = match board.get(uuid) {
                Some(t) => t,
                None => return (StatusCode::NOT_FOUND, Json(ErrorResponse { error: "task not found".into() })).into_response(),
            };
            Json(TaskResponse::from(task)).into_response()
        }
        Err(e) => {
            let status = match &e {
                crate::economy::task::TaskError::InvalidTransition { .. } => StatusCode::CONFLICT,
                crate::economy::task::TaskError::NotFound(_) => StatusCode::NOT_FOUND,
                _ => StatusCode::BAD_REQUEST,
            };
            (
                status,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
                .into_response()
        }
    }
}

pub async fn expire_task(
    State(board): State<SharedTaskBoard>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let Ok(uuid) = Uuid::parse_str(&id) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "invalid task id".into(),
            }),
        )
            .into_response();
    };

    let mut board = board.lock().await;
    match board.expire_task(uuid) {
        Ok(()) => {
            let task = match board.get(uuid) {
                Some(t) => t,
                None => return (StatusCode::NOT_FOUND, Json(ErrorResponse { error: "task not found".into() })).into_response(),
            };
            Json(TaskResponse::from(task)).into_response()
        }
        Err(e) => {
            let status = match &e {
                crate::economy::task::TaskError::InvalidTransition { .. } => StatusCode::CONFLICT,
                crate::economy::task::TaskError::NotFound(_) => StatusCode::NOT_FOUND,
                _ => StatusCode::BAD_REQUEST,
            };
            (
                status,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
                .into_response()
        }
    }
}

pub async fn delete_task(
    State(board): State<SharedTaskBoard>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let Ok(uuid) = Uuid::parse_str(&id) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "invalid task id".into(),
            }),
        )
            .into_response();
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
            (
                status,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
                .into_response()
        }
    }
}

// ── Task Handlers (with WAL state) ────────────────────────

pub async fn create_task_with_wal(
    State(state): State<AppState>,
    Json(body): Json<CreateTaskRequest>,
) -> impl IntoResponse {
    create_task(State(state.board), Json(body)).await
}

pub async fn list_tasks_with_wal(State(state): State<AppState>) -> impl IntoResponse {
    list_tasks(State(state.board)).await
}

pub async fn get_task_with_wal(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    get_task(State(state.board), Path(id)).await
}

pub async fn claim_task_with_wal(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<ClaimTaskRequest>,
) -> axum::response::Response {
    // Check reputation eligibility for high-value tasks
    if let Some(rep) = &state.reputation_system {
        if let Ok(uuid) = Uuid::parse_str(&id) {
            let board = state.board.lock().await;
            if let Some(task) = board.get(uuid) {
                let reward = task.reward;
                drop(board);
                let rep = rep.lock().await;
                if let Err(e) = rep.check_claim_eligibility(&body.assignee_id, reward) {
                    return (StatusCode::FORBIDDEN, Json(ErrorResponse { error: e }))
                        .into_response();
                }
            }
        }
    }
    claim_task(State(state.board), Path(id), Json(body))
        .await
        .into_response()
}

pub async fn start_task_with_wal(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    start_task(State(state.board), Path(id)).await
}

pub async fn submit_task_with_wal(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<SubmitTaskRequest>,
) -> impl IntoResponse {
    submit_task(State(state.board), Path(id), Json(body)).await
}

pub async fn review_task_with_wal(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<ReviewTaskRequest>,
) -> impl IntoResponse {
    review_task(State(state.board), Path(id), Json(body)).await
}

pub async fn complete_task_with_wal(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let id_for_rep = id.clone();
    let result = complete_task(State(state.board.clone()), Path(id)).await;

    // Update reputation on successful completion
    if let Some(rep) = &state.reputation_system {
        if let Ok(uuid) = Uuid::parse_str(&id_for_rep) {
            let board = state.board.lock().await;
            if let Some(task) = board.get(uuid) {
                if task.status == crate::economy::task::TaskStatus::Completed {
                    if let Some(ref assignee) = task.assignee_id {
                        let tick = *state.tick_rx.borrow();
                        let mut rep = rep.lock().await;
                        rep.on_task_completed_on_time(assignee, tick);
                    }
                }
            }
        }
    }

    result
}

pub async fn expire_task_with_wal(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let id_for_rep = id.clone();

    // Capture pre-expiry state to determine penalty type
    let pre_expiry_assignee = if let Ok(uuid) = Uuid::parse_str(&id) {
        let board = state.board.lock().await;
        board.get(uuid).and_then(|t| t.assignee_id.clone())
    } else {
        None
    };
    let pre_expiry_publisher = if let Ok(uuid) = Uuid::parse_str(&id) {
        let board = state.board.lock().await;
        board.get(uuid).map(|t| t.publisher_id.clone())
    } else {
        None
    };

    let result = expire_task(State(state.board.clone()), Path(id)).await;

    // Apply reputation penalties on successful expiry
    if let Some(rep) = &state.reputation_system {
        if let Ok(uuid) = Uuid::parse_str(&id_for_rep) {
            let board = state.board.lock().await;
            if let Some(task) = board.get(uuid) {
                if task.status == crate::economy::task::TaskStatus::Expired {
                    let tick = *state.tick_rx.borrow();
                    let mut rep = rep.lock().await;
                    if let Some(assignee) = pre_expiry_assignee {
                        // Claimed task expired — breach penalty
                        rep.on_task_breach(&assignee, tick);
                    } else {
                        // Published task expired — publisher penalty
                        if let Some(publisher) = pre_expiry_publisher {
                            rep.on_task_expired_published(&publisher, tick);
                        }
                    }
                }
            }
        }
    }

    result
}

pub async fn delete_task_with_wal(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    delete_task(State(state.board), Path(id)).await
}

// ── WAL Handlers ──────────────────────────────────────────

pub async fn wal_stats(State(state): State<AppState>) -> impl IntoResponse {
    let wal = state.wal.lock().await;
    Json(wal.stats())
}

pub async fn wal_snapshot(State(state): State<AppState>) -> impl IntoResponse {
    let mut wal = state.wal.lock().await;
    match wal.take_snapshot(&[], 0) {
        Ok(snapshot_file) => {
            Json(serde_json::json!({ "ok": true, "snapshot_file": snapshot_file })).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
            .into_response(),
    }
}

pub async fn wal_verify(State(state): State<AppState>) -> impl IntoResponse {
    let mut wal = state.wal.lock().await;
    let result = wal.recover();
    match result {
        Ok(recovery) => Json(serde_json::json!({
            "consistent": !recovery.corrupted_records,
            "event_count": recovery.event_counter,
            "recovered_from_snapshot": recovery.recovered_from_snapshot,
        }))
        .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
            .into_response(),
    }
}

/// Task + WAL routes for merging into the full router.
pub fn task_routes() -> axum::Router<AppState> {
    use axum::routing::*;
    axum::Router::new()
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
        .route("/wal/stats", get(wal_stats))
        .route("/wal/snapshot", post(wal_snapshot))
        .route("/wal/verify", get(wal_verify))
}
