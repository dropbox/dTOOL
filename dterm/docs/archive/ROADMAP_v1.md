# dterm-core Roadmap

**Date:** 2024-12-28
**Philosophy:** Prove it correct, then make it fast.

---

## Verification-First Development

Every component follows this process:

```
1. SPECIFY   →  TLA+ specification (what it should do)
2. IMPLEMENT →  Rust code (how it does it)
3. PROVE     →  Kani proofs (it can't overflow/panic)
4. FUZZ      →  cargo-fuzz (it handles any input)
5. TEST      →  proptest (properties hold)
6. CONFORM   →  esctest (matches VT standards)
```

**No component ships without passing all six.**

---

## Verification Tools

| Tool | Purpose | When |
|------|---------|------|
| **TLA+** | Specify state machines | Before implementation |
| **Kani** | Prove absence of UB, panics | Before merge |
| **MIRI** | Detect UB at runtime | Every CI run |
| **cargo-fuzz** | Find edge cases | Continuous (24/7) |
| **proptest** | Property-based testing | Every CI run |
| **esctest** | VT conformance | Release gates |
| **cargo-careful** | Extra UB checks | Weekly |

---

## Phase 1: Parser (Weeks 1-4)

### 1.1 TLA+ Specification (Week 1)

**Deliverable:** `tla/Parser.tla`

Specify the DEC ANSI parser state machine formally:

```tla
--------------------------- MODULE Parser ---------------------------
EXTENDS Integers, Sequences, FiniteSets

CONSTANTS
    States,           \* {Ground, Escape, CsiEntry, CsiParam, ...}
    InputBytes,       \* 0..255
    Actions           \* {Print, Execute, CsiDispatch, ...}

VARIABLES
    state,            \* Current parser state
    params,           \* Parameter accumulator
    intermediates     \* Intermediate bytes

TypeInvariant ==
    /\ state \in States
    /\ params \in Seq(0..65535)
    /\ Len(params) <= 16
    /\ intermediates \in Seq(0..255)
    /\ Len(intermediates) <= 4

\* State transition function
Transition(s, byte) ==
    CASE s = "Ground" /\ byte \in 0x20..0x7F -> [state |-> "Ground", action |-> "Print"]
      [] s = "Ground" /\ byte = 0x1B -> [state |-> "Escape", action |-> "None"]
      [] s = "Escape" /\ byte = 0x5B -> [state |-> "CsiEntry", action |-> "None"]
      \* ... complete specification

\* Safety: Parser never gets stuck
Safety == state \in States

\* Liveness: Every input eventually produces output or state change
Liveness == <>(\E action \in Actions : action # "None")

=======================================================================
```

**Verify with TLC:**
```bash
cd tla && tlc Parser.tla -deadlock
```

### 1.2 Rust Implementation (Week 2)

**Deliverable:** `crates/dterm-core/src/parser/`

```rust
// src/parser/state.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum State {
    Ground = 0,
    Escape,
    EscapeIntermediate,
    CsiEntry,
    CsiParam,
    CsiIntermediate,
    CsiIgnore,
    DcsEntry,
    DcsParam,
    DcsIntermediate,
    DcsPassthrough,
    DcsIgnore,
    OscString,
    SosPmApcString,
}

// src/parser/table.rs
/// Compile-time generated transition table
/// 256 bytes × 14 states = 3.5KB
pub const TRANSITIONS: [[Transition; 256]; 14] = {
    let mut table = [[Transition::default(); 256]; 14];
    // ... generate at compile time
    table
};

// src/parser/mod.rs
pub struct Parser {
    state: State,
    params: ArrayVec<u16, 16>,
    intermediates: ArrayVec<u8, 4>,
    osc_data: Vec<u8>,
}

impl Parser {
    /// Process input bytes, call sink for each action
    ///
    /// # Safety
    /// This function is proven by Kani to:
    /// - Never panic
    /// - Never access out-of-bounds
    /// - Always terminate
    pub fn advance<S: ActionSink>(&mut self, input: &[u8], sink: &mut S) {
        for &byte in input {
            let transition = TRANSITIONS[self.state as usize][byte as usize];

            // Execute action
            match transition.action {
                ActionType::Print => sink.print(byte as char),
                ActionType::Execute => sink.execute(byte),
                ActionType::Param => self.add_param_digit(byte),
                ActionType::Collect => self.collect_intermediate(byte),
                ActionType::CsiDispatch => {
                    sink.csi_dispatch(&self.params, &self.intermediates, byte);
                    self.clear();
                }
                // ...
            }

            self.state = transition.next_state;
        }
    }
}
```

