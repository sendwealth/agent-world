use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use dashmap::DashMap;
use dashmap::mapref::entry::Entry;
use uuid::Uuid;

use crate::economy::token_burn::AgentRecord;
use crate::economy::reward::{Ledger, LedgerEntry};
use crate::economy::task::{Task, TaskStatus};
use crate::world::enums::{AgentPhase, DeathReason};
use crate::world::event::WorldEvent;
use crate::world::state::EventBus;

use super::culture::CultureStore;

// ── Errors ───────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StateError {
    AgentNotFound(String),
    AgentAlreadyExists(String),
    AgentAlreadyDead(String),
    InvalidPhaseTransition { agent_id: String, from: AgentPhase, to: AgentPhase },
    TaskNotFound(String),
    InvalidTaskTransition { task_id: String, from: TaskStatus, to: TaskStatus },
    InsufficientTokens { agent_id: String, required: u64, available: u64 },
}

impl std::fmt::Display for StateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StateError::AgentNotFound(id) => write!(f, "agent not found: {}", id),
            StateError::AgentAlreadyExists(id) => write!(f, "agent already exists: {}", id),
            StateError::AgentAlreadyDead(id) => write!(f, "agent is already dead: {}", id),
            StateError::InvalidPhaseTransition { agent_id, from, to } => {
                write!(f, "invalid phase transition for {}: {:?} -> {:?}", agent_id, from, to)
            }
            StateError::TaskNotFound(id) => write!(f, "task not found: {}", id),
            StateError::InvalidTaskTransition { task_id, from, to } => {
                write!(f, "invalid task status transition for {}: {:?} -> {:?}", task_id, from, to)
            }
            StateError::InsufficientTokens { agent_id, required, available } => {
                write!(f, "insufficient tokens for {}: required {}, available {}", agent_id, required, available)
            }
        }
    }
}

impl std::error::Error for StateError {}

// ── Phase Transition Validation ──────────────────────────

/// Valid agent phase transitions.
///
/// Birth -> Childhood -> Adult -> Elder -> Dying -> Dead
/// Any living phase can transition directly to Dead (killed).
fn is_valid_phase_transition(from: AgentPhase, to: AgentPhase) -> bool {
    if from == to {
        return true; // no-op transitions are allowed
    }
    match from {
        AgentPhase::Birth => matches!(to, AgentPhase::Childhood | AgentPhase::Dead),
        AgentPhase::Childhood => matches!(to, AgentPhase::Adult | AgentPhase::Dead),
        AgentPhase::Adult => matches!(to, AgentPhase::Elder | AgentPhase::Dead),
        AgentPhase::Elder => matches!(to, AgentPhase::Dying | AgentPhase::Dead),
        AgentPhase::Dying => matches!(to, AgentPhase::Dead),
        AgentPhase::Dead => false, // dead agents cannot transition
    }
}

// ── WorldState ───────────────────────────────────────────

/// Thread-safe global world state.
///
/// Uses `DashMap` for concurrent agent/task storage and `AtomicU64` for the
/// tick counter. Token balances are stored directly in `AgentRecord` within
/// the agents map — a single source of truth avoids cross-map desync.
pub struct WorldState {
    /// Global tick counter, incremented atomically.
    tick: AtomicU64,
    /// Registered agents, keyed by string ID.
    /// Token balances are stored in `AgentRecord.tokens`.
    agents: DashMap<String, AgentRecord>,
    /// Active tasks, keyed by UUID string.
    tasks: DashMap<String, Task>,
    /// Shared ledger for recording financial transactions.
    ledger: Arc<std::sync::RwLock<Ledger>>,
    /// Optional event bus for broadcasting state changes.
    event_bus: Option<EventBus>,
    /// Culture store for organization cultures, clusters, and trust.
    culture_store: CultureStore,
}

impl WorldState {
    /// Create a new empty WorldState.
    pub fn new() -> Self {
        Self {
            tick: AtomicU64::new(0),
            agents: DashMap::new(),
            tasks: DashMap::new(),
            ledger: Arc::new(std::sync::RwLock::new(Ledger::new())),
            event_bus: None,
            culture_store: CultureStore::new(),
        }
    }

    /// Create a WorldState with an event bus for broadcasting state changes.
    pub fn with_event_bus(event_bus: EventBus) -> Self {
        let mut state = Self::new();
        state.event_bus = Some(event_bus);
        state
    }

    // ── Tick Counter ──────────────────────────────────────

    /// Get the current tick value.
    pub fn tick(&self) -> u64 {
        self.tick.load(Ordering::Acquire)
    }

    /// Advance the tick counter by one and return the new tick value.
    pub fn advance_tick(&self) -> u64 {
        let new_tick = self.tick.fetch_add(1, Ordering::AcqRel) + 1;
        self.emit(WorldEvent::TickAdvanced { tick: new_tick });
        new_tick
    }

    /// Set the tick counter to a specific value (useful for recovery).
    pub fn set_tick(&self, tick: u64) {
        self.tick.store(tick, Ordering::Release);
    }

    // ── Agent CRUD ────────────────────────────────────────

    /// Register a new agent with the given ID, name, and initial token balance.
    ///
    /// The agent starts in the `Birth` phase.
    /// Returns an error if an agent with the same ID already exists.
    /// Uses DashMap `entry()` for atomic check-and-insert (no TOCTOU race).
    pub fn register_agent(
        &self,
        id: String,
        name: String,
        initial_tokens: u64,
    ) -> Result<(), StateError> {
        self.register_agent_with_phase(id, name, initial_tokens, AgentPhase::Birth)
    }

