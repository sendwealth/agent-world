use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::world::event::WorldEvent;
use crate::world::state::EventBus;

// ── Constants ─────────────────────────────────────────────

/// Trading fee percentage (0.5% = 0.005, stored as basis points / 10000).
pub const TRADING_FEE_BPS: u64 = 50; // 50 bps = 0.5%
pub const BPS_DENOMINATOR: u64 = 10_000;

/// Minimum conditions for IPO: at least this many members.
pub const IPO_MIN_MEMBERS: usize = 3;
/// Minimum treasury for IPO.
pub const IPO_MIN_TREASURY: u64 = 1_000;

// ── Enums ─────────────────────────────────────────────────

/// Type of order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderType {
    /// Buy order.
    Buy,
    /// Sell order.
    Sell,
}

/// Order kind: limit (specific price) or market (best available).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderKind {
    /// Limit order: execute only at `price` or better.
    Limit,
    /// Market order: execute immediately at best available price.
    Market,
}

/// Status of an order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderStatus {
    /// Order is open and waiting to be filled.
    Open,
    /// Order has been partially filled.
    PartiallyFilled,
    /// Order has been completely filled.
    Filled,
    /// Order was cancelled.
    Cancelled,
}

/// Status of a stock listing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ListingStatus {
    /// Pre-IPO: shares issued but not publicly tradeable yet.
    PreIpo,
    /// Publicly listed and tradeable.
    Listed,
    /// Delisted (e.g. org dissolved).
    Delisted,
}

// ── Core Data Structures ─────────────────────────────────

/// A stock listing for an organization.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StockListing {
    /// Unique ID of this listing.
    pub id: String,
    /// The organization ID this stock belongs to.
    pub org_id: String,
    /// Ticker symbol (e.g. "ACME").
    pub ticker: String,
    /// Total number of shares issued.
    pub total_shares: u64,
    /// Current price per share (in Money).
    pub price: u64,
    /// Listing status.
    pub status: ListingStatus,
    /// Tick when the stock was listed/IPO'd.
    pub listed_tick: u64,
}

/// A share holding record for an agent in a stock.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ShareHolding {
    /// The agent who holds these shares.
    pub agent_id: String,
    /// The stock listing ID.
    pub stock_id: String,
    /// Number of shares held.
    pub quantity: u64,
}

/// An order in the order book.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Order {
    /// Unique order ID.
    pub id: String,
    /// The stock listing ID.
    pub stock_id: String,
    /// The agent who placed this order.
    pub agent_id: String,
    /// Buy or sell.
    pub order_type: OrderType,
    /// Limit or market.
    pub order_kind: OrderKind,
    /// Price per share (meaningful for limit orders; for market orders this is 0).
    pub price: u64,
    /// Total number of shares in the order.
    pub quantity: u64,
    /// Number of shares already filled.
    pub filled_quantity: u64,
    /// Status of the order.
    pub status: OrderStatus,
    /// Tick when the order was created.
    pub created_tick: u64,
}

impl Order {
    /// Remaining unfilled quantity.
    pub fn remaining(&self) -> u64 {
        self.quantity.saturating_sub(self.filled_quantity)
    }

    /// Whether the order is still active (can be filled).
    pub fn is_active(&self) -> bool {
        matches!(
            self.status,
            OrderStatus::Open | OrderStatus::PartiallyFilled
        )
    }
}

/// Record of a completed trade.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Trade {
    /// Unique trade ID.
    pub id: String,
    /// The stock listing ID.
    pub stock_id: String,
    /// Buy order ID.
    pub buy_order_id: String,
    /// Sell order ID.
    pub sell_order_id: String,
    /// Buyer agent ID.
    pub buyer_id: String,
    /// Seller agent ID.
    pub seller_id: String,
    /// Price per share at which the trade executed.
    pub price: u64,
    /// Number of shares traded.
    pub quantity: u64,
    /// Fee collected (in Money).
    pub fee: u64,
    /// Tick when the trade happened.
    pub tick: u64,
}

/// Dividend distribution record.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DividendRecord {
    /// Unique ID.
    pub id: String,
    /// Stock listing ID.
    pub stock_id: String,
    /// Org ID.
    pub org_id: String,
    /// Total profit being distributed.
    pub total_profit: u64,
    /// Dividend per share.
    pub dividend_per_share: u64,
    /// Tick when the dividend was distributed.
    pub tick: u64,
    /// Recipient details.
    pub recipients: Vec<DividendRecipient>,
}

/// A single recipient's dividend payout.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DividendRecipient {
    pub agent_id: String,
    pub shares: u64,
    pub amount: u64,
}

// ── Error Type ────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum StockMarketError {
    #[error("stock not found: {0}")]
    StockNotFound(String),
    #[error("order not found: {0}")]
    OrderNotFound(String),
    #[error("organization not found: {0}")]
    OrgNotFound(String),
    #[error("stock already listed for org: {0}")]
    AlreadyListed(String),
    #[error("stock is not publicly listed")]
    NotListed,
    #[error("stock is delisted")]
    Delisted,
    #[error("insufficient shares: have {0}, need {1}")]
    InsufficientShares(u64, u64),
    #[error("insufficient funds: have {0}, need {1}")]
    InsufficientFunds(u64, u64),
    #[error("cannot sell shares you do not own")]
    NotShareholder,
    #[error("order is not active")]
    OrderNotActive,
    #[error("IPO conditions not met: {0}")]
    IpoConditionsNotMet(String),
    #[error("ticker already taken: {0}")]
    TickerTaken(String),
    #[error("ticker cannot be empty")]
    EmptyTicker,
    #[error("total shares must be > 0")]
    InvalidShareCount,
    #[error("price must be > 0 for limit orders")]
    InvalidPrice,
    #[error("quantity must be > 0")]
    InvalidQuantity,
    #[error("no shares issued for stock: {0}")]
    NoSharesIssued(String),
    #[error("no profit to distribute")]
    NoProfitToDistribute,
    #[error("internal error: {0}")]
    Internal(String),
}

// ── Stock Market Store ────────────────────────────────────

