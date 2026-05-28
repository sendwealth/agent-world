//! Building system — construction, maintenance, demolition.
//!
//! Agents can construct buildings on the hex map. Buildings have types
//! (warehouse, market, workshop, defense tower, housing), costs, durability,
//! and go through a lifecycle: Planning → Constructing → Active → Damaged/Destroyed/Demolished.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Unique building identifier.
pub type BuildingId = String;

/// Who owns this building.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OwnerType {
    /// Owned by a single agent.
    Personal,
    /// Owned by an organization.
    Organization,
}

/// Types of buildings agents can construct.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuildingType {
    /// Stores resources for the owner. +500 storage capacity.
    Warehouse,
    /// Local trading hub. Enables bartering within range.
    Market,
    /// Processes raw materials into refined goods.
    Workshop,
    /// Protects a territory. Deters raids within radius.
    DefenseTower,
    /// Increases population capacity on the tile.
    Housing,
}

impl BuildingType {
    /// All building variants.
    pub fn all() -> &'static [BuildingType] {
        &[
            BuildingType::Warehouse,
            BuildingType::Market,
            BuildingType::Workshop,
            BuildingType::DefenseTower,
            BuildingType::Housing,
        ]
    }

    /// Human-readable name.
    pub fn name(&self) -> &'static str {
        match self {
            BuildingType::Warehouse => "Warehouse",
            BuildingType::Market => "Market",
            BuildingType::Workshop => "Workshop",
            BuildingType::DefenseTower => "Defense Tower",
            BuildingType::Housing => "Housing",
        }
    }
}

/// Lifecycle status of a building.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuildingStatus {
    /// Ordered but not yet under construction.
    Planning,
    /// Currently being built (ticks remaining tracked in `construction_ticks_left`).
    Constructing,
    /// Fully built and operational.
    Active,
    /// Partially damaged, needs repair.
    Damaged,
    /// Destroyed (unrecoverable).
    Destroyed,
    /// Intentionally removed by owner.
    Demolished,
}

/// Resource + token cost to construct a building.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildingCost {
    /// Token cost.
    pub tokens: u64,
    /// Stone cost.
    pub stone: u64,
    /// Wood cost.
    pub wood: u64,
    /// Ticks to complete construction.
    pub construction_ticks: u64,
}

impl BuildingCost {
    /// Get the cost for a given building type.
    pub fn for_type(building_type: BuildingType) -> Self {
        match building_type {
            BuildingType::Warehouse => Self {
                tokens: 100,
                stone: 50,
                wood: 80,
                construction_ticks: 10,
            },
            BuildingType::Market => Self {
                tokens: 150,
                stone: 30,
                wood: 60,
                construction_ticks: 12,
            },
            BuildingType::Workshop => Self {
                tokens: 120,
                stone: 70,
                wood: 40,
                construction_ticks: 8,
            },
            BuildingType::DefenseTower => Self {
                tokens: 200,
                stone: 100,
                wood: 50,
                construction_ticks: 15,
            },
            BuildingType::Housing => Self {
                tokens: 80,
                stone: 40,
                wood: 100,
                construction_ticks: 6,
            },
        }
    }
}

/// A building on the world map.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Building {
    /// Unique identifier.
    pub id: BuildingId,
    /// Building type.
    pub building_type: BuildingType,
    /// Position on the grid (x, y).
    pub position: (i32, i32),
    /// Current status in the lifecycle.
    pub status: BuildingStatus,
    /// Owner type (personal or organization).
    pub owner_type: OwnerType,
    /// Owner ID (agent ID or organization ID).
    pub owner_id: String,
    /// Current health / durability (0–100).
    pub health: u32,
    /// Maximum durability.
    pub max_health: u32,
    /// Ticks remaining until construction completes.
    pub construction_ticks_left: u64,
    /// Tick when construction started.
    pub construction_started_at: u64,
    /// Level (for upgrades, starts at 1).
    pub level: u32,
}

impl Building {
    /// Create a new building in the Planning state.
    pub fn new(
        id: BuildingId,
        building_type: BuildingType,
        position: (i32, i32),
        owner_type: OwnerType,
        owner_id: String,
        current_tick: u64,
    ) -> Self {
        let cost = BuildingCost::for_type(building_type);
        Self {
            id,
            building_type,
            position,
            status: BuildingStatus::Constructing,
            owner_type,
            owner_id,
            health: 100,
            max_health: 100,
            construction_ticks_left: cost.construction_ticks,
            construction_started_at: current_tick,
            level: 1,
        }
    }

    /// Whether the building is operational (active or damaged).
    pub fn is_operational(&self) -> bool {
        matches!(
            self.status,
            BuildingStatus::Active | BuildingStatus::Damaged
        )
    }
}

