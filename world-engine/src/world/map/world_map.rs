//! World map — the top-level spatial structure for the hex world.
//!
//! [`WorldMap`] stores tiles in a `HashMap<HexPos, Tile>` and supports
//! versioned snapshots for undo/time-travel, spatial queries, and
//! integration with the World Engine's tick-based simulation.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::hex::HexPos;
use super::terrain::{TerrainType, Tile};

/// A versioned snapshot of the entire world map.
///
/// Snapshots capture the complete tile state at a given tick, enabling
/// undo/redo and time-travel debugging of the world simulation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapSnapshot {
    /// The tick at which this snapshot was taken.
    pub tick: u64,
    /// Complete tile state at this tick.
    pub tiles: HashMap<HexPos, Tile>,
    /// Optional label for the snapshot (e.g., "before_battle").
    pub label: Option<String>,
}

/// The world map — a hex grid backed by a hash map for O(1) tile lookup.
///
/// Supports incremental versioned snapshots, spatial queries (neighbors,
/// radius, line-of-sight), and integrates with the WorldState tick loop.
///
/// # Thread Safety
///
/// `WorldMap` is not `Sync` — it should be owned by the single `WorldState`
/// and accessed through the main tick loop.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldMap {
    /// All tiles indexed by their hex position.
    tiles: HashMap<HexPos, Tile>,
    /// Map version / tick counter for change tracking.
    version: u64,
    /// Versioned snapshots (kept in order, newest last).
    snapshots: Vec<MapSnapshot>,
    /// Maximum number of snapshots to retain (prevents unbounded memory).
    max_snapshots: usize,
    /// Radius of the map (0 = infinite / no boundary).
    radius: i32,
}

impl WorldMap {
    /// Create a new empty world map with no boundary.
    pub fn new() -> Self {
        Self {
            tiles: HashMap::new(),
            version: 0,
            snapshots: Vec::new(),
            max_snapshots: 100,
            radius: 0,
        }
    }

    /// Create a map bounded to a hex radius from the origin.
    ///
    /// Only positions within the given radius (inclusive) are valid.
    pub fn with_radius(radius: i32) -> Self {
        Self {
            radius,
            ..Self::new()
        }
    }

    /// Create a map with a custom snapshot limit.
    pub fn with_snapshot_limit(max_snapshots: usize) -> Self {
        Self {
            max_snapshots,
            ..Self::new()
        }
    }

    // ── Tile access ───────────────────────────────────────────────────

    /// Get a tile by position.
    pub fn get(&self, pos: &HexPos) -> Option<&Tile> {
        self.tiles.get(pos)
    }

    /// Get a mutable tile by position.
    pub fn get_mut(&mut self, pos: &HexPos) -> Option<&mut Tile> {
        self.tiles.get_mut(pos)
    }

    /// Insert or replace a tile at the given position.
    pub fn insert(&mut self, tile: Tile) {
        self.version += 1;
        self.tiles.insert(tile.pos, tile);
    }

    /// Remove a tile at the given position.
    pub fn remove(&mut self, pos: &HexPos) -> Option<Tile> {
        if self.tiles.remove(pos).is_some() {
            self.version += 1;
        }
        self.tiles.get(pos).cloned()
    }

    /// Total number of tiles in the map.
    pub fn tile_count(&self) -> usize {
        self.tiles.len()
    }

    /// Current version (incremented on every mutation).
    pub fn version(&self) -> u64 {
        self.version
    }

    /// Whether the map has a boundary radius.
    pub fn is_bounded(&self) -> bool {
        self.radius > 0
    }

    /// The boundary radius (0 = unbounded).
    pub fn radius(&self) -> i32 {
        self.radius
    }

    /// Check whether a position is within the map boundary.
    pub fn is_in_bounds(&self, pos: &HexPos) -> bool {
        if self.radius == 0 {
            true
        } else {
            HexPos::ORIGIN.distance_to(pos) <= self.radius
        }
    }

    /// Iterate over all tiles.
    pub fn iter(&self) -> impl Iterator<Item = (&HexPos, &Tile)> {
        self.tiles.iter()
    }