    /// Register a new agent with a specific phase (useful for testing/recovery).
    ///
    /// Uses DashMap `entry()` for atomic check-and-insert.
    pub fn register_agent_with_phase(
        &self,
        id: String,
        name: String,
        initial_tokens: u64,
        phase: AgentPhase,
    ) -> Result<(), StateError> {
        let agent = AgentRecord {
            id: Uuid::new_v4(),
            name: name.clone(),
            phase,
            tokens: initial_tokens,
            skills: HashMap::new(),
            personality: String::new(),
        };

        match self.agents.entry(id.clone()) {
            Entry::Occupied(_) => return Err(StateError::AgentAlreadyExists(id)),
            Entry::Vacant(vacant) => vacant.insert(agent),
        };

        self.emit(WorldEvent::AgentSpawned {
            agent_id: id.clone(),
            name,
        });

        Ok(())
    }

    /// Deregister (remove) an agent from the world.
    ///
    /// Returns the agent record if found.
    /// Dead agents can be deregistered to clean up state.
    pub fn deregister_agent(&self, id: &str) -> Result<AgentRecord, StateError> {
        self.agents.remove(id)
            .map(|(_, agent)| agent)
            .ok_or_else(|| StateError::AgentNotFound(id.to_string()))
    }

    /// Get a clone of an agent record by ID.
    pub fn get_agent(&self, id: &str) -> Option<AgentRecord> {
        self.agents.get(id).map(|r| r.value().clone())
    }

    /// Check whether an agent exists.
    pub fn agent_exists(&self, id: &str) -> bool {
        self.agents.contains_key(id)
    }

    /// Get the number of registered agents.
    pub fn agent_count(&self) -> usize {
        self.agents.len()
    }

    /// Get the number of living agents (not Dead phase).
    pub fn living_agent_count(&self) -> usize {
        self.agents.iter()
            .filter(|entry| entry.value().phase != AgentPhase::Dead)
            .count()
    }

    /// List all agent IDs.
    pub fn agent_ids(&self) -> Vec<String> {
        self.agents.iter().map(|entry| entry.key().clone()).collect()
    }

    /// List all agent records.
    pub fn list_agents(&self) -> Vec<AgentRecord> {
        self.agents.iter().map(|entry| entry.value().clone()).collect()
    }

    /// List agents filtered by phase.
    pub fn agents_by_phase(&self, phase: AgentPhase) -> Vec<AgentRecord> {
        self.agents.iter()
            .filter(|entry| entry.value().phase == phase)
            .map(|entry| entry.value().clone())
            .collect()
    }

    // ── Agent Phase Management ────────────────────────────

    /// Update an agent's phase with transition validation.
    ///
    /// Valid transitions follow the lifecycle:
    /// Birth -> Childhood -> Adult -> Elder -> Dying -> Dead
    /// Any living phase may transition directly to Dead.
    ///
    /// Emits a `PhaseChanged` event on success.
    pub fn set_agent_phase(&self, agent_id: &str, new_phase: AgentPhase) -> Result<(), StateError> {
        let mut agent = self.agents.get_mut(agent_id)
            .ok_or_else(|| StateError::AgentNotFound(agent_id.to_string()))?;

        let old_phase = agent.phase;

        if !is_valid_phase_transition(old_phase, new_phase) {
            return Err(StateError::InvalidPhaseTransition {
                agent_id: agent_id.to_string(),
                from: old_phase,
                to: new_phase,
            });
        }

        agent.phase = new_phase;
        drop(agent); // release the DashMap guard before emitting

        self.emit(WorldEvent::PhaseChanged {
            agent_id: agent_id.to_string(),
            old_phase,
            new_phase,
        });

        Ok(())
    }

    /// Transition an agent to the Dead phase with the given reason.
    pub fn kill_agent(&self, agent_id: &str, reason: DeathReason) -> Result<AgentPhase, StateError> {
        let mut agent = self.agents.get_mut(agent_id)
            .ok_or_else(|| StateError::AgentNotFound(agent_id.to_string()))?;

        if agent.phase == AgentPhase::Dead {
            return Err(StateError::AgentAlreadyDead(agent_id.to_string()));
        }

        let old_phase = agent.phase;
        agent.phase = AgentPhase::Dead;

        drop(agent);

        self.emit(WorldEvent::AgentDied {
            agent_id: agent_id.to_string(),
            reason,
        });

        Ok(old_phase)
    }

    // ── Token Balance Management ──────────────────────────

    /// Get the token balance for an agent.
    ///
    /// Returns 0 if the agent is not found.
    pub fn token_balance(&self, agent_id: &str) -> u64 {
        self.agents.get(agent_id)
            .map(|r| r.tokens)
            .unwrap_or(0)
    }

    /// Set the token balance for an agent.
    ///
    /// Updates the balance directly in the AgentRecord.
    pub fn set_token_balance(&self, agent_id: &str, amount: u64) -> Result<(), StateError> {
        let mut agent = self.agents.get_mut(agent_id)
            .ok_or_else(|| StateError::AgentNotFound(agent_id.to_string()))?;

        let old_balance = agent.tokens;
        agent.tokens = amount;

        self.emit(WorldEvent::BalanceChanged {
            agent_id: agent_id.to_string(),
            currency: crate::world::enums::Currency::Token,
            old_balance,
            new_balance: amount,
        });

        Ok(())
    }

    /// Add tokens to an agent's balance.
    ///
    /// Uses saturating addition to prevent overflow.
    /// The read-modify-write is atomic via DashMap's entry lock.
    pub fn add_tokens(&self, agent_id: &str, amount: u64) -> Result<u64, StateError> {
        let mut agent = self.agents.get_mut(agent_id)
            .ok_or_else(|| StateError::AgentNotFound(agent_id.to_string()))?;

        let old_balance = agent.tokens;
        let new_balance = old_balance.saturating_add(amount);
        agent.tokens = new_balance;

        self.emit(WorldEvent::BalanceChanged {
            agent_id: agent_id.to_string(),
            currency: crate::world::enums::Currency::Token,
            old_balance,
            new_balance,
        });

        Ok(new_balance)
    }

