use serde::{Deserialize, Serialize};

/// Role of a member within an organization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemberRole {
    /// One of the founding members.
    Founder,
    /// A designated leader.
    Leader,
    /// Regular member.
    Member,
}

impl MemberRole {
    /// Whether this role has admin privileges (can manage members, dissolve org).
    pub fn is_admin(&self) -> bool {
        matches!(self, MemberRole::Founder | MemberRole::Leader)
    }
}

/// A single member entry in an organization.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OrgMember {
    /// The agent's ID.
    pub agent_id: String,
    /// The agent's display name (denormalized for convenience).
    pub agent_name: String,
    /// Role within the organization.
    pub role: MemberRole,
    /// Profit share ratio (0.0-1.0). All members' shares should sum to 1.0.
    pub share: f64,
    /// Tick when the member joined.
    pub joined_tick: u64,
}

/// Membership-related errors.
#[derive(Debug, thiserror::Error)]
pub enum MemberError {
    #[error("agent {0} is already a member")]
    AlreadyMember(String),
    #[error("agent {0} is not a member")]
    NotMember(String),
    #[error("cannot remove the last founder; dissolve the org instead")]
    CannotRemoveLastFounder,
    #[error("insufficient permissions: agent {0} cannot perform this action")]
    InsufficientPermissions(String),
    #[error("share must be between 0.0 and 1.0, got {0}")]
    InvalidShare(f64),
    #[error("total shares would exceed 1.0 (current: {0}, adding: {1})")]
    SharesExceedOne(f64, f64),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn member_role_admin() {
        assert!(MemberRole::Founder.is_admin());
        assert!(MemberRole::Leader.is_admin());
        assert!(!MemberRole::Member.is_admin());
    }

    #[test]
    fn member_serialization() {
        let member = OrgMember {
            agent_id: "test-agent-id".to_string(),
            agent_name: "Alice".to_string(),
            role: MemberRole::Founder,
            share: 0.5,
            joined_tick: 100,
        };
        let json = serde_json::to_string(&member).unwrap();
        let back: OrgMember = serde_json::from_str(&json).unwrap();
        assert_eq!(member, back);
    }

    #[test]
    fn role_serialization() {
        let json = serde_json::to_string(&MemberRole::Founder).unwrap();
        assert!(json.contains("founder"));
        let json = serde_json::to_string(&MemberRole::Leader).unwrap();
        assert!(json.contains("leader"));
    }
}
