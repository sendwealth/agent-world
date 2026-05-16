//! Integration tests for WAL + crash recovery.

use std::fs;

use agent_world_engine::wal::WAL;
use agent_world_engine::world::WorldEvent;
use agent_world_engine::world::enums::{Currency, DeathReason};

use tempfile::TempDir;

fn create_test_event(tick: u64) -> WorldEvent {
    WorldEvent::TickAdvanced { tick }
}

fn create_agent_event(agent_id: &str, name: &str) -> WorldEvent {
    WorldEvent::AgentSpawned {
        agent_id: agent_id.to_string(),
        name: name.to_string(),
    }
}

// ── Basic Operations ──────────────────────────────────────

#[test]
fn wal_create_and_open() {
    let dir = TempDir::new().unwrap();
    let mut wal = WAL::new(dir.path());
    wal.open().unwrap();

    assert!(dir.path().join("wal.log").exists());
    wal.close();
}

#[test]
fn wal_append_and_read_events() {
    let dir = TempDir::new().unwrap();
    let mut wal = WAL::new(dir.path());
    wal.open().unwrap();

    wal.append_event(&create_test_event(1)).unwrap();
    wal.append_event(&create_test_event(2)).unwrap();
    wal.close();

    // Read back
    let wal2 = WAL::new(dir.path());
    let result = wal2.read_all();
    assert_eq!(result.entries.len(), 2);
    assert_eq!(result.entries[0].entry_type, "event");
    assert!(!result.corrupted);
}

#[test]
fn wal_sequence_numbers() {
    let dir = TempDir::new().unwrap();
    let mut wal = WAL::new(dir.path());
    wal.open().unwrap();

    let seq1 = wal.append_event(&create_test_event(1)).unwrap();
    let seq2 = wal.append_event(&create_test_event(2)).unwrap();
    let seq3 = wal.append_event(&create_test_event(3)).unwrap();

    assert_eq!(seq1, 1);
    assert_eq!(seq2, 2);
    assert_eq!(seq3, 3);
    wal.close();
}

#[test]
fn wal_empty_file() {
    let dir = TempDir::new().unwrap();
    let wal = WAL::new(dir.path());
    let result = wal.read_all();
    assert_eq!(result.entries.len(), 0);
    assert!(!result.corrupted);
    assert_eq!(result.last_valid_sequence, 0);
}

// ── Binary Integrity ──────────────────────────────────────

#[test]
fn wal_detect_truncated_header() {
    let dir = TempDir::new().unwrap();
    let mut wal = WAL::new(dir.path());
    wal.open().unwrap();
    wal.append_event(&create_test_event(1)).unwrap();
    wal.close();

    // Truncate the file
    let path = dir.path().join("wal.log");
    let buf = fs::read(&path).unwrap();
    fs::write(&path, &buf[..buf.len() / 2]).unwrap();

    let wal2 = WAL::new(dir.path());
    let result = wal2.read_all();
    assert!(result.corrupted);
}

#[test]
fn wal_detect_corrupted_payload() {
    let dir = TempDir::new().unwrap();
    let mut wal = WAL::new(dir.path());
    wal.open().unwrap();
    wal.append_event(&create_test_event(1)).unwrap();
    wal.close();

    // Flip a byte in the payload area
    let path = dir.path().join("wal.log");
    let mut buf = fs::read(&path).unwrap();
    if buf.len() > 20 {
        buf[20] ^= 0xFF;
    }
    fs::write(&path, &buf).unwrap();

    let wal2 = WAL::new(dir.path());
    let result = wal2.read_all();
    assert!(result.corrupted);
}

// ── Snapshots ─────────────────────────────────────────────

