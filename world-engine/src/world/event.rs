use serde::{Deserialize, Serialize};

use super::enums::{AgentPhase, Currency, DeathReason};

/// Type of trust interaction between agents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrustInteractionType {
    Cooperation,
    Betrayal,
    TradeCompleted,
    TaskCompleted,
    Gift,
    Attack,
}

/// Discriminant for filtering events by kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EventType {
    TickAdvanced,
    AgentSpawned,
    AgentDying,
    AgentDied,
    AgentRescued,
    TransactionCompleted,
    BalanceChanged,
    PhaseChanged,
    RuleViolated,
    SnapshotTaken,
    EscrowCreated,
    EscrowClaimed,
    EscrowReleased,
    EscrowRefunded,
    EscrowFrozen,
    TaskCreated,
    TaskClaimed,
    TaskStarted,
    TaskSubmitted,
    TaskReviewed,
    TaskCompleted,
    TaskExpired,
    RewardDistributed,
    AgentRegistered,
    AgentDeregistered,
    AgentHeartbeat,
    ReputationChanged,
    ConfigReloaded,
    KnowledgeListed,
    KnowledgeDelisted,
    KnowledgePurchased,
    KnowledgeRated,
    TrustChanged,
    TrustInteraction,
    MentorshipEstablished,
    MentorshipProgress,
    MentorshipCompleted,
    WillCreated,
    InheritanceTriggered,
    TimeCapsuleBriefing,
    OrgCreated,
    OrgMemberJoined,
    OrgMemberLeft,
    OrgDissolved,
    OrgInactivated,
    StockIssued,
    StockIpo,
    StockTraded,
    StockTransferred,
    StockDividend,
    OrganizationCreated,
    OrganizationDissolved,
    OrganizationMemberJoined,
    OrganizationMemberLeft,
    ProposalCreated,
    ProposalVotingStarted,
    ProposalVoted,
    ProposalExecuted,
    ProposalRejected,
    BankAccountOpened,
    BankDeposit,
    BankWithdrawal,
    LoanApplied,
    LoanApproved,
    LoanDisbursed,
    LoanRepayment,
    BankRateAdjusted,
    MoneyMinted,
    BadDebtWrittenOff,
    SkillLevelUp,
    SkillMutated,
    FitnessEvaluated,
    OrgResourceConflict,
    OrgTerritoryClaimed,
    OrgFormationSuggested,
    // Treasury events
    TaxCollected,
    TreasuryDistributed,
    // Leadership events
    LeadershipElectionStarted,
    LeadershipChanged,
    // Diplomacy events
    TreatyProposed,
    TreatySigned,
    TreatyBroken,
    RelationChanged,
    // Offspring mutation events
    OffspringMutated,
}

