//! Human Action Queue — buffers actions submitted by human players between ticks.
//!
//! The world engine tick loop calls [`HumanActionQueue::drain`] at the start
//! of each tick to process pending human actions before AI decisions.
//! If a human agent has not submitted an action within `timeout_ticks` ticks,
//! [`HumanActionQueue::check_timeouts`] returns the agent IDs that need
//! fallback to AI decision (Rest / Survival Instinct).

use std::collections::{HashMap, VecDeque};

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

/// Action types that a human player can submit.
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
}

impl HumanActionType {
    /// Token cost for each action type (server-authoritative).
    /// Aligned with `api_agents_ext.rs` action costs.
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
        }
    }

    /// Token income for each action type.
    pub fn token_income(&self) -> u64 {
        match self {
            HumanActionType::Rest => 5,
            HumanActionType::Explore => 2,
            HumanActionType::Gather => 3,
            HumanActionType::Build => 5,
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
            _ => None,
        }
    }
}

/// A queued action submitted by a human player.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HumanAction {
    /// The human-controlled agent ID.
    pub agent_id: String,
    /// The action type.
    pub action_type: HumanActionType,
    /// Free-form parameters (e.g. trade target, message content, move direction).
    #[serde(default)]
    pub params: serde_json::Value,
    /// Tick when the action was submitted.
    pub submitted_tick: u64,
}

/// Tracks per-agent tick activity for timeout detection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HumanAgentState {
    /// The agent ID (same as AgentDto.id).
    pub agent_id: String,
    /// The authenticated human user ID who incarnated this agent.
    pub human_user_id: String,
    /// Display name.
    pub name: String,
    /// Tick when the agent was incarnated.
    pub incarnated_tick: u64,
    /// Last tick an action was submitted or executed.
    pub last_action_tick: u64,
    /// Whether newbie protection is active.
    pub newbie_protection: bool,
    /// Token balance at incarnate time.
    pub initial_tokens: u64,
}

/// Thread-safe human action queue shared across the application.
///
/// Uses `tokio::sync::Mutex` so it can be locked from async handlers without
/// blocking the runtime. All operations are O(n) in the number of queued
/// actions for a given agent, which is bounded by the tick rate.
pub struct HumanActionQueue {
    /// FIFO action queue.
    queue: Mutex<VecDeque<HumanAction>>,
    /// Per-agent metadata and timeout tracking.
    agents: Mutex<HashMap<String, HumanAgentState>>,
}

impl HumanActionQueue {
    pub fn new() -> Self {
        Self {
            queue: Mutex::new(VecDeque::new()),
            agents: Mutex::new(HashMap::new()),
        }
    }

    /// Register a new human agent.
    pub async fn register_agent(
        &self,
        agent_id: &str,
        human_user_id: &str,
        name: &str,
        incarnated_tick: u64,
        initial_tokens: u64,
        newbie_protection_ticks: u64,
    ) -> HumanAgentState {
        let state = HumanAgentState {
            agent_id: agent_id.to_string(),
            human_user_id: human_user_id.to_string(),
            name: name.to_string(),
            incarnated_tick,
            last_action_tick: incarnated_tick,
            newbie_protection: newbie_protection_ticks > 0,
            initial_tokens,
        };
        let mut agents = self.agents.lock().await;
        agents.insert(agent_id.to_string(), state.clone());
        state
    }

    /// Check if an agent is registered as a human-controlled agent.
    pub async fn is_human_agent(&self, agent_id: &str) -> bool {
        let agents = self.agents.lock().await;
        agents.contains_key(agent_id)
    }

    /// Get the human agent state.
    pub async fn get_agent_state(&self, agent_id: &str) -> Option<HumanAgentState> {
        let agents = self.agents.lock().await;
        agents.get(agent_id).cloned()
    }

    /// Enqueue an action for a human agent.
    pub async fn enqueue(&self, action: HumanAction) {
        let mut queue = self.queue.lock().await;
        queue.push_back(action);
    }

    /// Drain all queued actions at the start of a tick.
    ///
    /// Returns actions in FIFO order. Also updates `last_action_tick` for
    /// each agent that had a pending action.
    pub async fn drain(&self, current_tick: u64) -> Vec<HumanAction> {
        let mut queue = self.queue.lock().await;
        let actions: Vec<HumanAction> = queue.drain(..).collect();

        // Update last_action_tick for agents that submitted actions
        if !actions.is_empty() {
            let mut agents = self.agents.lock().await;
            for action in &actions {
                if let Some(state) = agents.get_mut(&action.agent_id) {
                    state.last_action_tick = current_tick;
                    // Disable newbie protection once the first action is executed
                    state.newbie_protection = false;
                }
            }
        }

        actions
    }

