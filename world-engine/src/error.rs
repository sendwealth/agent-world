//! Unified API error type for HTTP handlers.
//!
//! `AppError` replaces the manual `(StatusCode, Json(ErrorResponse { ... }))` boilerplate
//! that was repeated ~200 times across api.rs.  Handlers return `Result<impl IntoResponse, AppError>`
//! and use the `?` operator — the `From` impls below map each domain error to the correct
//! HTTP status automatically.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use serde::Serialize;

// ── JSON response envelope ──────────────────────────────────

#[derive(Debug, Serialize)]
struct ErrorBody {
    error: String,
    code: String,
}

// ── AppError ────────────────────────────────────────────────

#[derive(Debug)]
pub enum AppError {
    NotFound(String),
    BadRequest(String),
    Unauthorized(String),
    Forbidden(String),
    Conflict(String),
    Internal(String),
    ServiceUnavailable(String),
    UnprocessableEntity(String),
}

impl AppError {
    fn status_and_code(&self) -> (StatusCode, &'static str) {
        match self {
            AppError::NotFound(_) => (StatusCode::NOT_FOUND, "NOT_FOUND"),
            AppError::BadRequest(_) => (StatusCode::BAD_REQUEST, "BAD_REQUEST"),
            AppError::Unauthorized(_) => (StatusCode::UNAUTHORIZED, "UNAUTHORIZED"),
            AppError::Forbidden(_) => (StatusCode::FORBIDDEN, "FORBIDDEN"),
            AppError::Conflict(_) => (StatusCode::CONFLICT, "CONFLICT"),
            AppError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_SERVER_ERROR"),
            AppError::ServiceUnavailable(_) => {
                (StatusCode::SERVICE_UNAVAILABLE, "SERVICE_UNAVAILABLE")
            }
            AppError::UnprocessableEntity(_) => {
                (StatusCode::UNPROCESSABLE_ENTITY, "UNPROCESSABLE_ENTITY")
            }
        }
    }

    fn message(&self) -> &str {
        match self {
            AppError::NotFound(m)
            | AppError::BadRequest(m)
            | AppError::Unauthorized(m)
            | AppError::Forbidden(m)
            | AppError::Conflict(m)
            | AppError::Internal(m)
            | AppError::ServiceUnavailable(m)
            | AppError::UnprocessableEntity(m) => m,
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code) = self.status_and_code();
        let body = ErrorBody {
            error: self.message().to_owned(),
            code: code.to_owned(),
        };
        (status, Json(body)).into_response()
    }
}

// ── From<TaskError> ─────────────────────────────────────────

impl From<crate::economy::task::TaskError> for AppError {
    fn from(e: crate::economy::task::TaskError) -> Self {
        use crate::economy::task::TaskError;
        match &e {
            TaskError::NotFound(_) => AppError::NotFound(e.to_string()),
            TaskError::InvalidTransition { .. } | TaskError::AlreadyClaimed | TaskError::Expired => {
                AppError::Conflict(e.to_string())
            }
            TaskError::NotPublisher { .. } => AppError::Forbidden(e.to_string()),
            TaskError::NoAssignee | TaskError::ResultRequired => {
                AppError::BadRequest(e.to_string())
            }
        }
    }
}

// ── From<StockMarketError> ──────────────────────────────────

impl From<crate::economy::stock_market::StockMarketError> for AppError {
    fn from(e: crate::economy::stock_market::StockMarketError) -> Self {
        use crate::economy::stock_market::StockMarketError;
        match &e {
            StockMarketError::StockNotFound(_)
            | StockMarketError::OrderNotFound(_)
            | StockMarketError::OrgNotFound(_) => AppError::NotFound(e.to_string()),
            StockMarketError::NotListed
            | StockMarketError::Delisted
            | StockMarketError::OrderNotActive
            | StockMarketError::TickerTaken(_)
            | StockMarketError::AlreadyListed(_) => AppError::Conflict(e.to_string()),
            StockMarketError::InsufficientShares(_, _)
            | StockMarketError::InsufficientFunds(_, _)
            | StockMarketError::NotShareholder
            | StockMarketError::IpoConditionsNotMet(_)
            | StockMarketError::EmptyTicker
            | StockMarketError::InvalidShareCount
            | StockMarketError::InvalidPrice
            | StockMarketError::InvalidQuantity
            | StockMarketError::NoSharesIssued(_)
            | StockMarketError::NoProfitToDistribute => AppError::BadRequest(e.to_string()),
            StockMarketError::Internal(_) => AppError::Internal(e.to_string()),
        }
    }
}

// ── From<OrgError> ──────────────────────────────────────────

impl From<crate::organization::org::OrgError> for AppError {
    fn from(e: crate::organization::org::OrgError) -> Self {
        use crate::organization::org::OrgError;
        match &e {
            OrgError::NotFound(_) => AppError::NotFound(e.to_string()),
            OrgError::AgentAlreadyInOrg(_) | OrgError::OrgDissolved | OrgError::OrgInactive => {
                AppError::Conflict(e.to_string())
            }
            OrgError::NotEnoughFounders
            | OrgError::CharterRequired
            | OrgError::EmptyName
            | OrgError::InsufficientCreationFunds
            | OrgError::Member(_) => AppError::BadRequest(e.to_string()),
        }
    }
}

// ── From<BankingError> ──────────────────────────────────────

