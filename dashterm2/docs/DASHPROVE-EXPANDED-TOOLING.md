# DashProve Expanded Tooling Recommendations

**Date:** December 2025
**Purpose:** Additional verification, analysis, and testing tools to complement DashProve
**Status:** Research complete, ready for evaluation

---

## Executive Summary

This document expands the DashProve verification toolkit beyond the initial set (TLA+, Kani, Prusti, Miri). These tools are organized by:

1. **Rust Verification** - Formal methods for the new Rust core
2. **C/Objective-C Analysis** - For existing DashTerm2 codebase
3. **Concurrency Verification** - Critical for multi-tab terminal
4. **Fuzzing** - Finding edge cases through random input
5. **Security Analysis** - Taint tracking, vulnerability detection
6. **Protocol Verification** - For VT100/SSH state machines

---

## Part 1: Additional Rust Verification Tools

### 1.1 Verus (Microsoft/VMware Research)

**What:** Statically proves Rust code correctness using SMT solvers

**Capabilities:**
- Specification annotations for pre/post conditions
- Proves code satisfies specs for ALL possible executions
- Supports raw pointer verification (beyond standard Rust)
- Active development with growing feature set

**Use Case for DashProve:**
- Verify VT100 parser state transitions
- Prove buffer bounds for all inputs
- Verify cursor logic invariants

**Installation:**
```bash
# Requires nightly Rust
git clone https://github.com/verus-lang/verus
cd verus
./tools/get-z3.sh
source ./tools/activate
cd source && cargo build --release
```

**Example:**
```rust
use vstd::prelude::*;

verus! {
    fn cursor_move_right(x: u16, width: u16) -> (result: u16)
        requires
            width > 0,
            x < width,
        ensures
            result < width,
    {
        if x + 1 < width { x + 1 } else { x }
    }
}
```

**Priority:** HIGH - Complements Kani with different verification approach

---

### 1.2 Creusot (Deductive Verifier)

**What:** Translates Rust to Why3 for deductive verification

**Capabilities:**
- Proves absence of panics, overflows, assertion failures
- Annotations for functional correctness
- Leverages Why3 ecosystem and SMT solvers
- Good for algorithm verification

**Use Case for DashProve:**
- Verify sorting algorithms in buffer management
- Prove correctness of UTF-8 decoding
- Verify ring buffer invariants

**Installation:**
```bash
# Install Why3 first
opam install why3
# Then Creusot
cargo install --git https://github.com/creusot-rs/creusot
```

**Priority:** MEDIUM - Overlaps with Prusti but offers Why3 ecosystem

---

### 1.3 MIRAI (Meta/Facebook Abstract Interpreter)

**What:** Static analysis via abstract interpretation of Rust MIR

**Capabilities:**
- Linting to find unintentional panics
- Taint analysis for security bugs
- Constant-time analysis (crypto)
- Information leak detection
- CI-friendly integration

**Use Case for DashProve:**
- Verify SSH key handling doesn't leak secrets
- Find potential panics in parser code
- Detect taint flow from untrusted input

**Installation:**
```bash
cargo install --git https://github.com/endorlabs/MIRAI mirai
cargo mirai  # Run analysis
```

**Configuration (MIRAI.toml):**
```toml
[analysis]
diag_level = "default"  # or "paranoid" for stricter checks
```

**Priority:** HIGH - Security-focused, complements Kani/Prusti

---

### 1.4 Loom (Concurrency Testing)

**What:** Exhaustive concurrency permutation testing

**Capabilities:**
- Explores all possible thread interleavings
- State reduction to avoid combinatorial explosion
- Tests against C11 memory model
- Finds race conditions deterministically

**Use Case for DashProve:**
- Test LineBlock mutex scenarios
- Verify token pool thread safety
- Test multi-tab buffer access patterns

**Installation:**
```toml
[dev-dependencies]
loom = "0.7"
```

