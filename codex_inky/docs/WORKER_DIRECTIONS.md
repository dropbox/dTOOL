# WORKER DIRECTIONS

**Manager:** Claude (MANAGER)
**Date:** 2025-12-30
**Current Phase:** Phase 1 - Setup & First Port

---

## MISSION

Port Codex CLI TUI from ratatui to inky.

**End goal:** A working Codex CLI that uses inky-tui instead of ratatui.

---

## WORKER 0 Assignment: Phase 1 - Add inky Dependency

### Task 1: Add inky-tui to Cargo.toml

Edit `codex-rs/tui/Cargo.toml`:

```toml
[dependencies]
# ... existing deps ...

# inky TUI library (replacing ratatui)
inky-tui = { path = "../../../inky" }
```

### Task 2: Verify Build

```bash
cd ~/codex_inky/codex-rs
cargo check -p codex-tui
```

Fix any immediate issues.

### Task 3: Study markdown_render.rs

```bash
wc -l codex-rs/tui/src/markdown_render.rs
head -100 codex-rs/tui/src/markdown_render.rs
```

Understand:
- How they parse markdown (pulldown-cmark)
- How they convert to ratatui Spans/Lines
- What styles they apply

### Task 4: Create Porting Plan

Document in `docs/PORTING_PLAN.md`:
- List all files that use ratatui
- Estimate complexity of each
- Propose porting order

### Acceptance Criteria

1. `cargo check -p codex-tui` passes with inky-tui added
2. `docs/PORTING_PLAN.md` exists with file list
3. No breaking changes to existing code yet

### Commit Format

```
# 0: Phase 1 - Add inky-tui dependency and create porting plan

**Current Plan**: docs/PORTING_PLAN.md
**Phase**: Phase 1: Setup

## Changes
- Added inky-tui dependency to codex-tui
- Created porting plan with file inventory
- Identified porting order

## Tests
- cargo check passes

## Next AI
- Phase 2: Port markdown_render.rs
```

---

## Phase Roadmap

```
Phase 1: Setup & Planning          ░░░░░░░░░░░░░░░░░░░░ ← YOU ARE HERE
Phase 2: Port markdown_render.rs   ░░░░░░░░░░░░░░░░░░░░
Phase 3: Port chatwidget.rs        ░░░░░░░░░░░░░░░░░░░░
Phase 4: Port remaining widgets    ░░░░░░░░░░░░░░░░░░░░
Phase 5: Port app.rs / tui.rs      ░░░░░░░░░░░░░░░░░░░░
Phase 6: Remove ratatui            ░░░░░░░░░░░░░░░░░░░░
Phase 7: Test & Polish             ░░░░░░░░░░░░░░░░░░░░
```

---

## Key Files to Port

| File | Lines | Complexity | Priority |
|------|-------|------------|----------|
| `markdown_render.rs` | 600+ | Medium | 1st |
| `chatwidget.rs` | 3800+ | High | 2nd |
| `diff_render.rs` | 650+ | Medium | 3rd |
| `history_cell.rs` | 2500+ | High | 4th |
| `app.rs` | 1800+ | High | 5th |
| `tui.rs` | 500+ | Medium | 6th |

---

## Do NOT

- Change application logic (only rendering)
- Remove ratatui until all modules ported
- Break existing tests
- Skip the planning phase

Start with understanding, WORKER. Then port systematically.
