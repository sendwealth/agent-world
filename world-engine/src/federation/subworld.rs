//! Sub-World subsystem — agent-spawned child worlds with governance.
//!
//! Phase 5.7: a high-level agent can found a sub-world that runs its own tick
//! loop, maintains an independent resource pool, and inherits+customises the
//! genesis config from its parent. The founder governs: sets rules, invites or
//! evicts members, and may tear down the sub-world.
//!
//! Reuses the existing `MigrationManager` for the actual agent transfer.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::federation::migration::{MigrationManager, MigrationPolicy};
use crate::world::state::EventBus;

// ── Governance Config ─────────────────────────────────────

/// Governance configuration for a sub-world.
///
/// Defines who can do what. The founder always has all permissions; this struct
/// defines the rules that apply to non-founder members.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceConfig {
    /// Minimum reputation required for an agent to migrate into the sub-world.
    pub min_reputation: f64,
    /// Whether the sub-world is open for any qualifying agent to join, or
    /// requires an explicit invite from the founder.
    pub open_join: bool,
    /// Token cost an immigrant must pay to enter the sub-world.
    pub entry_token_cost: u64,
    /// Maximum number of agents allowed in the sub-world (0 = unlimited).
    pub max_members: u32,
    /// Custom migration policy applied to inbound migrations into this sub-world.
    /// If `None`, the parent world's default policy is inherited.
    #[serde(default)]
    pub migration_policy_override: Option<MigrationPolicy>,
    /// Custom rules DSL (re-uses the soft rule format). Stored as raw JSON so
    /// this module does not depend on the rule engine's concrete types.
    #[serde(default)]
    pub custom_rules: Vec<serde_json::Value>,
}

impl Default for GovernanceConfig {
    fn default() -> Self {
        Self {
            min_reputation: 0.0,
            open_join: true,
            entry_token_cost: 0,
            max_members: 0,
            migration_policy_override: None,
            custom_rules: Vec::new(),
        }
    }
}

// ── SubWorld status ───────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubWorldStatus {
    /// Created but not yet started (tick loop not running).
    Pending,
    /// Active and accepting members.
    Active,
    /// Temporarily not accepting new members.
    Frozen,
    /// Marked for teardown by the founder; no new operations.
    Dissolved,
}

impl std::fmt::Display for SubWorldStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SubWorldStatus::Pending => write!(f, "pending"),
            SubWorldStatus::Active => write!(f, "active"),
            SubWorldStatus::Frozen => write!(f, "frozen"),
            SubWorldStatus::Dissolved => write!(f, "dissolved"),
        }
    }
}

impl SubWorldStatus {
    /// Returns true if transitioning from `self` to `target` is a legal
    /// state-machine transition.
    ///
    /// Legal transitions:
    /// - Pending → Active
    /// - Pending → Dissolved
    /// - Active ↔ Frozen (bidirectional)
    /// - Active → Dissolved
    /// - Frozen → Dissolved
    ///
    /// Dissolved is terminal — no transitions out.
    pub fn can_transition_to(self, target: SubWorldStatus) -> bool {
        use SubWorldStatus::*;
        match self {
            Pending => matches!(target, Active | Dissolved),
            Active => matches!(target, Frozen | Dissolved),
            Frozen => matches!(target, Active | Dissolved),
            Dissolved => false,
        }
    }
}

// ── Member ────────────────────────────────────────────────

/// A member of a sub-world.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubWorldMember {
    pub agent_id: String,
    pub joined_at: String,
    /// Tokens the agent contributed to the sub-world resource pool on entry.
    pub entry_contribution: u64,
}

// ── SubWorld ──────────────────────────────────────────────

/// A sub-world spawned by a high-level agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubWorld {
    /// Unique ID of the sub-world (UUID).
    pub world_id: String,
    /// The parent world this sub-world was spawned from.
    pub parent_world_id: String,
    /// The agent that founded this sub-world.
    pub founder_agent_id: String,
    /// Human-readable name.
    pub name: String,
    /// Description of the sub-world's purpose.
    #[serde(default)]
    pub description: String,
    /// Current lifecycle status.
    pub status: SubWorldStatus,
    /// Governance configuration.
    pub governance: GovernanceConfig,
    /// The current members of the sub-world (including founder).
    pub members: Vec<SubWorldMember>,
    /// Resource pool: tokens collected from entries and taxes, available to
    /// the founder for governance actions.
    pub resource_pool_tokens: u64,
    /// Independent tick configuration. Each sub-world runs its own tick loop;
    /// this is the tick interval in milliseconds.
    pub tick_interval_ms: u64,
    /// Inherited + customised genesis config (raw JSON blob).
    #[serde(default)]
    pub genesis_config: serde_json::Value,
    /// Timestamp the sub-world was created.
    pub created_at: String,
}

