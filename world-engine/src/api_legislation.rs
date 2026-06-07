//! # Legislation Cycle API Routes
//!
//! REST API for the self-legislation cycle: election → proposal → vote → enactment.
//!
//! Routes:
//! - `POST   /legislation/cycles`                — Start a new legislation cycle
//! - `POST   /legislation/cycles/with-leader`     — Start cycle with pre-elected leader
//! - `GET    /legislation/cycles/:org_id`          — Get current cycle status
//! - `GET    /legislation/cycles/:org_id/rules`    — Get candidate rules
//! - `POST   /legislation/cycles/:org_id/rules`    — Submit a candidate rule
//! - `POST   /legislation/cycles/:org_id/voting`   — Start voting phase
//! - `POST   /legislation/cycles/:org_id/vote`     — Cast a vote
//! - `POST   /legislation/cycles/:org_id/tally`    — Tally and enact
//! - `GET    /legislation/cycles/active`           — List active cycles
//! - `GET    /legislation/cycles/completed`        — List completed cycles
//! - `GET    /legislation/cycles/:org_id/effects`  — Evaluate cycle effects
//! - `POST   /legislation/cycles/:org_id/repeal`   — Submit repeal proposal
//! - `POST   /legislation/cycles/full`             — Run full cycle (convenience)

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::api::{AppState, ErrorResponse};
use crate::organization::legislation_cycle::{
    CandidateRule, LegislationCycleError, LegislationCycleRecord,
};
use crate::organization::rule_engine::{RuleCondition, RuleEffect, RuleType};

// ── Request / Response Types ─────────────────────────────

#[derive(Debug, Deserialize)]
pub struct StartCycleRequest {
    pub org_id: Uuid,
    pub candidates: Vec<String>,
    #[serde(default)]
    pub reason: String,
}

#[derive(Debug, Deserialize)]
pub struct StartCycleWithLeaderRequest {
    pub org_id: Uuid,
    pub leader_id: String,
    #[serde(default)]
    pub reason: String,
}

