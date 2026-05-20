use std::collections::HashMap;
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
use crate::economy::stock_market::{
    StockMarket, StockListing, Order as StockOrder,
    OrderType, OrderKind, ListingStatus,
};
use crate::economy::task::{TaskBoard, Task};
use crate::organization::org::{OrganizationStore, OrgType};
use crate::organization::charter::{Charter, GovernanceModel, ProfitSharing};
use crate::organization::governance::GovernanceSystem;
use crate::organization::governance_metrics::GovernanceMetricsCollector;
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
pub type SharedBankingSystem = Arc<Mutex<BankingSystem>>;
pub type SharedTraceStore = Arc<Mutex<crate::tracing::TraceStore>>;
pub type SharedGovernanceMetricsCollector = Arc<Mutex<GovernanceMetricsCollector>>;

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
    #[serde(default)]
    pub personality: String,
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
    pub banking_system: Option<SharedBankingSystem>,
    pub trace_store: Option<SharedTraceStore>,
    pub external_agents: SharedExternalAgents,
    pub governance_metrics: Option<SharedGovernanceMetricsCollector>,
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
        banking_system: None,
        trace_store: Some(Arc::new(Mutex::new(crate::tracing::TraceStore::new()))),
        external_agents: Arc::new(Mutex::new(HashMap::new())),
        governance_metrics: None,
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
        banking_system: None,
        trace_store: Some(Arc::new(Mutex::new(crate::tracing::TraceStore::new()))),
        external_agents: Arc::new(Mutex::new(HashMap::new())),
        governance_metrics: None,
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
        .route("/api/v1/agents/:id", get(get_agent).delete(deregister_external_agent))
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
        // Agent Trace routes (latest before :tick to avoid capture)
        .route("/api/v1/agents/:id/traces", get(list_agent_traces))
        .route("/api/v1/agents/:id/traces/latest", get(get_latest_trace))
        .route("/api/v1/agents/:id/traces/:tick", get(get_trace_by_tick))
        .route("/api/v1/agents/:id/traces", post(submit_trace))
        // Data Export routes
        .route("/api/v1/export/snapshot", get(export_snapshot))
        .route("/api/v1/export/snapshot/:tick", get(export_snapshot_by_tick))
        .route("/api/v1/export/economy", get(export_economy))
        .route("/api/v1/export/query", post(export_query))
        // Third-Party Agent API
        .route("/api/v1/agents/register", post(register_external_agent))
        .route("/api/v1/agents/:id/action", post(execute_agent_action))
        .route("/api/v1/agents/:id/perception", get(get_agent_perception))
        .route("/api/v1/agents/:id/status", get(get_agent_status))
        // Modify existing agent GET to also support DELETE for deregister
        // Governance Metrics routes
        .route("/api/v1/governance/summary", get(governance_summary))
        .route("/api/v1/governance/orgs/:org_id", get(governance_org_metrics))
        .route("/api/v1/governance/orgs/:org_id/timeline", get(governance_org_timeline))
        .route("/api/v1/governance/comparison", get(governance_comparison))
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
        banking_system: None,
        trace_store: Some(Arc::new(Mutex::new(crate::tracing::TraceStore::new()))),
        external_agents: Arc::new(Mutex::new(HashMap::new())),
        governance_metrics: None,
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

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ListTasksQuery {
    pub status: Option<String>,
    pub publisher_id: Option<String>,
    pub assignee_id: Option<String>,
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

// ── Third-Party Agent Types ──────────────────────────────

/// Valid action types for external agents.
const ALLOWED_ACTIONS: &[&str] = &[
    "move", "gather", "trade", "communicate",
    "explore", "rest", "build", "claim_task", "submit_task",
];

/// Position in the world grid.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub x: i64,
    pub y: i64,
}

/// An externally registered agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalAgent {
    pub agent_id: String,
    pub name: String,
    pub api_key: String,
    pub capabilities: Vec<String>,
    pub config: serde_json::Value,
    pub alive: bool,
    pub phase: String,
    pub tokens: u64,
    pub money: u64,
    pub position: Position,
    pub registered_tick: u64,
}

/// Shared store for external agents.
pub type SharedExternalAgents = Arc<Mutex<HashMap<String, ExternalAgent>>>;

