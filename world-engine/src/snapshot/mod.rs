//! World state snapshot system.
//!
//! Provides periodic snapshotting of the complete world state with:
//! - Non-blocking snapshot creation (background task)
//! - Incremental storage (delta between full snapshots)
//! - Zstd compression
//! - Configurable interval and event-based triggers
//!
//! # Architecture
//!
//! ```text
//! Tick Loop ──[channel]──> SnapshotEngine ──> SnapshotStorage ──> Disk
//!                              │
//!                    full / delta decision
//! ```
//!
//! # Usage
//!
//! ```ignore
//! // Ignored: async API requiring tokio runtime + filesystem path.
//! let config = SnapshotConfig::default();
//! let (handle, storage) = SnapshotEngine::spawn(config, path, 256)?;
//!
//! // In the tick loop:
//! handle.notify_tick(tick, agents, events).await;
//!
//! // Query snapshots:
//! let snapshot = storage.lock().await.load_latest_full()?;
//! ```

pub mod snapshot_engine;
pub mod storage;
pub mod types;

pub use snapshot_engine::{SnapshotEngine, SnapshotEngineHandle, SnapshotRequest};
pub use storage::{CompressionStats, SnapshotStorage};
pub use types::{
    AgentSnapshot, SnapshotConfig, SnapshotDelta, SnapshotKind, SnapshotRecord, WorldSnapshot,
};
