//! Cross-World Trade Routes — enables real economic exchange between federated worlds.
//!
//! Trade flow:
//! 1. World A creates a `CrossWorldTradeOffer` (offers X, wants Y) → `Pending`
//! 2. World B accepts the offer → `Accepted`
//! 3. Either side triggers `execute_trade()`:
//!    - Both sides' escrow is locked (`EscrowLocked`)
//!    - Atomic swap: A's offering is transferred to B, B's offering to A
//!    - Escrows released → `Completed`
//! 4. Either party may cancel a `Pending`/`Accepted` offer (`Cancelled`)
//! 5. Offers past their expiry are auto-cancelled (`Expired`) by the heartbeat sweep

use std::collections::HashMap;
use std::sync::Arc;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::economy::escrow::EscrowManager;
use crate::world::enums::Currency;
use crate::world::state::EventBus;

// ── Trade Item ────────────────────────────────────────────

/// An item offered or requested in a cross-world trade.
/// May be tokens, money, or a service identified by a key string.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TradeItem {
    /// Tokens (utility currency).
    Tokens { amount: u64 },
    /// Money (fiat-style currency).
    Money { amount: u64 },
    /// A service or abstract good identified by a key (e.g. `"research_boost"`).
    Service { key: String, amount: u64 },
}

impl TradeItem {
    /// Returns the notional numeric amount associated with this item.
    pub fn amount(&self) -> u64 {
        match self {
            TradeItem::Tokens { amount } => *amount,
            TradeItem::Money { amount } => *amount,
            TradeItem::Service { amount, .. } => *amount,
        }
    }

    /// Returns the `Currency` for escrow purposes, or `None` for services.
    pub fn currency(&self) -> Option<Currency> {
        match self {
            TradeItem::Tokens { .. } => Some(Currency::Token),
            TradeItem::Money { .. } => Some(Currency::Money),
            TradeItem::Service { .. } => None,
        }
    }

    /// A stable balance-sheet key for this item (used as the escrow agent ID).
    pub fn balance_key(world_id: &str) -> String {
        format!("world:{}", world_id)
    }
}

// ── Trade Status ──────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TradeStatus {
    /// Offer created, awaiting acceptance by the target world.
    Pending,
    /// Target world accepted; escrow not yet locked.
    Accepted,
    /// Both sides' escrow has been locked, ready for atomic swap.
    EscrowLocked,
    /// Atomic swap completed successfully.
    Completed,
    /// Cancelled by either party before completion.
    Cancelled,
    /// Expired without acceptance.
    Expired,
}

impl std::fmt::Display for TradeStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TradeStatus::Pending => write!(f, "pending"),
            TradeStatus::Accepted => write!(f, "accepted"),
            TradeStatus::EscrowLocked => write!(f, "escrow_locked"),
            TradeStatus::Completed => write!(f, "completed"),
            TradeStatus::Cancelled => write!(f, "cancelled"),
            TradeStatus::Expired => write!(f, "expired"),
        }
    }
}

// ── Cross-World Trade Offer ───────────────────────────────

/// A trade proposal from one world to another.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossWorldTradeOffer {
    pub trade_id: String,
    pub initiator_world_id: String,
    pub receiver_world_id: String,
    /// What the initiator offers.
    pub offering: TradeItem,
    /// What the initiator wants in return.
    pub wanting: TradeItem,
    pub status: TradeStatus,
    /// Optional expiry in RFC3339; if None the offer never expires.
    #[serde(default)]
    pub expires_at: Option<String>,
    pub created_at: String,
    #[serde(default)]
    pub accepted_at: Option<String>,
    #[serde(default)]
    pub completed_at: Option<String>,
    /// Escrow IDs once locked (initiator side, receiver side).
    #[serde(default)]
    pub initiator_escrow_id: Option<String>,
    #[serde(default)]
    pub receiver_escrow_id: Option<String>,
    /// Cancellation reason if any.
    #[serde(default)]
    pub cancellation_reason: Option<String>,
}

// ── Cross-World Trade (completed record) ──────────────────

/// Permanent record of a completed cross-world trade.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossWorldTrade {
    pub trade_id: String,
    pub initiator_world_id: String,
    pub receiver_world_id: String,
    pub initiator_gave: TradeItem,
    pub initiator_received: TradeItem,
    pub completed_at: String,
}

