# Dasher & Dash.ai: Multi-Instance Architecture Plan

**Created:** 2025-12-16
**Purpose:** Define architecture for two distinct multi-instance patterns sharing common infrastructure

---

## Coordination Spectrum

Not just two binary modes - there's a **spectrum** of coordination levels:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    COORDINATION SPECTRUM                                    │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ISOLATED         SESSION          ORG-LEVEL        FULL SWARM             │
│  (Dash.ai         (Dash.ai         (Enterprise)     (Dasher)               │
│   single)          multi-agent)                                             │
│                                                                             │
│    ○──────────────────○──────────────────○──────────────────○              │
│    │                  │                  │                  │               │
│    │ No P2P           │ Sub-agent        │ Org boundary     │ Global        │
│    │ Pure serving     │ coordination     │ coordination     │ collaboration │
│    │                  │ within session   │                  │               │
│                                                                             │
│  Examples:           Examples:          Examples:          Examples:        │
│  - Quick Q&A         - Deep research    - Team knowledge   - Building sw   │
│  - Simple lookup     - Multi-step task  - Shared tools     - Code review   │
│  - Chat turn         - Agent spawns     - Org patterns     - Fleet learn   │
│                        sub-agents                                           │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Level 0: Isolated (Dash.ai Single-Turn)
**Use Case:** Quick Q&A, simple lookups

```
User → Instance → Response → Done (instance terminates)
         │
         └──▶ Telemetry to Nexus (aggregation only)
```

### Level 1: Session Coordination (Dash.ai Multi-Agent)
**Use Case:** Deep research, complex tasks requiring sub-agents

```
User → Primary Instance ──┬──▶ Sub-agent 1 (search)
              │           ├──▶ Sub-agent 2 (analyze)
              │           └──▶ Sub-agent 3 (synthesize)
              │                    │
              └────────────────────┴──▶ Coordinate within session boundary
                                        (temporary, session-scoped)
```

### Level 2: Organization Coordination
**Use Case:** Enterprise teams, shared organizational knowledge

```
┌─────────────────── Organization Boundary ───────────────────┐
│                                                             │
│  User A's Session    User B's Session    User C's Session  │
│        │                   │                   │            │
│        └───────────────────┴───────────────────┘            │
│                            │                                │
│                   Org Knowledge Base                        │
│                   - Approved patterns                       │
│                   - Team conventions                        │
│                   - Shared tools/prompts                    │
└─────────────────────────────────────────────────────────────┘
```

### Level 3: Full Swarm (Dasher)
**Use Case:** Autonomous software development, fleet-wide learning

```
┌─────────────────────────────────────────────────────────────┐
│                    DASHER SWARM                             │
│  All instances collaborate, learn together, coordinate work │
├─────────────────────────────────────────────────────────────┤
│  ✓ P2P mesh between all instances                          │
│  ✓ Global knowledge base (all learnings shared)            │
│  ✓ Task deduplication across entire swarm                  │
│  ✓ Conflict resolution (parallel code modifications)       │
│  ✓ Learning propagation (Instance A learns → all learn)    │
│  ✓ Work stealing and load balancing                        │
└─────────────────────────────────────────────────────────────┘
```

---

## Primary Deployment Patterns

While the spectrum exists, the two primary deployment patterns are:

### Pattern A: Dash.ai (Production Serving) - Levels 0-2
**Primary:** Per-session instances with optional sub-agent coordination
**Telemetry:** Always aggregated to Nexus
**Coordination:** None (L0), session-scoped (L1), or org-scoped (L2)

### Pattern B: Dasher (Swarm) - Level 3
**Primary:** Long-running collaborative instances
**Telemetry:** Aggregated + real-time P2P sharing
**Coordination:** Full global coordination

---

## Shared Infrastructure (Both Patterns)

```
┌─────────────────────────────────────────────────────────────┐
│               SHARED COMPONENTS                             │
├─────────────────────────────────────────────────────────────┤
│  1. Fleet Telemetry Aggregation                             │
│     - Instance-labeled Prometheus metrics                   │
│     - Cross-instance trace correlation                      │
│     - Fleet-wide Grafana dashboards                         │
│                                                             │
│  2. Centralized Eval System                                 │
│     - Quality scores by instance, model, prompt             │
│     - A/B test results aggregation                          │
│     - Regression detection                                  │
│                                                             │
│  3. Cost Tracking                                           │
│     - Per-instance, per-user, per-model costs              │
│     - Budget alerts and limits                              │
│     - Usage forecasting                                     │
│                                                             │
│  4. Smart Optimizer Selection                               │
│     - Auto-select optimizer based on task/data             │
│     - Track optimizer effectiveness across fleet            │
│     - Continuous optimization of prompts/models             │
└─────────────────────────────────────────────────────────────┘
```

---

## Architecture Diagram

```
                         ┌─────────────────────────────────────┐
                         │            NEXUS                    │
                         │    (Central Services Layer)         │
                         ├─────────────────────────────────────┤
                         │  ┌───────────┐  ┌───────────────┐  │
                         │  │ Knowledge │  │ Task Queue    │  │  ← Dasher only
                         │  │   Base    │  │ (dedup/coord) │  │
                         │  └───────────┘  └───────────────┘  │
                         │  ┌───────────┐  ┌───────────────┐  │
                         │  │ Fleet     │  │ Eval          │  │  ← Both patterns
                         │  │ Telemetry │  │ Aggregator    │  │
                         │  └───────────┘  └───────────────┘  │
                         │  ┌───────────┐  ┌───────────────┐  │
                         │  │ Cost      │  │ Optimizer     │  │  ← Both patterns
                         │  │ Tracker   │  │ Selector      │  │
                         │  └───────────┘  └───────────────┘  │
                         └──────────────┬──────────────────────┘
                                        │
        ┌───────────────────────────────┼───────────────────────────────┐
        │                               │                               │
        │         DASHER SWARM          │         DASH.AI SERVING       │
        │                               │                               │
        │  ┌─────┐  ┌─────┐  ┌─────┐   │   ┌─────┐  ┌─────┐  ┌─────┐  │
        │  │ D1  │◀▶│ D2  │◀▶│ D3  │   │   │ U1  │  │ U2  │  │ U3  │  │
        │  │     │  │     │  │     │   │   │     │  │     │  │     │  │
        │  └──┬──┘  └──┬──┘  └──┬──┘   │   └──┬──┘  └──┬──┘  └──┬──┘  │
        │     │        │        │       │      │        │        │      │
        │     └────────┼────────┘       │      │        │        │      │
        │         P2P mesh              │      │   (isolated)    │      │
        │                               │      │                 │      │
        └───────────────────────────────┼──────┴─────────────────┴──────┘
                                        │
                                        ▼
                         ┌─────────────────────────────────────┐
                         │       OBSERVABILITY STACK           │
                         │  Prometheus │ Grafana │ Jaeger      │
                         │  Kafka      │ ClickHouse (traces)   │
                         └─────────────────────────────────────┘
```

