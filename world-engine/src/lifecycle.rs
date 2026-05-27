//! Lifecycle subsystem — agent phase state machine and subsystem trait.
//!
//! Manages the agent lifecycle: Birth → Childhood → Adult → Elder → Dying → Dead.
//! Each phase has token consumption rates and ability ranges.
//! Phase transitions are tick-based and emit events via EventBus.
//!
//! Also defines the Subsystem trait used by the scheduler to dispatch
//! world logic in sequence with error isolation.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::economy::token_burn::AgentRecord;
use crate::world::enums::{AgentPhase, DeathReason};
use crate::world::event::WorldEvent;

// ═══════════════════════════════════════════════════════════════════════════
// Phase Ability Ranges
// ═══════════════════════════════════════════════════════════════════════════

/// Describes what an agent can do and how efficient it is in a given phase.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PhaseAbilities {
    /// Skill efficiency multiplier (0.0 to 1.0).
    pub skill_efficiency: f64,
    /// Whether the agent can learn new skills.
    pub can_learn: bool,
    /// Whether the agent can take tasks.
    pub can_take_tasks: bool,
    /// Whether the agent can trade.
    pub can_trade: bool,
    /// Whether the agent can teach others.
    pub can_teach: bool,
    /// Whether the agent can create/modify a will.
    pub can_write_will: bool,
    /// Whether the agent can communicate with others.
    pub can_communicate: bool,
}

