// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! High-level network coordinator for DashFlow applications.
//!
//! The `DashflowNetwork` struct provides a unified API for:
//! - Joining the local network with mDNS discovery
//! - Starting an HTTP server for peer communication
//! - Sending and receiving messages
//! - Managing peer registry and subscriptions
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use dashflow::network::{DashflowNetwork, AppConfig, Priority};
//!
//! // Join the local network
//! let network = DashflowNetwork::join(AppConfig::new("MyAgent")).await?;
//!
//! // Send a status update
//! network.broadcast("_status", json!({"working_on": "feature-x"}), Priority::Background).await?;
//!
//! // Check for messages
//! while let Some(msg) = network.next_message().await {
//!     handle_message(msg);
//! }
//!
//! // Clean shutdown
//! network.leave().await?;
//! ```

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use crate::constants::DEFAULT_HTTP_REQUEST_TIMEOUT;
use chrono::Utc;
use parking_lot::RwLock as ParkingLotRwLock;
use serde_json::Value;
use thiserror::Error;
use tokio::sync::broadcast;
use uuid::Uuid;

use super::discovery::{DiscoveryEvent, DiscoveryManager, MockDiscovery};
use super::messaging::{MessageDigest, MessagingClient, MessagingError};
use super::server::{NetworkEvent, NetworkServer, ServerConfig, StatusInfo};
use super::types::{
    AppConfig, AttentionMode, Channel, Message, NetworkIdentity, PeerId, PeerInfo, PeerRegistry,
    Priority,
};

// ============================================================================
// Errors
// ============================================================================

/// Errors from network operations
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum NetworkError {
    /// Failed to start server
    #[error("Failed to start server: {0}")]
    ServerError(String),

    /// Failed to start discovery
    #[error("Failed to start discovery: {0}")]
    DiscoveryError(#[from] super::discovery::DiscoveryError),

    /// Messaging error
    #[error("Messaging error: {0}")]
    MessagingError(#[from] MessagingError),

    /// Network not started
    #[error("Network not started - call join() first")]
    NotStarted,

    /// Network already started
    #[error("Network already started")]
    AlreadyStarted,

    /// Peer not found
    #[error("Peer not found: {0}")]
    PeerNotFound(PeerId),

    /// Request timeout
    #[error("Request timed out after {0:?}")]
    Timeout(Duration),

    /// HTTP client error
    #[error("HTTP error: {0}")]
    HttpError(String),
}

// ============================================================================
// Configuration
// ============================================================================

/// Configuration for joining the network
#[derive(Debug, Clone)]
pub struct NetworkConfig {
    /// App configuration
    pub app: AppConfig,

    /// Server configuration
    pub server: ServerConfig,

    /// Enable mDNS discovery (default: true)
    pub enable_discovery: bool,

    /// Enable HTTP server (default: true)
    pub enable_server: bool,

    /// Default timeout for requests
    pub default_timeout: Duration,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            app: AppConfig::default(),
            server: ServerConfig::default(),
            enable_discovery: true,
            enable_server: true,
            default_timeout: DEFAULT_HTTP_REQUEST_TIMEOUT, // 30s from constants
        }
    }
}

impl NetworkConfig {
    /// Create a new config with the given app config
    #[must_use]
    pub fn new(app: AppConfig) -> Self {
        Self {
            app,
            ..Default::default()
        }
    }

    /// Set the server port
    #[must_use]
    pub fn with_port(mut self, port: u16) -> Self {
        self.server.port = port;
        self
    }

    /// Disable mDNS discovery
    #[must_use]
    pub fn without_discovery(mut self) -> Self {
        self.enable_discovery = false;
        self
    }

    /// Disable HTTP server
    #[must_use]
    pub fn without_server(mut self) -> Self {
        self.enable_server = false;
        self
    }

    /// Set default timeout
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.default_timeout = timeout;
        self
    }
}

// ============================================================================
// Network State
// ============================================================================

/// Internal state shared across the network
struct NetworkState {
    identity: NetworkIdentity,
    registry: Arc<PeerRegistry>,
    messaging: MessagingClient,
    server: Option<NetworkServer>,
    discovery: Option<DiscoveryManager>,
    mock_discovery: Option<MockDiscovery>,
    status: ParkingLotRwLock<StatusInfo>,
    #[allow(dead_code)] // Architectural: Reserved for future config-based features
    config: NetworkConfig,
}

// ============================================================================
// DashflowNetwork
// ============================================================================

/// High-level network coordinator for DashFlow applications.
///
/// Provides a unified API for peer discovery, messaging, and coordination.
pub struct DashflowNetwork {
    state: Arc<NetworkState>,
    #[allow(dead_code)] // Architectural: Reserved for future event processing
    event_rx: Option<broadcast::Receiver<NetworkEvent>>,
    #[allow(dead_code)] // Architectural: Reserved for future event processing
    discovery_rx: Option<broadcast::Receiver<DiscoveryEvent>>,
}

