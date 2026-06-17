//! REST API handlers for agent-spawned sub-worlds (Phase 5.7).
//!
//! Routes mounted under `/api/v1/subworlds`.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::*,
    Json,
};

use crate::api::{api_err, api_ok, AppState};
use crate::federation::subworld::{
    RestCreateSubWorld, RestDissolve, RestEvict, RestInvite, RestMigrateIn,
    RestSetStatus, RestUpdateGovernance,
};

/// Helper: get the SubWorldManager from AppState, or return a 503.
macro_rules! get_manager {
    ($state:expr) => {
        match &$state.subworld_manager {
            Some(m) => m.clone(),
            None => {
                return api_err(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "subworld manager not configured",
                )
            }
        }
    };
}

/// POST /api/v1/subworlds — Create a new sub-world.
pub async fn subworld_create(
    State(state): State<AppState>,
    Json(body): Json<RestCreateSubWorld>,
) -> impl IntoResponse {
    let mgr = get_manager!(state);
    match mgr
        .create_subworld(
            &body.founder_agent_id,
            body.founder_reputation,
            &body.parent_world_id,
            &body.name,
            &body.description,
            body.governance,
            body.tick_interval_ms,
            body.genesis_config,
        )
        .await
    {
        Ok(sw) => api_ok(&sw),
        Err(e) => api_err(StatusCode::BAD_REQUEST, e),
    }
}

/// GET /api/v1/subworlds — List all sub-worlds.
pub async fn subworld_list(State(state): State<AppState>) -> impl IntoResponse {
    let mgr = get_manager!(state);
    let list = mgr.registry().list_all().await;
    api_ok(&list)
}

/// GET /api/v1/subworlds/:id — Get a sub-world by ID.
pub async fn subworld_get(
    State(state): State<AppState>,
    Path(world_id): Path<String>,
) -> impl IntoResponse {
    let mgr = get_manager!(state);
    match mgr.registry().get(&world_id).await {
        Some(sw) => api_ok(&sw),
        None => api_err(StatusCode::NOT_FOUND, "sub-world not found"),
    }
}

/// GET /api/v1/subworlds/parent/:parent_id — List children of a parent world.
pub async fn subworld_list_children(
    State(state): State<AppState>,
    Path(parent_id): Path<String>,
) -> impl IntoResponse {
    let mgr = get_manager!(state);
    let children = mgr.registry().list_children(&parent_id).await;
    api_ok(&children)
}

/// PUT /api/v1/subworlds/:id/governance — Update governance config.
pub async fn subworld_update_governance(
    State(state): State<AppState>,
    Path(world_id): Path<String>,
    Json(body): Json<RestUpdateGovernance>,
) -> impl IntoResponse {
    let mgr = get_manager!(state);
    match mgr
        .update_governance(&world_id, &body.actor_agent_id, body.governance)
        .await
    {
        Ok(sw) => api_ok(&sw),
        Err(e) => api_err(StatusCode::BAD_REQUEST, e),
    }
}

/// PUT /api/v1/subworlds/:id/status — Change status (freeze, activate, dissolve-flag).
pub async fn subworld_set_status(
    State(state): State<AppState>,
    Path(world_id): Path<String>,
    Json(body): Json<RestSetStatus>,
) -> impl IntoResponse {
    let mgr = get_manager!(state);
    match mgr
        .set_status(&world_id, &body.actor_agent_id, body.status)
        .await
    {
        Ok(sw) => api_ok(&sw),
        Err(e) => api_err(StatusCode::BAD_REQUEST, e),
    }
}

/// POST /api/v1/subworlds/:id/invite — Invite an agent (precondition check only).
pub async fn subworld_invite(
    State(state): State<AppState>,
    Path(world_id): Path<String>,
    Json(body): Json<RestInvite>,
) -> impl IntoResponse {
    let mgr = get_manager!(state);
    match mgr
        .invite_member(&world_id, &body.inviter_agent_id, &body.invitee_agent_id)
        .await
    {
        Ok(()) => api_ok(serde_json::json!({ "ok": true, "message": "invitation valid" })),
        Err(e) => api_err(StatusCode::BAD_REQUEST, e),
    }
}

