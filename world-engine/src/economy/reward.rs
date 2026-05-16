use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::world::enums::Currency;

// ── Transaction Type ──────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransactionType {
    /// Task reward paid to assignee.
    TaskReward,
    /// Platform commission collected by the central bank.
    PlatformFee,
    /// Task escrow refund to publisher (on expiry/cancellation).
    EscrowRefund,
    /// Token exchange between Money and Token.
    Exchange,
    /// Interest payment.
    Interest,
    /// Knowledge market transaction.
    Knowledge,
    /// Teaching fee.
    Teach,
}

// ── Ledger Entry ──────────────────────────────────────────

/// Immutable record of a single financial transaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerEntry {
    pub id: Uuid,
    /// Sender agent ID. None means the central bank.
    pub from_agent: Option<String>,
    /// Receiver agent ID. None means the central bank.
    pub to_agent: Option<String>,
    pub amount: u64,
    pub currency: Currency,
    pub tx_type: TransactionType,
    pub description: String,
    pub tick: u64,
    /// Reference to the source entity (e.g. task ID).
    pub reference_id: Option<String>,
}

// ── Ledger ────────────────────────────────────────────────

/// Append-only ledger that records all financial transactions.
pub struct Ledger {
    entries: Vec<LedgerEntry>,
}

impl Ledger {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Record a new transaction in the ledger.
    #[allow(clippy::too_many_arguments)]
    pub fn record(
        &mut self,
        from_agent: Option<String>,
        to_agent: Option<String>,
        amount: u64,
        currency: Currency,
        tx_type: TransactionType,
        description: String,
        tick: u64,
        reference_id: Option<String>,
    ) -> Uuid {
        let id = Uuid::new_v4();
        let entry = LedgerEntry {
            id,
            from_agent,
            to_agent,
            amount,
            currency,
            tx_type,
            description,
            tick,
            reference_id,
        };
        self.entries.push(entry);
        id
    }

    /// Get all ledger entries.
    pub fn list(&self) -> &[LedgerEntry] {
        &self.entries
    }

    /// Get entries by agent (either sender or receiver).
    pub fn list_by_agent(&self, agent_id: &str) -> Vec<&LedgerEntry> {
        self.entries
            .iter()
            .filter(|e| {
                e.from_agent.as_deref() == Some(agent_id)
                    || e.to_agent.as_deref() == Some(agent_id)
            })
            .collect()
    }

    /// Get entries by reference ID (e.g. task ID).
    pub fn list_by_reference(&self, reference_id: &str) -> Vec<&LedgerEntry> {
        self.entries
            .iter()
            .filter(|e| e.reference_id.as_deref() == Some(reference_id))
            .collect()
    }

    /// Get entries by transaction type.
    pub fn list_by_type(&self, tx_type: TransactionType) -> Vec<&LedgerEntry> {
        self.entries
            .iter()
            .filter(|e| e.tx_type == tx_type)
            .collect()
    }
}

impl Default for Ledger {
    fn default() -> Self {
        Self::new()
    }
}

// ── Central Bank ──────────────────────────────────────────

const CENTRAL_BANK_ID: &str = "central_bank";

/// Represents the central bank that collects platform fees.
pub struct CentralBank {
    /// Total fees collected, by currency.
    collected_fees: HashMap<Currency, u64>,
}

impl CentralBank {
    pub fn new() -> Self {
        Self {
            collected_fees: HashMap::new(),
        }
    }

    /// Record a fee collection.
    pub fn collect_fee(&mut self, amount: u64, currency: Currency) {
        *self.collected_fees.entry(currency).or_insert(0) += amount;
    }

    /// Get total fees collected for a currency.
    pub fn total_fees(&self, currency: Currency) -> u64 {
        self.collected_fees.get(&currency).copied().unwrap_or(0)
    }

    /// Get the central bank identifier.
    pub fn id() -> &'static str {
        CENTRAL_BANK_ID
    }
}

impl Default for CentralBank {
    fn default() -> Self {
        Self::new()
    }
}

// ── Reward Config ─────────────────────────────────────────

/// Configuration for reward distribution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RewardConfig {
    /// Platform fee rate in basis points (200 = 2%).
    pub platform_fee_bps: u32,
    /// Base XP awarded on task completion.
    pub base_xp: u64,
    /// Reputation change on task completion.
    pub reputation_gain: f64,
}

