// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! P2P Resource Discovery and Sharing
//!
//! Apps can advertise resources they have available (LLM endpoints, GPUs, storage, etc.)
//! and other apps in the colony can discover and request access to these resources.
//!
//! ## Resource Types
//!
//! - **LLM**: Language model endpoints with rate limits and latency info
//! - **GPU**: CUDA/ROCm compute resources
//! - **Storage**: Shared storage backends (S3, GCS, local)
//! - **VectorDb**: Vector database instances (Pinecone, Qdrant, etc.)
//! - **Service**: Generic service endpoints
//! - **Custom**: Extensible for new resource types
//!
//! ## Sharing Model
//!
//! Resources are shared through proxies - no credentials are exposed. The owning app
//! proxies requests and enforces rate limits, usage tracking, and access policies.
//!
//! ## Example
//!
//! ```rust,ignore
//! use dashflow::network::resources::*;
//!
//! // Advertise an LLM endpoint
//! let resource = AdvertisedResource::llm(LlmResourceInfo {
//!     provider: "aws_bedrock".to_string(),
//!     account: "prod-1".to_string(),
//!     region: "us-east-1".to_string(),
//!     model: "claude-3-5-sonnet".to_string(),
//!     rate_limit_remaining: 850,
//!     ..Default::default()
//! })
//! .with_policy(SharingPolicy::ColonyOpen);
//!
//! network.advertise_resource(resource).await?;
//!
//! // Discover resources from the colony
//! let llms = network.resources().available_llm_models().await;
//! ```

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use uuid::Uuid;

use crate::constants::DEFAULT_RESOURCE_BROADCAST_INTERVAL_SECS;
use super::PeerId;

// ============================================================================
// Resource Types
// ============================================================================

/// Type of resource being advertised
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "info")]
pub enum ResourceType {
    /// Language model endpoint
    Llm(LlmResourceInfo),
    /// GPU compute resource
    Gpu(GpuResourceInfo),
    /// Shared storage backend
    Storage(StorageResourceInfo),
    /// Vector database instance
    VectorDb(VectorDbResourceInfo),
    /// Generic service endpoint
    Service(ServiceResourceInfo),
    /// Custom resource type (extensible)
    Custom {
        /// Name of the custom resource type
        type_name: String,
        /// Resource-specific information as JSON
        info: serde_json::Value,
    },
}

impl ResourceType {
    /// Get the type name as a string
    #[must_use]
    pub fn type_name(&self) -> &str {
        match self {
            Self::Llm(_) => "llm",
            Self::Gpu(_) => "gpu",
            Self::Storage(_) => "storage",
            Self::VectorDb(_) => "vectordb",
            Self::Service(_) => "service",
            Self::Custom { type_name, .. } => type_name,
        }
    }
}

/// LLM endpoint resource information
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LlmResourceInfo {
    /// Provider name (e.g., "aws_bedrock", "openai", "anthropic")
    pub provider: String,
    /// Account identifier
    pub account: String,
    /// Region (for cloud providers)
    pub region: String,
    /// Model name/ID
    pub model: String,
    /// Remaining rate limit (requests/min)
    pub rate_limit_remaining: u32,
    /// Remaining tokens (if applicable)
    pub tokens_remaining: Option<u64>,
    /// Whether this endpoint is saturated
    pub saturated: bool,
    /// Recent latency in milliseconds
    pub latency_ms: u32,
    /// Supported capabilities (e.g., "vision", "tools", "streaming")
    pub capabilities: Vec<String>,
    /// Maximum context length
    pub max_context_length: Option<u32>,
}

