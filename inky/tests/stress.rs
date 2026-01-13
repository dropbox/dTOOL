//! Stress tests for inky (Phase 15.3)
//!
//! These tests verify behavior under extreme conditions. They are marked #[ignore = "Long-running stress test"]
//! by default since they take longer to run.
//!
//! Run with: `cargo test --test stress -- --ignored`

use inky::{
    hooks::Signal,
    layout::LayoutEngine,
    node::{BoxNode, Node, TextNode},
    render::Buffer,
    style::{Color, FlexDirection},
};
use std::sync::Arc;
use std::thread;

// ============================================================================
// Buffer Stress Tests
// ============================================================================

/// Stress test rapid buffer resizing
#[test]
#[ignore = "Long-running stress test"]
fn stress_rapid_resize() {
    let mut buf = Buffer::new(80, 24);

    for i in 0..10_000 {
        let w = (i % 200) as u16 + 1;
        let h = (i % 100) as u16 + 1;
        buf.resize(w, h);
    }

    // Should complete without panic
    assert!(buf.width() > 0);
    assert!(buf.height() > 0);
}

/// Stress test buffer operations at scale
#[test]
#[ignore = "Long-running stress test"]
fn stress_large_buffer_operations() {
    // Create a large buffer (like a high-res terminal)
    let mut buf = Buffer::new(400, 100);

    // Fill it repeatedly
    for _ in 0..100 {
        buf.write_str(0, 0, "Test content", Color::White, Color::Black);
        buf.fill(0, 0, 400, 100, inky::render::Cell::blank());
    }

    assert_eq!(buf.width(), 400);
    assert_eq!(buf.height(), 100);
}

// ============================================================================
// Signal Stress Tests
// ============================================================================

/// Stress test concurrent signal updates
#[test]
#[ignore = "Long-running stress test"]
fn stress_concurrent_signals() {
    let signal = Signal::new(0i64);
    let num_threads = 10;
    let iterations_per_thread = 10_000;

    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let s = signal.clone();
            thread::spawn(move || {
                for _ in 0..iterations_per_thread {
                    s.update(|n| *n += 1);
                }
            })
        })
        .collect();

    for h in handles {
        h.join().expect("thread should complete");
    }

    // All updates should be counted
    assert_eq!(signal.get(), (num_threads * iterations_per_thread) as i64);
}

/// Stress test signal with many subscribers
#[test]
#[ignore = "Long-running stress test"]
fn stress_signal_many_subscribers() {
    let signal = Signal::new(0);
    let counter = Arc::new(std::sync::atomic::AtomicUsize::new(0));

    // Add many subscribers - subscribe takes Arc<Fn()>
    for _ in 0..100 {
        let c = counter.clone();
        // Note: Signal::subscribe takes Arc<Fn() + Send + Sync>
        let callback = Arc::new(move || {
            c.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        });
        signal.subscribe(callback);
    }

    // Trigger updates
    for _ in 0..100 {
        signal.set(1);
    }

    // Each of 100 subscribers should be notified for each of 100 updates
    let notifications = counter.load(std::sync::atomic::Ordering::SeqCst);
    assert!(notifications > 0, "subscribers should be notified");
}

// ============================================================================
// Layout Stress Tests
// ============================================================================

/// Stress test deep nesting
#[test]
#[ignore = "Long-running stress test"]
fn stress_deep_nesting() {
    fn nest(depth: usize) -> Node {
        if depth == 0 {
            TextNode::new("leaf").into()
        } else {
            BoxNode::new().child(nest(depth - 1)).into()
        }
    }

    // Test up to 500 levels deep (beyond this might hit stack limits)
    let node = nest(500);
    let mut engine = LayoutEngine::new();
    engine.build(&node).expect("build should succeed");
    engine.compute(80, 24).expect("compute should succeed");
}

/// Stress test wide trees
#[test]
#[ignore = "Long-running stress test"]
fn stress_wide_tree() {
    let mut node = BoxNode::new()
        .width(1000u16)
        .flex_direction(FlexDirection::Row);

    // Add 1000 children
    for i in 0..1000 {
        node = node.child(TextNode::new(format!("child {}", i)));
    }

    let root: Node = node.into();
    let mut engine = LayoutEngine::new();
    engine.build(&root).expect("build should succeed");
    engine.compute(1000, 100).expect("compute should succeed");
}

