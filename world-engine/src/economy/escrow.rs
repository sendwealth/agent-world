use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::world::enums::Currency;
use crate::world::event::WorldEvent;
use crate::world::state::EventBus;

// ── Escrow Status ─────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EscrowStatus {
    /// Task published, reward locked, waiting for a claimant.
    Open,
    /// Claimed by an agent; deposit locked alongside the reward.
    Claimed,
    /// Task completed; funds released to the claimant.
    Completed,
    /// Task expired / cancelled; funds returned to the publisher.
    Refunded,
    /// Under dispute; funds frozen until arbitration.
    Disputed,
}

// ── Escrow Record ─────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EscrowRecord {
    pub id: Uuid,
    pub publisher: String,
    pub claimant: Option<String>,
    pub reward: u64,
    pub deposit: u64,
    pub currency: Currency,
    pub status: EscrowStatus,
    /// World tick when the escrow was created.
    pub created_tick: u64,
    /// Tick at which the escrow expires (None = no expiry).
    pub expires_at: Option<u64>,
}

// ── Errors ────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EscrowError {
    NotFound(String),
    /// The escrow is not in the required state for this operation.
    InvalidStatus {
        expected: EscrowStatus,
        actual: EscrowStatus,
    },
    /// The publisher does not have enough balance.
    InsufficientBalance {
        required: u64,
        available: u64,
    },
    /// The claimant does not have enough balance for the deposit.
    InsufficientDeposit {
        required: u64,
        available: u64,
    },
    /// A claimant is already assigned.
    AlreadyClaimed,
    /// The escrow has expired.
    Expired,
}

impl std::fmt::Display for EscrowError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EscrowError::NotFound(id) => write!(f, "escrow not found: {}", id),
            EscrowError::InvalidStatus { expected, actual } => {
                write!(
                    f,
                    "invalid escrow status: expected {:?}, got {:?}",
                    expected, actual
                )
            }
            EscrowError::InsufficientBalance {
                required,
                available,
            } => {
                write!(
                    f,
                    "insufficient balance: required {}, available {}",
                    required, available
                )
            }
            EscrowError::InsufficientDeposit {
                required,
                available,
            } => {
                write!(
                    f,
                    "insufficient deposit: required {}, available {}",
                    required, available
                )
            }
            EscrowError::AlreadyClaimed => write!(f, "escrow already claimed"),
            EscrowError::Expired => write!(f, "escrow has expired"),
        }
    }
}

impl std::error::Error for EscrowError {}

// ── Escrow Manager ────────────────────────────────────────

pub struct EscrowManager {
    escrows: HashMap<Uuid, EscrowRecord>,
    /// Agent balances used by the escrow system (publisher reward locking, claimant deposit).
    balances: HashMap<String, u64>,
    event_bus: Option<EventBus>,
}

impl EscrowManager {
    pub fn new() -> Self {
        Self {
            escrows: HashMap::new(),
            balances: HashMap::new(),
            event_bus: None,
        }
    }

    pub fn with_event_bus(event_bus: EventBus) -> Self {
        Self {
            escrows: HashMap::new(),
            balances: HashMap::new(),
            event_bus: Some(event_bus),
        }
    }

    // ── Balance helpers (test / simulation layer) ──────────

    /// Set an agent's balance in the escrow ledger.
    pub fn set_balance(&mut self, agent: &str, amount: u64) {
        self.balances.insert(agent.to_string(), amount);
    }

    /// Get an agent's balance from the escrow ledger.
    pub fn get_balance(&self, agent: &str) -> u64 {
        self.balances.get(agent).copied().unwrap_or(0)
    }

    /// Get a reference to an escrow record by ID.
    pub fn get(&self, id: Uuid) -> Option<&EscrowRecord> {
        self.escrows.get(&id)
    }

    /// List all escrow records.
    pub fn list(&self) -> Vec<&EscrowRecord> {
        self.escrows.values().collect()
    }

    // ── Core operations ────────────────────────────────────

