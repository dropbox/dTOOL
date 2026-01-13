// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! HTTP registry client for central and third-party package registries.
//!
//! This module implements the client for interacting with HTTP-based package
//! registries like dashswarm.com. It supports:
//!
//! - Package search (text and semantic)
//! - Package info retrieval
//! - Package download
//! - Version listing
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow::packages::{RegistryClient, RegistryClientConfig};
//!
//! // Create a client for the central registry
//! let client = RegistryClient::new(RegistryClientConfig::default())?;
//!
//! // Search for packages
//! let results = client.search("sentiment analysis").await?;
//!
//! // Get package info
//! let info = client.get_package("dashflow/sentiment-analysis").await?;
//!
//! // Download a package
//! let data = client.download("dashflow/sentiment-analysis", "1.2.0").await?;
//! ```

use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;

use super::dashswarm::DASHSWARM_DEFAULT_URL;
use super::manifest::PackageManifest;
use super::types::{HashAlgorithm, PackageId, PackageType, TrustLevel, Version, VersionReq};

/// Result type for client operations.
pub type ClientResult<T> = Result<T, ClientError>;

/// Errors that can occur during registry client operations.
#[derive(Debug, Clone, Error)]
#[non_exhaustive]
pub enum ClientError {
    /// Network error
    #[error("Network error: {0}")]
    Network(String),
    /// Request failed with status code.
    #[error("HTTP {status} error: {message}")]
    HttpStatus {
        /// HTTP status code.
        status: u16,
        /// Error message from server.
        message: String,
    },
    /// Response parse error
    #[error("Parse error: {0}")]
    ParseError(String),
    /// Package not found
    #[error("Package not found: {0}")]
    NotFound(String),
    /// Version not found.
    #[error("Version {version} not found for package {package}")]
    VersionNotFound {
        /// Package name.
        package: String,
        /// Version that was not found.
        version: String,
    },
    /// Authentication required
    #[error("Authentication required")]
    AuthRequired,
    /// Invalid configuration
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
    /// Rate limited.
    #[error("Rate limited{}", retry_after.map(|d| format!(", retry after {:?}", d)).unwrap_or_default())]
    RateLimited {
        /// Suggested wait duration before retrying.
        retry_after: Option<Duration>,
    },
    /// Timeout
    #[error("Request timed out")]
    Timeout,
    /// IO error
    #[error("IO error: {0}")]
    Io(String),
}

/// Configuration for the registry client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryClientConfig {
    /// Base URL of the registry API
    pub base_url: String,
    /// Whether this is an official registry
    pub official: bool,
    /// HTTP authentication (if required)
    pub auth: Option<HttpAuth>,
    /// Request timeout
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
    /// User agent string
    #[serde(default = "default_user_agent")]
    pub user_agent: String,
    /// Maximum retries for failed requests
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
}

fn default_timeout() -> u64 {
    30
}

fn default_user_agent() -> String {
    format!("DashFlow/{}", env!("CARGO_PKG_VERSION"))
}

fn default_max_retries() -> u32 {
    3
}

impl Default for RegistryClientConfig {
    fn default() -> Self {
        Self {
            base_url: DASHSWARM_DEFAULT_URL.to_string(),
            official: true,
            auth: None,
            timeout_secs: default_timeout(),
            user_agent: default_user_agent(),
            max_retries: default_max_retries(),
        }
    }
}

impl RegistryClientConfig {
    /// Create a new configuration with custom base URL.
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            official: false,
            ..Default::default()
        }
    }

    /// Create a configuration for the official registry.
    pub fn official() -> Self {
        Self::default()
    }

    /// Set authentication.
    #[must_use]
    pub fn with_auth(mut self, auth: HttpAuth) -> Self {
        self.auth = Some(auth);
        self
    }

    /// Set timeout.
    #[must_use]
    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }
}

/// HTTP authentication methods.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HttpAuth {
    /// Bearer token authentication (OAuth2, JWT)
    Bearer {
        /// The bearer token value
        token: String,
    },
    /// Basic authentication (RFC 7617)
    Basic {
        /// Username for basic auth
        username: String,
        /// Password for basic auth
        password: String,
    },
    /// API key authentication (custom header)
    ApiKey {
        /// The API key value
        key: String,
        /// Header name to use (e.g., "X-API-Key")
        header: String,
    },
}

