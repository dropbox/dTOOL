# Self-Improving Introspection System

**Version:** 1.3
**Date:** 2025-12-10
**Priority:** P0 - Core AI Capability
**Status:** COMPLETE - All Phases (1-6) Complete
**Prerequisite:** ROADMAP_TELEMETRY_UNIFICATION.md - ✅ Phases 1-2 Complete, Phases 3-4 in progress

---

## Executive Summary

A structured system for AI self-improvement through introspection, multi-model consensus,
hypothesis tracking, and meta-analysis. The AI examines its own execution, identifies
improvements, validates with external models, generates execution plans, and learns
from the history of its own improvement attempts.

---

## Design Principle: Full Opt-In by Default, Opt-Out Only

**Core Principle:** Everything is ON by default. Users disable what they don't want.
DashFlow is maximally helpful out of the box.

### All Features ON by Default

| Feature | Default | Opt-Out |
|---------|---------|---------|
| ExecutionTrace collection | **ON** | `.without_tracing()` |
| Local analysis (gaps, deprecation) | **ON** | `.without_analysis()` |
| Pattern detection | **ON** | `.without_patterns()` |
| Hypothesis tracking | **ON** | `.without_hypotheses()` |
| Meta-analysis | **ON** | `.without_meta_analysis()` |
| Plan generation | **ON** | `.without_plans()` |
| File storage (.dashflow/) | **ON** | `.without_storage()` |
| Multi-model consensus* | **ON** | `.without_consensus()` |
| Dasher plan generation** | **ON** | `.without_dasher()` |

*Runs automatically if API keys detected in environment
**Plans generated but NEVER executed without explicit human approval

### Example Usage

```rust
// Everything works automatically - no configuration
let app = graph.compile()?;
// - ExecutionTrace: collected
// - Analysis: running
// - Patterns: detected
// - Plans: generated
// - Consensus: running (if API keys present)
// - Storage: .dashflow/introspection/

// Disable specific features you don't want
let app = graph
    .compile()?
    .without_consensus()    // Don't call external AIs
    .without_storage();     // Don't write to disk

// Minimal mode - disable everything
let app = graph
    .compile()?
    .without_self_improvement();  // Just run the graph
```

### API Key Auto-Detection

```rust
// Multi-model consensus activates automatically when keys are present:
// - ANTHROPIC_API_KEY → Claude models
// - OPENAI_API_KEY → GPT models
// - GOOGLE_API_KEY → Gemini models
//
// If no keys present, consensus is silently skipped (no error).
// This means: set your keys, get consensus. No config needed.
```

### The Only Gate: Plan Execution Approval

```rust
// Dasher generates improvement plans automatically.
// But plans are NEVER executed without explicit human approval.
//
// This is the only "gate" in the entire system.
// Everything else runs automatically.
//
// Approval flow:
// 1. Plan generated → stored in .dashflow/introspection/plans/pending/
// 2. Human reviews plan
// 3. Human runs: `dashflow approve-plan <plan-id>`
// 4. Dasher implements the plan
```

---

## Core Questions the System Answers

### 1. Capability Gap Analysis
> "What functions (nodes) would have made me more efficient? What do I need that I don't have?
> Of the functionality I have now, how should it be modified to improve it and why?"

### 2. Deprecation Analysis
> "What do I have that is extraneous and can be deprecated, removed, deleted?
> Does removal or simplification gain me anything?"

### 3. Retrospective Analysis
> "What should I have done differently? What tools and systems would have made me
> more efficient in this application? In this specific task? As a DashFlow self-aware AI?"

### 4. Multi-Model Consensus
> "What do other AIs think? Ask them to be skeptical, rigorous, and judge my suggestions.
> Prefer AIs with different cores (Gemini, OpenAI, Anthropic) to catch biases.
> Do I believe this feedback? I am rigorous and open but also confident."

### 5. Execution Planning
> "Generate an execution plan for: optimizations, application-level improvements,
> and DashFlow platform improvements. Run periodically or on statistical triggers."

