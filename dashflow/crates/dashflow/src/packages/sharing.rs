// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

// Allow clippy warnings for package sharing module
// - clone_on_ref_ptr: Shared packages use Arc cloning
// Note: Lock operations now use unwrap_or_else(|e| e.into_inner()) for poison recovery (M-554)
#![allow(clippy::clone_on_ref_ptr, clippy::needless_pass_by_value)]

//! # Colony Package Sharing
//!
//! Packages discovered in colony peers can be shared without re-downloading.
//! Apps advertise their installed packages, and other apps can request them
//! through the P2P network.
//!
//! ## How It Works
//!
//! 1. Apps advertise installed packages on the `_packages` channel
//! 2. Other apps discover available packages in the colony
//! 3. When needing a package, apps can request it from a peer
//! 4. The peer can transfer the package data or provide a reference URL
//!
//! ## Example
//!
//! ```rust,ignore
//! use dashflow::packages::sharing::*;
//!
//! // Create a registry and register local packages
//! let mut registry = ColonyPackageRegistry::new();
//!
//! // Add a locally installed package
//! let pkg = SharedPackage::new(
//!     PackageId::new("dashflow", "sentiment-analysis"),
//!     Version::new(1, 2, 0),
//! ).shareable(true);
//!
//! registry.add_local_package(pkg);
//!
//! // Search for packages in the colony
//! if let Some(source) = registry.find_in_colony(
//!     &PackageId::new("dashflow", "sentiment-analysis"),
//!     &VersionReq::caret(Version::new(1, 0, 0)),
//! ) {
//!     println!("Found package at peer {}", source.peer);
//! }
//! ```

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use uuid::Uuid;

use crate::constants::DEFAULT_MDNS_TTL_SECS;
use super::dashswarm::DASHSWARM_DEFAULT_URL;
use super::{HashAlgorithm, PackageId, PackageType, Version, VersionReq};

// ============================================================================
// Standard channel for package advertisements
// ============================================================================

/// Standard channel name for package advertisements
pub const PACKAGES_CHANNEL: &str = "_packages";

/// How often to broadcast package advertisements (seconds)
/// Uses the same interval as mDNS TTL since package discovery follows network discovery
pub const PACKAGES_BROADCAST_INTERVAL: u32 = DEFAULT_MDNS_TTL_SECS;

// ============================================================================
// Shared Package Info
// ============================================================================

/// Information about a package that can be shared with colony peers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedPackage {
    /// Package identifier
    pub id: PackageId,
    /// Package version
    pub version: Version,
    /// Package type
    pub package_type: Option<PackageType>,
    /// Size in bytes
    pub size_bytes: u64,
    /// Hash of the package content
    pub hash: Option<String>,
    /// Hash algorithm used
    pub hash_algorithm: Option<HashAlgorithm>,
    /// Can share the package data directly?
    pub shareable: bool,
    /// Can only share a reference (peer downloads from registry)?
    pub reference_only: bool,
    /// When the package was installed
    pub installed_at: DateTime<Utc>,
}

impl SharedPackage {
    /// Create a new shared package entry
    #[must_use]
    pub fn new(id: PackageId, version: Version) -> Self {
        Self {
            id,
            version,
            package_type: None,
            size_bytes: 0,
            hash: None,
            hash_algorithm: None,
            shareable: false,
            reference_only: true,
            installed_at: Utc::now(),
        }
    }

    /// Set the package type
    #[must_use]
    pub fn with_type(mut self, package_type: PackageType) -> Self {
        self.package_type = Some(package_type);
        self
    }

    /// Set the package size
    #[must_use]
    pub fn with_size(mut self, size_bytes: u64) -> Self {
        self.size_bytes = size_bytes;
        self
    }

    /// Set the package hash
    #[must_use]
    pub fn with_hash(mut self, hash: impl Into<String>, algorithm: HashAlgorithm) -> Self {
        self.hash = Some(hash.into());
        self.hash_algorithm = Some(algorithm);
        self
    }

    /// Set whether package data can be shared directly
    #[must_use]
    pub fn shareable(mut self, shareable: bool) -> Self {
        self.shareable = shareable;
        if shareable {
            self.reference_only = false;
        }
        self
    }

    /// Set reference-only mode (peer provides URL, requester downloads)
    #[must_use]
    pub fn reference_only(mut self, reference_only: bool) -> Self {
        self.reference_only = reference_only;
        self
    }
}

// ============================================================================
// Sharing Policy
// ============================================================================