    /// Deduct tokens from an agent's balance.
    ///
    /// Returns an error if the agent doesn't have enough tokens.
    /// The read-modify-write is atomic via DashMap's entry lock.
    pub fn deduct_tokens(&self, agent_id: &str, amount: u64) -> Result<u64, StateError> {
        let mut agent = self.agents.get_mut(agent_id)
            .ok_or_else(|| StateError::AgentNotFound(agent_id.to_string()))?;

        let old_balance = agent.tokens;

        if old_balance < amount {
            return Err(StateError::InsufficientTokens {
                agent_id: agent_id.to_string(),
                required: amount,
                available: old_balance,
            });
        }

        let new_balance = old_balance - amount;
        agent.tokens = new_balance;

        self.emit(WorldEvent::BalanceChanged {
            agent_id: agent_id.to_string(),
            currency: crate::world::enums::Currency::Token,
            old_balance,
            new_balance,
        });

        Ok(new_balance)
    }

    /// Deduct tokens, clamping to zero instead of erroring on insufficient balance.
    ///
    /// Returns the actual amount deducted.
    /// The read-modify-write is atomic via DashMap's entry lock.
    pub fn deduct_tokens_clamped(&self, agent_id: &str, amount: u64) -> Result<u64, StateError> {
        let mut agent = self.agents.get_mut(agent_id)
            .ok_or_else(|| StateError::AgentNotFound(agent_id.to_string()))?;

        let old_balance = agent.tokens;
        let actual_deduction = amount.min(old_balance);
        let new_balance = old_balance - actual_deduction;
        agent.tokens = new_balance;

        self.emit(WorldEvent::BalanceChanged {
            agent_id: agent_id.to_string(),
            currency: crate::world::enums::Currency::Token,
            old_balance,
            new_balance,
        });

        Ok(actual_deduction)
    }

    /// Get the total token supply across all agents.
    pub fn total_token_supply(&self) -> u64 {
        self.agents.iter()
            .map(|entry| entry.value().tokens)
            .fold(0u64, |acc, v| acc.saturating_add(v))
    }

    // ── Task CRUD ─────────────────────────────────────────

    /// Create a new task in the world state.
    ///
    /// Returns the task UUID string on success.
    pub fn create_task(
        &self,
        title: String,
        description: String,
        reward: u64,
        publisher_id: String,
        expires_at: Option<u64>,
    ) -> String {
        let id = Uuid::new_v4();
        let tick = self.tick();
        let task = Task {
            id,
            title,
            description,
            status: TaskStatus::Published,
            reward,
            currency: crate::world::enums::Currency::Token,
            escrow_held: reward > 0,
            publisher_id: publisher_id.clone(),
            assignee_id: None,
            result: None,
            expires_at,
            created_tick: tick,
        };

        let task_id = id.to_string();
        self.tasks.insert(task_id.clone(), task);

        self.emit(WorldEvent::TaskCreated {
            task_id: task_id.clone(),
            publisher: publisher_id,
            reward,
        });

        task_id
    }

    /// Get a task by its UUID string.
    pub fn get_task(&self, task_id: &str) -> Option<Task> {
        self.tasks.get(task_id).map(|r| r.value().clone())
    }

    /// List all tasks.
    pub fn list_tasks(&self) -> Vec<Task> {
        self.tasks.iter().map(|entry| entry.value().clone()).collect()
    }

    /// List tasks by status.
    pub fn tasks_by_status(&self, status: TaskStatus) -> Vec<Task> {
        self.tasks.iter()
            .filter(|entry| entry.value().status == status)
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// Update a task's status.
    ///
    /// Returns `InvalidTaskTransition` if the transition is not valid per the
    /// task lifecycle state machine.
    pub fn update_task_status(&self, task_id: &str, new_status: TaskStatus) -> Result<(), StateError> {
        let mut task = self.tasks.get_mut(task_id)
            .ok_or_else(|| StateError::TaskNotFound(task_id.to_string()))?;

        let old_status = task.status;
        if !task.status.can_transition_to(&new_status) {
            return Err(StateError::InvalidTaskTransition {
                task_id: task_id.to_string(),
                from: old_status,
                to: new_status,
            });
        }

        task.status = new_status;
        Ok(())
    }

    /// Remove a task from the world state.
    pub fn remove_task(&self, task_id: &str) -> Result<Task, StateError> {
        self.tasks.remove(task_id)
            .map(|(_, task)| task)
            .ok_or_else(|| StateError::TaskNotFound(task_id.to_string()))
    }

    /// Get the number of tasks.
    pub fn task_count(&self) -> usize {
        self.tasks.len()
    }

    // ── Ledger Access ─────────────────────────────────────

    /// Record a transaction in the shared ledger.
    ///
    /// Returns the ledger entry UUID.
    /// Recovers from lock poisoning rather than panicking.
    #[allow(clippy::too_many_arguments)]
    pub fn record_transaction(
        &self,
        from_agent: Option<String>,
        to_agent: Option<String>,
        amount: u64,
        currency: crate::world::enums::Currency,
        tx_type: crate::economy::reward::TransactionType,
        description: String,
        reference_id: Option<String>,
    ) -> Uuid {
        let tick = self.tick();
        let mut ledger = self.ledger.write().unwrap_or_else(|e| e.into_inner());
        ledger.record(from_agent, to_agent, amount, currency, tx_type, description, tick, reference_id)
    }

    /// Get all ledger entries (cloned).
    ///
    /// Recovers from lock poisoning rather than panicking.
    pub fn ledger_entries(&self) -> Vec<LedgerEntry> {
        let ledger = self.ledger.read().unwrap_or_else(|e| e.into_inner());
        ledger.list().to_vec()
    }

    /// Get the number of ledger entries.
    ///
    /// Recovers from lock poisoning rather than panicking.
    pub fn ledger_entry_count(&self) -> usize {
        let ledger = self.ledger.read().unwrap_or_else(|e| e.into_inner());
        ledger.list().len()
    }

    // ── Culture Store Access ──────────────────────────────

    /// Get a reference to the culture store.
    pub fn culture_store(&self) -> &CultureStore {
        &self.culture_store
    }

    // ── Helpers ───────────────────────────────────────────

    /// Emit an event if an event bus is configured.
    fn emit(&self, event: WorldEvent) {
        if let Some(ref bus) = self.event_bus {
            bus.emit(event);
        }
    }
}

impl Default for WorldState {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::enums::DeathReason;

