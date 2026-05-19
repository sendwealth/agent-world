//! Skill mutation engine.
//!
//! Every evaluation cycle (default: every 1000 ticks), each agent has a
//! configurable probability (default: 5%) of a mutation. Mutations can
//! generate a new random skill or modify an existing one.

use std::collections::HashMap;

use rand::Rng;
use serde::{Deserialize, Serialize};

use super::skill_tree::SkillTree;

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

/// The mutation engine, responsible for generating random skill mutations.
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
            return None;
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
        // With seed 42 and rate 1.0, should get a mutation
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
        // Give agent all skills so new-skill path falls back to boost
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
}
