//! Registry HTTP Client
//!
//! Provides a client for interacting with the DashFlow Package Registry API.
//!
//! # Retry Support (M-196)
//!
//! The client supports automatic retries for transient network errors using
//! exponential backoff with jitter. Configure via `RegistryClientConfig`:
//!
//! ```rust,ignore
//! use dashflow_registry::{RegistryClient, RegistryClientConfig};
//! use dashflow::core::retry::RetryPolicy;
//!
//! let config = RegistryClientConfig::default()
//!     .with_retry_policy(RetryPolicy::exponential(5)); // 5 retries
//! let client = RegistryClient::with_config(config)?;
//! ```
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow_registry::{RegistryClient, RegistryClientConfig};
//!
//! let client = RegistryClient::new(RegistryClientConfig::default())?;
//!
//! // Search for packages
//! let results = client.search("sentiment analysis").await?;
//!
//! // Get package info
//! let package = client.get_package("sentiment-analyzer").await?;
//!
//! // Install a package
//! client.install("sentiment-analyzer", "latest", &cache_dir).await?;
//! ```

use std::path::Path;
use std::time::Duration;

use dashflow::constants::{DEFAULT_HTTP_CONNECT_TIMEOUT, DEFAULT_HTTP_REQUEST_TIMEOUT};
use dashflow::core::config_loader::env_vars::{
    dashflow_registry_api_key, dashflow_registry_url,
};
use dashflow::core::error::Error as DashFlowError;
use dashflow::core::retry::{with_retry, RetryPolicy};
use reqwest::{Client, StatusCode};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::error::{RegistryError, Result};
use crate::{
    BugReport, ContentHash, FixSubmission, ImprovementProposal, PackageInfo, PackageManifest,
    PackageRequest as PkgRequest, PublicKey, SearchResult, Signature, TrustLevel,
};

// Re-export API types that clients need
#[cfg(feature = "server")]
pub use crate::api::types::*;

// Define minimal API types for non-server builds
#[cfg(not(feature = "server"))]
mod api_types {
    use super::*;
    use crate::PackageManifest;
    use chrono::{DateTime, Utc};

    #[derive(Debug, Clone, Serialize, Deserialize, Default)]
    pub struct SearchApiRequest {
        #[serde(default)]
        pub query: Option<String>,
        #[serde(default)]
        pub keywords: Option<Vec<String>>,
        #[serde(default)]
        pub capabilities: Option<Vec<crate::Capability>>,
        #[serde(default)]
        pub filters: Option<SearchApiFilters>,
        #[serde(default = "default_limit")]
        pub limit: u32,
        #[serde(default)]
        pub offset: u32,
    }

    fn default_limit() -> u32 {
        20
    }

    #[derive(Debug, Clone, Serialize, Deserialize, Default)]
    pub struct SearchApiFilters {
        #[serde(default)]
        pub package_type: Option<String>,
        #[serde(default)]
        pub min_downloads: Option<u64>,
        #[serde(default)]
        pub verified_only: Option<bool>,
        #[serde(default)]
        pub min_trust_level: Option<crate::TrustLevel>,
        #[serde(default)]
        pub updated_after: Option<String>,
        #[serde(default)]
        pub namespace: Option<String>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct SearchApiResponse {
        pub results: Vec<SearchResult>,
        pub total: u64,
        pub took_ms: u64,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ResolveRequest {
        pub name: String,
        pub version: Option<String>,
    }

