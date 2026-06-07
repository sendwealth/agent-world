use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::world::enums::Currency;
use crate::world::event::WorldEvent;
use crate::world::state::EventBus;

use super::reward::{RewardConfig, RewardDistribution, RewardDistributor};

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskError {
    NotFound(String),
    InvalidTransition { from: TaskStatus, to: TaskStatus },
    AlreadyClaimed,
    NoAssignee,
    ResultRequired,
    NotPublisher { expected: String, actual: String },
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
    event_bus: Option<Arc<EventBus>>,
    /// Optional reward distributor for fee deduction, XP, reputation, and ledger.
    reward_distributor: Option<RewardDistributor>,
    /// Coordination tasks (multi-agent collaboration).
    coordination_tasks: HashMap<Uuid, CoordinationTask>,
}

impl TaskBoard {
    pub fn new() -> Self {
        Self {
            tasks: HashMap::new(),
            balances: HashMap::new(),
            escrows: HashMap::new(),
            event_bus: None,
            reward_distributor: None,
            coordination_tasks: HashMap::new(),
        }
    }

    pub fn with_event_bus(event_bus: EventBus) -> Self {
        Self {
            tasks: HashMap::new(),
            balances: HashMap::new(),
            escrows: HashMap::new(),
            event_bus: Some(Arc::new(event_bus)),
            reward_distributor: None,
            coordination_tasks: HashMap::new(),
        }
    }

    /// Create a TaskBoard that shares an existing EventBus via Arc.
    pub fn with_shared_event_bus(event_bus: Arc<EventBus>) -> Self {
        Self {
            tasks: HashMap::new(),
            balances: HashMap::new(),
            escrows: HashMap::new(),
            event_bus: Some(event_bus),
            reward_distributor: None,
            coordination_tasks: HashMap::new(),
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
            coordination_tasks: HashMap::new(),
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
            title,
            description,
            reward,
            publisher_id,
            created_tick,
            expires_at,
            Currency::Money,
        )
    }

    /// Create a new task with an explicit currency.
    /// If reward > 0, locks escrow from the publisher.
    #[allow(clippy::too_many_arguments)]
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
        let task = self
            .tasks
            .get(&id)
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
    pub fn claim_task(&mut self, id: Uuid, assignee_id: String) -> Result<(), TaskError> {
        let task = self
            .tasks
            .get_mut(&id)
            .ok_or_else(|| TaskError::NotFound(id.to_string()))?;

        if !task.status.can_transition_to(&TaskStatus::Claimed) {
            return Err(TaskError::InvalidTransition {
                from: task.status,
                to: TaskStatus::Claimed,
            });
        }

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
        let task = self
            .tasks
            .get_mut(&id)
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

        let task = self
            .tasks
            .get_mut(&id)
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
    pub fn review_task(
        &mut self,
        id: Uuid,
        reviewer_id: &str,
        approved: bool,
    ) -> Result<(), TaskError> {
        let task = self
            .tasks
            .get(&id)
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
    /// If no `RewardDistributor`, the full escrow is released directly (legacy behavior).
    pub fn complete_task(
        &mut self,
        id: Uuid,
        tick: u64,
    ) -> Result<Option<RewardDistribution>, TaskError> {
        // Validate and extract needed data
        let (assignee_id, currency) = {
            let task = self
                .tasks
                .get(&id)
                .ok_or_else(|| TaskError::NotFound(id.to_string()))?;
            if !task.status.can_transition_to(&TaskStatus::Completed) {
                return Err(TaskError::InvalidTransition {
                    from: task.status,
                    to: TaskStatus::Completed,
                });
            }
            (task.assignee_id.clone(), task.currency)
        };

        let escrow_amount = self.escrows.remove(&id);

        let distribution = if let (Some(ref assignee), Some(ref mut dist)) =
            (&assignee_id, self.reward_distributor.as_mut())
        {
            // Use reward distributor for fee + XP + reputation + ledger
            let gross = escrow_amount.unwrap_or(0);
            let result = dist.distribute_reward(&id.to_string(), assignee, gross, currency, tick);

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
                    self.balances
                        .insert(assignee.clone(), bal.saturating_add(escrow_amount));
                }
            }
            None
        };

        let task = self.tasks.get_mut(&id).unwrap();
        task.status = TaskStatus::Completed;
        task.escrow_held = false;

        self.emit(WorldEvent::TaskCompleted {
            task_id: id.to_string(),
        });

        Ok(distribution)
    }

