//! Crossover engine — combines parent skills and personality into offspring.
//!
//! When two agents produce an offspring, this module handles the merging
//! (crossover) of their skills and personality vectors, producing the
//! initial inherited state before offspring mutations are applied.

use std::collections::HashMap;

use rand::Rng;
use serde::{Deserialize, Serialize};

use super::mutation::{
    EnvironmentPressure, HeritableMutation, MutationEngine, OffspringMutation,
    OffspringMutationConfig, OffspringMutationResult, OffspringMutationType,
};

/// Strategy for combining skill levels from two parents.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CrossoverStrategy {
    /// Take the higher skill level from either parent.
    Max,
    /// Average the skill levels (rounded down).
    Average,
    /// Randomly pick one parent's level for each skill independently.
    Random,
}

impl Default for CrossoverStrategy {
    fn default() -> Self {
        Self::Average
    }
}

/// Configuration for the crossover engine.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CrossoverConfig {
    /// How to combine skill levels from parents.
    pub strategy: CrossoverStrategy,
    /// Fraction of parent A's personality to inherit (0.0–1.0).
    /// Parent B contributes `1.0 - personality_blend`.
    pub personality_blend: f64,
    /// Whether to run the offspring mutation engine after crossover.
    pub apply_mutations: bool,
    /// Offspring mutation config (used when `apply_mutations` is true).
    pub mutation_config: OffspringMutationConfig,
}

impl Default for CrossoverConfig {
    fn default() -> Self {
        Self {
            strategy: CrossoverStrategy::Average,
            personality_blend: 0.5,
            apply_mutations: true,
            mutation_config: OffspringMutationConfig::default(),
        }
    }
}

/// Result of a crossover operation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrossoverResult {
    pub offspring_id: String,
    pub parent_a_id: String,
    pub parent_b_id: String,
    /// Inherited skill levels after crossover (before mutations).
    pub inherited_skills: HashMap<String, (u32, f64)>,
    /// Inherited personality vector after crossover (before mutations).
    pub inherited_personality: Vec<f64>,
    /// Mutations applied (empty if `apply_mutations` is false).
    pub mutation_result: Option<OffspringMutationResult>,
}

/// The crossover engine.
pub struct CrossoverEngine {
    config: CrossoverConfig,
    mutation_engine: MutationEngine,
}

impl CrossoverEngine {
    pub fn new(config: CrossoverConfig, mutation_engine: MutationEngine) -> Self {
        Self {
            config,
            mutation_engine,
        }
    }

    /// Perform crossover between two parents, producing an offspring's
    /// initial skill and personality state, then optionally apply mutations.
    pub fn crossover<R: Rng + ?Sized>(
        &self,
        rng: &mut R,
        parent_a_skills: &HashMap<String, (u32, f64)>,
        parent_b_skills: &HashMap<String, (u32, f64)>,
        parent_a_personality: &[f64],
        parent_b_personality: &[f64],
        parent_a_heritable: &[HeritableMutation],
        parent_b_heritable: &[HeritableMutation],
        env: &EnvironmentPressure,
        offspring_id: &str,
        parent_a_id: &str,
        parent_b_id: &str,
    ) -> CrossoverResult {
        let inherited_skills = self.crossover_skills(
            parent_a_skills,
            parent_b_skills,
            &self.config.strategy,
        );

        let inherited_personality = self.crossover_personality(
            parent_a_personality,
            parent_b_personality,
            self.config.personality_blend,
        );

        let mutation_result = if self.config.apply_mutations {
            let combined_heritable: Vec<HeritableMutation> = parent_a_heritable
                .iter()
                .chain(parent_b_heritable.iter())
                .cloned()
                .collect();

            Some(self.mutation_engine.apply_offspring_mutations(
                rng,
                &self.config.mutation_config,
                env,
                &inherited_skills,
                &combined_heritable,
                offspring_id,
                parent_a_id,
                parent_b_id,
            ))
        } else {
            None
        };

        CrossoverResult {
            offspring_id: offspring_id.to_string(),
            parent_a_id: parent_a_id.to_string(),
            parent_b_id: parent_b_id.to_string(),
            inherited_skills,
            inherited_personality,
            mutation_result,
        }
    }

