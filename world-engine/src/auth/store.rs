use std::collections::HashMap;
use std::sync::Arc;

use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use uuid::Uuid;

use super::roles::HumanRole;

// ── JWT Claims ─────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// Subject — user ID (UUID string).
    pub sub: String,
    /// User role at token-creation time.
    pub role: HumanRole,
    /// Issued at (unix timestamp).
    pub iat: i64,
    /// Expiration (unix timestamp).
    pub exp: i64,
}

// ── Human User record ──────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HumanUser {
    pub id: String,
    pub username: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub role: HumanRole,
    pub created_at: i64,
    pub last_login: Option<i64>,
}

// ── Auth Store (in-memory, Mutex-guarded) ──────────────────────

pub type SharedAuthStore = Arc<Mutex<AuthStore>>;

#[derive(Debug, Clone)]
pub struct AuthStore {
    users: HashMap<String, HumanUser>,       // id → user
    username_index: HashMap<String, String>, // username → id
    jwt_secret: String,
    token_duration_hours: i64,
}

impl AuthStore {
    pub fn new(jwt_secret: &str) -> Self {
        Self {
            users: HashMap::new(),
            username_index: HashMap::new(),
            jwt_secret: jwt_secret.to_string(),
            token_duration_hours: 24,
        }
    }

    pub fn with_token_duration(mut self, hours: i64) -> Self {
        self.token_duration_hours = hours;
        self
    }

    // ── Registration ─────────────────────────────────────────

    pub fn register(
        &mut self,
        username: &str,
        password: &str,
        role: HumanRole,
    ) -> Result<HumanUser, String> {
        if username.trim().is_empty() {
            return Err("Username cannot be empty".into());
        }
        if password.len() < 6 {
            return Err("Password must be at least 6 characters".into());
        }
        if self.username_index.contains_key(username) {
            return Err("Username already taken".into());
        }

        let id = Uuid::new_v4().to_string();
        let password_hash = Self::hash_password(password);

        let user = HumanUser {
            id: id.clone(),
            username: username.to_string(),
            password_hash,
            role,
            created_at: Utc::now().timestamp(),
            last_login: None,
        };

        self.username_index.insert(username.to_string(), id.clone());
        self.users.insert(id, user.clone());
        Ok(user)
    }

    // ── Login ────────────────────────────────────────────────

    pub fn login(&mut self, username: &str, password: &str) -> Result<(HumanUser, String), String> {
        let user_id = self
            .username_index
            .get(username)
            .ok_or("Invalid username or password")?
            .clone();

        let user = self
            .users
            .get_mut(&user_id)
            .ok_or("Invalid username or password")?;

        if !Self::verify_password(password, &user.password_hash) {
            return Err("Invalid username or password".into());
        }

        user.last_login = Some(Utc::now().timestamp());
        let user_clone = user.clone();

        let token = self.issue_token(&user_clone)?;
        Ok((user_clone, token))
    }

    // ── Token operations ─────────────────────────────────────

    fn issue_token(&self, user: &HumanUser) -> Result<String, String> {
        let now = Utc::now();
        let claims = Claims {
            sub: user.id.clone(),
            role: user.role,
            iat: now.timestamp(),
            exp: (now + Duration::hours(self.token_duration_hours)).timestamp(),
        };

        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.jwt_secret.as_bytes()),
        )
        .map_err(|e| format!("Token encoding error: {}", e))
    }

    pub fn verify_token(&self, token: &str) -> Result<Claims, String> {
        let token_data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.jwt_secret.as_bytes()),
            &Validation::default(),
        )
        .map_err(|e| format!("Invalid token: {}", e))?;

        Ok(token_data.claims)
    }

    // ── User management ──────────────────────────────────────

    pub fn get_user(&self, id: &str) -> Option<HumanUser> {
        self.users.get(id).cloned()
    }

    pub fn update_role(&mut self, user_id: &str, new_role: HumanRole) -> Result<HumanUser, String> {
        let user = self.users.get_mut(user_id).ok_or("User not found")?;
        user.role = new_role;
        Ok(user.clone())
    }

    pub fn list_users(&self) -> Vec<HumanUser> {
        self.users.values().cloned().collect()
    }

    // ── Password hashing (SHA-256 with per-user salt) ────────
    // Note: For a simulation platform this is adequate.
    // Production should migrate to argon2. The hash format is:
    //   sha256$<salt_hex>$<hash_hex>

    fn hash_password(password: &str) -> String {
        use sha2::{Digest, Sha256};
        let salt = Uuid::new_v4().to_string();
        let mut hasher = Sha256::new();
        hasher.update(salt.as_bytes());
        hasher.update(password.as_bytes());
        let hash = format!("{:x}", hasher.finalize());
        format!("sha256${}${}", salt, hash)
    }

    fn verify_password(password: &str, stored: &str) -> bool {
        use sha2::{Digest, Sha256};
        let parts: Vec<&str> = stored.splitn(3, '$').collect();
        if parts.len() != 3 || parts[0] != "sha256" {
            return false;
        }
        let salt = parts[1];
        let expected = parts[2];
        let mut hasher = Sha256::new();
        hasher.update(salt.as_bytes());
        hasher.update(password.as_bytes());
        let actual = format!("{:x}", hasher.finalize());
        // Constant-time comparison to prevent timing attacks
        constant_time_eq::constant_time_eq(actual.as_bytes(), expected.as_bytes())
    }
}

