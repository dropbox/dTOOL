//! Latency benchmarks for measuring response time characteristics.
//!
//! Run with: cargo bench --package dterm-core --bench latency
//!
//! ## Purpose
//!
//! This benchmark suite measures the latency characteristics that matter for
//! interactive terminal use - specifically, how long it takes for input to
//! be processed and ready for display.
//!
//! ## Metrics
//!
//! 1. **Keystroke processing latency** - Time to process a single character
//!    Target: <2ms (preferably <500μs)
//!
//! 2. **Small batch latency** - Time to process a short command output (~100 bytes)
//!    Target: <1ms
//!
//! 3. **Line processing latency** - Time to process a single line of output
//!    Target: <1ms
//!
//! 4. **Escape sequence latency** - Time to process common escape sequences
//!    Target: <500μs per sequence
//!
//! 5. **Frame budget utilization** - How much of a 16.6ms frame budget is consumed
//!    Target: <5ms for typical frame workload
//!
//! ## Reference
//!
//! - foot terminal: <1ms keystroke-to-render
//! - Ghostty: <1ms input-to-screen
//! - Human perception threshold: ~10-20ms for noticeable delay

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use dterm_core::prelude::Terminal;
use std::time::Duration;

/// Configure latency benchmarks with appropriate measurement time.
///
/// Latency benchmarks need more samples for statistical significance
/// since we're measuring small durations.
fn latency_criterion() -> Criterion {
    Criterion::default()
        .measurement_time(Duration::from_secs(5))
        .sample_size(1000)
        .warm_up_time(Duration::from_secs(1))
}

/// Single keystroke processing latency.
///
/// Measures the time to process a single printable character.
/// This is the absolute minimum latency for any input.
fn bench_keystroke_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("latency/keystroke");
    group.measurement_time(Duration::from_secs(5));
    group.sample_size(1000);

    // Pre-create terminal to exclude setup from measurement
    let mut term = Terminal::new(24, 80);

    // Single ASCII character
    group.bench_function("single_ascii", |b| {
        b.iter(|| {
            term.process(black_box(b"a"));
        });
    });

    // Single newline (triggers line operations)
    group.bench_function("single_newline", |b| {
        b.iter(|| {
            term.process(black_box(b"\n"));
        });
    });

    // Single carriage return + newline (common)
    group.bench_function("crlf", |b| {
        b.iter(|| {
            term.process(black_box(b"\r\n"));
        });
    });

    // Single UTF-8 character (2-byte)
    group.bench_function("utf8_2byte", |b| {
        b.iter(|| {
            term.process(black_box("é".as_bytes()));
        });
    });

    // Single UTF-8 character (3-byte CJK)
    group.bench_function("utf8_3byte_cjk", |b| {
        b.iter(|| {
            term.process(black_box("中".as_bytes()));
        });
    });

    // Single UTF-8 character (4-byte emoji)
    group.bench_function("utf8_4byte_emoji", |b| {
        b.iter(|| {
            term.process(black_box("😀".as_bytes()));
        });
    });

    group.finish();
}

/// Escape sequence processing latency.
///
/// Measures time to process common escape sequences.
/// These are frequently used in interactive applications.
fn bench_escape_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("latency/escape");
    group.measurement_time(Duration::from_secs(5));
    group.sample_size(500);

    let mut term = Terminal::new(24, 80);

    // SGR reset (very common)
    group.bench_function("sgr_reset", |b| {
        b.iter(|| {
            term.process(black_box(b"\x1b[0m"));
        });
    });

    // SGR single attribute (bold)
    group.bench_function("sgr_bold", |b| {
        b.iter(|| {
            term.process(black_box(b"\x1b[1m"));
        });
    });

    // SGR foreground color (256-color)
    group.bench_function("sgr_fg_256", |b| {
        b.iter(|| {
            term.process(black_box(b"\x1b[38;5;196m"));
        });
    });

    // SGR RGB color
    group.bench_function("sgr_rgb", |b| {
        b.iter(|| {
            term.process(black_box(b"\x1b[38;2;255;128;64m"));
        });
    });

    // Cursor movement (single)
    group.bench_function("cursor_up", |b| {
        term.process(b"\x1b[12;40H"); // Center cursor first
        b.iter(|| {
            term.process(black_box(b"\x1b[A"));
            term.process(black_box(b"\x1b[B")); // Move back
        });
    });

    // Cursor position (CUP)
    group.bench_function("cursor_position", |b| {
        b.iter(|| {
            term.process(black_box(b"\x1b[12;40H"));
        });
    });

    // Erase to end of line
    group.bench_function("erase_to_eol", |b| {
        term.process(b"\x1b[H"); // Home
        b.iter(|| {
            term.process(black_box(b"\x1b[K"));
        });
    });

    // Clear screen
    group.bench_function("clear_screen", |b| {
        b.iter(|| {
            term.process(black_box(b"\x1b[2J"));
        });
    });

    // Scroll up single line
    group.bench_function("scroll_up", |b| {
        b.iter(|| {
            term.process(black_box(b"\x1b[S"));
        });
    });

    // Save/restore cursor
    group.bench_function("save_restore_cursor", |b| {
        b.iter(|| {
            term.process(black_box(b"\x1b7\x1b8"));
        });
    });

    group.finish();
}