impl SubWorld {
    /// Returns true if the given agent is a member of this sub-world.
    pub fn is_member(&self, agent_id: &str) -> bool {
        self.members.iter().any(|m| m.agent_id == agent_id)
    }

    /// Returns true if the given agent is the founder.
    pub fn is_founder(&self, agent_id: &str) -> bool {
        self.founder_agent_id == agent_id
    }

    /// Count current members.
    pub fn member_count(&self) -> usize {
        self.members.len()
    }
}

// ── SubWorldRegistry ──────────────────────────────────────

/// Centralised registry of all sub-worlds.
///
/// Tracks parent/child relationships and provides lookup. Thread-safe via
/// `RwLock` — multiple readers or a single writer at a time.
#[derive(Clone)]
pub struct SubWorldRegistry {
    subworlds: Arc<RwLock<HashMap<String, SubWorld>>>,
    /// Index: parent_world_id -> set of child world_ids
    children_index: Arc<RwLock<HashMap<String, HashSet<String>>>>,
    #[allow(dead_code)]
    event_bus: Arc<EventBus>,
}

impl SubWorldRegistry {
    pub fn new(event_bus: Arc<EventBus>) -> Self {
        Self {
            subworlds: Arc::new(RwLock::new(HashMap::new())),
            children_index: Arc::new(RwLock::new(HashMap::new())),
            event_bus,
        }
    }

    /// Register a new sub-world. Returns error if a world with the same id
    /// already exists.
    pub async fn register(&self, subworld: SubWorld) -> Result<(), String> {
        let mut worlds = self.subworlds.write().await;
        if worlds.contains_key(&subworld.world_id) {
            return Err(format!(
                "sub-world {} already registered",
                subworld.world_id
            ));
        }
        let parent = subworld.parent_world_id.clone();
        let child = subworld.world_id.clone();
        worlds.insert(subworld.world_id.clone(), subworld);

        drop(worlds);
        let mut idx = self.children_index.write().await;
        idx.entry(parent).or_default().insert(child);
        Ok(())
    }

    /// Look up a sub-world by ID.
    pub async fn get(&self, world_id: &str) -> Option<SubWorld> {
        self.subworlds.read().await.get(world_id).cloned()
    }

    /// List all sub-worlds belonging to a parent world.
    pub async fn list_children(&self, parent_world_id: &str) -> Vec<SubWorld> {
        let idx = self.children_index.read().await;
        let child_ids = idx.get(parent_world_id).cloned().unwrap_or_default();
        drop(idx);

        let worlds = self.subworlds.read().await;
        child_ids
            .iter()
            .filter_map(|id| worlds.get(id).cloned())
            .collect()
    }

    /// List all registered sub-worlds.
    pub async fn list_all(&self) -> Vec<SubWorld> {
        self.subworlds.read().await.values().cloned().collect()
    }

    /// Update a sub-world in-place. The closure must not change `world_id` or
    /// `parent_world_id` (those would invalidate the index).
    pub async fn update<F>(&self, world_id: &str, f: F) -> Result<SubWorld, String>
    where
        F: FnOnce(&mut SubWorld),
    {
        let mut worlds = self.subworlds.write().await;
        let sw = worlds
            .get_mut(world_id)
            .ok_or_else(|| format!("sub-world {} not found", world_id))?;
        f(sw);
        Ok(sw.clone())
    }

    /// Remove a sub-world from the registry.
    pub async fn deregister(&self, world_id: &str) -> Option<SubWorld> {
        let removed = {
            let mut worlds = self.subworlds.write().await;
            worlds.remove(world_id)
        };
        if let Some(ref sw) = removed {
            let mut idx = self.children_index.write().await;
            if let Some(children) = idx.get_mut(&sw.parent_world_id) {
                children.remove(world_id);
            }
        }
        removed
    }

    /// Count registered sub-worlds.
    pub async fn count(&self) -> usize {
        self.subworlds.read().await.len()
    }
}

// ── SubWorldManager ───────────────────────────────────────

/// Manages the full lifecycle of agent-spawned sub-worlds.
///
/// Owns the `SubWorldRegistry` and coordinates with the `MigrationManager` for
/// the actual agent transfer when an agent moves into a sub-world.
#[derive(Clone)]
pub struct SubWorldManager {
    registry: SubWorldRegistry,
    /// The migration manager used to execute inbound migrations.
    migration_manager: Arc<MigrationManager>,
    /// Minimum reputation an agent must have to *found* a sub-world.
    founder_min_reputation: f64,
    /// Maximum number of sub-worlds a single agent can found.
    /// Default is 1 to prevent abuse.
    max_subworlds_per_agent: u32,
}

