use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tracing::{error, info, warn};

use crate::world::EventBus;
use crate::world::event::WorldEvent;

// ── Genesis Config ────────────────────────────────────────

/// Full genesis.yaml world configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct GenesisConfig {
    #[serde(default)]
    pub world: WorldConfig,
    #[serde(default)]
    pub economy: EconomyConfig,
    #[serde(default)]
    pub lifecycle: LifecycleConfig,
    #[serde(default)]
    pub evolution: EvolutionConfig,
    #[serde(default)]
    pub a2a: A2aConfig,
    #[serde(default)]
    pub survival: SurvivalConfig,
    #[serde(default)]
    pub market: MarketConfig,
    #[serde(default)]
    pub safety: SafetyConfig,
    #[serde(default)]
    pub trust: TrustConfigSection,
    #[serde(default)]
    pub mentorship: MentorshipConfigSection,
    #[serde(default)]
    pub inheritance: InheritanceConfigSection,
    #[serde(default)]
    pub migration: MigrationConfigSection,
    #[serde(default)]
    pub federation: FederationConfigSection,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorldConfig {
    #[serde(default = "default_world_name")]
    pub name: String,
    #[serde(default = "default_tick_interval_ms")]
    pub tick_interval_ms: u64,
    #[serde(default = "default_max_agents")]
    pub max_agents: u32,
}

impl Default for WorldConfig {
    fn default() -> Self {
        Self {
            name: default_world_name(),
            tick_interval_ms: default_tick_interval_ms(),
            max_agents: default_max_agents(),
        }
    }
}

fn default_world_name() -> String { "agent-world-v1".to_string() }
fn default_tick_interval_ms() -> u64 { 1000 }
fn default_max_agents() -> u32 { 10 }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EconomyConfig {
    #[serde(default = "default_initial_tokens")]
    pub initial_tokens: u64,
    #[serde(default = "default_think_cost_per_token")]
    pub think_cost_per_token: u64,
    #[serde(default = "default_memory_cost_per_kb")]
    pub memory_cost_per_kb: f64,
    #[serde(default = "default_communicate_cost")]
    pub communicate_cost: u64,
    #[serde(default)]
    pub initial_money: u64,
    #[serde(default = "default_token_price")]
    pub token_price: u64,
    #[serde(default = "default_interest_rate")]
    pub interest_rate: f64,
}

impl Default for EconomyConfig {
    fn default() -> Self {
        Self {
            initial_tokens: default_initial_tokens(),
            think_cost_per_token: default_think_cost_per_token(),
            memory_cost_per_kb: default_memory_cost_per_kb(),
            communicate_cost: default_communicate_cost(),
            initial_money: 0,
            token_price: default_token_price(),
            interest_rate: default_interest_rate(),
        }
    }
}

fn default_initial_tokens() -> u64 { 100_000 }
fn default_think_cost_per_token() -> u64 { 1 }
fn default_memory_cost_per_kb() -> f64 { 0.1 }
fn default_communicate_cost() -> u64 { 10 }
fn default_token_price() -> u64 { 100 }
fn default_interest_rate() -> f64 { 0.001 }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LifecycleConfig {
    #[serde(default = "default_birth_tokens")]
    pub birth_tokens: u64,
    #[serde(default = "default_childhood_ticks")]
    pub childhood_ticks: u64,
    #[serde(default = "default_adult_ticks")]
    pub adult_ticks: u64,
    #[serde(default = "default_elder_ticks")]
    pub elder_ticks: u64,
    #[serde(default = "default_death_grace_ticks")]
    pub death_grace_ticks: u64,
}

impl Default for LifecycleConfig {
    fn default() -> Self {
        Self {
            birth_tokens: default_birth_tokens(),
            childhood_ticks: default_childhood_ticks(),
            adult_ticks: default_adult_ticks(),
            elder_ticks: default_elder_ticks(),
            death_grace_ticks: default_death_grace_ticks(),
        }
    }
}

