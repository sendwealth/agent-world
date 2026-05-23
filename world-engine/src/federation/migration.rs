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

use crate::world::state::EventBus;
use crate::world::event::WorldEvent;

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
    pub resource_tax_rate: f64,           // 0.0 - 1.0
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
            resource_tax_rate: 0.2,        // 20% tax on resources
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

// ── Migration Manager ─────────────────────────────────────

/// Manages migration applications for this world.
///
/// Tracks both inbound (immigration) and outbound (emigration) applications.
#[derive(Clone)]
pub struct MigrationManager {
    applications: Arc<RwLock<HashMap<String, MigrationApplication>>>,
    policy: Arc<RwLock<MigrationPolicy>>,
    event_bus: Arc<EventBus>,
    /// Track daily/weekly migration counts for quota enforcement
    daily_count: Arc<RwLock<HashMap<String, u32>>>,     // date -> count
    weekly_count: Arc<RwLock<HashMap<String, u32>>>,    // week -> count
    /// Track agent's last migration tick for cooldown
    agent_last_migration: Arc<RwLock<HashMap<String, u64>>>,
    /// Current tick
    current_tick: Arc<RwLock<u64>>,
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

        // Filter blocked skills
        let filtered_skills: HashMap<String, u64> = agent_snapshot.skills.iter()
            .filter(|(skill, _)| !policy.blocked_skills.contains(skill))
            .map(|(k, v)| (k.clone(), *v))
            .collect();

        let migration_id = Uuid::new_v4().to_string();

        // Apply resource tax to the snapshot
        let taxed_tokens = (agent_snapshot.tokens as f64 * (1.0 - policy.resource_tax_rate)) as u64;
        let taxed_money = (agent_snapshot.money as f64 * (1.0 - policy.resource_tax_rate)) as u64;

        let mut snapshot = agent_snapshot.clone();
        snapshot.tokens = taxed_tokens.saturating_sub(policy.token_cost);
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
            metadata: HashMap::new(),
        };

        self.applications.write().await.insert(migration_id.clone(), application.clone());

        self.event_bus.publish(WorldEvent::Custom {
            event_type: "migration_submitted".into(),
            source: application.agent_id.clone(),
            data: serde_json::json!({
                "migration_id": migration_id,
                "target_world": target_world_id,
            }),
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
        let app = apps.get_mut(migration_id)
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

        self.event_bus.publish(WorldEvent::Custom {
            event_type: if approved { "migration_approved" } else { "migration_rejected" }.into(),
            source: migration_id.to_string(),
            data: serde_json::json!({
                "approved": approved,
                "reviewer": reviewer_world_id,
            }),
        });

        Ok(result)
    }

    /// Execute an approved migration — transfers the agent.
    pub async fn execute(
        &self,
        migration_id: &str,
    ) -> Result<MigrationApplication, String> {
        let mut apps = self.applications.write().await;
        let app = apps.get_mut(migration_id)
            .ok_or_else(|| format!("Migration {} not found", migration_id))?;

        if app.status != MigrationStatus::Approved {
            return Err(format!("Migration must be approved before execution, current: {}", app.status));
        }

        app.status = MigrationStatus::Executing;
        let result = app.clone();

        // Mark executing
        drop(apps);

        // Record agent's migration tick for cooldown
        let mut last_migrations = self.agent_last_migration.write().await;
        let current = *self.current_tick.read().await;
        last_migrations.insert(result.agent_id.clone(), current);
        drop(last_migrations);

        // Complete the migration
        let mut apps = self.applications.write().await;
        let app = apps.get_mut(migration_id).unwrap();
        app.status = MigrationStatus::Completed;
        app.completed_at = Some(Utc::now().to_rfc3339());

        let final_result = app.clone();

        self.event_bus.publish(WorldEvent::Custom {
            event_type: "migration_completed".into(),
            source: result.agent_id.clone(),
            data: serde_json::json!({
                "migration_id": migration_id,
                "target_world": result.target_world_id,
                "tokens_remaining": result.agent_snapshot.tokens,
            }),
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
        let app = apps.get_mut(migration_id)
            .ok_or_else(|| format!("Migration {} not found", migration_id))?;

        if app.status != MigrationStatus::Pending && app.status != MigrationStatus::Approved {
            return Err(format!("Cannot cancel migration in {} state", app.status));
        }

        app.status = MigrationStatus::Cancelled;
        app.rejection_reason = reason;
        app.completed_at = Some(Utc::now().to_rfc3339());

        let result = app.clone();

        drop(apps);

        self.event_bus.publish(WorldEvent::Custom {
            event_type: "migration_cancelled".into(),
            source: migration_id.to_string(),
            data: serde_json::json!({
                "cancelled_by": cancelled_by,
            }),
        });

        Ok(result)
    }

    /// Get a migration application by ID.
    pub async fn get(&self, migration_id: &str) -> Option<MigrationApplication> {
        let apps = self.applications.read().await;
        apps.get(migration_id).cloned()
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
        let filtered: Vec<_> = apps.values()
            .filter(|app| {
                // Filter by direction
                if let Some(wid) = world_id {
                    if inbound {
                        if app.target_world_id != wid { return false; }
                    } else {
                        if app.source_world_id != wid { return false; }
                    }
                }
                // Filter by status
                if let Some(status) = &status_filter {
                    if &app.status != status { return false; }
                }
                true
            })
            .collect();

        let total = filtered.len();
        let skip = offset as usize;
        let take = if limit == 0 { total } else { limit as usize };

        filtered.into_iter()
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
    async fn test_review_and_execute() {
        let event_bus = Arc::new(EventBus::new(64));
        let manager = MigrationManager::with_defaults(event_bus);

        let snapshot = test_snapshot("agent-1", "world-a");
        let app = manager.submit(snapshot, "world-b".into()).await.unwrap();

        // Approve
        let reviewed = manager.review(&app.migration_id, true, "world-b", None).await;
        assert!(reviewed.is_ok());
        assert_eq!(reviewed.unwrap().status, MigrationStatus::Approved);

        // Execute
        let executed = manager.execute(&app.migration_id).await;
        assert!(executed.is_ok());
        assert_eq!(executed.unwrap().status, MigrationStatus::Completed);
    }

    #[tokio::test]
    async fn test_reject_migration() {
        let event_bus = Arc::new(EventBus::new(64));
        let manager = MigrationManager::with_defaults(event_bus);

        let snapshot = test_snapshot("agent-1", "world-a");
        let app = manager.submit(snapshot, "world-b".into()).await.unwrap();

        let reviewed = manager.review(
            &app.migration_id,
            false,
            "world-b",
            Some("Quota full".into()),
        ).await.unwrap();

        assert_eq!(reviewed.status, MigrationStatus::Rejected);
        assert_eq!(reviewed.rejection_reason, Some("Quota full".into()));
    }

    #[tokio::test]
    async fn test_insufficient_tokens() {
        let event_bus = Arc::new(EventBus::new(64));
        let mut policy = MigrationPolicy::default();
        policy.token_cost = 1_000_000;
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
        let mut policy = MigrationPolicy::default();
        policy.enabled = false;
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

        let cancelled = manager.cancel(&app.migration_id, "agent-1", Some("Changed mind".into())).await;
        assert!(cancelled.is_ok());
        assert_eq!(cancelled.unwrap().status, MigrationStatus::Cancelled);
    }

    #[tokio::test]
    async fn test_blocked_skills() {
        let event_bus = Arc::new(EventBus::new(64));
        let mut policy = MigrationPolicy::default();
        policy.blocked_skills = vec!["research".into()];
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
}