#[derive(Debug, Deserialize)]
pub struct SubmitCandidateRuleRequest {
    pub proposer_id: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    pub rule_type: String,
    #[serde(default)]
    pub conditions: Vec<RuleCondition>,
    #[serde(default)]
    pub effects: Vec<RuleEffect>,
    pub expires_tick: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct CastLegislationVoteRequest {
    pub voter_id: String,
    pub in_favor: bool,
}

#[derive(Debug, Deserialize)]
pub struct SubmitRepealRequest {
    pub proposer_id: String,
    pub target_rule_id: String,
    #[serde(default)]
    pub reason: String,
}

#[derive(Debug, Deserialize)]
pub struct RunFullCycleRequest {
    pub org_id: Uuid,
    pub candidates: Vec<String>,
    pub member_votes: Vec<MemberVoteEntry>,
    pub candidate_rules: Vec<SubmitCandidateRuleRequest>,
    #[serde(default)]
    pub reason: String,
}

#[derive(Debug, Deserialize)]
pub struct MemberVoteEntry {
    pub voter_id: String,
    pub in_favor: bool,
}

#[derive(Debug, Serialize)]
pub struct CycleResponse {
    pub cycle_id: String,
    pub org_id: String,
    pub status: String,
    pub leader_id: Option<String>,
    pub candidate_rules_count: usize,
    pub governance_proposal_id: Option<String>,
    pub enacted_rule_ids: Vec<String>,
    pub started_at_tick: u64,
    pub completed_at_tick: Option<u64>,
    pub trigger_reason: String,
}

impl From<&LegislationCycleRecord> for CycleResponse {
    fn from(r: &LegislationCycleRecord) -> Self {
        CycleResponse {
            cycle_id: r.cycle_id.to_string(),
            org_id: r.org_id.to_string(),
            status: r.status.to_string(),
            leader_id: r.leader_id.clone(),
            candidate_rules_count: r.candidate_rules.len(),
            governance_proposal_id: r.governance_proposal_id.map(|id| id.to_string()),
            enacted_rule_ids: r.enacted_rule_ids.clone(),
            started_at_tick: r.started_at_tick,
            completed_at_tick: r.completed_at_tick,
            trigger_reason: r.trigger_reason.clone(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct StartCycleResultResponse {
    pub cycle_id: String,
}

#[derive(Debug, Serialize)]
pub struct EnactedRulesResponse {
    pub enacted_rule_ids: Vec<String>,
    pub status: String,
}

#[derive(Debug, Serialize)]
pub struct CandidateRulesResponse {
    pub rules: Vec<CandidateRule>,
}

#[derive(Debug, Serialize)]
pub struct CycleStatusResponse {
    pub org_id: String,
    pub status: String,
}

#[derive(Debug, Serialize)]
pub struct FullCycleResultResponse {
    pub cycle_id: String,
    pub enacted_rule_ids: Vec<String>,
}

fn parse_rule_type(s: &str) -> Option<RuleType> {
    match s.to_lowercase().as_str() {
        "tax" => Some(RuleType::Tax),
        "trade" => Some(RuleType::Trade),
        "behavior" => Some(RuleType::Behavior),
        "diplomacy" => Some(RuleType::Diplomacy),
        "custom" => Some(RuleType::Custom),
        _ => None,
    }
}

fn legislation_error_status(e: &LegislationCycleError) -> StatusCode {
    match e {
        LegislationCycleError::NoActiveCycle { .. } => StatusCode::NOT_FOUND,
        LegislationCycleError::CycleAlreadyActive { .. } => StatusCode::CONFLICT,
        LegislationCycleError::OrganizationNotFound(_) => StatusCode::NOT_FOUND,
        LegislationCycleError::NotALeader { .. } => StatusCode::FORBIDDEN,
        LegislationCycleError::NoLeaderElected { .. } => StatusCode::CONFLICT,
        LegislationCycleError::NoCandidateRules { .. } => StatusCode::BAD_REQUEST,
        LegislationCycleError::ProposalSubmissionFailed { .. } => StatusCode::BAD_REQUEST,
        LegislationCycleError::VotingFailed { .. } => StatusCode::CONFLICT,
        LegislationCycleError::GovernanceError(_) => StatusCode::BAD_REQUEST,
    }
}

// ── Handlers ─────────────────────────────────────────────

/// Start a new legislation cycle (triggers election phase).
pub async fn start_cycle(
    State(state): State<AppState>,
    Json(body): Json<StartCycleRequest>,
) -> impl IntoResponse {
    let engine = match &state.legislation_cycle_engine {
        Some(e) => e.clone(),
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "legislation cycle engine not configured".into(),
                }),
            )
                .into_response()
        }
    };

    let tick = *state.tick_rx.borrow();
    let mut eng = engine.lock().await;

    match eng.start_cycle(body.org_id, body.candidates, tick, &body.reason) {
        Ok(cycle_id) => Json(StartCycleResultResponse {
            cycle_id: cycle_id.to_string(),
        })
        .into_response(),
        Err(e) => (
            legislation_error_status(&e),
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
            .into_response(),
    }
}

/// Start a cycle with a pre-elected leader (skips election phase).
pub async fn start_cycle_with_leader(
    State(state): State<AppState>,
    Json(body): Json<StartCycleWithLeaderRequest>,
) -> impl IntoResponse {
    let engine = match &state.legislation_cycle_engine {
        Some(e) => e.clone(),
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "legislation cycle engine not configured".into(),
                }),
            )
                .into_response()
        }
    };

    let tick = *state.tick_rx.borrow();
    let mut eng = engine.lock().await;

    match eng.start_cycle_with_leader(body.org_id, body.leader_id, tick, &body.reason) {
        Ok(cycle_id) => Json(StartCycleResultResponse {
            cycle_id: cycle_id.to_string(),
        })
        .into_response(),
        Err(e) => (
            legislation_error_status(&e),
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
            .into_response(),
    }
}