**Example:**
```rust
use loom::sync::Arc;
use loom::sync::atomic::{AtomicUsize, Ordering};
use loom::thread;

#[test]
fn test_concurrent_token_pool() {
    loom::model(|| {
        let pool = Arc::new(TokenPool::new());
        let pool2 = pool.clone();

        let t1 = thread::spawn(move || {
            let token = pool.acquire();
            pool.release(token);
        });

        let t2 = thread::spawn(move || {
            let token = pool2.acquire();
            pool2.release(token);
        });

        t1.join().unwrap();
        t2.join().unwrap();
    });
}
```

**Priority:** HIGH - Critical for multi-tab terminal correctness

---

### 1.5 cargo-careful (Extra UB Checking)

**What:** Runs Rust with extra undefined behavior checks

**Capabilities:**
- Catches UB that Miri might miss in some scenarios
- Easier to run than full Miri
- Good for quick sanity checks

**Installation:**
```bash
cargo install cargo-careful
cargo +nightly careful test
```

**Priority:** LOW - Miri is more comprehensive

---

## Part 2: C/Objective-C Analysis Tools

### 2.1 Infer (Meta/Facebook)

**What:** Static analyzer supporting Objective-C (!)

**Capabilities:**
- Null pointer exceptions
- Resource/memory leaks
- Concurrency race conditions
- Detects issues before runtime

**Use Case for DashProve:**
- Analyze existing DashTerm2 Objective-C code
- Find memory leaks in VT100*.m files
- Detect potential null dereferences

**Installation:**
```bash
brew install infer
# Or build from source for latest
```

**Usage:**
```bash
# For Xcode projects
infer run -- xcodebuild -project DashTerm2.xcodeproj -scheme DashTerm2 build

# Analyze specific files
infer run -- clang -c sources/VT100Token.m
```

**Priority:** **CRITICAL** - Only tool that analyzes existing Obj-C code!

---

### 2.2 CBMC (C Bounded Model Checker)

**What:** Formal verification for C/C++ via bounded model checking

**Capabilities:**
- Memory safety (array bounds, pointer safety)
- Undefined behavior detection
- User assertion checking
- Supports gcc/clang extensions

**Use Case for DashProve:**
- Verify C portions of LineBlock.mm
- Check buffer operations in ScreenCharArray
- Prove memory safety of grid operations

**Installation:**
```bash
brew install cbmc
```

**Usage:**
```bash
# Verify a C file with assertions
cbmc sources/VT100Grid.c --bounds-check --pointer-check

# With loop unwinding
cbmc file.c --unwind 10 --unwinding-assertions
```

**Priority:** HIGH - Formal verification for C code

---

### 2.3 Frama-C (C Verification Platform)

**What:** Comprehensive C analysis with ACSL specifications

**Capabilities:**
- **Eva plugin:** Proves absence of runtime errors via abstract interpretation
- **WP plugin:** Proves functional correctness with contracts
- **E-ACSL plugin:** Runtime annotation checking
- Used in safety-critical systems (aerospace, automotive)

**Use Case for DashProve:**
- NASA-grade verification of C parsing code
- Prove absence of buffer overflows
- Verify loop termination

**Installation:**
```bash
opam install frama-c
```

**Example ACSL Contract:**
```c
/*@ requires \valid(buffer + (0..size-1));
    requires size > 0;
    assigns buffer[0..size-1];
    ensures \forall integer i; 0 <= i < size ==> buffer[i] == 0;
*/
void clear_buffer(char *buffer, size_t size) {
    for (size_t i = 0; i < size; i++) {
        buffer[i] = 0;
    }
}
```

**Priority:** MEDIUM - Powerful but steep learning curve

---

### 2.4 Clang Static Analyzer

**What:** Built-in static analysis in Clang

**Capabilities:**
- Memory leak detection
- Dead store detection
- Null pointer analysis
- Already part of Xcode!

**Use Case for DashProve:**
- Quick analysis during builds
- Find obvious bugs in Obj-C code

**Usage:**
```bash
# Analyze with scan-build
scan-build xcodebuild -project DashTerm2.xcodeproj -scheme DashTerm2

# Or enable in Xcode: Product > Analyze
```

**Priority:** LOW - Already available, less powerful than Infer

---

## Part 3: Security Analysis Tools

### 3.1 CodeQL (GitHub)