### 6. Meta-Analysis
> "Inspect history of improvement plans. Assess patterns, momentum, dead ends.
> Generate hypotheses about what will happen next. Defend with expected data.
> Review past hypotheses - was I correct? Why? How to improve hypotheses?"

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         SELF-IMPROVEMENT LOOP                                │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌──────────────┐     ┌──────────────┐     ┌──────────────┐                │
│  │  Execution   │────→│ Introspection│────→│   Analysis   │                │
│  │   Traces     │     │   Collector  │     │    Engine    │                │
│  └──────────────┘     └──────────────┘     └──────┬───────┘                │
│                                                    │                         │
│                                      ┌─────────────┴─────────────┐          │
│                                      ▼                           ▼          │
│                          ┌──────────────────┐       ┌──────────────────┐   │
│                          │ Capability Gap   │       │   Deprecation    │   │
│                          │    Analysis      │       │    Analysis      │   │
│                          └────────┬─────────┘       └────────┬─────────┘   │
│                                   │                          │              │
│                                   └──────────┬───────────────┘              │
│                                              ▼                              │
│                               ┌──────────────────────────┐                 │
│                               │  Improvement Proposals   │                 │
│                               │  (ImprovementPlan)       │                 │
│                               └────────────┬─────────────┘                 │
│                                            │                                │
│                                            ▼                                │
│                               ┌──────────────────────────┐                 │
│                               │  Multi-Model Consensus   │                 │
│                               │  - Gemini Review         │                 │
│                               │  - OpenAI Review         │                 │
│                               │  - Claude Self-Review    │                 │
│                               └────────────┬─────────────┘                 │
│                                            │                                │
│                                            ▼                                │
│                               ┌──────────────────────────┐                 │
│                               │   Validated Plans        │                 │
│                               │   (ExecutionPlan)        │                 │
│                               └────────────┬─────────────┘                 │
│                                            │                                │
│              ┌─────────────────────────────┼─────────────────────────────┐  │
│              ▼                             ▼                             ▼  │
│  ┌─────────────────┐           ┌─────────────────┐          ┌──────────────┐
│  │  Dasher Agent   │           │   Meta-Analysis │          │   History    │
│  │  (Implements)   │           │   & Hypotheses  │          │   Storage    │
│  └─────────────────┘           └─────────────────┘          └──────────────┘
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Data Structures

### IntrospectionReport

```rust
/// A complete introspection report for one analysis cycle
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntrospectionReport {
    /// Unique identifier for this report
    pub id: Uuid,

    /// When this analysis was performed
    pub timestamp: DateTime<Utc>,

    /// Scope of analysis
    pub scope: IntrospectionScope,

    /// Statistical summary of analyzed executions
    pub execution_summary: ExecutionSummary,

    /// Capability gap analysis results
    pub capability_gaps: Vec<CapabilityGap>,

    /// Deprecation recommendations
    pub deprecations: Vec<DeprecationRecommendation>,

    /// Retrospective insights
    pub retrospective: RetrospectiveAnalysis,

    /// Generated improvement proposals (pre-validation)
    pub proposals: Vec<ImprovementProposal>,

    /// Multi-model consensus results
    pub consensus: Option<ConsensusResult>,

    /// Final validated execution plans
    pub execution_plans: Vec<ExecutionPlan>,

    /// Hypotheses about future outcomes
    pub hypotheses: Vec<Hypothesis>,

    /// Links to source data (execution traces, logs)
    pub citations: Vec<Citation>,

    /// Previous report ID for chain tracking
    pub previous_report_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IntrospectionScope {
    /// Single graph execution
    Execution { thread_id: String },
    /// Multiple executions of same graph
    GraphAggregate { graph_id: String, execution_count: usize },
    /// Time-based window
    TimeWindow { start: DateTime<Utc>, end: DateTime<Utc> },
    /// Periodic analysis (e.g., every N executions)
    Periodic { period: usize, iteration: usize },
    /// Full system analysis
    System,
}
```

