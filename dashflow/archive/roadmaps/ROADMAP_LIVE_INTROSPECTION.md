# DashFlow Live Introspection Roadmap

**Version:** 1.1.0
**Date:** 2025-12-09
**Status:** COMPLETE - Phase 10 Implemented (N=309-314)
**Author:** MANAGER AI (N=308), Workers (N=309-314)

---

## Executive Summary

This roadmap defines three distinct levels of introspection for DashFlow applications, enabling AI agents to achieve complete self-awareness at runtime:

1. **Platform Introspection** - DashFlow framework capabilities (shared by all apps)
2. **App Introspection** - Application-specific configuration and structure
3. **Live Execution Introspection** - Real-time state of active graph executions

---

## Current State (N=314 - Phase 10 Complete)

### Implemented Capabilities

| Feature | Module | Status |
|---------|--------|--------|
| Graph Manifest | `introspection.rs` | Complete |
| Platform Registry | `platform_registry.rs` | Complete |
| MCP Self-Documentation | `mcp_self_doc.rs` | Complete |
| **Platform Introspection** | `platform_introspection.rs` | **Complete (N=309)** |
| **App Introspection Enhancement** | `mcp_self_doc.rs` | **Complete (N=310)** |
| **Live Execution Introspection** | `live_introspection.rs` | **Complete (N=311-312)** |
| **Unified Introspection API** | `executor.rs` | **Complete (N=313)** |
| Execution Context | `introspection.rs` | Complete |
| Capability Manifest | `introspection.rs` | Complete |
| App Architecture | `platform_registry.rs` | Complete |

### Test Status
- **5589** dashflow lib tests (2 ignored)
- **320** streaming lib tests (20 ignored)
- **0** clippy warnings
- **0** doc warnings

### Outstanding Technical Debt (from v4 audit)

| Category | Remains | Notes |
|----------|---------|-------|
| Security/Config | 2 | Decoder validation (use `decode_message_strict()`) |
| Resource Safety | 1 | State diff memory (mitigated: 10MB limit) |
| Data Integrity | 1 | Decoder validation (use `decode_message_with_validation()`) |
| Resilience | 1 (partial) | Producer shutdown flush (5s timeout limit) |

---

## Phase 10: Live Introspection Architecture

### 10.1 Three-Level Introspection Model

```
+------------------------------------------------------------------+
|                    INTROSPECTION LEVELS                           |
+------------------------------------------------------------------+
|                                                                   |
|  Level 1: PLATFORM                                                |
|  +---------------------------------------------------------+     |
|  | DashFlow Framework Capabilities                          |     |
|  | - Available features (checkpointing, retries, limits)    |     |
|  | - Supported node types                                   |     |
|  | - Edge types (conditional, parallel, simple)             |     |
|  | - State types (MergeableState implementations)           |     |
|  | - Built-in templates (supervisor, react_agent)           |     |
|  | - Version information                                    |     |
|  | SCOPE: Shared by ALL DashFlow applications               |     |
|  +---------------------------------------------------------+     |
|                                                                   |
|  Level 2: APP                                                     |
|  +---------------------------------------------------------+     |
|  | Application-Specific Configuration                       |     |
|  | - Graph structure (nodes, edges, entry point)            |     |
|  | - Configured features (which opts-outs applied)          |     |
|  | - Custom node implementations                            |     |
|  | - State schema for this app                              |     |
|  | - Tools available to this app                            |     |
|  | - Dependencies (DashFlow + external crates)              |     |
|  | SCOPE: Specific to ONE compiled graph                    |     |
|  +---------------------------------------------------------+     |
|                                                                   |
|  Level 3: LIVE                                                    |
|  +---------------------------------------------------------+     |
|  | Runtime Execution State                                  |     |
|  | - Active executions (multiple concurrent possible)       |     |
|  | - Current node in each execution                         |     |
|  | - Current state values                                   |     |
|  | - Execution history (nodes visited, transitions)         |     |
|  | - Performance metrics (timing, resource usage)           |     |
|  | - Checkpoint status                                      |     |
|  | SCOPE: Per-execution instance (many per app)             |     |
|  +---------------------------------------------------------+     |
|                                                                   |
+------------------------------------------------------------------+
```

---

## 10.2 Platform Introspection Interface

### API Design

