//! Migration Manager — handles cross-world agent migration lifecycle.
//!
//! Migration flow:
//! 1. Agent submits migration application on source world
//! 2. Target world reviews and approves/rejects
//! 3. On approval, migration is executed:
//!    - Agent is removed from source world (emigration)
//!    - Agent snapshot is transferred to target world
//!    - Agent is spawned in target world (immigration)
//!    - Resource tax is applied
//!    - Skills may need recertification

use std::collections::HashMap;
use std::sync::Arc;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::world::event::WorldEvent;
use crate::world::state::EventBus;

// serde_json no longer needed here after removing Custom events

// ── Migration Status ──────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MigrationStatus {
    Pending,
    Approved,
    Rejected,
    Executing,
    Completed,
    Cancelled,
    Failed,
}

impl std::fmt::Display for MigrationStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MigrationStatus::Pending => write!(f, "pending"),
            MigrationStatus::Approved => write!(f, "approved"),
            MigrationStatus::Rejected => write!(f, "rejected"),
            MigrationStatus::Executing => write!(f, "executing"),
            MigrationStatus::Completed => write!(f, "completed"),
            MigrationStatus::Cancelled => write!(f, "cancelled"),
            MigrationStatus::Failed => write!(f, "failed"),
        }
    }
}

// ── Agent Snapshot ────────────────────────────────────────

/// Snapshot of an agent's state for migration transfer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSnapshot {
    pub agent_id: String,
    pub name: String,
    pub phase: String,
    pub tokens: u64,
    pub money: u64,
    pub reputation: f64,
    pub skills: HashMap<String, u64>,
    pub metadata: HashMap<String, String>,
    pub source_world_id: String,
    pub memory_data: Vec<u8>,
    pub public_key: String,
}

// ── Migration Policy ──────────────────────────────────────

/// Configuration for migration rules in a world.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationPolicy {
    pub enabled: bool,
    pub daily_quota: u32,
    pub weekly_quota: u32,
    pub min_reputation: f64,
    pub token_cost: u64,
    pub resource_tax_rate: f64, // 0.0 - 1.0
    pub require_skill_certification: bool,
    pub blocked_skills: Vec<String>,
    pub cooldown_ticks: u32,
}

impl Default for MigrationPolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            daily_quota: 10,
            weekly_quota: 50,
            min_reputation: 0.0,
            token_cost: 10_000,
            resource_tax_rate: 0.2, // 20% tax on resources
            require_skill_certification: false,
            blocked_skills: Vec::new(),
            cooldown_ticks: 100,
        }
    }
}

// ── Migration Application ─────────────────────────────────

/// A migration application tracks the full lifecycle of an agent transfer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationApplication {
    pub migration_id: String,
    pub agent_id: String,
    pub source_world_id: String,
    pub target_world_id: String,
    pub status: MigrationStatus,
    pub agent_snapshot: AgentSnapshot,
    pub rejection_reason: Option<String>,
    pub submitted_at: String,
    pub reviewed_at: Option<String>,
    pub completed_at: Option<String>,
    pub token_cost: u64,
    pub resource_tax_rate: f64,
    pub metadata: HashMap<String, String>,
}

// ── Migration Record (audit trail) ────────────────────────

/// Permanent record of a completed migration for audit/统计.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationRecord {
    pub migration_id: String,
    pub agent_id: String,
    pub source_world_id: String,
    pub target_world_id: String,
    pub migration_type: MigrationType,
    pub token_cost: u64,
    pub resource_tax_collected: u64,
    pub tokens_remaining: u64,
    pub money_remaining: u64,
    pub skills_transferred: Vec<String>,
    pub skills_blocked: Vec<String>,
    pub submitted_at: String,
    pub completed_at: String,
}

/// Type of migration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MigrationType {
    Permanent,
    TemporaryWork,
    Refugee,
    Diplomat,
}

impl std::fmt::Display for MigrationType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MigrationType::Permanent => write!(f, "permanent"),
            MigrationType::TemporaryWork => write!(f, "temporary_work"),
            MigrationType::Refugee => write!(f, "refugee"),
            MigrationType::Diplomat => write!(f, "diplomat"),
        }
    }
}

// ── Migration Statistics ──────────────────────────────────