### CapabilityGap

```rust
/// Analysis of missing or needed functionality
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityGap {
    /// What capability is missing or needed
    pub description: String,

    /// Evidence from execution traces
    pub evidence: Vec<Citation>,

    /// How this gap manifested (errors, retries, workarounds)
    pub manifestation: GapManifestation,

    /// Proposed solution
    pub proposed_solution: String,

    /// Expected impact if addressed
    pub expected_impact: Impact,

    /// Confidence in this analysis (0.0-1.0)
    pub confidence: f64,

    /// Category of gap
    pub category: GapCategory,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GapCategory {
    /// Missing node type
    MissingNode { suggested_signature: String },
    /// Missing tool
    MissingTool { tool_description: String },
    /// Inadequate existing functionality
    InadequateFunctionality { node: String, limitation: String },
    /// Missing integration
    MissingIntegration { external_system: String },
    /// Performance limitation
    PerformanceGap { bottleneck: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GapManifestation {
    /// Explicit errors
    Errors { count: usize, sample_messages: Vec<String> },
    /// High retry rates
    Retries { rate: f64, affected_nodes: Vec<String> },
    /// Manual workarounds in prompts
    PromptWorkarounds { patterns: Vec<String> },
    /// Suboptimal execution paths
    SuboptimalPaths { description: String },
    /// Missing data or context
    MissingContext { what: String },
}
```

### DeprecationRecommendation

```rust
/// Recommendation to remove or simplify functionality
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeprecationRecommendation {
    /// What to deprecate
    pub target: DeprecationTarget,

    /// Why it's extraneous
    pub rationale: String,

    /// Evidence of non-use or redundancy
    pub evidence: Vec<Citation>,

    /// What gains from removal
    pub benefits: Vec<String>,

    /// Risks of removal
    pub risks: Vec<String>,

    /// Confidence in recommendation (0.0-1.0)
    pub confidence: f64,

    /// Suggested migration path if any dependencies exist
    pub migration_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeprecationTarget {
    Node { name: String, usage_count: usize },
    Tool { name: String, last_used: Option<DateTime<Utc>> },
    Edge { from: String, to: String },
    Feature { name: String },
    CodePath { location: String },
}
```

### RetrospectiveAnalysis

```rust
/// What should have been done differently
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrospectiveAnalysis {
    /// What actually happened
    pub actual_execution: ExecutionSummary,

    /// What would have been better (counterfactuals)
    pub counterfactuals: Vec<Counterfactual>,

    /// Tools/systems that would have helped
    pub missing_tools: Vec<MissingToolAnalysis>,

    /// Application-specific insights
    pub application_insights: Vec<String>,

    /// Task-specific insights
    pub task_insights: Vec<String>,

    /// Platform-level insights (DashFlow improvements)
    pub platform_insights: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Counterfactual {
    /// What could have been done differently
    pub alternative: String,

    /// Expected outcome if alternative was taken
    pub expected_outcome: String,

    /// Why this wasn't done originally
    pub why_not_taken: String,

    /// Confidence that alternative would be better
    pub confidence: f64,
}
```

### Multi-Model Consensus

```rust
/// Results from consulting multiple AI models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusResult {
    /// Reviews from different models
    pub reviews: Vec<ModelReview>,

    /// Aggregated consensus score
    pub consensus_score: f64,

    /// Points of agreement
    pub agreements: Vec<String>,

    /// Points of disagreement
    pub disagreements: Vec<Disagreement>,

    /// Final synthesized judgment
    pub synthesis: String,

    /// Whether the original proposals were validated
    pub validated: bool,

    /// Modifications suggested by consensus
    pub modifications: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelReview {
    /// Which model provided this review
    pub model: ModelIdentifier,

    /// Overall assessment
    pub assessment: Assessment,

    /// Specific critiques
    pub critiques: Vec<Critique>,

    /// Suggestions for improvement
    pub suggestions: Vec<String>,

    /// Confidence in review
    pub confidence: f64,

    /// Raw response for transparency
    pub raw_response: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ModelIdentifier {
    Anthropic { model: String },  // e.g., "claude-3-opus"
    OpenAI { model: String },     // e.g., "gpt-4"
    Google { model: String },     // e.g., "gemini-pro"
    Other { provider: String, model: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Assessment {
    StronglyAgree,
    Agree,
    Neutral,
    Disagree,
    StronglyDisagree,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Critique {
    /// What is being critiqued
    pub target: String,

    /// The critique
    pub criticism: String,

    /// Severity
    pub severity: CritiqueSeverity,

    /// Suggested fix
    pub suggested_fix: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CritiqueSeverity {
    Minor,
    Moderate,
    Major,
    Critical,
}
```

