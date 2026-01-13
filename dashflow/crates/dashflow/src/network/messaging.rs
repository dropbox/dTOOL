// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Messaging layer for DashFlow network coordination.
//!
//! Provides channel subscription management, broadcast/direct messaging,
//! and request-response patterns for peer communication.
//!
//! ## Usage
//!
//! ```rust,ignore
//! use dashflow::network::{MessagingClient, AppConfig, Priority, Channel};
//!
//! // Create client
//! let client = MessagingClient::new(AppConfig::new("MyAgent"));
//!
//! // Subscribe to channels
//! client.subscribe(Channel::suggestions()).await?;
//! client.subscribe(Channel::new("custom:my-team")).await?;
//!
//! // Broadcast to a channel
//! client.broadcast("_status", json!({"progress": 0.5}), Priority::Background).await?;
//!
//! // Send to specific peer
//! client.send_to(peer_id, "_suggestions", json!({"tip": "..."}), Priority::Normal).await?;
//!
//! // Request/response
//! let response = client.request(peer_id, "query", json!({"q": "status?"})).await?;
//! ```

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::{oneshot, RwLock};
use uuid::Uuid;

use crate::constants::{DEFAULT_QUEUE_CAPACITY, DEFAULT_TIMEOUT_MS};
use super::types::{
    channels, AttentionMode, Channel, Message, MessageType, NetworkIdentity, PeerId, PeerRegistry,
    Priority,
};

// ============================================================================
// Channel Subscriptions
// ============================================================================

/// Manages channel subscriptions for message filtering
#[derive(Debug)]
pub struct SubscriptionManager {
    /// Subscribed channels (channel name -> subscription info)
    subscriptions: RwLock<HashMap<String, SubscriptionInfo>>,
}

/// Information about a channel subscription
#[derive(Debug, Clone)]
pub struct SubscriptionInfo {
    /// When the subscription was created
    pub subscribed_at: DateTime<Utc>,
    /// Whether to receive background messages (or only Critical/Normal)
    pub include_background: bool,
    /// Custom filter function name (for future extensibility)
    pub filter: Option<String>,
}

impl Default for SubscriptionInfo {
    fn default() -> Self {
        Self {
            subscribed_at: Utc::now(),
            include_background: true,
            filter: None,
        }
    }
}

impl SubscriptionManager {
    /// Create a new subscription manager with default subscriptions
    #[must_use]
    pub fn new() -> Self {
        let mut subs = HashMap::new();

        // Subscribe to standard channels by default (except _debug)
        for channel in [
            channels::PRESENCE,
            channels::STATUS,
            channels::SUGGESTIONS,
            channels::BUGS,
            channels::ERRORS,
            channels::LLM_USAGE,
        ] {
            subs.insert(channel.to_string(), SubscriptionInfo::default());
        }

        Self {
            subscriptions: RwLock::new(subs),
        }
    }

    /// Create with no default subscriptions
    #[must_use]
    pub fn empty() -> Self {
        Self {
            subscriptions: RwLock::new(HashMap::new()),
        }
    }

    /// Subscribe to a channel
    pub async fn subscribe(&self, channel: &Channel) {
        let mut subs = self.subscriptions.write().await;
        subs.entry(channel.name().to_string())
            .or_insert_with(SubscriptionInfo::default);
    }

    /// Subscribe with custom options
    pub async fn subscribe_with_options(&self, channel: &Channel, info: SubscriptionInfo) {
        let mut subs = self.subscriptions.write().await;
        subs.insert(channel.name().to_string(), info);
    }

    /// Unsubscribe from a channel
    pub async fn unsubscribe(&self, channel: &Channel) {
        let mut subs = self.subscriptions.write().await;
        subs.remove(channel.name());
    }

    /// Check if subscribed to a channel
    pub async fn is_subscribed(&self, channel: &Channel) -> bool {
        self.subscriptions.read().await.contains_key(channel.name())
    }

    /// Get all subscribed channel names
    pub async fn subscribed_channels(&self) -> Vec<String> {
        self.subscriptions.read().await.keys().cloned().collect()
    }

    /// Check if a message should be delivered based on subscriptions
    pub async fn should_deliver(&self, msg: &Message) -> bool {
        let subs = self.subscriptions.read().await;
        if let Some(info) = subs.get(msg.channel.name()) {
            // If not including background, filter them out
            if !info.include_background && msg.priority == Priority::Background {
                return false;
            }
            true
        } else {
            false
        }
    }
}