#[derive(Debug, Deserialize)]
pub struct RegisterAgentRequest {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub config: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct AgentActionRequest {
    pub action: String,
    #[serde(default)]
    pub params: serde_json::Value,
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

// ── Data Export Types ─────────────────────────────────────

fn default_format() -> String { "json".to_string() }

#[derive(Debug, Deserialize)]
pub struct ExportSnapshotQuery {
    #[serde(default = "default_format")]
    pub format: String,
}

#[derive(Debug, Serialize)]
pub struct WorldSnapshotExport {
    pub tick: u64,
    pub agents: Vec<AgentRecord>,
    pub messages: Vec<A2AMessage>,
    pub task_count: usize,
    pub exported_at: String,
}

#[derive(Debug, Deserialize)]
pub struct ExportEconomyQuery {
    #[serde(default)]
    pub from_tick: Option<u64>,
    #[serde(default)]
    pub to_tick: Option<u64>,
    #[serde(default = "default_format")]
    pub format: String,
}

#[derive(Debug, Serialize)]
pub struct EconomyTickData {
    pub tick: u64,
    pub total_money: u64,
    pub total_tokens: u64,
    pub alive_count: usize,
    pub gini_coefficient: f64,
    pub task_count: usize,
}

#[derive(Debug, Deserialize)]
pub struct ExportQueryRequest {
    pub filters: ExportFilters,
}

#[derive(Debug, Deserialize)]
pub struct ExportFilters {
    #[serde(default)]
    pub agent_ids: Option<Vec<String>>,
    #[serde(default)]
    pub event_types: Option<Vec<String>>,
    pub tick_range: Option<(u64, u64)>,
    #[serde(default = "default_format")]
    pub format: String,
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
        personality: String::new(),
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
            personality: String::new(),
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

#[allow(dead_code)]
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

#[allow(dead_code)]
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

// ── Trace Handlers ────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct ListTracesQuery {
    limit: Option<usize>,
    offset: Option<usize>,
}

async fn list_agent_traces(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
    Query(query): Query<ListTracesQuery>,
) -> impl IntoResponse {
    let store = match &state.trace_store {
        Some(s) => s.clone(),
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(ErrorResponse { error: "trace store not configured".into() })).into_response(),
    };
    let store = store.lock().await;
    let limit = query.limit.unwrap_or(100);
    let offset = query.offset.unwrap_or(0);
    let timeline = store.get_timeline(&agent_id, limit, offset);
    Json(&timeline).into_response()
}

async fn get_latest_trace(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
) -> impl IntoResponse {
    let store = match &state.trace_store {
        Some(s) => s.clone(),
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(ErrorResponse { error: "trace store not configured".into() })).into_response(),
    };
    let store = store.lock().await;
    match store.get_latest(&agent_id) {
        Some(trace) => Json(trace).into_response(),
        None => (StatusCode::NOT_FOUND, Json(ErrorResponse { error: "no traces for agent".into() })).into_response(),
    }
}

async fn get_trace_by_tick(
    State(state): State<AppState>,
    Path((agent_id, tick)): Path<(String, u64)>,
) -> impl IntoResponse {
    let store = match &state.trace_store {
        Some(s) => s.clone(),
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(ErrorResponse { error: "trace store not configured".into() })).into_response(),
    };
    let store = store.lock().await;
    match store.get_tick(&agent_id, tick) {
        Some(trace) => Json(trace).into_response(),
        None => (StatusCode::NOT_FOUND, Json(ErrorResponse { error: format!("no trace at tick {} for agent", tick) })).into_response(),
    }
}

async fn submit_trace(
    State(state): State<AppState>,
    Json(trace): Json<crate::tracing::TickTraceData>,
) -> impl IntoResponse {
    let store = match &state.trace_store {
        Some(s) => s.clone(),
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(ErrorResponse { error: "trace store not configured".into() })).into_response(),
    };
    let mut store = store.lock().await;
    store.save(trace);
    (StatusCode::OK, Json(serde_json::json!({"status": "ok"}))).into_response()
}

// ── Data Export Handlers ──────────────────────────────────

fn compute_gini(values: &[u64]) -> f64 {
    if values.is_empty() { return 0.0; }
    let n = values.len() as f64;
    let mean: f64 = values.iter().sum::<u64>() as f64 / n;
    if mean == 0.0 { return 0.0; }
    let mut sorted: Vec<u64> = values.to_vec();
    sorted.sort();
    let sum_diff: f64 = sorted.iter()
        .flat_map(|&xi| sorted.iter().map(move |&xj| (xi as f64 - xj as f64).abs()))
        .sum();
    sum_diff / (2.0 * n * n * mean)
}