    /// Expire a published or claimed task. Refunds escrow to publisher.
    pub fn expire_task(&mut self, id: Uuid) -> Result<(), TaskError> {
        let task = self
            .tasks
            .get(&id)
            .ok_or_else(|| TaskError::NotFound(id.to_string()))?;

        if !task.status.can_transition_to(&TaskStatus::Expired) {
            return Err(TaskError::InvalidTransition {
                from: task.status,
                to: TaskStatus::Expired,
            });
        }

        let publisher_id = task.publisher_id.clone();

        // Refund escrow to publisher
        if let Some(escrow_amount) = self.escrows.remove(&id) {
            let bal = self.get_balance(&publisher_id);
            self.balances
                .insert(publisher_id.clone(), bal + escrow_amount);
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
        let expired_ids: Vec<Uuid> = self
            .tasks
            .iter()
            .filter(|(_, task)| {
                matches!(task.status, TaskStatus::Published | TaskStatus::Claimed)
                    && task.expires_at.is_some_and(|exp| exp <= current_tick)
            })
            .map(|(id, _)| *id)
            .collect();

        for id in &expired_ids {
            if let Err(e) = self.expire_task(*id) {
                tracing::error!("Task expiration failed for {}: {:?}", id, e);
            }
        }

        expired_ids
    }

    // ── Coordination Tasks (Multi-Agent) ──────────────────

    /// Get a coordination task by ID.
    pub fn get_coordination_task(&self, id: Uuid) -> Option<&CoordinationTask> {
        self.coordination_tasks.get(&id)
    }

    /// List all coordination tasks.
    pub fn list_coordination_tasks(&self) -> Vec<&CoordinationTask> {
        self.coordination_tasks.values().collect()
    }

    /// List coordination tasks by status.
    pub fn list_coordination_tasks_by_status(
        &self,
        status: CoordinationTaskStatus,
    ) -> Vec<&CoordinationTask> {
        self.coordination_tasks
            .values()
            .filter(|t| t.status == status)
            .collect()
    }

    /// List coordination tasks where a given agent is a participant.
    pub fn list_coordination_tasks_by_participant(&self, agent_id: &str) -> Vec<&CoordinationTask> {
        self.coordination_tasks
            .values()
            .filter(|t| t.is_participant(agent_id))
            .collect()
    }

    /// Create a new coordination task. Escrows the reward pool from the coordinator.
    /// If `org_id` is set, only members of that organization may join.
    #[allow(clippy::too_many_arguments)]
    pub fn create_coordination_task(
        &mut self,
        title: String,
        description: String,
        reward_pool: u64,
        currency: Currency,
        coordinator_id: String,
        max_agents: usize,
        created_tick: u64,
        expires_at: Option<u64>,
        org_id: Option<String>,
    ) -> Result<Uuid, CoordinationTaskError> {
        let escrow_held = reward_pool > 0;
        if escrow_held {
            let available = self.get_balance(&coordinator_id);
            if available < reward_pool {
                // Allow creation with insufficient balance as liability (same as solo tasks)
            }
            self.balances.insert(
                coordinator_id.clone(),
                self.get_balance(&coordinator_id)
                    .saturating_sub(reward_pool),
            );
        }

        let id = Uuid::new_v4();
        let task = CoordinationTask {
            id,
            title,
            description,
            status: CoordinationTaskStatus::Open,
            reward_pool,
            currency,
            escrow_held,
            coordinator_id: coordinator_id.clone(),
            max_agents,
            participants: vec![coordinator_id.clone()],
            contributions: HashMap::new(),
            reward_overrides: HashMap::new(),
            org_id,
            expires_at,
            created_tick,
        };

        if escrow_held {
            self.escrows.insert(id, reward_pool);
        }

        self.coordination_tasks.insert(id, task);

        self.emit(WorldEvent::CoordinationTaskCreated {
            task_id: id.to_string(),
            coordinator_id,
            max_agents,
        });

        Ok(id)
    }

    /// Join an open coordination task.
    /// `is_org_member` is an optional closure that checks org membership.
    /// If the task has an `org_id`, the closure must return `true` for the agent to join.
    pub fn join_coordination_task<F>(
        &mut self,
        id: Uuid,
        agent_id: String,
        is_org_member: F,
    ) -> Result<(), CoordinationTaskError>
    where
        F: Fn(&str, &str) -> bool,
    {
        // Validate (immutable borrow scope)
        {
            let task = self
                .coordination_tasks
                .get(&id)
                .ok_or_else(|| CoordinationTaskError::NotFound(id.to_string()))?;

            if task.status != CoordinationTaskStatus::Open {
                return Err(CoordinationTaskError::InvalidTransition {
                    from: task.status,
                    to: CoordinationTaskStatus::Open,
                });
            }

            if task.is_participant(&agent_id) {
                return Err(CoordinationTaskError::AlreadyJoined);
            }

            if task.participant_count() >= task.max_agents {
                return Err(CoordinationTaskError::TaskFull);
            }

            // Check org membership if this task is org-scoped
            if let Some(ref org_id) = task.org_id {
                if !is_org_member(&agent_id, org_id) {
                    return Err(CoordinationTaskError::NotOrgMember {
                        agent_id,
                        org_id: org_id.clone(),
                    });
                }
            }
        }

        // Mutate
        let task = self.coordination_tasks.get_mut(&id).unwrap();
        task.participants.push(agent_id.clone());

        self.emit(WorldEvent::CoordinationTaskAgentJoined {
            task_id: id.to_string(),
            agent_id,
        });

        Ok(())
    }

    /// Submit a contribution to a coordination task.
    pub fn submit_coordination_contribution(
        &mut self,
        id: Uuid,
        agent_id: &str,
        content: String,
        tick: u64,
    ) -> Result<(), CoordinationTaskError> {
        if content.is_empty() {
            return Err(CoordinationTaskError::ContributionRequired);
        }

        let task = self
            .coordination_tasks
            .get(&id)
            .ok_or_else(|| CoordinationTaskError::NotFound(id.to_string()))?;

        if !task.is_participant(agent_id) {
            return Err(CoordinationTaskError::NotParticipant);
        }

        if task.contributions.contains_key(agent_id) {
            return Err(CoordinationTaskError::AlreadySubmitted);
        }

        let current_status = task.status;
        if !matches!(
            current_status,
            CoordinationTaskStatus::Open | CoordinationTaskStatus::InProgress
        ) {
            return Err(CoordinationTaskError::InvalidTransition {
                from: current_status,
                to: current_status,
            });
        }

        let task = self.coordination_tasks.get_mut(&id).unwrap();
        task.contributions.insert(
            agent_id.to_string(),
            Contribution {
                agent_id: agent_id.to_string(),
                content,
                submitted_tick: tick,
            },
        );

        // Auto-transition: if all participants have submitted, move to AllSubmitted
        let all_in = task
            .participants
            .iter()
            .all(|a| task.contributions.contains_key(a));
        if all_in {
            task.status = CoordinationTaskStatus::AllSubmitted;
        } else if task.status == CoordinationTaskStatus::Open {
            task.status = CoordinationTaskStatus::InProgress;
        }

        self.emit(WorldEvent::CoordinationTaskAgentSubmitted {
            task_id: id.to_string(),
            agent_id: agent_id.to_string(),
        });

        Ok(())
    }

    /// Complete a coordination task. Distributes the reward pool equally among participants.
    /// The coordinator can optionally provide reward_overrides to distribute unevenly.
    pub fn complete_coordination_task(
        &mut self,
        id: Uuid,
        reviewer_id: &str,
        reward_overrides: Option<HashMap<String, u64>>,
    ) -> Result<HashMap<String, u64>, CoordinationTaskError> {
        let task = self
            .coordination_tasks
            .get(&id)
            .ok_or_else(|| CoordinationTaskError::NotFound(id.to_string()))?;

        if task.coordinator_id != reviewer_id {
            return Err(CoordinationTaskError::NotCoordinator {
                expected: task.coordinator_id.clone(),
                actual: reviewer_id.to_string(),
            });
        }

        if !task
            .status
            .can_transition_to(&CoordinationTaskStatus::Completed)
        {
            return Err(CoordinationTaskError::InvalidTransition {
                from: task.status,
                to: CoordinationTaskStatus::Completed,
            });
        }

        if task.participants.is_empty() {
            return Err(CoordinationTaskError::NoParticipants);
        }

        // Calculate distribution
        let distribution = if let Some(ref overrides) = reward_overrides {
            // Use overrides if provided; validate total doesn't exceed pool
            let total: u64 = overrides.values().sum();
            if total > task.reward_pool {
                // Cap at pool; distribute proportionally
                let mut dist = HashMap::new();
                for agent_id in &task.participants {
                    let amount = overrides.get(agent_id).copied().unwrap_or(0);
                    let capped = if total > 0 {
                        (amount as u128 * task.reward_pool as u128 / total as u128) as u64
                    } else {
                        0
                    };
                    dist.insert(agent_id.clone(), capped);
                }
                dist
            } else {
                let mut dist: HashMap<String, u64> = overrides.clone();
                // Any participant not in overrides gets equal share of remainder
                let remaining = task.reward_pool - total;
                let unassigned: Vec<&String> = task
                    .participants
                    .iter()
                    .filter(|p| !overrides.contains_key(*p))
                    .collect();
                if !unassigned.is_empty() {
                    let share = remaining / unassigned.len() as u64;
                    for agent_id in unassigned {
                        dist.insert(agent_id.clone(), share);
                    }
                }
                dist
            }
        } else {
            // Equal distribution
            let share = task.equal_share();
            task.participants
                .iter()
                .map(|p| (p.clone(), share))
                .collect()
        };

        // Release escrow and credit participants
        let _escrow_amount = self.escrows.remove(&id);
        for (agent_id, amount) in &distribution {
            let bal = self.get_balance(agent_id);
            self.balances.insert(agent_id.clone(), bal + amount);
        }

        let contributor_count = distribution.len();
        let task = self.coordination_tasks.get_mut(&id).unwrap();
        task.status = CoordinationTaskStatus::Completed;
        task.escrow_held = false;
        task.reward_overrides = distribution.clone();

        self.emit(WorldEvent::CoordinationTaskCompleted {
            task_id: id.to_string(),
            contributor_count,
        });

        Ok(distribution)
    }

    /// Cancel a coordination task. Only the coordinator can cancel. Refunds escrow.
    pub fn cancel_coordination_task(
        &mut self,
        id: Uuid,
        coordinator_id: &str,
    ) -> Result<(), CoordinationTaskError> {
        // Validate first (immutable borrow)
        let (expected_coord, can_cancel) = {
            let task = self
                .coordination_tasks
                .get(&id)
                .ok_or_else(|| CoordinationTaskError::NotFound(id.to_string()))?;
            (
                task.coordinator_id.clone(),
                task.status
                    .can_transition_to(&CoordinationTaskStatus::Cancelled),
            )
        };

        if expected_coord != coordinator_id {
            return Err(CoordinationTaskError::NotCoordinator {
                expected: expected_coord,
                actual: coordinator_id.to_string(),
            });
        }

        if !can_cancel {
            return Err(CoordinationTaskError::InvalidTransition {
                from: self.coordination_tasks.get(&id).unwrap().status,
                to: CoordinationTaskStatus::Cancelled,
            });
        }

        // Refund escrow
        if let Some(escrow_amount) = self.escrows.remove(&id) {
            let bal = self.get_balance(coordinator_id);
            self.balances
                .insert(coordinator_id.to_string(), bal + escrow_amount);
        }

        // Update status
        let task = self.coordination_tasks.get_mut(&id).unwrap();
        task.status = CoordinationTaskStatus::Cancelled;
        task.escrow_held = false;

        self.emit(WorldEvent::CoordinationTaskCancelled {
            task_id: id.to_string(),
            coordinator_id: coordinator_id.to_string(),
        });

        Ok(())
    }

    /// Expire a coordination task. Refunds escrow to the coordinator.
    pub fn expire_coordination_task(&mut self, id: Uuid) -> Result<(), CoordinationTaskError> {
        // Validate first (immutable borrow)
        let (coordinator_id, can_expire) = {
            let task = self
                .coordination_tasks
                .get(&id)
                .ok_or_else(|| CoordinationTaskError::NotFound(id.to_string()))?;
            (
                task.coordinator_id.clone(),
                task.status
                    .can_transition_to(&CoordinationTaskStatus::Expired),
            )
        };

        if !can_expire {
            return Err(CoordinationTaskError::InvalidTransition {
                from: self.coordination_tasks.get(&id).unwrap().status,
                to: CoordinationTaskStatus::Expired,
            });
        }

        // Refund escrow
        if let Some(escrow_amount) = self.escrows.remove(&id) {
            let bal = self.get_balance(&coordinator_id);
            self.balances.insert(coordinator_id, bal + escrow_amount);
        }

        // Update status
        let task = self.coordination_tasks.get_mut(&id).unwrap();
        task.status = CoordinationTaskStatus::Expired;
        task.escrow_held = false;

        self.emit(WorldEvent::CoordinationTaskExpired {
            task_id: id.to_string(),
        });

        Ok(())
    }

    /// Batch-expire coordination tasks whose expires_at <= current_tick.
    pub fn process_coordination_expiry(&mut self, current_tick: u64) -> Vec<Uuid> {
        let expired: Vec<Uuid> = self
            .coordination_tasks
            .iter()
            .filter(|(_, task)| {
                matches!(
                    task.status,
                    CoordinationTaskStatus::Open | CoordinationTaskStatus::InProgress
                ) && task.expires_at.is_some_and(|exp| exp <= current_tick)
            })
            .map(|(id, _)| *id)
            .collect();

        for id in &expired {
            if let Err(e) = self.expire_coordination_task(*id) {
                tracing::error!("Coordination task expiration failed for {}: {:?}", id, e);
            }
        }

        expired
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

// ── Multi-Agent Coordination ──────────────────────────────

/// Status of a coordination (multi-agent) task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoordinationTaskStatus {
    /// Open for agents to join.
    Open,
    /// All agents have joined; work in progress.
    InProgress,
    /// All agents submitted their contributions; ready for review.
    AllSubmitted,
    /// Reviewed and completed; rewards distributed.
    Completed,
    /// Expired before completion.
    Expired,
    /// Cancelled by the coordinator.
    Cancelled,
}

impl CoordinationTaskStatus {
    pub fn can_transition_to(&self, next: &CoordinationTaskStatus) -> bool {
        matches!(
            (self, next),
            (
                CoordinationTaskStatus::Open,
                CoordinationTaskStatus::InProgress
            ) | (
                CoordinationTaskStatus::Open,
                CoordinationTaskStatus::Expired
            ) | (
                CoordinationTaskStatus::Open,
                CoordinationTaskStatus::Cancelled
            ) | (
                CoordinationTaskStatus::InProgress,
                CoordinationTaskStatus::AllSubmitted
            ) | (
                CoordinationTaskStatus::InProgress,
                CoordinationTaskStatus::Expired
            ) | (
                CoordinationTaskStatus::InProgress,
                CoordinationTaskStatus::Cancelled
            ) | (
                CoordinationTaskStatus::AllSubmitted,
                CoordinationTaskStatus::Completed
            ) | (
                CoordinationTaskStatus::AllSubmitted,
                CoordinationTaskStatus::Cancelled
            )
        )
    }
}

impl std::fmt::Display for CoordinationTaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CoordinationTaskStatus::Open => write!(f, "open"),
            CoordinationTaskStatus::InProgress => write!(f, "in_progress"),
            CoordinationTaskStatus::AllSubmitted => write!(f, "all_submitted"),
            CoordinationTaskStatus::Completed => write!(f, "completed"),
            CoordinationTaskStatus::Expired => write!(f, "expired"),
            CoordinationTaskStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}

/// A single agent's contribution to a coordination task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contribution {
    pub agent_id: String,
    /// The contribution payload (free-form string).
    pub content: String,
    /// Tick when the contribution was submitted.
    pub submitted_tick: u64,
}

