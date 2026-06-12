//! Research API endpoints — `/api/v2/*`.
//!
//! Provides aggregated world state, deep agent profiles, historical snapshots,
//! emergence metrics, and a real-time SSE event stream for external researchers.

use std::convert::Infallible;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{
        sse::{Event as SseEvent, Sse},
        IntoResponse,
    },
    routing::get,
    Json, Router,
};
use futures::stream::Stream;
use serde::{Deserialize, Serialize};
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;

use crate::api::{parse_event_types, AgentDto, AppState, ErrorResponse, SseQuery};

// ── Response Types ────────────────────────────────────────

/// `GET /api/v2/world/state` — aggregated world state.
#[derive(Debug, Serialize)]
pub struct WorldStateResponse {
    pub tick: u64,
    pub agent_count: usize,
    pub alive_count: usize,
    pub dead_count: usize,
    pub org_count: usize,
    pub total_money: u64,
    pub total_tokens: u64,
    pub resource_distribution: ResourceDistribution,
}

/// Breakdown of resource distribution across agents.
#[derive(Debug, Serialize)]
pub struct ResourceDistribution {
    pub total_money: u64,
    pub total_tokens: u64,
    pub avg_money_per_agent: f64,
    pub avg_tokens_per_agent: f64,
    pub gini_coefficient: Option<f64>,
}

/// `GET /api/v2/agents/{id}/profile` — deep agent profile.
#[derive(Debug, Serialize)]
pub struct AgentProfileResponse {
    pub id: String,
    pub name: String,
    pub phase: String,
    pub tokens: u64,
    pub money: u64,
    pub alive: bool,
    pub ticks_survived: u64,
    pub organization: Option<AgentOrgInfo>,
    pub reputation: Option<f64>,
}

/// Organization info attached to an agent profile.
#[derive(Debug, Serialize)]
pub struct AgentOrgInfo {
    pub org_id: String,
    pub org_name: String,
    pub org_type: String,
    pub role: String,
}

/// `GET /api/v2/world/history` query parameters.
#[derive(Debug, Deserialize, Default)]
pub struct HistoryQuery {
    pub from_tick: Option<u64>,
    pub to_tick: Option<u64>,
    pub limit: Option<u64>,
}

/// `GET /api/v2/metrics/emergence` — emergence metrics.
#[derive(Debug, Serialize)]
pub struct EmergenceMetricsResponse {
    pub tick: u64,
    pub cultural_diversity: CulturalDiversityMetrics,
    pub organization_metrics: OrganizationMetrics,
    pub economic_concentration: EconomicConcentrationMetrics,
}

/// Cultural diversity metrics.
#[derive(Debug, Serialize)]
pub struct CulturalDiversityMetrics {
    pub total_agents: usize,
    pub alive_agents: usize,
    pub dead_agents: usize,
    pub phase_distribution: PhaseDistribution,
}

/// Distribution of agents across lifecycle phases.
#[derive(Debug, Serialize)]
pub struct PhaseDistribution {
    pub birth: usize,
    pub childhood: usize,
    pub adult: usize,
    pub elder: usize,
    pub dying: usize,
    pub dead: usize,
}

/// Organization formation metrics.
#[derive(Debug, Serialize)]
pub struct OrganizationMetrics {
    pub total_orgs: usize,
    pub active_orgs: usize,
    pub inactive_orgs: usize,
    pub dissolved_orgs: usize,
    pub total_members: usize,
    pub org_type_distribution: OrgTypeDistribution,
}

/// Count of orgs per type.
#[derive(Debug, Serialize)]
pub struct OrgTypeDistribution {
    pub company: usize,
    pub guild: usize,
    pub alliance: usize,
    pub university: usize,
}

/// Economic concentration metrics.
#[derive(Debug, Serialize)]
pub struct EconomicConcentrationMetrics {
    pub total_money: u64,
    pub total_tokens: u64,
    pub gini_coefficient: Option<f64>,
    pub top_10_percent_share: Option<f64>,
}

// ── Router ────────────────────────────────────────────────

