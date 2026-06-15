use std::collections::HashMap;
use std::convert::Infallible;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{
        sse::{Event as SseEvent, Sse},
        IntoResponse,
    },
    Json,
};
use futures::stream::Stream;
use serde::{Deserialize, Serialize};
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;
use uuid::Uuid;

use crate::api::{A2AMessage, AgentDto, AppState, ErrorResponse};
use crate::world::event::WorldEvent;

#[derive(Debug, Deserialize)]
pub struct SseQuery {
    /// Comma-separated event type filter (e.g., "agent_died,agent_rescued")
    pub types: Option<String>,
    /// Filter events for a specific agent ID
    pub agent_id: Option<String>,
}

/// Parse a comma-separated string of event type names into `EventType` values.
/// Returns an error message for any unrecognized names.
pub fn parse_event_types(raw: &str) -> Result<Vec<crate::world::event::EventType>, String> {
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

pub async fn world_events_sse(
    State(state): State<AppState>,
    Query(query): Query<SseQuery>,
) -> Result<Sse<impl Stream<Item = Result<SseEvent, Infallible>>>, (StatusCode, Json<ErrorResponse>)>
{
    // Parse type filter if provided
    let type_filter: Option<std::collections::HashSet<crate::world::event::EventType>> =
        if let Some(ref types_str) = query.types {
            let parsed = parse_event_types(types_str)
                .map_err(|e| (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: e })))?;
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

// ── Request/Response Types ──────────────────────────────────

fn default_tokens() -> u64 {
    100_000
}

#[derive(Debug, Deserialize)]
pub struct SpawnAgentRequest {
    pub name: String,
    #[serde(default = "default_tokens")]
    pub tokens: u64,
    #[serde(default)]
    pub money: u64,
}

#[derive(Debug, Deserialize)]
pub struct SendMessageRequest {
    pub from_agent: String,
    pub to_agent: String,
    pub message_type: String,
    pub payload: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldStatsResponse {
    pub agent_count: usize,
    pub alive_count: usize,
    pub dead_count: usize,
    pub total_money: u64,
    pub total_tokens: u64,
    pub tick: u64,
    pub task_count: usize,
}

// ── Leaderboard ──────────────────────────────────────────

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LeaderboardEntry {
    pub agent_id: String,
    pub agent_name: String,
    pub value: f64,
    pub rank: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LeaderboardResponse {
    pub richest: Vec<LeaderboardEntry>,
    pub longest_lived: Vec<LeaderboardEntry>,
    pub highest_skill: Vec<LeaderboardEntry>,
    pub highest_reputation: Vec<LeaderboardEntry>,
}

pub async fn world_leaderboard(State(state): State<AppState>) -> impl IntoResponse {
    let agents = state.agents.lock().await;
    let alive_agents: Vec<&AgentDto> = agents.iter().filter(|a| a.alive).collect();
    let limit = 10usize;

    // Richest by money
    let mut by_money: Vec<&AgentDto> = alive_agents.to_vec();
    by_money.sort_by_key(|b| std::cmp::Reverse(b.money));
    let richest: Vec<LeaderboardEntry> = by_money
        .iter()
        .take(limit)
        .enumerate()
        .map(|(i, a)| LeaderboardEntry {
            agent_id: a.id.clone(),
            agent_name: a.name.clone(),
            value: a.money as f64,
            rank: i + 1,
        })
        .collect();

    // Longest lived by ticks_survived
    let mut by_age: Vec<&AgentDto> = alive_agents.to_vec();
    by_age.sort_by_key(|b| std::cmp::Reverse(b.ticks_survived));
    let longest_lived: Vec<LeaderboardEntry> = by_age
        .iter()
        .take(limit)
        .enumerate()
        .map(|(i, a)| LeaderboardEntry {
            agent_id: a.id.clone(),
            agent_name: a.name.clone(),
            value: a.ticks_survived as f64,
            rank: i + 1,
        })
        .collect();

    // Highest skill (max skill level across all skills)
    let mut by_skill: Vec<&AgentDto> = alive_agents.to_vec();
    #[allow(clippy::unnecessary_sort_by)]
    by_skill.sort_by(|a, b| {
        let max_a = a.skills.values().max().copied().unwrap_or(0);
        let max_b = b.skills.values().max().copied().unwrap_or(0);
        max_b.cmp(&max_a)
    });
    let highest_skill: Vec<LeaderboardEntry> = by_skill
        .iter()
        .take(limit)
        .enumerate()
        .map(|(i, a)| LeaderboardEntry {
            agent_id: a.id.clone(),
            agent_name: a.name.clone(),
            value: a.skills.values().max().copied().unwrap_or(0) as f64,
            rank: i + 1,
        })
        .collect();

    // Highest reputation — use reputation system if available, else empty
    let highest_reputation: Vec<LeaderboardEntry> = {
        if let Some(rep_sys) = state.reputation_system.as_ref() {
            let rep = rep_sys.lock().await;
            let rankings = rep.get_rankings(limit);
            drop(rep);
            rankings
                .into_iter()
                .filter(|r| alive_agents.iter().any(|a| a.id == r.agent_id))
                .map(|r| LeaderboardEntry {
                    agent_id: r.agent_id.clone(),
                    agent_name: alive_agents
                        .iter()
                        .find(|a| a.id == r.agent_id)
                        .map(|a| a.name.clone())
                        .unwrap_or_else(|| r.agent_id.clone()),
                    value: r.reputation,
                    rank: r.rank,
                })
                .collect()
        } else {
            Vec::new()
        }
    };

    Json(LeaderboardResponse {
        richest,
        longest_lived,
        highest_skill,
        highest_reputation,
    })
}

// ── World Stats Handler ───────────────────────────────────

pub async fn world_stats(State(state): State<AppState>) -> impl IntoResponse {
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

pub async fn spawn_agent(
    State(state): State<AppState>,
    Json(body): Json<SpawnAgentRequest>,
) -> impl IntoResponse {
    let agent = AgentDto {
        id: Uuid::new_v4().to_string(),
        name: body.name.clone(),
        phase: "adult".to_string(),
        tokens: body.tokens,
        money: body.money,
        alive: true,
        ticks_survived: 0,
        personality: String::new(),
        parent_ids: Vec::new(),
        generation: 0,
        skills: HashMap::new(),
        created_at: chrono::Utc::now().to_rfc3339(),
    };

    state.event_bus.emit(WorldEvent::AgentSpawned {
        agent_id: agent.id.clone(),
        name: agent.name.clone(),
    });

    let mut agents = state.agents.lock().await;
    agents.push(agent.clone());

    (StatusCode::CREATED, Json(agent)).into_response()
}

pub async fn list_agents(State(state): State<AppState>) -> impl IntoResponse {
    let agents = state.agents.lock().await;
    Json(&*agents).into_response()
}

pub async fn get_agent(State(state): State<AppState>, Path(id): Path<String>) -> impl IntoResponse {
    let agents = state.agents.lock().await;
    match agents.iter().find(|a| a.id == id) {
        Some(agent) => Json(agent.clone()).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "agent not found".into(),
            }),
        )
            .into_response(),
    }
}

// ── A2A Message Handlers ──────────────────────────────────

pub async fn send_message(
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

pub async fn list_messages(State(state): State<AppState>) -> impl IntoResponse {
    let messages = state.messages.lock().await;
    Json(&*messages).into_response()
}

// ── Tick Handlers ─────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct AdvanceTickRequest {
    #[serde(default = "default_count")]
    pub count: u64,
}

pub fn default_count() -> u64 {
    1
}

pub async fn advance_tick(
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

pub async fn get_tick(State(state): State<AppState>) -> impl IntoResponse {
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

pub fn default_snapshot_limit() -> Option<u64> {
    Some(100)
}

pub async fn list_snapshots(
    State(state): State<AppState>,
    Query(query): Query<ListSnapshotsQuery>,
) -> impl IntoResponse {
    let store = match &state.snapshot_store {
        Some(s) => s.clone(),
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "snapshot store not configured".into(),
                }),
            )
                .into_response()
        }
    };

    let store = store.lock().await;
    match store.list(query.from_tick, query.to_tick, query.limit) {
        Ok(snapshots) => Json(&snapshots).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
            .into_response(),
    }
}