/// Get the current cycle for an organization.
pub async fn get_cycle(
    State(state): State<AppState>,
    Path(org_id): Path<String>,
) -> impl IntoResponse {
    let engine = match &state.legislation_cycle_engine {
        Some(e) => e.clone(),
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "legislation cycle engine not configured".into(),
                }),
            )
                .into_response()
        }
    };

    let Ok(uuid) = Uuid::parse_str(&org_id) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "invalid org id".into(),
            }),
        )
            .into_response();
    };

    let eng = engine.lock().await;
    match eng.get_cycle(uuid) {
        Some(record) => Json(CycleResponse::from(record)).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("no cycle found for org {}", org_id),
            }),
        )
            .into_response(),
    }
}

/// Get candidate rules for an organization's active legislation cycle.
pub async fn get_candidate_rules(
    State(state): State<AppState>,
    Path(org_id): Path<String>,
) -> impl IntoResponse {
    let engine = match &state.legislation_cycle_engine {
        Some(e) => e.clone(),
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "legislation cycle engine not configured".into(),
                }),
            )
                .into_response()
        }
    };

    let Ok(uuid) = Uuid::parse_str(&org_id) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "invalid org id".into(),
            }),
        )
            .into_response();
    };

    let eng = engine.lock().await;
    match eng.get_candidate_rules(uuid) {
        Ok(rules) => Json(CandidateRulesResponse {
            rules: rules.to_vec(),
        })
        .into_response(),
        Err(e) => (
            legislation_error_status(&e),
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
            .into_response(),
    }
}

/// Submit a candidate rule to the active legislation cycle.
pub async fn submit_candidate_rule(
    State(state): State<AppState>,
    Path(org_id): Path<String>,
    Json(body): Json<SubmitCandidateRuleRequest>,
) -> impl IntoResponse {
    let engine = match &state.legislation_cycle_engine {
        Some(e) => e.clone(),
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "legislation cycle engine not configured".into(),
                }),
            )
                .into_response()
        }
    };

    let Ok(uuid) = Uuid::parse_str(&org_id) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "invalid org id".into(),
            }),
        )
            .into_response();
    };

    let rule_type = match parse_rule_type(&body.rule_type) {
        Some(t) => t,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "invalid rule_type, must be: tax, trade, behavior, diplomacy, custom"
                        .into(),
                }),
            )
                .into_response()
        }
    };

    let rule = CandidateRule {
        proposer_id: body.proposer_id,
        title: body.title,
        description: body.description,
        rule_type,
        conditions: body.conditions,
        effects: body.effects,
        expires_tick: body.expires_tick,
    };

    let mut eng = engine.lock().await;
    match eng.submit_candidate_rule(uuid, rule) {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"status": "ok"}))).into_response(),
        Err(e) => (
            legislation_error_status(&e),
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
            .into_response(),
    }
}

/// Transition to the voting phase.
pub async fn start_voting_phase(
    State(state): State<AppState>,
    Path(org_id): Path<String>,
) -> impl IntoResponse {
    let engine = match &state.legislation_cycle_engine {
        Some(e) => e.clone(),
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "legislation cycle engine not configured".into(),
                }),
            )
                .into_response()
        }
    };
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

    let Ok(uuid) = Uuid::parse_str(&org_id) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "invalid org id".into(),
            }),
        )
            .into_response();
    };

    let tick = *state.tick_rx.borrow();
    let mut eng = engine.lock().await;
    let mut gov = governance.lock().await;

    match eng.start_voting_phase(&mut gov, uuid, tick) {
        Ok(proposal_id) => {
            Json(serde_json::json!({
                "status": "voting_open",
                "proposal_id": proposal_id.to_string(),
            }))
            .into_response()
        }
        Err(e) => (
            legislation_error_status(&e),
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
            .into_response(),
    }
}

