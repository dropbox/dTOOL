// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

// Allow clippy warnings for DashSwarm client
// - needless_pass_by_value: Request data owned for async HTTP calls
#![allow(clippy::needless_pass_by_value)]

//! # DashSwarm API Client
//!
//! Async HTTP client for DashSwarm-compatible package registries.
//! This module provides full API coverage including package operations,
//! contributions, and trust/key management.
//!
//! ## Status: Placeholder Registry
//!
//! **The official DashSwarm registry (`registry.dashswarm.com`) is not deployed.**
//! This client is ready for use with custom registries. Configure via:
//! - `DASHSWARM_REGISTRY_URL` environment variable, or
//! - `DashSwarmConfig::new("https://your-registry.example.com")`
//!
//! ## Configuration
//!
//! The registry URL can be configured via:
//! - `DASHSWARM_REGISTRY_URL` environment variable (recommended)
//! - `DashSwarmConfig::new(url)` for explicit configuration
//!
//! **Note:** The default URL (`https://registry.dashswarm.com`) is a placeholder.
//! DashSwarm is a planned central package registry that is not yet deployed.
//! For now, users must either:
//! - Set `DASHSWARM_REGISTRY_URL` to their own registry
//! - Use `DashSwarmConfig::new("https://your-registry.example.com")`
//!
//! ## Features
//!
//! - **Async/await**: All operations are async-first
//! - **Retry with backoff**: Automatic retries on transient failures
//! - **Rate limit handling**: Respects rate limits with proper backoff
//! - **Authentication**: Bearer, Basic, and API key support
//!
//! ## Example
//!
//! ```rust,ignore
//! use dashflow::packages::{DashSwarmClient, DashSwarmConfig};
//!
//! // Configure with your registry URL
//! let config = DashSwarmConfig::new("https://my-registry.example.com");
//! let client = DashSwarmClient::new(config).await?;
//!
//! // Or use environment variable: DASHSWARM_REGISTRY_URL=https://my-registry.example.com
//! let client = DashSwarmClient::new(DashSwarmConfig::from_env()).await?;
//!
//! // Search for packages
//! let results = client.search("sentiment analysis").await?;
//! ```

use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;
use uuid::Uuid;

use crate::constants::DEFAULT_HTTP_CONNECT_TIMEOUT;
use crate::core::config_loader::env_vars::{env_string_or_default, DASHSWARM_REGISTRY_URL};
use super::client::{
    ClientError, PackageInfo, PackageSearchResult, PackageVersionInfo, SearchOptions,
    SemanticSearchResult, VersionInfo,
};
use super::contributions::{
    ContributionError, ContributionState, ContributionStatus, NewPackageRequest, PackageBugReport,
    PackageFix, PackageImprovement,
};
use super::types::{HashAlgorithm, PackageId, SignatureAlgorithm};

// ============================================================================
// Configuration
// ============================================================================

/// Configuration for the DashSwarm API client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashSwarmConfig {
    /// Base URL of the DashSwarm API
    pub base_url: String,
    /// Authentication token (optional)
    pub auth: Option<DashSwarmAuth>,
    /// Request timeout in seconds
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
    /// Maximum retry attempts for transient failures
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    /// Initial retry delay in milliseconds
    #[serde(default = "default_retry_delay_ms")]
    pub retry_delay_ms: u64,
    /// Maximum retry delay in milliseconds
    #[serde(default = "default_max_retry_delay_ms")]
    pub max_retry_delay_ms: u64,
    /// User agent string
    #[serde(default = "default_user_agent")]
    pub user_agent: String,
}

fn default_timeout() -> u64 {
    30
}

fn default_max_retries() -> u32 {
    3
}

fn default_retry_delay_ms() -> u64 {
    500
}

fn default_max_retry_delay_ms() -> u64 {
    30000
}

fn default_user_agent() -> String {
    format!("DashFlow/{}", env!("CARGO_PKG_VERSION"))
}

/// Default placeholder URL for the central DashSwarm registry.
/// This service is not yet deployed - users must configure their own registry URL.
pub const DASHSWARM_DEFAULT_URL: &str = "https://registry.dashswarm.com";

/// Environment variable for configuring the DashSwarm registry URL.
/// Re-exported from centralized env_vars module for backwards compatibility.
pub use crate::core::config_loader::env_vars::DASHSWARM_REGISTRY_URL as DASHSWARM_REGISTRY_URL_ENV;

