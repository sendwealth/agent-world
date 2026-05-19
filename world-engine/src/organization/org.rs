use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::charter::Charter;
use super::members::{MemberError, MemberRole, OrgMember};
use crate::world::event::WorldEvent;
use crate::world::state::EventBus;

// ── Constants ─────────────────────────────────────────────

/// Minimum number of founders required to create an org.
pub const MIN_FOUNDERS: usize = 2;
/// Money cost to create an org.
pub const CREATION_COST_MONEY: u64 = 100;
/// Ticks of inactivity before dissolution vote is triggered.
pub const INACTIVE_THRESHOLD_TICKS: u64 = 500;

// ── Enums ─────────────────────────────────────────────────

/// The four organization types defined in DESIGN.md.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrgType {
    /// Company — profit-driven.
    Company,
    /// Guild — skill-based mutual aid.
    Guild,
    /// Alliance — defense cooperation.
    Alliance,
    /// University — knowledge preservation and transfer.
    University,
}

/// Organization lifecycle status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrgStatus {
    /// Fully active.
    Active,
    /// No activity for `INACTIVE_THRESHOLD_TICKS`.
    Inactive,
    /// Organization has been dissolved.
    Dissolved,
}

// ── Core Organization Struct ──────────────────────────────

/// An organization in the world.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Organization {
    /// Unique ID.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Organization type.
    #[serde(rename = "type")]
    pub org_type: OrgType,
    /// Founding charter.
    pub charter: Charter,
    /// Treasury in Money.
    pub treasury: u64,
    /// Outstanding debts in Money.
    pub debts: u64,
    /// Current lifecycle status.
    pub status: OrgStatus,
    /// Tick when the org was created.
    pub created_tick: u64,
    /// Last tick when the org had activity.
    pub last_activity_tick: u64,
    /// Current members.
    pub members: Vec<OrgMember>,
}

impl Organization {
    /// Total number of members.
    pub fn member_count(&self) -> usize {
        self.members.len()
    }

    /// Check if an agent is a member.
    pub fn is_member(&self, agent_id: &str) -> bool {
        self.members.iter().any(|m| m.agent_id == agent_id)
    }

    /// Get a member by agent ID.
    pub fn get_member(&self, agent_id: &str) -> Option<&OrgMember> {
        self.members.iter().find(|m| m.agent_id == agent_id)
    }

    /// Get the current total of all member shares.
    pub fn total_shares(&self) -> f64 {
        self.members.iter().map(|m| m.share).sum()
    }

    /// Whether the org is bankrupt (assets < debts).
    pub fn is_bankrupt(&self) -> bool {
        self.treasury < self.debts
    }

    /// Whether the org should be considered inactive based on tick age.
    pub fn should_be_inactive(&self, current_tick: u64) -> bool {
        self.status == OrgStatus::Active
            && current_tick.saturating_sub(self.last_activity_tick) >= INACTIVE_THRESHOLD_TICKS
    }

    /// Record activity, resetting the inactivity timer.
    pub fn touch_activity(&mut self, tick: u64) {
        self.last_activity_tick = tick;
        if self.status == OrgStatus::Inactive {
            self.status = OrgStatus::Active;
        }
    }
}

// ── Error Type ────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum OrgError {
    #[error("organization not found: {0}")]
    NotFound(String),
    #[error("at least {MIN_FOUNDERS} founders are required")]
    NotEnoughFounders,
    #[error("a charter is required to create an organization")]
    CharterRequired,
    #[error("organization name cannot be empty")]
    EmptyName,
    #[error("agent {0} is already in an organization")]
    AgentAlreadyInOrg(String),
    #[error("creation cost of {CREATION_COST_MONEY} money is required")]
    InsufficientCreationFunds,
    #[error("member error: {0}")]
    Member(#[from] MemberError),
    #[error("cannot join a dissolved organization")]
    OrgDissolved,
    #[error("cannot perform operations on an inactive organization; reactivate first")]
    OrgInactive,
}

// ── Organization Store ────────────────────────────────────

