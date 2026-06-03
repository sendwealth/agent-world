//! Evolution subsystem — integrates skill trees, mutations, and natural
//! selection into the tick loop.

use std::collections::HashMap;
use std::sync::Mutex;

use rand::rngs::StdRng;
use rand::SeedableRng;
use uuid::Uuid;

use crate::economy::token_burn::AgentRecord;
use crate::world::enums::AgentPhase;
use crate::world::event::WorldEvent;
use crate::world::subsystem::Subsystem;

use super::crossover::{CrossoverConfig, CrossoverEngine};
use super::mutation::MutationEngine;
use super::mutation::MutationType;
use super::mutation::OffspringMutationConfig;
use super::selection::SelectionEngine;
use super::skill_tree::{SkillNode, SkillTree};

/// The evolution subsystem that participates in the tick loop.
///
/// On each tick it:
/// 1. Passively accumulates skill XP for living agents.
/// 2. On evaluation cycles (every N ticks), runs mutation and natural selection.
/// 3. Provides crossover/offspring mutation for reproduction events.
/// 4. Emits evolution-related events.
pub struct EvolutionSubsystem {
    /// Mutation engine (internally holds the skill tree).
    mutation: MutationEngine,
    /// Crossover engine for offspring generation.
    #[allow(dead_code)]
    crossover: CrossoverEngine,
    /// Natural selection engine.
    selection: Mutex<SelectionEngine>,
    /// RNG state.
    rng: Mutex<StdRng>,
    /// Config values.
    config: EvolutionSubsystemConfig,
    /// Tracks previous world token total for culling pressure detection.
    prev_world_tokens: Mutex<u64>,
    /// Social connection counts per agent (trust edges + org memberships).
    /// Updated externally via `update_social_connections`.
    social_connections: Mutex<HashMap<String, usize>>,
}

/// Configuration for the evolution subsystem.
pub struct EvolutionSubsystemConfig {
    /// Maximum skill level (matches `EvolutionConfig::skill_max_level`).
    pub skill_max_level: u32,
    /// Mutation probability per agent per evaluation cycle.
    pub mutation_rate: f64,
    /// Evaluation interval in ticks.
    pub evaluation_interval: u64,
    /// Maximum agents before over-capacity culling.
    pub max_agents: u32,
    /// Inactivity threshold in ticks.
    pub inactivity_threshold: u64,
    /// Initial tokens for fitness calculation.
    pub initial_tokens: u64,
    /// XP granted passively per tick to each living agent's skills.
    pub passive_xp_per_tick: f64,
    /// XP bonus for a boost mutation.
    pub mutation_boost_xp: f64,
    /// XP penalty for a decay mutation.
    pub mutation_decay_xp: f64,
    /// XP granted for a new skill mutation.
    pub mutation_new_skill_xp: f64,
    /// Offspring mutation configuration.
    pub offspring_mutation: OffspringMutationConfig,
    /// Crossover strategy for offspring skill inheritance.
    pub crossover_personality_blend: f64,
}

impl Default for EvolutionSubsystemConfig {
    fn default() -> Self {
        Self {
            skill_max_level: 10,
            mutation_rate: 0.05,
            evaluation_interval: 1000,
            max_agents: 100,
            inactivity_threshold: 500,
            initial_tokens: 100_000,
            passive_xp_per_tick: 1.0,
            mutation_boost_xp: 75.0,
            mutation_decay_xp: 30.0,
            mutation_new_skill_xp: 50.0,
            offspring_mutation: OffspringMutationConfig::default(),
            crossover_personality_blend: 0.5,
        }
    }
}

impl EvolutionSubsystem {
    pub fn new(config: EvolutionSubsystemConfig) -> Self {
        let skill_tree = SkillTree::new(config.skill_max_level);
        let mutation = MutationEngine::new(config.mutation_rate, skill_tree.clone());

        let crossover_config = CrossoverConfig {
            personality_blend: config.crossover_personality_blend,
            apply_mutations: true,
            mutation_config: config.offspring_mutation.clone(),
            ..Default::default()
        };
        let crossover = CrossoverEngine::new(crossover_config, mutation.clone());

        let selection_config = super::selection::SelectionConfig {
            evaluation_interval: config.evaluation_interval,
            inactivity_threshold: config.inactivity_threshold,
            max_agents: config.max_agents,
            ..Default::default()
        };
        let selection = SelectionEngine::new(selection_config, config.initial_tokens);

        Self {
            mutation,
            crossover,
            selection: Mutex::new(selection),
            rng: Mutex::new(StdRng::from_entropy()),
            config,
            prev_world_tokens: Mutex::new(0),
            social_connections: Mutex::new(HashMap::new()),
        }
    }

    /// Update the social connection counts used during fitness evaluation.
    ///
    /// Call this before an evaluation tick with the number of social
    /// connections (trust edges, organization memberships, etc.) per agent.
    pub fn update_social_connections(&self, connections: HashMap<String, usize>) {
        let mut sc = self.social_connections.lock().unwrap();
        *sc = connections;
    }

