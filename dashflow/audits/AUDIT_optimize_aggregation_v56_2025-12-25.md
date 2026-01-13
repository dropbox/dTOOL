# Skeptical Audit v56: optimize/aggregation.rs

**Date:** 2025-12-25
**Auditor:** Worker #1726
**File:** `crates/dashflow/src/optimize/aggregation.rs`
**Lines:** 362 (196 code, 166 tests)
**Test Coverage:** 14 tests (46% by line)

## Summary

Aggregation module provides majority voting for combining multiple JSON outputs.
Clean implementation with comprehensive error handling and good test coverage.
Found 1 P4 issue related to map iteration order.

**Result: NO P0/P1/P2/P3 issues found.**

## Architecture

```
Public API:
├── default_normalize(s: &str) -> Option<String>
│   └── Wraps normalize_text() with None for empty result
│
└── majority(values, normalize, field) -> Result<Value>
    └── Returns value containing most common field value
```

## Key Flows

1. **default_normalize()** (lines 51-58):
   - Apply normalize_text (NFD, lowercase, punctuation removal, article removal)
   - Return None if result is empty

2. **majority()** (lines 118-195):
   - Validate non-empty input
   - Determine field to vote on (explicit or last field)
   - Extract and normalize values
   - Count occurrences with first-index tracking
   - Return value with highest count (ties favor earlier occurrence)

## Issues Found

### P4 (Trivial)

#### M-867: Default field selection depends on serde_json map ordering

**Category:** Portability / Assumption

**Problem:**
When no field is explicitly specified, the code uses the "last" field from the
first value's object (lines 134-136):
```rust
obj.keys()
    .next_back()
    .ok_or_else(|| Error::Validation("First value has no fields".to_string()))?
```

serde_json's `Map` type uses either:
- `IndexMap` (with "preserve_order" feature): maintains insertion order
- `BTreeMap` (default): sorted alphabetically

This means "last field" could mean:
- Last inserted field (preserve_order)
- Lexicographically last field (default BTreeMap)

The test at line 300-308 assumes "answer" is last in `{"question": "What?", "answer": "Paris"}`,
but alphabetically "question" > "answer", so with BTreeMap, "question" would be selected.

**Impact:** Low - users should specify explicit field names; default behavior is
just a convenience. Test may fail with different serde_json feature flags.

**Fix:** Document the behavior explicitly or always require explicit field name.

---

## Positive Findings

1. **Comprehensive error handling** - Empty input, missing field, non-object values all handled
2. **Normalization filtering** - None from normalize function properly skips value
3. **Tie-breaking documented** - Earlier values win ties (implemented via first_idx tracking)
4. **Clean public API** - Two functions, well-documented with examples
5. **Excellent test coverage** - 14 tests covering edge cases (empty, missing, filtering)

## Test Coverage Analysis

| Test | Coverage |
|------|----------|
| test_default_normalize | Empty, whitespace, articles |
| test_majority_simple | Basic majority voting |
| test_majority_with_normalization | Case-insensitive voting |
| test_majority_specific_field | Multi-field values |
| test_majority_tie_prefers_earlier | Tie-breaking behavior |
| test_majority_empty_values | Empty input error |
| test_majority_missing_field | Missing field error |
| test_majority_uses_last_field_by_default | Default field selection |
| test_majority_with_normalize_filtering_none | Normalization filtering |
| test_majority_all_filtered_out | All values filtered error |
| test_majority_non_object_value | Non-object input error |
| test_majority_field_not_string | Non-string field error |

## Algorithm Analysis

The majority voting algorithm (lines 178-194):
1. Build HashMap: normalized_value -> (count, first_idx)
2. Find max by (count, Reverse(first_idx))
3. Return original value at first_idx

The `Reverse(first_idx)` ensures that for equal counts, smaller first_idx
(earlier occurrence) wins - this is correct and matches documentation.

## Conclusion

**NO SIGNIFICANT ISSUES** - Clean, well-tested aggregation module. One P4 issue
about map ordering that could be surprising but doesn't affect correctness when
users specify explicit field names (recommended practice).