impl Default for SubscriptionManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Pending Requests (for request/response pattern)
// ============================================================================

/// Tracks pending request-response exchanges
#[derive(Debug)]
struct PendingRequest {
    /// When the request was sent
    sent_at: DateTime<Utc>,
    /// Channel to send response on
    response_tx: oneshot::Sender<Message>,
    /// Request timeout
    timeout: Duration,
}

/// Manager for pending request-response exchanges
#[derive(Debug, Default)]
pub struct RequestManager {
    /// Pending requests keyed by request message ID
    pending: RwLock<HashMap<Uuid, PendingRequest>>,
}

impl RequestManager {
    /// Create a new request manager
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a pending request, returns a receiver for the response
    pub async fn register(
        &self,
        request_id: Uuid,
        timeout: Duration,
    ) -> oneshot::Receiver<Message> {
        let (tx, rx) = oneshot::channel();
        let mut pending = self.pending.write().await;
        pending.insert(
            request_id,
            PendingRequest {
                sent_at: Utc::now(),
                response_tx: tx,
                timeout,
            },
        );
        rx
    }

    /// Try to deliver a response to a pending request
    /// Returns true if delivered, false if no matching request
    pub async fn deliver_response(&self, msg: Message) -> bool {
        if let Some(reply_to) = msg.reply_to {
            let mut pending = self.pending.write().await;
            if let Some(req) = pending.remove(&reply_to) {
                // Check if still within timeout
                let elapsed = Utc::now().signed_duration_since(req.sent_at);
                if elapsed < req.timeout {
                    let _ = req.response_tx.send(msg);
                    return true;
                }
            }
        }
        false
    }

    /// Clean up expired requests
    pub async fn cleanup_expired(&self) {
        let mut pending = self.pending.write().await;
        let now = Utc::now();
        pending.retain(|_, req| {
            let elapsed = now.signed_duration_since(req.sent_at);
            elapsed < req.timeout
        });
    }

    /// Get count of pending requests
    pub async fn pending_count(&self) -> usize {
        self.pending.read().await.len()
    }
}

// ============================================================================
// Message Digest
// ============================================================================

/// Accumulated digest of background messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageDigest {
    /// Period start
    pub period_start: DateTime<Utc>,
    /// Period end
    pub period_end: DateTime<Utc>,
    /// Status updates count
    pub status_updates: usize,
    /// Suggestions count
    pub suggestions: usize,
    /// Bug reports count
    pub bug_reports: usize,
    /// Errors count
    pub errors: usize,
    /// LLM usage reports count
    pub llm_usage_reports: usize,
    /// Other messages count
    pub other: usize,
    /// Sample of recent messages (limited to avoid memory bloat)
    pub recent_samples: Vec<DigestSample>,
}

/// A sample message in the digest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DigestSample {
    /// Message type
    pub msg_type: MessageType,
    /// Channel
    pub channel: String,
    /// From peer
    pub from: PeerId,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Brief summary (first 100 chars of payload)
    pub summary: String,
}

impl MessageDigest {
    /// Create a new empty digest
    #[must_use]
    pub fn new() -> Self {
        Self {
            period_start: Utc::now(),
            period_end: Utc::now(),
            status_updates: 0,
            suggestions: 0,
            bug_reports: 0,
            errors: 0,
            llm_usage_reports: 0,
            other: 0,
            recent_samples: Vec::new(),
        }
    }

    /// Add a message to the digest
    pub fn add_message(&mut self, msg: &Message) {
        self.period_end = Utc::now();

        match msg.channel.name() {
            channels::STATUS => self.status_updates += 1,
            channels::SUGGESTIONS => self.suggestions += 1,
            channels::BUGS => self.bug_reports += 1,
            channels::ERRORS => self.errors += 1,
            channels::LLM_USAGE => self.llm_usage_reports += 1,
            _ => self.other += 1,
        }

        // Keep up to 10 recent samples
        if self.recent_samples.len() < 10 {
            let summary = msg
                .payload
                .to_string()
                .chars()
                .take(100)
                .collect::<String>();
            self.recent_samples.push(DigestSample {
                msg_type: msg.msg_type,
                channel: msg.channel.name().to_string(),
                from: msg.from,
                timestamp: msg.timestamp,
                summary,
            });
        }
    }

    /// Get total message count
    #[must_use]
    pub fn total_count(&self) -> usize {
        self.status_updates
            + self.suggestions
            + self.bug_reports
            + self.errors
            + self.llm_usage_reports
            + self.other
    }

