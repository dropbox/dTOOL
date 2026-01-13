# [MANAGER] FIX: Worker Cargo PATH Issue

**Date:** November 23, 2025
**Priority:** URGENT
**Issue:** Worker cannot find `cargo` command

---

## Problem

Worker shell session doesn't have cargo in PATH:
```
cargo test --workspace
→ (eval):1: command not found: cargo
```

## Root Cause

Worker's shell environment doesn't source `~/.cargo/env` automatically.

## Solution

**Use full path to cargo in all commands:**

```bash
# Instead of:
cargo test --workspace

# Use:
/Users/ayates/.cargo/bin/cargo test --workspace
```

**OR set PATH at start of each command:**
```bash
PATH="$HOME/.cargo/bin:$PATH" cargo test --workspace
```

## For This Session

Since you've already confirmed you're N=6, use the full path for all cargo commands:

```bash
/Users/ayates/.cargo/bin/cargo test --workspace 2>&1 | tail -20
/Users/ayates/.cargo/bin/cargo build --workspace
/Users/ayates/.cargo/bin/cargo clippy --workspace -- -D warnings
```

## Permanent Fix (For Future Reference)

The `run_worker.sh` script should set PATH before launching Claude. But for now, just use the full path.

---

**IMMEDIATE ACTION:** Use `/Users/ayates/.cargo/bin/cargo` for all cargo commands this session.
