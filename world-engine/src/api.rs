use std::sync::Arc;

use axum::{
    Json,
    Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post},
};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::economy::task::{TaskBoard, Task};
use crate::economy::ledger::MoneyLedger;
use crate::wal::WAL;
use crate::world::enums::Currency;

// ── Shared State ──────────────────────────────────────────

pub type SharedTaskBoard = Arc<Mutex<TaskBoard>>;
pub type SharedWAL = Arc<Mutex<WAL>>;

/// Combined state for the API with WAL support.
#[derive(Clone)]
pub struct AppState {
    pub board: SharedTaskBoard,
    pub wal: SharedWAL,
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
    let state = AppState { board, wal };
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

// ── Ledger State & Router ────────────────────────────────

pub type SharedLedger = Arc<Mutex<MoneyLedger>>;

/// Create a router for the double-entry ledger API.
pub fn create_ledger_router(ledger: SharedLedger) -> Router {
    Router::new()
        .route("/ledger/balance/:account_id", get(ledger_balance))
        .route("/ledger/balance/:account_id/:currency", get(ledger_balance_currency))
        .route("/ledger/transfer", post(ledger_transfer))
        .route("/ledger/exchange/money-to-tokens", post(ledger_exchange_money_to_tokens))
        .route("/ledger/exchange/tokens-to-money", post(ledger_exchange_tokens_to_money))
        .route("/ledger/interest", post(ledger_pay_interest))
        .route("/ledger/entries", get(ledger_entries))
        .route("/ledger/audit", get(ledger_audit))
        .route("/ledger/exchange-rate", get(ledger_exchange_rate))
        .route("/ledger/verify", get(ledger_verify))
        .route("/ledger/supply", get(ledger_supply))
        .route("/ledger/accounts", post(ledger_create_account))
        .with_state(ledger)
}

// ── Ledger Request Types ─────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct LedgerTransferRequest {
    pub from: String,
    pub to: String,
    pub amount: u64,
    pub currency: String,
    pub tx_type: String,
    pub description: String,
    pub tick: u64,
    pub reference_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LedgerExchangeRequest {
    pub agent_id: String,
    pub amount: u64,
    pub tick: u64,
}

#[derive(Debug, Deserialize)]
pub struct LedgerInterestRequest {
    pub agent_id: String,
    pub rate: f64,
    pub tick: u64,
}

#[derive(Debug, Deserialize)]
pub struct LedgerCreateAccountRequest {
    pub id: String,
    pub name: String,
}

// ── Ledger Response Types ────────────────────────────────

#[derive(Debug, Serialize)]
pub struct BalanceResponse {
    pub account_id: String,
    pub balance: u64,
    pub currency: String,
}

#[derive(Debug, Serialize)]
pub struct BalanceSheetResponse {
    pub account_id: String,
    pub balances: HashMapResponse,
}

#[derive(Debug, Serialize)]
pub struct HashMapResponse(Vec<(String, u64)>);

#[derive(Debug, Serialize)]
pub struct TransferResponse {
    pub debit_id: String,
    pub credit_id: String,
}

#[derive(Debug, Serialize)]
pub struct ExchangeResponse {
    pub pair_id: String,
    pub from_amount: u64,
    pub from_currency: String,
    pub to_amount: u64,
    pub to_currency: String,
}

#[derive(Debug, Serialize)]
pub struct InterestResponse {
    pub pair_id: String,
    pub principal: u64,
    pub interest: u64,
    pub new_balance: u64,
}

#[derive(Debug, Serialize)]
pub struct ExchangeRateResponse {
    pub tokens_per_money: u64,
}

#[derive(Debug, Serialize)]
pub struct VerifyResponse {
    pub balanced: bool,
}

#[derive(Debug, Serialize)]
pub struct SupplyResponse {
    pub money_supply: u64,
    pub token_supply: u64,
}

#[derive(Debug, Serialize)]
pub struct EntryResponse {
    pub id: String,
    pub pair_id: String,
    pub account_id: String,
    pub side: String,
    pub amount: u64,
    pub currency: String,
    pub tx_type: String,
    pub description: String,
    pub tick: u64,
    pub reference_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AuditResponse {
    pub id: String,
    pub tick: u64,
    pub operation: String,
    pub actor: String,
    pub details: serde_json::Value,
    pub entry_ids: Vec<String>,
}

// ── Ledger Handlers ──────────────────────────────────────

async fn ledger_balance(
    State(ledger): State<SharedLedger>,
    Path(account_id): Path<String>,
) -> impl IntoResponse {
    let ledger = ledger.lock().await;
    if !ledger.account_exists(&account_id) {
        return (StatusCode::NOT_FOUND, Json(ErrorResponse { error: "account not found".into() })).into_response();
    }
    let sheet = ledger.get_balance_sheet(&account_id);
    let balances: Vec<(String, u64)> = sheet.balances.into_iter()
        .map(|(k, v)| (format!("{:?}", k).to_lowercase(), v))
        .collect();
    Json(BalanceSheetResponse {
        account_id: sheet.account_id,
        balances: HashMapResponse(balances),
    }).into_response()
}

async fn ledger_balance_currency(
    State(ledger): State<SharedLedger>,
    Path((account_id, currency)): Path<(String, String)>,
) -> impl IntoResponse {
    let ledger = ledger.lock().await;
    let curr = match currency.as_str() {
        "money" => Currency::Money,
        "token" => Currency::Token,
        _ => return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "currency must be 'money' or 'token'".into() })).into_response(),
    };
    if !ledger.account_exists(&account_id) {
        return (StatusCode::NOT_FOUND, Json(ErrorResponse { error: "account not found".into() })).into_response();
    }
    let balance = ledger.get_balance(&account_id, curr);
    Json(BalanceResponse {
        account_id,
        balance,
        currency,
    }).into_response()
}

