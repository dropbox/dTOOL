//! Authentication module for Codex DashFlow
//!
//! Provides OAuth authentication flow for ChatGPT account sign-in
//! and credential storage using either file-based or OS keyring storage.
//!
//! ## Usage
//!
//! ```rust,ignore
//! use codex_dashflow_core::auth::{AuthManager, AuthCredentialsStoreMode};
//!
//! let auth = AuthManager::new(AuthCredentialsStoreMode::Auto)?;
//!
//! // Check if authenticated
//! if let Some(token) = auth.get_access_token()? {
//!     // Use token for API calls
//! } else {
//!     // Need to log in
//! }
//! ```

pub mod jwt;
pub mod oauth;
mod storage;

pub use jwt::{parse_id_token, IdTokenError, IdTokenInfo};
pub use storage::{get_codex_home, AuthCredentialsStoreMode, AuthDotJson, TokenData};

use chrono::Utc;
use std::sync::Arc;
use tracing::{debug, warn};

use storage::{create_auth_storage, AuthStorageBackend};

/// Current authentication status for display purposes
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AuthStatus {
    /// Authenticated via ChatGPT OAuth
    ChatGpt { email: Option<String> },
    /// Authenticated via stored API key
    ApiKey,
    /// Using API key from environment variable
    EnvApiKey,
    /// Not authenticated
    NotAuthenticated,
}

impl std::fmt::Display for AuthStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthStatus::ChatGpt { email: Some(e) } => write!(f, "Signed in with ChatGPT ({})", e),
            AuthStatus::ChatGpt { email: None } => write!(f, "Signed in with ChatGPT"),
            AuthStatus::ApiKey => write!(f, "Using stored API key"),
            AuthStatus::EnvApiKey => write!(f, "Using OPENAI_API_KEY from environment"),
            AuthStatus::NotAuthenticated => write!(f, "Not authenticated"),
        }
    }
}

/// Error type for authentication operations
#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Not authenticated. Run 'codex-dashflow login' to sign in.")]
    NotAuthenticated,

    #[error("Token expired. Run 'codex-dashflow login' to refresh.")]
    TokenExpired,

    #[error("Authentication failed: {0}")]
    AuthFailed(String),
}

/// Result type for authentication operations
pub type AuthResult<T> = std::result::Result<T, AuthError>;

/// Authentication manager for handling credentials
#[derive(Clone)]
pub struct AuthManager {
    storage: Arc<dyn AuthStorageBackend>,
}

impl AuthManager {
    /// Create a new AuthManager with the specified storage mode
    pub fn new(mode: AuthCredentialsStoreMode) -> AuthResult<Self> {
        let codex_home = get_codex_home()?;
        let storage = create_auth_storage(codex_home, mode);
        Ok(Self { storage })
    }

    /// Load the current authentication state
    pub fn load(&self) -> AuthResult<Option<AuthDotJson>> {
        Ok(self.storage.load()?)
    }

    /// Save authentication state
    pub fn save(&self, auth: &AuthDotJson) -> AuthResult<()> {
        Ok(self.storage.save(auth)?)
    }

    /// Delete all stored authentication
    pub fn logout(&self) -> AuthResult<bool> {
        Ok(self.storage.delete()?)
    }

    /// Check if user is authenticated (has valid token or API key)
    pub fn is_authenticated(&self) -> AuthResult<bool> {
        match self.storage.load()? {
            Some(auth) => {
                // Has API key
                if auth.openai_api_key.is_some() {
                    return Ok(true);
                }
                // Has OAuth token
                if let Some(tokens) = &auth.tokens {
                    // Check if token is expired
                    if let Some(expires_at) = tokens.expires_at {
                        return Ok(expires_at > Utc::now());
                    }
                    // No expiry means it's valid
                    return Ok(true);
                }
                Ok(false)
            }
            None => Ok(false),
        }
    }

