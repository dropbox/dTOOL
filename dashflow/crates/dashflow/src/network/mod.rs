// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! @dashflow-module
//! @name network
//! @category runtime
//! @status stable
//!
//! # DashFlow Network Coordination
//!
//! Automatic peer discovery and messaging for DashFlow applications.
//!
//! ## Overview
//!
//! DashFlow apps automatically discover each other on the local network and coordinate
//! through messaging. Apps share status, suggestions, and coordination signals - but
//! NOT write access. Each app maintains its own locks for self-editing.
//!
//! ## What IS Shared
//! - **Identity**: App name, version, capabilities
//! - **Status**: What the app is working on, progress
//! - **Suggestions**: "You might want to look at X"
//! - **Bug reports**: "I noticed issue Y in your output"
//! - **Coordination signals**: Intents, queries, acknowledgments
//!
//! ## What is NOT Shared
//! - **Write access**: Apps cannot modify each other's code/state
//! - **Lock ownership**: Locks are private, single-instance only
//! - **Execution control**: Apps cannot start/stop each other
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use dashflow::network::{DashflowNetwork, AppConfig, Priority};
//!
//! // Join the local network
//! let network = DashflowNetwork::join(AppConfig {
//!     name: "MyAgent".to_string(),
//!     capabilities: vec!["code-editing".to_string()],
//!     ..Default::default()
//! }).await?;
//!
//! // Send a status update
//! network.broadcast("_status", serde_json::json!({
//!     "working_on": "feature-x",
//!     "progress": 0.5,
//! }), Priority::Background).await?;
//!
//! // Check for messages
//! while let Some(msg) = network.next_message()? {
//!     handle_message(msg);
//! }
//! ```
//!
//! ## HTTP Server (requires `network` feature)
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
//!
//! ## Architecture
//!
//! ```text
//! Layer 1: mDNS/DNS-SD (discovery) - Zero config, firewall-friendly
//! Layer 2: HTTP (messaging) - Standard web traffic
//! Layer 3: WebSocket (events) - Real-time, bidirectional
//! ```
//!
//! ## Reference
//!
//! See `DESIGN_NETWORK_COORDINATION.md` for full design documentation.

#[cfg(feature = "network")]
mod coordinator;
#[cfg(feature = "network")]
mod discovery;
mod messaging;
#[cfg(feature = "network")]
mod resources;
#[cfg(feature = "network")]
mod server;
#[cfg(feature = "network")]
mod tools;
mod types;

pub use types::{
    // Standard channel names
    channels,
    // Core identity
    AppConfig,
    AttentionMode,
    // Channels and messaging
    Channel,
    CompactMessage,
    // LLM usage
    LlmEndpointStatus,
    LlmUsageReport,
    // Messages
    Message,
    MessageTarget,
    MessageType,
    NetworkIdentity,
    PeerId,
    PeerInfo,
    // Peer registry
    PeerRegistry,
    PeerStatus,
    Priority,
};

pub use messaging::{
    DigestSample,
    // Message digest
    MessageDigest,
    MessageHandler,
    // Router
    MessageRouter,
    // Messaging client
    MessagingClient,
    // Errors
    MessagingError,
    // Outbound queue
    OutboundMessage,
    OutboundQueue,
    // Request/response
    RequestManager,
    SubscriptionInfo,
    // Subscription management
    SubscriptionManager,
};

#[cfg(feature = "network")]
pub use server::{
    // Errors
    EnqueueError,
    IntrospectResponse,
    MessageRequest,
    MessageResponse,
    // Events
    NetworkEvent,
    // Server types
    NetworkServer,
    PeersResponse,
    QueueStats,
    ServerConfig,
    ServerState,
    StatusInfo,
    // Request/response types
    StatusResponse,
};

#[cfg(feature = "network")]
pub use discovery::{
    // Helpers
    get_local_ip,
    // Errors
    DiscoveryError,
    // Events
    DiscoveryEvent,
    // Discovery types
    DiscoveryManager,
    MockDiscovery,
    DEFAULT_TTL,
    DISCOVERY_TIMEOUT,
    HEARTBEAT_INTERVAL,
    // Constants
    SERVICE_TYPE,
};

#[cfg(feature = "network")]
pub use coordinator::{
    // Main coordinator
    DashflowNetwork,
    // Configuration
    NetworkConfig,
    // Errors
    NetworkError,
};

#[cfg(feature = "network")]
pub use tools::{
    // Tool schemas
    schemas as tool_schemas,
    DigestResponse,
    InboxResponse,
    MessageSummary,
    NetworkPeersResponse,
    // Response types
    NetworkStatusResponse,
    // Tool executor
    NetworkToolExecutor,
    PeerSummary,
    // Errors
    ToolError,
};

#[cfg(feature = "network")]
pub use resources::{
    // Advertisement
    AdvertisedResource,
    AllowResult,
    ColonyResource,
    // Registry
    ColonyResourceRegistry,
    // Strategy
    FailoverStrategy,
    GpuResourceInfo,
    LlmResourceInfo,
    ResourceGrant,
    ResourceLimits,
    // Messages
    ResourceMessage,
    // Request/response
    ResourceRequest,
    ResourceResponse,
    // Resource types
    ResourceType,
    ServiceResourceInfo,
    // Sharing policy
    SharingPolicy,
    StorageResourceInfo,
    VectorDbResourceInfo,
    RESOURCES_BROADCAST_INTERVAL,
    // Channel
    RESOURCES_CHANNEL,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_exports() {
        // Verify all expected types are accessible
        let config = AppConfig::default();
        assert!(!config.name.is_empty());

        let _id = PeerId::new();
        let _priority = Priority::Normal;
        let _mode = AttentionMode::Focused;
    }

    #[test]
    fn test_standard_channels() {
        assert_eq!(channels::PRESENCE, "_presence");
        assert_eq!(channels::STATUS, "_status");
        assert_eq!(channels::SUGGESTIONS, "_suggestions");
        assert_eq!(channels::BUGS, "_bugs");
        assert_eq!(channels::ERRORS, "_errors");
        assert_eq!(channels::LLM_USAGE, "_llm_usage");
        assert_eq!(channels::DEBUG, "_debug");
        assert_eq!(channels::WORKERS, "_workers");
        assert_eq!(channels::WORKER_JOIN, "_worker_join");
        assert_eq!(channels::WORKER_RESULT, "_worker_result");
    }
}