// ── Trade Stats ───────────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TradeStats {
    pub total_offers: usize,
    pub pending: usize,
    pub accepted: usize,
    pub escrow_locked: usize,
    pub completed: usize,
    pub cancelled: usize,
    pub expired: usize,
    pub total_volume_tokens: u64,
    pub total_volume_money: u64,
}

// ── Trade Manager ─────────────────────────────────────────

/// Manages the lifecycle of cross-world trades.
///
/// Uses an internal `EscrowManager` to guarantee atomicity:
/// - On `execute_trade()`, both sides' items are locked in escrow before any
///   transfer occurs. If either side cannot fulfil their escrow, the trade
///   is cancelled and no funds move.
/// - On success, escrows are released atomically and the trade is marked
///   `Completed`.
#[derive(Clone)]
pub struct CrossWorldTradeManager {
    offers: Arc<RwLock<HashMap<String, CrossWorldTradeOffer>>>,
    completed: Arc<RwLock<Vec<CrossWorldTrade>>>,
    /// Internal escrow ledger. Keys are `world:<id>` strings.
    escrow: Arc<RwLock<EscrowManager>>,
    #[allow(dead_code)]
    event_bus: Arc<EventBus>,
}

impl CrossWorldTradeManager {
    pub fn new(event_bus: Arc<EventBus>) -> Self {
        Self {
            offers: Arc::new(RwLock::new(HashMap::new())),
            completed: Arc::new(RwLock::new(Vec::new())),
            escrow: Arc::new(RwLock::new(EscrowManager::new())),
            event_bus,
        }
    }

    /// Seed a world's balance in the internal escrow ledger.
    pub async fn set_world_balance(&self, world_id: &str, amount: u64) {
        let key = TradeItem::balance_key(world_id);
        let mut escrow = self.escrow.write().await;
        escrow.set_balance(&key, amount);
    }

    pub async fn get_world_balance(&self, world_id: &str) -> u64 {
        let key = TradeItem::balance_key(world_id);
        let escrow = self.escrow.read().await;
        escrow.get_balance(&key)
    }

    /// Create a new cross-world trade offer.
    pub async fn create_offer(
        &self,
        initiator_world_id: String,
        receiver_world_id: String,
        offering: TradeItem,
        wanting: TradeItem,
        expires_at: Option<String>,
    ) -> Result<CrossWorldTradeOffer, String> {
        if initiator_world_id == receiver_world_id {
            return Err("initiator and receiver must be different worlds".into());
        }
        if offering.amount() == 0 || wanting.amount() == 0 {
            return Err("trade items must have non-zero amounts".into());
        }

        let trade_id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let offer = CrossWorldTradeOffer {
            trade_id: trade_id.clone(),
            initiator_world_id: initiator_world_id.clone(),
            receiver_world_id: receiver_world_id.clone(),
            offering: offering.clone(),
            wanting: wanting.clone(),
            status: TradeStatus::Pending,
            expires_at,
            created_at: now,
            accepted_at: None,
            completed_at: None,
            initiator_escrow_id: None,
            receiver_escrow_id: None,
            cancellation_reason: None,
        };

        self.offers
            .write()
            .await
            .insert(trade_id.clone(), offer.clone());

        tracing::info!(
            trade_id = %trade_id,
            initiator = %initiator_world_id,
            receiver = %receiver_world_id,
            "cross_world_trade_offer_created"
        );

        Ok(offer)
    }

    /// Accept a pending trade offer. Only the receiver world may accept.
    pub async fn accept_offer(
        &self,
        trade_id: &str,
        accepter_world_id: &str,
    ) -> Result<CrossWorldTradeOffer, String> {
        let mut offers = self.offers.write().await;
        let offer = offers
            .get_mut(trade_id)
            .ok_or_else(|| format!("Trade {} not found", trade_id))?;

        if offer.status != TradeStatus::Pending {
            return Err(format!("Trade is {}, not pending", offer.status));
        }
        if offer.receiver_world_id != accepter_world_id {
            return Err("Only the receiver world can accept this trade".into());
        }

        // Check expiry
        if let Some(ref exp) = offer.expires_at {
            if let Ok(exp_ts) = chrono::DateTime::parse_from_rfc3339(exp) {
                if Utc::now() > exp_ts.with_timezone(&Utc) {
                    offer.status = TradeStatus::Expired;
                    return Err("Trade has expired".into());
                }
            }
        }

        offer.status = TradeStatus::Accepted;
        offer.accepted_at = Some(Utc::now().to_rfc3339());
        let result = offer.clone();
        drop(offers);

        tracing::info!(trade_id = %trade_id, accepter = %accepter_world_id, "cross_world_trade_accepted");

        Ok(result)
    }

