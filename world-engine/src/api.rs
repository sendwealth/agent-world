use std::convert::Infallible;
use std::sync::Arc;

use axum::{
    Json,
    Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{
        sse::{Event as SseEvent, Sse},
        IntoResponse,
    },
    routing::{delete, get, post},
};
use futures::stream::Stream;
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, watch};
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;
use uuid::Uuid;

use crate::economy::banking::{
    BankingSystem, BankAccountType, Loan, LoanStatus, Collateral,
};
use crate::economy::marketplace::Marketplace;
use crate::economy::reputation::ReputationSystem;
use crate::economy::task::{TaskBoard, Task};
use crate::time_capsule::SnapshotStore;
use crate::wal::WAL;
use crate::world::event::WorldEvent;
use crate::world::state::EventBus;

// ── Shared State ──────────────────────────────────────────

pub type SharedTaskBoard = Arc<Mutex<TaskBoard>>;
pub type SharedWAL = Arc<Mutex<WAL>>;
pub type SharedSnapshotStore = Arc<Mutex<SnapshotStore>>;
pub type SharedMarketplace = Arc<Mutex<Marketplace>>;
pub type SharedReputationSystem = Arc<Mutex<ReputationSystem>>;
pub type SharedBankingSystem = Arc<Mutex<BankingSystem>>;

/// Agent record tracked by the world engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRecord {
    pub id: String,
    pub name: String,
    pub phase: String,
    pub tokens: u64,
    pub money: u64,
    pub alive: bool,
    pub ticks_survived: u64,
}

/// A2A message record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2AMessage {
    pub id: String,
    pub from_agent: String,
    pub to_agent: String,
    pub message_type: String,
    pub payload: String,
    pub tick: u64,
}

/// Combined state for the API with WAL + EventBus + agents + messages + snapshot store.
#[derive(Clone)]
pub struct AppState {
    pub board: SharedTaskBoard,
    pub wal: SharedWAL,
    pub event_bus: Arc<EventBus>,
    pub agents: Arc<Mutex<Vec<AgentRecord>>>,
    pub messages: Arc<Mutex<Vec<A2AMessage>>>,
    pub tick_tx: watch::Sender<u64>,
    pub tick_rx: watch::Receiver<u64>,
    pub snapshot_store: Option<SharedSnapshotStore>,
    pub marketplace: Option<SharedMarketplace>,
    pub reputation_system: Option<SharedReputationSystem>,
    pub banking_system: Option<SharedBankingSystem>,
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
    let (tick_tx, tick_rx) = watch::channel(0u64);
    let snapshot_store = SnapshotStore::new("./data/snapshots.db")
        .ok()
        .map(|s| Arc::new(Mutex::new(s)));
    let state = AppState {
        board,
        wal,
        event_bus,
        agents: Arc::new(Mutex::new(Vec::new())),
        messages: Arc::new(Mutex::new(Vec::new())),
        tick_tx,
        tick_rx,
        snapshot_store,
        marketplace: None,
        reputation_system: None,
        banking_system: None,
    };
    build_full_router(state)
}

/// Create router with explicit snapshot store path.
pub fn create_router_with_wal_and_snapshots(board: SharedTaskBoard, wal: SharedWAL, snapshot_path: &str) -> Router {
    let event_bus = Arc::new(EventBus::new(256));
    let (tick_tx, tick_rx) = watch::channel(0u64);
    let snapshot_store = SnapshotStore::new(snapshot_path)
        .ok()
        .map(|s| Arc::new(Mutex::new(s)));
    let state = AppState {
        board,
        wal,
        event_bus,
        agents: Arc::new(Mutex::new(Vec::new())),
        messages: Arc::new(Mutex::new(Vec::new())),
        tick_tx,
        tick_rx,
        snapshot_store,
        marketplace: None,
        reputation_system: None,
        banking_system: None,
    };
    build_full_router(state)
}