    /// Create a new escrow: locks the full reward from the publisher's balance.
    pub fn create_escrow(
        &mut self,
        publisher: &str,
        reward: u64,
        deposit: u64,
        currency: Currency,
        created_tick: u64,
        expires_at: Option<u64>,
    ) -> Result<Uuid, EscrowError> {
        let available = self.get_balance(publisher);
        if available < reward {
            return Err(EscrowError::InsufficientBalance {
                required: reward,
                available,
            });
        }

        // Lock the reward
        self.balances
            .insert(publisher.to_string(), available - reward);

        let id = Uuid::new_v4();
        let record = EscrowRecord {
            id,
            publisher: publisher.to_string(),
            claimant: None,
            reward,
            deposit,
            currency,
            status: EscrowStatus::Open,
            created_tick,
            expires_at,
        };
        self.escrows.insert(id, record);

        self.emit(WorldEvent::EscrowCreated {
            escrow_id: id.to_string(),
            publisher: publisher.to_string(),
            reward,
            currency,
        });

        Ok(id)
    }

    /// Claim an open escrow: locks the deposit from the claimant's balance.
    pub fn claim_escrow(&mut self, escrow_id: Uuid, claimant: &str) -> Result<(), EscrowError> {
        // Validate status first
        let deposit = {
            let record = self
                .escrows
                .get(&escrow_id)
                .ok_or_else(|| EscrowError::NotFound(escrow_id.to_string()))?;
            if record.status != EscrowStatus::Open {
                return Err(EscrowError::InvalidStatus {
                    expected: EscrowStatus::Open,
                    actual: record.status,
                });
            }
            record.deposit
        };

        // Check claimant balance
        let available = self.get_balance(claimant);
        if available < deposit {
            return Err(EscrowError::InsufficientDeposit {
                required: deposit,
                available,
            });
        }

        // Lock the deposit
        self.balances
            .insert(claimant.to_string(), available - deposit);

        // Update the record
        let record = self.escrows.get_mut(&escrow_id).unwrap();
        record.claimant = Some(claimant.to_string());
        record.status = EscrowStatus::Claimed;

        self.emit(WorldEvent::EscrowClaimed {
            escrow_id: escrow_id.to_string(),
            claimant: claimant.to_string(),
            deposit,
        });

        Ok(())
    }

    /// Complete an escrow: releases reward + deposit to the claimant.
    pub fn complete_escrow(&mut self, escrow_id: Uuid) -> Result<(), EscrowError> {
        // Extract needed data
        let (claimant, total, currency) = {
            let record = self
                .escrows
                .get(&escrow_id)
                .ok_or_else(|| EscrowError::NotFound(escrow_id.to_string()))?;
            if record.status != EscrowStatus::Claimed {
                return Err(EscrowError::InvalidStatus {
                    expected: EscrowStatus::Claimed,
                    actual: record.status,
                });
            }
            let claimant = record.claimant.clone().ok_or(EscrowError::AlreadyClaimed)?;
            (claimant, record.reward + record.deposit, record.currency)
        };

        // Release funds
        let current = self.get_balance(&claimant);
        self.balances.insert(claimant.clone(), current + total);

        // Update status
        self.escrows.get_mut(&escrow_id).unwrap().status = EscrowStatus::Completed;

        self.emit(WorldEvent::EscrowReleased {
            escrow_id: escrow_id.to_string(),
            recipient: claimant,
            amount: total,
            currency,
        });

        Ok(())
    }