### ExecutionPlan

```rust
/// A validated plan ready for implementation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPlan {
    /// Unique identifier
    pub id: Uuid,

    /// Human-readable title
    pub title: String,

    /// Detailed description
    pub description: String,

    /// Category of improvement
    pub category: PlanCategory,

    /// Priority (1 = highest)
    pub priority: u8,

    /// Estimated effort in AI commits
    pub estimated_commits: u8,

    /// Steps to implement
    pub steps: Vec<ImplementationStep>,

    /// Success criteria
    pub success_criteria: Vec<String>,

    /// Rollback plan if it fails
    pub rollback_plan: String,

    /// Dependencies on other plans
    pub dependencies: Vec<Uuid>,

    /// Citations to supporting evidence
    pub citations: Vec<Citation>,

    /// Consensus validation score
    pub validation_score: f64,

    /// Status tracking
    pub status: PlanStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PlanCategory {
    /// DashOpt optimization (prompts, demos, model selection)
    Optimization,
    /// Application-level code changes
    ApplicationImprovement,
    /// DashFlow platform improvements
    PlatformImprovement,
    /// Documentation or process improvements
    ProcessImprovement,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImplementationStep {
    /// Step number
    pub order: u8,

    /// What to do
    pub action: String,

    /// Files to modify
    pub files: Vec<String>,

    /// Verification method
    pub verification: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PlanStatus {
    Proposed,
    Validated,
    InProgress { started: DateTime<Utc>, assignee: String },
    Implemented { completed: DateTime<Utc>, commit_hash: String },
    Failed { reason: String },
    Superseded { by: Uuid },
}
```

### Hypothesis Tracking

```rust
/// A hypothesis about future outcomes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hypothesis {
    /// Unique identifier
    pub id: Uuid,

    /// The hypothesis statement
    pub statement: String,

    /// Reasoning behind the hypothesis
    pub reasoning: String,

    /// What data would validate this hypothesis
    pub expected_evidence: Vec<ExpectedEvidence>,

    /// When to evaluate this hypothesis
    pub evaluation_trigger: EvaluationTrigger,

    /// Current status
    pub status: HypothesisStatus,

    /// If evaluated, the outcome
    pub outcome: Option<HypothesisOutcome>,

    /// Lessons learned from this hypothesis
    pub lessons: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpectedEvidence {
    /// What metric or observation
    pub metric: String,

    /// Expected value or range
    pub expected_value: String,

    /// How to measure
    pub measurement_method: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EvaluationTrigger {
    /// After N executions
    AfterExecutions(usize),
    /// After time period
    AfterTime(Duration),
    /// After specific plan is implemented
    AfterPlan(Uuid),
    /// Manual trigger
    Manual,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HypothesisStatus {
    Active,
    Pending { waiting_for: String },
    Evaluated,
    Superseded { by: Uuid },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HypothesisOutcome {
    /// Was the hypothesis correct?
    pub correct: bool,

    /// Actual observed evidence
    pub observed_evidence: Vec<ObservedEvidence>,

    /// Analysis of why correct/incorrect
    pub analysis: String,

    /// What to do differently next time
    pub improvements_for_future: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservedEvidence {
    /// What was measured
    pub metric: String,

    /// What was observed
    pub observed_value: String,

    /// Matches expectation?
    pub matches_expected: bool,

    /// Citation to source data
    pub citation: Citation,
}
```