**What:** Semantic code analysis - query code like a database

**Capabilities:**
- Taint tracking across function calls
- Custom vulnerability queries
- Finds vulnerability variants across codebase
- Free for open source

**Use Case for DashProve:**
- Find all paths where untrusted input reaches dangerous sinks
- Detect escape sequence injection vulnerabilities
- Track SSH credential flow

**Installation:**
```bash
# Install CLI
brew install codeql

# Create database
codeql database create dashterm-db --language=cpp --command="xcodebuild ..."

# Run queries
codeql database analyze dashterm-db security-queries
```

**Example Query (find buffer overflows):**
```ql
import cpp

from FunctionCall call, Function f
where call.getTarget() = f
  and f.getName() = "memcpy"
  and not exists(BoundsCheck bc | bc.getCheckedExpr() = call.getArgument(2))
select call, "Unchecked memcpy could overflow buffer"
```

**Priority:** HIGH - Excellent for security auditing

---

### 3.2 Semgrep (Already Listed - Expanded)

**Additional Capabilities:**
- Write custom rules for DashTerm-specific patterns
- CI integration
- Supply chain security

**Custom Rule Example:**
```yaml
rules:
  - id: dashterm-force-unwrap
    patterns:
      - pattern: $X!
    message: "Force unwrap can crash - use safe unwrapping"
    languages: [swift]
    severity: ERROR
```

---

## Part 4: Fuzzing Tools

### 4.1 cargo-fuzz (libFuzzer for Rust)

**What:** Coverage-guided fuzzing for Rust

**Capabilities:**
- Automatic input generation
- Coverage-guided mutation
- Crash reproduction
- Input minimization

**Use Case for DashProve:**
- Fuzz VT100 parser with random escape sequences
- Find edge cases in UTF-8 decoder
- Test buffer operations with random data

**Installation:**
```bash
cargo install cargo-fuzz
```

**Usage:**
```bash
cargo fuzz init
cargo fuzz add vt100_parser
cargo +nightly fuzz run vt100_parser
```

**Fuzz Target Example:**
```rust
// fuzz/fuzz_targets/vt100_parser.rs
#![no_main]
use libfuzzer_sys::fuzz_target;
use dashterm_vt100::Parser;

fuzz_target!(|data: &[u8]| {
    let mut parser = Parser::new(80, 24);
    for byte in data {
        parser.process_byte(*byte);
    }
    // Parser should never panic
});
```

**Priority:** **CRITICAL** - Best way to find parser bugs

---

### 4.2 AFL++ (American Fuzzy Lop)

**What:** Coverage-guided fuzzer for C/C++

**Capabilities:**
- Extremely effective at finding bugs
- Persistent mode for fast fuzzing
- QEMU mode for black-box fuzzing

**Use Case for DashProve:**
- Fuzz existing C parsing code
- Find crashes in VT100*.m

**Installation:**
```bash
brew install afl-fuzz
```

**Priority:** HIGH - For C code before Rust migration

---

### 4.3 OSS-Fuzz Integration

**What:** Google's continuous fuzzing service

**Capabilities:**
- 24/7 fuzzing on Google infrastructure
- Automatic bug filing
- Regression testing

**Use Case for DashProve:**
- Continuous fuzzing of VT100 parser
- Public security validation

**Priority:** MEDIUM - After initial stability

---

## Part 5: Protocol & State Machine Verification

### 5.1 Alloy (Lightweight Formal Methods)

**What:** Model finder for software design

**Capabilities:**
- Automatic counterexample generation
- Good for protocol design
- Easier than TLA+ for some use cases

**Use Case for DashProve:**
- Model SSH protocol state machine
- Verify escape sequence grammar

**Installation:**
```bash
# Download from alloytools.org
brew install --cask alloy
```

**Priority:** LOW - TLA+ is more established

---

### 5.2 SPIN (Protocol Verifier)

**What:** Model checker for concurrent systems (Promela language)

**Capabilities:**
- Deadlock detection
- Assertion checking
- Linear temporal logic

**Use Case for DashProve:**
- Verify multi-tab synchronization
- Check for deadlocks in buffer access

