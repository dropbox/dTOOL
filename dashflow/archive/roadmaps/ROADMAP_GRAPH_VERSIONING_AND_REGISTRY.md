# Graph Registry & Versioning Roadmap

**Version:** 1.0
**Date:** 2025-12-09
**Status:** COMPLETE - Phase 7 Implemented (N=266-269)
**Focus:** Graph/Node Versioning and Multi-Graph Management
**User Request:** "lists of graphs and their states for the AI worker" + "node and program versioning design"
**Priority:** P1 (after stability, integrates with AI introspection)
**Implemented:** N=266-269 (135 registry tests)
**Note:** See ROADMAP_UNIFIED.md Phase 7 for completion details.

---

## Executive Summary

AI agents need to:
1. **See all graphs** they're running or have run
2. **Track graph versions** (detect when code changes)
3. **Version nodes** (know which version executed)
4. **Compare versions** (understand what changed)
5. **Registry of executions** (query past runs)

This enables AI agents to understand their execution history and evolution.

---

## Phase 1: Graph Registry (P1 - 15-20 hours)

### 1.1 GraphRegistry - Central Graph Catalog (8-10 hours)

**Purpose:** AI can query "what graphs exist?"

**Implementation:**
```rust
pub struct GraphRegistry {
    graphs: HashMap<String, GraphMetadata>,
}

pub struct GraphMetadata {
    pub graph_id: String,
    pub name: String,
    pub version: String,
    pub created_at: SystemTime,
    pub last_modified: SystemTime,
    pub description: String,
    pub tags: Vec<String>,
    pub node_count: usize,
    pub state_schema: StateSchema,
}

impl GraphRegistry {
    /// Register a graph with metadata
    pub fn register(&mut self, graph: &CompiledGraph, metadata: GraphMetadata) {
        // Store graph info
    }

    /// List all registered graphs
    pub fn list_graphs(&self) -> Vec<GraphMetadata> {
        // Return all graphs
    }

    /// Find graphs by tag
    pub fn find_by_tag(&self, tag: &str) -> Vec<GraphMetadata> {
        // Filter by tag
    }

    /// Get graph by ID
    pub fn get(&self, graph_id: &str) -> Option<&GraphMetadata> {
        // Lookup specific graph
    }
}
```

**Use case:**
```rust
let registry = GraphRegistry::new();

// Register graphs
registry.register(&agent_graph, GraphMetadata {
    graph_id: "agent_v1",
    name: "Coding Agent",
    version: "1.0.0",
    tags: vec!["coding", "production"],
    ...
});

// AI asks: "What graphs are available?"
let all_graphs = registry.list_graphs();

// AI asks: "Which graphs are for coding?"
let coding_graphs = registry.find_by_tag("coding");
```

---

### 1.2 ExecutionRegistry - Execution History (7-10 hours)

**Purpose:** AI can query "what executions have happened?"

**Implementation:**
```rust
pub struct ExecutionRegistry {
    executions: HashMap<String, ExecutionRecord>,
}

pub struct ExecutionRecord {
    pub thread_id: String,
    pub graph_id: String,
    pub graph_version: String,
    pub started_at: SystemTime,
    pub completed_at: Option<SystemTime>,
    pub status: ExecutionStatus,
    pub final_state: Option<serde_json::Value>,
    pub nodes_executed: Vec<String>,
    pub total_tokens: u64,
    pub error: Option<String>,
}

pub enum ExecutionStatus {
    Running,
    Completed,
    Failed,
    Interrupted,
}

impl ExecutionRegistry {
    /// Record new execution
    pub fn record_start(&mut self, thread_id: String, graph_id: String) {
        // Start tracking
    }

    /// Update execution status
    pub fn record_completion(&mut self, thread_id: String, result: ExecutionResult) {
        // Mark complete
    }

    /// List all executions for a graph
    pub fn list_by_graph(&self, graph_id: &str) -> Vec<&ExecutionRecord> {
        // Filter by graph
    }

    /// List recent executions
    pub fn list_recent(&self, limit: usize) -> Vec<&ExecutionRecord> {
        // Most recent N
    }

    /// Get execution details
    pub fn get(&self, thread_id: &str) -> Option<&ExecutionRecord> {
        // Specific execution
    }
}
```

