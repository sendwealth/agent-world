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
// Tool Marketplace API handlers
// ══════════════════════════════════════════════════════════════════════════════

// ── List (publish) a tool ───────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct TmListToolRequest {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub category: crate::economy::tool_marketplace::ToolCategory,
    pub owner_id: String,
    pub purchase_price: u64,
    pub rental_price_per_tick: u64,
    pub currency: crate::world::enums::Currency,
    pub listing_mode: crate::economy::tool_marketplace::ToolListingMode,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub created_tick: u64,
}

/// POST /api/v1/tool-marketplace/tools — List a new tool on the marketplace.
pub async fn tm_list_tool(
    State(state): State<AppState>,
    Json(body): Json<TmListToolRequest>,
) -> impl IntoResponse {
    let mp = match &state.tool_marketplace {
        Some(m) => m.clone(),
        None => {
            return api_err(
                StatusCode::SERVICE_UNAVAILABLE,
                "tool marketplace not configured",
            )
        }
    };
    let mut mp = mp.lock().await;
    match mp.list_tool(
        body.name,
        body.description,
        body.category,
        body.owner_id,
        body.purchase_price,
        body.rental_price_per_tick,
        body.currency,
        body.listing_mode,
        body.tags,
        body.created_tick,
    ) {
        Ok(id) => {
            let listing = mp.get(id).unwrap().clone();
            api_ok(listing)
        }
        Err(e) => api_err(StatusCode::BAD_REQUEST, e.to_string()),
    }
}

// ── Search / list tools ─────────────────────────────────────────────────────

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct TmSearchQuery {
    pub category: Option<crate::economy::tool_marketplace::ToolCategory>,
    pub owner_id: Option<String>,
    pub listing_mode: Option<crate::economy::tool_marketplace::ToolListingMode>,
    pub min_price: Option<u64>,
    pub max_price: Option<u64>,
    pub tag: Option<String>,
    pub query: Option<String>,
    pub min_rating: Option<f64>,
    pub sort: Option<crate::economy::tool_marketplace::ToolMarketplaceSort>,
    /// If true, include inactive/delisted listings.
    pub include_all: Option<bool>,
}

/// GET /api/v1/tool-marketplace/tools — Search/list tools.
pub async fn tm_search_tools(
    State(state): State<AppState>,
    Query(params): Query<TmSearchQuery>,
) -> impl IntoResponse {
    let mp = match &state.tool_marketplace {
        Some(m) => m.clone(),
        None => {
            return api_err(
                StatusCode::SERVICE_UNAVAILABLE,
                "tool marketplace not configured",
            )
        }
    };
    let mp = mp.lock().await;
    let listings = if params.include_all.unwrap_or(false) {
        mp.list_all()
    } else {
        let filter = crate::economy::tool_marketplace::ToolMarketplaceFilter {
            category: params.category,
            owner_id: params.owner_id,
            listing_mode: params.listing_mode,
            min_price: params.min_price,
            max_price: params.max_price,
            tag: params.tag,
            query: params.query,
            min_rating: params.min_rating,
            sort: params.sort,
        };
        mp.search(&filter)
    };
    api_ok(listings)
}

// ── Get single tool ─────────────────────────────────────────────────────────

/// GET /api/v1/tool-marketplace/tools/:id — Get a single tool listing.
pub async fn tm_get_tool(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
) -> impl IntoResponse {
    let mp = match &state.tool_marketplace {
        Some(m) => m.clone(),
        None => {
            return api_err(
                StatusCode::SERVICE_UNAVAILABLE,
                "tool marketplace not configured",
            )
        }
    };
    let mp = mp.lock().await;
    match mp.get(id) {
        Some(listing) => api_ok(listing),
        None => api_err(StatusCode::NOT_FOUND, "tool not found"),
    }
}

// ── Update tool ─────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct TmUpdateToolRequest {
    pub owner_id: String,
    pub purchase_price: Option<u64>,
    pub rental_price_per_tick: Option<u64>,
    pub status: Option<crate::economy::tool_marketplace::ToolListingStatus>,
    pub tags: Option<Vec<String>>,
}

