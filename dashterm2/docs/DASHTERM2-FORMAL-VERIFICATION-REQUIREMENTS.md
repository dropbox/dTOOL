# DashTerm2 Formal Verification Requirements

**Source Repo:** https://github.com/dropbox/dTOOL/dashterm2
**Target Repo:** https://github.com/dropbox/dashprove
**Date:** December 2025
**Status:** Phase 0 Hardening Complete (0 SwiftLint errors)

---

## Executive Summary

DashTerm2 is a high-performance terminal emulator forked from DashTerm2. After completing Phase 0 hardening (NASA/NSA-grade static analysis), we are ready for Phase 4: Rust Core Migration with Formal Verification.

This document specifies the formal verification requirements for components to be rewritten in Rust.

---

## Components Requiring Formal Verification

### 1. VT100 Parser (HIGHEST PRIORITY)

**Current Implementation:**
- `sources/VT100Terminal.m` (227KB) - Escape sequence parsing
- `sources/VT100ScreenMutableState.m` (293KB) - Screen buffer management
- `sources/VT100Grid.m` (121KB) - Character grid operations
- `sources/VT100CSIParser.m` (32KB) - Control sequence parsing

**Formal Verification Requirements:**

| Property | Description | Verification Method |
|----------|-------------|---------------------|
| State machine correctness | Parser transitions only to valid states | TLA+ model checking |
| Input completeness | All valid VT100/ANSI sequences handled | Exhaustive testing + Kani |
| No infinite loops | Parsing always terminates | Termination proof |
| Buffer bounds | No buffer overflows possible | Prusti/Creusot |
| Memory safety | No use-after-free, double-free | Miri + ASAN |

**Proposed TLA+ Specification:**

```tla+
--------------------------- MODULE VT100Parser ---------------------------
EXTENDS Integers, Sequences

CONSTANTS
    GROUND, ESCAPE, CSI, OSC, DCS, SOS, PM, APC,  \* Parser states
    MAX_PARAM_COUNT, MAX_BUFFER_SIZE

VARIABLES
    state,          \* Current parser state
    params,         \* Collected numeric parameters
    intermediate,   \* Intermediate characters
    buffer,         \* Output buffer
    cursor_x,       \* Cursor X position (0-indexed)
    cursor_y        \* Cursor Y position (0-indexed)

vars == <<state, params, intermediate, buffer, cursor_x, cursor_y>>

TypeInvariant ==
    /\ state \in {GROUND, ESCAPE, CSI, OSC, DCS, SOS, PM, APC}
    /\ Len(params) <= MAX_PARAM_COUNT
    /\ Len(buffer) <= MAX_BUFFER_SIZE
    /\ cursor_x >= 0 /\ cursor_x < 80
    /\ cursor_y >= 0 /\ cursor_y < 24

Init ==
    /\ state = GROUND
    /\ params = <<>>
    /\ intermediate = <<>>
    /\ buffer = <<>>
    /\ cursor_x = 0
    /\ cursor_y = 0

\* Parser never gets stuck
LivenessProperty == <>[]( state = GROUND )

\* Buffer never overflows
SafetyProperty == []( Len(buffer) <= MAX_BUFFER_SIZE )

\* Cursor always in bounds
CursorInBounds == []( cursor_x >= 0 /\ cursor_x < 80 /\ cursor_y >= 0 /\ cursor_y < 24 )
=========================================================================
```

**Kani Proof Harness (Rust):**

```rust
#[cfg(kani)]
mod verification {
    use super::*;

    #[kani::proof]
    #[kani::unwind(256)]
    fn cursor_bounds_preserved() {
        let width: u16 = kani::any();
        let height: u16 = kani::any();
        kani::assume(width > 0 && width <= 1000);
        kani::assume(height > 0 && height <= 1000);

        let mut parser = VT100Parser::new(width, height);
        let input: u8 = kani::any();

        parser.process_byte(input);

        kani::assert(
            parser.cursor_x < width && parser.cursor_y < height,
            "Cursor must remain in bounds"
        );
    }

    #[kani::proof]
    #[kani::unwind(1024)]
    fn no_buffer_overflow() {
        let mut parser = VT100Parser::new(80, 24);

        for _ in 0..256 {
            let input: u8 = kani::any();
            parser.process_byte(input);
        }

        kani::assert!(
            parser.buffer.len() <= MAX_BUFFER_SIZE,
            "Buffer must not overflow"
        );
    }

    #[kani::proof]
    fn state_machine_valid_transitions() {
        let mut parser = VT100Parser::new(80, 24);
        let input: u8 = kani::any();

        let old_state = parser.state;
        parser.process_byte(input);
        let new_state = parser.state;

        // Verify transition is valid according to spec
        kani::assert!(
            is_valid_transition(old_state, input, new_state),
            "Invalid state transition"
        );
    }
}
```

---

### 2. Buffer Management

**Current Implementation:**
- Ring buffer for scrollback history
- Screen buffer with attributes per cell
- Selection management

**Formal Verification Requirements:**

| Property | Description | Verification Method |
|----------|-------------|---------------------|
| Memory bounds | All accesses within allocated bounds | Prusti |
| No data races | Concurrent access is safe | ThreadSanitizer + RacerD |
| Ownership correctness | Rust borrow rules satisfied | Compile-time |
| Capacity invariants | Buffer size limits respected | Kani |

**Prusti Contract Example:**