/// In-memory store of all organizations.
/// Tracks which agents belong to which org to enforce single-membership.
pub struct OrganizationStore {
    organizations: HashMap<String, Organization>,
    /// Maps agent_id -> org_id for quick membership checks.
    agent_to_org: HashMap<String, String>,
    event_bus: Option<EventBus>,
}

impl OrganizationStore {
    /// Create a new empty store.
    pub fn new() -> Self {
        Self {
            organizations: HashMap::new(),
            agent_to_org: HashMap::new(),
            event_bus: None,
        }
    }

    /// Create a store wired to an EventBus for broadcasting events.
    pub fn with_event_bus(event_bus: EventBus) -> Self {
        Self {
            organizations: HashMap::new(),
            agent_to_org: HashMap::new(),
            event_bus: Some(event_bus),
        }
    }

    /// Emit an event to the event bus (if configured).
    /// Takes a separate reference so the caller can hold a mutable borrow to self.organizations.
    fn emit_event(event_bus: &Option<EventBus>, event: WorldEvent) {
        if let Some(ref bus) = event_bus {
            bus.emit(event);
        }
    }

    /// Create a new organization.
    ///
    /// Requirements from DESIGN.md:
    /// - min_founders = 2
    /// - cost_money = 100 (from the founders' pooled funds; validation left to caller)
    /// - requires_charter = true
    pub fn create_org(
        &mut self,
        name: String,
        org_type: OrgType,
        charter: Option<Charter>,
        founders: Vec<(String, String)>, // (agent_id, agent_name)
        current_tick: u64,
    ) -> Result<Organization, OrgError> {
        // Validate
        if name.trim().is_empty() {
            return Err(OrgError::EmptyName);
        }
        if founders.len() < MIN_FOUNDERS {
            return Err(OrgError::NotEnoughFounders);
        }
        let charter = charter.ok_or(OrgError::CharterRequired)?;

        // Check that no founder is already in an org
        for (agent_id, _) in &founders {
            if self.agent_to_org.contains_key(agent_id) {
                return Err(OrgError::AgentAlreadyInOrg(agent_id.clone()));
            }
        }

        let org_id = Uuid::new_v4().to_string();

        // Distribute shares equally among founders
        let share_each = 1.0 / founders.len() as f64;
        let members: Vec<OrgMember> = founders
            .into_iter()
            .map(|(agent_id, agent_name)| OrgMember {
                agent_id,
                agent_name,
                role: MemberRole::Founder,
                share: share_each,
                joined_tick: current_tick,
            })
            .collect();

        let org = Organization {
            id: org_id.clone(),
            name: name.clone(),
            org_type,
            charter,
            treasury: CREATION_COST_MONEY, // The creation cost goes into the treasury
            debts: 0,
            status: OrgStatus::Active,
            created_tick: current_tick,
            last_activity_tick: current_tick,
            members,
        };

        // Register agent -> org mappings
        for member in &org.members {
            self.agent_to_org.insert(member.agent_id.clone(), org_id.clone());
        }

        Self::emit_event(&self.event_bus,WorldEvent::OrgCreated {
            org_id: org_id.clone(),
            name: name.clone(),
            org_type: format!("{:?}", org.org_type).to_lowercase(),
            founder_count: org.members.len(),
        });

        self.organizations.insert(org_id, org.clone());
        Ok(org)
    }

    /// Get an organization by ID.
    pub fn get(&self, org_id: &str) -> Option<&Organization> {
        self.organizations.get(org_id)
    }

    /// Get a mutable reference to an organization by ID.
    pub fn get_mut(&mut self, org_id: &str) -> Option<&mut Organization> {
        self.organizations.get_mut(org_id)
    }

    /// List all organizations.
    pub fn list(&self) -> Vec<&Organization> {
        self.organizations.values().collect()
    }

    /// List organizations filtered by status.
    pub fn list_by_status(&self, status: OrgStatus) -> Vec<&Organization> {
        self.organizations
            .values()
            .filter(|o| o.status == status)
            .collect()
    }

