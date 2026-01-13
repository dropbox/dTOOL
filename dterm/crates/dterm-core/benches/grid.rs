//! Grid benchmarks.
//!
//! Run with: cargo bench --package dterm-core --bench grid

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use dterm_core::grid::Grid;
use dterm_core::scrollback::Scrollback;

fn bench_grid_write(c: &mut Criterion) {
    let mut group = c.benchmark_group("grid_write");

    // Test writing characters at different grid sizes
    let sizes = [(24, 80), (50, 132), (100, 200)];

    for (rows, cols) in sizes {
        let name = format!("{}x{}", rows, cols);
        let chars_per_screen = (rows * cols) as u64;
        group.throughput(Throughput::Elements(chars_per_screen));

        group.bench_with_input(
            BenchmarkId::new("write_char", &name),
            &(rows, cols),
            |b, &(rows, cols)| {
                b.iter(|| {
                    let mut grid = Grid::new(rows, cols);
                    for _ in 0..rows {
                        for _ in 0..cols {
                            grid.write_char('X');
                        }
                        grid.line_feed();
                        grid.carriage_return();
                    }
                    grid.cursor_row()
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("write_char_wrap", &name),
            &(rows, cols),
            |b, &(rows, cols)| {
                b.iter(|| {
                    let mut grid = Grid::new(rows, cols);
                    // Write more than screen can hold to test wrap + scroll
                    for _ in 0..(rows * cols * 2) {
                        grid.write_char_wrap('X');
                    }
                    grid.cursor_row()
                });
            },
        );
    }

    group.finish();
}

fn bench_grid_scroll(c: &mut Criterion) {
    let mut group = c.benchmark_group("grid_scroll");

    // Pre-fill grid with content
    let rows = 24;
    let cols = 80;
    let scrollback_sizes = [100, 1000, 10_000];

    for scrollback in scrollback_sizes {
        let name = format!("sb_{}", scrollback);

        group.bench_with_input(
            BenchmarkId::new("scroll_up_single", &name),
            &scrollback,
            |b, &scrollback| {
                let mut grid = Grid::with_scrollback(rows, cols, scrollback);
                // Fill with content
                for i in 0..((rows as usize) + scrollback / 2) {
                    for c in format!("Line {i}").chars() {
                        grid.write_char(c);
                    }
                    grid.line_feed();
                    grid.carriage_return();
                }

                b.iter(|| {
                    grid.scroll_up(black_box(1));
                    grid.scrollback_lines()
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("scroll_display", &name),
            &scrollback,
            |b, &scrollback| {
                let mut grid = Grid::with_scrollback(rows, cols, scrollback);
                // Fill with content
                for i in 0..(rows as usize + scrollback) {
                    for c in format!("Line {i}").chars() {
                        grid.write_char(c);
                    }
                    grid.line_feed();
                    grid.carriage_return();
                }

                b.iter(|| {
                    grid.scroll_display(black_box(10));
                    grid.scroll_display(black_box(-10));
                    grid.display_offset()
                });
            },
        );
    }

    group.finish();
}

fn bench_grid_scroll_with_tiered(c: &mut Criterion) {
    let mut group = c.benchmark_group("grid_scroll_tiered");

    // Test with tiered scrollback
    let ring_sizes = [100, 500, 1000];

    for ring_size in ring_sizes {
        let name = format!("ring_{}", ring_size);

        group.bench_with_input(
            BenchmarkId::new("scroll_to_tiered", &name),
            &ring_size,
            |b, &ring_size| {
                b.iter(|| {
                    let scrollback = Scrollback::new(1000, 10_000, 100_000_000);
                    let mut grid = Grid::with_tiered_scrollback(24, 80, ring_size, scrollback);

                    // Write enough to push to tiered scrollback
                    for i in 0..5000 {
                        for c in format!("Line {i}").chars() {
                            grid.write_char(c);
                        }
                        grid.line_feed();
                        grid.carriage_return();
                    }

                    (
                        grid.ring_buffer_scrollback(),
                        grid.tiered_scrollback_lines(),
                    )
                });
            },
        );
    }

    group.finish();
}

fn bench_grid_resize(c: &mut Criterion) {
    let mut group = c.benchmark_group("grid_resize");

    let transitions = [
        ("grow", (24, 80), (50, 132)),
        ("shrink", (50, 132), (24, 80)),
        ("same_cols", (24, 80), (50, 80)),
        ("same_rows", (24, 80), (24, 132)),
    ];

    for (name, (from_rows, from_cols), (to_rows, to_cols)) in transitions {
        group.bench_function(BenchmarkId::new("resize", name), |b| {
            b.iter(|| {
                let mut grid = Grid::new(from_rows, from_cols);
                // Fill with content
                for _ in 0..from_rows {
                    for _ in 0..from_cols {
                        grid.write_char('X');
                    }
                    grid.line_feed();
                    grid.carriage_return();
                }
                grid.resize(black_box(to_rows), black_box(to_cols));
                (grid.rows(), grid.cols())
            });
        });
    }

    group.finish();
}

fn bench_grid_cell_access(c: &mut Criterion) {
    let mut group = c.benchmark_group("grid_cell_access");

    let sizes = [(24, 80), (50, 132)];

    for (rows, cols) in sizes {
        let name = format!("{}x{}", rows, cols);

        // Pre-fill grid
        let mut grid = Grid::new(rows, cols);
        for row in 0..rows {
            for col in 0..cols {
                grid.set_cursor(row, col);
                grid.write_char('X');
            }
        }

        group.bench_with_input(BenchmarkId::new("cell_read", &name), &grid, |b, grid| {
            b.iter(|| {
                let mut sum: u32 = 0;
                for row in 0..grid.rows() {
                    for col in 0..grid.cols() {
                        if let Some(cell) = grid.cell(row, col) {
                            sum = sum.wrapping_add(cell.char() as u32);
                        }
                    }
                }
                sum
            });
        });

        group.bench_with_input(BenchmarkId::new("row_read", &name), &grid, |b, grid| {
            b.iter(|| {
                let mut sum: u32 = 0;
                for row_idx in 0..grid.rows() {
                    if let Some(row) = grid.row(row_idx) {
                        for cell in row.iter() {
                            sum = sum.wrapping_add(cell.char() as u32);
                        }
                    }
                }
                sum
            });
        });
    }

    group.finish();
}

fn bench_grid_cursor(c: &mut Criterion) {
    let mut group = c.benchmark_group("grid_cursor");

    let mut grid = Grid::new(24, 80);

    group.bench_function("set_cursor", |b| {
        b.iter(|| {
            for row in 0..24 {
                for col in 0..80 {
                    grid.set_cursor(black_box(row), black_box(col));
                }
            }
            grid.cursor_row()
        });
    });

    group.bench_function("move_cursor_by", |b| {
        b.iter(|| {
            grid.set_cursor(12, 40);
            for _ in 0..1000 {
                grid.move_cursor_by(black_box(1), black_box(0));
                grid.move_cursor_by(black_box(-1), black_box(0));
                grid.move_cursor_by(black_box(0), black_box(1));
                grid.move_cursor_by(black_box(0), black_box(-1));
            }
            grid.cursor_row()
        });
    });

    group.bench_function("cursor_save_restore", |b| {
        b.iter(|| {
            for _ in 0..1000 {
                grid.save_cursor();
                grid.set_cursor(0, 0);
                grid.restore_cursor();
            }
            grid.cursor_row()
        });
    });

    group.finish();
}

fn bench_grid_erase(c: &mut Criterion) {
    let mut group = c.benchmark_group("grid_erase");

    group.bench_function("erase_line", |b| {
        let mut grid = Grid::new(24, 80);
        // Fill with content
        for _ in 0..80 {
            grid.write_char('X');
        }

        b.iter(|| {
            grid.set_cursor(0, 40);
            grid.erase_line();
            // Refill
            grid.set_cursor(0, 0);
            for _ in 0..80 {
                grid.write_char('X');
            }
        });
    });

    group.bench_function("erase_screen", |b| {
        let mut grid = Grid::new(24, 80);

        b.iter(|| {
            // Fill
            for row in 0..24 {
                grid.set_cursor(row, 0);
                for _ in 0..80 {
                    grid.write_char('X');
                }
            }
            // Erase
            grid.erase_screen();
        });
    });

    group.bench_function("erase_to_end_of_screen", |b| {
        let mut grid = Grid::new(24, 80);
        // Fill
        for row in 0..24 {
            grid.set_cursor(row, 0);
            for _ in 0..80 {
                grid.write_char('X');
            }
        }

        b.iter(|| {
            grid.set_cursor(12, 40);
            grid.erase_to_end_of_screen();
        });
    });

    group.finish();
}

fn bench_grid_history(c: &mut Criterion) {
    let mut group = c.benchmark_group("grid_history");

    // Create grid with tiered scrollback and fill it
    let scrollback = Scrollback::new(1000, 10_000, 100_000_000);
    let mut grid = Grid::with_tiered_scrollback(24, 80, 500, scrollback);

    // Fill with content
    for i in 0..5000 {
        for c in format!("Line {i}").chars() {
            grid.write_char(c);
        }
        grid.line_feed();
        grid.carriage_return();
    }

    group.bench_function("get_history_line_recent", |b| {
        let total = grid.history_line_count();
        b.iter(|| {
            // Get recent lines (should be in ring buffer)
            for i in 0..100 {
                black_box(grid.get_history_line(total - 1 - i));
            }
        });
    });

    group.bench_function("get_history_line_old", |b| {
        b.iter(|| {
            // Get old lines (should be in tiered scrollback)
            for i in 0..100 {
                black_box(grid.get_history_line(i));
            }
        });
    });

    group.bench_function("get_history_line_rev", |b| {
        b.iter(|| {
            for i in 0..100 {
                black_box(grid.get_history_line_rev(i));
            }
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_grid_write,
    bench_grid_scroll,
    bench_grid_scroll_with_tiered,
    bench_grid_resize,
    bench_grid_cell_access,
    bench_grid_cursor,
    bench_grid_erase,
    bench_grid_history,
);
criterion_main!(benches);
