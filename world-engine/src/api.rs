use std::sync::Arc;

use axum::{
    Json,
    Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post},
};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::economy::marketplace::{
    Marketplace, KnowledgeListing, KnowledgeCategory,
    ListingStatus, MarketplaceFilter, MarketplaceSort,
};
use crate::economy::task::{TaskBoard, Task};
use crate::wal::WAL;

// ── Shared State ──────────────────────────────────────────

pub type SharedTaskBoard = Arc<Mutex<TaskBoard>>;
pub type SharedWAL = Arc<Mutex<WAL>>;
pub type SharedMarketplace = Arc<Mutex<Marketplace>>;

/// Combined state for the API with WAL support.
#[derive(Clone)]
pub struct AppState {
    pub board: SharedTaskBoard,
    pub wal: SharedWAL,
    pub marketplace: Option<SharedMarketplace>,
}

pub fn create_router(board: SharedTaskBoard) -> Router {
    Router::new()
        .route("/tasks", post(create_task))
        .route("/tasks", get(list_tasks))
        .route("/tasks/:id", get(get_task))
        .route("/tasks/:id/claim", post(claim_task))
        .route("/tasks/:id/start", post(start_task))
        .route("/tasks/:id/submit", post(submit_task))
        .route("/tasks/:id/review", post(review_task))
        .route("/tasks/:id/complete", post(complete_task))
        .route("/tasks/:id/expire", post(expire_task))
        .route("/tasks/:id", delete(delete_task))
        .with_state(board)
}

pub fn create_router_with_wal(board: SharedTaskBoard, wal: SharedWAL) -> Router {
    let state = AppState { board, wal, marketplace: None };
    Router::new()
        // Task routes
        .route("/tasks", post(create_task_with_wal))
        .route("/tasks", get(list_tasks_with_wal))
        .route("/tasks/:id", get(get_task_with_wal))
        .route("/tasks/:id/claim", post(claim_task_with_wal))
        .route("/tasks/:id/start", post(start_task_with_wal))
        .route("/tasks/:id/submit", post(submit_task_with_wal))
        .route("/tasks/:id/review", post(review_task_with_wal))
        .route("/tasks/:id/complete", post(complete_task_with_wal))
        .route("/tasks/:id/expire", post(expire_task_with_wal))
        .route("/tasks/:id", delete(delete_task_with_wal))
        // WAL routes
        .route("/wal/stats", get(wal_stats))
        .route("/wal/snapshot", post(wal_snapshot))
        .route("/wal/verify", get(wal_verify))
        .with_state(state)
}

pub fn create_router_with_marketplace(board: SharedTaskBoard, wal: SharedWAL, marketplace: SharedMarketplace) -> Router {
    let state = AppState { board, wal, marketplace: Some(marketplace) };
    Router::new()
        // Task routes
        .route("/tasks", post(create_task_with_wal))
        .route("/tasks", get(list_tasks_with_wal))
        .route("/tasks/:id", get(get_task_with_wal))
        .route("/tasks/:id/claim", post(claim_task_with_wal))
        .route("/tasks/:id/start", post(start_task_with_wal))
        .route("/tasks/:id/submit", post(submit_task_with_wal))
        .route("/tasks/:id/review", post(review_task_with_wal))
        .route("/tasks/:id/complete", post(complete_task_with_wal))
        .route("/tasks/:id/expire", post(expire_task_with_wal))
        .route("/tasks/:id", delete(delete_task_with_wal))
        // WAL routes
        .route("/wal/stats", get(wal_stats))
        .route("/wal/snapshot", post(wal_snapshot))
        .route("/wal/verify", get(wal_verify))
        // Marketplace routes
        .route("/marketplace/listings", post(marketplace_create_listing))
        .route("/marketplace/listings", get(marketplace_search_listings))
        .route("/marketplace/listings/:id", get(marketplace_get_listing))
        .route("/marketplace/listings/:id/purchase", post(marketplace_purchase_listing))
        .route("/marketplace/listings/:id/rate", post(marketplace_rate_listing))
        .route("/marketplace/listings/:id/delist", post(marketplace_delist_listing))
        .route("/marketplace/listings/:id/ratings", get(marketplace_get_ratings))
        .route("/marketplace/listings/:id/purchases", get(marketplace_get_purchases))
        .with_state(state)
}