/// Events emitted by the world engine.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
#[non_exhaustive]
pub enum WorldEvent {
    TickAdvanced { tick: u64 },
    AgentSpawned { agent_id: String, name: String },
    AgentDying { agent_id: String, reason: DeathReason, grace_ticks: u64 },
    AgentDied { agent_id: String, reason: DeathReason },
    AgentRescued { agent_id: String },
    TransactionCompleted { from: String, to: String, amount: u64, currency: Currency },
    BalanceChanged { agent_id: String, currency: Currency, old_balance: u64, new_balance: u64 },
    PhaseChanged { agent_id: String, old_phase: AgentPhase, new_phase: AgentPhase },
    RuleViolated { agent_id: String, rule: String, details: String },
    SnapshotTaken { tick: u64, path: String },
    EscrowCreated { escrow_id: String, publisher: String, reward: u64, currency: Currency },
    EscrowClaimed { escrow_id: String, claimant: String, deposit: u64 },
    EscrowReleased { escrow_id: String, recipient: String, amount: u64, currency: Currency },
    EscrowRefunded { escrow_id: String, recipient: String, amount: u64, currency: Currency },
    EscrowFrozen { escrow_id: String, reason: String },
    TaskCreated { task_id: String, publisher: String, reward: u64 },
    TaskClaimed { task_id: String, assignee: String },
    TaskStarted { task_id: String },
    TaskSubmitted { task_id: String },
    TaskReviewed { task_id: String, approved: bool },
    TaskCompleted { task_id: String },
    TaskExpired { task_id: String },
    RewardDistributed {
        task_id: String,
        assignee_id: String,
        gross_reward: u64,
        net_reward: u64,
        platform_fee: u64,
        xp_awarded: u64,
        reputation_change: f64,
    },
    ReputationChanged { agent_id: String, old_reputation: f64, new_reputation: f64, reason: String },
    AgentRegistered { agent_id: String, name: String },
    AgentDeregistered { agent_id: String, name: String },
    AgentHeartbeat { agent_id: String, timestamp: u64 },
    ConfigReloaded { source: String },
    KnowledgeListed { listing_id: String, publisher: String, price: u64, currency: Currency },
    KnowledgeDelisted { listing_id: String },
    KnowledgePurchased { listing_id: String, buyer: String, seller: String, price: u64, currency: Currency },
    KnowledgeRated { listing_id: String, rater: String, score: u8, average_rating: f64 },
    TrustChanged { agent_id: String, other_agent_id: String, old_trust: f64, new_trust: f64, reason: String },
    TrustInteraction { from: String, to: String, interaction: TrustInteractionType },
    MentorshipEstablished { mentor_id: String, apprentice_id: String, skill: String },
    MentorshipProgress { mentor_id: String, apprentice_id: String, skill: String, level_gained: u32 },
    MentorshipCompleted { mentor_id: String, apprentice_id: String, skill: String, final_level: u32 },
    WillCreated { agent_id: String, beneficiaries_count: usize },
    InheritanceTriggered { deceased_id: String, beneficiary_id: String, tokens_transferred: u64, skills_transferred: u32 },
    TimeCapsuleBriefing { tick: u64, summary: String },
    OrgCreated { org_id: String, name: String, org_type: String, founder_count: usize },
    OrgMemberJoined { org_id: String, agent_id: String, agent_name: String, role: String, total_members: usize },
    OrgMemberLeft { org_id: String, agent_id: String, remaining_members: usize },
    OrgDissolved { org_id: String, reason: String },
    OrgInactivated { org_id: String, inactive_since: u64, current_tick: u64 },
    StockIssued { stock_id: String, org_id: String, ticker: String, total_shares: u64, price: u64 },
    StockIpo { stock_id: String, org_id: String, ticker: String, price: u64, total_shares: u64 },
    StockTraded { trade_id: String, stock_id: String, buyer_id: String, seller_id: String, price: u64, quantity: u64, fee: u64 },
    StockTransferred { stock_id: String, from_agent: String, to_agent: String, quantity: u64 },
    StockDividend { dividend_id: String, stock_id: String, org_id: String, total_profit: u64, dividend_per_share: u64, recipient_count: usize },
    OrganizationCreated { org_id: uuid::Uuid, name: String, founder_id: String },
    OrganizationDissolved { org_id: uuid::Uuid, name: String },
    OrganizationMemberJoined { org_id: uuid::Uuid, agent_id: String, role: String },
    OrganizationMemberLeft { org_id: uuid::Uuid, agent_id: String },
    ProposalCreated { proposal_id: uuid::Uuid, org_id: uuid::Uuid, proposer_id: String, proposal_type: String },
    ProposalVotingStarted { proposal_id: uuid::Uuid, org_id: uuid::Uuid },
    ProposalVoted { proposal_id: uuid::Uuid, org_id: uuid::Uuid, voter_id: String, in_favor: bool },
    ProposalExecuted { proposal_id: uuid::Uuid, org_id: uuid::Uuid },
    ProposalRejected { proposal_id: uuid::Uuid, org_id: uuid::Uuid, reason: String },
    BankAccountOpened { account_id: String, owner_id: String, account_type: String },
    BankDeposit { account_id: String, owner_id: String, amount: u64, new_balance: u64 },
    BankWithdrawal { account_id: String, owner_id: String, amount: u64, new_balance: u64 },
    LoanApplied { loan_id: String, borrower_id: String, amount: u64, term_ticks: u64 },
    LoanApproved { loan_id: String, borrower_id: String, amount: u64 },
    LoanDisbursed { loan_id: String, borrower_id: String, amount: u64, due_tick: u64 },
    LoanRepayment { loan_id: String, borrower_id: String, amount: u64, outstanding_balance: u64, fully_repaid: bool },
    BankRateAdjusted { new_savings_rate: f64, new_loan_rate: f64 },
    MoneyMinted { amount: u64, total_supply: u64 },
    BadDebtWrittenOff { loan_id: String, borrower_id: String, amount: u64 },
    SkillLevelUp { agent_id: String, skill: String, new_level: u32 },
    SkillMutated { agent_id: String, mutation_type: String, skill: String, description: String },
    FitnessEvaluated { agent_id: String, score: f64, token_efficiency: f64, survival_duration: f64, skill_diversity: f64 },
    OrgResourceConflict { org_a: String, org_b: String, resource_point: String, winner: String, intensity: f64 },
    OrgTerritoryClaimed { org_id: String, region: String, influence: f64 },
    OrgFormationSuggested { agents: Vec<String>, suggested_type: String, reason: String },
    // Treasury events
    TaxCollected { org_id: String, payer_id: String, tax_kind: String, rate: f64, gross_amount: u64, tax_amount: u64, tick: u64 },
    TreasuryDistributed { org_id: String, strategy: String, total_amount: u64, allocations: Vec<(String, u64)>, tick: u64 },
    // Leadership events
    LeadershipElectionStarted { org_id: uuid::Uuid, candidates: Vec<String>, voting_method: String },
    LeadershipChanged { org_id: uuid::Uuid, old_leader_id: Option<String>, new_leader_id: String },
    // Diplomacy events
    TreatyProposed { treaty_id: String, org_a: String, org_b: String, treaty_type: String },
    TreatySigned { treaty_id: String, org_a: String, org_b: String },
    TreatyBroken { treaty_id: String, breaker: String, reason: String },
    RelationChanged { org_a: String, org_b: String, old_level: i8, new_level: i8 },
    // Offspring mutation events
    OffspringMutated {
        offspring_id: String,
        parent_a_id: String,
        parent_b_id: String,
        mutations_count: usize,
        mutation_types: Vec<String>,
        effective_mutation_rate: f64,
    },
}

