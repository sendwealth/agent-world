//! # Human-as-Agent Layer (Phase 5.5)
//!
//! Lets a real human incarnate as a peer agent inside the world.
//! Human actions are queued via REST and drained by [`HumanAgentSubsystem`]
//! at the start of every tick, so humans and AI agents share the same
//! survival rules (token burn, death judgment, newbie protection).
//!
//! Key types: [`HumanActionQueue`], [`HumanAgentRegistry`],
//!            [`HumanAgentSubsystem`], [`QueuedAction`].
//!
//! Wire-up lives in [`crate::api_human_agent`].

use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::world::agent::AgentRecord;
use crate::world::enums::AgentPhase;
use crate::world::event::WorldEvent;

// ═══════════════════════════════════════════════════════════════════════════
// Constants
// ═══════════════════════════════════════════════════════════════════════════

/// Number of ticks a human agent can go without an action before the
/// auto-pilot kicks in and submits a `rest` on their behalf.
pub const HUMAN_IDLE_TIMEOUT_TICKS: u64 = 5;

/// Initial token grant for a freshly incarnated human agent.
pub const HUMAN_INITIAL_TOKENS: u64 = 200;

/// Initial money grant for a freshly incarnated human agent.
pub const HUMAN_INITIAL_MONEY: u64 = 100;

/// Token cost per action — delegates to `HumanActionType::token_cost()` for a single source of truth.
pub fn action_token_cost(action: &str) -> u64 {
    HumanActionType::from_str_lossy(action)
        .map(|a| a.token_cost())
        .unwrap_or(0)
}

/// Token income per action — delegates to `HumanActionType::token_income()` for a single source of truth.
pub fn action_token_income(action: &str) -> u64 {
    HumanActionType::from_str_lossy(action)
        .map(|a| a.token_income())
        .unwrap_or(0)
}

// ═══════════════════════════════════════════════════════════════════════════
// Human Action Type (typed enum for validation + cost/income lookup)
// ═══════════════════════════════════════════════════════════════════════════

/// Typed action types that a human player can submit.
///
/// Provides validated `token_cost()` / `token_income()` lookups and
/// round-trip string conversion. Used by the `/human/*` routes for
/// input validation before enqueuing a [`QueuedAction`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum HumanActionType {
    Communicate,
    Trade,
    Rest,
    Explore,
    Gather,
    Build,
    Socialize,
    PracticeSkill,
    TeachSkill,
    Move,
    ClaimTask,
    SubmitTask,
}

impl HumanActionType {
    /// Token cost for each action type (server-authoritative).
    pub fn token_cost(&self) -> u64 {
        match self {
            HumanActionType::Communicate => 10,
            HumanActionType::Trade => 10,
            HumanActionType::Rest => 0,
            HumanActionType::Explore => 3,
            HumanActionType::Gather => 0,
            HumanActionType::Build => 20,
            HumanActionType::Socialize => 5,
            HumanActionType::PracticeSkill => 8,
            HumanActionType::TeachSkill => 15,
            HumanActionType::Move => 12,
            HumanActionType::ClaimTask => 5,
            HumanActionType::SubmitTask => 8,
        }
    }

    /// Token income for each action type.
    pub fn token_income(&self) -> u64 {
        match self {
            HumanActionType::Rest => 5,
            HumanActionType::Explore => 2,
            HumanActionType::Gather => 3,
            HumanActionType::Build => 5,
            HumanActionType::SubmitTask => 15,
            _ => 0,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            HumanActionType::Communicate => "communicate",
            HumanActionType::Trade => "trade",
            HumanActionType::Rest => "rest",
            HumanActionType::Explore => "explore",
            HumanActionType::Gather => "gather",
            HumanActionType::Build => "build",
            HumanActionType::Socialize => "socialize",
            HumanActionType::PracticeSkill => "practice_skill",
            HumanActionType::TeachSkill => "teach_skill",
            HumanActionType::Move => "move",
            HumanActionType::ClaimTask => "claim_task",
            HumanActionType::SubmitTask => "submit_task",
        }
    }