impl LlmResourceInfo {
    /// Create new LLM resource info
    #[must_use]
    pub fn new(provider: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            provider: provider.into(),
            model: model.into(),
            rate_limit_remaining: u32::MAX,
            ..Default::default()
        }
    }

    /// Check if endpoint is available (not saturated, has capacity)
    #[must_use]
    pub fn is_available(&self) -> bool {
        !self.saturated && self.rate_limit_remaining > 10
    }

    /// Set account
    #[must_use]
    pub fn with_account(mut self, account: impl Into<String>) -> Self {
        self.account = account.into();
        self
    }

    /// Set region
    #[must_use]
    pub fn with_region(mut self, region: impl Into<String>) -> Self {
        self.region = region.into();
        self
    }

    /// Set rate limit remaining
    #[must_use]
    pub fn with_rate_limit(mut self, remaining: u32) -> Self {
        self.rate_limit_remaining = remaining;
        self
    }

    /// Set latency
    #[must_use]
    pub fn with_latency(mut self, latency_ms: u32) -> Self {
        self.latency_ms = latency_ms;
        self
    }

    /// Add capability
    #[must_use]
    pub fn with_capability(mut self, cap: impl Into<String>) -> Self {
        self.capabilities.push(cap.into());
        self
    }
}

/// GPU compute resource information
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GpuResourceInfo {
    /// GPU model (e.g., "A100", "H100", "RTX 4090")
    pub model: String,
    /// VRAM in GB
    pub vram_gb: u32,
    /// CUDA version (if NVIDIA)
    pub cuda_version: Option<String>,
    /// ROCm version (if AMD)
    pub rocm_version: Option<String>,
    /// Available VRAM in GB
    pub available_vram_gb: u32,
    /// Current utilization percentage
    pub utilization_percent: u8,
    /// Compute capability (e.g., "8.0" for A100)
    pub compute_capability: Option<String>,
}

impl GpuResourceInfo {
    /// Create new GPU resource info
    #[must_use]
    pub fn new(model: impl Into<String>, vram_gb: u32) -> Self {
        Self {
            model: model.into(),
            vram_gb,
            available_vram_gb: vram_gb,
            ..Default::default()
        }
    }

    /// Check if GPU has available capacity
    #[must_use]
    pub fn is_available(&self) -> bool {
        self.utilization_percent < 90 && self.available_vram_gb > 0
    }
}

/// Shared storage resource information
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorageResourceInfo {
    /// Storage type (e.g., "s3", "gcs", "nfs", "local")
    pub storage_type: String,
    /// Bucket or mount point name
    pub name: String,
    /// Region (for cloud storage)
    pub region: Option<String>,
    /// Total capacity in bytes (if known)
    pub total_bytes: Option<u64>,
    /// Available capacity in bytes (if known)
    pub available_bytes: Option<u64>,
    /// Read throughput in MB/s
    pub read_throughput_mbps: Option<u32>,
    /// Write throughput in MB/s
    pub write_throughput_mbps: Option<u32>,
}

impl StorageResourceInfo {
    /// Create new storage resource info
    #[must_use]
    pub fn new(storage_type: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            storage_type: storage_type.into(),
            name: name.into(),
            ..Default::default()
        }
    }
}

/// Vector database resource information
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct VectorDbResourceInfo {
    /// Database type (e.g., "pinecone", "qdrant", "weaviate", "milvus")
    pub db_type: String,
    /// Index/collection name
    pub index_name: String,
    /// Vector dimensions
    pub dimensions: u32,
    /// Distance metric (e.g., "cosine", "euclidean", "dot")
    pub metric: String,
    /// Total vectors stored
    pub total_vectors: Option<u64>,
    /// Max queries per second
    pub max_qps: Option<u32>,
    /// Current QPS utilization
    pub current_qps: Option<u32>,
}

impl VectorDbResourceInfo {
    /// Create new vector DB resource info
    #[must_use]
    pub fn new(db_type: impl Into<String>, index_name: impl Into<String>, dimensions: u32) -> Self {
        Self {
            db_type: db_type.into(),
            index_name: index_name.into(),
            dimensions,
            metric: "cosine".to_string(),
            ..Default::default()
        }
    }

