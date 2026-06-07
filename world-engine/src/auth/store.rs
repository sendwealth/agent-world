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
        let password_hash = Self::hash_password_argon2(password)?;

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

        let verified = if user.password_hash.starts_with("argon2$") {
            Self::verify_password_argon2(password, &user.password_hash)
        } else if user.password_hash.starts_with("sha256$") {
            // Legacy SHA-256 hash: verify and flag for migration
            Self::verify_password_sha256(password, &user.password_hash)
        } else {
            false
        };

        if !verified {
            return Err("Invalid username or password".into());
        }

        // Auto-migrate legacy SHA-256 hashes to argon2 on successful login
        if user.password_hash.starts_with("sha256$") {
            user.password_hash = Self::hash_password_argon2(password)
                .map_err(|e| format!("Password migration error: {}", e))?;
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

    // ── Password hashing (argon2) ────────────────────────────
    // New hashes use argon2 with format: argon2$<encoded_hash_string>
    // Legacy SHA-256 hashes (sha256$<salt>$<hash>) are auto-migrated on login.

    fn hash_password_argon2(password: &str) -> Result<String, String> {
        use argon2::password_hash::rand_core::OsRng;
        use argon2::password_hash::SaltString;
        use argon2::{Argon2, PasswordHasher};

        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        let hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| format!("Argon2 hash error: {}", e))?;
        Ok(format!("argon2${}", hash))
    }

    fn verify_password_argon2(password: &str, stored: &str) -> bool {
        use argon2::password_hash::PasswordVerifier;
        use argon2::Argon2;

        // stored format: argon2$<PHC hash string>
        let hash_str = match stored.strip_prefix("argon2$") {
            Some(h) => h,
            None => return false,
        };

        let parsed_hash = match argon2::password_hash::PasswordHash::new(hash_str) {
            Ok(h) => h,
            Err(_) => return false,
        };

        Argon2::default()
            .verify_password(password.as_bytes(), &parsed_hash)
            .is_ok()
    }

    // ── Legacy SHA-256 verification (for migration) ──────────

    fn verify_password_sha256(password: &str, stored: &str) -> bool {
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
        constant_time_eq::constant_time_eq(actual.as_bytes(), expected.as_bytes())
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
        // New registrations use argon2
        assert!(user.password_hash.starts_with("argon2$"));

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
    fn test_constant_time_eq() {
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

    #[test]
    fn test_legacy_sha256_migration_on_login() {
        use sha2::{Digest, Sha256};

        let mut store = test_store();

        // Manually insert a user with a legacy SHA-256 hash
        let salt = "test-salt-123".to_string();
        let mut hasher = Sha256::new();
        hasher.update(salt.as_bytes());
        hasher.update(b"mypass123");
        let hash_hex = format!("{:x}", hasher.finalize());
        let legacy_hash = format!("sha256${}${}", salt, hash_hex);

        let user_id = Uuid::new_v4().to_string();
        let user = HumanUser {
            id: user_id.clone(),
            username: "legacy_user".to_string(),
            password_hash: legacy_hash.clone(),
            role: HumanRole::Observer,
            created_at: Utc::now().timestamp(),
            last_login: None,
        };
        store.username_index.insert("legacy_user".to_string(), user_id.clone());
        store.users.insert(user_id.clone(), user);

        // Login should succeed and auto-migrate to argon2
        let (logged_in, _token) = store.login("legacy_user", "mypass123").unwrap();
        assert!(logged_in.password_hash.starts_with("argon2$"),
            "Password should have been migrated to argon2, got: {}", logged_in.password_hash);

        // Verify the stored user was also updated
        let stored_user = store.users.get(&user_id).unwrap();
        assert!(stored_user.password_hash.starts_with("argon2$"));

        // Can still login with the same password after migration
        let (_, token2) = store.login("legacy_user", "mypass123").unwrap();
        assert!(!token2.is_empty());
    }

    #[test]
    fn test_legacy_sha256_wrong_password_fails() {
        let mut store = test_store();

        let legacy_hash = "sha256$some-salt$deadbeef".to_string();
        let user_id = Uuid::new_v4().to_string();
        let user = HumanUser {
            id: user_id.clone(),
            username: "legacy_fail".to_string(),
            password_hash: legacy_hash,
            role: HumanRole::Observer,
            created_at: Utc::now().timestamp(),
            last_login: None,
        };
        store.username_index.insert("legacy_fail".to_string(), user_id.clone());
        store.users.insert(user_id, user);

        let err = store.login("legacy_fail", "wrongpassword").unwrap_err();
        assert!(err.contains("Invalid"));
    }
}
