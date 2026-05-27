//! Snapshot storage backend — file-based storage with zstd compression.
//!
//! Stores snapshots on disk using zstd-compressed JSON. Supports:
//! - Full snapshots (complete world state)
//! - Incremental deltas (only changes from previous snapshot)
//! - Automatic pruning of old snapshots (group-based to protect delta chains)
//! - Compression ratio tracking

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use uuid::Uuid;

use super::types::{SnapshotConfig, SnapshotDelta, SnapshotKind, SnapshotRecord, WorldSnapshot};

/// File-based snapshot storage with zstd compression.
pub struct SnapshotStorage {
    /// Directory where snapshots are stored.
    directory: PathBuf,
    /// Snapshot configuration.
    config: SnapshotConfig,
    /// In-memory index: tick -> record metadata (lazy-loaded from disk).
    index: HashMap<u64, SnapshotIndexEntry>,
}

/// Index entry for quick lookup without loading compressed data.
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct SnapshotIndexEntry {
    tick: u64,
    kind: SnapshotKind,
    file_path: PathBuf,
    compressed_size: usize,
    uncompressed_size: usize,
    content_hash: String,
}

impl SnapshotStorage {
    /// Create a new snapshot storage backed by the given directory.
    pub fn new(directory: PathBuf, config: SnapshotConfig) -> Result<Self> {
        fs::create_dir_all(&directory)
            .with_context(|| format!("Failed to create snapshot directory: {:?}", directory))?;

        let mut storage = Self {
            directory,
            config,
            index: HashMap::new(),
        };
        storage.rebuild_index()?;
        Ok(storage)
    }

    /// Rebuild the in-memory index from files on disk.
    ///
    /// Public so that external code (e.g., a query handle) can refresh its
    /// view of the directory after the engine task has written new snapshots.
    pub fn rebuild_index_public(&mut self) -> Result<()> {
        self.rebuild_index()
    }

    /// Rebuild the in-memory index from files on disk.
    fn rebuild_index(&mut self) -> Result<()> {
        self.index.clear();
        let entries = fs::read_dir(&self.directory)
            .with_context(|| format!("Failed to read snapshot directory: {:?}", self.directory))?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("snap") {
                continue;
            }

            // Parse filename: tick_N_full.snap or tick_N_delta.snap
            // Format: "tick_<N>_<kind>.snap"
            let filename = path.file_stem().and_then(|f| f.to_str()).unwrap_or("");
            let (tick_str, kind_str) = match filename.strip_prefix("tick_") {
                Some(rest) => match rest.split_once('_') {
                    Some((t, k)) => (t, k),
                    None => continue,
                },
                None => continue,
            };

            let tick: u64 = match tick_str.parse() {
                Ok(t) => t,
                Err(_) => continue,
            };

            let kind = match kind_str {
                "full" => SnapshotKind::Full,
                "delta" => SnapshotKind::Delta,
                _ => continue,
            };

            let metadata = fs::metadata(&path)?;
            let compressed_size = metadata.len() as usize;

            // Load the record to get uncompressed_size and content_hash
            if let Ok(record) = self.load_record_from_disk(&path) {
                self.index.insert(tick, SnapshotIndexEntry {
                    tick,
                    kind,
                    file_path: path,
                    compressed_size,
                    uncompressed_size: record.uncompressed_size,
                    content_hash: record.content_hash,
                });
            }
        }

