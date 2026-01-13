# Self-Improvement Module Architecture

**Last Updated:** 2025-12-15

## Overview

The self-improvement module provides AI-driven introspection, analysis, and improvement capabilities for DashFlow applications. It enables the system to learn from execution traces, identify improvement opportunities, and generate actionable plans.

## Module Summary

Total: 30 modules, ~35,000 lines of code

### Core Modules

| Module | Lines | Purpose | CLI |
|--------|-------|---------|-----|
| `types.rs` | 3,725 | Core data structures (reports, plans, hypotheses) | - |
| `storage.rs` | 4,169 | Persistent storage for introspection data | - |
| `traits.rs` | 1,030 | Extensibility traits (Analyzer, Planner, Storable) | - |
| `error.rs` | 263 | Error types | - |
| `config.rs` | 626 | Configuration management | - |

### Analysis Modules

| Module | Lines | Purpose | CLI |
|--------|-------|---------|-----|
| `analyzers.rs` | 1,906 | Gap detection, deprecation analysis | `analyze` |
| `planners.rs` | 1,467 | Plan generation from analysis | `analyze` |
| `consensus.rs` | 1,451 | Multi-model validation | `analyze --depth deep` |
| `meta_analysis.rs` | 2,002 | Cross-execution pattern analysis | `analyze` |
| `integration.rs` | 2,248 | Main analysis orchestration | `analyze` |

### Runtime Modules

| Module | Lines | Purpose | CLI |
|--------|-------|---------|-----|
| `daemon.rs` | 2,248 | Background analysis daemon | `daemon` |
| `streaming_consumer.rs` | 623 | Kafka integration (library-only) | - |
| `parallel_analysis.rs` | 382 | Parallel trace processing (library-only) | - |
| `test_generation.rs` | 696 | Auto-generate regression tests | `generate-tests` |

### Observability Modules

| Module | Lines | Purpose | CLI |
|--------|-------|---------|-----|
| `alerts.rs` | 1,193 | Alert management and routing | `daemon --alert-*` |
| `events.rs` | 1,063 | Event pub/sub system | - |
| `logging.rs` | 550 | Structured logging | - |
| `metrics.rs` | 538 | Prometheus metrics | - |

### Resilience Modules

| Module | Lines | Purpose | CLI |
|--------|-------|---------|-----|
| `circuit_breaker.rs` | 1,018 | Prevent cascade failures | - |
| `rate_limiter.rs` | 601 | Throttle analysis frequency | - |
| `cache.rs` | 415 | Cached analysis results | - |
| `lazy_loading.rs` | 387 | Deferred data loading | - |

### Support Modules

| Module | Lines | Purpose | CLI |
|--------|-------|---------|-----|
| `health.rs` | 712 | Health checks and diagnostics | `introspect health` |
| `audit.rs` | 876 | Audit trail for changes | - |
| `redaction.rs` | 839 | PII/secret redaction | - |
| `trace_retention.rs` | 771 | Trace lifecycle management | - |
| `export_import.rs` | 880 | Data portability | - |
| `plugins.rs` | 696 | Plugin loading system | - |
| `testing.rs` | 993 | Test utilities | - |

## CLI Command Mapping

```
dashflow self-improve
├── analyze           # Run introspection analysis
│   ├── --depth       # metrics, local, deep (full)
│   ├── --reason      # Analysis trigger reason
│   ├── --storage     # Custom storage path
│   └── --format json # JSON output
│
├── plans             # List improvement plans
│   ├── --status      # pending, approved, implemented
│   ├── --storage     # Custom storage path
│   └── --format json # JSON output
│
├── approve           # Approve a plan
│   ├── <plan-id>     # UUID of plan to approve
│   ├── --assignee    # Implementation assignee
│   └── --storage     # Custom storage path
│
├── daemon            # Background analysis
│   ├── --interval    # Analysis interval (seconds)
│   ├── --storage     # Custom storage path
│   ├── --once        # Single cycle mode
│   ├── --alert-file  # Alert log file
│   ├── --alert-webhook  # Webhook URL
│   └── --no-console  # Disable console alerts
│
└── generate-tests    # Auto-generate tests
    ├── --limit       # Max tests to generate
    ├── --format json # JSON output format
    ├── --output      # Output file path
    ├── --traces      # Custom traces directory
    └── --timing      # Include timing assertions
```

## Data Flow

```
ExecutionTrace (from .dashflow/traces/)
         │
         ▼
┌─────────────────────┐
│     Analyzers       │ ← Gap detection, deprecation analysis
└─────────────────────┘
         │
         ▼
┌─────────────────────┐
│     Planners        │ ← Generate improvement proposals
└─────────────────────┘
         │
         ▼
┌─────────────────────┐
│     Consensus       │ ← Multi-model validation (optional)
└─────────────────────┘
         │
         ▼
┌─────────────────────┐
│ IntrospectionReport │ ← Stored in .dashflow/introspection/
└─────────────────────┘
         │
         ▼
┌─────────────────────┐
│   ExecutionPlan     │ ← Actionable improvement steps
└─────────────────────┘
```

## Extension Points

### Custom Analyzers

```rust
use dashflow::self_improvement::{Analyzer, AnalyzerContext, AnalysisOutput};

pub struct MyAnalyzer;

impl Analyzer for MyAnalyzer {
    fn name(&self) -> &str { "my-analyzer" }
    fn analyze(&self, traces: &[ExecutionTrace], ctx: &AnalyzerContext)
        -> Result<AnalysisOutput> { ... }
}
```

### Custom Planners

```rust
use dashflow::self_improvement::{Planner, PlannerInput, PlannerOutput};

pub struct MyPlanner;

impl Planner for MyPlanner {
    fn name(&self) -> &str { "my-planner" }
    fn generate(&self, input: PlannerInput) -> Result<PlannerOutput> { ... }
}
```

### Custom Storage Backends

```rust
use dashflow::self_improvement::{StorageBackend, Result};

pub struct RedisStorage { ... }

impl StorageBackend for RedisStorage {
    fn save_report(&self, report: &IntrospectionReport) -> Result<()> { ... }
    fn load_report(&self, id: &str) -> Result<IntrospectionReport> { ... }
    // ... other methods
}
```

## Storage Layout

```
.dashflow/introspection/
├── reports/
│   ├── 2025-12-15T10-30-00_abc123.json
│   └── 2025-12-15T10-30-00_abc123.md
├── plans/
│   ├── pending/
│   ├── approved/
│   ├── implemented/
│   └── failed/
├── hypotheses/
│   ├── active/
│   └── evaluated/
└── meta/
    ├── patterns.json
    └── momentum.json
```

## See Also

- [CLAUDE.md](../../../CLAUDE.md) - Project-wide AI worker guidelines
- [ROADMAP_CURRENT.md](../../../ROADMAP_CURRENT.md) - Active development roadmap
- [mod.rs](mod.rs) - Module re-exports and main documentation