**Use case:**
```rust
let exec_registry = ExecutionRegistry::new();

// AI asks: "What executions are running?"
let running = exec_registry.list_by_status(ExecutionStatus::Running);

// AI asks: "Which executions failed?"
let failed = exec_registry.list_by_status(ExecutionStatus::Failed);

// AI asks: "What were my last 10 runs?"
let recent = exec_registry.list_recent(10);

// AI asks: "How did execution X end?"
let exec = exec_registry.get("thread_123")?;
match exec.status {
    ExecutionStatus::Completed => "Success!",
    ExecutionStatus::Failed => format!("Failed: {}", exec.error.unwrap()),
    ...
}
```

---

## Phase 2: Graph Versioning (P1 - 15-20 hours)

### 2.1 Graph Version Detection (6-8 hours)

**Purpose:** Detect when graph code changes

**Implementation:**
```rust
pub struct GraphVersion {
    pub graph_id: String,
    pub version: String,
    pub content_hash: String,  // Hash of graph structure
    pub source_hash: String,   // Hash of source files
    pub created_at: SystemTime,
}

impl CompiledGraph {
    /// Generate version from graph structure
    pub fn compute_version(&self) -> GraphVersion {
        // Hash nodes, edges, configuration
        let mut hasher = DefaultHasher::new();

        // Hash node names
        for (name, _) in &self.nodes {
            name.hash(&mut hasher);
        }

        // Hash edges
        for (from, edges) in &self.edges {
            from.hash(&mut hasher);
            // Hash edge structure
        }

        let content_hash = format!("{:x}", hasher.finish());

        GraphVersion {
            content_hash,
            version: semver_from_hash(&content_hash),
            ...
        }
    }

    /// Check if graph has changed
    pub fn has_changed_since(&self, previous_version: &GraphVersion) -> bool {
        self.compute_version().content_hash != previous_version.content_hash
    }
}
```

**Use case:**
```rust
// On graph compilation
let current_version = graph.compute_version();

// Check if code changed
if let Some(prev) = version_store.get_previous("agent_graph") {
    if graph.has_changed_since(&prev) {
        println!("⚠️ Graph code has changed since last run!");
        println!("  Old version: {}", prev.version);
        println!("  New version: {}", current_version.version);

        // AI decides: re-run tests, notify user, etc.
    }
}

// Store new version
version_store.save(current_version);
```

---

### 2.2 Node Versioning (5-7 hours)

**Purpose:** Track which version of each node executed

**Implementation:**
```rust
pub struct NodeVersion {
    pub node_name: String,
    pub version: String,
    pub source_file: String,
    pub source_line: usize,
    pub code_hash: String,
}

impl CompiledGraph {
    /// Get versions of all nodes
    pub fn node_versions(&self) -> HashMap<String, NodeVersion> {
        // Map node name -> version
    }
}

// In execution tracking:
pub struct NodeExecution {
    pub node: String,
    pub node_version: String,  // NEW - which version ran
    pub duration: Duration,
    ...
}
```

**Use case:**
```rust
// AI reviews execution
let trace = graph.get_execution_trace(thread_id).await?;

for node_exec in trace.nodes_executed {
    println!("Node {} v{} executed in {:?}",
        node_exec.node,
        node_exec.node_version,  // Know which version!
        node_exec.duration
    );
}

// AI knows: "tool_execution v1.2.3 was slow, but v1.2.4 is fast"
```

---

### 2.3 Version Comparison (4-5 hours)

**Purpose:** AI understands what changed

**Implementation:**
```rust
pub struct GraphDiff {
    pub nodes_added: Vec<String>,
    pub nodes_removed: Vec<String>,
    pub nodes_modified: Vec<String>,
    pub edges_changed: Vec<EdgeChange>,
}

impl GraphVersion {
    /// Compare two graph versions
    pub fn diff(&self, other: &GraphVersion) -> GraphDiff {
        // Compute structural changes
    }

    /// Human-readable change description
    pub fn change_summary(&self, other: &GraphVersion) -> String {
        let diff = self.diff(other);
        format!(
            "Added {} nodes, removed {} nodes, modified {} nodes",
            diff.nodes_added.len(),
            diff.nodes_removed.len(),
            diff.nodes_modified.len()
        )
    }
}
```

**Use case:**
```rust
// AI compares versions
let old_version = version_store.get("agent_graph", "1.0.0")?;
let new_version = graph.compute_version();

let diff = new_version.diff(&old_version);

// AI sees:
// "Added nodes: ['validation', 'retry_logic']"
// "Removed nodes: ['old_deprecated_node']"
// "Modified nodes: ['tool_execution'] (now includes timeout)"
```

