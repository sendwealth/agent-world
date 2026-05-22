//! Skill mutation engine — individual mutations + offspring mutation system.
//!
//! Individual mutations: every evaluation cycle (default: every 1000 ticks),
//! each agent has a configurable probability (default: 5%) of a mutation.
//! Mutations can generate a new random skill or modify an existing one.
//!
//! Offspring mutations: when a new agent is born, it inherits skills from
//! parents and may experience mutations beyond simple inheritance:
//! - **Personality mutation**: a dimension of the personality vector shifts.
//! - **Skill level anomaly**: an inherited skill jumps up or drops down.
//! - **Environmental pressure**: resource scarcity increases the mutation rate.
//! - **Heritable mutations**: parent mutations can strengthen or disappear.

use std::collections::HashMap;

use rand::Rng;
use serde::{Deserialize, Serialize};

use super::skill_tree::SkillTree;

// ═══════════════════════════════════════════════════════════════════════════
// Individual mutations
// ═══════════════════════════════════════════════════════════════════════════

/// The type of mutation that occurred.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MutationType {
    /// A brand-new skill was generated.
    NewSkill,
    /// An existing skill gained bonus XP.
    SkillBoost,
    /// An existing skill lost some XP (rare).
    SkillDecay,
}

/// Result of a mutation event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MutationOutcome {
    pub mutation_type: MutationType,
    pub skill_name: String,
    pub description: String,
}

// ═══════════════════════════════════════════════════════════════════════════
// Offspring mutations
// ═══════════════════════════════════════════════════════════════════════════

/// The type of offspring-specific mutation.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OffspringMutationType {
    /// A personality dimension shifted beyond the inherited range.
    PersonalityShift,
    /// An inherited skill jumped in level (upward anomaly).
    SkillLevelJump,
    /// An inherited skill dropped in level (downward anomaly).
    SkillLevelDrop,
    /// A new skill appeared that neither parent had.
    NovelSkill,
}

/// A single mutation applied during offspring generation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OffspringMutation {
    pub mutation_type: OffspringMutationType,
    /// For skill mutations, the affected skill name.
    pub skill_name: Option<String>,
    /// For personality mutations, the index of the shifted dimension.
    pub personality_dimension: Option<usize>,
    /// Magnitude of the change (positive or negative).
    pub magnitude: f64,
    pub description: String,
}

/// Result of applying offspring mutations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OffspringMutationResult {
    pub offspring_id: String,
    pub parent_a_id: String,
    pub parent_b_id: String,
    pub mutations: Vec<OffspringMutation>,
    /// The effective mutation rate used (after environmental pressure).
    pub effective_mutation_rate: f64,
}

/// Configuration for the offspring mutation subsystem.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OffspringMutationConfig {
    /// Base probability of each offspring mutation (per mutation slot).
    pub base_offspring_mutation_rate: f64,
    /// Maximum number of mutations that can occur per offspring.
    pub max_offspring_mutations: usize,
    /// Number of personality dimensions.
    pub personality_dimensions: usize,
    /// How much a personality dimension can shift per mutation (0.0–1.0).
    pub personality_shift_magnitude: f64,
    /// How many skill levels a jump anomaly can add.
    pub skill_level_jump_range: u32,
    /// How many skill levels a drop anomaly can subtract.
    pub skill_level_drop_range: u32,
    /// Environmental pressure multiplier when world resources are scarce.
    /// Applied as: `rate * (1.0 + env_pressure_multiplier * scarcity)`.
    pub env_pressure_multiplier: f64,
    /// Probability that a parent's heritable mutation strengthens in offspring.
    pub heritable_strengthen_chance: f64,
    /// Probability that a parent's heritable mutation disappears in offspring.
    pub heritable_disappear_chance: f64,
}

impl Default for OffspringMutationConfig {
    fn default() -> Self {
        Self {
            base_offspring_mutation_rate: 0.15,
            max_offspring_mutations: 3,
            personality_dimensions: 5,
            personality_shift_magnitude: 0.2,
            skill_level_jump_range: 2,
            skill_level_drop_range: 1,
            env_pressure_multiplier: 2.0,
            heritable_strengthen_chance: 0.3,
            heritable_disappear_chance: 0.2,
        }
    }
}

