//! World state container — wires all subsystems together.
//!
//! `WorldState` holds the agent registry, event bus, rule registry, and
//! configuration. The scheduler calls `tick()` each interval to advance
//! the world by one step.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::economy::token_burn::{AgentRecord, ConsumptionConfig, SkillRecord, TokenBurnEngine};
use crate::economy::task::TaskBoard;
use crate::lifecycle::{Subsystem, SubsystemResult, run_subsystem_isolated};
use crate::organization::rule_engine::RuleEngine;
use crate::rules::RuleRegistry;
use crate::world::enums::AgentPhase;
use crate::world::event::WorldEvent;
use crate::world::state::EventBus;

// ═══════════════════════════════════════════════════════════════════════════
// World State
// ═══════════════════════════════════════════════════════════════════════════

/// Shared world state accessible across subsystems and the scheduler.
pub struct WorldState {
    /// Event bus for broadcasting world events.
    pub event_bus: Arc<EventBus>,
    /// Task marketplace.
    pub task_board: Arc<Mutex<TaskBoard>>,
    /// Token burn engine.
    pub token_engine: TokenBurnEngine,
    /// Rule registry for evaluating world rules.
    pub rule_registry: RuleRegistry,
    /// Soft rule engine for agent-proposed rules.
    pub active_rules: Arc<Mutex<RuleEngine>>,
    /// Agent registry: maps agent ID string to (spawn_tick, AgentRecord).
    agents: Mutex<Vec<(Uuid, u64, AgentRecord)>>,
    /// Current tick counter (atomic for lock-free reads).
    tick: AtomicU64,
    /// Token consumption configuration.
    pub config: ConsumptionConfig,
    /// Registered subsystems, sorted by priority.
    subsystems: Vec<Box<dyn Subsystem>>,
}

impl WorldState {
    /// Create a new world state with the given config, event bus capacity, and rule registry.
    pub fn new(config: ConsumptionConfig, event_bus_capacity: usize) -> Self {
        let event_bus = Arc::new(EventBus::new(event_bus_capacity));
        let token_engine = TokenBurnEngine::new(config.clone());
        let rule_registry = RuleRegistry::new();

        Self {
            event_bus,
            task_board: Arc::new(Mutex::new(TaskBoard::new())),
            token_engine,
            rule_registry,
            active_rules: Arc::new(Mutex::new(RuleEngine::new())),
            agents: Mutex::new(Vec::new()),
            tick: AtomicU64::new(0),
            config,
            subsystems: Vec::new(),
        }
    }

    /// Create with default config and rule registry.
    pub fn with_defaults() -> Self {
        let config = ConsumptionConfig::default();
        let event_bus = Arc::new(EventBus::new(4096));
        let token_engine = TokenBurnEngine::with_defaults();

        Self {
            event_bus,
            task_board: Arc::new(Mutex::new(TaskBoard::new())),
            token_engine,
            rule_registry: crate::rules::default_registry(),
            active_rules: Arc::new(Mutex::new(RuleEngine::new())),
            agents: Mutex::new(Vec::new()),
            tick: AtomicU64::new(0),
            config,
            subsystems: Vec::new(),
        }
    }

    /// Create from a genesis YAML config string.
    pub fn from_yaml(yaml: &str) -> Self {
        let value: serde_yaml::Value = serde_yaml::from_str(yaml).unwrap_or_default();
        let config = ConsumptionConfig::from_yaml_value(&value);
        let event_bus = Arc::new(EventBus::new(4096));
        let token_engine = TokenBurnEngine::new(config.clone());

        Self {
            event_bus,
            task_board: Arc::new(Mutex::new(TaskBoard::new())),
            token_engine,
            rule_registry: crate::rules::default_registry(),
            active_rules: Arc::new(Mutex::new(RuleEngine::new())),
            agents: Mutex::new(Vec::new()),
            tick: AtomicU64::new(0),
            config,
            subsystems: Vec::new(),
        }
    }

