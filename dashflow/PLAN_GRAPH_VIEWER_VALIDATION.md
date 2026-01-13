# Plan: StateGraph Viewer Rigorous Validation

**Created:** 2025-12-16
**Last Updated:** 2025-12-29 (Worker #2016 - M-306 COMPLETE)
**Purpose:** Skeptical, end-to-end validation of graph visualization features
**Status:** ASPIRATIONAL - This plan defines ideal coverage. See "Actual Test Coverage" section below for what exists.

---

## Actual Test Coverage (Added 2025-12-23)

The graph viewer has substantial real-world test coverage and has been production-hardened through fixes M-438 to M-455.

### What Actually Exists:

**UI Tests (`observability-ui/src/__tests__/`):**
- `mermaidRenderer.test.ts` (473 lines) - Comprehensive Mermaid rendering tests including:
  - Circle/DoubleCircle syntax validation (M-438, M-439 fixes)
  - XSS escape tests (7 tests for M-454 fix)
  - Edge case handling, node type rendering
- `jsonPatch.test.ts` - JSON patch operation tests
- `stateHash.test.ts` - Cross-language hash stability tests (M-91, M-92 fixes)
- `ErrorBoundary.test.tsx` - Error boundary component tests (M-455)

**Rust Tests:**
- `crates/dashflow/src/graph/tests.rs` - Graph structure tests
- `crates/dashflow/src/executor/tests.rs` - Execution tests
- `crates/dashflow/tests/end_to_end.rs` - E2E integration tests

**E2E Tests:**
- `test-utils/tests/live-graph-rigorous.spec.ts` - Live graph WebSocket tests

**Bug Fixes Completed:**
- M-438: Mermaid Circle syntax ✅
- M-439: Mermaid DoubleCircle syntax ✅
- M-440: Demo mode visual indicator ✅
- M-441: Mock event removal ✅
- M-449: Batch event data loss ✅
- M-450: Hash verification UI updates ✅
- M-451: WebSocket cleanup race ✅
- M-452: Unknown encoding error ✅
- M-453: Compression header disambiguation ✅
- M-454: Mermaid XSS prevention ✅
- M-455: Error boundaries ✅

### Coverage vs This Plan:

| Layer | Plan Status | Actual Coverage |
|-------|-------------|-----------------|
| 1. Model Definition | Unchecked | ✅ Partial - graph/tests.rs |
| 2. Compilation | Unchecked | ✅ Partial - executor/tests.rs |
| 3. Runtime Execution | Unchecked | ✅ E2E tests exist |
| 4. Trace Aggregation | Unchecked | ✅ COMPLETE #2014 - trace_analysis.rs tests (percentile, stats, aggregation) |
| 5. Serialization | Unchecked | ✅ mermaidRenderer.test.ts, stateHash.test.ts, debug.rs |
| 6. Rendering | Unchecked | ✅ mermaidRenderer.test.ts, E2E specs |

### Remaining Gaps (Low Priority):
- M-444: No React component tests for GraphCanvas, MermaidView, etc. (P2) - COMPLETE #2004
- M-446: No CLI watch command unit tests (P3) - COMPLETE #2003
- ~~Formal trace aggregation validation~~ - COMPLETE #2014

---

## Why Skepticism Is Required

Graph visualization is:
1. **Complex** - Multiple layers (model → runtime → aggregation → rendering)
2. **Novel** - Not many Rust frameworks do this well
3. **Easy to fake** - Static examples can look good but fail on real graphs
4. **Hard to test** - Visual correctness requires visual verification

**Without rigorous validation, we cannot claim this feature works.**

---

## Validation Layers

```
┌─────────────────────────────────────────────────────────────────┐
│                    GRAPH VIEWER PIPELINE                        │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  1. MODEL DEFINITION                                            │
│     StateGraph<S>::new()                                        │
│         .add_node("A", fn_a)                                    │
│         .add_edge("A", "B")                                     │
│         .add_conditional_edges("B", router, {...})              │
│                     │                                           │
│                     ▼                                           │
│  2. COMPILATION                                                 │
│     graph.compile() → CompiledGraph                             │
│         - Node registry populated                               │
│         - Edge map built                                        │
│         - Conditional routes resolved                           │
│                     │                                           │
│                     ▼                                           │
│  3. RUNTIME EXECUTION                                           │
│     graph.invoke(state) → Execution trace                       │
│         - Which nodes actually ran                              │
│         - What edges were traversed                             │
│         - Timing per node                                       │
│         - State at each step                                    │
│                     │                                           │
│                     ▼                                           │
│  4. TRACE AGGREGATION                                           │
│     TraceAnalyzer → Aggregated metrics                          │
│         - Node execution counts                                 │
│         - Average/p50/p99 latencies                             │
│         - Edge traversal frequencies                            │
│         - Bottleneck identification                             │
│                     │                                           │
│                     ▼                                           │
│  5. SERIALIZATION                                               │
│     GraphRenderer → Output format                               │
│         - Mermaid (text)                                        │
│         - DOT (Graphviz)                                        │
│         - JSON (structured)                                     │
│         - ASCII (terminal)                                      │
│                     │                                           │
│                     ▼                                           │
│  6. RENDERING                                                   │
│     Terminal/Browser/API                                        │
│         - Console ASCII art                                     │
│         - Web UI (live)                                         │
│         - API response                                          │
│         - Exported image                                        │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

---

## Layer 1: Model Definition Validation

### What to Test
- Node registration captures all metadata
- Edge definitions preserved (including conditional)
- START/END nodes handled correctly
- Parallel nodes marked as parallel

### Validation Script
```rust
#[test]
fn test_graph_structure_captured() {
    let graph = StateGraph::<TestState>::new()
        .add_node("classify", classify_fn)
        .add_node("search", search_fn)
        .add_node("generate", generate_fn)
        .add_edge(START, "classify")
        .add_conditional_edges("classify", router, hashmap! {
            "search" => "search",
            "direct" => "generate",
        })
        .add_edge("search", "generate")
        .add_edge("generate", END);

    let structure = graph.get_structure();

    // Verify all nodes present
    assert!(structure.nodes.contains("classify"));
    assert!(structure.nodes.contains("search"));
    assert!(structure.nodes.contains("generate"));

    // Verify edges
    assert!(structure.edges.contains(&(START, "classify")));
    assert!(structure.conditional_edges.contains_key("classify"));

    // Verify conditional routes
    let routes = &structure.conditional_edges["classify"];
    assert_eq!(routes.get("search"), Some(&"search"));
    assert_eq!(routes.get("direct"), Some(&"generate"));
}
```

### Success Criteria
- [ ] All nodes captured with names
- [ ] All edges captured (including START/END)
- [ ] Conditional edges captured with route names
- [ ] Node metadata (description) preserved

---

## Layer 2: Compilation Validation

### What to Test
- CompiledGraph has complete node/edge info
- No information lost during compilation
- Can reconstruct graph structure from compiled form

### Validation Script
```rust
#[test]
fn test_compiled_graph_structure_preserved() {
    let graph = build_test_graph();
    let compiled = graph.compile().unwrap();

    // Can we get structure back?
    let structure = compiled.get_structure();

    assert_eq!(structure.node_count(), 3);
    assert_eq!(structure.edge_count(), 4);  // including conditional

    // Export to mermaid should work
    let mermaid = compiled.to_mermaid().unwrap();
    assert!(mermaid.contains("classify"));
    assert!(mermaid.contains("search"));
    assert!(mermaid.contains("generate"));
}
```

### Success Criteria
- [ ] `compiled.get_structure()` returns complete info
- [ ] Node count matches definition
- [ ] Edge count matches definition
- [ ] `to_mermaid()` exports without error

---

## Layer 3: Runtime Execution Validation

### What to Test
- Execution traces capture actual path taken
- Node timing is accurate
- Conditional routing is recorded
- Parallel execution is marked

### Validation Script
```rust
#[test]
async fn test_execution_trace_accurate() {
    let graph = build_test_graph();
    let compiled = graph.compile().unwrap();

    // Execute with tracing enabled
    let config = RunnableConfig::default()
        .with_tracing(TracingConfig::default());

    let (result, trace) = compiled.invoke_with_trace(initial_state, config).await?;

    // Verify trace captures execution
    assert!(trace.nodes_executed.contains("classify"));
    assert!(trace.nodes_executed.contains("generate"));

    // Verify timing captured
    for node in &trace.nodes_executed {
        let timing = trace.get_node_timing(node);
        assert!(timing.duration_ms > 0);
        assert!(timing.start_time < timing.end_time);
    }

    // Verify conditional route recorded
    assert_eq!(trace.route_taken("classify"), Some("search"));
}
```

### Success Criteria
- [ ] All executed nodes in trace
- [ ] Node timing is non-zero and sensible
- [ ] Conditional routes recorded
- [ ] Parallel execution marked

---

## Layer 4: Trace Aggregation Validation

### What to Test
- Multiple executions aggregate correctly
- Statistics (avg, p50, p99) calculated properly
- Bottleneck identification works
- Edge traversal counts accurate

### Validation Script
```rust
#[test]
async fn test_trace_aggregation() {
    let graph = build_test_graph();
    let compiled = graph.compile().unwrap();

    // Run multiple times
    let mut traces = Vec::new();
    for _ in 0..10 {
        let (_, trace) = compiled.invoke_with_trace(state.clone(), config.clone()).await?;
        traces.push(trace);
    }

    // Aggregate
    let analyzer = TraceAnalyzer::new();
    let stats = analyzer.aggregate(&traces)?;

    // Verify aggregation
    assert_eq!(stats.execution_count, 10);

    for node in &["classify", "search", "generate"] {
        let node_stats = stats.get_node_stats(node);
        assert!(node_stats.avg_duration_ms > 0.0);
        assert!(node_stats.p50_duration_ms > 0.0);
        assert!(node_stats.p99_duration_ms >= node_stats.p50_duration_ms);
        assert_eq!(node_stats.execution_count, 10);
    }

    // Verify bottleneck detection
    let bottleneck = stats.slowest_node();
    assert!(bottleneck.is_some());
}
```

### Success Criteria
- [ ] Execution count matches runs
- [ ] Avg/p50/p99 calculated for each node
- [ ] p99 >= p50 >= avg (statistical sanity)
- [ ] Bottleneck identified correctly

---

## Layer 5: Serialization Validation

### What to Test
- Mermaid output is valid Mermaid syntax
- DOT output is valid Graphviz syntax
- JSON output has all required fields
- ASCII output renders in terminal

### Validation Tests

#### Mermaid Validation
```rust
#[test]
fn test_mermaid_valid_syntax() {
    let graph = build_test_graph().compile().unwrap();
    let mermaid = graph.to_mermaid().unwrap();

    // Basic syntax checks
    assert!(mermaid.starts_with("graph") || mermaid.starts_with("flowchart"));
    assert!(mermaid.contains("-->"));  // Has edges

    // Node names present
    assert!(mermaid.contains("classify"));
    assert!(mermaid.contains("search"));
    assert!(mermaid.contains("generate"));

    // Conditional syntax correct
    assert!(mermaid.contains("|"));  // Conditional label syntax

    // Can parse with mermaid-cli (if available)
    // mmdc -i graph.mmd -o graph.svg should succeed
}
```

#### ASCII Validation
```rust
#[test]
fn test_ascii_renders_correctly() {
    let graph = build_test_graph().compile().unwrap();
    let ascii = graph.to_ascii().unwrap();

    // Has box characters
    assert!(ascii.contains("┌") || ascii.contains("+"));
    assert!(ascii.contains("│") || ascii.contains("|"));

    // Has arrows
    assert!(ascii.contains("→") || ascii.contains("->") || ascii.contains("▼"));

    // Has node names
    assert!(ascii.contains("classify"));
    assert!(ascii.contains("search"));
    assert!(ascii.contains("generate"));

    // Renders without panic in terminal
    println!("{}", ascii);
}
```

#### JSON Validation
```rust
#[test]
fn test_json_complete() {
    let graph = build_test_graph().compile().unwrap();
    let json = graph.to_json().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

    // Has required fields
    assert!(parsed.get("nodes").is_some());
    assert!(parsed.get("edges").is_some());

    // Nodes have required fields
    let nodes = parsed["nodes"].as_array().unwrap();
    for node in nodes {
        assert!(node.get("id").is_some());
        assert!(node.get("name").is_some());
    }

    // Edges have required fields
    let edges = parsed["edges"].as_array().unwrap();
    for edge in edges {
        assert!(edge.get("from").is_some());
        assert!(edge.get("to").is_some());
    }
}
```

### Success Criteria
- [ ] Mermaid parses with mermaid-cli
- [ ] DOT parses with Graphviz
- [ ] JSON parses with serde_json
- [ ] ASCII renders without panic

---

## Layer 6: Rendering Validation

### Terminal ASCII Viewer

**Manual verification required with screenshot:**

```bash
cargo run -p librarian -- graph --view
```

Expected output:
```
┌─────────────────────────────────────────────────────────────────┐
│                    Superhuman Librarian Graph                    │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  [START] ─────────────────────────────────────────────────────  │
│      │                                                           │
│      ▼                                                           │
│  ┌─────────────────┐                                            │
│  │ understand_query │                                            │
│  └────────┬────────┘                                            │
│           │                                                      │
│     ┌─────┴─────┐                                               │
│     ▼           ▼                                               │
│  ┌──────┐   ┌──────┐   PARALLEL                                 │
│  │ sem  │   │ kw   │                                            │
│  └──┬───┘   └──┬───┘                                           │
│     └─────┬────┘                                                │
│           ▼                                                      │
│  ┌───────────────┐                                              │
│  │ merge_results │                                              │
│  └───────┬───────┘                                              │
│          ▼                                                       │
│      [END]                                                       │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

**Validation checklist:**
- [ ] Screenshot captured: `reports/graph_viewer_ascii.png`
- [ ] All nodes visible
- [ ] Edges connect correct nodes
- [ ] Parallel nodes shown side-by-side
- [ ] START/END clearly marked
- [ ] No rendering artifacts

### Mermaid Export

**Manual verification required:**

```bash
cargo run -p librarian -- graph --export mermaid > /tmp/librarian.mmd
```

Then render with mermaid-cli or online:
```bash
mmdc -i /tmp/librarian.mmd -o /tmp/librarian.svg
# OR paste into https://mermaid.live
```

**Validation checklist:**
- [ ] Screenshot captured: `reports/graph_viewer_mermaid.png`
- [ ] Mermaid syntax valid (no parse errors)
- [ ] All nodes rendered
- [ ] Edges connect correctly
- [ ] Conditional edges have labels
- [ ] Visually matches expected structure

### Web UI (Live Viewer)

**Manual verification required:**

```bash
cargo run -p librarian -- graph --live
# Opens http://localhost:8765
```

**Validation checklist:**
- [ ] Screenshot captured: `reports/graph_viewer_web.png`
- [ ] Server starts without error
- [ ] Browser opens to correct URL
- [ ] Graph renders in browser
- [ ] Nodes are clickable
- [ ] Execution highlighting works (if running query)
- [ ] Zoom/pan works

### Visual LLM Verification

**Use Claude vision to verify screenshots:**

For each screenshot, ask:
```
"Does this graph visualization correctly show:
1. All nodes labeled clearly
2. Edges connecting the right nodes
3. Conditional routing visible
4. Parallel execution indicated
5. START and END points clear
6. No visual artifacts or overlapping text

Rate accuracy 1-10 and list any issues."
```

**Validation checklist:**
- [ ] ASCII viewer: Score >= 8/10
- [ ] Mermaid export: Score >= 8/10
- [ ] Web UI: Score >= 8/10

---

## API Method Evaluation

### Current Options

| Method | Pros | Cons |
|--------|------|------|
| `to_mermaid()` | Standard format, many renderers | Text only, needs external render |
| `to_dot()` | Graphviz ecosystem, flexible | Requires Graphviz installed |
| `to_json()` | Programmatic access, flexible | Not human-readable |
| `to_ascii()` | Works anywhere, no deps | Limited layout options |
| Web UI | Interactive, rich | Requires server, browser |

### Recommendation

**Best approach: Multiple outputs for different use cases**

```rust
impl CompiledGraph<S> {
    /// Export graph structure as Mermaid diagram
    pub fn to_mermaid(&self) -> Result<String>;

    /// Export graph structure as Graphviz DOT
    pub fn to_dot(&self) -> Result<String>;

    /// Export graph structure as JSON
    pub fn to_json(&self) -> Result<String>;

    /// Render graph as ASCII art for terminal
    pub fn to_ascii(&self) -> Result<String>;

    /// Start live web viewer
    pub async fn start_live_viewer(&self, port: u16) -> Result<LiveViewer>;
}
```

**CLI should support all:**
```bash
librarian graph --view          # ASCII in terminal
librarian graph --export mermaid
librarian graph --export dot
librarian graph --export json
librarian graph --live          # Web UI
```

---

## Execution Plan

### Phase 1: API Existence Check (1 commit)
- [ ] Verify `to_mermaid()` exists and compiles
- [ ] Verify `to_dot()` exists or document as missing
- [ ] Verify `to_json()` exists and compiles
- [ ] Verify `to_ascii()` exists or document as missing
- [ ] Verify `start_live_viewer()` exists or document as missing

### Phase 2: Unit Tests (2-3 commits)
- [ ] Layer 1 tests: Model definition capture
- [ ] Layer 2 tests: Compilation preservation
- [ ] Layer 3 tests: Runtime trace accuracy
- [ ] Layer 4 tests: Aggregation correctness
- [ ] Layer 5 tests: Serialization validity

### Phase 3: Integration Tests (1-2 commits) ✅ COMPLETE #2015
- [x] End-to-end: Define → Compile → Execute → Export → Render
- [x] Test with simple graph (3 nodes) - `test_simple_graph_full_pipeline`
- [x] Test with complex graph (10+ nodes, conditionals, parallels) - `test_complex_graph_full_pipeline` (12 nodes)
- [x] Test with Librarian-style graph (real-world RAG pipeline) - `test_librarian_style_graph_full_pipeline`
- [x] Additional: Mermaid config variations, export format consistency, minimal graph edge cases

### Phase 4: Visual Verification (1 commit) ✅ COMPLETE #2016
- [x] Visual verification test harness with scoring system (6 tests)
- [x] Automated file output to `target/visual-verification/` (graph.md, graph.mmd, graph.dot, graph.txt, VERIFICATION_REPORT.md)
- [x] VisualScore struct with 6 criteria: nodes_visible, edges_correct, conditional_labels, parallel_indicated, start_end_marked, no_artifacts
- [x] All formats score 10/10 on 5-node RAG pipeline test graph
- [x] Complex 12-node pipeline also achieves 10/10 across all formats
- [x] Edge cases (empty, single node, linear chain) all pass validation

**Note:** Implemented as automated test harness rather than manual screenshots. This is better because:
- Repeatable verification on every test run
- Quantified scoring (not subjective LLM evaluation)
- File outputs available for manual inspection when needed
- CI-friendly (no manual steps required)

### Phase 5: Fix Issues (variable)
- [x] No issues found - all formats score 10/10
- [x] All exports produce valid syntax (Mermaid, DOT, ASCII)
- [x] No rendering artifacts detected

---

## Success Criteria

### Must Pass (P0)
- [ ] All Layer 1-5 unit tests pass
- [ ] At least one export format works end-to-end
- [ ] Visual LLM scores >= 7/10 for at least one format
- [ ] No crashes or panics during rendering

### Should Pass (P1)
- [ ] All export formats work (Mermaid, DOT, JSON, ASCII)
- [ ] Visual LLM scores >= 8/10 for all formats
- [ ] Complex graphs (10+ nodes) render correctly
- [ ] Conditional edges have visible labels

### Nice to Have (P2)
- [ ] Web UI live viewer works
- [ ] Execution replay animation works
- [ ] Latency heatmap in visualization

---

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Export methods don't exist | Medium | High | Check codebase first, implement if missing |
| Mermaid syntax invalid | Low | Medium | Test with mermaid-live before claiming |
| ASCII layout broken | Medium | Medium | Test with various terminal widths |
| Web UI doesn't start | Medium | High | Web UI is complex, may defer to P2 |
| Complex graphs unreadable | High | Medium | May need layout algorithm improvements |

---

## Timeline

| Day | Task | Deliverable |
|-----|------|-------------|
| 1 | API existence check | List of what exists vs missing |
| 2-3 | Unit tests | Tests for Layers 1-5 |
| 4 | Integration tests | End-to-end validation |
| 5 | Visual verification | Screenshots + LLM scores |
| 6+ | Fix issues | Working graph viewer |

---

## Appendix: Test Graph Definition

```rust
fn build_test_graph() -> StateGraph<TestState> {
    StateGraph::<TestState>::new()
        .add_node_with_metadata("classify", classify_fn, NodeMetadata {
            description: "Classify query intent",
        })
        .add_node_with_metadata("search", search_fn, NodeMetadata {
            description: "Search vector store",
        })
        .add_node_with_metadata("web", web_fn, NodeMetadata {
            description: "Search web",
        })
        .add_node_with_metadata("generate", generate_fn, NodeMetadata {
            description: "Generate response",
        })
        .add_edge(START, "classify")
        .add_conditional_edges("classify", route_classify, hashmap! {
            "vectorstore" => "search",
            "websearch" => "web",
        })
        .add_edge("search", "generate")
        .add_edge("web", "generate")
        .add_edge("generate", END)
}
```

This tests:
- Linear edges (START → classify)
- Conditional edges (classify → search OR web)
- Converging edges (search/web → generate)
- Terminal edge (generate → END)