fn default_birth_tokens() -> u64 { 100_000 }
fn default_childhood_ticks() -> u64 { 100 }
fn default_adult_ticks() -> u64 { 1000 }
fn default_elder_ticks() -> u64 { 200 }
fn default_death_grace_ticks() -> u64 { 10 }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EvolutionConfig {
    #[serde(default = "default_skill_max_level")]
    pub skill_max_level: u32,
    #[serde(default = "default_mutation_rate")]
    pub mutation_rate: f64,
    #[serde(default = "default_inheritance_ratio")]
    pub inheritance_ratio: f64,
    #[serde(default = "default_evaluation_interval")]
    pub evaluation_interval: u64,
    #[serde(default = "default_inactivity_threshold")]
    pub inactivity_threshold: u64,
    #[serde(default = "default_passive_xp_per_tick")]
    pub passive_xp_per_tick: f64,
    // Offspring mutation parameters
    #[serde(default = "default_offspring_mutation_rate")]
    pub offspring_mutation_rate: f64,
    #[serde(default = "default_max_offspring_mutations")]
    pub max_offspring_mutations: usize,
    #[serde(default = "default_personality_dimensions")]
    pub personality_dimensions: usize,
    #[serde(default = "default_personality_shift_magnitude")]
    pub personality_shift_magnitude: f64,
    #[serde(default = "default_skill_level_jump_range")]
    pub skill_level_jump_range: u32,
    #[serde(default = "default_skill_level_drop_range")]
    pub skill_level_drop_range: u32,
    #[serde(default = "default_env_pressure_multiplier")]
    pub env_pressure_multiplier: f64,
    #[serde(default = "default_heritable_strengthen_chance")]
    pub heritable_strengthen_chance: f64,
    #[serde(default = "default_heritable_disappear_chance")]
    pub heritable_disappear_chance: f64,
}

impl Default for EvolutionConfig {
    fn default() -> Self {
        Self {
            skill_max_level: default_skill_max_level(),
            mutation_rate: default_mutation_rate(),
            inheritance_ratio: default_inheritance_ratio(),
            evaluation_interval: default_evaluation_interval(),
            inactivity_threshold: default_inactivity_threshold(),
            passive_xp_per_tick: default_passive_xp_per_tick(),
            offspring_mutation_rate: default_offspring_mutation_rate(),
            max_offspring_mutations: default_max_offspring_mutations(),
            personality_dimensions: default_personality_dimensions(),
            personality_shift_magnitude: default_personality_shift_magnitude(),
            skill_level_jump_range: default_skill_level_jump_range(),
            skill_level_drop_range: default_skill_level_drop_range(),
            env_pressure_multiplier: default_env_pressure_multiplier(),
            heritable_strengthen_chance: default_heritable_strengthen_chance(),
            heritable_disappear_chance: default_heritable_disappear_chance(),
        }
    }
}

fn default_skill_max_level() -> u32 { 10 }
fn default_mutation_rate() -> f64 { 0.05 }
fn default_inheritance_ratio() -> f64 { 0.5 }
fn default_evaluation_interval() -> u64 { 1000 }
fn default_inactivity_threshold() -> u64 { 500 }
fn default_passive_xp_per_tick() -> f64 { 1.0 }
fn default_offspring_mutation_rate() -> f64 { 0.15 }
fn default_max_offspring_mutations() -> usize { 3 }
fn default_personality_dimensions() -> usize { 5 }
fn default_personality_shift_magnitude() -> f64 { 0.2 }
fn default_skill_level_jump_range() -> u32 { 2 }
fn default_skill_level_drop_range() -> u32 { 1 }
fn default_env_pressure_multiplier() -> f64 { 2.0 }
fn default_heritable_strengthen_chance() -> f64 { 0.3 }
fn default_heritable_disappear_chance() -> f64 { 0.2 }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct A2aConfig {
    #[serde(default = "default_protocol_version")]
    pub protocol_version: String,
    #[serde(default = "default_max_message_size_kb")]
    pub max_message_size_kb: u32,
    #[serde(default = "default_message_timeout_ms")]
    pub message_timeout_ms: u64,
    #[serde(default = "default_discovery_interval_ms")]
    pub discovery_interval_ms: u64,
}

