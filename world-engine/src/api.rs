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

use crate::economy::marketplace::Marketplace;
use crate::economy::reputation::ReputationSystem;
use crate::economy::stock_market::{
    StockMarket, StockListing, Order as StockOrder,
    OrderType, OrderKind, ListingStatus,
};
use crate::economy::task::{TaskBoard, Task};
use crate::organization::org::{OrganizationStore, OrgType};
use crate::organization::charter::{Charter, GovernanceModel, ProfitSharing};
use crate::organization::governance::GovernanceSystem;
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
pub type SharedOrganizationStore = Arc<Mutex<OrganizationStore>>;
pub type SharedStockMarket = Arc<Mutex<StockMarket>>;
pub type SharedGovernanceSystem = Arc<Mutex<GovernanceSystem>>;

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
    pub org_store: Option<SharedOrganizationStore>,
    pub stock_market: Option<SharedStockMarket>,
    pub governance: Option<SharedGovernanceSystem>,
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
        org_store: None,
        stock_market: None,
        governance: None,
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
        org_store: None,
        stock_market: None,
        governance: None,
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
        // Organization routes
        .route("/api/v1/orgs", post(create_org))
        .route("/api/v1/orgs", get(list_orgs))
        .route("/api/v1/orgs/:id", get(get_org))
        .route("/api/v1/orgs/:id/join", post(join_org))
        .route("/api/v1/orgs/:id/leave", post(leave_org))
        .route("/api/v1/orgs/:id/dissolve", post(dissolve_org))
        .route("/api/v1/orgs/:id/distribution", post(calculate_distribution))
        .route("/api/v1/orgs/:id/proposals", post(create_proposal))
        .route("/api/v1/orgs/:id/proposals", get(list_proposals))
        .route("/api/v1/proposals/:id", get(get_proposal))
        .route("/api/v1/proposals/:id/vote", post(vote_proposal))
        .route("/api/v1/proposals/:id/start-voting", post(start_voting))
        .route("/api/v1/proposals/:id/tally", post(tally_proposal))
        .route("/api/v1/proposals/:id/cancel", post(cancel_proposal))
        // Stock Market routes
        .route("/api/v1/stocks", get(list_stocks))
        .route("/api/v1/stocks", post(issue_shares))
        .route("/api/v1/stocks/:id", get(get_stock))
        .route("/api/v1/stocks/:id/ipo", post(ipo_stock))
        .route("/api/v1/stocks/:id/dividend", post(distribute_dividend))
        .route("/api/v1/orders", get(list_stock_orders))
        .route("/api/v1/orders/buy", post(place_buy_order))
        .route("/api/v1/orders/sell", post(place_sell_order))
        .route("/api/v1/orders/:id", get(get_order))
        .route("/api/v1/orders/:id/cancel", post(cancel_order))
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
        org_store: None,
        stock_market: None,
        governance: None,
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
            "org_created" => EventType::OrgCreated,
            "org_member_joined" => EventType::OrgMemberJoined,
            "org_member_left" => EventType::OrgMemberLeft,
            "org_dissolved" => EventType::OrgDissolved,
            "org_inactivated" => EventType::OrgInactivated,
            "knowledge_listed" => EventType::KnowledgeListed,
            "knowledge_delisted" => EventType::KnowledgeDelisted,
            "knowledge_purchased" => EventType::KnowledgePurchased,
            "knowledge_rated" => EventType::KnowledgeRated,
            "stock_issued" => EventType::StockIssued,
            "stock_ipo" => EventType::StockIpo,
            "stock_traded" => EventType::StockTraded,
            "stock_transferred" => EventType::StockTransferred,
            "stock_dividend" => EventType::StockDividend,
            "organization_created" => EventType::OrganizationCreated,
            "organization_dissolved" => EventType::OrganizationDissolved,
            "organization_member_joined" => EventType::OrganizationMemberJoined,
            "organization_member_left" => EventType::OrganizationMemberLeft,
            "proposal_created" => EventType::ProposalCreated,
            "proposal_voting_started" => EventType::ProposalVotingStarted,
            "proposal_voted" => EventType::ProposalVoted,
            "proposal_executed" => EventType::ProposalExecuted,
            "proposal_rejected" => EventType::ProposalRejected,
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

// ── Organization & Governance Request Types ───────────────

