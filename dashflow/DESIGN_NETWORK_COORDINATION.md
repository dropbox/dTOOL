# DashFlow Network Coordination

**Version:** 1.0
**Date:** 2025-12-09
**Priority:** P1 - Multi-Agent Infrastructure
**Status:** DESIGN
**Prerequisite:** DESIGN_PARALLEL_AI.md (N=333-335)

---

## Executive Summary

DashFlow apps automatically discover each other on the local network and coordinate
through messaging. Apps share status, suggestions, and coordination signals - but
NOT write access. Each app maintains its own locks for self-editing.

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                      DashFlow Network                                │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│   ┌──────────────┐     mDNS Discovery      ┌──────────────┐        │
│   │   Agent A    │◄───────────────────────►│   Agent B    │        │
│   │              │     (port 5353)          │              │        │
│   │  HTTP :8401  │◄──── Messages ─────────►│  HTTP :8523  │        │
│   │  WS /events  │     (REST + WebSocket)   │  WS /events  │        │
│   └──────┬───────┘                          └──────┬───────┘        │
│          │                                         │                │
│          │           ┌──────────────┐              │                │
│          └──────────►│   Agent C    │◄─────────────┘                │
│                      │  HTTP :8847  │                               │
│                      └──────────────┘                               │
│                                                                      │
│   Layer 1: mDNS/DNS-SD (discovery) - Zero config, firewall-friendly │
│   Layer 2: HTTP (messaging) - Standard web traffic                  │
│   Layer 3: WebSocket (events) - Real-time, bidirectional            │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

---

## Design Principles

### What IS Shared
- **Identity**: App name, version, capabilities
- **Status**: What the app is working on, progress
- **Suggestions**: "You might want to look at X"
- **Bug reports**: "I noticed issue Y in your output"
- **Coordination signals**: Intents, queries, acknowledgments

### What is NOT Shared
- **Write access**: Apps cannot modify each other's code/state
- **Lock ownership**: Locks are private, single-instance only
- **Execution control**: Apps cannot start/stop each other (see Organic Spawning)

---

## Discovery

### Local Network (Default)

Uses mDNS/DNS-SD for zero-config discovery:

```rust
// Each app advertises itself
// Service: _dashflow._tcp.local
// TXT records: name, version, capabilities, endpoint

let network = DashflowNetwork::join(AppConfig {
    name: "CodeAgent",
    capabilities: vec!["code-editing", "testing"],
}).await?;

// Automatic discovery of peers
let peers = network.peers().await;
```

### Internet (Opt-in)

Requires explicit configuration and authentication:

```rust
let network = DashflowNetwork::join_internet(InternetConfig {
    hub_url: "https://dashflow-hub.example.com",
    auth_token: env::var("DASHFLOW_HUB_TOKEN")?,
    // TLS required, peer certificates validated
}).await?;
```

---

## Channels

### Standard Channels (Predefined)

| Channel | Purpose | Priority | Default |
|---------|---------|----------|---------|
| `_presence` | Join/leave announcements | Normal | Subscribed |
| `_status` | Status updates ("working on X") | Background | Subscribed |
| `_suggestions` | Suggestions from other apps | Normal | Subscribed |
| `_bugs` | Bug reports from other apps | Normal | Subscribed |
| `_errors` | Error broadcasts | Critical | Subscribed |
| `_llm_usage` | LLM rate limits and usage stats | Background | Subscribed |
| `_debug` | Debug/verbose logging | Background | NOT subscribed |

### LLM Usage Channel (`_llm_usage`)

Colony apps share LLM rate limit status to coordinate access across accounts/regions:

```rust
// Automatic broadcast every 30 seconds (or on significant change)
network.broadcast("_llm_usage", json!({
    "endpoints": [
        {
            "provider": "aws_bedrock",
            "account": "prod-1",
            "region": "us-east-1",
            "model": "claude-3-5-sonnet",
            "rate_limit_remaining": 850,
            "tokens_remaining": 45000,
            "saturated": false,
            "latency_ms": 420,
        },
        {
            "provider": "aws_bedrock",
            "account": "prod-2",
            "region": "us-east-1",
            "model": "claude-3-5-sonnet",
            "rate_limit_remaining": 50,
            "saturated": true,  // Other apps should avoid this
            "latency_ms": 380,
        }
    ],
    "requests_last_minute": 127,
}), Priority::Background).await?;

// Other apps receive and update their routing tables
// → Avoid prod-2/us-east-1, prefer prod-1/us-east-1
```

### Custom Channels

```rust
// Subscribe to custom channel
network.subscribe("custom:my-team").await?;
network.subscribe("custom:optimization-cluster").await?;

// Only receive from subscribed channels
```

---

## Message Priority & Attention

### Priority Levels

