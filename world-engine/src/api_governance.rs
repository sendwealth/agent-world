use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::*,
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::api::{AppState, ErrorResponse};

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
pub struct AddArgumentRequest {
    pub author_id: String,
    pub stance: String,
    pub content: String,
    #[serde(default)]
    pub parent_argument_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ArgumentResponse {
    pub id: String,
    pub proposal_id: String,
    pub author_id: String,
    pub stance: String,
    pub content: String,
    pub parent_argument_id: Option<String>,
    pub replies: Vec<String>,
    pub created_at: u64,
}

#[derive(Debug, Deserialize)]
pub struct DistributionRequest {
    pub total_profit: u64,
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

pub fn proposal_to_response(
    proposal: &crate::organization::governance::Proposal,
) -> ProposalResponse {
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
pub fn parse_decision_mode(s: &str) -> Option<crate::organization::governance::DecisionMode> {
    use crate::organization::governance::DecisionMode;
    match s {
        "vote" => Some(DecisionMode::Vote),
        "dictator" => Some(DecisionMode::Dictator),
        "council" => Some(DecisionMode::Council),
        _ => None,
    }
}

pub fn parse_proposal_type(s: &str) -> Option<crate::organization::governance::ProposalType> {
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

pub fn governance_error_status(e: &crate::organization::governance::GovernanceError) -> StatusCode {
    use crate::organization::governance::GovernanceError;
    match e {
        GovernanceError::NotFound(_) | GovernanceError::OrganizationNotFound(_) => {
            StatusCode::NOT_FOUND
        }
        GovernanceError::AlreadyMember { .. } | GovernanceError::AlreadyVoted { .. } => {
            StatusCode::CONFLICT
        }
        GovernanceError::NotMember { .. } | GovernanceError::NotFounder { .. } => {
            StatusCode::FORBIDDEN
        }
        GovernanceError::InvalidTransition { .. }
        | GovernanceError::VotingNotOpen(_)
        | GovernanceError::ProposalNotOpen(_) => StatusCode::CONFLICT,
        GovernanceError::OrganizationDissolved(_) => StatusCode::GONE,
        GovernanceError::CannotRemoveFounder => StatusCode::FORBIDDEN,
        GovernanceError::EmptyName => StatusCode::BAD_REQUEST,
        GovernanceError::DiscussionPeriodNotElapsed { .. } => StatusCode::CONFLICT,
    }
}

// ── Governance Handlers ─────────────────────────────────

pub async fn calculate_distribution(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<DistributionRequest>,
) -> impl IntoResponse {
    let governance = match &state.governance {
        Some(g) => g.clone(),
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "governance system not configured".into(),
                }),
            )
                .into_response()
        }
    };

    let Ok(uuid) = Uuid::parse_str(&id) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "invalid org id".into(),
            }),
        )
            .into_response();
    };

    let gov = governance.lock().await;
    match gov.get_org(uuid) {
        Some(org) => {
            let dist = org.calculate_distribution(body.total_profit);
            Json(dist).into_response()
        }
        None => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "organization not found".into(),
            }),
        )
            .into_response(),
    }
}

pub async fn create_proposal(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<CreateProposalRequest>,
) -> impl IntoResponse {
    let governance = match &state.governance {
        Some(g) => g.clone(),
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "governance system not configured".into(),
                }),
            )
                .into_response()
        }
    };

    let Ok(org_uuid) = Uuid::parse_str(&id) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "invalid org id".into(),
            }),
        )
            .into_response();
    };

    let proposal_type = match parse_proposal_type(&body.proposal_type) {
        Some(t) => t,
        None => return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "invalid proposal_type, must be: amend_charter, accept_member, expel_member, dissolve_org, change_profit_sharing".into() })).into_response(),
    };

    if body.title.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "title is required".into(),
            }),
        )
            .into_response();
    }

    let tick = *state.tick_rx.borrow();
    let mut gov = governance.lock().await;
    match gov.create_proposal(
        org_uuid,
        body.proposer_id,
        proposal_type,
        body.title,
        body.description,
        tick,
        body.payload,
    ) {
        Ok(proposal_id) => {
            let proposal = gov.get_proposal(proposal_id).unwrap();
            (StatusCode::CREATED, Json(proposal_to_response(proposal))).into_response()
        }
        Err(e) => (
            governance_error_status(&e),
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
            .into_response(),
    }
}