/// HTTP-based registry client.
///
/// Provides methods for interacting with HTTP package registries.
pub struct RegistryClient {
    config: RegistryClientConfig,
    client: reqwest::blocking::Client,
}

impl RegistryClient {
    /// Create a new registry client.
    pub fn new(config: RegistryClientConfig) -> ClientResult<Self> {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::USER_AGENT,
            config
                .user_agent
                .parse()
                .map_err(|e| ClientError::InvalidConfig(format!("Invalid user agent: {}", e)))?,
        );

        // Add authentication headers if configured
        if let Some(auth) = &config.auth {
            match auth {
                HttpAuth::Bearer { token } => {
                    headers.insert(
                        reqwest::header::AUTHORIZATION,
                        format!("Bearer {}", token).parse().map_err(|e| {
                            ClientError::InvalidConfig(format!("Invalid token: {}", e))
                        })?,
                    );
                }
                HttpAuth::ApiKey { key, header } => {
                    headers.insert(
                        reqwest::header::HeaderName::try_from(header.as_str()).map_err(|e| {
                            ClientError::InvalidConfig(format!("Invalid header: {}", e))
                        })?,
                        key.parse().map_err(|e| {
                            ClientError::InvalidConfig(format!("Invalid API key: {}", e))
                        })?,
                    );
                }
                HttpAuth::Basic { username, password } => {
                    let credentials = base64::Engine::encode(
                        &base64::engine::general_purpose::STANDARD,
                        format!("{}:{}", username, password),
                    );
                    headers.insert(
                        reqwest::header::AUTHORIZATION,
                        format!("Basic {}", credentials).parse().map_err(|e| {
                            ClientError::InvalidConfig(format!("Invalid credentials: {}", e))
                        })?,
                    );
                }
            }
        }

        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs))
            .connect_timeout(Duration::from_secs(10))
            .default_headers(headers)
            .build()
            .map_err(|e| ClientError::Network(format!("Failed to build client: {}", e)))?;

        Ok(Self { config, client })
    }

    /// Create a client for the official registry.
    pub fn official() -> ClientResult<Self> {
        Self::new(RegistryClientConfig::official())
    }

    /// Get the base URL of this registry.
    pub fn base_url(&self) -> &str {
        &self.config.base_url
    }

    /// Check if this is an official registry.
    pub fn is_official(&self) -> bool {
        self.config.official
    }

    /// Build a URL for an API endpoint.
    fn api_url(&self, path: &str) -> String {
        format!(
            "{}/api/v1{}",
            self.config.base_url.trim_end_matches('/'),
            path
        )
    }

    /// Search for packages by text query.
    ///
    /// Searches package names, descriptions, and keywords.
    pub fn search(&self, query: &str) -> ClientResult<Vec<PackageSearchResult>> {
        let url = self.api_url(&format!("/search?q={}", urlencoding::encode(query)));
        self.get_json(&url)
    }

    /// Search for packages with filters.
    pub fn search_with_options(
        &self,
        options: &SearchOptions,
    ) -> ClientResult<Vec<PackageSearchResult>> {
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
        self.get_json(&url)
    }

    /// Search for packages using semantic similarity.
    ///
    /// Uses embeddings to find packages with similar functionality.
    pub fn search_semantic(&self, query: &str) -> ClientResult<Vec<SemanticSearchResult>> {
        let url = self.api_url("/search/semantic");
        let request = SemanticSearchRequest {
            query: query.to_string(),
            limit: 20,
        };
        self.post_json(&url, &request)
    }

    /// Browse packages by category.
    pub fn browse_category(&self, category: &str) -> ClientResult<Vec<PackageSearchResult>> {
        let url = self.api_url(&format!("/browse/{}", urlencoding::encode(category)));
        self.get_json(&url)
    }

    /// Get information about a package.
    pub fn get_package(&self, id: &str) -> ClientResult<PackageInfo> {
        let package_id = PackageId::parse(id)
            .ok_or_else(|| ClientError::InvalidConfig(format!("Invalid package ID: {}", id)))?;

        let url = self.api_url(&format!(
            "/packages/{}/{}",
            package_id.namespace(),
            package_id.name()
        ));
        self.get_json(&url)
    }

    /// List all versions of a package.
    pub fn list_versions(&self, id: &str) -> ClientResult<Vec<VersionInfo>> {
        let package_id = PackageId::parse(id)
            .ok_or_else(|| ClientError::InvalidConfig(format!("Invalid package ID: {}", id)))?;

        let url = self.api_url(&format!(
            "/packages/{}/{}/versions",
            package_id.namespace(),
            package_id.name()
        ));
        self.get_json(&url)
    }

    /// Get information about a specific version.
    pub fn get_version(&self, id: &str, version: &str) -> ClientResult<PackageVersionInfo> {
        let package_id = PackageId::parse(id)
            .ok_or_else(|| ClientError::InvalidConfig(format!("Invalid package ID: {}", id)))?;

        let url = self.api_url(&format!(
            "/packages/{}/{}/{}",
            package_id.namespace(),
            package_id.name(),
            version
        ));
        self.get_json(&url)
    }

    /// Download a package tarball.
    pub fn download(&self, id: &str, version: &str) -> ClientResult<PackageDownload> {
        let package_id = PackageId::parse(id)
            .ok_or_else(|| ClientError::InvalidConfig(format!("Invalid package ID: {}", id)))?;

        let url = self.api_url(&format!(
            "/packages/{}/{}/{}/download",
            package_id.namespace(),
            package_id.name(),
            version
        ));

        let response = self.client.get(&url).send().map_err(|e| {
            if e.is_timeout() {
                ClientError::Timeout
            } else {
                ClientError::Network(e.to_string())
            }
        })?;

        let status = response.status();
        if !status.is_success() {
            return Err(self.handle_error_status(status.as_u16(), &url));
        }

        // Get content hash from headers if available
        let hash = response
            .headers()
            .get("X-Content-Hash")
            .and_then(|v| v.to_str().ok())
            .map(String::from);

        let hash_algorithm = response
            .headers()
            .get("X-Hash-Algorithm")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| match s.to_lowercase().as_str() {
                "sha256" => Some(HashAlgorithm::Sha256),
                "sha384" => Some(HashAlgorithm::Sha384),
                "sha512" => Some(HashAlgorithm::Sha512),
                "blake3" => Some(HashAlgorithm::Blake3),
                _ => None,
            });

        let content_length = response.content_length();
        let data = response
            .bytes()
            .map_err(|e| ClientError::Network(format!("Failed to read response body: {}", e)))?
            .to_vec();

        Ok(PackageDownload {
            id: package_id,
            version: Version::parse(version).ok_or_else(|| {
                ClientError::InvalidConfig(format!("Invalid version: {}", version))
            })?,
            data,
            size_bytes: content_length.unwrap_or(0),
            hash,
            hash_algorithm,
        })
    }

    /// Download a package to a file.
    pub fn download_to_file(
        &self,
        id: &str,
        version: &str,
        path: &std::path::Path,
    ) -> ClientResult<()> {
        let download = self.download(id, version)?;
        std::fs::write(path, &download.data)
            .map_err(|e| ClientError::Io(format!("Failed to write file: {}", e)))?;
        Ok(())
    }

    /// Find the best version matching a requirement.
    pub fn find_matching_version(
        &self,
        id: &str,
        req: &VersionReq,
    ) -> ClientResult<Option<Version>> {
        let versions = self.list_versions(id)?;
        let matching: Vec<_> = versions
            .into_iter()
            .filter(|v| req.matches(&v.version))
            .collect();
        Ok(matching.into_iter().map(|v| v.version).max())
    }

    /// Get security advisories for a package.
    pub fn get_advisories(&self, id: &str) -> ClientResult<Vec<SecurityAdvisory>> {
        let package_id = PackageId::parse(id)
            .ok_or_else(|| ClientError::InvalidConfig(format!("Invalid package ID: {}", id)))?;

        let url = self.api_url(&format!(
            "/packages/{}/{}/advisories",
            package_id.namespace(),
            package_id.name()
        ));
        self.get_json(&url)
    }

    /// Make a GET request and parse JSON response.
    fn get_json<T: serde::de::DeserializeOwned>(&self, url: &str) -> ClientResult<T> {
        let response = self.client.get(url).send().map_err(|e| {
            if e.is_timeout() {
                ClientError::Timeout
            } else {
                ClientError::Network(e.to_string())
            }
        })?;

        let status = response.status();
        if !status.is_success() {
            return Err(self.handle_error_status(status.as_u16(), url));
        }

        response
            .json()
            .map_err(|e| ClientError::ParseError(format!("Failed to parse JSON: {}", e)))
    }

    /// Make a POST request with JSON body and parse JSON response.
    fn post_json<B: serde::Serialize, T: serde::de::DeserializeOwned>(
        &self,
        url: &str,
        body: &B,
    ) -> ClientResult<T> {
        let response = self.client.post(url).json(body).send().map_err(|e| {
            if e.is_timeout() {
                ClientError::Timeout
            } else {
                ClientError::Network(e.to_string())
            }
        })?;

        let status = response.status();
        if !status.is_success() {
            return Err(self.handle_error_status(status.as_u16(), url));
        }

        response
            .json()
            .map_err(|e| ClientError::ParseError(format!("Failed to parse JSON: {}", e)))
    }

    /// Handle error HTTP status codes.
    fn handle_error_status(&self, status: u16, url: &str) -> ClientError {
        match status {
            401 => ClientError::AuthRequired,
            404 => {
                // Try to extract package ID from URL for better error message
                if url.contains("/packages/") {
                    let parts: Vec<&str> = url.split("/packages/").collect();
                    if parts.len() > 1 {
                        let pkg_path = parts[1].split('/').take(2).collect::<Vec<_>>().join("/");
                        return ClientError::NotFound(pkg_path);
                    }
                }
                ClientError::NotFound(url.to_string())
            }
            429 => ClientError::RateLimited { retry_after: None },
            _ => ClientError::HttpStatus {
                status,
                message: format!("Request to {} failed", url),
            },
        }
    }
}

