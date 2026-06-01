//! Integration tests for ProviderConfigStore (SEN-574).
//!
//! Verification criteria:
//! 1. Data survives open/close (persistence across instances)
//! 2. SQLite api_key column is ciphertext, not plaintext
//! 3. Decryption round-trips correctly
//! 4. Auto-generates encryption key when env var is missing
//! 5. Deleting provider cascades to agent_model_assignments
//! 6. Concurrent r/w safe (SQLite WAL + Mutex)
//! 7. Full CRUD coverage

use std::sync::Arc;
use std::thread;

use agent_world_engine::provider_config::{
    CreateProviderInput, ProviderConfigStore,
    ProviderProtocol, UpdateProviderInput,
};

const TEST_KEY: [u8; 32] = [0xab; 32];

fn make_store() -> ProviderConfigStore {
    ProviderConfigStore::open_in_memory_with_key(TEST_KEY).unwrap()
}

fn make_input(name: &str, key: &str) -> CreateProviderInput {
    CreateProviderInput {
        protocol: ProviderProtocol::OpenaiCompatible,
        base_url: format!("https://{}.example.com", name),
        api_key: key.to_string(),
        api_version: "v1".into(),
        display_name: name.to_string(),
        is_default: false,
    }
}

// ── Criterion 1: Persistence across restarts ────────────

#[test]
fn data_survives_close_and_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("test_providers.db");
    let base_dir = dir.path().to_path_buf();
    let key_file = base_dir.join(".od/encryption_key");

    // Pre-create the encryption key file so both instances use the same key
    std::fs::create_dir_all(base_dir.join(".od")).unwrap();
    std::fs::write(&key_file, hex::encode(TEST_KEY)).unwrap();

    // Write with one instance
    {
        let store = ProviderConfigStore::open(&db_path, &base_dir).unwrap();
        let p = store.create(&make_input("openai", "sk-persist-test-123")).unwrap();
        assert_eq!(p.api_key, "sk-persist-test-123");
    }

    // Read with a fresh instance
    {
        let store = ProviderConfigStore::open(&db_path, &base_dir).unwrap();
        let list = store.list().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].api_key, "sk-persist-test-123");
        assert_eq!(list[0].display_name, "openai");
    }
}

// ── Criterion 2: api_key column is not plaintext ────────

#[test]
fn sqlite_column_is_ciphertext() {
    let store = make_store();
    let secret = "sk-this-should-be-encrypted-xyzzy";
    let p = store.create(&make_input("test", secret)).unwrap();

    // Use rusqlite directly to inspect raw bytes
    let raw = store.get_raw_encrypted_key(&p.id).unwrap().unwrap();
    let sb = secret.as_bytes();

    // The ciphertext must not contain the plaintext as a substring
    assert!(
        !raw.windows(sb.len()).any(|w| w == sb),
        "api_key_encrypted column must not contain plaintext"
    );

    // But decryption should recover the original
    let fetched = store.get(&p.id).unwrap().unwrap();
    assert_eq!(fetched.api_key, secret);
}

// ── Criterion 3: Decryption round-trip ──────────────────

#[test]
fn decrypt_roundtrip_with_unicode() {
    let store = make_store();
    let secret = "sk-unicode-测试-🔐-key";
    let p = store.create(&make_input("unicode", secret)).unwrap();
    let fetched = store.get(&p.id).unwrap().unwrap();
    assert_eq!(fetched.api_key, secret);
}

#[test]
fn decrypt_roundtrip_after_update() {
    let store = make_store();
    let p = store.create(&make_input("t", "old-key")).unwrap();
    store.update(&p.id, &UpdateProviderInput {
        api_key: Some("new-updated-key".into()),
        ..Default::default()
    }).unwrap();
    let fetched = store.get(&p.id).unwrap().unwrap();
    assert_eq!(fetched.api_key, "new-updated-key");
}

// ── Criterion 4: Auto-generate key ──────────────────────

#[test]
fn auto_generates_key_file_when_env_missing() {
    // This test requires AW_ENCRYPTION_KEY to NOT be set.
    // Since cargo test runs tests in parallel, another test might set it.
    // We skip gracefully if it's already set.
    if std::env::var("AW_ENCRYPTION_KEY").is_ok() {
        eprintln!("Skipping auto-generate test: AW_ENCRYPTION_KEY is set");
        return;
    }

    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("auto_key_test.db");
    let key_file = dir.path().join(".od/encryption_key");

    assert!(!key_file.exists(), "key file should not exist yet");

    let store = ProviderConfigStore::open(&db_path, dir.path()).unwrap();
    assert!(key_file.exists(), "key file should have been auto-generated");

    // Should be usable
    let p = store.create(&make_input("auto", "auto-key")).unwrap();
    assert_eq!(store.get(&p.id).unwrap().unwrap().api_key, "auto-key");
}

// ── Criterion 5: Delete cascades to assignments ─────────

#[test]
fn delete_provider_cascades_to_assignments() {
    let store = make_store();
    let p = store.create(&make_input("cascade", "key")).unwrap();

    store.assign_model("agent-x", &p.id, "model-a").unwrap();
    store.assign_model("agent-y", &p.id, "model-b").unwrap();
    assert_eq!(store.list_assignments().unwrap().len(), 2);

    assert!(store.delete(&p.id).unwrap());
    assert!(store.get(&p.id).unwrap().is_none());
    assert_eq!(store.list_assignments().unwrap().len(), 0, "assignments should be cascade-deleted");
}