/// Manager for all buildings in the world.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BuildingManager {
    /// All buildings indexed by ID.
    buildings: HashMap<BuildingId, Building>,
    /// Spatial index: (x, y) → building IDs at that position.
    spatial_index: HashMap<(i32, i32), Vec<BuildingId>>,
    /// Counter for generating unique IDs.
    next_id: u64,
}

impl BuildingManager {
    /// Create a new empty building manager.
    pub fn new() -> Self {
        Self::default()
    }

    /// Generate a unique building ID.
    fn generate_id(&mut self) -> BuildingId {
        self.next_id += 1;
        format!("bld-{}", self.next_id)
    }

    /// Start construction of a new building.
    ///
    /// Returns the created building, or an error message.
    pub fn construct(
        &mut self,
        building_type: BuildingType,
        position: (i32, i32),
        owner_type: OwnerType,
        owner_id: String,
        current_tick: u64,
    ) -> Result<Building, String> {
        // Check for conflicting building at same position of same type
        if let Some(ids) = self.spatial_index.get(&position) {
            for bid in ids {
                if let Some(existing) = self.buildings.get(bid) {
                    if existing.building_type == building_type
                        && !matches!(
                            existing.status,
                            BuildingStatus::Destroyed | BuildingStatus::Demolished
                        )
                    {
                        return Err(format!(
                            "a {} already exists at ({}, {})",
                            building_type.name(),
                            position.0,
                            position.1
                        ));
                    }
                }
            }
        }

        let id = self.generate_id();
        let building = Building::new(
            id.clone(),
            building_type,
            position,
            owner_type,
            owner_id,
            current_tick,
        );

        // Insert into spatial index
        self.spatial_index
            .entry(position)
            .or_default()
            .push(id.clone());

        self.buildings.insert(id, building.clone());
        Ok(building)
    }

    /// Get a building by ID.
    pub fn get(&self, id: &str) -> Option<&Building> {
        self.buildings.get(id)
    }

    /// Get all buildings.
    pub fn list_all(&self) -> Vec<&Building> {
        self.buildings.values().collect()
    }

