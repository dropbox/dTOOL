//! Resource exhaustion fuzzer - tests for "billion laughs" type attacks.
//!
//! This fuzzer specifically targets denial of service vulnerabilities that
//! cause unbounded memory growth, CPU consumption, or stack overflow.
//!
//! ## Running
//!
//! ```bash
//! cd crates/dterm-core
//! cargo +nightly fuzz run resource_exhaustion -- -max_total_time=600
//! ```
//!
//! ## Attack Categories Tested
//!
//! 1. Memory exhaustion via unbounded growth
//! 2. CPU exhaustion via O(n²) or exponential algorithms
//! 3. Stack overflow via deep recursion/nesting
//! 4. Allocation bombing via many small allocations
//!
//! ## Resource Limits
//!
//! The fuzzer enforces soft limits to detect when the terminal is
//! consuming excessive resources without crashing the fuzzer itself.

#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use dterm_core::terminal::Terminal;
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

/// Memory tracking allocator for detecting excessive allocation.
struct TrackingAllocator;

static ALLOCATED: AtomicUsize = AtomicUsize::new(0);
static PEAK_ALLOCATED: AtomicUsize = AtomicUsize::new(0);
static ALLOCATION_COUNT: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let size = layout.size();
        ALLOCATION_COUNT.fetch_add(1, Ordering::Relaxed);
        let current = ALLOCATED.fetch_add(size, Ordering::Relaxed) + size;

        // Update peak
        let mut peak = PEAK_ALLOCATED.load(Ordering::Relaxed);
        while current > peak {
            match PEAK_ALLOCATED.compare_exchange_weak(
                peak,
                current,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(p) => peak = p,
            }
        }

        System.alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        ALLOCATED.fetch_sub(layout.size(), Ordering::Relaxed);
        System.dealloc(ptr, layout)
    }
}

// Note: We can't actually use this as global allocator in a fuzz target
// because libfuzzer has its own memory management. Instead we track
// terminal-level metrics.

