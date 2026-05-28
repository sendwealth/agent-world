use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::*,
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::api::{AppState, ErrorResponse, SharedBankingSystem};
use crate::economy::banking::{BankAccountType, Collateral, Loan, LoanStatus};

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

pub fn parse_bank_account_type(s: &str) -> Option<BankAccountType> {
    match s {
        "savings" => Some(BankAccountType::Savings),
        "checking" => Some(BankAccountType::Checking),
        _ => None,
    }
}

pub fn parse_loan_status(s: &str) -> Option<LoanStatus> {
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

pub fn get_banking(
    state: &AppState,
) -> Result<SharedBankingSystem, (StatusCode, Json<ErrorResponse>)> {
    state.banking_system.clone().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse {
                error: "banking system not configured".into(),
            }),
        )
    })
}

pub async fn bank_open_account(
    State(state): State<AppState>,
    Json(body): Json<BankOpenAccountRequest>,
) -> impl IntoResponse {
    let banking = match get_banking(&state) {
        Ok(b) => b,
        Err(e) => return e.into_response(),
    };

    let account_type = match parse_bank_account_type(&body.account_type) {
        Some(t) => t,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "account_type must be 'savings' or 'checking'".into(),
                }),
            )
                .into_response()
        }
    };

    if body.owner_id.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "owner_id is required".into(),
            }),
        )
            .into_response();
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
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
            .into_response(),
    }
}

pub async fn bank_list_accounts(State(state): State<AppState>) -> impl IntoResponse {
    let banking = match get_banking(&state) {
        Ok(b) => b,
        Err(e) => return e.into_response(),
    };

    let banking = banking.lock().await;
    let accounts: Vec<BankAccountResponse> = banking
        .list_accounts()
        .into_iter()
        .map(|a| {
            let balance = banking.get_balance(a.id).unwrap_or(0);
            BankAccountResponse {
                id: a.id.to_string(),
                owner_id: a.owner_id.clone(),
                account_type: format!("{:?}", a.account_type).to_lowercase(),
                label: a.label.clone(),
                balance,
                created_tick: a.created_tick,
            }
        })
        .collect();
    Json(accounts).into_response()
}

pub async fn bank_get_account(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let banking = match get_banking(&state) {
        Ok(b) => b,
        Err(e) => return e.into_response(),
    };

    let Ok(uuid) = Uuid::parse_str(&id) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "invalid account id".into(),
            }),
        )
            .into_response();
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
        None => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "account not found".into(),
            }),
        )
            .into_response(),
    }
}

pub async fn bank_deposit(
    State(state): State<AppState>,
    Json(body): Json<BankDepositRequest>,
) -> impl IntoResponse {
    let banking = match get_banking(&state) {
        Ok(b) => b,
        Err(e) => return e.into_response(),
    };

    let Ok(uuid) = Uuid::parse_str(&body.account_id) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "invalid account id".into(),
            }),
        )
            .into_response();
    };

    let tick = *state.tick_rx.borrow();
    let mut banking = banking.lock().await;
    match banking.deposit(uuid, &body.owner_id, body.amount, tick) {
        Ok(result) => Json(serde_json::json!({
            "account_id": result.account_id,
            "amount": result.amount,
            "new_balance": result.new_balance,
        }))
        .into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
            .into_response(),
    }
}

pub async fn bank_withdraw(
    State(state): State<AppState>,
    Json(body): Json<BankWithdrawRequest>,
) -> impl IntoResponse {
    let banking = match get_banking(&state) {
        Ok(b) => b,
        Err(e) => return e.into_response(),
    };

    let Ok(uuid) = Uuid::parse_str(&body.account_id) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "invalid account id".into(),
            }),
        )
            .into_response();
    };

    let tick = *state.tick_rx.borrow();
    let mut banking = banking.lock().await;
    match banking.withdraw(uuid, &body.owner_id, body.amount, tick) {
        Ok(result) => Json(serde_json::json!({
            "account_id": result.account_id,
            "amount": result.amount,
            "new_balance": result.new_balance,
        }))
        .into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
            .into_response(),
    }
}