### 1.3 Kani Proofs (Week 3)

**Deliverable:** `crates/dterm-core/src/parser/proofs.rs`

```rust
#[cfg(kani)]
mod proofs {
    use super::*;

    /// Prove parser never panics on any input sequence
    #[kani::proof]
    #[kani::unwind(256)]  // Bound on input length for verification
    fn parser_never_panics() {
        let mut parser = Parser::new();
        let input: [u8; 256] = kani::any();
        let mut sink = NullSink;

        // This must not panic for ANY input
        parser.advance(&input, &mut sink);
    }

    /// Prove params array never overflows
    #[kani::proof]
    fn params_bounded() {
        let mut parser = Parser::new();

        // Simulate worst case: all param digits
        for _ in 0..1000 {
            let byte: u8 = kani::any();
            kani::assume(byte >= b'0' && byte <= b'9');
            parser.advance(&[byte], &mut NullSink);
        }

        // Params must never exceed 16
        kani::assert(parser.params.len() <= 16, "params overflow");
    }

    /// Prove intermediates array never overflows
    #[kani::proof]
    fn intermediates_bounded() {
        let mut parser = Parser::new();

        for _ in 0..100 {
            let byte: u8 = kani::any();
            kani::assume(byte >= 0x20 && byte <= 0x2F);  // Intermediate range
            parser.advance(&[0x1B, byte], &mut NullSink);  // ESC + intermediate
        }

        kani::assert(parser.intermediates.len() <= 4, "intermediates overflow");
    }

    /// Prove state is always valid after any transition
    #[kani::proof]
    fn state_always_valid() {
        let mut parser = Parser::new();
        let byte: u8 = kani::any();

        parser.advance(&[byte], &mut NullSink);

        // State must be a valid enum variant
        kani::assert((parser.state as u8) < 14, "invalid state");
    }

    /// Prove transition table has no invalid entries
    #[kani::proof]
    fn transition_table_valid() {
        let state: usize = kani::any();
        let byte: usize = kani::any();

        kani::assume(state < 14);
        kani::assume(byte < 256);

        let transition = TRANSITIONS[state][byte];
        kani::assert((transition.next_state as u8) < 14, "invalid next state");
    }
}
```

**Run Kani:**
```bash
cargo kani --package dterm-core --function parser_never_panics
cargo kani --package dterm-core --function params_bounded
cargo kani --package dterm-core --function intermediates_bounded
```

### 1.4 Fuzzing (Week 3-4, then continuous)

**Deliverable:** `crates/dterm-core/fuzz/`

```rust
// fuzz/fuzz_targets/parser.rs
#![no_main]
use libfuzzer_sys::fuzz_target;
use dterm_core::parser::{Parser, ActionSink};

struct CountingSink {
    actions: usize,
}

impl ActionSink for CountingSink {
    fn print(&mut self, _: char) { self.actions += 1; }
    fn execute(&mut self, _: u8) { self.actions += 1; }
    fn csi_dispatch(&mut self, _: &[u16], _: &[u8], _: u8) { self.actions += 1; }
    // ...
}

fuzz_target!(|data: &[u8]| {
    let mut parser = Parser::new();
    let mut sink = CountingSink { actions: 0 };

    // Must not panic, must not hang
    parser.advance(data, &mut sink);

    // Sanity: some input should produce some output
    // (not a hard requirement, but useful signal)
});
```

**Run fuzzer:**
```bash
cargo +nightly fuzz run parser -- -max_len=65536 -jobs=8
```

**OSS-Fuzz integration:** Submit to OSS-Fuzz for continuous 24/7 fuzzing.

