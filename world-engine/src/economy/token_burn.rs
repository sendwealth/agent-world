use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::world::enums::AgentPhase;

// ── Skill Record ─────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillRecord {
    pub name: String,
    pub level: u32,
    pub experience: f64,
}

// ── Agent Record ─────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRecord {
    pub id: Uuid,
    pub name: String,
    pub phase: AgentPhase,
    pub tokens: u64,
    pub skills: HashMap<String, SkillRecord>,
    /// Personality vector serialized as JSON. Stored as a string to avoid
    /// tight coupling with the Python-side model; the agent runtime owns the
    /// schema.  Empty string means "not yet initialized" (will use defaults).
    #[serde(default)]
    pub personality: String,
    /// Number of tasks this agent has completed successfully.
    #[serde(default)]
    pub tasks_completed: u32,
    /// Number of tasks this agent has attempted (claimed or started).
    #[serde(default)]
    pub tasks_attempted: u32,
}

impl AgentRecord {
    /// Record that this agent has attempted a new task (claimed or started).
    pub fn record_task_attempt(&mut self) {
        self.tasks_attempted = self.tasks_attempted.saturating_add(1);
    }

    /// Record that this agent has completed a task successfully.
    pub fn record_task_completed(&mut self) {
        self.tasks_completed = self.tasks_completed.saturating_add(1);
    }
}

// ── Consumption Config ───────────────────────────────────

/// Configurable token consumption rules.
/// These map to fields read from genesis.yaml.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsumptionConfig {
    /// Base token burn per tick for an adult agent.
    #[serde(default = "default_base_burn")]
    pub base_burn_per_tick: f64,

    /// Phase multipliers: childhood, adult, elder.
    #[serde(default = "default_phase_multipliers")]
    pub phase_multipliers: PhaseMultipliers,

    /// Cost multiplier per skill level per tick.
    #[serde(default = "default_skill_cost_per_level")]
    pub skill_cost_per_level: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseMultipliers {
    #[serde(default = "default_childhood_mult")]
    pub childhood: f64,
    #[serde(default = "default_adult_mult")]
    pub adult: f64,
    #[serde(default = "default_elder_mult")]
    pub elder: f64,
}

fn default_base_burn() -> f64 {
    10.0
}
fn default_skill_cost_per_level() -> f64 {
    0.5
}
fn default_childhood_mult() -> f64 {
    0.5
}
fn default_adult_mult() -> f64 {
    1.0
}
fn default_elder_mult() -> f64 {
    0.7
}

fn default_phase_multipliers() -> PhaseMultipliers {
    PhaseMultipliers {
        childhood: default_childhood_mult(),
        adult: default_adult_mult(),
        elder: default_elder_mult(),
    }
}

impl Default for ConsumptionConfig {
    fn default() -> Self {
        Self {
            base_burn_per_tick: default_base_burn(),
            phase_multipliers: default_phase_multipliers(),
            skill_cost_per_level: default_skill_cost_per_level(),
        }
    }
}

impl ConsumptionConfig {
    /// Load consumption config from a genesis.yaml-compatible value.
    /// Expects a map with optional `economy.consumption` section.
    pub fn from_yaml_value(value: &serde_yaml::Value) -> Self {
        let mut config = Self::default();
        if let Some(economy) = value.get("economy") {
            if let Some(base) = economy.get("base_burn_per_tick").and_then(|v| v.as_f64()) {
                config.base_burn_per_tick = base;
            }
            if let Some(scl) = economy.get("skill_cost_per_level").and_then(|v| v.as_f64()) {
                config.skill_cost_per_level = scl;
            }
            if let Some(pm) = economy.get("phase_multipliers") {
                if let Some(c) = pm.get("childhood").and_then(|v| v.as_f64()) {
                    config.phase_multipliers.childhood = c;
                }
                if let Some(a) = pm.get("adult").and_then(|v| v.as_f64()) {
                    config.phase_multipliers.adult = a;
                }
                if let Some(e) = pm.get("elder").and_then(|v| v.as_f64()) {
                    config.phase_multipliers.elder = e;
                }
            }
        }
        config
    }
}

// ── Burn Result ──────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct AgentBurn {
    pub agent_id: Uuid,
    pub agent_name: String,
    pub phase: AgentPhase,
    pub burn_amount: u64,
    pub tokens_before: u64,
    pub tokens_after: u64,
}