    /// Check if vector DB has capacity
    #[must_use]
    pub fn is_available(&self) -> bool {
        match (self.max_qps, self.current_qps) {
            (Some(max), Some(current)) => current < max * 90 / 100,
            _ => true, // Assume available if no QPS info
        }
    }
}

/// Generic service resource information
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceResourceInfo {
    /// Service type (e.g., "api", "database", "cache")
    pub service_type: String,
    /// Service name
    pub name: String,
    /// Service version
    pub version: Option<String>,
    /// Base URL
    pub base_url: Option<String>,
    /// Supported capabilities
    pub capabilities: Vec<String>,
    /// Current health status
    pub healthy: bool,
    /// Current latency in ms
    pub latency_ms: Option<u32>,
}

impl ServiceResourceInfo {
    /// Create new service resource info
    #[must_use]
    pub fn new(service_type: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            service_type: service_type.into(),
            name: name.into(),
            healthy: true,
            ..Default::default()
        }
    }
}

// ============================================================================
// Resource Advertisement
// ============================================================================

/// An advertised resource available for sharing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvertisedResource {
    /// Unique resource ID
    pub id: String,
    /// The resource being advertised
    pub resource_type: ResourceType,
    /// Additional capabilities this resource provides
    pub capabilities: Vec<String>,
    /// Whether this resource can be shared with other peers
    pub shareable: bool,
    /// Current health status
    pub healthy: bool,
    /// Type-specific metrics (JSON for flexibility)
    pub metrics: serde_json::Value,
    /// Sharing policy for this resource
    pub policy: SharingPolicy,
    /// When this resource was last updated
    pub updated_at: DateTime<Utc>,
    /// TTL for this advertisement in seconds
    pub ttl_seconds: u32,
}

impl AdvertisedResource {
    /// Create a new LLM resource advertisement
    #[must_use]
    pub fn llm(info: LlmResourceInfo) -> Self {
        let id = format!("llm-{}-{}-{}", info.provider, info.region, info.model);
        Self {
            id,
            resource_type: ResourceType::Llm(info),
            capabilities: Vec::new(),
            shareable: false,
            healthy: true,
            metrics: serde_json::Value::Null,
            policy: SharingPolicy::Private,
            updated_at: Utc::now(),
            ttl_seconds: 60,
        }
    }

    /// Create a new GPU resource advertisement
    #[must_use]
    pub fn gpu(info: GpuResourceInfo) -> Self {
        let id = format!("gpu-{}", &Uuid::new_v4().to_string()[..8]);
        Self {
            id,
            resource_type: ResourceType::Gpu(info),
            capabilities: Vec::new(),
            shareable: false,
            healthy: true,
            metrics: serde_json::Value::Null,
            policy: SharingPolicy::Private,
            updated_at: Utc::now(),
            ttl_seconds: 60,
        }
    }

    /// Create a new storage resource advertisement
    #[must_use]
    pub fn storage(info: StorageResourceInfo) -> Self {
        let id = format!("storage-{}-{}", info.storage_type, info.name);
        Self {
            id,
            resource_type: ResourceType::Storage(info),
            capabilities: Vec::new(),
            shareable: false,
            healthy: true,
            metrics: serde_json::Value::Null,
            policy: SharingPolicy::Private,
            updated_at: Utc::now(),
            ttl_seconds: 60,
        }
    }

    /// Create a new vector DB resource advertisement
    #[must_use]
    pub fn vector_db(info: VectorDbResourceInfo) -> Self {
        let id = format!("vectordb-{}-{}", info.db_type, info.index_name);
        Self {
            id,
            resource_type: ResourceType::VectorDb(info),
            capabilities: Vec::new(),
            shareable: false,
            healthy: true,
            metrics: serde_json::Value::Null,
            policy: SharingPolicy::Private,
            updated_at: Utc::now(),
            ttl_seconds: 60,
        }
    }