### 1.5 Property Tests (Week 4)

**Deliverable:** `crates/dterm-core/src/parser/tests.rs`

```rust
use proptest::prelude::*;

proptest! {
    /// Any valid escape sequence parses without panic
    #[test]
    fn parse_any_csi(
        params in prop::collection::vec(0u16..1000, 0..16),
        intermediate in prop::collection::vec(0x20u8..0x30, 0..4),
        final_byte in 0x40u8..0x7F,
    ) {
        let mut parser = Parser::new();
        let mut sink = VecSink::new();

        // Build CSI sequence: ESC [ params ; ... intermediate final
        let mut seq = vec![0x1B, b'['];
        for (i, p) in params.iter().enumerate() {
            if i > 0 { seq.push(b';'); }
            seq.extend(p.to_string().bytes());
        }
        seq.extend(&intermediate);
        seq.push(final_byte);

        parser.advance(&seq, &mut sink);

        // Should produce exactly one CsiDispatch
        prop_assert!(sink.actions.iter().any(|a| matches!(a, Action::CsiDispatch { .. })));
    }

    /// Parser state is consistent after any input
    #[test]
    fn state_consistent(input in prop::collection::vec(any::<u8>(), 0..1000)) {
        let mut parser = Parser::new();
        let mut sink = NullSink;

        parser.advance(&input, &mut sink);

        // State must be valid
        prop_assert!((parser.state as u8) < 14);
        // Params must be bounded
        prop_assert!(parser.params.len() <= 16);
        // Intermediates must be bounded
        prop_assert!(parser.intermediates.len() <= 4);
    }

    /// Reset returns to ground state
    #[test]
    fn reset_clears_state(input in prop::collection::vec(any::<u8>(), 0..100)) {
        let mut parser = Parser::new();
        let mut sink = NullSink;

        parser.advance(&input, &mut sink);
        parser.reset();

        prop_assert_eq!(parser.state, State::Ground);
        prop_assert!(parser.params.is_empty());
        prop_assert!(parser.intermediates.is_empty());
    }
}
```

### 1.6 VT Conformance (Week 4)

**Deliverable:** Passing esctest suite

```bash
# Run esctest (from Invisible Island's vttest)
esctest --expected-terminal=xterm --max-vt-level=5 \
    --output-format=json \
    --test-runner="./target/release/dterm-esctest-runner"
```

**Conformance targets:**
- VT100: 100%
- VT220: 100%
- VT420: 95%+
- xterm extensions: 90%+

---

## Phase 2: Grid (Weeks 5-8)

### 2.1 TLA+ Specification (Week 5)

**Deliverable:** `tla/Grid.tla`

```tla
--------------------------- MODULE Grid ---------------------------
EXTENDS Integers, Sequences

CONSTANTS
    MaxRows,          \* e.g., 1000
    MaxCols,          \* e.g., 500
    PageSize          \* e.g., 65536 bytes

VARIABLES
    pages,            \* Sequence of pages
    rows,             \* Current row count
    cols,             \* Current column count
    cursor            \* {row, col}

TypeInvariant ==
    /\ rows \in 1..MaxRows
    /\ cols \in 1..MaxCols
    /\ cursor.row \in 0..rows-1
    /\ cursor.col \in 0..cols-1
    /\ \A p \in DOMAIN pages : Len(pages[p]) = PageSize

\* Cursor never goes out of bounds
CursorInBounds ==
    /\ cursor.row < rows
    /\ cursor.col < cols

\* Resize maintains invariants
Resize(newRows, newCols) ==
    /\ rows' = newRows
    /\ cols' = newCols
    /\ cursor' = [
        row |-> IF cursor.row >= newRows THEN newRows - 1 ELSE cursor.row,
        col |-> IF cursor.col >= newCols THEN newCols - 1 ELSE cursor.col
       ]
    /\ CursorInBounds'

=======================================================================
```

### 2.2 Kani Proofs for Grid (Week 6)

