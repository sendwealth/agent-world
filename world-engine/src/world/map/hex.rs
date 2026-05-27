//! Hexagonal coordinate system using axial (cube) coordinates.
//!
//! Implements a pointy-top hex grid with the standard axial coordinate system
//! (q, r) where `s = -q - r` is implicit. This is the most common hex coordinate
//! system for game worlds and supports efficient neighbor/distance calculations.
//!
//! # Coordinate System
//!
//! ```text
//!        (+s)           Pointy-top orientation:
//!         \             Axial coords (q, r) with implicit s = -q - r.
//!    (-r) -- (q, r) -- (+r)    Cube constraint: q + r + s = 0
//!         /
//!        (-s)
//! ```

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Axial hex coordinate for the pointy-top hex grid.
///
/// Uses axial coordinates (q, r) with an implicit third axis `s = -q - r`
/// to satisfy the cube coordinate constraint `q + r + s = 0`.
///
/// The optional `layer` field supports multi-layer maps (e.g., underground,
/// surface, sky) for future expansion.
///
/// Serializes as a string key `"q,r,Llayer"` for use as HashMap keys in JSON.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HexPos {
    /// Column coordinate (axial q).
    pub q: i32,
    /// Row coordinate (axial r).
    pub r: i32,
    /// Layer (0 = surface by default). Allows vertical stacking of maps.
    pub layer: i32,
}

// Custom serialization: serialize as "q,r,Llayer" string for JSON map keys.
impl Serialize for HexPos {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if self.layer == 0 {
            serializer.serialize_str(&format!("{},{}", self.q, self.r))
        } else {
            serializer.serialize_str(&format!("{},{},L{}", self.q, self.r, self.layer))
        }
    }
}

impl<'de> Deserialize<'de> for HexPos {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        let parts: Vec<&str> = s.split(',').collect();
        if parts.len() < 2 {
            return Err(serde::de::Error::custom("expected 'q,r' or 'q,r,Llayer'"));
        }
        let q: i32 = parts[0].parse().map_err(serde::de::Error::custom)?;
        let r: i32 = parts[1].parse().map_err(serde::de::Error::custom)?;
        let layer = if parts.len() > 2 {
            parts[2].trim_start_matches('L').parse().unwrap_or(0)
        } else {
            0
        };
        Ok(HexPos { q, r, layer })
    }
}

impl HexPos {
    /// Create a new hex position at the given axial coordinates on the surface layer (0).
    pub fn new(q: i32, r: i32) -> Self {
        Self { q, r, layer: 0 }
    }

    /// Create a hex position with an explicit layer.
    pub fn with_layer(q: i32, r: i32, layer: i32) -> Self {
        Self { q, r, layer }
    }

    /// The implicit cube coordinate `s = -q - r`.
    pub fn s(&self) -> i32 {
        -self.q - self.r
    }

    /// Origin position (0, 0, layer 0).
    pub const ORIGIN: HexPos = HexPos { q: 0, r: 0, layer: 0 };

    // ── Neighbor offsets for pointy-top hex grid ──────────────────────

    /// The six neighbor offset directions for a pointy-top hex grid
    /// in axial coordinates (dq, dr).
    ///
    /// ```text
    ///      (+1, -1)  (-1, 0)
    ///           \    /
    ///        (0, -1)--(0, +1)
    ///           /    \
    ///      (-1, +1)  (+1, 0)
    /// ```
    pub const NEIGHBOR_OFFSETS: [(i32, i32); 6] = [
        (1, 0),   // East
        (1, -1),  // Northeast
        (0, -1),  // Northwest
        (-1, 0),  // West
        (-1, 1),  // Southwest
        (0, 1),   // Southeast
    ];

    /// Return the six neighboring positions (same layer).
    ///
    /// Order: East, Northeast, Northwest, West, Southwest, Southeast.
    pub fn neighbors(&self) -> [HexPos; 6] {
        Self::NEIGHBOR_OFFSETS.map(|(dq, dr)| HexPos {
            q: self.q + dq,
            r: self.r + dr,
            layer: self.layer,
        })
    }