/// In-memory stock market with order book, share registry, and trade history.
pub struct StockMarket {
    /// Stock listings by stock ID.
    stocks: HashMap<String, StockListing>,
    /// Maps org_id -> stock_id (one stock per org).
    org_to_stock: HashMap<String, String>,
    /// Maps ticker -> stock_id.
    ticker_to_stock: HashMap<String, String>,
    /// Share holdings: (stock_id, agent_id) -> ShareHolding.
    holdings: HashMap<(String, String), ShareHolding>,
    /// Open orders by stock ID.
    orders: HashMap<String, Order>,
    /// Completed trades.
    trades: Vec<Trade>,
    /// Dividend history.
    dividends: Vec<DividendRecord>,
    /// Event bus for broadcasting events.
    event_bus: Option<EventBus>,
}

impl Default for StockMarket {
    fn default() -> Self {
        Self::new()
    }
}

impl StockMarket {
    /// Create a new empty stock market.
    pub fn new() -> Self {
        Self {
            stocks: HashMap::new(),
            org_to_stock: HashMap::new(),
            ticker_to_stock: HashMap::new(),
            holdings: HashMap::new(),
            orders: HashMap::new(),
            trades: Vec::new(),
            dividends: Vec::new(),
            event_bus: None,
        }
    }

    /// Create a stock market wired to an EventBus.
    pub fn with_event_bus(event_bus: EventBus) -> Self {
        Self {
            event_bus: Some(event_bus),
            ..Self::new()
        }
    }

    fn emit(&self, event: WorldEvent) {
        if let Some(ref bus) = self.event_bus {
            bus.emit(event);
        }
    }

    // ── Stock Issuance ────────────────────────────────────

    /// Issue shares for an organization (creates a stock listing in PreIpo state).
    /// The org founder/leader calls this to set up the stock before IPO.
    pub fn issue_shares(
        &mut self,
        org_id: String,
        ticker: String,
        total_shares: u64,
        price_per_share: u64,
        tick: u64,
    ) -> Result<StockListing, StockMarketError> {
        if ticker.trim().is_empty() {
            return Err(StockMarketError::EmptyTicker);
        }
        let ticker = ticker.to_uppercase();
        if total_shares == 0 {
            return Err(StockMarketError::InvalidShareCount);
        }
        if price_per_share == 0 {
            return Err(StockMarketError::InvalidPrice);
        }
        if self.org_to_stock.contains_key(&org_id) {
            return Err(StockMarketError::AlreadyListed(org_id));
        }
        if self.ticker_to_stock.contains_key(&ticker) {
            return Err(StockMarketError::TickerTaken(ticker));
        }

        let stock_id = Uuid::new_v4().to_string();
        let listing = StockListing {
            id: stock_id.clone(),
            org_id: org_id.clone(),
            ticker: ticker.clone(),
            total_shares,
            price: price_per_share,
            status: ListingStatus::PreIpo,
            listed_tick: tick,
        };

        self.org_to_stock.insert(org_id.clone(), stock_id.clone());
        self.ticker_to_stock.insert(ticker, stock_id.clone());
        self.stocks.insert(stock_id.clone(), listing.clone());

        self.emit(WorldEvent::StockIssued {
            stock_id: stock_id.clone(),
            org_id,
            ticker: listing.ticker.clone(),
            total_shares,
            price: price_per_share,
        });

        Ok(listing)
    }

    /// Get a stock listing by ID.
    pub fn get_stock(&self, stock_id: &str) -> Option<&StockListing> {
        self.stocks.get(stock_id)
    }

    /// Get a mutable reference to a stock listing by ID.
    pub fn get_stock_mut(&mut self, stock_id: &str) -> Option<&mut StockListing> {
        self.stocks.get_mut(stock_id)
    }

    /// Get a stock listing by org ID.
    pub fn get_stock_by_org(&self, org_id: &str) -> Option<&StockListing> {
        self.org_to_stock
            .get(org_id)
            .and_then(|id| self.stocks.get(id))
    }

    /// Get a stock listing by ticker.
    pub fn get_stock_by_ticker(&self, ticker: &str) -> Option<&StockListing> {
        self.ticker_to_stock
            .get(&ticker.to_uppercase())
            .and_then(|id| self.stocks.get(id))
    }

    /// List all stocks.
    pub fn list_stocks(&self) -> Vec<&StockListing> {
        self.stocks.values().collect()
    }

    // ── IPO ───────────────────────────────────────────────

    /// Take a stock public (IPO). The org must meet conditions:
    /// - Org must have >= IPO_MIN_MEMBERS
    /// - Org treasury must be >= IPO_MIN_TREASURY
    ///
    /// After IPO, the stock becomes publicly tradeable.
    ///
    /// `org_member_count` and `org_treasury` are provided by the caller
    /// (typically looked up from OrganizationStore).
    pub fn ipo(
        &mut self,
        stock_id: &str,
        org_member_count: usize,
        org_treasury: u64,
        tick: u64,
    ) -> Result<StockListing, StockMarketError> {
        let stock = self
            .stocks
            .get_mut(stock_id)
            .ok_or_else(|| StockMarketError::StockNotFound(stock_id.to_string()))?;

        if stock.status == ListingStatus::Listed {
            return Err(StockMarketError::AlreadyListed(stock.org_id.clone()));
        }
        if stock.status == ListingStatus::Delisted {
            return Err(StockMarketError::Delisted);
        }

        // Validate IPO conditions
        if org_member_count < IPO_MIN_MEMBERS {
            return Err(StockMarketError::IpoConditionsNotMet(format!(
                "need at least {} members, have {}",
                IPO_MIN_MEMBERS, org_member_count
            )));
        }
        if org_treasury < IPO_MIN_TREASURY {
            return Err(StockMarketError::IpoConditionsNotMet(format!(
                "need at least {} treasury, have {}",
                IPO_MIN_TREASURY, org_treasury
            )));
        }

        stock.status = ListingStatus::Listed;
        stock.listed_tick = tick;

        let updated = stock.clone();

        self.emit(WorldEvent::StockIpo {
            stock_id: stock_id.to_string(),
            org_id: updated.org_id.clone(),
            ticker: updated.ticker.clone(),
            price: updated.price,
            total_shares: updated.total_shares,
        });

        Ok(updated)
    }

    // ── Share Transfers ───────────────────────────────────

