# DashFlow Target Architecture

**Version:** 1.0
**Date:** 2025-12-10
**Purpose:** Define the optimal end-state architecture for DashFlow cleanup phases

---

## 1. Graph Trait Design

### Target State

```rust
// Base trait - sufficient for sequential-only graphs
pub trait GraphState:
    Clone + Send + Sync + Serialize + for<'de> Deserialize<'de> + 'static
{
}

// Extension trait for parallel execution only
pub trait MergeableState: GraphState {
    fn merge(&mut self, other: &Self);
}

// Blanket impl: GraphState auto-implements for anything with the bounds
impl<T> GraphState for T
where
    T: Clone + Send + Sync + Serialize + for<'de> Deserialize<'de> + 'static
{
}
```

### Graph API

```rust
// Sequential graphs - only require GraphState
let graph = StateGraph::<MySimpleState>::new();
graph.add_node("a", node_a);
graph.add_edge("a", "b");
let compiled = graph.compile()?;  // Works without MergeableState

// Parallel graphs - require MergeableState
let graph = StateGraph::<MyMergeableState>::new();
graph.add_parallel_edges("fork", vec!["a", "b"]);  // Compile error if not MergeableState
let compiled = graph.compile()?;  // Validates MergeableState requirement
```

### Compile-Time Enforcement

```rust
impl<S: GraphState> StateGraph<S> {
    pub fn compile(self) -> Result<CompiledGraph<S>> {
        if self.has_parallel_edges && !S::IS_MERGEABLE {
            return Err(Error::MergeableRequired(
                "Graph has parallel edges but state doesn't implement MergeableState"
            ));
        }
        // ... compile logic
    }
}

impl<S: MergeableState> StateGraph<S> {
    // Parallel-specific methods only available when S: MergeableState
    pub fn add_parallel_edges(&mut self, from: &str, to: Vec<&str>) -> &mut Self {
        self.has_parallel_edges = true;
        // ...
    }
}
```

### Common Blanket Implementations

```rust
impl MergeableState for String {
    fn merge(&mut self, other: &Self) {
        self.push_str(other);
    }
}

impl<T: Clone> MergeableState for Vec<T> {
    fn merge(&mut self, other: &Self) {
        self.extend(other.iter().cloned());
    }
}

impl<K: Clone + Eq + Hash, V: Clone> MergeableState for HashMap<K, V> {
    fn merge(&mut self, other: &Self) {
        self.extend(other.iter().map(|(k, v)| (k.clone(), v.clone())));
    }
}
```

---

## 2. Metrics/Observability Architecture

### Target State: Single Source of Truth

```
dashflow-observability (AUTHORITATIVE)
├── GLOBAL_REGISTRY: OnceLock<Arc<MetricsRegistry>>
├── GLOBAL_RECORDER: OnceLock<Arc<MetricsRecorder>>
├── metrics_registry() -> Arc<MetricsRegistry>    // Public accessor
├── export_metrics() -> Result<String>            // Unified export
└── Gathers from both custom AND default Prometheus registries

All crates use:
├── dashflow_observability::metrics_registry()
└── Register to shared registry
```

### Metric Naming Convention

| Type | Pattern | Example |
|------|---------|---------|
| Counter | `*_total` suffix | `graph_invocations_total` |
| Gauge | Descriptive name | `active_graph_executions` |
| Histogram | `*_seconds` or `*_bytes` | `node_duration_seconds` |
| Labels | Consistent naming | `token_type` (not `type`) |

### Export Behavior

```rust
pub fn export(&self) -> Result<String> {
    // Gather from custom registry
    let mut metrics = self.registry.gather();

    // Also gather from default Prometheus registry
    // (for crates that can't depend on dashflow-observability due to cycles)
    metrics.extend(prometheus::gather());

    // Encode and return
    TextEncoder::new().encode(&metrics, &mut buffer)?;
    String::from_utf8(buffer)
}
```

---

## 3. Node Safety: Graph Builder API

### Target State: Progressive Safety Levels