    /// Create a new service resource advertisement
    #[must_use]
    pub fn service(info: ServiceResourceInfo) -> Self {
        let id = format!("service-{}-{}", info.service_type, info.name);
        Self {
            id,
            resource_type: ResourceType::Service(info),
            capabilities: Vec::new(),
            shareable: false,
            healthy: true,
            metrics: serde_json::Value::Null,
            policy: SharingPolicy::Private,
            updated_at: Utc::now(),
            ttl_seconds: 60,
        }
    }

    /// Create a custom resource advertisement
    #[must_use]
    pub fn custom(type_name: impl Into<String>, info: serde_json::Value) -> Self {
        let type_name = type_name.into();
        let id = format!("custom-{}-{}", type_name, &Uuid::new_v4().to_string()[..8]);
        Self {
            id,
            resource_type: ResourceType::Custom { type_name, info },
            capabilities: Vec::new(),
            shareable: false,
            healthy: true,
            metrics: serde_json::Value::Null,
            policy: SharingPolicy::Private,
            updated_at: Utc::now(),
            ttl_seconds: 60,
        }
    }

    /// Mark as shareable
    #[must_use]
    pub fn shareable(mut self) -> Self {
        self.shareable = true;
        self
    }

    /// Set sharing policy
    #[must_use]
    pub fn with_policy(mut self, policy: SharingPolicy) -> Self {
        if !matches!(policy, SharingPolicy::Private) {
            self.shareable = true;
        }
        self.policy = policy;
        self
    }

    /// Add capability
    #[must_use]
    pub fn with_capability(mut self, cap: impl Into<String>) -> Self {
        self.capabilities.push(cap.into());
        self
    }

    /// Set metrics
    #[must_use]
    pub fn with_metrics(mut self, metrics: serde_json::Value) -> Self {
        self.metrics = metrics;
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
        let age = Utc::now().signed_duration_since(self.updated_at);
        age.num_seconds() > i64::from(self.ttl_seconds)
    }

    /// Refresh the advertisement timestamp
    pub fn refresh(&mut self) {
        self.updated_at = Utc::now();
    }
}

// ============================================================================
// Sharing Policy
// ============================================================================

/// Policy for sharing a resource with other peers
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SharingPolicy {
    /// No sharing - resource is private
    #[default]
    Private,
    /// Share with any colony member automatically
    ColonyOpen,
    /// Share only with specific peers
    AllowList {
        /// List of peer IDs allowed to access this resource
        allowed_peers: Vec<PeerId>,
    },
    /// Require manual approval for each request
    RequestApproval,
    /// Metered sharing - charge for usage (future)
    Metered {
        /// Cost per request (in arbitrary units)
        cost_per_request: u32,
    },
}

impl SharingPolicy {
    /// Check if a peer is allowed to access under this policy
    #[must_use]
    pub fn allows(&self, peer: PeerId) -> AllowResult {
        match self {
            Self::Private => AllowResult::Denied,
            Self::ColonyOpen => AllowResult::Allowed,
            Self::AllowList { allowed_peers } => {
                if allowed_peers.contains(&peer) {
                    AllowResult::Allowed
                } else {
                    AllowResult::Denied
                }
            }
            Self::RequestApproval => AllowResult::PendingApproval,
            Self::Metered { .. } => AllowResult::Allowed, // Access allowed, will be charged
        }
    }
}

/// Result of checking access policy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllowResult {
    /// Access is allowed
    Allowed,
    /// Access is denied
    Denied,
    /// Access requires manual approval
    PendingApproval,
}

// ============================================================================
// Resource Request/Response
// ============================================================================

/// A request to access another peer's resource
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceRequest {
    /// Unique request ID
    pub id: Uuid,
    /// Requesting peer
    pub from: PeerId,
    /// Resource being requested
    pub resource_id: String,
    /// Requested duration in seconds
    pub duration_seconds: u32,
    /// Purpose of the request
    pub purpose: Option<String>,
    /// When the request was made
    pub timestamp: DateTime<Utc>,
}