/// Tracks heritable mutations that can propagate through generations.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HeritableMutation {
    pub mutation_type: OffspringMutationType,
    pub skill_name: Option<String>,
    /// Number of generations this mutation has persisted.
    pub generations: u32,
    /// Current strength multiplier (can increase or decrease across generations).
    pub strength: f64,
}

/// Resource scarcity level for environmental pressure calculation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EnvironmentPressure {
    /// Ratio of remaining world tokens to initial tokens (0.0–1.0).
    /// Lower values mean more scarcity → higher mutation rate.
    pub token_ratio: f64,
    /// Ratio of current agent count to max_agents (0.0+, may exceed 1.0).
    pub population_pressure: f64,
}

impl EnvironmentPressure {
    /// Compute a combined scarcity score (0.0–1.0), where 1.0 = maximum pressure.
    pub fn scarcity(&self) -> f64 {
        let token_scarcity = 1.0 - self.token_ratio.clamp(0.0, 1.0);
        let pop_scarcity = (self.population_pressure - 1.0).clamp(0.0, 1.0);
        // Weighted combination: token scarcity is the primary driver
        (token_scarcity * 0.7 + pop_scarcity * 0.3).clamp(0.0, 1.0)
    }
}

/// The mutation engine, responsible for generating random skill mutations.
#[derive(Clone)]
pub struct MutationEngine {
    /// Probability of a mutation per agent per evaluation cycle (0.0–1.0).
    pub mutation_rate: f64,
    /// Reference skill tree for picking valid skill names.
    skill_tree: SkillTree,
}

impl MutationEngine {
    pub fn new(mutation_rate: f64, skill_tree: SkillTree) -> Self {
        Self {
            mutation_rate,
            skill_tree,
        }
    }

    /// Attempt a mutation for one agent. Returns `Some(MutationOutcome)` if a
    /// mutation occurs, `None` otherwise.
    ///
    /// Uses deterministic RNG from the provided `Rng`.
    pub fn try_mutation<R: Rng + ?Sized>(
        &self,
        rng: &mut R,
        existing_skills: &HashMap<String, (u32, f64)>,
    ) -> Option<MutationOutcome> {
        // Roll against mutation rate
        if rng.gen::<f64>() >= self.mutation_rate {
            return None;
        }

        // Decide mutation type: 60% new skill, 25% boost, 15% decay
        let roll = rng.gen::<f64>();
        if roll < 0.60 {
            self.mutation_new_skill(rng, existing_skills)
        } else if roll < 0.85 {
            self.mutation_boost(rng, existing_skills)
        } else {
            self.mutation_decay(rng, existing_skills)
        }
    }

    /// Generate a new skill the agent doesn't already have.
    fn mutation_new_skill<R: Rng + ?Sized>(
        &self,
        rng: &mut R,
        existing_skills: &HashMap<String, (u32, f64)>,
    ) -> Option<MutationOutcome> {
        let all_skills = self.skill_tree.all_skill_names();
        let missing: Vec<&String> = all_skills
            .iter()
            .filter(|name| !existing_skills.contains_key(*name))
            .collect();

        if missing.is_empty() {
            // Agent has all skills; give a boost instead
            return self.mutation_boost(rng, existing_skills);
        }

        let idx = rng.gen_range(0..missing.len());
        let skill_name = missing[idx].clone();

        Some(MutationOutcome {
            mutation_type: MutationType::NewSkill,
            skill_name: skill_name.clone(),
            description: format!("Mutation: acquired new skill '{}'", skill_name),
        })
    }

    /// Boost an existing skill with bonus XP.
    fn mutation_boost<R: Rng + ?Sized>(
        &self,
        rng: &mut R,
        existing_skills: &HashMap<String, (u32, f64)>,
    ) -> Option<MutationOutcome> {
        if existing_skills.is_empty() {
            return self.mutation_new_skill(rng, existing_skills);
        }

        let skills: Vec<&String> = existing_skills.keys().collect();
        let idx = rng.gen_range(0..skills.len());
        let skill_name = skills[idx].clone();

        Some(MutationOutcome {
            mutation_type: MutationType::SkillBoost,
            skill_name: skill_name.clone(),
            description: format!("Mutation: skill '{}' gained bonus XP", skill_name),
        })
    }