impl Default for A2aConfig {
    fn default() -> Self {
        Self {
            protocol_version: default_protocol_version(),
            max_message_size_kb: default_max_message_size_kb(),
            message_timeout_ms: default_message_timeout_ms(),
            discovery_interval_ms: default_discovery_interval_ms(),
        }
    }
}

fn default_protocol_version() -> String { "v1".to_string() }
fn default_max_message_size_kb() -> u32 { 64 }
fn default_message_timeout_ms() -> u64 { 30_000 }
fn default_discovery_interval_ms() -> u64 { 5000 }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SurvivalConfig {
    #[serde(default = "default_priorities")]
    pub priorities: Vec<String>,
}

impl Default for SurvivalConfig {
    fn default() -> Self {
        Self {
            priorities: default_priorities(),
        }
    }
}

fn default_priorities() -> Vec<String> {
    vec![
        "token_critical".into(),
        "threat_response".into(),
        "message_response".into(),
        "task_completion".into(),
        "opportunity_seek".into(),
        "social_maintain".into(),
        "skill_improve".into(),
        "explore".into(),
    ]
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MarketConfig {
    #[serde(default = "default_task_expiry_ticks")]
    pub task_expiry_ticks: u64,
    #[serde(default = "default_min_reward_money")]
    pub min_reward_money: u64,
    #[serde(default = "default_reputation_decay")]
    pub reputation_decay: f64,
}

impl Default for MarketConfig {
    fn default() -> Self {
        Self {
            task_expiry_ticks: default_task_expiry_ticks(),
            min_reward_money: default_min_reward_money(),
            reputation_decay: default_reputation_decay(),
        }
    }
}

fn default_task_expiry_ticks() -> u64 { 500 }
fn default_min_reward_money() -> u64 { 1 }
fn default_reputation_decay() -> f64 { 0.01 }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SafetyConfig {
    #[serde(default = "default_max_agents_per_org")]
    pub max_agents_per_org: u32,
    #[serde(default = "default_anti_monopoly_threshold")]
    pub anti_monopoly_threshold: f64,
    #[serde(default = "default_new_agent_protection_ticks")]
    pub new_agent_protection_ticks: u64,
}

impl Default for SafetyConfig {
    fn default() -> Self {
        Self {
            max_agents_per_org: default_max_agents_per_org(),
            anti_monopoly_threshold: default_anti_monopoly_threshold(),
            new_agent_protection_ticks: default_new_agent_protection_ticks(),
        }
    }
}

fn default_max_agents_per_org() -> u32 { 5 }
fn default_anti_monopoly_threshold() -> f64 { 0.3 }
fn default_new_agent_protection_ticks() -> u64 { 50 }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrustConfigSection {
    #[serde(default = "default_trust_cooperation_gain")]
    pub cooperation_gain: f64,
    #[serde(default = "default_trust_betrayal_loss")]
    pub betrayal_loss: f64,
    #[serde(default = "default_trust_decay_rate")]
    pub decay_rate: f64,
    #[serde(default = "default_trust_interaction_interval")]
    pub interaction_interval: u64,
}

impl Default for TrustConfigSection {
    fn default() -> Self {
        Self {
            cooperation_gain: default_trust_cooperation_gain(),
            betrayal_loss: default_trust_betrayal_loss(),
            decay_rate: default_trust_decay_rate(),
            interaction_interval: default_trust_interaction_interval(),
        }
    }
}

fn default_trust_cooperation_gain() -> f64 { 0.1 }
fn default_trust_betrayal_loss() -> f64 { 0.3 }
fn default_trust_decay_rate() -> f64 { 0.001 }
fn default_trust_interaction_interval() -> u64 { 50 }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MentorshipConfigSection {
    #[serde(default = "default_mentorship_ticks_per_level")]
    pub ticks_per_level: u64,
    #[serde(default = "default_mentorship_transfer_ratio")]
    pub transfer_ratio: f64,
    #[serde(default = "default_mentorship_max_apprentices")]
    pub max_apprentices_per_mentor: u32,
}

impl Default for MentorshipConfigSection {
    fn default() -> Self {
        Self {
            ticks_per_level: default_mentorship_ticks_per_level(),
            transfer_ratio: default_mentorship_transfer_ratio(),
            max_apprentices_per_mentor: default_mentorship_max_apprentices(),
        }
    }
}

fn default_mentorship_ticks_per_level() -> u64 { 20 }
fn default_mentorship_transfer_ratio() -> f64 { 0.7 }
fn default_mentorship_max_apprentices() -> u32 { 3 }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InheritanceConfigSection {
    #[serde(default = "default_inheritance_ratio_config")]
    pub inheritance_ratio: f64,
    #[serde(default = "default_skill_transfer_ratio_config")]
    pub skill_transfer_ratio: f64,
}

impl Default for InheritanceConfigSection {
    fn default() -> Self {
        Self {
            inheritance_ratio: default_inheritance_ratio_config(),
            skill_transfer_ratio: default_skill_transfer_ratio_config(),
        }
    }
}

fn default_inheritance_ratio_config() -> f64 { 0.5 }
fn default_skill_transfer_ratio_config() -> f64 { 0.3 }

// ── Migration Config ─────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MigrationConfigSection {
    #[serde(default = "default_migration_enabled")]
    pub enabled: bool,
    #[serde(default = "default_daily_quota")]
    pub daily_quota: u32,
    #[serde(default = "default_weekly_quota")]
    pub weekly_quota: u32,
    #[serde(default = "default_min_reputation")]
    pub min_reputation: f64,
    #[serde(default = "default_token_cost")]
    pub token_cost: u64,
    #[serde(default = "default_resource_tax_rate")]
    pub resource_tax_rate: f64,
    #[serde(default)]
    pub require_skill_certification: bool,
    #[serde(default)]
    pub blocked_skills: Vec<String>,
    #[serde(default = "default_cooldown_ticks")]
    pub cooldown_ticks: u32,
}