```rust
pub enum Priority {
    Critical,   // Conflicts, errors - ALWAYS delivered immediately
    Normal,     // Regular coordination - delivered to queue
    Background, // FYI only - batched into periodic digest
}
```

### Attention Modes

```rust
pub enum AttentionMode {
    Realtime,   // All messages delivered immediately (noisy)
    Focused,    // Only Critical immediate; Normal queued; Background digest
    Minimal,    // Only Critical immediate; everything else in digest
}

// Default: Focused
network.set_attention_mode(AttentionMode::Focused)?;
```

### Message Flow

```
┌─────────────────────────────────────────────────────────┐
│                 Incoming Messages                        │
├──────────────┬──────────────┬───────────────────────────┤
│   Critical   │    Normal    │      Background           │
│  (conflicts) │  (requests)  │    (FYI, status)          │
└──────┬───────┴──────┬───────┴───────────┬───────────────┘
       │              │                    │
       ▼              ▼                    ▼
  ┌─────────┐   ┌──────────┐        ┌───────────┐
  │IMMEDIATE│   │  QUEUE   │        │  DIGEST   │
  │ Handler │   │ (check   │        │ (batch    │
  │         │   │  when    │        │  every    │
  │         │   │  ready)  │        │  60s)     │
  └─────────┘   └──────────┘        └───────────┘
```

---

## Flooding Prevention

### Rate Limiting

```rust
const MAX_MESSAGES_PER_PEER_PER_SECOND: u32 = 10;
const MAX_QUEUE_SIZE: usize = 1000;
const QUEUE_EVICTION: QueueEviction = QueueEviction::LowestPriority;
```

### Channel-Based Filtering

Apps only receive messages from subscribed channels.

### TTL Expiration

```rust
// Messages expire and are dropped
const DEFAULT_TTL_SECONDS: u32 = 60;
```

---

## Message Format

### Compact Binary (Routine Messages)

```rust
// 32-byte header for efficiency
struct CompactMessage {
    msg_type: u8,        // 1 byte: HEARTBEAT, STATUS, etc.
    priority: u8,        // 1 byte
    from: [u8; 16],      // 16 bytes: UUID
    channel_hash: u32,   // 4 bytes: FNV hash of channel name
    timestamp: u32,      // 4 bytes: Unix timestamp
    payload_len: u16,    // 2 bytes
    flags: u16,          // 2 bytes: encrypted, compressed
    // payload: msgpack bytes
}

// Type codes
const MSG_HEARTBEAT: u8 = 0x01;    // No payload
const MSG_STATUS: u8 = 0x02;       // Status enum
const MSG_SUGGESTION: u8 = 0x03;
const MSG_BUG_REPORT: u8 = 0x04;
const MSG_ERROR: u8 = 0x05;
const MSG_QUERY: u8 = 0x10;
const MSG_RESPONSE: u8 = 0x11;
const MSG_EXTENDED: u8 = 0xFF;     // Full JSON follows
```

### JSON (Custom Messages)

```json
{
  "id": "msg-uuid",
  "from": "app-uuid",
  "to": "broadcast | group:channel | app-uuid",
  "type": "status | suggestion | bug | query | response",
  "topic": "user-defined",
  "payload": { },
  "priority": "critical | normal | background",
  "timestamp": "ISO-8601",
  "ttl": 60,
  "reply_to": "msg-uuid or null"
}
```

---

## API

### Joining the Network

```rust
// Local network (default)
let network = DashflowNetwork::join(AppConfig {
    name: "MyAgent",
    capabilities: vec!["code-editing"],
}).await?;

// Internet (opt-in)
let network = DashflowNetwork::join_internet(InternetConfig {
    hub_url: "https://hub.example.com",
    auth_token: "...",
}).await?;
```

### Sending Messages

```rust
// Broadcast to channel
network.broadcast("_status", json!({
    "working_on": "dashflow.network",
    "progress": 0.5,
}), Priority::Background).await?;

// Send suggestion to specific peer
network.send_to(peer_id, "_suggestions", json!({
    "suggestion": "Consider caching the API response",
    "file": "src/api.rs",
    "line": 42,
}), Priority::Normal).await?;

// Report a bug
network.send_to(peer_id, "_bugs", json!({
    "bug": "Potential race condition",
    "file": "src/locks.rs",
    "description": "Lock not released on error path",
}), Priority::Normal).await?;

// Query with response
let response = network.request(peer_id, "query", json!({
    "question": "What tests are failing?",
})).await?;
```

### Receiving Messages

```rust
// Set attention mode
network.set_attention_mode(AttentionMode::Focused)?;

// Check for messages (non-blocking)
while let Some(msg) = network.next_message()? {
    match msg.channel.as_str() {
        "_suggestions" => handle_suggestion(msg),
        "_bugs" => handle_bug_report(msg),
        _ => log::debug!("Received: {:?}", msg),
    }
}

// Get digest (for background messages)
let digest = network.digest()?;
println!("{}", digest);
// "Since last check: 5 status updates, 2 suggestions, 0 errors"
```