    /// Format as human-readable string
    #[must_use]
    pub fn format(&self) -> String {
        format!(
            "Since {}: {} status updates, {} suggestions, {} bug reports, {} errors",
            self.period_start.format("%H:%M:%S"),
            self.status_updates,
            self.suggestions,
            self.bug_reports,
            self.errors
        )
    }
}

impl Default for MessageDigest {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Outbound Message Queue
// ============================================================================

/// Message ready to be sent to a peer
#[derive(Debug, Clone)]
pub struct OutboundMessage {
    /// Target peer (or None for broadcast)
    pub target_peer: Option<PeerId>,
    /// Target endpoint (resolved from peer registry)
    pub target_endpoint: Option<std::net::SocketAddr>,
    /// The message
    pub message: Message,
    /// Retry count
    pub retries: u32,
    /// Max retries
    pub max_retries: u32,
}

/// Queue for outbound messages
#[derive(Debug, Default)]
pub struct OutboundQueue {
    /// Pending outbound messages
    queue: RwLock<VecDeque<OutboundMessage>>,
    /// Max queue size
    max_size: usize,
}

impl OutboundQueue {
    /// Create a new outbound queue
    #[must_use]
    pub fn new(max_size: usize) -> Self {
        Self {
            queue: RwLock::new(VecDeque::new()),
            max_size,
        }
    }

    /// Enqueue a message for sending
    pub async fn enqueue(&self, msg: OutboundMessage) -> Result<(), MessagingError> {
        let mut queue = self.queue.write().await;
        if queue.len() >= self.max_size {
            return Err(MessagingError::QueueFull);
        }
        queue.push_back(msg);
        Ok(())
    }

    /// Get next message to send
    pub async fn next(&self) -> Option<OutboundMessage> {
        self.queue.write().await.pop_front()
    }

    /// Re-enqueue a message for retry
    pub async fn retry(&self, mut msg: OutboundMessage) -> Result<(), MessagingError> {
        msg.retries += 1;
        if msg.retries > msg.max_retries {
            return Err(MessagingError::MaxRetriesExceeded);
        }
        self.enqueue(msg).await
    }

    /// Get queue length
    pub async fn len(&self) -> usize {
        self.queue.read().await.len()
    }

    /// Check if queue is empty
    pub async fn is_empty(&self) -> bool {
        self.queue.read().await.is_empty()
    }
}

// ============================================================================
// Messaging Errors
// ============================================================================

/// Errors that can occur during messaging
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum MessagingError {
    /// Outbound queue is full
    #[error("Outbound queue full")]
    QueueFull,
    /// Max retries exceeded
    #[error("Max retries exceeded")]
    MaxRetriesExceeded,
    /// Peer not found
    #[error("Peer not found: {0}")]
    PeerNotFound(PeerId),
    /// Request timed out
    #[error("Request timed out")]
    RequestTimeout,
    /// Not subscribed to channel
    #[error("Not subscribed to channel: {0}")]
    NotSubscribed(String),
    /// Network error
    #[error("Network error: {0}")]
    NetworkError(String),
    /// Serialization error
    #[error("Serialization error: {0}")]
    SerializationError(String),
}

// ============================================================================
// Messaging Client
// ============================================================================

/// High-level client for network messaging
///
/// Provides channel subscription, broadcast/direct messaging, and request-response
/// patterns for peer communication.
pub struct MessagingClient {
    /// Our identity
    identity: NetworkIdentity,
    /// Peer registry
    peers: PeerRegistry,
    /// Channel subscriptions
    subscriptions: SubscriptionManager,
    /// Pending requests
    requests: RequestManager,
    /// Outbound message queue
    outbound: OutboundQueue,
    /// Incoming message queue (filtered by subscriptions)
    inbox: RwLock<VecDeque<Message>>,
    /// Current attention mode
    attention_mode: RwLock<AttentionMode>,
    /// Message digest for background messages
    digest: RwLock<MessageDigest>,
    /// Max inbox size
    max_inbox_size: usize,
    /// Default request timeout in seconds
    default_request_timeout: i64,
}

impl MessagingClient {
    /// Create a new messaging client
    #[must_use]
    pub fn new(identity: NetworkIdentity) -> Self {
        Self {
            identity,
            peers: PeerRegistry::new(),
            subscriptions: SubscriptionManager::new(),
            requests: RequestManager::new(),
            outbound: OutboundQueue::new(DEFAULT_QUEUE_CAPACITY),
            inbox: RwLock::new(VecDeque::new()),
            attention_mode: RwLock::new(AttentionMode::Focused),
            digest: RwLock::new(MessageDigest::new()),
            max_inbox_size: DEFAULT_QUEUE_CAPACITY,
            default_request_timeout: (DEFAULT_TIMEOUT_MS / 1000) as i64,
        }
    }