```rust
#[requires(capacity > 0)]
#[ensures(result.len() == 0)]
#[ensures(result.capacity() == capacity)]
pub fn new_buffer(capacity: usize) -> ScreenBuffer {
    ScreenBuffer {
        cells: Vec::with_capacity(capacity),
        capacity,
    }
}

#[requires(self.cells.len() < self.capacity)]
#[ensures(self.cells.len() == old(self.cells.len()) + 1)]
pub fn push_cell(&mut self, cell: Cell) {
    self.cells.push(cell);
}

#[requires(x < self.width && y < self.height)]
#[ensures(result.is_some() ==> result.unwrap().x == x && result.unwrap().y == y)]
pub fn get_cell(&self, x: usize, y: usize) -> Option<&Cell> {
    self.cells.get(y * self.width + x)
}
```

---

### 3. Cursor Logic

**Formal Verification Requirements:**

| Property | Description | Verification Method |
|----------|-------------|---------------------|
| Bounds preservation | Cursor never exceeds screen dimensions | Kani |
| Wrap correctness | Line wrapping works correctly | Property-based testing |
| Scroll correctness | Scrolling preserves invariants | TLA+ |
| Origin mode | Origin mode calculations correct | Unit tests + Kani |

**Kani Proof:**

```rust
#[kani::proof]
fn cursor_movement_bounds() {
    let width: u16 = kani::any();
    let height: u16 = kani::any();
    kani::assume(width > 0 && width <= 1000);
    kani::assume(height > 0 && height <= 1000);

    let mut cursor = Cursor::new(width, height);

    // Test all movement operations
    let movement: u8 = kani::any_where(|&m| m < 8);
    let amount: u16 = kani::any();

    match movement {
        0 => cursor.move_up(amount),
        1 => cursor.move_down(amount),
        2 => cursor.move_left(amount),
        3 => cursor.move_right(amount),
        4 => cursor.move_to(kani::any(), kani::any()),
        5 => cursor.carriage_return(),
        6 => cursor.line_feed(),
        _ => cursor.tab(),
    }

    kani::assert!(cursor.x < width, "X must be in bounds");
    kani::assert!(cursor.y < height, "Y must be in bounds");
}
```

---

### 4. Text Shaping (Unicode)

**Formal Verification Requirements:**

| Property | Description | Verification Method |
|----------|-------------|---------------------|
| UTF-8 validity | All output is valid UTF-8 | Compile-time (Rust) |
| Grapheme clustering | Combining chars handled correctly | ICU test suite |
| Width calculation | Display width correct for all chars | Property-based testing |
| Bidi correctness | Bidirectional text renders correctly | UAX #9 compliance tests |

---

### 5. SSH Layer (Security Critical)

**Formal Verification Requirements:**

| Property | Description | Verification Method |
|----------|-------------|---------------------|
| Protocol correctness | SSH protocol state machine correct | TLA+ |
| No plaintext leaks | Secrets never written to logs/memory | Taint analysis |
| Constant-time crypto | No timing side channels | CT-Wasm verification |
| Key handling | Private keys properly zeroed | Memory audit + Prusti |

**Recommended Approach:** Use formally verified crypto library (e.g., libsodium, ring).

---

## Verification Tools Required

### Tier 1: Model Checking (State Machines)

```bash
# TLA+ Toolbox
brew install tla-plus-toolbox

# Or command-line TLC
brew install adoptopenjdk
wget https://github.com/tlaplus/tlaplus/releases/download/v1.8.0/tla2tools.jar
```

### Tier 2: Rust Verification

```bash
# Kani - Model checker for Rust
cargo install --locked kani-verifier
kani setup

# Prusti - Verification framework
rustup component add rust-src --toolchain nightly
cargo install prusti-driver

# MIRI - Undefined behavior detector
rustup +nightly component add miri
```

### Tier 3: Memory Safety

```bash
# Run Miri on tests
cargo +nightly miri test

# Address Sanitizer
RUSTFLAGS="-Z sanitizer=address" cargo +nightly test
```

### Tier 4: Property-Based Testing

```rust
// Cargo.toml
[dev-dependencies]
proptest = "1.4"
quickcheck = "1.0"

// Example property test
proptest! {
    #[test]
    fn cursor_always_in_bounds(
        width in 1u16..1000,
        height in 1u16..1000,
        ops in prop::collection::vec(any::<CursorOp>(), 0..100)
    ) {
        let mut cursor = Cursor::new(width, height);
        for op in ops {
            cursor.apply(op);
            prop_assert!(cursor.x < width);
            prop_assert!(cursor.y < height);
        }
    }
}
```

---

## Verification Milestones

| Milestone | Components | Target |
|-----------|------------|--------|
| M1 | Cursor logic with Kani proofs | Phase 4.1 |
| M2 | Buffer management with Prusti contracts | Phase 4.2 |
| M3 | VT100 parser TLA+ specification | Phase 4.3 |
| M4 | VT100 parser Kani verification | Phase 4.4 |
| M5 | Full integration with property tests | Phase 4.5 |

---

## Success Criteria

1. **TLA+ model** passes TLC model checker with no errors
2. **Kani proofs** verify all safety properties
3. **Prusti contracts** compile and verify
4. **Property tests** achieve >95% coverage
5. **Miri** reports no undefined behavior
6. **No panics** possible in verified code paths

---

## References

- [TLA+ Home](https://lamport.azurewebsites.net/tla/tla.html)
- [Kani Rust Verifier](https://model-checking.github.io/kani/)
- [Prusti User Guide](https://viperproject.github.io/prusti-dev/)
- [VT100 Specification](https://vt100.net/docs/)
- [ECMA-48 Control Functions](https://www.ecma-international.org/publications-and-standards/standards/ecma-48/)

---

## Contact

**Source:** DashTerm2 project at https://github.com/dropbox/dTOOL/dashterm2
**Verification:** DashProve project at https://github.com/dropbox/dashprove