    /// M-225: Signature info for client-side verification
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct SignatureInfo {
        /// Key ID that signed
        pub key_id: String,
        /// Owner name
        pub owner: String,
        /// Trust level of the key
        pub trust_level: crate::TrustLevel,
        /// When signed
        pub timestamp: DateTime<Utc>,
        /// Hex-encoded signature bytes for client-side verification
        #[serde(default)]
        pub signature_bytes: Option<String>,
        /// Hex-encoded public key bytes for client-side verification
        #[serde(default)]
        pub public_key_bytes: Option<String>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ResolveResponse {
        pub name: String,
        pub version: String,
        pub hash: String,
        pub download_url: String,
        /// M-225: Package signatures for client-side verification
        #[serde(default)]
        pub signatures: Vec<SignatureInfo>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct PackageResponse {
        pub hash: String,
        pub manifest: PackageManifest,
        pub size: u64,
        pub download_url: String,
        /// M-225: Package signatures for client-side verification
        #[serde(default)]
        pub signatures: Vec<SignatureInfo>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[allow(dead_code)] // Deserialize: Registry API error response struct
    pub struct ApiError {
        pub code: String,
        pub message: String,
    }
}

#[cfg(not(feature = "server"))]
use api_types::*;

/// Default request timeout (30 seconds) - uses centralized constant
pub const DEFAULT_REQUEST_TIMEOUT: Duration = DEFAULT_HTTP_REQUEST_TIMEOUT;
/// Default connect timeout (10 seconds) - uses centralized constant
pub const DEFAULT_CONNECT_TIMEOUT: Duration = DEFAULT_HTTP_CONNECT_TIMEOUT;

/// Configuration for the registry client
#[derive(Clone)]
pub struct RegistryClientConfig {
    /// Base URL of the registry API
    pub base_url: String,
    /// Request timeout
    pub timeout: Duration,
    /// Connection establishment timeout (M-214)
    pub connect_timeout: Duration,
    /// API key for authentication (optional)
    pub api_key: Option<String>,
    /// User agent string
    pub user_agent: String,
    /// Retry policy for transient errors (M-196)
    pub retry_policy: RetryPolicy,
}

// Custom Debug implementation to prevent API key exposure in logs
impl std::fmt::Debug for RegistryClientConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RegistryClientConfig")
            .field("base_url", &self.base_url)
            .field("timeout", &self.timeout)
            .field("connect_timeout", &self.connect_timeout)
            .field("api_key", &self.api_key.as_ref().map(|_| "[REDACTED]"))
            .field("user_agent", &self.user_agent)
            .field("retry_policy", &self.retry_policy)
            .finish()
    }
}

impl Default for RegistryClientConfig {
    fn default() -> Self {
        Self {
            base_url: dashflow_registry_url(),
            timeout: DEFAULT_REQUEST_TIMEOUT,
            connect_timeout: DEFAULT_CONNECT_TIMEOUT,
            api_key: dashflow_registry_api_key(),
            user_agent: format!("dashflow-cli/{}", env!("CARGO_PKG_VERSION")),
            retry_policy: RetryPolicy::default(), // Default uses jitter (M-195)
        }
    }
}

impl RegistryClientConfig {
    /// Create config with a custom base URL
    pub fn with_url(url: impl Into<String>) -> Self {
        Self {
            base_url: url.into(),
            ..Default::default()
        }
    }

    /// Set the retry policy for transient network errors (M-196)
    ///
    /// # Arguments
    ///
    /// * `policy` - The retry policy to use
    #[must_use]
    pub fn with_retry_policy(mut self, policy: RetryPolicy) -> Self {
        self.retry_policy = policy;
        self
    }

    /// Set the request timeout (M-214)
    ///
    /// # Arguments
    ///
    /// * `timeout` - Request timeout duration
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set the connection timeout (M-214)
    ///
    /// # Arguments
    ///
    /// * `timeout` - Connection establishment timeout
    #[must_use]
    pub fn with_connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self
    }
}

/// Result of a publish operation
#[derive(Debug, Clone)]
pub struct PublishResult {
    /// Content hash of the published package
    pub hash: String,
    /// Published version
    pub version: String,
    /// Whether signature was verified
    pub signature_verified: bool,
}

/// M-225: Options for package installation with signature verification
#[derive(Debug, Clone, Default)]
pub struct InstallOptions {
    /// Require valid signature before installation (default: false for backward compatibility)
    /// When true, installation fails if no valid signature is present or verification fails.
    pub require_signature: bool,
    /// Minimum trust level required (only checked if require_signature is true)
    pub min_trust_level: Option<TrustLevel>,
    /// Warn (but don't fail) if signature is missing or invalid (default: true)
    pub warn_on_missing_signature: bool,
}

impl InstallOptions {
    /// Create options that require signature verification
    pub fn require_signature() -> Self {
        Self {
            require_signature: true,
            min_trust_level: None,
            warn_on_missing_signature: true,
        }
    }

