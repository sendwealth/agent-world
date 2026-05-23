pub mod a2a;
pub mod api;
pub mod config;
pub mod economy;
pub mod engine;
pub mod evolution;
pub mod grpc_pool;
pub mod human;
pub mod lifecycle;
pub mod organization;
pub mod persistence;
pub mod rules;
pub mod time_capsule;
pub mod tracing;
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
    Rule, RuleCategory, RuleConflictPolicy, RuleContext, RuleRegistry, RuleResult,
    TokenConsumptionRule, DeathJudgmentRule, NewbieProtectionRule,
    VoluntaryTradingRule, AntiMonopolyRule, DebtCeilingRule,
    CommunicationHonestyRule, ContractBindingRule,
    ResourceExhaustionRule, ReproductionRunawayRule,
    default_registry, custom_registry, custom_registry_full,
};

pub use world::discovery::{
    AgentProfile, AgentRegistry, AgentStatus, DiscoveryError, SharedAgentRegistry,
};

pub use world::{
    EventBus, FilteredReceiver, SharedEventBus, WorldEvent, EventType,
    WorldState, Subsystem, SubsystemRegistry,
    Scheduler,
    GenesisConfig,
    TokenBurnSubsystem, DeathJudgmentSubsystem,
    RuleCheckSubsystem, EventBroadcastSubsystem,
    TickProfiler, TickPhase, TickProfileReport, TickTiming,
};

pub use evolution::EvolutionSubsystem;

pub use persistence::{
    SerializableAgentEntry, SerializableAgentRecord, SerializableWorldState,
    SqlitePersistence, StatePersistence,
};
