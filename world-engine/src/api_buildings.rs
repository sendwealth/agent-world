use std::collections::HashMap;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::*,
    Json,
};
use serde::Deserialize;

use crate::api::{AppState, ErrorResponse};
use crate::world::event::WorldEvent;

pub fn default_owner_type() -> String {
    "personal".to_string()
}
pub fn default_health_restore() -> u32 {
    50
}

#[derive(Debug, Deserialize)]
pub struct BuildRequest {
    building_type: String,
    #[serde(default)]
    x: i32,
    #[serde(default)]
    y: i32,
    #[serde(default = "default_owner_type")]
    owner_type: String,
}

pub async fn build_building(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
    Json(body): Json<BuildRequest>,
) -> impl IntoResponse {
    let building_type = match body.building_type.as_str() {
        "warehouse" => crate::world::map::building::BuildingType::Warehouse,
        "market" => crate::world::map::building::BuildingType::Market,
        "workshop" => crate::world::map::building::BuildingType::Workshop,
        "defense_tower" => crate::world::map::building::BuildingType::DefenseTower,
        "housing" => crate::world::map::building::BuildingType::Housing,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: format!("unknown building_type '{}'", body.building_type),
                }),
            )
                .into_response()
        }
    };

    let owner_type = match body.owner_type.as_str() {
        "personal" => crate::world::map::building::OwnerType::Personal,
        "organization" => crate::world::map::building::OwnerType::Organization,
        _ => crate::world::map::building::OwnerType::Personal,
    };

    // Deduct tokens from agent
    {
        let mut external = state.external_agents.lock().await;
        if let Some(agent) = external.get_mut(&agent_id) {
            if !agent.alive {
                return (
                    StatusCode::GONE,
                    Json(ErrorResponse {
                        error: "agent is dead".into(),
                    }),
                )
                    .into_response();
            }
            let cost = crate::world::map::building::BuildingCost::for_type(building_type);
            if agent.tokens < cost.tokens {
                return (
                    StatusCode::PAYMENT_REQUIRED,
                    Json(ErrorResponse {
                        error: format!(
                            "insufficient tokens: need {}, have {}",
                            cost.tokens, agent.tokens
                        ),
                    }),
                )
                    .into_response();
            }
            agent.tokens -= cost.tokens;
        } else {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "agent not found".into(),
                }),
            )
                .into_response();
        }
    }

    let tick = *state.tick_rx.borrow();
    let mut mgr = state.building_manager.lock().await;
    match mgr.construct(
        building_type,
        (body.x, body.y),
        owner_type,
        agent_id.clone(),
        tick,
    ) {
        Ok(building) => {
            state.event_bus.emit(WorldEvent::BuildingConstructed {
                building_id: building.id.clone(),
                building_type: format!("{:?}", building.building_type).to_lowercase(),
                owner_id: agent_id,
                position: (body.x, body.y),
            });
            (
                StatusCode::CREATED,
                Json(serde_json::to_value(&building).unwrap_or_else(|e| {
                    tracing::error!("failed to serialize building: {}", e);
                    serde_json::json!({"error": "serialization failed"})
                })),
            )
                .into_response()
        }
        Err(e) => (StatusCode::CONFLICT, Json(ErrorResponse { error: e })).into_response(),
    }
}

pub async fn list_buildings(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let mgr = state.building_manager.lock().await;
    let buildings: Vec<_> = if let Some(owner) = params.get("owner") {
        mgr.get_by_owner(owner).into_iter().collect()
    } else {
        mgr.list_all()
    };
    (StatusCode::OK, Json(buildings)).into_response()
}

pub async fn list_buildings_at(
    State(state): State<AppState>,
    Path((x, y)): Path<(i32, i32)>,
) -> impl IntoResponse {
    let mgr = state.building_manager.lock().await;
    let buildings = mgr.get_at((x, y));
    (StatusCode::OK, Json(buildings)).into_response()
}

pub async fn get_building(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let mgr = state.building_manager.lock().await;
    match mgr.get(&id) {
        Some(b) => match serde_json::to_value(b) {
            Ok(v) => (StatusCode::OK, Json(v)).into_response(),
            Err(e) => {
                tracing::error!("failed to serialize building: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "serialization failed"}))).into_response()
            }
        },
        None => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "building not found".into(),
            }),
        )
            .into_response(),
    }
}

#[derive(Debug, Deserialize)]
pub struct MaintainRequest {
    #[serde(default = "default_health_restore")]
    health_restore: u32,
}

pub async fn maintain_building(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<MaintainRequest>,
) -> impl IntoResponse {
    let mut mgr = state.building_manager.lock().await;
    match mgr.maintain(&id, body.health_restore) {
        Ok(building) => {
            state.event_bus.emit(WorldEvent::BuildingMaintained {
                building_id: id,
                health_restored: body.health_restore,
                new_health: building.health,
            });
            (
                StatusCode::OK,
                Json(serde_json::to_value(&building).unwrap_or_else(|e| {
                    tracing::error!("failed to serialize building: {}", e);
                    serde_json::json!({"error": "serialization failed"})
                })),
            )
                .into_response()
        }
        Err(e) => (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: e })).into_response(),
    }
}

pub async fn demolish_building(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let mut mgr = state.building_manager.lock().await;
    match mgr.demolish(&id) {
        Ok(building) => {
            let owner_id = building.owner_id.clone();
            state.event_bus.emit(WorldEvent::BuildingDemolished {
                building_id: id,
                owner_id,
            });
            (
                StatusCode::OK,
                Json(serde_json::to_value(&building).unwrap_or_else(|e| {
                    tracing::error!("failed to serialize building: {}", e);
                    serde_json::json!({"error": "serialization failed"})
                })),
            )
                .into_response()
        }
        Err(e) => (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: e })).into_response(),
    }
}

/// Building management routes.
pub fn building_routes() -> axum::Router<AppState> {
    axum::Router::new()
        .route("/api/v1/agents/:id/build", post(build_building))
        .route("/api/v1/map/buildings", get(list_buildings))
        .route("/api/v1/map/:x/:y/buildings", get(list_buildings_at))
        .route("/api/v1/buildings/:id", get(get_building))
        .route("/api/v1/buildings/:id/maintain", post(maintain_building))
        .route("/api/v1/buildings/:id/demolish", post(demolish_building))
}
