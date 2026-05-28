use std::collections::HashMap;

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::*,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::api::{AppState, ErrorResponse, AgentRecord, ExternalAgent, Position, ALLOWED_ACTIONS, SharedExternalAgents};
use crate::world::event::WorldEvent;

// ── Request Types ──────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct RegisterAgentRequest {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub config: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct AgentActionRequest {
    pub action: String,
    #[serde(default)]
    pub params: serde_json::Value,
}

// ── Third-Party Agent API Handlers ────────────────────────

/// Register a new third-party agent.
pub async fn register_external_agent(
    State(state): State<AppState>,
    Json(body): Json<RegisterAgentRequest>,
) -> impl IntoResponse {
    if body.name.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "name is required".into() })).into_response();
    }

    let agent_id = Uuid::new_v4().to_string();
    let api_key = Uuid::new_v4().to_string();
    let tick = *state.tick_rx.borrow();

    let name = body.name.clone();
    let agent = ExternalAgent {
        agent_id: agent_id.clone(),
        name: name.clone(),
        api_key: api_key.clone(),
        capabilities: body.capabilities,
        config: body.config,
        alive: true,
        phase: "exploration".to_string(),
        tokens: 100_000,
        money: 0,
        position: Position { x: 0, y: 0 },
        registered_tick: tick,
    };

    {
        let mut external = state.external_agents.lock().await;
        external.insert(agent_id.clone(), agent);
    }

    // Also add to the shared agents list for world_stats compatibility
    {
        let mut agents = state.agents.lock().await;
        agents.push(AgentRecord {
            id: agent_id.clone(),
            name: name.clone(),
            phase: "exploration".to_string(),
            tokens: 100_000,
            money: 0,
            alive: true,
            ticks_survived: 0,
            personality: String::new(),
            parent_ids: Vec::new(),
            generation: 0,
            skills: HashMap::new(),
        });
    }

    (StatusCode::CREATED, Json(serde_json::json!({
        "agent_id": agent_id,
        "api_key": api_key,
        "name": name,
    }))).into_response()
}

/// Deregister (remove) a third-party agent.
pub async fn deregister_external_agent(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let agent_id = {
        let mut external = state.external_agents.lock().await;
        match external.remove(&id) {
            Some(agent) => agent.agent_id,
            None => return (StatusCode::NOT_FOUND, Json(ErrorResponse { error: "agent not found".into() })).into_response(),
        }
    };

    // Also remove from the shared agents list
    {
        let mut agents = state.agents.lock().await;
        agents.retain(|a| a.id != agent_id);
    }

    (StatusCode::OK, Json(serde_json::json!({
        "deregistered": agent_id,
    }))).into_response()
}

