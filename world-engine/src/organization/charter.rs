use serde::{Deserialize, Serialize};

/// Organization charter — the founding document that defines governance rules.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Charter {
    /// Mission statement / purpose description.
    pub purpose: String,
    /// Governance model.
    pub governance: GovernanceModel,
    /// Profit sharing mode.
    pub profit_sharing: ProfitSharing,
    /// Monthly membership fee in Money (0 = no fee).
    #[serde(default)]
    pub membership_fee: u64,
}

/// How decisions are made within the organization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GovernanceModel {
    /// All members vote on decisions.
    Vote,
    /// Single leader decides.
    Dictator,
    /// Council of leaders decides.
    Council,
}

/// How profits are distributed among members.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfitSharing {
    /// Equal split among all members.
    Equal,
    /// Proportional to member share.
    Proportional,
    /// Custom distribution defined in charter.
    Custom,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn charter_round_trip() {
        let charter = Charter {
            purpose: "Build great software".to_string(),
            governance: GovernanceModel::Vote,
            profit_sharing: ProfitSharing::Proportional,
            membership_fee: 10,
        };
        let json = serde_json::to_string(&charter).unwrap();
        let back: Charter = serde_json::from_str(&json).unwrap();
        assert_eq!(charter, back);
    }

    #[test]
    fn governance_serialization() {
        let json = serde_json::to_string(&GovernanceModel::Vote).unwrap();
        assert!(json.contains("vote"));
    }
}
