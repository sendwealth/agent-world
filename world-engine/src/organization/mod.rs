pub mod governance;

pub use governance::{
    GovernanceSystem, DecisionMode, ProposalType, ProposalStatus, ProfitSharingMode,
    Proposal, Vote, GovernanceError, GovernanceConfig,
};