    fn make_state() -> WorldState {
        WorldState::new()
    }

    // ── Tick Counter ──────────────────────────────────────

    #[test]
    fn test_tick_starts_at_zero() {
        let state = make_state();
        assert_eq!(state.tick(), 0);
    }

    #[test]
    fn test_advance_tick_increments() {
        let state = make_state();
        assert_eq!(state.advance_tick(), 1);
        assert_eq!(state.advance_tick(), 2);
        assert_eq!(state.advance_tick(), 3);
        assert_eq!(state.tick(), 3);
    }

    #[test]
    fn test_set_tick() {
        let state = make_state();
        state.set_tick(42);
        assert_eq!(state.tick(), 42);
        state.set_tick(0);
        assert_eq!(state.tick(), 0);
    }

    #[test]
    fn test_advance_tick_after_set() {
        let state = make_state();
        state.set_tick(100);
        assert_eq!(state.advance_tick(), 101);
    }

    // ── Agent Registration ────────────────────────────────

    #[test]
    fn test_register_agent_basic() {
        let state = make_state();
        state.register_agent("a1".to_string(), "Alice".to_string(), 1000).unwrap();

        assert!(state.agent_exists("a1"));
        assert_eq!(state.agent_count(), 1);
        assert_eq!(state.living_agent_count(), 1);

        let agent = state.get_agent("a1").unwrap();
        assert_eq!(agent.name, "Alice");
        assert_eq!(agent.phase, AgentPhase::Birth);
        assert_eq!(agent.tokens, 1000);
    }

    #[test]
    fn test_register_agent_duplicate_fails() {
        let state = make_state();
        state.register_agent("a1".to_string(), "Alice".to_string(), 1000).unwrap();
        let result = state.register_agent("a1".to_string(), "Bob".to_string(), 500);
        assert!(matches!(result, Err(StateError::AgentAlreadyExists(_))));
    }

    #[test]
    fn test_register_agent_with_phase() {
        let state = make_state();
        state.register_agent_with_phase(
            "a1".to_string(), "Alice".to_string(), 1000, AgentPhase::Adult
        ).unwrap();

        let agent = state.get_agent("a1").unwrap();
        assert_eq!(agent.phase, AgentPhase::Adult);
    }

    #[test]
    fn test_register_multiple_agents() {
        let state = make_state();
        state.register_agent("a1".to_string(), "Alice".to_string(), 1000).unwrap();
        state.register_agent("a2".to_string(), "Bob".to_string(), 2000).unwrap();
        state.register_agent("a3".to_string(), "Charlie".to_string(), 3000).unwrap();

        assert_eq!(state.agent_count(), 3);
        assert_eq!(state.living_agent_count(), 3);
    }

    // ── Agent Deregistration ──────────────────────────────

    #[test]
    fn test_deregister_agent() {
        let state = make_state();
        state.register_agent("a1".to_string(), "Alice".to_string(), 1000).unwrap();

        let agent = state.deregister_agent("a1").unwrap();
        assert_eq!(agent.name, "Alice");
        assert!(!state.agent_exists("a1"));
        assert_eq!(state.agent_count(), 0);
        assert_eq!(state.token_balance("a1"), 0);
    }

    #[test]
    fn test_deregister_nonexistent_fails() {
        let state = make_state();
        let result = state.deregister_agent("nobody");
        assert!(matches!(result, Err(StateError::AgentNotFound(_))));
    }

    #[test]
    fn test_deregister_cleans_up_balance() {
        let state = make_state();
        state.register_agent("a1".to_string(), "Alice".to_string(), 5000).unwrap();
        assert_eq!(state.token_balance("a1"), 5000);

        state.deregister_agent("a1").unwrap();
        assert_eq!(state.token_balance("a1"), 0);
    }

    // ── Agent Queries ─────────────────────────────────────

    #[test]
    fn test_agent_ids() {
        let state = make_state();
        state.register_agent("a1".to_string(), "Alice".to_string(), 0).unwrap();
        state.register_agent("a2".to_string(), "Bob".to_string(), 0).unwrap();

        let mut ids = state.agent_ids();
        ids.sort();
        assert_eq!(ids, vec!["a1", "a2"]);
    }

    #[test]
    fn test_list_agents() {
        let state = make_state();
        state.register_agent("a1".to_string(), "Alice".to_string(), 100).unwrap();
        state.register_agent("a2".to_string(), "Bob".to_string(), 200).unwrap();

        let agents = state.list_agents();
        assert_eq!(agents.len(), 2);
    }

    #[test]
    fn test_agents_by_phase() {
        let state = make_state();
        state.register_agent("a1".to_string(), "Alice".to_string(), 0).unwrap();
        state.register_agent_with_phase(
            "a2".to_string(), "Bob".to_string(), 0, AgentPhase::Adult
        ).unwrap();
        state.register_agent_with_phase(
            "a3".to_string(), "Charlie".to_string(), 0, AgentPhase::Adult
        ).unwrap();

        assert_eq!(state.agents_by_phase(AgentPhase::Birth).len(), 1);
        assert_eq!(state.agents_by_phase(AgentPhase::Adult).len(), 2);
        assert_eq!(state.agents_by_phase(AgentPhase::Dead).len(), 0);
    }

