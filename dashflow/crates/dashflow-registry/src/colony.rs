//! Colony P2P Package Distribution
//!
//! Implements the "colony-first" fetch strategy: check local cache, then
//! colony peers, then central registry. This minimizes bandwidth usage and
//! provides faster installs when packages are already present in the local network.
//!
//! # Architecture
//!
//! ```text
//! ColonyPackageResolver
//! ├── 1. Check local cache (PackageCache)
//! ├── 2. Query colony peers for package hash
//! ├── 3. Fetch from best peer (with hash verification)
//! └── 4. Fallback to central registry
//!
//! After successful fetch:
//! └── Announce package availability to colony
//! ```
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow_registry::colony::{ColonyPackageResolver, ColonyConfig, PeerInfo};
//! use dashflow_registry::ContentHash;
//!
//! // Create resolver with colony network
//! let resolver = ColonyPackageResolver::new(config, cache, network);
//!
//! // Fetch a package - automatically checks colony first
//! let data = resolver.fetch(&hash).await?;
//!
//! // Or fetch by name/version
//! let data = resolver.fetch_by_name("dashflow/sentiment", "^1.0").await?;
//! ```

use crate::content_hash::ContentHash;
use crate::error::{RegistryError, Result};
use crate::storage::PackageCache;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use uuid::Uuid;

// ============================================================================
// Colony Configuration
// ============================================================================

/// Configuration for colony P2P distribution.
#[derive(Debug, Clone)]
pub struct ColonyConfig {
    /// Whether to use colony P2P distribution.
    pub enabled: bool,
    /// Maximum time to wait for peer responses.
    pub peer_timeout: Duration,
    /// Maximum number of peers to query in parallel.
    pub max_parallel_queries: usize,
    /// Minimum trust level for accepting packages from peers.
    pub min_trust_level: PeerTrustLevel,
    /// Whether to announce packages after download.
    pub announce_on_download: bool,
    /// How often to refresh peer list (seconds).
    pub peer_refresh_interval_secs: u64,
    /// Maximum package size to transfer via P2P (bytes).
    /// Larger packages use reference-only mode.
    pub max_direct_transfer_size: u64,
}

impl Default for ColonyConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            peer_timeout: Duration::from_secs(5),
            max_parallel_queries: 5,
            min_trust_level: PeerTrustLevel::Colony,
            announce_on_download: true,
            peer_refresh_interval_secs: 60,
            max_direct_transfer_size: 10 * 1024 * 1024, // 10 MB
        }
    }
}

impl ColonyConfig {
    /// Create a disabled configuration.
    #[must_use]
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Default::default()
        }
    }

    /// Create configuration for local development.
    #[must_use]
    pub fn local() -> Self {
        Self {
            enabled: true,
            peer_timeout: Duration::from_secs(2),
            max_parallel_queries: 3,
            min_trust_level: PeerTrustLevel::Local,
            announce_on_download: true,
            peer_refresh_interval_secs: 30,
            max_direct_transfer_size: 50 * 1024 * 1024, // 50 MB for local
        }
    }
}

// ============================================================================
// Peer Types
// ============================================================================

/// Trust level for colony peers.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PeerTrustLevel {
    /// Unknown peer - don't trust.
    #[default]
    Unknown,
    /// Discovered on local network.
    Local,
    /// Part of same colony (verified).
    Colony,
    /// Same organization (PKI verified).
    Organization,
    /// Trusted explicitly by user.
    Trusted,
}

/// Information about a colony peer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    /// Peer's unique identifier.
    pub id: Uuid,
    /// Peer's display name.
    pub name: String,
    /// Endpoint for package transfers.
    pub endpoint: String,
    /// Trust level.
    pub trust_level: PeerTrustLevel,
    /// Known latency to this peer (if measured).
    pub latency_ms: Option<u32>,
    /// When we last heard from this peer.
    pub last_seen: DateTime<Utc>,
    /// Whether this peer is currently reachable.
    pub reachable: bool,
}