    pub fn from_str_lossy(s: &str) -> Option<Self> {
        match s {
            "communicate" => Some(HumanActionType::Communicate),
            "trade" => Some(HumanActionType::Trade),
            "rest" => Some(HumanActionType::Rest),
            "explore" => Some(HumanActionType::Explore),
            "gather" => Some(HumanActionType::Gather),
            "build" => Some(HumanActionType::Build),
            "socialize" => Some(HumanActionType::Socialize),
            "practice_skill" => Some(HumanActionType::PracticeSkill),
            "teach_skill" => Some(HumanActionType::TeachSkill),
            "move" => Some(HumanActionType::Move),
            "claim_task" => Some(HumanActionType::ClaimTask),
            "submit_task" => Some(HumanActionType::SubmitTask),
            _ => None,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Queued Action
// ═══════════════════════════════════════════════════════════════════════════

/// A human-submitted action waiting to be applied at the next tick.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueuedAction {
    /// Unique id for this queued action (used for dedupe / cancel).
    pub id: String,
    /// The incarnated agent's id (matches the `ExternalAgent` / `AgentDto` id).
    pub agent_id: String,
    /// Action verb — must be in `ALLOWED_ACTIONS`.
    pub action: String,
    /// Free-form JSON params (e.g. `{"direction":"north"}`).
    #[serde(default)]
    pub params: serde_json::Value,
    /// Tick at which the action was enqueued.
    pub enqueued_tick: u64,
    /// Whether the action has been applied by the subsystem.
    #[serde(default)]
    pub applied: bool,
}

// ═══════════════════════════════════════════════════════════════════════════
// Human Action Queue
// ═══════════════════════════════════════════════════════════════════════════

/// Per-process FIFO of pending human actions.
///
/// Wrapped in an [`Arc<Mutex<…>>`] and shared between the REST handlers
/// (producers) and [`HumanAgentSubsystem`] (consumer). Bounded to
/// `MAX_QUEUED_ACTIONS` per agent to prevent runaway memory growth.
#[derive(Debug, Default)]
pub struct HumanActionQueue {
    actions: Vec<QueuedAction>,
}

/// Maximum number of unprocessed actions an agent may have queued at once.
pub const MAX_QUEUED_ACTIONS_PER_AGENT: usize = 16;

pub type SharedHumanActionQueue = Arc<Mutex<HumanActionQueue>>;

impl HumanActionQueue {
    /// Create an empty queue.
    pub fn new() -> Self {
        Self::default()
    }

    /// Shared, empty queue.
    pub fn shared() -> SharedHumanActionQueue {
        Arc::new(Mutex::new(Self::new()))
    }

    /// Push a new action. Returns `Err(message)` if the per-agent cap is hit.
    pub fn enqueue(&mut self, mut action: QueuedAction) -> Result<(), String> {
        let count = self
            .actions
            .iter()
            .filter(|a| a.agent_id == action.agent_id && !a.applied)
            .count();
        if count >= MAX_QUEUED_ACTIONS_PER_AGENT {
            return Err(format!(
                "agent {} already has {} pending actions",
                action.agent_id, count
            ));
        }
        if action.id.is_empty() {
            action.id = Uuid::new_v4().to_string();
        }
        self.actions.push(action);
        Ok(())
    }

    /// Drain all pending (not-yet-applied) actions for a given agent,
    /// preserving enqueue order. Returned actions are marked `applied`
    /// in the underlying store so subsequent calls do not re-emit them.
    pub fn drain_for_agent(&mut self, agent_id: &str) -> Vec<QueuedAction> {
        let mut out = Vec::new();
        for a in self.actions.iter_mut() {
            if a.agent_id == agent_id && !a.applied {
                a.applied = true;
                out.push(a.clone());
            }
        }
        out
    }

    /// Drain all pending actions across every agent, grouped by agent_id.
    pub fn drain_all(&mut self) -> HashMap<String, Vec<QueuedAction>> {
        let mut grouped: HashMap<String, Vec<QueuedAction>> = HashMap::new();
        for a in self.actions.iter_mut() {
            if !a.applied {
                a.applied = true;
                grouped.entry(a.agent_id.clone()).or_default().push(a.clone());
            }
        }
        grouped
    }

    /// List pending actions for an agent (does not mark them applied).
    pub fn pending_for_agent(&self, agent_id: &str) -> Vec<&QueuedAction> {
        self.actions
            .iter()
            .filter(|a| a.agent_id == agent_id && !a.applied)
            .collect()
    }

    /// Total number of pending (unapplied) actions across all agents.
    pub fn pending_count(&self) -> usize {
        self.actions.iter().filter(|a| !a.applied).count()
    }

    /// Drop applied actions older than `retain_since_tick` to keep memory bounded.
    pub fn gc(&mut self, retain_since_tick: u64) {
        self.actions.retain(|a| !a.applied || a.enqueued_tick >= retain_since_tick);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Human Agent Registry
// ═══════════════════════════════════════════════════════════════════════════

/// A human-incarnated agent's bookkeeping record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HumanAgent {
    /// Agent id (matches the WorldState / ExternalAgent / AgentDto id).
    pub agent_id: String,
    /// The authenticated human user id that incarnated this agent.
    pub human_id: String,
    /// Display name (player-chosen).
    pub name: String,
    /// Initial token grant.
    pub initial_tokens: u64,
    /// Initial money grant.
    pub initial_money: u64,
    /// Tick at which the incarnation happened.
    pub spawned_tick: u64,
    /// Last tick at which the human submitted an action.
    pub last_action_tick: u64,
    /// Whether the agent is still alive.
    pub alive: bool,
    /// Free-form display metadata (e.g. chosen avatar emoji).
    #[serde(default)]
    pub metadata: serde_json::Value,
}

/// Maps `human_id` → active incarnation, plus reverse lookup `agent_id` → `human_id`.
#[derive(Debug, Default)]
pub struct HumanAgentRegistry {
    by_agent: HashMap<String, HumanAgent>,
    by_human: HashMap<String, String>, // human_id -> agent_id
}

pub type SharedHumanAgentRegistry = Arc<Mutex<HumanAgentRegistry>>;

impl HumanAgentRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn shared() -> SharedHumanAgentRegistry {
        Arc::new(Mutex::new(Self::new()))
    }

    /// Register a new incarnation. Returns `Err` if the human already has
    /// a living incarnation.
    pub fn register(&mut self, agent: HumanAgent) -> Result<(), String> {
        if let Some(existing_id) = self.by_human.get(&agent.human_id) {
            if let Some(existing) = self.by_agent.get(existing_id) {
                if existing.alive {
                    return Err(format!(
                        "human {} already has a living incarnation (agent {})",
                        agent.human_id, existing.agent_id
                    ));
                }
            }
        }
        self.by_human
            .insert(agent.human_id.clone(), agent.agent_id.clone());
        self.by_agent.insert(agent.agent_id.clone(), agent);
        Ok(())
    }

    pub fn get_by_agent(&self, agent_id: &str) -> Option<&HumanAgent> {
        self.by_agent.get(agent_id)
    }

    pub fn get_by_human(&self, human_id: &str) -> Option<&HumanAgent> {
        self.by_human
            .get(human_id)
            .and_then(|aid| self.by_agent.get(aid))
    }

    /// Mark the incarnation dead (called when the engine kills the agent).
    pub fn mark_dead(&mut self, agent_id: &str) {
        if let Some(a) = self.by_agent.get_mut(agent_id) {
            a.alive = false;
        }
    }

    /// Update `last_action_tick` for the given agent.
    pub fn touch_action(&mut self, agent_id: &str, tick: u64) {
        if let Some(a) = self.by_agent.get_mut(agent_id) {
            a.last_action_tick = tick;
        }
    }

    /// Iterate every incarnation (alive or dead).
    pub fn iter_all(&self) -> impl Iterator<Item = &HumanAgent> {
        self.by_agent.values()
    }

    /// Iterate only living incarnations.
    pub fn iter_alive(&self) -> impl Iterator<Item = &HumanAgent> {
        self.by_agent.values().filter(|a| a.alive)
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Human Agent Subsystem
// ═══════════════════════════════════════════════════════════════════════════

/// Subsystem that drains the human action queue at the start of each tick
/// and applies the actions to the engine's agent roster.
///
/// Registered **first** in the `SubsystemRegistry` so human actions land
/// before token burn / death judgment evaluate the same tick.
pub struct HumanAgentSubsystem {
    pub queue: SharedHumanActionQueue,
    pub registry: SharedHumanAgentRegistry,
}

impl HumanAgentSubsystem {
    pub fn new(
        queue: SharedHumanActionQueue,
        registry: SharedHumanAgentRegistry,
    ) -> Self {
        Self { queue, registry }
    }
}

impl crate::world::Subsystem for HumanAgentSubsystem {
    fn name(&self) -> &str {
        "human_agent"
    }

    fn on_tick(
        &self,
        tick: u64,
        agents: &mut [(Uuid, u64, AgentRecord)],
    ) -> Vec<WorldEvent> {
        let mut events = Vec::new();

        // 1. Drain queued actions for any agents present in the roster.
        // The scheduler thread drives `on_tick` and is never inside a tokio
        // runtime worker, so `blocking_lock` is safe there. Tests run inside
        // a runtime so we fall back to a spin on `try_lock`.
        let grouped = {
            let mut q = spin_lock(&self.queue);
            let g = q.drain_all();
            q.gc(tick.saturating_sub(1024));
            g
        };

        for (agent_id_str, actions) in grouped {
            // Find the agent in the engine roster.
            let agent_uid = match Uuid::parse_str(&agent_id_str) {
                Ok(u) => u,
                Err(_) => continue,
            };
            let agent_slot = match agents.iter_mut().find(|(uid, _, _)| *uid == agent_uid) {
                Some(slot) => slot,
                None => continue,
            };
            let (_id, _spawn_tick, record) = agent_slot;
            if record.phase == AgentPhase::Dead {
                continue;
            }

            for action in actions {
                // Apply token cost / income.
                let cost = action_token_cost(&action.action);
                let income = action_token_income(&action.action);
                let tokens_before = record.tokens;
                if cost > 0 {
                    record.tokens = record.tokens.saturating_sub(cost);
                }
                if income > 0 {
                    record.tokens = record.tokens.saturating_add(income);
                }

                // Bookkeeping in registry.
                {
                    if let Ok(mut reg) = self.registry.try_lock() {
                        reg.touch_action(&agent_id_str, tick);
                    }
                }

                if cost > 0 || income > 0 {
                    events.push(WorldEvent::BalanceChanged {
                        agent_id: agent_id_str.clone(),
                        agent_name: record.name.clone(),
                        currency: crate::world::enums::Currency::Token,
                        old_balance: tokens_before,
                        new_balance: record.tokens,
                        tick,
                    });
                }
            }
        }

        // 2. Auto-pilot: humans who have not acted in `HUMAN_IDLE_TIMEOUT_TICKS`
        //    automatically rest, so they don't get penalised for inactivity.
        {
            if let Ok(reg) = self.registry.try_lock() {
                let idle_agents: Vec<(String, String)> = reg
                    .iter_alive()
                    .filter(|a| tick.saturating_sub(a.last_action_tick) >= HUMAN_IDLE_TIMEOUT_TICKS)
                    .map(|a| (a.agent_id.clone(), a.name.clone()))
                    .collect();
                drop(reg);

                for (agent_id_str, agent_name) in idle_agents {
                    let agent_uid = match Uuid::parse_str(&agent_id_str) {
                        Ok(u) => u,
                        Err(_) => continue,
                    };
                    let agent_slot = match agents.iter_mut().find(|(uid, _, _)| *uid == agent_uid) {
                        Some(slot) => slot,
                        None => continue,
                    };
                    let (_id, _spawn_tick, record) = agent_slot;
                    if record.phase == AgentPhase::Dead {
                        continue;
                    }

                    let tokens_before = record.tokens;
                    let income = action_token_income("rest");
                    record.tokens = record.tokens.saturating_add(income);

                    events.push(WorldEvent::BalanceChanged {
                        agent_id: agent_id_str.clone(),
                        agent_name,
                        currency: crate::world::enums::Currency::Token,
                        old_balance: tokens_before,
                        new_balance: record.tokens,
                        tick,
                    });
                }
            }
        }

        events
    }
}

/// Acquire a tokio `Mutex` from a synchronous context.
///
/// Production runs on the scheduler thread (never a runtime worker) so
/// `blocking_lock` is safe. Tests run inside a runtime where
/// `blocking_lock` panics, so we fall back to a brief spin on `try_lock`.
fn spin_lock<T>(m: &tokio::sync::Mutex<T>) -> tokio::sync::MutexGuard<'_, T> {
    // Try a non-blocking acquire first — works in tests.
    for _ in 0..100 {
        if let Ok(g) = m.try_lock() {
            return g;
        }
        std::thread::sleep(std::time::Duration::from_micros(100));
    }
    // Fall back to `blocking_lock` — panics inside a runtime, but works
    // on the scheduler thread.
    m.blocking_lock()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::agent::AgentRecord;
    use crate::world::enums::AgentPhase;
    use crate::world::subsystem::Subsystem;
    use uuid::Uuid;

    fn make_record(name: &str, tokens: u64) -> (Uuid, u64, AgentRecord) {
        (
            Uuid::new_v4(),
            0,
            AgentRecord {
                id: Uuid::new_v4(),
                name: name.to_string(),
                phase: AgentPhase::Adult,
                tokens,
                skills: HashMap::new(),
                personality: String::new(),
                tasks_completed: 0,
                tasks_attempted: 0,
            },
        )
    }

    #[test]
    fn queue_enqueue_and_drain_for_agent() {
        let mut q = HumanActionQueue::new();
        let aid = Uuid::new_v4().to_string();

        q.enqueue(QueuedAction {
            id: String::new(),
            agent_id: aid.clone(),
            action: "rest".into(),
            params: serde_json::json!({}),
            enqueued_tick: 1,
            applied: false,
        })
        .unwrap();
        q.enqueue(QueuedAction {
            id: String::new(),
            agent_id: aid.clone(),
            action: "explore".into(),
            params: serde_json::json!({}),
            enqueued_tick: 2,
            applied: false,
        })
        .unwrap();

        assert_eq!(q.pending_count(), 2);

        let drained = q.drain_for_agent(&aid);
        assert_eq!(drained.len(), 2);
        assert_eq!(drained[0].action, "rest");
        assert_eq!(drained[1].action, "explore");

        assert_eq!(q.pending_count(), 0);
        assert!(q.drain_for_agent(&aid).is_empty());
    }

    #[test]
    fn queue_cap_prevents_flood() {
        let mut q = HumanActionQueue::new();
        let aid = Uuid::new_v4().to_string();
        for _ in 0..MAX_QUEUED_ACTIONS_PER_AGENT {
            q.enqueue(QueuedAction {
                id: String::new(),
                agent_id: aid.clone(),
                action: "rest".into(),
                params: serde_json::json!({}),
                enqueued_tick: 0,
                applied: false,
            })
            .unwrap();
        }
        let res = q.enqueue(QueuedAction {
            id: String::new(),
            agent_id: aid.clone(),
            action: "rest".into(),
            params: serde_json::json!({}),
            enqueued_tick: 0,
            applied: false,
        });
        assert!(res.is_err());
    }

    #[test]
    fn registry_blocks_duplicate_living_incarnation() {
        let mut reg = HumanAgentRegistry::new();
        let human = "human-1".to_string();
        let a1 = HumanAgent {
            agent_id: "agent-1".into(),
            human_id: human.clone(),
            name: "Player1".into(),
            initial_tokens: 100,
            initial_money: 50,
            spawned_tick: 0,
            last_action_tick: 0,
            alive: true,
            metadata: serde_json::json!({}),
        };
        reg.register(a1).unwrap();

        let a2 = HumanAgent {
            agent_id: "agent-2".into(),
            human_id: human.clone(),
            name: "Player1-bis".into(),
            initial_tokens: 100,
            initial_money: 50,
            spawned_tick: 5,
            last_action_tick: 5,
            alive: true,
            metadata: serde_json::json!({}),
        };
        assert!(reg.register(a2).is_err());

        // After death, the same human can incarnate again.
        reg.mark_dead("agent-1");
        let res = reg.register(HumanAgent {
            agent_id: "agent-3".into(),
            human_id: human,
            name: "Player1-again".into(),
            initial_tokens: 100,
            initial_money: 50,
            spawned_tick: 10,
            last_action_tick: 10,
            alive: true,
            metadata: serde_json::json!({}),
        });
        assert!(res.is_ok());
    }

    #[tokio::test]
    async fn subsystem_applies_queued_rest_and_emits_balance_event() {
        let queue = HumanActionQueue::shared();
        let registry = HumanAgentRegistry::shared();

        let (uid, _spawn, record) = make_record("Alice", 50);
        let agent_id = uid.to_string();
        let initial_tokens = record.tokens;

        registry.lock().await.register(HumanAgent {
            agent_id: agent_id.clone(),
            human_id: "human-1".into(),
            name: "Alice".into(),
            initial_tokens,
            initial_money: 0,
            spawned_tick: 0,
            last_action_tick: 0,
            alive: true,
            metadata: serde_json::json!({}),
        }).unwrap();

        queue.lock().await.enqueue(QueuedAction {
            id: String::new(),
            agent_id: agent_id.clone(),
            action: "rest".into(),
            params: serde_json::json!({}),
            enqueued_tick: 1,
            applied: false,
        }).unwrap();

        let sub = HumanAgentSubsystem::new(queue.clone(), registry.clone());
        let mut agents = [(uid, 0, record.clone())];
        let events = sub.on_tick(1, &mut agents);

        // rest is free and gives +5 tokens.
        assert_eq!(agents[0].2.tokens, initial_tokens + 5);
        let aid_for_match = agent_id.clone();
        assert!(events.iter().any(|e| matches!(
            e,
            WorldEvent::BalanceChanged { agent_id, .. } if agent_id == &aid_for_match
        )));
    }

    #[tokio::test]
    async fn subsystem_auto_pilots_idle_human_to_rest() {
        let queue = HumanActionQueue::shared();
        let registry = HumanAgentRegistry::shared();

        let (uid, _spawn, record) = make_record("Bob", 30);
        let agent_id = uid.to_string();

        // Spawned at tick 0, never acted. Tick now = HUMAN_IDLE_TIMEOUT_TICKS + 1.
        registry.lock().await.register(HumanAgent {
            agent_id: agent_id.clone(),
            human_id: "human-2".into(),
            name: "Bob".into(),
            initial_tokens: 30,
            initial_money: 0,
            spawned_tick: 0,
            last_action_tick: 0,
            alive: true,
            metadata: serde_json::json!({}),
        }).unwrap();

        let sub = HumanAgentSubsystem::new(queue.clone(), registry.clone());
        let mut agents = [(uid, 0, record)];
        let before = agents[0].2.tokens;
        let events = sub.on_tick(HUMAN_IDLE_TIMEOUT_TICKS + 1, &mut agents);

        assert!(agents[0].2.tokens > before, "auto-rest should grant tokens");
        assert!(events.iter().any(|e| matches!(
            e,
            WorldEvent::BalanceChanged { agent_name, .. } if agent_name == "Bob"
        )));
    }
}