---

## Part 1: Fleet Telemetry (BOTH PATTERNS)

### Current State
- Prometheus metrics exist but NO instance labels
- Single-instance aggregation only
- No fleet-wide dashboards

### Required Changes

#### 1.1 Instance Labels on All Metrics

```rust
// crates/dashflow-observability/src/metrics.rs

/// Fleet-aware metric labels (add to ALL metrics)
pub struct FleetLabels {
    pub instance_id: String,      // Unique instance UUID
    pub deployment: String,       // "dasher" | "dash-ai" | "dev"
    pub version: String,          // App version
    pub host: String,             // Hostname/pod name
    pub region: Option<String>,   // For geo-distributed
}

impl FleetLabels {
    pub fn from_env() -> Self {
        Self {
            instance_id: env::var("DASHFLOW_INSTANCE_ID")
                .unwrap_or_else(|_| Uuid::new_v4().to_string()),
            deployment: env::var("DASHFLOW_DEPLOYMENT")
                .unwrap_or_else(|_| "dev".to_string()),
            version: env!("CARGO_PKG_VERSION").to_string(),
            host: hostname::get().unwrap_or_default(),
            region: env::var("DASHFLOW_REGION").ok(),
        }
    }

    pub fn as_labels(&self) -> [(&'static str, String); 4] {
        [
            ("instance_id", self.instance_id.clone()),
            ("deployment", self.deployment.clone()),
            ("version", self.version.clone()),
            ("host", self.host.clone()),
        ]
    }
}
```

#### 1.2 Prometheus Remote Write

```yaml
# prometheus.yml addition
remote_write:
  - url: "${NEXUS_PROMETHEUS_URL}/api/v1/write"
    headers:
      Authorization: "Bearer ${NEXUS_API_TOKEN}"
    queue_config:
      max_samples_per_send: 10000
      batch_send_deadline: 5s
```

#### 1.3 Fleet Dashboards

```
New Grafana dashboards:
├── fleet_overview.json          # All instances at a glance
├── fleet_instance_comparison.json   # Compare specific instances
├── fleet_quality_trends.json    # Quality over time by deployment
├── fleet_cost_breakdown.json    # Cost by instance/user/model
└── fleet_anomaly_detection.json # Outlier instances
```

**Queries:**
```promql
# Quality score by instance
avg by (instance_id) (dashstream_quality_score)

# Error rate per deployment
sum by (deployment) (rate(dashstream_errors_total[5m]))

# Top 10 slowest instances
topk(10, avg by (instance_id) (dashstream_latency_seconds))

# Cost per user (Dash.ai)
sum by (user_id) (dashstream_cost_dollars_total)
```

---

## Part 2: Eval Aggregation (BOTH PATTERNS)

### Purpose
Track quality across ALL instances to detect:
- Model regressions
- Prompt degradation
- Instance-specific issues
- A/B test results

### Architecture

```rust
// crates/dashflow-nexus/src/eval_aggregator.rs

pub struct EvalAggregator {
    /// Store: ClickHouse or TimescaleDB for high-cardinality time series
    store: Box<dyn EvalStore>,
}

#[derive(Debug, Serialize)]
pub struct EvalRecord {
    pub timestamp: DateTime<Utc>,
    pub instance_id: String,
    pub deployment: String,

    // What was evaluated
    pub graph_name: String,
    pub node_name: String,
    pub prompt_version: String,
    pub model: String,

    // Results
    pub quality_score: f64,
    pub latency_ms: u64,
    pub tokens_in: u32,
    pub tokens_out: u32,
    pub cost_dollars: f64,

    // Context
    pub user_id: Option<String>,       // Dash.ai
    pub task_id: Option<String>,       // Dasher
    pub ab_test_variant: Option<String>,
}

impl EvalAggregator {
    /// Query aggregated quality by model
    pub async fn quality_by_model(&self, since: Duration) -> Vec<ModelQuality> { ... }

    /// Detect regression (quality dropped vs baseline)
    pub async fn detect_regression(&self, threshold: f64) -> Vec<Regression> { ... }

    /// A/B test significance calculation
    pub async fn ab_test_results(&self, test_id: &str) -> ABTestResult { ... }

    /// Instance-level anomalies
    pub async fn instance_anomalies(&self) -> Vec<InstanceAnomaly> { ... }
}
```

### CLI Integration

```bash
# View fleet-wide eval summary
dashflow nexus evals summary --since 24h

# Compare models across fleet
dashflow nexus evals compare-models --models gpt-4o,claude-3-opus

# View A/B test results
dashflow nexus evals ab-test results --test prompt_v2_vs_v1

# Detect regressions
dashflow nexus evals regressions --threshold 0.05
```

---

## Part 3: Nexus Services (DASHER ONLY)

These services are specific to the collaborative swarm pattern.

### 3.1 Knowledge Base

```rust
// crates/dashflow-nexus/src/knowledge.rs

/// What the swarm has collectively learned
pub struct KnowledgeBase {
    store: Box<dyn KnowledgeStore>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Knowledge {
    /// Pattern that leads to success
    SuccessPattern {
        pattern: String,
        confidence: f64,
        source_instances: Vec<String>,
        examples: Vec<TraceId>,
    },

    /// Pitfall to avoid
    Pitfall {
        description: String,
        severity: Severity,
        mitigation: String,
        discovered_by: String,
    },

    /// Optimized prompt
    PromptOptimization {
        original: String,
        optimized: String,
        improvement: f64,
        optimizer_used: String,
    },

    /// Code pattern that works well
    CodePattern {
        language: String,
        pattern: String,
        use_case: String,
        approved_by: Vec<String>,
    },
}

impl KnowledgeBase {
    /// When an instance learns something, broadcast to all
    pub async fn share(&self, knowledge: Knowledge) -> Result<()> {
        self.store.insert(knowledge.clone()).await?;
        self.broadcast_to_swarm(knowledge).await
    }

    /// Query relevant knowledge for a task
    pub async fn query(&self, task: &str) -> Vec<Knowledge> { ... }

    /// Get patterns relevant to current work
    pub async fn patterns_for(&self, context: &ExecutionContext) -> Vec<SuccessPattern> { ... }
}
```

