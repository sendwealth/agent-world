//! Seeded world generation for reproducible experiments.
//!
//! [`WorldSeeder`] uses a deterministic PRNG (ChaCha8 via `rand` crate)
//! to generate terrain, resources, and agents from a given seed. This
//! ensures experiments are fully reproducible: same seed + config = same world.
//!
//! # Usage
//!
//! ```rust,ignore
//! use agent_world_engine::world::seeder::WorldSeeder;
//!
//! let mut seeder = WorldSeeder::new(42);
//! let terrain = seeder.generate_terrain(100, 100);
//! let resources = seeder.generate_resources(&terrain, 0.3);
//! let agents = seeder.generate_agents(50, 100);
//! ```

use std::collections::HashMap;

use rand::SeedableRng;
use rand::rngs::StdRng;
use rand::Rng;
use serde::{Deserialize, Serialize};

use super::agent::Agent;
use super::enums::AgentPhase;

/// Terrain types in the world grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Terrain {
    /// Flat land — easy to traverse, supports resources.
    Plains,
    /// Forest — moderate traversal, rich in resources.
    Forest,
    /// Water — impassable for most agents.
    Water,
    /// Mountain — slow traversal, limited resources.
    Mountain,
}

impl Terrain {
    /// All terrain variants.
    pub fn all() -> &'static [Terrain] {
        &[Terrain::Plains, Terrain::Forest, Terrain::Water, Terrain::Mountain]
    }
}

/// A resource on the world map.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resource {
    pub id: String,
    pub x: usize,
    pub y: usize,
    pub kind: String,
    pub amount: u64,
}

/// Seeded world generator for deterministic experiment setup.
pub struct WorldSeeder {
    seed: u64,
    rng: StdRng,
}

impl WorldSeeder {
    /// Create a new seeder with the given seed.
    ///
    /// The seed determines all subsequent generated content.
    /// Same seed always produces the same world.
    pub fn new(seed: u64) -> Self {
        Self {
            seed,
            rng: StdRng::seed_from_u64(seed),
        }
    }

    /// Return the seed used by this seeder.
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// Generate a terrain grid of the given dimensions.
    ///
    /// The distribution is roughly:
    /// - 50% Plains
    /// - 25% Forest
    /// - 15% Water
    /// - 10% Mountain
    pub fn generate_terrain(&mut self, width: usize, height: usize) -> Vec<Vec<Terrain>> {
        let mut grid = Vec::with_capacity(height);
        for _ in 0..height {
            let mut row = Vec::with_capacity(width);
            for _ in 0..width {
                let roll: f64 = self.rng.gen();
                let terrain = if roll < 0.50 {
                    Terrain::Plains
                } else if roll < 0.75 {
                    Terrain::Forest
                } else if roll < 0.90 {
                    Terrain::Water
                } else {
                    Terrain::Mountain
                };
                row.push(terrain);
            }
            grid.push(row);
        }
        grid
    }

    /// Generate resources on walkable terrain (Plains, Forest).
    ///
    /// Args:
    /// - `terrain`: The terrain grid (from `generate_terrain`).
    /// - `density`: Resource density 0.0–1.0 (probability per walkable cell).
    ///
    /// Returns a list of resources with random types and amounts.
    pub fn generate_resources(
        &mut self,
        terrain: &[Vec<Terrain>],
        density: f64,
    ) -> Vec<Resource> {
        let resource_kinds = ["food", "wood", "stone", "mineral"];
        let mut resources = Vec::new();
        let mut counter = 0u64;

        for (y, row) in terrain.iter().enumerate() {
            for (x, cell) in row.iter().enumerate() {
                // Only place resources on walkable terrain
                if !matches!(cell, Terrain::Plains | Terrain::Forest) {
                    continue;
                }
                if self.rng.gen::<f64>() < density {
                    let kind_idx = self.rng.gen_range(0..resource_kinds.len());
                    let amount = self.rng.gen_range(10..100);
                    counter += 1;
                    resources.push(Resource {
                        id: format!("res-{}", counter),
                        x,
                        y,
                        kind: resource_kinds[kind_idx].to_string(),
                        amount,
                    });
                }
            }
        }
        resources
    }