    /// Refund an escrow: returns reward to publisher and deposit to claimant (if any).
    pub fn refund_escrow(&mut self, escrow_id: Uuid) -> Result<(), EscrowError> {
        // Extract needed data
        let (publisher, reward, claimant, deposit, currency) = {
            let record = self
                .escrows
                .get(&escrow_id)
                .ok_or_else(|| EscrowError::NotFound(escrow_id.to_string()))?;
            match record.status {
                EscrowStatus::Open | EscrowStatus::Claimed => {}
                _ => {
                    return Err(EscrowError::InvalidStatus {
                        expected: EscrowStatus::Open,
                        actual: record.status,
                    });
                }
            }
            (
                record.publisher.clone(),
                record.reward,
                record.claimant.clone(),
                record.deposit,
                record.currency,
            )
        };

        // Return reward to publisher
        let pub_bal = self.get_balance(&publisher);
        self.balances.insert(publisher.clone(), pub_bal + reward);

        // Return deposit to claimant if one exists
        if let Some(ref claimant) = claimant {
            let claim_bal = self.get_balance(claimant);
            self.balances.insert(claimant.clone(), claim_bal + deposit);
        }

        // Update status
        self.escrows.get_mut(&escrow_id).unwrap().status = EscrowStatus::Refunded;

        self.emit(WorldEvent::EscrowRefunded {
            escrow_id: escrow_id.to_string(),
            recipient: publisher,
            amount: reward,
            currency,
        });

        Ok(())
    }

    /// Dispute an escrow: freezes funds pending arbitration.
    pub fn dispute_escrow(&mut self, escrow_id: Uuid, reason: &str) -> Result<(), EscrowError> {
        let record = self
            .escrows
            .get_mut(&escrow_id)
            .ok_or_else(|| EscrowError::NotFound(escrow_id.to_string()))?;

        if record.status != EscrowStatus::Claimed {
            return Err(EscrowError::InvalidStatus {
                expected: EscrowStatus::Claimed,
                actual: record.status,
            });
        }

        record.status = EscrowStatus::Disputed;

        self.emit(WorldEvent::EscrowFrozen {
            escrow_id: escrow_id.to_string(),
            reason: reason.to_string(),
        });

        Ok(())
    }

    /// Resolve a dispute: release funds to claimant (true) or refund to publisher (false).
    pub fn resolve_dispute(
        &mut self,
        escrow_id: Uuid,
        favor_claimant: bool,
    ) -> Result<(), EscrowError> {
        // Validate and extract data
        let (publisher, claimant, reward, deposit, currency) = {
            let record = self
                .escrows
                .get(&escrow_id)
                .ok_or_else(|| EscrowError::NotFound(escrow_id.to_string()))?;
            if record.status != EscrowStatus::Disputed {
                return Err(EscrowError::InvalidStatus {
                    expected: EscrowStatus::Disputed,
                    actual: record.status,
                });
            }
            (
                record.publisher.clone(),
                record.claimant.clone(),
                record.reward,
                record.deposit,
                record.currency,
            )
        };

        if favor_claimant {
            let claimant = claimant.ok_or(EscrowError::AlreadyClaimed)?;
            let total = reward + deposit;
            let current = self.get_balance(&claimant);
            self.balances.insert(claimant.clone(), current + total);

            self.escrows.get_mut(&escrow_id).unwrap().status = EscrowStatus::Completed;

            self.emit(WorldEvent::EscrowReleased {
                escrow_id: escrow_id.to_string(),
                recipient: claimant,
                amount: total,
                currency,
            });
        } else {
            // Refund reward to publisher
            let pub_bal = self.get_balance(&publisher);
            self.balances.insert(publisher.clone(), pub_bal + reward);

            // Refund deposit to claimant
            if let Some(ref claimant) = claimant {
                let claim_bal = self.get_balance(claimant);
                self.balances.insert(claimant.clone(), claim_bal + deposit);
            }

            self.escrows.get_mut(&escrow_id).unwrap().status = EscrowStatus::Refunded;

            self.emit(WorldEvent::EscrowRefunded {
                escrow_id: escrow_id.to_string(),
                recipient: publisher,
                amount: reward,
                currency,
            });
        }

        Ok(())
    }

    /// Process expired escrows for a given tick.
    /// Refunds all escrows whose expires_at <= current_tick and are still Open or Claimed.
    pub fn process_expiry(&mut self, current_tick: u64) -> Vec<Uuid> {
        let expired_ids: Vec<Uuid> = self
            .escrows
            .iter()
            .filter(|(_, record)| {
                matches!(record.status, EscrowStatus::Open | EscrowStatus::Claimed)
                    && record.expires_at.is_some_and(|exp| exp <= current_tick)
            })
            .map(|(id, _)| *id)
            .collect();

        for id in &expired_ids {
            let _ = self.refund_escrow(*id);
        }

        expired_ids
    }

