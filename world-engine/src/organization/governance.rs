use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::world::state::EventBus;
use crate::organization::rule_engine::RuleEngine;

// ── Decision Mode ─────────────────────────────────────────

/// How an organization makes decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionMode {
    /// Democratic voting — simple majority wins.
    Vote,
    /// Founder has absolute power — proposals auto-approve if from founder.
    Dictator,
    /// Council of leaders decides — majority of leader+ votes wins.
    Council,
}

impl DecisionMode {
    pub fn all() -> Vec<DecisionMode> {
        vec![DecisionMode::Vote, DecisionMode::Dictator, DecisionMode::Council]
    }
}

impl std::fmt::Display for DecisionMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DecisionMode::Vote => write!(f, "vote"),
            DecisionMode::Dictator => write!(f, "dictator"),
            DecisionMode::Council => write!(f, "council"),
        }
    }
}

// ── Profit Sharing Mode ───────────────────────────────────

/// How an organization distributes profits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfitSharingMode {
    /// Equal split among all members.
    Equal,
    /// Proportional to contribution score.
    Proportional,
    /// Custom weights defined by the organization.
    Custom,
}

impl std::fmt::Display for ProfitSharingMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProfitSharingMode::Equal => write!(f, "equal"),
            ProfitSharingMode::Proportional => write!(f, "proportional"),
            ProfitSharingMode::Custom => write!(f, "custom"),
        }
    }
}

// ── Proposal Types ────────────────────────────────────────

/// Types of proposals that can be submitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProposalType {
    /// Modify the organization charter / constitution.
    AmendCharter,
    /// Accept a new member.
    AcceptMember,
    /// Kick out an existing member.
    ExpelMember,
    /// Dissolve the organization.
    DissolveOrg,
    /// Change the profit distribution method.
    ChangeProfitSharing,
    /// Propose a new soft rule for the organization.
    SoftRuleProposal,
}

impl std::fmt::Display for ProposalType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProposalType::AmendCharter => write!(f, "amend_charter"),
            ProposalType::AcceptMember => write!(f, "accept_member"),
            ProposalType::ExpelMember => write!(f, "expel_member"),
            ProposalType::DissolveOrg => write!(f, "dissolve_org"),
            ProposalType::ChangeProfitSharing => write!(f, "change_profit_sharing"),
            ProposalType::SoftRuleProposal => write!(f, "soft_rule_proposal"),
        }
    }
}

// ── Proposal Status ───────────────────────────────────────

/// Lifecycle status of a proposal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProposalStatus {
    /// Proposal created, open for discussion.
    Discussion,
    /// Voting is open.
    Voting,
    /// Proposal passed and executed.
    Executed,
    /// Proposal rejected by vote.
    Rejected,
    /// Proposal cancelled by proposer.
    Cancelled,
}

impl ProposalStatus {
    pub fn can_transition_to(&self, next: &ProposalStatus) -> bool {
        matches!(
            (self, next),
            (ProposalStatus::Discussion, ProposalStatus::Voting)
                | (ProposalStatus::Discussion, ProposalStatus::Cancelled)
                | (ProposalStatus::Voting, ProposalStatus::Executed)
                | (ProposalStatus::Voting, ProposalStatus::Rejected)
                | (ProposalStatus::Voting, ProposalStatus::Cancelled)
        )
    }
}

impl std::fmt::Display for ProposalStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProposalStatus::Discussion => write!(f, "discussion"),
            ProposalStatus::Voting => write!(f, "voting"),
            ProposalStatus::Executed => write!(f, "executed"),
            ProposalStatus::Rejected => write!(f, "rejected"),
            ProposalStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}

// ── Member Role ───────────────────────────────────────────

/// Role of a member within an organization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemberRole {
    Founder,
    Leader,
    Member,
}

impl MemberRole {
    /// Voting weight: founder=3, leader=2, member=1.
    pub fn vote_weight(&self) -> u32 {
        match self {
            MemberRole::Founder => 3,
            MemberRole::Leader => 2,
            MemberRole::Member => 1,
        }
    }
}

impl std::fmt::Display for MemberRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MemberRole::Founder => write!(f, "founder"),
            MemberRole::Leader => write!(f, "leader"),
            MemberRole::Member => write!(f, "member"),
        }
    }
}

// ── Data Records ──────────────────────────────────────────

/// A member of an organization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrgMember {
    pub agent_id: String,
    pub role: MemberRole,
    pub contribution_score: u64,
    pub joined_at: u64,
}

/// A vote cast on a proposal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vote {
    pub voter_id: String,
    pub in_favor: bool,
    pub weight: u32,
    pub voted_at: u64,
}

/// A governance proposal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proposal {
    pub id: Uuid,
    pub org_id: Uuid,
    pub proposer_id: String,
    pub proposal_type: ProposalType,
    pub title: String,
    pub description: String,
    pub status: ProposalStatus,
    pub votes: Vec<Vote>,
    pub created_at: u64,
    /// Optional data payload (e.g. target agent_id for AcceptMember/ExpelMember).
    pub payload: Option<serde_json::Value>,
}

impl Proposal {
    /// Total weighted votes in favor.
    pub fn votes_for(&self) -> u32 {
        self.votes.iter().filter(|v| v.in_favor).map(|v| v.weight).sum()
    }

    /// Total weighted votes against.
    pub fn votes_against(&self) -> u32 {
        self.votes.iter().filter(|v| !v.in_favor).map(|v| v.weight).sum()
    }
}

/// An organization with its members and governance config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Organization {
    pub id: Uuid,
    pub name: String,
    pub charter: String,
    pub decision_mode: DecisionMode,
    pub profit_sharing: ProfitSharingMode,
    pub members: HashMap<String, OrgMember>,
    pub dissolved: bool,
    /// Custom profit sharing weights (agent_id -> weight), only used when mode is Custom.
    pub custom_weights: HashMap<String, u64>,
    pub created_at: u64,
}

