# TLA+ Specifications (DashFlow)

This directory contains **active** TLA+ specs used to model-check core DashFlow protocols.

## Specifications

| File | Description | Status | TLC Verified |
|------|-------------|--------|--------------|
| `GraphExecution.tla` | Graph node ordering, parallel execution, deadlock freedom | TLA-002 | TLA-003, TLA-004 (#2147) |
| `CheckpointRestore.tla` | Checkpoint/restore state consistency | TLA-005 | TLA-006 (#2147) |
| `TimeTravel.tla` | Time-travel debugging protocol | TLA-013 | ✅ Verified (local) |
| `WALProtocol.tla` | Write-ahead log durability and compaction | TLA-007 | TLA-008 (#2147) |
| `DashStreamKafka.tla` | DashStream Kafka producer/consumer at-least-once delivery | TLA-009 | TLA-010 (#2148) |
| `ParallelExecution.tla` | Fan-out/fan-in parallel node execution, deterministic merge | TLA-011 | TLA-012 (#2147) |

### Verification Results (#2147, #2148)

**DashStreamKafka.tla** - 89M states, all invariants pass (#2148):
- TypeInvariant: State types correct
- AllPartitionsOrdered: Same-thread messages ordered in partition
- CheckpointConsistency: Consumer resumes without gaps
- NoMessageLoss: Acknowledged messages stay in partition
- NoSkippedMessages: Consumer offset never exceeds log length
- EventualDelivery (liveness): All ack'd messages eventually delivered

**GraphExecution.tla** - 10 states, all invariants pass:
- TypeInvariant: State types correct
- SafetyInvariant: Predecessors complete before node activates
- NoOrphanExecution: Only reachable nodes execute
- ExactlyOnceInvariant: No duplicate execution
- NoDeadlock: System terminates (success or error)

**CheckpointRestore.tla** - 2.7M states, all invariants pass:
- TypeInvariant: State types correct
- CheckpointInvariant: Restore produces identical state
- IdempotentRestore: Restoring twice gives same result

**WALProtocol.tla** - 11.4M states, all invariants pass:
- TypeInvariant: State types correct
- NoDuplicateSeqs: Unique sequence numbers
- MonotonicOrder: Events ordered in segments

**ParallelExecution.tla** - 52K states, all invariants pass:
- TypeInvariant: State types correct
- FanInRequiresCompletion: All branches complete before merge
- DeterministicMerge: Final state independent of merge order
- NoEarlyExecution: No branch executes before fan-out
- MergeOnlyDuringFanIn: Merged state zero until fan-in

**TimeTravel.tla** - 4.3M distinct states, all invariants/properties pass (local):
- TypeInvariant: State types correct
- CursorConsistency: Cursor never exceeds highWaterMark
- StateDeterminism: Cursor always matches recorded history
- ReconstructionProperty: All recorded states are reconstructable
- MonotonicHighWaterMark: High water mark never decreases

## Prerequisites

- Java (TLC is a Java program)
- Optional: TLA+ Toolbox (interactive UI, ships `tla2tools.jar`)

macOS (Homebrew):
```bash
brew install --cask tla+-toolbox
brew install openjdk
```

## Run model checking manually

For specs with set/tuple constants (GraphExecution, ParallelExecution), use the MC_*.tla wrapper:
```bash
cd docs/tlaplus

# GraphExecution (uses MC_GraphExecution.tla for constants)
java -XX:+UseParallelGC -cp "/Applications/TLA+ Toolbox.app/Contents/Eclipse/tla2tools.jar" \
  tlc2.TLC MC_GraphExecution -config MC_GraphExecution.cfg -workers auto -deadlock

# CheckpointRestore
java -XX:+UseParallelGC -cp "/Applications/TLA+ Toolbox.app/Contents/Eclipse/tla2tools.jar" \
  tlc2.TLC CheckpointRestore -config CheckpointRestore.cfg -workers auto -deadlock

# WALProtocol
java -XX:+UseParallelGC -cp "/Applications/TLA+ Toolbox.app/Contents/Eclipse/tla2tools.jar" \
  tlc2.TLC WALProtocol -config WALProtocol.cfg -workers auto -deadlock

# ParallelExecution (uses MC_ParallelExecution.tla for constants)
java -XX:+UseParallelGC -cp "/Applications/TLA+ Toolbox.app/Contents/Eclipse/tla2tools.jar" \
  tlc2.TLC MC_ParallelExecution -config MC_ParallelExecution.cfg -workers auto -deadlock

# DashStreamKafka (uses MC_DashStreamKafka.tla for reduced constants)
java -XX:+UseParallelGC -cp "/Applications/TLA+ Toolbox.app/Contents/Eclipse/tla2tools.jar" \
  tlc2.TLC MC_DashStreamKafka -config MC_DashStreamKafka.cfg -workers auto -deadlock
```

The `-deadlock` flag tells TLC that terminal states (deadlocks) are expected and valid.

## Run model checking via script

From repo root:
```bash
./scripts/check_tlaplus.sh
```

The script automatically handles MC wrapper modules:
- For specs with `MC_*.tla` wrappers, uses the MC version (TLC-compatible constants)
- For specs with simple integer constants, uses the original spec directly
- Skips MC_* files when iterating (they're used by their parent spec)
- Auto-downloads `tla2tools.jar` into `target/tlaplus/` if missing (disable with `TLA_AUTO_DOWNLOAD=false`)
- Supports running a subset: `TLAPLUS_SPECS=TimeTravel ./scripts/check_tlaplus.sh`

Artifacts (states, fingerprints, logs) are written under `target/tlaplus/`.

## Adding a new spec

1. Add `MySpec.tla`
2. Add `MySpec.cfg` with:
   - Constant assignments for TLC (use simple integer constants)
   - `SPECIFICATION Spec` (or your chosen top-level spec)
   - `INVARIANT ...` and/or `PROPERTY ...` entries
3. If using set/tuple constants, create `MC_MySpec.tla` and `MC_MySpec.cfg`:
   - MC module extends your spec and defines concrete constants
   - MC config uses `<-` to substitute MC constants
4. Ensure `./scripts/check_tlaplus.sh` passes.

## Model Checking Tips

- Use bounded constants (e.g., MaxSeq = 5) to keep state space manageable
- TLC config files don't support TLA+ expressions - use MC wrapper modules
- RECURSIVE functions must be declared before use
- Use `-workers auto` to utilize all CPU cores
- Use `-deadlock` to allow expected terminal states
