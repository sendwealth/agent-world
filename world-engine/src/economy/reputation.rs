use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::world::event::WorldEvent;
use crate::world::state::EventBus;

// ── Reputation Config ────────────────────────────────────

/// Configuration for the reputation system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReputationConfig {
    /// Tasks with reward above this value require minimum reputation to claim.
    pub high_value_threshold: u64,
    /// Minimum reputation required to claim high-value tasks.
    pub min_reputation_for_high_value: f64,
    /// Reputation bonus for completing a task on time (before expires_at).
    pub on_time_bonus: f64,
    /// Reputation penalty for task expiry while claimed (breach of contract).
    pub breach_penalty: f64,
    /// Base penalty for any task expiry.
    pub expiry_penalty: f64,
    /// Reputation penalty applied per tick while agent has low reputation.
    pub penalty_decay_per_tick: f64,
    /// Maximum reputation score.
    pub max_reputation: f64,
    /// Minimum reputation score.
    pub min_reputation: f64,
}

impl Default for ReputationConfig {
    fn default() -> Self {
        Self {
            high_value_threshold: 500,
            min_reputation_for_high_value: 10.0,
            on_time_bonus: 1.0,
            breach_penalty: 5.0,
            expiry_penalty: 2.0,
            penalty_decay_per_tick: 0.1,
            max_reputation: 100.0,
            min_reputation: -100.0,
        }
    }
}

// ── Reputation Change Reason ─────────────────────────────

/// Why a reputation change occurred.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReputationChangeReason {
    /// Task completed successfully.
    TaskCompleted,
    /// Task completed before expiry (on-time bonus).
    TaskCompletedOnTime,
    /// Task completed but review was rejected.
    TaskRejected,
    /// Task expired while claimed by agent (breach).
    TaskBreachExpired,
    /// Task expired while published (no penalty to any agent).
    TaskExpiredPublished,
    /// Time-decay recovery: penalty decays over time.
    TimeDecayRecovery,
    /// Manual adjustment by admin/system.
    ManualAdjustment,
}

impl std::fmt::Display for ReputationChangeReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReputationChangeReason::TaskCompleted => write!(f, "task_completed"),
            ReputationChangeReason::TaskCompletedOnTime => write!(f, "task_completed_on_time"),
            ReputationChangeReason::TaskRejected => write!(f, "task_rejected"),
            ReputationChangeReason::TaskBreachExpired => write!(f, "task_breach_expired"),
            ReputationChangeReason::TaskExpiredPublished => write!(f, "task_expired_published"),
            ReputationChangeReason::TimeDecayRecovery => write!(f, "time_decay_recovery"),
            ReputationChangeReason::ManualAdjustment => write!(f, "manual_adjustment"),
        }
    }
}

// ── Reputation Ranking Entry ─────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReputationRankingEntry {
    pub agent_id: String,
    pub reputation: f64,
    pub rank: usize,
}

// ── Reputation System ────────────────────────────────────

/// Manages agent reputation scores and their integration with the task marketplace.
///
/// Responsibilities:
/// 1. Track per-agent reputation scores
/// 2. Enforce reputation thresholds for high-value tasks
/// 3. Apply bonuses/penalties based on task outcomes
/// 4. Time-decay recovery for penalized agents
/// 5. Broadcast reputation change events
/// 6. Provide reputation rankings for dashboard visualization
pub struct ReputationSystem {
    config: ReputationConfig,
    /// Agent reputation scores.
    scores: HashMap<String, f64>,
    /// Tick at which each agent last had a reputation change.
    last_change_tick: HashMap<String, u64>,
    event_bus: Option<EventBus>,
}

impl ReputationSystem {
    pub fn new(config: ReputationConfig) -> Self {
        Self {
            config,
            scores: HashMap::new(),
            last_change_tick: HashMap::new(),
            event_bus: None,
        }
    }

    pub fn with_event_bus(config: ReputationConfig, event_bus: EventBus) -> Self {
        Self {
            config,
            scores: HashMap::new(),
            last_change_tick: HashMap::new(),
            event_bus: Some(event_bus),
        }
    }

    // ── Score Access ────────────────────────────────────────

    /// Get an agent's reputation score. Defaults to 0.0 for unknown agents.
    pub fn get_reputation(&self, agent_id: &str) -> f64 {
        self.scores.get(agent_id).copied().unwrap_or(0.0)
    }