/// Policy for sharing packages with other colony members
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PackageSharingPolicy {
    /// Share all packages with colony members
    ShareAll,
    /// Share only official packages (from official registry)
    ShareOfficial,
    /// Share only specific packages.
    ShareList {
        /// List of package IDs to share.
        packages: Vec<PackageId>,
    },
    /// Don't share any packages
    #[default]
    NoSharing,
}

impl PackageSharingPolicy {
    /// Check if a package should be shared under this policy
    #[must_use]
    pub fn should_share(&self, id: &PackageId, is_official: bool) -> bool {
        match self {
            Self::ShareAll => true,
            Self::ShareOfficial => is_official,
            Self::ShareList { packages } => packages.contains(id),
            Self::NoSharing => false,
        }
    }

    /// Create a policy to share specific packages
    #[must_use]
    pub fn share_list(packages: Vec<PackageId>) -> Self {
        Self::ShareList { packages }
    }
}

// ============================================================================
// Package Advertisement
// ============================================================================

/// A peer's unique identifier (matching network module)
pub type PeerId = Uuid;

/// Advertisement of packages available from a peer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageAdvertisement {
    /// The advertising peer's ID
    pub peer_id: PeerId,
    /// Peer's app name
    pub app_name: String,
    /// Available packages
    pub packages: Vec<SharedPackage>,
    /// Sharing policy
    pub policy: PackageSharingPolicy,
    /// When this advertisement was created
    pub timestamp: DateTime<Utc>,
    /// Time-to-live in seconds
    pub ttl_seconds: u32,
}

impl PackageAdvertisement {
    /// Create a new package advertisement
    #[must_use]
    pub fn new(peer_id: PeerId, app_name: impl Into<String>) -> Self {
        Self {
            peer_id,
            app_name: app_name.into(),
            packages: Vec::new(),
            policy: PackageSharingPolicy::NoSharing,
            timestamp: Utc::now(),
            ttl_seconds: PACKAGES_BROADCAST_INTERVAL * 2,
        }
    }

    /// Add a package to the advertisement
    #[must_use]
    pub fn with_package(mut self, package: SharedPackage) -> Self {
        self.packages.push(package);
        self
    }

    /// Add multiple packages
    #[must_use]
    pub fn with_packages(mut self, packages: Vec<SharedPackage>) -> Self {
        self.packages.extend(packages);
        self
    }

    /// Set the sharing policy
    #[must_use]
    pub fn with_policy(mut self, policy: PackageSharingPolicy) -> Self {
        self.policy = policy;
        self
    }

    /// Set TTL
    #[must_use]
    pub fn with_ttl(mut self, ttl_seconds: u32) -> Self {
        self.ttl_seconds = ttl_seconds;
        self
    }

    /// Check if advertisement has expired
    #[must_use]
    pub fn is_expired(&self) -> bool {
        let age = Utc::now().signed_duration_since(self.timestamp);
        age.num_seconds() > i64::from(self.ttl_seconds)
    }

    /// Get packages that match a version requirement
    #[must_use]
    pub fn matching_packages(
        &self,
        id: &PackageId,
        version_req: &VersionReq,
    ) -> Vec<&SharedPackage> {
        self.packages
            .iter()
            .filter(|p| &p.id == id && version_req.matches(&p.version))
            .collect()
    }
}

// ============================================================================
// Colony Package Source
// ============================================================================

/// How to transfer a package from a colony peer
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransferMethod {
    /// Direct transfer of package bytes
    DirectTransfer,
    /// Reference only - requester downloads from original source
    ReferenceOnly,
}

/// Source of a package found in the colony
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColonyPackageSource {
    /// The peer that has this package
    pub peer: PeerId,
    /// Peer's app name (for display)
    pub peer_name: String,
    /// Package ID
    pub package_id: PackageId,
    /// Package version available
    pub version: Version,
    /// Package size in bytes
    pub size_bytes: u64,
    /// How the package can be transferred
    pub transfer_method: TransferMethod,
    /// Estimated latency to this peer (if known)
    pub latency_ms: Option<u32>,
    /// When we last verified this source
    pub last_verified: DateTime<Utc>,
}

impl ColonyPackageSource {
    /// Create a new colony package source
    #[must_use]
    pub fn new(
        peer: PeerId,
        peer_name: impl Into<String>,
        package_id: PackageId,
        version: Version,
    ) -> Self {
        Self {
            peer,
            peer_name: peer_name.into(),
            package_id,
            version,
            size_bytes: 0,
            transfer_method: TransferMethod::ReferenceOnly,
            latency_ms: None,
            last_verified: Utc::now(),
        }
    }

    /// Set size
    #[must_use]
    pub fn with_size(mut self, size_bytes: u64) -> Self {
        self.size_bytes = size_bytes;
        self
    }

