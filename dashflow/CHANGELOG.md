# Changelog

All notable changes to DashFlow will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

## [1.11.3] - 2025-12-09

### Added - Three-Level Live Introspection (N=309-314)

Complete runtime self-awareness for AI agents:

- **Platform Introspection** (N=309) - DashFlow framework capabilities
  - `platform_introspection()` method returning version, features, node/edge types, templates
  - 7 MCP endpoints: `/mcp/platform/version`, `/mcp/platform/features`, etc.
- **App Introspection Enhancement** (N=310) - Application-specific configuration
  - `/mcp/tools` and `/mcp/state-schema` endpoints
  - `ConfiguredFeatureInfo` for feature configuration details
- **Live Execution Introspection** (N=311-312) - Runtime execution state
  - `ExecutionTracker` for managing active executions
  - 10 MCP endpoints: `/mcp/live/executions`, `/mcp/live/executions/:id`, etc.
  - Real-time event streaming via SSE (`/mcp/live/events`)
- **Unified Three-Level API** (N=313) - Combined introspection interface
  - `unified_introspection()` method returning all three levels
  - JSON serialization for AI consumption
- **Documentation & Example** (N=314) - `three_level_introspection.rs` example
- **133 new tests** (5456 → 5589)

### Added - MCP Self-Documentation Protocol (N=297-308)

Every DashFlow app becomes a self-documenting MCP server:

- **McpSelfDocServer** (N=297) - HTTP server with standardized endpoints
  - `/mcp/about`, `/mcp/capabilities`, `/mcp/architecture`, `/mcp/implementation`
  - Natural language query interface
- **Schema Version & Metadata Standards** (N=298) - Forward-compatible schemas
  - SCHEMA_VERSION constant, node_metadata_keys, graph_metadata_keys modules
- **CLI Help Integration** (N=299) - Automatic help flag handling
  - HelpLevel::from_args(), process_cli_help(), CompiledGraph::process_cli_help()
- **Enhanced Query Interface** (N=300) - Intelligent pattern matching
  - 11 query categories with synonym support and fuzzy matching
- **Node Drill-Down Endpoints** (N=302) - Detailed node inspection
  - `/mcp/nodes`, `/mcp/nodes/:name`, `/mcp/features`
- **Dependencies & Edges Endpoints** (N=303) - Graph connectivity inspection
  - `/mcp/dependencies`, `/mcp/edges`
- **57 MCP self-doc tests**

### Added - Default-Enabled Features (N=272-277)

All important features are now enabled by default with opt-out pattern ("batteries included"):

- **Default Graph Validation** (N=272) - `compile()` now validates automatically
  - `compile_without_validation()` for opt-out
- **Default Resource Limits** (N=273) - Max recursion 25, node timeout 5min, state size 100MB
  - `without_limits()` for opt-out
- **Default Introspection** (N=274) - `manifest()`, `platform()`, `introspect()` always available
  - `without_introspection()` for opt-out
- **Default Memory Checkpointer** (N=275) - MemoryCheckpointer enabled by default
  - `without_checkpointing()` for opt-out
- **Default Retry Policy** (N=276) - 3 retries with exponential backoff
  - `without_retries()` for opt-out
- **Default Metrics/Performance** (N=277) - Auto-collected metrics, `performance()` API
  - `without_metrics()` for opt-out

### Added - AI Self-Awareness (N=247-270)

Complete AI introspection and self-awareness infrastructure:

- **Graph Manifest Generation** - Auto-generated on compile
- **Platform Registry** - Query DashFlow capabilities
- **Execution Introspection** - Live state querying, tracing, decision explanation
- **Performance Monitoring** - Bottleneck detection, optimization suggestions
- **Graph Versioning** - Version detection, comparison, state history
- **GraphRegistry & ExecutionRegistry** - Track graph catalog and execution history
- **AISelfKnowledge unified API** - Combines all introspection features

### Changed

- **Test count**: 5,589 dashflow lib tests (2 ignored) + 320 streaming tests (20 ignored)
- **Clippy warnings**: 0
- **New introspection tests**: 800+ (introspection: 377, platform: 229, registry: 135, mcp: 57)

---

## [1.11.2] - 2025-12-06

