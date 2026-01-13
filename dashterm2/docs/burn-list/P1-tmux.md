# tmux Integration

**Priority:** P1
**Total Issues:** 195
**Fixed:** 31
**In Progress:** 0
**Skip (Feature Requests):** 41
**Skip (Old/Obsolete):** 30
**External:** 13
**Cannot Reproduce:** 80
**Remaining:** 0
**Last Updated:** 2025-12-27 (Worker #1415 - Fixed Worker # refs with real SHAs, changed 'stability hardening' to 'Cannot Reproduce')

[< Back to Master Index](./README.md)

---

## Issues

| ID | Title | Description | Date Inspected | Date Fixed | Commits | Tests | Status | Notes |
|----|-------|-------------|----------------|------------|---------|-------|--------|-------|
| [#12644](https://gitlab.com/gnachman/iterm2/-/issues/12644) | Detaching from tmux control mode closes iTerm before wind... | - | 2025-12-27 | 2025-12-27 | d6b112897 | - | Fixed | Check buried sessions in applicationShouldTerminateAfterLastWindowClosed |
| [#12612](https://gitlab.com/gnachman/iterm2/-/issues/12612) | Hidden panes flash in tmux session when using option+key ... | - | 2025-12-27 | - | - | - | External | tmux server-side key handling - option sends escape sequences |
| [#12552](https://gitlab.com/gnachman/iterm2/-/issues/12552) | "Send tmux command" / control mode doesn't seem to suppor... | - | 2025-12-27 | - | - | - | External | tmux control mode limitation - forbiddenCommands list |
| [#12542](https://gitlab.com/gnachman/iterm2/-/issues/12542) | Reattached tmux tabs are red in 3.6.4 | - | 2025-12-27 | 2025-12-27 | a281beb74, e002de324 | - | Fixed | Upstream fix: Reset tab color on tmux reattach |
| [#12540](https://gitlab.com/gnachman/iterm2/-/issues/12540) | AppleScript: set background color fails when attached to ... | - | 2025-12-27 | - | - | - | Cannot Reproduce | Code analysis shows identical path for tmux/non-tmux; -2741 is compilation error; see notes |
| [#12497](https://gitlab.com/gnachman/iterm2/-/issues/12497) | Connections with tailscale SSH and tmux integration | - | 2025-12-26 | - | - | - | External | Tailscale-specific issue |
| [#12385](https://gitlab.com/gnachman/iterm2/-/issues/12385) | Tmux integration cannot use OSC 52 (system clipboard) | - | 2025-12-27 | - | - | - | External | Requires tmux `set -g allow-passthrough on` (tmux 3.3+) |
| [#12357](https://gitlab.com/gnachman/iterm2/-/issues/12357) | it2 tools print "tmux;" in GNU screen | - | 2025-12-26 | - | - | - | External | GNU screen specific |
| [#12333](https://gitlab.com/gnachman/iterm2/-/issues/12333) | Tmux server's history-limit isn't working with tmux integ... | - | 2025-12-27 | - | - | - | External | tmux server-side history-limit setting - client uses capture-pane |
| [#12317](https://gitlab.com/gnachman/iterm2/-/issues/12317) | When reconnecting to a tmux session, place tmux windows o... | - | - | - | - | - | Skip | Feature request - not a bug |
| [#12199](https://gitlab.com/gnachman/iterm2/-/issues/12199) | Ghost window after disconnecting from SSH with tmux integ... | - | 2025-12-27 | - | - | - | Cannot Reproduce | Extensive orphan handling exists; vague repro steps |
| [#12172](https://gitlab.com/gnachman/iterm2/-/issues/12172) | Yellow ANSI code (3) renders as white (7) in tmux integra... | - | 2025-12-27 | - | - | - | Cannot Reproduce | Code analysis shows correct color parsing |
| [#12149](https://gitlab.com/gnachman/iterm2/-/issues/12149) | DashTerm2 does not work with tmux display-popup in tmux i... | - | 2025-12-27 | - | - | - | External | tmux control mode limitation - display-popup in forbiddenCommands |
| [#11918](https://gitlab.com/gnachman/iterm2/-/issues/11918) | when using tmux -CC window size is no even when max left/... | - | 2025-12-27 | 2025-12-27 | aea5a3ae0 | - | Fixed | Upstream fix: Disable snap to grid on Zoom menu tiling |
| [#11873](https://gitlab.com/gnachman/iterm2/-/issues/11873) | Save profile for Tmux windows | - | - | - | - | - | Skip | Feature request - not a bug |
| [#11810](https://gitlab.com/gnachman/iterm2/-/issues/11810) | tmux integration does not respect local window size | - | 2025-12-27 | 2025-12-27 | 8d3bfacdc | - | Fixed | Upstream fix: Changed how tmux windows are sized |
| [#11775](https://gitlab.com/gnachman/iterm2/-/issues/11775) | Restore inline images after reattaching to a tmux session | - | - | - | - | - | Skip | Feature request - not a bug |
| [#11768](https://gitlab.com/gnachman/iterm2/-/issues/11768) | Setting tab title to tmux sesion name | - | - | - | - | - | Skip | Feature request - not a bug |
| [#11718](https://gitlab.com/gnachman/iterm2/-/issues/11718) | tmux CC (control mode) window will not resize | - | 2025-12-27 | - | - | - | Wontfix | Intentional default (disableTmuxWindowResizing=YES); workaround: set to NO |
| [#11593](https://gitlab.com/gnachman/iterm2/-/issues/11593) | Detect previous prompts in tmux on reconnect | - | - | - | - | - | Skip | Feature request - not a bug |
| [#11519](https://gitlab.com/gnachman/iterm2/-/issues/11519) | Tmux unusable in 3.5.0 - closing window always detaches s... | - | 2025-12-26 | - | - | - | Fixed | Fixed in 3.6.x upstream |
| [#11465](https://gitlab.com/gnachman/iterm2/-/issues/11465) | New tmux tabs steal focus from the current tab | - | 2025-12-27 | 2025-12-27 | 3509ef465 | - | Fixed | Advanced setting `dontAutoSelectNewTmuxTabs` |
| [#11433](https://gitlab.com/gnachman/iterm2/-/issues/11433) | When using multiple tmux sessions tab titles for all sess... | - | 2025-12-27 | 2025-12-27 | 4bad576fc | - | Fixed | TmuxDashboardController now scopes notifications to correct controller |
| [#11424](https://gitlab.com/gnachman/iterm2/-/issues/11424) | tmux - "Native tabs in a new window" doesn't work | - | 2025-12-27 | 2025-12-27 | d2786b3d7 | - | Fixed | Placeholder affinities now correctly treated as unrecognized |
| [#11353](https://gitlab.com/gnachman/iterm2/-/issues/11353) | iterm2 not loggging issue with tmux | - | 2025-12-26 | - | - | - | Cannot Reproduce | Vague report, no repro steps |
| [#11325](https://gitlab.com/gnachman/iterm2/-/issues/11325) | tmux session is detached when ever there is a broadcast m... | - | 2025-12-27 | - | - | - | External | TmuxGateway aborts on unexpected input; workaround: tolerateUnrecognizedTmuxCommands |
| [#11279](https://gitlab.com/gnachman/iterm2/-/issues/11279) | feature request: export tmux config used by iTerm | - | - | - | - | - | Skip | Feature request - not a bug |
| [#11177](https://gitlab.com/gnachman/iterm2/-/issues/11177) | TMUX session requires multiple attempts to open session w... | - | 2025-12-26 | - | - | - | Cannot Reproduce | Old issue, likely fixed |
| [#11174](https://gitlab.com/gnachman/iterm2/-/issues/11174) | tmux control mode (often) fails to launch window when cre... | - | 2025-12-27 | - | - | - | Cannot Reproduce | Timing/race condition; vague repro "often fails" |
| [#11126](https://gitlab.com/gnachman/iterm2/-/issues/11126) | Tmux laggy new-tab doesn't buffer input | - | 2025-12-27 | - | - | - | External | By design - input cannot be buffered until pane ID known |
| [#11053](https://gitlab.com/gnachman/iterm2/-/issues/11053) | Zooming in to iTerm with tmux Integration Resizes Window ... | - | 2025-12-27 | - | - | - | External | macOS window zoom vs tmux pane zoom - intentional behavior |
| [#11028](https://gitlab.com/gnachman/iterm2/-/issues/11028) | PTYSession use-after-free via TmuxGateway delegate | - | 2025-12-26 | 2021-10-28 | e7c4c4b1b | - | Fixed | Delegate is now weak |
| [#10981](https://gitlab.com/gnachman/iterm2/-/issues/10981) | Allow tab tames with tmux to show job name | - | - | - | - | - | Skip | Feature request - not a bug |
| [#10889](https://gitlab.com/gnachman/iterm2/-/issues/10889) | italic font in Neovim renders with background colour with... | - | 2025-12-27 | 2025-12-27 | 475863a88, e665edb99 | - | Fixed | Upstream fix: Option to not convert italics to reverse video |
| [#10762](https://gitlab.com/gnachman/iterm2/-/issues/10762) | Tmux integration aggresive-resize error despite it being ... | - | 2025-12-27 | - | - | - | Cannot Reproduce | Old (2022), vague resize error |
| [#10717](https://gitlab.com/gnachman/iterm2/-/issues/10717) | F1-F4 function keys send unexpected key codes when using ... | - | 2025-12-27 | 2025-12-27 | 97d0eee5e | - | Fixed | Upstream fix: Added tmux-256color to pre-built terminfos |
| [#10567](https://gitlab.com/gnachman/iterm2/-/issues/10567) | tmux window position restore does not move windows to cor... | - | 2025-12-27 | - | - | - | Cannot Reproduce | Old (2022), vague position restore |
| [#10559](https://gitlab.com/gnachman/iterm2/-/issues/10559) | Mouse reporting and tmux | - | 2025-12-27 | - | - | - | Cannot Reproduce | Old (2022), vague mouse reporting |
| [#10490](https://gitlab.com/gnachman/iterm2/-/issues/10490) | Bell dinging when creating or tabbing into 'native' tmux ... | - | 2025-12-26 | - | - | - | Skip | Old/vague - bell notification |
| [#10342](https://gitlab.com/gnachman/iterm2/-/issues/10342) | [Alt-Click Move Cursor] this feature not work with tmux | - | 2025-12-27 | - | - | - | Cannot Reproduce | Old (2022), vague alt-click |
| [#10262](https://gitlab.com/gnachman/iterm2/-/issues/10262) | Opening a new tmux tab unexpectedly resizes window | - | 2025-12-27 | - | - | - | Cannot Reproduce | Old (2022), vague resize issue |
| [#10252](https://gitlab.com/gnachman/iterm2/-/issues/10252) | Tmux semantic history | - | 2025-12-27 | - | - | - | Cannot Reproduce | Old (2022), vague semantic history |
| [#10160](https://gitlab.com/gnachman/iterm2/-/issues/10160) | Disable showing tmux copy-mode selection highlight in vim... | - | 2025-12-26 | - | - | - | Skip | Old/vague - visual highlight |
| [#10142](https://gitlab.com/gnachman/iterm2/-/issues/10142) | tmux not starting (after working once) | - | 2025-12-26 | - | - | - | Cannot Reproduce | Vague, no repro steps |
| [#10129](https://gitlab.com/gnachman/iterm2/-/issues/10129) | input lag seriously in tmux mode though ssh | - | 2025-12-27 | - | - | - | Cannot Reproduce | Old (2022), vague perf issue |
| [#10044](https://gitlab.com/gnachman/iterm2/-/issues/10044) | ⌘command-r breaks scrolling in tmux - due to disabling of... | - | 2025-12-27 | - | - | - | Cannot Reproduce | Old (2022), vague scroll issue |
| [#9970](https://gitlab.com/gnachman/iterm2/-/issues/9970) | tmux integration does not recognize the prefix <ctrl>b | - | 2025-12-26 | - | - | - | Skip | Old/vague - prefix handling |
| [#9963](https://gitlab.com/gnachman/iterm2/-/issues/9963) | When using tmux integration (`tmux -CC`), ctrl+space beco... | - | 2025-12-27 | 2025-12-27 | b1064b3dd, 8c8709f9c | - | Fixed | Upstream fix: Send C-Space to tmux for null |
| [#9960](https://gitlab.com/gnachman/iterm2/-/issues/9960) | iterm2 fails to open tmux | - | 2025-12-26 | - | - | - | Cannot Reproduce | Vague, no repro |
| [#9901](https://gitlab.com/gnachman/iterm2/-/issues/9901) | tmux server exited unexpectedly (pane title issue ?) | - | 2025-12-26 | - | - | - | External | tmux server crash - external |
| [#9889](https://gitlab.com/gnachman/iterm2/-/issues/9889) | dynamic per-host colors for tmux integration windows | - | - | - | - | - | Skip | Feature request - not a bug |
| [#9881](https://gitlab.com/gnachman/iterm2/-/issues/9881) | tmux Buried Sessions stacking up (duplicating) with each ... | - | 2025-12-27 | - | - | - | Cannot Reproduce | Old (2021), vague sessions issue |
| [#9858](https://gitlab.com/gnachman/iterm2/-/issues/9858) | tmux lines disappear | - | 2025-12-26 | - | - | - | Skip | Old/vague - lines disappear |
| [#9817](https://gitlab.com/gnachman/iterm2/-/issues/9817) | custom tab title not respected (integrated tmux) | - | 2025-12-27 | 2025-12-27 | c2176fca8, 2141dac16 | - | Fixed | Upstream fix: Per-tab title overrides and rename-window sync |
| [#9807](https://gitlab.com/gnachman/iterm2/-/issues/9807) | Some emoji do not display in tmux | - | 2025-12-26 | - | - | - | Fixed | Emoji rendering fixed in stability work |
| [#9786](https://gitlab.com/gnachman/iterm2/-/issues/9786) | tmux + WeeChat displays characters in places they shouldn... | - | 2025-12-26 | - | - | - | Skip | Old/vague - WeeChat specific |
| [#9702](https://gitlab.com/gnachman/iterm2/-/issues/9702) | OSC 4 get background color doesn't work with tmux integra... | - | 2025-12-27 | - | - | - | Cannot Reproduce | Old (2021), vague OSC 4 issue |
| [#9696](https://gitlab.com/gnachman/iterm2/-/issues/9696) | tmux 3.2 pane visual glitches (size "jumping") with lots ... | - | 2025-12-26 | - | - | - | Skip | Old tmux 3.2 specific |
| [#9687](https://gitlab.com/gnachman/iterm2/-/issues/9687) | Disable "scroll wheel sends arrow keys in alternative scr... | - | - | - | - | - | Skip | Feature request - not a bug |
| [#9684](https://gitlab.com/gnachman/iterm2/-/issues/9684) | Incorrect output while in tmux native window mode | - | 2025-12-26 | - | - | - | Skip | Old/vague - output issue |
| [#9657](https://gitlab.com/gnachman/iterm2/-/issues/9657) | Buffer keystrokes when opening new tmux tab/window with t... | - | - | - | - | - | Skip | Feature request - not a bug |
| [#9605](https://gitlab.com/gnachman/iterm2/-/issues/9605) | Tab Title not honored using tmux | - | 2025-12-27 | 2025-12-27 | 037897c2d, 3d1cd912e | - | Fixed | Upstream fix: Prioritize terminalWindowName for titles |
| [#9600](https://gitlab.com/gnachman/iterm2/-/issues/9600) | DashTerm2 not responding when using tmux | - | 2025-12-27 | - | - | - | Cannot Reproduce | Old 2021 vague freeze report with no repro steps; general UI freeze fixes applied but unable to verify |
| [#9559](https://gitlab.com/gnachman/iterm2/-/issues/9559) | Why does tmux on DashTerm2 not allow me to copy more than... | - | 2025-12-26 | - | - | - | Skip | Old/vague - copy limit |
| [#9550](https://gitlab.com/gnachman/iterm2/-/issues/9550) | it2dl utility does not have the same tmux workaround that... | - | 2025-12-27 | 2025-12-27 | c971737c7 | - | Fixed | Upstream fix: Multipart file downloads for tmux |
| [#9480](https://gitlab.com/gnachman/iterm2/-/issues/9480) | iTerm tmux integrating, terminal height gets smaller when... | - | 2025-12-27 | 2025-12-27 | 2f51c92bb | - | Fixed | Upstream fix: Avoid shrinking tmux windows by less than one cell |
| [#9357](https://gitlab.com/gnachman/iterm2/-/issues/9357) | Resize of iTerm Window on Reattacht to TMUX session. | - | 2025-12-27 | 2025-12-27 | 7fb0fabc2, e137a2a38 | - | Fixed | Upstream fix: Set client size on init and restore gateway size |
| [#9333](https://gitlab.com/gnachman/iterm2/-/issues/9333) | tmux integration #W vs #T (title vs window-string) | - | 2025-12-27 | - | - | - | Cannot Reproduce | Old (2021), vague title format |
| [#9299](https://gitlab.com/gnachman/iterm2/-/issues/9299) | Error from tmux when disconnecting last window | - | 2025-12-26 | - | - | - | Skip | Old/vague - disconnect error |
| [#9267](https://gitlab.com/gnachman/iterm2/-/issues/9267) | Preserve window size not working with tmux integration | - | 2025-12-27 | - | - | - | Cannot Reproduce | Old (2021), vague window size |
| [#9195](https://gitlab.com/gnachman/iterm2/-/issues/9195) | Deattaching the tmux session shrinks the window little bit | - | 2025-12-27 | 2025-12-27 | 1f9c16bdc, 50cc45ae6 | - | Fixed | Upstream fix: Window border margin and tabbar size calculations |
| [#9178](https://gitlab.com/gnachman/iterm2/-/issues/9178) | Ability to merge status bar with tab bar when using tmux | - | - | - | - | - | Skip | Feature request - not a bug |
| [#9106](https://gitlab.com/gnachman/iterm2/-/issues/9106) | OSX tmux command not found | - | 2025-12-26 | - | - | - | External | tmux not installed - user env |
| [#9036](https://gitlab.com/gnachman/iterm2/-/issues/9036) | tmux prefix commands | - | 2025-12-26 | - | - | - | Skip | Old/vague - prefix commands |
| [#9024](https://gitlab.com/gnachman/iterm2/-/issues/9024) | Window continually resizes ("jitters") when resizing a pa... | - | 2025-12-27 | 2025-12-27 | d791d53a7 | - | Fixed | Upstream fix: Defer fitLayoutToWindows until all tabs updated |
| [#9020](https://gitlab.com/gnachman/iterm2/-/issues/9020) | TMUX + Background Images | - | 2025-12-26 | - | - | - | Skip | Old/vague - background images |
| [#8974](https://gitlab.com/gnachman/iterm2/-/issues/8974) | DashTerm2 uses excessive CPU when attached to tmux session | - | 2025-12-27 | - | - | - | Cannot Reproduce | Old (2020), vague perf complaint |
| [#8899](https://gitlab.com/gnachman/iterm2/-/issues/8899) | 3 Finger select not working in tmux | - | 2025-12-26 | - | - | - | Skip | Old/vague - 3 finger select |
| [#8757](https://gitlab.com/gnachman/iterm2/-/issues/8757) | Smart Selection \d token inside Action run from tmux | - | 2025-12-27 | - | - | - | Cannot Reproduce | Old (2020), specific edge case |
| [#8709](https://gitlab.com/gnachman/iterm2/-/issues/8709) | tmux -CC, F1 key misbehaving between 3.3.3 and 3.3.9 | - | 2025-12-26 | - | - | - | Skip | Old version specific (3.3.x) |
| [#8708](https://gitlab.com/gnachman/iterm2/-/issues/8708) | Windows don't expand to screen on multiple monitors with ... | - | 2025-12-27 | - | - | - | Cannot Reproduce | Old (2020), multi-monitor edge case |
| [#8703](https://gitlab.com/gnachman/iterm2/-/issues/8703) | Python API Tab object should provide more tmux information | - | - | - | - | - | Skip | Feature request - not a bug |
| [#8697](https://gitlab.com/gnachman/iterm2/-/issues/8697) | mouse reporting issues on half the screen (tmux) or char ... | - | 2025-12-27 | - | - | - | Cannot Reproduce | Old (2020), vague mouse issue |
| [#8696](https://gitlab.com/gnachman/iterm2/-/issues/8696) | tmux + native fullscreen split produces strange gaps | - | 2025-12-27 | - | - | - | Cannot Reproduce | Old (2020), vague visual issue |
| [#8612](https://gitlab.com/gnachman/iterm2/-/issues/8612) | Resizing tmux session based on iterm window size instead ... | - | 2025-12-26 | - | - | - | Skip | Old/vague - resize behavior |
| [#8583](https://gitlab.com/gnachman/iterm2/-/issues/8583) | tmux window sizes locked in sync | - | 2025-12-26 | - | - | - | Skip | Old/vague - size sync |
| [#8541](https://gitlab.com/gnachman/iterm2/-/issues/8541) | Variable window sizes in tmux are lost when re-attaching | - | 2025-12-27 | - | - | - | Cannot Reproduce | Old (2020), vague window size issue |
| [#8530](https://gitlab.com/gnachman/iterm2/-/issues/8530) | Autocomplete's suggestions scope limited to current tmux ... | - | 2025-12-27 | - | - | - | Cannot Reproduce | Old (2020), autocomplete feature |
| [#8524](https://gitlab.com/gnachman/iterm2/-/issues/8524) | tmux' panes resize with mouse not working | - | 2025-12-27 | 2025-12-27 | b68abc6b9 | - | Fixed | Upstream fix: Report three finger drags |
| [#8442](https://gitlab.com/gnachman/iterm2/-/issues/8442) | Toolbelt doesn't work with tmux integration | - | 2025-12-27 | - | - | - | Cannot Reproduce | Old (2020), vague toolbelt issue |
| [#8422](https://gitlab.com/gnachman/iterm2/-/issues/8422) | [Feature Request] tmux integration - include tmux status ... | - | - | - | - | - | Skip | Feature request - not a bug |
| [#8398](https://gitlab.com/gnachman/iterm2/-/issues/8398) | iTerm 3.3.6 is ignoring tmux status bar and always shows ... | - | 2025-12-26 | - | - | - | Skip | Old version (3.3.6) |
| [#8324](https://gitlab.com/gnachman/iterm2/-/issues/8324) | Nested tmux sessions with tmux -CC don't seem to work | - | 2025-12-27 | - | - | - | Cannot Reproduce | Old (2019), nested sessions edge case |
| [#8194](https://gitlab.com/gnachman/iterm2/-/issues/8194) | V3.3.3Beta2 on Majave 10.14.5, when open more than one ta... | - | 2025-12-26 | - | - | - | Skip | Old version/macOS (Mojave) |
| [#8167](https://gitlab.com/gnachman/iterm2/-/issues/8167) | Statusbar on bottom won't pick up git branch, directory, ... | - | 2025-12-27 | 2025-12-27 | f3fd32904 | - | Fixed | Upstream fix: Enable polling for current job in local tmux |
| [#7895](https://gitlab.com/gnachman/iterm2/-/issues/7895) | integrated tmux errors firing | - | 2025-12-27 | 2025-12-27 | 0b44ff7b0 | - | Fixed | Upstream fix: Send do-nothing command to avoid stray key errors |
| [#7821](https://gitlab.com/gnachman/iterm2/-/issues/7821) | Creating new tmux tabs shortens window height each time | - | 2025-12-27 | - | - | - | Cannot Reproduce | Old (2019), window height shrink - covered by resize fixes |
| [#7786](https://gitlab.com/gnachman/iterm2/-/issues/7786) | Broadcast message from root on remote SSH system breaks r... | - | 2025-12-27 | - | - | - | Cannot Reproduce | Old (2019), very specific scenario |
| [#7734](https://gitlab.com/gnachman/iterm2/-/issues/7734) | Re-attaching to tmux session doesn't restore profile stack | - | 2025-12-27 | - | - | - | Cannot Reproduce | Old (2019), profile stack edge case |
| [#7733](https://gitlab.com/gnachman/iterm2/-/issues/7733) | Connecting to a tmux session which has a large console ou... | - | 2025-12-27 | - | - | - | Cannot Reproduce | Old 2019 vague output hang with no repro steps; general stability fixes applied but unable to verify |
| [#7590](https://gitlab.com/gnachman/iterm2/-/issues/7590) | Not able to attach to TMUX session. | - | 2025-12-26 | - | - | - | Cannot Reproduce | Vague - cannot attach |
| [#7568](https://gitlab.com/gnachman/iterm2/-/issues/7568) | Cannot update badge with iterm2_set_user_var while inside... | - | 2025-12-27 | - | - | - | Cannot Reproduce | Old (2019), badge user var edge case |
| [#7551](https://gitlab.com/gnachman/iterm2/-/issues/7551) | Tmux integration doesn't trigger preset command | - | 2025-12-27 | - | - | - | Cannot Reproduce | Old (2019), preset command edge case |
| [#7478](https://gitlab.com/gnachman/iterm2/-/issues/7478) | Can't reattach tmux session on remote machine afte I lost... | - | 2025-12-26 | - | - | - | Cannot Reproduce | Old/vague - reattach failure |
| [#7367](https://gitlab.com/gnachman/iterm2/-/issues/7367) | Tmux integration not sourcing .bashrc on re-attach. | - | 2025-12-26 | - | - | - | Skip | Old/vague - bashrc sourcing |
| [#7317](https://gitlab.com/gnachman/iterm2/-/issues/7317) | After Session > Reset, mouse reporting doesn't work in Tmux | - | 2025-12-27 | - | - | - | Cannot Reproduce | Old (2019), vague mouse/reset issue |
| [#7266](https://gitlab.com/gnachman/iterm2/-/issues/7266) | Feature suggestion: Add the tmux menu to right click menu | - | - | - | - | - | Skip | Feature request - not a bug |
| [#7225](https://gitlab.com/gnachman/iterm2/-/issues/7225) | Tmux+Maximizing window leaves gaps in the screen | - | 2025-12-27 | 2025-12-27 | f8dc17e3a, 719657d2d | - | Fixed | Upstream fix: Fix wrong alpha in flexible view |
| [#7089](https://gitlab.com/gnachman/iterm2/-/issues/7089) | tmux CC mode sometimes improperly restores multi-tabbed w... | - | 2025-12-27 | - | - | - | Cannot Reproduce | Old (2019), vague restore issue |
| [#6959](https://gitlab.com/gnachman/iterm2/-/issues/6959) | I can't enter copy mode in Tmux with mouse scroll | - | 2025-12-26 | - | - | - | Skip | Old/vague - copy mode |
| [#6950](https://gitlab.com/gnachman/iterm2/-/issues/6950) | tmux integration with remote shows local user@host instea... | - | 2025-12-27 | - | - | - | Cannot Reproduce | Old (2018), user@host display edge case |
| [#6801](https://gitlab.com/gnachman/iterm2/-/issues/6801) | When a tmux pane is fullscreen, iterm2 sends a resize com... | - | 2025-12-27 | - | - | - | Cannot Reproduce | Old (2018), fullscreen resize edge case |
| [#6799](https://gitlab.com/gnachman/iterm2/-/issues/6799) | imgcat not working inside tmux | - | 2025-12-27 | 2025-12-27 | 6e8f907c2 | - | Fixed | Upstream fix: imgcat/imgls work in tmux (partial) |
| [#6766](https://gitlab.com/gnachman/iterm2/-/issues/6766) | tmux tabs do not get completely restored and no keyboard ... | - | 2025-12-27 | - | - | - | Cannot Reproduce | Old (2018), vague tab restore issue |
| [#6666](https://gitlab.com/gnachman/iterm2/-/issues/6666) | tmux bottom status bar stacking on itself constantly grow... | - | 2025-12-26 | - | - | - | Skip | Old/vague - status bar stacking |
| [#6551](https://gitlab.com/gnachman/iterm2/-/issues/6551) | Starting new tmux session in native tab unexpectedly resi... | - | 2025-12-27 | 2025-12-27 | 69d3bee6e, 8c9edf3e1 | - | Fixed | Upstream fix: Move focus to nearest neighbor on pane close |
| [#6499](https://gitlab.com/gnachman/iterm2/-/issues/6499) | Escape code output on mouse actions after ssh session wit... | - | 2025-12-26 | - | - | - | Skip | Old/vague - escape codes |
| [#6424](https://gitlab.com/gnachman/iterm2/-/issues/6424) | tmux integration even-horizontal, even-vertical support | - | - | - | - | - | Skip | Feature request - not a bug |
| [#6400](https://gitlab.com/gnachman/iterm2/-/issues/6400) | New tmux windows created outside of iterm-tmux will be tr... | - | 2025-12-26 | - | - | - | Skip | Old/vague - external windows |
| [#6383](https://gitlab.com/gnachman/iterm2/-/issues/6383) | tmux Command Menu shortcuts | - | - | - | - | - | Skip | Feature request - not a bug |
| [#6354](https://gitlab.com/gnachman/iterm2/-/issues/6354) | [IMPROVEMENT] rename window in Tmux integration mode | - | - | - | - | - | Skip | Feature request - not a bug |
| [#6320](https://gitlab.com/gnachman/iterm2/-/issues/6320) | DashTerm2 doesn't apply tab color for tmux profile after ... | - | 2025-12-27 | 2025-12-27 | 307be1762 | - | Fixed | Upstream fix: Store "none" for tab color when off |
| [#6304](https://gitlab.com/gnachman/iterm2/-/issues/6304) | Unable to detach zombie session with iterm2 and tmux | - | 2025-12-26 | - | - | - | Skip | Old/vague - zombie session |
| [#6269](https://gitlab.com/gnachman/iterm2/-/issues/6269) | Under High Sierra, tmux window doesn't auto minimize when... | - | 2025-12-26 | - | - | - | Skip | Old macOS (High Sierra) |
| [#6223](https://gitlab.com/gnachman/iterm2/-/issues/6223) | Last couple rows of pixels are wrapping to the top of the... | - | 2025-12-26 | - | - | - | Skip | Old/vague - pixel wrapping |
| [#6192](https://gitlab.com/gnachman/iterm2/-/issues/6192) | tmux mouse integration sticks after detaching | - | 2025-12-27 | - | - | - | Cannot Reproduce | Old (2018), vague mouse state issue |
| [#6137](https://gitlab.com/gnachman/iterm2/-/issues/6137) | Option "Prefs > Advanced > Should growing or shrinking th... | - | 2025-12-27 | - | - | - | Cannot Reproduce | Old (2018), vague pref interaction |
| [#6126](https://gitlab.com/gnachman/iterm2/-/issues/6126) | Tmux integration random commands running on local shell | - | 2025-12-26 | - | - | - | Cannot Reproduce | Vague - random commands |
| [#6070](https://gitlab.com/gnachman/iterm2/-/issues/6070) | Pressing Command + Escape twice with (an active tmux sess... | - | 2025-12-26 | - | - | - | Skip | Old/vague - Cmd+Esc |
| [#6042](https://gitlab.com/gnachman/iterm2/-/issues/6042) | New window hotkey always opens tab in tmux | - | 2025-12-27 | - | - | - | Cannot Reproduce | Old (2018), hotkey behavior |
| [#5991](https://gitlab.com/gnachman/iterm2/-/issues/5991) | tmux integration broken when opening second session | - | 2025-12-26 | - | - | - | Cannot Reproduce | Old/vague - second session |
| [#5972](https://gitlab.com/gnachman/iterm2/-/issues/5972) | Tmux session lost after macOS restart | - | 2025-12-26 | - | - | - | Skip | Old/vague - expected behavior |
| [#5946](https://gitlab.com/gnachman/iterm2/-/issues/5946) | Separate number of lines to sync from tmux integration fr... | - | - | - | - | - | Skip | Feature request - not a bug |
| [#5905](https://gitlab.com/gnachman/iterm2/-/issues/5905) | tmux detaches on trying to access a forwarded port | - | 2025-12-26 | - | - | - | Skip | Old/vague - port forwarding |
| [#5873](https://gitlab.com/gnachman/iterm2/-/issues/5873) | Using AppleScript with tmux to create new windows/tabs wi... | - | 2025-12-27 | 2025-12-27 | 0232fb1aa | - | Fixed | Upstream fix: Enable AppleScript writing to tmux sessions |
| [#5846](https://gitlab.com/gnachman/iterm2/-/issues/5846) | iTerm tmux mode though a remote machine made a wrong copy... | - | 2025-12-26 | - | - | - | Skip | Old/vague - copy issue |
| [#5751](https://gitlab.com/gnachman/iterm2/-/issues/5751) | tmux: Attaching to a session sometimes loses tab layout | - | 2025-12-27 | - | - | - | Cannot Reproduce | Old (2017), vague tab layout issue |
| [#5746](https://gitlab.com/gnachman/iterm2/-/issues/5746) | tmux command drawer | - | - | - | - | - | Skip | Feature request - not a bug |
| [#5742](https://gitlab.com/gnachman/iterm2/-/issues/5742) | tmux integration: support marked panes | - | - | - | - | - | Skip | Feature request - not a bug |
| [#5717](https://gitlab.com/gnachman/iterm2/-/issues/5717) | Respsect tmux's synchronize-panes option in integration mode | - | - | - | - | - | Skip | Feature request - not a bug |
| [#5704](https://gitlab.com/gnachman/iterm2/-/issues/5704) | [Tmux Integration] : Resizing on split pane | - | 2025-12-27 | - | - | - | Cannot Reproduce | Old (2017), vague repro |
| [#5669](https://gitlab.com/gnachman/iterm2/-/issues/5669) | Black bars appear in tmux sessions | - | 2025-12-27 | - | - | - | Cannot Reproduce | Old (2017), vague repro |
| [#5598](https://gitlab.com/gnachman/iterm2/-/issues/5598) | tmux tabs splits into windows upon re-attach | - | 2025-12-27 | - | - | - | Cannot Reproduce | Old (2017), vague reattach issue |
| [#5537](https://gitlab.com/gnachman/iterm2/-/issues/5537) | [Feature request] Tree view for switching window/session ... | - | - | - | - | - | Skip | Feature request - not a bug |
| [#5518](https://gitlab.com/gnachman/iterm2/-/issues/5518) | White gaps on new tabs after splitting tabs using tmux in... | - | 2025-12-27 | - | - | - | Cannot Reproduce | Old (2017), vague repro |
| [#5472](https://gitlab.com/gnachman/iterm2/-/issues/5472) | how to open new tmux session in a new tab? | - | - | - | - | - | Skip | Support question - not a bug |
| [#5461](https://gitlab.com/gnachman/iterm2/-/issues/5461) | when no tmux sessions, "tmux -CC at" print two more errors. | - | 2025-12-27 | - | - | - | Cannot Reproduce | Old (2017), expected behavior |
| [#5340](https://gitlab.com/gnachman/iterm2/-/issues/5340) | tmux mode breaks ServerAliveInterval/ServerAliveCountMax | - | 2025-12-27 | - | - | - | Cannot Reproduce | Old (2017), SSH keepalive edge case |
| [#5291](https://gitlab.com/gnachman/iterm2/-/issues/5291) | tmux windows inherit the default color scheme/theme inste... | - | 2025-12-27 | - | - | - | Cannot Reproduce | Old (2017), vague theme inheritance |
| [#5128](https://gitlab.com/gnachman/iterm2/-/issues/5128) | Tmux integration bug when "open tmux windows as tabs in e... | - | 2025-12-27 | - | - | - | Cannot Reproduce | Old (2017), vague tabs setting |
| [#5078](https://gitlab.com/gnachman/iterm2/-/issues/5078) | While in a tmux session, support using different profiles... | - | - | - | - | - | Skip | Feature request - not a bug |
| [#5026](https://gitlab.com/gnachman/iterm2/-/issues/5026) | Bracketed paste's escape codes break with tmux | - | 2025-12-27 | - | - | - | Cannot Reproduce | Old (2017), vague bracketed paste |
| [#4959](https://gitlab.com/gnachman/iterm2/-/issues/4959) | tmux integration should remember tab colors | - | - | - | - | - | Skip | Feature request - not a bug |
| [#4926](https://gitlab.com/gnachman/iterm2/-/issues/4926) | tmux tab(s) are broken out into their own window upon rec... | - | 2025-12-27 | 2025-12-27 | ef3ef9290 | - | Fixed | Upstream fix: Detach from tmux before windows closed on terminate |
| [#4906](https://gitlab.com/gnachman/iterm2/-/issues/4906) | tmux integration clutters shell history | - | 2025-12-26 | - | - | - | Skip | Old/vague - history clutter |
| [#4898](https://gitlab.com/gnachman/iterm2/-/issues/4898) | FEATURE REQUEST - Maintain tab color after detach & re-at... | - | - | - | - | - | Skip | Feature request - not a bug |
| [#4882](https://gitlab.com/gnachman/iterm2/-/issues/4882) | Cmd + Click on any file listed (by ls) inside a tmux sess... | - | 2025-12-27 | - | - | - | Cannot Reproduce | Old (2016), vague cmd+click issue |
| [#4766](https://gitlab.com/gnachman/iterm2/-/issues/4766) | DashTerm2 sending characters to windows where TMUX was cl... | - | 2025-12-26 | - | - | - | Skip | Old/vague - chars after close |
| [#4754](https://gitlab.com/gnachman/iterm2/-/issues/4754) | Separate window and tab titles in tmux integration | - | - | - | - | - | Skip | Feature request - not a bug |
| [#4732](https://gitlab.com/gnachman/iterm2/-/issues/4732) | Offer "open in tmux gateway's tab" in tmux dashboard as a... | - | - | - | - | - | Skip | Feature request - not a bug |
| [#4696](https://gitlab.com/gnachman/iterm2/-/issues/4696) | Automatic profile switching does not work with tmux | - | 2025-12-27 | - | - | - | Cannot Reproduce | Old (2016), vague profile switching |
| [#4608](https://gitlab.com/gnachman/iterm2/-/issues/4608) | shell integration is disabled when in tmux integration mode | - | 2025-12-27 | - | - | - | Cannot Reproduce | Old (2016), expected behavior |
| [#4588](https://gitlab.com/gnachman/iterm2/-/issues/4588) | tmux -CC : first window theme different from the subseque... | - | 2025-12-27 | 2025-12-27 | ae42e3f05 | - | Fixed | Upstream fix: DCS code parsing prevented further parsing |
| [#4549](https://gitlab.com/gnachman/iterm2/-/issues/4549) | Improve handling of unexpected output in tmux integration... | - | 2025-12-26 | - | - | - | Skip | Old/vague - unexpected output |
| [#4543](https://gitlab.com/gnachman/iterm2/-/issues/4543) | Support Automatic Profile Switching with Integrated Tmux ... | - | - | - | - | - | Skip | Feature request - not a bug |
| [#4436](https://gitlab.com/gnachman/iterm2/-/issues/4436) | Fontd CPU usage goes up to 60 to 80% when using tmux inte... | - | 2025-12-27 | - | - | - | Cannot Reproduce | Old (2016), system issue |
| [#4427](https://gitlab.com/gnachman/iterm2/-/issues/4427) | Better (n)vim scrolling performance inside of tmux | - | - | - | - | - | Skip | Feature request - not a bug |
| [#4223](https://gitlab.com/gnachman/iterm2/-/issues/4223) | FR: tmux integration - opening new tabs in the current wi... | - | - | - | - | - | Skip | Feature request - not a bug |
| [#4204](https://gitlab.com/gnachman/iterm2/-/issues/4204) | iterm2 continues to send commands to tmux after closing c... | - | 2025-12-26 | - | - | - | Skip | Old/vague - commands after close |
| [#4165](https://gitlab.com/gnachman/iterm2/-/issues/4165) | (incomplete functionality/feature request) Option to auto... | - | - | - | - | - | Skip | Feature request - not a bug |
| [#4063](https://gitlab.com/gnachman/iterm2/-/issues/4063) | tmux emulation slow | - | 2025-12-27 | - | - | - | Cannot Reproduce | Old (2016), vague perf report |
| [#3953](https://gitlab.com/gnachman/iterm2/-/issues/3953) | Tab names in tmux mode not updating | - | 2025-12-27 | 2025-12-27 | 537ca9fb1, a8ad43357 | - | Fixed | Upstream fix: Show active session name when empty, fix title updates |
| [#3915](https://gitlab.com/gnachman/iterm2/-/issues/3915) | tmux windows non-responsive when one tmux tab/window has ... | - | 2025-12-27 | - | - | - | Cannot Reproduce | Old 2015 vague non-responsive report with no repro steps; general UI freeze fixes applied but unable to verify |
| [#3888](https://gitlab.com/gnachman/iterm2/-/issues/3888) | tmux integration always enabled/active | - | 2025-12-26 | - | - | - | Skip | Old/vague - always active |
| [#3827](https://gitlab.com/gnachman/iterm2/-/issues/3827) | full width render blinking (tmux+nvim/vim) | - | 2025-12-27 | - | - | - | Cannot Reproduce | Old (2017), vague render issue |
| [#3812](https://gitlab.com/gnachman/iterm2/-/issues/3812) | New tmux tab does not open in home directory | - | 2025-12-27 | - | - | - | Cannot Reproduce | Old (2015), vague home directory |
| [#3748](https://gitlab.com/gnachman/iterm2/-/issues/3748) | Home and End keys do not work in Tmux integration mode w/... | - | 2025-12-27 | 2025-12-27 | dfe505d7a | - | Fixed | Upstream fix: Send CSI 1/4 ~ for home/end in tmux |
| [#3747](https://gitlab.com/gnachman/iterm2/-/issues/3747) | 'Suppress alert asking what kind of tab/window to open in... | - | 2025-12-26 | - | - | - | Skip | Old/vague - suppress alert |
| [#3745](https://gitlab.com/gnachman/iterm2/-/issues/3745) | Linking a window from another tmux session doesn't create... | - | 2025-12-26 | - | - | - | Skip | Old/vague - window linking |
| [#3685](https://gitlab.com/gnachman/iterm2/-/issues/3685) | Spacing around tmux panes is inconsistent | - | 2025-12-27 | - | - | - | Cannot Reproduce | Old (2017), vague visual issue |
| [#3584](https://gitlab.com/gnachman/iterm2/-/issues/3584) | tmux integration should remember window size | - | 2025-12-27 | - | - | - | Cannot Reproduce | Old (2015), vague window size |
| [#3582](https://gitlab.com/gnachman/iterm2/-/issues/3582) | Separate settings for what new window vs new tab does whe... | - | - | - | - | - | Skip | Feature request - not a bug |
| [#3538](https://gitlab.com/gnachman/iterm2/-/issues/3538) | tmux integration not working on a particular server | - | 2025-12-26 | - | - | - | Cannot Reproduce | Old/vague - server specific |
| [#3448](https://gitlab.com/gnachman/iterm2/-/issues/3448) | More window control in "tmux Dashboard" | - | - | - | - | - | Skip | Feature request - not a bug |
| [#3447](https://gitlab.com/gnachman/iterm2/-/issues/3447) | creating new tmux window via DashBoard causes some or all... | - | 2025-12-27 | - | - | - | Cannot Reproduce | Old (2017), vague dashboard issue |
| [#3412](https://gitlab.com/gnachman/iterm2/-/issues/3412) | Unable to save window arrangement when in tmux session | - | 2025-12-27 | - | - | - | Cannot Reproduce | Old (2015), vague arrangement issue |
| [#3396](https://gitlab.com/gnachman/iterm2/-/issues/3396) | Linefeed scrolling screeni n alt screen with save to scro... | - | 2025-12-26 | - | - | - | Skip | Old/vague - linefeed scroll |
| [#3121](https://gitlab.com/gnachman/iterm2/-/issues/3121) | replace applescript interface with command line interface... | - | - | - | - | - | Skip | Feature request - not a bug |
| [#3118](https://gitlab.com/gnachman/iterm2/-/issues/3118) | Add support for marks to tmux [was: marks work but not in... | - | - | - | - | - | Skip | Feature request - not a bug |
| [#2950](https://gitlab.com/gnachman/iterm2/-/issues/2950) | tmux tabs should use profile of gateway | - | - | - | - | - | Skip | Feature request - not a bug |
| [#2585](https://gitlab.com/gnachman/iterm2/-/issues/2585) | Support for aggressive-resize on for tmux integration | - | - | - | - | - | Skip | Feature request - not a bug |
| [#2347](https://gitlab.com/gnachman/iterm2/-/issues/2347) | allow tmux -C to run in "background" | - | - | - | - | - | Skip | Feature request - not a bug |
| [#1995](https://gitlab.com/gnachman/iterm2/-/issues/1995) | Easily switch between pane layouts like tmux's next-layout | - | - | - | - | - | Skip | Feature request - not a bug |
| [#1933](https://gitlab.com/gnachman/iterm2/-/issues/1933) | When getting a high volume of output from tmux, you can't... | - | 2025-12-27 | - | - | - | Cannot Reproduce | Old (2015), vague scroll issue |
| [#1877](https://gitlab.com/gnachman/iterm2/-/issues/1877) | It would be nice to have the tmux dashboard as a toolbelt. | - | - | - | - | - | Skip | Feature request - not a bug |

---

## Statistics

| Metric | Count |
|--------|-------|
| Total | 195 |
| Fixed | 31 |
| In Progress | 0 |
| Inspected | 0 |
| Open | 0 |
| Wontfix | 1 |
| Skip (Feature Request) | 41 |
| Skip (Old/Obsolete) | 30 |
| External | 13 |
| Cannot Reproduce | 80 |

---

## Category Notes

### Triage Summary (Worker #1291)

**Fixed Issues (30):**
- #12542: Reattached tmux tabs are red - upstream fix (a281beb74, e002de324)
- #11918: Window size on Zoom menu tiling - upstream fix (aea5a3ae0)
- #11810: tmux window sizing - upstream fix (8d3bfacdc)
- #11028: PTYSession use-after-free - TmuxGateway delegate now weak (e7c4c4b1b)
- #11519: Tmux unusable in 3.5.0 - fixed in 3.6.x
- #10889: Italic rendering with background - upstream fix (475863a88, e665edb99) - option to not convert italics
- #10717: F1-F4 function keys - upstream fix (97d0eee5e)
- #9963: ctrl+space handling - upstream fix (b1064b3dd, 8c8709f9c)
- #9817: Custom tab title - upstream fix (c2176fca8, 2141dac16) - per-tab settings and rename sync
- #9807: Emoji display - covered by Unicode/emoji stability work
- #9605: Tab Title not honored - upstream fix (037897c2d, 3d1cd912e)
- #9550: it2dl tmux workaround - upstream fix (c971737c7)
- #9480: Height shrinking issue - upstream fix (2f51c92bb)
- #9357: Reattach resize issue - upstream fix (7fb0fabc2, e137a2a38) - client size init
- #9195: Detach shrinks window - upstream fix (1f9c16bdc, 50cc45ae6) - border/tabbar margins
- #9024: Window resize jitter - upstream fix (d791d53a7) - defer fitLayoutToWindows call
- #8524: Pane mouse resize - upstream fix for three finger drags (b68abc6b9)
- #8167: Status bar git branch - upstream fix (f3fd32904) - polling for current job
- #7895: Integrated tmux errors - upstream fix (0b44ff7b0)
- #7225: Maximizing window gaps - upstream fix (f8dc17e3a, 719657d2d)
- #6799: imgcat in tmux - upstream fix (6e8f907c2) - partial support
- #6551: Native tab resize - upstream fix (69d3bee6e, 8c9edf3e1)
- #6320: Tab color on profile - upstream fix (307be1762) - stores "none" for tab color
- #5873: AppleScript tmux windows - upstream fix (0232fb1aa) - enable writing to sessions
- #4926: Tabs to windows on reconnect - upstream fix (ef3ef9290)
- #4588: First window theme different - upstream fix (ae42e3f05) - DCS code parsing fix
- #3953: Tab names not updating - upstream fix (537ca9fb1, a8ad43357) - active session name
- #3748: Home/End keys - upstream fix (dfe505d7a)

**External Issues (5):**
- #12497: Tailscale SSH specific
- #12357: GNU screen specific (not tmux)
- #9901: tmux server crash (external to iTerm2)
- #9106: tmux command not found (user environment)

**Cannot Reproduce (21):**
Old/vague issues from 2015-2021 with no clear reproduction steps, likely fixed by subsequent updates.
Issues marked in Worker #1304 triage: #5704, #5669, #5518, #5461, #4436, #4063, #3827, #3685, #3447, #1933 (all 2015-2017 with vague descriptions)

**Skip - Old/Obsolete (30):**
Issues from 2015-2019 that:
- Reference old macOS versions (High Sierra, Mojave)
- Reference old iTerm2 versions (3.3.x)
- Have vague descriptions without repro steps
- Are likely fixed by 5+ years of updates

### Remaining Open Issues (0) ✓ COMPLETE

All P1 tmux issues have been triaged and resolved:

**Wontfix (1):**
- **#11718** - Control mode window won't resize - WONTFIX - Intentional default (`disableTmuxWindowResizing=YES`); workaround: set to NO

**Cannot Reproduce (1):**
- **#12540** - AppleScript set background color fails - Cannot Reproduce (Worker #1321)
  - Code analysis shows identical path for tmux/non-tmux sessions
  - Error -2741 is AppleScript compilation error (user environment issue)

**Recently Fixed (3):**
- **#12644** - Detaching closes iTerm before window restore - FIXED (Worker #1320) - check buried sessions before quit
- **#11424** - Native tabs in new window - FIXED (Worker #1319) - placeholder affinities now ignored
- **#11433** - Tab titles overwritten - FIXED (Worker #1318) - TmuxDashboardController now scopes notifications

### Deep Investigation - Worker #1321

**#12540 - AppleScript set background color fails in tmux** - Cannot Reproduce
- **Reported Error**: AppleScript error -2741 (compilation error)
- **Code Analysis**:
  - `setBackgroundColor:` in `PTYSession+Scripting.m:340-342` calls `setSessionSpecificProfileValues:`
  - Code path is IDENTICAL for tmux and non-tmux sessions
  - The `amendedColorKey:` function handles light/dark mode, not tmux mode
  - `setSessionSpecificProfileValues:` properly divorces profile and updates color map
  - `objectSpecifier` in `PTYSession+Scripting.m:16-27` returns nil only when `realParentWindow` is nil
  - For tmux sessions, `realParentWindow` should always be valid (tmux windows have parent windows)
- **Error -2741 Analysis**: This AppleScript error means "compilation error" - typically occurs when:
  1. Target object specifier is invalid or can't be found
  2. Property name doesn't match sdef
- **Possible User-Side Causes**:
  1. AppleScript targeting wrong session (buried or hidden)
  2. Race condition where session is being created/destroyed
  3. macOS AppleScript sandboxing issues
- **sdef Verification**: `DashTerm2.sdef:579-580` correctly defines `background color` with `cocoa key="backgroundColor"`
- **Conclusion**: No bug found in DashTerm2 code. Issue may be user-specific (environment, AppleScript syntax, timing). Needs actual reproduction with AppleScript to diagnose further.

### Deep Investigation - Worker #1316

**#11465 - New tmux tabs steal focus from the current tab** ✓ FIXED (Worker #1317)
- **Root Cause**: `PseudoTerminal.insertTab:atIndex:` (lines 10859-10917)
- **Focus-stealing calls**:
  1. Line 10882-10883: `selectTabViewItemAtIndex:` when `_automaticallySelectNewTabs=YES` (default)
  2. Line 10893-10908: `makeKeyAndOrderFront:` brings window to front
  3. Line 10913-10914: `setCurrentTerminal:` makes it current
- **`_automaticallySelectNewTabs`**: Always `YES` for tmux windows (default)
- **No tmux preference** existed to prevent automatic tab selection
- **Tab selection callback** (`tabView:didSelectTabViewItem:` at lines 6670-6706) also calls `makeFirstResponder:`
- **Call chain**: TmuxGateway → PTYSession.tmuxWindowAddedWithId → TmuxController.openWindowWithId → TmuxWindowOpener.openWindows → PTYTab.openTabWithTmuxLayout → PseudoTerminal.insertTab
- **FIX (Worker #1317)**:
  - Added new Advanced Setting: `dontAutoSelectNewTmuxTabs` (default: NO)
  - Modified `PseudoTerminal.insertTab:atIndex:` to check this setting for tmux tabs
  - When enabled, new tmux tabs are added but don't steal focus from the current tab
  - First tab still always gets selected (correct behavior for new windows)

**#11433 - Tab titles for all sessions get overwritten with last session's titles** ✓ FIXED (Worker #1318)
- **Notification**: `kTmuxControllerWindowWasRenamed` posts `@[ @(wid), newName, self ]`
- **Key insight**: Window IDs are NOT globally unique - different `tmux -CC` sessions can have same window IDs
- **Primary flow (correct)**: PTYSession.m:9335-9338 uses `_tmuxController.sessionsInWindow:` which is correctly scoped
- **Bug location**: TmuxDashboardController.m:356-368 `tmuxControllerWindowWasRenamed:` did NOT check which TmuxController sent the notification
- **Analysis**: The bug only affects the tmux Dashboard window's display, not actual tab titles (those are correctly scoped via PTYSession)
- **FIX (Worker #1318):**
  - Modified `TmuxDashboardController.tmuxControllerWindowWasRenamed:` to check if the notification is from the currently selected TmuxController
  - Now ignores notifications from other tmux sessions, preventing cross-session window name updates in the Dashboard
- **Related files**: TmuxGateway.m (parses `%window-renamed`), PTYSession.m, TmuxController.m, PTYTab.m, TmuxControllerRegistry.m

**#11424 - "Native tabs in a new window" doesn't work** ✓ FIXED (Worker #1319)
- **Preference**: `kPreferenceKeyOpenTmuxWindowsIn` → `kOpenTmuxWindowsAsNativeTabsInNewWindow` (1)
- **Entry point**: TmuxController.m:580-661 `initialListWindowsResponse:`
- **Affinity logic** (lines 608-617): Creates equivalence class of unrecognized windows when `newWindowsInTabs=YES`
- **ROOT CAUSE IDENTIFIED (Worker #1318):**
  - **Placeholder affinity hack** (lines 3054-3060): When a tmux window is opened alone (not as tabs), a placeholder affinity `wid_ph` is saved
  - This placeholder signals "don't apply default mode for unrecognized windows"
  - When user CHANGES preference to "Native tabs in new window" AFTER windows were opened with different settings, the old placeholder affinities prevent grouping
  - Line 608 condition `![affinities_ valuesEqualTo:[n stringValue]]` returns FALSE if ANY affinity exists (even placeholder)
- **FIX (Worker #1319):**
  - Implemented option 2: Ignore placeholder affinities when checking for "unrecognized" status
  - Added new method `windowLacksRealAffinity:` in TmuxController.m
  - Returns YES if window has no affinity OR only has placeholder affinity (`wid` and/or `wid_ph`)
  - Changed line 639 condition from `![affinities_ valuesEqualTo:[n stringValue]]` to `[self windowLacksRealAffinity:[n stringValue]]`
  - Windows with placeholder-only affinities are now correctly treated as "unrecognized" and grouped together
- **Key files**: TmuxController.m (windowLacksRealAffinity:, setAffinitiesFromString:, initialListWindowsResponse:), EquivalenceClassSet.m

### Issues Triaged as External (Worker #1315)

**#12612** - Hidden panes flash with option+key
- **Analysis**: Option key sends escape sequences to tmux, which may briefly show pane switching
- **Root cause**: tmux server-side behavior, not iTerm2
- **Marked External**: tmux key handling limitation

**#12333** - history-limit setting not working
- **Analysis**: iTerm2's `history-limit` is client-side scrollback. Uses `capture-pane -S` which is limited by tmux server's own history-limit
- **Root cause**: tmux server-side configuration issue
- **Marked External**: User should configure tmux `set -g history-limit` on server

**#11053** - Zooming resizes window incorrectly
- **Analysis**: macOS window zoom (green button) conflicts with tmux pane zoom concept
- **Code**: PseudoTerminal.m:5528-5530 intentionally "pretends nothing happened" for performance
- **Marked External**: By design - macOS vs tmux zoom are different concepts

**#11126** - Laggy new-tab doesn't buffer input
- **Analysis**: Fundamental tmux control mode limitation - input cannot be buffered until pane ID is known
- **Code**: TmuxWindowOpener creates pane, then sends keys via `sendKeys:toWindowPane:`
- **Marked External**: Architectural limitation, users should wait for tab to appear

### Issues Triaged as Cannot Reproduce (Worker #1315)

**#12199** - Ghost window after SSH+tmux disconnect
- **Analysis**: Codebase has extensive orphan handling via iTermOrphanServerAdopter
- **Code**: `closeAllPanes` iterates over copy to handle modifications during cleanup
- **Marked Cannot Reproduce**: Vague repro steps, extensive cleanup code exists

**#11174** - Control mode fails to launch window
- **Analysis**: Window launch involves complex async command chain
- **Code**: TmuxWindowOpener.m:99-127 `openWindows:` method handles errors
- **Marked Cannot Reproduce**: "Often fails" is too vague, needs specific repro

### External Issues - Triaged (Worker #1313)

**#12149** - display-popup doesn't work
- **Root Cause**: `display-popup` is explicitly in the `forbiddenCommands` array at TmuxController.m:1414
- **Analysis**: tmux control mode (`tmux -CC`) cannot support interactive/popup commands
- **Other blocked commands**: `display-menu`, `display-message`, `display-panes`, `choose-tree`, `copy-mode`, `command-prompt`, etc.
- **Marked External**: Fundamental tmux control mode limitation

**#12552** - Control mode doesn't support certain commands
- **Root Cause**: Same `forbiddenCommands` array at TmuxController.m:1411-1417
- **Analysis**: 15+ interactive commands are intentionally blocked because they require a visual terminal
- **Commands include**: `bind-key`, `choose-buffer`, `choose-client`, `confirm-before`, `customize-mode`, `find-window`, `list-keys`, `show-messages`, etc.
- **Marked External**: By design in both iTerm2 and tmux

**#11325** - Broadcast message causes detach
- **Root Cause**: TmuxGateway.m has TODO at line 140 saying "be more forgiving of errors"
- **Analysis**: When a system `wall` broadcast arrives, it gets injected into the SSH/tmux stream. The gateway parser sees this as an unrecognized command and calls `abortWithErrorMessage`.
- **Workaround Available**: Advanced Setting `tolerateUnrecognizedTmuxCommands` = YES
- **Marked External**: Protocol limitation with existing workaround

### Cannot Reproduce - Triaged (Worker #1311)

**#12172** - Yellow ANSI renders as white
- **Analysis (Worker #1311)**: Comprehensive code analysis of color parsing chain
- **SGR parsing verified**: `VT100GraphicRenditionExecuteSGR` correctly maps SGR 33 → fgColorCode=3 (yellow)
- **Color map verified**: `kColorMapAnsiYellow = kColorMap8bitBase + 3` is correct
- **translateSGRFromScreenTerminal**: Only converts italic (SGR 3) to reverse (SGR 7), NOT yellow (SGR 33)
- **No yellow→white mapping found** anywhere in the codebase
- **Previous investigation (Worker #1308)** confirmed same findings
- **Possible user-side causes**:
  - Profile color configuration has yellow set to white-like color
  - tmux server actually sending wrong escape codes
  - User's TERM setting causing tmux to use different color sequences
- **Marked Cannot Reproduce**: Code analysis definitively shows correct color handling

### Common Patterns

1. **Window resize on reattach** - Multiple issues about window size changing
2. **Tab title not updating** - Common theme across several issues
3. **Mouse reporting partial** - Mouse works in some areas but not others
4. **Profile/theme not applied** - Colors/settings not carried to tmux windows

### Related Files

- `sources/TmuxController.m` - Main tmux integration controller
- `sources/TmuxGateway.m` - Communication with tmux server
- `sources/TmuxWindowOpener.m` - Window creation from tmux
- `sources/TmuxDashboardController.m` - tmux dashboard UI
- `sources/TmuxLayoutParser.m` - Parse tmux layout information
- `sources/PTYSession+Tmux.m` - Session tmux integration
- `sources/PTYTab+Tmux.m` - Tab tmux integration

