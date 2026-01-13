// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! MCP-compatible tools for network coordination.
//!
//! These tools can be exposed via MCP to allow AI agents to interact with
//! the DashFlow network coordination system.
//!
//! ## Available Tools
//!
//! - `network_status` - Get current network status and identity
//! - `network_peers` - List known peers with capabilities
//! - `network_send_message` - Send a message to a peer
//! - `network_broadcast` - Broadcast a message to all peers
//! - `network_inbox` - Check for incoming messages
//! - `network_digest` - Get digest of background messages
//! - `network_subscribe` - Subscribe to a channel
//! - `network_unsubscribe` - Unsubscribe from a channel

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;

use super::coordinator::DashflowNetwork;
use super::types::{AttentionMode, Message, PeerId, PeerInfo, Priority};

// ============================================================================
// Tool Definitions
// ============================================================================

/// Tool schemas for MCP registration
pub mod schemas {
    use serde_json::Value;

    /// Network status tool schema
    pub const NETWORK_STATUS: &str = r#"{
        "name": "network_status",
        "description": "Get the current network status including identity, endpoint, and peer count",
        "inputSchema": {
            "type": "object",
            "properties": {},
            "required": []
        }
    }"#;

    /// Network peers tool schema
    pub const NETWORK_PEERS: &str = r#"{
        "name": "network_peers",
        "description": "List all known peers on the network with their capabilities and status",
        "inputSchema": {
            "type": "object",
            "properties": {
                "capability": {
                    "type": "string",
                    "description": "Filter peers by capability (optional)"
                },
                "online_only": {
                    "type": "boolean",
                    "description": "Only show online peers (default: true)"
                }
            },
            "required": []
        }
    }"#;

    /// Network send message tool schema
    pub const NETWORK_SEND_MESSAGE: &str = r#"{
        "name": "network_send_message",
        "description": "Send a message to a specific peer",
        "inputSchema": {
            "type": "object",
            "properties": {
                "peer_id": {
                    "type": "string",
                    "description": "Target peer ID (UUID format)"
                },
                "channel": {
                    "type": "string",
                    "description": "Channel name (e.g., '_suggestions', '_bugs', custom channel)"
                },
                "payload": {
                    "type": "object",
                    "description": "Message payload (JSON object)"
                },
                "priority": {
                    "type": "string",
                    "enum": ["background", "normal", "critical"],
                    "description": "Message priority (default: normal)"
                }
            },
            "required": ["peer_id", "channel", "payload"]
        }
    }"#;

    /// Network broadcast tool schema
    pub const NETWORK_BROADCAST: &str = r#"{
        "name": "network_broadcast",
        "description": "Broadcast a message to all peers on a channel",
        "inputSchema": {
            "type": "object",
            "properties": {
                "channel": {
                    "type": "string",
                    "description": "Channel name (e.g., '_status', '_errors')"
                },
                "payload": {
                    "type": "object",
                    "description": "Message payload (JSON object)"
                },
                "priority": {
                    "type": "string",
                    "enum": ["background", "normal", "critical"],
                    "description": "Message priority (default: normal)"
                }
            },
            "required": ["channel", "payload"]
        }
    }"#;

    /// Network inbox tool schema
    pub const NETWORK_INBOX: &str = r#"{
        "name": "network_inbox",
        "description": "Check for incoming messages in the inbox",
        "inputSchema": {
            "type": "object",
            "properties": {
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of messages to return (default: 10)"
                }
            },
            "required": []
        }
    }"#;

    /// Network digest tool schema
    pub const NETWORK_DIGEST: &str = r#"{
        "name": "network_digest",
        "description": "Get a summary digest of background messages since last check",
        "inputSchema": {
            "type": "object",
            "properties": {
                "clear": {
                    "type": "boolean",
                    "description": "Clear digest after reading (default: true)"
                }
            },
            "required": []
        }
    }"#;

    /// Network subscribe tool schema
    pub const NETWORK_SUBSCRIBE: &str = r#"{
        "name": "network_subscribe",
        "description": "Subscribe to a channel to receive messages",
        "inputSchema": {
            "type": "object",
            "properties": {
                "channel": {
                    "type": "string",
                    "description": "Channel name to subscribe to"
                }
            },
            "required": ["channel"]
        }
    }"#;

    /// Network unsubscribe tool schema
    pub const NETWORK_UNSUBSCRIBE: &str = r#"{
        "name": "network_unsubscribe",
        "description": "Unsubscribe from a channel",
        "inputSchema": {
            "type": "object",
            "properties": {
                "channel": {
                    "type": "string",
                    "description": "Channel name to unsubscribe from"
                }
            },
            "required": ["channel"]
        }
    }"#;

    /// Network set attention tool schema
    pub const NETWORK_SET_ATTENTION: &str = r#"{
        "name": "network_set_attention",
        "description": "Set the attention mode for incoming messages",
        "inputSchema": {
            "type": "object",
            "properties": {
                "mode": {
                    "type": "string",
                    "enum": ["realtime", "focused", "minimal"],
                    "description": "Attention mode: realtime (all immediate), focused (default - critical immediate, normal queued, background digest), minimal (only critical immediate)"
                }
            },
            "required": ["mode"]
        }
    }"#;

    /// Get all tool schemas as JSON array
    pub fn all_schemas() -> Vec<Value> {
        [
            NETWORK_STATUS,
            NETWORK_PEERS,
            NETWORK_SEND_MESSAGE,
            NETWORK_BROADCAST,
            NETWORK_INBOX,
            NETWORK_DIGEST,
            NETWORK_SUBSCRIBE,
            NETWORK_UNSUBSCRIBE,
            NETWORK_SET_ATTENTION,
        ]
        .iter()
        .filter_map(|schema| match serde_json::from_str::<Value>(schema) {
            Ok(value) => Some(value),
            Err(err) => {
                tracing::warn!(error = %err, "Invalid network tool schema JSON");
                None
            }
        })
        .collect()
    }
}