    /// Set an agent's reputation score directly (clamped to valid range).
    pub fn set_reputation(&mut self, agent_id: &str, score: f64, tick: u64) {
        let clamped = score.clamp(self.config.min_reputation, self.config.max_reputation);
        let old = self.get_reputation(agent_id);
        self.scores.insert(agent_id.to_string(), clamped);
        self.last_change_tick.insert(agent_id.to_string(), tick);

        if (old - clamped).abs() > f64::EPSILON {
            self.emit_change(agent_id, old, clamped, ReputationChangeReason::ManualAdjustment);
        }
    }

    // ── Threshold Check ─────────────────────────────────────

    /// Check whether an agent can claim a task with the given reward.
    ///
    /// Returns `Ok(())` if the agent is allowed, `Err` with a description if not.
    pub fn check_claim_eligibility(&self, agent_id: &str, reward: u64) -> Result<(), String> {
        if reward >= self.config.high_value_threshold {
            let rep = self.get_reputation(agent_id);
            if rep < self.config.min_reputation_for_high_value {
                return Err(format!(
                    "reputation too low for high-value task: {:.1} < {:.1} (required for tasks >= {} reward)",
                    rep,
                    self.config.min_reputation_for_high_value,
                    self.config.high_value_threshold
                ));
            }
        }
        Ok(())
    }

    /// Check if a task is considered "high-value" based on the configured threshold.
    pub fn is_high_value_task(&self, reward: u64) -> bool {
        reward >= self.config.high_value_threshold
    }

    // ── Quality Impact ──────────────────────────────────────

    /// Apply a reputation bonus when a task is completed on time.
    ///
    /// Returns the actual reputation change applied.
    pub fn on_task_completed_on_time(&mut self, agent_id: &str, tick: u64) -> f64 {
        let bonus = self.config.on_time_bonus;
        self.apply_delta(agent_id, bonus, tick, ReputationChangeReason::TaskCompletedOnTime)
    }

    /// Apply a reputation penalty when a claimed task expires (breach of contract).
    ///
    /// Returns the actual reputation change applied (negative).
    pub fn on_task_breach(&mut self, agent_id: &str, tick: u64) -> f64 {
        let penalty = -self.config.breach_penalty;
        self.apply_delta(agent_id, penalty, tick, ReputationChangeReason::TaskBreachExpired)
    }

    /// Apply a small reputation penalty when a published task expires (no assignee).
    pub fn on_task_expired_published(&mut self, publisher_id: &str, tick: u64) -> f64 {
        let penalty = -self.config.expiry_penalty;
        self.apply_delta(publisher_id, penalty, tick, ReputationChangeReason::TaskExpiredPublished)
    }

    /// Apply a reputation change with the standard task completion bonus.
    pub fn on_task_completed(&mut self, agent_id: &str, tick: u64) -> f64 {
        self.apply_delta(agent_id, 0.0, tick, ReputationChangeReason::TaskCompleted)
    }

    // ── Time Decay Recovery ─────────────────────────────────

    /// Process time-decay recovery for all agents.
    ///
    /// Agents with negative reputation recover slightly each tick.
    /// Recovery is capped at 0 (does not make reputation positive).
    /// Returns the list of (agent_id, reputation_change) pairs that were applied.
    pub fn process_time_decay(&mut self, current_tick: u64) -> Vec<(String, f64)> {
        let mut changes = Vec::new();
        let agent_ids: Vec<String> = self.scores.keys().cloned().collect();

        for agent_id in agent_ids {
            let rep = self.get_reputation(&agent_id);
            if rep < 0.0 {
                // Apply decay: penalty decreases over time (reputation recovers toward 0)
                let recovery = self.config.penalty_decay_per_tick;
                let new_rep = (rep + recovery).min(0.0); // Cap at 0
                let actual_delta = new_rep - rep;

                if actual_delta.abs() > f64::EPSILON {
                    self.scores.insert(agent_id.clone(), new_rep);
                    self.last_change_tick.insert(agent_id.clone(), current_tick);
                    self.emit_change(&agent_id, rep, new_rep, ReputationChangeReason::TimeDecayRecovery);
                    changes.push((agent_id, actual_delta));
                }
            }
        }

        changes
    }

    // ── Rankings ────────────────────────────────────────────

    /// Get agents ranked by reputation (descending).
    pub fn get_rankings(&self, limit: usize) -> Vec<ReputationRankingEntry> {
        let mut entries: Vec<_> = self
            .scores
            .iter()
            .map(|(id, &rep)| (id.clone(), rep))
            .collect();
        entries.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        entries
            .into_iter()
            .take(limit)
            .enumerate()
            .map(|(rank, (agent_id, reputation))| ReputationRankingEntry {
                agent_id,
                reputation,
                rank: rank + 1,
            })
            .collect()
    }

