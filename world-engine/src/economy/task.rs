use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::world::enums::Currency;
use crate::world::event::WorldEvent;
use crate::world::state::EventBus;

use super::reward::{RewardConfig, RewardDistributor, RewardDistribution};
use super::reputation::{ReputationConfig, ReputationSystem};

// ── Task Status ───────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    /// Task published, waiting for a claimant.
    Published,
    /// A worker has claimed the task.
    Claimed,
    /// Worker has started working on the task.
    InProgress,
    /// Worker has submitted their result.
    Submitted,
    /// Publisher has reviewed and approved the result.
    Reviewed,
    /// Task fully completed; escrow released.
    Completed,
    /// Task expired; escrow refunded.
    Expired,
}

impl TaskStatus {
    /// Check whether a transition from self to `next` is valid.
    pub fn can_transition_to(&self, next: &TaskStatus) -> bool {
        matches!(
            (self, next),
            (TaskStatus::Published, TaskStatus::Claimed)
                | (TaskStatus::Published, TaskStatus::Expired)
                | (TaskStatus::Claimed, TaskStatus::InProgress)
                | (TaskStatus::Claimed, TaskStatus::Expired)
                | (TaskStatus::InProgress, TaskStatus::Submitted)
                | (TaskStatus::Submitted, TaskStatus::Reviewed)
                | (TaskStatus::Reviewed, TaskStatus::Completed)
        )
    }

    /// Returns all valid next statuses from the current status.
    pub fn valid_transitions(&self) -> Vec<TaskStatus> {
        match self {
            TaskStatus::Published => vec![TaskStatus::Claimed, TaskStatus::Expired],
            TaskStatus::Claimed => vec![TaskStatus::InProgress, TaskStatus::Expired],
            TaskStatus::InProgress => vec![TaskStatus::Submitted],
            TaskStatus::Submitted => vec![TaskStatus::Reviewed],
            TaskStatus::Reviewed => vec![TaskStatus::Completed],
            TaskStatus::Completed | TaskStatus::Expired => vec![],
        }
    }

    /// All task statuses.
    pub fn all() -> Vec<TaskStatus> {
        vec![
            TaskStatus::Published,
            TaskStatus::Claimed,
            TaskStatus::InProgress,
            TaskStatus::Submitted,
            TaskStatus::Reviewed,
            TaskStatus::Completed,
            TaskStatus::Expired,
        ]
    }
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskStatus::Published => write!(f, "published"),
            TaskStatus::Claimed => write!(f, "claimed"),
            TaskStatus::InProgress => write!(f, "in_progress"),
            TaskStatus::Submitted => write!(f, "submitted"),
            TaskStatus::Reviewed => write!(f, "reviewed"),
            TaskStatus::Completed => write!(f, "completed"),
            TaskStatus::Expired => write!(f, "expired"),
        }
    }
}

// ── Task Record ───────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: Uuid,
    pub title: String,
    pub description: String,
    pub status: TaskStatus,
    pub reward: u64,
    /// Currency of the task reward. Defaults to Money if not specified.
    pub currency: Currency,
    pub escrow_held: bool,
    pub publisher_id: String,
    pub assignee_id: Option<String>,
    pub result: Option<String>,
    pub expires_at: Option<u64>,
    pub created_tick: u64,
}

// ── Errors ────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum TaskError {
    NotFound(String),
    InvalidTransition { from: TaskStatus, to: TaskStatus },
    AlreadyClaimed,
    NoAssignee,
    ResultRequired,
    NotPublisher { expected: String, actual: String },
    InsufficientReputation { agent_id: String, reputation: f64, required: f64, reward: u64 },
    Expired,
}

impl std::fmt::Display for TaskError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskError::NotFound(id) => write!(f, "task not found: {}", id),
            TaskError::InvalidTransition { from, to } => {
                write!(f, "invalid transition: {} -> {}", from, to)
            }
            TaskError::AlreadyClaimed => write!(f, "task already claimed"),
            TaskError::NoAssignee => write!(f, "task has no assignee"),
            TaskError::ResultRequired => write!(f, "result is required"),
            TaskError::NotPublisher { expected, actual } => {
                write!(
                    f,
                    "only the publisher can review: expected {}, got {}",
                    expected, actual
                )
            }
            TaskError::InsufficientReputation { agent_id, reputation, required, reward } => {
                write!(
                    f,
                    "agent {} reputation {:.1} below required {:.1} for high-value task (reward: {})",
                    agent_id, reputation, required, reward
                )
            }
            TaskError::Expired => write!(f, "task has expired"),
        }
    }
}

impl std::error::Error for TaskError {}

// ── Task Board ────────────────────────────────────────────