impl Organization {
    pub fn new(name: String, founder_id: String, decision_mode: DecisionMode, created_at: u64) -> Self {
        let id = Uuid::new_v4();
        let mut members = HashMap::new();
        members.insert(founder_id.clone(), OrgMember {
            agent_id: founder_id,
            role: MemberRole::Founder,
            contribution_score: 0,
            joined_at: created_at,
        });

        Organization {
            id,
            name,
            charter: String::new(),
            decision_mode,
            profit_sharing: ProfitSharingMode::Equal,
            members,
            dissolved: false,
            custom_weights: HashMap::new(),
            created_at,
        }
    }

    pub fn is_member(&self, agent_id: &str) -> bool {
        self.members.contains_key(agent_id)
    }

    pub fn is_founder(&self, agent_id: &str) -> bool {
        self.members.get(agent_id).is_some_and(|m| m.role == MemberRole::Founder)
    }

    pub fn member_role(&self, agent_id: &str) -> Option<MemberRole> {
        self.members.get(agent_id).map(|m| m.role)
    }

    /// Total voting weight of all members.
    pub fn total_vote_weight(&self) -> u32 {
        self.members.values().map(|m| m.role.vote_weight()).sum()
    }

    /// Add a member with the given role.
    pub fn add_member(&mut self, agent_id: String, role: MemberRole, tick: u64) {
        self.members.insert(agent_id.clone(), OrgMember {
            agent_id,
            role,
            contribution_score: 0,
            joined_at: tick,
        });
    }

    /// Remove a member. Returns the removed member if they existed.
    pub fn remove_member(&mut self, agent_id: &str) -> Option<OrgMember> {
        self.members.remove(agent_id)
    }

    /// Calculate profit distribution based on total profit and mode.
    pub fn calculate_distribution(&self, total_profit: u64) -> HashMap<String, u64> {
        if self.members.is_empty() || total_profit == 0 {
            return HashMap::new();
        }

        match self.profit_sharing {
            ProfitSharingMode::Equal => {
                let count = self.members.len() as u64;
                let share = total_profit / count;
                let remainder = total_profit % count;
                let mut dist = HashMap::new();
                for (i, agent_id) in self.members.keys().enumerate() {
                    let extra = if (i as u64) < remainder { 1 } else { 0 };
                    dist.insert(agent_id.clone(), share + extra);
                }
                dist
            }
            ProfitSharingMode::Proportional => {
                let total_contribution: u64 = self.members.values().map(|m| m.contribution_score.max(1)).sum();
                let mut dist = HashMap::new();
                let mut allocated = 0u64;
                for (agent_id, member) in &self.members {
                    let score = member.contribution_score.max(1);
                    let share = (total_profit * score) / total_contribution;
                    dist.insert(agent_id.clone(), share);
                    allocated += share;
                }
                // Distribute remainder to first member
                if allocated < total_profit {
                    if let Some(first_id) = self.members.keys().next() {
                        let entry = dist.get_mut(first_id).unwrap();
                        *entry += total_profit - allocated;
                    }
                }
                dist
            }
            ProfitSharingMode::Custom => {
                let total_weight: u64 = self.custom_weights.values().sum();
                if total_weight == 0 {
                    // Fall back to equal
                    let count = self.members.len() as u64;
                    let share = total_profit / count;
                    let remainder = total_profit % count;
                    let mut dist = HashMap::new();
                    for (i, agent_id) in self.members.keys().enumerate() {
                        let extra = if (i as u64) < remainder { 1 } else { 0 };
                        dist.insert(agent_id.clone(), share + extra);
                    }
                    return dist;
                }
                let mut dist = HashMap::new();
                let mut allocated = 0u64;
                for agent_id in self.members.keys() {
                    let weight = self.custom_weights.get(agent_id).copied().unwrap_or(1);
                    let share = (total_profit * weight) / total_weight;
                    dist.insert(agent_id.clone(), share);
                    allocated += share;
                }
                if allocated < total_profit {
                    if let Some(first_id) = self.members.keys().next() {
                        let entry = dist.get_mut(first_id).unwrap();
                        *entry += total_profit - allocated;
                    }
                }
                dist
            }
        }
    }
}

// ── Errors ────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GovernanceError {
    NotFound(String),
    OrganizationNotFound(Uuid),
    AlreadyMember { org_id: Uuid, agent_id: String },
    NotMember { org_id: Uuid, agent_id: String },
    NotFounder { org_id: Uuid, agent_id: String },
    InvalidTransition { from: ProposalStatus, to: ProposalStatus },
    AlreadyVoted { proposal_id: Uuid, voter_id: String },
    VotingNotOpen(Uuid),
    ProposalNotOpen(Uuid),
    OrganizationDissolved(Uuid),
    CannotRemoveFounder,
    EmptyName,
}

impl std::fmt::Display for GovernanceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GovernanceError::NotFound(id) => write!(f, "proposal not found: {}", id),
            GovernanceError::OrganizationNotFound(id) => write!(f, "organization not found: {}", id),
            GovernanceError::AlreadyMember { org_id, agent_id } => {
                write!(f, "agent {} is already a member of org {}", agent_id, org_id)
            }
            GovernanceError::NotMember { org_id, agent_id } => {
                write!(f, "agent {} is not a member of org {}", agent_id, org_id)
            }
            GovernanceError::NotFounder { org_id, agent_id } => {
                write!(f, "agent {} is not the founder of org {}", agent_id, org_id)
            }
            GovernanceError::InvalidTransition { from, to } => {
                write!(f, "invalid proposal transition: {} -> {}", from, to)
            }
            GovernanceError::AlreadyVoted { proposal_id, voter_id } => {
                write!(f, "agent {} already voted on proposal {}", voter_id, proposal_id)
            }
            GovernanceError::VotingNotOpen(id) => {
                write!(f, "voting is not open for proposal {}", id)
            }
            GovernanceError::ProposalNotOpen(id) => {
                write!(f, "proposal {} is not open for discussion", id)
            }
            GovernanceError::OrganizationDissolved(id) => {
                write!(f, "organization {} has been dissolved", id)
            }
            GovernanceError::CannotRemoveFounder => {
                write!(f, "cannot remove the founder; transfer ownership first")
            }
            GovernanceError::EmptyName => write!(f, "organization name cannot be empty"),
        }
    }
}

