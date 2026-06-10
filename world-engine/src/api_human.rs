use std::collections::HashMap;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::*,
    Json,
};

use crate::agentworld::a2a::v1::{
    world_message::Payload, BountyPayload, OraclePayload, OracleType as ProtoOracleType,
    WorldMessage,
};
use crate::api::{AppState, ErrorResponse};
use crate::auth::{extractors::require_capability, Capability, RequireAuth};
use crate::human::store::{
    ClaimAgentRequest, ClaimBountyRequest, CompleteBountyRequest, CreateBountyRequest,
    InfluenceRankingsQuery, InvestRequest, ListBountiesQuery, ListOraclesQuery, OracleType,
    OracleResponseRequest, RechargeRequest, SendOracleRequest,
};
use crate::world::event::WorldEvent;

// ── Human Participation API Handlers ──────────────────────

pub async fn human_stats(State(state): State<AppState>) -> impl IntoResponse {
    let tick = *state.tick_rx.borrow();
    let mut store = state.human_store.lock().await;
    store.set_tick(tick);
    let stats = store.get_stats();
    Json(stats).into_response()
}

pub async fn human_list_claimed_agents(
    State(state): State<AppState>,
    RequireAuth(auth): RequireAuth,
    Query(_query): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    // SECURITY: Always use authenticated user ID, ignore query param
    let store = state.human_store.lock().await;
    let agents: Vec<crate::human::store::ClaimedAgent> = store.list_claimed_agents(&auth.user_id);
    Json(agents).into_response()
}

pub async fn human_claim_agent(
    State(state): State<AppState>,
    RequireAuth(auth): RequireAuth,
    Json(body): Json<ClaimAgentRequest>,
) -> impl IntoResponse {
    // Find the agent in the world state
    let agent = {
        let agents = state.agents.lock().await;
        match agents.iter().find(|a| a.id == body.agent_id) {
            Some(a) => (a.name.clone(), a.tokens, a.money, a.ticks_survived),
            None => {
                return (
                    StatusCode::NOT_FOUND,
                    Json(ErrorResponse {
                        error: "Agent not found".into(),
                    }),
                )
                    .into_response()
            }
        }
    };

    // SECURITY: Use authenticated user ID, ignore body.human_id
    let tick = *state.tick_rx.borrow();
    let mut store = state.human_store.lock().await;
    store.set_tick(tick);
    let skills_map: HashMap<String, u32> = HashMap::new();
    let claimed = store.claim_agent(
        &auth.user_id,
        &body.agent_id,
        &agent.0,
        agent.1,
        agent.2,
        0.0,
        skills_map,
        agent.3,
    );
    (StatusCode::CREATED, Json(claimed)).into_response()
}

pub async fn human_list_oracles(
    State(state): State<AppState>,
    Query(query): Query<ListOraclesQuery>,
) -> impl IntoResponse {
    let store = state.human_store.lock().await;
    let oracles: Vec<crate::human::store::Oracle> = store.list_oracles(&query);
    Json(oracles).into_response()
}

pub async fn human_send_oracle(
    State(state): State<AppState>,
    RequireAuth(auth): RequireAuth,
    Json(mut body): Json<SendOracleRequest>,
) -> impl IntoResponse {
    // SECURITY: Replace client-provided human_id with authenticated user ID
    body.human_id = auth.user_id.clone();
    if body.content.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Oracle content cannot be empty".into(),
            }),
        )
            .into_response();
    }
    if body.content.len() > 500 {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Oracle content exceeds 500 characters".into(),
            }),
        )
            .into_response();
    }
    if body.target_agent_id.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "target_agent_id is required".into(),
            }),
        )
            .into_response();
    }

    // Validate target_agent_id exists in the world
    {
        let agents = state.agents.lock().await;
        if !agents.iter().any(|a| a.id == body.target_agent_id) {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "Target agent not found".into(),
                }),
            )
                .into_response();
        }
    }

    let tick = *state.tick_rx.borrow();
    let mut store = state.human_store.lock().await;
    store.set_tick(tick);
    let oracle = store.send_oracle(body);

    // Push Oracle message to the agent via WorldMessageRouter
    if let Some(ref router) = state.world_msg_router {
        let oracle_type = match oracle.oracle_type {
            OracleType::Guidance => ProtoOracleType::Guidance as i32,
            OracleType::Warning => ProtoOracleType::Warning as i32,
            OracleType::Blessing => ProtoOracleType::Blessing as i32,
            OracleType::Curse => ProtoOracleType::Curse as i32,
        };
        let msg = WorldMessage {
            id: uuid::Uuid::new_v4().to_string(),
            payload: Some(Payload::Oracle(OraclePayload {
                oracle_id: oracle.id.clone(),
                oracle_type,
                content: oracle.content.clone(),
                from_human: true,
                human_id: oracle.human_id.clone(),
            })),
            timestamp: chrono::Utc::now().timestamp(),
        };
        let delivered = router.deliver(&oracle.target_agent_id, msg).await;

        if delivered {
            // Update oracle status: Pending → Delivered
            store.deliver_oracle(&oracle.id);
        }
    }

    // Emit OracleDelivered event for observability
    state.event_bus.emit(WorldEvent::OracleDelivered {
        oracle_id: oracle.id.clone(),
        agent_id: oracle.target_agent_id.clone(),
        content: oracle.content.clone(),
    });

    (StatusCode::CREATED, Json(oracle)).into_response()
}