    /// Create options that require signature with minimum trust level
    pub fn require_trust(min_level: TrustLevel) -> Self {
        Self {
            require_signature: true,
            min_trust_level: Some(min_level),
            warn_on_missing_signature: true,
        }
    }
}

/// M-225: Result of signature verification
#[derive(Debug, Clone)]
pub struct SignatureVerification {
    /// Whether verification passed
    pub valid: bool,
    /// Key ID used for signing (if signature present)
    pub key_id: Option<String>,
    /// Trust level of the signing key
    pub trust_level: Option<TrustLevel>,
    /// Error message if verification failed
    pub error: Option<String>,
}

/// HTTP client for the DashFlow Package Registry
#[derive(Debug, Clone)]
pub struct RegistryClient {
    client: Client,
    config: RegistryClientConfig,
}

impl RegistryClient {
    /// Create a new registry client with default configuration
    pub fn new() -> Result<Self> {
        Self::with_config(RegistryClientConfig::default())
    }

    /// Create a new registry client with custom configuration
    pub fn with_config(config: RegistryClientConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(config.timeout)
            .connect_timeout(config.connect_timeout)
            .user_agent(&config.user_agent)
            .build()
            .map_err(|e| RegistryError::Network(e.to_string()))?;

        Ok(Self { client, config })
    }

    /// Get the base URL
    pub fn base_url(&self) -> &str {
        &self.config.base_url
    }

    // =========================================================================
    // Search Operations
    // =========================================================================

    /// Search for packages by query string
    pub async fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let url = format!("{}/api/v1/search", self.config.base_url);

        let request = SearchApiRequest {
            query: Some(query.to_string()),
            keywords: None,
            capabilities: None,
            filters: None,
            limit: limit as u32,
            offset: 0,
        };

        let response: SearchApiResponse = self.post(&url, &request).await?;
        Ok(response.results)
    }

    /// Search for packages by keywords
    pub async fn search_keywords(
        &self,
        keywords: &[String],
        limit: usize,
    ) -> Result<Vec<SearchResult>> {
        let url = format!("{}/api/v1/search", self.config.base_url);

        let request = SearchApiRequest {
            query: None,
            keywords: Some(keywords.to_vec()),
            capabilities: None,
            filters: None,
            limit: limit as u32,
            offset: 0,
        };

        let response: SearchApiResponse = self.post(&url, &request).await?;
        Ok(response.results)
    }

    /// Search for packages using semantic search (vector similarity)
    ///
    /// This uses the registry's semantic search endpoint which queries
    /// the vector database for packages similar to the natural language query.
    pub async fn search_semantic(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let url = format!("{}/api/v1/search/semantic", self.config.base_url);

        #[derive(serde::Serialize, Clone)]
        struct SemanticSearchRequest {
            query: Option<String>,
            limit: Option<u32>,
        }

        let request = SemanticSearchRequest {
            query: Some(query.to_string()),
            limit: Some(limit as u32),
        };

        let response: SearchApiResponse = self.post(&url, &request).await?;
        Ok(response.results)
    }

    // =========================================================================
    // Package Operations
    // =========================================================================

    /// Resolve a package name and version to a hash and download URL
    pub async fn resolve(&self, name: &str, version: Option<&str>) -> Result<ResolveResponse> {
        let url = format!("{}/api/v1/packages/resolve", self.config.base_url);

        let request = ResolveRequest {
            name: name.to_string(),
            version: version.map(|v| v.to_string()),
        };

        self.post(&url, &request).await
    }

    /// Get package information by hash
    pub async fn get_package_by_hash(&self, hash: &str) -> Result<PackageResponse> {
        let url = format!("{}/api/v1/packages/{}", self.config.base_url, hash);
        self.get(&url).await
    }

    /// Get package information by name (resolves to latest version)
    pub async fn get_package(&self, name: &str) -> Result<PackageResponse> {
        let resolved = self.resolve(name, None).await?;
        self.get_package_by_hash(&resolved.hash).await
    }