**Level 1: Warning by Default (Backward Compatible)**
```rust
pub fn add_node(&mut self, name: impl Into<String>, node: impl Node<S>) -> &mut Self {
    let name = name.into();
    if self.nodes.contains_key(&name) {
        tracing::warn!("Node '{}' already exists, overwriting", name);
    }
    self.nodes.insert(name, Arc::new(node));
    self
}
```

**Level 2: Strict Mode**
```rust
let graph = StateGraph::new().strict();  // Returns StrictStateGraph
graph.add_node("a", node)?;  // Returns Result, fails on duplicate
```

**Level 3: Explicit Intent APIs**
```rust
impl<S: GraphState> StateGraph<S> {
    /// Add node, error if exists
    pub fn add_node_strict(&mut self, name: &str, node: impl Node<S>) -> Result<&mut Self>

    /// Replace existing node (explicit overwrite)
    pub fn replace_node(&mut self, name: &str, node: impl Node<S>) -> &mut Self

    /// Add or replace (current behavior, explicit)
    pub fn upsert_node(&mut self, name: &str, node: impl Node<S>) -> &mut Self
}
```

### Extended Builder API

```rust
impl<S: GraphState> StateGraph<S> {
    // Queries
    pub fn has_node(&self, name: &str) -> bool
    pub fn node_count(&self) -> usize
    pub fn has_edge(&self, from: &str, to: &str) -> bool

    // Bulk operations
    pub fn add_chain(&mut self, nodes: &[&str]) -> &mut Self  // A -> B -> C

    // Removal (for dynamic reconfiguration)
    pub fn remove_node(&mut self, name: &str) -> Option<BoxedNode<S>>
}
```

---

## 4. Checkpoint Architecture

### Target State: Layered Design

**Layer 1: Policy (When)**
```rust
pub enum CheckpointPolicy {
    Every,                              // Every node (current default)
    EveryN(usize),                      // Every N nodes
    OnMarkers(HashSet<String>),         // Only at marked nodes
    OnStateChange { min_delta: usize }, // On significant change
    Never,                              // Disable checkpointing
}
```

**Layer 2: Strategy (How)**
```rust
pub enum CheckpointStrategy {
    Full,                               // Full state clone
    Differential { full_every: usize }, // Diff-based with periodic full
}
```

**Layer 3: Storage (Where)**
```rust
pub trait Checkpointer<S>: Send + Sync {
    async fn save(&self, checkpoint: Checkpoint<S>) -> Result<String>;
    async fn load(&self, id: &str) -> Result<Option<Checkpoint<S>>>;
    async fn get_latest(&self, thread_id: &str) -> Result<Option<Checkpoint<S>>>;
}
// Implementations: MemoryCheckpointer, SqliteCheckpointer, RedisCheckpointer, etc.
```

### Compiled Graph API

```rust
impl<S: GraphState> CompiledGraph<S> {
    pub fn with_checkpoint_policy(self, policy: CheckpointPolicy) -> Self
    pub fn with_checkpoint_strategy(self, strategy: CheckpointStrategy) -> Self
    pub fn with_checkpointer<C: Checkpointer<S>>(self, checkpointer: C) -> Self

    // Marker API for explicit checkpoints
    pub fn checkpoint_here(&mut self, marker: &str)
}
```

### Differential Checkpoint Implementation

```rust
pub struct DifferentialCheckpointer<S, C: Checkpointer<S>> {
    inner: C,
    base_state: RwLock<Option<S>>,
    full_every: usize,
    checkpoint_count: AtomicUsize,
}

impl<S: GraphState + StateDiff, C: Checkpointer<S>> Checkpointer<S>
    for DifferentialCheckpointer<S, C>
{
    async fn save(&self, checkpoint: Checkpoint<S>) -> Result<String> {
        let count = self.checkpoint_count.fetch_add(1, Ordering::SeqCst);

        if count % self.full_every == 0 {
            // Store full checkpoint, update base
            *self.base_state.write().await = Some(checkpoint.state.clone());
            self.inner.save(checkpoint).await
        } else if let Some(base) = self.base_state.read().await.as_ref() {
            // Store differential
            let diff = checkpoint.state.diff_from(base);
            self.inner.save_diff(checkpoint.id, diff).await
        } else {
            self.inner.save(checkpoint).await
        }
    }
}
```