#[derive(Debug, Deserialize)]
pub struct CreateOrgRequest {
    pub name: String,
    #[serde(rename = "type")]
    pub org_type: String,
    pub charter: CharterRequest,
    pub founders: Vec<FounderRequest>,
    pub founder_id: String,
    pub decision_mode: String,
}

#[derive(Debug, Deserialize)]
pub struct CharterRequest {
    #[serde(default)]
    pub purpose: String,
    #[serde(default = "default_governance")]
    pub governance: String,
    #[serde(default = "default_profit_sharing")]
    pub profit_sharing: String,
    #[serde(default)]
    pub membership_fee: u64,
}

fn default_governance() -> String { "vote".to_string() }
fn default_profit_sharing() -> String { "equal".to_string() }

#[derive(Debug, Deserialize)]
pub struct FounderRequest {
    pub agent_id: String,
    pub agent_name: String,
}

#[derive(Debug, Deserialize)]
pub struct JoinOrgRequest {
    pub agent_id: String,
    pub agent_name: String,
}

#[derive(Debug, Deserialize)]
pub struct LeaveOrgRequest {
    pub agent_id: String,
}

#[derive(Debug, Deserialize)]
pub struct DissolveOrgRequest {
    pub requester_id: String,
    #[serde(default)]
    pub reason: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateProposalRequest {
    pub proposer_id: String,
    pub proposal_type: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    pub payload: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct VoteProposalRequest {
    pub voter_id: String,
    pub in_favor: bool,
}

#[derive(Debug, Deserialize)]
pub struct StartVotingRequest {
    pub requester_id: String,
}

#[derive(Debug, Deserialize)]
pub struct CancelProposalRequest {
    pub requester_id: String,
}

#[derive(Debug, Deserialize)]
pub struct DistributionRequest {
    pub total_profit: u64,
}

// ── Organization & Governance Response Types ──────────────


#[derive(Debug, Serialize)]
pub struct OrgResponse {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub org_type: String,
    pub status: String,
    pub treasury: u64,
    pub debts: u64,
    pub member_count: usize,
    pub members: Vec<OrgMemberResponse>,
    pub created_tick: u64,
    pub last_activity_tick: u64,
    pub charter: String,
    pub decision_mode: String,
    pub profit_sharing: String,
    pub dissolved: bool,
    pub created_at: u64,
}

#[derive(Debug, Serialize)]
pub struct OrgMemberResponse {
    pub agent_id: String,
    pub agent_name: String,
    pub role: String,
    pub share: f64,
    pub joined_tick: u64,
}

#[derive(Debug, Serialize)]
pub struct MemberResponse {
    pub agent_id: String,
    pub role: String,
    pub contribution_score: u64,
    pub joined_at: u64,
}

#[derive(Debug, Serialize)]
pub struct ProposalResponse {
    pub id: String,
    pub org_id: String,
    pub proposer_id: String,
    pub proposal_type: String,
    pub title: String,
    pub description: String,
    pub status: String,
    pub votes_for: u32,
    pub votes_against: u32,
    pub total_votes: u32,
    pub created_at: u64,
}

impl From<&crate::organization::org::Organization> for OrgResponse {
    fn from(org: &crate::organization::org::Organization) -> Self {
        OrgResponse {
            id: org.id.clone(),
            name: org.name.clone(),
            org_type: format!("{:?}", org.org_type).to_lowercase(),
            status: format!("{:?}", org.status).to_lowercase(),
            treasury: org.treasury,
            debts: org.debts,
            member_count: org.members.len(),
            members: org.members.iter().map(|m| OrgMemberResponse {
                agent_id: m.agent_id.clone(),
                agent_name: m.agent_name.clone(),
                role: format!("{:?}", m.role).to_lowercase(),
                share: m.share,
                joined_tick: m.joined_tick,
            }).collect(),
            created_tick: org.created_tick,
            last_activity_tick: org.last_activity_tick,
            charter: String::new(),
            decision_mode: String::new(),
            profit_sharing: String::new(),
            dissolved: false,
            created_at: org.created_tick,
        }
    }
}

fn org_to_response(org: &crate::organization::governance::Organization) -> OrgResponse {
    OrgResponse {
        id: org.id.to_string(),
        name: org.name.clone(),
        org_type: String::new(),
        status: String::new(),
        treasury: 0,
        debts: 0,
        member_count: org.members.len(),
        members: Vec::new(),
        created_tick: org.created_at,
        last_activity_tick: org.created_at,
        charter: org.charter.clone(),
        decision_mode: org.decision_mode.to_string(),
        profit_sharing: org.profit_sharing.to_string(),
        dissolved: org.dissolved,
        created_at: org.created_at,
    }
}

fn proposal_to_response(proposal: &crate::organization::governance::Proposal) -> ProposalResponse {
    ProposalResponse {
        id: proposal.id.to_string(),
        org_id: proposal.org_id.to_string(),
        proposer_id: proposal.proposer_id.clone(),
        proposal_type: proposal.proposal_type.to_string(),
        title: proposal.title.clone(),
        description: proposal.description.clone(),
        status: proposal.status.to_string(),
        votes_for: proposal.votes_for(),
        votes_against: proposal.votes_against(),
        total_votes: proposal.votes_for() + proposal.votes_against(),
        created_at: proposal.created_at,
    }
}

fn parse_decision_mode(s: &str) -> Option<crate::organization::governance::DecisionMode> {
    use crate::organization::governance::DecisionMode;
    match s {
        "vote" => Some(DecisionMode::Vote),
        "dictator" => Some(DecisionMode::Dictator),
        "council" => Some(DecisionMode::Council),
        _ => None,
    }
}

fn parse_proposal_type(s: &str) -> Option<crate::organization::governance::ProposalType> {
    use crate::organization::governance::ProposalType;
    match s {
        "amend_charter" => Some(ProposalType::AmendCharter),
        "accept_member" => Some(ProposalType::AcceptMember),
        "expel_member" => Some(ProposalType::ExpelMember),
        "dissolve_org" => Some(ProposalType::DissolveOrg),
        "change_profit_sharing" => Some(ProposalType::ChangeProfitSharing),
        _ => None,
    }
}

fn governance_error_status(e: &crate::organization::governance::GovernanceError) -> StatusCode {
    use crate::organization::governance::GovernanceError;
    match e {
        GovernanceError::NotFound(_)
        | GovernanceError::OrganizationNotFound(_) => StatusCode::NOT_FOUND,
        GovernanceError::AlreadyMember { .. }
        | GovernanceError::AlreadyVoted { .. } => StatusCode::CONFLICT,
        GovernanceError::NotMember { .. }
        | GovernanceError::NotFounder { .. } => StatusCode::FORBIDDEN,
        GovernanceError::InvalidTransition { .. }
        | GovernanceError::VotingNotOpen(_)
        | GovernanceError::ProposalNotOpen(_) => StatusCode::CONFLICT,
        GovernanceError::OrganizationDissolved(_) => StatusCode::GONE,
        GovernanceError::CannotRemoveFounder => StatusCode::FORBIDDEN,
        GovernanceError::EmptyName => StatusCode::BAD_REQUEST,
    }
}

// ── Organization & Governance Handlers ────────────────────


async fn create_org(
    State(state): State<AppState>,
    Json(body): Json<CreateOrgRequest>,
) -> impl IntoResponse {
    let store = match &state.org_store {
        Some(s) => s.clone(),
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(ErrorResponse { error: "organization system not configured".into() })).into_response(),
    };

    let org_type = match body.org_type.as_str() {
        "company" => OrgType::Company,
        "guild" => OrgType::Guild,
        "alliance" => OrgType::Alliance,
        "university" => OrgType::University,
        other => return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: format!("unknown org type: {}", other) })).into_response(),
    };

    let governance = match body.charter.governance.as_str() {
        "vote" => GovernanceModel::Vote,
        "dictator" => GovernanceModel::Dictator,
        "council" => GovernanceModel::Council,
        other => return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: format!("unknown governance model: {}", other) })).into_response(),
    };

    let profit_sharing = match body.charter.profit_sharing.as_str() {
        "equal" => ProfitSharing::Equal,
        "proportional" => ProfitSharing::Proportional,
        "custom" => ProfitSharing::Custom,
        other => return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: format!("unknown profit sharing mode: {}", other) })).into_response(),
    };

    let charter = Charter {
        purpose: body.charter.purpose,
        governance,
        profit_sharing,
        membership_fee: body.charter.membership_fee,
    };

    let founders: Vec<(String, String)> = body.founders.into_iter()
        .map(|f| (f.agent_id, f.agent_name))
        .collect();

    let tick = *state.tick_rx.borrow();

    let mut store = store.lock().await;
    match store.create_org(body.name, org_type, Some(charter), founders, tick) {
        Ok(org) => (StatusCode::CREATED, Json(OrgResponse::from(&org))).into_response(),
        Err(e) => {
            let status = match &e {
                crate::organization::org::OrgError::NotFound(_) => StatusCode::NOT_FOUND,
                crate::organization::org::OrgError::NotEnoughFounders => StatusCode::BAD_REQUEST,
                crate::organization::org::OrgError::CharterRequired => StatusCode::BAD_REQUEST,
                crate::organization::org::OrgError::EmptyName => StatusCode::BAD_REQUEST,
                crate::organization::org::OrgError::AgentAlreadyInOrg(_) => StatusCode::CONFLICT,
                _ => StatusCode::BAD_REQUEST,
            };
            (status, Json(ErrorResponse { error: e.to_string() })).into_response()
        }
    }
}