    /// Check for human agents that have timed out (no action for `timeout_ticks`).
    ///
    /// Returns the agent IDs that need AI fallback.
    pub async fn check_timeouts(&self, current_tick: u64, timeout_ticks: u64) -> Vec<String> {
        let agents = self.agents.lock().await;
        agents
            .iter()
            .filter(|(_, state)| {
                current_tick.saturating_sub(state.last_action_tick) >= timeout_ticks
            })
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Update the last_action_tick for an agent (e.g. when AI fallback is applied).
    pub async fn touch_agent(&self, agent_id: &str, tick: u64) {
        let mut agents = self.agents.lock().await;
        if let Some(state) = agents.get_mut(agent_id) {
            state.last_action_tick = tick;
        }
    }

    /// Remove a human agent (e.g. on death).
    pub async fn remove_agent(&self, agent_id: &str) {
        let mut agents = self.agents.lock().await;
        agents.remove(agent_id);
    }

    /// List all registered human agents.
    pub async fn list_agents(&self) -> Vec<HumanAgentState> {
        let agents = self.agents.lock().await;
        agents.values().cloned().collect()
    }

    /// Get the number of pending actions.
    pub async fn pending_count(&self) -> usize {
        let queue = self.queue.lock().await;
        queue.len()
    }
}

impl Default for HumanActionQueue {
    fn default() -> Self {
        Self::new()
    }
}

/// Type alias for shared references.
pub type SharedHumanActionQueue = std::sync::Arc<HumanActionQueue>;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_enqueue_and_drain() {
        let queue = HumanActionQueue::new();
        queue
            .register_agent("a1", "user1", "Alice", 0, 1000, 5)
            .await;

        queue
            .enqueue(HumanAction {
                agent_id: "a1".into(),
                action_type: HumanActionType::Rest,
                params: serde_json::json!({}),
                submitted_tick: 1,
            })
            .await;
        queue
            .enqueue(HumanAction {
                agent_id: "a1".into(),
                action_type: HumanActionType::Explore,
                params: serde_json::json!({"direction": "north"}),
                submitted_tick: 1,
            })
            .await;

        assert_eq!(queue.pending_count().await, 2);

        let drained = queue.drain(1).await;
        assert_eq!(drained.len(), 2);
        assert_eq!(drained[0].action_type, HumanActionType::Rest);
        assert_eq!(drained[1].action_type, HumanActionType::Explore);

        // Queue should be empty after drain
        assert_eq!(queue.pending_count().await, 0);

        // last_action_tick should be updated
        let state = queue.get_agent_state("a1").await.unwrap();
        assert_eq!(state.last_action_tick, 1);
        // Newbie protection should be disabled after first action execution
        assert!(!state.newbie_protection);
    }

    #[tokio::test]
    async fn test_timeout_detection() {
        let queue = HumanActionQueue::new();
        queue
            .register_agent("a1", "user1", "Alice", 0, 1000, 0)
            .await;
        queue
            .register_agent("a2", "user2", "Bob", 0, 1000, 0)
            .await;

        // a1 submits action at tick 1, a2 never submits
        queue
            .enqueue(HumanAction {
                agent_id: "a1".into(),
                action_type: HumanActionType::Rest,
                params: serde_json::json!({}),
                submitted_tick: 1,
            })
            .await;
        queue.drain(1).await; // a1's last_action_tick = 1

        // At tick 4 (3 ticks since a1's last action, 4 since a2's incarnate)
        let timed_out = queue.check_timeouts(4, 3).await;
        assert!(timed_out.contains(&"a2".to_string())); // a2 timed out
        // a1 has 4 - 1 = 3 ticks since last action, which >= 3
        assert!(timed_out.contains(&"a1".to_string()));

        // At tick 3, a1 has 3-1=2 ticks, a2 has 3-0=3 ticks
        let timed_out = queue.check_timeouts(3, 3).await;
        assert!(timed_out.contains(&"a2".to_string()));
        assert!(!timed_out.contains(&"a1".to_string()));
    }

    #[tokio::test]
    async fn test_newbie_protection_disabled_on_first_action() {
        let queue = HumanActionQueue::new();
        queue
            .register_agent("a1", "user1", "Alice", 0, 1000, 10)
            .await;

        let state = queue.get_agent_state("a1").await.unwrap();
        assert!(state.newbie_protection);

        // Submit and drain an action
        queue
            .enqueue(HumanAction {
                agent_id: "a1".into(),
                action_type: HumanActionType::Rest,
                params: serde_json::json!({}),
                submitted_tick: 1,
            })
            .await;
        queue.drain(1).await;

        let state = queue.get_agent_state("a1").await.unwrap();
        assert!(!state.newbie_protection);
    }

    #[tokio::test]
    async fn test_action_token_cost() {
        assert_eq!(HumanActionType::Rest.token_cost(), 0);
        assert_eq!(HumanActionType::Rest.token_income(), 5);
        assert_eq!(HumanActionType::Communicate.token_cost(), 10);
        assert_eq!(HumanActionType::Explore.token_cost(), 3);
        assert_eq!(HumanActionType::Build.token_cost(), 20);
    }

    #[tokio::test]
    async fn test_is_human_agent() {
        let queue = HumanActionQueue::new();
        queue
            .register_agent("a1", "user1", "Alice", 0, 1000, 0)
            .await;

        assert!(queue.is_human_agent("a1").await);
        assert!(!queue.is_human_agent("a2").await);
    }

    #[tokio::test]
    async fn test_remove_agent() {
        let queue = HumanActionQueue::new();
        queue
            .register_agent("a1", "user1", "Alice", 0, 1000, 0)
            .await;
        assert!(queue.is_human_agent("a1").await);

        queue.remove_agent("a1").await;
        assert!(!queue.is_human_agent("a1").await);
    }
}