    // ── Helpers ────────────────────────────────────────────

    fn emit(&self, event: WorldEvent) {
        if let Some(ref bus) = self.event_bus {
            bus.emit(event);
        }
    }
}

impl Default for EscrowManager {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ─────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::event::EventType;

    fn make_manager() -> EscrowManager {
        let mut mgr = EscrowManager::new();
        mgr.set_balance("publisher", 10_000);
        mgr.set_balance("claimant", 5_000);
        mgr
    }

    // ── Create escrow ──────────────────────────────────────

    #[test]
    fn test_create_escrow_locks_reward() {
        let mut mgr = make_manager();
        let id = mgr
            .create_escrow("publisher", 1000, 200, Currency::Token, 1, None)
            .unwrap();
        assert_eq!(mgr.get_balance("publisher"), 9_000);
        let record = mgr.get(id).unwrap();
        assert_eq!(record.reward, 1000);
        assert_eq!(record.deposit, 200);
        assert_eq!(record.status, EscrowStatus::Open);
        assert_eq!(record.publisher, "publisher");
        assert!(record.claimant.is_none());
    }

    #[test]
    fn test_create_escrow_insufficient_balance() {
        let mut mgr = make_manager();
        let result = mgr.create_escrow("publisher", 20_000, 200, Currency::Token, 1, None);
        assert!(result.is_err());
        assert_eq!(mgr.get_balance("publisher"), 10_000);
    }

    #[test]
    fn test_create_escrow_with_expiry() {
        let mut mgr = make_manager();
        let id = mgr
            .create_escrow("publisher", 500, 100, Currency::Money, 10, Some(100))
            .unwrap();
        let record = mgr.get(id).unwrap();
        assert_eq!(record.expires_at, Some(100));
    }

    // ── Claim escrow ───────────────────────────────────────

    #[test]
    fn test_claim_escrow_locks_deposit() {
        let mut mgr = make_manager();
        let id = mgr
            .create_escrow("publisher", 1000, 200, Currency::Token, 1, None)
            .unwrap();
        mgr.claim_escrow(id, "claimant").unwrap();
        assert_eq!(mgr.get_balance("claimant"), 4_800);
        let record = mgr.get(id).unwrap();
        assert_eq!(record.status, EscrowStatus::Claimed);
        assert_eq!(record.claimant.as_deref(), Some("claimant"));
    }

    #[test]
    fn test_claim_escrow_insufficient_deposit() {
        let mut mgr = make_manager();
        mgr.set_balance("poor_claimant", 50);
        let id = mgr
            .create_escrow("publisher", 1000, 200, Currency::Token, 1, None)
            .unwrap();
        let result = mgr.claim_escrow(id, "poor_claimant");
        assert!(result.is_err());
        assert_eq!(mgr.get(id).unwrap().status, EscrowStatus::Open);
    }

    #[test]
    fn test_claim_escrow_already_claimed() {
        let mut mgr = make_manager();
        let id = mgr
            .create_escrow("publisher", 1000, 200, Currency::Token, 1, None)
            .unwrap();
        mgr.claim_escrow(id, "claimant").unwrap();
        mgr.set_balance("other_claimant", 5_000);
        let result = mgr.claim_escrow(id, "other_claimant");
        assert!(result.is_err());
    }

    #[test]
    fn test_claim_escrow_wrong_status_completed() {
        let mut mgr = make_manager();
        let id = mgr
            .create_escrow("publisher", 1000, 200, Currency::Token, 1, None)
            .unwrap();
        mgr.claim_escrow(id, "claimant").unwrap();
        mgr.complete_escrow(id).unwrap();
        let result = mgr.claim_escrow(id, "new_claimant");
        assert!(result.is_err());
    }