pub async fn bank_apply_loan(
    State(state): State<AppState>,
    Json(body): Json<BankApplyLoanRequest>,
) -> impl IntoResponse {
    let banking = match get_banking(&state) {
        Ok(b) => b,
        Err(e) => return e.into_response(),
    };

    if body.borrower_id.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "borrower_id is required".into(),
            }),
        )
            .into_response();
    }

    let tick = *state.tick_rx.borrow();
    let mut banking = banking.lock().await;
    match banking.apply_for_loan(
        &body.borrower_id,
        body.amount,
        body.term_ticks,
        body.collateral,
        tick,
    ) {
        Ok(result) => (
            StatusCode::CREATED,
            Json(serde_json::json!({
                "loan_id": result.loan_id.to_string(),
                "borrower_id": result.borrower_id,
                "principal": result.principal,
                "interest_rate": result.interest_rate,
                "term_ticks": result.term_ticks,
                "status": format!("{:?}", result.status).to_lowercase(),
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
            .into_response(),
    }
}

pub async fn bank_list_loans(
    State(state): State<AppState>,
    Query(query): Query<BankListLoansQuery>,
) -> impl IntoResponse {
    let banking = match get_banking(&state) {
        Ok(b) => b,
        Err(e) => return e.into_response(),
    };

    let status_filter = query.status.as_deref().and_then(parse_loan_status);
    let banking = banking.lock().await;
    let loans: Vec<LoanResponse> = banking
        .list_loans(query.borrower_id.as_deref(), status_filter)
        .into_iter()
        .map(LoanResponse::from)
        .collect();
    Json(loans).into_response()
}

pub async fn bank_get_loan(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let banking = match get_banking(&state) {
        Ok(b) => b,
        Err(e) => return e.into_response(),
    };

    let Ok(uuid) = Uuid::parse_str(&id) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "invalid loan id".into(),
            }),
        )
            .into_response();
    };

    let banking = banking.lock().await;
    match banking.get_loan(uuid) {
        Some(loan) => Json(LoanResponse::from(loan)).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "loan not found".into(),
            }),
        )
            .into_response(),
    }
}

pub async fn bank_approve_loan(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let banking = match get_banking(&state) {
        Ok(b) => b,
        Err(e) => return e.into_response(),
    };

    let Ok(uuid) = Uuid::parse_str(&id) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "invalid loan id".into(),
            }),
        )
            .into_response();
    };

    let tick = *state.tick_rx.borrow();
    let mut banking = banking.lock().await;
    match banking.approve_loan(uuid, tick) {
        Ok(loan) => Json(LoanResponse::from(&loan)).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
            .into_response(),
    }
}

pub async fn bank_disburse_loan(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let banking = match get_banking(&state) {
        Ok(b) => b,
        Err(e) => return e.into_response(),
    };

    let Ok(uuid) = Uuid::parse_str(&id) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "invalid loan id".into(),
            }),
        )
            .into_response();
    };

    let tick = *state.tick_rx.borrow();
    let mut banking = banking.lock().await;
    match banking.disburse_loan(uuid, tick) {
        Ok(loan) => Json(LoanResponse::from(&loan)).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
            .into_response(),
    }
}

pub async fn bank_repay_loan(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<BankRepayRequest>,
) -> impl IntoResponse {
    let banking = match get_banking(&state) {
        Ok(b) => b,
        Err(e) => return e.into_response(),
    };

    let Ok(uuid) = Uuid::parse_str(&id) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "invalid loan id".into(),
            }),
        )
            .into_response();
    };

    let tick = *state.tick_rx.borrow();
    let mut banking = banking.lock().await;
    match banking.repay_loan(uuid, body.amount, tick) {
        Ok(result) => Json(serde_json::json!({
            "loan_id": result.loan_id.to_string(),
            "amount_paid": result.amount_paid,
            "outstanding_balance": result.outstanding_balance,
            "fully_repaid": result.fully_repaid,
        }))
        .into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
            .into_response(),
    }
}

pub async fn bank_adjust_rates(
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
    }))
    .into_response()
}

pub async fn bank_mint_money(
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
    (
        StatusCode::CREATED,
        Json(serde_json::json!({
            "amount": result.amount,
            "total_money_supply": result.total_money_supply,
        })),
    )
        .into_response()
}

pub async fn bank_write_off(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let banking = match get_banking(&state) {
        Ok(b) => b,
        Err(e) => return e.into_response(),
    };

    let Ok(uuid) = Uuid::parse_str(&id) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "invalid loan id".into(),
            }),
        )
            .into_response();
    };

    let tick = *state.tick_rx.borrow();
    let mut banking = banking.lock().await;
    match banking.write_off_bad_debt(uuid, tick) {
        Ok(result) => Json(serde_json::json!({
            "loan_id": result.loan_id.to_string(),
            "amount_written_off": result.amount_written_off,
        }))
        .into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
            .into_response(),
    }
}

pub async fn bank_stats(State(state): State<AppState>) -> impl IntoResponse {
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
    }))
    .into_response()
}

/// Banking routes.
pub fn bank_routes() -> axum::Router<AppState> {
    axum::Router::new()
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
}