pub async fn human_get_oracle(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let store = state.human_store.lock().await;
    match store.get_oracle(&id) {
        Some(oracle) => Json(oracle).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "Oracle not found".into(),
            }),
        )
            .into_response(),
    }
}

pub async fn human_oracle_response(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<OracleResponseRequest>,
) -> impl IntoResponse {
    if body.response.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Response content cannot be empty".into(),
            }),
        )
            .into_response();
    }
    let mut store = state.human_store.lock().await;
    match store.respond_to_oracle(&id, &body.agent_id, &body.response) {
        Some(oracle) => Json(oracle).into_response(),
        None => (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Oracle not found or cannot be responded to".into(),
            }),
        )
            .into_response(),
    }
}

pub async fn human_list_bounties(
    State(state): State<AppState>,
    Query(query): Query<ListBountiesQuery>,
) -> impl IntoResponse {
    let store = state.human_store.lock().await;
    let bounties: Vec<crate::human::store::Bounty> = store.list_bounties(&query);
    Json(bounties).into_response()
}

pub async fn human_create_bounty(
    State(state): State<AppState>,
    RequireAuth(auth): RequireAuth,
    Json(mut body): Json<CreateBountyRequest>,
) -> impl IntoResponse {
    // RBAC: require PublishTasks capability
    if let Err(e) = require_capability(&auth, Capability::PublishTasks) {
        return e.into_response();
    }
    // SECURITY: Replace client-provided human_id with authenticated user ID
    body.human_id = auth.user_id.clone();
    if body.title.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Bounty title cannot be empty".into(),
            }),
        )
            .into_response();
    }
    if body.reward == 0 {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Reward must be greater than 0".into(),
            }),
        )
            .into_response();
    }

    let tick = *state.tick_rx.borrow();
    let mut store = state.human_store.lock().await;
    store.set_tick(tick);
    let bounty = store.create_bounty(body);

    // Push Bounty message to agents via WorldMessageRouter
    if let Some(ref router) = state.world_msg_router {
        let msg = WorldMessage {
            id: uuid::Uuid::new_v4().to_string(),
            payload: Some(Payload::Bounty(BountyPayload {
                bounty_id: bounty.id.clone(),
                title: bounty.title.clone(),
                description: bounty.description.clone(),
                reward: bounty.reward,
                deadline_tick: bounty.expires_tick.unwrap_or(0) as i64,
                human_id: bounty.human_id.clone(),
            })),
            timestamp: chrono::Utc::now().timestamp(),
        };
        // If bounty targets a specific agent, deliver directly; otherwise broadcast
        if let Some(ref target_id) = bounty.target_agent_id {
            router.deliver(target_id, msg).await;
        } else {
            router.broadcast(msg).await;
        }
    }

    // Emit BountyPublished event for observability
    state.event_bus.emit(WorldEvent::BountyPublished {
        bounty_id: bounty.id.clone(),
        title: bounty.title.clone(),
        reward: bounty.reward,
    });

    (StatusCode::CREATED, Json(bounty)).into_response()
}