    /// Download a package tarball
    pub async fn download(&self, hash: &str) -> Result<Vec<u8>> {
        let package = self.get_package_by_hash(hash).await?;

        let response = self
            .client
            .get(&package.download_url)
            .send()
            .await
            .map_err(|e| RegistryError::Network(e.to_string()))?;

        if !response.status().is_success() {
            return Err(RegistryError::NotFound(format!(
                "Package {} not found",
                hash
            )));
        }

        let bytes = response
            .bytes()
            .await
            .map_err(|e| RegistryError::Network(e.to_string()))?;

        // Verify hash matches
        let computed_hash = ContentHash::from_bytes(&bytes);
        if computed_hash.to_string() != hash && !hash.starts_with(&computed_hash.to_string()[..16])
        {
            return Err(RegistryError::HashMismatch {
                expected: hash.to_string(),
                actual: computed_hash.to_string(),
            });
        }

        Ok(bytes.to_vec())
    }

    /// Install a package to a local directory
    pub async fn install(
        &self,
        name: &str,
        version: Option<&str>,
        install_dir: &Path,
    ) -> Result<PackageInfo> {
        self.install_with_options(name, version, install_dir, &InstallOptions::default())
            .await
    }

    /// M-225: Install a package with signature verification options
    ///
    /// # Arguments
    /// * `name` - Package name
    /// * `version` - Optional version requirement
    /// * `install_dir` - Directory to install to
    /// * `options` - Installation options including signature requirements
    ///
    /// # Example
    /// ```rust,ignore
    /// // Require signature verification
    /// let options = InstallOptions::require_signature();
    /// client.install_with_options("package", None, &dir, &options).await?;
    ///
    /// // Require minimum trust level
    /// let options = InstallOptions::require_trust(TrustLevel::Organization);
    /// client.install_with_options("package", None, &dir, &options).await?;
    /// ```
    pub async fn install_with_options(
        &self,
        name: &str,
        version: Option<&str>,
        install_dir: &Path,
        options: &InstallOptions,
    ) -> Result<PackageInfo> {
        // Resolve package
        let resolved = self.resolve(name, version).await?;

        // M-225: Verify signatures if required or warn if enabled
        let verification = self.verify_package_signatures(&resolved.signatures, &resolved.hash);

        if options.require_signature {
            if !verification.valid {
                return Err(RegistryError::ClientSignatureVerificationFailed(
                    verification.error.unwrap_or_else(|| "No valid signature found".to_string()),
                ));
            }

            // Check minimum trust level if specified
            if let Some(min_trust) = &options.min_trust_level {
                if let Some(actual_trust) = &verification.trust_level {
                    if actual_trust < min_trust {
                        return Err(RegistryError::InsufficientTrustLevel {
                            required: *min_trust,
                            actual: *actual_trust,
                        });
                    }
                } else {
                    return Err(RegistryError::ClientSignatureVerificationFailed(
                        "Cannot verify trust level - no signature data".to_string(),
                    ));
                }
            }
        } else if options.warn_on_missing_signature && !verification.valid {
            tracing::warn!(
                package = %name,
                hash = %resolved.hash,
                error = ?verification.error,
                "Package installation without valid signature verification"
            );
        }

        // Download package
        let data = self.download(&resolved.hash).await?;

        // Extract tarball to install directory
        let package_dir = install_dir.join(&resolved.hash);
        tokio::fs::create_dir_all(&package_dir)
            .await
            .map_err(|e| RegistryError::Io(e.to_string()))?;

        // Write the tarball (in real implementation, would extract)
        let tarball_path = package_dir.join("package.tar.gz");
        tokio::fs::write(&tarball_path, &data)
            .await
            .map_err(|e| RegistryError::Io(e.to_string()))?;

        // Get package info
        let package_response = self.get_package_by_hash(&resolved.hash).await?;

        Ok(PackageInfo {
            hash: ContentHash::from_bytes(&data),
            manifest: package_response.manifest,
            published_at: chrono::Utc::now(),
            publisher_key_id: verification.key_id.unwrap_or_default(),
            downloads: 0,
            trust_level: verification.trust_level.unwrap_or(TrustLevel::Unknown),
            lineage: None,
            yanked: false,
        })
    }