        Ok(())
    }

    /// Store a full snapshot.
    pub fn save_full(&mut self, snapshot: &WorldSnapshot) -> Result<SnapshotRecord> {
        let json = serde_json::to_vec(snapshot)
            .context("Failed to serialize full snapshot")?;
        let uncompressed_size = json.len();

        let compressed_data = zstd::encode_all(&json[..], self.config.compression_level)
            .context("Failed to compress snapshot with zstd")?;
        let compressed_size = compressed_data.len();

        let record = SnapshotRecord {
            id: Uuid::new_v4(),
            tick: snapshot.tick,
            kind: SnapshotKind::Full,
            compressed_data: compressed_data.clone(),
            uncompressed_size,
            compressed_size,
            timestamp: snapshot.timestamp,
            content_hash: snapshot.content_hash.clone(),
        };

        let filename = format!("tick_{}_full.snap", snapshot.tick);
        let path = self.directory.join(&filename);

        // If there's already a delta at this tick, remove it — full takes precedence
        if let Some(existing) = self.index.get(&snapshot.tick) {
            if existing.kind == SnapshotKind::Delta {
                let _ = fs::remove_file(&existing.file_path);
            }
        }

        let record_json = serde_json::to_vec(&record)
            .context("Failed to serialize snapshot record")?;
        fs::write(&path, &record_json)
            .with_context(|| format!("Failed to write snapshot to {:?}", path))?;

        // Update index
        self.index.insert(snapshot.tick, SnapshotIndexEntry {
            tick: snapshot.tick,
            kind: SnapshotKind::Full,
            file_path: path,
            compressed_size,
            uncompressed_size,
            content_hash: snapshot.content_hash.clone(),
        });

        self.prune_if_needed()?;

        Ok(record)
    }

    /// Store an incremental delta.
    ///
    /// If a full snapshot already exists at this tick, the delta is NOT saved —
    /// the full snapshot is preserved to maintain delta chain integrity.
    pub fn save_delta(&mut self, delta: &SnapshotDelta) -> Result<SnapshotRecord> {
        // P0-2 fix: Never overwrite or delete an existing full snapshot.
        // A full snapshot at the same tick is strictly better than a delta.
        if let Some(existing) = self.index.get(&delta.tick) {
            if existing.kind == SnapshotKind::Full {
                // Full snapshot already exists at this tick — skip delta,
                // return the existing record so the caller doesn't error.
                let record = self.load_record_from_disk(&existing.file_path)?;
                return Ok(record);
            }
        }

        let json = serde_json::to_vec(delta)
            .context("Failed to serialize snapshot delta")?;
        let uncompressed_size = json.len();

        let compressed_data = zstd::encode_all(&json[..], self.config.compression_level)
            .context("Failed to compress delta with zstd")?;
        let compressed_size = compressed_data.len();

        let record = SnapshotRecord {
            id: Uuid::new_v4(),
            tick: delta.tick,
            kind: SnapshotKind::Delta,
            compressed_data: compressed_data.clone(),
            uncompressed_size,
            compressed_size,
            timestamp: delta.timestamp,
            content_hash: delta.content_hash.clone(),
        };

        let filename = format!("tick_{}_delta.snap", delta.tick);
        let path = self.directory.join(&filename);
        let record_json = serde_json::to_vec(&record)
            .context("Failed to serialize delta record")?;
        fs::write(&path, &record_json)
            .with_context(|| format!("Failed to write delta to {:?}", path))?;

        self.index.insert(delta.tick, SnapshotIndexEntry {
            tick: delta.tick,
            kind: SnapshotKind::Delta,
            file_path: path,
            compressed_size,
            uncompressed_size,
            content_hash: delta.content_hash.clone(),
        });

        self.prune_if_needed()?;

        Ok(record)
    }

    /// Load the latest full snapshot.
    pub fn load_latest_full(&self) -> Result<Option<WorldSnapshot>> {
        let latest_tick = self.index.iter()
            .filter(|(_, e)| e.kind == SnapshotKind::Full)
            .map(|(tick, _)| *tick)
            .max();

        match latest_tick {
            Some(tick) => self.load_full_at(tick).map(Some),
            None => Ok(None),
        }
    }

    /// Load a full snapshot at a specific tick.
    pub fn load_full_at(&self, tick: u64) -> Result<WorldSnapshot> {
        let entry = self.index.get(&tick)
            .ok_or_else(|| anyhow::anyhow!("No snapshot at tick {}", tick))?;

        if entry.kind != SnapshotKind::Full {
            anyhow::bail!("Snapshot at tick {} is not a full snapshot", tick);
        }

        let record = self.load_record_from_disk(&entry.file_path)?;
        let json = zstd::decode_all(&record.compressed_data[..])
            .context("Failed to decompress snapshot")?;
        let snapshot: WorldSnapshot = serde_json::from_slice(&json)
            .context("Failed to deserialize snapshot")?;
        Ok(snapshot)
    }

    /// Reconstruct the full world state at a given tick by replaying deltas.
    ///
    /// Finds the nearest full snapshot before `tick`, then applies deltas
    /// forward to reconstruct the state at `tick`.
    pub fn reconstruct_at(&self, tick: u64) -> Result<WorldSnapshot> {
        // Find the latest full snapshot at or before the target tick
        let base_tick = self.index.iter()
            .filter(|(t, e)| **t <= tick && e.kind == SnapshotKind::Full)
            .map(|(t, _)| *t)
            .max()
            .ok_or_else(|| anyhow::anyhow!("No full snapshot found at or before tick {}", tick))?;

        let mut snapshot = self.load_full_at(base_tick)?;

        if base_tick == tick {
            return Ok(snapshot);
        }

        // Apply deltas from base_tick+1 to tick
        for t in (base_tick + 1)..=tick {
            if let Some(entry) = self.index.get(&t) {
                if entry.kind == SnapshotKind::Delta {
                    let record = self.load_record_from_disk(&entry.file_path)?;
                    let json = zstd::decode_all(&record.compressed_data[..])?;
                    let delta: SnapshotDelta = serde_json::from_slice(&json)?;
                    snapshot = delta.apply(&snapshot);
                    snapshot.tick = t;
                }
            }
            // If no snapshot at this tick, the state is unchanged
        }

        Ok(snapshot)
    }

    /// List all snapshot ticks in order.
    pub fn list_ticks(&self) -> Vec<u64> {
        let mut ticks: Vec<u64> = self.index.keys().copied().collect();
        ticks.sort();
        ticks
    }

    /// Get the number of stored snapshots.
    pub fn count(&self) -> usize {
        self.index.len()
    }

    /// Get compression statistics.
    pub fn compression_stats(&self) -> CompressionStats {
        let mut stats = CompressionStats::default();
        for entry in self.index.values() {
            stats.total_uncompressed += entry.uncompressed_size;
            stats.total_compressed += entry.compressed_size;
            stats.snapshot_count += 1;
        }
        if stats.total_uncompressed > 0 {
            stats.ratio = 1.0 - (stats.total_compressed as f64 / stats.total_uncompressed as f64);
        }
        stats
    }

    /// Load a snapshot record from disk.
    fn load_record_from_disk(&self, path: &Path) -> Result<SnapshotRecord> {
        let data = fs::read(path)
            .with_context(|| format!("Failed to read snapshot file: {:?}", path))?;
        let record: SnapshotRecord = serde_json::from_slice(&data)
            .with_context(|| format!("Failed to parse snapshot file: {:?}", path))?;
        Ok(record)
    }

    /// Prune old snapshots if we exceed max_snapshots.
    ///
    /// Pruning operates on **groups**: a full snapshot plus all its subsequent
    /// deltas (up to the next full snapshot) form an indivisible group. We
    /// only prune entire groups, starting from the oldest. This guarantees
    /// that any remaining delta can always be reconstructed from its base
    /// full snapshot.
    fn prune_if_needed(&mut self) -> Result<()> {
        if self.config.max_snapshots == 0 {
            return Ok(());
        }

        let ticks = self.list_ticks();
        if ticks.len() <= self.config.max_snapshots {
            return Ok(());
        }

        // Build groups: each group starts with a full snapshot and includes
        // all subsequent deltas until the next full snapshot.
        // Groups: [(full_tick, [tick1, tick2, ...]), ...]
        let mut groups: Vec<(u64, Vec<u64>)> = Vec::new();
        let mut current_group: Option<(u64, Vec<u64>)> = None;

        for tick in &ticks {
            if let Some(entry) = self.index.get(tick) {
                if entry.kind == SnapshotKind::Full {
                    // Start a new group
                    if let Some(g) = current_group.take() {
                        groups.push(g);
                    }
                    current_group = Some((*tick, vec![*tick]));
                } else {
                    // Delta — append to current group
                    if let Some((_base, ref mut members)) = current_group {
                        members.push(*tick);
                    }
                    // If no current group, this is an orphan delta — skip it
                    // (shouldn't happen with correct usage)
                }
            }
        }
        if let Some(g) = current_group.take() {
            groups.push(g);
        }

        // We must keep at least the latest group
        if groups.len() <= 1 {
            return Ok(());
        }

        // Prune oldest groups until we're within budget
        let mut total_count = ticks.len();
        let groups_to_remove = groups.len() - 1; // Keep at least the last group

        for group in groups.iter().take(groups_to_remove) {
            if total_count <= self.config.max_snapshots {
                break;
            }

            let (_, group_ticks) = group;
            for tick in group_ticks {
                if let Some(entry) = self.index.remove(tick) {
                    let _ = fs::remove_file(&entry.file_path);
                    total_count -= 1;
                }
            }
        }

        Ok(())
    }
}