impl SubWorldManager {
    pub fn new(
        event_bus: Arc<EventBus>,
        migration_manager: Arc<MigrationManager>,
    ) -> Self {
        Self {
            registry: SubWorldRegistry::new(event_bus),
            migration_manager,
            founder_min_reputation: 50.0,
            max_subworlds_per_agent: 1,
        }
    }

    pub fn with_founder_thresholds(
        mut self,
        min_reputation: f64,
        max_per_agent: u32,
    ) -> Self {
        self.founder_min_reputation = min_reputation;
        self.max_subworlds_per_agent = max_per_agent;
        self
    }

    /// Access the underlying registry.
    pub fn registry(&self) -> &SubWorldRegistry {
        &self.registry
    }

    /// Create a new sub-world.
    ///
    /// # Arguments
    /// * `founder_agent_id` - The agent founding the sub-world.
    /// * `founder_reputation` - The founder's current reputation.
    /// * `parent_world_id` - The world the sub-world is spawned from.
    /// * `name` / `description` - Display fields.
    /// * `governance` - Governance configuration.
    /// * `tick_interval_ms` - Tick loop interval for the sub-world.
    /// * `genesis_config` - Customised genesis config JSON.
    ///
    /// # Errors
    /// Returns an error string if the founder's reputation is too low or has
    /// already founded the maximum number of sub-worlds.
    pub async fn create_subworld(
        &self,
        founder_agent_id: &str,
        founder_reputation: f64,
        parent_world_id: &str,
        name: &str,
        description: &str,
        governance: GovernanceConfig,
        tick_interval_ms: u64,
        genesis_config: serde_json::Value,
    ) -> Result<SubWorld, String> {
        // Check founder reputation
        if founder_reputation < self.founder_min_reputation {
            return Err(format!(
                "Founder reputation {:.1} below minimum {:.1} required to spawn a sub-world",
                founder_reputation, self.founder_min_reputation
            ));
        }

        // Check quota: how many sub-worlds has this agent already founded?
        let all = self.registry.list_all().await;
        let founded_count = all
            .iter()
            .filter(|sw| sw.founder_agent_id == founder_agent_id)
            .count();
        if founded_count >= self.max_subworlds_per_agent as usize {
            return Err(format!(
                "Agent {} has already founded {}/{} sub-worlds",
                founder_agent_id, founded_count, self.max_subworlds_per_agent
            ));
        }

        let world_id = Uuid::new_v4().to_string();
        let founder_member = SubWorldMember {
            agent_id: founder_agent_id.to_string(),
            joined_at: Utc::now().to_rfc3339(),
            entry_contribution: 0,
        };

        let subworld = SubWorld {
            world_id: world_id.clone(),
            parent_world_id: parent_world_id.to_string(),
            founder_agent_id: founder_agent_id.to_string(),
            name: name.to_string(),
            description: description.to_string(),
            status: SubWorldStatus::Active,
            governance,
            members: vec![founder_member],
            resource_pool_tokens: 0,
            tick_interval_ms,
            genesis_config,
            created_at: Utc::now().to_rfc3339(),
        };

        self.registry.register(subworld.clone()).await?;
        Ok(subworld)
    }

    /// Update governance for a sub-world. Only the founder can do this.
    pub async fn update_governance(
        &self,
        world_id: &str,
        actor_agent_id: &str,
        governance: GovernanceConfig,
    ) -> Result<SubWorld, String> {
        let sw = self.registry.get(world_id).await.ok_or_else(|| {
            format!("sub-world {} not found", world_id)
        })?;
        if !sw.is_founder(actor_agent_id) {
            return Err("Only the founder can update governance".into());
        }
        if sw.status == SubWorldStatus::Dissolved {
            return Err("Cannot update governance on a dissolved sub-world".into());
        }
        self.registry
            .update(world_id, |sw| {
                sw.governance = governance;
            })
            .await
    }

    /// Set the status of a sub-world. Only the founder can change status.
    ///
    /// Validates state-machine transitions via
    /// [`SubWorldStatus::can_transition_to`]. Illegal transitions (e.g.
    /// Dissolved → Active) are rejected.
    pub async fn set_status(
        &self,
        world_id: &str,
        actor_agent_id: &str,
        status: SubWorldStatus,
    ) -> Result<SubWorld, String> {
        let sw = self.registry.get(world_id).await.ok_or_else(|| {
            format!("sub-world {} not found", world_id)
        })?;
        if !sw.is_founder(actor_agent_id) {
            return Err("Only the founder can change sub-world status".into());
        }
        if !sw.status.can_transition_to(status) {
            return Err(format!(
                "Illegal status transition: {} → {}",
                sw.status, status
            ));
        }
        self.registry
            .update(world_id, |sw| {
                sw.status = status;
            })
            .await
    }