### 3.2 Task Queue with Deduplication

```rust
// crates/dashflow-nexus/src/task_queue.rs

pub struct TaskQueue {
    queue: Box<dyn DistributedQueue>,
    dedup: DeduplicationIndex,
}

#[derive(Debug)]
pub struct SwarmTask {
    pub id: TaskId,
    pub description: String,
    pub priority: Priority,
    pub dependencies: Vec<TaskId>,
    pub assigned_to: Option<InstanceId>,
    pub status: TaskStatus,
    pub dedup_key: String,  // For deduplication
}

impl TaskQueue {
    /// Submit task with deduplication
    pub async fn submit(&self, task: SwarmTask) -> Result<TaskId> {
        // Check if equivalent task already exists/completed
        if let Some(existing) = self.dedup.find_equivalent(&task.dedup_key).await? {
            return Ok(existing.id); // Return existing instead of creating duplicate
        }

        self.queue.push(task).await
    }

    /// Claim next available task for this instance
    pub async fn claim(&self, instance_id: &str) -> Option<SwarmTask> {
        self.queue.claim_for(instance_id).await
    }

    /// Mark task complete and share learnings
    pub async fn complete(&self, task_id: TaskId, result: TaskResult) -> Result<()> {
        self.queue.complete(task_id, &result).await?;

        // Extract and share learnings
        if let Some(knowledge) = result.extract_knowledge() {
            self.knowledge_base.share(knowledge).await?;
        }

        Ok(())
    }
}
```

### 3.3 Learning Propagation Bus

```rust
// crates/dashflow-nexus/src/learning_bus.rs

pub struct LearningBus {
    /// Kafka or similar for high-throughput messaging
    transport: Box<dyn MessageTransport>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum LearningEvent {
    /// Instance discovered a successful pattern
    PatternDiscovered {
        instance_id: String,
        pattern: SuccessPattern,
    },

    /// Instance found a pitfall
    PitfallFound {
        instance_id: String,
        pitfall: Pitfall,
    },

    /// Prompt was optimized
    PromptOptimized {
        instance_id: String,
        graph: String,
        node: String,
        improvement: f64,
    },

    /// Model comparison results
    ModelComparisonComplete {
        instance_id: String,
        results: Vec<ModelComparison>,
    },
}

impl LearningBus {
    /// Publish learning to all instances
    pub async fn publish(&self, event: LearningEvent) -> Result<()> {
        self.transport.send("learnings", event).await
    }

    /// Subscribe to learnings from other instances
    pub async fn subscribe(&self) -> impl Stream<Item = LearningEvent> {
        self.transport.subscribe("learnings").await
    }
}
```

---

## Part 4: Smart Optimizer Selection (BOTH PATTERNS)

### Current State (Post-Commits 850-853)
- 15 optimizers with academic citations (COMPLETE)
- `OPTIMIZER_GUIDE.md` created (COMPLETE)
- `dashflow introspect optimizers` CLI command (COMPLETE)
- `recommend_optimizer()` considers example count + finetune (COMPLETE)
- **MISSING: Hierarchical optimization levels**
- **MISSING: Automatic selection based on full task analysis**

---

## Part 4A: Hierarchical Optimization Levels (CRITICAL NEW)

### The Problem

Current DashOptimize only optimizes at **prompt level**. But real applications need optimization at multiple levels:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    OPTIMIZATION HIERARCHY                                   │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  PROMPT          NODE            SUBGRAPH        GRAPH          FLEET       │
│  (current)       (NEW)           (NEW)           (NEW)          (NEW)       │
│                                                                             │
│  Optimize        Optimize        Optimize        Optimize       Learn from  │
│  single LLM      one node's      subset of       full graph     all fleet   │
│  prompt          behavior        connected       structure      instances   │
│                                  nodes                                      │
│                                                                             │
│  Example:        Example:        Example:        Example:       Example:    │
│  "Summarize      Improve         RAG pipeline    Add/remove     "MIPROv2    │
│   this text"     retriever       (retrieve →     nodes, change  works 20%   │
│                  recall          grade → gen)    edges          better for  │
│                                                                 code tasks" │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Level 1: Prompt Optimization (CURRENT)
**What:** Optimize the prompt text for a single LLM call
**Optimizers:** All 15 current optimizers (MIPROv2, BootstrapFewShot, GRPO, etc.)
**Input:** Signature (prompt template) + training examples
**Output:** Better prompt text

```rust
// Current API - prompt level only
let optimizer = MIPROv2::new(metric);
let better_prompt = optimizer.compile(&signature, &trainset, llm).await?;
```

### Level 2: Node Optimization (NEW)
**What:** Optimize a single node's behavior within a graph
**Scope:** One node, including its prompt + any configuration
**Considerations:** Node's role in graph, upstream/downstream effects

```rust
// NEW: Node-level optimization
let optimizer = NodeOptimizer::new()
    .target_node("retriever")
    .metric(retrieval_recall_metric)
    .constraints(NodeConstraints {
        max_latency_ms: Some(500),
        max_cost_per_call: Some(0.01),
    });

let better_node = optimizer.optimize(&graph, &trainset).await?;
```

**What to optimize:**
- Node's prompt (if LLM node)
- Node's parameters (k for retrieval, temperature, etc.)
- Node's retry/timeout settings
- Node's model choice

### Level 3: Subgraph Optimization (NEW)
**What:** Optimize a connected subset of nodes together
**Scope:** Pipeline sections (e.g., RAG: retrieve → grade → generate)
**Why:** Nodes interact - optimizing together finds better combinations