**Priority:** LOW - TLA+ covers similar ground

---

## Part 6: Specialized Testing

### 6.1 vttest (VT100 Conformance)

**What:** VT100/VT220/VT520 conformance test suite

**Capabilities:**
- Tests escape sequence handling
- Verifies terminal behavior
- Industry standard

**Use Case for DashProve:**
- Verify VT100 parser correctness
- Regression testing after changes

**Installation:**
```bash
brew install vttest
```

**Usage:**
```bash
# Run in terminal to test
vttest
```

**Priority:** **CRITICAL** - Must-have for terminal emulator

---

### 6.2 esctest (DashTerm2's Own Test Suite)

**What:** DashTerm2's escape sequence test framework

**Location:** Already in DashTerm2 codebase

**Priority:** **CRITICAL** - Already available, must be maintained

---

## Tool Priority Summary

### Tier 1: Must Have (Implement Immediately)

| Tool | Target | Purpose |
|------|--------|---------|
| **Infer** | Obj-C | Only formal analyzer for existing code |
| **cargo-fuzz** | Rust | Find parser edge cases |
| **Loom** | Rust | Verify concurrency correctness |
| **vttest** | Terminal | Conformance verification |
| **MIRAI** | Rust | Security taint analysis |

### Tier 2: High Value (Implement Soon)

| Tool | Target | Purpose |
|------|--------|---------|
| Verus | Rust | Alternative to Kani proofs |
| CBMC | C | Formal verification of C code |
| CodeQL | All | Security vulnerability hunting |
| AFL++ | C | Fuzz existing code |

### Tier 3: Nice to Have (Evaluate Later)

| Tool | Target | Purpose |
|------|--------|---------|
| Creusot | Rust | Why3-based verification |
| Frama-C | C | NASA-grade C verification |
| Alloy | Design | Protocol modeling |
| cargo-careful | Rust | Quick UB checks |

---

## Installation Script

```bash
#!/bin/bash
# install-dashprove-tools.sh

# Tier 1 - Must Have
brew install infer vttest

# Rust tools (requires nightly)
rustup install nightly
cargo install cargo-fuzz
cargo install --git https://github.com/endorlabs/MIRAI mirai

# Tier 2 - High Value
brew install cbmc codeql afl-fuzz

# Loom is a dev dependency, add to Cargo.toml:
# [dev-dependencies]
# loom = "0.7"

echo "DashProve tools installed. See DASHPROVE-EXPANDED-TOOLING.md for usage."
```

---

## Integration with CI

```yaml
# .github/workflows/dashprove.yml
name: DashProve Verification

on: [push, pull_request]

jobs:
  infer:
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v4
      - run: brew install infer
      - run: infer run -- xcodebuild -project DashTerm2.xcodeproj -scheme DashTerm2

  rust-verify:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: nightly
      - run: cargo install kani-verifier && kani setup
      - run: cd rust-core && cargo kani

  fuzz:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: cargo +nightly fuzz run vt100_parser -- -max_total_time=300

  loom:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: cd rust-core && cargo test --release -- --test-threads=1
        env:
          LOOM_MAX_PREEMPTIONS: 3
```

---

## Next Steps

1. **Immediate:** Run Infer on existing Obj-C codebase
2. **Week 1:** Set up cargo-fuzz for Rust parser prototype
3. **Week 2:** Implement Loom tests for token pool
4. **Week 3:** Run vttest conformance suite
5. **Ongoing:** Integrate tools into CI pipeline

---

## References

- [Verus](https://github.com/verus-lang/verus)
- [Creusot](https://github.com/creusot-rs/creusot)
- [MIRAI](https://github.com/endorlabs/MIRAI)
- [Loom](https://github.com/tokio-rs/loom)
- [Infer](https://fbinfer.com/)
- [CBMC](https://www.cprover.org/cbmc/)
- [Frama-C](https://frama-c.com/)
- [CodeQL](https://codeql.github.com/)
- [cargo-fuzz](https://github.com/rust-fuzz/cargo-fuzz)
- [vttest](https://invisible-island.net/vttest/)

---

*DashProve Expanded Tooling - December 2025*