    #[test]
    fn test_get_nonexistent_agent() {
        let state = make_state();
        assert!(state.get_agent("nobody").is_none());
    }

    #[test]
    fn test_living_agent_count_excludes_dead() {
        let state = make_state();
        state.register_agent("a1".to_string(), "Alice".to_string(), 100).unwrap();
        state.register_agent("a2".to_string(), "Bob".to_string(), 100).unwrap();
        state.register_agent_with_phase(
            "a3".to_string(), "Charlie".to_string(), 0, AgentPhase::Dead
        ).unwrap();

        assert_eq!(state.agent_count(), 3);
        assert_eq!(state.living_agent_count(), 2);
    }

    // ── Agent Phase Management ────────────────────────────

    #[test]
    fn test_set_agent_phase_valid_transitions() {
        let state = make_state();
        state.register_agent("a1".to_string(), "Alice".to_string(), 1000).unwrap();

        state.set_agent_phase("a1", AgentPhase::Childhood).unwrap();
        assert_eq!(state.get_agent("a1").unwrap().phase, AgentPhase::Childhood);

        state.set_agent_phase("a1", AgentPhase::Adult).unwrap();
        assert_eq!(state.get_agent("a1").unwrap().phase, AgentPhase::Adult);

        state.set_agent_phase("a1", AgentPhase::Elder).unwrap();
        assert_eq!(state.get_agent("a1").unwrap().phase, AgentPhase::Elder);

        state.set_agent_phase("a1", AgentPhase::Dying).unwrap();
        assert_eq!(state.get_agent("a1").unwrap().phase, AgentPhase::Dying);

        state.set_agent_phase("a1", AgentPhase::Dead).unwrap();
        assert_eq!(state.get_agent("a1").unwrap().phase, AgentPhase::Dead);
    }

    #[test]
    fn test_set_phase_nonexistent_fails() {
        let state = make_state();
        let result = state.set_agent_phase("nobody", AgentPhase::Adult);
        assert!(matches!(result, Err(StateError::AgentNotFound(_))));
    }

    #[test]
    fn test_invalid_phase_transition_birth_to_adult() {
        let state = make_state();
        state.register_agent("a1".to_string(), "Alice".to_string(), 1000).unwrap();

        let result = state.set_agent_phase("a1", AgentPhase::Adult);
        assert!(matches!(result, Err(StateError::InvalidPhaseTransition { .. })));
        // Phase should remain unchanged
        assert_eq!(state.get_agent("a1").unwrap().phase, AgentPhase::Birth);
    }

    #[test]
    fn test_invalid_phase_transition_dead_to_birth() {
        let state = make_state();
        state.register_agent("a1".to_string(), "Alice".to_string(), 1000).unwrap();
        state.kill_agent("a1", DeathReason::TokenDepleted).unwrap();

        let result = state.set_agent_phase("a1", AgentPhase::Birth);
        assert!(matches!(result, Err(StateError::InvalidPhaseTransition { .. })));
        assert_eq!(state.get_agent("a1").unwrap().phase, AgentPhase::Dead);
    }

    #[test]
    fn test_valid_skip_to_dead() {
        let state = make_state();
        state.register_agent("a1".to_string(), "Alice".to_string(), 1000).unwrap();

        // Any living phase can transition directly to Dead
        state.set_agent_phase("a1", AgentPhase::Dead).unwrap();
        assert_eq!(state.get_agent("a1").unwrap().phase, AgentPhase::Dead);
    }

    #[test]
    fn test_kill_agent() {
        let state = make_state();
        state.register_agent("a1".to_string(), "Alice".to_string(), 1000).unwrap();

        let old_phase = state.kill_agent("a1", DeathReason::TokenDepleted).unwrap();
        assert_eq!(old_phase, AgentPhase::Birth);
        assert_eq!(state.get_agent("a1").unwrap().phase, AgentPhase::Dead);
        assert_eq!(state.living_agent_count(), 0);
    }

    #[test]
    fn test_kill_already_dead_fails() {
        let state = make_state();
        state.register_agent("a1".to_string(), "Alice".to_string(), 1000).unwrap();
        state.kill_agent("a1", DeathReason::TokenDepleted).unwrap();

        let result = state.kill_agent("a1", DeathReason::TokenDepleted);
        assert!(matches!(result, Err(StateError::AgentAlreadyDead(_))));
    }

    #[test]
    fn test_kill_agent_with_reason() {
        let state = make_state();
        state.register_agent("a1".to_string(), "Alice".to_string(), 1000).unwrap();

        let old_phase = state.kill_agent("a1", DeathReason::HumanTerminated).unwrap();
        assert_eq!(old_phase, AgentPhase::Birth);
        assert_eq!(state.get_agent("a1").unwrap().phase, AgentPhase::Dead);
    }

    // ── Token Balance ─────────────────────────────────────

    #[test]
    fn test_token_balance_initial() {
        let state = make_state();
        state.register_agent("a1".to_string(), "Alice".to_string(), 5000).unwrap();
        assert_eq!(state.token_balance("a1"), 5000);
    }

    #[test]
    fn test_token_balance_nonexistent_is_zero() {
        let state = make_state();
        assert_eq!(state.token_balance("nobody"), 0);
    }

    #[test]
    fn test_set_token_balance() {
        let state = make_state();
        state.register_agent("a1".to_string(), "Alice".to_string(), 1000).unwrap();

        state.set_token_balance("a1", 500).unwrap();
        assert_eq!(state.token_balance("a1"), 500);

        // Also synced in AgentRecord
        assert_eq!(state.get_agent("a1").unwrap().tokens, 500);
    }

    #[test]
    fn test_set_token_balance_nonexistent_fails() {
        let state = make_state();
        let result = state.set_token_balance("nobody", 100);
        assert!(matches!(result, Err(StateError::AgentNotFound(_))));
    }