async fn list_orgs(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let store = match &state.org_store {
        Some(s) => s.clone(),
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(ErrorResponse { error: "organization system not configured".into() })).into_response(),
    };

    let store = store.lock().await;
    let orgs: Vec<OrgResponse> = store.list().into_iter().map(OrgResponse::from).collect();
    Json(orgs).into_response()
}

async fn get_org(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let store = match &state.org_store {
        Some(s) => s.clone(),
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(ErrorResponse { error: "organization system not configured".into() })).into_response(),
    };

    let store = store.lock().await;
    match store.get(&id) {
        Some(org) => Json(OrgResponse::from(org)).into_response(),
        None => (StatusCode::NOT_FOUND, Json(ErrorResponse { error: "organization not found".into() })).into_response(),
    }
}

async fn join_org(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<JoinOrgRequest>,
) -> impl IntoResponse {
    let store = match &state.org_store {
        Some(s) => s.clone(),
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(ErrorResponse { error: "organization system not configured".into() })).into_response(),
    };

    let tick = *state.tick_rx.borrow();
    let mut store = store.lock().await;
    match store.join_org(&id, body.agent_id, body.agent_name, tick) {
        Ok(org) => Json(OrgResponse::from(&org)).into_response(),
        Err(e) => {
            let status = match &e {
                crate::organization::org::OrgError::NotFound(_) => StatusCode::NOT_FOUND,
                crate::organization::org::OrgError::OrgDissolved => StatusCode::CONFLICT,
                crate::organization::org::OrgError::AgentAlreadyInOrg(_) => StatusCode::CONFLICT,
                _ => StatusCode::BAD_REQUEST,
            };
            (status, Json(ErrorResponse { error: e.to_string() })).into_response()
        }
    }
}

