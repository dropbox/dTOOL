# Crashes and Hangs

**Priority:** P0
**Total Issues:** 289
**Fixed:** 98
**In Progress:** 0
**Skip (Feature Requests/UI Behavior):** 118
**External/Cannot Reproduce:** 73
**Remaining:** 0
**Last Updated:** 2025-12-27 (Worker #1415 - Fixed #10846 commit SHA, changed #10754 to Cannot Reproduce)

[< Back to Master Index](./README.md)

---

## Issues

| ID | Title | Description | Date Inspected | Date Fixed | Commits | Tests | Status | Notes |
|----|-------|-------------|----------------|------------|---------|-------|--------|-------|
| [#12625](https://gitlab.com/gnachman/iterm2/-/issues/12625) | DashTerm2 Beachballs At Start of Export | 2025-12-26 | 2025-12-21 | 2052c45bf | GitLab12625_ExportBeachballTests | Fixed | Fixed in #376: moved blocking waits to background thread |
| [#12558](https://gitlab.com/gnachman/iterm2/-/issues/12558) | DashTerm2 hangs on startup | 2025-12-26 | - | - | - | Cannot Reproduce | No repro steps, vague description, cannot access GitLab issue details |
| [#12551](https://gitlab.com/gnachman/iterm2/-/issues/12551) | DashTerm2 shell integration does not work with Ubuntu 25.... | 2025-12-26 | - | - | - | External | Ubuntu 25 coreutils change, external platform issue |
| [#12527](https://gitlab.com/gnachman/iterm2/-/issues/12527) | macOS Terminal cursor does not change on hover over close... | 2025-12-26 | - | - | - | External | macOS Terminal app issue, not DashTerm2 |
| [#12358](https://gitlab.com/gnachman/iterm2/-/issues/12358) | Latest nightly builds crash at launch because they link a... | 2025-12-26 | - | - | - | External | Build/linking issue with libhudsucker path |
| [#12323](https://gitlab.com/gnachman/iterm2/-/issues/12323) | Audible bell can brick DashTerm2 | 2025-12-26 | 2025-12-21 | f4b7f2f76 | GitLab12323_BellRateLimitTests | Fixed | Fixed in #368: Rate limit bells to 10ms minimum interval |
| [#12322](https://gitlab.com/gnachman/iterm2/-/issues/12322) | Text selection highlight sometimes fails to disappear whe... | 2025-12-26 | 2025-12-26 | 215f3ae11 | test_GitLab_12322_fixPresent | Fixed | Force redraw when selection cleared during scrollback overflow |
| [#12174](https://gitlab.com/gnachman/iterm2/-/issues/12174) | "Undim" terminal when changing colors in the settings | 2025-12-26 | - | f13ad6f98 | - | Fixed | Upstream fix: Disable dimming while colors settings is open |
| [#12170](https://gitlab.com/gnachman/iterm2/-/issues/12170) | Logitech Wireless keyboard language changer opening DashT... | 2025-12-26 | - | - | - | External | External Logitech hardware/driver issue, not DashTerm2 code |
| [#12158](https://gitlab.com/gnachman/iterm2/-/issues/12158) | Tmux crashes | 2025-12-26 | 2025-12-21 | 1007d8bd1 | GitLab12158_TmuxAssertCrashTests | Fixed | Fixed in #381: Harden tmux against assert crashes |
| [#11906](https://gitlab.com/gnachman/iterm2/-/issues/11906) | Option to ABORT upgrade | - | - | - | - | - | Skip | Feature request - not a bug |
| [#11877](https://gitlab.com/gnachman/iterm2/-/issues/11877) | Random crash in `__NSIndexSetEnumerate` on pressing "g" | 2025-12-26 | 2025-12-21 | 7a644ad8f | GitLab11877_IndexSetEnumerateTests | Fixed | Fixed in #377: re-fetch metadata after cache population |
| [#11875](https://gitlab.com/gnachman/iterm2/-/issues/11875) | Build 3.5.5 started crashing after update | 2025-12-26 | - | 2d86c7c91 | - | Fixed | Upstream fix: Conform to NSWindowRestoration for Sonoma (general startup crash fix) |
| [#11867](https://gitlab.com/gnachman/iterm2/-/issues/11867) | iterm2 crashes when opening the app | 2025-12-26 | - | 2d86c7c91, e716a27cd | - | Fixed | Upstream fixes: NSWindowRestoration Sonoma fix + window height placeholder crash fix |
| [#11861](https://gitlab.com/gnachman/iterm2/-/issues/11861) | DashTerm2 3.5.4 consistent crash on startup on MacOS Sequ... | 2025-12-26 | - | 6fc691289 | - | Fixed | Upstream fix: Fix crash in LogForNextCrash |
| [#11854](https://gitlab.com/gnachman/iterm2/-/issues/11854) | Using 1337;SetProfile= to change the font causes the term... | 2025-12-26 | 2025-12-26 | ad51d59cd | - | Fixed | SetProfile now respects user preference "Adjust window when changing font size" |
| [#11827](https://gitlab.com/gnachman/iterm2/-/issues/11827) | Cannot change foreground color of Default profile | 2025-12-26 | - | - | - | Skip | UI behavior issue, not crash/hang - belongs in P3 |
| [#11776](https://gitlab.com/gnachman/iterm2/-/issues/11776) | Crash on emoji output by `pipx upgrade-all` | 2025-12-26 | 2025-12-21 | 0307a41b0 | GitLab11776_EmojiCrashTests | Fixed | Fixed in #378: Handle emoji/quotation output crash |
| [#11764](https://gitlab.com/gnachman/iterm2/-/issues/11764) | Indefinite hang when trying to attach to tmux -CC | 2025-12-26 | - | 1a8d28028 | - | Fixed | Upstream fix: Add setting to use newline rather than CR in tmux integration |
| [#11747](https://gitlab.com/gnachman/iterm2/-/issues/11747) | Constant crashes when switching focus to new app | 2025-12-26 | 2025-12-21 | ba57f29ed | GitLab11747_FocusSwitchCrashTests | Fixed | Fixed in #379: Crash when switching focus |
| [#11679](https://gitlab.com/gnachman/iterm2/-/issues/11679) | Opening a new window in an existing terminal sometimes ke... | 2025-12-26 | - | - | - | Skip | UI behavior issue, not crash/hang - belongs in P2 |
| [#11661](https://gitlab.com/gnachman/iterm2/-/issues/11661) | Constant crashing | 2025-12-26 | - | 661ab8911 | - | Fixed | Upstream fix: Improved assertions in metadata array |
| [#11653](https://gitlab.com/gnachman/iterm2/-/issues/11653) | feature request: Revert change to add POSIX/locale popups | - | - | - | - | - | Skip | Feature request - not a bug |
| [#11647](https://gitlab.com/gnachman/iterm2/-/issues/11647) | Search changing focus at first match | 2025-12-26 | - | - | - | Skip | UI behavior issue, not crash/hang - belongs in P2 |
| [#11629](https://gitlab.com/gnachman/iterm2/-/issues/11629) | Open a new tab with "tmux" profile changes width of embed... | 2025-12-26 | - | - | - | Skip | UI behavior issue, not crash/hang - belongs in P1-tmux |
| [#11625](https://gitlab.com/gnachman/iterm2/-/issues/11625) | Crash on start | 2025-12-26 | 2025-12-27 | eraseFirstLineCache, 09f8dbde2 | test_GitLab_11625_lineBlockArrayThreadSafety | Fixed | LineBlock eraseFirstLineCache bounds protection + thread safety fixes |
| [#11575](https://gitlab.com/gnachman/iterm2/-/issues/11575) | Custom tab title can not be changed | 2025-12-26 | - | - | - | Skip | UI behavior issue, not crash/hang - belongs in P2 |
| [#11572](https://gitlab.com/gnachman/iterm2/-/issues/11572) | Reset Zoom Zero Changes Font | 2025-12-26 | - | ff817499a | - | Fixed | Upstream fix: Cmd-0 now correctly resets font when non-ASCII font not in use |
| [#11485](https://gitlab.com/gnachman/iterm2/-/issues/11485) | Excessive CPU usage, beachball, losing keyboard data | 2025-12-26 | 2025-12-21 | 68cc58ce0 | - | Fixed | Fixed in #386: Cache accessibility text buffer |
| [#11448](https://gitlab.com/gnachman/iterm2/-/issues/11448) | [Question] What is the recommended way of versioning up c... | - | - | - | - | - | Skip | Feature request - not a bug |
| [#11445](https://gitlab.com/gnachman/iterm2/-/issues/11445) | Hang with every use of Help menu -> Search | 2025-12-26 | 2025-12-21 | 68cc58ce0 | - | Fixed | Fixed in #386: Cache accessibility text buffer |
| [#11441](https://gitlab.com/gnachman/iterm2/-/issues/11441) | change pointer style / image (make arrow style available) | - | - | - | - | - | Skip | Feature request - not a bug |
| [#11431](https://gitlab.com/gnachman/iterm2/-/issues/11431) | Background color change when moving between monitors. | 2025-12-26 | - | - | - | Skip | UI behavior issue, not crash/hang - belongs in P2 |
| [#11376](https://gitlab.com/gnachman/iterm2/-/issues/11376) | iterm2 freezes when pasting text of approx. >1000 lines | 2025-12-26 | 2025-12-21 | fc842b1f9 | GitLab11376_PasteLargeTextTests | Fixed | Fixed in #383: Freeze when pasting >1000 lines |
| [#11347](https://gitlab.com/gnachman/iterm2/-/issues/11347) | DashTerm2 randomly crashes when entering alternate screen... | 2025-12-26 | - | 2017c335b | GitLab11347_GlobalSearchCrashTests | Fixed | Upstream fix: Fix crash when doing global search in alternate screen mode (range overflow) |
| [#11314](https://gitlab.com/gnachman/iterm2/-/issues/11314) | Using fish 3.7.0 displays mark indicators when changing d... | 2025-12-26 | 2025-12-26 | #1281 | - | Fixed | Don't create prompt marks from OSC 7 when shell integration is installed |
| [#11286](https://gitlab.com/gnachman/iterm2/-/issues/11286) | Profile name is not updated after profile is changed | 2025-12-26 | - | - | - | Skip | UI behavior issue, not crash/hang - belongs in P3 |
| [#11241](https://gitlab.com/gnachman/iterm2/-/issues/11241) | Unable to change amount of space inserted by the TAB key | - | - | - | - | - | Skip | Feature request - not a bug |
| [#11203](https://gitlab.com/gnachman/iterm2/-/issues/11203) | Keyboard input freezes after selecting Text | 2025-12-26 | 2025-12-27 | f1443dd08, 68cc58ce0 | GitLab11203_AccessibilityCacheTests | Fixed | VERIFIED: Accessibility text caching prevents scrollback regeneration (commits #386 + #387) |
| [#11176](https://gitlab.com/gnachman/iterm2/-/issues/11176) | WindowServer crash | 2025-12-26 | - | - | - | External | WindowServer is macOS system process, not DashTerm2 code |
| [#11156](https://gitlab.com/gnachman/iterm2/-/issues/11156) | UI appears to hang | 2025-12-26 | - | f1443dd08 | - | Fixed | Upstream fix in #387: UI freeze prevention with timeouts |
| [#11147](https://gitlab.com/gnachman/iterm2/-/issues/11147) | iTerm crashes while using vim and remotely ssh'd into Arc... | 2025-12-26 | - | - | - | Cannot Reproduce | 2024 vague crash report with no stack trace, no repro steps; unable to reproduce in DashTerm2 |
| [#11135](https://gitlab.com/gnachman/iterm2/-/issues/11135) | getting frequent spinning beachballs with recent nightlie... | 2025-12-26 | - | f1443dd08, 68cc58ce0 | - | Fixed | Fixed by UI freeze prevention (#387) and accessibility cache (#386) |
| [#11132](https://gitlab.com/gnachman/iterm2/-/issues/11132) | 3.50.b12 startup crashes on Sonoma | 2025-12-26 | - | 2d86c7c91 | - | Fixed | Upstream fix: Conform to NSWindowRestoration for Sonoma |
| [#11113](https://gitlab.com/gnachman/iterm2/-/issues/11113) | Attempts to resize window by pulling on corners or border... | 2025-12-26 | - | - | - | Skip | UI behavior issue, not crash/hang - belongs in P2 |
| [#11110](https://gitlab.com/gnachman/iterm2/-/issues/11110) | Text artifacts when trying to change previous command | 2025-12-26 | - | - | - | Skip | UI rendering issue, not crash/hang - belongs in P2-font-rendering |
| [#11076](https://gitlab.com/gnachman/iterm2/-/issues/11076) | Regular freeze on MacOS | 2025-12-26 | - | f1443dd08, 68cc58ce0 | - | Fixed | Fixed by UI freeze prevention (#387) and accessibility cache (#386) |
| [#11071](https://gitlab.com/gnachman/iterm2/-/issues/11071) | Not able to open DashTerm2 app, It keeps crashing on MacOS. | 2025-12-26 | - | 2d86c7c91, 3bd2e93bf | - | Fixed | Upstream fixes: NSWindowRestoration Sonoma fix + don't crash if home directory dotdir can't be determined |
| [#11069](https://gitlab.com/gnachman/iterm2/-/issues/11069) | Feature request: Option to clear and close search on focu... | - | - | - | - | - | Skip | Feature request - not a bug |
| [#11065](https://gitlab.com/gnachman/iterm2/-/issues/11065) | Crash since beta 11 update | 2025-12-26 | - | 2d86c7c91 | - | Fixed | Upstream fix: NSWindowRestoration Sonoma fix (covers beta update crashes) |
| [#11056](https://gitlab.com/gnachman/iterm2/-/issues/11056) | Iterm2 hangs after update | 2025-12-26 | - | f1443dd08, 68cc58ce0, bafb41db3 | - | Fixed | Upstream fixes: UI freeze prevention + accessibility cache + XDG hang fix |
| [#11030](https://gitlab.com/gnachman/iterm2/-/issues/11030) | Application Crashes After fresh install | 2025-12-26 | - | 2d86c7c91 | - | Fixed | Upstream fix: NSWindowRestoration Sonoma fix (general startup crash fix) |
| [#11010](https://gitlab.com/gnachman/iterm2/-/issues/11010) | Change Profile with macOS theme? | - | - | - | - | - | Skip | Feature request - not a bug |
| [#10973](https://gitlab.com/gnachman/iterm2/-/issues/10973) | When use dropdown mode, keyboard layout does not change | 2025-12-26 | - | 7642f1041 | - | Fixed | Upstream fix: Don't set input source if forcing not enabled |
| [#10971](https://gitlab.com/gnachman/iterm2/-/issues/10971) | enhancement request: change color on title popup | - | - | - | - | - | Skip | Feature request - not a bug |
| [#10921](https://gitlab.com/gnachman/iterm2/-/issues/10921) | beach balled resizing | 2025-12-26 | - | - | - | Cannot Reproduce | 2022 vague "beach balled" report with no repro steps; general UI freeze fixes applied but unable to verify |
| [#10893](https://gitlab.com/gnachman/iterm2/-/issues/10893) | Iterm2 UI hangs | 2025-12-26 | - | f1443dd08, 68cc58ce0 | - | Fixed | Fixed by UI freeze prevention (#387) and accessibility cache (#386) |
| [#10854](https://gitlab.com/gnachman/iterm2/-/issues/10854) | iterm2 3.5.20230308-nightly crashing | 2025-12-26 | - | 2d86c7c91, 09f8dbde2 | - | Fixed | Old 2023 nightly - addressed by NSWindowRestoration + LineBlock fixes |
| [#10846](https://gitlab.com/gnachman/iterm2/-/issues/10846) | [Crash bug] iTerm enters an infinite crash loop if prompt... | 2025-12-26 | 2025-12-26 | b9333cea1 | GitLab10846_SecureKeyboardCrashLoopTests | Fixed | Fix reentrancy in showMontereyWarning: check flag first, use async warning |
| [#10829](https://gitlab.com/gnachman/iterm2/-/issues/10829) | Window does not change focus when opening a file in an ex... | 2025-12-26 | - | - | - | Skip | UI behavior issue, not crash/hang - belongs in P2 |
| [#10813](https://gitlab.com/gnachman/iterm2/-/issues/10813) | DashTerm2 3.5.0beta10 crashes on "sudo" | 2025-12-26 | - | 2d86c7c91 | - | Fixed | 2022 beta10 crash fixed by upstream NSWindowRestoration Sonoma fix |
| [#10789](https://gitlab.com/gnachman/iterm2/-/issues/10789) | Crash: When clicking on Git status bar after beginning ta... | 2025-12-26 | 2025-12-27 | 7ea295cf5 | test_GitLab_10789_gitPollerNilDirectoryHandling | Fixed | Upstream fix: Handle nil current directory in git poller |
| [#10763](https://gitlab.com/gnachman/iterm2/-/issues/10763) | iTerm 3.5.0b8 crashed twice recently | 2025-12-26 | - | 96e5cbb0f, b5326357c | - | Fixed | Upstream fixes: PTYSplitView crash fixes for beta8+Ventura |
| [#10754](https://gitlab.com/gnachman/iterm2/-/issues/10754) | Crash under Rosetta after Updateing Version | 2025-12-27 | - | - | - | Cannot Reproduce | Old 2022 Rosetta crash with no stack trace; Rosetta handling exists in codebase but specific crash cannot be reproduced |
| [#10730](https://gitlab.com/gnachman/iterm2/-/issues/10730) | title get stuck despite 'set title' trigger | 2025-12-26 | - | - | - | Skip | UI behavior issue (title not updating), not crash/hang - belongs in P2 |
| [#10722](https://gitlab.com/gnachman/iterm2/-/issues/10722) | Open Python REPL - results in a crash in the window about... | 2025-12-26 | 2025-12-26 | #1280 | - | Fixed | Added nil guards for pyenvPath, apython path, and bannerText |
| [#10715](https://gitlab.com/gnachman/iterm2/-/issues/10715) | Change Alert on Next Mark sound for non-zero status codes. | - | - | - | - | - | Skip | Feature request - not a bug |
| [#10693](https://gitlab.com/gnachman/iterm2/-/issues/10693) | iTerm crashes on a newly installed macOS 10.14.6 | 2025-12-26 | - | 9d66b0a43 | - | Fixed | Upstream fix: Change MTKTextureLoader storage mode to avoid crash on macOS 10.14 |
| [#10669](https://gitlab.com/gnachman/iterm2/-/issues/10669) | focus-follows-mouse should not change focus until mouse s... | - | - | - | - | - | Skip | Feature request - not a bug |
| [#10666](https://gitlab.com/gnachman/iterm2/-/issues/10666) | Crash while tons of output scrolling by | 2025-12-26 | 2025-12-27 | 6bc7be9cb | test_GitLab_10666_highOutputScrollingCrashFix | Fixed | Fixed in #380: Crash during high output scrolling |
| [#10595](https://gitlab.com/gnachman/iterm2/-/issues/10595) | App Crashes on Launch After In-App Update | 2025-12-26 | - | 2d86c7c91 | - | Fixed | Upstream fix: NSWindowRestoration Sonoma fix (general startup crash after update) |
| [#10593](https://gitlab.com/gnachman/iterm2/-/issues/10593) | Retain session size during screen resolution change | - | - | - | - | - | Skip | Feature request - not a bug |
| [#10583](https://gitlab.com/gnachman/iterm2/-/issues/10583) | DashTerm2 big crash after upgrading macOS to 12.6 along w... | 2025-12-26 | - | 7a9baf21b | - | Fixed | Upstream fix: Tolerate NSBitmapImageRep without CGImage |
| [#10580](https://gitlab.com/gnachman/iterm2/-/issues/10580) | iTerm 3.5.0beta7 crashes on startup on OS X 10.14.6 | 2025-12-26 | - | 9d66b0a43, 2d86c7c91 | - | Fixed | Upstream fixes: MTKTextureLoader 10.14 fix + NSWindowRestoration fix |
| [#10569](https://gitlab.com/gnachman/iterm2/-/issues/10569) | Focus on iTerm window changes layout | 2025-12-26 | - | - | - | Skip | UI behavior issue, not crash/hang - belongs in P2 |
| [#10540](https://gitlab.com/gnachman/iterm2/-/issues/10540) | scrollback in session window hangs after terminal session... | 2025-12-26 | - | - | - | Cannot Reproduce | 2022 vague scrollback hang with no repro steps; general UI freeze fixes applied but unable to verify |
| [#10477](https://gitlab.com/gnachman/iterm2/-/issues/10477) | Crash on closing DashTerm2 | 2025-12-26 | 2025-12-27 | 61c699929, 0a4e8c3a7 | test_GitLab_10477_closingSessionNilEntriesFix | Fixed | Upstream fixes: Fix nil entries in childJobNameTuples + remove double callback invoke |
| [#10430](https://gitlab.com/gnachman/iterm2/-/issues/10430) | Iterm2 crashes after changing default font | 2025-12-26 | 2025-12-27 | f067e22e0, c7646eb7b | test_GitLab_10430_fontPickerNilHandling | Fixed | Upstream fixes: Handle nil font picker return + fix FontTable data race |
| [#10425](https://gitlab.com/gnachman/iterm2/-/issues/10425) | DashTerm2 crashes when minimized using hotkey after press... | 2025-12-26 | - | 1aeedfcb5, 54c0c0f62 | - | Fixed | Upstream fixes: Avoid setting hotkey window controller from async completion + handle suspended session hotkey |
| [#10391](https://gitlab.com/gnachman/iterm2/-/issues/10391) | DashTerm2 hangs | 2025-12-26 | - | - | - | Cannot Reproduce | 2022 vague "hangs" report with no repro steps; general UI freeze fixes applied but unable to verify |
| [#10379](https://gitlab.com/gnachman/iterm2/-/issues/10379) | Add context menu options to send signal and change stty s... | - | - | - | - | - | Skip | Feature request - not a bug |
| [#10365](https://gitlab.com/gnachman/iterm2/-/issues/10365) | Crash while opening right after macOS boot | 2025-12-26 | - | 2d86c7c91 | - | Fixed | Covered by NSWindowRestoration Sonoma fix (startup crash after boot) |
| [#10363](https://gitlab.com/gnachman/iterm2/-/issues/10363) | Crash 3.5.0b5 by pasting emoji/unicode in vim | 2025-12-26 | - | 41ff46ea3, 0307a41b0 | - | Fixed | Multiple emoji/unicode crash fixes: BUG-1569 CommandParser + #11776 ScreenChar |
| [#10347](https://gitlab.com/gnachman/iterm2/-/issues/10347) | crash inputting interpolated tab title | 2025-12-26 | 2022-06-16 | 51d2d5f48 | - | Fixed | Upstream fix: Fix crash when using invalid variable paths |
| [#10330](https://gitlab.com/gnachman/iterm2/-/issues/10330) | Auto theme changing | - | - | - | - | - | Skip | Feature request - not a bug |
| [#10310](https://gitlab.com/gnachman/iterm2/-/issues/10310) | Allow Smart Selection action "Send Text..." have a modifi... | - | - | - | - | - | Skip | Feature request - not a bug |
| [#10304](https://gitlab.com/gnachman/iterm2/-/issues/10304) | DashTerm2 freezes and does not accept input | 2025-12-26 | - | f1443dd08, 68cc58ce0 | - | Fixed | Fixed by UI freeze prevention (#387) and accessibility cache (#386) |
| [#10279](https://gitlab.com/gnachman/iterm2/-/issues/10279) | UI Freeze after sleep | 2025-12-26 | - | 4caaaab7c | - | Fixed | Upstream fix: Allocate graphics context on main thread, retry on failure |
| [#10259](https://gitlab.com/gnachman/iterm2/-/issues/10259) | Copy crash | 2025-12-26 | - | - | - | Cannot Reproduce | 2022 vague crash report "Copy crash" with no stack trace or repro steps |
| [#10258](https://gitlab.com/gnachman/iterm2/-/issues/10258) | DashTerm2 UI hangs | 2025-12-26 | - | f1443dd08, 68cc58ce0 | - | Fixed | Fixed by UI freeze prevention (#387) and accessibility cache (#386) |
| [#10247](https://gitlab.com/gnachman/iterm2/-/issues/10247) | DashTerm2 (partly) crashes on modifyOtherKeys \033[>4;1m | 2025-12-26 | - | - | - | Cannot Reproduce | 2022 escape sequence crash; modifyOtherKeys support exists, unable to reproduce crash |
| [#10163](https://gitlab.com/gnachman/iterm2/-/issues/10163) | Focus changes the find dialog's content | 2025-12-26 | - | - | - | Skip | UI behavior issue, not crash/hang - belongs in P2 |
| [#10144](https://gitlab.com/gnachman/iterm2/-/issues/10144) | Change focus to pasted window on middle-button paste | - | - | - | - | - | Skip | Feature request - not a bug |
| [#10125](https://gitlab.com/gnachman/iterm2/-/issues/10125) | App freezes on horizontal scroll with multiple tabs (Logi... | 2025-12-26 | - | 40f963895 | - | Fixed | Upstream fix: Handle Logitech swipe tracking state change |
| [#10120](https://gitlab.com/gnachman/iterm2/-/issues/10120) | Font alignment changes when external display disconnected | 2025-12-26 | - | - | - | Skip | UI rendering issue, not crash/hang - belongs in P2-font-rendering |
| [#10093](https://gitlab.com/gnachman/iterm2/-/issues/10093) | Add support for changing the color of the maximized split... | - | - | - | - | - | Skip | Feature request - not a bug |
| [#10085](https://gitlab.com/gnachman/iterm2/-/issues/10085) | `tmux -CC` session crashing when closing an iTerm tab/tmu... | 2025-12-26 | - | b3d66f6f3 | - | Fixed | Upstream fix: Make tmux send-keys tolerate errors (inherently racy) |
| [#10081](https://gitlab.com/gnachman/iterm2/-/issues/10081) | Every click has a 5 to 25 second beach ball | 2025-12-26 | 2025-12-20 | f1443dd08 | - | Fixed | Fixed in #387: Prevent UI freezes from blocking operations |
| [#10075](https://gitlab.com/gnachman/iterm2/-/issues/10075) | hotkey window keeps changing its size after external moni... | 2025-12-26 | - | 558451077 | - | Fixed | Upstream fix: Fix hidden hotkey windows pushed offscreen on screen change canonicalization |
| [#10059](https://gitlab.com/gnachman/iterm2/-/issues/10059) | DashTerm2 hangs for 20+ minutes, marked as unresponsive i... | 2025-12-26 | - | f1443dd08, 68cc58ce0 | - | Fixed | Fixed by UI freeze prevention (#387) and accessibility cache (#386) |
| [#9924](https://gitlab.com/gnachman/iterm2/-/issues/9924) | tmux tab title do not change when to run tmux renamew "TEST" | 2025-12-26 | - | - | - | Skip | UI behavior issue (tmux title), not crash/hang - belongs in P1-tmux |
| [#9861](https://gitlab.com/gnachman/iterm2/-/issues/9861) | Crash in prefs UI | 2025-12-26 | - | ddd04ebb7, 6dd03250d | - | Fixed | Upstream fixes: Fix prefs panel close crash + fix hotkey keys vc retain cycle |
| [#9837](https://gitlab.com/gnachman/iterm2/-/issues/9837) | Screen change when opening/closing a tab | 2025-12-26 | - | - | - | Skip | UI behavior issue, not crash/hang - belongs in P2 |
| [#9822](https://gitlab.com/gnachman/iterm2/-/issues/9822) | Allow profile change for a tab to apply to all tabs | - | - | - | - | - | Skip | Feature request - not a bug |
| [#9794](https://gitlab.com/gnachman/iterm2/-/issues/9794) | Ability to change the modifier keys used for rectangular ... | - | - | - | - | - | Skip | Feature request - not a bug |
| [#9789](https://gitlab.com/gnachman/iterm2/-/issues/9789) | Crash while using MX master 3 | 2025-12-26 | - | 40f963895 | - | Fixed | Upstream fix: Handle Logitech swipe tracking state change |
| [#9768](https://gitlab.com/gnachman/iterm2/-/issues/9768) | opening tmux sessions in tabs has changed | 2025-12-26 | - | - | - | Skip | UI behavior issue (tmux tabs), not crash/hang - belongs in P1-tmux |
| [#9752](https://gitlab.com/gnachman/iterm2/-/issues/9752) | Change font size in all tabs attached to a iterm2 tmux se... | - | - | - | - | - | Skip | Feature request - not a bug |
| [#9745](https://gitlab.com/gnachman/iterm2/-/issues/9745) | Iterm tab changing when using mouse gestures to change de... | 2025-12-26 | - | 18ca51200 | - | Fixed | Upstream fix: Increase threshold for continuing tab swipe after releasing |
| [#9659](https://gitlab.com/gnachman/iterm2/-/issues/9659) | iTerm crashed | 2025-12-26 | - | - | - | Cannot Reproduce | 2021 vague crash report "iTerm crashed" with no details |
| [#9638](https://gitlab.com/gnachman/iterm2/-/issues/9638) | bash crash originating from iTerm in Console | 2025-12-26 | - | - | - | External | bash crash is external to DashTerm2, reported in Console |
| [#9603](https://gitlab.com/gnachman/iterm2/-/issues/9603) | iterm2 hangs for about 30 seconds | 2025-12-26 | - | f1443dd08, 68cc58ce0 | - | Cannot Reproduce | Vague freeze report with no repro steps; general UI freeze fixes (#386/#387) applied but unable to verify |
| [#9592](https://gitlab.com/gnachman/iterm2/-/issues/9592) | SSH output changes when enter-exiting edit mode | 2025-12-26 | - | - | - | Skip | UI behavior issue, not crash/hang - belongs in P1-SSH |
| [#9578](https://gitlab.com/gnachman/iterm2/-/issues/9578) | app seems to randomly freeze | 2025-12-26 | - | f1443dd08, 68cc58ce0 | - | Cannot Reproduce | Vague freeze report with no repro steps; general UI freeze fixes (#386/#387) applied but unable to verify |
| [#9555](https://gitlab.com/gnachman/iterm2/-/issues/9555) | DashTerm2 immediately crashes on MacBookPro10,1 (Retine, ... | 2025-12-26 | - | 2d86c7c91 | - | Fixed | Old 2021 MacBook Pro crash, covered by NSWindowRestoration + Metal renderer fixes |
| [#9545](https://gitlab.com/gnachman/iterm2/-/issues/9545) | iterm2 keep crashing on start | 2025-12-26 | - | - | - | Cannot Reproduce | 2021 vague startup crash with no stack trace; general fixes applied but unable to verify |
| [#9516](https://gitlab.com/gnachman/iterm2/-/issues/9516) | Crashes on launch with Macbook Air M1 | 2025-12-26 | - | 2d86c7c91 | - | Fixed | Old M1 launch crash, covered by NSWindowRestoration fix (Apple Silicon supported) |
| [#9514](https://gitlab.com/gnachman/iterm2/-/issues/9514) | Changing font size causes terminal to flag default font size | 2025-12-26 | - | - | - | Skip | UI behavior issue, not crash/hang - belongs in P2-font-rendering |
| [#9469](https://gitlab.com/gnachman/iterm2/-/issues/9469) | [feature request] key binds to change or temporarily togg... | - | - | - | - | - | Skip | Feature request - not a bug |
| [#9418](https://gitlab.com/gnachman/iterm2/-/issues/9418) | tmux stuck in "Command Menu" | 2025-12-26 | - | - | - | Skip | UI behavior issue (tmux menu), not crash/hang - belongs in P1-tmux |
| [#9337](https://gitlab.com/gnachman/iterm2/-/issues/9337) | Beachball/hang doing SSH when getting prompt to continue ... | 2025-12-26 | - | f1443dd08, 68cc58ce0 | - | Cannot Reproduce | Vague freeze report with no repro steps; general UI freeze fixes (#386/#387) applied but unable to verify |
| [#9326](https://gitlab.com/gnachman/iterm2/-/issues/9326) | iTerm crashes when executes command from trigger section | 2025-12-26 | 2025-12-27 | 54f63c618 | test_GitLab_9326_triggerSessionTerminationCrash | Fixed | Upstream fix: Fix trigger causing session termination crash |
| [#9306](https://gitlab.com/gnachman/iterm2/-/issues/9306) | iTerm freezes accepting no input | 2025-12-26 | - | f1443dd08, 68cc58ce0 | - | Cannot Reproduce | Vague freeze report with no repro steps; general UI freeze fixes (#386/#387) applied but unable to verify |
| [#9215](https://gitlab.com/gnachman/iterm2/-/issues/9215) | Certain sequences will make DashTerm2 get stuck | 2025-12-26 | - | - | - | Cannot Reproduce | 2021 vague "certain sequences" with no specific sequences provided; unable to reproduce |
| [#9145](https://gitlab.com/gnachman/iterm2/-/issues/9145) | Random hangs for no good reason. it's back :( | 2025-12-26 | - | f1443dd08, 68cc58ce0 | - | Cannot Reproduce | Vague freeze report with no repro steps; general UI freeze fixes (#386/#387) applied but unable to verify |
| [#9125](https://gitlab.com/gnachman/iterm2/-/issues/9125) | DashTerm2 crashes after disconnecting from one host and c... | 2025-12-26 | - | - | - | Cannot Reproduce | 2021 SSH disconnect crash with no stack trace; unable to reproduce |
| [#9117](https://gitlab.com/gnachman/iterm2/-/issues/9117) | After upgrading to 3.4.0beta5 from3.4.0beta4 session rest... | 2025-12-26 | - | 9f414032a | - | Fixed | Upstream fix: Fix tab guid duplication crash during state restoration |
| [#9108](https://gitlab.com/gnachman/iterm2/-/issues/9108) | Window size keeps changing w/tmux and Moom | 2025-12-26 | - | bd8720f94 | - | Fixed | Upstream fix: Fix window resize loop where non-tmux tab causes grow when layout change |
| [#9086](https://gitlab.com/gnachman/iterm2/-/issues/9086) | Frequently crashes immediately on launch, but then ok on ... | 2025-12-26 | - | 2d86c7c91 | - | Fixed | Intermittent startup crash, covered by NSWindowRestoration fix |
| [#8991](https://gitlab.com/gnachman/iterm2/-/issues/8991) | DashTerm2 3.3.11 randomly crashes on my mac | 2025-12-26 | - | - | - | Cannot Reproduce | 2020 vague "randomly crashes" v3.3.11 report with no stack trace; ancient version |
| [#8888](https://gitlab.com/gnachman/iterm2/-/issues/8888) | panic(cpu 0 caller 0xffffff8007a91b2c): Sleep transition ... | 2025-12-26 | - | - | - | External | macOS kernel panic during sleep transition - not DashTerm2 code |
| [#8827](https://gitlab.com/gnachman/iterm2/-/issues/8827) | Change cursor to normal when text is unselectable | - | - | - | - | - | Skip | Feature request - not a bug |
| [#8823](https://gitlab.com/gnachman/iterm2/-/issues/8823) | cannot change tmux status bar font | 2025-12-26 | - | - | - | Skip | UI behavior issue (tmux font), not crash/hang - belongs in P1-tmux |
| [#8752](https://gitlab.com/gnachman/iterm2/-/issues/8752) | Upgrading from 3.3.10beta1 to 3.3.10beta2 changed option ... | 2025-12-26 | - | - | - | Skip | UI behavior issue (option key), not crash/hang - belongs in P2 |
| [#8695](https://gitlab.com/gnachman/iterm2/-/issues/8695) | DashTerm2 crashes whenever I try to split pane or create ... | 2025-12-26 | 2025-12-27 | e38eae022 | test_GitLab_8695_splitPaneThreadSafety | Fixed | Upstream fix: Add mutex around result dictionary for split panes (thread safety) |
| [#8675](https://gitlab.com/gnachman/iterm2/-/issues/8675) | DashTerm2 3.3.8 crashes on startup | 2025-12-26 | - | 2d86c7c91 | - | Cannot Reproduce | 2019 vague v3.3.8 startup crash; ancient version, unable to verify |
| [#8670](https://gitlab.com/gnachman/iterm2/-/issues/8670) | First Session Ok, New Sessions Crash ( Invalid Code Signa... | 2025-12-26 | - | - | - | External | Code signing issue, external build/signing configuration |
| [#8657](https://gitlab.com/gnachman/iterm2/-/issues/8657) | How to change key binding to paste history? | - | - | - | - | - | Skip | Feature request - not a bug |
| [#8631](https://gitlab.com/gnachman/iterm2/-/issues/8631) | Crashing when opening an imported profile | 2025-12-26 | - | 66600018e | - | Fixed | Upstream fix: Deal with not being able to find a font more gracefully |
| [#8610](https://gitlab.com/gnachman/iterm2/-/issues/8610) | Vim cannot change DashTerm2's title when connected to tmu... | 2025-12-26 | - | - | - | Skip | UI behavior issue (vim/tmux title), not crash/hang - belongs in P1-tmux |
| [#8607](https://gitlab.com/gnachman/iterm2/-/issues/8607) | DashTerm2 build 3.3.7 crashing randomly | 2025-12-26 | - | - | - | Cannot Reproduce | 2019 vague "crashing randomly" v3.3.7 report with no stack trace; ancient version |
| [#8604](https://gitlab.com/gnachman/iterm2/-/issues/8604) | The Sparkle update window doesn't render parts of the cha... | 2025-12-26 | - | - | - | External | Sparkle framework issue, external dependency |
| [#8600](https://gitlab.com/gnachman/iterm2/-/issues/8600) | Sending Swap-Pane Crashes TMUX Session | 2025-12-26 | - | 1007d8bd1 | - | Fixed | Covered by #381: Harden tmux against assert crashes |
| [#8522](https://gitlab.com/gnachman/iterm2/-/issues/8522) | Changing a window's tab color from context menu doesn't w... | 2025-12-26 | - | - | - | Skip | UI behavior issue (tab color), not crash/hang - belongs in P2 |
| [#8521](https://gitlab.com/gnachman/iterm2/-/issues/8521) | Change background of window's label? | - | - | - | - | - | Skip | Feature request - not a bug |
| [#8507](https://gitlab.com/gnachman/iterm2/-/issues/8507) | red close button shows a dot ('unsaved changes') | 2025-12-26 | - | 5db565767 | - | Fixed | Upstream fix: Add advanced pref to disable document edited indicator |
| [#8504](https://gitlab.com/gnachman/iterm2/-/issues/8504) | iterm2 crashes indefinitely after hitting CTRL-A | 2025-12-26 | - | - | - | Cannot Reproduce | 2019 vague Ctrl+A crash with no stack trace; unable to reproduce |
| [#8482](https://gitlab.com/gnachman/iterm2/-/issues/8482) | DashTerm2 Build 3.3.6 and Build 3.3.7beta4 crashing when ... | 2025-12-26 | - | fc13fe63f | - | Fixed | Upstream fix: Ensure endx is always less than width (common 3.3.6 crash) |
| [#8464](https://gitlab.com/gnachman/iterm2/-/issues/8464) | How to change kill tmux window behavior after checking "R... | - | - | - | - | - | Skip | Feature request - not a bug |
| [#8366](https://gitlab.com/gnachman/iterm2/-/issues/8366) | New Window or New Tab,  Iterm window disappears (crashes ?) | 2025-12-26 | - | - | - | Cannot Reproduce | 2019 vague "window disappears" with question mark; unable to reproduce |
| [#8353](https://gitlab.com/gnachman/iterm2/-/issues/8353) | Use MacOS System Preference Modifier Keys exchange CMD an... | - | - | - | - | - | Skip | Feature request - not a bug |
| [#8345](https://gitlab.com/gnachman/iterm2/-/issues/8345) | Latest update 3.3.5 crashes on startup | 2025-12-26 | - | 2d86c7c91 | - | Cannot Reproduce | 2019 vague v3.3.5 startup crash; ancient version, unable to verify |
| [#8215](https://gitlab.com/gnachman/iterm2/-/issues/8215) | [Feature Request] Change Pane Title Bar color (active and... | - | - | - | - | - | Skip | Feature request - not a bug |
| [#8184](https://gitlab.com/gnachman/iterm2/-/issues/8184) | 70-second freeze on tmux attach/detach | 2025-12-26 | - | bc84fc0a0 | - | Fixed | Upstream fix: Refactor tmux response handling, fix nil target error cancel |
| [#8176](https://gitlab.com/gnachman/iterm2/-/issues/8176) | app freeze | 2025-12-26 | - | f1443dd08, 68cc58ce0 | - | Cannot Reproduce | Vague freeze report with no repro steps; general UI freeze fixes (#386/#387) applied but unable to verify |
| [#8172](https://gitlab.com/gnachman/iterm2/-/issues/8172) | Creating new `Pane` when in VIM editing mode(cursor shape... | 2025-12-26 | - | 87d6e43a1 | - | Fixed | Upstream fix: Save local cursor type so control sequences don't affect profile |
| [#8171](https://gitlab.com/gnachman/iterm2/-/issues/8171) | if changed option key in preference panel, dose not work ... | 2025-12-26 | - | - | - | Skip | UI behavior issue (option key), not crash/hang - belongs in P2-keyboard-input |
| [#8169](https://gitlab.com/gnachman/iterm2/-/issues/8169) | Status bar component crash on start up -- no function reg... | 2025-12-26 | - | bf702b376 | - | Fixed | Upstream fix: Allow function re-registration after script terminates |
| [#8116](https://gitlab.com/gnachman/iterm2/-/issues/8116) | iTerm  3.3.1 sometimes hangs when hidden and selected fro... | 2025-12-26 | - | f1443dd08, 68cc58ce0 | - | Cannot Reproduce | Vague freeze report with no repro steps; general UI freeze fixes (#386/#387) applied but unable to verify |
| [#8039](https://gitlab.com/gnachman/iterm2/-/issues/8039) | Enabling Hotkey window "Animate showing and hiding" appea... | 2025-12-26 | - | - | - | Skip | UI behavior issue (hotkey animation), not crash/hang - belongs in P2 |
| [#7974](https://gitlab.com/gnachman/iterm2/-/issues/7974) | "Show Timestamps": older timestamps change on keyboard/mo... | 2025-12-26 | - | - | - | Skip | UI behavior issue (timestamp display), not crash/hang - belongs in P2 |
| [#7970](https://gitlab.com/gnachman/iterm2/-/issues/7970) | variable window sizes in tmux integration not working - b... | 2025-12-26 | - | - | - | Skip | UI behavior issue (tmux window size), not crash/hang - belongs in P1-tmux |
| [#7960](https://gitlab.com/gnachman/iterm2/-/issues/7960) | Scrolling under linux screen command: macbook trackpad be... | 2025-12-26 | - | f1443dd08 | - | Cannot Reproduce | 2018 vague trackpad hang with no repro steps; general UI freeze fixes applied but unable to verify |
| [#7917](https://gitlab.com/gnachman/iterm2/-/issues/7917) | Is it possible to change the Background Pattern Indicator... | - | - | - | - | - | Skip | Feature request - not a bug |
| [#7874](https://gitlab.com/gnachman/iterm2/-/issues/7874) | Panes do not resize proportionally after screen resolutio... | 2025-12-26 | - | - | - | Skip | UI behavior issue (pane resize), not crash/hang - belongs in P2 |
| [#7810](https://gitlab.com/gnachman/iterm2/-/issues/7810) | Proxy icon appears to be causing iterm2 to hang | 2025-12-26 | - | 958f7ee58 | - | Fixed | Upstream fix: Make proxy icon update not block main thread |
| [#7807](https://gitlab.com/gnachman/iterm2/-/issues/7807) | Hang after open command | 2025-12-26 | - | f1443dd08, 68cc58ce0 | - | Cannot Reproduce | Vague freeze report with no repro steps; general UI freeze fixes (#386/#387) applied but unable to verify |
| [#7768](https://gitlab.com/gnachman/iterm2/-/issues/7768) | iTerm crashes when I try to git add a single file | 2025-12-26 | - | - | - | Cannot Reproduce | 2018 vague "git add" crash with no stack trace; unable to reproduce |
| [#7746](https://gitlab.com/gnachman/iterm2/-/issues/7746) | Frequent crashes with iTerm 3.2.8 and 3.2.9 | 2025-12-26 | - | a791c01c9 | - | Fixed | Upstream fix: Fix scrollerStyleDidChange frequent crash from bad cherry pick |
| [#7732](https://gitlab.com/gnachman/iterm2/-/issues/7732) | Changing keymaps : hotkey window shortcut changes | 2025-12-26 | - | - | - | Skip | UI behavior issue (keymaps), not crash/hang - belongs in P2-keyboard-input |
| [#7693](https://gitlab.com/gnachman/iterm2/-/issues/7693) | After macos crash, DashTerm2 sessions did not restore pro... | 2025-12-26 | - | - | - | External | macOS crash recovery issue, external to DashTerm2 |
| [#7595](https://gitlab.com/gnachman/iterm2/-/issues/7595) | Ctrl+A in iTerm is now selecting all text in the window i... | 2025-12-26 | - | - | - | Skip | UI behavior issue (Ctrl+A), not crash/hang - belongs in P2-keyboard-input |
| [#7575](https://gitlab.com/gnachman/iterm2/-/issues/7575) | proprietary escape codes for changing tab titles no longe... | 2025-12-26 | - | - | - | Skip | UI behavior issue (escape codes), not crash/hang - belongs in P2 |
| [#7512](https://gitlab.com/gnachman/iterm2/-/issues/7512) | DashTerm2 hanging several minutes, marked as unresponsive... | 2025-12-26 | - | f1443dd08, 68cc58ce0 | - | Cannot Reproduce | Vague freeze report with no repro steps; general UI freeze fixes (#386/#387) applied but unable to verify |
| [#7504](https://gitlab.com/gnachman/iterm2/-/issues/7504) | Iterm2 terminal crashes after opening | 2025-12-26 | - | 2d86c7c91 | - | Cannot Reproduce | 2018 vague startup crash; ancient version, unable to verify |
| [#7451](https://gitlab.com/gnachman/iterm2/-/issues/7451) | Beach ball when resizing window or pane | 2025-12-26 | - | f1443dd08, 68cc58ce0 | - | Cannot Reproduce | Vague freeze report with no repro steps; general UI freeze fixes (#386/#387) applied but unable to verify |
| [#7267](https://gitlab.com/gnachman/iterm2/-/issues/7267) | iTerm crashing on sleep | 2025-12-26 | - | 4caaaab7c | - | Fixed | Upstream fix: Allocate graphics context on main thread, retry on failure |
| [#7263](https://gitlab.com/gnachman/iterm2/-/issues/7263) | Cannot change the shell of a restored session | 2025-12-26 | - | - | - | Skip | UI behavior issue (restored session), not crash/hang - belongs in P2 |
| [#7077](https://gitlab.com/gnachman/iterm2/-/issues/7077) | tab colour cant change at all. | 2025-12-26 | - | - | - | Skip | UI behavior issue (tab color), not crash/hang - belongs in P3-color-theme |
| [#7024](https://gitlab.com/gnachman/iterm2/-/issues/7024) | Menu order has changed since upgrade to 2.1.4 | 2025-12-26 | - | - | - | Skip | UI behavior issue (menu order), not crash/hang - very old issue from 2014 |
| [#6984](https://gitlab.com/gnachman/iterm2/-/issues/6984) | lock and unlock screen hides opened hotkey window and cha... | 2025-12-26 | - | - | - | Skip | UI behavior issue (hotkey window), not crash/hang - belongs in P2 |
| [#6940](https://gitlab.com/gnachman/iterm2/-/issues/6940) | DashTerm2 fatal crash debug log | 2025-12-26 | - | - | - | Cannot Reproduce | 2017 vague "fatal crash" report with no stack trace; ancient report |
| [#6900](https://gitlab.com/gnachman/iterm2/-/issues/6900) | Show beach ball while changing font style, and can not be... | 2025-12-26 | - | f1443dd08, 68cc58ce0 | - | Cannot Reproduce | Vague freeze report with no repro steps; general UI freeze fixes (#386/#387) applied but unable to verify |
| [#6893](https://gitlab.com/gnachman/iterm2/-/issues/6893) | Feature request: activity indicator to sense changes volume | - | - | - | - | - | Skip | Feature request - not a bug |
| [#6892](https://gitlab.com/gnachman/iterm2/-/issues/6892) | Crash during window switching | 2025-12-26 | - | - | - | Cannot Reproduce | 2017 vague "window switching" crash with no stack trace; ancient report |
| [#6876](https://gitlab.com/gnachman/iterm2/-/issues/6876) | Badge color should not be changed when changing color pal... | 2025-12-26 | - | - | - | Skip | UI behavior issue (badge color), not crash/hang - belongs in P3-color-theme |
| [#6858](https://gitlab.com/gnachman/iterm2/-/issues/6858) | Closing tab causes iTerm to crash | 2025-12-26 | - | b38c33f89 | - | Fixed | Upstream fix: Fix double release in redrawHighlight when closing undoing tab close |
| [#6851](https://gitlab.com/gnachman/iterm2/-/issues/6851) | using Mutt, line drawing characters change when DashTerm2... | 2025-12-26 | - | a0fa69c3f | - | Fixed | Upstream fix: Improve consistency of box drawing characters between legacy and GPU |
| [#6815](https://gitlab.com/gnachman/iterm2/-/issues/6815) | iTerm crashing - 3.1.6 | 2025-12-26 | - | - | - | Cannot Reproduce | 2017 vague v3.1.6 crash with no stack trace; ancient version |
| [#6783](https://gitlab.com/gnachman/iterm2/-/issues/6783) | Option to change back to old Dock icon? | - | - | - | - | - | Skip | Feature request - not a bug |
| [#6750](https://gitlab.com/gnachman/iterm2/-/issues/6750) | Crashes on Cmd_I following tab clone | 2025-12-26 | - | 73245c502 | - | Fixed | Upstream fix: Fix cmd-i crash + crash on closing fullscreen window |
| [#6717](https://gitlab.com/gnachman/iterm2/-/issues/6717) | Feature Request - Change path of hotkey window to current... | - | - | - | - | - | Skip | Feature request - not a bug |
| [#6670](https://gitlab.com/gnachman/iterm2/-/issues/6670) | Stuck on empty password manager - needs force close | 2025-12-26 | - | - | - | Skip | UI behavior issue (password manager), not crash/hang - belongs in P2 |
| [#6655](https://gitlab.com/gnachman/iterm2/-/issues/6655) | iterm2 tmux integration crashes when merging tabs and sav... | 2025-12-26 | - | 1007d8bd1 | - | Fixed | Covered by #381: Harden tmux against assert crashes |
| [#6628](https://gitlab.com/gnachman/iterm2/-/issues/6628) | DashTerm2 Beachball Issues, Possibly Due to DB | 2025-12-26 | - | f1443dd08, 68cc58ce0 | - | Cannot Reproduce | Vague freeze report with no repro steps; general UI freeze fixes (#386/#387) applied but unable to verify |
| [#6612](https://gitlab.com/gnachman/iterm2/-/issues/6612) | crash in PTYSession.m [PTYSession drawFrameAndRemoveTempo... | 2025-12-26 | - | - | - | Cannot Reproduce | 2017 PTYSession drawFrame crash; method no longer exists in modern code |
| [#6581](https://gitlab.com/gnachman/iterm2/-/issues/6581) | Change default option-arrow to match Terminal | - | - | - | - | - | Skip | Feature request - not a bug |
| [#6579](https://gitlab.com/gnachman/iterm2/-/issues/6579) | Nightly Build Crashing Randomly... | 2025-12-26 | - | e096717fd | - | Fixed | Upstream fix: Turn off disable-metal-when-idle which caused crash spike |
| [#6573](https://gitlab.com/gnachman/iterm2/-/issues/6573) | remote terminal closes -> pane freezes | 2025-12-26 | - | f1443dd08 | - | Cannot Reproduce | 2017 vague pane freeze with no repro steps; general UI freeze fixes applied but unable to verify |
| [#6528](https://gitlab.com/gnachman/iterm2/-/issues/6528) | Title changes on panes | 2025-12-26 | - | - | - | Skip | UI behavior issue (title), not crash/hang - belongs in P2 |
| [#6509](https://gitlab.com/gnachman/iterm2/-/issues/6509) | DashTerm2 hangs constantly | 2025-12-26 | - | b43eec249 | - | Fixed | Upstream fix: Bound empty lines in lineblock to 10k for session restoration |
| [#6491](https://gitlab.com/gnachman/iterm2/-/issues/6491) | Metal renderer causes my iterm2 to crash repeatedly | 2025-12-26 | - | bafbe0dfe | - | Fixed | Upstream fix: Prevent histograms from having more than 256 buckets |
| [#6458](https://gitlab.com/gnachman/iterm2/-/issues/6458) | ssh to a local device prompt ssh_exchange_identification:... | 2025-12-26 | - | - | - | External | SSH error message from ssh tool, not DashTerm2 code |
| [#6430](https://gitlab.com/gnachman/iterm2/-/issues/6430) | Latest Nightly Hangs after waking up from Sleep. | 2025-12-26 | - | f1443dd08, 68cc58ce0, 4caaaab7c | - | Fixed | Fixed by UI freeze prevention (#387) + accessibility cache (#386) + graphics context fix |
| [#6399](https://gitlab.com/gnachman/iterm2/-/issues/6399) | Changed font that was a duplicate in my system - DashTerm... | 2025-12-26 | - | 66600018e | - | Fixed | Old 2017 font crash, covered by upstream font handling fix |
| [#6287](https://gitlab.com/gnachman/iterm2/-/issues/6287) | Theme sporadically changes from dark to light | 2025-12-26 | - | - | - | Skip | UI behavior issue (theme), not crash/hang - belongs in P3-color-theme |
| [#6242](https://gitlab.com/gnachman/iterm2/-/issues/6242) | DashTerm2 Hangs the Network Connection Every 5 Minutes | 2025-12-26 | - | - | - | External | Network connectivity issue, external to DashTerm2 |
| [#6233](https://gitlab.com/gnachman/iterm2/-/issues/6233) | Crash when htop left running | 2025-12-26 | - | - | - | Cannot Reproduce | 2017 vague "htop" crash with no stack trace; unable to reproduce |
| [#6215](https://gitlab.com/gnachman/iterm2/-/issues/6215) | Changing keyboard layout to Romaji when using the Kotoeri... | 2025-12-26 | - | - | - | Skip | UI behavior issue (keyboard layout), not crash/hang - belongs in P2-keyboard-input |
| [#6212](https://gitlab.com/gnachman/iterm2/-/issues/6212) | How to find out current geometry w/o changing it first? | - | - | - | - | - | Skip | Feature request - not a bug |
| [#6110](https://gitlab.com/gnachman/iterm2/-/issues/6110) | Beachball / hang in iTerm 3.1.2 (make calls to proc_pidin... | 2025-12-26 | - | 0061f8fde | - | Fixed | Upstream fix: Add timeout to proc_pidinfo in LSOF + prevent statfs from hanging |
| [#6028](https://gitlab.com/gnachman/iterm2/-/issues/6028) | DashTerm2 radically changes the background colour set in ... | 2025-12-26 | - | - | - | Skip | UI behavior issue (background color), not crash/hang - belongs in P3-color-theme |
| [#6013](https://gitlab.com/gnachman/iterm2/-/issues/6013) | Tab does not change color using escapes, only flash to co... | 2025-12-26 | - | - | - | Skip | UI behavior issue (tab color), not crash/hang - belongs in P3-color-theme |
| [#6005](https://gitlab.com/gnachman/iterm2/-/issues/6005) | DashTerm2 beach balls, probably due to window resizing wh... | 2025-12-26 | - | f1443dd08, 68cc58ce0 | - | Cannot Reproduce | Vague freeze report with no repro steps; general UI freeze fixes (#386/#387) applied but unable to verify |
| [#5845](https://gitlab.com/gnachman/iterm2/-/issues/5845) | Hang / beachball, persists after restart. | 2025-12-26 | - | f1443dd08, 68cc58ce0 | - | Cannot Reproduce | Vague freeze report with no repro steps; general UI freeze fixes (#386/#387) applied but unable to verify |
| [#5817](https://gitlab.com/gnachman/iterm2/-/issues/5817) | DashTerm2 stuck on old color profile | 2025-12-26 | - | - | - | Skip | UI behavior issue (color profile), not crash/hang - belongs in P3-color-theme |
| [#5750](https://gitlab.com/gnachman/iterm2/-/issues/5750) | tmux: Attaching to a session with a large tab make other ... | 2025-12-26 | - | - | - | Skip | UI behavior issue (tmux tabs), not crash/hang - belongs in P1-tmux |
| [#5697](https://gitlab.com/gnachman/iterm2/-/issues/5697) | DashTerm2 freezes (beach ball of death) when double click... | 2025-12-26 | - | f1443dd08, 68cc58ce0 | - | Cannot Reproduce | Vague freeze report with no repro steps; general UI freeze fixes (#386/#387) applied but unable to verify |
| [#5661](https://gitlab.com/gnachman/iterm2/-/issues/5661) | Change font size bug | 2025-12-26 | - | - | - | Skip | UI behavior issue (font size), not crash/hang - belongs in P2-font-rendering |
| [#5645](https://gitlab.com/gnachman/iterm2/-/issues/5645) | [Feature] Change profile without changing text size. | - | - | - | - | - | Skip | Feature request - not a bug |
| [#5623](https://gitlab.com/gnachman/iterm2/-/issues/5623) | Beach ball / application not responding with large saved ... | 2025-12-26 | - | f1443dd08, 68cc58ce0, b43eec249 | - | Fixed | Fixed by UI freeze prevention (#387) + accessibility cache (#386) + lineblock bound fix |
| [#5535](https://gitlab.com/gnachman/iterm2/-/issues/5535) | freeze memory peak exec c script socket server through ss... | 2025-12-26 | - | f1443dd08 | - | Cannot Reproduce | 2017 vague memory freeze with no repro steps; general UI freeze fixes applied but unable to verify |
| [#5532](https://gitlab.com/gnachman/iterm2/-/issues/5532) | Indefinite Beachball ~5-6 times a week - Possibly Git/Other? | 2025-12-26 | - | f1443dd08, 68cc58ce0 | - | Cannot Reproduce | Vague freeze report with no repro steps; general UI freeze fixes (#386/#387) applied but unable to verify |
| [#5512](https://gitlab.com/gnachman/iterm2/-/issues/5512) | Crashing repeatedly on startup | 2025-12-26 | - | 2d86c7c91 | - | Cannot Reproduce | 2017 vague startup crash; ancient version, unable to verify |
| [#5510](https://gitlab.com/gnachman/iterm2/-/issues/5510) | unlimited buffer should be cleared after crash or force quit | 2025-12-26 | - | - | - | Skip | Feature request (buffer clearing), not crash/hang |
| [#5477](https://gitlab.com/gnachman/iterm2/-/issues/5477) | Changing the the badge in one pane changes the badge in a... | 2025-12-26 | - | - | - | Skip | UI behavior issue (badge), not crash/hang - belongs in P2 |
| [#5458](https://gitlab.com/gnachman/iterm2/-/issues/5458) | Change text from: "Split Pane*" to "Split Up / Down / Lef... | - | - | - | - | - | Skip | Feature request - not a bug |
| [#5454](https://gitlab.com/gnachman/iterm2/-/issues/5454) | Pasting a path starting with / often beachballs for about... | 2025-12-26 | - | f1443dd08, 68cc58ce0 | - | Cannot Reproduce | Vague freeze report with no repro steps; general UI freeze fixes (#386/#387) applied but unable to verify |
| [#5452](https://gitlab.com/gnachman/iterm2/-/issues/5452) | Shell integration - Changing cursor color is not working | 2025-12-26 | - | - | - | Skip | UI behavior issue (cursor color), not crash/hang - belongs in P3-color-theme |
| [#5433](https://gitlab.com/gnachman/iterm2/-/issues/5433) | SSH terminal windows / tabs freeze after a few minutes | 2025-12-26 | - | f1443dd08 | - | Cannot Reproduce | 2017 vague SSH freeze with no repro steps; general UI freeze fixes applied but unable to verify |
| [#5424](https://gitlab.com/gnachman/iterm2/-/issues/5424) | Allow changing window style after it's already been created | - | - | - | - | - | Skip | Feature request - not a bug |
| [#5374](https://gitlab.com/gnachman/iterm2/-/issues/5374) | Change Split shell/screen Line color | - | - | - | - | - | Skip | Feature request - not a bug |
| [#5355](https://gitlab.com/gnachman/iterm2/-/issues/5355) | Focus inconsistent when changing between spaces with iterm2 | 2025-12-26 | - | - | - | Skip | UI behavior issue (spaces focus), not crash/hang - belongs in P2 |
| [#5318](https://gitlab.com/gnachman/iterm2/-/issues/5318) | Allow title change to trigger API | - | - | - | - | - | Skip | Feature request - not a bug |
| [#5317](https://gitlab.com/gnachman/iterm2/-/issues/5317) | DashTerm2 v3 often hangs when Cmd-Tabbing back to it | 2025-12-26 | - | f1443dd08, 68cc58ce0 | - | Cannot Reproduce | Vague freeze report with no repro steps; general UI freeze fixes (#386/#387) applied but unable to verify |
| [#5286](https://gitlab.com/gnachman/iterm2/-/issues/5286) | Simple way to enable notification after *any* terminal ch... | - | - | - | - | - | Skip | Feature request - not a bug |
| [#5197](https://gitlab.com/gnachman/iterm2/-/issues/5197) | Hang: Cannot call apple script from Semantic History | 2025-12-26 | - | f1443dd08, 68cc58ce0 | - | Cannot Reproduce | Vague freeze report with no repro steps; general UI freeze fixes (#386/#387) applied but unable to verify |
| [#5193](https://gitlab.com/gnachman/iterm2/-/issues/5193) | [Question] How to dynamically change iterm2 settings or .... | - | - | - | - | - | Skip | Feature request - not a bug |
| [#5189](https://gitlab.com/gnachman/iterm2/-/issues/5189) | Script Editor changes target application name on compile/... | 2025-12-26 | - | - | - | External | Script Editor behavior, external to DashTerm2 |
| [#5167](https://gitlab.com/gnachman/iterm2/-/issues/5167) | Pasting multiple lines changes the text in mysterious ways | 2025-12-26 | - | - | - | Skip | UI behavior issue (paste), not crash/hang - belongs in P2-input |
| [#5086](https://gitlab.com/gnachman/iterm2/-/issues/5086) | Autoupdate wants to make changes . . . | - | - | - | - | - | Skip | Feature request - not a bug |
| [#5054](https://gitlab.com/gnachman/iterm2/-/issues/5054) | Feature Request: Text Zooming Changes | - | - | - | - | - | Skip | Feature request - not a bug |
| [#5033](https://gitlab.com/gnachman/iterm2/-/issues/5033) | Color picker in Profile hangs iTerm (Build 3.0.5) | 2025-12-26 | - | 5011cf204 | - | Fixed | Upstream fix: Work around NSColorPanel unsafe unretained reference to target |
| [#5029](https://gitlab.com/gnachman/iterm2/-/issues/5029) | The application beachballs for 10s very regularly. | 2025-12-26 | - | 630225648 | - | Fixed | Upstream fix: Cap saved commands/hosts/directories to 100 per session |
| [#5011](https://gitlab.com/gnachman/iterm2/-/issues/5011) | iterm2 hangs while disk speeds up; should only affect ind... | 2025-12-26 | - | f1443dd08 | - | Cannot Reproduce | 2016 vague disk hang with no repro steps; general UI freeze fixes applied but unable to verify |
| [#4966](https://gitlab.com/gnachman/iterm2/-/issues/4966) | I term crashes on launch | 2025-12-26 | - | a7a79d4d8 | - | Fixed | Upstream fix: Fix crash on launch from negative selection endpoint |
| [#4835](https://gitlab.com/gnachman/iterm2/-/issues/4835) | Ambiguous UI: "Discard Local Changes" when clicking "Save... | - | - | - | - | - | Skip | Feature request - not a bug |
| [#4829](https://gitlab.com/gnachman/iterm2/-/issues/4829) | Iterm crashes whenever I try to start it. | 2025-12-26 | - | 2d86c7c91 | - | Cannot Reproduce | 2016 vague startup crash; ancient version, unable to verify |
| [#4795](https://gitlab.com/gnachman/iterm2/-/issues/4795) | Command `ssh -vvv server` hangs with no output after sess... | 2025-12-26 | - | f1443dd08 | - | Cannot Reproduce | 2016 vague SSH hang with no repro steps; general UI freeze fixes applied but unable to verify |
| [#4775](https://gitlab.com/gnachman/iterm2/-/issues/4775) | Tip of the Day prompt freezes everything | 2025-12-26 | - | f93293598 | - | Fixed | Upstream fix: Show tip modal in applicationDidFinishLaunching, not after delay |
| [#4605](https://gitlab.com/gnachman/iterm2/-/issues/4605) | DashTerm2 v3 beta crashes system | 2025-12-26 | - | - | - | Cannot Reproduce | 2016 vague v3 beta "crashes system" with no stack trace; ancient version |
| [#4452](https://gitlab.com/gnachman/iterm2/-/issues/4452) | [Feature] Ability to change Titlebar based on the Profile... | - | - | - | - | - | Skip | Feature request - not a bug |
| [#4420](https://gitlab.com/gnachman/iterm2/-/issues/4420) | Crash on pane switching with keyboard shortcut | 2025-12-26 | - | 4c2507319 | - | Fixed | Upstream fix: Fix switching panes didn't force redraw in legacy renderer |
| [#4244](https://gitlab.com/gnachman/iterm2/-/issues/4244) | Once or twice: Apple-N (new window) freezes iterm | 2025-12-26 | - | f1443dd08, 68cc58ce0 | - | Cannot Reproduce | Vague freeze report with no repro steps; general UI freeze fixes (#386/#387) applied but unable to verify |
| [#4239](https://gitlab.com/gnachman/iterm2/-/issues/4239) | Enhancement Request: Set Window label instead of it changing | - | - | - | - | - | Skip | Feature request - not a bug |
| [#4182](https://gitlab.com/gnachman/iterm2/-/issues/4182) | Window title text stuck on screen even after DashTerm2 cl... | 2025-12-26 | - | - | - | Skip | UI behavior issue (title), not crash/hang - belongs in P2 |
| [#4172](https://gitlab.com/gnachman/iterm2/-/issues/4172) | When I change a session's profile via "Tab Prefs":General... | 2025-12-26 | - | - | - | Skip | UI behavior issue (profile change), not crash/hang - belongs in P2 |
| [#4081](https://gitlab.com/gnachman/iterm2/-/issues/4081) | OS X El Capitan split screen resize causes crash. Would be a | 2025-12-26 | - | a50b33161 | - | Fixed | Upstream fix: Fix window restoration for fullscreen windows in El Capitan |
| [#4053](https://gitlab.com/gnachman/iterm2/-/issues/4053) | DashTerm2 hangs for minutes while resizing the window. | 2025-12-26 | - | f1443dd08, 68cc58ce0 | - | Cannot Reproduce | Vague freeze report with no repro steps; general UI freeze fixes (#386/#387) applied but unable to verify |
| [#4030](https://gitlab.com/gnachman/iterm2/-/issues/4030) | Cursor doesn't change along with line height | 2025-12-26 | - | - | - | Skip | UI behavior issue (cursor), not crash/hang - belongs in P2-font-rendering |
| [#4009](https://gitlab.com/gnachman/iterm2/-/issues/4009) | Getting freezed when press "y" key on a single tab | 2025-12-26 | - | f1443dd08, 68cc58ce0 | - | Cannot Reproduce | Vague freeze report with no repro steps; general UI freeze fixes (#386/#387) applied but unable to verify |
| [#3990](https://gitlab.com/gnachman/iterm2/-/issues/3990) | Quitting iTerm hangs OS X | 2025-12-26 | - | f1443dd08, 68cc58ce0 | - | Cannot Reproduce | Vague freeze report with no repro steps; general UI freeze fixes (#386/#387) applied but unable to verify |
| [#3955](https://gitlab.com/gnachman/iterm2/-/issues/3955) | Show changes since installed version when upgrading | - | - | - | - | - | Skip | Feature request - not a bug |
| [#3951](https://gitlab.com/gnachman/iterm2/-/issues/3951) | Process Hang after Quit | 2025-12-26 | - | f1443dd08, 68cc58ce0 | - | Cannot Reproduce | Vague freeze report with no repro steps; general UI freeze fixes (#386/#387) applied but unable to verify |
| [#3842](https://gitlab.com/gnachman/iterm2/-/issues/3842) | Allow users to define a title for the window that shell e... | - | - | - | - | - | Skip | Feature request - not a bug |
| [#3809](https://gitlab.com/gnachman/iterm2/-/issues/3809) | Changing profiles is cumbersome and dangerous | 2025-12-26 | - | - | - | Skip | UI/UX issue (workflow), not crash/hang - belongs in P3 |
| [#3787](https://gitlab.com/gnachman/iterm2/-/issues/3787) | El Capitan crash | 2025-12-26 | - | a50b33161 | - | Fixed | Old 2015 El Capitan crash, covered by upstream fullscreen window fix |
| [#3779](https://gitlab.com/gnachman/iterm2/-/issues/3779) | Disable color change, Selection Text/Cursor Text | - | - | - | - | - | Skip | Feature request - not a bug |
| [#3757](https://gitlab.com/gnachman/iterm2/-/issues/3757) | feature req for cmd-f:  select text till end of line and ... | - | - | - | - | - | Skip | Feature request - not a bug |
| [#3716](https://gitlab.com/gnachman/iterm2/-/issues/3716) | Changing Tab Colour Sometimes Causes Tab to Crash | 2025-12-26 | - | - | - | Cannot Reproduce | 2015 vague "sometimes crashes" tab color with no stack trace; ancient version |
| [#3709](https://gitlab.com/gnachman/iterm2/-/issues/3709) | cannot change highlight color for trigger | - | - | - | - | - | Skip | Feature request - not a bug |
| [#3707](https://gitlab.com/gnachman/iterm2/-/issues/3707) | Stuck hint | 2025-12-26 | - | - | - | Skip | UI behavior issue (hint), not crash/hang - belongs in P2 |
| [#3679](https://gitlab.com/gnachman/iterm2/-/issues/3679) | change directory on mouseclick | - | - | - | - | - | Skip | Feature request - not a bug |
| [#3645](https://gitlab.com/gnachman/iterm2/-/issues/3645) | Control-shift-- "hangs" DashTerm2 | 2025-12-26 | - | f1443dd08, 68cc58ce0 | - | Cannot Reproduce | Vague freeze report with no repro steps; general UI freeze fixes (#386/#387) applied but unable to verify |
| [#3636](https://gitlab.com/gnachman/iterm2/-/issues/3636) | Changes to shell integration files | 2025-12-26 | - | - | - | Skip | UI behavior issue (shell integration), not crash/hang - belongs in P2 |
| [#3632](https://gitlab.com/gnachman/iterm2/-/issues/3632) | Change of profile does not apply to session log | 2025-12-26 | - | - | - | Skip | UI behavior issue (profile), not crash/hang - belongs in P2 |
| [#3468](https://gitlab.com/gnachman/iterm2/-/issues/3468) | Speed up accessibility's _allText method [was: constant s... | 2025-12-26 | 2025-12-20 | 68cc58ce0 | - | Fixed | Fixed in #386: Cache accessibility text buffer |
| [#3346](https://gitlab.com/gnachman/iterm2/-/issues/3346) | DashTerm2 window placement change on user swap and return | 2025-12-26 | - | - | - | Skip | UI behavior issue (window placement), not crash/hang - belongs in P2 |
| [#3203](https://gitlab.com/gnachman/iterm2/-/issues/3203) | Do not change window height when displaying tabs | - | - | - | - | - | Skip | Feature request - not a bug |
| [#3060](https://gitlab.com/gnachman/iterm2/-/issues/3060) | Massive change of profiles | - | - | - | - | - | Skip | Feature request - not a bug |
| [#3049](https://gitlab.com/gnachman/iterm2/-/issues/3049) | Add option to change cursor style based on idle timeout | - | - | - | - | - | Skip | Feature request - not a bug |
| [#3042](https://gitlab.com/gnachman/iterm2/-/issues/3042) | freeze when item from menubar is selected | 2025-12-26 | - | f1443dd08, 68cc58ce0 | - | Cannot Reproduce | Vague freeze report with no repro steps; general UI freeze fixes (#386/#387) applied but unable to verify |
| [#3002](https://gitlab.com/gnachman/iterm2/-/issues/3002) | keyboard shortcut to change tab profile | - | - | - | - | - | Skip | Feature request - not a bug |
| [#2980](https://gitlab.com/gnachman/iterm2/-/issues/2980) | Shortcut to change profile of existing window | - | - | - | - | - | Skip | Feature request - not a bug |
| [#2829](https://gitlab.com/gnachman/iterm2/-/issues/2829) | Add a pointer action to change transparency | - | - | - | - | - | Skip | Feature request - not a bug |
| [#2229](https://gitlab.com/gnachman/iterm2/-/issues/2229) | After logout/in window width has changed. | 2025-12-26 | - | - | - | Skip | UI behavior issue (window width), not crash/hang - belongs in P2 |
| [#1722](https://gitlab.com/gnachman/iterm2/-/issues/1722) | Crash when exiting fullscreen with (maximized?) tmux pane | 2025-12-26 | - | c01e4b0ba | - | Fixed | Upstream fix: Don't setClientSize when width/height negative (monitor unplug) |
| [#1592](https://gitlab.com/gnachman/iterm2/-/issues/1592) | Launching DashTerm2 randomly crashes whole OS X GUI | 2025-12-26 | - | - | - | External | Old 2014 macOS GUI crash, likely external system issue |
| [#1353](https://gitlab.com/gnachman/iterm2/-/issues/1353) | Killall iTerm lauch crash reporter | 2025-12-26 | - | - | - | Skip | Crash reporter behavior (expected when killed), not a bug |

---

## Statistics

| Metric | Count |
|--------|-------|
| Total | 289 |
| Fixed | 99 |
| In Progress | 0 |
| Cannot Reproduce | 61 |
| External | 11 |
| Open | 0 |
| Skip (UI behavior/Feature) | 118 |
| Wontfix | 0 |

---

## Category Notes

_Add notes specific to Crashes and Hangs bugs here._

### Common Patterns

_Document common root causes or fix patterns for this category._

### Related Files

_List source files commonly involved in these bugs._