    // ── Complete escrow ────────────────────────────────────

    #[test]
    fn test_complete_escrow_releases_to_claimant() {
        let mut mgr = make_manager();
        let id = mgr
            .create_escrow("publisher", 1000, 200, Currency::Token, 1, None)
            .unwrap();
        mgr.claim_escrow(id, "claimant").unwrap();
        mgr.complete_escrow(id).unwrap();
        // claimant gets reward (1000) + deposit back (200) = 1200 added to remaining 4800
        assert_eq!(mgr.get_balance("claimant"), 4_800 + 1_200);
        assert_eq!(mgr.get(id).unwrap().status, EscrowStatus::Completed);
    }

    #[test]
    fn test_complete_escrow_wrong_status_open() {
        let mut mgr = make_manager();
        let id = mgr
            .create_escrow("publisher", 1000, 200, Currency::Token, 1, None)
            .unwrap();
        let result = mgr.complete_escrow(id);
        assert!(result.is_err());
    }

    // ── Refund escrow ──────────────────────────────────────

    #[test]
    fn test_refund_open_escrow_returns_reward() {
        let mut mgr = make_manager();
        let id = mgr
            .create_escrow("publisher", 1000, 200, Currency::Token, 1, None)
            .unwrap();
        mgr.refund_escrow(id).unwrap();
        assert_eq!(mgr.get_balance("publisher"), 10_000);
        assert_eq!(mgr.get(id).unwrap().status, EscrowStatus::Refunded);
    }

    #[test]
    fn test_refund_claimed_escrow_returns_both() {
        let mut mgr = make_manager();
        let id = mgr
            .create_escrow("publisher", 1000, 200, Currency::Token, 1, None)
            .unwrap();
        mgr.claim_escrow(id, "claimant").unwrap();
        mgr.refund_escrow(id).unwrap();
        // publisher gets reward back
        assert_eq!(mgr.get_balance("publisher"), 10_000);
        // claimant gets deposit back
        assert_eq!(mgr.get_balance("claimant"), 5_000);
    }

    #[test]
    fn test_refund_completed_escrow_fails() {
        let mut mgr = make_manager();
        let id = mgr
            .create_escrow("publisher", 1000, 200, Currency::Token, 1, None)
            .unwrap();
        mgr.claim_escrow(id, "claimant").unwrap();
        mgr.complete_escrow(id).unwrap();
        let result = mgr.refund_escrow(id);
        assert!(result.is_err());
    }

    // ── Dispute escrow ─────────────────────────────────────

    #[test]
    fn test_dispute_freezes_funds() {
        let mut mgr = make_manager();
        let id = mgr
            .create_escrow("publisher", 1000, 200, Currency::Token, 1, None)
            .unwrap();
        mgr.claim_escrow(id, "claimant").unwrap();
        mgr.dispute_escrow(id, "quality issue").unwrap();
        assert_eq!(mgr.get(id).unwrap().status, EscrowStatus::Disputed);
        // Balances should not change during dispute
        assert_eq!(mgr.get_balance("publisher"), 9_000);
        assert_eq!(mgr.get_balance("claimant"), 4_800);
    }

    #[test]
    fn test_dispute_open_escrow_fails() {
        let mut mgr = make_manager();
        let id = mgr
            .create_escrow("publisher", 1000, 200, Currency::Token, 1, None)
            .unwrap();
        let result = mgr.dispute_escrow(id, "bad");
        assert!(result.is_err());
    }

    #[test]
    fn test_dispute_completed_escrow_fails() {
        let mut mgr = make_manager();
        let id = mgr
            .create_escrow("publisher", 1000, 200, Currency::Token, 1, None)
            .unwrap();
        mgr.claim_escrow(id, "claimant").unwrap();
        mgr.complete_escrow(id).unwrap();
        let result = mgr.dispute_escrow(id, "bad");
        assert!(result.is_err());
    }

    // ── Resolve dispute ────────────────────────────────────