---

## Phase 3: State Registry (P1 - 10-12 hours)

### 3.1 State Snapshots (6-8 hours)

**Purpose:** AI can query past states

**Implementation:**
```rust
pub struct StateRegistry {
    snapshots: HashMap<String, Vec<StateSnapshot>>,
}

pub struct StateSnapshot {
    pub thread_id: String,
    pub checkpoint_id: String,
    pub timestamp: SystemTime,
    pub node: String,
    pub state: serde_json::Value,
    pub size_bytes: usize,
}

impl StateRegistry {
    /// Store state snapshot
    pub async fn snapshot(&mut self, thread_id: &str, checkpoint: &Checkpoint) {
        // Store snapshot
    }

    /// Get all states for a thread
    pub async fn get_history(&self, thread_id: &str) -> Vec<StateSnapshot> {
        // Return all snapshots
    }

    /// Get state at specific time
    pub async fn get_at_time(&self, thread_id: &str, time: SystemTime) -> Option<StateSnapshot> {
        // Closest snapshot to time
    }
}
```

**Use case:**
```rust
// AI asks: "What was my state 5 minutes ago?"
let snapshot = state_registry.get_at_time(
    thread_id,
    SystemTime::now() - Duration::from_secs(300)
).await?;

// AI asks: "How has my state changed?"
let history = state_registry.get_history(thread_id).await?;
for (i, snapshot) in history.windows(2).enumerate() {
    let diff = json_diff(&snapshot[0].state, &snapshot[1].state);
    println!("Change {}: {}", i, diff);
}
```

---

### 3.2 State Diff Visualization (4-5 hours)

**Purpose:** AI sees what changed between states

**Implementation:**
```rust
pub fn state_diff(before: &serde_json::Value, after: &serde_json::Value) -> StateDiff {
    // Use json-patch or similar
}

pub struct StateDiff {
    pub added: Vec<String>,      // New fields
    pub removed: Vec<String>,    // Deleted fields
    pub modified: Vec<FieldDiff>, // Changed fields
}

pub struct FieldDiff {
    pub path: String,
    pub before: serde_json::Value,
    pub after: serde_json::Value,
}
```

**Use case:**
```rust
// AI reviews state changes
let before = state_registry.get_at_checkpoint("thread_123", "checkpoint_1").await?;
let after = state_registry.get_at_checkpoint("thread_123", "checkpoint_2").await?;

let diff = state_diff(&before.state, &after.state);

// AI sees:
// "Added: messages[3] = 'User asked about async'"
// "Modified: pending_tool_calls (2 -> 0)"
// "Removed: temp_calculation"
```

---

## Integration Pattern

### Combined API for AI Self-Awareness

```rust
pub struct AISelfAwareness {
    graph_registry: GraphRegistry,
    execution_registry: ExecutionRegistry,
    state_registry: StateRegistry,
    version_tracker: VersionTracker,
}

impl AISelfAwareness {
    /// Complete self-awareness query
    pub async fn introspect(&self, thread_id: &str) -> Introspection {
        Introspection {
            // What graph am I?
            graph: self.graph_registry.get(...)?,

            // Which version?
            version: self.version_tracker.current_version(...)?,

            // What's my current state?
            current_state: self.state_registry.latest(thread_id).await?,

            // What have I executed?
            execution: self.execution_registry.get(thread_id)?,

            // How am I performing?
            performance: self.execution.performance_summary(),
        }
    }

    /// Query API for AI
    pub async fn query(&self, question: &str) -> String {
        match question {
            "what graphs are running?" => {
                let running = self.execution_registry.list_running();
                format!("{} graphs running: {:?}", running.len(), running)
            }
            "what's my current state?" => {
                let state = self.state_registry.latest(...).await?;
                serde_json::to_string_pretty(&state.state)?
            }
            "what version am I?" => {
                let version = self.version_tracker.current_version(...)?;
                format!("Graph v{}, nodes: {:?}", version.version, version.node_versions)
            }
            _ => "Unknown question".to_string(),
        }
    }
}
```

---

## Storage Backend

**Checkpoint Integration:**
```rust
// Store in checkpoint metadata
pub struct Checkpoint {
    pub id: String,
    pub thread_id: String,
    pub state: S,

    // NEW versioning fields:
    pub graph_version: String,
    pub node_versions: HashMap<String, String>,
    pub execution_metadata: ExecutionMetadata,
}

// Query historical executions
let checkpoints = checkpointer.list_all().await?;
for cp in checkpoints {
    println!("Thread {} used graph v{} with nodes v{:?}",
        cp.thread_id,
        cp.graph_version,
        cp.node_versions
    );
}
```