/// Short command output latency.
///
/// Simulates processing typical shell command output.
fn bench_command_output_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("latency/command");
    group.measurement_time(Duration::from_secs(5));
    group.sample_size(200);

    let mut term = Terminal::new(24, 80);

    // pwd output (~20 bytes)
    let pwd_output = b"/Users/developer/projects\n";
    group.bench_function("pwd", |b| {
        b.iter(|| {
            term.process(black_box(pwd_output));
        });
    });

    // echo output (~50 bytes)
    let echo_output = b"Hello, World! This is a test message.\n";
    group.bench_function("echo", |b| {
        b.iter(|| {
            term.process(black_box(echo_output));
        });
    });

    // Single ls entry with colors (~100 bytes)
    let ls_entry = b"\x1b[0m\x1b[01;34mDocuments\x1b[0m  \x1b[01;34mDownloads\x1b[0m  \x1b[01;32mscript.sh\x1b[0m\n";
    group.bench_function("ls_colored_line", |b| {
        b.iter(|| {
            term.process(black_box(ls_entry));
        });
    });

    // git status line (~150 bytes)
    let git_line = b"\x1b[32m        modified:   src/terminal/mod.rs\x1b[0m\n";
    group.bench_function("git_status_line", |b| {
        b.iter(|| {
            term.process(black_box(git_line));
        });
    });

    // Prompt line with colors (~100 bytes)
    let prompt = b"\x1b[1;32muser@host\x1b[0m:\x1b[1;34m~/projects/dterm\x1b[0m$ ";
    group.bench_function("shell_prompt", |b| {
        b.iter(|| {
            term.process(black_box(prompt));
        });
    });

    group.finish();
}

/// Line processing latency at various widths.
///
/// Measures time to process complete lines of different lengths.
fn bench_line_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("latency/line");
    group.measurement_time(Duration::from_secs(5));
    group.sample_size(200);

    let mut term = Terminal::new(24, 80);

    let line_sizes = [20, 40, 80, 120, 200];

    for size in line_sizes {
        // Plain ASCII line
        let plain_line: Vec<u8> = std::iter::repeat_n(b'X', size)
            .chain(std::iter::once(b'\n'))
            .collect();

        group.bench_with_input(BenchmarkId::new("plain", size), &plain_line, |b, line| {
            b.iter(|| {
                term.process(black_box(line));
            });
        });
    }

    // Line with wrapping (>80 chars on 80-col terminal)
    let wrap_line: Vec<u8> = std::iter::repeat_n(b'W', 100)
        .chain(std::iter::once(b'\n'))
        .collect();
    group.bench_function("with_wrap", |b| {
        b.iter(|| {
            term.process(black_box(&wrap_line));
        });
    });

    group.finish();
}

/// Frame budget benchmark.
///
/// Measures how much work can be done in a 16.6ms frame budget (60 FPS).
/// A typical frame should leave headroom for rendering.
fn bench_frame_budget(c: &mut Criterion) {
    let mut group = c.benchmark_group("latency/frame_budget");
    group.measurement_time(Duration::from_secs(5));
    group.sample_size(100);

    let mut term = Terminal::new(24, 80);

    // Simulate typical frame: ~10 lines of output
    let frame_output: Vec<u8> = (0..10)
        .flat_map(|i| format!("Line {}: Some typical terminal output here\r\n", i).into_bytes())
        .collect();

    group.bench_function("typical_10_lines", |b| {
        b.iter(|| {
            term.process(black_box(&frame_output));
        });
    });

    // Heavy frame: screen full of scrolling output
    let heavy_output: Vec<u8> = (0..24)
        .flat_map(|i| {
            format!(
                "\x1b[33mLine {:2}\x1b[0m: Heavy output with colors and content\r\n",
                i
            )
            .into_bytes()
        })
        .collect();

    group.bench_function("heavy_24_lines_colored", |b| {
        b.iter(|| {
            term.process(black_box(&heavy_output));
        });
    });

    // Cursor-heavy frame (vim-like): many cursor movements
    let cursor_heavy: Vec<u8> = (0..100)
        .flat_map(|i| format!("\x1b[{};{}H*", (i % 24) + 1, (i % 80) + 1).into_bytes())
        .collect();

    group.bench_function("cursor_heavy_100_moves", |b| {
        b.iter(|| {
            term.process(black_box(&cursor_heavy));
        });
    });

    group.finish();
}