---

## 5. Pattern Detection Architecture

### Target State: Extensible Adapter Pattern

The current adapter pattern is correct. Target enhancements:

```rust
pub trait PatternEngine: Send + Sync {
    fn detect_patterns(&self, context: &PatternContext) -> Vec<UnifiedPattern>;
    fn engine_name(&self) -> &str;
}

pub struct UnifiedPatternEngine {
    engines: Vec<Box<dyn PatternEngine>>,  // Extensible
    config: UnifiedPatternEngineConfig,
}

impl UnifiedPatternEngine {
    pub fn builder() -> UnifiedPatternEngineBuilder { ... }

    /// Add a custom pattern engine
    pub fn add_engine<E: PatternEngine + 'static>(&mut self, engine: E) {
        self.engines.push(Box::new(engine));
    }

    /// Detect patterns from all engines, deduplicate, filter by threshold
    pub fn detect_all(&self, context: &PatternContext) -> Vec<UnifiedPattern> {
        self.engines
            .iter()
            .flat_map(|e| e.detect_patterns(context))
            .filter(|p| p.confidence >= self.config.min_confidence)
            .deduplicate_by(|p| &p.signature)
            .collect()
    }
}
```

### Built-in Adapters

- `ExecutionPatternAdapter` - wraps `PatternRecognizer`
- `SelfImprovementPatternAdapter` - wraps `PatternDetector`
- `CrossAgentPatternAdapter` - wraps `CrossAgentLearner`

### Required Tests

```rust
#[cfg(test)]
mod tests {
    // Adapter coverage
    fn test_execution_adapter_converts_patterns()
    fn test_self_improvement_adapter_converts_patterns()
    fn test_cross_agent_adapter_converts_patterns()

    // Deduplication
    fn test_dedup_removes_similar_patterns()
    fn test_dedup_preserves_distinct_patterns()

    // Thresholds
    fn test_confidence_threshold_filters()
    fn test_strength_threshold_filters()

    // Integration
    fn test_unified_engine_combines_all_sources()
}
```

---

## Implementation Order for Optimal Architecture

| Phase | Component | Why This Order |
|-------|-----------|----------------|
| 1 | Node Safety (warning) | Immediate UX win, ~10 lines |
| 2 | Optional MergeableState | Highest dev experience impact |
| 3 | Single Metrics Registry | Correctness - single source of truth |
| 4 | Checkpoint Policy | Performance - defer expensive work |
| 5 | Differential Checkpoints | Performance - reduce memory churn |
| 6 | Pattern Engine Tests | Quality - lock in new API |
| 7 | Extended Builder API | Polish - better graph construction UX |

---

## 6. Dynamic Self-Optimizing Runtime

### Design Principle: Graphs Are Data

Graphs are runtime data structures, not compiled Rust. This enables:
- Dynamic graph definition from JSON/YAML/DB/API
- Hot-swapping nodes and edges
- Self-optimization loops without recompilation
- AI-driven graph mutation and experimentation

### Graph Lifecycle

```rust
// 1. DEFINE: From any source (code, JSON, API)
let spec = GraphSpec::from_json(json_string)?;

// 2. BUILD: Runtime construction with node registry
let graph = GraphBuilder::from_spec(&spec, &node_registry)?;

// 3. VALIDATE: Fast compile() for structure validation
let app = graph.compile()?;  // O(nodes + edges)

// 4. EXECUTE: Run with state
let result = app.invoke(initial_state).await?;

// 5. OBSERVE: Metrics, patterns, telemetry
let patterns = app.detected_patterns();

// 6. MUTATE: Modify graph based on observations
graph.remove_node("slow_node")?;
graph.add_node("fast_node", optimized_impl);

// 7. REPEAT: Fast recompile and re-execute
let app = graph.compile()?;  // Cached if structure unchanged
```

### Node Registry (Dynamic Node Discovery)

