use std::collections::HashMap;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::*,
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::agentworld::a2a::v1::{
    world_message::Payload, BountyPayload, OraclePayload, OracleType as ProtoOracleType,
    WorldMessage,
};
use crate::api::{AppState, ErrorResponse};
use crate::auth::{extractors::require_capability, Capability, RequireAuth};
use crate::human_agent::HumanActionType;
use crate::human_agent::{HumanAgent, QueuedAction};
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
        // ── Human-as-Agent (Phase 5.5) endpoints ──
        .route("/human/incarnate", post(human_incarnate))
        .route("/human/agents/:id/action", post(human_submit_action))
        .route("/human/agents/:id/state", get(human_agent_state))
        .route("/human/agents/list-human", get(human_list_all))
}

// ── Human-as-Agent (Phase 5.5) ───────────────────────────

/// Request body for POST /api/v1/human/incarnate.
#[derive(Debug, Deserialize)]
pub struct IncarnateRequest {
    /// Display name for the human-controlled agent.
    pub name: String,
}

/// Response body for incarnate.
#[derive(Debug, Serialize)]
pub struct IncarnateResponse {
    pub agent_id: String,
    pub name: String,
    pub tokens: u64,
    pub newbie_protection_ticks: u64,
    pub incarnated_tick: u64,
}

/// Request body for POST /api/v1/human/agents/:id/action.
#[derive(Debug, Deserialize)]
pub struct SubmitActionRequest {
    /// The action type (communicate, trade, rest, explore, gather, build, etc.)
    pub action: String,
    /// Free-form parameters for the action.
    #[serde(default)]
    pub params: serde_json::Value,
}

/// Response body for action submission.
#[derive(Debug, Serialize)]
pub struct SubmitActionResponse {
    pub agent_id: String,
    pub action: String,
    pub queued: bool,
    pub token_cost: u64,
    pub submitted_tick: u64,
}

/// POST /api/v1/human/incarnate
///
/// Registers a human as an agent in the world. Allocates initial token balance
/// and newbie protection. The agent is added to both AppState.agents and
/// WorldState.agents so it participates in the same survival rules as AI agents.
pub async fn human_incarnate(
    State(state): State<AppState>,
    RequireAuth(auth): RequireAuth,
    Json(body): Json<IncarnateRequest>,
) -> impl IntoResponse {
    if body.name.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "name is required".into(),
            }),
        )
            .into_response();
    }

    // Read human-agent config from genesis, falling back to defaults.
    let (initial_tokens, newbie_protection_ticks) = match &state.genesis_config {
        Some(cfg) => (
            cfg.human_agent.initial_tokens,
            cfg.human_agent.newbie_protection_ticks,
        ),
        None => (
            crate::config::HumanAgentConfig::default().initial_tokens,
            crate::config::HumanAgentConfig::default().newbie_protection_ticks,
        ),
    };

    let agent_id = Uuid::new_v4().to_string();
    let tick = *state.tick_rx.borrow();
    let name = body.name.clone();
    let created_at = chrono::Utc::now().to_rfc3339();

    // Register in the human agent registry
    {
        let mut reg = state.human_agent_registry.lock().await;
        if let Err(e) = reg.register(HumanAgent {
            agent_id: agent_id.clone(),
            human_id: auth.user_id.clone(),
            name: name.clone(),
            initial_tokens,
            initial_money: 0,
            spawned_tick: tick,
            last_action_tick: tick,
            alive: true,
            metadata: serde_json::json!({}),
        }) {
            return (
                StatusCode::CONFLICT,
                Json(ErrorResponse { error: e }),
            )
                .into_response();
        }
    }

    // Add to AppState.agents (shared agents list)
    {
        let mut agents = state.agents.lock().await;
        agents.push(crate::api::AgentDto {
            id: agent_id.clone(),
            name: name.clone(),
            phase: "adult".to_string(),
            tokens: initial_tokens,
            money: 0,
            alive: true,
            ticks_survived: 0,
            personality: String::new(),
            parent_ids: Vec::new(),
            generation: 0,
            skills: HashMap::new(),
            created_at,
        });
    }

    // Insert into WorldState.agents so survival subsystems process this agent
    if let Some(ref ws) = state.world_state {
        let agent_uuid = Uuid::parse_str(&agent_id).unwrap_or_else(|_| Uuid::new_v4());
        let mut ws_guard = ws.lock().await;
        ws_guard.agents.push((
            agent_uuid,
            tick,
            crate::world::agent::AgentRecord {
                id: agent_uuid,
                name: name.clone(),
                phase: crate::world::enums::AgentPhase::Adult,
                tokens: initial_tokens,
                skills: std::collections::HashMap::new(),
                personality: String::new(),
                tasks_completed: 0,
                tasks_attempted: 0,
            },
        ));
    }

    // Emit spawn event
    state.event_bus.emit(WorldEvent::AgentSpawned {
        agent_id: agent_id.clone(),
        name: name.clone(),
    });

    tracing::info!(
        agent_id = %agent_id,
        human_user_id = %auth.user_id,
        name = %name,
        "Human agent incarnated"
    );


    (
        StatusCode::CREATED,
        Json(IncarnateResponse {
            agent_id,
            name,
            tokens: initial_tokens,
            newbie_protection_ticks,
            incarnated_tick: tick,
        }),
    )
        .into_response()
}