// ── Criterion 6: Concurrent r/w safety ──────────────────

#[test]
fn concurrent_reads_and_writes() {
    let store = Arc::new(make_store());

    // Seed one provider
    let p = store.create(&make_input("concurrent", "key")).unwrap();
    let provider_id = p.id.clone();

    let mut handles = vec![];

    // Spawn readers
    for _ in 0..4 {
        let s = store.clone();
        let pid = provider_id.clone();
        handles.push(thread::spawn(move || {
            for _ in 0..50 {
                let fetched = s.get(&pid).unwrap().unwrap();
                assert_eq!(fetched.api_key, "key");
            }
        }));
    }

    // Spawn writers (assignments)
    for i in 0..4 {
        let s = store.clone();
        let pid = provider_id.clone();
        handles.push(thread::spawn(move || {
            for j in 0..25 {
                let agent_id = format!("agent-{}-{}", i, j);
                s.assign_model(&agent_id, &pid, "model-x").unwrap();
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    let assignments = store.list_assignments().unwrap();
    assert_eq!(assignments.len(), 100); // 4 threads * 25 agents
}

// ── Criterion 7: Full CRUD coverage ─────────────────────

#[test]
fn full_crud_lifecycle() {
    let store = make_store();

    // CREATE
    let p = store.create(&CreateProviderInput {
        protocol: ProviderProtocol::Anthropic,
        base_url: "https://api.anthropic.com".into(),
        api_key: "sk-ant-123".into(),
        api_version: "2023-06-01".into(),
        display_name: "Anthropic".into(),
        is_default: false,
    }).unwrap();

    // READ
    let fetched = store.get(&p.id).unwrap().unwrap();
    assert_eq!(fetched.protocol, ProviderProtocol::Anthropic);
    assert_eq!(fetched.api_key, "sk-ant-123");
    assert_eq!(fetched.display_name, "Anthropic");

    // UPDATE
    let updated = store.update(&p.id, &UpdateProviderInput {
        api_key: Some("sk-ant-456".into()),
        display_name: Some("Anthropic Updated".into()),
        is_default: Some(true),
        ..Default::default()
    }).unwrap().unwrap();
    assert_eq!(updated.api_key, "sk-ant-456");
    assert_eq!(updated.display_name, "Anthropic Updated");
    assert!(updated.is_default);
    assert_eq!(updated.base_url, "https://api.anthropic.com"); // unchanged

    // LIST
    let all = store.list().unwrap();
    assert_eq!(all.len(), 1);

    // ASSIGN MODEL
    let a = store.assign_model("agent-1", &p.id, "claude-3-opus").unwrap();
    assert_eq!(a.model_id, "claude-3-opus");

    // GET ASSIGNMENT
    let ga = store.get_assignment("agent-1").unwrap().unwrap();
    assert_eq!(ga.provider_id, p.id);

    // LIST ASSIGNMENTS
    assert_eq!(store.list_assignments().unwrap().len(), 1);

    // REMOVE ASSIGNMENT
    assert!(store.remove_assignment("agent-1").unwrap());
    assert!(store.get_assignment("agent-1").unwrap().is_none());

    // DELETE
    assert!(store.delete(&p.id).unwrap());
    assert!(store.get(&p.id).unwrap().is_none());
    assert_eq!(store.list().unwrap().len(), 0);
}

// ── Edge cases ──────────────────────────────────────────

#[test]
fn multiple_providers_different_protocols() {
    let store = make_store();
    let protocols = [
        ProviderProtocol::OpenaiCompatible,
        ProviderProtocol::Anthropic,
        ProviderProtocol::Ollama,
        ProviderProtocol::Gemini,
        ProviderProtocol::Azure,
    ];

    for (i, proto) in protocols.iter().enumerate() {
        store.create(&CreateProviderInput {
            protocol: proto.clone(),
            base_url: format!("https://provider-{}.com", i),
            api_key: format!("key-{}", i),
            api_version: String::new(),
            display_name: format!("Provider {}", i),
            is_default: false,
        }).unwrap();
    }

    let all = store.list().unwrap();
    assert_eq!(all.len(), 5);

    // Verify each protocol round-trips
    for (i, proto) in protocols.iter().enumerate() {
        assert_eq!(all[i].protocol, *proto);
    }
}

#[test]
fn empty_api_key_works() {
    let store = make_store();
    let p = store.create(&CreateProviderInput {
        protocol: ProviderProtocol::Ollama,
        base_url: "http://localhost:11434".into(),
        api_key: String::new(), // Ollama doesn't need a key
        api_version: String::new(),
        display_name: "Local Ollama".into(),
        is_default: false,
    }).unwrap();
    assert_eq!(store.get(&p.id).unwrap().unwrap().api_key, "");
}

#[test]
fn long_api_key_roundtrips() {
    let store = make_store();
    let long_key = "sk-".to_string() + &"a".repeat(2000);
    let p = store.create(&CreateProviderInput {
        protocol: ProviderProtocol::OpenaiCompatible,
        base_url: "https://api.openai.com".into(),
        api_key: long_key.clone(),
        api_version: String::new(),
        display_name: "Long Key".into(),
        is_default: false,
    }).unwrap();
    assert_eq!(store.get(&p.id).unwrap().unwrap().api_key, long_key);
}
