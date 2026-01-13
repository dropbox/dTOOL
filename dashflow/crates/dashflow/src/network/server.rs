// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! HTTP server and WebSocket infrastructure for DashFlow network coordination.
//!
//! ## Endpoints
//!
//! | Method | Path | Description |
//! |--------|------|-------------|
//! | GET | `/dashflow/status` | App identity, capabilities, current work |
//! | GET | `/dashflow/peers` | Known peers (for peer exchange) |
//! | POST | `/dashflow/message` | Receive incoming message |
//! | GET | `/dashflow/introspect` | Full introspection (for coordination) |
//! | WS | `/dashflow/events` | Real-time event stream |
//!
//! ## Usage
//!
//! ```rust,ignore
//! use dashflow::network::{NetworkServer, ServerConfig, NetworkIdentity, AppConfig};
//!
//! let identity = NetworkIdentity::new(AppConfig::new("MyAgent"));
//! let server = NetworkServer::new(identity, ServerConfig::default());
//!
//! // Start server (returns bound address)
//! let addr = server.start().await?;
//! println!("Server listening on {}", addr);
//! ```

use axum::{
    extract::{
        ws::{Message as WsMessage, WebSocket, WebSocketUpgrade},
        State,
    },
    response::{Json, Response},
    routing::{get, post},
    Router,
};
#[cfg(feature = "network")]
use thiserror::Error;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use uuid::Uuid;

use crate::constants::{
    DEFAULT_QUEUE_CAPACITY, DEFAULT_RESOURCE_BROADCAST_INTERVAL_SECS, DEFAULT_WS_CHANNEL_CAPACITY,
};
use super::types::{
    AttentionMode, Message, MessageType, NetworkIdentity, PeerId, PeerInfo, PeerRegistry, Priority,
};

// ============================================================================
// Server Configuration
// ============================================================================

/// Configuration for the network server
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Port to bind to (0 = auto-assign)
    pub port: u16,

    /// Host to bind to
    pub host: String,

    /// Maximum message queue size
    pub max_queue_size: usize,

    /// Maximum messages per peer per second
    pub rate_limit_per_peer: u32,

    /// Default TTL for messages in seconds
    pub default_ttl: u32,

    /// WebSocket channel capacity
    pub ws_channel_capacity: usize,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            port: 0, // Auto-assign
            host: "0.0.0.0".to_string(),
            max_queue_size: DEFAULT_QUEUE_CAPACITY,
            rate_limit_per_peer: 10,
            default_ttl: DEFAULT_RESOURCE_BROADCAST_INTERVAL_SECS,
            ws_channel_capacity: DEFAULT_WS_CHANNEL_CAPACITY,
        }
    }
}

impl ServerConfig {
    /// Create config with specific port
    #[must_use]
    pub fn with_port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    /// Create config with specific host
    #[must_use]
    pub fn with_host(mut self, host: impl Into<String>) -> Self {
        self.host = host.into();
        self
    }

    /// Set max queue size
    #[must_use]
    pub fn with_max_queue_size(mut self, size: usize) -> Self {
        self.max_queue_size = size;
        self
    }
}

// ============================================================================
// Server State
// ============================================================================

/// Shared server state
pub struct ServerState {
    /// This server's identity
    pub identity: NetworkIdentity,

    /// Known peers
    pub peers: PeerRegistry,

    /// Message queue for incoming messages
    pub message_queue: RwLock<VecDeque<Message>>,

    /// Current attention mode
    pub attention_mode: RwLock<AttentionMode>,

    /// Broadcast channel for WebSocket events
    pub event_tx: broadcast::Sender<NetworkEvent>,

    /// Server configuration
    pub config: ServerConfig,

    /// Current status (what we're working on)
    pub current_status: RwLock<Option<StatusInfo>>,

    /// Rate limit tracking per peer
    pub rate_limits: RwLock<std::collections::HashMap<PeerId, RateLimitTracker>>,
}

/// Current status information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusInfo {
    /// What we're currently working on
    pub working_on: String,
    /// Progress (0.0 to 1.0)
    pub progress: f64,
    /// When this status was set
    pub updated_at: DateTime<Utc>,
    /// Additional details
    pub details: Option<serde_json::Value>,
}