```rust
#[cfg(kani)]
mod grid_proofs {
    use super::*;

    /// Prove cell access never panics
    #[kani::proof]
    fn cell_access_safe() {
        let rows: u16 = kani::any();
        let cols: u16 = kani::any();
        kani::assume(rows > 0 && rows <= 1000);
        kani::assume(cols > 0 && cols <= 500);

        let grid = Grid::new(rows, cols);

        let row: u16 = kani::any();
        let col: u16 = kani::any();
        kani::assume(row < rows);
        kani::assume(col < cols);

        // Must not panic
        let _ = grid.cell(row, col);
    }

    /// Prove offset calculation is correct
    #[kani::proof]
    fn offset_calculation_correct() {
        let page = Page::new();

        let offset: u32 = kani::any();
        kani::assume(offset < PAGE_SIZE as u32);
        kani::assume(offset % std::mem::size_of::<Cell>() as u32 == 0);

        let cell_ref = CellRef { offset };

        // Access must be within bounds
        let ptr = page.data.as_ptr();
        let cell_ptr = unsafe { ptr.add(offset as usize) as *const Cell };
        kani::assert(cell_ptr < unsafe { ptr.add(PAGE_SIZE) } as *const Cell, "out of bounds");
    }

    /// Prove resize preserves cursor invariant
    #[kani::proof]
    fn resize_cursor_valid() {
        let mut grid = Grid::new(100, 80);

        // Move cursor somewhere
        let row: u16 = kani::any();
        let col: u16 = kani::any();
        kani::assume(row < 100);
        kani::assume(col < 80);
        grid.set_cursor(row, col);

        // Resize to smaller
        let new_rows: u16 = kani::any();
        let new_cols: u16 = kani::any();
        kani::assume(new_rows > 0 && new_rows <= 100);
        kani::assume(new_cols > 0 && new_cols <= 80);

        grid.resize(new_rows, new_cols);

        // Cursor must still be valid
        kani::assert(grid.cursor.row < new_rows, "cursor row out of bounds");
        kani::assert(grid.cursor.col < new_cols, "cursor col out of bounds");
    }

    /// Prove style deduplication doesn't overflow
    #[kani::proof]
    #[kani::unwind(1000)]
    fn style_table_bounded() {
        let mut styles = StyleTable::new();

        for _ in 0..1000 {
            let style = Style {
                fg: kani::any(),
                bg: kani::any(),
                attrs: kani::any(),
            };
            let id = styles.intern(style);
            kani::assert(id.0 < styles.len() as u16, "invalid style id");
        }
    }
}
```

### 2.3 Memory Safety for Unsafe Code (Week 7)

All unsafe code requires:

1. **Safety comment** explaining why it's sound
2. **Kani proof** showing no UB
3. **MIRI passing** under CI

```rust
impl Page {
    /// Get cell at offset
    ///
    /// # Safety
    /// - `offset` must be < PAGE_SIZE
    /// - `offset` must be aligned to Cell
    /// - The memory at offset must be initialized
    ///
    /// Proven by: `kani::cell_access_safe`
    #[inline]
    pub unsafe fn cell_unchecked(&self, offset: u32) -> &Cell {
        debug_assert!(offset < PAGE_SIZE as u32);
        debug_assert!(offset as usize % std::mem::align_of::<Cell>() == 0);

        &*(self.data.as_ptr().add(offset as usize) as *const Cell)
    }
}
```

---

## Phase 3: Scrollback (Weeks 9-12)

### 3.1 TLA+ for Tier Transitions

**Deliverable:** `tla/Scrollback.tla`