impl ResourceRequest {
    /// Create a new resource request
    #[must_use]
    pub fn new(from: PeerId, resource_id: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            from,
            resource_id: resource_id.into(),
            duration_seconds: 3600, // Default 1 hour
            purpose: None,
            timestamp: Utc::now(),
        }
    }

    /// Set requested duration
    #[must_use]
    pub fn with_duration(mut self, seconds: u32) -> Self {
        self.duration_seconds = seconds;
        self
    }

    /// Set purpose
    #[must_use]
    pub fn with_purpose(mut self, purpose: impl Into<String>) -> Self {
        self.purpose = Some(purpose.into());
        self
    }
}

/// Response to a resource request
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status")]
pub enum ResourceResponse {
    /// Access granted
    Granted {
        /// The grant details
        grant: ResourceGrant,
    },
    /// Access denied
    Denied {
        /// Reason for denial
        reason: String,
    },
    /// Request is pending approval
    PendingApproval {
        /// Estimated wait time in seconds
        estimated_wait_seconds: Option<u32>,
    },
    /// Resource is unavailable
    Unavailable {
        /// Reason for unavailability
        reason: String,
    },
}

/// A grant to access a resource
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceGrant {
    /// Unique grant ID
    pub id: Uuid,
    /// Resource being granted
    pub resource_id: String,
    /// Peer receiving the grant
    pub grantee: PeerId,
    /// Peer providing the resource
    pub grantor: PeerId,
    /// Access token for the proxy
    pub access_token: String,
    /// Proxy URL to use for access
    pub proxy_url: String,
    /// When the grant was issued
    pub issued_at: DateTime<Utc>,
    /// When the grant expires
    pub expires_at: DateTime<Utc>,
    /// Usage limits
    pub limits: Option<ResourceLimits>,
}

impl ResourceGrant {
    /// Create a new resource grant
    #[must_use]
    pub fn new(
        resource_id: impl Into<String>,
        grantee: PeerId,
        grantor: PeerId,
        proxy_url: impl Into<String>,
        duration: Duration,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            resource_id: resource_id.into(),
            grantee,
            grantor,
            access_token: Uuid::new_v4().to_string(),
            proxy_url: proxy_url.into(),
            issued_at: now,
            expires_at: now
                + chrono::Duration::from_std(duration).unwrap_or(chrono::Duration::hours(1)),
            limits: None,
        }
    }

    /// Check if grant is expired
    #[must_use]
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }

    /// Set limits
    #[must_use]
    pub fn with_limits(mut self, limits: ResourceLimits) -> Self {
        self.limits = Some(limits);
        self
    }
}

/// Usage limits for a resource grant
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    /// Maximum requests per minute
    pub max_requests_per_minute: Option<u32>,
    /// Maximum total requests
    pub max_total_requests: Option<u32>,
    /// Maximum tokens (for LLM)
    pub max_tokens: Option<u64>,
    /// Maximum bytes (for storage)
    pub max_bytes: Option<u64>,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_requests_per_minute: Some(60),
            max_total_requests: None,
            max_tokens: None,
            max_bytes: None,
        }
    }
}

// ============================================================================
// Colony Resource Registry
// ============================================================================

/// Registry of all resources discovered in the colony
#[derive(Debug, Default)]
pub struct ColonyResourceRegistry {
    /// Resources indexed by resource ID
    resources: Arc<RwLock<HashMap<String, ColonyResource>>>,
}

/// A resource discovered from the colony
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColonyResource {
    /// The advertised resource
    pub resource: AdvertisedResource,
    /// Peer that owns this resource
    pub owner: PeerId,
    /// When we last received an update
    pub last_seen: DateTime<Utc>,
}

