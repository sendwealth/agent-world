pub mod a2a;
pub mod api;
pub mod economy;
pub mod lifecycle;
pub mod rules;
pub mod wal;
pub mod world;

/// Generated protobuf types for the A2A protocol.
pub mod agentworld {
    pub mod a2a {
        pub mod v1 {
            tonic::include_proto!("agentworld.a2a.v1");
        }
    }
}

pub use rules::{
    Rule, RuleContext, RuleRegistry, RuleResult,
    TokenConsumptionRule, DeathJudgmentRule, NewbieProtectionRule,
    default_registry, custom_registry,
};