/// Search options for filtering results.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SearchOptions {
    /// Text query
    pub query: Option<String>,
    /// Filter by package type
    pub package_type: Option<PackageType>,
    /// Minimum download count
    pub min_downloads: Option<u64>,
    /// Only show verified packages
    #[serde(default)]
    pub verified_only: bool,
    /// Filter by category
    pub category: Option<String>,
    /// Maximum results
    pub limit: Option<usize>,
    /// Offset for pagination
    pub offset: Option<usize>,
    /// Sort order
    pub sort_by: Option<SortOrder>,
}

impl SearchOptions {
    /// Create new search options with a query.
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: Some(query.into()),
            ..Default::default()
        }
    }

    /// Filter by package type.
    #[must_use]
    pub fn with_type(mut self, pkg_type: PackageType) -> Self {
        self.package_type = Some(pkg_type);
        self
    }

    /// Only show verified packages.
    pub fn verified(mut self) -> Self {
        self.verified_only = true;
        self
    }

    /// Filter by category.
    pub fn in_category(mut self, category: impl Into<String>) -> Self {
        self.category = Some(category.into());
        self
    }

    /// Set result limit.
    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Set sort order.
    pub fn sort(mut self, order: SortOrder) -> Self {
        self.sort_by = Some(order);
        self
    }
}