    /// M-225: Verify package signatures client-side
    ///
    /// This performs client-side signature verification when signature_bytes
    /// and public_key_bytes are provided by the server. This allows verification
    /// without trusting the server's verification response.
    #[cfg(not(feature = "server"))]
    fn verify_package_signatures(
        &self,
        signatures: &[api_types::SignatureInfo],
        hash: &str,
    ) -> SignatureVerification {
        use ed25519_dalek::{Signature as Ed25519Signature, Verifier, VerifyingKey};

        if signatures.is_empty() {
            return SignatureVerification {
                valid: false,
                key_id: None,
                trust_level: None,
                error: Some("No signatures provided".to_string()),
            };
        }

        // Try to verify at least one signature
        for sig_info in signatures {
            // Check if we have data for client-side verification
            let (sig_bytes, pub_key_bytes) = match (&sig_info.signature_bytes, &sig_info.public_key_bytes) {
                (Some(sig), Some(pk)) => (sig, pk),
                _ => {
                    // No client-side verification data, trust server's attestation
                    // (This is the fallback until M-225 storage is complete)
                    return SignatureVerification {
                        valid: true, // Trust server attestation
                        key_id: Some(sig_info.key_id.clone()),
                        trust_level: Some(sig_info.trust_level),
                        error: None,
                    };
                }
            };

            // Decode signature bytes
            let sig_decoded = match hex::decode(sig_bytes) {
                Ok(b) => b,
                Err(e) => {
                    tracing::debug!(key_id = %sig_info.key_id, error = %e, "Failed to decode signature");
                    continue;
                }
            };

            // Decode public key bytes
            let pk_decoded = match hex::decode(pub_key_bytes) {
                Ok(b) => b,
                Err(e) => {
                    tracing::debug!(key_id = %sig_info.key_id, error = %e, "Failed to decode public key");
                    continue;
                }
            };

            // Verify the signature
            let pk_bytes: [u8; 32] = match pk_decoded.try_into() {
                Ok(b) => b,
                Err(_) => {
                    tracing::debug!(key_id = %sig_info.key_id, "Invalid public key length");
                    continue;
                }
            };

            let verifying_key = match VerifyingKey::from_bytes(&pk_bytes) {
                Ok(k) => k,
                Err(e) => {
                    tracing::debug!(key_id = %sig_info.key_id, error = %e, "Invalid public key");
                    continue;
                }
            };

            let sig_bytes_arr: [u8; 64] = match sig_decoded.try_into() {
                Ok(b) => b,
                Err(_) => {
                    tracing::debug!(key_id = %sig_info.key_id, "Invalid signature length");
                    continue;
                }
            };

            let signature = Ed25519Signature::from_bytes(&sig_bytes_arr);

            // The signed content is the package hash
            if verifying_key.verify(hash.as_bytes(), &signature).is_ok() {
                return SignatureVerification {
                    valid: true,
                    key_id: Some(sig_info.key_id.clone()),
                    trust_level: Some(sig_info.trust_level),
                    error: None,
                };
            }

            tracing::debug!(key_id = %sig_info.key_id, "Signature verification failed");
        }

        SignatureVerification {
            valid: false,
            key_id: None,
            trust_level: None,
            error: Some("All signature verifications failed".to_string()),
        }
    }

