# DashProve Expanded Tooling - Part 2

**Date:** December 2025
**Purpose:** Additional verification, analysis, and quality tools beyond Part 1
**Companion to:** DASHPROVE-EXPANDED-TOOLING.md

---

## Categories Covered

1. **Sanitizers** - Runtime bug detection
2. **Symbolic Execution** - Path exploration
3. **Theorem Provers** - Mathematical proofs
4. **SMT Solvers** - Constraint solving
5. **Memory Profiling** - Allocation analysis
6. **Code Coverage** - Test completeness
7. **Mutation Testing** - Test quality
8. **Supply Chain Security** - Dependency auditing
9. **Benchmarking** - Performance regression
10. **Developer Workflow** - Continuous checking
11. **Time-Travel Debugging** - Record/replay
12. **Differential Testing** - Cross-implementation validation

---

## Part 1: Sanitizers (Runtime Bug Detection)

### 1.1 AddressSanitizer (ASan)

**What:** Detects memory errors at runtime

**Detects:**
- Out-of-bounds access (heap, stack, global)
- Use-after-free
- Use-after-return
- Double-free
- Memory leaks

**Usage (C/Obj-C):**
```bash
# Compile with ASan
clang -fsanitize=address -g sources/VT100Grid.m -o test

# Xcode: Edit Scheme > Diagnostics > Address Sanitizer
```

**Usage (Rust):**
```bash
RUSTFLAGS="-Z sanitizer=address" cargo +nightly test
```

**Priority:** HIGH - Already mentioned but essential

---

### 1.2 ThreadSanitizer (TSan)

**What:** Detects data races and deadlocks

**Detects:**
- Data races between threads
- Deadlocks
- Lock ordering violations

**Usage:**
```bash
# C/Obj-C
clang -fsanitize=thread -g file.m -o test

# Rust
RUSTFLAGS="-Z sanitizer=thread" cargo +nightly test
```

**Use Case for DashProve:**
- Detect races in LineBlock global mutex usage
- Find data races in multi-tab scenarios
- Verify token pool thread safety

**Priority:** **CRITICAL** - Multi-tab terminal needs race detection

---

### 1.3 MemorySanitizer (MSan)

**What:** Detects use of uninitialized memory

**Detects:**
- Reading uninitialized heap memory
- Reading uninitialized stack memory
- Passing uninitialized memory to functions

**Usage:**
```bash
clang -fsanitize=memory -g file.c -o test
```

**Note:** Not available for Obj-C on macOS (Linux only)

**Priority:** MEDIUM - Important for C code paths

---

### 1.4 UndefinedBehaviorSanitizer (UBSan)

**What:** Detects undefined behavior at runtime

**Detects:**
- Integer overflow
- Null pointer dereference
- Misaligned pointers
- Invalid shift operations
- Out-of-bounds array access

**Usage:**
```bash
# C/Obj-C
clang -fsanitize=undefined -g file.m -o test

# Rust (limited - Rust prevents most UB at compile time)
RUSTFLAGS="-Z sanitizer=undefined" cargo +nightly test
```

**Priority:** HIGH - Catches subtle bugs

---

### 1.5 Combined Sanitizer Script

```bash
#!/bin/bash
# run-sanitizers.sh

# ASan
echo "=== Running AddressSanitizer ==="
xcodebuild -project DashTerm2.xcodeproj -scheme DashTerm2 \
  -enableAddressSanitizer YES \
  test

# TSan
echo "=== Running ThreadSanitizer ==="
xcodebuild -project DashTerm2.xcodeproj -scheme DashTerm2 \
  -enableThreadSanitizer YES \
  test

# UBSan
echo "=== Running UndefinedBehaviorSanitizer ==="
xcodebuild -project DashTerm2.xcodeproj -scheme DashTerm2 \
  -enableUndefinedBehaviorSanitizer YES \
  test
```

---

## Part 2: Symbolic Execution

### 2.1 KLEE

**What:** Dynamic symbolic execution engine for LLVM

**Capabilities:**
- Explores all possible execution paths
- Generates test cases automatically
- Finds bugs that random testing misses
- Proves absence of bugs on explored paths

**Use Case for DashProve:**
- Generate test cases for VT100 parser
- Find edge cases in escape sequence handling
- Explore all branches in state machine

**Installation:**
```bash
# Docker (recommended)
docker pull klee/klee:latest
docker run -it klee/klee
```

**Usage:**
```bash
# Compile to LLVM bitcode
clang -emit-llvm -c -g parser.c -o parser.bc

# Run KLEE
klee parser.bc
```