impl std::error::Error for GovernanceError {}

// ── Governance Config ─────────────────────────────────────

/// Configuration for governance parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceConfig {
    /// Minimum discussion period in ticks before voting can start (default: 0).
    pub discussion_period_ticks: u64,
    /// Required quorum as a fraction of total vote weight (0.0–1.0, default: 0.5).
    pub quorum_fraction: f64,
    /// Fraction of votes that must be in favor to pass (0.0–1.0, default: 0.5).
    pub pass_threshold: f64,
}

impl Default for GovernanceConfig {
    fn default() -> Self {
        GovernanceConfig {
            discussion_period_ticks: 0,
            quorum_fraction: 0.5,
            pass_threshold: 0.5,
        }
    }
}

// ── Governance System ─────────────────────────────────────

/// Manages organizations, proposals, and voting.
pub struct GovernanceSystem {
    pub organizations: HashMap<Uuid, Organization>,
    pub proposals: HashMap<Uuid, Proposal>,
    pub config: GovernanceConfig,
    /// Soft rule engine for agent-proposed rules.
    pub active_rules: RuleEngine,
    event_bus: Option<Arc<EventBus>>,
}

impl GovernanceSystem {
    pub fn new() -> Self {
        Self {
            organizations: HashMap::new(),
            proposals: HashMap::new(),
            config: GovernanceConfig::default(),
            active_rules: RuleEngine::new(),
            event_bus: None,
        }
    }

    pub fn with_event_bus(event_bus: EventBus) -> Self {
        let arc_bus = Arc::new(event_bus);
        let active_rules = RuleEngine::with_event_bus(arc_bus.clone());
        Self {
            organizations: HashMap::new(),
            proposals: HashMap::new(),
            config: GovernanceConfig::default(),
            active_rules,
            event_bus: Some(arc_bus),
        }
    }

    pub fn with_shared_event_bus(event_bus: Arc<EventBus>) -> Self {
        let active_rules = RuleEngine::with_event_bus(event_bus.clone());
        Self {
            organizations: HashMap::new(),
            proposals: HashMap::new(),
            config: GovernanceConfig::default(),
            active_rules,
            event_bus: Some(event_bus),
        }
    }

    // ── Organization CRUD ──────────────────────────────────

    /// Create a new organization. The creator becomes the founder.
    pub fn create_org(
        &mut self,
        name: String,
        founder_id: String,
        decision_mode: DecisionMode,
        tick: u64,
    ) -> Result<Uuid, GovernanceError> {
        if name.is_empty() {
            return Err(GovernanceError::EmptyName);
        }

        let org = Organization::new(name, founder_id.clone(), decision_mode, tick);
        let org_id = org.id;
        self.organizations.insert(org_id, org);

        self.emit(crate::world::event::WorldEvent::OrganizationCreated {
            org_id,
            name: self.organizations.get(&org_id).unwrap().name.clone(),
            founder_id,
        });

        Ok(org_id)
    }

    /// Get an organization by ID.
    pub fn get_org(&self, org_id: Uuid) -> Option<&Organization> {
        self.organizations.get(&org_id)
    }

    /// Get a mutable organization by ID.
    pub fn get_org_mut(&mut self, org_id: Uuid) -> Option<&mut Organization> {
        self.organizations.get_mut(&org_id)
    }

    /// List all organizations.
    pub fn list_orgs(&self) -> Vec<&Organization> {
        self.organizations.values().collect()
    }

    /// List organizations an agent belongs to.
    pub fn list_agent_orgs(&self, agent_id: &str) -> Vec<&Organization> {
        self.organizations
            .values()
            .filter(|org| org.is_member(agent_id))
            .collect()
    }

    /// Dissolve an organization. Only the founder can dissolve.
    pub fn dissolve_org(&mut self, org_id: Uuid, requester_id: &str) -> Result<(), GovernanceError> {
        let org = self.organizations.get(&org_id)
            .ok_or(GovernanceError::OrganizationNotFound(org_id))?;

        if !org.is_founder(requester_id) {
            return Err(GovernanceError::NotFounder { org_id, agent_id: requester_id.to_string() });
        }

        let org = self.organizations.get_mut(&org_id).unwrap();
        org.dissolved = true;
        let name = org.name.clone();

        self.emit(crate::world::event::WorldEvent::OrganizationDissolved {
            org_id,
            name,
        });

        Ok(())
    }

    // ── Member Management ──────────────────────────────────

    /// Join an organization directly (for open orgs or when approved).
    pub fn join_org(&mut self, org_id: Uuid, agent_id: String, tick: u64) -> Result<(), GovernanceError> {
        let org = self.organizations.get_mut(&org_id)
            .ok_or(GovernanceError::OrganizationNotFound(org_id))?;

        if org.dissolved {
            return Err(GovernanceError::OrganizationDissolved(org_id));
        }
        if org.is_member(&agent_id) {
            return Err(GovernanceError::AlreadyMember { org_id, agent_id });
        }

        org.add_member(agent_id.clone(), MemberRole::Member, tick);

        self.emit(crate::world::event::WorldEvent::OrganizationMemberJoined {
            org_id,
            agent_id: agent_id.clone(),
            role: MemberRole::Member.to_string(),
        });

        Ok(())
    }