async fn leave_org(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<LeaveOrgRequest>,
) -> impl IntoResponse {
    let store = match &state.org_store {
        Some(s) => s.clone(),
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(ErrorResponse { error: "organization system not configured".into() })).into_response(),
    };

    let tick = *state.tick_rx.borrow();
    let mut store = store.lock().await;
    match store.leave_org(&id, &body.agent_id, tick) {
        Ok(org) => Json(OrgResponse::from(&org)).into_response(),
        Err(e) => {
            let status = match &e {
                crate::organization::org::OrgError::NotFound(_) => StatusCode::NOT_FOUND,
                crate::organization::org::OrgError::OrgDissolved => StatusCode::CONFLICT,
                _ => StatusCode::BAD_REQUEST,
            };
            (status, Json(ErrorResponse { error: e.to_string() })).into_response()
        }
    }
}

async fn dissolve_org(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<DissolveOrgRequest>,
) -> impl IntoResponse {
    let store = match &state.org_store {
        Some(s) => s.clone(),
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(ErrorResponse { error: "organization system not configured".into() })).into_response(),
    };

    let mut store = store.lock().await;

    // Verify requester is a founder/leader
    let org = store.get(&id);
    match org {
        None => return (StatusCode::NOT_FOUND, Json(ErrorResponse { error: "organization not found".into() })).into_response(),
        Some(org) => {
            let member = org.get_member(&body.requester_id);
            match member {
                None => return (StatusCode::FORBIDDEN, Json(ErrorResponse { error: "requester is not a member".into() })).into_response(),
                Some(m) if !m.role.is_admin() => return (StatusCode::FORBIDDEN, Json(ErrorResponse { error: "only founders or leaders can dissolve".into() })).into_response(),
                _ => {}
            }
        }
    }

    let reason = if body.reason.is_empty() { "manual_dissolution".to_string() } else { body.reason };
    match store.dissolve_org(&id, &reason) {
        Ok(()) => Json(serde_json::json!({ "dissolved": true, "org_id": id })).into_response(),
        Err(e) => {
            let status = match &e {
                crate::organization::org::OrgError::NotFound(_) => StatusCode::NOT_FOUND,
                _ => StatusCode::BAD_REQUEST,
            };
            (status, Json(ErrorResponse { error: e.to_string() })).into_response()
        }
    }
}