impl Default for MigrationConfigSection {
    fn default() -> Self {
        Self {
            enabled: default_migration_enabled(),
            daily_quota: default_daily_quota(),
            weekly_quota: default_weekly_quota(),
            min_reputation: default_min_reputation(),
            token_cost: default_token_cost(),
            resource_tax_rate: default_resource_tax_rate(),
            require_skill_certification: false,
            blocked_skills: Vec::new(),
            cooldown_ticks: default_cooldown_ticks(),
        }
    }
}

fn default_migration_enabled() -> bool { true }
fn default_daily_quota() -> u32 { 10 }
fn default_weekly_quota() -> u32 { 50 }
fn default_min_reputation() -> f64 { 0.0 }
fn default_token_cost() -> u64 { 10_000 }
fn default_resource_tax_rate() -> f64 { 0.2 }
fn default_cooldown_ticks() -> u32 { 100 }

// ── Federation Config ─────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FederationConfigSection {
    #[serde(default = "default_heartbeat_timeout")]
    pub heartbeat_timeout_secs: u64,
    #[serde(default = "default_world_id")]
    pub world_id: String,
    #[serde(default)]
    pub bootstrap_peers: Vec<String>,
}

impl Default for FederationConfigSection {
    fn default() -> Self {
        Self {
            heartbeat_timeout_secs: default_heartbeat_timeout(),
            world_id: default_world_id(),
            bootstrap_peers: Vec::new(),
        }
    }
}

fn default_heartbeat_timeout() -> u64 { 90 }
fn default_world_id() -> String { uuid::Uuid::new_v4().to_string() }

// ── Validation ────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct ValidationError {
    pub field: String,
    pub reason: String,
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "validation error on '{}': {}", self.field, self.reason)
    }
}

impl std::error::Error for ValidationError {}