fn agents_to_csv(agents: &[AgentRecord]) -> String {
    let mut csv = String::from("id,name,phase,tokens,money,alive,ticks_survived,personality\n");
    for a in agents {
        csv.push_str(&format!(
            "{},{},{},{},{},{},{},{}\n",
            a.id, a.name, a.phase, a.tokens, a.money, a.alive, a.ticks_survived, a.personality
        ));
    }
    csv
}

async fn export_snapshot(
    State(state): State<AppState>,
    Query(query): Query<ExportSnapshotQuery>,
) -> impl IntoResponse {
    let agents = state.agents.lock().await;
    let messages = state.messages.lock().await;
    let board = state.board.lock().await;
    let tick = *state.tick_rx.borrow();
    let task_count = board.list().len();

    let exported_at = chrono::Utc::now().to_rfc3339();
    let snapshot = WorldSnapshotExport {
        tick,
        agents: agents.clone(),
        messages: messages.clone(),
        task_count,
        exported_at: exported_at.clone(),
    };

    match query.format.as_str() {
        "csv" => {
            let csv = agents_to_csv(&agents);
            let body = axum::body::Body::from(csv);
            let mut resp = axum::response::Response::new(body);
            *resp.status_mut() = StatusCode::OK;
            resp.headers_mut().insert("content-type", "text/csv".parse().unwrap());
            resp.headers_mut().insert("content-disposition", "attachment; filename=\"world_snapshot.csv\"".parse().unwrap());
            resp
        }
        _ => {
            match serde_json::to_string_pretty(&snapshot) {
                Ok(json) => {
                    let body = axum::body::Body::from(json);
                    let mut resp = axum::response::Response::new(body);
                    *resp.status_mut() = StatusCode::OK;
                    resp.headers_mut().insert("content-type", "application/json".parse().unwrap());
                    resp.headers_mut().insert("content-disposition", "attachment; filename=\"world_snapshot.json\"".parse().unwrap());
                    resp
                }
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: e.to_string() })).into_response(),
            }
        }
    }
}

async fn export_snapshot_by_tick(
    State(state): State<AppState>,
    Path(tick): Path<u64>,
    Query(query): Query<ExportSnapshotQuery>,
) -> impl IntoResponse {
    let agents = state.agents.lock().await;
    let messages = state.messages.lock().await;
    let board = state.board.lock().await;
    let task_count = board.list().len();

    // Filter messages to the specific tick
    let filtered_messages: Vec<A2AMessage> = messages.iter()
        .filter(|m| m.tick == tick)
        .cloned()
        .collect();

    let exported_at = chrono::Utc::now().to_rfc3339();
    let snapshot = WorldSnapshotExport {
        tick,
        agents: agents.clone(),
        messages: filtered_messages,
        task_count,
        exported_at: exported_at.clone(),
    };

    match query.format.as_str() {
        "csv" => {
            let csv = agents_to_csv(&agents);
            let body = axum::body::Body::from(csv);
            let mut resp = axum::response::Response::new(body);
            *resp.status_mut() = StatusCode::OK;
            resp.headers_mut().insert("content-type", "text/csv".parse().unwrap());
            resp.headers_mut().insert("content-disposition", "attachment; filename=\"world_snapshot.csv\"".parse().unwrap());
            resp
        }
        _ => {
            match serde_json::to_string_pretty(&snapshot) {
                Ok(json) => {
                    let body = axum::body::Body::from(json);
                    let mut resp = axum::response::Response::new(body);
                    *resp.status_mut() = StatusCode::OK;
                    resp.headers_mut().insert("content-type", "application/json".parse().unwrap());
                    resp.headers_mut().insert("content-disposition", "attachment; filename=\"world_snapshot.json\"".parse().unwrap());
                    resp
                }
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: e.to_string() })).into_response(),
            }
        }
    }
}