impl From<crate::economy::banking::BankingError> for AppError {
    fn from(e: crate::economy::banking::BankingError) -> Self {
        use crate::economy::banking::BankingError;
        match &e {
            BankingError::AccountNotFound(_) | BankingError::LoanNotFound(_) => {
                AppError::NotFound(e.to_string())
            }
            BankingError::DuplicateAccount(_) | BankingError::DuplicateAccountType { .. } => {
                AppError::Conflict(e.to_string())
            }
            BankingError::NoBankAccount { .. } => AppError::NotFound(e.to_string()),
            BankingError::InsufficientFunds { .. }
            | BankingError::InvalidLoanStatus { .. }
            | BankingError::InsufficientCollateral { .. }
            | BankingError::LoanAmountExceedsMax { .. }
            | BankingError::Ledger(_) => AppError::BadRequest(e.to_string()),
        }
    }
}

// ── From<InvestmentError> ───────────────────────────────────

impl From<crate::economy::investment::InvestmentError> for AppError {
    fn from(e: crate::economy::investment::InvestmentError) -> Self {
        use crate::economy::investment::InvestmentError;
        match &e {
            InvestmentError::ProductNotFound(_) | InvestmentError::PositionNotFound(_) => {
                AppError::NotFound(e.to_string())
            }
            InvestmentError::Unauthorized(_) => AppError::Forbidden(e.to_string()),
            InvestmentError::DuplicateIdempotencyKey(_) => AppError::Conflict(e.to_string()),
            InvestmentError::InvalidShareCount
            | InvestmentError::InvalidPrice
            | InvestmentError::InvalidTotalShares
            | InvestmentError::InvalidPerformanceScore => AppError::BadRequest(e.to_string()),
            InvestmentError::ProductNotActive
            | InvestmentError::ProductFrozen
            | InvestmentError::ProductClosed
            | InvestmentError::SelfInvestment
            | InvestmentError::InsufficientShares(_, _)
            | InvestmentError::InvestorShareLimit(_, _)
            | InvestmentError::TotalShareLimit(_, _)
            | InvestmentError::NoPosition
            | InvestmentError::NoProfitToDistribute
            | InvestmentError::NoShareholders
            | InvestmentError::ProductAlreadyExists(_) => {
                AppError::UnprocessableEntity(e.to_string())
            }
        }
    }
}

// ── From<FederationError> ───────────────────────────────────

impl From<crate::a2a::federation::FederationError> for AppError {
    fn from(e: crate::a2a::federation::FederationError) -> Self {
        use crate::a2a::federation::FederationError;
        match &e {
            FederationError::WorldNotFound(_) | FederationError::TreatyNotFound(_) => {
                AppError::NotFound(e.to_string())
            }
            FederationError::WorldAlreadyRegistered(_)
            | FederationError::TreatyAlreadyExists { .. }
            | FederationError::SanctionAlreadyActive(_) => AppError::Conflict(e.to_string()),
            FederationError::SelfAction
            | FederationError::InvalidTreatyStatus { .. }
            | FederationError::InvalidDiplomaticStatus { .. }
            | FederationError::RelationTooLow { .. }
            | FederationError::AtWar(_)
            | FederationError::NoActiveSanction(_)
            | FederationError::NoPeaceProposal(_) => AppError::BadRequest(e.to_string()),
        }
    }
}

// ── From<std::io::Error> ────────────────────────────────────

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::Internal(e.to_string())
    }
}

// ── From<AuthError> ─────────────────────────────────────────

impl From<crate::auth::extractors::AuthError> for AppError {
    fn from(e: crate::auth::extractors::AuthError) -> Self {
        use crate::auth::extractors::AuthError;
        match e {
            AuthError::MissingToken => AppError::Unauthorized("Authentication required".into()),
            AuthError::InvalidToken(msg) => AppError::Unauthorized(msg),
            AuthError::Forbidden(msg) => AppError::Forbidden(msg),
        }
    }
}

// ── From<GovernanceError> ──────────────────────────────────

impl From<crate::organization::governance::GovernanceError> for AppError {
    fn from(e: crate::organization::governance::GovernanceError) -> Self {
        use crate::organization::governance::GovernanceError;
        match &e {
            GovernanceError::NotFound(_)
            | GovernanceError::OrganizationNotFound(_)
            | GovernanceError::CannotRemoveFounder => AppError::NotFound(e.to_string()),
            GovernanceError::AlreadyMember { .. }
            | GovernanceError::OrganizationDissolved(_)
            | GovernanceError::InvalidTransition { .. }
            | GovernanceError::AlreadyVoted { .. } => AppError::Conflict(e.to_string()),
            GovernanceError::NotMember { .. }
            | GovernanceError::NotFounder { .. }
            | GovernanceError::VotingNotOpen(_)
            | GovernanceError::ProposalNotOpen(_)
            | GovernanceError::EmptyName
            | GovernanceError::DiscussionPeriodNotElapsed { .. } => {
                AppError::BadRequest(e.to_string())
            }
        }
    }
}

// ── From<RuleEngineError> ────────────────────────────────────

impl From<crate::organization::rule_engine::RuleEngineError> for AppError {
    fn from(e: crate::organization::rule_engine::RuleEngineError) -> Self {
        use crate::organization::rule_engine::RuleEngineError;
        match &e {
            RuleEngineError::NotFound(_) => AppError::NotFound(e.to_string()),
            RuleEngineError::AlreadyActive(_)
            | RuleEngineError::NotProposed(_)
            | RuleEngineError::AlreadyVoted { .. } => AppError::Conflict(e.to_string()),
            RuleEngineError::Expired(_) | RuleEngineError::Repealed(_) => {
                AppError::NotFound(e.to_string())
            }
        }
    }
}