**Priority:** MEDIUM - Powerful but steep learning curve

---

### 2.2 KLEE for Rust (via verification-annotations)

**What:** Rust FFI to KLEE symbolic execution

**Installation:**
```toml
[dev-dependencies]
verification-annotations = "0.1"
```

**Usage:**
```rust
use verification_annotations::prelude::*;

fn parse_byte(b: u8) -> State {
    // KLEE will explore all 256 possible values
    let byte = u8::abstract_value();
    parser.process(byte)
}
```

**Priority:** LOW - Kani is more mature for Rust

---

## Part 3: Theorem Provers & Proof Assistants

### 3.1 Lean 4

**What:** Programming language and theorem prover

**Capabilities:**
- Formal mathematical proofs
- Dependent types
- Interactive theorem proving
- Can extract verified code

**Use Case for DashProve:**
- Prove VT100 state machine correctness mathematically
- Verify parser termination
- Prove UTF-8 decoder correctness

**Installation:**
```bash
# Install elan (Lean version manager)
curl https://raw.githubusercontent.com/leanprover/elan/master/elan-init.sh -sSf | sh

# Install Lean 4
elan default leanprover/lean4:stable
```

**Example (State Machine Proof):**
```lean
inductive ParserState where
  | ground
  | escape
  | csi
  | osc

def validTransition : ParserState → UInt8 → ParserState → Prop :=
  fun old byte new =>
    match old, byte with
    | .ground, 0x1B => new = .escape
    | .escape, 0x5B => new = .csi
    | _, _ => True  -- simplified

theorem parser_always_reaches_ground :
  ∀ (s : ParserState) (input : List UInt8),
    ∃ (n : Nat), reachesGround s input n := by
  sorry  -- proof goes here
```

**Priority:** LOW - Academic-grade, high investment

---

### 3.2 Coq

**What:** Formal proof assistant

**Capabilities:**
- Mechanized mathematical proofs
- Extract verified code to OCaml/Haskell
- Used for CompCert (verified C compiler)

**Use Case for DashProve:**
- Verify critical algorithms
- Extract proven-correct Rust code

**Installation:**
```bash
brew install coq
```

**Priority:** LOW - Very high learning curve

---

### 3.3 Isabelle/HOL

**What:** Generic proof assistant

**Capabilities:**
- Higher-order logic proofs
- Code generation
- Used in seL4 (verified microkernel)

**Priority:** LOW - Alternative to Coq

---

## Part 4: SMT Solvers

### 4.1 Z3 (Microsoft Research)

**What:** Satisfiability Modulo Theories solver

**Capabilities:**
- Solves logical constraints
- Backend for many verification tools
- Can be used directly for constraint problems

**Use Case for DashProve:**
- Backend for Kani/Prusti/Verus
- Direct constraint solving for complex invariants

**Installation:**
```bash
brew install z3

# Python bindings
pip install z3-solver
```

**Direct Usage Example:**
```python
from z3 import *

# Prove cursor bounds
x, y, width, height = Ints('x y width height')

s = Solver()
s.add(width > 0, height > 0)
s.add(x >= 0, x < width)
s.add(y >= 0, y < height)

# After move_right
new_x = If(x + 1 < width, x + 1, x)

# Prove new_x is still in bounds
s.add(Not(And(new_x >= 0, new_x < width)))
print(s.check())  # unsat = property holds
```

**Priority:** MEDIUM - Useful for custom verification

---

### 4.2 CVC5

**What:** Alternative SMT solver

**Capabilities:**
- Often faster than Z3 on some problems
- Good for finite model finding
- Supports quantifiers

**Installation:**
```bash
brew install cvc5
```

**Priority:** LOW - Z3 is more common

---

## Part 5: Memory Profiling

### 5.1 Heaptrack

**What:** Heap memory profiler

**Capabilities:**
- Traces all allocations with stack traces
- Finds memory leaks
- Identifies allocation hotspots
- Shows temporary allocations
- Flame graph visualization
- Lower overhead than Valgrind

**Use Case for DashProve:**
- Profile VT100Token allocation patterns
- Find memory leaks in Obj-C code
- Identify allocation hotspots

**Installation:**
```bash
# Linux only (not macOS native)
# Use in Docker or Linux VM
apt install heaptrack heaptrack-gui
```

**Usage:**
```bash
heaptrack ./dashterm2
heaptrack_gui heaptrack.dashterm2.*.gz
```

**Priority:** MEDIUM - Linux only limits usefulness

---

### 5.2 Valgrind Suite

