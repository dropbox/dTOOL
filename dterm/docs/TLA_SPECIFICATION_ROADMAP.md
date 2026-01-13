# TLA+ Specification Roadmap

**Status:** Complete (TLC validation done for all specs; see `HINT.md`)
**Target:** Formally verified terminal core with machine-checkable proofs

## Vision

dterm-core aims to be the **first terminal emulator with formally verified core invariants**. This document defines the roadmap for achieving rigorous TLA+ specifications that:

1. **Prove safety properties** - No crashes, no data loss, no undefined behavior
2. **Model all state transitions** - Every operation has a formal specification
3. **Verify cross-module invariants** - Components compose correctly
4. **Enable machine checking** - All properties verified by TLC model checker

---

## Current Specifications

| Spec | Lines | Invariants | Theorems | Status |
|------|-------|------------|----------|--------|
| Terminal.tla | ~470 | 8 | 6 | Validated (TLC) |
| Grid.tla | ~790 | 12 | 9 | Validated (TLC, cell_flags integrated) |
| Parser.tla | ~590 | 3 | 9 | Validated (TLC) |
| PagePool.tla | ~360 | 8 | 9 | Validated (TLC) |
| Scrollback.tla | ~510 | 8 | 12 | Validated (TLC) |
| Selection.tla | ~200 | 4 | 2 | Validated (TLC) |
| TerminalModes.tla | ~150 | 2 | 1 | Validated (TLC) |
| Coalesce.tla | ~100 | 2 | 1 | Validated (TLC) |

---

## Phase 1: Complete Grid.tla cell_flags Integration (Complete)

**Status:** Complete - operations updated, wide-char handling integrated

The `cell_flags` integration is complete across cursor movement, scrolling,
erase operations, and wide-char writes. See `tla/Grid.tla` for the finalized
actions and invariants.

---

## Phase 2: Model Checking Configuration (Complete)

**Status:** Complete - `tla/*.cfg` exists and TLC validation passes

Create TLC configuration files for each specification.

### Grid.cfg
```
CONSTANTS
    MaxRows = 5
    MaxCols = 10
    MaxScrollback = 10
    PageSize = 64

INIT Init
NEXT Next

INVARIANTS
    TypeInvariant
    Safety
    WideCharSafety

PROPERTIES
    TypeSafe
    SafetyHolds
    WideCharsSafe
```

### Parser.cfg
```
CONSTANTS
    Ground = "Ground"
    Escape = "Escape"
    EscapeIntermediate = "EscapeIntermediate"
    CsiEntry = "CsiEntry"
    CsiParam = "CsiParam"
    CsiIntermediate = "CsiIntermediate"
    CsiIgnore = "CsiIgnore"
    DcsEntry = "DcsEntry"
    DcsParam = "DcsParam"
    DcsIntermediate = "DcsIntermediate"
    DcsPassthrough = "DcsPassthrough"
    DcsIgnore = "DcsIgnore"
    OscString = "OscString"
    SosPmApcString = "SosPmApcString"

INIT Init
NEXT Next

INVARIANTS
    TypeInvariant
    Safety
    NoStuckStates

PROPERTIES
    TypeSafety
    CANRecovery
    SUBRecovery
    ParserNeverStuck
    Determinism
```

### PagePool.cfg
```
CONSTANTS
    MaxPages = 10
    PageSize = 64

INIT Init
NEXT Next

INVARIANTS
    TypeInvariant
    Safety
    ActivePagesHaveGeneration
    NoValidPinToFreedPage
    DoubleFreeImpossible

PROPERTIES
    TypeInvariantPreserved
    SafetyPreserved
    UseAfterFreeDetectable
    NoDoubleFree
```

### Scrollback.cfg
```
CONSTANTS
    HotLimit = 5
    WarmLimit = 10
    ColdLimit = 20
    MemoryBudget = 1000
    LineSize = 10
    LZ4Ratio = 2
    ZstdRatio = 4
    BlockSize = 2

INIT Init
NEXT Next

INVARIANTS
    TypeInvariant
    Safety
    TierAgeOrdering
    TierCapacityRespected

PROPERTIES
    TypeSafe
    SafetyHolds
    TierAgeOrderingHolds
    DataIntegrity
```