impl ColonyResourceRegistry {
    /// Create a new empty registry
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add or update a resource
    pub fn upsert(&self, owner: PeerId, resource: AdvertisedResource) {
        let mut resources = self
            .resources
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let key = format!("{}:{}", owner, resource.id);
        resources.insert(
            key,
            ColonyResource {
                resource,
                owner,
                last_seen: Utc::now(),
            },
        );
    }

    /// Remove a resource
    pub fn remove(&self, owner: PeerId, resource_id: &str) {
        let mut resources = self
            .resources
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let key = format!("{}:{}", owner, resource_id);
        resources.remove(&key);
    }

    /// Get all resources of a specific type
    #[must_use]
    pub fn resources_of_type(&self, type_name: &str) -> Vec<ColonyResource> {
        self.resources
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .values()
            .filter(|r| {
                r.resource.resource_type.type_name() == type_name && !r.resource.is_expired()
            })
            .cloned()
            .collect()
    }

    /// Get all LLM resources
    #[must_use]
    pub fn llm_resources(&self) -> Vec<ColonyResource> {
        self.resources_of_type("llm")
    }

    /// Get all GPU resources
    #[must_use]
    pub fn gpu_resources(&self) -> Vec<ColonyResource> {
        self.resources_of_type("gpu")
    }

    /// Get all storage resources
    #[must_use]
    pub fn storage_resources(&self) -> Vec<ColonyResource> {
        self.resources_of_type("storage")
    }

    /// Get all vector DB resources
    #[must_use]
    pub fn vectordb_resources(&self) -> Vec<ColonyResource> {
        self.resources_of_type("vectordb")
    }