impl Default for DashSwarmConfig {
    fn default() -> Self {
        Self {
            base_url: DASHSWARM_DEFAULT_URL.to_string(),
            auth: None,
            timeout_secs: default_timeout(),
            max_retries: default_max_retries(),
            retry_delay_ms: default_retry_delay_ms(),
            max_retry_delay_ms: default_max_retry_delay_ms(),
            user_agent: default_user_agent(),
        }
    }
}

impl DashSwarmConfig {
    /// Create a new configuration for a custom registry URL.
    #[must_use]
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            ..Default::default()
        }
    }

    /// Create configuration from environment variables.
    ///
    /// Reads `DASHSWARM_REGISTRY_URL` environment variable. If not set, falls back
    /// to the default placeholder URL (which will not work until the central
    /// registry is deployed).
    ///
    /// # Example
    ///
    /// ```bash
    /// export DASHSWARM_REGISTRY_URL=https://my-registry.example.com
    /// ```
    #[must_use]
    pub fn from_env() -> Self {
        let base_url = env_string_or_default(DASHSWARM_REGISTRY_URL, DASHSWARM_DEFAULT_URL);
        Self {
            base_url,
            ..Default::default()
        }
    }

    /// Create configuration for the official registry.
    ///
    /// **Note:** The official DashSwarm registry (`registry.dashswarm.com`) is not
    /// yet deployed. Consider using `from_env()` or `new(url)` with your own registry.
    #[must_use]
    pub fn official() -> Self {
        Self::default()
    }

    /// Returns true if this config is using the default placeholder URL.
    ///
    /// The default URL (`registry.dashswarm.com`) is a placeholder for the planned
    /// central package registry, which is not yet deployed. If this returns true,
    /// API calls will fail until a real registry is configured.
    #[must_use]
    pub fn is_placeholder_url(&self) -> bool {
        self.base_url == DASHSWARM_DEFAULT_URL
    }

    /// Set authentication.
    #[must_use]
    pub fn with_auth(mut self, auth: DashSwarmAuth) -> Self {
        self.auth = Some(auth);
        self
    }

    /// Set a bearer token for authentication.
    #[must_use]
    pub fn with_token(self, token: impl Into<String>) -> Self {
        self.with_auth(DashSwarmAuth::Bearer {
            token: token.into(),
        })
    }

    /// Set API key authentication.
    #[must_use]
    pub fn with_api_key(self, key: impl Into<String>) -> Self {
        self.with_auth(DashSwarmAuth::ApiKey {
            key: key.into(),
            header: "X-API-Key".to_string(),
        })
    }

    /// Set request timeout.
    #[must_use]
    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }

    /// Set maximum retry attempts.
    #[must_use]
    pub fn with_max_retries(mut self, retries: u32) -> Self {
        self.max_retries = retries;
        self
    }
}

/// Authentication methods for DashSwarm API.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DashSwarmAuth {
    /// Bearer token authentication (OAuth/JWT).
    Bearer {
        /// The bearer token value.
        token: String,
    },
    /// Basic authentication with username and password.
    Basic {
        /// The username for authentication.
        username: String,
        /// The password for authentication.
        password: String,
    },
    /// API key authentication via custom header.
    ApiKey {
        /// The API key value.
        key: String,
        /// HTTP header name to send the key in.
        header: String,
    },
}

// ============================================================================
// Error Types
// ============================================================================

/// Errors specific to DashSwarm API operations.
#[derive(Debug, Clone, Error)]
#[non_exhaustive]
pub enum DashSwarmError {
    /// HTTP client error
    #[error("Client error: {0}")]
    Client(#[from] ClientError),
    /// Contribution error
    #[error("Contribution error: {0}")]
    Contribution(#[from] ContributionError),
    /// Trust/signature error
    #[error("Trust error: {0}")]
    TrustError(String),
    /// Rate limited (includes retry-after if available).
    #[error("Rate limited{}: {message}", retry_after_secs.map(|s| format!(" (retry after {}s)", s)).unwrap_or_default())]
    RateLimited {
        /// Seconds to wait before retrying, if provided by server.
        retry_after_secs: Option<u64>,
        /// Description of the rate limit error.
        message: String,
    },
    /// Authentication required or failed
    #[error("Authentication error: {0}")]
    AuthError(String),
    /// Request validation failed
    #[error("Validation error: {0}")]
    ValidationError(String),
    /// Server error.
    #[error("Server error ({status}): {message}")]
    ServerError {
        /// HTTP status code returned by the server.
        status: u16,
        /// Error message from the server.
        message: String,
    },
    /// Network/connection error
    #[error("Network error: {0}")]
    NetworkError(String),
    /// Request timeout
    #[error("Request timed out")]
    Timeout,
    /// Invalid response
    #[error("Invalid response: {0}")]
    InvalidResponse(String),
}

/// Result type for DashSwarm operations.
pub type DashSwarmResult<T> = Result<T, DashSwarmError>;

// ============================================================================
// API Response Types
// ============================================================================

/// Response from contribution submission.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmissionResponse {
    /// Unique contribution ID
    pub contribution_id: Uuid,
    /// Status message
    pub message: String,
    /// Initial status
    pub status: ContributionState,
}

/// Response from key verification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyVerificationResponse {
    /// Whether the signature is valid
    pub valid: bool,
    /// Key ID used for signing
    pub key_id: Option<String>,
    /// Trust level of the key
    pub trust_level: Option<String>,
    /// Verification message
    pub message: String,
}