impl WorldEvent {
    pub fn event_type(&self) -> EventType {
        match self {
            WorldEvent::TickAdvanced { .. } => EventType::TickAdvanced,
            WorldEvent::AgentSpawned { .. } => EventType::AgentSpawned,
            WorldEvent::AgentDying { .. } => EventType::AgentDying,
            WorldEvent::AgentDied { .. } => EventType::AgentDied,
            WorldEvent::AgentRescued { .. } => EventType::AgentRescued,
            WorldEvent::TransactionCompleted { .. } => EventType::TransactionCompleted,
            WorldEvent::BalanceChanged { .. } => EventType::BalanceChanged,
            WorldEvent::PhaseChanged { .. } => EventType::PhaseChanged,
            WorldEvent::RuleViolated { .. } => EventType::RuleViolated,
            WorldEvent::SnapshotTaken { .. } => EventType::SnapshotTaken,
            WorldEvent::EscrowCreated { .. } => EventType::EscrowCreated,
            WorldEvent::EscrowClaimed { .. } => EventType::EscrowClaimed,
            WorldEvent::EscrowReleased { .. } => EventType::EscrowReleased,
            WorldEvent::EscrowRefunded { .. } => EventType::EscrowRefunded,
            WorldEvent::EscrowFrozen { .. } => EventType::EscrowFrozen,
            WorldEvent::TaskCreated { .. } => EventType::TaskCreated,
            WorldEvent::TaskClaimed { .. } => EventType::TaskClaimed,
            WorldEvent::TaskStarted { .. } => EventType::TaskStarted,
            WorldEvent::TaskSubmitted { .. } => EventType::TaskSubmitted,
            WorldEvent::TaskReviewed { .. } => EventType::TaskReviewed,
            WorldEvent::TaskCompleted { .. } => EventType::TaskCompleted,
            WorldEvent::TaskExpired { .. } => EventType::TaskExpired,
            WorldEvent::RewardDistributed { .. } => EventType::RewardDistributed,
            WorldEvent::ReputationChanged { .. } => EventType::ReputationChanged,
            WorldEvent::AgentRegistered { .. } => EventType::AgentRegistered,
            WorldEvent::AgentDeregistered { .. } => EventType::AgentDeregistered,
            WorldEvent::AgentHeartbeat { .. } => EventType::AgentHeartbeat,
            WorldEvent::ConfigReloaded { .. } => EventType::ConfigReloaded,
            WorldEvent::KnowledgeListed { .. } => EventType::KnowledgeListed,
            WorldEvent::KnowledgeDelisted { .. } => EventType::KnowledgeDelisted,
            WorldEvent::KnowledgePurchased { .. } => EventType::KnowledgePurchased,
            WorldEvent::KnowledgeRated { .. } => EventType::KnowledgeRated,
            WorldEvent::TrustChanged { .. } => EventType::TrustChanged,
            WorldEvent::TrustInteraction { .. } => EventType::TrustInteraction,
            WorldEvent::MentorshipEstablished { .. } => EventType::MentorshipEstablished,
            WorldEvent::MentorshipProgress { .. } => EventType::MentorshipProgress,
            WorldEvent::MentorshipCompleted { .. } => EventType::MentorshipCompleted,
            WorldEvent::WillCreated { .. } => EventType::WillCreated,
            WorldEvent::InheritanceTriggered { .. } => EventType::InheritanceTriggered,
            WorldEvent::TimeCapsuleBriefing { .. } => EventType::TimeCapsuleBriefing,
            WorldEvent::OrgCreated { .. } => EventType::OrgCreated,
            WorldEvent::OrgMemberJoined { .. } => EventType::OrgMemberJoined,
            WorldEvent::OrgMemberLeft { .. } => EventType::OrgMemberLeft,
            WorldEvent::OrgDissolved { .. } => EventType::OrgDissolved,
            WorldEvent::OrgInactivated { .. } => EventType::OrgInactivated,
            WorldEvent::StockIssued { .. } => EventType::StockIssued,
            WorldEvent::StockIpo { .. } => EventType::StockIpo,
            WorldEvent::StockTraded { .. } => EventType::StockTraded,
            WorldEvent::StockTransferred { .. } => EventType::StockTransferred,
            WorldEvent::StockDividend { .. } => EventType::StockDividend,
            WorldEvent::OrganizationCreated { .. } => EventType::OrganizationCreated,
            WorldEvent::OrganizationDissolved { .. } => EventType::OrganizationDissolved,
            WorldEvent::OrganizationMemberJoined { .. } => EventType::OrganizationMemberJoined,
            WorldEvent::OrganizationMemberLeft { .. } => EventType::OrganizationMemberLeft,
            WorldEvent::ProposalCreated { .. } => EventType::ProposalCreated,
            WorldEvent::ProposalVotingStarted { .. } => EventType::ProposalVotingStarted,
            WorldEvent::ProposalVoted { .. } => EventType::ProposalVoted,
            WorldEvent::ProposalExecuted { .. } => EventType::ProposalExecuted,
            WorldEvent::ProposalRejected { .. } => EventType::ProposalRejected,
            WorldEvent::BankAccountOpened { .. } => EventType::BankAccountOpened,
            WorldEvent::BankDeposit { .. } => EventType::BankDeposit,
            WorldEvent::BankWithdrawal { .. } => EventType::BankWithdrawal,
            WorldEvent::LoanApplied { .. } => EventType::LoanApplied,
            WorldEvent::LoanApproved { .. } => EventType::LoanApproved,
            WorldEvent::LoanDisbursed { .. } => EventType::LoanDisbursed,
            WorldEvent::LoanRepayment { .. } => EventType::LoanRepayment,
            WorldEvent::BankRateAdjusted { .. } => EventType::BankRateAdjusted,
            WorldEvent::MoneyMinted { .. } => EventType::MoneyMinted,
            WorldEvent::BadDebtWrittenOff { .. } => EventType::BadDebtWrittenOff,
            WorldEvent::SkillLevelUp { .. } => EventType::SkillLevelUp,
            WorldEvent::SkillMutated { .. } => EventType::SkillMutated,
            WorldEvent::FitnessEvaluated { .. } => EventType::FitnessEvaluated,
            WorldEvent::OrgResourceConflict { .. } => EventType::OrgResourceConflict,
            WorldEvent::OrgTerritoryClaimed { .. } => EventType::OrgTerritoryClaimed,
            WorldEvent::OrgFormationSuggested { .. } => EventType::OrgFormationSuggested,
            WorldEvent::TaxCollected { .. } => EventType::TaxCollected,
            WorldEvent::TreasuryDistributed { .. } => EventType::TreasuryDistributed,
            WorldEvent::LeadershipElectionStarted { .. } => EventType::LeadershipElectionStarted,
            WorldEvent::LeadershipChanged { .. } => EventType::LeadershipChanged,
            WorldEvent::TreatyProposed { .. } => EventType::TreatyProposed,
            WorldEvent::TreatySigned { .. } => EventType::TreatySigned,
            WorldEvent::TreatyBroken { .. } => EventType::TreatyBroken,
            WorldEvent::RelationChanged { .. } => EventType::RelationChanged,
            WorldEvent::OffspringMutated { .. } => EventType::OffspringMutated,
        }
    }