    /// Set transfer method
    #[must_use]
    pub fn with_transfer_method(mut self, method: TransferMethod) -> Self {
        self.transfer_method = method;
        self
    }

    /// Set latency
    #[must_use]
    pub fn with_latency(mut self, latency_ms: u32) -> Self {
        self.latency_ms = Some(latency_ms);
        self
    }

    /// Check if this source supports direct transfer
    #[must_use]
    pub fn supports_direct_transfer(&self) -> bool {
        self.transfer_method == TransferMethod::DirectTransfer
    }
}

// ============================================================================
// Package Request/Response
// ============================================================================

/// A request to get a package from a colony peer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageRequest {
    /// Unique request ID
    pub id: Uuid,
    /// Requesting peer
    pub requester: PeerId,
    /// Requester's app name
    pub requester_name: String,
    /// Package being requested
    pub package_id: PackageId,
    /// Version requested
    pub version: Version,
    /// Purpose of the request (for logging/approval)
    pub purpose: String,
    /// When the request was made
    pub timestamp: DateTime<Utc>,
}

impl PackageRequest {
    /// Create a new package request
    #[must_use]
    pub fn new(
        requester: PeerId,
        requester_name: impl Into<String>,
        package_id: PackageId,
        version: Version,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            requester,
            requester_name: requester_name.into(),
            package_id,
            version,
            purpose: "installation".to_string(),
            timestamp: Utc::now(),
        }
    }

    /// Set the purpose
    #[must_use]
    pub fn with_purpose(mut self, purpose: impl Into<String>) -> Self {
        self.purpose = purpose.into();
        self
    }
}

/// Response to a package request
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status")]
pub enum PackageResponse {
    /// Package data is being sent directly
    Data {
        /// Package data bytes (base64 encoded)
        data: String,
        /// Hash of the data
        hash: String,
        /// Hash algorithm
        hash_algorithm: HashAlgorithm,
    },
    /// Reference to download the package
    Reference {
        /// URL to download from
        url: String,
        /// Expected hash
        hash: Option<String>,
        /// Hash algorithm
        hash_algorithm: Option<HashAlgorithm>,
    },
    /// Request denied
    Denied {
        /// Reason for denial
        reason: String,
    },
    /// Package not found
    NotFound {
        /// Additional info
        message: String,
    },
}

impl PackageResponse {
    /// Create a data response
    #[must_use]
    pub fn data(data: String, hash: String, hash_algorithm: HashAlgorithm) -> Self {
        Self::Data {
            data,
            hash,
            hash_algorithm,
        }
    }

    /// Create a reference response
    #[must_use]
    pub fn reference(url: impl Into<String>) -> Self {
        Self::Reference {
            url: url.into(),
            hash: None,
            hash_algorithm: None,
        }
    }

    /// Create a reference response with hash
    #[must_use]
    pub fn reference_with_hash(
        url: impl Into<String>,
        hash: impl Into<String>,
        algorithm: HashAlgorithm,
    ) -> Self {
        Self::Reference {
            url: url.into(),
            hash: Some(hash.into()),
            hash_algorithm: Some(algorithm),
        }
    }

    /// Create a denied response
    #[must_use]
    pub fn denied(reason: impl Into<String>) -> Self {
        Self::Denied {
            reason: reason.into(),
        }
    }

    /// Create a not found response
    #[must_use]
    pub fn not_found(message: impl Into<String>) -> Self {
        Self::NotFound {
            message: message.into(),
        }
    }

    /// Check if response is successful (data or reference)
    #[must_use]
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Data { .. } | Self::Reference { .. })
    }
}

// ============================================================================
// Package Messages (for _packages channel)
// ============================================================================

/// Message types for the `_packages` channel
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PackageMessage {
    /// Advertising available packages
    Advertisement(PackageAdvertisement),
    /// Requesting a package
    Request(PackageRequest),
    /// Response to a request
    Response {
        /// ID of the original request
        request_id: Uuid,
        /// The response
        response: PackageResponse,
    },
}

// ============================================================================
// Colony Package Registry
// ============================================================================

/// Entry for a package discovered from colony
#[derive(Debug, Clone)]
pub struct ColonyPackageEntry {
    /// The shared package info
    pub package: SharedPackage,
    /// Peer that has this package
    pub peer: PeerId,
    /// Peer's name
    pub peer_name: String,
    /// When we last saw this advertisement
    pub last_seen: DateTime<Utc>,
    /// TTL from the advertisement
    pub ttl_seconds: u32,
}

impl ColonyPackageEntry {
    /// Check if this entry has expired
    #[must_use]
    pub fn is_expired(&self) -> bool {
        let age = Utc::now().signed_duration_since(self.last_seen);
        age.num_seconds() > i64::from(self.ttl_seconds)
    }
}