**What:** Dynamic analysis framework

**Tools:**
| Tool | Purpose |
|------|---------|
| **Memcheck** | Memory error detection |
| **Helgrind** | Thread error detection |
| **DRD** | Data race detection |
| **Cachegrind** | Cache profiling |
| **Callgrind** | Call graph profiling |
| **Massif** | Heap profiling |

**Installation:**
```bash
# Note: Limited macOS support, best on Linux
brew install valgrind  # May not work on newer macOS
```

**Usage:**
```bash
# Memory errors
valgrind --tool=memcheck ./program

# Thread races
valgrind --tool=helgrind ./program

# Heap profiling
valgrind --tool=massif ./program
```

**Priority:** LOW - Limited macOS support

---

### 5.3 Instruments (macOS Native)

**What:** Apple's profiling toolkit

**Tools:**
| Template | Purpose |
|----------|---------|
| **Allocations** | Memory allocation tracking |
| **Leaks** | Memory leak detection |
| **Time Profiler** | CPU profiling |
| **System Trace** | Lock/thread analysis |

**Usage:**
```bash
# Command line
xcrun xctrace record --template 'Allocations' --launch ./DashTerm2

# Or use Instruments.app GUI
```

**Priority:** **HIGH** - Native macOS, no overhead issues

---

## Part 6: Code Coverage

### 6.1 Tarpaulin (Rust)

**What:** Code coverage tool for Rust

**Capabilities:**
- Line coverage tracking
- Multiple output formats (HTML, JSON, Lcov)
- Supports tests, doctests, benchmarks
- Coveralls/Codecov integration

**Installation:**
```bash
cargo install cargo-tarpaulin
```

**Usage:**
```bash
# Generate HTML report
cargo tarpaulin --out Html

# With specific features
cargo tarpaulin --features "full" --out Lcov
```

**Priority:** HIGH - Essential for test completeness

---

### 6.2 llvm-cov (C/Obj-C)

**What:** LLVM-based code coverage

**Usage:**
```bash
# Compile with coverage
clang -fprofile-instr-generate -fcoverage-mapping file.m -o test

# Run
LLVM_PROFILE_FILE="test.profraw" ./test

# Generate report
llvm-profdata merge test.profraw -o test.profdata
llvm-cov show ./test -instr-profile=test.profdata
```

**Priority:** MEDIUM - Instruments is easier on macOS

---

### 6.3 Xcode Coverage

**Usage:**
```bash
# Enable in scheme: Edit Scheme > Test > Options > Code Coverage

# Run tests
xcodebuild test -project DashTerm2.xcodeproj -scheme DashTerm2 \
  -enableCodeCoverage YES

# View in Xcode: Report Navigator > Coverage
```

**Priority:** HIGH - Native integration

---

## Part 7: Mutation Testing

### 7.1 cargo-mutants

**What:** Mutation testing for Rust

**Concept:** Introduces bugs (mutations) into code and checks if tests catch them. If a mutant survives, tests are inadequate.

**Mutations Applied:**
- Replace `+` with `-`
- Replace `true` with `false`
- Replace `>` with `>=`
- Delete statements
- Replace return values

**Installation:**
```bash
cargo install cargo-mutants
```

**Usage:**
```bash
# Run mutation testing
cargo mutants

# On specific files
cargo mutants --file src/parser.rs
```

**Example Output:**
```
Caught: replace + with - in cursor_move_right
MISSED: replace >= with > in bounds_check  # Test gap!
```

**Priority:** HIGH - Reveals test quality issues

---

## Part 8: Supply Chain Security

### 8.1 cargo-audit

**What:** Checks dependencies against RustSec advisory database

**Capabilities:**
- Detects known vulnerabilities in dependencies
- Warns about yanked crates
- CI integration

**Installation:**
```bash
cargo install cargo-audit
```

**Usage:**
```bash
cargo audit

# Fix vulnerabilities
cargo audit fix
```

**Priority:** **CRITICAL** - Security requirement

---

### 8.2 cargo-deny

**What:** Comprehensive dependency linting

**Checks:**
| Check | Purpose |
|-------|---------|
| **licenses** | Verify acceptable licenses |
| **bans** | Block specific crates |
| **advisories** | Security vulnerabilities |
| **sources** | Trusted registries only |

**Installation:**
```bash
cargo install cargo-deny
```