    /// Transfer shares from one agent to another (direct transfer, no order book).
    pub fn transfer_shares(
        &mut self,
        stock_id: &str,
        from_agent: &str,
        to_agent: &str,
        quantity: u64,
    ) -> Result<(), StockMarketError> {
        let _stock = self
            .stocks
            .get(stock_id)
            .ok_or_else(|| StockMarketError::StockNotFound(stock_id.to_string()))?;

        // Debit from sender
        let from_key = (stock_id.to_string(), from_agent.to_string());
        let from_holding = self
            .holdings
            .get_mut(&from_key)
            .ok_or(StockMarketError::NotShareholder)?;

        if from_holding.quantity < quantity {
            return Err(StockMarketError::InsufficientShares(
                from_holding.quantity,
                quantity,
            ));
        }
        from_holding.quantity -= quantity;
        if from_holding.quantity == 0 {
            self.holdings.remove(&from_key);
        }

        // Credit to receiver
        let to_key = (stock_id.to_string(), to_agent.to_string());
        let to_holding = self.holdings.entry(to_key).or_insert_with(|| ShareHolding {
            agent_id: to_agent.to_string(),
            stock_id: stock_id.to_string(),
            quantity: 0,
        });
        to_holding.quantity += quantity;

        self.emit(WorldEvent::StockTransferred {
            stock_id: stock_id.to_string(),
            from_agent: from_agent.to_string(),
            to_agent: to_agent.to_string(),
            quantity,
        });

        Ok(())
    }

    /// Get share holding for an agent in a stock.
    pub fn get_holding(&self, stock_id: &str, agent_id: &str) -> Option<&ShareHolding> {
        self.holdings
            .get(&(stock_id.to_string(), agent_id.to_string()))
    }

    /// Get all holdings for an agent across all stocks.
    pub fn get_agent_holdings(&self, agent_id: &str) -> Vec<&ShareHolding> {
        self.holdings
            .values()
            .filter(|h| h.agent_id == agent_id)
            .collect()
    }

    /// Get all holdings for a stock.
    pub fn get_stock_holdings(&self, stock_id: &str) -> Vec<&ShareHolding> {
        self.holdings
            .values()
            .filter(|h| h.stock_id == stock_id)
            .collect()
    }

    /// Credit shares to an agent (used for IPO allocation and integration tests).
    pub fn credit_shares(&mut self, stock_id: &str, agent_id: &str, quantity: u64) {
        let key = (stock_id.to_string(), agent_id.to_string());
        let holding = self.holdings.entry(key).or_insert_with(|| ShareHolding {
            agent_id: agent_id.to_string(),
            stock_id: stock_id.to_string(),
            quantity: 0,
        });
        holding.quantity += quantity;
    }

    /// Internal: debit shares from an agent. Returns error if insufficient.
    fn debit_shares(
        &mut self,
        stock_id: &str,
        agent_id: &str,
        quantity: u64,
    ) -> Result<(), StockMarketError> {
        let key = (stock_id.to_string(), agent_id.to_string());
        let holding = self
            .holdings
            .get_mut(&key)
            .ok_or(StockMarketError::NotShareholder)?;

        if holding.quantity < quantity {
            return Err(StockMarketError::InsufficientShares(
                holding.quantity,
                quantity,
            ));
        }
        holding.quantity -= quantity;
        if holding.quantity == 0 {
            self.holdings.remove(&key);
        }
        Ok(())
    }

    // ── Order Book ────────────────────────────────────────

    /// Place a buy order.
    /// For limit orders, `price` must be > 0. For market orders, price is ignored.
    /// `agent_funds` is the agent's available money balance (checked externally).
    #[allow(clippy::too_many_arguments)]
    pub fn place_buy_order(
        &mut self,
        stock_id: &str,
        agent_id: &str,
        order_kind: OrderKind,
        price: u64,
        quantity: u64,
        agent_funds: u64,
        tick: u64,
    ) -> Result<Order, StockMarketError> {
        if quantity == 0 {
            return Err(StockMarketError::InvalidQuantity);
        }

        let stock = self
            .stocks
            .get(stock_id)
            .ok_or_else(|| StockMarketError::StockNotFound(stock_id.to_string()))?;

        if stock.status != ListingStatus::Listed {
            return Err(StockMarketError::NotListed);
        }

        let limit_price = match order_kind {
            OrderKind::Limit => {
                if price == 0 {
                    return Err(StockMarketError::InvalidPrice);
                }
                price
            }
            OrderKind::Market => stock.price, // Use current market price for fund check
        };

        // Check agent has enough funds
        let total_cost = limit_price.saturating_mul(quantity);
        if agent_funds < total_cost {
            return Err(StockMarketError::InsufficientFunds(agent_funds, total_cost));
        }

        let order = Order {
            id: Uuid::new_v4().to_string(),
            stock_id: stock_id.to_string(),
            agent_id: agent_id.to_string(),
            order_type: OrderType::Buy,
            order_kind,
            price: limit_price,
            quantity,
            filled_quantity: 0,
            status: OrderStatus::Open,
            created_tick: tick,
        };

        let order_id = order.id.clone();
        self.orders.insert(order_id.clone(), order.clone());

        // Try to match immediately
        self.match_orders(stock_id, tick)?;

        // Return the (possibly updated) order
        Ok(self.orders.get(&order_id).cloned().unwrap_or(order))
    }

    /// Place a sell order.
    /// The agent must hold enough shares of the stock.
    pub fn place_sell_order(
        &mut self,
        stock_id: &str,
        agent_id: &str,
        order_kind: OrderKind,
        price: u64,
        quantity: u64,
        tick: u64,
    ) -> Result<Order, StockMarketError> {
        if quantity == 0 {
            return Err(StockMarketError::InvalidQuantity);
        }

        let stock = self
            .stocks
            .get(stock_id)
            .ok_or_else(|| StockMarketError::StockNotFound(stock_id.to_string()))?;

        if stock.status != ListingStatus::Listed {
            return Err(StockMarketError::NotListed);
        }

        // Check agent holds enough shares
        let held = self
            .holdings
            .get(&(stock_id.to_string(), agent_id.to_string()))
            .map(|h| h.quantity)
            .unwrap_or(0);
        if held < quantity {
            return Err(StockMarketError::InsufficientShares(held, quantity));
        }

        let limit_price = match order_kind {
            OrderKind::Limit => {
                if price == 0 {
                    return Err(StockMarketError::InvalidPrice);
                }
                price
            }
            OrderKind::Market => stock.price,
        };

        let order = Order {
            id: Uuid::new_v4().to_string(),
            stock_id: stock_id.to_string(),
            agent_id: agent_id.to_string(),
            order_type: OrderType::Sell,
            order_kind,
            price: limit_price,
            quantity,
            filled_quantity: 0,
            status: OrderStatus::Open,
            created_tick: tick,
        };

        let order_id = order.id.clone();
        self.orders.insert(order_id.clone(), order.clone());

        // Try to match immediately
        self.match_orders(stock_id, tick)?;

        Ok(self.orders.get(&order_id).cloned().unwrap_or(order))
    }

