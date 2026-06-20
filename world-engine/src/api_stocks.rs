use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::*,
    Json,
};
use serde::{Deserialize, Serialize};

use std::collections::BTreeMap;

use crate::api::{AppState, ErrorResponse};
use crate::economy::stock_market::{
    ListingStatus, Order as StockOrder, OrderKind, OrderType, StockListing,
};

#[derive(Debug, Deserialize)]
pub struct IssueSharesRequest {
    pub org_id: String,
    pub ticker: String,
    pub total_shares: u64,
    pub price: u64,
}

#[derive(Debug, Deserialize)]
pub struct IpoRequest {
    pub org_member_count: usize,
    pub org_treasury: u64,
}

#[derive(Debug, Deserialize)]
pub struct BuyOrderRequest {
    pub stock_id: String,
    pub agent_id: String,
    pub order_kind: String,
    pub price: u64,
    pub quantity: u64,
    pub agent_funds: u64,
}

#[derive(Debug, Deserialize)]
pub struct SellOrderRequest {
    pub stock_id: String,
    pub agent_id: String,
    pub order_kind: String,
    pub price: u64,
    pub quantity: u64,
}

#[derive(Debug, Deserialize)]
pub struct CancelOrderRequest {
    pub agent_id: String,
}

#[derive(Debug, Deserialize)]
pub struct DividendRequest {
    pub total_profit: u64,
}

#[derive(Debug, Deserialize, Default)]
pub struct ListOrdersQuery {
    pub stock_id: Option<String>,
    pub agent_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct StockResponse {
    pub id: String,
    pub org_id: String,
    pub ticker: String,
    pub total_shares: u64,
    pub price: u64,
    pub status: String,
    pub listed_tick: u64,
}

impl From<&StockListing> for StockResponse {
    fn from(s: &StockListing) -> Self {
        StockResponse {
            id: s.id.clone(),
            org_id: s.org_id.clone(),
            ticker: s.ticker.clone(),
            total_shares: s.total_shares,
            price: s.price,
            status: match s.status {
                ListingStatus::PreIpo => "pre_ipo".to_string(),
                ListingStatus::Listed => "listed".to_string(),
                ListingStatus::Delisted => "delisted".to_string(),
            },
            listed_tick: s.listed_tick,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct OrderResponse {
    pub id: String,
    pub stock_id: String,
    pub agent_id: String,
    pub order_type: String,
    pub order_kind: String,
    pub price: u64,
    pub quantity: u64,
    pub filled_quantity: u64,
    pub status: String,
    pub created_tick: u64,
}

impl From<&StockOrder> for OrderResponse {
    fn from(o: &StockOrder) -> Self {
        OrderResponse {
            id: o.id.clone(),
            stock_id: o.stock_id.clone(),
            agent_id: o.agent_id.clone(),
            order_type: match o.order_type {
                OrderType::Buy => "buy".to_string(),
                OrderType::Sell => "sell".to_string(),
            },
            order_kind: match o.order_kind {
                OrderKind::Limit => "limit".to_string(),
                OrderKind::Market => "market".to_string(),
            },
            price: o.price,
            quantity: o.quantity,
            filled_quantity: o.filled_quantity,
            status: match o.status {
                crate::economy::stock_market::OrderStatus::Open => "open".to_string(),
                crate::economy::stock_market::OrderStatus::PartiallyFilled => {
                    "partially_filled".to_string()
                }
                crate::economy::stock_market::OrderStatus::Filled => "filled".to_string(),
                crate::economy::stock_market::OrderStatus::Cancelled => "cancelled".to_string(),
            },
            created_tick: o.created_tick,
        }
    }
}

/// One aggregated price point in a stock's trade history.
#[derive(Debug, Serialize)]
pub struct StockHistoryPoint {
    pub tick: u64,
    pub price: u64,
    pub volume: u64,
}

pub fn stock_error_status(e: &crate::economy::stock_market::StockMarketError) -> StatusCode {
    use crate::economy::stock_market::StockMarketError;
    match e {
        StockMarketError::StockNotFound(_) => StatusCode::NOT_FOUND,
        StockMarketError::OrderNotFound(_) => StatusCode::NOT_FOUND,
        StockMarketError::OrgNotFound(_) => StatusCode::NOT_FOUND,
        StockMarketError::NotListed => StatusCode::CONFLICT,
        StockMarketError::Delisted => StatusCode::CONFLICT,
        StockMarketError::InsufficientShares(_, _) => StatusCode::BAD_REQUEST,
        StockMarketError::InsufficientFunds(_, _) => StatusCode::BAD_REQUEST,
        StockMarketError::NotShareholder => StatusCode::BAD_REQUEST,
        StockMarketError::OrderNotActive => StatusCode::CONFLICT,
        StockMarketError::IpoConditionsNotMet(_) => StatusCode::BAD_REQUEST,
        StockMarketError::TickerTaken(_) => StatusCode::CONFLICT,
        StockMarketError::AlreadyListed(_) => StatusCode::CONFLICT,
        StockMarketError::EmptyTicker => StatusCode::BAD_REQUEST,
        StockMarketError::InvalidShareCount => StatusCode::BAD_REQUEST,
        StockMarketError::InvalidPrice => StatusCode::BAD_REQUEST,
        StockMarketError::InvalidQuantity => StatusCode::BAD_REQUEST,
        StockMarketError::NoSharesIssued(_) => StatusCode::BAD_REQUEST,
        StockMarketError::NoProfitToDistribute => StatusCode::BAD_REQUEST,
        StockMarketError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

pub async fn list_stocks(State(state): State<AppState>) -> impl IntoResponse {
    let sm = match &state.stock_market {
        Some(s) => s.clone(),
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "stock market not configured".into(),
                }),
            )
                .into_response()
        }
    };

    let sm = sm.lock().await;
    let stocks: Vec<StockResponse> = sm
        .list_stocks()
        .into_iter()
        .map(StockResponse::from)
        .collect();
    Json(stocks).into_response()
}

pub async fn issue_shares(
    State(state): State<AppState>,
    Json(body): Json<IssueSharesRequest>,
) -> impl IntoResponse {
    let sm = match &state.stock_market {
        Some(s) => s.clone(),
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "stock market not configured".into(),
                }),
            )
                .into_response()
        }
    };

    let tick = *state.tick_rx.borrow();
    let mut sm = sm.lock().await;
    match sm.issue_shares(
        body.org_id,
        body.ticker,
        body.total_shares,
        body.price,
        tick,
    ) {
        Ok(stock) => (StatusCode::CREATED, Json(StockResponse::from(&stock))).into_response(),
        Err(e) => (
            stock_error_status(&e),
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
            .into_response(),
    }
}