---

## Implementation Plan

### Phase 1: Graph Registry (15-20h)
- N=241-243: GraphRegistry, ExecutionRegistry, integration

### Phase 2: Versioning (15-20h)
- N=244-246: GraphVersion, NodeVersion, comparison

### Phase 3: State Registry (10-12h)
- N=247-248: StateRegistry, state diff

### Integration (5-8h)
- N=249: Combined AISelfAwareness API

**Total:** 45-60 hours

---

## Use Cases

### 1. AI Reviews Its Execution History

```rust
let history = execution_registry.list_by_graph("agent_v1");

for exec in history {
    println!("Run {}: {} nodes, {} tokens, status: {:?}",
        exec.thread_id,
        exec.nodes_executed.len(),
        exec.total_tokens,
        exec.status
    );
}
```

### 2. AI Detects Code Changes

```rust
let current = graph.compute_version();
let previous = version_tracker.get_previous("agent_graph")?;

if current.content_hash != previous.content_hash {
    let diff = current.diff(&previous);
    println!("⚠️ Code changed: {}", diff.change_summary());

    // AI decision: re-run tests, notify user, etc.
}
```

### 3. AI Finds Past States

```rust
// "Show me my state when I last called tool X"
let history = state_registry.get_history(thread_id).await?;

for snapshot in history {
    if snapshot.node == "tool_execution" {
        println!("Found at {}: {:?}",
            snapshot.timestamp,
            snapshot.state.get("tool_args")
        );
    }
}
```

---

## Schema Design

### Graph Version Schema
```json
{
  "graph_id": "coding_agent",
  "version": "1.2.3",
  "content_hash": "a1b2c3d4",
  "nodes": {
    "reasoning": {"version": "1.0.0", "hash": "..."},
    "tool_exec": {"version": "1.1.0", "hash": "..."}
  },
  "created_at": "2025-12-06T10:00:00Z"
}
```

### Execution Record Schema
```json
{
  "thread_id": "session_123",
  "graph_id": "coding_agent",
  "graph_version": "1.2.3",
  "status": "Completed",
  "nodes_executed": ["user_input", "reasoning", "tool_exec", "output"],
  "total_tokens": 15234,
  "total_duration_ms": 8500,
  "checkpoints": ["cp_1", "cp_2", "cp_3"]
}
```

### State Snapshot Schema
```json
{
  "thread_id": "session_123",
  "checkpoint_id": "cp_2",
  "timestamp": "2025-12-06T10:05:23Z",
  "node": "reasoning",
  "state": { /* full state */ },
  "size_bytes": 45234
}
```

---

## Success Criteria

**After Phase 1:**
- [x] AI lists all available graphs
- [x] AI sees all running executions
- [x] AI queries execution history

**After Phase 2:**
- [x] Graph changes detected automatically
- [x] Node versions tracked in execution
- [x] Version diffs show what changed

**After Phase 3:**
- [x] State history queryable
- [x] State diffs visualized
- [x] Past states retrievable

**After Integration:**
- [x] Single API for all introspection
- [x] AI can answer "what/when/why" questions
- [x] Full execution auditability

---

## Timeline

**Current:** Finish stability roadmap (20-30h remaining)

**Then:** Graph Registry & Versioning (45-60h)

**Then:** AI Introspection features (70-90h)

**Total:** 2-3 months for complete AI self-awareness

---

## Progress Tracking

| Phase | Component | Status | Tests | Commit |
|-------|-----------|--------|-------|--------|
| 1.1 | GraphRegistry - Central Graph Catalog | Complete | 26 | #266 |
| 1.2 | ExecutionRegistry - Execution History | Complete | 32 | #266 |
| 2.1 | Graph Version Detection | Complete | 14 | #267 |
| 2.2 | Node Versioning | Complete | 4 | #267 |
| 2.3 | Version Comparison | Complete | 12 | #267 |
| 3.1 | State Snapshots | Complete | 28 | #268 |
| 3.2 | State Diff Visualization | Complete | 19 | #268 |
| Int | AISelfKnowledge Unified API | Complete | - | #270 |

**Total Tests:** 135 graph registry tests passing

**Status: 100% COMPLETE**

---

**This enables AI agents to fully understand their execution context, history, and evolution.**