/// Rate limit tracking for a peer
#[derive(Debug, Clone)]
pub struct RateLimitTracker {
    /// Message count in current window
    pub count: u32,
    /// Window start time
    pub window_start: DateTime<Utc>,
}

impl RateLimitTracker {
    fn new() -> Self {
        Self {
            count: 0,
            window_start: Utc::now(),
        }
    }

    fn check_and_increment(&mut self, limit: u32) -> bool {
        let now = Utc::now();
        let elapsed = now.signed_duration_since(self.window_start);

        // Reset window if more than 1 second has passed
        if elapsed.num_seconds() >= 1 {
            self.count = 0;
            self.window_start = now;
        }

        if self.count >= limit {
            false
        } else {
            self.count += 1;
            true
        }
    }
}

impl ServerState {
    /// Create new server state
    #[must_use]
    pub fn new(identity: NetworkIdentity, config: ServerConfig) -> Self {
        let (event_tx, _) = broadcast::channel(config.ws_channel_capacity);
        Self {
            identity,
            peers: PeerRegistry::new(),
            message_queue: RwLock::new(VecDeque::new()),
            attention_mode: RwLock::new(AttentionMode::default()),
            event_tx,
            config,
            current_status: RwLock::new(None),
            rate_limits: RwLock::new(std::collections::HashMap::new()),
        }
    }

    /// Set current status
    pub async fn set_status(&self, working_on: impl Into<String>, progress: f64) {
        let mut status = self.current_status.write().await;
        *status = Some(StatusInfo {
            working_on: working_on.into(),
            progress: progress.clamp(0.0, 1.0),
            updated_at: Utc::now(),
            details: None,
        });
    }

    /// Clear current status
    pub async fn clear_status(&self) {
        let mut status = self.current_status.write().await;
        *status = None;
    }

    /// Enqueue a message, respecting rate limits and queue size
    pub async fn enqueue_message(&self, msg: Message) -> Result<(), EnqueueError> {
        // Check rate limit
        {
            let mut rate_limits = self.rate_limits.write().await;
            let tracker = rate_limits
                .entry(msg.from)
                .or_insert_with(RateLimitTracker::new);
            if !tracker.check_and_increment(self.config.rate_limit_per_peer) {
                return Err(EnqueueError::RateLimited);
            }
        }

        // Check if expired
        if msg.is_expired() {
            return Err(EnqueueError::Expired);
        }

        // Enqueue with eviction if needed
        let mut queue = self.message_queue.write().await;
        if queue.len() >= self.config.max_queue_size {
            // Evict lowest priority message
            self.evict_lowest_priority(&mut queue);
        }
        queue.push_back(msg.clone());

        // Broadcast event for WebSocket subscribers
        let _ = self.event_tx.send(NetworkEvent::MessageReceived {
            id: msg.id,
            from: msg.from,
            msg_type: msg.msg_type,
            channel: msg.channel.name().to_string(),
            priority: msg.priority,
        });

        Ok(())
    }

    /// Evict lowest priority message from queue
    fn evict_lowest_priority(&self, queue: &mut VecDeque<Message>) {
        // Find index of lowest priority message (prefer older)
        let mut lowest_idx = None;
        let mut lowest_priority = Priority::Critical;

        for (idx, msg) in queue.iter().enumerate() {
            if msg.priority < lowest_priority
                || (msg.priority == lowest_priority && lowest_idx.is_none())
            {
                lowest_priority = msg.priority;
                lowest_idx = Some(idx);
            }
        }

        if let Some(idx) = lowest_idx {
            queue.remove(idx);
        }
    }

    /// Get next message respecting attention mode
    pub async fn next_message(&self) -> Option<Message> {
        let mode = *self.attention_mode.read().await;
        let mut queue = self.message_queue.write().await;

        match mode {
            AttentionMode::Realtime => queue.pop_front(),
            AttentionMode::Focused => {
                // Only return Critical immediately, Normal stays in queue
                if let Some(idx) = queue.iter().position(|m| m.priority == Priority::Critical) {
                    queue.remove(idx)
                } else {
                    queue.pop_front()
                }
            }
            AttentionMode::Minimal => {
                // Only return Critical
                if let Some(idx) = queue.iter().position(|m| m.priority == Priority::Critical) {
                    queue.remove(idx)
                } else {
                    None
                }
            }
        }
    }