/// Build the full router with all endpoints (tasks, WAL, world, agents, A2A).
pub fn build_full_router(state: AppState) -> Router {
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
        // World routes (SSE + stats)
        .route("/api/v1/world/events", get(world_events_sse))
        .route("/api/v1/world/stats", get(world_stats))
        // Agent routes
        .route("/api/v1/agents", get(list_agents))
        .route("/api/v1/agents", post(spawn_agent))
        .route("/api/v1/agents/:id", get(get_agent))
        // A2A routes
        .route("/api/v1/messages", post(send_message))
        .route("/api/v1/messages", get(list_messages))
        // Tick control
        .route("/api/v1/tick", post(advance_tick))
        .route("/api/v1/tick", get(get_tick))
        // Time Capsule / World Briefing
        .route("/api/v1/snapshots", get(list_snapshots))
        .route("/api/v1/snapshots/latest", get(get_latest_snapshot))
        .route("/api/v1/snapshots/:tick", get(get_snapshot_by_tick))
        .route("/api/v1/snapshots", post(create_snapshot))
        .route("/api/v1/snapshots/export/json", get(export_snapshots_json))
        .route("/api/v1/snapshots/export/csv", get(export_snapshots_csv))
        // Banking routes
        .route("/bank/accounts", post(bank_open_account))
        .route("/bank/accounts", get(bank_list_accounts))
        .route("/bank/accounts/:id", get(bank_get_account))
        .route("/bank/deposit", post(bank_deposit))
        .route("/bank/withdraw", post(bank_withdraw))
        .route("/bank/loans", post(bank_apply_loan))
        .route("/bank/loans", get(bank_list_loans))
        .route("/bank/loans/:id", get(bank_get_loan))
        .route("/bank/loans/:id/approve", post(bank_approve_loan))
        .route("/bank/loans/:id/disburse", post(bank_disburse_loan))
        .route("/bank/loans/:id/repay", post(bank_repay_loan))
        .route("/bank/central-bank/rates", post(bank_adjust_rates))
        .route("/bank/central-bank/mint", post(bank_mint_money))
        .route("/bank/central-bank/write-off/:id", post(bank_write_off))
        .route("/bank/stats", get(bank_stats))
        .with_state(state)
}

