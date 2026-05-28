use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::*,
};
use serde::Deserialize;

use crate::api::{api_err, api_ok, AppState};

// ══════════════════════════════════════════════════════════════════════════════
// Reputation API handlers
// ══════════════════════════════════════════════════════════════════════════════

/// GET /api/v1/reputation/:agent_id — Get agent's reputation score.
pub async fn rep_get_reputation(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
) -> impl IntoResponse {
    let rep = match &state.reputation_system {
        Some(r) => r.clone(),
        None => {
            return api_err(
                StatusCode::SERVICE_UNAVAILABLE,
                "reputation system not configured",
            )
        }
    };
    let rep = rep.lock().await;
    let score = rep.get_reputation(&agent_id);
    api_ok(serde_json::json!({"agent_id": agent_id, "reputation": score}))
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct RepRankingsQuery {
    pub limit: Option<usize>,
}

/// GET /api/v1/reputation/rankings — Get reputation rankings.
pub async fn rep_rankings(
    State(state): State<AppState>,
    Query(params): Query<RepRankingsQuery>,
) -> impl IntoResponse {
    let rep = match &state.reputation_system {
        Some(r) => r.clone(),
        None => {
            return api_err(
                StatusCode::SERVICE_UNAVAILABLE,
                "reputation system not configured",
            )
        }
    };
    let rep = rep.lock().await;
    let rankings = rep.get_rankings(params.limit.unwrap_or(50));
    api_ok(rankings)
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct RepLowReputationQuery {
    pub threshold: Option<f64>,
}

/// GET /api/v1/reputation/low-reputation — Get agents with low reputation.
pub async fn rep_low_reputation(
    State(state): State<AppState>,
    Query(params): Query<RepLowReputationQuery>,
) -> impl IntoResponse {
    let rep = match &state.reputation_system {
        Some(r) => r.clone(),
        None => {
            return api_err(
                StatusCode::SERVICE_UNAVAILABLE,
                "reputation system not configured",
            )
        }
    };
    let rep = rep.lock().await;
    let agents = rep.get_low_reputation_agents(params.threshold.unwrap_or(-10.0));
    api_ok(agents)
}

/// GET /api/v1/reputation/config — Get the reputation system configuration.
pub async fn rep_get_config(State(state): State<AppState>) -> impl IntoResponse {
    let rep = match &state.reputation_system {
        Some(r) => r.clone(),
        None => {
            return api_err(
                StatusCode::SERVICE_UNAVAILABLE,
                "reputation system not configured",
            )
        }
    };
    let rep = rep.lock().await;
    api_ok(rep.config())
}

/// Reputation routes.
pub fn reputation_routes() -> axum::Router<AppState> {
    axum::Router::new()
        .route("/api/v1/reputation/:agent_id", get(rep_get_reputation))
        .route("/api/v1/reputation/rankings", get(rep_rankings))
        .route("/api/v1/reputation/low-reputation", get(rep_low_reputation))
        .route("/api/v1/reputation/config", get(rep_get_config))
}
