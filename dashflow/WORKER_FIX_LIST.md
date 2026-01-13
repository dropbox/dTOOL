# WORKER FIX LIST - Codex DashFlow Audit Issues

**Created:** 2025-12-19
**Last Updated:** 2025-12-19 (Worker #1264 - Issue 29 prometheus-exporter metrics consolidated to librarian)
**Priority:** Fix in order listed (P0 first)

---

## COMPLETED ISSUES

### Session #1200-1206

| Issue | Commit | Description |
|-------|--------|-------------|
| Issue 1 (P0) | #1199 | Workspace Cargo.toml - fixed |
| Issue 2 (P1) | #547 | Deprecated TraceStep re-export - removed |
| Issue 3 (P1) | prior | Non-portable symlink - removed |
| Issue 4-5 (P2) | #547 | Stale CLI documentation (codex_dashflow) |
| Issue 9-11 | #1199 | LintError issues - fixed |
| Issue 12 | #1202 | QUICKSTART.md - updated |
| Issue 13 | #1205 | prometheus.yml - stale job configs removed |
| Issue 14 | #1201 | run_multi_turn_tests.sh - updated |
| Issue 15 | #1203 | docs/COOKBOOK.md - 7 broken links fixed |
| Issue 16 | #1204 | docs/BEST_PRACTICES.md - updated references |
| Issue 17 | #1204 | docs/EVALUATION_TROUBLESHOOTING.md - updated |
| Issue 18 | #1201 | docs/EXAMPLE_APPS.md - rewritten |
| Issue 19 | #1206 | docs/RELEASE_NOTES_v1.7.0.md - historical note added |
| Issue 20 | #1204 | observability-ui/README.md - updated |
| Issue 21 | N/A | PLAN_GUTENBERG_RAG.md - file no longer exists (moot) |
| Issue 22 | #1201 | .gitignore stale entry - removed |

---

## NEW P1 ISSUES - Critical Documentation (Found by Worker #1208)

### Issue 30: examples/README.md references 12+ deleted apps ✓ COMPLETE (#1210)
**File:** `/Users/ayates/dashflow/examples/README.md`
**Status:** Completely rewritten to document only `librarian` and `common`
- Removed all references to deleted apps
- Added historical note about consolidated apps
- Kept useful quick start examples (providers, vector stores)
- Kept DashStream documentation
- Updated testing and performance sections

### Issue 31: examples/apps/TESTING.md references deleted apps ✓ COMPLETE (#1213)
**File:** `/Users/ayates/dashflow/examples/apps/TESTING.md`
**Status:** Completely rewritten to focus on `librarian` testing
- Removed all references to 14 deleted apps
- Added historical note about consolidation
- Updated commands for librarian package

### Issue 32: examples/apps/README_OBSERVABILITY.md references deleted apps ✓ COMPLETE (#1213)
**File:** `/Users/ayates/dashflow/examples/apps/README_OBSERVABILITY.md`
**Status:** Completely rewritten to focus on `librarian` observability
- Removed references to document_search_streaming, advanced_rag, code_assistant
- Added historical note about consolidation
- Updated all metrics examples for librarian

### Issue 33: examples/apps/README_LOAD_TESTING.md references deleted apps ✓ COMPLETE (#1213)
**File:** `/Users/ayates/dashflow/examples/apps/README_LOAD_TESTING.md`
**Status:** Completely rewritten to focus on `librarian` load testing
- Removed 1300+ lines referencing deleted apps
- Simplified to essential load testing patterns
- Added historical note about consolidation

### Issue 34: crates/dashflow-streaming/README.md references deleted apps ✓ COMPLETE (#1213)
**File:** `/Users/ayates/dashflow/crates/dashflow-streaming/README.md`
**Status:** Updated ~15 references to deleted apps
- Changed all document_search references to librarian
- Updated eval suite examples to use librarian
- Updated test function names in code examples

### Issue 35: crates/dashflow-evals/README.md references deleted apps ✓ COMPLETE (#1213)
**File:** `/Users/ayates/dashflow/crates/dashflow-evals/README.md`
**Status:** Updated references to deleted apps
- Changed document_search golden dataset path to librarian
- Updated example applications section
- Added historical note about consolidation

---

## P2: MEDIUM - Stale Docstrings in Rust Crates

### Issue 27: dashflow-evals docstrings reference deleted apps ✓ COMPLETE (#1207)
**Files:** (8 files updated)
- `crates/dashflow-evals/src/lib.rs` - updated
- `crates/dashflow-evals/src/golden_dataset.rs` - updated
- `crates/dashflow-evals/src/report.rs` - updated
- `crates/dashflow-evals/src/report/markdown.rs` - updated
- `crates/dashflow-evals/src/report/json.rs` - updated
- `crates/dashflow-evals/src/report/html.rs` - updated
- `crates/dashflow-evals/src/baseline.rs` - updated (4 occurrences)
- `crates/dashflow-evals/tests/integration_test.rs` - updated (test + function renamed)

**Status:** All docstrings updated to reference `librarian` example

### Issue 28: dashflow-streaming docstrings reference deleted apps ✓ COMPLETE (#1208)
**Files:** (5 files updated, 1 test file left as-is for mock data)
- `crates/dashflow-streaming/src/bin/eval_runner.rs` - updated
- `crates/dashflow-streaming/src/evals/mod.rs` - updated
- `crates/dashflow-streaming/src/evals/baseline.rs` - updated (docstrings + tests)
- `crates/dashflow-streaming/src/evals/dataset.rs` - updated
- `crates/dashflow-streaming/src/evals/test_harness.rs` - updated
- `crates/dashflow-streaming/tests/evals_integration.rs` - mock data left as-is (not confusing)

**Status:** All docstrings updated to reference `librarian` example

### Issue 29: dashflow-prometheus-exporter has app-specific metrics ✓ COMPLETE (#1264)
**Files:**
- `crates/dashflow-prometheus-exporter/src/main.rs` - Updated
- `crates/dashflow-prometheus-exporter/tests/integration_test.rs` - Updated

**Status:** COMPLETE - metrics consolidated to librarian_*
**Changes made:**
- Renamed struct fields from code_assistant_*/document_search_streaming_* to librarian_*
- Updated Metrics::new() to create librarian_* metrics
- Updated update_from_quality_event() to route legacy app names to librarian metrics
- Updated all unit tests to use librarian metrics
- Updated integration tests to expect librarian_* metrics
- Added backward compatibility: legacy app types still routed to librarian metrics

---

## P2: MEDIUM - Code Quality (codex_dashflow)

### Issue 23: Large source files need refactoring
**Files:**
- `streaming.rs` (7,271 lines) - Consider splitting into modules
- `codex.rs` (5,004 lines) - Consider splitting into modules
- `turn_diff_tracker.rs` (3,182 lines)
- `llm.rs` (2,849 lines)
- `tool_execution.rs` (2,654 lines)

**Impact:** Maintenance burden, harder to navigate
**Fix:** Gradual refactoring into sub-modules

### Issue 24: Excessive unwrap() usage
**Files:** 47 files in codex_dashflow/crates/core/src
**Count:** 1,794 total occurrences
**Impact:** Potential panics in production code
**Fix:** Replace with proper error handling where appropriate (prioritize non-test code)

---

## P3: LOW - Stale Reports

### Issue 25: WORKER_DIRECTIVE.md references deleted example apps ✓ COMPLETE (#1235)
**File:** `/Users/ayates/dashflow/WORKER_DIRECTIVE.md`
**Line:** 6019
**Status:** Removed code_assistant reference, added note about consolidation

### Issue 26: Historical reports reference deleted apps
**Files:**
- `reports/main/observability_stack_assessment_2025-12-13.md`
- `reports/main/observability_deep_dive_2025-12-13.md`
- `reports/main/archive_gap_analysis_2025-12-03/`
- `docs/completed_phases/` (multiple files)

**Impact:** Confusing for readers following historical documentation
**Fix:** Add note that example apps were consolidated, or archive reports

---

## M-71: println! → tracing Migration ✓ COMPLETE (#1213-1216)

**Status:** 143 of 174 println! calls converted to structured tracing
**Remaining:** 31 calls (all in doc comments/examples/tests - acceptable)

**Files converted:**
- mipro_v2.rs, simba.rs, bootstrap_optuna.rs, autoprompt.rs, random_search.rs
- modules/react.rs, modules/chain_of_thought.rs, modules/multi_chain_comparison.rs, modules/avatar.rs
- propose.rs, llm_node.rs, bootstrap.rs

**Benefits:**
- Structured log fields enable better log querying
- Log levels (info/debug/warn) allow filtering
- Consistent patterns across optimize/ module

---

## NEW ISSUES - Comprehensive Audit (Found by Worker #1218)

### Issue 36: Stale app references in test files ✓ COMPLETE (#1225)
**Files:**
- `crates/dashflow-streaming/tests/evals_integration.rs` - updated mock data
- `test-utils/tests/streaming_parity.rs` - updated comment reference

**Status:** All mock data app names changed from document_search/code_assistant to librarian/test_app

### Issue 37: #[allow(dead_code)] suppression overuse (P3)
**Files:** 56 files across workspace
**Key offenders:**
- `crates/dashflow/src/optimize/optimizers/` - 6 files
- `crates/dashflow/src/checkpoint/` - 4 files
- `crates/dashflow/src/self_improvement/` - 2 files
- `crates/dashflow/src/colony/` - 2 files
- `crates/dashflow/src/lint/` - 3 files

**Problem:** Dead code is being suppressed instead of removed
**Impact:** Code bloat, maintenance burden
**Fix:** Audit each file - remove dead code or document why it's needed

### Issue 38: Deprecated Zapier crate ✓ COMPLETE (#1227)
**File:** (removed) formerly `crates/dashflow-zapier/README.md`
**Status:** Added prominent deprecation notice to README

**Changes:**
- Added block quote deprecation warning at top of README
- Links to sunset notice URL
- Warns against use for new projects
- lib.rs already has all items marked `#[deprecated]`

### Issue 39: dashflow-evals eval_runner.rs uses eprintln! ✓ COMPLETE (prior worker)
**File:** `crates/dashflow-evals/src/eval_runner.rs`
**Status:** Already fixed - all eprintln! calls converted to tracing

**Remaining eprintln! in crate (acceptable):**
- `multi_model.rs:1586,1747,1931,2078` - Test code (#[ignore = "requires API key"])
- `regression.rs:239,241` - Doc comment examples

### Issue 40: Postgres checkpointer uses eprintln! ✓ COMPLETE (M-72, #1228)
**Files:**
- `crates/dashflow-postgres-checkpointer/src/lib.rs` (previously checkpointer.rs:65) - converted to tracing::error!
- `crates/dashflow-memory/src/chat_message_histories/postgres.rs:172` - converted to tracing::error!

**Status:** Fixed as part of M-72 eprintln! migration

### Issue 41: Registry server uses eprintln! - INTENTIONALLY KEPT
**File:** `crates/dashflow-registry/src/bin/registry_server.rs:239`
**Count:** 1 call

**Status:** Intentionally kept as fallback for when tracing subscriber setup fails.
This is the only place eprintln! is appropriate - can't use tracing if tracing setup failed.

---

## M-72: eprintln! → tracing Migration (P2) ✓ COMPLETE (#1228)
**Status:** 9/10 calls converted
**Scope:** ~10 calls in production library code

**Files converted:**
- ✓ `crates/dashflow-evals/src/eval_runner.rs` - 7 calls → tracing::info!/debug!/warn!
- ✓ `crates/dashflow-postgres-checkpointer/src/checkpointer.rs` - 1 call → tracing::error!
- ✓ `crates/dashflow-memory/src/chat_message_histories/postgres.rs` - 1 call → tracing::error!
- ✗ `crates/dashflow-registry/src/bin/registry_server.rs` - 1 call (KEEP: fallback for tracing setup failure)

**Note:** eprintln! in tests, examples, and CLI apps is acceptable.
Registry server eprintln! intentionally kept as fallback when tracing subscriber fails to initialize.

---

## M-73: println! in dashflow core crate (P3) - TRACKED
**Status:** TRACKED as M-407 in ROADMAP_CURRENT.md (added 2025-12-21)
**Scope:** 401 println! calls across 108 files in `crates/dashflow/src/`

**High-count files (>10 calls):**
- `executor.rs` - 50 calls
- `platform_registry.rs` - 24 calls
- `core/agent_patterns.rs` - 18 calls
- `event.rs` - 16 calls
- `self_improvement/observability.rs` - 13 calls
- `introspection/integration.rs` - 12 calls
- `trace_analysis.rs` - 10 calls
- `core/runnable.rs` - 10 calls
- `graph.rs` - 10 calls
- `optimize/distillation/mod.rs` - 9 calls

**Impact:** Inconsistent logging across core crate
**Fix:** Gradual conversion to tracing (lower priority than M-72)

---

## NEW ISSUES - Extended Audit (Found by Worker #1219)

### Issue 42: docs/COOKBOOK.md references deleted apps ✓ COMPLETE (#1222)
**File:** `/Users/ayates/dashflow/docs/COOKBOOK.md`
**Status:** Updated 2 broken links (lines 596, 716)
- Changed error_recovery and streaming_aggregator references to librarian

### Issue 43: docs/TESTING.md references deleted apps ✓ COMPLETE (#1222)
**File:** `/Users/ayates/dashflow/docs/TESTING.md`
**Status:** Updated 1 reference (line 563)
- Changed advanced_rag command to librarian

### Issue 44: docs/DEVELOPER_EXPERIENCE.md references deleted apps ✓ COMPLETE (#1223)
**File:** `/Users/ayates/dashflow/docs/DEVELOPER_EXPERIENCE.md`
**Status:** Updated 15+ references to document_search
- All EVAL_APP references changed to librarian
- Updated cargo run commands to use librarian
- Updated golden dataset path

### Issue 45: crates/dashflow-derive/README.md references deleted apps ✓ COMPLETE (#1222)
**File:** `/Users/ayates/dashflow/crates/dashflow-derive/README.md`
**Status:** Updated 5 references
- Changed streaming_aggregator section to use librarian
- Added historical note about consolidated apps

### Issue 46: docs/RELEASE_NOTES_v1.9.0.md references deleted apps ✓ COMPLETE (#1233)
**File:** `/Users/ayates/dashflow/docs/RELEASE_NOTES_v1.9.0.md`
**Count:** 20+ references
**Status:** Added historical note at top (like v1.7.0)

### Issue 47: 15 stale scripts reference deleted apps ✓ COMPLETE (#1224)
**Directory:** `scripts/`
**Status:** All 15 obsolete scripts deleted
**Files deleted:**
- `demo_apps_smoke.sh` - tested 14+ deleted apps
- `e2e_stack_validation.sh` - document_search_streaming
- `validate_python_app1.sh`, `validate_python_app2.sh`, `validate_python_app3.sh`
- `validate_rust_app1.sh`
- `test_observability_pipeline.sh`
- `benchmark_app1.sh`
- `validate_advanced_rag.sh` - advanced_rag
- `run_eval.sh` - document_search
- `validate_all_apps.sh` - all deleted apps
- `validate_code_assistant.sh` - code_assistant
- `setup-eval-hooks.sh` - document_search
- `load_test_apps.sh`
- `validate_document_search.sh` - document_search

**Note:** Librarian has its own testing via `cargo test -p librarian`

### Issue 48: test-utils/tests/streaming_parity.rs references deleted app ✓ COMPLETE (#1225)
**File:** `/Users/ayates/dashflow/test-utils/tests/streaming_parity.rs`
**Status:** Updated comment reference to librarian

### Issue 58: test-matrix crate is entirely obsolete ✓ COMPLETE (#1226)
**Directory:** `/Users/ayates/dashflow/test-matrix/`
**Status:** Entire crate deleted (~770 lines)
- Cargo.toml, src/lib.rs, tests/test_matrix_runner.rs all deleted
- Removed from workspace members in root Cargo.toml
- Crate tested apps that no longer exist (document_search, advanced_rag, code_assistant)

### Issue 49: crates/dashflow-standard-tests references deleted app - NOT AN ISSUE
**File:** `/Users/ayates/dashflow/crates/dashflow-standard-tests/tests/complete_eval_loop.rs`
**Line:** 48
**Analysis:** The `document_search` at line 48 is a **tool name**, not an app reference.
The `DocumentSearchTool` struct implements a mock search tool with name "document_search".
This is a valid tool name for a document search capability - not a reference to the deleted app.
**Status:** Closed - Not an issue

### Issue 50: crates/dashflow-prometheus-exporter/README.md references deleted apps ✓ COMPLETE (#1222)
**File:** `/Users/ayates/dashflow/crates/dashflow-prometheus-exporter/README.md`
**Status:** Updated test coverage description to use generic app references

### Issue 51: docs/EVALUATION_GUIDE.md references deleted apps ✓ COMPLETE (#1223)
**File:** `/Users/ayates/dashflow/docs/EVALUATION_GUIDE.md`
**Status:** Updated 10+ references to document_search
- All cargo run commands changed to librarian
- Updated golden dataset path to librarian/data/
- Updated troubleshooting commands

### Issue 52: docs/EVALUATION_BEST_PRACTICES.md references deleted apps ✓ COMPLETE (#1223)
**File:** `/Users/ayates/dashflow/docs/EVALUATION_BEST_PRACTICES.md`
**Status:** Updated 1 reference
- Changed golden dataset path from document_search to librarian

### Issue 53: docs/AI_PARTS_CATALOG.md references deleted apps ✓ COMPLETE (#1223)
**File:** `/Users/ayates/dashflow/docs/AI_PARTS_CATALOG.md`
**Status:** Updated 4 references
- Changed document_search_streaming example to librarian
- Updated multi-turn test examples reference
- Updated eval example reference to librarian
- Changed document_search stats to librarian

---

## Code Quality Statistics (Reference)

These are informational metrics for tracking code quality:

| Metric | Count | Files | Notes |
|--------|-------|-------|-------|
| `.unwrap()` in dashflow/src | 6,779 | 259 | Priority: Non-test code first |
| `.expect()` in dashflow/src | 441 | 57 | Better than unwrap but still panics |
| `#[allow(dead_code)]` | 56 files | - | Should audit |
| `#[deprecated]` items | ~50 | 17 crates | Normal for evolving API |
| `todo!()` macros | 0 actual | - | All in doc comments (OK) |
| `unimplemented!()` | 0 actual | - | All in doc comments (OK) |

---

## Summary

| Priority | Total | Completed | Remaining |
|----------|-------|-----------|-----------|
| P0 | 1 | 1 | 0 |
| P1 | 23 | 23 | 0 |
| P2 | 17 | 15 | 2 |
| P3 | 27 | 17 | 10 |
| M-71 | 174 calls | 143 converted | 31 (doc/test) |
| M-72 | ~10 calls | 9 | 1 (intentional) |
| M-73 | 401 calls | 0 | 401 |

**Completed: 61 issues + M-71 + M-72 migrations**
**Remaining: 10 issues (2 P2 + 8 P3) + M-73**

*Note: Issues 67-86 added and completed in extended audit (#1237-1258)*

---

## Worker Directive

**ALL P1 ISSUES COMPLETE!** Main documentation audit done. Extended audit continues.

### Next Priority: P2 Code Quality Issues

**Remaining P2 issues:**

1. **Issue 23** - Large source files - gradual refactoring
2. **Issue 24** - Excessive unwrap() usage - proper error handling

*Issues 36, 39, 42-45, 47, 48, 50-53 completed in #1218-1227*
*Issue 29 (prometheus-exporter metrics) completed in #1264*
*M-72 completed in #1218 (9/10 calls, 1 intentionally kept)*

### Lower Priority: P3 Issues

- **Issue 26** - Historical reports references
- **Issue 37** - #[allow(dead_code)] cleanup (56 files)
- **Issue 54** - Stub implementations in continuous_learning.rs
- **Issue 55** - Qdrant SPARSE/HYBRID modes not implemented
- **Issue 56** - Dev-dependency version inconsistency
- **Issue 57** - OpenAI Assistant incomplete features

*Issues 25, 38, 40, 41, 46 completed/resolved in M-72 and #1227-1235*

**M-73** - println! in dashflow core (401 calls) - TRACKED as M-407 in ROADMAP_CURRENT.md

**Both workspaces build with zero warnings.**

---

## NEW ISSUES - Extended Audit (Found by Worker #1228)

### Issue 54: Stub implementations in continuous_learning.rs (P3)
**File:** `crates/dashflow-evals/src/continuous_learning.rs`
**Lines:** 9, 254, 270, 282, 298-322

**Problem:** Contains stub implementations marked with TODO comments:
- `generate_from_failure_stub()` - line 299
- `generate_from_uncertainty_stub()` - line 322
- Module-level comment says "Stub implementation"

**Impact:** Incomplete feature implementation
**Fix:** Either implement fully or mark module as experimental

### Issue 55: Qdrant SPARSE/HYBRID modes not implemented (P3)
**File:** `crates/dashflow-qdrant/src/qdrant.rs`
**Count:** 11 "not yet implemented" comments for SPARSE/HYBRID mode

**Lines with "not yet implemented":**
- 1194, 1222, 1392, 2380, 2381, 2399, 2407, 2504, 2507, 2907, 2998, 3003

**Problem:** Features documented in API but return errors when used
**Impact:** Confusing API - appears to support modes it doesn't
**Fix:** Either implement or remove from public API

### Issue 56: Dev-dependency version inconsistency (P3)
**Files:** Multiple crate Cargo.toml files
**Count:** 15+ crates with `version = "1.0.0"` instead of workspace version

**Examples:**
- `dashflow-azure-openai/Cargo.toml:32` - `dashflow-langsmith = { version = "1.0.0" }`
- `dashflow-chroma/Cargo.toml:27` - `dashflow-openai = { version = "1.0.0" }`
- `dashflow-gemini/Cargo.toml:31` - `dashflow-standard-tests = { version = "1.0.0" }`

**Problem:** Dev-dependencies using hardcoded 1.0.0 instead of workspace.version
**Impact:** Version drift potential, inconsistent manifest
**Fix:** Use `version.workspace = true` for internal dev-dependencies

### Issue 57: OpenAI Assistant incomplete features (P3 - Info)
**File:** `crates/dashflow-openai/src/assistant.rs`
**Lines:** 536, 669

**Problem:** Features noted as "not yet implemented":
- Thread metadata (line 536)
- Tools override (line 669)

**Impact:** Incomplete LangChain parity
**Fix:** Track in roadmap for future implementation

---

## NEW ISSUES - Extended Audit (Found by Worker #1230)

### Issue 59: test_streaming_queries.sh obsolete ✓ COMPLETE (#1230)
**File:** `/Users/ayates/dashflow/test_streaming_queries.sh`
**Status:** Deleted - referenced deleted document_search_streaming app
- Script tested binary `./target/release/document_search_streaming` which no longer exists

### Issue 60: docker-compose.yml stale app references ✓ COMPLETE (#1230)
**File:** `/Users/ayates/dashflow/docker-compose.yml`
**Status:** Updated
- Removed health check endpoints for document_search_streaming, advanced_rag, code_assistant
- Removed commented container configurations for deleted apps
- Added historical note about app consolidation
- Updated example to use librarian

### Issue 61: PLAN_GUTENBERG_RAG.md references deleted app ✓ COMPLETE (#1230)
**File:** Moved to `/Users/ayates/dashflow/archive/plans/PLAN_GUTENBERG_RAG.md`
**Status:** Archived
- Plan file referenced advanced_rag app (26+ instances)
- No longer accurate since advanced_rag was consolidated into librarian

### Issue 62: PLAN_BOOK_SEARCH_PARAGON.md references deleted app ✓ COMPLETE (#1230)
**File:** Moved to `/Users/ayates/dashflow/archive/plans/PLAN_BOOK_SEARCH_PARAGON.md`
**Status:** Archived
- Plan file compared features to advanced_rag app
- Librarian already implements this plan - keeping as historical reference

### Issue 63: docs/PYTHON_PARITY_REPORT.md references deleted apps ✓ COMPLETE (#1233)
**File:** `/Users/ayates/dashflow/docs/PYTHON_PARITY_REPORT.md`
**Count:** 12+ references to streaming_aggregator, research_team, etc.
**Status:** Added historical note at top
- Technical findings remain valid since underlying DashFlow features unchanged

### Issue 64: docs/AI_PARTS_CATALOG.md still references deleted apps ✓ COMPLETE (#1234)
**File:** `/Users/ayates/dashflow/docs/AI_PARTS_CATALOG.md`
**Line:** 6090
**Status:** Updated example path from research_team/checkpoint_demo to librarian

### Issue 65: docs/COOKBOOK.md still references deleted apps ✓ COMPLETE (#1234)
**File:** `/Users/ayates/dashflow/docs/COOKBOOK.md`
**Line:** 628
**Status:** Updated checkpoint_demo link to reference checkpointer crates instead

### Issue 66: docs/RELEASE_NOTES_v1.10.0.md references deleted apps ✓ COMPLETE (#1236)
**File:** `/Users/ayates/dashflow/docs/RELEASE_NOTES_v1.10.0.md`
**Count:** 1 reference
**Status:** Added historical note at top

### Issue 67: run_optimization_tests.sh references deleted app ✓ COMPLETE (#1237)
**File:** `/Users/ayates/dashflow/run_optimization_tests.sh`
**Status:** Deleted
- Script tested `document_search_optimized` binary which no longer exists
- No replacement needed (optimization tests should use librarian if needed)

### Issue 68: playwright capture_screenshot.js references deleted app ✓ COMPLETE (#1238)
**File:** `crates/dashflow-evals/tests/playwright/capture_screenshot.js`
**Lines:** 8-9
**Status:** Updated paths from document_search to librarian

### Issue 69: analyze_validation_failures.py references deleted app ✓ COMPLETE (#1240)
**File:** `scripts/analyze_validation_failures.py`
**Line:** 215
**Status:** Updated path from document_search to librarian

### Issue 70: update_keywords_phase3.py references deleted app ✓ COMPLETE (#1240)
**File:** `scripts/update_keywords_phase3.py`
**Line:** 139
**Status:** Updated path from document_search to librarian

### Issue 71: update_latency_thresholds.py references deleted app ✓ COMPLETE (#1240)
**File:** `scripts/update_latency_thresholds.py`
**Lines:** 48, 60
**Status:** Updated paths from document_search to librarian

### Issue 72: grafana_dashboard.test.js references deleted app ✓ COMPLETE (#1238)
**File:** `test-utils/tests/grafana_dashboard.test.js`
**Line:** 9
**Status:** Updated command from advanced_rag to librarian

### Issue 73: scripts/VALIDATION_README.md obsolete ✓ COMPLETE (#1238)
**File:** `scripts/VALIDATION_README.md`
**Status:** Deleted (~320 lines)
- README documented validation scripts that were deleted in Issue 47
- Referenced validate_all_apps.sh, validate_document_search.sh, etc.

### Issue 74: COOKBOOK.md broken link to multi_model_comparison ✓ COMPLETE (#1243)
**File:** `docs/COOKBOOK.md`
**Line:** 758
**Status:** Updated link from deleted multi_model_comparison app to dashflow-evals crate

### Issue 75: docs/gpt4_vision_iterations/ contains stale references ✓ COMPLETE (#1246)
**Directory:** `docs/gpt4_vision_iterations/`
**Files:** 18 files referencing document_search
**Status:** Added historical note to README.md
- Historical N=93-97 iteration documentation
- Paths referencing `examples/apps/document_search/` now at `examples/apps/librarian/`
- Methodology and lessons learned remain valid

### Issue 76: examples/PHASE5_SUMMARY.md references deleted apps ✓ COMPLETE (#1246)
**File:** `examples/PHASE5_SUMMARY.md`
**Count:** 50+ references to document_search, advanced_rag, code_assistant
**Status:** Added historical note at top
- Historical Phase 5 validation documentation from N=1188-1194
- Apps consolidated into librarian
- Validation methodology and performance findings remain relevant

### Issue 77: docs/completed_phases/ contains stale references ✓ COMPLETE (#1246)
**Directory:** `docs/completed_phases/`
**Files:** Multiple files referencing document_search, code_assistant, advanced_rag
**Status:** Added historical note to README.md
- Archive of Phase 5-8 documentation
- Explains app consolidation into librarian

### Issue 78: docs/PHASE3_COMPLETION_SUMMARY.md references deleted app ✓ COMPLETE (#1246)
**File:** `docs/PHASE3_COMPLETION_SUMMARY.md`
**Lines:** 62, 272
**Status:** Added historical note at top
- References multi_model_comparison example app
- Functionality available in dashflow-evals crate

### Issue 79: docs/COMPLETED_INITIATIVES.md references deleted app ✓ COMPLETE (#1246)
**File:** `docs/COMPLETED_INITIATIVES.md`
**Line:** 21
**Status:** Added historical note at top
- References document_search app (accurate at time of writing)
- Explains apps were consolidated into librarian

### Issue 80: PLATFORM_AUDIT_150_ISSUES.md references deleted script ✓ COMPLETE (#1246)
**File:** `PLATFORM_AUDIT_150_ISSUES.md`
**Line:** 96
**Status:** Added note about moot issues
- References e2e_stack_validation.sh (deleted in Issue 47)
- Note clarifies some issues are now moot due to consolidation

### Issue 81: docs/completed_phases/README.md has outdated version ✓ COMPLETE (#1246)
**File:** `docs/completed_phases/README.md`
**Line:** 5
**Status:** Updated version from v1.10.0 to v1.11.x
- Workspace version is 1.11.3

### Issue 82: Duplicate WORKER_FIX_LIST files ✓ COMPLETE (#1246)
**Files:**
- `WORKER_FIX_LIST_1200.md` - Historical snapshot from worker #1200
- `WORKER_FIX_LIST_1207.md` - Historical snapshot about M-71 migration
**Status:** Archived to `archive/worker_fix_lists/`
- Content merged into main WORKER_FIX_LIST.md
- Files moved to archive for historical reference

### Issue 83: Stale root-level files ✓ COMPLETE (#1246)
**Files:**
- `WORKER_DIRECTIVE_PHASE20_BACKUP.md` - Backup directive from Dec 15
- `README_AI_INTROSPECTION_SECTION.md` - Draft README section never integrated
**Status:** Archived
- WORKER_DIRECTIVE_PHASE20_BACKUP.md → archive/worker_directives/
- README_AI_INTROSPECTION_SECTION.md → archive/drafts/

### Issue 84: 32 redundant health check reports ✓ COMPLETE (#1258)
**Directory:** `reports/main/HEALTH_CHECK_2025-12-12-*.md`
**Count:** 32 files from worker iterations N448-N481
**Status:** Archived to reports/main/archive_health_checks_2025-12-12/
- Each worker iteration generated a new health check report
- All from Dec 12 - excessive bloat for single day
- Archived to reduce clutter while preserving historical data

### Issue 85: Empty platform_lint directory ✓ COMPLETE (#1258)
**Directory:** `crates/dashflow/src/platform_lint/`
**Status:** Removed
- Empty directory with no references in codebase
- Created but never populated

### Issue 86: Stale target directory not gitignored ✓ COMPLETE (#1258)
**Directory:** `target_test_verify2/`
**Size:** 20GB
**Status:** Added `target_test_verify*/` pattern to .gitignore
- Stale build artifacts from earlier test run
- Should be manually deleted to free space