    pub fn agent_id(&self) -> Option<&str> {
        match self {
            WorldEvent::TickAdvanced { .. } => None,
            WorldEvent::AgentSpawned { agent_id, .. } => Some(agent_id),
            WorldEvent::AgentDying { agent_id, .. } => Some(agent_id),
            WorldEvent::AgentDied { agent_id, .. } => Some(agent_id),
            WorldEvent::AgentRescued { agent_id } => Some(agent_id),
            WorldEvent::TransactionCompleted { from, .. } => Some(from),
            WorldEvent::BalanceChanged { agent_id, .. } => Some(agent_id),
            WorldEvent::PhaseChanged { agent_id, .. } => Some(agent_id),
            WorldEvent::RuleViolated { agent_id, .. } => Some(agent_id),
            WorldEvent::SnapshotTaken { .. } => None,
            WorldEvent::EscrowCreated { .. } => None,
            WorldEvent::EscrowClaimed { .. } => None,
            WorldEvent::EscrowReleased { .. } => None,
            WorldEvent::EscrowRefunded { .. } => None,
            WorldEvent::EscrowFrozen { .. } => None,
            WorldEvent::TaskCreated { .. } => None,
            WorldEvent::TaskClaimed { .. } => None,
            WorldEvent::TaskStarted { .. } => None,
            WorldEvent::TaskSubmitted { .. } => None,
            WorldEvent::TaskReviewed { .. } => None,
            WorldEvent::TaskCompleted { .. } => None,
            WorldEvent::TaskExpired { .. } => None,
            WorldEvent::RewardDistributed { assignee_id, .. } => Some(assignee_id),
            WorldEvent::ReputationChanged { agent_id, .. } => Some(agent_id),
            WorldEvent::AgentRegistered { agent_id, .. } => Some(agent_id),
            WorldEvent::AgentDeregistered { agent_id, .. } => Some(agent_id),
            WorldEvent::AgentHeartbeat { agent_id, .. } => Some(agent_id),
            WorldEvent::ConfigReloaded { .. } => None,
            WorldEvent::KnowledgeListed { publisher, .. } => Some(publisher),
            WorldEvent::KnowledgeDelisted { .. } => None,
            WorldEvent::KnowledgePurchased { buyer, .. } => Some(buyer),
            WorldEvent::KnowledgeRated { rater, .. } => Some(rater),
            WorldEvent::TrustChanged { agent_id, .. } => Some(agent_id),
            WorldEvent::TrustInteraction { from, .. } => Some(from),
            WorldEvent::MentorshipEstablished { mentor_id, .. } => Some(mentor_id),
            WorldEvent::MentorshipProgress { mentor_id, .. } => Some(mentor_id),
            WorldEvent::MentorshipCompleted { mentor_id, .. } => Some(mentor_id),
            WorldEvent::WillCreated { agent_id, .. } => Some(agent_id),
            WorldEvent::InheritanceTriggered { deceased_id, .. } => Some(deceased_id),
            WorldEvent::TimeCapsuleBriefing { .. } => None,
            WorldEvent::OrgCreated { .. } => None,
            WorldEvent::OrgMemberJoined { agent_id, .. } => Some(agent_id),
            WorldEvent::OrgMemberLeft { agent_id, .. } => Some(agent_id),
            WorldEvent::OrgDissolved { .. } => None,
            WorldEvent::OrgInactivated { .. } => None,
            WorldEvent::StockIssued { .. } => None,
            WorldEvent::StockIpo { .. } => None,
            WorldEvent::StockTraded { buyer_id, .. } => Some(buyer_id),
            WorldEvent::StockTransferred { from_agent, .. } => Some(from_agent),
            WorldEvent::StockDividend { .. } => None,
            WorldEvent::OrganizationCreated { founder_id, .. } => Some(founder_id),
            WorldEvent::OrganizationDissolved { .. } => None,
            WorldEvent::OrganizationMemberJoined { agent_id, .. } => Some(agent_id),
            WorldEvent::OrganizationMemberLeft { agent_id, .. } => Some(agent_id),
            WorldEvent::ProposalCreated { proposer_id, .. } => Some(proposer_id),
            WorldEvent::ProposalVotingStarted { .. } => None,
            WorldEvent::ProposalVoted { voter_id, .. } => Some(voter_id),
            WorldEvent::ProposalExecuted { .. } => None,
            WorldEvent::ProposalRejected { .. } => None,
            WorldEvent::BankAccountOpened { owner_id, .. } => Some(owner_id),
            WorldEvent::BankDeposit { owner_id, .. } => Some(owner_id),
            WorldEvent::BankWithdrawal { owner_id, .. } => Some(owner_id),
            WorldEvent::LoanApplied { borrower_id, .. } => Some(borrower_id),
            WorldEvent::LoanApproved { borrower_id, .. } => Some(borrower_id),
            WorldEvent::LoanDisbursed { borrower_id, .. } => Some(borrower_id),
            WorldEvent::LoanRepayment { borrower_id, .. } => Some(borrower_id),
            WorldEvent::BankRateAdjusted { .. } => None,
            WorldEvent::MoneyMinted { .. } => None,
            WorldEvent::BadDebtWrittenOff { borrower_id, .. } => Some(borrower_id),
            WorldEvent::SkillLevelUp { agent_id, .. } => Some(agent_id),
            WorldEvent::SkillMutated { agent_id, .. } => Some(agent_id),
            WorldEvent::FitnessEvaluated { agent_id, .. } => Some(agent_id),
            WorldEvent::OrgResourceConflict { .. } => None,
            WorldEvent::OrgTerritoryClaimed { .. } => None,
            WorldEvent::OrgFormationSuggested { .. } => None,
            WorldEvent::TaxCollected { payer_id, .. } => Some(payer_id),
            WorldEvent::TreasuryDistributed { .. } => None,
            WorldEvent::LeadershipElectionStarted { .. } => None,
            WorldEvent::LeadershipChanged { new_leader_id, .. } => Some(new_leader_id),
            WorldEvent::TreatyProposed { .. } => None,
            WorldEvent::TreatySigned { .. } => None,
            WorldEvent::TreatyBroken { breaker, .. } => Some(breaker),
            WorldEvent::RelationChanged { .. } => None,
            WorldEvent::OffspringMutated { offspring_id, .. } => Some(offspring_id),
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("WorldEvent serialization is infallible")
    }

    pub fn to_json_pretty(&self) -> String {
        serde_json::to_string_pretty(self).expect("WorldEvent serialization is infallible")
    }

    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_tick_round_trip() {
        let event = WorldEvent::TickAdvanced { tick: 42 };
        let json = serde_json::to_string(&event).unwrap();
        let back: WorldEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, back);
    }

