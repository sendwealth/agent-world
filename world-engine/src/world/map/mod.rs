//! World map module — hex terrain, buildings, and spatial structures.

pub mod building;

pub use building::{
    Building, BuildingType, BuildingStatus, BuildingCost, OwnerType,
    BuildingManager, BuildingId,
};
