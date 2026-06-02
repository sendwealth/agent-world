//! # Organization Subsystem
//!
//! Multi-agent organizations with governance, leadership elections,
//! treasury, diplomacy, competition, and rule engine.
//!
//! Key types: Organization, OrganizationStore, GovernanceSystem,
//!            LeadershipEngine, CompetitionEngine, Treasury,
//!            Charter, RuleEngine, DiplomacyEngine
//! Depends on: world (WorldState, EventBus), economy (BankingSystem)
//!
//! Sub-modules: charter, competition, diplomacy, governance,
//!              governance_metrics, leadership, legislation_cycle,
//!              members, org, rule_engine, treasury
//!
pub mod charter;
pub mod competition;
pub mod diplomacy;
pub mod governance;
pub mod governance_metrics;
pub mod leadership;
pub mod legislation_cycle;
pub mod members;
pub mod org;
pub mod rule_engine;
pub mod treasury;

pub use charter::{Charter, GovernanceModel, ProfitSharing};
pub use competition::{
    CompetitionEngine, FormationSuggestion, OrgInvitation, RecruitmentResult,
    ResourceConflictResult, TerritoryRegion, TerritoryResult, FORMATION_SCAN_INTERVAL,
    LOSER_RESOURCE_PENALTY, WINNER_RESOURCE_BONUS,
};
pub use diplomacy::{
    DiplomacyEngine, DiplomacyError, RelationLevel, Treaty, TreatyStatus, TreatyType,
};
pub use governance::{
    DecisionMode, GovernanceConfig, GovernanceError, GovernanceSystem, ProfitSharingMode, Proposal,
    ProposalStatus, ProposalType, Vote,
};
pub use governance_metrics::{
    GovernanceEvent, GovernanceMetricsCollector, OrgMetrics, WorldGovernanceSummary,
};
pub use leadership::{
    Ballot, Election, ElectionStatus, LeadershipEngine, LeadershipError, VotingMethod,
};
pub use legislation_cycle::{
    CandidateRule, CycleEffectSummary, CycleStatus, LegislationCycleConfig,
    LegislationCycleEngine, LegislationCycleError, LegislationCycleRecord,
};
pub use members::{MemberError, MemberRole, OrgMember};
pub use org::{
    OrgError, OrgStatus, OrgType, Organization, OrganizationStore, CREATION_COST_MONEY,
    INACTIVE_THRESHOLD_TICKS, MIN_FOUNDERS,
};
pub use rule_engine::{
    apply_effect, RuleCondition, RuleEffect, RuleEngine, RuleEngineError, RuleStatus, RuleType,
    SoftRule,
};
pub use treasury::{
    DistributionRecord, DistributionStrategy, TaxConfig, TaxKind, TaxRecord, Treasury,
    TreasuryError, DEFAULT_INCOME_TAX_RATE, DEFAULT_TRADE_TAX_RATE, DEFAULT_WEALTH_TAX_RATE,
    MAX_TAX_RATE, MIN_TAX_RATE,
};
