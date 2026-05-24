use serde::{Deserialize, Serialize};
use std::fmt;

// ── Capability enum (9 operations from DESIGN.md §12.1) ──────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    ViewWorld,
    ViewAgents,
    Invest,
    PublishTasks,
    Arbitrate,
    ModifyWorldParams,
    CreateAgent,
    PauseWorld,
    ResetWorld,
}

impl fmt::Display for Capability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Capability::ViewWorld => write!(f, "view_world"),
            Capability::ViewAgents => write!(f, "view_agents"),
            Capability::Invest => write!(f, "invest"),
            Capability::PublishTasks => write!(f, "publish_tasks"),
            Capability::Arbitrate => write!(f, "arbitrate"),
            Capability::ModifyWorldParams => write!(f, "modify_world_params"),
            Capability::CreateAgent => write!(f, "create_agent"),
            Capability::PauseWorld => write!(f, "pause_world"),
            Capability::ResetWorld => write!(f, "reset_world"),
        }
    }
}

// ── HumanRole enum (6 roles from DESIGN.md §12.1) ────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HumanRole {
    Observer,      // 观察者 — read only
    Investor,      // 投资者 — invest in agents
    TaskPublisher, // 任务发布者 — publish bounties
    Arbiter,       // 仲裁者 — resolve disputes
    Experimenter,  // 实验者 — modify world params
    Creator,       // 创建者 — full access
}

impl HumanRole {
    /// Returns the capabilities granted to this role per DESIGN.md §12.1.
    pub fn capabilities(&self) -> &'static [Capability] {
        match self {
            HumanRole::Observer => &[
                Capability::ViewWorld,
                Capability::ViewAgents,
            ],
            HumanRole::Investor => &[
                Capability::ViewWorld,
                Capability::ViewAgents,
                Capability::Invest,
            ],
            HumanRole::TaskPublisher => &[
                Capability::ViewWorld,
                Capability::ViewAgents,
                Capability::PublishTasks,
            ],
            HumanRole::Arbiter => &[
                Capability::ViewWorld,
                Capability::ViewAgents,
                Capability::Arbitrate,
            ],
            HumanRole::Experimenter => &[
                Capability::ViewWorld,
                Capability::ViewAgents,
                Capability::ModifyWorldParams,
            ],
            HumanRole::Creator => &[
                Capability::ViewWorld,
                Capability::ViewAgents,
                Capability::Invest,
                Capability::PublishTasks,
                Capability::Arbitrate,
                Capability::ModifyWorldParams,
                Capability::CreateAgent,
                Capability::PauseWorld,
                Capability::ResetWorld,
            ],
        }
    }

    /// Check whether this role has a specific capability.
    pub fn has_capability(&self, cap: Capability) -> bool {
        self.capabilities().contains(&cap)
    }
}

