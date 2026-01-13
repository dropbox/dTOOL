# ⚠️ ARCHIVED - Migrated to DashFlow Paragon Apps

| Director | Status |
|:--------:|:------:|
| TOOL | ACTIVE |

> **This repository has been archived.** Codex DashFlow is now part of the DashFlow paragon apps collection.

## New Location

```bash
git clone https://github.com/dropbox/dashflow.git
cd dashflow/examples/apps/codex-dashflow
```

## Why the Change?

- **Unified Platform** - All paragon apps share common infrastructure
- **Better Integration** - Direct access to DashFlow's optimization, introspection, and observability
- **Consistent Tooling** - Same CLI patterns, telemetry, and deployment across apps
- **Active Development** - DashFlow is actively maintained with continuous improvements

## See Also

- [DashFlow Paragon Apps Roadmap](https://github.com/dropbox/dashflow/blob/main/ROADMAP_CURRENT.md#part-36-paragon-apps-in-progress)
- [Librarian](https://github.com/dropbox/dashflow/tree/main/examples/apps/librarian) - RAG over classic books (complete)
- [Codebase RAG](https://github.com/dropbox/dashflow/tree/main/examples/apps/codebase-rag) - Codebase question answering (new)

---

*This repo is read-only. Please use the DashFlow version for new development.*

---

# (ARCHIVED) Codex DashFlow - Agentic Coding CLI

[![Tests](https://img.shields.io/badge/tests-4%2C477%20passing-brightgreen)]()
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange)]()
[![DashFlow](https://img.shields.io/badge/DashFlow-v1.11.3-blue)]()

A port of [OpenAI's Codex CLI](https://github.com/openai/codex) using the [DashFlow](https://github.com/dropbox/dashflow) graph-based agent orchestration framework.

## Project Status

| Metric | Value |
|--------|-------|
| **Tests** | 4,477 passing (13 ignored) |
| **Clippy** | 0 warnings |
| **DashFlow Version** | v1.11.3 (N=296) |
| **Iteration** | N=545 |

### Recent Features (Dec 2025)

- **AI Self-Awareness**: Agent understands its own architecture via GraphManifest injection
- **`codex architecture`**: View agent graph structure
- **`codex capabilities`**: Discover DashFlow platform features
- **`codex features`**: List compiled feature flags
- **`codex version --agent`**: Show agent graph version and registry status
- **PlatformRegistry Integration**: AI knows platform capabilities at runtime
- **GraphRegistry Integration**: Agent graphs are versioned and trackable

## Project Relationship

```
~/dashflow/        → PLATFORM (library we depend on)
~/codex/           → REFERENCE (OpenAI's implementation, read-only)
~/codex_dashflow/  → THIS APPLICATION (what we're building)
```

**This is an APPLICATION that uses DashFlow as a dependency**, similar to how a web app uses a framework. We don't copy DashFlow code here - we import it via Cargo:

```toml
[dependencies]
dashflow = { git = "https://github.com/dropbox/dashflow.git" }
```

### Platform vs Application Changes

- **Application changes** → make in `~/codex_dashflow/` (this repo)
- **Platform improvements** → make in `~/dashflow/` on a **feature branch**

If you discover bugs or needed features in DashFlow while building this app, fix them in `~/dashflow/` on a dedicated branch with platform-focused commits. See `CLAUDE.md` for the full workflow.

## Overview

This project reimplements the Codex CLI coding agent using DashFlow's StateGraph for agent workflow orchestration. The goal is to combine Codex's excellent coding agent UX with DashFlow's:

- **Graph-based orchestration**: Agent reasoning as composable graph nodes
- **Streaming telemetry**: Real-time visibility into agent decisions
- **Checkpointing**: Persistent sessions across restarts
- **Optimization**: DashOptimize for systematic prompt improvement

## Architecture

```
UserInput → Reasoning → ToolSelection → ToolExecution → ResultAnalysis → (loop or complete)
```

Each step is a DashFlow StateGraph node with edges defining the agent flow.

## Key Features

- **TUI Interface**: Full-screen terminal UI (Ratatui) for interactive coding
- **Exec Mode**: Non-interactive mode for automation
- **Authentication**: OAuth sign-in with ChatGPT account or API key
- **Sandbox**: Secure command execution (Seatbelt on macOS, Landlock+seccomp on Linux)
- **MCP Support**: Model Context Protocol server/client
- **Session Persistence**: Resume conversations via DashFlow checkpointing
- **Agent Visibility**: Debug agent decisions with `dashstream tail`
- **Prompt Optimization**: DashOptimize integration for systematic prompt improvement
- **AI Self-Awareness**: Agent understands its own architecture via DashFlow introspection

## AI Self-Awareness Features

Codex DashFlow uses DashFlow's introspection capabilities to give the AI agent awareness of its own architecture and platform capabilities.

### Agent Architecture Command

View the agent's graph structure:

```bash
# Full architecture details
codex-dashflow architecture

# Concise output
codex-dashflow architecture --brief
# Output:
# Agent Architecture (DashFlow StateGraph)
# ========================================
# Nodes: user_input → reasoning → tool_selection → tool_execution → result_analysis
# Entry: user_input
# Exit: result_analysis (conditional)
```

### Platform Capabilities Command

Discover what DashFlow features are available:

```bash
# Text output
codex-dashflow capabilities

# JSON format
codex-dashflow capabilities --format json
# Output:
# DashFlow Platform Capabilities
# ==============================
# - StateGraph: Graph-based workflow orchestration
# - Streaming: Real-time event callbacks
# - Checkpointing: Session persistence
# - Introspection: AI self-awareness
```

### Compiled Features Command

List which features are compiled into the binary:

```bash
# Text output
codex-dashflow features

# JSON format
codex-dashflow features --format json
# Output:
# Compiled Features
# =================
# dashstream: enabled
# postgres: disabled
# llm-judge: disabled
```

### Agent Version Command

Show agent graph version and registry status:

```bash
# Standard version
codex-dashflow version

# Agent graph info
codex-dashflow version --agent

# JSON format
codex-dashflow version --agent --format json
# Output:
# Codex DashFlow Agent
# ====================
# Graph: codex_dashflow_agent v0.1.0
# Registry Status: Registered
# Nodes: 5 (user_input, reasoning, tool_selection, tool_execution, result_analysis)
```

### Runtime Introspection

The agent can understand itself during execution:
- **GraphManifest** is automatically injected into the system prompt
- Agent knows its nodes, edges, and available tools
- Can answer questions like "What are your capabilities?"

### Authentication

Sign in with your ChatGPT account or use an API key:

```bash
# OAuth sign-in (opens browser)
codex-dashflow login

# Use API key
export OPENAI_API_KEY=sk-...

# Check auth status
codex-dashflow doctor

# Sign out
codex-dashflow logout
```

Credentials are stored securely:
- **macOS**: Keychain (with file fallback)
- **Linux**: File-based in `~/.codex-dashflow/auth.json`

### Sandbox Security

Commands executed by the agent run in a security sandbox to prevent unintended system modifications:

- **macOS**: Uses Apple's Seatbelt (sandbox-exec) for filesystem and network restrictions
- **Linux**: Uses Landlock LSM for filesystem restrictions and seccomp for network blocking

Three sandbox modes are available:
- `read-only` (default): Read filesystem, no writes, no network access
- `workspace-write`: Write within the working directory and /tmp, no network
- `danger-full-access`: No restrictions (only use in isolated/containerized environments)

The sandbox requires:
- **macOS**: Built-in sandbox-exec (available on all macOS versions)
- **Linux**: Kernel 5.13+ for Landlock support (run `uname -r` to check)

## Reference Projects (in ~/)

- **DashFlow Platform**: `~/dashflow/` - https://github.com/dropbox/dashflow (DO NOT EDIT - this is our dependency)
- **OpenAI Codex**: `~/codex/` - https://github.com/openai/codex (study for architecture patterns)

## Building

```bash
# Build basic binary (no telemetry/checkpointing)
cargo build --release

# Build with full features (recommended for production)
cargo build --release --features dashstream,postgres

# Build debug
cargo build
```

### Feature Flags

| Feature | Description | Requirements |
|---------|-------------|--------------|
| `dashstream` | Enable DashFlow Streaming telemetry to Kafka | Requires `protoc` compiler |
| `postgres` | Enable PostgreSQL session checkpointing | Requires PostgreSQL client libraries |

**Note:** The basic build works for most use cases. Enable `dashstream` for real-time agent visibility (`dashstream tail`) and `postgres` for persistent session checkpointing.

```bash
# Verify protoc is available (needed for dashstream feature)
protoc --version

# Run tests
cargo test --all
```

## Usage

### Interactive Mode (TUI)

Start the terminal UI for interactive coding sessions:

```bash
./target/release/codex-dashflow
```

Keys:
- Type your message and press Enter to submit
- Press `Esc` to exit

### Exec Mode (Non-Interactive)

Run a single prompt without the TUI:

```bash
# Simple query
./target/release/codex-dashflow --exec "List files in the current directory"

# With verbose output (shows tool calls)
./target/release/codex-dashflow --exec "Create a hello.txt file" --verbose

# Using mock LLM for testing (no API key required)
./target/release/codex-dashflow --exec "Test prompt" --mock
```

### CLI Options

```
codex-dashflow [OPTIONS] [COMMAND]

Commands:
  optimize      Optimize prompts using collected training data
  mcp-server    Run as an MCP server (exposes codex as a tool for other MCP clients)
  completions   Generate shell completions for the CLI
  version       Show detailed version information
  doctor        Check system setup and configuration
  init          Initialize configuration file with defaults
  login         Sign in with your ChatGPT account or API key
  logout        Sign out and clear stored credentials
  introspect    Display agent introspection data (graph structure, capabilities)
  architecture  Display agent graph structure (alias for introspect --brief)
  capabilities  Display DashFlow platform capabilities
  sessions      List saved sessions (checkpoints) that can be resumed

Options:
  -e, --exec <PROMPT>           Run in non-interactive mode with the given prompt
      --stdin                   Read prompt from stdin (for multiline prompts or piping)
      --prompt-file <PATH>      Read prompt from a file
  -d, --working-dir <DIR>       Working directory for file operations
  -t, --max-turns <N>           Maximum number of agent turns (0 = unlimited)
  -s, --session [<ID>]          Session ID to resume (use without argument for latest)
  -m, --model <MODEL>           LLM model to use
  -v, --verbose                 Output tool calls and results
  -q, --quiet                   Suppress all non-essential output
  -c, --config <PATH>           Path to config file
      --mock                    Use mock LLM for testing
      --dry-run                 Show resolved configuration and exit
      --check                   Validate configuration and exit with status
      --json                    Output in JSON format (for use with --check or --dry-run)

  DashFlow Streaming options:
      --dashstream                     Enable DashFlow Streaming telemetry
      --dashstream-bootstrap <SERVER>  Kafka bootstrap servers (e.g., "localhost:9092")
      --dashstream-topic <TOPIC>       Kafka topic for events (default: codex-events)

  Execution policy options:
      --approval-mode <MODE>           Tool approval mode (default: on-dangerous)
                                       Values: never, on-first-use, on-dangerous, always

  Sandbox options:
  -S, --sandbox <MODE>                 Sandbox mode for command execution (default: read-only)
                                       Values: read-only, workspace-write, danger-full-access

  Training data options:
      --collect-training               Collect training data from successful runs
      --load-optimized-prompts         Load optimized prompts from PromptRegistry

  System prompt options:
      --system-prompt <PROMPT>         Custom system prompt (overrides defaults)
      --system-prompt-file <PATH>      Path to file containing system prompt

  Session checkpointing options:
      --postgres <CONN_STRING>         PostgreSQL connection string for session checkpointing
      --checkpoint-path <PATH>         Path for file-based session checkpointing
      --auto-resume                    Enable auto-resume of most recent session on startup
      --no-auto-resume                 Disable auto-resume
      --auto-resume-max-age <SECS>     Maximum age in seconds for auto-resume sessions

  Introspection options:
      --introspection                  Enable AI introspection (graph manifest in system prompt)
      --no-introspection               Disable AI introspection

  -h, --help               Print help
  -V, --version            Print version
```

### Prompt Optimization

Codex DashFlow includes tools for collecting training data and optimizing prompts:

```bash
# Automatically collect training data during exec mode
./target/release/codex-dashflow --exec "List files" --collect-training

# Collect training data in TUI mode (shows score after each interaction)
./target/release/codex-dashflow --collect-training

# Show training data statistics
./target/release/codex-dashflow optimize stats

# Add a training example manually
./target/release/codex-dashflow optimize add \
  -i "List all Rust files" \
  -o "I'll search for .rs files..." \
  -s 0.9 \
  --tools shell

# Run prompt optimization
./target/release/codex-dashflow optimize run --few-shot-count 3

# Show current optimized prompts
./target/release/codex-dashflow optimize show
```

Training data is automatically scored based on:
- Completion status (complete vs error)
- Tool execution success rate
- Turn efficiency (fewer turns is better)

In TUI mode with `--collect-training`, the score is displayed after each interaction completes.

Training data is stored at `~/.codex-dashflow/training.toml` and optimized prompts at `~/.codex-dashflow/prompts.toml`.

### MCP Server Mode

Run Codex DashFlow as an MCP (Model Context Protocol) server to expose the coding agent as a tool for other MCP clients:

```bash
# Start as MCP server (uses stdio transport)
./target/release/codex-dashflow mcp-server

# With custom working directory
./target/release/codex-dashflow mcp-server --working-dir /path/to/project

# With specific sandbox mode
./target/release/codex-dashflow mcp-server --sandbox workspace-write

# For testing (uses mock LLM)
./target/release/codex-dashflow mcp-server --mock
```

The MCP server exposes a `codex` tool that other MCP clients can invoke:

```json
{
  "name": "codex",
  "arguments": {
    "prompt": "List all files in the current directory",
    "working_dir": "/path/to/project",  // optional
    "max_turns": 5,                      // optional (0 = unlimited)
    "sandbox_mode": "workspace-write"   // optional
  }
}
```

The tool returns:
- `response`: The agent's final response text
- `turns`: Number of agent turns executed
- `status`: Completion status (complete, turn_limit_reached, error, etc.)
- `tool_calls`: List of tools called during execution

### Agent Introspection

Codex DashFlow includes an AI introspection feature that allows the agent to understand its own workflow. When enabled, the agent's graph structure (nodes, edges, available tools) is included in the system prompt.

```bash
# View agent graph structure (JSON format)
./target/release/codex-dashflow introspect

# View as human-readable text
./target/release/codex-dashflow introspect --format text

# View as Mermaid diagram
./target/release/codex-dashflow introspect --format mermaid
```

Runtime control via CLI flags:
```bash
# Enable introspection (overrides config)
./target/release/codex-dashflow --introspection --exec "What tools can you use?"

# Disable introspection to reduce system prompt size
./target/release/codex-dashflow --no-introspection --exec "List files"
```

Configuration via `config.toml`:
```toml
[dashflow]
introspection_enabled = true  # default: true
```

### Configuration

Create `~/.codex-dashflow/config.toml` (see [examples/config.toml](examples/config.toml) for a full example):

```toml
# Default model
model = "gpt-4"

# Maximum turns per session (0 = unlimited)
max_turns = 10

# Working directory
working_dir = "."

# Enable training data collection by default (for both TUI and exec mode)
collect_training = true

# Sandbox mode for command execution (read-only, workspace-write, danger-full-access)
sandbox_mode = "read-only"

# DashFlow settings
[dashflow]
streaming_enabled = true
checkpointer = "memory"

# MCP servers (optional)
[[mcp_servers]]
name = "filesystem"
type = "stdio"
command = "mcp-server-filesystem"
args = ["/home/user/projects"]

# Execution policy (optional)
[policy]
approval_mode = "on_dangerous"
include_dangerous_patterns = true
```

### Session Persistence

Enable session checkpointing to save and resume conversations across restarts:

```toml
[dashflow]
# Enable checkpointing (required for session persistence)
checkpointing_enabled = true

# File-based checkpoints (default: memory-only)
checkpoint_path = "~/.codex-dashflow/checkpoints"

# Auto-resume the most recent session on TUI startup
auto_resume = true

# Skip sessions older than 24 hours (86400 seconds)
auto_resume_max_age_secs = 86400
```

**Session management CLI:**

```bash
# List saved sessions
codex-dashflow sessions

# Show session details
codex-dashflow sessions --show <ID>

# Resume a specific session
codex-dashflow --session <ID>

# Resume the most recent session
codex-dashflow --session

# Delete a session
codex-dashflow sessions --delete <ID>

# Delete all sessions
codex-dashflow sessions --delete-all --force
```

When `auto_resume` is enabled, the TUI shows an `[auto-resumed]` indicator in the status bar. If no sessions are available, a warning message is displayed.

## Crate Structure

```
crates/
├── apply-patch/       # Unified diff patch application (pure Rust)
├── cli/               # Command-line interface entry point
├── core/              # Agent state, graph nodes, LLM client, tools
├── exec/              # Non-interactive execution mode
├── file-search/       # File and code search (glob, regex, fuzzy)
├── mcp/               # Model Context Protocol client
├── mcp-server/        # MCP server mode (exposes codex as a tool)
├── process-hardening/ # Security hardening (disable core dumps, ptrace)
├── sandbox/           # Seatbelt/Landlock sandbox wrappers
└── tui/               # Ratatui terminal UI
```

## Development

This project uses a worker/manager AI collaboration model:
- **Workers**: Implement features following the phased roadmap in CLAUDE.md
- **Managers**: Provide guidance and course corrections via [MANAGER] commits
- Progress tracked via git commit iteration numbers

See `CLAUDE.md` for AI worker/manager collaboration guidelines and implementation priorities.

## Benchmarks

Performance benchmarks are available for measuring agent loop and node performance.

### Running Benchmarks

```bash
# Run all criterion benchmarks with statistical analysis
cargo bench -p codex-dashflow-core --bench nodes

# Run the simpler agent loop benchmark
cargo bench -p codex-dashflow-core --bench agent_loop

# Run CLI prompt loading benchmarks
cargo bench -p codex-dashflow-cli --bench prompt_loading
```

### Available Benchmark Groups

**State Benchmarks** (`nodes` harness):
- `state_creation` - Baseline state creation time
- `state_with_message` - State with user message
- `state_with_tool_calls` - State with pending tool calls

**Graph Benchmarks** (`nodes` harness):
- `graph_build` - DashFlow StateGraph construction

**Node Benchmarks** (`nodes` harness):
- `node_user_input` - User input node processing
- `node_reasoning_mock` - Reasoning node with mock LLM
- `node_tool_selection` - Tool selection without policy
- `node_tool_selection_with_policy` - Tool selection with ExecPolicy
- `node_tool_execution_mock` - Mock tool execution
- `node_result_analysis` - Result analysis node

**Agent Loop Benchmarks** (`nodes` harness):
- `agent_loop_single_turn` - Full agent loop, 1 turn
- `agent_loop_three_turns` - Full agent loop, 3 turns

**Checkpointing Benchmarks** (`nodes` harness):
- `agent_loop_memory_checkpointing` - With memory checkpointer
- `agent_loop_file_checkpointing` - With file checkpointer
- `checkpointing_overhead` - Comparative overhead analysis

**Optimize Module Benchmarks** (`nodes` harness):
- `prompt_registry_defaults` - Default prompt registry creation
- `prompt_registry_get_prompt` - Prompt retrieval
- `prompt_registry_from_toml` - TOML parsing

### Interpreting Results

Criterion provides statistical analysis including:
- Mean execution time with confidence intervals
- Standard deviation
- Outlier detection
- Comparison with previous runs

Results are saved to `target/criterion/` for historical comparison.