```rust
// NEW: Subgraph-level optimization
let subgraph = graph.extract_subgraph(&["retriever", "grader", "generator"]);

let optimizer = SubgraphOptimizer::new()
    .metric(end_to_end_quality_metric)
    .search_space(SubgraphSearchSpace {
        allow_reordering: false,
        allow_node_removal: false,
        optimize_connections: true,
    });

let better_subgraph = optimizer.optimize(&subgraph, &trainset).await?;
```

### Level 4: Graph Optimization (NEW)
**What:** Optimize entire graph structure
**Scope:** All nodes, edges, branching, parallelism
**Operations:** Add/remove nodes, change edges, parallelize paths

```rust
// NEW: Graph-level optimization
let optimizer = GraphOptimizer::new()
    .metric(graph_quality_metric)
    .budget(OptimizationBudget {
        max_evals: 1000,
        max_time: Duration::from_secs(3600),
        max_cost: 50.0,
    })
    .search_space(GraphSearchSpace {
        allow_node_addition: true,  // Add quality gates, caches
        allow_node_removal: true,   // Remove unnecessary steps
        allow_parallelization: true,// Parallelize independent paths
        allow_edge_changes: true,   // Change routing
    });

let better_graph = optimizer.optimize(&graph, &trainset).await?;
```

### Level 5: App Optimization (NEW - Dash.ai specific)
**What:** Optimize across multiple graphs in an application
**Scope:** All graphs, shared components, routing between graphs
**Example:** Librarian has search graph, chat graph, analysis graph

```rust
// NEW: App-level optimization
let optimizer = AppOptimizer::new()
    .metric(app_quality_metric)
    .shared_components(&["embeddings", "llm_cache"]);

let better_app = optimizer.optimize(&app, &trainset).await?;
```

### Level 6: Fleet Optimization (NEW - Both patterns)
**What:** Learn from all fleet instances to improve future optimization
**Scope:** All instances across Dash.ai and Dasher deployments
**Data:** Aggregated eval results, optimizer outcomes, user feedback

```rust
// NEW: Fleet-level learning
impl Nexus {
    /// Learn from fleet-wide optimization outcomes
    pub async fn update_optimizer_priors(&self) -> Result<()> {
        // Aggregate outcomes from all instances
        let outcomes = self.eval_store.optimizer_outcomes(since: 7.days()).await?;

        // Update selection priors
        // "MIPROv2 works 20% better for code generation tasks"
        // "SIMBA outperforms on agent tasks with <50 examples"
        self.optimizer_selector.update_priors(outcomes).await
    }
}
```

### Integration with Introspection

```rust
// Unified optimization API with automatic level selection
impl DashFlowIntrospection {
    /// Optimize at the appropriate level
    pub async fn optimize(
        &self,
        target: OptimizationTarget,  // Prompt | Node | Subgraph | Graph | App
        trainset: &Dataset,
        constraints: OptimizationConstraints,
    ) -> Result<OptimizationResult> {
        // 1. Analyze target to determine optimal level
        let analysis = self.analyze_optimization_target(&target, trainset)?;

        // 2. Select optimizer for that level
        let optimizer = self.select_optimizer(&analysis)?;

        // 3. Run optimization
        let result = optimizer.optimize(&target, trainset).await?;

        // 4. Record outcome for fleet learning
        self.record_optimization_outcome(&result).await?;

        Ok(result)
    }
}
```

### CLI for Hierarchical Optimization

```bash
# Prompt level (current)
dashflow optimize --prompt summarizer.yaml --trainset data.jsonl

# Node level (NEW)
dashflow optimize --graph app.yaml --node retriever --trainset data.jsonl

# Subgraph level (NEW)
dashflow optimize --graph app.yaml --subgraph "retriever,grader,generator" --trainset data.jsonl

# Graph level (NEW)
dashflow optimize --graph app.yaml --trainset data.jsonl --allow-structure-changes

# App level (NEW)
dashflow optimize --app librarian/ --trainset data.jsonl

# Auto-select level (NEW)
dashflow optimize --auto --target app.yaml --trainset data.jsonl
# Output: "Analyzing... Recommending subgraph-level optimization for RAG pipeline"
```

---

## Part 4B: Automatic Selection Based on Task Analysis

```rust
// crates/dashflow/src/optimize/auto_select.rs

pub struct OptimizerSelector {
    /// Historical performance data from fleet
    fleet_data: Arc<FleetOptimizerData>,
}

#[derive(Debug)]
pub struct TaskAnalysis {
    /// Number of training examples
    pub num_examples: usize,

    /// Type of task
    pub task_type: TaskType,  // Classification, Generation, Extraction, etc.

    /// Data characteristics
    pub data_diversity: f64,      // 0-1, how diverse are examples
    pub avg_input_length: usize,
    pub avg_output_length: usize,

    /// Model capabilities
    pub can_finetune: bool,
    pub model_family: ModelFamily,

    /// Resource constraints
    pub max_eval_calls: Option<usize>,
    pub max_time_seconds: Option<u64>,
    pub max_cost_dollars: Option<f64>,
}

#[derive(Debug, Clone, Copy)]
pub enum TaskType {
    Classification,      // Fixed output categories
    Generation,          // Free-form text
    Extraction,          // Extract from input
    Transformation,      // Transform input format
    Reasoning,           // Multi-step reasoning
    CodeGeneration,      // Generate code
    Summarization,       // Condense text
}

impl OptimizerSelector {
    /// Analyze task and data to select best optimizer
    pub fn select(&self, analysis: &TaskAnalysis) -> OptimizerRecommendation {
        let mut candidates = Vec::new();

        // Rule-based selection
        match analysis.task_type {
            TaskType::Classification if analysis.num_examples >= 50 => {
                candidates.push(("MIPROv2", 0.9, "Best for classification with sufficient data"));
            }
            TaskType::CodeGeneration => {
                candidates.push(("SIMBA", 0.85, "Self-improving works well for code"));
            }
            TaskType::Reasoning => {
                candidates.push(("MIPROv2", 0.8, "Good for multi-step reasoning"));
                candidates.push(("COPRO", 0.7, "Instruction refinement helps reasoning"));
            }
            _ => {}
        }

        // Finetune path
        if analysis.can_finetune && analysis.num_examples >= 100 {
            candidates.push(("GRPO", 0.95, "Finetuning available with enough data"));
            candidates.push(("BootstrapFinetune", 0.85, "Distillation to smaller model"));
        }

        // Resource constraints
        if let Some(max_calls) = analysis.max_eval_calls {
            if max_calls < 100 {
                candidates.retain(|(name, _, _)| *name != "GEPA"); // GEPA needs many evals
            }
        }

        // Fleet data: what has worked for similar tasks?
        if let Some(fleet_best) = self.fleet_data.best_for_task_type(analysis.task_type) {
            candidates.push((fleet_best.optimizer, fleet_best.score, "Best performing in fleet"));
        }

        // Sort by score and return best
        candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        OptimizerRecommendation {
            recommended: candidates.first().map(|(n, _, _)| n.to_string())
                .unwrap_or_else(|| "BootstrapFewShot".to_string()),
            alternatives: candidates.iter().skip(1).take(2)
                .map(|(n, s, r)| Alternative { name: n.to_string(), score: *s, reason: r.to_string() })
                .collect(),
            reasoning: self.explain_selection(&candidates, analysis),
        }
    }

    /// Analyze a graph and its dataset to determine task type
    pub fn analyze_task(
        &self,
        graph: &CompiledGraph,
        dataset: &Dataset,
    ) -> TaskAnalysis {
        TaskAnalysis {
            num_examples: dataset.len(),
            task_type: self.infer_task_type(graph, dataset),
            data_diversity: self.compute_diversity(dataset),
            avg_input_length: dataset.avg_input_tokens(),
            avg_output_length: dataset.avg_output_tokens(),
            can_finetune: self.check_finetune_available(graph),
            model_family: self.detect_model_family(graph),
            max_eval_calls: None,
            max_time_seconds: None,
            max_cost_dollars: None,
        }
    }
}
```