    /// Return the neighbor in a specific direction (0–5).
    ///
    /// Direction index:
    /// - 0: East
    /// - 1: Northeast
    /// - 2: Northwest
    /// - 3: West
    /// - 4: Southwest
    /// - 5: Southeast
    pub fn neighbor(&self, direction: usize) -> HexPos {
        let (dq, dr) = Self::NEIGHBOR_OFFSETS[direction % 6];
        HexPos {
            q: self.q + dq,
            r: self.r + dr,
            layer: self.layer,
        }
    }

    /// Compute the hex-grid distance to another position on the same layer.
    ///
    /// Uses the cube distance formula: `max(|dq|, |dr|, |ds|)` where `ds = -dq - dr`.
    /// This is equivalent to `(|dq| + |dr| + |ds|) / 2`.
    pub fn distance_to(&self, other: &HexPos) -> i32 {
        let dq = (self.q - other.q).abs();
        let dr = (self.r - other.r).abs();
        let ds = (self.s() - other.s()).abs();
        dq.max(dr).max(ds)
    }

    /// Whether `other` is an immediate neighbor (distance == 1, same layer).
    pub fn is_neighbor(&self, other: &HexPos) -> bool {
        self.layer == other.layer && self.distance_to(other) == 1
    }

    /// Return all positions within `radius` (inclusive) on the same layer.
    ///
    /// A radius of 0 returns just this position. Radius 1 returns this
    /// position plus its 6 neighbors (7 total).
    pub fn ring(&self, radius: i32) -> Vec<HexPos> {
        let mut result = Vec::new();
        for dq in -radius..=radius {
            let dr_min = (-radius).max(-dq - radius);
            let dr_max = radius.min(-dq + radius);
            for dr in dr_min..=dr_max {
                result.push(HexPos {
                    q: self.q + dq,
                    r: self.r + dr,
                    layer: self.layer,
                });
            }
        }
        result
    }

    /// Linearly interpolate between two hex positions (for line drawing).
    fn lerp(a: i32, b: i32, t: f64) -> f64 {
        a as f64 + (b as f64 - a as f64) * t
    }

    /// Return all positions on the straight line from `self` to `other` (inclusive).
    ///
    /// Uses the cube-coordinate line-drawing algorithm.
    pub fn line_to(&self, other: &HexPos) -> Vec<HexPos> {
        let distance = self.distance_to(other);
        if distance == 0 {
            return vec![*self];
        }

        let mut result = Vec::with_capacity((distance + 1) as usize);
        let n = distance as f64;
        for i in 0..=distance {
            let t = i as f64 / n;
            let q = Self::lerp(self.q, other.q, t).round() as i32;
            let r = Self::lerp(self.r, other.r, t).round() as i32;
            result.push(HexPos {
                q,
                r,
                layer: self.layer,
            });
        }
        result
    }

    /// Convert to offset coordinates (odd-r) for storage/interop.
    ///
    /// Odd-r offset is commonly used in tilemap editors.
    pub fn to_offset(&self) -> (i32, i32) {
        let col = self.q + (self.r - (self.r & 1)) / 2;
        let row = self.r;
        (col, row)
    }

    /// Convert from odd-r offset coordinates to axial.
    pub fn from_offset(col: i32, row: i32) -> Self {
        let q = col - (row - (row & 1)) / 2;
        let r = row;
        HexPos::new(q, r)
    }
}

impl fmt::Display for HexPos {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({},{},L{})", self.q, self.r, self.layer)
    }
}

