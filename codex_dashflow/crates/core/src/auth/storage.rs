//! Authentication credential storage
//!
//! Supports multiple storage backends:
//! - File: `~/.codex-dashflow/auth.json`
//! - Keyring: OS-native secure storage (macOS Keychain, etc.)
//! - Auto: prefers keyring, falls back to file
//!
//! Based on patterns from OpenAI Codex `core/src/auth/storage.rs`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt::Debug;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::warn;

/// Determine where Codex DashFlow should store CLI auth credentials.
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuthCredentialsStoreMode {
    #[default]
    /// Persist credentials in CODEX_HOME/auth.json.
    File,
    /// Persist credentials in the keyring. Fail if unavailable.
    Keyring,
    /// Use keyring when available; otherwise, fall back to a file in CODEX_HOME.
    Auto,
}

/// Token data for OAuth authentication
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TokenData {
    /// Access token for API calls
    pub access_token: String,
    /// Refresh token for obtaining new access tokens
    pub refresh_token: Option<String>,
    /// Token expiration time
    pub expires_at: Option<DateTime<Utc>>,
    /// Account identifier (ChatGPT account ID)
    pub account_id: Option<String>,
    /// Email associated with the account
    pub email: Option<String>,
}

/// Expected structure for $CODEX_HOME/auth.json.
#[derive(Default, Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct AuthDotJson {
    /// API key (if using direct API key auth)
    #[serde(
        rename = "OPENAI_API_KEY",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub openai_api_key: Option<String>,

    /// OAuth tokens (if using OAuth flow)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens: Option<TokenData>,

    /// Last token refresh time
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_refresh: Option<DateTime<Utc>>,
}

/// Get the codex-dashflow home directory
pub fn get_codex_home() -> std::io::Result<PathBuf> {
    if let Ok(codex_home) = std::env::var("CODEX_DASHFLOW_HOME") {
        return Ok(PathBuf::from(codex_home));
    }

    let home_dir = dirs::home_dir().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::NotFound, "Home directory not found")
    })?;

    Ok(home_dir.join(".codex-dashflow"))
}

pub(super) fn get_auth_file(codex_home: &Path) -> PathBuf {
    codex_home.join("auth.json")
}

pub(super) fn delete_file_if_exists(codex_home: &Path) -> std::io::Result<bool> {
    let auth_file = get_auth_file(codex_home);
    match std::fs::remove_file(&auth_file) {
        Ok(()) => Ok(true),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(err) => Err(err),
    }
}

/// Trait for authentication storage backends
pub(super) trait AuthStorageBackend: Debug + Send + Sync {
    fn load(&self) -> std::io::Result<Option<AuthDotJson>>;
    fn save(&self, auth: &AuthDotJson) -> std::io::Result<()>;
    fn delete(&self) -> std::io::Result<bool>;
}

/// File-based authentication storage
#[derive(Clone, Debug)]
pub(super) struct FileAuthStorage {
    codex_home: PathBuf,
}

impl FileAuthStorage {
    pub(super) fn new(codex_home: PathBuf) -> Self {
        Self { codex_home }
    }

    /// Attempt to read and parse the `auth.json` file
    pub(super) fn try_read_auth_json(&self, auth_file: &Path) -> std::io::Result<AuthDotJson> {
        let mut file = File::open(auth_file)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        let auth_dot_json: AuthDotJson = serde_json::from_str(&contents)?;
        Ok(auth_dot_json)
    }
}

impl AuthStorageBackend for FileAuthStorage {
    fn load(&self) -> std::io::Result<Option<AuthDotJson>> {
        let auth_file = get_auth_file(&self.codex_home);
        let auth_dot_json = match self.try_read_auth_json(&auth_file) {
            Ok(auth) => auth,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(err),
        };
        Ok(Some(auth_dot_json))
    }

    fn save(&self, auth_dot_json: &AuthDotJson) -> std::io::Result<()> {
        let auth_file = get_auth_file(&self.codex_home);

        if let Some(parent) = auth_file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json_data = serde_json::to_string_pretty(auth_dot_json)?;
        let mut options = OpenOptions::new();
        options.truncate(true).write(true).create(true);
        #[cfg(unix)]
        {
            options.mode(0o600);
        }
        let mut file = options.open(auth_file)?;
        file.write_all(json_data.as_bytes())?;
        file.flush()?;
        Ok(())
    }

    fn delete(&self) -> std::io::Result<bool> {
        delete_file_if_exists(&self.codex_home)
    }
}

const KEYRING_SERVICE: &str = "Codex DashFlow Auth";