/// Sort order for search results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortOrder {
    /// Sort by relevance (default for searches)
    Relevance,
    /// Sort by download count
    Downloads,
    /// Sort by most recent update
    RecentlyUpdated,
    /// Sort alphabetically by name
    Name,
}

impl SortOrder {
    /// Get string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Relevance => "relevance",
            Self::Downloads => "downloads",
            Self::RecentlyUpdated => "recently_updated",
            Self::Name => "name",
        }
    }
}

/// Package search result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageSearchResult {
    /// Package ID
    pub id: PackageId,
    /// Human-readable name
    pub name: String,
    /// Description
    pub description: String,
    /// Package type
    pub package_type: PackageType,
    /// Latest version
    pub latest_version: Version,
    /// Keywords
    pub keywords: Vec<String>,
    /// Total downloads
    pub downloads: u64,
    /// Trust level
    pub trust_level: TrustLevel,
    /// Is verified
    pub verified: bool,
}

/// Semantic search request.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SemanticSearchRequest {
    query: String,
    limit: usize,
}

/// Semantic search result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticSearchResult {
    /// Package info
    pub package: PackageSearchResult,
    /// Similarity score (0.0 to 1.0)
    pub score: f64,
    /// Highlighted matching parts
    pub highlights: Vec<String>,
}