/// A coordination task that multiple agents collaborate on.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoordinationTask {
    pub id: Uuid,
    pub title: String,
    pub description: String,
    pub status: CoordinationTaskStatus,
    /// Total reward pool for this task, escrowed from the coordinator.
    pub reward_pool: u64,
    pub currency: Currency,
    pub escrow_held: bool,
    /// The agent who created and coordinates the task.
    pub coordinator_id: String,
    /// Maximum number of agents that can participate.
    pub max_agents: usize,
    /// Agents currently participating.
    pub participants: Vec<String>,
    /// Contributions submitted by participants (agent_id → contribution).
    pub contributions: HashMap<String, Contribution>,
    /// Optional per-agent reward overrides set during completion.
    pub reward_overrides: HashMap<String, u64>,
    /// If set, only members of this organization can join the task.
    pub org_id: Option<String>,
    pub expires_at: Option<u64>,
    pub created_tick: u64,
}

impl CoordinationTask {
    /// How many agents have joined.
    pub fn participant_count(&self) -> usize {
        self.participants.len()
    }

    /// How many participants have submitted contributions.
    pub fn submitted_count(&self) -> usize {
        self.contributions.len()
    }

    /// Whether all participants have submitted.
    pub fn all_submitted(&self) -> bool {
        !self.participants.is_empty()
            && self
                .participants
                .iter()
                .all(|a| self.contributions.contains_key(a))
    }

