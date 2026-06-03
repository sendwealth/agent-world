//! API Key authentication and rate limiting for `/api/v2/*` research endpoints.
//!
//! - `ApiKeyStore` loads keys from the `API_KEYS` env var (comma-separated).
//! - `RateLimiter` implements a per-key token-bucket (60 req/min).
//! - The `auth_middleware` Axum layer extracts `X-API-Key`, validates, and rate-limits.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use axum::{
    extract::{Request, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use tokio::sync::Mutex;

// ── Types ──────────────────────────────────────────────────

/// Shared handle to the API key store + rate limiter.
pub type SharedApiKeyStore = Arc<ApiKeyStore>;

/// In-memory store of valid API keys.
pub struct ApiKeyStore {
    keys: HashMap<String, RateLimiter>,
}

/// Per-key token bucket state.
struct TokenBucket {
    tokens: f64,
    last_refill: Instant,
}

/// Per-key rate limiter wrapping a token bucket.
pub struct RateLimiter {
    bucket: Mutex<TokenBucket>,
    max_tokens: f64,
    refill_per_sec: f64,
}

/// JSON body returned on auth / rate-limit failures.
#[derive(Debug, Serialize)]
pub struct AuthErrorResponse {
    pub error: String,
}

/// Rate-limit headers appended to every v2 response.
#[derive(Debug, Clone, Serialize)]
pub struct RateLimitHeaders {
    /// Maximum requests per minute.
    pub limit: u64,
    /// Remaining requests in the current window.
    pub remaining: u64,
    /// Seconds until the bucket fully resets.
    pub reset: u64,
}

// ── ApiKeyStore ────────────────────────────────────────────

impl ApiKeyStore {
    /// Create a store from a comma-separated string of keys.
    ///
    /// Empty string or empty entries are skipped.
    /// Each key gets its own `RateLimiter` with 60 req/min.
    pub fn from_csv(csv: &str) -> Self {
        let mut keys = HashMap::new();
        for key in csv.split(',') {
            let key = key.trim();
            if !key.is_empty() {
                keys.insert(key.to_string(), RateLimiter::new(60));
            }
        }
        Self { keys }
    }

    /// Load keys from the `API_KEYS` env var. Returns `None` if the var is
    /// missing or empty (disables auth entirely).
    pub fn from_env() -> Option<Self> {
        let val = std::env::var("API_KEYS").ok()?;
        if val.trim().is_empty() {
            return None;
        }
        let store = Self::from_csv(&val);
        if store.keys.is_empty() {
            return None;
        }
        Some(store)
    }

    /// Returns the number of configured API keys.
    pub fn key_count(&self) -> usize {
        self.keys.len()
    }

    /// Validate a key and consume one rate-limit token.
    ///
    /// Returns `Ok(RateLimitHeaders)` on success, `Err(StatusCode)` on failure
    /// (401 for invalid key, 429 for rate limit exceeded).
    pub async fn check(&self, api_key: &str) -> Result<RateLimitHeaders, StatusCode> {
        let limiter = self.keys.get(api_key).ok_or(StatusCode::UNAUTHORIZED)?;

        limiter.consume().await
    }
}

// ── RateLimiter ────────────────────────────────────────────

impl RateLimiter {
    /// Create a new limiter with `max_per_min` requests per minute.
    pub fn new(max_per_min: u64) -> Self {
        let max = max_per_min as f64;
        Self {
            bucket: Mutex::new(TokenBucket {
                tokens: max,
                last_refill: Instant::now(),
            }),
            max_tokens: max,
            refill_per_sec: max / 60.0,
        }
    }

    /// Consume one token. Refills first based on elapsed time.
    async fn consume(&self) -> Result<RateLimitHeaders, StatusCode> {
        let mut bucket = self.bucket.lock().await;
        let now = Instant::now();
        let elapsed = now.duration_since(bucket.last_refill).as_secs_f64();

        // Refill
        bucket.tokens = (bucket.tokens + elapsed * self.refill_per_sec).min(self.max_tokens);
        bucket.last_refill = now;

        if bucket.tokens < 1.0 {
            return Err(StatusCode::TOO_MANY_REQUESTS);
        }

        bucket.tokens -= 1.0;

        let remaining = bucket.tokens.floor() as u64;
        let reset = (self.max_tokens / self.refill_per_sec).ceil() as u64;

        Ok(RateLimitHeaders {
            limit: self.max_tokens as u64,
            remaining,
            reset,
        })
    }
}

// ── Axum middleware ────────────────────────────────────────

/// Auth + rate-limit middleware for `/api/v2/*` routes.
///
/// Extracts the `X-API-Key` header, validates it, and checks the rate limit.
/// On success the request is forwarded with rate-limit headers on the response.
/// On failure returns 401 (missing/invalid key) or 429 (rate limited).
pub async fn auth_middleware(
    State(store): State<SharedApiKeyStore>,
    request: Request,
    next: axum::middleware::Next,
) -> Response {
    let api_key = request
        .headers()
        .get("X-API-Key")
        .and_then(|v| v.to_str().ok());

    match api_key {
        None => (
            StatusCode::UNAUTHORIZED,
            axum::Json(AuthErrorResponse {
                error: "Missing X-API-Key header".into(),
            }),
        )
            .into_response(),
        Some(key) => match store.check(key).await {
            Ok(headers) => {
                let mut response = next.run(request).await;
                let hdrs = response.headers_mut();
                hdrs.insert(
                    "X-RateLimit-Limit",
                    headers.limit.to_string().parse().expect("valid header value"),
                );
                hdrs.insert(
                    "X-RateLimit-Remaining",
                    headers.remaining.to_string().parse().expect("valid header value"),
                );
                hdrs.insert(
                    "X-RateLimit-Reset",
                    headers.reset.to_string().parse().expect("valid header value"),
                );
                response
            }
            Err(status) => {
                let error_msg = if status == StatusCode::TOO_MANY_REQUESTS {
                    "Rate limit exceeded"
                } else {
                    "Invalid API key"
                };
                (
                    status,
                    axum::Json(AuthErrorResponse {
                        error: error_msg.to_string(),
                    }),
                )
                    .into_response()
            }
        },
    }
}

// ── Tests ──────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_csv_splits_and_trims() {
        let store = ApiKeyStore::from_csv(" key1 , key2 , , key3 ");
        assert_eq!(store.keys.len(), 3);
        assert!(store.keys.contains_key("key1"));
        assert!(store.keys.contains_key("key2"));
        assert!(store.keys.contains_key("key3"));
    }

    #[test]
    fn from_csv_empty_string() {
        let store = ApiKeyStore::from_csv("");
        assert!(store.keys.is_empty());
    }

    #[tokio::test]
    async fn valid_key_passes() {
        let store = ApiKeyStore::from_csv("test-key");
        let result = store.check("test-key").await;
        assert!(result.is_ok());
        let headers = result.unwrap();
        assert_eq!(headers.limit, 60);
        assert_eq!(headers.remaining, 59);
    }

    #[tokio::test]
    async fn invalid_key_rejected() {
        let store = ApiKeyStore::from_csv("test-key");
        let result = store.check("wrong-key").await;
        assert_eq!(result.unwrap_err(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn rate_limit_enforced() {
        let store = ApiKeyStore::from_csv("key");
        // Consume 60 tokens
        for _ in 0..60 {
            assert!(store.check("key").await.is_ok());
        }
        // 61st should fail
        let result = store.check("key").await;
        assert_eq!(result.unwrap_err(), StatusCode::TOO_MANY_REQUESTS);
    }
}