    /// Get our peer ID
    #[must_use]
    pub fn peer_id(&self) -> PeerId {
        self.identity.id
    }

    /// Get our identity
    #[must_use]
    pub fn identity(&self) -> &NetworkIdentity {
        &self.identity
    }

    /// Get the peer registry
    #[must_use]
    pub fn peers(&self) -> &PeerRegistry {
        &self.peers
    }

    /// Get the subscription manager
    #[must_use]
    pub fn subscriptions(&self) -> &SubscriptionManager {
        &self.subscriptions
    }

    // -------------------------------------------------------------------------
    // Subscriptions
    // -------------------------------------------------------------------------

    /// Subscribe to a channel
    pub async fn subscribe(&self, channel: Channel) {
        self.subscriptions.subscribe(&channel).await;
    }

    /// Unsubscribe from a channel
    pub async fn unsubscribe(&self, channel: Channel) {
        self.subscriptions.unsubscribe(&channel).await;
    }

    /// Get subscribed channels
    pub async fn subscribed_channels(&self) -> Vec<String> {
        self.subscriptions.subscribed_channels().await
    }

    // -------------------------------------------------------------------------
    // Attention Mode
    // -------------------------------------------------------------------------

    /// Set attention mode
    pub async fn set_attention_mode(&self, mode: AttentionMode) {
        let mut current = self.attention_mode.write().await;
        *current = mode;
    }

    /// Get current attention mode
    pub async fn attention_mode(&self) -> AttentionMode {
        *self.attention_mode.read().await
    }

    // -------------------------------------------------------------------------
    // Sending Messages
    // -------------------------------------------------------------------------

    /// Broadcast a message to all peers on a channel
    pub async fn broadcast(
        &self,
        channel: impl Into<String>,
        payload: serde_json::Value,
        priority: Priority,
    ) -> Result<Uuid, MessagingError> {
        let channel = Channel::new(channel.into());
        let msg = Message::broadcast(self.identity.id, channel, MessageType::Extended, payload)
            .with_priority(priority);
        let id = msg.id;

        self.outbound
            .enqueue(OutboundMessage {
                target_peer: None,
                target_endpoint: None,
                message: msg,
                retries: 0,
                max_retries: 3,
            })
            .await?;

        Ok(id)
    }

    /// Send a message to a specific peer
    pub async fn send_to(
        &self,
        peer_id: PeerId,
        channel: impl Into<String>,
        payload: serde_json::Value,
        priority: Priority,
    ) -> Result<Uuid, MessagingError> {
        let peer = self
            .peers
            .get(peer_id)
            .ok_or(MessagingError::PeerNotFound(peer_id))?;

        let channel = Channel::new(channel.into());
        let msg = Message::to_peer(
            self.identity.id,
            peer_id,
            channel,
            MessageType::Extended,
            payload,
        )
        .with_priority(priority);
        let id = msg.id;

        self.outbound
            .enqueue(OutboundMessage {
                target_peer: Some(peer_id),
                target_endpoint: Some(peer.endpoint),
                message: msg,
                retries: 0,
                max_retries: 3,
            })
            .await?;

        Ok(id)
    }

    /// Send a suggestion to a peer
    pub async fn send_suggestion(
        &self,
        peer_id: PeerId,
        suggestion: impl Into<String>,
        context: Option<serde_json::Value>,
    ) -> Result<Uuid, MessagingError> {
        let mut payload = serde_json::json!({
            "suggestion": suggestion.into(),
        });
        if let Some(ctx) = context {
            payload["context"] = ctx;
        }
        self.send_to(peer_id, channels::SUGGESTIONS, payload, Priority::Normal)
            .await
    }

    /// Send a bug report to a peer
    pub async fn send_bug_report(
        &self,
        peer_id: PeerId,
        bug: impl Into<String>,
        details: Option<serde_json::Value>,
    ) -> Result<Uuid, MessagingError> {
        let mut payload = serde_json::json!({
            "bug": bug.into(),
        });
        if let Some(d) = details {
            payload["details"] = d;
        }
        self.send_to(peer_id, channels::BUGS, payload, Priority::Normal)
            .await
    }