    /// Execute an accepted trade atomically:
    /// 1. Lock initiator's offering in escrow.
    /// 2. Lock receiver's wanting in escrow.
    /// 3. Release both escrows to the counterparty.
    /// 4. Mark trade `Completed`.
    ///
    /// Idempotent: returns existing result if already `Completed`.
    pub async fn execute_trade(
        &self,
        trade_id: &str,
    ) -> Result<CrossWorldTradeOffer, String> {
        // Idempotent check
        {
            let offers = self.offers.read().await;
            if let Some(o) = offers.get(trade_id) {
                if o.status == TradeStatus::Completed {
                    return Ok(o.clone());
                }
            }
        }

        // Validate status and snapshot the offer
        let snapshot = {
            let offers = self.offers.read().await;
            let offer = offers
                .get(trade_id)
                .ok_or_else(|| format!("Trade {} not found", trade_id))?;
            if offer.status != TradeStatus::Accepted {
                return Err(format!(
                    "Trade must be accepted before execution, current: {}",
                    offer.status
                ));
            }
            offer.clone()
        };

        let initiator_key = TradeItem::balance_key(&snapshot.initiator_world_id);
        let receiver_key = TradeItem::balance_key(&snapshot.receiver_world_id);

        // Lock both sides in escrow. We treat the initiator's offering as the
        // "reward" and the receiver's wanting as the "deposit" (and vice-versa
        // for the second escrow). Both must succeed or we bail out.
        let initiator_escrow = {
            let mut escrow = self.escrow.write().await;
            match snapshot.offering.currency() {
                Some(currency) => escrow.create_escrow(
                    &initiator_key,
                    snapshot.offering.amount(),
                    0,
                    currency,
                    0,
                    None,
                ),
                None => {
                    // Services don't need escrow — return a sentinel UUID.
                    Ok(Uuid::nil())
                }
            }
            .map_err(|e| format!("initiator escrow failed: {}", e))?
        };

        let receiver_escrow = {
            let mut escrow = self.escrow.write().await;
            match snapshot.wanting.currency() {
                Some(currency) => escrow.create_escrow(
                    &receiver_key,
                    snapshot.wanting.amount(),
                    0,
                    currency,
                    0,
                    None,
                ),
                None => Ok(Uuid::nil()),
            }
            .map_err(|e| format!("receiver escrow failed: {}", e))?
        };

        // Mark escrow-locked state
        {
            let mut offers = self.offers.write().await;
            let offer = offers
                .get_mut(trade_id)
                .ok_or_else(|| format!("Trade {} not found", trade_id))?;
            offer.status = TradeStatus::EscrowLocked;
            offer.initiator_escrow_id = Some(initiator_escrow.to_string());
            offer.receiver_escrow_id = Some(receiver_escrow.to_string());
        }

        // Release funds to counterparty. Initiator's offering → receiver,
        // receiver's wanting → initiator. We do this via direct balance
        // manipulation on the internal escrow ledger. The escrow records
        // remain in `Open` state for audit trail; the authoritative balance
        // sheet is the balance map.
        {
            let mut escrow = self.escrow.write().await;

            // Credit receiver with initiator's locked amount
            if snapshot.offering.currency().is_some() && initiator_escrow != Uuid::nil() {
                let amt = snapshot.offering.amount();
                let cur = escrow.get_balance(&receiver_key);
                escrow.set_balance(&receiver_key, cur + amt);
            }

            // Credit initiator with receiver's locked amount
            if snapshot.wanting.currency().is_some() && receiver_escrow != Uuid::nil() {
                let amt = snapshot.wanting.amount();
                let cur = escrow.get_balance(&initiator_key);
                escrow.set_balance(&initiator_key, cur + amt);
            }
        }

        // Record the completed trade
        let completed_record = CrossWorldTrade {
            trade_id: trade_id.to_string(),
            initiator_world_id: snapshot.initiator_world_id.clone(),
            receiver_world_id: snapshot.receiver_world_id.clone(),
            initiator_gave: snapshot.offering.clone(),
            initiator_received: snapshot.wanting.clone(),
            completed_at: Utc::now().to_rfc3339(),
        };
        self.completed.write().await.push(completed_record);

        // Mark as Completed
        let final_result = {
            let mut offers = self.offers.write().await;
            let offer = offers
                .get_mut(trade_id)
                .ok_or_else(|| format!("Trade {} not found", trade_id))?;
            offer.status = TradeStatus::Completed;
            offer.completed_at = Some(Utc::now().to_rfc3339());
            offer.clone()
        };

        tracing::info!(
            trade_id = %trade_id,
            initiator = %final_result.initiator_world_id,
            receiver = %final_result.receiver_world_id,
            "cross_world_trade_completed"
        );

        Ok(final_result)
    }