pub struct TaskBoard {
    tasks: HashMap<Uuid, Task>,
    /// Agent balances for escrow simulation.
    balances: HashMap<String, u64>,
    /// Escrow amounts locked per task.
    escrows: HashMap<Uuid, u64>,
    event_bus: Option<EventBus>,
    /// Optional reward distributor for fee deduction, XP, reputation, and ledger.
    reward_distributor: Option<RewardDistributor>,
    /// Optional reputation system for marketplace-reputation integration.
    reputation_system: Option<ReputationSystem>,
}

impl TaskBoard {
    pub fn new() -> Self {
        Self {
            tasks: HashMap::new(),
            balances: HashMap::new(),
            escrows: HashMap::new(),
            event_bus: None,
            reward_distributor: None,
            reputation_system: None,
        }
    }

    pub fn with_event_bus(event_bus: EventBus) -> Self {
        Self {
            tasks: HashMap::new(),
            balances: HashMap::new(),
            escrows: HashMap::new(),
            event_bus: Some(event_bus),
            reward_distributor: None,
            reputation_system: None,
        }
    }

    /// Create a TaskBoard with reward distribution enabled.
    pub fn with_reward_distributor(config: RewardConfig) -> Self {
        Self {
            tasks: HashMap::new(),
            balances: HashMap::new(),
            escrows: HashMap::new(),
            event_bus: None,
            reward_distributor: Some(RewardDistributor::new(config)),
            reputation_system: None,
        }
    }

    /// Create a TaskBoard with both reward distribution and reputation system.
    pub fn with_reputation_system(reward_config: RewardConfig, rep_config: ReputationConfig) -> Self {
        Self {
            tasks: HashMap::new(),
            balances: HashMap::new(),
            escrows: HashMap::new(),
            event_bus: None,
            reward_distributor: Some(RewardDistributor::new(reward_config)),
            reputation_system: Some(ReputationSystem::new(rep_config)),
        }
    }

    /// Create a TaskBoard with event bus, reward distribution, and reputation system.
    pub fn with_all(event_bus: EventBus, reward_config: RewardConfig, rep_config: ReputationConfig) -> Self {
        Self {
            tasks: HashMap::new(),
            balances: HashMap::new(),
            escrows: HashMap::new(),
            event_bus: Some(event_bus),
            reward_distributor: Some(RewardDistributor::new(reward_config)),
            reputation_system: Some(ReputationSystem::new(rep_config)),
        }
    }

    /// Get a reference to the reward distributor (if configured).
    pub fn reward_distributor(&self) -> Option<&RewardDistributor> {
        self.reward_distributor.as_ref()
    }

    /// Get a mutable reference to the reward distributor (if configured).
    pub fn reward_distributor_mut(&mut self) -> Option<&mut RewardDistributor> {
        self.reward_distributor.as_mut()
    }

    /// Get a reference to the reputation system (if configured).
    pub fn reputation_system(&self) -> Option<&ReputationSystem> {
        self.reputation_system.as_ref()
    }

    /// Get a mutable reference to the reputation system (if configured).
    pub fn reputation_system_mut(&mut self) -> Option<&mut ReputationSystem> {
        self.reputation_system.as_mut()
    }

    // ── Balance helpers ────────────────────────────────────

    pub fn set_balance(&mut self, agent: &str, amount: u64) {
        self.balances.insert(agent.to_string(), amount);
    }

    pub fn get_balance(&self, agent: &str) -> u64 {
        self.balances.get(agent).copied().unwrap_or(0)
    }

    // ── Query ─────────────────────────────────────────────

    pub fn get(&self, id: Uuid) -> Option<&Task> {
        self.tasks.get(&id)
    }

    pub fn list(&self) -> Vec<&Task> {
        self.tasks.values().collect()
    }

    pub fn list_by_status(&self, status: TaskStatus) -> Vec<&Task> {
        self.tasks.values().filter(|t| t.status == status).collect()
    }

    pub fn list_by_publisher(&self, publisher_id: &str) -> Vec<&Task> {
        self.tasks
            .values()
            .filter(|t| t.publisher_id == publisher_id)
            .collect()
    }

    pub fn list_by_assignee(&self, assignee_id: &str) -> Vec<&Task> {
        self.tasks
            .values()
            .filter(|t| t.assignee_id.as_deref() == Some(assignee_id))
            .collect()
    }

    // ── CRUD ──────────────────────────────────────────────

    /// Create a new task. If reward > 0, locks escrow from the publisher.
    pub fn create_task(
        &mut self,
        title: String,
        description: String,
        reward: u64,
        publisher_id: String,
        created_tick: u64,
        expires_at: Option<u64>,
    ) -> Result<Uuid, TaskError> {
        self.create_task_with_currency(
            title, description, reward, publisher_id, created_tick, expires_at, Currency::Money,
        )
    }