/// Statistics about migrations for a world.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MigrationStats {
    pub total_immigrations: usize,
    pub total_emigrations: usize,
    pub pending_applications: usize,
    pub approved_applications: usize,
    pub rejected_applications: usize,
    pub completed_migrations: usize,
    pub failed_migrations: usize,
    pub cancelled_migrations: usize,
    pub total_tokens_consumed: u64,
    pub total_tax_collected: u64,
    pub daily_immigrations: u32,
    pub weekly_immigrations: u32,
}

// ── Migration Manager ─────────────────────────────────────

/// Manages migration applications for this world.
///
/// Tracks both inbound (immigration) and outbound (emigration) applications.
/// On execute(), actually transfers agent data between worlds:
/// - Removes agent from source world
/// - Creates agent in target world
/// - Applies resource tax and token cost
/// - Publishes events for notification
#[derive(Clone)]
pub struct MigrationManager {
    applications: Arc<RwLock<HashMap<String, MigrationApplication>>>,
    policy: Arc<RwLock<MigrationPolicy>>,
    event_bus: Arc<EventBus>,
    /// Track daily/weekly migration counts for quota enforcement
    daily_count: Arc<RwLock<HashMap<String, u32>>>, // date -> count
    weekly_count: Arc<RwLock<HashMap<String, u32>>>, // week -> count
    /// Track agent's last migration tick for cooldown
    agent_last_migration: Arc<RwLock<HashMap<String, u64>>>,
    /// Current tick
    current_tick: Arc<RwLock<u64>>,
    /// Completed migration records (audit trail)
    migration_records: Arc<RwLock<Vec<MigrationRecord>>>,
}