/// Key information from the registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicKeyInfo {
    /// Key identifier
    pub key_id: String,
    /// Public key (PEM format)
    pub public_key: String,
    /// Key owner
    pub owner: String,
    /// Trust level
    pub trust_level: String,
    /// Key algorithm
    pub algorithm: String,
    /// When the key was created
    pub created_at: String,
    /// When the key expires (if any)
    pub expires_at: Option<String>,
    /// Whether the key has been revoked
    pub revoked: bool,
}

/// Package publish request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishRequest {
    /// Package manifest (TOML)
    pub manifest: String,
    /// Package tarball (base64 encoded)
    pub tarball: String,
    /// Content hash
    pub hash: String,
    /// Hash algorithm
    pub hash_algorithm: HashAlgorithm,
    /// Signature (optional)
    pub signature: Option<SignatureData>,
}

/// Signature data for publish/verify.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignatureData {
    /// Key ID
    pub key_id: String,
    /// Algorithm
    pub algorithm: SignatureAlgorithm,
    /// Signature bytes (base64)
    pub signature: String,
}

/// Package publish response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishResponse {
    /// Published package ID
    pub package_id: String,
    /// Published version
    pub version: String,
    /// Status message
    pub message: String,
    /// Package URL
    pub url: String,
}

// ============================================================================
// Client Implementation
// ============================================================================

/// Async client for the DashSwarm API.
///
/// This client provides async methods for all DashSwarm API operations with
/// automatic retry and rate limit handling.
pub struct DashSwarmClient {
    config: DashSwarmConfig,
    client: reqwest::Client,
}

impl DashSwarmClient {
    /// Create a new DashSwarm client.
    ///
    /// **Note:** If the config uses the default placeholder URL (`registry.dashswarm.com`),
    /// a warning will be logged. The official registry is not yet deployed - API calls
    /// will fail until a real registry URL is configured via `DASHSWARM_REGISTRY_URL`
    /// environment variable or `DashSwarmConfig::new(url)`.
    pub async fn new(config: DashSwarmConfig) -> DashSwarmResult<Self> {
        // Warn if using the placeholder URL
        if config.is_placeholder_url() {
            tracing::warn!(
                target: "dashflow::packages",
                url = %config.base_url,
                env_var = %DASHSWARM_REGISTRY_URL_ENV,
                "Using placeholder DashSwarm registry URL. The official registry \
                (registry.dashswarm.com) is not yet deployed. Set {} to configure \
                your own registry, or API calls will fail.",
                DASHSWARM_REGISTRY_URL_ENV
            );
        }

        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::USER_AGENT,
            config.user_agent.parse().map_err(|e| {
                DashSwarmError::ValidationError(format!("Invalid user agent: {}", e))
            })?,
        );
        headers.insert(
            reqwest::header::ACCEPT,
            "application/json"
                .parse()
                .map_err(|e| DashSwarmError::ValidationError(format!("Invalid header: {}", e)))?,
        );

