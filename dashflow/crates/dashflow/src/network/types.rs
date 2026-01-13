// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

// Allow clippy warnings for network types
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::clone_on_ref_ptr)]

//! Core types for DashFlow network coordination.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};
use uuid::Uuid;

// ============================================================================
// Identity
// ============================================================================

/// Unique identifier for a peer on the network
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PeerId(Uuid);

impl PeerId {
    /// Create a new random peer ID
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Create from an existing UUID
    #[must_use]
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Get the underlying UUID
    #[must_use]
    pub fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl Default for PeerId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for PeerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", &self.0.to_string()[..8])
    }
}

/// Configuration for joining the network
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Human-readable name for this app
    pub name: String,

    /// App version
    pub version: String,

    /// Capabilities this app provides
    pub capabilities: Vec<String>,

    /// HTTP port to listen on (0 = auto-assign)
    pub http_port: u16,

    /// Custom metadata
    pub metadata: HashMap<String, String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            name: "DashflowApp".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            capabilities: Vec::new(),
            http_port: 0, // Auto-assign
            metadata: HashMap::new(),
        }
    }
}

impl AppConfig {
    /// Create a new app config with the given name
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Default::default()
        }
    }

    /// Add a capability
    #[must_use]
    pub fn with_capability(mut self, cap: impl Into<String>) -> Self {
        self.capabilities.push(cap.into());
        self
    }

    /// Add multiple capabilities
    #[must_use]
    pub fn with_capabilities(mut self, caps: Vec<String>) -> Self {
        self.capabilities.extend(caps);
        self
    }

    /// Set the HTTP port
    #[must_use]
    pub fn with_port(mut self, port: u16) -> Self {
        self.http_port = port;
        self
    }

    /// Add metadata
    #[must_use]
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

/// Identity of this app on the network
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkIdentity {
    /// Unique peer ID
    pub id: PeerId,

    /// App configuration
    pub config: AppConfig,

    /// When this identity was created
    pub created_at: DateTime<Utc>,

    /// Local endpoint address
    pub endpoint: Option<SocketAddr>,
}

impl NetworkIdentity {
    /// Create a new network identity from config
    #[must_use]
    pub fn new(config: AppConfig) -> Self {
        Self {
            id: PeerId::new(),
            config,
            created_at: Utc::now(),
            endpoint: None,
        }
    }

    /// Set the endpoint address
    #[must_use]
    pub fn with_endpoint(mut self, addr: SocketAddr) -> Self {
        self.endpoint = Some(addr);
        self
    }
}

// ============================================================================
// Peer Registry
// ============================================================================

/// Status of a known peer
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PeerStatus {
    /// Peer is online and responsive
    Online,
    /// Peer hasn't responded recently
    Stale,
    /// Peer announced departure
    Offline,
    /// Connection failed
    Unreachable,
}

/// Information about a discovered peer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    /// Peer's unique ID
    pub id: PeerId,

    /// Peer's name
    pub name: String,

    /// Peer's version
    pub version: String,

    /// Peer's capabilities
    pub capabilities: Vec<String>,

    /// Peer's endpoint
    pub endpoint: SocketAddr,

    /// Current status
    pub status: PeerStatus,

    /// When the peer was discovered
    pub discovered_at: DateTime<Utc>,

    /// When we last heard from the peer
    pub last_seen: DateTime<Utc>,

    /// Custom metadata from the peer
    pub metadata: HashMap<String, String>,
}

impl PeerInfo {
    /// Create new peer info from discovery
    #[must_use]
    pub fn new(id: PeerId, name: impl Into<String>, endpoint: SocketAddr) -> Self {
        let now = Utc::now();
        Self {
            id,
            name: name.into(),
            version: String::new(),
            capabilities: Vec::new(),
            endpoint,
            status: PeerStatus::Online,
            discovered_at: now,
            last_seen: now,
            metadata: HashMap::new(),
        }
    }

    /// Update last seen timestamp
    pub fn touch(&mut self) {
        self.last_seen = Utc::now();
        self.status = PeerStatus::Online;
    }

    /// Check if peer has a specific capability
    #[must_use]
    pub fn has_capability(&self, cap: &str) -> bool {
        self.capabilities.iter().any(|c| c == cap)
    }

    /// Mark peer as stale (no recent activity)
    pub fn mark_stale(&mut self) {
        self.status = PeerStatus::Stale;
    }

    /// Mark peer as offline (graceful departure)
    pub fn mark_offline(&mut self) {
        self.status = PeerStatus::Offline;
    }