    /// Create a new task with an explicit currency.
    /// If reward > 0, locks escrow from the publisher.
    pub fn create_task_with_currency(
        &mut self,
        title: String,
        description: String,
        reward: u64,
        publisher_id: String,
        created_tick: u64,
        expires_at: Option<u64>,
        currency: Currency,
    ) -> Result<Uuid, TaskError> {
        let escrow_held = reward > 0;
        if escrow_held {
            let available = self.get_balance(&publisher_id);
            if available < reward {
                // For simplicity we allow creating with insufficient balance;
                // the escrow is still held as a liability.
            }
            self.balances.insert(
                publisher_id.clone(),
                self.get_balance(&publisher_id).saturating_sub(reward),
            );
            let id = Uuid::new_v4();
            self.escrows.insert(id, reward);

            let task = Task {
                id,
                title,
                description,
                status: TaskStatus::Published,
                reward,
                currency,
                escrow_held: true,
                publisher_id,
                assignee_id: None,
                result: None,
                expires_at,
                created_tick,
            };
            self.tasks.insert(id, task);

            self.emit(WorldEvent::TaskCreated {
                task_id: id.to_string(),
                publisher: self.tasks.get(&id).unwrap().publisher_id.clone(),
                reward,
            });

            return Ok(id);
        }

        let id = Uuid::new_v4();
        let task = Task {
            id,
            title,
            description,
            status: TaskStatus::Published,
            reward: 0,
            currency,
            escrow_held: false,
            publisher_id,
            assignee_id: None,
            result: None,
            expires_at,
            created_tick,
        };
        self.tasks.insert(id, task);

        self.emit(WorldEvent::TaskCreated {
            task_id: id.to_string(),
            publisher: self.tasks.get(&id).unwrap().publisher_id.clone(),
            reward: 0,
        });

        Ok(id)
    }

    /// Delete a task. Only published tasks can be deleted.
    /// Refunds escrow if held.
    pub fn delete_task(&mut self, id: Uuid) -> Result<(), TaskError> {
        let task = self.tasks.get(&id)
            .ok_or_else(|| TaskError::NotFound(id.to_string()))?;

        if task.status != TaskStatus::Published {
            return Err(TaskError::InvalidTransition {
                from: task.status,
                to: TaskStatus::Expired,
            });
        }

        // Refund escrow
        if let Some(escrow_amount) = self.escrows.remove(&id) {
            let publisher = &task.publisher_id;
            let bal = self.get_balance(publisher);
            self.balances.insert(publisher.clone(), bal + escrow_amount);
        }

        self.tasks.remove(&id);
        Ok(())
    }

    // ── Lifecycle ─────────────────────────────────────────

    /// Claim a published task.
    ///
    /// If a `ReputationSystem` is configured, agents below the reputation threshold
    /// cannot claim high-value tasks (reward >= configured threshold).
    pub fn claim_task(&mut self, id: Uuid, assignee_id: String) -> Result<(), TaskError> {
        let task = self.tasks.get(&id)
            .ok_or_else(|| TaskError::NotFound(id.to_string()))?;

        if !task.status.can_transition_to(&TaskStatus::Claimed) {
            return Err(TaskError::InvalidTransition {
                from: task.status,
                to: TaskStatus::Claimed,
            });
        }

        // Reputation threshold check for high-value tasks
        if let Some(ref rep_sys) = self.reputation_system {
            if let Err(_) = rep_sys.check_claim_eligibility(&assignee_id, task.reward) {
                let agent_rep = rep_sys.get_reputation(&assignee_id);
                let required_rep = rep_sys.config().min_reputation_for_high_value;
                let reward = task.reward;
                return Err(TaskError::InsufficientReputation {
                    agent_id: assignee_id,
                    reputation: agent_rep,
                    required: required_rep,
                    reward,
                });
            }
        }

        let task = self.tasks.get_mut(&id).unwrap();
        task.status = TaskStatus::Claimed;
        task.assignee_id = Some(assignee_id.clone());

        self.emit(WorldEvent::TaskClaimed {
            task_id: id.to_string(),
            assignee: assignee_id,
        });

        Ok(())
    }

    /// Start working on a claimed task.
    pub fn start_task(&mut self, id: Uuid) -> Result<(), TaskError> {
        let task = self.tasks.get_mut(&id)
            .ok_or_else(|| TaskError::NotFound(id.to_string()))?;

        if !task.status.can_transition_to(&TaskStatus::InProgress) {
            return Err(TaskError::InvalidTransition {
                from: task.status,
                to: TaskStatus::InProgress,
            });
        }

        task.status = TaskStatus::InProgress;

        self.emit(WorldEvent::TaskStarted {
            task_id: id.to_string(),
        });

        Ok(())
    }

    /// Submit result for an in-progress task.
    pub fn submit_result(&mut self, id: Uuid, result: String) -> Result<(), TaskError> {
        if result.is_empty() {
            return Err(TaskError::ResultRequired);
        }

        let task = self.tasks.get_mut(&id)
            .ok_or_else(|| TaskError::NotFound(id.to_string()))?;

        if !task.status.can_transition_to(&TaskStatus::Submitted) {
            return Err(TaskError::InvalidTransition {
                from: task.status,
                to: TaskStatus::Submitted,
            });
        }

        task.status = TaskStatus::Submitted;
        task.result = Some(result);

        self.emit(WorldEvent::TaskSubmitted {
            task_id: id.to_string(),
        });

        Ok(())
    }