    #[test]
    fn test_resolve_dispute_favor_claimant() {
        let mut mgr = make_manager();
        let id = mgr
            .create_escrow("publisher", 1000, 200, Currency::Token, 1, None)
            .unwrap();
        mgr.claim_escrow(id, "claimant").unwrap();
        mgr.dispute_escrow(id, "quality issue").unwrap();
        mgr.resolve_dispute(id, true).unwrap();
        // claimant gets reward + deposit
        assert_eq!(mgr.get_balance("claimant"), 4_800 + 1_200);
        assert_eq!(mgr.get(id).unwrap().status, EscrowStatus::Completed);
    }

    #[test]
    fn test_resolve_dispute_favor_publisher() {
        let mut mgr = make_manager();
        let id = mgr
            .create_escrow("publisher", 1000, 200, Currency::Token, 1, None)
            .unwrap();
        mgr.claim_escrow(id, "claimant").unwrap();
        mgr.dispute_escrow(id, "quality issue").unwrap();
        mgr.resolve_dispute(id, false).unwrap();
        // publisher gets reward back
        assert_eq!(mgr.get_balance("publisher"), 10_000);
        // claimant gets deposit back
        assert_eq!(mgr.get_balance("claimant"), 5_000);
        assert_eq!(mgr.get(id).unwrap().status, EscrowStatus::Refunded);
    }

    #[test]
    fn test_resolve_non_disputed_escrow_fails() {
        let mut mgr = make_manager();
        let id = mgr
            .create_escrow("publisher", 1000, 200, Currency::Token, 1, None)
            .unwrap();
        mgr.claim_escrow(id, "claimant").unwrap();
        let result = mgr.resolve_dispute(id, true);
        assert!(result.is_err());
    }

    // ── Process expiry ─────────────────────────────────────

    #[test]
    fn test_process_expiry_refunds_expired_open() {
        let mut mgr = make_manager();
        let id = mgr
            .create_escrow("publisher", 1000, 200, Currency::Token, 1, Some(50))
            .unwrap();
        let expired = mgr.process_expiry(100);
        assert_eq!(expired.len(), 1);
        assert_eq!(expired[0], id);
        assert_eq!(mgr.get(id).unwrap().status, EscrowStatus::Refunded);
        assert_eq!(mgr.get_balance("publisher"), 10_000);
    }

    #[test]
    fn test_process_expiry_refunds_expired_claimed() {
        let mut mgr = make_manager();
        let id = mgr
            .create_escrow("publisher", 1000, 200, Currency::Token, 1, Some(50))
            .unwrap();
        mgr.claim_escrow(id, "claimant").unwrap();
        let expired = mgr.process_expiry(100);
        assert_eq!(expired.len(), 1);
        assert_eq!(mgr.get(id).unwrap().status, EscrowStatus::Refunded);
        assert_eq!(mgr.get_balance("publisher"), 10_000);
        assert_eq!(mgr.get_balance("claimant"), 5_000);
    }

    #[test]
    fn test_process_expiry_skips_not_yet_expired() {
        let mut mgr = make_manager();
        let id = mgr
            .create_escrow("publisher", 1000, 200, Currency::Token, 1, Some(200))
            .unwrap();
        let expired = mgr.process_expiry(100);
        assert!(expired.is_empty());
        assert_eq!(mgr.get(id).unwrap().status, EscrowStatus::Open);
    }

    #[test]
    fn test_process_expiry_no_expiry_set() {
        let mut mgr = make_manager();
        let _id = mgr
            .create_escrow("publisher", 1000, 200, Currency::Token, 1, None)
            .unwrap();
        let expired = mgr.process_expiry(10_000);
        assert!(expired.is_empty());
    }

