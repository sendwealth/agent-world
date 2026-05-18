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
use futures::stream::Stream;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::economy::task::{Task, TaskBoard};
use crate::wal::WAL;
use crate::world::event::WorldEvent;
use crate::world::state::EventBus;
use crate::world::AgentRegistry;

// ── Shared State ──────────────────────────────────────────

pub type SharedTaskBoard = Arc<Mutex<TaskBoard>>;
pub type SharedWAL = Arc<Mutex<WAL>>;
pub type SharedAgentRegistry = Arc<Mutex<AgentRegistry>>;
pub type SharedEventBus = Arc<EventBus>;

/// Combined state for the API with WAL and agent support.
#[derive(Clone)]
pub struct AppState {
    pub board: SharedTaskBoard,
    pub wal: SharedWAL,
    pub agents: SharedAgentRegistry,
    pub event_bus: SharedEventBus,
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

pub fn create_router_with_wal(board: SharedTaskBoard, wal: SharedWAL) -> Router {
    let event_bus = Arc::new(EventBus::new(256));
    let agents = Arc::new(Mutex::new(AgentRegistry::with_event_bus(
        (*event_bus).clone(),
    )));
    create_router_full(board, wal, agents, event_bus)
}

pub fn create_router_full(
    board: SharedTaskBoard,
    wal: SharedWAL,
    agents: SharedAgentRegistry,
    event_bus: SharedEventBus,
) -> Router {
    let state = AppState {
        board,
        wal,
        agents,
        event_bus,
    };
    Router::new()
        // Task routes
        .route("/tasks", post(create_task_full))
        .route("/tasks", get(list_tasks_full))
        .route("/tasks/:id", get(get_task_full))
        .route("/tasks/:id/claim", post(claim_task_full))
        .route("/tasks/:id/start", post(start_task_full))
        .route("/tasks/:id/submit", post(submit_task_full))
        .route("/tasks/:id/review", post(review_task_full))
        .route("/tasks/:id/complete", post(complete_task_full))
        .route("/tasks/:id/expire", post(expire_task_full))
        .route("/tasks/:id", delete(delete_task_full))
        // Agent routes
        .route("/agents", get(list_agents))
        .route("/agents", post(create_agent))
        .route("/agents/:id", get(get_agent))
        // World routes
        .route("/world/stats", get(world_stats))
        .route("/world/events", get(world_events_handler))
        .route("/world/leaderboard", get(world_leaderboard))
        // WAL routes
        .route("/wal/stats", get(wal_stats))
        .route("/wal/snapshot", post(wal_snapshot))
        .route("/wal/verify", get(wal_verify))
        .with_state(state)
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

#[derive(Debug, Deserialize)]
pub struct CreateAgentRequest {
    pub name: String,
    #[serde(default = "default_tokens")]
    pub initial_tokens: u64,
    #[serde(default = "default_money")]
    pub initial_money: u64,
}

fn default_tokens() -> u64 {
    100
}
fn default_money() -> u64 {
    1000
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
#[serde(rename_all = "camelCase")]
pub struct AgentResponse {
    pub id: String,
    pub name: String,
    pub phase: String,
    pub money: u64,
    pub tokens: u64,
    pub reputation: f64,
    pub skills: std::collections::HashMap<String, u64>,
    pub alive: bool,
    pub age: u64,
    pub created_at: String,
}

impl From<&crate::world::Agent> for AgentResponse {
    fn from(agent: &crate::world::Agent) -> Self {
        AgentResponse {
            id: agent.id.clone(),
            name: agent.name.clone(),
            phase: format!("{:?}", agent.phase).to_lowercase(),
            money: agent.money,
            tokens: agent.tokens,
            reputation: agent.reputation,
            skills: agent.skills.clone(),
            alive: agent.alive,
            age: agent.age,
            created_at: agent.created_at.clone(),
        }
    }
}

/// World stats response matching the frontend TypeScript type.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldStatsResponse {
    pub agent_count: usize,
    pub alive_count: usize,
    pub dead_count: usize,
    pub gdp: u64,
    pub inflation_rate: f64,
    pub total_money: u64,
    pub tick: u64,
}

/// Leaderboard entry matching the frontend TypeScript type.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LeaderboardEntry {
    pub agent_id: String,
    pub agent_name: String,
    pub value: f64,
    pub rank: usize,
}

/// Leaderboard response matching the frontend TypeScript type.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LeaderboardResponse {
    pub richest: Vec<LeaderboardEntry>,
    pub longest_lived: Vec<LeaderboardEntry>,
    pub highest_skill: Vec<LeaderboardEntry>,
    pub highest_reputation: Vec<LeaderboardEntry>,
}