    /// Get message queue length
    pub async fn queue_len(&self) -> usize {
        self.message_queue.read().await.len()
    }

    /// Set attention mode
    pub async fn set_attention_mode(&self, mode: AttentionMode) {
        let mut current = self.attention_mode.write().await;
        *current = mode;
    }

    /// Subscribe to WebSocket events
    pub fn subscribe_events(&self) -> broadcast::Receiver<NetworkEvent> {
        self.event_tx.subscribe()
    }
}

/// Error when enqueuing a message
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum EnqueueError {
    /// Rate limit exceeded
    #[error("Rate limit exceeded")]
    RateLimited,
    /// Message has expired
    #[error("Message expired")]
    Expired,
    /// Queue is full (shouldn't happen with eviction)
    #[error("Message queue full")]
    QueueFull,
}

// ============================================================================
// Events (for WebSocket broadcast)
// ============================================================================

/// Events broadcast to WebSocket subscribers
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NetworkEvent {
    /// A message was received
    MessageReceived {
        /// Unique message ID
        id: Uuid,
        /// Sender peer ID
        from: PeerId,
        /// Type of message
        msg_type: MessageType,
        /// Channel the message was sent on
        channel: String,
        /// Message priority
        priority: Priority,
    },
    /// A peer joined the network
    PeerJoined {
        /// Peer ID of the joining peer
        id: PeerId,
        /// Display name of the peer
        name: String,
        /// Capabilities advertised by the peer
        capabilities: Vec<String>,
    },
    /// A peer left the network
    PeerLeft {
        /// Peer ID of the leaving peer
        id: PeerId,
        /// Display name of the peer
        name: String,
    },
    /// A peer's status changed
    PeerStatusChanged {
        /// Peer ID whose status changed
        id: PeerId,
        /// New status of the peer
        status: super::types::PeerStatus,
    },
    /// Server status update
    StatusUpdate {
        /// What the server is currently working on
        working_on: String,
        /// Progress as a value from 0.0 to 1.0
        progress: f64,
    },
}

// ============================================================================
// HTTP Handlers
// ============================================================================

/// Response for /dashflow/status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusResponse {
    /// Peer ID
    pub id: PeerId,
    /// App name
    pub name: String,
    /// App version
    pub version: String,
    /// Capabilities
    pub capabilities: Vec<String>,
    /// Endpoint address
    pub endpoint: Option<SocketAddr>,
    /// Current status
    pub status: Option<StatusInfo>,
    /// Online peer count
    pub peer_count: usize,
    /// Message queue length
    pub queue_length: usize,
    /// Server uptime
    pub uptime_seconds: i64,
    /// Current attention mode
    pub attention_mode: AttentionMode,
}

/// Response for /dashflow/peers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeersResponse {
    /// List of known peers
    pub peers: Vec<PeerInfo>,
    /// Total count
    pub total: usize,
    /// Online count
    pub online: usize,
}

/// Request body for /dashflow/message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageRequest {
    /// Message to send
    pub message: Message,
}

/// Response for /dashflow/message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageResponse {
    /// Whether the message was accepted
    pub accepted: bool,
    /// Message ID
    pub message_id: Uuid,
    /// Error message if not accepted
    pub error: Option<String>,
}

/// Response for /dashflow/introspect
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntrospectResponse {
    /// Server identity
    pub identity: NetworkIdentity,
    /// Current status
    pub status: Option<StatusInfo>,
    /// Known peers
    pub peers: Vec<PeerInfo>,
    /// Message queue stats
    pub queue_stats: QueueStats,
    /// Attention mode
    pub attention_mode: AttentionMode,
}

/// Queue statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueStats {
    /// Current queue length
    pub length: usize,
    /// Max queue size
    pub max_size: usize,
    /// Messages by priority
    pub by_priority: std::collections::HashMap<String, usize>,
}

// ============================================================================
// HTTP Handler Implementations
// ============================================================================

