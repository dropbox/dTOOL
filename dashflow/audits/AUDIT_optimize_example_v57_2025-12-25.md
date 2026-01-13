# Skeptical Audit v57: optimize/example.rs

**Date:** 2025-12-25
**Auditor:** Worker #1726
**File:** `crates/dashflow/src/optimize/example.rs`
**Lines:** 294 (205 code, 89 tests)
**Test Coverage:** 7 tests (30% by line)

## Summary

Example module provides a training example type for DashOptimize. Simple wrapper
around serde_json::Map with input/output field separation. Clean implementation
with builder pattern and reasonable heuristics.

**Result: NO P0/P1/P2/P3 issues found.**

## Architecture

```
Example
├── data: Map<String, Value>        # All fields
└── input_keys: Option<Vec<String>> # Explicit input field names

Builder Pattern:
├── new() -> Self
├── from_map(Map) -> Self
├── with(key, value) -> Self
├── with_field(key, value) -> Self  # alias
└── with_inputs(&[&str]) -> Self

Accessors:
├── get(&str) -> Option<&Value>
├── data() -> &Map
├── inputs() -> Map  # Only input fields
├── labels() -> Map  # Only output fields
├── len() -> usize
└── is_empty() -> bool
```

## Key Flows

1. **inputs()** (lines 97-125):
   - If input_keys explicitly specified: filter to those keys
   - Otherwise: exclude hardcoded OUTPUT_FIELDS heuristic

2. **labels()** (lines 131-159):
   - If input_keys explicitly specified: return non-input fields
   - Otherwise: return only hardcoded OUTPUT_FIELDS

## Issues Found

### P4 (Trivial)

#### M-868: Hardcoded OUTPUT_FIELDS heuristic may be surprising

**Category:** Design Choice

**Problem:**
When `input_keys` is not explicitly specified, the module uses a hardcoded list
of common output field names (lines 107-117):
```rust
const OUTPUT_FIELDS: &[&str] = &[
    "answer", "output", "label", "category", "classification",
    "result", "prediction", "rationale", "reasoning",
];
```

Fields with these names are automatically treated as outputs. Users with custom
field naming may get unexpected behavior.

**Impact:** None for users who call `with_inputs()` explicitly. Only affects
users relying on automatic inference, which is documented behavior.

**Fix:** Documentation recommends explicit `with_inputs()` for production use.

---

## Positive Findings

1. **Clean builder pattern** - Fluent API for constructing examples
2. **Explicit input specification** - `with_inputs()` allows precise control
3. **Reasonable defaults** - Heuristic covers common field names
4. **Good serde support** - Serializable/Deserializable with skip_serializing_if
5. **Multiple From impls** - Convenient creation from arrays and vecs
6. **Default impl** - Standard Rust pattern

## Test Coverage Analysis

| Test | Coverage |
|------|----------|
| test_example_creation | Basic From impl |
| test_example_inputs | Heuristic input detection |
| test_example_with | Builder pattern |
| test_example_empty | Empty state |
| test_with_field_alias | Alias method |
| test_explicit_input_keys | with_inputs() |
| test_labels_heuristic | Heuristic label detection |

## Code Quality Notes

1. **No unsafe code** - Pure safe Rust
2. **No panics** - All operations return Option or are infallible
3. **No unwrap/expect** - Clean error-free implementation
4. **Efficient cloning** - Only clones when building new Map in inputs()/labels()

## Performance Note

The `inputs()` and `labels()` methods use `Vec::contains()` for filtering when
`input_keys` is specified, which is O(n) per lookup. For typical examples with
< 10 fields, this is negligible. For large examples, a HashSet would be more
efficient, but this is not worth optimizing given typical usage patterns.

## Conclusion

**NO SIGNIFICANT ISSUES** - Simple, well-designed module for training example
representation. One P4 note about heuristic field detection, but users can
override with explicit `with_inputs()`. Clean implementation with no edge cases
or error conditions.