    /// Mark peer as unreachable (connection failed)
    pub fn mark_unreachable(&mut self) {
        self.status = PeerStatus::Unreachable;
    }
}

/// Registry of known peers
#[derive(Debug, Default)]
pub struct PeerRegistry {
    /// Known peers indexed by ID
    peers: Arc<RwLock<HashMap<PeerId, PeerInfo>>>,
}

impl PeerRegistry {
    /// Create a new empty registry
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add or update a peer
    pub fn upsert(&self, peer: PeerInfo) {
        let mut peers = self.peers.write().expect("lock poisoned");
        peers.insert(peer.id, peer);
    }

    /// Get a peer by ID
    #[must_use]
    pub fn get(&self, id: PeerId) -> Option<PeerInfo> {
        self.peers.read().expect("lock poisoned").get(&id).cloned()
    }

    /// Remove a peer
    pub fn remove(&self, id: PeerId) -> Option<PeerInfo> {
        self.peers.write().expect("lock poisoned").remove(&id)
    }

    /// Get all online peers
    #[must_use]
    pub fn online_peers(&self) -> Vec<PeerInfo> {
        self.peers
            .read()
            .expect("lock poisoned")
            .values()
            .filter(|p| p.status == PeerStatus::Online)
            .cloned()
            .collect()
    }

    /// Get all known peers
    #[must_use]
    pub fn all_peers(&self) -> Vec<PeerInfo> {
        self.peers
            .read()
            .expect("lock poisoned")
            .values()
            .cloned()
            .collect()
    }

    /// Get peers with a specific capability
    #[must_use]
    pub fn peers_with_capability(&self, cap: &str) -> Vec<PeerInfo> {
        self.peers
            .read()
            .expect("lock poisoned")
            .values()
            .filter(|p| p.status == PeerStatus::Online && p.has_capability(cap))
            .cloned()
            .collect()
    }

    /// Count of online peers
    #[must_use]
    pub fn online_count(&self) -> usize {
        self.peers
            .read()
            .expect("lock poisoned")
            .values()
            .filter(|p| p.status == PeerStatus::Online)
            .count()
    }

    /// Total peer count
    #[must_use]
    pub fn total_count(&self) -> usize {
        self.peers.read().expect("lock poisoned").len()
    }

    /// Mark stale peers (no activity in given duration)
    pub fn mark_stale_peers(&self, stale_threshold: chrono::Duration) {
        let cutoff = Utc::now() - stale_threshold;
        let mut peers = self.peers.write().expect("lock poisoned");
        for peer in peers.values_mut() {
            if peer.status == PeerStatus::Online && peer.last_seen < cutoff {
                peer.status = PeerStatus::Stale;
            }
        }
    }

    /// Remove peers that have been offline for too long
    pub fn prune_old_peers(&self, prune_threshold: chrono::Duration) {
        let cutoff = Utc::now() - prune_threshold;
        let mut peers = self.peers.write().expect("lock poisoned");
        peers.retain(|_, p| {
            p.status == PeerStatus::Online || p.status == PeerStatus::Stale || p.last_seen > cutoff
        });
    }
}

impl Clone for PeerRegistry {
    fn clone(&self) -> Self {
        let peers = self.peers.read().expect("lock poisoned").clone();
        Self {
            peers: Arc::new(RwLock::new(peers)),
        }
    }
}

// ============================================================================
// Channels and Priority
// ============================================================================

/// Standard channel names
pub mod channels {
    /// Join/leave announcements
    pub const PRESENCE: &str = "_presence";
    /// Status updates ("working on X")
    pub const STATUS: &str = "_status";
    /// Suggestions from other apps
    pub const SUGGESTIONS: &str = "_suggestions";
    /// Bug reports from other apps
    pub const BUGS: &str = "_bugs";
    /// Error broadcasts
    pub const ERRORS: &str = "_errors";
    /// LLM rate limits and usage stats
    pub const LLM_USAGE: &str = "_llm_usage";
    /// Debug/verbose logging
    pub const DEBUG: &str = "_debug";
    /// Worker messages (progress, status requests)
    pub const WORKERS: &str = "_workers";
    /// Worker join announcements
    pub const WORKER_JOIN: &str = "_worker_join";
    /// Worker result reporting
    pub const WORKER_RESULT: &str = "_worker_result";
}

/// A channel for message routing
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Channel(String);

impl Channel {
    /// Create a new channel
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// Get the channel name
    #[must_use]
    pub fn name(&self) -> &str {
        &self.0
    }

