use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::org::Organization;
use crate::world::event::WorldEvent;
use crate::world::state::EventBus;

// ── Constants ─────────────────────────────────────────────

/// Default income tax rate (10%).
pub const DEFAULT_INCOME_TAX_RATE: f64 = 0.10;
/// Default wealth tax rate (2%).
pub const DEFAULT_WEALTH_TAX_RATE: f64 = 0.02;
/// Default trade tax rate (5%).
pub const DEFAULT_TRADE_TAX_RATE: f64 = 0.05;
/// Maximum tax rate allowed (50%).
pub const MAX_TAX_RATE: f64 = 0.50;
/// Minimum tax rate allowed (0%).
pub const MIN_TAX_RATE: f64 = 0.00;

// ── Tax Types ─────────────────────────────────────────────

/// Types of taxes that can be levied by an organization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaxKind {
    /// Tax on income/earnings of members.
    IncomeTax,
    /// Tax on accumulated wealth of members.
    WealthTax,
    /// Tax on trade transactions conducted within the org.
    TradeTax,
}

impl std::fmt::Display for TaxKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaxKind::IncomeTax => write!(f, "income_tax"),
            TaxKind::WealthTax => write!(f, "wealth_tax"),
            TaxKind::TradeTax => write!(f, "trade_tax"),
        }
    }
}

// ── Distribution Strategies ───────────────────────────────

/// Strategy for distributing treasury funds to members.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DistributionStrategy {
    /// Equal split among all members.
    EqualDistribution,
    /// Proportional to each member's contribution/performance score.
    PerformanceBased,
    /// More funds go to members with fewer resources.
    NeedBased,
}

// ── Tax Configuration ─────────────────────────────────────

/// Per-tax-kind rate configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TaxConfig {
    /// Income tax rate (0.0–0.50).
    pub income_tax_rate: f64,
    /// Wealth tax rate (0.0–0.50).
    pub wealth_tax_rate: f64,
    /// Trade tax rate (0.0–0.50).
    pub trade_tax_rate: f64,
}

impl Default for TaxConfig {
    fn default() -> Self {
        Self {
            income_tax_rate: DEFAULT_INCOME_TAX_RATE,
            wealth_tax_rate: DEFAULT_WEALTH_TAX_RATE,
            trade_tax_rate: DEFAULT_TRADE_TAX_RATE,
        }
    }
}

impl TaxConfig {
    /// Create a new tax config with validated rates.
    pub fn new(income: f64, wealth: f64, trade: f64) -> Self {
        Self {
            income_tax_rate: clamp_rate(income),
            wealth_tax_rate: clamp_rate(wealth),
            trade_tax_rate: clamp_rate(trade),
        }
    }

    /// Get the rate for a specific tax kind.
    pub fn rate_for(&self, kind: TaxKind) -> f64 {
        match kind {
            TaxKind::IncomeTax => self.income_tax_rate,
            TaxKind::WealthTax => self.wealth_tax_rate,
            TaxKind::TradeTax => self.trade_tax_rate,
        }
    }

    /// Update the rate for a specific tax kind.
    pub fn set_rate(&mut self, kind: TaxKind, rate: f64) {
        let clamped = clamp_rate(rate);
        match kind {
            TaxKind::IncomeTax => self.income_tax_rate = clamped,
            TaxKind::WealthTax => self.wealth_tax_rate = clamped,
            TaxKind::TradeTax => self.trade_tax_rate = clamped,
        }
    }
}

/// Clamp a tax rate to the valid range [0.0, 0.50].
fn clamp_rate(rate: f64) -> f64 {
    rate.clamp(MIN_TAX_RATE, MAX_TAX_RATE)
}

// ── Tax Record ────────────────────────────────────────────

/// Record of a single tax collection event.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TaxRecord {
    /// Unique record ID.
    pub id: String,
    /// Organization ID.
    pub org_id: String,
    /// Agent who paid the tax.
    pub payer_id: String,
    /// Type of tax.
    pub tax_kind: TaxKind,
    /// Tax rate applied.
    pub rate: f64,
    /// Gross amount before tax.
    pub gross_amount: u64,
    /// Tax amount collected.
    pub tax_amount: u64,
    /// Tick when the tax was collected.
    pub tick: u64,
}

