# Next Session: GPT-4 Vision Critique + Design Improvements

**Priority:** HIGH - User Requested
**Context Required:** ~15-20% (fresh session recommended)
**User Mandate:** "Use LLM as Judge to visually inspect reports. Identify 10 improvements. Execute them."

---

## What's Ready for Critique

**Files to Analyze:**
1. **Screenshot:** examples/apps/document_search/outputs/eval_report_screenshot.png (866KB, 1280x7025px)
2. **HTML Source:** examples/apps/document_search/outputs/eval_report.html (297KB)
3. **JSON Data:** examples/apps/document_search/outputs/eval_report.json (28KB)
4. **SVG Charts:** 3 files (eval_quality.svg, eval_latency.svg, eval_pass_fail.svg)

**Browser Windows Already Open:** User can see current design

---

## Task: GPT-4 Vision Analysis

### Step 1: Send Screenshot to GPT-4 Vision

```python
# Use: examples/apps/document_search/outputs/eval_report_screenshot.png
# Model: gpt-4o (with vision)
# Prompt: "You are an expert UX designer and data visualization specialist..."
```

**Ask GPT-4:**
- Analyze evaluation report screenshot
- Find 10 specific design improvements
- Focus on: information density, visual hierarchy, data storytelling, professional polish, statistical rigor
- Output as structured JSON with priority, category, problem, solution, implementation

### Step 2: Categorize Improvements

**Expected categories:**
1. **Information Density** - Show more data per screen
2. **Visual Design** - Colors, typography, layout
3. **Data Storytelling** - Narrative flow, insights prominence
4. **Statistical Rigor** - Confidence intervals, distributions, trends
5. **Interactivity** - Filtering, sorting, drill-down

### Step 3: Prioritize

**Critical (Must Do):**
- Information density issues
- Data clarity problems
- Missing key insights

**High (Should Do):**
- Visual polish
- Statistical details
- Better charts

**Medium (Nice to Have):**
- Advanced interactions
- Additional metrics

---

## Implementation Plan (10 Improvements)

### For Each Improvement:

**1. Update HTML Generator**
- File: `crates/dashflow-evals/src/report/html.rs`
- Add new sections/elements
- Enhance existing displays

**2. Update CSS**
- File: Same (embedded CSS in HTML generator)
- Add styles for new elements
- Refine existing styles

**3. Update Chart Generation**
- File: `crates/dashflow-evals/src/report/charts.rs`
- Add new chart types
- Enhance existing charts

**4. Test Changes**
```bash
# Re-run evaluation
cargo run --bin eval --package document_search

# Validate with Playwright
node crates/dashflow-evals/tests/playwright/validate_report.js

# Compare screenshots (before/after)
```

**5. Validate with GPT-4 Vision Again**
- Send new screenshot
- Ask: "Are these improvements effective?"
- Iterate if needed

---

## Expected Improvements (Educated Guesses)

### Information Density
1. Add per-dimension quality breakdown table
2. Show failure patterns (which keywords missing most often?)
3. Add quality distribution by category
4. Include latency percentiles (P50, P90, P95, P99)

### Visual Design
5. Improve color palette (Dropbox brand colors?)
6. Better typography hierarchy
7. Add visual separators between sections

### Data Storytelling
8. Add executive summary (top insights)
9. Show trends (quality over time if historical data)
10. Highlight critical failures (low safety scores, hallucinations)

---

## Success Criteria

**After improvements:**
- ✅ More information visible without scrolling
- ✅ Key insights immediately obvious
- ✅ Professional Dropbox-quality design
- ✅ Statistical rigor (confidence intervals, distributions)
- ✅ Compelling narrative (tells a story about quality)
- ✅ GPT-4 vision approves improvements
- ✅ Playwright tests still pass
- ✅ User approves final design

---

## Files to Modify

**Core:**
- crates/dashflow-evals/src/report/html.rs (~500 lines, will grow to ~800)
- crates/dashflow-evals/src/report/charts.rs (~600 lines, will grow to ~900)

**Possible New Files:**
- crates/dashflow-evals/src/report/insights.rs (auto-generate insights)
- crates/dashflow-evals/src/report/statistics.rs (confidence intervals, distributions)

**Tests:**
- Update Playwright validation for new elements
- Add screenshot comparison tests

---

## Estimated Effort

**GPT-4 Vision Critique:** 5-10 minutes (API call + parse results)
**Implementation:** 10 improvements × 15-30 minutes = 2.5-5 hours
**Testing:** 30-60 minutes (re-run eval, validate, iterate)
**Total:** 3-6 hours

**Context:** 15-20% (need fresh session)

---

## Quick Start for Next Session

```bash
# 1. Install OpenAI Python
pip3 install --break-system-packages openai

# 2. Run vision critique
python3 /tmp/vision_critique.py examples/apps/document_search/outputs/eval_report_screenshot.png > gpt4_critique.json

# 3. Parse results
cat gpt4_critique.json | jq '.improvements[]'

# 4. Implement improvements one by one
# Start with Critical priority

# 5. Re-generate report
cargo run --bin eval --package document_search

# 6. Validate
node crates/dashflow-evals/tests/playwright/validate_report.js

# 7. Compare screenshots
# Send both to GPT-4 vision: "Which is better?"
```

---

## Current Session Accomplishments

**To recap what we proved:**
- ✅ Gap #1 fixed (parallel merging)
- ✅ LLM-as-judge works (27 API tests)
- ✅ Adversarial detection (95%)
- ✅ 50-scenario eval completed
- ✅ Reports generated and validated
- ✅ Playwright automation working
- ✅ Screenshot captured

**Ready for:** Design improvements via GPT-4 vision

---

**Next Worker: Read this file, install OpenAI properly, run vision critique, implement 10 improvements.**