impl Default for RewardConfig {
    fn default() -> Self {
        Self {
            platform_fee_bps: 200, // 2%
            base_xp: 50,
            reputation_gain: 2.0,
        }
    }
}

impl RewardConfig {
    /// Calculate the platform fee for a given reward amount.
    pub fn calculate_fee(&self, reward: u64) -> u64 {
        // Use u128 to avoid overflow on large rewards, then truncate.
        ((reward as u128 * self.platform_fee_bps as u128) / 10_000) as u64
    }

    /// Calculate the net reward after platform fee.
    pub fn calculate_net_reward(&self, reward: u64) -> u64 {
        reward.saturating_sub(self.calculate_fee(reward))
    }
}

// ── Reward Distribution Result ────────────────────────────

/// Result of distributing a task reward.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RewardDistribution {
    /// Original escrow/reward amount.
    pub gross_reward: u64,
    /// Platform fee taken by the central bank.
    pub platform_fee: u64,
    /// Net reward paid to the assignee.
    pub net_reward: u64,
    /// XP awarded to the assignee.
    pub xp_awarded: u64,
    /// Reputation change applied to the assignee.
    pub reputation_change: f64,
    /// Ledger entry ID for the reward transaction.
    pub reward_ledger_id: Uuid,
    /// Ledger entry ID for the fee transaction.
    pub fee_ledger_id: Uuid,
}

// ── Reward Distributor ────────────────────────────────────

/// Handles reward distribution when a task is completed.
///
/// Responsibilities:
/// 1. Calculate and deduct 2% platform fee → central bank
/// 2. Pay net reward to the assignee
/// 3. Award XP to the assignee
/// 4. Update assignee reputation (+2.0)
/// 5. Record both transactions in the ledger
pub struct RewardDistributor {
    config: RewardConfig,
    central_bank: CentralBank,
    ledger: Ledger,
    /// Agent reputation scores.
    reputation: HashMap<String, f64>,
    /// Agent XP totals.
    experience: HashMap<String, u64>,
    /// Agent balances for reward payouts.
    balances: HashMap<String, u64>,
}

impl RewardDistributor {
    pub fn new(config: RewardConfig) -> Self {
        Self {
            config,
            central_bank: CentralBank::new(),
            ledger: Ledger::new(),
            reputation: HashMap::new(),
            experience: HashMap::new(),
            balances: HashMap::new(),
        }
    }

    // ── Balance helpers ────────────────────────────────────

    pub fn set_balance(&mut self, agent: &str, amount: u64) {
        self.balances.insert(agent.to_string(), amount);
    }

    pub fn get_balance(&self, agent: &str) -> u64 {
        self.balances.get(agent).copied().unwrap_or(0)
    }

    // ── Reputation helpers ─────────────────────────────────

    pub fn get_reputation(&self, agent: &str) -> f64 {
        self.reputation.get(agent).copied().unwrap_or(0.0)
    }

    // ── XP helpers ─────────────────────────────────────────

    pub fn get_experience(&self, agent: &str) -> u64 {
        self.experience.get(agent).copied().unwrap_or(0)
    }

    // ── Ledger access ──────────────────────────────────────

    pub fn ledger(&self) -> &Ledger {
        &self.ledger
    }

    pub fn central_bank(&self) -> &CentralBank {
        &self.central_bank
    }

    // ── Core: distribute reward ────────────────────────────