    /// Generate agents with deterministic properties.
    ///
    /// Each agent gets a unique ID, a random name (from a fixed pool),
    /// initial tokens, and starts in the Adult phase.
    ///
    /// Args:
    /// - `count`: Number of agents to generate.
    /// - `initial_tokens`: Starting token balance for each agent.
    pub fn generate_agents(&mut self, count: usize, initial_tokens: u64) -> Vec<Agent> {
        let names = [
            "Alice", "Bob", "Carol", "Dave", "Eve", "Frank", "Grace", "Henry",
            "Iris", "Jack", "Kate", "Leo", "Mia", "Noah", "Olga", "Paul",
            "Quinn", "Rosa", "Sven", "Tara", "Uma", "Victor", "Wendy", "Xavier",
            "Yuki", "Zara", "Amir", "Bela", "Chen", "Dara", "Elena", "Felix",
        ];

        let mut agents = Vec::with_capacity(count);
        for i in 0..count {
            let name_idx = self.rng.gen_range(0..names.len());
            let reputation = self.rng.gen_range(0.0..1.0);
            let mut skills = HashMap::new();
            // Give each agent 1-3 random skills
            let skill_count = self.rng.gen_range(1..=3);
            let all_skills = ["trading", "farming", "building", "research", "teaching"];
            for _ in 0..skill_count {
                let sk = self.rng.gen_range(0..all_skills.len());
                let level = self.rng.gen_range(1..=5);
                skills.insert(all_skills[sk].to_string(), level);
            }

            agents.push(Agent {
                id: format!("agent-{:03}", i + 1),
                name: names[name_idx].to_string(),
                phase: AgentPhase::Adult,
                money: 0,
                tokens: initial_tokens,
                reputation,
                skills,
                alive: true,
                age: 0,
                created_at: String::new(),
            });
        }
        agents
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_terrain() {
        let mut a = WorldSeeder::new(42);
        let mut b = WorldSeeder::new(42);
        let ta = a.generate_terrain(10, 10);
        let tb = b.generate_terrain(10, 10);
        assert_eq!(ta, tb);
    }

    #[test]
    fn different_seed_different_terrain() {
        let mut a = WorldSeeder::new(42);
        let mut b = WorldSeeder::new(99);
        let ta = a.generate_terrain(10, 10);
        let tb = b.generate_terrain(10, 10);
        assert_ne!(ta, tb);
    }

    #[test]
    fn terrain_dimensions() {
        let mut s = WorldSeeder::new(42);
        let t = s.generate_terrain(50, 30);
        assert_eq!(t.len(), 30);
        assert_eq!(t[0].len(), 50);
    }

    #[test]
    fn resource_density_zero() {
        let mut s = WorldSeeder::new(42);
        let t = s.generate_terrain(10, 10);
        let r = s.generate_resources(&t, 0.0);
        assert!(r.is_empty());
    }

    #[test]
    fn resource_density_one() {
        let mut s = WorldSeeder::new(42);
        let t = s.generate_terrain(10, 10);
        let r = s.generate_resources(&t, 1.0);
        // All walkable cells should have resources
        let walkable = t.iter().flat_map(|row| row.iter())
            .filter(|c| matches!(c, Terrain::Plains | Terrain::Forest))
            .count();
        assert_eq!(r.len(), walkable);
    }

    #[test]
    fn agent_count_and_initial_tokens() {
        let mut s = WorldSeeder::new(42);
        let agents = s.generate_agents(20, 500);
        assert_eq!(agents.len(), 20);
        for a in &agents {
            assert_eq!(a.tokens, 500);
            assert!(a.alive);
            assert_eq!(a.phase, AgentPhase::Adult);
            assert!(!a.skills.is_empty());
        }
    }

    #[test]
    fn same_seed_same_agents() {
        let mut a = WorldSeeder::new(42);
        let mut b = WorldSeeder::new(42);
        let aa = a.generate_agents(10, 100);
        let ba = b.generate_agents(10, 100);
        assert_eq!(aa.len(), ba.len());
        for (a, b) in aa.iter().zip(ba.iter()) {
            assert_eq!(a.id, b.id);
            assert_eq!(a.name, b.name);
            assert_eq!(a.tokens, b.tokens);
        }
    }

    #[test]
    fn seeder_returns_seed() {
        let s = WorldSeeder::new(12345);
        assert_eq!(s.seed(), 12345);
    }
}