    /// Get agents with reputation below a threshold.
    pub fn get_low_reputation_agents(&self, threshold: f64) -> Vec<(String, f64)> {
        self.scores
            .iter()
            .filter(|(_, &rep)| rep < threshold)
            .map(|(id, &rep)| (id.clone(), rep))
            .collect()
    }

    // ── Helpers ─────────────────────────────────────────────

    fn apply_delta(
        &mut self,
        agent_id: &str,
        delta: f64,
        tick: u64,
        reason: ReputationChangeReason,
    ) -> f64 {
        let old = self.get_reputation(agent_id);
        let new = (old + delta).clamp(self.config.min_reputation, self.config.max_reputation);
        let actual_delta = new - old;

        if actual_delta.abs() > f64::EPSILON {
            self.scores.insert(agent_id.to_string(), new);
            self.last_change_tick.insert(agent_id.to_string(), tick);
            self.emit_change(agent_id, old, new, reason);
        }

        actual_delta
    }

    fn emit_change(&self, agent_id: &str, old: f64, new: f64, reason: ReputationChangeReason) {
        if let Some(ref bus) = self.event_bus {
            bus.emit(WorldEvent::ReputationChanged {
                agent_id: agent_id.to_string(),
                old_reputation: old,
                new_reputation: new,
                reason: reason.to_string(),
            });
        }
    }

    // ── Accessors ───────────────────────────────────────────

    pub fn config(&self) -> &ReputationConfig {
        &self.config
    }

    pub fn scores(&self) -> &HashMap<String, f64> {
        &self.scores
    }
}