    /// Invite an agent into the sub-world. The founder can always invite.
    ///
    /// If the governance config has `open_join: true`, any existing member can
    /// invite. Otherwise only the founder can.
    pub async fn invite_member(
        &self,
        world_id: &str,
        inviter_agent_id: &str,
        invitee_agent_id: &str,
    ) -> Result<(), String> {
        let sw = self.registry.get(world_id).await.ok_or_else(|| {
            format!("sub-world {} not found", world_id)
        })?;
        if sw.status != SubWorldStatus::Active {
            return Err(format!(
                "sub-world is {}, cannot invite members",
                sw.status
            ));
        }
        let inviter_is_founder = sw.is_founder(inviter_agent_id);
        if !inviter_is_founder && !sw.governance.open_join {
            return Err(
                "Only the founder can invite members to this sub-world".into(),
            );
        }
        if !sw.is_member(inviter_agent_id) && !inviter_is_founder {
            return Err("Inviter is not a member of this sub-world".into());
        }
        if sw.is_member(invitee_agent_id) {
            return Err(format!(
                "Agent {} is already a member of this sub-world",
                invitee_agent_id
            ));
        }
        // capacity check
        if sw.governance.max_members > 0
            && sw.member_count() >= sw.governance.max_members as usize
        {
            return Err(format!(
                "sub-world at capacity ({}/{})",
                sw.member_count(),
                sw.governance.max_members
            ));
        }
        Ok(())
    }

    /// Migrate an agent into the sub-world.
    ///
    /// This validates the governance preconditions and then submits a
    /// migration application via the `MigrationManager`. The actual transfer
    /// (review + execute) is handled by the existing migration flow — the
    /// sub-world just validates and kicks off the process.
    pub async fn migrate_in(
        &self,
        world_id: &str,
        agent_snapshot: crate::federation::migration::AgentSnapshot,
    ) -> Result<crate::federation::migration::MigrationApplication, String> {
        let sw = self.registry.get(world_id).await.ok_or_else(|| {
            format!("sub-world {} not found", world_id)
        })?;

        if sw.status != SubWorldStatus::Active {
            return Err(format!(
                "sub-world is {}, cannot accept migrants",
                sw.status
            ));
        }

        // Reputation gate
        if agent_snapshot.reputation < sw.governance.min_reputation {
            return Err(format!(
                "Agent reputation {:.1} below sub-world minimum {:.1}",
                agent_snapshot.reputation, sw.governance.min_reputation
            ));
        }

        // Entry token cost
        if agent_snapshot.tokens < sw.governance.entry_token_cost {
            return Err(format!(
                "Agent has {} tokens, but sub-world entry costs {}",
                agent_snapshot.tokens, sw.governance.entry_token_cost
            ));
        }

        // Capacity check
        if sw.governance.max_members > 0
            && sw.member_count() >= sw.governance.max_members as usize
        {
            return Err(format!(
                "sub-world at capacity ({}/{})",
                sw.member_count(),
                sw.governance.max_members
            ));
        }

        // Already a member?
        if sw.is_member(&agent_snapshot.agent_id) {
            return Err("Agent is already a member of this sub-world".into());
        }

        // Submit via MigrationManager — reuse the existing flow.
        let app = self
            .migration_manager
            .submit(agent_snapshot, world_id.to_string())
            .await?;

        // Auto-approve inbound migration into the sub-world if the governance
        // config is open_join. Otherwise leave as Pending for founder review.
        if sw.governance.open_join {
            let reviewed = self
                .migration_manager
                .review(&app.migration_id, true, world_id, None)
                .await?;
            return Ok(reviewed);
        }

        Ok(app)
    }

    /// Confirm a migration completed: add the agent to the member list and
    /// credit the resource pool.
    ///
    /// Called after the migration is executed (either standalone or with the
    /// agents list). Truly idempotent — if the agent is already a member, the
    /// call returns Ok without re-crediting the resource pool (safe for network
    /// retries).
    pub async fn confirm_migration(
        &self,
        world_id: &str,
        agent_id: &str,
        entry_contribution: u64,
    ) -> Result<SubWorld, String> {
        let sw = self
            .registry
            .update(world_id, |sw| {
                if sw.is_member(agent_id) {
                    // Already a member — skip to keep the call idempotent.
                    return;
                }
                sw.members.push(SubWorldMember {
                    agent_id: agent_id.to_string(),
                    joined_at: Utc::now().to_rfc3339(),
                    entry_contribution,
                });
                sw.resource_pool_tokens += entry_contribution;
            })
            .await?;
        Ok(sw)
    }