    /// Cancel an open order.
    pub fn cancel_order(
        &mut self,
        order_id: &str,
        requester: &str,
    ) -> Result<Order, StockMarketError> {
        let order = self
            .orders
            .get_mut(order_id)
            .ok_or_else(|| StockMarketError::OrderNotFound(order_id.to_string()))?;

        if order.agent_id != requester {
            return Err(StockMarketError::OrderNotFound(order_id.to_string()));
        }
        if !order.is_active() {
            return Err(StockMarketError::OrderNotActive);
        }

        order.status = OrderStatus::Cancelled;
        Ok(order.clone())
    }

    /// Get an order by ID.
    pub fn get_order(&self, order_id: &str) -> Option<&Order> {
        self.orders.get(order_id)
    }

    /// List orders, optionally filtered by stock and/or agent.
    pub fn list_orders(&self, stock_id: Option<&str>, agent_id: Option<&str>) -> Vec<&Order> {
        self.orders
            .values()
            .filter(|o| stock_id.is_none_or(|s| o.stock_id == s))
            .filter(|o| agent_id.is_none_or(|a| o.agent_id == a))
            .collect()
    }

    /// List trades, optionally filtered by stock.
    pub fn list_trades(&self, stock_id: Option<&str>) -> Vec<&Trade> {
        self.trades
            .iter()
            .filter(|t| stock_id.is_none_or(|s| t.stock_id == s))
            .collect()
    }

    /// Get the order book for a stock: (buy_orders, sell_orders).
    /// Buy orders sorted by price descending (best bid first).
    /// Sell orders sorted by price ascending (best ask first).
    pub fn get_order_book(&self, stock_id: &str) -> (Vec<&Order>, Vec<&Order>) {
        let mut buys: Vec<&Order> = self
            .orders
            .values()
            .filter(|o| o.stock_id == stock_id && o.order_type == OrderType::Buy && o.is_active())
            .collect();
        buys.sort_by(|a, b| {
            b.price
                .cmp(&a.price)
                .then_with(|| a.created_tick.cmp(&b.created_tick))
        });

        let mut sells: Vec<&Order> = self
            .orders
            .values()
            .filter(|o| o.stock_id == stock_id && o.order_type == OrderType::Sell && o.is_active())
            .collect();
        sells.sort_by(|a, b| {
            a.price
                .cmp(&b.price)
                .then_with(|| a.created_tick.cmp(&b.created_tick))
        });

        (buys, sells)
    }

    // ── Order Matching ────────────────────────────────────

    /// Match buy and sell orders for a given stock.
    /// Uses price-time priority.
    fn match_orders(&mut self, stock_id: &str, tick: u64) -> Result<(), StockMarketError> {
        // Collect matching pairs first to avoid double borrow
        let matches = self.find_matches(stock_id);

        for (buy_id, sell_id, match_price, match_qty) in matches {
            self.execute_trade(&buy_id, &sell_id, match_price, match_qty, tick)?;
        }

        Ok(())
    }

    fn find_matches(&self, stock_id: &str) -> Vec<(String, String, u64, u64)> {
        let (buys, sells) = self.get_order_book(stock_id);
        let mut matches = Vec::new();

        // Track how much remaining quantity each order has during matching
        let mut buy_remaining: HashMap<String, u64> = HashMap::new();
        let mut sell_remaining: HashMap<String, u64> = HashMap::new();

        for buy in &buys {
            let buy_rem = buy_remaining
                .entry(buy.id.clone())
                .or_insert(buy.remaining());

            for sell in &sells {
                let sell_rem = sell_remaining
                    .entry(sell.id.clone())
                    .or_insert(sell.remaining());

                if *buy_rem == 0 || *sell_rem == 0 {
                    continue;
                }

                // Check price compatibility
                let prices_match = match (buy.order_kind, sell.order_kind) {
                    (OrderKind::Market, OrderKind::Market) => true,
                    (OrderKind::Market, OrderKind::Limit) => true,
                    (OrderKind::Limit, OrderKind::Market) => true,
                    (OrderKind::Limit, OrderKind::Limit) => buy.price >= sell.price,
                };

                if !prices_match {
                    // Since sells are sorted by price asc, if this sell doesn't match,
                    // neither will later ones with higher prices
                    if buy.order_kind == OrderKind::Limit
                        && sell.order_kind == OrderKind::Limit
                        && buy.price < sell.price
                    {
                        break;
                    }
                    continue;
                }

                // Determine execution price
                let exec_price = match (buy.order_kind, sell.order_kind) {
                    (OrderKind::Market, OrderKind::Limit) => sell.price,
                    (OrderKind::Limit, OrderKind::Market) => buy.price,
                    (OrderKind::Limit, OrderKind::Limit) => {
                        // Use the earlier order's price (price-time priority)
                        if buy.created_tick <= sell.created_tick {
                            buy.price
                        } else {
                            sell.price
                        }
                    }
                    (OrderKind::Market, OrderKind::Market) => {
                        // Use the stock's current price
                        self.stocks.get(stock_id).map(|s| s.price).unwrap_or(1)
                    }
                };

                let match_qty = (*buy_rem).min(*sell_rem);
                matches.push((buy.id.clone(), sell.id.clone(), exec_price, match_qty));

                *buy_rem -= match_qty;
                *sell_rem -= match_qty;
            }
        }

        matches
    }