pub async fn list_proposals(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let governance = match &state.governance {
        Some(g) => g.clone(),
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "governance system not configured".into(),
                }),
            )
                .into_response()
        }
    };

    let Ok(org_uuid) = Uuid::parse_str(&id) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "invalid org id".into(),
            }),
        )
            .into_response();
    };

    let gov = governance.lock().await;
    let proposals: Vec<ProposalResponse> = gov
        .list_org_proposals(org_uuid)
        .into_iter()
        .map(proposal_to_response)
        .collect();
    Json(proposals).into_response()
}

pub async fn get_proposal(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let governance = match &state.governance {
        Some(g) => g.clone(),
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "governance system not configured".into(),
                }),
            )
                .into_response()
        }
    };

    let Ok(uuid) = Uuid::parse_str(&id) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "invalid proposal id".into(),
            }),
        )
            .into_response();
    };

    let gov = governance.lock().await;
    match gov.get_proposal(uuid) {
        Some(proposal) => Json(proposal_to_response(proposal)).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "proposal not found".into(),
            }),
        )
            .into_response(),
    }
}

pub async fn vote_proposal(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<VoteProposalRequest>,
) -> impl IntoResponse {
    let governance = match &state.governance {
        Some(g) => g.clone(),
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "governance system not configured".into(),
                }),
            )
                .into_response()
        }
    };

    let Ok(uuid) = Uuid::parse_str(&id) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "invalid proposal id".into(),
            }),
        )
            .into_response();
    };

    let tick = *state.tick_rx.borrow();
    let mut gov = governance.lock().await;
    match gov.vote(uuid, body.voter_id, body.in_favor, tick) {
        Ok(()) => {
            let proposal = gov.get_proposal(uuid).unwrap();
            Json(proposal_to_response(proposal)).into_response()
        }
        Err(e) => (
            governance_error_status(&e),
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
            .into_response(),
    }
}

pub async fn start_voting(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<StartVotingRequest>,
) -> impl IntoResponse {
    let governance = match &state.governance {
        Some(g) => g.clone(),
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "governance system not configured".into(),
                }),
            )
                .into_response()
        }
    };

    let Ok(uuid) = Uuid::parse_str(&id) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "invalid proposal id".into(),
            }),
        )
            .into_response();
    };

    let current_tick = *state.tick_rx.borrow();
    let mut gov = governance.lock().await;
    match gov.start_voting(uuid, &body.requester_id, current_tick) {
        Ok(()) => {
            let proposal = gov.get_proposal(uuid).unwrap();
            Json(proposal_to_response(proposal)).into_response()
        }
        Err(e) => (
            governance_error_status(&e),
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
            .into_response(),
    }
}

pub async fn tally_proposal(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let governance = match &state.governance {
        Some(g) => g.clone(),
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "governance system not configured".into(),
                }),
            )
                .into_response()
        }
    };

    let Ok(uuid) = Uuid::parse_str(&id) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "invalid proposal id".into(),
            }),
        )
            .into_response();
    };

    let mut gov = governance.lock().await;
    match gov.tally_proposal(uuid) {
        Ok(_status) => {
            let proposal = gov.get_proposal(uuid).unwrap();
            Json(proposal_to_response(proposal)).into_response()
        }
        Err(e) => (
            governance_error_status(&e),
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
            .into_response(),
    }
}