### CLI: Auto Mode

```bash
# Current (manual)
dashflow optimize --graph graph.yaml --trainset train.jsonl --optimizer MIPROv2

# NEW: Auto-select (analyzes task and data)
dashflow optimize --graph graph.yaml --trainset train.jsonl --auto

# Show what would be selected without running
dashflow optimize --graph graph.yaml --trainset train.jsonl --auto --dry-run

# Output:
# Task Analysis:
#   Task type: CodeGeneration
#   Examples: 127
#   Data diversity: 0.73
#   Can finetune: No
#
# Recommended Optimizer: SIMBA
#   Reason: Self-improving works well for code generation
#
# Alternatives:
#   - MIPROv2 (0.8): Good for production with 100+ examples
#   - COPRO (0.7): Instruction refinement
#
# Run with --yes to proceed, or specify --optimizer to override
```

---

## Part 5: Production Serving Pattern (DASH.AI)

### Instance Lifecycle

```
User Request → Load Balancer → Spawn Instance → Process → Response → Terminate
                                    │
                                    ▼
                             ┌──────────────┐
                             │ Instance #N  │
                             │              │
                             │ - User state │
                             │ - Session ID │
                             │ - Telemetry  │──────▶ Nexus (aggregation only)
                             │              │
                             └──────────────┘
```

### Key Differences from Dasher

| Aspect | Dasher (Swarm) | Dash.ai (Serving) |
|--------|---------------|-------------------|
| Instance lifetime | Long-running | Per-session |
| Communication | P2P mesh | None (isolated) |
| State sharing | Yes (knowledge base) | No |
| Task coordination | Yes (dedup, deps) | No |
| Telemetry | Aggregated + P2P | Aggregated only |
| Scaling | Fixed pool | Auto-scale with load |

### Required Components

```rust
// crates/dashflow/src/serving/mod.rs

pub struct ServingConfig {
    /// Unique session identifier
    pub session_id: String,

    /// User identifier for telemetry grouping
    pub user_id: String,

    /// Instance identifier for fleet tracking
    pub instance_id: String,

    /// Where to send telemetry
    pub nexus_endpoint: String,

    /// A/B test variant assignment
    pub ab_variant: Option<String>,
}

pub struct ServingInstance {
    config: ServingConfig,
    graph: CompiledGraph,
    telemetry: TelemetryReporter,
}

impl ServingInstance {
    pub async fn new(config: ServingConfig, graph: CompiledGraph) -> Self {
        // Initialize telemetry with fleet labels
        let telemetry = TelemetryReporter::new(FleetLabels {
            instance_id: config.instance_id.clone(),
            deployment: "dash-ai".to_string(),
            ..Default::default()
        });

        Self { config, graph, telemetry }
    }

    /// Process user request
    pub async fn invoke(&self, input: Value) -> Result<Value> {
        let start = Instant::now();

        // Execute graph
        let result = self.graph.invoke(input).await;

        // Report telemetry to Nexus
        self.telemetry.report(EvalRecord {
            instance_id: self.config.instance_id.clone(),
            user_id: Some(self.config.user_id.clone()),
            ab_test_variant: self.config.ab_variant.clone(),
            latency_ms: start.elapsed().as_millis() as u64,
            ..result.metrics()
        }).await;

        result
    }
}
```

---

## Implementation Phases (REVISED after ULTRATHINK)

### Phase 1: Data Layer Foundation (15-20 commits)
**Required for any production deployment**

| # | Task | Commits |
|---|------|---------|
| 1.1 | PostgreSQL schema for operational data | 3 |
| 1.2 | Redis integration for hot data | 3 |
| 1.3 | ClickHouse schema for analytics | 3 |
| 1.4 | S3/GCS archival pipeline | 3 |
| 1.5 | Data layer abstraction traits | 3 |
| 1.6 | Migration tooling | 3 |

### Phase 2: Fleet Telemetry (10-15 commits)
**Both patterns need this**

| # | Task | Commits |
|---|------|---------|
| 2.1 | FleetLabels on all metrics | 3 |
| 2.2 | Prometheus remote_write | 2 |
| 2.3 | Fleet Grafana dashboards | 3 |
| 2.4 | Edge aggregation for 100k scale | 4 |
| 2.5 | Trace sampling | 3 |

### Phase 3: Nexus API Layer (12-15 commits)
**gRPC + REST for sync operations**

| # | Task | Commits |
|---|------|---------|
| 3.1 | nexus.proto definition | 2 |
| 3.2 | gRPC server implementation | 4 |
| 3.3 | REST gateway | 3 |
| 3.4 | Authentication (mTLS/JWT) | 3 |
| 3.5 | Rate limiting at API layer | 3 |

