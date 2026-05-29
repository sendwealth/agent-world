//! # agent-world-engine
//!
//! Open-world AI-agent survival simulation engine.
//! Comprises: world core, economy, organizations, A2A protocol,
//! evolution, federation, auth, persistence, and REST/gRPC APIs.
//!
//! Entry points: [`api::build_router`] (REST), [`main`] (server bootstrap)
//!
pub mod a2a;
pub mod api;
pub mod api_ab_experiment;
pub mod api_agents_ext;
pub mod api_auth;
pub mod api_auth_handlers;
pub mod api_bank;
pub mod api_behavior_log;
pub mod api_buildings;
pub mod api_diplomacy;
pub mod api_dsl;
pub mod api_experiment;
pub mod api_export;
pub mod api_export_v1;
pub mod api_federation;
pub mod api_governance;
pub mod api_human;
pub mod api_investment;
pub mod api_marketplace;
pub mod api_network_graph;
pub mod api_org;
pub mod api_population;
pub mod api_plugins;
pub mod api_report;
pub mod api_reputation;
pub mod api_research;
pub mod api_stocks;
pub mod api_tasks;
pub mod api_coordination_tasks;
pub mod api_traces;
pub mod api_world;
pub mod auth;
pub mod config;
pub mod dsl;
pub mod economy;
pub mod error;
pub mod engine;
pub mod evolution;
pub mod federation;
pub mod grpc_pool;
pub mod human;
pub mod lifecycle;
pub mod observability;
pub mod organization;
pub mod persistence;
pub mod plugin;
pub mod rules;
pub mod snapshot;
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
    pub mod federation {
        pub mod v1 {
            tonic::include_proto!("agentworld.federation.v1");
        }
    }
}

pub use rules::{
    custom_registry, custom_registry_full, default_registry, AntiMonopolyRule,
    CommunicationHonestyRule, ContractBindingRule, DeathJudgmentRule, DebtCeilingRule,
    NewbieProtectionRule, ReproductionRunawayRule, ResourceExhaustionRule, Rule, RuleCategory,
    RuleConflictPolicy, RuleContext, RuleRegistry, RuleResult, TokenConsumptionRule,
    VoluntaryTradingRule,
};

pub use world::discovery::{
    AgentProfile, AgentRegistry, AgentStatus, DiscoveryError, SharedAgentRegistry,
};

pub use world::{
    DeathJudgmentSubsystem, EventBroadcastSubsystem, EventBus, EventType, FilteredReceiver,
    GenesisConfig, RuleCheckSubsystem, Scheduler, SharedEventBus, Subsystem, SubsystemRegistry,
    TickPhase, TickProfileReport, TickProfiler, TickTiming, TokenBurnSubsystem, WorldEvent,
    WorldState,
};

pub use evolution::EvolutionSubsystem;

pub use dsl::{
    builtin_templates, get_template, parse_json, parse_yaml, parse_yaml_multi, to_json,
    to_rule_conditions, to_rule_effects, to_rule_type, to_yaml, DslAction, DslCondition, DslRule,
    ParseResult, RuleScope, RuleTemplate, TriggerConfig,
};

pub use persistence::{
    SerializableAgentEntry, SerializableAgentRecord, SerializableWorldState, SqlitePersistence,
    StatePersistence,
};

pub use snapshot::{
    AgentSnapshot, CompressionStats, SnapshotConfig, SnapshotDelta, SnapshotEngine,
    SnapshotEngineHandle, SnapshotKind, SnapshotRecord, SnapshotRequest, SnapshotStorage,
    WorldSnapshot,
};

pub use plugin::{
    HookResult, OnAgentAction, OnAgentSpawn, OnEvent, OnShutdown, OnStartup, OnTickEnd,
    OnTickStart, OnTransaction, PluginContext, PluginError, PluginHooks, PluginInfo,
    PluginManager, PluginMetadata, PluginRegistry, PluginState, PluginSubsystemBridge,
    Permission, PermissionSet, SharedPluginManager, TransactionContext,
};
