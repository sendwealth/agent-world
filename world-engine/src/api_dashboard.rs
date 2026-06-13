//! Dashboard API endpoints — aggregated data for the dashboard UI pages.
//!
//! These endpoints read from existing subsystems (agents, snapshots,
//! governance metrics, trust network, mentorship, federation) and return
//! JSON suitable for the dashboard frontend. When a subsystem is not
//! configured, endpoints return empty defaults (`{}` or `[]`) rather than
//! 404, since the frontend has fallback handling.

use axum::{
    extract::{Query, State},
    response::IntoResponse,
    routing::get,
    Json,
};
use serde::{Deserialize, Serialize};

use crate::api::AppState;

// ── Query Types ───────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct GovernanceTimelineQuery {
    pub event_type: Option<crate::world::event::EventType>,
    pub from_tick: Option<u64>,
    pub to_tick: Option<u64>,
}

// ── Response Types ───────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct EconomyResponse {
    pub tick: u64,
    pub total_tokens: u64,
    pub total_money: u64,
    pub agent_count: usize,
    pub alive_count: usize,
    pub gini_coefficient: f64,
    pub history: Vec<SnapshotSummary>,
}

#[derive(Debug, Serialize, Clone)]
pub struct SnapshotSummary {
    pub tick: u64,
    pub timestamp: i64,
    pub total_population: u64,
    pub active_agents: u64,
    pub gdp: u64,
    pub gini_coefficient: f64,
}

#[derive(Debug, Serialize)]
pub struct PopulationResponse {
    pub tick: u64,
    pub alive_count: usize,
    pub dead_count: usize,
    pub total_spawned: usize,
    pub total_tokens: u64,
    pub total_money: u64,
    pub max_generation: u32,
    pub phase_distribution: Vec<PhaseBreakdown>,
}

#[derive(Debug, Serialize)]
pub struct PhaseBreakdown {
    pub phase: String,
    pub count: usize,
}