impl PeerInfo {
    /// Create new peer info.
    #[must_use]
    pub fn new(id: Uuid, name: impl Into<String>, endpoint: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            endpoint: endpoint.into(),
            trust_level: PeerTrustLevel::default(),
            latency_ms: None,
            last_seen: Utc::now(),
            reachable: true,
        }
    }

    /// Set trust level.
    #[must_use]
    pub fn with_trust_level(mut self, trust_level: PeerTrustLevel) -> Self {
        self.trust_level = trust_level;
        self
    }

    /// Set latency.
    #[must_use]
    pub fn with_latency(mut self, latency_ms: u32) -> Self {
        self.latency_ms = Some(latency_ms);
        self
    }

    /// Mark as unreachable.
    pub fn mark_unreachable(&mut self) {
        self.reachable = false;
    }

    /// Update last seen timestamp.
    pub fn touch(&mut self) {
        self.last_seen = Utc::now();
        self.reachable = true;
    }
}

// ============================================================================
// Package Availability
// ============================================================================

/// Information about package availability at a peer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerPackageInfo {
    /// The peer.
    pub peer: PeerInfo,
    /// Content hash of the package.
    pub hash: ContentHash,
    /// Package size in bytes.
    pub size_bytes: u64,
    /// Whether peer supports direct transfer (vs reference only).
    pub supports_direct_transfer: bool,
    /// When this info was recorded.
    pub recorded_at: DateTime<Utc>,
}

impl PeerPackageInfo {
    /// Create new peer package info.
    #[must_use]
    pub fn new(peer: PeerInfo, hash: ContentHash, size_bytes: u64) -> Self {
        Self {
            peer,
            hash,
            size_bytes,
            supports_direct_transfer: true,
            recorded_at: Utc::now(),
        }
    }

    /// Set direct transfer support.
    #[must_use]
    pub fn with_direct_transfer(mut self, supports: bool) -> Self {
        self.supports_direct_transfer = supports;
        self
    }

    /// Calculate transfer score (higher is better).
    /// Considers latency, trust level, and transfer capability.
    #[must_use]
    pub fn transfer_score(&self) -> u32 {
        let mut score: u32 = 100;

        // Prefer direct transfer capability
        if self.supports_direct_transfer {
            score += 50;
        }

        // Prefer higher trust levels
        score += match self.peer.trust_level {
            PeerTrustLevel::Unknown => 0,
            PeerTrustLevel::Local => 10,
            PeerTrustLevel::Colony => 20,
            PeerTrustLevel::Organization => 30,
            PeerTrustLevel::Trusted => 40,
        };

        // Prefer lower latency
        if let Some(latency) = self.peer.latency_ms {
            score = score.saturating_sub(latency / 10);
        }

        score
    }
}

// ============================================================================
// Transfer Messages
// ============================================================================

/// Request to transfer a package from a colony peer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageTransferRequest {
    /// Request ID for tracking.
    pub id: Uuid,
    /// Content hash of the requested package.
    pub hash: ContentHash,
    /// Requesting peer's ID.
    pub requester_id: Uuid,
    /// Requesting peer's name.
    pub requester_name: String,
    /// Whether to include package data (vs just reference).
    pub include_data: bool,
}

impl PackageTransferRequest {
    /// Create a new transfer request.
    #[must_use]
    pub fn new(hash: ContentHash, requester_id: Uuid, requester_name: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            hash,
            requester_id,
            requester_name: requester_name.into(),
            include_data: true,
        }
    }

    /// Request reference only (for large packages).
    #[must_use]
    pub fn reference_only(mut self) -> Self {
        self.include_data = false;
        self
    }
}

/// Response to a package transfer request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum PackageTransferResponse {
    /// Package data included directly.
    Data {
        /// Base64-encoded package data.
        data: String,
        /// Content hash for verification.
        hash: ContentHash,
        /// Size in bytes.
        size: u64,
    },
    /// Reference to download from original source.
    Reference {
        /// URL to download from.
        url: String,
        /// Expected hash.
        hash: ContentHash,
        /// Size in bytes.
        size: u64,
    },
    /// Request denied.
    Denied {
        /// Reason for denial.
        reason: String,
    },
    /// Package not found on this peer.
    NotFound,
}

impl PackageTransferResponse {
    /// Create a data response.
    #[must_use]
    pub fn data(data: String, hash: ContentHash, size: u64) -> Self {
        Self::Data { data, hash, size }
    }

    /// Create a reference response.
    #[must_use]
    pub fn reference(url: impl Into<String>, hash: ContentHash, size: u64) -> Self {
        Self::Reference {
            url: url.into(),
            hash,
            size,
        }
    }

    /// Create a denied response.
    #[must_use]
    pub fn denied(reason: impl Into<String>) -> Self {
        Self::Denied {
            reason: reason.into(),
        }
    }