    /// Decay an existing skill (lose some XP).
    fn mutation_decay<R: Rng + ?Sized>(
        &self,
        rng: &mut R,
        existing_skills: &HashMap<String, (u32, f64)>,
    ) -> Option<MutationOutcome> {
        if existing_skills.is_empty() {
            return self.mutation_new_skill(rng, existing_skills);
        }

        let skills: Vec<&String> = existing_skills.keys().collect();
        let idx = rng.gen_range(0..skills.len());
        let skill_name = skills[idx].clone();

        Some(MutationOutcome {
            mutation_type: MutationType::SkillDecay,
            skill_name: skill_name.clone(),
            description: format!("Mutation: skill '{}' lost XP", skill_name),
        })
    }

    /// XP amount to apply for a boost mutation (scaled by level).
    pub fn boost_xp_amount(&self, current_level: u32) -> f64 {
        // Higher-level skills get bigger boosts to stay meaningful
        50.0 * (1.0 + current_level as f64 * 0.5)
    }

    /// XP amount to subtract for a decay mutation.
    pub fn decay_xp_amount(&self, current_level: u32) -> f64 {
        // Decays are smaller than boosts
        25.0 * (1.0 + current_level as f64 * 0.3)
    }

    /// XP granted when a new skill is acquired via mutation.
    pub fn new_skill_initial_xp(&self) -> f64 {
        50.0
    }

    // ── Offspring mutation methods ───────────────────────────────────────

    /// Compute the effective mutation rate for offspring, accounting for
    /// environmental pressure.
    ///
    /// Formula: `base_rate * (1.0 + env_pressure_multiplier * scarcity)`
    pub fn effective_offspring_rate(
        base_rate: f64,
        config: &OffspringMutationConfig,
        env: &EnvironmentPressure,
    ) -> f64 {
        let scarcity = env.scarcity();
        let rate = base_rate * (1.0 + config.env_pressure_multiplier * scarcity);
        rate.clamp(0.0, 1.0)
    }

    /// Apply offspring mutations to inherited skill/personality data.
    ///
    /// Returns a list of mutations that actually occurred. The caller is
    /// responsible for applying the mutations to the offspring's records.
    pub fn apply_offspring_mutations<R: Rng + ?Sized>(
        &self,
        rng: &mut R,
        config: &OffspringMutationConfig,
        env: &EnvironmentPressure,
        inherited_skills: &HashMap<String, (u32, f64)>,
        parent_heritable: &[HeritableMutation],
        offspring_id: &str,
        parent_a_id: &str,
        parent_b_id: &str,
    ) -> OffspringMutationResult {
        let effective_rate = Self::effective_offspring_rate(
            config.base_offspring_mutation_rate,
            config,
            env,
        );

        let mut mutations = Vec::new();

        // Roll for each potential mutation slot (up to max_offspring_mutations)
        for _ in 0..config.max_offspring_mutations {
            if rng.gen::<f64>() >= effective_rate {
                continue;
            }

            // Pick mutation type: 30% personality, 25% skill jump, 20% skill drop, 25% novel skill
            let roll = rng.gen::<f64>();
            let mutation = if roll < 0.30 {
                self.offspring_personality_mutation(rng, config)
            } else if roll < 0.55 {
                self.offspring_skill_jump(rng, config, inherited_skills)
            } else if roll < 0.75 {
                self.offspring_skill_drop(rng, config, inherited_skills)
            } else {
                self.offspring_novel_skill(rng, inherited_skills)
            };

            if let Some(m) = mutation {
                mutations.push(m);
            }
        }

        // Process heritable mutations from parents
        for parent_mutation in parent_heritable {
            let roll = rng.gen::<f64>();
            if roll < config.heritable_disappear_chance {
                // Mutation disappears — nothing to do
                continue;
            }

            let strengthened = roll < config.heritable_strengthen_chance + config.heritable_disappear_chance;
            if strengthened {
                // Mutation strengthens: re-apply with increased magnitude
                let mut m = OffspringMutation {
                    mutation_type: parent_mutation.mutation_type,
                    skill_name: parent_mutation.skill_name.clone(),
                    personality_dimension: None,
                    magnitude: parent_mutation.strength * 1.5,
                    description: format!(
                        "Heritable mutation strengthened (gen {}, strength {:.2})",
                        parent_mutation.generations + 1,
                        parent_mutation.strength * 1.5,
                    ),
                };
                if matches!(parent_mutation.mutation_type, OffspringMutationType::PersonalityShift) {
                    let dim = rng.gen_range(0..config.personality_dimensions);
                    m.personality_dimension = Some(dim);
                }
                mutations.push(m);
            }
        }

        OffspringMutationResult {
            offspring_id: offspring_id.to_string(),
            parent_a_id: parent_a_id.to_string(),
            parent_b_id: parent_b_id.to_string(),
            effective_mutation_rate: effective_rate,
            mutations,
        }
    }