async fn ledger_transfer(
    State(ledger): State<SharedLedger>,
    Json(body): Json<LedgerTransferRequest>,
) -> impl IntoResponse {
    let currency = match body.currency.as_str() {
        "money" => Currency::Money,
        "token" => Currency::Token,
        _ => return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "currency must be 'money' or 'token'".into() })).into_response(),
    };
    let tx_type = match body.tx_type.as_str() {
        "task_reward" => crate::economy::reward::TransactionType::TaskReward,
        "platform_fee" => crate::economy::reward::TransactionType::PlatformFee,
        "escrow_refund" => crate::economy::reward::TransactionType::EscrowRefund,
        "exchange" => crate::economy::reward::TransactionType::Exchange,
        "interest" => crate::economy::reward::TransactionType::Interest,
        "knowledge" => crate::economy::reward::TransactionType::Knowledge,
        "teach" => crate::economy::reward::TransactionType::Teach,
        _ => return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "invalid tx_type".into() })).into_response(),
    };

    let mut ledger = ledger.lock().await;
    match ledger.transfer(
        &body.from, &body.to, body.amount, currency,
        tx_type, body.description, body.tick, body.reference_id,
    ) {
        Ok((debit_id, credit_id)) => Json(TransferResponse {
            debit_id: debit_id.to_string(),
            credit_id: credit_id.to_string(),
        }).into_response(),
        Err(e) => {
            let status = match &e {
                crate::economy::ledger::LedgerError::InsufficientBalance { .. } => StatusCode::CONFLICT,
                crate::economy::ledger::LedgerError::AccountNotFound(_) => StatusCode::NOT_FOUND,
                _ => StatusCode::BAD_REQUEST,
            };
            (status, Json(ErrorResponse { error: e.to_string() })).into_response()
        }
    }
}

async fn ledger_exchange_money_to_tokens(
    State(ledger): State<SharedLedger>,
    Json(body): Json<LedgerExchangeRequest>,
) -> impl IntoResponse {
    let mut ledger = ledger.lock().await;
    match ledger.exchange_money_to_tokens(&body.agent_id, body.amount, body.tick) {
        Ok(result) => Json(ExchangeResponse {
            pair_id: result.pair_id.to_string(),
            from_amount: result.from_amount,
            from_currency: format!("{:?}", result.from_currency).to_lowercase(),
            to_amount: result.to_amount,
            to_currency: format!("{:?}", result.to_currency).to_lowercase(),
        }).into_response(),
        Err(e) => {
            let status = match &e {
                crate::economy::ledger::LedgerError::InsufficientBalance { .. } => StatusCode::CONFLICT,
                crate::economy::ledger::LedgerError::AccountNotFound(_) => StatusCode::NOT_FOUND,
                _ => StatusCode::BAD_REQUEST,
            };
            (status, Json(ErrorResponse { error: e.to_string() })).into_response()
        }
    }
}