// ── Request Types ─────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreateTaskRequest {
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub reward: u64,
    pub publisher_id: String,
    pub expires_at: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct ClaimTaskRequest {
    pub assignee_id: String,
}

#[derive(Debug, Deserialize)]
pub struct SubmitTaskRequest {
    pub result: String,
}

#[derive(Debug, Deserialize)]
pub struct ReviewTaskRequest {
    pub approved: bool,
    pub reviewer_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct ListTasksQuery {
    pub status: Option<String>,
    pub publisher_id: Option<String>,
    pub assignee_id: Option<String>,
}

impl Default for ListTasksQuery {
    fn default() -> Self {
        Self {
            status: None,
            publisher_id: None,
            assignee_id: None,
        }
    }
}

// ── Response Types ────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct TaskResponse {
    pub id: String,
    pub title: String,
    pub description: String,
    pub status: String,
    pub reward: u64,
    pub escrow_held: bool,
    pub publisher_id: String,
    pub assignee_id: Option<String>,
    pub result: Option<String>,
    pub expires_at: Option<u64>,
    pub created_tick: u64,
}

impl From<&Task> for TaskResponse {
    fn from(task: &Task) -> Self {
        TaskResponse {
            id: task.id.to_string(),
            title: task.title.clone(),
            description: task.description.clone(),
            status: task.status.to_string(),
            reward: task.reward,
            escrow_held: task.escrow_held,
            publisher_id: task.publisher_id.clone(),
            assignee_id: task.assignee_id.clone(),
            result: task.result.clone(),
            expires_at: task.expires_at,
            created_tick: task.created_tick,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

// ── Handlers ──────────────────────────────────────────────

async fn create_task(
    State(board): State<SharedTaskBoard>,
    Json(body): Json<CreateTaskRequest>,
) -> impl IntoResponse {
    if body.title.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "title is required".into() })).into_response();
    }
    if body.publisher_id.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "publisher_id is required".into() })).into_response();
    }

    let mut board = board.lock().await;
    match board.create_task(
        body.title,
        body.description,
        body.reward,
        body.publisher_id,
        0, // created_tick — would come from world clock in production
        body.expires_at,
    ) {
        Ok(id) => {
            let task = board.get(id).unwrap();
            (StatusCode::CREATED, Json(TaskResponse::from(task))).into_response()
        }
        Err(e) => {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: e.to_string() })).into_response()
        }
    }
}

async fn list_tasks(
    State(board): State<SharedTaskBoard>,
) -> impl IntoResponse {
    let board = board.lock().await;
    let tasks: Vec<TaskResponse> = board.list().into_iter().map(TaskResponse::from).collect();
    Json(tasks).into_response()
}

async fn get_task(
    State(board): State<SharedTaskBoard>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let Ok(uuid) = Uuid::parse_str(&id) else {
        return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "invalid task id".into() })).into_response();
    };

    let board = board.lock().await;
    match board.get(uuid) {
        Some(task) => Json(TaskResponse::from(task)).into_response(),
        None => (StatusCode::NOT_FOUND, Json(ErrorResponse { error: "task not found".into() })).into_response(),
    }
}

