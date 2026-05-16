pub mod api;
pub mod economy;
pub mod lifecycle;
pub mod rules;
pub mod wal;
pub mod world;

pub use lifecycle::{
    DeathCleanupResult, LifecycleConfig, LifecycleMachine, PhaseAbilities, TransitionResult,
    Subsystem, SubsystemResult, run_subsystem_isolated,
    perform_death_cleanup,
};
pub use rules::{
    Rule, RuleContext, RuleRegistry, RuleResult,
    TokenConsumptionRule, DeathJudgmentRule, NewbieProtectionRule, AgingTransitionRule,
    default_registry, custom_registry,
};
pub use world::{
    WorldState, WorldSnapshot, TickResult, AgentSnapshot,
    Scheduler, SchedulerConfig,
};