/// Frontend-facing event representation matching the TypeScript WorldEvent type.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontendEvent {
    pub id: String,
    #[serde(rename = "type")]
    pub event_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_name: Option<String>,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<f64>,
    pub timestamp: String,
    pub tick: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

// ── WorldEvent -> FrontendEvent conversion ────────────────

impl WorldEvent {
    fn to_frontend_event(&self, _agents: &SharedAgentRegistry) -> FrontendEvent {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        match self {
            WorldEvent::TickAdvanced { tick } => FrontendEvent {
                id,
                event_type: "inflation".to_string(),
                agent_id: None,
                agent_name: None,
                target_id: None,
                target_name: None,
                description: format!("Tick #{} advanced", tick),
                amount: None,
                timestamp: now,
                tick: *tick,
                data: None,
            },
            WorldEvent::AgentSpawned { agent_id, name } => FrontendEvent {
                id,
                event_type: "agent_spawn".to_string(),
                agent_id: Some(agent_id.clone()),
                agent_name: Some(name.clone()),
                target_id: None,
                target_name: None,
                description: format!("Agent {} was spawned", name),
                amount: None,
                timestamp: now,
                tick: 0,
                data: None,
            },
            WorldEvent::AgentDied { agent_id, reason } => FrontendEvent {
                id,
                event_type: "agent_death".to_string(),
                agent_id: Some(agent_id.clone()),
                agent_name: None,
                target_id: None,
                target_name: None,
                description: format!("Agent died: {:?}", reason),
                amount: None,
                timestamp: now,
                tick: 0,
                data: Some(serde_json::json!({ "reason": format!("{:?}", reason) })),
            },
            WorldEvent::AgentDying { agent_id, reason, grace_ticks } => FrontendEvent {
                id,
                event_type: "agent_death".to_string(),
                agent_id: Some(agent_id.clone()),
                agent_name: None,
                target_id: None,
                target_name: None,
                description: format!("Agent is dying: {:?} (grace: {} ticks)", reason, grace_ticks),
                amount: None,
                timestamp: now,
                tick: 0,
                data: Some(serde_json::json!({ "reason": format!("{:?}", reason), "grace_ticks": grace_ticks })),
            },
            WorldEvent::AgentRescued { agent_id } => FrontendEvent {
                id,
                event_type: "trade".to_string(),
                agent_id: Some(agent_id.clone()),
                agent_name: None,
                target_id: None,
                target_name: None,
                description: "Agent was rescued".to_string(),
                amount: None,
                timestamp: now,
                tick: 0,
                data: None,
            },
            WorldEvent::TransactionCompleted { from, to, amount, currency } => FrontendEvent {
                id,
                event_type: "trade".to_string(),
                agent_id: Some(from.clone()),
                agent_name: None,
                target_id: Some(to.clone()),
                target_name: None,
                description: format!("Transaction: {} {:?} from {} to {}", amount, currency, from, to),
                amount: Some(*amount as f64),
                timestamp: now,
                tick: 0,
                data: Some(serde_json::json!({ "currency": format!("{:?}", currency) })),
            },
            WorldEvent::BalanceChanged { agent_id, currency, old_balance, new_balance } => FrontendEvent {
                id,
                event_type: "trade".to_string(),
                agent_id: Some(agent_id.clone()),
                agent_name: None,
                target_id: None,
                target_name: None,
                description: format!("Balance changed: {} {:?} -> {} ({:?})", old_balance, currency, new_balance, currency),
                amount: Some((*new_balance as f64) - (*old_balance as f64)),
                timestamp: now,
                tick: 0,
                data: Some(serde_json::json!({ "currency": format!("{:?}", currency), "old_balance": old_balance, "new_balance": new_balance })),
            },
            WorldEvent::PhaseChanged { agent_id, old_phase, new_phase } => FrontendEvent {
                id,
                event_type: "skill_up".to_string(),
                agent_id: Some(agent_id.clone()),
                agent_name: None,
                target_id: None,
                target_name: None,
                description: format!("Phase changed: {:?} -> {:?}", old_phase, new_phase),
                amount: None,
                timestamp: now,
                tick: 0,
                data: None,
            },
            WorldEvent::RuleViolated { agent_id, rule, details } => FrontendEvent {
                id,
                event_type: "message".to_string(),
                agent_id: Some(agent_id.clone()),
                agent_name: None,
                target_id: None,
                target_name: None,
                description: format!("Rule violated: {} - {}", rule, details),
                amount: None,
                timestamp: now,
                tick: 0,
                data: None,
            },
            WorldEvent::SnapshotTaken { tick, path } => FrontendEvent {
                id,
                event_type: "message".to_string(),
                agent_id: None,
                agent_name: None,
                target_id: None,
                target_name: None,
                description: format!("Snapshot taken at tick {}: {}", tick, path),
                amount: None,
                timestamp: now,
                tick: *tick,
                data: None,
            },
            WorldEvent::EscrowCreated { escrow_id, publisher, reward, currency } => FrontendEvent {
                id,
                event_type: "investment".to_string(),
                agent_id: Some(publisher.clone()),
                agent_name: None,
                target_id: None,
                target_name: None,
                description: format!("Escrow created: {} {:?} for {}", reward, currency, publisher),
                amount: Some(*reward as f64),
                timestamp: now,
                tick: 0,
                data: Some(serde_json::json!({ "escrow_id": escrow_id, "currency": format!("{:?}", currency) })),
            },
            WorldEvent::EscrowClaimed { escrow_id, claimant, deposit } => FrontendEvent {
                id,
                event_type: "trade".to_string(),
                agent_id: Some(claimant.clone()),
                agent_name: None,
                target_id: None,
                target_name: None,
                description: format!("Escrow claimed by {} (deposit: {})", claimant, deposit),
                amount: Some(*deposit as f64),
                timestamp: now,
                tick: 0,
                data: Some(serde_json::json!({ "escrow_id": escrow_id })),
            },
            WorldEvent::EscrowReleased { escrow_id, recipient, amount, currency } => FrontendEvent {
                id,
                event_type: "trade".to_string(),
                agent_id: Some(recipient.clone()),
                agent_name: None,
                target_id: None,
                target_name: None,
                description: format!("Escrow released: {} {:?} to {}", amount, currency, recipient),
                amount: Some(*amount as f64),
                timestamp: now,
                tick: 0,
                data: Some(serde_json::json!({ "escrow_id": escrow_id, "currency": format!("{:?}", currency) })),
            },
            WorldEvent::EscrowRefunded { escrow_id, recipient, amount, currency } => FrontendEvent {
                id,
                event_type: "trade".to_string(),
                agent_id: Some(recipient.clone()),
                agent_name: None,
                target_id: None,
                target_name: None,
                description: format!("Escrow refunded: {} {:?} to {}", amount, currency, recipient),
                amount: Some(*amount as f64),
                timestamp: now,
                tick: 0,
                data: Some(serde_json::json!({ "escrow_id": escrow_id, "currency": format!("{:?}", currency) })),
            },
            WorldEvent::EscrowFrozen { escrow_id, reason } => FrontendEvent {
                id,
                event_type: "message".to_string(),
                agent_id: None,
                agent_name: None,
                target_id: None,
                target_name: None,
                description: format!("Escrow frozen: {} ({})", escrow_id, reason),
                amount: None,
                timestamp: now,
                tick: 0,
                data: Some(serde_json::json!({ "escrow_id": escrow_id })),
            },
            WorldEvent::TaskCreated { task_id, publisher, reward } => FrontendEvent {
                id,
                event_type: "task_created".to_string(),
                agent_id: Some(publisher.clone()),
                agent_name: None,
                target_id: Some(task_id.clone()),
                target_name: None,
                description: format!("Task created by {} with reward {}", publisher, reward),
                amount: Some(*reward as f64),
                timestamp: now,
                tick: 0,
                data: Some(serde_json::json!({ "task_id": task_id })),
            },
            WorldEvent::TaskClaimed { task_id, assignee } => FrontendEvent {
                id,
                event_type: "task_claimed".to_string(),
                agent_id: Some(assignee.clone()),
                agent_name: None,
                target_id: Some(task_id.clone()),
                target_name: None,
                description: format!("Task {} claimed by {}", task_id, assignee),
                amount: None,
                timestamp: now,
                tick: 0,
                data: Some(serde_json::json!({ "task_id": task_id })),
            },
            WorldEvent::TaskStarted { task_id } => FrontendEvent {
                id,
                event_type: "task_claimed".to_string(),
                agent_id: None,
                agent_name: None,
                target_id: Some(task_id.clone()),
                target_name: None,
                description: format!("Task {} started", task_id),
                amount: None,
                timestamp: now,
                tick: 0,
                data: Some(serde_json::json!({ "task_id": task_id })),
            },
            WorldEvent::TaskSubmitted { task_id } => FrontendEvent {
                id,
                event_type: "task_completed".to_string(),
                agent_id: None,
                agent_name: None,
                target_id: Some(task_id.clone()),
                target_name: None,
                description: format!("Task {} submitted", task_id),
                amount: None,
                timestamp: now,
                tick: 0,
                data: Some(serde_json::json!({ "task_id": task_id })),
            },
            WorldEvent::TaskReviewed { task_id, approved } => FrontendEvent {
                id,
                event_type: "task_completed".to_string(),
                agent_id: None,
                agent_name: None,
                target_id: Some(task_id.clone()),
                target_name: None,
                description: format!("Task {} reviewed: {}", task_id, if *approved { "approved" } else { "rejected" }),
                amount: None,
                timestamp: now,
                tick: 0,
                data: Some(serde_json::json!({ "task_id": task_id, "approved": approved })),
            },
            WorldEvent::TaskCompleted { task_id } => FrontendEvent {
                id,
                event_type: "task_completed".to_string(),
                agent_id: None,
                agent_name: None,
                target_id: Some(task_id.clone()),
                target_name: None,
                description: format!("Task {} completed", task_id),
                amount: None,
                timestamp: now,
                tick: 0,
                data: Some(serde_json::json!({ "task_id": task_id })),
            },
            WorldEvent::TaskExpired { task_id } => FrontendEvent {
                id,
                event_type: "message".to_string(),
                agent_id: None,
                agent_name: None,
                target_id: Some(task_id.clone()),
                target_name: None,
                description: format!("Task {} expired", task_id),
                amount: None,
                timestamp: now,
                tick: 0,
                data: Some(serde_json::json!({ "task_id": task_id })),
            },
            WorldEvent::RewardDistributed {
                task_id,
                assignee_id,
                gross_reward,
                net_reward,
                platform_fee,
                xp_awarded,
                reputation_change,
            } => FrontendEvent {
                id,
                event_type: "reputation_change".to_string(),
                agent_id: Some(assignee_id.clone()),
                agent_name: None,
                target_id: Some(task_id.clone()),
                target_name: None,
                description: format!(
                    "Reward: {} (fee: {}, xp: {}, rep: +{:.1})",
                    net_reward, platform_fee, xp_awarded, reputation_change
                ),
                amount: Some(*net_reward as f64),
                timestamp: now,
                tick: 0,
                data: Some(serde_json::json!({
                    "task_id": task_id,
                    "gross_reward": gross_reward,
                    "platform_fee": platform_fee,
                    "xp_awarded": xp_awarded,
                    "reputation_change": reputation_change,
                })),
            },
        }
    }
}