### Phase 4: Multi-Tenancy (15-20 commits)
**Required for Dash.ai production**

| # | Task | Commits |
|---|------|---------|
| 4.1 | OrganizationContext on all operations | 4 |
| 4.2 | Tenant isolation in Kafka (partitioning) | 3 |
| 4.3 | Tenant isolation in ClickHouse | 3 |
| 4.4 | Tenant isolation in Redis | 2 |
| 4.5 | Quota management | 4 |
| 4.6 | Billing/metering integration | 4 |

### Phase 5: Resilience Layer (10-12 commits)
**Required for 100k scale**

| # | Task | Commits |
|---|------|---------|
| 5.1 | Distributed rate limiting (Redis) | 3 |
| 5.2 | Circuit breakers | 3 |
| 5.3 | Backpressure handling | 3 |
| 5.4 | Graceful degradation | 3 |

### Phase 6: Optimization Infrastructure (12-15 commits)
**Smart optimizer + rollback**

| # | Task | Commits |
|---|------|---------|
| 6.1 | Optimization versioning | 4 |
| 6.2 | Canary deployments for prompts | 3 |
| 6.3 | Auto-rollback on regression | 3 |
| 6.4 | Model selection optimizer | 3 |
| 6.5 | `dashflow optimize --auto` | 2 |

### Phase 7: Hierarchical Optimization (15-20 commits)
**Beyond prompt level**

| # | Task | Commits |
|---|------|---------|
| 7.1 | Turn-level optimization | 4 |
| 7.2 | Node-level optimization | 3 |
| 7.3 | Subgraph optimization | 4 |
| 7.4 | Graph structure optimization | 5 |
| 7.5 | Session personalization | 4 |

### Phase 8: Collaboration Features (15-20 commits)
**Dasher swarm specific**

| # | Task | Commits |
|---|------|---------|
| 8.1 | Knowledge Base service | 4 |
| 8.2 | Task Queue with deduplication | 4 |
| 8.3 | Learning Propagation (Kafka topics) | 3 |
| 8.4 | Conflict resolution | 3 |
| 8.5 | Fleet-level learning | 4 |

### Phase 9: Compliance (10-12 commits)
**Enterprise requirements**

| # | Task | Commits |
|---|------|---------|
| 9.1 | Audit logging (immutable) | 4 |
| 9.2 | GDPR deletion workflow | 4 |
| 9.3 | Data residency controls | 4 |

---

## Total Effort Estimate (REVISED)

| Phase | Commits | Priority | Required For |
|-------|---------|----------|--------------|
| 1: Data Layer | 15-20 | **P0** | Any production |
| 2: Fleet Telemetry | 10-15 | **P0** | Any production |
| 3: Nexus API | 12-15 | **P0** | Any production |
| 4: Multi-Tenancy | 15-20 | **P0** | Dash.ai production |
| 5: Resilience | 10-12 | **P0** | 100k scale |
| 6: Optimization | 12-15 | **P1** | Smart optimization |
| 7: Hierarchy | 15-20 | **P1** | Advanced optimization |
| 8: Collaboration | 15-20 | **P1** | Dasher only |
| 9: Compliance | 10-12 | **P1** | Enterprise sales |
| **Total** | **115-150** | | |

**Note:** Original estimate was 45-60 commits. After ULTRATHINK, realistic estimate is **115-150 commits** for production-ready system at 100k scale.

---

## Success Criteria

### Fleet Telemetry
- [ ] Every metric has `instance_id` label
- [ ] Grafana shows per-instance breakdown
- [ ] Can query: "Which instance has highest error rate?"

### Eval Aggregation
- [ ] Quality regressions detected within 1 hour
- [ ] A/B tests show statistical significance
- [ ] Cost tracked per user/session

### Smart Optimizer
- [ ] `dashflow optimize --auto` selects appropriate optimizer
- [ ] Selection considers task type, not just example count
- [ ] Fleet data improves recommendations over time

### Dasher Nexus
- [ ] Instances share learnings in real-time
- [ ] Tasks not duplicated across swarm
- [ ] Conflicts resolved automatically

### Dash.ai Serving
- [ ] Instances are truly isolated
- [ ] Telemetry aggregates correctly
- [ ] Can A/B test across user cohorts

---

## Architecture Decisions (ANSWERED)

### 1. Storage for Eval Data: ClickHouse vs TimescaleDB vs InfluxDB

**Recommendation: ClickHouse**

| System | Strengths | Weaknesses | Fit |
|--------|-----------|------------|-----|
| **ClickHouse** | Fastest for analytics, excellent compression, SQL-like | Complex to operate, not great for updates | **BEST** for high-cardinality eval metrics |
| **TimescaleDB** | PostgreSQL compatible, good for time-series | Slower than ClickHouse at scale | Good if already using Postgres |
| **InfluxDB** | Purpose-built for metrics | Limited query language, expensive | Better for simple metrics |

**Why ClickHouse:**
- 100k+ concurrent Dash.ai sessions = billions of eval records/day
- High cardinality: instance_id × user_id × model × prompt_version
- Analytical queries: "What's the p95 latency for GPT-4 on code tasks this week?"
- Compression: 10-20x reduction (critical at this scale)
- Materialized views for real-time aggregation

```sql
-- Example: Quality by model, task type (ClickHouse)
SELECT
    model,
    task_type,
    quantile(0.95)(latency_ms) as p95_latency,
    avg(quality_score) as avg_quality,
    count() as total_requests
FROM eval_records
WHERE timestamp > now() - INTERVAL 1 DAY
GROUP BY model, task_type
ORDER BY avg_quality DESC
```

### 2. Transport: Kafka Only (Simplified)

**Recommendation: Kafka for everything** (already in stack, no need for second system)

| Requirement | Kafka Can Handle? | How |
|-------------|-------------------|-----|
| High throughput (1M+ msg/sec) | Yes | Partitioning, batching |
| Low latency (<50ms) | Yes | `linger.ms=0`, `acks=1` for non-critical |
| Durability | Yes | Replication, retention |
| P2P collaboration | Yes | Dedicated topic with consumer groups |

**Why NOT add NATS:**
- Kafka is already in the DashFlow stack
- Two message systems = 2x operational complexity
- Modern Kafka 3.x has improved latency
- One team to train, one system to monitor

