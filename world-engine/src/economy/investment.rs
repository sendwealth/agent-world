use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::world::event::WorldEvent;
use crate::world::state::EventBus;

// ── Constants ─────────────────────────────────────────────

/// Default max share a single investor can hold (30% = 3000 bps).
pub const DEFAULT_MAX_INVESTOR_SHARE_BPS: u64 = 3000;
/// Default max total issued shares as fraction of total (70% = 7000 bps).
pub const DEFAULT_MAX_TOTAL_SHARE_BPS: u64 = 7000;
/// Default virtual-to-token exchange rate (1 virtual = 100 tokens).
pub const DEFAULT_VIRTUAL_TO_TOKEN_RATE: u64 = 100;
/// Basis point denominator.
pub const BPS_DENOMINATOR: u64 = 10_000;

// ── Enums ─────────────────────────────────────────────────

/// Entity type that can be invested in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InvestmentEntityType {
    Agent,
    Organization,
}

/// Lifecycle state of an investment product.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InvestmentStatus {
    Active,
    Frozen,
    Closed,
}

/// Status of an individual investor position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PositionStatus {
    Active,
    Withdrawing,
    Closed,
}

// ── Data Structures ───────────────────────────────────────

/// Configuration for the investment system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvestmentConfig {
    /// Max fraction (bps) a single investor can hold of a product.
    pub max_investor_share_bps: u64,
    /// Max total fraction (bps) that can be issued.
    pub max_total_share_bps: u64,
    /// Virtual currency to token exchange rate.
    pub virtual_to_token_rate: u64,
}

impl Default for InvestmentConfig {
    fn default() -> Self {
        Self {
            max_investor_share_bps: DEFAULT_MAX_INVESTOR_SHARE_BPS,
            max_total_share_bps: DEFAULT_MAX_TOTAL_SHARE_BPS,
            virtual_to_token_rate: DEFAULT_VIRTUAL_TO_TOKEN_RATE,
        }
    }
}

/// An investment product backed by an Agent or Organization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvestmentProduct {
    pub id: String,
    pub target_id: String,
    pub target_name: String,
    pub entity_type: InvestmentEntityType,
    pub total_shares: u64,
    pub issued_shares: u64,
    pub price_per_share: u64,
    pub performance_score: f64,
    pub status: InvestmentStatus,
    /// The owner/creator of this product (the entity being invested in).
    pub owner_id: String,
    pub created_tick: u64,
}

/// An investor's position in a product.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvestmentPosition {
    pub id: String,
    pub product_id: String,
    pub investor_id: String,
    pub shares: u64,
    pub avg_buy_price: u64,
    pub cost_basis: u64,
    pub status: PositionStatus,
    pub created_tick: u64,
}

/// A transaction record (buy/sell).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvestmentTransaction {
    pub id: String,
    pub product_id: String,
    pub investor_id: String,
    pub tx_type: InvestmentTxType,
    pub shares: u64,
    pub price_per_share: u64,
    pub total_amount: u64,
    pub idempotency_key: Option<String>,
    pub tick: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InvestmentTxType {
    Buy,
    Sell,
}

/// A dividend distribution record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DividendDistribution {
    pub id: String,
    pub product_id: String,
    pub target_id: String,
    pub total_profit: u64,
    pub dividend_per_share: u64,
    pub recipients: Vec<DividendRecipient>,
    pub distributor_id: String,
    pub tick: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DividendRecipient {
    pub investor_id: String,
    pub shares: u64,
    pub amount: u64,
}

/// Portfolio entry for an investor in a single product.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioEntry {
    pub product_id: String,
    pub target_name: String,
    pub shares: u64,
    pub avg_buy_price: u64,
    pub cost_basis: u64,
    pub current_value: u64,
    pub pnl: i64,
    pub status: PositionStatus,
}

/// Leaderboard entry sorted by total portfolio value.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaderboardEntry {
    pub investor_id: String,
    pub total_value: u64,
    pub position_count: usize,
}

// ── Request Types ─────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreateProductRequest {
    pub target_id: String,
    pub target_name: String,
    pub entity_type: InvestmentEntityType,
    pub total_shares: u64,
    pub price_per_share: u64,
    /// The entity owner — must match the target_id's owner in a real system.
    pub owner_id: String,
}