    /// Leave an organization. Founders cannot leave (must dissolve or transfer).
    pub fn leave_org(&mut self, org_id: Uuid, agent_id: &str) -> Result<(), GovernanceError> {
        let org = self.organizations.get(&org_id)
            .ok_or(GovernanceError::OrganizationNotFound(org_id))?;

        if !org.is_member(agent_id) {
            return Err(GovernanceError::NotMember { org_id, agent_id: agent_id.to_string() });
        }
        if org.is_founder(agent_id) {
            return Err(GovernanceError::CannotRemoveFounder);
        }

        let org = self.organizations.get_mut(&org_id).unwrap();
        org.remove_member(agent_id);

        self.emit(crate::world::event::WorldEvent::OrganizationMemberLeft {
            org_id,
            agent_id: agent_id.to_string(),
        });

        Ok(())
    }

    /// Set a member's role. Only the founder can change roles.
    pub fn set_member_role(
        &mut self,
        org_id: Uuid,
        agent_id: &str,
        new_role: MemberRole,
        requester_id: &str,
    ) -> Result<(), GovernanceError> {
        let org = self.organizations.get(&org_id)
            .ok_or(GovernanceError::OrganizationNotFound(org_id))?;

        if !org.is_founder(requester_id) {
            return Err(GovernanceError::NotFounder { org_id, agent_id: requester_id.to_string() });
        }
        if !org.is_member(agent_id) {
            return Err(GovernanceError::NotMember { org_id, agent_id: agent_id.to_string() });
        }

        let org = self.organizations.get_mut(&org_id).unwrap();
        if let Some(member) = org.members.get_mut(agent_id) {
            member.role = new_role;
        }

        Ok(())
    }

    // ── Proposal Lifecycle ─────────────────────────────────

    /// Create a new proposal.
    #[allow(clippy::too_many_arguments)]
    pub fn create_proposal(
        &mut self,
        org_id: Uuid,
        proposer_id: String,
        proposal_type: ProposalType,
        title: String,
        description: String,
        tick: u64,
        payload: Option<serde_json::Value>,
    ) -> Result<Uuid, GovernanceError> {
        let org = self.organizations.get(&org_id)
            .ok_or(GovernanceError::OrganizationNotFound(org_id))?;

        if org.dissolved {
            return Err(GovernanceError::OrganizationDissolved(org_id));
        }
        if !org.is_member(&proposer_id) {
            return Err(GovernanceError::NotMember { org_id, agent_id: proposer_id });
        }

        // In dictator mode, if the proposer is the founder, auto-execute
        if org.decision_mode == DecisionMode::Dictator && org.is_founder(&proposer_id) {
            let proposal = Proposal {
                id: Uuid::new_v4(),
                org_id,
                proposer_id: proposer_id.clone(),
                proposal_type,
                title,
                description,
                status: ProposalStatus::Executed,
                votes: vec![],
                created_at: tick,
                payload,
            };
            let proposal_id = proposal.id;
            self.proposals.insert(proposal_id, proposal);
            self.execute_proposal_side_effect(proposal_id)?;
            return Ok(proposal_id);
        }

        let proposal = Proposal {
            id: Uuid::new_v4(),
            org_id,
            proposer_id: proposer_id.clone(),
            proposal_type,
            title,
            description,
            status: ProposalStatus::Discussion,
            votes: vec![],
            created_at: tick,
            payload,
        };

        let proposal_id = proposal.id;
        self.proposals.insert(proposal_id, proposal);

        self.emit(crate::world::event::WorldEvent::ProposalCreated {
            proposal_id,
            org_id,
            proposer_id,
            proposal_type: self.proposals.get(&proposal_id).unwrap().proposal_type.to_string(),
        });

        Ok(proposal_id)
    }

    /// Move a proposal from Discussion to Voting phase.
    pub fn start_voting(&mut self, proposal_id: Uuid, requester_id: &str) -> Result<(), GovernanceError> {
        let proposal = self.proposals.get(&proposal_id)
            .ok_or(GovernanceError::NotFound(proposal_id.to_string()))?;

        if proposal.status != ProposalStatus::Discussion {
            return Err(GovernanceError::InvalidTransition {
                from: proposal.status,
                to: ProposalStatus::Voting,
            });
        }

        let org_id = proposal.org_id;
        let org = self.organizations.get(&org_id)
            .ok_or(GovernanceError::OrganizationNotFound(org_id))?;

        if !org.is_member(requester_id) {
            return Err(GovernanceError::NotMember { org_id, agent_id: requester_id.to_string() });
        }

        let proposal = self.proposals.get_mut(&proposal_id).unwrap();
        proposal.status = ProposalStatus::Voting;

        self.emit(crate::world::event::WorldEvent::ProposalVotingStarted {
            proposal_id,
            org_id,
        });

        Ok(())
    }

    /// Cast a vote on a proposal.
    pub fn vote(
        &mut self,
        proposal_id: Uuid,
        voter_id: String,
        in_favor: bool,
        tick: u64,
    ) -> Result<(), GovernanceError> {
        let proposal = self.proposals.get(&proposal_id)
            .ok_or(GovernanceError::NotFound(proposal_id.to_string()))?;

        if proposal.status != ProposalStatus::Voting {
            return Err(GovernanceError::VotingNotOpen(proposal_id));
        }

        // Check if already voted
        if proposal.votes.iter().any(|v| v.voter_id == voter_id) {
            return Err(GovernanceError::AlreadyVoted { proposal_id, voter_id });
        }

        let org_id = proposal.org_id;
        let org = self.organizations.get(&org_id)
            .ok_or(GovernanceError::OrganizationNotFound(org_id))?;

        if !org.is_member(&voter_id) {
            return Err(GovernanceError::NotMember { org_id, agent_id: voter_id });
        }

        let role = org.member_role(&voter_id).unwrap();
        let weight = role.vote_weight();

        let vote = Vote {
            voter_id: voter_id.clone(),
            in_favor,
            weight,
            voted_at: tick,
        };

        let proposal = self.proposals.get_mut(&proposal_id).unwrap();
        proposal.votes.push(vote);

        self.emit(crate::world::event::WorldEvent::ProposalVoted {
            proposal_id,
            org_id,
            voter_id,
            in_favor,
        });

        Ok(())
    }