    /// Generate a personality shift mutation.
    fn offspring_personality_mutation<R: Rng + ?Sized>(
        &self,
        rng: &mut R,
        config: &OffspringMutationConfig,
    ) -> Option<OffspringMutation> {
        let dim = rng.gen_range(0..config.personality_dimensions);
        let shift = (rng.gen::<f64>() - 0.5) * 2.0 * config.personality_shift_magnitude;

        Some(OffspringMutation {
            mutation_type: OffspringMutationType::PersonalityShift,
            skill_name: None,
            personality_dimension: Some(dim),
            magnitude: shift,
            description: format!(
                "Personality dimension {} shifted by {:.3}",
                dim, shift,
            ),
        })
    }

    /// Generate a skill level jump mutation (upward anomaly).
    fn offspring_skill_jump<R: Rng + ?Sized>(
        &self,
        rng: &mut R,
        config: &OffspringMutationConfig,
        inherited_skills: &HashMap<String, (u32, f64)>,
    ) -> Option<OffspringMutation> {
        if inherited_skills.is_empty() {
            return None;
        }

        let skills: Vec<&String> = inherited_skills.keys().collect();
        let idx = rng.gen_range(0..skills.len());
        let skill_name = skills[idx].clone();
        let jump = rng.gen_range(1..=config.skill_level_jump_range) as f64;

        Some(OffspringMutation {
            mutation_type: OffspringMutationType::SkillLevelJump,
            skill_name: Some(skill_name.clone()),
            personality_dimension: None,
            magnitude: jump,
            description: format!(
                "Skill '{}' jumped {} level(s) (anomaly)",
                skill_name, jump as u32,
            ),
        })
    }

    /// Generate a skill level drop mutation (downward anomaly).
    fn offspring_skill_drop<R: Rng + ?Sized>(
        &self,
        rng: &mut R,
        config: &OffspringMutationConfig,
        inherited_skills: &HashMap<String, (u32, f64)>,
    ) -> Option<OffspringMutation> {
        if inherited_skills.is_empty() {
            return None;
        }

        let skills: Vec<&String> = inherited_skills.keys().collect();
        let idx = rng.gen_range(0..skills.len());
        let skill_name = skills[idx].clone();
        let drop = rng.gen_range(1..=config.skill_level_drop_range) as f64;

        Some(OffspringMutation {
            mutation_type: OffspringMutationType::SkillLevelDrop,
            skill_name: Some(skill_name.clone()),
            personality_dimension: None,
            magnitude: -drop,
            description: format!(
                "Skill '{}' dropped {} level(s) (anomaly)",
                skill_name, drop as u32,
            ),
        })
    }

