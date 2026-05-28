use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::*,
    Json,
};
use serde::{Deserialize, Serialize};

use crate::api::{AppState, ErrorResponse};
use crate::organization::charter::{Charter, GovernanceModel, ProfitSharing};
use crate::organization::org::OrgType;

pub fn default_governance() -> String {
    "vote".to_string()
}
pub fn default_profit_sharing() -> String {
    "equal".to_string()
}

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
            members: org
                .members
                .iter()
                .map(|m| OrgMemberResponse {
                    agent_id: m.agent_id.clone(),
                    agent_name: m.agent_name.clone(),
                    role: format!("{:?}", m.role).to_lowercase(),
                    share: m.share,
                    joined_tick: m.joined_tick,
                })
                .collect(),
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
pub fn org_to_response(org: &crate::organization::governance::Organization) -> OrgResponse {
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

// ── Organization & Governance Handlers ────────────────────

pub async fn create_org(
    State(state): State<AppState>,
    Json(body): Json<CreateOrgRequest>,
) -> impl IntoResponse {
    let store = match &state.org_store {
        Some(s) => s.clone(),
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "organization system not configured".into(),
                }),
            )
                .into_response()
        }
    };

    let org_type = match body.org_type.as_str() {
        "company" => OrgType::Company,
        "guild" => OrgType::Guild,
        "alliance" => OrgType::Alliance,
        "university" => OrgType::University,
        other => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: format!("unknown org type: {}", other),
                }),
            )
                .into_response()
        }
    };

    let governance = match body.charter.governance.as_str() {
        "vote" => GovernanceModel::Vote,
        "dictator" => GovernanceModel::Dictator,
        "council" => GovernanceModel::Council,
        other => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: format!("unknown governance model: {}", other),
                }),
            )
                .into_response()
        }
    };

    let profit_sharing = match body.charter.profit_sharing.as_str() {
        "equal" => ProfitSharing::Equal,
        "proportional" => ProfitSharing::Proportional,
        "custom" => ProfitSharing::Custom,
        other => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: format!("unknown profit sharing mode: {}", other),
                }),
            )
                .into_response()
        }
    };

    let charter = Charter {
        purpose: body.charter.purpose,
        governance,
        profit_sharing,
        membership_fee: body.charter.membership_fee,
    };

    let founders: Vec<(String, String)> = body
        .founders
        .into_iter()
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

pub async fn list_orgs(State(state): State<AppState>) -> impl IntoResponse {
    let store = match &state.org_store {
        Some(s) => s.clone(),
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "organization system not configured".into(),
                }),
            )
                .into_response()
        }
    };

    let store = store.lock().await;
    let orgs: Vec<OrgResponse> = store.list().into_iter().map(OrgResponse::from).collect();
    Json(orgs).into_response()
}

pub async fn get_org(State(state): State<AppState>, Path(id): Path<String>) -> impl IntoResponse {
    let store = match &state.org_store {
        Some(s) => s.clone(),
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "organization system not configured".into(),
                }),
            )
                .into_response()
        }
    };

    let store = store.lock().await;
    match store.get(&id) {
        Some(org) => Json(OrgResponse::from(org)).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "organization not found".into(),
            }),
        )
            .into_response(),
    }
}

pub async fn join_org(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<JoinOrgRequest>,
) -> impl IntoResponse {
    let store = match &state.org_store {
        Some(s) => s.clone(),
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "organization system not configured".into(),
                }),
            )
                .into_response()
        }
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

pub async fn leave_org(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<LeaveOrgRequest>,
) -> impl IntoResponse {
    let store = match &state.org_store {
        Some(s) => s.clone(),
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "organization system not configured".into(),
                }),
            )
                .into_response()
        }
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

pub async fn dissolve_org(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<DissolveOrgRequest>,
) -> impl IntoResponse {
    let store = match &state.org_store {
        Some(s) => s.clone(),
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "organization system not configured".into(),
                }),
            )
                .into_response()
        }
    };

    let mut store = store.lock().await;

    // Verify requester is a founder/leader
    let org = store.get(&id);
    match org {
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "organization not found".into(),
                }),
            )
                .into_response()
        }
        Some(org) => {
            let member = org.get_member(&body.requester_id);
            match member {
                None => {
                    return (
                        StatusCode::FORBIDDEN,
                        Json(ErrorResponse {
                            error: "requester is not a member".into(),
                        }),
                    )
                        .into_response()
                }
                Some(m) if !m.role.is_admin() => {
                    return (
                        StatusCode::FORBIDDEN,
                        Json(ErrorResponse {
                            error: "only founders or leaders can dissolve".into(),
                        }),
                    )
                        .into_response()
                }
                _ => {}
            }
        }
    }

    let reason = if body.reason.is_empty() {
        "manual_dissolution".to_string()
    } else {
        body.reason
    };
    match store.dissolve_org(&id, &reason) {
        Ok(()) => Json(serde_json::json!({ "dissolved": true, "org_id": id })).into_response(),
        Err(e) => {
            let status = match &e {
                crate::organization::org::OrgError::NotFound(_) => StatusCode::NOT_FOUND,
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

/// Organization CRUD routes.
pub fn org_routes() -> axum::Router<AppState> {
    axum::Router::new()
        .route("/api/v1/orgs", post(create_org))
        .route("/api/v1/orgs", get(list_orgs))
        .route("/api/v1/orgs/:id", get(get_org))
        .route("/api/v1/orgs/:id/join", post(join_org))
        .route("/api/v1/orgs/:id/leave", post(leave_org))
        .route("/api/v1/orgs/:id/dissolve", post(dissolve_org))
}
