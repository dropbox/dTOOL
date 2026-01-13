//! PostgreSQL Metadata Store
//!
//! Stores package metadata, versions, contributions, and provides name resolution.
//! Uses tokio-postgres with PostgreSQL for persistence.

use crate::content_hash::ContentHash;
use crate::contribution::ContributionStatus;
use crate::package::PackageInfo;
use crate::{RegistryError, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Metadata store trait for package registry operations.
#[async_trait]
pub trait MetadataStore: Send + Sync {
    /// Store metadata for a new package version.
    async fn store_package(&self, info: &PackageInfo) -> Result<()>;

    /// Get package info by content hash.
    async fn get_by_hash(&self, hash: &ContentHash) -> Result<Option<PackageInfo>>;

    /// Resolve package name and version requirement to a content hash.
    async fn resolve(&self, name: &str, version_req: &VersionReq) -> Result<Option<Resolution>>;

    /// Resolve package name to latest version.
    async fn resolve_latest(&self, name: &str) -> Result<Option<Resolution>>;

    /// List all versions of a package.
    async fn list_versions(&self, name: &str) -> Result<Vec<VersionInfo>>;

    /// Mark a package version as yanked.
    async fn yank(&self, hash: &ContentHash) -> Result<()>;

    /// Unyank a package version.
    async fn unyank(&self, hash: &ContentHash) -> Result<()>;

    /// Search packages by keyword (basic text search).
    async fn search_keyword(
        &self,
        query: &str,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<PackageInfo>>;

    /// Increment download count.
    async fn increment_downloads(&self, hash: &ContentHash) -> Result<u64>;

    /// Check if a package name exists.
    async fn name_exists(&self, name: &str) -> Result<bool>;
}

// ============================================================================
// Contribution Store
// ============================================================================

/// Stored contribution record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredContribution {
    /// Unique contribution ID.
    pub id: Uuid,
    /// Contribution type: 'bug', 'improvement', 'request', 'fix'.
    pub contribution_type: String,
    /// Package hash (if applicable).
    pub package_hash: Option<String>,
    /// Title.
    pub title: String,
    /// Description.
    pub description: String,
    /// Current status.
    pub status: ContributionStatus,
    /// Reporter public key.
    pub reporter_public_key: String,
    /// Reporter name (optional).
    pub reporter_name: Option<String>,
    /// Reporter app ID (optional).
    pub reporter_app_id: Option<Uuid>,
    /// Is the reporter an AI.
    pub reporter_is_ai: bool,
    /// Type-specific data as JSON.
    pub data: serde_json::Value,
    /// Signature.
    pub signature: String,
    /// When created.
    pub created_at: DateTime<Utc>,
    /// When last updated.
    pub updated_at: DateTime<Utc>,
}

/// Stored review record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredReview {
    /// Unique review ID.
    pub id: Uuid,
    /// Contribution ID this reviews.
    pub contribution_id: Uuid,
    /// Reviewer app ID (optional).
    pub reviewer_app_id: Option<Uuid>,
    /// Reviewer public key.
    pub reviewer_public_key: String,
    /// Reviewer name (optional).
    pub reviewer_name: Option<String>,
    /// Is the reviewer an AI.
    pub reviewer_is_ai: bool,
    /// Verdict: 'approve', 'approve_with_suggestions', 'request_changes', 'reject', 'abstain'.
    pub verdict: String,
    /// Confidence score 0.0-1.0.
    pub confidence: f32,
    /// Justification text.
    pub justification: Option<String>,
    /// Signature.
    pub signature: String,
    /// When created.
    pub created_at: DateTime<Utc>,
}

/// Contribution store trait for contribution operations.
#[async_trait]
pub trait ContributionStore: Send + Sync {
    /// Store a new contribution.
    async fn store_contribution(&self, contribution: &StoredContribution) -> Result<Uuid>;

    /// Get contribution by ID.
    async fn get_contribution(&self, id: Uuid) -> Result<Option<StoredContribution>>;

    /// Update contribution status.
    async fn update_contribution_status(&self, id: Uuid, status: ContributionStatus) -> Result<()>;

    /// List contributions with optional filters.
    async fn list_contributions(
        &self,
        package_hash: Option<&str>,
        status: Option<ContributionStatus>,
        limit: usize,
        offset: usize,
    ) -> Result<(Vec<StoredContribution>, u64)>;

    /// Store a review.
    async fn store_review(&self, review: &StoredReview) -> Result<Uuid>;

    /// Get reviews for a contribution.
    async fn get_reviews(&self, contribution_id: Uuid) -> Result<Vec<StoredReview>>;

    /// Get review consensus for a contribution.
    async fn get_consensus(&self, contribution_id: Uuid) -> Result<Option<ContributionConsensus>>;
}

/// Contribution consensus information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContributionConsensus {
    /// Total reviews count.
    pub total_reviews: u64,
    /// Approve votes count.
    pub approve_count: u64,
    /// Reject votes count.
    pub reject_count: u64,
    /// Average confidence.
    pub avg_confidence: f32,
}

// ============================================================================
// API Key Store
// ============================================================================

/// Trust level for API keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApiKeyTrustLevel {
    /// Basic API key with standard rate limits.
    Basic,
    /// Verified key with higher rate limits.
    Verified,
    /// Trusted/admin key with full access.
    Trusted,
}

impl std::fmt::Display for ApiKeyTrustLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApiKeyTrustLevel::Basic => write!(f, "basic"),
            ApiKeyTrustLevel::Verified => write!(f, "verified"),
            ApiKeyTrustLevel::Trusted => write!(f, "trusted"),
        }
    }
}

impl std::str::FromStr for ApiKeyTrustLevel {
    type Err = RegistryError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "basic" => Ok(ApiKeyTrustLevel::Basic),
            "verified" => Ok(ApiKeyTrustLevel::Verified),
            "trusted" => Ok(ApiKeyTrustLevel::Trusted),
            _ => Err(RegistryError::Validation(format!(
                "Invalid trust level: {}",
                s
            ))),
        }
    }
}

/// Stored API key record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredApiKey {
    /// Unique key ID.
    pub id: Uuid,
    /// The API key hash (we store hash, not the raw key).
    pub key_hash: String,
    /// Key prefix for identification (e.g., "dk_live_abc...").
    pub key_prefix: String,
    /// Owner agent ID (optional).
    pub agent_id: Option<Uuid>,
    /// Owner name/description.
    pub name: String,
    /// Trust level.
    pub trust_level: ApiKeyTrustLevel,
    /// Scopes/permissions (e.g., ["read", "write", "admin"]).
    pub scopes: Vec<String>,
    /// Rate limit override (requests per minute), None uses default.
    pub rate_limit_rpm: Option<u32>,
    /// Is this key active?
    pub active: bool,
    /// When the key expires (None = never).
    pub expires_at: Option<DateTime<Utc>>,
    /// Last time this key was used.
    pub last_used_at: Option<DateTime<Utc>>,
    /// When created.
    pub created_at: DateTime<Utc>,
    /// When last updated.
    pub updated_at: DateTime<Utc>,
}

/// Result of API key verification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyVerification {
    /// The stored key info.
    pub key: StoredApiKey,
    /// Whether the key is currently valid.
    pub valid: bool,
    /// Reason if invalid.
    pub invalid_reason: Option<String>,
}

/// API key store trait for key operations.
#[async_trait]
pub trait ApiKeyStore: Send + Sync {
    /// Store a new API key.
    async fn store_api_key(&self, key: &StoredApiKey) -> Result<Uuid>;

    /// Get API key by hash.
    async fn get_api_key_by_hash(&self, key_hash: &str) -> Result<Option<StoredApiKey>>;

    /// Get API key by ID.
    async fn get_api_key(&self, id: Uuid) -> Result<Option<StoredApiKey>>;

    /// Verify an API key and return verification result.
    async fn verify_api_key(&self, key_hash: &str) -> Result<ApiKeyVerification>;

    /// Update last_used_at timestamp for a key.
    async fn touch_api_key(&self, key_hash: &str) -> Result<()>;

    /// Revoke (deactivate) an API key.
    async fn revoke_api_key(&self, id: Uuid) -> Result<()>;

    /// List API keys for an agent.
    async fn list_api_keys(
        &self,
        agent_id: Option<Uuid>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<StoredApiKey>>;
}

/// Generate a new API key with prefix.
pub fn generate_api_key(prefix: &str) -> (String, String, String) {
    use sha2::{Digest, Sha256};

    // Generate 32 random bytes
    let random_bytes: [u8; 32] = rand::random();
    let key_suffix = hex::encode(&random_bytes[..24]); // 48 hex chars

    // Full key: prefix + suffix
    let full_key = format!("{}_{}", prefix, key_suffix);

    // Key prefix for display (first 8 chars of suffix)
    let key_prefix = format!("{}_{}", prefix, &key_suffix[..8]);

    // Hash the full key for storage
    let mut hasher = Sha256::new();
    hasher.update(full_key.as_bytes());
    let key_hash = hex::encode(hasher.finalize());

    (full_key, key_prefix, key_hash)
}

/// Hash an API key for lookup.
pub fn hash_api_key(key: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(key.as_bytes());
    hex::encode(hasher.finalize())
}

/// Resolution result - maps name@version to content hash.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resolution {
    /// Package name.
    pub name: String,
    /// Resolved version.
    pub version: Version,
    /// Content hash.
    pub hash: ContentHash,
    /// When this version was published.
    pub published_at: DateTime<Utc>,
    /// Is this version yanked?
    pub yanked: bool,
}

/// Version info for listing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionInfo {
    /// Version number.
    pub version: Version,
    /// Content hash.
    pub hash: ContentHash,
    /// When published.
    pub published_at: DateTime<Utc>,
    /// Download count.
    pub downloads: u64,
    /// Is yanked?
    pub yanked: bool,
}