/// Create a router for testing with a provided EventBus and tick channel.
pub fn create_router_for_test(
    board: SharedTaskBoard,
    wal: SharedWAL,
    event_bus: Arc<EventBus>,
    tick_tx: watch::Sender<u64>,
    tick_rx: watch::Receiver<u64>,
) -> Router {
    let snapshot_store = SnapshotStore::new_in_memory()
        .ok()
        .map(|s| Arc::new(Mutex::new(s)));
    let state = AppState {
        board,
        wal,
        event_bus,
        agents: Arc::new(Mutex::new(Vec::new())),
        messages: Arc::new(Mutex::new(Vec::new())),
        tick_tx,
        tick_rx,
        snapshot_store,
        marketplace: None,
        reputation_system: None,
        banking_system: None,
    };
    build_full_router(state)
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
pub struct SpawnAgentRequest {
    pub name: String,
    #[serde(default = "default_tokens")]
    pub tokens: u64,
    #[serde(default)]
    pub money: u64,
}

fn default_tokens() -> u64 {
    100_000
}

#[derive(Debug, Deserialize)]
pub struct SendMessageRequest {
    pub from_agent: String,
    pub to_agent: String,
    pub message_type: String,
    pub payload: String,
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

#[derive(Debug, Serialize)]
pub struct WorldStatsResponse {
    pub agent_count: usize,
    pub alive_count: usize,
    pub dead_count: usize,
    pub total_money: u64,
    pub total_tokens: u64,
    pub tick: u64,
    pub task_count: usize,
}

// ── Task Handlers (no WAL) ────────────────────────────────

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

// ── World SSE Handler ─────────────────────────────────────

/// Query parameters for the SSE endpoint.
#[derive(Debug, Deserialize, Default)]
pub struct SseQuery {
    /// Comma-separated event type filter (e.g., "agent_died,agent_rescued")
    pub types: Option<String>,
    /// Filter events for a specific agent ID
    pub agent_id: Option<String>,
}

/// Parse a comma-separated string of event type names into `EventType` values.
/// Returns an error message for any unrecognized names.
fn parse_event_types(raw: &str) -> Result<Vec<crate::world::event::EventType>, String> {
    use crate::world::event::EventType;
    let mut types = Vec::new();
    for name in raw.split(',') {
        let name = name.trim();
        if name.is_empty() {
            continue;
        }
        let et = match name {
            "tick_advanced" => EventType::TickAdvanced,
            "agent_spawned" => EventType::AgentSpawned,
            "agent_dying" => EventType::AgentDying,
            "agent_died" => EventType::AgentDied,
            "agent_rescued" => EventType::AgentRescued,
            "transaction_completed" => EventType::TransactionCompleted,
            "balance_changed" => EventType::BalanceChanged,
            "phase_changed" => EventType::PhaseChanged,
            "rule_violated" => EventType::RuleViolated,
            "snapshot_taken" => EventType::SnapshotTaken,
            "escrow_created" => EventType::EscrowCreated,
            "escrow_claimed" => EventType::EscrowClaimed,
            "escrow_released" => EventType::EscrowReleased,
            "escrow_refunded" => EventType::EscrowRefunded,
            "escrow_frozen" => EventType::EscrowFrozen,
            "task_created" => EventType::TaskCreated,
            "task_claimed" => EventType::TaskClaimed,
            "task_started" => EventType::TaskStarted,
            "task_submitted" => EventType::TaskSubmitted,
            "task_reviewed" => EventType::TaskReviewed,
            "task_completed" => EventType::TaskCompleted,
            "task_expired" => EventType::TaskExpired,
            "reward_distributed" => EventType::RewardDistributed,
            "agent_registered" => EventType::AgentRegistered,
            "agent_deregistered" => EventType::AgentDeregistered,
            "agent_heartbeat" => EventType::AgentHeartbeat,
            "reputation_changed" => EventType::ReputationChanged,
            "config_reloaded" => EventType::ConfigReloaded,
            other => return Err(format!("unknown event type: {}", other)),
        };
        types.push(et);
    }
    Ok(types)
}

async fn world_events_sse(
    State(state): State<AppState>,
    Query(query): Query<SseQuery>,
) -> Result<Sse<impl Stream<Item = Result<SseEvent, Infallible>>>, (StatusCode, Json<ErrorResponse>)> {
    // Parse type filter if provided
    let type_filter: Option<std::collections::HashSet<crate::world::event::EventType>> = if let Some(ref types_str) = query.types {
        let parsed = parse_event_types(types_str).map_err(|e| {
            (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: e }))
        })?;
        if parsed.is_empty() {
            None
        } else {
            Some(parsed.into_iter().collect())
        }
    } else {
        None
    };

    let agent_filter: Option<String> = query.agent_id.filter(|s| !s.is_empty());

    let rx = state.event_bus.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(move |result| {
        let type_filter = type_filter.clone();
        let agent_filter = agent_filter.clone();
        match result {
            Ok(event) => {
                // Apply type filter
                if let Some(ref types) = type_filter {
                    if !types.contains(&event.event_type()) {
                        return None;
                    }
                }
                // Apply agent_id filter
                if let Some(ref filter_id) = agent_filter {
                    match event.agent_id() {
                        Some(aid) if aid == filter_id => {}
                        _ => return None,
                    }
                }
                let data = event.to_json();
                Some(Ok(SseEvent::default().data(data)))
            }
            Err(_) => None, // Skip lagged/closed errors
        }
    });

    Ok(Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(std::time::Duration::from_secs(15))
            .text("ping"),
    ))
}

// ── World Stats Handler ───────────────────────────────────

async fn world_stats(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let agents = state.agents.lock().await;
    let board = state.board.lock().await;
    let tick = *state.tick_rx.borrow();

    let alive_count = agents.iter().filter(|a| a.alive).count();
    let dead_count = agents.iter().filter(|a| !a.alive).count();
    let total_tokens: u64 = agents.iter().map(|a| a.tokens).sum();
    let total_money: u64 = agents.iter().map(|a| a.money).sum();
    let task_count = board.list().len();

    Json(WorldStatsResponse {
        agent_count: agents.len(),
        alive_count,
        dead_count,
        total_money,
        total_tokens,
        tick,
        task_count,
    })
}

// ── Agent Handlers ────────────────────────────────────────

async fn spawn_agent(
    State(state): State<AppState>,
    Json(body): Json<SpawnAgentRequest>,
) -> impl IntoResponse {
    let agent = AgentRecord {
        id: Uuid::new_v4().to_string(),
        name: body.name.clone(),
        phase: "adult".to_string(),
        tokens: body.tokens,
        money: body.money,
        alive: true,
        ticks_survived: 0,
    };

    state.event_bus.emit(WorldEvent::AgentSpawned {
        agent_id: agent.id.clone(),
        name: agent.name.clone(),
    });

    let mut agents = state.agents.lock().await;
    agents.push(agent.clone());

    (StatusCode::CREATED, Json(agent)).into_response()
}