pub async fn human_get_bounty(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let store = state.human_store.lock().await;
    match store.get_bounty(&id) {
        Some(bounty) => Json(bounty).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "Bounty not found".into(),
            }),
        )
            .into_response(),
    }
}

/// Claim a bounty — agent-side endpoint (no RequireAuth since agents
/// authenticate differently). However, we log the claimant for audit.
pub async fn human_claim_bounty(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<ClaimBountyRequest>,
) -> impl IntoResponse {
    let mut store = state.human_store.lock().await;
    match store.claim_bounty(&id, &body.agent_id) {
        Some(bounty) => Json(bounty).into_response(),
        None => (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Bounty not available for claiming".into(),
            }),
        )
            .into_response(),
    }
}

/// Complete a bounty — agent-side endpoint.
pub async fn human_complete_bounty(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<CompleteBountyRequest>,
) -> impl IntoResponse {
    let mut store = state.human_store.lock().await;
    match store.complete_bounty(&id, &body.result) {
        Some(bounty) => Json(bounty).into_response(),
        None => (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Bounty cannot be completed".into(),
            }),
        )
            .into_response(),
    }
}

pub async fn human_cancel_bounty(
    State(state): State<AppState>,
    RequireAuth(auth): RequireAuth,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let mut store = state.human_store.lock().await;
    // SECURITY: Verify ownership — only the creator can cancel
    let bounty = match store.get_bounty(&id) {
        Some(b) => b,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "Bounty not found".into(),
                }),
            )
                .into_response()
        }
    };
    if bounty.human_id != auth.user_id {
        return (
            StatusCode::FORBIDDEN,
            Json(ErrorResponse {
                error: "Only the bounty creator can cancel".into(),
            }),
        )
            .into_response();
    }
    match store.cancel_bounty(&id) {
        Some(bounty) => Json(bounty).into_response(),
        None => (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Bounty cannot be cancelled".into(),
            }),
        )
            .into_response(),
    }
}

pub async fn human_get_portfolio(
    State(state): State<AppState>,
    Path(human_id): Path<String>,
) -> impl IntoResponse {
    let store = state.human_store.lock().await;
    match store.get_portfolio(&human_id) {
        Some(portfolio) => Json(portfolio).into_response(),
        None => {
            // Return empty portfolio
            let empty = crate::human::store::HumanPortfolio {
                human_id,
                total_assets: 0,
                total_invested: 0,
                total_pnl: 0,
                holdings: Vec::new(),
                history: Vec::new(),
            };
            Json(empty).into_response()
        }
    }
}

pub async fn human_invest(
    State(state): State<AppState>,
    RequireAuth(auth): RequireAuth,
    Json(body): Json<InvestRequest>,
) -> impl IntoResponse {
    // RBAC: require Invest capability
    if let Err(e) = require_capability(&auth, Capability::Invest) {
        return e.into_response();
    }
    if body.amount == 0 {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Investment amount must be greater than 0".into(),
            }),
        )
            .into_response();
    }

    // Find agent name
    let agent_name = {
        let agents = state.agents.lock().await;
        match agents.iter().find(|a| a.id == body.agent_id) {
            Some(a) => a.name.clone(),
            None => {
                return (
                    StatusCode::NOT_FOUND,
                    Json(ErrorResponse {
                        error: "Agent not found".into(),
                    }),
                )
                    .into_response()
            }
        }
    };

    // SECURITY: Use authenticated user ID, ignore body.human_id
    let tick = *state.tick_rx.borrow();
    let mut store = state.human_store.lock().await;
    store.set_tick(tick);
    let portfolio = store.invest(&auth.user_id, &body.agent_id, &agent_name, body.amount);
    Json(portfolio).into_response()
}

pub async fn human_rankings(
    State(state): State<AppState>,
    Query(query): Query<InfluenceRankingsQuery>,
) -> impl IntoResponse {
    let sort_by = query.sort_by.as_deref().unwrap_or("total_influence");
    let limit = query.limit.unwrap_or(50);
    let store = state.human_store.lock().await;
    let rankings: Vec<crate::human::store::HumanInfluenceEntry> =
        store.get_influence_rankings(sort_by, limit);
    Json(rankings).into_response()
}