async fn export_economy(
    State(state): State<AppState>,
    Query(query): Query<ExportEconomyQuery>,
) -> impl IntoResponse {
    let agents = state.agents.lock().await;
    let board = state.board.lock().await;
    let current_tick = *state.tick_rx.borrow();

    let _from_tick = query.from_tick.unwrap_or(0);
    let _to_tick = query.to_tick.unwrap_or(current_tick);

    // For now we compute a single data point for the current state
    // (since we don't store per-tick history in memory)
    let tick = current_tick;
    let total_money: u64 = agents.iter().map(|a| a.money).sum();
    let total_tokens: u64 = agents.iter().map(|a| a.tokens).sum();
    let alive_count = agents.iter().filter(|a| a.alive).count();
    let task_count = board.list().len();
    let money_values: Vec<u64> = agents.iter().map(|a| a.money).collect();
    let gini = compute_gini(&money_values);

    // Build data points — single point for current state
    let data = vec![EconomyTickData {
        tick,
        total_money,
        total_tokens,
        alive_count,
        gini_coefficient: gini,
        task_count,
    }];

    match query.format.as_str() {
        "csv" => {
            let mut csv = String::from("tick,total_money,total_tokens,alive_count,gini,task_count\n");
            for d in &data {
                csv.push_str(&format!(
                    "{},{},{},{},{},{}\n",
                    d.tick, d.total_money, d.total_tokens, d.alive_count, d.gini_coefficient, d.task_count
                ));
            }
            let body = axum::body::Body::from(csv);
            let mut resp = axum::response::Response::new(body);
            *resp.status_mut() = StatusCode::OK;
            resp.headers_mut().insert("content-type", "text/csv".parse().unwrap());
            resp.headers_mut().insert("content-disposition", "attachment; filename=\"economy_export.csv\"".parse().unwrap());
            resp
        }
        _ => {
            match serde_json::to_string_pretty(&data) {
                Ok(json) => {
                    let body = axum::body::Body::from(json);
                    let mut resp = axum::response::Response::new(body);
                    *resp.status_mut() = StatusCode::OK;
                    resp.headers_mut().insert("content-type", "application/json".parse().unwrap());
                    resp.headers_mut().insert("content-disposition", "attachment; filename=\"economy_export.json\"".parse().unwrap());
                    resp
                }
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: e.to_string() })).into_response(),
            }
        }
    }
}

async fn export_query(
    State(state): State<AppState>,
    Json(body): Json<ExportQueryRequest>,
) -> impl IntoResponse {
    let filters = body.filters;

    // Validate tick_range
    let tick_range = match filters.tick_range {
        Some((from, to)) if from <= to => Some((from, to)),
        Some((from, to)) => {
            return (StatusCode::BAD_REQUEST, Json(ErrorResponse {
                error: format!("invalid tick_range: start ({}) must be <= end ({})", from, to),
            })).into_response();
        }
        None => {
            return (StatusCode::BAD_REQUEST, Json(ErrorResponse {
                error: "tick_range is required".into(),
            })).into_response();
        }
    };

    // Filter agents
    let agents = state.agents.lock().await;
    let filtered_agents: Vec<AgentRecord> = match &filters.agent_ids {
        Some(ids) => agents.iter()
            .filter(|a| ids.contains(&a.id))
            .cloned()
            .collect(),
        None => agents.clone(),
    };

    // Filter traces
    let trace_data = match &state.trace_store {
        Some(store) => {
            let store = store.lock().await;
            let agent_ids_ref = filters.agent_ids.as_deref();
            let all_traces = store.get_all_traces(agent_ids_ref, tick_range);

            // Filter by event_types if provided
            if let Some(ref event_types) = filters.event_types {
                all_traces.into_iter()
                    .filter(|trace| {
                        trace.phases.iter().any(|p| event_types.contains(&p.phase))
                    })
                    .map(|t| t.clone())
                    .collect::<Vec<_>>()
            } else {
                all_traces.into_iter().cloned().collect::<Vec<_>>()
            }
        }
        None => Vec::new(),
    };

    let result = serde_json::json!({
        "agents": filtered_agents,
        "traces": trace_data,
        "tick_range": tick_range,
    });

    match filters.format.as_str() {
        "csv" => {
            let csv = agents_to_csv(&filtered_agents);
            let body = axum::body::Body::from(csv);
            let mut resp = axum::response::Response::new(body);
            *resp.status_mut() = StatusCode::OK;
            resp.headers_mut().insert("content-type", "text/csv".parse().unwrap());
            resp.headers_mut().insert("content-disposition", "attachment; filename=\"query_export.csv\"".parse().unwrap());
            resp
        }
        _ => {
            match serde_json::to_string_pretty(&result) {
                Ok(json) => {
                    let body = axum::body::Body::from(json);
                    let mut resp = axum::response::Response::new(body);
                    *resp.status_mut() = StatusCode::OK;
                    resp.headers_mut().insert("content-type", "application/json".parse().unwrap());
                    resp.headers_mut().insert("content-disposition", "attachment; filename=\"query_export.json\"".parse().unwrap());
                    resp
                }
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: e.to_string() })).into_response(),
            }
        }
    }
}

// ── Third-Party Agent API Handlers ────────────────────────