async fn list_agents(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let agents = state.agents.lock().await;
    Json(&*agents).into_response()
}

async fn get_agent(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let agents = state.agents.lock().await;
    match agents.iter().find(|a| a.id == id) {
        Some(agent) => Json(agent.clone()).into_response(),
        None => (StatusCode::NOT_FOUND, Json(ErrorResponse { error: "agent not found".into() })).into_response(),
    }
}

// ── A2A Message Handlers ──────────────────────────────────

async fn send_message(
    State(state): State<AppState>,
    Json(body): Json<SendMessageRequest>,
) -> impl IntoResponse {
    let tick = *state.tick_rx.borrow();

    let msg = A2AMessage {
        id: Uuid::new_v4().to_string(),
        from_agent: body.from_agent.clone(),
        to_agent: body.to_agent.clone(),
        message_type: body.message_type.clone(),
        payload: body.payload.clone(),
        tick,
    };

    state.event_bus.emit(WorldEvent::TransactionCompleted {
        from: msg.from_agent.clone(),
        to: msg.to_agent.clone(),
        amount: 0,
        currency: crate::world::enums::Currency::Token,
    });

    let mut messages = state.messages.lock().await;
    messages.push(msg.clone());

    (StatusCode::CREATED, Json(msg)).into_response()
}

async fn list_messages(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let messages = state.messages.lock().await;
    Json(&*messages).into_response()
}

// ── Tick Handlers ─────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct AdvanceTickRequest {
    #[serde(default = "default_count")]
    pub count: u64,
}

fn default_count() -> u64 {
    1
}

async fn advance_tick(
    State(state): State<AppState>,
    Json(body): Json<AdvanceTickRequest>,
) -> impl IntoResponse {
    let count = body.count.max(1);
    let current = *state.tick_rx.borrow();
    let new_tick = current + count;

    for t in (current + 1)..=new_tick {
        state.event_bus.emit(WorldEvent::TickAdvanced { tick: t });
    }

    let _ = state.tick_tx.send(new_tick);

    Json(serde_json::json!({
        "tick": new_tick,
        "advanced": count,
    }))
}

async fn get_tick(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let tick = *state.tick_rx.borrow();
    Json(serde_json::json!({ "tick": tick }))
}

// ── Time Capsule / Snapshot Handlers ──────────────────────

#[derive(Debug, Deserialize, Default)]
pub struct ListSnapshotsQuery {
    pub from_tick: Option<u64>,
    pub to_tick: Option<u64>,
    #[serde(default = "default_snapshot_limit")]
    pub limit: Option<u64>,
}

fn default_snapshot_limit() -> Option<u64> {
    Some(100)
}

async fn list_snapshots(
    State(state): State<AppState>,
    Query(query): Query<ListSnapshotsQuery>,
) -> impl IntoResponse {
    let store = match &state.snapshot_store {
        Some(s) => s.clone(),
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(ErrorResponse { error: "snapshot store not configured".into() })).into_response(),
    };

    let store = store.lock().await;
    match store.list(query.from_tick, query.to_tick, query.limit) {
        Ok(snapshots) => Json(&snapshots).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: e.to_string() })).into_response(),
    }
}

async fn get_latest_snapshot(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let store = match &state.snapshot_store {
        Some(s) => s.clone(),
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(ErrorResponse { error: "snapshot store not configured".into() })).into_response(),
    };

    let store = store.lock().await;
    match store.latest() {
        Ok(Some(snapshot)) => Json(&snapshot).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, Json(ErrorResponse { error: "no snapshots available".into() })).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: e.to_string() })).into_response(),
    }
}

async fn get_snapshot_by_tick(
    State(state): State<AppState>,
    Path(tick): Path<u64>,
) -> impl IntoResponse {
    let store = match &state.snapshot_store {
        Some(s) => s.clone(),
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(ErrorResponse { error: "snapshot store not configured".into() })).into_response(),
    };

    let store = store.lock().await;
    match store.get_by_tick(tick) {
        Ok(Some(snapshot)) => Json(&snapshot).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, Json(ErrorResponse { error: format!("no snapshot at tick {}", tick) })).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: e.to_string() })).into_response(),
    }
}