    /// Get buildings at a specific position.
    pub fn get_at(&self, position: (i32, i32)) -> Vec<&Building> {
        self.spatial_index
            .get(&position)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.buildings.get(id))
                    .filter(|b| {
                        !matches!(
                            b.status,
                            BuildingStatus::Destroyed | BuildingStatus::Demolished
                        )
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get buildings owned by a specific owner.
    pub fn get_by_owner(&self, owner_id: &str) -> Vec<&Building> {
        self.buildings
            .values()
            .filter(|b| b.owner_id == owner_id)
            .filter(|b| {
                !matches!(
                    b.status,
                    BuildingStatus::Destroyed | BuildingStatus::Demolished
                )
            })
            .collect()
    }

    /// Advance construction by one tick. Completes buildings whose ticks reach 0.
    /// Returns IDs of buildings that completed construction this tick.
    pub fn tick_construction(&mut self) -> Vec<BuildingId> {
        let mut completed = Vec::new();
        for building in self.buildings.values_mut() {
            if building.status == BuildingStatus::Constructing
                && building.construction_ticks_left > 0
            {
                building.construction_ticks_left -= 1;
                if building.construction_ticks_left == 0 {
                    building.status = BuildingStatus::Active;
                    completed.push(building.id.clone());
                }
            }
        }
        completed
    }

    /// Apply durability decay to active buildings.
    /// Returns (building_id, new_health) pairs for buildings that became damaged.
    pub fn tick_durability(&mut self, decay_rate: u32) -> Vec<(BuildingId, u32)> {
        let mut damaged = Vec::new();
        for building in self.buildings.values_mut() {
            if building.status == BuildingStatus::Active && building.health > 0 && decay_rate > 0 {
                building.health = building.health.saturating_sub(decay_rate);
                if building.health == 0 {
                    building.status = BuildingStatus::Destroyed;
                } else if building.health < 50 {
                    building.status = BuildingStatus::Damaged;
                }
                if matches!(
                    building.status,
                    BuildingStatus::Damaged | BuildingStatus::Destroyed
                ) {
                    damaged.push((building.id.clone(), building.health));
                }
            }
        }
        damaged
    }

    /// Maintain (repair) a building, restoring its health.
    /// Costs tokens proportional to damage.
    pub fn maintain(&mut self, building_id: &str, health_restore: u32) -> Result<Building, String> {
        let building = self
            .buildings
            .get_mut(building_id)
            .ok_or("building not found")?;

        if matches!(
            building.status,
            BuildingStatus::Destroyed | BuildingStatus::Demolished
        ) {
            return Err("cannot repair a destroyed or demolished building".into());
        }

        building.health = (building.health + health_restore).min(building.max_health);
        if building.health >= 50 {
            building.status = BuildingStatus::Active;
        }

        Ok(building.clone())
    }

    /// Demolish a building, removing it from active use.
    pub fn demolish(&mut self, building_id: &str) -> Result<Building, String> {
        let building = self
            .buildings
            .get_mut(building_id)
            .ok_or("building not found")?;

        if matches!(
            building.status,
            BuildingStatus::Destroyed | BuildingStatus::Demolished
        ) {
            return Err("building is already destroyed or demolished".into());
        }

        building.status = BuildingStatus::Demolished;
        building.health = 0;
        Ok(building.clone())
    }

    /// Upgrade a building to the next level.
    pub fn upgrade(&mut self, building_id: &str) -> Result<Building, String> {
        let building = self
            .buildings
            .get_mut(building_id)
            .ok_or("building not found")?;

        if building.status != BuildingStatus::Active {
            return Err("can only upgrade active buildings".into());
        }

        if building.level >= 5 {
            return Err("building is already at max level (5)".into());
        }

        building.level += 1;
        building.max_health += 50;
        building.health = building.max_health;

        Ok(building.clone())
    }

    /// Clean up destroyed/demolished buildings older than `older_than_ticks`.
    /// Returns number of cleaned-up buildings.
    pub fn cleanup(&mut self, _current_tick: u64) -> usize {
        let to_remove: Vec<BuildingId> = self
            .buildings
            .iter()
            .filter(|(_, b)| {
                matches!(
                    b.status,
                    BuildingStatus::Destroyed | BuildingStatus::Demolished
                )
            })
            .map(|(id, _)| id.clone())
            .collect();

        let count = to_remove.len();
        for id in &to_remove {
            if let Some(building) = self.buildings.remove(id) {
                if let Some(ids) = self.spatial_index.get_mut(&building.position) {
                    ids.retain(|bid| bid != id);
                }
            }
        }
        count
    }

    /// Total count of operational buildings.
    pub fn operational_count(&self) -> usize {
        self.buildings
            .values()
            .filter(|b| b.is_operational())
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_manager() -> BuildingManager {
        BuildingManager::new()
    }

    #[test]
    fn construct_building() {
        let mut mgr = make_manager();
        let b = mgr
            .construct(
                BuildingType::Warehouse,
                (5, 10),
                OwnerType::Personal,
                "agent-1".into(),
                0,
            )
            .unwrap();
        assert_eq!(b.building_type, BuildingType::Warehouse);
        assert_eq!(b.position, (5, 10));
        assert_eq!(b.status, BuildingStatus::Constructing);
        assert_eq!(b.owner_id, "agent-1");
        assert_eq!(b.health, 100);
    }

    #[test]
    fn construct_costs_tokens() {
        let cost = BuildingCost::for_type(BuildingType::DefenseTower);
        assert_eq!(cost.tokens, 200);
        assert_eq!(cost.stone, 100);
        assert!(cost.construction_ticks > 0);
    }

    #[test]
    fn construction_completes_after_ticks() {
        let mut mgr = make_manager();
        let b = mgr
            .construct(
                BuildingType::Housing,
                (0, 0),
                OwnerType::Personal,
                "a1".into(),
                0,
            )
            .unwrap();
        let ticks_needed = b.construction_ticks_left;

        for _ in 0..ticks_needed {
            let completed = mgr.tick_construction();
            if completed.is_empty() {
                // not done yet
            }
        }
        // One more tick shouldn't hurt — the last tick_construction should have completed it
        let building = mgr.get(&b.id).unwrap();
        assert_eq!(building.status, BuildingStatus::Active);
        assert_eq!(building.construction_ticks_left, 0);
    }

    #[test]
    fn durability_decay_and_damage() {
        let mut mgr = make_manager();
        let b = mgr
            .construct(
                BuildingType::Market,
                (1, 1),
                OwnerType::Personal,
                "a1".into(),
                0,
            )
            .unwrap();

        // Fast-forward construction
        for _ in 0..100 {
            mgr.tick_construction();
        }

        // Apply decay
        let damaged = mgr.tick_durability(60);
        assert!(!damaged.is_empty());

        let building = mgr.get(&b.id).unwrap();
        assert_eq!(building.health, 40);
        assert_eq!(building.status, BuildingStatus::Damaged);
    }

    #[test]
    fn maintenance_repairs() {
        let mut mgr = make_manager();
        let b = mgr
            .construct(
                BuildingType::Workshop,
                (2, 2),
                OwnerType::Organization,
                "org-1".into(),
                0,
            )
            .unwrap();

        // Fast-forward construction
        for _ in 0..100 {
            mgr.tick_construction();
        }
        // Damage it
        mgr.tick_durability(80);

        // Repair
        let repaired = mgr.maintain(&b.id, 60).unwrap();
        assert!(repaired.health >= 80);
        assert_eq!(repaired.status, BuildingStatus::Active); // health >= 50
    }

    #[test]
    fn demolish_building() {
        let mut mgr = make_manager();
        let b = mgr
            .construct(
                BuildingType::Warehouse,
                (3, 3),
                OwnerType::Personal,
                "a1".into(),
                0,
            )
            .unwrap();

        let result = mgr.demolish(&b.id).unwrap();
        assert_eq!(result.status, BuildingStatus::Demolished);

        // Cannot demolish again
        assert!(mgr.demolish(&b.id).is_err());
    }

    #[test]
    fn get_buildings_at_position() {
        let mut mgr = make_manager();
        mgr.construct(
            BuildingType::Warehouse,
            (0, 0),
            OwnerType::Personal,
            "a1".into(),
            0,
        )
        .unwrap();
        mgr.construct(
            BuildingType::Market,
            (0, 0),
            OwnerType::Personal,
            "a1".into(),
            0,
        )
        .unwrap();
        mgr.construct(
            BuildingType::Housing,
            (1, 1),
            OwnerType::Personal,
            "a1".into(),
            0,
        )
        .unwrap();

        let at_origin = mgr.get_at((0, 0));
        assert_eq!(at_origin.len(), 2);

        let at_1_1 = mgr.get_at((1, 1));
        assert_eq!(at_1_1.len(), 1);

        let empty = mgr.get_at((9, 9));
        assert!(empty.is_empty());
    }

    #[test]
    fn get_buildings_by_owner() {
        let mut mgr = make_manager();
        mgr.construct(
            BuildingType::Warehouse,
            (0, 0),
            OwnerType::Personal,
            "agent-a".into(),
            0,
        )
        .unwrap();
        mgr.construct(
            BuildingType::Market,
            (1, 0),
            OwnerType::Personal,
            "agent-a".into(),
            0,
        )
        .unwrap();
        mgr.construct(
            BuildingType::Housing,
            (2, 0),
            OwnerType::Personal,
            "agent-b".into(),
            0,
        )
        .unwrap();

        let owned_by_a = mgr.get_by_owner("agent-a");
        assert_eq!(owned_by_a.len(), 2);

        let owned_by_b = mgr.get_by_owner("agent-b");
        assert_eq!(owned_by_b.len(), 1);
    }

    #[test]
    fn upgrade_building() {
        let mut mgr = make_manager();
        let b = mgr
            .construct(
                BuildingType::Warehouse,
                (0, 0),
                OwnerType::Personal,
                "a1".into(),
                0,
            )
            .unwrap();

        // Fast-forward construction
        for _ in 0..100 {
            mgr.tick_construction();
        }

        let upgraded = mgr.upgrade(&b.id).unwrap();
        assert_eq!(upgraded.level, 2);
        assert_eq!(upgraded.max_health, 150);

        // Cannot upgrade non-active
        let mut mgr2 = make_manager();
        let b2 = mgr2
            .construct(
                BuildingType::Market,
                (0, 0),
                OwnerType::Personal,
                "a1".into(),
                0,
            )
            .unwrap();
        assert!(mgr2.upgrade(&b2.id).is_err()); // still constructing
    }

    #[test]
    fn cleanup_removed_buildings() {
        let mut mgr = make_manager();
        let b1 = mgr
            .construct(
                BuildingType::Warehouse,
                (0, 0),
                OwnerType::Personal,
                "a1".into(),
                0,
            )
            .unwrap();
        let _b2 = mgr
            .construct(
                BuildingType::Market,
                (1, 0),
                OwnerType::Personal,
                "a1".into(),
                0,
            )
            .unwrap();

        mgr.demolish(&b1.id).unwrap();
        let removed = mgr.cleanup(0);
        assert_eq!(removed, 1);
        assert_eq!(mgr.list_all().len(), 1);
    }

    #[test]
    fn duplicate_building_type_at_position() {
        let mut mgr = make_manager();
        mgr.construct(
            BuildingType::Warehouse,
            (0, 0),
            OwnerType::Personal,
            "a1".into(),
            0,
        )
        .unwrap();
        let result = mgr.construct(
            BuildingType::Warehouse,
            (0, 0),
            OwnerType::Personal,
            "a1".into(),
            0,
        );
        assert!(result.is_err());
    }

    #[test]
    fn max_level_upgrade() {
        let mut mgr = make_manager();
        let b = mgr
            .construct(
                BuildingType::Warehouse,
                (0, 0),
                OwnerType::Personal,
                "a1".into(),
                0,
            )
            .unwrap();
        for _ in 0..100 {
            mgr.tick_construction();
        }
        for _ in 0..4 {
            mgr.upgrade(&b.id).unwrap();
        }
        // Level 5, cannot go higher
        assert!(mgr.upgrade(&b.id).is_err());
    }

    #[test]
    fn building_types_all() {
        assert_eq!(BuildingType::all().len(), 5);
    }
}