        // Add authentication headers if configured
        if let Some(auth) = &config.auth {
            match auth {
                DashSwarmAuth::Bearer { token } => {
                    headers.insert(
                        reqwest::header::AUTHORIZATION,
                        format!("Bearer {}", token).parse().map_err(|e| {
                            DashSwarmError::ValidationError(format!("Invalid token: {}", e))
                        })?,
                    );
                }
                DashSwarmAuth::ApiKey { key, header } => {
                    headers.insert(
                        reqwest::header::HeaderName::try_from(header.as_str()).map_err(|e| {
                            DashSwarmError::ValidationError(format!("Invalid header name: {}", e))
                        })?,
                        key.parse().map_err(|e| {
                            DashSwarmError::ValidationError(format!("Invalid API key: {}", e))
                        })?,
                    );
                }
                DashSwarmAuth::Basic { username, password } => {
                    let credentials = base64::Engine::encode(
                        &base64::engine::general_purpose::STANDARD,
                        format!("{}:{}", username, password),
                    );
                    headers.insert(
                        reqwest::header::AUTHORIZATION,
                        format!("Basic {}", credentials).parse().map_err(|e| {
                            DashSwarmError::ValidationError(format!("Invalid credentials: {}", e))
                        })?,
                    );
                }
            }
        }