// ── Stock Market Handlers ────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct IssueSharesRequest {
    pub org_id: String,
    pub ticker: String,
    pub total_shares: u64,
    pub price: u64,
}

#[derive(Debug, Deserialize)]
pub struct IpoRequest {
    pub org_member_count: usize,
    pub org_treasury: u64,
}

#[derive(Debug, Deserialize)]
pub struct BuyOrderRequest {
    pub stock_id: String,
    pub agent_id: String,
    pub order_kind: String,
    pub price: u64,
    pub quantity: u64,
    pub agent_funds: u64,
}

#[derive(Debug, Deserialize)]
pub struct SellOrderRequest {
    pub stock_id: String,
    pub agent_id: String,
    pub order_kind: String,
    pub price: u64,
    pub quantity: u64,
}

#[derive(Debug, Deserialize)]
pub struct CancelOrderRequest {
    pub agent_id: String,
}

#[derive(Debug, Deserialize)]
pub struct DividendRequest {
    pub total_profit: u64,
}

#[derive(Debug, Deserialize, Default)]
pub struct ListOrdersQuery {
    pub stock_id: Option<String>,
    pub agent_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct StockResponse {
    pub id: String,
    pub org_id: String,
    pub ticker: String,
    pub total_shares: u64,
    pub price: u64,
    pub status: String,
    pub listed_tick: u64,
}

impl From<&StockListing> for StockResponse {
    fn from(s: &StockListing) -> Self {
        StockResponse {
            id: s.id.clone(),
            org_id: s.org_id.clone(),
            ticker: s.ticker.clone(),
            total_shares: s.total_shares,
            price: s.price,
            status: match s.status {
                ListingStatus::PreIpo => "pre_ipo".to_string(),
                ListingStatus::Listed => "listed".to_string(),
                ListingStatus::Delisted => "delisted".to_string(),
            },
            listed_tick: s.listed_tick,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct OrderResponse {
    pub id: String,
    pub stock_id: String,
    pub agent_id: String,
    pub order_type: String,
    pub order_kind: String,
    pub price: u64,
    pub quantity: u64,
    pub filled_quantity: u64,
    pub status: String,
    pub created_tick: u64,
}

impl From<&StockOrder> for OrderResponse {
    fn from(o: &StockOrder) -> Self {
        OrderResponse {
            id: o.id.clone(),
            stock_id: o.stock_id.clone(),
            agent_id: o.agent_id.clone(),
            order_type: match o.order_type {
                OrderType::Buy => "buy".to_string(),
                OrderType::Sell => "sell".to_string(),
            },
            order_kind: match o.order_kind {
                OrderKind::Limit => "limit".to_string(),
                OrderKind::Market => "market".to_string(),
            },
            price: o.price,
            quantity: o.quantity,
            filled_quantity: o.filled_quantity,
            status: match o.status {
                crate::economy::stock_market::OrderStatus::Open => "open".to_string(),
                crate::economy::stock_market::OrderStatus::PartiallyFilled => "partially_filled".to_string(),
                crate::economy::stock_market::OrderStatus::Filled => "filled".to_string(),
                crate::economy::stock_market::OrderStatus::Cancelled => "cancelled".to_string(),
            },
            created_tick: o.created_tick,
        }
    }
}

fn stock_error_status(e: &crate::economy::stock_market::StockMarketError) -> StatusCode {
    use crate::economy::stock_market::StockMarketError;
    match e {
        StockMarketError::StockNotFound(_) => StatusCode::NOT_FOUND,
        StockMarketError::OrderNotFound(_) => StatusCode::NOT_FOUND,
        StockMarketError::OrgNotFound(_) => StatusCode::NOT_FOUND,
        StockMarketError::NotListed => StatusCode::CONFLICT,
        StockMarketError::Delisted => StatusCode::CONFLICT,
        StockMarketError::InsufficientShares(_, _) => StatusCode::BAD_REQUEST,
        StockMarketError::InsufficientFunds(_, _) => StatusCode::BAD_REQUEST,
        StockMarketError::NotShareholder => StatusCode::BAD_REQUEST,
        StockMarketError::OrderNotActive => StatusCode::CONFLICT,
        StockMarketError::IpoConditionsNotMet(_) => StatusCode::BAD_REQUEST,
        StockMarketError::TickerTaken(_) => StatusCode::CONFLICT,
        StockMarketError::AlreadyListed(_) => StatusCode::CONFLICT,
        StockMarketError::EmptyTicker => StatusCode::BAD_REQUEST,
        StockMarketError::InvalidShareCount => StatusCode::BAD_REQUEST,
        StockMarketError::InvalidPrice => StatusCode::BAD_REQUEST,
        StockMarketError::InvalidQuantity => StatusCode::BAD_REQUEST,
        StockMarketError::NoSharesIssued(_) => StatusCode::BAD_REQUEST,
        StockMarketError::NoProfitToDistribute => StatusCode::BAD_REQUEST,
        StockMarketError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

async fn list_stocks(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let sm = match &state.stock_market {
        Some(s) => s.clone(),
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(ErrorResponse { error: "stock market not configured".into() })).into_response(),
    };

    let sm = sm.lock().await;
    let stocks: Vec<StockResponse> = sm.list_stocks().into_iter().map(StockResponse::from).collect();
    Json(stocks).into_response()
}

async fn issue_shares(
    State(state): State<AppState>,
    Json(body): Json<IssueSharesRequest>,
) -> impl IntoResponse {
    let sm = match &state.stock_market {
        Some(s) => s.clone(),
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(ErrorResponse { error: "stock market not configured".into() })).into_response(),
    };

    let tick = *state.tick_rx.borrow();
    let mut sm = sm.lock().await;
    match sm.issue_shares(body.org_id, body.ticker, body.total_shares, body.price, tick) {
        Ok(stock) => (StatusCode::CREATED, Json(StockResponse::from(&stock))).into_response(),
        Err(e) => (stock_error_status(&e), Json(ErrorResponse { error: e.to_string() })).into_response(),
    }
}

async fn get_stock(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let sm = match &state.stock_market {
        Some(s) => s.clone(),
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(ErrorResponse { error: "stock market not configured".into() })).into_response(),
    };

    let sm = sm.lock().await;
    match sm.get_stock(&id) {
        Some(stock) => Json(StockResponse::from(stock)).into_response(),
        None => (StatusCode::NOT_FOUND, Json(ErrorResponse { error: "stock not found".into() })).into_response(),
    }
}

async fn ipo_stock(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<IpoRequest>,
) -> impl IntoResponse {
    let sm = match &state.stock_market {
        Some(s) => s.clone(),
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(ErrorResponse { error: "stock market not configured".into() })).into_response(),
    };

    let tick = *state.tick_rx.borrow();
    let mut sm = sm.lock().await;
    match sm.ipo(&id, body.org_member_count, body.org_treasury, tick) {
        Ok(stock) => Json(StockResponse::from(&stock)).into_response(),
        Err(e) => (stock_error_status(&e), Json(ErrorResponse { error: e.to_string() })).into_response(),
    }
}

async fn place_buy_order(
    State(state): State<AppState>,
    Json(body): Json<BuyOrderRequest>,
) -> impl IntoResponse {
    let sm = match &state.stock_market {
        Some(s) => s.clone(),
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(ErrorResponse { error: "stock market not configured".into() })).into_response(),
    };

