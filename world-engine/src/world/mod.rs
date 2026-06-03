//! # World Core
//!
//! Simulation kernel: agents, hex map, event bus, tick scheduler,
//! subsystem registry, seeder, genesis, and intervention guard.
//!
//! Key types: WorldState, EventBus, Agent, AgentRegistry, WorldMap,
//!            HexPos, Scheduler, GenesisConfig, SubsystemRegistry
//! Depends on: economy, organization, engine, rules, lifecycle
//!
pub mod agent;
pub mod discovery;
pub mod engine;
pub mod enums;
pub mod event;
pub mod genesis;
pub mod intervention;
pub mod map;
pub mod scheduler;
pub mod seeder;
pub mod state;
pub mod subsystem;
pub mod subsystems;
pub mod tick_profiler;

pub use agent::{Agent, AgentRecord, AgentRegistry, SkillRecord};
pub use event::{EventType, WorldEvent, TrustInteractionType};
pub use genesis::GenesisConfig;
pub use intervention::{InterventionCheckerSubsystem, InterventionConfig as InterventionSubsystemConfig, MessageInterventionGuard};
pub use scheduler::Scheduler;
pub use seeder::{WorldSeeder, Terrain, Resource};
pub use map::{HexPos, TerrainType, Tile, ResourceNode, WorldMap, MapSnapshot};
pub use state::{EventBus, FilteredReceiver, SharedEventBus, WorldState};
pub use subsystem::{Subsystem, SubsystemRegistry};
pub use subsystems::{
    TokenBurnSubsystem, DeathJudgmentSubsystem,
    RuleCheckSubsystem, EventBroadcastSubsystem,
    TrustDecaySubsystem, MentorshipProgressSubsystem, EscrowExpirySubsystem,
};
pub use tick_profiler::{TickProfiler, TickPhase, TickProfileReport, TickTiming};