/// Detailed package information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageInfo {
    /// Package ID
    pub id: PackageId,
    /// Human-readable name
    pub name: String,
    /// Description
    pub description: String,
    /// Full readme (markdown)
    pub readme: Option<String>,
    /// Package type
    pub package_type: PackageType,
    /// All versions
    pub versions: Vec<Version>,
    /// Latest version
    pub latest_version: Version,
    /// Keywords
    pub keywords: Vec<String>,
    /// Categories
    pub categories: Vec<String>,
    /// License
    pub license: String,
    /// Repository URL
    pub repository: Option<String>,
    /// Documentation URL
    pub documentation: Option<String>,
    /// Author
    pub author: String,
    /// Total downloads
    pub downloads: u64,
    /// Trust level
    pub trust_level: TrustLevel,
    /// Is verified
    pub verified: bool,
    /// Creation timestamp
    pub created_at: String,
    /// Last updated timestamp
    pub updated_at: String,
}

/// Version information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionInfo {
    /// The version
    pub version: Version,
    /// Download count for this version
    pub downloads: u64,
    /// Release timestamp
    pub published_at: String,
    /// Is this a prerelease
    pub prerelease: bool,
    /// Is this version deprecated
    pub deprecated: bool,
}

/// Detailed version information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageVersionInfo {
    /// Package ID
    pub id: PackageId,
    /// Version
    pub version: Version,
    /// Full manifest
    pub manifest: PackageManifest,
    /// Download count
    pub downloads: u64,
    /// Package size in bytes
    pub size_bytes: u64,
    /// Content hash
    pub hash: String,
    /// Hash algorithm
    pub hash_algorithm: HashAlgorithm,
    /// Release timestamp
    pub published_at: String,
}

/// Downloaded package data.
#[derive(Debug, Clone)]
pub struct PackageDownload {
    /// Package ID
    pub id: PackageId,
    /// Version
    pub version: Version,
    /// Raw package data (tarball)
    pub data: Vec<u8>,
    /// Size in bytes
    pub size_bytes: u64,
    /// Content hash (if provided by server)
    pub hash: Option<String>,
    /// Hash algorithm (if provided)
    pub hash_algorithm: Option<HashAlgorithm>,
}

impl PackageDownload {
    /// Verify the download against its hash.
    pub fn verify(&self) -> bool {
        if let (Some(expected_hash), Some(algorithm)) = (&self.hash, &self.hash_algorithm) {
            let actual_hash = match algorithm {
                HashAlgorithm::Sha256 => {
                    use sha2::{Digest, Sha256};
                    let mut hasher = Sha256::new();
                    hasher.update(&self.data);
                    hex::encode(hasher.finalize())
                }
                HashAlgorithm::Sha384 => {
                    use sha2::{Digest, Sha384};
                    let mut hasher = Sha384::new();
                    hasher.update(&self.data);
                    hex::encode(hasher.finalize())
                }
                HashAlgorithm::Sha512 => {
                    use sha2::{Digest, Sha512};
                    let mut hasher = Sha512::new();
                    hasher.update(&self.data);
                    hex::encode(hasher.finalize())
                }
                HashAlgorithm::Blake3 => {
                    // Use blake2 as fallback since blake3 crate not available
                    use blake2::{Blake2b512, Digest};
                    let mut hasher = Blake2b512::new();
                    hasher.update(&self.data);
                    hex::encode(hasher.finalize())
                }
            };
            actual_hash.eq_ignore_ascii_case(expected_hash)
        } else {
            // No hash to verify against
            true
        }
    }
}