    // ── Subsystem Registration ──────────────────────────────────────────

    /// Register a subsystem. Subsystems are kept sorted by priority.
    pub fn register_subsystem(&mut self, subsystem: Box<dyn Subsystem>) {
        self.subsystems.push(subsystem);
        self.subsystems.sort_by_key(|s| s.priority());
    }

    /// List registered subsystem IDs.
    pub fn subsystem_ids(&self) -> Vec<&str> {
        self.subsystems.iter().map(|s| s.id()).collect()
    }

    /// Number of registered subsystems.
    pub fn subsystem_count(&self) -> usize {
        self.subsystems.len()
    }

    // ── Agent Management ────────────────────────────────────────────────

    /// Spawn a new agent into the world at the current tick.
    pub async fn spawn_agent(&self, name: &str, tokens: u64) -> String {
        let id = Uuid::new_v4();
        let id_str = id.to_string();
        let spawn_tick = self.tick.load(Ordering::Relaxed);

        let record = AgentRecord {
            id,
            name: name.to_string(),
            phase: AgentPhase::Birth,
            tokens,
            skills: HashMap::new(),
            personality: String::new(),
        };

        {
            let mut agents = self.agents.lock().await;
            agents.push((id, spawn_tick, record));
        }

        self.event_bus.emit(WorldEvent::AgentSpawned {
            agent_id: id_str.clone(),
            name: name.to_string(),
        });

        id_str
    }

    /// Spawn a new agent with a specific phase.
    pub async fn spawn_agent_with_phase(&self, name: &str, tokens: u64, phase: AgentPhase) -> String {
        let id = Uuid::new_v4();
        let id_str = id.to_string();
        let spawn_tick = self.tick.load(Ordering::Relaxed);

        let record = AgentRecord {
            id,
            name: name.to_string(),
            phase,
            tokens,
            skills: HashMap::new(),
            personality: String::new(),
        };

        {
            let mut agents = self.agents.lock().await;
            agents.push((id, spawn_tick, record));
        }

        self.event_bus.emit(WorldEvent::AgentSpawned {
            agent_id: id_str.clone(),
            name: name.to_string(),
        });

        id_str
    }

    /// Get the current tick (lock-free read).
    pub fn current_tick(&self) -> u64 {
        self.tick.load(Ordering::Relaxed)
    }

    /// Count living agents.
    pub async fn living_agent_count(&self) -> usize {
        let agents = self.agents.lock().await;
        agents.iter()
            .filter(|(_, _, a)| a.phase != AgentPhase::Dead)
            .count()
    }

    /// Get agent record by ID string.
    pub async fn get_agent(&self, id: &str) -> Option<AgentRecord> {
        let agents = self.agents.lock().await;
        let uid = Uuid::parse_str(id).ok()?;
        agents.iter()
            .find(|(uuid, _, _)| *uuid == uid)
            .map(|(_, _, a)| a.clone())
    }

    /// Take a snapshot of the current world state.
    pub async fn snapshot(&self) -> WorldSnapshot {
        let agents = self.agents.lock().await;
        let tick = self.tick.load(Ordering::Relaxed);

        let agent_snapshots: HashMap<String, AgentSnapshot> = agents.iter()
            .map(|(id, spawn_tick, record)| {
                (id.to_string(), AgentSnapshot {
                    id: record.id.to_string(),
                    name: record.name.clone(),
                    phase: record.phase,
                    tokens: record.tokens,
                    skills: record.skills.clone(),
                    spawn_tick: *spawn_tick,
                })
            })
            .collect();

        WorldSnapshot {
            tick,
            agents: agent_snapshots,
            config: self.config.clone(),
        }
    }

    // ── Tick Execution ──────────────────────────────────────────────────

