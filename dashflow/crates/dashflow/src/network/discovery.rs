// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! mDNS/DNS-SD discovery for automatic peer finding.
//!
//! This module provides zero-configuration service discovery on the local network
//! using mDNS (Multicast DNS) and DNS-SD (Service Discovery).

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;

use parking_lot::RwLock;
use thiserror::Error;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::constants::{
    DEFAULT_BROADCAST_CHANNEL_CAPACITY, DEFAULT_HEALTH_CHECK_INTERVAL, DEFAULT_MDNS_TTL_SECS,
    SHORT_TIMEOUT,
};
use super::types::{NetworkIdentity, PeerId, PeerInfo, PeerRegistry};

/// DNS-SD service type for DashFlow
pub const SERVICE_TYPE: &str = "_dashflow._tcp.local.";

/// Default mDNS TTL (time-to-live) in seconds
/// Now uses centralized constant from `crate::constants::DEFAULT_MDNS_TTL_SECS`
pub const DEFAULT_TTL: u32 = DEFAULT_MDNS_TTL_SECS;

/// Discovery timeout for initial scan (5 seconds)
/// Uses SHORT_TIMEOUT from constants for fast-fail on network scans.
pub const DISCOVERY_TIMEOUT: Duration = SHORT_TIMEOUT;

/// Heartbeat interval for re-announcing presence (30 seconds)
/// Uses DEFAULT_HEALTH_CHECK_INTERVAL from constants - same as health check frequency.
pub const HEARTBEAT_INTERVAL: Duration = DEFAULT_HEALTH_CHECK_INTERVAL;

// ============================================================================
// Errors
// ============================================================================