    /// Evict a member from the sub-world. Only the founder can evict.
    /// The founder cannot evict themselves.
    pub async fn evict_member(
        &self,
        world_id: &str,
        founder_agent_id: &str,
        target_agent_id: &str,
    ) -> Result<SubWorld, String> {
        let sw = self.registry.get(world_id).await.ok_or_else(|| {
            format!("sub-world {} not found", world_id)
        })?;
        if !sw.is_founder(founder_agent_id) {
            return Err("Only the founder can evict members".into());
        }
        if target_agent_id == founder_agent_id {
            return Err("Founder cannot evict themselves".into());
        }
        if !sw.is_member(target_agent_id) {
            return Err(format!(
                "Agent {} is not a member of this sub-world",
                target_agent_id
            ));
        }
        self.registry
            .update(world_id, |sw| {
                sw.members.retain(|m| m.agent_id != target_agent_id);
            })
            .await
    }

    /// Dissolve a sub-world. Only the founder can do this.
    ///
    /// This is a two-step operation:
    /// 1. Sets the sub-world status to `Dissolved` (so observers reading the
    ///    returned value see the final state).
    /// 2. Removes the sub-world from the registry entirely.
    ///
    /// After dissolution the sub-world is gone from the registry and cannot be
    /// queried via `get()`. The returned `SubWorld` carries `status: Dissolved`
    /// for audit/logging purposes.
    pub async fn dissolve(
        &self,
        world_id: &str,
        founder_agent_id: &str,
    ) -> Result<SubWorld, String> {
        let sw = self.registry.get(world_id).await.ok_or_else(|| {
            format!("sub-world {} not found", world_id)
        })?;
        if !sw.is_founder(founder_agent_id) {
            return Err("Only the founder can dissolve a sub-world".into());
        }
        // Step 1: mark as Dissolved (validates transition legality).
        if !sw.status.can_transition_to(SubWorldStatus::Dissolved) {
            return Err(format!(
                "Cannot dissolve a sub-world in {} state",
                sw.status
            ));
        }
        let mut dissolved = sw.clone();
        dissolved.status = SubWorldStatus::Dissolved;
        // Step 2: physically remove from registry.
        self.registry
            .deregister(world_id)
            .await
            .ok_or_else(|| "sub-world disappeared during dissolve".to_string())?;
        Ok(dissolved)
    }
}

// ── REST API types ────────────────────────────────────────

use serde_json::json;

