pub mod charter;
pub mod competition;
pub mod governance;
pub mod members;
pub mod org;

pub use charter::{Charter, GovernanceModel, ProfitSharing};
pub use competition::{
    CompetitionEngine, ResourceConflictResult, RecruitmentResult, OrgInvitation,
    TerritoryResult, TerritoryRegion, FormationSuggestion,
    WINNER_RESOURCE_BONUS, LOSER_RESOURCE_PENALTY, FORMATION_SCAN_INTERVAL,
};
pub use governance::{
    GovernanceSystem, DecisionMode, ProposalType, ProposalStatus, ProfitSharingMode,
    Proposal, Vote, GovernanceError, GovernanceConfig,
};
pub use members::{MemberRole, OrgMember, MemberError};
pub use org::{Organization, OrganizationStore, OrgType, OrgStatus, OrgError, MIN_FOUNDERS, CREATION_COST_MONEY, INACTIVE_THRESHOLD_TICKS};