/// Record of a treasury distribution event.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DistributionRecord {
    /// Unique record ID.
    pub id: String,
    /// Organization ID.
    pub org_id: String,
    /// Distribution strategy used.
    pub strategy: DistributionStrategy,
    /// Total amount distributed.
    pub total_amount: u64,
    /// Per-member allocation: (agent_id, amount).
    pub allocations: Vec<(String, u64)>,
    /// Tick when the distribution occurred.
    pub tick: u64,
}

// ── Error Type ────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum TreasuryError {
    #[error("agent {0} is not a member of this organization")]
    NotMember(String),
    #[error("organization is dissolved")]
    OrgDissolved,
    #[error("insufficient treasury balance: have {0}, need {1}")]
    InsufficientBalance(u64, u64),
    #[error("no members to distribute to")]
    NoMembers,
    #[error("tax rate {0} exceeds maximum {1}")]
    RateExceedsMax(f64, f64),
    #[error("amount cannot be zero")]
    ZeroAmount,
    #[error("performance scores required for PerformanceBased distribution")]
    PerformanceScoresRequired,
    #[error("wealth data required for NeedBased distribution")]
    WealthDataRequired,
}

// ── Treasury Engine ───────────────────────────────────────

/// Manages tax collection and resource distribution for organizations.
///
/// Each `Treasury` instance is scoped to a single organization. It tracks
/// tax collection history, current balance, and distribution records.
pub struct Treasury {
    /// Organization ID this treasury belongs to.
    org_id: String,
    /// Tax rate configuration.
    tax_config: TaxConfig,
    /// Distribution strategy.
    distribution_strategy: DistributionStrategy,
    /// History of all tax collections.
    tax_history: Vec<TaxRecord>,
    /// History of all distributions.
    distribution_history: Vec<DistributionRecord>,
    /// Event bus for emitting events.
    event_bus: Option<EventBus>,
    /// Monotonic counter for record IDs.
    next_record_id: u64,
}

impl Treasury {
    /// Create a new treasury for an organization.
    pub fn new(org_id: String) -> Self {
        Self {
            org_id,
            tax_config: TaxConfig::default(),
            distribution_strategy: DistributionStrategy::EqualDistribution,
            tax_history: Vec::new(),
            distribution_history: Vec::new(),
            event_bus: None,
            next_record_id: 1,
        }
    }

    /// Create a treasury wired to an EventBus.
    pub fn with_event_bus(org_id: String, event_bus: EventBus) -> Self {
        Self {
            org_id,
            tax_config: TaxConfig::default(),
            distribution_strategy: DistributionStrategy::EqualDistribution,
            tax_history: Vec::new(),
            distribution_history: Vec::new(),
            event_bus: Some(event_bus),
            next_record_id: 1,
        }
    }

    /// Create a treasury with custom tax configuration.
    pub fn with_config(
        org_id: String,
        tax_config: TaxConfig,
        strategy: DistributionStrategy,
    ) -> Self {
        Self {
            org_id,
            tax_config,
            distribution_strategy: strategy,
            tax_history: Vec::new(),
            distribution_history: Vec::new(),
            event_bus: None,
            next_record_id: 1,
        }
    }

    // ── Accessors ─────────────────────────────────────────

    /// Get the organization ID.
    pub fn org_id(&self) -> &str {
        &self.org_id
    }

    /// Get the current tax configuration.
    pub fn tax_config(&self) -> &TaxConfig {
        &self.tax_config
    }

    /// Get a mutable reference to the tax configuration.
    pub fn tax_config_mut(&mut self) -> &mut TaxConfig {
        &mut self.tax_config
    }

    /// Get the current distribution strategy.
    pub fn distribution_strategy(&self) -> DistributionStrategy {
        self.distribution_strategy
    }

    /// Set the distribution strategy.
    pub fn set_distribution_strategy(&mut self, strategy: DistributionStrategy) {
        self.distribution_strategy = strategy;
    }

    // ── Tax Collection ────────────────────────────────────

