# DashFlow CLI Reference

**Version:** 1.11
**Last Updated:** 2026-01-04 (Worker #2471 - Added missing `dashflow evals` command documentation)

The DashFlow CLI provides unified tooling for AI workflow orchestration, streaming telemetry, and prompt optimization.

---

## Installation

The CLI is built from the `dashflow-cli` crate:

```bash
cargo install --path crates/dashflow-cli
# or
cargo build --release -p dashflow-cli
```

---

## Command Categories

### Timeline (Recommended)

Unified interface for graph execution observation (M-38).

| Command | Description |
|---------|-------------|
| `dashflow timeline live` | Watch live graph execution (TUI) |
| `dashflow timeline replay` | Replay historical execution (time-travel debugging) |
| `dashflow timeline view` | View a static graph visualization in a browser |
| `dashflow timeline export` | Export a graph visualization to standalone HTML |

### Streaming Telemetry

Commands for real-time visibility into agent execution.

| Command | Description |
|---------|-------------|
| `dashflow tail` | Stream live events from Kafka |
| `dashflow inspect` | Show thread details and execution history |
| `dashflow diff` | Compare two checkpoints |
| `dashflow export` | Export thread data to JSON |
| `dashflow flamegraph` | Generate flamegraph for performance visualization |
| `dashflow costs` | Analyze token costs across executions |
| `dashflow profile` | Profile execution performance |
| `dashflow analyze` | Analyze exported JSON files offline (no Kafka required) |
| `dashflow watch` | **(DEPRECATED)** Live graph visualization TUI - use `timeline live` instead |
| `dashflow replay` | **(DEPRECATED)** Replay execution from checkpoint - use `timeline replay` instead |

### Prompt Optimization

Commands for systematic prompt improvement (from DashOptimize).

| Command | Description |
|---------|-------------|
| `dashflow optimize` | Run prompt optimization on a graph |
| `dashflow eval` | Evaluate graph performance on a test dataset |
| `dashflow train` | Train or fine-tune models (distillation, RL) |
| `dashflow dataset` | Dataset utilities (generate, validate, inspect) |
| `dashflow baseline` | Manage evaluation baselines (save, list, check, delete) |
| `dashflow evals` | Manage evaluation test cases and golden datasets (list, show, promote) |

### Developer Tools

| Command | Description |
|---------|-------------|
| `dashflow visualize serve` | **(DEPRECATED)** Start graph visualization server - use `timeline view/export` instead |
| `dashflow debug` | Interactive debugger for step-through graph execution |

### Pattern Detection

| Command | Description |
|---------|-------------|
| `dashflow patterns` | Detect patterns in execution traces |

### Code Quality

| Command | Description |
|---------|-------------|
| `dashflow lint` | Lint for platform feature reimplementations |

### Parallel AI Development

| Command | Description |
|---------|-------------|
| `dashflow locks` | Manage parallel AI development locks |

### Infrastructure & Health

| Command | Description |
|---------|-------------|
| `dashflow status` | Quick infrastructure check (Docker, containers, ports) |
| `dashflow introspect health` | Comprehensive platform verification (Graph Engine, Checkpointing, Modules, LLM, + infrastructure) |
| `dashflow executions` | Query persisted executions from EventStore (list, show, events) |

**When to use which:**
- **`status`**: Quick DevOps check - "Are my containers running?" Provides auto-recovery hints.
- **`introspect health`**: Platform verification - "Is DashFlow working correctly?" Includes core functionality tests (Graph Engine, File Checkpointing, Module Discovery) plus optional infrastructure checks.

### Introspection

| Command | Description |
|---------|-------------|
| `dashflow introspect` | Query DashFlow module information |
| `dashflow introspect health` | Platform health verification (see above) |
| `dashflow mcp-server` | MCP server for AI introspection (HTTP API) |

### Self-Improvement

| Command | Description |
|---------|-------------|
| `dashflow self-improve` | Self-improvement commands for AI agents |

### Project Scaffolding

| Command | Description |
|---------|-------------|
| `dashflow new` | Create a new DashFlow application with production defaults |

### Package Registry

| Command | Description |
|---------|-------------|
| `dashflow pkg` | Package registry operations (search, install, publish) |

---

## Common Usage Examples

### Stream Live Events

```bash
# Stream events from default Kafka broker
dashflow tail

# Stream from specific topic
dashflow tail --topic my-agent-events
```

### Introspection

```bash
# Search modules by keyword
dashflow introspect search distillation

# Show module details
dashflow introspect show optimize::distillation

# List all CLI commands
dashflow introspect cli

# Health check
dashflow introspect health --skip-infra

# JSON output for automation (available on list, search, show, cli, health)
dashflow introspect list --format json
dashflow introspect search retriever --format json
dashflow introspect health --format json
```

### Self-Improvement

```bash
# Analyze execution and generate improvement plans
dashflow self-improve analyze
dashflow self-improve analyze --format json    # JSON output for automation

# List pending improvement plans
dashflow self-improve plans
dashflow self-improve plans --format json      # JSON output for automation

# Approve a plan for implementation
dashflow self-improve approve <plan-id>

# Start daemon with JSON output
dashflow self-improve daemon --once --format json

# Generate tests in JSON format
dashflow self-improve generate-tests --format json
```

### Executions

```bash
# List recent executions
dashflow executions list
dashflow executions list --format json         # JSON output for automation

# Show execution details
dashflow executions show exec-abc123
dashflow executions show exec-abc123 --format json

# Show events for an execution
dashflow executions events exec-abc123
dashflow executions events exec-abc123 --format json
```

### Lock Management

```bash
# List all locks
dashflow locks list

# Acquire a lock
dashflow locks acquire dashflow-openai --worker "my-worker" --purpose "Feature X"

# Release a lock
dashflow locks release dashflow-openai --worker "my-worker"
```

### Prompt Optimization

```bash
# Run optimization on a graph
dashflow optimize --graph my_graph --dataset training.json

# Evaluate performance
dashflow eval --graph my_graph --dataset test.json
```

### Dataset

```bash
# Show dataset statistics
dashflow dataset stats -i data.jsonl
dashflow dataset stats -i data.jsonl --output-format json  # JSON output for automation

# Validate dataset format
dashflow dataset validate -i data.jsonl --format jsonl

# Inspect examples
dashflow dataset inspect -i data.jsonl --head 5
```

### Evals (Test Case Management)

```bash
# List pending tests awaiting review
dashflow evals list
dashflow evals list --format json          # JSON output for automation
dashflow evals list --needs-review         # Show only tests needing review

# Show details of a specific pending test
dashflow evals show test-abc123
dashflow evals show test-abc123 --format json

# Promote a pending test to a golden scenario
dashflow evals promote test-abc123 --golden-dir .dashflow/golden
dashflow evals promote test-abc123 --golden-dir .dashflow/golden --dry-run  # Preview first
dashflow evals promote test-abc123 --golden-dir .dashflow/golden \
  --description "Tests correct response to greeting" \
  --quality-threshold 0.9 \
  --difficulty simple
```

### Code Quality

```bash
# Lint current directory for platform reimplementations
dashflow lint

# Lint specific path with detailed explanations
dashflow lint src/ --explain

# Lint with JSON output
dashflow lint --format json

# Manage lint feedback
dashflow lint feedback list
dashflow lint feedback summary
```

---

## Getting Help

Each command supports `--help` for detailed usage:

```bash
dashflow --help
dashflow tail --help
dashflow optimize --help
```

---

## See Also

- [CLAUDE.md](../CLAUDE.md) - AI worker instructions with CLI examples
- [DESIGN_INVARIANTS.md](../DESIGN_INVARIANTS.md) - Architectural guidelines
- [ROADMAP_CURRENT.md](../ROADMAP_CURRENT.md) - Current development roadmap