    let order_kind = match body.order_kind.as_str() {
        "limit" => OrderKind::Limit,
        "market" => OrderKind::Market,
        other => return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: format!("unknown order kind: {}", other) })).into_response(),
    };

    let tick = *state.tick_rx.borrow();
    let mut sm = sm.lock().await;
    match sm.place_buy_order(&body.stock_id, &body.agent_id, order_kind, body.price, body.quantity, body.agent_funds, tick) {
        Ok(order) => (StatusCode::CREATED, Json(OrderResponse::from(&order))).into_response(),
        Err(e) => (stock_error_status(&e), Json(ErrorResponse { error: e.to_string() })).into_response(),
    }
}

async fn place_sell_order(
    State(state): State<AppState>,
    Json(body): Json<SellOrderRequest>,
) -> impl IntoResponse {
    let sm = match &state.stock_market {
        Some(s) => s.clone(),
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(ErrorResponse { error: "stock market not configured".into() })).into_response(),
    };

    let order_kind = match body.order_kind.as_str() {
        "limit" => OrderKind::Limit,
        "market" => OrderKind::Market,
        other => return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: format!("unknown order kind: {}", other) })).into_response(),
    };

    let tick = *state.tick_rx.borrow();
    let mut sm = sm.lock().await;
    match sm.place_sell_order(&body.stock_id, &body.agent_id, order_kind, body.price, body.quantity, tick) {
        Ok(order) => (StatusCode::CREATED, Json(OrderResponse::from(&order))).into_response(),
        Err(e) => (stock_error_status(&e), Json(ErrorResponse { error: e.to_string() })).into_response(),
    }
}