/// Register a new third-party agent.
async fn register_external_agent(
    State(state): State<AppState>,
    Json(body): Json<RegisterAgentRequest>,
) -> impl IntoResponse {
    if body.name.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "name is required".into() })).into_response();
    }

    let agent_id = Uuid::new_v4().to_string();
    let api_key = Uuid::new_v4().to_string();
    let tick = *state.tick_rx.borrow();

    let name = body.name.clone();
    let agent = ExternalAgent {
        agent_id: agent_id.clone(),
        name: name.clone(),
        api_key: api_key.clone(),
        capabilities: body.capabilities,
        config: body.config,
        alive: true,
        phase: "exploration".to_string(),
        tokens: 100_000,
        money: 0,
        position: Position { x: 0, y: 0 },
        registered_tick: tick,
    };

    {
        let mut external = state.external_agents.lock().await;
        external.insert(agent_id.clone(), agent);
    }

    // Also add to the shared agents list for world_stats compatibility
    {
        let mut agents = state.agents.lock().await;
        agents.push(AgentRecord {
            id: agent_id.clone(),
            name: name.clone(),
            phase: "exploration".to_string(),
            tokens: 100_000,
            money: 0,
            alive: true,
            ticks_survived: 0,
            personality: String::new(),
        });
    }

    (StatusCode::CREATED, Json(serde_json::json!({
        "agent_id": agent_id,
        "api_key": api_key,
        "name": name,
    }))).into_response()
}

/// Deregister (remove) a third-party agent.
async fn deregister_external_agent(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let agent_id = {
        let mut external = state.external_agents.lock().await;
        match external.remove(&id) {
            Some(agent) => agent.agent_id,
            None => return (StatusCode::NOT_FOUND, Json(ErrorResponse { error: "agent not found".into() })).into_response(),
        }
    };

    // Also remove from the shared agents list
    {
        let mut agents = state.agents.lock().await;
        agents.retain(|a| a.id != agent_id);
    }

    (StatusCode::OK, Json(serde_json::json!({
        "deregistered": agent_id,
    }))).into_response()
}

/// Execute an action as a third-party agent.
async fn execute_agent_action(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<AgentActionRequest>,
) -> impl IntoResponse {
    // Validate action
    if !ALLOWED_ACTIONS.contains(&body.action.as_str()) {
        return (StatusCode::BAD_REQUEST, Json(ErrorResponse {
            error: format!("unknown action '{}'", body.action),
        })).into_response();
    }

    // Check agent exists and is alive
    let mut external = state.external_agents.lock().await;
    let agent = match external.get_mut(&id) {
        Some(a) if a.alive => a,
        Some(_) => return (StatusCode::GONE, Json(ErrorResponse { error: "agent is dead".into() })).into_response(),
        None => return (StatusCode::NOT_FOUND, Json(ErrorResponse { error: "agent not found".into() })).into_response(),
    };

    // Execute action — update position for "move", etc.
    let tick = *state.tick_rx.borrow();
    let success = match body.action.as_str() {
        "move" => {
            if let Some(dir) = body.params.get("direction").and_then(|d| d.as_str()) {
                let distance = body.params.get("distance")
                    .and_then(|d| d.as_u64())
                    .unwrap_or(1) as i64;
                match dir {
                    "north" => agent.position.y += distance,
                    "south" => agent.position.y -= distance,
                    "east" => agent.position.x += distance,
                    "west" => agent.position.x -= distance,
                    _ => {}
                }
            }
            true
        }
        "gather" => {
            agent.money += 10;
            true
        }
        "rest" => true,
        "explore" => true,
        "communicate" => true,
        "trade" => true,
        "build" => true,
        "claim_task" => true,
        "submit_task" => true,
        _ => false,
    };

    let action_name = body.action.clone();

    (StatusCode::OK, Json(serde_json::json!({
        "action": action_name,
        "success": success,
        "tick": tick,
    }))).into_response()
}

/// Get perception data for a third-party agent.
async fn get_agent_perception(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let external = state.external_agents.lock().await;
    let agent = match external.get(&id) {
        Some(a) if a.alive => a,
        Some(_) => return (StatusCode::GONE, Json(ErrorResponse { error: "agent is dead".into() })).into_response(),
        None => return (StatusCode::NOT_FOUND, Json(ErrorResponse { error: "agent not found".into() })).into_response(),
    };

    let tick = *state.tick_rx.borrow();

    // Build perception from the world state
    let agents = state.agents.lock().await;
    let nearby_agents: Vec<serde_json::Value> = agents.iter()
        .filter(|a| a.alive && a.id != id)
        .take(10)
        .map(|a| serde_json::json!({
            "id": a.id,
            "name": a.name,
        }))
        .collect();

    let nearby_resources: Vec<serde_json::Value> = vec![
        serde_json::json!({ "type": "food", "position": { "x": 1, "y": 1 } }),
        serde_json::json!({ "type": "wood", "position": { "x": 3, "y": 5 } }),
    ];

    (StatusCode::OK, Json(serde_json::json!({
        "agent_id": id,
        "nearby_agents": nearby_agents,
        "nearby_resources": nearby_resources,
        "position": agent.position,
        "world_tick": tick,
    }))).into_response()
}

