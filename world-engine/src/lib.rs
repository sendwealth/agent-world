pub mod api;
pub mod economy;
pub mod lifecycle;
pub mod rules;
pub mod wal;
pub mod world;

pub use rules::{
    Rule, RuleContext, RuleRegistry, RuleResult,
    TokenConsumptionRule, DeathJudgmentRule, NewbieProtectionRule,
    default_registry, custom_registry,
};

pub use world::discovery::{
    AgentProfile, AgentRegistry, AgentStatus, DiscoveryError, SharedAgentRegistry,
};