    /// Broadcast an error
    pub async fn broadcast_error(
        &self,
        error: impl Into<String>,
        details: Option<serde_json::Value>,
    ) -> Result<Uuid, MessagingError> {
        let mut payload = serde_json::json!({
            "error": error.into(),
        });
        if let Some(d) = details {
            payload["details"] = d;
        }
        self.broadcast(channels::ERRORS, payload, Priority::Critical)
            .await
    }

    /// Broadcast status update
    pub async fn broadcast_status(
        &self,
        working_on: impl Into<String>,
        progress: f64,
    ) -> Result<Uuid, MessagingError> {
        let payload = serde_json::json!({
            "working_on": working_on.into(),
            "progress": progress.clamp(0.0, 1.0),
        });
        self.broadcast(channels::STATUS, payload, Priority::Background)
            .await
    }

    // -------------------------------------------------------------------------
    // Request/Response
    // -------------------------------------------------------------------------

    /// Send a request and wait for response
    pub async fn request(
        &self,
        peer_id: PeerId,
        topic: impl Into<String>,
        payload: serde_json::Value,
    ) -> Result<Message, MessagingError> {
        self.request_with_timeout(
            peer_id,
            topic,
            payload,
            Duration::seconds(self.default_request_timeout),
        )
        .await
    }

    /// Send a request with custom timeout
    pub async fn request_with_timeout(
        &self,
        peer_id: PeerId,
        topic: impl Into<String>,
        payload: serde_json::Value,
        timeout: Duration,
    ) -> Result<Message, MessagingError> {
        let peer = self
            .peers
            .get(peer_id)
            .ok_or(MessagingError::PeerNotFound(peer_id))?;

        let channel = Channel::new(topic.into());
        let msg = Message::to_peer(
            self.identity.id,
            peer_id,
            channel,
            MessageType::Query,
            payload,
        )
        .with_priority(Priority::Normal);
        let request_id = msg.id;

        // Register for response
        let rx = self.requests.register(request_id, timeout).await;

        // Send the request
        self.outbound
            .enqueue(OutboundMessage {
                target_peer: Some(peer_id),
                target_endpoint: Some(peer.endpoint),
                message: msg,
                retries: 0,
                max_retries: 3,
            })
            .await?;

        // Wait for response
        let timeout_std = std::time::Duration::from_secs(timeout.num_seconds() as u64);
        match tokio::time::timeout(timeout_std, rx).await {
            Ok(Ok(response)) => Ok(response),
            Ok(Err(_)) => Err(MessagingError::RequestTimeout),
            Err(_) => {
                // Clean up the pending request
                self.requests.cleanup_expired().await;
                Err(MessagingError::RequestTimeout)
            }
        }
    }

    /// Send a response to a query
    pub async fn respond(
        &self,
        to_message: &Message,
        payload: serde_json::Value,
    ) -> Result<Uuid, MessagingError> {
        let peer_id = to_message.from;
        let peer = self
            .peers
            .get(peer_id)
            .ok_or(MessagingError::PeerNotFound(peer_id))?;

        let msg = Message::to_peer(
            self.identity.id,
            peer_id,
            to_message.channel.clone(),
            MessageType::Response,
            payload,
        )
        .with_priority(Priority::Normal)
        .replying_to(to_message.id);
        let id = msg.id;

        self.outbound
            .enqueue(OutboundMessage {
                target_peer: Some(peer_id),
                target_endpoint: Some(peer.endpoint),
                message: msg,
                retries: 0,
                max_retries: 3,
            })
            .await?;

        Ok(id)
    }

    // -------------------------------------------------------------------------
    // Receiving Messages
    // -------------------------------------------------------------------------

    /// Handle an incoming message (from server or transport)
    pub async fn handle_incoming(&self, msg: Message) {
        // Check if it's a response to a pending request
        if msg.msg_type == MessageType::Response
            && self.requests.deliver_response(msg.clone()).await
        {
            return; // Response delivered to waiting request
        }

        // Check subscription filter
        if !self.subscriptions.should_deliver(&msg).await {
            return;
        }

        let mode = *self.attention_mode.read().await;

        match (mode, msg.priority) {
            // Critical always goes to inbox
            (_, Priority::Critical) => {
                self.add_to_inbox(msg).await;
            }
            // Realtime: everything to inbox
            (AttentionMode::Realtime, _) => {
                self.add_to_inbox(msg).await;
            }
            // Focused: Normal to inbox, Background to digest
            (AttentionMode::Focused, Priority::Normal) => {
                self.add_to_inbox(msg).await;
            }
            (AttentionMode::Focused, Priority::Background) => {
                self.add_to_digest(&msg).await;
            }
            // Minimal: Only Critical to inbox (handled above), rest to digest
            (AttentionMode::Minimal, _) => {
                self.add_to_digest(&msg).await;
            }
        }
    }