/// Get the status of a third-party agent.
async fn get_agent_status(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let external = state.external_agents.lock().await;
    let agent = match external.get(&id) {
        Some(a) => a,
        None => return (StatusCode::NOT_FOUND, Json(ErrorResponse { error: "agent not found".into() })).into_response(),
    };

    let tick = *state.tick_rx.borrow();

    (StatusCode::OK, Json(serde_json::json!({
        "agent_id": agent.agent_id,
        "name": agent.name,
        "alive": agent.alive,
        "phase": agent.phase,
        "tokens": agent.tokens,
        "money": agent.money,
        "position": agent.position,
        "registered_tick": agent.registered_tick,
        "current_tick": tick,
    }))).into_response()
}

// ── Governance Metrics Handlers ──────────────────────────────

/// Query parameters for governance comparison endpoint.
#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct GovernanceComparisonQuery {
    pub org_ids: Option<String>,
}

impl Default for GovernanceComparisonQuery {
    fn default() -> Self {
        Self { org_ids: None }
    }
}

/// Query parameters for governance timeline endpoint.
#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct GovernanceTimelineQuery {
    pub event_type: Option<crate::world::event::EventType>,
    pub from_tick: Option<u64>,
    pub to_tick: Option<u64>,
}

impl Default for GovernanceTimelineQuery {
    fn default() -> Self {
        Self {
            event_type: None,
            from_tick: None,
            to_tick: None,
        }
    }
}

/// GET /api/v1/governance/summary — World governance summary.
async fn governance_summary(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let metrics = match &state.governance_metrics {
        Some(m) => m.clone(),
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(ErrorResponse { error: "governance metrics not configured".into() })).into_response(),
    };
    let metrics = metrics.lock().await;
    let summary = metrics.get_world_governance_summary();
    Json(summary).into_response()
}

/// GET /api/v1/governance/orgs/:org_id — Single org governance metrics.
async fn governance_org_metrics(
    State(state): State<AppState>,
    Path(org_id): Path<Uuid>,
) -> impl IntoResponse {
    let metrics = match &state.governance_metrics {
        Some(m) => m.clone(),
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(ErrorResponse { error: "governance metrics not configured".into() })).into_response(),
    };
    let metrics = metrics.lock().await;
    let m = metrics.get_org_metrics(org_id);
    // Check if org has any data (zeroed defaults means untracked)
    if m.election_count == 0 && m.tax_collection_count == 0 && m.treaties_signed == 0 && m.member_count == 0 {
        return (StatusCode::NOT_FOUND, Json(ErrorResponse { error: format!("no metrics for org {}", org_id) })).into_response();
    }
    Json(m).into_response()
}

/// GET /api/v1/governance/orgs/:org_id/timeline — Governance event timeline.
async fn governance_org_timeline(
    State(state): State<AppState>,
    Path(org_id): Path<Uuid>,
    Query(query): Query<GovernanceTimelineQuery>,
) -> impl IntoResponse {
    let metrics = match &state.governance_metrics {
        Some(m) => m.clone(),
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(ErrorResponse { error: "governance metrics not configured".into() })).into_response(),
    };
    let from = query.from_tick.unwrap_or(0);
    let to = query.to_tick.unwrap_or(u64::MAX);
    let metrics = metrics.lock().await;
    let timeline = metrics.get_timeline(org_id, query.event_type, (from, to));
    Json(timeline).into_response()
}

/// GET /api/v1/governance/comparison — Compare multiple orgs.
async fn governance_comparison(
    State(state): State<AppState>,
    Query(query): Query<GovernanceComparisonQuery>,
) -> impl IntoResponse {
    let org_ids: Vec<Uuid> = query
        .org_ids
        .unwrap_or_default()
        .split(',')
        .filter_map(|s| s.trim().parse::<Uuid>().ok())
        .collect();

    if org_ids.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse { error: "org_ids query parameter required (comma-separated UUIDs)".into() }),
        ).into_response();
    }

    let metrics = match &state.governance_metrics {
        Some(m) => m.clone(),
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(ErrorResponse { error: "governance metrics not configured".into() })).into_response(),
    };
    let metrics = metrics.lock().await;
    let comparison: Vec<_> = org_ids.iter().map(|id| metrics.get_org_metrics(*id)).collect();
    Json(comparison).into_response()
}