    /// Get the access token for API calls
    ///
    /// Returns the OAuth access token if available, otherwise None.
    /// Check OPENAI_API_KEY env var separately for API key auth.
    pub fn get_access_token(&self) -> AuthResult<Option<String>> {
        match self.storage.load()? {
            Some(auth) => {
                if let Some(tokens) = auth.tokens {
                    // Check if expired
                    if let Some(expires_at) = tokens.expires_at {
                        if expires_at <= Utc::now() {
                            return Err(AuthError::TokenExpired);
                        }
                    }
                    return Ok(Some(tokens.access_token));
                }
                Ok(None)
            }
            None => Ok(None),
        }
    }

    /// Get the stored API key (if any)
    pub fn get_api_key(&self) -> AuthResult<Option<String>> {
        match self.storage.load()? {
            Some(auth) => Ok(auth.openai_api_key),
            None => Ok(None),
        }
    }

    /// Store an API key (for manual API key auth)
    pub fn store_api_key(&self, api_key: &str) -> AuthResult<()> {
        let mut auth = self.storage.load()?.unwrap_or_default();
        auth.openai_api_key = Some(api_key.to_string());
        self.storage.save(&auth)?;
        Ok(())
    }

    /// Store OAuth tokens from a successful login
    pub fn store_tokens(&self, tokens: TokenData) -> AuthResult<()> {
        let mut auth = self.storage.load()?.unwrap_or_default();
        auth.tokens = Some(tokens);
        auth.last_refresh = Some(Utc::now());
        self.storage.save(&auth)?;
        Ok(())
    }

    /// Get account info if authenticated with OAuth
    pub fn get_account_info(&self) -> AuthResult<Option<(String, Option<String>)>> {
        match self.storage.load()? {
            Some(auth) => {
                if let Some(tokens) = auth.tokens {
                    let account_id = tokens.account_id.unwrap_or_else(|| "unknown".to_string());
                    return Ok(Some((account_id, tokens.email)));
                }
                Ok(None)
            }
            None => Ok(None),
        }
    }

    /// Get the best available authentication method
    ///
    /// Priority:
    /// 1. OAuth access token (if valid)
    /// 2. Stored API key
    /// 3. OPENAI_API_KEY environment variable
    pub fn get_auth_token(&self) -> AuthResult<Option<String>> {
        // Try OAuth token first
        match self.get_access_token() {
            Ok(Some(token)) => return Ok(Some(token)),
            Ok(None) => {}
            Err(AuthError::TokenExpired) => {
                // Token expired, fall through to other methods
            }
            Err(e) => return Err(e),
        }

        // Try stored API key
        if let Some(api_key) = self.get_api_key()? {
            return Ok(Some(api_key));
        }

        // Try environment variable
        if let Ok(api_key) = std::env::var("OPENAI_API_KEY") {
            return Ok(Some(api_key));
        }

        Ok(None)
    }

    /// Attempt to refresh expired OAuth tokens
    ///
    /// If the current access token is expired and a refresh token is available,
    /// this will attempt to obtain new tokens from the OAuth server.
    ///
    /// Returns the new access token on success, or an error if refresh failed.
    pub async fn refresh_if_expired(&self) -> AuthResult<Option<String>> {
        let auth = match self.storage.load()? {
            Some(a) => a,
            None => return Ok(None),
        };

        let tokens = match &auth.tokens {
            Some(t) => t,
            None => return Ok(None),
        };

        // Check if token is actually expired
        if let Some(expires_at) = tokens.expires_at {
            if expires_at > Utc::now() {
                // Token is still valid
                return Ok(Some(tokens.access_token.clone()));
            }
        } else {
            // No expiry means it's valid
            return Ok(Some(tokens.access_token.clone()));
        }

        // Token is expired, attempt refresh
        let refresh_token = match &tokens.refresh_token {
            Some(rt) => rt.clone(),
            None => {
                return Err(AuthError::AuthFailed(
                    "Token expired and no refresh token available".to_string(),
                ));
            }
        };

        debug!("Access token expired, attempting refresh");

        let refreshed =
            oauth::refresh_tokens(oauth::DEFAULT_ISSUER, oauth::CLIENT_ID, &refresh_token)
                .await
                .map_err(|e| AuthError::AuthFailed(format!("Token refresh failed: {e}")))?;

        // Parse the new ID token for expiry and email
        let parsed = oauth::parse_id_token(&refreshed.id_token)
            .map_err(|e| AuthError::AuthFailed(format!("Failed to parse refreshed token: {e}")))?;

        // Store the new tokens
        let new_tokens = TokenData {
            access_token: refreshed.access_token.clone(),
            refresh_token: Some(refreshed.refresh_token),
            expires_at: parsed.expires_at,
            account_id: tokens.account_id.clone(),
            email: parsed.email.or(tokens.email.clone()),
        };

        let mut new_auth = auth.clone();
        new_auth.tokens = Some(new_tokens);
        new_auth.last_refresh = Some(Utc::now());

        self.storage.save(&new_auth)?;

        debug!("Token refresh successful");
        Ok(Some(refreshed.access_token))
    }