    #[test]
    fn test_process_expiry_multiple_mixed() {
        let mut mgr = make_manager();
        let id1 = mgr
            .create_escrow("publisher", 100, 20, Currency::Token, 1, Some(50))
            .unwrap();
        let id2 = mgr
            .create_escrow("publisher", 200, 40, Currency::Token, 1, Some(200))
            .unwrap();
        let id3 = mgr
            .create_escrow("publisher", 300, 60, Currency::Token, 1, Some(50))
            .unwrap();

        let expired = mgr.process_expiry(100);
        assert_eq!(expired.len(), 2);
        assert!(expired.contains(&id1));
        assert!(expired.contains(&id3));
        assert!(!expired.contains(&id2));
        assert_eq!(mgr.get(id2).unwrap().status, EscrowStatus::Open);
    }

    // ── Not found ──────────────────────────────────────────

    #[test]
    fn test_operations_on_nonexistent_escrow() {
        let mgr = EscrowManager::new();
        let fake_id = Uuid::new_v4();
        assert!(mgr.get(fake_id).is_none());
    }

    // ── Full lifecycle ─────────────────────────────────────

    #[test]
    fn test_full_lifecycle_create_claim_complete() {
        let mut mgr = make_manager();
        let id = mgr
            .create_escrow("publisher", 1000, 200, Currency::Token, 1, None)
            .unwrap();
        assert_eq!(mgr.get_balance("publisher"), 9_000);
        mgr.claim_escrow(id, "claimant").unwrap();
        assert_eq!(mgr.get_balance("claimant"), 4_800);
        mgr.complete_escrow(id).unwrap();
        assert_eq!(mgr.get_balance("claimant"), 6_000);
        assert_eq!(mgr.get_balance("publisher"), 9_000);
    }

    #[test]
    fn test_full_lifecycle_create_claim_refund() {
        let mut mgr = make_manager();
        let id = mgr
            .create_escrow("publisher", 1000, 200, Currency::Token, 1, None)
            .unwrap();
        mgr.claim_escrow(id, "claimant").unwrap();
        mgr.refund_escrow(id).unwrap();
        assert_eq!(mgr.get_balance("publisher"), 10_000);
        assert_eq!(mgr.get_balance("claimant"), 5_000);
    }

    #[test]
    fn test_full_lifecycle_create_claim_dispute_resolve_claimant() {
        let mut mgr = make_manager();
        let id = mgr
            .create_escrow("publisher", 1000, 200, Currency::Token, 1, None)
            .unwrap();
        mgr.claim_escrow(id, "claimant").unwrap();
        mgr.dispute_escrow(id, "bad work").unwrap();
        mgr.resolve_dispute(id, true).unwrap();
        assert_eq!(mgr.get_balance("claimant"), 6_000);
        assert_eq!(mgr.get_balance("publisher"), 9_000);
    }

    #[test]
    fn test_full_lifecycle_create_claim_dispute_resolve_publisher() {
        let mut mgr = make_manager();
        let id = mgr
            .create_escrow("publisher", 1000, 200, Currency::Token, 1, None)
            .unwrap();
        mgr.claim_escrow(id, "claimant").unwrap();
        mgr.dispute_escrow(id, "bad work").unwrap();
        mgr.resolve_dispute(id, false).unwrap();
        assert_eq!(mgr.get_balance("publisher"), 10_000);
        assert_eq!(mgr.get_balance("claimant"), 5_000);
    }

    // ── Serialization ──────────────────────────────────────

    #[test]
    fn test_escrow_record_serialization() {
        let record = EscrowRecord {
            id: Uuid::new_v4(),
            publisher: "alice".to_string(),
            claimant: Some("bob".to_string()),
            reward: 500,
            deposit: 100,
            currency: Currency::Token,
            status: EscrowStatus::Claimed,
            created_tick: 1,
            expires_at: Some(100),
        };
        let json = serde_json::to_string(&record).unwrap();
        let back: EscrowRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(record.id, back.id);
        assert_eq!(record.publisher, back.publisher);
        assert_eq!(record.claimant, back.claimant);
        assert_eq!(record.reward, back.reward);
        assert_eq!(record.deposit, back.deposit);
        assert_eq!(record.currency, back.currency);
        assert_eq!(record.status, back.status);
        assert_eq!(record.created_tick, back.created_tick);
        assert_eq!(record.expires_at, back.expires_at);
    }