```tla
--------------------------- MODULE Scrollback ---------------------------
EXTENDS Integers, Sequences

CONSTANTS
    HotLimit,         \* e.g., 1000 lines
    WarmLimit,        \* e.g., 10000 lines
    MemoryBudget      \* e.g., 100MB

VARIABLES
    hot,              \* Lines in hot tier
    warm,             \* Compressed blocks in warm tier
    cold,             \* Pages on disk
    memoryUsed        \* Current memory usage

TypeInvariant ==
    /\ Len(hot) <= HotLimit
    /\ memoryUsed <= MemoryBudget

\* Promote from hot to warm when hot is full
PromoteHotToWarm ==
    /\ Len(hot) = HotLimit
    /\ warm' = Append(warm, Compress(SubSeq(hot, 1, HotLimit \div 2)))
    /\ hot' = SubSeq(hot, HotLimit \div 2 + 1, HotLimit)
    /\ UNCHANGED cold

\* Evict from warm to cold when over budget
EvictWarmToCold ==
    /\ memoryUsed > MemoryBudget
    /\ Len(warm) > 0
    /\ cold' = Append(cold, warm[1])
    /\ warm' = Tail(warm)
    /\ memoryUsed' = memoryUsed - Size(warm[1])

\* Memory budget is always respected
MemoryBudgetInvariant ==
    memoryUsed <= MemoryBudget + HotLimit * LineSize

=======================================================================
```

### 3.2 Kani Proofs for Compression

```rust
#[cfg(kani)]
mod scrollback_proofs {
    /// Prove compression is reversible
    #[kani::proof]
    fn compression_roundtrip() {
        let data: [u8; 1024] = kani::any();

        let compressed = lz4_flex::compress_prepend_size(&data);
        let decompressed = lz4_flex::decompress_size_prepended(&compressed).unwrap();

        kani::assert(data == decompressed.as_slice(), "compression not reversible");
    }

    /// Prove tier transition preserves line count
    #[kani::proof]
    fn tier_transition_preserves_lines() {
        let mut scrollback = Scrollback::new(1000, 10000, 100_000_000);

        let initial_count: usize = kani::any();
        kani::assume(initial_count <= 100);

        for i in 0..initial_count {
            scrollback.push_line(Line::new(&format!("line {}", i)));
        }

        kani::assert(scrollback.line_count() == initial_count, "line count mismatch");
    }

    /// Prove memory budget is respected
    #[kani::proof]
    fn memory_budget_enforced() {
        let budget: usize = kani::any();
        kani::assume(budget >= 1_000_000 && budget <= 1_000_000_000);

        let mut scrollback = Scrollback::new(1000, 10000, budget);

        // Push many lines
        for _ in 0..10000 {
            scrollback.push_line(Line::new("x".repeat(100)));
        }

        kani::assert(scrollback.memory_used() <= budget + EPSILON, "budget exceeded");
    }
}
```

---

## Phase 4: Search (Weeks 13-14)

### 4.1 Search Correctness Properties

```rust
#[cfg(kani)]
mod search_proofs {
    /// Prove no false negatives (if line contains query, it's found)
    #[kani::proof]
    fn no_false_negatives() {
        let mut index = SearchIndex::new();

        let line: [u8; 32] = kani::any();
        let line_str = std::str::from_utf8(&line).unwrap_or("");

        index.index_line(0, line_str);

        // If we search for a substring that exists, we must find it
        if line_str.len() >= 3 {
            let query = &line_str[0..3];
            let results: Vec<_> = index.search(query).collect();
            kani::assert(results.contains(&0), "false negative");
        }
    }
}

proptest! {
    /// Search results are always valid line numbers
    #[test]
    fn search_results_valid(
        lines in prop::collection::vec("[a-z]{10,50}", 100..1000),
        query in "[a-z]{3,10}",
    ) {
        let mut index = SearchIndex::new();

        for (i, line) in lines.iter().enumerate() {
            index.index_line(i, line);
        }

        for line_num in index.search(&query) {
            prop_assert!(line_num < lines.len());
            // Verify it's actually a match
            prop_assert!(lines[line_num].contains(&query));
        }
    }
}
```

---

## Phase 5: Checkpoints (Weeks 15-16)

### 5.1 Checkpoint Properties