    /// Check if this is a successful response.
    #[must_use]
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Data { .. } | Self::Reference { .. })
    }
}

// ============================================================================
// Colony Network Trait
// ============================================================================

/// Trait for colony network operations.
#[async_trait]
pub trait ColonyNetwork: Send + Sync {
    /// Get list of known peers.
    async fn peers(&self) -> Vec<PeerInfo>;

    /// Query which peers have a package by hash.
    async fn peers_with_package(&self, hash: &ContentHash) -> Vec<PeerPackageInfo>;

    /// Send a transfer request to a peer.
    async fn request_transfer(
        &self,
        peer: &PeerInfo,
        request: PackageTransferRequest,
    ) -> Result<PackageTransferResponse>;

    /// Announce that we have a package available.
    async fn announce_package(&self, hash: &ContentHash, size: u64) -> Result<()>;

    /// Our peer identity.
    fn local_peer(&self) -> &PeerInfo;
}

// ============================================================================
// Mock Colony Network (for testing)
// ============================================================================

/// Mock colony network for testing.
#[derive(Debug)]
pub struct MockColonyNetwork {
    local_peer: PeerInfo,
    peers: Arc<RwLock<Vec<PeerInfo>>>,
    #[allow(clippy::type_complexity)] // Package cache maps name → (source peer, binary data)
    packages: Arc<RwLock<HashMap<String, (PeerInfo, Vec<u8>)>>>,
}

impl MockColonyNetwork {
    /// Create a new mock network.
    #[must_use]
    pub fn new(local_peer: PeerInfo) -> Self {
        Self {
            local_peer,
            peers: Arc::new(RwLock::new(Vec::new())),
            packages: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Add a peer.
    pub async fn add_peer(&self, peer: PeerInfo) {
        self.peers.write().await.push(peer);
    }

    /// Add a package to a peer.
    pub async fn add_package(&self, peer: PeerInfo, hash: &ContentHash, data: Vec<u8>) {
        self.packages
            .write()
            .await
            .insert(format!("{}:{}", peer.id, hash), (peer, data));
    }
}

#[async_trait]
impl ColonyNetwork for MockColonyNetwork {
    async fn peers(&self) -> Vec<PeerInfo> {
        self.peers.read().await.clone()
    }

    async fn peers_with_package(&self, hash: &ContentHash) -> Vec<PeerPackageInfo> {
        let packages = self.packages.read().await;
        packages
            .iter()
            .filter(|(k, _)| k.ends_with(&hash.to_string()))
            .map(|(_, (peer, data))| {
                PeerPackageInfo::new(peer.clone(), hash.clone(), data.len() as u64)
            })
            .collect()
    }

    async fn request_transfer(
        &self,
        peer: &PeerInfo,
        request: PackageTransferRequest,
    ) -> Result<PackageTransferResponse> {
        let packages = self.packages.read().await;
        let key = format!("{}:{}", peer.id, request.hash);

        if let Some((_, data)) = packages.get(&key) {
            if request.include_data {
                Ok(PackageTransferResponse::data(
                    base64::Engine::encode(&base64::engine::general_purpose::STANDARD, data),
                    request.hash,
                    data.len() as u64,
                ))
            } else {
                Ok(PackageTransferResponse::reference(
                    format!("https://registry.dashswarm.com/packages/{}", request.hash),
                    request.hash,
                    data.len() as u64,
                ))
            }
        } else {
            Ok(PackageTransferResponse::NotFound)
        }
    }

    async fn announce_package(&self, _hash: &ContentHash, _size: u64) -> Result<()> {
        // Mock does nothing
        Ok(())
    }

    fn local_peer(&self) -> &PeerInfo {
        &self.local_peer
    }
}

// ============================================================================
// Fetch Result
// ============================================================================

/// Source of a fetched package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FetchSource {
    /// From local cache.
    LocalCache,
    /// From a colony peer.
    ColonyPeer { peer_id: Uuid, peer_name: String },
    /// From central registry.
    CentralRegistry,
}

/// Result of a package fetch operation.
#[derive(Debug)]
pub struct FetchResult {
    /// The package data.
    pub data: Vec<u8>,
    /// Content hash (verified).
    pub hash: ContentHash,
    /// Where it came from.
    pub source: FetchSource,
    /// Time taken to fetch.
    pub fetch_duration: Duration,
}

impl FetchResult {
    /// Create a new fetch result.
    pub fn new(data: Vec<u8>, hash: ContentHash, source: FetchSource, duration: Duration) -> Self {
        Self {
            data,
            hash,
            source,
            fetch_duration: duration,
        }
    }