impl GenesisConfig {
    /// Parse genesis.yaml from disk.
    pub fn load_from_file(path: &Path) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let content = std::fs::read_to_string(path)?;
        let config: GenesisConfig = serde_yaml::from_str(&content)?;
        Ok(config)
    }

    /// Validate the configuration, returning a list of errors (empty if valid).
    pub fn validate(&self) -> Vec<ValidationError> {
        let mut errors = Vec::new();

        // World
        if self.world.tick_interval_ms == 0 {
            errors.push(ValidationError {
                field: "world.tick_interval_ms".into(),
                reason: "must be > 0".into(),
            });
        }
        if self.world.max_agents == 0 {
            errors.push(ValidationError {
                field: "world.max_agents".into(),
                reason: "must be > 0".into(),
            });
        }

        // Economy
        if self.economy.initial_tokens == 0 {
            errors.push(ValidationError {
                field: "economy.initial_tokens".into(),
                reason: "must be > 0".into(),
            });
        }
        if self.economy.token_price == 0 {
            errors.push(ValidationError {
                field: "economy.token_price".into(),
                reason: "must be > 0".into(),
            });
        }
        if self.economy.interest_rate < 0.0 {
            errors.push(ValidationError {
                field: "economy.interest_rate".into(),
                reason: "must be >= 0".into(),
            });
        }

        // Lifecycle
        if self.lifecycle.birth_tokens == 0 {
            errors.push(ValidationError {
                field: "lifecycle.birth_tokens".into(),
                reason: "must be > 0".into(),
            });
        }
        if self.lifecycle.childhood_ticks == 0 {
            errors.push(ValidationError {
                field: "lifecycle.childhood_ticks".into(),
                reason: "must be > 0".into(),
            });
        }
        if self.lifecycle.adult_ticks == 0 {
            errors.push(ValidationError {
                field: "lifecycle.adult_ticks".into(),
                reason: "must be > 0".into(),
            });
        }
        if self.lifecycle.elder_ticks == 0 {
            errors.push(ValidationError {
                field: "lifecycle.elder_ticks".into(),
                reason: "must be > 0".into(),
            });
        }

        // Evolution
        if self.evolution.skill_max_level == 0 {
            errors.push(ValidationError {
                field: "evolution.skill_max_level".into(),
                reason: "must be > 0".into(),
            });
        }
        if self.evolution.mutation_rate < 0.0 || self.evolution.mutation_rate > 1.0 {
            errors.push(ValidationError {
                field: "evolution.mutation_rate".into(),
                reason: "must be between 0.0 and 1.0".into(),
            });
        }
        if self.evolution.inheritance_ratio < 0.0 || self.evolution.inheritance_ratio > 1.0 {
            errors.push(ValidationError {
                field: "evolution.inheritance_ratio".into(),
                reason: "must be between 0.0 and 1.0".into(),
            });
        }

        // A2A
        if self.a2a.max_message_size_kb == 0 {
            errors.push(ValidationError {
                field: "a2a.max_message_size_kb".into(),
                reason: "must be > 0".into(),
            });
        }

        // Market
        if self.market.reputation_decay < 0.0 || self.market.reputation_decay > 1.0 {
            errors.push(ValidationError {
                field: "market.reputation_decay".into(),
                reason: "must be between 0.0 and 1.0".into(),
            });
        }

        // Safety
        if self.safety.anti_monopoly_threshold <= 0.0 || self.safety.anti_monopoly_threshold > 1.0 {
            errors.push(ValidationError {
                field: "safety.anti_monopoly_threshold".into(),
                reason: "must be between 0.0 (exclusive) and 1.0".into(),
            });
        }

        errors
    }
}

// ── Config Watcher ────────────────────────────────────────

/// Shared, hot-reloadable configuration.
///
/// Uses a `tokio::sync::watch` channel internally so subscribers can be
/// notified atomically when a new config is staged.
pub type SharedConfig = Arc<ConfigManager>;

/// Manages the live configuration, with atomic swap-on-tick semantics.
pub struct ConfigManager {
    /// Path to genesis.yaml on disk.
    path: PathBuf,

    /// The currently active configuration (applied at last tick boundary).
    active: tokio::sync::RwLock<GenesisConfig>,

    /// Staged configuration pending application at next tick boundary.
    /// `None` means no reload is pending.
    pending: Mutex<Option<GenesisConfig>>,

    /// Optional event bus for broadcasting config-changed events.
    event_bus: Option<Arc<EventBus>>,
}

