# N=97 Critical Discovery: Screenshot Timing Issue

**Date:** November 17, 2025
**Time:** 20:40 UTC (3:40 AM PST)
**Iteration:** 97
**Status:** EVALUATION IN PROGRESS

## Problem Discovered

The GPT-4 vision feedback collected in N=96/N=97 was based on a screenshot created BEFORE the N=96 improvements were implemented.

### Timeline Evidence

```
eval_report.html:         2025-11-17 19:11:39 (7:11 PM PST)
eval_report_screenshot:   2025-11-17 19:14:09 (7:14 PM PST)
N=96 commit (b1484b5a6):  2025-11-17 19:37:30 (7:37 PM PST) ← 26 minutes AFTER
```

### Impact

The GPT-4 feedback in `ITERATION4_GPT4_FEEDBACK.txt` **does not reflect N=96 improvements**. It identifies the exact same issues that N=96 claimed to fix:

1. **Whitespace** - N=96 reduced padding/margins, GPT-4 still sees "excessive whitespace" (Critical)
2. **Color contrast** - N=96 added high-contrast colors, GPT-4 says "lacks contrast" (High)
3. **Executive summary** - N=96 added 6 insights, GPT-4 says "lacks clear overview" (Medium)
4. **Recommendations linkage** - N=96 added data links, GPT-4 says "not linked" (Medium)
5. **Typography** - N=96 standardized, GPT-4 says "inconsistent" (Medium)

## Current Actions

### Completed
- ✅ Identified timing issue
- ✅ Rebuilt binary with N=96 changes (already up-to-date)
- ⏳ Running fresh evaluation (background shell f8d0d6)

### In Progress
- Evaluation running since 03:42 UTC (5 minutes elapsed as of 03:47 UTC)
- Output: `/tmp/eval_n97_output.txt`
- Status: "🧪 Running evaluations..." (expected 5-10 min total for 50 scenarios)

### Next Steps
1. Wait for evaluation to complete (~5 more minutes)
2. Capture new screenshot with Playwright
3. Collect fresh GPT-4 vision feedback
4. Assess if N=96 improvements are visible and effective
5. Determine score and whether 8-9/10 target achieved

## Lessons

**Screenshot Timing is Critical:**
- Must regenerate screenshot AFTER code changes and build
- Cannot rely on existing screenshots from previous iterations
- Process: Code change → Build → Eval → Screenshot → GPT-4 feedback

**N=96 Session Gap:**
- N=96 implemented improvements but did not verify them with fresh evaluation
- Left N=97 to discover the validation gap
- Should have run full cycle: Implement → Test → Verify → Commit

## Background Processes

- `f8d0d6`: Evaluation running (started 03:42 UTC)
  - Command: `OPENAI_API_KEY="..." cargo run --bin eval --release --package document_search`
  - Output: `/tmp/eval_n97_output.txt`

## Files

- `ITERATION4_GPT4_FEEDBACK.txt` - GPT-4 feedback on OLD screenshot (before N=96)
- `N96_DESIGN_IMPROVEMENTS.md` - N=96 implementation details
- `N96_SESSION_STATUS.md` - N=96 session notes (outdated shell IDs)

## Context for Next Worker

If evaluation completes:
1. Check output: `tail -100 /tmp/eval_n97_output.txt`
2. Verify report generated: `ls -lth examples/apps/document_search/outputs/`
3. Capture screenshot: `node crates/dashflow-evals/tests/playwright/capture_screenshot.js`
4. Get GPT-4 feedback: `OPENAI_API_KEY="..." python3 scripts/python/vision_critique.py examples/apps/document_search/outputs/eval_report_screenshot.png > ITERATION5_GPT4_FEEDBACK.txt`
5. Compare ITERATION5 feedback to ITERATION4 to see if N=96 improvements are visible

If evaluation hangs:
- Check process: `ps aux | grep eval`
- Check API key in environment
- Try running directly (not in background)