async fn create_snapshot(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let store = match &state.snapshot_store {
        Some(s) => s.clone(),
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(ErrorResponse { error: "snapshot store not configured".into() })).into_response(),
    };

    let tick = *state.tick_rx.borrow();
    let agents = state.agents.lock().await;

    let snapshot = crate::time_capsule::build_snapshot(tick, &agents.iter().map(|a| {
        let id = Uuid::parse_str(&a.id).unwrap_or_else(|_| Uuid::new_v4());
        let record = crate::economy::token_burn::AgentRecord {
            id,
            name: a.name.clone(),
            phase: if a.alive { crate::world::enums::AgentPhase::Adult } else { crate::world::enums::AgentPhase::Dead },
            tokens: a.tokens,
            skills: std::collections::HashMap::new(),
        };
        (id, a.ticks_survived, record)
    }).collect::<Vec<_>>(), &[]);

    let store = store.lock().await;
    match store.save(&snapshot) {
        Ok(id) => {
            state.event_bus.emit(WorldEvent::SnapshotTaken {
                tick,
                path: format!("snapshot:{}", id),
            });
            (StatusCode::CREATED, Json(&snapshot)).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: e.to_string() })).into_response(),
    }
}

async fn export_snapshots_json(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let store = match &state.snapshot_store {
        Some(s) => s.clone(),
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(ErrorResponse { error: "snapshot store not configured".into() })).into_response(),
    };

    let store = store.lock().await;
    match store.export_json() {
        Ok(json) => {
            let body = axum::body::Body::from(json);
            let mut resp = axum::response::Response::new(body);
            *resp.status_mut() = StatusCode::OK;
            resp.headers_mut().insert("content-type", "application/json".parse().unwrap());
            resp.headers_mut().insert("content-disposition", "attachment; filename=\"world_snapshots.json\"".parse().unwrap());
            resp
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: e.to_string() })).into_response(),
    }
}

async fn export_snapshots_csv(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let store = match &state.snapshot_store {
        Some(s) => s.clone(),
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(ErrorResponse { error: "snapshot store not configured".into() })).into_response(),
    };

    let store = store.lock().await;
    match store.export_csv() {
        Ok(csv) => {
            let body = axum::body::Body::from(csv);
            let mut resp = axum::response::Response::new(body);
            *resp.status_mut() = StatusCode::OK;
            resp.headers_mut().insert("content-type", "text/csv".parse().unwrap());
            resp.headers_mut().insert("content-disposition", "attachment; filename=\"world_snapshots.csv\"".parse().unwrap());
            resp
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: e.to_string() })).into_response(),
    }
}

// ── Banking Request/Response Types ───────────────────────

#[derive(Debug, Deserialize)]
pub struct BankOpenAccountRequest {
    pub owner_id: String,
    pub account_type: String, // "savings" or "checking"
    #[serde(default)]
    pub label: String,
}

#[derive(Debug, Deserialize)]
pub struct BankDepositRequest {
    pub account_id: String,
    pub owner_id: String,
    pub amount: u64,
}

#[derive(Debug, Deserialize)]
pub struct BankWithdrawRequest {
    pub account_id: String,
    pub owner_id: String,
    pub amount: u64,
}

#[derive(Debug, Deserialize)]
pub struct BankApplyLoanRequest {
    pub borrower_id: String,
    pub amount: u64,
    pub term_ticks: u64,
    pub collateral: Option<Collateral>,
}

#[derive(Debug, Deserialize)]
pub struct BankRepayRequest {
    pub amount: u64,
}

#[derive(Debug, Deserialize)]
pub struct BankAdjustRatesRequest {
    pub savings_rate: f64,
    pub loan_rate: f64,
}

#[derive(Debug, Deserialize)]
pub struct BankMintRequest {
    pub amount: u64,
}