/// Registry of packages available from colony peers
///
/// This registry tracks packages advertised by other apps in the colony
/// and provides lookup methods to find packages for installation.
#[derive(Debug, Default)]
pub struct ColonyPackageRegistry {
    /// Packages from colony peers, keyed by "{peer_id}:{package_id}:{version}"
    colony_packages: Arc<RwLock<HashMap<String, ColonyPackageEntry>>>,
    /// Local packages (installed on this node), keyed by "{package_id}:{version}"
    local_packages: Arc<RwLock<HashMap<String, SharedPackage>>>,
    /// Our peer ID
    local_peer_id: Option<PeerId>,
    /// Our app name
    local_app_name: String,
    /// Our sharing policy
    sharing_policy: Arc<RwLock<PackageSharingPolicy>>,
}

impl ColonyPackageRegistry {
    /// Create a new empty registry
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a registry with local identity
    #[must_use]
    pub fn with_identity(peer_id: PeerId, app_name: impl Into<String>) -> Self {
        Self {
            local_peer_id: Some(peer_id),
            local_app_name: app_name.into(),
            ..Default::default()
        }
    }

    /// Set the local peer identity
    pub fn set_identity(&mut self, peer_id: PeerId, app_name: impl Into<String>) {
        self.local_peer_id = Some(peer_id);
        self.local_app_name = app_name.into();
    }

    /// Set the sharing policy
    pub fn set_sharing_policy(&self, policy: PackageSharingPolicy) {
        *self
            .sharing_policy
            .write()
            .unwrap_or_else(|e| e.into_inner()) = policy;
    }

    /// Get the current sharing policy
    #[must_use]
    pub fn sharing_policy(&self) -> PackageSharingPolicy {
        self.sharing_policy
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    // ========================================================================
    // Local Package Management
    // ========================================================================

    /// Add a locally installed package
    pub fn add_local_package(&self, package: SharedPackage) {
        let key = format!("{}:{}", package.id, package.version);
        self.local_packages
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(key, package);
    }

    /// Remove a local package
    pub fn remove_local_package(&self, id: &PackageId, version: &Version) {
        let key = format!("{}:{}", id, version);
        self.local_packages
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&key);
    }

    /// Get a local package
    #[must_use]
    pub fn get_local_package(&self, id: &PackageId, version: &Version) -> Option<SharedPackage> {
        let key = format!("{}:{}", id, version);
        self.local_packages
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(&key)
            .cloned()
    }

    /// List all local packages
    #[must_use]
    pub fn local_packages(&self) -> Vec<SharedPackage> {
        self.local_packages
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .values()
            .cloned()
            .collect()
    }

    /// Create an advertisement for our local packages
    #[must_use]
    pub fn create_advertisement(&self) -> Option<PackageAdvertisement> {
        let peer_id = self.local_peer_id?;
        let policy = self.sharing_policy();

        let packages: Vec<SharedPackage> = self
            .local_packages
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .values()
            .filter(|p| {
                // Only include packages that should be shared
                let is_official = p.id.namespace() == "dashflow";
                policy.should_share(&p.id, is_official) && (p.shareable || p.reference_only)
            })
            .cloned()
            .collect();

        if packages.is_empty() && matches!(policy, PackageSharingPolicy::NoSharing) {
            return None;
        }

        Some(
            PackageAdvertisement::new(peer_id, &self.local_app_name)
                .with_packages(packages)
                .with_policy(policy),
        )
    }

    // ========================================================================
    // Colony Package Management
    // ========================================================================

    /// Process a package advertisement from a peer
    pub fn process_advertisement(&self, advertisement: PackageAdvertisement) {
        if advertisement.is_expired() {
            return;
        }

        let mut colony_packages = self
            .colony_packages
            .write()
            .unwrap_or_else(|e| e.into_inner());

        // Remove old entries from this peer
        colony_packages.retain(|k, _| !k.starts_with(&format!("{}:", advertisement.peer_id)));

        // Add new entries
        for package in advertisement.packages {
            let key = format!(
                "{}:{}:{}",
                advertisement.peer_id, package.id, package.version
            );
            colony_packages.insert(
                key,
                ColonyPackageEntry {
                    package,
                    peer: advertisement.peer_id,
                    peer_name: advertisement.app_name.clone(),
                    last_seen: Utc::now(),
                    ttl_seconds: advertisement.ttl_seconds,
                },
            );
        }
    }

    /// Remove all packages from a peer
    pub fn remove_peer_packages(&self, peer_id: PeerId) {
        self.colony_packages
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .retain(|k, _| !k.starts_with(&format!("{}:", peer_id)));
    }