    /// Check if an agent is a participant.
    pub fn is_participant(&self, agent_id: &str) -> bool {
        self.participants.iter().any(|p| p == agent_id)
    }

    /// Compute the equal share of the reward pool.
    pub fn equal_share(&self) -> u64 {
        if self.participants.is_empty() {
            0
        } else {
            self.reward_pool / self.participants.len() as u64
        }
    }
}

/// Errors for coordination task operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoordinationTaskError {
    NotFound(String),
    InvalidTransition {
        from: CoordinationTaskStatus,
        to: CoordinationTaskStatus,
    },
    AlreadyJoined,
    NotParticipant,
    AlreadySubmitted,
    TaskFull,
    NotCoordinator {
        expected: String,
        actual: String,
    },
    Expired,
    NoParticipants,
    ContributionRequired,
    /// Agent is not a member of the organization that owns this task.
    NotOrgMember {
        agent_id: String,
        org_id: String,
    },
}

impl std::fmt::Display for CoordinationTaskError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CoordinationTaskError::NotFound(id) => write!(f, "coordination task not found: {}", id),
            CoordinationTaskError::InvalidTransition { from, to } => {
                write!(f, "invalid transition: {} -> {}", from, to)
            }
            CoordinationTaskError::AlreadyJoined => write!(f, "agent already joined this task"),
            CoordinationTaskError::NotParticipant => write!(f, "agent is not a participant"),
            CoordinationTaskError::AlreadySubmitted => {
                write!(f, "agent already submitted contribution")
            }
            CoordinationTaskError::TaskFull => write!(f, "task has reached maximum participants"),
            CoordinationTaskError::NotCoordinator { expected, actual } => {
                write!(
                    f,
                    "only the coordinator can perform this action: expected {}, got {}",
                    expected, actual
                )
            }
            CoordinationTaskError::Expired => write!(f, "task has expired"),
            CoordinationTaskError::NoParticipants => write!(f, "task has no participants"),
            CoordinationTaskError::ContributionRequired => {
                write!(f, "contribution content is required")
            }
            CoordinationTaskError::NotOrgMember { agent_id, org_id } => {
                write!(
                    f,
                    "agent {} is not a member of organization {}",
                    agent_id, org_id
                )
            }
        }
    }
}