    #[test]
    fn test_add_tokens() {
        let state = make_state();
        state.register_agent("a1".to_string(), "Alice".to_string(), 1000).unwrap();

        let new_balance = state.add_tokens("a1", 500).unwrap();
        assert_eq!(new_balance, 1500);
        assert_eq!(state.token_balance("a1"), 1500);
    }

    #[test]
    fn test_add_tokens_nonexistent_fails() {
        let state = make_state();
        let result = state.add_tokens("nobody", 100);
        assert!(matches!(result, Err(StateError::AgentNotFound(_))));
    }

    #[test]
    fn test_add_tokens_saturates() {
        let state = make_state();
        state.register_agent("a1".to_string(), "Alice".to_string(), u64::MAX).unwrap();

        let new_balance = state.add_tokens("a1", 1).unwrap();
        assert_eq!(new_balance, u64::MAX);
    }

    #[test]
    fn test_deduct_tokens() {
        let state = make_state();
        state.register_agent("a1".to_string(), "Alice".to_string(), 1000).unwrap();

        let new_balance = state.deduct_tokens("a1", 300).unwrap();
        assert_eq!(new_balance, 700);
        assert_eq!(state.token_balance("a1"), 700);
    }

    #[test]
    fn test_deduct_tokens_insufficient_fails() {
        let state = make_state();
        state.register_agent("a1".to_string(), "Alice".to_string(), 100).unwrap();

        let result = state.deduct_tokens("a1", 200);
        assert!(matches!(result, Err(StateError::InsufficientTokens { .. })));
        // Balance unchanged
        assert_eq!(state.token_balance("a1"), 100);
    }

    #[test]
    fn test_deduct_tokens_exact() {
        let state = make_state();
        state.register_agent("a1".to_string(), "Alice".to_string(), 1000).unwrap();

        let new_balance = state.deduct_tokens("a1", 1000).unwrap();
        assert_eq!(new_balance, 0);
    }

    #[test]
    fn test_deduct_tokens_nonexistent_fails() {
        let state = make_state();
        let result = state.deduct_tokens("nobody", 100);
        assert!(matches!(result, Err(StateError::AgentNotFound(_))));
    }

    #[test]
    fn test_deduct_tokens_clamped() {
        let state = make_state();
        state.register_agent("a1".to_string(), "Alice".to_string(), 50).unwrap();

        let actual = state.deduct_tokens_clamped("a1", 100).unwrap();
        assert_eq!(actual, 50);
        assert_eq!(state.token_balance("a1"), 0);
    }

    #[test]
    fn test_deduct_tokens_clamped_exact() {
        let state = make_state();
        state.register_agent("a1".to_string(), "Alice".to_string(), 100).unwrap();

        let actual = state.deduct_tokens_clamped("a1", 100).unwrap();
        assert_eq!(actual, 100);
        assert_eq!(state.token_balance("a1"), 0);
    }

    #[test]
    fn test_deduct_tokens_clamped_nonexistent_fails() {
        let state = make_state();
        let result = state.deduct_tokens_clamped("nobody", 100);
        assert!(matches!(result, Err(StateError::AgentNotFound(_))));
    }

    #[test]
    fn test_total_token_supply() {
        let state = make_state();
        state.register_agent("a1".to_string(), "Alice".to_string(), 1000).unwrap();
        state.register_agent("a2".to_string(), "Bob".to_string(), 2000).unwrap();
        state.register_agent("a3".to_string(), "Charlie".to_string(), 3000).unwrap();

        assert_eq!(state.total_token_supply(), 6000);
    }

    #[test]
    fn test_total_token_supply_after_deduction() {
        let state = make_state();
        state.register_agent("a1".to_string(), "Alice".to_string(), 1000).unwrap();
        state.register_agent("a2".to_string(), "Bob".to_string(), 2000).unwrap();

        state.deduct_tokens("a1", 500).unwrap();
        assert_eq!(state.total_token_supply(), 2500);
    }

    #[test]
    fn test_total_token_supply_empty() {
        let state = make_state();
        assert_eq!(state.total_token_supply(), 0);
    }

    // ── Task CRUD ─────────────────────────────────────────

    #[test]
    fn test_create_task() {
        let state = make_state();
        state.set_tick(10);

        let task_id = state.create_task(
            "Test Task".to_string(),
            "A test".to_string(),
            100,
            "publisher-1".to_string(),
            Some(100),
        );

        let task = state.get_task(&task_id).unwrap();
        assert_eq!(task.title, "Test Task");
        assert_eq!(task.description, "A test");
        assert_eq!(task.reward, 100);
        assert_eq!(task.status, TaskStatus::Published);
        assert_eq!(task.publisher_id, "publisher-1");
        assert_eq!(task.expires_at, Some(100));
        assert_eq!(task.created_tick, 10);
    }

    #[test]
    fn test_create_task_no_reward() {
        let state = make_state();
        let task_id = state.create_task(
            "Free Task".to_string(),
            "No reward".to_string(),
            0,
            "publisher-1".to_string(),
            None,
        );

        let task = state.get_task(&task_id).unwrap();
        assert_eq!(task.reward, 0);
        assert!(!task.escrow_held);
    }

    #[test]
    fn test_list_tasks() {
        let state = make_state();
        let _id1 = state.create_task("T1".to_string(), "".to_string(), 0, "p1".to_string(), None);
        let _id2 = state.create_task("T2".to_string(), "".to_string(), 0, "p1".to_string(), None);

        let tasks = state.list_tasks();
        assert_eq!(tasks.len(), 2);
    }

