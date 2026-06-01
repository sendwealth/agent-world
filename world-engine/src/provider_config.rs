//! Provider configuration persistence with AES-256-GCM encryption.
//!
//! Stores LLM provider configs (API keys, endpoints) and agent-to-model
//! assignments in SQLite. API keys are encrypted at rest using AES-256-GCM.
//!
//! Key management:
//! - Reads `AW_ENCRYPTION_KEY` env var (hex-encoded 32-byte key)
//! - Falls back to auto-generating a key stored in `.od/encryption_key`

use std::path::Path;
use std::sync::Mutex;

use aes_gcm::aead::{Aead, AeadCore, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Nonce};
use anyhow::{bail, Context};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const NONCE_SIZE: usize = 12;
const KEY_SIZE: usize = 32;
const ENV_KEY: &str = "AW_ENCRYPTION_KEY";
const KEY_FILE: &str = ".od/encryption_key";

/// Supported LLM provider protocols.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderProtocol {
    OpenaiCompatible,
    Anthropic,
    Ollama,
    Gemini,
    Azure,
}

impl ProviderProtocol {
    pub fn as_str(&self) -> &str {
        match self {
            Self::OpenaiCompatible => "openai_compatible",
            Self::Anthropic => "anthropic",
            Self::Ollama => "ollama",
            Self::Gemini => "gemini",
            Self::Azure => "azure",
        }
    }

    pub fn from_str_lossy(s: &str) -> Self {
        match s {
            "anthropic" => Self::Anthropic,
            "ollama" => Self::Ollama,
            "gemini" => Self::Gemini,
            "azure" => Self::Azure,
            _ => Self::OpenaiCompatible,
        }
    }
}

/// A provider configuration record (decrypted).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub id: String,
    pub protocol: ProviderProtocol,
    pub base_url: String,
    pub api_key: String,
    pub api_version: String,
    pub display_name: String,
    pub is_default: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// Input for creating a new provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateProviderInput {
    pub protocol: ProviderProtocol,
    pub base_url: String,
    pub api_key: String,
    #[serde(default)]
    pub api_version: String,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub is_default: bool,
}

/// Input for updating an existing provider.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateProviderInput {
    pub protocol: Option<ProviderProtocol>,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub api_version: Option<String>,
    pub display_name: Option<String>,
    pub is_default: Option<bool>,
}

/// An agent-to-model assignment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentModelAssignment {
    pub agent_id: String,
    pub provider_id: String,
    pub model_id: String,
    pub updated_at: String,
}

// -- Encryption helpers --

fn resolve_encryption_key(base_dir: &Path) -> anyhow::Result<[u8; KEY_SIZE]> {
    // 1. Try env var
    if let Ok(hex_key) = std::env::var(ENV_KEY) {
        let key = hex::decode(&hex_key).context("AW_ENCRYPTION_KEY is not valid hex")?;
        if key.len() != KEY_SIZE {
            bail!(
                "AW_ENCRYPTION_KEY must be {} bytes ({} hex chars), got {} bytes",
                KEY_SIZE, KEY_SIZE * 2, key.len()
            );
        }
        let mut arr = [0u8; KEY_SIZE];
        arr.copy_from_slice(&key);
        return Ok(arr);
    }

    // 2. Try key file
    let key_path = base_dir.join(KEY_FILE);
    if key_path.exists() {
        let hex_key = std::fs::read_to_string(&key_path)
            .context("Failed to read encryption key file")?
            .trim()
            .to_string();
        let key = hex::decode(&hex_key).context("Key file is not valid hex")?;
        if key.len() != KEY_SIZE {
            bail!("Key file must contain {} bytes ({} hex chars)", KEY_SIZE, KEY_SIZE * 2);
        }
        let mut arr = [0u8; KEY_SIZE];
        arr.copy_from_slice(&key);
        return Ok(arr);
    }

    // 3. Auto-generate
    let mut key = [0u8; KEY_SIZE];
    rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut key);
    if let Some(parent) = key_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&key_path, hex::encode(key))
        .context("Failed to write auto-generated encryption key")?;
    tracing::info!("Auto-generated encryption key at {:?}", key_path);
    Ok(key)
}