#[test]
fn wal_take_and_load_snapshot() {
    let dir = TempDir::new().unwrap();
    let mut wal = WAL::new(dir.path());
    wal.open().unwrap();

    let events = vec![create_test_event(1), create_test_event(2), create_test_event(3)];
    let snapshot_file = wal.take_snapshot(&events, 3).unwrap();
    wal.close();

    let wal2 = WAL::new(dir.path());
    let result = wal2.load_snapshot(&snapshot_file).unwrap().unwrap();
    assert_eq!(result.0.len(), 3); // events
    assert_eq!(result.1, 3); // event_counter
}

#[test]
fn wal_detect_corrupted_snapshot() {
    let dir = TempDir::new().unwrap();
    let mut wal = WAL::new(dir.path());
    wal.open().unwrap();

    let events = vec![create_test_event(1), create_test_event(2)];
    let snapshot_file = wal.take_snapshot(&events, 2).unwrap();
    wal.close();

    // Tamper with the snapshot
    let path = dir.path().join("snapshots").join(&snapshot_file);
    let raw = fs::read_to_string(&path).unwrap();
    let mut snapshot: serde_json::Value = serde_json::from_str(&raw).unwrap();
    // Add a fake event to break checksum
    snapshot["events"].as_array_mut().unwrap().push(serde_json::json!({"type": "tick_advanced", "payload": {"tick": 999}}));
    fs::write(&path, serde_json::to_string_pretty(&snapshot).unwrap()).unwrap();

    let wal2 = WAL::new(dir.path());
    let result = wal2.load_snapshot(&snapshot_file).unwrap();
    assert!(result.is_none()); // Checksum mismatch
}

#[test]
fn wal_find_latest_snapshot() {
    let dir = TempDir::new().unwrap();
    let mut wal = WAL::new(dir.path());
    wal.open().unwrap();

    wal.take_snapshot(&[create_test_event(1)], 1).unwrap();
    wal.take_snapshot(&[create_test_event(1), create_test_event(2)], 2).unwrap();
    wal.close();

    let wal2 = WAL::new(dir.path());
    let latest = wal2.find_latest_snapshot().unwrap();
    let result = wal2.load_snapshot(&latest).unwrap().unwrap();
    assert_eq!(result.0.len(), 2);
}

#[test]
fn wal_cleanup_old_snapshots() {
    let dir = TempDir::new().unwrap();
    let mut wal = WAL::new(dir.path());
    wal.open().unwrap();

    // Take 5 snapshots
    for i in 0..5 {
        wal.take_snapshot(&[create_test_event(i as u64)], i + 1).unwrap();
    }
    wal.close();

    let snapshot_files: Vec<_> = fs::read_dir(dir.path().join("snapshots"))
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();
    assert!(snapshot_files.len() <= 3); // Should keep only 3
}

// ── Full Recovery ─────────────────────────────────────────

#[test]
fn wal_recover_events_only() {
    let dir = TempDir::new().unwrap();
    let mut wal = WAL::new(dir.path());
    wal.open().unwrap();
    wal.append_event(&create_test_event(1)).unwrap();
    wal.append_event(&create_test_event(2)).unwrap();
    wal.append_event(&create_test_event(3)).unwrap();
    wal.close();

    let mut wal2 = WAL::new(dir.path());
    let result = wal2.recover().unwrap();
    assert_eq!(result.events.len(), 3);
    assert_eq!(result.event_counter, 3);
    assert!(!result.recovered_from_snapshot);
    assert_eq!(result.wal_entries_replayed, 3);
    assert!(!result.corrupted_records);
}

#[test]
fn wal_recover_snapshot_plus_wal() {
    let dir = TempDir::new().unwrap();
    let mut wal = WAL::new(dir.path());
    wal.open().unwrap();

    // Emit 3 events and take snapshot
    wal.append_event(&create_test_event(1)).unwrap();
    wal.append_event(&create_test_event(2)).unwrap();
    wal.append_event(&create_test_event(3)).unwrap();
    wal.take_snapshot(
        &[create_test_event(1), create_test_event(2), create_test_event(3)],
        3,
    )
    .unwrap();

    // Emit 2 more events after snapshot
    wal.append_event(&create_test_event(4)).unwrap();
    wal.append_event(&create_test_event(5)).unwrap();
    wal.close();

    // Recover
    let mut wal2 = WAL::new(dir.path());
    let result = wal2.recover().unwrap();
    assert!(result.recovered_from_snapshot);
    assert_eq!(result.wal_entries_replayed, 2);
    assert_eq!(result.events.len(), 5);
}