    /// Iterate over all tiles mutably.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&HexPos, &mut Tile)> {
        self.tiles.iter_mut()
    }

    // ── Spatial queries ───────────────────────────────────────────────

    /// Get all walkable neighbors of a position.
    ///
    /// Returns tiles that exist in the map, are walkable, and are within bounds.
    pub fn walkable_neighbors(&self, pos: &HexPos) -> Vec<&Tile> {
        pos.neighbors()
            .iter()
            .filter(|p| self.is_in_bounds(p))
            .filter_map(|p| self.tiles.get(p))
            .filter(|t| t.is_walkable())
            .collect()
    }

    /// Get all tiles within `radius` of the given position.
    pub fn tiles_in_radius(&self, center: &HexPos, radius: i32) -> Vec<&Tile> {
        center
            .ring(radius)
            .iter()
            .filter_map(|p| self.tiles.get(p))
            .collect()
    }

    /// Get all walkable tiles within `radius` of the given position.
    pub fn walkable_in_radius(&self, center: &HexPos, radius: i32) -> Vec<&Tile> {
        self.tiles_in_radius(center, radius)
            .into_iter()
            .filter(|t| t.is_walkable())
            .collect()
    }

    /// Find tiles containing a specific terrain type.
    pub fn tiles_by_terrain(&self, terrain: TerrainType) -> Vec<&Tile> {
        self.tiles
            .values()
            .filter(|t| t.terrain == terrain)
            .collect()
    }

    /// Find tiles occupied by a specific agent.
    pub fn tiles_with_agent(&self, agent_id: &str) -> Vec<&Tile> {
        self.tiles
            .values()
            .filter(|t| t.agent_ids.contains(agent_id))
            .collect()
    }

    /// Find tiles with buildings.
    pub fn tiles_with_buildings(&self) -> Vec<&Tile> {
        self.tiles
            .values()
            .filter(|t| t.building_id.is_some())
            .collect()
    }

    /// Check line of sight between two positions (all intermediate tiles walkable).
    ///
    /// Returns `true` if every tile on the line from `from` to `to` is walkable.
    /// This is a simple implementation; true LOS would consider elevation.
    pub fn has_line_of_sight(&self, from: &HexPos, to: &HexPos) -> bool {
        from.line_to(to)
            .iter()
            .all(|p| self.tiles.get(p).is_some_and(|t| t.is_walkable()))
    }

    // ── Snapshot system ───────────────────────────────────────────────

    /// Take a snapshot of the current map state.
    ///
    /// Returns the snapshot index.
    pub fn snapshot(&mut self, tick: u64, label: Option<String>) -> usize {
        let snapshot = MapSnapshot {
            tick,
            tiles: self.tiles.clone(),
            label,
        };
        self.snapshots.push(snapshot);

        // Trim old snapshots
        while self.snapshots.len() > self.max_snapshots {
            self.snapshots.remove(0);
        }

        self.snapshots.len() - 1
    }

    /// Take a snapshot with a label.
    pub fn snapshot_labeled(&mut self, tick: u64, label: &str) -> usize {
        self.snapshot(tick, Some(label.to_string()))
    }

    /// Restore from a snapshot by index.
    ///
    /// Returns `true` if the snapshot existed and was restored.
    pub fn restore_snapshot(&mut self, index: usize) -> bool {
        if let Some(snapshot) = self.snapshots.get(index).cloned() {
            self.tiles = snapshot.tiles;
            self.version += 1;
            true
        } else {
            false
        }
    }

    /// Number of stored snapshots.
    pub fn snapshot_count(&self) -> usize {
        self.snapshots.len()
    }

    /// Get a reference to a snapshot by index.
    pub fn get_snapshot(&self, index: usize) -> Option<&MapSnapshot> {
        self.snapshots.get(index)
    }

    /// Clear all snapshots to free memory.
    pub fn clear_snapshots(&mut self) {
        self.snapshots.clear();
    }

    // ── Generation helpers ────────────────────────────────────────────

    /// Generate a hex map of the given radius filled with a single terrain type.
    ///
    /// Uses the ring algorithm to enumerate all positions within radius.
    pub fn generate_uniform(radius: i32, terrain: TerrainType) -> Self {
        let mut map = Self::with_radius(radius);
        for pos in HexPos::ORIGIN.ring(radius) {
            map.tiles.insert(pos, Tile::new(pos, terrain));
        }
        map.version = 1;
        map
    }

    /// Populate the map from the seeder's terrain grid.
    ///
    /// Converts the seeder's 2D grid (using offset coordinates) to axial hex
    /// positions. Each grid cell becomes a tile.
    pub fn from_seeder_grid(
        grid: &[Vec<super::super::seeder::Terrain>],
    ) -> Self {
        let mut map = Self::new();
        for (row_idx, row) in grid.iter().enumerate() {
            for (col_idx, seeder_terrain) in row.iter().enumerate() {
                let pos = HexPos::from_offset(col_idx as i32, row_idx as i32);
                let terrain = TerrainType::from_seeder_terrain(seeder_terrain);
                map.tiles.insert(pos, Tile::new(pos, terrain));
            }
        }
        map.version = 1;
        map
    }

    // ── Tick integration ──────────────────────────────────────────────

    /// Run per-tick updates on all tiles (resource regeneration, etc.).
    pub fn tick(&mut self) {
        for tile in self.tiles.values_mut() {
            tile.tick_resource_regen();
        }
        self.version += 1;
    }
}