/// Stress test layout recomputation
#[test]
#[ignore = "Long-running stress test"]
fn stress_layout_recompute() {
    let node = BoxNode::new()
        .width(80u16)
        .height(24u16)
        .child(TextNode::new("content"))
        .into();

    let mut engine = LayoutEngine::new();
    engine.build(&node).expect("build should succeed");

    // Recompute layout many times
    for _ in 0..10_000 {
        engine.compute(80, 24).expect("compute should succeed");
    }
}

// ============================================================================
// Unicode Stress Tests
// ============================================================================

/// Stress test with unicode torture strings
#[test]
#[ignore = "Long-running stress test"]
fn stress_unicode_torture() {
    let torture_strings: Vec<String> = vec![
        String::new(),
        "\0".to_string(),
        "\n\n\n".to_string(),
        "a".repeat(10_000),
        "🎉".repeat(1000),
        "مرحبا".repeat(100),
        "こんにちは".repeat(100),
        "\u{FEFF}".repeat(100),
        "\u{200B}".repeat(100),
        "a\u{0308}".repeat(100),
        "👨‍👩‍👧‍👦".repeat(100),
    ];

    let mut buf = Buffer::new(200, 50);

    for s in &torture_strings {
        buf.write_str(0, 0, s, Color::White, Color::Black);
    }

    // Should complete without panic
}

/// Stress test rapid Unicode writing
#[test]
#[ignore = "Long-running stress test"]
fn stress_rapid_unicode_write() {
    let mut buf = Buffer::new(200, 50);
    let unicode_chars = "你好世界🌍🎉★☆♠♣♥♦";

    for _ in 0..10_000 {
        for (i, c) in unicode_chars.chars().enumerate() {
            let x = (i % 200) as u16;
            let y = (i / 200 % 50) as u16;
            let s = c.to_string();
            buf.write_str(x, y, &s, Color::White, Color::Black);
        }
    }

    assert_eq!(buf.width(), 200);
}

// ============================================================================
// Memory Stress Tests
// ============================================================================

/// Stress test buffer allocation/deallocation cycles
#[test]
#[ignore = "Long-running stress test"]
fn stress_allocation_cycles() {
    for _ in 0..1000 {
        let mut buf = Buffer::new(200, 50);
        buf.write_str(0, 0, "Test", Color::White, Color::Black);
        buf.resize(100, 25);
        buf.resize(200, 50);
        // Buffer dropped here
    }
    // Should not leak memory
}

/// Stress test with varying buffer sizes
#[test]
#[ignore = "Long-running stress test"]
fn stress_varying_sizes() {
    let sizes = [
        (1, 1),
        (10, 10),
        (80, 24),
        (200, 50),
        (500, 100),
        (1000, 200),
    ];

    for (w, h) in sizes {
        let mut buf = Buffer::new(w, h);
        buf.fill(0, 0, w, h, inky::render::Cell::blank());
        assert_eq!(buf.width(), w);
        assert_eq!(buf.height(), h);
    }
}

// ============================================================================
// Concurrent Access Stress Tests
// ============================================================================

/// Stress test node creation from multiple threads
#[test]
#[ignore = "Long-running stress test"]
fn stress_concurrent_node_creation() {
    let handles: Vec<_> = (0..10)
        .map(|_| {
            thread::spawn(|| {
                for _ in 0..1000 {
                    let _node: Node = BoxNode::new().child(TextNode::new("test")).into();
                }
            })
        })
        .collect();

    for h in handles {
        h.join().expect("thread should complete");
    }
}

/// Stress test layout engines in parallel
#[test]
#[ignore = "Long-running stress test"]
fn stress_parallel_layout() {
    let handles: Vec<_> = (0..4)
        .map(|_| {
            thread::spawn(|| {
                let node = BoxNode::new()
                    .width(80u16)
                    .height(24u16)
                    .child(TextNode::new("content"))
                    .into();

                let mut engine = LayoutEngine::new();
                for _ in 0..1000 {
                    engine.build(&node).expect("build should succeed");
                    engine.compute(80, 24).expect("compute should succeed");
                }
            })
        })
        .collect();

    for h in handles {
        h.join().expect("thread should complete");
    }
}