pub async fn get_latest_snapshot(State(state): State<AppState>) -> impl IntoResponse {
    let store = match &state.snapshot_store {
        Some(s) => s.clone(),
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "snapshot store not configured".into(),
                }),
            )
                .into_response()
        }
    };

    let store = store.lock().await;
    match store.latest() {
        Ok(Some(snapshot)) => Json(&snapshot).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "no snapshots available".into(),
            }),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
            .into_response(),
    }
}

pub async fn get_snapshot_by_tick(
    State(state): State<AppState>,
    Path(tick): Path<u64>,
) -> impl IntoResponse {
    let store = match &state.snapshot_store {
        Some(s) => s.clone(),
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "snapshot store not configured".into(),
                }),
            )
                .into_response()
        }
    };

    let store = store.lock().await;
    match store.get_by_tick(tick) {
        Ok(Some(snapshot)) => Json(&snapshot).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("no snapshot at tick {}", tick),
            }),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
            .into_response(),
    }
}

pub async fn create_snapshot(State(state): State<AppState>) -> impl IntoResponse {
    let store = match &state.snapshot_store {
        Some(s) => s.clone(),
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "snapshot store not configured".into(),
                }),
            )
                .into_response()
        }
    };

    let tick = *state.tick_rx.borrow();
    let agents = state.agents.lock().await;

    let snapshot = crate::time_capsule::build_snapshot(
        tick,
        &agents
            .iter()
            .map(|a| {
                let id = Uuid::parse_str(&a.id).unwrap_or_else(|_| Uuid::new_v4());
                let record = crate::economy::token_burn::AgentRecord {
                    id,
                    name: a.name.clone(),
                    phase: if a.alive {
                        crate::world::enums::AgentPhase::Adult
                    } else {
                        crate::world::enums::AgentPhase::Dead
                    },
                    tokens: a.tokens,
                    skills: std::collections::HashMap::new(),
                    personality: String::new(),
                    tasks_completed: 0,
                    tasks_attempted: 0,
                };
                (id, a.ticks_survived, record)
            })
            .collect::<Vec<_>>(),
        &[],
    );

    let store = store.lock().await;
    match store.save(&snapshot) {
        Ok(id) => {
            state.event_bus.emit(WorldEvent::SnapshotTaken {
                tick,
                path: format!("snapshot:{}", id),
            });
            (StatusCode::CREATED, Json(&snapshot)).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
            .into_response(),
    }
}