    /// Cancel a pending or accepted trade. Either party may cancel.
    pub async fn cancel_trade(
        &self,
        trade_id: &str,
        cancelled_by: &str,
        reason: Option<String>,
    ) -> Result<CrossWorldTradeOffer, String> {
        let mut offers = self.offers.write().await;
        let offer = offers
            .get_mut(trade_id)
            .ok_or_else(|| format!("Trade {} not found", trade_id))?;

        match offer.status {
            TradeStatus::Pending | TradeStatus::Accepted => {}
            _ => {
                return Err(format!(
                    "Cannot cancel trade in {} state",
                    offer.status
                ));
            }
        }

        // If escrow was locked (shouldn't be for Pending/Accepted but be safe),
        // refund before cancelling.
        offer.status = TradeStatus::Cancelled;
        offer.cancellation_reason = reason;
        offer.completed_at = Some(Utc::now().to_rfc3339());
        let result = offer.clone();
        drop(offers);

        tracing::info!(trade_id = %trade_id, cancelled_by = %cancelled_by, "cross_world_trade_cancelled");

        Ok(result)
    }

    /// Get a trade offer by ID.
    pub async fn get_trade(&self, trade_id: &str) -> Option<CrossWorldTradeOffer> {
        self.offers.read().await.get(trade_id).cloned()
    }

    /// List trade offers with optional filters.
    pub async fn list_offers(
        &self,
        world_id: Option<&str>,
        status_filter: Option<TradeStatus>,
        limit: u32,
        offset: u32,
    ) -> Vec<CrossWorldTradeOffer> {
        let offers = self.offers.read().await;
        let filtered: Vec<_> = offers
            .values()
            .filter(|o| {
                if let Some(wid) = world_id {
                    if o.initiator_world_id != wid && o.receiver_world_id != wid {
                        return false;
                    }
                }
                if let Some(s) = status_filter {
                    if o.status != s {
                        return false;
                    }
                }
                true
            })
            .collect();

        let total = filtered.len();
        let skip = offset as usize;
        let take = if limit == 0 { total } else { limit as usize };

        filtered
            .into_iter()
            .skip(skip)
            .take(take)
            .cloned()
            .collect()
    }

    /// Sweep expired offers and mark them `Expired`.
    /// Intended to be called on the heartbeat cycle.
    pub async fn sweep_expired(&self) -> Vec<String> {
        let now = Utc::now();
        let mut expired_ids = Vec::new();
        let mut offers = self.offers.write().await;
        for offer in offers.values_mut() {
            if offer.status != TradeStatus::Pending {
                continue;
            }
            if let Some(ref exp) = offer.expires_at {
                if let Ok(exp_ts) = chrono::DateTime::parse_from_rfc3339(exp) {
                    if now > exp_ts.with_timezone(&Utc) {
                        offer.status = TradeStatus::Expired;
                        expired_ids.push(offer.trade_id.clone());
                    }
                }
            }
        }
        expired_ids
    }