fn encrypt(key: &[u8; KEY_SIZE], plaintext: &str) -> anyhow::Result<(Vec<u8>, Vec<u8>)> {
    let cipher = Aes256Gcm::new_from_slice(key)?;
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ciphertext = cipher
        .encrypt(&nonce, plaintext.as_bytes())
        .map_err(|e| anyhow::anyhow!("encryption failed: {}", e))?;
    Ok((ciphertext, nonce.to_vec()))
}

fn decrypt(key: &[u8; KEY_SIZE], ciphertext: &[u8], nonce: &[u8]) -> anyhow::Result<String> {
    if nonce.len() != NONCE_SIZE {
        bail!("invalid nonce length: expected {}, got {}", NONCE_SIZE, nonce.len());
    }
    let cipher = Aes256Gcm::new_from_slice(key)?;
    let nonce = Nonce::from_slice(nonce);
    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| anyhow::anyhow!("decryption failed: {}", e))?;
    String::from_utf8(plaintext).context("decrypted bytes are not valid UTF-8")
}

// -- ProviderConfigStore --

/// SQLite-backed store for provider configurations with encrypted API keys.
pub struct ProviderConfigStore {
    conn: Mutex<Connection>,
    enc_key: [u8; KEY_SIZE],
}

impl ProviderConfigStore {
    /// Open (or create) the store at the given SQLite path.
    pub fn open(db_path: &Path, base_dir: &Path) -> anyhow::Result<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(db_path)?;
        conn.execute_batch(crate::persistence::sqlite::SCHEMA_SQL)?;
        conn.execute_batch("PRAGMA journal_mode = WAL;")?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        let enc_key = resolve_encryption_key(base_dir)?;
        Ok(Self { conn: Mutex::new(conn), enc_key })
    }

    /// Open an in-memory store for testing. Generates a temporary key.
    pub fn open_in_memory() -> anyhow::Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch(crate::persistence::sqlite::SCHEMA_SQL)?;
        conn.execute_batch("PRAGMA journal_mode = WAL;")?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        let enc_key = resolve_encryption_key(Path::new("."))?;
        Ok(Self { conn: Mutex::new(conn), enc_key })
    }

    /// Open in-memory with a specific key (deterministic tests).
    pub fn open_in_memory_with_key(enc_key: [u8; KEY_SIZE]) -> anyhow::Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch(crate::persistence::sqlite::SCHEMA_SQL)?;
        conn.execute_batch("PRAGMA journal_mode = WAL;")?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        Ok(Self { conn: Mutex::new(conn), enc_key })
    }

    /// Create a new provider configuration.
    pub fn create(&self, input: &CreateProviderInput) -> anyhow::Result<ProviderConfig> {
        let id = Uuid::new_v4().to_string();
        let (ciphertext, nonce) = encrypt(&self.enc_key, &input.api_key)?;
        let conn = self.conn.lock().map_err(|e| anyhow::anyhow!("lock poisoned: {}", e))?;

        if input.is_default {
            conn.execute("UPDATE provider_configs SET is_default = 0 WHERE is_default = 1", [])?;
        }

        conn.execute(
            "INSERT INTO provider_configs (id, protocol, base_url, api_key_encrypted, api_key_nonce, api_version, display_name, is_default) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![id, input.protocol.as_str(), input.base_url, ciphertext, nonce, input.api_version, input.display_name, input.is_default as i64],
        )?;

        drop(conn);
        self.get(&id).map(|opt| opt.context("failed to read back created provider")).map(|r| r.unwrap())
    }

    /// Get a provider by ID.
    pub fn get(&self, id: &str) -> anyhow::Result<Option<ProviderConfig>> {
        let conn = self.conn.lock().map_err(|e| anyhow::anyhow!("lock poisoned: {}", e))?;
        self.get_inner(&conn, id)
    }

    fn get_inner(&self, conn: &Connection, id: &str) -> anyhow::Result<Option<ProviderConfig>> {
        let result = conn.query_row(
            "SELECT id, protocol, base_url, api_key_encrypted, api_key_nonce, api_version, display_name, is_default, created_at, updated_at FROM provider_configs WHERE id = ?1",
            params![id],
            |row| Ok((
                row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?,
                row.get::<_, Vec<u8>>(3)?, row.get::<_, Vec<u8>>(4)?, row.get::<_, String>(5)?,
                row.get::<_, String>(6)?, row.get::<_, i64>(7)?, row.get::<_, String>(8)?, row.get::<_, String>(9)?,
            )),
        );
        match result {
            Ok((id, proto, url, ct, nonce, aver, disp, isdef, cat, uat)) => {
                let api_key = decrypt(&self.enc_key, &ct, &nonce)?;
                Ok(Some(ProviderConfig {
                    id, protocol: ProviderProtocol::from_str_lossy(&proto),
                    base_url: url, api_key, api_version: aver, display_name: disp,
                    is_default: isdef != 0, created_at: cat, updated_at: uat,
                }))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// List all provider configurations.
    pub fn list(&self) -> anyhow::Result<Vec<ProviderConfig>> {
        let conn = self.conn.lock().map_err(|e| anyhow::anyhow!("lock poisoned: {}", e))?;
        let mut stmt = conn.prepare(
            "SELECT id, protocol, base_url, api_key_encrypted, api_key_nonce, api_version, display_name, is_default, created_at, updated_at FROM provider_configs ORDER BY created_at"
        )?;
        let rows = stmt.query_map([], |row| Ok((
            row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?,
            row.get::<_, Vec<u8>>(3)?, row.get::<_, Vec<u8>>(4)?, row.get::<_, String>(5)?,
            row.get::<_, String>(6)?, row.get::<_, i64>(7)?, row.get::<_, String>(8)?, row.get::<_, String>(9)?,
        )))?;
        let mut configs = Vec::new();
        for row in rows {
            let (id, proto, url, ct, nonce, aver, disp, isdef, cat, uat) = row?;
            let api_key = decrypt(&self.enc_key, &ct, &nonce)?;
            configs.push(ProviderConfig {
                id, protocol: ProviderProtocol::from_str_lossy(&proto),
                base_url: url, api_key, api_version: aver, display_name: disp,
                is_default: isdef != 0, created_at: cat, updated_at: uat,
            });
        }
        Ok(configs)
    }

    /// Update a provider configuration.
    ///
    /// The entire read-modify-write cycle is performed within a single lock
    /// scope to avoid TOCTOU races between concurrent updates.
    pub fn update(&self, id: &str, input: &UpdateProviderInput) -> anyhow::Result<Option<ProviderConfig>> {
        let conn = self.conn.lock().map_err(|e| anyhow::anyhow!("lock poisoned: {}", e))?;

        // Read existing within the same lock scope
        let existing = match self.get_inner(&conn, id)? {
            Some(c) => c,
            None => return Ok(None),
        };

        if input.is_default == Some(true) {
            conn.execute("UPDATE provider_configs SET is_default = 0 WHERE is_default = 1 AND id != ?1", params![id])?;
        }
        let api_key_str = input.api_key.as_ref().unwrap_or(&existing.api_key);
        let (ciphertext, nonce) = encrypt(&self.enc_key, api_key_str)?;
        let protocol = input.protocol.as_ref().map(|p| p.as_str()).unwrap_or(existing.protocol.as_str());
        let base_url = input.base_url.as_ref().unwrap_or(&existing.base_url);
        let api_version = input.api_version.as_ref().unwrap_or(&existing.api_version);
        let display_name = input.display_name.as_ref().unwrap_or(&existing.display_name);
        let is_default = input.is_default.unwrap_or(existing.is_default);
        conn.execute(
            "UPDATE provider_configs SET protocol=?1, base_url=?2, api_key_encrypted=?3, api_key_nonce=?4, api_version=?5, display_name=?6, is_default=?7, updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?8",
            params![protocol, base_url, ciphertext, nonce, api_version, display_name, is_default as i64, id],
        )?;

        // Read back within the same lock — no TOCTOU gap
        self.get_inner(&conn, id)
    }

    /// Delete a provider. Cascades to agent_model_assignments.
    pub fn delete(&self, id: &str) -> anyhow::Result<bool> {
        let conn = self.conn.lock().map_err(|e| anyhow::anyhow!("lock poisoned: {}", e))?;
        let rows = conn.execute("DELETE FROM provider_configs WHERE id = ?1", params![id])?;
        Ok(rows > 0)
    }

    // -- Agent Model Assignments --

    /// Assign a model to an agent.
    pub fn assign_model(&self, agent_id: &str, provider_id: &str, model_id: &str) -> anyhow::Result<AgentModelAssignment> {
        let conn = self.conn.lock().map_err(|e| anyhow::anyhow!("lock poisoned: {}", e))?;
        let exists: bool = conn.query_row(
            "SELECT COUNT(*) > 0 FROM provider_configs WHERE id = ?1", params![provider_id], |row| row.get(0),
        )?;
        if !exists { bail!("provider {} not found", provider_id); }
        conn.execute(
            "INSERT INTO agent_model_assignments (agent_id, provider_id, model_id, updated_at) VALUES (?1, ?2, ?3, strftime('%Y-%m-%dT%H:%M:%fZ','now')) ON CONFLICT(agent_id) DO UPDATE SET provider_id=excluded.provider_id, model_id=excluded.model_id, updated_at=excluded.updated_at",
            params![agent_id, provider_id, model_id],
        )?;
        Ok(AgentModelAssignment {
            agent_id: agent_id.to_string(), provider_id: provider_id.to_string(),
            model_id: model_id.to_string(), updated_at: chrono::Utc::now().to_rfc3339(),
        })
    }

    /// Get the model assignment for a specific agent.
    pub fn get_assignment(&self, agent_id: &str) -> anyhow::Result<Option<AgentModelAssignment>> {
        let conn = self.conn.lock().map_err(|e| anyhow::anyhow!("lock poisoned: {}", e))?;
        let result = conn.query_row(
            "SELECT agent_id, provider_id, model_id, updated_at FROM agent_model_assignments WHERE agent_id = ?1",
            params![agent_id],
            |row| Ok(AgentModelAssignment { agent_id: row.get(0)?, provider_id: row.get(1)?, model_id: row.get(2)?, updated_at: row.get(3)? }),
        );
        match result {
            Ok(a) => Ok(Some(a)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// List all agent model assignments.
    pub fn list_assignments(&self) -> anyhow::Result<Vec<AgentModelAssignment>> {
        let conn = self.conn.lock().map_err(|e| anyhow::anyhow!("lock poisoned: {}", e))?;
        let mut stmt = conn.prepare("SELECT agent_id, provider_id, model_id, updated_at FROM agent_model_assignments ORDER BY agent_id")?;
        let rows = stmt.query_map([], |row| Ok(AgentModelAssignment { agent_id: row.get(0)?, provider_id: row.get(1)?, model_id: row.get(2)?, updated_at: row.get(3)? }))?;
        let mut out = Vec::new();
        for r in rows { out.push(r?); }
        Ok(out)
    }

    /// Remove a model assignment for an agent.
    pub fn remove_assignment(&self, agent_id: &str) -> anyhow::Result<bool> {
        let conn = self.conn.lock().map_err(|e| anyhow::anyhow!("lock poisoned: {}", e))?;
        let rows = conn.execute("DELETE FROM agent_model_assignments WHERE agent_id = ?1", params![agent_id])?;
        Ok(rows > 0)
    }

    /// Get raw encrypted key bytes (test only).
    pub fn get_raw_encrypted_key(&self, id: &str) -> anyhow::Result<Option<Vec<u8>>> {
        let conn = self.conn.lock().unwrap();
        let result = conn.query_row(
            "SELECT api_key_encrypted FROM provider_configs WHERE id = ?1", params![id], |row| row.get::<_, Vec<u8>>(0),
        );
        match result {
            Ok(v) => Ok(Some(v)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_key() -> [u8; KEY_SIZE] { [42u8; KEY_SIZE] }
    fn make_store() -> ProviderConfigStore { ProviderConfigStore::open_in_memory_with_key(make_test_key()).unwrap() }

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let key = make_test_key();
        let (ct, nonce) = encrypt(&key, "sk-12345-secret").unwrap();
        assert_eq!(decrypt(&key, &ct, &nonce).unwrap(), "sk-12345-secret");
    }

    #[test]
    fn encrypt_produces_different_ciphertext() {
        let key = make_test_key();
        let (ct1, _) = encrypt(&key, "same").unwrap();
        let (ct2, _) = encrypt(&key, "same").unwrap();
        assert_ne!(ct1, ct2);
    }

    #[test]
    fn decrypt_wrong_key_fails() {
        let k1 = make_test_key();
        let k2 = [99u8; KEY_SIZE];
        let (ct, nonce) = encrypt(&k1, "secret").unwrap();
        assert!(decrypt(&k2, &ct, &nonce).is_err());
    }

    #[test]
    fn create_and_get_provider() {
        let store = make_store();
        let created = store.create(&CreateProviderInput {
            protocol: ProviderProtocol::OpenaiCompatible,
            base_url: "https://api.openai.com/v1".into(),
            api_key: "sk-test-12345".into(),
            api_version: "2024-01-01".into(),
            display_name: "OpenAI".into(),
            is_default: true,
        }).unwrap();
        assert_eq!(created.api_key, "sk-test-12345");
        assert!(created.is_default);
        let fetched = store.get(&created.id).unwrap().unwrap();
        assert_eq!(fetched.api_key, "sk-test-12345");
    }

    #[test]
    fn list_providers() {
        let store = make_store();
        for i in 0..3 {
            store.create(&CreateProviderInput {
                protocol: ProviderProtocol::Anthropic, base_url: format!("https://api{}.anthropic.com", i),
                api_key: format!("key-{}", i), api_version: String::new(),
                display_name: format!("P{}", i), is_default: false,
            }).unwrap();
        }
        assert_eq!(store.list().unwrap().len(), 3);
    }

    #[test]
    fn update_provider() {
        let store = make_store();
        let c = store.create(&CreateProviderInput {
            protocol: ProviderProtocol::Ollama, base_url: "http://localhost:11434".into(),
            api_key: "no-key".into(), api_version: String::new(),
            display_name: "Ollama".into(), is_default: false,
        }).unwrap();
        let u = store.update(&c.id, &UpdateProviderInput {
            display_name: Some("Updated".into()), api_key: Some("new-key".into()), ..Default::default()
        }).unwrap().unwrap();
        assert_eq!(u.display_name, "Updated");
        assert_eq!(u.api_key, "new-key");
        assert_eq!(u.base_url, "http://localhost:11434");
    }

    #[test]
    fn delete_provider_cascades_assignments() {
        let store = make_store();
        let p = store.create(&CreateProviderInput {
            protocol: ProviderProtocol::OpenaiCompatible, base_url: "https://api.openai.com".into(),
            api_key: "sk-key".into(), api_version: String::new(), display_name: "OAI".into(), is_default: false,
        }).unwrap();
        store.assign_model("a1", &p.id, "gpt-4o").unwrap();
        store.assign_model("a2", &p.id, "gpt-4o-mini").unwrap();
        assert_eq!(store.list_assignments().unwrap().len(), 2);
        assert!(store.delete(&p.id).unwrap());
        assert!(store.get(&p.id).unwrap().is_none());
        assert_eq!(store.list_assignments().unwrap().len(), 0);
    }

    #[test]
    fn get_nonexistent_returns_none() {
        assert!(make_store().get("no-id").unwrap().is_none());
    }

    #[test]
    fn delete_nonexistent_returns_false() {
        assert!(!make_store().delete("no-id").unwrap());
    }

    #[test]
    fn assign_and_get_model() {
        let store = make_store();
        let p = store.create(&CreateProviderInput {
            protocol: ProviderProtocol::Gemini, base_url: "https://genai.googleapis.com".into(),
            api_key: "gemini-key".into(), api_version: "v1".into(), display_name: "Gemini".into(), is_default: false,
        }).unwrap();
        let a = store.assign_model("agent-42", &p.id, "gemini-pro").unwrap();
        assert_eq!(a.model_id, "gemini-pro");
        let fetched = store.get_assignment("agent-42").unwrap().unwrap();
        assert_eq!(fetched.provider_id, p.id);
    }

    #[test]
    fn assign_model_upserts() {
        let store = make_store();
        let p1 = store.create(&CreateProviderInput {
            protocol: ProviderProtocol::OpenaiCompatible, base_url: "https://a.com".into(),
            api_key: "k1".into(), api_version: String::new(), display_name: "P1".into(), is_default: false,
        }).unwrap();
        let p2 = store.create(&CreateProviderInput {
            protocol: ProviderProtocol::Anthropic, base_url: "https://b.com".into(),
            api_key: "k2".into(), api_version: String::new(), display_name: "P2".into(), is_default: false,
        }).unwrap();
        store.assign_model("a1", &p1.id, "gpt-4o").unwrap();
        store.assign_model("a1", &p2.id, "claude-3").unwrap();
        let asgn = store.list_assignments().unwrap();
        assert_eq!(asgn.len(), 1);
        assert_eq!(asgn[0].provider_id, p2.id);
    }

    #[test]
    fn assign_model_nonexistent_provider_fails() {
        assert!(make_store().assign_model("a1", "no-provider", "m1").is_err());
    }

    #[test]
    fn remove_assignment() {
        let store = make_store();
        let p = store.create(&CreateProviderInput {
            protocol: ProviderProtocol::Ollama, base_url: "http://localhost:11434".into(),
            api_key: "none".into(), api_version: String::new(), display_name: "Ol".into(), is_default: false,
        }).unwrap();
        store.assign_model("a1", &p.id, "llama3").unwrap();
        assert!(store.remove_assignment("a1").unwrap());
        assert!(store.get_assignment("a1").unwrap().is_none());
        assert!(!store.remove_assignment("a1").unwrap());
    }

    #[test]
    fn encrypted_key_not_plaintext_in_db() {
        let store = make_store();
        let secret = "sk-super-secret-key-12345";
        let p = store.create(&CreateProviderInput {
            protocol: ProviderProtocol::OpenaiCompatible, base_url: "https://api.openai.com".into(),
            api_key: secret.into(), api_version: String::new(), display_name: "T".into(), is_default: false,
        }).unwrap();
        let enc = store.get_raw_encrypted_key(&p.id).unwrap().unwrap();
        let sb = secret.as_bytes();
        assert!(!enc.windows(sb.len()).any(|w| w == sb), "API key must not be plaintext in DB");
        assert_eq!(store.get(&p.id).unwrap().unwrap().api_key, secret);
    }

    #[test]
    fn only_one_default_provider() {
        let store = make_store();
        let p1 = store.create(&CreateProviderInput {
            protocol: ProviderProtocol::OpenaiCompatible, base_url: "https://a.com".into(),
            api_key: "k1".into(), api_version: String::new(), display_name: "P1".into(), is_default: true,
        }).unwrap();
        let p2 = store.create(&CreateProviderInput {
            protocol: ProviderProtocol::Anthropic, base_url: "https://b.com".into(),
            api_key: "k2".into(), api_version: String::new(), display_name: "P2".into(), is_default: true,
        }).unwrap();
        assert!(!store.get(&p1.id).unwrap().unwrap().is_default);
        assert!(store.get(&p2.id).unwrap().unwrap().is_default);
    }

    #[test]
    fn protocol_roundtrip_all_variants() {
        let store = make_store();
        let protos = vec![
            ProviderProtocol::OpenaiCompatible, ProviderProtocol::Anthropic,
            ProviderProtocol::Ollama, ProviderProtocol::Gemini, ProviderProtocol::Azure,
        ];
        for proto in &protos {
            let c = store.create(&CreateProviderInput {
                protocol: proto.clone(), base_url: "https://ex.com".into(),
                api_key: "k".into(), api_version: String::new(), display_name: format!("{:?}", proto), is_default: false,
            }).unwrap();
            assert_eq!(store.get(&c.id).unwrap().unwrap().protocol, *proto);
        }
    }
}