/// POST /api/v1/human/agents/:id/action
///
/// Submit an action for a human-controlled agent. The action is queued and
/// will be executed at the start of the next tick (before AI decisions).
pub async fn human_submit_action(
    State(state): State<AppState>,
    RequireAuth(auth): RequireAuth,
    Path(agent_id): Path<String>,
    Json(body): Json<SubmitActionRequest>,
) -> impl IntoResponse {
    // Validate action type
    let action_type = match HumanActionType::from_str_lossy(&body.action) {
        Some(at) => at,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: format!("unknown action type '{}'", body.action),
                }),
            )
                .into_response();
        }
    };

    // Verify agent is a registered human agent
    let agent = {
        let reg = state.human_agent_registry.lock().await;
        match reg.get_by_agent(&agent_id) {
            Some(a) => a.clone(),
            None => {
                return (
                    StatusCode::NOT_FOUND,
                    Json(ErrorResponse {
                        error: "agent not found or not a human-controlled agent".into(),
                    }),
                )
                    .into_response();
            }
        }
    };

    // Security: verify the authenticated user owns this agent
    if agent.human_id != auth.user_id {
        return (
            StatusCode::FORBIDDEN,
            Json(ErrorResponse {
                error: "you do not control this agent".into(),
            }),
        )
            .into_response();
    }

    // Check agent is alive
    {
        let agents = state.agents.lock().await;
        match agents.iter().find(|a| a.id == agent_id) {
            Some(a) if a.alive => {}
            Some(_) => {
                return (
                    StatusCode::GONE,
                    Json(ErrorResponse {
                        error: "agent is dead".into(),
                    }),
                )
                    .into_response();
            }
            None => {
                return (
                    StatusCode::NOT_FOUND,
                    Json(ErrorResponse {
                        error: "agent not found".into(),
                    }),
                )
                    .into_response();
            }
        }
    }

    let tick = *state.tick_rx.borrow();
    let token_cost = action_type.token_cost();

    // Enqueue the action
    {
        let mut q = state.human_action_queue.lock().await;
        if let Err(e) = q.enqueue(QueuedAction {
            id: String::new(),
            agent_id: agent_id.clone(),
            action: action_type.as_str().to_string(),
            params: body.params,
            enqueued_tick: tick,
            applied: false,
        }) {
            return (
                StatusCode::TOO_MANY_REQUESTS,
                Json(ErrorResponse { error: e }),
            )
                .into_response();
        }
    }

    (
        StatusCode::ACCEPTED,
        Json(SubmitActionResponse {
            agent_id,
            action: action_type.as_str().to_string(),
            queued: true,
            token_cost,
            submitted_tick: tick,
        }),
    )
        .into_response()
}

/// GET /api/v1/human/agents/:id/state
///
/// Get the current state of a human-controlled agent, including timeout info.
pub async fn human_agent_state(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
) -> impl IntoResponse {
    let agent = {
        let reg = state.human_agent_registry.lock().await;
        match reg.get_by_agent(&agent_id) {
            Some(a) => a.clone(),
            None => {
                return (
                    StatusCode::NOT_FOUND,
                    Json(ErrorResponse {
                        error: "human agent not found".into(),
                    }),
                )
                    .into_response();
            }
        }
    };
    let current_tick = *state.tick_rx.borrow();
    let timeout_ticks = match &state.genesis_config {
        Some(cfg) => cfg.human_agent.timeout_ticks,
        None => crate::config::HumanAgentConfig::default().timeout_ticks,
    };
    let ticks_since_last_action = current_tick.saturating_sub(agent.last_action_tick);
    let will_timeout = ticks_since_last_action >= timeout_ticks;

    let pending = state.human_action_queue.lock().await.pending_count();

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "agent_state": agent,
            "current_tick": current_tick,
            "ticks_since_last_action": ticks_since_last_action,
            "timeout_ticks": timeout_ticks,
            "will_timeout": will_timeout,
            "pending_actions": pending,
        })),
    )
        .into_response()
}

/// GET /api/v1/human/agents/list-human
///
/// List all human-controlled agents.
pub async fn human_list_all(State(state): State<AppState>) -> impl IntoResponse {
    let agents: Vec<HumanAgent> = state
        .human_agent_registry
        .lock()
        .await
        .iter_alive()
        .cloned()
        .collect();
    Json(agents).into_response()
}