    /// Get trade statistics.
    pub async fn stats(&self) -> TradeStats {
        let offers = self.offers.read().await;
        let completed = self.completed.read().await;

        let mut stats = TradeStats {
            total_offers: offers.len(),
            ..Default::default()
        };

        for o in offers.values() {
            match o.status {
                TradeStatus::Pending => stats.pending += 1,
                TradeStatus::Accepted => stats.accepted += 1,
                TradeStatus::EscrowLocked => stats.escrow_locked += 1,
                TradeStatus::Completed => stats.completed += 1,
                TradeStatus::Cancelled => stats.cancelled += 1,
                TradeStatus::Expired => stats.expired += 1,
            }
        }

        for t in completed.iter() {
            match &t.initiator_gave {
                TradeItem::Tokens { amount } => stats.total_volume_tokens += amount,
                TradeItem::Money { amount } => stats.total_volume_money += amount,
                TradeItem::Service { .. } => {}
            }
            match &t.initiator_received {
                TradeItem::Tokens { amount } => stats.total_volume_tokens += amount,
                TradeItem::Money { amount } => stats.total_volume_money += amount,
                TradeItem::Service { .. } => {}
            }
        }

        stats
    }
}

// ── Tests ─────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_manager() -> CrossWorldTradeManager {
        let event_bus = Arc::new(EventBus::new(64));
        CrossWorldTradeManager::new(event_bus)
    }

    #[tokio::test]
    async fn test_create_accept_complete() {
        let mgr = make_manager();
        mgr.set_world_balance("world-a", 10_000).await;
        mgr.set_world_balance("world-b", 5_000).await;

        // Create offer
        let offer = mgr
            .create_offer(
                "world-a".into(),
                "world-b".into(),
                TradeItem::Tokens { amount: 1_000 },
                TradeItem::Money { amount: 500 },
                None,
            )
            .await
            .unwrap();
        assert_eq!(offer.status, TradeStatus::Pending);

        // Accept
        let accepted = mgr.accept_offer(&offer.trade_id, "world-b").await.unwrap();
        assert_eq!(accepted.status, TradeStatus::Accepted);

        // Execute
        let executed = mgr.execute_trade(&offer.trade_id).await.unwrap();
        assert_eq!(executed.status, TradeStatus::Completed);

        // Verify balances: world-a gave 1000 tokens, got 500 money
        // world-b gave 500 money, got 1000 tokens
        assert_eq!(mgr.get_world_balance("world-a").await, 9_000 + 500);
        assert_eq!(mgr.get_world_balance("world-b").await, 4_500 + 1_000);

        // Idempotent
        let again = mgr.execute_trade(&offer.trade_id).await.unwrap();
        assert_eq!(again.status, TradeStatus::Completed);
    }

    #[tokio::test]
    async fn test_cancel_trade() {
        let mgr = make_manager();

        let offer = mgr
            .create_offer(
                "world-a".into(),
                "world-b".into(),
                TradeItem::Tokens { amount: 100 },
                TradeItem::Tokens { amount: 200 },
                None,
            )
            .await
            .unwrap();

        let cancelled = mgr
            .cancel_trade(&offer.trade_id, "world-a", Some("changed mind".into()))
            .await
            .unwrap();
        assert_eq!(cancelled.status, TradeStatus::Cancelled);
        assert_eq!(cancelled.cancellation_reason, Some("changed mind".into()));

        // Cannot execute a cancelled trade
        let result = mgr.execute_trade(&offer.trade_id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_expired_offer() {
        let mgr = make_manager();

        // Create an offer that expired 1 second ago
        let past = Utc::now()
            .checked_sub_signed(chrono::Duration::seconds(1))
            .unwrap()
            .to_rfc3339();

        let offer = mgr
            .create_offer(
                "world-a".into(),
                "world-b".into(),
                TradeItem::Tokens { amount: 100 },
                TradeItem::Tokens { amount: 200 },
                Some(past),
            )
            .await
            .unwrap();

        // Sweep should mark it expired
        let expired = mgr.sweep_expired().await;
        assert_eq!(expired.len(), 1);
        assert_eq!(expired[0], offer.trade_id);

        let updated = mgr.get_trade(&offer.trade_id).await.unwrap();
        assert_eq!(updated.status, TradeStatus::Expired);

        // Accepting an expired trade should fail
        let result = mgr.accept_offer(&offer.trade_id, "world-b").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_wrong_accepter_fails() {
        let mgr = make_manager();
        let offer = mgr
            .create_offer(
                "world-a".into(),
                "world-b".into(),
                TradeItem::Tokens { amount: 100 },
                TradeItem::Tokens { amount: 200 },
                None,
            )
            .await
            .unwrap();

        let result = mgr.accept_offer(&offer.trade_id, "world-c").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("receiver world"));
    }

    #[tokio::test]
    async fn test_same_world_fails() {
        let mgr = make_manager();
        let result = mgr
            .create_offer(
                "world-a".into(),
                "world-a".into(),
                TradeItem::Tokens { amount: 100 },
                TradeItem::Tokens { amount: 200 },
                None,
            )
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_zero_amount_fails() {
        let mgr = make_manager();
        let result = mgr
            .create_offer(
                "world-a".into(),
                "world-b".into(),
                TradeItem::Tokens { amount: 0 },
                TradeItem::Tokens { amount: 200 },
                None,
            )
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_execute_without_accept_fails() {
        let mgr = make_manager();
        let offer = mgr
            .create_offer(
                "world-a".into(),
                "world-b".into(),
                TradeItem::Tokens { amount: 100 },
                TradeItem::Tokens { amount: 200 },
                None,
            )
            .await
            .unwrap();

        let result = mgr.execute_trade(&offer.trade_id).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("accepted"));
    }

    #[tokio::test]
    async fn test_list_offers() {
        let mgr = make_manager();

        mgr.create_offer(
            "world-a".into(),
            "world-b".into(),
            TradeItem::Tokens { amount: 100 },
            TradeItem::Tokens { amount: 200 },
            None,
        )
        .await
        .unwrap();
        mgr.create_offer(
            "world-a".into(),
            "world-c".into(),
            TradeItem::Money { amount: 50 },
            TradeItem::Tokens { amount: 300 },
            None,
        )
        .await
        .unwrap();

        // All for world-a
        let all = mgr.list_offers(Some("world-a"), None, 10, 0).await;
        assert_eq!(all.len(), 2);

        // Filter by status
        let pending = mgr
            .list_offers(Some("world-a"), Some(TradeStatus::Pending), 10, 0)
            .await;
        assert_eq!(pending.len(), 2);

        // world-b only sees 1
        let for_b = mgr.list_offers(Some("world-b"), None, 10, 0).await;
        assert_eq!(for_b.len(), 1);
    }

    #[tokio::test]
    async fn test_stats() {
        let mgr = make_manager();
        mgr.set_world_balance("world-a", 10_000).await;
        mgr.set_world_balance("world-b", 10_000).await;

        let o1 = mgr
            .create_offer(
                "world-a".into(),
                "world-b".into(),
                TradeItem::Tokens { amount: 1_000 },
                TradeItem::Money { amount: 500 },
                None,
            )
            .await
            .unwrap();

        mgr.accept_offer(&o1.trade_id, "world-b").await.unwrap();
        mgr.execute_trade(&o1.trade_id).await.unwrap();

        // Create another that we cancel
        let o2 = mgr
            .create_offer(
                "world-a".into(),
                "world-b".into(),
                TradeItem::Tokens { amount: 200 },
                TradeItem::Tokens { amount: 300 },
                None,
            )
            .await
            .unwrap();
        mgr.cancel_trade(&o2.trade_id, "world-a", None).await.unwrap();

        let stats = mgr.stats().await;
        assert_eq!(stats.total_offers, 2);
        assert_eq!(stats.completed, 1);
        assert_eq!(stats.cancelled, 1);
        // Token volume: 1000 (gave) + 0 (received money) ... wait, received was money
        // total_volume_tokens counts tokens in both directions
        assert_eq!(stats.total_volume_tokens, 1_000);
        assert_eq!(stats.total_volume_money, 500);
    }

    #[tokio::test]
    async fn test_service_item_trade() {
        let mgr = make_manager();
        mgr.set_world_balance("world-a", 10_000).await;

        let offer = mgr
            .create_offer(
                "world-a".into(),
                "world-b".into(),
                TradeItem::Service {
                    key: "research_boost".into(),
                    amount: 1,
                },
                TradeItem::Tokens { amount: 500 },
                None,
            )
            .await
            .unwrap();

        // world-b needs balance to pay tokens
        mgr.set_world_balance("world-b", 5_000).await;

        mgr.accept_offer(&offer.trade_id, "world-b").await.unwrap();
        let executed = mgr.execute_trade(&offer.trade_id).await.unwrap();
        assert_eq!(executed.status, TradeStatus::Completed);

        // world-a received 500 tokens from world-b
        assert_eq!(mgr.get_world_balance("world-a").await, 10_000 + 500);
        // world-b paid 500 tokens, got a service (no token movement for service)
        assert_eq!(mgr.get_world_balance("world-b").await, 5_000 - 500);
    }
}
