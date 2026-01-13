# DashOptimize: Optimizer Selection Guide

## Quick Reference

| Scenario | Recommended Optimizer | Why |
|----------|----------------------|-----|
| **Quick prototyping, ~10 examples** | `BootstrapFewShot` | Fast, minimal data |
| **Production optimization, 50+ examples** | `MIPROv2` | Best benchmarked (5/7 tasks, 13% gains) |
| **Model finetuning available** | `GRPO` | Actual RL weight updates |
| **Self-improving agents** | `SIMBA` | Introspective improvement |
| **Instruction-only optimization** | `COPRO` | No few-shot, just instructions |
| **Distillation to smaller model** | `BootstrapFinetune` | Weight transfer |
| **Genetic/evolutionary approach** | `GEPA` | Population-based search |
| **Gradient-free prompt search** | `AutoPrompt` | Discrete token optimization |

## Tier 1: Recommended Defaults

### MIPROv2 (Best Benchmarked)
- **Use when**: Complex multi-stage programs, 50+ training examples
- **Requirements**: ≥2 training examples, prompt model + task model
- **Cannot use when**: Zero training data
- **Benchmark**: 5/7 tasks, up to 13% accuracy improvement
- **Citation**: [arxiv:2406.11695](https://arxiv.org/abs/2406.11695) (Khattab et al., EMNLP 2024)

### BootstrapFewShot (Quick Start)
- **Use when**: Getting started, limited data (~10 examples)
- **Requirements**: ~10+ examples, teacher model
- **Cannot use when**: Zero examples
- **Benchmark**: Foundation of DSPy, proven reliable
- **Citation**: [arxiv:2310.03714](https://arxiv.org/abs/2310.03714) (DSPy)

### GRPO (Finetuning)
- **Use when**: You can finetune the model, need RL-based optimization
- **Requirements**: LLM with RL/finetuning API (open-weight models)
- **Cannot use when**: API-only models (GPT-4, Claude)
- **Benchmark**: 51.7% → 60.9% on MATH benchmark
- **Citation**: [arxiv:2402.03300](https://arxiv.org/abs/2402.03300) (DeepSeek-R1)

## Tier 2: Specialized

### SIMBA
- **Use when**: Need adaptive, self-improving optimization
- **Requirements**: Metric function, sufficient batch size
- **Key feature**: Analyzes its own trajectories to improve
- **Citation**: [DSPy teleprompt](https://github.com/stanfordnlp/dspy)

### COPRO
- **Use when**: Instruction-only optimization (no few-shot needed)
- **Requirements**: Metric function, structured tasks
- **Key feature**: Iterative instruction refinement
- **Citation**: [arxiv:2310.03714](https://arxiv.org/abs/2310.03714) (DSPy)

### COPROv2
- **Use when**: COPRO with confidence-based scoring
- **Requirements**: Same as COPRO
- **Key feature**: Extended COPRO with improved candidate selection
- **Citation**: [arxiv:2310.03714](https://arxiv.org/abs/2310.03714) (DSPy extension)

### BootstrapFinetune
- **Use when**: Distilling large model to smaller model
- **Requirements**: Finetunable target model
- **Cannot use when**: API-only models
- **Citation**: [arxiv:2310.03714](https://arxiv.org/abs/2310.03714) (DSPy)

## Tier 3: Niche

| Optimizer | Use Case | Citation |
|-----------|----------|----------|
| `RandomSearch` | Simple baseline exploration | [arxiv:2310.03714](https://arxiv.org/abs/2310.03714) |
| `GEPA` | Genetic/evolutionary optimization | [arxiv:2507.19457](https://arxiv.org/abs/2507.19457) |
| `Ensemble` | Combining multiple program variants | Standard ensemble learning |
| `KNNFewShot` | Example selection via embeddings | [arxiv:2310.03714](https://arxiv.org/abs/2310.03714) |
| `LabeledFewShot` | Direct labeled example use | [arxiv:2310.03714](https://arxiv.org/abs/2310.03714) |
| `BetterTogether` | Meta-optimization strategies | [DSPy teleprompt](https://github.com/stanfordnlp/dspy) |
| `BootstrapOptuna` | Optuna-backed hyperparameter search | DSPy + [Optuna](https://arxiv.org/abs/1907.10902) |
| `AutoPrompt` | Gradient-free discrete prompt search | [arxiv:2010.15980](https://arxiv.org/abs/2010.15980) (Shin et al., 2020) |

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
Start
  │
  ├─ Can you finetune the model?
  │     ├─ Yes → GRPO (best for model weight optimization)
  │     └─ No → Continue
  │
  ├─ How much training data?
  │     ├─ <10 examples → BootstrapFewShot
  │     ├─ 10-50 examples → BootstrapFewShot or MIPROv2
  │     └─ 50+ examples → MIPROv2 (recommended)
  │
  ├─ Need self-improving behavior?
  │     └─ Yes → SIMBA
  │
  ├─ Need interpretable optimization?
  │     └─ Yes → COPRO (instruction refinement)
  │
  └─ Default → MIPROv2
```

## Complete Optimizer Reference Table

| Optimizer | Type | Min Examples | Requires | Citation |
|-----------|------|--------------|----------|----------|
| [`MIPROv2`] | Instruction + Few-shot | 2 | prompt_model, task_model | arxiv:2406.11695 |
| [`BootstrapFewShot`] | Few-shot | 10 | teacher_model | arxiv:2310.03714 |
| [`GRPO`] | RL Finetuning | 10 | finetunable_model | arxiv:2402.03300 |
| [`SIMBA`] | Self-reflective | 20 | metric_function | DSPy |
| [`COPRO`] | Instruction | 10 | metric_function | arxiv:2310.03714 |
| [`COPROv2`] | Instruction | 10 | metric_function | arxiv:2310.03714 |
| [`GEPA`] | Genetic | 10 | — | arxiv:2507.19457 |
| [`BootstrapFinetune`] | Distillation | 50 | finetunable_model | arxiv:2310.03714 |
| [`RandomSearch`] | Exploration | 50 | — | arxiv:2310.03714 |
| [`Ensemble`] | Combination | 0 | multiple_variants | Standard |
| [`KNNFewShot`] | Example selection | 20 | embedding_model | arxiv:2310.03714 |
| [`LabeledFewShot`] | Direct | 5 | labeled_data | arxiv:2310.03714 |
| [`BetterTogether`] | Meta | 20 | — | DSPy |
| [`BootstrapOptuna`] | Hyperparameter | 50 | optuna | DSPy + Optuna |
| [`AutoPrompt`] | Discrete search | 10 | — | arxiv:2010.15980 |

## Common Patterns

### Pattern 1: RAG Pipeline Optimization
```rust
// Start with BootstrapFewShot to get working demos
let optimizer = BootstrapFewShot::new(metric);
let optimized = optimizer.compile(&signature, &trainset, llm).await?;

// Then scale with MIPROv2 for production
let optimizer = MIPROv2::builder()
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

- [DashOptimize Overview](../README.md)
- [docs/OPTIMIZER_GUIDE.md](../../../../../docs/OPTIMIZER_GUIDE.md) (user-facing documentation)
- Individual optimizer module documentation