// ── Governance API Tests ──────────────────────────────────────────────────

#[cfg(test)]
mod governance_api_tests {
    use super::*;
    use axum::body::Body;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    /// Build a test AppState with governance_metrics wired up.
    fn build_test_state() -> (AppState, tempfile::TempDir) {
        let bus = Arc::new(EventBus::new(256));
        let collector = GovernanceMetricsCollector::new(&bus);
        // Allow background task to subscribe
        std::thread::sleep(std::time::Duration::from_millis(10));

        let (tick_tx, tick_rx) = watch::channel(0u64);
        let board = Arc::new(Mutex::new(TaskBoard::new()));
        let tmp = tempfile::tempdir().expect("tempdir");
        let wal = Arc::new(Mutex::new(WAL::new(tmp.path())));

        let state = AppState {
            board,
            wal,
            event_bus: bus,
            agents: Arc::new(Mutex::new(Vec::new())),
            messages: Arc::new(Mutex::new(Vec::new())),
            tick_tx,
            tick_rx,
            snapshot_store: None,
            marketplace: None,
            reputation_system: None,
            org_store: None,
            stock_market: None,
            governance: None,
            banking_system: None,
            trace_store: None,
            external_agents: Arc::new(Mutex::new(std::collections::HashMap::new())),
            governance_metrics: Some(Arc::new(Mutex::new(collector))),
        };
        (state, tmp)
    }

    /// Helper to extract JSON body from a response.
    async fn body_to_json(body: Body) -> serde_json::Value {
        let bytes = body
            .collect()
            .await
            .expect("failed to read body")
            .to_bytes();
        serde_json::from_slice(&bytes).expect("failed to parse JSON")
    }

    #[tokio::test]
    async fn test_governance_summary_returns_world_summary() {
        let (state, _tmp) = build_test_state();
        let bus = state.event_bus.clone();

        // Emit some governance events
        let org_a = Uuid::new_v4();
        let org_b = Uuid::new_v4();
        bus.emit(WorldEvent::TaxCollected {
            org_id: org_a.to_string(),
            payer_id: "p1".into(),
            tax_kind: "IncomeTax".into(),
            rate: 0.1,
            gross_amount: 1000,
            tax_amount: 100,
            tick: 5,
        });
        bus.emit(WorldEvent::TreatySigned {
            treaty_id: "t-1".into(),
            org_a: org_a.to_string(),
            org_b: org_b.to_string(),
        });

        tokio::time::sleep(std::time::Duration::from_millis(30)).await;

        let app = build_full_router(state);
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/api/v1/governance/summary")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let json = body_to_json(response.into_body()).await;
        assert_eq!(json["total_orgs"], 2);
        assert_eq!(json["total_tax_collected"], 100);
        assert_eq!(json["total_treaties"], 2); // Each org counts as 1 signing
    }

    #[tokio::test]
    async fn test_governance_org_metrics_returns_per_org_data() {
        let (state, _tmp) = build_test_state();
        let bus = state.event_bus.clone();
        let org_id = Uuid::new_v4();

        bus.emit(WorldEvent::OrganizationMemberJoined {
            org_id,
            agent_id: "a1".into(),
            role: "Member".into(),
        });
        bus.emit(WorldEvent::TaxCollected {
            org_id: org_id.to_string(),
            payer_id: "a1".into(),
            tax_kind: "IncomeTax".into(),
            rate: 0.1,
            gross_amount: 500,
            tax_amount: 50,
            tick: 10,
        });

        tokio::time::sleep(std::time::Duration::from_millis(30)).await;

        let app = build_full_router(state);
        let uri = format!("/api/v1/governance/orgs/{}", org_id);
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri(&uri)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let json = body_to_json(response.into_body()).await;
        assert_eq!(json["org_id"], org_id.to_string());
        assert_eq!(json["total_tax_collected"], 50);
        assert_eq!(json["member_count"], 1);
        assert_eq!(json["tax_collection_count"], 1);
    }