```rust
/// Platform-level introspection - DashFlow framework capabilities
/// Shared by ALL DashFlow applications
pub trait PlatformIntrospection {
    /// Get DashFlow version information
    fn dashflow_version(&self) -> VersionInfo;

    /// List all available features (checkpointing, retries, etc.)
    fn available_features(&self) -> Vec<FeatureInfo>;

    /// List supported node types
    fn supported_node_types(&self) -> Vec<NodeTypeInfo>;

    /// List supported edge types
    fn supported_edge_types(&self) -> Vec<EdgeTypeInfo>;

    /// List built-in templates
    fn built_in_templates(&self) -> Vec<TemplateInfo>;

    /// List available MergeableState implementations
    fn state_implementations(&self) -> Vec<StateTypeInfo>;

    /// Query platform capabilities by name
    fn query_capability(&self, name: &str) -> Option<CapabilityInfo>;

    /// Export as JSON for AI consumption
    fn to_json(&self) -> String;
}

/// Version information for DashFlow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionInfo {
    pub version: String,           // "1.11.3"
    pub rust_version: String,      // "1.75.0"
    pub features_enabled: Vec<String>, // ["checkpointing", "metrics"]
    pub build_timestamp: Option<String>,
}

/// Information about a DashFlow feature
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureInfo {
    pub name: String,
    pub description: String,
    pub default_enabled: bool,
    pub opt_out_method: Option<String>, // "without_checkpointing()"
    pub documentation_url: Option<String>,
}
```

### MCP Endpoints

| Endpoint | Description |
|----------|-------------|
| `GET /mcp/platform/version` | DashFlow version and build info |
| `GET /mcp/platform/features` | All available features |
| `GET /mcp/platform/node-types` | Supported node types |
| `GET /mcp/platform/edge-types` | Supported edge types |
| `GET /mcp/platform/templates` | Built-in templates |
| `GET /mcp/platform/states` | Available state implementations |
| `GET /mcp/platform/query?q=...` | Natural language query |

---

## 10.3 App Introspection Interface

### API Design

```rust
/// App-level introspection - specific to one compiled graph
pub trait AppIntrospection {
    /// Get the graph manifest (structure)
    fn manifest(&self) -> &GraphManifest;

    /// Get app architecture analysis
    fn architecture(&self) -> &AppArchitecture;

    /// Get capability manifest (what this app can do)
    fn capabilities(&self) -> &CapabilityManifest;

    /// List nodes with their metadata
    fn nodes(&self) -> Vec<NodeInfo>;

    /// Get specific node details
    fn node(&self, name: &str) -> Option<NodeDetailInfo>;

    /// List edges (connections between nodes)
    fn edges(&self) -> Vec<EdgeInfo>;

    /// List tools available to this app
    fn tools(&self) -> Vec<ToolInfo>;

    /// List dependencies
    fn dependencies(&self) -> DependenciesInfo;

    /// Get configured features (which are enabled/disabled)
    fn configured_features(&self) -> Vec<ConfiguredFeatureInfo>;

    /// Get state schema
    fn state_schema(&self) -> Option<StateSchema>;

    /// Unified introspection query
    fn introspect(&self) -> GraphIntrospection;
}

/// Information about a configured feature
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfiguredFeatureInfo {
    pub name: String,
    pub enabled: bool,
    pub configuration: Option<serde_json::Value>,
}
```

### MCP Endpoints (Existing - Enhanced)

| Endpoint | Description |
|----------|-------------|
| `GET /mcp/about` | App overview |
| `GET /mcp/capabilities` | What this app can do |
| `GET /mcp/architecture` | Graph structure analysis |
| `GET /mcp/implementation` | Implementation details |
| `GET /mcp/nodes` | List all nodes |
| `GET /mcp/nodes/:name` | Specific node details |
| `GET /mcp/edges` | All edges/connections |
| `GET /mcp/features` | Features used by this app |
| `GET /mcp/dependencies` | Dependencies list |
| `GET /mcp/tools` | Available tools |
| `GET /mcp/state-schema` | State structure |

---

## 10.4 Live Execution Introspection Interface

### API Design