#[test]
fn wal_simulate_crash_and_recover() {
    let dir = TempDir::new().unwrap();

    // Phase 1: Start, emit events, simulate crash (no graceful shutdown)
    let mut wal1 = WAL::new(dir.path());
    wal1.open().unwrap();
    wal1.append_event(&create_agent_event("agent-001", "Alice")).unwrap();
    wal1.append_event(&create_agent_event("agent-002", "Bob")).unwrap();
    wal1.append_event(&WorldEvent::TransactionCompleted {
        from: "agent-001".into(),
        to: "agent-002".into(),
        amount: 100,
        currency: Currency::Token,
    })
    .unwrap();
    // Simulate crash: just drop without close/snapshot
    drop(wal1);

    // Phase 2: Restart and recover
    let mut wal2 = WAL::new(dir.path());
    let result = wal2.recover().unwrap();
    assert_eq!(result.events.len(), 3);
    assert_eq!(result.event_counter, 3);
    assert!(!result.corrupted_records);
}

#[test]
fn wal_recover_after_snapshot_and_crash() {
    let dir = TempDir::new().unwrap();

    // Phase 1: Start, emit events, take snapshot, emit more, crash
    let mut wal1 = WAL::new(dir.path());
    wal1.open().unwrap();
    for i in 1..=5 {
        wal1.append_event(&create_test_event(i)).unwrap();
    }
    wal1.take_snapshot(
        &[
            create_test_event(1),
            create_test_event(2),
            create_test_event(3),
            create_test_event(4),
            create_test_event(5),
        ],
        5,
    )
    .unwrap();
    wal1.append_event(&create_test_event(6)).unwrap();
    wal1.append_event(&create_test_event(7)).unwrap();
    // Simulate crash
    drop(wal1);

    // Phase 2: Recover
    let mut wal2 = WAL::new(dir.path());
    let result = wal2.recover().unwrap();
    assert!(result.recovered_from_snapshot);
    assert_eq!(result.events.len(), 7);
    assert_eq!(result.event_counter, 7);
}

#[test]
fn wal_recover_continues_after_recovery() {
    let dir = TempDir::new().unwrap();

    // Phase 1: First run
    let mut wal1 = WAL::new(dir.path());
    wal1.open().unwrap();
    wal1.append_event(&create_test_event(1)).unwrap();
    wal1.append_event(&create_test_event(2)).unwrap();
    drop(wal1);

    // Phase 2: Recover and add more
    let mut wal2 = WAL::new(dir.path());
    wal2.open().unwrap();
    let _ = wal2.recover().unwrap();
    wal2.append_event(&create_test_event(3)).unwrap();
    wal2.append_event(&create_test_event(4)).unwrap();
    drop(wal2);

    // Phase 3: Second recovery
    let mut wal3 = WAL::new(dir.path());
    let result = wal3.recover().unwrap();
    assert_eq!(result.events.len(), 4);
    assert_eq!(result.event_counter, 4);
}

// ── File Rotation ─────────────────────────────────────────

