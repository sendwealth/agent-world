//! World map module — hex terrain, buildings, and spatial structures.
//!
//! This module implements a hexagonal grid world map with:
//! - **HexPos**: Axial coordinate system (q, r, layer) for pointy-top hex grids
//! - **TerrainType**: Six biome types (Plains, Forest, Mountain, Water, Desert, Tundra)
//! - **Tile**: Per-hex state (terrain, resources, buildings, agents)
//! - **ResourceNode**: Harvestable and renewable resources
//! - **WorldMap**: Top-level spatial structure with versioned snapshots
//! - **Building**: Construction and lifecycle management
//!
//! # Example
//!
//! ```
//! use agent_world_engine::world::map::{WorldMap, HexPos, TerrainType, Tile};
//!
//! let mut map = WorldMap::with_radius(10);
//! map.insert(Tile::new(HexPos::new(0, 0), TerrainType::Plains));
//! map.insert(Tile::new(HexPos::new(1, 0), TerrainType::Forest));
//!
//! let neighbors = map.walkable_neighbors(&HexPos::new(0, 0));
//! assert_eq!(neighbors.len(), 1);
//! ```

pub mod building;
pub mod hex;
pub mod terrain;
pub mod world_map;

pub use building::{
    Building, BuildingCost, BuildingId, BuildingManager, BuildingStatus, BuildingType, OwnerType,
};
pub use hex::HexPos;
pub use terrain::{ResourceNode, TerrainType, Tile};
pub use world_map::{MapSnapshot, WorldMap};