// ============================================================================
// Tool Responses
// ============================================================================

/// Response for network_status tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkStatusResponse {
    /// This peer's unique identifier
    pub peer_id: String,
    /// Display name of this peer
    pub name: String,
    /// Version of the DashFlow app
    pub version: String,
    /// Capabilities this peer advertises
    pub capabilities: Vec<String>,
    /// HTTP endpoint for this peer (if serving)
    pub endpoint: Option<String>,
    /// Number of known peers in the network
    pub peer_count: usize,
    /// Current attention mode (focused, monitoring, etc.)
    pub attention_mode: String,
}

/// Response for network_peers tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkPeersResponse {
    /// List of peer summaries
    pub peers: Vec<PeerSummary>,
    /// Total number of peers
    pub total: usize,
}

/// Summary of a peer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerSummary {
    /// Peer's unique identifier
    pub id: String,
    /// Display name of the peer
    pub name: String,
    /// Version of the peer's DashFlow app
    pub version: String,
    /// Capabilities the peer advertises
    pub capabilities: Vec<String>,
    /// HTTP endpoint URL for the peer
    pub endpoint: String,
    /// Current status (online, busy, offline, etc.)
    pub status: String,
    /// Last seen timestamp (RFC 3339 format)
    pub last_seen: String,
}

impl From<PeerInfo> for PeerSummary {
    fn from(peer: PeerInfo) -> Self {
        Self {
            id: peer.id.as_uuid().to_string(),
            name: peer.name,
            version: peer.version,
            capabilities: peer.capabilities,
            endpoint: peer.endpoint.to_string(),
            status: format!("{:?}", peer.status),
            last_seen: peer.last_seen.to_rfc3339(),
        }
    }
}

/// Response for network_inbox tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboxResponse {
    /// List of message summaries
    pub messages: Vec<MessageSummary>,
    /// Total number of messages
    pub count: usize,
}