/// Compute a stable key from the codex home path
fn compute_store_key(codex_home: &Path) -> std::io::Result<String> {
    let canonical = codex_home
        .canonicalize()
        .unwrap_or_else(|_| codex_home.to_path_buf());
    let path_str = canonical.to_string_lossy();
    let mut hasher = Sha256::new();
    hasher.update(path_str.as_bytes());
    let digest = hasher.finalize();
    let hex = format!("{digest:x}");
    let truncated = hex.get(..16).unwrap_or(&hex);
    Ok(format!("cli|{truncated}"))
}

/// Keyring-based authentication storage
#[derive(Clone, Debug)]
struct KeyringAuthStorage {
    codex_home: PathBuf,
}

impl KeyringAuthStorage {
    fn new(codex_home: PathBuf) -> Self {
        Self { codex_home }
    }

    fn load_from_keyring(&self, key: &str) -> std::io::Result<Option<AuthDotJson>> {
        let entry = keyring::Entry::new(KEYRING_SERVICE, key)
            .map_err(|e| std::io::Error::other(e.to_string()))?;

        match entry.get_password() {
            Ok(serialized) => serde_json::from_str(&serialized).map(Some).map_err(|err| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("failed to deserialize CLI auth from keyring: {err}"),
                )
            }),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(err) => Err(std::io::Error::other(format!(
                "failed to load CLI auth from keyring: {err}"
            ))),
        }
    }

    fn save_to_keyring(&self, key: &str, value: &str) -> std::io::Result<()> {
        let entry = keyring::Entry::new(KEYRING_SERVICE, key)
            .map_err(|e| std::io::Error::other(e.to_string()))?;

        entry.set_password(value).map_err(|err| {
            let message = format!("failed to write OAuth tokens to keyring: {err}");
            warn!("{message}");
            std::io::Error::other(message)
        })
    }
}

impl AuthStorageBackend for KeyringAuthStorage {
    fn load(&self) -> std::io::Result<Option<AuthDotJson>> {
        let key = compute_store_key(&self.codex_home)?;
        self.load_from_keyring(&key)
    }

    fn save(&self, auth: &AuthDotJson) -> std::io::Result<()> {
        let key = compute_store_key(&self.codex_home)?;
        let serialized = serde_json::to_string(auth).map_err(std::io::Error::other)?;
        self.save_to_keyring(&key, &serialized)?;
        if let Err(err) = delete_file_if_exists(&self.codex_home) {
            warn!("failed to remove CLI auth fallback file: {err}");
        }
        Ok(())
    }

    fn delete(&self) -> std::io::Result<bool> {
        let key = compute_store_key(&self.codex_home)?;
        let entry = keyring::Entry::new(KEYRING_SERVICE, &key)
            .map_err(|e| std::io::Error::other(e.to_string()))?;

        let keyring_removed = match entry.delete_credential() {
            Ok(()) => true,
            Err(keyring::Error::NoEntry) => false,
            Err(err) => {
                return Err(std::io::Error::other(format!(
                    "failed to delete auth from keyring: {err}"
                )))
            }
        };
        let file_removed = delete_file_if_exists(&self.codex_home)?;
        Ok(keyring_removed || file_removed)
    }
}

/// Auto storage that prefers keyring, falls back to file
#[derive(Clone, Debug)]
struct AutoAuthStorage {
    keyring_storage: Arc<KeyringAuthStorage>,
    file_storage: Arc<FileAuthStorage>,
}

impl AutoAuthStorage {
    fn new(codex_home: PathBuf) -> Self {
        Self {
            keyring_storage: Arc::new(KeyringAuthStorage::new(codex_home.clone())),
            file_storage: Arc::new(FileAuthStorage::new(codex_home)),
        }
    }
}

impl AuthStorageBackend for AutoAuthStorage {
    fn load(&self) -> std::io::Result<Option<AuthDotJson>> {
        match self.keyring_storage.load() {
            Ok(Some(auth)) => Ok(Some(auth)),
            Ok(None) => self.file_storage.load(),
            Err(err) => {
                warn!("failed to load CLI auth from keyring, falling back to file storage: {err}");
                self.file_storage.load()
            }
        }
    }

    fn save(&self, auth: &AuthDotJson) -> std::io::Result<()> {
        match self.keyring_storage.save(auth) {
            Ok(()) => Ok(()),
            Err(err) => {
                warn!("failed to save auth to keyring, falling back to file storage: {err}");
                self.file_storage.save(auth)
            }
        }
    }

    fn delete(&self) -> std::io::Result<bool> {
        // Keyring storage will delete from disk as well
        self.keyring_storage.delete()
    }
}