#[derive(Debug, Serialize)]
pub struct BriefingResponse {
    pub tick: u64,
    pub agent_count: usize,
    pub alive_count: usize,
    pub total_tokens: u64,
    pub total_money: u64,
    pub latest_snapshot: Option<SnapshotSummary>,
    pub governance_summary: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct TrustNetworkResponse {
    pub total_relationships: usize,
    pub avg_trust_score: f64,
    pub allies_count: usize,
    pub enemies_count: usize,
    pub top_allies: Vec<TrustEdgeDto>,
    pub top_enemies: Vec<TrustEdgeDto>,
}

#[derive(Debug, Serialize, Clone)]
pub struct TrustEdgeDto {
    pub from_agent: String,
    pub to_agent: String,
    pub score: f64,
    pub interaction_count: u64,
    pub last_interaction_tick: u64,
}

#[derive(Debug, Serialize)]
pub struct EvolutionTreeResponse {
    pub branches: Vec<crate::evolution::skill_tree::SkillBranch>,
    pub max_level: u32,
    pub skill_stats: Vec<SkillStat>,
}

#[derive(Debug, Serialize)]
pub struct SkillStat {
    pub name: String,
    pub count: usize,
    pub avg_level: f64,
    pub max_level: u32,
}

#[derive(Debug, Serialize)]
pub struct MentorshipRelationsResponse {
    pub total_sessions: usize,
    pub active_sessions: usize,
    pub completed_sessions: usize,
    pub sessions: Vec<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct DiplomacyRelationsResponse {
    pub worlds: Vec<serde_json::Value>,
    pub treaties: Vec<serde_json::Value>,
    pub summary: Option<serde_json::Value>,
}

// ── Handlers ─────────────────────────────────────────────

/// GET /api/v1/world/economy — Economic indicators (GDP, tokens, money).
pub async fn world_economy(State(state): State<AppState>) -> impl IntoResponse {
    let agents = state.agents.lock().await;
    let tick = *state.tick_rx.borrow();

    let alive_agents: Vec<&crate::api::AgentDto> = agents.iter().filter(|a| a.alive).collect();
    let total_tokens: u64 = alive_agents.iter().map(|a| a.tokens).sum();
    let total_money: u64 = alive_agents.iter().map(|a| a.money).sum();
    let gini = compute_gini(&alive_agents);

    // Fetch recent snapshots for history
    let history = match &state.snapshot_store {
        Some(store) => {
            let store = store.lock().await;
            store
                .list(None, None, Some(20))
                .unwrap_or_default()
                .into_iter()
                .map(|s| SnapshotSummary {
                    tick: s.tick,
                    timestamp: s.timestamp,
                    total_population: s.total_population,
                    active_agents: s.active_agents,
                    gdp: s.gdp,
                    gini_coefficient: s.gini_coefficient,
                })
                .collect()
        }
        None => Vec::new(),
    };

    Json(EconomyResponse {
        tick,
        total_tokens,
        total_money,
        agent_count: agents.len(),
        alive_count: alive_agents.len(),
        gini_coefficient: gini,
        history,
    })
    .into_response()
}

/// GET /api/v1/world/population — Population statistics.
pub async fn world_population(State(state): State<AppState>) -> impl IntoResponse {
    let agents = state.agents.lock().await;
    let tick = *state.tick_rx.borrow();

    let alive_count = agents.iter().filter(|a| a.alive).count();
    let dead_count = agents.iter().filter(|a| !a.alive).count();
    let total_spawned = agents.len();
    let total_tokens: u64 = agents.iter().filter(|a| a.alive).map(|a| a.tokens).sum();
    let total_money: u64 = agents.iter().filter(|a| a.alive).map(|a| a.money).sum();
    let max_generation = agents.iter().map(|a| a.generation).max().unwrap_or(0);

    // Phase distribution
    let mut phase_map: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    for a in agents.iter() {
        *phase_map.entry(a.phase.clone()).or_insert(0) += 1;
    }
    let phase_distribution: Vec<PhaseBreakdown> = phase_map
        .into_iter()
        .map(|(phase, count)| PhaseBreakdown { phase, count })
        .collect();

    Json(PopulationResponse {
        tick,
        alive_count,
        dead_count,
        total_spawned,
        total_tokens,
        total_money,
        max_generation,
        phase_distribution,
    })
    .into_response()
}

/// GET /api/v1/governance/timeline — Worldwide governance event timeline.
pub async fn governance_timeline(
    State(state): State<AppState>,
    Query(query): Query<GovernanceTimelineQuery>,
) -> impl IntoResponse {
    let metrics = match &state.governance_metrics {
        Some(m) => m.clone(),
        None => return Json(Vec::<serde_json::Value>::new()).into_response(),
    };
    let from = query.from_tick.unwrap_or(0);
    let to = query.to_tick.unwrap_or(u64::MAX);
    let metrics = metrics.lock().await;
    let timeline = metrics.get_world_timeline(query.event_type, (from, to));
    Json(timeline).into_response()
}

/// GET /api/v1/briefing — World briefing (aggregated snapshot).
pub async fn world_briefing(State(state): State<AppState>) -> impl IntoResponse {
    let agents = state.agents.lock().await;
    let tick = *state.tick_rx.borrow();

    let alive_count = agents.iter().filter(|a| a.alive).count();
    let total_tokens: u64 = agents.iter().filter(|a| a.alive).map(|a| a.tokens).sum();
    let total_money: u64 = agents.iter().filter(|a| a.alive).map(|a| a.money).sum();

    // Latest snapshot
    let latest_snapshot = match &state.snapshot_store {
        Some(store) => {
            let store = store.lock().await;
            store.list(None, None, Some(1)).unwrap_or_default().into_iter().next().map(
                |s| SnapshotSummary {
                    tick: s.tick,
                    timestamp: s.timestamp,
                    total_population: s.total_population,
                    active_agents: s.active_agents,
                    gdp: s.gdp,
                    gini_coefficient: s.gini_coefficient,
                },
            )
        }
        None => None,
    };

    // Governance summary
    let governance_summary = match &state.governance_metrics {
        Some(m) => {
            let metrics = m.lock().await;
            Some(serde_json::to_value(metrics.get_world_governance_summary()).ok()).and_then(|v| v)
        }
        None => None,
    };

    Json(BriefingResponse {
        tick,
        agent_count: agents.len(),
        alive_count,
        total_tokens,
        total_money,
        latest_snapshot,
        governance_summary,
    })
    .into_response()
}

/// GET /api/v1/trust/network — Trust network overview (all edges + aggregates).
pub async fn trust_network_overview(State(state): State<AppState>) -> impl IntoResponse {
    let Some(ref net) = state.trust_network else {
        return Json(TrustNetworkResponse {
            total_relationships: 0,
            avg_trust_score: 0.0,
            allies_count: 0,
            enemies_count: 0,
            top_allies: Vec::new(),
            top_enemies: Vec::new(),
        })
        .into_response();
    };
    let net = net.lock().await;

    let all_edges: Vec<&crate::economy::trust::TrustEdge> = net.all_edges();

    let total_relationships = all_edges.len();
    let avg_trust_score = if total_relationships > 0 {
        all_edges.iter().map(|e| e.score).sum::<f64>() / total_relationships as f64
    } else {
        0.0
    };

    let mut allies: Vec<TrustEdgeDto> = all_edges
        .iter()
        .filter(|e| e.score > 0.3)
        .map(|e| TrustEdgeDto {
            from_agent: e.from_agent.clone(),
            to_agent: e.to_agent.clone(),
            score: e.score,
            interaction_count: e.interaction_count,
            last_interaction_tick: e.last_interaction_tick,
        })
        .collect();
    allies.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

    let mut enemies: Vec<TrustEdgeDto> = all_edges
        .iter()
        .filter(|e| e.score < -0.3)
        .map(|e| TrustEdgeDto {
            from_agent: e.from_agent.clone(),
            to_agent: e.to_agent.clone(),
            score: e.score,
            interaction_count: e.interaction_count,
            last_interaction_tick: e.last_interaction_tick,
        })
        .collect();
    enemies.sort_by(|a, b| a.score.partial_cmp(&b.score).unwrap_or(std::cmp::Ordering::Equal));

    let allies_count = allies.len();
    let enemies_count = enemies.len();

    // Top 20 each
    allies.truncate(20);
    enemies.truncate(20);

    Json(TrustNetworkResponse {
        total_relationships,
        avg_trust_score,
        allies_count,
        enemies_count,
        top_allies: allies,
        top_enemies: enemies,
    })
    .into_response()
}

/// GET /api/v1/evolution/tree — Evolution skill tree + aggregated skill stats.
pub async fn evolution_tree(State(state): State<AppState>) -> impl IntoResponse {
    let skill_tree = crate::evolution::skill_tree::SkillTree::new(10);

    let agents = state.agents.lock().await;

    // Aggregate skill stats from alive agents
    let mut skill_data: std::collections::HashMap<String, (u32, u64, u32)> =
        std::collections::HashMap::new(); // (sum_levels, count, max_level)
    for agent in agents.iter().filter(|a| a.alive) {
        for (skill_name, &level) in &agent.skills {
            let entry = skill_data
                .entry(skill_name.clone())
                .or_insert((0, 0, 0));
            entry.0 += level;
            entry.1 += 1;
            if level > entry.2 {
                entry.2 = level;
            }
        }
    }

    let skill_stats: Vec<SkillStat> = skill_data
        .into_iter()
        .map(|(name, (sum, count, max))| SkillStat {
            name,
            count: count as usize,
            avg_level: if count > 0 {
                sum as f64 / count as f64
            } else {
                0.0
            },
            max_level: max,
        })
        .collect();

    Json(EvolutionTreeResponse {
        branches: skill_tree.branches,
        max_level: skill_tree.max_level,
        skill_stats,
    })
    .into_response()
}

/// GET /api/v1/mentorship/relations — All mentorship sessions + counts.
pub async fn mentorship_relations(State(state): State<AppState>) -> impl IntoResponse {
    let Some(ref sys) = state.mentorship_system else {
        return Json(MentorshipRelationsResponse {
            total_sessions: 0,
            active_sessions: 0,
            completed_sessions: 0,
            sessions: Vec::new(),
        })
        .into_response();
    };
    let sys = sys.lock().await;

    let total = sys.session_count();
    let active = sys.active_count();
    let completed = sys.completed_count();

    let sessions: Vec<serde_json::Value> = sys
        .all_sessions()
        .into_iter()
        .filter_map(|s| serde_json::to_value(s).ok())
        .collect();

    Json(MentorshipRelationsResponse {
        total_sessions: total,
        active_sessions: active,
        completed_sessions: completed,
        sessions,
    })
    .into_response()
}

/// GET /api/v1/diplomacy/relations — Federation diplomacy overview.
pub async fn diplomacy_relations(State(state): State<AppState>) -> impl IntoResponse {
    let Some(ref fed) = state.federation else {
        return Json(DiplomacyRelationsResponse {
            worlds: Vec::new(),
            treaties: Vec::new(),
            summary: None,
        })
        .into_response();
    };
    let fed = fed.lock().await;

    let worlds: Vec<serde_json::Value> = fed
        .list_worlds()
        .into_iter()
        .filter_map(|w| serde_json::to_value(w).ok())
        .collect();

    let treaties: Vec<serde_json::Value> = fed
        .list_treaties(None, None)
        .into_iter()
        .filter_map(|t| serde_json::to_value(t).ok())
        .collect();

    let summary = serde_json::to_value(fed.summary()).ok();

    Json(DiplomacyRelationsResponse {
        worlds,
        treaties,
        summary,
    })
    .into_response()
}

// ── Routes ───────────────────────────────────────────────

pub fn dashboard_routes() -> axum::Router<AppState> {
    axum::Router::new()
        .route("/world/economy", get(world_economy))
        .route("/world/population", get(world_population))
        .route("/governance/timeline", get(governance_timeline))
        .route("/briefing", get(world_briefing))
        .route("/trust/network", get(trust_network_overview))
        .route("/evolution/tree", get(evolution_tree))
        .route("/mentorship/relations", get(mentorship_relations))
        .route("/diplomacy/relations", get(diplomacy_relations))
}

// ── Helpers ──────────────────────────────────────────────

/// Compute the Gini coefficient of token wealth across agents.
fn compute_gini(agents: &[&crate::api::AgentDto]) -> f64 {
    if agents.is_empty() {
        return 0.0;
    }
    let mut wealths: Vec<f64> = agents.iter().map(|a| a.tokens as f64).collect();
    wealths.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = wealths.len() as f64;
    let sum: f64 = wealths.iter().sum();
    if sum == 0.0 {
        return 0.0;
    }
    let weighted_sum: f64 = wealths
        .iter()
        .enumerate()
        .map(|(i, w)| (i as f64 + 1.0) * w)
        .sum();
    (2.0 * weighted_sum) / (n * sum) - (n + 1.0) / n
}
