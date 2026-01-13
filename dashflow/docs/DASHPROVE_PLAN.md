# DashProve: Unified AI-Native Verification Platform

**Last Updated:** 2026-01-02 (Worker #2337 - Add missing Last Updated headers)

**Version:** 1.0
**Created:** 2025-12-17
**Status:** Approved for Development
**Type:** Standalone Project (separate from DashFlow)

---

## Executive Summary

DashProve is a unified verification platform that abstracts over multiple formal verification tools (LEAN 4, TLA+, Kani, Alloy, Coq, etc.) and provides an AI-native interface for automated proof generation, verification, and learning.

**Primary Users:**
- **DashFlow**: Verify the AI operating system's core semantics and implementation
- **Dasher**: Enable the agentic coding system to verify code it writes and modifications it makes
- **External Projects**: Any Rust/AI project needing formal verification

**Key Innovation:** One specification language, multiple backends, AI-assisted proof synthesis, continuous learning from successful proofs.

---

## Why DashProve?

### The Problem

Current formal verification tools are:
1. **Fragmented** - Each tool has its own language, paradigm, and workflow
2. **Expert-Only** - Requires years of specialized training
3. **Slow** - Verification can take hours or days
4. **No Learning** - Each proof starts from scratch
5. **Human-Centric** - Not designed for AI consumption

### The Solution

DashProve provides:
1. **Unified Language** - One spec language compiles to all backends
2. **AI-Native Interface** - Structured I/O, incremental verification, confidence scores
3. **Intelligent Dispatch** - Automatically selects best tool for each property
4. **Proof Learning** - Builds corpus, learns strategies, improves over time
5. **Proof Synthesis** - AI generates proofs with tool assistance

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────────┐
│                              DASHPROVE ARCHITECTURE                              │
├─────────────────────────────────────────────────────────────────────────────────┤
│                                                                                  │
│  ┌────────────────────────────────────────────────────────────────────────────┐ │
│  │                              CLIENTS                                       │ │
│  │                                                                            │ │
│  │   ┌──────────────┐    ┌──────────────┐    ┌──────────────┐               │ │
│  │   │  DashFlow    │    │   Dasher     │    │   External   │               │ │
│  │   │  (AI OS)     │    │ (AI Coder)   │    │   Projects   │               │ │
│  │   └──────────────┘    └──────────────┘    └──────────────┘               │ │
│  │           │                  │                  │                         │ │
│  └───────────┴──────────────────┴──────────────────┴─────────────────────────┘ │
│                                  │                                              │
│                                  ▼                                              │
│  ┌────────────────────────────────────────────────────────────────────────────┐ │
│  │                         DASHPROVE API                                      │ │
│  │                                                                            │ │
│  │   • REST API (for services)                                               │ │
│  │   • Rust crate (for embedding)                                            │ │
│  │   • CLI (for humans and scripts)                                          │ │
│  │   • Language Server Protocol (for IDEs)                                   │ │
│  │                                                                            │ │
│  └────────────────────────────────────────────────────────────────────────────┘ │
│                                  │                                              │
│                                  ▼                                              │
│  ┌────────────────────────────────────────────────────────────────────────────┐ │
│  │              UNIFIED SPECIFICATION LANGUAGE (USL)                          │ │
│  │                                                                            │ │
│  │   • Theorems (mathematical properties)                                    │ │
│  │   • Temporal properties (system behavior over time)                       │ │
│  │   • Contracts (pre/post conditions)                                       │ │
│  │   • Invariants (always-true properties)                                   │ │
│  │   • Refinements (implementation matches spec)                             │ │
│  │                                                                            │ │
│  └────────────────────────────────────────────────────────────────────────────┘ │
│                                  │                                              │
│                                  ▼                                              │
│  ┌────────────────────────────────────────────────────────────────────────────┐ │
│  │                    INTELLIGENT DISPATCHER                                  │ │
│  │                                                                            │ │
│  │   • Analyzes property type                                                │ │
│  │   • Selects optimal backend(s)                                            │ │
│  │   • Parallelizes when beneficial                                          │ │
│  │   • Combines results from multiple backends                               │ │
│  │   • Learns from verification history                                      │ │
│  │                                                                            │ │
│  └────────────────────────────────────────────────────────────────────────────┘ │
│                                  │                                              │
│          ┌───────────┬──────────┼──────────┬───────────┬───────────┐           │
│          ▼           ▼          ▼          ▼           ▼           ▼           │
│     ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐   │
│     │ LEAN 4  │ │  TLA+   │ │  Kani   │ │  Alloy  │ │   Coq   │ │  Dafny  │   │
│     │ Backend │ │ Backend │ │ Backend │ │ Backend │ │ Backend │ │ Backend │   │
│     └─────────┘ └─────────┘ └─────────┘ └─────────┘ └─────────┘ └─────────┘   │
│          │           │          │          │           │           │           │
│          └───────────┴──────────┼──────────┴───────────┴───────────┘           │
│                                 ▼                                               │
│  ┌────────────────────────────────────────────────────────────────────────────┐ │
│  │                      PROOF LEARNING SYSTEM                                 │ │
│  │                                                                            │ │
│  │   • Proof corpus (searchable database of successful proofs)               │ │
│  │   • Strategy learner (which approaches work for which problems)           │ │
│  │   • Tactic database (effectiveness statistics per context)                │ │
│  │   • Similarity search (find related proofs)                               │ │
│  │   • Transfer learning (apply proof patterns to new problems)              │ │
│  │                                                                            │ │
│  └────────────────────────────────────────────────────────────────────────────┘ │
│                                 │                                               │
│                                 ▼                                               │
│  ┌────────────────────────────────────────────────────────────────────────────┐ │
│  │                      AI PROOF ASSISTANT                                    │ │
│  │                                                                            │ │
│  │   • Proof sketch elaboration (AI outlines, tool fills details)            │ │
│  │   • Tactic suggestion (recommend next proof step)                         │ │
│  │   • Counterexample explanation (explain why property fails)               │ │
│  │   • Proof repair (fix proofs after code changes)                          │ │
│  │   • Natural language interaction (describe what to prove)                 │ │
│  │                                                                            │ │
│  └────────────────────────────────────────────────────────────────────────────┘ │
│                                                                                  │
└─────────────────────────────────────────────────────────────────────────────────┘
```

---

## Unified Specification Language (USL)

### Design Goals

1. **Expressive** - Can encode any property from any backend
2. **Readable** - Looks like mathematics/logic, not code
3. **Compilable** - Translates to LEAN, TLA+, Kani, Alloy, etc.
4. **Extensible** - New backends can be added

### Grammar Overview

```
// Types
type Node = { id: String, execute: State -> Result<State> }
type Graph = { nodes: Set<Node>, edges: Relation<Node, Node> }
type State = { messages: List<Message>, data: Map<String, Value> }

// Theorem (compiles to LEAN, Coq, Isabelle)
theorem acyclic_terminates {
    forall g: Graph, s: State .
        acyclic(g) implies
        exists s': State . executes(g, s, s') and is_terminal(s')
}

// Temporal property (compiles to TLA+, SPIN)
temporal eventually_checkpoint {
    forall execution: Trace .
        always(eventually(at_checkpoint(execution)))
}

temporal no_deadlock {
    always(exists agent in agents . enabled(agent))
}

// Contract (compiles to Kani, Creusot, Prusti)
contract Graph::add_node(self, node: Node) -> Result<()> {
    requires {
        not self.contains(node.id)
    }
    ensures {
        self'.contains(node.id)
        self'.nodes.len() == self.nodes.len() + 1
        forall n in self.nodes . n in self'.nodes  // preservation
    }
    ensures_err {
        self' == self  // unchanged on error
    }
}

// Invariant (compiles to Alloy for bounded checking, LEAN for proof)
invariant graph_connectivity {
    forall g: Graph .
        forall n in g.nodes .
            reachable(g.entry, n, g.edges) or n == g.entry
}

// Refinement (compiles to LEAN, proves implementation matches spec)
refinement optimized_graph refines graph_spec {
    abstraction {
        OptimizedGraph.to_graph(og) == g
    }
    simulation {
        forall og: OptimizedGraph, action: Action .
            step(og.to_graph(), action) == step(og, action).to_graph()
    }
}

// Probabilistic (compiles to Storm, PRISM)
probabilistic response_time_bound {
    probability(response_time < 100ms) >= 0.99
}

// Security (compiles to Tamarin, ProVerif)
security no_information_leak {
    forall t1, t2: Tenant . t1 != t2 implies
        not(can_observe(t1, actions(t2)))
}
```

### Compilation Targets

| USL Construct | LEAN 4 | TLA+ | Kani | Alloy | Coq |
|---------------|--------|------|------|-------|-----|
| `theorem` | theorem/lemma | - | - | assert | Theorem |
| `temporal` | - | spec | - | - | - |
| `contract` | - | - | proof harness | - | Program |
| `invariant` | theorem | invariant | assertion | fact | Lemma |
| `refinement` | refinement | - | - | - | Refinement |

---

## AI-Native Interface

### Core Types

```rust
/// Result of verification - fully structured, no text parsing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    /// Overall status
    pub status: VerificationStatus,

    /// Structured proof (if proven)
    pub proof: Option<StructuredProof>,

    /// Structured counterexample (if disproven)
    pub counterexample: Option<StructuredCounterexample>,

    /// Confidence score (0.0 - 1.0) for partial results
    pub confidence: f64,

    /// Suggestions for how to proceed
    pub suggestions: Vec<Suggestion>,

    /// Related proofs from corpus
    pub related_proofs: Vec<ProofReference>,

    /// Which backends were used
    pub backends_used: Vec<BackendInfo>,

    /// Performance metrics
    pub metrics: VerificationMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VerificationStatus {
    /// Property proven true
    Proven,

    /// Property proven false (counterexample found)
    Disproven,

    /// Could not determine (timeout, resource limit, undecidable)
    Unknown { reason: UnknownReason },

    /// Partially verified
    Partial {
        verified_percentage: f64,
        remaining: Vec<Subgoal>,
    },
}

/// Structured proof - machine readable
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuredProof {
    pub id: ProofId,
    pub property: Property,
    pub steps: Vec<ProofStep>,
    pub dependencies: Vec<ProofId>,
    pub backend: BackendId,
    pub verification_time: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProofStep {
    Axiom { name: String },
    Tactic { name: String, args: Vec<Term>, subgoals: Vec<Goal> },
    Lemma { id: ProofId, instantiation: Vec<(Var, Term)> },
    Induction { var: Var, base: Box<ProofStep>, step: Box<ProofStep> },
    Cases { discriminant: Term, cases: Vec<(Pattern, Box<ProofStep>)> },
    Rewrite { equation: ProofId, direction: Direction, location: Path },
}

/// Structured counterexample - machine readable
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuredCounterexample {
    pub property: Property,
    pub witness: HashMap<Var, Value>,
    pub trace: Option<Vec<TraceStep>>,  // For temporal properties
    pub minimized: bool,
    pub explanation: StructuredExplanation,
}

/// Suggestions for AI to act on
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Suggestion {
    /// Try a different proof strategy
    TryStrategy {
        strategy: Strategy,
        confidence: f64,
        reason: String,
    },

    /// Add a helper lemma
    AddLemma {
        statement: Property,
        why_helpful: String,
    },

    /// Strengthen the invariant
    StrengthenInvariant {
        current: Property,
        proposed: Property,
    },

    /// The property might be false - here's a potential counterexample
    PotentialCounterexample {
        witness: HashMap<Var, Value>,
        confidence: f64,
    },

    /// Split into subgoals
    SplitGoal {
        subgoals: Vec<Property>,
    },

    /// Use a similar proof as template
    UseTemplate {
        similar_proof: ProofId,
        similarity: f64,
        adaptation_hints: Vec<String>,
    },
}
```

### API Design

```rust
/// Main DashProve client
pub struct DashProve {
    config: Config,
    backends: BackendRegistry,
    corpus: ProofCorpus,
    learner: StrategyLearner,
}

impl DashProve {
    /// Verify a specification
    pub async fn verify(&self, spec: &Spec) -> Result<VerificationResult> {
        // 1. Parse and type-check spec
        let typed = self.typecheck(spec)?;

        // 2. Select backends
        let backends = self.dispatcher.select(&typed);

        // 3. Run verification (potentially parallel)
        let results = self.run_backends(&typed, &backends).await?;

        // 4. Merge and enhance results
        let merged = self.merge_results(results);
        let enhanced = self.enhance_with_suggestions(merged);

        // 5. Learn from result
        self.learner.observe(&enhanced);

        Ok(enhanced)
    }

    /// Incremental verification after changes
    pub async fn verify_incremental(
        &self,
        spec: &Spec,
        changes: &[Change],
    ) -> Result<IncrementalResult> {
        // Identify affected properties
        let affected = self.dependency_graph.affected(changes);

        // Verify only affected, reuse cache for rest
        let cached = self.cache.get_valid(&spec.id, &affected);
        let new = self.verify_subset(&affected).await?;

        Ok(IncrementalResult {
            cached_reused: cached.len(),
            newly_verified: new.len(),
            results: cached.into_iter().chain(new).collect(),
        })
    }

    /// Elaborate a proof sketch into full proof
    pub async fn elaborate_sketch(
        &self,
        property: &Property,
        sketch: &ProofSketch,
    ) -> Result<ElaborationResult> {
        // Try to fill in the sketch
        let elaboration = self.elaborator.elaborate(property, sketch).await?;

        Ok(ElaborationResult {
            proof: elaboration.proof,
            gaps: elaboration.unfilled_gaps,
            suggestions: elaboration.suggestions,
        })
    }

    /// Find similar proofs
    pub fn find_similar(&self, property: &Property, k: usize) -> Vec<SimilarProof> {
        self.corpus.search_similar(property, k)
    }

    /// Get tactic suggestions for a goal
    pub fn suggest_tactics(&self, goal: &Goal) -> Vec<TacticSuggestion> {
        self.learner.suggest_tactics(goal)
    }

    /// Explain a counterexample in natural language
    pub fn explain_counterexample(
        &self,
        ce: &StructuredCounterexample,
    ) -> Explanation {
        self.explainer.explain(ce)
    }
}
```

### CLI Interface

```bash
# Verify a specification file
dashprove verify spec.usl

# Verify with specific backends
dashprove verify spec.usl --backends lean,tla+

# Incremental verification
dashprove verify spec.usl --incremental --since HEAD~1

# Find similar proofs
dashprove search "termination for recursive functions"

# Explain a counterexample
dashprove explain counterexample.json

# Interactive proof mode
dashprove prove spec.usl --interactive

# Check proof corpus statistics
dashprove corpus stats

# Export to specific backend (for debugging)
dashprove export spec.usl --target lean --output spec.lean
```

### REST API

```yaml
# OpenAPI spec excerpt
paths:
  /verify:
    post:
      summary: Verify a specification
      requestBody:
        content:
          application/json:
            schema:
              $ref: '#/components/schemas/VerifyRequest'
      responses:
        '200':
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/VerificationResult'

  /verify/incremental:
    post:
      summary: Incremental verification after changes

  /sketch/elaborate:
    post:
      summary: Elaborate a proof sketch

  /corpus/search:
    get:
      summary: Search proof corpus
      parameters:
        - name: query
          in: query
          schema:
            type: string
        - name: k
          in: query
          schema:
            type: integer
            default: 10

  /tactics/suggest:
    post:
      summary: Get tactic suggestions for a goal
```

---

## Backend Integration

### Backend Trait

```rust
/// Trait that all verification backends must implement
#[async_trait]
pub trait VerificationBackend: Send + Sync {
    /// Unique identifier
    fn id(&self) -> BackendId;

    /// What property types this backend supports
    fn supports(&self) -> Vec<PropertyType>;

    /// Compile USL to backend-specific format
    fn compile(&self, spec: &TypedSpec) -> Result<CompiledSpec>;

    /// Run verification
    async fn verify(&self, compiled: &CompiledSpec) -> Result<BackendResult>;

    /// Parse backend output into structured result
    fn parse_result(&self, raw: &RawOutput) -> Result<BackendResult>;

    /// Get suggested tactics for a goal (if supported)
    fn suggest_tactics(&self, goal: &Goal) -> Vec<TacticSuggestion> {
        vec![]  // Default: no suggestions
    }

    /// Check if backend is available
    async fn health_check(&self) -> HealthStatus;
}

/// Result from a single backend
pub struct BackendResult {
    pub backend: BackendId,
    pub status: VerificationStatus,
    pub proof: Option<BackendProof>,
    pub counterexample: Option<BackendCounterexample>,
    pub diagnostics: Vec<Diagnostic>,
    pub time_taken: Duration,
}
```

### LEAN 4 Backend

```rust
pub struct Lean4Backend {
    lean_path: PathBuf,
    mathlib_available: bool,
    cache: ProofCache,
}

impl Lean4Backend {
    fn compile_theorem(&self, theorem: &Theorem) -> String {
        format!(
            r#"
            theorem {name} : {statement} := by
              {tactics}
            "#,
            name = theorem.name,
            statement = self.compile_prop(&theorem.property),
            tactics = self.compile_tactics(&theorem.hints),
        )
    }

    fn compile_prop(&self, prop: &Property) -> String {
        match prop {
            Property::ForAll { var, ty, body } => {
                format!("∀ {} : {}, {}", var, self.compile_type(ty), self.compile_prop(body))
            }
            Property::Implies { lhs, rhs } => {
                format!("{} → {}", self.compile_prop(lhs), self.compile_prop(rhs))
            }
            // ... more cases
        }
    }
}

#[async_trait]
impl VerificationBackend for Lean4Backend {
    fn id(&self) -> BackendId { BackendId::Lean4 }

    fn supports(&self) -> Vec<PropertyType> {
        vec![
            PropertyType::Theorem,
            PropertyType::Invariant,
            PropertyType::Refinement,
        ]
    }

    async fn verify(&self, compiled: &CompiledSpec) -> Result<BackendResult> {
        // Write to temp file
        let temp = self.write_temp(&compiled.lean_code)?;

        // Run lake build
        let output = Command::new(&self.lean_path)
            .arg("build")
            .arg(&temp)
            .output()
            .await?;

        // Parse output
        self.parse_lean_output(&output)
    }
}
```

### TLA+ Backend

```rust
pub struct TlaPlusBackend {
    tla_path: PathBuf,
    tlc_path: PathBuf,
    apalache_path: Option<PathBuf>,  // Symbolic model checker
}

impl TlaPlusBackend {
    fn compile_temporal(&self, temporal: &TemporalProperty) -> String {
        match temporal {
            TemporalProperty::Always(inner) => {
                format!("[][{}]_vars", self.compile_predicate(inner))
            }
            TemporalProperty::Eventually(inner) => {
                format!("<>({})", self.compile_predicate(inner))
            }
            TemporalProperty::LeadsTo { from, to } => {
                format!("({}) ~> ({})",
                    self.compile_predicate(from),
                    self.compile_predicate(to))
            }
            // ... more cases
        }
    }
}

#[async_trait]
impl VerificationBackend for TlaPlusBackend {
    fn id(&self) -> BackendId { BackendId::TlaPlus }

    fn supports(&self) -> Vec<PropertyType> {
        vec![
            PropertyType::Temporal,
            PropertyType::Invariant,
        ]
    }

    async fn verify(&self, compiled: &CompiledSpec) -> Result<BackendResult> {
        // Run TLC model checker
        let output = Command::new(&self.tlc_path)
            .arg("-config")
            .arg(&compiled.config_path)
            .arg(&compiled.tla_path)
            .output()
            .await?;

        self.parse_tlc_output(&output)
    }
}
```

### Kani Backend (Rust Implementation Verification)

```rust
pub struct KaniBackend {
    kani_path: PathBuf,
}

impl KaniBackend {
    fn compile_contract(&self, contract: &Contract) -> String {
        format!(
            r#"
            #[kani::proof]
            fn verify_{name}() {{
                let {params} = kani::any();

                // Preconditions
                kani::assume({preconditions});

                // Call function
                let result = {function_call};

                // Postconditions
                kani::assert({postconditions});
            }}
            "#,
            name = contract.function_name,
            params = contract.params.join(", "),
            preconditions = self.compile_preconditions(&contract.requires),
            function_call = self.compile_call(&contract),
            postconditions = self.compile_postconditions(&contract.ensures),
        )
    }
}

#[async_trait]
impl VerificationBackend for KaniBackend {
    fn id(&self) -> BackendId { BackendId::Kani }

    fn supports(&self) -> Vec<PropertyType> {
        vec![PropertyType::Contract]
    }

    async fn verify(&self, compiled: &CompiledSpec) -> Result<BackendResult> {
        let output = Command::new(&self.kani_path)
            .arg("--harness")
            .arg(&compiled.harness_name)
            .arg(&compiled.crate_path)
            .output()
            .await?;

        self.parse_kani_output(&output)
    }
}
```

---

## Proof Learning System

### Architecture

```rust
pub struct ProofLearningSystem {
    /// Database of successful proofs
    corpus: ProofCorpus,

    /// Embedding model for similarity
    embedder: ProofEmbedder,

    /// Strategy prediction model
    strategy_model: StrategyModel,

    /// Tactic effectiveness statistics
    tactic_stats: TacticDatabase,
}

impl ProofLearningSystem {
    /// Record a successful proof
    pub fn record_success(&mut self, result: &VerificationResult) {
        if let Some(proof) = &result.proof {
            // Add to corpus
            self.corpus.insert(proof.clone());

            // Update embeddings
            let embedding = self.embedder.embed(&proof.property);
            self.corpus.index(&proof.id, embedding);

            // Update strategy model
            let strategy = self.extract_strategy(proof);
            self.strategy_model.record_success(&proof.property, &strategy);

            // Update tactic stats
            for step in &proof.steps {
                self.tactic_stats.record(step, &proof.property, true);
            }
        }
    }

    /// Find similar proofs
    pub fn find_similar(&self, property: &Property, k: usize) -> Vec<SimilarProof> {
        let query = self.embedder.embed(property);
        let neighbors = self.corpus.nearest_neighbors(&query, k);

        neighbors.into_iter().map(|(id, similarity)| {
            SimilarProof {
                proof: self.corpus.get(&id).unwrap(),
                similarity,
                adaptation_hints: self.compute_adaptation_hints(property, &id),
            }
        }).collect()
    }

    /// Predict best strategy for a property
    pub fn predict_strategy(&self, property: &Property) -> Vec<StrategyPrediction> {
        self.strategy_model.predict(property)
    }

    /// Suggest tactics for a goal
    pub fn suggest_tactics(&self, goal: &Goal) -> Vec<TacticSuggestion> {
        // Get tactics that worked in similar contexts
        let context = self.extract_context(goal);
        self.tactic_stats.best_for_context(&context)
    }
}
```

### Embedding Model

```rust
/// Embeds proofs and properties into vector space for similarity search
pub struct ProofEmbedder {
    model: EmbeddingModel,
    dimension: usize,
}

impl ProofEmbedder {
    /// Embed a property into vector space
    pub fn embed(&self, property: &Property) -> Embedding {
        // Structural embedding based on property AST
        let structural = self.embed_structure(property);

        // Semantic embedding based on property meaning
        let semantic = self.model.encode(&property.to_string());

        // Combine
        self.combine(structural, semantic)
    }

    fn embed_structure(&self, property: &Property) -> Vec<f32> {
        // Encode property structure as features
        let mut features = vec![];

        // Quantifier depth
        features.push(self.count_quantifiers(property) as f32);

        // Implication depth
        features.push(self.count_implications(property) as f32);

        // Types used
        features.extend(self.encode_types(property));

        // Function symbols
        features.extend(self.encode_functions(property));

        features
    }
}
```

---

## Integration with DashFlow and Dasher

### DashFlow Integration

```rust
// In DashFlow: crates/dashflow/src/formal/dashprove.rs

use dashprove::{DashProve, Spec, VerificationResult};

/// DashFlow's interface to DashProve
pub struct DashFlowVerifier {
    client: DashProve,
}

impl DashFlowVerifier {
    /// Verify a graph modification is safe
    pub async fn verify_modification(
        &self,
        graph: &StateGraph,
        modification: &Modification,
    ) -> Result<ModificationVerification> {
        // Generate USL spec for modification safety
        let spec = self.generate_modification_spec(graph, modification);

        // Verify
        let result = self.client.verify(&spec).await?;

        Ok(ModificationVerification {
            safe: result.status == VerificationStatus::Proven,
            proof: result.proof,
            counterexample: result.counterexample,
            suggestions: result.suggestions,
        })
    }

    /// Verify loop properties
    pub async fn verify_loop(
        &self,
        loop_def: &VerifiedLoop,
    ) -> Result<LoopVerification> {
        let spec = self.generate_loop_spec(loop_def);
        let result = self.client.verify(&spec).await?;

        Ok(LoopVerification {
            checkpoint_reachable: self.extract_checkpoint_result(&result),
            progress_guaranteed: self.extract_progress_result(&result),
            safe_interruptible: self.extract_safety_result(&result),
            full_result: result,
        })
    }

    /// Compile runtime monitor from spec
    pub fn compile_monitor(&self, spec: &Spec) -> RuntimeMonitor {
        self.client.compile_to_monitor(spec)
    }
}
```

### Dasher Integration

```rust
// In Dasher: Agentic coding system

/// Dasher's verification-aware code generation
pub struct VerifiedCodeGenerator {
    dashprove: DashProve,
    llm: Box<dyn LLM>,
}

impl VerifiedCodeGenerator {
    /// Generate code with verification
    pub async fn generate_verified(
        &self,
        task: &CodingTask,
    ) -> Result<VerifiedCode> {
        // 1. Generate initial code
        let code = self.llm.generate_code(task).await?;

        // 2. Generate specification from task
        let spec = self.generate_spec(task);

        // 3. Verify code against spec
        let mut result = self.dashprove.verify_code(&code, &spec).await?;

        // 4. If verification fails, iterate
        let mut attempts = 0;
        while result.status != VerificationStatus::Proven && attempts < 5 {
            // Use counterexample to guide fix
            let fix_guidance = self.analyze_failure(&result);

            // Generate fixed code
            let fixed = self.llm.fix_code(&code, &fix_guidance).await?;

            // Re-verify
            result = self.dashprove.verify_code(&fixed, &spec).await?;
            attempts += 1;
        }

        Ok(VerifiedCode {
            code,
            specification: spec,
            proof: result.proof,
            verification_attempts: attempts,
        })
    }

    /// Verify before committing changes
    pub async fn verify_commit(
        &self,
        changes: &[FileChange],
    ) -> Result<CommitVerification> {
        // Extract affected functions
        let affected = self.extract_affected_functions(changes);

        // Get existing contracts
        let contracts = self.extract_contracts(&affected);

        // Incremental verification
        let result = self.dashprove.verify_incremental(&contracts, changes).await?;

        Ok(CommitVerification {
            safe_to_commit: result.all_passed(),
            failures: result.failures,
            suggestions: result.suggestions,
        })
    }
}
```

---

## Project Structure

```
dashprove/
├── Cargo.toml                 # Workspace root
├── README.md
├── docs/
│   ├── USL_SPECIFICATION.md   # Language specification
│   ├── BACKEND_GUIDE.md       # How to add new backends
│   ├── API_REFERENCE.md       # API documentation
│   └── EXAMPLES.md            # Usage examples
│
├── crates/
│   ├── dashprove/             # Main library
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── api.rs         # Public API
│   │   │   ├── spec.rs        # Specification types
│   │   │   ├── result.rs      # Result types
│   │   │   └── client.rs      # Client implementation
│   │   └── Cargo.toml
│   │
│   ├── dashprove-usl/         # Unified Specification Language
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── grammar.rs     # Parser
│   │   │   ├── ast.rs         # AST types
│   │   │   ├── typecheck.rs   # Type checker
│   │   │   └── compile.rs     # Compilation to backends
│   │   └── Cargo.toml
│   │
│   ├── dashprove-dispatcher/  # Intelligent backend selection
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── selector.rs    # Backend selection logic
│   │   │   ├── parallel.rs    # Parallel execution
│   │   │   └── merge.rs       # Result merging
│   │   └── Cargo.toml
│   │
│   ├── dashprove-backends/    # Backend implementations
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── trait.rs       # Backend trait
│   │   │   ├── lean4.rs       # LEAN 4 backend
│   │   │   ├── tlaplus.rs     # TLA+ backend
│   │   │   ├── kani.rs        # Kani backend
│   │   │   ├── alloy.rs       # Alloy backend
│   │   │   ├── coq.rs         # Coq backend
│   │   │   └── dafny.rs       # Dafny backend
│   │   └── Cargo.toml
│   │
│   ├── dashprove-learning/    # Proof learning system
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── corpus.rs      # Proof corpus
│   │   │   ├── embedder.rs    # Proof embeddings
│   │   │   ├── strategy.rs    # Strategy learning
│   │   │   └── tactics.rs     # Tactic database
│   │   └── Cargo.toml
│   │
│   ├── dashprove-ai/          # AI assistance
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── sketch.rs      # Proof sketch elaboration
│   │   │   ├── suggest.rs     # Tactic suggestion
│   │   │   ├── explain.rs     # Counterexample explanation
│   │   │   └── repair.rs      # Proof repair
│   │   └── Cargo.toml
│   │
│   ├── dashprove-server/      # REST API server
│   │   ├── src/
│   │   │   ├── main.rs
│   │   │   ├── routes.rs
│   │   │   └── handlers.rs
│   │   └── Cargo.toml
│   │
│   └── dashprove-cli/         # Command-line interface
│       ├── src/
│       │   └── main.rs
│       └── Cargo.toml
│
├── usl/                       # USL standard library
│   ├── prelude.usl            # Built-in types and functions
│   ├── graph.usl              # Graph theory
│   ├── temporal.usl           # Temporal logic
│   └── contracts.usl          # Contract patterns
│
└── tests/
    ├── integration/           # Integration tests
    ├── backends/              # Backend-specific tests
    └── examples/              # Example specifications
```

---

## Roadmap

### Phase 1: Foundation (Weeks 1-4)

| Week | Milestone |
|------|-----------|
| 1 | USL grammar design and parser |
| 2 | AST types and type checker |
| 3 | Backend trait and registry |
| 4 | Basic CLI and project structure |

### Phase 2: Core Backends (Weeks 5-10)

| Week | Milestone |
|------|-----------|
| 5-6 | LEAN 4 backend (theorems, invariants) |
| 7-8 | TLA+ backend (temporal properties) |
| 9-10 | Kani backend (contracts) |

### Phase 3: Intelligence (Weeks 11-16)

| Week | Milestone |
|------|-----------|
| 11-12 | Intelligent dispatcher |
| 13-14 | Proof corpus and similarity search |
| 15-16 | Strategy learning |

### Phase 4: AI Assistance (Weeks 17-22)

| Week | Milestone |
|------|-----------|
| 17-18 | Proof sketch elaboration |
| 19-20 | Tactic suggestion |
| 21-22 | Counterexample explanation |

### Phase 5: Integration (Weeks 23-26)

| Week | Milestone |
|------|-----------|
| 23-24 | DashFlow integration |
| 25-26 | Dasher integration |

### Phase 6: Production (Weeks 27-30)

| Week | Milestone |
|------|-----------|
| 27-28 | REST API server |
| 29-30 | Documentation and examples |

---

## Success Criteria

1. **USL Expressiveness**: Can encode 90%+ of properties from LEAN, TLA+, and Kani
2. **Backend Coverage**: At least 4 backends fully integrated
3. **AI Effectiveness**: Proof sketch elaboration succeeds >50% of the time
4. **Learning Value**: Similar proof lookup improves success rate by >20%
5. **DashFlow Integration**: All DashFlow verification uses DashProve
6. **Dasher Integration**: Dasher can verify code before committing
7. **Performance**: <10s for simple properties, <5min for complex ones

---

## TLA+ Protocol Specifications (When Implemented)

When DashProve is built, priority TLA+ specs for DashFlow:

| Spec | Verifies |
|------|----------|
| `GraphExecution.tla` | Node ordering, no deadlock |
| `CheckpointRestore.tla` | No lost state, idempotent restore |
| `TimeTravel.tla` | Cursor consistency, monotonic seq |
| `ParallelExecution.tla` | No race conditions in parallel nodes |
| `DistributedScheduler.tla` | Worker assignment, fault tolerance |
| `StateDiff.tla` | Diff/patch invertibility |
| `EventOrdering.tla` | Out-of-order handling correctness |
| `HashVerification.tla` | Corruption detection completeness |

These specs would verify DashFlow's distributed protocols BEFORE implementation.

---

## Multi-Tool Strategy Summary

| Tool | Verifies | When to Use |
|------|----------|-------------|
| **TLA+** | System design, concurrency, protocols | Before/during implementation |
| **LEAN 4** | Algorithm correctness, type-level proofs | Compile-time guarantees |
| **Kani/CBMC** | Memory safety, bounds | Unsafe code, FFI |
| **Miri** | Undefined behavior | CI runtime checks |
| **Alloy** | Data structure invariants | Design phase |

---

## Future Extensions

1. **More Backends**: Isabelle, F*, Verus, Creusot, SPIN
2. **Neural Verification**: Integration with Marabou, α,β-CROWN
3. **Probabilistic**: Storm, PRISM integration
4. **Security**: Tamarin, ProVerif integration
5. **Visual Editor**: GUI for writing specifications
6. **Cloud Service**: DashProve as a service

---

## References

- [LEAN 4](https://lean-lang.org/)
- [TLA+](https://lamport.azurewebsites.net/tla/tla.html)
- [Kani](https://github.com/model-checking/kani)
- [Alloy](https://alloytools.org/)
- [Z3](https://github.com/Z3Prover/z3)
- [DashFlow](../README.md)