pub async fn get_stock(State(state): State<AppState>, Path(id): Path<String>) -> impl IntoResponse {
    let sm = match &state.stock_market {
        Some(s) => s.clone(),
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "stock market not configured".into(),
                }),
            )
                .into_response()
        }
    };

    let sm = sm.lock().await;
    match sm.get_stock(&id) {
        Some(stock) => Json(StockResponse::from(stock)).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "stock not found".into(),
            }),
        )
            .into_response(),
    }
}

/// `GET /api/v1/stocks/:id/history`
///
/// Returns the per-tick aggregated price history for a stock.
/// Trades are grouped by tick; each point contains the last trade price
/// in that tick and the total volume. Points are sorted ascending by tick.
/// Returns `[]` when the stock has no trades (e.g. pre-IPO or freshly listed).
pub async fn get_stock_history(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let sm = match &state.stock_market {
        Some(s) => s.clone(),
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "stock market not configured".into(),
                }),
            )
                .into_response()
        }
    };

    let sm = sm.lock().await;

    // 404 if the stock itself doesn't exist — keeps the endpoint consistent
    // with `GET /stocks/:id`.
    if sm.get_stock(&id).is_none() {
        return (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "stock not found".into(),
            }),
        )
            .into_response();
    }

    // Aggregate trades by tick. `list_trades` returns trades in insertion
    // order (chronological), so iterating and keeping the last price per
    // tick yields a correct per-tick close. BTreeMap gives us ascending tick
    // ordering for free.
    let trades = sm.list_trades(Some(&id));

    let mut by_tick: BTreeMap<u64, (u64, u64)> = BTreeMap::new();
    for t in &trades {
        let entry = by_tick.entry(t.tick).or_insert((t.price, 0));
        // Last-write-wins for price (trades are chronological), sum volume.
        entry.0 = t.price;
        entry.1 += t.quantity;
    }

    let history: Vec<StockHistoryPoint> = by_tick
        .into_iter()
        .map(|(tick, (price, volume))| StockHistoryPoint {
            tick,
            price,
            volume,
        })
        .collect();

    Json(history).into_response()
}

pub async fn ipo_stock(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<IpoRequest>,
) -> impl IntoResponse {
    let sm = match &state.stock_market {
        Some(s) => s.clone(),
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "stock market not configured".into(),
                }),
            )
                .into_response()
        }
    };

    let tick = *state.tick_rx.borrow();
    let mut sm = sm.lock().await;
    match sm.ipo(&id, body.org_member_count, body.org_treasury, tick) {
        Ok(stock) => Json(StockResponse::from(&stock)).into_response(),
        Err(e) => (
            stock_error_status(&e),
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
            .into_response(),
    }
}