### Added - Design Feedback Implementation (N=217-226)

All 17 design feedback items from codex_dashflow port are now complete:

- **SandboxedShellTool** (N=226) - OS-level sandbox enforcement for shell commands
  - `SandboxMode`: Strict, Permissive, Disabled modes
  - `SandboxCapabilities`: Platform detection (Seatbelt/macOS, Landlock/Linux)
  - Builder pattern with configurable writable/readable roots

- **McpToolRegistry** (N=226) - MCP (Model Context Protocol) integration utilities
  - `McpResponse`: Structured response types preserving MIME types (Text, Resource, Image)
  - Dynamic tool registration with callback support
  - Mock response support for testing

- **ResumeRunner** (N=224) - Resume-aware workflow execution with environment validation
  - Working directory and sandbox mode validation
  - Environment variable validation hooks
  - Custom validator trait for extensibility

- **CheckpointError enum** (N=223) - Typed checkpoint failure handling

- **ToolOutputShaper middleware** (N=222) - Tool output truncation and redaction
  - Configurable max output bytes with custom truncation messages
  - Pattern-based redaction for sensitive data

- **FnTracer** (N=221) - Closure-based execution tracing
  - `with_tracer()` method for CompiledGraph

- **CompiledGraph::validate()** (N=219) - Graph structure validation

- **dashflow-testing crate** (N=218) - MockTool and testing utilities

- **Telemetry event batching** (N=217) - Reduced scheduler overhead at high volume
  - Configurable `telemetry_batch_size` and `telemetry_batch_timeout_ms`
  - EventBatch message support with background batch worker

### Added - Coding Agent Support (N=193-199)

Complete infrastructure for building production coding agents:

- **dashflow-context** - Context window management with token counting (tiktoken-rs)
- **dashflow-git-tool** - Git integration with libgit2
- **dashflow-project** - Project context discovery (14 languages, 23 frameworks)
- **dashflow-prompts** - Prompt registry with version management and A/B testing
- **Shell safety analysis** - Command risk analysis with dangerous pattern detection
- **ApprovalNode** - Built-in approval flow with risk levels and timeouts
- **SQLite checkpointer** - Lightweight persistence with WAL mode
- **StateGraph debugging** - Mermaid export and execution tracing
- **Alternative streaming backends** - InMemory, File, SQLite (no Kafka required)

### Changed

- **Test count**: 4,612 lib tests + 9 testing crate tests (4,621 total)
- **Clippy warnings**: 0
- **Doc warnings**: 0
- **Design feedback items**: 17/17 complete (100%)
- **Coding agent gaps**: 10/10 resolved

---

## [1.11.1] - 2025-12-05

### Added

- **Native SVG flamegraph generation** - `dashflow flamegraph --format svg` now generates SVG directly using `inferno` crate without external tools

### Fixed (N=99-162)

- **109 bugs fixed** (84 from code audit N=99-139 + 25 from Codex audit N=152-162)
- HTTP doctest panic - added `no_run` attribute
- Tracing headers dropped in loop - changed `.headers()` to `.header()` append
- State diff data loss (`unwrap_or_default`) - proper error handling with fallback
- Non-atomic checkpoint index writes - temp file + atomic rename + fsync
- Corrupt checkpoint breaks listing - skip corrupt files with warning
- Blocking mutex in async hot path - replaced with `AtomicU64`
- Executor timeout handling - added proper timeouts
- Connection pool limits - configured connection pools
- Missing error context - added context to 78+ error paths
- Rate limiting gaps - implemented rate limiters
- DLQ retry mechanism - proper retry with exponential backoff
- Producer per-call timeout - configurable per-request timeouts
- TLS/SASL configuration - full security config support
- Flow control improvements - backpressure handling
- Schema validation enhancements - JSON schema validation
- Compression configuration - multiple algorithm support
- Structured logging - converted eprintln! to tracing

### Changed

- **Test count**: 6,641+ lib tests, 397+ doc tests (7,000+ total)
- **Test definitions**: 9,360+ (up from 5,626)
- **Optimizer count**: 14 algorithms (up from 11)

---

## [1.11.0] - 2025-12-03

### Migration