async fn list_stock_orders(
    State(state): State<AppState>,
    Query(query): Query<ListOrdersQuery>,
) -> impl IntoResponse {
    let sm = match &state.stock_market {
        Some(s) => s.clone(),
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(ErrorResponse { error: "stock market not configured".into() })).into_response(),
    };

    let sm = sm.lock().await;
    let orders: Vec<OrderResponse> = sm.list_orders(query.stock_id.as_deref(), query.agent_id.as_deref())
        .into_iter()
        .map(OrderResponse::from)
        .collect();
    Json(orders).into_response()
}

async fn get_order(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let sm = match &state.stock_market {
        Some(s) => s.clone(),
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(ErrorResponse { error: "stock market not configured".into() })).into_response(),
    };

    let sm = sm.lock().await;
    match sm.get_order(&id) {
        Some(order) => Json(OrderResponse::from(order)).into_response(),
        None => (StatusCode::NOT_FOUND, Json(ErrorResponse { error: "order not found".into() })).into_response(),
    }
}

async fn cancel_order(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<CancelOrderRequest>,
) -> impl IntoResponse {
    let sm = match &state.stock_market {
        Some(s) => s.clone(),
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(ErrorResponse { error: "stock market not configured".into() })).into_response(),
    };

    let mut sm = sm.lock().await;
    match sm.cancel_order(&id, &body.agent_id) {
        Ok(order) => Json(OrderResponse::from(&order)).into_response(),
        Err(e) => (stock_error_status(&e), Json(ErrorResponse { error: e.to_string() })).into_response(),
    }
}

async fn distribute_dividend(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<DividendRequest>,
) -> impl IntoResponse {
    let sm = match &state.stock_market {
        Some(s) => s.clone(),
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(ErrorResponse { error: "stock market not configured".into() })).into_response(),
    };

    let tick = *state.tick_rx.borrow();
    let mut sm = sm.lock().await;
    match sm.distribute_dividends(&id, body.total_profit, tick) {
        Ok(record) => (StatusCode::CREATED, Json(&record)).into_response(),
        Err(e) => (stock_error_status(&e), Json(ErrorResponse { error: e.to_string() })).into_response(),
    }
}

// ── Governance Handlers ─────────────────────────────────

async fn calculate_distribution(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<DistributionRequest>,
) -> impl IntoResponse {
    let governance = match &state.governance {
        Some(g) => g.clone(),
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(ErrorResponse { error: "governance system not configured".into() })).into_response(),
    };

    let Ok(uuid) = Uuid::parse_str(&id) else {
        return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "invalid org id".into() })).into_response();
    };

    let gov = governance.lock().await;
    match gov.get_org(uuid) {
        Some(org) => {
            let dist = org.calculate_distribution(body.total_profit);
            Json(dist).into_response()
        }
        None => (StatusCode::NOT_FOUND, Json(ErrorResponse { error: "organization not found".into() })).into_response(),
    }
}

async fn create_proposal(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<CreateProposalRequest>,
) -> impl IntoResponse {
    let governance = match &state.governance {
        Some(g) => g.clone(),
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(ErrorResponse { error: "governance system not configured".into() })).into_response(),
    };

    let Ok(org_uuid) = Uuid::parse_str(&id) else {
        return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "invalid org id".into() })).into_response();
    };

    let proposal_type = match parse_proposal_type(&body.proposal_type) {
        Some(t) => t,
        None => return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "invalid proposal_type, must be: amend_charter, accept_member, expel_member, dissolve_org, change_profit_sharing".into() })).into_response(),
    };

    if body.title.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "title is required".into() })).into_response();
    }

    let tick = *state.tick_rx.borrow();
    let mut gov = governance.lock().await;
    match gov.create_proposal(org_uuid, body.proposer_id, proposal_type, body.title, body.description, tick, body.payload) {
        Ok(proposal_id) => {
            let proposal = gov.get_proposal(proposal_id).unwrap();
            (StatusCode::CREATED, Json(proposal_to_response(proposal))).into_response()
        }
        Err(e) => (governance_error_status(&e), Json(ErrorResponse { error: e.to_string() })).into_response(),
    }
}