    #[test]
    fn test_tasks_by_status() {
        let state = make_state();
        let id1 = state.create_task("T1".to_string(), "".to_string(), 0, "p1".to_string(), None);
        let _id2 = state.create_task("T2".to_string(), "".to_string(), 0, "p1".to_string(), None);

        state.update_task_status(&id1, TaskStatus::Claimed).unwrap();

        assert_eq!(state.tasks_by_status(TaskStatus::Published).len(), 1);
        assert_eq!(state.tasks_by_status(TaskStatus::Claimed).len(), 1);
    }

    #[test]
    fn test_update_task_status_valid_transition() {
        let state = make_state();
        let task_id = state.create_task("T1".to_string(), "".to_string(), 0, "p1".to_string(), None);

        state.update_task_status(&task_id, TaskStatus::Claimed).unwrap();
        assert_eq!(state.get_task(&task_id).unwrap().status, TaskStatus::Claimed);

        state.update_task_status(&task_id, TaskStatus::InProgress).unwrap();
        assert_eq!(state.get_task(&task_id).unwrap().status, TaskStatus::InProgress);
    }

    #[test]
    fn test_update_task_status_invalid_transition() {
        let state = make_state();
        let task_id = state.create_task("T1".to_string(), "".to_string(), 0, "p1".to_string(), None);

        let result = state.update_task_status(&task_id, TaskStatus::Completed);
        assert!(matches!(result, Err(StateError::InvalidTaskTransition { .. })));
        // Status should remain unchanged
        assert_eq!(state.get_task(&task_id).unwrap().status, TaskStatus::Published);
    }

    #[test]
    fn test_update_task_status_not_found() {
        let state = make_state();
        let result = state.update_task_status("nonexistent", TaskStatus::Claimed);
        assert!(matches!(result, Err(StateError::TaskNotFound(_))));
    }

    #[test]
    fn test_remove_task() {
        let state = make_state();
        let task_id = state.create_task("T1".to_string(), "".to_string(), 0, "p1".to_string(), None);

        let task = state.remove_task(&task_id).unwrap();
        assert_eq!(task.title, "T1");
        assert!(state.get_task(&task_id).is_none());
        assert_eq!(state.task_count(), 0);
    }

    #[test]
    fn test_remove_task_nonexistent() {
        let state = make_state();
        let result = state.remove_task("nonexistent");
        assert!(matches!(result, Err(StateError::TaskNotFound(_))));
    }

    #[test]
    fn test_task_count() {
        let state = make_state();
        assert_eq!(state.task_count(), 0);

        state.create_task("T1".to_string(), "".to_string(), 0, "p1".to_string(), None);
        assert_eq!(state.task_count(), 1);

        state.create_task("T2".to_string(), "".to_string(), 0, "p1".to_string(), None);
        assert_eq!(state.task_count(), 2);
    }

    // ── Ledger ────────────────────────────────────────────

    #[test]
    fn test_record_transaction() {
        let state = make_state();
        state.set_tick(42);

        let id = state.record_transaction(
            Some("a1".to_string()),
            Some("a2".to_string()),
            100,
            crate::world::enums::Currency::Token,
            crate::economy::reward::TransactionType::TaskReward,
            "test transaction".to_string(),
            Some("task-1".to_string()),
        );

        assert!(!id.is_nil());
        assert_eq!(state.ledger_entry_count(), 1);

        let entries = state.ledger_entries();
        assert_eq!(entries[0].amount, 100);
        assert_eq!(entries[0].tick, 42);
        assert_eq!(entries[0].from_agent.as_deref(), Some("a1"));
        assert_eq!(entries[0].to_agent.as_deref(), Some("a2"));
    }

    #[test]
    fn test_multiple_transactions() {
        let state = make_state();

        state.record_transaction(
            Some("a1".to_string()), Some("a2".to_string()), 100,
            crate::world::enums::Currency::Token,
            crate::economy::reward::TransactionType::TaskReward,
            "tx1".to_string(), None,
        );
        state.record_transaction(
            Some("a2".to_string()), None, 2,
            crate::world::enums::Currency::Token,
            crate::economy::reward::TransactionType::PlatformFee,
            "tx2".to_string(), None,
        );

        assert_eq!(state.ledger_entry_count(), 2);
    }

    // ── Event Bus Integration ─────────────────────────────

    #[test]
    fn test_emit_tick_advanced_event() {
        let bus = EventBus::new(64);
        let mut rx = bus.subscribe();
        let state = WorldState::with_event_bus(bus);

        state.advance_tick();
        let event = rx.try_recv().unwrap();
        assert_eq!(event, WorldEvent::TickAdvanced { tick: 1 });
    }

    #[test]
    fn test_emit_agent_spawned_event() {
        let bus = EventBus::new(64);
        let mut rx = bus.subscribe();
        let state = WorldState::with_event_bus(bus);

        state.register_agent("a1".to_string(), "Alice".to_string(), 1000).unwrap();

        let event = rx.try_recv().unwrap();
        assert!(matches!(event, WorldEvent::AgentSpawned { .. }));
        if let WorldEvent::AgentSpawned { agent_id, name } = event {
            assert_eq!(agent_id, "a1");
            assert_eq!(name, "Alice");
        }
    }

    #[test]
    fn test_emit_phase_changed_event() {
        let bus = EventBus::new(64);
        let mut rx = bus.subscribe();
        let state = WorldState::with_event_bus(bus);

        state.register_agent("a1".to_string(), "Alice".to_string(), 1000).unwrap();
        let _ = rx.try_recv(); // consume AgentSpawned

        state.set_agent_phase("a1", AgentPhase::Childhood).unwrap();
        let event = rx.try_recv().unwrap();
        assert_eq!(
            event,
            WorldEvent::PhaseChanged {
                agent_id: "a1".to_string(),
                old_phase: AgentPhase::Birth,
                new_phase: AgentPhase::Childhood,
            }
        );
    }