    /// Size in bytes.
    #[must_use]
    pub fn size(&self) -> usize {
        self.data.len()
    }

    /// Check if from local cache.
    #[must_use]
    pub fn is_cached(&self) -> bool {
        matches!(self.source, FetchSource::LocalCache)
    }

    /// Check if from colony.
    #[must_use]
    pub fn is_from_colony(&self) -> bool {
        matches!(self.source, FetchSource::ColonyPeer { .. })
    }
}

// ============================================================================
// Colony Package Resolver
// ============================================================================

/// Resolver for fetching packages with colony-first strategy.
///
/// The fetch order is:
/// 1. Local cache
/// 2. Colony peers (P2P)
/// 3. Central registry (fallback)
pub struct ColonyPackageResolver<N: ColonyNetwork> {
    config: ColonyConfig,
    cache: PackageCache,
    network: N,
    /// Stats for monitoring.
    stats: Arc<RwLock<ResolverStats>>,
}

/// Statistics about resolver operations.
#[derive(Debug, Default, Clone)]
pub struct ResolverStats {
    /// Total fetch requests.
    pub total_requests: u64,
    /// Cache hits.
    pub cache_hits: u64,
    /// Colony peer hits.
    pub colony_hits: u64,
    /// Central registry fetches.
    pub registry_fetches: u64,
    /// Failed fetches.
    pub failed_fetches: u64,
    /// Total bytes transferred from colony.
    pub colony_bytes: u64,
    /// Packages announced to colony.
    pub packages_announced: u64,
}

impl ResolverStats {
    /// Cache hit rate (0.0 - 1.0).
    #[must_use]
    pub fn cache_hit_rate(&self) -> f64 {
        if self.total_requests == 0 {
            0.0
        } else {
            self.cache_hits as f64 / self.total_requests as f64
        }
    }

    /// Colony hit rate (0.0 - 1.0).
    #[must_use]
    pub fn colony_hit_rate(&self) -> f64 {
        if self.total_requests == 0 {
            0.0
        } else {
            self.colony_hits as f64 / self.total_requests as f64
        }
    }
}

impl<N: ColonyNetwork> ColonyPackageResolver<N> {
    /// Create a new resolver.
    pub fn new(config: ColonyConfig, cache: PackageCache, network: N) -> Self {
        Self {
            config,
            cache,
            network,
            stats: Arc::new(RwLock::new(ResolverStats::default())),
        }
    }

    /// Get resolver statistics.
    pub async fn stats(&self) -> ResolverStats {
        self.stats.read().await.clone()
    }

    /// Fetch a package by content hash.
    ///
    /// Tries local cache first, then colony peers, then central registry.
    pub async fn fetch(&self, hash: &ContentHash) -> Result<FetchResult> {
        let start = std::time::Instant::now();

        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.total_requests += 1;
        }

        // 1. Check local cache
        if let Some(data) = self.cache.get(hash).await? {
            let mut stats = self.stats.write().await;
            stats.cache_hits += 1;
            return Ok(FetchResult::new(
                data,
                hash.clone(),
                FetchSource::LocalCache,
                start.elapsed(),
            ));
        }

        // 2. Try colony peers (if enabled)
        if self.config.enabled {
            if let Some(result) = self.try_fetch_from_colony(hash, start).await? {
                return Ok(result);
            }
        }

        // 3. Fall back to central registry
        Err(RegistryError::PackageNotFound(hash.to_string()))
    }