async fn claim_task(
    State(board): State<SharedTaskBoard>,
    Path(id): Path<String>,
    Json(body): Json<ClaimTaskRequest>,
) -> impl IntoResponse {
    let Ok(uuid) = Uuid::parse_str(&id) else {
        return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "invalid task id".into() })).into_response();
    };

    let mut board = board.lock().await;
    match board.claim_task(uuid, body.assignee_id) {
        Ok(()) => {
            let task = board.get(uuid).unwrap();
            Json(TaskResponse::from(task)).into_response()
        }
        Err(e) => {
            let status = match &e {
                crate::economy::task::TaskError::InvalidTransition { .. } => StatusCode::CONFLICT,
                crate::economy::task::TaskError::NotFound(_) => StatusCode::NOT_FOUND,
                _ => StatusCode::BAD_REQUEST,
            };
            (status, Json(ErrorResponse { error: e.to_string() })).into_response()
        }
    }
}

async fn start_task(
    State(board): State<SharedTaskBoard>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let Ok(uuid) = Uuid::parse_str(&id) else {
        return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "invalid task id".into() })).into_response();
    };

    let mut board = board.lock().await;
    match board.start_task(uuid) {
        Ok(()) => {
            let task = board.get(uuid).unwrap();
            Json(TaskResponse::from(task)).into_response()
        }
        Err(e) => {
            let status = match &e {
                crate::economy::task::TaskError::InvalidTransition { .. } => StatusCode::CONFLICT,
                crate::economy::task::TaskError::NotFound(_) => StatusCode::NOT_FOUND,
                _ => StatusCode::BAD_REQUEST,
            };
            (status, Json(ErrorResponse { error: e.to_string() })).into_response()
        }
    }
}

async fn submit_task(
    State(board): State<SharedTaskBoard>,
    Path(id): Path<String>,
    Json(body): Json<SubmitTaskRequest>,
) -> impl IntoResponse {
    let Ok(uuid) = Uuid::parse_str(&id) else {
        return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "invalid task id".into() })).into_response();
    };

    let mut board = board.lock().await;
    match board.submit_result(uuid, body.result) {
        Ok(()) => {
            let task = board.get(uuid).unwrap();
            Json(TaskResponse::from(task)).into_response()
        }
        Err(e) => {
            let status = match &e {
                crate::economy::task::TaskError::InvalidTransition { .. } => StatusCode::CONFLICT,
                crate::economy::task::TaskError::NotFound(_) => StatusCode::NOT_FOUND,
                crate::economy::task::TaskError::ResultRequired => StatusCode::BAD_REQUEST,
                _ => StatusCode::BAD_REQUEST,
            };
            (status, Json(ErrorResponse { error: e.to_string() })).into_response()
        }
    }
}

async fn review_task(
    State(board): State<SharedTaskBoard>,
    Path(id): Path<String>,
    Json(body): Json<ReviewTaskRequest>,
) -> impl IntoResponse {
    let Ok(uuid) = Uuid::parse_str(&id) else {
        return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "invalid task id".into() })).into_response();
    };

    let mut board = board.lock().await;
    match board.review_task(uuid, &body.reviewer_id, body.approved) {
        Ok(()) => {
            let task = board.get(uuid).unwrap();
            Json(TaskResponse::from(task)).into_response()
        }
        Err(e) => {
            let status = match &e {
                crate::economy::task::TaskError::InvalidTransition { .. } => StatusCode::CONFLICT,
                crate::economy::task::TaskError::NotFound(_) => StatusCode::NOT_FOUND,
                crate::economy::task::TaskError::NotPublisher { .. } => StatusCode::FORBIDDEN,
                _ => StatusCode::BAD_REQUEST,
            };
            (status, Json(ErrorResponse { error: e.to_string() })).into_response()
        }
    }
}

async fn complete_task(
    State(board): State<SharedTaskBoard>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let Ok(uuid) = Uuid::parse_str(&id) else {
        return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "invalid task id".into() })).into_response();
    };

    let mut board = board.lock().await;
    match board.complete_task(uuid, 0) {
        Ok(_) => {
            let task = board.get(uuid).unwrap();
            Json(TaskResponse::from(task)).into_response()
        }
        Err(e) => {
            let status = match &e {
                crate::economy::task::TaskError::InvalidTransition { .. } => StatusCode::CONFLICT,
                crate::economy::task::TaskError::NotFound(_) => StatusCode::NOT_FOUND,
                _ => StatusCode::BAD_REQUEST,
            };
            (status, Json(ErrorResponse { error: e.to_string() })).into_response()
        }
    }
}

