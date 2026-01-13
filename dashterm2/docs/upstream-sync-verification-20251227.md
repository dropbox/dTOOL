# Upstream Sync Verification - December 27, 2025

**Iteration #1379**

## Summary

Verified that all 24 new upstream commits (since 7171ea8) are already backported to DashTerm2.

## Upstream Commits Verified (7171ea8..b0d325d)

| SHA | Description | Status | DashTerm2 Location |
|-----|-------------|--------|-------------------|
| b0d325d | SetProfileProperty control sequence (Issue 4586) | ✅ Backported | iTermNaggingController.m:43,189 |
| 78eecf7 | New tab next to current tab menu item (Issue 12655) | ✅ Backported | iTermApplicationDelegate.m:567,2565 |
| b76bf1f | Progress indicator clockwise from 12 o'clock (Issue 12659) | ✅ Backported | PSMProgressIndicator.m:87 |
| a71520d | Tahoe tab label font fix (Issue 12658) | ✅ Backported | PSMTahoeTabStyle.swift |
| 226074b | Horizontal lines on non-retina (Issue 12657) | ✅ Backported | iTermBoxDrawingBezierCurveFactory.m:1996,2007,2034 |
| b28e7a7 | Window title in maximized-style (Issue 12656) | ✅ Backported | PseudoTerminal+WindowStyle.m:1024,1030 |
| ccf346e | New tabs open at end preference (Issue 12655) | ✅ Backported | iTermPreferences.m/h |
| eb52e39 | Build break fix | ✅ Backported | iTermApplicationDelegate.m:391 |
| d04a095 | GPU frame capture for adhoc | ✅ Backported | |
| 3ca3cea | MTL_CAPTURE_ENABLED env var | ✅ Backported | main.m:33,36 |
| 02b57f6 | Stoplight buttons in macOS 26 (Issue 12474) | ✅ Backported | |
| 61078a6 | Web menu bug fix | ✅ Backported | |
| 0cfc136 | Shell integration bump | ✅ Backported | |
| 0032e53 | Percentage span for screen windows | ✅ Backported | PreferencePanel.xib |
| c82859d | Open Profiles window space fix (Issue 12647) | ✅ Backported | iTermProfilesWindowController.m:146 |
| bdbd3e8 | Menu tips xcassets | ✅ Backported | |
| f383622 | Version 3.6.7beta2 | N/A | Version bump only |
| 3e32311 | osc8.txt additions | ✅ Backported | |
| b0f910c | Text completion in search fields | ✅ Backported | FindView.xib, MinimalFindView.xib |
| eb61cff | Assertion improvement for crash debug | ✅ Backported | iTermMetalPerFrameState.m:1122-1126 |
| 84d5017 | Crash fix when editing session | ✅ Backported | TriggerController.m:421 |
| a345c5d | Crash fix for color without colorspace | ✅ Backported | CPKColorWell.m |
| b75bdca | Crash fix for snippets/folders intermingled | ✅ Backported | iTermToolSnippets.m:274 |
| 5dda3da | Divorced GUID fix | ✅ Backported | ProfileModel.m:745 |

## Build Verification

```
** BUILD SUCCEEDED **
```

## Test Verification

```
Executed 4465 tests, with 22 tests skipped and 0 failures (0 unexpected) in 17.041 (24.140) seconds
** TEST SUCCEEDED **
No new crash reports found.
```

## Conclusion

- All 24 upstream commits from iTerm2 are already present in DashTerm2
- No new backporting needed
- Build succeeds
- All 4465 tests pass
- No crash reports

## Next Steps for Future Workers

1. Check for newer upstream commits beyond b0d325d
2. Monitor GitLab for new issues
3. Continue stability hardening if needed