    /// Prune expired entries
    pub fn prune_expired(&self) {
        self.colony_packages
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .retain(|_, entry| !entry.is_expired());
    }

    // ========================================================================
    // Package Discovery
    // ========================================================================

    /// Find a package in the colony
    ///
    /// Returns the best source for a package matching the version requirement.
    /// Prefers sources with direct transfer and lower latency.
    #[must_use]
    pub fn find_in_colony(
        &self,
        id: &PackageId,
        version_req: &VersionReq,
    ) -> Option<ColonyPackageSource> {
        let colony_packages = self
            .colony_packages
            .read()
            .unwrap_or_else(|e| e.into_inner());

        // Find all matching packages
        let mut candidates: Vec<&ColonyPackageEntry> = colony_packages
            .values()
            .filter(|entry| {
                !entry.is_expired()
                    && &entry.package.id == id
                    && version_req.matches(&entry.package.version)
            })
            .collect();

        if candidates.is_empty() {
            return None;
        }

        // Sort by preference:
        // 1. Direct transfer capability
        // 2. Higher version
        // 3. More recently seen
        candidates.sort_by(|a, b| {
            // Prefer shareable (direct transfer)
            let a_direct = a.package.shareable as u8;
            let b_direct = b.package.shareable as u8;
            match b_direct.cmp(&a_direct) {
                std::cmp::Ordering::Equal => {}
                other => return other,
            }

            // Then prefer higher version
            match b.package.version.cmp(&a.package.version) {
                std::cmp::Ordering::Equal => {}
                other => return other,
            }

            // Then prefer more recently seen
            b.last_seen.cmp(&a.last_seen)
        });

        let best = candidates.first()?;

        Some(
            ColonyPackageSource::new(
                best.peer,
                &best.peer_name,
                best.package.id.clone(),
                best.package.version.clone(),
            )
            .with_size(best.package.size_bytes)
            .with_transfer_method(if best.package.shareable {
                TransferMethod::DirectTransfer
            } else {
                TransferMethod::ReferenceOnly
            }),
        )
    }

    /// Find all sources for a package in the colony
    #[must_use]
    pub fn find_all_in_colony(
        &self,
        id: &PackageId,
        version_req: &VersionReq,
    ) -> Vec<ColonyPackageSource> {
        let colony_packages = self
            .colony_packages
            .read()
            .unwrap_or_else(|e| e.into_inner());

        colony_packages
            .values()
            .filter(|entry| {
                !entry.is_expired()
                    && &entry.package.id == id
                    && version_req.matches(&entry.package.version)
            })
            .map(|entry| {
                ColonyPackageSource::new(
                    entry.peer,
                    &entry.peer_name,
                    entry.package.id.clone(),
                    entry.package.version.clone(),
                )
                .with_size(entry.package.size_bytes)
                .with_transfer_method(if entry.package.shareable {
                    TransferMethod::DirectTransfer
                } else {
                    TransferMethod::ReferenceOnly
                })
            })
            .collect()
    }

    /// Get all unique packages available in the colony
    #[must_use]
    pub fn available_packages(&self) -> Vec<(PackageId, Version)> {
        let colony_packages = self
            .colony_packages
            .read()
            .unwrap_or_else(|e| e.into_inner());

        let mut packages: Vec<(PackageId, Version)> = colony_packages
            .values()
            .filter(|entry| !entry.is_expired())
            .map(|entry| (entry.package.id.clone(), entry.package.version.clone()))
            .collect();

        packages.sort();
        packages.dedup();
        packages
    }

    /// Check if a package is available in the colony
    #[must_use]
    pub fn is_available_in_colony(&self, id: &PackageId, version_req: &VersionReq) -> bool {
        self.find_in_colony(id, version_req).is_some()
    }

    /// Get statistics about colony packages
    #[must_use]
    pub fn stats(&self) -> ColonyPackageStats {
        let colony_packages = self
            .colony_packages
            .read()
            .unwrap_or_else(|e| e.into_inner());
        let local_packages = self
            .local_packages
            .read()
            .unwrap_or_else(|e| e.into_inner());

        let active_entries: Vec<&ColonyPackageEntry> = colony_packages
            .values()
            .filter(|e| !e.is_expired())
            .collect();

        let unique_peers: std::collections::HashSet<PeerId> =
            active_entries.iter().map(|e| e.peer).collect();

        let unique_packages: std::collections::HashSet<String> = active_entries
            .iter()
            .map(|e| e.package.id.to_string())
            .collect();

        ColonyPackageStats {
            local_packages: local_packages.len(),
            colony_entries: active_entries.len(),
            unique_colony_packages: unique_packages.len(),
            peers_with_packages: unique_peers.len(),
        }
    }