/// Build the `/api/v2/*` research routes (without auth middleware).
/// The caller wraps this in the auth layer.
pub fn research_routes() -> Router<AppState> {
    Router::new()
        .route("/api/v2/world/state", get(get_world_state))
        .route("/api/v2/agents", get(list_agents_v2))
        .route("/api/v2/agents/{id}/profile", get(get_agent_profile))
        .route("/api/v2/world/history", get(get_world_history))
        .route("/api/v2/metrics/emergence", get(get_emergence_metrics))
        .route("/api/v2/world/events/stream", get(research_events_sse))
}

// ── Handlers ──────────────────────────────────────────────

/// `GET /api/v2/world/state`
async fn get_world_state(State(state): State<AppState>) -> impl IntoResponse {
    let agents = state.agents.lock().await;
    let tick = *state.tick_rx.borrow();

    let alive_count = agents.iter().filter(|a| a.alive).count();
    let dead_count = agents.iter().filter(|a| !a.alive).count();
    let total_money: u64 = agents.iter().map(|a| a.money).sum();
    let total_tokens: u64 = agents.iter().map(|a| a.tokens).sum();
    let alive_agent_count = agents.iter().filter(|a| a.alive).count();

    let gini = compute_gini(&agents);

    let org_count = if let Some(ref org_store) = state.org_store {
        let store = org_store.lock().await;
        store.list().len()
    } else {
        0
    };

    let resource_distribution = ResourceDistribution {
        total_money,
        total_tokens,
        avg_money_per_agent: if alive_agent_count > 0 {
            total_money as f64 / alive_agent_count as f64
        } else {
            0.0
        },
        avg_tokens_per_agent: if alive_agent_count > 0 {
            total_tokens as f64 / alive_agent_count as f64
        } else {
            0.0
        },
        gini_coefficient: gini,
    };

    Json(WorldStateResponse {
        tick,
        agent_count: agents.len(),
        alive_count,
        dead_count,
        org_count,
        total_money,
        total_tokens,
        resource_distribution,
    })
}

/// `GET /api/v2/agents` — list all agents (v2, auth-protected).
async fn list_agents_v2(State(state): State<AppState>) -> impl IntoResponse {
    let agents = state.agents.lock().await;
    Json(&*agents).into_response()
}

/// `GET /api/v2/agents/{id}/profile`
async fn get_agent_profile(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let agents = state.agents.lock().await;
    let agent = match agents.iter().find(|a| a.id == id) {
        Some(a) => a.clone(),
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "agent not found".into(),
                }),
            )
                .into_response()
        }
    };
    drop(agents);

    // Look up org membership
    let org_info = if let Some(ref org_store) = state.org_store {
        let store = org_store.lock().await;
        store.list().iter().find_map(|org| {
            org.members.iter().find_map(|m| {
                if m.agent_id == id {
                    Some(AgentOrgInfo {
                        org_id: org.id.clone(),
                        org_name: org.name.clone(),
                        org_type: format!("{:?}", org.org_type).to_lowercase(),
                        role: format!("{:?}", m.role).to_lowercase(),
                    })
                } else {
                    None
                }
            })
        })
    } else {
        None
    };

    // Look up reputation if available
    let reputation = if let Some(ref rep_system) = state.reputation_system {
        let system = rep_system.lock().await;
        let score = system.get_reputation(&id);
        // Only include if non-default (agent has reputation data)
        Some(score)
    } else {
        None
    };

    Json(AgentProfileResponse {
        id: agent.id,
        name: agent.name,
        phase: agent.phase,
        tokens: agent.tokens,
        money: agent.money,
        alive: agent.alive,
        ticks_survived: agent.ticks_survived,
        organization: org_info,
        reputation,
    })
    .into_response()
}

/// `GET /api/v2/world/history`
async fn get_world_history(
    State(state): State<AppState>,
    Query(query): Query<HistoryQuery>,
) -> impl IntoResponse {
    let store = match state.snapshot_store {
        Some(ref s) => s,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "snapshot store not available".into(),
                }),
            )
                .into_response()
        }
    };

    let result = {
        let store = store.lock().await;
        store.list(query.from_tick, query.to_tick, query.limit)
    };

    match result {
        Ok(snapshots) => Json(snapshots).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("failed to query snapshots: {}", e),
            }),
        )
            .into_response(),
    }
}

