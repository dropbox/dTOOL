# Codex DashFlow Upgrade Plan (Upstream OpenAI Codex CLI)

**Last Updated:** 2026-01-03 (Worker #2374 - Apply patch workflow + system prompts)

This plan upgrades `examples/apps/codex-dashflow` toward feature parity with upstream OpenAI Codex CLI, while preserving DashFlow’s observability (WAL + GraphEvents + telemetry sinks) and platform guarantees.

**Manager directive:** `WORKER_DIRECTIVE.md` (P0).

---

## Upstream Target (Reproducible Pointer)

Upstream release artifact:

- npm: `@openai/codex` version `0.77.0` (`npm view @openai/codex version`)
- Commit SHA: `ee9d441777b81716a78cdd371bd78a2cccaa7855` (2026-01-03)
- Repository: `https://github.com/openai/codex`

Notes:

- The upstream `openai/codex` monorepo uses development versions in-tree (`0.0.0-dev` / `0.0.0`) and does not necessarily tag npm releases. For upgrade work, record the upstream commit SHA used for analysis alongside the target npm version.
- The pinned commit SHA above represents the baseline for feature comparison. Update this SHA when performing subsequent upgrade analyses.

---

## Upstream Architecture Snapshot (openai/codex)

Top-level layout (high-level):

- `codex-cli/` (TypeScript CLI package, npm)
- `codex-rs/` (Rust core + CLI + TUI + sandboxing + MCP)
- `shell-tool-mcp/` (MCP server pattern for shell tools)
- `sdk/` (language SDKs)

Relevant upstream CLI capabilities observed in `codex-rs/cli`:

- Interactive TUI (default mode)
- Non-interactive execution (`exec`)
- Login / stored credentials
- Resume sessions
- Sandbox commands (Seatbelt/Landlock/Windows)
- MCP commands + MCP stdio server
- Apply latest agent diff to git working tree

---

## DashFlow Mapping (Where Features Should Live)

DashFlow already has platform pieces that should be reused:

- CLI framework: `crates/dashflow-cli` (the `dashflow` binary)
- Tool integrations: `crates/dashflow-shell-tool`, `crates/dashflow-file-tool`, `crates/dashflow-github`, etc.
- Observability: `crates/dashflow-observability` + GraphEvents → WAL persistence
- App examples: `examples/apps/codex-dashflow` (paragon app surface)

Principle:

- Put shared primitives in crates (DashFlow core/tooling).
- Keep “product surface” commands and UX in `examples/apps/codex-dashflow`.

---

## Feature Comparison Matrix (Initial)

| Feature | Upstream Codex | Codex DashFlow (today) | Action |
|--------:|:--------------:|:----------------------:|:------|
| Interactive chat UX (TUI) | ✓ | Basic stdin loop | Upgrade UX incrementally |
| Non-interactive exec | ✓ | ✓ | `exec` subcommand runs a single prompt and exits |
| Session persistence + resume | ✓ | ✓ | Persist AgentState to JSON and resume via `--session`/`--resume` |
| Patch/apply to git | ✓ | Partial (`refactor --apply`) | Add unified “apply” that writes patch + `git apply` |
| MCP server | ✓ (experimental) | ✓ | `mcp-server` stdio server exposes tools via JSON-RPC |
| Sandboxing | ✓ | ✗ | Defer (platform-dependent); document requirements |
| Telemetry/tracing | ✓ | Partial | Ensure OTel + WAL events are emitted |

---

## Detailed Missing Features Analysis (CODEX-003)

**Completed:** 2026-01-03 (Worker #2370)

### Critical Gaps (P1)

| Feature | Upstream | Codex DashFlow | Gap Details |
|---------|----------|----------------|-------------|
| Session persistence | `codex-rs/session` | ✓ | Persisted in Codex DashFlow via JSON session files (`--session`, `--resume`) |
| MCP server | `shell-tool-mcp/`, `codex-rs/mcp-server` | ✓ | Implemented as `codex-dashflow mcp-server` |
| Interactive TUI | Full `ratatui` TUI | Basic `stdin` loop | Upstream has full terminal UI with panels, scrolling, syntax highlighting |
| Non-interactive exec | `exec` subcommand | ✓ | Implemented as `codex-dashflow exec` |
| Git patch workflow | `apply` command | `refactor --apply` only | Upstream generates unified diffs, applies via `git apply`, supports `--dry-run` |

### Lower Priority Gaps (P2)

| Feature | Upstream | Codex DashFlow | Gap Details |
|---------|----------|----------------|-------------|
| Sandboxing | Seatbelt (macOS), Landlock (Linux), Windows | ✗ | Upstream restricts file/network access during agent execution |
| Credential management | Login + stored tokens | Uses env vars only | Upstream has `login` command, stores API keys securely |
| Streaming display | Token-by-token in TUI | ✓ (best-effort) | `chat`/`exec` support `--stream` (uses `ChatModel::stream()` when available); still no full TUI panels |
| Vision/image support | Image tool | ✗ | Upstream can read and analyze images |

### Tool Comparison

| Tool | Upstream Name | Codex DashFlow | Notes |
|------|---------------|----------------|-------|
| Read file | `read_file` | `read_file` | ✓ Equivalent |
| Write file | `write_file` | `write_file` | ✓ Equivalent |
| Edit file | `edit_file` / `apply_diff` | `edit_file` | Upstream has richer diff-based editing |
| List files | `list_directory` | `list_files` | ✓ Equivalent (includes recursive) |
| Shell exec | `execute_command` | `shell_exec` | ✓ Equivalent (with timeout) |
| Git operations | Dedicated git tools | Via `shell_exec` | Upstream has specialized git tool |
| Web search | `web_search` | ✗ | Upstream can search web |

---

## DashFlow-Specific Features to Preserve (CODEX-006)

**Completed:** 2026-01-03 (Worker #2370)

These features are DashFlow platform advantages that MUST be preserved during any upgrade:

### 1. GraphEvents Observability

```rust
// Codex DashFlow wires CollectingCallback for full event capture
let callback = CollectingCallback::new();
let agent_with_telemetry = agent.with_callback(callback);

// Events captured: GraphStart, NodeStart, NodeEnd, EdgeTraversal, StateChanged, GraphEnd
```

**Why preserve:** No upstream equivalent. DashFlow provides complete graph execution visibility.

### 2. OpenTelemetry Tracing Integration

```rust
// Built-in OTel support via dashflow-observability
use dashflow_observability::{init_tracing, TracingConfig};
let config = TracingConfig::new()
    .with_service_name("codex-dashflow")
    .with_otlp_endpoint(endpoint);
init_tracing(config).await?;
```

**Why preserve:** Production-ready distributed tracing. Upstream has limited telemetry.

### 3. Telemetry Disable Flag

```rust
// Clean opt-out via CLI flag
#[arg(long, global = true)]
no_telemetry: bool,

// Sets DASHFLOW_TELEMETRY_DISABLED=1
```

**Why preserve:** User privacy control without code changes.

### 4. Platform Tool Abstractions

DashFlow has reusable tool crates:
- `dashflow-shell-tool` - Shell execution with sandboxing hooks
- `dashflow-file-tool` - File operations with validation
- `dashflow-github` - GitHub API integration

**Why preserve:** Codex DashFlow tools should delegate to platform crates, not duplicate logic.

### 5. Prebuilt Agent Pattern

```rust
use dashflow::prebuilt::create_react_agent;

// Single function creates full ReAct agent with tool integration
let agent = create_react_agent(model, tools)?;
```

**Why preserve:** Consistent agent architecture across all DashFlow apps.

### 6. WAL Event Persistence

```rust
// Graph events automatically persist to WAL when enabled
// DASHFLOW_WAL_ENABLED=true
```

**Why preserve:** Enables post-hoc analysis, debugging, and audit trails.

---

## Prioritized Work Items

### P0 (Prove observability in a live run) - COMPLETE ✅

1. ✅ Ensure Codex DashFlow can run with tracing/telemetry enabled and disabled deterministically.
2. ✅ Add an ignored E2E integration test that runs a real solve prompt and asserts telemetry + GraphEvents are captured (requires external API key).
3. ✅ Produce a report artifact under `reports/` for the first successful run (screenshots/trace IDs/paths).

**Evidence:** `reports/codex_leetcode_observability_2026-01-03-16-45.md` demonstrates:
- 6 GraphEvents captured (GraphStart, NodeStart, NodeEnd, StateChanged, GraphEnd)
- Agent solved LeetCode Two Sum in 6.5s
- Text snapshots saved to ~/Desktop

### P1 (Approach upstream parity) - COMPLETE ✅

1. ✅ **DONE** Implement session persistence + `resume` in Codex DashFlow.
2. ✅ **DONE** Implement an MCP server entrypoint compatible with DashFlow tool registry.
3. ✅ **DONE** Add an "apply patch" workflow that produces deterministic diffs and applies them via git.
4. ✅ **DONE** Add dedicated `exec` command for non-interactive single-prompt execution.
5. ✅ **DONE** Update system prompts for tool discipline and upstream parity.

### Session Persistence (CODEX-009)

Implemented JSON session storage for the agentic `chat` loop and `exec` runs:

```bash
# Persist or resume (resumes automatically if the file exists)
codex-dashflow chat --session ./codex.session.json

# Require an existing session (defaults to ~/.codex-dashflow/sessions/default.json)
codex-dashflow chat --resume

# Exec with session persistence
codex-dashflow exec "refactor this crate" --session ./codex.session.json
```

---

## MCP Server Implementation (CODEX-007)

**Completed:** 2026-01-03 (Worker #2371)

### Overview

Implemented an MCP (Model Context Protocol) stdio server that exposes Codex DashFlow tools via JSON-RPC 2.0 over stdin/stdout. This allows LLM clients (Claude Code, OpenAI, etc.) to connect and use the tools.

### Usage

```bash
# Start the MCP stdio server
codex-dashflow mcp-server

# With custom working directory
codex-dashflow mcp-server --working-dir /path/to/project
```

### Protocol Support

The server implements JSON-RPC 2.0 with MCP extensions:

| Method | Description |
|--------|-------------|
| `initialize` | Handshake and capability negotiation |
| `tools/list` | List available tools |
| `tools/call` | Execute a tool |
| `ping` | Health check |
| `notifications/initialized` | Client ready notification |

### Tools Exposed

| Tool | Description | Schema |
|------|-------------|--------|
| `read_file` | Read file contents | `{ path: string }` |
| `write_file` | Write content to file | `{ path: string, content: string }` |
| `edit_file` | Edit file by replacing text | `{ path: string, old_text: string, new_text: string }` |
| `list_files` | List directory contents | `{ path?: string, recursive?: boolean }` |
| `shell_exec` | Execute shell commands | `{ command: string, timeout_secs?: number }` |

### Files Added

- `examples/apps/codex-dashflow/src/mcp_server.rs` - MCP stdio server implementation

### Test Coverage

12 unit tests covering:
- Server creation and tool registration
- Initialize handshake
- Tool listing
- Tool execution (read_file, write_file, list_files, shell_exec)
- Error handling (unknown tool, invalid JSON-RPC, method not found)
- Notification handling

### Integration with Claude Code

To use Codex DashFlow as an MCP server with Claude Code, add to your Claude Code configuration:

```json
{
  "mcpServers": {
    "codex-dashflow": {
      "command": "/path/to/codex-dashflow",
      "args": ["mcp-server", "--working-dir", "/path/to/project"]
    }
  }
}
```

---

## Exec Command Implementation (CODEX-008)

**Completed:** 2026-01-03 (Worker #2372)

### Overview

Implemented a non-interactive `exec` command that runs the coding agent with a single prompt, executes any necessary tool calls, and prints the result. Useful for scripting and CI/CD pipelines.

### Usage

```bash
# Basic execution
codex-dashflow exec "What is 2 + 2?"

# With working directory for file operations
codex-dashflow exec "List all Rust files in this directory" -d /path/to/project

# With context files (content is prepended to prompt)
codex-dashflow exec "Explain this code" --context src/lib.rs --context src/main.rs

# JSON output format
codex-dashflow exec "Generate a hello world function" --format json
```

### Options

| Option | Short | Description |
|--------|-------|-------------|
| `--working-dir` | `-d` | Working directory for file operations |
| `--context` | `-c` | Context files to include (can be repeated) |
| `--format` | `-f` | Output format: `text` (default) or `json` |

### JSON Output Format

When using `--format json`, output is:

```json
{
  "success": true,
  "result": "...",
  "duration_ms": 1234
}
```

### Implementation Details

- Reuses `run_single_query()` from the agent module
- Context files are read and prepended to the prompt with file path headers
- Respects `--no-telemetry` global flag
- Full tool execution support (read/write files, shell commands)

### Test Coverage

4 tests added to `tests/e2e.rs`:
- `test_cli_exec_help` - Verifies help output
- `test_e2e_cli_exec` - Basic execution (requires API key)
- `test_e2e_cli_exec_with_context_file` - Context file support (requires API key)
- `test_e2e_cli_exec_json_format` - JSON output format (requires API key)

---

## Apply Command Implementation (CODEX-010)

**Completed:** 2026-01-03 (Worker #2374)

### Overview

Implemented a git patch/apply workflow that applies unified diffs to the git working tree using `git apply`. Supports dry-run mode for previewing changes and can show current git diff status.

### Usage

```bash
# Apply a patch from a file
codex-dashflow apply --patch changes.patch

# Apply a patch from stdin
cat changes.patch | codex-dashflow apply

# Dry-run to check if patch applies cleanly
codex-dashflow apply --patch changes.patch --dry-run

# Show current unstaged changes (like git diff)
codex-dashflow apply --show-diff

# Show staged changes (like git diff --cached)
codex-dashflow apply --show-staged

# Specify working directory
codex-dashflow apply --patch changes.patch -d /path/to/repo
```

### Options

| Option | Short | Description |
|--------|-------|-------------|
| `--patch` | `-p` | Path to patch file (reads stdin if not provided) |
| `--working-dir` | `-d` | Git repository directory |
| `--dry-run` | | Check if patch applies without modifying files |
| `--show-diff` | | Show unstaged changes as diff |
| `--show-staged` | | Show staged changes as diff |

### Files Added

- `examples/apps/codex-dashflow/src/apply.rs` - Unified diff generation and git apply wrapper

### Key Functions

- `generate_unified_diff()` - Creates unified diff from original/modified strings using LCS algorithm
- `git_apply()` - Applies patches via `git apply` with dry-run support
- `git_diff()` - Returns unstaged changes
- `git_diff_staged()` - Returns staged changes

### Test Coverage

6 tests in `apply.rs`:
- `test_generate_unified_diff_simple` - Basic line replacement
- `test_generate_unified_diff_addition` - Line addition
- `test_generate_unified_diff_deletion` - Line deletion
- `test_lcs_empty` - Empty sequence handling
- `test_lcs_identical` - Identical sequences
- `test_lcs_partial_match` - Partial matching

1 test in `tests/e2e.rs`:
- `test_cli_apply_help` - Verifies CLI help output

---

## System Prompt Update (CODEX-011)

**Completed:** 2026-01-03 (Worker #2374)

### Overview

Enhanced the agent system prompt with structured tool usage guidelines and workflow best practices.

### Changes

The updated `CODING_ASSISTANT_PROMPT` now includes:

1. **Tool Usage Guidelines**: Detailed documentation for each tool (read_file, write_file, edit_file, list_files, shell_exec) with input examples and usage notes
2. **Workflow Best Practices**: Five-point guide (Understand First, Plan Before Acting, Make Minimal Changes, Verify Your Work, Explain Clearly)
3. **Safety Guidelines**: Clear rules about file operations and destructive commands
4. **Response Format**: Structured output expectations for task completion

### Location

`examples/apps/codex-dashflow/src/agent/mod.rs` - `CODING_ASSISTANT_PROMPT` constant