    #[test]
    fn test_emit_balance_changed_event() {
        let bus = EventBus::new(64);
        let mut rx = bus.subscribe();
        let state = WorldState::with_event_bus(bus);

        state.register_agent("a1".to_string(), "Alice".to_string(), 1000).unwrap();
        let _ = rx.try_recv(); // consume AgentSpawned

        state.set_token_balance("a1", 500).unwrap();
        let event = rx.try_recv().unwrap();
        assert_eq!(
            event,
            WorldEvent::BalanceChanged {
                agent_id: "a1".to_string(),
                currency: crate::world::enums::Currency::Token,
                old_balance: 1000,
                new_balance: 500,
            }
        );
    }

    #[test]
    fn test_emit_agent_died_event() {
        let bus = EventBus::new(64);
        let mut rx = bus.subscribe();
        let state = WorldState::with_event_bus(bus);

        state.register_agent("a1".to_string(), "Alice".to_string(), 1000).unwrap();
        let _ = rx.try_recv(); // consume AgentSpawned

        state.kill_agent("a1", DeathReason::TokenDepleted).unwrap();
        let event = rx.try_recv().unwrap();
        assert!(matches!(event, WorldEvent::AgentDied { .. }));
        if let WorldEvent::AgentDied { agent_id, reason } = event {
            assert_eq!(agent_id, "a1");
            assert_eq!(reason, DeathReason::TokenDepleted);
        }
    }

    #[test]
    fn test_emit_task_created_event() {
        let bus = EventBus::new(64);
        let mut rx = bus.subscribe();
        let state = WorldState::with_event_bus(bus);

        state.create_task(
            "Test Task".to_string(), "desc".to_string(), 100,
            "publisher-1".to_string(), None,
        );

        let event = rx.try_recv().unwrap();
        assert!(matches!(event, WorldEvent::TaskCreated { .. }));
        if let WorldEvent::TaskCreated { task_id, publisher, reward } = event {
            assert_eq!(publisher, "publisher-1");
            assert_eq!(reward, 100);
            assert!(!task_id.is_empty());
        }
    }

    #[test]
    fn test_no_event_without_bus() {
        let state = make_state();
        // These should not panic even without an event bus
        state.advance_tick();
        state.register_agent("a1".to_string(), "Alice".to_string(), 1000).unwrap();
        state.set_agent_phase("a1", AgentPhase::Childhood).unwrap();
        state.set_token_balance("a1", 500).unwrap();
        state.kill_agent("a1", DeathReason::TokenDepleted).unwrap();
    }

    // ── Thread Safety ─────────────────────────────────────

    #[test]
    fn test_concurrent_agent_registration() {
        use std::sync::Arc;
        use std::thread;

        let state = Arc::new(make_state());
        let mut handles = vec![];

        for i in 0..100 {
            let state = state.clone();
            handles.push(thread::spawn(move || {
                let id = format!("agent-{}", i);
                state.register_agent(id, format!("Agent {}", i), 1000).unwrap();
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(state.agent_count(), 100);
    }

    #[test]
    fn test_concurrent_tick_advance() {
        use std::sync::Arc;
        use std::thread;

        let state = Arc::new(make_state());
        let mut handles = vec![];

        for _ in 0..10 {
            let state = state.clone();
            handles.push(thread::spawn(move || {
                for _ in 0..100 {
                    state.advance_tick();
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(state.tick(), 1000);
    }

    #[test]
    fn test_concurrent_token_operations() {
        use std::sync::Arc;
        use std::thread;

        let state = Arc::new(make_state());
        state.register_agent("a1".to_string(), "Alice".to_string(), 10000).unwrap();

        let mut handles = vec![];

        // 10 threads each deduct 100 tokens
        for _ in 0..10 {
            let state = state.clone();
            handles.push(thread::spawn(move || {
                state.deduct_tokens("a1", 100).unwrap();
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(state.token_balance("a1"), 9000);
    }

    #[test]
    fn test_concurrent_register_same_id() {
        use std::sync::Arc;
        use std::thread;

        let state = Arc::new(make_state());
        let mut handles = vec![];

        // All threads try to register the same ID — exactly one should succeed
        for i in 0..20 {
            let state = state.clone();
            handles.push(thread::spawn(move || {
                state.register_agent("same-id".to_string(), format!("Agent {}", i), 1000)
            }));
        }

        let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        let successes = results.iter().filter(|r| r.is_ok()).count();
        let failures = results.iter().filter(|r| r.is_err()).count();

        assert_eq!(successes, 1);
        assert_eq!(failures, 19);
        assert_eq!(state.agent_count(), 1);
    }

    // ── Error Display ─────────────────────────────────────

    #[test]
    fn test_error_display() {
        assert!(StateError::AgentNotFound("a1".to_string()).to_string().contains("a1"));
        assert!(StateError::AgentAlreadyExists("a1".to_string()).to_string().contains("a1"));
        assert!(StateError::AgentAlreadyDead("a1".to_string()).to_string().contains("dead"));
        assert!(StateError::TaskNotFound("t1".to_string()).to_string().contains("t1"));
        assert!(StateError::InsufficientTokens {
            agent_id: "a1".to_string(),
            required: 100,
            available: 50,
        }.to_string().contains("50"));
        assert!(StateError::InvalidTaskTransition {
            task_id: "t1".to_string(),
            from: TaskStatus::Published,
            to: TaskStatus::Completed,
        }.to_string().contains("t1"));
        assert!(StateError::InvalidPhaseTransition {
            agent_id: "a1".to_string(),
            from: AgentPhase::Dead,
            to: AgentPhase::Birth,
        }.to_string().contains("invalid"));
    }

    // ── Default ───────────────────────────────────────────

    #[test]
    fn test_default() {
        let state = WorldState::default();
        assert_eq!(state.tick(), 0);
        assert_eq!(state.agent_count(), 0);
        assert_eq!(state.task_count(), 0);
    }
}