/// PUT /api/v1/tool-marketplace/tools/:id — Update a tool listing.
pub async fn tm_update_tool(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
    Json(body): Json<TmUpdateToolRequest>,
) -> impl IntoResponse {
    let mp = match &state.tool_marketplace {
        Some(m) => m.clone(),
        None => {
            return api_err(
                StatusCode::SERVICE_UNAVAILABLE,
                "tool marketplace not configured",
            )
        }
    };
    let mut mp = mp.lock().await;
    match mp.update_tool(
        id,
        &body.owner_id,
        body.purchase_price,
        body.rental_price_per_tick,
        body.status,
        body.tags,
    ) {
        Ok(()) => {
            let listing = mp.get(id).unwrap().clone();
            api_ok(listing)
        }
        Err(e) => api_err(StatusCode::BAD_REQUEST, e.to_string()),
    }
}

// ── Delist tool ─────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct TmDelistRequest {
    pub owner_id: String,
}

/// POST /api/v1/tool-marketplace/tools/:id/delist — Delist a tool.
pub async fn tm_delist_tool(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
    Json(body): Json<TmDelistRequest>,
) -> impl IntoResponse {
    let mp = match &state.tool_marketplace {
        Some(m) => m.clone(),
        None => {
            return api_err(
                StatusCode::SERVICE_UNAVAILABLE,
                "tool marketplace not configured",
            )
        }
    };
    let mut mp = mp.lock().await;
    match mp.delist_tool(id, &body.owner_id) {
        Ok(()) => api_ok(serde_json::json!({"status": "delisted"})),
        Err(e) => api_err(StatusCode::BAD_REQUEST, e.to_string()),
    }
}

// ── Purchase tool ───────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct TmPurchaseRequest {
    pub buyer_id: String,
    #[serde(default)]
    pub tick: u64,
}

/// POST /api/v1/tool-marketplace/tools/:id/purchase — Purchase a tool.
pub async fn tm_purchase_tool(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
    Json(body): Json<TmPurchaseRequest>,
) -> impl IntoResponse {
    let mp = match &state.tool_marketplace {
        Some(m) => m.clone(),
        None => {
            return api_err(
                StatusCode::SERVICE_UNAVAILABLE,
                "tool marketplace not configured",
            )
        }
    };
    let mut mp = mp.lock().await;
    match mp.purchase_tool(id, &body.buyer_id, body.tick) {
        Ok(record) => api_ok(record),
        Err(e) => api_err(StatusCode::BAD_REQUEST, e.to_string()),
    }
}

// ── Rent tool ───────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct TmRentRequest {
    pub renter_id: String,
    pub duration_ticks: u64,
    #[serde(default)]
    pub current_tick: u64,
}

/// POST /api/v1/tool-marketplace/tools/:id/rent — Rent a tool.
pub async fn tm_rent_tool(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
    Json(body): Json<TmRentRequest>,
) -> impl IntoResponse {
    let mp = match &state.tool_marketplace {
        Some(m) => m.clone(),
        None => {
            return api_err(
                StatusCode::SERVICE_UNAVAILABLE,
                "tool marketplace not configured",
            )
        }
    };
    let mut mp = mp.lock().await;
    match mp.rent_tool(id, &body.renter_id, body.duration_ticks, body.current_tick) {
        Ok(record) => api_ok(record),
        Err(e) => api_err(StatusCode::BAD_REQUEST, e.to_string()),
    }
}

// ── Cancel rental ───────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct TmCancelRentalRequest {
    pub renter_id: String,
}

/// POST /api/v1/tool-marketplace/rentals/:id/cancel — Cancel an active rental.
pub async fn tm_cancel_rental(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
    Json(body): Json<TmCancelRentalRequest>,
) -> impl IntoResponse {
    let mp = match &state.tool_marketplace {
        Some(m) => m.clone(),
        None => {
            return api_err(
                StatusCode::SERVICE_UNAVAILABLE,
                "tool marketplace not configured",
            )
        }
    };
    let mut mp = mp.lock().await;
    match mp.cancel_rental(id, &body.renter_id) {
        Ok(()) => api_ok(serde_json::json!({"status": "cancelled"})),
        Err(e) => api_err(StatusCode::BAD_REQUEST, e.to_string()),
    }
}