/// Security advisory for a package.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityAdvisory {
    /// Advisory ID
    pub id: String,
    /// Title
    pub title: String,
    /// Description
    pub description: String,
    /// Severity level
    pub severity: AdvisorySeverity,
    /// Affected versions
    pub affected_versions: Vec<VersionReq>,
    /// Fixed in version (if available)
    pub fixed_in: Option<Version>,
    /// CVE IDs
    pub cve_ids: Vec<String>,
    /// Published timestamp
    pub published_at: String,
}

/// Advisory severity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AdvisorySeverity {
    /// Informational
    Low,
    /// Moderate impact
    Medium,
    /// Significant impact
    High,
    /// Severe/critical
    Critical,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_client_config_default() {
        let config = RegistryClientConfig::default();
        assert_eq!(config.base_url, "https://registry.dashswarm.com");
        assert!(config.official);
        assert!(config.auth.is_none());
        assert_eq!(config.timeout_secs, 30);
    }

    #[test]
    fn test_registry_client_config_new() {
        let config = RegistryClientConfig::new("https://example.com");
        assert_eq!(config.base_url, "https://example.com");
        assert!(!config.official);
    }

    #[test]
    fn test_registry_client_config_with_auth() {
        let config = RegistryClientConfig::default().with_auth(HttpAuth::Bearer {
            token: "test-token".to_string(),
        });
        assert!(config.auth.is_some());
    }

    #[test]
    fn test_search_options_builder() {
        let options = SearchOptions::new("sentiment")
            .with_type(PackageType::NodeLibrary)
            .verified()
            .limit(10)
            .sort(SortOrder::Downloads);

        assert_eq!(options.query, Some("sentiment".to_string()));
        assert_eq!(options.package_type, Some(PackageType::NodeLibrary));
        assert!(options.verified_only);
        assert_eq!(options.limit, Some(10));
        assert_eq!(options.sort_by, Some(SortOrder::Downloads));
    }

    #[test]
    fn test_sort_order_as_str() {
        assert_eq!(SortOrder::Relevance.as_str(), "relevance");
        assert_eq!(SortOrder::Downloads.as_str(), "downloads");
        assert_eq!(SortOrder::RecentlyUpdated.as_str(), "recently_updated");
        assert_eq!(SortOrder::Name.as_str(), "name");
    }

    #[test]
    fn test_client_error_display() {
        let err = ClientError::NotFound("test/pkg".to_string());
        assert!(err.to_string().contains("test/pkg"));

        let err = ClientError::HttpStatus {
            status: 500,
            message: "Internal error".to_string(),
        };
        assert!(err.to_string().contains("500"));

        let err = ClientError::RateLimited {
            retry_after: Some(Duration::from_secs(60)),
        };
        assert!(err.to_string().contains("60"));
    }

    #[test]
    fn test_advisory_severity_ordering() {
        assert!(AdvisorySeverity::Low < AdvisorySeverity::Medium);
        assert!(AdvisorySeverity::Medium < AdvisorySeverity::High);
        assert!(AdvisorySeverity::High < AdvisorySeverity::Critical);
    }

    #[test]
    fn test_package_download_verify_no_hash() {
        let download = PackageDownload {
            id: PackageId::new("test", "pkg"),
            version: Version::new(1, 0, 0),
            data: b"test data".to_vec(),
            size_bytes: 9,
            hash: None,
            hash_algorithm: None,
        };
        // Should return true when no hash to verify
        assert!(download.verify());
    }

    #[test]
    fn test_package_download_verify_sha256() {
        use sha2::{Digest, Sha256};

        let data = b"test data";
        let mut hasher = Sha256::new();
        hasher.update(data);
        let hash = hex::encode(hasher.finalize());

        let download = PackageDownload {
            id: PackageId::new("test", "pkg"),
            version: Version::new(1, 0, 0),
            data: data.to_vec(),
            size_bytes: data.len() as u64,
            hash: Some(hash),
            hash_algorithm: Some(HashAlgorithm::Sha256),
        };
        assert!(download.verify());

        // Test with wrong hash
        let bad_download = PackageDownload {
            hash: Some("wrong_hash".to_string()),
            ..download
        };
        assert!(!bad_download.verify());
    }
}