    /// Check if this is a standard (built-in) channel
    #[must_use]
    pub fn is_standard(&self) -> bool {
        self.0.starts_with('_')
    }

    /// Create standard channels
    #[must_use]
    pub fn presence() -> Self {
        Self::new(channels::PRESENCE)
    }

    /// Create the status channel for health/liveness updates.
    #[must_use]
    pub fn status() -> Self {
        Self::new(channels::STATUS)
    }

    /// Create the suggestions channel for improvement proposals.
    #[must_use]
    pub fn suggestions() -> Self {
        Self::new(channels::SUGGESTIONS)
    }

    /// Create the bugs channel for bug reports and issues.
    #[must_use]
    pub fn bugs() -> Self {
        Self::new(channels::BUGS)
    }

    /// Create the errors channel for error notifications.
    #[must_use]
    pub fn errors() -> Self {
        Self::new(channels::ERRORS)
    }

    /// Create the LLM usage channel for API call tracking.
    #[must_use]
    pub fn llm_usage() -> Self {
        Self::new(channels::LLM_USAGE)
    }

    /// Create the debug channel for diagnostic messages.
    #[must_use]
    pub fn debug() -> Self {
        Self::new(channels::DEBUG)
    }

    /// Create the workers channel for worker coordination.
    #[must_use]
    pub fn workers() -> Self {
        Self::new(channels::WORKERS)
    }

    /// Create the worker join channel for worker registration.
    #[must_use]
    pub fn worker_join() -> Self {
        Self::new(channels::WORKER_JOIN)
    }

    /// Create the worker result channel for task completions.
    #[must_use]
    pub fn worker_result() -> Self {
        Self::new(channels::WORKER_RESULT)
    }
}

impl std::fmt::Display for Channel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Message priority levels
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Priority {
    /// FYI only - batched into periodic digest
    Background = 0,
    /// Regular coordination - delivered to queue
    #[default]
    Normal = 1,
    /// Conflicts, errors - ALWAYS delivered immediately
    Critical = 2,
}

/// How the app handles incoming messages
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum AttentionMode {
    /// All messages delivered immediately (noisy)
    Realtime,
    /// Only Critical immediate; Normal queued; Background digest
    #[default]
    Focused,
    /// Only Critical immediate; everything else in digest
    Minimal,
}

// ============================================================================
// Messages
// ============================================================================

/// Type of message
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageType {
    /// Heartbeat (no payload)
    Heartbeat,
    /// Status update
    Status,
    /// Suggestion
    Suggestion,
    /// Bug report
    BugReport,
    /// Error notification
    Error,
    /// Query (expects response)
    Query,
    /// Response to a query
    Response,
    /// Extended (full JSON)
    Extended,
}

impl MessageType {
    /// Get the compact type code
    #[must_use]
    pub fn type_code(&self) -> u8 {
        match self {
            Self::Heartbeat => 0x01,
            Self::Status => 0x02,
            Self::Suggestion => 0x03,
            Self::BugReport => 0x04,
            Self::Error => 0x05,
            Self::Query => 0x10,
            Self::Response => 0x11,
            Self::Extended => 0xFF,
        }
    }

    /// Create from type code
    #[must_use]
    pub fn from_code(code: u8) -> Option<Self> {
        match code {
            0x01 => Some(Self::Heartbeat),
            0x02 => Some(Self::Status),
            0x03 => Some(Self::Suggestion),
            0x04 => Some(Self::BugReport),
            0x05 => Some(Self::Error),
            0x10 => Some(Self::Query),
            0x11 => Some(Self::Response),
            0xFF => Some(Self::Extended),
            _ => None,
        }
    }
}

/// A network message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Unique message ID
    pub id: Uuid,

    /// Sender's peer ID
    pub from: PeerId,

    /// Target (broadcast, channel, or peer)
    pub to: MessageTarget,

    /// Message type
    pub msg_type: MessageType,

    /// Channel name
    pub channel: Channel,

    /// Message payload (JSON)
    pub payload: serde_json::Value,

    /// Priority level
    pub priority: Priority,

    /// When the message was sent
    pub timestamp: DateTime<Utc>,

    /// Time-to-live in seconds
    pub ttl: u32,

    /// ID of message this is replying to
    pub reply_to: Option<Uuid>,
}

/// Target for a message
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageTarget {
    /// Broadcast to all peers
    Broadcast,
    /// Send to specific channel subscribers
    Channel(Channel),
    /// Send to specific peer
    Peer(PeerId),
}