**Configuration (deny.toml):**
```toml
[licenses]
allow = ["MIT", "Apache-2.0", "BSD-3-Clause"]
deny = ["GPL-3.0"]

[bans]
deny = [
    { name = "openssl" },  # Prefer rustls
]
multiple-versions = "warn"

[advisories]
db-path = "~/.cargo/advisory-db"
vulnerability = "deny"
unmaintained = "warn"

[sources]
allow-registry = ["https://github.com/rust-lang/crates.io-index"]
```

**Usage:**
```bash
cargo deny check
```

**Priority:** **CRITICAL** - Security + license compliance

---

### 8.3 Dependabot / Renovate

**What:** Automated dependency updates

**Setup (.github/dependabot.yml):**
```yaml
version: 2
updates:
  - package-ecosystem: "cargo"
    directory: "/"
    schedule:
      interval: "weekly"
    open-pull-requests-limit: 10

  - package-ecosystem: "npm"
    directory: "/"
    schedule:
      interval: "weekly"
```

**Priority:** HIGH - Keeps dependencies current

---

## Part 9: Benchmarking

### 9.1 Criterion.rs

**What:** Statistics-driven microbenchmarking for Rust

**Capabilities:**
- Statistical analysis of performance
- Detects performance regressions
- HTML reports with graphs
- Comparison between runs

**Installation:**
```toml
[dev-dependencies]
criterion = "0.5"

[[bench]]
name = "parser_bench"
harness = false
```

**Example:**
```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_parse_ascii(c: &mut Criterion) {
    let data = vec![b'y'; 10000];
    let mut parser = Parser::new(80, 24);

    c.bench_function("parse 10k ascii", |b| {
        b.iter(|| {
            for byte in &data {
                parser.process_byte(black_box(*byte));
            }
        })
    });
}

criterion_group!(benches, bench_parse_ascii);
criterion_main!(benches);
```

**Usage:**
```bash
cargo bench

# Compare to baseline
cargo bench -- --save-baseline main
cargo bench -- --baseline main
```

**Priority:** **HIGH** - Prevents performance regressions

---

### 9.2 hyperfine

**What:** Command-line benchmarking tool

**Capabilities:**
- Statistical analysis
- Warmup runs
- Comparison between commands
- Export to JSON/Markdown

**Installation:**
```bash
brew install hyperfine
```

**Usage:**
```bash
# Benchmark command
hyperfine 'yes | head -100000'

# Compare implementations
hyperfine 'cat bigfile.txt' './dashterm2-new < bigfile.txt' './dashterm2-old < bigfile.txt'

# With warmup
hyperfine --warmup 3 'cargo build --release'
```

**Priority:** HIGH - Quick performance comparisons

---

## Part 10: Developer Workflow

### 10.1 bacon

**What:** Background code checker for Rust

**Capabilities:**
- Continuous compilation checking
- Quick switching between tasks (clippy, test, doc)
- Minimal interaction while coding

**Installation:**
```bash
cargo install bacon
```

**Usage:**
```bash
bacon  # Start watching

# Keyboard shortcuts:
# c - clippy
# t - tests
# d - docs
# q - quit
```

**Priority:** MEDIUM - Nice developer experience

---

### 10.2 Clippy

**What:** Rust linter with 800+ lints

**Lint Categories:**
| Category | Default | Purpose |
|----------|---------|---------|
| correctness | deny | Wrong/useless code |
| suspicious | warn | Likely wrong |
| style | warn | Idiomatic issues |
| complexity | warn | Unnecessarily complex |
| perf | warn | Performance issues |
| pedantic | allow | Strict checks |
| restriction | allow | Feature restrictions |

**Usage:**
```bash
# Run clippy
cargo clippy

# Pedantic mode
cargo clippy -- -W clippy::pedantic

# Fix automatically
cargo clippy --fix
```

**Recommended Configuration (.clippy.toml):**
```toml
msrv = "1.70"
cognitive-complexity-threshold = 30
```

**Priority:** **CRITICAL** - Must have for Rust

---

## Part 11: Time-Travel Debugging

### 11.1 rr (Record and Replay)

**What:** Record program execution for deterministic replay debugging

**Capabilities:**
- Record execution once
- Replay infinitely with full GDB
- Reverse execution (step backwards!)
- Find intermittent bugs

**Limitations:**
- Linux only (x86-64, some ARM)
- Requires kernel 4.7+

**Installation:**
```bash
# Linux
apt install rr

# Or from source
git clone https://github.com/rr-debugger/rr
cd rr && mkdir build && cd build
cmake .. && make && make install
```

**Usage:**
```bash
# Record
rr record ./dashterm2

# Replay with GDB
rr replay

# In GDB:
(gdb) reverse-continue
(gdb) reverse-step
```

