//! Concrete [`Subsystem`] implementations for the tick pipeline.
//!
//! Each subsystem corresponds to one phase of the tick loop:
//! 1. **TokenBurnSubsystem** — deduct tokens per tick (R001)
//! 2. **DeathJudgmentSubsystem** — check for token depletion, transition to Dead (R002)
//! 3. **RuleCheckSubsystem** — run the full rule registry (R001–R003 combined)
//! 4. **EventBroadcastSubsystem** — emit `TickAdvanced` and broadcast collected events

use std::sync::Arc;

use uuid::Uuid;

use crate::economy::escrow::EscrowManager;
use crate::economy::mentorship::MentorshipConfig;
use crate::economy::mentorship::MentorshipSystem;
use crate::economy::reputation::{ReputationConfig, ReputationSystem};
use crate::economy::token_burn::{AgentRecord, TokenBurnEngine};
use crate::economy::trust::{TrustConfig, TrustNetwork};
use crate::lifecycle::{LifecycleConfig, LifecycleMachine};
use crate::rules::RuleRegistry;
use crate::world::enums::{AgentPhase, DeathReason};
use crate::world::event::WorldEvent;
use crate::world::state::EventBus;
use crate::world::subsystem::Subsystem;

// ═══════════════════════════════════════════════════════════════════════════
// Token Burn Subsystem (R001)
// ═══════════════════════════════════════════════════════════════════════════

/// Burns tokens for every living agent each tick.
pub struct TokenBurnSubsystem {
    engine: TokenBurnEngine,
}

impl TokenBurnSubsystem {
    pub fn new(engine: TokenBurnEngine) -> Self {
        Self { engine }
    }
}

impl Subsystem for TokenBurnSubsystem {
    fn name(&self) -> &str {
        "token_burn"
    }