```rust
/// Live execution introspection - runtime state of active executions
pub trait LiveIntrospection {
    /// List all active executions
    fn active_executions(&self) -> Vec<ExecutionSummary>;

    /// Get detailed state of a specific execution
    fn execution(&self, execution_id: &str) -> Option<ExecutionState>;

    /// Get current node for an execution
    fn current_node(&self, execution_id: &str) -> Option<String>;

    /// Get current state values for an execution
    fn current_state(&self, execution_id: &str) -> Option<serde_json::Value>;

    /// Get execution history (nodes visited)
    fn execution_history(&self, execution_id: &str) -> Vec<ExecutionStep>;

    /// Get performance metrics for an execution
    fn execution_metrics(&self, execution_id: &str) -> Option<ExecutionMetrics>;

    /// Get checkpoint status for an execution
    fn checkpoint_status(&self, execution_id: &str) -> Option<CheckpointStatus>;

    /// Subscribe to execution events (WebSocket/SSE)
    fn subscribe(&self, execution_id: &str) -> ExecutionEventStream;

    /// Query execution state with natural language
    fn query(&self, execution_id: &str, query: &str) -> QueryResponse;
}

/// Summary of an active execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionSummary {
    pub execution_id: String,
    pub graph_name: String,
    pub started_at: String,
    pub current_node: String,
    pub iteration: u32,
    pub status: ExecutionStatus,
}

/// Detailed execution state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionState {
    pub execution_id: String,
    pub graph_name: String,
    pub started_at: String,
    pub current_node: String,
    pub previous_node: Option<String>,
    pub iteration: u32,
    pub total_nodes_visited: u32,
    pub state: serde_json::Value,
    pub metrics: ExecutionMetrics,
    pub checkpoint: Option<CheckpointStatus>,
    pub status: ExecutionStatus,
}

/// A single step in execution history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionStep {
    pub step_number: u32,
    pub node_name: String,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub duration_ms: Option<u64>,
    pub state_before: Option<serde_json::Value>,
    pub state_after: Option<serde_json::Value>,
    pub outcome: StepOutcome,
}

/// Execution status
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ExecutionStatus {
    Running,
    Paused,
    WaitingForInput,
    Completed,
    Failed,
    Cancelled,
}

/// Real-time execution event stream
pub struct ExecutionEventStream {
    receiver: tokio::sync::broadcast::Receiver<ExecutionEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionEvent {
    NodeEntered { node: String, timestamp: String },
    NodeExited { node: String, duration_ms: u64, timestamp: String },
    StateChanged { diff: serde_json::Value, timestamp: String },
    CheckpointCreated { checkpoint_id: String, timestamp: String },
    ExecutionCompleted { final_state: serde_json::Value, timestamp: String },
    ExecutionFailed { error: String, timestamp: String },
}
```