    // ========================================================================
    // Request Handling
    // ========================================================================

    /// Handle a package request from another peer
    ///
    /// Returns the response to send back to the requester.
    #[must_use]
    pub fn handle_request(&self, request: &PackageRequest) -> PackageResponse {
        // Check if we have the package locally
        let local_package = self.get_local_package(&request.package_id, &request.version);

        let Some(package) = local_package else {
            return PackageResponse::not_found(format!(
                "Package {}@{} not found locally",
                request.package_id, request.version
            ));
        };

        // Check sharing policy
        let policy = self.sharing_policy();
        let is_official = package.id.namespace() == "dashflow";

        if !policy.should_share(&package.id, is_official) {
            return PackageResponse::denied("Package sharing not allowed by policy");
        }

        // Check if we can share this package
        if !package.shareable && !package.reference_only {
            return PackageResponse::denied("Package is not configured for sharing");
        }

        // For now, return reference-only responses since we don't have package data storage
        // In a full implementation, this would read the package from disk and send it
        if package.shareable {
            // In a full implementation, we'd read the package data here
            // For now, just return a reference
            PackageResponse::reference(format!(
                "{}/packages/{}/{}/download",
                DASHSWARM_DEFAULT_URL,
                package.id.namespace(),
                package.id.name()
            ))
        } else {
            PackageResponse::reference(format!(
                "{}/packages/{}/{}/download",
                DASHSWARM_DEFAULT_URL,
                package.id.namespace(),
                package.id.name()
            ))
        }
    }
}

impl Clone for ColonyPackageRegistry {
    fn clone(&self) -> Self {
        let colony_packages = self
            .colony_packages
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        let local_packages = self
            .local_packages
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        let sharing_policy = self
            .sharing_policy
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone();

        Self {
            colony_packages: Arc::new(RwLock::new(colony_packages)),
            local_packages: Arc::new(RwLock::new(local_packages)),
            local_peer_id: self.local_peer_id,
            local_app_name: self.local_app_name.clone(),
            sharing_policy: Arc::new(RwLock::new(sharing_policy)),
        }
    }
}

// ============================================================================
// Statistics
// ============================================================================

/// Statistics about the colony package registry
#[derive(Debug, Clone, Default)]
pub struct ColonyPackageStats {
    /// Number of local packages
    pub local_packages: usize,
    /// Number of entries from colony peers
    pub colony_entries: usize,
    /// Number of unique packages in colony
    pub unique_colony_packages: usize,
    /// Number of peers sharing packages
    pub peers_with_packages: usize,
}

// ============================================================================
// Error Types
// ============================================================================

/// Errors from package sharing operations
#[derive(Debug, Clone, thiserror::Error)]
#[non_exhaustive]
pub enum SharingError {
    /// Package not found
    #[error("Package not found: {0}")]
    PackageNotFound(String),

    /// Request denied
    #[error("Request denied: {0}")]
    RequestDenied(String),

    /// Network error
    #[error("Network error: {0}")]
    NetworkError(String),

    /// Transfer failed
    #[error("Transfer failed: {0}")]
    TransferFailed(String),

    /// Verification failed
    #[error("Verification failed: {0}")]
    VerificationFailed(String),

    /// Timeout
    #[error("Timeout waiting for response")]
    Timeout,
}