    fn on_tick(&self, tick: u64, agents: &mut [(Uuid, u64, AgentRecord)]) -> Vec<WorldEvent> {
        let mut events = Vec::new();

        for (_id, _spawn_tick, agent) in agents.iter_mut() {
            if agent.phase == AgentPhase::Dead || agent.phase == AgentPhase::Birth {
                continue;
            }

            let burn_amount = self.engine.calculate_tick_burn(agent);
            if burn_amount == 0 {
                continue;
            }

            let tokens_before = agent.tokens;
            let actual_burn = burn_amount.min(agent.tokens);
            agent.tokens -= actual_burn;

            if actual_burn > 0 {
                events.push(WorldEvent::BalanceChanged {
                    agent_id: agent.id.to_string(),
                    currency: crate::world::enums::Currency::Token,
                    old_balance: tokens_before,
                    new_balance: agent.tokens,
                });
            }
        }

        let _ = tick; // used for logging in future
        events
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Death Judgment Subsystem (R002)
// ═══════════════════════════════════════════════════════════════════════════

/// Checks for agents that have run out of tokens and transitions them to Dead.
pub struct DeathJudgmentSubsystem {
    /// Grace ticks after tokens hit zero before actual death.
    pub grace_ticks: u64,
}

impl DeathJudgmentSubsystem {
    pub fn new(grace_ticks: u64) -> Self {
        Self { grace_ticks }
    }
}

impl Subsystem for DeathJudgmentSubsystem {
    fn name(&self) -> &str {
        "death_judgment"
    }

    fn on_tick(&self, _tick: u64, agents: &mut [(Uuid, u64, AgentRecord)]) -> Vec<WorldEvent> {
        let mut events = Vec::new();

        for (_id, _spawn_tick, agent) in agents.iter_mut() {
            if agent.phase == AgentPhase::Dead || agent.phase == AgentPhase::Birth {
                continue;
            }

            if agent.tokens > 0 {
                continue;
            }

            // Agent is dying
            events.push(WorldEvent::AgentDying {
                agent_id: agent.id.to_string(),
                reason: DeathReason::TokenDepleted,
                grace_ticks: self.grace_ticks,
            });

            // If no grace period, immediately kill
            if self.grace_ticks == 0 {
                agent.phase = AgentPhase::Dead;
                events.push(WorldEvent::AgentDied {
                    agent_id: agent.id.to_string(),
                    reason: DeathReason::TokenDepleted,
                });
            }
        }

        events
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Rule Check Subsystem (R001–R003 combined via RuleRegistry)
// ═══════════════════════════════════════════════════════════════════════════

/// Runs the full [`RuleRegistry`] against every agent.
///
/// This is the "full pipeline" subsystem — it can replace the individual
/// `TokenBurnSubsystem` + `DeathJudgmentSubsystem` when you want the rule
/// engine to drive everything.
pub struct RuleCheckSubsystem {
    registry: RuleRegistry,
}

impl RuleCheckSubsystem {
    pub fn new(registry: RuleRegistry) -> Self {
        Self { registry }
    }
}

impl Subsystem for RuleCheckSubsystem {
    fn name(&self) -> &str {
        "rule_check"
    }

    fn on_tick(&self, tick: u64, agents: &mut [(Uuid, u64, AgentRecord)]) -> Vec<WorldEvent> {
        let all_results = self.registry.evaluate_all(tick, agents);
        let mut events = Vec::new();
        for (_agent_id, results) in all_results {
            for result in results {
                events.extend(result.events);
            }
        }
        events
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Event Broadcast Subsystem
// ═══════════════════════════════════════════════════════════════════════════

/// Produces a `TickAdvanced` event at the end of each tick.
///
/// Note: actual broadcasting to the [`EventBus`] is handled by
/// [`WorldState::tick`](crate::world::state::WorldState), which emits all
/// collected events after running subsystems. This subsystem only *generates*
/// the event.
pub struct EventBroadcastSubsystem {
    _event_bus: Arc<EventBus>,
}

impl EventBroadcastSubsystem {
    pub fn new(event_bus: Arc<EventBus>) -> Self {
        Self {
            _event_bus: event_bus,
        }
    }
}

impl Subsystem for EventBroadcastSubsystem {
    fn name(&self) -> &str {
        "event_broadcast"
    }

    fn on_tick(&self, tick: u64, _agents: &mut [(Uuid, u64, AgentRecord)]) -> Vec<WorldEvent> {
        vec![WorldEvent::TickAdvanced { tick }]
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Lifecycle Aging Subsystem
// ═══════════════════════════════════════════════════════════════════════════

/// Evaluates tick-based aging transitions for all agents each tick.
///
/// Uses [`LifecycleMachine`] to check Childhood→Adult, Adult→Elder,
/// Elder→Dead (natural death) transitions based on spawn tick thresholds.
pub struct LifecycleAgingSubsystem {
    machine: LifecycleMachine,
}

impl LifecycleAgingSubsystem {
    pub fn new(config: LifecycleConfig) -> Self {
        Self {
            machine: LifecycleMachine::new(config),
        }
    }
}

impl Subsystem for LifecycleAgingSubsystem {
    fn name(&self) -> &str {
        "lifecycle_aging"
    }

    fn on_tick(&self, tick: u64, agents: &mut [(Uuid, u64, AgentRecord)]) -> Vec<WorldEvent> {
        let mut events = Vec::new();

        for (_id, spawn_tick, agent) in agents.iter_mut() {
            if agent.phase == AgentPhase::Dead || agent.phase == AgentPhase::Birth {
                continue;
            }

            let result = self.machine.evaluate_aging(tick, *spawn_tick, agent);
            let transition_events = self.machine.events_for_transition(agent, &result);

            // If agent died from old age, perform cleanup
            if let crate::lifecycle::TransitionResult::Died { .. } = &result {
                let cleanup = crate::lifecycle::perform_death_cleanup(agent);
                events.extend(cleanup.events);
            }

            events.extend(transition_events);
        }

        events
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Reputation Decay Subsystem
// ═══════════════════════════════════════════════════════════════════════════

/// Runs reputation time-decay each tick so that negative reputations
/// gradually recover toward zero.
///
/// This subsystem wraps a [`ReputationSystem`] and must be kept alive
/// for the lifetime of the engine. Access the underlying system via
/// the `system()` accessor for task completion/breach events.
pub struct ReputationDecaySubsystem {
    system: std::sync::Mutex<ReputationSystem>,
}

impl ReputationDecaySubsystem {
    pub fn new(config: ReputationConfig) -> Self {
        Self {
            system: std::sync::Mutex::new(ReputationSystem::new(config)),
        }
    }

    pub fn new_with_event_bus(config: ReputationConfig, event_bus: EventBus) -> Self {
        Self {
            system: std::sync::Mutex::new(ReputationSystem::with_event_bus(config, event_bus)),
        }
    }

    /// Access the underlying ReputationSystem (locked).
    pub fn system(&self) -> &std::sync::Mutex<ReputationSystem> {
        &self.system
    }
}

impl Subsystem for ReputationDecaySubsystem {
    fn name(&self) -> &str {
        "reputation_decay"
    }

    fn on_tick(&self, tick: u64, _agents: &mut [(Uuid, u64, AgentRecord)]) -> Vec<WorldEvent> {
        let mut system = self.system.lock().unwrap_or_else(|e| { tracing::error!("subsystems lock poisoned: {}", e); e.into_inner() });
        let _changes = system.process_time_decay(tick);
        // Reputation changes are emitted internally by ReputationSystem
        Vec::new()
    }
}

// ========================================================================
// Trust Decay Subsystem
// ========================================================================

/// Decays all trust edges toward zero each tick.
pub struct TrustDecaySubsystem {
    network: std::sync::Mutex<TrustNetwork>,
}

impl TrustDecaySubsystem {
    pub fn new(config: TrustConfig) -> Self {
        Self {
            network: std::sync::Mutex::new(TrustNetwork::new(config)),
        }
    }

    pub fn new_with_event_bus(config: TrustConfig, event_bus: EventBus) -> Self {
        Self {
            network: std::sync::Mutex::new(TrustNetwork::with_event_bus(config, event_bus)),
        }
    }

    pub fn network(&self) -> &std::sync::Mutex<TrustNetwork> {
        &self.network
    }
}

impl Subsystem for TrustDecaySubsystem {
    fn name(&self) -> &str {
        "trust_decay"
    }

    fn on_tick(&self, tick: u64, _agents: &mut [(Uuid, u64, AgentRecord)]) -> Vec<WorldEvent> {
        let mut net = self.network.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let _changes = net.decay_trust(tick);
        Vec::new()
    }
}

// ========================================================================
// Mentorship Progress Subsystem
// ========================================================================

/// Advances active mentorship sessions each tick.
pub struct MentorshipProgressSubsystem {
    system: std::sync::Mutex<MentorshipSystem>,
}

impl MentorshipProgressSubsystem {
    pub fn new(config: MentorshipConfig) -> Self {
        Self {
            system: std::sync::Mutex::new(MentorshipSystem::new(config)),
        }
    }

    pub fn new_with_event_bus(config: MentorshipConfig, event_bus: EventBus) -> Self {
        Self {
            system: std::sync::Mutex::new(MentorshipSystem::with_event_bus(config, event_bus)),
        }
    }

    pub fn system(&self) -> &std::sync::Mutex<MentorshipSystem> {
        &self.system
    }
}

impl Subsystem for MentorshipProgressSubsystem {
    fn name(&self) -> &str {
        "mentorship_progress"
    }

    fn on_tick(&self, tick: u64, agents: &mut [(Uuid, u64, AgentRecord)]) -> Vec<WorldEvent> {
        let mut sys = self.system.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let _completed = sys.progress_tick(tick, agents).unwrap_or_default();
        Vec::new()
    }
}

// ========================================================================
// Escrow Expiry Subsystem
// ========================================================================

/// Processes escrow expiry each tick.
pub struct EscrowExpirySubsystem {
    manager: std::sync::Mutex<EscrowManager>,
}

impl Default for EscrowExpirySubsystem {
    fn default() -> Self {
        Self::new()
    }
}

impl EscrowExpirySubsystem {
    pub fn new() -> Self {
        Self {
            manager: std::sync::Mutex::new(EscrowManager::new()),
        }
    }

    pub fn new_with_event_bus(event_bus: EventBus) -> Self {
        Self {
            manager: std::sync::Mutex::new(EscrowManager::with_event_bus(event_bus)),
        }
    }

    pub fn manager(&self) -> &std::sync::Mutex<EscrowManager> {
        &self.manager
    }
}

impl Subsystem for EscrowExpirySubsystem {
    fn name(&self) -> &str {
        "escrow_expiry"
    }

    fn on_tick(&self, tick: u64, _agents: &mut [(Uuid, u64, AgentRecord)]) -> Vec<WorldEvent> {
        let mut mgr = self.manager.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let _expired = mgr.process_expiry(tick);
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_agent(phase: AgentPhase, tokens: u64) -> (Uuid, u64, AgentRecord) {
        (
            Uuid::new_v4(),
            0,
            AgentRecord {
                id: Uuid::new_v4(),
                name: "test".to_string(),
                phase,
                tokens,
                skills: HashMap::new(),
                personality: String::new(),
                tasks_completed: 0,
                tasks_attempted: 0,
            },
        )
    }

    #[test]
    fn token_burn_subsystem_deducts_tokens() {
        let sub = TokenBurnSubsystem::new(TokenBurnEngine::with_defaults());
        let mut agents = vec![make_agent(AgentPhase::Adult, 100)];

        let events = sub.on_tick(1, &mut agents);

        assert_eq!(agents[0].2.tokens, 90);
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0], WorldEvent::BalanceChanged { .. }));
    }

    #[test]
    fn token_burn_skips_dead_and_birth() {
        let sub = TokenBurnSubsystem::new(TokenBurnEngine::with_defaults());
        let mut agents = vec![
            make_agent(AgentPhase::Dead, 100),
            make_agent(AgentPhase::Birth, 100),
            make_agent(AgentPhase::Adult, 100),
        ];

        let events = sub.on_tick(1, &mut agents);

        assert_eq!(agents[0].2.tokens, 100); // dead unchanged
        assert_eq!(agents[1].2.tokens, 100); // birth unchanged
        assert_eq!(agents[2].2.tokens, 90); // adult burned
        assert_eq!(events.len(), 1); // only adult event
    }

    #[test]
    fn death_judgment_kills_at_zero() {
        let sub = DeathJudgmentSubsystem::new(0);
        let mut agents = vec![make_agent(AgentPhase::Adult, 0)];

        let events = sub.on_tick(1, &mut agents);

        assert_eq!(agents[0].2.phase, AgentPhase::Dead);
        assert_eq!(events.len(), 2); // AgentDying + AgentDied
    }

    #[test]
    fn death_judgment_grace_period() {
        let sub = DeathJudgmentSubsystem::new(10);
        let mut agents = vec![make_agent(AgentPhase::Adult, 0)];

        let events = sub.on_tick(1, &mut agents);

        assert_ne!(agents[0].2.phase, AgentPhase::Dead);
        assert_eq!(events.len(), 1); // only AgentDying
    }

    #[test]
    fn death_judgment_skips_healthy_agents() {
        let sub = DeathJudgmentSubsystem::new(0);
        let mut agents = vec![make_agent(AgentPhase::Adult, 50)];

        let events = sub.on_tick(1, &mut agents);

        assert_eq!(agents[0].2.phase, AgentPhase::Adult);
        assert!(events.is_empty());
    }

    #[test]
    fn rule_check_subsystem_runs_rules() {
        use crate::rules::default_registry;

        let registry = default_registry();
        let sub = RuleCheckSubsystem::new(registry);
        let mut agents = vec![make_agent(AgentPhase::Adult, 100)];

        let events = sub.on_tick(1, &mut agents);

        // R003 (no phase change for adult) + R001 (burn) + R002 (no death)
        assert!(agents[0].2.tokens < 100, "tokens should have been burned");
        assert!(!events.is_empty());
    }

    #[test]
    fn event_broadcast_produces_tick_advanced() {
        let bus = Arc::new(EventBus::new(64));

        let sub = EventBroadcastSubsystem::new(bus.clone());
        let mut agents: Vec<(Uuid, u64, AgentRecord)> = vec![];

        let events = sub.on_tick(42, &mut agents);

        assert_eq!(events.len(), 1);
        assert_eq!(events[0], WorldEvent::TickAdvanced { tick: 42 });
    }
}