```rust
pub struct NodeRegistry {
    factories: HashMap<String, Box<dyn NodeFactory>>,
}

impl NodeRegistry {
    /// Register a node factory by name
    pub fn register<F: NodeFactory + 'static>(&mut self, name: &str, factory: F) {
        self.factories.insert(name.to_string(), Box::new(factory));
    }

    /// Create node from name (for dynamic graph building)
    pub fn create_node<S: GraphState>(&self, name: &str, config: Value) -> Result<BoxedNode<S>> {
        self.factories.get(name)
            .ok_or(Error::UnknownNodeType(name.to_string()))?
            .create(config)
    }
}

// Usage: AI discovers available nodes
let available = registry.list_nodes();  // ["tool.search", "llm.chat", "transform.json", ...]
```

### Interpreter Mode (Fast Optimization Loops)

```rust
impl<S: GraphState> StateGraph<S> {
    /// Execute without compile step - for rapid iteration
    /// WARNING: Skips validation, may fail at runtime
    pub async fn execute_unvalidated(&self, state: S) -> Result<S> {
        // Direct interpretation without freezing structures
    }

    /// Compile with delta - only revalidate changed parts
    pub fn compile_delta(&self, previous: &CompiledGraph<S>) -> Result<CompiledGraph<S>> {
        // Reuse unchanged parts, validate only modifications
    }
}

// Optimization loop pattern
let mut graph = base_graph.clone();
let mut best_result = None;

for variant in variants {
    graph.apply_mutation(&variant);
    let result = graph.execute_unvalidated(state.clone()).await?;  // Fast path

    if result.score > best_result.score {
        best_result = Some(result);
        graph.compile()?;  // Validate winning variant
    }
}
```

### Graph Manifest (Import/Export)

```rust
// Export graph as portable manifest
let manifest = graph.manifest();  // Already exists: graph/mod.rs:1112
let json = serde_json::to_string(&manifest)?;

// Import from manifest + registry
let graph = GraphBuilder::from_manifest(&manifest, &node_registry)?;

// Cache compiled graphs by structural hash
let hash = manifest.structural_hash();
let app = cache.get_or_compile(hash, || graph.compile())?;
```

### Telemetry Integration

```rust
impl<S: GraphState> CompiledGraph<S> {
    /// Enable manifest broadcast with telemetry
    pub fn with_manifest_telemetry(self) -> Self {
        // Emit GraphManifest event before execution
        // Tag all events with graph_id/graph_version
    }
}

// Telemetry events include graph context
StreamEvent {
    graph_id: "abc123",
    graph_version: 7,
    manifest_hash: "sha256:...",
    node_name: "researcher",
    // ... event data
}
```

### Node Configuration Mutation (Runtime Prompt/Config Changes)

AI agents can modify node configurations (including prompts) at runtime without rebuilding the graph structure.

```rust
/// Node configuration is mutable data, separate from node logic
pub struct NodeConfig {
    /// Unique node identifier
    pub name: String,
    /// Node type (maps to factory in registry)
    pub node_type: String,
    /// Runtime-mutable configuration (prompts, parameters, etc.)
    pub config: Value,
}

impl<S: GraphState> StateGraph<S> {
    /// Get current configuration for a node
    pub fn get_node_config(&self, name: &str) -> Option<&NodeConfig> {
        self.node_configs.get(name)
    }

    /// Update node configuration without changing graph structure
    /// Returns previous config for rollback
    pub fn update_node_config(&mut self, name: &str, config: Value) -> Result<Value> {
        let node_config = self.node_configs.get_mut(name)
            .ok_or(Error::NodeNotFound(name.to_string()))?;
        let previous = std::mem::replace(&mut node_config.config, config);
        Ok(previous)
    }

    /// Batch update multiple node configs atomically
    pub fn update_configs(&mut self, updates: HashMap<String, Value>) -> Result<HashMap<String, Value>> {
        let mut previous = HashMap::new();
        for (name, config) in updates {
            previous.insert(name.clone(), self.update_node_config(&name, config)?);
        }
        Ok(previous)
    }
}
```

#### Prompt Mutation Example