async fn expire_task(
    State(board): State<SharedTaskBoard>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let Ok(uuid) = Uuid::parse_str(&id) else {
        return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "invalid task id".into() })).into_response();
    };

    let mut board = board.lock().await;
    match board.expire_task(uuid) {
        Ok(()) => {
            let task = board.get(uuid).unwrap();
            Json(TaskResponse::from(task)).into_response()
        }
        Err(e) => {
            let status = match &e {
                crate::economy::task::TaskError::InvalidTransition { .. } => StatusCode::CONFLICT,
                crate::economy::task::TaskError::NotFound(_) => StatusCode::NOT_FOUND,
                _ => StatusCode::BAD_REQUEST,
            };
            (status, Json(ErrorResponse { error: e.to_string() })).into_response()
        }
    }
}

async fn delete_task(
    State(board): State<SharedTaskBoard>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let Ok(uuid) = Uuid::parse_str(&id) else {
        return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "invalid task id".into() })).into_response();
    };

    let mut board = board.lock().await;
    match board.delete_task(uuid) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => {
            let status = match &e {
                crate::economy::task::TaskError::InvalidTransition { .. } => StatusCode::CONFLICT,
                crate::economy::task::TaskError::NotFound(_) => StatusCode::NOT_FOUND,
                _ => StatusCode::BAD_REQUEST,
            };
            (status, Json(ErrorResponse { error: e.to_string() })).into_response()
        }
    }
}

// ── Task Handlers (with WAL state) ────────────────────────

async fn create_task_with_wal(
    State(state): State<AppState>,
    Json(body): Json<CreateTaskRequest>,
) -> impl IntoResponse {
    create_task(State(state.board), Json(body)).await
}

async fn list_tasks_with_wal(
    State(state): State<AppState>,
) -> impl IntoResponse {
    list_tasks(State(state.board)).await
}

async fn get_task_with_wal(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    get_task(State(state.board), Path(id)).await
}

async fn claim_task_with_wal(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<ClaimTaskRequest>,
) -> impl IntoResponse {
    claim_task(State(state.board), Path(id), Json(body)).await
}

async fn start_task_with_wal(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    start_task(State(state.board), Path(id)).await
}

async fn submit_task_with_wal(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<SubmitTaskRequest>,
) -> impl IntoResponse {
    submit_task(State(state.board), Path(id), Json(body)).await
}

async fn review_task_with_wal(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<ReviewTaskRequest>,
) -> impl IntoResponse {
    review_task(State(state.board), Path(id), Json(body)).await
}

async fn complete_task_with_wal(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    complete_task(State(state.board), Path(id)).await
}

async fn expire_task_with_wal(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    expire_task(State(state.board), Path(id)).await
}

async fn delete_task_with_wal(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    delete_task(State(state.board), Path(id)).await
}

// ── WAL Handlers ──────────────────────────────────────────

async fn wal_stats(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let wal = state.wal.lock().await;
    Json(wal.stats())
}

async fn wal_snapshot(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let mut wal = state.wal.lock().await;
    match wal.take_snapshot(&[], 0) {
        Ok(snapshot_file) => Json(serde_json::json!({ "ok": true, "snapshot_file": snapshot_file })).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: e.to_string() })).into_response(),
    }
}

async fn wal_verify(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let mut wal = state.wal.lock().await;
    let result = wal.recover();
    match result {
        Ok(recovery) => Json(serde_json::json!({
            "consistent": !recovery.corrupted_records,
            "event_count": recovery.event_counter,
            "recovered_from_snapshot": recovery.recovered_from_snapshot,
        })).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: e.to_string() })).into_response(),
    }
}