impl Default for WorldMap {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_map() -> WorldMap {
        let mut map = WorldMap::with_radius(2);
        for pos in HexPos::ORIGIN.ring(2) {
            let terrain = if pos.distance_to(&HexPos::ORIGIN) <= 1 {
                TerrainType::Plains
            } else {
                TerrainType::Forest
            };
            map.tiles.insert(pos, Tile::new(pos, terrain));
        }
        map.version = 1;
        map
    }

    #[test]
    fn empty_map() {
        let map = WorldMap::new();
        assert_eq!(map.tile_count(), 0);
        assert_eq!(map.version(), 0);
        assert!(!map.is_bounded());
    }

    #[test]
    fn bounded_map() {
        let map = WorldMap::with_radius(5);
        assert!(map.is_bounded());
        assert_eq!(map.radius(), 5);
    }

    #[test]
    fn insert_and_get() {
        let mut map = WorldMap::new();
        let pos = HexPos::new(3, 4);
        let tile = Tile::new(pos, TerrainType::Mountain);
        map.insert(tile);
        assert_eq!(map.tile_count(), 1);
        assert!(map.get(&pos).is_some());
        assert_eq!(map.get(&pos).unwrap().terrain, TerrainType::Mountain);
    }

    #[test]
    fn in_bounds_check() {
        let map = WorldMap::with_radius(2);
        assert!(map.is_in_bounds(&HexPos::new(0, 0)));
        assert!(map.is_in_bounds(&HexPos::new(2, 0)));
        assert!(!map.is_in_bounds(&HexPos::new(3, 0)));
    }

    #[test]
    fn unbounded_always_in_bounds() {
        let map = WorldMap::new();
        assert!(map.is_in_bounds(&HexPos::new(999, 999)));
    }

    #[test]
    fn walkable_neighbors() {
        let map = make_test_map();
        let nbrs = map.walkable_neighbors(&HexPos::ORIGIN);
        // All 6 neighbors should be plains (distance <= 1)
        assert_eq!(nbrs.len(), 6);
        for tile in &nbrs {
            assert!(tile.is_walkable());
        }
    }

    #[test]
    fn tiles_in_radius() {
        let map = make_test_map();
        let tiles = map.tiles_in_radius(&HexPos::ORIGIN, 1);
        assert_eq!(tiles.len(), 7); // center + 6 neighbors
    }

    #[test]
    fn tiles_by_terrain() {
        let map = make_test_map();
        let plains = map.tiles_by_terrain(TerrainType::Plains);
        let forests = map.tiles_by_terrain(TerrainType::Forest);
        assert!(!plains.is_empty());
        assert!(!forests.is_empty());
        assert_eq!(plains.len() + forests.len(), map.tile_count());
    }