    /// Award passive XP to all living agents' skills.
    fn passive_xp(&self, agents: &mut [(Uuid, u64, AgentRecord)]) -> Vec<WorldEvent> {
        let mut events = Vec::new();

        for (_, _, agent) in agents.iter_mut() {
            if agent.phase == AgentPhase::Dead || agent.phase == AgentPhase::Birth {
                continue;
            }

            let xp = self.config.passive_xp_per_tick;
            for skill in agent.skills.values_mut() {
                let old_level = skill.level;
                let max_level = self.config.skill_max_level;

                if skill.level < max_level {
                    skill.experience += xp;

                    // Check for level up
                    while skill.level < max_level {
                        let threshold = SkillNode::xp_threshold_for_level(skill.level);
                        // Note: we reuse the SkillNode threshold formula
                        // 100 * 2^(level-1)
                        if skill.experience >= threshold {
                            skill.experience -= threshold;
                            skill.level += 1;
                        } else {
                            break;
                        }
                    }

                    if skill.level > old_level {
                        events.push(WorldEvent::SkillLevelUp {
                            agent_id: agent.id.to_string(),
                            skill: skill.name.clone(),
                            new_level: skill.level,
                        });
                    }
                }
            }
        }

        events
    }

    /// Run mutation + selection on an evaluation cycle.
    fn run_evaluation_cycle(
        &self,
        tick: u64,
        agents: &mut [(Uuid, u64, AgentRecord)],
    ) -> Vec<WorldEvent> {
        let mut events = Vec::new();

        // Compute current world token total
        let current_tokens: u64 = agents.iter().map(|(_, _, a)| a.tokens).sum();

        let prev_tokens = {
            let mut prev = self.prev_world_tokens.lock().unwrap();
            let old = *prev;
            *prev = current_tokens;
            old
        };

        // Phase 1: Mutations
        {
            let mut rng = self.rng.lock().unwrap();
            for (_, _, agent) in agents.iter_mut() {
                if agent.phase == AgentPhase::Dead || agent.phase == AgentPhase::Birth {
                    continue;
                }

                let existing: HashMap<String, (u32, f64)> = agent
                    .skills
                    .iter()
                    .map(|(k, s)| (k.clone(), (s.level, s.experience)))
                    .collect();

                if let Some(outcome) = self.mutation.try_mutation(&mut *rng, &existing) {
                    match outcome.mutation_type {
                        MutationType::NewSkill => {
                            let initial_xp = self.config.mutation_new_skill_xp;
                            agent.skills.insert(
                                outcome.skill_name.clone(),
                                crate::economy::token_burn::SkillRecord {
                                    name: outcome.skill_name.clone(),
                                    level: 1,
                                    experience: initial_xp,
                                },
                            );
                        }
                        MutationType::SkillBoost => {
                            if let Some(skill) = agent.skills.get_mut(&outcome.skill_name) {
                                let boost = self.config.mutation_boost_xp
                                    * (1.0 + skill.level as f64 * 0.5);
                                skill.experience += boost;
                            }
                        }
                        MutationType::SkillDecay => {
                            if let Some(skill) = agent.skills.get_mut(&outcome.skill_name) {
                                let decay = self.config.mutation_decay_xp;
                                skill.experience = (skill.experience - decay).max(0.0);
                                // Level down if XP went negative (clamped)
                                // Don't go below level 1
                            }
                        }
                    }

                    events.push(WorldEvent::SkillMutated {
                        agent_id: agent.id.to_string(),
                        mutation_type: format!("{:?}", outcome.mutation_type),
                        skill: outcome.skill_name.clone(),
                        description: outcome.description.clone(),
                    });
                }
            }
        }

        // Phase 2: Natural selection
        {
            let mut selection = self.selection.lock().unwrap();
            let social = self.social_connections.lock().unwrap();
            let (reports, to_cull) =
                selection.evaluate_cycle(tick, agents, current_tokens, prev_tokens, &social);

            // Emit fitness reports for all evaluated agents
            for report in reports {
                events.push(WorldEvent::FitnessEvaluated {
                    agent_id: report.agent_id.clone(),
                    score: report.score,
                    token_efficiency: report.token_efficiency,
                    survival_duration: report.survival_duration,
                    skill_diversity: report.skill_diversity,
                });
            }

            // Mark culled agents as Dying
            for agent_id_str in to_cull {
                for (_, _, agent) in agents.iter_mut() {
                    if agent.id.to_string() == agent_id_str
                        && agent.phase != AgentPhase::Dead
                        && agent.phase != AgentPhase::Dying
                    {
                        agent.phase = AgentPhase::Dying;
                        events.push(WorldEvent::AgentDying {
                            agent_id: agent.id.to_string(),
                            reason: crate::world::enums::DeathReason::NaturalDeath,
                            grace_ticks: 0,
                        });
                        events.push(WorldEvent::AgentDied {
                            agent_id: agent.id.to_string(),
                            reason: crate::world::enums::DeathReason::NaturalDeath,
                        });
                        agent.phase = AgentPhase::Dead;
                        break;
                    }
                }
            }
        }

        // Update last-active tracking
        {
            let mut selection = self.selection.lock().unwrap();
            for (_, _, agent) in agents.iter() {
                if agent.phase != AgentPhase::Dead && agent.phase != AgentPhase::Birth {
                    selection.mark_active(&agent.id.to_string(), tick);
                }
            }
        }

        events
    }
}