impl ConfigManager {
    /// Create a new config manager by loading genesis.yaml from disk.
    pub fn new(path: impl Into<PathBuf>, event_bus: Option<Arc<EventBus>>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let path = path.into();
        let config = GenesisConfig::load_from_file(&path)?;
        info!("Loaded genesis config from {:?}", path);

        let errors = config.validate();
        if !errors.is_empty() {
            for e in &errors {
                error!("Config validation error: {}", e);
            }
            return Err(format!("genesis.yaml has {} validation error(s)", errors.len()).into());
        }

        Ok(Self {
            path,
            active: tokio::sync::RwLock::new(config),
            pending: Mutex::new(None),
            event_bus,
        })
    }

    /// Get a reference to the currently active config.
    pub async fn get(&self) -> tokio::sync::RwLockReadGuard<'_, GenesisConfig> {
        self.active.read().await
    }

    /// Stage a new config for application at the next tick boundary.
    /// Validates the config first; returns `Err` and keeps the old config if invalid.
    pub async fn stage_reload(&self, new_config: GenesisConfig) -> Result<(), Vec<ValidationError>> {
        let errors = new_config.validate();
        if !errors.is_empty() {
            for e in &errors {
                warn!("Rejecting config reload: {}", e);
            }
            return Err(errors);
        }

        let mut pending = self.pending.lock().await;
        info!("Staging new config for next tick boundary");
        *pending = Some(new_config);
        Ok(())
    }

    /// Apply any pending config at the tick boundary.
    /// Call this at the start of each tick. Returns `true` if config was swapped.
    pub async fn apply_pending(&self) -> bool {
        let new_config = {
            let mut pending = self.pending.lock().await;
            pending.take()
        };

        if let Some(new_config) = new_config {
            let mut active = self.active.write().await;
            info!(
                "Applying new config at tick boundary (tick_interval_ms={})",
                new_config.world.tick_interval_ms
            );
            *active = new_config;

            // Broadcast config-changed event
            if let Some(ref bus) = self.event_bus {
                bus.emit(WorldEvent::ConfigReloaded {
                    source: self.path.to_string_lossy().to_string(),
                });
            }

            return true;
        }

        false
    }

    /// Re-read the config file from disk, validate, and stage it.
    pub async fn reload_from_disk(&self) -> Result<(), String> {
        let config = GenesisConfig::load_from_file(&self.path)
            .map_err(|e| format!("Failed to parse genesis.yaml: {}", e))?;

        self.stage_reload(config)
            .await
            .map_err(|errors| {
                let msgs: Vec<String> = errors.iter().map(|e| e.to_string()).collect();
                format!("Config validation failed: {}", msgs.join("; "))
            })
    }

    /// Path to the watched genesis.yaml file.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

// ── File Watcher Task ─────────────────────────────────────