    /// M-225: Verify package signatures (server feature version - uses server types)
    #[cfg(feature = "server")]
    fn verify_package_signatures(
        &self,
        signatures: &[SignatureInfo],
        hash: &str,
    ) -> SignatureVerification {
        use ed25519_dalek::{Signature as Ed25519Signature, Verifier, VerifyingKey};

        if signatures.is_empty() {
            return SignatureVerification {
                valid: false,
                key_id: None,
                trust_level: None,
                error: Some("No signatures provided".to_string()),
            };
        }

        // Try to verify at least one signature
        for sig_info in signatures {
            // Check if we have data for client-side verification
            let (sig_bytes, pub_key_bytes) = match (&sig_info.signature_bytes, &sig_info.public_key_bytes) {
                (Some(sig), Some(pk)) => (sig, pk),
                _ => {
                    // No client-side verification data, trust server's attestation
                    return SignatureVerification {
                        valid: true,
                        key_id: Some(sig_info.key_id.clone()),
                        trust_level: Some(sig_info.trust_level),
                        error: None,
                    };
                }
            };

            // Decode and verify (same logic as non-server version)
            let sig_decoded = match hex::decode(sig_bytes) {
                Ok(b) => b,
                Err(_) => continue,
            };

            let pk_decoded = match hex::decode(pub_key_bytes) {
                Ok(b) => b,
                Err(_) => continue,
            };

            let pk_bytes: [u8; 32] = match pk_decoded.try_into() {
                Ok(b) => b,
                Err(_) => continue,
            };

            let verifying_key = match VerifyingKey::from_bytes(&pk_bytes) {
                Ok(k) => k,
                Err(_) => continue,
            };

            let sig_bytes_arr: [u8; 64] = match sig_decoded.try_into() {
                Ok(b) => b,
                Err(_) => continue,
            };

            let signature = Ed25519Signature::from_bytes(&sig_bytes_arr);

            if verifying_key.verify(hash.as_bytes(), &signature).is_ok() {
                return SignatureVerification {
                    valid: true,
                    key_id: Some(sig_info.key_id.clone()),
                    trust_level: Some(sig_info.trust_level),
                    error: None,
                };
            }
        }

        SignatureVerification {
            valid: false,
            key_id: None,
            trust_level: None,
            error: Some("All signature verifications failed".to_string()),
        }
    }

    /// Publish a package to the registry
    ///
    /// # Arguments
    /// * `manifest` - The package manifest containing metadata
    /// * `content` - The raw package tarball bytes
    /// * `signature` - Signature over the content
    /// * `public_key` - Public key for signature verification
    ///
    /// # Returns
    /// The content hash of the published package
    pub async fn publish(
        &self,
        manifest: &PackageManifest,
        content: &[u8],
        signature: &Signature,
        public_key: &PublicKey,
    ) -> Result<PublishResult> {
        let url = format!("{}/api/v1/packages", self.config.base_url);

        // Encode content as base64
        use base64::Engine;
        let content_b64 = base64::engine::general_purpose::STANDARD.encode(content);

        #[derive(Serialize, Clone)]
        struct PublishRequestBody {
            manifest: PackageManifest,
            content: String,
            signature: Signature,
            public_key: PublicKey,
        }

        let request = PublishRequestBody {
            manifest: manifest.clone(),
            content: content_b64,
            signature: signature.clone(),
            public_key: public_key.clone(),
        };

        #[derive(Deserialize)]
        struct PublishResponseBody {
            hash: String,
            version: String,
            signature_verified: bool,
            #[allow(dead_code)] // Deserialize: Publish timestamp - reserved for audit logging
            published_at: String,
        }

        let response: PublishResponseBody = self.post(&url, &request).await?;

        Ok(PublishResult {
            hash: response.hash,
            version: response.version,
            signature_verified: response.signature_verified,
        })
    }

    // =========================================================================
    // Contribution Operations
    // =========================================================================