    /// Tally votes and close the proposal.
    pub fn tally_proposal(&mut self, proposal_id: Uuid) -> Result<ProposalStatus, GovernanceError> {
        let proposal = self.proposals.get(&proposal_id)
            .ok_or(GovernanceError::NotFound(proposal_id.to_string()))?;

        if proposal.status != ProposalStatus::Voting {
            return Err(GovernanceError::InvalidTransition {
                from: proposal.status,
                to: ProposalStatus::Executed,
            });
        }

        let org_id = proposal.org_id;
        let votes_for = proposal.votes_for();
        let votes_against = proposal.votes_against();
        let total_votes = votes_for + votes_against;

        let org = self.organizations.get(&org_id)
            .ok_or(GovernanceError::OrganizationNotFound(org_id))?;
        let total_weight = org.total_vote_weight();

        // Check quorum
        let quorum_needed = (total_weight as f64 * self.config.quorum_fraction).ceil() as u32;
        if total_votes < quorum_needed {
            let proposal = self.proposals.get_mut(&proposal_id).unwrap();
            proposal.status = ProposalStatus::Rejected;

            self.emit(crate::world::event::WorldEvent::ProposalRejected {
                proposal_id,
                org_id,
                reason: "quorum not met".to_string(),
            });

            return Ok(ProposalStatus::Rejected);
        }

        let passed = if total_votes > 0 {
            (votes_for as f64 / total_votes as f64) >= self.config.pass_threshold
        } else {
            false
        };

        if passed {
            let proposal = self.proposals.get_mut(&proposal_id).unwrap();
            proposal.status = ProposalStatus::Executed;

            self.emit(crate::world::event::WorldEvent::ProposalExecuted {
                proposal_id,
                org_id,
            });

            // Execute side effects
            let _ = org;
            self.execute_proposal_side_effect(proposal_id)?;

            Ok(ProposalStatus::Executed)
        } else {
            let proposal = self.proposals.get_mut(&proposal_id).unwrap();
            proposal.status = ProposalStatus::Rejected;

            self.emit(crate::world::event::WorldEvent::ProposalRejected {
                proposal_id,
                org_id,
                reason: "vote threshold not met".to_string(),
            });

            Ok(ProposalStatus::Rejected)
        }
    }

    /// Cancel a proposal. Only the proposer can cancel.
    pub fn cancel_proposal(&mut self, proposal_id: Uuid, requester_id: &str) -> Result<(), GovernanceError> {
        let proposal = self.proposals.get(&proposal_id)
            .ok_or(GovernanceError::NotFound(proposal_id.to_string()))?;

        if proposal.proposer_id != requester_id {
            return Err(GovernanceError::NotFound(format!(
                "only the proposer can cancel proposal {}", proposal_id
            )));
        }

        if !proposal.status.can_transition_to(&ProposalStatus::Cancelled) {
            return Err(GovernanceError::InvalidTransition {
                from: proposal.status,
                to: ProposalStatus::Cancelled,
            });
        }

        let proposal = self.proposals.get_mut(&proposal_id).unwrap();
        proposal.status = ProposalStatus::Cancelled;

        Ok(())
    }

    // ── Query ──────────────────────────────────────────────

    pub fn get_proposal(&self, proposal_id: Uuid) -> Option<&Proposal> {
        self.proposals.get(&proposal_id)
    }

    pub fn list_org_proposals(&self, org_id: Uuid) -> Vec<&Proposal> {
        self.proposals.values().filter(|p| p.org_id == org_id).collect()
    }

    // ── Side Effect Execution ──────────────────────────────