pub async fn cancel_proposal(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<CancelProposalRequest>,
) -> impl IntoResponse {
    let governance = match &state.governance {
        Some(g) => g.clone(),
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "governance system not configured".into(),
                }),
            )
                .into_response()
        }
    };

    let Ok(uuid) = Uuid::parse_str(&id) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "invalid proposal id".into(),
            }),
        )
            .into_response();
    };

    let mut gov = governance.lock().await;
    match gov.cancel_proposal(uuid, &body.requester_id) {
        Ok(()) => {
            let proposal = gov.get_proposal(uuid).unwrap();
            Json(proposal_to_response(proposal)).into_response()
        }
        Err(e) => (
            governance_error_status(&e),
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
            .into_response(),
    }
}

pub fn argument_to_response(
    arg: &crate::organization::governance::DebateArgument,
) -> ArgumentResponse {
    ArgumentResponse {
        id: arg.id.to_string(),
        proposal_id: arg.proposal_id.to_string(),
        author_id: arg.author_id.clone(),
        stance: arg.stance.to_string(),
        content: arg.content.clone(),
        parent_argument_id: arg.parent_argument_id.map(|id| id.to_string()),
        replies: arg.replies.iter().map(|id| id.to_string()).collect(),
        created_at: arg.created_at,
    }
}

pub async fn add_argument(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<AddArgumentRequest>,
) -> impl IntoResponse {
    let governance = match &state.governance {
        Some(g) => g.clone(),
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "governance system not configured".into(),
                }),
            )
                .into_response()
        }
    };

    let Ok(uuid) = Uuid::parse_str(&id) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "invalid proposal id".into(),
            }),
        )
            .into_response();
    };

    let stance = match body.stance.as_str() {
        "in_favor" => crate::organization::governance::DebateStance::InFavor,
        "against" => crate::organization::governance::DebateStance::Against,
        "neutral" => crate::organization::governance::DebateStance::Neutral,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "invalid stance: must be 'in_favor', 'against', or 'neutral'".into(),
                }),
            )
                .into_response()
        }
    };

    let current_tick = *state.tick_rx.borrow();
    let mut gov = governance.lock().await;

    let result = if let Some(parent_id_str) = body.parent_argument_id {
        let Ok(parent_uuid) = Uuid::parse_str(&parent_id_str) else {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "invalid parent_argument_id".into(),
                }),
            )
                .into_response();
        };
        gov.reply_to_argument(
            uuid,
            parent_uuid,
            body.author_id,
            stance,
            body.content,
            current_tick,
        )
    } else {
        gov.add_argument(uuid, body.author_id, stance, body.content, current_tick)
    };

    match result {
        Ok(arg_id) => {
            let proposal = gov.get_proposal(uuid).unwrap();
            let arg = proposal.arguments.iter().find(|a| a.id == arg_id).unwrap();
            Json(argument_to_response(arg)).into_response()
        }
        Err(e) => (
            governance_error_status(&e),
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
            .into_response(),
    }
}

pub async fn list_arguments(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let governance = match &state.governance {
        Some(g) => g.clone(),
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "governance system not configured".into(),
                }),
            )
                .into_response()
        }
    };

    let Ok(uuid) = Uuid::parse_str(&id) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "invalid proposal id".into(),
            }),
        )
            .into_response();
    };

    let gov = governance.lock().await;
    match gov.list_arguments(uuid) {
        Ok(args) => Json(
            args.into_iter()
                .map(argument_to_response)
                .collect::<Vec<_>>(),
        )
        .into_response(),
        Err(e) => (
            governance_error_status(&e),
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
            .into_response(),
    }
}

// ── Governance Metrics Handlers ──────────────────────────────

/// Query parameters for governance comparison endpoint.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct GovernanceComparisonQuery {
    pub org_ids: Option<String>,
}

/// Query parameters for governance timeline endpoint.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct GovernanceTimelineQuery {
    pub event_type: Option<crate::world::event::EventType>,
    pub from_tick: Option<u64>,
    pub to_tick: Option<u64>,
}

