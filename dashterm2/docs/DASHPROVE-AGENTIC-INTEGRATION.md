# DashProve Agentic Coding Integration

**Date:** December 2025
**Purpose:** Continuous feedback systems for AI coding agents
**Status:** Design complete, ready for implementation

---

## Executive Summary

AI coding agents (like Claude workers in DashTerm2) can dramatically improve their efficiency by using continuous background feedback tools instead of manually invoking build commands. This document describes the integration architecture.

---

## The Problem

Traditional AI coding workflow:
```
1. Agent writes code
2. Agent runs: cargo build
3. Wait 5-30 seconds for result
4. Agent reads error output
5. Agent fixes error
6. Repeat
```

This is slow because:
- Build commands have startup overhead
- Agent must remember to run checks
- No feedback during editing
- Full rebuilds even for small changes

---

## The Solution: Continuous Feedback Loop

```
┌─────────────────────────────────────────────────────────────┐
│                     AI AGENT                                 │
│  ┌─────────────────────────────────────────────────────┐    │
│  │ 1. Write code to file                                │    │
│  │ 2. Check feedback files (non-blocking)               │    │
│  │ 3. If errors → fix immediately                       │    │
│  │ 4. If clean → continue to next task                  │    │
│  └─────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────┘
          ↑                              ↑
          │ read                         │ read
          │                              │
┌─────────┴──────────┐      ┌───────────┴────────────┐
│ /tmp/dashterm-     │      │ /tmp/dashterm-         │
│ feedback/          │      │ feedback/              │
│ rust_errors.txt    │      │ status.json            │
│ swift_errors.txt   │      │ last_update            │
└─────────┬──────────┘      └───────────┬────────────┘
          │ write                       │ write
          │                              │
┌─────────┴──────────────────────────────┴────────────┐
│              BACKGROUND WATCHERS                     │
│  ┌──────────┐  ┌──────────┐  ┌──────────────────┐  │
│  │  bacon   │  │  cargo   │  │  fswatch +       │  │
│  │  (rust)  │  │  watch   │  │  swiftlint       │  │
│  └──────────┘  └──────────┘  └──────────────────┘  │
└──────────────────────────────────────────────────────┘
          ↑                              ↑
          │ watch                        │ watch
          │                              │
┌─────────┴──────────────────────────────┴────────────┐
│                   SOURCE FILES                       │
│  rust-core/*.rs    sources/*.swift    sources/*.m   │
└──────────────────────────────────────────────────────┘
```

---

## Key Components

### 1. bacon (Rust Background Checker)

**What:** Watches Rust files, runs `cargo check` on changes, reports errors instantly.

**Why it's perfect for agents:**
- Runs continuously in background
- Instant feedback (< 1 second for incremental)
- Structured output for parsing
- Minimal resource usage

**Integration:**
```bash
# Start bacon in background
bacon --export-locations > /tmp/bacon-output.json &

# Agent checks for errors (non-blocking)
if grep -q '"error"' /tmp/bacon-output.json; then
    # Fix the error
fi
```

### 2. cargo watch (Test Runner)

**What:** Runs tests automatically when files change.

**Integration:**
```bash
cargo watch -x "test --no-fail-fast" 2>&1 | tee /tmp/test-output.txt &
```

### 3. fswatch + swiftlint (Swift/ObjC)

**What:** Watches for Swift/ObjC changes, runs linting.

**Integration:**
```bash
fswatch -0 sources/*.swift | while read -d '' file; do
    swiftlint lint "$file" >> /tmp/swift-errors.txt
done &
```

### 4. Feedback Aggregator

**What:** Combines all watcher outputs into unified status.

**Files produced:**
```
/tmp/dashterm-feedback/
├── status.json           # {"rust_build": "ok", "tests": "error", ...}
├── rust_errors.txt       # Current Rust compilation errors
├── rust_warnings.txt     # Clippy warnings
├── rust_test_fails.txt   # Failed test names
├── swift_errors.txt      # Swift/ObjC errors
├── last_update           # Timestamp
└── watcher_pids          # For cleanup
```

---

## Worker Integration

### Modified run_worker.sh

```bash
#!/bin/bash
# run_worker.sh with continuous feedback

# Start feedback loop before worker
./scripts/agentic-feedback-loop.sh start

# Trap to cleanup on exit
trap './scripts/agentic-feedback-loop.sh stop' EXIT

# Run worker with access to feedback files
export FEEDBACK_DIR="/tmp/dashterm-feedback"

PROMPT="You are a worker. Before committing, check $FEEDBACK_DIR/status.json.
If any component shows 'error', fix it first.
Read $FEEDBACK_DIR/rust_errors.txt for specific errors.

Your task: $TASK"

claude --dangerously-skip-permissions -p "$PROMPT" \
    --output-format stream-json \
    2>&1 | tee "$LOG_FILE"
```

### Agent Prompting

Add to worker system prompt:

```markdown
## Continuous Feedback Integration

You have access to continuous build feedback. Instead of running
`cargo build` or `xcodebuild` manually, check these files:

- `/tmp/dashterm-feedback/status.json` - Overall status
- `/tmp/dashterm-feedback/rust_errors.txt` - Current Rust errors
- `/tmp/dashterm-feedback/swift_errors.txt` - Swift/ObjC errors

**Workflow:**
1. Write code using Edit/Write tools
2. Wait 1-2 seconds for watchers to process
3. Read status.json to check if build passed
4. If errors, read the specific error file and fix
5. Repeat until status shows "ok"

**Commands available:**
- `./scripts/agentic-feedback-loop.sh errors` - Show all current errors
- `./scripts/agentic-feedback-loop.sh wait` - Block until all checks pass
```

---

## API for Agent Tools

### Check Status (Non-blocking)

```python
import json
import os

def check_build_status():
    """Returns current build status without blocking."""
    status_file = "/tmp/dashterm-feedback/status.json"
    if os.path.exists(status_file):
        with open(status_file) as f:
            return json.load(f)
    return {"status": "unknown"}

def get_errors():
    """Returns current errors."""
    errors = {}
    for name in ["rust_errors", "swift_errors", "rust_test_fails"]:
        path = f"/tmp/dashterm-feedback/{name}.txt"
        if os.path.exists(path):
            with open(path) as f:
                content = f.read().strip()
                if content:
                    errors[name] = content
    return errors

def is_clean():
    """Returns True if all checks pass."""
    status = check_build_status()
    return all(
        v.get("status") == "ok"
        for k, v in status.items()
        if k != "system"
    )
```

### Wait for Clean (Blocking)

```python
import time

def wait_for_clean(timeout=300):
    """Block until all checks pass or timeout."""
    start = time.time()
    while time.time() - start < timeout:
        if is_clean():
            return True
        time.sleep(2)
    return False
```

### MCP Tool Definition

```json
{
  "name": "check_build_status",
  "description": "Check current build/test status from continuous watchers",
  "parameters": {
    "component": {
      "type": "string",
      "enum": ["all", "rust_build", "rust_tests", "swift", "clippy"],
      "default": "all"
    }
  }
}
```

---

## Performance Comparison

| Approach | Time to First Error | Overhead |
|----------|---------------------|----------|
| Manual `cargo build` | 5-30s | High (full rebuild) |
| Manual `cargo check` | 2-10s | Medium |
| bacon continuous | **< 1s** | **Minimal** |
| cargo watch | 2-5s | Low |

**Estimated speedup:** 5-10x faster feedback loop for AI agents.

---

## Extended Tool Support

### Tools with Continuous/Watch Modes

| Tool | Command | Output | Best For |
|------|---------|--------|----------|
| **bacon** | `bacon` | Streaming | Rust compilation |
| **cargo watch** | `cargo watch -x check` | On-change | General Rust |
| **watchexec** | `watchexec -e rs cargo test` | Cross-platform | Any command |
| **entr** | `ls *.rs \| entr cargo check` | Simple | Unix pipes |
| **fswatch** | `fswatch -0 dir` | macOS native | File watching |
| **nodemon** | `nodemon --exec cargo test` | Node.js | Familiar syntax |
| **reflex** | `reflex -r '\.rs$' cargo check` | Go-based | Regex patterns |

### Language-Specific Watchers

| Language | Tool | Command |
|----------|------|---------|
| Rust | bacon | `bacon` |
| Swift | swift-watch (custom) | `fswatch + swiftc -parse` |
| TypeScript | tsc | `tsc --watch` |
| Python | pytest-watch | `ptw` |
| Go | air | `air` |

---

## Installation

```bash
# Required
cargo install bacon
cargo install cargo-watch
brew install fswatch

# Optional
brew install watchexec
brew install entr

# Verify
bacon --version
cargo watch --version
fswatch --version
```

---

## Usage

### Start Feedback Loop
```bash
./scripts/agentic-feedback-loop.sh start
```

### Check Status
```bash
./scripts/agentic-feedback-loop.sh status
```

### Get Current Errors
```bash
./scripts/agentic-feedback-loop.sh errors
```

### Wait for Clean Build
```bash
./scripts/agentic-feedback-loop.sh wait 300  # 5 min timeout
```

### Stop All Watchers
```bash
./scripts/agentic-feedback-loop.sh stop
```

---

## Future Enhancements

### 1. LSP Integration
- Use rust-analyzer LSP for even faster feedback
- Get errors as-you-type without saving

### 2. Smart Prioritization
- Track which errors agent is working on
- Prioritize checking affected files

### 3. Error Deduplication
- Group related errors
- Show root cause first

### 4. Predictive Compilation
- Start compiling before agent finishes writing
- Speculate on likely changes

### 5. Multi-Project Support
- Handle monorepos
- Coordinate watchers across workspaces

---

## Integration Checklist

- [ ] Install bacon, cargo-watch, fswatch
- [ ] Add `agentic-feedback-loop.sh` to scripts/
- [ ] Update worker prompt with feedback file locations
- [ ] Modify run_worker.sh to start/stop feedback loop
- [ ] Add MCP tool for status checking
- [ ] Test with sample worker task

---

*DashProve Agentic Integration - December 2025*