### Leaving the Network

```rust
network.leave().await?; // Sends goodbye, cleans up
```

---

## Security

### Local Network

- Trust all peers by default
- No authentication required
- No encryption (LAN traffic)

### Internet (Outside LAN)

- TLS required for all connections
- Peer authentication via hub
- Optional message-level encryption

```rust
// Encrypted message (peer's public key)
network.send_encrypted(peer_id, "secret", json!({
    "sensitive_data": "...",
})).await?;
```

---

## Scale Path

### Phase 1: Mesh (Dozens of Peers)

Current design - direct connections between all peers.

### Phase 2: Gossip (Thousands)

Epidemic broadcast protocol - messages spread through network.

### Phase 3: Federation (Millions, Internet)

Hierarchical hubs with regional routing.

```rust
// Transport abstraction allows swapping
trait NetworkTransport {
    async fn send(&self, peer: PeerId, msg: Message) -> Result<()>;
    async fn broadcast(&self, channel: &str, msg: Message) -> Result<()>;
}

// Implementations
struct MeshTransport { /* Phase 1 */ }
struct GossipTransport { /* Phase 2 */ }
struct FederatedTransport { /* Phase 3 */ }
```

---

## HTTP Endpoints

Each app exposes:

```
GET  /dashflow/status     → App identity, capabilities, current work
GET  /dashflow/peers      → Known peers (for peer exchange)
POST /dashflow/message    → Receive incoming message
WS   /dashflow/events     → Real-time event stream
GET  /dashflow/introspect → Full introspection (for coordination)
```

---

## Implementation Phases

| Phase | Commit | Description | Status |
|-------|--------|-------------|--------|
| 1 | N=339 | Core: peer registry, identity, message types | ✅ COMPLETE |
| 2 | N=340 | Communication: HTTP server, WebSocket | ✅ COMPLETE |
| 3 | N=341 | Messaging: channels, broadcast, direct, request/response | ✅ COMPLETE |
| 4 | N=342 | Discovery: mDNS, high-level coordinator, MCP tools | ✅ COMPLETE |
| 5 | N=343 | P2P Resource Discovery: generic resources, sharing, registry | ✅ COMPLETE |

### Phase 1 (N=339) - COMPLETE
- `network/mod.rs` - Module structure and exports
- `network/types.rs` - Core types:
  - `PeerId`, `AppConfig`, `NetworkIdentity`
  - `PeerInfo`, `PeerStatus`, `PeerRegistry`
  - `Channel`, `Priority`, `AttentionMode`
  - `Message`, `MessageType`, `MessageTarget`, `CompactMessage`
  - `LlmEndpointStatus`, `LlmUsageReport`
- 12 new tests (6028 total lib tests)

### Phase 2 (N=340) - COMPLETE
- `network/server.rs` - HTTP server and WebSocket infrastructure:
  - `ServerConfig` - Server configuration with port, host, rate limits
  - `ServerState` - Shared state with message queue, rate limiting, event broadcast
  - `NetworkServer` - Axum-based HTTP/WebSocket server
  - `StatusInfo` - Current work status tracking
  - `RateLimitTracker` - Per-peer rate limit enforcement
  - `NetworkEvent` - WebSocket event types (MessageReceived, PeerJoined, etc.)
- HTTP endpoints:
  - `GET /dashflow/status` - App identity, capabilities, current work
  - `GET /dashflow/peers` - Known peers for peer exchange
  - `POST /dashflow/message` - Receive incoming messages
  - `GET /dashflow/introspect` - Full introspection data
  - `WS /dashflow/events` - Real-time WebSocket event stream
- Features:
  - Message queue with priority-based eviction
  - Attention modes (Realtime/Focused/Minimal)
  - Rate limiting per peer (configurable)
  - WebSocket broadcast for real-time events
- New `network` feature flag in Cargo.toml
- 12 new tests (6040 total lib tests)

### Phase 3 (N=341) - COMPLETE
- `network/messaging.rs` - Messaging layer with high-level client API:
  - `SubscriptionManager` - Channel subscription management with default subscriptions
  - `SubscriptionInfo` - Subscription configuration (timestamps, filters)
  - `RequestManager` - Pending request/response tracking with timeouts
  - `MessageDigest` - Background message aggregation for digest mode
  - `DigestSample` - Sample messages in digest
  - `OutboundQueue` - Queue for messages pending delivery
  - `OutboundMessage` - Message ready for transport with retry tracking
  - `MessagingClient` - High-level client API:
    - Channel subscription (subscribe/unsubscribe)
    - Attention mode control
    - Broadcast messaging
    - Direct peer-to-peer messaging
    - Convenience methods (send_suggestion, send_bug_report, broadcast_error, broadcast_status)
    - Request/response pattern with configurable timeout
    - Incoming message handling with subscription filtering
    - Inbox management with priority-based retrieval
    - Digest accumulation for background messages
    - Outbound queue access for transport layer
  - `MessageRouter` - Route incoming messages to registered handlers
  - `MessagingError` - Error types for messaging operations