    /// Distribute a task reward upon completion.
    ///
    /// Returns the distribution details.
    pub fn distribute_reward(
        &mut self,
        task_id: &str,
        assignee_id: &str,
        gross_reward: u64,
        currency: Currency,
        tick: u64,
    ) -> RewardDistribution {
        let platform_fee = self.config.calculate_fee(gross_reward);
        let net_reward = gross_reward.saturating_sub(platform_fee);

        // 1. Pay net reward to assignee
        let assignee_bal = self.get_balance(assignee_id);
        self.balances.insert(assignee_id.to_string(), assignee_bal.saturating_add(net_reward));

        // 2. Collect platform fee
        self.central_bank.collect_fee(platform_fee, currency);

        // 3. Award XP
        let current_xp = self.get_experience(assignee_id);
        self.experience.insert(assignee_id.to_string(), current_xp.saturating_add(self.config.base_xp));

        // 4. Update reputation
        let current_rep = self.get_reputation(assignee_id);
        self.reputation.insert(
            assignee_id.to_string(),
            current_rep + self.config.reputation_gain,
        );

        // 5. Record reward transaction in ledger
        let reward_ledger_id = self.ledger.record(
            None, // from: escrow (no specific agent)
            Some(assignee_id.to_string()),
            net_reward,
            currency,
            TransactionType::TaskReward,
            format!("Task {} reward (net after {} fee)", task_id, platform_fee),
            tick,
            Some(task_id.to_string()),
        );

        // 6. Record fee transaction in ledger
        let fee_ledger_id = self.ledger.record(
            Some(assignee_id.to_string()),
            None, // to: central bank
            platform_fee,
            currency,
            TransactionType::PlatformFee,
            format!("Task {} platform fee ({}bps)", task_id, self.config.platform_fee_bps),
            tick,
            Some(task_id.to_string()),
        );

        RewardDistribution {
            gross_reward,
            platform_fee,
            net_reward,
            xp_awarded: self.config.base_xp,
            reputation_change: self.config.reputation_gain,
            reward_ledger_id,
            fee_ledger_id,
        }
    }
}

