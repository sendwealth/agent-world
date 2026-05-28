//! Evolution subsystem — skill trees, mutations, crossover, and natural selection.
//!
//! This module implements the evolution mechanics for agents:
//! - **Skill tree** with branching paths (e.g. coding → frontend/backend/systems)
//! - **Skill depth** levels 1–10 with exponential XP thresholds
//! - **Skill mutations** occurring with configurable probability each evaluation cycle
//! - **Offspring mutations** — personality shift, skill level anomaly, novel skills
//! - **Environmental pressure** — resource scarcity increases mutation rate
//! - **Heritable mutations** — parent mutations can strengthen or disappear
//! - **Crossover** — combining two parents' skills and personality
//! - **Natural selection** evaluating agent fitness and applying culling pressure
//! - **Decline** for agents inactive beyond a threshold

pub mod crossover;
pub mod mutation;
pub mod selection;
pub mod skill_tree;
pub mod subsystem;

pub use crossover::{CrossoverConfig, CrossoverEngine, CrossoverResult, CrossoverStrategy};
pub use mutation::{
    EnvironmentPressure, HeritableMutation, MutationEngine, MutationOutcome, MutationType,
    OffspringMutation, OffspringMutationConfig, OffspringMutationResult, OffspringMutationType,
};
pub use selection::{FitnessEvaluator, FitnessReport, SelectionEngine};
pub use skill_tree::{SkillBranch, SkillNode, SkillTree};
pub use subsystem::EvolutionSubsystem;