    /// Collect tax from an agent and add it to the organization's treasury.
    ///
    /// Returns the `TaxRecord` for the collection, or an error if the
    /// agent is not a member or the org is dissolved.
    pub fn collect_tax(
        &mut self,
        org: &mut Organization,
        payer_id: &str,
        tax_kind: TaxKind,
        gross_amount: u64,
        tick: u64,
    ) -> Result<TaxRecord, TreasuryError> {
        if gross_amount == 0 {
            return Err(TreasuryError::ZeroAmount);
        }
        if org.status == super::org::OrgStatus::Dissolved {
            return Err(TreasuryError::OrgDissolved);
        }
        if !org.is_member(payer_id) {
            return Err(TreasuryError::NotMember(payer_id.to_string()));
        }

        let rate = self.tax_config.rate_for(tax_kind);
        let tax_amount = ((gross_amount as f64) * rate).floor() as u64;
        let tax_amount = tax_amount.max(1); // minimum 1 unit

        let record = TaxRecord {
            id: format!("tax-{}", self.next_record_id),
            org_id: self.org_id.clone(),
            payer_id: payer_id.to_string(),
            tax_kind,
            rate,
            gross_amount,
            tax_amount,
            tick,
        };
        self.next_record_id += 1;

        // Add to org treasury
        org.treasury += tax_amount;
        org.touch_activity(tick);

        self.emit_event(WorldEvent::TaxCollected {
            org_id: self.org_id.clone(),
            payer_id: payer_id.to_string(),
            tax_kind: format!("{}", tax_kind),
            rate,
            gross_amount,
            tax_amount,
            tick,
        });

        self.tax_history.push(record.clone());
        Ok(record)
    }

    // ── Distribution ──────────────────────────────────────

    /// Distribute treasury funds to members using the configured strategy.
    ///
    /// `performance_scores` is used only for `PerformanceBased` strategy.
    /// `wealth_data` is used only for `NeedBased` strategy.
    /// Returns the `DistributionRecord` or an error.
    pub fn distribute(
        &mut self,
        org: &mut Organization,
        amount: u64,
        tick: u64,
        performance_scores: Option<&HashMap<String, f64>>,
        wealth_data: Option<&HashMap<String, u64>>,
    ) -> Result<DistributionRecord, TreasuryError> {
        if amount == 0 {
            return Err(TreasuryError::ZeroAmount);
        }
        if org.members.is_empty() {
            return Err(TreasuryError::NoMembers);
        }
        if org.status == super::org::OrgStatus::Dissolved {
            return Err(TreasuryError::OrgDissolved);
        }
        if org.treasury < amount {
            return Err(TreasuryError::InsufficientBalance(org.treasury, amount));
        }

        let allocations = match self.distribution_strategy {
            DistributionStrategy::EqualDistribution => Self::distribute_equal(&org.members, amount),
            DistributionStrategy::PerformanceBased => {
                let scores = performance_scores.ok_or(TreasuryError::PerformanceScoresRequired)?;
                Self::distribute_performance(&org.members, scores, amount)
            }
            DistributionStrategy::NeedBased => {
                let wealth = wealth_data.ok_or(TreasuryError::WealthDataRequired)?;
                Self::distribute_need(&org.members, wealth, amount)
            }
        };

        // Deduct from org treasury
        org.treasury -= amount;
        org.touch_activity(tick);

        let record = DistributionRecord {
            id: format!("dist-{}", self.next_record_id),
            org_id: self.org_id.clone(),
            strategy: self.distribution_strategy,
            total_amount: amount,
            allocations: allocations.clone(),
            tick,
        };
        self.next_record_id += 1;

        self.emit_event(WorldEvent::TreasuryDistributed {
            org_id: self.org_id.clone(),
            strategy: format!("{}", self.distribution_strategy),
            total_amount: amount,
            allocations: allocations.clone(),
            tick,
        });

        self.distribution_history.push(record.clone());
        Ok(record)
    }

    // ── Balance & History ─────────────────────────────────

    /// Get the current treasury balance (delegates to org).
    pub fn get_balance(&self, org: &Organization) -> u64 {
        org.treasury
    }

    /// Get the full tax collection history.
    pub fn get_tax_history(&self) -> &[TaxRecord] {
        &self.tax_history
    }

    /// Get the full distribution history.
    pub fn get_distribution_history(&self) -> &[DistributionRecord] {
        &self.distribution_history
    }