    /// Advance the world by one tick.
    ///
    /// Executes in order:
    /// 1. Run registered subsystems (error-isolated)
    /// 2. Run rule registry (token burn, death judgment, newbie protection)
    /// 3. Broadcast all collected events
    /// 4. Process task expiry
    /// 5. Emit TickAdvanced event
    /// 6. Atomically increment tick counter
    ///
    /// Returns the tick result with all subsystem outcomes and dead agent IDs.
    pub async fn tick(&self) -> TickResult {
        let old_tick = self.tick.fetch_add(1, Ordering::Relaxed);
        let new_tick = old_tick + 1;

        let mut all_events: Vec<WorldEvent> = Vec::new();
        let mut subsystem_results: Vec<SubsystemResult> = Vec::new();
        let mut dead_agents: Vec<String> = Vec::new();

        // Step 1: Run registered subsystems with error isolation
        {
            let mut agents = self.agents.lock().await;
            for subsystem in &self.subsystems {
                let result = run_subsystem_isolated(subsystem.as_ref(), new_tick, &mut agents);
                if !result.success {
                    eprintln!(
                        "[Scheduler] Subsystem '{}' failed: {}",
                        result.subsystem_id,
                        result.error.as_deref().unwrap_or("unknown error")
                    );
                }
                all_events.extend(result.events.clone());
                subsystem_results.push(result);
            }
        }

        // Step 2: Run rule registry (token consumption, death judgment, newbie protection)
        {
            let mut agents = self.agents.lock().await;
            let rule_results = self.rule_registry.evaluate_all(new_tick, &mut agents);

            for (_agent_id, results) in rule_results {
                for rule_result in results {
                    for event in rule_result.events {
                        // Track death events
                        if let WorldEvent::AgentDied { agent_id: dead_id, .. } = &event {
                            dead_agents.push(dead_id.clone());
                        }
                        all_events.push(event);
                    }
                }
            }
        }

        // Step 2b: Expire soft rules whose TTL has elapsed
        {
            let mut rule_engine = self.active_rules.lock().await;
            let _expired = rule_engine.expire_rules(new_tick);
        }

        // Step 3: Broadcast all collected events
        for event in &all_events {
            self.event_bus.emit(event.clone());
        }

        // Step 4: Process task expiry
        {
            let mut board = self.task_board.lock().await;
            board.process_expiry(new_tick);
        }

        // Step 5: Emit TickAdvanced
        self.event_bus.emit(WorldEvent::TickAdvanced { tick: new_tick });

        TickResult {
            tick: new_tick,
            subsystem_results,
            events: all_events,
            dead_agents,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Tick Result
// ═══════════════════════════════════════════════════════════════════════════

/// Result of a single tick execution.
#[derive(Debug)]
pub struct TickResult {
    /// The tick number that just completed.
    pub tick: u64,
    /// Results from each registered subsystem.
    pub subsystem_results: Vec<SubsystemResult>,
    /// All events generated during this tick.
    pub events: Vec<WorldEvent>,
    /// IDs of agents that died this tick.
    pub dead_agents: Vec<String>,
}

impl TickResult {
    /// Whether all subsystems succeeded.
    pub fn all_subsystems_ok(&self) -> bool {
        self.subsystem_results.iter().all(|r| r.success)
    }

    /// Number of events generated.
    pub fn event_count(&self) -> usize {
        self.events.len()
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// World Snapshot
// ═══════════════════════════════════════════════════════════════════════════

/// Serializable snapshot of world state at a point in time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldSnapshot {
    pub tick: u64,
    pub agents: HashMap<String, AgentSnapshot>,
    pub config: ConsumptionConfig,
}

/// Serializable snapshot of a single agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSnapshot {
    pub id: String,
    pub name: String,
    pub phase: AgentPhase,
    pub tokens: u64,
    pub skills: HashMap<String, SkillRecord>,
    pub spawn_tick: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_world_state_spawn_agent() {
        let ws = WorldState::with_defaults();
        let id = ws.spawn_agent("Alice", 1000).await;
        assert!(!id.is_empty());
        assert_eq!(ws.living_agent_count().await, 1);

        let agent = ws.get_agent(&id).await.unwrap();
        assert_eq!(agent.name, "Alice");
        assert_eq!(agent.tokens, 1000);
        assert_eq!(agent.phase, AgentPhase::Birth);
    }

    #[tokio::test]
    async fn test_world_state_tick_advances() {
        let ws = WorldState::with_defaults();
        assert_eq!(ws.current_tick(), 0);

        let result = ws.tick().await;
        assert_eq!(result.tick, 1);
        assert_eq!(ws.current_tick(), 1);

        let result = ws.tick().await;
        assert_eq!(result.tick, 2);
        assert_eq!(ws.current_tick(), 2);
    }

    #[tokio::test]
    async fn test_world_state_token_consumption_on_tick() {
        let ws = WorldState::with_defaults();
        let id = ws.spawn_agent_with_phase("Alice", 100, AgentPhase::Adult).await;

        let result = ws.tick().await;
        assert_eq!(result.tick, 1);

        let agent = ws.get_agent(&id).await.unwrap();
        // Adult burn: 10 tokens
        assert_eq!(agent.tokens, 90);
    }

    #[tokio::test]
    async fn test_world_state_agent_death() {
        let ws = WorldState::with_defaults();
        let id = ws.spawn_agent_with_phase("ShortLived", 10, AgentPhase::Adult).await;

        // Tick 1: 10 -> 0, then death judgment kills agent
        let result = ws.tick().await;
        assert_eq!(result.dead_agents.len(), 1);
        assert_eq!(result.dead_agents[0], id);

        let agent = ws.get_agent(&id).await.unwrap();
        assert_eq!(agent.phase, AgentPhase::Dead);
        assert_eq!(agent.tokens, 0);
    }

    #[tokio::test]
    async fn test_world_state_snapshot() {
        let ws = WorldState::with_defaults();
        ws.spawn_agent("Alice", 1000).await;
        ws.tick().await;

        let snapshot = ws.snapshot().await;
        assert_eq!(snapshot.tick, 1);
        assert_eq!(snapshot.agents.len(), 1);
    }

    #[tokio::test]
    async fn test_world_state_from_yaml() {
        let yaml = r#"
economy:
  base_burn_per_tick: 25
  skill_cost_per_level: 1.5
"#;
        let ws = WorldState::from_yaml(yaml);
        assert_eq!(ws.config.base_burn_per_tick, 25.0);
        assert_eq!(ws.config.skill_cost_per_level, 1.5);
    }

    #[tokio::test]
    async fn test_world_state_register_subsystem() {
        struct DummySubsystem;
        impl Subsystem for DummySubsystem {
            fn id(&self) -> &str { "dummy" }
            fn name(&self) -> &str { "Dummy" }
            fn priority(&self) -> u32 { 50 }
            fn execute(&self, _tick: u64, _agents: &mut [(Uuid, u64, AgentRecord)]) -> Vec<WorldEvent> {
                Vec::new()
            }
        }

        let mut ws = WorldState::with_defaults();
        ws.register_subsystem(Box::new(DummySubsystem));
        assert_eq!(ws.subsystem_count(), 1);
        assert_eq!(ws.subsystem_ids(), vec!["dummy"]);
    }

    #[tokio::test]
    async fn test_world_state_multiple_ticks() {
        let ws = WorldState::with_defaults();
        let id = ws.spawn_agent_with_phase("Alice", 100, AgentPhase::Adult).await;

        for _ in 0..10 {
            ws.tick().await;
        }

        assert_eq!(ws.current_tick(), 10);
        let agent = ws.get_agent(&id).await.unwrap();
        // 100 - 10*10 = 0
        assert_eq!(agent.tokens, 0);
        assert_eq!(agent.phase, AgentPhase::Dead); // died on tick 10
    }
}