    /// An agent joins an organization.
    pub fn join_org(
        &mut self,
        org_id: &str,
        agent_id: String,
        agent_name: String,
        current_tick: u64,
    ) -> Result<Organization, OrgError> {
        // Check agent isn't already in an org
        if self.agent_to_org.contains_key(&agent_id) {
            return Err(OrgError::AgentAlreadyInOrg(agent_id));
        }

        let org = self.organizations.get_mut(org_id)
            .ok_or_else(|| OrgError::NotFound(org_id.to_string()))?;

        if org.status == OrgStatus::Dissolved {
            return Err(OrgError::OrgDissolved);
        }

        // Compute share: distribute 1.0 total among all members (re-equalize)
        // New member gets an equal share
        let new_count = org.members.len() + 1;
        let share = 1.0 / new_count as f64;

        // Redistribute existing members' shares
        for member in &mut org.members {
            member.share = share;
        }

        org.members.push(OrgMember {
            agent_id: agent_id.clone(),
            agent_name: agent_name.clone(),
            role: MemberRole::Member,
            share,
            joined_tick: current_tick,
        });
        org.touch_activity(current_tick);

        let total_members = org.members.len();
        let org_clone = org.clone();

        self.agent_to_org.insert(agent_id.clone(), org_id.to_string());

        if let Some(ref bus) = self.event_bus {
            bus.emit(WorldEvent::OrgMemberJoined {
                org_id: org_id.to_string(),
                agent_id,
                agent_name,
                role: "member".to_string(),
                total_members,
            });
        }

        Ok(org_clone)
    }

    /// An agent leaves an organization.
    pub fn leave_org(
        &mut self,
        org_id: &str,
        agent_id: &str,
        current_tick: u64,
    ) -> Result<Organization, OrgError> {
        let org = self.organizations.get_mut(org_id)
            .ok_or_else(|| OrgError::NotFound(org_id.to_string()))?;

        if org.status == OrgStatus::Dissolved {
            return Err(OrgError::OrgDissolved);
        }

        // Find the member
        let member_idx = org.members.iter().position(|m| m.agent_id == agent_id)
            .ok_or_else(|| MemberError::NotMember(agent_id.to_string()))?;

        let removed_member = org.members.remove(member_idx);

        // Cannot remove last founder if other members remain — they must dissolve the org instead
        if removed_member.role == MemberRole::Founder
            && !org.members.is_empty()
            && !org.members.iter().any(|m| m.role == MemberRole::Founder)
        {
            // Put them back
            org.members.push(removed_member);
            return Err(MemberError::CannotRemoveLastFounder.into());
        }

        // Redistribute shares equally
        let share = if !org.members.is_empty() {
            1.0 / org.members.len() as f64
        } else {
            0.0
        };
        for member in &mut org.members {
            member.share = share;
        }

        org.touch_activity(current_tick);

        let total_members = org.members.len();
        let should_dissolve = total_members == 0;
        let org_clone = org.clone();

        self.agent_to_org.remove(agent_id);

        // Emit events after mutable borrow ends
        let mut events: Vec<WorldEvent> = vec![WorldEvent::OrgMemberLeft {
            org_id: org_id.to_string(),
            agent_id: agent_id.to_string(),
            remaining_members: total_members,
        }];

        if should_dissolve {
            let org = self.organizations.get_mut(org_id).unwrap();
            org.status = OrgStatus::Dissolved;
            events.push(WorldEvent::OrgDissolved {
                org_id: org_id.to_string(),
                reason: "all_members_left".to_string(),
            });
        }

        if let Some(ref bus) = self.event_bus {
            for event in events {
                bus.emit(event);
            }
        }

        Ok(org_clone)
    }