    /// Review a submitted task. Only the publisher can review.
    pub fn review_task(&mut self, id: Uuid, reviewer_id: &str, approved: bool) -> Result<(), TaskError> {
        let task = self.tasks.get(&id)
            .ok_or_else(|| TaskError::NotFound(id.to_string()))?;

        if task.publisher_id != reviewer_id {
            return Err(TaskError::NotPublisher {
                expected: task.publisher_id.clone(),
                actual: reviewer_id.to_string(),
            });
        }

        if !task.status.can_transition_to(&TaskStatus::Reviewed) {
            return Err(TaskError::InvalidTransition {
                from: task.status,
                to: TaskStatus::Reviewed,
            });
        }

        let task = self.tasks.get_mut(&id).unwrap();

        if approved {
            task.status = TaskStatus::Reviewed;
            self.emit(WorldEvent::TaskReviewed {
                task_id: id.to_string(),
                approved: true,
            });
        } else {
            // Rejected — go back to in_progress so the worker can resubmit
            task.status = TaskStatus::InProgress;
            self.emit(WorldEvent::TaskReviewed {
                task_id: id.to_string(),
                approved: false,
            });
        }

        Ok(())
    }

    /// Complete a reviewed task. Releases escrow to the assignee.
    ///
    /// If a `RewardDistributor` is configured:
    /// - Platform fee (2%) is deducted and sent to the central bank
    /// - Net reward is paid to the assignee
    /// - XP and reputation are awarded
    /// - Transactions are recorded in the ledger
    /// - A `RewardDistributed` event is emitted
    ///
    /// If a `ReputationSystem` is configured:
    /// - An on-time completion bonus is applied if the task was completed before expiry
    ///
    /// If no `RewardDistributor`, the full escrow is released directly (legacy behavior).
    pub fn complete_task(&mut self, id: Uuid, tick: u64) -> Result<Option<RewardDistribution>, TaskError> {
        // Validate and extract needed data
        let (assignee_id, currency, expires_at) = {
            let task = self.tasks.get(&id)
                .ok_or_else(|| TaskError::NotFound(id.to_string()))?;
            if !task.status.can_transition_to(&TaskStatus::Completed) {
                return Err(TaskError::InvalidTransition {
                    from: task.status,
                    to: TaskStatus::Completed,
                });
            }
            (task.assignee_id.clone(), task.currency, task.expires_at)
        };

        let escrow_amount = self.escrows.remove(&id);

        let distribution = if let (Some(ref assignee), Some(ref mut dist)) = (&assignee_id, self.reward_distributor.as_mut()) {
            // Use reward distributor for fee + XP + reputation + ledger
            let gross = escrow_amount.unwrap_or(0);
            let result = dist.distribute_reward(
                &id.to_string(),
                assignee,
                gross,
                currency,
                tick,
            );

            // Update TaskBoard balance to stay consistent with distributor
            self.balances.insert(
                assignee.clone(),
                self.get_balance(assignee).saturating_add(result.net_reward),
            );

            // Emit RewardDistributed event
            self.emit(WorldEvent::RewardDistributed {
                task_id: id.to_string(),
                assignee_id: assignee.clone(),
                gross_reward: result.gross_reward,
                net_reward: result.net_reward,
                platform_fee: result.platform_fee,
                xp_awarded: result.xp_awarded,
                reputation_change: result.reputation_change,
            });

            Some(result)
        } else {
            // Legacy: release full escrow to assignee
            if let Some(escrow_amount) = escrow_amount {
                if let Some(ref assignee) = assignee_id {
                    let bal = self.get_balance(assignee);
                    self.balances.insert(assignee.clone(), bal.saturating_add(escrow_amount));
                }
            }
            None
        };

        // Apply reputation on-time bonus if ReputationSystem is configured
        if let (Some(ref assignee), Some(ref mut rep_sys)) = (&assignee_id, self.reputation_system.as_mut()) {
            let is_on_time = expires_at.map_or(true, |exp| tick <= exp);
            if is_on_time {
                rep_sys.on_task_completed_on_time(assignee, tick);
            }
        }

        let task = self.tasks.get_mut(&id).unwrap();
        task.status = TaskStatus::Completed;
        task.escrow_held = false;

        self.emit(WorldEvent::TaskCompleted {
            task_id: id.to_string(),
        });

        Ok(distribution)
    }