impl std::error::Error for CoordinationTaskError {}

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
        board
            .create_task(
                "Test Task".to_string(),
                "A test task".to_string(),
                100,
                "publisher".to_string(),
                1,
                None,
            )
            .unwrap()
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
        let id = board
            .create_task(
                "Free Task".to_string(),
                "No reward".to_string(),
                0,
                "publisher".to_string(),
                1,
                None,
            )
            .unwrap();
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
        assert_eq!(
            board.get(id).unwrap().assignee_id.as_deref(),
            Some("worker")
        );

        board.start_task(id).unwrap();
        assert_eq!(board.get(id).unwrap().status, TaskStatus::InProgress);

        board
            .submit_result(id, "Work is done!".to_string())
            .unwrap();
        assert_eq!(board.get(id).unwrap().status, TaskStatus::Submitted);
        assert_eq!(
            board.get(id).unwrap().result.as_deref(),
            Some("Work is done!")
        );

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
        let id1 = board
            .create_task("T1".into(), "".into(), 50, "publisher".into(), 1, Some(10))
            .unwrap();
        let id2 = board
            .create_task("T2".into(), "".into(), 50, "publisher".into(), 1, Some(100))
            .unwrap();
        let id3 = board
            .create_task("T3".into(), "".into(), 50, "publisher".into(), 1, Some(10))
            .unwrap();

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
        let id1 = board
            .create_task("T1".into(), "".into(), 0, "p1".into(), 1, None)
            .unwrap();
        let _id2 = board
            .create_task("T2".into(), "".into(), 0, "p1".into(), 1, None)
            .unwrap();
        board.claim_task(id1, "w1".to_string()).unwrap();

        assert_eq!(board.list_by_status(TaskStatus::Published).len(), 1);
        assert_eq!(board.list_by_status(TaskStatus::Claimed).len(), 1);
    }

    #[test]
    fn test_list_by_publisher() {
        let mut board = make_board();
        board.set_balance("p2", 1000);
        board
            .create_task("T1".into(), "".into(), 0, "publisher".into(), 1, None)
            .unwrap();
        board
            .create_task("T2".into(), "".into(), 0, "p2".into(), 1, None)
            .unwrap();

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

        let id = board
            .create_task(
                "Lifecycle".into(),
                "desc".into(),
                200,
                "publisher".into(),
                1,
                None,
            )
            .unwrap();
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
        assert!(matches!(
            reviewed,
            WorldEvent::TaskReviewed { approved: true, .. }
        ));

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
        board
            .reward_distributor_mut()
            .unwrap()
            .set_balance("worker", 0);

        let id = board
            .create_task(
                "Reward Task".into(),
                "desc".into(),
                1000,
                "publisher".into(),
                1,
                None,
            )
            .unwrap();
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
        board
            .reward_distributor_mut()
            .unwrap()
            .set_balance("worker", 0);

        let id = board
            .create_task_with_currency(
                "Token Task".into(),
                "desc".into(),
                5000,
                "publisher".into(),
                1,
                None,
                Currency::Token,
            )
            .unwrap();
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
        board
            .reward_distributor_mut()
            .unwrap()
            .set_balance("worker", 0);

        let id = board
            .create_task(
                "Event Task".into(),
                "desc".into(),
                500,
                "publisher".into(),
                1,
                None,
            )
            .unwrap();
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
        if let WorldEvent::RewardDistributed {
            task_id,
            assignee_id,
            gross_reward,
            net_reward,
            platform_fee,
            xp_awarded,
            reputation_change,
        } = reward_evt
        {
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

        let id = board
            .create_task(
                "Legacy Task".into(),
                "desc".into(),
                100,
                "publisher".into(),
                1,
                None,
            )
            .unwrap();
        board.claim_task(id, "worker".to_string()).unwrap();
        board.start_task(id).unwrap();
        board.submit_result(id, "Done".into()).unwrap();
        board.review_task(id, "publisher", true).unwrap();

        let result = board.complete_task(id, 10).unwrap();
        // No distributor → returns None, full escrow released
        assert!(result.is_none());
        assert_eq!(board.get_balance("worker"), 100);
    }

    // ── Coordination Task Integration Tests ────────────────

    #[test]
    fn test_coordination_task_full_lifecycle_with_reward_distribution() {
        // Integration test: 3 agents collaborate on a team task,
        // submit contributions, coordinator completes with reward overrides.
        let mut board = make_board();
        board.set_balance("coordinator", 10_000);
        board.set_balance("agent_a", 1_000);
        board.set_balance("agent_b", 1_000);

        // 1. Create coordination task with 1000 reward pool, max 3 agents
        let id = board
            .create_coordination_task(
                "Build Bridge".into(),
                "Collaborative bridge building".into(),
                1000,
                Currency::Money,
                "coordinator".into(),
                3,
                1,
                None,
                None,
            )
            .unwrap();

        // Verify initial state
        let task = board.get_coordination_task(id).unwrap();
        assert_eq!(task.status, CoordinationTaskStatus::Open);
        assert_eq!(task.reward_pool, 1000);
        assert!(task.escrow_held);
        assert_eq!(task.participant_count(), 1); // coordinator auto-joins
        assert_eq!(board.get_balance("coordinator"), 9_000); // 1000 escrowed

        // 2. Two more agents join
        board
            .join_coordination_task(id, "agent_a".into(), |_, _| true)
            .unwrap();
        board
            .join_coordination_task(id, "agent_b".into(), |_, _| true)
            .unwrap();

        let task = board.get_coordination_task(id).unwrap();
        assert_eq!(task.participant_count(), 3);

        // 3. All participants submit contributions
        board
            .submit_coordination_contribution(id, "coordinator", "Designed blueprint".into(), 10)
            .unwrap();
        // Status should auto-transition to InProgress after first submission
        let task = board.get_coordination_task(id).unwrap();
        assert_eq!(task.status, CoordinationTaskStatus::InProgress);

        board
            .submit_coordination_contribution(id, "agent_a", "Built foundations".into(), 15)
            .unwrap();
        board
            .submit_coordination_contribution(id, "agent_b", "Painted bridge".into(), 20)
            .unwrap();

        // All submitted → auto-transition to AllSubmitted
        let task = board.get_coordination_task(id).unwrap();
        assert_eq!(task.status, CoordinationTaskStatus::AllSubmitted);
        assert!(task.all_submitted());

        // 4. Coordinator completes with reward overrides (proportional to contribution)
        let mut overrides = HashMap::new();
        overrides.insert("coordinator".into(), 400);
        overrides.insert("agent_a".into(), 400);
        overrides.insert("agent_b".into(), 200);

        let distribution = board
            .complete_coordination_task(id, "coordinator", Some(overrides))
            .unwrap();

        // Verify reward distribution
        assert_eq!(distribution.get("coordinator"), Some(&400));
        assert_eq!(distribution.get("agent_a"), Some(&400));
        assert_eq!(distribution.get("agent_b"), Some(&200));

        // Verify balances
        assert_eq!(board.get_balance("coordinator"), 9_400); // 9000 + 400 reward (original 10000 - 1000 escrow + 400)
        assert_eq!(board.get_balance("agent_a"), 1_400); // 1000 + 400
        assert_eq!(board.get_balance("agent_b"), 1_200); // 1000 + 200

        // Verify final state
        let task = board.get_coordination_task(id).unwrap();
        assert_eq!(task.status, CoordinationTaskStatus::Completed);
        assert!(!task.escrow_held);
    }

    #[test]
    fn test_coordination_task_org_restriction_and_equal_distribution() {
        // Integration test: org-scoped task, non-members can't join,
        // equal reward distribution when no overrides.
        let mut board = make_board();
        board.set_balance("org_lead", 10_000);
        board.set_balance("org_member", 1_000);

        // 1. Create org-scoped coordination task
        let id = board
            .create_coordination_task(
                "Org Project".into(),
                "Members only".into(),
                600,
                Currency::Token,
                "org_lead".into(),
                3,
                1,
                None,
                Some("org_123".into()),
            )
            .unwrap();

        // 2. Org member joins successfully
        board
            .join_coordination_task(id, "org_member".into(), |agent_id, org_id| {
                // Simulate org membership check
                org_id == "org_123" && (agent_id == "org_member" || agent_id == "org_lead")
            })
            .unwrap();

        // 3. Non-member fails to join
        let result = board.join_coordination_task(id, "outsider".into(), |agent_id, org_id| {
            org_id == "org_123" && (agent_id == "org_member" || agent_id == "org_lead")
        });
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            CoordinationTaskError::NotOrgMember {
                agent_id: "outsider".into(),
                org_id: "org_123".into(),
            }
        );

        // Verify only 2 participants
        let task = board.get_coordination_task(id).unwrap();
        assert_eq!(task.participant_count(), 2);

        // 4. Submit contributions
        board
            .submit_coordination_contribution(id, "org_lead", "Led the project".into(), 5)
            .unwrap();
        board
            .submit_coordination_contribution(id, "org_member", "Did the work".into(), 10)
            .unwrap();

        // 5. Complete with equal distribution (no overrides)
        let distribution = board
            .complete_coordination_task(id, "org_lead", None)
            .unwrap();

        // Equal split: 600 / 2 = 300 each
        assert_eq!(distribution.get("org_lead"), Some(&300));
        assert_eq!(distribution.get("org_member"), Some(&300));

        // Verify balances
        assert_eq!(board.get_balance("org_lead"), 9_700); // 10000 - 600 + 300
        assert_eq!(board.get_balance("org_member"), 1_300); // 1000 + 300
    }

    #[test]
    fn test_coordination_task_cancel_and_expiry() {
        let mut board = make_board();
        board.set_balance("coord", 10_000);

        // Create + cancel
        let id1 = board
            .create_coordination_task(
                "Cancel Test".into(),
                "Will be cancelled".into(),
                500,
                Currency::Money,
                "coord".into(),
                3,
                1,
                None,
                None,
            )
            .unwrap();
        assert_eq!(board.get_balance("coord"), 9_500);

        board
            .cancel_coordination_task(id1, "coord")
            .unwrap();
        let task = board.get_coordination_task(id1).unwrap();
        assert_eq!(task.status, CoordinationTaskStatus::Cancelled);
        assert!(!task.escrow_held);
        assert_eq!(board.get_balance("coord"), 10_000); // refund

        // Create + expire
        let id2 = board
            .create_coordination_task(
                "Expire Test".into(),
                "Will expire".into(),
                300,
                Currency::Money,
                "coord".into(),
                3,
                1,
                Some(100),
                None,
            )
            .unwrap();
        assert_eq!(board.get_balance("coord"), 9_700);

        let expired = board.process_coordination_expiry(200);
        assert_eq!(expired.len(), 1);
        assert_eq!(expired[0], id2);

        let task = board.get_coordination_task(id2).unwrap();
        assert_eq!(task.status, CoordinationTaskStatus::Expired);
        assert_eq!(board.get_balance("coord"), 10_000); // refund
    }

    #[test]
    fn test_coordination_task_already_joined_and_full() {
        let mut board = make_board();
        board.set_balance("coord", 10_000);

        let id = board
            .create_coordination_task(
                "Small Team".into(),
                "Max 2 agents".into(),
                100,
                Currency::Money,
                "coord".into(),
                2,
                1,
                None,
                None,
            )
            .unwrap();

        // Coordinator auto-joined, so 1 slot left
        // Joining again fails
        let result = board.join_coordination_task(id, "coord".into(), |_, _| true);
        assert_eq!(result.unwrap_err(), CoordinationTaskError::AlreadyJoined);

        // One agent joins successfully
        board
            .join_coordination_task(id, "agent_a".into(), |_, _| true)
            .unwrap();

        // Third agent fails — task full
        let result = board.join_coordination_task(id, "agent_b".into(), |_, _| true);
        assert_eq!(result.unwrap_err(), CoordinationTaskError::TaskFull);
    }
}
