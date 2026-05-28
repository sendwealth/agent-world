use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::*,
    Json,
};
use serde::Deserialize;

use crate::api::{api_err, api_ok, AppState};

// ══════════════════════════════════════════════════════════════════════════════
// Marketplace API handlers
// ══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Deserialize)]
pub struct MpPublishListingRequest {
    pub title: String,
    #[serde(default)]
    pub description: String,
    pub category: crate::economy::marketplace::KnowledgeCategory,
    pub content_hash: String,
    pub price: u64,
    pub currency: crate::world::enums::Currency,
    pub publisher_id: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub tick: u64,
}

/// POST /api/v1/marketplace/listings — Publish a new knowledge listing.
pub async fn mp_publish_listing(
    State(state): State<AppState>,
    Json(body): Json<MpPublishListingRequest>,
) -> impl IntoResponse {
    let mp = match &state.marketplace {
        Some(m) => m.clone(),
        None => {
            return api_err(
                StatusCode::SERVICE_UNAVAILABLE,
                "marketplace not configured",
            )
        }
    };
    let mut mp = mp.lock().await;
    match mp.publish_listing(
        body.title,
        body.description,
        body.category,
        body.content_hash,
        body.price,
        body.currency,
        body.publisher_id,
        body.tags,
        body.tick,
    ) {
        Ok(id) => {
            let listing = mp.get(id).unwrap().clone();
            api_ok(listing)
        }
        Err(e) => api_err(StatusCode::BAD_REQUEST, e.to_string()),
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct MpListListingsQuery {
    pub category: Option<crate::economy::marketplace::KnowledgeCategory>,
    pub publisher_id: Option<String>,
    pub min_price: Option<u64>,
    pub max_price: Option<u64>,
    pub tag: Option<String>,
    pub query: Option<String>,
    pub min_purchases: Option<u64>,
    pub min_rating: Option<f64>,
    pub sort: Option<crate::economy::marketplace::MarketplaceSort>,
    /// If true, include inactive/delisted listings.
    pub include_all: Option<bool>,
}

/// GET /api/v1/marketplace/listings — List/search knowledge listings.
pub async fn mp_list_listings(
    State(state): State<AppState>,
    Query(params): Query<MpListListingsQuery>,
) -> impl IntoResponse {
    let mp = match &state.marketplace {
        Some(m) => m.clone(),
        None => {
            return api_err(
                StatusCode::SERVICE_UNAVAILABLE,
                "marketplace not configured",
            )
        }
    };
    let mp = mp.lock().await;
    let listings = if params.include_all.unwrap_or(false) {
        mp.list_all()
    } else {
        let filter = crate::economy::marketplace::MarketplaceFilter {
            category: params.category,
            publisher_id: params.publisher_id,
            min_price: params.min_price,
            max_price: params.max_price,
            tag: params.tag,
            query: params.query,
            min_purchases: params.min_purchases,
            min_rating: params.min_rating,
            sort: params.sort,
        };
        mp.search(&filter)
    };
    api_ok(listings)
}

/// GET /api/v1/marketplace/listings/:id — Get a single listing.
pub async fn mp_get_listing(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
) -> impl IntoResponse {
    let mp = match &state.marketplace {
        Some(m) => m.clone(),
        None => {
            return api_err(
                StatusCode::SERVICE_UNAVAILABLE,
                "marketplace not configured",
            )
        }
    };
    let mp = mp.lock().await;
    match mp.get(id) {
        Some(listing) => api_ok(listing),
        None => api_err(StatusCode::NOT_FOUND, "listing not found"),
    }
}

#[derive(Debug, Deserialize)]
pub struct MpUpdateListingRequest {
    pub publisher_id: String,
    pub price: Option<u64>,
    pub status: Option<crate::economy::marketplace::ListingStatus>,
    pub tags: Option<Vec<String>>,
}

/// PUT /api/v1/marketplace/listings/:id — Update a listing.
pub async fn mp_update_listing(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
    Json(body): Json<MpUpdateListingRequest>,
) -> impl IntoResponse {
    let mp = match &state.marketplace {
        Some(m) => m.clone(),
        None => {
            return api_err(
                StatusCode::SERVICE_UNAVAILABLE,
                "marketplace not configured",
            )
        }
    };
    let mut mp = mp.lock().await;
    match mp.update_listing(id, &body.publisher_id, body.price, body.status, body.tags) {
        Ok(()) => {
            let listing = mp.get(id).unwrap().clone();
            api_ok(listing)
        }
        Err(e) => api_err(StatusCode::BAD_REQUEST, e.to_string()),
    }
}

#[derive(Debug, Deserialize)]
pub struct MpDelistRequest {
    pub publisher_id: String,
}

/// POST /api/v1/marketplace/listings/:id/delist — Delist a listing.
pub async fn mp_delist_listing(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
    Json(body): Json<MpDelistRequest>,
) -> impl IntoResponse {
    let mp = match &state.marketplace {
        Some(m) => m.clone(),
        None => {
            return api_err(
                StatusCode::SERVICE_UNAVAILABLE,
                "marketplace not configured",
            )
        }
    };
    let mut mp = mp.lock().await;
    match mp.delist_listing(id, &body.publisher_id) {
        Ok(()) => api_ok(serde_json::json!({"status": "delisted"})),
        Err(e) => api_err(StatusCode::BAD_REQUEST, e.to_string()),
    }
}

#[derive(Debug, Deserialize)]
pub struct MpPurchaseRequest {
    pub buyer_id: String,
    #[serde(default)]
    pub tick: u64,
}

/// POST /api/v1/marketplace/listings/:id/purchase — Purchase a listing.
pub async fn mp_purchase_listing(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
    Json(body): Json<MpPurchaseRequest>,
) -> impl IntoResponse {
    let mp = match &state.marketplace {
        Some(m) => m.clone(),
        None => {
            return api_err(
                StatusCode::SERVICE_UNAVAILABLE,
                "marketplace not configured",
            )
        }
    };
    let mut mp = mp.lock().await;
    match mp.purchase_listing(id, &body.buyer_id, body.tick) {
        Ok(record) => api_ok(record),
        Err(e) => api_err(StatusCode::BAD_REQUEST, e.to_string()),
    }
}

#[derive(Debug, Deserialize)]
pub struct MpRateRequest {
    pub rater_id: String,
    pub score: u8,
    pub review: Option<String>,
    #[serde(default)]
    pub tick: u64,
}

/// POST /api/v1/marketplace/listings/:id/rate — Rate a purchased listing.
pub async fn mp_rate_listing(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
    Json(body): Json<MpRateRequest>,
) -> impl IntoResponse {
    let mp = match &state.marketplace {
        Some(m) => m.clone(),
        None => {
            return api_err(
                StatusCode::SERVICE_UNAVAILABLE,
                "marketplace not configured",
            )
        }
    };
    let mut mp = mp.lock().await;
    match mp.rate_listing(id, &body.rater_id, body.score, body.review, body.tick) {
        Ok(rating_id) => api_ok(serde_json::json!({"rating_id": rating_id.to_string()})),
        Err(e) => api_err(StatusCode::BAD_REQUEST, e.to_string()),
    }
}

/// GET /api/v1/marketplace/listings/:id/ratings — List ratings for a listing.
pub async fn mp_list_ratings(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
) -> impl IntoResponse {
    let mp = match &state.marketplace {
        Some(m) => m.clone(),
        None => {
            return api_err(
                StatusCode::SERVICE_UNAVAILABLE,
                "marketplace not configured",
            )
        }
    };
    let mp = mp.lock().await;
    let ratings = mp.listing_ratings(id);
    api_ok(ratings)
}

/// GET /api/v1/marketplace/balance/:agent_id — Get agent's marketplace balance.
pub async fn mp_get_balance(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
) -> impl IntoResponse {
    let mp = match &state.marketplace {
        Some(m) => m.clone(),
        None => {
            return api_err(
                StatusCode::SERVICE_UNAVAILABLE,
                "marketplace not configured",
            )
        }
    };
    let mp = mp.lock().await;
    api_ok(serde_json::json!({"agent_id": agent_id, "balance": mp.get_balance(&agent_id)}))
}

#[derive(Debug, Deserialize)]
pub struct MpSetBalanceRequest {
    pub agent_id: String,
    pub amount: u64,
}

/// POST /api/v1/marketplace/balance — Set agent balance (admin/seed).
pub async fn mp_set_balance(
    State(state): State<AppState>,
    Json(body): Json<MpSetBalanceRequest>,
) -> impl IntoResponse {
    let mp = match &state.marketplace {
        Some(m) => m.clone(),
        None => {
            return api_err(
                StatusCode::SERVICE_UNAVAILABLE,
                "marketplace not configured",
            )
        }
    };
    let mut mp = mp.lock().await;
    mp.set_balance(&body.agent_id, body.amount);
    api_ok(serde_json::json!({"agent_id": body.agent_id, "balance": body.amount}))
}

#[derive(Debug, Deserialize)]
pub struct MpTransferRequest {
    pub from: String,
    pub to: String,
    pub amount: u64,
    pub currency: crate::world::enums::Currency,
}

/// POST /api/v1/marketplace/transfer — Transfer tokens between agents.
pub async fn mp_transfer(
    State(state): State<AppState>,
    Json(body): Json<MpTransferRequest>,
) -> impl IntoResponse {
    let mp = match &state.marketplace {
        Some(m) => m.clone(),
        None => {
            return api_err(
                StatusCode::SERVICE_UNAVAILABLE,
                "marketplace not configured",
            )
        }
    };
    let mut mp = mp.lock().await;
    match mp.transfer(&body.from, &body.to, body.amount, body.currency) {
        Ok(()) => api_ok(serde_json::json!({"status": "transferred"})),
        Err(e) => api_err(StatusCode::BAD_REQUEST, e.to_string()),
    }
}

/// Marketplace routes.
pub fn marketplace_routes() -> axum::Router<AppState> {
    axum::Router::new()
        .route("/api/v1/marketplace/listings", post(mp_publish_listing))
        .route("/api/v1/marketplace/listings", get(mp_list_listings))
        .route("/api/v1/marketplace/listings/:id", get(mp_get_listing))
        .route("/api/v1/marketplace/listings/:id", put(mp_update_listing))
        .route(
            "/api/v1/marketplace/listings/:id/delist",
            post(mp_delist_listing),
        )
        .route(
            "/api/v1/marketplace/listings/:id/purchase",
            post(mp_purchase_listing),
        )
        .route(
            "/api/v1/marketplace/listings/:id/rate",
            post(mp_rate_listing),
        )
        .route(
            "/api/v1/marketplace/listings/:id/ratings",
            get(mp_list_ratings),
        )
        .route("/api/v1/marketplace/balance/:agent_id", get(mp_get_balance))
        .route("/api/v1/marketplace/balance", post(mp_set_balance))
        .route("/api/v1/marketplace/transfer", post(mp_transfer))
}
