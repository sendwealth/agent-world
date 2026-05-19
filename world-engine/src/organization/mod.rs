pub mod charter;
pub mod governance;
pub mod members;
pub mod org;

pub use charter::{Charter, GovernanceModel, ProfitSharing};
pub use governance::{
    GovernanceSystem, DecisionMode, ProposalType, ProposalStatus, ProfitSharingMode,
    Proposal, Vote, GovernanceError, GovernanceConfig,
};
pub use members::{MemberRole, OrgMember, MemberError};
pub use org::{Organization, OrganizationStore, OrgType, OrgStatus, OrgError, MIN_FOUNDERS, CREATION_COST_MONEY, INACTIVE_THRESHOLD_TICKS};