/// Errors from discovery operations
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum DiscoveryError {
    /// Failed to create mDNS responder
    #[error("Failed to create mDNS responder: {0}")]
    ResponderFailed(String),

    /// Failed to register service
    #[error("Failed to register service: {0}")]
    RegistrationFailed(String),

    /// Failed to start discovery
    #[error("Failed to start discovery: {0}")]
    BrowseFailed(String),

    /// Network interface error
    #[error("Network interface error: {0}")]
    InterfaceError(String),

    /// Discovery already running
    #[error("Discovery already running")]
    AlreadyRunning,

    /// Discovery not running
    #[error("Discovery not running")]
    NotRunning,

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

// ============================================================================
// Events
// ============================================================================

/// Events emitted by the discovery system
#[derive(Debug, Clone)]
pub enum DiscoveryEvent {
    /// A new peer was discovered
    PeerDiscovered(PeerInfo),

    /// A peer went offline (goodbye received)
    PeerGoodbye(PeerId),

    /// A peer's info was updated
    PeerUpdated(PeerInfo),

    /// Discovery started
    Started,

    /// Discovery stopped
    Stopped,

    /// Error during discovery
    Error(String),
}

// ============================================================================
// TXT Record Encoding (for future mDNS implementation)
// ============================================================================

/// TXT record keys for service metadata
#[allow(dead_code)] // Architectural: Part of future mDNS implementation
mod txt_keys {
    pub const PEER_ID: &str = "id";
    pub const VERSION: &str = "v";
    pub const CAPABILITIES: &str = "caps";
    pub const METADATA: &str = "meta";
}

/// Encode app metadata into DNS TXT record format
#[allow(dead_code)] // Architectural: Part of future mDNS implementation
fn encode_txt_records(identity: &NetworkIdentity) -> Vec<String> {
    let mut records = vec![
        format!("{}={}", txt_keys::PEER_ID, identity.id.as_uuid()),
        format!("{}={}", txt_keys::VERSION, identity.config.version),
    ];

    if !identity.config.capabilities.is_empty() {
        records.push(format!(
            "{}={}",
            txt_keys::CAPABILITIES,
            identity.config.capabilities.join(",")
        ));
    }

    if !identity.config.metadata.is_empty() {
        let meta: Vec<String> = identity
            .config
            .metadata
            .iter()
            .map(|(k, v)| format!("{}:{}", k, v))
            .collect();
        records.push(format!("{}={}", txt_keys::METADATA, meta.join(";")));
    }

    records
}

/// Decode DNS TXT records into peer metadata
#[allow(dead_code)] // Architectural: Part of future mDNS implementation
fn decode_txt_records(records: &[String]) -> ParsedTxtRecords {
    let mut parsed = ParsedTxtRecords::default();

    for record in records {
        if let Some((key, value)) = record.split_once('=') {
            match key {
                txt_keys::PEER_ID => {
                    if let Ok(uuid) = Uuid::parse_str(value) {
                        parsed.peer_id = Some(PeerId::from_uuid(uuid));
                    }
                }
                txt_keys::VERSION => {
                    parsed.version = Some(value.to_string());
                }
                txt_keys::CAPABILITIES => {
                    parsed.capabilities = value.split(',').map(String::from).collect();
                }
                txt_keys::METADATA => {
                    for pair in value.split(';') {
                        if let Some((k, v)) = pair.split_once(':') {
                            parsed.metadata.insert(k.to_string(), v.to_string());
                        }
                    }
                }
                _ => {}
            }
        }
    }

    parsed
}

#[allow(dead_code)] // Architectural: Part of future mDNS implementation
#[derive(Default)]
struct ParsedTxtRecords {
    peer_id: Option<PeerId>,
    version: Option<String>,
    capabilities: Vec<String>,
    metadata: HashMap<String, String>,
}

// ============================================================================
// Discovery Manager (Simplified - manual peer registration)
// ============================================================================

/// High-level discovery manager.
///
/// Note: Full mDNS implementation requires async stream pinning which has
/// compatibility issues. This is a simplified version that supports manual
/// peer registration and the event system.
pub struct DiscoveryManager {
    identity: NetworkIdentity,
    registry: Arc<PeerRegistry>,
    event_tx: broadcast::Sender<DiscoveryEvent>,
    running: Arc<RwLock<bool>>,
}

impl DiscoveryManager {
    /// Create a new discovery manager
    #[must_use]
    pub fn new(identity: NetworkIdentity, registry: Arc<PeerRegistry>) -> Self {
        let (event_tx, _) = broadcast::channel(DEFAULT_BROADCAST_CHANNEL_CAPACITY);
        Self {
            identity,
            registry,
            event_tx,
            running: Arc::new(RwLock::new(false)),
        }
    }

    /// Start discovery (marks as running)
    pub async fn start(&mut self) -> Result<(), DiscoveryError> {
        if *self.running.read() {
            return Err(DiscoveryError::AlreadyRunning);
        }

        *self.running.write() = true;
        if self.event_tx.send(DiscoveryEvent::Started).is_err() {
            tracing::debug!("No subscribers for discovery Started event");
        }

        tracing::info!(
            "Discovery started for '{}' (manual peer registration mode)",
            self.identity.config.name
        );

        Ok(())
    }

    /// Stop discovery
    pub async fn stop(&mut self) -> Result<(), DiscoveryError> {
        if !*self.running.read() {
            return Err(DiscoveryError::NotRunning);
        }

        *self.running.write() = false;
        if self.event_tx.send(DiscoveryEvent::Stopped).is_err() {
            tracing::debug!("No subscribers for discovery Stopped event");
        }

        Ok(())
    }

    /// Subscribe to discovery events
    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<DiscoveryEvent> {
        self.event_tx.subscribe()
    }

    /// Get the peer registry
    #[must_use]
    pub fn registry(&self) -> &Arc<PeerRegistry> {
        &self.registry
    }

    /// Check if discovery is running
    #[must_use]
    pub fn is_running(&self) -> bool {
        *self.running.read()
    }

    /// Manually register a discovered peer
    pub fn register_peer(&self, peer: PeerInfo) {
        let is_new = self.registry.get(peer.id).is_none();
        let peer_id = peer.id;
        let peer_name = peer.name.clone();
        self.registry.upsert(peer.clone());

        let event = if is_new {
            DiscoveryEvent::PeerDiscovered(peer)
        } else {
            DiscoveryEvent::PeerUpdated(peer)
        };

        if self.event_tx.send(event).is_err() {
            tracing::warn!(
                peer_id = %peer_id.as_uuid(),
                peer_name = %peer_name,
                is_new = is_new,
                "Discovery event dropped - no subscribers (cluster observers may have stale state)"
            );
        }
    }

    /// Remove a peer (mark as offline)
    pub fn remove_peer(&self, peer_id: PeerId) {
        if let Some(mut peer) = self.registry.get(peer_id) {
            peer.mark_offline();
            self.registry.upsert(peer);
        }
        if self
            .event_tx
            .send(DiscoveryEvent::PeerGoodbye(peer_id))
            .is_err()
        {
            tracing::warn!(
                peer_id = %peer_id.as_uuid(),
                "PeerGoodbye event dropped - no subscribers (cluster observers may have stale state)"
            );
        }
    }
}

// ============================================================================
// Mock Discovery (for testing without network)
// ============================================================================

/// Mock discovery for testing
pub struct MockDiscovery {
    registry: Arc<PeerRegistry>,
    event_tx: broadcast::Sender<DiscoveryEvent>,
}

impl MockDiscovery {
    /// Create a new mock discovery
    #[must_use]
    pub fn new(registry: Arc<PeerRegistry>) -> Self {
        let (event_tx, _) = broadcast::channel(DEFAULT_BROADCAST_CHANNEL_CAPACITY);
        Self { registry, event_tx }
    }

    /// Simulate discovering a peer
    pub fn simulate_peer_discovered(&self, peer: PeerInfo) {
        let peer_id = peer.id;
        self.registry.upsert(peer.clone());
        if self
            .event_tx
            .send(DiscoveryEvent::PeerDiscovered(peer))
            .is_err()
        {
            tracing::debug!(
                peer_id = %peer_id.as_uuid(),
                "MockDiscovery: PeerDiscovered event has no subscribers"
            );
        }
    }

    /// Simulate a peer going offline
    pub fn simulate_peer_goodbye(&self, peer_id: PeerId) {
        if let Some(mut peer) = self.registry.get(peer_id) {
            peer.mark_offline();
            self.registry.upsert(peer);
        }
        if self
            .event_tx
            .send(DiscoveryEvent::PeerGoodbye(peer_id))
            .is_err()
        {
            tracing::debug!(
                peer_id = %peer_id.as_uuid(),
                "MockDiscovery: PeerGoodbye event has no subscribers"
            );
        }
    }

    /// Subscribe to events
    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<DiscoveryEvent> {
        self.event_tx.subscribe()
    }

    /// Get the registry
    #[must_use]
    pub fn registry(&self) -> &Arc<PeerRegistry> {
        &self.registry
    }
}

impl Default for MockDiscovery {
    fn default() -> Self {
        Self::new(Arc::new(PeerRegistry::new()))
    }
}

// ============================================================================
// Helpers
// ============================================================================

/// Get local network addresses suitable for mDNS advertising
pub fn get_local_addresses() -> Result<Vec<IpAddr>, DiscoveryError> {
    let interfaces =
        if_addrs::get_if_addrs().map_err(|e| DiscoveryError::InterfaceError(e.to_string()))?;

    let addrs: Vec<IpAddr> = interfaces
        .into_iter()
        .filter(|iface| !iface.is_loopback())
        .map(|iface| iface.ip())
        .filter(|ip| match ip {
            IpAddr::V4(v4) => !v4.is_link_local(),
            IpAddr::V6(v6) => !v6.is_loopback(),
        })
        .collect();

    Ok(addrs)
}

/// Get the primary local IP address
#[must_use]
pub fn get_local_ip() -> Option<IpAddr> {
    get_local_addresses()
        .ok()?
        .into_iter()
        .find(|ip| ip.is_ipv4())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::{AppConfig, PeerStatus};
    use std::net::SocketAddr;

    #[test]
    fn test_txt_record_encoding() {
        let identity = NetworkIdentity::new(
            AppConfig::new("TestApp")
                .with_capability("code-editing")
                .with_capability("testing")
                .with_metadata("team", "platform"),
        );

        let records = encode_txt_records(&identity);

        assert!(records.iter().any(|r| r.starts_with("id=")));
        assert!(records.iter().any(|r| r.contains("code-editing")));
        assert!(records.iter().any(|r| r.contains("team:platform")));
    }

    #[test]
    fn test_txt_record_decoding() {
        let records = vec![
            "id=550e8400-e29b-41d4-a716-446655440000".to_string(),
            "v=1.0.0".to_string(),
            "caps=code-editing,testing".to_string(),
            "meta=team:platform;env:prod".to_string(),
        ];

        let parsed = decode_txt_records(&records);

        assert!(parsed.peer_id.is_some());
        assert_eq!(parsed.version, Some("1.0.0".to_string()));
        assert_eq!(parsed.capabilities.len(), 2);
        assert_eq!(parsed.metadata.get("team"), Some(&"platform".to_string()));
        assert_eq!(parsed.metadata.get("env"), Some(&"prod".to_string()));
    }

    #[test]
    fn test_mock_discovery() {
        let mock = MockDiscovery::default();
        let addr: SocketAddr = "192.168.1.100:8080".parse().unwrap();
        let peer = PeerInfo::new(PeerId::new(), "TestPeer", addr);
        let peer_id = peer.id;

        mock.simulate_peer_discovered(peer);

        assert_eq!(mock.registry().online_count(), 1);

        mock.simulate_peer_goodbye(peer_id);

        let peer = mock.registry().get(peer_id).unwrap();
        assert_eq!(peer.status, PeerStatus::Offline);
    }

    #[test]
    fn test_get_local_addresses() {
        let addrs = get_local_addresses();
        assert!(addrs.is_ok());
    }

    #[tokio::test]
    async fn test_discovery_manager() {
        let identity = NetworkIdentity::new(AppConfig::new("TestApp"));
        let registry = Arc::new(PeerRegistry::new());
        let mut manager = DiscoveryManager::new(identity, registry.clone());

        assert!(!manager.is_running());

        manager.start().await.unwrap();
        assert!(manager.is_running());

        let peer = PeerInfo::new(
            PeerId::new(),
            "TestPeer",
            "192.168.1.100:8080".parse().unwrap(),
        );
        manager.register_peer(peer.clone());

        assert_eq!(registry.online_count(), 1);

        manager.stop().await.unwrap();
        assert!(!manager.is_running());
    }
}