    /// Expire a published or claimed task. Refunds escrow to publisher.
    ///
    /// If a `ReputationSystem` is configured:
    /// - For claimed tasks: the assignee receives a breach penalty
    /// - For published tasks: the publisher receives a small expiry penalty
    pub fn expire_task(&mut self, id: Uuid) -> Result<(), TaskError> {
        let task = self.tasks.get(&id)
            .ok_or_else(|| TaskError::NotFound(id.to_string()))?;

        if !task.status.can_transition_to(&TaskStatus::Expired) {
            return Err(TaskError::InvalidTransition {
                from: task.status,
                to: TaskStatus::Expired,
            });
        }

        let publisher_id = task.publisher_id.clone();
        let assignee_id = task.assignee_id.clone();
        let was_claimed = task.status == TaskStatus::Claimed;

        // Refund escrow to publisher
        if let Some(escrow_amount) = self.escrows.remove(&id) {
            let bal = self.get_balance(&publisher_id);
            self.balances.insert(publisher_id.clone(), bal + escrow_amount);
        }

        // Apply reputation penalties
        if let Some(ref mut rep_sys) = self.reputation_system {
            if was_claimed {
                // Assignee breached: they claimed but failed to deliver
                if let Some(ref assignee) = assignee_id {
                    rep_sys.on_task_breach(assignee, 0);
                }
            } else {
                // Published task expired: small penalty for publisher
                rep_sys.on_task_expired_published(&publisher_id, 0);
            }
        }

        let task = self.tasks.get_mut(&id).unwrap();
        task.status = TaskStatus::Expired;
        task.escrow_held = false;

        self.emit(WorldEvent::TaskExpired {
            task_id: id.to_string(),
        });

        Ok(())
    }

    /// Batch-expire tasks whose expires_at <= current_tick.
    pub fn process_expiry(&mut self, current_tick: u64) -> Vec<Uuid> {
        let expired_ids: Vec<Uuid> = self.tasks.iter()
            .filter(|(_, task)| {
                matches!(task.status, TaskStatus::Published | TaskStatus::Claimed)
                    && task.expires_at.map_or(false, |exp| exp <= current_tick)
            })
            .map(|(id, _)| *id)
            .collect();

        for id in &expired_ids {
            let _ = self.expire_task(*id);
        }

        expired_ids
    }

    // ── Helpers ────────────────────────────────────────────

    fn emit(&self, event: WorldEvent) {
        if let Some(ref bus) = self.event_bus {
            bus.emit(event);
        }
    }
}

impl Default for TaskBoard {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ─────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_board() -> TaskBoard {
        let mut board = TaskBoard::new();
        board.set_balance("publisher", 10_000);
        board.set_balance("worker", 5_000);
        board
    }

    fn create_default_task(board: &mut TaskBoard) -> Uuid {
        board.create_task(
            "Test Task".to_string(),
            "A test task".to_string(),
            100,
            "publisher".to_string(),
            1,
            None,
        ).unwrap()
    }

    // ── State Machine ──────────────────────────────────────

    #[test]
    fn test_forward_transitions() {
        assert!(TaskStatus::Published.can_transition_to(&TaskStatus::Claimed));
        assert!(TaskStatus::Claimed.can_transition_to(&TaskStatus::InProgress));
        assert!(TaskStatus::InProgress.can_transition_to(&TaskStatus::Submitted));
        assert!(TaskStatus::Submitted.can_transition_to(&TaskStatus::Reviewed));
        assert!(TaskStatus::Reviewed.can_transition_to(&TaskStatus::Completed));
    }

    #[test]
    fn test_expiry_transition() {
        assert!(TaskStatus::Published.can_transition_to(&TaskStatus::Expired));
        assert!(TaskStatus::Claimed.can_transition_to(&TaskStatus::Expired));
    }

    #[test]
    fn test_invalid_transitions() {
        assert!(!TaskStatus::Published.can_transition_to(&TaskStatus::Completed));
        assert!(!TaskStatus::Published.can_transition_to(&TaskStatus::InProgress));
        assert!(!TaskStatus::Claimed.can_transition_to(&TaskStatus::Submitted));
        assert!(!TaskStatus::Completed.can_transition_to(&TaskStatus::Published));
        assert!(!TaskStatus::Expired.can_transition_to(&TaskStatus::Published));
    }

    #[test]
    fn test_terminal_states_have_no_transitions() {
        assert!(TaskStatus::Completed.valid_transitions().is_empty());
        assert!(TaskStatus::Expired.valid_transitions().is_empty());
    }

    #[test]
    fn test_all_statuses_count() {
        assert_eq!(TaskStatus::all().len(), 7);
    }

    // ── Create Task ────────────────────────────────────────

    #[test]
    fn test_create_task_with_escrow() {
        let mut board = make_board();
        let id = create_default_task(&mut board);
        let task = board.get(id).unwrap();
        assert_eq!(task.status, TaskStatus::Published);
        assert_eq!(task.reward, 100);
        assert!(task.escrow_held);
        assert_eq!(board.get_balance("publisher"), 9_900);
    }