/// Execute an action as a third-party agent.
pub async fn execute_agent_action(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<AgentActionRequest>,
) -> impl IntoResponse {
    // Validate action
    if !ALLOWED_ACTIONS.contains(&body.action.as_str()) {
        return (StatusCode::BAD_REQUEST, Json(ErrorResponse {
            error: format!("unknown action '{}'", body.action),
        })).into_response();
    }

    // Check agent exists and is alive
    let mut external = state.external_agents.lock().await;
    let agent = match external.get_mut(&id) {
        Some(a) if a.alive => a,
        Some(_) => return (StatusCode::GONE, Json(ErrorResponse { error: "agent is dead".into() })).into_response(),
        None => return (StatusCode::NOT_FOUND, Json(ErrorResponse { error: "agent not found".into() })).into_response(),
    };

    // Execute action — update position for "move", etc.
    let tick = *state.tick_rx.borrow();
    let success = match body.action.as_str() {
        "move" => {
            if let Some(dir) = body.params.get("direction").and_then(|d| d.as_str()) {
                let distance = body.params.get("distance")
                    .and_then(|d| d.as_u64())
                    .unwrap_or(1) as i64;
                match dir {
                    "north" => agent.position.y += distance,
                    "south" => agent.position.y -= distance,
                    "east" => agent.position.x += distance,
                    "west" => agent.position.x -= distance,
                    _ => {}
                }
            }
            true
        }
        "gather" => {
            agent.money += 10;
            true
        }
        "rest" => true,
        "explore" => true,
        "communicate" => true,
        "trade" => true,
        "build" => true,
        "claim_task" => true,
        "submit_task" => true,
        _ => false,
    };

    // Auto-advance the World Engine tick on every agent action.
    // This ensures that `GET /api/v1/world/stats` reflects agent activity
    // even when no external tick scheduler is driving the game loop.
    {
        let current = *state.tick_rx.borrow();
        let new_tick = current + 1;
        state.event_bus.emit(WorldEvent::TickAdvanced { tick: new_tick });
        let _ = state.tick_tx.send(new_tick);
    }

    // Sync changes back to the shared agents list so GET /api/v1/agents reflects updates
    {
        let mut agents = state.agents.lock().await;
        if let Some(record) = agents.iter_mut().find(|a| a.id == id) {
            record.money = agent.money;
            record.tokens = agent.tokens;
            record.alive = agent.alive;
        }
    }

    let action_name = body.action.clone();

    (StatusCode::OK, Json(serde_json::json!({
        "action": action_name,
        "success": success,
        "tick": tick,
    }))).into_response()
}

/// Get perception data for a third-party agent.
pub async fn get_agent_perception(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let external = state.external_agents.lock().await;
    let agent = match external.get(&id) {
        Some(a) if a.alive => a,
        Some(_) => return (StatusCode::GONE, Json(ErrorResponse { error: "agent is dead".into() })).into_response(),
        None => return (StatusCode::NOT_FOUND, Json(ErrorResponse { error: "agent not found".into() })).into_response(),
    };

    let tick = *state.tick_rx.borrow();

    // Build perception from the world state
    let agents = state.agents.lock().await;
    let nearby_agents: Vec<serde_json::Value> = agents.iter()
        .filter(|a| a.alive && a.id != id)
        .take(10)
        .map(|a| serde_json::json!({
            "id": a.id,
            "name": a.name,
        }))
        .collect();

    let nearby_resources: Vec<serde_json::Value> = vec![
        serde_json::json!({ "type": "food", "position": { "x": 1, "y": 1 } }),
        serde_json::json!({ "type": "wood", "position": { "x": 3, "y": 5 } }),
    ];

    (StatusCode::OK, Json(serde_json::json!({
        "agent_id": id,
        "nearby_agents": nearby_agents,
        "nearby_resources": nearby_resources,
        "position": agent.position,
        "world_tick": tick,
    }))).into_response()
}

/// Get the status of a third-party agent.
pub async fn get_agent_status(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let external = state.external_agents.lock().await;
    let agent = match external.get(&id) {
        Some(a) => a,
        None => return (StatusCode::NOT_FOUND, Json(ErrorResponse { error: "agent not found".into() })).into_response(),
    };

    let tick = *state.tick_rx.borrow();

    (StatusCode::OK, Json(serde_json::json!({
        "agent_id": agent.agent_id,
        "name": agent.name,
        "alive": agent.alive,
        "phase": agent.phase,
        "tokens": agent.tokens,
        "money": agent.money,
        "position": agent.position,
        "registered_tick": agent.registered_tick,
        "current_tick": tick,
    }))).into_response()
}

/// Third-party agent API routes.
pub fn agents_ext_routes() -> axum::Router<AppState> {
    axum::Router::new()
        .route("/api/v1/agents/register", post(register_external_agent))
        .route("/api/v1/agents/:id", delete(deregister_external_agent))
        .route("/api/v1/agents/:id/action", post(execute_agent_action))
        .route("/api/v1/agents/:id/perception", get(get_agent_perception))
        .route("/api/v1/agents/:id/status", get(get_agent_status))
}
