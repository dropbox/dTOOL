# DashOptimize: Complete Optimizer Guide

**Last Updated:** 2026-01-02 (Worker #2319 - Fix missing optimizers in OPTIMIZER_GUIDE)

DashOptimize provides 17 prompt optimization algorithms for improving LLM prompts using training data. All optimizers are ported from DSPy with proper academic citations.

## Quick Start: Which Optimizer Should I Use?

| Scenario | Optimizer | Why |
|----------|-----------|-----|
| Quick prototyping (~10 examples) | `BootstrapFewShot` | Fast, minimal data |
| Production optimization (50+ examples) | `MIPROv2` | Best benchmarked |
| Model finetuning available | `GRPO` | RL weight updates |
| Self-improving agents | `SIMBA` | Introspective |
| Instruction-only | `COPRO` | No few-shot |
| Gradient-free prompt search | `AutoPrompt` | Discrete optimization |

## CLI: Get Recommendations

DashFlow includes a built-in optimizer selection tool:

```bash
# List all optimizers with tier classification
dashflow introspect optimizers

# Get recommendation based on your data size
dashflow introspect optimizers --examples 50

# Get recommendation if you can finetune
dashflow introspect optimizers --examples 50 --can-finetune

# Show details for a specific optimizer
dashflow introspect optimizers --name MIPROv2

# Filter by tier (1=Recommended, 2=Specialized, 3=Niche)
dashflow introspect optimizers --tier 1

# JSON output for automation
dashflow introspect optimizers --format json
```

## Tier 1: Recommended Defaults

### MIPROv2 (Best Benchmarked)

Multi-stage instruction and demonstration optimization with Bayesian search.

- **Use when**: Complex multi-stage programs, 50+ training examples
- **Cannot use when**: Zero training data
- **Min examples**: 2
- **Benchmark**: 5/7 tasks, up to 13% accuracy improvement
- **Citation**: [arxiv:2406.11695](https://arxiv.org/abs/2406.11695) (Khattab et al., EMNLP 2024)

```rust
use dashflow::optimize::optimizers::{MIPROv2, MIPROv2Builder, MetricFn};

let metric: MetricFn = Arc::new(|predicted, expected| {
    // Return 1.0 for match, 0.0 for mismatch
    if predicted.get("answer") == expected.get("answer") { 1.0 } else { 0.0 }
});

let optimizer = MIPROv2Builder::new(metric)
    .num_trials(50)
    .auto_mode(AutoMode::Auto)
    .build()?;

let optimized_signature = optimizer.compile(&signature, &trainset, llm).await?;
```

### BootstrapFewShot (Quick Start)

Generate few-shot examples from successful traces.

- **Use when**: Quick prototyping, limited data (~10 examples)
- **Cannot use when**: Zero examples
- **Min examples**: 10
- **Citation**: [arxiv:2310.03714](https://arxiv.org/abs/2310.03714) (DSPy)

```rust
use dashflow::optimize::optimizers::BootstrapFewShot;

let optimizer = BootstrapFewShot::new(metric);
let optimized = optimizer.compile(&signature, &trainset, llm).await?;
```

### GRPO (Finetuning)

Group Relative Policy Optimization for model finetuning.

- **Use when**: Model finetuning available, RL-based optimization needed
- **Cannot use when**: API-only models (GPT-4, Claude)
- **Min examples**: 10
- **Benchmark**: 51.7% → 60.9% on MATH benchmark
- **Citation**: [arxiv:2402.03300](https://arxiv.org/abs/2402.03300) (DeepSeek-R1)

```rust
use dashflow::optimize::optimizers::{GRPO, GRPOConfig};

let config = GRPOConfig::default()
    .with_num_iterations(100)
    .with_batch_size(32);

let optimizer = GRPO::new(config, metric);
let optimized = optimizer.compile(&signature, &trainset, llm).await?;
```

## Tier 2: Specialized

### SIMBA

Stochastic Introspective Mini-Batch Ascent with self-reflection.

- **Use when**: Need adaptive, self-improving optimization
- **Min examples**: 20
- **Citation**: [DSPy teleprompt](https://github.com/stanfordnlp/dspy)

### COPRO

Collaborative Prompt Optimizer for instruction refinement.

- **Use when**: Instruction-only optimization (no few-shot needed)
- **Min examples**: 10
- **Citation**: [arxiv:2310.03714](https://arxiv.org/abs/2310.03714)

### COPROv2

COPRO with confidence-based scoring.

- **Use when**: COPRO-like optimization with improved candidate scoring
- **Min examples**: 10
- **Citation**: [arxiv:2310.03714](https://arxiv.org/abs/2310.03714)

### BootstrapFinetune

Distill prompt-based program into model weight updates.

- **Use when**: Distilling to smaller/faster model
- **Cannot use when**: API-only models
- **Min examples**: 50
- **Citation**: [arxiv:2310.03714](https://arxiv.org/abs/2310.03714)

### AutoPrompt

Gradient-free discrete prompt search.

- **Use when**: Need gradient-free prompt discovery, token-level optimization
- **Min examples**: 10
- **Benchmark**: Elicits factual knowledge from LMs without fine-tuning
- **Citation**: [arxiv:2010.15980](https://arxiv.org/abs/2010.15980) (Shin et al., 2020)

### Avatar

Adaptive Virtual Agent Training and Refinement through feedback analysis.

- **Use when**: Optimizing AI agent instructions based on execution feedback
- **Use when**: Clear success/failure signal for task completion
- **Cannot use when**: No clear success/failure signal available
- **Min examples**: 10
- **Citation**: [arxiv:2310.03714](https://arxiv.org/abs/2310.03714) (DSPy)

### InferRules

Rule induction optimizer that generates human-readable rules from examples.

- **Use when**: Need interpretable, human-readable optimization output
- **Use when**: Want to extract explicit guidelines from examples
- **Use when**: Need transparent decision rules for auditing
- **Cannot use when**: Rules would be too brittle for the task
- **Min examples**: 10
- **Citation**: [arxiv:2310.03714](https://arxiv.org/abs/2310.03714) (DSPy)

## Tier 3: Niche

| Optimizer | Use Case | Min Examples | Citation |
|-----------|----------|--------------|----------|
| `RandomSearch` | Simple baseline exploration | 50 | arxiv:2310.03714 |
| `GEPA` | Genetic/evolutionary optimization | 10 | arxiv:2507.19457 |
| `Ensemble` | Combining multiple program variants | 0 | Standard |
| `KNNFewShot` | Example selection via embeddings | 20 | arxiv:2310.03714 |
| `LabeledFewShot` | Direct labeled example use | 5 | arxiv:2310.03714 |
| `BetterTogether` | Meta-optimization strategies | 20 | DSPy |
| `BootstrapOptuna` | Optuna-backed hyperparameter search | 50 | DSPy + Optuna |

## Data Requirements

| Data Size | Recommended Path |
|-----------|-----------------|
| 0 examples | Cannot optimize (need at least 2) |
| 2-10 examples | `BootstrapFewShot` |
| 10-50 examples | `BootstrapFewShot` → `MIPROv2` |
| 50-200 examples | `MIPROv2` (best results) |
| 200+ examples | `MIPROv2` with more trials, or `GRPO` if finetuning |

## Decision Tree

```
Can you finetune the model?
  ├─ Yes → GRPO (best for model weight optimization)
  └─ No → How much training data?
            ├─ <10 examples → BootstrapFewShot
            ├─ 10-50 examples → BootstrapFewShot or MIPROv2
            └─ 50+ examples → MIPROv2 (recommended)
```

## Common Patterns

### Pattern 1: RAG Pipeline Optimization

```rust
// Start with BootstrapFewShot to get working demos
let optimizer = BootstrapFewShot::new(metric);
let optimized = optimizer.compile(&signature, &trainset, llm).await?;

// Then scale with MIPROv2 for production
let optimizer = MIPROv2Builder::new(metric)
    .num_trials(100)
    .build()?;
let production = optimizer.compile(&optimized, &trainset, llm).await?;
```

### Pattern 2: Agent Instruction Tuning

```rust
// Use COPRO for instruction-only optimization
let optimizer = COPRO::builder()
    .depth(5)
    .breadth(3)
    .build()?;
let agent_sig = optimizer.compile(&agent_signature, &feedback_data, llm).await?;
```

### Pattern 3: Model Distillation

```rust
// Use BootstrapFinetune to distill GPT-4 to GPT-3.5
let optimizer = BootstrapFinetune::new(
    gpt4_teacher,
    gpt35_student,
    metric,
);
let distilled = optimizer.compile(&trainset).await?;
```

## See Also

- [API Documentation](https://docs.rs/dashflow/latest/dashflow/optimize/optimizers/) (when published)
- [OPTIMIZER_SELECTION.md](../crates/dashflow/src/optimize/optimizers/OPTIMIZER_SELECTION.md) - Developer reference
- [DSPy Paper](https://arxiv.org/abs/2310.03714) - Original framework
