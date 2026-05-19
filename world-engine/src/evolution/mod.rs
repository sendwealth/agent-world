//! Evolution subsystem — skill trees, mutations, and natural selection.
//!
//! This module implements the evolution mechanics for agents:
//! - **Skill tree** with branching paths (e.g. coding → frontend/backend/systems)
//! - **Skill depth** levels 1–10 with exponential XP thresholds
//! - **Skill mutations** occurring with configurable probability each evaluation cycle
//! - **Natural selection** evaluating agent fitness and applying culling pressure
//! - **Decline** for agents inactive beyond a threshold

pub mod skill_tree;
pub mod mutation;
pub mod selection;
pub mod subsystem;

pub use skill_tree::{SkillTree, SkillBranch, SkillNode};
pub use mutation::{MutationEngine, MutationType, MutationOutcome};
pub use selection::{FitnessEvaluator, FitnessReport, SelectionEngine};
pub use subsystem::EvolutionSubsystem;