impl Default for HexPos {
    fn default() -> Self {
        Self::ORIGIN
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origin_is_zero() {
        let o = HexPos::ORIGIN;
        assert_eq!(o.q, 0);
        assert_eq!(o.r, 0);
        assert_eq!(o.layer, 0);
        assert_eq!(o.s(), 0);
    }

    #[test]
    fn cube_constraint() {
        // q + r + s == 0
        let pos = HexPos::new(3, -5);
        assert_eq!(pos.q + pos.r + pos.s(), 0);
    }

    #[test]
    fn distance_to_self() {
        let pos = HexPos::new(3, 7);
        assert_eq!(pos.distance_to(&pos), 0);
    }

    #[test]
    fn distance_symmetry() {
        let a = HexPos::new(1, 2);
        let b = HexPos::new(4, -1);
        assert_eq!(a.distance_to(&b), b.distance_to(&a));
    }

    #[test]
    fn distance_known_values() {
        // (0,0) to (1,0) = 1 (east neighbor)
        assert_eq!(HexPos::ORIGIN.distance_to(&HexPos::new(1, 0)), 1);
        // (0,0) to (2,-1) = 2
        assert_eq!(HexPos::ORIGIN.distance_to(&HexPos::new(2, -1)), 2);
        // (0,0) to (3,-2) = 3
        assert_eq!(HexPos::ORIGIN.distance_to(&HexPos::new(3, -2)), 3);
    }

    #[test]
    fn six_neighbors() {
        let pos = HexPos::new(0, 0);
        let nbrs = pos.neighbors();
        assert_eq!(nbrs.len(), 6);

        // All neighbors should be at distance 1
        for nbr in &nbrs {
            assert_eq!(pos.distance_to(nbr), 1);
        }

        // All neighbors should be distinct
        let set: std::collections::HashSet<_> = nbrs.iter().collect();
        assert_eq!(set.len(), 6);
    }

    #[test]
    fn neighbor_by_direction() {
        let pos = HexPos::new(0, 0);
        assert_eq!(pos.neighbor(0), HexPos::new(1, 0));   // East
        assert_eq!(pos.neighbor(1), HexPos::new(1, -1));  // NE
        assert_eq!(pos.neighbor(2), HexPos::new(0, -1));  // NW
        assert_eq!(pos.neighbor(3), HexPos::new(-1, 0));  // West
        assert_eq!(pos.neighbor(4), HexPos::new(-1, 1));  // SW
        assert_eq!(pos.neighbor(5), HexPos::new(0, 1));   // SE
    }

    #[test]
    fn is_neighbor_check() {
        let origin = HexPos::new(0, 0);
        assert!(origin.is_neighbor(&HexPos::new(1, 0)));
        assert!(origin.is_neighbor(&HexPos::new(0, -1)));
        assert!(!origin.is_neighbor(&HexPos::new(2, 0)));
        assert!(!origin.is_neighbor(&HexPos::with_layer(1, 0, 1)));
    }

    #[test]
    fn ring_radius_zero() {
        let pos = HexPos::new(5, 5);
        let ring = pos.ring(0);
        assert_eq!(ring.len(), 1);
        assert_eq!(ring[0], pos);
    }

    #[test]
    fn ring_radius_one() {
        let ring = HexPos::ORIGIN.ring(1);
        assert_eq!(ring.len(), 7); // center + 6 neighbors
    }

    #[test]
    fn ring_radius_two() {
        let ring = HexPos::ORIGIN.ring(2);
        // Hex numbers: 1 + 6 + 12 = 19
        assert_eq!(ring.len(), 19);
    }

    #[test]
    fn line_to_straight() {
        let start = HexPos::new(0, 0);
        let end = HexPos::new(3, 0);
        let line = start.line_to(&end);
        assert_eq!(line.len(), 4);
        assert_eq!(line[0], start);
        assert_eq!(line[3], end);
    }

    #[test]
    fn line_to_diagonal() {
        let start = HexPos::new(0, 0);
        let end = HexPos::new(3, -3);
        let line = start.line_to(&end);
        assert_eq!(line[0], start);
        assert_eq!(line.last().unwrap(), &end);
    }

    #[test]
    fn offset_round_trip() {
        let positions = [
            HexPos::new(0, 0),
            HexPos::new(1, 0),
            HexPos::new(0, 1),
            HexPos::new(-1, 2),
            HexPos::new(3, -2),
        ];
        for pos in positions {
            let (col, row) = pos.to_offset();
            let recovered = HexPos::from_offset(col, row);
            assert_eq!(pos, recovered, "round-trip failed for {}", pos);
        }
    }

    #[test]
    fn display_format() {
        let pos = HexPos::new(3, -2);
        assert_eq!(format!("{}", pos), "(3,-2,L0)");
    }

    #[test]
    fn layer_preserved() {
        let pos = HexPos::with_layer(1, 2, 3);
        assert_eq!(pos.layer, 3);
        let nbrs = pos.neighbors();
        for nbr in &nbrs {
            assert_eq!(nbr.layer, 3);
        }
    }

    #[test]
    fn serialization_round_trip() {
        let pos = HexPos::with_layer(5, -3, 2);
        let json = serde_json::to_string(&pos).unwrap();
        let recovered: HexPos = serde_json::from_str(&json).unwrap();
        assert_eq!(pos, recovered);
    }
}