// ── Marketplace Request Types ─────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreateListingRequest {
    pub title: String,
    #[serde(default)]
    pub description: String,
    pub category: KnowledgeCategory,
    pub content_hash: String,
    pub price: u64,
    #[serde(default = "default_currency")]
    pub currency: crate::world::enums::Currency,
    pub publisher_id: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

fn default_currency() -> crate::world::enums::Currency {
    crate::world::enums::Currency::Token
}

#[derive(Debug, Deserialize)]
pub struct PurchaseListingRequest {
    pub buyer_id: String,
}

#[derive(Debug, Deserialize)]
pub struct RateListingRequest {
    pub rater_id: String,
    pub score: u8,
    pub review: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct SearchListingsQuery {
    pub category: Option<KnowledgeCategory>,
    pub publisher_id: Option<String>,
    pub min_price: Option<u64>,
    pub max_price: Option<u64>,
    pub tag: Option<String>,
    pub query: Option<String>,
    pub min_purchases: Option<u64>,
    pub min_rating: Option<f64>,
    pub sort: Option<MarketplaceSort>,
}

impl Default for SearchListingsQuery {
    fn default() -> Self {
        Self {
            category: None,
            publisher_id: None,
            min_price: None,
            max_price: None,
            tag: None,
            query: None,
            min_purchases: None,
            min_rating: None,
            sort: None,
        }
    }
}

impl From<SearchListingsQuery> for MarketplaceFilter {
    fn from(q: SearchListingsQuery) -> Self {
        MarketplaceFilter {
            category: q.category,
            publisher_id: q.publisher_id,
            min_price: q.min_price,
            max_price: q.max_price,
            tag: q.tag,
            query: q.query,
            min_purchases: q.min_purchases,
            min_rating: q.min_rating,
            sort: q.sort,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct DelistListingRequest {
    pub publisher_id: String,
}

// ── Marketplace Response Types ────────────────────────────

#[derive(Debug, Serialize)]
pub struct ListingResponse {
    pub id: String,
    pub title: String,
    pub description: String,
    pub category: String,
    pub content_hash: String,
    pub price: u64,
    pub currency: String,
    pub publisher_id: String,
    pub status: String,
    pub purchase_count: u64,
    pub average_rating: f64,
    pub rating_count: u64,
    pub tags: Vec<String>,
    pub created_tick: u64,
}

impl From<&KnowledgeListing> for ListingResponse {
    fn from(l: &KnowledgeListing) -> Self {
        ListingResponse {
            id: l.id.to_string(),
            title: l.title.clone(),
            description: l.description.clone(),
            category: format!("{:?}", l.category).to_lowercase(),
            content_hash: l.content_hash.clone(),
            price: l.price,
            currency: format!("{:?}", l.currency).to_lowercase(),
            publisher_id: l.publisher_id.clone(),
            status: format!("{:?}", l.status).to_lowercase(),
            purchase_count: l.purchase_count,
            average_rating: l.average_rating(),
            rating_count: l.rating_count,
            tags: l.tags.clone(),
            created_tick: l.created_tick,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct PurchaseResponse {
    pub id: String,
    pub listing_id: String,
    pub buyer_id: String,
    pub seller_id: String,
    pub price: u64,
    pub currency: String,
    pub tick: u64,
}

impl From<&crate::economy::marketplace::PurchaseRecord> for PurchaseResponse {
    fn from(p: &crate::economy::marketplace::PurchaseRecord) -> Self {
        PurchaseResponse {
            id: p.id.to_string(),
            listing_id: p.listing_id.to_string(),
            buyer_id: p.buyer_id.clone(),
            seller_id: p.seller_id.clone(),
            price: p.price,
            currency: format!("{:?}", p.currency).to_lowercase(),
            tick: p.tick,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct RatingResponse {
    pub id: String,
    pub listing_id: String,
    pub rater_id: String,
    pub score: u8,
    pub review: Option<String>,
    pub tick: u64,
}

impl From<&crate::economy::marketplace::Rating> for RatingResponse {
    fn from(r: &crate::economy::marketplace::Rating) -> Self {
        RatingResponse {
            id: r.id.to_string(),
            listing_id: r.listing_id.to_string(),
            rater_id: r.rater_id.clone(),
            score: r.score,
            review: r.review.clone(),
            tick: r.tick,
        }
    }
}

// ── Marketplace Handlers ──────────────────────────────────

async fn marketplace_create_listing(
    State(state): State<AppState>,
    Json(body): Json<CreateListingRequest>,
) -> impl IntoResponse {
    let Some(ref mp) = state.marketplace else {
        return (StatusCode::NOT_IMPLEMENTED, Json(ErrorResponse { error: "marketplace not configured".into() })).into_response();
    };

    if body.title.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "title is required".into() })).into_response();
    }
    if body.publisher_id.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "publisher_id is required".into() })).into_response();
    }

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
        0,
    ) {
        Ok(id) => {
            let listing = mp.get(id).unwrap();
            (StatusCode::CREATED, Json(ListingResponse::from(listing))).into_response()
        }
        Err(e) => {
            let status = match &e {
                crate::economy::marketplace::MarketplaceError::InvalidPrice => StatusCode::BAD_REQUEST,
                _ => StatusCode::BAD_REQUEST,
            };
            (status, Json(ErrorResponse { error: e.to_string() })).into_response()
        }
    }
}

async fn marketplace_search_listings(
    State(state): State<AppState>,
    Query(params): Query<SearchListingsQuery>,
) -> impl IntoResponse {
    let Some(ref mp) = state.marketplace else {
        return (StatusCode::NOT_IMPLEMENTED, Json(ErrorResponse { error: "marketplace not configured".into() })).into_response();
    };

    let mp = mp.lock().await;
    let filter: MarketplaceFilter = params.into();
    let listings: Vec<ListingResponse> = mp.search(&filter)
        .into_iter()
        .map(ListingResponse::from)
        .collect();
    Json(listings).into_response()
}

async fn marketplace_get_listing(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let Some(ref mp) = state.marketplace else {
        return (StatusCode::NOT_IMPLEMENTED, Json(ErrorResponse { error: "marketplace not configured".into() })).into_response();
    };

    let Ok(uuid) = Uuid::parse_str(&id) else {
        return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "invalid listing id".into() })).into_response();
    };

    let mp = mp.lock().await;
    match mp.get(uuid) {
        Some(listing) => Json(ListingResponse::from(listing)).into_response(),
        None => (StatusCode::NOT_FOUND, Json(ErrorResponse { error: "listing not found".into() })).into_response(),
    }
}

