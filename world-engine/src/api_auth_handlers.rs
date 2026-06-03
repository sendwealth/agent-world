use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::*,
    Json,
};
use serde::{Deserialize, Serialize};

use crate::api::{AppState, ErrorResponse};
use crate::auth::{extractors::require_capability, Capability, HumanRole, HumanUser, RequireAuth};

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    username: String,
    password: String,
    role: HumanRole,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Debug, Serialize)]
struct AuthResponse {
    user: HumanUser,
    token: String,
}

pub async fn auth_register(
    State(state): State<AppState>,
    Json(body): Json<RegisterRequest>,
) -> impl IntoResponse {
    let mut store = state.auth_store.lock().await;
    match store.register(&body.username, &body.password, body.role) {
        Ok(user) => (StatusCode::CREATED, Json(user)).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: e })).into_response(),
    }
}

pub async fn auth_login(
    State(state): State<AppState>,
    Json(body): Json<LoginRequest>,
) -> impl IntoResponse {
    let mut store = state.auth_store.lock().await;
    match store.login(&body.username, &body.password) {
        Ok((user, token)) => (StatusCode::OK, Json(AuthResponse { user, token })).into_response(),
        Err(e) => (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error: e })).into_response(),
    }
}

pub async fn auth_me(
    State(state): State<AppState>,
    RequireAuth(auth): RequireAuth,
) -> impl IntoResponse {
    let store = state.auth_store.lock().await;
    match store.get_user(&auth.user_id) {
        Some(user) => Json(user).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "User not found".into(),
            }),
        )
            .into_response(),
    }
}

pub async fn auth_list_users(
    State(state): State<AppState>,
    RequireAuth(auth): RequireAuth,
) -> impl IntoResponse {
    if let Err(e) = require_capability(&auth, Capability::CreateAgent) {
        return e.into_response();
    }
    let store = state.auth_store.lock().await;
    Json(store.list_users()).into_response()
}

pub async fn auth_update_role(
    State(state): State<AppState>,
    RequireAuth(auth): RequireAuth,
    Path(user_id): Path<String>,
    Json(body): Json<UpdateRoleRequest>,
) -> impl IntoResponse {
    if let Err(e) = require_capability(&auth, Capability::CreateAgent) {
        return e.into_response();
    }
    let mut store = state.auth_store.lock().await;
    match store.update_role(&user_id, body.role) {
        Ok(user) => Json(user).into_response(),
        Err(e) => (StatusCode::NOT_FOUND, Json(ErrorResponse { error: e })).into_response(),
    }
}

#[derive(Debug, Deserialize)]
pub struct UpdateRoleRequest {
    role: HumanRole,
}

/// Auth routes.
pub fn auth_routes() -> axum::Router<AppState> {
    axum::Router::new()
        .route("/auth/register", post(auth_register))
        .route("/auth/login", post(auth_login))
        .route("/auth/me", get(auth_me))
        .route("/auth/users", get(auth_list_users))
        .route("/auth/users/:id/role", post(auth_update_role))
}