**Architecture (Simplified):**
```
┌─────────────────────────────────────────────────────────────────┐
│                    TRANSPORT LAYER (Kafka Only)                 │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  Topics by use case:                                            │
│                                                                 │
│  dashstream-telemetry     High volume, can lose some (acks=1)  │
│  dashstream-evals         Durable, analytics (acks=all)        │
│  dashstream-learning      P2P learnings (low latency config)   │
│  dashstream-tasks         Task queue (exactly-once)            │
│  dashstream-dlq           Dead letter queue                    │
│                                                                 │
│  Partitioning strategy:                                         │
│  - telemetry: by instance_id (parallelism)                     │
│  - evals: by org_id (tenant isolation)                         │
│  - learning: by topic (broadcast to all)                       │
│  - tasks: by task_type (work distribution)                     │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

**Kafka tuning for different needs:**
```properties
# High throughput (telemetry)
linger.ms=5
batch.size=65536
acks=1

# Low latency (learning, tasks)
linger.ms=0
batch.size=1
acks=1

# Durability (evals)
acks=all
min.insync.replicas=2
```

### 3. Scale Requirements (CLARIFIED)

| Deployment | Concurrent Instances | Messages/sec | Evals/day |
|------------|---------------------|--------------|-----------|
| **Dasher** | Dozens (10-50) | 100s | 10k-100k |
| **Dash.ai** | 100k+ | 1M+ | 10B+ |

**Implications for Architecture:**

```
┌─────────────────────────────────────────────────────────────────┐
│                    SCALE TIERS                                  │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  TIER 1: Dev/Test (1-10 instances)                             │
│  - Single Prometheus, single Kafka broker                       │
│  - SQLite for evals                                            │
│  - In-memory channels (no external messaging)                  │
│                                                                 │
│  TIER 2: Dasher Production (10-100 instances)                  │
│  - Prometheus with 15s scrape                                  │
│  - Kafka cluster (3 brokers)                                   │
│  - ClickHouse single node                                      │
│  - Low-latency Kafka topics (linger.ms=0)                      │
│                                                                 │
│  TIER 3: Dash.ai Production (1k-100k instances)                │
│  - Prometheus federation + Thanos                              │
│  - Kafka cluster (10+ brokers, partitioned by instance)        │
│  - ClickHouse cluster (sharded by date)                        │
│  - Sampling: 1% of traces at 100k scale                        │
│  - Pre-aggregation in edge collectors                          │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

**100k Scale Specific Requirements:**

1. **Edge Aggregation** - Instances aggregate locally before sending to Nexus
   ```rust
   // Don't send every eval - aggregate locally first
   pub struct EdgeAggregator {
       buffer: Vec<EvalRecord>,
       flush_interval: Duration,  // 10s at 100k scale
   }

   impl EdgeAggregator {
       pub async fn add(&mut self, eval: EvalRecord) {
           self.buffer.push(eval);
           if self.buffer.len() >= 100 || self.last_flush.elapsed() > self.flush_interval {
               self.flush().await;
           }
       }

       async fn flush(&mut self) {
           // Send aggregated stats, not individual records
           let summary = self.buffer.summarize();
           self.nexus.report_summary(summary).await;
           self.buffer.clear();
       }
   }
   ```

2. **Sampling** - Sample traces at high scale
   ```rust
   pub struct TraceSampler {
       rate: f64,  // 0.01 = 1% at 100k scale
   }

   impl TraceSampler {
       pub fn should_sample(&self, trace_id: &str) -> bool {
           // Deterministic sampling based on trace_id
           let hash = hash(trace_id);
           (hash as f64 / u64::MAX as f64) < self.rate
       }
   }
   ```

3. **Sharding** - Partition by instance_id for parallel processing
   ```sql
   -- ClickHouse table with sharding
   CREATE TABLE eval_records ON CLUSTER cluster
   (
       timestamp DateTime,
       instance_id String,
       ...
   ) ENGINE = ReplicatedMergeTree('/clickhouse/tables/{shard}/eval_records', '{replica}')
   PARTITION BY toYYYYMMDD(timestamp)
   ORDER BY (instance_id, timestamp)
   ```

### 4. Consensus for Coordination

**Dasher:** Simple leader election via Kafka consumer groups
- Consumer group rebalancing handles leader election
- Kafka transactions for exactly-once task claiming
- No need for separate etcd/Zookeeper beyond what Kafka uses

**Dash.ai:** No consensus needed (isolated instances)
- Instances don't coordinate with each other
- Nexus is single source of truth for aggregation

---

## ULTRATHINK: Critical Production Gaps

The following gaps were identified through deep analysis and MUST be addressed before 100k scale production.

### Gap 1: Missing Data Layers (CRITICAL)

**Problem:** ClickHouse alone is insufficient. Need multi-tier storage:

```
┌─────────────────────────────────────────────────────────────────┐
│                    REQUIRED DATA LAYERS                         │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  HOT (sub-10ms) - Redis/KeyDB                                  │
│  ├── Active session state                                       │
│  ├── Last 5 minutes of metrics                                 │
│  ├── Real-time dashboards                                       │
│  └── Distributed rate limiting tokens                          │
│                                                                 │
│  OPERATIONAL (ACID) - PostgreSQL                               │
│  ├── Task queue state (claim/release needs transactions)       │
│  ├── Lock state                                                 │
│  ├── User accounts and organizations                           │
│  ├── Optimization versions                                      │
│  └── Audit logs                                                 │
│                                                                 │
│  ANALYTICAL (batch) - ClickHouse                               │
│  ├── Historical evals                                           │
│  ├── Aggregated telemetry                                       │
│  ├── Quality trends                                             │
│  └── Cost analytics                                             │
│                                                                 │
│  COLD (archive) - S3/GCS                                       │
│  ├── Traces older than 30 days                                 │
│  ├── Full replay capability                                     │
│  └── Compliance archives                                        │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### Gap 2: Synchronous API Layer (CRITICAL)

**Problem:** Pub/sub doesn't work for request-response operations.

**Required:** gRPC or REST API for synchronous operations:

```protobuf
// nexus.proto - MUST ADD
service NexusService {
    // Synchronous (gRPC)
    rpc ClaimTask(ClaimRequest) returns (TaskResponse);
    rpc AcquireLock(LockRequest) returns (LockResponse);
    rpc HealthCheck(HealthRequest) returns (HealthResponse);
    rpc SelectOptimizer(TaskAnalysis) returns (OptimizerRecommendation);

    // Streaming (can use Kafka underneath)
    rpc StreamEvals(stream EvalRecord) returns (Ack);
}
```

### Gap 3: Multi-Tenancy (CRITICAL for Dash.ai)

**Problem:** Cannot serve multiple organizations without tenant isolation.

```rust
// MUST ADD: Organization context on every operation
pub struct OrganizationContext {
    pub org_id: String,
    pub tenant_id: String,
    pub tier: BillingTier,      // Free, Pro, Enterprise
    pub quotas: OrgQuotas,
    pub data_residency: DataRegion,
    pub feature_flags: Vec<String>,
}