impl Message {
    /// Create a new broadcast message
    #[must_use]
    pub fn broadcast(
        from: PeerId,
        channel: Channel,
        msg_type: MessageType,
        payload: serde_json::Value,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            from,
            to: MessageTarget::Broadcast,
            msg_type,
            channel,
            payload,
            priority: Priority::Normal,
            timestamp: Utc::now(),
            ttl: 60,
            reply_to: None,
        }
    }

    /// Create a directed message to a peer
    #[must_use]
    pub fn to_peer(
        from: PeerId,
        to: PeerId,
        channel: Channel,
        msg_type: MessageType,
        payload: serde_json::Value,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            from,
            to: MessageTarget::Peer(to),
            msg_type,
            channel,
            payload,
            priority: Priority::Normal,
            timestamp: Utc::now(),
            ttl: 60,
            reply_to: None,
        }
    }

    /// Set priority
    #[must_use]
    pub fn with_priority(mut self, priority: Priority) -> Self {
        self.priority = priority;
        self
    }

    /// Set TTL
    #[must_use]
    pub fn with_ttl(mut self, ttl: u32) -> Self {
        self.ttl = ttl;
        self
    }

    /// Set reply-to
    #[must_use]
    pub fn replying_to(mut self, msg_id: Uuid) -> Self {
        self.reply_to = Some(msg_id);
        self
    }

    /// Check if message has expired
    #[must_use]
    pub fn is_expired(&self) -> bool {
        let age = Utc::now().signed_duration_since(self.timestamp);
        age.num_seconds() > i64::from(self.ttl)
    }
}

/// Compact binary message format (32-byte header)
#[derive(Debug, Clone)]
pub struct CompactMessage {
    /// Message type (1 byte)
    pub msg_type: MessageType,
    /// Priority (1 byte)
    pub priority: Priority,
    /// Sender UUID (16 bytes)
    pub from: PeerId,
    /// FNV hash of channel name (4 bytes)
    pub channel_hash: u32,
    /// Unix timestamp (4 bytes)
    pub timestamp: u32,
    /// Payload length (2 bytes)
    pub payload_len: u16,
    /// Flags (2 bytes)
    pub flags: u16,
    /// Payload bytes
    pub payload: Vec<u8>,
}

impl CompactMessage {
    /// Flag: message is encrypted
    pub const FLAG_ENCRYPTED: u16 = 0x0001;
    /// Flag: payload is compressed
    pub const FLAG_COMPRESSED: u16 = 0x0002;

    /// Calculate FNV-1a hash of channel name
    #[must_use]
    pub fn hash_channel(name: &str) -> u32 {
        let mut hash: u32 = 2166136261;
        for byte in name.bytes() {
            hash ^= u32::from(byte);
            hash = hash.wrapping_mul(16777619);
        }
        hash
    }
}

// ============================================================================
// LLM Usage
// ============================================================================

/// Status of an LLM endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmEndpointStatus {
    /// Provider name (e.g., "aws_bedrock", "openai")
    pub provider: String,

    /// Account identifier
    pub account: String,

    /// Region (for cloud providers)
    pub region: String,

    /// Model name
    pub model: String,

    /// Remaining rate limit
    pub rate_limit_remaining: u32,

    /// Remaining tokens (if applicable)
    pub tokens_remaining: Option<u64>,

    /// Whether this endpoint is saturated (others should avoid)
    pub saturated: bool,

    /// Recent latency in ms
    pub latency_ms: u32,
}

impl LlmEndpointStatus {
    /// Create a new endpoint status
    #[must_use]
    pub fn new(provider: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            provider: provider.into(),
            account: String::new(),
            region: String::new(),
            model: model.into(),
            rate_limit_remaining: u32::MAX,
            tokens_remaining: None,
            saturated: false,
            latency_ms: 0,
        }
    }

    /// Check if endpoint is available (not saturated, has capacity)
    #[must_use]
    pub fn is_available(&self) -> bool {
        !self.saturated && self.rate_limit_remaining > 10
    }
}

/// LLM usage report for sharing with peers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmUsageReport {
    /// Status of each endpoint
    pub endpoints: Vec<LlmEndpointStatus>,

    /// Total requests in last minute
    pub requests_last_minute: u32,

    /// When this report was generated
    pub timestamp: DateTime<Utc>,
}

impl LlmUsageReport {
    /// Create a new empty report
    #[must_use]
    pub fn new() -> Self {
        Self {
            endpoints: Vec::new(),
            requests_last_minute: 0,
            timestamp: Utc::now(),
        }
    }

    /// Add an endpoint status
    pub fn add_endpoint(&mut self, status: LlmEndpointStatus) {
        self.endpoints.push(status);
    }