pub async fn human_list_interventions(
    State(state): State<AppState>,
    Query(query): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let human_id = query.get("human_id").map(|s| s.as_str());
    let limit: usize = query
        .get("limit")
        .and_then(|s| s.parse().ok())
        .unwrap_or(50);
    let store = state.human_store.lock().await;
    let interventions: Vec<crate::human::store::HumanInterventionEvent> =
        store.list_interventions(human_id, limit);
    Json(interventions).into_response()
}

// ── Token Recharge ────────────────────────────────────────

/// Recharge tokens for an agent (Human → Agent credit).
/// Requires Human auth. The agent must exist in the world.
pub async fn human_recharge_agent(
    State(state): State<AppState>,
    RequireAuth(auth): RequireAuth,
    Path(agent_id): Path<String>,
    Json(body): Json<RechargeRequest>,
) -> impl IntoResponse {
    if body.amount == 0 {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Recharge amount must be greater than 0".into(),
            }),
        )
            .into_response();
    }

    // Verify the agent exists and add tokens
    {
        let mut agents = state.agents.lock().await;
        match agents.iter_mut().find(|a| a.id == agent_id) {
            Some(agent) => {
                agent.tokens += body.amount;
            }
            None => {
                return (
                    StatusCode::NOT_FOUND,
                    Json(ErrorResponse {
                        error: "Agent not found".into(),
                    }),
                )
                    .into_response()
            }
        }
    }

    let tick = *state.tick_rx.borrow();
    let mut store = state.human_store.lock().await;
    store.set_tick(tick);
    let recharge_id = store.recharge_agent(&agent_id, &auth.user_id, body.amount);

    let response = serde_json::json!({
        "id": recharge_id,
        "agent_id": agent_id,
        "human_id": auth.user_id,
        "amount": body.amount,
        "tick": tick,
    });

    (StatusCode::OK, Json(response)).into_response()
}

/// Get agent energy/token status (for Dashboard display).
pub async fn human_agent_energy(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
) -> impl IntoResponse {
    let agents = state.agents.lock().await;
    match agents.iter().find(|a| a.id == agent_id) {
        Some(agent) => {
            let response = serde_json::json!({
                "agent_id": agent.id,
                "name": agent.name,
                "tokens": agent.tokens,
                "money": agent.money,
                "phase": agent.phase,
                "alive": agent.alive,
                "ticks_survived": agent.ticks_survived,
            });
            Json(response).into_response()
        }
        None => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "Agent not found".into(),
            }),
        )
            .into_response(),
    }
}

/// Get recharge history for an agent.
pub async fn human_recharge_history(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
) -> impl IntoResponse {
    let store = state.human_store.lock().await;
    let history = store.get_recharge_history(&agent_id, 50);
    Json(history).into_response()
}

/// Human participation routes.
pub fn human_routes() -> axum::Router<AppState> {
    axum::Router::new()
        .route("/human/stats", get(human_stats))
        .route("/human/agents", get(human_list_claimed_agents))
        .route("/human/agents/claim", post(human_claim_agent))
        .route("/human/oracles", get(human_list_oracles))
        .route("/human/oracles", post(human_send_oracle))
        .route("/human/oracles/:id", get(human_get_oracle))
        .route(
            "/agents/:id/oracle-response",
            post(human_oracle_response),
        )
        .route("/human/bounties", get(human_list_bounties))
        .route("/human/bounties", post(human_create_bounty))
        .route("/human/bounties/:id", get(human_get_bounty))
        .route("/human/bounties/:id/claim", post(human_claim_bounty))
        .route(
            "/human/bounties/:id/complete",
            post(human_complete_bounty),
        )
        .route(
            "/human/bounties/:id/cancel",
            post(human_cancel_bounty),
        )
        .route(
            "/human/portfolio/:human_id",
            get(human_get_portfolio),
        )
        .route("/human/portfolio/invest", post(human_invest))
        .route("/human/rankings", get(human_rankings))
        .route("/human/interventions", get(human_list_interventions))
        // Token recharge endpoints
        .route("/human/agents/:id/recharge", post(human_recharge_agent))
        .route("/human/agents/:id/energy", get(human_agent_energy))
        .route("/human/agents/:id/recharge-history", get(human_recharge_history))
}
