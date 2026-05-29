use std::collections::HashMap;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::api::{AppState, ErrorResponse};
use crate::economy::task::{CoordinationTask, CoordinationTaskError};

// ── Request Types ─────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreateCoordinationTaskRequest {
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub reward_pool: u64,
    #[serde(default)]
    pub currency: Option<String>,
    pub coordinator_id: String,
    #[serde(default = "default_max_agents")]
    pub max_agents: usize,
    pub expires_at: Option<u64>,
    /// If set, only members of this org can join the task.
    pub org_id: Option<String>,
}

fn default_max_agents() -> usize {
    5
}

#[derive(Debug, Deserialize)]
pub struct JoinCoordinationTaskRequest {
    pub agent_id: String,
}

#[derive(Debug, Deserialize)]
pub struct SubmitContributionRequest {
    pub agent_id: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
pub struct CompleteCoordinationTaskRequest {
    pub reviewer_id: String,
    pub reward_overrides: Option<HashMap<String, u64>>,
}

#[derive(Debug, Deserialize)]
pub struct CancelCoordinationTaskRequest {
    pub coordinator_id: String,
}

// ── Response Types ─────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct CoordinationTaskResponse {
    pub id: String,
    pub title: String,
    pub description: String,
    pub status: String,
    pub reward_pool: u64,
    pub currency: String,
    pub escrow_held: bool,
    pub coordinator_id: String,
    pub max_agents: usize,
    pub participants: Vec<String>,
    pub contributions: HashMap<String, ContributionResponse>,
    pub reward_overrides: HashMap<String, u64>,
    pub org_id: Option<String>,
    pub expires_at: Option<u64>,
    pub created_tick: u64,
}

#[derive(Debug, Serialize)]
pub struct ContributionResponse {
    pub agent_id: String,
    pub content: String,
    pub submitted_tick: u64,
}

impl From<&CoordinationTask> for CoordinationTaskResponse {
    fn from(task: &CoordinationTask) -> Self {
        CoordinationTaskResponse {
            id: task.id.to_string(),
            title: task.title.clone(),
            description: task.description.clone(),
            status: task.status.to_string(),
            reward_pool: task.reward_pool,
            currency: currency_to_string(task.currency),
            escrow_held: task.escrow_held,
            coordinator_id: task.coordinator_id.clone(),
            max_agents: task.max_agents,
            participants: task.participants.clone(),
            contributions: task
                .contributions
                .iter()
                .map(|(k, v)| {
                    (
                        k.clone(),
                        ContributionResponse {
                            agent_id: v.agent_id.clone(),
                            content: v.content.clone(),
                            submitted_tick: v.submitted_tick,
                        },
                    )
                })
                .collect(),
            reward_overrides: task.reward_overrides.clone(),
            org_id: task.org_id.clone(),
            expires_at: task.expires_at,
            created_tick: task.created_tick,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct RewardDistributionResponse {
    pub task_id: String,
    pub distribution: HashMap<String, u64>,
}

// ── Currency Helper ─────────────────────────────────────────

fn parse_currency(s: Option<&String>) -> crate::world::enums::Currency {
    match s.map(|v| v.as_str()) {
        Some("token") => crate::world::enums::Currency::Token,
        _ => crate::world::enums::Currency::Money,
    }
}

fn currency_to_string(c: crate::world::enums::Currency) -> String {
    match c {
        crate::world::enums::Currency::Token => "token".to_string(),
        crate::world::enums::Currency::Money => "money".to_string(),
    }
}

// ── Handlers ────────────────────────────────────────────────

pub async fn create_coordination_task(
    State(state): State<AppState>,
    Json(body): Json<CreateCoordinationTaskRequest>,
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
    if body.coordinator_id.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "coordinator_id is required".into(),
            }),
        )
            .into_response();
    }
    if body.max_agents == 0 {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "max_agents must be at least 1".into(),
            }),
        )
            .into_response();
    }

    let currency = parse_currency(body.currency.as_ref());

    // If org_id is set, optionally verify the coordinator is an org member
    if let Some(ref org_id) = body.org_id {
        if let Some(ref org_store) = state.org_store {
            let orgs = org_store.lock().await;
            if let Some(org) = orgs.get(org_id) {
                let is_member = org.members.iter().any(|m| m.agent_id == body.coordinator_id);
                if !is_member {
                    return (
                        StatusCode::FORBIDDEN,
                        Json(ErrorResponse {
                            error: format!(
                                "coordinator {} is not a member of organization {}",
                                body.coordinator_id, org_id
                            ),
                        }),
                    )
                        .into_response();
                }
            }
        }
    }

    let tick = *state.tick_rx.borrow();
    let mut board = state.board.lock().await;
    match board.create_coordination_task(
        body.title,
        body.description,
        body.reward_pool,
        currency,
        body.coordinator_id,
        body.max_agents,
        tick,
        body.expires_at,
        body.org_id,
    ) {
        Ok(id) => {
            let task = board.get_coordination_task(id).unwrap();
            (
                StatusCode::CREATED,
                Json(CoordinationTaskResponse::from(task)),
            )
                .into_response()
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

pub async fn list_coordination_tasks(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let board = state.board.lock().await;
    let tasks: Vec<CoordinationTaskResponse> = board
        .list_coordination_tasks()
        .into_iter()
        .map(CoordinationTaskResponse::from)
        .collect();
    Json(tasks).into_response()
}

pub async fn get_coordination_task(
    State(state): State<AppState>,
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

    let board = state.board.lock().await;
    match board.get_coordination_task(uuid) {
        Some(task) => Json(CoordinationTaskResponse::from(task)).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "coordination task not found".into(),
            }),
        )
            .into_response(),
    }
}