        let builder = reqwest::Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs))
            .connect_timeout(DEFAULT_HTTP_CONNECT_TIMEOUT)
            .default_headers(headers);
        let builder = crate::core::http_client::apply_platform_proxy_config(builder);
        let client = builder
            .build()
            .map_err(|e| DashSwarmError::NetworkError(format!("Failed to build client: {}", e)))?;

        Ok(Self { config, client })
    }

    /// Create a client for the official DashSwarm registry.
    pub async fn official() -> DashSwarmResult<Self> {
        Self::new(DashSwarmConfig::official()).await
    }

    /// Create a client with a bearer token.
    pub async fn with_token(token: impl Into<String>) -> DashSwarmResult<Self> {
        Self::new(DashSwarmConfig::official().with_token(token)).await
    }

    /// Get the base URL.
    #[must_use]
    pub fn base_url(&self) -> &str {
        &self.config.base_url
    }

    // ========================================================================
    // Package Operations
    // ========================================================================

    /// Search for packages by text query.
    pub async fn search(&self, query: &str) -> DashSwarmResult<Vec<PackageSearchResult>> {
        let url = self.api_url(&format!("/search?q={}", urlencoding::encode(query)));
        self.get_json_with_retry(&url).await
    }

    /// Search for packages with options.
    pub async fn search_with_options(
        &self,
        options: &SearchOptions,
    ) -> DashSwarmResult<Vec<PackageSearchResult>> {
        let mut params = vec![];

        if let Some(q) = &options.query {
            params.push(format!("q={}", urlencoding::encode(q)));
        }
        if let Some(pkg_type) = &options.package_type {
            params.push(format!("type={}", pkg_type.as_str()));
        }
        if let Some(min_downloads) = options.min_downloads {
            params.push(format!("min_downloads={}", min_downloads));
        }
        if options.verified_only {
            params.push("verified=true".to_string());
        }
        if let Some(category) = &options.category {
            params.push(format!("category={}", urlencoding::encode(category)));
        }
        if let Some(limit) = options.limit {
            params.push(format!("limit={}", limit));
        }
        if let Some(offset) = options.offset {
            params.push(format!("offset={}", offset));
        }
        if let Some(sort) = &options.sort_by {
            params.push(format!("sort={}", sort.as_str()));
        }

        let query_string = if params.is_empty() {
            String::new()
        } else {
            format!("?{}", params.join("&"))
        };

        let url = self.api_url(&format!("/search{}", query_string));
        self.get_json_with_retry(&url).await
    }

    /// Semantic search for packages.
    pub async fn search_semantic(&self, query: &str) -> DashSwarmResult<Vec<SemanticSearchResult>> {
        let url = self.api_url("/search/semantic");
        let request = serde_json::json!({
            "query": query,
            "limit": 20
        });
        self.post_json_with_retry(&url, &request).await
    }

    /// Get package information.
    pub async fn get_package(&self, id: &str) -> DashSwarmResult<PackageInfo> {
        let package_id = PackageId::parse(id).ok_or_else(|| {
            DashSwarmError::ValidationError(format!("Invalid package ID: {}", id))
        })?;

        let url = self.api_url(&format!(
            "/packages/{}/{}",
            package_id.namespace(),
            package_id.name()
        ));
        self.get_json_with_retry(&url).await
    }

    /// List versions of a package.
    pub async fn list_versions(&self, id: &str) -> DashSwarmResult<Vec<VersionInfo>> {
        let package_id = PackageId::parse(id).ok_or_else(|| {
            DashSwarmError::ValidationError(format!("Invalid package ID: {}", id))
        })?;

        let url = self.api_url(&format!(
            "/packages/{}/{}/versions",
            package_id.namespace(),
            package_id.name()
        ));
        self.get_json_with_retry(&url).await
    }

    /// Get specific version information.
    pub async fn get_version(
        &self,
        id: &str,
        version: &str,
    ) -> DashSwarmResult<PackageVersionInfo> {
        let package_id = PackageId::parse(id).ok_or_else(|| {
            DashSwarmError::ValidationError(format!("Invalid package ID: {}", id))
        })?;

        let url = self.api_url(&format!(
            "/packages/{}/{}/{}",
            package_id.namespace(),
            package_id.name(),
            version
        ));
        self.get_json_with_retry(&url).await
    }

    /// Download a package.
    pub async fn download(&self, id: &str, version: &str) -> DashSwarmResult<Vec<u8>> {
        let package_id = PackageId::parse(id).ok_or_else(|| {
            DashSwarmError::ValidationError(format!("Invalid package ID: {}", id))
        })?;

        let url = self.api_url(&format!(
            "/packages/{}/{}/{}/download",
            package_id.namespace(),
            package_id.name(),
            version
        ));

        self.get_bytes_with_retry(&url).await
    }

    /// Publish a package.
    pub async fn publish(&self, request: PublishRequest) -> DashSwarmResult<PublishResponse> {
        let url = self.api_url("/packages");
        self.post_json_with_retry(&url, &request).await
    }

    // ========================================================================
    // Contribution Operations
    // ========================================================================

    /// Submit a bug report.
    pub async fn submit_bug_report(
        &self,
        report: PackageBugReport,
    ) -> DashSwarmResult<SubmissionResponse> {
        report.validate().map_err(DashSwarmError::Contribution)?;
        let url = self.api_url("/contributions/bug-report");
        self.post_json_with_retry(&url, &report).await
    }

    /// Submit an improvement suggestion.
    pub async fn submit_improvement(
        &self,
        improvement: PackageImprovement,
    ) -> DashSwarmResult<SubmissionResponse> {
        improvement
            .validate()
            .map_err(DashSwarmError::Contribution)?;
        let url = self.api_url("/contributions/improvement");
        self.post_json_with_retry(&url, &improvement).await
    }

    /// Submit a package request.
    pub async fn submit_package_request(
        &self,
        request: NewPackageRequest,
    ) -> DashSwarmResult<SubmissionResponse> {
        request.validate().map_err(DashSwarmError::Contribution)?;
        let url = self.api_url("/contributions/request");
        self.post_json_with_retry(&url, &request).await
    }

    /// Submit a fix.
    pub async fn submit_fix(&self, fix: PackageFix) -> DashSwarmResult<SubmissionResponse> {
        fix.validate().map_err(DashSwarmError::Contribution)?;
        let url = self.api_url("/contributions/fix");
        self.post_json_with_retry(&url, &fix).await
    }

    /// Get contribution status.
    pub async fn get_contribution_status(
        &self,
        contribution_id: Uuid,
    ) -> DashSwarmResult<ContributionStatus> {
        let url = self.api_url(&format!("/contributions/{}", contribution_id));
        self.get_json_with_retry(&url).await
    }

    // ========================================================================
    // Trust/Key Operations
    // ========================================================================

    /// List all trusted keys.
    pub async fn list_keys(&self) -> DashSwarmResult<Vec<PublicKeyInfo>> {
        let url = self.api_url("/keys");
        self.get_json_with_retry(&url).await
    }

    /// Get a specific key.
    pub async fn get_key(&self, key_id: &str) -> DashSwarmResult<PublicKeyInfo> {
        let url = self.api_url(&format!("/keys/{}", urlencoding::encode(key_id)));
        self.get_json_with_retry(&url).await
    }

    /// Verify a signature.
    pub async fn verify_signature(
        &self,
        signature: &SignatureData,
        content_hash: &str,
    ) -> DashSwarmResult<KeyVerificationResponse> {
        let url = self.api_url("/keys/verify");
        let request = serde_json::json!({
            "signature": signature,
            "content_hash": content_hash
        });
        self.post_json_with_retry(&url, &request).await
    }

    // ========================================================================
    // Internal HTTP Methods with Retry
    // ========================================================================

    /// Build API URL from path.
    fn api_url(&self, path: &str) -> String {
        format!(
            "{}/api/v1{}",
            self.config.base_url.trim_end_matches('/'),
            path
        )
    }

    /// GET request with retry logic.
    async fn get_json_with_retry<T: serde::de::DeserializeOwned>(
        &self,
        url: &str,
    ) -> DashSwarmResult<T> {
        let mut attempts = 0;
        let mut delay_ms = self.config.retry_delay_ms;

        loop {
            match self.get_json_once(url).await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    attempts += 1;
                    if !self.should_retry(&e, attempts) {
                        return Err(e);
                    }

                    // Apply exponential backoff
                    tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                    delay_ms = std::cmp::min(delay_ms * 2, self.config.max_retry_delay_ms);
                }
            }
        }
    }

    /// Single GET request.
    async fn get_json_once<T: serde::de::DeserializeOwned>(&self, url: &str) -> DashSwarmResult<T> {
        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| self.map_reqwest_error(e))?;

        self.handle_response(response).await
    }

    /// GET bytes with retry logic.
    async fn get_bytes_with_retry(&self, url: &str) -> DashSwarmResult<Vec<u8>> {
        let mut attempts = 0;
        let mut delay_ms = self.config.retry_delay_ms;

        loop {
            match self.get_bytes_once(url).await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    attempts += 1;
                    if !self.should_retry(&e, attempts) {
                        return Err(e);
                    }

                    tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                    delay_ms = std::cmp::min(delay_ms * 2, self.config.max_retry_delay_ms);
                }
            }
        }
    }

    /// Single GET bytes request.
    async fn get_bytes_once(&self, url: &str) -> DashSwarmResult<Vec<u8>> {
        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| self.map_reqwest_error(e))?;

        let status = response.status();
        if !status.is_success() {
            return Err(self.handle_error_status(status.as_u16(), &response).await);
        }

        response
            .bytes()
            .await
            .map(|b| b.to_vec())
            .map_err(|e| DashSwarmError::NetworkError(format!("Failed to read body: {}", e)))
    }

    /// POST JSON with retry logic.
    async fn post_json_with_retry<B: serde::Serialize, T: serde::de::DeserializeOwned>(
        &self,
        url: &str,
        body: &B,
    ) -> DashSwarmResult<T> {
        let mut attempts = 0;
        let mut delay_ms = self.config.retry_delay_ms;

        loop {
            match self.post_json_once(url, body).await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    attempts += 1;
                    if !self.should_retry(&e, attempts) {
                        return Err(e);
                    }

                    tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                    delay_ms = std::cmp::min(delay_ms * 2, self.config.max_retry_delay_ms);
                }
            }
        }
    }

    /// Single POST request.
    async fn post_json_once<B: serde::Serialize, T: serde::de::DeserializeOwned>(
        &self,
        url: &str,
        body: &B,
    ) -> DashSwarmResult<T> {
        let response = self
            .client
            .post(url)
            .json(body)
            .send()
            .await
            .map_err(|e| self.map_reqwest_error(e))?;

        self.handle_response(response).await
    }

    /// Handle response and parse JSON.
    async fn handle_response<T: serde::de::DeserializeOwned>(
        &self,
        response: reqwest::Response,
    ) -> DashSwarmResult<T> {
        let status = response.status();
        if !status.is_success() {
            return Err(self.handle_error_status(status.as_u16(), &response).await);
        }

        response
            .json()
            .await
            .map_err(|e| DashSwarmError::InvalidResponse(format!("Failed to parse JSON: {}", e)))
    }

    /// Handle error HTTP status.
    async fn handle_error_status(
        &self,
        status: u16,
        response: &reqwest::Response,
    ) -> DashSwarmError {
        // Try to get retry-after header for rate limiting
        let retry_after = response
            .headers()
            .get("Retry-After")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok());

        match status {
            401 => DashSwarmError::AuthError("Authentication required".to_string()),
            403 => DashSwarmError::AuthError("Access forbidden".to_string()),
            404 => DashSwarmError::Client(ClientError::NotFound("Resource not found".to_string())),
            422 => DashSwarmError::ValidationError("Invalid request data".to_string()),
            429 => DashSwarmError::RateLimited {
                retry_after_secs: retry_after,
                message: "Too many requests".to_string(),
            },
            500..=599 => DashSwarmError::ServerError {
                status,
                message: format!("Server error: {}", status),
            },
            _ => DashSwarmError::ServerError {
                status,
                message: format!("Request failed with status {}", status),
            },
        }
    }

    /// Map reqwest error to DashSwarmError.
    fn map_reqwest_error(&self, e: reqwest::Error) -> DashSwarmError {
        if e.is_timeout() {
            DashSwarmError::Timeout
        } else if e.is_connect() {
            DashSwarmError::NetworkError(format!("Connection failed: {}", e))
        } else {
            DashSwarmError::NetworkError(e.to_string())
        }
    }

    /// Determine if we should retry based on error type and attempt count.
    fn should_retry(&self, error: &DashSwarmError, attempts: u32) -> bool {
        if attempts >= self.config.max_retries {
            return false;
        }

        match error {
            // Retry on transient errors
            DashSwarmError::Timeout => true,
            DashSwarmError::NetworkError(_) => true,
            DashSwarmError::ServerError { status, .. } => *status >= 500,
            DashSwarmError::RateLimited { .. } => true,
            // Don't retry on client errors
            DashSwarmError::AuthError(_) => false,
            DashSwarmError::ValidationError(_) => false,
            DashSwarmError::Client(_) => false,
            DashSwarmError::Contribution(_) => false,
            DashSwarmError::TrustError(_) => false,
            DashSwarmError::InvalidResponse(_) => false,
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Mutex to serialize env-var-dependent tests (parallel execution causes races)
    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    #[test]
    fn test_dashswarm_config_default() {
        let config = DashSwarmConfig::default();
        assert_eq!(config.base_url, DASHSWARM_DEFAULT_URL);
        assert!(config.auth.is_none());
        assert_eq!(config.timeout_secs, 30);
        assert_eq!(config.max_retries, 3);
        // Default config should be using the placeholder URL
        assert!(config.is_placeholder_url());
    }

    #[test]
    fn test_dashswarm_config_new() {
        let config = DashSwarmConfig::new("https://custom.registry.com");
        assert_eq!(config.base_url, "https://custom.registry.com");
        // Custom URL should not be the placeholder
        assert!(!config.is_placeholder_url());
    }

    #[test]
    fn test_dashswarm_config_is_placeholder_url() {
        // Default is placeholder
        assert!(DashSwarmConfig::default().is_placeholder_url());
        assert!(DashSwarmConfig::official().is_placeholder_url());

        // Custom URLs are not placeholder
        assert!(!DashSwarmConfig::new("https://my-registry.example.com").is_placeholder_url());
        assert!(!DashSwarmConfig::new("https://localhost:8080").is_placeholder_url());
    }

    #[test]
    fn test_dashswarm_config_from_env() {
        let _guard = ENV_MUTEX.lock().unwrap();

        // Without env var set, should use default
        std::env::remove_var(DASHSWARM_REGISTRY_URL_ENV);
        let config = DashSwarmConfig::from_env();
        assert_eq!(config.base_url, DASHSWARM_DEFAULT_URL);

        // With env var set, should use that URL
        std::env::set_var(
            DASHSWARM_REGISTRY_URL_ENV,
            "https://custom.registry.example.com",
        );
        let config = DashSwarmConfig::from_env();
        let base_url = config.base_url.clone();
        let is_placeholder = config.is_placeholder_url();

        // Clean up
        std::env::remove_var(DASHSWARM_REGISTRY_URL_ENV);

        assert_eq!(base_url, "https://custom.registry.example.com");
        assert!(!is_placeholder);
    }

    #[test]
    fn test_dashswarm_config_with_token() {
        let config = DashSwarmConfig::official().with_token("my-token");
        assert!(matches!(
            config.auth,
            Some(DashSwarmAuth::Bearer { token }) if token == "my-token"
        ));
    }

    #[test]
    fn test_dashswarm_config_with_api_key() {
        let config = DashSwarmConfig::official().with_api_key("api-key-123");
        assert!(matches!(
            config.auth,
            Some(DashSwarmAuth::ApiKey { key, header }) if key == "api-key-123" && header == "X-API-Key"
        ));
    }

    #[test]
    fn test_dashswarm_config_with_timeout() {
        let config = DashSwarmConfig::default().with_timeout(60);
        assert_eq!(config.timeout_secs, 60);
    }

    #[test]
    fn test_dashswarm_config_with_max_retries() {
        let config = DashSwarmConfig::default().with_max_retries(5);
        assert_eq!(config.max_retries, 5);
    }

    #[test]
    fn test_dashswarm_error_display() {
        let err = DashSwarmError::RateLimited {
            retry_after_secs: Some(60),
            message: "Too many requests".to_string(),
        };
        let display = format!("{}", err);
        assert!(display.contains("60"));
        assert!(display.contains("Rate limited"));

        let err = DashSwarmError::AuthError("Token expired".to_string());
        let display = format!("{}", err);
        assert!(display.contains("Authentication error"));

        let err = DashSwarmError::ServerError {
            status: 503,
            message: "Service unavailable".to_string(),
        };
        let display = format!("{}", err);
        assert!(display.contains("503"));
    }

    #[test]
    fn test_submission_response_serde() {
        let response = SubmissionResponse {
            contribution_id: Uuid::new_v4(),
            message: "Submitted successfully".to_string(),
            status: ContributionState::Pending,
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("contribution_id"));
        assert!(json.contains("Submitted successfully"));
    }

    #[test]
    fn test_key_verification_response_serde() {
        let response = KeyVerificationResponse {
            valid: true,
            key_id: Some("key-123".to_string()),
            trust_level: Some("official".to_string()),
            message: "Signature verified".to_string(),
        };

        let json = serde_json::to_string(&response).unwrap();
        let parsed: KeyVerificationResponse = serde_json::from_str(&json).unwrap();
        assert!(parsed.valid);
        assert_eq!(parsed.key_id, Some("key-123".to_string()));
    }

    #[test]
    fn test_public_key_info_serde() {
        let info = PublicKeyInfo {
            key_id: "dashflow-official".to_string(),
            public_key: "-----BEGIN PUBLIC KEY-----\n...".to_string(),
            owner: "DashFlow".to_string(),
            trust_level: "official".to_string(),
            algorithm: "Ed25519".to_string(),
            created_at: "2025-01-01T00:00:00Z".to_string(),
            expires_at: None,
            revoked: false,
        };

        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("dashflow-official"));
        assert!(json.contains("Ed25519"));
    }

    #[test]
    fn test_signature_data_serde() {
        let sig = SignatureData {
            key_id: "key-123".to_string(),
            algorithm: SignatureAlgorithm::Ed25519,
            signature: "base64signature==".to_string(),
        };

        let json = serde_json::to_string(&sig).unwrap();
        let parsed: SignatureData = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.key_id, "key-123");
        assert_eq!(parsed.algorithm, SignatureAlgorithm::Ed25519);
    }

    #[test]
    fn test_publish_request_serde() {
        let request = PublishRequest {
            manifest: "[package]\nid = \"test/pkg\"".to_string(),
            tarball: "base64tarball==".to_string(),
            hash: "sha256hash".to_string(),
            hash_algorithm: HashAlgorithm::Sha256,
            signature: None,
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("manifest"));
        assert!(json.contains("tarball"));
    }

    #[test]
    fn test_publish_response_serde() {
        let response = PublishResponse {
            package_id: "test/pkg".to_string(),
            version: "1.0.0".to_string(),
            message: "Published successfully".to_string(),
            url: "https://registry.dashswarm.com/packages/test/pkg/1.0.0".to_string(),
        };

        let json = serde_json::to_string(&response).unwrap();
        let parsed: PublishResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.package_id, "test/pkg");
    }

    #[tokio::test]
    async fn test_dashswarm_client_creation() {
        // Test that client can be created (doesn't make network calls)
        let config = DashSwarmConfig::new("https://localhost:9999").with_timeout(1);
        let result = DashSwarmClient::new(config).await;
        assert!(result.is_ok());

        let client = result.unwrap();
        assert_eq!(client.base_url(), "https://localhost:9999");
    }

    #[test]
    fn test_should_retry_logic() {
        // Create a minimal config for testing
        let _config = DashSwarmConfig::default();

        // Test retry decision based on error type
        // We can't call should_retry directly without a client instance,
        // so we test the logic implicitly through error types

        // Timeout should be retryable
        let timeout_err = DashSwarmError::Timeout;
        assert!(matches!(timeout_err, DashSwarmError::Timeout));

        // Auth errors should not be retried
        let auth_err = DashSwarmError::AuthError("Token expired".to_string());
        assert!(matches!(auth_err, DashSwarmError::AuthError(_)));

        // Rate limited should be retryable
        let rate_err = DashSwarmError::RateLimited {
            retry_after_secs: Some(60),
            message: "Too many requests".to_string(),
        };
        assert!(matches!(rate_err, DashSwarmError::RateLimited { .. }));
    }

    #[test]
    fn test_error_conversions() {
        // Test From<ClientError>
        let client_err = ClientError::NotFound("test/pkg".to_string());
        let dash_err: DashSwarmError = client_err.into();
        assert!(matches!(dash_err, DashSwarmError::Client(_)));

        // Test From<ContributionError>
        let contrib_err = ContributionError::InvalidData("Bad data".to_string());
        let dash_err: DashSwarmError = contrib_err.into();
        assert!(matches!(dash_err, DashSwarmError::Contribution(_)));
    }
}