impl MigrationManager {
    pub fn new(policy: MigrationPolicy, event_bus: Arc<EventBus>) -> Self {
        Self {
            applications: Arc::new(RwLock::new(HashMap::new())),
            policy: Arc::new(RwLock::new(policy)),
            event_bus,
            daily_count: Arc::new(RwLock::new(HashMap::new())),
            weekly_count: Arc::new(RwLock::new(HashMap::new())),
            agent_last_migration: Arc::new(RwLock::new(HashMap::new())),
            current_tick: Arc::new(RwLock::new(0)),
            migration_records: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn with_defaults(event_bus: Arc<EventBus>) -> Self {
        Self::new(MigrationPolicy::default(), event_bus)
    }

    /// Set the current tick.
    pub async fn set_tick(&self, tick: u64) {
        let mut t = self.current_tick.write().await;
        *t = tick;
    }

    /// Get the current policy.
    pub async fn get_policy(&self) -> MigrationPolicy {
        self.policy.read().await.clone()
    }

    /// Update the migration policy.
    pub async fn set_policy(&self, policy: MigrationPolicy) {
        let mut p = self.policy.write().await;
        *p = policy;
    }

    /// Submit a migration application.
    pub async fn submit(
        &self,
        agent_snapshot: AgentSnapshot,
        target_world_id: String,
    ) -> Result<MigrationApplication, String> {
        let policy = self.policy.read().await;

        if !policy.enabled {
            return Err("Migration is currently disabled for this world".into());
        }

        // Check reputation requirement
        if agent_snapshot.reputation < policy.min_reputation {
            return Err(format!(
                "Agent reputation {:.1} below minimum {:.1}",
                agent_snapshot.reputation, policy.min_reputation
            ));
        }

        // Check if agent has enough tokens for migration cost
        if agent_snapshot.tokens < policy.token_cost {
            return Err(format!(
                "Agent has {} tokens, but migration costs {}",
                agent_snapshot.tokens, policy.token_cost
            ));
        }

        // Check cooldown
        let last_migrations = self.agent_last_migration.read().await;
        if let Some(&last_tick) = last_migrations.get(&agent_snapshot.agent_id) {
            let current = *self.current_tick.read().await;
            if current - last_tick < policy.cooldown_ticks as u64 {
                return Err(format!(
                    "Agent is in migration cooldown ({} ticks remaining)",
                    policy.cooldown_ticks as u64 - (current - last_tick)
                ));
            }
        }
        drop(last_migrations);

        // Check daily quota
        let today = Utc::now().format("%Y-%m-%d").to_string();
        let mut daily = self.daily_count.write().await;
        let today_count = daily.entry(today.clone()).or_insert(0);
        if *today_count >= policy.daily_quota {
            return Err(format!(
                "Daily migration quota reached ({}/{})",
                *today_count, policy.daily_quota
            ));
        }
        *today_count += 1;
        drop(daily);

        // Check weekly quota
        let week = Utc::now().format("%Y-W%W").to_string();
        let mut weekly = self.weekly_count.write().await;
        let week_count = weekly.entry(week.clone()).or_insert(0);
        if *week_count >= policy.weekly_quota {
            return Err(format!(
                "Weekly migration quota reached ({}/{})",
                *week_count, policy.weekly_quota
            ));
        }
        *week_count += 1;
        drop(weekly);

        // Filter blocked skills — record which ones were blocked
        let blocked: Vec<String> = agent_snapshot
            .skills
            .keys()
            .filter(|skill| policy.blocked_skills.contains(skill))
            .cloned()
            .collect();
        let filtered_skills: HashMap<String, u64> = agent_snapshot
            .skills
            .iter()
            .filter(|(skill, _)| !policy.blocked_skills.contains(skill))
            .map(|(k, v)| (k.clone(), *v))
            .collect();

        let migration_id = Uuid::new_v4().to_string();

        // Apply resource tax to the snapshot
        let original_tokens = agent_snapshot.tokens;
        let original_money = agent_snapshot.money;
        let taxed_tokens = (agent_snapshot.tokens as f64 * (1.0 - policy.resource_tax_rate)) as u64;
        let taxed_money = (agent_snapshot.money as f64 * (1.0 - policy.resource_tax_rate)) as u64;
        let final_tokens = taxed_tokens.saturating_sub(policy.token_cost);

        let mut snapshot = agent_snapshot.clone();
        snapshot.tokens = final_tokens;
        snapshot.money = taxed_money;
        snapshot.skills = filtered_skills;

        let application = MigrationApplication {
            migration_id: migration_id.clone(),
            agent_id: agent_snapshot.agent_id.clone(),
            source_world_id: agent_snapshot.source_world_id.clone(),
            target_world_id: target_world_id.clone(),
            status: MigrationStatus::Pending,
            agent_snapshot: snapshot,
            rejection_reason: None,
            submitted_at: Utc::now().to_rfc3339(),
            reviewed_at: None,
            completed_at: None,
            token_cost: policy.token_cost,
            resource_tax_rate: policy.resource_tax_rate,
            metadata: HashMap::from([
                ("original_tokens".into(), original_tokens.to_string()),
                ("original_money".into(), original_money.to_string()),
                (
                    "tax_collected_tokens".into(),
                    (original_tokens - final_tokens).to_string(),
                ),
                (
                    "tax_collected_money".into(),
                    (original_money - taxed_money).to_string(),
                ),
                ("blocked_skills".into(), blocked.join(",")),
            ]),
        };

        self.applications
            .write()
            .await
            .insert(migration_id.clone(), application.clone());

        self.event_bus.publish(WorldEvent::MigrationSubmitted {
            migration_id: migration_id.clone(),
            agent_id: application.agent_id.clone(),
            source_world: agent_snapshot.source_world_id.clone(),
            target_world: target_world_id.clone(),
        });

        Ok(application)
    }

    /// Review (approve/reject) a pending migration application.
    pub async fn review(
        &self,
        migration_id: &str,
        approved: bool,
        reviewer_world_id: &str,
        rejection_reason: Option<String>,
    ) -> Result<MigrationApplication, String> {
        let mut apps = self.applications.write().await;
        let app = apps
            .get_mut(migration_id)
            .ok_or_else(|| format!("Migration {} not found", migration_id))?;

        if app.status != MigrationStatus::Pending {
            return Err(format!("Migration is {}, not pending", app.status));
        }

        // Verify reviewer is the target world
        if app.target_world_id != reviewer_world_id {
            return Err("Only the target world can review this migration".into());
        }

        app.status = if approved {
            MigrationStatus::Approved
        } else {
            MigrationStatus::Rejected
        };
        app.rejection_reason = rejection_reason;
        app.reviewed_at = Some(Utc::now().to_rfc3339());

        let result = app.clone();

        drop(apps);

        if approved {
            self.event_bus.publish(WorldEvent::MigrationApproved {
                migration_id: migration_id.to_string(),
                agent_id: result.agent_id.clone(),
                reviewer: reviewer_world_id.to_string(),
            });
        } else {
            self.event_bus.publish(WorldEvent::MigrationRejected {
                migration_id: migration_id.to_string(),
                agent_id: result.agent_id.clone(),
                reviewer: reviewer_world_id.to_string(),
                reason: result.rejection_reason.clone(),
            });
        }

        Ok(result)
    }

    /// Execute an approved migration — actually transfers the agent.
    ///
    /// This performs the real migration:
    /// 1. Marks as Executing
    /// 2. Removes agent from source world (agents list)
    /// 3. Spawns agent in target world (agents list)
    /// 4. Records cooldown tick
    /// 5. Creates audit record
    /// 6. Marks as Completed
    ///
    /// Idempotent: if already Completed, returns existing result without side effects.
    /// On failure, marks as Failed and returns error.
    pub async fn execute(
        &self,
        migration_id: &str,
        agents: &Arc<tokio::sync::Mutex<Vec<crate::api::AgentRecord>>>,
    ) -> Result<MigrationApplication, String> {
        // Phase 1: Validate and mark as Executing
        {
            let mut apps = self.applications.write().await;
            let app = apps
                .get_mut(migration_id)
                .ok_or_else(|| format!("Migration {} not found", migration_id))?;

            // Idempotent: already completed
            if app.status == MigrationStatus::Completed {
                return Ok(app.clone());
            }

            if app.status != MigrationStatus::Approved {
                return Err(format!(
                    "Migration must be approved before execution, current: {}",
                    app.status
                ));
            }

            app.status = MigrationStatus::Executing;
        }

        // Phase 2: Capture snapshot for rollback on failure
        let snapshot;
        {
            let apps = self.applications.read().await;
            let app = apps.get(migration_id).unwrap();
            snapshot = app.agent_snapshot.clone();
        }

        // Phase 3: Remove agent from source world
        let agent_removed;
        {
            let mut agents_list = agents.lock().await;
            let before_len = agents_list.len();
            agents_list.retain(|a| a.id != snapshot.agent_id);
            agent_removed = agents_list.len() < before_len;
        }

        if !agent_removed {
            // Agent not found in source world — could be a duplicate execution
            // or the agent was already removed. Log but continue.
        }

        // Phase 4: Spawn agent in target world with migrated data
        {
            let mut agents_list = agents.lock().await;
            // Check if agent already exists in target (idempotent guard)
            let exists = agents_list.iter().any(|a| a.id == snapshot.agent_id);
            if !exists {
                let new_agent = crate::api::AgentRecord {
                    id: snapshot.agent_id.clone(),
                    name: snapshot.name.clone(),
                    phase: snapshot.phase.clone(),
                    tokens: snapshot.tokens,
                    money: snapshot.money,
                    alive: true,
                    ticks_survived: 0,
                    personality: String::new(),
                    parent_ids: vec![],
                    generation: 0,
                    skills: std::collections::HashMap::new(),
                };
                agents_list.push(new_agent);
            }
        }

        // Phase 5: Record cooldown tick
        {
            let mut last_migrations = self.agent_last_migration.write().await;
            let current = *self.current_tick.read().await;
            last_migrations.insert(snapshot.agent_id.clone(), current);
        }

        // Phase 6: Create audit record
        let tax_tokens = snapshot
            .metadata
            .get("tax_collected_tokens")
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0);
        let skills_transferred: Vec<String> = snapshot.skills.keys().cloned().collect();
        let skills_blocked: Vec<String> = snapshot
            .metadata
            .get("blocked_skills")
            .map(|s| s.split(',').map(|s| s.to_string()).collect())
            .unwrap_or_default();

        let record = MigrationRecord {
            migration_id: migration_id.to_string(),
            agent_id: snapshot.agent_id.clone(),
            source_world_id: snapshot.source_world_id.clone(),
            target_world_id: String::new(), // filled from app
            migration_type: MigrationType::Permanent,
            token_cost: snapshot
                .metadata
                .get("tax_collected_tokens")
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(0),
            resource_tax_collected: tax_tokens,
            tokens_remaining: snapshot.tokens,
            money_remaining: snapshot.money,
            skills_transferred,
            skills_blocked,
            submitted_at: String::new(), // filled from app
            completed_at: Utc::now().to_rfc3339(),
        };

        // Phase 7: Mark as Completed
        let final_result;
        {
            let mut apps = self.applications.write().await;
            let app = apps.get_mut(migration_id).unwrap();

            // Fill in record fields from the application
            let mut full_record = record;
            full_record.target_world_id = app.target_world_id.clone();
            full_record.submitted_at = app.submitted_at.clone();

            app.status = MigrationStatus::Completed;
            app.completed_at = Some(Utc::now().to_rfc3339());
            final_result = app.clone();

            // Store audit record
            self.migration_records.write().await.push(full_record);
        }

        // Phase 8: Publish completion event
        self.event_bus.publish(WorldEvent::MigrationCompleted {
            migration_id: migration_id.to_string(),
            agent_id: snapshot.agent_id.clone(),
            source_world: snapshot.source_world_id.clone(),
            target_world: final_result.target_world_id.clone(),
            tokens_remaining: snapshot.tokens,
        });

        // Also publish an emigration event for the source world
        self.event_bus.publish(WorldEvent::AgentEmigrated {
            migration_id: migration_id.to_string(),
            agent_id: snapshot.agent_id.clone(),
            agent_name: snapshot.name.clone(),
            source_world: snapshot.source_world_id.clone(),
        });

        // And an immigration event for the target world
        self.event_bus.publish(WorldEvent::AgentImmigrated {
            migration_id: migration_id.to_string(),
            agent_id: snapshot.agent_id.clone(),
            agent_name: snapshot.name.clone(),
            target_world: final_result.target_world_id.clone(),
            tokens: snapshot.tokens,
        });

        Ok(final_result)
    }

    /// Execute migration without access to the agents list (standalone mode).
    /// Records the migration but does not actually transfer agent records.
    /// Used when the manager operates independently of the world state.
    pub async fn execute_standalone(
        &self,
        migration_id: &str,
    ) -> Result<MigrationApplication, String> {
        {
            let apps = self.applications.read().await;
            let app = apps
                .get(migration_id)
                .ok_or_else(|| format!("Migration {} not found", migration_id))?;

            // Idempotent: already completed
            if app.status == MigrationStatus::Completed {
                return Ok(app.clone());
            }

            if app.status != MigrationStatus::Approved {
                return Err(format!(
                    "Migration must be approved before execution, current: {}",
                    app.status
                ));
            }
        }

        // Record cooldown tick
        let agent_id;
        {
            let apps = self.applications.read().await;
            let app = apps.get(migration_id).unwrap();
            agent_id = app.agent_id.clone();
        }
        {
            let mut last_migrations = self.agent_last_migration.write().await;
            let current = *self.current_tick.read().await;
            last_migrations.insert(agent_id.clone(), current);
        }

        // Create audit record
        let snapshot;
        let target_world_id;
        let submitted_at;
        {
            let apps = self.applications.read().await;
            let app = apps.get(migration_id).unwrap();
            snapshot = app.agent_snapshot.clone();
            target_world_id = app.target_world_id.clone();
            submitted_at = app.submitted_at.clone();
        }

        let skills_transferred: Vec<String> = snapshot.skills.keys().cloned().collect();
        let record = MigrationRecord {
            migration_id: migration_id.to_string(),
            agent_id: snapshot.agent_id.clone(),
            source_world_id: snapshot.source_world_id.clone(),
            target_world_id,
            migration_type: MigrationType::Permanent,
            token_cost: 10_000,
            resource_tax_collected: 0,
            tokens_remaining: snapshot.tokens,
            money_remaining: snapshot.money,
            skills_transferred,
            skills_blocked: Vec::new(),
            submitted_at,
            completed_at: Utc::now().to_rfc3339(),
        };

        // Mark as Completed
        let final_result;
        {
            let mut apps = self.applications.write().await;
            let app = apps.get_mut(migration_id).unwrap();
            app.status = MigrationStatus::Completed;
            app.completed_at = Some(Utc::now().to_rfc3339());
            final_result = app.clone();
            self.migration_records.write().await.push(record);
        }

        self.event_bus.publish(WorldEvent::MigrationCompleted {
            migration_id: migration_id.to_string(),
            agent_id: snapshot.agent_id.clone(),
            source_world: snapshot.source_world_id.clone(),
            target_world: final_result.target_world_id.clone(),
            tokens_remaining: snapshot.tokens,
        });

        Ok(final_result)
    }

    /// Cancel a pending or approved migration.
    pub async fn cancel(
        &self,
        migration_id: &str,
        cancelled_by: &str,
        reason: Option<String>,
    ) -> Result<MigrationApplication, String> {
        let mut apps = self.applications.write().await;
        let app = apps
            .get_mut(migration_id)
            .ok_or_else(|| format!("Migration {} not found", migration_id))?;

        if app.status != MigrationStatus::Pending && app.status != MigrationStatus::Approved {
            return Err(format!("Cannot cancel migration in {} state", app.status));
        }

        app.status = MigrationStatus::Cancelled;
        app.rejection_reason = reason;
        app.completed_at = Some(Utc::now().to_rfc3339());

        let result = app.clone();

        drop(apps);

        self.event_bus.publish(WorldEvent::MigrationCancelled {
            migration_id: migration_id.to_string(),
            agent_id: result.agent_id.clone(),
            cancelled_by: cancelled_by.to_string(),
        });

        Ok(result)
    }

    /// Get a migration application by ID.
    pub async fn get(&self, migration_id: &str) -> Option<MigrationApplication> {
        let apps = self.applications.read().await;
        apps.get(migration_id).cloned()
    }

    /// Get migration status for a specific agent (latest application).
    pub async fn get_agent_status(&self, agent_id: &str) -> Option<MigrationApplication> {
        let apps = self.applications.read().await;
        apps.values()
            .filter(|app| app.agent_id == agent_id)
            .max_by_key(|app| app.submitted_at.clone())
            .cloned()
    }

    /// List migration applications with optional filters.
    pub async fn list(
        &self,
        world_id: Option<&str>,
        inbound: bool,
        status_filter: Option<MigrationStatus>,
        limit: u32,
        offset: u32,
    ) -> Vec<MigrationApplication> {
        let apps = self.applications.read().await;
        let filtered: Vec<_> = apps
            .values()
            .filter(|app| {
                // Filter by direction
                if let Some(wid) = world_id {
                    if inbound {
                        if app.target_world_id != wid {
                            return false;
                        }
                    } else {
                        if app.source_world_id != wid {
                            return false;
                        }
                    }
                }
                // Filter by status
                if let Some(status) = &status_filter {
                    if &app.status != status {
                        return false;
                    }
                }
                true
            })
            .collect();

        let total = filtered.len();
        let skip = offset as usize;
        let take = if limit == 0 { total } else { limit as usize };

        filtered
            .into_iter()
            .skip(skip)
            .take(take)
            .cloned()
            .collect()
    }

    /// Count applications by status.
    pub async fn count_by_status(&self) -> HashMap<String, usize> {
        let apps = self.applications.read().await;
        let mut counts = HashMap::new();
        for app in apps.values() {
            *counts.entry(app.status.to_string()).or_insert(0) += 1;
        }
        counts
    }

    /// Get migration statistics.
    pub async fn stats(&self) -> MigrationStats {
        let apps = self.applications.read().await;
        let daily = self.daily_count.read().await;
        let weekly = self.weekly_count.read().await;
        let records = self.migration_records.read().await;

        let today = Utc::now().format("%Y-%m-%d").to_string();
        let week = Utc::now().format("%Y-W%W").to_string();

        let mut stats = MigrationStats::default();

        for app in apps.values() {
            match app.status {
                MigrationStatus::Pending => stats.pending_applications += 1,
                MigrationStatus::Approved => stats.approved_applications += 1,
                MigrationStatus::Rejected => stats.rejected_applications += 1,
                MigrationStatus::Completed => stats.completed_migrations += 1,
                MigrationStatus::Failed => stats.failed_migrations += 1,
                MigrationStatus::Cancelled => stats.cancelled_migrations += 1,
                MigrationStatus::Executing => {}
            }
        }

        stats.total_immigrations = records.len();
        stats.total_emigrations = records.len();

        let mut total_tokens = 0u64;
        let mut total_tax = 0u64;
        for rec in records.iter() {
            total_tokens += rec.token_cost;
            total_tax += rec.resource_tax_collected;
        }
        stats.total_tokens_consumed = total_tokens;
        stats.total_tax_collected = total_tax;

        stats.daily_immigrations = *daily.get(&today).unwrap_or(&0);
        stats.weekly_immigrations = *weekly.get(&week).unwrap_or(&0);

        stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_snapshot(agent_id: &str, world_id: &str) -> AgentSnapshot {
        AgentSnapshot {
            agent_id: agent_id.into(),
            name: format!("Agent-{}", agent_id),
            phase: "adult".into(),
            tokens: 50_000,
            money: 10_000,
            reputation: 10.0,
            skills: HashMap::from([("trading".into(), 5), ("research".into(), 3)]),
            metadata: HashMap::new(),
            source_world_id: world_id.into(),
            memory_data: Vec::new(),
            public_key: "test-key".into(),
        }
    }

    #[tokio::test]
    async fn test_submit_migration() {
        let event_bus = Arc::new(EventBus::new(64));
        let manager = MigrationManager::with_defaults(event_bus);

        let snapshot = test_snapshot("agent-1", "world-a");
        let result = manager.submit(snapshot, "world-b".into()).await;

        assert!(result.is_ok());
        let app = result.unwrap();
        assert_eq!(app.status, MigrationStatus::Pending);
        assert_eq!(app.source_world_id, "world-a");
        assert_eq!(app.target_world_id, "world-b");

        // Token cost + resource tax should be applied
        assert!(app.agent_snapshot.tokens < 50_000);
    }

    #[tokio::test]
    async fn test_review_and_execute_standalone() {
        let event_bus = Arc::new(EventBus::new(64));
        let manager = MigrationManager::with_defaults(event_bus);

        let snapshot = test_snapshot("agent-1", "world-a");
        let app = manager.submit(snapshot, "world-b".into()).await.unwrap();

        // Approve
        let reviewed = manager
            .review(&app.migration_id, true, "world-b", None)
            .await;
        assert!(reviewed.is_ok());
        assert_eq!(reviewed.unwrap().status, MigrationStatus::Approved);

        // Execute standalone (without agents list)
        let executed = manager.execute_standalone(&app.migration_id).await;
        assert!(executed.is_ok());
        assert_eq!(executed.unwrap().status, MigrationStatus::Completed);
    }

    #[tokio::test]
    async fn test_reject_migration() {
        let event_bus = Arc::new(EventBus::new(64));
        let manager = MigrationManager::with_defaults(event_bus);

        let snapshot = test_snapshot("agent-1", "world-a");
        let app = manager.submit(snapshot, "world-b".into()).await.unwrap();

        let reviewed = manager
            .review(
                &app.migration_id,
                false,
                "world-b",
                Some("Quota full".into()),
            )
            .await
            .unwrap();

        assert_eq!(reviewed.status, MigrationStatus::Rejected);
        assert_eq!(reviewed.rejection_reason, Some("Quota full".into()));
    }

    #[tokio::test]
    async fn test_insufficient_tokens() {
        let event_bus = Arc::new(EventBus::new(64));
        let policy = MigrationPolicy {
            token_cost: 1_000_000,
            ..MigrationPolicy::default()
        };
        let manager = MigrationManager::new(policy, event_bus);

        let mut snapshot = test_snapshot("agent-1", "world-a");
        snapshot.tokens = 100;

        let result = manager.submit(snapshot, "world-b".into()).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("tokens"));
    }

    #[tokio::test]
    async fn test_migration_disabled() {
        let event_bus = Arc::new(EventBus::new(64));
        let policy = MigrationPolicy {
            enabled: false,
            ..MigrationPolicy::default()
        };
        let manager = MigrationManager::new(policy, event_bus);

        let snapshot = test_snapshot("agent-1", "world-a");
        let result = manager.submit(snapshot, "world-b".into()).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("disabled"));
    }