// ── Active rentals ──────────────────────────────────────────────────────────

/// GET /api/v1/tool-marketplace/rentals/active/:agent_id — List active rentals for agent.
pub async fn tm_active_rentals(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
) -> impl IntoResponse {
    let mp = match &state.tool_marketplace {
        Some(m) => m.clone(),
        None => {
            return api_err(
                StatusCode::SERVICE_UNAVAILABLE,
                "tool marketplace not configured",
            )
        }
    };
    let mp = mp.lock().await;
    let rentals = mp.list_active_rentals(&agent_id);
    api_ok(rentals)
}

// ── Process rental expiry ───────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct TmExpireRentalsRequest {
    pub current_tick: u64,
}

/// POST /api/v1/tool-marketplace/rentals/expire — Process rental expirations.
pub async fn tm_expire_rentals(
    State(state): State<AppState>,
    Json(body): Json<TmExpireRentalsRequest>,
) -> impl IntoResponse {
    let mp = match &state.tool_marketplace {
        Some(m) => m.clone(),
        None => {
            return api_err(
                StatusCode::SERVICE_UNAVAILABLE,
                "tool marketplace not configured",
            )
        }
    };
    let mut mp = mp.lock().await;
    let expired = mp.process_rental_expiry(body.current_tick);
    api_ok(serde_json::json!({
        "expired_count": expired.len(),
        "expired_rental_ids": expired,
    }))
}

// ── Rate tool ───────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct TmRateRequest {
    pub rater_id: String,
    pub score: u8,
    pub review: Option<String>,
    #[serde(default)]
    pub tick: u64,
}

/// POST /api/v1/tool-marketplace/tools/:id/rate — Rate a tool.
pub async fn tm_rate_tool(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
    Json(body): Json<TmRateRequest>,
) -> impl IntoResponse {
    let mp = match &state.tool_marketplace {
        Some(m) => m.clone(),
        None => {
            return api_err(
                StatusCode::SERVICE_UNAVAILABLE,
                "tool marketplace not configured",
            )
        }
    };
    let mut mp = mp.lock().await;
    match mp.rate_tool(id, &body.rater_id, body.score, body.review, body.tick) {
        Ok(rating_id) => api_ok(serde_json::json!({"rating_id": rating_id.to_string()})),
        Err(e) => api_err(StatusCode::BAD_REQUEST, e.to_string()),
    }
}

// ── List ratings ────────────────────────────────────────────────────────────

/// GET /api/v1/tool-marketplace/tools/:id/ratings — List ratings for a tool.
pub async fn tm_list_ratings(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
) -> impl IntoResponse {
    let mp = match &state.tool_marketplace {
        Some(m) => m.clone(),
        None => {
            return api_err(
                StatusCode::SERVICE_UNAVAILABLE,
                "tool marketplace not configured",
            )
        }
    };
    let mp = mp.lock().await;
    let ratings = mp.tool_ratings(id);
    api_ok(ratings)
}

// ── Check ownership ─────────────────────────────────────────────────────────

/// GET /api/v1/tool-marketplace/tools/:id/ownership/:agent_id — Check if agent owns tool.
pub async fn tm_check_ownership(
    State(state): State<AppState>,
    Path((tool_id, agent_id)): Path<(uuid::Uuid, String)>,
) -> impl IntoResponse {
    let mp = match &state.tool_marketplace {
        Some(m) => m.clone(),
        None => {
            return api_err(
                StatusCode::SERVICE_UNAVAILABLE,
                "tool marketplace not configured",
            )
        }
    };
    let mp = mp.lock().await;
    api_ok(serde_json::json!({
        "agent_id": agent_id,
        "tool_id": tool_id.to_string(),
        "owns": mp.owns_tool(&agent_id, tool_id),
        "has_active_rental": mp.has_active_rental(&agent_id, tool_id),
    }))
}

// ── Balance ─────────────────────────────────────────────────────────────────

