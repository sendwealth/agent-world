use std::collections::HashMap;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::*,
    Json,
};
use serde::{Deserialize, Serialize};

use crate::api::{AgentDto, AppState, ErrorResponse};

// ── Population Evolution Handlers ────────────────────────────────

/// Skill distribution entry.
#[derive(Debug, Serialize)]
pub struct SkillDistribution {
    pub name: String,
    pub count: usize,
    pub avg_level: f64,
    pub max_level: u32,
}

/// Phase distribution entry.
#[derive(Debug, Serialize)]
pub struct PhaseDistribution {
    pub phase: String,
    pub count: usize,
}

/// Full population statistics response.
#[derive(Debug, Serialize)]
pub struct PopulationStatsResponse {
    pub tick: u64,
    pub alive_count: usize,
    pub dead_count: usize,
    pub total_spawned: usize,
    pub birth_rate: f64,
    pub death_rate: f64,
    pub total_tokens: u64,
    pub total_money: u64,
    pub skill_distribution: Vec<SkillDistribution>,
    pub phase_distribution: Vec<PhaseDistribution>,
    pub shannon_index: f64,
    pub simpson_index: f64,
    pub avg_fitness: f64,
    pub max_generation: u32,
}

/// Single data point in the population timeline.
#[derive(Debug, Serialize)]
pub struct PopulationTimelinePoint {
    pub tick: u64,
    pub alive_count: usize,
    pub dead_count: usize,
    pub total_tokens: u64,
    pub total_money: u64,
    pub skill_types: usize,
    pub avg_fitness: f64,
}

/// Species tracker entry — a distinct "species" defined by a unique skill signature.
#[derive(Debug, Serialize)]
pub struct SpeciesEntry {
    pub skill_signature: String,
    pub count: usize,
    pub generations: Vec<u32>,
    pub alive: usize,
}

/// Genealogy node for the lineage tree.
#[derive(Debug, Serialize)]
pub struct GenealogyNode {
    pub agent: AgentDto,
    pub children: Vec<GenealogyNode>,
}

/// Compute skill distribution from agents' skills maps.
pub fn compute_skill_distribution(agents: &[AgentDto]) -> Vec<SkillDistribution> {
    let mut skill_map: HashMap<String, (usize, f64, u32)> = HashMap::new();
    for agent in agents {
        if !agent.alive {
            continue;
        }
        for (name, &level) in &agent.skills {
            let entry = skill_map.entry(name.clone()).or_insert((0, 0.0, 0));
            entry.0 += 1;
            entry.1 += level as f64;
            entry.2 = entry.2.max(level);
        }
    }
    let mut result: Vec<SkillDistribution> = skill_map
        .into_iter()
        .map(
            |(name, (count, total_level, max_level))| SkillDistribution {
                name,
                count,
                avg_level: if count > 0 {
                    total_level / count as f64
                } else {
                    0.0
                },
                max_level,
            },
        )
        .collect();
    result.sort_by_key(|a| std::cmp::Reverse(a.count));
    result
}

/// Compute phase distribution from agents list.
pub fn compute_phase_distribution(agents: &[AgentDto]) -> Vec<PhaseDistribution> {
    let mut phase_map: HashMap<String, usize> = HashMap::new();
    for agent in agents {
        if !agent.alive {
            continue;
        }
        *phase_map.entry(agent.phase.clone()).or_insert(0) += 1;
    }
    let mut result: Vec<PhaseDistribution> = phase_map
        .into_iter()
        .map(|(phase, count)| PhaseDistribution { phase, count })
        .collect();
    result.sort_by_key(|a| std::cmp::Reverse(a.count));
    result
}

/// Compute Shannon diversity index from skill counts.
/// H = -Σ (p_i * ln(p_i)) where p_i = count_i / total
pub fn compute_shannon_index(skills: &[SkillDistribution]) -> f64 {
    let total: usize = skills.iter().map(|s| s.count).sum();
    if total == 0 {
        return 0.0;
    }
    let mut h = 0.0;
    for s in skills {
        if s.count > 0 {
            let p = s.count as f64 / total as f64;
            h -= p * p.ln();
        }
    }
    h
}

