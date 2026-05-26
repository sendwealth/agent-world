use axum::{
    extract::FromRequestParts,
    http::request::Parts,
    response::{IntoResponse, Response},
};
use serde::Serialize;

use super::roles::Capability;
use super::store::SharedAuthStore;

use crate::api::AppState;

// ── Authenticated user info (extracted from JWT) ───────────────

#[derive(Debug, Clone, Serialize)]
pub struct AuthUser {
    pub user_id: String,
    pub role: super::roles::HumanRole,
}

// ── RequireAuth extractor ──────────────────────────────────────
// Returns 401 if no valid token is present.

#[derive(Debug, Clone)]
pub struct RequireAuth(pub AuthUser);

#[axum::async_trait]
impl FromRequestParts<AppState> for RequireAuth {
    type Rejection = AuthError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let auth_user = extract_auth_from_parts(parts, &state.auth_store).await?;
        Ok(RequireAuth(auth_user))
    }
}

// ── OptionalAuth extractor ─────────────────────────────────────
// Returns None (not an error) if no token / invalid token.

#[derive(Debug, Clone)]
pub struct OptionalAuth(pub Option<AuthUser>);

#[axum::async_trait]
impl FromRequestParts<AppState> for OptionalAuth {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let auth_user = extract_auth_from_parts(parts, &state.auth_store).await.ok();
        Ok(OptionalAuth(auth_user))
    }
}

// ── Capability guard (for use in handler body) ─────────────────

pub fn require_capability(auth: &AuthUser, cap: Capability) -> Result<(), AuthError> {
    if auth.role.has_capability(cap) {
        Ok(())
    } else {
        Err(AuthError::Forbidden(format!(
            "Role '{}' does not have '{}' capability",
            auth.role, cap
        )))
    }
}

// ── Error types ────────────────────────────────────────────────

#[derive(Debug)]
pub enum AuthError {
    MissingToken,
    InvalidToken(String),
    Forbidden(String),
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        use axum::http::StatusCode;
        use axum::Json;
        use serde_json::json;

        match self {
            AuthError::MissingToken => (
                StatusCode::UNAUTHORIZED,
                Json(json!({ "error": "Authentication required" })),
            )
                .into_response(),
            AuthError::InvalidToken(msg) => (
                StatusCode::UNAUTHORIZED,
                Json(json!({ "error": msg })),
            )
                .into_response(),
            AuthError::Forbidden(msg) => (
                StatusCode::FORBIDDEN,
                Json(json!({ "error": msg })),
            )
                .into_response(),
        }
    }
}

// ── Internal helper ────────────────────────────────────────────

async fn extract_auth_from_parts(
    parts: &Parts,
    auth_store: &SharedAuthStore,
) -> Result<AuthUser, AuthError> {
    let auth_header = parts
        .headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or(AuthError::MissingToken)?;

    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or(AuthError::InvalidToken(
            "Invalid Authorization header format".into(),
        ))?;

    let store = auth_store.lock().await;
    let claims = store
        .verify_token(token)
        .map_err(AuthError::InvalidToken)?;

    Ok(AuthUser {
        user_id: claims.sub,
        role: claims.role,
    })
}