#[derive(Debug, Deserialize, Default)]
pub struct BankListLoansQuery {
    pub borrower_id: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct BankAccountResponse {
    pub id: String,
    pub owner_id: String,
    pub account_type: String,
    pub label: String,
    pub balance: u64,
    pub created_tick: u64,
}

#[derive(Debug, Serialize)]
pub struct LoanResponse {
    pub id: String,
    pub borrower_id: String,
    pub principal: u64,
    pub outstanding_balance: u64,
    pub interest_rate: f64,
    pub term_ticks: u64,
    pub status: String,
    pub collateral: Option<Collateral>,
    pub created_tick: u64,
    pub disbursed_tick: Option<u64>,
    pub due_tick: Option<u64>,
    pub total_repaid: u64,
    pub ticks_overdue: u64,
}

impl From<&Loan> for LoanResponse {
    fn from(loan: &Loan) -> Self {
        LoanResponse {
            id: loan.id.to_string(),
            borrower_id: loan.borrower_id.clone(),
            principal: loan.principal,
            outstanding_balance: loan.outstanding_balance,
            interest_rate: loan.interest_rate,
            term_ticks: loan.term_ticks,
            status: format!("{:?}", loan.status).to_lowercase(),
            collateral: loan.collateral.clone(),
            created_tick: loan.created_tick,
            disbursed_tick: loan.disbursed_tick,
            due_tick: loan.due_tick,
            total_repaid: loan.total_repaid,
            ticks_overdue: loan.ticks_overdue,
        }
    }
}

fn parse_bank_account_type(s: &str) -> Option<BankAccountType> {
    match s {
        "savings" => Some(BankAccountType::Savings),
        "checking" => Some(BankAccountType::Checking),
        _ => None,
    }
}

fn parse_loan_status(s: &str) -> Option<LoanStatus> {
    match s {
        "pending" => Some(LoanStatus::Pending),
        "approved" => Some(LoanStatus::Approved),
        "active" => Some(LoanStatus::Active),
        "repaid" => Some(LoanStatus::Repaid),
        "defaulted" => Some(LoanStatus::Defaulted),
        "written_off" => Some(LoanStatus::WrittenOff),
        _ => None,
    }
}

// ── Banking Handlers ─────────────────────────────────────

fn get_banking(state: &AppState) -> Result<SharedBankingSystem, (StatusCode, Json<ErrorResponse>)> {
    state.banking_system.clone().ok_or_else(|| {
        (StatusCode::SERVICE_UNAVAILABLE, Json(ErrorResponse { error: "banking system not configured".into() }))
    })
}

async fn bank_open_account(
    State(state): State<AppState>,
    Json(body): Json<BankOpenAccountRequest>,
) -> impl IntoResponse {
    let banking = match get_banking(&state) {
        Ok(b) => b,
        Err(e) => return e.into_response(),
    };

    let account_type = match parse_bank_account_type(&body.account_type) {
        Some(t) => t,
        None => return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "account_type must be 'savings' or 'checking'".into() })).into_response(),
    };

    if body.owner_id.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "owner_id is required".into() })).into_response();
    }

    let label = if body.label.is_empty() {
        format!("{:?} {}", account_type, body.owner_id)
    } else {
        body.label.clone()
    };

    let tick = *state.tick_rx.borrow();
    let mut banking = banking.lock().await;
    match banking.open_account(&body.owner_id, account_type, &label, tick) {
        Ok(account) => {
            let balance = banking.get_balance(account.id).unwrap_or(0);
            let resp = BankAccountResponse {
                id: account.id.to_string(),
                owner_id: account.owner_id.clone(),
                account_type: format!("{:?}", account.account_type).to_lowercase(),
                label: account.label.clone(),
                balance,
                created_tick: account.created_tick,
            };
            (StatusCode::CREATED, Json(resp)).into_response()
        }
        Err(e) => (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: e.to_string() })).into_response(),
    }
}

async fn bank_list_accounts(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let banking = match get_banking(&state) {
        Ok(b) => b,
        Err(e) => return e.into_response(),
    };

    let banking = banking.lock().await;
    let accounts: Vec<BankAccountResponse> = banking.list_accounts().into_iter().map(|a| {
        let balance = banking.get_balance(a.id).unwrap_or(0);
        BankAccountResponse {
            id: a.id.to_string(),
            owner_id: a.owner_id.clone(),
            account_type: format!("{:?}", a.account_type).to_lowercase(),
            label: a.label.clone(),
            balance,
            created_tick: a.created_tick,
        }
    }).collect();
    Json(accounts).into_response()
}