### Citation System

```rust
/// Reference to source data for traceability
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Citation {
    /// Citation identifier (e.g., "[1]", "[trace-abc123]")
    pub id: String,

    /// Type of source
    pub source_type: CitationSource,

    /// Human-readable description
    pub description: String,

    /// How to retrieve the source data
    pub retrieval: CitationRetrieval,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CitationSource {
    /// Reference to an ExecutionTrace
    ExecutionTrace { thread_id: String, timestamp: DateTime<Utc> },

    /// Aggregated statistics
    Aggregation { query: String, result_summary: String },

    /// Previous introspection report
    IntrospectionReport { report_id: Uuid },

    /// Git commit
    GitCommit { hash: String, message_summary: String },

    /// External AI review
    ModelReview { model: ModelIdentifier, timestamp: DateTime<Utc> },

    /// Log file
    LogFile { path: String, line_range: Option<(usize, usize)> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CitationRetrieval {
    /// Retrieve from ExecutionTrace storage
    TraceStorage { query: String },

    /// Retrieve from git
    Git { command: String },

    /// Retrieve from file
    File { path: String },

    /// Inline data (for small citations)
    Inline { data: String },
}
```

---

## Storage Structure

All introspection data is stored in git for history tracking:

```
.dashflow/
├── introspection/
│   ├── reports/
│   │   ├── 2025-12-09T15-30-00_abc123.md    # Human-readable report
│   │   ├── 2025-12-09T15-30-00_abc123.json  # Machine-readable data
│   │   └── ...
│   ├── plans/
│   │   ├── active/
│   │   │   ├── plan_001_capability_gap.md
│   │   │   └── plan_002_deprecation.md
│   │   ├── implemented/
│   │   │   └── plan_000_initial.md
│   │   └── failed/
│   │       └── ...
│   ├── hypotheses/
│   │   ├── active/
│   │   │   └── hyp_001_retry_reduction.md
│   │   └── evaluated/
│   │       └── hyp_000_initial.md
│   ├── consensus/
│   │   └── 2025-12-09T15-30-00_reviews.md
│   └── meta/
│       ├── patterns.md          # Recurring patterns observed
│       ├── momentum.md          # Improvement velocity tracking
│       ├── dead_ends.md         # Approaches that didn't work
│       └── design_notes.md      # Notes for future iterations
```

### Report Format (Markdown)