    /// Get the best available authentication method, refreshing if needed
    ///
    /// Same as get_auth_token but will attempt to refresh expired OAuth tokens.
    pub async fn get_auth_token_with_refresh(&self) -> AuthResult<Option<String>> {
        // Try OAuth token first
        match self.get_access_token() {
            Ok(Some(token)) => return Ok(Some(token)),
            Ok(None) => {}
            Err(AuthError::TokenExpired) => {
                // Token expired, try to refresh
                match self.refresh_if_expired().await {
                    Ok(Some(token)) => return Ok(Some(token)),
                    Ok(None) => {}
                    Err(e) => {
                        warn!("Token refresh failed: {e}");
                        // Fall through to other methods
                    }
                }
            }
            Err(e) => return Err(e),
        }

        // Try stored API key
        if let Some(api_key) = self.get_api_key()? {
            return Ok(Some(api_key));
        }

        // Try environment variable
        if let Ok(api_key) = std::env::var("OPENAI_API_KEY") {
            return Ok(Some(api_key));
        }

        Ok(None)
    }

    /// Get the current authentication status for display
    pub fn get_status(&self) -> AuthStatus {
        // Check OAuth tokens first
        if let Ok(Some(auth)) = self.load() {
            if let Some(ref tokens) = auth.tokens {
                // Check if not expired
                if let Some(expires_at) = tokens.expires_at {
                    if expires_at > Utc::now() {
                        return AuthStatus::ChatGpt {
                            email: tokens.email.clone(),
                        };
                    }
                } else {
                    // No expiry means valid
                    return AuthStatus::ChatGpt {
                        email: tokens.email.clone(),
                    };
                }
            }
            // Check stored API key
            if auth.openai_api_key.is_some() {
                return AuthStatus::ApiKey;
            }
        }

        // Check environment variable
        if std::env::var("OPENAI_API_KEY").is_ok() {
            return AuthStatus::EnvApiKey;
        }

        AuthStatus::NotAuthenticated
    }

    /// Set up authentication for LLM calls
    ///
    /// This checks stored credentials and sets OPENAI_API_KEY env var if needed.
    /// Returns the auth status for display purposes.
    pub fn setup_for_llm(&self) -> AuthStatus {
        let status = self.get_status();

        // If we have stored auth (ChatGPT or API key), ensure OPENAI_API_KEY is set
        if let Ok(Some(token)) = self.get_auth_token() {
            // Only set if not already set or if we have stored auth
            if std::env::var("OPENAI_API_KEY").is_err()
                || matches!(status, AuthStatus::ChatGpt { .. } | AuthStatus::ApiKey)
            {
                // SAFETY: Single-threaded setup, called before spawning agent tasks
                unsafe {
                    std::env::set_var("OPENAI_API_KEY", token);
                }
            }
        }

        status
    }

    /// Set up authentication for LLM calls, refreshing expired tokens if needed
    ///
    /// This is the async version of setup_for_llm that will attempt to refresh
    /// expired OAuth tokens before falling back to other auth methods.
    pub async fn setup_for_llm_async(&self) -> AuthStatus {
        // Try to refresh expired tokens
        if let Err(e) = self.refresh_if_expired().await {
            debug!("Token refresh during setup failed (may be expected): {e}");
        }

        // Now use the synchronous setup
        self.setup_for_llm()
    }
}