    fn execute_trade(
        &mut self,
        buy_order_id: &str,
        sell_order_id: &str,
        price: u64,
        quantity: u64,
        tick: u64,
    ) -> Result<Trade, StockMarketError> {
        // Get buyer and seller IDs, and stock_id upfront (clone to release borrow)
        let (buyer_id, seller_id, stock_id) = {
            let buy = self
                .orders
                .get(buy_order_id)
                .ok_or_else(|| StockMarketError::OrderNotFound(buy_order_id.to_string()))?;
            let sell = self
                .orders
                .get(sell_order_id)
                .ok_or_else(|| StockMarketError::OrderNotFound(sell_order_id.to_string()))?;
            (
                buy.agent_id.clone(),
                sell.agent_id.clone(),
                buy.stock_id.clone(),
            )
        };

        // Calculate fee (0.5% of total value)
        let total_value = price.saturating_mul(quantity);
        let fee = total_value.saturating_mul(TRADING_FEE_BPS) / BPS_DENOMINATOR;

        // Transfer shares
        self.debit_shares(&stock_id, &seller_id, quantity)?;
        self.credit_shares(&stock_id, &buyer_id, quantity);

        // Update order statuses
        for oid in &[buy_order_id.to_string(), sell_order_id.to_string()] {
            if let Some(order) = self.orders.get_mut(oid) {
                order.filled_quantity += quantity;
                if order.filled_quantity >= order.quantity {
                    order.status = OrderStatus::Filled;
                } else {
                    order.status = OrderStatus::PartiallyFilled;
                }
            }
        }

        // Update stock price to last trade price
        if let Some(stock) = self.stocks.get_mut(&stock_id) {
            stock.price = price;
        }

        let trade = Trade {
            id: Uuid::new_v4().to_string(),
            stock_id: stock_id.clone(),
            buy_order_id: buy_order_id.to_string(),
            sell_order_id: sell_order_id.to_string(),
            buyer_id: buyer_id.clone(),
            seller_id: seller_id.clone(),
            price,
            quantity,
            fee,
            tick,
        };

        self.trades.push(trade.clone());

        self.emit(WorldEvent::StockTraded {
            trade_id: trade.id.clone(),
            stock_id: stock_id.clone(),
            buyer_id: buyer_id.clone(),
            seller_id: seller_id.clone(),
            price,
            quantity,
            fee,
        });

        Ok(trade)
    }

    // ── Dividends ─────────────────────────────────────────

    /// Distribute dividends to shareholders based on org profit.
    /// Returns the dividend record.
    pub fn distribute_dividends(
        &mut self,
        stock_id: &str,
        total_profit: u64,
        tick: u64,
    ) -> Result<DividendRecord, StockMarketError> {
        let stock = self
            .stocks
            .get(stock_id)
            .ok_or_else(|| StockMarketError::StockNotFound(stock_id.to_string()))?;

        if total_profit == 0 {
            return Err(StockMarketError::NoProfitToDistribute);
        }

        let holdings = self.get_stock_holdings(stock_id);
        if holdings.is_empty() {
            return Err(StockMarketError::NoSharesIssued(stock_id.to_string()));
        }

        // Calculate dividend per share
        let dividend_per_share = total_profit / stock.total_shares;

        let mut recipients = Vec::new();
        for holding in &holdings {
            let amount = dividend_per_share.saturating_mul(holding.quantity);
            if amount > 0 {
                recipients.push(DividendRecipient {
                    agent_id: holding.agent_id.clone(),
                    shares: holding.quantity,
                    amount,
                });
            }
        }

        let record = DividendRecord {
            id: Uuid::new_v4().to_string(),
            stock_id: stock_id.to_string(),
            org_id: stock.org_id.clone(),
            total_profit,
            dividend_per_share,
            tick,
            recipients: recipients.clone(),
        };

        self.dividends.push(record.clone());

        self.emit(WorldEvent::StockDividend {
            dividend_id: record.id.clone(),
            stock_id: stock_id.to_string(),
            org_id: stock.org_id.clone(),
            total_profit,
            dividend_per_share,
            recipient_count: recipients.len(),
        });

        Ok(record)
    }

    /// List dividend history, optionally filtered by stock.
    pub fn list_dividends(&self, stock_id: Option<&str>) -> Vec<&DividendRecord> {
        self.dividends
            .iter()
            .filter(|d| stock_id.is_none_or(|s| d.stock_id == s))
            .collect()
    }

    // ── Delist ────────────────────────────────────────────