```rust
proptest! {
    /// Checkpoint + restore produces identical state
    #[test]
    fn checkpoint_restore_identical(
        lines in prop::collection::vec("[a-z0-9 ]{10,100}", 100..500),
        cursor_row in 0usize..100,
        cursor_col in 0usize..80,
    ) {
        let mut grid = Grid::new(100, 80);
        let mut scrollback = Scrollback::new(1000, 10000, 100_000_000);

        // Populate
        for line in &lines {
            scrollback.push_line(Line::new(line));
        }
        grid.set_cursor(cursor_row.min(99) as u16, cursor_col.min(79) as u16);

        // Checkpoint
        let temp_dir = tempfile::tempdir().unwrap();
        checkpoint_save(&grid, &scrollback, temp_dir.path()).unwrap();

        // Restore
        let (restored_grid, restored_scrollback) = checkpoint_restore(temp_dir.path()).unwrap();

        // Verify identical
        prop_assert_eq!(grid.cursor, restored_grid.cursor);
        prop_assert_eq!(scrollback.line_count(), restored_scrollback.line_count());

        for i in 0..scrollback.line_count() {
            prop_assert_eq!(
                scrollback.get_line(i).text(),
                restored_scrollback.get_line(i).text()
            );
        }
    }
}
```

---

## CI Pipeline

```yaml
# .github/workflows/verify.yml
name: Verification

on: [push, pull_request]

jobs:
  # Standard tests
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo test --all-features

  # Kani proofs
  kani:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: model-checking/kani-github-action@v1
      - run: cargo kani --all-features

  # MIRI for UB detection
  miri:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@nightly
        with:
          components: miri
      - run: cargo +nightly miri test

  # Property tests
  proptest:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo test --release -- --include-ignored proptest

  # Fuzzing (short run in CI, long run in OSS-Fuzz)
  fuzz:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@nightly
      - run: cargo install cargo-fuzz
      - run: cargo +nightly fuzz run parser -- -max_total_time=60

  # TLA+ model checking
  tla:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-java@v4
        with:
          java-version: '17'
          distribution: 'temurin'
      - run: |
          wget https://github.com/tlaplus/tlaplus/releases/download/v1.8.0/TLAToolbox-1.8.0-linux.gtk.x86_64.zip
          unzip TLAToolbox-*.zip
      - run: |
          cd tla
          java -jar ../toolbox/tla2tools.jar -deadlock Parser.tla
          java -jar ../toolbox/tla2tools.jar -deadlock Grid.tla
          java -jar ../toolbox/tla2tools.jar -deadlock Scrollback.tla

  # VT conformance
  conformance:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo build --release
      - run: |
          pip install esctest
          esctest --test-runner=./target/release/dterm-esctest-runner --output-format=json > results.json
          python scripts/check_conformance.py results.json
```

---

## Milestone Gates

### M1: Parser (Week 4)
- [ ] TLA+ spec verified with TLC
- [ ] Kani proofs pass for all safety properties
- [ ] MIRI clean
- [ ] 1 billion iterations of fuzzing without crash
- [ ] proptest passes 10,000 cases
- [ ] VT100 conformance 100%
- [ ] Throughput > 400 MB/s

### M2: Grid (Week 8)
- [ ] TLA+ spec verified
- [ ] Kani proofs for all unsafe code
- [ ] Cell size = 8 bytes (verified)
- [ ] Offset calculations proven correct
- [ ] Resize proven to maintain invariants

### M3: Scrollback (Week 12)
- [ ] TLA+ spec for tier transitions
- [ ] Memory budget invariant proven
- [ ] Compression roundtrip proven
- [ ] 1M lines in < 100MB (measured)

### M4: Search + Checkpoints (Week 16)
- [ ] No false negatives proven
- [ ] Checkpoint/restore identity proven
- [ ] Search < 10ms for 1M lines (measured)
- [ ] Restore < 1s (measured)

### M5: Integration (Week 18)
- [ ] FFI bindings complete
- [ ] dashterm2 integration working
- [ ] Benchmark: 5x faster than iTerm2 baseline
- [ ] 30 days of fuzzing without crash

---

## Summary

**The goal is not to be probably correct. The goal is to be provably correct.**

Every line of dterm-core exists because:
1. It was specified in TLA+
2. It was proven safe with Kani
3. It was fuzzed for billions of iterations
4. It was tested with property-based tests
5. It conforms to VT standards

This is how we build a terminal that never crashes, never loses data, and can be trusted with AI agent workflows.
