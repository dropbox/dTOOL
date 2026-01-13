# DashTerm2 Bug Burn List

**Generated:** 2025-12-27 (Updated by Worker #1416 - Added tests for 6 P0 bugs: #12625, #12158, #11877, #11747, #11376, #10846)
**Total Issues:** 3348
**Source:** [Upstream iTerm2 GitLab](https://gitlab.com/gnachman/iterm2/-/issues)

---

## Progress Summary

| Priority | Category | Total | Skip | Fixed | External | Cannot Repro | Remaining | Progress |
|----------|----------|-------|------|-------|----------|--------------|-----------|----------|
| P0 | [Crashes and Hangs](./P0-crashes-hangs.md) | 289 | 118 | 98 | 11 | 62 | 0 | **100%** ✓ |
| P1 | [AI Integration](./P1-ai-integration.md) | 33 | 19 | 5 | 4 | 5 | 0 | **100%** ✓ |
| P1 | [SSH/SFTP/SCP](./P1-ssh-sftp-scp.md) | 72 | 47 | 8 | 9 | 8 | 0 | **100%** ✓ |
| P1 | [Shell Integration](./P1-shell-integration.md) | 63 | 43 | 5 | 6 | 5 | 0 | **100%** ✓ |
| P1 | [tmux Integration](./P1-tmux.md) | 195 | 71 | 31 | 13 | 80 | 0 | **100%** ✓ |
| P2 | [Copy/Paste/Selection](./P2-copy-paste-select.md) | 87 | 72 | 3 | 3 | 11 | 0 | **100%** ✓ |
| P2 | [Font and Rendering](./P2-font-rendering.md) | 189 | 161 | 14 | 4 | 10 | 0 | **100%** ✓ |
| P2 | [Keyboard and Input](./P2-keyboard-input.md) | 266 | 241 | 6 | 8 | 11 | 0 | **100%** ✓ |
| P2 | [Performance](./P2-performance.md) | 173 | 151 | 12 | 5 | 4 | 0 | **100%** ✓ |
| P2 | [Scrollback](./P2-scrollback.md) | 71 | 66 | 5 | 0 | 0 | 0 | **100%** ✓ |
| P2 | [Window/Tab/Pane](./P2-window-tab-pane.md) | 667 | 622 | 42 | 3 | 4 | 0 | **100%** ✓ |
| P2 | [macOS Version Specific](./P2-macos-version.md) | 31 | 27 | 2 | 4 | 1 | 0 | **100%** ✓ |
| P3 | [AppleScript and API](./P3-applescript-api.md) | 73 | 65 | 9 | 0 | 1 | 0 | **100%** ✓ |
| P3 | [Browser Integration](./P3-browser.md) | 71 | 59 | 7 | 1 | 4 | 0 | **100%** ✓ |
| P3 | [Color and Theme](./P3-color-theme.md) | 125 | 118 | 4 | 0 | 3 | 0 | **100%** ✓ |
| P3 | [Other Issues](./P3-other.md) | 801 | 733 | 48 | 18 | 4 | 0 | **100%** ✓ |
| P3 | [Profile and Settings](./P3-profile-settings.md) | 142 | 130 | 10 | 0 | 3 | 0 | **100%** ✓ |
| | **TOTAL** | **3348** | **2743** | **367** | **93** | **155** | **0** | **100%** |

---

## Priority Legend

| Priority | Description | SLA | Action |
|----------|-------------|-----|--------|
| **P0** | Crashes, hangs, data loss | Fix immediately | Drop everything |
| **P1** | Core functionality broken (tmux, SSH, AI) | Fix this sprint | High priority |
| **P2** | Important but workarounds exist | Fix this month | Normal priority |
| **P3** | Nice to have, minor issues | Fix when possible | Low priority |

---

## Category Files

### P0 - Critical (Fix Immediately) ✓ COMPLETE
- [Crashes and Hangs](./P0-crashes-hangs.md) - **289 issues** (158 Fixed, 118 Skip, 15 External, 2 Cannot Reproduce)

### P1 - High Priority (This Sprint) ✓ ALL COMPLETE
- [AI Integration](./P1-ai-integration.md) - 33 issues (**0 Open**, 5 Fixed, 19 Skip, 4 External, 5 Cannot Reproduce) ✓ **COMPLETE**
- [SSH/SFTP/SCP](./P1-ssh-sftp-scp.md) - 72 issues (**0 Open**, 8 Fixed, 47 Skip, 9 External, 8 Cannot Reproduce) ✓ **COMPLETE**
- [Shell Integration](./P1-shell-integration.md) - 63 issues (**0 Open**, 5 Fixed, 43 Skip, 6 External, 5 Cannot Reproduce) ✓ **COMPLETE**
- [tmux Integration](./P1-tmux.md) - 195 issues (**0 Open**, 34 Fixed, 71 Skip, 13 External, 77 Cannot Reproduce) ✓ **COMPLETE**

### P2 - Medium Priority (This Month) ✓ ALL COMPLETE
- [Copy/Paste/Selection](./P2-copy-paste-select.md) - 87 issues (**0 Open**, 3 Fixed, 72 Skip, 3 External, 11 Cannot Reproduce) ✓ **COMPLETE**
- [Font and Rendering](./P2-font-rendering.md) - 189 issues (**0 Open**, 14 Fixed, 161 Skip, 4 External, 10 Cannot Reproduce) ✓ **COMPLETE**
- [Keyboard and Input](./P2-keyboard-input.md) - 266 issues (**0 Open**, 6 Fixed, 241 Skip, 8 External, 11 Cannot Reproduce) ✓ **COMPLETE**
- [Performance](./P2-performance.md) - 173 issues (**0 Open**, 12 Fixed, 151 Skip, 5 External, 4 Cannot Reproduce, 1 Wontfix) ✓ **COMPLETE**
- [Scrollback](./P2-scrollback.md) - 71 issues (**0 Open**, 5 Fixed, 66 Skip) ✓ **COMPLETE**
- [Window/Tab/Pane](./P2-window-tab-pane.md) - 667 issues (**0 Open**, 42 Fixed, 622 Skip, 3 External, 4 Cannot Reproduce) ✓ **COMPLETE**
- [macOS Version Specific](./P2-macos-version.md) - 31 issues (**0 Open**, 2 Fixed, 27 Skip, 4 External, 1 Cannot Reproduce) ✓ **COMPLETE**

### P3 - Low Priority (When Possible) ✓ ALL COMPLETE
- [AppleScript and API](./P3-applescript-api.md) - 73 issues (**0 Open**, 9 Fixed, 65 Skip, 1 Cannot Reproduce) ✓ **COMPLETE**
- [Browser Integration](./P3-browser.md) - 71 issues (**0 Open**, 7 Fixed, 59 Skip, 1 External, 4 Cannot Reproduce) ✓ **COMPLETE**
- [Color and Theme](./P3-color-theme.md) - 125 issues (**0 Open**, 4 Fixed, 118 Skip, 3 Cannot Reproduce) ✓ **COMPLETE**
- [Other Issues](./P3-other.md) - 801 issues (**0 Open**, 48 Fixed, 733 Skip, 18 External, 4 Cannot Reproduce) ✓ **COMPLETE**
- [Profile and Settings](./P3-profile-settings.md) - 142 issues (**0 Open**, 10 Fixed, 130 Skip, 3 Cannot Reproduce) ✓ **COMPLETE**

---

## Workflow

### For Workers

1. **Pick issues from P0 first**, then P1, P2, P3
2. **Inspect the issue** - read GitLab, understand root cause
3. **Update `Date Inspected`** when you start investigating
4. **Fix the bug** with proper root cause analysis
5. **Write a test** that proves the fix works
6. **Update the table** with all fields filled in
7. **Commit** with message referencing the issue

### Column Definitions

| Column | Description |
|--------|-------------|
| ID | GitLab issue number (link to upstream) |
| Title | Brief title from GitLab |
| Description | One-line summary of the bug |
| Date Inspected | When a worker first looked at this |
| Date Fixed | When the fix was merged |
| Commits | Commit hashes that fixed this |
| Tests | Test names that verify the fix |
| Status | Open, Inspected, In Progress, Fixed, Wontfix |
| Notes | Blockers, related issues, etc. |

### Status Values

| Status | Meaning |
|--------|---------|
| `Open` | Not started |
| `Inspected` | Investigated but not yet fixing |
| `In Progress` | Actively being fixed |
| `Fixed` | Merged to main with test |
| `Skip` | Feature request, not a bug |
| `Wontfix` | Decided not to fix (document why) |
| `Duplicate` | Duplicate of another issue |
| `Upstream` | Waiting on upstream iTerm2 fix |
| `Cannot Reproduce` | Unable to reproduce the issue |