impl fmt::Display for HumanRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HumanRole::Observer => write!(f, "observer"),
            HumanRole::Investor => write!(f, "investor"),
            HumanRole::TaskPublisher => write!(f, "task_publisher"),
            HumanRole::Arbiter => write!(f, "arbiter"),
            HumanRole::Experimenter => write!(f, "experimenter"),
            HumanRole::Creator => write!(f, "creator"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_observer_capabilities() {
        let role = HumanRole::Observer;
        assert!(role.has_capability(Capability::ViewWorld));
        assert!(role.has_capability(Capability::ViewAgents));
        assert!(!role.has_capability(Capability::Invest));
        assert!(!role.has_capability(Capability::PublishTasks));
        assert!(!role.has_capability(Capability::Arbitrate));
        assert!(!role.has_capability(Capability::ModifyWorldParams));
        assert!(!role.has_capability(Capability::CreateAgent));
        assert!(!role.has_capability(Capability::PauseWorld));
        assert!(!role.has_capability(Capability::ResetWorld));
        assert_eq!(role.capabilities().len(), 2);
    }

    #[test]
    fn test_investor_capabilities() {
        let role = HumanRole::Investor;
        assert!(role.has_capability(Capability::ViewWorld));
        assert!(role.has_capability(Capability::ViewAgents));
        assert!(role.has_capability(Capability::Invest));
        assert!(!role.has_capability(Capability::PublishTasks));
        assert_eq!(role.capabilities().len(), 3);
    }

    #[test]
    fn test_task_publisher_capabilities() {
        let role = HumanRole::TaskPublisher;
        assert!(role.has_capability(Capability::PublishTasks));
        assert!(!role.has_capability(Capability::Invest));
        assert_eq!(role.capabilities().len(), 3);
    }

    #[test]
    fn test_arbiter_capabilities() {
        let role = HumanRole::Arbiter;
        assert!(role.has_capability(Capability::Arbitrate));
        assert!(!role.has_capability(Capability::Invest));
        assert_eq!(role.capabilities().len(), 3);
    }

    #[test]
    fn test_experimenter_capabilities() {
        let role = HumanRole::Experimenter;
        assert!(role.has_capability(Capability::ModifyWorldParams));
        assert!(!role.has_capability(Capability::Invest));
        assert_eq!(role.capabilities().len(), 3);
    }

    #[test]
    fn test_creator_has_all() {
        let role = HumanRole::Creator;
        assert_eq!(role.capabilities().len(), 9);
        for cap in [
            Capability::ViewWorld,
            Capability::ViewAgents,
            Capability::Invest,
            Capability::PublishTasks,
            Capability::Arbitrate,
            Capability::ModifyWorldParams,
            Capability::CreateAgent,
            Capability::PauseWorld,
            Capability::ResetWorld,
        ] {
            assert!(role.has_capability(cap), "Creator should have {:?}", cap);
        }
    }

    #[test]
    fn test_full_permission_matrix() {
        // DESIGN.md §12.1: 9 operations × 6 roles = 54 checks
        let matrix: [(HumanRole, Capability, bool); 54] = [
            // Observer
            (HumanRole::Observer, Capability::ViewWorld, true),
            (HumanRole::Observer, Capability::ViewAgents, true),
            (HumanRole::Observer, Capability::Invest, false),
            (HumanRole::Observer, Capability::PublishTasks, false),
            (HumanRole::Observer, Capability::Arbitrate, false),
            (HumanRole::Observer, Capability::ModifyWorldParams, false),
            (HumanRole::Observer, Capability::CreateAgent, false),
            (HumanRole::Observer, Capability::PauseWorld, false),
            (HumanRole::Observer, Capability::ResetWorld, false),
            // Investor
            (HumanRole::Investor, Capability::ViewWorld, true),
            (HumanRole::Investor, Capability::ViewAgents, true),
            (HumanRole::Investor, Capability::Invest, true),
            (HumanRole::Investor, Capability::PublishTasks, false),
            (HumanRole::Investor, Capability::Arbitrate, false),
            (HumanRole::Investor, Capability::ModifyWorldParams, false),
            (HumanRole::Investor, Capability::CreateAgent, false),
            (HumanRole::Investor, Capability::PauseWorld, false),
            (HumanRole::Investor, Capability::ResetWorld, false),
            // TaskPublisher
            (HumanRole::TaskPublisher, Capability::ViewWorld, true),
            (HumanRole::TaskPublisher, Capability::ViewAgents, true),
            (HumanRole::TaskPublisher, Capability::Invest, false),
            (HumanRole::TaskPublisher, Capability::PublishTasks, true),
            (HumanRole::TaskPublisher, Capability::Arbitrate, false),
            (HumanRole::TaskPublisher, Capability::ModifyWorldParams, false),
            (HumanRole::TaskPublisher, Capability::CreateAgent, false),
            (HumanRole::TaskPublisher, Capability::PauseWorld, false),
            (HumanRole::TaskPublisher, Capability::ResetWorld, false),
            // Arbiter
            (HumanRole::Arbiter, Capability::ViewWorld, true),
            (HumanRole::Arbiter, Capability::ViewAgents, true),
            (HumanRole::Arbiter, Capability::Invest, false),
            (HumanRole::Arbiter, Capability::PublishTasks, false),
            (HumanRole::Arbiter, Capability::Arbitrate, true),
            (HumanRole::Arbiter, Capability::ModifyWorldParams, false),
            (HumanRole::Arbiter, Capability::CreateAgent, false),
            (HumanRole::Arbiter, Capability::PauseWorld, false),
            (HumanRole::Arbiter, Capability::ResetWorld, false),
            // Experimenter
            (HumanRole::Experimenter, Capability::ViewWorld, true),
            (HumanRole::Experimenter, Capability::ViewAgents, true),
            (HumanRole::Experimenter, Capability::Invest, false),
            (HumanRole::Experimenter, Capability::PublishTasks, false),
            (HumanRole::Experimenter, Capability::Arbitrate, false),
            (HumanRole::Experimenter, Capability::ModifyWorldParams, true),
            (HumanRole::Experimenter, Capability::CreateAgent, false),
            (HumanRole::Experimenter, Capability::PauseWorld, false),
            (HumanRole::Experimenter, Capability::ResetWorld, false),
            // Creator
            (HumanRole::Creator, Capability::ViewWorld, true),
            (HumanRole::Creator, Capability::ViewAgents, true),
            (HumanRole::Creator, Capability::Invest, true),
            (HumanRole::Creator, Capability::PublishTasks, true),
            (HumanRole::Creator, Capability::Arbitrate, true),
            (HumanRole::Creator, Capability::ModifyWorldParams, true),
            (HumanRole::Creator, Capability::CreateAgent, true),
            (HumanRole::Creator, Capability::PauseWorld, true),
            (HumanRole::Creator, Capability::ResetWorld, true),
        ];

        for (role, cap, expected) in matrix {
            assert_eq!(
                role.has_capability(cap),
                expected,
                "{:?}.has_capability({:?}) should be {}",
                role, cap, expected
            );
        }
    }
}