/// Create the appropriate auth storage backend based on mode
pub(super) fn create_auth_storage(
    codex_home: PathBuf,
    mode: AuthCredentialsStoreMode,
) -> Arc<dyn AuthStorageBackend> {
    match mode {
        AuthCredentialsStoreMode::File => Arc::new(FileAuthStorage::new(codex_home)),
        AuthCredentialsStoreMode::Keyring => Arc::new(KeyringAuthStorage::new(codex_home)),
        AuthCredentialsStoreMode::Auto => Arc::new(AutoAuthStorage::new(codex_home)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use pretty_assertions::assert_eq;
    use tempfile::tempdir;

    #[test]
    fn file_storage_load_returns_auth_dot_json() -> anyhow::Result<()> {
        let codex_home = tempdir()?;
        let storage = FileAuthStorage::new(codex_home.path().to_path_buf());
        let auth_dot_json = AuthDotJson {
            openai_api_key: Some("test-key".to_string()),
            tokens: None,
            last_refresh: Some(Utc::now()),
        };

        storage.save(&auth_dot_json)?;

        let loaded = storage.load()?;
        assert_eq!(Some(auth_dot_json), loaded);
        Ok(())
    }

    #[test]
    fn file_storage_save_persists_auth_dot_json() -> anyhow::Result<()> {
        let codex_home = tempdir()?;
        let storage = FileAuthStorage::new(codex_home.path().to_path_buf());
        let auth_dot_json = AuthDotJson {
            openai_api_key: Some("test-key".to_string()),
            tokens: None,
            last_refresh: Some(Utc::now()),
        };

        let file = get_auth_file(codex_home.path());
        storage.save(&auth_dot_json)?;

        let same_auth_dot_json = storage.try_read_auth_json(&file)?;
        assert_eq!(auth_dot_json, same_auth_dot_json);
        Ok(())
    }

    #[test]
    fn file_storage_delete_removes_auth_file() -> anyhow::Result<()> {
        let dir = tempdir()?;
        let auth_dot_json = AuthDotJson {
            openai_api_key: Some("sk-test-key".to_string()),
            tokens: None,
            last_refresh: None,
        };
        let storage = create_auth_storage(dir.path().to_path_buf(), AuthCredentialsStoreMode::File);
        storage.save(&auth_dot_json)?;
        assert!(dir.path().join("auth.json").exists());
        let storage = FileAuthStorage::new(dir.path().to_path_buf());
        let removed = storage.delete()?;
        assert!(removed);
        assert!(!dir.path().join("auth.json").exists());
        Ok(())
    }

    #[test]
    fn test_auth_dot_json_with_tokens() -> anyhow::Result<()> {
        let auth = AuthDotJson {
            openai_api_key: None,
            tokens: Some(TokenData {
                access_token: "access-token".to_string(),
                refresh_token: Some("refresh-token".to_string()),
                expires_at: Some(Utc::now()),
                account_id: Some("account-123".to_string()),
                email: Some("user@example.com".to_string()),
            }),
            last_refresh: Some(Utc::now()),
        };

        let json = serde_json::to_string(&auth)?;
        let parsed: AuthDotJson = serde_json::from_str(&json)?;

        assert!(parsed.tokens.is_some());
        assert_eq!(parsed.tokens.as_ref().unwrap().access_token, "access-token");
        Ok(())
    }

    #[test]
    fn test_auth_credentials_store_mode_default() {
        let mode: AuthCredentialsStoreMode = Default::default();
        assert_eq!(mode, AuthCredentialsStoreMode::File);
    }

    #[test]
    fn test_auth_credentials_store_mode_serialization() -> anyhow::Result<()> {
        let file_mode = AuthCredentialsStoreMode::File;
        let keyring_mode = AuthCredentialsStoreMode::Keyring;
        let auto_mode = AuthCredentialsStoreMode::Auto;

        // Serialize
        let file_json = serde_json::to_string(&file_mode)?;
        let keyring_json = serde_json::to_string(&keyring_mode)?;
        let auto_json = serde_json::to_string(&auto_mode)?;

        assert_eq!(file_json, "\"file\"");
        assert_eq!(keyring_json, "\"keyring\"");
        assert_eq!(auto_json, "\"auto\"");

        // Deserialize
        let parsed_file: AuthCredentialsStoreMode = serde_json::from_str("\"file\"")?;
        let parsed_keyring: AuthCredentialsStoreMode = serde_json::from_str("\"keyring\"")?;
        let parsed_auto: AuthCredentialsStoreMode = serde_json::from_str("\"auto\"")?;

        assert_eq!(parsed_file, AuthCredentialsStoreMode::File);
        assert_eq!(parsed_keyring, AuthCredentialsStoreMode::Keyring);
        assert_eq!(parsed_auto, AuthCredentialsStoreMode::Auto);
        Ok(())
    }

    #[test]
    fn test_token_data_serialization() -> anyhow::Result<()> {
        let token = TokenData {
            access_token: "test-access".to_string(),
            refresh_token: Some("test-refresh".to_string()),
            expires_at: None,
            account_id: None,
            email: None,
        };

        let json = serde_json::to_string(&token)?;
        let parsed: TokenData = serde_json::from_str(&json)?;

        assert_eq!(parsed.access_token, "test-access");
        assert_eq!(parsed.refresh_token, Some("test-refresh".to_string()));
        assert!(parsed.expires_at.is_none());
        Ok(())
    }

    #[test]
    fn test_token_data_minimal() -> anyhow::Result<()> {
        let token = TokenData {
            access_token: "min-access".to_string(),
            refresh_token: None,
            expires_at: None,
            account_id: None,
            email: None,
        };

        let json = serde_json::to_string(&token)?;
        let parsed: TokenData = serde_json::from_str(&json)?;

        assert_eq!(parsed.access_token, "min-access");
        assert!(parsed.refresh_token.is_none());
        Ok(())
    }

    #[test]
    fn test_auth_dot_json_default() {
        let auth = AuthDotJson::default();
        assert!(auth.openai_api_key.is_none());
        assert!(auth.tokens.is_none());
        assert!(auth.last_refresh.is_none());
    }

    #[test]
    fn test_auth_dot_json_api_key_only() -> anyhow::Result<()> {
        let auth = AuthDotJson {
            openai_api_key: Some("sk-test-key-12345".to_string()),
            tokens: None,
            last_refresh: None,
        };

        let json = serde_json::to_string(&auth)?;
        assert!(json.contains("OPENAI_API_KEY"));
        assert!(json.contains("sk-test-key-12345"));

        let parsed: AuthDotJson = serde_json::from_str(&json)?;
        assert_eq!(parsed.openai_api_key, Some("sk-test-key-12345".to_string()));
        Ok(())
    }

    #[test]
    fn test_get_auth_file_returns_correct_path() {
        let codex_home = PathBuf::from("/home/test/.codex-dashflow");
        let auth_file = get_auth_file(&codex_home);
        assert_eq!(
            auth_file,
            PathBuf::from("/home/test/.codex-dashflow/auth.json")
        );
    }

    #[test]
    fn test_delete_file_if_exists_nonexistent() -> anyhow::Result<()> {
        let dir = tempdir()?;
        let result = delete_file_if_exists(dir.path())?;
        assert!(!result); // File didn't exist
        Ok(())
    }

    #[test]
    fn test_delete_file_if_exists_existing() -> anyhow::Result<()> {
        let dir = tempdir()?;
        let auth_file = get_auth_file(dir.path());
        std::fs::create_dir_all(dir.path())?;
        std::fs::write(&auth_file, "{}")?;

        let result = delete_file_if_exists(dir.path())?;
        assert!(result); // File existed and was deleted
        assert!(!auth_file.exists());
        Ok(())
    }

    #[test]
    fn test_file_storage_load_nonexistent() -> anyhow::Result<()> {
        let dir = tempdir()?;
        let storage = FileAuthStorage::new(dir.path().to_path_buf());
        let result = storage.load()?;
        assert!(result.is_none());
        Ok(())
    }

    #[test]
    fn test_file_storage_save_creates_directory() -> anyhow::Result<()> {
        let dir = tempdir()?;
        let nested_path = dir.path().join("nested").join("path");
        let storage = FileAuthStorage::new(nested_path.clone());

        let auth = AuthDotJson {
            openai_api_key: Some("test-key".to_string()),
            tokens: None,
            last_refresh: None,
        };

        storage.save(&auth)?;
        assert!(get_auth_file(&nested_path).exists());
        Ok(())
    }

    #[test]
    fn test_file_storage_roundtrip_with_all_fields() -> anyhow::Result<()> {
        let dir = tempdir()?;
        let storage = FileAuthStorage::new(dir.path().to_path_buf());
        let timestamp = Utc::now();

        let auth = AuthDotJson {
            openai_api_key: Some("api-key".to_string()),
            tokens: Some(TokenData {
                access_token: "access".to_string(),
                refresh_token: Some("refresh".to_string()),
                expires_at: Some(timestamp),
                account_id: Some("acc-123".to_string()),
                email: Some("test@test.com".to_string()),
            }),
            last_refresh: Some(timestamp),
        };

        storage.save(&auth)?;
        let loaded = storage.load()?.unwrap();

        assert_eq!(loaded.openai_api_key, auth.openai_api_key);
        assert_eq!(loaded.tokens.as_ref().unwrap().access_token, "access");
        assert_eq!(
            loaded.tokens.as_ref().unwrap().email,
            Some("test@test.com".to_string())
        );
        Ok(())
    }

    #[test]
    fn test_file_storage_overwrite() -> anyhow::Result<()> {
        let dir = tempdir()?;
        let storage = FileAuthStorage::new(dir.path().to_path_buf());

        let auth1 = AuthDotJson {
            openai_api_key: Some("first-key".to_string()),
            tokens: None,
            last_refresh: None,
        };
        storage.save(&auth1)?;

        let auth2 = AuthDotJson {
            openai_api_key: Some("second-key".to_string()),
            tokens: None,
            last_refresh: None,
        };
        storage.save(&auth2)?;

        let loaded = storage.load()?.unwrap();
        assert_eq!(loaded.openai_api_key, Some("second-key".to_string()));
        Ok(())
    }

    #[test]
    fn test_create_auth_storage_file_mode() {
        let dir = tempdir().unwrap();
        let storage = create_auth_storage(dir.path().to_path_buf(), AuthCredentialsStoreMode::File);
        // Can load from file storage
        let result = storage.load();
        assert!(result.is_ok());
    }

    #[test]
    fn test_file_storage_try_read_invalid_json() -> anyhow::Result<()> {
        let dir = tempdir()?;
        let auth_file = get_auth_file(dir.path());
        std::fs::create_dir_all(dir.path())?;
        std::fs::write(&auth_file, "not valid json")?;

        let storage = FileAuthStorage::new(dir.path().to_path_buf());
        let result = storage.load();
        assert!(result.is_err());
        Ok(())
    }

    #[test]
    fn test_file_storage_delete_returns_false_when_no_file() -> anyhow::Result<()> {
        let dir = tempdir()?;
        let storage = FileAuthStorage::new(dir.path().to_path_buf());
        let deleted = storage.delete()?;
        assert!(!deleted);
        Ok(())
    }

    #[test]
    fn test_file_storage_delete_returns_true_when_file_exists() -> anyhow::Result<()> {
        let dir = tempdir()?;
        let storage = FileAuthStorage::new(dir.path().to_path_buf());

        let auth = AuthDotJson::default();
        storage.save(&auth)?;

        let deleted = storage.delete()?;
        assert!(deleted);
        assert!(!get_auth_file(dir.path()).exists());
        Ok(())
    }

    #[test]
    fn test_compute_store_key_deterministic() -> anyhow::Result<()> {
        let path = PathBuf::from("/tmp/test");
        let key1 = compute_store_key(&path)?;
        let key2 = compute_store_key(&path)?;
        assert_eq!(key1, key2);
        assert!(key1.starts_with("cli|"));
        Ok(())
    }

    #[test]
    fn test_compute_store_key_different_paths() -> anyhow::Result<()> {
        let path1 = PathBuf::from("/tmp/test1");
        let path2 = PathBuf::from("/tmp/test2");
        let key1 = compute_store_key(&path1)?;
        let key2 = compute_store_key(&path2)?;
        assert_ne!(key1, key2);
        Ok(())
    }

    #[test]
    fn test_token_data_equality() {
        let token1 = TokenData {
            access_token: "token".to_string(),
            refresh_token: Some("refresh".to_string()),
            expires_at: None,
            account_id: None,
            email: None,
        };
        let token2 = token1.clone();
        assert_eq!(token1, token2);
    }

    #[test]
    fn test_auth_dot_json_equality() {
        let auth1 = AuthDotJson {
            openai_api_key: Some("key".to_string()),
            tokens: None,
            last_refresh: None,
        };
        let auth2 = auth1.clone();
        assert_eq!(auth1, auth2);
    }

    #[test]
    fn test_get_codex_home_uses_env_var() -> anyhow::Result<()> {
        // Save original value
        let original = std::env::var("CODEX_DASHFLOW_HOME").ok();

        std::env::set_var("CODEX_DASHFLOW_HOME", "/custom/path");
        let result = get_codex_home()?;
        assert_eq!(result, PathBuf::from("/custom/path"));

        // Restore
        match original {
            Some(val) => std::env::set_var("CODEX_DASHFLOW_HOME", val),
            None => std::env::remove_var("CODEX_DASHFLOW_HOME"),
        }
        Ok(())
    }
}
