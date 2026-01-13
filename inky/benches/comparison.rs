#![allow(clippy::unwrap_used)]
#![allow(missing_docs)]
//! Competitive benchmark suite comparing inky to other TUI frameworks.
//!
//! Run with: `cargo bench --bench comparison`
//!
//! This benchmark suite compares inky's performance against:
//! - ratatui (Rust) - when `compat-ratatui` feature is enabled
//!
//! ## Standard Scenarios
//!
//! 1. **Empty terminal** - Startup/buffer creation cost
//! 2. **Text grid** - Layout + render (10x10, 50x50)
//! 3. **Chat UI** - Realistic app (10, 100, 1000 messages)
//! 4. **Full redraw** - Worst case (80x24, 200x50)
//! 5. **Incremental render** - Stable tree (realistic app scenario)
//!
//! ## Running with HTML reports
//!
//! ```bash
//! cargo bench --bench comparison -- --save-baseline inky-v1
//! cargo bench --bench comparison -- --baseline inky-v1
//! ```
//!
//! Reports are saved to `target/criterion/`.

mod comparison_impl;

use criterion::{criterion_group, criterion_main};

criterion_group!(
    benches,
    comparison_impl::bench_empty_terminal,
    comparison_impl::bench_text_grid,
    comparison_impl::bench_chat_ui,
    comparison_impl::bench_full_redraw,
    comparison_impl::bench_incremental_render,
);

criterion_main!(benches);