    #[test]
    fn event_agent_spawned_round_trip() {
        let event = WorldEvent::AgentSpawned {
            agent_id: "agent-001".into(),
            name: "Alice".into(),
        };
        let json = serde_json::to_string(&event).unwrap();
        let back: WorldEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, back);
    }

    #[test]
    fn event_agent_dying_round_trip() {
        let event = WorldEvent::AgentDying {
            agent_id: "agent-001".into(),
            reason: DeathReason::TokenDepleted,
            grace_ticks: 10,
        };
        let json = serde_json::to_string(&event).unwrap();
        let back: WorldEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, back);
    }

    #[test]
    fn event_agent_died_round_trip() {
        let event = WorldEvent::AgentDied {
            agent_id: "agent-001".into(),
            reason: DeathReason::TokenDepleted,
        };
        let json = serde_json::to_string(&event).unwrap();
        let back: WorldEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, back);
    }

    #[test]
    fn event_agent_rescued_round_trip() {
        let event = WorldEvent::AgentRescued {
            agent_id: "agent-001".into(),
        };
        let json = serde_json::to_string(&event).unwrap();
        let back: WorldEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, back);
    }

    #[test]
    fn event_transaction_round_trip() {
        let event = WorldEvent::TransactionCompleted {
            from: "agent-001".into(),
            to: "agent-002".into(),
            amount: 100,
            currency: Currency::Token,
        };
        let json = serde_json::to_string(&event).unwrap();
        let back: WorldEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, back);
    }

    #[test]
    fn event_phase_changed_round_trip() {
        let event = WorldEvent::PhaseChanged {
            agent_id: "agent-001".into(),
            old_phase: AgentPhase::Childhood,
            new_phase: AgentPhase::Adult,
        };
        let json = serde_json::to_string(&event).unwrap();
        let back: WorldEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, back);
    }

    #[test]
    fn event_serialized_format() {
        let event = WorldEvent::TickAdvanced { tick: 1 };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"tick_advanced\""));
    }

    #[test]
    fn event_death_reason_serialized() {
        let event = WorldEvent::AgentDied {
            agent_id: "a1".into(),
            reason: DeathReason::TokenDepleted,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("token_depleted"));
    }

    #[test]
    fn event_type_discriminant() {
        assert_eq!(
            WorldEvent::TickAdvanced { tick: 1 }.event_type(),
            EventType::TickAdvanced
        );
        assert_eq!(
            WorldEvent::AgentSpawned {
                agent_id: "a".into(),
                name: "b".into(),
            }
            .event_type(),
            EventType::AgentSpawned
        );
        assert_eq!(
            WorldEvent::AgentDied {
                agent_id: "a".into(),
                reason: DeathReason::TokenDepleted,
            }
            .event_type(),
            EventType::AgentDied
        );
    }

    #[test]
    fn agent_id_returns_none_for_tick() {
        assert!(WorldEvent::TickAdvanced { tick: 1 }.agent_id().is_none());
    }

    #[test]
    fn agent_id_returns_none_for_snapshot() {
        assert!(WorldEvent::SnapshotTaken {
            tick: 1,
            path: "snap.json".into(),
        }
        .agent_id()
        .is_none());
    }

    #[test]
    fn agent_id_returns_some_for_agent_events() {
        assert_eq!(
            WorldEvent::AgentSpawned {
                agent_id: "a1".into(),
                name: "Alice".into(),
            }
            .agent_id(),
            Some("a1")
        );
    }

    #[test]
    fn agent_id_transaction_returns_from() {
        assert_eq!(
            WorldEvent::TransactionCompleted {
                from: "sender".into(),
                to: "receiver".into(),
                amount: 50,
                currency: Currency::Money,
            }
            .agent_id(),
            Some("sender")
        );
    }

    #[test]
    fn to_json_and_from_json_roundtrip() {
        let event = WorldEvent::BalanceChanged {
            agent_id: "a1".into(),
            currency: Currency::Token,
            old_balance: 100,
            new_balance: 50,
        };
        let json = event.to_json();
        let back = WorldEvent::from_json(&json).unwrap();
        assert_eq!(event, back);
    }

    #[test]
    fn to_json_pretty_produces_multiline() {
        let event = WorldEvent::TickAdvanced { tick: 1 };
        let pretty = event.to_json_pretty();
        assert!(pretty.contains('\n'));
    }

    #[test]
    fn from_json_invalid_returns_error() {
        assert!(WorldEvent::from_json("not json").is_err());
    }
}