// Data isolation via Kafka partitioning + ClickHouse filtering
// - Kafka: partition by org_id
// - ClickHouse: org_id in every table, row-level security
// - Redis: key prefix by org_id
```

### Gap 4: Billing and Metering (CRITICAL for monetization)

```rust
// MUST ADD: Usage metering
pub struct MeterEvent {
    pub org_id: String,
    pub user_id: String,
    pub event_type: MeterType,  // TokensUsed, LLMCall, StorageBytes
    pub quantity: u64,
    pub cost_cents: u64,
    pub timestamp: DateTime<Utc>,
}

// Integration: Stripe usage-based billing, Orb, or custom
```

### Gap 5: Distributed Rate Limiting (CRITICAL at scale)

**Problem:** In-memory rate limiters don't work across pods.

```rust
// MUST ADD: Redis-backed rate limiter
pub struct DistributedRateLimiter {
    redis: redis::Client,
}

impl DistributedRateLimiter {
    pub async fn acquire(&self, key: &str, tokens: u64, window: Duration) -> Result<bool> {
        // Token bucket using Redis INCR + EXPIRE
    }
}
```

### Gap 6: Backpressure and Circuit Breakers (CRITICAL)

**Problem:** At 100k scale, overload WILL happen.

```rust
// MUST ADD: Resilience layer
pub struct ResilienceConfig {
    circuit_breakers: HashMap<Service, CircuitBreaker>,
    bulkheads: HashMap<Service, Semaphore>,
    rate_limits: DistributedRateLimiter,
    retry_config: RetryConfig,
}

// Instance behavior when Nexus is overloaded:
// 1. Buffer locally (up to X MB)
// 2. Exponential backoff with jitter
// 3. Drop low-priority telemetry
// 4. Keep high-priority (errors, critical events)
```

### Gap 7: Optimization Rollback (HIGH)

**Problem:** Bad optimizations can cause regressions. Need versioning and rollback.

```rust
// MUST ADD: Optimization versioning
pub struct OptimizationVersion {
    pub id: Uuid,
    pub graph_id: String,
    pub node_id: String,
    pub version: u32,
    pub prompt_hash: String,
    pub quality_score: f64,
    pub traffic_percentage: f64,  // Canary support
    pub status: OptimizationStatus,
}

// Auto-rollback on quality regression
pub async fn auto_rollback_check(&self) -> Result<Vec<RollbackEvent>> {
    // If quality drops >10% in last 5 min, rollback
}
```

### Gap 8: GDPR/Compliance (HIGH for EU)

```rust
// MUST ADD: Data retention and deletion
pub struct DataRetention {
    user_data_retention: Duration,     // 90 days default
    aggregate_retention: Duration,     // 3 years
    deletion_queue: DeletionQueue,     // GDPR right to be forgotten
}

// ClickHouse DELETE is expensive - plan for it:
// - Partition by date for easy drops
// - User data in separate tables
// - Anonymization vs deletion tradeoffs
```

### Gap 9: Expanded Optimization Hierarchy (from ULTRATHINK)

**Current hierarchy is missing 4 levels:**

```
COMPLETE HIERARCHY (10 levels):

1. MESSAGE      - Optimize structure within a single LLM message
2. PROMPT       - Optimize full prompt (current - 15 optimizers work here)
3. TURN         - Optimize multi-turn strategy (when to use tools, etc.)
4. NODE         - Optimize node configuration (Plan Level 2)
5. CONVERSATION - Optimize state flow, memory management
6. SUBGRAPH     - Optimize connected nodes together (Plan Level 3)
7. GRAPH        - Optimize graph structure (Plan Level 4)
8. SESSION      - Optimize across user sessions (personalization)
9. APP          - Optimize across multiple graphs (Plan Level 5)
10. FLEET       - Cross-instance learning (Plan Level 6)
```

**Also missing: Model Selection as cross-cutting concern**
- FrugalGPT pattern: Try cheap model first, escalate if needed
- 10-100x cost savings possible

### Gap 10: Audit Logging (HIGH for enterprise)

```rust
// MUST ADD: Tamper-proof audit trail
pub struct AuditEvent {
    pub timestamp: DateTime<Utc>,
    pub actor: Actor,
    pub resource: Resource,
    pub action: Action,
    pub outcome: Outcome,
    pub ip_address: Option<IpAddr>,
    pub request_id: String,
}

// Store in immutable storage (S3/BigQuery with object lock)
```

---

## Framework vs Application Clarification

This plan describes **DashFlow framework features**, not the Dasher application:

| Layer | What | Who Builds |
|-------|------|------------|
| **DashFlow Framework** | Fleet telemetry, Nexus services, optimizer hierarchy | This plan |
| **Dasher App** | Claude Code-like autonomous dev | Built ON DashFlow |
| **Dash.ai App** | ChatGPT for Dropbox | Built ON DashFlow |

**Framework APIs exposed:**
```rust
// What DashFlow provides:
dashflow::nexus::Nexus                    // Central coordination
dashflow::nexus::FleetTelemetry           // Instance-labeled metrics
dashflow::nexus::EvalAggregator           // Cross-instance evals
dashflow::nexus::KnowledgeBase            // Shared learnings
dashflow::nexus::TaskQueue                // Distributed task queue
dashflow::optimize::HierarchicalOptimizer // Multi-level optimization
dashflow::serving::ServingInstance        // Production serving wrapper

// What apps build:
// Dasher: Uses TaskQueue, KnowledgeBase, P2P networking
// Dash.ai: Uses ServingInstance, EvalAggregator, FleetTelemetry
```