### Terminal.cfg (Composite)
```
CONSTANTS
    MaxRows = 4
    MaxCols = 8
    MaxScrollback = 8
    MaxParams = 4
    MaxIntermediates = 2
    PageSize = 32
    MemoryBudget = 500
    BlockSize = 2

INIT Init
NEXT Next

INVARIANTS
    TypeInvariant
    SafetyInvariant

PROPERTIES
    TypeSafe
    SafetyHolds
    GridScrollbackSync
    OriginModeWorks
    WideCharsValid
    SelectionValid
```

---

## Phase 3: Optional Enhancements (Backlog)

**Priority: LOW**
**Estimated Effort: 3-4 hours**

### Grid.tla - Missing Properties

1. **Cursor Never Splits Wide Char**
   ```tla
   CursorNotOnPlaceholder ==
       ~("WidePlaceholder" \in cell_flags[<<cursor.row, cursor.col>>])
   ```

2. **Erase Clears Both Wide and Placeholder**
   ```tla
   ErasePreservesWideCharIntegrity ==
       \* If a Wide cell is erased, its Placeholder must also be erased
       \A r \in 0..rows-1, c \in 0..cols-2:
           (cells[<<r,c>>] = 0 /\ "Wide" \notin cell_flags[<<r,c>>]) =>
               ("WidePlaceholder" \notin cell_flags[<<r,c+1>>])
   ```

3. **Resize Handles Wide Chars at New Column Boundary**
   ```tla
   ResizeWideCharAtBoundary ==
       \* When resizing, if a Wide char ends up at the last column,
       \* both the Wide and its Placeholder must be cleared
       \A r \in 0..rows'-1:
           ~("Wide" \in cell_flags'[<<r, cols'-1>>])
   ```

### Parser.tla - Missing Properties

1. **Escape Sequence Length Bound**
   ```tla
   EscapeSequenceBounded ==
       \* From any non-Ground state, Ground is reachable within
       \* MAX_PARAMS + MAX_INTERMEDIATES + 2 bytes
       TRUE  \* Proof by construction of recovery path
   ```

2. **No Infinite Loops**
   ```tla
   NoInfiniteLoop ==
       \* The parser never enters a cycle that doesn't include Ground
       \* (All cycles pass through Ground)
       TRUE  \* Structural property of state machine
   ```

### PagePool.tla - Missing Properties

1. **Memory Bounded**
   ```tla
   MemoryBounded ==
       Cardinality(active_pages) * PageSize <= MaxPages * PageSize
   ```

2. **Allocation Fairness**
   ```tla
   AllocationFair ==
       \* Free pages are reused before allocating new ones
       (free_pages /= {} /\ AllocPage) => reused' > reused
   ```

### Scrollback.tla - Missing Properties

1. **Compression Ratio Bounds**
   ```tla
   CompressionRatioValid ==
       /\ WarmMemory <= (WarmLineCount * LineSize) \div LZ4Ratio + 1
       /\ \* Cold is on disk, doesn't count
   ```

2. **Access Latency Ordering**
   ```tla
   \* Hot < Warm < Cold access latency (informal, not model-checkable)
   \* But we can verify the tier structure supports this
   TierStructureSupportsLatency ==
       /\ Len(hot) <= HotLimit  \* Hot is bounded for fast access
       /\ \A i \in 1..Len(warm): warm[i].lineCount <= BlockSize * 10
   ```

---

## Phase 4: Cross-Module Refinement

**Priority: MEDIUM**
**Estimated Effort: 4-6 hours**

### Terminal.tla Enhancements

The composite spec needs to model actual byte processing:

1. **Full Byte Processing**
   ```tla
   ProcessByteComplete(byte) ==
       \* Parser processes byte
       /\ ParserTransition(byte)
       \* Parser action dispatches to grid
       /\ CASE "Print" \in parser_actions -> GridPrintChar(byte)
            [] "Execute" \in parser_actions -> GridExecuteControl(byte)
            [] "CsiDispatch" \in parser_actions -> GridCsiDispatch(parser_params)
            [] OTHER -> UNCHANGED grid_vars
   ```

2. **Alternate Screen Buffer**
   ```tla
   \* Model the alternate screen buffer (used by vim, less, etc.)
   VARIABLES
       alt_grid_cells,
       alt_grid_cell_flags,
       alt_cursor

   SwitchToAlternateScreen ==
       /\ mode_alternate_screen' = TRUE
       /\ \* Save main screen, switch to alt

   SwitchToMainScreen ==
       /\ mode_alternate_screen' = FALSE
       /\ \* Restore main screen
   ```

3. **Selection Interaction with Grid**
   ```tla
   \* Selection must track grid changes
   SelectionInvalidatedOnScroll ==
       (display_offset' /= display_offset) =>
           (selection_state' = "None" \/ SelectionAdjustedForScroll)
   ```

---

## Phase 5: Refinement Mapping to Rust

**Priority: LOW (but important for completeness)**
**Estimated Effort: Ongoing**

Document the mapping between TLA+ specs and Rust implementation:

| TLA+ Concept | Rust Implementation | File:Line |
|--------------|---------------------|-----------|
| `grid_cursor` | `Grid.cursor` | grid/mod.rs:45 |
| `grid_cells` | `Grid.rows[].cells[]` | grid/row.rs:* |
| `cell_flags.Wide` | `CellFlags::WIDE_CHAR` | grid/mod.rs:28 |
| `parser_state` | `Parser.state` | parser/mod.rs:* |
| `scrollback_hot` | `Scrollback.hot` | scrollback/mod.rs:* |
| `pool_generation` | `PagePool.generation` | grid/page.rs:* |

---

---

## Phase 0: Rust Warning Hygiene (Historical)

**Status:** Historical note - track warnings via current `cargo` output

Use `cargo build` or `cargo clippy` output to track current warning counts.

---

## Validation Checklist (Reference)

Completed for current specs; reuse for future additions.

Before marking TLA+ work complete:

- [ ] All specs parse without errors (`tla2tools.jar -parse`)
- [ ] All specs type-check (`tla2tools.jar -check`)
- [ ] TLC runs without errors on all specs with small constants
- [ ] All INVARIANTS hold (no counterexamples found)
- [ ] All PROPERTIES hold (no counterexamples found)
- [ ] State space is explored completely (or bounded appropriately)
- [ ] Documentation maps specs to implementation

### TLC Command Reference

```bash
# Parse check
java -jar tla2tools.jar -parse Grid.tla

# Model check with config
java -jar tla2tools.jar -config Grid.cfg Grid.tla

# Model check with specific workers
java -jar tla2tools.jar -workers 4 -config Grid.cfg Grid.tla

# Generate state graph (for small models)
java -jar tla2tools.jar -dump dot states.dot -config Grid.cfg Grid.tla
```

---

## Success Criteria

The TLA+ specification work is complete when:

1. **Coverage**: Every public operation in dterm-core has a corresponding TLA+ action
2. **Safety**: All safety invariants are proven to hold
3. **Liveness**: Key liveness properties (like GroundReachable) are proven
4. **Composition**: Terminal.tla successfully composes all modules
5. **Validation**: TLC finds no counterexamples with realistic constants
6. **Documentation**: Clear mapping between spec and implementation

---

## References

- [TLA+ Video Course](https://lamport.azurewebsites.net/video/videos.html) - Leslie Lamport
- [Specifying Systems](https://lamport.azurewebsites.net/tla/book.html) - The TLA+ Book
- [TLC Model Checker](https://github.com/tlaplus/tlaplus) - TLA+ Tools
- [Amazon's Use of TLA+](https://lamport.azurewebsites.net/tla/amazon.html) - Industry case study
- [VT100 Parser State Machine](https://vt100.net/emu/dec_ansi_parser) - Parser reference