/// Resolve agent names in a FrontendEvent using the agent registry.
async fn resolve_event_names(mut event: FrontendEvent, agents: &SharedAgentRegistry) -> FrontendEvent {
    let registry = agents.lock().await;
    if let Some(ref aid) = event.agent_id {
        if event.agent_name.is_none() {
            if let Some(agent) = registry.get(aid) {
                event.agent_name = Some(agent.name.clone());
            }
        }
    }
    if let Some(ref tid) = event.target_id {
        if event.target_name.is_none() {
            if let Some(agent) = registry.get(tid) {
                event.target_name = Some(agent.name.clone());
            }
        }
    }
    event
}

// ── Task Handlers (original, for backwards compat) ────────

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
        0,
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

// ── Task Handlers (with full AppState) ────────────────────

async fn create_task_full(
    State(state): State<AppState>,
    Json(body): Json<CreateTaskRequest>,
) -> impl IntoResponse {
    create_task(State(state.board), Json(body)).await
}

async fn list_tasks_full(
    State(state): State<AppState>,
) -> impl IntoResponse {
    list_tasks(State(state.board)).await
}

async fn get_task_full(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    get_task(State(state.board), Path(id)).await
}

async fn claim_task_full(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<ClaimTaskRequest>,
) -> impl IntoResponse {
    claim_task(State(state.board), Path(id), Json(body)).await
}