    #[test]
    fn test_escrow_status_serialization() {
        for status in [
            EscrowStatus::Open,
            EscrowStatus::Claimed,
            EscrowStatus::Completed,
            EscrowStatus::Refunded,
            EscrowStatus::Disputed,
        ] {
            let json = serde_json::to_string(&status).unwrap();
            let back: EscrowStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(status, back);
        }
    }

    // ── List ───────────────────────────────────────────────

    #[test]
    fn test_list_escrows() {
        let mut mgr = make_manager();
        assert!(mgr.list().is_empty());
        mgr.create_escrow("publisher", 100, 20, Currency::Token, 1, None)
            .unwrap();
        mgr.create_escrow("publisher", 200, 40, Currency::Token, 1, None)
            .unwrap();
        assert_eq!(mgr.list().len(), 2);
    }

    // ── Zero deposit ───────────────────────────────────────

    #[test]
    fn test_zero_deposit_claim() {
        let mut mgr = make_manager();
        let id = mgr
            .create_escrow("publisher", 1000, 0, Currency::Token, 1, None)
            .unwrap();
        mgr.set_balance("free_claimant", 0);
        mgr.claim_escrow(id, "free_claimant").unwrap();
        assert_eq!(mgr.get_balance("free_claimant"), 0);
        mgr.complete_escrow(id).unwrap();
        assert_eq!(mgr.get_balance("free_claimant"), 1000);
    }

    // ── Money currency ─────────────────────────────────────

    #[test]
    fn test_escrow_with_money_currency() {
        let mut mgr = make_manager();
        let id = mgr
            .create_escrow("publisher", 500, 100, Currency::Money, 1, None)
            .unwrap();
        mgr.claim_escrow(id, "claimant").unwrap();
        mgr.complete_escrow(id).unwrap();
        let record = mgr.get(id).unwrap();
        assert_eq!(record.currency, Currency::Money);
    }

    // ── Error display ──────────────────────────────────────

    #[test]
    fn test_error_display() {
        assert!(EscrowError::NotFound("test".into())
            .to_string()
            .contains("test"));
        assert!(EscrowError::Expired.to_string().contains("expired"));
        assert!(EscrowError::AlreadyClaimed
            .to_string()
            .contains("already claimed"));
    }

    // ── Event bus integration ──────────────────────────────

    #[test]
    fn test_event_bus_escrow_created() {
        let bus = EventBus::new(64);
        let mut rx = bus.subscribe();
        let mut mgr = EscrowManager::with_event_bus(bus);
        mgr.set_balance("publisher", 1000);
        let id = mgr
            .create_escrow("publisher", 100, 20, Currency::Token, 1, None)
            .unwrap();

        let event = rx.try_recv().unwrap();
        match event {
            WorldEvent::EscrowCreated {
                escrow_id,
                publisher,
                reward,
                currency,
            } => {
                assert_eq!(escrow_id, id.to_string());
                assert_eq!(publisher, "publisher");
                assert_eq!(reward, 100);
                assert_eq!(currency, Currency::Token);
            }
            _ => panic!("expected EscrowCreated event, got {:?}", event),
        }
    }

    #[test]
    fn test_event_bus_full_lifecycle() {
        let bus = EventBus::new(64);
        let mut rx = bus.subscribe();
        let mut mgr = EscrowManager::with_event_bus(bus);
        mgr.set_balance("publisher", 10_000);
        mgr.set_balance("claimant", 5_000);

        let id = mgr
            .create_escrow("publisher", 1000, 200, Currency::Token, 1, None)
            .unwrap();
        let _ = rx.try_recv().unwrap(); // EscrowCreated

        mgr.claim_escrow(id, "claimant").unwrap();
        let claimed = rx.try_recv().unwrap();
        assert_eq!(claimed.event_type(), EventType::EscrowClaimed);

        mgr.complete_escrow(id).unwrap();
        let released = rx.try_recv().unwrap();
        assert_eq!(released.event_type(), EventType::EscrowReleased);
    }
}
