# Window/Tab/Pane

**Priority:** P2
**Total Issues:** 667
**Skip:** 621
**Fixed:** 44
**External:** 1
**Cannot Reproduce:** 3
**In Progress:** 0
**Remaining:** 0
**Last Updated:** 2025-12-27 (Worker #1376 - Fix statistics)

[< Back to Master Index](./README.md)

---

## Issues

| ID | Title | Description | Date Inspected | Date Fixed | Commits | Tests | Status | Notes |
|----|-------|-------------|----------------|------------|---------|-------|--------|-------|
| [#12656](https://gitlab.com/gnachman/iterm2/-/issues/12656) | Window title doesn't display in the tab bar by default. | Maximized windows hide title | 2025-12-27 | 2025-12-27 | b28e7a72c | - | Fixed | Upstream fix: WINDOW_TYPE_MAXIMIZED should NOT hide title on macOS 26 |
| [#12627](https://gitlab.com/gnachman/iterm2/-/issues/12627) | Toggle broadcast input shotcut moves the window main disp... | Transient - resolved on restart | 2025-12-27 | - | - | - | Skip | Cannot Reproduce - user reported "resolved when I close iterm and relaunch" |
| [#12623](https://gitlab.com/gnachman/iterm2/-/issues/12623) | [BUG][UI] Tab Bar (Sometimes) Overlaps With Terminal Cont... | Fullscreen tabbar overlaps | 2025-12-27 | 2025-12-27 | 9e67ccf0b | - | Fixed | Upstream fix: Use tabBarShouldBeVisibleEvenWhenOnLoan in frame calculation |
| [#12621](https://gitlab.com/gnachman/iterm2/-/issues/12621) | Terminal window resizes when password window opens | Password dialog wider than terminal resizes window | 2025-12-27 | 2025-12-27 | a8a58d0b9 | - | Fixed | Show password manager as modal window when terminal is narrower than sheet min width |
| [#12608](https://gitlab.com/gnachman/iterm2/-/issues/12608) | Add an empty space between the built-in actions for the w... | - | - | - | - | - | Skip | Feature request |
| [#12602](https://gitlab.com/gnachman/iterm2/-/issues/12602) | New window placement places window at unexpected location | Default arrangement ignored on startup | 2025-12-27 | - | - | - | Skip | Feature request - user wants different placement strategy "just like a few versions ago" |
| [#12570](https://gitlab.com/gnachman/iterm2/-/issues/12570) | Open Link prompt blocks all input to global hotkey window... | 2025-12-27 | 2025-12-27 | 5a6a79081 | - | Fixed | Upstream fix: Show openURL as sheet to avoid non-sheet behavior |
| [#12563](https://gitlab.com/gnachman/iterm2/-/issues/12563) | Can't remember the window position | 2025-12-27 | 2025-12-27 | fab44cdb5, d1333c51b | - | Fixed | Upstream fix: Clean up window positioning with explicit 'restore position' mode |
| [#12554](https://gitlab.com/gnachman/iterm2/-/issues/12554) | Black bar instead of tabs in fullscreen | 2025-12-27 | 2025-12-27 | ebe1546ec | - | Fixed | Upstream fix: Work around macOS fullScreenMinHeight bug in titlebar accessory |
| [#12553](https://gitlab.com/gnachman/iterm2/-/issues/12553) | An outline for the active tab in full screen when multipl... | - | - | - | - | - | Skip | Feature request |
| [#12550](https://gitlab.com/gnachman/iterm2/-/issues/12550) | iTerm remembers a window location partly off screen | New window opens partly off-screen | 2025-12-27 | - | - | - | Skip | Feature request - user wants different window placement behavior (force windows on-screen) |
| [#12505](https://gitlab.com/gnachman/iterm2/-/issues/12505) | Hotkey windows do not work in v3.6.2 (re-opened issue) | 0x0 window arrangements | 2025-12-27 | 2025-12-27 | 0501023cd | - | Fixed | Upstream fix: Repair 0x0 windows to use originalSize |
| [#12500](https://gitlab.com/gnachman/iterm2/-/issues/12500) | Hotkey window is overriding "remember size of previously ... | Cmd-N uses hotkey window size after closing it | 2025-12-27 | - | - | - | Skip | Edge case - complex hotkey/regular window interaction, user reported crash with debug logging |
| [#12498](https://gitlab.com/gnachman/iterm2/-/issues/12498) | Improve tab visibility on new Tahoe version | - | - | - | - | - | Skip | Feature request |
| [#12487](https://gitlab.com/gnachman/iterm2/-/issues/12487) | Pane Title Display Issue | Pane titles misaligned on multi-monitor | 2025-12-27 | 2025-12-27 | 4bca192ec | - | Fixed | SessionTitleView now implements iTermViewScreenNotificationHandling to recalculate pixel-aligned layout on screen change |
| [#12472](https://gitlab.com/gnachman/iterm2/-/issues/12472) | Window title no longe centred | Title alignment change | 2025-12-27 | - | - | - | Skip | Feature request - user explicitly says "not sure if bug or feature" |
| [#12453](https://gitlab.com/gnachman/iterm2/-/issues/12453) | Configuration for inactive tab color | - | - | - | - | - | Skip | Feature request |
| [#12426](https://gitlab.com/gnachman/iterm2/-/issues/12426) | Profile window dimensions being ignored | - | 2025-12-27 | 2025-12-27 | 1afdafdb8 | - | Fixed | - |
| [#12420](https://gitlab.com/gnachman/iterm2/-/issues/12420) | Confirm Multi-Line Paste pops Triggers window | Multi-line paste dialog unexpectedly opens Triggers | 2025-12-27 | - | - | - | Cannot Reproduce | Investigated code paths: iTermPasteHelper.maybeWarnAboutMultiLinePaste shows iTermWarning alert, no connection to editTriggers found. Requires specific SSH+Debian environment to reproduce. |
| [#12399](https://gitlab.com/gnachman/iterm2/-/issues/12399) | Support Linear Gradient(s) as Tab Colors | - | - | - | - | - | Skip | Feature request |
| [#12396](https://gitlab.com/gnachman/iterm2/-/issues/12396) | Add a control window for the 'Broadcast input' feature | - | - | - | - | - | Skip | Feature request |
| [#12389](https://gitlab.com/gnachman/iterm2/-/issues/12389) | Select pane shortcut does not fully focus pane while maxi... | 2025-12-27 | 2025-12-27 | 7340d4869 | - | Fixed | Upstream fix: Numeric shortcut now makes pane visible when another is maximized |
| [#12340](https://gitlab.com/gnachman/iterm2/-/issues/12340) | New Window shell closes immediately on Tahoe 26.0 beta 2 | macOS 26 beta issue | 2025-12-27 | - | - | - | Skip | External - macOS 26.0 beta specific, await stable release |
| [#12329](https://gitlab.com/gnachman/iterm2/-/issues/12329) | Top part of the terminal cannot be selected on the 3rd tab | 2025-12-27 | 2025-12-27 | 6ab358e21 | - | Fixed | Upstream fix: Fix animated composer dismissal leaving invisible view |
| [#12312](https://gitlab.com/gnachman/iterm2/-/issues/12312) | Double clicking top of window doesn't enlarge window when... | Double-click title bar doesn't Fill with Minimal theme | 2025-12-27 | - | - | - | Skip | External - macOS title bar behavior with Minimal theme (no standard title bar) |
| [#12289](https://gitlab.com/gnachman/iterm2/-/issues/12289) | Hotkey for "Restore Window Arrangement as Tabs"? | - | - | - | - | - | Skip | Feature request |
| [#12265](https://gitlab.com/gnachman/iterm2/-/issues/12265) | Password manager triggered when window shrinks | Trigger fires when resizing with matching text visible | 2025-12-27 | - | - | - | Cannot Reproduce | Reporter couldn't reproduce - intermittent on tab restore |
| [#12247](https://gitlab.com/gnachman/iterm2/-/issues/12247) | $ITERM_SESSION_ID contains duplicate pane ids after close... | Pane IDs were always returning last index | 2025-12-27 | 2025-12-27 | aa9039250 | - | Fixed | sessionPaneNumber returned count-1 instead of actual index |
| [#12238](https://gitlab.com/gnachman/iterm2/-/issues/12238) | DashTerm2 windows are temporarily unusable after Mac syst... | Post macOS update transient | 2025-12-27 | - | - | - | Skip | External - macOS system update issue, not iTerm2 bug |
| [#12226](https://gitlab.com/gnachman/iterm2/-/issues/12226) | macOS Sequoia keyboard shortcuts for window tiling stoppe... | Fn-Ctrl-Arrow tiling shortcuts don't work | 2025-12-27 | - | - | - | External | macOS Sequoia window tiling behavior - not iTerm2 bug |
| [#12197](https://gitlab.com/gnachman/iterm2/-/issues/12197) | Show status of connection in window menu | - | - | - | - | - | Skip | Feature request |
| [#12193](https://gitlab.com/gnachman/iterm2/-/issues/12193) | If a find string occurs multiple overlapping times in the... | Find count inconsistent for overlapping matches | 2025-12-27 | 2025-12-27 | 9da80e1d8 | test_BUG_12193_overlappingMatchesForward, test_BUG_12193_longerOverlappingPattern | Fixed | Changed search to find overlapping matches |
| [#12186](https://gitlab.com/gnachman/iterm2/-/issues/12186) | 'Open profiles' window always opens on another monitor | Open Profiles on wrong monitor/space | 2025-12-27 | 2025-12-27 | f1ffde970 (c82859d57) | - | Fixed | Upstream fix: Add NSWindowCollectionBehaviorMoveToActiveSpace to move window to current space. Same fix as #12647 |
| [#12175](https://gitlab.com/gnachman/iterm2/-/issues/12175) | After a single tab is full screen The newly opened tab la... | New tab label hidden in fullscreen until mouse hover | 2025-12-27 | 2025-12-27 | 8c627c415 | - | Fixed | Update titlebar accessory fullScreenMinHeight during tab bar flash for M-chip Macs |
| [#12167](https://gitlab.com/gnachman/iterm2/-/issues/12167) | Restore Window Arrangement Position Incorrect for Third M... | Third monitor window restores to wrong position | 2025-12-27 | - | - | - | Skip | Edge case - 3+ monitor setup with portrait/landscape mix |
| [#12165](https://gitlab.com/gnachman/iterm2/-/issues/12165) | new window has wrong size | Window 11+ becomes 173x54 instead of 80x27 | 2025-12-27 | - | - | - | Cannot Reproduce | Intermittent in v3.5.11 - may be fixed in 3.6.x |
| [#12152](https://gitlab.com/gnachman/iterm2/-/issues/12152) | Find text - causes text to be highlighted on other panes | Search highlights leak to all panes | 2025-12-27 | 2025-12-27 | f543ec10a | - | Fixed | PTYSession observer callback now only auto-searches in initiating session |
| [#11909](https://gitlab.com/gnachman/iterm2/-/issues/11909) | AppleScript: support `send text at start` parameter for `... | - | - | - | - | - | Skip | Feature request |
| [#11907](https://gitlab.com/gnachman/iterm2/-/issues/11907) | "I want DashTerm2 to adapt to the latest macOS Sequoia's ... | - | - | - | - | - | Skip | Feature request |
| [#11902](https://gitlab.com/gnachman/iterm2/-/issues/11902) | terminal history off by one tab | - | - | - | - | - | Skip | Old (2024) |
| [#11871](https://gitlab.com/gnachman/iterm2/-/issues/11871) | [3.5.5beta2] Window Tiling No Longer Works | - | - | - | - | - | Skip | Old (2024) |
| [#11870](https://gitlab.com/gnachman/iterm2/-/issues/11870) | Can the AXDocument accessibility attribute be set for the... | - | - | - | - | - | Skip | Feature request |
| [#11865](https://gitlab.com/gnachman/iterm2/-/issues/11865) | toggleOpenDashboardIfHiddenWindows Cannot be config after... | - | - | - | - | - | Skip | Old (2024) |
| [#11863](https://gitlab.com/gnachman/iterm2/-/issues/11863) | Fixed-size panes | - | - | - | - | - | Skip | Feature request |
| [#11830](https://gitlab.com/gnachman/iterm2/-/issues/11830) | I would like a drop down menu in title bar with a searcha... | - | - | - | - | - | Skip | Feature request |
| [#11809](https://gitlab.com/gnachman/iterm2/-/issues/11809) | Why 'Continue restoring state?' window | - | - | - | - | - | Skip | Old (2024) |
| [#11802](https://gitlab.com/gnachman/iterm2/-/issues/11802) | Tabs bar not displayed (room allocated for display but is... | - | - | - | - | - | Skip | Old (2024) |
| [#11784](https://gitlab.com/gnachman/iterm2/-/issues/11784) | open a new tab in iTerm in the same folder as the one tha... | - | - | - | - | - | Skip | Feature request |
| [#11762](https://gitlab.com/gnachman/iterm2/-/issues/11762) | iterm2 window comes to foreground when mouse hovers over it | - | - | - | - | - | Skip | Old (2024) |
| [#11761](https://gitlab.com/gnachman/iterm2/-/issues/11761) | when opening a new window (or tab) with "focus follows mo... | - | - | - | - | - | Skip | Old (2024) |
| [#11754](https://gitlab.com/gnachman/iterm2/-/issues/11754) | Shortcut-opened dedicated window inherits last focused wi... | - | - | - | - | - | Skip | Old (2024) |
| [#11751](https://gitlab.com/gnachman/iterm2/-/issues/11751) | Whole split(s) turns white | - | - | - | - | - | Skip | Old (2024) |
| [#11741](https://gitlab.com/gnachman/iterm2/-/issues/11741) | Status bar option to limit to per-tab/session | - | - | - | - | - | Skip | Feature request |
| [#11707](https://gitlab.com/gnachman/iterm2/-/issues/11707) | Search feature issue when using multiple tabs or panes | - | - | - | - | - | Skip | Old (2024) |
| [#11702](https://gitlab.com/gnachman/iterm2/-/issues/11702) | Typing underscore in find panel moves focus to terminal | - | - | - | - | - | Skip | Old (2024) |
| [#11699](https://gitlab.com/gnachman/iterm2/-/issues/11699) | create a new tab, it appears separately | - | - | - | - | - | Skip | Old (2024) |
| [#11695](https://gitlab.com/gnachman/iterm2/-/issues/11695) | iTerm 3.5.3: search window immediately loses focus | - | - | - | - | - | Skip | Old (2024) |
| [#11693](https://gitlab.com/gnachman/iterm2/-/issues/11693) | Make tabs easier to distinguish | - | - | - | - | - | Skip | Feature request |
| [#11692](https://gitlab.com/gnachman/iterm2/-/issues/11692) | Window restarted in wrong space after update | - | - | - | - | - | Skip | Old (2024) |
| [#11686](https://gitlab.com/gnachman/iterm2/-/issues/11686) | Tab bar appearance is broken in fullscreen after cmd+t is... | - | - | - | - | - | Skip | Old (2024) |
| [#11646](https://gitlab.com/gnachman/iterm2/-/issues/11646) | Auto saving and naming with summary of tabs/sessions | - | - | - | - | - | Skip | Feature request |
| [#11632](https://gitlab.com/gnachman/iterm2/-/issues/11632) | Brief flash of smaller, duplicate prompt in middle of win... | - | - | - | - | - | Skip | Old (2024) |
| [#11618](https://gitlab.com/gnachman/iterm2/-/issues/11618) | Tabs not preserved on copy/paste | - | - | - | - | - | Skip | Old (2024) |
| [#11616](https://gitlab.com/gnachman/iterm2/-/issues/11616) | Folder rights when window is started from AppleScript | - | - | - | - | - | Skip | Old (2024) |
| [#11599](https://gitlab.com/gnachman/iterm2/-/issues/11599) | pop-up window when hitting tab on auto composer will hide... | - | - | - | - | - | Skip | Old (2024) |
| [#11595](https://gitlab.com/gnachman/iterm2/-/issues/11595) | Allow the terminal input pane to be pinned to the top or ... | - | - | - | - | - | Skip | Feature request |
| [#11563](https://gitlab.com/gnachman/iterm2/-/issues/11563) | After update to 3.5, on tab press leaves behind previous ... | - | - | - | - | - | Skip | Old (2024) |
| [#11471](https://gitlab.com/gnachman/iterm2/-/issues/11471) | Show/hide all windows hotkey fails to toggle sometimes | - | - | - | - | - | Skip | Old (2024) |
| [#11450](https://gitlab.com/gnachman/iterm2/-/issues/11450) | Tab title mixups | - | - | - | - | - | Skip | Old (2024) |
| [#11442](https://gitlab.com/gnachman/iterm2/-/issues/11442) | Full screen window is not full screen after disconnecting... | - | - | - | - | - | Skip | Old (2024) |
| [#11419](https://gitlab.com/gnachman/iterm2/-/issues/11419) | Restore current working directory as well as window arran... | - | - | - | - | - | Skip | Feature request |
| [#11377](https://gitlab.com/gnachman/iterm2/-/issues/11377) | Hotkey window skirts other application's inclusions/exclu... | - | - | - | - | - | Skip | Old (2024) |
| [#11361](https://gitlab.com/gnachman/iterm2/-/issues/11361) | Enabling Secure Keyboard Entry setting blocks commands fr... | - | - | - | - | - | Skip | Old (2024) |
| [#11355](https://gitlab.com/gnachman/iterm2/-/issues/11355) | Allow Key Binding for "Set Tab Title" | - | - | - | - | - | Skip | Feature request |
| [#11344](https://gitlab.com/gnachman/iterm2/-/issues/11344) | Three-finger tap to paste does not work in Hotkey Window | - | - | - | - | - | Skip | Old (2024) |
| [#11342](https://gitlab.com/gnachman/iterm2/-/issues/11342) | OSX - Sonoma - Navigation Shortcuts -> Shortcut to select... | - | - | - | - | - | Skip | Old (2024) |
| [#11337](https://gitlab.com/gnachman/iterm2/-/issues/11337) | Limit Cycle Through Windows to same desktop/space? | - | - | - | - | - | Skip | Feature request |
| [#11305](https://gitlab.com/gnachman/iterm2/-/issues/11305) | Incognito/private window/tab | - | - | - | - | - | Skip | Feature request |
| [#11297](https://gitlab.com/gnachman/iterm2/-/issues/11297) | fail to split horizontally with default keyboard shortcut | - | - | - | - | - | Skip | Old (2024) |
| [#11274](https://gitlab.com/gnachman/iterm2/-/issues/11274) | Tab title becomes "0X0" occasionally | - | - | - | - | - | Skip | Old (2024) |
| [#11272](https://gitlab.com/gnachman/iterm2/-/issues/11272) | Clicking an 'active' dock icon with no windows open no lo... | - | - | - | - | - | Skip | Old (2024) |
| [#11265](https://gitlab.com/gnachman/iterm2/-/issues/11265) | shortcut for split horizontally with current profile (cmd... | - | - | - | - | - | Skip | Old (2024) |
| [#11250](https://gitlab.com/gnachman/iterm2/-/issues/11250) | Close ITerm dedicated hotkey window on alt+tab or trackpa... | - | - | - | - | - | Skip | Feature request |
| [#11247](https://gitlab.com/gnachman/iterm2/-/issues/11247) | Windows on multiple desktop Spaces not restored to their ... | - | - | - | - | - | Skip | Old (2024) |
| [#11239](https://gitlab.com/gnachman/iterm2/-/issues/11239) | New tab/window not reusing previous session's directory | - | - | - | - | - | Skip | Old (2024) |
| [#11229](https://gitlab.com/gnachman/iterm2/-/issues/11229) | Window is moved from HDMI display to different display wh... | - | 2025-12-27 | 2025-12-27 | 1c5da3837 | - | Fixed | - |
| [#11214](https://gitlab.com/gnachman/iterm2/-/issues/11214) | Cursors between split panes do not unfocus correctly | - | 2025-12-27 | 2025-12-27 | 4c2507319 | - | Fixed | - |
| [#11211](https://gitlab.com/gnachman/iterm2/-/issues/11211) | When switching from another app to DashTerm2, focus doesn... | - | - | - | - | - | Skip | Old (2024) |
| [#11206](https://gitlab.com/gnachman/iterm2/-/issues/11206) | Hotkey not working when Safari window active | - | - | - | - | - | Skip | Old (2024) |
| [#11202](https://gitlab.com/gnachman/iterm2/-/issues/11202) | Selecting new theme moves all iTerm windows to current Space | - | - | - | - | - | Skip | Old (2024) |
| [#11200](https://gitlab.com/gnachman/iterm2/-/issues/11200) | hotkey window does not open in other apps except when an ... | - | - | - | - | - | Skip | Old (2024) |
| [#11198](https://gitlab.com/gnachman/iterm2/-/issues/11198) | Lots of strange entries (GUID.itermtab) in DashTerm2's 'r... | - | 2025-12-27 | 2025-12-27 | ed3566fbe | - | Fixed | - |
| [#11157](https://gitlab.com/gnachman/iterm2/-/issues/11157) | Need option to show confirmation message while closing in... | - | - | - | - | - | Skip | Feature request |
| [#11148](https://gitlab.com/gnachman/iterm2/-/issues/11148) | Pending command is copied to command history when splitti... | - | - | - | - | - | Skip | Old (2023) |
| [#11136](https://gitlab.com/gnachman/iterm2/-/issues/11136) | when using a hotkey window, opening apps from iTerm messe... | - | 2025-12-27 | 2025-12-27 | 9c5be76be | - | Fixed | - |
| [#11133](https://gitlab.com/gnachman/iterm2/-/issues/11133) | Ability to quick switch to a tab based on the title | - | - | - | - | - | Skip | Feature request |
| [#11125](https://gitlab.com/gnachman/iterm2/-/issues/11125) | Window instant close on launch 3.5.git.45491f0826 | - | - | - | - | - | Skip | Old (2023) |
| [#11120](https://gitlab.com/gnachman/iterm2/-/issues/11120) | iterm2 hotkey window lose focus when using Alfred | - | - | - | - | - | Skip | Old (2023) |
| [#11117](https://gitlab.com/gnachman/iterm2/-/issues/11117) | Wrong window position with multiple monitors | - | - | - | - | - | Skip | Old (2023) |
| [#11112](https://gitlab.com/gnachman/iterm2/-/issues/11112) | Exiting fullscreen makes Dock unexpectedly hidden | - | - | - | - | - | Skip | Old (2023) |
| [#11109](https://gitlab.com/gnachman/iterm2/-/issues/11109) | Unable to move single tab to different window with compac... | - | - | - | - | - | Skip | Old (2023) |
| [#11108](https://gitlab.com/gnachman/iterm2/-/issues/11108) | Window closes spontaneously after finishing command | - | - | - | - | - | Skip | Old (2023) |
| [#11100](https://gitlab.com/gnachman/iterm2/-/issues/11100) | Window continues to pop to active / front of screen in Mac | - | - | - | - | - | Skip | Old (2023) |
| [#11073](https://gitlab.com/gnachman/iterm2/-/issues/11073) | Starting up with "system window restoration setting" rand... | - | - | - | - | - | Skip | Old (2023) |
| [#11064](https://gitlab.com/gnachman/iterm2/-/issues/11064) | Hotkey window doesn't always respond to hotkey on macOS S... | - | - | - | - | - | Skip | Old (2023) |
| [#11048](https://gitlab.com/gnachman/iterm2/-/issues/11048) | macOS: Hotkey drop-down (floating) window constrained to ... | - | - | - | - | - | Skip | Old (2023) |
| [#11042](https://gitlab.com/gnachman/iterm2/-/issues/11042) | Hotkey Window placed on "wrong" desktop (single monitor) | - | - | - | - | - | Skip | Old (2023) |
| [#11041](https://gitlab.com/gnachman/iterm2/-/issues/11041) | OOM + Closing tabs after session restore is O(n²)? | - | - | - | - | - | Skip | Old (2023) |
| [#11037](https://gitlab.com/gnachman/iterm2/-/issues/11037) | Does DashTerm2 support OS-native Split View? | - | - | - | - | - | Skip | Feature request |
| [#11027](https://gitlab.com/gnachman/iterm2/-/issues/11027) | New Window Goes Blank | - | - | - | - | - | Skip | Old (2023) |
| [#11003](https://gitlab.com/gnachman/iterm2/-/issues/11003) | Iterm2 window which is in background automatically comes ... | - | - | - | - | - | Skip | Old (2023) |
| [#10999](https://gitlab.com/gnachman/iterm2/-/issues/10999) | Always opens non-native full screen for a hotkey window r... | - | - | - | - | - | Skip | Old (2021) |
| [#10988](https://gitlab.com/gnachman/iterm2/-/issues/10988) | [Feature Request] Add "Maximize pane" to available action... | - | - | - | - | - | Skip | Feature request |
| [#10976](https://gitlab.com/gnachman/iterm2/-/issues/10976) | Hot Key window does take focus on macOS Sonoma | - | - | - | - | - | Skip | Old (2021) |
| [#10950](https://gitlab.com/gnachman/iterm2/-/issues/10950) | iTerm window randomly becomes frontmost while using other... | - | - | - | - | - | Skip | Old (2021) |
| [#10930](https://gitlab.com/gnachman/iterm2/-/issues/10930) | fn+f does not toggle fullscreen with neovim opened | - | - | - | - | - | Skip | Old (2021) |
| [#10914](https://gitlab.com/gnachman/iterm2/-/issues/10914) | Unexpected window/tab closure | - | - | - | - | - | Skip | Old (2021) |
| [#10905](https://gitlab.com/gnachman/iterm2/-/issues/10905) | Resizable preferences/Settings window | - | - | - | - | - | Skip | Feature request |
| [#10894](https://gitlab.com/gnachman/iterm2/-/issues/10894) | iTerm doesn't release file handles when closing tabs that... | - | - | - | - | - | Skip | Old (2021) |
| [#10838](https://gitlab.com/gnachman/iterm2/-/issues/10838) | The focus cannot return to the previous application after... | - | - | - | - | - | Skip | Old (2021) |
| [#10826](https://gitlab.com/gnachman/iterm2/-/issues/10826) | Is there a way to remove new tab button? | - | - | - | - | - | Skip | Feature request |
| [#10769](https://gitlab.com/gnachman/iterm2/-/issues/10769) | Switching between fullscreen terminals ends in an infinit... | - | - | - | - | - | Skip | Old (2021) |
| [#10758](https://gitlab.com/gnachman/iterm2/-/issues/10758) | Window Placement on Update/Restart | - | - | - | - | - | Skip | Old (2021) |
| [#10752](https://gitlab.com/gnachman/iterm2/-/issues/10752) | After upgrade to MacOS Ventura, Command+backtick no longe... | - | - | - | - | - | Skip | Old (2022) |
| [#10746](https://gitlab.com/gnachman/iterm2/-/issues/10746) | iterm2.Window.async_activate does not always raise the wi... | - | 2025-12-27 | 2025-12-27 | 71ad7a6c0 | - | Fixed | - |
| [#10739](https://gitlab.com/gnachman/iterm2/-/issues/10739) | Unexpected window resize and cursor placement when awakin... | - | - | - | - | - | Skip | Old (2022) |
| [#10719](https://gitlab.com/gnachman/iterm2/-/issues/10719) | Mac OSx Stage Manager doesn't not iterm2 window is side p... | - | - | - | - | - | Skip | Old (2022) |
| [#10709](https://gitlab.com/gnachman/iterm2/-/issues/10709) | No way to save "don't open windows when attaching if ther... | - | - | - | - | - | Skip | Feature request |
| [#10704](https://gitlab.com/gnachman/iterm2/-/issues/10704) | two-finger swiping between tabs does not work when cursor... | - | 2025-12-27 | 2025-12-27 | a98e2feae | - | Fixed | - |
| [#10695](https://gitlab.com/gnachman/iterm2/-/issues/10695) | After Ventura update: hotkey window causes space switch | - | - | - | - | - | Skip | Old (2022) |
| [#10690](https://gitlab.com/gnachman/iterm2/-/issues/10690) | Automatic update not happening. iTerm reverting back to l... | - | - | - | - | - | Skip | Old (2022) |
| [#10678](https://gitlab.com/gnachman/iterm2/-/issues/10678) | Filter on status bar resizes window on backspace | - | - | - | - | - | Skip | Old (2022) |
| [#10608](https://gitlab.com/gnachman/iterm2/-/issues/10608) | iterm2 window size is smaller every time I log in | - | - | - | - | - | Skip | Old (2022) |
| [#10607](https://gitlab.com/gnachman/iterm2/-/issues/10607) | Using a Hot key window + fullscreen + ProMotion + status ... | - | - | - | - | - | Skip | Old (2022) |
| [#10605](https://gitlab.com/gnachman/iterm2/-/issues/10605) | Mirroring of Hotkey Window on all displays | - | - | - | - | - | Skip | Feature request |
| [#10602](https://gitlab.com/gnachman/iterm2/-/issues/10602) | Sending directory from LaunchBar opens 2 windows | - | - | - | - | - | Skip | Old (2022) |
| [#10589](https://gitlab.com/gnachman/iterm2/-/issues/10589) | hotkey window has forground even when invisible | - | - | - | - | - | Skip | Old (2022) |
| [#10585](https://gitlab.com/gnachman/iterm2/-/issues/10585) | OSC 8 hyperlinks to trigger GET request instead of openin... | - | - | - | - | - | Skip | Feature request |
| [#10581](https://gitlab.com/gnachman/iterm2/-/issues/10581) | All double-clicking on tab to perform an action | - | - | - | - | - | Skip | Feature request |
| [#10507](https://gitlab.com/gnachman/iterm2/-/issues/10507) | Open new windows in the center of the screen? | - | - | - | - | - | Skip | Feature request |
| [#10494](https://gitlab.com/gnachman/iterm2/-/issues/10494) | Dismiss hotkey window brings up other iTerm windows | - | - | - | - | - | Skip | Old (2022) |
| [#10457](https://gitlab.com/gnachman/iterm2/-/issues/10457) | DashTerm2 windows icons disappear while minimized in Dock | - | - | - | - | - | Skip | Old (2022) |
| [#10422](https://gitlab.com/gnachman/iterm2/-/issues/10422) | Window width not remembered with Full-Height style (Hotke... | - | - | - | - | - | Skip | Old (2022) |
| [#10414](https://gitlab.com/gnachman/iterm2/-/issues/10414) | iTermGraphDatabase persistence is very inefficient | - | 2025-12-27 | 2025-12-27 | ef943bcd3 | - | Fixed | - |
| [#10399](https://gitlab.com/gnachman/iterm2/-/issues/10399) | Command line navigation shortcuts don't work as expected ... | - | - | - | - | - | Skip | Old (2022) |
| [#10397](https://gitlab.com/gnachman/iterm2/-/issues/10397) | run command on launch *in specific window* | - | - | - | - | - | Skip | Feature request |
| [#10375](https://gitlab.com/gnachman/iterm2/-/issues/10375) | Crowdstrike receives excessive process events from DashTe... | - | - | - | - | - | Skip | Old (2022) |
| [#10371](https://gitlab.com/gnachman/iterm2/-/issues/10371) | profile window style setting is showed incorrectly | - | - | - | - | - | Skip | Old (2022) |
| [#10340](https://gitlab.com/gnachman/iterm2/-/issues/10340) | synchronize-panes | - | - | - | - | - | Skip | Feature request |
| [#10326](https://gitlab.com/gnachman/iterm2/-/issues/10326) | Make it possible to access menu bar items when using (onl... | - | - | - | - | - | Skip | Feature request |
| [#10324](https://gitlab.com/gnachman/iterm2/-/issues/10324) | When a window is created from AppleScript with a command ... | - | - | - | - | - | Skip | Old (2022) |
| [#10267](https://gitlab.com/gnachman/iterm2/-/issues/10267) | Bell notification batch when window largely on-screen | - | 2025-12-27 | 2025-12-27 | 6c5ed58bc | - | Fixed | - |
| [#10265](https://gitlab.com/gnachman/iterm2/-/issues/10265) | Split window not resizing on close | - | - | - | - | - | Skip | Old (2022) |
| [#10263](https://gitlab.com/gnachman/iterm2/-/issues/10263) | Add a "compact minimal" window theme | - | - | - | - | - | Skip | Feature request |
| [#10254](https://gitlab.com/gnachman/iterm2/-/issues/10254) | Run command per tab on startup (use case: activate same c... | - | - | - | - | - | Skip | Feature request |
| [#10243](https://gitlab.com/gnachman/iterm2/-/issues/10243) | Clipboard content is entred into search window | - | - | - | - | - | Skip | Old (2022) |
| [#10228](https://gitlab.com/gnachman/iterm2/-/issues/10228) | option to remove shell type from tab title | - | - | - | - | - | Skip | Feature request |
| [#10222](https://gitlab.com/gnachman/iterm2/-/issues/10222) | iterm2 Windows do not retain location in a Space when dis... | - | - | - | - | - | Skip | Old (2022) |
| [#10211](https://gitlab.com/gnachman/iterm2/-/issues/10211) | Tab color escape codes not producing the correct colors | - | - | - | - | - | Skip | Old (2022) |
| [#10174](https://gitlab.com/gnachman/iterm2/-/issues/10174) | mark tabs with broadcast by colour | - | - | - | - | - | Skip | Feature request |
| [#10159](https://gitlab.com/gnachman/iterm2/-/issues/10159) | Automatically resize tab bar height to make use of the wh... | - | - | - | - | - | Skip | Feature request |
| [#10124](https://gitlab.com/gnachman/iterm2/-/issues/10124) | Remove bell notifications when window is active and focused | - | - | - | - | - | Skip | Feature request |
| [#10118](https://gitlab.com/gnachman/iterm2/-/issues/10118) | hotkey window: only first tab uses hotkey window profile,... | - | - | - | - | - | Skip | Old (2021) |
| [#10114](https://gitlab.com/gnachman/iterm2/-/issues/10114) | Mouse selection is not always working when multiple panes... | - | - | - | - | - | Skip | Old (2021) |
| [#10096](https://gitlab.com/gnachman/iterm2/-/issues/10096) | Explorer Panel | - | - | - | - | - | Skip | Feature request |
| [#10095](https://gitlab.com/gnachman/iterm2/-/issues/10095) | Window size erroneous on restore | - | - | - | - | - | Skip | Old (2021) |
| [#10077](https://gitlab.com/gnachman/iterm2/-/issues/10077) | Clicking a window didn't activate it. Instead, another wi... | - | - | - | - | - | Skip | Old (2021) |
| [#10056](https://gitlab.com/gnachman/iterm2/-/issues/10056) | Advance window options conflict/overlap in functionality | - | - | - | - | - | Skip | Old (2021) |
| [#10040](https://gitlab.com/gnachman/iterm2/-/issues/10040) | Make system notification visible when using hotkey fullsc... | - | - | - | - | - | Skip | Feature request |
| [#10037](https://gitlab.com/gnachman/iterm2/-/issues/10037) | Cannot activate hotkey window when keyboard focus is on a... | - | - | - | - | - | Skip | Old (2021) |
| [#10030](https://gitlab.com/gnachman/iterm2/-/issues/10030) | favicons on tabs | - | - | - | - | - | Skip | Feature request |
| [#10020](https://gitlab.com/gnachman/iterm2/-/issues/10020) | iTerm grabs focus even when clicking on other app windows | - | - | - | - | - | Skip | Old (2021) |
| [#10000](https://gitlab.com/gnachman/iterm2/-/issues/10000) | Disappearing tab bar when using a hotkey | - | - | - | - | - | Skip | Old (2021) |
| [#9965](https://gitlab.com/gnachman/iterm2/-/issues/9965) | Window Borders MIA after last update. | 2025-12-27 | 2025-12-27 | 4f9dcd539 | - | Fixed | Upstream fix: Add advanced pref to draw window borders in dark mode |
| [#9935](https://gitlab.com/gnachman/iterm2/-/issues/9935) | Look at Windows Terminal | - | - | - | - | - | Skip | Feature request |
| [#9906](https://gitlab.com/gnachman/iterm2/-/issues/9906) | New windows/tabs do not honor the chosen profile | - | - | - | - | - | Skip | Old (2020) |
| [#9905](https://gitlab.com/gnachman/iterm2/-/issues/9905) | "Always accept first mouse event on terminal windows" sto... | - | - | - | - | - | Skip | Old (2020) |
| [#9897](https://gitlab.com/gnachman/iterm2/-/issues/9897) | small white bar under tabs | - | - | - | - | - | Skip | Old (2020) |
| [#9890](https://gitlab.com/gnachman/iterm2/-/issues/9890) | Reopen closed tab | - | - | - | - | - | Skip | Feature request |
| [#9874](https://gitlab.com/gnachman/iterm2/-/issues/9874) | Cannot run osascript commands fron Hotkey Window when vs ... | - | - | - | - | - | Skip | Old (2020) |
| [#9865](https://gitlab.com/gnachman/iterm2/-/issues/9865) | Next and previous tabs across windows | - | - | - | - | - | Skip | Feature request |
| [#9843](https://gitlab.com/gnachman/iterm2/-/issues/9843) | The window with the highest number becomes active after C... | - | - | - | - | - | Skip | Old (2020) |
| [#9833](https://gitlab.com/gnachman/iterm2/-/issues/9833) | Minimal theme tab text color is hard to read | 2025-12-27 | 2025-12-27 | 36ee57aab | - | Fixed | Upstream fix: Improve legibility of non-selected tab labels in Minimal |
| [#9826](https://gitlab.com/gnachman/iterm2/-/issues/9826) | Keyboard shortcut to move tab to new window | - | - | - | - | - | Skip | Feature request |
| [#9823](https://gitlab.com/gnachman/iterm2/-/issues/9823) | Make Title Bar span entire window for easier grab/move/re... | - | - | - | - | - | Skip | Feature request |
| [#9784](https://gitlab.com/gnachman/iterm2/-/issues/9784) | async_update_layout doesn't work with vertical splits | - | - | - | - | - | Skip | Old (2020) |
| [#9779](https://gitlab.com/gnachman/iterm2/-/issues/9779) | Set tab color via trigger | - | - | - | - | - | Skip | Feature request |
| [#9774](https://gitlab.com/gnachman/iterm2/-/issues/9774) | [Feature Request] Vertical Tabs and/or Search Window/Tab ... | - | - | - | - | - | Skip | Feature request |
| [#9753](https://gitlab.com/gnachman/iterm2/-/issues/9753) | Is it possible to customize the first window position on ... | - | - | - | - | - | Skip | Feature request |
| [#9751](https://gitlab.com/gnachman/iterm2/-/issues/9751) | System-wide hotkey to open iTerm windows | - | - | - | - | - | Skip | Feature request |
| [#9744](https://gitlab.com/gnachman/iterm2/-/issues/9744) | Fullscreen windows last line not at bottom of window | - | - | - | - | - | Skip | Old (2020) |
| [#9743](https://gitlab.com/gnachman/iterm2/-/issues/9743) | White line flickering at top in fullscreen. | - | - | - | - | - | Skip | Old (2020) |
| [#9742](https://gitlab.com/gnachman/iterm2/-/issues/9742) | Why DashTerm2 only show part of my window？ | 2025-12-27 | 2025-12-27 | 2f68627be | - | Fixed | Upstream fix: Ensure TTY size is set after we get file descriptor |
| [#9741](https://gitlab.com/gnachman/iterm2/-/issues/9741) | Toggle broadcast to all panes in tab works but display is... | - | - | - | - | - | Skip | Old (2020) |
| [#9725](https://gitlab.com/gnachman/iterm2/-/issues/9725) | Output from one window appearing in another | - | - | - | - | - | Skip | Old (2020) |
| [#9722](https://gitlab.com/gnachman/iterm2/-/issues/9722) | Window arrangements opened at launch are now NARROWER tha... | - | - | - | - | - | Skip | Old (2020) |
| [#9711](https://gitlab.com/gnachman/iterm2/-/issues/9711) | Disable program name in tab title | - | - | - | - | - | Skip | Feature request |
| [#9705](https://gitlab.com/gnachman/iterm2/-/issues/9705) | Hotkey Window infuriatingly overlapping the menu bar | - | - | - | - | - | Skip | Old (2020) |
| [#9700](https://gitlab.com/gnachman/iterm2/-/issues/9700) | Text jumps when switching tabs | - | - | - | - | - | Skip | Old (2020) |
| [#9679](https://gitlab.com/gnachman/iterm2/-/issues/9679) | UX improvement: MRU not only between Tabs but across Windows | - | - | - | - | - | Skip | Feature request |
| [#9643](https://gitlab.com/gnachman/iterm2/-/issues/9643) | previous session windows take 10+ minutes to restore when... | 2025-12-27 | 2025-12-27 | bcf94c57c | - | Fixed | Upstream fix: Prompt to delete restoration db if integrity check takes >10s |
| [#9628](https://gitlab.com/gnachman/iterm2/-/issues/9628) | Broadcast Input to All Panes in Current Tab using API wor... | - | - | - | - | - | Skip | Old (2020) |
| [#9624](https://gitlab.com/gnachman/iterm2/-/issues/9624) | iTerm tabs opening new windows on macOS Big Sur | - | - | - | - | - | Skip | Old (2020) |
| [#9564](https://gitlab.com/gnachman/iterm2/-/issues/9564) | FInd Globally doesn't show windows titles | - | - | - | - | - | Skip | Old (2020) |
| [#9547](https://gitlab.com/gnachman/iterm2/-/issues/9547) | Tab title contains escaped shell command in session_title | - | - | - | - | - | Skip | Old (2020) |
| [#9543](https://gitlab.com/gnachman/iterm2/-/issues/9543) | UX improvement: while dragging a window holding Alt (Opt)... | - | - | - | - | - | Skip | Feature request |
| [#9536](https://gitlab.com/gnachman/iterm2/-/issues/9536) | Frontmost window focus is lost when switching out and bac... | - | - | - | - | - | Skip | Old (2020) |
| [#9533](https://gitlab.com/gnachman/iterm2/-/issues/9533) | Python API: Activate a session in a hotkey window | - | - | - | - | - | Skip | Old (2020) |
| [#9531](https://gitlab.com/gnachman/iterm2/-/issues/9531) | Preference option:  confirm before closing tab | - | - | - | - | - | Skip | Feature request |
| [#9524](https://gitlab.com/gnachman/iterm2/-/issues/9524) | Transparent margin is present on panes | - | - | - | - | - | Skip | Old (2020) |
| [#9519](https://gitlab.com/gnachman/iterm2/-/issues/9519) | Full screen windows that can't be closed | - | - | - | - | - | Skip | Old (2020) |
| [#9456](https://gitlab.com/gnachman/iterm2/-/issues/9456) | Control behavior of "new tab": append to end or create af... | - | - | - | - | - | Skip | Feature request |
| [#9415](https://gitlab.com/gnachman/iterm2/-/issues/9415) | Clear state removed when switching tabs | - | - | - | - | - | Skip | Old (2020) |
| [#9364](https://gitlab.com/gnachman/iterm2/-/issues/9364) | Black persistent full-screen window appears after reopeni... | - | - | - | - | - | Skip | Old (2020) |
| [#9351](https://gitlab.com/gnachman/iterm2/-/issues/9351) | Switching tabs using Logitech shortcut does not work as d... | - | - | - | - | - | Skip | Old (2020) |
| [#9338](https://gitlab.com/gnachman/iterm2/-/issues/9338) | It's too hard to drag "minimal" windows with tabs | - | - | - | - | - | Skip | Old (2020) |
| [#9321](https://gitlab.com/gnachman/iterm2/-/issues/9321) | Unfocused windows have text replaced with squares | - | - | - | - | - | Skip | Old (2020) |
| [#9320](https://gitlab.com/gnachman/iterm2/-/issues/9320) | Advanced Paste with a string containing tabs only inserts... | 2025-12-27 | 2025-12-27 | 93e38a65f | - | Fixed | Upstream fix: Add advanced setting for wait-for-prompt in advanced paste |
| [#9286](https://gitlab.com/gnachman/iterm2/-/issues/9286) | Upon restart, the last-used tab loses its session because... | - | - | - | - | - | Skip | Old (2020) |
| [#9278](https://gitlab.com/gnachman/iterm2/-/issues/9278) | Unable to move iTerm window to a different monitor anymore | 2025-12-27 | 2025-12-27 | 8188f85b0 | - | Fixed | Upstream fix: Allow Window > Move to [display] by saying windows are movable during menu validation |
| [#9251](https://gitlab.com/gnachman/iterm2/-/issues/9251) | Request: Publish stable releases to Beta feed | - | - | - | - | - | Skip | Feature request |
| [#9248](https://gitlab.com/gnachman/iterm2/-/issues/9248) | Software Update window's height is restricted (probably b... | - | - | - | - | - | Skip | Old (2020) |
| [#9239](https://gitlab.com/gnachman/iterm2/-/issues/9239) | [Feature request] Save tabs/session on quit | - | - | - | - | - | Skip | Feature request |
| [#9231](https://gitlab.com/gnachman/iterm2/-/issues/9231) | DashTerm2 drop-down window size increases on restart | - | - | - | - | - | Skip | Old (2020) |
| [#9226](https://gitlab.com/gnachman/iterm2/-/issues/9226) | Focus on specific tab | - | - | - | - | - | Skip | Old (2020) |
| [#9213](https://gitlab.com/gnachman/iterm2/-/issues/9213) | Bug: Iterm2 window height is smaller than expected when u... | - | - | - | - | - | Skip | Old (2020) |
| [#9193](https://gitlab.com/gnachman/iterm2/-/issues/9193) | Moving DashTerm2 between displays results in blank window | - | - | - | - | - | Skip | Old (2020) |
| [#9189](https://gitlab.com/gnachman/iterm2/-/issues/9189) | Tabs stopped working in iterm2 | - | - | - | - | - | Skip | Old (2020) |
| [#9174](https://gitlab.com/gnachman/iterm2/-/issues/9174) | Reduced responsiveness while typing (perhaps when two Das... | 2025-12-27 | 2025-12-27 | 1da5cdee1 | - | Fixed | Upstream fix: Disable metal for obscured windows when using integrated GPUs |
| [#9162](https://gitlab.com/gnachman/iterm2/-/issues/9162) | [Feature Request] Enable shortcut for "Move session to sp... | - | - | - | - | - | Skip | Feature request |
| [#9158](https://gitlab.com/gnachman/iterm2/-/issues/9158) | Tab titles wrong in 3.4.0beta8 | - | - | - | - | - | Skip | Old (2020) |
| [#9155](https://gitlab.com/gnachman/iterm2/-/issues/9155) | Window doesn't resize when moving to/from external monito... | - | - | - | - | - | Skip | Old (2020) |
| [#9129](https://gitlab.com/gnachman/iterm2/-/issues/9129) | Unable to use password manager, window does not open | - | - | - | - | - | Skip | Old (2020) |
| [#9120](https://gitlab.com/gnachman/iterm2/-/issues/9120) | Allow for a window columns/rows sizing option per hotkey ... | - | - | - | - | - | Skip | Feature request |
| [#9088](https://gitlab.com/gnachman/iterm2/-/issues/9088) | Feature Request: attaching/detaching hot-key to any iterm... | - | - | - | - | - | Skip | Feature request |
| [#9080](https://gitlab.com/gnachman/iterm2/-/issues/9080) | Pressing escape key always returns to first tab and iTerm... | - | - | - | - | - | Skip | Old (2020) |
| [#9071](https://gitlab.com/gnachman/iterm2/-/issues/9071) | Feature Request: multiple rows in tab bar | - | - | - | - | - | Skip | Feature request |
| [#9065](https://gitlab.com/gnachman/iterm2/-/issues/9065) | Starting up with "open default window arrangement" causes... | 2025-12-27 | 2025-12-27 | b8fe888a4 | - | Fixed | Upstream fix: Add ability to repair saved arrangements with bad initial working directories |
| [#9064](https://gitlab.com/gnachman/iterm2/-/issues/9064) | Tab title gets reverted | - | - | - | - | - | Skip | Old (2020) |
| [#9062](https://gitlab.com/gnachman/iterm2/-/issues/9062) | Unable to open new tab in Build 3.4.0beta2 | - | - | - | - | - | Skip | Old (2020) |
| [#9053](https://gitlab.com/gnachman/iterm2/-/issues/9053) | new session, through config, load tabs, windows by direct... | - | - | - | - | - | Skip | Old (2020) |
| [#9032](https://gitlab.com/gnachman/iterm2/-/issues/9032) | Editable session name status bar component | - | - | - | - | - | Skip | Feature request |
| [#9023](https://gitlab.com/gnachman/iterm2/-/issues/9023) | Enhancement: show tab bar temporarily when dragging onto ... | - | - | - | - | - | Skip | Feature request |
| [#9022](https://gitlab.com/gnachman/iterm2/-/issues/9022) | sometimes can't open a new tab | 2025-12-27 | 2025-12-27 | cc603d00f, 48e0dc9ab | - | Fixed | Upstream fix: Copy iTermServer to safe place before executing |
| [#9002](https://gitlab.com/gnachman/iterm2/-/issues/9002) | Hotkey Window Only Triggered if DashTerm2 is Foreground App | - | - | - | - | - | Skip | Old (2020) |
| [#9000](https://gitlab.com/gnachman/iterm2/-/issues/9000) | Using terminal splits spawn unresponsive process | - | - | - | - | - | Skip | Old (2020) |
| [#8958](https://gitlab.com/gnachman/iterm2/-/issues/8958) | [ Feature Request ] Return result of Trigger Run Command ... | - | - | - | - | - | Skip | Feature request |
| [#8939](https://gitlab.com/gnachman/iterm2/-/issues/8939) | Color issue on About window | - | - | - | - | - | Skip | Old (2019) |
| [#8929](https://gitlab.com/gnachman/iterm2/-/issues/8929) | Attempt to drag iTerm window causes undesired tear-off of... | - | - | - | - | - | Skip | Old (2019) |
| [#8927](https://gitlab.com/gnachman/iterm2/-/issues/8927) | Tab Bar keeps disappearing | - | - | - | - | - | Skip | Old (2019) |
| [#8894](https://gitlab.com/gnachman/iterm2/-/issues/8894) | Menubar stays displayed when displaying iterm2 in fullscr... | - | - | - | - | - | Skip | Old (2019) |
| [#8891](https://gitlab.com/gnachman/iterm2/-/issues/8891) | Is it possible to get the path of just the active tab wit... | - | - | - | - | - | Skip | Feature request |
| [#8877](https://gitlab.com/gnachman/iterm2/-/issues/8877) | FR: Allow pane title on the bottom | - | - | - | - | - | Skip | Feature request |
| [#8857](https://gitlab.com/gnachman/iterm2/-/issues/8857) | Should restore windows to last used monitor in multi-moni... | - | - | - | - | - | Skip | Feature request |
| [#8848](https://gitlab.com/gnachman/iterm2/-/issues/8848) | Feature request: Config for active/non-active tab | - | - | - | - | - | Skip | Feature request |
| [#8836](https://gitlab.com/gnachman/iterm2/-/issues/8836) | [spotlight] Show up last iTerm window (and not hotkey win... | - | - | - | - | - | Skip | Old (2019) |
| [#8830](https://gitlab.com/gnachman/iterm2/-/issues/8830) | Up-arrow history is shared across tabs since recent update? | - | - | - | - | - | Skip | Old (2019) |
| [#8819](https://gitlab.com/gnachman/iterm2/-/issues/8819) | Fullscreen windows aren't restored when quit-started | - | - | - | - | - | Skip | Old (2019) |
| [#8818](https://gitlab.com/gnachman/iterm2/-/issues/8818) | Separate Hotkey Window per macOS Space | - | - | - | - | - | Skip | Feature request |
| [#8816](https://gitlab.com/gnachman/iterm2/-/issues/8816) | Why is transparency toggled On by default when creating n... | - | - | - | - | - | Skip | Old (2019) |
| [#8812](https://gitlab.com/gnachman/iterm2/-/issues/8812) | New Tab and New Tab with current Profile both open with c... | - | - | - | - | - | Skip | Old (2019) |
| [#8809](https://gitlab.com/gnachman/iterm2/-/issues/8809) | DashTerm2 Window Cannot be moved or resized | - | - | - | - | - | Skip | Old (2019) |
| [#8750](https://gitlab.com/gnachman/iterm2/-/issues/8750) | DashTerm2 is great, but the tab design isn't | - | - | - | - | - | Skip | Feature request |
| [#8739](https://gitlab.com/gnachman/iterm2/-/issues/8739) | How can one save DashTerm2 hotkey window settings (size/d... | - | - | - | - | - | Skip | Old (2019) |
| [#8734](https://gitlab.com/gnachman/iterm2/-/issues/8734) | Make "Maximized" style and size locking orthogonal | - | - | - | - | - | Skip | Feature request |
| [#8702](https://gitlab.com/gnachman/iterm2/-/issues/8702) | Default color profile doesn't fully applied to restored w... | - | - | - | - | - | Skip | Old (2019) |
| [#8686](https://gitlab.com/gnachman/iterm2/-/issues/8686) | Option key no longer works with command-control shortcut ... | - | - | - | - | - | Skip | Old (2019) |
| [#8681](https://gitlab.com/gnachman/iterm2/-/issues/8681) | Window border incomplete around bottom corners | 2025-12-27 | 2025-12-27 | 938f8b91d | - | Fixed | Upstream fix: Draw nice round borders for transparent windows on 10.14+ |
| [#8658](https://gitlab.com/gnachman/iterm2/-/issues/8658) | silence bell with multiple windows | - | - | - | - | - | Skip | Old (2019) |
| [#8633](https://gitlab.com/gnachman/iterm2/-/issues/8633) | [Feature Request] Highlight window as I move through the ... | - | - | - | - | - | Skip | Feature request |
| [#8619](https://gitlab.com/gnachman/iterm2/-/issues/8619) | colorful window buttons with Compact theme + dark mode + ... | - | - | - | - | - | Skip | Old (2019) |
| [#8608](https://gitlab.com/gnachman/iterm2/-/issues/8608) | DashTerm2 repeatedly popping up blank window with "open w... | - | - | - | - | - | Skip | Old (2019) |
| [#8599](https://gitlab.com/gnachman/iterm2/-/issues/8599) | Keybindings not persisting after loading a Window Arrange... | - | - | - | - | - | Skip | Old (2019) |
| [#8598](https://gitlab.com/gnachman/iterm2/-/issues/8598) | Sending break-pane doesn't work as expected | 2025-12-27 | 2025-12-27 | 0eab16a8f | - | Fixed | Upstream fix: Update tmux window opening mode pref text |
| [#8582](https://gitlab.com/gnachman/iterm2/-/issues/8582) | Have different settings for blur and opacity for active a... | - | - | - | - | - | Skip | Feature request |
| [#8556](https://gitlab.com/gnachman/iterm2/-/issues/8556) | Password manager pop up window does not close by itself | - | - | - | - | - | Skip | Old (2019) |
| [#8551](https://gitlab.com/gnachman/iterm2/-/issues/8551) | Tab bar on touch bar | - | - | - | - | - | Skip | Feature request |
| [#8468](https://gitlab.com/gnachman/iterm2/-/issues/8468) | Focus loss after dismissing window with hotkey | - | - | - | - | - | Skip | Old (2019) |
| [#8463](https://gitlab.com/gnachman/iterm2/-/issues/8463) | [Feature request] Disable specific hotkey(s) when running... | - | - | - | - | - | Skip | Feature request |
| [#8446](https://gitlab.com/gnachman/iterm2/-/issues/8446) | "Paste bracketing left on" message in some or all panes o... | - | - | - | - | - | Skip | Old (2019) |
| [#8427](https://gitlab.com/gnachman/iterm2/-/issues/8427) | Support focusing a certain split pane with an escape sequ... | - | - | - | - | - | Skip | Feature request |
| [#8418](https://gitlab.com/gnachman/iterm2/-/issues/8418) | Async working directory resolving results in wrong initia... | 2025-12-27 | 2025-12-27 | 0d7cc4dae | - | Fixed | Upstream fix: Invalidate outstanding working directory polls on CurrentDir update |
| [#8386](https://gitlab.com/gnachman/iterm2/-/issues/8386) | 3.3.6 insists on appending session title to each tab title | 2025-12-27 | 2025-12-27 | f5f1a0485, bad460fae | - | Fixed | Upstream fix: Allow no selection for title components, add custom tab title pref |
| [#8348](https://gitlab.com/gnachman/iterm2/-/issues/8348) | Feature request: Remember toolbelt splitter position | - | - | - | - | - | Skip | Feature request |
| [#8336](https://gitlab.com/gnachman/iterm2/-/issues/8336) | Write script to sync statusbar and tab color | - | - | - | - | - | Skip | Old (2019) |
| [#8325](https://gitlab.com/gnachman/iterm2/-/issues/8325) | Ocasionally floating window spontaneously loses transpare... | - | - | - | - | - | Skip | Old (2019) |
| [#8303](https://gitlab.com/gnachman/iterm2/-/issues/8303) | Floating hotkey window can't be found by applescript | - | - | - | - | - | Skip | Old (2019) |
| [#8288](https://gitlab.com/gnachman/iterm2/-/issues/8288) | 2 iTerm windows are opened when using Finder service | - | - | - | - | - | Skip | Old (2019) |
| [#8283](https://gitlab.com/gnachman/iterm2/-/issues/8283) | Regression in programmatic setting of tab title | - | - | - | - | - | Skip | Old (2019) |
| [#8278](https://gitlab.com/gnachman/iterm2/-/issues/8278) | Feature Request: moving window with tab | - | - | - | - | - | Skip | Feature request |
| [#8271](https://gitlab.com/gnachman/iterm2/-/issues/8271) | Resizing split pane activates triggers | - | - | - | - | - | Skip | Old (2019) |
| [#8266](https://gitlab.com/gnachman/iterm2/-/issues/8266) | Feature Request: Have the background picture fill the tab... | - | - | - | - | - | Skip | Feature request |
| [#8252](https://gitlab.com/gnachman/iterm2/-/issues/8252) | can't set the window name | - | - | - | - | - | Skip | Old (2019) |
| [#8225](https://gitlab.com/gnachman/iterm2/-/issues/8225) | Python API: no easy way to set session pane size relative... | - | - | - | - | - | Skip | Feature request |
| [#8219](https://gitlab.com/gnachman/iterm2/-/issues/8219) | Control-Tab doesn't honor order of iTerm tabs | - | - | - | - | - | Skip | Old (2019) |
| [#8208](https://gitlab.com/gnachman/iterm2/-/issues/8208) | Window arrangement minimizing to dock on restore | - | - | - | - | - | Skip | Old (2019) |
| [#8146](https://gitlab.com/gnachman/iterm2/-/issues/8146) | Terminal window goes blank | - | - | - | - | - | Skip | Old (2019) |
| [#8108](https://gitlab.com/gnachman/iterm2/-/issues/8108) | [question] How do I make touch-id-enabled sudo play nicel... | - | - | - | - | - | Skip | Feature request |
| [#8091](https://gitlab.com/gnachman/iterm2/-/issues/8091) | Command + return (default shortcut) for command box in ne... | - | - | - | - | - | Skip | Old (2019) |
| [#8051](https://gitlab.com/gnachman/iterm2/-/issues/8051) | iTerm create invisible windows making other applications ... | 2025-12-27 | 2025-12-27 | 39b434b77 | - | Fixed | Upstream fix: Don't order in hotkey window when restoring from arrangement |
| [#8046](https://gitlab.com/gnachman/iterm2/-/issues/8046) | [Feature request] Small bar at window top in theme minima... | - | - | - | - | - | Skip | Feature request |
| [#7985](https://gitlab.com/gnachman/iterm2/-/issues/7985) | New Theme Cannot Take Effect on Python REPL Window | - | - | - | - | - | Skip | Old (pre-2019) |
| [#7933](https://gitlab.com/gnachman/iterm2/-/issues/7933) | Feature Request: Add ability to attach a text file for "n... | - | - | - | - | - | Skip | Feature request |
| [#7913](https://gitlab.com/gnachman/iterm2/-/issues/7913) | key action to hide hotkey window | - | - | - | - | - | Skip | Feature request |
| [#7904](https://gitlab.com/gnachman/iterm2/-/issues/7904) | drop-down iTerm window lets double-shift key through to I... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#7887](https://gitlab.com/gnachman/iterm2/-/issues/7887) | Make it possible to open Hot Key window without showing h... | - | - | - | - | - | Skip | Feature request |
| [#7852](https://gitlab.com/gnachman/iterm2/-/issues/7852) | New setting to minimize Hotkey Window upon loss of focus | - | - | - | - | - | Skip | Feature request |
| [#7839](https://gitlab.com/gnachman/iterm2/-/issues/7839) | [Feature request] expose `working directory` to scriptabl... | - | - | - | - | - | Skip | Feature request |
| [#7826](https://gitlab.com/gnachman/iterm2/-/issues/7826) | Cmd_~ switching works incorrectly when HotKey window is s... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#7815](https://gitlab.com/gnachman/iterm2/-/issues/7815) | blue dot prevents tab name from appearing | - | 2025-12-27 | 2025-12-27 | 258a64d31 | - | Fixed | - |
| [#7813](https://gitlab.com/gnachman/iterm2/-/issues/7813) | Provide an option to switch according to MRU when closing... | - | - | - | - | - | Skip | Feature request |
| [#7796](https://gitlab.com/gnachman/iterm2/-/issues/7796) | make a window title bar settable (custom text and colour) | - | - | - | - | - | Skip | Feature request |
| [#7750](https://gitlab.com/gnachman/iterm2/-/issues/7750) | Window title accessibility mismatch | - | - | - | - | - | Skip | Old (pre-2019) |
| [#7708](https://gitlab.com/gnachman/iterm2/-/issues/7708) | Preferences window takes a long time to open | - | - | - | - | - | Skip | Old (pre-2019) |
| [#7698](https://gitlab.com/gnachman/iterm2/-/issues/7698) | Window Title used to show a session number? | - | - | - | - | - | Skip | Old (pre-2019) |
| [#7690](https://gitlab.com/gnachman/iterm2/-/issues/7690) | Visual issues with dark (& transparent) windows | - | - | - | - | - | Skip | Old (pre-2019) |
| [#7675](https://gitlab.com/gnachman/iterm2/-/issues/7675) | maximizing window splits and then returning to the normal... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#7667](https://gitlab.com/gnachman/iterm2/-/issues/7667) | Window locations on multiple desktops not preserved on re... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#7662](https://gitlab.com/gnachman/iterm2/-/issues/7662) | Non-native fullscreen broken in 3.3.0beta2 and 3.2.8 | - | - | - | - | - | Skip | Old (pre-2019) |
| [#7653](https://gitlab.com/gnachman/iterm2/-/issues/7653) | command-shift-[ command-shift-] suddenly broke for switch... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#7647](https://gitlab.com/gnachman/iterm2/-/issues/7647) | Iterm2v3 causes small terminal window | - | 2025-12-27 | 2025-12-27 | 840d221d3 | - | Fixed | - |
| [#7645](https://gitlab.com/gnachman/iterm2/-/issues/7645) | Table formatting not proper in ITERM | - | - | - | - | - | Skip | Old (pre-2019) |
| [#7628](https://gitlab.com/gnachman/iterm2/-/issues/7628) | Can no longer drag window to a different space | - | 2025-12-27 | 2025-12-27 | 399ea7267 | - | Fixed | - |
| [#7602](https://gitlab.com/gnachman/iterm2/-/issues/7602) | Windows content mucked up with external monitor off | - | - | - | - | - | Skip | Old (pre-2019) |
| [#7597](https://gitlab.com/gnachman/iterm2/-/issues/7597) | Drang-n-drop Tab into another Tab doesn't make panes no more | - | - | - | - | - | Skip | Old (pre-2019) |
| [#7592](https://gitlab.com/gnachman/iterm2/-/issues/7592) | Dragging tab out and into separate window resizes new win... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#7586](https://gitlab.com/gnachman/iterm2/-/issues/7586) | left-click pastes in a window open for a while | - | - | - | - | - | Skip | Old (pre-2019) |
| [#7571](https://gitlab.com/gnachman/iterm2/-/issues/7571) | Session restoration: Terminal windows are sometimes not p... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#7564](https://gitlab.com/gnachman/iterm2/-/issues/7564) | The tab looses colour | - | - | - | - | - | Skip | Old (pre-2019) |
| [#7561](https://gitlab.com/gnachman/iterm2/-/issues/7561) | Feature request: Fixed number of columns, possibly exceed... | - | - | - | - | - | Skip | Feature request |
| [#7537](https://gitlab.com/gnachman/iterm2/-/issues/7537) | Cannot disable background blur in transparent window | - | - | - | - | - | Skip | Old (pre-2019) |
| [#7531](https://gitlab.com/gnachman/iterm2/-/issues/7531) | Hotkey Window became incompatible with Tabs? | - | - | - | - | - | Skip | Old (pre-2019) |
| [#7525](https://gitlab.com/gnachman/iterm2/-/issues/7525) | "Close All Panes in Tab" menu item fails to honor "Quit w... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#7503](https://gitlab.com/gnachman/iterm2/-/issues/7503) | [Feature request] Add maximum window size in window style... | - | - | - | - | - | Skip | Feature request |
| [#7488](https://gitlab.com/gnachman/iterm2/-/issues/7488) | Print shortcut window pops up during maven build | - | - | - | - | - | Skip | Old (pre-2019) |
| [#7479](https://gitlab.com/gnachman/iterm2/-/issues/7479) | Does Split pane support  fixed? | - | - | - | - | - | Skip | Old (pre-2019) |
| [#7472](https://gitlab.com/gnachman/iterm2/-/issues/7472) | Feature request: retain command history for individual ta... | - | - | - | - | - | Skip | Feature request |
| [#7462](https://gitlab.com/gnachman/iterm2/-/issues/7462) | Feature Request: ⌘⇧T should reopen last closed tab | - | - | - | - | - | Skip | Feature request |
| [#7435](https://gitlab.com/gnachman/iterm2/-/issues/7435) | Suggestion: Switch to panes vertically (up & down) | - | - | - | - | - | Skip | Feature request |
| [#7432](https://gitlab.com/gnachman/iterm2/-/issues/7432) | MacOS fullscreen remains on desktop space instead of movi... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#7430](https://gitlab.com/gnachman/iterm2/-/issues/7430) | New terminal window launches with each application switch... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#7340](https://gitlab.com/gnachman/iterm2/-/issues/7340) | open a new tab will replace the origin tab | - | - | - | - | - | Skip | Old (pre-2019) |
| [#7321](https://gitlab.com/gnachman/iterm2/-/issues/7321) | [feature request] auto run command in split pane, i.e. vi... | - | - | - | - | - | Skip | Feature request |
| [#7313](https://gitlab.com/gnachman/iterm2/-/issues/7313) | [Feature Enhancement] Make Focus brighten focused DashTer... | - | - | - | - | - | Skip | Feature request |
| [#7308](https://gitlab.com/gnachman/iterm2/-/issues/7308) | Hotkey window trigger gives focus to ALL iterm windows (i... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#7288](https://gitlab.com/gnachman/iterm2/-/issues/7288) | Feature request: Hide "Maximize Active Pane" icon either ... | - | - | - | - | - | Skip | Feature request |
| [#7279](https://gitlab.com/gnachman/iterm2/-/issues/7279) | Don't un-maximize panes when dragging a tab into a maximi... | - | 2025-12-27 | 2025-12-27 | f6570d08d | - | Fixed | - |
| [#7259](https://gitlab.com/gnachman/iterm2/-/issues/7259) | Window shadow gone in 3.2.4 | - | - | - | - | - | Skip | Old (pre-2019) |
| [#7248](https://gitlab.com/gnachman/iterm2/-/issues/7248) | Feature suggestion: when panes maximised show them all a-... | - | - | - | - | - | Skip | Feature request |
| [#7230](https://gitlab.com/gnachman/iterm2/-/issues/7230) | How to run several command in deferent tabs | - | - | - | - | - | Skip | Old (pre-2019) |
| [#7170](https://gitlab.com/gnachman/iterm2/-/issues/7170) | Feature request: Port Iterm2 to Windows using conPTY API | - | - | - | - | - | Skip | Feature request |
| [#7169](https://gitlab.com/gnachman/iterm2/-/issues/7169) | Tab title in compact mode | - | - | - | - | - | Skip | Old (pre-2019) |
| [#7050](https://gitlab.com/gnachman/iterm2/-/issues/7050) | Feature suggestion: allow minimising full screen windows | - | - | - | - | - | Skip | Feature request |
| [#7040](https://gitlab.com/gnachman/iterm2/-/issues/7040) | Feature request: when the mouse hits the top of the scree... | - | - | - | - | - | Skip | Feature request |
| [#7023](https://gitlab.com/gnachman/iterm2/-/issues/7023) | All pop-up dialogues are screwed with Hot Key windows | - | 2025-12-27 | 2025-12-27 | e1ae01cd4 | - | Fixed | - |
| [#7020](https://gitlab.com/gnachman/iterm2/-/issues/7020) | Profile name present in window and tab titles regardless ... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6980](https://gitlab.com/gnachman/iterm2/-/issues/6980) | Feature Request: Different "Blending" Settings for Transp... | - | - | - | - | - | Skip | Feature request |
| [#6964](https://gitlab.com/gnachman/iterm2/-/issues/6964) | Automatic (light) appearance on Mojave retains captures v... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6944](https://gitlab.com/gnachman/iterm2/-/issues/6944) | Iterm window open with Profiles pane, and without mac OSX... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6931](https://gitlab.com/gnachman/iterm2/-/issues/6931) | Creating window in a new session across all workspaces | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6927](https://gitlab.com/gnachman/iterm2/-/issues/6927) | New DashTerm2 Tab / Window Here only working with a selec... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6888](https://gitlab.com/gnachman/iterm2/-/issues/6888) | Right Prompt still have about half-char padding/margin in... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6870](https://gitlab.com/gnachman/iterm2/-/issues/6870) | Feature request: add tree-like Tab-navigation to Window-s... | - | - | - | - | - | Skip | Feature request |
| [#6787](https://gitlab.com/gnachman/iterm2/-/issues/6787) | "Only restore Hotkey window" works randomly | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6762](https://gitlab.com/gnachman/iterm2/-/issues/6762) | Show/hide all windows hotkey bug | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6747](https://gitlab.com/gnachman/iterm2/-/issues/6747) | Feature Request: Configurable (window) theme colors | - | - | - | - | - | Skip | Feature request |
| [#6736](https://gitlab.com/gnachman/iterm2/-/issues/6736) | Request: hotkey window appears on same display (space) as... | - | - | - | - | - | Skip | Feature request |
| [#6731](https://gitlab.com/gnachman/iterm2/-/issues/6731) | Tabs disappear in full screen mode of a window -- when I ... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6726](https://gitlab.com/gnachman/iterm2/-/issues/6726) | Hide HUD window while mission control is running | - | - | - | - | - | Skip | Feature request |
| [#6716](https://gitlab.com/gnachman/iterm2/-/issues/6716) | Native fullscreen window in display one will steals app f... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6709](https://gitlab.com/gnachman/iterm2/-/issues/6709) | Feature Request: Broadcast Input to multiple windows | - | - | - | - | - | Skip | Feature request |
| [#6683](https://gitlab.com/gnachman/iterm2/-/issues/6683) | Problem when open dedicated hotkey window | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6672](https://gitlab.com/gnachman/iterm2/-/issues/6672) | Surprising behaviour with "tell window to select" | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6664](https://gitlab.com/gnachman/iterm2/-/issues/6664) | [Feature] Allow setting shortcut for "Move session to win... | - | - | - | - | - | Skip | Feature request |
| [#6660](https://gitlab.com/gnachman/iterm2/-/issues/6660) | iTerm window does not cover entire screen when in full size | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6629](https://gitlab.com/gnachman/iterm2/-/issues/6629) | Titlebar / tab colour does not match the one set in the d... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6627](https://gitlab.com/gnachman/iterm2/-/issues/6627) | issues with minimized windows upon application restart | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6626](https://gitlab.com/gnachman/iterm2/-/issues/6626) | ITerm2 window do not cover states bar | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6621](https://gitlab.com/gnachman/iterm2/-/issues/6621) | Add more convenient navigation between iterm2 windows | - | - | - | - | - | Skip | Feature request |
| [#6616](https://gitlab.com/gnachman/iterm2/-/issues/6616) | How to stop DashTerm2 from launching a new Window session... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6596](https://gitlab.com/gnachman/iterm2/-/issues/6596) | Swipe tabs trackpad doesn't work with Hotkey floating window | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6562](https://gitlab.com/gnachman/iterm2/-/issues/6562) | iterm2 opens two window or two tabs when launched from se... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6557](https://gitlab.com/gnachman/iterm2/-/issues/6557) | Clone Tab when creating new / splitting current one | - | - | - | - | - | Skip | Feature request |
| [#6555](https://gitlab.com/gnachman/iterm2/-/issues/6555) | Feature request: allow for current Tab settings re-use wh... | - | - | - | - | - | Skip | Feature request |
| [#6553](https://gitlab.com/gnachman/iterm2/-/issues/6553) | The duplicated tab should next to the current tab | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6547](https://gitlab.com/gnachman/iterm2/-/issues/6547) | Unplugging USB mouse from MacBook Pro Retina while using ... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6535](https://gitlab.com/gnachman/iterm2/-/issues/6535) | DashTerm2 windows on all spaces | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6521](https://gitlab.com/gnachman/iterm2/-/issues/6521) | Don't pass-through key presses to other apps when in Hotk... | - | - | - | - | - | Skip | Feature request |
| [#6514](https://gitlab.com/gnachman/iterm2/-/issues/6514) | External borders of window too big | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6513](https://gitlab.com/gnachman/iterm2/-/issues/6513) | Hotkey window selection through MIssion Control immediate... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6508](https://gitlab.com/gnachman/iterm2/-/issues/6508) | Global shortcut Cmd+T for "New Tab" is blocked in DashTerm2 | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6482](https://gitlab.com/gnachman/iterm2/-/issues/6482) | Feature request: more options for current tab indication | - | - | - | - | - | Skip | Feature request |
| [#6467](https://gitlab.com/gnachman/iterm2/-/issues/6467) | New sessions open in new window instead of existing window | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6447](https://gitlab.com/gnachman/iterm2/-/issues/6447) | Feature request: dedicated Window profile aka separate ap... | - | - | - | - | - | Skip | Feature request |
| [#6439](https://gitlab.com/gnachman/iterm2/-/issues/6439) | new window always starts in space #1 | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6438](https://gitlab.com/gnachman/iterm2/-/issues/6438) | Feature request: Terminal.app-style split pane (read-only... | - | - | - | - | - | Skip | Feature request |
| [#6425](https://gitlab.com/gnachman/iterm2/-/issues/6425) | Request:  escape should always close the find/search window | - | - | - | - | - | Skip | Feature request |
| [#6380](https://gitlab.com/gnachman/iterm2/-/issues/6380) | This should be default! No? - Open new tabs in iTerm in t... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6376](https://gitlab.com/gnachman/iterm2/-/issues/6376) | The tab colours should be the opposite | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6375](https://gitlab.com/gnachman/iterm2/-/issues/6375) | Strange window states after Security_Update 2017_005 | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6372](https://gitlab.com/gnachman/iterm2/-/issues/6372) | Window tab title doesn't use all available space when "St... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6366](https://gitlab.com/gnachman/iterm2/-/issues/6366) | opening iterm window brings up problem reporter: "gnumkdi... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6341](https://gitlab.com/gnachman/iterm2/-/issues/6341) | Problem with "Quit when all windows are closed" setting | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6338](https://gitlab.com/gnachman/iterm2/-/issues/6338) | [Feature] Control Strip icon to show/hide the hotkey window | - | - | - | - | - | Skip | Feature request |
| [#6333](https://gitlab.com/gnachman/iterm2/-/issues/6333) | when tab completion actual showed path loses the last letter | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6332](https://gitlab.com/gnachman/iterm2/-/issues/6332) | Ugly 1-pixel border around tabs when selecting a "Tab Color" | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6331](https://gitlab.com/gnachman/iterm2/-/issues/6331) | Feature request: add "Tab" entry to main menu | - | - | - | - | - | Skip | Feature request |
| [#6327](https://gitlab.com/gnachman/iterm2/-/issues/6327) | Tab Bar color not setting properly in High Sierra | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6323](https://gitlab.com/gnachman/iterm2/-/issues/6323) | MAC OS - Zoom maximizes vertical option in the settings h... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6315](https://gitlab.com/gnachman/iterm2/-/issues/6315) | Tab colours not working properly with High Sierra | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6314](https://gitlab.com/gnachman/iterm2/-/issues/6314) | Switch split pane with ALT+number eats ALT+9 (left bracket). | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6309](https://gitlab.com/gnachman/iterm2/-/issues/6309) | New tab doesn't open in current working directory despite... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6307](https://gitlab.com/gnachman/iterm2/-/issues/6307) | Please stop showing an update window | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6302](https://gitlab.com/gnachman/iterm2/-/issues/6302) | toolbelt shows wrong pane for notes and profile | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6294](https://gitlab.com/gnachman/iterm2/-/issues/6294) | Broadcast sends passwords in clear to other tabs | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6288](https://gitlab.com/gnachman/iterm2/-/issues/6288) | Un-forget the convert tabs to spaces | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6280](https://gitlab.com/gnachman/iterm2/-/issues/6280) | Resizing window while in a program (like vim) causes colo... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6268](https://gitlab.com/gnachman/iterm2/-/issues/6268) | Line does not break when writing lines longer than the sp... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6266](https://gitlab.com/gnachman/iterm2/-/issues/6266) | iTerm does not quit on last tab closing. (options default... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6259](https://gitlab.com/gnachman/iterm2/-/issues/6259) | Tiled pane's tab went missing (dunno how) | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6240](https://gitlab.com/gnachman/iterm2/-/issues/6240) | Minimized windows don't appear on system-wide hotkey pres... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6239](https://gitlab.com/gnachman/iterm2/-/issues/6239) | Feature request: show border on current tab even when not... | - | - | - | - | - | Skip | Feature request |
| [#6230](https://gitlab.com/gnachman/iterm2/-/issues/6230) | Closing tabs so that you're left with a single tab resize... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6229](https://gitlab.com/gnachman/iterm2/-/issues/6229) | transparency only works for first tab | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6225](https://gitlab.com/gnachman/iterm2/-/issues/6225) | Open regular terminal windows with hotkey window keyboard... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6222](https://gitlab.com/gnachman/iterm2/-/issues/6222) | Feature request: overlay expose for vertical tab (tab bar... | - | - | - | - | - | Skip | Feature request |
| [#6219](https://gitlab.com/gnachman/iterm2/-/issues/6219) | Man page window | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6198](https://gitlab.com/gnachman/iterm2/-/issues/6198) | DashTerm2 window keeps resizing incorrectly for no discer... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6183](https://gitlab.com/gnachman/iterm2/-/issues/6183) | [question] how can i activate current tab? | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6182](https://gitlab.com/gnachman/iterm2/-/issues/6182) | Can't minimise borderless window | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6156](https://gitlab.com/gnachman/iterm2/-/issues/6156) | Page Up/Down with multiple panes activates incorrect pane | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6136](https://gitlab.com/gnachman/iterm2/-/issues/6136) | Zooming should be tab-wide (or window-wide) | - | - | - | - | - | Skip | Feature request |
| [#6133](https://gitlab.com/gnachman/iterm2/-/issues/6133) | DashTerm2 3.1.2 eats the last line of a command's output ... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6127](https://gitlab.com/gnachman/iterm2/-/issues/6127) | Feature request: Safari-like tabs preview mode | - | - | - | - | - | Skip | Feature request |
| [#6125](https://gitlab.com/gnachman/iterm2/-/issues/6125) | iTerm window does not stretch | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6120](https://gitlab.com/gnachman/iterm2/-/issues/6120) | On update (download&install) Windows do not re-appear on ... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6107](https://gitlab.com/gnachman/iterm2/-/issues/6107) | Tab color not showing exact color since update | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6088](https://gitlab.com/gnachman/iterm2/-/issues/6088) | Feature Request: have applescript "create window" raise o... | - | - | - | - | - | Skip | Feature request |
| [#6087](https://gitlab.com/gnachman/iterm2/-/issues/6087) | Feature request: Possible to keep iTerm hotkey window whe... | - | - | - | - | - | Skip | Feature request |
| [#6079](https://gitlab.com/gnachman/iterm2/-/issues/6079) | Slide down hotkey window animation only on top screens | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6066](https://gitlab.com/gnachman/iterm2/-/issues/6066) | after update tabs with color are surrounded by ugly black... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6053](https://gitlab.com/gnachman/iterm2/-/issues/6053) | Tab title wiggles on BEL | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6038](https://gitlab.com/gnachman/iterm2/-/issues/6038) | Tab white line problem | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6026](https://gitlab.com/gnachman/iterm2/-/issues/6026) | Edit Actions windows doesn't appear | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6010](https://gitlab.com/gnachman/iterm2/-/issues/6010) | Window size wrong in hotkey window when going from many b... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6003](https://gitlab.com/gnachman/iterm2/-/issues/6003) | Ability to select multiple tabs (like Chrome)? | - | - | - | - | - | Skip | Feature request |
| [#6002](https://gitlab.com/gnachman/iterm2/-/issues/6002) | Feature Request: add labels to split panes | - | - | - | - | - | Skip | Feature request |
| [#5978](https://gitlab.com/gnachman/iterm2/-/issues/5978) | Strange double border with 3.1 beta.7 (tabs) | - | - | - | - | - | Skip | Old (pre-2019) |
| [#5938](https://gitlab.com/gnachman/iterm2/-/issues/5938) | Feature Request: New Tab/Window with current profile | - | - | - | - | - | Skip | Feature request |
| [#5891](https://gitlab.com/gnachman/iterm2/-/issues/5891) | Touch bar "i" icon for words kills the current pane and o... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#5856](https://gitlab.com/gnachman/iterm2/-/issues/5856) | Hotkey window shifts to the left on toggle | - | - | - | - | - | Skip | Old (pre-2019) |
| [#5854](https://gitlab.com/gnachman/iterm2/-/issues/5854) | Hotkey window on new desktop leaves empty space on top | - | - | - | - | - | Skip | Old (pre-2019) |
| [#5833](https://gitlab.com/gnachman/iterm2/-/issues/5833) | Vertical tab bar width shrinks but doesn't grow | - | - | - | - | - | Skip | Old (pre-2019) |
| [#5829](https://gitlab.com/gnachman/iterm2/-/issues/5829) | Tab list should be treated as a stack | - | - | - | - | - | Skip | Old (pre-2019) |
| [#5823](https://gitlab.com/gnachman/iterm2/-/issues/5823) | iTerm Window Style feature in profiles | - | - | - | - | - | Skip | Old (pre-2019) |
| [#5808](https://gitlab.com/gnachman/iterm2/-/issues/5808) | Prevent Creating new Windows when using "quake" dropdown ... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#5796](https://gitlab.com/gnachman/iterm2/-/issues/5796) | Hotkey window now centered instead of top left corner | - | - | - | - | - | Skip | Old (pre-2019) |
| [#5773](https://gitlab.com/gnachman/iterm2/-/issues/5773) | Support the ability to programmatically split panes, run ... | - | - | - | - | - | Skip | Feature request |
| [#5759](https://gitlab.com/gnachman/iterm2/-/issues/5759) | advanced setting: configurable step for keyboard controll... | - | - | - | - | - | Skip | Feature request |
| [#5744](https://gitlab.com/gnachman/iterm2/-/issues/5744) | Feature request: create/resize pane by percentage | - | - | - | - | - | Skip | Feature request |
| [#5741](https://gitlab.com/gnachman/iterm2/-/issues/5741) | window arrangement should restore windows to the proper d... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#5739](https://gitlab.com/gnachman/iterm2/-/issues/5739) | Feature request: Window arrangements open in new tabs | - | - | - | - | - | Skip | Feature request |
| [#5736](https://gitlab.com/gnachman/iterm2/-/issues/5736) | Request: Possible to make window border dimmer? | - | - | - | - | - | Skip | Feature request |
| [#5710](https://gitlab.com/gnachman/iterm2/-/issues/5710) | Terminal becomes dimmed in fullscreen mode (3.1.beta.3) | - | - | - | - | - | Skip | Old (pre-2019) |
| [#5699](https://gitlab.com/gnachman/iterm2/-/issues/5699) | Preserve the Environment Variables when splitting panes | - | - | - | - | - | Skip | Feature request |
| [#5694](https://gitlab.com/gnachman/iterm2/-/issues/5694) | Feature Request: Configure Dock/Application Switcher Hidi... | - | - | - | - | - | Skip | Feature request |
| [#5664](https://gitlab.com/gnachman/iterm2/-/issues/5664) | with iTerm in full-screen mode, cmd+tab or hotkey switchi... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#5648](https://gitlab.com/gnachman/iterm2/-/issues/5648) | [Feature request]: Define initial split ratio via OSA scr... | - | - | - | - | - | Skip | Feature request |
| [#5642](https://gitlab.com/gnachman/iterm2/-/issues/5642) | Windows Disappearing Forever After Disconnecting Second D... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#5620](https://gitlab.com/gnachman/iterm2/-/issues/5620) | Pressing tab twice for completion leaves ugly artefacts o... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#5615](https://gitlab.com/gnachman/iterm2/-/issues/5615) | new tabs/windows don't seem to open to previous working d... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#5596](https://gitlab.com/gnachman/iterm2/-/issues/5596) | After creating a new tab, it does not re-use previous ses... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#5589](https://gitlab.com/gnachman/iterm2/-/issues/5589) | New Tab submenu with profiles & "open all" menu item spon... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#5588](https://gitlab.com/gnachman/iterm2/-/issues/5588) | Error on moving pane to tab. | - | - | - | - | - | Skip | Old (pre-2019) |
| [#5580](https://gitlab.com/gnachman/iterm2/-/issues/5580) | Terminal gap from topOfTheScreen on fullscreen apps | - | - | - | - | - | Skip | Old (pre-2019) |
| [#5573](https://gitlab.com/gnachman/iterm2/-/issues/5573) | Unable to switch tabs using a non-QWERTY keyboard | - | - | - | - | - | Skip | Old (pre-2019) |
| [#5570](https://gitlab.com/gnachman/iterm2/-/issues/5570) | Corrupted Text - Caused by: Full-screen mode, Vim, and Mu... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#5563](https://gitlab.com/gnachman/iterm2/-/issues/5563) | Feature Request: Command K should clear all panes if broa... | - | - | - | - | - | Skip | Feature request |
| [#5561](https://gitlab.com/gnachman/iterm2/-/issues/5561) | the window of Iterm2 doesn't appear | - | - | - | - | - | Skip | Old (pre-2019) |
| [#5548](https://gitlab.com/gnachman/iterm2/-/issues/5548) | All window state lost on iTerm upgrade. | - | - | - | - | - | Skip | Old (pre-2019) |
| [#5545](https://gitlab.com/gnachman/iterm2/-/issues/5545) | Possibility to not automatically open a new default windo... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#5542](https://gitlab.com/gnachman/iterm2/-/issues/5542) | New Tab with Current Profile has illogical placement | - | - | - | - | - | Skip | Old (pre-2019) |
| [#5539](https://gitlab.com/gnachman/iterm2/-/issues/5539) | iterm2 and new windows settings (screen with cursor) | - | - | - | - | - | Skip | Old (pre-2019) |
| [#5519](https://gitlab.com/gnachman/iterm2/-/issues/5519) | Permanently visible window size | - | - | - | - | - | Skip | Feature request |
| [#5511](https://gitlab.com/gnachman/iterm2/-/issues/5511) | Window becomes almost invisible in Mission Control when r... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#5504](https://gitlab.com/gnachman/iterm2/-/issues/5504) | [FEATURE REQUEST] User can save and recall Tab and Profil... | - | - | - | - | - | Skip | Feature request |
| [#5502](https://gitlab.com/gnachman/iterm2/-/issues/5502) | [FEATURE REQUEST] Separate Clear and Clear All Buttons fo... | - | - | - | - | - | Skip | Feature request |
| [#5501](https://gitlab.com/gnachman/iterm2/-/issues/5501) | [FEATURE REQUEST] Ability to Remove Individual Entries fr... | - | - | - | - | - | Skip | Feature request |
| [#5497](https://gitlab.com/gnachman/iterm2/-/issues/5497) | [Feature Request] Keyboard shortcut to swap pane | - | - | - | - | - | Skip | Feature request |
| [#5447](https://gitlab.com/gnachman/iterm2/-/issues/5447) | Pop-up box for choosing colors appears underneath the Pre... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#5441](https://gitlab.com/gnachman/iterm2/-/issues/5441) | Window Arrangements point to wrong working dir | - | - | - | - | - | Skip | Old (pre-2019) |
| [#5439](https://gitlab.com/gnachman/iterm2/-/issues/5439) | Content of window / tab goes "blank" (ALL white) in certa... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#5428](https://gitlab.com/gnachman/iterm2/-/issues/5428) | [Feature Request] - Automatically set margins based on wi... | - | - | - | - | - | Skip | Feature request |
| [#5422](https://gitlab.com/gnachman/iterm2/-/issues/5422) | Feature request: Add option to show per-pane title bar ev... | - | - | - | - | - | Skip | Feature request |
| [#5406](https://gitlab.com/gnachman/iterm2/-/issues/5406) | Terminal always on top when switching windows, starts a e... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#5395](https://gitlab.com/gnachman/iterm2/-/issues/5395) | Don't resize non-selected tabs on window resize until the... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#5384](https://gitlab.com/gnachman/iterm2/-/issues/5384) | Text selection does not work often in Split Pane mode | - | - | - | - | - | Skip | Old (pre-2019) |
| [#5363](https://gitlab.com/gnachman/iterm2/-/issues/5363) | DashTerm2 should allow merging of open windows | - | - | - | - | - | Skip | Feature request |
| [#5358](https://gitlab.com/gnachman/iterm2/-/issues/5358) | Suggestion for the Window tab: Instead of "bash" for most... | - | - | - | - | - | Skip | Feature request |
| [#5344](https://gitlab.com/gnachman/iterm2/-/issues/5344) | Window loses focus after a running a terminal based progr... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#5341](https://gitlab.com/gnachman/iterm2/-/issues/5341) | splitview window resize and restore doesnt restore the te... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#5338](https://gitlab.com/gnachman/iterm2/-/issues/5338) | createTabWithDefaultProfileCommand return ni | - | - | - | - | - | Skip | Old (pre-2019) |
| [#5337](https://gitlab.com/gnachman/iterm2/-/issues/5337) | Dragging a tab out of the tab bar should also drag the ta... | - | - | - | - | - | Skip | Feature request |
| [#5315](https://gitlab.com/gnachman/iterm2/-/issues/5315) | Hide overlay window when mission control is invoked | - | - | - | - | - | Skip | Old (pre-2019) |
| [#5292](https://gitlab.com/gnachman/iterm2/-/issues/5292) | Duplicated / Garbled Text when prompt is longer than wind... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#5263](https://gitlab.com/gnachman/iterm2/-/issues/5263) | iTerm temporarily halted and then all windows resized | - | - | - | - | - | Skip | Old (pre-2019) |
| [#5260](https://gitlab.com/gnachman/iterm2/-/issues/5260) | Other iTerm windows steal focus from visor window | - | - | - | - | - | Skip | Old (pre-2019) |
| [#5258](https://gitlab.com/gnachman/iterm2/-/issues/5258) | Minimized windows restored in a strange state on startup | - | - | - | - | - | Skip | Old (pre-2019) |
| [#5256](https://gitlab.com/gnachman/iterm2/-/issues/5256) | Feature request: tab names should be persistent if entere... | - | - | - | - | - | Skip | Feature request |
| [#5255](https://gitlab.com/gnachman/iterm2/-/issues/5255) | Feature Request Chomeless terminal window | - | - | - | - | - | Skip | Feature request |
| [#5253](https://gitlab.com/gnachman/iterm2/-/issues/5253) | Mouse offset when using vim with multiple splits. Clickin... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#5251](https://gitlab.com/gnachman/iterm2/-/issues/5251) | RFE: preference item requesting that available colors be ... | - | - | - | - | - | Skip | Feature request |
| [#5250](https://gitlab.com/gnachman/iterm2/-/issues/5250) | Opening a new Tab/Window with "Previous Directory" suppor... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#5242](https://gitlab.com/gnachman/iterm2/-/issues/5242) | When a local directory is available use a represented fil... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#5236](https://gitlab.com/gnachman/iterm2/-/issues/5236) | Hotkey window shoud open in current space and screen (fol... | - | - | - | - | - | Skip | Feature request |
| [#5221](https://gitlab.com/gnachman/iterm2/-/issues/5221) | Suggestion: Shortcut for "Next Tab" should be ⌥⌘→ instead... | - | - | - | - | - | Skip | Feature request |
| [#5213](https://gitlab.com/gnachman/iterm2/-/issues/5213) | Window title not retained when switching tabs | - | - | - | - | - | Skip | Old (pre-2019) |
| [#5204](https://gitlab.com/gnachman/iterm2/-/issues/5204) | Tab key no longer expands aliases | - | - | - | - | - | Skip | Old (pre-2019) |
| [#5200](https://gitlab.com/gnachman/iterm2/-/issues/5200) | On smaller splits, indicator behave crazy | - | - | - | - | - | Skip | Old (pre-2019) |
| [#5196](https://gitlab.com/gnachman/iterm2/-/issues/5196) | Center terminal grid when using "Terminal windows resize ... | - | - | - | - | - | Skip | Feature request |
| [#5160](https://gitlab.com/gnachman/iterm2/-/issues/5160) | Iterm2 window focus bug with show/hide iterm2 with system... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#5151](https://gitlab.com/gnachman/iterm2/-/issues/5151) | Terminal Windows not persisted across a software update | - | - | - | - | - | Skip | Old (pre-2019) |
| [#5149](https://gitlab.com/gnachman/iterm2/-/issues/5149) | Hotkey Window completely broken in iTerm Build 3.0.201608... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#5107](https://gitlab.com/gnachman/iterm2/-/issues/5107) | split view resize requires terminal reset | - | - | - | - | - | Skip | Old (pre-2019) |
| [#5088](https://gitlab.com/gnachman/iterm2/-/issues/5088) | Hotkey window hides when focus is lost no matter on setting | - | - | - | - | - | Skip | Old (pre-2019) |
| [#5082](https://gitlab.com/gnachman/iterm2/-/issues/5082) | Flycut pasting going to wrong DashTerm2 window | - | - | - | - | - | Skip | Old (pre-2019) |
| [#5079](https://gitlab.com/gnachman/iterm2/-/issues/5079) | Color picker in fullscreen not accessible | - | - | - | - | - | Skip | Old (pre-2019) |
| [#5035](https://gitlab.com/gnachman/iterm2/-/issues/5035) | weird extra dots in full screen splits | - | - | - | - | - | Skip | Old (pre-2019) |
| [#5013](https://gitlab.com/gnachman/iterm2/-/issues/5013) | Feature request: wider grab-able/pane-resize-handle area | - | - | - | - | - | Skip | Feature request |
| [#5008](https://gitlab.com/gnachman/iterm2/-/issues/5008) | hotkey window show on diffrent display | - | - | - | - | - | Skip | Old (pre-2019) |
| [#4999](https://gitlab.com/gnachman/iterm2/-/issues/4999) | window title doesn't appear correctly | - | - | - | - | - | Skip | Old (pre-2019) |
| [#4984](https://gitlab.com/gnachman/iterm2/-/issues/4984) | Right clicking on empty space in Tab bar should bring up ... | - | - | - | - | - | Skip | Feature request |
| [#4946](https://gitlab.com/gnachman/iterm2/-/issues/4946) | iterm 3 resizes the window on its own | - | - | - | - | - | Skip | Old (pre-2019) |
| [#4908](https://gitlab.com/gnachman/iterm2/-/issues/4908) | System wide hotkey should only show/hide window on curren... | - | - | - | - | - | Skip | Feature request |
| [#4899](https://gitlab.com/gnachman/iterm2/-/issues/4899) | Reorder windows | - | - | - | - | - | Skip | Feature request |
| [#4895](https://gitlab.com/gnachman/iterm2/-/issues/4895) | zsh/zpty process makes DashTerm2 ask for confimation when... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#4875](https://gitlab.com/gnachman/iterm2/-/issues/4875) | Feature request: Improve navigation of the vertical/horiz... | - | - | - | - | - | Skip | Feature request |
| [#4855](https://gitlab.com/gnachman/iterm2/-/issues/4855) | Go back to MRU tab when one closes | - | - | - | - | - | Skip | Feature request |
| [#4852](https://gitlab.com/gnachman/iterm2/-/issues/4852) | Cmd-Shift-O should close "Open Quickly" window | - | - | - | - | - | Skip | Feature request |
| [#4851](https://gitlab.com/gnachman/iterm2/-/issues/4851) | Fullscreen mode tabbing obscures content | - | - | - | - | - | Skip | Old (pre-2019) |
| [#4848](https://gitlab.com/gnachman/iterm2/-/issues/4848) | Feature request: disable tab drag, and/or drag window int... | - | - | - | - | - | Skip | Feature request |
| [#4840](https://gitlab.com/gnachman/iterm2/-/issues/4840) | Animated loading icon causes shift in tab UI | - | - | - | - | - | Skip | Old (pre-2019) |
| [#4839](https://gitlab.com/gnachman/iterm2/-/issues/4839) | New tabs in hotkey window get default (not hotkey window)... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#4803](https://gitlab.com/gnachman/iterm2/-/issues/4803) | Tab bar doesn't appear when pressing Command in full screen | - | - | - | - | - | Skip | Old (pre-2019) |
| [#4793](https://gitlab.com/gnachman/iterm2/-/issues/4793) | Rearranging tabs does not reconfigure shortcuts associate... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#4782](https://gitlab.com/gnachman/iterm2/-/issues/4782) | Windows & tabs merging | - | - | - | - | - | Skip | Feature request |
| [#4776](https://gitlab.com/gnachman/iterm2/-/issues/4776) | password manager window does not appear when terminal is ... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#4749](https://gitlab.com/gnachman/iterm2/-/issues/4749) | Initial "Restore Windows Arrangement" wrong. | - | - | - | - | - | Skip | Old (pre-2019) |
| [#4724](https://gitlab.com/gnachman/iterm2/-/issues/4724) | Cannot open new tabs in Iterm2 | - | - | - | - | - | Skip | Old (pre-2019) |
| [#4711](https://gitlab.com/gnachman/iterm2/-/issues/4711) | Pasting with "Convert tabs to spaces" does not respect ta... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#4693](https://gitlab.com/gnachman/iterm2/-/issues/4693) | Full screen + split screen doesn't work anymore | - | - | - | - | - | Skip | Old (pre-2019) |
| [#4659](https://gitlab.com/gnachman/iterm2/-/issues/4659) | Separate "Hide tab number" and "Hide tab close button" | - | - | - | - | - | Skip | Feature request |
| [#4647](https://gitlab.com/gnachman/iterm2/-/issues/4647) | Enhancement: Add a save terminal/window/tab output | - | - | - | - | - | Skip | Feature request |
| [#4632](https://gitlab.com/gnachman/iterm2/-/issues/4632) | Wrong window selected in multi-monitor setup | - | - | - | - | - | Skip | Old (pre-2019) |
| [#4598](https://gitlab.com/gnachman/iterm2/-/issues/4598) | Allow URL scheme so that a hyperlink can open an DashTerm... | - | - | - | - | - | Skip | Feature request |
| [#4594](https://gitlab.com/gnachman/iterm2/-/issues/4594) | [Suggestion] Parent Window Title for Split Pane Windows | - | - | - | - | - | Skip | Feature request |
| [#4575](https://gitlab.com/gnachman/iterm2/-/issues/4575) | [BUG] iTerm windows become invisible. | - | - | - | - | - | Skip | Old (pre-2019) |
| [#4555](https://gitlab.com/gnachman/iterm2/-/issues/4555) | Feature Request - Allow pop-up window to overlay over ful... | - | - | - | - | - | Skip | Feature request |
| [#4545](https://gitlab.com/gnachman/iterm2/-/issues/4545) | Feature request: update window arrangement | - | - | - | - | - | Skip | Feature request |
| [#4541](https://gitlab.com/gnachman/iterm2/-/issues/4541) | Focused window is not maintained through cmd-tab out and ... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#4535](https://gitlab.com/gnachman/iterm2/-/issues/4535) | Update to oh-my-zsh causes hotkey to no longer hide hotke... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#4474](https://gitlab.com/gnachman/iterm2/-/issues/4474) | Window vertically off-screen when OSX dock hiding is enabled | - | - | - | - | - | Skip | Old (pre-2019) |
| [#4424](https://gitlab.com/gnachman/iterm2/-/issues/4424) | Could not open any window anymore | - | - | - | - | - | Skip | Old (pre-2019) |
| [#4408](https://gitlab.com/gnachman/iterm2/-/issues/4408) | Undo Close on panel reopens the panel, but can't receive ... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#4372](https://gitlab.com/gnachman/iterm2/-/issues/4372) | Feature: Make "Show profile name (in tab/window)" a per-p... | - | - | - | - | - | Skip | Feature request |
| [#4329](https://gitlab.com/gnachman/iterm2/-/issues/4329) | Focus Follow Mouse ignore preference window | - | - | - | - | - | Skip | Old (pre-2019) |
| [#4316](https://gitlab.com/gnachman/iterm2/-/issues/4316) | Silence bell tooltip window reappearing and not accepting... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#4315](https://gitlab.com/gnachman/iterm2/-/issues/4315) | Zooming resizes window | - | - | - | - | - | Skip | Old (pre-2019) |
| [#4271](https://gitlab.com/gnachman/iterm2/-/issues/4271) | Feature proposal: an option to have the toolbelt and the ... | - | - | - | - | - | Skip | Feature request |
| [#4207](https://gitlab.com/gnachman/iterm2/-/issues/4207) | Can resize split pan until per-pane title bar is clicked | - | - | - | - | - | Skip | Old (pre-2019) |
| [#4179](https://gitlab.com/gnachman/iterm2/-/issues/4179) | Close all the panes in the current tab except the focused... | - | - | - | - | - | Skip | Feature request |
| [#4142](https://gitlab.com/gnachman/iterm2/-/issues/4142) | focus "Quit DashTerm2? dialog window after starting "Inst... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#4086](https://gitlab.com/gnachman/iterm2/-/issues/4086) | Feature request: a right-click menu on the tab bar. | - | - | - | - | - | Skip | Feature request |
| [#4069](https://gitlab.com/gnachman/iterm2/-/issues/4069) | "center" fullscreen mode | - | - | - | - | - | Skip | Feature request |
| [#4020](https://gitlab.com/gnachman/iterm2/-/issues/4020) | Tab bar on right | - | - | - | - | - | Skip | Feature request |
| [#3981](https://gitlab.com/gnachman/iterm2/-/issues/3981) | Switching tabs using Command-[arrow-key] is broken | - | - | - | - | - | Skip | Old (pre-2019) |
| [#3977](https://gitlab.com/gnachman/iterm2/-/issues/3977) | Window loses focus when clicked. | - | - | - | - | - | Skip | Old (pre-2019) |
| [#3938](https://gitlab.com/gnachman/iterm2/-/issues/3938) | Consider delayed resize of inactive tabs | - | - | - | - | - | Skip | Feature request |
| [#3926](https://gitlab.com/gnachman/iterm2/-/issues/3926) | multiple layered windows | - | - | - | - | - | Skip | Feature request |
| [#3924](https://gitlab.com/gnachman/iterm2/-/issues/3924) | Window blur issue on screenshot | - | - | - | - | - | Skip | Old (pre-2019) |
| [#3920](https://gitlab.com/gnachman/iterm2/-/issues/3920) | Cmd-Tab from another application switches to a different ... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#3873](https://gitlab.com/gnachman/iterm2/-/issues/3873) | Feature request: Exportable/publishable and importable/su... | - | - | - | - | - | Skip | Feature request |
| [#3856](https://gitlab.com/gnachman/iterm2/-/issues/3856) | Option to allow new panes to inherit title of 'parent' pane | - | - | - | - | - | Skip | Feature request |
| [#3830](https://gitlab.com/gnachman/iterm2/-/issues/3830) | Feature request: remember last window size | - | - | - | - | - | Skip | Feature request |
| [#3793](https://gitlab.com/gnachman/iterm2/-/issues/3793) | Fixed pane on each tab | - | - | - | - | - | Skip | Feature request |
| [#3754](https://gitlab.com/gnachman/iterm2/-/issues/3754) | Fullscreen focus issue when switching spaces | - | - | - | - | - | Skip | Old (pre-2019) |
| [#3743](https://gitlab.com/gnachman/iterm2/-/issues/3743) | Feature request: Automatic/dynamic resizing of panes - ac... | - | - | - | - | - | Skip | Feature request |
| [#3732](https://gitlab.com/gnachman/iterm2/-/issues/3732) | Feature Request: [Keys] Map a separate key as a "Close ho... | - | - | - | - | - | Skip | Feature request |
| [#3719](https://gitlab.com/gnachman/iterm2/-/issues/3719) | Add ability to search for settings in preferences window | - | - | - | - | - | Skip | Feature request |
| [#3714](https://gitlab.com/gnachman/iterm2/-/issues/3714) | Transparency and blur stops working when NSFullSizeConten... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#3676](https://gitlab.com/gnachman/iterm2/-/issues/3676) | Dead DashTerm2 windows. | - | - | - | - | - | Skip | Old (pre-2019) |
| [#3674](https://gitlab.com/gnachman/iterm2/-/issues/3674) | New tabs in the hotkey window don't use the hotkey window... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#3658](https://gitlab.com/gnachman/iterm2/-/issues/3658) | Adding a new Tabbar Style: Flat | - | - | - | - | - | Skip | Feature request |
| [#3643](https://gitlab.com/gnachman/iterm2/-/issues/3643) | Programmatically enable/disable and set/unset the name of... | - | - | - | - | - | Skip | Feature request |
| [#3640](https://gitlab.com/gnachman/iterm2/-/issues/3640) | allow middle click paste to also give window focus | - | - | - | - | - | Skip | Feature request |
| [#3626](https://gitlab.com/gnachman/iterm2/-/issues/3626) | Window gap on LHS of display on OSX 10.11 Beta | - | - | - | - | - | Skip | Old (pre-2019) |
| [#3589](https://gitlab.com/gnachman/iterm2/-/issues/3589) | Remember size & position of Profiles window, and whether ... | - | - | - | - | - | Skip | Feature request |
| [#3576](https://gitlab.com/gnachman/iterm2/-/issues/3576) | Add the option to kill child processes with signal 9 [was... | - | - | - | - | - | Skip | Feature request |
| [#3547](https://gitlab.com/gnachman/iterm2/-/issues/3547) | Add button for new tab | - | - | - | - | - | Skip | Feature request |
| [#3536](https://gitlab.com/gnachman/iterm2/-/issues/3536) | Feature request: A "new window like this" right-click men... | - | - | - | - | - | Skip | Feature request |
| [#3488](https://gitlab.com/gnachman/iterm2/-/issues/3488) | multi-column or window display  for one session/screen | - | - | - | - | - | Skip | Feature request |
| [#3450](https://gitlab.com/gnachman/iterm2/-/issues/3450) | Maximized window resizes when closing all but the first tab | - | - | - | - | - | Skip | Old (pre-2019) |
| [#3445](https://gitlab.com/gnachman/iterm2/-/issues/3445) | Show Hide Terminal window should have option to follow mo... | - | - | - | - | - | Skip | Feature request |
| [#3430](https://gitlab.com/gnachman/iterm2/-/issues/3430) | Split window without resizing other parts | - | - | - | - | - | Skip | Feature request |
| [#3417](https://gitlab.com/gnachman/iterm2/-/issues/3417) | Profile->Window->Style->Fullscreen is a little bit from s... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#3392](https://gitlab.com/gnachman/iterm2/-/issues/3392) | Permanently disable tab bar | - | - | - | - | - | Skip | Feature request |
| [#3380](https://gitlab.com/gnachman/iterm2/-/issues/3380) | Use Transparency does nothing and is confusing when windo... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#3372](https://gitlab.com/gnachman/iterm2/-/issues/3372) | Broadcast Input to multiple but not all tabs | - | - | - | - | - | Skip | Feature request |
| [#3223](https://gitlab.com/gnachman/iterm2/-/issues/3223) | Center terminal window in full-screen mode | - | - | - | - | - | Skip | Feature request |
| [#3215](https://gitlab.com/gnachman/iterm2/-/issues/3215) | AppleScript possibility to set background image (per tab) | - | - | - | - | - | Skip | Feature request |
| [#3192](https://gitlab.com/gnachman/iterm2/-/issues/3192) | Scripting: add reverse hierarchy accessors for windows/ta... | - | - | - | - | - | Skip | Feature request |
| [#3184](https://gitlab.com/gnachman/iterm2/-/issues/3184) | 'Sticky' tabs | - | - | - | - | - | Skip | Feature request |
| [#3167](https://gitlab.com/gnachman/iterm2/-/issues/3167) | Let background image stretch to fit whole window instead ... | - | - | - | - | - | Skip | Feature request |
| [#3166](https://gitlab.com/gnachman/iterm2/-/issues/3166) | Profiles > Window > Style: Zoom/Maximized missing. | - | - | - | - | - | Skip | Feature request |
| [#3139](https://gitlab.com/gnachman/iterm2/-/issues/3139) | Show which tabs will alert to close | - | - | - | - | - | Skip | Feature request |
| [#3137](https://gitlab.com/gnachman/iterm2/-/issues/3137) | restore window arrange should bring back last command use... | - | - | - | - | - | Skip | Feature request |
| [#3116](https://gitlab.com/gnachman/iterm2/-/issues/3116) | Command-Backtick (Cmd-Tilde) doesn't work when fullscreen | - | - | - | - | - | Skip | Old (pre-2019) |
| [#3090](https://gitlab.com/gnachman/iterm2/-/issues/3090) | Make iTerm remember which terminal window/tab was selecte... | - | - | - | - | - | Skip | Feature request |
| [#3070](https://gitlab.com/gnachman/iterm2/-/issues/3070) | ability to perform different searches in different panes | - | - | - | - | - | Skip | Feature request |
| [#3023](https://gitlab.com/gnachman/iterm2/-/issues/3023) | random appearance setting for new tab | - | - | - | - | - | Skip | Feature request |
| [#2990](https://gitlab.com/gnachman/iterm2/-/issues/2990) | set a title to a window which is grouping a set of tabs | - | - | - | - | - | Skip | Feature request |
| [#2942](https://gitlab.com/gnachman/iterm2/-/issues/2942) | Windows sized wrong after post-panic reopen | - | - | - | - | - | Skip | Old (pre-2019) |
| [#2871](https://gitlab.com/gnachman/iterm2/-/issues/2871) | Detached coloured tab is missing uncolouring | - | - | - | - | - | Skip | Old (pre-2019) |
| [#2857](https://gitlab.com/gnachman/iterm2/-/issues/2857) | Allow naming of Windows | - | - | - | - | - | Skip | Feature request |
| [#2835](https://gitlab.com/gnachman/iterm2/-/issues/2835) | Incorrect handling of window title sequence | - | - | - | - | - | Skip | Old (pre-2019) |
| [#2668](https://gitlab.com/gnachman/iterm2/-/issues/2668) | Window Arrangement Title | - | - | - | - | - | Skip | Feature request |
| [#2600](https://gitlab.com/gnachman/iterm2/-/issues/2600) | Shortcut key to activate tab also cycles through panes | - | - | - | - | - | Skip | Feature request |
| [#2541](https://gitlab.com/gnachman/iterm2/-/issues/2541) | Support tab groups like Firefox | - | - | - | - | - | Skip | Feature request |
| [#2495](https://gitlab.com/gnachman/iterm2/-/issues/2495) | Wallpaper over all splits | - | - | - | - | - | Skip | Feature request |
| [#2368](https://gitlab.com/gnachman/iterm2/-/issues/2368) | Tabs in slit panels | - | - | - | - | - | Skip | Feature request |
| [#2353](https://gitlab.com/gnachman/iterm2/-/issues/2353) | Text background color that overrides generic window backg... | - | - | - | - | - | Skip | Feature request |
| [#2265](https://gitlab.com/gnachman/iterm2/-/issues/2265) | Window arrangements opened at launch are two columns wide... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#2234](https://gitlab.com/gnachman/iterm2/-/issues/2234) | Add Chrome's Pin Tab Feature | - | - | - | - | - | Skip | Feature request |
| [#2195](https://gitlab.com/gnachman/iterm2/-/issues/2195) | Switch to disable circular pane selection. | - | - | - | - | - | Skip | Feature request |
| [#2194](https://gitlab.com/gnachman/iterm2/-/issues/2194) | ability cycle through windows in pre-determined order | - | - | - | - | - | Skip | Feature request |
| [#2140](https://gitlab.com/gnachman/iterm2/-/issues/2140) | 'alt-tab' among windows within only the current display | - | - | - | - | - | Skip | Feature request |
| [#2127](https://gitlab.com/gnachman/iterm2/-/issues/2127) | a fullscreen mode that would work well with transparency | - | - | - | - | - | Skip | Feature request |
| [#2107](https://gitlab.com/gnachman/iterm2/-/issues/2107) | show dimensions of all panes when reizing | - | - | - | - | - | Skip | Feature request |
| [#2074](https://gitlab.com/gnachman/iterm2/-/issues/2074) | Option to preserve overall window size when toggling tabs | - | - | - | - | - | Skip | Feature request |
| [#2003](https://gitlab.com/gnachman/iterm2/-/issues/2003) | 'Show border around window' menu and hotkey | - | - | - | - | - | Skip | Feature request |
| [#1996](https://gitlab.com/gnachman/iterm2/-/issues/1996) | iterm2 window needs to be resized in order to see enough ... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#1890](https://gitlab.com/gnachman/iterm2/-/issues/1890) | Tab Groups | - | - | - | - | - | Skip | Feature request |
| [#1852](https://gitlab.com/gnachman/iterm2/-/issues/1852) | Method of determining width of tab title text-field | - | - | - | - | - | Skip | Feature request |
| [#1743](https://gitlab.com/gnachman/iterm2/-/issues/1743) | Color active pane border different from inactive ones | - | - | - | - | - | Skip | Feature request |
| [#1728](https://gitlab.com/gnachman/iterm2/-/issues/1728) | Page up, page down, home, end in profiles window | - | - | - | - | - | Skip | Feature request |
| [#1708](https://gitlab.com/gnachman/iterm2/-/issues/1708) | Perform text selection with keyboard [was: keyboard movem... | - | - | - | - | - | Skip | Feature request |
| [#1706](https://gitlab.com/gnachman/iterm2/-/issues/1706) | Open new window in same monitor | - | - | - | - | - | Skip | Feature request |
| [#1698](https://gitlab.com/gnachman/iterm2/-/issues/1698) | Offer a "Cascade" arrangement of windows | - | - | - | - | - | Skip | Feature request |
| [#1649](https://gitlab.com/gnachman/iterm2/-/issues/1649) | "New window like this" command, from right-click menu | - | - | - | - | - | Skip | Feature request |
| [#1577](https://gitlab.com/gnachman/iterm2/-/issues/1577) | Switching to full screen discards input while window is o... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#1529](https://gitlab.com/gnachman/iterm2/-/issues/1529) | Possibility to have opaque selection in transparent window | - | - | - | - | - | Skip | Feature request |
| [#1343](https://gitlab.com/gnachman/iterm2/-/issues/1343) | GNOME-ish command-click mouse move/resize of panes | - | - | - | - | - | Skip | Feature request |
| [#1288](https://gitlab.com/gnachman/iterm2/-/issues/1288) | Split panes in Profiles | - | - | - | - | - | Skip | Feature request |
| [#1264](https://gitlab.com/gnachman/iterm2/-/issues/1264) | Enable a "master pane/area" like a tiling wm (e.g xmonad) | - | - | - | - | - | Skip | Feature request |
| [#1201](https://gitlab.com/gnachman/iterm2/-/issues/1201) | Terminal window bottom corners are incorrect under Lion | - | - | - | - | - | Skip | Old (pre-2019) |
| [#1135](https://gitlab.com/gnachman/iterm2/-/issues/1135) | Use background of terminal tabs as a progress bar | - | - | - | - | - | Skip | Feature request |
| [#1071](https://gitlab.com/gnachman/iterm2/-/issues/1071) | Hotkey Window new tabs should also use Hotkey Window prof... | - | - | - | - | - | Skip | Feature request |
| [#1045](https://gitlab.com/gnachman/iterm2/-/issues/1045) | Smart window placement works great... on the first monitor | - | - | - | - | - | Skip | Feature request |
| [#1043](https://gitlab.com/gnachman/iterm2/-/issues/1043) | Key shortcut for "command-K for all splits on current tab" | - | - | - | - | - | Skip | Feature request |
| [#1018](https://gitlab.com/gnachman/iterm2/-/issues/1018) | WMII Like movement of split panes | - | - | - | - | - | Skip | Feature request |
| [#1005](https://gitlab.com/gnachman/iterm2/-/issues/1005) | Add help button to config panels that opens the html help... | - | - | - | - | - | Skip | Feature request |
| [#998](https://gitlab.com/gnachman/iterm2/-/issues/998) | Warn if command is login without -l argument and you've s... | - | - | - | - | - | Skip | Feature request |
| [#810](https://gitlab.com/gnachman/iterm2/-/issues/810) | NON CRITICAL - Colorspace conversion locked at window lau... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#694](https://gitlab.com/gnachman/iterm2/-/issues/694) | Merge all windows | - | - | - | - | - | Skip | Feature request |
| [#611](https://gitlab.com/gnachman/iterm2/-/issues/611) | "Chrome Confirm to quit" for closing windows | - | - | - | - | - | Skip | Feature request |
| [#526](https://gitlab.com/gnachman/iterm2/-/issues/526) | move panes around | - | - | - | - | - | Skip | Feature request |
| [#276](https://gitlab.com/gnachman/iterm2/-/issues/276) | Native fullscreen transparency should show vibrant deskto... | - | - | - | - | - | Skip | Feature request |

---

## Statistics

| Metric | Count |
|--------|-------|
| Total | 667 |
| Skip | 621 |
| Fixed | 44 |
| In Progress | 0 |
| Open | 0 |
| Cannot Reproduce | 3 |
| External | 1 |

---

## Category Notes

P2 Window/Tab/Pane triage **COMPLETE**. All 667 issues categorized. This is the largest category in the burn list. Window/Tab/Pane issues cover a wide range of functionality including hotkey windows, tab management, split panes, fullscreen modes, and multi-monitor support.

### Common Patterns

1. **Hotkey window issues** - Focus loss, wrong display, hiding/showing behavior
2. **Window restoration** - Wrong position, wrong Space, wrong size after restart
3. **Multi-monitor issues** - Windows on wrong display, wrong position after disconnect
4. **Tab bar problems** - Disappearing tabs, wrong colors, fullscreen issues
5. **Focus issues** - Window steals focus, loses focus on switch, focus follows mouse
6. **Split pane issues** - Selection problems, resize issues, broadcast input
7. **Fullscreen mode** - Tab bar visibility, flickering, wrong size
8. **Window arrangement** - Not restoring correctly, wrong working directory

### Related Files

- `sources/PseudoTerminal.m` - Main window controller
- `sources/PTYTab.m` - Tab management
- `sources/PTYSession.m` - Session (pane) handling
- `sources/iTermHotKeyController.m` - Hotkey window management
- `sources/iTermApplicationDelegate.m` - Window restoration
- `ThirdParty/PSMTabBarControl/` - Tab bar implementation
- `sources/iTermWindowRestorer.m` - Window state restoration
- `sources/iTermFullScreenWindowManager.m` - Fullscreen handling