    /// Execute the side effect of an approved proposal.
    fn execute_proposal_side_effect(&mut self, proposal_id: Uuid) -> Result<(), GovernanceError> {
        let proposal = self.proposals.get(&proposal_id)
            .ok_or(GovernanceError::NotFound(proposal_id.to_string()))?;

        let org_id = proposal.org_id;
        let proposal_type = proposal.proposal_type;
        let payload = proposal.payload.clone();

        match proposal_type {
            ProposalType::AcceptMember => {
                if let Some(payload) = payload {
                    if let Some(agent_id) = payload.get("agent_id").and_then(|v| v.as_str()) {
                        let org = self.organizations.get_mut(&org_id)
                            .ok_or(GovernanceError::OrganizationNotFound(org_id))?;
                        if !org.is_member(agent_id) {
                            org.add_member(agent_id.to_string(), MemberRole::Member, proposal.created_at);
                        }
                    }
                }
            }
            ProposalType::ExpelMember => {
                if let Some(payload) = payload {
                    if let Some(agent_id) = payload.get("agent_id").and_then(|v| v.as_str()) {
                        let org = self.organizations.get_mut(&org_id)
                            .ok_or(GovernanceError::OrganizationNotFound(org_id))?;
                        if org.is_founder(agent_id) {
                            // Cannot expel founder
                        } else {
                            org.remove_member(agent_id);
                        }
                    }
                }
            }
            ProposalType::DissolveOrg => {
                let org = self.organizations.get_mut(&org_id)
                    .ok_or(GovernanceError::OrganizationNotFound(org_id))?;
                org.dissolved = true;
            }
            ProposalType::ChangeProfitSharing => {
                if let Some(payload) = payload {
                    if let Some(mode_str) = payload.get("mode").and_then(|v| v.as_str()) {
                        let new_mode = match mode_str {
                            "equal" => ProfitSharingMode::Equal,
                            "proportional" => ProfitSharingMode::Proportional,
                            "custom" => ProfitSharingMode::Custom,
                            _ => ProfitSharingMode::Equal,
                        };
                        let org = self.organizations.get_mut(&org_id)
                            .ok_or(GovernanceError::OrganizationNotFound(org_id))?;
                        org.profit_sharing = new_mode;

                        // If custom, load weights from payload
                        if new_mode == ProfitSharingMode::Custom {
                            if let Some(weights) = payload.get("weights").and_then(|v| v.as_object()) {
                                let mut custom_weights = HashMap::new();
                                for (k, v) in weights {
                                    if let Some(w) = v.as_u64() {
                                        custom_weights.insert(k.clone(), w);
                                    }
                                }
                                org.custom_weights = custom_weights;
                            }
                        }
                    }
                }
            }
            ProposalType::AmendCharter => {
                if let Some(payload) = payload {
                    if let Some(charter) = payload.get("charter").and_then(|v| v.as_str()) {
                        let org = self.organizations.get_mut(&org_id)
                            .ok_or(GovernanceError::OrganizationNotFound(org_id))?;
                        org.charter = charter.to_string();
                    }
                }
            }
            ProposalType::SoftRuleProposal => {
                if let Some(payload) = payload {
                    // Extract rule data from payload and activate in the rule engine
                    let rule_id = payload.get("rule_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();

                    if !rule_id.is_empty() {
                        let _ = self.active_rules.activate_rule(&rule_id);
                    }
                }
            }
        }

        Ok(())
    }

    // ── Helpers ────────────────────────────────────────────

    fn emit(&self, event: crate::world::event::WorldEvent) {
        if let Some(ref bus) = self.event_bus {
            bus.emit(event);
        }
    }
}

impl Default for GovernanceSystem {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ─────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_system() -> GovernanceSystem {
        GovernanceSystem::new()
    }

    fn make_org(system: &mut GovernanceSystem) -> Uuid {
        system.create_org("Test Org".to_string(), "founder".to_string(), DecisionMode::Vote, 0).unwrap()
    }

    // ── Organization Creation ──────────────────────────────

    #[test]
    fn test_create_org() {
        let mut sys = make_system();
        let org_id = make_org(&mut sys);
        let org = sys.get_org(org_id).unwrap();
        assert_eq!(org.name, "Test Org");
        assert_eq!(org.decision_mode, DecisionMode::Vote);
        assert!(org.is_member("founder"));
        assert!(org.is_founder("founder"));
        assert_eq!(org.members.len(), 1);
    }

    #[test]
    fn test_create_org_empty_name_fails() {
        let mut sys = make_system();
        let result = sys.create_org("".to_string(), "founder".to_string(), DecisionMode::Vote, 0);
        assert!(result.is_err());
    }

    // ── Member Management ──────────────────────────────────

    #[test]
    fn test_join_and_leave_org() {
        let mut sys = make_system();
        let org_id = make_org(&mut sys);

        sys.join_org(org_id, "member1".to_string(), 1).unwrap();
        let org = sys.get_org(org_id).unwrap();
        assert_eq!(org.members.len(), 2);
        assert!(org.is_member("member1"));

        sys.leave_org(org_id, "member1").unwrap();
        let org = sys.get_org(org_id).unwrap();
        assert_eq!(org.members.len(), 1);
        assert!(!org.is_member("member1"));
    }

    #[test]
    fn test_join_already_member_fails() {
        let mut sys = make_system();
        let org_id = make_org(&mut sys);

        let result = sys.join_org(org_id, "founder".to_string(), 1);
        assert!(result.is_err());
    }

    #[test]
    fn test_founder_cannot_leave() {
        let mut sys = make_system();
        let org_id = make_org(&mut sys);

        let result = sys.leave_org(org_id, "founder");
        assert!(result.is_err());
    }

    #[test]
    fn test_non_member_cannot_leave() {
        let mut sys = make_system();
        let org_id = make_org(&mut sys);

        let result = sys.leave_org(org_id, "nonmember");
        assert!(result.is_err());
    }

    #[test]
    fn test_vote_weights() {
        assert_eq!(MemberRole::Founder.vote_weight(), 3);
        assert_eq!(MemberRole::Leader.vote_weight(), 2);
        assert_eq!(MemberRole::Member.vote_weight(), 1);
    }

    // ── Proposal Lifecycle ─────────────────────────────────

    #[test]
    fn test_proposal_lifecycle_vote_mode() {
        let mut sys = make_system();
        let org_id = make_org(&mut sys);
        sys.join_org(org_id, "member1".to_string(), 1).unwrap();

        // Create proposal
        let proposal_id = sys.create_proposal(
            org_id,
            "member1".to_string(),
            ProposalType::AmendCharter,
            "Update Charter".to_string(),
            "New charter text".to_string(),
            5,
            Some(serde_json::json!({ "charter": "New charter text" })),
        ).unwrap();

        let proposal = sys.get_proposal(proposal_id).unwrap();
        assert_eq!(proposal.status, ProposalStatus::Discussion);

        // Start voting
        sys.start_voting(proposal_id, "member1").unwrap();
        let proposal = sys.get_proposal(proposal_id).unwrap();
        assert_eq!(proposal.status, ProposalStatus::Voting);

        // Vote
        sys.vote(proposal_id, "founder".to_string(), true, 10).unwrap();
        sys.vote(proposal_id, "member1".to_string(), true, 10).unwrap();

        // Tally
        let result = sys.tally_proposal(proposal_id).unwrap();
        assert_eq!(result, ProposalStatus::Executed);

        // Check side effect executed
        let org = sys.get_org(org_id).unwrap();
        assert_eq!(org.charter, "New charter text");
    }