    #[test]
    fn snapshot_and_restore() {
        let mut map = make_test_map();
        let idx = map.snapshot(0, Some("initial".into()));

        // Modify the map
        let water_pos = HexPos::new(0, 0);
        map.insert(Tile::new(water_pos, TerrainType::Water));
        assert_eq!(map.get(&water_pos).unwrap().terrain, TerrainType::Water);

        // Restore
        assert!(map.restore_snapshot(idx));
        assert_eq!(
            map.get(&water_pos).unwrap().terrain,
            TerrainType::Plains
        );
    }

    #[test]
    fn snapshot_limit() {
        let mut map = WorldMap::with_snapshot_limit(3);
        map.insert(Tile::new(HexPos::ORIGIN, TerrainType::Plains));
        for i in 0..5 {
            map.snapshot(i as u64, None);
        }
        assert_eq!(map.snapshot_count(), 3);
    }

    #[test]
    fn generate_uniform() {
        let map = WorldMap::generate_uniform(3, TerrainType::Desert);
        assert_eq!(map.tile_count(), 37); // 1 + 6 + 12 + 18
        assert!(map.tiles.values().all(|t| t.terrain == TerrainType::Desert));
    }

    #[test]
    fn from_seeder_grid() {
        let mut seeder = crate::world::seeder::WorldSeeder::new(42);
        let grid = seeder.generate_terrain(5, 5);
        let map = WorldMap::from_seeder_grid(&grid);
        assert_eq!(map.tile_count(), 25);
    }

    #[test]
    fn tick_regen() {
        let mut map = WorldMap::new();
        let pos = HexPos::new(0, 0);
        let mut tile = Tile::new(pos, TerrainType::Forest);
        tile.add_resource(super::super::terrain::ResourceNode::renewable(
            "r1", "wood", 100, 10,
        ));
        tile.resources[0].amount = 50;
        map.insert(tile);

        map.tick();
        let t = map.get(&pos).unwrap();
        assert_eq!(t.resources[0].amount, 60);
    }

    #[test]
    fn agent_tracking() {
        let mut map = WorldMap::new();
        let pos = HexPos::new(0, 0);
        map.insert(Tile::new(pos, TerrainType::Plains));

        map.get_mut(&pos).unwrap().add_agent("agent-1");
        let tiles = map.tiles_with_agent("agent-1");
        assert_eq!(tiles.len(), 1);

        map.get_mut(&pos).unwrap().remove_agent("agent-1");
        let tiles = map.tiles_with_agent("agent-1");
        assert_eq!(tiles.len(), 0);
    }

    #[test]
    fn line_of_sight() {
        let mut map = WorldMap::new();
        for q in 0..5 {
            let pos = HexPos::new(q, 0);
            map.insert(Tile::new(pos, TerrainType::Plains));
        }
        assert!(map.has_line_of_sight(&HexPos::new(0, 0), &HexPos::new(4, 0)));
    }

    #[test]
    fn line_of_sight_blocked() {
        let mut map = WorldMap::new();
        for q in 0..5 {
            let pos = HexPos::new(q, 0);
            let terrain = if q == 2 {
                TerrainType::Water
            } else {
                TerrainType::Plains
            };
            map.insert(Tile::new(pos, terrain));
        }
        assert!(!map.has_line_of_sight(&HexPos::new(0, 0), &HexPos::new(4, 0)));
    }

    #[test]
    fn serialization_round_trip() {
        let mut map = make_test_map();
        map.get_mut(&HexPos::ORIGIN).unwrap().add_agent("agent-1");
        map.snapshot(0, Some("test".into()));

        let json = serde_json::to_string(&map).unwrap();
        let recovered: WorldMap = serde_json::from_str(&json).unwrap();
        assert_eq!(recovered.tile_count(), map.tile_count());
        assert_eq!(recovered.version(), map.version());
        assert_eq!(recovered.snapshot_count(), map.snapshot_count());
    }
}