async fn start_task_full(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    start_task(State(state.board), Path(id)).await
}

async fn submit_task_full(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<SubmitTaskRequest>,
) -> impl IntoResponse {
    submit_task(State(state.board), Path(id), Json(body)).await
}

async fn review_task_full(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<ReviewTaskRequest>,
) -> impl IntoResponse {
    review_task(State(state.board), Path(id), Json(body)).await
}

async fn complete_task_full(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    complete_task(State(state.board), Path(id)).await
}

async fn expire_task_full(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    expire_task(State(state.board), Path(id)).await
}

async fn delete_task_full(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    delete_task(State(state.board), Path(id)).await
}

// ── Agent Handlers ────────────────────────────────────────

async fn list_agents(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let registry = state.agents.lock().await;
    let agents: Vec<AgentResponse> = registry.list().into_iter().map(AgentResponse::from).collect();
    Json(agents).into_response()
}

async fn get_agent(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let registry = state.agents.lock().await;
    match registry.get(&id) {
        Some(agent) => Json(AgentResponse::from(agent)).into_response(),
        None => (StatusCode::NOT_FOUND, Json(ErrorResponse { error: "agent not found".into() })).into_response(),
    }
}

async fn create_agent(
    State(state): State<AppState>,
    Json(body): Json<CreateAgentRequest>,
) -> impl IntoResponse {
    if body.name.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "name is required".into() })).into_response();
    }

    let mut registry = state.agents.lock().await;
    let id = registry.spawn_agent(body.name, body.initial_tokens, body.initial_money);
    let agent = registry.get(&id).unwrap();
    (StatusCode::CREATED, Json(AgentResponse::from(agent))).into_response()
}