```rust
// AI agent experimenting with prompts at runtime
let mut graph = base_graph.clone();

// Define prompt variants to test
let prompt_variants = vec![
    json!({"system_prompt": "You are a helpful assistant."}),
    json!({"system_prompt": "You are an expert researcher. Be thorough and cite sources."}),
    json!({"system_prompt": "You are a concise analyst. Give brief, factual answers."}),
];

let mut best_score = 0.0;
let mut best_prompt = None;

for (i, prompt_config) in prompt_variants.iter().enumerate() {
    // Update the LLM node's prompt without rebuilding graph
    let previous = graph.update_node_config("llm_agent", prompt_config.clone())?;

    // Execute with new prompt (no recompile needed for config changes)
    let result = graph.execute_unvalidated(test_state.clone()).await?;

    // Score the result (custom evaluation logic)
    let score = evaluate_result(&result);

    if score > best_score {
        best_score = score;
        best_prompt = Some(prompt_config.clone());
    }

    // Telemetry automatically tags with config version
    // StreamEvent { node_config_hash: "abc123", prompt_variant: i, ... }
}

// Apply winning prompt permanently
if let Some(best) = best_prompt {
    graph.update_node_config("llm_agent", best)?;
    graph.compile()?;  // Validate final configuration
}
```

#### Configuration Schema Discovery

```rust
impl NodeRegistry {
    /// Get the configuration schema for a node type
    /// AI agents use this to understand what can be configured
    pub fn config_schema(&self, node_type: &str) -> Option<&JsonSchema> {
        self.factories.get(node_type)?.config_schema()
    }

    /// List all configurable parameters for a node type
    pub fn configurable_params(&self, node_type: &str) -> Vec<ConfigParam> {
        // Returns: [
        //   ConfigParam { name: "system_prompt", type: "string", required: true },
        //   ConfigParam { name: "temperature", type: "number", default: 0.7 },
        //   ConfigParam { name: "max_tokens", type: "integer", default: 1000 },
        // ]
    }
}

// AI discovers what it can modify
let llm_params = registry.configurable_params("llm.chat");
// AI now knows it can adjust: system_prompt, temperature, max_tokens, etc.
```

#### Self-Modifying Agent Pattern

```rust
/// An agent that optimizes its own prompts based on feedback
pub struct SelfOptimizingAgent<S: GraphState> {
    graph: StateGraph<S>,
    registry: Arc<NodeRegistry>,
    prompt_history: Vec<(Value, f64)>,  // (config, score) pairs
}

impl<S: GraphState> SelfOptimizingAgent<S> {
    /// Execute and learn from the result
    pub async fn execute_and_learn(&mut self, state: S, feedback: impl Fn(&S) -> f64) -> Result<S> {
        let result = self.graph.execute_unvalidated(state).await?;
        let score = feedback(&result);

        // Record this configuration's performance
        let current_config = self.graph.get_node_config("llm_agent")
            .map(|c| c.config.clone())
            .unwrap_or_default();
        self.prompt_history.push((current_config, score));

        // Periodically evolve prompts based on history
        if self.prompt_history.len() % 10 == 0 {
            self.evolve_prompts().await?;
        }

        Ok(result)
    }

    /// Use an LLM to generate improved prompts based on history
    async fn evolve_prompts(&mut self) -> Result<()> {
        // Analyze prompt_history to find patterns
        // Generate new prompt variants using meta-LLM
        // Update graph configuration with best candidates
    }
}
```

### Closing the Self-Improvement Loop (Auto-Apply)

The existing `self_improvement/` system generates plans but requires manual approval. For fully autonomous optimization, connect the output back to graph configs:

```rust
impl SelfImprovementOrchestrator {
    /// Apply approved improvement plans directly to graph configs
    /// Called after consensus validation passes
    pub fn apply_to_graph<S: GraphState>(
        &self,
        graph: &mut StateGraph<S>,
        plan: &ExecutionPlan,
    ) -> Result<ApplyResult> {
        let mut applied = Vec::new();

        for action in &plan.actions {
            match &action.action_type {
                ActionType::UpdatePrompt { node, new_prompt } => {
                    let previous = graph.update_node_config(
                        node,
                        json!({"system_prompt": new_prompt})
                    )?;
                    applied.push(ConfigChange { node: node.clone(), previous, new: new_prompt.clone() });
                }
                ActionType::UpdateParameter { node, param, value } => {
                    let mut config = graph.get_node_config(node)
                        .ok_or(Error::NodeNotFound(node.clone()))?
                        .config.clone();
                    config[param] = value.clone();
                    graph.update_node_config(node, config)?;
                    applied.push(ConfigChange { node: node.clone(), param: param.clone(), value: value.clone() });
                }
                // ... other action types
            }
        }

        Ok(ApplyResult { applied, plan_id: plan.id.clone() })
    }
}

// Full autonomous loop
let mut orchestrator = SelfImprovementOrchestrator::new();
loop {
    let result = graph.execute_unvalidated(state.clone()).await?;
    let trace = graph.get_execution_trace();

    // Analyze and generate improvement proposals
    let proposals = orchestrator.analyze(&trace)?;

    // Validate with multi-model consensus (if API keys present)
    let validated = orchestrator.validate_proposals(proposals).await?;

    // Auto-apply validated improvements (closes the loop!)
    for plan in validated {
        orchestrator.apply_to_graph(&mut graph, &plan)?;
    }
}
```

### Config Versioning in Telemetry

Every config change is versioned and tagged in telemetry for attribution:

```rust
/// Node configuration with version tracking
pub struct NodeConfig {
    pub name: String,
    pub node_type: String,
    pub config: Value,
    pub version: u64,           // Auto-incremented on each update
    pub config_hash: String,    // SHA256 for deduplication/caching
    pub updated_at: DateTime<Utc>,
    pub updated_by: Option<String>,  // "human", "self_improvement", "ab_test"
}

impl<S: GraphState> StateGraph<S> {
    pub fn update_node_config(&mut self, name: &str, config: Value) -> Result<Value> {
        let node_config = self.node_configs.get_mut(name)
            .ok_or(Error::NodeNotFound(name.to_string()))?;

        // Validate against schema before applying
        if let Some(schema) = self.registry.config_schema(&node_config.node_type) {
            schema.validate(&config)?;
        }

        let previous = std::mem::replace(&mut node_config.config, config.clone());
        node_config.version += 1;
        node_config.config_hash = sha256_hash(&config);
        node_config.updated_at = Utc::now();

        Ok(previous)
    }
}

// Telemetry events include config attribution
StreamEvent {
    graph_id: "abc123",
    graph_version: 7,
    manifest_hash: "sha256:...",
    node_name: "llm_agent",
    node_config_version: 3,        // Which config version
    node_config_hash: "def456...", // For deduplication
    // ... event data
}
```

### Node Configs in Introspection

Expose current configs through the introspection API:

```rust
pub struct GraphManifest {
    pub nodes: Vec<NodeManifest>,
    pub edges: Vec<EdgeManifest>,
    pub entry_point: String,
    pub node_configs: HashMap<String, NodeConfig>,  // Current configs
}

impl<S: GraphState> CompiledGraph<S> {
    /// Get current configuration for a specific node
    pub fn node_config(&self, name: &str) -> Option<&NodeConfig> {
        self.manifest().node_configs.get(name)
    }

    /// Get all node configurations
    pub fn all_node_configs(&self) -> &HashMap<String, NodeConfig> {
        &self.manifest().node_configs
    }
}

// AI can inspect its own prompts
let my_prompt = compiled.node_config("llm_agent")
    .map(|c| c.config["system_prompt"].as_str())
    .flatten();
```

### Optimization Loop Telemetry (Meta-Learning)

Track the optimization process itself, not just individual executions:

```rust
/// Telemetry for the optimization loop itself
pub struct OptimizationTrace {
    pub optimization_id: String,
    pub strategy: OptimizationStrategy,
    pub target_node: String,
    pub target_param: String,
    pub variants_tested: Vec<VariantResult>,
    pub best_variant: Option<VariantResult>,
    pub termination_reason: TerminationReason,
    pub total_duration: Duration,
    pub improvement_delta: f64,  // Best score - initial score
}

pub struct VariantResult {
    pub variant_id: String,
    pub config: NodeConfig,
    pub execution_trace_id: String,  // Links to ExecutionTrace
    pub score: f64,
    pub metrics: HashMap<String, f64>,
}

pub enum TerminationReason {
    MaxIterations(usize),
    ConvergenceThreshold(f64),
    TimeLimit(Duration),
    NoImprovement { iterations: usize },
    UserStopped,
}

// Enables meta-learning questions:
// - "Which optimization strategies work best for prompt tuning?"
// - "How many variants are typically needed to find improvement?"
// - "What's the average improvement delta for temperature tuning?"
```

### Package-to-Config Integration

Load prompts from packages directly into node configs:

```rust
impl PackageRegistry {
    /// Get a prompt from an installed package
    pub fn get_prompt(&self, prompt_id: &str) -> Result<PromptTemplate> {
        // prompt_id format: "package-name/prompt-name" or just "prompt-name"
        let (package, name) = parse_prompt_id(prompt_id)?;
        let package = self.get_installed_package(&package)?;
        package.get_prompt(&name)
    }
}

impl PromptTemplate {
    /// Convert to node config format
    pub fn to_node_config(&self) -> Value {
        json!({
            "system_prompt": self.system,
            "user_template": self.user_template,
            "temperature": self.recommended_temperature,
            "max_tokens": self.recommended_max_tokens,
        })
    }
}

// Usage: Load community prompt into graph
let prompt = package_registry.get_prompt("sentiment-pack/analyzer-v2")?;
graph.update_node_config("sentiment_node", prompt.to_node_config())?;
```

---

## 7. AI Ergonomics (Primary Users Are AI Agents)

### Design Principle: Frictionless Happy Path

AIs should be able to build graphs with minimal boilerplate. Default to the simplest safe path.

### Execution Modes

```rust
// LIGHTWEIGHT MODE: Fast iteration, no overhead
let app = graph.compile()?
    .for_testing();  // Alias for without_metrics().without_checkpointing().without_retries()

// OBSERVABILITY MODE: Production with full telemetry
let app = graph.compile()?
    .with_observability();  // Alias for metrics + checkpointing + tracing
```

### Mode Helpers

```rust
impl<S: GraphState> CompiledGraph<S> {
    /// Testing mode: no metrics, no checkpointing, no retries, fast timeouts
    pub fn for_testing(self) -> Self {
        self.without_metrics()
            .without_checkpointing()
            .without_retries()
            .with_timeout(Duration::from_secs(30))
    }

    /// Production mode with full observability
    pub fn with_observability(self) -> Self {
        self.with_metrics()
            .with_checkpointing()
            .with_default_timeouts()
    }

    /// Disable all timeouts (useful for long-running AI tasks)
    pub fn without_timeouts(self) -> Self {
        self.with_timeout(Duration::MAX)
            .with_node_timeout(Duration::MAX)
    }

    /// Sensible defaults for production
    pub fn with_default_timeouts(self) -> Self {
        self.with_timeout(Duration::from_secs(300))
            .with_node_timeout(Duration::from_secs(60))
    }
}
```

### Builder Consistency

Ensure `GraphBuilder` exposes all `StateGraph` methods for fluent usage:

```rust
// Both should work identically
let g1 = StateGraph::new().strict();
let g2 = GraphBuilder::new().strict();  // Must also work

// Convenience constructor
let g = GraphBuilder::strict_builder();  // Strict from the start
```

### Actionable Error Messages

All errors should include:
1. What went wrong
2. Why it's a problem
3. How to fix it (with code snippet)

```rust
// Example: Parallel edges without merge
Error::MergeableRequired {
    message: "Graph has parallel edges but state doesn't implement MergeableState",
    suggestion: r#"
Add merge support to your state:

    impl MergeableState for YourState {
        fn merge(&mut self, other: &Self) {
            // Merge logic here
        }
    }

Or use compile_with_merge() for explicit parallel support.
"#,
}

// Example: Duplicate node
Error::DuplicateNodeName {
    name: "researcher",
    message: "Node 'researcher' already exists",
    suggestion: "Use try_add_node() for error handling or add_node_or_replace() for intentional overwrite",
}
```