// ============================================================================
// In-Memory Implementation (for testing and development)
// ============================================================================

/// In-memory metadata store for testing.
pub struct InMemoryMetadataStore {
    /// Package info by content hash.
    packages: RwLock<HashMap<String, PackageInfo>>,
    /// Name to versions mapping (name -> [(version, hash)])
    name_index: RwLock<HashMap<String, Vec<(Version, ContentHash)>>>,
    /// Keyword index (word -> [hash])
    keyword_index: RwLock<HashMap<String, Vec<ContentHash>>>,
    /// Contributions by ID.
    contributions: RwLock<HashMap<Uuid, StoredContribution>>,
    /// Reviews by ID.
    reviews: RwLock<HashMap<Uuid, StoredReview>>,
    /// API keys by hash.
    api_keys: RwLock<HashMap<String, StoredApiKey>>,
}

impl InMemoryMetadataStore {
    /// Create a new in-memory metadata store.
    pub fn new() -> Self {
        Self {
            packages: RwLock::new(HashMap::new()),
            name_index: RwLock::new(HashMap::new()),
            keyword_index: RwLock::new(HashMap::new()),
            contributions: RwLock::new(HashMap::new()),
            reviews: RwLock::new(HashMap::new()),
            api_keys: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for InMemoryMetadataStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MetadataStore for InMemoryMetadataStore {
    async fn store_package(&self, info: &PackageInfo) -> Result<()> {
        let hash_str = info.hash.to_string();
        let name = info.manifest.qualified_name();

        // Store package info
        {
            let mut packages = self.packages.write().await;
            packages.insert(hash_str.clone(), info.clone());
        }

        // Update name index
        {
            let mut name_index = self.name_index.write().await;
            let versions = name_index.entry(name.clone()).or_default();

            // Remove any existing entry for this version
            versions.retain(|(v, _)| v != &info.manifest.version);

            // Add new entry
            versions.push((info.manifest.version.clone(), info.hash.clone()));

            // Sort by version descending
            versions.sort_by(|a, b| b.0.cmp(&a.0));
        }

        // Update keyword index
        {
            let mut keyword_index = self.keyword_index.write().await;

            // Index keywords from manifest
            for keyword in &info.manifest.keywords {
                let words: Vec<String> = keyword
                    .to_lowercase()
                    .split_whitespace()
                    .map(String::from)
                    .collect();
                for word in words {
                    let hashes = keyword_index.entry(word).or_default();
                    if !hashes.iter().any(|h| h == &info.hash) {
                        hashes.push(info.hash.clone());
                    }
                }
            }

            // Index name
            for word in info.manifest.name.to_lowercase().split('-') {
                let hashes = keyword_index.entry(word.to_string()).or_default();
                if !hashes.iter().any(|h| h == &info.hash) {
                    hashes.push(info.hash.clone());
                }
            }

            // Index description words
            for word in info.manifest.description.to_lowercase().split_whitespace() {
                if word.len() > 3 {
                    // Skip short words
                    let hashes = keyword_index.entry(word.to_string()).or_default();
                    if !hashes.iter().any(|h| h == &info.hash) {
                        hashes.push(info.hash.clone());
                    }
                }
            }
        }

        Ok(())
    }

    async fn get_by_hash(&self, hash: &ContentHash) -> Result<Option<PackageInfo>> {
        let packages = self.packages.read().await;
        Ok(packages.get(&hash.to_string()).cloned())
    }

    async fn resolve(&self, name: &str, version_req: &VersionReq) -> Result<Option<Resolution>> {
        let name_index = self.name_index.read().await;
        let packages = self.packages.read().await;

        if let Some(versions) = name_index.get(name) {
            // Find best matching version (highest that satisfies requirement)
            for (version, hash) in versions {
                if version_req.matches(version) {
                    if let Some(info) = packages.get(&hash.to_string()) {
                        return Ok(Some(Resolution {
                            name: name.to_string(),
                            version: version.clone(),
                            hash: hash.clone(),
                            published_at: info.published_at,
                            yanked: info.yanked,
                        }));
                    }
                }
            }
        }

        Ok(None)
    }

    async fn resolve_latest(&self, name: &str) -> Result<Option<Resolution>> {
        let name_index = self.name_index.read().await;
        let packages = self.packages.read().await;

        if let Some(versions) = name_index.get(name) {
            // Get the highest version
            if let Some((version, hash)) = versions.first() {
                if let Some(info) = packages.get(&hash.to_string()) {
                    return Ok(Some(Resolution {
                        name: name.to_string(),
                        version: version.clone(),
                        hash: hash.clone(),
                        published_at: info.published_at,
                        yanked: info.yanked,
                    }));
                }
            }
        }

        Ok(None)
    }

    async fn list_versions(&self, name: &str) -> Result<Vec<VersionInfo>> {
        let name_index = self.name_index.read().await;
        let packages = self.packages.read().await;

        if let Some(versions) = name_index.get(name) {
            let mut result = Vec::new();
            for (version, hash) in versions {
                if let Some(info) = packages.get(&hash.to_string()) {
                    result.push(VersionInfo {
                        version: version.clone(),
                        hash: hash.clone(),
                        published_at: info.published_at,
                        downloads: info.downloads,
                        yanked: info.yanked,
                    });
                }
            }
            Ok(result)
        } else {
            Ok(Vec::new())
        }
    }

    async fn yank(&self, hash: &ContentHash) -> Result<()> {
        let mut packages = self.packages.write().await;
        if let Some(info) = packages.get_mut(&hash.to_string()) {
            info.yanked = true;
            Ok(())
        } else {
            Err(RegistryError::PackageNotFound(hash.to_string()))
        }
    }

    async fn unyank(&self, hash: &ContentHash) -> Result<()> {
        let mut packages = self.packages.write().await;
        if let Some(info) = packages.get_mut(&hash.to_string()) {
            info.yanked = false;
            Ok(())
        } else {
            Err(RegistryError::PackageNotFound(hash.to_string()))
        }
    }

    async fn search_keyword(
        &self,
        query: &str,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<PackageInfo>> {
        let keyword_index = self.keyword_index.read().await;
        let packages = self.packages.read().await;

        // Collect matching hashes with scores
        let mut scores: HashMap<String, usize> = HashMap::new();
        let query_words: Vec<String> = query
            .to_lowercase()
            .split_whitespace()
            .map(String::from)
            .collect();

        for word in &query_words {
            if let Some(hashes) = keyword_index.get(word) {
                for hash in hashes {
                    *scores.entry(hash.to_string()).or_default() += 1;
                }
            }
        }

        // Sort by score descending
        let mut scored: Vec<_> = scores.into_iter().collect();
        scored.sort_by(|a, b| b.1.cmp(&a.1));

        // Apply pagination and collect results
        let result: Vec<PackageInfo> = scored
            .into_iter()
            .skip(offset)
            .take(limit)
            .filter_map(|(hash, _)| packages.get(&hash).cloned())
            .collect();

        Ok(result)
    }

    async fn increment_downloads(&self, hash: &ContentHash) -> Result<u64> {
        let mut packages = self.packages.write().await;
        if let Some(info) = packages.get_mut(&hash.to_string()) {
            info.downloads += 1;
            Ok(info.downloads)
        } else {
            Err(RegistryError::PackageNotFound(hash.to_string()))
        }
    }

    async fn name_exists(&self, name: &str) -> Result<bool> {
        let name_index = self.name_index.read().await;
        Ok(name_index.contains_key(name))
    }
}

#[async_trait]
impl ContributionStore for InMemoryMetadataStore {
    async fn store_contribution(&self, contribution: &StoredContribution) -> Result<Uuid> {
        let mut contributions = self.contributions.write().await;
        contributions.insert(contribution.id, contribution.clone());
        Ok(contribution.id)
    }

    async fn get_contribution(&self, id: Uuid) -> Result<Option<StoredContribution>> {
        let contributions = self.contributions.read().await;
        Ok(contributions.get(&id).cloned())
    }

    async fn update_contribution_status(&self, id: Uuid, status: ContributionStatus) -> Result<()> {
        let mut contributions = self.contributions.write().await;
        if let Some(c) = contributions.get_mut(&id) {
            c.status = status;
            c.updated_at = Utc::now();
            Ok(())
        } else {
            Err(RegistryError::NotFound(format!(
                "Contribution {} not found",
                id
            )))
        }
    }

    async fn list_contributions(
        &self,
        package_hash: Option<&str>,
        status: Option<ContributionStatus>,
        limit: usize,
        offset: usize,
    ) -> Result<(Vec<StoredContribution>, u64)> {
        let contributions = self.contributions.read().await;

        let filtered: Vec<_> = contributions
            .values()
            .filter(|c| {
                // Filter by package hash if specified
                if let Some(ph) = package_hash {
                    if c.package_hash.as_deref() != Some(ph) {
                        return false;
                    }
                }
                // Filter by status if specified
                if let Some(s) = status {
                    if c.status != s {
                        return false;
                    }
                }
                true
            })
            .cloned()
            .collect();

        let total = filtered.len() as u64;
        let paginated: Vec<_> = filtered.into_iter().skip(offset).take(limit).collect();

        Ok((paginated, total))
    }

    async fn store_review(&self, review: &StoredReview) -> Result<Uuid> {
        let mut reviews = self.reviews.write().await;
        reviews.insert(review.id, review.clone());
        Ok(review.id)
    }

    async fn get_reviews(&self, contribution_id: Uuid) -> Result<Vec<StoredReview>> {
        let reviews = self.reviews.read().await;
        Ok(reviews
            .values()
            .filter(|r| r.contribution_id == contribution_id)
            .cloned()
            .collect())
    }

    async fn get_consensus(&self, contribution_id: Uuid) -> Result<Option<ContributionConsensus>> {
        let reviews = self.reviews.read().await;
        let contribution_reviews: Vec<_> = reviews
            .values()
            .filter(|r| r.contribution_id == contribution_id)
            .collect();

        if contribution_reviews.is_empty() {
            return Ok(None);
        }

        let total = contribution_reviews.len() as u64;
        let approve_count = contribution_reviews
            .iter()
            .filter(|r| r.verdict == "approve" || r.verdict == "approve_with_suggestions")
            .count() as u64;
        let reject_count = contribution_reviews
            .iter()
            .filter(|r| r.verdict == "reject")
            .count() as u64;
        let avg_confidence = contribution_reviews
            .iter()
            .map(|r| r.confidence)
            .sum::<f32>()
            / total as f32;

        Ok(Some(ContributionConsensus {
            total_reviews: total,
            approve_count,
            reject_count,
            avg_confidence,
        }))
    }
}

#[async_trait]
impl ApiKeyStore for InMemoryMetadataStore {
    async fn store_api_key(&self, key: &StoredApiKey) -> Result<Uuid> {
        let mut api_keys = self.api_keys.write().await;
        api_keys.insert(key.key_hash.clone(), key.clone());
        Ok(key.id)
    }

    async fn get_api_key_by_hash(&self, key_hash: &str) -> Result<Option<StoredApiKey>> {
        let api_keys = self.api_keys.read().await;
        Ok(api_keys.get(key_hash).cloned())
    }

    async fn get_api_key(&self, id: Uuid) -> Result<Option<StoredApiKey>> {
        let api_keys = self.api_keys.read().await;
        Ok(api_keys.values().find(|k| k.id == id).cloned())
    }

    async fn verify_api_key(&self, key_hash: &str) -> Result<ApiKeyVerification> {
        let api_keys = self.api_keys.read().await;
        match api_keys.get(key_hash) {
            Some(key) => {
                // Check if key is active
                if !key.active {
                    return Ok(ApiKeyVerification {
                        key: key.clone(),
                        valid: false,
                        invalid_reason: Some("API key has been revoked".to_string()),
                    });
                }

                // Check expiration
                if let Some(expires_at) = key.expires_at {
                    if Utc::now() > expires_at {
                        return Ok(ApiKeyVerification {
                            key: key.clone(),
                            valid: false,
                            invalid_reason: Some("API key has expired".to_string()),
                        });
                    }
                }

                Ok(ApiKeyVerification {
                    key: key.clone(),
                    valid: true,
                    invalid_reason: None,
                })
            }
            None => Err(RegistryError::NotFound("API key not found".to_string())),
        }
    }

    async fn touch_api_key(&self, key_hash: &str) -> Result<()> {
        let mut api_keys = self.api_keys.write().await;
        if let Some(key) = api_keys.get_mut(key_hash) {
            key.last_used_at = Some(Utc::now());
            key.updated_at = Utc::now();
            Ok(())
        } else {
            Err(RegistryError::NotFound("API key not found".to_string()))
        }
    }

    async fn revoke_api_key(&self, id: Uuid) -> Result<()> {
        let mut api_keys = self.api_keys.write().await;
        for key in api_keys.values_mut() {
            if key.id == id {
                key.active = false;
                key.updated_at = Utc::now();
                return Ok(());
            }
        }
        Err(RegistryError::NotFound(format!("API key {} not found", id)))
    }

    async fn list_api_keys(
        &self,
        agent_id: Option<Uuid>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<StoredApiKey>> {
        let api_keys = self.api_keys.read().await;

        let filtered: Vec<_> = api_keys
            .values()
            .filter(|k| {
                if let Some(aid) = agent_id {
                    k.agent_id == Some(aid)
                } else {
                    true
                }
            })
            .cloned()
            .collect();

        Ok(filtered.into_iter().skip(offset).take(limit).collect())
    }
}

// ============================================================================
// PostgreSQL Implementation (behind feature flag)
// ============================================================================

#[cfg(feature = "postgres")]
pub mod postgres {
    use super::*;
    use crate::package::{Lineage, PackageManifest, PackageType, TrustLevel};
    use deadpool_postgres::{Config, ManagerConfig, Pool, RecyclingMethod, Runtime};
    use tokio_postgres::NoTls;

    /// PostgreSQL metadata store using tokio-postgres with connection pooling.
    pub struct PostgresMetadataStore {
        pool: Pool,
    }

    impl PostgresMetadataStore {
        /// Connect to PostgreSQL database.
        pub async fn connect(database_url: &str) -> Result<Self> {
            // Parse the database URL into config
            let config = database_url
                .parse::<tokio_postgres::Config>()
                .map_err(|e| RegistryError::StorageError(format!("Invalid database URL: {}", e)))?;

            let mut pool_config = Config::new();
            pool_config.host = config.get_hosts().first().map(|h| match h {
                tokio_postgres::config::Host::Tcp(s) => s.clone(),
                #[cfg(unix)]
                tokio_postgres::config::Host::Unix(p) => p.to_string_lossy().to_string(),
            });
            pool_config.port = config.get_ports().first().copied();
            pool_config.user = config.get_user().map(String::from);
            pool_config.password = config
                .get_password()
                .map(|p| String::from_utf8_lossy(p).to_string());
            pool_config.dbname = config.get_dbname().map(String::from);
            pool_config.manager = Some(ManagerConfig {
                recycling_method: RecyclingMethod::Fast,
            });

            let pool = pool_config
                .create_pool(Some(Runtime::Tokio1), NoTls)
                .map_err(|e| {
                    RegistryError::StorageError(format!("Failed to create pool: {}", e))
                })?;

            Ok(Self { pool })
        }

        /// Connect with existing pool.
        pub fn with_pool(pool: Pool) -> Self {
            Self { pool }
        }

        /// Run database migrations.
        pub async fn migrate(&self) -> Result<()> {
            let client = self.pool.get().await.map_err(|e| {
                RegistryError::StorageError(format!("Failed to get connection: {}", e))
            })?;
            client
                .batch_execute(SCHEMA_SQL)
                .await
                .map_err(|e| RegistryError::StorageError(format!("Migration failed: {}", e)))?;
            Ok(())
        }

        /// Get the underlying pool.
        pub fn pool(&self) -> &Pool {
            &self.pool
        }
    }

    /// Database schema SQL.
    const SCHEMA_SQL: &str = r#"
-- Packages table: stores package metadata
CREATE TABLE IF NOT EXISTS packages (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    content_hash VARCHAR(128) UNIQUE NOT NULL,
    name VARCHAR(255) NOT NULL,
    namespace VARCHAR(255),
    version VARCHAR(64) NOT NULL,
    description TEXT NOT NULL,
    package_type VARCHAR(64) NOT NULL DEFAULT 'library',
    keywords TEXT[] DEFAULT '{}',
    manifest JSONB NOT NULL,
    publisher_key_id VARCHAR(255) NOT NULL,
    trust_level VARCHAR(32) NOT NULL DEFAULT 'unknown',
    lineage JSONB,
    downloads BIGINT NOT NULL DEFAULT 0,
    yanked BOOLEAN NOT NULL DEFAULT FALSE,
    published_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Unique constraint on name + version
    UNIQUE(name, namespace, version)
);

-- Indexes for common queries
CREATE INDEX IF NOT EXISTS idx_packages_name ON packages(name);
CREATE INDEX IF NOT EXISTS idx_packages_namespace_name ON packages(namespace, name);
CREATE INDEX IF NOT EXISTS idx_packages_keywords ON packages USING GIN(keywords);
CREATE INDEX IF NOT EXISTS idx_packages_package_type ON packages(package_type);
CREATE INDEX IF NOT EXISTS idx_packages_published_at ON packages(published_at DESC);
CREATE INDEX IF NOT EXISTS idx_packages_downloads ON packages(downloads DESC);

-- Full-text search index
CREATE INDEX IF NOT EXISTS idx_packages_fts ON packages
    USING GIN(to_tsvector('english', name || ' ' || description || ' ' || array_to_string(keywords, ' ')));

-- Capabilities table: stores package capabilities
CREATE TABLE IF NOT EXISTS capabilities (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    package_id UUID NOT NULL REFERENCES packages(id) ON DELETE CASCADE,
    name VARCHAR(255) NOT NULL,
    version VARCHAR(64),
    description TEXT,
    capability_type VARCHAR(32) NOT NULL DEFAULT 'provides', -- 'provides' or 'requires'

    UNIQUE(package_id, name, capability_type)
);

CREATE INDEX IF NOT EXISTS idx_capabilities_name ON capabilities(name);
CREATE INDEX IF NOT EXISTS idx_capabilities_type ON capabilities(capability_type);

-- Dependencies table
CREATE TABLE IF NOT EXISTS dependencies (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    package_id UUID NOT NULL REFERENCES packages(id) ON DELETE CASCADE,
    dependency_name VARCHAR(255) NOT NULL,
    version_req VARCHAR(64) NOT NULL,
    pinned_hash VARCHAR(128),
    optional BOOLEAN NOT NULL DEFAULT FALSE,

    UNIQUE(package_id, dependency_name)
);

CREATE INDEX IF NOT EXISTS idx_dependencies_name ON dependencies(dependency_name);

-- Download statistics (time-series)
CREATE TABLE IF NOT EXISTS download_stats (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    package_id UUID NOT NULL REFERENCES packages(id) ON DELETE CASCADE,
    date DATE NOT NULL,
    count BIGINT NOT NULL DEFAULT 0,

    UNIQUE(package_id, date)
);

CREATE INDEX IF NOT EXISTS idx_download_stats_package_date ON download_stats(package_id, date DESC);

-- Trigger to update updated_at
CREATE OR REPLACE FUNCTION update_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS packages_updated_at ON packages;
CREATE TRIGGER packages_updated_at
    BEFORE UPDATE ON packages
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at();

-- ============================================================================
-- Contributions Tables
-- ============================================================================

-- Contributions table: stores all contribution types
CREATE TABLE IF NOT EXISTS contributions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    contribution_type VARCHAR(32) NOT NULL, -- 'bug', 'improvement', 'request', 'fix'
    package_hash VARCHAR(128), -- May be NULL for package requests
    title VARCHAR(500) NOT NULL,
    description TEXT NOT NULL,
    status VARCHAR(32) NOT NULL DEFAULT 'submitted',
    -- Reporter info (denormalized for simplicity)
    reporter_app_id UUID,
    reporter_name VARCHAR(255),
    reporter_public_key VARCHAR(255) NOT NULL,
    reporter_is_ai BOOLEAN NOT NULL DEFAULT FALSE,
    -- Type-specific data stored as JSONB
    data JSONB NOT NULL DEFAULT '{}',
    -- Signature
    signature VARCHAR(512) NOT NULL,
    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_contributions_type ON contributions(contribution_type);
CREATE INDEX IF NOT EXISTS idx_contributions_package ON contributions(package_hash);
CREATE INDEX IF NOT EXISTS idx_contributions_status ON contributions(status);
CREATE INDEX IF NOT EXISTS idx_contributions_reporter ON contributions(reporter_public_key);
CREATE INDEX IF NOT EXISTS idx_contributions_created ON contributions(created_at DESC);

-- Reviews table: stores reviews for contributions
CREATE TABLE IF NOT EXISTS reviews (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    contribution_id UUID NOT NULL REFERENCES contributions(id) ON DELETE CASCADE,
    -- Reviewer info
    reviewer_app_id UUID,
    reviewer_name VARCHAR(255),
    reviewer_public_key VARCHAR(255) NOT NULL,
    reviewer_is_ai BOOLEAN NOT NULL DEFAULT FALSE,
    -- Review data
    verdict VARCHAR(32) NOT NULL, -- 'approve', 'approve_with_suggestions', 'request_changes', 'reject', 'abstain'
    confidence REAL NOT NULL,
    justification TEXT,
    suggestions JSONB, -- Array of suggestions
    -- Signature
    signature VARCHAR(512) NOT NULL,
    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_reviews_contribution ON reviews(contribution_id);
CREATE INDEX IF NOT EXISTS idx_reviews_reviewer ON reviews(reviewer_public_key);
CREATE INDEX IF NOT EXISTS idx_reviews_verdict ON reviews(verdict);

-- Contribution consensus view (materialized for performance)
CREATE OR REPLACE VIEW contribution_consensus AS
SELECT
    c.id AS contribution_id,
    COUNT(r.id) AS total_reviews,
    COUNT(CASE WHEN r.verdict = 'approve' OR r.verdict = 'approve_with_suggestions' THEN 1 END) AS approve_count,
    COUNT(CASE WHEN r.verdict = 'reject' THEN 1 END) AS reject_count,
    AVG(r.confidence) AS avg_confidence,
    MAX(r.created_at) AS last_review_at
FROM contributions c
LEFT JOIN reviews r ON r.contribution_id = c.id
GROUP BY c.id;

-- Trigger to update contributions.updated_at
DROP TRIGGER IF EXISTS contributions_updated_at ON contributions;
CREATE TRIGGER contributions_updated_at
    BEFORE UPDATE ON contributions
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at();

-- ============================================================================
-- API Keys Table
-- ============================================================================

-- API keys table: stores hashed API keys for authentication
CREATE TABLE IF NOT EXISTS api_keys (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    key_hash VARCHAR(128) UNIQUE NOT NULL, -- SHA-256 hash of the API key
    key_prefix VARCHAR(32) NOT NULL, -- First chars for identification (e.g., "dk_live_abc...")
    agent_id UUID, -- Owner agent ID (optional)
    name VARCHAR(255) NOT NULL, -- Key description/name
    trust_level VARCHAR(32) NOT NULL DEFAULT 'basic', -- 'basic', 'verified', 'trusted'
    scopes TEXT[] NOT NULL DEFAULT '{}', -- Permissions: 'read', 'write', 'admin'
    rate_limit_rpm INTEGER, -- Override rate limit (NULL uses default)
    active BOOLEAN NOT NULL DEFAULT TRUE,
    expires_at TIMESTAMPTZ, -- NULL means never expires
    last_used_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_api_keys_key_hash ON api_keys(key_hash);
CREATE INDEX IF NOT EXISTS idx_api_keys_agent_id ON api_keys(agent_id);
CREATE INDEX IF NOT EXISTS idx_api_keys_active ON api_keys(active);

-- Trigger to update api_keys.updated_at
DROP TRIGGER IF EXISTS api_keys_updated_at ON api_keys;
CREATE TRIGGER api_keys_updated_at
    BEFORE UPDATE ON api_keys
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at();
"#;

    /// Parse a PackageInfo from a tokio_postgres Row.
    fn package_info_from_row(row: &tokio_postgres::Row) -> Result<PackageInfo> {
        let content_hash: String = row.get("content_hash");
        let manifest_json: serde_json::Value = row.get("manifest");
        let trust_level_str: String = row.get("trust_level");
        let lineage_json: Option<serde_json::Value> = row.get("lineage");
        let downloads: i64 = row.get("downloads");
        let yanked: bool = row.get("yanked");
        let published_at: DateTime<Utc> = row.get("published_at");
        let publisher_key_id: String = row.get("publisher_key_id");

        let hash = ContentHash::from_string(&content_hash)
            .map_err(|e| RegistryError::StorageError(format!("Invalid hash: {}", e)))?;

        let manifest: PackageManifest = serde_json::from_value(manifest_json)
            .map_err(|e| RegistryError::StorageError(format!("Invalid manifest JSON: {}", e)))?;

        let trust_level = match trust_level_str.as_str() {
            "unknown" => TrustLevel::Unknown,
            "community" => TrustLevel::Community,
            "organization" => TrustLevel::Organization,
            "official" => TrustLevel::Official,
            _ => TrustLevel::Unknown,
        };

        let lineage: Option<Lineage> = lineage_json
            .map(serde_json::from_value)
            .transpose()
            .map_err(|e| RegistryError::StorageError(format!("Invalid lineage JSON: {}", e)))?;

        Ok(PackageInfo {
            hash,
            manifest,
            published_at,
            publisher_key_id,
            downloads: downloads as u64,
            trust_level,
            lineage,
            yanked,
        })
    }

    #[async_trait]
    impl MetadataStore for PostgresMetadataStore {
        async fn store_package(&self, info: &PackageInfo) -> Result<()> {
            let client = self.pool.get().await.map_err(|e| {
                RegistryError::StorageError(format!("Failed to get connection: {}", e))
            })?;

            let hash_str = info.hash.to_string();
            let manifest_json = serde_json::to_value(&info.manifest).map_err(|e| {
                RegistryError::StorageError(format!("Failed to serialize manifest: {}", e))
            })?;
            let lineage_json = info
                .lineage
                .as_ref()
                .map(serde_json::to_value)
                .transpose()
                .map_err(|e| {
                    RegistryError::StorageError(format!("Failed to serialize lineage: {}", e))
                })?;

            let package_type = match &info.manifest.package_type {
                PackageType::Agent => "agent",
                PackageType::Tool => "tool",
                PackageType::Prompt => "prompt",
                PackageType::Embedding => "embedding",
                PackageType::Retrieval => "retrieval",
                PackageType::Application => "application",
                PackageType::Library => "library",
                PackageType::Other(s) => s.as_str(),
            };

            let trust_level = match info.trust_level {
                TrustLevel::Unknown => "unknown",
                TrustLevel::Community => "community",
                TrustLevel::Organization => "organization",
                TrustLevel::Official => "official",
            };

            let version_str = info.manifest.version.to_string();
            let downloads = info.downloads as i64;

            client
                .execute(
                    r#"
                INSERT INTO packages (
                    content_hash, name, namespace, version, description,
                    package_type, keywords, manifest, publisher_key_id,
                    trust_level, lineage, downloads, yanked, published_at
                ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
                ON CONFLICT (content_hash) DO UPDATE SET
                    downloads = EXCLUDED.downloads,
                    yanked = EXCLUDED.yanked,
                    updated_at = NOW()
            "#,
                    &[
                        &hash_str,
                        &info.manifest.name,
                        &info.manifest.namespace,
                        &version_str,
                        &info.manifest.description,
                        &package_type,
                        &info.manifest.keywords,
                        &manifest_json,
                        &info.publisher_key_id,
                        &trust_level,
                        &lineage_json,
                        &downloads,
                        &info.yanked,
                        &info.published_at,
                    ],
                )
                .await
                .map_err(|e| {
                    RegistryError::StorageError(format!("Failed to store package: {}", e))
                })?;

            // Get the package ID
            let row = client
                .query_opt(
                    "SELECT id FROM packages WHERE content_hash = $1",
                    &[&hash_str],
                )
                .await
                .map_err(|e| {
                    RegistryError::StorageError(format!("Failed to get package ID: {}", e))
                })?;

            if let Some(row) = row {
                let pkg_id: uuid::Uuid = row.get(0);

                // Clear existing capabilities
                client
                    .execute("DELETE FROM capabilities WHERE package_id = $1", &[&pkg_id])
                    .await
                    .map_err(|e| {
                        RegistryError::StorageError(format!("Failed to clear capabilities: {}", e))
                    })?;

                // Insert provides
                for cap in &info.manifest.provides {
                    client.execute(r#"
                        INSERT INTO capabilities (package_id, name, version, description, capability_type)
                        VALUES ($1, $2, $3, $4, 'provides')
                    "#, &[&pkg_id, &cap.name, &cap.version, &cap.description]).await
                        .map_err(|e| RegistryError::StorageError(format!("Failed to store capability: {}", e)))?;
                }

                // Insert requires
                for cap in &info.manifest.requires {
                    client.execute(r#"
                        INSERT INTO capabilities (package_id, name, version, description, capability_type)
                        VALUES ($1, $2, $3, $4, 'requires')
                    "#, &[&pkg_id, &cap.name, &cap.version, &cap.description]).await
                        .map_err(|e| RegistryError::StorageError(format!("Failed to store capability: {}", e)))?;
                }

                // Store dependencies
                client
                    .execute("DELETE FROM dependencies WHERE package_id = $1", &[&pkg_id])
                    .await
                    .map_err(|e| {
                        RegistryError::StorageError(format!("Failed to clear dependencies: {}", e))
                    })?;

                for dep in &info.manifest.dependencies {
                    let pinned_hash = dep.hash.as_ref().map(|h| h.to_string());
                    client.execute(r#"
                        INSERT INTO dependencies (package_id, dependency_name, version_req, pinned_hash, optional)
                        VALUES ($1, $2, $3, $4, $5)
                    "#, &[&pkg_id, &dep.name, &dep.version_req, &pinned_hash, &dep.optional]).await
                        .map_err(|e| RegistryError::StorageError(format!("Failed to store dependency: {}", e)))?;
                }
            }

            Ok(())
        }

        async fn get_by_hash(&self, hash: &ContentHash) -> Result<Option<PackageInfo>> {
            let client = self.pool.get().await.map_err(|e| {
                RegistryError::StorageError(format!("Failed to get connection: {}", e))
            })?;

            let hash_str = hash.to_string();
            let row = client
                .query_opt(
                    r#"
                SELECT id, content_hash, name, namespace, version, description,
                       package_type, keywords, manifest, publisher_key_id,
                       trust_level, lineage, downloads, yanked, published_at
                FROM packages
                WHERE content_hash = $1
            "#,
                    &[&hash_str],
                )
                .await
                .map_err(|e| RegistryError::StorageError(format!("Query failed: {}", e)))?;

            row.map(|r| package_info_from_row(&r)).transpose()
        }

        async fn resolve(
            &self,
            name: &str,
            version_req: &VersionReq,
        ) -> Result<Option<Resolution>> {
            let client = self.pool.get().await.map_err(|e| {
                RegistryError::StorageError(format!("Failed to get connection: {}", e))
            })?;

            // Get all versions for this name
            let rows = client
                .query(
                    r#"
                SELECT content_hash, version, name, published_at, yanked
                FROM packages
                WHERE (name = $1 OR (namespace || '/' || name) = $1)
                ORDER BY published_at DESC
            "#,
                    &[&name],
                )
                .await
                .map_err(|e| RegistryError::StorageError(format!("Query failed: {}", e)))?;

            // Find best matching version
            for row in rows {
                let hash_str: String = row.get(0);
                let version_str: String = row.get(1);
                let pkg_name: String = row.get(2);
                let published_at: DateTime<Utc> = row.get(3);
                let yanked: bool = row.get(4);

                if let Ok(version) = Version::parse(&version_str) {
                    if version_req.matches(&version) {
                        let hash = ContentHash::from_string(&hash_str).map_err(|e| {
                            RegistryError::StorageError(format!("Invalid hash: {}", e))
                        })?;
                        return Ok(Some(Resolution {
                            name: pkg_name,
                            version,
                            hash,
                            published_at,
                            yanked,
                        }));
                    }
                }
            }

            Ok(None)
        }

        async fn resolve_latest(&self, name: &str) -> Result<Option<Resolution>> {
            let client = self.pool.get().await.map_err(|e| {
                RegistryError::StorageError(format!("Failed to get connection: {}", e))
            })?;

            let row = client
                .query_opt(
                    r#"
                SELECT content_hash, version, name, published_at, yanked
                FROM packages
                WHERE (name = $1 OR (namespace || '/' || name) = $1)
                  AND yanked = FALSE
                ORDER BY published_at DESC
                LIMIT 1
            "#,
                    &[&name],
                )
                .await
                .map_err(|e| RegistryError::StorageError(format!("Query failed: {}", e)))?;

            if let Some(row) = row {
                let hash_str: String = row.get(0);
                let version_str: String = row.get(1);
                let pkg_name: String = row.get(2);
                let published_at: DateTime<Utc> = row.get(3);
                let yanked: bool = row.get(4);

                let version = Version::parse(&version_str)
                    .map_err(|e| RegistryError::StorageError(format!("Invalid version: {}", e)))?;
                let hash = ContentHash::from_string(&hash_str)
                    .map_err(|e| RegistryError::StorageError(format!("Invalid hash: {}", e)))?;
                Ok(Some(Resolution {
                    name: pkg_name,
                    version,
                    hash,
                    published_at,
                    yanked,
                }))
            } else {
                Ok(None)
            }
        }

        async fn list_versions(&self, name: &str) -> Result<Vec<VersionInfo>> {
            let client = self.pool.get().await.map_err(|e| {
                RegistryError::StorageError(format!("Failed to get connection: {}", e))
            })?;

            let rows = client
                .query(
                    r#"
                SELECT content_hash, version, published_at, downloads, yanked
                FROM packages
                WHERE (name = $1 OR (namespace || '/' || name) = $1)
                ORDER BY published_at DESC
            "#,
                    &[&name],
                )
                .await
                .map_err(|e| RegistryError::StorageError(format!("Query failed: {}", e)))?;

            let mut versions = Vec::new();
            for row in rows {
                let hash_str: String = row.get(0);
                let version_str: String = row.get(1);
                let published_at: DateTime<Utc> = row.get(2);
                let downloads: i64 = row.get(3);
                let yanked: bool = row.get(4);

                let version = Version::parse(&version_str)
                    .map_err(|e| RegistryError::StorageError(format!("Invalid version: {}", e)))?;
                let hash = ContentHash::from_string(&hash_str)
                    .map_err(|e| RegistryError::StorageError(format!("Invalid hash: {}", e)))?;
                versions.push(VersionInfo {
                    version,
                    hash,
                    published_at,
                    downloads: downloads as u64,
                    yanked,
                });
            }

            Ok(versions)
        }

        async fn yank(&self, hash: &ContentHash) -> Result<()> {
            let client = self.pool.get().await.map_err(|e| {
                RegistryError::StorageError(format!("Failed to get connection: {}", e))
            })?;

            let hash_str = hash.to_string();
            let rows_affected = client
                .execute(
                    "UPDATE packages SET yanked = TRUE WHERE content_hash = $1",
                    &[&hash_str],
                )
                .await
                .map_err(|e| RegistryError::StorageError(format!("Update failed: {}", e)))?;

            if rows_affected == 0 {
                Err(RegistryError::PackageNotFound(hash.to_string()))
            } else {
                Ok(())
            }
        }

        async fn unyank(&self, hash: &ContentHash) -> Result<()> {
            let client = self.pool.get().await.map_err(|e| {
                RegistryError::StorageError(format!("Failed to get connection: {}", e))
            })?;

            let hash_str = hash.to_string();
            let rows_affected = client
                .execute(
                    "UPDATE packages SET yanked = FALSE WHERE content_hash = $1",
                    &[&hash_str],
                )
                .await
                .map_err(|e| RegistryError::StorageError(format!("Update failed: {}", e)))?;

            if rows_affected == 0 {
                Err(RegistryError::PackageNotFound(hash.to_string()))
            } else {
                Ok(())
            }
        }

        async fn search_keyword(
            &self,
            query: &str,
            limit: usize,
            offset: usize,
        ) -> Result<Vec<PackageInfo>> {
            let client = self.pool.get().await.map_err(|e| {
                RegistryError::StorageError(format!("Failed to get connection: {}", e))
            })?;

            let limit_i64 = limit as i64;
            let offset_i64 = offset as i64;

            // Use PostgreSQL full-text search
            let rows = client.query(r#"
                SELECT id, content_hash, name, namespace, version, description,
                       package_type, keywords, manifest, publisher_key_id,
                       trust_level, lineage, downloads, yanked, published_at
                FROM packages
                WHERE to_tsvector('english', name || ' ' || description || ' ' || array_to_string(keywords, ' '))
                      @@ plainto_tsquery('english', $1)
                ORDER BY ts_rank(
                    to_tsvector('english', name || ' ' || description || ' ' || array_to_string(keywords, ' ')),
                    plainto_tsquery('english', $1)
                ) DESC, downloads DESC
                LIMIT $2 OFFSET $3
            "#, &[&query, &limit_i64, &offset_i64]).await
                .map_err(|e| RegistryError::StorageError(format!("Search query failed: {}", e)))?;

            rows.iter().map(package_info_from_row).collect()
        }

        async fn increment_downloads(&self, hash: &ContentHash) -> Result<u64> {
            let client = self.pool.get().await.map_err(|e| {
                RegistryError::StorageError(format!("Failed to get connection: {}", e))
            })?;

            let hash_str = hash.to_string();
            let row = client
                .query_opt(
                    r#"
                UPDATE packages
                SET downloads = downloads + 1
                WHERE content_hash = $1
                RETURNING downloads
            "#,
                    &[&hash_str],
                )
                .await
                .map_err(|e| RegistryError::StorageError(format!("Update failed: {}", e)))?;

            match row {
                Some(row) => {
                    let count: i64 = row.get(0);
                    Ok(count as u64)
                }
                None => Err(RegistryError::PackageNotFound(hash.to_string())),
            }
        }

        async fn name_exists(&self, name: &str) -> Result<bool> {
            let client = self.pool.get().await.map_err(|e| {
                RegistryError::StorageError(format!("Failed to get connection: {}", e))
            })?;

            let row = client.query_opt(
                "SELECT 1 FROM packages WHERE name = $1 OR (namespace || '/' || name) = $1 LIMIT 1",
                &[&name]
            ).await
                .map_err(|e| RegistryError::StorageError(format!("Query failed: {}", e)))?;

            Ok(row.is_some())
        }
    }

    #[async_trait]
    impl ContributionStore for PostgresMetadataStore {
        async fn store_contribution(&self, contribution: &StoredContribution) -> Result<Uuid> {
            let client = self.pool.get().await.map_err(|e| {
                RegistryError::StorageError(format!("Failed to get connection: {}", e))
            })?;

            let status_str = format!("{:?}", contribution.status).to_lowercase();

            client
                .execute(
                    r#"
                INSERT INTO contributions (
                    id, contribution_type, package_hash, title, description, status,
                    reporter_app_id, reporter_name, reporter_public_key, reporter_is_ai,
                    data, signature, created_at, updated_at
                ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
            "#,
                    &[
                        &contribution.id,
                        &contribution.contribution_type,
                        &contribution.package_hash,
                        &contribution.title,
                        &contribution.description,
                        &status_str,
                        &contribution.reporter_app_id,
                        &contribution.reporter_name,
                        &contribution.reporter_public_key,
                        &contribution.reporter_is_ai,
                        &contribution.data,
                        &contribution.signature,
                        &contribution.created_at,
                        &contribution.updated_at,
                    ],
                )
                .await
                .map_err(|e| {
                    RegistryError::StorageError(format!("Failed to store contribution: {}", e))
                })?;

            Ok(contribution.id)
        }

        async fn get_contribution(&self, id: Uuid) -> Result<Option<StoredContribution>> {
            let client = self.pool.get().await.map_err(|e| {
                RegistryError::StorageError(format!("Failed to get connection: {}", e))
            })?;

            let row = client
                .query_opt(
                    r#"
                SELECT id, contribution_type, package_hash, title, description, status,
                       reporter_app_id, reporter_name, reporter_public_key, reporter_is_ai,
                       data, signature, created_at, updated_at
                FROM contributions
                WHERE id = $1
            "#,
                    &[&id],
                )
                .await
                .map_err(|e| RegistryError::StorageError(format!("Query failed: {}", e)))?;

            row.map(|r| contribution_from_row(&r)).transpose()
        }

        async fn update_contribution_status(
            &self,
            id: Uuid,
            status: ContributionStatus,
        ) -> Result<()> {
            let client = self.pool.get().await.map_err(|e| {
                RegistryError::StorageError(format!("Failed to get connection: {}", e))
            })?;

            let status_str = format!("{:?}", status).to_lowercase();

            let rows_affected = client
                .execute(
                    "UPDATE contributions SET status = $1, updated_at = NOW() WHERE id = $2",
                    &[&status_str, &id],
                )
                .await
                .map_err(|e| RegistryError::StorageError(format!("Update failed: {}", e)))?;

            if rows_affected == 0 {
                Err(RegistryError::NotFound(format!(
                    "Contribution {} not found",
                    id
                )))
            } else {
                Ok(())
            }
        }

        async fn list_contributions(
            &self,
            package_hash: Option<&str>,
            status: Option<ContributionStatus>,
            limit: usize,
            offset: usize,
        ) -> Result<(Vec<StoredContribution>, u64)> {
            let client = self.pool.get().await.map_err(|e| {
                RegistryError::StorageError(format!("Failed to get connection: {}", e))
            })?;

            let limit_i64 = limit as i64;
            let offset_i64 = offset as i64;

            let base_select = r#"
                SELECT id, contribution_type, package_hash, title, description, status,
                       reporter_app_id, reporter_name, reporter_public_key, reporter_is_ai,
                       data, signature, created_at, updated_at
                FROM contributions"#;

            // Execute queries based on filter combination
            let (rows, total): (Vec<tokio_postgres::Row>, i64) = match (package_hash, status) {
                (Some(ph), Some(s)) => {
                    let status_str = format!("{:?}", s).to_lowercase();
                    let ph_str = ph.to_string();
                    let count_row = client.query_one(
                        "SELECT COUNT(*) FROM contributions WHERE package_hash = $1 AND status = $2",
                        &[&ph_str, &status_str]
                    ).await.map_err(|e| RegistryError::StorageError(format!("Count failed: {}", e)))?;
                    let total: i64 = count_row.get(0);

                    let query = format!("{} WHERE package_hash = $1 AND status = $2 ORDER BY created_at DESC LIMIT $3 OFFSET $4", base_select);
                    let rows = client
                        .query(&query, &[&ph_str, &status_str, &limit_i64, &offset_i64])
                        .await
                        .map_err(|e| RegistryError::StorageError(format!("Query failed: {}", e)))?;
                    (rows, total)
                }
                (Some(ph), None) => {
                    let ph_str = ph.to_string();
                    let count_row = client
                        .query_one(
                            "SELECT COUNT(*) FROM contributions WHERE package_hash = $1",
                            &[&ph_str],
                        )
                        .await
                        .map_err(|e| RegistryError::StorageError(format!("Count failed: {}", e)))?;
                    let total: i64 = count_row.get(0);

                    let query = format!(
                        "{} WHERE package_hash = $1 ORDER BY created_at DESC LIMIT $2 OFFSET $3",
                        base_select
                    );
                    let rows = client
                        .query(&query, &[&ph_str, &limit_i64, &offset_i64])
                        .await
                        .map_err(|e| RegistryError::StorageError(format!("Query failed: {}", e)))?;
                    (rows, total)
                }
                (None, Some(s)) => {
                    let status_str = format!("{:?}", s).to_lowercase();
                    let count_row = client
                        .query_one(
                            "SELECT COUNT(*) FROM contributions WHERE status = $1",
                            &[&status_str],
                        )
                        .await
                        .map_err(|e| RegistryError::StorageError(format!("Count failed: {}", e)))?;
                    let total: i64 = count_row.get(0);

                    let query = format!(
                        "{} WHERE status = $1 ORDER BY created_at DESC LIMIT $2 OFFSET $3",
                        base_select
                    );
                    let rows = client
                        .query(&query, &[&status_str, &limit_i64, &offset_i64])
                        .await
                        .map_err(|e| RegistryError::StorageError(format!("Query failed: {}", e)))?;
                    (rows, total)
                }
                (None, None) => {
                    let count_row = client
                        .query_one("SELECT COUNT(*) FROM contributions", &[])
                        .await
                        .map_err(|e| RegistryError::StorageError(format!("Count failed: {}", e)))?;
                    let total: i64 = count_row.get(0);

                    let query = format!(
                        "{} ORDER BY created_at DESC LIMIT $1 OFFSET $2",
                        base_select
                    );
                    let rows = client
                        .query(&query, &[&limit_i64, &offset_i64])
                        .await
                        .map_err(|e| RegistryError::StorageError(format!("Query failed: {}", e)))?;
                    (rows, total)
                }
            };

            let contributions: Result<Vec<_>> = rows.iter().map(contribution_from_row).collect();

            Ok((contributions?, total as u64))
        }

        async fn store_review(&self, review: &StoredReview) -> Result<Uuid> {
            let client = self.pool.get().await.map_err(|e| {
                RegistryError::StorageError(format!("Failed to get connection: {}", e))
            })?;

            client
                .execute(
                    r#"
                INSERT INTO reviews (
                    id, contribution_id, reviewer_app_id, reviewer_name, reviewer_public_key,
                    reviewer_is_ai, verdict, confidence, justification, signature, created_at
                ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            "#,
                    &[
                        &review.id,
                        &review.contribution_id,
                        &review.reviewer_app_id,
                        &review.reviewer_name,
                        &review.reviewer_public_key,
                        &review.reviewer_is_ai,
                        &review.verdict,
                        &review.confidence,
                        &review.justification,
                        &review.signature,
                        &review.created_at,
                    ],
                )
                .await
                .map_err(|e| {
                    RegistryError::StorageError(format!("Failed to store review: {}", e))
                })?;

            Ok(review.id)
        }

        async fn get_reviews(&self, contribution_id: Uuid) -> Result<Vec<StoredReview>> {
            let client = self.pool.get().await.map_err(|e| {
                RegistryError::StorageError(format!("Failed to get connection: {}", e))
            })?;

            let rows = client
                .query(
                    r#"
                SELECT id, contribution_id, reviewer_app_id, reviewer_name, reviewer_public_key,
                       reviewer_is_ai, verdict, confidence, justification, signature, created_at
                FROM reviews
                WHERE contribution_id = $1
                ORDER BY created_at ASC
            "#,
                    &[&contribution_id],
                )
                .await
                .map_err(|e| RegistryError::StorageError(format!("Query failed: {}", e)))?;

            rows.iter().map(review_from_row).collect()
        }

        async fn get_consensus(
            &self,
            contribution_id: Uuid,
        ) -> Result<Option<ContributionConsensus>> {
            let client = self.pool.get().await.map_err(|e| {
                RegistryError::StorageError(format!("Failed to get connection: {}", e))
            })?;

            let row = client
                .query_opt(
                    r#"
                SELECT total_reviews, approve_count, reject_count, avg_confidence
                FROM contribution_consensus
                WHERE contribution_id = $1
            "#,
                    &[&contribution_id],
                )
                .await
                .map_err(|e| RegistryError::StorageError(format!("Query failed: {}", e)))?;

            match row {
                Some(r) => {
                    let total: i64 = r.get(0);
                    if total == 0 {
                        return Ok(None);
                    }
                    Ok(Some(ContributionConsensus {
                        total_reviews: total as u64,
                        approve_count: r.get::<_, i64>(1) as u64,
                        reject_count: r.get::<_, i64>(2) as u64,
                        avg_confidence: r.get::<_, Option<f64>>(3).unwrap_or(0.0) as f32,
                    }))
                }
                None => Ok(None),
            }
        }
    }

    /// Parse a StoredContribution from a tokio_postgres Row.
    fn contribution_from_row(row: &tokio_postgres::Row) -> Result<StoredContribution> {
        let status_str: String = row.get("status");
        let status = match status_str.as_str() {
            "submitted" => ContributionStatus::Submitted,
            "underreview" | "under_review" => ContributionStatus::UnderReview,
            "awaitingconsensus" | "awaiting_consensus" => ContributionStatus::AwaitingConsensus,
            "approved" => ContributionStatus::Approved,
            "rejected" => ContributionStatus::Rejected,
            "closed" => ContributionStatus::Closed,
            "autoapproved" | "auto_approved" => ContributionStatus::AutoApproved,
            "merged" => ContributionStatus::Merged,
            _ => ContributionStatus::Submitted,
        };

        Ok(StoredContribution {
            id: row.get("id"),
            contribution_type: row.get("contribution_type"),
            package_hash: row.get("package_hash"),
            title: row.get("title"),
            description: row.get("description"),
            status,
            reporter_public_key: row.get("reporter_public_key"),
            reporter_name: row.get("reporter_name"),
            reporter_app_id: row.get("reporter_app_id"),
            reporter_is_ai: row.get("reporter_is_ai"),
            data: row.get("data"),
            signature: row.get("signature"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
    }

    /// Parse a StoredReview from a tokio_postgres Row.
    fn review_from_row(row: &tokio_postgres::Row) -> Result<StoredReview> {
        Ok(StoredReview {
            id: row.get("id"),
            contribution_id: row.get("contribution_id"),
            reviewer_app_id: row.get("reviewer_app_id"),
            reviewer_public_key: row.get("reviewer_public_key"),
            reviewer_name: row.get("reviewer_name"),
            reviewer_is_ai: row.get("reviewer_is_ai"),
            verdict: row.get("verdict"),
            confidence: row.get("confidence"),
            justification: row.get("justification"),
            signature: row.get("signature"),
            created_at: row.get("created_at"),
        })
    }

    /// Parse a StoredApiKey from a tokio_postgres Row.
    fn api_key_from_row(row: &tokio_postgres::Row) -> Result<StoredApiKey> {
        let trust_level_str: String = row.get("trust_level");
        let trust_level = trust_level_str
            .parse::<ApiKeyTrustLevel>()
            .unwrap_or(ApiKeyTrustLevel::Basic);
        let scopes: Vec<String> = row.get("scopes");
        let rate_limit: Option<i32> = row.get("rate_limit_rpm");

        Ok(StoredApiKey {
            id: row.get("id"),
            key_hash: row.get("key_hash"),
            key_prefix: row.get("key_prefix"),
            agent_id: row.get("agent_id"),
            name: row.get("name"),
            trust_level,
            scopes,
            rate_limit_rpm: rate_limit.map(|r| r as u32),
            active: row.get("active"),
            expires_at: row.get("expires_at"),
            last_used_at: row.get("last_used_at"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
    }

    #[async_trait]
    impl ApiKeyStore for PostgresMetadataStore {
        async fn store_api_key(&self, key: &StoredApiKey) -> Result<Uuid> {
            let client = self.pool.get().await.map_err(|e| {
                RegistryError::StorageError(format!("Failed to get connection: {}", e))
            })?;

            let trust_level_str = key.trust_level.to_string();
            let rate_limit = key.rate_limit_rpm.map(|r| r as i32);

            client
                .execute(
                    r#"
                INSERT INTO api_keys (
                    id, key_hash, key_prefix, agent_id, name, trust_level,
                    scopes, rate_limit_rpm, active, expires_at, last_used_at,
                    created_at, updated_at
                ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            "#,
                    &[
                        &key.id,
                        &key.key_hash,
                        &key.key_prefix,
                        &key.agent_id,
                        &key.name,
                        &trust_level_str,
                        &key.scopes,
                        &rate_limit,
                        &key.active,
                        &key.expires_at,
                        &key.last_used_at,
                        &key.created_at,
                        &key.updated_at,
                    ],
                )
                .await
                .map_err(|e| {
                    RegistryError::StorageError(format!("Failed to store API key: {}", e))
                })?;

            Ok(key.id)
        }

        async fn get_api_key_by_hash(&self, key_hash: &str) -> Result<Option<StoredApiKey>> {
            let client = self.pool.get().await.map_err(|e| {
                RegistryError::StorageError(format!("Failed to get connection: {}", e))
            })?;

            let row = client
                .query_opt(
                    r#"
                SELECT id, key_hash, key_prefix, agent_id, name, trust_level,
                       scopes, rate_limit_rpm, active, expires_at, last_used_at,
                       created_at, updated_at
                FROM api_keys
                WHERE key_hash = $1
            "#,
                    &[&key_hash],
                )
                .await
                .map_err(|e| RegistryError::StorageError(format!("Query failed: {}", e)))?;

            row.map(|r| api_key_from_row(&r)).transpose()
        }

        async fn get_api_key(&self, id: Uuid) -> Result<Option<StoredApiKey>> {
            let client = self.pool.get().await.map_err(|e| {
                RegistryError::StorageError(format!("Failed to get connection: {}", e))
            })?;

            let row = client
                .query_opt(
                    r#"
                SELECT id, key_hash, key_prefix, agent_id, name, trust_level,
                       scopes, rate_limit_rpm, active, expires_at, last_used_at,
                       created_at, updated_at
                FROM api_keys
                WHERE id = $1
            "#,
                    &[&id],
                )
                .await
                .map_err(|e| RegistryError::StorageError(format!("Query failed: {}", e)))?;

            row.map(|r| api_key_from_row(&r)).transpose()
        }

        async fn verify_api_key(&self, key_hash: &str) -> Result<ApiKeyVerification> {
            match self.get_api_key_by_hash(key_hash).await? {
                Some(key) => {
                    // Check if key is active
                    if !key.active {
                        return Ok(ApiKeyVerification {
                            key,
                            valid: false,
                            invalid_reason: Some("API key has been revoked".to_string()),
                        });
                    }

                    // Check expiration
                    if let Some(expires_at) = key.expires_at {
                        if Utc::now() > expires_at {
                            return Ok(ApiKeyVerification {
                                key,
                                valid: false,
                                invalid_reason: Some("API key has expired".to_string()),
                            });
                        }
                    }

                    Ok(ApiKeyVerification {
                        key,
                        valid: true,
                        invalid_reason: None,
                    })
                }
                None => Err(RegistryError::NotFound("API key not found".to_string())),
            }
        }

        async fn touch_api_key(&self, key_hash: &str) -> Result<()> {
            let client = self.pool.get().await.map_err(|e| {
                RegistryError::StorageError(format!("Failed to get connection: {}", e))
            })?;

            let rows_affected = client
                .execute(
                    "UPDATE api_keys SET last_used_at = NOW() WHERE key_hash = $1",
                    &[&key_hash],
                )
                .await
                .map_err(|e| RegistryError::StorageError(format!("Update failed: {}", e)))?;

            if rows_affected == 0 {
                Err(RegistryError::NotFound("API key not found".to_string()))
            } else {
                Ok(())
            }
        }

        async fn revoke_api_key(&self, id: Uuid) -> Result<()> {
            let client = self.pool.get().await.map_err(|e| {
                RegistryError::StorageError(format!("Failed to get connection: {}", e))
            })?;

            let rows_affected = client
                .execute("UPDATE api_keys SET active = FALSE WHERE id = $1", &[&id])
                .await
                .map_err(|e| RegistryError::StorageError(format!("Update failed: {}", e)))?;

            if rows_affected == 0 {
                Err(RegistryError::NotFound(format!("API key {} not found", id)))
            } else {
                Ok(())
            }
        }

        async fn list_api_keys(
            &self,
            agent_id: Option<Uuid>,
            limit: usize,
            offset: usize,
        ) -> Result<Vec<StoredApiKey>> {
            let client = self.pool.get().await.map_err(|e| {
                RegistryError::StorageError(format!("Failed to get connection: {}", e))
            })?;

            let limit_i64 = limit as i64;
            let offset_i64 = offset as i64;

            let rows = match agent_id {
                Some(aid) => {
                    client
                        .query(
                            r#"
                        SELECT id, key_hash, key_prefix, agent_id, name, trust_level,
                               scopes, rate_limit_rpm, active, expires_at, last_used_at,
                               created_at, updated_at
                        FROM api_keys
                        WHERE agent_id = $1
                        ORDER BY created_at DESC
                        LIMIT $2 OFFSET $3
                    "#,
                            &[&aid, &limit_i64, &offset_i64],
                        )
                        .await
                }
                None => {
                    client
                        .query(
                            r#"
                        SELECT id, key_hash, key_prefix, agent_id, name, trust_level,
                               scopes, rate_limit_rpm, active, expires_at, last_used_at,
                               created_at, updated_at
                        FROM api_keys
                        ORDER BY created_at DESC
                        LIMIT $1 OFFSET $2
                    "#,
                            &[&limit_i64, &offset_i64],
                        )
                        .await
                }
            }
            .map_err(|e| RegistryError::StorageError(format!("Query failed: {}", e)))?;

            rows.iter().map(api_key_from_row).collect()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::package::{PackageManifest, TrustLevel};

    fn create_test_package(name: &str, version: &str) -> PackageInfo {
        let manifest = PackageManifest::builder()
            .name(name)
            .version(version)
            .description("A test package")
            .keywords(vec!["test", "example"])
            .build()
            .unwrap();

        let hash = ContentHash::from_bytes(format!("{}-{}", name, version).as_bytes());

        PackageInfo {
            hash,
            manifest,
            published_at: Utc::now(),
            publisher_key_id: "test-key".to_string(),
            downloads: 0,
            trust_level: TrustLevel::Community,
            lineage: None,
            yanked: false,
        }
    }

    #[tokio::test]
    async fn test_in_memory_store_package() {
        let store = InMemoryMetadataStore::new();
        let pkg = create_test_package("test-pkg", "1.0.0");

        store.store_package(&pkg).await.unwrap();

        let retrieved = store.get_by_hash(&pkg.hash).await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().manifest.name, "test-pkg");
    }

    #[tokio::test]
    async fn test_in_memory_resolve() {
        let store = InMemoryMetadataStore::new();

        let pkg1 = create_test_package("my-package", "1.0.0");
        let pkg2 = create_test_package("my-package", "1.1.0");
        let pkg3 = create_test_package("my-package", "2.0.0");

        store.store_package(&pkg1).await.unwrap();
        store.store_package(&pkg2).await.unwrap();
        store.store_package(&pkg3).await.unwrap();

        // Resolve ^1.0 should get 1.1.0
        let version_req = VersionReq::parse("^1.0").unwrap();
        let resolution = store.resolve("my-package", &version_req).await.unwrap();
        assert!(resolution.is_some());
        assert_eq!(resolution.unwrap().version.to_string(), "1.1.0");

        // Resolve >=2.0 should get 2.0.0
        let version_req = VersionReq::parse(">=2.0").unwrap();
        let resolution = store.resolve("my-package", &version_req).await.unwrap();
        assert!(resolution.is_some());
        assert_eq!(resolution.unwrap().version.to_string(), "2.0.0");
    }

    #[tokio::test]
    async fn test_in_memory_resolve_latest() {
        let store = InMemoryMetadataStore::new();

        let pkg1 = create_test_package("latest-pkg", "1.0.0");
        let pkg2 = create_test_package("latest-pkg", "2.0.0");

        store.store_package(&pkg1).await.unwrap();
        store.store_package(&pkg2).await.unwrap();

        let resolution = store.resolve_latest("latest-pkg").await.unwrap();
        assert!(resolution.is_some());
        assert_eq!(resolution.unwrap().version.to_string(), "2.0.0");
    }

    #[tokio::test]
    async fn test_in_memory_list_versions() {
        let store = InMemoryMetadataStore::new();

        let pkg1 = create_test_package("multi-version", "1.0.0");
        let pkg2 = create_test_package("multi-version", "1.1.0");
        let pkg3 = create_test_package("multi-version", "2.0.0");

        store.store_package(&pkg1).await.unwrap();
        store.store_package(&pkg2).await.unwrap();
        store.store_package(&pkg3).await.unwrap();

        let versions = store.list_versions("multi-version").await.unwrap();
        assert_eq!(versions.len(), 3);
    }

    #[tokio::test]
    async fn test_in_memory_yank() {
        let store = InMemoryMetadataStore::new();
        let pkg = create_test_package("yank-test", "1.0.0");

        store.store_package(&pkg).await.unwrap();

        // Yank
        store.yank(&pkg.hash).await.unwrap();

        let retrieved = store.get_by_hash(&pkg.hash).await.unwrap().unwrap();
        assert!(retrieved.yanked);

        // Unyank
        store.unyank(&pkg.hash).await.unwrap();

        let retrieved = store.get_by_hash(&pkg.hash).await.unwrap().unwrap();
        assert!(!retrieved.yanked);
    }

    #[tokio::test]
    async fn test_in_memory_search() {
        let store = InMemoryMetadataStore::new();

        let pkg1 = create_test_package("sentiment-analyzer", "1.0.0");
        let pkg2 = create_test_package("code-generator", "1.0.0");

        store.store_package(&pkg1).await.unwrap();
        store.store_package(&pkg2).await.unwrap();

        let results = store.search_keyword("sentiment", 10, 0).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].manifest.name, "sentiment-analyzer");
    }

    #[tokio::test]
    async fn test_in_memory_downloads() {
        let store = InMemoryMetadataStore::new();
        let pkg = create_test_package("download-test", "1.0.0");

        store.store_package(&pkg).await.unwrap();

        let count1 = store.increment_downloads(&pkg.hash).await.unwrap();
        assert_eq!(count1, 1);

        let count2 = store.increment_downloads(&pkg.hash).await.unwrap();
        assert_eq!(count2, 2);
    }

    #[tokio::test]
    async fn test_in_memory_name_exists() {
        let store = InMemoryMetadataStore::new();
        let pkg = create_test_package("exists-test", "1.0.0");

        assert!(!store.name_exists("exists-test").await.unwrap());

        store.store_package(&pkg).await.unwrap();

        assert!(store.name_exists("exists-test").await.unwrap());
    }

    // ============================================================================
    // API Key Store Tests
    // ============================================================================

    fn create_test_api_key(name: &str, active: bool) -> StoredApiKey {
        let (_, key_prefix, key_hash) = generate_api_key("dk_test");
        StoredApiKey {
            id: Uuid::new_v4(),
            key_hash,
            key_prefix,
            agent_id: Some(Uuid::new_v4()),
            name: name.to_string(),
            trust_level: ApiKeyTrustLevel::Basic,
            scopes: vec!["read".to_string()],
            rate_limit_rpm: None,
            active,
            expires_at: None,
            last_used_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn test_api_key_store_and_get() {
        let store = InMemoryMetadataStore::new();
        let key = create_test_api_key("Test Key", true);
        let key_hash = key.key_hash.clone();
        let key_id = key.id;

        // Store key
        let stored_id = store.store_api_key(&key).await.unwrap();
        assert_eq!(stored_id, key_id);

        // Get by hash
        let retrieved = store.get_api_key_by_hash(&key_hash).await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "Test Key");

        // Get by ID
        let retrieved = store.get_api_key(key_id).await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id, key_id);
    }

    #[tokio::test]
    async fn test_api_key_verify_valid() {
        let store = InMemoryMetadataStore::new();
        let key = create_test_api_key("Valid Key", true);
        let key_hash = key.key_hash.clone();

        store.store_api_key(&key).await.unwrap();

        let verification = store.verify_api_key(&key_hash).await.unwrap();
        assert!(verification.valid);
        assert!(verification.invalid_reason.is_none());
    }

    #[tokio::test]
    async fn test_api_key_verify_revoked() {
        let store = InMemoryMetadataStore::new();
        let key = create_test_api_key("Revoked Key", false);
        let key_hash = key.key_hash.clone();

        store.store_api_key(&key).await.unwrap();

        let verification = store.verify_api_key(&key_hash).await.unwrap();
        assert!(!verification.valid);
        assert!(verification.invalid_reason.is_some());
        assert!(verification.invalid_reason.unwrap().contains("revoked"));
    }

    #[tokio::test]
    async fn test_api_key_verify_expired() {
        let store = InMemoryMetadataStore::new();
        let mut key = create_test_api_key("Expired Key", true);
        key.expires_at = Some(Utc::now() - chrono::Duration::hours(1));
        let key_hash = key.key_hash.clone();

        store.store_api_key(&key).await.unwrap();

        let verification = store.verify_api_key(&key_hash).await.unwrap();
        assert!(!verification.valid);
        assert!(verification.invalid_reason.is_some());
        assert!(verification.invalid_reason.unwrap().contains("expired"));
    }

    #[tokio::test]
    async fn test_api_key_revoke() {
        let store = InMemoryMetadataStore::new();
        let key = create_test_api_key("To Revoke", true);
        let key_hash = key.key_hash.clone();
        let key_id = key.id;

        store.store_api_key(&key).await.unwrap();

        // Verify it's valid first
        let verification = store.verify_api_key(&key_hash).await.unwrap();
        assert!(verification.valid);

        // Revoke it
        store.revoke_api_key(key_id).await.unwrap();

        // Should be invalid now
        let verification = store.verify_api_key(&key_hash).await.unwrap();
        assert!(!verification.valid);
    }

    #[tokio::test]
    async fn test_api_key_touch() {
        let store = InMemoryMetadataStore::new();
        let key = create_test_api_key("Touch Test", true);
        let key_hash = key.key_hash.clone();

        store.store_api_key(&key).await.unwrap();

        // Initially last_used_at is None
        let before = store.get_api_key_by_hash(&key_hash).await.unwrap().unwrap();
        assert!(before.last_used_at.is_none());

        // Touch the key
        store.touch_api_key(&key_hash).await.unwrap();

        // Now last_used_at should be set
        let after = store.get_api_key_by_hash(&key_hash).await.unwrap().unwrap();
        assert!(after.last_used_at.is_some());
    }

    #[tokio::test]
    async fn test_api_key_list() {
        let store = InMemoryMetadataStore::new();

        let agent_id = Uuid::new_v4();
        let mut key1 = create_test_api_key("Key 1", true);
        key1.agent_id = Some(agent_id);
        let mut key2 = create_test_api_key("Key 2", true);
        key2.agent_id = Some(agent_id);
        let key3 = create_test_api_key("Key 3 - Different Agent", true);

        store.store_api_key(&key1).await.unwrap();
        store.store_api_key(&key2).await.unwrap();
        store.store_api_key(&key3).await.unwrap();

        // List all keys
        let all = store.list_api_keys(None, 10, 0).await.unwrap();
        assert_eq!(all.len(), 3);

        // List only agent's keys
        let agent_keys = store.list_api_keys(Some(agent_id), 10, 0).await.unwrap();
        assert_eq!(agent_keys.len(), 2);
    }

    #[test]
    fn test_generate_api_key() {
        let (full_key, key_prefix, key_hash) = generate_api_key("dk_test");

        // Full key should start with prefix
        assert!(full_key.starts_with("dk_test_"));

        // Key prefix should be shorter
        assert!(key_prefix.starts_with("dk_test_"));
        assert!(key_prefix.len() < full_key.len());

        // Hash should be valid hex
        assert_eq!(key_hash.len(), 64); // SHA-256 = 32 bytes = 64 hex chars

        // Verify hash matches the key
        let computed_hash = hash_api_key(&full_key);
        assert_eq!(computed_hash, key_hash);
    }
}