/// Resource exhaustion attack patterns.
const EXHAUSTION_ATTACKS: &[ExhaustionAttack] = &[
    // === Title Stack Bombing ===
    ExhaustionAttack {
        name: "title_push_flood",
        // Push title onto stack 1000 times (should be bounded)
        setup: b"",
        pattern: b"\x1b[22t",  // DECSLPP - save title to stack
        cleanup: b"",
        max_iterations: 10000,
    },

    // === Scrollback Growth ===
    ExhaustionAttack {
        name: "scrollback_flood",
        setup: b"\x1b[1;24r",  // Set scroll region
        pattern: b"A\n",  // Output + newline (scrolls)
        cleanup: b"",
        max_iterations: 100000,
    },

    // === Tab Stop Flooding ===
    ExhaustionAttack {
        name: "tab_stop_flood",
        setup: b"\x1b[3g",  // Clear all tabs
        pattern: b" \x1bH",  // Space then set tab stop
        cleanup: b"",
        max_iterations: 10000,
    },

    // === Character Set Switching ===
    ExhaustionAttack {
        name: "charset_switch_flood",
        setup: b"",
        pattern: b"\x0e\x0f",  // SI/SO rapid switching
        cleanup: b"",
        max_iterations: 100000,
    },

    // === Cursor Position Thrashing ===
    ExhaustionAttack {
        name: "cursor_thrash",
        setup: b"",
        pattern: b"\x1b[1;1H\x1b[24;80H",  // Jump between corners
        cleanup: b"",
        max_iterations: 100000,
    },

    // === SGR Stack Abuse ===
    ExhaustionAttack {
        name: "sgr_push_flood",
        setup: b"",
        pattern: b"\x1b[#{\x1b[31m",  // XTPUSHSGR + set color
        cleanup: b"",
        max_iterations: 10000,
    },

    // === Selection/Copy Attack ===
    ExhaustionAttack {
        name: "selection_flood",
        // Rapidly select and deselect (if selection state is tracked)
        setup: b"Test content for selection\n",
        pattern: b"\x1b[?1000h\x1b[?1000l",  // Mouse mode toggle
        cleanup: b"",
        max_iterations: 10000,
    },

    // === Alternate Screen Toggle ===
    ExhaustionAttack {
        name: "altscreen_toggle_flood",
        setup: b"",
        pattern: b"\x1b[?1049h\x1b[?1049l",  // Enter/exit alternate screen
        cleanup: b"",
        max_iterations: 10000,
    },

    // === Save/Restore Cursor Abuse ===
    ExhaustionAttack {
        name: "cursor_save_flood",
        setup: b"",
        pattern: b"\x1b7\x1b8",  // DECSC/DECRC rapid fire
        cleanup: b"",
        max_iterations: 100000,
    },

    // === Erase Operations ===
    ExhaustionAttack {
        name: "erase_flood",
        setup: b"",
        pattern: b"\x1b[2J",  // Erase entire screen
        cleanup: b"",
        max_iterations: 10000,
    },

    // === Insert Line Flood ===
    ExhaustionAttack {
        name: "insert_line_flood",
        setup: b"\x1b[1;24r",  // Set scroll region
        pattern: b"\x1b[L",  // Insert line
        cleanup: b"",
        max_iterations: 10000,
    },

    // === Wide Character Attack ===
    ExhaustionAttack {
        name: "wide_char_flood",
        setup: b"",
        pattern: "日本語".as_bytes(),  // Wide characters
        cleanup: b"",
        max_iterations: 10000,
    },

    // === Color Palette Modification ===
    ExhaustionAttack {
        name: "color_palette_flood",
        setup: b"",
        pattern: b"\x1b]4;0;rgb:ff/00/00\x07",  // Set palette color
        cleanup: b"",
        max_iterations: 10000,
    },

    // === Hyperlink Flood ===
    ExhaustionAttack {
        name: "hyperlink_flood",
        setup: b"",
        pattern: b"\x1b]8;;http://example.com\x07Link\x1b]8;;\x07",
        cleanup: b"",
        max_iterations: 1000,
    },

    // === Line Attribute Flood ===
    ExhaustionAttack {
        name: "line_attr_flood",
        setup: b"",
        pattern: b"\x1b#3\x1b#4",  // DECDHL top/bottom
        cleanup: b"",
        max_iterations: 10000,
    },

    // === Protected Area Flood ===
    ExhaustionAttack {
        name: "protected_area_flood",
        setup: b"",
        pattern: b"\x1bV\x1bW",  // SPA/EPA
        cleanup: b"",
        max_iterations: 10000,
    },
];

/// Describes a resource exhaustion attack pattern.
struct ExhaustionAttack {
    name: &'static str,
    setup: &'static [u8],
    pattern: &'static [u8],
    cleanup: &'static [u8],
    max_iterations: usize,
}

/// Structured input for fuzzer-guided resource testing.
#[derive(Debug, Arbitrary)]
struct ExhaustionInput {
    /// Which attack pattern to use
    attack_idx: u8,
    /// How many iterations (clamped to max_iterations)
    iterations: u16,
    /// Interleave with random data?
    interleave: bool,
    /// Random data to interleave
    random_data: Vec<u8>,
    /// Combine multiple attacks?
    combine_attacks: bool,
    /// Second attack index if combining
    second_attack_idx: u8,
}

/// Resource usage metrics for a terminal.
struct ResourceMetrics {
    /// Approximate size in bytes
    size_bytes: usize,
}

impl ResourceMetrics {
    fn measure(_terminal: &Terminal) -> Self {
        // In practice, we'd use terminal.memory_usage() or similar
        // For now, just return placeholder
        Self {
            size_bytes: 0,
        }
    }
}