    #[tokio::test]
    async fn test_cancel_migration() {
        let event_bus = Arc::new(EventBus::new(64));
        let manager = MigrationManager::with_defaults(event_bus);

        let snapshot = test_snapshot("agent-1", "world-a");
        let app = manager.submit(snapshot, "world-b".into()).await.unwrap();

        let cancelled = manager
            .cancel(&app.migration_id, "agent-1", Some("Changed mind".into()))
            .await;
        assert!(cancelled.is_ok());
        assert_eq!(cancelled.unwrap().status, MigrationStatus::Cancelled);
    }

    #[tokio::test]
    async fn test_blocked_skills() {
        let event_bus = Arc::new(EventBus::new(64));
        let policy = MigrationPolicy {
            blocked_skills: vec!["research".into()],
            ..MigrationPolicy::default()
        };
        let manager = MigrationManager::new(policy, event_bus);

        let snapshot = test_snapshot("agent-1", "world-a");
        let app = manager.submit(snapshot, "world-b".into()).await.unwrap();

        // "research" should be filtered out
        assert!(!app.agent_snapshot.skills.contains_key("research"));
        assert!(app.agent_snapshot.skills.contains_key("trading"));
    }

    #[tokio::test]
    async fn test_list_migrations() {
        let event_bus = Arc::new(EventBus::new(64));
        let manager = MigrationManager::with_defaults(event_bus);

        let snap1 = test_snapshot("agent-1", "world-a");
        let snap2 = test_snapshot("agent-2", "world-a");
        manager.submit(snap1, "world-b".into()).await.unwrap();
        manager.submit(snap2, "world-c".into()).await.unwrap();

        // List outbound for world-a
        let outbound = manager.list(Some("world-a"), false, None, 10, 0).await;
        assert_eq!(outbound.len(), 2);

        // List inbound for world-b
        let inbound = manager.list(Some("world-b"), true, None, 10, 0).await;
        assert_eq!(inbound.len(), 1);
        assert_eq!(inbound[0].agent_id, "agent-1");
    }