    #[test]
    fn test_proposal_rejected_by_vote() {
        let mut sys = make_system();
        let org_id = make_org(&mut sys);
        sys.join_org(org_id, "member1".to_string(), 1).unwrap();

        let proposal_id = sys.create_proposal(
            org_id,
            "member1".to_string(),
            ProposalType::AmendCharter,
            "Bad Idea".to_string(),
            "No".to_string(),
            5,
            Some(serde_json::json!({ "charter": "Bad" })),
        ).unwrap();

        sys.start_voting(proposal_id, "member1").unwrap();
        sys.vote(proposal_id, "founder".to_string(), false, 10).unwrap();
        sys.vote(proposal_id, "member1".to_string(), true, 10).unwrap();

        // founder weight=3, member1 weight=1; against=3, for=1 → rejected
        let result = sys.tally_proposal(proposal_id).unwrap();
        assert_eq!(result, ProposalStatus::Rejected);
    }

    #[test]
    fn test_dictator_mode_auto_executes() {
        let mut sys = make_system();
        let org_id = sys.create_org("Dictator Org".to_string(), "founder".to_string(), DecisionMode::Dictator, 0).unwrap();

        let proposal_id = sys.create_proposal(
            org_id,
            "founder".to_string(),
            ProposalType::AmendCharter,
            "Update".to_string(),
            "By fiat".to_string(),
            5,
            Some(serde_json::json!({ "charter": "Dictator's charter" })),
        ).unwrap();

        // Should auto-execute without voting
        let proposal = sys.get_proposal(proposal_id).unwrap();
        assert_eq!(proposal.status, ProposalStatus::Executed);

        let org = sys.get_org(org_id).unwrap();
        assert_eq!(org.charter, "Dictator's charter");
    }

    #[test]
    fn test_accept_member_proposal() {
        let mut sys = make_system();
        let org_id = make_org(&mut sys);

        let proposal_id = sys.create_proposal(
            org_id,
            "founder".to_string(),
            ProposalType::AcceptMember,
            "Accept Bob".to_string(),
            "Bob is cool".to_string(),
            5,
            Some(serde_json::json!({ "agent_id": "bob" })),
        ).unwrap();

        sys.start_voting(proposal_id, "founder").unwrap();
        sys.vote(proposal_id, "founder".to_string(), true, 10).unwrap();
        sys.tally_proposal(proposal_id).unwrap();

        let org = sys.get_org(org_id).unwrap();
        assert!(org.is_member("bob"));
    }

    #[test]
    fn test_expel_member_proposal() {
        let mut sys = make_system();
        let org_id = make_org(&mut sys);
        sys.join_org(org_id, "bad_actor".to_string(), 1).unwrap();

        let proposal_id = sys.create_proposal(
            org_id,
            "founder".to_string(),
            ProposalType::ExpelMember,
            "Kick bad_actor".to_string(),
            "Misconduct".to_string(),
            5,
            Some(serde_json::json!({ "agent_id": "bad_actor" })),
        ).unwrap();

        sys.start_voting(proposal_id, "founder").unwrap();
        sys.vote(proposal_id, "founder".to_string(), true, 10).unwrap();
        sys.tally_proposal(proposal_id).unwrap();

        let org = sys.get_org(org_id).unwrap();
        assert!(!org.is_member("bad_actor"));
    }

    #[test]
    fn test_dissolve_org_proposal() {
        let mut sys = make_system();
        let org_id = make_org(&mut sys);

        let proposal_id = sys.create_proposal(
            org_id,
            "founder".to_string(),
            ProposalType::DissolveOrg,
            "Dissolve".to_string(),
            "We're done".to_string(),
            5,
            None,
        ).unwrap();

        sys.start_voting(proposal_id, "founder").unwrap();
        sys.vote(proposal_id, "founder".to_string(), true, 10).unwrap();
        sys.tally_proposal(proposal_id).unwrap();

        let org = sys.get_org(org_id).unwrap();
        assert!(org.dissolved);
    }

    #[test]
    fn test_change_profit_sharing_proposal() {
        let mut sys = make_system();
        let org_id = make_org(&mut sys);

        let proposal_id = sys.create_proposal(
            org_id,
            "founder".to_string(),
            ProposalType::ChangeProfitSharing,
            "Change to proportional".to_string(),
            "Fair distribution".to_string(),
            5,
            Some(serde_json::json!({ "mode": "proportional" })),
        ).unwrap();

        sys.start_voting(proposal_id, "founder").unwrap();
        sys.vote(proposal_id, "founder".to_string(), true, 10).unwrap();
        sys.tally_proposal(proposal_id).unwrap();

        let org = sys.get_org(org_id).unwrap();
        assert_eq!(org.profit_sharing, ProfitSharingMode::Proportional);
    }

