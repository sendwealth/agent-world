use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::*,
    Json,
};
use serde::Deserialize;

use crate::api::AppState;

// ── Federation / Cross-World Diplomacy Types ──────────────

#[derive(Debug, Deserialize)]
pub struct FedRegisterWorldRequest {
    pub id: String,
    pub name: String,
    pub endpoint: String,
    #[serde(default)]
    pub tick: u64,
}

#[derive(Debug, Deserialize)]
pub struct FedEstablishRelationsRequest {
    pub world_id: String,
    #[serde(default)]
    pub tick: u64,
}

#[derive(Debug, Deserialize)]
pub struct FedProposeTreatyRequest {
    pub world_id: String,
    pub treaty_type: String,
    #[serde(default)]
    pub duration_ticks: Option<u64>,
    #[serde(default)]
    pub terms: String,
    #[serde(default)]
    pub tick: u64,
}

#[derive(Debug, Deserialize)]
pub struct FedSanctionRequest {
    pub world_id: String,
    pub reason: String,
    #[serde(default)]
    pub tick: u64,
}

#[derive(Debug, Deserialize)]
pub struct FedSeverTiesRequest {
    pub world_id: String,
    #[serde(default)]
    pub tick: u64,
}

#[derive(Debug, Deserialize)]
pub struct FedDeclareWarRequest {
    pub world_id: String,
    #[serde(default)]
    pub tick: u64,
}

#[derive(Debug, Deserialize)]
pub struct FedProposePeaceRequest {
    pub world_id: String,
    #[serde(default)]
    pub tick: u64,
}

#[derive(Debug, Deserialize)]
pub struct FedAcceptTreatyRequest {
    #[serde(default)]
    pub tick: u64,
}

#[derive(Debug, Deserialize)]
pub struct FedBreakTreatyRequest {
    #[serde(default)]
    pub tick: u64,
}

#[derive(Debug, Deserialize)]
pub struct FedRejectTreatyRequest {}

#[derive(Debug, Deserialize)]
pub struct FedListTreatiesQuery {
    pub world_id: Option<String>,
    pub status: Option<String>,
}

pub fn fed_error_to_status(e: &crate::a2a::federation::FederationError) -> StatusCode {
    match e {
        crate::a2a::federation::FederationError::WorldNotFound(_)
        | crate::a2a::federation::FederationError::TreatyNotFound(_) => StatusCode::NOT_FOUND,
        crate::a2a::federation::FederationError::WorldAlreadyRegistered(_) => StatusCode::CONFLICT,
        crate::a2a::federation::FederationError::TreatyAlreadyExists { .. }
        | crate::a2a::federation::FederationError::SanctionAlreadyActive(_) => StatusCode::CONFLICT,
        _ => StatusCode::BAD_REQUEST,
    }
}

pub fn parse_treaty_type(s: &str) -> Option<crate::a2a::federation::CrossWorldTreatyType> {
    match s {
        "non_aggression" => Some(crate::a2a::federation::CrossWorldTreatyType::NonAggression),
        "trade_pact" => Some(crate::a2a::federation::CrossWorldTreatyType::TradePact),
        "military_alliance" => Some(crate::a2a::federation::CrossWorldTreatyType::MilitaryAlliance),
        "research_exchange" => Some(crate::a2a::federation::CrossWorldTreatyType::ResearchExchange),
        "cultural_exchange" => Some(crate::a2a::federation::CrossWorldTreatyType::CulturalExchange),
        _ => None,
    }
}

pub fn parse_treaty_status(s: &str) -> Option<crate::a2a::federation::CrossWorldTreatyStatus> {
    match s {
        "proposed" => Some(crate::a2a::federation::CrossWorldTreatyStatus::Proposed),
        "active" => Some(crate::a2a::federation::CrossWorldTreatyStatus::Active),
        "rejected" => Some(crate::a2a::federation::CrossWorldTreatyStatus::Rejected),
        "broken" => Some(crate::a2a::federation::CrossWorldTreatyStatus::Broken),
        "expired" => Some(crate::a2a::federation::CrossWorldTreatyStatus::Expired),
        _ => None,
    }
}

macro_rules! require_federation {
    ($state:expr) => {
        match $state.federation {
            Some(ref f) => f,
            None => return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error": "federation system not available"}))).into_response(),
        }
    };
}

// ── Federation Endpoint Handlers ──────────────────────────

pub async fn fed_register_world(
    State(state): State<AppState>,
    Json(body): Json<FedRegisterWorldRequest>,
) -> impl IntoResponse {
    let fed = require_federation!(state);
    let mut fed = fed.lock().await;
    let id = body.id.clone();
    match fed.register_world(body.id, body.name, body.endpoint, body.tick) {
        Ok(()) => {
            match fed.list_worlds().into_iter().find(|w| w.id == id) {
                Some(world) => (StatusCode::CREATED, Json(serde_json::json!(world))).into_response(),
                None => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "world registered but not found on read-back"}))).into_response(),
            }
        }
        Err(e) => (
            fed_error_to_status(&e),
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

pub async fn fed_list_worlds(State(state): State<AppState>) -> impl IntoResponse {
    let fed = require_federation!(state);
    let fed = fed.lock().await;
    let worlds: Vec<_> = fed.list_worlds().into_iter().cloned().collect();
    (StatusCode::OK, Json(serde_json::json!(worlds))).into_response()
}

pub async fn fed_get_world(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let fed = require_federation!(state);
    let fed = fed.lock().await;
    match fed.get_world(&id) {
        Some(world) => (StatusCode::OK, Json(serde_json::json!(world))).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "world not found"})),
        )
            .into_response(),
    }
}