    /// Generate a novel skill mutation (neither parent had this skill).
    fn offspring_novel_skill<R: Rng + ?Sized>(
        &self,
        rng: &mut R,
        inherited_skills: &HashMap<String, (u32, f64)>,
    ) -> Option<OffspringMutation> {
        let all_skills = self.skill_tree.all_skill_names();
        let missing: Vec<&String> = all_skills
            .iter()
            .filter(|name| !inherited_skills.contains_key(*name))
            .collect();

        if missing.is_empty() {
            return None;
        }

        let idx = rng.gen_range(0..missing.len());
        let skill_name = missing[idx].clone();

        Some(OffspringMutation {
            mutation_type: OffspringMutationType::NovelSkill,
            skill_name: Some(skill_name.clone()),
            personality_dimension: None,
            magnitude: 1.0,
            description: format!(
                "Novel skill '{}' appeared (neither parent had it)",
                skill_name,
            ),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    fn seeded_rng() -> StdRng {
        StdRng::seed_from_u64(42)
    }

    fn make_engine(rate: f64) -> MutationEngine {
        MutationEngine::new(rate, SkillTree::default())
    }

    // ── Individual mutation tests ────────────────────────────────────────

    #[test]
    fn zero_rate_never_mutates() {
        let engine = make_engine(0.0);
        let mut rng = seeded_rng();
        let skills = HashMap::new();
        let result = engine.try_mutation(&mut rng, &skills);
        assert!(result.is_none());
    }

    #[test]
    fn full_rate_always_mutates() {
        let engine = make_engine(1.0);
        let mut rng = seeded_rng();
        let skills = HashMap::new();
        let result = engine.try_mutation(&mut rng, &skills);
        assert!(result.is_some());
    }

    #[test]
    fn new_skill_mutation_for_empty_agent() {
        let engine = make_engine(1.0);
        let mut rng = seeded_rng();
        let skills = HashMap::new();
        let result = engine.try_mutation(&mut rng, &skills);
        assert!(result.is_some());
        if let Some(outcome) = result {
            assert!(matches!(outcome.mutation_type, MutationType::NewSkill));
        }
    }

    #[test]
    fn boost_mutation_for_skilled_agent() {
        let engine = make_engine(1.0);
        let mut rng = StdRng::seed_from_u64(12345);
        let mut skills = HashMap::new();
        for name in engine.skill_tree.all_skill_names() {
            skills.insert(name, (1, 0.0));
        }
        let result = engine.try_mutation(&mut rng, &skills);
        assert!(result.is_some());
    }

    #[test]
    fn boost_xp_scales_with_level() {
        let engine = make_engine(0.05);
        assert!(engine.boost_xp_amount(5) > engine.boost_xp_amount(1));
    }

    #[test]
    fn decay_xp_less_than_boost() {
        let engine = make_engine(0.05);
        assert!(engine.decay_xp_amount(5) < engine.boost_xp_amount(5));
    }

    #[test]
    fn new_skill_initial_xp_positive() {
        let engine = make_engine(0.05);
        assert!(engine.new_skill_initial_xp() > 0.0);
    }

    // ── Offspring mutation tests ─────────────────────────────────────────

    #[test]
    fn environment_pressure_scarcity_no_pressure() {
        let env = EnvironmentPressure {
            token_ratio: 1.0,
            population_pressure: 0.5,
        };
        assert!((env.scarcity() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn environment_pressure_scarcity_full_pressure() {
        let env = EnvironmentPressure {
            token_ratio: 0.0,
            population_pressure: 2.0,
        };
        assert!((env.scarcity() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn environment_pressure_scarcity_partial() {
        let env = EnvironmentPressure {
            token_ratio: 0.5,
            population_pressure: 1.0,
        };
        // token_scarcity = 0.5, pop_scarcity = 0.0 → 0.5 * 0.7 + 0.0 * 0.3 = 0.35
        assert!((env.scarcity() - 0.35).abs() < 1e-10);
    }

    #[test]
    fn effective_offspring_rate_no_pressure() {
        let config = OffspringMutationConfig::default();
        let env = EnvironmentPressure {
            token_ratio: 1.0,
            population_pressure: 0.5,
        };
        let rate = MutationEngine::effective_offspring_rate(0.15, &config, &env);
        // base_rate * (1.0 + 2.0 * 0.0) = 0.15
        assert!((rate - 0.15).abs() < f64::EPSILON);
    }

    #[test]
    fn effective_offspring_rate_high_pressure() {
        let config = OffspringMutationConfig::default();
        let env = EnvironmentPressure {
            token_ratio: 0.0,
            population_pressure: 2.0,
        };
        let rate = MutationEngine::effective_offspring_rate(0.15, &config, &env);
        // base_rate * (1.0 + 2.0 * 1.0) = 0.45
        assert!((rate - 0.45).abs() < f64::EPSILON);
    }

    #[test]
    fn effective_offspring_rate_clamped_to_one() {
        let config = OffspringMutationConfig {
            env_pressure_multiplier: 10.0,
            ..Default::default()
        };
        let env = EnvironmentPressure {
            token_ratio: 0.0,
            population_pressure: 2.0,
        };
        let rate = MutationEngine::effective_offspring_rate(0.5, &config, &env);
        // 0.5 * (1.0 + 10.0 * 1.0) = 5.5 → clamped to 1.0
        assert!((rate - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn offspring_mutations_zero_rate_none() {
        let engine = make_engine(0.05);
        let config = OffspringMutationConfig {
            base_offspring_mutation_rate: 0.0,
            ..Default::default()
        };
        let env = EnvironmentPressure {
            token_ratio: 1.0,
            population_pressure: 0.5,
        };
        let mut rng = seeded_rng();
        let skills = HashMap::new();

        let result = engine.apply_offspring_mutations(
            &mut rng, &config, &env, &skills, &[],
            "offspring-1", "parent-a", "parent-b",
        );

        assert!(result.mutations.is_empty());
        assert!((result.effective_mutation_rate - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn offspring_mutations_full_rate_guaranteed() {
        let engine = make_engine(0.05);
        let config = OffspringMutationConfig {
            base_offspring_mutation_rate: 1.0,
            max_offspring_mutations: 3,
            ..Default::default()
        };
        let env = EnvironmentPressure {
            token_ratio: 1.0,
            population_pressure: 0.5,
        };
        let mut rng = seeded_rng();
        let mut skills = HashMap::new();
        skills.insert("coding".to_string(), (3, 100.0));

        let result = engine.apply_offspring_mutations(
            &mut rng, &config, &env, &skills, &[],
            "offspring-1", "parent-a", "parent-b",
        );

        // With rate 1.0, should produce up to max_offspring_mutations mutations
        assert!(!result.mutations.is_empty());
        assert!(result.mutations.len() <= config.max_offspring_mutations);
    }

    #[test]
    fn offspring_personality_mutation_bounded() {
        let engine = make_engine(0.05);
        let config = OffspringMutationConfig {
            personality_dimensions: 5,
            personality_shift_magnitude: 0.2,
            ..Default::default()
        };
        let mut rng = seeded_rng();

        if let Some(m) = engine.offspring_personality_mutation(&mut rng, &config) {
            assert!(m.personality_dimension.unwrap() < 5);
            assert!(m.magnitude.abs() <= 0.2);
        }
    }

    #[test]
    fn offspring_skill_jump_no_skills() {
        let engine = make_engine(0.05);
        let config = OffspringMutationConfig::default();
        let mut rng = seeded_rng();
        let skills = HashMap::new();

        let result = engine.offspring_skill_jump(&mut rng, &config, &skills);
        assert!(result.is_none());
    }

    #[test]
    fn offspring_skill_drop_no_skills() {
        let engine = make_engine(0.05);
        let config = OffspringMutationConfig::default();
        let mut rng = seeded_rng();
        let skills = HashMap::new();

        let result = engine.offspring_skill_drop(&mut rng, &config, &skills);
        assert!(result.is_none());
    }

    #[test]
    fn offspring_skill_jump_with_skills() {
        let engine = make_engine(0.05);
        let config = OffspringMutationConfig {
            skill_level_jump_range: 2,
            ..Default::default()
        };
        let mut rng = seeded_rng();
        let mut skills = HashMap::new();
        skills.insert("coding".to_string(), (3, 100.0));

        let result = engine.offspring_skill_jump(&mut rng, &config, &skills);
        assert!(result.is_some());
        let m = result.unwrap();
        assert!(matches!(m.mutation_type, OffspringMutationType::SkillLevelJump));
        assert_eq!(m.skill_name.as_deref(), Some("coding"));
        assert!(m.magnitude >= 1.0 && m.magnitude <= 2.0);
    }

    #[test]
    fn offspring_skill_drop_with_skills() {
        let engine = make_engine(0.05);
        let config = OffspringMutationConfig {
            skill_level_drop_range: 1,
            ..Default::default()
        };
        let mut rng = seeded_rng();
        let mut skills = HashMap::new();
        skills.insert("coding".to_string(), (3, 100.0));

        let result = engine.offspring_skill_drop(&mut rng, &config, &skills);
        assert!(result.is_some());
        let m = result.unwrap();
        assert!(matches!(m.mutation_type, OffspringMutationType::SkillLevelDrop));
        assert!((m.magnitude - (-1.0)).abs() < f64::EPSILON);
    }

    #[test]
    fn offspring_novel_skill_picks_missing() {
        let engine = make_engine(0.05);
        let mut rng = seeded_rng();
        let mut skills = HashMap::new();
        skills.insert("coding".to_string(), (1, 0.0));

        let result = engine.offspring_novel_skill(&mut rng, &skills);
        assert!(result.is_some());
        let m = result.unwrap();
        assert!(matches!(m.mutation_type, OffspringMutationType::NovelSkill));
        // Should not be "coding" since the parent had it
        assert_ne!(m.skill_name.as_deref(), Some("coding"));
    }

    #[test]
    fn offspring_novel_skill_none_when_all_known() {
        let engine = make_engine(0.05);
        let mut rng = seeded_rng();
        let mut skills = HashMap::new();
        for name in engine.skill_tree.all_skill_names() {
            skills.insert(name, (1, 0.0));
        }

        let result = engine.offspring_novel_skill(&mut rng, &skills);
        assert!(result.is_none());
    }

    #[test]
    fn heritable_mutation_can_strengthen() {
        let engine = make_engine(0.05);
        let config = OffspringMutationConfig {
            base_offspring_mutation_rate: 0.0, // disable random mutations
            heritable_strengthen_chance: 1.0,
            heritable_disappear_chance: 0.0,
            ..Default::default()
        };
        let env = EnvironmentPressure {
            token_ratio: 1.0,
            population_pressure: 0.5,
        };
        let mut rng = StdRng::seed_from_u64(999);
        let skills = HashMap::new();

        let heritable = vec![HeritableMutation {
            mutation_type: OffspringMutationType::PersonalityShift,
            skill_name: None,
            generations: 1,
            strength: 0.3,
        }];

        let result = engine.apply_offspring_mutations(
            &mut rng, &config, &env, &skills, &heritable,
            "offspring-1", "parent-a", "parent-b",
        );

        assert_eq!(result.mutations.len(), 1);
        assert!((result.mutations[0].magnitude - 0.45).abs() < f64::EPSILON);
    }

    #[test]
    fn heritable_mutation_can_disappear() {
        let engine = make_engine(0.05);
        let config = OffspringMutationConfig {
            base_offspring_mutation_rate: 0.0,
            heritable_strengthen_chance: 0.0,
            heritable_disappear_chance: 1.0,
            ..Default::default()
        };
        let env = EnvironmentPressure {
            token_ratio: 1.0,
            population_pressure: 0.5,
        };
        let mut rng = seeded_rng();
        let skills = HashMap::new();

        let heritable = vec![HeritableMutation {
            mutation_type: OffspringMutationType::SkillLevelJump,
            skill_name: Some("coding".to_string()),
            generations: 2,
            strength: 0.5,
        }];

        let result = engine.apply_offspring_mutations(
            &mut rng, &config, &env, &skills, &heritable,
            "offspring-1", "parent-a", "parent-b",
        );

        assert!(result.mutations.is_empty());
    }

    #[test]
    fn offspring_mutation_result_fields() {
        let engine = make_engine(0.05);
        let config = OffspringMutationConfig {
            base_offspring_mutation_rate: 1.0,
            max_offspring_mutations: 1,
            ..Default::default()
        };
        let env = EnvironmentPressure {
            token_ratio: 1.0,
            population_pressure: 0.5,
        };
        let mut rng = seeded_rng();
        let skills = HashMap::new();

        let result = engine.apply_offspring_mutations(
            &mut rng, &config, &env, &skills, &[],
            "offspring-x", "mom", "dad",
        );

        assert_eq!(result.offspring_id, "offspring-x");
        assert_eq!(result.parent_a_id, "mom");
        assert_eq!(result.parent_b_id, "dad");
    }

    #[test]
    fn environment_pressure_scarcity_clamped() {
        // token_ratio > 1.0 should clamp to 1.0
        let env = EnvironmentPressure {
            token_ratio: 2.0,
            population_pressure: 0.0,
        };
        assert!((env.scarcity() - 0.0).abs() < f64::EPSILON);
    }
}