    /// Get tax history filtered by agent.
    pub fn get_tax_history_for_agent(&self, agent_id: &str) -> Vec<&TaxRecord> {
        self.tax_history
            .iter()
            .filter(|r| r.payer_id == agent_id)
            .collect()
    }

    /// Get total tax collected.
    pub fn total_tax_collected(&self) -> u64 {
        self.tax_history.iter().map(|r| r.tax_amount).sum()
    }

    /// Get total amount distributed.
    pub fn total_distributed(&self) -> u64 {
        self.distribution_history
            .iter()
            .map(|r| r.total_amount)
            .sum()
    }

    // ── Distribution Helpers ──────────────────────────────

    /// Equal distribution: split amount evenly among all members.
    fn distribute_equal(members: &[super::members::OrgMember], amount: u64) -> Vec<(String, u64)> {
        if members.is_empty() {
            return vec![];
        }
        let per_member = amount / members.len() as u64;
        let remainder = amount % members.len() as u64;

        members
            .iter()
            .enumerate()
            .map(|(i, m)| {
                let extra = if (i as u64) < remainder { 1 } else { 0 };
                (m.agent_id.clone(), per_member + extra)
            })
            .collect()
    }

    /// Performance-based distribution: proportional to performance scores.
    fn distribute_performance(
        members: &[super::members::OrgMember],
        scores: &HashMap<String, f64>,
        amount: u64,
    ) -> Vec<(String, u64)> {
        let total_score: f64 = members
            .iter()
            .map(|m| scores.get(&m.agent_id).copied().unwrap_or(0.0))
            .sum();

        if total_score <= 0.0 {
            // Fallback to equal if no scores
            return Self::distribute_equal(members, amount);
        }

        let mut allocations = Vec::with_capacity(members.len());
        let mut allocated = 0u64;

        for (i, member) in members.iter().enumerate() {
            let score = scores.get(&member.agent_id).copied().unwrap_or(0.0);
            let share = if i == members.len() - 1 {
                // Last member gets the remainder to avoid rounding loss
                amount - allocated
            } else {
                ((score / total_score) * amount as f64).floor() as u64
            };
            allocated += share;
            allocations.push((member.agent_id.clone(), share));
        }

        allocations
    }

    /// Need-based distribution: more to members with lower wealth.
    fn distribute_need(
        members: &[super::members::OrgMember],
        wealth_data: &HashMap<String, u64>,
        amount: u64,
    ) -> Vec<(String, u64)> {
        // Weight = 1 / (1 + wealth). Members with less wealth get higher weight.
        let weights: Vec<f64> = members
            .iter()
            .map(|m| {
                let w = wealth_data.get(&m.agent_id).copied().unwrap_or(0) as f64;
                1.0 / (1.0 + w)
            })
            .collect();

        let total_weight: f64 = weights.iter().sum();

        if total_weight <= 0.0 {
            return Self::distribute_equal(members, amount);
        }

        let mut allocations = Vec::with_capacity(members.len());
        let mut allocated = 0u64;

        for (i, member) in members.iter().enumerate() {
            let share = if i == members.len() - 1 {
                amount - allocated
            } else {
                ((weights[i] / total_weight) * amount as f64).floor() as u64
            };
            allocated += share;
            allocations.push((member.agent_id.clone(), share));
        }

        allocations
    }

    // ── Event Emission ────────────────────────────────────

    fn emit_event(&self, event: WorldEvent) {
        if let Some(ref bus) = self.event_bus {
            bus.emit(event);
        }
    }
}

impl std::fmt::Display for DistributionStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DistributionStrategy::EqualDistribution => write!(f, "equal_distribution"),
            DistributionStrategy::PerformanceBased => write!(f, "performance_based"),
            DistributionStrategy::NeedBased => write!(f, "need_based"),
        }
    }
}