impl DashflowNetwork {
    /// Join the local network with default configuration.
    ///
    /// This starts the HTTP server and mDNS discovery.
    pub async fn join(app: AppConfig) -> Result<Self, NetworkError> {
        Self::join_with_config(NetworkConfig::new(app)).await
    }

    /// Join the local network with custom configuration.
    pub async fn join_with_config(config: NetworkConfig) -> Result<Self, NetworkError> {
        let mut identity = NetworkIdentity::new(config.app.clone());
        let registry = Arc::new(PeerRegistry::new());
        let messaging = MessagingClient::new(identity.clone());

        // Start HTTP server if enabled
        let (server, endpoint, event_rx) = if config.enable_server {
            let server = NetworkServer::new(identity.clone(), config.server.clone());
            let addr = server
                .start()
                .await
                .map_err(|e| NetworkError::ServerError(e.to_string()))?;
            let rx = server.state().subscribe_events();
            (Some(server), Some(addr), Some(rx))
        } else {
            (None, None, None)
        };

        // Update identity with endpoint
        if let Some(addr) = endpoint {
            identity = identity.with_endpoint(addr);
        }

        // Start discovery if enabled
        let (discovery, discovery_rx) = if config.enable_discovery && endpoint.is_some() {
            let mut discovery = DiscoveryManager::new(identity.clone(), Arc::clone(&registry));
            discovery.start().await?;
            let rx = discovery.subscribe();
            (Some(discovery), Some(rx))
        } else {
            (None, None)
        };

        let state = Arc::new(NetworkState {
            identity,
            registry,
            messaging,
            server,
            discovery,
            mock_discovery: None,
            status: ParkingLotRwLock::new(StatusInfo {
                working_on: String::new(),
                progress: 0.0,
                updated_at: Utc::now(),
                details: None,
            }),
            config,
        });

        Ok(Self {
            state,
            event_rx,
            discovery_rx,
        })
    }

    /// Create a mock network for testing (no actual networking).
    #[must_use]
    pub fn mock(app: AppConfig) -> Self {
        let identity = NetworkIdentity::new(app.clone());
        let registry = Arc::new(PeerRegistry::new());
        let messaging = MessagingClient::new(identity.clone());
        let mock_discovery = MockDiscovery::new(Arc::clone(&registry));

        let state = Arc::new(NetworkState {
            identity,
            registry,
            messaging,
            server: None,
            discovery: None,
            mock_discovery: Some(mock_discovery),
            status: ParkingLotRwLock::new(StatusInfo {
                working_on: String::new(),
                progress: 0.0,
                updated_at: Utc::now(),
                details: None,
            }),
            config: NetworkConfig::new(app).without_discovery().without_server(),
        });

        Self {
            state,
            event_rx: None,
            discovery_rx: None,
        }
    }

    /// Leave the network gracefully.
    pub async fn leave(self) -> Result<(), NetworkError> {
        // Stop discovery first
        if self.state.discovery.is_some() {
            tracing::info!("Leaving network (discovery will stop on drop)");
        }

        // Server stops on drop
        if self.state.server.is_some() {
            tracing::info!("Stopping HTTP server");
        }

        Ok(())
    }

    // ========================================================================
    // Identity & Status
    // ========================================================================

    /// Get this app's peer ID
    #[must_use]
    pub fn peer_id(&self) -> PeerId {
        self.state.identity.id
    }

    /// Get this app's identity
    #[must_use]
    pub fn identity(&self) -> &NetworkIdentity {
        &self.state.identity
    }

    /// Get the local endpoint address
    #[must_use]
    pub fn endpoint(&self) -> Option<SocketAddr> {
        self.state.identity.endpoint
    }

    /// Update status (what we're working on)
    pub fn set_status(&self, working_on: impl Into<String>, progress: f64) {
        let mut status = self.state.status.write();
        status.working_on = working_on.into();
        status.progress = progress;
        status.updated_at = Utc::now();
    }

    /// Clear status
    pub fn clear_status(&self) {
        let mut status = self.state.status.write();
        status.working_on = String::new();
        status.progress = 0.0;
        status.updated_at = Utc::now();
    }

    /// Set attention mode
    pub async fn set_attention_mode(&self, mode: AttentionMode) {
        self.state.messaging.set_attention_mode(mode).await;
    }

    /// Get current attention mode
    pub async fn attention_mode(&self) -> AttentionMode {
        self.state.messaging.attention_mode().await
    }

    // ========================================================================
    // Peers
    // ========================================================================

    /// Get all online peers
    #[must_use]
    pub fn peers(&self) -> Vec<PeerInfo> {
        self.state.registry.online_peers()
    }

