# DashOptimize: Prompt Optimization for DashFlow

**Last Updated:** 2026-01-02 (Worker #2337 - Add missing Last Updated headers)

**Version:** 1.11.3
**Status:** Production Ready
**Date:** 2025-12-08

## Overview

DashOptimize is a comprehensive prompt optimization framework integrated into DashFlow Rust. It provides automatic prompt engineering through data-driven optimization algorithms, enabling systematic improvement of LLM-based applications.

**Key Capabilities:**
- Automatic few-shot example generation
- Instruction optimization via LLM meta-prompting
- Multi-stage optimization (demos + instructions)
- Graph-level optimization across multiple nodes
- Production features: A/B testing, cost monitoring, data collection
- Model distillation from large to small models

---

## Core Concepts

### 1. Signatures

A **Signature** defines the input/output interface for an LLM call:

```rust
use dashflow::optimize::make_signature;

// Simple signature: input -> output
let sig = make_signature(
    "question -> answer",
    "Answer questions accurately"
)?;

// Multi-field signature
let sig = make_signature(
    "context, question -> reasoning, answer",
    "Use context to answer questions with reasoning"
)?;
```

**Signature Components:**
- **Input fields**: What the LLM receives (e.g., question, context)
- **Output fields**: What the LLM produces (e.g., answer, reasoning)
- **Instructions**: High-level task description
- **Prefixes**: Field-level instructions (auto-inferred or explicit)

### 2. LLMNode - Optimizable Graph Nodes

`LLMNode` is an optimizable DashFlow node that uses signatures:

```rust
use dashflow::optimize::LLMNode;

let node = LLMNode::new(
    signature,
    llm_client,
    "qa_node"
);

// LLMNode implements:
// - Node trait (integrates with StateGraph)
// - Optimizable trait (can be optimized by optimizers)
```

### 3. Optimizers

Optimizers improve LLMNode performance using labeled training data:

| Optimizer | Description | Use Case |
|-----------|-------------|----------|
| **BootstrapFewShot** | Generates few-shot examples from successful traces | Quick baseline optimization |
| **LabeledFewShot** | Selects demos from pre-labeled dataset | When you have labeled data |
| **KNNFewShot** | Nearest-neighbor demo selection at runtime | Dynamic demo selection |
| **RandomSearch** | Multi-strategy random search across configurations | Baseline comparison, ablation studies |
| **SIMBA** | Sequential instruction management via behavior analysis | Systematic instruction refinement |
| **GEPA** | Genetic evolutionary optimization with LLM reflection | Complex optimization with feedback |
| **MIPROv2** | Multi-stage instruction + demo optimization | Maximum quality improvement |
| **COPRO** | LLM-based instruction and prefix optimization | Instruction tuning |
| **Ensemble** | Combines multiple optimized variants | Robustness via voting |
| **BootstrapOptuna** | Automatic hyperparameter tuning via Bayesian optimization | Finding optimal configuration automatically |
| **GRPO** | Group Relative Policy Optimization (reinforcement learning) | Fine-tuning models for complex reasoning |
| **BootstrapFinetune** | Collects execution traces for fine-tuning dataset export | Model distillation and cost optimization |
| **BetterTogether** | Meta-optimizer that composes multiple optimizers | Experimenting with optimization pipelines |

### 4. Modules

Pre-built patterns for common LLM use cases:

- **ChainOfThought**: Adds reasoning before answering
- **ReAct**: Tool-using agent with Thought→Action→Observation loop
- **MultiChainComparison**: Compares multiple reasoning attempts (self-consistency)

---

## Quick Start

### Basic Optimization Example

```rust
use dashflow::optimize::{
    make_signature, LLMNode, BootstrapFewShot,
    OptimizerConfig, exact_match
};
use dashflow_openai::OpenAIClient;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

// 1. Define your state
#[derive(Clone, Serialize, Deserialize)]
struct QAState {
    question: String,
    answer: String,
}

// 2. Create signature
let signature = make_signature(
    "question -> answer",
    "Answer questions accurately and concisely"
)?;

// 3. Create LLM client
let llm = Arc::new(OpenAIClient::new("gpt-4o-mini")?);

// 4. Create optimizable node
let mut node = LLMNode::new(signature, llm, "qa_node");

// 5. Prepare training data
let trainset = vec![
    QAState {
        question: "What is 2+2?".to_string(),
        answer: "4".to_string()
    },
    // ... more examples
];

// 6. Define metric
let metric = Arc::new(|expected: &QAState, predicted: &QAState| {
    Ok(exact_match(&expected.answer, &predicted.answer))
});

// 7. Create and run optimizer
let optimizer = BootstrapFewShot::new(
    OptimizerConfig::new()
        .with_max_few_shot_examples(5)
        .with_max_iterations(3),
    metric
);

let result = optimizer.optimize(&mut node, &trainset).await?;

println!("Improvement: {:.1}% → {:.1}%",
    result.initial_score * 100.0,
    result.final_score * 100.0
);
```

---

## Optimizers In-Depth

### BootstrapFewShot

**Algorithm:** Generate few-shot demonstrations from successful predictions.

**How it works:**
1. Run LLM on training examples
2. Collect successful predictions (metric > threshold)
3. Add successful (input, output) pairs as few-shot demos
4. Iterate until convergence or max iterations

**Best for:** Quick baseline optimization, cold-start scenarios

```rust
let optimizer = BootstrapFewShot::new(
    OptimizerConfig::new()
        .with_max_few_shot_examples(8)
        .with_max_iterations(5)
        .with_min_improvement(0.02), // Stop if improvement < 2%
    metric
);
```

### LabeledFewShot

**Algorithm:** Select representative examples from labeled dataset.

**Strategies:**
- **Sequential**: Pick first K examples
- **Random**: Sample K examples (with optional seed)

**Best for:** When you have pre-labeled training data

```rust
use dashflow::optimize::{LabeledFewShot, SamplingStrategy};

let optimizer = LabeledFewShot::builder()
    .max_demos(10)
    .strategy(SamplingStrategy::Random { seed: Some(42) })
    .metric(metric)
    .build();
```

### KNNFewShot

**Algorithm:** Runtime nearest-neighbor demo selection.

**How it works:**
1. Embed all training examples
2. At inference time, find K nearest neighbors to input
3. Use nearest neighbors as few-shot demos

**Best for:** Dynamic demo selection, diverse input distributions

**Requirements:** Embedding function for similarity search

```rust
use dashflow::optimize::KNNFewShot;

let optimizer = KNNFewShot::builder()
    .k(5) // 5 nearest neighbors
    .metric(metric)
    .embedding_fn(embed_fn) // Your embedding function
    .build();
```

### MIPROv2

**Algorithm:** Multi-stage optimization of instructions and demonstrations.

**Stages:**
1. **Instruction Optimization**: Generate and evaluate candidate instructions
2. **Demo Bootstrap**: Generate few-shot examples
3. **Joint Refinement**: Optimize instructions with demos present

**Best for:** Maximum quality improvement, production applications

**Configuration:**
- **Manual mode**: You specify instruction/demo candidates
- **Auto mode**: MIPROv2 generates candidates

```rust
use dashflow::optimize::{MIPROv2, MIPROv2Mode};

// Auto mode (MIPROv2 generates candidates)
let optimizer = MIPROv2::builder()
    .mode(MIPROv2Mode::Auto {
        num_instruction_candidates: 10,
        max_bootstrapped_demos: 8,
        max_labeled_demos: 4,
    })
    .metric(metric)
    .llm(llm_for_proposal)
    .build()?;

// Manual mode (you provide candidates)
let optimizer = MIPROv2::builder()
    .mode(MIPROv2Mode::Manual {
        instruction_candidates: vec![
            "Answer concisely".to_string(),
            "Provide detailed reasoning".to_string(),
        ],
        demo_candidates: trainset.clone(),
    })
    .metric(metric)
    .build()?;
```

### COPRO

**Algorithm:** LLM-based instruction and field prefix optimization.

**How it works:**
1. Generate instruction candidates via LLM meta-prompting
2. Evaluate each candidate on validation set
3. Select best instruction
4. Optionally optimize field prefixes

**Best for:** Instruction tuning when few-shot isn't enough

```rust
use dashflow::optimize::COPRO;

let optimizer = COPRO::builder()
    .num_candidates(8)
    .optimize_prefixes(true) // Also optimize field prefixes
    .metric(metric)
    .llm(llm_for_proposal)
    .build()?;
```

### Ensemble

**Algorithm:** Combine multiple optimized variants via majority voting.

**How it works:**
1. Take N optimized LLMNodes (from different optimizers)
2. At inference, run all N nodes
3. Return majority vote or aggregated response

**Best for:** Robustness, reducing variance

```rust
use dashflow::optimize::Ensemble;

let optimizer = Ensemble::builder()
    .add_node(node1) // Optimized with BootstrapFewShot
    .add_node(node2) // Optimized with MIPROv2
    .add_node(node3) // Optimized with COPRO
    .size(3)
    .reduce_fn(majority_vote) // Custom aggregation
    .build()?;
```

### RandomSearch

**Algorithm:** Multi-strategy random search across candidate configurations.

**How it works:**
1. Generate candidates using 4 different strategies (seed convention):
   - **seed = -3**: Zero-shot baseline (no demos)
   - **seed = -2**: Labeled few-shot only (uses provided labeled data)
   - **seed = -1**: Unshuffled bootstrap (deterministic bootstrap ordering)
   - **seed >= 0**: Shuffled bootstrap with random demo counts (full randomization)
2. Evaluate each candidate on validation set
3. Select best performing candidate
4. Optional early stopping when target score reached

**Best for:** Baseline comparisons, ablation studies, understanding which optimization strategy works best

**Key Innovation:** Seed convention encodes optimization strategy, making search systematic rather than purely random.

```rust
use dashflow::optimize::{RandomSearch, OptimizerConfig};

let optimizer = RandomSearch::new(
    OptimizerConfig::new()
        .with_max_iterations(10), // Try 10 different candidates
    metric,
    vec![-3, -2, -1, 0, 1, 2], // Seeds to try (covers all strategies)
)?;

// Or use builder for more control
let optimizer = RandomSearch::builder()
    .seeds(vec![-3, -2, -1, 0, 1, 2, 3, 4, 5])
    .target_score(0.95) // Stop early if score > 0.95
    .metric(metric)
    .build()?;
```

**Use case example:** Before investing in complex optimization (MIPROv2), use RandomSearch to establish baseline and understand whether zero-shot, labeled few-shot, or bootstrap performs best for your task.

### SIMBA

**Algorithm:** Sequential Instruction Management via Behavior Analysis.

**How it works:**
1. Optimize instructions sequentially (not in isolation)
2. Two optimization strategies:
   - **AppendADemo**: Add successful examples as demonstrations
   - **AppendARule**: Extract patterns and add as instruction rules
3. Use Poisson sampling for dataset selection (probabilistic coverage)
4. Percentile-based performance bucketing (top performers vs. baseline)
5. Softmax sampling with temperature for candidate selection

**Best for:** Systematic instruction refinement, understanding which instructions work

**Key Innovation:** Sequential optimization preserves instruction order and context, unlike batch optimization which treats instructions independently.

```rust
use dashflow::optimize::{SIMBA, SIMBAStrategy};

let optimizer = SIMBA::builder()
    .strategies(vec![
        SIMBAStrategy::AppendADemo,  // Add successful demos
        SIMBAStrategy::AppendARule,  // Extract and add rules
    ])
    .num_candidates(10) // Generate 10 candidates per strategy
    .k(5) // Top-5 + baseline selection
    .metric(metric)
    .build()?;

// Or use simple constructor with defaults
let optimizer = SIMBA::new(metric)?;
```

**Configuration options:**
- `num_candidates`: Number of instruction candidates to generate (default: 10)
- `k`: Top-K selection for best performers (default: 3)
- `temperature`: Softmax temperature for probabilistic sampling (default: 1.4)
- `lambda`: Poisson parameter for dataset sampling (default: 3.0)

**Use case example:** Iteratively refine instructions by analyzing which examples succeed/fail and either adding them as demos or extracting patterns as rules.

### GEPA

**Algorithm:** Genetic Evolutionary Prompt Algorithm with LLM-based reflection.

**How it works:**
1. Start with initial instruction population
2. Evaluate each instruction on validation set with **textual feedback**
3. Select parents from current population:
   - **CurrentBest** (greedy): Always pick best scoring instruction
   - **Pareto** (probabilistic): Sample from Pareto frontier
4. LLM reflects on failures and proposes improved instructions
5. Add improved instructions to population
6. Repeat until budget exhausted or convergence

**Best for:** Complex optimization requiring feedback, tasks where failures have patterns

**Key Innovation:** Uses LLM as meta-optimizer to reflect on failures and propose improvements - "LLM optimizing LLM". Metric returns both score AND textual feedback explaining why the prediction failed.

```rust
use dashflow::optimize::{GEPA, GEPAConfig, ParentSelection};

// Define metric that returns score + feedback
let metric = Arc::new(|expected: &QAState, predicted: &QAState| {
    let score = exact_match(&expected.answer, &predicted.answer);
    let feedback = if score < 1.0 {
        format!("Expected '{}' but got '{}'", expected.answer, predicted.answer)
    } else {
        "Correct".to_string()
    };
    Ok((score, feedback))
});

let optimizer = GEPA::builder()
    .config(GEPAConfig {
        num_generations: 5,
        population_size: 10,
        parent_selection: ParentSelection::Pareto, // Probabilistic diversity
        max_metric_calls: 100, // Budget control
        max_full_evals: 20,
        seed: Some(42), // Reproducible
    })
    .metric(metric)
    .llm(llm_for_reflection) // LLM that generates improved instructions
    .build()?;
```

**Parent selection strategies:**
- **CurrentBest**: Greedy selection of best instruction (exploitation)
- **Pareto**: Probabilistic sampling from Pareto frontier (exploration + exploitation)

**Use case example:** Optimize instructions for complex reasoning tasks where understanding failure modes helps generate better prompts. The LLM sees feedback like "Reasoning was correct but answer format was wrong" and proposes improved instructions.

---

### BootstrapOptuna

**Description:** Automatic hyperparameter tuning using Bayesian optimization to find optimal configuration.

**What it optimizes:**
- Number of few-shot demonstrations (`max_demos`)
- Demonstration pool size (`max_bootstrapped_demos`)
- Can be extended to: temperature, top_p, max_tokens, learning_rate

**How it works:**
1. Defines search space for hyperparameters
2. Uses Bayesian optimization (Optuna) to efficiently explore space
3. Evaluates candidate configurations on validation set
4. Returns optimal hyperparameters that maximize metric

**Key parameters:**
```rust
BootstrapOptuna::new()
    .with_num_candidate_programs(20)      // Number of trials
    .with_max_bootstrapped_demos(30)      // Max pool size to test
    .with_max_demos(10)                   // Max demos to test
    .with_random_seed(42)                 // Reproducibility
```

**When to use:**
- You're unsure what hyperparameters work best
- Want to avoid manual trial-and-error
- Have computational budget for multiple trials
- Need systematic exploration of configuration space

**Example:**
```rust
let optimizer = BootstrapOptuna::new()
    .with_num_candidate_programs(20)
    .with_max_bootstrapped_demos(30)
    .with_max_demos(10);

let (result, selected_demos) = optimizer
    .optimize(&node, &trainset, &valset, &metric)
    .await?;

println!("Optimal config: {} demos", selected_demos.len());
println!("Score: {:.3}", result.final_score);
```

**Key insight:** Bayesian optimization is smarter than grid search - it learns from previous trials to focus on promising regions of the search space.

**See:** `examples/auto_hyperparameter_tuning.rs`

---

### GRPO (Group Relative Policy Optimization)

**Description:** Reinforcement learning optimizer that fine-tunes model weights (not just prompts) based on reward signals.

**What it optimizes:**
- Model weights through gradient updates
- Uses policy gradient methods with group-relative reward normalization

**How it works:**
1. Generates multiple rollouts (completions) per example
2. Computes rewards for each completion using metric function
3. Normalizes rewards within each group (reduces variance)
4. Computes policy gradients
5. Updates model weights to maximize expected reward

**Key parameters:**
```rust
let config = GrpoConfig {
    num_rollouts: 4,         // Completions per example
    learning_rate: 1e-5,     // Conservative for stability
    num_iterations: 10,      // RL training iterations
    min_reward: 0.3,         // Reward clipping
    max_reward: 1.0,
};

let grpo = GrpoOptimizer::new(config);
```

**When to use:**
- Complex reasoning tasks where prompt engineering isn't enough
- Need model adaptation to specific domain
- Have compute resources for model fine-tuning
- Want cutting-edge performance (state-of-the-art for complex tasks)

**Example:**
```rust
let mut grpo = GrpoOptimizer::new(config);

// Optimize node through RL fine-tuning
let result = grpo
    .optimize_node(&mut node, &trainset, &valset, &metric)
    .await?;

println!("RL optimization complete!");
println!("Initial: {:.3} → Final: {:.3}",
         result.initial_score, result.final_score);
```

**Key insight:** GRPO's group-relative normalization makes training more stable by normalizing rewards within each example's rollout group rather than globally. This reduces variance and improves convergence.

**Research:** Based on "Grounded Reference Policy Optimization" (Arbor paper)

**See:** `examples/grpo_multihop_qa.rs`

---

### BootstrapFinetune

**Description:** Collects execution traces from DashStream and exports them as fine-tuning datasets for model training.

**What it optimizes:**
- Cost: Distill large model (GPT-4) knowledge into smaller model (GPT-3.5)
- Performance: Create specialized models from production data

**How it works:**
1. Collects traces from DashStream (graph execution logs)
2. Filters successful executions (discards failures)
3. Converts to fine-tuning format (OpenAI JSONL, etc.)
4. Exports dataset for model training

**Key parameters:**
```rust
// Trace collection happens automatically via DashStream
// BootstrapFinetune reads from Kafka topic

let collector = TraceCollector::new("localhost:9092", "traces-topic").await?;
let traces = collector.collect(Duration::from_secs(60)).await?;

// Filter and export
let successful = traces.into_iter()
    .filter(|t| matches!(t.outputs, PredictionOrFailed::Success(_)))
    .collect();

export_to_jsonl(successful, "finetune_dataset.jsonl")?;
```

**When to use:**
- High-volume production application (cost matters)
- Want to distill expensive model into cheaper one
- Have successful execution traces to learn from
- Need specialized model for specific domain

**Cost savings example:**
- Before: GPT-4 @ $30/1M tokens
- After: Fine-tuned GPT-3.5 @ $2/1M tokens
- Savings: 93% reduction ($28/month per 1M tokens)

**Integration with DashStream:**
BootstrapFinetune leverages existing DashStream infrastructure:
- DashStream logs all graph executions to Kafka
- No runtime overhead (logging happens anyway)
- Persistent storage (can replay/reprocess traces)
- Scalable (Kafka handles high volume)

**Example:**
```rust
// Traces collected automatically by DashStream
// Read from Kafka and filter
let traces = collect_traces_from_dashstream().await?;
let successful = filter_successful(traces);

// Convert to OpenAI format
let dataset = convert_to_openai_jsonl(successful)?;

// Export
std::fs::write("finetune_dataset.jsonl", dataset)?;

// Upload to OpenAI
// openai api fine_tunes.create -t finetune_dataset.jsonl
```

**See:** `examples/finetune_dataset_export.rs`

---

### BetterTogether

**Description:** Meta-optimizer that composes multiple optimizers into powerful optimization pipelines.

**What it optimizes:**
- Combines different optimization strategies
- Allows experimentation with optimizer sequences
- Tracks intermediate results for analysis

**How it works:**
1. Define pipeline of optimizers (e.g., FewShot → Optuna → GRPO)
2. Execute sequentially: output of one becomes input to next
3. Track intermediate scores and improvements
4. Return final optimized node + pipeline metrics

**Composition strategies:**
- **Sequential**: Run optimizers one after another (implemented)
- **Parallel**: Run all, pick best (implemented via `Ensemble::builder().with_size(k)`)
- **Ensemble**: Combine results with reduce function (implemented via `Ensemble::builder().with_reduce_fn()`)

**Example:**
```rust
let mut pipeline = BetterTogether::new()
    .add_optimizer(Box::new(BootstrapFewShot::new()
        .with_max_demos(5)))
    .add_optimizer(Box::new(BootstrapOptuna::new()
        .with_num_candidate_programs(10)))
    .add_optimizer(Box::new(MIPROv2::new()))
    .with_strategy(CompositionStrategy::Sequential);

let result = pipeline
    .optimize(&mut node, &trainset, &valset, &metric)
    .await?;

// Inspect pipeline stages
for stage in pipeline.pipeline_stages() {
    println!("{}: {:.3} → {:.3} (+{:.3})",
             stage.optimizer_name,
             stage.initial_score,
             stage.final_score,
             stage.improvement);
}
```

**When to use:**
- Experimenting with different optimization strategies
- Want compound improvements (multiple optimizers)
- Need to understand which optimizers help most
- Building custom optimization workflows

**Recommended pipelines:**

1. **Quick Bootstrap:**
   ```
   BootstrapFewShot → BootstrapOptuna
   ```
   Best for: Fast iteration, limited data

2. **Full Optimization:**
   ```
   BootstrapFewShot → MIPROv2 → GRPO
   ```
   Best for: Maximum quality, production workloads

3. **Cost Optimization:**
   ```
   BootstrapFewShot → COPRO → BootstrapFinetune
   ```
   Best for: High-volume applications, budget constraints

**Key insight:** Optimizer order matters! Different sequences yield different results. BetterTogether lets you experiment systematically to find the best pipeline for your task.

**See:** `examples/optimizer_composition.rs`

---

## Modules In-Depth

### ChainOfThought

Adds explicit reasoning step before final answer.

**Signature transformation:**
```
question -> answer
↓
question -> reasoning, answer
```

```rust
use dashflow::optimize::modules::ChainOfThought;

let cot_node = ChainOfThought::new(
    base_signature,
    llm,
    "cot_reasoner"
);

// Automatically adds 'reasoning' output field
// LLM generates reasoning, then answer
```

**Use cases:**
- Math problems
- Complex reasoning tasks
- Debugging LLM logic

### ReAct

Tool-using agent with iterative Thought→Action→Observation loop.

```rust
use dashflow::optimize::modules::{ReAct, Tool, SimpleTool};

// Define tools
let search_tool = SimpleTool::new(
    "search",
    "Search the web for information",
    |query: &str| async move {
        Ok(search_web(query).await?)
    }
);

let react_node = ReAct::new(
    signature,
    llm,
    vec![Arc::new(search_tool)],
    "react_agent"
);

// Executes: Thought → Action(tool) → Observation → ... → Final Answer
```

**Use cases:**
- Research agents
- Data retrieval tasks
- Multi-step problem solving

### MultiChainComparison

Self-consistency via multiple reasoning attempts.

**Algorithm:**
1. Sample M completions (with temperature > 0)
2. Compare reasoning paths
3. Synthesize final answer

```rust
use dashflow::optimize::modules::MultiChainComparison;

let mc_node = MultiChainComparison::new(
    signature,
    llm,
    3, // M=3 reasoning attempts
    "multi_chain"
);

// Signature extended with reasoning_attempt_1, reasoning_attempt_2, reasoning_attempt_3
// Final signature: reasoning_attempt_1, reasoning_attempt_2, reasoning_attempt_3 -> rationale, answer
```

**Use cases:**
- High-stakes decisions
- Ambiguous problems
- Reducing LLM variance

---

## Graph Optimization

Optimize multiple nodes in a DashFlow workflow together.

### Why Graph Optimization?

**Problem:** Optimizing nodes independently ignores interactions.

**Example:**
- Node A (research) → Node B (summarize) → Node C (answer)
- Optimizing each node separately may not optimize end-to-end quality

**Solution:** GraphOptimizer considers node interactions and optimizes for final output quality.

### GraphOptimizer

```rust
use dashflow::optimize::{GraphOptimizer, OptimizationStrategy};

let graph_optimizer = GraphOptimizer::new(
    end_to_end_metric, // Metric on final output
    OptimizationStrategy::Sequential, // Or Joint, Alternating
);

let optimized_graph = graph_optimizer
    .optimize_graph(graph, trainset)
    .await?;
```

**Strategies:**
- **Sequential**: Optimize nodes in execution order (A → B → C)
- **Joint**: Optimize all nodes simultaneously (considers interactions)
- **Alternating**: Alternate between nodes, multiple passes

**Use cases:**
- Multi-node workflows
- Complex reasoning pipelines
- Production applications with multiple stages

---

## Production Features

### A/B Testing

Compare multiple optimized variants in production.

```rust
use dashflow::optimize::ab_testing::{ABTest, Variant, TrafficSplitStrategy};

// Create variants
let variant_a = Variant::new("baseline", node_baseline);
let variant_b = Variant::new("optimized", node_optimized);

// Create A/B test
let ab_test = ABTest::builder()
    .add_variant(variant_a)
    .add_variant(variant_b)
    .traffic_strategy(TrafficSplitStrategy::Percentage {
        splits: vec![("baseline", 50.0), ("optimized", 50.0)]
    })
    .metric(metric)
    .build()?;

// Route traffic
let result = ab_test.run(state).await?;

// Analyze results
let report = ab_test.analyze(100)?; // Min 100 samples
println!("Winner: {}, p-value: {:.4}", report.winner, report.p_value);
```

**Features:**
- Traffic splitting (percentage, user-based)
- Statistical significance testing (t-test, chi-square)
- Automatic winner detection
- Report generation

### Cost Monitoring

Track LLM API costs during optimization and production.

> **Note:** The `dashflow::optimize::cost_monitoring` module is deprecated.
> Use `dashflow_observability::cost` instead. See the
> [Migration Guide](MIGRATION_GUIDE.md#cost-monitoring-v1113) for details.

```rust
use dashflow_observability::cost::{
    CostTracker, ModelPricing, Pricing, BudgetConfig, BudgetEnforcer
};

// Create tracker with custom pricing
let pricing = ModelPricing::new()
    .with_model("gpt-4o", Pricing::per_1m(2.50, 10.00))
    .with_model("gpt-4o-mini", Pricing::per_1m(0.15, 0.60));
let mut tracker = CostTracker::new(pricing);

// Or use comprehensive defaults (OpenAI, Anthropic, Google models)
let mut tracker = CostTracker::with_defaults();

// Record LLM calls
tracker.record_llm_call("gpt-4o", 1500, 800, Some("research_node"))?;

// Get cost report
let report = tracker.report();
println!("Spent today: ${:.4}", report.spent_today);
println!("Total cost: ${:.4}", report.total_cost());

// With budget enforcement
let config = BudgetConfig::with_daily_limit(100.0)  // $100/day
    .warning_threshold(0.8)   // Warn at 80%
    .enforce_hard_limit(true); // Block requests when exceeded

let enforcer = BudgetEnforcer::new(
    CostTracker::with_defaults(),
    config
);
enforcer.record_and_check("gpt-4o", 1000, 500)?;
```

**Features:**
- Real-time cost tracking with per-model pricing
- Budget limits with hard enforcement
- Per-model, per-node, per-user, and per-session cost breakdowns
- Prometheus metrics export
- Comprehensive multi-provider pricing database

### Data Collection

Collect production data for continuous optimization.

```rust
use dashflow::optimize::data_collection::{
    DataCollector,
    SamplingStrategy,
    CollectionConfig
};

let collector = DataCollector::new(
    CollectionConfig {
        sampling_strategy: SamplingStrategy::Percentage(0.1), // 10% sampling
        storage_backend: StorageBackend::File("data/production.jsonl"),
        max_samples: Some(10_000),
    }
);

// Collect during execution
collector.record(state, prediction, score).await?;

// Analyze collected data
let analysis = collector.analyze()?;
println!("Class balance: {:?}", analysis.class_distribution);
println!("Average score: {:.2}", analysis.average_score);

// Export for re-optimization
let new_trainset = collector.export_trainset()?;
```

**Features:**
- Flexible sampling strategies (percentage, time-based, quality-based)
- Class balance analysis
- Automatic data export
- Integration with optimizers

---

## Model Distillation

Distill large models (GPT-4) into smaller, cheaper models (GPT-3.5, local models).

### Workflow

1. **Teacher model** (GPT-4) generates synthetic training data
2. **Student model** (GPT-3.5, local) trained on synthetic data
3. **Evaluation** compares teacher vs student quality
4. **ROI analysis** measures cost savings

### Example

```rust
use dashflow::optimize::distillation::{
    TeacherModel,
    StudentModel,
    DistillationConfig
};

// 1. Generate synthetic data with teacher
let teacher = TeacherModel::new(gpt4_client);
let synthetic_data = teacher.generate_trainset(
    seed_examples,
    1000 // Generate 1000 examples
).await?;

// 2. Train student (OpenAI fine-tuning)
let student = StudentModel::openai_finetune(
    "gpt-3.5-turbo",
    synthetic_data
);
let fine_tuned_model = student.train().await?;

// 3. Evaluate
let teacher_score = evaluate(&teacher, testset, metric).await?;
let student_score = evaluate(&fine_tuned_model, testset, metric).await?;

println!("Teacher: {:.1}%, Student: {:.1}%",
    teacher_score * 100.0,
    student_score * 100.0
);

// 4. Cost analysis
let teacher_cost_per_1k = 0.60; // GPT-4 cost
let student_cost_per_1k = 0.002; // Fine-tuned GPT-3.5 cost
let savings = (teacher_cost_per_1k - student_cost_per_1k) / teacher_cost_per_1k;
println!("Cost savings: {:.1}%", savings * 100.0);
```

**Approaches:**
- **OpenAI Fine-tuning**: API-based, easiest
- **Local Fine-tuning**: MLX + Ollama, full control *(placeholder - requires external setup)*
- **Prompt Optimization**: Use BootstrapFewShot with student model

---

## Metrics

Evaluation metrics for optimization.

### Built-in Metrics

```rust
use dashflow::optimize::{exact_match, f1_score, precision, recall};

// Exact match (0.0 or 1.0)
exact_match("positive", "positive"); // 1.0
exact_match("positive", "negative"); // 0.0

// F1 score (token-level)
f1_score("the quick brown fox", "the fast brown fox"); // ~0.8

// Precision / Recall
precision("the quick brown", "the brown"); // 1.0 (all predicted tokens correct)
recall("the quick brown", "the brown"); // 0.67 (2/3 reference tokens found)
```

### Custom Metrics

```rust
use dashflow::optimize::MetricFn;

let custom_metric: MetricFn<MyState> = Arc::new(|expected, predicted| {
    // Your metric logic
    let score = compute_similarity(expected, predicted);
    Ok(score)
});
```

### Multi-Objective Optimization

Optimize for multiple metrics simultaneously.

```rust
use dashflow::optimize::multi_objective::{
    MultiObjectiveOptimizer,
    Objective,
    ParetoFront
};

let optimizer = MultiObjectiveOptimizer::new(vec![
    Objective::new("accuracy", accuracy_metric, 1.0), // Weight 1.0
    Objective::new("cost", cost_metric, 0.5), // Weight 0.5 (less important)
    Objective::new("latency", latency_metric, 0.3), // Weight 0.3
]);

let results = optimizer.optimize(node, trainset).await?;

// Get Pareto frontier (trade-off curves)
let frontier = results.pareto_front();
for point in frontier.points() {
    println!("Accuracy: {:.2}, Cost: ${:.2}, Latency: {}ms",
        point.metrics["accuracy"],
        point.metrics["cost"],
        point.metrics["latency"]
    );
}
```

---

## Best Practices

### 1. Start Simple, Iterate

```
BootstrapFewShot → LabeledFewShot → MIPROv2 → Ensemble
```

- Start with BootstrapFewShot (fastest)
- If not enough, try MIPROv2 (instruction optimization)
- For production, consider Ensemble (robustness)

### 2. Training Data Quality > Quantity

- 50 high-quality examples > 500 noisy examples
- Ensure examples cover input distribution
- Balance classes if classification task

### 3. Metric Choice Matters

- Use task-specific metrics (exact match for classification, F1 for generation)
- Consider multiple metrics (quality + cost + latency)
- Validate metrics correlate with human judgment

### 4. Graph Optimization for Multi-Node Workflows

- Always use GraphOptimizer for multi-node graphs
- Choose strategy based on node dependencies:
  - Sequential: Linear pipelines (A → B → C)
  - Joint: Complex interactions
  - Alternating: Best quality, slower

### 5. Production Monitoring

- Enable cost monitoring during optimization
- Use A/B testing before full rollout
- Collect production data for continuous improvement

### 6. Distillation for Cost Savings

- Distill to smaller models once quality is stable
- Measure cost/quality trade-off explicitly
- Consider local models (Ollama) for sensitive data

---

## API Reference

### Core Types

```rust
// Signature creation
pub fn make_signature(spec: &str, instructions: &str) -> Result<Signature>

// LLMNode
pub struct LLMNode<S: GraphState> { ... }
impl<S: GraphState> LLMNode<S> {
    pub fn new(signature: Signature, llm: Arc<dyn ChatModel>, name: &str) -> Self
}

// Optimizer trait
pub trait Optimizer<S: GraphState> {
    async fn optimize(&self, node: &mut impl Optimizable<S>, trainset: &[S])
        -> Result<OptimizationResult>;
}

// OptimizerConfig
pub struct OptimizerConfig {
    pub max_few_shot_examples: usize,
    pub max_iterations: usize,
    pub min_improvement: f64,
    pub random_seed: Option<u64>,
}
```

### Optimizers

```rust
// BootstrapFewShot
pub struct BootstrapFewShot<S: GraphState> { ... }
impl<S: GraphState> BootstrapFewShot<S> {
    pub fn new(config: OptimizerConfig, metric: MetricFn<S>) -> Self
}

// LabeledFewShot
pub struct LabeledFewShot<S: GraphState> { ... }
impl<S: GraphState> LabeledFewShot<S> {
    pub fn builder() -> LabeledFewShotBuilder<S>
}

// MIPROv2
pub struct MIPROv2<S: GraphState> { ... }
impl<S: GraphState> MIPROv2<S> {
    pub fn builder() -> MIPROv2Builder<S>
}

// See docs.rs for complete API
```

---

## Examples

### End-to-End Example: Question Answering Optimization

See: `crates/dashflow/examples/bootstrap_fewshot_example.rs`

### Multi-Node Graph Optimization

See: `crates/dashflow/examples/graph_optimization_example.rs`

### A/B Testing in Production

See: `crates/dashflow/examples/ab_testing_example.rs`

### Cost Monitoring

See: `crates/dashflow/examples/cost_monitoring_example.rs`

### Model Distillation

See: `crates/dashflow/examples/model_distillation_example.rs`

---

## Performance

**Optimization Speed:**
- BootstrapFewShot: ~30s for 50 examples (3 iterations)
- MIPROv2: ~5-10min for 50 examples (auto mode)
- GraphOptimizer: ~2-5min per node (sequential strategy)

**Quality Improvements (Typical):**
- BootstrapFewShot: +10-20% accuracy
- MIPROv2: +20-40% accuracy
- Ensemble: +5-10% accuracy over best single

**Cost Savings (Distillation):**
- GPT-4 → Fine-tuned GPT-3.5: ~99.7% cost reduction
- Quality retention: ~85-95% of teacher performance

---

## Troubleshooting

### Q: Optimization not improving quality

**Solutions:**
1. Check metric is correct (matches task)
2. Increase training data size (min 30-50 examples)
3. Try different optimizer (MIPROv2 vs BootstrapFewShot)
4. Check LLM capability (GPT-4 vs GPT-3.5)

### Q: Optimization too slow

**Solutions:**
1. Reduce max_iterations (default 3)
2. Use BootstrapFewShot instead of MIPROv2
3. Reduce training set size for initial experiments
4. Use faster LLM (GPT-3.5 vs GPT-4)

### Q: GraphOptimizer fails with errors

**Solutions:**
1. Ensure all nodes implement Optimizable trait
2. Check state types match across nodes
3. Verify metric applies to final output state
4. Try Sequential strategy first (simplest)

### Q: Budget exceeded during optimization

**Solutions:**
1. Enable cost monitoring with budget limit
2. Reduce max_iterations
3. Use smaller trainset
4. Use cheaper LLM for proposal (COPRO/MIPROv2)

---

## Testing

### Integration Tests

DashOptimize includes comprehensive integration tests in two categories:

#### 1. Mock-Based Tests (Fast, CI-Friendly)

Located in: `crates/dashflow/tests/phase2b_optimizer_integration.rs`

These tests use mock nodes and optimizers to verify optimizer composition, error handling, and integration patterns without making real API calls.

```bash
# Run all mock-based tests
cargo test --package dashflow --test phase2b_optimizer_integration
```

#### 2. Real End-to-End Tests (Manual, Real API Calls)

Located in: `crates/dashflow/tests/phase2b_real_e2e_tests.rs`

These tests make **REAL API calls to OpenAI** to verify that optimizers work correctly with actual LLMs. They are marked with `#[ignore]` to prevent accidental execution in CI (costs money).

**Tests included:**
1. `test_bootstrap_fewshot_real_openai` - BootstrapFewShot with real sentiment classification
2. `test_grpo_reward_computation_real` - GRPO rollout generation and reward normalization
3. `test_bootstrap_finetune_trace_structure` - Fine-tuning dataset export validation
4. `test_full_optimization_pipeline_real` - Complete optimization workflow

**How to run:**

```bash
# 1. Set your OpenAI API key
export OPENAI_API_KEY="sk-proj-..."

# Or load from .env file
source .env

# 2. Run specific test
cargo test --package dashflow \
  --test phase2b_real_e2e_tests \
  test_bootstrap_fewshot_real_openai \
  --ignored \
  -- --nocapture

# 3. Run all real E2E tests
cargo test --package dashflow \
  --test phase2b_real_e2e_tests \
  --ignored \
  -- --nocapture
```

**What these tests verify:**
- Real OpenAI API integration works correctly
- BootstrapFewShot generates useful few-shot examples from training data
- GRPO reward computation and group relative normalization work correctly
- Fine-tuning dataset export produces valid OpenAI JSONL format
- Complete optimization pipelines improve model performance

**Cost:** Each test makes 5-15 API calls to `gpt-4o-mini` (~$0.01-0.03 per test run)

### Unit Tests

Each optimizer has comprehensive unit tests covering:
- Configuration and initialization
- Optimization logic
- Error handling
- Edge cases

Run all unit tests:
```bash
cargo test --package dashflow
```

---

## Contributing

DashOptimize is part of DashFlow. Contributions welcome!

**Areas for contribution:**
- New optimizers (LIPO, OPRO, etc.)
- Additional metrics (BLEU, ROUGE, semantic similarity)
- More modules (ProgramOfThought, etc.)
- Performance improvements
- Documentation and examples

See: [CONTRIBUTING_DOCS.md](CONTRIBUTING_DOCS.md)

---

## References

### Papers

- **DashOptimize** (Stanford): "Programming—not prompting—Foundation Models" (arXiv:2310.03714)
- **COPRO**: "Large Language Models as Optimizers" (arXiv:2309.03409)
- **MIPROv2**: "Optimizing Instructions and Demonstrations for Multi-Stage Tasks" (arXiv:2406.11695)

### Related Projects

- **DashOptimize** (Python): https://github.com/stanfordnlp/dashoptimize
- **DashFlow** (Python): https://github.com/dashflow-ai/dashflow
- **DashFlow** (Python): https://github.com/dashflow-ai/dashflow

---

## License

MIT License - See LICENSE file

---

© 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)