pub async fn fed_deregister_world(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let fed = require_federation!(state);
    let mut fed = fed.lock().await;
    match fed.deregister_world(&id) {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({"status": "deregistered"})),
        )
            .into_response(),
        Err(e) => (
            fed_error_to_status(&e),
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

pub async fn fed_establish_relations(
    State(state): State<AppState>,
    Json(body): Json<FedEstablishRelationsRequest>,
) -> impl IntoResponse {
    let fed = require_federation!(state);
    let mut fed = fed.lock().await;
    match fed.establish_relations(&body.world_id, body.tick) {
        Ok(()) => {
            match fed.get_world(&body.world_id) {
                Some(world) => (StatusCode::OK, Json(serde_json::json!(world))).into_response(),
                None => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "operation succeeded but world not found on read-back"}))).into_response(),
            }
        }
        Err(e) => (
            fed_error_to_status(&e),
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

pub async fn fed_list_treaties(
    State(state): State<AppState>,
    Query(query): Query<FedListTreatiesQuery>,
) -> impl IntoResponse {
    let fed = require_federation!(state);
    let fed = fed.lock().await;
    let status_filter = query.status.as_deref().and_then(parse_treaty_status);
    let treaties: Vec<_> = fed
        .list_treaties(query.world_id.as_deref(), status_filter)
        .into_iter()
        .cloned()
        .collect();
    (StatusCode::OK, Json(serde_json::json!(treaties))).into_response()
}

pub async fn fed_propose_treaty(
    State(state): State<AppState>,
    Json(body): Json<FedProposeTreatyRequest>,
) -> impl IntoResponse {
    let Some(treaty_type) = parse_treaty_type(&body.treaty_type) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "invalid treaty_type"})),
        )
            .into_response();
    };
    let fed = require_federation!(state);
    let mut fed = fed.lock().await;
    match fed.propose_treaty(
        &body.world_id,
        treaty_type,
        body.tick,
        body.duration_ticks,
        body.terms.clone(),
    ) {
        Ok(id) => {
            match fed.get_treaty(&id) {
                Some(treaty) => (StatusCode::CREATED, Json(serde_json::json!(treaty))).into_response(),
                None => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "treaty created but not found on read-back"}))).into_response(),
            }
        }
        Err(e) => (
            fed_error_to_status(&e),
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

pub async fn fed_get_treaty(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let fed = require_federation!(state);
    let fed = fed.lock().await;
    match fed.get_treaty(&id) {
        Some(treaty) => (StatusCode::OK, Json(serde_json::json!(treaty))).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "treaty not found"})),
        )
            .into_response(),
    }
}