/// Summary of a message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageSummary {
    /// Unique message ID
    pub id: String,
    /// Sender peer ID
    pub from: String,
    /// Channel the message was sent on
    pub channel: String,
    /// Type of message (status, suggestion, etc.)
    pub msg_type: String,
    /// Priority level of the message
    pub priority: String,
    /// Timestamp of the message (RFC 3339 format)
    pub timestamp: String,
    /// Message payload as JSON
    pub payload: Value,
}

impl From<Message> for MessageSummary {
    fn from(msg: Message) -> Self {
        Self {
            id: msg.id.to_string(),
            from: msg.from.as_uuid().to_string(),
            channel: msg.channel.name().to_string(),
            msg_type: format!("{:?}", msg.msg_type),
            priority: format!("{:?}", msg.priority),
            timestamp: msg.timestamp.to_rfc3339(),
            payload: msg.payload,
        }
    }
}

/// Response for network_digest tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DigestResponse {
    /// Human-readable summary of recent activity
    pub summary: String,
    /// Number of status messages in the digest
    pub status_count: usize,
    /// Number of suggestion messages
    pub suggestion_count: usize,
    /// Number of bug report messages
    pub bug_count: usize,
    /// Number of error messages
    pub error_count: usize,
    /// Total number of messages in the digest
    pub total_count: usize,
    /// Sample messages from the digest
    pub samples: Vec<String>,
}

// ============================================================================
// Tool Executor
// ============================================================================

/// Executor for network coordination tools
pub struct NetworkToolExecutor {
    network: Arc<DashflowNetwork>,
}

impl NetworkToolExecutor {
    /// Create a new tool executor
    #[must_use]
    pub fn new(network: Arc<DashflowNetwork>) -> Self {
        Self { network }
    }

    /// Execute a tool by name
    pub async fn execute(&self, tool_name: &str, params: Value) -> Result<Value, ToolError> {
        match tool_name {
            "network_status" => self.network_status().await,
            "network_peers" => self.network_peers(params).await,
            "network_send_message" => self.network_send_message(params).await,
            "network_broadcast" => self.network_broadcast(params).await,
            "network_inbox" => self.network_inbox(params).await,
            "network_digest" => self.network_digest(params).await,
            "network_subscribe" => self.network_subscribe(params).await,
            "network_unsubscribe" => self.network_unsubscribe(params).await,
            "network_set_attention" => self.network_set_attention(params).await,
            _ => Err(ToolError::UnknownTool(tool_name.to_string())),
        }
    }

    async fn network_status(&self) -> Result<Value, ToolError> {
        let identity = self.network.identity();
        let attention_mode = self.network.attention_mode().await;
        let response = NetworkStatusResponse {
            peer_id: identity.id.as_uuid().to_string(),
            name: identity.config.name.clone(),
            version: identity.config.version.clone(),
            capabilities: identity.config.capabilities.clone(),
            endpoint: self.network.endpoint().map(|e| e.to_string()),
            peer_count: self.network.peer_count(),
            attention_mode: format!("{:?}", attention_mode),
        };
        Ok(serde_json::to_value(response)?)
    }

    async fn network_peers(&self, params: Value) -> Result<Value, ToolError> {
        let capability = params.get("capability").and_then(|v| v.as_str());
        let online_only = params
            .get("online_only")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let peers: Vec<PeerInfo> = if let Some(cap) = capability {
            self.network.peers_with_capability(cap)
        } else if online_only {
            self.network.peers()
        } else {
            self.network.registry().all_peers()
        };

        let summaries: Vec<PeerSummary> = peers.into_iter().map(Into::into).collect();
        let total = summaries.len();

        let response = NetworkPeersResponse {
            peers: summaries,
            total,
        };
        Ok(serde_json::to_value(response)?)
    }