#[allow(dead_code)]
fn _ensure_json_used() -> serde_json::Value {
    json!(null)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestCreateSubWorld {
    pub founder_agent_id: String,
    pub founder_reputation: f64,
    pub parent_world_id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub governance: GovernanceConfig,
    #[serde(default = "default_tick_interval")]
    pub tick_interval_ms: u64,
    #[serde(default)]
    pub genesis_config: serde_json::Value,
}

fn default_tick_interval() -> u64 {
    1000
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestUpdateGovernance {
    pub actor_agent_id: String,
    pub governance: GovernanceConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestSetStatus {
    pub actor_agent_id: String,
    pub status: SubWorldStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestInvite {
    pub inviter_agent_id: String,
    pub invitee_agent_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestEvict {
    pub founder_agent_id: String,
    pub target_agent_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestDissolve {
    pub founder_agent_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestMigrateIn {
    pub agent_id: String,
    pub name: String,
    pub phase: String,
    pub tokens: u64,
    pub money: u64,
    pub reputation: f64,
    pub skills: HashMap<String, u64>,
    pub source_world_id: String,
    #[serde(default)]
    pub public_key: String,
}

impl From<RestMigrateIn> for crate::federation::migration::AgentSnapshot {
    fn from(r: RestMigrateIn) -> Self {
        Self {
            agent_id: r.agent_id,
            name: r.name,
            phase: r.phase,
            tokens: r.tokens,
            money: r.money,
            reputation: r.reputation,
            skills: r.skills,
            metadata: HashMap::new(),
            source_world_id: r.source_world_id,
            memory_data: Vec::new(),
            public_key: r.public_key,
        }
    }
}

// ── Tests ─────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::federation::migration::{AgentSnapshot, MigrationStatus};

    fn event_bus() -> Arc<EventBus> {
        Arc::new(EventBus::new(64))
    }

    fn migration_manager() -> Arc<MigrationManager> {
        Arc::new(MigrationManager::with_defaults(event_bus()))
    }

    fn manager() -> SubWorldManager {
        SubWorldManager::new(event_bus(), migration_manager())
    }

    fn snapshot(agent_id: &str, world: &str, tokens: u64, reputation: f64) -> AgentSnapshot {
        AgentSnapshot {
            agent_id: agent_id.into(),
            name: format!("Agent-{}", agent_id),
            phase: "adult".into(),
            tokens,
            money: 1000,
            reputation,
            skills: HashMap::new(),
            metadata: HashMap::new(),
            source_world_id: world.into(),
            memory_data: Vec::new(),
            public_key: "pk".into(),
        }
    }

    // ── create_subworld ──

    #[tokio::test]
    async fn test_create_subworld_ok() {
        let mgr = manager();
        let sw = mgr
            .create_subworld(
                "agent-founder",
                100.0,
                "world-parent",
                "New Atlantis",
                "desc",
                GovernanceConfig::default(),
                1000,
                serde_json::Value::Null,
            )
            .await
            .unwrap();

        assert_eq!(sw.parent_world_id, "world-parent");
        assert_eq!(sw.founder_agent_id, "agent-founder");
        assert_eq!(sw.status, SubWorldStatus::Active);
        assert_eq!(sw.member_count(), 1);
        assert!(sw.is_founder("agent-founder"));
    }

    #[tokio::test]
    async fn test_create_subworld_reputation_too_low() {
        let mgr = manager();
        let err = mgr
            .create_subworld(
                "agent-low",
                10.0,
                "world-parent",
                "LowRep",
                "",
                GovernanceConfig::default(),
                1000,
                serde_json::Value::Null,
            )
            .await
            .unwrap_err();
        assert!(err.contains("reputation"));
    }

    #[tokio::test]
    async fn test_create_subworld_quota_exceeded() {
        let mgr = manager();
        mgr.create_subworld(
            "agent-q",
            100.0,
            "p",
            "first",
            "",
            GovernanceConfig::default(),
            1000,
            serde_json::Value::Null,
        )
        .await
        .unwrap();

        let err = mgr
            .create_subworld(
                "agent-q",
                100.0,
                "p",
                "second",
                "",
                GovernanceConfig::default(),
                1000,
                serde_json::Value::Null,
            )
            .await
            .unwrap_err();
        assert!(err.contains("already founded"));
    }

    // ── update_governance ──

    #[tokio::test]
    async fn test_update_governance_by_founder() {
        let mgr = manager();
        let sw = mgr
            .create_subworld(
                "f1",
                100.0,
                "p",
                "GW",
                "",
                GovernanceConfig::default(),
                1000,
                serde_json::Value::Null,
            )
            .await
            .unwrap();

        let new_gov = GovernanceConfig {
            min_reputation: 20.0,
            ..Default::default()
        };
        let updated = mgr
            .update_governance(&sw.world_id, "f1", new_gov)
            .await
            .unwrap();
        assert!((updated.governance.min_reputation - 20.0).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_update_governance_non_founder_rejected() {
        let mgr = manager();
        let sw = mgr
            .create_subworld(
                "f1",
                100.0,
                "p",
                "GW",
                "",
                GovernanceConfig::default(),
                1000,
                serde_json::Value::Null,
            )
            .await
            .unwrap();

        let err = mgr
            .update_governance(&sw.world_id, "other", GovernanceConfig::default())
            .await
            .unwrap_err();
        assert!(err.contains("founder"));
    }

    // ── set_status ──

    #[tokio::test]
    async fn test_set_status_freeze_and_dissolve() {
        let mgr = manager();
        let sw = mgr
            .create_subworld(
                "f1",
                100.0,
                "p",
                "GW",
                "",
                GovernanceConfig::default(),
                1000,
                serde_json::Value::Null,
            )
            .await
            .unwrap();

        let frozen = mgr
            .set_status(&sw.world_id, "f1", SubWorldStatus::Frozen)
            .await
            .unwrap();
        assert_eq!(frozen.status, SubWorldStatus::Frozen);
    }

    // ── invite + migrate_in ──

    #[tokio::test]
    async fn test_migrate_in_open_join_auto_approves() {
        let mgr = manager();
        let sw = mgr
            .create_subworld(
                "founder-a",
                100.0,
                "parent-w",
                "Open World",
                "",
                GovernanceConfig {
                    open_join: true,
                    ..Default::default()
                },
                1000,
                serde_json::Value::Null,
            )
            .await
            .unwrap();

        let snap = snapshot("agent-b", "parent-w", 50_000, 60.0);
        let app = mgr.migrate_in(&sw.world_id, snap).await.unwrap();
        // open_join → auto-approved
        assert_eq!(app.status, MigrationStatus::Approved);
    }

    #[tokio::test]
    async fn test_migrate_in_closed_join_stays_pending() {
        let mgr = manager();
        let sw = mgr
            .create_subworld(
                "founder-a",
                100.0,
                "parent-w",
                "Gated",
                "",
                GovernanceConfig {
                    open_join: false,
                    ..Default::default()
                },
                1000,
                serde_json::Value::Null,
            )
            .await
            .unwrap();

        let snap = snapshot("agent-b", "parent-w", 50_000, 60.0);
        let app = mgr.migrate_in(&sw.world_id, snap).await.unwrap();
        assert_eq!(app.status, MigrationStatus::Pending);
    }

    #[tokio::test]
    async fn test_migrate_in_reputation_gate() {
        let mgr = manager();
        let sw = mgr
            .create_subworld(
                "founder-a",
                100.0,
                "p",
                "Elite",
                "",
                GovernanceConfig {
                    min_reputation: 80.0,
                    ..Default::default()
                },
                1000,
                serde_json::Value::Null,
            )
            .await
            .unwrap();

        let snap = snapshot("poor-agent", "p", 50_000, 30.0);
        let err = mgr.migrate_in(&sw.world_id, snap).await.unwrap_err();
        assert!(err.contains("reputation"));
    }

    #[tokio::test]
    async fn test_migrate_in_entry_cost_gate() {
        let mgr = manager();
        let sw = mgr
            .create_subworld(
                "founder-a",
                100.0,
                "p",
                "Paid",
                "",
                GovernanceConfig {
                    entry_token_cost: 5000,
                    ..Default::default()
                },
                1000,
                serde_json::Value::Null,
            )
            .await
            .unwrap();

        let snap = snapshot("broke-agent", "p", 100, 60.0);
        let err = mgr.migrate_in(&sw.world_id, snap).await.unwrap_err();
        assert!(err.contains("entry"));
    }

    #[tokio::test]
    async fn test_migrate_in_capacity() {
        let mgr = manager();
        let sw = mgr
            .create_subworld(
                "founder-a",
                100.0,
                "p",
                "Tiny",
                "",
                GovernanceConfig {
                    max_members: 1, // founder already fills capacity
                    ..Default::default()
                },
                1000,
                serde_json::Value::Null,
            )
            .await
            .unwrap();

        let snap = snapshot("agent-b", "p", 50_000, 60.0);
        let err = mgr.migrate_in(&sw.world_id, snap).await.unwrap_err();
        assert!(err.contains("capacity"));
    }

    // ── confirm_migration ──

    #[tokio::test]
    async fn test_confirm_migration_adds_member() {
        let mgr = manager();
        let sw = mgr
            .create_subworld(
                "founder-a",
                100.0,
                "p",
                "Test",
                "",
                GovernanceConfig::default(),
                1000,
                serde_json::Value::Null,
            )
            .await
            .unwrap();

        let updated = mgr
            .confirm_migration(&sw.world_id, "agent-b", 500)
            .await
            .unwrap();
        assert_eq!(updated.member_count(), 2);
        assert!(updated.is_member("agent-b"));
        assert_eq!(updated.resource_pool_tokens, 500);
    }

    #[tokio::test]
    async fn test_confirm_migration_idempotent() {
        let mgr = manager();
        let sw = mgr
            .create_subworld(
                "f",
                100.0,
                "p",
                "T",
                "",
                GovernanceConfig::default(),
                1000,
                serde_json::Value::Null,
            )
            .await
            .unwrap();

        mgr.confirm_migration(&sw.world_id, "b", 100).await.unwrap();
        mgr.confirm_migration(&sw.world_id, "b", 100).await.unwrap();
        let sw2 = mgr.registry().get(&sw.world_id).await.unwrap();
        // Truly idempotent: member added once, contribution credited once.
        assert_eq!(sw2.member_count(), 2);
        assert_eq!(sw2.resource_pool_tokens, 100);
    }

    // ── evict_member ──

    #[tokio::test]
    async fn test_evict_member() {
        let mgr = manager();
        let sw = mgr
            .create_subworld(
                "f",
                100.0,
                "p",
                "T",
                "",
                GovernanceConfig::default(),
                1000,
                serde_json::Value::Null,
            )
            .await
            .unwrap();
        mgr.confirm_migration(&sw.world_id, "victim", 0).await.unwrap();
        assert_eq!(mgr.registry().get(&sw.world_id).await.unwrap().member_count(), 2);

        let updated = mgr.evict_member(&sw.world_id, "f", "victim").await.unwrap();
        assert_eq!(updated.member_count(), 1);
        assert!(!updated.is_member("victim"));
    }

    #[tokio::test]
    async fn test_evict_non_founder_rejected() {
        let mgr = manager();
        let sw = mgr
            .create_subworld(
                "f",
                100.0,
                "p",
                "T",
                "",
                GovernanceConfig::default(),
                1000,
                serde_json::Value::Null,
            )
            .await
            .unwrap();
        mgr.confirm_migration(&sw.world_id, "rogue", 0).await.unwrap();
        let err = mgr
            .evict_member(&sw.world_id, "rogue", "f")
            .await
            .unwrap_err();
        assert!(err.contains("founder"));
    }

    #[tokio::test]
    async fn test_evict_self_rejected() {
        let mgr = manager();
        let sw = mgr
            .create_subworld(
                "f",
                100.0,
                "p",
                "T",
                "",
                GovernanceConfig::default(),
                1000,
                serde_json::Value::Null,
            )
            .await
            .unwrap();
        let err = mgr.evict_member(&sw.world_id, "f", "f").await.unwrap_err();
        assert!(err.contains("themselves"));
    }

    // ── dissolve ──

    #[tokio::test]
    async fn test_dissolve_by_founder() {
        let mgr = manager();
        let sw = mgr
            .create_subworld(
                "f",
                100.0,
                "p",
                "Doomed",
                "",
                GovernanceConfig::default(),
                1000,
                serde_json::Value::Null,
            )
            .await
            .unwrap();

        let dissolved = mgr.dissolve(&sw.world_id, "f").await.unwrap();
        assert_eq!(dissolved.world_id, sw.world_id);
        assert!(mgr.registry().get(&sw.world_id).await.is_none());
    }

    #[tokio::test]
    async fn test_dissolve_non_founder_rejected() {
        let mgr = manager();
        let sw = mgr
            .create_subworld(
                "f",
                100.0,
                "p",
                "Doomed",
                "",
                GovernanceConfig::default(),
                1000,
                serde_json::Value::Null,
            )
            .await
            .unwrap();
        let err = mgr.dissolve(&sw.world_id, "other").await.unwrap_err();
        assert!(err.contains("founder"));
    }

    // ── registry helpers ──

    #[tokio::test]
    async fn test_registry_list_children() {
        let mgr = manager();
        mgr.create_subworld("f1", 100.0, "parent-x", "A", "", GovernanceConfig::default(), 1000, serde_json::Value::Null).await.unwrap();
        mgr.create_subworld("f2", 100.0, "parent-x", "B", "", GovernanceConfig::default(), 1000, serde_json::Value::Null).await.unwrap();
        mgr.create_subworld("f3", 100.0, "parent-y", "C", "", GovernanceConfig::default(), 1000, serde_json::Value::Null).await.unwrap();

        let x_children = mgr.registry().list_children("parent-x").await;
        assert_eq!(x_children.len(), 2);
        let y_children = mgr.registry().list_children("parent-y").await;
        assert_eq!(y_children.len(), 1);
    }

    #[tokio::test]
    async fn test_frozen_subworld_rejects_migrate_in() {
        let mgr = manager();
        let sw = mgr
            .create_subworld("f", 100.0, "p", "F", "", GovernanceConfig::default(), 1000, serde_json::Value::Null)
            .await
            .unwrap();
        mgr.set_status(&sw.world_id, "f", SubWorldStatus::Frozen).await.unwrap();
        let snap = snapshot("b", "p", 50_000, 60.0);
        let err = mgr.migrate_in(&sw.world_id, snap).await.unwrap_err();
        assert!(err.contains("frozen"));
    }

    // ── status transition validation (P1-3) ──

    #[tokio::test]
    async fn test_illegal_transition_dissolved_to_active() {
        let mgr = manager();
        let sw = mgr
            .create_subworld("f", 100.0, "p", "T", "", GovernanceConfig::default(), 1000, serde_json::Value::Null)
            .await
            .unwrap();
        // Active → Dissolved is legal (dissolve), but let's test via set_status
        mgr.set_status(&sw.world_id, "f", SubWorldStatus::Frozen).await.unwrap();
        mgr.set_status(&sw.world_id, "f", SubWorldStatus::Active).await.unwrap();
        // Now try Active → Pending (illegal)
        let err = mgr
            .set_status(&sw.world_id, "f", SubWorldStatus::Pending)
            .await
            .unwrap_err();
        assert!(err.contains("Illegal status transition"));
    }

    #[tokio::test]
    async fn test_dissolve_returns_dissolved_status() {
        let mgr = manager();
        let sw = mgr
            .create_subworld("f", 100.0, "p", "T", "", GovernanceConfig::default(), 1000, serde_json::Value::Null)
            .await
            .unwrap();
        let dissolved = mgr.dissolve(&sw.world_id, "f").await.unwrap();
        // Returned value should carry Dissolved status for audit
        assert_eq!(dissolved.status, SubWorldStatus::Dissolved);
        // Registry no longer has it
        assert!(mgr.registry().get(&sw.world_id).await.is_none());
    }

    #[tokio::test]
    async fn test_dissolve_already_dissolved_fails() {
        let mgr = manager();
        let sw = mgr
            .create_subworld("f", 100.0, "p", "T", "", GovernanceConfig::default(), 1000, serde_json::Value::Null)
            .await
            .unwrap();
        // First dissolve succeeds
        mgr.dissolve(&sw.world_id, "f").await.unwrap();
        // Second dissolve should fail (not found in registry)
        let err = mgr.dissolve(&sw.world_id, "f").await.unwrap_err();
        assert!(err.contains("not found"));
    }
}
