//! Genesis configuration — loads world parameters from `genesis.yaml`.
//!
//! Provides [`GenesisConfig`] which deserialises the `world` section of
//! `genesis.yaml` and exposes the tick interval, agent cap, lifecycle
//! durations, and other parameters needed by the engine.

use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Top-level genesis configuration, mirroring `config/genesis.yaml`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GenesisConfig {
    #[serde(default)]
    pub world: WorldSection,
    #[serde(default)]
    pub economy: EconomySection,
    #[serde(default)]
    pub lifecycle: LifecycleSection,
    #[serde(default)]
    pub safety: SafetySection,
}

impl GenesisConfig {
    /// Parse from a YAML string.
    pub fn from_yaml(yaml: &str) -> Result<Self, serde_yaml::Error> {
        serde_yaml::from_str(yaml)
    }

    /// Load from a file on disk.
    pub fn from_file(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let contents = std::fs::read_to_string(path)?;
        Ok(Self::from_yaml(&contents)?)
    }

    /// The tick interval as a [`Duration`].
    pub fn tick_interval(&self) -> Duration {
        Duration::from_millis(self.world.tick_interval_ms)
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Sections
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldSection {
    #[serde(default = "default_name")]
    pub name: String,
    #[serde(default = "default_tick_interval_ms")]
    pub tick_interval_ms: u64,
    #[serde(default = "default_max_agents")]
    pub max_agents: u32,
}

impl Default for WorldSection {
    fn default() -> Self {
        Self {
            name: default_name(),
            tick_interval_ms: default_tick_interval_ms(),
            max_agents: default_max_agents(),
        }
    }
}

fn default_name() -> String {
    "agent-world-v1".to_string()
}
fn default_tick_interval_ms() -> u64 {
    1000
}
fn default_max_agents() -> u32 {
    10
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EconomySection {
    #[serde(default = "default_initial_tokens")]
    pub initial_tokens: u64,
    #[serde(default = "default_think_cost")]
    pub think_cost_per_token: u64,
    #[serde(default = "default_memory_cost")]
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

impl Default for EconomySection {
    fn default() -> Self {
        Self {
            initial_tokens: default_initial_tokens(),
            think_cost_per_token: default_think_cost(),
            memory_cost_per_kb: default_memory_cost(),
            communicate_cost: default_communicate_cost(),
            initial_money: 0,
            token_price: default_token_price(),
            interest_rate: default_interest_rate(),
        }
    }
}

fn default_initial_tokens() -> u64 {
    100_000
}
fn default_think_cost() -> u64 {
    1
}
fn default_memory_cost() -> f64 {
    0.1
}
fn default_communicate_cost() -> u64 {
    10
}
fn default_token_price() -> u64 {
    100
}
fn default_interest_rate() -> f64 {
    0.001
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LifecycleSection {
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

impl Default for LifecycleSection {
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

fn default_birth_tokens() -> u64 {
    100_000
}
fn default_childhood_ticks() -> u64 {
    100
}
fn default_adult_ticks() -> u64 {
    1000
}
fn default_elder_ticks() -> u64 {
    200
}
fn default_death_grace_ticks() -> u64 {
    10
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetySection {
    #[serde(default = "default_max_agents_per_org")]
    pub max_agents_per_org: u32,
    #[serde(default = "default_anti_monopoly")]
    pub anti_monopoly_threshold: f64,
    #[serde(default = "default_protection_ticks")]
    pub new_agent_protection_ticks: u64,
}

impl Default for SafetySection {
    fn default() -> Self {
        Self {
            max_agents_per_org: default_max_agents_per_org(),
            anti_monopoly_threshold: default_anti_monopoly(),
            new_agent_protection_ticks: default_protection_ticks(),
        }
    }
}

fn default_max_agents_per_org() -> u32 {
    5
}
fn default_anti_monopoly() -> f64 {
    0.3
}
fn default_protection_ticks() -> u64 {
    50
}

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
  initial_tokens: 200000
  think_cost_per_token: 2
  memory_cost_per_kb: 0.2
  communicate_cost: 20
  initial_money: 100
  token_price: 200
  interest_rate: 0.002
lifecycle:
  birth_tokens: 200000
  childhood_ticks: 50
  adult_ticks: 500
  elder_ticks: 100
  death_grace_ticks: 5
safety:
  max_agents_per_org: 3
  anti_monopoly_threshold: 0.2
  new_agent_protection_ticks: 30
"#;
        let config = GenesisConfig::from_yaml(yaml).unwrap();
        assert_eq!(config.world.name, "test-world");
        assert_eq!(config.world.tick_interval_ms, 500);
        assert_eq!(config.world.max_agents, 5);
        assert_eq!(config.tick_interval(), Duration::from_millis(500));
        assert_eq!(config.economy.initial_tokens, 200_000);
        assert_eq!(config.lifecycle.death_grace_ticks, 5);
        assert_eq!(config.safety.new_agent_protection_ticks, 30);
    }

    #[test]
    fn default_config_values() {
        let config = GenesisConfig::default();
        assert_eq!(config.world.tick_interval_ms, 1000);
        assert_eq!(config.world.max_agents, 10);
        assert_eq!(config.economy.initial_tokens, 100_000);
        assert_eq!(config.lifecycle.death_grace_ticks, 10);
    }

    #[test]
    fn parse_partial_yaml_uses_defaults() {
        let yaml = r#"
world:
  tick_interval_ms: 2000
"#;
        let config = GenesisConfig::from_yaml(yaml).unwrap();
        assert_eq!(config.world.tick_interval_ms, 2000);
        // Everything else should be defaults
        assert_eq!(config.world.name, "agent-world-v1");
        assert_eq!(config.economy.initial_tokens, 100_000);
        assert_eq!(config.lifecycle.death_grace_ticks, 10);
    }

    #[test]
    fn parse_empty_yaml_uses_all_defaults() {
        let yaml = "{}";
        let config = GenesisConfig::from_yaml(yaml).unwrap();
        assert_eq!(config.world.tick_interval_ms, 1000);
        assert_eq!(config.world.max_agents, 10);
    }
}