/// Spawn a background task that watches genesis.yaml for changes using the
/// `notify` crate and triggers hot-reload on the `ConfigManager`.
///
/// Returns a `JoinHandle` for the watcher loop and a cancellation token.
pub fn spawn_config_watcher(
    config_manager: SharedConfig,
) -> Result<(tokio::task::JoinHandle<()>, tokio::sync::oneshot::Sender<()>), Box<dyn std::error::Error + Send + Sync>> {
    let watch_path = config_manager.path().to_path_buf();

    // Debounce: coalesce rapid file-writes within this window.
    let debounce = Duration::from_millis(500);

    let (fs_tx, fs_rx) = std::sync::mpsc::channel();

    // Create the watcher (blocking-oriented, we bridge to tokio via a std thread).
    let mut watcher = RecommendedWatcher::new(
        move |res: Result<Event, notify::Error>| {
            if let Ok(event) = res {
                // We only care about modify / create / close-write events on our file.
                match event.kind {
                    EventKind::Modify(_) | EventKind::Create(_) => {
                        // Check if any of the affected paths is our genesis.yaml.
                        let relevant = event.paths.iter().any(|p| p == &watch_path);
                        if relevant {
                            let _ = fs_tx.send(());
                        }
                    }
                    _ => {}
                }
            }
        },
        notify::Config::default().with_poll_interval(debounce),
    )?;

    // Watch the parent directory (more robust than watching the file itself;
    // some editors replace the file via rename which breaks direct file watches).
    let parent = config_manager.path().parent().unwrap_or_else(|| Path::new("."));
    watcher.watch(parent, RecursiveMode::NonRecursive)?;

    // oneshot channel for graceful cancellation.
    let (cancel_tx, mut cancel_rx) = tokio::sync::oneshot::channel::<()>();

    let handle = tokio::spawn(async move {
        // Keep the watcher alive for the lifetime of this task.
        let _watcher = watcher;

        // Bridge the std::sync::mpsc::Receiver into tokio via a blocking thread.
        let (async_tx, mut async_rx) = tokio::sync::mpsc::channel::<()>(32);
        tokio::spawn(async move {
            // We need to move fs_rx into a blocking context.
            let rx = fs_rx;
            // Use tokio::task::spawn_blocking to poll the std channel.
            tokio::task::spawn_blocking(move || {
                while let Ok(()) = rx.recv() {
                    if async_tx.blocking_send(()).is_err() {
                        break;
                    }
                }
            })
            .await
            .ok();
        });

        loop {
            tokio::select! {
                Some(()) = async_rx.recv() => {
                    // Debounce: drain any queued notifications.
                    tokio::time::sleep(debounce).await;
                    while async_rx.try_recv().is_ok() {}

                    info!("genesis.yaml change detected, reloading...");
                    match config_manager.reload_from_disk().await {
                        Ok(()) => {
                            info!("Config staged successfully — will apply at next tick boundary");
                        }
                        Err(e) => {
                            error!("Config reload rejected: {} — keeping current config", e);
                        }
                    }
                }
                _ = &mut cancel_rx => {
                    info!("Config watcher shutting down");
                    break;
                }
                else => break,
            }
        }
    });

    Ok((handle, cancel_tx))
}

// ── Tests ─────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_genesis_yaml() {
        let yaml = r#"
world:
  name: "test-world"
  tick_interval_ms: 500
  max_agents: 5
economy:
  initial_tokens: 50000
  think_cost_per_token: 2
  memory_cost_per_kb: 0.2
  communicate_cost: 5
  initial_money: 0
  token_price: 200
  interest_rate: 0.002
lifecycle:
  birth_tokens: 50000
  childhood_ticks: 50
  adult_ticks: 500
  elder_ticks: 100
  death_grace_ticks: 5
evolution:
  skill_max_level: 10
  mutation_rate: 0.05
  inheritance_ratio: 0.5
a2a:
  protocol_version: "v1"
  max_message_size_kb: 64
  message_timeout_ms: 30000
  discovery_interval_ms: 5000
survival:
  priorities:
    - token_critical
    - explore
market:
  task_expiry_ticks: 300
  min_reward_money: 2
  reputation_decay: 0.02
safety:
  max_agents_per_org: 3
  anti_monopoly_threshold: 0.25
  new_agent_protection_ticks: 30
