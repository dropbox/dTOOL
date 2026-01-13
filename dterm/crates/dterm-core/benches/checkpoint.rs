//! Checkpoint benchmarks.
//!
//! ## Running
//!
//! ```bash
//! cargo bench --bench checkpoint
//! ```
//!
//! ## Metrics
//!
//! - `checkpoint/save_empty`: Save empty grid
//! - `checkpoint/save_full`: Save grid with full content
//! - `checkpoint/save_scrollback`: Save grid with scrollback
//! - `checkpoint/restore_empty`: Restore empty checkpoint
//! - `checkpoint/restore_full`: Restore full checkpoint
//! - `checkpoint/restore_scrollback`: Restore checkpoint with scrollback

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use dterm_core::checkpoint::{CheckpointConfig, CheckpointManager};
use dterm_core::grid::Grid;
use dterm_core::scrollback::Scrollback;
use tempfile::tempdir;

/// Create a grid with content.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn create_grid_with_content(rows: u16, cols: u16, fill_ratio: f64) -> Grid {
    let mut grid = Grid::new(rows, cols);
    // SAFETY: f64 product of u16 values with fill_ratio <= 1.0 fits in usize
    let total_chars = ((f64::from(rows)) * (f64::from(cols)) * fill_ratio) as usize;

    for i in 0..total_chars {
        // SAFETY: i % 26 always produces 0-25, which fits in u8
        let c = (b'A' + (i % 26) as u8) as char;
        grid.write_char_wrap(c);
    }

    grid
}

/// Create a scrollback with lines.
fn create_scrollback_with_lines(line_count: usize) -> Scrollback {
    let mut scrollback = Scrollback::new(1000, 10_000, 100_000_000);

    for i in 0..line_count {
        scrollback.push_str(&format!(
            "Line {}: Lorem ipsum dolor sit amet, consectetur adipiscing elit.",
            i
        ));
    }

    scrollback
}

fn bench_save(c: &mut Criterion) {
    let mut group = c.benchmark_group("checkpoint/save");

    // Empty grid
    group.bench_function("empty_grid", |b| {
        let dir = tempdir().unwrap();
        let mut manager = CheckpointManager::new(dir.path());
        let grid = Grid::new(24, 80);

        b.iter(|| {
            manager.save(black_box(&grid), None).unwrap();
        });
    });

    // Grid 24x80 full of content
    group.bench_function("24x80_full", |b| {
        let dir = tempdir().unwrap();
        let mut manager = CheckpointManager::new(dir.path());
        let grid = create_grid_with_content(24, 80, 1.0);

        b.iter(|| {
            manager.save(black_box(&grid), None).unwrap();
        });
    });

    // Grid 50x200 half content
    group.bench_function("50x200_half", |b| {
        let dir = tempdir().unwrap();
        let mut manager = CheckpointManager::new(dir.path());
        let grid = create_grid_with_content(50, 200, 0.5);

        b.iter(|| {
            manager.save(black_box(&grid), None).unwrap();
        });
    });

    // Grid with 1K lines scrollback
    group.bench_function("with_1k_scrollback", |b| {
        let dir = tempdir().unwrap();
        let mut manager = CheckpointManager::new(dir.path());
        let grid = Grid::new(24, 80);
        let scrollback = create_scrollback_with_lines(1000);

        b.iter(|| {
            manager
                .save(black_box(&grid), Some(black_box(&scrollback)))
                .unwrap();
        });
    });

    // Grid with 10K lines scrollback
    group.bench_function("with_10k_scrollback", |b| {
        let dir = tempdir().unwrap();
        let mut manager = CheckpointManager::new(dir.path());
        let grid = Grid::new(24, 80);
        let scrollback = create_scrollback_with_lines(10_000);

        b.iter(|| {
            manager
                .save(black_box(&grid), Some(black_box(&scrollback)))
                .unwrap();
        });
    });

    group.finish();
}

