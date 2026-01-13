# AI Technical Specification
## Inky (inky-tui)

**Purpose**: Rust-native terminal UI library with React-like components, flexbox layout, retained-mode rendering, and AI assistant widgets.
**Scope**: Architecture, APIs, modules, tools, testing for the `inky` library (package `inky-tui`), examples, and optional features.
**Audience**: AI coding agents working in this repo.

---

## Architecture Overview

Inky renders a retained tree of nodes (boxes, text, statics) into a cell buffer,
computes layout via Taffy (flexbox/grid), and diffs buffers to emit minimal
terminal updates. Backends include ANSI/crossterm by default and optional GPU
rendering via wgpu (feature-gated).

### Rendering Pipeline

1. Build a `Node` tree (components and macros).
2. Compute layout with Taffy from `style` and `layout` modules.
3. Render nodes into a `Buffer` of `Cell`s.
4. Diff current vs previous buffers and emit changes.
5. Output via terminal backends (crossterm, optional GPU).

---

## Core Modules

- `src/lib.rs`: Public API exports and crate-level docs.
- `src/app.rs`: Application runner, event loop, render scheduling.
- `src/node.rs`: Node types (Box/Text/Static) and tree structure.
- `src/style.rs`: Style model, mapping to layout.
- `src/layout.rs`: Taffy integration and layout cache.
- `src/render/`: Buffer, cells, renderers, GPU buffer abstractions.
- `src/diff.rs`: Buffer diffing for incremental rendering.
- `src/hooks/`: Signals, input, focus, and hook APIs.
- `src/components/`: Built-in components (input, select, progress, charts, AI views).
- `src/macros.rs`: `ink!` and layout helper macros.
- `src/perception.rs`: Text/token render views and optional image snapshots.
- `src/clipboard.rs`: OSC 52 clipboard copy/paste helpers.
- `src/animation.rs`: Easing and animation primitives.
- `src/elm.rs`: Model-Update-View style architecture helpers.
- `src/stylesheet.rs`: Named styles and cascading application.
- `src/accessibility.rs`: ARIA roles and announcements.
- `src/terminal/`: Terminal backends and signal handling.

---

## Features and Optional Dependencies

- `gpu`: Enables wgpu-backed GPU rendering paths.
- `image`: Enables PNG snapshot output for Perception APIs.
- `tracing`: Enables tracing instrumentation.

---

## Key Dependencies

- `taffy`: Flexbox/grid layout engine.
- `crossterm`: Terminal I/O, events, and rendering.
- `pulldown-cmark`: Markdown parsing for Markdown component.
- `unicode-width`: Text width calculation.
- `bitflags`, `smallvec`, `smartstring`, `indexmap`, `rustc-hash`: Performance- and memory-focused utilities.

---

## Build, Test, and Tooling

Required checks before commit:

```bash
cargo fmt
cargo clippy --all-features -- -D warnings
cargo test
```

Quick checks and examples:

```bash
cargo check
cargo run --example hello
cargo run --example codex_tui
```

MSRV is Rust 1.70 (see `Cargo.toml`).

---

## Examples

Examples live under `examples/` and include:

- `hello`, `counter`, `widgets`, `focus`, `form`
- `visualization`, `dashboard`
- `codex_tui`, `claude_tui`

---

## Documentation Sources

- `README.md`: Project overview and installation.
- `docs/ARCHITECTURE_PLAN.md`: Detailed design and phase status.
- `docs/WORKER_DIRECTIONS.md`: Execution and phase tracking.
