//! Terrain types, tile model, and resource nodes for the hex world map.
//!
//! Extends the basic 4-type [`Terrain`](super::super::seeder::Terrain) from the seeder
//! with 6 terrain types and per-tile state (resources, buildings, agents).

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use super::hex::HexPos;

/// Terrain types for the hex world map.
///
/// Six terrain types covering major biomes. Each has different traversal
/// costs and resource generation characteristics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerrainType {
    /// Flat grassland — easy traversal, supports agriculture.
    Plains,
    /// Dense woodland — moderate traversal, rich in wood and herbs.
    Forest,
    /// Rugged highlands — slow traversal, minerals and stone.
    Mountain,
    /// Lakes, rivers, seas — impassable for most, fish/transport.
    Water,
    /// Arid sandy regions — moderate traversal, scarce resources.
    Desert,
    /// Frozen northern/southern lands — slow traversal, ice/fur.
    Tundra,
}

impl TerrainType {
    /// All terrain variants.
    pub fn all() -> &'static [TerrainType] {
        &[
            TerrainType::Plains,
            TerrainType::Forest,
            TerrainType::Mountain,
            TerrainType::Water,
            TerrainType::Desert,
            TerrainType::Tundra,
        ]
    }

    /// Human-readable name.
    pub fn name(&self) -> &'static str {
        match self {
            TerrainType::Plains => "Plains",
            TerrainType::Forest => "Forest",
            TerrainType::Mountain => "Mountain",
            TerrainType::Water => "Water",
            TerrainType::Desert => "Desert",
            TerrainType::Tundra => "Tundra",
        }
    }

    /// Movement cost to enter this terrain (1 = normal, higher = slower).
    ///
    /// Returns `None` if the terrain is impassable.
    pub fn movement_cost(&self) -> Option<u32> {
        match self {
            TerrainType::Plains => Some(1),
            TerrainType::Forest => Some(2),
            TerrainType::Mountain => Some(3),
            TerrainType::Water => None,
            TerrainType::Desert => Some(2),
            TerrainType::Tundra => Some(3),
        }
    }

    /// Whether agents can walk on this terrain.
    pub fn is_walkable(&self) -> bool {
        self.movement_cost().is_some()
    }

    /// Convert from the seeder's [`Terrain`](super::super::seeder::Terrain) enum.
    ///
    /// Maps the 4 seeder terrains to the 6-map terrain types. Plains, Forest,
    /// Water, and Mountain have direct mappings.
    pub fn from_seeder_terrain(terrain: &super::super::seeder::Terrain) -> Self {
        match terrain {
            super::super::seeder::Terrain::Plains => TerrainType::Plains,
            super::super::seeder::Terrain::Forest => TerrainType::Forest,
            super::super::seeder::Terrain::Water => TerrainType::Water,
            super::super::seeder::Terrain::Mountain => TerrainType::Mountain,
        }
    }
}

/// A resource node on a tile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceNode {
    /// Unique resource node identifier.
    pub id: String,
    /// Resource type (e.g., "food", "wood", "stone", "mineral", "fish", "fur").
    pub kind: String,
    /// Current remaining amount.
    pub amount: u64,
    /// Maximum amount (for regeneration tracking).
    pub max_amount: u64,
    /// Whether this resource regenerates over ticks.
    pub renewable: bool,
    /// Regeneration rate per tick (if renewable).
    pub regen_rate: u64,
}

impl ResourceNode {
    /// Create a new non-renewable resource node.
    pub fn new(id: impl Into<String>, kind: impl Into<String>, amount: u64) -> Self {
        let id = id.into();
        let kind = kind.into();
        Self {
            id,
            kind,
            amount,
            max_amount: amount,
            renewable: false,
            regen_rate: 0,
        }
    }

    /// Create a renewable resource node.
    pub fn renewable(
        id: impl Into<String>,
        kind: impl Into<String>,
        amount: u64,
        regen_rate: u64,
    ) -> Self {
        let id = id.into();
        let kind = kind.into();
        Self {
            id,
            kind,
            amount,
            max_amount: amount,
            renewable: true,
            regen_rate,
        }
    }