#[cfg(feature = "network")]
async fn handle_status(State(state): State<Arc<ServerState>>) -> Json<StatusResponse> {
    let status = state.current_status.read().await.clone();
    let attention_mode = *state.attention_mode.read().await;

    Json(StatusResponse {
        id: state.identity.id,
        name: state.identity.config.name.clone(),
        version: state.identity.config.version.clone(),
        capabilities: state.identity.config.capabilities.clone(),
        endpoint: state.identity.endpoint,
        status,
        peer_count: state.peers.online_count(),
        queue_length: state.message_queue.read().await.len(),
        uptime_seconds: Utc::now()
            .signed_duration_since(state.identity.created_at)
            .num_seconds(),
        attention_mode,
    })
}

#[cfg(feature = "network")]
async fn handle_peers(State(state): State<Arc<ServerState>>) -> Json<PeersResponse> {
    let peers = state.peers.all_peers();
    let online = state.peers.online_count();
    let total = peers.len();

    Json(PeersResponse {
        peers,
        total,
        online,
    })
}

#[cfg(feature = "network")]
async fn handle_message(
    State(state): State<Arc<ServerState>>,
    Json(request): Json<MessageRequest>,
) -> Json<MessageResponse> {
    let message_id = request.message.id;

    match state.enqueue_message(request.message).await {
        Ok(()) => Json(MessageResponse {
            accepted: true,
            message_id,
            error: None,
        }),
        Err(e) => Json(MessageResponse {
            accepted: false,
            message_id,
            error: Some(e.to_string()),
        }),
    }
}

#[cfg(feature = "network")]
async fn handle_introspect(State(state): State<Arc<ServerState>>) -> Json<IntrospectResponse> {
    let status = state.current_status.read().await.clone();
    let attention_mode = *state.attention_mode.read().await;
    let peers = state.peers.all_peers();

    // Calculate queue stats
    let queue = state.message_queue.read().await;
    let mut by_priority = std::collections::HashMap::new();
    for msg in queue.iter() {
        let key = format!("{:?}", msg.priority);
        *by_priority.entry(key).or_insert(0) += 1;
    }

    Json(IntrospectResponse {
        identity: state.identity.clone(),
        status,
        peers,
        queue_stats: QueueStats {
            length: queue.len(),
            max_size: state.config.max_queue_size,
            by_priority,
        },
        attention_mode,
    })
}

// ============================================================================
// WebSocket Handler
// ============================================================================

#[cfg(feature = "network")]
async fn handle_ws_upgrade(
    ws: WebSocketUpgrade,
    State(state): State<Arc<ServerState>>,
) -> Response {
    ws.on_upgrade(|socket| handle_websocket(socket, state))
}