async fn bank_get_account(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let banking = match get_banking(&state) {
        Ok(b) => b,
        Err(e) => return e.into_response(),
    };

    let Ok(uuid) = Uuid::parse_str(&id) else {
        return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "invalid account id".into() })).into_response();
    };

    let banking = banking.lock().await;
    match banking.get_account(uuid) {
        Some(account) => {
            let balance = banking.get_balance(account.id).unwrap_or(0);
            let resp = BankAccountResponse {
                id: account.id.to_string(),
                owner_id: account.owner_id.clone(),
                account_type: format!("{:?}", account.account_type).to_lowercase(),
                label: account.label.clone(),
                balance,
                created_tick: account.created_tick,
            };
            Json(resp).into_response()
        }
        None => (StatusCode::NOT_FOUND, Json(ErrorResponse { error: "account not found".into() })).into_response(),
    }
}

async fn bank_deposit(
    State(state): State<AppState>,
    Json(body): Json<BankDepositRequest>,
) -> impl IntoResponse {
    let banking = match get_banking(&state) {
        Ok(b) => b,
        Err(e) => return e.into_response(),
    };

    let Ok(uuid) = Uuid::parse_str(&body.account_id) else {
        return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "invalid account id".into() })).into_response();
    };

    let tick = *state.tick_rx.borrow();
    let mut banking = banking.lock().await;
    match banking.deposit(uuid, &body.owner_id, body.amount, tick) {
        Ok(result) => Json(serde_json::json!({
            "account_id": result.account_id,
            "amount": result.amount,
            "new_balance": result.new_balance,
        })).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: e.to_string() })).into_response(),
    }
}

async fn bank_withdraw(
    State(state): State<AppState>,
    Json(body): Json<BankWithdrawRequest>,
) -> impl IntoResponse {
    let banking = match get_banking(&state) {
        Ok(b) => b,
        Err(e) => return e.into_response(),
    };

    let Ok(uuid) = Uuid::parse_str(&body.account_id) else {
        return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "invalid account id".into() })).into_response()
    };

    let tick = *state.tick_rx.borrow();
    let mut banking = banking.lock().await;
    match banking.withdraw(uuid, &body.owner_id, body.amount, tick) {
        Ok(result) => Json(serde_json::json!({
            "account_id": result.account_id,
            "amount": result.amount,
            "new_balance": result.new_balance,
        })).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: e.to_string() })).into_response(),
    }
}

async fn bank_apply_loan(
    State(state): State<AppState>,
    Json(body): Json<BankApplyLoanRequest>,
) -> impl IntoResponse {
    let banking = match get_banking(&state) {
        Ok(b) => b,
        Err(e) => return e.into_response(),
    };

    if body.borrower_id.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "borrower_id is required".into() })).into_response();
    }

    let tick = *state.tick_rx.borrow();
    let mut banking = banking.lock().await;
    match banking.apply_for_loan(&body.borrower_id, body.amount, body.term_ticks, body.collateral, tick) {
        Ok(result) => (StatusCode::CREATED, Json(serde_json::json!({
            "loan_id": result.loan_id.to_string(),
            "borrower_id": result.borrower_id,
            "principal": result.principal,
            "interest_rate": result.interest_rate,
            "term_ticks": result.term_ticks,
            "status": format!("{:?}", result.status).to_lowercase(),
        }))).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: e.to_string() })).into_response(),
    }
}

async fn bank_list_loans(
    State(state): State<AppState>,
    Query(query): Query<BankListLoansQuery>,
) -> impl IntoResponse {
    let banking = match get_banking(&state) {
        Ok(b) => b,
        Err(e) => return e.into_response(),
    };

    let status_filter = query.status.as_deref().and_then(parse_loan_status);
    let banking = banking.lock().await;
    let loans: Vec<LoanResponse> = banking.list_loans(query.borrower_id.as_deref(), status_filter)
        .into_iter()
        .map(LoanResponse::from)
        .collect();
    Json(loans).into_response()
}

async fn bank_get_loan(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let banking = match get_banking(&state) {
        Ok(b) => b,
        Err(e) => return e.into_response(),
    };

    let Ok(uuid) = Uuid::parse_str(&id) else {
        return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "invalid loan id".into() })).into_response();
    };

    let banking = banking.lock().await;
    match banking.get_loan(uuid) {
        Some(loan) => Json(LoanResponse::from(loan)).into_response(),
        None => (StatusCode::NOT_FOUND, Json(ErrorResponse { error: "loan not found".into() })).into_response(),
    }
}