    /// Get a specific peer
    #[must_use]
    pub fn get_peer(&self, id: PeerId) -> Option<PeerInfo> {
        self.state.registry.get(id)
    }

    /// Find peers with a specific capability
    #[must_use]
    pub fn peers_with_capability(&self, capability: &str) -> Vec<PeerInfo> {
        self.state.registry.peers_with_capability(capability)
    }

    /// Get count of online peers
    #[must_use]
    pub fn peer_count(&self) -> usize {
        self.state.registry.online_count()
    }

    /// Get the peer registry
    #[must_use]
    pub fn registry(&self) -> &PeerRegistry {
        &self.state.registry
    }

    // ========================================================================
    // Subscriptions
    // ========================================================================

    /// Subscribe to a channel
    pub async fn subscribe(&self, channel: &str) {
        self.state.messaging.subscribe(Channel::new(channel)).await;
    }

    /// Unsubscribe from a channel
    pub async fn unsubscribe(&self, channel: &str) {
        self.state
            .messaging
            .unsubscribe(Channel::new(channel))
            .await;
    }

    /// Check if subscribed to a channel
    pub async fn is_subscribed(&self, channel: &str) -> bool {
        let channels = self.state.messaging.subscribed_channels().await;
        channels.contains(&channel.to_string())
    }

    // ========================================================================
    // Messaging
    // ========================================================================

    /// Broadcast a message to all peers on a channel.
    pub async fn broadcast(
        &self,
        channel: &str,
        payload: Value,
        priority: Priority,
    ) -> Result<Uuid, NetworkError> {
        let msg_id = self
            .state
            .messaging
            .broadcast(channel, payload, priority)
            .await?;
        Ok(msg_id)
    }

    /// Send a message to a specific peer.
    pub async fn send_to(
        &self,
        peer_id: PeerId,
        channel: &str,
        payload: Value,
        priority: Priority,
    ) -> Result<Uuid, NetworkError> {
        let msg_id = self
            .state
            .messaging
            .send_to(peer_id, channel, payload, priority)
            .await?;
        Ok(msg_id)
    }

    /// Send a suggestion to a peer.
    pub async fn send_suggestion(
        &self,
        peer_id: PeerId,
        suggestion: &str,
        context: Option<Value>,
    ) -> Result<Uuid, NetworkError> {
        let msg_id = self
            .state
            .messaging
            .send_suggestion(peer_id, suggestion, context)
            .await?;
        Ok(msg_id)
    }

    /// Send a bug report to a peer.
    pub async fn send_bug_report(
        &self,
        peer_id: PeerId,
        bug: &str,
        details: Option<Value>,
    ) -> Result<Uuid, NetworkError> {
        let msg_id = self
            .state
            .messaging
            .send_bug_report(peer_id, bug, details)
            .await?;
        Ok(msg_id)
    }

    /// Broadcast an error to all peers.
    pub async fn broadcast_error(
        &self,
        error: &str,
        details: Option<Value>,
    ) -> Result<Uuid, NetworkError> {
        let msg_id = self.state.messaging.broadcast_error(error, details).await?;
        Ok(msg_id)
    }

    /// Broadcast a status update.
    pub async fn broadcast_status(
        &self,
        working_on: &str,
        progress: f64,
    ) -> Result<Uuid, NetworkError> {
        let msg_id = self
            .state
            .messaging
            .broadcast_status(working_on, progress)
            .await?;
        Ok(msg_id)
    }

    /// Send a request and wait for response.
    pub async fn request(
        &self,
        peer_id: PeerId,
        topic: &str,
        query: Value,
    ) -> Result<Message, NetworkError> {
        let response = self.state.messaging.request(peer_id, topic, query).await?;
        Ok(response)
    }

    /// Send a request with custom timeout.
    pub async fn request_with_timeout(
        &self,
        peer_id: PeerId,
        topic: &str,
        query: Value,
        timeout: chrono::Duration,
    ) -> Result<Message, NetworkError> {
        let response = self
            .state
            .messaging
            .request_with_timeout(peer_id, topic, query, timeout)
            .await?;
        Ok(response)
    }

    // ========================================================================
    // Message Reception
    // ========================================================================

    /// Get the next message from the inbox.
    ///
    /// Returns `None` if no messages are available.
    pub async fn next_message(&self) -> Option<Message> {
        self.state.messaging.next_message().await
    }

    /// Get all pending messages (peek without removing).
    pub async fn peek_messages(&self) -> Vec<Message> {
        self.state.messaging.peek_messages().await
    }

    /// Get the message digest (summary of background messages).
    pub async fn digest(&self) -> MessageDigest {
        self.state.messaging.peek_digest().await
    }

    /// Take the message digest (clears after reading).
    pub async fn take_digest(&self) -> MessageDigest {
        self.state.messaging.take_digest().await
    }