#[derive(Debug, Clone)]
pub struct BurnResult {
    pub tick: u64,
    pub burns: Vec<AgentBurn>,
    pub total_burned: u64,
}

// ── Token Burn Engine ────────────────────────────────────

#[derive(Clone)]
pub struct TokenBurnEngine {
    config: ConsumptionConfig,
}

impl TokenBurnEngine {
    pub fn new(config: ConsumptionConfig) -> Self {
        Self { config }
    }

    pub fn with_defaults() -> Self {
        Self::new(ConsumptionConfig::default())
    }

    /// Get the phase multiplier for a given agent phase.
    pub fn phase_multiplier(&self, phase: AgentPhase) -> f64 {
        match phase {
            AgentPhase::Childhood => self.config.phase_multipliers.childhood,
            AgentPhase::Adult => self.config.phase_multipliers.adult,
            AgentPhase::Elder => self.config.phase_multipliers.elder,
            _ => 0.0,
        }
    }

    /// Calculate the token burn for a single agent in one tick.
    ///
    /// Returns 0 for dead or birth-phase agents.
    /// The burn is: base_burn * phase_multiplier + sum(skill_level * skill_cost_per_level)
    pub fn calculate_tick_burn(&self, agent: &AgentRecord) -> u64 {
        let phase_mult = self.phase_multiplier(agent.phase);
        if phase_mult == 0.0 {
            return 0;
        }

        // Base survival cost
        let base = self.config.base_burn_per_tick * phase_mult;

        // Skill maintenance cost (higher skills cost more)
        let skill_cost: f64 = agent
            .skills
            .values()
            .map(|s| s.level as f64 * self.config.skill_cost_per_level)
            .sum();

        let total = base + skill_cost;
        total as u64
    }

    /// Process token consumption for a batch of agents in a single tick.
    ///
    /// - Skips dead agents and birth-phase agents (0 burn).
    /// - Deducts tokens, clamping to 0 (agents never go negative).
    /// - Returns a `BurnResult` with per-agent details.
    pub fn process_tick(&self, tick: u64, agents: &mut [AgentRecord]) -> BurnResult {
        let mut burns = Vec::with_capacity(agents.len());
        let mut total_burned: u64 = 0;

        for agent in agents.iter_mut() {
            let burn_amount = self.calculate_tick_burn(agent);
            if burn_amount == 0 {
                continue;
            }

            let tokens_before = agent.tokens;
            let actual_burn = burn_amount.min(agent.tokens);
            agent.tokens -= actual_burn;
            total_burned += actual_burn;

            burns.push(AgentBurn {
                agent_id: agent.id,
                agent_name: agent.name.clone(),
                phase: agent.phase,
                burn_amount: actual_burn,
                tokens_before,
                tokens_after: agent.tokens,
            });
        }

        BurnResult {
            tick,
            burns,
            total_burned,
        }
    }
}

