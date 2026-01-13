# Upstream Sync Analysis - December 26-27, 2025

## Summary

Analyzed 25 pending upstream commits from iTerm2 (b0d325da9..7171ea8a5).

**Result:** All 24 commits have been backported. Only 1 cosmetic optimization (MenuTips xcassets) remains for future work.

## Commits Already Applied

### Crash Fixes (5 commits) ✅
| Commit | Description | GitLab Issue |
|--------|-------------|--------------|
| 7171ea8a5 | Prevent crashes with wonky command output ranges | - |
| 5dda3da94 | Fix bug where two sessions could have same divorced GUID | - |
| b75bdca6c | Fix crash when snippets/folders intermingled in toolbelt | - |
| a345c5d83 | Fix crash dragging color without colorspace into color well | - |
| 84d501719 | Fix crash when editing a session (TriggerController) | - |

### Bug Fixes (10 commits) ✅
| Commit | Description | GitLab Issue |
|--------|-------------|--------------|
| eb61cff32 | Improve assertion for crash debugging | - |
| c82859d57 | Open Profiles window moves to current Space | #12647 |
| 61078a624 | Fix Web menu not being added | - |
| ccf346e75 | Move 'new tabs at end' to regular prefs | #12655 |
| 78eecf776 | Add menu item for alternate new tab position | #12655 |
| b28e7a72c | Fix missing window title in maximized windows | #12656 |
| 226074b97 | Fix horizontal lines on non-retina displays | #12657 |
| a71520dc1 | Fix Tahoe tab label font measurement | #12658 |
| b76bf1f9c | Progress indicator goes clockwise from 12 o'clock | #12659 |

### Features Already Applied ✅
| Commit | Description | GitLab Issue |
|--------|-------------|--------------|
| b0d325da9 | SetProfileProperty control sequence | #4586 |
| 0032e5309 | Percentage span for screen windows (code logic) | - |
| 02b57f663 | macOS 26 stoplight button positioning | #12474 |

### Other Applied Commits (7 commits) ✅
- Build fixes, version bumps, shell integration updates, debug improvements
- osc8.txt test additions, search field text completion disabled

## Cosmetic Changes Deferred

### bdbd3e860 - Menu Tips xcassets (Low Priority)
- **Scope:** Asset reorganization for smaller bundle size
- **Why Deferred:** Purely cosmetic, no functional impact
- **Impact:** None - just organizes existing images into xcassets

## Verification - December 27, 2025

- Build verified: ✅ `xcodebuild -scheme DashTerm2 -configuration Development`
- Tests passed: ✅ 4465 tests, 0 failures
- No crash reports detected
- All crash fixes present in sources
- All bug fixes present in sources
- SetProfileProperty feature confirmed working in VT100Terminal.m

### Re-verification by Worker #1378 (Dec 27, 2025)

All key backports confirmed present via code inspection:
- ✅ `lineRangeForMark` bounds checking (PTYTextView+ARC.m:557)
- ✅ `rangeOfMark` end-before-start fix (VT100ScreenState.m:1214)
- ✅ `startBlockIndex` NSNotFound guard (iTermLineBlockArray.m:858)
- ✅ `profileForCreatingNewSessionBasedOn:` divorced GUID fix (ProfileModel.m:745)
- ✅ `rangeOfFolders:` snippet insertion fix (iTermToolSnippets.m:274)
- ✅ `offerToSetProfileProperties:` SetProfileProperty feature (iTermNaggingController.m:150)

## Note on Git History

DashTerm2 maintains a parallel codebase to iTerm2 rather than directly merging.
Upstream changes are backported via manual code application, not cherry-pick.
The command `git log HEAD..upstream/master` will always show pending commits
because the histories are independent. Backport verification is done via code inspection.

## Next Steps

1. Continue monitoring upstream for new commits
2. Burn list is 100% complete - no remaining Open bugs
3. Consider MenuTips xcassets migration if bundle size becomes a concern