/// GET /api/v1/governance/summary — World governance summary.
pub async fn governance_summary(State(state): State<AppState>) -> impl IntoResponse {
    let metrics = match &state.governance_metrics {
        Some(m) => m.clone(),
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "governance metrics not configured".into(),
                }),
            )
                .into_response()
        }
    };
    let metrics = metrics.lock().await;
    let summary = metrics.get_world_governance_summary();
    Json(summary).into_response()
}

/// GET /api/v1/governance/orgs/:org_id — Single org governance metrics.
pub async fn governance_org_metrics(
    State(state): State<AppState>,
    Path(org_id): Path<Uuid>,
) -> impl IntoResponse {
    let metrics = match &state.governance_metrics {
        Some(m) => m.clone(),
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "governance metrics not configured".into(),
                }),
            )
                .into_response()
        }
    };
    let metrics = metrics.lock().await;
    let m = metrics.get_org_metrics(org_id);
    // Check if org has any data (zeroed defaults means untracked)
    if m.election_count == 0
        && m.tax_collection_count == 0
        && m.treaties_signed == 0
        && m.member_count == 0
    {
        return (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("no metrics for org {}", org_id),
            }),
        )
            .into_response();
    }
    Json(m).into_response()
}

/// GET /api/v1/governance/orgs/:org_id/timeline — Governance event timeline.
pub async fn governance_org_timeline(
    State(state): State<AppState>,
    Path(org_id): Path<Uuid>,
    Query(query): Query<GovernanceTimelineQuery>,
) -> impl IntoResponse {
    let metrics = match &state.governance_metrics {
        Some(m) => m.clone(),
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "governance metrics not configured".into(),
                }),
            )
                .into_response()
        }
    };
    let from = query.from_tick.unwrap_or(0);
    let to = query.to_tick.unwrap_or(u64::MAX);
    let metrics = metrics.lock().await;
    let timeline = metrics.get_timeline(org_id, query.event_type, (from, to));
    Json(timeline).into_response()
}

/// GET /api/v1/governance/comparison — Compare multiple orgs.
pub async fn governance_comparison(
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
            Json(ErrorResponse {
                error: "org_ids query parameter required (comma-separated UUIDs)".into(),
            }),
        )
            .into_response();
    }

    let metrics = match &state.governance_metrics {
        Some(m) => m.clone(),
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "governance metrics not configured".into(),
                }),
            )
                .into_response()
        }
    };
    let metrics = metrics.lock().await;
    let comparison: Vec<_> = org_ids
        .iter()
        .map(|id| metrics.get_org_metrics(*id))
        .collect();
    Json(comparison).into_response()
}

/// Governance + proposals + metrics routes.
pub fn governance_routes() -> axum::Router<AppState> {
    axum::Router::new()
        .route(
            "/api/v1/orgs/:id/distribution",
            post(calculate_distribution),
        )
        .route("/api/v1/orgs/:id/proposals", post(create_proposal))
        .route("/api/v1/orgs/:id/proposals", get(list_proposals))
        .route("/api/v1/proposals/:id", get(get_proposal))
        .route("/api/v1/proposals/:id/vote", post(vote_proposal))
        .route("/api/v1/proposals/:id/start-voting", post(start_voting))
        .route("/api/v1/proposals/:id/tally", post(tally_proposal))
        .route("/api/v1/proposals/:id/cancel", post(cancel_proposal))
        .route("/api/v1/proposals/:id/arguments", post(add_argument))
        .route("/api/v1/proposals/:id/arguments", get(list_arguments))
        .route("/api/v1/governance/summary", get(governance_summary))
        .route(
            "/api/v1/governance/orgs/:org_id",
            get(governance_org_metrics),
        )
        .route(
            "/api/v1/governance/orgs/:org_id/timeline",
            get(governance_org_timeline),
        )
        .route("/api/v1/governance/comparison", get(governance_comparison))
}