/// Compute Simpson diversity index from skill counts.
/// D = 1 - Σ(n_i * (n_i - 1)) / (N * (N - 1))
pub fn compute_simpson_index(skills: &[SkillDistribution]) -> f64 {
    let total: usize = skills.iter().map(|s| s.count).sum();
    if total < 2 {
        return 0.0;
    }
    let n = total as f64;
    let numerator: f64 = skills
        .iter()
        .map(|s| {
            let ni = s.count as f64;
            ni * (ni - 1.0)
        })
        .sum();
    1.0 - numerator / (n * (n - 1.0))
}

/// Escape a CSV field: wrap in quotes if it contains comma, quote, or newline.
pub fn csv_escape(field: &str) -> String {
    if field.contains(',') || field.contains('"') || field.contains('\n') {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

/// GET /api/v1/population/stats — Population evolution statistics.
pub async fn population_stats(State(state): State<AppState>) -> impl IntoResponse {
    let agents = state.agents.lock().await;
    let tick = *state.tick_rx.borrow();

    let alive_agents: Vec<&AgentDto> = agents.iter().filter(|a| a.alive).collect();
    let dead_count = agents.iter().filter(|a| !a.alive).count();
    let alive_count = alive_agents.len();

    let total_tokens: u64 = alive_agents.iter().map(|a| a.tokens).sum();
    let total_money: u64 = alive_agents.iter().map(|a| a.money).sum();

    let skill_distribution = compute_skill_distribution(&agents);
    let phase_distribution = compute_phase_distribution(&agents);

    let shannon_index = compute_shannon_index(&skill_distribution);
    let simpson_index = compute_simpson_index(&skill_distribution);

    let avg_fitness = if alive_count > 0 {
        alive_agents
            .iter()
            .map(|a| a.ticks_survived as f64)
            .sum::<f64>()
            / alive_count as f64
    } else {
        0.0
    };

    let max_generation = agents.iter().map(|a| a.generation).max().unwrap_or(0);

    let birth_rate = if tick > 0 {
        (agents.len() as f64 / tick as f64) * 1000.0
    } else {
        0.0
    };
    let death_rate = if tick > 0 {
        (dead_count as f64 / tick as f64) * 1000.0
    } else {
        0.0
    };

    let response = PopulationStatsResponse {
        tick,
        alive_count,
        dead_count,
        total_spawned: agents.len(),
        birth_rate,
        death_rate,
        total_tokens,
        total_money,
        skill_distribution,
        phase_distribution,
        shannon_index,
        simpson_index,
        avg_fitness,
        max_generation,
    };

    Json(response).into_response()
}

/// Query parameters for population timeline.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct PopulationTimelineQuery {
    pub from_tick: Option<u64>,
    pub to_tick: Option<u64>,
    pub interval: Option<u64>,
}

/// GET /api/v1/population/timeline — Population evolution over time.
pub async fn population_timeline(
    State(state): State<AppState>,
    Query(query): Query<PopulationTimelineQuery>,
) -> impl IntoResponse {
    let agents = state.agents.lock().await;
    let current_tick = *state.tick_rx.borrow();
    let _interval = query.interval.unwrap_or(100);
    let from_tick = query.from_tick.unwrap_or(0);
    let to_tick = query.to_tick.unwrap_or(current_tick);

    // Try to use snapshot data for accurate historical data
    if let Some(ref snapshot_store) = state.snapshot_store {
        let store = snapshot_store.lock().await;
        if let Ok(snapshots) = store.list(None, None, Some(1000)) {
            if !snapshots.is_empty() {
                let mut timeline: Vec<PopulationTimelinePoint> = Vec::new();
                for snap in &snapshots {
                    if snap.tick < from_tick || snap.tick > to_tick {
                        continue;
                    }
                    timeline.push(PopulationTimelinePoint {
                        tick: snap.tick,
                        alive_count: snap.active_agents as usize,
                        dead_count: snap.total_population.saturating_sub(snap.active_agents)
                            as usize,
                        total_tokens: snap.gdp,
                        total_money: 0,
                        skill_types: snap.skill_distribution_top5.len(),
                        avg_fitness: 0.0,
                    });
                }
                // Append current state as final point
                let alive_count = agents.iter().filter(|a| a.alive).count();
                let dead_count = agents.iter().filter(|a| !a.alive).count();
                timeline.push(PopulationTimelinePoint {
                    tick: current_tick,
                    alive_count,
                    dead_count,
                    total_tokens: agents.iter().filter(|a| a.alive).map(|a| a.tokens).sum(),
                    total_money: agents.iter().filter(|a| a.alive).map(|a| a.money).sum(),
                    skill_types: compute_skill_distribution(&agents).len(),
                    avg_fitness: if alive_count > 0 {
                        agents
                            .iter()
                            .filter(|a| a.alive)
                            .map(|a| a.ticks_survived as f64)
                            .sum::<f64>()
                            / alive_count as f64
                    } else {
                        0.0
                    },
                });
                return Json(timeline).into_response();
            }
        }
    }

    // Fallback: return current state as a single data point
    let alive_count = agents.iter().filter(|a| a.alive).count();
    let dead_count = agents.iter().filter(|a| !a.alive).count();
    let timeline = vec![PopulationTimelinePoint {
        tick: current_tick,
        alive_count,
        dead_count,
        total_tokens: agents.iter().filter(|a| a.alive).map(|a| a.tokens).sum(),
        total_money: agents.iter().filter(|a| a.alive).map(|a| a.money).sum(),
        skill_types: compute_skill_distribution(&agents).len(),
        avg_fitness: if alive_count > 0 {
            agents
                .iter()
                .filter(|a| a.alive)
                .map(|a| a.ticks_survived as f64)
                .sum::<f64>()
                / alive_count as f64
        } else {
            0.0
        },
    }];
    Json(timeline).into_response()
}

/// GET /api/v1/population/species — Species (skill-signature) classification.
pub async fn population_species(State(state): State<AppState>) -> impl IntoResponse {
    let agents = state.agents.lock().await;

    // Group agents by their sorted skill signature
    let mut species_map: HashMap<String, Vec<&AgentDto>> = HashMap::new();
    for agent in agents.iter().filter(|a| a.alive) {
        let mut skills: Vec<&String> = agent.skills.keys().collect();
        skills.sort();
        let sig = skills
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join(",");
        species_map.entry(sig).or_default().push(agent);
    }

    let species: Vec<SpeciesEntry> = species_map
        .into_iter()
        .map(|(sig, agents)| SpeciesEntry {
            skill_signature: if sig.is_empty() {
                "(no skills)".to_string()
            } else {
                sig
            },
            count: agents.len(),
            generations: agents.iter().map(|a| a.generation).collect(),
            alive: agents.len(), // all alive since we filtered
        })
        .collect();

    Json(species).into_response()
}

/// GET /api/v1/population/diversity — Gene diversity metrics.
pub async fn population_diversity(State(state): State<AppState>) -> impl IntoResponse {
    let agents = state.agents.lock().await;
    let skill_distribution = compute_skill_distribution(&agents);
    let shannon = compute_shannon_index(&skill_distribution);
    let simpson = compute_simpson_index(&skill_distribution);

    let alive_count = agents.iter().filter(|a| a.alive).count();

    // Skill richness (number of distinct skills)
    let skill_richness = skill_distribution.len();

    // Evenness (Shannon evenness = H / ln(S))
    let evenness = if skill_richness > 1 && shannon > 0.0 {
        shannon / (skill_richness as f64).ln()
    } else {
        0.0
    };

    // Generation diversity
    let max_gen = agents.iter().map(|a| a.generation).max().unwrap_or(0);
    let avg_gen = if alive_count > 0 {
        agents
            .iter()
            .filter(|a| a.alive)
            .map(|a| a.generation as f64)
            .sum::<f64>()
            / alive_count as f64
    } else {
        0.0
    };

    #[derive(Serialize)]
    struct DiversityResponse {
        shannon_index: f64,
        simpson_index: f64,
        skill_richness: usize,
        evenness: f64,
        max_generation: u32,
        avg_generation: f64,
        alive_count: usize,
    }

    Json(DiversityResponse {
        shannon_index: shannon,
        simpson_index: simpson,
        skill_richness,
        evenness,
        max_generation: max_gen,
        avg_generation: avg_gen,
        alive_count,
    })
    .into_response()
}

/// GET /api/v1/population/events — Evolution event types supported.
/// Returns the list of evolution event types that the system tracks.
/// For real-time event streaming, use the SSE endpoint at /api/v1/world/events.
pub async fn population_events(State(_state): State<AppState>) -> impl IntoResponse {
    #[derive(Serialize)]
    struct EventInfo {
        event_type: &'static str,
        description: &'static str,
    }

    let events = vec![
        EventInfo {
            event_type: "offspring_mutated",
            description: "An offspring was produced with mutations from two parents",
        },
        EventInfo {
            event_type: "skill_level_up",
            description: "An agent's skill increased in level",
        },
        EventInfo {
            event_type: "skill_mutated",
            description: "An agent's skill was mutated",
        },
        EventInfo {
            event_type: "fitness_evaluated",
            description: "An agent's fitness was evaluated",
        },
        EventInfo {
            event_type: "agent_spawned",
            description: "A new agent was spawned",
        },
        EventInfo {
            event_type: "agent_died",
            description: "An agent died",
        },
    ];

    Json(events).into_response()
}

/// GET /api/v1/population/genealogy/:id — Genealogy tree for a specific agent.
pub async fn population_genealogy(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let agents = state.agents.lock().await;

    // Find the target agent
    let target = match agents.iter().find(|a| a.id == id) {
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

    // Build genealogy by recursively finding parents (max depth 10)
    fn build_tree(agents: &[AgentDto], agent: &AgentDto, depth: usize) -> GenealogyNode {
        let children: Vec<GenealogyNode> = if depth < 10 {
            agents
                .iter()
                .filter(|a| a.parent_ids.contains(&agent.id))
                .map(|child| build_tree(agents, child, depth + 1))
                .collect()
        } else {
            Vec::new()
        };
        GenealogyNode {
            agent: agent.clone(),
            children,
        }
    }

    // Find all ancestors up the tree
    let mut all_ancestors: Vec<AgentDto> = Vec::new();
    let mut queue: Vec<String> = target.parent_ids.clone();
    let mut visited: std::collections::HashSet<String> = std::collections::HashSet::new();
    while let Some(pid) = queue.pop() {
        if visited.contains(&pid) {
            continue;
        }
        visited.insert(pid.clone());
        if let Some(parent) = agents.iter().find(|a| a.id == pid) {
            queue.extend(parent.parent_ids.iter().cloned());
            all_ancestors.push(parent.clone());
        }
    }

    // Build descendant tree from target
    let tree = build_tree(&agents, &target, 0);

    #[derive(Serialize)]
    struct GenealogyResponse {
        target: GenealogyNode,
        ancestors: Vec<AgentDto>,
    }

    Json(GenealogyResponse {
        target: tree,
        ancestors: all_ancestors,
    })
    .into_response()
}

/// GET /api/v1/population/export/csv — Export population data as CSV.
pub async fn population_export_csv(State(state): State<AppState>) -> impl IntoResponse {
    let agents = state.agents.lock().await;
    let tick = *state.tick_rx.borrow();

    let estimated_capacity = agents.len() * 256;
    let mut csv = String::with_capacity(estimated_capacity + 128);
    csv.push_str("agent_id,name,phase,alive,tokens,money,ticks_survived,generation,parent_ids,skills,personality\n");

    for agent in agents.iter() {
        let parent_ids = csv_escape(&agent.parent_ids.join(";"));
        let skills = csv_escape(
            &agent
                .skills
                .iter()
                .map(|(k, v)| format!("{}:{}", k, v))
                .collect::<Vec<_>>()
                .join(","),
        );
        let personality = csv_escape(&agent.personality);
        let name = csv_escape(&agent.name);
        csv.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{}\n",
            agent.id,
            name,
            agent.phase,
            agent.alive,
            agent.tokens,
            agent.money,
            agent.ticks_survived,
            agent.generation,
            parent_ids,
            skills,
            personality,
        ));
    }

    let body = axum::body::Body::from(csv);
    let mut resp = axum::response::Response::new(body);
    *resp.status_mut() = StatusCode::OK;
    resp.headers_mut()
        .insert("content-type", "text/csv".parse().unwrap());
    resp.headers_mut().insert(
        "content-disposition",
        format!("attachment; filename=\"population_tick_{}.csv\"", tick)
            .parse()
            .unwrap(),
    );
    resp
}

/// Population evolution routes.
pub fn population_routes() -> axum::Router<AppState> {
    axum::Router::new()
        .route("/population/stats", get(population_stats))
        .route("/population/species", get(population_species))
        .route("/population/diversity", get(population_diversity))
        .route("/population/events", get(population_events))
        .route(
            "/population/genealogy/:id",
            get(population_genealogy),
        )
        .route("/population/timeline", get(population_timeline))
        .route("/population/export/csv", get(population_export_csv))
}