```markdown
# Introspection Report: 2025-12-09T15:30:00

**Scope:** GraphAggregate (customer_service_bot, 47 executions)
**Previous Report:** [2025-12-08T12:00:00](./2025-12-08T12-00-00_def456.md)

## Executive Summary

Analysis of 47 executions revealed 3 capability gaps, 1 deprecation candidate,
and generated 2 validated execution plans. Multi-model consensus achieved 0.85
agreement score.

## Execution Statistics

| Metric | Value | vs Previous |
|--------|-------|-------------|
| Total Executions | 47 | +12 |
| Success Rate | 89.4% | +2.1% |
| Avg Duration | 3.2s | -0.4s |
| Retry Rate | 8.5% | -1.2% |

## Capability Gap Analysis

### Gap 1: Missing sentiment analysis tool [HIGH]

**Evidence:** [trace-001], [trace-017], [trace-034]

The agent repeatedly attempts to infer customer sentiment from text patterns
using heuristics in the prompt. This is error-prone and adds 200ms latency.

**Manifestation:** Prompt workarounds detected in 34% of executions:
> "Based on the customer's word choice and punctuation..."

**Proposed Solution:** Add `SentimentAnalysisTool` node with pre-trained model.

**Expected Impact:**
- Reduce retry rate by ~3%
- Reduce latency by ~200ms per execution
- Improve sentiment accuracy from ~70% to ~95%

**Confidence:** 0.85

---

## Deprecation Recommendations

### Deprecate: `legacy_greeting_node` [MEDIUM]

**Evidence:** [aggregation-001]

This node was executed 0 times in the last 47 executions. The `greeting_router`
node handles all greeting logic.

**Benefits:**
- Remove 127 lines of dead code
- Simplify graph structure

**Risks:**
- None identified (no callers)

**Confidence:** 0.95

---

## Multi-Model Consensus

### Reviewers
- Claude 3.5 Sonnet (self-review)
- GPT-4 Turbo
- Gemini Pro

### Assessment Summary

| Model | Gap 1 | Deprecation 1 | Overall |
|-------|-------|---------------|---------|
| Claude | Agree | Strongly Agree | 0.90 |
| GPT-4 | Agree | Agree | 0.85 |
| Gemini | Agree | Neutral | 0.75 |

**Consensus Score:** 0.85

### Key Disagreement

Gemini expressed concern about deprecation without explicit migration testing:
> "While the node appears unused, recommend adding deprecation warning for
> one release cycle before removal."

**Resolution:** Added deprecation warning step to execution plan.

---

## Execution Plans

### Plan 001: Add Sentiment Analysis Tool [VALIDATED]

**Category:** Application Improvement
**Priority:** 1
**Estimated Effort:** 2 AI commits
**Validation Score:** 0.85

**Steps:**
1. Create `SentimentAnalysisTool` in `src/tools/sentiment.rs`
2. Add node to graph after `message_parser` node
3. Update `response_generator` to use sentiment field
4. Add 10 test cases for sentiment detection
5. Run benchmark comparing old vs new approach

**Success Criteria:**
- [ ] Sentiment accuracy ≥ 90% on test set
- [ ] Latency reduction ≥ 150ms
- [ ] Retry rate reduction ≥ 2%

**Citations:** [trace-001], [trace-017], [aggregation-001]

---

## Hypotheses

### Hypothesis 001: Retry Rate Will Drop Below 6%

**Statement:** After implementing Plan 001 (sentiment tool), the overall retry
rate will drop from 8.5% to below 6%.

**Reasoning:** 34% of retries appear to be sentiment-related. Removing prompt
heuristics should eliminate most of these.

**Expected Evidence:**
- Retry rate < 6% after 50 executions
- Sentiment-related retries < 1%

**Evaluation Trigger:** After 50 executions post-implementation

---

## Meta-Analysis

### Pattern Observed

This is the third consecutive report identifying prompt workarounds as a
capability gap source. Consider systematic audit of all prompts for
heuristic patterns.

### Momentum

Improvement velocity: +2.1% success rate per week (4 week average)
Current trajectory suggests 95% success rate achievable in ~3 weeks.

### Previous Hypothesis Review

**Hypothesis from 2025-12-08:** "Adding caching will reduce latency by 30%"
- **Outcome:** PARTIALLY CORRECT
- **Observed:** Latency reduced by 22% (not 30%)
- **Analysis:** Cache hit rate lower than expected (45% vs 70%)
- **Lesson:** Be more conservative with cache hit rate estimates

---

## Design Notes for Future Iterations

1. The sentiment analysis gap was visible 3 reports ago but not prioritized.
   Consider lowering threshold for capability gap escalation.

2. Multi-model consensus adds ~45 seconds per analysis. Consider caching
   model reviews for similar patterns.

3. Hypothesis accuracy: 67% (4/6 correct). Main error: overestimating
   improvement magnitudes. Apply 0.7x adjustment factor.

---

## Citations

- [trace-001]: ExecutionTrace thread_id=abc123 @ 2025-12-09T10:15:00
- [trace-017]: ExecutionTrace thread_id=def456 @ 2025-12-09T11:30:00
- [trace-034]: ExecutionTrace thread_id=ghi789 @ 2025-12-09T14:00:00
- [aggregation-001]: Query "SELECT node, count(*) FROM executions GROUP BY node"
```

---

## Implementation Phases

### Restructured for MVP-First Approach

Based on critical review, restructuring to MVP-first:

**MVP (N=328-333):** CapabilityGap + ExecutionPlan + Local Storage + CLI
**Phase 2 (N=334-340):** Deprecation + Retrospective + Multi-model (feature flag)
**Phase 3 (N=341-346):** Hypotheses + Meta-analysis + Dasher integration

### Design Improvement: Tiered Analysis

```rust
pub enum AnalysisDepth {
    Metrics,        // Per-execution: just collect
    LocalAnalysis,  // Periodic: local analysis only
    DeepAnalysis,   // On-demand: multi-model consensus
}
```

### Design Improvement: DashOpt Integration

```rust
impl CapabilityGap {
    /// Convert gap to DashOpt experiment for validation
    pub fn to_optimization_experiment(&self) -> OptimizationExperiment { ... }
}
```

---

### Phase 1: Core Data Structures (N=328) - ✅ COMPLETE

**Status:** Complete - N=328
**Implementation:**
- Created `crates/dashflow/src/self_improvement/` module with:
  - `types.rs` - All core data structures (IntrospectionReport, CapabilityGap, DeprecationRecommendation, ExecutionPlan, Hypothesis, Citation, etc.)
  - `storage.rs` - File-based storage in `.dashflow/introspection/` with JSON and markdown formats
  - `mod.rs` - Module exports and documentation
- Implemented serialization to JSON and markdown for all types
- 17 new tests for self-improvement types and storage
- All types include builder patterns and convenience methods
- Integrated with lib.rs exports

### Phase 2: Analysis Engines (N=329) - ✅ COMPLETE

**Status:** Complete - N=329
**Implementation:**
- Created `crates/dashflow/src/self_improvement/analyzers.rs` with:
  - `CapabilityGapAnalyzer` - Analyzes ExecutionTraces for capability gaps
    - Error pattern detection (recurring errors → missing functionality)
    - Retry pattern analysis (high retry rates → inadequate tooling)
    - Performance gap detection (slow nodes → bottlenecks)
    - Missing tool detection (tool-related errors)
  - `DeprecationAnalyzer` - Identifies unused/redundant components
    - Unused node detection (known nodes not executed)
    - Low-usage tool identification
  - `RetrospectiveAnalyzer` - Counterfactual analysis
    - Execution summary generation
    - Counterfactual suggestions for failures/slow executions
    - Missing tool identification
    - Application, task, and platform insights
  - `PatternDetector` - Identifies recurring patterns
    - Recurring error patterns
    - Performance degradation trends
    - Execution flow patterns (dominant and rare paths)
    - Resource usage patterns (token consumption)
- Configurable analysis via `*Config` structs
- 11 new tests (5927 total, up from 5916)
- All analyzers exported via lib.rs

### Phase 3: Multi-Model Consensus (N=335) - ✅ COMPLETE

**Status:** Complete - N=335
**Implementation:**
- Created `crates/dashflow/src/self_improvement/consensus.rs` with:
  - `ModelReviewer` async trait for AI model review integration
  - `ReviewRequest` and `ReviewFocus` types for structured review requests
  - `ExecutionContext` for providing execution statistics context
  - `AnthropicReviewer` - Claude models via ANTHROPIC_API_KEY
  - `OpenAIReviewer` - GPT models via OPENAI_API_KEY
  - `GoogleReviewer` - Gemini models via GOOGLE_API_KEY
  - `MockReviewer` for testing without API calls
  - `ConsensusBuilder` for orchestrating multi-model reviews
  - Response parsing with assessment, confidence, critiques extraction
  - `synthesize_consensus()` for aggregating reviews into ConsensusResult
- Auto-configuration from environment variables
- 18 new tests (5971 total, up from 5953)
- All consensus types exported via mod.rs

### Phase 4: Plan Generation (N=336) ✅ COMPLETE

1. `PlanGenerator` - Creates ExecutionPlans from analysis ✅
2. `PlanValidator` - Validates against consensus ✅
3. `PlanTracker` - Tracks plan status over time ✅