pub async fn fed_accept_treaty(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<FedAcceptTreatyRequest>,
) -> impl IntoResponse {
    let fed = require_federation!(state);
    let mut fed = fed.lock().await;
    match fed.accept_treaty(&id, body.tick) {
        Ok(()) => {
            match fed.get_treaty(&id) {
                Some(treaty) => (StatusCode::OK, Json(serde_json::json!(treaty))).into_response(),
                None => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "operation succeeded but treaty not found on read-back"}))).into_response(),
            }
        }
        Err(e) => (
            fed_error_to_status(&e),
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

pub async fn fed_reject_treaty(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(_body): Json<FedRejectTreatyRequest>,
) -> impl IntoResponse {
    let fed = require_federation!(state);
    let mut fed = fed.lock().await;
    match fed.reject_treaty(&id) {
        Ok(()) => {
            match fed.get_treaty(&id) {
                Some(treaty) => (StatusCode::OK, Json(serde_json::json!(treaty))).into_response(),
                None => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "operation succeeded but treaty not found on read-back"}))).into_response(),
            }
        }
        Err(e) => (
            fed_error_to_status(&e),
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

pub async fn fed_break_treaty(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<FedBreakTreatyRequest>,
) -> impl IntoResponse {
    let fed = require_federation!(state);
    let mut fed = fed.lock().await;
    match fed.break_treaty(&id, body.tick) {
        Ok(()) => {
            match fed.get_treaty(&id) {
                Some(treaty) => (StatusCode::OK, Json(serde_json::json!(treaty))).into_response(),
                None => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "operation succeeded but treaty not found on read-back"}))).into_response(),
            }
        }
        Err(e) => (
            fed_error_to_status(&e),
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

pub async fn fed_impose_sanctions(
    State(state): State<AppState>,
    Json(body): Json<FedSanctionRequest>,
) -> impl IntoResponse {
    let fed = require_federation!(state);
    let mut fed = fed.lock().await;
    match fed.impose_sanctions(&body.world_id, body.reason.clone(), body.tick) {
        Ok(()) => {
            match fed.get_world(&body.world_id) {
                Some(world) => (StatusCode::OK, Json(serde_json::json!(world))).into_response(),
                None => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "operation succeeded but world not found on read-back"}))).into_response(),
            }
        }
        Err(e) => (
            fed_error_to_status(&e),
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

pub async fn fed_lift_sanctions(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let fed = require_federation!(state);
    let mut fed = fed.lock().await;
    match fed.lift_sanctions(&id) {
        Ok(status) => (
            StatusCode::OK,
            Json(serde_json::json!({"world_id": id, "diplomatic_status": status.to_string()})),
        )
            .into_response(),
        Err(e) => (
            fed_error_to_status(&e),
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

pub async fn fed_sever_ties(
    State(state): State<AppState>,
    Json(body): Json<FedSeverTiesRequest>,
) -> impl IntoResponse {
    let fed = require_federation!(state);
    let mut fed = fed.lock().await;
    match fed.sever_ties(&body.world_id, body.tick) {
        Ok(()) => {
            match fed.get_world(&body.world_id) {
                Some(world) => (StatusCode::OK, Json(serde_json::json!(world))).into_response(),
                None => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "operation succeeded but world not found on read-back"}))).into_response(),
            }
        }
        Err(e) => (
            fed_error_to_status(&e),
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

pub async fn fed_declare_war(
    State(state): State<AppState>,
    Json(body): Json<FedDeclareWarRequest>,
) -> impl IntoResponse {
    let fed = require_federation!(state);
    let mut fed = fed.lock().await;
    match fed.declare_war(&body.world_id, body.tick) {
        Ok(()) => {
            match fed.get_world(&body.world_id) {
                Some(world) => (StatusCode::OK, Json(serde_json::json!(world))).into_response(),
                None => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "operation succeeded but world not found on read-back"}))).into_response(),
            }
        }
        Err(e) => (
            fed_error_to_status(&e),
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

pub async fn fed_propose_peace(
    State(state): State<AppState>,
    Json(body): Json<FedProposePeaceRequest>,
) -> impl IntoResponse {
    let fed = require_federation!(state);
    let mut fed = fed.lock().await;
    match fed.propose_peace(&body.world_id, body.tick) {
        Ok(treaty_id) => (StatusCode::CREATED, Json(serde_json::json!({"treaty_id": treaty_id, "world_id": body.world_id, "status": "peace_proposed"}))).into_response(),
        Err(e) => (fed_error_to_status(&e), Json(serde_json::json!({"error": e.to_string()}))).into_response(),
    }
}

pub async fn fed_accept_peace(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let fed = require_federation!(state);
    let mut fed = fed.lock().await;
    match fed.accept_peace(&id, 0) {
        Ok(()) => {
            match fed.get_world(&id) {
                Some(world) => (StatusCode::OK, Json(serde_json::json!(world))).into_response(),
                None => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "operation succeeded but world not found on read-back"}))).into_response(),
            }
        }
        Err(e) => (
            fed_error_to_status(&e),
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

pub async fn fed_summary(State(state): State<AppState>) -> impl IntoResponse {
    let fed = require_federation!(state);
    let fed = fed.lock().await;
    let summary = fed.summary();
    (StatusCode::OK, Json(serde_json::json!(summary))).into_response()
}

/// Cross-world diplomacy routes.
pub fn diplomacy_routes() -> axum::Router<AppState> {
    axum::Router::new()
        .route("/federation/worlds", post(fed_register_world))
        .route("/federation/worlds", get(fed_list_worlds))
        .route("/federation/worlds/:id", get(fed_get_world))
        .route(
            "/federation/worlds/:id",
            delete(fed_deregister_world),
        )
        .route(
            "/federation/establish-relations",
            post(fed_establish_relations),
        )
        .route("/federation/treaties", get(fed_list_treaties))
        .route("/federation/treaties", post(fed_propose_treaty))
        .route("/federation/treaties/:id", get(fed_get_treaty))
        .route(
            "/federation/treaties/:id/accept",
            post(fed_accept_treaty),
        )
        .route(
            "/federation/treaties/:id/reject",
            post(fed_reject_treaty),
        )
        .route(
            "/federation/treaties/:id/break",
            post(fed_break_treaty),
        )
        .route("/federation/sanctions", post(fed_impose_sanctions))
        .route(
            "/federation/sanctions/:id/lift",
            post(fed_lift_sanctions),
        )
        .route("/federation/sever-ties", post(fed_sever_ties))
        .route("/federation/declare-war", post(fed_declare_war))
        .route("/federation/propose-peace", post(fed_propose_peace))
        .route(
            "/federation/accept-peace/:id",
            post(fed_accept_peace),
        )
        .route("/federation/summary", get(fed_summary))
}