    /// Combine skills from two parents using the given strategy.
    pub fn crossover_skills(
        &self,
        parent_a: &HashMap<String, (u32, f64)>,
        parent_b: &HashMap<String, (u32, f64)>,
        strategy: &CrossoverStrategy,
    ) -> HashMap<String, (u32, f64)> {
        let mut result = HashMap::new();

        // Collect all skill names from both parents
        let all_skills: std::collections::HashSet<&String> =
            parent_a.keys().chain(parent_b.keys()).collect();

        for skill_name in all_skills {
            let a = parent_a.get(skill_name);
            let b = parent_b.get(skill_name);

            let combined = match (a, b) {
                (Some(&(level_a, xp_a)), Some(&(level_b, xp_b))) => match strategy {
                    CrossoverStrategy::Max => {
                        if level_a >= level_b {
                            (level_a, xp_a)
                        } else {
                            (level_b, xp_b)
                        }
                    }
                    CrossoverStrategy::Average => {
                        let level = (level_a + level_b) / 2;
                        let xp = (xp_a + xp_b) / 2.0;
                        (level.max(1), xp)
                    }
                    CrossoverStrategy::Random => {
                        // Caller should use RNG variant; fall back to average
                        let level = (level_a + level_b) / 2;
                        let xp = (xp_a + xp_b) / 2.0;
                        (level.max(1), xp)
                    }
                },
                (Some(&(level, xp)), None) | (None, Some(&(level, xp))) => {
                    (level, xp)
                }
                (None, None) => unreachable!(),
            };

            result.insert(skill_name.clone(), combined);
        }

        result
    }

    /// Combine skills from two parents using a random strategy per skill.
    pub fn crossover_skills_random<R: Rng + ?Sized>(
        &self,
        rng: &mut R,
        parent_a: &HashMap<String, (u32, f64)>,
        parent_b: &HashMap<String, (u32, f64)>,
    ) -> HashMap<String, (u32, f64)> {
        let mut result = HashMap::new();

        let all_skills: std::collections::HashSet<&String> =
            parent_a.keys().chain(parent_b.keys()).collect();

        for skill_name in all_skills {
            let a = parent_a.get(skill_name);
            let b = parent_b.get(skill_name);

            let combined = match (a, b) {
                (Some(&val_a), Some(&val_b)) => {
                    if rng.gen::<bool>() {
                        val_a
                    } else {
                        val_b
                    }
                }
                (Some(&val), None) | (None, Some(&val)) => val,
                (None, None) => unreachable!(),
            };

            result.insert(skill_name.clone(), combined);
        }

        result
    }

    /// Blend two personality vectors using linear interpolation.
    ///
    /// `blend` is the weight for parent_a (0.0 = all parent_b, 1.0 = all parent_a).
    pub fn crossover_personality(
        &self,
        parent_a: &[f64],
        parent_b: &[f64],
        blend: f64,
    ) -> Vec<f64> {
        let len = parent_a.len().max(parent_b.len());
        let mut result = Vec::with_capacity(len);

        for i in 0..len {
            let a = parent_a.get(i).copied().unwrap_or(0.0);
            let b = parent_b.get(i).copied().unwrap_or(0.0);
            let blended = a * blend + b * (1.0 - blend);
            result.push(blended);
        }

        result
    }

