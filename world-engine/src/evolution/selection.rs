//! Natural selection — fitness evaluation and culling pressure.
//!
//! Every evaluation cycle (default: 1000 ticks), agents are evaluated on:
//! - Token efficiency (tokens remaining vs. initial)
//! - Survival duration (ticks alive)
//! - Task completion rate (placeholder — 0 for now)
//! - Social network size (number of skills as proxy)
//! - Skill diversity (number of distinct skills)
//!
//! Culling pressure increases when:
//! - World token supply is declining
//! - Agent count exceeds `max_agents`
//!
//! Agents with 500+ ticks of inactivity (no state changes) enter decline.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::economy::token_burn::AgentRecord;
use crate::world::enums::AgentPhase;

/// Weights for each fitness dimension.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FitnessWeights {
    pub token_efficiency: f64,
    pub survival_duration: f64,
    pub task_completion: f64,
    pub social_network: f64,
    pub skill_diversity: f64,
}

impl Default for FitnessWeights {
    fn default() -> Self {
        Self {
            token_efficiency: 0.25,
            survival_duration: 0.20,
            task_completion: 0.20,
            social_network: 0.15,
            skill_diversity: 0.20,
        }
    }
}

/// Fitness report for a single agent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FitnessReport {
    pub agent_id: String,
    pub score: f64,
    pub token_efficiency: f64,
    pub survival_duration: f64,
    pub task_completion: f64,
    pub social_network: f64,
    pub skill_diversity: f64,
}

/// Configuration for the selection engine.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SelectionConfig {
    /// How many ticks between evaluation cycles.
    pub evaluation_interval: u64,
    /// Ticks of inactivity before an agent enters decline.
    pub inactivity_threshold: u64,
    /// Fitness weights.
    pub weights: FitnessWeights,
    /// Maximum agents before accelerated culling kicks in.
    pub max_agents: u32,
    /// Base culling rate (fraction of lowest-fitness agents flagged per cycle).
    pub base_cull_rate: f64,
    /// Additional cull rate when over capacity.
    pub over_capacity_cull_rate: f64,
}

impl Default for SelectionConfig {
    fn default() -> Self {
        Self {
            evaluation_interval: 1000,
            inactivity_threshold: 500,
            weights: FitnessWeights::default(),
            max_agents: 10,
            base_cull_rate: 0.1,
            over_capacity_cull_rate: 0.3,
        }
    }
}

/// Evaluates agent fitness for natural selection.
pub struct FitnessEvaluator {
    weights: FitnessWeights,
    initial_tokens: u64,
}

impl FitnessEvaluator {
    pub fn new(weights: FitnessWeights, initial_tokens: u64) -> Self {
        Self {
            weights,
            initial_tokens,
        }
    }

    /// Evaluate fitness for a single agent.
    ///
    /// `spawn_tick` is the tick when the agent was created, used to compute
    /// survival duration. `current_tick` is the current simulation tick.
    /// `last_active_tick` is the last tick the agent had any state change.
    pub fn evaluate(
        &self,
        agent: &AgentRecord,
        spawn_tick: u64,
        current_tick: u64,
        _last_active_tick: u64,
    ) -> FitnessReport {
        let token_efficiency = if self.initial_tokens > 0 {
            agent.tokens as f64 / self.initial_tokens as f64
        } else {
            0.0
        };

        let survival_duration = {
            let age = current_tick.saturating_sub(spawn_tick) as f64;
            // Normalize: 1000 ticks = score of 1.0, caps at 1.0
            (age / 1000.0).min(1.0)
        };

        // Task completion rate: placeholder (no task tracking in AgentRecord yet)
        let task_completion = 0.0;

        // Social network: use number of skills as a proxy
        let social_network = {
            let count = agent.skills.len() as f64;
            // Normalize: 5 skills = score of 1.0, caps at 1.0
            (count / 5.0).min(1.0)
        };

        // Skill diversity: count distinct skills
        let skill_diversity = {
            let count = agent.skills.len() as f64;
            (count / 5.0).min(1.0)
        };

        let score = self.weights.token_efficiency * token_efficiency
            + self.weights.survival_duration * survival_duration
            + self.weights.task_completion * task_completion
            + self.weights.social_network * social_network
            + self.weights.skill_diversity * skill_diversity;

        FitnessReport {
            agent_id: agent.id.to_string(),
            score,
            token_efficiency,
            survival_duration,
            task_completion,
            social_network,
            skill_diversity,
        }
    }
}