    /// Harvest `n` units from this resource. Returns the actual amount harvested
    /// (may be less if the resource is depleted).
    pub fn harvest(&mut self, n: u64) -> u64 {
        let taken = n.min(self.amount);
        self.amount -= taken;
        taken
    }

    /// Tick-based regeneration for renewable resources.
    pub fn tick_regen(&mut self) {
        if self.renewable && self.amount < self.max_amount {
            self.amount = (self.amount + self.regen_rate).min(self.max_amount);
        }
    }

    /// Whether this resource is fully depleted.
    pub fn is_depleted(&self) -> bool {
        self.amount == 0
    }
}

/// A single hex tile on the world map.
///
/// Each tile has a terrain type, optional resources, optional building reference,
/// and the set of agents currently occupying it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tile {
    /// Position of this tile on the hex grid.
    pub pos: HexPos,
    /// Terrain type.
    pub terrain: TerrainType,
    /// Resource nodes on this tile.
    pub resources: Vec<ResourceNode>,
    /// ID of the building on this tile (if any).
    pub building_id: Option<String>,
    /// IDs of agents currently occupying this tile.
    pub agent_ids: HashSet<String>,
}

impl Tile {
    /// Create a new tile with the given terrain at the given position.
    pub fn new(pos: HexPos, terrain: TerrainType) -> Self {
        Self {
            pos,
            terrain,
            resources: Vec::new(),
            building_id: None,
            agent_ids: HashSet::new(),
        }
    }

    /// Whether agents can enter this tile.
    pub fn is_walkable(&self) -> bool {
        self.terrain.is_walkable()
    }

    /// Add a resource node to this tile.
    pub fn add_resource(&mut self, resource: ResourceNode) {
        self.resources.push(resource);
    }

    /// Remove fully depleted resources from this tile.
    /// Returns the number of removed resources.
    pub fn cleanup_resources(&mut self) -> usize {
        let before = self.resources.len();
        self.resources.retain(|r| !r.is_depleted());
        before - self.resources.len()
    }

    /// Tick regeneration on all renewable resources.
    pub fn tick_resource_regen(&mut self) {
        for resource in &mut self.resources {
            resource.tick_regen();
        }
    }

    /// Place an agent on this tile.
    pub fn add_agent(&mut self, agent_id: impl Into<String>) {
        self.agent_ids.insert(agent_id.into());
    }

    /// Remove an agent from this tile.
    pub fn remove_agent(&mut self, agent_id: &str) -> bool {
        self.agent_ids.remove(agent_id)
    }

    /// Number of agents on this tile.
    pub fn agent_count(&self) -> usize {
        self.agent_ids.len()
    }

    /// Place a building on this tile.
    pub fn set_building(&mut self, building_id: impl Into<String>) {
        self.building_id = Some(building_id.into());
    }

    /// Remove the building from this tile.
    pub fn clear_building(&mut self) {
        self.building_id = None;
    }