async fn bank_approve_loan(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let banking = match get_banking(&state) {
        Ok(b) => b,
        Err(e) => return e.into_response(),
    };

    let Ok(uuid) = Uuid::parse_str(&id) else {
        return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "invalid loan id".into() })).into_response()
    };

    let tick = *state.tick_rx.borrow();
    let mut banking = banking.lock().await;
    match banking.approve_loan(uuid, tick) {
        Ok(loan) => Json(LoanResponse::from(&loan)).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: e.to_string() })).into_response(),
    }
}

async fn bank_disburse_loan(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let banking = match get_banking(&state) {
        Ok(b) => b,
        Err(e) => return e.into_response(),
    };

    let Ok(uuid) = Uuid::parse_str(&id) else {
        return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "invalid loan id".into() })).into_response()
    };

    let tick = *state.tick_rx.borrow();
    let mut banking = banking.lock().await;
    match banking.disburse_loan(uuid, tick) {
        Ok(loan) => Json(LoanResponse::from(&loan)).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: e.to_string() })).into_response(),
    }
}

async fn bank_repay_loan(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<BankRepayRequest>,
) -> impl IntoResponse {
    let banking = match get_banking(&state) {
        Ok(b) => b,
        Err(e) => return e.into_response(),
    };

    let Ok(uuid) = Uuid::parse_str(&id) else {
        return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "invalid loan id".into() })).into_response()
    };

    let tick = *state.tick_rx.borrow();
    let mut banking = banking.lock().await;
    match banking.repay_loan(uuid, body.amount, tick) {
        Ok(result) => Json(serde_json::json!({
            "loan_id": result.loan_id.to_string(),
            "amount_paid": result.amount_paid,
            "outstanding_balance": result.outstanding_balance,
            "fully_repaid": result.fully_repaid,
        })).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: e.to_string() })).into_response(),
    }
}

async fn bank_adjust_rates(
    State(state): State<AppState>,
    Json(body): Json<BankAdjustRatesRequest>,
) -> impl IntoResponse {
    let banking = match get_banking(&state) {
        Ok(b) => b,
        Err(e) => return e.into_response(),
    };

    let mut banking = banking.lock().await;
    let result = banking.adjust_rates(body.savings_rate, body.loan_rate);
    Json(serde_json::json!({
        "new_savings_rate": result.new_savings_rate,
        "new_loan_rate": result.new_loan_rate,
    })).into_response()
}

async fn bank_mint_money(
    State(state): State<AppState>,
    Json(body): Json<BankMintRequest>,
) -> impl IntoResponse {
    let banking = match get_banking(&state) {
        Ok(b) => b,
        Err(e) => return e.into_response(),
    };

    let tick = *state.tick_rx.borrow();
    let mut banking = banking.lock().await;
    let result = banking.mint_money(body.amount, tick);
    (StatusCode::CREATED, Json(serde_json::json!({
        "amount": result.amount,
        "total_money_supply": result.total_money_supply,
    }))).into_response()
}

async fn bank_write_off(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let banking = match get_banking(&state) {
        Ok(b) => b,
        Err(e) => return e.into_response(),
    };

    let Ok(uuid) = Uuid::parse_str(&id) else {
        return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "invalid loan id".into() })).into_response()
    };

    let tick = *state.tick_rx.borrow();
    let mut banking = banking.lock().await;
    match banking.write_off_bad_debt(uuid, tick) {
        Ok(result) => Json(serde_json::json!({
            "loan_id": result.loan_id.to_string(),
            "amount_written_off": result.amount_written_off,
        })).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: e.to_string() })).into_response(),
    }
}

async fn bank_stats(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let banking = match get_banking(&state) {
        Ok(b) => b,
        Err(e) => return e.into_response(),
    };

    let banking = banking.lock().await;
    Json(serde_json::json!({
        "total_accounts": banking.list_accounts().len(),
        "total_loans": banking.list_loans(None, None).len(),
        "active_loans": banking.list_loans(None, Some(LoanStatus::Active)).len(),
        "defaulted_loans": banking.list_loans(None, Some(LoanStatus::Defaulted)).len(),
        "total_money_supply": banking.total_money_supply(),
        "total_loan_debt": banking.total_loan_debt(),
        "savings_rate": banking.config().savings_rate,
        "loan_rate": banking.config().loan_rate,
    })).into_response()
}