// ── Tests ─────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_system() -> ReputationSystem {
        ReputationSystem::new(ReputationConfig::default())
    }

    // ── Config ──────────────────────────────────────────────

    #[test]
    fn test_config_defaults() {
        let config = ReputationConfig::default();
        assert_eq!(config.high_value_threshold, 500);
        assert_eq!(config.min_reputation_for_high_value, 10.0);
        assert_eq!(config.on_time_bonus, 1.0);
        assert_eq!(config.breach_penalty, 5.0);
        assert_eq!(config.expiry_penalty, 2.0);
        assert_eq!(config.penalty_decay_per_tick, 0.1);
        assert_eq!(config.max_reputation, 100.0);
        assert_eq!(config.min_reputation, -100.0);
    }

    // ── Basic Score ─────────────────────────────────────────

    #[test]
    fn test_default_reputation_is_zero() {
        let sys = make_system();
        assert_eq!(sys.get_reputation("unknown"), 0.0);
    }

    #[test]
    fn test_set_reputation() {
        let mut sys = make_system();
        sys.set_reputation("agent-1", 25.0, 1);
        assert_eq!(sys.get_reputation("agent-1"), 25.0);
    }

    #[test]
    fn test_set_reputation_clamps_max() {
        let mut sys = make_system();
        sys.set_reputation("agent-1", 200.0, 1);
        assert_eq!(sys.get_reputation("agent-1"), 100.0);
    }

    #[test]
    fn test_set_reputation_clamps_min() {
        let mut sys = make_system();
        sys.set_reputation("agent-1", -200.0, 1);
        assert_eq!(sys.get_reputation("agent-1"), -100.0);
    }

    // ── Threshold Check ─────────────────────────────────────

    #[test]
    fn test_claim_eligibility_low_reward_always_allowed() {
        let sys = make_system();
        assert!(sys.check_claim_eligibility("newbie", 100).is_ok());
        assert!(sys.check_claim_eligibility("newbie", 499).is_ok());
    }

    #[test]
    fn test_claim_eligibility_high_value_requires_reputation() {
        let mut sys = make_system();
        // New agent has 0 reputation, below 10.0 threshold
        assert!(sys.check_claim_eligibility("newbie", 500).is_err());
        assert!(sys.check_claim_eligibility("newbie", 1000).is_err());

        // Give enough reputation
        sys.set_reputation("agent-1", 10.0, 1);
        assert!(sys.check_claim_eligibility("agent-1", 500).is_ok());
        assert!(sys.check_claim_eligibility("agent-1", 10000).is_ok());
    }

    #[test]
    fn test_claim_eligibility_exact_threshold() {
        let mut sys = make_system();
        sys.set_reputation("agent-1", 10.0, 1);
        assert!(sys.check_claim_eligibility("agent-1", 500).is_ok());

        sys.set_reputation("agent-2", 9.99, 1);
        assert!(sys.check_claim_eligibility("agent-2", 500).is_err());
    }

    #[test]
    fn test_is_high_value_task() {
        let sys = make_system();
        assert!(!sys.is_high_value_task(499));
        assert!(sys.is_high_value_task(500));
        assert!(sys.is_high_value_task(1000));
    }

    // ── Quality Impact ──────────────────────────────────────

    #[test]
    fn test_on_task_completed_on_time() {
        let mut sys = make_system();
        let change = sys.on_task_completed_on_time("worker", 10);
        assert_eq!(change, 1.0);
        assert_eq!(sys.get_reputation("worker"), 1.0);
    }

    #[test]
    fn test_on_task_breach() {
        let mut sys = make_system();
        sys.set_reputation("worker", 20.0, 1);
        let change = sys.on_task_breach("worker", 10);
        assert_eq!(change, -5.0);
        assert_eq!(sys.get_reputation("worker"), 15.0);
    }

    #[test]
    fn test_on_task_breach_clamps_at_min() {
        let mut sys = make_system();
        sys.set_reputation("worker", -98.0, 1);
        let change = sys.on_task_breach("worker", 10);
        // -98 - 5 = -103, clamped to -100
        assert_eq!(change, -2.0);
        assert_eq!(sys.get_reputation("worker"), -100.0);
    }

    #[test]
    fn test_on_task_expired_published() {
        let mut sys = make_system();
        sys.set_reputation("publisher", 10.0, 1);
        let change = sys.on_task_expired_published("publisher", 10);
        assert_eq!(change, -2.0);
        assert_eq!(sys.get_reputation("publisher"), 8.0);
    }

    #[test]
    fn test_multiple_events_accumulate() {
        let mut sys = make_system();
        sys.on_task_completed_on_time("worker", 1); // +1
        sys.on_task_completed_on_time("worker", 2); // +1
        sys.on_task_completed_on_time("worker", 3); // +1
        assert_eq!(sys.get_reputation("worker"), 3.0);

        sys.on_task_breach("worker", 4); // -5
        assert_eq!(sys.get_reputation("worker"), -2.0);

        sys.on_task_completed_on_time("worker", 5); // +1
        assert_eq!(sys.get_reputation("worker"), -1.0);
    }

    // ── Time Decay Recovery ─────────────────────────────────

    #[test]
    fn test_time_decay_recovers_negative_reputation() {
        let mut sys = make_system();
        sys.set_reputation("worker", -10.0, 1);

        let changes = sys.process_time_decay(2);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].0, "worker");
        assert!((changes[0].1 - 0.1).abs() < 0.001); // +0.1 recovery
        assert!((sys.get_reputation("worker") - (-9.9)).abs() < 0.001);
    }

    #[test]
    fn test_time_decay_does_not_affect_positive_reputation() {
        let mut sys = make_system();
        sys.set_reputation("worker", 10.0, 1);

        let changes = sys.process_time_decay(2);
        assert!(changes.is_empty());
        assert_eq!(sys.get_reputation("worker"), 10.0);
    }

    #[test]
    fn test_time_decay_does_not_affect_zero_reputation() {
        let mut sys = make_system();
        let changes = sys.process_time_decay(2);
        assert!(changes.is_empty());
    }

    #[test]
    fn test_time_decay_recovers_to_zero_not_above() {
        let mut sys = make_system();
        sys.set_reputation("worker", -0.05, 1);

        let changes = sys.process_time_decay(2);
        // -0.05 + 0.1 = 0.05, but we only recover up to 0
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].1, 0.05); // actual delta applied
        assert_eq!(sys.get_reputation("worker"), 0.0);
    }

    #[test]
    fn test_time_decay_multiple_ticks() {
        let mut sys = make_system();
        sys.set_reputation("worker", -5.0, 1);

        for tick in 2..=51 {
            sys.process_time_decay(tick);
        }
        // 50 ticks * 0.1 = 5.0 recovery, exactly reaching 0
        assert!((sys.get_reputation("worker") - 0.0).abs() < 0.01);
    }

    // ── Rankings ────────────────────────────────────────────

    #[test]
    fn test_rankings_sorted_descending() {
        let mut sys = make_system();
        sys.set_reputation("a", 50.0, 1);
        sys.set_reputation("b", 30.0, 1);
        sys.set_reputation("c", 80.0, 1);
        sys.set_reputation("d", 10.0, 1);

        let rankings = sys.get_rankings(10);
        assert_eq!(rankings.len(), 4);
        assert_eq!(rankings[0].agent_id, "c");
        assert_eq!(rankings[0].rank, 1);
        assert_eq!(rankings[0].reputation, 80.0);

        assert_eq!(rankings[1].agent_id, "a");
        assert_eq!(rankings[1].rank, 2);

        assert_eq!(rankings[2].agent_id, "b");
        assert_eq!(rankings[2].rank, 3);

        assert_eq!(rankings[3].agent_id, "d");
        assert_eq!(rankings[3].rank, 4);
    }

    #[test]
    fn test_rankings_with_limit() {
        let mut sys = make_system();
        sys.set_reputation("a", 50.0, 1);
        sys.set_reputation("b", 30.0, 1);
        sys.set_reputation("c", 80.0, 1);

        let rankings = sys.get_rankings(2);
        assert_eq!(rankings.len(), 2);
        assert_eq!(rankings[0].agent_id, "c");
        assert_eq!(rankings[1].agent_id, "a");
    }

    #[test]
    fn test_rankings_empty() {
        let sys = make_system();
        let rankings = sys.get_rankings(10);
        assert!(rankings.is_empty());
    }

    // ── Low Reputation Agents ───────────────────────────────

    #[test]
    fn test_get_low_reputation_agents() {
        let mut sys = make_system();
        sys.set_reputation("a", 50.0, 1);
        sys.set_reputation("b", -5.0, 1);
        sys.set_reputation("c", 10.0, 1);
        sys.set_reputation("d", -20.0, 1);

        let low = sys.get_low_reputation_agents(0.0);
        assert_eq!(low.len(), 2);
    }

    // ── Event Broadcasting ──────────────────────────────────

    #[test]
    fn test_event_broadcast_on_reputation_change() {
        let bus = EventBus::new(64);
        let mut rx = bus.subscribe();
        let mut sys = ReputationSystem::with_event_bus(ReputationConfig::default(), bus);

        sys.on_task_completed_on_time("worker", 1);

        let event = rx.try_recv().unwrap();
        match event {
            WorldEvent::ReputationChanged { agent_id, old_reputation, new_reputation, reason } => {
                assert_eq!(agent_id, "worker");
                assert_eq!(old_reputation, 0.0);
                assert_eq!(new_reputation, 1.0);
                assert_eq!(reason, "task_completed_on_time");
            }
            _ => panic!("Expected ReputationChanged event"),
        }
    }

    #[test]
    fn test_no_event_when_no_change() {
        let bus = EventBus::new(64);
        let mut rx = bus.subscribe();
        let mut sys = ReputationSystem::with_event_bus(ReputationConfig::default(), bus);

        // Set to max, then try to add more
        sys.set_reputation("agent-1", 100.0, 1);
        let _ = rx.try_recv(); // consume the set event

        sys.on_task_completed_on_time("agent-1", 2);
        // Already at max, no change should happen
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn test_breach_and_recovery_workflow() {
        let mut sys = make_system();

        // Agent starts at 0, completes tasks to build reputation
        for _ in 0..5 {
            sys.on_task_completed_on_time("worker", 1);
        }
        assert_eq!(sys.get_reputation("worker"), 5.0);

        // Can now claim tasks up to reward 499 but not 500+
        assert!(sys.check_claim_eligibility("worker", 400).is_ok());
        assert!(sys.check_claim_eligibility("worker", 500).is_err());

        // Complete more tasks to reach high-value threshold
        for _ in 0..5 {
            sys.on_task_completed_on_time("worker", 2);
        }
        assert_eq!(sys.get_reputation("worker"), 10.0);
        assert!(sys.check_claim_eligibility("worker", 500).is_ok());

        // Breach a task
        sys.on_task_breach("worker", 3);
        assert_eq!(sys.get_reputation("worker"), 5.0);
        assert!(sys.check_claim_eligibility("worker", 500).is_err());

        // Time decay doesn't help when negative is needed (worker is at +5)
        sys.process_time_decay(4);
        assert_eq!(sys.get_reputation("worker"), 5.0);

        // Multiple breaches push into negative
        sys.on_task_breach("worker", 5); // 5 - 5 = 0
        sys.on_task_breach("worker", 6); // 0 - 5 = -5
        assert_eq!(sys.get_reputation("worker"), -5.0);

        // Time decay recovers
        sys.process_time_decay(7);
        assert_eq!(sys.get_reputation("worker"), -4.9);
    }
}