    /// Handle an incoming message.
    ///
    /// This processes the message according to attention mode and subscriptions.
    pub async fn handle_incoming(&self, msg: Message) {
        self.state.messaging.handle_incoming(msg).await;
    }

    // ========================================================================
    // Events
    // ========================================================================

    /// Subscribe to network events (server events).
    #[must_use]
    pub fn subscribe_events(&self) -> Option<broadcast::Receiver<NetworkEvent>> {
        self.state
            .server
            .as_ref()
            .map(|s| s.state().subscribe_events())
    }

    /// Subscribe to discovery events.
    #[must_use]
    pub fn subscribe_discovery(&self) -> Option<broadcast::Receiver<DiscoveryEvent>> {
        self.state.discovery.as_ref().map(|d| d.subscribe())
    }

    // ========================================================================
    // Mock Helpers (for testing)
    // ========================================================================

    /// Simulate discovering a peer (mock mode only).
    pub fn mock_peer_discovered(&self, peer: PeerInfo) {
        if let Some(ref mock) = self.state.mock_discovery {
            mock.simulate_peer_discovered(peer);
        }
    }

    /// Simulate receiving a message (mock mode only).
    pub async fn mock_receive_message(&self, msg: Message) {
        self.state.messaging.handle_incoming(msg).await;
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::MessageType;

    #[test]
    fn test_network_config() {
        let config = NetworkConfig::new(AppConfig::new("TestApp"))
            .with_port(8080)
            .with_timeout(Duration::from_secs(60))
            .without_discovery();

        assert_eq!(config.server.port, 8080);
        assert_eq!(config.default_timeout, Duration::from_secs(60));
        assert!(!config.enable_discovery);
    }

    #[test]
    fn test_mock_network() {
        let network = DashflowNetwork::mock(AppConfig::new("TestApp"));

        assert!(network.endpoint().is_none());
        assert_eq!(network.peer_count(), 0);
    }

    #[test]
    fn test_mock_peer_discovery() {
        let network = DashflowNetwork::mock(AppConfig::new("TestApp"));

        let peer = PeerInfo::new(
            PeerId::new(),
            "MockPeer",
            "192.168.1.100:8080".parse().unwrap(),
        );
        network.mock_peer_discovered(peer.clone());

        assert_eq!(network.peer_count(), 1);
        assert!(network.get_peer(peer.id).is_some());
    }

    #[tokio::test]
    async fn test_subscriptions() {
        let network = DashflowNetwork::mock(AppConfig::new("TestApp"));

        // Custom subscription
        network.subscribe("custom:my-team").await;
        assert!(network.is_subscribed("custom:my-team").await);

        network.unsubscribe("custom:my-team").await;
        assert!(!network.is_subscribed("custom:my-team").await);
    }

    #[tokio::test]
    async fn test_attention_mode() {
        let network = DashflowNetwork::mock(AppConfig::new("TestApp"));

        assert_eq!(network.attention_mode().await, AttentionMode::Focused);

        network.set_attention_mode(AttentionMode::Minimal).await;
        assert_eq!(network.attention_mode().await, AttentionMode::Minimal);
    }

    #[test]
    fn test_status() {
        let network = DashflowNetwork::mock(AppConfig::new("TestApp"));

        network.set_status("feature-x", 0.5_f64);
        network.clear_status();
        // Just verify no panic
    }

    #[tokio::test]
    async fn test_mock_receive_message() {
        let network = DashflowNetwork::mock(AppConfig::new("TestApp"));

        // Subscribe to status channel first
        network.subscribe("_status").await;

        let msg = Message::broadcast(
            PeerId::new(),
            Channel::status(),
            MessageType::Status,
            serde_json::json!({"progress": 0.5}),
        )
        .with_priority(Priority::Normal);

        network.mock_receive_message(msg).await;

        // In Focused mode, Normal priority goes to inbox
        let received = network.next_message().await;
        assert!(received.is_some());
    }

    #[tokio::test]
    async fn test_digest() {
        let network = DashflowNetwork::mock(AppConfig::new("TestApp"));

        // Subscribe to status channel
        network.subscribe("_status").await;

        // Background messages go to digest in Focused mode
        let msg = Message::broadcast(
            PeerId::new(),
            Channel::status(),
            MessageType::Status,
            serde_json::json!({"progress": 0.5}),
        )
        .with_priority(Priority::Background);

        network.mock_receive_message(msg).await;

        let digest = network.digest().await;
        assert_eq!(digest.status_updates, 1);

        let taken = network.take_digest().await;
        assert_eq!(taken.status_updates, 1);

        // After take, digest should be cleared
        let empty_digest = network.digest().await;
        assert_eq!(empty_digest.status_updates, 0);
    }
}