    /// Get available endpoints sorted by preference (lowest latency, most capacity)
    #[must_use]
    pub fn available_endpoints(&self) -> Vec<&LlmEndpointStatus> {
        let mut available: Vec<_> = self.endpoints.iter().filter(|e| e.is_available()).collect();
        available.sort_by(|a, b| {
            // Prefer: not saturated, higher capacity, lower latency
            a.latency_ms.cmp(&b.latency_ms)
        });
        available
    }
}

impl Default for LlmUsageReport {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_peer_id() {
        let id1 = PeerId::new();
        let id2 = PeerId::new();
        assert_ne!(id1, id2);

        let display = format!("{}", id1);
        assert_eq!(display.len(), 8); // Short display
    }

    #[test]
    fn test_app_config() {
        let config = AppConfig::new("TestApp")
            .with_capability("code-editing")
            .with_capability("testing")
            .with_port(8080)
            .with_metadata("team", "platform");

        assert_eq!(config.name, "TestApp");
        assert_eq!(config.capabilities.len(), 2);
        assert_eq!(config.http_port, 8080);
        assert_eq!(config.metadata.get("team"), Some(&"platform".to_string()));
    }

    #[test]
    fn test_peer_registry() {
        let registry = PeerRegistry::new();
        let addr = "127.0.0.1:8080".parse().unwrap();

        let peer = PeerInfo::new(PeerId::new(), "TestPeer", addr);
        let id = peer.id;

        registry.upsert(peer);

        assert_eq!(registry.total_count(), 1);
        assert_eq!(registry.online_count(), 1);

        let retrieved = registry.get(id).unwrap();
        assert_eq!(retrieved.name, "TestPeer");
    }

    #[test]
    fn test_peer_capabilities() {
        let addr = "127.0.0.1:8080".parse().unwrap();
        let mut peer = PeerInfo::new(PeerId::new(), "TestPeer", addr);
        peer.capabilities = vec!["code-editing".to_string(), "testing".to_string()];

        assert!(peer.has_capability("code-editing"));
        assert!(peer.has_capability("testing"));
        assert!(!peer.has_capability("debugging"));
    }

    #[test]
    fn test_channel() {
        let ch = Channel::presence();
        assert!(ch.is_standard());
        assert_eq!(ch.name(), "_presence");

        let custom = Channel::new("my-team");
        assert!(!custom.is_standard());
    }

    #[test]
    fn test_message_type_codes() {
        assert_eq!(MessageType::Heartbeat.type_code(), 0x01);
        assert_eq!(MessageType::from_code(0x01), Some(MessageType::Heartbeat));
        assert_eq!(MessageType::from_code(0x99), None);
    }

    #[test]
    fn test_message_creation() {
        let from = PeerId::new();
        let msg = Message::broadcast(
            from,
            Channel::status(),
            MessageType::Status,
            serde_json::json!({"progress": 0.5}),
        )
        .with_priority(Priority::Background)
        .with_ttl(30);

        assert_eq!(msg.from, from);
        assert_eq!(msg.priority, Priority::Background);
        assert_eq!(msg.ttl, 30);
        assert!(!msg.is_expired());
    }

    #[test]
    fn test_compact_message_hash() {
        let hash1 = CompactMessage::hash_channel("_status");
        let hash2 = CompactMessage::hash_channel("_status");
        let hash3 = CompactMessage::hash_channel("_bugs");

        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_llm_endpoint_status() {
        let mut status = LlmEndpointStatus::new("aws_bedrock", "claude-3-5-sonnet");
        status.rate_limit_remaining = 100;
        status.latency_ms = 250;

        assert!(status.is_available());

        status.saturated = true;
        assert!(!status.is_available());
    }

    #[test]
    fn test_llm_usage_report() {
        let mut report = LlmUsageReport::new();

        let mut status1 = LlmEndpointStatus::new("aws_bedrock", "claude-3-5-sonnet");
        status1.rate_limit_remaining = 100;
        status1.latency_ms = 250;

        let mut status2 = LlmEndpointStatus::new("openai", "gpt-4");
        status2.rate_limit_remaining = 50;
        status2.saturated = true;

        report.add_endpoint(status1);
        report.add_endpoint(status2);

        let available = report.available_endpoints();
        assert_eq!(available.len(), 1);
        assert_eq!(available[0].provider, "aws_bedrock");
    }

    #[test]
    fn test_priority_ordering() {
        assert!(Priority::Critical > Priority::Normal);
        assert!(Priority::Normal > Priority::Background);
    }
}