// ── World Stats Handler ──────────────────────────────────

async fn world_stats(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let registry = state.agents.lock().await;
    let stats = WorldStatsResponse {
        agent_count: registry.count(),
        alive_count: registry.alive_count(),
        dead_count: registry.dead_count(),
        gdp: registry.total_money(),
        inflation_rate: 0.0,
        total_money: registry.total_money(),
        tick: registry.tick(),
    };
    Json(stats).into_response()
}

// ── World Leaderboard Handler ─────────────────────────────

async fn world_leaderboard(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let registry = state.agents.lock().await;
    let agents = registry.list();

    // Richest by money
    let mut richest: Vec<_> = agents.iter().filter(|a| a.alive).collect();
    richest.sort_by(|a, b| b.money.cmp(&a.money));
    let richest: Vec<LeaderboardEntry> = richest.iter().take(10).enumerate().map(|(i, a)| LeaderboardEntry {
        agent_id: a.id.clone(),
        agent_name: a.name.clone(),
        value: a.money as f64,
        rank: i + 1,
    }).collect();

    // Longest lived by age
    let mut longest_lived: Vec<_> = agents.iter().collect();
    longest_lived.sort_by(|a, b| b.age.cmp(&a.age));
    let longest_lived: Vec<LeaderboardEntry> = longest_lived.iter().take(10).enumerate().map(|(i, a)| LeaderboardEntry {
        agent_id: a.id.clone(),
        agent_name: a.name.clone(),
        value: a.age as f64,
        rank: i + 1,
    }).collect();

    // Highest skill (sum of all skill levels)
    let mut highest_skill: Vec<_> = agents.iter().map(|a| {
        let total: u64 = a.skills.values().sum();
        (a, total)
    }).collect();
    highest_skill.sort_by(|a, b| b.1.cmp(&a.1));
    let highest_skill: Vec<LeaderboardEntry> = highest_skill.iter().take(10).enumerate().map(|(i, (a, total))| LeaderboardEntry {
        agent_id: a.id.clone(),
        agent_name: a.name.clone(),
        value: *total as f64,
        rank: i + 1,
    }).collect();

    // Highest reputation
    let mut highest_rep: Vec<_> = agents.iter().collect();
    highest_rep.sort_by(|a, b| b.reputation.partial_cmp(&a.reputation).unwrap_or(std::cmp::Ordering::Equal));
    let highest_reputation: Vec<LeaderboardEntry> = highest_rep.iter().take(10).enumerate().map(|(i, a)| LeaderboardEntry {
        agent_id: a.id.clone(),
        agent_name: a.name.clone(),
        value: a.reputation,
        rank: i + 1,
    }).collect();

    let response = LeaderboardResponse {
        richest,
        longest_lived,
        highest_skill,
        highest_reputation,
    };
    Json(response).into_response()
}

// ── World Events Handler (SSE) ────────────────────────────

async fn world_events_handler(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<SseEvent, Infallible>>> {
    let agents = state.agents.clone();
    let mut rx = state.event_bus.subscribe();

    let stream = async_stream::stream! {
        loop {
            match rx.recv().await {
                Ok(world_event) => {
                    let mut fe = world_event.to_frontend_event(&agents);
                    fe = resolve_event_names(fe, &agents).await;
                    if let Ok(json) = serde_json::to_string(&fe) {
                        yield Ok(SseEvent::default().data(json));
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    continue;
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    break;
                }
            }
        }
    };

    Sse::new(stream).keep_alive(KeepAlive::default())
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