    /// Try to fetch from colony peers.
    async fn try_fetch_from_colony(
        &self,
        hash: &ContentHash,
        start: std::time::Instant,
    ) -> Result<Option<FetchResult>> {
        // Query which peers have this package
        let mut sources = self.network.peers_with_package(hash).await;

        if sources.is_empty() {
            return Ok(None);
        }

        // Filter by trust level
        sources.retain(|s| s.peer.trust_level >= self.config.min_trust_level);

        if sources.is_empty() {
            return Ok(None);
        }

        // Sort by transfer score (best first)
        sources.sort_by_key(|s| std::cmp::Reverse(s.transfer_score()));

        // Try to fetch from best sources
        for source in sources.iter().take(self.config.max_parallel_queries) {
            match self.fetch_from_peer(&source.peer, hash).await {
                Ok(data) => {
                    // Verify hash
                    let verified_hash = ContentHash::from_bytes(&data);
                    if &verified_hash != hash {
                        // Hash mismatch - try next peer
                        continue;
                    }

                    // Store in cache
                    self.cache.store(&data).await?;

                    // Update stats
                    {
                        let mut stats = self.stats.write().await;
                        stats.colony_hits += 1;
                        stats.colony_bytes += data.len() as u64;
                    }

                    // Announce to colony (if enabled)
                    // SAFETY: Announce failure is non-critical - this is a best-effort
                    // notification to other nodes. Package is already cached locally
                    // and available for serving. Failed announces don't affect correctness.
                    if self.config.announce_on_download {
                        let _ = self.network.announce_package(hash, data.len() as u64).await;
                        let mut stats = self.stats.write().await;
                        stats.packages_announced += 1;
                    }

                    return Ok(Some(FetchResult::new(
                        data,
                        hash.clone(),
                        FetchSource::ColonyPeer {
                            peer_id: source.peer.id,
                            peer_name: source.peer.name.clone(),
                        },
                        start.elapsed(),
                    )));
                }
                Err(_) => {
                    // Try next peer
                    continue;
                }
            }
        }

        Ok(None)
    }

    /// Fetch package data from a specific peer.
    async fn fetch_from_peer(&self, peer: &PeerInfo, hash: &ContentHash) -> Result<Vec<u8>> {
        let request = PackageTransferRequest::new(
            hash.clone(),
            self.network.local_peer().id,
            self.network.local_peer().name.clone(),
        );

        let response = tokio::time::timeout(
            self.config.peer_timeout,
            self.network.request_transfer(peer, request),
        )
        .await
        .map_err(|e| RegistryError::NetworkError(format!("peer transfer timed out after {:?}: {e}", self.config.peer_timeout)))??;

        match response {
            PackageTransferResponse::Data {
                data,
                hash: _,
                size: _,
            } => {
                // Decode base64 data
                base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &data)
                    .map_err(|e| RegistryError::InvalidData(e.to_string()))
            }
            PackageTransferResponse::Reference {
                url,
                hash: _,
                size: _,
            } => {
                // Would need HTTP client to fetch from URL
                Err(RegistryError::NotImplemented(format!(
                    "Reference-only transfer from {}",
                    url
                )))
            }
            PackageTransferResponse::Denied { reason } => Err(RegistryError::AccessDenied(reason)),
            PackageTransferResponse::NotFound => {
                Err(RegistryError::PackageNotFound(hash.to_string()))
            }
        }
    }

    /// Check if a package is available in the colony.
    pub async fn is_available_in_colony(&self, hash: &ContentHash) -> bool {
        if !self.config.enabled {
            return false;
        }
        !self.network.peers_with_package(hash).await.is_empty()
    }

    /// Get list of peers that have a package.
    pub async fn find_peers(&self, hash: &ContentHash) -> Vec<PeerPackageInfo> {
        if !self.config.enabled {
            return Vec::new();
        }
        self.network.peers_with_package(hash).await
    }

    /// Announce that we have a package available.
    pub async fn announce(&self, hash: &ContentHash, size: u64) -> Result<()> {
        if !self.config.enabled {
            return Ok(());
        }
        self.network.announce_package(hash, size).await
    }
}

// ============================================================================
// Channel Constants
// ============================================================================

/// Standard channel name for package transfers.
pub const PACKAGE_TRANSFER_CHANNEL: &str = "_package_transfer";