    #[test]
    fn test_create_task_no_reward() {
        let mut board = make_board();
        let id = board.create_task(
            "Free Task".to_string(),
            "No reward".to_string(),
            0,
            "publisher".to_string(),
            1,
            None,
        ).unwrap();
        let task = board.get(id).unwrap();
        assert!(!task.escrow_held);
    }

    // ── Full Lifecycle ─────────────────────────────────────

    #[test]
    fn test_full_lifecycle() {
        let mut board = make_board();
        let id = create_default_task(&mut board);
        assert_eq!(board.get(id).unwrap().status, TaskStatus::Published);

        board.claim_task(id, "worker".to_string()).unwrap();
        assert_eq!(board.get(id).unwrap().status, TaskStatus::Claimed);
        assert_eq!(board.get(id).unwrap().assignee_id.as_deref(), Some("worker"));

        board.start_task(id).unwrap();
        assert_eq!(board.get(id).unwrap().status, TaskStatus::InProgress);

        board.submit_result(id, "Work is done!".to_string()).unwrap();
        assert_eq!(board.get(id).unwrap().status, TaskStatus::Submitted);
        assert_eq!(board.get(id).unwrap().result.as_deref(), Some("Work is done!"));

        board.review_task(id, "publisher", true).unwrap();
        assert_eq!(board.get(id).unwrap().status, TaskStatus::Reviewed);

        board.complete_task(id, 10).unwrap();
        assert_eq!(board.get(id).unwrap().status, TaskStatus::Completed);
        assert!(!board.get(id).unwrap().escrow_held);
        assert_eq!(board.get_balance("worker"), 5_100);
    }

    // ── State Guards ───────────────────────────────────────