**Use Case for DashProve:**
- Debug intermittent concurrency bugs
- Reproduce race conditions deterministically

**Priority:** MEDIUM - Linux only limits usefulness

---

### 11.2 LLDB Time Travel (macOS)

**Workaround:** Use Xcode's checkpoint/restore feature

```
(lldb) process save-core /tmp/checkpoint1
(lldb) process load-core /tmp/checkpoint1
```

**Priority:** LOW - Not true time-travel

---

## Part 12: Differential Testing

### 12.1 Concept

**What:** Compare outputs between implementations

**For DashProve:**
1. Run same escape sequences through DashTerm2 and reference terminal (xterm, VT100.net)
2. Compare resulting screen state
3. Any difference is a bug

**Implementation:**
```python
#!/usr/bin/env python3
# differential_test.py

import subprocess
import difflib

def run_terminal(binary, input_data):
    proc = subprocess.run([binary], input=input_data, capture_output=True)
    return proc.stdout

def test_escape_sequence(seq):
    dashterm_output = run_terminal('./dashterm2', seq)
    reference_output = run_terminal('./reference_term', seq)

    if dashterm_output != reference_output:
        diff = difflib.unified_diff(
            reference_output.decode().splitlines(),
            dashterm_output.decode().splitlines()
        )
        print(f"MISMATCH for {seq.hex()}:")
        print('\n'.join(diff))
        return False
    return True

# Test all CSI sequences
for param in range(256):
    test_escape_sequence(f"\x1b[{param}m".encode())
```

**Priority:** HIGH - Essential for correctness

---

## Tool Installation Master Script

```bash
#!/bin/bash
# install-dashprove-tools-full.sh

set -e

echo "=== Installing DashProve Full Toolset ==="

# Rust tools
echo "Installing Rust tools..."
rustup component add clippy llvm-tools-preview
cargo install cargo-tarpaulin cargo-mutants cargo-audit cargo-deny
cargo install cargo-fuzz criterion bacon

# Sanitizers (built into clang/rustc)
echo "Sanitizers are built-in to clang and rustc"

# SMT Solvers
echo "Installing SMT solvers..."
brew install z3

# Coverage
echo "Installing coverage tools..."
brew install lcov

# Benchmarking
echo "Installing benchmarking tools..."
brew install hyperfine

# Security
echo "Security tools..."
brew install semgrep

# Supply chain
echo "Setting up Dependabot..."
cat > .github/dependabot.yml << 'EOF'
version: 2
updates:
  - package-ecosystem: "cargo"
    directory: "/"
    schedule:
      interval: "weekly"
EOF

echo "=== Installation Complete ==="
echo "See DASHPROVE-TOOLING-PART2.md for usage instructions"
```

---

## CI Integration

```yaml
# .github/workflows/dashprove-full.yml
name: DashProve Full Suite

on: [push, pull_request]

jobs:
  sanitizers:
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v4
      - name: ASan
        run: |
          xcodebuild test -scheme DashTerm2 -enableAddressSanitizer YES
      - name: TSan
        run: |
          xcodebuild test -scheme DashTerm2 -enableThreadSanitizer YES

  coverage:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: cargo install cargo-tarpaulin
      - run: cargo tarpaulin --out Xml
      - uses: codecov/codecov-action@v3

  mutation:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: cargo install cargo-mutants
      - run: cargo mutants --timeout 300

  supply-chain:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: cargo audit
      - run: cargo deny check

  benchmarks:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: cargo bench -- --save-baseline pr-${{ github.sha }}
```

---

## Summary: All DashProve Tools

### Part 1 Tools (Previous Document)
- TLA+, Kani, Prusti, Miri (formal verification)
- Infer, CBMC, Frama-C (C/Obj-C analysis)
- cargo-fuzz, AFL++ (fuzzing)
- Loom, MIRAI (concurrency/security)
- CodeQL, Semgrep (security)
- vttest (conformance)

### Part 2 Tools (This Document)
- Sanitizers: ASan, TSan, MSan, UBSan
- Symbolic: KLEE
- Theorem provers: Lean 4, Coq
- SMT: Z3, CVC5
- Memory: Heaptrack, Valgrind, Instruments
- Coverage: Tarpaulin, llvm-cov, Xcode
- Mutation: cargo-mutants
- Supply chain: cargo-audit, cargo-deny, Dependabot
- Benchmarking: Criterion, hyperfine
- Workflow: bacon, Clippy
- Debugging: rr
- Differential testing

---

**Total Tools Catalogued: 40+**

*DashProve Tooling Part 2 - December 2025*