async fn list_proposals(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let governance = match &state.governance {
        Some(g) => g.clone(),
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(ErrorResponse { error: "governance system not configured".into() })).into_response(),
    };

    let Ok(org_uuid) = Uuid::parse_str(&id) else {
        return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "invalid org id".into() })).into_response();
    };

    let gov = governance.lock().await;
    let proposals: Vec<ProposalResponse> = gov.list_org_proposals(org_uuid)
        .into_iter()
        .map(proposal_to_response)
        .collect();
    Json(proposals).into_response()
}

async fn get_proposal(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let governance = match &state.governance {
        Some(g) => g.clone(),
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(ErrorResponse { error: "governance system not configured".into() })).into_response(),
    };

    let Ok(uuid) = Uuid::parse_str(&id) else {
        return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "invalid proposal id".into() })).into_response();
    };

    let gov = governance.lock().await;
    match gov.get_proposal(uuid) {
        Some(proposal) => Json(proposal_to_response(proposal)).into_response(),
        None => (StatusCode::NOT_FOUND, Json(ErrorResponse { error: "proposal not found".into() })).into_response(),
    }
}

async fn vote_proposal(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<VoteProposalRequest>,
) -> impl IntoResponse {
    let governance = match &state.governance {
        Some(g) => g.clone(),
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(ErrorResponse { error: "governance system not configured".into() })).into_response(),
    };

    let Ok(uuid) = Uuid::parse_str(&id) else {
        return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "invalid proposal id".into() })).into_response();
    };

    let tick = *state.tick_rx.borrow();
    let mut gov = governance.lock().await;
    match gov.vote(uuid, body.voter_id, body.in_favor, tick) {
        Ok(()) => {
            let proposal = gov.get_proposal(uuid).unwrap();
            Json(proposal_to_response(proposal)).into_response()
        }
        Err(e) => (governance_error_status(&e), Json(ErrorResponse { error: e.to_string() })).into_response(),
    }
}

async fn start_voting(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<StartVotingRequest>,
) -> impl IntoResponse {
    let governance = match &state.governance {
        Some(g) => g.clone(),
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(ErrorResponse { error: "governance system not configured".into() })).into_response(),
    };

    let Ok(uuid) = Uuid::parse_str(&id) else {
        return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "invalid proposal id".into() })).into_response();
    };

    let mut gov = governance.lock().await;
    match gov.start_voting(uuid, &body.requester_id) {
        Ok(()) => {
            let proposal = gov.get_proposal(uuid).unwrap();
            Json(proposal_to_response(proposal)).into_response()
        }
        Err(e) => (governance_error_status(&e), Json(ErrorResponse { error: e.to_string() })).into_response(),
    }
}

async fn tally_proposal(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let governance = match &state.governance {
        Some(g) => g.clone(),
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(ErrorResponse { error: "governance system not configured".into() })).into_response(),
    };

    let Ok(uuid) = Uuid::parse_str(&id) else {
        return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "invalid proposal id".into() })).into_response();
    };

    let mut gov = governance.lock().await;
    match gov.tally_proposal(uuid) {
        Ok(_status) => {
            let proposal = gov.get_proposal(uuid).unwrap();
            Json(proposal_to_response(proposal)).into_response()
        }
        Err(e) => (governance_error_status(&e), Json(ErrorResponse { error: e.to_string() })).into_response(),
    }
}

async fn cancel_proposal(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<CancelProposalRequest>,
) -> impl IntoResponse {
    let governance = match &state.governance {
        Some(g) => g.clone(),
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(ErrorResponse { error: "governance system not configured".into() })).into_response(),
    };

    let Ok(uuid) = Uuid::parse_str(&id) else {
        return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "invalid proposal id".into() })).into_response();
    };

    let mut gov = governance.lock().await;
    match gov.cancel_proposal(uuid, &body.requester_id) {
        Ok(()) => {
            let proposal = gov.get_proposal(uuid).unwrap();
            Json(proposal_to_response(proposal)).into_response()
        }
        Err(e) => (governance_error_status(&e), Json(ErrorResponse { error: e.to_string() })).into_response(),
    }
}