/// Cast a vote on the active legislation proposal.
pub async fn cast_vote(
    State(state): State<AppState>,
    Path(org_id): Path<String>,
    Json(body): Json<CastLegislationVoteRequest>,
) -> impl IntoResponse {
    let engine = match &state.legislation_cycle_engine {
        Some(e) => e.clone(),
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "legislation cycle engine not configured".into(),
                }),
            )
                .into_response()
        }
    };
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

    let Ok(uuid) = Uuid::parse_str(&org_id) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "invalid org id".into(),
            }),
        )
            .into_response();
    };

    let tick = *state.tick_rx.borrow();
    let eng = engine.lock().await;
    let mut gov = governance.lock().await;

    match eng.cast_vote(&mut gov, uuid, body.voter_id, body.in_favor, tick) {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"status": "vote_recorded"})))
            .into_response(),
        Err(e) => (
            legislation_error_status(&e),
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
            .into_response(),
    }
}

/// Tally votes and enact passed rules.
pub async fn tally_and_enact(
    State(state): State<AppState>,
    Path(org_id): Path<String>,
) -> impl IntoResponse {
    let engine = match &state.legislation_cycle_engine {
        Some(e) => e.clone(),
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "legislation cycle engine not configured".into(),
                }),
            )
                .into_response()
        }
    };
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

    let Ok(uuid) = Uuid::parse_str(&org_id) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "invalid org id".into(),
            }),
        )
            .into_response();
    };

    let tick = *state.tick_rx.borrow();
    let mut eng = engine.lock().await;
    let mut gov = governance.lock().await;

    match eng.tally_and_enact(&mut gov, uuid, tick) {
        Ok(enacted) => {
            let status = if enacted.is_empty() {
                "rejected"
            } else {
                "enacted"
            };
            Json(EnactedRulesResponse {
                enacted_rule_ids: enacted,
                status: status.to_string(),
            })
            .into_response()
        }
        Err(e) => (
            legislation_error_status(&e),
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
            .into_response(),
    }
}

/// List all active cycles.
pub async fn list_active_cycles(State(state): State<AppState>) -> impl IntoResponse {
    let engine = match &state.legislation_cycle_engine {
        Some(e) => e.clone(),
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "legislation cycle engine not configured".into(),
                }),
            )
                .into_response()
        }
    };

    let eng = engine.lock().await;
    let cycles: Vec<CycleResponse> = eng.active_cycles().iter().map(|r| CycleResponse::from(*r)).collect();
    Json(cycles).into_response()
}

/// List all completed cycles.
pub async fn list_completed_cycles(State(state): State<AppState>) -> impl IntoResponse {
    let engine = match &state.legislation_cycle_engine {
        Some(e) => e.clone(),
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "legislation cycle engine not configured".into(),
                }),
            )
                .into_response()
        }
    };

    let eng = engine.lock().await;
    let cycles: Vec<CycleResponse> = eng.completed_cycles().iter().map(|r| CycleResponse::from(*r)).collect();
    Json(cycles).into_response()
}

/// Evaluate the effects of a completed cycle.
pub async fn evaluate_effects(
    State(state): State<AppState>,
    Path(org_id): Path<String>,
) -> impl IntoResponse {
    let engine = match &state.legislation_cycle_engine {
        Some(e) => e.clone(),
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "legislation cycle engine not configured".into(),
                }),
            )
                .into_response()
        }
    };
    let rule_engine = match &state.rule_engine {
        Some(re) => re.clone(),
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "rule engine not configured".into(),
                }),
            )
                .into_response()
        }
    };

    let Ok(uuid) = Uuid::parse_str(&org_id) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "invalid org id".into(),
            }),
        )
            .into_response();
    };

    let eng = engine.lock().await;
    let re = rule_engine.lock().await;
    let summary = eng.evaluate_cycle_effects(&re, uuid);
    Json(summary).into_response()
}