- 13 new tests (6041 total lib tests)
- 0 clippy warnings

### Phase 4 (N=342) - COMPLETE
- `network/discovery.rs` - mDNS/DNS-SD discovery infrastructure:
  - `DiscoveryManager` - Manual peer registration with event broadcast
  - `MockDiscovery` - Mock discovery for testing without network
  - `DiscoveryEvent` - Events for peer discovered/goodbye/updated
  - `DiscoveryError` - Error types for discovery operations
  - TXT record encoding/decoding (prepared for full mDNS)
  - Helper functions: `get_local_ip()`, `get_local_addresses()`
  - Service constants: `SERVICE_TYPE`, `DEFAULT_TTL`, `HEARTBEAT_INTERVAL`
- `network/coordinator.rs` - High-level DashflowNetwork API:
  - `DashflowNetwork` - Unified API for network coordination:
    - `join()` / `join_with_config()` - Join local network
    - `mock()` - Create mock network for testing
    - `broadcast()` / `send_to()` - Send messages
    - `subscribe()` / `unsubscribe()` - Channel subscriptions
    - `next_message()` / `peek_messages()` - Receive messages
    - `digest()` / `take_digest()` - Background message digest
    - `set_status()` / `clear_status()` - Status updates
    - `set_attention_mode()` - Attention mode control
    - `peers()` / `peers_with_capability()` - Peer queries
    - `leave()` - Clean network shutdown
  - `NetworkConfig` - Network configuration builder
  - `NetworkError` - Error types for network operations
- `network/tools.rs` - MCP-compatible tools for AI agents:
  - `network_status` - Get current network status and identity
  - `network_peers` - List known peers with capabilities
  - `network_send_message` - Send a message to a peer
  - `network_broadcast` - Broadcast a message to all peers
  - `network_inbox` - Check for incoming messages
  - `network_digest` - Get digest of background messages
  - `network_subscribe` - Subscribe to a channel
  - `network_unsubscribe` - Unsubscribe from a channel
  - `network_set_attention` - Set attention mode
  - `NetworkToolExecutor` - Execute tools by name with JSON params
  - Tool schemas for MCP registration
- Updated `network/mod.rs` with new exports
- 22 new tests in Phase 4 modules
- 0 clippy warnings

### Phase 5 (N=343) - COMPLETE
- `network/resources.rs` - P2P Resource Discovery and Sharing:
  - `ResourceType` - Generic resource types (Llm, Gpu, Storage, VectorDb, Service, Custom)
  - `LlmResourceInfo` - LLM endpoint details (provider, account, region, model, rate limits)
  - `GpuResourceInfo` - GPU compute resources (model, VRAM, CUDA/ROCm version)
  - `StorageResourceInfo` - Shared storage (S3, GCS, NFS, local)
  - `VectorDbResourceInfo` - Vector databases (Pinecone, Qdrant, Weaviate)
  - `ServiceResourceInfo` - Generic service endpoints
  - `AdvertisedResource` - Resource advertisement with capabilities and health
  - `SharingPolicy` - Access control (Private, ColonyOpen, AllowList, RequestApproval, Metered)
  - `AllowResult` - Policy check results
  - `ResourceRequest` / `ResourceResponse` - Request/response protocol
  - `ResourceGrant` - Access grant with token, proxy URL, expiry, limits
  - `ResourceLimits` - Rate limits and quotas
  - `ColonyResourceRegistry` - Colony-wide resource aggregation
  - `ColonyResource` - Resource with owner info
  - `ResourceMessage` - Channel message types (Advertisement, Request, Response, RevokeGrant, Withdrawn)
  - `FailoverStrategy` - Resource selection strategies (Priority, RoundRobin, LeastLoaded, CostOptimized)
- Constants: `RESOURCES_CHANNEL`, `RESOURCES_BROADCAST_INTERVAL`
- 42 new tests (6083 total lib tests)
- 0 clippy warnings

---

## Success Criteria

- [ ] Apps discover each other automatically on LAN
- [ ] Messages delivered with correct priority
- [ ] Flooding prevented (rate limits, channel filtering)
- [ ] AI attention managed (digest mode works)
- [ ] Internet mode works with authentication
- [ ] All existing tests pass
- [ ] 0 clippy warnings

---

## Version History

| Date | Change | Author |
|------|--------|--------|
| 2025-12-09 | Initial design | MANAGER |