// ── Tests ─────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_distributor() -> RewardDistributor {
        let mut dist = RewardDistributor::new(RewardConfig::default());
        dist.set_balance("worker", 1_000);
        dist
    }

    // ── RewardConfig ───────────────────────────────────────

    #[test]
    fn test_config_default_fee_is_2_percent() {
        let config = RewardConfig::default();
        assert_eq!(config.platform_fee_bps, 200);
        assert_eq!(config.calculate_fee(1000), 20);
        assert_eq!(config.calculate_fee(500), 10);
        assert_eq!(config.calculate_net_reward(1000), 980);
        assert_eq!(config.calculate_net_reward(500), 490);
    }

    #[test]
    fn test_config_fee_rounding() {
        let config = RewardConfig::default();
        // 2% of 99 = 1.98 → truncates to 1
        assert_eq!(config.calculate_fee(99), 1);
        // 2% of 1 = 0.02 → truncates to 0
        assert_eq!(config.calculate_fee(1), 0);
        // 2% of 50 = 1.0 → exactly 1
        assert_eq!(config.calculate_fee(50), 1);
    }

    #[test]
    fn test_config_fee_on_zero() {
        let config = RewardConfig::default();
        assert_eq!(config.calculate_fee(0), 0);
        assert_eq!(config.calculate_net_reward(0), 0);
    }

    // ── CentralBank ────────────────────────────────────────

    #[test]
    fn test_central_bank_collect_and_query() {
        let mut bank = CentralBank::new();
        bank.collect_fee(100, Currency::Money);
        bank.collect_fee(50, Currency::Money);
        bank.collect_fee(200, Currency::Token);
        assert_eq!(bank.total_fees(Currency::Money), 150);
        assert_eq!(bank.total_fees(Currency::Token), 200);
        assert_eq!(bank.total_fees(Currency::Money), 150); // still 150
    }

    #[test]
    fn test_central_bank_no_fees_by_default() {
        let bank = CentralBank::new();
        assert_eq!(bank.total_fees(Currency::Money), 0);
        assert_eq!(bank.total_fees(Currency::Token), 0);
    }

    // ── Ledger ─────────────────────────────────────────────

    #[test]
    fn test_ledger_record_and_list() {
        let mut ledger = Ledger::new();
        let id = ledger.record(
            Some("alice".into()),
            Some("bob".into()),
            100,
            Currency::Money,
            TransactionType::TaskReward,
            "test".into(),
            1,
            Some("task-1".into()),
        );
        assert_eq!(ledger.list().len(), 1);
        assert_eq!(ledger.list()[0].id, id);
        assert_eq!(ledger.list()[0].amount, 100);
    }

    #[test]
    fn test_ledger_list_by_agent() {
        let mut ledger = Ledger::new();
        ledger.record(Some("alice".into()), Some("bob".into()), 100, Currency::Money, TransactionType::TaskReward, "t1".into(), 1, None);
        ledger.record(Some("carol".into()), Some("alice".into()), 200, Currency::Money, TransactionType::TaskReward, "t2".into(), 1, None);
        ledger.record(Some("bob".into()), Some("carol".into()), 50, Currency::Token, TransactionType::Teach, "t3".into(), 1, None);

        let alice_entries = ledger.list_by_agent("alice");
        assert_eq!(alice_entries.len(), 2);
        let bob_entries = ledger.list_by_agent("bob");
        assert_eq!(bob_entries.len(), 2);
    }

    #[test]
    fn test_ledger_list_by_reference() {
        let mut ledger = Ledger::new();
        ledger.record(Some("a".into()), Some("b".into()), 100, Currency::Money, TransactionType::TaskReward, "r1".into(), 1, Some("task-1".into()));
        ledger.record(Some("b".into()), None, 2, Currency::Money, TransactionType::PlatformFee, "r1-fee".into(), 1, Some("task-1".into()));
        ledger.record(Some("c".into()), Some("d".into()), 50, Currency::Money, TransactionType::TaskReward, "r2".into(), 1, Some("task-2".into()));

        let task1_entries = ledger.list_by_reference("task-1");
        assert_eq!(task1_entries.len(), 2);
        let task2_entries = ledger.list_by_reference("task-2");
        assert_eq!(task2_entries.len(), 1);
    }

    #[test]
    fn test_ledger_list_by_type() {
        let mut ledger = Ledger::new();
        ledger.record(Some("a".into()), Some("b".into()), 100, Currency::Money, TransactionType::TaskReward, "r1".into(), 1, None);
        ledger.record(Some("b".into()), None, 2, Currency::Money, TransactionType::PlatformFee, "r2".into(), 1, None);
        ledger.record(Some("c".into()), Some("d".into()), 50, Currency::Money, TransactionType::TaskReward, "r3".into(), 1, None);

        assert_eq!(ledger.list_by_type(TransactionType::TaskReward).len(), 2);
        assert_eq!(ledger.list_by_type(TransactionType::PlatformFee).len(), 1);
        assert_eq!(ledger.list_by_type(TransactionType::Exchange).len(), 0);
    }

    #[test]
    fn test_ledger_empty() {
        let ledger = Ledger::new();
        assert!(ledger.list().is_empty());
        assert!(ledger.list_by_agent("nobody").is_empty());
    }

    // ── RewardDistributor ──────────────────────────────────

    #[test]
    fn test_distribute_reward_basic() {
        let mut dist = make_distributor();
        let result = dist.distribute_reward("task-1", "worker", 1000, Currency::Money, 10);

        // 2% of 1000 = 20 fee, net = 980
        assert_eq!(result.gross_reward, 1000);
        assert_eq!(result.platform_fee, 20);
        assert_eq!(result.net_reward, 980);
        assert_eq!(result.xp_awarded, 50);
        assert_eq!(result.reputation_change, 2.0);

        // Balance updated: 1000 + 980 = 1980
        assert_eq!(dist.get_balance("worker"), 1_980);
        // XP: 50
        assert_eq!(dist.get_experience("worker"), 50);
        // Reputation: 2.0
        assert_eq!(dist.get_reputation("worker"), 2.0);
        // Central bank: 20
        assert_eq!(dist.central_bank().total_fees(Currency::Money), 20);
    }

    #[test]
    fn test_distribute_reward_multiple_tasks() {
        let mut dist = make_distributor();
        dist.distribute_reward("task-1", "worker", 1000, Currency::Money, 10);
        dist.distribute_reward("task-2", "worker", 500, Currency::Money, 20);

        // Balance: 1000 + 980 + 490 = 2470
        assert_eq!(dist.get_balance("worker"), 2_470);
        // XP: 50 + 50 = 100
        assert_eq!(dist.get_experience("worker"), 100);
        // Reputation: 2.0 + 2.0 = 4.0
        assert_eq!(dist.get_reputation("worker"), 4.0);
        // Central bank: 20 + 10 = 30
        assert_eq!(dist.central_bank().total_fees(Currency::Money), 30);
    }

    #[test]
    fn test_distribute_reward_zero_reward() {
        let mut dist = make_distributor();
        let result = dist.distribute_reward("task-free", "worker", 0, Currency::Money, 10);

        assert_eq!(result.gross_reward, 0);
        assert_eq!(result.platform_fee, 0);
        assert_eq!(result.net_reward, 0);
        // XP and reputation still awarded
        assert_eq!(result.xp_awarded, 50);
        assert_eq!(result.reputation_change, 2.0);
        assert_eq!(dist.get_balance("worker"), 1_000);
        assert_eq!(dist.get_experience("worker"), 50);
        assert_eq!(dist.get_reputation("worker"), 2.0);
    }

    #[test]
    fn test_distribute_reward_small_amount() {
        let mut dist = make_distributor();
        // 2% of 10 = 0.2 → truncates to 0
        let result = dist.distribute_reward("task-small", "worker", 10, Currency::Money, 10);
        assert_eq!(result.platform_fee, 0);
        assert_eq!(result.net_reward, 10);
        assert_eq!(dist.get_balance("worker"), 1_010);
    }

    #[test]
    fn test_distribute_reward_creates_ledger_entries() {
        let mut dist = make_distributor();
        dist.distribute_reward("task-1", "worker", 1000, Currency::Money, 10);

        let entries = dist.ledger().list();
        assert_eq!(entries.len(), 2);

        // First entry: reward
        let reward_entry = &entries[0];
        assert_eq!(reward_entry.amount, 980);
        assert_eq!(reward_entry.tx_type, TransactionType::TaskReward);
        assert_eq!(reward_entry.to_agent.as_deref(), Some("worker"));
        assert_eq!(reward_entry.reference_id.as_deref(), Some("task-1"));
        assert_eq!(reward_entry.tick, 10);

        // Second entry: fee
        let fee_entry = &entries[1];
        assert_eq!(fee_entry.amount, 20);
        assert_eq!(fee_entry.tx_type, TransactionType::PlatformFee);
        assert_eq!(fee_entry.from_agent.as_deref(), Some("worker"));
        assert_eq!(fee_entry.to_agent, None);
        assert_eq!(fee_entry.reference_id.as_deref(), Some("task-1"));
    }

    #[test]
    fn test_distribute_reward_ledger_query_by_task() {
        let mut dist = make_distributor();
        dist.distribute_reward("task-1", "worker", 1000, Currency::Money, 10);
        dist.distribute_reward("task-2", "worker", 500, Currency::Money, 20);

        let task1 = dist.ledger().list_by_reference("task-1");
        assert_eq!(task1.len(), 2);
        let task2 = dist.ledger().list_by_reference("task-2");
        assert_eq!(task2.len(), 2);
    }

    #[test]
    fn test_distribute_different_agents() {
        let mut dist = make_distributor();
        dist.set_balance("worker2", 500);

        dist.distribute_reward("task-1", "worker", 1000, Currency::Money, 10);
        dist.distribute_reward("task-2", "worker2", 500, Currency::Money, 20);

        assert_eq!(dist.get_balance("worker"), 1_980);
        assert_eq!(dist.get_balance("worker2"), 990);
        assert_eq!(dist.get_reputation("worker"), 2.0);
        assert_eq!(dist.get_reputation("worker2"), 2.0);
    }

    #[test]
    fn test_distribute_reward_with_token_currency() {
        let mut dist = make_distributor();
        let result = dist.distribute_reward("task-tokens", "worker", 5000, Currency::Token, 5);

        assert_eq!(result.platform_fee, 100);
        assert_eq!(result.net_reward, 4900);
        assert_eq!(dist.get_balance("worker"), 5_900);
        assert_eq!(dist.central_bank().total_fees(Currency::Token), 100);
    }

    // ── Custom config ──────────────────────────────────────

    #[test]
    fn test_custom_config_higher_fee() {
        let config = RewardConfig {
            platform_fee_bps: 500, // 5%
            base_xp: 100,
            reputation_gain: 5.0,
        };
        let mut dist = RewardDistributor::new(config);
        dist.set_balance("worker", 0);

        let result = dist.distribute_reward("task-1", "worker", 1000, Currency::Money, 1);
        assert_eq!(result.platform_fee, 50);
        assert_eq!(result.net_reward, 950);
        assert_eq!(result.xp_awarded, 100);
        assert_eq!(result.reputation_change, 5.0);
    }

    #[test]
    fn test_custom_config_zero_fee() {
        let config = RewardConfig {
            platform_fee_bps: 0,
            base_xp: 10,
            reputation_gain: 1.0,
        };
        let mut dist = RewardDistributor::new(config);
        dist.set_balance("worker", 0);

        let result = dist.distribute_reward("task-1", "worker", 1000, Currency::Money, 1);
        assert_eq!(result.platform_fee, 0);
        assert_eq!(result.net_reward, 1000);
    }

    // ── Ledger entry serialization ─────────────────────────

    #[test]
    fn test_ledger_entry_serialization() {
        let entry = LedgerEntry {
            id: Uuid::new_v4(),
            from_agent: Some("alice".into()),
            to_agent: Some("bob".into()),
            amount: 100,
            currency: Currency::Money,
            tx_type: TransactionType::TaskReward,
            description: "Task reward".into(),
            tick: 42,
            reference_id: Some("task-1".into()),
        };
        let json = serde_json::to_string(&entry).unwrap();
        let back: LedgerEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(entry.id, back.id);
        assert_eq!(entry.from_agent, back.from_agent);
        assert_eq!(entry.to_agent, back.to_agent);
        assert_eq!(entry.amount, back.amount);
        assert_eq!(entry.tx_type, back.tx_type);
        assert_eq!(entry.tick, back.tick);
    }

    #[test]
    fn test_transaction_type_serialization() {
        for tx_type in [
            TransactionType::TaskReward,
            TransactionType::PlatformFee,
            TransactionType::EscrowRefund,
            TransactionType::Exchange,
            TransactionType::Interest,
            TransactionType::Knowledge,
            TransactionType::Teach,
        ] {
            let json = serde_json::to_string(&tx_type).unwrap();
            let back: TransactionType = serde_json::from_str(&json).unwrap();
            assert_eq!(tx_type, back);
        }
    }

    #[test]
    fn test_reward_distribution_serialization() {
        let dist = RewardDistribution {
            gross_reward: 1000,
            platform_fee: 20,
            net_reward: 980,
            xp_awarded: 50,
            reputation_change: 2.0,
            reward_ledger_id: Uuid::new_v4(),
            fee_ledger_id: Uuid::new_v4(),
        };
        let json = serde_json::to_string(&dist).unwrap();
        let back: RewardDistribution = serde_json::from_str(&json).unwrap();
        assert_eq!(dist.gross_reward, back.gross_reward);
        assert_eq!(dist.net_reward, back.net_reward);
        assert_eq!(dist.platform_fee, back.platform_fee);
    }

    // ── Overflow protection ──────────────────────────────────

    #[test]
    fn test_overflow_balance_saturates_at_max() {
        let mut dist = RewardDistributor::new(RewardConfig {
            platform_fee_bps: 0,
            base_xp: 0,
            reputation_gain: 0.0,
        });
        dist.set_balance("worker", u64::MAX);
        let result = dist.distribute_reward("task-1", "worker", 1, Currency::Money, 1);
        // Balance should saturate at u64::MAX, not overflow
        assert_eq!(dist.get_balance("worker"), u64::MAX);
        assert_eq!(result.net_reward, 1);
    }

    #[test]
    fn test_overflow_xp_saturates_at_max() {
        let mut dist = RewardDistributor::new(RewardConfig {
            platform_fee_bps: 0,
            base_xp: u64::MAX,
            reputation_gain: 0.0,
        });
        dist.set_balance("worker", 0);
        dist.distribute_reward("task-1", "worker", 0, Currency::Money, 1);
        // XP from first task = u64::MAX
        assert_eq!(dist.get_experience("worker"), u64::MAX);
        // Second task should saturate
        dist.distribute_reward("task-2", "worker", 0, Currency::Money, 1);
        assert_eq!(dist.get_experience("worker"), u64::MAX);
    }

    // ── New agent (no set_balance) ───────────────────────────

    #[test]
    fn test_distribute_reward_new_agent_no_prior_balance() {
        let mut dist = RewardDistributor::new(RewardConfig::default());
        let result = dist.distribute_reward("task-1", "newbie", 1000, Currency::Money, 1);
        // New agent starts with 0 balance, gets net reward
        assert_eq!(result.net_reward, 980);
        assert_eq!(dist.get_balance("newbie"), 980);
        assert_eq!(dist.get_experience("newbie"), 50);
        assert_eq!(dist.get_reputation("newbie"), 2.0);
    }
}