/// `GET /api/v2/metrics/emergence`
async fn get_emergence_metrics(State(state): State<AppState>) -> impl IntoResponse {
    let agents = state.agents.lock().await;
    let tick = *state.tick_rx.borrow();

    let alive_count = agents.iter().filter(|a| a.alive).count();
    let dead_count = agents.iter().filter(|a| !a.alive).count();

    let phase_distribution = PhaseDistribution {
        birth: agents.iter().filter(|a| a.phase == "birth").count(),
        childhood: agents.iter().filter(|a| a.phase == "childhood").count(),
        adult: agents.iter().filter(|a| a.phase == "adult").count(),
        elder: agents.iter().filter(|a| a.phase == "elder").count(),
        dying: agents.iter().filter(|a| a.phase == "dying").count(),
        dead: dead_count,
    };

    let gini = compute_gini(&agents);

    let total_money: u64 = agents.iter().map(|a| a.money).sum();
    let total_tokens: u64 = agents.iter().map(|a| a.tokens).sum();

    // Top 10% wealth share
    let top_10_share = compute_top_percent_share(&agents, 0.1);

    drop(agents);

    // Organization metrics
    let org_metrics = if let Some(ref org_store) = state.org_store {
        let store = org_store.lock().await;
        let orgs = store.list();
        let active = orgs
            .iter()
            .filter(|o| o.status == crate::organization::org::OrgStatus::Active)
            .count();
        let inactive = orgs
            .iter()
            .filter(|o| o.status == crate::organization::org::OrgStatus::Inactive)
            .count();
        let dissolved = orgs
            .iter()
            .filter(|o| o.status == crate::organization::org::OrgStatus::Dissolved)
            .count();
        let total_members: usize = orgs.iter().map(|o| o.member_count()).sum();

        let type_dist = OrgTypeDistribution {
            company: orgs
                .iter()
                .filter(|o| matches!(o.org_type, crate::organization::org::OrgType::Company))
                .count(),
            guild: orgs
                .iter()
                .filter(|o| matches!(o.org_type, crate::organization::org::OrgType::Guild))
                .count(),
            alliance: orgs
                .iter()
                .filter(|o| matches!(o.org_type, crate::organization::org::OrgType::Alliance))
                .count(),
            university: orgs
                .iter()
                .filter(|o| matches!(o.org_type, crate::organization::org::OrgType::University))
                .count(),
        };

        OrganizationMetrics {
            total_orgs: orgs.len(),
            active_orgs: active,
            inactive_orgs: inactive,
            dissolved_orgs: dissolved,
            total_members,
            org_type_distribution: type_dist,
        }
    } else {
        OrganizationMetrics {
            total_orgs: 0,
            active_orgs: 0,
            inactive_orgs: 0,
            dissolved_orgs: 0,
            total_members: 0,
            org_type_distribution: OrgTypeDistribution {
                company: 0,
                guild: 0,
                alliance: 0,
                university: 0,
            },
        }
    };

    Json(EmergenceMetricsResponse {
        tick,
        cultural_diversity: CulturalDiversityMetrics {
            total_agents: alive_count + dead_count,
            alive_agents: alive_count,
            dead_agents: dead_count,
            phase_distribution,
        },
        organization_metrics: org_metrics,
        economic_concentration: EconomicConcentrationMetrics {
            total_money,
            total_tokens,
            gini_coefficient: gini,
            top_10_percent_share: top_10_share,
        },
    })
}