/// GET /api/v1/tool-marketplace/balance/:agent_id — Get agent's tool marketplace balance.
pub async fn tm_get_balance(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
) -> impl IntoResponse {
    let mp = match &state.tool_marketplace {
        Some(m) => m.clone(),
        None => {
            return api_err(
                StatusCode::SERVICE_UNAVAILABLE,
                "tool marketplace not configured",
            )
        }
    };
    let mp = mp.lock().await;
    api_ok(serde_json::json!({"agent_id": agent_id, "balance": mp.get_balance(&agent_id)}))
}

#[derive(Debug, Deserialize)]
pub struct TmSetBalanceRequest {
    pub agent_id: String,
    pub amount: u64,
}

/// POST /api/v1/tool-marketplace/balance — Set agent balance (admin/seed).
pub async fn tm_set_balance(
    State(state): State<AppState>,
    Json(body): Json<TmSetBalanceRequest>,
) -> impl IntoResponse {
    let mp = match &state.tool_marketplace {
        Some(m) => m.clone(),
        None => {
            return api_err(
                StatusCode::SERVICE_UNAVAILABLE,
                "tool marketplace not configured",
            )
        }
    };
    let mut mp = mp.lock().await;
    mp.set_balance(&body.agent_id, body.amount);
    api_ok(serde_json::json!({"agent_id": body.agent_id, "balance": body.amount}))
}

// ── Tool purchases ──────────────────────────────────────────────────────────

/// GET /api/v1/tool-marketplace/tools/:id/purchases — List purchase records for a tool.
pub async fn tm_list_purchases(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
) -> impl IntoResponse {
    let mp = match &state.tool_marketplace {
        Some(m) => m.clone(),
        None => {
            return api_err(
                StatusCode::SERVICE_UNAVAILABLE,
                "tool marketplace not configured",
            )
        }
    };
    let mp = mp.lock().await;
    let purchases = mp.tool_purchases(id);
    api_ok(purchases)
}

// ══════════════════════════════════════════════════════════════════════════════
// Router
// ══════════════════════════════════════════════════════════════════════════════

/// Tool marketplace routes.
pub fn tool_marketplace_routes() -> axum::Router<AppState> {
    axum::Router::new()
        // Tool CRUD
        .route("/api/v1/tool-marketplace/tools", post(tm_list_tool))
        .route("/api/v1/tool-marketplace/tools", get(tm_search_tools))
        .route("/api/v1/tool-marketplace/tools/:id", get(tm_get_tool))
        .route("/api/v1/tool-marketplace/tools/:id", put(tm_update_tool))
        .route(
            "/api/v1/tool-marketplace/tools/:id/delist",
            post(tm_delist_tool),
        )
        // Purchase / Rent
        .route(
            "/api/v1/tool-marketplace/tools/:id/purchase",
            post(tm_purchase_tool),
        )
        .route(
            "/api/v1/tool-marketplace/tools/:id/rent",
            post(tm_rent_tool),
        )
        // Purchase history
        .route(
            "/api/v1/tool-marketplace/tools/:id/purchases",
            get(tm_list_purchases),
        )
        // Ratings
        .route(
            "/api/v1/tool-marketplace/tools/:id/rate",
            post(tm_rate_tool),
        )
        .route(
            "/api/v1/tool-marketplace/tools/:id/ratings",
            get(tm_list_ratings),
        )
        // Ownership check
        .route(
            "/api/v1/tool-marketplace/tools/:id/ownership/:agent_id",
            get(tm_check_ownership),
        )
        // Rentals
        .route(
            "/api/v1/tool-marketplace/rentals/:id/cancel",
            post(tm_cancel_rental),
        )
        .route(
            "/api/v1/tool-marketplace/rentals/active/:agent_id",
            get(tm_active_rentals),
        )
        .route(
            "/api/v1/tool-marketplace/rentals/expire",
            post(tm_expire_rentals),
        )
        // Balance
        .route(
            "/api/v1/tool-marketplace/balance/:agent_id",
            get(tm_get_balance),
        )
        .route("/api/v1/tool-marketplace/balance", post(tm_set_balance))
}