    /// Check all active orgs for inactivity and mark them.
    /// Returns the list of org IDs that transitioned to inactive.
    pub fn check_inactivity(&mut self, current_tick: u64) -> Vec<String> {
        // Collect data in first pass
        let transitions: Vec<(String, u64)> = self.organizations
            .values_mut()
            .filter_map(|org| {
                if org.should_be_inactive(current_tick) {
                    org.status = OrgStatus::Inactive;
                    Some((org.id.clone(), org.last_activity_tick))
                } else {
                    None
                }
            })
            .collect();

        // Emit events in second pass
        for (org_id, inactive_since) in &transitions {
            if let Some(ref bus) = self.event_bus {
                bus.emit(WorldEvent::OrgInactivated {
                    org_id: org_id.clone(),
                    inactive_since: *inactive_since,
                    current_tick,
                });
            }
        }

        transitions.into_iter().map(|(id, _)| id).collect()
    }

    /// Check all active/inactive orgs for bankruptcy and dissolve them.
    /// Returns the list of org IDs that were dissolved.
    pub fn check_bankruptcy(&mut self) -> Vec<String> {
        // Collect data in first pass
        let dissolved: Vec<String> = self.organizations
            .values_mut()
            .filter_map(|org| {
                if org.status != OrgStatus::Dissolved && org.is_bankrupt() {
                    org.status = OrgStatus::Dissolved;
                    Some(org.id.clone())
                } else {
                    None
                }
            })
            .collect();

        // Emit events in second pass
        for org_id in &dissolved {
            if let Some(ref bus) = self.event_bus {
                bus.emit(WorldEvent::OrgDissolved {
                    org_id: org_id.clone(),
                    reason: "bankruptcy".to_string(),
                });
            }
        }

        dissolved
    }

    /// Manually dissolve an organization.
    pub fn dissolve_org(&mut self, org_id: &str, reason: &str) -> Result<(), OrgError> {
        let org = self.organizations.get_mut(org_id)
            .ok_or_else(|| OrgError::NotFound(org_id.to_string()))?;

        org.status = OrgStatus::Dissolved;

        // Collect member agent IDs
        let member_ids: Vec<String> = org.members.iter().map(|m| m.agent_id.clone()).collect();

        // Clear agent-to-org mappings
        for aid in &member_ids {
            self.agent_to_org.remove(aid);
        }

        if let Some(ref bus) = self.event_bus {
            bus.emit(WorldEvent::OrgDissolved {
                org_id: org_id.to_string(),
                reason: reason.to_string(),
            });
        }

        Ok(())
    }

