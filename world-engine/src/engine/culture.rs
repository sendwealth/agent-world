//! Culture data storage and operations for the World Engine.
//!
//! Stores organization culture vectors and regional cultural cluster data
//! in a thread-safe manner, consistent with the engine's DashMap-based patterns.

use std::collections::HashMap;

use dashmap::DashMap;
use serde::{Deserialize, Serialize};

/// An organization's culture vector — aggregate of member values.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrgCultureVector {
    /// Organization identifier.
    pub org_id: String,
    /// Cooperation norm [0.0, 1.0].
    pub cooperation_norm: f64,
    /// Competition norm [0.0, 1.0].
    pub competition_norm: f64,
    /// Exploration norm [0.0, 1.0].
    pub exploration_norm: f64,
    /// Tradition strength [0.0, 1.0].
    pub tradition_strength: f64,
    /// Innovation norm [0.0, 1.0].
    pub innovation_norm: f64,
}

/// A cultural cluster — group of agents with similar culture.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CulturalCluster {
    /// Cluster identifier.
    pub cluster_id: String,
    /// Agent IDs belonging to this cluster.
    pub agent_ids: Vec<String>,
    /// Center personality vector (keyed by dimension name).
    pub center_personality: HashMap<String, f64>,
    /// Center value weights (keyed by dimension name).
    pub center_values: HashMap<String, f64>,
    /// Associated region ID (if any).
    pub region_id: String,
}

/// Inter-group trust record between two groups.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupTrustRecord {
    /// Source group.
    pub source_group: String,
    /// Target group.
    pub target_group: String,
    /// Trust value [MIN_OUT_GROUP_TRUST, 1.0].
    pub trust_value: f64,
    /// Number of interactions recorded.
    pub interaction_count: u64,
}

/// Minimum out-group trust floor.
pub const MIN_OUT_GROUP_TRUST: f64 = 0.1;

/// Maximum cultural pressure per tick.
pub const MAX_CULTURE_PRESSURE_PER_TICK: f64 = 0.001;

/// Thread-safe culture store for organization cultures, cultural clusters,
/// and inter-group trust records.
pub struct CultureStore {
    /// org_id -> OrgCultureVector
    org_cultures: DashMap<String, OrgCultureVector>,
    /// cluster_id -> CulturalCluster
    clusters: DashMap<String, CulturalCluster>,
    /// (source_group, target_group) -> GroupTrustRecord
    trust_records: DashMap<(String, String), GroupTrustRecord>,
}

impl CultureStore {
    /// Create a new empty CultureStore.
    pub fn new() -> Self {
        Self {
            org_cultures: DashMap::new(),
            clusters: DashMap::new(),
            trust_records: DashMap::new(),
        }
    }

    // ── Organization Culture ──

    /// Store an organization culture vector.
    pub fn set_org_culture(&self, culture: OrgCultureVector) {
        self.org_cultures.insert(culture.org_id.clone(), culture);
    }

    /// Get an organization culture vector.
    pub fn get_org_culture(&self, org_id: &str) -> Option<OrgCultureVector> {
        self.org_cultures.get(org_id).map(|r| r.value().clone())
    }

    /// Remove an organization culture.
    pub fn remove_org_culture(&self, org_id: &str) -> Option<OrgCultureVector> {
        self.org_cultures.remove(org_id).map(|(_, v)| v)
    }

    /// List all org culture IDs.
    pub fn org_culture_ids(&self) -> Vec<String> {
        self.org_cultures.iter().map(|e| e.key().clone()).collect()
    }

    // ── Cultural Clusters ──

    /// Store a cultural cluster.
    pub fn set_cluster(&self, cluster: CulturalCluster) {
        self.clusters.insert(cluster.cluster_id.clone(), cluster);
    }

    /// Get a cultural cluster by ID.
    pub fn get_cluster(&self, cluster_id: &str) -> Option<CulturalCluster> {
        self.clusters.get(cluster_id).map(|r| r.value().clone())
    }

    /// Remove a cultural cluster.
    pub fn remove_cluster(&self, cluster_id: &str) -> Option<CulturalCluster> {
        self.clusters.remove(cluster_id).map(|(_, v)| v)
    }

    /// List all cluster IDs.
    pub fn cluster_ids(&self) -> Vec<String> {
        self.clusters.iter().map(|e| e.key().clone()).collect()
    }