async fn marketplace_purchase_listing(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<PurchaseListingRequest>,
) -> impl IntoResponse {
    let Some(ref mp) = state.marketplace else {
        return (StatusCode::NOT_IMPLEMENTED, Json(ErrorResponse { error: "marketplace not configured".into() })).into_response();
    };

    let Ok(uuid) = Uuid::parse_str(&id) else {
        return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "invalid listing id".into() })).into_response();
    };

    let mut mp = mp.lock().await;
    match mp.purchase_listing(uuid, &body.buyer_id, 0) {
        Ok(record) => (StatusCode::OK, Json(PurchaseResponse::from(&record))).into_response(),
        Err(e) => {
            let status = match &e {
                crate::economy::marketplace::MarketplaceError::NotFound(_) => StatusCode::NOT_FOUND,
                crate::economy::marketplace::MarketplaceError::InsufficientBalance { .. } => StatusCode::PAYMENT_REQUIRED,
                crate::economy::marketplace::MarketplaceError::SelfPurchase => StatusCode::FORBIDDEN,
                crate::economy::marketplace::MarketplaceError::ListingInactive => StatusCode::CONFLICT,
                crate::economy::marketplace::MarketplaceError::ListingDelisted => StatusCode::CONFLICT,
                _ => StatusCode::BAD_REQUEST,
            };
            (status, Json(ErrorResponse { error: e.to_string() })).into_response()
        }
    }
}