pub async fn place_buy_order(
    State(state): State<AppState>,
    Json(body): Json<BuyOrderRequest>,
) -> impl IntoResponse {
    let sm = match &state.stock_market {
        Some(s) => s.clone(),
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "stock market not configured".into(),
                }),
            )
                .into_response()
        }
    };

    let order_kind = match body.order_kind.as_str() {
        "limit" => OrderKind::Limit,
        "market" => OrderKind::Market,
        other => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: format!("unknown order kind: {}", other),
                }),
            )
                .into_response()
        }
    };

    let tick = *state.tick_rx.borrow();
    let mut sm = sm.lock().await;
    match sm.place_buy_order(
        &body.stock_id,
        &body.agent_id,
        order_kind,
        body.price,
        body.quantity,
        body.agent_funds,
        tick,
    ) {
        Ok(order) => (StatusCode::CREATED, Json(OrderResponse::from(&order))).into_response(),
        Err(e) => (
            stock_error_status(&e),
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
            .into_response(),
    }
}

pub async fn place_sell_order(
    State(state): State<AppState>,
    Json(body): Json<SellOrderRequest>,
) -> impl IntoResponse {
    let sm = match &state.stock_market {
        Some(s) => s.clone(),
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "stock market not configured".into(),
                }),
            )
                .into_response()
        }
    };

    let order_kind = match body.order_kind.as_str() {
        "limit" => OrderKind::Limit,
        "market" => OrderKind::Market,
        other => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: format!("unknown order kind: {}", other),
                }),
            )
                .into_response()
        }
    };

    let tick = *state.tick_rx.borrow();
    let mut sm = sm.lock().await;
    match sm.place_sell_order(
        &body.stock_id,
        &body.agent_id,
        order_kind,
        body.price,
        body.quantity,
        tick,
    ) {
        Ok(order) => (StatusCode::CREATED, Json(OrderResponse::from(&order))).into_response(),
        Err(e) => (
            stock_error_status(&e),
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
            .into_response(),
    }
}

pub async fn list_stock_orders(
    State(state): State<AppState>,
    Query(query): Query<ListOrdersQuery>,
) -> impl IntoResponse {
    let sm = match &state.stock_market {
        Some(s) => s.clone(),
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "stock market not configured".into(),
                }),
            )
                .into_response()
        }
    };

    let sm = sm.lock().await;
    let orders: Vec<OrderResponse> = sm
        .list_orders(query.stock_id.as_deref(), query.agent_id.as_deref())
        .into_iter()
        .map(OrderResponse::from)
        .collect();
    Json(orders).into_response()
}

pub async fn get_order(State(state): State<AppState>, Path(id): Path<String>) -> impl IntoResponse {
    let sm = match &state.stock_market {
        Some(s) => s.clone(),
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "stock market not configured".into(),
                }),
            )
                .into_response()
        }
    };

    let sm = sm.lock().await;
    match sm.get_order(&id) {
        Some(order) => Json(OrderResponse::from(order)).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "order not found".into(),
            }),
        )
            .into_response(),
    }
}

pub async fn cancel_order(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<CancelOrderRequest>,
) -> impl IntoResponse {
    let sm = match &state.stock_market {
        Some(s) => s.clone(),
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "stock market not configured".into(),
                }),
            )
                .into_response()
        }
    };

    let mut sm = sm.lock().await;
    match sm.cancel_order(&id, &body.agent_id) {
        Ok(order) => Json(OrderResponse::from(&order)).into_response(),
        Err(e) => (
            stock_error_status(&e),
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
            .into_response(),
    }
}

pub async fn distribute_dividend(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<DividendRequest>,
) -> impl IntoResponse {
    let sm = match &state.stock_market {
        Some(s) => s.clone(),
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "stock market not configured".into(),
                }),
            )
                .into_response()
        }
    };

    let tick = *state.tick_rx.borrow();
    let mut sm = sm.lock().await;
    match sm.distribute_dividends(&id, body.total_profit, tick) {
        Ok(record) => (StatusCode::CREATED, Json(&record)).into_response(),
        Err(e) => (
            stock_error_status(&e),
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
            .into_response(),
    }
}

/// Stock market routes.
pub fn stock_routes() -> axum::Router<AppState> {
    axum::Router::new()
        .route("/stocks", get(list_stocks))
        .route("/stocks", post(issue_shares))
        .route("/stocks/:id", get(get_stock))
        .route("/stocks/:id/history", get(get_stock_history))
        .route("/stocks/:id/ipo", post(ipo_stock))
        .route("/stocks/:id/dividend", post(distribute_dividend))
        .route("/orders", get(list_stock_orders))
        .route("/orders/buy", post(place_buy_order))
        .route("/orders/sell", post(place_sell_order))
        .route("/orders/:id", get(get_order))
        .route("/orders/:id/cancel", post(cancel_order))
}