impl std::fmt::Debug for AuthManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthManager")
            .field("storage", &"<storage>")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, MutexGuard, OnceLock, PoisonError};
    use tempfile::tempdir;

    /// Guard for controlling access to CODEX_DASHFLOW_HOME env var during tests
    struct TestEnvGuard {
        _guard: MutexGuard<'static, ()>,
        _dir: tempfile::TempDir,
    }

    impl TestEnvGuard {
        fn new() -> std::io::Result<Self> {
            static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
            let guard = LOCK
                .get_or_init(Mutex::default)
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            let dir = tempdir()?;
            unsafe {
                std::env::set_var("CODEX_DASHFLOW_HOME", dir.path());
            }
            Ok(Self {
                _guard: guard,
                _dir: dir,
            })
        }
    }

    impl Drop for TestEnvGuard {
        fn drop(&mut self) {
            unsafe {
                std::env::remove_var("CODEX_DASHFLOW_HOME");
            }
        }
    }

    fn create_test_manager() -> AuthResult<(AuthManager, TestEnvGuard)> {
        let guard = TestEnvGuard::new().map_err(AuthError::Io)?;
        let manager = AuthManager::new(AuthCredentialsStoreMode::File)?;
        Ok((manager, guard))
    }

    #[test]
    fn test_not_authenticated_by_default() -> AuthResult<()> {
        let (manager, _guard) = create_test_manager()?;
        assert!(!manager.is_authenticated()?);
        Ok(())
    }

    #[test]
    fn test_store_and_retrieve_api_key() -> AuthResult<()> {
        let (manager, _guard) = create_test_manager()?;

        manager.store_api_key("sk-test-key")?;

        assert!(manager.is_authenticated()?);
        assert_eq!(manager.get_api_key()?, Some("sk-test-key".to_string()));
        Ok(())
    }

    #[test]
    fn test_store_and_retrieve_tokens() -> AuthResult<()> {
        let (manager, _guard) = create_test_manager()?;

        let tokens = TokenData {
            access_token: "access-token".to_string(),
            refresh_token: Some("refresh-token".to_string()),
            expires_at: Some(Utc::now() + chrono::Duration::hours(1)),
            account_id: Some("account-123".to_string()),
            email: Some("user@example.com".to_string()),
        };
        manager.store_tokens(tokens)?;

        assert!(manager.is_authenticated()?);
        assert_eq!(
            manager.get_access_token()?,
            Some("access-token".to_string())
        );
        Ok(())
    }

    #[test]
    fn test_logout_clears_credentials() -> AuthResult<()> {
        let (manager, _guard) = create_test_manager()?;

        manager.store_api_key("sk-test-key")?;
        assert!(manager.is_authenticated()?);

        manager.logout()?;
        assert!(!manager.is_authenticated()?);
        Ok(())
    }

    #[test]
    fn test_expired_token_not_authenticated() -> AuthResult<()> {
        let (manager, _guard) = create_test_manager()?;

        let tokens = TokenData {
            access_token: "access-token".to_string(),
            refresh_token: Some("refresh-token".to_string()),
            expires_at: Some(Utc::now() - chrono::Duration::hours(1)), // Expired
            account_id: None,
            email: None,
        };
        manager.store_tokens(tokens)?;

        assert!(!manager.is_authenticated()?);
        assert!(matches!(
            manager.get_access_token(),
            Err(AuthError::TokenExpired)
        ));
        Ok(())
    }

    #[test]
    fn test_auth_status_not_authenticated() -> AuthResult<()> {
        let (manager, _guard) = create_test_manager()?;
        assert_eq!(manager.get_status(), AuthStatus::NotAuthenticated);
        Ok(())
    }

    #[test]
    fn test_auth_status_api_key() -> AuthResult<()> {
        let (manager, _guard) = create_test_manager()?;
        manager.store_api_key("sk-test-key")?;
        assert_eq!(manager.get_status(), AuthStatus::ApiKey);
        Ok(())
    }

    #[test]
    fn test_auth_status_chatgpt() -> AuthResult<()> {
        let (manager, _guard) = create_test_manager()?;
        let tokens = TokenData {
            access_token: "access-token".to_string(),
            refresh_token: Some("refresh-token".to_string()),
            expires_at: Some(Utc::now() + chrono::Duration::hours(1)),
            account_id: Some("account-123".to_string()),
            email: Some("user@example.com".to_string()),
        };
        manager.store_tokens(tokens)?;
        assert_eq!(
            manager.get_status(),
            AuthStatus::ChatGpt {
                email: Some("user@example.com".to_string())
            }
        );
        Ok(())
    }

    #[test]
    fn test_auth_status_display() {
        assert_eq!(
            format!(
                "{}",
                AuthStatus::ChatGpt {
                    email: Some("test@example.com".to_string())
                }
            ),
            "Signed in with ChatGPT (test@example.com)"
        );
        assert_eq!(
            format!("{}", AuthStatus::ChatGpt { email: None }),
            "Signed in with ChatGPT"
        );
        assert_eq!(format!("{}", AuthStatus::ApiKey), "Using stored API key");
        assert_eq!(
            format!("{}", AuthStatus::EnvApiKey),
            "Using OPENAI_API_KEY from environment"
        );
        assert_eq!(
            format!("{}", AuthStatus::NotAuthenticated),
            "Not authenticated"
        );
    }

    #[tokio::test]
    async fn test_refresh_if_expired_valid_token() -> AuthResult<()> {
        let (manager, _guard) = create_test_manager()?;

        // Store valid (non-expired) token
        let tokens = TokenData {
            access_token: "valid-access-token".to_string(),
            refresh_token: Some("refresh-token".to_string()),
            expires_at: Some(Utc::now() + chrono::Duration::hours(1)),
            account_id: Some("account-123".to_string()),
            email: Some("user@example.com".to_string()),
        };
        manager.store_tokens(tokens)?;

        // refresh_if_expired should return the existing token (no refresh needed)
        let result = manager.refresh_if_expired().await?;
        assert_eq!(result, Some("valid-access-token".to_string()));
        Ok(())
    }

    #[tokio::test]
    async fn test_refresh_if_expired_no_token() -> AuthResult<()> {
        let (manager, _guard) = create_test_manager()?;

        // No tokens stored
        let result = manager.refresh_if_expired().await?;
        assert_eq!(result, None);
        Ok(())
    }

    #[tokio::test]
    async fn test_refresh_if_expired_no_refresh_token() -> AuthResult<()> {
        let (manager, _guard) = create_test_manager()?;

        // Store expired token without refresh token
        let tokens = TokenData {
            access_token: "expired-access-token".to_string(),
            refresh_token: None, // No refresh token
            expires_at: Some(Utc::now() - chrono::Duration::hours(1)), // Expired
            account_id: None,
            email: None,
        };
        manager.store_tokens(tokens)?;

        // Should fail because no refresh token available
        let result = manager.refresh_if_expired().await;
        assert!(matches!(result, Err(AuthError::AuthFailed(_))));
        Ok(())
    }

    #[tokio::test]
    async fn test_get_auth_token_with_refresh_valid_token() -> AuthResult<()> {
        let (manager, _guard) = create_test_manager()?;

        // Store valid token
        let tokens = TokenData {
            access_token: "valid-token".to_string(),
            refresh_token: Some("refresh-token".to_string()),
            expires_at: Some(Utc::now() + chrono::Duration::hours(1)),
            account_id: None,
            email: None,
        };
        manager.store_tokens(tokens)?;

        let result = manager.get_auth_token_with_refresh().await?;
        assert_eq!(result, Some("valid-token".to_string()));
        Ok(())
    }

    #[tokio::test]
    async fn test_get_auth_token_with_refresh_falls_back_to_api_key() -> AuthResult<()> {
        let (manager, _guard) = create_test_manager()?;

        // Store expired token (refresh will fail since no real server)
        let tokens = TokenData {
            access_token: "expired-token".to_string(),
            refresh_token: Some("invalid-refresh-token".to_string()),
            expires_at: Some(Utc::now() - chrono::Duration::hours(1)), // Expired
            account_id: None,
            email: None,
        };
        manager.store_tokens(tokens)?;

        // Also store an API key as fallback
        manager.store_api_key("sk-fallback-key")?;

        // Should fall back to API key after refresh fails (network error)
        let result = manager.get_auth_token_with_refresh().await?;
        assert_eq!(result, Some("sk-fallback-key".to_string()));
        Ok(())
    }
}