/// `GET /api/v2/world/events/stream` — SSE endpoint for research.
/// Reuses EventBus subscription with optional filtering.
async fn research_events_sse(
    State(state): State<AppState>,
    Query(query): Query<SseQuery>,
) -> Result<Sse<impl Stream<Item = Result<SseEvent, Infallible>>>, (StatusCode, Json<ErrorResponse>)>
{
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

    let rx = state.event_bus.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(move |result| {
        let event = match result {
            Ok(e) => e,
            Err(_) => return Some(Ok(SseEvent::default().data("[\"lagged\"]"))),
        };

        // Apply type filter
        if let Some(ref filter) = type_filter {
            if !filter.contains(&event.event_type()) {
                return None;
            }
        }

        // Apply agent filter
        if let Some(ref aid) = query.agent_id {
            if event.agent_id() != Some(aid.as_str()) {
                return None;
            }
        }

        let data = event.to_json();
        Some(Ok(SseEvent::default().data(data)))
    });

    Ok(Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(std::time::Duration::from_secs(15))
            .text("ping"),
    ))
}

// ── Helpers ───────────────────────────────────────────────

/// Compute the Gini coefficient for token distribution among alive agents.
fn compute_gini(agents: &[AgentDto]) -> Option<f64> {
    let alive: Vec<u64> = agents
        .iter()
        .filter(|a| a.alive)
        .map(|a| a.tokens)
        .collect();
    if alive.len() < 2 {
        return None;
    }
    let mut sorted = alive;
    sorted.sort();

    let n = sorted.len() as f64;
    let sum: u64 = sorted.iter().sum();
    if sum == 0 {
        return Some(0.0);
    }

    let weighted_sum: f64 = sorted
        .iter()
        .enumerate()
        .map(|(i, &v)| ((i as f64 + 1.0) * 2.0 - n - 1.0) * v as f64)
        .sum();

    Some(weighted_sum / (n * sum as f64))
}

/// Compute the share of total tokens held by the top `pct` of agents.
fn compute_top_percent_share(agents: &[AgentDto], pct: f64) -> Option<f64> {
    let mut amounts: Vec<u64> = agents
        .iter()
        .filter(|a| a.alive)
        .map(|a| a.tokens)
        .collect();
    if amounts.is_empty() {
        return None;
    }
    amounts.sort_by(|a, b| b.cmp(a));

    let total: u64 = amounts.iter().sum();
    if total == 0 {
        return Some(0.0);
    }

    let top_count = ((amounts.len() as f64) * pct).ceil() as usize;
    let top_count = top_count.max(1).min(amounts.len());
    let top_sum: u64 = amounts[..top_count].iter().sum();

    Some(top_sum as f64 / total as f64)
}

// ── Tests ──────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_agent(id: &str, tokens: u64, alive: bool) -> AgentDto {
        AgentDto {
            id: id.to_string(),
            name: format!("Agent-{}", id),
            phase: "adult".to_string(),
            tokens,
            money: tokens / 2,
            alive,
            ticks_survived: 0,
            personality: String::new(),
            parent_ids: Vec::new(),
            generation: 0,
            skills: HashMap::new(),
            created_at: String::new(),
        }
    }

    #[test]
    fn gini_perfect_equality() {
        let agents: Vec<AgentDto> = (0..10)
            .map(|i| make_agent(&i.to_string(), 100, true))
            .collect();
        let gini = compute_gini(&agents).unwrap();
        assert!(gini.abs() < 0.001, "gini should be ~0, got {}", gini);
    }

    #[test]
    fn gini_perfect_inequality() {
        let mut agents: Vec<AgentDto> = (0..10)
            .map(|i| make_agent(&i.to_string(), 0, true))
            .collect();
        agents[0].tokens = 1000;
        let gini = compute_gini(&agents).unwrap();
        assert!(gini > 0.8, "gini should be close to 1, got {}", gini);
    }

    #[test]
    fn gini_too_few_agents() {
        let agents = vec![make_agent("1", 100, true)];
        assert!(compute_gini(&agents).is_none());
    }

    #[test]
    fn top_percent_share_basic() {
        let agents: Vec<AgentDto> = (0..10)
            .map(|i| make_agent(&i.to_string(), (i as u64 + 1) * 10, true))
            .collect();
        let share = compute_top_percent_share(&agents, 0.1).unwrap();
        // Top 10% of 10 agents = top 1 agent (100 tokens), total = 550
        assert!((share - 100.0 / 550.0).abs() < 0.001, "share={}", share);
    }
}