async fn marketplace_rate_listing(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<RateListingRequest>,
) -> impl IntoResponse {
    let Some(ref mp) = state.marketplace else {
        return (StatusCode::NOT_IMPLEMENTED, Json(ErrorResponse { error: "marketplace not configured".into() })).into_response();
    };

    let Ok(uuid) = Uuid::parse_str(&id) else {
        return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "invalid listing id".into() })).into_response();
    };

    let mut mp = mp.lock().await;
    match mp.rate_listing(uuid, &body.rater_id, body.score, body.review, 0) {
        Ok(rating_id) => {
            let ratings = mp.listing_ratings(uuid);
            let rating = ratings.into_iter().find(|r| r.id == rating_id).unwrap();
            (StatusCode::CREATED, Json(RatingResponse::from(rating))).into_response()
        }
        Err(e) => {
            let status = match &e {
                crate::economy::marketplace::MarketplaceError::NotFound(_) => StatusCode::NOT_FOUND,
                crate::economy::marketplace::MarketplaceError::InvalidRating => StatusCode::BAD_REQUEST,
                crate::economy::marketplace::MarketplaceError::AlreadyRated => StatusCode::CONFLICT,
                crate::economy::marketplace::MarketplaceError::NotPurchased => StatusCode::FORBIDDEN,
                _ => StatusCode::BAD_REQUEST,
            };
            (status, Json(ErrorResponse { error: e.to_string() })).into_response()
        }
    }
}

async fn marketplace_delist_listing(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<DelistListingRequest>,
) -> impl IntoResponse {
    let Some(ref mp) = state.marketplace else {
        return (StatusCode::NOT_IMPLEMENTED, Json(ErrorResponse { error: "marketplace not configured".into() })).into_response();
    };

    let Ok(uuid) = Uuid::parse_str(&id) else {
        return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "invalid listing id".into() })).into_response();
    };

    let mut mp = mp.lock().await;
    match mp.delist_listing(uuid, &body.publisher_id) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => {
            let status = match &e {
                crate::economy::marketplace::MarketplaceError::NotFound(_) => StatusCode::NOT_FOUND,
                crate::economy::marketplace::MarketplaceError::Unauthorized(_) => StatusCode::FORBIDDEN,
                _ => StatusCode::BAD_REQUEST,
            };
            (status, Json(ErrorResponse { error: e.to_string() })).into_response()
        }
    }
}

async fn marketplace_get_ratings(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let Some(ref mp) = state.marketplace else {
        return (StatusCode::NOT_IMPLEMENTED, Json(ErrorResponse { error: "marketplace not configured".into() })).into_response();
    };

    let Ok(uuid) = Uuid::parse_str(&id) else {
        return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "invalid listing id".into() })).into_response();
    };

    let mp = mp.lock().await;
    let ratings: Vec<RatingResponse> = mp.listing_ratings(uuid)
        .into_iter()
        .map(RatingResponse::from)
        .collect();
    Json(ratings).into_response()
}

async fn marketplace_get_purchases(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let Some(ref mp) = state.marketplace else {
        return (StatusCode::NOT_IMPLEMENTED, Json(ErrorResponse { error: "marketplace not configured".into() })).into_response();
    };

    let Ok(uuid) = Uuid::parse_str(&id) else {
        return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "invalid listing id".into() })).into_response();
    };

    let mp = mp.lock().await;
    let purchases: Vec<PurchaseResponse> = mp.listing_purchases(uuid)
        .into_iter()
        .map(PurchaseResponse::from)
        .collect();
    Json(purchases).into_response()
}