impl Subsystem for EvolutionSubsystem {
    fn name(&self) -> &str {
        "evolution"
    }

    fn on_tick(&self, tick: u64, agents: &mut [(Uuid, u64, AgentRecord)]) -> Vec<WorldEvent> {
        let mut events = Vec::new();

        // Passive XP every tick
        events.extend(self.passive_xp(agents));

        // Evaluation cycle at configured intervals
        if tick > 0 && tick.is_multiple_of(self.config.evaluation_interval) {
            events.extend(self.run_evaluation_cycle(tick, agents));
        }

        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_agent(phase: AgentPhase, tokens: u64) -> (Uuid, u64, AgentRecord) {
        let id = Uuid::new_v4();
        (
            id,
            0,
            AgentRecord {
                id,
                name: "test".to_string(),
                phase,
                tokens,
                skills: HashMap::new(),
                personality: String::new(),
                tasks_completed: 0,
                tasks_attempted: 0,
            },
        )
    }

    fn make_agent_with_skills(
        phase: AgentPhase,
        tokens: u64,
        skills: Vec<(&str, u32, f64)>,
    ) -> (Uuid, u64, AgentRecord) {
        let id = Uuid::new_v4();
        (
            id,
            0,
            AgentRecord {
                id,
                name: "test".to_string(),
                phase,
                tokens,
                skills: skills
                    .into_iter()
                    .map(|(name, level, xp)| {
                        (
                            name.to_string(),
                            crate::economy::token_burn::SkillRecord {
                                name: name.to_string(),
                                level,
                                experience: xp,
                            },
                        )
                    })
                    .collect(),
                personality: String::new(),
                tasks_completed: 0,
                tasks_attempted: 0,
            },
        )
    }

    fn make_subsystem() -> EvolutionSubsystem {
        EvolutionSubsystem::new(EvolutionSubsystemConfig {
            passive_xp_per_tick: 10.0, // faster for testing
            ..Default::default()
        })
    }

    #[test]
    fn subsystem_name() {
        let sub = make_subsystem();
        assert_eq!(sub.name(), "evolution");
    }

    #[test]
    fn passive_xp_accumulates() {
        let sub = make_subsystem();
        let mut agents = vec![make_agent_with_skills(
            AgentPhase::Adult,
            1000,
            vec![("coding", 1, 0.0)],
        )];

        let events = sub.on_tick(1, &mut agents);

        assert!(agents[0].2.skills["coding"].experience > 0.0);
        let _ = events;
    }

    #[test]
    fn passive_xp_skips_dead() {
        let sub = make_subsystem();
        let mut agents = vec![make_agent_with_skills(
            AgentPhase::Dead,
            1000,
            vec![("coding", 1, 0.0)],
        )];

        sub.on_tick(1, &mut agents);

        assert!((agents[0].2.skills["coding"].experience).abs() < f64::EPSILON);
    }

    #[test]
    fn level_up_event_emitted() {
        let sub = make_subsystem();
        // Skill at level 1 with 95 XP, threshold for 1→2 is 100.
        // passive_xp_per_tick is 10, so one tick gives 105 XP → level 2
        let mut agents = vec![make_agent_with_skills(
            AgentPhase::Adult,
            1000,
            vec![("coding", 1, 95.0)],
        )];

        let events = sub.on_tick(1, &mut agents);

        assert_eq!(agents[0].2.skills["coding"].level, 2);
        assert!(events.iter().any(|e| matches!(
            e,
            WorldEvent::SkillLevelUp { skill, new_level: 2, .. } if skill == "coding"
        )));
    }

    #[test]
    fn evaluation_cycle_at_interval() {
        let sub = make_subsystem();
        let mut agents = vec![make_agent(AgentPhase::Adult, 1000)];

        // Tick 999 should not trigger evaluation
        let events = sub.on_tick(999, &mut agents);
        let has_fitness = events
            .iter()
            .any(|e| matches!(e, WorldEvent::FitnessEvaluated { .. }));
        assert!(!has_fitness);

        // Tick 1000 should trigger evaluation
        let events = sub.on_tick(1000, &mut agents);
        let has_fitness = events
            .iter()
            .any(|e| matches!(e, WorldEvent::FitnessEvaluated { .. }));
        assert!(has_fitness);
    }

    #[test]
    fn mutation_can_occur_at_evaluation() {
        // Use rate 1.0 to guarantee mutation
        let sub = EvolutionSubsystem::new(EvolutionSubsystemConfig {
            mutation_rate: 1.0,
            ..Default::default()
        });
        let mut agents = vec![make_agent(AgentPhase::Adult, 100_000)];

        let events = sub.on_tick(1000, &mut agents);
        let has_mutation = events
            .iter()
            .any(|e| matches!(e, WorldEvent::SkillMutated { .. }));
        assert!(has_mutation);
    }
}