// ── Tests ────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_agent(phase: AgentPhase, tokens: u64) -> AgentRecord {
        AgentRecord {
            id: Uuid::new_v4(),
            name: "test-agent".to_string(),
            phase,
            tokens,
            skills: HashMap::new(),
            personality: String::new(),
            tasks_completed: 0,
            tasks_attempted: 0,
        }
    }

    fn make_agent_with_skills(
        phase: AgentPhase,
        tokens: u64,
        skills: Vec<(&str, u32)>,
    ) -> AgentRecord {
        AgentRecord {
            id: Uuid::new_v4(),
            name: "test-agent".to_string(),
            phase,
            tokens,
            skills: skills
                .into_iter()
                .map(|(name, level)| {
                    (
                        name.to_string(),
                        SkillRecord {
                            name: name.to_string(),
                            level,
                            experience: 0.0,
                        },
                    )
                })
                .collect(),
            personality: String::new(),
            tasks_completed: 0,
            tasks_attempted: 0,
        }
    }

    // ── Phase multiplier tests ──

    #[test]
    fn test_phase_multiplier_childhood() {
        let engine = TokenBurnEngine::with_defaults();
        let agent = make_agent(AgentPhase::Childhood, 1000);
        // base=10, childhood mult=0.5 => 5.0
        assert_eq!(engine.calculate_tick_burn(&agent), 5);
    }

    #[test]
    fn test_phase_multiplier_adult() {
        let engine = TokenBurnEngine::with_defaults();
        let agent = make_agent(AgentPhase::Adult, 1000);
        // base=10, adult mult=1.0 => 10
        assert_eq!(engine.calculate_tick_burn(&agent), 10);
    }

    #[test]
    fn test_phase_multiplier_elder() {
        let engine = TokenBurnEngine::with_defaults();
        let agent = make_agent(AgentPhase::Elder, 1000);
        // base=10, elder mult=0.7 => 7.0
        assert_eq!(engine.calculate_tick_burn(&agent), 7);
    }

    #[test]
    fn test_phase_multiplier_dead_is_zero() {
        let engine = TokenBurnEngine::with_defaults();
        let agent = make_agent(AgentPhase::Dead, 1000);
        assert_eq!(engine.calculate_tick_burn(&agent), 0);
    }

    #[test]
    fn test_phase_multiplier_birth_is_zero() {
        let engine = TokenBurnEngine::with_defaults();
        let agent = make_agent(AgentPhase::Birth, 1000);
        assert_eq!(engine.calculate_tick_burn(&agent), 0);
    }

    // ── Skill maintenance cost tests ──

    #[test]
    fn test_skill_cost_single_skill() {
        let engine = TokenBurnEngine::with_defaults();
        let agent = make_agent_with_skills(AgentPhase::Adult, 10000, vec![("mining", 5)]);
        // base=10 * 1.0 + skill: 5 * 0.5 = 10 + 2.5 = 12.5 => 12
        assert_eq!(engine.calculate_tick_burn(&agent), 12);
    }

    #[test]
    fn test_skill_cost_multiple_skills() {
        let engine = TokenBurnEngine::with_defaults();
        let agent = make_agent_with_skills(
            AgentPhase::Adult,
            10000,
            vec![("mining", 5), ("trading", 3), ("crafting", 8)],
        );
        // base=10 + skills: (5+3+8) * 0.5 = 10 + 8 = 18
        assert_eq!(engine.calculate_tick_burn(&agent), 18);
    }

    #[test]
    fn test_skill_cost_with_childhood_multiplier() {
        let engine = TokenBurnEngine::with_defaults();
        let agent = make_agent_with_skills(AgentPhase::Childhood, 10000, vec![("mining", 4)]);
        // base=10 * 0.5 + skills: 4 * 0.5 = 5 + 2 = 7
        assert_eq!(engine.calculate_tick_burn(&agent), 7);
    }

    #[test]
    fn test_skill_cost_with_elder_multiplier() {
        let engine = TokenBurnEngine::with_defaults();
        let agent = make_agent_with_skills(AgentPhase::Elder, 10000, vec![("mining", 10)]);
        // base=10 * 0.7 + skills: 10 * 0.5 = 7 + 5 = 12
        assert_eq!(engine.calculate_tick_burn(&agent), 12);
    }

    #[test]
    fn test_no_skills_base_only() {
        let engine = TokenBurnEngine::with_defaults();
        let agent = make_agent(AgentPhase::Adult, 10000);
        // base=10 * 1.0 + 0 = 10
        assert_eq!(engine.calculate_tick_burn(&agent), 10);
    }

    // ── Batch processing tests ──

    #[test]
    fn test_process_tick_multiple_agents() {
        let engine = TokenBurnEngine::with_defaults();
        let mut agents = vec![
            make_agent(AgentPhase::Childhood, 1000),
            make_agent(AgentPhase::Adult, 2000),
            make_agent(AgentPhase::Elder, 1500),
        ];

        let result = engine.process_tick(1, &mut agents);

        assert_eq!(result.tick, 1);
        assert_eq!(result.burns.len(), 3);

        // Verify each agent's deduction
        assert_eq!(agents[0].tokens, 1000 - 5); // childhood: 10*0.5=5
        assert_eq!(agents[1].tokens, 2000 - 10); // adult: 10*1.0=10
        assert_eq!(agents[2].tokens, 1500 - 7); // elder: 10*0.7=7

        assert_eq!(result.total_burned, 5 + 10 + 7);
    }

    #[test]
    fn test_process_tick_skips_dead() {
        let engine = TokenBurnEngine::with_defaults();
        let mut agents = vec![
            make_agent(AgentPhase::Dead, 500),
            make_agent(AgentPhase::Adult, 1000),
        ];

        let result = engine.process_tick(1, &mut agents);

        assert_eq!(result.burns.len(), 1);
        assert_eq!(agents[0].tokens, 500); // Dead agent unchanged
        assert_eq!(agents[1].tokens, 990); // Adult burns 10
    }

    #[test]
    fn test_process_tick_skips_birth() {
        let engine = TokenBurnEngine::with_defaults();
        let mut agents = vec![
            make_agent(AgentPhase::Birth, 500),
            make_agent(AgentPhase::Adult, 1000),
        ];

        let result = engine.process_tick(1, &mut agents);

        assert_eq!(result.burns.len(), 1);
        assert_eq!(agents[0].tokens, 500); // Birth agent unchanged
    }

    #[test]
    fn test_process_tick_clamps_to_zero() {
        let engine = TokenBurnEngine::with_defaults();
        let mut agents = vec![
            make_agent(AgentPhase::Adult, 3), // only 3 tokens, burn is 10
        ];

        let result = engine.process_tick(1, &mut agents);

        assert_eq!(result.burns.len(), 1);
        assert_eq!(agents[0].tokens, 0); // Clamped to 0, not negative
        assert_eq!(result.burns[0].burn_amount, 3); // Only burned what was available
        assert_eq!(result.burns[0].tokens_before, 3);
        assert_eq!(result.burns[0].tokens_after, 0);
    }

    #[test]
    fn test_process_tick_empty_agents() {
        let engine = TokenBurnEngine::with_defaults();
        let mut agents: Vec<AgentRecord> = vec![];

        let result = engine.process_tick(1, &mut agents);

        assert_eq!(result.burns.len(), 0);
        assert_eq!(result.total_burned, 0);
    }

    #[test]
    fn test_process_tick_all_dead() {
        let engine = TokenBurnEngine::with_defaults();
        let mut agents = vec![
            make_agent(AgentPhase::Dead, 0),
            make_agent(AgentPhase::Dead, 100),
        ];

        let result = engine.process_tick(1, &mut agents);

        assert_eq!(result.burns.len(), 0);
        assert_eq!(result.total_burned, 0);
    }

    // ── Configurable rules tests ──

    #[test]
    fn test_custom_base_burn() {
        let config = ConsumptionConfig {
            base_burn_per_tick: 50.0,
            ..Default::default()
        };
        let engine = TokenBurnEngine::new(config);
        let agent = make_agent(AgentPhase::Adult, 10000);
        // 50 * 1.0 = 50
        assert_eq!(engine.calculate_tick_burn(&agent), 50);
    }

    #[test]
    fn test_custom_phase_multipliers() {
        let config = ConsumptionConfig {
            phase_multipliers: PhaseMultipliers {
                childhood: 0.3,
                adult: 1.5,
                elder: 0.6,
            },
            ..Default::default()
        };
        let engine = TokenBurnEngine::new(config);

        let child = make_agent(AgentPhase::Childhood, 10000);
        let adult = make_agent(AgentPhase::Adult, 10000);
        let elder = make_agent(AgentPhase::Elder, 10000);

        assert_eq!(engine.calculate_tick_burn(&child), 3); // 10 * 0.3 = 3
        assert_eq!(engine.calculate_tick_burn(&adult), 15); // 10 * 1.5 = 15
        assert_eq!(engine.calculate_tick_burn(&elder), 6); // 10 * 0.6 = 6
    }

    #[test]
    fn test_custom_skill_cost_per_level() {
        let config = ConsumptionConfig {
            skill_cost_per_level: 2.0,
            ..Default::default()
        };
        let engine = TokenBurnEngine::new(config);
        let agent = make_agent_with_skills(AgentPhase::Adult, 10000, vec![("mining", 5)]);
        // base=10 * 1.0 + skill: 5 * 2.0 = 10 + 10 = 20
        assert_eq!(engine.calculate_tick_burn(&agent), 20);
    }

    #[test]
    fn test_config_from_yaml_value() {
        let yaml_str = r#"
economy:
  base_burn_per_tick: 25
  skill_cost_per_level: 1.5
  phase_multipliers:
    childhood: 0.4
    adult: 1.2
    elder: 0.8
"#;
        let value: serde_yaml::Value = serde_yaml::from_str(yaml_str).unwrap();
        let config = ConsumptionConfig::from_yaml_value(&value);

        assert_eq!(config.base_burn_per_tick, 25.0);
        assert_eq!(config.skill_cost_per_level, 1.5);
        assert_eq!(config.phase_multipliers.childhood, 0.4);
        assert_eq!(config.phase_multipliers.adult, 1.2);
        assert_eq!(config.phase_multipliers.elder, 0.8);
    }

    #[test]
    fn test_config_from_yaml_defaults_when_missing() {
        let yaml_str = r#"
some_other_section:
  foo: bar
"#;
        let value: serde_yaml::Value = serde_yaml::from_str(yaml_str).unwrap();
        let config = ConsumptionConfig::from_yaml_value(&value);

        // Should fall back to defaults
        assert_eq!(config.base_burn_per_tick, 10.0);
        assert_eq!(config.skill_cost_per_level, 0.5);
        assert_eq!(config.phase_multipliers.childhood, 0.5);
        assert_eq!(config.phase_multipliers.adult, 1.0);
        assert_eq!(config.phase_multipliers.elder, 0.7);
    }

    // ── Multi-tick simulation test ──

    #[test]
    fn test_multi_tick_burn_drains_tokens() {
        let engine = TokenBurnEngine::with_defaults();
        let mut agents = vec![make_agent(AgentPhase::Adult, 30)];
        // Adult burns 10/tick

        engine.process_tick(1, &mut agents);
        assert_eq!(agents[0].tokens, 20);

        engine.process_tick(2, &mut agents);
        assert_eq!(agents[0].tokens, 10);

        engine.process_tick(3, &mut agents);
        assert_eq!(agents[0].tokens, 0);

        // 4th tick: already at 0, burn is 10 but clamped to 0
        let result = engine.process_tick(4, &mut agents);
        assert_eq!(agents[0].tokens, 0);
        assert_eq!(result.burns[0].burn_amount, 0);
    }

    #[test]
    fn test_mixed_agents_multi_tick() {
        let engine = TokenBurnEngine::with_defaults();
        let mut agents = vec![
            make_agent_with_skills(AgentPhase::Childhood, 100, vec![("basic", 2)]),
            make_agent_with_skills(AgentPhase::Adult, 200, vec![("mining", 5), ("trading", 3)]),
            make_agent_with_skills(AgentPhase::Elder, 100, vec![("wisdom", 10)]),
        ];

        // Childhood: base 10*0.5=5 + skills 2*0.5=1 => 6/tick
        // Adult:     base 10*1.0=10 + skills (5+3)*0.5=4 => 14/tick
        // Elder:     base 10*0.7=7 + skills 10*0.5=5 => 12/tick

        let result = engine.process_tick(1, &mut agents);
        assert_eq!(result.total_burned, 6 + 14 + 12);
        assert_eq!(agents[0].tokens, 94);
        assert_eq!(agents[1].tokens, 186);
        assert_eq!(agents[2].tokens, 88);
    }

    #[test]
    fn test_burn_result_details() {
        let engine = TokenBurnEngine::with_defaults();
        let id = Uuid::new_v4();
        let mut agents = vec![AgentRecord {
            id,
            name: "alice".to_string(),
            phase: AgentPhase::Adult,
            tokens: 100,
            skills: HashMap::new(),
            personality: String::new(),
            tasks_completed: 0,
            tasks_attempted: 0,
        }];

        let result = engine.process_tick(42, &mut agents);

        assert_eq!(result.tick, 42);
        assert_eq!(result.burns.len(), 1);
        assert_eq!(result.burns[0].agent_id, id);
        assert_eq!(result.burns[0].agent_name, "alice");
        assert_eq!(result.burns[0].phase, AgentPhase::Adult);
        assert_eq!(result.burns[0].burn_amount, 10);
        assert_eq!(result.burns[0].tokens_before, 100);
        assert_eq!(result.burns[0].tokens_after, 90);
        assert_eq!(result.total_burned, 10);
    }
}