    async fn network_send_message(&self, params: Value) -> Result<Value, ToolError> {
        let peer_id_str = params
            .get("peer_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::MissingParameter("peer_id".to_string()))?;

        let peer_id = uuid::Uuid::parse_str(peer_id_str)
            .map(PeerId::from_uuid)
            .map_err(|e| {
                ToolError::InvalidParameter("peer_id".to_string(), format!("invalid UUID: {e}"))
            })?;

        let channel = params
            .get("channel")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::MissingParameter("channel".to_string()))?;

        let payload = params
            .get("payload")
            .cloned()
            .unwrap_or(Value::Object(serde_json::Map::new()));

        let priority = parse_priority(params.get("priority").and_then(|v| v.as_str()));

        let msg_id = self
            .network
            .send_to(peer_id, channel, payload, priority)
            .await
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

        Ok(serde_json::json!({
            "success": true,
            "message_id": msg_id.to_string()
        }))
    }

    async fn network_broadcast(&self, params: Value) -> Result<Value, ToolError> {
        let channel = params
            .get("channel")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::MissingParameter("channel".to_string()))?;

        let payload = params
            .get("payload")
            .cloned()
            .unwrap_or(Value::Object(serde_json::Map::new()));

        let priority = parse_priority(params.get("priority").and_then(|v| v.as_str()));

        let msg_id = self
            .network
            .broadcast(channel, payload, priority)
            .await
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

        Ok(serde_json::json!({
            "success": true,
            "message_id": msg_id.to_string()
        }))
    }

    async fn network_inbox(&self, params: Value) -> Result<Value, ToolError> {
        let limit = params.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;

        // Peek at messages without removing them
        let messages = self.network.peek_messages().await;
        let summaries: Vec<MessageSummary> =
            messages.into_iter().take(limit).map(Into::into).collect();
        let count = summaries.len();

        let response = InboxResponse {
            messages: summaries,
            count,
        };
        Ok(serde_json::to_value(response)?)
    }

    async fn network_digest(&self, params: Value) -> Result<Value, ToolError> {
        let clear = params
            .get("clear")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let digest = if clear {
            self.network.take_digest().await
        } else {
            self.network.digest().await
        };

        // Build summary string
        let total = digest.total_count();
        let summary = format!(
            "Since last check: {} status updates, {} suggestions, {} bugs, {} errors ({} total)",
            digest.status_updates, digest.suggestions, digest.bug_reports, digest.errors, total
        );

        let response = DigestResponse {
            summary,
            status_count: digest.status_updates,
            suggestion_count: digest.suggestions,
            bug_count: digest.bug_reports,
            error_count: digest.errors,
            total_count: total,
            samples: digest
                .recent_samples
                .iter()
                .map(|s| s.summary.clone())
                .collect(),
        };

        Ok(serde_json::to_value(response)?)
    }

    async fn network_subscribe(&self, params: Value) -> Result<Value, ToolError> {
        let channel = params
            .get("channel")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::MissingParameter("channel".to_string()))?;

        self.network.subscribe(channel).await;

        Ok(serde_json::json!({
            "success": true,
            "channel": channel,
            "subscribed": true
        }))
    }

    async fn network_unsubscribe(&self, params: Value) -> Result<Value, ToolError> {
        let channel = params
            .get("channel")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::MissingParameter("channel".to_string()))?;

        self.network.unsubscribe(channel).await;

        Ok(serde_json::json!({
            "success": true,
            "channel": channel,
            "subscribed": false
        }))
    }

    async fn network_set_attention(&self, params: Value) -> Result<Value, ToolError> {
        let mode_str = params
            .get("mode")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::MissingParameter("mode".to_string()))?;

        let mode = match mode_str.to_lowercase().as_str() {
            "realtime" => AttentionMode::Realtime,
            "focused" => AttentionMode::Focused,
            "minimal" => AttentionMode::Minimal,
            _ => {
                return Err(ToolError::InvalidParameter(
                    "mode".to_string(),
                    "must be realtime, focused, or minimal".to_string(),
                ))
            }
        };

        self.network.set_attention_mode(mode).await;

        Ok(serde_json::json!({
            "success": true,
            "mode": mode_str
        }))
    }
}

// ============================================================================
// Errors
// ============================================================================