/// POST /api/v1/subworlds/:id/migrate — Submit a migration into this sub-world.
pub async fn subworld_migrate(
    State(state): State<AppState>,
    Path(world_id): Path<String>,
    Json(body): Json<RestMigrateIn>,
) -> impl IntoResponse {
    let mgr = get_manager!(state);
    let snapshot: crate::federation::migration::AgentSnapshot = body.into();
    match mgr.migrate_in(&world_id, snapshot).await {
        Ok(app) => api_ok(&app),
        Err(e) => api_err(StatusCode::BAD_REQUEST, e),
    }
}

/// POST /api/v1/subworlds/:id/confirm — Confirm a completed migration (add member + credit pool).
pub async fn subworld_confirm_migration(
    State(state): State<AppState>,
    Path(world_id): Path<String>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let mgr = get_manager!(state);
    let agent_id = body
        .get("agent_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let entry_contribution = body
        .get("entry_contribution")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    if agent_id.is_empty() {
        return api_err(StatusCode::BAD_REQUEST, "agent_id is required");
    }
    match mgr
        .confirm_migration(&world_id, &agent_id, entry_contribution)
        .await
    {
        Ok(sw) => api_ok(&sw),
        Err(e) => api_err(StatusCode::BAD_REQUEST, e),
    }
}

/// POST /api/v1/subworlds/:id/evict — Evict a member (founder only).
pub async fn subworld_evict(
    State(state): State<AppState>,
    Path(world_id): Path<String>,
    Json(body): Json<RestEvict>,
) -> impl IntoResponse {
    let mgr = get_manager!(state);
    match mgr
        .evict_member(&world_id, &body.founder_agent_id, &body.target_agent_id)
        .await
    {
        Ok(sw) => api_ok(&sw),
        Err(e) => api_err(StatusCode::BAD_REQUEST, e),
    }
}

/// DELETE /api/v1/subworlds/:id — Dissolve a sub-world (founder only).
pub async fn subworld_dissolve(
    State(state): State<AppState>,
    Path(world_id): Path<String>,
    Json(body): Json<RestDissolve>,
) -> impl IntoResponse {
    let mgr = get_manager!(state);
    match mgr.dissolve(&world_id, &body.founder_agent_id).await {
        Ok(sw) => api_ok(&sw),
        Err(e) => api_err(StatusCode::BAD_REQUEST, e),
    }
}

/// Sub-world REST routes.
pub fn subworld_routes() -> axum::Router<AppState> {
    axum::Router::new()
        .route("/subworlds", post(subworld_create))
        .route("/subworlds", get(subworld_list))
        .route("/subworlds/parent/:parent_id", get(subworld_list_children))
        .route("/subworlds/:id", get(subworld_get))
        .route("/subworlds/:id/governance", put(subworld_update_governance))
        .route("/subworlds/:id/status", put(subworld_set_status))
        .route("/subworlds/:id/invite", post(subworld_invite))
        .route("/subworlds/:id/migrate", post(subworld_migrate))
        .route(
            "/subworlds/:id/confirm",
            post(subworld_confirm_migration),
        )
        .route("/subworlds/:id/evict", post(subworld_evict))
        .route("/subworlds/:id", delete(subworld_dissolve))
}

// ── Tests ─────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use crate::api::{build_full_router, AppState, TestOverrides};
    use crate::economy::task::TaskBoard;
    use crate::federation::migration::{MigrationManager, MigrationPolicy};
    use crate::federation::subworld::{
        GovernanceConfig, RestCreateSubWorld, SubWorldManager,
    };
    use crate::api::SharedWAL;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use tower::ServiceExt;

    fn test_wal() -> SharedWAL {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.keep();
        Arc::new(Mutex::new(crate::wal::WAL::new(&path)))
    }

    fn test_state() -> AppState {
        let board = Arc::new(Mutex::new(TaskBoard::new()));
        let wal = test_wal();
        let event_bus = Arc::new(crate::world::state::EventBus::new(256));
        let migration_mgr = Arc::new(MigrationManager::new(
            MigrationPolicy::default(),
            event_bus.clone(),
        ));
        let subworld_mgr = Arc::new(SubWorldManager::new(
            event_bus.clone(),
            migration_mgr.clone(),
        ));
        AppState::new(
            board,
            wal,
            TestOverrides {
                event_bus: Some(event_bus.clone()),
                migration_manager: Some(Arc::new(Mutex::new(
                    MigrationManager::new(MigrationPolicy::default(), event_bus.clone()),
                ))),
                subworld_manager: Some(subworld_mgr),
                ..Default::default()
            },
        )
    }

    fn app() -> axum::Router {
        build_full_router(test_state())
    }

    async fn create_subworld_via_api(
        app: &axum::Router,
        founder: &str,
        reputation: f64,
        parent: &str,
        name: &str,
    ) -> String {
        let body = serde_json::json!({
            "founder_agent_id": founder,
            "founder_reputation": reputation,
            "parent_world_id": parent,
            "name": name,
            "description": "",
            "governance": GovernanceConfig::default(),
            "tick_interval_ms": 1000u64,
            "genesis_config": serde_json::Value::Null,
        });
        // Use the strongly-typed struct path via the same JSON
        let _typed: RestCreateSubWorld = serde_json::from_value(body.clone()).unwrap();
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/subworlds")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        json["data"]["world_id"]
            .as_str()
            .unwrap()
            .to_string()
    }

    #[tokio::test]
    async fn e2e_create_list_get() {
        let app = app();
        let id = create_subworld_via_api(&app, "founder-1", 100.0, "parent-w", "Alpha").await;
        assert!(!id.is_empty());

        // List should contain it
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/v1/subworlds")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let arr = json["data"].as_array().unwrap();
        assert_eq!(arr.len(), 1);

        // GET by id
        let resp = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/api/v1/subworlds/{}", id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn e2e_create_low_reputation_fails() {
        let app = app();
        let body = serde_json::json!({
            "founder_agent_id": "low-rep",
            "founder_reputation": 1.0,
            "parent_world_id": "p",
            "name": "Fail",
            "tick_interval_ms": 1000u64,
        });
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/subworlds")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn e2e_full_create_migrate_evict_dissolve() {
        let app = app();

        // Step 1: founder creates an open sub-world
        let id = create_subworld_via_api(
            &app,
            "founder-1",
            100.0,
            "parent-w",
            "Open Colony",
        )
        .await;

        // Step 2: another agent migrates in
        let mig_body = serde_json::json!({
            "agent_id": "immigrant-1",
            "name": "Immigrant One",
            "phase": "adult",
            "tokens": 50000u64,
            "money": 1000u64,
            "reputation": 60.0,
            "skills": {},
            "source_world_id": "parent-w",
        });
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/v1/subworlds/{}/migrate", id))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&mig_body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        // Step 3: confirm migration → adds member
        let confirm_body = serde_json::json!({
            "agent_id": "immigrant-1",
            "entry_contribution": 200u64,
        });
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/v1/subworlds/{}/confirm", id))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&confirm_body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["data"]["members"].as_array().unwrap().len(), 2);

        // Step 4: founder evicts the immigrant
        let evict_body = serde_json::json!({
            "founder_agent_id": "founder-1",
            "target_agent_id": "immigrant-1",
        });
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/v1/subworlds/{}/evict", id))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&evict_body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        // Step 5: founder dissolves the sub-world
        let dissolve_body = serde_json::json!({
            "founder_agent_id": "founder-1",
        });
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri(format!("/api/v1/subworlds/{}", id))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&dissolve_body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        // Confirm gone
        let resp = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/api/v1/subworlds/{}", id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn e2e_governance_update_by_founder() {
        let app = app();
        let id = create_subworld_via_api(&app, "f1", 100.0, "p", "Gov").await;

        let gov_body = serde_json::json!({
            "actor_agent_id": "f1",
            "governance": {
                "min_reputation": 25.0,
                "open_join": false,
                "entry_token_cost": 1000u64,
                "max_members": 10u32,
                "custom_rules": [],
            },
        });
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri(format!("/api/v1/subworlds/{}/governance", id))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&gov_body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }
}