    /// Total resource amount across all nodes of the given kind.
    pub fn total_resource_of_kind(&self, kind: &str) -> u64 {
        self.resources
            .iter()
            .filter(|r| r.kind == kind)
            .map(|r| r.amount)
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terrain_type_walkability() {
        assert!(TerrainType::Plains.is_walkable());
        assert!(TerrainType::Forest.is_walkable());
        assert!(TerrainType::Mountain.is_walkable());
        assert!(!TerrainType::Water.is_walkable());
        assert!(TerrainType::Desert.is_walkable());
        assert!(TerrainType::Tundra.is_walkable());
    }

    #[test]
    fn terrain_movement_costs() {
        assert_eq!(TerrainType::Plains.movement_cost(), Some(1));
        assert_eq!(TerrainType::Forest.movement_cost(), Some(2));
        assert_eq!(TerrainType::Mountain.movement_cost(), Some(3));
        assert_eq!(TerrainType::Water.movement_cost(), None);
        assert_eq!(TerrainType::Desert.movement_cost(), Some(2));
        assert_eq!(TerrainType::Tundra.movement_cost(), Some(3));
    }

    #[test]
    fn terrain_names() {
        assert_eq!(TerrainType::Plains.name(), "Plains");
        assert_eq!(TerrainType::Desert.name(), "Desert");
        assert_eq!(TerrainType::Tundra.name(), "Tundra");
    }

    #[test]
    fn terrain_all_count() {
        assert_eq!(TerrainType::all().len(), 6);
    }

    #[test]
    fn from_seeder_terrain() {
        assert_eq!(
            TerrainType::from_seeder_terrain(&super::super::super::seeder::Terrain::Plains),
            TerrainType::Plains
        );
        assert_eq!(
            TerrainType::from_seeder_terrain(&super::super::super::seeder::Terrain::Forest),
            TerrainType::Forest
        );
    }

    #[test]
    fn resource_node_harvest() {
        let mut res = ResourceNode::new("res-1", "wood", 100);
        assert_eq!(res.harvest(30), 30);
        assert_eq!(res.amount, 70);
        // Over-harvest capped
        let mut res2 = ResourceNode::new("res-2", "food", 10);
        assert_eq!(res2.harvest(20), 10);
        assert_eq!(res2.amount, 0);
        assert!(res2.is_depleted());
    }

    #[test]
    fn resource_node_regen() {
        let mut res = ResourceNode::renewable("res-1", "fish", 100, 10);
        res.amount = 70;
        res.tick_regen();
        assert_eq!(res.amount, 80);
        // Regen capped at max
        res.tick_regen();
        res.tick_regen();
        res.tick_regen();
        assert_eq!(res.amount, 100);
    }

    #[test]
    fn tile_agents() {
        let mut tile = Tile::new(HexPos::new(0, 0), TerrainType::Plains);
        tile.add_agent("agent-1");
        tile.add_agent("agent-2");
        assert_eq!(tile.agent_count(), 2);
        // Duplicate ignored
        tile.add_agent("agent-1");
        assert_eq!(tile.agent_count(), 2);
        // Remove
        assert!(tile.remove_agent("agent-1"));
        assert_eq!(tile.agent_count(), 1);
        assert!(!tile.remove_agent("nonexistent"));
    }

    #[test]
    fn tile_building() {
        let mut tile = Tile::new(HexPos::new(1, 0), TerrainType::Forest);
        assert!(tile.building_id.is_none());
        tile.set_building("bld-1");
        assert_eq!(tile.building_id.as_deref(), Some("bld-1"));
        tile.clear_building();
        assert!(tile.building_id.is_none());
    }

    #[test]
    fn tile_resources() {
        let mut tile = Tile::new(HexPos::new(2, 0), TerrainType::Forest);
        tile.add_resource(ResourceNode::new("r1", "wood", 50));
        tile.add_resource(ResourceNode::new("r2", "food", 30));
        assert_eq!(tile.total_resource_of_kind("wood"), 50);
        assert_eq!(tile.total_resource_of_kind("food"), 30);
        assert_eq!(tile.total_resource_of_kind("stone"), 0);
    }

    #[test]
    fn tile_cleanup_depleted() {
        let mut tile = Tile::new(HexPos::new(3, 0), TerrainType::Plains);
        tile.add_resource(ResourceNode::new("r1", "wood", 0)); // depleted
        tile.add_resource(ResourceNode::new("r2", "food", 30));
        assert_eq!(tile.cleanup_resources(), 1);
        assert_eq!(tile.resources.len(), 1);
    }

    #[test]
    fn tile_walkability() {
        let water = Tile::new(HexPos::new(0, 0), TerrainType::Water);
        assert!(!water.is_walkable());
        let plains = Tile::new(HexPos::new(1, 0), TerrainType::Plains);
        assert!(plains.is_walkable());
    }

    #[test]
    fn serialization_round_trip() {
        let mut tile = Tile::new(HexPos::with_layer(3, -2, 1), TerrainType::Desert);
        tile.add_agent("agent-1");
        tile.set_building("bld-42");
        tile.add_resource(ResourceNode::renewable("r1", "mineral", 200, 5));

        let json = serde_json::to_string(&tile).unwrap();
        let recovered: Tile = serde_json::from_str(&json).unwrap();
        assert_eq!(recovered.pos, tile.pos);
        assert_eq!(recovered.terrain, tile.terrain);
        assert_eq!(recovered.building_id, tile.building_id);
        assert_eq!(recovered.agent_ids, tile.agent_ids);
    }
}