impl PhaseAbilities {
    /// Get abilities for a given agent phase.
    pub fn for_phase(phase: AgentPhase) -> Self {
        match phase {
            AgentPhase::Birth => Self {
                skill_efficiency: 0.0,
                can_learn: true,
                can_take_tasks: false,
                can_trade: false,
                can_teach: false,
                can_write_will: false,
                can_communicate: true,
            },
            AgentPhase::Childhood => Self {
                skill_efficiency: 0.3,
                can_learn: true,
                can_take_tasks: true,
                can_trade: false,
                can_teach: false,
                can_write_will: false,
                can_communicate: true,
            },
            AgentPhase::Adult => Self {
                skill_efficiency: 1.0,
                can_learn: true,
                can_take_tasks: true,
                can_trade: true,
                can_teach: true,
                can_write_will: true,
                can_communicate: true,
            },
            AgentPhase::Elder => Self {
                skill_efficiency: 0.6,
                can_learn: true,
                can_take_tasks: true,
                can_trade: true,
                can_teach: true,
                can_write_will: true,
                can_communicate: true,
            },
            AgentPhase::Dying => Self {
                skill_efficiency: 0.1,
                can_learn: false,
                can_take_tasks: false,
                can_trade: false,
                can_teach: false,
                can_write_will: true,
                can_communicate: true,
            },
            AgentPhase::Dead => Self {
                skill_efficiency: 0.0,
                can_learn: false,
                can_take_tasks: false,
                can_trade: false,
                can_teach: false,
                can_write_will: false,
                can_communicate: false,
            },
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Lifecycle Config
// ═══════════════════════════════════════════════════════════════════════════

/// Configurable lifecycle parameters.
/// Maps to the `lifecycle` section in genesis.yaml.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LifecycleConfig {
    /// Number of ticks for the Childhood phase (after birth transition).
    #[serde(default = "default_childhood_ticks")]
    pub childhood_ticks: u64,

    /// Number of ticks for the Adult phase.
    #[serde(default = "default_adult_ticks")]
    pub adult_ticks: u64,

    /// Number of ticks for the Elder phase before natural death.
    #[serde(default = "default_elder_ticks")]
    pub elder_ticks: u64,

    /// Grace ticks after tokens hit zero before actual death.
    #[serde(default = "default_death_grace_ticks")]
    pub death_grace_ticks: u64,
}

fn default_childhood_ticks() -> u64 {
    100
}
fn default_adult_ticks() -> u64 {
    1000
}
fn default_elder_ticks() -> u64 {
    200
}
fn default_death_grace_ticks() -> u64 {
    10
}

impl Default for LifecycleConfig {
    fn default() -> Self {
        Self {
            childhood_ticks: default_childhood_ticks(),
            adult_ticks: default_adult_ticks(),
            elder_ticks: default_elder_ticks(),
            death_grace_ticks: default_death_grace_ticks(),
        }
    }
}

impl LifecycleConfig {
    /// Load from a genesis.yaml-compatible value.
    pub fn from_yaml_value(value: &serde_yaml::Value) -> Self {
        let mut config = Self::default();
        if let Some(lc) = value.get("lifecycle") {
            if let Some(v) = lc.get("childhood_ticks").and_then(|v| v.as_u64()) {
                config.childhood_ticks = v;
            }
            if let Some(v) = lc.get("adult_ticks").and_then(|v| v.as_u64()) {
                config.adult_ticks = v;
            }
            if let Some(v) = lc.get("elder_ticks").and_then(|v| v.as_u64()) {
                config.elder_ticks = v;
            }
            if let Some(v) = lc.get("death_grace_ticks").and_then(|v| v.as_u64()) {
                config.death_grace_ticks = v;
            }
        }
        config
    }

    /// Compute the tick at which Childhood ends (transition to Adult).
    /// Agent is born at `spawn_tick`, transitions Birth→Childhood at `spawn_tick + 1`.
    /// Childhood ends at `spawn_tick + 1 + childhood_ticks`.
    pub fn childhood_end_tick(&self, spawn_tick: u64) -> u64 {
        spawn_tick + 1 + self.childhood_ticks
    }

    /// Compute the tick at which Adulthood ends (transition to Elder).
    pub fn adult_end_tick(&self, spawn_tick: u64) -> u64 {
        self.childhood_end_tick(spawn_tick) + self.adult_ticks
    }

    /// Compute the tick at which Elder phase ends (natural death).
    pub fn elder_end_tick(&self, spawn_tick: u64) -> u64 {
        self.adult_end_tick(spawn_tick) + self.elder_ticks
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Lifecycle State Machine
// ═══════════════════════════════════════════════════════════════════════════

/// Possible outcomes of checking lifecycle transitions.
#[derive(Debug, Clone, PartialEq)]
pub enum TransitionResult {
    /// No transition needed.
    NoTransition,
    /// Phase changed from old to new.
    PhaseChanged {
        old_phase: AgentPhase,
        new_phase: AgentPhase,
    },
    /// Agent has died.
    Died {
        reason: DeathReason,
        old_phase: AgentPhase,
    },
}

/// The lifecycle state machine.
///
/// Determines valid transitions and evaluates tick-based aging transitions.
pub struct LifecycleMachine {
    config: LifecycleConfig,
}

impl LifecycleMachine {
    pub fn new(config: LifecycleConfig) -> Self {
        Self { config }
    }

    pub fn with_defaults() -> Self {
        Self::new(LifecycleConfig::default())
    }

    /// Check if a transition from one phase to another is valid.
    pub fn can_transition(from: AgentPhase, to: AgentPhase) -> bool {
        match (from, to) {
            // Normal lifecycle progression
            (AgentPhase::Birth, AgentPhase::Childhood) => true,
            (AgentPhase::Childhood, AgentPhase::Adult) => true,
            (AgentPhase::Adult, AgentPhase::Elder) => true,
            (AgentPhase::Elder, AgentPhase::Dead) => true,

            // Death can happen from any living phase
            (AgentPhase::Childhood, AgentPhase::Dying) => true,
            (AgentPhase::Adult, AgentPhase::Dying) => true,
            (AgentPhase::Elder, AgentPhase::Dying) => true,
            (AgentPhase::Birth, AgentPhase::Dead) => true,
            (AgentPhase::Childhood, AgentPhase::Dead) => true,
            (AgentPhase::Adult, AgentPhase::Dead) => true,

            // Dying → Dead (grace period expired)
            (AgentPhase::Dying, AgentPhase::Dead) => true,

            // Rescue: Dying back to previous phase
            (AgentPhase::Dying, AgentPhase::Adult) => true,

            // Dead is terminal
            (AgentPhase::Dead, _) => false,

            // No backwards transitions in normal flow
            _ => false,
        }
    }

    /// Evaluate aging transitions based on the current tick.
    ///
    /// This checks tick-based transitions (Childhood→Adult, Adult→Elder, Elder→Dead).
    /// Does NOT handle death-by-token-depletion (that's R002's job).
    ///
    /// Returns a TransitionResult and mutates the agent's phase if needed.
    pub fn evaluate_aging(
        &self,
        current_tick: u64,
        spawn_tick: u64,
        agent: &mut AgentRecord,
    ) -> TransitionResult {
        // Dead agents never transition
        if agent.phase == AgentPhase::Dead {
            return TransitionResult::NoTransition;
        }

        let old_phase = agent.phase;

        // Check natural aging transitions based on tick thresholds
        let new_phase = match agent.phase {
            AgentPhase::Childhood => {
                if current_tick >= self.config.childhood_end_tick(spawn_tick) {
                    Some(AgentPhase::Adult)
                } else {
                    None
                }
            }
            AgentPhase::Adult => {
                if current_tick >= self.config.adult_end_tick(spawn_tick) {
                    Some(AgentPhase::Elder)
                } else {
                    None
                }
            }
            AgentPhase::Elder => {
                if current_tick >= self.config.elder_end_tick(spawn_tick) {
                    // Natural death from old age
                    return TransitionResult::Died {
                        reason: DeathReason::NaturalDeath,
                        old_phase,
                    };
                }
                None
            }
            // Birth, Dying transitions are handled by other rules
            _ => None,
        };

        if let Some(new) = new_phase {
            if Self::can_transition(old_phase, new) {
                agent.phase = new;
                return TransitionResult::PhaseChanged {
                    old_phase,
                    new_phase: new,
                };
            }
        }

        TransitionResult::NoTransition
    }

    /// Generate WorldEvents from a TransitionResult.
    pub fn events_for_transition(
        &self,
        agent: &AgentRecord,
        result: &TransitionResult,
    ) -> Vec<WorldEvent> {
        match result {
            TransitionResult::NoTransition => vec![],
            TransitionResult::PhaseChanged { old_phase, new_phase } => {
                vec![WorldEvent::PhaseChanged {
                    agent_id: agent.id.to_string(),
                    old_phase: *old_phase,
                    new_phase: *new_phase,
                }]
            }
            TransitionResult::Died { reason, old_phase } => {
                vec![
                    WorldEvent::AgentDying {
                        agent_id: agent.id.to_string(),
                        reason: *reason,
                        grace_ticks: 0,
                    },
                    WorldEvent::PhaseChanged {
                        agent_id: agent.id.to_string(),
                        old_phase: *old_phase,
                        new_phase: AgentPhase::Dead,
                    },
                    WorldEvent::AgentDied {
                        agent_id: agent.id.to_string(),
                        reason: *reason,
                    },
                ]
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Death Cleanup
// ═══════════════════════════════════════════════════════════════════════════

/// Result of cleaning up a dead agent's resources.
#[derive(Debug, Clone)]
pub struct DeathCleanupResult {
    /// The agent that was cleaned up.
    pub agent_id: Uuid,
    /// Tokens that were destroyed (remaining balance).
    pub tokens_destroyed: u64,
    /// Skills that were archived (name, level).
    pub skills_archived: Vec<(String, u32)>,
    /// Events generated during cleanup.
    pub events: Vec<WorldEvent>,
}

/// Perform resource recovery and cleanup for a dead agent.
///
/// This should be called when an agent transitions to Dead.
/// - Remaining tokens are destroyed (removed from circulation).
/// - Skills are archived for potential tombstone creation.
/// - Appropriate events are generated.
pub fn perform_death_cleanup(agent: &mut AgentRecord) -> DeathCleanupResult {
    let tokens_destroyed = agent.tokens;
    let skills_archived: Vec<(String, u32)> = agent
        .skills
        .iter()
        .map(|(name, skill)| (name.clone(), skill.level))
        .collect();

    let mut events = Vec::new();

    // Record token destruction
    if tokens_destroyed > 0 {
        events.push(WorldEvent::BalanceChanged {
            agent_id: agent.id.to_string(),
            currency: crate::world::enums::Currency::Token,
            old_balance: tokens_destroyed,
            new_balance: 0,
        });
        agent.tokens = 0;
    }

    // Clear skills (archived externally via skills_archived)
    agent.skills.clear();

    // Ensure phase is Dead
    agent.phase = AgentPhase::Dead;

    DeathCleanupResult {
        agent_id: agent.id,
        tokens_destroyed,
        skills_archived,
        events,
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Subsystem Trait
// ═══════════════════════════════════════════════════════════════════════════

/// A world subsystem that executes logic each tick.
///
/// Subsystems are dispatched in priority order (lower = earlier) by the
/// scheduler. Errors in one subsystem do not prevent others from running.
pub trait Subsystem: Send + Sync {
    /// Unique subsystem identifier (e.g. "token_burn", "death_judgment").
    fn id(&self) -> &str;

    /// Human-readable subsystem name.
    fn name(&self) -> &str;

    /// Execution priority (lower runs first).
    fn priority(&self) -> u32;

    /// Execute the subsystem for one tick.
    ///
    /// Receives the current tick number and mutable access to all agent records.
    /// Returns a list of events generated during execution.
    fn execute(&self, tick: u64, agents: &mut [(Uuid, u64, AgentRecord)]) -> Vec<WorldEvent>;
}

// ═══════════════════════════════════════════════════════════════════════════
// Subsystem Error Isolation
// ═══════════════════════════════════════════════════════════════════════════

/// Wraps a subsystem execution result, capturing any panic or error.
#[derive(Debug)]
pub struct SubsystemResult {
    /// The subsystem ID that produced this result.
    pub subsystem_id: String,
    /// Events generated during execution (empty if failed).
    pub events: Vec<WorldEvent>,
    /// Whether execution completed successfully.
    pub success: bool,
    /// Error message if execution failed.
    pub error: Option<String>,
}

impl SubsystemResult {
    /// Create a successful result with events.
    pub fn ok(subsystem_id: &str, events: Vec<WorldEvent>) -> Self {
        Self {
            subsystem_id: subsystem_id.to_string(),
            events,
            success: true,
            error: None,
        }
    }

    /// Create a failed result with an error message.
    pub fn err(subsystem_id: &str, error: String) -> Self {
        Self {
            subsystem_id: subsystem_id.to_string(),
            events: Vec::new(),
            success: false,
            error: Some(error),
        }
    }
}

/// Execute a subsystem with error isolation.
///
/// If the subsystem panics or returns an error, it is caught and returned
/// as a failed `SubsystemResult`. Other subsystems continue executing.
pub fn run_subsystem_isolated(
    subsystem: &dyn Subsystem,
    tick: u64,
    agents: &mut [(Uuid, u64, AgentRecord)],
) -> SubsystemResult {
    let id = subsystem.id().to_string();

    // Use std::panic::catch_unwind for panic isolation.
    // Note: This requires the agents slice to be UnwindSafe. Since we're
    // doing mutable borrowing, we use AssertUnwindSafe.
    use std::panic::AssertUnwindSafe;
    match std::panic::catch_unwind(AssertUnwindSafe(|| {
        subsystem.execute(tick, agents)
    })) {
        Ok(events) => SubsystemResult::ok(&id, events),
        Err(payload) => {
            let msg = if let Some(s) = payload.downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = payload.downcast_ref::<String>() {
                s.clone()
            } else {
                "unknown panic".to_string()
            };
            SubsystemResult::err(&id, msg)
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_agent(phase: AgentPhase, tokens: u64) -> AgentRecord {
        AgentRecord {
            id: Uuid::new_v4(),
            name: "test-agent".to_string(),
            phase,
            tokens,
            skills: HashMap::new(),
            personality: String::new(),
            tasks_completed: 0,
            tasks_attempted: 0,
        }
    }

    fn make_agent_with_skills(
        phase: AgentPhase,
        tokens: u64,
        skills: Vec<(&str, u32)>,
    ) -> AgentRecord {
        AgentRecord {
            id: Uuid::new_v4(),
            name: "test-agent".to_string(),
            phase,
            tokens,
            skills: skills
                .into_iter()
                .map(|(name, level)| {
                    (
                        name.to_string(),
                        crate::economy::token_burn::SkillRecord {
                            name: name.to_string(),
                            level,
                            experience: 0.0,
                        },
                    )
                })
                .collect(),
            personality: String::new(),
            tasks_completed: 0,
            tasks_attempted: 0,
        }
    }

    // ── PhaseAbilities tests ──

    #[test]
    fn test_birth_abilities() {
        let abilities = PhaseAbilities::for_phase(AgentPhase::Birth);
        assert_eq!(abilities.skill_efficiency, 0.0);
        assert!(abilities.can_learn);
        assert!(!abilities.can_take_tasks);
        assert!(!abilities.can_trade);
        assert!(abilities.can_communicate);
    }

    #[test]
    fn test_childhood_abilities() {
        let abilities = PhaseAbilities::for_phase(AgentPhase::Childhood);
        assert!((abilities.skill_efficiency - 0.3).abs() < f64::EPSILON);
        assert!(abilities.can_learn);
        assert!(abilities.can_take_tasks);
        assert!(!abilities.can_trade);
        assert!(!abilities.can_teach);
    }

    #[test]
    fn test_adult_abilities() {
        let abilities = PhaseAbilities::for_phase(AgentPhase::Adult);
        assert_eq!(abilities.skill_efficiency, 1.0);
        assert!(abilities.can_learn);
        assert!(abilities.can_take_tasks);
        assert!(abilities.can_trade);
        assert!(abilities.can_teach);
        assert!(abilities.can_write_will);
    }

    #[test]
    fn test_elder_abilities() {
        let abilities = PhaseAbilities::for_phase(AgentPhase::Elder);
        assert!((abilities.skill_efficiency - 0.6).abs() < f64::EPSILON);
        assert!(abilities.can_learn);
        assert!(abilities.can_take_tasks);
        assert!(abilities.can_trade);
        assert!(abilities.can_teach);
        assert!(abilities.can_write_will);
    }

    #[test]
    fn test_dying_abilities() {
        let abilities = PhaseAbilities::for_phase(AgentPhase::Dying);
        assert!((abilities.skill_efficiency - 0.1).abs() < f64::EPSILON);
        assert!(!abilities.can_learn);
        assert!(!abilities.can_take_tasks);
        assert!(!abilities.can_trade);
        assert!(abilities.can_write_will); // Can still write will
        assert!(abilities.can_communicate); // Can still say goodbye
    }

    #[test]
    fn test_dead_abilities() {
        let abilities = PhaseAbilities::for_phase(AgentPhase::Dead);
        assert_eq!(abilities.skill_efficiency, 0.0);
        assert!(!abilities.can_learn);
        assert!(!abilities.can_take_tasks);
        assert!(!abilities.can_trade);
        assert!(!abilities.can_teach);
        assert!(!abilities.can_write_will);
        assert!(!abilities.can_communicate);
    }

    // ── LifecycleConfig tests ──

    #[test]
    fn test_default_config() {
        let config = LifecycleConfig::default();
        assert_eq!(config.childhood_ticks, 100);
        assert_eq!(config.adult_ticks, 1000);
        assert_eq!(config.elder_ticks, 200);
        assert_eq!(config.death_grace_ticks, 10);
    }

    #[test]
    fn test_config_tick_boundaries() {
        let config = LifecycleConfig::default();
        let spawn_tick = 0u64;

        // Birth at 0, Birth→Childhood at tick 1 (via R003)
        // Childhood ends at 0 + 1 + 100 = 101
        assert_eq!(config.childhood_end_tick(spawn_tick), 101);
        // Adult ends at 101 + 1000 = 1101
        assert_eq!(config.adult_end_tick(spawn_tick), 1101);
        // Elder ends at 1101 + 200 = 1301
        assert_eq!(config.elder_end_tick(spawn_tick), 1301);
    }

    #[test]
    fn test_config_tick_boundaries_with_spawn_offset() {
        let config = LifecycleConfig::default();
        let spawn_tick = 50u64;

        // Childhood ends at 50 + 1 + 100 = 151
        assert_eq!(config.childhood_end_tick(spawn_tick), 151);
        // Adult ends at 151 + 1000 = 1151
        assert_eq!(config.adult_end_tick(spawn_tick), 1151);
        // Elder ends at 1151 + 200 = 1351
        assert_eq!(config.elder_end_tick(spawn_tick), 1351);
    }

    #[test]
    fn test_config_from_yaml() {
        let yaml_str = r#"
lifecycle:
  childhood_ticks: 200
  adult_ticks: 2000
  elder_ticks: 400
  death_grace_ticks: 20
"#;
        let value: serde_yaml::Value = serde_yaml::from_str(yaml_str).unwrap();
        let config = LifecycleConfig::from_yaml_value(&value);
        assert_eq!(config.childhood_ticks, 200);
        assert_eq!(config.adult_ticks, 2000);
        assert_eq!(config.elder_ticks, 400);
        assert_eq!(config.death_grace_ticks, 20);
    }

    #[test]
    fn test_config_from_yaml_defaults_when_missing() {
        let yaml_str = r#"
some_other_section:
  foo: bar
"#;
        let value: serde_yaml::Value = serde_yaml::from_str(yaml_str).unwrap();
        let config = LifecycleConfig::from_yaml_value(&value);
        assert_eq!(config.childhood_ticks, 100);
        assert_eq!(config.adult_ticks, 1000);
        assert_eq!(config.elder_ticks, 200);
        assert_eq!(config.death_grace_ticks, 10);
    }

    // ── can_transition tests ──

    #[test]
    fn test_valid_transitions() {
        assert!(LifecycleMachine::can_transition(AgentPhase::Birth, AgentPhase::Childhood));
        assert!(LifecycleMachine::can_transition(AgentPhase::Childhood, AgentPhase::Adult));
        assert!(LifecycleMachine::can_transition(AgentPhase::Adult, AgentPhase::Elder));
        assert!(LifecycleMachine::can_transition(AgentPhase::Elder, AgentPhase::Dead));
        assert!(LifecycleMachine::can_transition(AgentPhase::Adult, AgentPhase::Dying));
        assert!(LifecycleMachine::can_transition(AgentPhase::Dying, AgentPhase::Dead));
        assert!(LifecycleMachine::can_transition(AgentPhase::Dying, AgentPhase::Adult));
        assert!(LifecycleMachine::can_transition(AgentPhase::Childhood, AgentPhase::Dead));
        assert!(LifecycleMachine::can_transition(AgentPhase::Adult, AgentPhase::Dead));
    }

    #[test]
    fn test_invalid_transitions() {
        // Dead is terminal
        assert!(!LifecycleMachine::can_transition(AgentPhase::Dead, AgentPhase::Birth));
        assert!(!LifecycleMachine::can_transition(AgentPhase::Dead, AgentPhase::Childhood));
        assert!(!LifecycleMachine::can_transition(AgentPhase::Dead, AgentPhase::Adult));
        assert!(!LifecycleMachine::can_transition(AgentPhase::Dead, AgentPhase::Elder));
        assert!(!LifecycleMachine::can_transition(AgentPhase::Dead, AgentPhase::Dying));

        // No backwards transitions
        assert!(!LifecycleMachine::can_transition(AgentPhase::Adult, AgentPhase::Childhood));
        assert!(!LifecycleMachine::can_transition(AgentPhase::Elder, AgentPhase::Adult));
        assert!(!LifecycleMachine::can_transition(AgentPhase::Childhood, AgentPhase::Birth));

        // No skipping phases forward
        assert!(!LifecycleMachine::can_transition(AgentPhase::Birth, AgentPhase::Adult));
        assert!(!LifecycleMachine::can_transition(AgentPhase::Childhood, AgentPhase::Elder));
    }

    // ── evaluate_aging tests ──

    #[test]
    fn test_no_transition_for_dead() {
        let machine = LifecycleMachine::with_defaults();
        let mut agent = make_agent(AgentPhase::Dead, 0);
        let result = machine.evaluate_aging(9999, 0, &mut agent);
        assert_eq!(result, TransitionResult::NoTransition);
    }

    #[test]
    fn test_childhood_to_adult_at_threshold() {
        let machine = LifecycleMachine::with_defaults();
        let mut agent = make_agent(AgentPhase::Childhood, 500);

        // childhood_end_tick(0) = 0 + 1 + 100 = 101
        // At tick 100, still childhood
        let result = machine.evaluate_aging(100, 0, &mut agent);
        assert_eq!(result, TransitionResult::NoTransition);
        assert_eq!(agent.phase, AgentPhase::Childhood);

        // At tick 101, should transition to Adult
        let result = machine.evaluate_aging(101, 0, &mut agent);
        assert_eq!(
            result,
            TransitionResult::PhaseChanged {
                old_phase: AgentPhase::Childhood,
                new_phase: AgentPhase::Adult,
            }
        );
        assert_eq!(agent.phase, AgentPhase::Adult);
    }

    #[test]
    fn test_adult_to_elder_at_threshold() {
        let machine = LifecycleMachine::with_defaults();
        let mut agent = make_agent(AgentPhase::Adult, 500);

        // adult_end_tick(0) = 101 + 1000 = 1101
        // At tick 1100, still adult
        let result = machine.evaluate_aging(1100, 0, &mut agent);
        assert_eq!(result, TransitionResult::NoTransition);
        assert_eq!(agent.phase, AgentPhase::Adult);

        // At tick 1101, should transition to Elder
        let result = machine.evaluate_aging(1101, 0, &mut agent);
        assert_eq!(
            result,
            TransitionResult::PhaseChanged {
                old_phase: AgentPhase::Adult,
                new_phase: AgentPhase::Elder,
            }
        );
        assert_eq!(agent.phase, AgentPhase::Elder);
    }

    #[test]
    fn test_elder_dies_at_threshold() {
        let machine = LifecycleMachine::with_defaults();
        let mut agent = make_agent(AgentPhase::Elder, 500);

        // elder_end_tick(0) = 1101 + 200 = 1301
        // At tick 1300, still elder
        let result = machine.evaluate_aging(1300, 0, &mut agent);
        assert_eq!(result, TransitionResult::NoTransition);
        assert_eq!(agent.phase, AgentPhase::Elder);

        // At tick 1301, should die from natural causes
        let result = machine.evaluate_aging(1301, 0, &mut agent);
        assert_eq!(
            result,
            TransitionResult::Died {
                reason: DeathReason::NaturalDeath,
                old_phase: AgentPhase::Elder,
            }
        );
    }

    #[test]
    fn test_no_aging_for_birth_phase() {
        let machine = LifecycleMachine::with_defaults();
        let mut agent = make_agent(AgentPhase::Birth, 500);
        let result = machine.evaluate_aging(9999, 0, &mut agent);
        assert_eq!(result, TransitionResult::NoTransition);
        assert_eq!(agent.phase, AgentPhase::Birth);
    }

    #[test]
    fn test_no_aging_for_dying_phase() {
        let machine = LifecycleMachine::with_defaults();
        let mut agent = make_agent(AgentPhase::Dying, 0);
        let result = machine.evaluate_aging(9999, 0, &mut agent);
        assert_eq!(result, TransitionResult::NoTransition);
        assert_eq!(agent.phase, AgentPhase::Dying);
    }

    #[test]
    fn test_full_lifecycle_aging() {
        let machine = LifecycleMachine::with_defaults();
        let mut agent = make_agent(AgentPhase::Childhood, 10000);
        let spawn_tick = 0u64;

        // Childhood: ticks 1-100
        assert_eq!(machine.evaluate_aging(50, spawn_tick, &mut agent), TransitionResult::NoTransition);
        assert_eq!(agent.phase, AgentPhase::Childhood);

        // Childhood -> Adult at tick 101
        assert!(matches!(
            machine.evaluate_aging(101, spawn_tick, &mut agent),
            TransitionResult::PhaseChanged { .. }
        ));
        assert_eq!(agent.phase, AgentPhase::Adult);

        // Adult: ticks 101-1100
        assert_eq!(machine.evaluate_aging(500, spawn_tick, &mut agent), TransitionResult::NoTransition);
        assert_eq!(agent.phase, AgentPhase::Adult);

        // Adult -> Elder at tick 1101
        assert!(matches!(
            machine.evaluate_aging(1101, spawn_tick, &mut agent),
            TransitionResult::PhaseChanged { .. }
        ));
        assert_eq!(agent.phase, AgentPhase::Elder);

        // Elder: ticks 1101-1300
        assert_eq!(machine.evaluate_aging(1200, spawn_tick, &mut agent), TransitionResult::NoTransition);
        assert_eq!(agent.phase, AgentPhase::Elder);

        // Elder -> Dead at tick 1301
        assert!(matches!(
            machine.evaluate_aging(1301, spawn_tick, &mut agent),
            TransitionResult::Died { .. }
        ));
    }

    // ── events_for_transition tests ──

    #[test]
    fn test_events_for_no_transition() {
        let machine = LifecycleMachine::with_defaults();
        let agent = make_agent(AgentPhase::Adult, 100);
        let events = machine.events_for_transition(&agent, &TransitionResult::NoTransition);
        assert!(events.is_empty());
    }

    #[test]
    fn test_events_for_phase_changed() {
        let machine = LifecycleMachine::with_defaults();
        let agent = make_agent(AgentPhase::Adult, 100);
        let result = TransitionResult::PhaseChanged {
            old_phase: AgentPhase::Childhood,
            new_phase: AgentPhase::Adult,
        };
        let events = machine.events_for_transition(&agent, &result);
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0], WorldEvent::PhaseChanged {
            old_phase: AgentPhase::Childhood,
            new_phase: AgentPhase::Adult,
            ..
        }));
    }

    #[test]
    fn test_events_for_died() {
        let machine = LifecycleMachine::with_defaults();
        let agent = make_agent(AgentPhase::Elder, 100);
        let result = TransitionResult::Died {
            reason: DeathReason::TokenDepleted,
            old_phase: AgentPhase::Elder,
        };
        let events = machine.events_for_transition(&agent, &result);
        assert_eq!(events.len(), 3); // AgentDying + PhaseChanged + AgentDied
        assert!(matches!(&events[0], WorldEvent::AgentDying { .. }));
        assert!(matches!(&events[1], WorldEvent::PhaseChanged { .. }));
        assert!(matches!(&events[2], WorldEvent::AgentDied { .. }));
    }

    // ── Death cleanup tests ──

    #[test]
    fn test_death_cleanup_destroys_tokens() {
        let mut agent = make_agent(AgentPhase::Dead, 500);
        let result = perform_death_cleanup(&mut agent);

        assert_eq!(result.tokens_destroyed, 500);
        assert_eq!(agent.tokens, 0);
        assert_eq!(agent.phase, AgentPhase::Dead);
    }

    #[test]
    fn test_death_cleanup_archives_skills() {
        let mut agent = make_agent_with_skills(AgentPhase::Dead, 100, vec![
            ("mining", 5),
            ("trading", 3),
        ]);
        let result = perform_death_cleanup(&mut agent);

        assert_eq!(result.skills_archived.len(), 2);
        assert!(result.skills_archived.contains(&("mining".to_string(), 5)));
        assert!(result.skills_archived.contains(&("trading".to_string(), 3)));
        assert!(agent.skills.is_empty());
    }

    #[test]
    fn test_death_cleanup_with_zero_tokens() {
        let mut agent = make_agent(AgentPhase::Adult, 0);
        let result = perform_death_cleanup(&mut agent);

        assert_eq!(result.tokens_destroyed, 0);
        assert_eq!(result.events.len(), 0); // No BalanceChanged event for 0 tokens
        assert_eq!(agent.phase, AgentPhase::Dead);
    }

    #[test]
    fn test_death_cleanup_emits_balance_event() {
        let mut agent = make_agent(AgentPhase::Adult, 250);
        let result = perform_death_cleanup(&mut agent);

        assert_eq!(result.events.len(), 1);
        match &result.events[0] {
            WorldEvent::BalanceChanged {
                old_balance,
                new_balance,
                ..
            } => {
                assert_eq!(*old_balance, 250);
                assert_eq!(*new_balance, 0);
            }
            _ => panic!("Expected BalanceChanged event"),
        }
    }

    // ── Custom config test ──

    #[test]
    fn test_custom_config_transitions() {
        let config = LifecycleConfig {
            childhood_ticks: 50,
            adult_ticks: 500,
            elder_ticks: 100,
            death_grace_ticks: 5,
        };
        let machine = LifecycleMachine::new(config);
        let mut agent = make_agent(AgentPhase::Childhood, 500);

        // childhood_end_tick(0) = 0 + 1 + 50 = 51
        assert_eq!(machine.evaluate_aging(50, 0, &mut agent), TransitionResult::NoTransition);
        let result = machine.evaluate_aging(51, 0, &mut agent);
        assert!(matches!(result, TransitionResult::PhaseChanged { .. }));
        assert_eq!(agent.phase, AgentPhase::Adult);

        // adult_end_tick(0) = 51 + 500 = 551
        assert_eq!(machine.evaluate_aging(550, 0, &mut agent), TransitionResult::NoTransition);
        let result = machine.evaluate_aging(551, 0, &mut agent);
        assert!(matches!(result, TransitionResult::PhaseChanged { .. }));
        assert_eq!(agent.phase, AgentPhase::Elder);

        // elder_end_tick(0) = 551 + 100 = 651
        assert_eq!(machine.evaluate_aging(650, 0, &mut agent), TransitionResult::NoTransition);
        let result = machine.evaluate_aging(651, 0, &mut agent);
        assert!(matches!(result, TransitionResult::Died { .. }));
    }

    // ── Subsystem tests ──

    struct TestSubsystem;

    impl Subsystem for TestSubsystem {
        fn id(&self) -> &str { "test" }
        fn name(&self) -> &str { "Test Subsystem" }
        fn priority(&self) -> u32 { 100 }

        fn execute(&self, _tick: u64, agents: &mut [(Uuid, u64, AgentRecord)]) -> Vec<WorldEvent> {
            // Burn 1 token from each agent
            for (_, _, agent) in agents.iter_mut() {
                if agent.tokens > 0 {
                    agent.tokens -= 1;
                }
            }
            Vec::new()
        }
    }

    struct PanickingSubsystem;

    impl Subsystem for PanickingSubsystem {
        fn id(&self) -> &str { "panic" }
        fn name(&self) -> &str { "Panicking Subsystem" }
        fn priority(&self) -> u32 { 200 }

        fn execute(&self, _tick: u64, _agents: &mut [(Uuid, u64, AgentRecord)]) -> Vec<WorldEvent> {
            panic!("intentional panic for testing");
        }
    }

    #[test]
    fn test_subsystem_execute() {
        let sub = TestSubsystem;
        let id = Uuid::new_v4();
        let mut agents: Vec<(Uuid, u64, AgentRecord)> = vec![
            (id, 0, make_agent(AgentPhase::Adult, 100)),
        ];

        let events = sub.execute(1, &mut agents);
        assert!(events.is_empty());
        assert_eq!(agents[0].2.tokens, 99);
    }

    #[test]
    fn test_run_subsystem_isolated_success() {
        let sub = TestSubsystem;
        let id = Uuid::new_v4();
        let mut agents: Vec<(Uuid, u64, AgentRecord)> = vec![
            (id, 0, make_agent(AgentPhase::Adult, 100)),
        ];

        let result = run_subsystem_isolated(&sub, 1, &mut agents);
        assert!(result.success);
        assert!(result.error.is_none());
        assert_eq!(agents[0].2.tokens, 99);
    }

    #[test]
    fn test_run_subsystem_isolated_panic() {
        let sub = PanickingSubsystem;
        let id = Uuid::new_v4();
        let mut agents: Vec<(Uuid, u64, AgentRecord)> = vec![
            (id, 0, make_agent(AgentPhase::Adult, 100)),
        ];

        let result = run_subsystem_isolated(&sub, 1, &mut agents);
        assert!(!result.success);
        assert!(result.error.is_some());
        assert!(result.error.as_ref().unwrap().contains("intentional panic"));
    }
}