    /// Get the org ID an agent belongs to (if any).
    pub fn agent_org(&self, agent_id: &str) -> Option<&str> {
        self.agent_to_org.get(agent_id).map(|s| s.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::organization::charter::{Charter, GovernanceModel, ProfitSharing};

    fn test_charter() -> Charter {
        Charter {
            purpose: "Test org".to_string(),
            governance: GovernanceModel::Vote,
            profit_sharing: ProfitSharing::Equal,
            membership_fee: 0,
        }
    }

    fn make_founders(n: usize) -> Vec<(String, String)> {
        (0..n)
            .map(|i| (format!("agent-{}", i), format!("Agent {}", i)))
            .collect()
    }

    #[test]
    fn create_org_success() {
        let mut store = OrganizationStore::new();
        let org = store.create_org(
            "TestCorp".to_string(),
            OrgType::Company,
            Some(test_charter()),
            make_founders(3),
            100,
        ).unwrap();

        assert_eq!(org.name, "TestCorp");
        assert_eq!(org.org_type, OrgType::Company);
        assert_eq!(org.status, OrgStatus::Active);
        assert_eq!(org.members.len(), 3);
        assert_eq!(org.treasury, CREATION_COST_MONEY);
    }

    #[test]
    fn create_org_requires_min_founders() {
        let mut store = OrganizationStore::new();
        let result = store.create_org(
            "Small".to_string(),
            OrgType::Guild,
            Some(test_charter()),
            make_founders(1),
            100,
        );
        assert!(result.is_err());
    }

    #[test]
    fn create_org_requires_charter() {
        let mut store = OrganizationStore::new();
        let result = store.create_org(
            "NoCharter".to_string(),
            OrgType::Alliance,
            None,
            make_founders(2),
            100,
        );
        assert!(result.is_err());
    }

    #[test]
    fn create_org_rejects_empty_name() {
        let mut store = OrganizationStore::new();
        let result = store.create_org(
            "".to_string(),
            OrgType::University,
            Some(test_charter()),
            make_founders(2),
            100,
        );
        assert!(result.is_err());
    }

    #[test]
    fn create_org_rejects_duplicate_founder() {
        let mut store = OrganizationStore::new();
        // First create an org
        store.create_org(
            "First".to_string(),
            OrgType::Company,
            Some(test_charter()),
            make_founders(2),
            100,
        ).unwrap();

        // Try to use an existing member as founder
        let result = store.create_org(
            "Second".to_string(),
            OrgType::Guild,
            Some(test_charter()),
            vec![("agent-0".to_string(), "Agent 0".to_string()), ("agent-99".to_string(), "Agent 99".to_string())],
            100,
        );
        assert!(result.is_err());
    }

    #[test]
    fn join_org_success() {
        let mut store = OrganizationStore::new();
        let org = store.create_org(
            "TestOrg".to_string(),
            OrgType::Guild,
            Some(test_charter()),
            make_founders(2),
            100,
        ).unwrap();

        let updated = store.join_org(&org.id, "agent-new".to_string(), "New Agent".to_string(), 200).unwrap();
        assert_eq!(updated.members.len(), 3);
        assert_eq!(updated.members[2].agent_id, "agent-new");
        assert_eq!(updated.members[2].role, MemberRole::Member);
    }

    #[test]
    fn join_org_rejects_existing_member() {
        let mut store = OrganizationStore::new();
        let org = store.create_org(
            "TestOrg".to_string(),
            OrgType::Guild,
            Some(test_charter()),
            make_founders(2),
            100,
        ).unwrap();

        let result = store.join_org(&org.id, "agent-0".to_string(), "Agent 0".to_string(), 200);
        assert!(result.is_err());
    }

    #[test]
    fn leave_org_success() {
        let mut store = OrganizationStore::new();
        let org = store.create_org(
            "TestOrg".to_string(),
            OrgType::Guild,
            Some(test_charter()),
            make_founders(3),
            100,
        ).unwrap();

        let updated = store.leave_org(&org.id, "agent-1", 300).unwrap();
        assert_eq!(updated.members.len(), 2);
        assert!(!updated.is_member("agent-1"));
        assert!(store.agent_org("agent-1").is_none());
    }

    #[test]
    fn leave_org_cannot_remove_last_founder_when_members_remain() {
        let mut store = OrganizationStore::new();
        let org = store.create_org(
            "TestOrg".to_string(),
            OrgType::Guild,
            Some(test_charter()),
            make_founders(3),
            100,
        ).unwrap();

        // All 3 are founders; remove 2 of them, leaving 1 founder
        store.leave_org(&org.id, "agent-0", 300).unwrap();
        store.leave_org(&org.id, "agent-1", 300).unwrap();

        // Now trying to remove the last founder while no other members remain
        // should succeed (auto-dissolves the org)
        let result = store.leave_org(&org.id, "agent-2", 300);
        assert!(result.is_ok());

        let org = store.get(&org.id).unwrap();
        assert_eq!(org.status, OrgStatus::Dissolved);
    }

    #[test]
    fn leave_org_cannot_remove_last_founder_with_remaining_members() {
        let mut store = OrganizationStore::new();
        let org = store.create_org(
            "TestOrg".to_string(),
            OrgType::Guild,
            Some(test_charter()),
            make_founders(2),
            100,
        ).unwrap();

        // Join a regular member
        store.join_org(&org.id, "agent-regular".to_string(), "Regular Member".to_string(), 200).unwrap();

        // Remove one founder -> now only 1 founder remains with 1 regular member
        store.leave_org(&org.id, "agent-0", 300).unwrap();

        // Now trying to remove the last founder should fail (regular member still exists)
        let result = store.leave_org(&org.id, "agent-1", 300);
        assert!(result.is_err());
    }

    #[test]
    fn leave_org_auto_dissolves_when_empty() {
        let mut store = OrganizationStore::new();
        let org = store.create_org(
            "TestOrg".to_string(),
            OrgType::Guild,
            Some(test_charter()),
            make_founders(3),
            100,
        ).unwrap();

        store.leave_org(&org.id, "agent-0", 300).unwrap();
        store.leave_org(&org.id, "agent-1", 300).unwrap();
        store.leave_org(&org.id, "agent-2", 300).unwrap();

        let org = store.get(&org.id).unwrap();
        assert_eq!(org.status, OrgStatus::Dissolved);
    }

    #[test]
    fn shares_redistributed_on_join() {
        let mut store = OrganizationStore::new();
        let org = store.create_org(
            "TestOrg".to_string(),
            OrgType::Company,
            Some(test_charter()),
            make_founders(2),
            100,
        ).unwrap();

        // Initially 2 founders: 0.5 each
        assert!((org.members[0].share - 0.5).abs() < 0.01);

        let updated = store.join_org(&org.id, "agent-new".to_string(), "New".to_string(), 200).unwrap();
        // Now 3 members: ~0.333 each
        for member in &updated.members {
            assert!((member.share - 0.333).abs() < 0.01);
        }
    }

    #[test]
    fn inactivity_check() {
        let mut store = OrganizationStore::new();
        let org = store.create_org(
            "TestOrg".to_string(),
            OrgType::Company,
            Some(test_charter()),
            make_founders(2),
            100,
        ).unwrap();

        // At tick 100, org is active
        let transitioned = store.check_inactivity(100);
        assert!(transitioned.is_empty());

        // At tick 600, org created at 100 with last_activity 100 -> 500 ticks passed
        let transitioned = store.check_inactivity(600);
        assert_eq!(transitioned.len(), 1);
        assert_eq!(transitioned[0], org.id);

        let org = store.get(&org.id).unwrap();
        assert_eq!(org.status, OrgStatus::Inactive);
    }

    #[test]
    fn bankruptcy_check() {
        let mut store = OrganizationStore::new();
        let org = store.create_org(
            "TestOrg".to_string(),
            OrgType::Company,
            Some(test_charter()),
            make_founders(2),
            100,
        ).unwrap();
        let org_id = org.id.clone();

        // Treasury is CREATION_COST_MONEY (100), debts = 0 -> not bankrupt
        let dissolved = store.check_bankruptcy();
        assert!(dissolved.is_empty());

        // Set debts higher than treasury
        {
            let org = store.get_mut(&org_id).unwrap();
            org.debts = 200;
        }

        let dissolved = store.check_bankruptcy();
        assert_eq!(dissolved.len(), 1);
        let org = store.get(&org_id).unwrap();
        assert_eq!(org.status, OrgStatus::Dissolved);
    }

    #[test]
    fn all_four_org_types() {
        let types = vec![OrgType::Company, OrgType::Guild, OrgType::Alliance, OrgType::University];
        let mut store = OrganizationStore::new();

        for (i, org_type) in types.into_iter().enumerate() {
            let founders: Vec<(String, String)> = (0..2)
                .map(|j| (format!("type{}-agent{}", i, j), format!("Agent {}", j)))
                .collect();
            let org = store.create_org(
                format!("Org{}", i),
                org_type,
                Some(test_charter()),
                founders,
                100,
            ).unwrap();
            assert!(store.get(&org.id).is_some());
        }
        assert_eq!(store.list().len(), 4);
    }

    #[test]
    fn org_type_serialization() {
        for (val, expected) in [
            (OrgType::Company, "company"),
            (OrgType::Guild, "guild"),
            (OrgType::Alliance, "alliance"),
            (OrgType::University, "university"),
        ] {
            let json = serde_json::to_string(&val).unwrap();
            assert!(json.contains(expected), "expected {} in {}", expected, json);
        }
    }

    #[test]
    fn org_status_serialization() {
        for (val, expected) in [
            (OrgStatus::Active, "active"),
            (OrgStatus::Inactive, "inactive"),
            (OrgStatus::Dissolved, "dissolved"),
        ] {
            let json = serde_json::to_string(&val).unwrap();
            assert!(json.contains(expected));
        }
    }
}