/// Submit a repeal proposal.
pub async fn submit_repeal_proposal(
    State(state): State<AppState>,
    Path(org_id): Path<String>,
    Json(body): Json<SubmitRepealRequest>,
) -> impl IntoResponse {
    let engine = match &state.legislation_cycle_engine {
        Some(e) => e.clone(),
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "legislation cycle engine not configured".into(),
                }),
            )
                .into_response()
        }
    };

    let Ok(uuid) = Uuid::parse_str(&org_id) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "invalid org id".into(),
            }),
        )
            .into_response();
    };

    let mut eng = engine.lock().await;
    match eng.submit_repeal_proposal(uuid, body.proposer_id, body.target_rule_id, body.reason) {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"status": "ok"}))).into_response(),
        Err(e) => (
            legislation_error_status(&e),
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
            .into_response(),
    }
}

/// Run the complete legislation cycle in one call (convenience endpoint).
pub async fn run_full_cycle(
    State(state): State<AppState>,
    Json(body): Json<RunFullCycleRequest>,
) -> impl IntoResponse {
    let engine = match &state.legislation_cycle_engine {
        Some(e) => e.clone(),
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "legislation cycle engine not configured".into(),
                }),
            )
                .into_response()
        }
    };
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

    let tick = *state.tick_rx.borrow();

    let candidate_rules: Vec<CandidateRule> = body
        .candidate_rules
        .into_iter()
        .map(|r| {
            let rule_type = parse_rule_type(&r.rule_type).unwrap_or(RuleType::Custom);
            CandidateRule {
                proposer_id: r.proposer_id,
                title: r.title,
                description: r.description,
                rule_type,
                conditions: r.conditions,
                effects: r.effects,
                expires_tick: r.expires_tick,
            }
        })
        .collect();

    let member_votes: Vec<(String, bool)> = body
        .member_votes
        .into_iter()
        .map(|v| (v.voter_id, v.in_favor))
        .collect();

    let mut leadership = crate::organization::leadership::LeadershipEngine::new();
    let mut eng = engine.lock().await;
    let mut gov = governance.lock().await;

    match eng.run_full_cycle(
        &mut gov,
        &mut leadership,
        body.org_id,
        body.candidates,
        &member_votes,
        candidate_rules,
        tick,
        &body.reason,
    ) {
        Ok((cycle_id, enacted)) => Json(FullCycleResultResponse {
            cycle_id: cycle_id.to_string(),
            enacted_rule_ids: enacted,
        })
        .into_response(),
        Err(e) => (
            legislation_error_status(&e),
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
            .into_response(),
    }
}

// ── Route Registration ───────────────────────────────────

pub fn legislation_routes() -> Router<AppState> {
    Router::new()
        .route("/legislation/cycles", post(start_cycle))
        .route("/legislation/cycles/with-leader", post(start_cycle_with_leader))
        .route("/legislation/cycles/full", post(run_full_cycle))
        .route("/legislation/cycles/active", get(list_active_cycles))
        .route("/legislation/cycles/completed", get(list_completed_cycles))
        .route("/legislation/cycles/:org_id", get(get_cycle))
        .route("/legislation/cycles/:org_id/rules", get(get_candidate_rules))
        .route("/legislation/cycles/:org_id/rules", post(submit_candidate_rule))
        .route("/legislation/cycles/:org_id/voting", post(start_voting_phase))
        .route("/legislation/cycles/:org_id/vote", post(cast_vote))
        .route("/legislation/cycles/:org_id/tally", post(tally_and_enact))
        .route("/legislation/cycles/:org_id/effects", get(evaluate_effects))
        .route("/legislation/cycles/:org_id/repeal", post(submit_repeal_proposal))
}