pub async fn export_snapshots_json(State(state): State<AppState>) -> impl IntoResponse {
    let store = match &state.snapshot_store {
        Some(s) => s.clone(),
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "snapshot store not configured".into(),
                }),
            )
                .into_response()
        }
    };

    let store = store.lock().await;
    match store.export_json() {
        Ok(json) => {
            let body = axum::body::Body::from(json);
            let mut resp = axum::response::Response::new(body);
            *resp.status_mut() = StatusCode::OK;
            resp.headers_mut()
                .insert("content-type", "application/json".parse().expect("valid header value"));
            resp.headers_mut().insert(
                "content-disposition",
                "attachment; filename=\"world_snapshots.json\""
                    .parse()
                    .expect("valid header value"),
            );
            resp
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
            .into_response(),
    }
}

pub async fn export_snapshots_csv(State(state): State<AppState>) -> impl IntoResponse {
    let store = match &state.snapshot_store {
        Some(s) => s.clone(),
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "snapshot store not configured".into(),
                }),
            )
                .into_response()
        }
    };

    let store = store.lock().await;
    match store.export_csv() {
        Ok(csv) => {
            let body = axum::body::Body::from(csv);
            let mut resp = axum::response::Response::new(body);
            *resp.status_mut() = StatusCode::OK;
            resp.headers_mut()
                .insert("content-type", "text/csv".parse().expect("valid header value"));
            resp.headers_mut().insert(
                "content-disposition",
                "attachment; filename=\"world_snapshots.csv\""
                    .parse()
                    .expect("valid header value"),
            );
            resp
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
            .into_response(),
    }
}

/// World SSE + agents + messages + tick + snapshot routes.
pub fn world_routes() -> axum::Router<AppState> {
    use axum::routing::*;
    axum::Router::new()
        .route("/world/events", get(world_events_sse))
        .route("/world/:id/events", get(world_events_sse))
        .route("/world/stats", get(world_stats))
        .route("/world/leaderboard", get(world_leaderboard))
        .route("/agents", get(list_agents))
        .route("/agents", post(spawn_agent))
        .route("/agents/:id", get(get_agent))
        .route("/messages", post(send_message))
        .route("/messages", get(list_messages))
        .route("/tick", post(advance_tick))
        .route("/tick", get(get_tick))
        .route("/snapshots", get(list_snapshots))
        .route("/snapshots/latest", get(get_latest_snapshot))
        .route("/snapshots/:tick", get(get_snapshot_by_tick))
        .route("/snapshots", post(create_snapshot))
        .route("/snapshots/export/json", get(export_snapshots_json))
        .route("/snapshots/export/csv", get(export_snapshots_csv))
}
