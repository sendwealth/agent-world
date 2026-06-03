//! Write-Ahead Log (WAL) for crash recovery.
//!
//! Binary record format (all big-endian):
//!   [4 bytes]  Magic number (0x57414C52 = "WALR")
//!   [2 bytes]  Version (uint16, currently 1)
//!   [1 byte]   Record type (0x01 = Event, 0x02 = SnapshotMarker)
//!   [4 bytes]  CRC32 checksum of payload (uint32)
//!   [4 bytes]  Payload length (uint32)
//!   [N bytes]  Payload (JSON-encoded WALEntry)
//!   [1 byte]   Record terminator (0xFF)
//!
//! Recovery flow: load latest snapshot → replay WAL entries after snapshot marker.

mod crc;

use std::fs;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::world::WorldEvent;

// ── Constants ─────────────────────────────────────────────

const MAGIC: u32 = 0x57414C52; // "WALR"
const VERSION: u16 = 1;
const RECORD_EVENT: u8 = 0x01;
const RECORD_SNAPSHOT_MARKER: u8 = 0x02;
const TERMINATOR: u8 = 0xFF;
const HEADER_SIZE: usize = 4 + 2 + 1 + 4 + 4; // 15 bytes
const MAX_ENTRIES_PER_FILE: usize = 1000;