    #[test]
    fn test_non_member_cannot_create_proposal() {
        let mut sys = make_system();
        let org_id = make_org(&mut sys);

        let result = sys.create_proposal(
            org_id,
            "outsider".to_string(),
            ProposalType::AmendCharter,
            "Hack".to_string(),
            "Nope".to_string(),
            5,
            None,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_double_vote_fails() {
        let mut sys = make_system();
        let org_id = make_org(&mut sys);

        let proposal_id = sys.create_proposal(
            org_id,
            "founder".to_string(),
            ProposalType::AmendCharter,
            "Test".to_string(),
            "Test".to_string(),
            5,
            None,
        ).unwrap();

        sys.start_voting(proposal_id, "founder").unwrap();
        sys.vote(proposal_id, "founder".to_string(), true, 10).unwrap();

        let result = sys.vote(proposal_id, "founder".to_string(), true, 10);
        assert!(result.is_err());
    }

    #[test]
    fn test_cancel_proposal() {
        let mut sys = make_system();
        let org_id = make_org(&mut sys);

        let proposal_id = sys.create_proposal(
            org_id,
            "founder".to_string(),
            ProposalType::AmendCharter,
            "Test".to_string(),
            "Cancel me".to_string(),
            5,
            None,
        ).unwrap();

        sys.cancel_proposal(proposal_id, "founder").unwrap();
        let proposal = sys.get_proposal(proposal_id).unwrap();
        assert_eq!(proposal.status, ProposalStatus::Cancelled);
    }

    // ── Profit Distribution ────────────────────────────────

    #[test]
    fn test_equal_profit_distribution() {
        let mut sys = make_system();
        let org_id = make_org(&mut sys);
        sys.join_org(org_id, "m1".to_string(), 1).unwrap();
        sys.join_org(org_id, "m2".to_string(), 1).unwrap();

        let org = sys.get_org(org_id).unwrap();
        let dist = org.calculate_distribution(300);
        assert_eq!(dist.get("founder"), Some(&100));
        assert_eq!(dist.get("m1"), Some(&100));
        assert_eq!(dist.get("m2"), Some(&100));
    }

    #[test]
    fn test_proportional_profit_distribution() {
        let mut sys = make_system();
        let org_id = make_org(&mut sys);

        // Set profit sharing mode to Proportional
        sys.get_org_mut(org_id).unwrap().profit_sharing = ProfitSharingMode::Proportional;

        sys.join_org(org_id, "m1".to_string(), 1).unwrap();

        // Set contribution scores
        sys.get_org_mut(org_id).unwrap().members.get_mut("m1").unwrap().contribution_score = 100;

        let org = sys.get_org(org_id).unwrap();
        // Verify contribution scores
        assert_eq!(org.members.get("m1").unwrap().contribution_score, 100);
        assert_eq!(org.members.get("founder").unwrap().contribution_score, 0);

        // founder has contribution_score 0 (min 1), m1 has 100
        // total = 1 + 100 = 101
        let dist = org.calculate_distribution(101);
        // m1 should get the lion's share
        let m1_share = *dist.get("m1").unwrap();
        let founder_share = *dist.get("founder").unwrap();
        assert!(m1_share > founder_share, "m1 share ({}) should be > founder share ({})", m1_share, founder_share);
        assert_eq!(m1_share + founder_share, 101);
    }

    #[test]
    fn test_custom_profit_distribution() {
        let mut sys = make_system();
        let org_id = sys.create_org("Custom".to_string(), "founder".to_string(), DecisionMode::Vote, 0).unwrap();
        sys.join_org(org_id, "m1".to_string(), 1).unwrap();

        {
            let org = sys.get_org_mut(org_id).unwrap();
            org.profit_sharing = ProfitSharingMode::Custom;
            org.custom_weights.insert("founder".to_string(), 3);
            org.custom_weights.insert("m1".to_string(), 1);
        }

        let org = sys.get_org(org_id).unwrap();
        let dist = org.calculate_distribution(400);
        assert_eq!(dist.get("founder"), Some(&300));
        assert_eq!(dist.get("m1"), Some(&100));
    }

    // ── Dissolve ───────────────────────────────────────────

    #[test]
    fn test_dissolve_org_only_founder() {
        let mut sys = make_system();
        let org_id = make_org(&mut sys);
        sys.join_org(org_id, "m1".to_string(), 1).unwrap();

        // Non-founder cannot dissolve
        let result = sys.dissolve_org(org_id, "m1");
        assert!(result.is_err());

        // Founder can dissolve
        sys.dissolve_org(org_id, "founder").unwrap();
        assert!(sys.get_org(org_id).unwrap().dissolved);
    }

    // ── Serialization ──────────────────────────────────────

    #[test]
    fn test_decision_mode_serialization() {
        for mode in DecisionMode::all() {
            let json = serde_json::to_string(&mode).unwrap();
            let back: DecisionMode = serde_json::from_str(&json).unwrap();
            assert_eq!(mode, back);
        }
    }

    #[test]
    fn test_proposal_status_serialization() {
        let statuses = vec![
            ProposalStatus::Discussion,
            ProposalStatus::Voting,
            ProposalStatus::Executed,
            ProposalStatus::Rejected,
            ProposalStatus::Cancelled,
        ];
        for status in statuses {
            let json = serde_json::to_string(&status).unwrap();
            let back: ProposalStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(status, back);
        }
    }

    #[test]
    fn test_organization_serialization() {
        let org = Organization::new("Test".to_string(), "founder".to_string(), DecisionMode::Council, 100);
        let json = serde_json::to_string(&org).unwrap();
        let back: Organization = serde_json::from_str(&json).unwrap();
        assert_eq!(org.id, back.id);
        assert_eq!(org.name, back.name);
        assert_eq!(org.decision_mode, back.decision_mode);
        assert_eq!(org.members.len(), back.members.len());
    }

    // ── Event Bus Integration ──────────────────────────────

    #[test]
    fn test_event_bus_create_org() {
        let bus = crate::world::state::EventBus::new(64);
        let mut rx = bus.subscribe();
        let mut sys = GovernanceSystem::with_event_bus(bus);

        let org_id = sys.create_org("Event Org".to_string(), "founder".to_string(), DecisionMode::Vote, 0).unwrap();

        let event = rx.try_recv().unwrap();
        match event {
            crate::world::event::WorldEvent::OrganizationCreated { org_id: eid, name, founder_id } => {
                assert_eq!(eid, org_id);
                assert_eq!(name, "Event Org");
                assert_eq!(founder_id, "founder");
            }
            _ => panic!("Expected OrganizationCreated event"),
        }
    }
}