    /// Find which cluster an agent belongs to.
    // TODO: add reverse index agent_id→cluster_id (DashMap<String, String>) maintained in set_cluster()
    // to avoid O(C × A) linear scan over all clusters.
    pub fn find_agent_cluster(&self, agent_id: &str) -> Option<CulturalCluster> {
        for entry in self.clusters.iter() {
            if entry.value().agent_ids.contains(&agent_id.to_string()) {
                return Some(entry.value().clone());
            }
        }
        None
    }

    // ── Inter-Group Trust ──

    /// Set trust between two groups.
    pub fn set_trust(&self, source: &str, target: &str, value: f64) {
        let clamped = value.clamp(MIN_OUT_GROUP_TRUST, 1.0);
        let key = (source.to_string(), target.to_string());
        self.trust_records.insert(
            key,
            GroupTrustRecord {
                source_group: source.to_string(),
                target_group: target.to_string(),
                trust_value: clamped,
                interaction_count: 0,
            },
        );
    }

    /// Get trust value between two groups. Returns default 0.3 if not set.
    pub fn get_trust(&self, source: &str, target: &str) -> f64 {
        let key = (source.to_string(), target.to_string());
        self.trust_records
            .get(&key)
            .map(|r| r.trust_value)
            .unwrap_or(0.3)
    }

    /// Adjust trust between two groups by a delta, clamped to valid range.
    pub fn adjust_trust(&self, source: &str, target: &str, delta: f64) -> f64 {
        let key = (source.to_string(), target.to_string());
        let mut entry = self
            .trust_records
            .entry(key)
            .or_insert_with(|| GroupTrustRecord {
                source_group: source.to_string(),
                target_group: target.to_string(),
                trust_value: 0.3,
                interaction_count: 0,
            });

        entry.trust_value = (entry.trust_value + delta).clamp(MIN_OUT_GROUP_TRUST, 1.0);
        entry.interaction_count += 1;
        entry.trust_value
    }

    /// Get all trust records for a given source group.
    pub fn trust_for_group(&self, group_id: &str) -> Vec<GroupTrustRecord> {
        self.trust_records
            .iter()
            .filter(|e| e.key().0 == group_id)
            .map(|e| e.value().clone())
            .collect()
    }
}

impl Default for CultureStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_org_culture_crud() {
        let store = CultureStore::new();

        let culture = OrgCultureVector {
            org_id: "org_1".to_string(),
            cooperation_norm: 0.7,
            competition_norm: 0.3,
            exploration_norm: 0.5,
            tradition_strength: 0.4,
            innovation_norm: 0.6,
        };

        store.set_org_culture(culture.clone());
        let retrieved = store.get_org_culture("org_1").unwrap();
        assert_eq!(retrieved.cooperation_norm, 0.7);

        store.remove_org_culture("org_1");
        assert!(store.get_org_culture("org_1").is_none());
    }

    #[test]
    fn test_cluster_crud() {
        let store = CultureStore::new();

        let cluster = CulturalCluster {
            cluster_id: "cluster_0".to_string(),
            agent_ids: vec!["a1".to_string(), "a2".to_string()],
            center_personality: HashMap::from([("openness".to_string(), 0.5)]),
            center_values: HashMap::from([("cooperation_weight".to_string(), 0.6)]),
            region_id: "region_1".to_string(),
        };

        store.set_cluster(cluster);
        let retrieved = store.get_cluster("cluster_0").unwrap();
        assert_eq!(retrieved.agent_ids.len(), 2);

        // Find by agent
        let found = store.find_agent_cluster("a1").unwrap();
        assert_eq!(found.cluster_id, "cluster_0");

        assert!(store.find_agent_cluster("a99").is_none());
    }

    #[test]
    fn test_trust_crud() {
        let store = CultureStore::new();

        // Default trust
        assert_eq!(store.get_trust("g1", "g2"), 0.3);

        // Set trust
        store.set_trust("g1", "g2", 0.8);
        assert_eq!(store.get_trust("g1", "g2"), 0.8);

        // Adjust trust
        let new_val = store.adjust_trust("g1", "g2", -0.1);
        assert!((new_val - 0.7).abs() < 1e-9);

        // Trust is floored at MIN_OUT_GROUP_TRUST
        store.set_trust("g1", "g3", 0.05);
        assert_eq!(store.get_trust("g1", "g3"), MIN_OUT_GROUP_TRUST);
    }

    #[test]
    fn test_trust_for_group() {
        let store = CultureStore::new();
        store.set_trust("g1", "g2", 0.6);
        store.set_trust("g1", "g3", 0.8);

        let records = store.trust_for_group("g1");
        assert_eq!(records.len(), 2);
    }
}