pub async fn join_coordination_task(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<JoinCoordinationTaskRequest>,
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

    if body.agent_id.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "agent_id is required".into(),
            }),
        )
            .into_response();
    }

    // Check if the task is org-scoped and validate membership
    let org_check = {
        let board = state.board.lock().await;
        let task = board.get_coordination_task(uuid);
        match task {
            Some(t) => t.org_id.clone(),
            None => None,
        }
    };

    if let Some(ref org_id) = org_check {
        if let Some(ref store) = state.org_store {
            let orgs = store.lock().await;
            if let Some(org) = orgs.get(org_id) {
                let is_member = org.members.iter().any(|m| m.agent_id == body.agent_id);
                if !is_member {
                    return (
                        StatusCode::FORBIDDEN,
                        Json(ErrorResponse {
                            error: format!(
                                "agent {} is not a member of organization {}",
                                body.agent_id, org_id
                            ),
                        }),
                    )
                        .into_response();
                }
            }
        }
    }

    let mut board = state.board.lock().await;
    match board.join_coordination_task(uuid, body.agent_id.clone(), |_a, _o| true) {
        Ok(()) => {
            let task = board.get_coordination_task(uuid).unwrap();
            Json(CoordinationTaskResponse::from(task)).into_response()
        }
        Err(e) => {
            let status = match &e {
                CoordinationTaskError::InvalidTransition { .. } => StatusCode::CONFLICT,
                CoordinationTaskError::NotFound(_) => StatusCode::NOT_FOUND,
                CoordinationTaskError::AlreadyJoined => StatusCode::CONFLICT,
                CoordinationTaskError::TaskFull => StatusCode::CONFLICT,
                CoordinationTaskError::NotOrgMember { .. } => StatusCode::FORBIDDEN,
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

pub async fn submit_contribution(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<SubmitContributionRequest>,
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

    if body.agent_id.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "agent_id is required".into(),
            }),
        )
            .into_response();
    }

    if body.content.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "content is required".into(),
            }),
        )
            .into_response();
    }

    let tick = *state.tick_rx.borrow();
    let mut board = state.board.lock().await;
    match board.submit_coordination_contribution(uuid, &body.agent_id, body.content, tick) {
        Ok(()) => {
            let task = board.get_coordination_task(uuid).unwrap();
            Json(CoordinationTaskResponse::from(task)).into_response()
        }
        Err(e) => {
            let status = match &e {
                CoordinationTaskError::InvalidTransition { .. } => StatusCode::CONFLICT,
                CoordinationTaskError::NotFound(_) => StatusCode::NOT_FOUND,
                CoordinationTaskError::NotParticipant => StatusCode::FORBIDDEN,
                CoordinationTaskError::AlreadySubmitted => StatusCode::CONFLICT,
                CoordinationTaskError::ContributionRequired => StatusCode::BAD_REQUEST,
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

pub async fn complete_coordination_task(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<CompleteCoordinationTaskRequest>,
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

    let mut board = state.board.lock().await;
    match board.complete_coordination_task(uuid, &body.reviewer_id, body.reward_overrides) {
        Ok(distribution) => (
            StatusCode::OK,
            Json(RewardDistributionResponse {
                task_id: id,
                distribution,
            }),
        )
            .into_response(),
        Err(e) => {
            let status = match &e {
                CoordinationTaskError::InvalidTransition { .. } => StatusCode::CONFLICT,
                CoordinationTaskError::NotFound(_) => StatusCode::NOT_FOUND,
                CoordinationTaskError::NotCoordinator { .. } => StatusCode::FORBIDDEN,
                CoordinationTaskError::NoParticipants => StatusCode::BAD_REQUEST,
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

pub async fn cancel_coordination_task(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<CancelCoordinationTaskRequest>,
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

    let mut board = state.board.lock().await;
    match board.cancel_coordination_task(uuid, &body.coordinator_id) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => {
            let status = match &e {
                CoordinationTaskError::InvalidTransition { .. } => StatusCode::CONFLICT,
                CoordinationTaskError::NotFound(_) => StatusCode::NOT_FOUND,
                CoordinationTaskError::NotCoordinator { .. } => StatusCode::FORBIDDEN,
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

/// Coordination task routes for merging into the full router.
pub fn coordination_task_routes() -> axum::Router<AppState> {
    use axum::routing::*;
    axum::Router::new()
        .route("/coordination-tasks", post(create_coordination_task))
        .route("/coordination-tasks", get(list_coordination_tasks))
        .route("/coordination-tasks/:id", get(get_coordination_task))
        .route("/coordination-tasks/:id/join", post(join_coordination_task))
        .route(
            "/coordination-tasks/:id/contribute",
            post(submit_contribution),
        )
        .route(
            "/coordination-tasks/:id/complete",
            post(complete_coordination_task),
        )
        .route(
            "/coordination-tasks/:id/cancel",
            post(cancel_coordination_task),
        )
}