**Accomplishments:**
- `PlanGenerator` transforms CapabilityGaps, DeprecationRecommendations, and RetrospectiveAnalysis into ExecutionPlans
- `PlanValidator` validates plans against multi-model ConsensusResult with configurable thresholds
- `PlanTracker` manages plan lifecycle (pending → in_progress → implemented/failed)
- Added storage methods: `update_plan`, `list_pending_plans`, `list_implemented_plans`, `list_failed_plans`, `move_plan_to_implemented`, `move_plan_to_failed`
- 16 new tests (5989 total lib tests)
- All planner types exported via mod.rs

### Phase 5: Meta-Analysis (N=337-339) ✅ COMPLETE

**Implemented in N=337:**
- `HypothesisTracker` - Creates hypotheses from gaps/plans/deprecations, evaluates against metrics
- `MetaAnalyzer` - Analyzes patterns across reports, calculates improvement momentum
- `DesignNoteGenerator` - Creates design notes for future AI iterations
- All meta-analysis types exported via mod.rs
- 79 self_improvement tests passing

### Phase 6: Integration (N=338) - ✅ COMPLETE

**Status:** Complete - N=338
**Implementation:**
- Created `crates/dashflow/src/self_improvement/integration.rs` with:
  - `TriggerSystem` - Automatic introspection triggers (per-execution, periodic, time-based, error-spike)
  - `TriggerConfig` - Configurable trigger thresholds and behaviors
  - `ExecutionStats` - Thread-safe execution statistics tracking
  - `DasherIntegration` - Plan implementation lifecycle management
  - `IntrospectionOrchestrator` - Full pipeline coordination
  - `OrchestratorConfig` - Configurable orchestrator behavior
  - CLI support functions: `run_cli_introspection`, `approve_plan_cli`, `list_plans_cli`
- 12 new tests for integration module (91 total self_improvement tests)
- 6016 total lib tests passing, 0 clippy warnings
- All integration types exported via mod.rs

---

## Trigger Conditions

Introspection runs automatically when:

| Trigger | Condition | Scope |
|---------|-----------|-------|
| Per-Execution | Every graph execution | Execution |
| Periodic | Every N executions (configurable) | GraphAggregate |
| Time-Based | Every T hours (configurable) | TimeWindow |
| Error Spike | Error rate > threshold | System |
| Manual | CLI command | Any |

---

## Integration with Dasher

Dasher (Claude Code + DashFlow) implements validated plans:

```rust
/// Dasher picks up validated plans and implements them
pub struct DasherIntegration {
    /// Path to .dashflow/introspection/plans/active/
    plans_dir: PathBuf,

    /// Callback when plan is implemented
    on_implemented: Box<dyn Fn(&ExecutionPlan, &str) -> Result<()>>,
}

impl DasherIntegration {
    /// Get next plan to implement
    pub fn next_plan(&self) -> Option<ExecutionPlan> {
        // Returns highest priority validated plan
    }

    /// Mark plan as implemented
    pub fn mark_implemented(&self, plan_id: Uuid, commit_hash: &str) -> Result<()> {
        // Move from active/ to implemented/
        // Update plan status
        // Trigger hypothesis evaluation if applicable
    }
}
```

---

## Success Criteria

- [x] Introspection reports generated automatically
- [x] Multi-model consensus working with ≥2 providers
- [x] Hypothesis tracking with accuracy measurement
- [x] Meta-analysis identifying patterns
- [x] Dasher integration for plan implementation
- [x] Git-based history with citations
- [x] All existing tests pass (6016 lib tests)
- [x] 0 clippy warnings

---

## Estimated Effort

| Phase | Commits | Description |
|-------|---------|-------------|
| 1 | 3 | Core data structures |
| 2 | 4 | Analysis engines |
| 3 | 3 | Multi-model consensus |
| 4 | 3 | Plan generation |
| 5 | 3 | Meta-analysis |
| 6 | 3 | Integration |
| **Total** | **19** | |

---

## Version History

| Date | Change | Author |
|------|--------|--------|
| 2025-12-09 | Initial design | MANAGER |