// ── Types ─────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WALEntry {
    pub sequence: u64,
    #[serde(rename = "type")]
    pub entry_type: String, // "event" or "snapshot_marker"
    pub data: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct WALRecoveryResult {
    pub entries: Vec<WALEntry>,
    pub snapshot_file: Option<String>,
    pub corrupted: bool,
    pub last_valid_sequence: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum WALError {
    #[error("WAL is not open")]
    NotOpen,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SnapshotData {
    version: u64,
    timestamp: String,
    event_counter: u64,
    events: Vec<WorldEvent>,
    checksum: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SnapshotMarkerData {
    snapshot_file: String,
    timestamp: String,
}

// ── WAL Implementation ────────────────────────────────────

pub struct WAL {
    file_path: PathBuf,
    writer: Option<BufWriter<fs::File>>,
    entry_count: usize,
    current_sequence: u64,
    // TODO: Use for WAL recovery and compaction operations.
    #[allow(dead_code)]
    data_dir: PathBuf,
    snapshot_dir: PathBuf,
    archive_dir: PathBuf,
}

impl WAL {
    /// Create a new WAL rooted at `data_dir`.
    pub fn new<P: AsRef<Path>>(data_dir: P) -> Self {
        let data_dir = data_dir.as_ref().to_path_buf();
        let snapshot_dir = data_dir.join("snapshots");
        let archive_dir = data_dir.join("wal-archive");
        let file_path = data_dir.join("wal.log");

        // Ensure directories exist
        fs::create_dir_all(&data_dir).ok();
        fs::create_dir_all(&snapshot_dir).ok();
        fs::create_dir_all(&archive_dir).ok();

        Self {
            file_path,
            writer: None,
            entry_count: 0,
            current_sequence: 0,
            data_dir,
            snapshot_dir,
            archive_dir,
        }
    }

    /// Open the WAL for appending.
    pub fn open(&mut self) -> Result<(), WALError> {
        if self.writer.is_some() {
            return Ok(());
        }

        // Count existing entries if file exists
        if self.file_path.exists() {
            let existing = self.read_all();
            self.entry_count = existing.entries.len();
            self.current_sequence = existing.last_valid_sequence;
        }

        // Open file in append mode
        let file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.file_path)?;
        self.writer = Some(BufWriter::new(file));
        Ok(())
    }

    /// Close the WAL.
    pub fn close(&mut self) {
        if let Some(mut w) = self.writer.take() {
            let _ = w.flush();
        }
    }

    /// Append an event to the WAL. Returns the sequence number.
    pub fn append_event(&mut self, event: &WorldEvent) -> Result<u64, WALError> {
        self.current_sequence += 1;
        let entry = WALEntry {
            sequence: self.current_sequence,
            entry_type: "event".into(),
            data: serde_json::to_value(event)?,
        };
        self.write_record(RECORD_EVENT, &entry)?;
        self.entry_count += 1;
        self.check_rotation()?;
        Ok(self.current_sequence)
    }

    /// Append a snapshot marker to the WAL.
    pub fn append_snapshot_marker(&mut self, snapshot_file: &str) -> Result<u64, WALError> {
        self.current_sequence += 1;
        let marker = SnapshotMarkerData {
            snapshot_file: snapshot_file.into(),
            timestamp: chrono::Utc::now().to_rfc3339(),
        };
        let entry = WALEntry {
            sequence: self.current_sequence,
            entry_type: "snapshot_marker".into(),
            data: serde_json::to_value(&marker)?,
        };
        self.write_record(RECORD_SNAPSHOT_MARKER, &entry)?;
        self.entry_count += 1;
        Ok(self.current_sequence)
    }

    /// Write a binary record to the WAL file.
    fn write_record(&mut self, record_type: u8, entry: &WALEntry) -> Result<(), WALError> {
        let writer = self.writer.as_mut().ok_or(WALError::NotOpen)?;

        let payload = serde_json::to_vec(entry)?;
        let crc = crc::crc32(&payload);

        // Build header: magic(4) + version(2) + type(1) + crc(4) + len(4) = 15 bytes
        let mut header = [0u8; HEADER_SIZE];
        header[0..4].copy_from_slice(&MAGIC.to_be_bytes());
        header[4..6].copy_from_slice(&VERSION.to_be_bytes());
        header[6] = record_type;
        header[7..11].copy_from_slice(&crc.to_be_bytes());
        header[11..15].copy_from_slice(&(payload.len() as u32).to_be_bytes());

        writer.write_all(&header)?;
        writer.write_all(&payload)?;
        writer.write_all(&[TERMINATOR])?;
        writer.flush()?;

        Ok(())
    }

    /// Read all valid entries from the WAL file.
    pub fn read_all(&self) -> WALRecoveryResult {
        let mut result = WALRecoveryResult {
            entries: Vec::new(),
            snapshot_file: None,
            corrupted: false,
            last_valid_sequence: 0,
        };

        if !self.file_path.exists() {
            return result;
        }

        let buf = match fs::read(&self.file_path) {
            Ok(b) => b,
            Err(_) => return result,
        };

        let mut offset = 0;
        while offset < buf.len() {
            // Need at least header size
            if offset + HEADER_SIZE > buf.len() {
                result.corrupted = true;
                break;
            }

            // Read header
            let magic = u32::from_be_bytes(buf[offset..offset + 4].try_into().unwrap());
            if magic != MAGIC {
                result.corrupted = true;
                offset += 1;
                continue;
            }

            let version = u16::from_be_bytes(buf[offset + 4..offset + 6].try_into().unwrap());
            if version != VERSION {
                result.corrupted = true;
                offset += HEADER_SIZE;
                continue;
            }

            let stored_crc = u32::from_be_bytes(buf[offset + 7..offset + 11].try_into().unwrap());
            let payload_len =
                u32::from_be_bytes(buf[offset + 11..offset + 15].try_into().unwrap()) as usize;

            // Check full record available
            if offset + HEADER_SIZE + payload_len + 1 > buf.len() {
                result.corrupted = true;
                break;
            }

            // Check terminator
            if buf[offset + HEADER_SIZE + payload_len] != TERMINATOR {
                result.corrupted = true;
                offset += HEADER_SIZE + payload_len + 1;
                continue;
            }

            // Verify CRC
            let payload = &buf[offset + HEADER_SIZE..offset + HEADER_SIZE + payload_len];
            let computed_crc = crc::crc32(payload);
            if computed_crc != stored_crc {
                result.corrupted = true;
                offset += HEADER_SIZE + payload_len + 1;
                continue;
            }

            // Parse entry
            match serde_json::from_slice::<WALEntry>(payload) {
                Ok(entry) => {
                    if entry.sequence > result.last_valid_sequence {
                        result.last_valid_sequence = entry.sequence;
                    }

                    // Track latest snapshot marker
                    if entry.entry_type == "snapshot_marker" {
                        if let Ok(marker) =
                            serde_json::from_value::<SnapshotMarkerData>(entry.data.clone())
                        {
                            result.snapshot_file = Some(marker.snapshot_file);
                        }
                    }

                    result.entries.push(entry);
                }
                Err(_) => {
                    result.corrupted = true;
                }
            }

            offset += HEADER_SIZE + payload_len + 1;
        }

        result
    }

    /// Check if WAL rotation is needed.
    fn check_rotation(&mut self) -> Result<(), WALError> {
        if self.entry_count < MAX_ENTRIES_PER_FILE {
            return Ok(());
        }

        // Close current writer
        self.close();

        // Archive the current WAL file
        let timestamp = chrono::Utc::now()
            .to_rfc3339_opts(chrono::SecondsFormat::Millis, false)
            .replace([':', '.'], "-");
        let archive_path = self.archive_dir.join(format!("wal-{}.log", timestamp));

        if self.file_path.exists() {
            fs::rename(&self.file_path, &archive_path)?;
        }

        // Reset counters
        self.entry_count = 0;

        // Open new WAL file
        self.open()
    }

    /// Take a snapshot of events and write to disk.
    pub fn take_snapshot(
        &mut self,
        events: &[WorldEvent],
        event_counter: u64,
    ) -> Result<String, WALError> {
        let timestamp = chrono::Utc::now()
            .to_rfc3339_opts(chrono::SecondsFormat::Millis, false)
            .replace([':', '.'], "-");
        let snapshot_file = format!("snapshot-{}.json", timestamp);
        let snapshot_path = self.snapshot_dir.join(&snapshot_file);

        let checksum = compute_state_checksum(events, event_counter);
        let snapshot = SnapshotData {
            version: 1,
            timestamp: chrono::Utc::now().to_rfc3339(),
            event_counter,
            events: events.to_vec(),
            checksum,
        };

        // Atomic write: temp file + rename
        let temp_path = snapshot_path.with_extension("json.tmp");
        let json = serde_json::to_string_pretty(&snapshot)?;
        fs::write(&temp_path, &json)?;
        fs::rename(&temp_path, &snapshot_path)?;

        // Record snapshot marker in WAL
        if self.writer.is_some() {
            self.append_snapshot_marker(&snapshot_file)?;
        }

        // Clean up old snapshots (keep latest 3)
        self.cleanup_old_snapshots(3);

        Ok(snapshot_file)
    }

    /// Load a snapshot from disk.
    pub fn load_snapshot(
        &self,
        snapshot_file: &str,
    ) -> Result<Option<(Vec<WorldEvent>, u64, String)>, WALError> {
        let path = self.snapshot_dir.join(snapshot_file);
        if !path.exists() {
            return Ok(None);
        }

        let raw = fs::read_to_string(&path)?;
        let snapshot: SnapshotData = serde_json::from_str(&raw)?;

        // Verify checksum
        let computed = compute_state_checksum(&snapshot.events, snapshot.event_counter);
        if computed != snapshot.checksum {
            tracing::error!("[WAL] Snapshot checksum mismatch: {}", snapshot_file);
            return Ok(None);
        }

        Ok(Some((
            snapshot.events,
            snapshot.event_counter,
            snapshot.checksum,
        )))
    }

    /// Find the latest snapshot file.
    pub fn find_latest_snapshot(&self) -> Option<String> {
        if !self.snapshot_dir.exists() {
            return None;
        }

        let mut files: Vec<_> = fs::read_dir(&self.snapshot_dir)
            .ok()?
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_str()
                    .map(|n| n.starts_with("snapshot-") && n.ends_with(".json"))
                    .unwrap_or(false)
            })
            .collect();

        files.sort_by_key(|e| e.file_name());
        files
            .last()
            .map(|e| e.file_name().to_str().unwrap().to_string())
    }

    /// Full recovery: load latest snapshot → replay WAL.
    pub fn recover(&mut self) -> Result<RecoveryResult, WALError> {
        let mut events: Vec<WorldEvent> = Vec::new();
        let mut event_counter: u64 = 0;
        let mut recovered_from_snapshot = false;

        // Step 1: Load latest snapshot
        if let Some(latest) = self.find_latest_snapshot() {
            if let Ok(Some((snap_events, snap_counter, _))) = self.load_snapshot(&latest) {
                events = snap_events;
                event_counter = snap_counter;
                recovered_from_snapshot = true;
                tracing::info!(
                    "[WAL] Loaded snapshot: {} ({} events, counter: {})",
                    latest,
                    events.len(),
                    event_counter
                );
            }
        }

        // Step 2: Read and replay WAL entries
        let wal_result = self.read_all();

        let mut should_replay = !recovered_from_snapshot;
        let mut wal_entries_replayed: usize = 0;

        for entry in &wal_result.entries {
            if entry.entry_type == "snapshot_marker" {
                if let Ok(_marker) =
                    serde_json::from_value::<SnapshotMarkerData>(entry.data.clone())
                {
                    if recovered_from_snapshot {
                        // Start replaying after the matching snapshot marker
                        should_replay = true;
                    }
                }
                continue;
            }

            if !should_replay {
                continue;
            }

            if entry.entry_type == "event" {
                if let Ok(event) = serde_json::from_value::<WorldEvent>(entry.data.clone()) {
                    events.push(event);
                    event_counter += 1;
                    wal_entries_replayed += 1;
                }
            }
        }

        self.current_sequence = wal_result.last_valid_sequence;

        Ok(RecoveryResult {
            events,
            event_counter,
            recovered_from_snapshot,
            wal_entries_replayed,
            corrupted_records: wal_result.corrupted,
        })
    }

    /// Get current WAL stats.
    pub fn stats(&self) -> WALStats {
        let snapshot_count = if self.snapshot_dir.exists() {
            fs::read_dir(&self.snapshot_dir)
                .map(|rd| {
                    rd.filter_map(|e| e.ok())
                        .filter(|e| {
                            e.file_name()
                                .to_str()
                                .map(|n| n.starts_with("snapshot-") && n.ends_with(".json"))
                                .unwrap_or(false)
                        })
                        .count()
                })
                .unwrap_or(0)
        } else {
            0
        };

        let archive_count = if self.archive_dir.exists() {
            fs::read_dir(&self.archive_dir)
                .map(|rd| {
                    rd.filter_map(|e| e.ok())
                        .filter(|e| {
                            e.file_name()
                                .to_str()
                                .map(|n| n.starts_with("wal-") && n.ends_with(".log"))
                                .unwrap_or(false)
                        })
                        .count()
                })
                .unwrap_or(0)
        } else {
            0
        };

        WALStats {
            entry_count: self.entry_count,
            current_sequence: self.current_sequence,
            file_path: self.file_path.to_string_lossy().into(),
            snapshot_count,
            archive_count,
        }
    }

    /// Verify data consistency.
    pub fn verify_consistency(
        &self,
        events: &[WorldEvent],
        event_counter: u64,
        expected_checksum: &str,
    ) -> bool {
        let computed = compute_state_checksum(events, event_counter);
        computed == expected_checksum
    }

    fn cleanup_old_snapshots(&self, keep_count: usize) {
        if !self.snapshot_dir.exists() {
            return;
        }

        let mut files: Vec<_> = fs::read_dir(&self.snapshot_dir)
            .ok()
            .map(|rd| {
                rd.filter_map(|e| e.ok())
                    .filter(|e| {
                        e.file_name()
                            .to_str()
                            .map(|n| n.starts_with("snapshot-") && n.ends_with(".json"))
                            .unwrap_or(false)
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        files.sort_by_key(|e| e.file_name());
        files.reverse();

        for entry in files.iter().skip(keep_count) {
            let _ = fs::remove_file(entry.path());
        }
    }
}

// ── Result Types ──────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct RecoveryResult {
    pub events: Vec<WorldEvent>,
    pub event_counter: u64,
    pub recovered_from_snapshot: bool,
    pub wal_entries_replayed: usize,
    pub corrupted_records: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct WALStats {
    pub entry_count: usize,
    pub current_sequence: u64,
    pub file_path: String,
    pub snapshot_count: usize,
    pub archive_count: usize,
}

// ── Helpers ───────────────────────────────────────────────

fn compute_state_checksum(events: &[WorldEvent], event_counter: u64) -> String {
    let mut hasher = Sha256::new();
    hasher.update(event_counter.to_le_bytes());
    for event in events {
        hasher.update(event.to_json().as_bytes());
    }
    format!("{:x}", hasher.finalize())
}