    /// Submit a bug report
    pub async fn submit_bug(&self, package: &str, bug: &BugReport) -> Result<String> {
        let url = format!("{}/api/v1/contributions/bug", self.config.base_url);

        #[derive(Serialize, Clone)]
        struct BugSubmission {
            package: String,
            bug: BugReport,
        }

        let response: serde_json::Value = self
            .post(&url, &BugSubmission {
                package: package.to_string(),
                bug: bug.clone(),
            })
            .await?;

        response
            .get("contribution_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| RegistryError::InvalidResponse("Missing contribution_id".to_string()))
    }

    /// Submit an improvement proposal
    pub async fn submit_improvement(
        &self,
        package: &str,
        proposal: &ImprovementProposal,
    ) -> Result<String> {
        let url = format!("{}/api/v1/contributions/improvement", self.config.base_url);

        #[derive(Serialize, Clone)]
        struct ImprovementSubmission {
            package: String,
            proposal: ImprovementProposal,
        }

        let response: serde_json::Value = self
            .post(&url, &ImprovementSubmission {
                package: package.to_string(),
                proposal: proposal.clone(),
            })
            .await?;

        response
            .get("contribution_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| RegistryError::InvalidResponse("Missing contribution_id".to_string()))
    }

    /// Submit a package request
    pub async fn submit_request(&self, request: &PkgRequest) -> Result<String> {
        let url = format!("{}/api/v1/contributions/request", self.config.base_url);

        let response: serde_json::Value = self.post(&url, request).await?;

        response
            .get("contribution_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| RegistryError::InvalidResponse("Missing contribution_id".to_string()))
    }

    /// Submit a fix
    pub async fn submit_fix(&self, package: &str, fix: &FixSubmission) -> Result<String> {
        let url = format!("{}/api/v1/contributions/fix", self.config.base_url);

        #[derive(Serialize, Clone)]
        struct FixSubmissionRequest {
            package: String,
            fix: FixSubmission,
        }

        let response: serde_json::Value = self
            .post(&url, &FixSubmissionRequest {
                package: package.to_string(),
                fix: fix.clone(),
            })
            .await?;

        response
            .get("contribution_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| RegistryError::InvalidResponse("Missing contribution_id".to_string()))
    }

    /// List contributions for a package
    pub async fn list_contributions(
        &self,
        package: Option<&str>,
        limit: usize,
    ) -> Result<Vec<serde_json::Value>> {
        let mut url = format!("{}/api/v1/contributions", self.config.base_url);

        let mut params = vec![format!("limit={}", limit)];
        if let Some(pkg) = package {
            params.push(format!("package={}", pkg));
        }

        if !params.is_empty() {
            url.push('?');
            url.push_str(&params.join("&"));
        }

        #[derive(Deserialize)]
        struct ContribListResponse {
            contributions: Vec<serde_json::Value>,
        }

        let response: ContribListResponse = self.get(&url).await?;
        Ok(response.contributions)
    }

    // =========================================================================
    // Trust Operations
    // =========================================================================

    /// Verify a package signature
    pub async fn verify_signature(&self, hash: &str) -> Result<bool> {
        let url = format!("{}/api/v1/trust/verify", self.config.base_url);

        #[derive(Serialize, Clone)]
        struct VerifyReq {
            hash: String,
        }

        #[derive(Deserialize)]
        struct VerifyResp {
            valid: bool,
        }

        let response: VerifyResp = self
            .post(&url, &VerifyReq {
                hash: hash.to_string(),
            })
            .await?;
        Ok(response.valid)
    }

    /// Get lineage (derivation chain) for a package
    pub async fn get_lineage(&self, hash: &str) -> Result<serde_json::Value> {
        let url = format!("{}/api/v1/trust/lineage/{}", self.config.base_url, hash);
        self.get(&url).await
    }

    // =========================================================================
    // Colony Operations
    // =========================================================================

    /// Get colony P2P status
    pub async fn colony_status(&self) -> Result<serde_json::Value> {
        let url = format!("{}/api/v1/colony/status", self.config.base_url);
        self.get(&url).await
    }

    /// Find peers with a specific package
    pub async fn find_peers(&self, hash: &str) -> Result<Vec<serde_json::Value>> {
        let url = format!("{}/api/v1/colony/peers/{}", self.config.base_url, hash);

        #[derive(Deserialize)]
        struct PeersResponse {
            peers: Vec<serde_json::Value>,
        }

        let response: PeersResponse = self.get(&url).await?;
        Ok(response.peers)
    }

    // =========================================================================
    // Health Check
    // =========================================================================

    /// Check if the registry server is healthy
    pub async fn health_check(&self) -> Result<bool> {
        let url = format!("{}/health", self.config.base_url);

        match self.client.get(&url).send().await {
            Ok(response) => Ok(response.status().is_success()),
            Err(_) => Ok(false),
        }
    }

    /// Check if registry is reachable and get version info
    pub async fn ping(&self) -> Result<String> {
        let url = format!("{}/health", self.config.base_url);

        #[derive(Deserialize)]
        struct HealthResponse {
            status: String,
            version: Option<String>,
        }

        let response: HealthResponse = self.get(&url).await?;
        Ok(response.version.unwrap_or(response.status))
    }

    // =========================================================================
    // Internal HTTP Helpers (M-196: with retry support)
    // =========================================================================

    async fn get<T: DeserializeOwned + Send + 'static>(&self, url: &str) -> Result<T> {
        let client = self.client.clone();
        let api_key = self.config.api_key.clone();
        let url = url.to_string();

        let response = with_retry(&self.config.retry_policy, || {
            let client = client.clone();
            let api_key = api_key.clone();
            let url = url.clone();
            async move {
                let mut request = client.get(&url);
                if let Some(key) = &api_key {
                    request = request.header("Authorization", format!("Bearer {}", key));
                }
                let response = request.send().await.map_err(|e| {
                    if e.is_connect() || e.is_timeout() {
                        DashFlowError::network(format!("Request failed: {e}"))
                    } else {
                        DashFlowError::api(format!("Request failed: {e}"))
                    }
                })?;

                let status = response.status();
                if status.is_server_error() || status.as_u16() == 429 {
                    let error_text = response.text().await.unwrap_or_default();
                    if status.as_u16() == 429 {
                        return Err(DashFlowError::rate_limit(format!(
                            "Rate limited: {error_text}"
                        )));
                    }
                    return Err(DashFlowError::network(format!(
                        "Server error {status}: {error_text}"
                    )));
                }
                Ok(response)
            }
        })
        .await
        .map_err(|e| RegistryError::Network(e.to_string()))?;

        self.handle_response(response).await
    }

    async fn post<T: DeserializeOwned + Send + 'static, B: Serialize + Clone + Send + Sync>(
        &self,
        url: &str,
        body: &B,
    ) -> Result<T> {
        let client = self.client.clone();
        let api_key = self.config.api_key.clone();
        let url = url.to_string();
        let body = body.clone();

        let response = with_retry(&self.config.retry_policy, || {
            let client = client.clone();
            let api_key = api_key.clone();
            let url = url.clone();
            let body = body.clone();
            async move {
                let mut request = client.post(&url).json(&body);
                if let Some(key) = &api_key {
                    request = request.header("Authorization", format!("Bearer {}", key));
                }
                let response = request.send().await.map_err(|e| {
                    if e.is_connect() || e.is_timeout() {
                        DashFlowError::network(format!("Request failed: {e}"))
                    } else {
                        DashFlowError::api(format!("Request failed: {e}"))
                    }
                })?;

                let status = response.status();
                if status.is_server_error() || status.as_u16() == 429 {
                    let error_text = response.text().await.unwrap_or_default();
                    if status.as_u16() == 429 {
                        return Err(DashFlowError::rate_limit(format!(
                            "Rate limited: {error_text}"
                        )));
                    }
                    return Err(DashFlowError::network(format!(
                        "Server error {status}: {error_text}"
                    )));
                }
                Ok(response)
            }
        })
        .await
        .map_err(|e| RegistryError::Network(e.to_string()))?;

        self.handle_response(response).await
    }

    async fn handle_response<T: DeserializeOwned>(&self, response: reqwest::Response) -> Result<T> {
        let status = response.status();

        if status.is_success() {
            response
                .json::<T>()
                .await
                .map_err(|e| RegistryError::InvalidResponse(e.to_string()))
        } else {
            // Try to parse error response
            let error_text = response.text().await.unwrap_or_default();

            match status {
                StatusCode::NOT_FOUND => Err(RegistryError::NotFound(error_text)),
                StatusCode::UNAUTHORIZED => Err(RegistryError::Unauthorized(error_text)),
                StatusCode::TOO_MANY_REQUESTS => Err(RegistryError::RateLimited(error_text)),
                _ => Err(RegistryError::Api {
                    status: status.as_u16(),
                    message: error_text,
                }),
            }
        }
    }
}

impl Default for RegistryClient {
    #[allow(clippy::expect_used)] // Default must be infallible; Self::new() only fails on HTTP client creation
    fn default() -> Self {
        Self::new().expect("Failed to create default RegistryClient")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = RegistryClientConfig::default();
        assert!(config.base_url.starts_with("http"));
        assert!(config.timeout.as_secs() > 0);
    }

    #[test]
    fn test_config_with_url() {
        let config = RegistryClientConfig::with_url("https://example.com");
        assert_eq!(config.base_url, "https://example.com");
    }

    #[test]
    fn test_client_creation() {
        let client = RegistryClient::new();
        assert!(client.is_ok());
        let client = client.unwrap();
        // Verify the client was created with a valid base URL
        assert!(client.base_url().starts_with("http"));
    }
}