"#;

        let config: GenesisConfig = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(config.world.name, "test-world");
        assert_eq!(config.world.tick_interval_ms, 500);
        assert_eq!(config.world.max_agents, 5);
        assert_eq!(config.economy.initial_tokens, 50_000);
        assert_eq!(config.lifecycle.childhood_ticks, 50);
        assert_eq!(config.evolution.mutation_rate, 0.05);
        assert_eq!(config.safety.max_agents_per_org, 3);
    }

    #[test]
    fn defaults_used_when_sections_missing() {
        let yaml = "{}";
        let config: GenesisConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.world.tick_interval_ms, 1000);
        assert_eq!(config.world.max_agents, 10);
        assert_eq!(config.economy.initial_tokens, 100_000);
        assert_eq!(config.lifecycle.birth_tokens, 100_000);
        assert_eq!(config.evolution.skill_max_level, 10);
    }

    #[test]
    fn validate_valid_config() {
        let config = GenesisConfig::default();
        assert!(config.validate().is_empty());
    }

    #[test]
    fn validate_catches_zero_tick_interval() {
        let mut config = GenesisConfig::default();
        config.world.tick_interval_ms = 0;
        let errors = config.validate();
        assert!(errors.iter().any(|e| e.field == "world.tick_interval_ms"));
    }

    #[test]
    fn validate_catches_zero_max_agents() {
        let mut config = GenesisConfig::default();
        config.world.max_agents = 0;
        let errors = config.validate();
        assert!(errors.iter().any(|e| e.field == "world.max_agents"));
    }

    #[test]
    fn validate_catches_negative_interest_rate() {
        let mut config = GenesisConfig::default();
        config.economy.interest_rate = -0.1;
        let errors = config.validate();
        assert!(errors.iter().any(|e| e.field == "economy.interest_rate"));
    }

    #[test]
    fn validate_catches_out_of_range_mutation_rate() {
        let mut config = GenesisConfig::default();
        config.evolution.mutation_rate = 1.5;
        let errors = config.validate();
        assert!(errors.iter().any(|e| e.field == "evolution.mutation_rate"));
    }

    #[test]
    fn validate_catches_zero_lifecycle_ticks() {
        let mut config = GenesisConfig::default();
        config.lifecycle.childhood_ticks = 0;
        let errors = config.validate();
        assert!(errors.iter().any(|e| e.field == "lifecycle.childhood_ticks"));
    }

    #[test]
    fn validate_catches_zero_skill_max_level() {
        let mut config = GenesisConfig::default();
        config.evolution.skill_max_level = 0;
        let errors = config.validate();
        assert!(errors.iter().any(|e| e.field == "evolution.skill_max_level"));
    }

    #[tokio::test]
    async fn config_manager_stages_and_applies() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("genesis.yaml");
        let config = GenesisConfig::default();
        std::fs::write(&path, serde_yaml::to_string(&config).unwrap()).unwrap();

        let mgr = ConfigManager::new(&path, None).unwrap();

        // Modify config
        let mut new_config = GenesisConfig::default();
        new_config.world.tick_interval_ms = 500;

        mgr.stage_reload(new_config.clone()).await.unwrap();

        // Pending should not affect active yet
        let active = mgr.get().await;
        assert_eq!(active.world.tick_interval_ms, 1000);
        drop(active);

        // Apply at tick boundary
        let applied = mgr.apply_pending().await;
        assert!(applied);

        let active = mgr.get().await;
        assert_eq!(active.world.tick_interval_ms, 500);
    }

    #[tokio::test]
    async fn config_manager_rejects_invalid_stage() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("genesis.yaml");
        let config = GenesisConfig::default();
        std::fs::write(&path, serde_yaml::to_string(&config).unwrap()).unwrap();

        let mgr = ConfigManager::new(&path, None).unwrap();

        let mut bad_config = GenesisConfig::default();
        bad_config.world.tick_interval_ms = 0; // invalid

        let result = mgr.stage_reload(bad_config).await;
        assert!(result.is_err());

        // Active config unchanged
        let active = mgr.get().await;
        assert_eq!(active.world.tick_interval_ms, 1000);
    }

    #[tokio::test]
    async fn config_manager_apply_pending_noop_when_none() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("genesis.yaml");
        let config = GenesisConfig::default();
        std::fs::write(&path, serde_yaml::to_string(&config).unwrap()).unwrap();

        let mgr = ConfigManager::new(&path, None).unwrap();
        let applied = mgr.apply_pending().await;
        assert!(!applied);
    }

    #[tokio::test]
    async fn config_manager_broadcasts_event_on_apply() {
        let event_bus = Arc::new(EventBus::new(64));
        let mut rx = event_bus.subscribe();

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("genesis.yaml");
        let config = GenesisConfig::default();
        std::fs::write(&path, serde_yaml::to_string(&config).unwrap()).unwrap();

        let mgr = ConfigManager::new(&path, Some(event_bus)).unwrap();

        let mut new_config = GenesisConfig::default();
        new_config.world.max_agents = 20;
        mgr.stage_reload(new_config).await.unwrap();
        let applied = mgr.apply_pending().await;
        assert!(applied);

        let event = rx.try_recv().unwrap();
        assert!(matches!(event, WorldEvent::ConfigReloaded { .. }));
    }
}
