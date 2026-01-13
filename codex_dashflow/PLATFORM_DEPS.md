# Platform Dependencies

This document tracks DashFlow platform features that codex_dashflow depends on or is ready to use.

## list_threads() - Session Listing (Integrated)

**DashFlow Commit**: a20487e
**Date**: 2025-12-07
**Status**: ✅ Fully Integrated (N=431+)

The DashFlow Checkpointer trait now includes a `list_threads()` method for enumerating all sessions (threads) that have checkpoints. This enables implementing session listing/management features.

### API

```rust
use dashflow::{Checkpointer, ThreadInfo};

// List all threads with checkpoints
let threads: Vec<ThreadInfo> = checkpointer.list_threads().await?;

for thread in threads {
    println!("Session: {} (updated: {:?})", thread.thread_id, thread.updated_at);
}
```

### ThreadInfo Fields

| Field | Type | Description |
|-------|------|-------------|
| `thread_id` | `String` | Session identifier |
| `latest_checkpoint_id` | `String` | ID of most recent checkpoint |
| `updated_at` | `SystemTime` | When last checkpoint was saved |
| `checkpoint_count` | `Option<usize>` | Number of checkpoints (may be None) |

### Integration Status

All features using this API are now implemented:

1. ✅ `sessions` subcommand - Lists saved sessions (`crates/cli/src/lib.rs`)
2. ✅ `--session` flag without argument - Resumes latest session
3. ✅ Session management - View (`--info`), delete (`--delete`, `--delete-all`)
4. ✅ TUI `/sessions` and `/delete` commands (`crates/tui/src/app.rs`)
5. ✅ Auto-resume on TUI startup (`auto_resume` config option)

### Implementation Location

- `crates/core/src/runner.rs:988` - `list_sessions()` wraps `list_threads()`
- `crates/core/src/runner.rs:961` - Re-exports `ThreadInfo` from DashFlow