### Pattern Engine Integration

Wire pattern detection into CLI and introspection:

```rust
// CLI command
dashflow analyze --patterns ./execution_traces.json

// Programmatic access
let report = app.introspect()?;
let patterns = report.detected_patterns();  // Uses UnifiedPatternEngine
let actionable = patterns.iter().filter(|p| p.is_actionable()).collect();
```

### Checkpoint Ergonomics

```rust
// Simple: checkpoint every N nodes
let app = graph.compile()?.with_checkpoint_every(5);

// Advanced: checkpoint only at markers
let app = graph.compile()?.with_checkpoint_on_markers(vec!["save_point", "critical_node"]);

// Disable for fast iteration
let app = graph.compile()?.without_checkpointing();
```

---

## Success Criteria

### Graph System
- [ ] Sequential graphs work without `MergeableState`
- [ ] Parallel edges require `MergeableState` at compile time
- [ ] Duplicate node insertion emits warning
- [ ] Strict mode available for error-on-duplicate
- [ ] `GraphBuilder` exposes `strict()` and all `StateGraph` methods

### AI Ergonomics
- [ ] `for_testing()` helper bundles lightweight defaults
- [ ] `with_observability()` helper bundles production defaults
- [ ] `without_timeouts()` and `with_default_timeouts()` helpers
- [ ] All errors include actionable suggestions with code snippets
- [ ] Pattern engine wired into CLI and introspection

### Observability
- [ ] Single `/metrics` endpoint exports ALL DashFlow metrics
- [ ] Consistent metric naming (`_total`, `token_type`)
- [ ] No duplicate metrics across crates

### Checkpointing
- [ ] Configurable checkpoint frequency
- [ ] `with_checkpoint_every(n)` helper
- [ ] Differential checkpoints reduce memory overhead by 50%+
- [ ] Default behavior unchanged (backward compatible)

### Pattern Detection
- [ ] Unified API with adapters
- [ ] Extensible for custom engines
- [ ] Comprehensive test coverage
- [ ] Integrated into CLI (`dashflow analyze --patterns`)
- [ ] Integrated into IntrospectionReport

### Dynamic Runtime
- [ ] NodeRegistry for dynamic node discovery
- [ ] `register()` and `create_node()` methods
- [ ] Graph construction from JSON/YAML manifest
- [ ] `from_manifest()` and `from_json()` builders
- [ ] Interpreter mode for fast iteration
- [ ] `execute_unvalidated()` for rapid experimentation
- [ ] `compile_delta()` for incremental validation
- [ ] Manifest telemetry integration
- [ ] Graph ID/version tagged on all events
- [ ] Structural hash caching for compiled graphs

### Configuration Mutation (Self-Modifying AI)
- [ ] `NodeConfig` struct separating config from node logic
- [ ] `get_node_config()` and `update_node_config()` methods
- [ ] `update_configs()` for atomic batch updates
- [ ] Config changes without graph recompilation
- [ ] `config_schema()` for node type introspection
- [ ] `configurable_params()` returns adjustable parameters
- [ ] `SelfOptimizingAgent` pattern implemented
- [ ] Prompt mutation example in documentation
- [ ] Config version tagging in telemetry events

### Closed-Loop Self-Improvement
- [ ] `SelfImprovementOrchestrator.apply_to_graph()` method
- [ ] Auto-apply validated plans to graph configs
- [ ] `NodeConfig.version` auto-incremented on update
- [ ] `NodeConfig.config_hash` for deduplication
- [ ] `NodeConfig.updated_by` attribution ("human", "self_improvement", "ab_test")
- [ ] `node_config_version` in StreamEvent telemetry
- [ ] `OptimizationTrace` for meta-learning
- [ ] `VariantResult` linking configs to execution traces
- [ ] `TerminationReason` enum for optimization stops
- [ ] Schema validation on config updates
- [ ] `node_configs` in `GraphManifest` for introspection
- [ ] `compiled.node_config(name)` accessor
- [ ] Package prompts loadable via `get_prompt()` → `to_node_config()`