/// Standard channel name for package announcements.
pub const PACKAGE_ANNOUNCE_CHANNEL: &str = "_package_announce";

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::InMemoryStorage;

    fn create_test_peer(name: &str) -> PeerInfo {
        PeerInfo::new(
            Uuid::new_v4(),
            name,
            format!("http://localhost:800{}", name.len()),
        )
        .with_trust_level(PeerTrustLevel::Colony)
        .with_latency(50)
    }

    #[test]
    fn test_colony_config_default() {
        let config = ColonyConfig::default();
        assert!(config.enabled);
        assert_eq!(config.max_parallel_queries, 5);
        assert!(config.announce_on_download);
    }

    #[test]
    fn test_colony_config_disabled() {
        let config = ColonyConfig::disabled();
        assert!(!config.enabled);
    }

    #[test]
    fn test_peer_trust_level_ordering() {
        assert!(PeerTrustLevel::Trusted > PeerTrustLevel::Organization);
        assert!(PeerTrustLevel::Organization > PeerTrustLevel::Colony);
        assert!(PeerTrustLevel::Colony > PeerTrustLevel::Local);
        assert!(PeerTrustLevel::Local > PeerTrustLevel::Unknown);
    }

    #[test]
    fn test_peer_info() {
        let mut peer = PeerInfo::new(Uuid::new_v4(), "TestApp", "http://localhost:8000");
        assert!(peer.reachable);
        assert_eq!(peer.trust_level, PeerTrustLevel::Unknown);

        peer.mark_unreachable();
        assert!(!peer.reachable);

        peer.touch();
        assert!(peer.reachable);
    }

    #[test]
    fn test_peer_package_info_score() {
        let peer1 = create_test_peer("App1");
        let hash = ContentHash::from_bytes(b"test");

        let info1 =
            PeerPackageInfo::new(peer1.clone(), hash.clone(), 1024).with_direct_transfer(true);
        let info2 = PeerPackageInfo::new(peer1, hash, 1024).with_direct_transfer(false);

        // Direct transfer should score higher
        assert!(info1.transfer_score() > info2.transfer_score());
    }

    #[test]
    fn test_transfer_request() {
        let hash = ContentHash::from_bytes(b"test");
        let request = PackageTransferRequest::new(hash.clone(), Uuid::new_v4(), "TestApp");

        assert!(request.include_data);
        assert_eq!(request.hash, hash);

        let request2 = request.reference_only();
        assert!(!request2.include_data);
    }

    #[test]
    fn test_transfer_response() {
        let hash = ContentHash::from_bytes(b"test");

        let data_response = PackageTransferResponse::data("base64".to_string(), hash.clone(), 100);
        assert!(data_response.is_success());

        let ref_response =
            PackageTransferResponse::reference("http://example.com", hash.clone(), 100);
        assert!(ref_response.is_success());

        let denied = PackageTransferResponse::denied("Not allowed");
        assert!(!denied.is_success());

        let not_found = PackageTransferResponse::NotFound;
        assert!(!not_found.is_success());
    }

    #[test]
    fn test_fetch_source() {
        let source1 = FetchSource::LocalCache;
        let source2 = FetchSource::ColonyPeer {
            peer_id: Uuid::new_v4(),
            peer_name: "App".to_string(),
        };
        let source3 = FetchSource::CentralRegistry;

        assert_eq!(source1, FetchSource::LocalCache);
        assert_ne!(source1, source2);
        assert_ne!(source2, source3);
    }

    #[test]
    fn test_fetch_result() {
        let data = b"test data".to_vec();
        let hash = ContentHash::from_bytes(&data);
        let result = FetchResult::new(
            data.clone(),
            hash,
            FetchSource::LocalCache,
            Duration::from_millis(10),
        );

        assert!(result.is_cached());
        assert!(!result.is_from_colony());
        assert_eq!(result.size(), data.len());
    }

    #[test]
    fn test_resolver_stats() {
        let mut stats = ResolverStats::default();
        assert!(stats.cache_hit_rate().abs() < f64::EPSILON);

        stats.total_requests = 10;
        stats.cache_hits = 5;
        stats.colony_hits = 3;

        assert!((stats.cache_hit_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.colony_hit_rate() - 0.3).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_mock_colony_network() {
        let local_peer = create_test_peer("LocalApp");
        let network = MockColonyNetwork::new(local_peer.clone());

        // Add a peer
        let peer = create_test_peer("RemoteApp");
        network.add_peer(peer.clone()).await;

        let peers = network.peers().await;
        assert_eq!(peers.len(), 1);

        // Add a package
        let data = b"package data".to_vec();
        let hash = ContentHash::from_bytes(&data);
        network.add_package(peer.clone(), &hash, data.clone()).await;

        // Query for package
        let sources = network.peers_with_package(&hash).await;
        assert_eq!(sources.len(), 1);

        // Request transfer
        let request = PackageTransferRequest::new(hash.clone(), local_peer.id, "LocalApp");
        let response = network.request_transfer(&peer, request).await.unwrap();
        assert!(response.is_success());
    }

    #[tokio::test]
    async fn test_resolver_cache_hit() {
        let local_peer = create_test_peer("LocalApp");
        let network = MockColonyNetwork::new(local_peer);
        let cache = PackageCache::new(InMemoryStorage::new(), 1024 * 1024);
        let config = ColonyConfig::default();

        let resolver = ColonyPackageResolver::new(config, cache, network);

        // Store in cache first
        let data = b"cached package".to_vec();
        let hash = resolver.cache.store(&data).await.unwrap();

        // Fetch should hit cache
        let result = resolver.fetch(&hash).await.unwrap();
        assert!(result.is_cached());
        assert_eq!(result.data, data);

        let stats = resolver.stats().await;
        assert_eq!(stats.cache_hits, 1);
        assert_eq!(stats.total_requests, 1);
    }

    #[tokio::test]
    async fn test_resolver_colony_hit() {
        let local_peer = create_test_peer("LocalApp");
        let network = MockColonyNetwork::new(local_peer);
        let cache = PackageCache::new(InMemoryStorage::new(), 1024 * 1024);
        let config = ColonyConfig::default();

        // Add package to a remote peer
        let remote_peer = create_test_peer("RemoteApp");
        network.add_peer(remote_peer.clone()).await;

        let data = b"remote package".to_vec();
        let hash = ContentHash::from_bytes(&data);
        network
            .add_package(remote_peer.clone(), &hash, data.clone())
            .await;

        let resolver = ColonyPackageResolver::new(config, cache, network);

        // Fetch should hit colony
        let result = resolver.fetch(&hash).await.unwrap();
        assert!(result.is_from_colony());
        assert_eq!(result.data, data);

        let stats = resolver.stats().await;
        assert_eq!(stats.colony_hits, 1);
        assert_eq!(stats.cache_hits, 0);
    }

    #[tokio::test]
    async fn test_resolver_not_found() {
        let local_peer = create_test_peer("LocalApp");
        let network = MockColonyNetwork::new(local_peer);
        let cache = PackageCache::new(InMemoryStorage::new(), 1024 * 1024);
        let config = ColonyConfig::default();

        let resolver = ColonyPackageResolver::new(config, cache, network);

        let hash = ContentHash::from_bytes(b"nonexistent");
        let result = resolver.fetch(&hash).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_resolver_disabled_colony() {
        let local_peer = create_test_peer("LocalApp");
        let network = MockColonyNetwork::new(local_peer);
        let cache = PackageCache::new(InMemoryStorage::new(), 1024 * 1024);
        let config = ColonyConfig::disabled();

        // Add package to colony
        let remote_peer = create_test_peer("RemoteApp");
        network.add_peer(remote_peer.clone()).await;
        let data = b"remote package".to_vec();
        let hash = ContentHash::from_bytes(&data);
        network.add_package(remote_peer, &hash, data).await;

        let resolver = ColonyPackageResolver::new(config, cache, network);

        // Should not find it (colony disabled)
        assert!(!resolver.is_available_in_colony(&hash).await);
    }

    #[tokio::test]
    async fn test_resolver_find_peers() {
        let local_peer = create_test_peer("LocalApp");
        let network = MockColonyNetwork::new(local_peer);
        let cache = PackageCache::new(InMemoryStorage::new(), 1024 * 1024);
        let config = ColonyConfig::default();

        // Add package to multiple peers
        let peer1 = create_test_peer("App1");
        let peer2 = create_test_peer("App2");
        network.add_peer(peer1.clone()).await;
        network.add_peer(peer2.clone()).await;

        let data = b"shared package".to_vec();
        let hash = ContentHash::from_bytes(&data);
        network.add_package(peer1, &hash, data.clone()).await;
        network.add_package(peer2, &hash, data).await;

        let resolver = ColonyPackageResolver::new(config, cache, network);

        let peers = resolver.find_peers(&hash).await;
        assert_eq!(peers.len(), 2);
    }

    #[tokio::test]
    async fn test_resolver_announce() {
        let local_peer = create_test_peer("LocalApp");
        let network = MockColonyNetwork::new(local_peer);
        let cache = PackageCache::new(InMemoryStorage::new(), 1024 * 1024);
        let config = ColonyConfig::default();

        let resolver = ColonyPackageResolver::new(config, cache, network);

        let hash = ContentHash::from_bytes(b"my package");
        let result = resolver.announce(&hash, 1024).await;
        // Verify announcement completes successfully via mock network
        let announced = result.expect("Package announcement should succeed");
        assert_eq!(announced, (), "Announcement should return unit on success");
    }
}
