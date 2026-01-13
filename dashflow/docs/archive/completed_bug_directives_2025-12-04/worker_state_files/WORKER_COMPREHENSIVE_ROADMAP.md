# Worker Comprehensive Roadmap - Complete Rebranding & DashOptimize

**Date:** 2025-12-03 16:17
**Priority Order:** MANDATORY → HIGH → MEDIUM
**Total Time:** 50 min mandatory + 12-18 hours for DashOptimize features

---

## 🔴 MANDATORY: Complete Rebranding (45 minutes)

**Status:** INCOMPLETE
**Priority:** ABSOLUTE - Must finish before ANY other work

### Task 1: DashStream → DashStream Rebranding

**Current State:**
- ❌ 5 files named "dashstream_*"
- ❌ 46 Kafka topic names use "dashstream-events"
- ❌ ~500 comment references to "DashStream"

**Execution:** See FINAL_ABSOLUTE_DIRECTIVE.md (5 steps, 45 min)

**Success Criteria:**
- [ ] All files renamed to dashstream
- [ ] All topics use "dashstream-events"
- [ ] Comments use "DashStream"
- [ ] `rg -i dashstream` shows minimal results

---

## 🟡 HIGH PRIORITY: DashOptimize Integration (12-18 hours)

**Source:** DASHOPT_INTEGRATION_PLAN.md (from another AI)

**Priority after rebranding complete.**

### Task 2: Add dashopt_types Module (4-6 hours)

**Purpose:** Enable optimization of multimodal workflows

**Types to add:**
```
dashflow/src/optimize/types/
├── mod.rs
├── audio.rs       # Audio inputs for audio-capable LLMs
├── citation.rs    # Source citations for RAG (Anthropic Citations API)
├── code.rs        # Code generation with language tagging
├── document.rs    # Citation-enabled documents
├── file.rs        # File inputs (PDFs) with base64 encoding
├── history.rs     # Conversation history container
├── image.rs       # Image inputs for vision models
├── reasoning.rs   # Native reasoning (o1-series models)
└── tool.rs        # ToolCall/ToolCalls for function calling
```

**Value:**
- Multimodal optimization (images, audio)
- Native o1 model support (reasoning_effort)
- Production RAG with citations
- Agentic workflow optimization

**Implementation:**
1. Create types/ directory
2. Port each type from ~/dsp_rs/dashopt_types/src/
3. Add to Signature system
4. Write tests for each
5. Update documentation

**Estimated:** 4-6 hours

---

### Task 3: Unified CLI (8-12 hours)

**Purpose:** Single `dashflow` CLI for all operations

**Current State:**
- ✅ `dashstream-cli` exists (streaming: tail, inspect, replay, etc.)
- ❌ No CLI for optimization, evaluation, training

**Proposal:** Rename `dashstream-cli` → `dashflow-cli` and add commands:

```
dashflow <command>

Streaming Commands (existing):
  tail        Stream live events
  inspect     Show thread details
  replay      Replay execution
  diff        Compare checkpoints
  export      Export to JSON
  flamegraph  Performance visualization
  costs       Token cost analysis
  profile     Performance profiling

Optimization Commands (new):
  optimize    Run optimization
    --graph <path>      Graph definition
    --trainset <path>   Training data
    --optimizer <name>  bootstrap|simba|gepa|mipro|grpo|copro
    --metric <name>     accuracy|f1|custom
    --output <path>     Save optimized graph

  eval        Run evaluation
    --graph <path>      Graph definition
    --testset <path>    Test data
    --output <path>     Evaluation results

  train       Training/fine-tuning
    --type <type>       distillation|finetune
    --config <path>     Training config

  dataset     Dataset utilities
    --generate          Generate synthetic data
    --validate          Validate format
    --stats             Show statistics
```

**Implementation:**
1. Rename crate: dashstream-cli → dashflow-cli
2. Rename binary: dashstream → dashflow
3. Create commands/ modules for optimize, eval, train, dataset
4. Keep all existing streaming commands
5. Update documentation

**Estimated:** 8-12 hours

---

## 🟢 LOW PRIORITY: Future Considerations

### Skip dashopt_adapters

**Reason:** DashFlow handles prompt formatting inline in `llm_node.rs`

**If needed later:** Can add JSON/XML output adapters for structured parsing

---

### Data Balancing/Normalizing

**Clarification:** Already architecturally correct in `distillation/synthetic.rs`

**No action required.**

---

## 📋 EXECUTION ORDER (STRICT)

### 1. FIRST: Complete Rebranding (45 min)
**Worker N=40 (Next):**
- Fix dashstream rebranding (45 min)
- Verify completion
- Create completion marker commit

**Only after this is 100% done, proceed to #2.**

---

### 2. SECOND: DashOptimize Types (4-6 hours)
**Worker N=41+ (After rebranding):**
- Port 9 types from dsp_rs/dashopt_types
- Integrate with Signature
- Test each type
- Document usage

---

### 3. THIRD: Unified CLI (8-12 hours)
**Worker N=42+ (After types):**
- Rename dashstream-cli → dashflow-cli
- Add optimize command
- Add eval command
- Add train command
- Add dataset command
- Test all commands
- Update docs

---

## ⏱️ TOTAL TIMELINE

**Immediate (Mandatory):** 45 minutes - DashStream rebrand
**Phase 1 (High Priority):** 4-6 hours - Types module
**Phase 2 (High Priority):** 8-12 hours - Unified CLI
**Total:** ~13-19 hours for complete DashOptimize integration

---

## ✅ SUCCESS CRITERIA

### Rebranding Complete (Mandatory):
- [ ] Zero "dashstream" file names
- [ ] All topics use "dashstream"
- [ ] Comments use "DashStream"
- [ ] Zero compilation errors
- [ ] All tests pass

### DashOptimize Integration Complete (After rebranding):
- [ ] 9 types ported to dashflow/src/optimize/types/
- [ ] Types integrated with Signature system
- [ ] Unified `dashflow` CLI with 12+ commands
- [ ] All commands tested and working
- [ ] Documentation updated

---

## 🎯 WORKER N=40: YOUR ORDERS

**Step 1:** Execute FINAL_ABSOLUTE_DIRECTIVE.md (45 min)
**Step 2:** Create completion marker commit
**Step 3:** THEN Worker N=41 can start on dashopt_types

**DO NOT skip Step 1. User explicitly requested dashstream rebranding.**

**DO NOT start DashOptimize work until rebranding 100% complete.**

---

## 📊 SOURCES

- FINAL_ABSOLUTE_DIRECTIVE.md (my directive)
- DASHOPT_INTEGRATION_PLAN.md (other AI's plan)
- User's explicit request for dashstream rebranding
- Rigorous gap analysis

---

**Priority:** MANDATORY rebranding first, then high-value DashOptimize features.

**No shortcuts. No "good enough." Complete everything.**