    /// Add message to inbox
    async fn add_to_inbox(&self, msg: Message) {
        let mut inbox = self.inbox.write().await;
        if inbox.len() >= self.max_inbox_size {
            // Evict oldest non-critical
            if let Some(idx) = inbox.iter().position(|m| m.priority != Priority::Critical) {
                inbox.remove(idx);
            } else {
                inbox.pop_front();
            }
        }
        inbox.push_back(msg);
    }

    /// Add message to digest
    async fn add_to_digest(&self, msg: &Message) {
        let mut digest = self.digest.write().await;
        digest.add_message(msg);
    }

    /// Get next message from inbox
    pub async fn next_message(&self) -> Option<Message> {
        let mode = *self.attention_mode.read().await;
        let mut inbox = self.inbox.write().await;

        match mode {
            AttentionMode::Realtime => inbox.pop_front(),
            AttentionMode::Focused => {
                // Prefer critical
                if let Some(idx) = inbox.iter().position(|m| m.priority == Priority::Critical) {
                    inbox.remove(idx)
                } else {
                    inbox.pop_front()
                }
            }
            AttentionMode::Minimal => {
                // Only critical
                if let Some(idx) = inbox.iter().position(|m| m.priority == Priority::Critical) {
                    inbox.remove(idx)
                } else {
                    None
                }
            }
        }
    }

    /// Check for messages without removing them
    pub async fn peek_messages(&self) -> Vec<Message> {
        self.inbox.read().await.iter().cloned().collect()
    }

    /// Get inbox length
    pub async fn inbox_len(&self) -> usize {
        self.inbox.read().await.len()
    }

    /// Get and reset the digest
    pub async fn take_digest(&self) -> MessageDigest {
        let mut digest = self.digest.write().await;
        std::mem::take(&mut *digest)
    }

    /// Get digest without resetting
    pub async fn peek_digest(&self) -> MessageDigest {
        self.digest.read().await.clone()
    }

    // -------------------------------------------------------------------------
    // Outbound Queue Access
    // -------------------------------------------------------------------------

    /// Get next outbound message (for transport layer)
    pub async fn next_outbound(&self) -> Option<OutboundMessage> {
        self.outbound.next().await
    }

    /// Retry a failed outbound message
    pub async fn retry_outbound(&self, msg: OutboundMessage) -> Result<(), MessagingError> {
        self.outbound.retry(msg).await
    }

    /// Get outbound queue length
    pub async fn outbound_len(&self) -> usize {
        self.outbound.len().await
    }
}

// ============================================================================
// Router - Routes messages to appropriate handlers
// ============================================================================

/// Message handler callback type
pub type MessageHandler = Box<dyn Fn(&Message) + Send + Sync>;

/// Routes incoming messages to registered handlers
pub struct MessageRouter {
    /// Handlers by channel name
    handlers: RwLock<HashMap<String, Vec<Arc<MessageHandler>>>>,
    /// Default handler for unmatched messages
    default_handler: RwLock<Option<Arc<MessageHandler>>>,
}

impl MessageRouter {
    /// Create a new router
    #[must_use]
    pub fn new() -> Self {
        Self {
            handlers: RwLock::new(HashMap::new()),
            default_handler: RwLock::new(None),
        }
    }

    /// Register a handler for a channel
    pub async fn on(&self, channel: impl Into<String>, handler: MessageHandler) {
        let mut handlers = self.handlers.write().await;
        handlers
            .entry(channel.into())
            .or_default()
            .push(Arc::new(handler));
    }

    /// Set default handler for unmatched messages
    pub async fn on_default(&self, handler: MessageHandler) {
        let mut default = self.default_handler.write().await;
        *default = Some(Arc::new(handler));
    }

    /// Route a message to handlers
    pub async fn route(&self, msg: &Message) {
        let handlers = self.handlers.read().await;
        if let Some(channel_handlers) = handlers.get(msg.channel.name()) {
            for handler in channel_handlers {
                handler(msg);
            }
        } else {
            let default = self.default_handler.read().await;
            if let Some(handler) = default.as_ref() {
                handler(msg);
            }
        }
    }
}

impl Default for MessageRouter {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for MessageRouter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MessageRouter").finish_non_exhaustive()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::types::AppConfig;