    /// Get available LLM models across the colony
    #[must_use]
    pub fn available_llm_models(&self) -> Vec<String> {
        let mut models: Vec<String> = self
            .llm_resources()
            .iter()
            .filter_map(|r| {
                if let ResourceType::Llm(info) = &r.resource.resource_type {
                    if info.is_available() && r.resource.shareable {
                        Some(info.model.clone())
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();
        models.sort();
        models.dedup();
        models
    }

    /// Find the healthiest endpoint for a model
    #[must_use]
    pub fn healthiest_llm_endpoint(&self, model: &str) -> Option<ColonyResource> {
        self.llm_resources()
            .into_iter()
            .filter(|r| {
                if let ResourceType::Llm(info) = &r.resource.resource_type {
                    info.model == model && info.is_available() && r.resource.shareable
                } else {
                    false
                }
            })
            .min_by(|a, b| {
                // Prefer: lowest latency, highest rate limit remaining
                let a_info = match &a.resource.resource_type {
                    ResourceType::Llm(info) => info,
                    _ => return std::cmp::Ordering::Equal,
                };
                let b_info = match &b.resource.resource_type {
                    ResourceType::Llm(info) => info,
                    _ => return std::cmp::Ordering::Equal,
                };
                a_info.latency_ms.cmp(&b_info.latency_ms)
            })
    }

    /// Get all shareable resources
    #[must_use]
    pub fn shareable_resources(&self) -> Vec<ColonyResource> {
        self.resources
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .values()
            .filter(|r| r.resource.shareable && !r.resource.is_expired())
            .cloned()
            .collect()
    }

    /// Prune expired resources
    pub fn prune_expired(&self) {
        let mut resources = self
            .resources
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        resources.retain(|_, r| !r.resource.is_expired());
    }

    /// Get total resource count
    #[must_use]
    pub fn total_count(&self) -> usize {
        self.resources
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .len()
    }
}

impl Clone for ColonyResourceRegistry {
    fn clone(&self) -> Self {
        let resources = self
            .resources
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        Self {
            resources: Arc::new(RwLock::new(resources)),
        }
    }
}

// ============================================================================
// Resource Channel
// ============================================================================

/// Standard channel for resource advertisements
pub const RESOURCES_CHANNEL: &str = "_resources";

/// Broadcast interval for resource advertisements (seconds)
/// Now uses centralized constant from `crate::constants::DEFAULT_RESOURCE_BROADCAST_INTERVAL_SECS`
pub const RESOURCES_BROADCAST_INTERVAL: u32 = DEFAULT_RESOURCE_BROADCAST_INTERVAL_SECS;

/// Resource message types for the _resources channel
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ResourceMessage {
    /// Advertising available resources
    Advertisement {
        /// List of resources being advertised
        resources: Vec<AdvertisedResource>,
    },
    /// Requesting access to a resource
    Request(ResourceRequest),
    /// Response to a request
    Response {
        /// ID of the request being responded to
        request_id: Uuid,
        /// The response (granted, denied, etc.)
        response: ResourceResponse,
    },
    /// Revoking a grant
    RevokeGrant {
        /// ID of the grant being revoked
        grant_id: Uuid,
    },
    /// Resource no longer available
    Withdrawn {
        /// ID of the resource being withdrawn
        resource_id: String,
    },
}

// ============================================================================
// Failover Strategy
// ============================================================================

/// Strategy for selecting resources when multiple are available
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum FailoverStrategy {
    /// Use highest priority (lowest latency, most capacity)
    #[default]
    Priority,
    /// Round-robin across available resources
    RoundRobin,
    /// Least-loaded resource
    LeastLoaded,
    /// Optimize for cost (for metered resources)
    CostOptimized,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_llm_resource_info() {
        let info = LlmResourceInfo::new("aws_bedrock", "claude-3-5-sonnet")
            .with_account("prod-1")
            .with_region("us-east-1")
            .with_rate_limit(850)
            .with_latency(250)
            .with_capability("vision");

        assert_eq!(info.provider, "aws_bedrock");
        assert_eq!(info.model, "claude-3-5-sonnet");
        assert_eq!(info.account, "prod-1");
        assert_eq!(info.region, "us-east-1");
        assert!(info.is_available());
        assert_eq!(info.capabilities, vec!["vision"]);
    }

    #[test]
    fn test_llm_resource_availability() {
        let mut info = LlmResourceInfo::new("openai", "gpt-4");
        info.rate_limit_remaining = 100;
        assert!(info.is_available());

        info.saturated = true;
        assert!(!info.is_available());

        info.saturated = false;
        info.rate_limit_remaining = 5;
        assert!(!info.is_available());
    }

    #[test]
    fn test_gpu_resource_info() {
        let info = GpuResourceInfo::new("A100", 80);
        assert_eq!(info.model, "A100");
        assert_eq!(info.vram_gb, 80);
        assert!(info.is_available());
    }

    #[test]
    fn test_advertised_resource() {
        let llm_info = LlmResourceInfo::new("aws_bedrock", "claude-3-5-sonnet");
        let resource = AdvertisedResource::llm(llm_info)
            .shareable()
            .with_policy(SharingPolicy::ColonyOpen)
            .with_ttl(120);

        assert!(resource.shareable);
        assert_eq!(resource.ttl_seconds, 120);
        assert!(!resource.is_expired());
    }

    #[test]
    fn test_sharing_policy() {
        let peer = PeerId::new();

        assert_eq!(SharingPolicy::Private.allows(peer), AllowResult::Denied);
        assert_eq!(SharingPolicy::ColonyOpen.allows(peer), AllowResult::Allowed);

        let allow_list = SharingPolicy::AllowList {
            allowed_peers: vec![peer],
        };
        assert_eq!(allow_list.allows(peer), AllowResult::Allowed);
        assert_eq!(allow_list.allows(PeerId::new()), AllowResult::Denied);

        assert_eq!(
            SharingPolicy::RequestApproval.allows(peer),
            AllowResult::PendingApproval
        );
    }

    #[test]
    fn test_resource_request() {
        let from = PeerId::new();
        let request = ResourceRequest::new(from, "llm-aws_bedrock-us-east-1-claude")
            .with_duration(7200)
            .with_purpose("Large batch processing");

        assert_eq!(request.from, from);
        assert_eq!(request.duration_seconds, 7200);
        assert_eq!(request.purpose, Some("Large batch processing".to_string()));
    }

    #[test]
    fn test_resource_grant() {
        let grantee = PeerId::new();
        let grantor = PeerId::new();
        let grant = ResourceGrant::new(
            "llm-test",
            grantee,
            grantor,
            "http://localhost:8080/proxy/llm",
            Duration::from_secs(3600),
        )
        .with_limits(ResourceLimits {
            max_requests_per_minute: Some(100),
            ..Default::default()
        });

        assert!(!grant.is_expired());
        assert_eq!(grant.grantee, grantee);
        assert_eq!(grant.grantor, grantor);
        assert!(grant.limits.is_some());
    }

    #[test]
    fn test_colony_resource_registry() {
        let registry = ColonyResourceRegistry::new();
        let owner = PeerId::new();

        let llm_info = LlmResourceInfo::new("aws_bedrock", "claude-3-5-sonnet")
            .with_rate_limit(850)
            .with_latency(250);
        let resource = AdvertisedResource::llm(llm_info)
            .shareable()
            .with_policy(SharingPolicy::ColonyOpen);

        registry.upsert(owner, resource);

        assert_eq!(registry.total_count(), 1);
        assert_eq!(registry.llm_resources().len(), 1);
        assert!(registry
            .available_llm_models()
            .contains(&"claude-3-5-sonnet".to_string()));
    }

    #[test]
    fn test_healthiest_endpoint_selection() {
        let registry = ColonyResourceRegistry::new();

        // Add two endpoints for the same model with different latencies
        let owner1 = PeerId::new();
        let info1 = LlmResourceInfo::new("aws_bedrock", "claude-3-5-sonnet").with_latency(500);
        let resource1 = AdvertisedResource::llm(info1)
            .shareable()
            .with_policy(SharingPolicy::ColonyOpen);
        registry.upsert(owner1, resource1);

        let owner2 = PeerId::new();
        let info2 = LlmResourceInfo::new("aws_bedrock", "claude-3-5-sonnet").with_latency(200); // Lower latency
        let resource2 = AdvertisedResource::llm(info2)
            .shareable()
            .with_policy(SharingPolicy::ColonyOpen);
        registry.upsert(owner2, resource2);

        let healthiest = registry.healthiest_llm_endpoint("claude-3-5-sonnet");
        assert!(healthiest.is_some());

        // Should pick the one with lower latency
        if let Some(resource) = healthiest {
            if let ResourceType::Llm(info) = &resource.resource.resource_type {
                assert_eq!(info.latency_ms, 200);
            }
        }
    }

    #[test]
    fn test_resource_type_serialization() {
        let llm_info = LlmResourceInfo::new("aws_bedrock", "claude-3-5-sonnet");
        let resource_type = ResourceType::Llm(llm_info);

        let json = serde_json::to_string(&resource_type).unwrap();
        assert!(json.contains("\"type\":\"Llm\""));

        let parsed: ResourceType = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.type_name(), "llm");
    }

    #[test]
    fn test_resource_message_serialization() {
        let llm_info = LlmResourceInfo::new("aws_bedrock", "claude-3-5-sonnet");
        let resource = AdvertisedResource::llm(llm_info).with_policy(SharingPolicy::ColonyOpen);

        let message = ResourceMessage::Advertisement {
            resources: vec![resource],
        };

        let json = serde_json::to_string(&message).unwrap();
        assert!(json.contains("\"type\":\"Advertisement\""));

        let parsed: ResourceMessage = serde_json::from_str(&json).unwrap();
        if let ResourceMessage::Advertisement { resources } = parsed {
            assert_eq!(resources.len(), 1);
        } else {
            panic!("Wrong message type");
        }
    }
}