// ── Tests ─────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::organization::charter::{Charter, GovernanceModel, ProfitSharing};
    use crate::organization::members::{MemberRole, OrgMember};
    use crate::organization::org::{OrgStatus, OrgType};
    use uuid::Uuid;

    fn test_charter() -> Charter {
        Charter {
            purpose: "Test org".to_string(),
            governance: GovernanceModel::Vote,
            profit_sharing: ProfitSharing::Equal,
            membership_fee: 0,
        }
    }

    fn make_org_with_treasury(treasury: u64) -> Organization {
        let id = Uuid::new_v4().to_string();
        Organization {
            id: id.clone(),
            name: "TestOrg".to_string(),
            org_type: OrgType::Company,
            charter: test_charter(),
            treasury,
            debts: 0,
            status: OrgStatus::Active,
            created_tick: 100,
            last_activity_tick: 100,
            members: vec![
                OrgMember {
                    agent_id: "agent-0".to_string(),
                    agent_name: "Alice".to_string(),
                    role: MemberRole::Founder,
                    share: 0.5,
                    joined_tick: 100,
                },
                OrgMember {
                    agent_id: "agent-1".to_string(),
                    agent_name: "Bob".to_string(),
                    role: MemberRole::Founder,
                    share: 0.5,
                    joined_tick: 100,
                },
                OrgMember {
                    agent_id: "agent-2".to_string(),
                    agent_name: "Carol".to_string(),
                    role: MemberRole::Member,
                    share: 0.0,
                    joined_tick: 100,
                },
            ],
        }
    }

    // ── Tax Collection Tests ──────────────────────────────

    #[test]
    fn test_collect_income_tax() {
        let mut treasury = Treasury::new("org-1".to_string());
        let mut org = make_org_with_treasury(500);

        let record = treasury
            .collect_tax(&mut org, "agent-0", TaxKind::IncomeTax, 1000, 200)
            .unwrap();

        assert_eq!(record.payer_id, "agent-0");
        assert_eq!(record.tax_kind, TaxKind::IncomeTax);
        assert!((record.rate - DEFAULT_INCOME_TAX_RATE).abs() < 0.001);
        // tax = 1000 * 0.10 = 100
        assert_eq!(record.tax_amount, 100);
        assert_eq!(record.gross_amount, 1000);
        // Org treasury: 500 + 100 = 600
        assert_eq!(org.treasury, 600);
    }

    #[test]
    fn test_collect_wealth_tax() {
        let mut treasury = Treasury::new("org-1".to_string());
        let mut org = make_org_with_treasury(500);

        let record = treasury
            .collect_tax(&mut org, "agent-1", TaxKind::WealthTax, 5000, 200)
            .unwrap();

        assert_eq!(record.tax_kind, TaxKind::WealthTax);
        assert!((record.rate - DEFAULT_WEALTH_TAX_RATE).abs() < 0.001);
        // tax = 5000 * 0.02 = 100
        assert_eq!(record.tax_amount, 100);
    }

    #[test]
    fn test_collect_trade_tax() {
        let mut treasury = Treasury::new("org-1".to_string());
        let mut org = make_org_with_treasury(500);

        let record = treasury
            .collect_tax(&mut org, "agent-2", TaxKind::TradeTax, 200, 200)
            .unwrap();

        assert_eq!(record.tax_kind, TaxKind::TradeTax);
        assert!((record.rate - DEFAULT_TRADE_TAX_RATE).abs() < 0.001);
        // tax = 200 * 0.05 = 10
        assert_eq!(record.tax_amount, 10);
    }

    #[test]
    fn test_collect_tax_rejects_non_member() {
        let mut treasury = Treasury::new("org-1".to_string());
        let mut org = make_org_with_treasury(500);

        let result = treasury.collect_tax(&mut org, "agent-999", TaxKind::IncomeTax, 1000, 200);
        assert!(result.is_err());
        match result.unwrap_err() {
            TreasuryError::NotMember(id) => assert_eq!(id, "agent-999"),
            e => panic!("expected NotMember, got {:?}", e),
        }
    }

    #[test]
    fn test_collect_tax_rejects_dissolved_org() {
        let mut treasury = Treasury::new("org-1".to_string());
        let mut org = make_org_with_treasury(500);
        org.status = OrgStatus::Dissolved;

        let result = treasury.collect_tax(&mut org, "agent-0", TaxKind::IncomeTax, 1000, 200);
        assert!(matches!(result.unwrap_err(), TreasuryError::OrgDissolved));
    }

    #[test]
    fn test_collect_tax_rejects_zero_amount() {
        let mut treasury = Treasury::new("org-1".to_string());
        let mut org = make_org_with_treasury(500);

        let result = treasury.collect_tax(&mut org, "agent-0", TaxKind::IncomeTax, 0, 200);
        assert!(matches!(result.unwrap_err(), TreasuryError::ZeroAmount));
    }

    // ── Distribution Tests ────────────────────────────────

    #[test]
    fn test_distribute_equal() {
        let mut treasury = Treasury::new("org-1".to_string());
        let mut org = make_org_with_treasury(300);

        let record = treasury.distribute(&mut org, 300, 200, None, None).unwrap();

        assert_eq!(record.total_amount, 300);
        assert_eq!(record.strategy, DistributionStrategy::EqualDistribution);
        assert_eq!(record.allocations.len(), 3);
        // 300 / 3 = 100 each
        for (_, amount) in &record.allocations {
            assert_eq!(*amount, 100);
        }
        // Treasury depleted
        assert_eq!(org.treasury, 0);
    }

    #[test]
    fn test_distribute_equal_with_remainder() {
        let mut treasury = Treasury::new("org-1".to_string());
        let mut org = make_org_with_treasury(100);

        let record = treasury.distribute(&mut org, 100, 200, None, None).unwrap();

        // 100 / 3 = 33 each, remainder 1 → first member gets 34
        let total: u64 = record.allocations.iter().map(|(_, a)| *a).sum();
        assert_eq!(total, 100);
    }

    #[test]
    fn test_distribute_performance_based() {
        let mut treasury = Treasury::with_config(
            "org-1".to_string(),
            TaxConfig::default(),
            DistributionStrategy::PerformanceBased,
        );
        let mut org = make_org_with_treasury(300);

        let scores = HashMap::from([
            ("agent-0".to_string(), 80.0),
            ("agent-1".to_string(), 20.0),
            ("agent-2".to_string(), 0.0),
        ]);

        let record = treasury
            .distribute(&mut org, 300, 200, Some(&scores), None)
            .unwrap();

        assert_eq!(record.strategy, DistributionStrategy::PerformanceBased);
        // agent-0 gets 240, agent-1 gets 60, agent-2 gets 0
        let allocations: HashMap<String, u64> = record.allocations.into_iter().collect();
        assert_eq!(*allocations.get("agent-0").unwrap(), 240);
        assert_eq!(*allocations.get("agent-1").unwrap(), 60);
        assert_eq!(*allocations.get("agent-2").unwrap(), 0);
    }

    #[test]
    fn test_distribute_need_based() {
        let mut treasury = Treasury::with_config(
            "org-1".to_string(),
            TaxConfig::default(),
            DistributionStrategy::NeedBased,
        );
        let mut org = make_org_with_treasury(300);

        let wealth = HashMap::from([
            ("agent-0".to_string(), 1000), // rich → gets less
            ("agent-1".to_string(), 100),  // moderate
            ("agent-2".to_string(), 0),    // poor → gets more
        ]);

        let record = treasury
            .distribute(&mut org, 300, 200, None, Some(&wealth))
            .unwrap();

        assert_eq!(record.strategy, DistributionStrategy::NeedBased);
        // agent-2 (wealth=0) should get the most, agent-0 (wealth=1000) the least
        let allocations: HashMap<String, u64> = record.allocations.into_iter().collect();
        assert!(
            allocations.get("agent-2").unwrap() > allocations.get("agent-0").unwrap(),
            "need-based should give more to poorer members"
        );
    }

    #[test]
    fn test_distribute_rejects_insufficient_balance() {
        let mut treasury = Treasury::new("org-1".to_string());
        let mut org = make_org_with_treasury(50);

        let result = treasury.distribute(&mut org, 100, 200, None, None);
        assert!(matches!(
            result.unwrap_err(),
            TreasuryError::InsufficientBalance(50, 100)
        ));
    }

    #[test]
    fn test_distribute_rejects_no_members() {
        let mut treasury = Treasury::new("org-1".to_string());
        let mut org = make_org_with_treasury(300);
        org.members.clear();

        let result = treasury.distribute(&mut org, 300, 200, None, None);
        assert!(matches!(result.unwrap_err(), TreasuryError::NoMembers));
    }

    // ── History & Balance Tests ───────────────────────────

    #[test]
    fn test_get_balance() {
        let treasury = Treasury::new("org-1".to_string());
        let org = make_org_with_treasury(500);
        assert_eq!(treasury.get_balance(&org), 500);
    }

    #[test]
    fn test_tax_history() {
        let mut treasury = Treasury::new("org-1".to_string());
        let mut org = make_org_with_treasury(500);

        treasury
            .collect_tax(&mut org, "agent-0", TaxKind::IncomeTax, 1000, 200)
            .unwrap();
        treasury
            .collect_tax(&mut org, "agent-1", TaxKind::TradeTax, 500, 210)
            .unwrap();

        assert_eq!(treasury.get_tax_history().len(), 2);
        assert_eq!(treasury.get_tax_history_for_agent("agent-0").len(), 1);
        assert_eq!(treasury.get_tax_history_for_agent("agent-1").len(), 1);
        assert!(treasury.get_tax_history_for_agent("agent-2").is_empty());

        // total_tax_collected: 100 + 25 = 125
        assert_eq!(treasury.total_tax_collected(), 125);
    }

    #[test]
    fn test_distribution_history() {
        let mut treasury = Treasury::new("org-1".to_string());
        let mut org = make_org_with_treasury(600);

        treasury.distribute(&mut org, 300, 200, None, None).unwrap();
        treasury.distribute(&mut org, 200, 210, None, None).unwrap();

        assert_eq!(treasury.get_distribution_history().len(), 2);
        assert_eq!(treasury.total_distributed(), 500);
    }

    // ── Tax Config Tests ──────────────────────────────────

    #[test]
    fn test_custom_tax_config() {
        let config = TaxConfig::new(0.20, 0.05, 0.15);
        assert!((config.income_tax_rate - 0.20).abs() < 0.001);
        assert!((config.wealth_tax_rate - 0.05).abs() < 0.001);
        assert!((config.trade_tax_rate - 0.15).abs() < 0.001);
    }

    #[test]
    fn test_tax_config_clamps_to_max() {
        let config = TaxConfig::new(0.80, 0.60, 0.55);
        assert!((config.income_tax_rate - MAX_TAX_RATE).abs() < 0.001);
        assert!((config.wealth_tax_rate - MAX_TAX_RATE).abs() < 0.001);
        assert!((config.trade_tax_rate - MAX_TAX_RATE).abs() < 0.001);
    }

    #[test]
    fn test_set_rate_per_kind() {
        let mut config = TaxConfig::default();
        config.set_rate(TaxKind::IncomeTax, 0.25);
        config.set_rate(TaxKind::WealthTax, 0.10);
        assert!((config.rate_for(TaxKind::IncomeTax) - 0.25).abs() < 0.001);
        assert!((config.rate_for(TaxKind::WealthTax) - 0.10).abs() < 0.001);
    }

    // ── Full Workflow Test ────────────────────────────────

    #[test]
    fn test_full_treasury_workflow() {
        let mut treasury = Treasury::new("org-1".to_string());
        let mut org = make_org_with_treasury(100);

        // Collect taxes
        treasury
            .collect_tax(&mut org, "agent-0", TaxKind::IncomeTax, 1000, 200)
            .unwrap();
        treasury
            .collect_tax(&mut org, "agent-1", TaxKind::IncomeTax, 1000, 200)
            .unwrap();
        treasury
            .collect_tax(&mut org, "agent-2", TaxKind::TradeTax, 500, 200)
            .unwrap();

        // 100 + 100 + 100 + 25 = 325
        assert_eq!(org.treasury, 325);
        assert_eq!(treasury.total_tax_collected(), 225);

        // Distribute
        treasury.set_distribution_strategy(DistributionStrategy::EqualDistribution);
        let record = treasury.distribute(&mut org, 300, 210, None, None).unwrap();
        assert_eq!(record.allocations.len(), 3);

        // 325 - 300 = 25 remaining
        assert_eq!(org.treasury, 25);
        assert_eq!(treasury.total_distributed(), 300);

        // History check
        assert_eq!(treasury.get_tax_history().len(), 3);
        assert_eq!(treasury.get_distribution_history().len(), 1);
    }
}