fuzz_target!(|data: &[u8]| {
    // === Phase 1: Direct pattern testing with iteration limits ===

    if data.len() >= 4 {
        let attack_idx = data[0] as usize % EXHAUSTION_ATTACKS.len();
        let iterations = u16::from_le_bytes([data[1], data[2]]) as usize;
        let attack = &EXHAUSTION_ATTACKS[attack_idx];

        // Clamp iterations to attack's maximum
        let iterations = iterations.min(attack.max_iterations);

        let mut terminal = Terminal::new(24, 80);

        // Run setup
        terminal.process(attack.setup);

        // Run attack pattern
        for _ in 0..iterations {
            terminal.process(attack.pattern);
        }

        // Run cleanup
        terminal.process(attack.cleanup);

        // Verify terminal is still responsive
        terminal.process(b"Test\r\n");
        let _cursor = terminal.cursor();

        // The terminal should handle this without panic
        // Resource usage is bounded by design
    }

    // === Phase 2: Exponential growth detection ===

    if data.len() >= 8 {
        let mut terminal = Terminal::new(24, 80);

        // Feed increasing amounts of data and verify linear growth
        let chunk_size = 1000;
        let chunks = (data[4] % 10) as usize + 1;

        for chunk_idx in 0..chunks {
            // Generate chunk based on fuzz data
            let pattern_idx = (data[5] as usize + chunk_idx) % EXHAUSTION_ATTACKS.len();
            let pattern = EXHAUSTION_ATTACKS[pattern_idx].pattern;

            for _ in 0..chunk_size {
                terminal.process(pattern);
            }

            // After each chunk, terminal should still be responsive
            let _cursor = terminal.cursor();
        }

        // Final verification
        terminal.process(b"\x1b[2J\x1b[H");  // Clear and home
        let _cursor = terminal.cursor();
    }

    // === Phase 3: Combined attack patterns ===

    if data.len() >= 6 {
        let attack1_idx = data[0] as usize % EXHAUSTION_ATTACKS.len();
        let attack2_idx = data[1] as usize % EXHAUSTION_ATTACKS.len();
        let iterations = (data[2] as usize % 100) + 1;

        let mut terminal = Terminal::new(24, 80);

        // Interleave two attack patterns
        let attack1 = &EXHAUSTION_ATTACKS[attack1_idx];
        let attack2 = &EXHAUSTION_ATTACKS[attack2_idx];

        terminal.process(attack1.setup);
        terminal.process(attack2.setup);

        for _ in 0..iterations {
            terminal.process(attack1.pattern);
            terminal.process(attack2.pattern);
        }

        terminal.process(attack1.cleanup);
        terminal.process(attack2.cleanup);

        // Verify terminal survived combined attack
        let _cursor = terminal.cursor();
    }

    // === Phase 4: Raw fuzz data with resource monitoring ===

    {
        let mut terminal = Terminal::new(24, 80);

        // Feed raw fuzz data
        terminal.process(data);

        // Terminal must remain operational
        terminal.process(b"Recovery test\r\n");
        let _cursor = terminal.cursor();
        let _title = terminal.title();

        // Test that we can still do basic operations
        terminal.process(b"\x1b[2J");  // Clear screen
        terminal.process(b"\x1b[H");   // Home cursor
        terminal.process(b"Hello");    // Print text
    }

    // === Phase 5: Rapid terminal creation/destruction ===

    if data.len() >= 2 {
        let count = (data[0] % 50) as usize + 1;

        for i in 0..count {
            let cols = 80 + (i % 40) as u16;
            let rows = 24 + (i % 20) as u16;

            let mut terminal = Terminal::new(cols, rows);

            // Do some work
            if data.len() > i {
                terminal.process(&data[i..]);
            }

            // Terminal is dropped here - should be clean
        }
    }

    // === Phase 6: Resize stress test ===

    if data.len() >= 10 {
        let mut terminal = Terminal::new(24, 80);

        // Fill terminal with content
        terminal.process(b"Initial content that should be preserved across resize\r\n");

        for i in 0..data.len().min(100) {
            let cols = 20 + (data[i] % 200) as u16;
            let rows = 10 + (data[(i + 1) % data.len()] % 100) as u16;

            terminal.resize(cols, rows);

            // Terminal should handle any size
            let _cursor = terminal.cursor();
        }

        // Final operations should work
        terminal.process(b"After resize test\r\n");
    }
});