    /// Delist a stock (e.g. when org dissolves).
    pub fn delist(&mut self, stock_id: &str) -> Result<(), StockMarketError> {
        let stock = self
            .stocks
            .get_mut(stock_id)
            .ok_or_else(|| StockMarketError::StockNotFound(stock_id.to_string()))?;

        stock.status = ListingStatus::Delisted;

        // Cancel all open orders for this stock
        for order in self.orders.values_mut() {
            if order.stock_id == stock_id && order.is_active() {
                order.status = OrderStatus::Cancelled;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_stock_market() -> StockMarket {
        StockMarket::new()
    }

    #[test]
    fn issue_shares_success() {
        let mut sm = make_stock_market();
        let stock = sm
            .issue_shares("org-1".into(), "ACME".into(), 1000, 10, 100)
            .unwrap();
        assert_eq!(stock.org_id, "org-1");
        assert_eq!(stock.ticker, "ACME");
        assert_eq!(stock.total_shares, 1000);
        assert_eq!(stock.price, 10);
        assert_eq!(stock.status, ListingStatus::PreIpo);
    }

    #[test]
    fn issue_shares_rejects_empty_ticker() {
        let mut sm = make_stock_market();
        let result = sm.issue_shares("org-1".into(), "".into(), 1000, 10, 100);
        assert!(result.is_err());
    }

    #[test]
    fn issue_shares_rejects_zero_shares() {
        let mut sm = make_stock_market();
        let result = sm.issue_shares("org-1".into(), "ACME".into(), 0, 10, 100);
        assert!(result.is_err());
    }

    #[test]
    fn issue_shares_rejects_zero_price() {
        let mut sm = make_stock_market();
        let result = sm.issue_shares("org-1".into(), "ACME".into(), 1000, 0, 100);
        assert!(result.is_err());
    }

    #[test]
    fn issue_shares_rejects_duplicate_org() {
        let mut sm = make_stock_market();
        sm.issue_shares("org-1".into(), "ACME".into(), 1000, 10, 100)
            .unwrap();
        let result = sm.issue_shares("org-1".into(), "ACM2".into(), 1000, 10, 100);
        assert!(result.is_err());
    }

    #[test]
    fn issue_shares_rejects_duplicate_ticker() {
        let mut sm = make_stock_market();
        sm.issue_shares("org-1".into(), "ACME".into(), 1000, 10, 100)
            .unwrap();
        let result = sm.issue_shares("org-2".into(), "ACME".into(), 1000, 10, 100);
        assert!(result.is_err());
    }

    #[test]
    fn ipo_success() {
        let mut sm = make_stock_market();
        let stock = sm
            .issue_shares("org-1".into(), "ACME".into(), 1000, 10, 100)
            .unwrap();
        let updated = sm
            .ipo(&stock.id, IPO_MIN_MEMBERS, IPO_MIN_TREASURY, 200)
            .unwrap();
        assert_eq!(updated.status, ListingStatus::Listed);
        assert_eq!(updated.listed_tick, 200);
    }

    #[test]
    fn ipo_rejects_insufficient_members() {
        let mut sm = make_stock_market();
        let stock = sm
            .issue_shares("org-1".into(), "ACME".into(), 1000, 10, 100)
            .unwrap();
        let result = sm.ipo(&stock.id, 1, IPO_MIN_TREASURY, 200);
        assert!(result.is_err());
    }

    #[test]
    fn ipo_rejects_insufficient_treasury() {
        let mut sm = make_stock_market();
        let stock = sm
            .issue_shares("org-1".into(), "ACME".into(), 1000, 10, 100)
            .unwrap();
        let result = sm.ipo(&stock.id, IPO_MIN_MEMBERS, 100, 200);
        assert!(result.is_err());
    }

    #[test]
    fn transfer_shares_success() {
        let mut sm = make_stock_market();
        let stock = sm
            .issue_shares("org-1".into(), "ACME".into(), 1000, 10, 100)
            .unwrap();

        // Credit shares to agent-1
        sm.credit_shares(&stock.id, "agent-1", 100);

        sm.transfer_shares(&stock.id, "agent-1", "agent-2", 50)
            .unwrap();

        assert_eq!(sm.get_holding(&stock.id, "agent-1").unwrap().quantity, 50);
        assert_eq!(sm.get_holding(&stock.id, "agent-2").unwrap().quantity, 50);
    }

    #[test]
    fn transfer_shares_insufficient() {
        let mut sm = make_stock_market();
        let stock = sm
            .issue_shares("org-1".into(), "ACME".into(), 1000, 10, 100)
            .unwrap();
        sm.credit_shares(&stock.id, "agent-1", 10);

        let result = sm.transfer_shares(&stock.id, "agent-1", "agent-2", 50);
        assert!(result.is_err());
    }

    #[test]
    fn buy_order_limit_matched_immediately() {
        let mut sm = make_stock_market();
        let stock = sm
            .issue_shares("org-1".into(), "ACME".into(), 1000, 10, 100)
            .unwrap();
        sm.ipo(&stock.id, IPO_MIN_MEMBERS, IPO_MIN_TREASURY, 200)
            .unwrap();

        // Agent-1 holds shares and places a sell order
        sm.credit_shares(&stock.id, "agent-1", 100);
        sm.place_sell_order(&stock.id, "agent-1", OrderKind::Limit, 10, 50, 300)
            .unwrap();

        // Agent-2 places a buy order at the same price
        let order = sm
            .place_buy_order(&stock.id, "agent-2", OrderKind::Limit, 10, 50, 10000, 300)
            .unwrap();
        assert_eq!(order.status, OrderStatus::Filled);
        assert_eq!(order.filled_quantity, 50);

        // Check holdings
        assert_eq!(sm.get_holding(&stock.id, "agent-2").unwrap().quantity, 50);
        assert_eq!(sm.get_holding(&stock.id, "agent-1").unwrap().quantity, 50); // 100 - 50 sold
    }

    #[test]
    fn buy_order_market() {
        let mut sm = make_stock_market();
        let stock = sm
            .issue_shares("org-1".into(), "ACME".into(), 1000, 10, 100)
            .unwrap();
        sm.ipo(&stock.id, IPO_MIN_MEMBERS, IPO_MIN_TREASURY, 200)
            .unwrap();

        sm.credit_shares(&stock.id, "agent-1", 100);
        sm.place_sell_order(&stock.id, "agent-1", OrderKind::Limit, 10, 50, 300)
            .unwrap();

        let order = sm
            .place_buy_order(&stock.id, "agent-2", OrderKind::Market, 0, 50, 10000, 300)
            .unwrap();
        assert_eq!(order.status, OrderStatus::Filled);
    }

    #[test]
    fn buy_order_insufficient_funds() {
        let mut sm = make_stock_market();
        let stock = sm
            .issue_shares("org-1".into(), "ACME".into(), 1000, 10, 100)
            .unwrap();
        sm.ipo(&stock.id, IPO_MIN_MEMBERS, IPO_MIN_TREASURY, 200)
            .unwrap();

        let result = sm.place_buy_order(&stock.id, "agent-2", OrderKind::Limit, 10, 50, 100, 300);
        assert!(result.is_err());
    }

    #[test]
    fn sell_order_insufficient_shares() {
        let mut sm = make_stock_market();
        let stock = sm
            .issue_shares("org-1".into(), "ACME".into(), 1000, 10, 100)
            .unwrap();
        sm.ipo(&stock.id, IPO_MIN_MEMBERS, IPO_MIN_TREASURY, 200)
            .unwrap();

        let result = sm.place_sell_order(&stock.id, "agent-1", OrderKind::Limit, 10, 50, 300);
        assert!(result.is_err());
    }

    #[test]
    fn cancel_order_success() {
        let mut sm = make_stock_market();
        let stock = sm
            .issue_shares("org-1".into(), "ACME".into(), 1000, 10, 100)
            .unwrap();
        sm.ipo(&stock.id, IPO_MIN_MEMBERS, IPO_MIN_TREASURY, 200)
            .unwrap();

        let order = sm
            .place_buy_order(&stock.id, "agent-1", OrderKind::Limit, 10, 50, 10000, 300)
            .unwrap();
        let cancelled = sm.cancel_order(&order.id, "agent-1").unwrap();
        assert_eq!(cancelled.status, OrderStatus::Cancelled);
    }

    #[test]
    fn cancel_order_wrong_agent() {
        let mut sm = make_stock_market();
        let stock = sm
            .issue_shares("org-1".into(), "ACME".into(), 1000, 10, 100)
            .unwrap();
        sm.ipo(&stock.id, IPO_MIN_MEMBERS, IPO_MIN_TREASURY, 200)
            .unwrap();

        let order = sm
            .place_buy_order(&stock.id, "agent-1", OrderKind::Limit, 10, 50, 10000, 300)
            .unwrap();
        let result = sm.cancel_order(&order.id, "agent-2");
        assert!(result.is_err());
    }

    #[test]
    fn partial_fill() {
        let mut sm = make_stock_market();
        let stock = sm
            .issue_shares("org-1".into(), "ACME".into(), 1000, 10, 100)
            .unwrap();
        sm.ipo(&stock.id, IPO_MIN_MEMBERS, IPO_MIN_TREASURY, 200)
            .unwrap();

        sm.credit_shares(&stock.id, "agent-1", 100);
        sm.place_sell_order(&stock.id, "agent-1", OrderKind::Limit, 10, 100, 300)
            .unwrap();

        // Buy only 30 shares
        let order = sm
            .place_buy_order(&stock.id, "agent-2", OrderKind::Limit, 10, 30, 10000, 300)
            .unwrap();
        assert_eq!(order.status, OrderStatus::Filled);

        // Sell order should be partially filled
        let sell_orders: Vec<_> = sm
            .list_orders(Some(&stock.id), None)
            .into_iter()
            .filter(|o| o.order_type == OrderType::Sell)
            .collect();
        assert_eq!(sell_orders[0].filled_quantity, 30);
        assert_eq!(sell_orders[0].remaining(), 70);
    }

    #[test]
    fn trading_fee_calculated() {
        let mut sm = make_stock_market();
        let stock = sm
            .issue_shares("org-1".into(), "ACME".into(), 1000, 10, 100)
            .unwrap();
        sm.ipo(&stock.id, IPO_MIN_MEMBERS, IPO_MIN_TREASURY, 200)
            .unwrap();

        sm.credit_shares(&stock.id, "agent-1", 100);
        sm.place_sell_order(&stock.id, "agent-1", OrderKind::Limit, 10, 50, 300)
            .unwrap();
        sm.place_buy_order(&stock.id, "agent-2", OrderKind::Limit, 10, 50, 10000, 300)
            .unwrap();

        let trades = sm.list_trades(Some(&stock.id));
        assert_eq!(trades.len(), 1);
        // 10 * 50 = 500, fee = 500 * 0.005 = 2.5, but integer math: 500 * 50 / 10000 = 2
        assert_eq!(trades[0].fee, 2);
    }

    #[test]
    fn distribute_dividends_success() {
        let mut sm = make_stock_market();
        let stock = sm
            .issue_shares("org-1".into(), "ACME".into(), 1000, 10, 100)
            .unwrap();

        sm.credit_shares(&stock.id, "agent-1", 600);
        sm.credit_shares(&stock.id, "agent-2", 400);

        let record = sm.distribute_dividends(&stock.id, 1000, 200).unwrap();
        assert_eq!(record.total_profit, 1000);
        assert_eq!(record.dividend_per_share, 1); // 1000 / 1000 shares
        assert_eq!(record.recipients.len(), 2);

        // agent-1 gets 600 * 1 = 600
        assert_eq!(
            record
                .recipients
                .iter()
                .find(|r| r.agent_id == "agent-1")
                .unwrap()
                .amount,
            600
        );
        // agent-2 gets 400 * 1 = 400
        assert_eq!(
            record
                .recipients
                .iter()
                .find(|r| r.agent_id == "agent-2")
                .unwrap()
                .amount,
            400
        );
    }

    #[test]
    fn distribute_dividends_zero_profit() {
        let mut sm = make_stock_market();
        let stock = sm
            .issue_shares("org-1".into(), "ACME".into(), 1000, 10, 100)
            .unwrap();
        sm.credit_shares(&stock.id, "agent-1", 600);

        let result = sm.distribute_dividends(&stock.id, 0, 200);
        assert!(result.is_err());
    }

    #[test]
    fn delist_cancels_orders() {
        let mut sm = make_stock_market();
        let stock = sm
            .issue_shares("org-1".into(), "ACME".into(), 1000, 10, 100)
            .unwrap();
        sm.ipo(&stock.id, IPO_MIN_MEMBERS, IPO_MIN_TREASURY, 200)
            .unwrap();

        sm.place_buy_order(&stock.id, "agent-1", OrderKind::Limit, 10, 50, 10000, 300)
            .unwrap();
        sm.delist(&stock.id).unwrap();

        let stock = sm.get_stock(&stock.id).unwrap();
        assert_eq!(stock.status, ListingStatus::Delisted);

        let orders: Vec<_> = sm
            .list_orders(Some(&stock.id), None)
            .into_iter()
            .filter(|o| o.status == OrderStatus::Cancelled)
            .collect();
        assert_eq!(orders.len(), 1);
    }

    #[test]
    fn get_stock_by_ticker_case_insensitive() {
        let mut sm = make_stock_market();
        sm.issue_shares("org-1".into(), "acme".into(), 1000, 10, 100)
            .unwrap();

        assert!(sm.get_stock_by_ticker("ACME").is_some());
        assert!(sm.get_stock_by_ticker("acme").is_some());
    }

    #[test]
    fn order_book_sorted_correctly() {
        let mut sm = make_stock_market();
        let stock = sm
            .issue_shares("org-1".into(), "ACME".into(), 10000, 10, 100)
            .unwrap();
        sm.ipo(&stock.id, IPO_MIN_MEMBERS, IPO_MIN_TREASURY, 200)
            .unwrap();

        sm.credit_shares(&stock.id, "seller", 1000);

        // Multiple sell orders at different prices
        sm.place_sell_order(&stock.id, "seller", OrderKind::Limit, 12, 10, 300)
            .unwrap();
        sm.place_sell_order(&stock.id, "seller", OrderKind::Limit, 11, 10, 300)
            .unwrap();
        sm.place_sell_order(&stock.id, "seller", OrderKind::Limit, 13, 10, 300)
            .unwrap();

        // Multiple buy orders at different prices (none high enough to match sells)
        sm.place_buy_order(&stock.id, "buyer-1", OrderKind::Limit, 9, 10, 10000, 300)
            .unwrap();
        sm.place_buy_order(&stock.id, "buyer-2", OrderKind::Limit, 8, 10, 10000, 300)
            .unwrap();
        sm.place_buy_order(&stock.id, "buyer-3", OrderKind::Limit, 7, 10, 10000, 300)
            .unwrap();

        let (buys, sells) = sm.get_order_book(&stock.id);

        // Buys: best bid first (highest price)
        assert_eq!(buys[0].price, 9);
        assert_eq!(buys[1].price, 8);
        assert_eq!(buys[2].price, 7);

        // Sells: best ask first (lowest price)
        assert_eq!(sells[0].price, 11);
        assert_eq!(sells[1].price, 12);
        assert_eq!(sells[2].price, 13);
    }

    #[test]
    fn cannot_trade_pre_ipo() {
        let mut sm = make_stock_market();
        let stock = sm
            .issue_shares("org-1".into(), "ACME".into(), 1000, 10, 100)
            .unwrap();
        // Not IPO'd yet

        let result = sm.place_buy_order(&stock.id, "agent-1", OrderKind::Limit, 10, 50, 10000, 300);
        assert!(result.is_err());
    }

    #[test]
    fn get_agent_holdings() {
        let mut sm = make_stock_market();
        let stock1 = sm
            .issue_shares("org-1".into(), "AAA".into(), 1000, 10, 100)
            .unwrap();
        let stock2 = sm
            .issue_shares("org-2".into(), "BBB".into(), 1000, 20, 100)
            .unwrap();

        sm.credit_shares(&stock1.id, "agent-1", 100);
        sm.credit_shares(&stock2.id, "agent-1", 200);

        let holdings = sm.get_agent_holdings("agent-1");
        assert_eq!(holdings.len(), 2);
    }

    // ── list_trades (backs the /stocks/:id/history endpoint) ──

    /// Helper: set up a listed stock with shares credited to a seller so
    /// trades can execute at a chosen tick.
    fn setup_listed_stock_with_shares() -> (StockMarket, String) {
        let mut sm = make_stock_market();
        let stock = sm
            .issue_shares("org-1".into(), "ACME".into(), 10000, 10, 100)
            .unwrap();
        sm.ipo(&stock.id, IPO_MIN_MEMBERS, IPO_MIN_TREASURY, 200)
            .unwrap();
        sm.credit_shares(&stock.id, "seller", 1000);
        (sm, stock.id)
    }

    #[test]
    fn list_trades_empty_for_stock_with_no_trades() {
        let (sm, stock_id) = setup_listed_stock_with_shares();
        // No trades placed yet.
        let trades = sm.list_trades(Some(&stock_id));
        assert!(trades.is_empty());
    }

    #[test]
    fn list_trades_returns_only_matching_stock() {
        let (mut sm, stock_a) = setup_listed_stock_with_shares();
        let stock_b = sm
            .issue_shares("org-2".into(), "BEE".into(), 10000, 10, 100)
            .unwrap();
        sm.ipo(&stock_b.id, IPO_MIN_MEMBERS, IPO_MIN_TREASURY, 200)
            .unwrap();
        sm.credit_shares(&stock_b.id, "seller-b", 1000);

        // One trade on stock A at tick 300
        sm.place_sell_order(&stock_a, "seller", OrderKind::Limit, 10, 50, 300)
            .unwrap();
        sm.place_buy_order(&stock_a, "buyer", OrderKind::Limit, 10, 50, 10000, 300)
            .unwrap();

        // One trade on stock B at tick 300
        sm.place_sell_order(&stock_b.id, "seller-b", OrderKind::Limit, 20, 10, 300)
            .unwrap();
        sm.place_buy_order(&stock_b.id, "buyer-b", OrderKind::Limit, 20, 10, 10000, 300)
            .unwrap();

        let a_trades = sm.list_trades(Some(&stock_a));
        assert_eq!(a_trades.len(), 1);
        assert_eq!(a_trades[0].stock_id, stock_a);

        let b_trades = sm.list_trades(Some(&stock_b.id));
        assert_eq!(b_trades.len(), 1);
        assert_eq!(b_trades[0].stock_id, stock_b.id);
    }

    #[test]
    fn list_trades_multiple_trades_same_tick_aggregate_correctly() {
        let (mut sm, stock_id) = setup_listed_stock_with_shares();

        // Two trades at tick 300, two at tick 310 — different prices.
        // Sell 1 @ 10 (tick 300)
        sm.place_sell_order(&stock_id, "seller", OrderKind::Limit, 10, 20, 300)
            .unwrap();
        sm.place_buy_order(&stock_id, "buyer", OrderKind::Limit, 10, 20, 10000, 300)
            .unwrap();

        // Sell 2 @ 12 (tick 300)
        sm.place_sell_order(&stock_id, "seller", OrderKind::Limit, 12, 10, 300)
            .unwrap();
        sm.place_buy_order(&stock_id, "buyer", OrderKind::Limit, 12, 10, 10000, 300)
            .unwrap();

        // Sell 3 @ 15 (tick 310)
        sm.place_sell_order(&stock_id, "seller", OrderKind::Limit, 15, 5, 310)
            .unwrap();
        sm.place_buy_order(&stock_id, "buyer", OrderKind::Limit, 15, 5, 10000, 310)
            .unwrap();

        let trades = sm.list_trades(Some(&stock_id));
        assert_eq!(trades.len(), 3);

        // Verify trades are chronologically ordered by checking tick values.
        let ticks: Vec<u64> = trades.iter().map(|t| t.tick).collect();
        assert_eq!(ticks, vec![300, 300, 310]);

        // Verify the aggregation logic the endpoint uses:
        // last price per tick wins, volumes sum.
        let mut by_tick: std::collections::BTreeMap<u64, (u64, u64)> =
            std::collections::BTreeMap::new();
        for t in &trades {
            let entry = by_tick.entry(t.tick).or_insert((t.price, 0));
            entry.0 = t.price;
            entry.1 += t.quantity;
        }

        let points: Vec<(u64, u64, u64)> = by_tick
            .into_iter()
            .map(|(tick, (price, volume))| (tick, price, volume))
            .collect();

        // Two aggregated points, ascending by tick.
        assert_eq!(points.len(), 2);
        assert_eq!(points[0], (300, 12, 30)); // last price 12, total qty 20+10
        assert_eq!(points[1], (310, 15, 5)); // single trade
    }
}