    /// Apply offspring mutations to a CrossoverResult, updating inherited
    /// skills and personality in place.
    pub fn apply_mutation_effects(
        &self,
        result: &mut CrossoverResult,
        max_skill_level: u32,
    ) {
        let mutations = result.mutation_result.as_ref()
            .map(|r| r.mutations.clone())
            .unwrap_or_default();

        for m in &mutations {
            match m.mutation_type {
                OffspringMutationType::SkillLevelJump => {
                    if let Some(ref skill_name) = m.skill_name {
                        if let Some((level, _xp)) = result.inherited_skills.get_mut(skill_name) {
                            *level = (*level)
                                .saturating_add(m.magnitude.max(0.0) as u32)
                                .min(max_skill_level);
                        }
                    }
                }
                OffspringMutationType::SkillLevelDrop => {
                    if let Some(ref skill_name) = m.skill_name {
                        if let Some((level, _xp)) = result.inherited_skills.get_mut(skill_name) {
                            *level = (*level)
                                .saturating_sub(m.magnitude.abs().min(*level as f64) as u32)
                                .max(1);
                        }
                    }
                }
                OffspringMutationType::NovelSkill => {
                    if let Some(ref skill_name) = m.skill_name {
                        result.inherited_skills
                            .entry(skill_name.clone())
                            .or_insert((1, 0.0));
                    }
                }
                OffspringMutationType::PersonalityShift => {
                    if let Some(dim) = m.personality_dimension {
                        if dim < result.inherited_personality.len() {
                            result.inherited_personality[dim] += m.magnitude;
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    use crate::evolution::skill_tree::SkillTree;

    fn make_crossover_engine() -> CrossoverEngine {
        let mutation_engine = MutationEngine::new(0.05, SkillTree::default());
        CrossoverEngine::new(CrossoverConfig::default(), mutation_engine)
    }

    fn seeded_rng() -> StdRng {
        StdRng::seed_from_u64(42)
    }

    // ── Skill crossover tests ────────────────────────────────────────────

    #[test]
    fn crossover_skills_average_strategy() {
        let engine = make_crossover_engine();
        let mut a = HashMap::new();
        a.insert("coding".to_string(), (4, 100.0));
        a.insert("trading".to_string(), (2, 50.0));

        let mut b = HashMap::new();
        b.insert("coding".to_string(), (6, 200.0));
        b.insert("crafting".to_string(), (3, 75.0));

        let result = engine.crossover_skills(&a, &b, &CrossoverStrategy::Average);

        // (4+6)/2 = 5
        assert_eq!(result["coding"].0, 5);
        // (100+200)/2 = 150
        assert!((result["coding"].1 - 150.0).abs() < f64::EPSILON);
        // Only in parent A
        assert_eq!(result["trading"].0, 2);
        // Only in parent B
        assert_eq!(result["crafting"].0, 3);
    }

    #[test]
    fn crossover_skills_max_strategy() {
        let engine = make_crossover_engine();
        let mut a = HashMap::new();
        a.insert("coding".to_string(), (4, 100.0));

        let mut b = HashMap::new();
        b.insert("coding".to_string(), (6, 200.0));

        let result = engine.crossover_skills(&a, &b, &CrossoverStrategy::Max);

        assert_eq!(result["coding"].0, 6);
        assert!((result["coding"].1 - 200.0).abs() < f64::EPSILON);
    }

    #[test]
    fn crossover_skills_random_strategy() {
        let engine = make_crossover_engine();
        let mut rng = seeded_rng();
        let mut a = HashMap::new();
        a.insert("coding".to_string(), (4, 100.0));

        let mut b = HashMap::new();
        b.insert("coding".to_string(), (6, 200.0));

        let result = engine.crossover_skills_random(&mut rng, &a, &b);

        // Should be either parent's value
        assert!(
            result["coding"].0 == 4 || result["coding"].0 == 6,
            "Random crossover should pick one parent's level"
        );
    }

    #[test]
    fn crossover_skills_empty_parents() {
        let engine = make_crossover_engine();
        let a = HashMap::new();
        let b = HashMap::new();

        let result = engine.crossover_skills(&a, &b, &CrossoverStrategy::Average);
        assert!(result.is_empty());
    }

    #[test]
    fn crossover_skills_one_empty_parent() {
        let engine = make_crossover_engine();
        let mut a = HashMap::new();
        a.insert("coding".to_string(), (3, 50.0));
        let b = HashMap::new();

        let result = engine.crossover_skills(&a, &b, &CrossoverStrategy::Average);
        assert_eq!(result["coding"].0, 3);
        assert!((result["coding"].1 - 50.0).abs() < f64::EPSILON);
    }

    // ── Personality crossover tests ──────────────────────────────────────

    #[test]
    fn crossover_personality_equal_blend() {
        let engine = make_crossover_engine();
        let a = vec![0.8, 0.2, 0.5];
        let b = vec![0.4, 0.6, 0.5];

        let result = engine.crossover_personality(&a, &b, 0.5);

        assert!((result[0] - 0.6).abs() < f64::EPSILON);
        assert!((result[1] - 0.4).abs() < f64::EPSILON);
        assert!((result[2] - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn crossover_personality_all_parent_a() {
        let engine = make_crossover_engine();
        let a = vec![0.9, 0.1];
        let b = vec![0.1, 0.9];

        let result = engine.crossover_personality(&a, &b, 1.0);

        assert!((result[0] - 0.9).abs() < f64::EPSILON);
        assert!((result[1] - 0.1).abs() < f64::EPSILON);
    }

    #[test]
    fn crossover_personality_different_lengths() {
        let engine = make_crossover_engine();
        let a = vec![0.5];
        let b = vec![0.3, 0.7, 0.9];

        let result = engine.crossover_personality(&a, &b, 0.5);

        assert_eq!(result.len(), 3);
        assert!((result[0] - 0.4).abs() < f64::EPSILON);
        assert!((result[1] - 0.35).abs() < f64::EPSILON);
        assert!((result[2] - 0.45).abs() < f64::EPSILON);
    }

    #[test]
    fn crossover_personality_empty() {
        let engine = make_crossover_engine();
        let a: Vec<f64> = vec![];
        let b: Vec<f64> = vec![];

        let result = engine.crossover_personality(&a, &b, 0.5);
        assert!(result.is_empty());
    }

    // ── Full crossover result tests ──────────────────────────────────────

    #[test]
    fn full_crossover_with_mutations() {
        let mutation_engine = MutationEngine::new(0.05, SkillTree::default());
        let engine = CrossoverEngine::new(
            CrossoverConfig {
                apply_mutations: true,
                mutation_config: OffspringMutationConfig {
                    base_offspring_mutation_rate: 1.0,
                    ..Default::default()
                },
                ..Default::default()
            },
            mutation_engine,
        );

        let mut rng = seeded_rng();
        let mut a_skills = HashMap::new();
        a_skills.insert("coding".to_string(), (5, 200.0));

        let mut b_skills = HashMap::new();
        b_skills.insert("coding".to_string(), (3, 100.0));
        b_skills.insert("trading".to_string(), (2, 50.0));

        let a_personality = vec![0.8, 0.2, 0.5, 0.3, 0.7];
        let b_personality = vec![0.4, 0.6, 0.5, 0.7, 0.3];

        let env = EnvironmentPressure {
            token_ratio: 1.0,
            population_pressure: 0.5,
        };

        let result = engine.crossover(
            &mut rng,
            &a_skills, &b_skills,
            &a_personality, &b_personality,
            &[], &[],
            &env,
            "offspring-1", "parent-a", "parent-b",
        );

        assert_eq!(result.offspring_id, "offspring-1");
        assert_eq!(result.parent_a_id, "parent-a");
        assert_eq!(result.parent_b_id, "parent-b");
        assert!(result.mutation_result.is_some());
        // Average strategy: coding = (5+3)/2 = 4
        assert_eq!(result.inherited_skills["coding"].0, 4);
    }

    // ── Mutation effect application tests ────────────────────────────────

    #[test]
    fn apply_mutation_effects_skill_jump() {
        let engine = make_crossover_engine();
        let mut result = CrossoverResult {
            offspring_id: "o1".to_string(),
            parent_a_id: "a".to_string(),
            parent_b_id: "b".to_string(),
            inherited_skills: HashMap::new(),
            inherited_personality: vec![0.5, 0.5],
            mutation_result: Some(OffspringMutationResult {
                offspring_id: "o1".to_string(),
                parent_a_id: "a".to_string(),
                parent_b_id: "b".to_string(),
                mutations: vec![OffspringMutation {
                    mutation_type: OffspringMutationType::SkillLevelJump,
                    skill_name: Some("coding".to_string()),
                    personality_dimension: None,
                    magnitude: 2.0,
                    description: "jump".to_string(),
                }],
                effective_mutation_rate: 1.0,
            }),
        };
        result.inherited_skills.insert("coding".to_string(), (3, 100.0));

        engine.apply_mutation_effects(&mut result, 10);

        assert_eq!(result.inherited_skills["coding"].0, 5);
    }

    #[test]
    fn apply_mutation_effects_skill_drop_min_level_one() {
        let engine = make_crossover_engine();
        let mut result = CrossoverResult {
            offspring_id: "o1".to_string(),
            parent_a_id: "a".to_string(),
            parent_b_id: "b".to_string(),
            inherited_skills: HashMap::new(),
            inherited_personality: vec![0.5],
            mutation_result: Some(OffspringMutationResult {
                offspring_id: "o1".to_string(),
                parent_a_id: "a".to_string(),
                parent_b_id: "b".to_string(),
                mutations: vec![OffspringMutation {
                    mutation_type: OffspringMutationType::SkillLevelDrop,
                    skill_name: Some("coding".to_string()),
                    personality_dimension: None,
                    magnitude: -5.0,
                    description: "drop".to_string(),
                }],
                effective_mutation_rate: 1.0,
            }),
        };
        result.inherited_skills.insert("coding".to_string(), (2, 100.0));

        engine.apply_mutation_effects(&mut result, 10);

        // Should clamp to 1, not go to 0
        assert_eq!(result.inherited_skills["coding"].0, 1);
    }

    #[test]
    fn apply_mutation_effects_novel_skill() {
        let engine = make_crossover_engine();
        let mut result = CrossoverResult {
            offspring_id: "o1".to_string(),
            parent_a_id: "a".to_string(),
            parent_b_id: "b".to_string(),
            inherited_skills: HashMap::new(),
            inherited_personality: vec![],
            mutation_result: Some(OffspringMutationResult {
                offspring_id: "o1".to_string(),
                parent_a_id: "a".to_string(),
                parent_b_id: "b".to_string(),
                mutations: vec![OffspringMutation {
                    mutation_type: OffspringMutationType::NovelSkill,
                    skill_name: Some("exploration".to_string()),
                    personality_dimension: None,
                    magnitude: 1.0,
                    description: "novel".to_string(),
                }],
                effective_mutation_rate: 1.0,
            }),
        };

        engine.apply_mutation_effects(&mut result, 10);

        assert_eq!(result.inherited_skills["exploration"].0, 1);
        assert!((result.inherited_skills["exploration"].1).abs() < f64::EPSILON);
    }

    #[test]
    fn apply_mutation_effects_personality_shift() {
        let engine = make_crossover_engine();
        let mut result = CrossoverResult {
            offspring_id: "o1".to_string(),
            parent_a_id: "a".to_string(),
            parent_b_id: "b".to_string(),
            inherited_skills: HashMap::new(),
            inherited_personality: vec![0.5, 0.3],
            mutation_result: Some(OffspringMutationResult {
                offspring_id: "o1".to_string(),
                parent_a_id: "a".to_string(),
                parent_b_id: "b".to_string(),
                mutations: vec![OffspringMutation {
                    mutation_type: OffspringMutationType::PersonalityShift,
                    skill_name: None,
                    personality_dimension: Some(1),
                    magnitude: 0.2,
                    description: "shift".to_string(),
                }],
                effective_mutation_rate: 1.0,
            }),
        };

        engine.apply_mutation_effects(&mut result, 10);

        assert!((result.inherited_personality[1] - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn apply_mutation_effects_skill_jump_clamped_to_max() {
        let engine = make_crossover_engine();
        let mut result = CrossoverResult {
            offspring_id: "o1".to_string(),
            parent_a_id: "a".to_string(),
            parent_b_id: "b".to_string(),
            inherited_skills: HashMap::new(),
            inherited_personality: vec![],
            mutation_result: Some(OffspringMutationResult {
                offspring_id: "o1".to_string(),
                parent_a_id: "a".to_string(),
                parent_b_id: "b".to_string(),
                mutations: vec![OffspringMutation {
                    mutation_type: OffspringMutationType::SkillLevelJump,
                    skill_name: Some("coding".to_string()),
                    personality_dimension: None,
                    magnitude: 5.0,
                    description: "jump".to_string(),
                }],
                effective_mutation_rate: 1.0,
            }),
        };
        result.inherited_skills.insert("coding".to_string(), (8, 100.0));

        engine.apply_mutation_effects(&mut result, 10);

        // 8 + 5 = 13, clamped to max_level 10
        assert_eq!(result.inherited_skills["coding"].0, 10);
    }
}