/// The natural selection engine that runs evaluation cycles.
pub struct SelectionEngine {
    config: SelectionConfig,
    evaluator: FitnessEvaluator,
    /// Tracks last active tick per agent.
    last_active: HashMap<String, u64>,
}

impl SelectionEngine {
    pub fn new(config: SelectionConfig, initial_tokens: u64) -> Self {
        let evaluator = FitnessEvaluator::new(config.weights.clone(), initial_tokens);
        Self {
            config,
            evaluator,
            last_active: HashMap::new(),
        }
    }

    /// Whether this tick is an evaluation cycle.
    pub fn is_evaluation_tick(&self, tick: u64) -> bool {
        tick > 0 && tick % self.config.evaluation_interval == 0
    }

    /// Update last-active tracking for an agent.
    pub fn mark_active(&mut self, agent_id: &str, tick: u64) {
        self.last_active.insert(agent_id.to_string(), tick);
    }

    /// Check if an agent is in decline (inactive too long).
    pub fn is_in_decline(&self, agent_id: &str, current_tick: u64) -> bool {
        match self.last_active.get(agent_id) {
            Some(last) => current_tick.saturating_sub(*last) >= self.config.inactivity_threshold,
            None => false,
        }
    }

    /// Run a full evaluation cycle. Returns fitness reports for all living
    /// agents, sorted by score ascending (worst first).
    ///
    /// Also returns a list of agent IDs that should be culled due to:
    /// 1. Inactivity beyond threshold
    /// 2. Low fitness score (bottom percentile) when under culling pressure
    pub fn evaluate_cycle(
        &mut self,
        tick: u64,
        agents: &[(uuid::Uuid, u64, AgentRecord)],
        total_world_tokens: u64,
        previous_world_tokens: u64,
    ) -> (Vec<FitnessReport>, Vec<String>) {
        let mut reports = Vec::new();
        let mut to_cull = Vec::new();
        let mut living_count = 0usize;

        for (id, spawn_tick, agent) in agents {
            if agent.phase == AgentPhase::Dead || agent.phase == AgentPhase::Birth {
                continue;
            }

            living_count += 1;

            let last_active = self.last_active.get(&id.to_string()).copied().unwrap_or(*spawn_tick);
            let report = self.evaluator.evaluate(agent, *spawn_tick, tick, last_active);
            reports.push(report);
        }

        // Sort ascending by score (worst first)
        reports.sort_by(|a, b| a.score.partial_cmp(&b.score).unwrap_or(std::cmp::Ordering::Equal));

        // Determine culling pressure
        let over_capacity = living_count as u32 > self.config.max_agents;
        let tokens_declining = total_world_tokens < previous_world_tokens;

        // Cull inactive agents
        for report in &reports {
            if self.is_in_decline(&report.agent_id, tick) {
                to_cull.push(report.agent_id.clone());
            }
        }

        // Apply additional culling under pressure
        let cull_rate = if over_capacity {
            self.config.over_capacity_cull_rate
        } else if tokens_declining {
            self.config.base_cull_rate
        } else {
            0.0
        };

        if cull_rate > 0.0 {
            let cull_count = ((living_count as f64) * cull_rate).ceil() as usize;
            // Cull from the lowest-scoring agents (already sorted ascending)
            for report in reports.iter().take(cull_count) {
                if !to_cull.contains(&report.agent_id) {
                    to_cull.push(report.agent_id.clone());
                }
            }
        }

        (reports, to_cull)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use uuid::Uuid;

    fn make_agent(phase: AgentPhase, tokens: u64, skill_count: usize) -> (Uuid, u64, AgentRecord) {
        let id = Uuid::new_v4();
        let mut skills = HashMap::new();
        let skill_names = ["coding", "trading", "crafting", "mining", "exploration"];
        for i in 0..skill_count.min(skill_names.len()) {
            skills.insert(
                skill_names[i].to_string(),
                crate::economy::token_burn::SkillRecord {
                    name: skill_names[i].to_string(),
                    level: 1,
                    experience: 0.0,
                },
            );
        }
        (
            id,
            0,
            AgentRecord {
                id,
                name: "test".to_string(),
                phase,
                tokens,
                skills,
            },
        )
    }

    #[test]
    fn fitness_all_dimensions() {
        let weights = FitnessWeights::default();
        let evaluator = FitnessEvaluator::new(weights, 100_000);
        let agent = make_agent(AgentPhase::Adult, 50_000, 3).2;
        let report = evaluator.evaluate(&agent, 0, 500, 500);

        assert!((report.token_efficiency - 0.5).abs() < f64::EPSILON);
        assert!(report.score > 0.0);
        assert!(report.score <= 1.0);
    }

    #[test]
    fn fitness_zero_tokens() {
        let weights = FitnessWeights::default();
        let evaluator = FitnessEvaluator::new(weights, 100_000);
        let agent = make_agent(AgentPhase::Adult, 0, 0).2;
        let report = evaluator.evaluate(&agent, 0, 100, 100);
        assert!((report.token_efficiency).abs() < f64::EPSILON);
    }

    #[test]
    fn evaluation_tick_every_1000() {
        let engine = SelectionEngine::new(SelectionConfig::default(), 100_000);
        assert!(!engine.is_evaluation_tick(999));
        assert!(engine.is_evaluation_tick(1000));
        assert!(!engine.is_evaluation_tick(1500));
        assert!(engine.is_evaluation_tick(2000));
    }

    #[test]
    fn decline_after_inactivity() {
        let mut engine = SelectionEngine::new(SelectionConfig {
            inactivity_threshold: 500,
            ..Default::default()
        }, 100_000);
        let agent = make_agent(AgentPhase::Adult, 100, 0);
        engine.mark_active(&agent.0.to_string(), 100);

        assert!(!engine.is_in_decline(&agent.0.to_string(), 599));
        assert!(engine.is_in_decline(&agent.0.to_string(), 600));
    }

    #[test]
    fn evaluate_cycle_culls_low_fitness() {
        let mut engine = SelectionEngine::new(SelectionConfig {
            evaluation_interval: 1000,
            over_capacity_cull_rate: 0.5,
            max_agents: 1, // trigger over-capacity
            ..Default::default()
        }, 100_000);

        let a1 = make_agent(AgentPhase::Adult, 90_000, 5); // high fitness
        let a2 = make_agent(AgentPhase::Adult, 10_000, 0); // low fitness

        engine.mark_active(&a1.0.to_string(), 0);
        engine.mark_active(&a2.0.to_string(), 0);

        let agents = vec![a1, a2];
        let (reports, culled) = engine.evaluate_cycle(1000, &agents, 100_000, 100_000);

        assert_eq!(reports.len(), 2);
        // Lowest score first
        assert!(reports[0].score <= reports[1].score);
        // At least one should be culled due to over-capacity
        assert!(!culled.is_empty());
    }

    #[test]
    fn evaluate_cycle_culls_inactive() {
        let mut engine = SelectionEngine::new(SelectionConfig {
            evaluation_interval: 1000,
            inactivity_threshold: 500,
            ..Default::default()
        }, 100_000);

        let a1 = make_agent(AgentPhase::Adult, 100, 0);
        // Agent active long ago
        engine.mark_active(&a1.0.to_string(), 100);

        let agents = vec![a1];
        let (_, culled) = engine.evaluate_cycle(1000, &agents, 100_000, 100_000);

        assert!(culled.contains(&agents[0].0.to_string()));
    }

    #[test]
    fn no_culling_without_pressure() {
        let mut engine = SelectionEngine::new(SelectionConfig {
            evaluation_interval: 1000,
            max_agents: 100,
            base_cull_rate: 0.0,
            over_capacity_cull_rate: 0.0,
            inactivity_threshold: 500,
            ..Default::default()
        }, 100_000);

        let a1 = make_agent(AgentPhase::Adult, 100, 0);
        engine.mark_active(&a1.0.to_string(), 600); // recently active (within threshold)

        let agents = vec![a1];
        let (_, culled) = engine.evaluate_cycle(1000, &agents, 100_000, 100_000);

        assert!(culled.is_empty());
    }
}