    #[test]
    fn test_cannot_claim_non_published() {
        let mut board = make_board();
        let id = create_default_task(&mut board);
        board.claim_task(id, "worker".to_string()).unwrap();

        let result = board.claim_task(id, "other".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn test_cannot_submit_empty_result() {
        let mut board = make_board();
        let id = create_default_task(&mut board);
        board.claim_task(id, "worker".to_string()).unwrap();
        board.start_task(id).unwrap();

        let result = board.submit_result(id, "".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn test_non_publisher_cannot_review() {
        let mut board = make_board();
        let id = create_default_task(&mut board);
        board.claim_task(id, "worker".to_string()).unwrap();
        board.start_task(id).unwrap();
        board.submit_result(id, "Done".to_string()).unwrap();

        let result = board.review_task(id, "imposter", true);
        assert!(result.is_err());
    }

    #[test]
    fn test_cannot_skip_states() {
        let mut board = make_board();
        let id = create_default_task(&mut board);

        let result = board.complete_task(id, 10);
        assert!(result.is_err());
    }

    // ── Expiry ─────────────────────────────────────────────

    #[test]
    fn test_expire_published_task() {
        let mut board = make_board();
        let id = create_default_task(&mut board);
        assert_eq!(board.get_balance("publisher"), 9_900);

        board.expire_task(id).unwrap();
        assert_eq!(board.get(id).unwrap().status, TaskStatus::Expired);
        assert!(!board.get(id).unwrap().escrow_held);
        assert_eq!(board.get_balance("publisher"), 10_000);
    }

    #[test]
    fn test_cannot_expire_in_progress() {
        let mut board = make_board();
        let id = create_default_task(&mut board);
        board.claim_task(id, "worker".to_string()).unwrap();
        board.start_task(id).unwrap();

        let result = board.expire_task(id);
        assert!(result.is_err());
    }

    #[test]
    fn test_batch_expiry() {
        let mut board = make_board();
        let id1 = board.create_task("T1".into(), "".into(), 50, "publisher".into(), 1, Some(10)).unwrap();
        let id2 = board.create_task("T2".into(), "".into(), 50, "publisher".into(), 1, Some(100)).unwrap();
        let id3 = board.create_task("T3".into(), "".into(), 50, "publisher".into(), 1, Some(10)).unwrap();

        let expired = board.process_expiry(50);
        assert_eq!(expired.len(), 2);
        assert!(expired.contains(&id1));
        assert!(expired.contains(&id3));
        assert!(!expired.contains(&id2));
    }

    // ── Delete ─────────────────────────────────────────────

    #[test]
    fn test_delete_published_task() {
        let mut board = make_board();
        let id = create_default_task(&mut board);
        board.delete_task(id).unwrap();
        assert!(board.get(id).is_none());
        assert_eq!(board.get_balance("publisher"), 10_000);
    }

    #[test]
    fn test_cannot_delete_claimed_task() {
        let mut board = make_board();
        let id = create_default_task(&mut board);
        board.claim_task(id, "worker".to_string()).unwrap();

        let result = board.delete_task(id);
        assert!(result.is_err());
    }

    // ── Query ──────────────────────────────────────────────

    #[test]
    fn test_list_by_status() {
        let mut board = make_board();
        let id1 = board.create_task("T1".into(), "".into(), 0, "p1".into(), 1, None).unwrap();
        let id2 = board.create_task("T2".into(), "".into(), 0, "p1".into(), 1, None).unwrap();
        board.claim_task(id1, "w1".to_string()).unwrap();

        assert_eq!(board.list_by_status(TaskStatus::Published).len(), 1);
        assert_eq!(board.list_by_status(TaskStatus::Claimed).len(), 1);
    }

    #[test]
    fn test_list_by_publisher() {
        let mut board = make_board();
        board.set_balance("p2", 1000);
        board.create_task("T1".into(), "".into(), 0, "publisher".into(), 1, None).unwrap();
        board.create_task("T2".into(), "".into(), 0, "p2".into(), 1, None).unwrap();

        assert_eq!(board.list_by_publisher("publisher").len(), 1);
        assert_eq!(board.list_by_publisher("p2").len(), 1);
    }

    #[test]
    fn test_list_by_assignee() {
        let mut board = make_board();
        let id = create_default_task(&mut board);
        board.claim_task(id, "worker".to_string()).unwrap();

        assert_eq!(board.list_by_assignee("worker").len(), 1);
        assert_eq!(board.list_by_assignee("nobody").len(), 0);
    }

    // ── Event Bus Integration ──────────────────────────────

    #[test]
    fn test_event_bus_task_lifecycle() {
        let bus = EventBus::new(64);
        let mut rx = bus.subscribe();
        let mut board = TaskBoard::with_event_bus(bus);
        board.set_balance("publisher", 10_000);

        let id = board.create_task(
            "Lifecycle".into(), "desc".into(), 200, "publisher".into(), 1, None
        ).unwrap();
        let _ = rx.try_recv().unwrap(); // TaskCreated

        board.claim_task(id, "worker".to_string()).unwrap();
        let claimed = rx.try_recv().unwrap();
        assert!(matches!(claimed, WorldEvent::TaskClaimed { .. }));

        board.start_task(id).unwrap();
        let started = rx.try_recv().unwrap();
        assert!(matches!(started, WorldEvent::TaskStarted { .. }));

        board.submit_result(id, "Done".into()).unwrap();
        let submitted = rx.try_recv().unwrap();
        assert!(matches!(submitted, WorldEvent::TaskSubmitted { .. }));

        board.review_task(id, "publisher", true).unwrap();
        let reviewed = rx.try_recv().unwrap();
        assert!(matches!(reviewed, WorldEvent::TaskReviewed { approved: true, .. }));

        board.complete_task(id, 10).unwrap();
        let completed = rx.try_recv().unwrap();
        assert!(matches!(completed, WorldEvent::TaskCompleted { .. }));
    }

    // ── Serialization ──────────────────────────────────────

    #[test]
    fn test_task_status_serialization() {
        for status in TaskStatus::all() {
            let json = serde_json::to_string(&status).unwrap();
            let back: TaskStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(status, back);
        }
    }

    #[test]
    fn test_task_record_serialization() {
        let task = Task {
            id: Uuid::new_v4(),
            title: "Test".into(),
            description: "Desc".into(),
            status: TaskStatus::InProgress,
            reward: 100,
            currency: Currency::Money,
            escrow_held: true,
            publisher_id: "p1".into(),
            assignee_id: Some("w1".into()),
            result: None,
            expires_at: Some(100),
            created_tick: 1,
        };
        let json = serde_json::to_string(&task).unwrap();
        let back: Task = serde_json::from_str(&json).unwrap();
        assert_eq!(task.id, back.id);
        assert_eq!(task.title, back.title);
        assert_eq!(task.status, back.status);
        assert_eq!(task.assignee_id, back.assignee_id);
    }

    // ── Review Rejection ───────────────────────────────────

    #[test]
    fn test_review_rejection_goes_back_to_in_progress() {
        let mut board = make_board();
        let id = create_default_task(&mut board);
        board.claim_task(id, "worker".to_string()).unwrap();
        board.start_task(id).unwrap();
        board.submit_result(id, "Bad work".into()).unwrap();

        board.review_task(id, "publisher", false).unwrap();
        assert_eq!(board.get(id).unwrap().status, TaskStatus::InProgress);

        // Worker can resubmit
        board.submit_result(id, "Better work".into()).unwrap();
        assert_eq!(board.get(id).unwrap().status, TaskStatus::Submitted);
    }

    // ── RewardDistributor Integration ──────────────────────────

    #[test]
    fn test_complete_task_with_reward_distributor() {
        let mut board = TaskBoard::with_reward_distributor(RewardConfig::default());
        board.set_balance("publisher", 10_000);
        board.reward_distributor_mut().unwrap().set_balance("worker", 0);

        let id = board.create_task(
            "Reward Task".into(), "desc".into(), 1000, "publisher".into(), 1, None,
        ).unwrap();
        board.claim_task(id, "worker".to_string()).unwrap();
        board.start_task(id).unwrap();
        board.submit_result(id, "Done".into()).unwrap();
        board.review_task(id, "publisher", true).unwrap();

        let dist = board.complete_task(id, 42).unwrap().unwrap();
        assert_eq!(dist.gross_reward, 1000);
        assert_eq!(dist.platform_fee, 20);
        assert_eq!(dist.net_reward, 980);

        // TaskBoard balance is updated consistently
        assert_eq!(board.get_balance("worker"), 980);

        // RewardDistributor also tracks the balance
        let rd = board.reward_distributor().unwrap();
        assert_eq!(rd.get_balance("worker"), 980);
        assert_eq!(rd.get_experience("worker"), 50);
        assert_eq!(rd.get_reputation("worker"), 2.0);

        // Ledger entries have the correct tick
        let entries = rd.ledger().list();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].tick, 42);
        assert_eq!(entries[1].tick, 42);
    }

    #[test]
    fn test_complete_task_with_token_currency() {
        let mut board = TaskBoard::with_reward_distributor(RewardConfig::default());
        board.set_balance("publisher", 10_000);
        board.reward_distributor_mut().unwrap().set_balance("worker", 0);

        let id = board.create_task_with_currency(
            "Token Task".into(), "desc".into(), 5000, "publisher".into(), 1, None, Currency::Token,
        ).unwrap();
        board.claim_task(id, "worker".to_string()).unwrap();
        board.start_task(id).unwrap();
        board.submit_result(id, "Done".into()).unwrap();
        board.review_task(id, "publisher", true).unwrap();

        let dist = board.complete_task(id, 5).unwrap().unwrap();
        assert_eq!(dist.gross_reward, 5000);
        assert_eq!(dist.platform_fee, 100);
        assert_eq!(dist.net_reward, 4900);

        // Ledger entries use Token currency
        let rd = board.reward_distributor().unwrap();
        let entries = rd.ledger().list();
        assert_eq!(entries[0].currency, Currency::Token);
        assert_eq!(entries[1].currency, Currency::Token);
        assert_eq!(rd.central_bank().total_fees(Currency::Token), 100);
    }

    #[test]
    fn test_complete_task_emits_reward_distributed_event() {
        let bus = EventBus::new(64);
        let mut rx = bus.subscribe();
        let mut board = TaskBoard::with_event_bus(bus);
        board.reward_distributor = Some(RewardDistributor::new(RewardConfig::default()));
        board.set_balance("publisher", 10_000);
        board.reward_distributor_mut().unwrap().set_balance("worker", 0);

        let id = board.create_task(
            "Event Task".into(), "desc".into(), 500, "publisher".into(), 1, None,
        ).unwrap();
        let _ = rx.try_recv().unwrap(); // TaskCreated

        board.claim_task(id, "worker".to_string()).unwrap();
        let _ = rx.try_recv().unwrap(); // TaskClaimed

        board.start_task(id).unwrap();
        let _ = rx.try_recv().unwrap(); // TaskStarted

        board.submit_result(id, "Done".into()).unwrap();
        let _ = rx.try_recv().unwrap(); // TaskSubmitted

        board.review_task(id, "publisher", true).unwrap();
        let _ = rx.try_recv().unwrap(); // TaskReviewed

        board.complete_task(id, 10).unwrap();

        // Should get RewardDistributed then TaskCompleted
        let reward_evt = rx.try_recv().unwrap();
        assert!(matches!(reward_evt, WorldEvent::RewardDistributed { .. }));
        if let WorldEvent::RewardDistributed { task_id, assignee_id, gross_reward, net_reward, platform_fee, xp_awarded, reputation_change } = reward_evt {
            assert_eq!(task_id, id.to_string());
            assert_eq!(assignee_id, "worker");
            assert_eq!(gross_reward, 500);
            assert_eq!(platform_fee, 10);
            assert_eq!(net_reward, 490);
            assert_eq!(xp_awarded, 50);
            assert_eq!(reputation_change, 2.0);
        }

        let completed_evt = rx.try_recv().unwrap();
        assert!(matches!(completed_evt, WorldEvent::TaskCompleted { .. }));
    }

    #[test]
    fn test_complete_task_no_distributor_legacy_path() {
        let mut board = TaskBoard::new();
        board.set_balance("publisher", 10_000);

        let id = board.create_task(
            "Legacy Task".into(), "desc".into(), 100, "publisher".into(), 1, None,
        ).unwrap();
        board.claim_task(id, "worker".to_string()).unwrap();
        board.start_task(id).unwrap();
        board.submit_result(id, "Done".into()).unwrap();
        board.review_task(id, "publisher", true).unwrap();

        let result = board.complete_task(id, 10).unwrap();
        // No distributor → returns None, full escrow released
        assert!(result.is_none());
        assert_eq!(board.get_balance("worker"), 100);
    }
}