fn bench_restore(c: &mut Criterion) {
    let mut group = c.benchmark_group("checkpoint/restore");

    // Empty grid restore
    group.bench_function("empty_grid", |b| {
        let dir = tempdir().unwrap();
        let mut manager = CheckpointManager::new(dir.path());
        let grid = Grid::new(24, 80);
        manager.save(&grid, None).unwrap();

        b.iter(|| {
            black_box(manager.restore().unwrap());
        });
    });

    // Grid 24x80 full restore
    group.bench_function("24x80_full", |b| {
        let dir = tempdir().unwrap();
        let mut manager = CheckpointManager::new(dir.path());
        let grid = create_grid_with_content(24, 80, 1.0);
        manager.save(&grid, None).unwrap();

        b.iter(|| {
            black_box(manager.restore().unwrap());
        });
    });

    // Grid with 1K lines scrollback restore
    group.bench_function("with_1k_scrollback", |b| {
        let dir = tempdir().unwrap();
        let mut manager = CheckpointManager::new(dir.path());
        let grid = Grid::new(24, 80);
        let scrollback = create_scrollback_with_lines(1000);
        manager.save(&grid, Some(&scrollback)).unwrap();

        b.iter(|| {
            black_box(manager.restore().unwrap());
        });
    });

    // Grid with 10K lines scrollback restore
    group.bench_function("with_10k_scrollback", |b| {
        let dir = tempdir().unwrap();
        let mut manager = CheckpointManager::new(dir.path());
        let grid = Grid::new(24, 80);
        let scrollback = create_scrollback_with_lines(10_000);
        manager.save(&grid, Some(&scrollback)).unwrap();

        b.iter(|| {
            black_box(manager.restore().unwrap());
        });
    });

    group.finish();
}

fn bench_compression(c: &mut Criterion) {
    let mut group = c.benchmark_group("checkpoint/compression");

    // Uncompressed save
    group.bench_function("save_uncompressed", |b| {
        let dir = tempdir().unwrap();
        let config = CheckpointConfig {
            compress: false,
            ..Default::default()
        };
        let mut manager = CheckpointManager::with_config(dir.path(), config);
        let grid = create_grid_with_content(24, 80, 1.0);

        b.iter(|| {
            manager.save(black_box(&grid), None).unwrap();
        });
    });

    // Compressed save (level 1)
    group.bench_function("save_compressed_level1", |b| {
        let dir = tempdir().unwrap();
        let config = CheckpointConfig {
            compress: true,
            compression_level: 1,
            ..Default::default()
        };
        let mut manager = CheckpointManager::with_config(dir.path(), config);
        let grid = create_grid_with_content(24, 80, 1.0);

        b.iter(|| {
            manager.save(black_box(&grid), None).unwrap();
        });
    });

    // Compressed save (level 3 - default)
    group.bench_function("save_compressed_level3", |b| {
        let dir = tempdir().unwrap();
        let config = CheckpointConfig {
            compress: true,
            compression_level: 3,
            ..Default::default()
        };
        let mut manager = CheckpointManager::with_config(dir.path(), config);
        let grid = create_grid_with_content(24, 80, 1.0);

        b.iter(|| {
            manager.save(black_box(&grid), None).unwrap();
        });
    });

    group.finish();
}

fn bench_file_size(c: &mut Criterion) {
    // This is a measurement benchmark - measure file sizes
    let mut group = c.benchmark_group("checkpoint/file_size");
    group.sample_size(10); // Fewer samples for file size tests

    for &line_count in &[100, 1000, 10_000] {
        group.throughput(Throughput::Elements(line_count as u64));
        group.bench_with_input(
            BenchmarkId::new("scrollback_lines", line_count),
            &line_count,
            |b, &count| {
                let dir = tempdir().unwrap();
                let mut manager = CheckpointManager::new(dir.path());
                let grid = Grid::new(24, 80);
                let scrollback = create_scrollback_with_lines(count);

                b.iter(|| {
                    let path = manager
                        .save(black_box(&grid), Some(black_box(&scrollback)))
                        .unwrap();
                    let size = std::fs::metadata(&path).unwrap().len();
                    black_box(size)
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_save,
    bench_restore,
    bench_compression,
    bench_file_size,
);

criterion_main!(benches);