    #[tokio::test]
    async fn test_subscription_manager_defaults() {
        let mgr = SubscriptionManager::new();

        // Standard channels should be subscribed
        assert!(mgr.is_subscribed(&Channel::presence()).await);
        assert!(mgr.is_subscribed(&Channel::status()).await);
        assert!(mgr.is_subscribed(&Channel::suggestions()).await);
        assert!(mgr.is_subscribed(&Channel::errors()).await);

        // Debug should NOT be subscribed
        assert!(!mgr.is_subscribed(&Channel::debug()).await);
    }

    #[tokio::test]
    async fn test_subscription_subscribe_unsubscribe() {
        let mgr = SubscriptionManager::empty();

        let channel = Channel::new("custom:test");
        assert!(!mgr.is_subscribed(&channel).await);

        mgr.subscribe(&channel).await;
        assert!(mgr.is_subscribed(&channel).await);

        mgr.unsubscribe(&channel).await;
        assert!(!mgr.is_subscribed(&channel).await);
    }

    #[tokio::test]
    async fn test_subscription_should_deliver() {
        let mgr = SubscriptionManager::new();

        let from = PeerId::new();

        // Subscribed channel - should deliver
        let msg = Message::broadcast(
            from,
            Channel::status(),
            MessageType::Status,
            serde_json::json!({}),
        );
        assert!(mgr.should_deliver(&msg).await);

        // Unsubscribed channel - should not deliver
        let msg = Message::broadcast(
            from,
            Channel::debug(),
            MessageType::Status,
            serde_json::json!({}),
        );
        assert!(!mgr.should_deliver(&msg).await);
    }

    #[tokio::test]
    async fn test_message_digest() {
        let mut digest = MessageDigest::new();
        let from = PeerId::new();

        // Add various messages
        digest.add_message(&Message::broadcast(
            from,
            Channel::status(),
            MessageType::Status,
            serde_json::json!({"progress": 0.5}),
        ));
        digest.add_message(&Message::broadcast(
            from,
            Channel::suggestions(),
            MessageType::Suggestion,
            serde_json::json!({"tip": "use caching"}),
        ));
        digest.add_message(&Message::broadcast(
            from,
            Channel::errors(),
            MessageType::Error,
            serde_json::json!({"error": "something failed"}),
        ));

        assert_eq!(digest.status_updates, 1);
        assert_eq!(digest.suggestions, 1);
        assert_eq!(digest.errors, 1);
        assert_eq!(digest.total_count(), 3);
        assert_eq!(digest.recent_samples.len(), 3);
    }

    #[tokio::test]
    async fn test_outbound_queue() {
        let queue = OutboundQueue::new(2);
        let from = PeerId::new();

        let msg1 = OutboundMessage {
            target_peer: None,
            target_endpoint: None,
            message: Message::broadcast(
                from,
                Channel::status(),
                MessageType::Status,
                serde_json::json!({}),
            ),
            retries: 0,
            max_retries: 3,
        };

        let msg2 = OutboundMessage {
            target_peer: None,
            target_endpoint: None,
            message: Message::broadcast(
                from,
                Channel::status(),
                MessageType::Status,
                serde_json::json!({}),
            ),
            retries: 0,
            max_retries: 3,
        };

        assert!(queue.enqueue(msg1).await.is_ok());
        assert!(queue.enqueue(msg2).await.is_ok());
        assert_eq!(queue.len().await, 2);

        // Queue is full
        let msg3 = OutboundMessage {
            target_peer: None,
            target_endpoint: None,
            message: Message::broadcast(
                from,
                Channel::status(),
                MessageType::Status,
                serde_json::json!({}),
            ),
            retries: 0,
            max_retries: 3,
        };
        assert_eq!(queue.enqueue(msg3).await, Err(MessagingError::QueueFull));
    }

    #[tokio::test]
    async fn test_messaging_client_broadcast() {
        let identity = NetworkIdentity::new(AppConfig::new("TestClient"));
        let client = MessagingClient::new(identity);

        let result = client
            .broadcast(
                "_status",
                serde_json::json!({"test": true}),
                Priority::Normal,
            )
            .await;
        assert!(result.is_ok());

        // Should be in outbound queue
        assert_eq!(client.outbound_len().await, 1);

        let outbound = client.next_outbound().await.unwrap();
        assert!(outbound.target_peer.is_none()); // Broadcast
        assert_eq!(outbound.message.priority, Priority::Normal);
    }