- Completed migration from dashflow_rs to dashflow
- Consolidated architecture (merged dashflow-core + dashflow-dashflow into single dashflow crate)
- All features preserved with improved naming
- Namespace updated: `dashflow::*` → `dashflow::*`

### Added - Unified CLI (N=42)

- **dashflow-cli** - Unified CLI combining streaming telemetry + prompt optimization
  - `dashflow optimize` - Run prompt optimization with 11 algorithms (Bootstrap, SIMBA, GEPA, MIPROv2, COPRO, COPROv2, GRPO, KNN, etc.)
  - `dashflow eval` - Evaluate graph performance with multiple metrics (exact_match, F1, precision, LLM-as-judge)
  - `dashflow train distill` - Knowledge distillation from teacher to student model
  - `dashflow train finetune` - OpenAI fine-tuning API integration
  - `dashflow train synthetic` - Synthetic training data generation
  - `dashflow train rl` - GRPO reinforcement learning training
  - `dashflow dataset validate/stats/convert/split/sample/inspect` - Dataset utilities
- Binary renamed from `dashstream` to `dashflow`
- 8 existing streaming commands preserved (tail, inspect, replay, diff, export, flamegraph, costs, profile)

### Added - Multimodal Types (N=41)

- **dashopt_types module** - 9 specialized types for multimodal LLM workflows
  - Image, Audio, File, Citation, Document, Code, History, Reasoning, ToolCall/ToolCalls

### Added - Phase 2B (RL Infrastructure)

- **GRPO optimizer** - Group Relative Policy Optimization for chain-of-thought reasoning
- **BootstrapFinetune optimizer** - Generate fine-tuning datasets from execution traces
- **BetterTogether meta-optimizer** - Composition of multiple optimization strategies
- **TraceCollector** with Kafka/DashStream integration
- RL API extensions for ChatModel

### Changed

- Crate names: `dashflow-*` → `dashflow-*`
- Main crate: `dashflow-dashflow` → `dashflow`
- Streaming: `DashStream` → `DashStream`
- CLI: `dashstream-cli` → `dashflow-cli`, binary `dashstream` → `dashflow`

### Removed

- External DashOptimize wrapper dependency (native DashOptimize provides all functionality)
- Separate `dashflow-core` crate (merged into `dashflow`)

### Fixed

- Test import conflicts from namespace consolidation
- Type alias conflicts in test modules

---

## [1.10.0] - 2025-11-25

### Added

- Phase 2A optimization infrastructure (SIMBA, GEPA, RandomSearch)
- A/B testing framework with traffic splitting
- Cost monitoring and budget management
- Distillation pipeline for model compression

### Changed

- Expanded test coverage to 1,645+ tests
- Improved parallel execution performance

---

## [1.9.0] - 2025-11-15

### Added

- DashFlow Streaming binary protocol
- CLI inspector (`dashstream`) with 8 debugging commands
- Kafka integration for distributed telemetry
- Protocol Buffers encoding with Zstd compression

---

## [1.8.0] - 2025-11-01

### Added

- DashOptimize module with 6 base optimizers
- BootstrapFewShot algorithm
- KNNFewShot algorithm
- Ensemble optimization methods

---

## [1.7.0] - 2025-10-15

### Added

- Production observability stack (Prometheus, Grafana, Jaeger)
- Dead letter queue for failed messages
- Alert system for sequence anomalies
- Rate limiting with per-tenant quotas

---

## [1.6.0] - 2025-10-01

### Added

- Checkpointing with 4 backends (PostgreSQL, Redis, S3, DynamoDB)
- Human-in-the-loop workflow support
- Stateful workflow resumption

### Changed

- Checkpointing performance: 526× faster than Python

---

## [1.5.0] - 2025-09-15

### Added

- Parallel execution with `add_parallel_edges`
- MergeableState trait for result aggregation
- Subgraph composition

---

## [1.0.0] - 2025-08-01

### Added

- Initial release of DashFlow (then dashflow_rs)
- StateGraph-based agent orchestration
- Node and edge primitives
- Conditional routing
- Basic execution engine

---

## Migration Guide

For migration guidance, see [docs/GOLDEN_PATH.md](docs/GOLDEN_PATH.md) for recommended API patterns.