// We only need constant_time_eq for password verification.
// Since it's a small dep, we inline a simple implementation.
mod constant_time_eq {
    pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
        if a.len() != b.len() {
            return false;
        }
        let mut result: u8 = 0;
        for (x, y) in a.iter().zip(b.iter()) {
            result |= x ^ y;
        }
        result == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_store() -> AuthStore {
        AuthStore::new("test-secret-key-for-testing")
    }

    #[test]
    fn test_register_and_login() {
        let mut store = test_store();
        let user = store
            .register("alice", "password123", HumanRole::Investor)
            .unwrap();
        assert_eq!(user.username, "alice");
        assert_eq!(user.role, HumanRole::Investor);

        let (logged_in, token) = store.login("alice", "password123").unwrap();
        assert_eq!(logged_in.id, user.id);
        assert!(!token.is_empty());
    }

    #[test]
    fn test_register_duplicate_fails() {
        let mut store = test_store();
        store
            .register("bob", "pass123456", HumanRole::Observer)
            .unwrap();
        let err = store
            .register("bob", "otherpass1", HumanRole::Investor)
            .unwrap_err();
        assert!(err.contains("already taken"));
    }

    #[test]
    fn test_register_short_password() {
        let mut store = test_store();
        let err = store
            .register("charlie", "short", HumanRole::Observer)
            .unwrap_err();
        assert!(err.contains("at least 6"));
    }

    #[test]
    fn test_login_wrong_password() {
        let mut store = test_store();
        store
            .register("dave", "password123", HumanRole::Observer)
            .unwrap();
        let err = store.login("dave", "wrongpass1").unwrap_err();
        assert!(err.contains("Invalid"));
    }

    #[test]
    fn test_login_unknown_user() {
        let mut store = test_store();
        let err = store.login("nobody", "password123").unwrap_err();
        assert!(err.contains("Invalid"));
    }

    #[test]
    fn test_jwt_encode_decode() {
        let mut store = test_store();
        store
            .register("eve", "password123", HumanRole::Creator)
            .unwrap();
        let (user, token) = store.login("eve", "password123").unwrap();

        let claims = store.verify_token(&token).unwrap();
        assert_eq!(claims.sub, user.id);
        assert_eq!(claims.role, HumanRole::Creator);
    }

    #[test]
    fn test_invalid_token_rejected() {
        let store = test_store();
        let err = store.verify_token("totally.invalid.token").unwrap_err();
        assert!(err.contains("Invalid token"));
    }

    #[test]
    fn test_wrong_secret_rejected() {
        let mut store = test_store();
        store
            .register("frank", "password123", HumanRole::Observer)
            .unwrap();
        let (_, token) = store.login("frank", "password123").unwrap();

        let other_store = AuthStore::new("different-secret");
        let err = other_store.verify_token(&token).unwrap_err();
        assert!(err.contains("Invalid token"));
    }

    #[test]
    fn test_password_hash_constant_time() {
        // Verify our constant_time_eq works correctly
        assert!(constant_time_eq::constant_time_eq(b"hello", b"hello"));
        assert!(!constant_time_eq::constant_time_eq(b"hello", b"world"));
        assert!(!constant_time_eq::constant_time_eq(b"hello", b"hella"));
    }

    #[test]
    fn test_get_user() {
        let mut store = test_store();
        let user = store
            .register("grace", "password123", HumanRole::Experimenter)
            .unwrap();
        let found = store.get_user(&user.id).unwrap();
        assert_eq!(found.username, "grace");
        assert_eq!(found.role, HumanRole::Experimenter);
    }

    #[test]
    fn test_update_role() {
        let mut store = test_store();
        let user = store
            .register("heidi", "password123", HumanRole::Observer)
            .unwrap();
        let updated = store.update_role(&user.id, HumanRole::Investor).unwrap();
        assert_eq!(updated.role, HumanRole::Investor);
    }

    #[test]
    fn test_list_users() {
        let mut store = test_store();
        store
            .register("u1", "password123", HumanRole::Observer)
            .unwrap();
        store
            .register("u2", "password123", HumanRole::Investor)
            .unwrap();
        assert_eq!(store.list_users().len(), 2);
    }

    #[test]
    fn test_password_has_unique_salts() {
        let mut store = test_store();
        let u1 = store
            .register("s1", "same_pass1", HumanRole::Observer)
            .unwrap();
        let u2 = store
            .register("s2", "same_pass1", HumanRole::Observer)
            .unwrap();
        // Same password, different hashes due to unique salts
        assert_ne!(u1.password_hash, u2.password_hash);
    }
}