#[test]
fn wal_rotation_after_1000_entries() {
    let dir = TempDir::new().unwrap();
    let mut wal = WAL::new(dir.path());
    wal.open().unwrap();

    // Write 1001 entries
    for i in 0..1001 {
        wal.append_event(&create_test_event(i)).unwrap();
    }
    wal.close();

    // Should have archived the old WAL
    let archive_dir = dir.path().join("wal-archive");
    let archive_files: Vec<_> = fs::read_dir(&archive_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();
    assert_eq!(archive_files.len(), 1);

    // Current WAL should still exist
    assert!(dir.path().join("wal.log").exists());
}

// ── Data Consistency ──────────────────────────────────────

#[test]
fn wal_verify_consistency() {
    let dir = TempDir::new().unwrap();
    let mut wal = WAL::new(dir.path());
    wal.open().unwrap();

    let events = vec![create_test_event(1), create_test_event(2)];
    let snapshot_file = wal.take_snapshot(&events, 2).unwrap();
    wal.close();

    let wal2 = WAL::new(dir.path());
    let result = wal2.load_snapshot(&snapshot_file).unwrap().unwrap();
    let checksum = result.2;
    assert!(wal2.verify_consistency(&events, 2, &checksum));
}

#[test]
fn wal_detect_inconsistent_data() {
    let dir = TempDir::new().unwrap();
    let mut wal = WAL::new(dir.path());
    wal.open().unwrap();

    let events = vec![create_test_event(1), create_test_event(2)];
    let snapshot_file = wal.take_snapshot(&events, 2).unwrap();
    wal.close();

    let wal2 = WAL::new(dir.path());
    let result = wal2.load_snapshot(&snapshot_file).unwrap().unwrap();
    let checksum = result.2;

    // Tamper with events
    let tampered = vec![create_test_event(1), create_test_event(999)];
    assert!(!wal2.verify_consistency(&tampered, 2, &checksum));
}

// ── WAL Stats ─────────────────────────────────────────────

#[test]
fn wal_stats_report() {
    let dir = TempDir::new().unwrap();
    let mut wal = WAL::new(dir.path());
    wal.open().unwrap();

    wal.append_event(&create_test_event(1)).unwrap();
    wal.append_event(&create_test_event(2)).unwrap();
    wal.take_snapshot(&[create_test_event(1), create_test_event(2)], 2).unwrap();
    wal.close();

    let stats = wal.stats();
    assert_eq!(stats.entry_count, 3); // 2 events + 1 snapshot marker
    assert_eq!(stats.current_sequence, 3);
    assert_eq!(stats.snapshot_count, 1);
    assert_eq!(stats.archive_count, 0);
}

// ── Various Event Types ───────────────────────────────────

#[test]
fn wal_all_event_types_roundtrip() {
    let dir = TempDir::new().unwrap();
    let mut wal = WAL::new(dir.path());
    wal.open().unwrap();

    let events = vec![
        WorldEvent::TickAdvanced { tick: 1 },
        WorldEvent::AgentSpawned { agent_id: "a1".into(), name: "Alice".into() },
        WorldEvent::AgentDying { agent_id: "a1".into(), reason: DeathReason::TokenDepleted, grace_ticks: 10 },
        WorldEvent::AgentDied { agent_id: "a1".into(), reason: DeathReason::TokenDepleted },
        WorldEvent::AgentRescued { agent_id: "a1".into() },
        WorldEvent::TransactionCompleted { from: "a1".into(), to: "a2".into(), amount: 100, currency: Currency::Token },
        WorldEvent::TaskCreated { task_id: "t1".into(), publisher: "a1".into(), reward: 50 },
        WorldEvent::TaskClaimed { task_id: "t1".into(), assignee: "a2".into() },
    ];

    for event in &events {
        wal.append_event(event).unwrap();
    }
    wal.close();

    let mut wal2 = WAL::new(dir.path());
    let result = wal2.recover().unwrap();
    assert_eq!(result.events.len(), 8);

    // Verify each event deserializes correctly
    for (i, original) in events.iter().enumerate() {
        let recovered = &result.events[i];
        let original_json = original.to_json();
        let recovered_json = recovered.to_json();
        assert_eq!(original_json, recovered_json, "Event {} mismatch", i);
    }
}