#[cfg(feature = "network")]
async fn handle_websocket(mut socket: WebSocket, state: Arc<ServerState>) {
    let mut rx = state.subscribe_events();

    // Send initial status
    let status = StatusResponse {
        id: state.identity.id,
        name: state.identity.config.name.clone(),
        version: state.identity.config.version.clone(),
        capabilities: state.identity.config.capabilities.clone(),
        endpoint: state.identity.endpoint,
        status: state.current_status.read().await.clone(),
        peer_count: state.peers.online_count(),
        queue_length: state.message_queue.read().await.len(),
        uptime_seconds: Utc::now()
            .signed_duration_since(state.identity.created_at)
            .num_seconds(),
        attention_mode: *state.attention_mode.read().await,
    };

    if let Ok(json) = serde_json::to_string(&status) {
        let _ = socket.send(WsMessage::Text(json)).await;
    }

    // Forward events to WebSocket
    loop {
        tokio::select! {
            // Receive events from broadcast channel
            result = rx.recv() => {
                match result {
                    Ok(event) => {
                        if let Ok(json) = serde_json::to_string(&event) {
                            if socket.send(WsMessage::Text(json)).await.is_err() {
                                break; // Client disconnected
                            }
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("WebSocket client lagged by {} events", n);
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        break;
                    }
                }
            }
            // Handle incoming messages from client
            msg = socket.recv() => {
                match msg {
                    Some(Ok(WsMessage::Close(_))) | None => break,
                    Some(Ok(WsMessage::Ping(data))) => {
                        let _ = socket.send(WsMessage::Pong(data)).await;
                    }
                    _ => {}
                }
            }
        }
    }
}

// ============================================================================
// Network Server
// ============================================================================

/// HTTP/WebSocket server for network coordination
#[cfg(feature = "network")]
pub struct NetworkServer {
    state: Arc<ServerState>,
}

#[cfg(feature = "network")]
impl NetworkServer {
    /// Create a new network server
    #[must_use]
    pub fn new(identity: NetworkIdentity, config: ServerConfig) -> Self {
        Self {
            state: Arc::new(ServerState::new(identity, config)),
        }
    }

    /// Get shared server state
    #[must_use]
    pub fn state(&self) -> Arc<ServerState> {
        Arc::clone(&self.state)
    }

    /// Build the router
    fn build_router(&self) -> Router {
        Router::new()
            .route("/dashflow/status", get(handle_status))
            .route("/dashflow/peers", get(handle_peers))
            .route("/dashflow/message", post(handle_message))
            .route("/dashflow/introspect", get(handle_introspect))
            .route("/dashflow/events", get(handle_ws_upgrade))
            .with_state(Arc::clone(&self.state))
    }

    /// Start the server and return the bound address
    ///
    /// The server runs in a background task. Errors from the server are logged
    /// but not returned (since this method returns immediately after binding).
    /// Use [`Self::run`] for blocking execution that propagates errors.
    pub async fn start(&self) -> std::io::Result<SocketAddr> {
        let addr: SocketAddr = format!("{}:{}", self.state.config.host, self.state.config.port)
            .parse()
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidInput, err))?;

        let listener = tokio::net::TcpListener::bind(addr).await?;
        let bound_addr = listener.local_addr()?;

        let router = self.build_router();
        let server_addr = bound_addr; // Copy for logging

        // M-192: Surface network server startup failures instead of swallowing with .ok()
        tokio::spawn(async move {
            if let Err(e) = axum::serve(listener, router).await {
                tracing::error!(
                    server_addr = %server_addr,
                    error = %e,
                    "Network server failed: {}", e
                );
            }
        });

        Ok(bound_addr)
    }

    /// Start the server and block until shutdown
    pub async fn run(&self) -> std::io::Result<()> {
        let addr: SocketAddr = format!("{}:{}", self.state.config.host, self.state.config.port)
            .parse()
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidInput, err))?;

        let listener = tokio::net::TcpListener::bind(addr).await?;
        let router = self.build_router();

        axum::serve(listener, router).await
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::types::{AppConfig, Channel};

    #[test]
    fn test_server_config_default() {
        let config = ServerConfig::default();
        assert_eq!(config.port, 0);
        assert_eq!(config.max_queue_size, 1000);
        assert_eq!(config.rate_limit_per_peer, 10);
    }

    #[test]
    fn test_server_config_builder() {
        let config = ServerConfig::default()
            .with_port(8080)
            .with_host("127.0.0.1")
            .with_max_queue_size(500);

        assert_eq!(config.port, 8080);
        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.max_queue_size, 500);
    }

    #[test]
    fn test_rate_limit_tracker() {
        let mut tracker = RateLimitTracker::new();

        // Should allow up to limit
        for _ in 0..10 {
            assert!(tracker.check_and_increment(10));
        }

        // Should reject after limit
        assert!(!tracker.check_and_increment(10));
    }

    #[tokio::test]
    async fn test_server_state_creation() {
        let identity = NetworkIdentity::new(AppConfig::new("TestServer"));
        let config = ServerConfig::default();
        let state = ServerState::new(identity, config);

        assert_eq!(state.peers.total_count(), 0);
        assert_eq!(state.queue_len().await, 0);
    }

    #[tokio::test]
    async fn test_server_state_status() {
        let identity = NetworkIdentity::new(AppConfig::new("TestServer"));
        let state = ServerState::new(identity, ServerConfig::default());

        assert!(state.current_status.read().await.is_none());

        state.set_status("building feature X", 0.5).await;

        let status = state.current_status.read().await;
        assert!(status.is_some());
        let status = status.as_ref().unwrap();
        assert_eq!(status.working_on, "building feature X");
        assert!((status.progress - 0.5).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_enqueue_message() {
        let identity = NetworkIdentity::new(AppConfig::new("TestServer"));
        let state = ServerState::new(identity, ServerConfig::default());

        let from = PeerId::new();
        let msg = Message::broadcast(
            from,
            Channel::status(),
            MessageType::Status,
            serde_json::json!({"test": true}),
        );

        let result = state.enqueue_message(msg).await;
        assert!(result.is_ok());
        assert_eq!(state.queue_len().await, 1);
    }

    #[tokio::test]
    async fn test_enqueue_rate_limit() {
        let identity = NetworkIdentity::new(AppConfig::new("TestServer"));
        let config = ServerConfig::default();
        let limit = config.rate_limit_per_peer;
        let state = ServerState::new(identity, config);

        let from = PeerId::new();

        // Send up to rate limit
        for _ in 0..limit {
            let msg = Message::broadcast(
                from,
                Channel::status(),
                MessageType::Status,
                serde_json::json!({}),
            );
            assert!(state.enqueue_message(msg).await.is_ok());
        }

        // Next should be rate limited
        let msg = Message::broadcast(
            from,
            Channel::status(),
            MessageType::Status,
            serde_json::json!({}),
        );
        assert_eq!(
            state.enqueue_message(msg).await,
            Err(EnqueueError::RateLimited)
        );
    }

    #[tokio::test]
    async fn test_attention_mode() {
        let identity = NetworkIdentity::new(AppConfig::new("TestServer"));
        let state = ServerState::new(identity, ServerConfig::default());

        // Enqueue normal and critical messages
        let from = PeerId::new();

        let normal_msg = Message::broadcast(
            from,
            Channel::status(),
            MessageType::Status,
            serde_json::json!({"priority": "normal"}),
        )
        .with_priority(Priority::Normal);

        let critical_msg = Message::broadcast(
            from,
            Channel::errors(),
            MessageType::Error,
            serde_json::json!({"priority": "critical"}),
        )
        .with_priority(Priority::Critical);

        state.enqueue_message(normal_msg).await.unwrap();
        state.enqueue_message(critical_msg).await.unwrap();

        // In Focused mode, critical should come first
        state.set_attention_mode(AttentionMode::Focused).await;
        let first = state.next_message().await.unwrap();
        assert_eq!(first.priority, Priority::Critical);
    }

    #[tokio::test]
    async fn test_queue_eviction() {
        let identity = NetworkIdentity::new(AppConfig::new("TestServer"));
        let config = ServerConfig::default().with_max_queue_size(2);
        let state = ServerState::new(identity, config);

        let from = PeerId::new();

        // Fill queue with background messages
        let bg1 = Message::broadcast(
            from,
            Channel::status(),
            MessageType::Status,
            serde_json::json!({"msg": 1}),
        )
        .with_priority(Priority::Background);

        let bg2 = Message::broadcast(
            from,
            Channel::status(),
            MessageType::Status,
            serde_json::json!({"msg": 2}),
        )
        .with_priority(Priority::Background);

        state.enqueue_message(bg1).await.unwrap();
        state.enqueue_message(bg2).await.unwrap();

        // Add critical message - should evict a background one
        let critical = Message::broadcast(
            from,
            Channel::errors(),
            MessageType::Error,
            serde_json::json!({"msg": "critical"}),
        )
        .with_priority(Priority::Critical);

        state.enqueue_message(critical).await.unwrap();

        // Queue should still be at max
        assert_eq!(state.queue_len().await, 2);

        // Critical should be in queue
        let queue = state.message_queue.read().await;
        assert!(queue.iter().any(|m| m.priority == Priority::Critical));
    }

    #[test]
    fn test_network_event_serialization() {
        let event = NetworkEvent::MessageReceived {
            id: Uuid::new_v4(),
            from: PeerId::new(),
            msg_type: MessageType::Status,
            channel: "_status".to_string(),
            priority: Priority::Normal,
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("message_received"));
        assert!(json.contains("_status"));
    }

    #[test]
    fn test_status_response_serialization() {
        let response = StatusResponse {
            id: PeerId::new(),
            name: "TestApp".to_string(),
            version: "1.0.0".to_string(),
            capabilities: vec!["code-editing".to_string()],
            endpoint: None,
            status: None,
            peer_count: 5,
            queue_length: 10,
            uptime_seconds: 3600,
            attention_mode: AttentionMode::Focused,
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("TestApp"));
        assert!(json.contains("code-editing"));
    }
}