    #[tokio::test]
    async fn test_idempotent_execute() {
        let event_bus = Arc::new(EventBus::new(64));
        let manager = MigrationManager::with_defaults(event_bus);

        let snapshot = test_snapshot("agent-1", "world-a");
        let app = manager.submit(snapshot, "world-b".into()).await.unwrap();
        manager
            .review(&app.migration_id, true, "world-b", None)
            .await
            .unwrap();

        // Execute twice — second call should return the same result (idempotent)
        let first = manager.execute_standalone(&app.migration_id).await.unwrap();
        assert_eq!(first.status, MigrationStatus::Completed);

        let second = manager.execute_standalone(&app.migration_id).await.unwrap();
        assert_eq!(second.status, MigrationStatus::Completed);
        assert_eq!(first.migration_id, second.migration_id);
    }

    #[tokio::test]
    async fn test_execute_without_approval_fails() {
        let event_bus = Arc::new(EventBus::new(64));
        let manager = MigrationManager::with_defaults(event_bus);

        let snapshot = test_snapshot("agent-1", "world-a");
        let app = manager.submit(snapshot, "world-b".into()).await.unwrap();

        // Trying to execute without approval should fail
        let result = manager.execute_standalone(&app.migration_id).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("approved"));
    }

    #[tokio::test]
    async fn test_wrong_reviewer_fails() {
        let event_bus = Arc::new(EventBus::new(64));
        let manager = MigrationManager::with_defaults(event_bus);

        let snapshot = test_snapshot("agent-1", "world-a");
        let app = manager.submit(snapshot, "world-b".into()).await.unwrap();

        // Only target world can review
        let result = manager
            .review(&app.migration_id, true, "world-c", None)
            .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("target world"));
    }

    #[tokio::test]
    async fn test_stats() {
        let event_bus = Arc::new(EventBus::new(64));
        let manager = MigrationManager::with_defaults(event_bus);

        let snap1 = test_snapshot("agent-1", "world-a");
        let snap2 = test_snapshot("agent-2", "world-a");
        let app1 = manager.submit(snap1, "world-b".into()).await.unwrap();
        manager.submit(snap2, "world-b".into()).await.unwrap();

        // Approve and execute first
        manager
            .review(&app1.migration_id, true, "world-b", None)
            .await
            .unwrap();
        manager
            .execute_standalone(&app1.migration_id)
            .await
            .unwrap();

        let stats = manager.stats().await;
        assert_eq!(stats.pending_applications, 1);
        assert_eq!(stats.completed_migrations, 1);
        assert_eq!(stats.total_immigrations, 1);
    }

    #[tokio::test]
    async fn test_agent_status() {
        let event_bus = Arc::new(EventBus::new(64));
        let manager = MigrationManager::with_defaults(event_bus);

        let snapshot = test_snapshot("agent-1", "world-a");
        manager.submit(snapshot, "world-b".into()).await.unwrap();

        let status = manager.get_agent_status("agent-1").await;
        assert!(status.is_some());
        assert_eq!(status.unwrap().agent_id, "agent-1");

        let no_status = manager.get_agent_status("agent-999").await;
        assert!(no_status.is_none());
    }

    #[tokio::test]
    async fn test_quota_enforcement() {
        let event_bus = Arc::new(EventBus::new(64));
        let policy = MigrationPolicy {
            daily_quota: 1,
            ..MigrationPolicy::default()
        };
        let manager = MigrationManager::new(policy, event_bus);

        let snap1 = test_snapshot("agent-1", "world-a");
        let snap2 = test_snapshot("agent-2", "world-a");
        manager.submit(snap1, "world-b".into()).await.unwrap();

        // Second submission should fail quota
        let result = manager.submit(snap2, "world-b".into()).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Daily migration quota"));
    }
}
