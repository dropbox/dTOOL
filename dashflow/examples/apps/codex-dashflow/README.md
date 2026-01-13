# Codex DashFlow (Paragon App)

AI-powered code generation and understanding, implemented as a DashFlow paragon app.

Legacy standalone repo archival note: `docs/CODEX_DASHFLOW_ARCHIVE_NOTICE.md`.

## Run

```bash
cargo run -p codex-dashflow -- --help
```

If you want to use OpenAI-backed models, load your keys first:

```bash
cp .env.template .env
source .env
```

## Commands

### Code Generation & Understanding

```bash
# Generate code from natural language
cargo run -p codex-dashflow -- generate "write a rust function that parses a CSV line"

# Explain code in plain English
cargo run -p codex-dashflow -- explain --file examples/apps/codex-dashflow/src/lib.rs

# Suggest refactoring improvements
cargo run -p codex-dashflow -- refactor --file examples/apps/codex-dashflow/src/lib.rs

# Generate unit tests
cargo run -p codex-dashflow -- test --file src/lib.rs

# Generate documentation
cargo run -p codex-dashflow -- docs --file src/lib.rs
```

### Interactive Chat (Agentic Mode)

```bash
# Start an interactive chat session with tool access
cargo run -p codex-dashflow -- chat

# With streaming output
cargo run -p codex-dashflow -- chat --stream

# With session persistence
cargo run -p codex-dashflow -- chat --session ./codex.session.json

# Resume a previous session
cargo run -p codex-dashflow -- chat --resume
```

The chat mode provides an agent with tools for reading/writing files and executing shell commands.

### Non-Interactive Execution

```bash
# Execute a single prompt and exit
cargo run -p codex-dashflow -- exec "List all Rust files in this directory"

# With working directory
cargo run -p codex-dashflow -- exec "Explain the main function" -d /path/to/project

# With context files
cargo run -p codex-dashflow -- exec "Review this code" --context src/main.rs

# JSON output format
cargo run -p codex-dashflow -- exec "Generate a function" --format json
```

### MCP Server

Run as a Model Context Protocol stdio server for integration with LLM clients:

```bash
cargo run -p codex-dashflow -- mcp-server --working-dir /path/to/project
```

Exposes tools: `read_file`, `write_file`, `edit_file`, `list_files`, `shell_exec`.

### Git Patch Workflow

```bash
# Apply a patch file
cargo run -p codex-dashflow -- apply --patch changes.patch

# Dry-run to verify patch applies
cargo run -p codex-dashflow -- apply --patch changes.patch --dry-run

# Show current unstaged changes
cargo run -p codex-dashflow -- apply --show-diff

# Show staged changes
cargo run -p codex-dashflow -- apply --show-staged
```

## Global Options

- `--no-telemetry` - Disable telemetry collection

## Tests

```bash
# Run unit and integration tests
cargo test -p codex-dashflow

# Run E2E tests (requires OPENAI_API_KEY)
source .env && export OPENAI_API_KEY
cargo test -p codex-dashflow --test e2e -- --ignored
```

## Features

- **DashFlow GraphEvents**: Full observability via `CollectingCallback`
- **OpenTelemetry**: Production-ready distributed tracing
- **Session Persistence**: Resume conversations across sessions
- **MCP Protocol**: Compatible with Claude Code and other MCP clients
- **Streaming**: Token-by-token output in chat/exec modes