    #[tokio::test]
    async fn test_messaging_client_send_to_unknown_peer() {
        let identity = NetworkIdentity::new(AppConfig::new("TestClient"));
        let client = MessagingClient::new(identity);

        let unknown_peer = PeerId::new();
        let result = client
            .send_to(
                unknown_peer,
                "_suggestions",
                serde_json::json!({}),
                Priority::Normal,
            )
            .await;

        assert_eq!(result, Err(MessagingError::PeerNotFound(unknown_peer)));
    }

    #[tokio::test]
    async fn test_messaging_client_handle_incoming() {
        let identity = NetworkIdentity::new(AppConfig::new("TestClient"));
        let client = MessagingClient::new(identity);

        let from = PeerId::new();

        // Critical message should go to inbox
        let critical_msg = Message::broadcast(
            from,
            Channel::errors(),
            MessageType::Error,
            serde_json::json!({"error": "test"}),
        )
        .with_priority(Priority::Critical);

        client.handle_incoming(critical_msg).await;
        assert_eq!(client.inbox_len().await, 1);

        // Background message in Focused mode goes to digest
        let bg_msg = Message::broadcast(
            from,
            Channel::status(),
            MessageType::Status,
            serde_json::json!({"progress": 0.5}),
        )
        .with_priority(Priority::Background);

        client.handle_incoming(bg_msg).await;
        assert_eq!(client.inbox_len().await, 1); // Still 1
        let digest = client.peek_digest().await;
        assert_eq!(digest.status_updates, 1);
    }

    #[tokio::test]
    async fn test_messaging_client_attention_mode() {
        let identity = NetworkIdentity::new(AppConfig::new("TestClient"));
        let client = MessagingClient::new(identity);

        // Default is Focused
        assert_eq!(client.attention_mode().await, AttentionMode::Focused);

        client.set_attention_mode(AttentionMode::Realtime).await;
        assert_eq!(client.attention_mode().await, AttentionMode::Realtime);

        // In Realtime, background messages go to inbox
        let from = PeerId::new();
        let bg_msg = Message::broadcast(
            from,
            Channel::status(),
            MessageType::Status,
            serde_json::json!({}),
        )
        .with_priority(Priority::Background);

        client.handle_incoming(bg_msg).await;
        assert_eq!(client.inbox_len().await, 1);
    }

    #[tokio::test]
    async fn test_request_manager() {
        let mgr = RequestManager::new();
        let request_id = Uuid::new_v4();
        let timeout = Duration::seconds(30);

        let rx = mgr.register(request_id, timeout).await;
        assert_eq!(mgr.pending_count().await, 1);

        // Create response
        let from = PeerId::new();
        let response = Message::to_peer(
            from,
            PeerId::new(),
            Channel::new("test"),
            MessageType::Response,
            serde_json::json!({"result": "ok"}),
        )
        .replying_to(request_id);

        // Deliver response
        assert!(mgr.deliver_response(response).await);
        assert_eq!(mgr.pending_count().await, 0);

        // Should receive the response
        let received = rx.await.unwrap();
        assert_eq!(received.msg_type, MessageType::Response);
    }

    #[tokio::test]
    async fn test_message_router() {
        let router = MessageRouter::new();

        use std::sync::atomic::{AtomicUsize, Ordering};
        let call_count = Arc::new(AtomicUsize::new(0));
        let count_clone = Arc::clone(&call_count);

        router
            .on(
                "_status",
                Box::new(move |_msg| {
                    count_clone.fetch_add(1, Ordering::SeqCst);
                }),
            )
            .await;

        let from = PeerId::new();
        let msg = Message::broadcast(
            from,
            Channel::status(),
            MessageType::Status,
            serde_json::json!({}),
        );

        router.route(&msg).await;
        assert_eq!(call_count.load(Ordering::SeqCst), 1);

        router.route(&msg).await;
        assert_eq!(call_count.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn test_messaging_error_display() {
        assert_eq!(MessagingError::QueueFull.to_string(), "Outbound queue full");
        assert_eq!(
            MessagingError::RequestTimeout.to_string(),
            "Request timed out"
        );

        let peer_id = PeerId::new();
        let err = MessagingError::PeerNotFound(peer_id);
        assert!(err.to_string().contains("Peer not found"));
    }
}