/// Result type for sharing operations
pub type SharingResult<T> = Result<T, SharingError>;

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shared_package() {
        let pkg = SharedPackage::new(
            PackageId::new("dashflow", "sentiment-analysis"),
            Version::new(1, 2, 0),
        )
        .with_type(PackageType::NodeLibrary)
        .with_size(1024 * 1024)
        .shareable(true);

        assert_eq!(pkg.id.namespace(), "dashflow");
        assert_eq!(pkg.id.name(), "sentiment-analysis");
        assert!(pkg.shareable);
        assert!(!pkg.reference_only);
    }

    #[test]
    fn test_sharing_policy() {
        let pkg_id = PackageId::new("dashflow", "test");
        let community_id = PackageId::new("community", "test");

        // ShareAll
        assert!(PackageSharingPolicy::ShareAll.should_share(&pkg_id, true));
        assert!(PackageSharingPolicy::ShareAll.should_share(&community_id, false));

        // ShareOfficial
        assert!(PackageSharingPolicy::ShareOfficial.should_share(&pkg_id, true));
        assert!(!PackageSharingPolicy::ShareOfficial.should_share(&community_id, false));

        // ShareList
        let list = PackageSharingPolicy::share_list(vec![pkg_id.clone()]);
        assert!(list.should_share(&pkg_id, true));
        assert!(!list.should_share(&community_id, false));

        // NoSharing
        assert!(!PackageSharingPolicy::NoSharing.should_share(&pkg_id, true));
    }

    #[test]
    fn test_package_advertisement() {
        let peer_id = Uuid::new_v4();
        let pkg = SharedPackage::new(PackageId::new("dashflow", "test"), Version::new(1, 0, 0))
            .shareable(true);

        let advert = PackageAdvertisement::new(peer_id, "TestApp")
            .with_package(pkg)
            .with_policy(PackageSharingPolicy::ShareAll);

        assert_eq!(advert.peer_id, peer_id);
        assert_eq!(advert.packages.len(), 1);
        assert!(!advert.is_expired());
    }

    #[test]
    fn test_advertisement_matching() {
        let peer_id = Uuid::new_v4();
        let pkg1 = SharedPackage::new(PackageId::new("dashflow", "test"), Version::new(1, 0, 0));
        let pkg2 = SharedPackage::new(PackageId::new("dashflow", "test"), Version::new(2, 0, 0));
        let pkg3 = SharedPackage::new(PackageId::new("dashflow", "other"), Version::new(1, 0, 0));

        let advert =
            PackageAdvertisement::new(peer_id, "TestApp").with_packages(vec![pkg1, pkg2, pkg3]);

        let test_id = PackageId::new("dashflow", "test");
        let matches = advert.matching_packages(&test_id, &VersionReq::any());
        assert_eq!(matches.len(), 2);

        let v1_matches =
            advert.matching_packages(&test_id, &VersionReq::exact(Version::new(1, 0, 0)));
        assert_eq!(v1_matches.len(), 1);
    }

    #[test]
    fn test_colony_package_source() {
        let peer_id = Uuid::new_v4();
        let source = ColonyPackageSource::new(
            peer_id,
            "TestApp",
            PackageId::new("dashflow", "test"),
            Version::new(1, 0, 0),
        )
        .with_size(1024)
        .with_transfer_method(TransferMethod::DirectTransfer)
        .with_latency(50);

        assert_eq!(source.peer, peer_id);
        assert!(source.supports_direct_transfer());
        assert_eq!(source.latency_ms, Some(50));
    }

    #[test]
    fn test_package_request_response() {
        let requester = Uuid::new_v4();
        let request = PackageRequest::new(
            requester,
            "RequesterApp",
            PackageId::new("dashflow", "test"),
            Version::new(1, 0, 0),
        )
        .with_purpose("Installing for testing");

        assert_eq!(request.requester, requester);
        assert_eq!(request.purpose, "Installing for testing");

        let response = PackageResponse::reference("https://example.com/package.tar.gz");
        assert!(response.is_success());

        let denied = PackageResponse::denied("Policy violation");
        assert!(!denied.is_success());
    }

    #[test]
    fn test_colony_package_registry_local() {
        let registry = ColonyPackageRegistry::new();

        let pkg = SharedPackage::new(PackageId::new("dashflow", "test"), Version::new(1, 0, 0))
            .shareable(true);

        registry.add_local_package(pkg.clone());

        let retrieved =
            registry.get_local_package(&PackageId::new("dashflow", "test"), &Version::new(1, 0, 0));
        assert!(retrieved.is_some());

        let packages = registry.local_packages();
        assert_eq!(packages.len(), 1);
    }

    #[test]
    fn test_colony_package_registry_colony() {
        let registry = ColonyPackageRegistry::new();
        let peer_id = Uuid::new_v4();

        let pkg = SharedPackage::new(PackageId::new("dashflow", "test"), Version::new(1, 0, 0))
            .shareable(true);

        let advert = PackageAdvertisement::new(peer_id, "RemoteApp")
            .with_package(pkg)
            .with_policy(PackageSharingPolicy::ShareAll);

        registry.process_advertisement(advert);

        // Find in colony
        let source =
            registry.find_in_colony(&PackageId::new("dashflow", "test"), &VersionReq::any());
        assert!(source.is_some());

        let source = source.unwrap();
        assert_eq!(source.peer, peer_id);
        assert_eq!(source.peer_name, "RemoteApp");
    }

    #[test]
    fn test_colony_package_registry_find_best() {
        let registry = ColonyPackageRegistry::new();

        // Add packages from two peers
        let peer1 = Uuid::new_v4();
        let pkg1 = SharedPackage::new(PackageId::new("dashflow", "test"), Version::new(1, 0, 0))
            .reference_only(true);

        let peer2 = Uuid::new_v4();
        let pkg2 = SharedPackage::new(PackageId::new("dashflow", "test"), Version::new(1, 0, 0))
            .shareable(true);

        registry.process_advertisement(
            PackageAdvertisement::new(peer1, "App1")
                .with_package(pkg1)
                .with_policy(PackageSharingPolicy::ShareAll),
        );

        registry.process_advertisement(
            PackageAdvertisement::new(peer2, "App2")
                .with_package(pkg2)
                .with_policy(PackageSharingPolicy::ShareAll),
        );

        // Should prefer peer2 with direct transfer
        let source =
            registry.find_in_colony(&PackageId::new("dashflow", "test"), &VersionReq::any());
        assert!(source.is_some());
        let source = source.unwrap();
        assert_eq!(source.peer, peer2);
        assert!(source.supports_direct_transfer());
    }

    #[test]
    fn test_create_advertisement() {
        let peer_id = Uuid::new_v4();
        let mut registry = ColonyPackageRegistry::new();
        registry.set_identity(peer_id, "MyApp");
        registry.set_sharing_policy(PackageSharingPolicy::ShareAll);

        let pkg = SharedPackage::new(PackageId::new("dashflow", "test"), Version::new(1, 0, 0))
            .shareable(true);

        registry.add_local_package(pkg);

        let advert = registry.create_advertisement();
        assert!(advert.is_some());

        let advert = advert.unwrap();
        assert_eq!(advert.peer_id, peer_id);
        assert_eq!(advert.packages.len(), 1);
    }

    #[test]
    fn test_handle_request() {
        let peer_id = Uuid::new_v4();
        let mut registry = ColonyPackageRegistry::new();
        registry.set_identity(peer_id, "MyApp");
        registry.set_sharing_policy(PackageSharingPolicy::ShareAll);

        let pkg = SharedPackage::new(PackageId::new("dashflow", "test"), Version::new(1, 0, 0))
            .shareable(true);

        registry.add_local_package(pkg);

        // Request for existing package
        let request = PackageRequest::new(
            Uuid::new_v4(),
            "OtherApp",
            PackageId::new("dashflow", "test"),
            Version::new(1, 0, 0),
        );

        let response = registry.handle_request(&request);
        assert!(response.is_success());

        // Request for non-existent package
        let request2 = PackageRequest::new(
            Uuid::new_v4(),
            "OtherApp",
            PackageId::new("dashflow", "nonexistent"),
            Version::new(1, 0, 0),
        );

        let response2 = registry.handle_request(&request2);
        assert!(!response2.is_success());
        assert!(matches!(response2, PackageResponse::NotFound { .. }));
    }

    #[test]
    fn test_stats() {
        let registry = ColonyPackageRegistry::new();
        let peer_id = Uuid::new_v4();

        // Add local package
        registry.add_local_package(SharedPackage::new(
            PackageId::new("dashflow", "local"),
            Version::new(1, 0, 0),
        ));

        // Add colony packages
        registry.process_advertisement(
            PackageAdvertisement::new(peer_id, "RemoteApp")
                .with_package(SharedPackage::new(
                    PackageId::new("dashflow", "remote"),
                    Version::new(1, 0, 0),
                ))
                .with_policy(PackageSharingPolicy::ShareAll),
        );

        let stats = registry.stats();
        assert_eq!(stats.local_packages, 1);
        assert_eq!(stats.colony_entries, 1);
        assert_eq!(stats.peers_with_packages, 1);
    }

    #[test]
    fn test_package_message_serialization() {
        let peer_id = Uuid::new_v4();
        let advert = PackageAdvertisement::new(peer_id, "TestApp")
            .with_policy(PackageSharingPolicy::ShareAll);

        let msg = PackageMessage::Advertisement(advert);
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"Advertisement\""));

        let parsed: PackageMessage = serde_json::from_str(&json).unwrap();
        if let PackageMessage::Advertisement(a) = parsed {
            assert_eq!(a.peer_id, peer_id);
        } else {
            panic!("Wrong message type");
        }
    }

    #[test]
    fn test_remove_peer_packages() {
        let registry = ColonyPackageRegistry::new();
        let peer_id = Uuid::new_v4();

        registry.process_advertisement(
            PackageAdvertisement::new(peer_id, "RemoteApp")
                .with_package(SharedPackage::new(
                    PackageId::new("dashflow", "test"),
                    Version::new(1, 0, 0),
                ))
                .with_policy(PackageSharingPolicy::ShareAll),
        );

        assert!(registry
            .is_available_in_colony(&PackageId::new("dashflow", "test"), &VersionReq::any()));

        registry.remove_peer_packages(peer_id);

        assert!(!registry
            .is_available_in_colony(&PackageId::new("dashflow", "test"), &VersionReq::any()));
    }
}