    #[tokio::test]
    async fn test_governance_org_metrics_returns_404_for_unknown_org() {
        let (state, _tmp) = build_test_state();
        let unknown_id = Uuid::new_v4();

        let app = build_full_router(state);
        let uri = format!("/api/v1/governance/orgs/{}", unknown_id);
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri(&uri)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_governance_timeline_returns_filtered_events() {
        let (state, _tmp) = build_test_state();
        let bus = state.event_bus.clone();
        let org_id = Uuid::new_v4();

        bus.emit(WorldEvent::TaxCollected {
            org_id: org_id.to_string(),
            payer_id: "p1".into(),
            tax_kind: "IncomeTax".into(),
            rate: 0.1,
            gross_amount: 100,
            tax_amount: 10,
            tick: 5,
        });
        bus.emit(WorldEvent::TaxCollected {
            org_id: org_id.to_string(),
            payer_id: "p2".into(),
            tax_kind: "IncomeTax".into(),
            rate: 0.1,
            gross_amount: 200,
            tax_amount: 20,
            tick: 15,
        });
        bus.emit(WorldEvent::LeadershipElectionStarted {
            org_id,
            candidates: vec!["c1".into()],
            voting_method: "SimpleMajority".into(),
        });

        tokio::time::sleep(std::time::Duration::from_millis(30)).await;

        let app = build_full_router(state);

        // Query with tick range filter [0, 10]
        let uri = format!(
            "/api/v1/governance/orgs/{}/timeline?from_tick=0&to_tick=10",
            org_id
        );
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri(&uri)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let json = body_to_json(response.into_body()).await;
        let events = json.as_array().expect("expected array");
        // tick 5 TaxCollected + tick 0 LeadershipElectionStarted = 2 events
        assert_eq!(events.len(), 2);
    }

    #[tokio::test]
    async fn test_governance_comparison_returns_multiple_orgs() {
        let (state, _tmp) = build_test_state();
        let bus = state.event_bus.clone();
        let org_a = Uuid::new_v4();
        let org_b = Uuid::new_v4();

        // org_a: tax + election
        bus.emit(WorldEvent::TaxCollected {
            org_id: org_a.to_string(),
            payer_id: "p1".into(),
            tax_kind: "IncomeTax".into(),
            rate: 0.1,
            gross_amount: 100,
            tax_amount: 10,
            tick: 1,
        });
        bus.emit(WorldEvent::LeadershipElectionStarted {
            org_id: org_a,
            candidates: vec!["c1".into(), "c2".into()],
            voting_method: "SimpleMajority".into(),
        });

        // org_b: treaty only
        bus.emit(WorldEvent::TreatySigned {
            treaty_id: "t-1".into(),
            org_a: org_a.to_string(),
            org_b: org_b.to_string(),
        });

        tokio::time::sleep(std::time::Duration::from_millis(30)).await;

        let app = build_full_router(state);
        let uri = format!(
            "/api/v1/governance/comparison?org_ids={},{}",
            org_a, org_b
        );
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri(&uri)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let json = body_to_json(response.into_body()).await;
        let comparison = json.as_array().expect("expected array");
        assert_eq!(comparison.len(), 2);

        // org_a should have tax and election data
        let metrics_a = &comparison[0];
        assert_eq!(metrics_a["total_tax_collected"], 10);
        assert_eq!(metrics_a["election_count"], 1);

        // org_b should have treaty data
        let metrics_b = &comparison[1];
        assert_eq!(metrics_b["treaties_signed"], 1);
    }

    #[tokio::test]
    async fn test_governance_comparison_returns_400_without_org_ids() {
        let (state, _tmp) = build_test_state();
        let app = build_full_router(state);

        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/api/v1/governance/comparison")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_governance_summary_returns_503_when_not_configured() {
        let board = Arc::new(Mutex::new(TaskBoard::new()));
        let tmp = tempfile::tempdir().expect("tempdir");
        let wal = Arc::new(Mutex::new(WAL::new(tmp.path())));
        let bus = Arc::new(EventBus::new(256));
        let (tick_tx, tick_rx) = watch::channel(0u64);

        // State without governance_metrics
        let state = AppState {
            board,
            wal,
            event_bus: bus,
            agents: Arc::new(Mutex::new(Vec::new())),
            messages: Arc::new(Mutex::new(Vec::new())),
            tick_tx,
            tick_rx,
            snapshot_store: None,
            marketplace: None,
            reputation_system: None,
            org_store: None,
            stock_market: None,
            governance: None,
            banking_system: None,
            trace_store: None,
            external_agents: Arc::new(Mutex::new(std::collections::HashMap::new())),
            governance_metrics: None,
        };

        let app = build_full_router(state);
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/api/v1/governance/summary")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }
}