/// Compression statistics.
#[derive(Debug, Clone, Default)]
pub struct CompressionStats {
    /// Total uncompressed size in bytes.
    pub total_uncompressed: usize,
    /// Total compressed size in bytes.
    pub total_compressed: usize,
    /// Compression ratio (0.0 to 1.0, higher = better compression).
    pub ratio: f64,
    /// Number of snapshots stored.
    pub snapshot_count: usize,
}

impl CompressionStats {
    /// Format the compression ratio as a percentage.
    pub fn ratio_percent(&self) -> f64 {
        self.ratio * 100.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::enums::AgentPhase;
    use crate::economy::token_burn::AgentRecord;
    use std::collections::HashMap;

    fn make_agent(name: &str, tokens: u64) -> (uuid::Uuid, u64, AgentRecord) {
        (
            uuid::Uuid::new_v4(),
            0,
            AgentRecord {
                id: uuid::Uuid::new_v4(),
                name: name.to_string(),
                phase: AgentPhase::Adult,
                tokens,
                skills: HashMap::new(),
                personality: String::new(),
                tasks_completed: 0,
                tasks_attempted: 0,
            },
        )
    }

    fn make_config() -> SnapshotConfig {
        SnapshotConfig {
            interval_ticks: 10,
            compression_level: 3,
            max_snapshots: 0,
            trigger_event_types: Vec::new(),
        }
    }

    #[test]
    fn storage_save_and_load_full() {
        let dir = tempfile::tempdir().unwrap();
        let config = make_config();
        let mut storage = SnapshotStorage::new(dir.path().to_path_buf(), config).unwrap();

        let agents = vec![make_agent("Alice", 1000)];
        let snapshot = WorldSnapshot::from_world_state(1, &agents);

        storage.save_full(&snapshot).unwrap();

        let loaded = storage.load_full_at(1).unwrap();
        assert_eq!(loaded.tick, 1);
        assert_eq!(loaded.agents.len(), 1);
        assert_eq!(loaded.agents[0].name, "Alice");
        assert!(loaded.verify_hash());
    }

    #[test]
    fn storage_load_latest_full() {
        let dir = tempfile::tempdir().unwrap();
        let config = make_config();
        let mut storage = SnapshotStorage::new(dir.path().to_path_buf(), config).unwrap();

        let agents = vec![make_agent("Alice", 1000)];
        let snap1 = WorldSnapshot::from_world_state(1, &agents);
        let snap2 = WorldSnapshot::from_world_state(2, &agents);

        storage.save_full(&snap1).unwrap();
        storage.save_full(&snap2).unwrap();

        let latest = storage.load_latest_full().unwrap().unwrap();
        assert_eq!(latest.tick, 2);
    }

    #[test]
    fn storage_save_and_load_delta() {
        let dir = tempfile::tempdir().unwrap();
        let config = make_config();
        let mut storage = SnapshotStorage::new(dir.path().to_path_buf(), config).unwrap();

        let agents_t1 = vec![make_agent("Alice", 1000)];
        let snapshot_t1 = WorldSnapshot::from_world_state(1, &agents_t1);
        storage.save_full(&snapshot_t1).unwrap();

        // Create delta: Alice's tokens change
        let mut agents_t2 = agents_t1.clone();
        agents_t2[0].2.tokens = 900;
        let delta = SnapshotDelta::diff(&snapshot_t1, 2, &agents_t2);
        storage.save_delta(&delta).unwrap();

        // Reconstruct at tick 2
        let reconstructed = storage.reconstruct_at(2).unwrap();
        assert_eq!(reconstructed.agents.len(), 1);
        assert_eq!(reconstructed.agents[0].tokens, 900);
    }

    #[test]
    fn storage_compression_ratio() {
        let dir = tempfile::tempdir().unwrap();
        let config = make_config();
        let mut storage = SnapshotStorage::new(dir.path().to_path_buf(), config).unwrap();

        // Create a larger snapshot for meaningful compression
        let agents: Vec<_> = (0..50)
            .map(|i| make_agent(&format!("Agent_{}", i), 1000))
            .collect();
        let snapshot = WorldSnapshot::from_world_state(1, &agents);
        storage.save_full(&snapshot).unwrap();

        let stats = storage.compression_stats();
        assert!(stats.snapshot_count > 0);
        assert!(stats.ratio > 0.0, "Compression ratio should be positive");
        println!(
            "Compression: {} -> {} bytes ({:.1}% reduction)",
            stats.total_uncompressed,
            stats.total_compressed,
            stats.ratio_percent(),
        );
    }

    #[test]
    fn storage_list_ticks() {
        let dir = tempfile::tempdir().unwrap();
        let config = make_config();
        let mut storage = SnapshotStorage::new(dir.path().to_path_buf(), config).unwrap();

        let agents = vec![make_agent("Alice", 1000)];
        for tick in [1u64, 5, 10] {
            let snapshot = WorldSnapshot::from_world_state(tick, &agents);
            storage.save_full(&snapshot).unwrap();
        }

        let ticks = storage.list_ticks();
        assert_eq!(ticks, vec![1, 5, 10]);
    }

    #[test]
    fn storage_prune_old_snapshots_group_based() {
        let dir = tempfile::tempdir().unwrap();
        let config = SnapshotConfig {
            max_snapshots: 4,
            ..make_config()
        };
        let mut storage = SnapshotStorage::new(dir.path().to_path_buf(), config).unwrap();

        let agents = vec![make_agent("Alice", 1000)];

        // Group 1: full at tick 1, deltas at tick 2, 3
        let snap1 = WorldSnapshot::from_world_state(1, &agents);
        storage.save_full(&snap1).unwrap();
        let delta2 = SnapshotDelta::diff(&snap1, 2, &agents);
        storage.save_delta(&delta2).unwrap();
        let delta3 = SnapshotDelta::diff(&snap1, 3, &agents);
        storage.save_delta(&delta3).unwrap();

        // Group 2: full at tick 10
        let snap10 = WorldSnapshot::from_world_state(10, &agents);
        storage.save_full(&snap10).unwrap();

        assert_eq!(storage.count(), 4); // 3 + 1

        // Adding tick 11 should trigger pruning of group 1
        let delta11 = SnapshotDelta::diff(&snap10, 11, &agents);
        storage.save_delta(&delta11).unwrap();

        // Group 1 (ticks 1, 2, 3) should be pruned, leaving group 2 (ticks 10, 11)
        assert_eq!(storage.count(), 2);
        let ticks = storage.list_ticks();
        assert!(ticks.contains(&10), "Latest full snapshot should be kept");
        assert!(ticks.contains(&11), "Latest delta should be kept");
    }

    #[test]
    fn storage_prune_keeps_at_least_one_group() {
        let dir = tempfile::tempdir().unwrap();
        let config = SnapshotConfig {
            max_snapshots: 2,
            ..make_config()
        };
        let mut storage = SnapshotStorage::new(dir.path().to_path_buf(), config).unwrap();

        let agents = vec![make_agent("Alice", 1000)];
        for tick in [1u64, 2, 3] {
            let snapshot = WorldSnapshot::from_world_state(tick, &agents);
            storage.save_full(&snapshot).unwrap();
        }

        // With max_snapshots=2, the oldest full (tick 1) group should be pruned
        assert_eq!(storage.count(), 2);
        let ticks = storage.list_ticks();
        assert!(ticks.contains(&3), "Latest full snapshot should be kept");
    }

    #[test]
    fn save_delta_preserves_existing_full() {
        // P0-2 regression test: saving a delta at a tick that already has a
        // full snapshot must NOT delete the full snapshot.
        let dir = tempfile::tempdir().unwrap();
        let config = make_config();
        let mut storage = SnapshotStorage::new(dir.path().to_path_buf(), config).unwrap();

        let agents = vec![make_agent("Alice", 1000)];
        let snapshot = WorldSnapshot::from_world_state(1, &agents);
        storage.save_full(&snapshot).unwrap();

        // Try to save a delta at the same tick
        let delta = SnapshotDelta::diff(&snapshot, 1, &agents);
        storage.save_delta(&delta).unwrap();

        // The full snapshot should still be there
        let loaded = storage.load_full_at(1).unwrap();
        assert_eq!(loaded.tick, 1);
        assert_eq!(loaded.agents.len(), 1);
    }

    #[test]
    fn filename_parsing() {
        // Verify the fixed filename parser handles "tick_N_full.snap" correctly
        let dir = tempfile::tempdir().unwrap();
        let config = make_config();
        let mut storage = SnapshotStorage::new(dir.path().to_path_buf(), config).unwrap();

        let agents = vec![make_agent("Alice", 1000)];
        let snapshot = WorldSnapshot::from_world_state(42, &agents);
        storage.save_full(&snapshot).unwrap();

        // Rebuild index from disk to verify filename parsing works
        storage.rebuild_index_public().unwrap();
        assert_eq!(storage.count(), 1);
        assert!(storage.load_full_at(42).is_ok());
    }
}