async fn ledger_exchange_tokens_to_money(
    State(ledger): State<SharedLedger>,
    Json(body): Json<LedgerExchangeRequest>,
) -> impl IntoResponse {
    let mut ledger = ledger.lock().await;
    match ledger.exchange_tokens_to_money(&body.agent_id, body.amount, body.tick) {
        Ok(result) => Json(ExchangeResponse {
            pair_id: result.pair_id.to_string(),
            from_amount: result.from_amount,
            from_currency: format!("{:?}", result.from_currency).to_lowercase(),
            to_amount: result.to_amount,
            to_currency: format!("{:?}", result.to_currency).to_lowercase(),
        }).into_response(),
        Err(e) => {
            let status = match &e {
                crate::economy::ledger::LedgerError::InsufficientBalance { .. } => StatusCode::CONFLICT,
                crate::economy::ledger::LedgerError::AccountNotFound(_) => StatusCode::NOT_FOUND,
                _ => StatusCode::BAD_REQUEST,
            };
            (status, Json(ErrorResponse { error: e.to_string() })).into_response()
        }
    }
}

async fn ledger_pay_interest(
    State(ledger): State<SharedLedger>,
    Json(body): Json<LedgerInterestRequest>,
) -> impl IntoResponse {
    let mut ledger = ledger.lock().await;
    match ledger.pay_interest(&body.agent_id, body.rate, body.tick) {
        Ok(Some(result)) => Json(InterestResponse {
            pair_id: result.pair_id.to_string(),
            principal: result.principal,
            interest: result.interest,
            new_balance: result.new_balance,
        }).into_response(),
        Ok(None) => Json(serde_json::json!({ "result": "no_interest", "reason": "zero balance or truncated to zero" })).into_response(),
        Err(e) => {
            let status = match &e {
                crate::economy::ledger::LedgerError::AccountNotFound(_) => StatusCode::NOT_FOUND,
                _ => StatusCode::BAD_REQUEST,
            };
            (status, Json(ErrorResponse { error: e.to_string() })).into_response()
        }
    }
}

async fn ledger_entries(
    State(ledger): State<SharedLedger>,
) -> impl IntoResponse {
    let ledger = ledger.lock().await;
    let entries: Vec<EntryResponse> = ledger.list_entries().iter().map(|e| EntryResponse {
        id: e.id.to_string(),
        pair_id: e.pair_id.to_string(),
        account_id: e.account_id.clone(),
        side: format!("{:?}", e.side).to_lowercase(),
        amount: e.amount,
        currency: format!("{:?}", e.currency).to_lowercase(),
        tx_type: format!("{:?}", e.tx_type).to_lowercase(),
        description: e.description.clone(),
        tick: e.tick,
        reference_id: e.reference_id.clone(),
    }).collect();
    Json(entries).into_response()
}

async fn ledger_audit(
    State(ledger): State<SharedLedger>,
) -> impl IntoResponse {
    let ledger = ledger.lock().await;
    let records: Vec<AuditResponse> = ledger.audit_log().iter().map(|a| AuditResponse {
        id: a.id.to_string(),
        tick: a.tick,
        operation: a.operation.clone(),
        actor: a.actor.clone(),
        details: a.details.clone(),
        entry_ids: a.entry_ids.iter().map(|id| id.to_string()).collect(),
    }).collect();
    Json(records).into_response()
}

async fn ledger_exchange_rate(
    State(ledger): State<SharedLedger>,
) -> impl IntoResponse {
    let ledger = ledger.lock().await;
    Json(ExchangeRateResponse {
        tokens_per_money: ledger.exchange_rate().tokens_per_money,
    })
}

async fn ledger_verify(
    State(ledger): State<SharedLedger>,
) -> impl IntoResponse {
    let ledger = ledger.lock().await;
    Json(VerifyResponse {
        balanced: ledger.verify_all(),
    })
}

async fn ledger_supply(
    State(ledger): State<SharedLedger>,
) -> impl IntoResponse {
    let ledger = ledger.lock().await;
    Json(SupplyResponse {
        money_supply: ledger.total_money_supply(),
        token_supply: ledger.total_token_supply(),
    })
}

async fn ledger_create_account(
    State(ledger): State<SharedLedger>,
    Json(body): Json<LedgerCreateAccountRequest>,
) -> impl IntoResponse {
    let mut ledger = ledger.lock().await;
    if ledger.account_exists(&body.id) {
        return (StatusCode::CONFLICT, Json(ErrorResponse { error: "account already exists".into() })).into_response();
    }
    ledger.create_account(crate::economy::ledger::Account::new_agent(&body.id, &body.name));
    StatusCode::CREATED.into_response()
}