/// Errors from tool execution
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ToolError {
    /// The requested tool does not exist
    #[error("Unknown tool: {0}")]
    UnknownTool(String),

    /// A required parameter was not provided
    #[error("Missing required parameter: {0}")]
    MissingParameter(String),

    /// A parameter had an invalid value
    #[error("Invalid parameter {0}: {1}")]
    InvalidParameter(String, String),

    /// Tool execution failed
    #[error("Execution failed: {0}")]
    ExecutionFailed(String),

    /// JSON serialization/deserialization error
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

// ============================================================================
// Helpers
// ============================================================================

fn parse_priority(s: Option<&str>) -> Priority {
    match s {
        Some("background") => Priority::Background,
        Some("critical") => Priority::Critical,
        _ => Priority::Normal,
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::AppConfig;

    #[test]
    fn test_all_schemas_valid_json() {
        let schemas = schemas::all_schemas();
        assert_eq!(schemas.len(), 9);

        for schema in &schemas {
            assert!(schema.get("name").is_some());
            assert!(schema.get("description").is_some());
            assert!(schema.get("inputSchema").is_some());
        }
    }

    #[test]
    fn test_peer_summary_from_peer_info() {
        let peer = PeerInfo::new(
            PeerId::new(),
            "TestPeer",
            "192.168.1.100:8080".parse().unwrap(),
        );

        let summary: PeerSummary = peer.into();
        assert_eq!(summary.name, "TestPeer");
        assert_eq!(summary.endpoint, "192.168.1.100:8080");
    }

    #[tokio::test]
    async fn test_network_status_tool() {
        let network = Arc::new(DashflowNetwork::mock(AppConfig::new("TestApp")));
        let executor = NetworkToolExecutor::new(network);

        let result = executor
            .execute("network_status", serde_json::json!({}))
            .await;
        assert!(result.is_ok());

        let response = result.unwrap();
        assert_eq!(response.get("name").unwrap(), "TestApp");
    }

    #[tokio::test]
    async fn test_network_peers_tool() {
        let network = Arc::new(DashflowNetwork::mock(AppConfig::new("TestApp")));

        // Add a mock peer
        let peer = PeerInfo::new(
            PeerId::new(),
            "MockPeer",
            "192.168.1.100:8080".parse().unwrap(),
        );
        network.mock_peer_discovered(peer);

        let executor = NetworkToolExecutor::new(network);

        let result = executor
            .execute("network_peers", serde_json::json!({}))
            .await;
        assert!(result.is_ok());

        let response = result.unwrap();
        assert_eq!(response.get("total").unwrap(), 1);
    }

    #[tokio::test]
    async fn test_subscribe_unsubscribe_tools() {
        let network = Arc::new(DashflowNetwork::mock(AppConfig::new("TestApp")));
        let executor = NetworkToolExecutor::new(network.clone());

        // Subscribe
        let result = executor
            .execute(
                "network_subscribe",
                serde_json::json!({"channel": "custom:test"}),
            )
            .await;
        assert!(result.is_ok());
        assert!(network.is_subscribed("custom:test").await);

        // Unsubscribe
        let result = executor
            .execute(
                "network_unsubscribe",
                serde_json::json!({"channel": "custom:test"}),
            )
            .await;
        assert!(result.is_ok());
        assert!(!network.is_subscribed("custom:test").await);
    }

    #[tokio::test]
    async fn test_set_attention_tool() {
        let network = Arc::new(DashflowNetwork::mock(AppConfig::new("TestApp")));
        let executor = NetworkToolExecutor::new(network.clone());

        let result = executor
            .execute(
                "network_set_attention",
                serde_json::json!({"mode": "minimal"}),
            )
            .await;
        assert!(result.is_ok());
        assert_eq!(network.attention_mode().await, AttentionMode::Minimal);
    }

    #[tokio::test]
    async fn test_unknown_tool() {
        let network = Arc::new(DashflowNetwork::mock(AppConfig::new("TestApp")));
        let executor = NetworkToolExecutor::new(network);

        let result = executor
            .execute("unknown_tool", serde_json::json!({}))
            .await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ToolError::UnknownTool(_)));
    }
}