#[derive(Debug, Deserialize)]
pub struct BuySharesRequest {
    pub product_id: String,
    pub investor_id: String,
    pub shares: u64,
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SellSharesRequest {
    pub product_id: String,
    pub investor_id: String,
    pub shares: u64,
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CloseInvestmentRequest {
    pub product_id: String,
    /// The entity requesting closure — must be the product owner.
    pub requester_id: String,
}

#[derive(Debug, Deserialize)]
pub struct DistributeReturnsRequest {
    pub product_id: String,
    pub profit: u64,
    /// The entity distributing returns — must be the product owner.
    pub distributor_id: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePerformanceRequest {
    pub product_id: String,
    pub performance_score: f64,
}

#[derive(Debug, Deserialize)]
pub struct FreezeProductRequest {
    pub product_id: String,
    /// Must be the product owner.
    pub requester_id: String,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct ListTransactionsQuery {
    pub product_id: Option<String>,
    pub investor_id: Option<String>,
}

// ── Error Type ────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum InvestmentError {
    #[error("product not found: {0}")]
    ProductNotFound(String),
    #[error("position not found: {0}")]
    PositionNotFound(String),
    #[error("product already exists for target: {0}")]
    ProductAlreadyExists(String),
    #[error("product is not active")]
    ProductNotActive,
    #[error("product is frozen")]
    ProductFrozen,
    #[error("product is closed")]
    ProductClosed,
    #[error("self-investment not allowed")]
    SelfInvestment,
    #[error("insufficient shares: have {0}, need {1}")]
    InsufficientShares(u64, u64),
    #[error("share count must be > 0")]
    InvalidShareCount,
    #[error("price must be > 0")]
    InvalidPrice,
    #[error("total shares must be > 0")]
    InvalidTotalShares,
    #[error("investor share limit exceeded: would hold {0} bps, max {1} bps")]
    InvestorShareLimit(u64, u64),
    #[error("total issue limit exceeded: would issue {0} bps, max {1} bps")]
    TotalShareLimit(u64, u64),
    #[error("no position to sell")]
    NoPosition,
    #[error("no profit to distribute")]
    NoProfitToDistribute,
    #[error("no shareholders to distribute to")]
    NoShareholders,
    #[error("duplicate idempotency key: {0}")]
    DuplicateIdempotencyKey(String),
    #[error("authorization failed: {0}")]
    Unauthorized(String),
    #[error("performance score must be between 0.0 and 10.0")]
    InvalidPerformanceScore,
}

// ── Investment System ─────────────────────────────────────

pub struct InvestmentSystem {
    products: HashMap<String, InvestmentProduct>,
    /// target_id -> product_id (one product per target).
    target_to_product: HashMap<String, String>,
    positions: HashMap<String, InvestmentPosition>,
    /// (product_id, investor_id) -> position_id.
    position_index: HashMap<(String, String), String>,
    transactions: Vec<InvestmentTransaction>,
    dividends: Vec<DividendDistribution>,
    /// Idempotency key dedup.
    idempotency_keys: HashMap<String, String>,
    config: InvestmentConfig,
    event_bus: Option<EventBus>,
    current_tick: u64,
}

impl Default for InvestmentSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl InvestmentSystem {
    pub fn new() -> Self {
        Self {
            products: HashMap::new(),
            target_to_product: HashMap::new(),
            positions: HashMap::new(),
            position_index: HashMap::new(),
            transactions: Vec::new(),
            dividends: Vec::new(),
            idempotency_keys: HashMap::new(),
            config: InvestmentConfig::default(),
            event_bus: None,
            current_tick: 0,
        }
    }

    pub fn with_config(config: InvestmentConfig) -> Self {
        Self {
            config,
            ..Self::new()
        }
    }

    pub fn with_event_bus(event_bus: EventBus) -> Self {
        Self {
            event_bus: Some(event_bus),
            ..Self::new()
        }
    }

    pub fn with_config_and_event_bus(config: InvestmentConfig, event_bus: EventBus) -> Self {
        Self {
            config,
            event_bus: Some(event_bus),
            ..Self::new()
        }
    }

    pub fn set_tick(&mut self, tick: u64) {
        self.current_tick = tick;
    }

    fn emit(&self, event: WorldEvent) {
        if let Some(ref bus) = self.event_bus {
            bus.emit(event);
        }
    }

    // ── Product Management ────────────────────────────────

    /// Create a new investment product. The `owner_id` is recorded and later
    /// required for privileged operations (close, distribute, freeze).
    pub fn create_product(
        &mut self,
        req: CreateProductRequest,
    ) -> Result<InvestmentProduct, InvestmentError> {
        if req.total_shares == 0 {
            return Err(InvestmentError::InvalidTotalShares);
        }
        if req.price_per_share == 0 {
            return Err(InvestmentError::InvalidPrice);
        }
        if self.target_to_product.contains_key(&req.target_id) {
            return Err(InvestmentError::ProductAlreadyExists(req.target_id.clone()));
        }

        let product_id = Uuid::new_v4().to_string();
        let product = InvestmentProduct {
            id: product_id.clone(),
            target_id: req.target_id.clone(),
            target_name: req.target_name,
            entity_type: req.entity_type,
            total_shares: req.total_shares,
            issued_shares: 0,
            price_per_share: req.price_per_share,
            performance_score: 5.0,
            status: InvestmentStatus::Active,
            owner_id: req.owner_id,
            created_tick: self.current_tick,
        };

        self.target_to_product
            .insert(req.target_id.clone(), product_id.clone());
        self.products.insert(product_id.clone(), product.clone());

        self.emit(WorldEvent::InvestmentProductCreated {
            product_id: product_id.clone(),
            target_id: req.target_id,
            total_shares: req.total_shares,
            price: req.price_per_share,
        });

        Ok(product)
    }

    pub fn get_product(&self, product_id: &str) -> Option<&InvestmentProduct> {
        self.products.get(product_id)
    }

    pub fn get_product_by_target(&self, target_id: &str) -> Option<&InvestmentProduct> {
        self.target_to_product
            .get(target_id)
            .and_then(|id| self.products.get(id))
    }

    pub fn list_products(&self) -> Vec<&InvestmentProduct> {
        self.products.values().collect()
    }

    /// Freeze a product — only the product owner can freeze.
    pub fn freeze_product(
        &mut self,
        req: FreezeProductRequest,
    ) -> Result<InvestmentProduct, InvestmentError> {
        let product = self
            .products
            .get(&req.product_id)
            .ok_or_else(|| InvestmentError::ProductNotFound(req.product_id.clone()))?;

        // P0 FIX: Authorization check
        if product.owner_id != req.requester_id {
            return Err(InvestmentError::Unauthorized(
                "only the product owner can freeze".to_string(),
            ));
        }

        if product.status == InvestmentStatus::Closed {
            return Err(InvestmentError::ProductClosed);
        }

        let product = self.products.get_mut(&req.product_id).unwrap();
        product.status = InvestmentStatus::Frozen;
        Ok(product.clone())
    }

    /// Update performance score — adjusts price based on score.
    pub fn update_performance(
        &mut self,
        product_id: &str,
        score: f64,
    ) -> Result<InvestmentProduct, InvestmentError> {
        if !(0.0..=10.0).contains(&score) {
            return Err(InvestmentError::InvalidPerformanceScore);
        }

        let product = self
            .products
            .get_mut(product_id)
            .ok_or_else(|| InvestmentError::ProductNotFound(product_id.to_string()))?;

        if product.status != InvestmentStatus::Active {
            return Err(InvestmentError::ProductNotActive);
        }

        let old_score = product.performance_score;
        product.performance_score = score;

        // Adjust price: sensitivity 0.5, delta applied proportionally
        let delta = score - old_score;
        let price_adjustment = (product.price_per_share as f64 * delta * 0.5 / 10.0) as i64;
        let new_price = (product.price_per_share as i64 + price_adjustment).max(1) as u64;
        product.price_per_share = new_price;

        Ok(product.clone())
    }

    // ── Buy / Sell ────────────────────────────────────────

    /// Buy shares in an investment product.
    pub fn buy_shares(
        &mut self,
        req: BuySharesRequest,
    ) -> Result<(InvestmentPosition, InvestmentTransaction), InvestmentError> {
        if req.shares == 0 {
            return Err(InvestmentError::InvalidShareCount);
        }

        // Check idempotency
        if let Some(ref key) = req.idempotency_key {
            if self.idempotency_keys.contains_key(key) {
                return Err(InvestmentError::DuplicateIdempotencyKey(key.clone()));
            }
        }

        let product = self
            .products
            .get(&req.product_id)
            .ok_or_else(|| InvestmentError::ProductNotFound(req.product_id.clone()))?;

        if product.status == InvestmentStatus::Frozen {
            return Err(InvestmentError::ProductFrozen);
        }
        if product.status == InvestmentStatus::Closed {
            return Err(InvestmentError::ProductClosed);
        }

        // Self-investment prevention
        if product.target_id == req.investor_id {
            return Err(InvestmentError::SelfInvestment);
        }

        // Check total issue limit
        let new_issued = product.issued_shares + req.shares;
        let total_bps = (new_issued as u128 * BPS_DENOMINATOR as u128
            / product.total_shares as u128) as u64;
        if total_bps > self.config.max_total_share_bps {
            return Err(InvestmentError::TotalShareLimit(
                total_bps,
                self.config.max_total_share_bps,
            ));
        }

        // Check investor share limit
        let current_shares = self
            .position_index
            .get(&(req.product_id.clone(), req.investor_id.clone()))
            .and_then(|pid| self.positions.get(pid))
            .map(|p| p.shares)
            .unwrap_or(0);
        let new_investor_shares = current_shares + req.shares;
        let investor_bps = (new_investor_shares as u128 * BPS_DENOMINATOR as u128
            / product.total_shares as u128) as u64;
        if investor_bps > self.config.max_investor_share_bps {
            return Err(InvestmentError::InvestorShareLimit(
                investor_bps,
                self.config.max_investor_share_bps,
            ));
        }

        // Clone values needed after mutation
        let price_per_share = product.price_per_share;
        let total_amount = price_per_share.saturating_mul(req.shares);
        let product_id = req.product_id.clone();
        let investor_id = req.investor_id.clone();

        // Update issued shares
        self.products.get_mut(&product_id).unwrap().issued_shares += req.shares;

        // Update or create position
        let position = if let Some(pos_id) = self
            .position_index
            .get(&(product_id.clone(), investor_id.clone()))
            .cloned()
        {
            let pos = self.positions.get_mut(&pos_id).unwrap();
            let new_cost = pos.cost_basis + total_amount;
            pos.shares += req.shares;
            pos.cost_basis = new_cost;
            pos.avg_buy_price = new_cost / pos.shares;
            pos.clone()
        } else {
            let pos_id = Uuid::new_v4().to_string();
            let pos = InvestmentPosition {
                id: pos_id.clone(),
                product_id: product_id.clone(),
                investor_id: investor_id.clone(),
                shares: req.shares,
                avg_buy_price: price_per_share,
                cost_basis: total_amount,
                status: PositionStatus::Active,
                created_tick: self.current_tick,
            };
            self.position_index
                .insert((product_id.clone(), investor_id.clone()), pos_id.clone());
            self.positions.insert(pos_id, pos.clone());
            pos
        };

        // Record transaction
        let tx_id = Uuid::new_v4().to_string();
        let tx = InvestmentTransaction {
            id: tx_id,
            product_id: product_id.clone(),
            investor_id: investor_id.clone(),
            tx_type: InvestmentTxType::Buy,
            shares: req.shares,
            price_per_share,
            total_amount,
            idempotency_key: req.idempotency_key.clone(),
            tick: self.current_tick,
        };

        if let Some(key) = req.idempotency_key {
            self.idempotency_keys.insert(key, tx.id.clone());
        }
        self.transactions.push(tx.clone());

        self.emit(WorldEvent::InvestmentPurchased {
            product_id,
            investor_id,
            shares: req.shares,
            total_amount,
        });

        Ok((position, tx))
    }

    /// Sell shares in an investment product.
    pub fn sell_shares(
        &mut self,
        req: SellSharesRequest,
    ) -> Result<(InvestmentPosition, InvestmentTransaction), InvestmentError> {
        if req.shares == 0 {
            return Err(InvestmentError::InvalidShareCount);
        }

        // Check idempotency
        if let Some(ref key) = req.idempotency_key {
            if self.idempotency_keys.contains_key(key) {
                return Err(InvestmentError::DuplicateIdempotencyKey(key.clone()));
            }
        }

        let product = self
            .products
            .get(&req.product_id)
            .ok_or_else(|| InvestmentError::ProductNotFound(req.product_id.clone()))?;

        if product.status == InvestmentStatus::Frozen {
            return Err(InvestmentError::ProductFrozen);
        }
        if product.status == InvestmentStatus::Closed {
            return Err(InvestmentError::ProductClosed);
        }

        let pos_id = self
            .position_index
            .get(&(req.product_id.clone(), req.investor_id.clone()))
            .ok_or(InvestmentError::NoPosition)?
            .clone();

        let position = self
            .positions
            .get(&pos_id)
            .ok_or(InvestmentError::NoPosition)?;

        if position.shares < req.shares {
            return Err(InvestmentError::InsufficientShares(
                position.shares,
                req.shares,
            ));
        }

        // Clone values needed after mutation
        let price_per_share = product.price_per_share;
        let total_amount = price_per_share.saturating_mul(req.shares);
        let product_id = req.product_id.clone();
        let investor_id = req.investor_id.clone();

        // Update issued shares
        self.products
            .get_mut(&product_id)
            .unwrap()
            .issued_shares -= req.shares;

        // Update position
        let position = self.positions.get_mut(&pos_id).unwrap();
        position.shares -= req.shares;
        if position.shares == 0 {
            position.status = PositionStatus::Closed;
        }
        let updated_pos = position.clone();

        // Record transaction
        let tx_id = Uuid::new_v4().to_string();
        let tx = InvestmentTransaction {
            id: tx_id,
            product_id: product_id.clone(),
            investor_id: investor_id.clone(),
            tx_type: InvestmentTxType::Sell,
            shares: req.shares,
            price_per_share,
            total_amount,
            idempotency_key: req.idempotency_key.clone(),
            tick: self.current_tick,
        };

        if let Some(key) = req.idempotency_key {
            self.idempotency_keys.insert(key, tx.id.clone());
        }
        self.transactions.push(tx.clone());

        self.emit(WorldEvent::InvestmentSold {
            product_id,
            investor_id,
            shares: req.shares,
            total_amount,
        });

        Ok((updated_pos, tx))
    }

    // ── Close Investment (P0: Authorization Required) ──────

    /// Close an investment product. **Only the product owner** can close it.
    /// This settles all positions and returns funds to investors.
    pub fn close_investment(
        &mut self,
        req: CloseInvestmentRequest,
    ) -> Result<InvestmentProduct, InvestmentError> {
        let product = self
            .products
            .get(&req.product_id)
            .ok_or_else(|| InvestmentError::ProductNotFound(req.product_id.clone()))?;

        // P0 FIX: Authorization check — only the product owner can close
        if product.owner_id != req.requester_id {
            return Err(InvestmentError::Unauthorized(
                "only the product owner can close this investment".to_string(),
            ));
        }

        if product.status == InvestmentStatus::Closed {
            return Err(InvestmentError::ProductClosed);
        }

        // Close all active positions
        let product_id = req.product_id.clone();
        let positions_to_close: Vec<String> = self
            .positions
            .values()
            .filter(|p| p.product_id == product_id && p.status == PositionStatus::Active)
            .map(|p| p.id.clone())
            .collect();

        for pos_id in positions_to_close {
            if let Some(pos) = self.positions.get_mut(&pos_id) {
                pos.status = PositionStatus::Closed;
            }
        }

        // Mark product as closed
        let product = self.products.get_mut(&product_id).unwrap();
        product.status = InvestmentStatus::Closed;
        Ok(product.clone())
    }

    // ── Distribute Returns (P0: Permission Required) ──────

    /// Distribute returns/profits to shareholders.
    /// **Only the product owner** can trigger distribution.
    pub fn distribute_returns(
        &mut self,
        req: DistributeReturnsRequest,
    ) -> Result<DividendDistribution, InvestmentError> {
        let product = self
            .products
            .get(&req.product_id)
            .ok_or_else(|| InvestmentError::ProductNotFound(req.product_id.clone()))?;

        // P0 FIX: Permission check — only the product owner can distribute
        if product.owner_id != req.distributor_id {
            return Err(InvestmentError::Unauthorized(
                "only the product owner can distribute returns".to_string(),
            ));
        }

        if product.status != InvestmentStatus::Active {
            return Err(InvestmentError::ProductNotActive);
        }

        if req.profit == 0 {
            return Err(InvestmentError::NoProfitToDistribute);
        }

        // Collect shareholder info
        let product_id = req.product_id.clone();
        let holdings: Vec<(String, u64)> = self
            .positions
            .values()
            .filter(|p| p.product_id == product_id && p.status == PositionStatus::Active)
            .map(|p| (p.investor_id.clone(), p.shares))
            .collect();

        if holdings.is_empty() {
            return Err(InvestmentError::NoShareholders);
        }

        let total_issued = product.issued_shares;
        if total_issued == 0 {
            return Err(InvestmentError::NoShareholders);
        }

        // Calculate dividend per share using u128 for overflow protection
        let dividend_per_share = (req.profit as u128 / total_issued as u128) as u64;

        let mut recipients = Vec::new();
        for (investor_id, shares) in &holdings {
            let amount = dividend_per_share.saturating_mul(*shares);
            if amount > 0 {
                recipients.push(DividendRecipient {
                    investor_id: investor_id.clone(),
                    shares: *shares,
                    amount,
                });
            }
        }

        let distribution = DividendDistribution {
            id: Uuid::new_v4().to_string(),
            product_id: product_id.clone(),
            target_id: product.target_id.clone(),
            total_profit: req.profit,
            dividend_per_share,
            recipients: recipients.clone(),
            distributor_id: req.distributor_id,
            tick: self.current_tick,
        };

        self.dividends.push(distribution.clone());

        self.emit(WorldEvent::InvestmentDividend {
            dividend_id: distribution.id.clone(),
            product_id,
            target_id: product.target_id.clone(),
            total_profit: req.profit,
            recipient_count: recipients.len(),
        });

        Ok(distribution)
    }

    // ── Query Methods ─────────────────────────────────────

    pub fn get_portfolio(&self, investor_id: &str) -> Vec<PortfolioEntry> {
        self.positions
            .values()
            .filter(|p| p.investor_id == investor_id)
            .filter_map(|pos| {
                let product = self.products.get(&pos.product_id)?;
                let current_value = product.price_per_share.saturating_mul(pos.shares);
                let pnl = current_value as i64 - pos.cost_basis as i64;
                Some(PortfolioEntry {
                    product_id: pos.product_id.clone(),
                    target_name: product.target_name.clone(),
                    shares: pos.shares,
                    avg_buy_price: pos.avg_buy_price,
                    cost_basis: pos.cost_basis,
                    current_value,
                    pnl,
                    status: pos.status,
                })
            })
            .filter(|e| e.status == PositionStatus::Active)
            .collect()
    }

    pub fn get_leaderboard(&self) -> Vec<LeaderboardEntry> {
        let mut entries: HashMap<String, u64> = HashMap::new();
        let mut counts: HashMap<String, usize> = HashMap::new();

        for pos in self.positions.values() {
            if pos.status != PositionStatus::Active {
                continue;
            }
            if let Some(product) = self.products.get(&pos.product_id) {
                let value = product.price_per_share.saturating_mul(pos.shares);
                *entries.entry(pos.investor_id.clone()).or_default() += value;
                *counts.entry(pos.investor_id.clone()).or_default() += 1;
            }
        }

        let mut result: Vec<LeaderboardEntry> = entries
            .into_iter()
            .map(|(investor_id, total_value)| LeaderboardEntry {
                position_count: counts.get(&investor_id).copied().unwrap_or(0),
                investor_id,
                total_value,
            })
            .collect();
        result.sort_by(|a, b| b.total_value.cmp(&a.total_value));
        result
    }

    pub fn list_transactions(&self, query: &ListTransactionsQuery) -> Vec<&InvestmentTransaction> {
        self.transactions
            .iter()
            .filter(|t| {
                query
                    .product_id
                    .as_ref()
                    .is_none_or(|pid| t.product_id == *pid)
            })
            .filter(|t| {
                query
                    .investor_id
                    .as_ref()
                    .is_none_or(|iid| t.investor_id == *iid)
            })
            .collect()
    }

    pub fn list_dividends(&self, product_id: Option<&str>) -> Vec<&DividendDistribution> {
        self.dividends
            .iter()
            .filter(|d| product_id.is_none_or(|pid| d.product_id == *pid))
            .collect()
    }

    // ── Currency Conversion ───────────────────────────────

    pub fn virtual_to_tokens(&self, amount: u64) -> u64 {
        amount.saturating_mul(self.config.virtual_to_token_rate)
    }

    pub fn tokens_to_virtual(&self, tokens: u64) -> u64 {
        tokens / self.config.virtual_to_token_rate
    }
}

// ── Tests ─────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_system() -> InvestmentSystem {
        InvestmentSystem::new()
    }

    fn make_product(system: &mut InvestmentSystem) -> InvestmentProduct {
        system.create_product(CreateProductRequest {
            target_id: "agent-1".to_string(),
            target_name: "Agent One".to_string(),
            entity_type: InvestmentEntityType::Agent,
            total_shares: 10_000,
            price_per_share: 100,
            owner_id: "owner-1".to_string(),
        }).unwrap()
    }

    #[test]
    fn create_product_success() {
        let mut sys = make_system();
        let p = make_product(&mut sys);
        assert_eq!(p.target_id, "agent-1");
        assert_eq!(p.total_shares, 10_000);
        assert_eq!(p.status, InvestmentStatus::Active);
        assert_eq!(p.owner_id, "owner-1");
    }

    #[test]
    fn create_product_rejects_zero_shares() {
        let mut sys = make_system();
        let res = sys.create_product(CreateProductRequest {
            target_id: "agent-1".to_string(),
            target_name: "A".to_string(),
            entity_type: InvestmentEntityType::Agent,
            total_shares: 0,
            price_per_share: 100,
            owner_id: "owner-1".to_string(),
        });
        assert!(res.is_err());
    }

    #[test]
    fn create_product_rejects_zero_price() {
        let mut sys = make_system();
        let res = sys.create_product(CreateProductRequest {
            target_id: "agent-1".to_string(),
            target_name: "A".to_string(),
            entity_type: InvestmentEntityType::Agent,
            total_shares: 1000,
            price_per_share: 0,
            owner_id: "owner-1".to_string(),
        });
        assert!(res.is_err());
    }

    #[test]
    fn create_product_rejects_duplicate_target() {
        let mut sys = make_system();
        make_product(&mut sys);
        let res = sys.create_product(CreateProductRequest {
            target_id: "agent-1".to_string(),
            target_name: "A".to_string(),
            entity_type: InvestmentEntityType::Agent,
            total_shares: 1000,
            price_per_share: 100,
            owner_id: "owner-1".to_string(),
        });
        assert!(res.is_err());
    }

    #[test]
    fn buy_shares_success() {
        let mut sys = make_system();
        let p = make_product(&mut sys);
        let (pos, tx) = sys.buy_shares(BuySharesRequest {
            product_id: p.id.clone(),
            investor_id: "investor-1".to_string(),
            shares: 100,
            idempotency_key: None,
        }).unwrap();
        assert_eq!(pos.shares, 100);
        assert_eq!(tx.total_amount, 10_000);
    }

    #[test]
    fn buy_shares_prevents_self_investment() {
        let mut sys = make_system();
        let p = make_product(&mut sys);
        let res = sys.buy_shares(BuySharesRequest {
            product_id: p.id.clone(),
            investor_id: "agent-1".to_string(), // same as target_id
            shares: 100,
            idempotency_key: None,
        });
        assert!(res.is_err());
    }

    #[test]
    fn buy_shares_enforces_investor_limit() {
        let mut sys = make_system();
        let p = make_product(&mut sys);
        // 30% of 10_000 = 3_000 max per investor
        let res = sys.buy_shares(BuySharesRequest {
            product_id: p.id.clone(),
            investor_id: "investor-1".to_string(),
            shares: 4_000, // 40% > 30%
            idempotency_key: None,
        });
        assert!(res.is_err());
    }

    #[test]
    fn buy_shares_idempotency() {
        let mut sys = make_system();
        let p = make_product(&mut sys);
        let req1 = BuySharesRequest {
            product_id: p.id.clone(),
            investor_id: "investor-1".to_string(),
            shares: 100,
            idempotency_key: Some("key-1".to_string()),
        };
        sys.buy_shares(req1).unwrap();
        let req2 = BuySharesRequest {
            product_id: p.id.clone(),
            investor_id: "investor-1".to_string(),
            shares: 100,
            idempotency_key: Some("key-1".to_string()),
        };
        let res = sys.buy_shares(req2);
        assert!(res.is_err());
    }

    #[test]
    fn sell_shares_success() {
        let mut sys = make_system();
        let p = make_product(&mut sys);
        sys.buy_shares(BuySharesRequest {
            product_id: p.id.clone(),
            investor_id: "investor-1".to_string(),
            shares: 100,
            idempotency_key: None,
        }).unwrap();

        let (pos, _tx) = sys.sell_shares(SellSharesRequest {
            product_id: p.id.clone(),
            investor_id: "investor-1".to_string(),
            shares: 50,
            idempotency_key: None,
        }).unwrap();
        assert_eq!(pos.shares, 50);
        assert_eq!(pos.status, PositionStatus::Active);
    }

    #[test]
    fn sell_shares_insufficient() {
        let mut sys = make_system();
        let p = make_product(&mut sys);
        sys.buy_shares(BuySharesRequest {
            product_id: p.id.clone(),
            investor_id: "investor-1".to_string(),
            shares: 50,
            idempotency_key: None,
        }).unwrap();

        let res = sys.sell_shares(SellSharesRequest {
            product_id: p.id.clone(),
            investor_id: "investor-1".to_string(),
            shares: 100,
            idempotency_key: None,
        });
        assert!(res.is_err());
    }

    #[test]
    fn sell_all_shares_closes_position() {
        let mut sys = make_system();
        let p = make_product(&mut sys);
        sys.buy_shares(BuySharesRequest {
            product_id: p.id.clone(),
            investor_id: "investor-1".to_string(),
            shares: 100,
            idempotency_key: None,
        }).unwrap();

        let (pos, _) = sys.sell_shares(SellSharesRequest {
            product_id: p.id.clone(),
            investor_id: "investor-1".to_string(),
            shares: 100,
            idempotency_key: None,
        }).unwrap();
        assert_eq!(pos.status, PositionStatus::Closed);
    }

    #[test]
    fn close_investment_authorization_required() {
        let mut sys = make_system();
        let p = make_product(&mut sys);
        sys.buy_shares(BuySharesRequest {
            product_id: p.id.clone(),
            investor_id: "investor-1".to_string(),
            shares: 100,
            idempotency_key: None,
        }).unwrap();

        // Unauthorized: wrong requester
        let res = sys.close_investment(CloseInvestmentRequest {
            product_id: p.id.clone(),
            requester_id: "attacker".to_string(),
        });
        assert!(res.is_err());

        // Authorized: correct owner
        let res = sys.close_investment(CloseInvestmentRequest {
            product_id: p.id.clone(),
            requester_id: "owner-1".to_string(),
        });
        assert!(res.is_ok());
        let closed = res.unwrap();
        assert_eq!(closed.status, InvestmentStatus::Closed);
    }

    #[test]
    fn distribute_returns_authorization_required() {
        let mut sys = make_system();
        let p = make_product(&mut sys);
        sys.buy_shares(BuySharesRequest {
            product_id: p.id.clone(),
            investor_id: "investor-1".to_string(),
            shares: 100,
            idempotency_key: None,
        }).unwrap();

        // Unauthorized: wrong distributor
        let res = sys.distribute_returns(DistributeReturnsRequest {
            product_id: p.id.clone(),
            profit: 1000,
            distributor_id: "attacker".to_string(),
        });
        assert!(res.is_err());

        // Authorized: correct owner
        let res = sys.distribute_returns(DistributeReturnsRequest {
            product_id: p.id.clone(),
            profit: 1000,
            distributor_id: "owner-1".to_string(),
        });
        assert!(res.is_ok());
        let dist = res.unwrap();
        assert_eq!(dist.total_profit, 1000);
        assert_eq!(dist.recipients.len(), 1);
    }

    #[test]
    fn distribute_returns_rejects_zero_profit() {
        let mut sys = make_system();
        let p = make_product(&mut sys);
        let res = sys.distribute_returns(DistributeReturnsRequest {
            product_id: p.id.clone(),
            profit: 0,
            distributor_id: "owner-1".to_string(),
        });
        assert!(res.is_err());
    }

    #[test]
    fn freeze_product_authorization_required() {
        let mut sys = make_system();
        let p = make_product(&mut sys);

        // Unauthorized
        let res = sys.freeze_product(FreezeProductRequest {
            product_id: p.id.clone(),
            requester_id: "attacker".to_string(),
        });
        assert!(res.is_err());

        // Authorized
        let res = sys.freeze_product(FreezeProductRequest {
            product_id: p.id.clone(),
            requester_id: "owner-1".to_string(),
        });
        assert!(res.is_ok());
        assert_eq!(res.unwrap().status, InvestmentStatus::Frozen);
    }

    #[test]
    fn cannot_buy_frozen_product() {
        let mut sys = make_system();
        let p = make_product(&mut sys);
        sys.freeze_product(FreezeProductRequest {
            product_id: p.id.clone(),
            requester_id: "owner-1".to_string(),
        }).unwrap();

        let res = sys.buy_shares(BuySharesRequest {
            product_id: p.id.clone(),
            investor_id: "investor-1".to_string(),
            shares: 100,
            idempotency_key: None,
        });
        assert!(res.is_err());
    }

    #[test]
    fn update_performance_adjusts_price() {
        let mut sys = make_system();
        let p = make_product(&mut sys);
        let updated = sys.update_performance(&p.id, 8.0).unwrap();
        assert_eq!(updated.performance_score, 8.0);
        // Price should have increased
        assert!(updated.price_per_share >= p.price_per_share);
    }

    #[test]
    fn portfolio_excludes_closed() {
        let mut sys = make_system();
        let p = make_product(&mut sys);
        sys.buy_shares(BuySharesRequest {
            product_id: p.id.clone(),
            investor_id: "investor-1".to_string(),
            shares: 100,
            idempotency_key: None,
        }).unwrap();
        sys.sell_shares(SellSharesRequest {
            product_id: p.id.clone(),
            investor_id: "investor-1".to_string(),
            shares: 100,
            idempotency_key: None,
        }).unwrap();

        let portfolio = sys.get_portfolio("investor-1");
        assert!(portfolio.is_empty());
    }

    #[test]
    fn leaderboard_sorted_by_value() {
        let mut sys = make_system();
        let p = make_product(&mut sys);
        sys.buy_shares(BuySharesRequest {
            product_id: p.id.clone(),
            investor_id: "investor-1".to_string(),
            shares: 500,
            idempotency_key: None,
        }).unwrap();
        sys.buy_shares(BuySharesRequest {
            product_id: p.id.clone(),
            investor_id: "investor-2".to_string(),
            shares: 100,
            idempotency_key: None,
        }).unwrap();

        let lb = sys.get_leaderboard();
        assert_eq!(lb[0].investor_id, "investor-1");
        assert_eq!(lb[1].investor_id, "investor-2");
    }

    #[test]
    fn transaction_history_filtering() {
        let mut sys = make_system();
        let p = make_product(&mut sys);
        sys.buy_shares(BuySharesRequest {
            product_id: p.id.clone(),
            investor_id: "investor-1".to_string(),
            shares: 100,
            idempotency_key: None,
        }).unwrap();
        sys.sell_shares(SellSharesRequest {
            product_id: p.id.clone(),
            investor_id: "investor-1".to_string(),
            shares: 50,
            idempotency_key: None,
        }).unwrap();

        let all = sys.list_transactions(&ListTransactionsQuery::default());
        assert_eq!(all.len(), 2);

        let buys = sys.list_transactions(&ListTransactionsQuery {
            product_id: Some(p.id.clone()),
            investor_id: None,
        });
        assert_eq!(buys.len(), 2);
    }

    #[test]
    fn currency_conversion() {
        let sys = make_system();
        assert_eq!(sys.virtual_to_tokens(1), 100);
        assert_eq!(sys.tokens_to_virtual(200), 2);
    }

    #[test]
    fn avg_buy_price_calculation() {
        let mut sys = make_system();
        let p = make_product(&mut sys);
        sys.buy_shares(BuySharesRequest {
            product_id: p.id.clone(),
            investor_id: "investor-1".to_string(),
            shares: 100,
            idempotency_key: None,
        }).unwrap();
        // Update price
        sys.update_performance(&p.id, 8.0).unwrap();
        let updated_price = sys.get_product(&p.id).unwrap().price_per_share;

        // Buy more at new price
        let (pos, _) = sys.buy_shares(BuySharesRequest {
            product_id: p.id.clone(),
            investor_id: "investor-1".to_string(),
            shares: 100,
            idempotency_key: None,
        }).unwrap();
        // avg should be between original and new price
        assert!(pos.avg_buy_price > 100);
        assert!(pos.avg_buy_price < updated_price + 100);
    }

    #[test]
    fn close_investment_closes_all_positions() {
        let mut sys = make_system();
        let p = make_product(&mut sys);
        sys.buy_shares(BuySharesRequest {
            product_id: p.id.clone(),
            investor_id: "investor-1".to_string(),
            shares: 100,
            idempotency_key: None,
        }).unwrap();
        sys.buy_shares(BuySharesRequest {
            product_id: p.id.clone(),
            investor_id: "investor-2".to_string(),
            shares: 200,
            idempotency_key: None,
        }).unwrap();

        sys.close_investment(CloseInvestmentRequest {
            product_id: p.id.clone(),
            requester_id: "owner-1".to_string(),
        }).unwrap();

        let portfolio1 = sys.get_portfolio("investor-1");
        let portfolio2 = sys.get_portfolio("investor-2");
        assert!(portfolio1.is_empty());
        assert!(portfolio2.is_empty());
    }
}