### MCP Endpoints (New)

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/mcp/live/executions` | GET | List all active executions |
| `/mcp/live/executions/:id` | GET | Get execution state |
| `/mcp/live/executions/:id/node` | GET | Current node |
| `/mcp/live/executions/:id/state` | GET | Current state values |
| `/mcp/live/executions/:id/history` | GET | Execution history |
| `/mcp/live/executions/:id/metrics` | GET | Performance metrics |
| `/mcp/live/executions/:id/checkpoint` | GET | Checkpoint status |
| `/mcp/live/executions/:id/events` | WS/SSE | Real-time events |
| `/mcp/live/executions/:id/query` | POST | Natural language query |

---

## 10.5 Implementation Plan

### Phase 10A: Platform Introspection (2-3 commits)

1. **N=308**: Create `platform_introspection.rs` module
   - Implement `PlatformIntrospection` trait
   - Add version info gathering
   - Add feature enumeration
   - Unit tests

2. **N=309**: Add MCP endpoints for platform introspection
   - `/mcp/platform/*` endpoints
   - Integration with existing `McpSelfDocServer`
   - Tests

3. **N=310**: Platform query interface
   - Natural language query support for platform capabilities
   - Documentation generation

### Phase 10B: App Introspection Enhancement (2-3 commits)

4. **N=311**: Enhance `AppIntrospection` trait
   - Add `configured_features()` method
   - Add `state_schema()` method
   - Enhance existing methods with richer metadata

5. **N=312**: New MCP endpoints
   - `/mcp/tools` endpoint
   - `/mcp/state-schema` endpoint
   - Enhanced `/mcp/features` with config details

### Phase 10C: Live Execution Introspection (4-5 commits)

6. **N=313**: Create `live_introspection.rs` module
   - `LiveIntrospection` trait
   - `ExecutionTracker` for managing active executions
   - Basic state tracking

7. **N=314**: Execution history and metrics
   - `ExecutionStep` recording
   - `ExecutionMetrics` collection
   - Integration with existing `ExecutionContext`

8. **N=315**: Real-time event streaming
   - `ExecutionEventStream` implementation
   - Broadcast channel for events
   - WebSocket/SSE endpoint preparation

9. **N=316**: MCP live endpoints
   - `/mcp/live/*` endpoints
   - HTTP handlers with proper async streaming
   - Tests

10. **N=317**: Live query interface
    - Natural language query for execution state
    - "What node am I on?" type queries
    - Integration tests

### Phase 10D: Integration and Polish (2 commits)

11. **N=318**: Unified introspection API
    - Single `introspect()` method returning all three levels
    - Documentation update
    - Example application update

12. **N=319**: Final testing and documentation
    - End-to-end integration tests
    - README update
    - ROADMAP_UNIFIED.md update

---

## 10.6 Estimated Effort

| Phase | Commits | Complexity | Description |
|-------|---------|------------|-------------|
| 10A | 3 | Medium | Platform introspection |
| 10B | 2-3 | Low | App introspection enhancement |
| 10C | 5 | High | Live execution introspection (new) |
| 10D | 2 | Low | Integration and polish |
| **Total** | **12-13** | - | ~2.5 hours AI time |

---

## 10.7 Outstanding Issues Integration

The following items from the v4 audit should be addressed during this phase:

### Security Hardening (During Phase 10C)
- Ensure live introspection endpoints validate input
- Add rate limiting for live execution queries
- Prevent information leakage in execution state

### Resource Safety (During Phase 10C)
- Bound execution history retention (default: last 100 steps per execution)
- Limit concurrent execution tracking (default: 1000 active executions)
- Auto-cleanup completed executions after configurable TTL

### Observability (During Phase 10D)
- Add Prometheus metrics for introspection queries
- Log introspection access patterns
- Track query latencies

---

## Success Criteria

### Phase 10 Complete When:

- [x] `compiled.platform_introspection()` returns `PlatformIntrospection` with all DashFlow capabilities
- [x] `compiled.introspect()` returns enhanced `GraphIntrospection` with feature configs
- [x] `compiled.live_executions()` returns `Vec<ExecutionSummary>` for runtime state
- [x] `compiled.unified_introspection()` returns all three levels in one call
- [x] All three levels accessible via MCP HTTP endpoints
- [x] Real-time event streaming works via SSE endpoints (`/mcp/live/events`)
- [x] Natural language queries work via `/mcp/platform/query` and `/mcp/introspect?q=...`
- [x] Example application demonstrates all three levels (`three_level_introspection.rs`)
- [x] 133+ new tests added (5456→5589 = 133 new tests)
- [x] 0 clippy warnings, 0 doc warnings
- [x] README documentation updated (N=314)

---

## API Summary

```rust
// Phase 10 implementation - Actual API (N=314):
let compiled = graph.compile()?;

// Level 1: Platform (DashFlow framework)
let platform = compiled.platform_introspection();
platform.version();           // VersionInfo
platform.features();          // Vec<FeatureInfo>
platform.node_types();        // Vec<NodeTypeInfo>
platform.edge_types();        // Vec<EdgeTypeInfo>
platform.templates();         // Vec<TemplateInfo>
platform.state_implementations(); // Vec<StateTypeInfo>

// Level 2: App (This specific graph)
let app = compiled.introspect();
app.manifest();          // GraphManifest
app.architecture();      // AppArchitecture
app.capabilities();      // CapabilityManifest

// Level 3: Live (Runtime executions) - requires ExecutionTracker
let tracker = ExecutionTracker::new();
let compiled = compiled.with_execution_tracker(tracker.clone());
let executions = compiled.live_executions();  // Vec<ExecutionSummary>

// Unified introspection - all three levels in one call
let unified = compiled.unified_introspection();
unified.platform;        // PlatformIntrospection
unified.app;             // GraphIntrospection
unified.live;            // Vec<ExecutionSummary>
unified.to_json()?;      // Serialize all levels

// Real-time event streaming
let stream = tracker.subscribe();             // All events
let stream = tracker.subscribe_to_execution("exec-123"); // Filtered
while let Some(event) = stream.recv().await { ... }
```

---

## References

- Current MCP implementation: `crates/dashflow/src/mcp_self_doc.rs`
- Current introspection: `crates/dashflow/src/introspection.rs`
- Platform registry: `crates/dashflow/src/platform_registry.rs`
- Executor with streaming: `crates/dashflow/src/executor.rs`
- Outstanding issues: `OPENAI_CATEGORIZED_ISSUES_2025-12-05_v4.md`

---

**This roadmap provides the blueprint for making DashFlow applications fully self-aware at all three levels: platform, app, and live execution.**