/// Interactive typing simulation.
///
/// Simulates typing a command character by character,
/// measuring per-character overhead.
fn bench_typing_simulation(c: &mut Criterion) {
    let mut group = c.benchmark_group("latency/typing");
    group.measurement_time(Duration::from_secs(5));
    group.sample_size(200);

    let mut term = Terminal::new(24, 80);

    // Type a simple command: "ls -la"
    let command = b"ls -la";
    group.bench_function("type_ls_la", |b| {
        b.iter(|| {
            for byte in command.iter() {
                term.process(black_box(std::slice::from_ref(byte)));
            }
        });
    });

    // Type a longer command: "git commit -m 'message'"
    let git_command = b"git commit -m 'Initial commit message'";
    group.bench_function("type_git_commit", |b| {
        b.iter(|| {
            for byte in git_command.iter() {
                term.process(black_box(std::slice::from_ref(byte)));
            }
        });
    });

    // Backspace correction (common during typing)
    let with_backspace = b"mistke\x08\x08ake"; // "mistake" with correction
    group.bench_function("type_with_backspace", |b| {
        b.iter(|| {
            term.process(black_box(with_backspace));
        });
    });

    group.finish();
}

/// Terminal state query latency.
///
/// Measures time to query terminal state (used for rendering decisions).
fn bench_state_query_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("latency/state_query");
    group.measurement_time(Duration::from_secs(5));
    group.sample_size(1000);

    // Create a terminal with some content
    let mut term = Terminal::new(24, 80);
    term.process(b"Some content on the terminal\r\n");
    term.process(b"\x1b[31mColored text\x1b[0m\r\n");

    // Cursor position query
    group.bench_function("cursor_position", |b| {
        b.iter(|| black_box(term.cursor()));
    });

    // Grid access
    group.bench_function("grid_access", |b| {
        b.iter(|| black_box(term.grid()));
    });

    // Cell read at specific position
    group.bench_function("cell_read", |b| {
        let grid = term.grid();
        b.iter(|| black_box(grid.cell(0, 0)));
    });

    // Row iteration
    group.bench_function("row_read", |b| {
        let grid = term.grid();
        b.iter(|| black_box(grid.row(0)));
    });

    group.finish();
}

/// Terminal creation latency.
///
/// Measures cold-start performance.
fn bench_terminal_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("latency/creation");
    group.measurement_time(Duration::from_secs(5));
    group.sample_size(200);

    // Standard terminal
    group.bench_function("standard_24x80", |b| {
        b.iter(|| {
            let term = Terminal::new(black_box(24), black_box(80));
            black_box(term.cursor())
        });
    });

    // Large terminal
    group.bench_function("large_50x132", |b| {
        b.iter(|| {
            let term = Terminal::new(black_box(50), black_box(132));
            black_box(term.cursor())
        });
    });

    // Very large terminal (4K equivalent)
    group.bench_function("huge_100x200", |b| {
        b.iter(|| {
            let term = Terminal::new(black_box(100), black_box(200));
            black_box(term.cursor())
        });
    });

    group.finish();
}

/// Summary benchmark: end-to-end interactive latency.
///
/// This benchmark simulates a realistic interactive session
/// and measures overall responsiveness.
fn bench_interactive_session(c: &mut Criterion) {
    let mut group = c.benchmark_group("latency/interactive_summary");
    group.measurement_time(Duration::from_secs(5));
    group.sample_size(100);

    let mut term = Terminal::new(24, 80);

    // Simulate: user types command, gets output, types another
    let session_sequence: Vec<&[u8]> = vec![
        // Shell prompt
        b"\x1b[1;32muser@host\x1b[0m:\x1b[1;34m~\x1b[0m$ ",
        // User types "ls"
        b"l",
        b"s",
        b"\r",
        // Output
        b"\x1b[0m\x1b[01;34mDocuments\x1b[0m  \x1b[01;34mDownloads\x1b[0m  file.txt\r\n",
        // Next prompt
        b"\x1b[1;32muser@host\x1b[0m:\x1b[1;34m~\x1b[0m$ ",
    ];

    group.bench_function("shell_interaction", |b| {
        b.iter(|| {
            for chunk in &session_sequence {
                term.process(black_box(*chunk));
            }
        });
    });

    // Vim-like interaction: cursor movements + edits
    let vim_sequence: Vec<&[u8]> = vec![
        b"\x1b[H",  // Home
        b"\x1b[2J", // Clear
        b"Line 1: Some code here\r\n",
        b"Line 2: More code\r\n",
        b"\x1b[1;1H", // Go to line 1
        b"\x1b[K",    // Clear line
        b"Line 1: Modified\r\n",
        b"\x1b[2;1H", // Go to line 2
    ];

    group.bench_function("vim_like_edit", |b| {
        b.iter(|| {
            for chunk in &vim_sequence {
                term.process(black_box(*chunk));
            }
        });
    });

    group.finish();
}

criterion_group! {
    name = benches;
    config = latency_criterion();
    targets = bench_keystroke_latency,
              bench_escape_latency,
              bench_command_output_latency,
              bench_line_latency,
              bench_frame_budget,
              bench_typing_simulation,
              bench_state_query_latency,
              bench_terminal_creation,
              bench_interactive_session,
}

criterion_main!(benches);
