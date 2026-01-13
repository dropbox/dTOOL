# Complete Upstream DashTerm2 Issues Tracker

**Source:** https://gitlab.com/gnachman/iterm2/-/issues
**Total Open Issues:** 3348
**Generated:** December 20, 2025

---

## Category Summary

| Category | Count | Priority |
|----------|-------|----------|
| Crashes/Hangs | 289 | P0 - CRITICAL |
| AI Integration | 33 | P1 - HIGH |
| tmux Integration | 195 | P1 - HIGH |
| SSH/SFTP/SCP | 72 | P1 - HIGH |
| Shell Integration | 63 | P1 - HIGH |
| Performance | 173 | P2 - MEDIUM |
| Scrollback | 71 | P2 - MEDIUM |
| Font/Rendering | 189 | P2 - MEDIUM |
| Window/Tab/Pane | 667 | P2 - MEDIUM |
| Color/Theme | 125 | P3 - LOW |
| Browser | 71 | P3 - LOW |
| Keyboard/Input | 266 | P2 - MEDIUM |
| Profile/Settings | 142 | P3 - LOW |
| AppleScript/API | 73 | P3 - LOW |
| macOS Version | 31 | P2 - MEDIUM |
| Copy/Paste/Select | 87 | P2 - MEDIUM |
| Other | 801 | P3 - LOW |
| **TOTAL** | **3348** | |

---

## Crashes and Hangs (P0 - CRITICAL)

**Count:** 289

| Issue | Title |
|-------|-------|
| [#12625](https://gitlab.com/gnachman/iterm2/-/issues/12625) | DashTerm2 Beachballs At Start of Export |
| [#12558](https://gitlab.com/gnachman/iterm2/-/issues/12558) | DashTerm2 hangs on startup |
| [#12551](https://gitlab.com/gnachman/iterm2/-/issues/12551) | DashTerm2 shell integration does not work with Ubuntu 25.10 (Questing Quokka) due to coreutils changes |
| [#12527](https://gitlab.com/gnachman/iterm2/-/issues/12527) | macOS Terminal cursor does not change on hover over close (cross) button |
| [#12358](https://gitlab.com/gnachman/iterm2/-/issues/12358) | Latest nightly builds crash at launch because they link against wrong path for libhudsucker |
| [#12323](https://gitlab.com/gnachman/iterm2/-/issues/12323) | Audible bell can brick DashTerm2 |
| [#12322](https://gitlab.com/gnachman/iterm2/-/issues/12322) | Text selection highlight sometimes fails to disappear when underlying text changes |
| [#12174](https://gitlab.com/gnachman/iterm2/-/issues/12174) | "Undim" terminal when changing colors in the settings |
| [#12170](https://gitlab.com/gnachman/iterm2/-/issues/12170) | Logitech Wireless keyboard language changer opening DashTerm2 |
| [#12158](https://gitlab.com/gnachman/iterm2/-/issues/12158) | Tmux crashes |
| [#11906](https://gitlab.com/gnachman/iterm2/-/issues/11906) | Option to ABORT upgrade |
| [#11877](https://gitlab.com/gnachman/iterm2/-/issues/11877) | Random crash in `__NSIndexSetEnumerate` on pressing "g" |
| [#11875](https://gitlab.com/gnachman/iterm2/-/issues/11875) | Build 3.5.5 started crashing after update |
| [#11867](https://gitlab.com/gnachman/iterm2/-/issues/11867) | iterm2 crashes when opening the app |
| [#11861](https://gitlab.com/gnachman/iterm2/-/issues/11861) | DashTerm2 3.5.4 consistent crash on startup on MacOS Sequoia 15.0 |
| [#11854](https://gitlab.com/gnachman/iterm2/-/issues/11854) | Using 1337;SetProfile= to change the font causes the terminal to misbehave in 3.5.5beta1 when "Adjus... |
| [#11827](https://gitlab.com/gnachman/iterm2/-/issues/11827) | Cannot change foreground color of Default profile |
| [#11776](https://gitlab.com/gnachman/iterm2/-/issues/11776) | Crash on emoji output by `pipx upgrade-all` |
| [#11764](https://gitlab.com/gnachman/iterm2/-/issues/11764) | Indefinite hang when trying to attach to tmux -CC |
| [#11747](https://gitlab.com/gnachman/iterm2/-/issues/11747) | Constant crashes when switching focus to new app |
| [#11679](https://gitlab.com/gnachman/iterm2/-/issues/11679) | Opening a new window in an existing terminal sometimes keeps crashing |
| [#11661](https://gitlab.com/gnachman/iterm2/-/issues/11661) | Constant crashing |
| [#11653](https://gitlab.com/gnachman/iterm2/-/issues/11653) | feature request: Revert change to add POSIX/locale popups |
| [#11647](https://gitlab.com/gnachman/iterm2/-/issues/11647) | Search changing focus at first match |
| [#11629](https://gitlab.com/gnachman/iterm2/-/issues/11629) | Open a new tab with "tmux" profile changes width of embedding window although "Terminal window resiz... |
| [#11625](https://gitlab.com/gnachman/iterm2/-/issues/11625) | Crash on start |
| [#11575](https://gitlab.com/gnachman/iterm2/-/issues/11575) | Custom tab title can not be changed |
| [#11572](https://gitlab.com/gnachman/iterm2/-/issues/11572) | Reset Zoom Zero Changes Font |
| [#11485](https://gitlab.com/gnachman/iterm2/-/issues/11485) | Excessive CPU usage, beachball, losing keyboard data |
| [#11448](https://gitlab.com/gnachman/iterm2/-/issues/11448) | [Question] What is the recommended way of versioning up changes in settings |
| [#11445](https://gitlab.com/gnachman/iterm2/-/issues/11445) | Hang with every use of Help menu -> Search |
| [#11441](https://gitlab.com/gnachman/iterm2/-/issues/11441) | change pointer style / image (make arrow style available) |
| [#11431](https://gitlab.com/gnachman/iterm2/-/issues/11431) | Background color change when moving between monitors. |
| [#11376](https://gitlab.com/gnachman/iterm2/-/issues/11376) | iterm2 freezes when pasting text of approx. >1000 lines |
| [#11347](https://gitlab.com/gnachman/iterm2/-/issues/11347) | DashTerm2 randomly crashes when entering alternate screen mode |
| [#11314](https://gitlab.com/gnachman/iterm2/-/issues/11314) | Using fish 3.7.0 displays mark indicators when changing directory due to OSC 7 |
| [#11286](https://gitlab.com/gnachman/iterm2/-/issues/11286) | Profile name is not updated after profile is changed |
| [#11241](https://gitlab.com/gnachman/iterm2/-/issues/11241) | Unable to change amount of space inserted by the TAB key |
| [#11203](https://gitlab.com/gnachman/iterm2/-/issues/11203) | Keyboard input freezes after selecting Text |
| [#11176](https://gitlab.com/gnachman/iterm2/-/issues/11176) | WindowServer crash |
| [#11156](https://gitlab.com/gnachman/iterm2/-/issues/11156) | UI appears to hang |
| [#11147](https://gitlab.com/gnachman/iterm2/-/issues/11147) | iTerm crashes while using vim and remotely ssh'd into Arch Linux |
| [#11135](https://gitlab.com/gnachman/iterm2/-/issues/11135) | getting frequent spinning beachballs with recent nightlies (Sonoma 14.0) |
| [#11132](https://gitlab.com/gnachman/iterm2/-/issues/11132) | 3.50.b12 startup crashes on Sonoma |
| [#11113](https://gitlab.com/gnachman/iterm2/-/issues/11113) | Attempts to resize window by pulling on corners or borders causes immediate crash |
| [#11110](https://gitlab.com/gnachman/iterm2/-/issues/11110) | Text artifacts when trying to change previous command |
| [#11076](https://gitlab.com/gnachman/iterm2/-/issues/11076) | Regular freeze on MacOS |
| [#11071](https://gitlab.com/gnachman/iterm2/-/issues/11071) | Not able to open DashTerm2 app, It keeps crashing on MacOS. |
| [#11069](https://gitlab.com/gnachman/iterm2/-/issues/11069) | Feature request: Option to clear and close search on focus change |
| [#11065](https://gitlab.com/gnachman/iterm2/-/issues/11065) | Crash since beta 11 update |
| [#11056](https://gitlab.com/gnachman/iterm2/-/issues/11056) | Iterm2 hangs after update |
| [#11030](https://gitlab.com/gnachman/iterm2/-/issues/11030) | Application Crashes After fresh install |
| [#11010](https://gitlab.com/gnachman/iterm2/-/issues/11010) | Change Profile with macOS theme? |
| [#10973](https://gitlab.com/gnachman/iterm2/-/issues/10973) | When use dropdown mode, keyboard layout does not change |
| [#10971](https://gitlab.com/gnachman/iterm2/-/issues/10971) | enhancement request: change color on title popup |
| [#10921](https://gitlab.com/gnachman/iterm2/-/issues/10921) | beach balled resizing |
| [#10893](https://gitlab.com/gnachman/iterm2/-/issues/10893) | Iterm2 UI hangs |
| [#10854](https://gitlab.com/gnachman/iterm2/-/issues/10854) | iterm2 3.5.20230308-nightly crashing |
| [#10846](https://gitlab.com/gnachman/iterm2/-/issues/10846) | [Crash bug] iTerm enters an infinite crash loop if prompted for secure keyboard input |
| [#10829](https://gitlab.com/gnachman/iterm2/-/issues/10829) | Window does not change focus when opening a file in an external app |
| [#10813](https://gitlab.com/gnachman/iterm2/-/issues/10813) | DashTerm2 3.5.0beta10 crashes on "sudo" |
| [#10789](https://gitlab.com/gnachman/iterm2/-/issues/10789) | Crash: When clicking on Git status bar after beginning tab switch swipe gesture |
| [#10763](https://gitlab.com/gnachman/iterm2/-/issues/10763) | iTerm 3.5.0b8 crashed twice recently |
| [#10754](https://gitlab.com/gnachman/iterm2/-/issues/10754) | Crash under Rosetta after Updateing Version |
| [#10730](https://gitlab.com/gnachman/iterm2/-/issues/10730) | title get stuck despite 'set title' trigger |
| [#10722](https://gitlab.com/gnachman/iterm2/-/issues/10722) | Open Python REPL - results in a crash in the window about 1 second after opening |
| [#10715](https://gitlab.com/gnachman/iterm2/-/issues/10715) | Change Alert on Next Mark sound for non-zero status codes. |
| [#10693](https://gitlab.com/gnachman/iterm2/-/issues/10693) | iTerm crashes on a newly installed macOS 10.14.6 |
| [#10669](https://gitlab.com/gnachman/iterm2/-/issues/10669) | focus-follows-mouse should not change focus until mouse stops |
| [#10666](https://gitlab.com/gnachman/iterm2/-/issues/10666) | Crash while tons of output scrolling by |
| [#10595](https://gitlab.com/gnachman/iterm2/-/issues/10595) | App Crashes on Launch After In-App Update |
| [#10593](https://gitlab.com/gnachman/iterm2/-/issues/10593) | Retain session size during screen resolution change |
| [#10583](https://gitlab.com/gnachman/iterm2/-/issues/10583) | DashTerm2 big crash after upgrading macOS to 12.6 along with Xcode and Xcode dev tools. |
| [#10580](https://gitlab.com/gnachman/iterm2/-/issues/10580) | iTerm 3.5.0beta7 crashes on startup on OS X 10.14.6 |
| [#10569](https://gitlab.com/gnachman/iterm2/-/issues/10569) | Focus on iTerm window changes layout |
| [#10540](https://gitlab.com/gnachman/iterm2/-/issues/10540) | scrollback in session window hangs after terminal session is restarted. |
| [#10477](https://gitlab.com/gnachman/iterm2/-/issues/10477) | Crash on closing DashTerm2 |
| [#10430](https://gitlab.com/gnachman/iterm2/-/issues/10430) | Iterm2 crashes after changing default font |
| [#10425](https://gitlab.com/gnachman/iterm2/-/issues/10425) | DashTerm2 crashes when minimized using hotkey after pressing up to select previous command |
| [#10391](https://gitlab.com/gnachman/iterm2/-/issues/10391) | DashTerm2 hangs |
| [#10379](https://gitlab.com/gnachman/iterm2/-/issues/10379) | Add context menu options to send signal and change stty settings |
| [#10365](https://gitlab.com/gnachman/iterm2/-/issues/10365) | Crash while opening right after macOS boot |
| [#10363](https://gitlab.com/gnachman/iterm2/-/issues/10363) | Crash 3.5.0b5 by pasting emoji/unicode in vim |
| [#10347](https://gitlab.com/gnachman/iterm2/-/issues/10347) | crash inputting interpolated tab title |
| [#10330](https://gitlab.com/gnachman/iterm2/-/issues/10330) | Auto theme changing |
| [#10310](https://gitlab.com/gnachman/iterm2/-/issues/10310) | Allow Smart Selection action "Send Text..." have a modifier (Option key) to change it to "Copy Text ... |
| [#10304](https://gitlab.com/gnachman/iterm2/-/issues/10304) | DashTerm2 freezes and does not accept input |
| [#10279](https://gitlab.com/gnachman/iterm2/-/issues/10279) | UI Freeze after sleep |
| [#10259](https://gitlab.com/gnachman/iterm2/-/issues/10259) | Copy crash |
| [#10258](https://gitlab.com/gnachman/iterm2/-/issues/10258) | DashTerm2 UI hangs |
| [#10247](https://gitlab.com/gnachman/iterm2/-/issues/10247) | DashTerm2 (partly) crashes on modifyOtherKeys \033[>4;1m |
| [#10163](https://gitlab.com/gnachman/iterm2/-/issues/10163) | Focus changes the find dialog's content |
| [#10144](https://gitlab.com/gnachman/iterm2/-/issues/10144) | Change focus to pasted window on middle-button paste |
| [#10125](https://gitlab.com/gnachman/iterm2/-/issues/10125) | App freezes on horizontal scroll with multiple tabs (Logi MX Master) |
| [#10120](https://gitlab.com/gnachman/iterm2/-/issues/10120) | Font alignment changes when external display disconnected |
| [#10093](https://gitlab.com/gnachman/iterm2/-/issues/10093) | Add support for changing the color of the maximized split icon |
| [#10085](https://gitlab.com/gnachman/iterm2/-/issues/10085) | `tmux -CC` session crashing when closing an iTerm tab/tmux window. |
| [#10081](https://gitlab.com/gnachman/iterm2/-/issues/10081) | Every click has a 5 to 25 second beach ball |
| [#10075](https://gitlab.com/gnachman/iterm2/-/issues/10075) | hotkey window keeps changing its size after external monitor wakes up |
| [#10059](https://gitlab.com/gnachman/iterm2/-/issues/10059) | DashTerm2 hangs for 20+ minutes, marked as unresponsive in Activity Monitor |
| [#9924](https://gitlab.com/gnachman/iterm2/-/issues/9924) | tmux tab title do not change when to run tmux renamew "TEST" |
| [#9861](https://gitlab.com/gnachman/iterm2/-/issues/9861) | Crash in prefs UI |
| [#9837](https://gitlab.com/gnachman/iterm2/-/issues/9837) | Screen change when opening/closing a tab |
| [#9822](https://gitlab.com/gnachman/iterm2/-/issues/9822) | Allow profile change for a tab to apply to all tabs |
| [#9794](https://gitlab.com/gnachman/iterm2/-/issues/9794) | Ability to change the modifier keys used for rectangular selection |
| [#9789](https://gitlab.com/gnachman/iterm2/-/issues/9789) | Crash while using MX master 3 |
| [#9768](https://gitlab.com/gnachman/iterm2/-/issues/9768) | opening tmux sessions in tabs has changed |
| [#9752](https://gitlab.com/gnachman/iterm2/-/issues/9752) | Change font size in all tabs attached to a iterm2 tmux session |
| [#9745](https://gitlab.com/gnachman/iterm2/-/issues/9745) | Iterm tab changing when using mouse gestures to change desktops |
| [#9659](https://gitlab.com/gnachman/iterm2/-/issues/9659) | iTerm crashed |
| [#9638](https://gitlab.com/gnachman/iterm2/-/issues/9638) | bash crash originating from iTerm in Console |
| [#9603](https://gitlab.com/gnachman/iterm2/-/issues/9603) | iterm2 hangs for about 30 seconds |
| [#9592](https://gitlab.com/gnachman/iterm2/-/issues/9592) | SSH output changes when enter-exiting edit mode |
| [#9578](https://gitlab.com/gnachman/iterm2/-/issues/9578) | app seems to randomly freeze |
| [#9555](https://gitlab.com/gnachman/iterm2/-/issues/9555) | DashTerm2 immediately crashes on MacBookPro10,1 (Retine, Middle 2012) |
| [#9545](https://gitlab.com/gnachman/iterm2/-/issues/9545) | iterm2 keep crashing on start |
| [#9516](https://gitlab.com/gnachman/iterm2/-/issues/9516) | Crashes on launch with Macbook Air M1 |
| [#9514](https://gitlab.com/gnachman/iterm2/-/issues/9514) | Changing font size causes terminal to flag default font size |
| [#9469](https://gitlab.com/gnachman/iterm2/-/issues/9469) | [feature request] key binds to change or temporarily toggle opacity. |
| [#9418](https://gitlab.com/gnachman/iterm2/-/issues/9418) | tmux stuck in "Command Menu" |
| [#9337](https://gitlab.com/gnachman/iterm2/-/issues/9337) | Beachball/hang doing SSH when getting prompt to continue on 3.4.2  makes product unusable. |
| [#9326](https://gitlab.com/gnachman/iterm2/-/issues/9326) | iTerm crashes when executes command from trigger section |
| [#9306](https://gitlab.com/gnachman/iterm2/-/issues/9306) | iTerm freezes accepting no input |
| [#9215](https://gitlab.com/gnachman/iterm2/-/issues/9215) | Certain sequences will make DashTerm2 get stuck |
| [#9145](https://gitlab.com/gnachman/iterm2/-/issues/9145) | Random hangs for no good reason. it's back :( |
| [#9125](https://gitlab.com/gnachman/iterm2/-/issues/9125) | DashTerm2 crashes after disconnecting from one host and connecting to another in a tmux window |
| [#9117](https://gitlab.com/gnachman/iterm2/-/issues/9117) | After upgrading to 3.4.0beta5 from3.4.0beta4 session restoration is stuck |
| [#9108](https://gitlab.com/gnachman/iterm2/-/issues/9108) | Window size keeps changing w/tmux and Moom |
| [#9086](https://gitlab.com/gnachman/iterm2/-/issues/9086) | Frequently crashes immediately on launch, but then ok on re-launch. |
| [#8991](https://gitlab.com/gnachman/iterm2/-/issues/8991) | DashTerm2 3.3.11 randomly crashes on my mac |
| [#8888](https://gitlab.com/gnachman/iterm2/-/issues/8888) | panic(cpu 0 caller 0xffffff8007a91b2c): Sleep transition timed out after 180 seconds while calling p... |
| [#8827](https://gitlab.com/gnachman/iterm2/-/issues/8827) | Change cursor to normal when text is unselectable |
| [#8823](https://gitlab.com/gnachman/iterm2/-/issues/8823) | cannot change tmux status bar font |
| [#8752](https://gitlab.com/gnachman/iterm2/-/issues/8752) | Upgrading from 3.3.10beta1 to 3.3.10beta2 changed option key mapping in default profile |
| [#8695](https://gitlab.com/gnachman/iterm2/-/issues/8695) | DashTerm2 crashes whenever I try to split pane or create a new tab |
| [#8675](https://gitlab.com/gnachman/iterm2/-/issues/8675) | DashTerm2 3.3.8 crashes on startup |
| [#8670](https://gitlab.com/gnachman/iterm2/-/issues/8670) | First Session Ok, New Sessions Crash ( Invalid Code Signature ) |
| [#8657](https://gitlab.com/gnachman/iterm2/-/issues/8657) | How to change key binding to paste history? |
| [#8631](https://gitlab.com/gnachman/iterm2/-/issues/8631) | Crashing when opening an imported profile |
| [#8610](https://gitlab.com/gnachman/iterm2/-/issues/8610) | Vim cannot change DashTerm2's title when connected to tmux via ssh title |
| [#8607](https://gitlab.com/gnachman/iterm2/-/issues/8607) | DashTerm2 build 3.3.7 crashing randomly |
| [#8604](https://gitlab.com/gnachman/iterm2/-/issues/8604) | The Sparkle update window doesn't render parts of the changelog |
| [#8600](https://gitlab.com/gnachman/iterm2/-/issues/8600) | Sending Swap-Pane Crashes TMUX Session |
| [#8522](https://gitlab.com/gnachman/iterm2/-/issues/8522) | Changing a window's tab color from context menu doesn't work for single-tab windows |
| [#8521](https://gitlab.com/gnachman/iterm2/-/issues/8521) | Change background of window's label? |
| [#8507](https://gitlab.com/gnachman/iterm2/-/issues/8507) | red close button shows a dot ('unsaved changes') |
| [#8504](https://gitlab.com/gnachman/iterm2/-/issues/8504) | iterm2 crashes indefinitely after hitting CTRL-A |
| [#8482](https://gitlab.com/gnachman/iterm2/-/issues/8482) | DashTerm2 Build 3.3.6 and Build 3.3.7beta4 crashing when opening preferences on MacOS Mojave 10.14.6 |
| [#8464](https://gitlab.com/gnachman/iterm2/-/issues/8464) | How to change kill tmux window behavior after checking "Remember my choice" |
| [#8366](https://gitlab.com/gnachman/iterm2/-/issues/8366) | New Window or New Tab,  Iterm window disappears (crashes ?) |
| [#8353](https://gitlab.com/gnachman/iterm2/-/issues/8353) | Use MacOS System Preference Modifier Keys exchange CMD and option, the option key doesn't work in iT... |
| [#8345](https://gitlab.com/gnachman/iterm2/-/issues/8345) | Latest update 3.3.5 crashes on startup |
| [#8215](https://gitlab.com/gnachman/iterm2/-/issues/8215) | [Feature Request] Change Pane Title Bar color (active and inactive seperately) |
| [#8184](https://gitlab.com/gnachman/iterm2/-/issues/8184) | 70-second freeze on tmux attach/detach |
| [#8176](https://gitlab.com/gnachman/iterm2/-/issues/8176) | app freeze |
| [#8172](https://gitlab.com/gnachman/iterm2/-/issues/8172) | Creating new `Pane` when in VIM editing mode(cursor shape is a vertical bar) will change the new pan... |
| [#8171](https://gitlab.com/gnachman/iterm2/-/issues/8171) | if changed option key in preference panel, dose not work for Esc+ key. |
| [#8169](https://gitlab.com/gnachman/iterm2/-/issues/8169) | Status bar component crash on start up -- no function registered for invocation “coro()”. |
| [#8116](https://gitlab.com/gnachman/iterm2/-/issues/8116) | iTerm  3.3.1 sometimes hangs when hidden and selected from Dock again |
| [#8039](https://gitlab.com/gnachman/iterm2/-/issues/8039) | Enabling Hotkey window "Animate showing and hiding" appears to cause crash on `tmux -CC` |
| [#7974](https://gitlab.com/gnachman/iterm2/-/issues/7974) | "Show Timestamps": older timestamps change on keyboard/mouse input |
| [#7970](https://gitlab.com/gnachman/iterm2/-/issues/7970) | variable window sizes in tmux integration not working - both windows change size |
| [#7960](https://gitlab.com/gnachman/iterm2/-/issues/7960) | Scrolling under linux screen command: macbook trackpad behaviour changes |
| [#7917](https://gitlab.com/gnachman/iterm2/-/issues/7917) | Is it possible to change the Background Pattern Indicator colour? |
| [#7874](https://gitlab.com/gnachman/iterm2/-/issues/7874) | Panes do not resize proportionally after screen resolution change |
| [#7810](https://gitlab.com/gnachman/iterm2/-/issues/7810) | Proxy icon appears to be causing iterm2 to hang |
| [#7807](https://gitlab.com/gnachman/iterm2/-/issues/7807) | Hang after open command |
| [#7768](https://gitlab.com/gnachman/iterm2/-/issues/7768) | iTerm crashes when I try to git add a single file |
| [#7746](https://gitlab.com/gnachman/iterm2/-/issues/7746) | Frequent crashes with iTerm 3.2.8 and 3.2.9 |
| [#7732](https://gitlab.com/gnachman/iterm2/-/issues/7732) | Changing keymaps : hotkey window shortcut changes |
| [#7693](https://gitlab.com/gnachman/iterm2/-/issues/7693) | After macos crash, DashTerm2 sessions did not restore properly and many bins behaved weirdly |
| [#7595](https://gitlab.com/gnachman/iterm2/-/issues/7595) | Ctrl+A in iTerm is now selecting all text in the window instead of bringing cursor to the first char... |
| [#7575](https://gitlab.com/gnachman/iterm2/-/issues/7575) | proprietary escape codes for changing tab titles no longer seem to work |
| [#7512](https://gitlab.com/gnachman/iterm2/-/issues/7512) | DashTerm2 hanging several minutes, marked as unresponsive in Activity Monitor |
| [#7504](https://gitlab.com/gnachman/iterm2/-/issues/7504) | Iterm2 terminal crashes after opening |
| [#7451](https://gitlab.com/gnachman/iterm2/-/issues/7451) | Beach ball when resizing window or pane |
| [#7267](https://gitlab.com/gnachman/iterm2/-/issues/7267) | iTerm crashing on sleep |
| [#7263](https://gitlab.com/gnachman/iterm2/-/issues/7263) | Cannot change the shell of a restored session |
| [#7077](https://gitlab.com/gnachman/iterm2/-/issues/7077) | tab colour cant change at all. |
| [#7024](https://gitlab.com/gnachman/iterm2/-/issues/7024) | Menu order has changed since upgrade to 2.1.4 |
| [#6984](https://gitlab.com/gnachman/iterm2/-/issues/6984) | lock and unlock screen hides opened hotkey window and changes main window to iterm2 |
| [#6940](https://gitlab.com/gnachman/iterm2/-/issues/6940) | DashTerm2 fatal crash debug log |
| [#6900](https://gitlab.com/gnachman/iterm2/-/issues/6900) | Show beach ball while changing font style, and can not be launch after force close |
| [#6893](https://gitlab.com/gnachman/iterm2/-/issues/6893) | Feature request: activity indicator to sense changes volume |
| [#6892](https://gitlab.com/gnachman/iterm2/-/issues/6892) | Crash during window switching |
| [#6876](https://gitlab.com/gnachman/iterm2/-/issues/6876) | Badge color should not be changed when changing color palette(?) |
| [#6858](https://gitlab.com/gnachman/iterm2/-/issues/6858) | Closing tab causes iTerm to crash |
| [#6851](https://gitlab.com/gnachman/iterm2/-/issues/6851) | using Mutt, line drawing characters change when DashTerm2 window is idle |
| [#6815](https://gitlab.com/gnachman/iterm2/-/issues/6815) | iTerm crashing - 3.1.6 |
| [#6783](https://gitlab.com/gnachman/iterm2/-/issues/6783) | Option to change back to old Dock icon? |
| [#6750](https://gitlab.com/gnachman/iterm2/-/issues/6750) | Crashes on Cmd_I following tab clone |
| [#6717](https://gitlab.com/gnachman/iterm2/-/issues/6717) | Feature Request - Change path of hotkey window to currently active application path |
| [#6670](https://gitlab.com/gnachman/iterm2/-/issues/6670) | Stuck on empty password manager - needs force close |
| [#6655](https://gitlab.com/gnachman/iterm2/-/issues/6655) | iterm2 tmux integration crashes when merging tabs and save window arrangement |
| [#6628](https://gitlab.com/gnachman/iterm2/-/issues/6628) | DashTerm2 Beachball Issues, Possibly Due to DB |
| [#6612](https://gitlab.com/gnachman/iterm2/-/issues/6612) | crash in PTYSession.m [PTYSession drawFrameAndRemoveTemporarilyDisablementOfMetal] |
| [#6581](https://gitlab.com/gnachman/iterm2/-/issues/6581) | Change default option-arrow to match Terminal |
| [#6579](https://gitlab.com/gnachman/iterm2/-/issues/6579) | Nightly Build Crashing Randomly... |
| [#6573](https://gitlab.com/gnachman/iterm2/-/issues/6573) | remote terminal closes -> pane freezes |
| [#6528](https://gitlab.com/gnachman/iterm2/-/issues/6528) | Title changes on panes |
| [#6509](https://gitlab.com/gnachman/iterm2/-/issues/6509) | DashTerm2 hangs constantly |
| [#6491](https://gitlab.com/gnachman/iterm2/-/issues/6491) | Metal renderer causes my iterm2 to crash repeatedly |
| [#6458](https://gitlab.com/gnachman/iterm2/-/issues/6458) | ssh to a local device prompt ssh_exchange_identification: Connection closed by remote host |
| [#6430](https://gitlab.com/gnachman/iterm2/-/issues/6430) | Latest Nightly Hangs after waking up from Sleep. |
| [#6399](https://gitlab.com/gnachman/iterm2/-/issues/6399) | Changed font that was a duplicate in my system - DashTerm2 froze and will not restart; Key: no document... |
| [#6287](https://gitlab.com/gnachman/iterm2/-/issues/6287) | Theme sporadically changes from dark to light |
| [#6242](https://gitlab.com/gnachman/iterm2/-/issues/6242) | DashTerm2 Hangs the Network Connection Every 5 Minutes |
| [#6233](https://gitlab.com/gnachman/iterm2/-/issues/6233) | Crash when htop left running |
| [#6215](https://gitlab.com/gnachman/iterm2/-/issues/6215) | Changing keyboard layout to Romaji when using the Kotoeri layouts inserts an extra semicolon into th... |
| [#6212](https://gitlab.com/gnachman/iterm2/-/issues/6212) | How to find out current geometry w/o changing it first? |
| [#6110](https://gitlab.com/gnachman/iterm2/-/issues/6110) | Beachball / hang in iTerm 3.1.2 (make calls to proc_pidinfo async) |
| [#6028](https://gitlab.com/gnachman/iterm2/-/issues/6028) | DashTerm2 radically changes the background colour set in Emacs |
| [#6013](https://gitlab.com/gnachman/iterm2/-/issues/6013) | Tab does not change color using escapes, only flash to color briefly |
| [#6005](https://gitlab.com/gnachman/iterm2/-/issues/6005) | DashTerm2 beach balls, probably due to window resizing when disconnecting an external monitor |
| [#5845](https://gitlab.com/gnachman/iterm2/-/issues/5845) | Hang / beachball, persists after restart. |
| [#5817](https://gitlab.com/gnachman/iterm2/-/issues/5817) | DashTerm2 stuck on old color profile |
| [#5750](https://gitlab.com/gnachman/iterm2/-/issues/5750) | tmux: Attaching to a session with a large tab make other tabs to hang |
| [#5697](https://gitlab.com/gnachman/iterm2/-/issues/5697) | DashTerm2 freezes (beach ball of death) when double clicking very long (>500 000 characters) text that ... |
| [#5661](https://gitlab.com/gnachman/iterm2/-/issues/5661) | Change font size bug |
| [#5645](https://gitlab.com/gnachman/iterm2/-/issues/5645) | [Feature] Change profile without changing text size. |
| [#5623](https://gitlab.com/gnachman/iterm2/-/issues/5623) | Beach ball / application not responding with large saved application state |
| [#5535](https://gitlab.com/gnachman/iterm2/-/issues/5535) | freeze memory peak exec c script socket server through ssh on vbox debian |
| [#5532](https://gitlab.com/gnachman/iterm2/-/issues/5532) | Indefinite Beachball ~5-6 times a week - Possibly Git/Other? |
| [#5512](https://gitlab.com/gnachman/iterm2/-/issues/5512) | Crashing repeatedly on startup |
| [#5510](https://gitlab.com/gnachman/iterm2/-/issues/5510) | unlimited buffer should be cleared after crash or force quit |
| [#5477](https://gitlab.com/gnachman/iterm2/-/issues/5477) | Changing the the badge in one pane changes the badge in all panes |
| [#5458](https://gitlab.com/gnachman/iterm2/-/issues/5458) | Change text from: "Split Pane*" to "Split Up / Down / Left / Right" |
| [#5454](https://gitlab.com/gnachman/iterm2/-/issues/5454) | Pasting a path starting with / often beachballs for about 1 second |
| [#5452](https://gitlab.com/gnachman/iterm2/-/issues/5452) | Shell integration - Changing cursor color is not working |
| [#5433](https://gitlab.com/gnachman/iterm2/-/issues/5433) | SSH terminal windows / tabs freeze after a few minutes |
| [#5424](https://gitlab.com/gnachman/iterm2/-/issues/5424) | Allow changing window style after it's already been created |
| [#5374](https://gitlab.com/gnachman/iterm2/-/issues/5374) | Change Split shell/screen Line color |
| [#5355](https://gitlab.com/gnachman/iterm2/-/issues/5355) | Focus inconsistent when changing between spaces with iterm2 |
| [#5318](https://gitlab.com/gnachman/iterm2/-/issues/5318) | Allow title change to trigger API |
| [#5317](https://gitlab.com/gnachman/iterm2/-/issues/5317) | DashTerm2 v3 often hangs when Cmd-Tabbing back to it |
| [#5286](https://gitlab.com/gnachman/iterm2/-/issues/5286) | Simple way to enable notification after *any* terminal change |
| [#5197](https://gitlab.com/gnachman/iterm2/-/issues/5197) | Hang: Cannot call apple script from Semantic History |
| [#5193](https://gitlab.com/gnachman/iterm2/-/issues/5193) | [Question] How to dynamically change iterm2 settings or .plist file? |
| [#5189](https://gitlab.com/gnachman/iterm2/-/issues/5189) | Script Editor changes target application name on compile/save |
| [#5167](https://gitlab.com/gnachman/iterm2/-/issues/5167) | Pasting multiple lines changes the text in mysterious ways |
| [#5086](https://gitlab.com/gnachman/iterm2/-/issues/5086) | Autoupdate wants to make changes . . . |
| [#5054](https://gitlab.com/gnachman/iterm2/-/issues/5054) | Feature Request: Text Zooming Changes |
| [#5033](https://gitlab.com/gnachman/iterm2/-/issues/5033) | Color picker in Profile hangs iTerm (Build 3.0.5) |
| [#5029](https://gitlab.com/gnachman/iterm2/-/issues/5029) | The application beachballs for 10s very regularly. |
| [#5011](https://gitlab.com/gnachman/iterm2/-/issues/5011) | iterm2 hangs while disk speeds up; should only affect individual terminal |
| [#4966](https://gitlab.com/gnachman/iterm2/-/issues/4966) | I term crashes on launch |
| [#4835](https://gitlab.com/gnachman/iterm2/-/issues/4835) | Ambiguous UI: "Discard Local Changes" when clicking "Save Current Settings to Folder" |
| [#4829](https://gitlab.com/gnachman/iterm2/-/issues/4829) | Iterm crashes whenever I try to start it. |
| [#4795](https://gitlab.com/gnachman/iterm2/-/issues/4795) | Command `ssh -vvv server` hangs with no output after session restore |
| [#4775](https://gitlab.com/gnachman/iterm2/-/issues/4775) | Tip of the Day prompt freezes everything |
| [#4605](https://gitlab.com/gnachman/iterm2/-/issues/4605) | DashTerm2 v3 beta crashes system |
| [#4452](https://gitlab.com/gnachman/iterm2/-/issues/4452) | [Feature] Ability to change Titlebar based on the Profile with Multiple Tabs |
| [#4420](https://gitlab.com/gnachman/iterm2/-/issues/4420) | Crash on pane switching with keyboard shortcut |
| [#4244](https://gitlab.com/gnachman/iterm2/-/issues/4244) | Once or twice: Apple-N (new window) freezes iterm |
| [#4239](https://gitlab.com/gnachman/iterm2/-/issues/4239) | Enhancement Request: Set Window label instead of it changing |
| [#4182](https://gitlab.com/gnachman/iterm2/-/issues/4182) | Window title text stuck on screen even after DashTerm2 closed |
| [#4172](https://gitlab.com/gnachman/iterm2/-/issues/4172) | When I change a session's profile via "Tab Prefs":General:"Change Session's Profile", the "Tab Prefs... |
| [#4081](https://gitlab.com/gnachman/iterm2/-/issues/4081) | OS X El Capitan split screen resize causes crash. Would be a |
| [#4053](https://gitlab.com/gnachman/iterm2/-/issues/4053) | DashTerm2 hangs for minutes while resizing the window. |
| [#4030](https://gitlab.com/gnachman/iterm2/-/issues/4030) | Cursor doesn't change along with line height |
| [#4009](https://gitlab.com/gnachman/iterm2/-/issues/4009) | Getting freezed when press "y" key on a single tab |
| [#3990](https://gitlab.com/gnachman/iterm2/-/issues/3990) | Quitting iTerm hangs OS X |
| [#3955](https://gitlab.com/gnachman/iterm2/-/issues/3955) | Show changes since installed version when upgrading |
| [#3951](https://gitlab.com/gnachman/iterm2/-/issues/3951) | Process Hang after Quit |
| [#3842](https://gitlab.com/gnachman/iterm2/-/issues/3842) | Allow users to define a title for the window that shell escapes can't change |
| [#3809](https://gitlab.com/gnachman/iterm2/-/issues/3809) | Changing profiles is cumbersome and dangerous |
| [#3787](https://gitlab.com/gnachman/iterm2/-/issues/3787) | El Capitan crash |
| [#3779](https://gitlab.com/gnachman/iterm2/-/issues/3779) | Disable color change, Selection Text/Cursor Text |
| [#3757](https://gitlab.com/gnachman/iterm2/-/issues/3757) | feature req for cmd-f:  select text till end of line and flexible change focus when highlighting |
| [#3716](https://gitlab.com/gnachman/iterm2/-/issues/3716) | Changing Tab Colour Sometimes Causes Tab to Crash |
| [#3709](https://gitlab.com/gnachman/iterm2/-/issues/3709) | cannot change highlight color for trigger |
| [#3707](https://gitlab.com/gnachman/iterm2/-/issues/3707) | Stuck hint |
| [#3679](https://gitlab.com/gnachman/iterm2/-/issues/3679) |  change directory on mouseclick |
| [#3645](https://gitlab.com/gnachman/iterm2/-/issues/3645) | Control-shift-- "hangs" DashTerm2 |
| [#3636](https://gitlab.com/gnachman/iterm2/-/issues/3636) | Changes to shell integration files |
| [#3632](https://gitlab.com/gnachman/iterm2/-/issues/3632) | Change of profile does not apply to session log |
| [#3468](https://gitlab.com/gnachman/iterm2/-/issues/3468) | Speed up accessibility's _allText method [was: constant spin wheel hanging iterm] |
| [#3346](https://gitlab.com/gnachman/iterm2/-/issues/3346) | DashTerm2 window placement change on user swap and return |
| [#3203](https://gitlab.com/gnachman/iterm2/-/issues/3203) | Do not change window height when displaying tabs |
| [#3060](https://gitlab.com/gnachman/iterm2/-/issues/3060) | Massive change of profiles |
| [#3049](https://gitlab.com/gnachman/iterm2/-/issues/3049) | Add option to change cursor style based on idle timeout |
| [#3042](https://gitlab.com/gnachman/iterm2/-/issues/3042) | freeze when item from menubar is selected |
| [#3002](https://gitlab.com/gnachman/iterm2/-/issues/3002) | keyboard shortcut to change tab profile |
| [#2980](https://gitlab.com/gnachman/iterm2/-/issues/2980) | Shortcut to change profile of existing window |
| [#2829](https://gitlab.com/gnachman/iterm2/-/issues/2829) | Add a pointer action to change transparency |
| [#2229](https://gitlab.com/gnachman/iterm2/-/issues/2229) | After logout/in window width has changed. |
| [#1722](https://gitlab.com/gnachman/iterm2/-/issues/1722) | Crash when exiting fullscreen with (maximized?) tmux pane |
| [#1592](https://gitlab.com/gnachman/iterm2/-/issues/1592) | Launching DashTerm2 randomly crashes whole OS X GUI |
| [#1353](https://gitlab.com/gnachman/iterm2/-/issues/1353) | Killall iTerm lauch crash reporter |

---

## AI Integration (P1)

**Count:** 33

| Issue | Title |
|-------|-------|
| [#12654](https://gitlab.com/gnachman/iterm2/-/issues/12654) | Running iterm2_shell_integration.zsh breaks AI agents |
| [#12640](https://gitlab.com/gnachman/iterm2/-/issues/12640) | Ability to configure AI settings locally or per-machine |
| [#12595](https://gitlab.com/gnachman/iterm2/-/issues/12595) | Lost ability to "fork/clone/delete" a chat message in AI Chat |
| [#12539](https://gitlab.com/gnachman/iterm2/-/issues/12539) | AI Agent Not Loading |
| [#12521](https://gitlab.com/gnachman/iterm2/-/issues/12521) | Support Zero Data Retention orgs in AI Chat |
| [#12506](https://gitlab.com/gnachman/iterm2/-/issues/12506) | AI Chats cannot see data from restored sessions |
| [#12479](https://gitlab.com/gnachman/iterm2/-/issues/12479) | rendering issue in openai codex |
| [#12451](https://gitlab.com/gnachman/iterm2/-/issues/12451) | AI Chat says "Plugin not found" even if plugin installed |
| [#12387](https://gitlab.com/gnachman/iterm2/-/issues/12387) | Support AWS Bedrock for AI assistant |
| [#12380](https://gitlab.com/gnachman/iterm2/-/issues/12380) | AI API responds "Unsupported parameter: 'max_tokens'" |
| [#12330](https://gitlab.com/gnachman/iterm2/-/issues/12330) | Customize the AI LLM deepseek |
| [#12292](https://gitlab.com/gnachman/iterm2/-/issues/12292) | Custom AI generation command |
| [#12260](https://gitlab.com/gnachman/iterm2/-/issues/12260) | DeepSeek AI integration |
| [#12237](https://gitlab.com/gnachman/iterm2/-/issues/12237) | Support for Microsoft Copilot AI |
| [#12182](https://gitlab.com/gnachman/iterm2/-/issues/12182) | Error from OpenAI api: Missing required parameter: 'messages' |
| [#11900](https://gitlab.com/gnachman/iterm2/-/issues/11900) | Option to use Google Gemini API for AI completions instead of OpenAI |
| [#11869](https://gitlab.com/gnachman/iterm2/-/issues/11869) | AI configuration screen should contemplate other solutions |
| [#11856](https://gitlab.com/gnachman/iterm2/-/issues/11856) | add ai models: o1-preview & o1-mini |
| [#11808](https://gitlab.com/gnachman/iterm2/-/issues/11808) | Configurable OpenAI API options aren't accounted for on the AI tab of the Settings |
| [#11800](https://gitlab.com/gnachman/iterm2/-/issues/11800) | control-k preceding the command line when gen AI creates the command |
| [#11677](https://gitlab.com/gnachman/iterm2/-/issues/11677) | Hide "Engage Artificial Inteligence" from the menu when AI is disabled |
| [#11612](https://gitlab.com/gnachman/iterm2/-/issues/11612) | openAI key not working |
| [#11582](https://gitlab.com/gnachman/iterm2/-/issues/11582) | AI token counter seems to work in reverse? |
| [#11561](https://gitlab.com/gnachman/iterm2/-/issues/11561) | Allow OpenAI-compatible server |
| [#11541](https://gitlab.com/gnachman/iterm2/-/issues/11541) | AI completion using a local LLM is prepending the name of the shell |
| [#11535](https://gitlab.com/gnachman/iterm2/-/issues/11535) | AI Returns an error for every query - max_tokens is too large: 127795 when model is gpt-4o |
| [#11512](https://gitlab.com/gnachman/iterm2/-/issues/11512) | Enable/disable AI per profile |
| [#11509](https://gitlab.com/gnachman/iterm2/-/issues/11509) | AI features should have an option for using local LLMs |
| [#11493](https://gitlab.com/gnachman/iterm2/-/issues/11493) | AI Prompt Sends max tokens in Composer. |
| [#11427](https://gitlab.com/gnachman/iterm2/-/issues/11427) | AI prompt suggestions |
| [#11416](https://gitlab.com/gnachman/iterm2/-/issues/11416) | Using AI suggestions with Azure OpenAI backend |
| [#11260](https://gitlab.com/gnachman/iterm2/-/issues/11260) | Need help to setup AI feature of iterm2 |
| [#6955](https://gitlab.com/gnachman/iterm2/-/issues/6955) | enhancement - us AI to capture errors / look up solutions on stackoverflow - present solution |

---

## tmux Integration (P1)

**Count:** 195

| Issue | Title |
|-------|-------|
| [#12644](https://gitlab.com/gnachman/iterm2/-/issues/12644) | Detaching from tmux control mode closes iTerm before window gets unburied |
| [#12612](https://gitlab.com/gnachman/iterm2/-/issues/12612) | Hidden panes flash in tmux session when using option+key shortcuts |
| [#12552](https://gitlab.com/gnachman/iterm2/-/issues/12552) | "Send tmux command" / control mode doesn't seem to support if/if-shell |
| [#12542](https://gitlab.com/gnachman/iterm2/-/issues/12542) | Reattached tmux tabs are red in 3.6.4 |
| [#12540](https://gitlab.com/gnachman/iterm2/-/issues/12540) | AppleScript: set background color fails when attached to a tmux session |
| [#12497](https://gitlab.com/gnachman/iterm2/-/issues/12497) | Connections with tailscale SSH and tmux integration |
| [#12385](https://gitlab.com/gnachman/iterm2/-/issues/12385) | Tmux integration cannot use OSC 52 (system clipboard) |
| [#12357](https://gitlab.com/gnachman/iterm2/-/issues/12357) | it2 tools print "tmux;" in GNU screen |
| [#12333](https://gitlab.com/gnachman/iterm2/-/issues/12333) | Tmux server's history-limit isn't working with tmux integration |
| [#12317](https://gitlab.com/gnachman/iterm2/-/issues/12317) | When reconnecting to a tmux session, place tmux windows on the same desktop as the window I am conne... |
| [#12199](https://gitlab.com/gnachman/iterm2/-/issues/12199) | Ghost window after disconnecting from SSH with tmux integration due to network interruption |
| [#12172](https://gitlab.com/gnachman/iterm2/-/issues/12172) | Yellow ANSI code (3) renders as white (7) in tmux integration |
| [#12149](https://gitlab.com/gnachman/iterm2/-/issues/12149) | DashTerm2 does not work with tmux display-popup in tmux itergration mode |
| [#11918](https://gitlab.com/gnachman/iterm2/-/issues/11918) | when using tmux -CC window size is no even when max left/right |
| [#11873](https://gitlab.com/gnachman/iterm2/-/issues/11873) | Save profile for Tmux windows |
| [#11810](https://gitlab.com/gnachman/iterm2/-/issues/11810) | tmux integration does not respect local window size |
| [#11775](https://gitlab.com/gnachman/iterm2/-/issues/11775) | Restore inline images after reattaching to a tmux session |
| [#11768](https://gitlab.com/gnachman/iterm2/-/issues/11768) | Setting tab title to tmux sesion name |
| [#11718](https://gitlab.com/gnachman/iterm2/-/issues/11718) | tmux CC (control mode) window will not resize |
| [#11593](https://gitlab.com/gnachman/iterm2/-/issues/11593) | Detect previous prompts in tmux on reconnect |
| [#11519](https://gitlab.com/gnachman/iterm2/-/issues/11519) | Tmux unusable in 3.5.0 - closing window always detaches session |
| [#11465](https://gitlab.com/gnachman/iterm2/-/issues/11465) | New tmux tabs steal focus from the current tab |
| [#11433](https://gitlab.com/gnachman/iterm2/-/issues/11433) | When using multiple tmux sessions tab titles for all sessions get overwritten with the titles from t... |
| [#11424](https://gitlab.com/gnachman/iterm2/-/issues/11424) | tmux - "Native tabs in a new window" doesn't work |
| [#11353](https://gitlab.com/gnachman/iterm2/-/issues/11353) | iterm2 not loggging issue with tmux |
| [#11325](https://gitlab.com/gnachman/iterm2/-/issues/11325) | tmux session is detached when ever there is a broadcast message |
| [#11279](https://gitlab.com/gnachman/iterm2/-/issues/11279) | feature request: export tmux config used by iTerm |
| [#11177](https://gitlab.com/gnachman/iterm2/-/issues/11177) | TMUX session requires multiple attempts to open session window |
| [#11174](https://gitlab.com/gnachman/iterm2/-/issues/11174) | tmux control mode (often) fails to launch window when creating or attaching to a session |
| [#11126](https://gitlab.com/gnachman/iterm2/-/issues/11126) | Tmux laggy new-tab doesn't buffer input |
| [#11053](https://gitlab.com/gnachman/iterm2/-/issues/11053) | Zooming in to iTerm with tmux Integration Resizes Window Instead of Just Text |
| [#11028](https://gitlab.com/gnachman/iterm2/-/issues/11028) | PTYSession use-after-free via TmuxGateway delegate |
| [#10981](https://gitlab.com/gnachman/iterm2/-/issues/10981) | Allow tab tames with tmux to show job name |
| [#10889](https://gitlab.com/gnachman/iterm2/-/issues/10889) | italic font in Neovim renders with background colour with tmux -CC |
| [#10762](https://gitlab.com/gnachman/iterm2/-/issues/10762) | Tmux integration aggresive-resize error despite it being disabled |
| [#10717](https://gitlab.com/gnachman/iterm2/-/issues/10717) | F1-F4 function keys send unexpected key codes when using tmux integration |
| [#10567](https://gitlab.com/gnachman/iterm2/-/issues/10567) | tmux window position restore does not move windows to correct desktop |
| [#10559](https://gitlab.com/gnachman/iterm2/-/issues/10559) | Mouse reporting and tmux |
| [#10490](https://gitlab.com/gnachman/iterm2/-/issues/10490) | Bell dinging when creating or tabbing into 'native' tmux terminal windows and tabs |
| [#10342](https://gitlab.com/gnachman/iterm2/-/issues/10342) | [Alt-Click Move Cursor] this feature not work with tmux |
| [#10262](https://gitlab.com/gnachman/iterm2/-/issues/10262) | Opening a new tmux tab unexpectedly resizes window |
| [#10252](https://gitlab.com/gnachman/iterm2/-/issues/10252) | Tmux semantic history |
| [#10160](https://gitlab.com/gnachman/iterm2/-/issues/10160) | Disable showing tmux copy-mode selection highlight in vim/nvim |
| [#10142](https://gitlab.com/gnachman/iterm2/-/issues/10142) | tmux not starting (after working once) |
| [#10129](https://gitlab.com/gnachman/iterm2/-/issues/10129) | input lag seriously in tmux mode though ssh |
| [#10044](https://gitlab.com/gnachman/iterm2/-/issues/10044) | ⌘command-r breaks scrolling in tmux - due to disabling of Mouse-Reporting? |
| [#9970](https://gitlab.com/gnachman/iterm2/-/issues/9970) | tmux integration does not recognize the prefix <ctrl>b |
| [#9963](https://gitlab.com/gnachman/iterm2/-/issues/9963) | When using tmux integration (`tmux -CC`), ctrl+space becomes the null character (0x0) which makes em... |
| [#9960](https://gitlab.com/gnachman/iterm2/-/issues/9960) | iterm2 fails to open tmux |
| [#9901](https://gitlab.com/gnachman/iterm2/-/issues/9901) | tmux server exited unexpectedly (pane title issue ?) |
| [#9889](https://gitlab.com/gnachman/iterm2/-/issues/9889) | dynamic per-host colors for tmux integration windows |
| [#9881](https://gitlab.com/gnachman/iterm2/-/issues/9881) | tmux Buried Sessions stacking up (duplicating) with each Command-Q and re-launch of DashTerm2 |
| [#9858](https://gitlab.com/gnachman/iterm2/-/issues/9858) | tmux lines disappear |
| [#9817](https://gitlab.com/gnachman/iterm2/-/issues/9817) | custom tab title not respected (integrated tmux) |
| [#9807](https://gitlab.com/gnachman/iterm2/-/issues/9807) | Some emoji do not display in tmux |
| [#9786](https://gitlab.com/gnachman/iterm2/-/issues/9786) | tmux + WeeChat displays characters in places they shouldn't be |
| [#9702](https://gitlab.com/gnachman/iterm2/-/issues/9702) | OSC 4 get background color doesn't work with tmux integration |
| [#9696](https://gitlab.com/gnachman/iterm2/-/issues/9696) | tmux 3.2 pane visual glitches (size "jumping") with lots of output |
| [#9687](https://gitlab.com/gnachman/iterm2/-/issues/9687) | Disable "scroll wheel sends arrow keys in alternative screen mode" for tmux, enable for tmux+vim |
| [#9684](https://gitlab.com/gnachman/iterm2/-/issues/9684) | Incorrect output while in tmux native window mode |
| [#9657](https://gitlab.com/gnachman/iterm2/-/issues/9657) | Buffer keystrokes when opening new tmux tab/window with tmux integration |
| [#9605](https://gitlab.com/gnachman/iterm2/-/issues/9605) | Tab Title not honored using tmux |
| [#9600](https://gitlab.com/gnachman/iterm2/-/issues/9600) | DashTerm2 not responding when using tmux |
| [#9559](https://gitlab.com/gnachman/iterm2/-/issues/9559) | Why does tmux on DashTerm2 not allow me to copy more than ~350 lines onto my local Mac clipboard? |
| [#9550](https://gitlab.com/gnachman/iterm2/-/issues/9550) | it2dl utility does not have the same tmux workaround that it2ul does |
| [#9480](https://gitlab.com/gnachman/iterm2/-/issues/9480) | iTerm tmux integrating, terminal height gets smaller when tmux -CC starts |
| [#9357](https://gitlab.com/gnachman/iterm2/-/issues/9357) | Resize of iTerm Window on Reattacht to TMUX session. |
| [#9333](https://gitlab.com/gnachman/iterm2/-/issues/9333) | tmux integration #W vs #T (title vs window-string) |
| [#9299](https://gitlab.com/gnachman/iterm2/-/issues/9299) | Error from tmux when disconnecting last window |
| [#9267](https://gitlab.com/gnachman/iterm2/-/issues/9267) | Preserve window size not working with tmux integration |
| [#9195](https://gitlab.com/gnachman/iterm2/-/issues/9195) | Deattaching the tmux session shrinks the window little bit |
| [#9178](https://gitlab.com/gnachman/iterm2/-/issues/9178) | Ability to merge status bar with tab bar when using tmux |
| [#9106](https://gitlab.com/gnachman/iterm2/-/issues/9106) | OSX tmux command not found |
| [#9036](https://gitlab.com/gnachman/iterm2/-/issues/9036) | tmux prefix commands |
| [#9024](https://gitlab.com/gnachman/iterm2/-/issues/9024) | Window continually resizes ("jitters") when resizing a pane in a tmux window |
| [#9020](https://gitlab.com/gnachman/iterm2/-/issues/9020) | TMUX + Background Images |
| [#8974](https://gitlab.com/gnachman/iterm2/-/issues/8974) | DashTerm2 uses excessive CPU when attached to tmux session |
| [#8899](https://gitlab.com/gnachman/iterm2/-/issues/8899) | 3 Finger select not working in tmux |
| [#8757](https://gitlab.com/gnachman/iterm2/-/issues/8757) | Smart Selection \d token inside Action run from tmux |
| [#8709](https://gitlab.com/gnachman/iterm2/-/issues/8709) | tmux -CC, F1 key misbehaving between 3.3.3 and 3.3.9 |
| [#8708](https://gitlab.com/gnachman/iterm2/-/issues/8708) | Windows don't expand to screen on multiple monitors with different resolutions with tmux |
| [#8703](https://gitlab.com/gnachman/iterm2/-/issues/8703) | Python API Tab object should provide more tmux information |
| [#8697](https://gitlab.com/gnachman/iterm2/-/issues/8697) | mouse reporting issues on half the screen (tmux) or char > 540 (non tmux) |
| [#8696](https://gitlab.com/gnachman/iterm2/-/issues/8696) | tmux + native fullscreen split produces strange gaps |
| [#8612](https://gitlab.com/gnachman/iterm2/-/issues/8612) | Resizing tmux session based on iterm window size instead resizing the iterm window based on the tmux... |
| [#8583](https://gitlab.com/gnachman/iterm2/-/issues/8583) | tmux window sizes locked in sync |
| [#8541](https://gitlab.com/gnachman/iterm2/-/issues/8541) | Variable window sizes in tmux are lost when re-attaching |
| [#8530](https://gitlab.com/gnachman/iterm2/-/issues/8530) | Autocomplete's suggestions scope limited to current tmux pane |
| [#8524](https://gitlab.com/gnachman/iterm2/-/issues/8524) | tmux' panes resize with mouse not working |
| [#8442](https://gitlab.com/gnachman/iterm2/-/issues/8442) | Toolbelt doesn't work with tmux integration |
| [#8422](https://gitlab.com/gnachman/iterm2/-/issues/8422) | [Feature Request] tmux integration - include tmux status line? |
| [#8398](https://gitlab.com/gnachman/iterm2/-/issues/8398) | iTerm 3.3.6 is ignoring tmux status bar and always shows the native one |
| [#8324](https://gitlab.com/gnachman/iterm2/-/issues/8324) | Nested tmux sessions with tmux -CC don't seem to work |
| [#8194](https://gitlab.com/gnachman/iterm2/-/issues/8194) | V3.3.3Beta2 on Majave 10.14.5, when open more than one tab, tmux/screen display error |
| [#8167](https://gitlab.com/gnachman/iterm2/-/issues/8167) | Statusbar on bottom won't pick up git branch, directory, process etc through tmux |
| [#7895](https://gitlab.com/gnachman/iterm2/-/issues/7895) | integrated tmux errors firing |
| [#7821](https://gitlab.com/gnachman/iterm2/-/issues/7821) | Creating new tmux tabs shortens window height each time |
| [#7786](https://gitlab.com/gnachman/iterm2/-/issues/7786) | Broadcast message from root on remote SSH system breaks remote tmux session/tabs |
| [#7734](https://gitlab.com/gnachman/iterm2/-/issues/7734) | Re-attaching to tmux session doesn't restore profile stack |
| [#7733](https://gitlab.com/gnachman/iterm2/-/issues/7733) | Connecting to a tmux session which has a large console output buffered fails to fully connect |
| [#7590](https://gitlab.com/gnachman/iterm2/-/issues/7590) | Not able to attach to TMUX session. |
| [#7568](https://gitlab.com/gnachman/iterm2/-/issues/7568) | Cannot update badge with iterm2_set_user_var while inside tmux |
| [#7551](https://gitlab.com/gnachman/iterm2/-/issues/7551) | Tmux integration doesn't trigger preset command |
| [#7478](https://gitlab.com/gnachman/iterm2/-/issues/7478) | Can't reattach tmux session on remote machine afte I lost network (computer goes to sleep) |
| [#7367](https://gitlab.com/gnachman/iterm2/-/issues/7367) | Tmux integration not sourcing .bashrc on re-attach. |
| [#7317](https://gitlab.com/gnachman/iterm2/-/issues/7317) | After Session > Reset, mouse reporting doesn't work in Tmux |
| [#7266](https://gitlab.com/gnachman/iterm2/-/issues/7266) | Feature suggestion: Add the tmux menu to right click menu |
| [#7225](https://gitlab.com/gnachman/iterm2/-/issues/7225) | Tmux+Maximizing window leaves gaps in the screen |
| [#7089](https://gitlab.com/gnachman/iterm2/-/issues/7089) | tmux CC mode sometimes improperly restores multi-tabbed window into separate windows |
| [#6959](https://gitlab.com/gnachman/iterm2/-/issues/6959) | I can't enter copy mode in Tmux with mouse scroll |
| [#6950](https://gitlab.com/gnachman/iterm2/-/issues/6950) | tmux integration with remote shows local user@host instead of remote |
| [#6801](https://gitlab.com/gnachman/iterm2/-/issues/6801) | When a tmux pane is fullscreen, iterm2 sends a resize command to make it smaller when it does not ne... |
| [#6799](https://gitlab.com/gnachman/iterm2/-/issues/6799) | imgcat not working inside tmux |
| [#6766](https://gitlab.com/gnachman/iterm2/-/issues/6766) | tmux tabs do not get completely restored and no keyboard input accepted |
| [#6666](https://gitlab.com/gnachman/iterm2/-/issues/6666) | tmux bottom status bar stacking on itself constantly growing (see screenshot) |
| [#6551](https://gitlab.com/gnachman/iterm2/-/issues/6551) | Starting new tmux session in native tab unexpectedly resizes window |
| [#6499](https://gitlab.com/gnachman/iterm2/-/issues/6499) | Escape code output on mouse actions after ssh session with tmux is interrupted |
| [#6424](https://gitlab.com/gnachman/iterm2/-/issues/6424) | tmux integration even-horizontal, even-vertical support |
| [#6400](https://gitlab.com/gnachman/iterm2/-/issues/6400) | New tmux windows created outside of iterm-tmux will be treated as new windows instead of new tabs in... |
| [#6383](https://gitlab.com/gnachman/iterm2/-/issues/6383) | tmux Command Menu shortcuts |
| [#6354](https://gitlab.com/gnachman/iterm2/-/issues/6354) | [IMPROVEMENT] rename window in Tmux integration mode |
| [#6320](https://gitlab.com/gnachman/iterm2/-/issues/6320) | DashTerm2 doesn't apply tab color for tmux profile after starting a new tmux session |
| [#6304](https://gitlab.com/gnachman/iterm2/-/issues/6304) | Unable to detach zombie session with iterm2 and tmux |
| [#6269](https://gitlab.com/gnachman/iterm2/-/issues/6269) | Under High Sierra, tmux window doesn't auto minimize when window style is "No Title Bar" upon `-CC a... |
| [#6223](https://gitlab.com/gnachman/iterm2/-/issues/6223) | Last couple rows of pixels are wrapping to the top of the display when detaching from tmux |
| [#6192](https://gitlab.com/gnachman/iterm2/-/issues/6192) | tmux mouse integration sticks after detaching |
| [#6137](https://gitlab.com/gnachman/iterm2/-/issues/6137) | Option "Prefs > Advanced > Should growing or shrinking the font in a session that's broadcasting inp... |
| [#6126](https://gitlab.com/gnachman/iterm2/-/issues/6126) | Tmux integration random commands running on local shell |
| [#6070](https://gitlab.com/gnachman/iterm2/-/issues/6070) | Pressing Command + Escape twice with (an active tmux session + buried session) to hide the active se... |
| [#6042](https://gitlab.com/gnachman/iterm2/-/issues/6042) | New window hotkey always opens tab in tmux |
| [#5991](https://gitlab.com/gnachman/iterm2/-/issues/5991) | tmux integration broken when opening second session |
| [#5972](https://gitlab.com/gnachman/iterm2/-/issues/5972) | Tmux session lost after macOS restart |
| [#5946](https://gitlab.com/gnachman/iterm2/-/issues/5946) | Separate number of lines to sync from tmux integration from number of lines of history to keep |
| [#5905](https://gitlab.com/gnachman/iterm2/-/issues/5905) | tmux detaches on trying to access a forwarded port |
| [#5873](https://gitlab.com/gnachman/iterm2/-/issues/5873) | Using AppleScript with tmux to create new windows/tabs with sessions that are known to tmux |
| [#5846](https://gitlab.com/gnachman/iterm2/-/issues/5846) | iTerm tmux mode though a remote machine made a wrong copy on soft wraps. |
| [#5751](https://gitlab.com/gnachman/iterm2/-/issues/5751) | tmux: Attaching to a session sometimes loses tab layout |
| [#5746](https://gitlab.com/gnachman/iterm2/-/issues/5746) | tmux command drawer |
| [#5742](https://gitlab.com/gnachman/iterm2/-/issues/5742) | tmux integration: support marked panes |
| [#5717](https://gitlab.com/gnachman/iterm2/-/issues/5717) | Respsect tmux's synchronize-panes option in integration mode |
| [#5704](https://gitlab.com/gnachman/iterm2/-/issues/5704) | [Tmux Integration] : Resizing on split pane |
| [#5669](https://gitlab.com/gnachman/iterm2/-/issues/5669) | Black bars appear in tmux sessions |
| [#5598](https://gitlab.com/gnachman/iterm2/-/issues/5598) | tmux tabs splits into windows upon re-attach |
| [#5537](https://gitlab.com/gnachman/iterm2/-/issues/5537) | [Feature request] Tree view for switching window/session in tmux integration mode |
| [#5518](https://gitlab.com/gnachman/iterm2/-/issues/5518) | White gaps on new tabs after splitting tabs using tmux integration |
| [#5472](https://gitlab.com/gnachman/iterm2/-/issues/5472) | how to open new tmux session in a new tab? |
| [#5461](https://gitlab.com/gnachman/iterm2/-/issues/5461) | when no tmux sessions, "tmux -CC at" print two more errors. |
| [#5340](https://gitlab.com/gnachman/iterm2/-/issues/5340) | tmux mode breaks ServerAliveInterval/ServerAliveCountMax |
| [#5291](https://gitlab.com/gnachman/iterm2/-/issues/5291) | tmux windows inherit the default color scheme/theme instead of the one from the host profile. |
| [#5128](https://gitlab.com/gnachman/iterm2/-/issues/5128) | Tmux integration bug when "open tmux windows as tabs in existing window" setting is selected |
| [#5078](https://gitlab.com/gnachman/iterm2/-/issues/5078) | While in a tmux session, support using different profiles by default when creating new tabs v.s. new... |
| [#5026](https://gitlab.com/gnachman/iterm2/-/issues/5026) | Bracketed paste's escape codes break with tmux |
| [#4959](https://gitlab.com/gnachman/iterm2/-/issues/4959) | tmux integration should remember tab colors |
| [#4926](https://gitlab.com/gnachman/iterm2/-/issues/4926) | tmux tab(s) are broken out into their own window upon reconnecting |
| [#4906](https://gitlab.com/gnachman/iterm2/-/issues/4906) | tmux integration clutters shell history |
| [#4898](https://gitlab.com/gnachman/iterm2/-/issues/4898) | FEATURE REQUEST - Maintain tab color after detach & re-attach in tmux (also, maintain order of tabs?... |
| [#4882](https://gitlab.com/gnachman/iterm2/-/issues/4882) | Cmd + Click on any file listed (by ls) inside a tmux session does not work |
| [#4766](https://gitlab.com/gnachman/iterm2/-/issues/4766) | DashTerm2 sending characters to windows where TMUX was closed |
| [#4754](https://gitlab.com/gnachman/iterm2/-/issues/4754) | Separate window and tab titles in tmux integration |
| [#4732](https://gitlab.com/gnachman/iterm2/-/issues/4732) | Offer "open in tmux gateway's tab" in tmux dashboard as an alternative to open in windows |
| [#4696](https://gitlab.com/gnachman/iterm2/-/issues/4696) | Automatic profile switching does not work with tmux |
| [#4608](https://gitlab.com/gnachman/iterm2/-/issues/4608) | shell integration is disabled when in tmux integration mode |
| [#4588](https://gitlab.com/gnachman/iterm2/-/issues/4588) | tmux -CC : first window theme different from the subsequent ones |
| [#4549](https://gitlab.com/gnachman/iterm2/-/issues/4549) | Improve handling of unexpected output in tmux integration [was: tmux integration does not properly h... |
| [#4543](https://gitlab.com/gnachman/iterm2/-/issues/4543) | Support Automatic Profile Switching with Integrated Tmux windows |
| [#4436](https://gitlab.com/gnachman/iterm2/-/issues/4436) | Fontd CPU usage goes up to 60 to 80% when using tmux integration |
| [#4427](https://gitlab.com/gnachman/iterm2/-/issues/4427) | Better (n)vim scrolling performance inside of tmux |
| [#4223](https://gitlab.com/gnachman/iterm2/-/issues/4223) | FR: tmux integration - opening new tabs in the current window from shell |
| [#4204](https://gitlab.com/gnachman/iterm2/-/issues/4204) | iterm2 continues to send commands to tmux after closing command control |
| [#4165](https://gitlab.com/gnachman/iterm2/-/issues/4165) | (incomplete functionality/feature request) Option to auto un-hide the tab after tmux client session ... |
| [#4063](https://gitlab.com/gnachman/iterm2/-/issues/4063) | tmux emulation slow |
| [#3953](https://gitlab.com/gnachman/iterm2/-/issues/3953) | Tab names in tmux mode not updating  |
| [#3915](https://gitlab.com/gnachman/iterm2/-/issues/3915) | tmux windows non-responsive when one tmux tab/window has a lot of data outputting to window |
| [#3888](https://gitlab.com/gnachman/iterm2/-/issues/3888) | tmux integration always enabled/active |
| [#3827](https://gitlab.com/gnachman/iterm2/-/issues/3827) | full width render blinking (tmux+nvim/vim) |
| [#3812](https://gitlab.com/gnachman/iterm2/-/issues/3812) | New tmux tab does not open in home directory |
| [#3748](https://gitlab.com/gnachman/iterm2/-/issues/3748) | Home and End keys do not work in Tmux integration mode w/ screen-256color |
| [#3747](https://gitlab.com/gnachman/iterm2/-/issues/3747) | 'Suppress alert asking what kind of tab/window to open in tmux integration' option prevents normal w... |
| [#3745](https://gitlab.com/gnachman/iterm2/-/issues/3745) | Linking a window from another tmux session doesn't create a new tab |
| [#3685](https://gitlab.com/gnachman/iterm2/-/issues/3685) | Spacing around tmux panes is inconsistent |
| [#3584](https://gitlab.com/gnachman/iterm2/-/issues/3584) | tmux integration should remember window size |
| [#3582](https://gitlab.com/gnachman/iterm2/-/issues/3582) | Separate settings for what new window vs new tab does when in a tmux session |
| [#3538](https://gitlab.com/gnachman/iterm2/-/issues/3538) | tmux integration not working on a particular server  |
| [#3448](https://gitlab.com/gnachman/iterm2/-/issues/3448) | More window control in "tmux Dashboard" |
| [#3447](https://gitlab.com/gnachman/iterm2/-/issues/3447) | creating new tmux window via DashBoard causes some or all hidden windows to become visible |
| [#3412](https://gitlab.com/gnachman/iterm2/-/issues/3412) | Unable to save window arrangement when in tmux session |
| [#3396](https://gitlab.com/gnachman/iterm2/-/issues/3396) | Linefeed scrolling screeni n alt screen with save to scrollback off clears selection [was: Long upda... |
| [#3121](https://gitlab.com/gnachman/iterm2/-/issues/3121) | replace applescript interface with command line interface (ala tmux) |
| [#3118](https://gitlab.com/gnachman/iterm2/-/issues/3118) | Add support for marks to tmux [was: marks work but not inside tmux] |
| [#2950](https://gitlab.com/gnachman/iterm2/-/issues/2950) | tmux tabs should use profile of gateway |
| [#2585](https://gitlab.com/gnachman/iterm2/-/issues/2585) | Support for aggressive-resize on for tmux integration |
| [#2347](https://gitlab.com/gnachman/iterm2/-/issues/2347) | allow tmux -C to run in "background" |
| [#1995](https://gitlab.com/gnachman/iterm2/-/issues/1995) | Easily switch between pane layouts like tmux's next-layout |
| [#1933](https://gitlab.com/gnachman/iterm2/-/issues/1933) | When getting a high volume of output from tmux, you can't send a C-c |
| [#1877](https://gitlab.com/gnachman/iterm2/-/issues/1877) | It would be nice to have the tmux dashboard as a toolbelt. |

---

## SSH/SFTP/SCP (P1)

**Count:** 72

| Issue | Title |
|-------|-------|
| [#12412](https://gitlab.com/gnachman/iterm2/-/issues/12412) | After Tahoe update, ssh and ping to local network no longer work |
| [#12369](https://gitlab.com/gnachman/iterm2/-/issues/12369) | SCP shell integration not working |
| [#12364](https://gitlab.com/gnachman/iterm2/-/issues/12364) | can't ping or ssh from DashTerm2 but only on certain networks! very weird. |
| [#12360](https://gitlab.com/gnachman/iterm2/-/issues/12360) | Cursor is lost and scrolling cannot be used after an SSH session disconnect |
| [#12291](https://gitlab.com/gnachman/iterm2/-/issues/12291) | rclone to sftp targets fail with iterm2 |
| [#12276](https://gitlab.com/gnachman/iterm2/-/issues/12276) | Allow indicating an scp command to use rather that using internal scp library |
| [#12245](https://gitlab.com/gnachman/iterm2/-/issues/12245) | SSH Integration failed password + OTP login |
| [#12236](https://gitlab.com/gnachman/iterm2/-/issues/12236) | Mouse doesn't work with Textual apps over SSH starting with 3.5.6 |
| [#12229](https://gitlab.com/gnachman/iterm2/-/issues/12229) | Cant find way to send Ctrl+Left/Right/... to application on ssh remote |
| [#12213](https://gitlab.com/gnachman/iterm2/-/issues/12213) | Unable to connect to SSH profile using GSSAPIAuthentication |
| [#12164](https://gitlab.com/gnachman/iterm2/-/issues/12164) | SSH Handler doesn't work with colon character in username field |
| [#12157](https://gitlab.com/gnachman/iterm2/-/issues/12157) | lrzsz stop work if open DashTerm2 through ssh:// URL scheme |
| [#11901](https://gitlab.com/gnachman/iterm2/-/issues/11901) | On Mac Sequoia, with a default bash (/opt/homebrew/bin/bash), I cannot ssh to a local machine on my ... |
| [#11846](https://gitlab.com/gnachman/iterm2/-/issues/11846) | Random ssh disconnects |
| [#11839](https://gitlab.com/gnachman/iterm2/-/issues/11839) | undeclared identifier gNMSSHTraceCallback |
| [#11694](https://gitlab.com/gnachman/iterm2/-/issues/11694) | Add ed25519 ssh key support for SCP |
| [#11682](https://gitlab.com/gnachman/iterm2/-/issues/11682) | shell integration can't scp: download failed |
| [#11604](https://gitlab.com/gnachman/iterm2/-/issues/11604) | Regression in 3.5.0's environment handling when used as an ssh URL handler |
| [#11589](https://gitlab.com/gnachman/iterm2/-/issues/11589) | [feature request] Shell Autocomplete support for Auto Compoer over SSH |
| [#11483](https://gitlab.com/gnachman/iterm2/-/issues/11483) | delay with ssh/git commands and ssh agent secretive |
| [#11430](https://gitlab.com/gnachman/iterm2/-/issues/11430) | SCP shell integration tries to connect to hosts that are just filename:linenumber |
| [#11422](https://gitlab.com/gnachman/iterm2/-/issues/11422) | shell integration can't scp: "Authentication Error" |
| [#11411](https://gitlab.com/gnachman/iterm2/-/issues/11411) | SSH shell integration not working with passwordless key. |
| [#11393](https://gitlab.com/gnachman/iterm2/-/issues/11393) | scp via shell integration not working on Raspberry Pi (mDNS issue?) |
| [#11266](https://gitlab.com/gnachman/iterm2/-/issues/11266) | scp fails when select a fail and right click -> download with scp from ... |
| [#11196](https://gitlab.com/gnachman/iterm2/-/issues/11196) | Doesn't reach server via ssh without restarting iterm2 |
| [#10884](https://gitlab.com/gnachman/iterm2/-/issues/10884) | Setting a profile to connect to a remote system via ssh does not trigger that the session is active ... |
| [#10833](https://gitlab.com/gnachman/iterm2/-/issues/10833) | DashTerm2 not using my ssh key define in |
| [#10774](https://gitlab.com/gnachman/iterm2/-/issues/10774) | it2ssh gibberish |
| [#10699](https://gitlab.com/gnachman/iterm2/-/issues/10699) | Can't upload or download files via scp |
| [#10495](https://gitlab.com/gnachman/iterm2/-/issues/10495) | Hight mem usage by monitoring via ssh since maybe version 3.5.0.beta4 |
| [#10463](https://gitlab.com/gnachman/iterm2/-/issues/10463) | Feature Request: local line editing for ssh |
| [#10351](https://gitlab.com/gnachman/iterm2/-/issues/10351) | Repeated notifications-bells while switching from and back to a tab where SSH session terminated |
| [#10156](https://gitlab.com/gnachman/iterm2/-/issues/10156) | Question with SCP multihop copy |
| [#9933](https://gitlab.com/gnachman/iterm2/-/issues/9933) | Download with scp when using public keys? |
| [#9618](https://gitlab.com/gnachman/iterm2/-/issues/9618) | Emulate Clusterssh from Linux |
| [#9494](https://gitlab.com/gnachman/iterm2/-/issues/9494) | PKCS11Provider support for scp in shell integration |
| [#9413](https://gitlab.com/gnachman/iterm2/-/issues/9413) | SSH Statusbar shows only hostname & folder |
| [#9323](https://gitlab.com/gnachman/iterm2/-/issues/9323) | Keyboard arrows don't work on SSH to Ubuntu remotes.  Macbook Air thinks I'm clicking the BELL chara... |
| [#8470](https://gitlab.com/gnachman/iterm2/-/issues/8470) | Escape codes not working if run right after exiting ssh |
| [#8404](https://gitlab.com/gnachman/iterm2/-/issues/8404) | [Feature Request] KeePass / KeePassHttp Integration |
| [#8197](https://gitlab.com/gnachman/iterm2/-/issues/8197) | Badge wont update in ssh (works fine in local mac env) |
| [#8142](https://gitlab.com/gnachman/iterm2/-/issues/8142) | it2ul is much slower than scp in command |
| [#8113](https://gitlab.com/gnachman/iterm2/-/issues/8113) | Username on status bar does not update when switching user in an ssh session |
| [#8035](https://gitlab.com/gnachman/iterm2/-/issues/8035) | Opening SSH links with iTerm 3.3.0 |
| [#7743](https://gitlab.com/gnachman/iterm2/-/issues/7743) | VIM layout issue while doing SSH via DashTerm2 |
| [#7469](https://gitlab.com/gnachman/iterm2/-/issues/7469) | Not opening ssh links from Chrome if link contains additional command. |
| [#7455](https://gitlab.com/gnachman/iterm2/-/issues/7455) | imgcat not working over ssh |
| [#7411](https://gitlab.com/gnachman/iterm2/-/issues/7411) | Feature request: support `ProxyCommand` of ssh_config files |
| [#7341](https://gitlab.com/gnachman/iterm2/-/issues/7341) | SCP copy using triggers failing |
| [#7096](https://gitlab.com/gnachman/iterm2/-/issues/7096) | When I ssh into my Mac from a non-Mac, my prompt is full of escape characters. |
| [#6722](https://gitlab.com/gnachman/iterm2/-/issues/6722) | how to set tab name to ssh host |
| [#6710](https://gitlab.com/gnachman/iterm2/-/issues/6710) | SSH does not set window title |
| [#6515](https://gitlab.com/gnachman/iterm2/-/issues/6515) | Shell Integration SCP Doesn't Work with ssh_config Match Directives |
| [#6234](https://gitlab.com/gnachman/iterm2/-/issues/6234) | Unable to SCP |
| [#5659](https://gitlab.com/gnachman/iterm2/-/issues/5659) | Tab and window title not updated after logging out of SSH remote session |
| [#5609](https://gitlab.com/gnachman/iterm2/-/issues/5609) | copy/paste can miss some character when used with ssh+gnu screen in a remote server |
| [#5566](https://gitlab.com/gnachman/iterm2/-/issues/5566) | SSH URL handler not populating server name |
| [#5560](https://gitlab.com/gnachman/iterm2/-/issues/5560) | Locales not set for ssh session |
| [#5485](https://gitlab.com/gnachman/iterm2/-/issues/5485) | unable to use download file option from shell integration because scp prompts for key/passwords |
| [#5028](https://gitlab.com/gnachman/iterm2/-/issues/5028) | support for copying text back to the clipboard of the local machine when sshing |
| [#4779](https://gitlab.com/gnachman/iterm2/-/issues/4779) | right-click on file to download via SCP, doesn't use the ssh config's aliases |
| [#4677](https://gitlab.com/gnachman/iterm2/-/issues/4677) | SSH Bookmark Manager / Extend Password Manager with field for SSH url |
| [#4657](https://gitlab.com/gnachman/iterm2/-/issues/4657) | Add support for ssh_config ProxyCommand to enable multiple hops in the SCP |
| [#4570](https://gitlab.com/gnachman/iterm2/-/issues/4570) | Feature Request: File transfers panel feature instead of integrated third part sftp  iterm2-zmodem |
| [#4371](https://gitlab.com/gnachman/iterm2/-/issues/4371) | Badges do not work when SSH into CentOS 6 . |
| [#4194](https://gitlab.com/gnachman/iterm2/-/issues/4194) | Improve hostname detectiong for SCP and Automatic Profile Switching [was: Shell Integration determin... |
| [#4171](https://gitlab.com/gnachman/iterm2/-/issues/4171) | Drag-drop file upload using scp fails |
| [#3739](https://gitlab.com/gnachman/iterm2/-/issues/3739) | Extend right-click to scp to download directories |
| [#3317](https://gitlab.com/gnachman/iterm2/-/issues/3317) | Recursive scp, i.e., scp'ing directories |
| [#2492](https://gitlab.com/gnachman/iterm2/-/issues/2492) | Semantic history does not detect filenames when ssh-ed into remote machine |
| [#1608](https://gitlab.com/gnachman/iterm2/-/issues/1608) | Better Integration with SSH sessions |

---

## Shell Integration (P1)

**Count:** 63

| Issue | Title |
|-------|-------|
| [#12641](https://gitlab.com/gnachman/iterm2/-/issues/12641) | Fish 4.x shell corrupts with "Load shell integration automatically" enabled |
| [#12616](https://gitlab.com/gnachman/iterm2/-/issues/12616) | FYI: Fish will soon include built-in support for most of shell integration |
| [#12518](https://gitlab.com/gnachman/iterm2/-/issues/12518) | Shell integration significantly slows prompt rendering |
| [#12382](https://gitlab.com/gnachman/iterm2/-/issues/12382) | Shell integration: `OSC 133; D` not considered the end of a command |
| [#12240](https://gitlab.com/gnachman/iterm2/-/issues/12240) | Shell integration is showing some very strange results |
| [#12168](https://gitlab.com/gnachman/iterm2/-/issues/12168) | "Install Shell Integration" Should Indicate Whether It has already been installed successfully. |
| [#11378](https://gitlab.com/gnachman/iterm2/-/issues/11378) | Load shell integration automatically : breaks connexion if a user starts typing too early |
| [#11294](https://gitlab.com/gnachman/iterm2/-/issues/11294) | command not found: iterm2_shell_integration.zsh |
| [#11016](https://gitlab.com/gnachman/iterm2/-/issues/11016) | Shell integration breaks custom bash PROMPT_COMMAND using exit status |
| [#10537](https://gitlab.com/gnachman/iterm2/-/issues/10537) | Shell integration is messed up with starship prompt |
| [#10528](https://gitlab.com/gnachman/iterm2/-/issues/10528) | bash shell integration not work? like CurrentDir not output |
| [#10300](https://gitlab.com/gnachman/iterm2/-/issues/10300) | Update shell integration documentation |
| [#10280](https://gitlab.com/gnachman/iterm2/-/issues/10280) | Shell integration on website is outdated |
| [#10218](https://gitlab.com/gnachman/iterm2/-/issues/10218) | Shell integration interferes with working directory preservation for new tab/window/pane |
| [#10183](https://gitlab.com/gnachman/iterm2/-/issues/10183) | Shell integration causes issues viewing files in /usr/bin/less on remote host |
| [#10172](https://gitlab.com/gnachman/iterm2/-/issues/10172) | iTerm Shell Integrations mangle login shell command output |
| [#9840](https://gitlab.com/gnachman/iterm2/-/issues/9840) | .iterm2_shell_integration.bash reports syntax error |
| [#9831](https://gitlab.com/gnachman/iterm2/-/issues/9831) | Re-sourcing .bash_profile breaks DashTerm2 shell integration if $PROMPT_COMMAND is defined |
| [#9750](https://gitlab.com/gnachman/iterm2/-/issues/9750) | Shell integration can be a pest |
| [#8806](https://gitlab.com/gnachman/iterm2/-/issues/8806) | [ENQUIRY] Shell Integration: Command History |
| [#8554](https://gitlab.com/gnachman/iterm2/-/issues/8554) | "Install Shell Integration" dialog should make clear where it's installed |
| [#8543](https://gitlab.com/gnachman/iterm2/-/issues/8543) | fish shell integration bad hostname |
| [#8367](https://gitlab.com/gnachman/iterm2/-/issues/8367) | Shell Integration and iterm2 server is a SPOF |
| [#8089](https://gitlab.com/gnachman/iterm2/-/issues/8089) | .iterm2_shell_integration.bash returns -t bad option |
| [#7966](https://gitlab.com/gnachman/iterm2/-/issues/7966) | .iterm_shell_integration.zsh gives annoying message if I use "setopt nounset" |
| [#7480](https://gitlab.com/gnachman/iterm2/-/issues/7480) | TCSH Shell Integration Fails from Quotations |
| [#7467](https://gitlab.com/gnachman/iterm2/-/issues/7467) | .iterm2_shell_integration.bash fails on a strict bash shell (with solution) |
| [#7154](https://gitlab.com/gnachman/iterm2/-/issues/7154) | Feature Suggestion: Detect if client is DashTerm2 for shell integration |
| [#6885](https://gitlab.com/gnachman/iterm2/-/issues/6885) | Add shell integration and others script to brew or brew cask |
| [#6727](https://gitlab.com/gnachman/iterm2/-/issues/6727) | bash shell integration should warn if extdebug is on |
| [#6588](https://gitlab.com/gnachman/iterm2/-/issues/6588) | .iterm2_shell_integration.fish doesn't check fish version, and uses `string` function which may not ... |
| [#6542](https://gitlab.com/gnachman/iterm2/-/issues/6542) | shell integration wierd error with zsh and powerline font |
| [#6319](https://gitlab.com/gnachman/iterm2/-/issues/6319) | Installing shell integration under fish overwrites my config |
| [#6260](https://gitlab.com/gnachman/iterm2/-/issues/6260) | Shell Integration doesn't work in Midnight Commander |
| [#6177](https://gitlab.com/gnachman/iterm2/-/issues/6177) | Question regarding shell-integration -- script location |
| [#5964](https://gitlab.com/gnachman/iterm2/-/issues/5964) | Shell integration turns off after first command when installed with another PS1-affecting PROMPT_COM... |
| [#5790](https://gitlab.com/gnachman/iterm2/-/issues/5790) | Shell Integration: "hostname: Unknown host" |
| [#5724](https://gitlab.com/gnachman/iterm2/-/issues/5724) | Installing Shell Integration can fail if file exists |
| [#5695](https://gitlab.com/gnachman/iterm2/-/issues/5695) | advertised shell integrations aren't working.  no errors on install |
| [#5503](https://gitlab.com/gnachman/iterm2/-/issues/5503) | Shell integration interferes with shared bash history |
| [#5479](https://gitlab.com/gnachman/iterm2/-/issues/5479) | Using Shell Integration with pom files |
| [#5092](https://gitlab.com/gnachman/iterm2/-/issues/5092) | Shell Integration don't work |
| [#5017](https://gitlab.com/gnachman/iterm2/-/issues/5017) | Bash Shell Integration + asciinema, export functions ? |
| [#4991](https://gitlab.com/gnachman/iterm2/-/issues/4991) | Bash Shell Integration and git autocompletion don't work well together |
| [#4892](https://gitlab.com/gnachman/iterm2/-/issues/4892) | Security of Shell Integration and a Privacy Policy |
| [#4843](https://gitlab.com/gnachman/iterm2/-/issues/4843) | Streamline fish shell integration |
| [#4816](https://gitlab.com/gnachman/iterm2/-/issues/4816) | Shell Integration appears to be incompatible with bash in vi mode for history search |
| [#4797](https://gitlab.com/gnachman/iterm2/-/issues/4797) | Feature request: allow alternate paths for shell integration script & utilities |
| [#4587](https://gitlab.com/gnachman/iterm2/-/issues/4587) | Shell integration incompatible with bash-git-prompt |
| [#4504](https://gitlab.com/gnachman/iterm2/-/issues/4504) | BUG shell-integration |
| [#4292](https://gitlab.com/gnachman/iterm2/-/issues/4292) | dot file for fish shell integration does not follow XDG path convention |
| [#4277](https://gitlab.com/gnachman/iterm2/-/issues/4277) | what is the correct behavior when splitting a terminal affected by shell integration matching? |
| [#4257](https://gitlab.com/gnachman/iterm2/-/issues/4257) | Shell integration doesn't work with mosh |
| [#4225](https://gitlab.com/gnachman/iterm2/-/issues/4225) | Small problem with shell integration host detection |
| [#4160](https://gitlab.com/gnachman/iterm2/-/issues/4160) | RHEL 7.2 conflicts with Shell Integration |
| [#4140](https://gitlab.com/gnachman/iterm2/-/issues/4140) | With RedHat 7.2, bash 4.2.46(1), .iterm2_shell_integration.bash causes error on login. |
| [#4133](https://gitlab.com/gnachman/iterm2/-/issues/4133) | Shell Integration doesn't work for me |
| [#3982](https://gitlab.com/gnachman/iterm2/-/issues/3982) | shell integrations breaks console |
| [#3865](https://gitlab.com/gnachman/iterm2/-/issues/3865) | Shell Integration not working |
| [#3769](https://gitlab.com/gnachman/iterm2/-/issues/3769) | DashTerm2 Shell Integration affects CodeRunner |
| [#3735](https://gitlab.com/gnachman/iterm2/-/issues/3735) | shell integration typeset error |
| [#3652](https://gitlab.com/gnachman/iterm2/-/issues/3652) | shell_integration.html contains broken link |
| [#3435](https://gitlab.com/gnachman/iterm2/-/issues/3435) | DashTerm2 Shell Integrations don't play well with show-mode-prompt in Bash |

---

## Performance (P2)

**Count:** 173

| Issue | Title |
|-------|-------|
| [#12645](https://gitlab.com/gnachman/iterm2/-/issues/12645) | iTerm causes severe keyboard lag and dropped characters in other applications |
| [#12635](https://gitlab.com/gnachman/iterm2/-/issues/12635) | Memory consumption statistics per tab/window ? |
| [#12544](https://gitlab.com/gnachman/iterm2/-/issues/12544) | Slow scrolling |
| [#12456](https://gitlab.com/gnachman/iterm2/-/issues/12456) | Excessive CPU usage and associated lag |
| [#12332](https://gitlab.com/gnachman/iterm2/-/issues/12332) | High CPU usage while idle |
| [#12275](https://gitlab.com/gnachman/iterm2/-/issues/12275) | Extensive memory allocation when iterm2 is idle. |
| [#11910](https://gitlab.com/gnachman/iterm2/-/issues/11910) | Memory usage shwon in the status bar is not correct |
| [#11851](https://gitlab.com/gnachman/iterm2/-/issues/11851) | iTerm is consuming lots of energy on my MacBook Pro |
| [#11822](https://gitlab.com/gnachman/iterm2/-/issues/11822) | Slower scroll in alternate screen mode |
| [#11770](https://gitlab.com/gnachman/iterm2/-/issues/11770) | iTerm 3.5+ latency and speed issue with large scrollback buffer |
| [#11712](https://gitlab.com/gnachman/iterm2/-/issues/11712) | input lag & slow performance |
| [#11602](https://gitlab.com/gnachman/iterm2/-/issues/11602) | Multi-second lag when focusing window, started in 3.5 |
| [#11584](https://gitlab.com/gnachman/iterm2/-/issues/11584) | Random lags with keyboard inputs |
| [#11555](https://gitlab.com/gnachman/iterm2/-/issues/11555) | Iterm2 start up slower than previous version |
| [#11530](https://gitlab.com/gnachman/iterm2/-/issues/11530) | MemorySaurus |
| [#11382](https://gitlab.com/gnachman/iterm2/-/issues/11382) | DashTerm2 performance compared to other popular terminal emulators |
| [#11301](https://gitlab.com/gnachman/iterm2/-/issues/11301) | Repeated images shown using Iterm 2 protocol consume all OS Memory |
| [#11261](https://gitlab.com/gnachman/iterm2/-/issues/11261) | High CPU and Memory Usage |
| [#11245](https://gitlab.com/gnachman/iterm2/-/issues/11245) | Slow and Lagging Scrolling/Redraw in Neovim after Beta10 |
| [#11216](https://gitlab.com/gnachman/iterm2/-/issues/11216) | High CPU usage when idle with status bar enabled |
| [#11154](https://gitlab.com/gnachman/iterm2/-/issues/11154) | High memory usage |
| [#11094](https://gitlab.com/gnachman/iterm2/-/issues/11094) | Laggy cursor position when starting new line |
| [#11079](https://gitlab.com/gnachman/iterm2/-/issues/11079) | Using 'sz' to download large files leads to memory explosion |
| [#11013](https://gitlab.com/gnachman/iterm2/-/issues/11013) | Hotkey window full screen has noticeable input lag after upgrading macOS to Ventura |
| [#10996](https://gitlab.com/gnachman/iterm2/-/issues/10996) | High CPU usage (100%+) with DashTerm2 idling in the background |
| [#10974](https://gitlab.com/gnachman/iterm2/-/issues/10974) | Very slow response; typing loses characters... |
| [#10940](https://gitlab.com/gnachman/iterm2/-/issues/10940) | Screen output is very slow |
| [#10843](https://gitlab.com/gnachman/iterm2/-/issues/10843) | OS zoom feature is laggy while iterm2 is fullscreen |
| [#10810](https://gitlab.com/gnachman/iterm2/-/issues/10810) | Memory usage on terminal that is buffering |
| [#10768](https://gitlab.com/gnachman/iterm2/-/issues/10768) | Very slow when create new tab |
| [#10766](https://gitlab.com/gnachman/iterm2/-/issues/10766) | Huge Amount of Memory Used, without currently running processes or jobs |
| [#10756](https://gitlab.com/gnachman/iterm2/-/issues/10756) | Significant performance hit with tab creation/switching in version 3.5.0beta9 |
| [#10726](https://gitlab.com/gnachman/iterm2/-/issues/10726) | Latency in cursor when moving to new line |
| [#10712](https://gitlab.com/gnachman/iterm2/-/issues/10712) | Excessive memory usage, rainbow wheel at startup |
| [#10651](https://gitlab.com/gnachman/iterm2/-/issues/10651) | DashTerm2  homebrew shellenv slows done  prompt returns. |
| [#10601](https://gitlab.com/gnachman/iterm2/-/issues/10601) | keyboard input lags when DashTerm2 3.4.16 is in full screen (non-native) |
| [#10565](https://gitlab.com/gnachman/iterm2/-/issues/10565) | "imgcatted" images don't get freed when overdrawn, causing huge memory demands and interactivity iss... |
| [#10558](https://gitlab.com/gnachman/iterm2/-/issues/10558) | Switching to DashTerm2 is slow |
| [#10440](https://gitlab.com/gnachman/iterm2/-/issues/10440) | Key repeat for non-arrows gets very slow after time |
| [#10362](https://gitlab.com/gnachman/iterm2/-/issues/10362) | Surprising CPU usage when idling in focus |
| [#10329](https://gitlab.com/gnachman/iterm2/-/issues/10329) | Query regarding GPU based performance improvement options in near furture |
| [#10299](https://gitlab.com/gnachman/iterm2/-/issues/10299) | High continuous CPU utilization |
| [#10282](https://gitlab.com/gnachman/iterm2/-/issues/10282) | High CPU usage with small amount of text updating |
| [#10138](https://gitlab.com/gnachman/iterm2/-/issues/10138) | Rendering performance low with "formatted hyperlinks" |
| [#10130](https://gitlab.com/gnachman/iterm2/-/issues/10130) | Keyboard repeat slow, keyboard response slow(er than I sometimes type) |
| [#10083](https://gitlab.com/gnachman/iterm2/-/issues/10083) | M1 Python Environment Bad CPU Type |
| [#10042](https://gitlab.com/gnachman/iterm2/-/issues/10042) | Poor performance with fzf reverse history search on M1 Max |
| [#10012](https://gitlab.com/gnachman/iterm2/-/issues/10012) | Poor Scrolling Performance |
| [#9997](https://gitlab.com/gnachman/iterm2/-/issues/9997) | using mouse to switch panes is painfully slow |
| [#9920](https://gitlab.com/gnachman/iterm2/-/issues/9920) | iTerm uses CPU drawing while the app is idle |
| [#9918](https://gitlab.com/gnachman/iterm2/-/issues/9918) | High CPU usage when idle |
| [#9866](https://gitlab.com/gnachman/iterm2/-/issues/9866) | ITERM2 command line slows down after a few days of use on Big Sur & MacBook Pro 16 |
| [#9860](https://gitlab.com/gnachman/iterm2/-/issues/9860) | Poor drawing performance |
| [#9776](https://gitlab.com/gnachman/iterm2/-/issues/9776) | CPU Spike in BigSur macOS |
| [#9723](https://gitlab.com/gnachman/iterm2/-/issues/9723) | memory not cleared on scrollback buffer clear |
| [#9712](https://gitlab.com/gnachman/iterm2/-/issues/9712) | 100% single core CPU use continually |
| [#9709](https://gitlab.com/gnachman/iterm2/-/issues/9709) | high cpu usage |
| [#9580](https://gitlab.com/gnachman/iterm2/-/issues/9580) | DashTerm2 build 3.4.4 high CPU usage |
| [#9544](https://gitlab.com/gnachman/iterm2/-/issues/9544) | High CPU usage at idle (25%; no status bar) |
| [#9525](https://gitlab.com/gnachman/iterm2/-/issues/9525) | [NSApplication sharedApplication] call is slow when an application is started in DashTerm2 |
| [#9289](https://gitlab.com/gnachman/iterm2/-/issues/9289) | Nightly builds should not lag git commits |
| [#9181](https://gitlab.com/gnachman/iterm2/-/issues/9181) | Script execution is slow |
| [#9176](https://gitlab.com/gnachman/iterm2/-/issues/9176) | Performance on shell spawn |
| [#9128](https://gitlab.com/gnachman/iterm2/-/issues/9128) | Slow tab opening |
| [#9127](https://gitlab.com/gnachman/iterm2/-/issues/9127) | How to monitor memory usage by windows and tabs ? |
| [#9124](https://gitlab.com/gnachman/iterm2/-/issues/9124) | Memory leak in 3.4.20200907-nightly |
| [#9119](https://gitlab.com/gnachman/iterm2/-/issues/9119) | CPU Usage |
| [#9109](https://gitlab.com/gnachman/iterm2/-/issues/9109) | DashTerm2 performance is slow/erratic with some fonts at high resolution |
| [#9093](https://gitlab.com/gnachman/iterm2/-/issues/9093) | High CPU and memory, most options in menu bar greyed out, and mouse will not select any text.. |
| [#9026](https://gitlab.com/gnachman/iterm2/-/issues/9026) | Slow startup on 10.15.5 compared to Terminal.app |
| [#8976](https://gitlab.com/gnachman/iterm2/-/issues/8976) | high cpu usage despite no activity |
| [#8924](https://gitlab.com/gnachman/iterm2/-/issues/8924) | Slow Scrolling/Redraw in Neovim |
| [#8868](https://gitlab.com/gnachman/iterm2/-/issues/8868) | CPU usage constant around 40% when idle |
| [#8856](https://gitlab.com/gnachman/iterm2/-/issues/8856) | Drawing performance drops when ligatures enabled |
| [#8840](https://gitlab.com/gnachman/iterm2/-/issues/8840) | Memory leaks |
| [#8778](https://gitlab.com/gnachman/iterm2/-/issues/8778) | I experienced excessive CPU & memory use issue, when I opened the iterm2 |
| [#8777](https://gitlab.com/gnachman/iterm2/-/issues/8777) | High CPU usage with kafkacat colored stream |
| [#8754](https://gitlab.com/gnachman/iterm2/-/issues/8754) | Iterm Tab Very slow |
| [#8737](https://gitlab.com/gnachman/iterm2/-/issues/8737) | Massive CPU load via kernel_task |
| [#8728](https://gitlab.com/gnachman/iterm2/-/issues/8728) | Feature request: compact CPU, Memory, Network, and Battery components in status bar. |
| [#8662](https://gitlab.com/gnachman/iterm2/-/issues/8662) | Power state (cord vs battery) reverses text foreground/background on Macbook Pro 2019 |
| [#8646](https://gitlab.com/gnachman/iterm2/-/issues/8646) | [Question] DynamicProfiles slow operation with multiple (hundreds) of profiles |
| [#8640](https://gitlab.com/gnachman/iterm2/-/issues/8640) | High CPU usage even when idle |
| [#8594](https://gitlab.com/gnachman/iterm2/-/issues/8594) | Ref 7091 - DashTerm2 slow to open new terminal window - 5 to 10 seconds |
| [#8563](https://gitlab.com/gnachman/iterm2/-/issues/8563) | CPU temperature in status bar |
| [#8449](https://gitlab.com/gnachman/iterm2/-/issues/8449) | [feature] Slower refresh rate for cpu/network/memory meters |
| [#8408](https://gitlab.com/gnachman/iterm2/-/issues/8408) | 3.3.7beta1 Login Shell and Co Processes slow |
| [#8314](https://gitlab.com/gnachman/iterm2/-/issues/8314) | Input lag |
| [#8269](https://gitlab.com/gnachman/iterm2/-/issues/8269) | High CPU use even when window is minimized |
| [#8242](https://gitlab.com/gnachman/iterm2/-/issues/8242) | high cpu usage while idling after a few hours with the latest update (some days ago). |
| [#8228](https://gitlab.com/gnachman/iterm2/-/issues/8228) | DashTerm2 status bar shows incorrect memory |
| [#8128](https://gitlab.com/gnachman/iterm2/-/issues/8128) | High Memory usage since last version |
| [#8117](https://gitlab.com/gnachman/iterm2/-/issues/8117) | High CPU Usage with Custom Scripted Status Bar Component |
| [#8034](https://gitlab.com/gnachman/iterm2/-/issues/8034) | Performance issue after fullscreen |
| [#7992](https://gitlab.com/gnachman/iterm2/-/issues/7992) | Feature Request: Charging wattage in battery status bar component |
| [#7962](https://gitlab.com/gnachman/iterm2/-/issues/7962) | iterm lagging a lot |
| [#7942](https://gitlab.com/gnachman/iterm2/-/issues/7942) | DashTerm2 spawn several git processes and consume lot of CPU |
| [#7929](https://gitlab.com/gnachman/iterm2/-/issues/7929) | When connectivity exists, but DNS fails in the network, iTerm becomes slow and sometimes unresponsiv... |
| [#7876](https://gitlab.com/gnachman/iterm2/-/issues/7876) | Blur performance issue |
| [#7872](https://gitlab.com/gnachman/iterm2/-/issues/7872) | Coprocess via key binding runs slowly |
| [#7856](https://gitlab.com/gnachman/iterm2/-/issues/7856) | Terrible UI performance when working with large terminal windows |
| [#7712](https://gitlab.com/gnachman/iterm2/-/issues/7712) | Excessive Memory Usage (23GB Virtual, 530MB Resident) after 2 months |
| [#7604](https://gitlab.com/gnachman/iterm2/-/issues/7604) | Constant high CPU usage on Mojave when logged into more than one user |
| [#7591](https://gitlab.com/gnachman/iterm2/-/issues/7591) | Poor performance when macbook pro is powered |
| [#7579](https://gitlab.com/gnachman/iterm2/-/issues/7579) | memory leak reproducible |
| [#7509](https://gitlab.com/gnachman/iterm2/-/issues/7509) | Slow start getting the login terminal back |
| [#7468](https://gitlab.com/gnachman/iterm2/-/issues/7468) | Sluggish performance using fullscreen 32 inch 4K monitor |
| [#7424](https://gitlab.com/gnachman/iterm2/-/issues/7424) | Provide option to stop accidental paste of large inputs over slow buffers |
| [#7410](https://gitlab.com/gnachman/iterm2/-/issues/7410) | 100% CPU Utilization |
| [#7362](https://gitlab.com/gnachman/iterm2/-/issues/7362) | 3.2.5 Background 30-75% CPU |
| [#7359](https://gitlab.com/gnachman/iterm2/-/issues/7359) | DashTerm2 100% CPU usage while idle |
| [#7351](https://gitlab.com/gnachman/iterm2/-/issues/7351) | Build 3.2.5 burning too much CPU when there activity in a window |
| [#7333](https://gitlab.com/gnachman/iterm2/-/issues/7333) | Performance Issues |
| [#7304](https://gitlab.com/gnachman/iterm2/-/issues/7304) | Lag Switching Between Tabs And Windows |
| [#7303](https://gitlab.com/gnachman/iterm2/-/issues/7303) | Lagging now (Build 3.2.5) even under single user account |
| [#7246](https://gitlab.com/gnachman/iterm2/-/issues/7246) | DashTerm2 scroll lags significantly when in foreground, but not background |
| [#7219](https://gitlab.com/gnachman/iterm2/-/issues/7219) | mc causes high cpu usage |
| [#7209](https://gitlab.com/gnachman/iterm2/-/issues/7209) | high cpu usage with just printing to the terminal |
| [#7198](https://gitlab.com/gnachman/iterm2/-/issues/7198) | Significant lag after update to 3.2.3 |
| [#7185](https://gitlab.com/gnachman/iterm2/-/issues/7185) | Very laggy experience when using DashTerm2 without full screen |
| [#7150](https://gitlab.com/gnachman/iterm2/-/issues/7150) | 3.2.2 (input) latency on Mojave |
| [#7123](https://gitlab.com/gnachman/iterm2/-/issues/7123) | GPU rendering performance on Mojave in full-screen |
| [#7100](https://gitlab.com/gnachman/iterm2/-/issues/7100) | opening new session extremely slow on Mojave |
| [#7008](https://gitlab.com/gnachman/iterm2/-/issues/7008) | Performance (scrolling in vim) difference between DashTerm2 and kitty |
| [#6939](https://gitlab.com/gnachman/iterm2/-/issues/6939) | DashTerm2 becomes very slow over time on macOS X High Sierra |
| [#6917](https://gitlab.com/gnachman/iterm2/-/issues/6917) | GPU rendering is much slower on my setup |
| [#6902](https://gitlab.com/gnachman/iterm2/-/issues/6902) | Degraded performance on discrete GPU (nvidia) |
| [#6899](https://gitlab.com/gnachman/iterm2/-/issues/6899) | Big Performance Hit For Disabling Title Bar |
| [#6767](https://gitlab.com/gnachman/iterm2/-/issues/6767) | Slow scrolling with GPU rendering |
| [#6721](https://gitlab.com/gnachman/iterm2/-/issues/6721) | Being used under different user accounts on same machine starts lagging badly |
| [#6680](https://gitlab.com/gnachman/iterm2/-/issues/6680) | Add Paste Slowly to the right-click menu |
| [#6647](https://gitlab.com/gnachman/iterm2/-/issues/6647) | Memory Issues |
| [#6586](https://gitlab.com/gnachman/iterm2/-/issues/6586) | Cannot release memory even all windows closed (and buffer cleared) |
| [#6532](https://gitlab.com/gnachman/iterm2/-/issues/6532) | Document impact of triggers on performance (benchmarking) |
| [#6359](https://gitlab.com/gnachman/iterm2/-/issues/6359) | iTerm slows at internal MacBook monitor |
| [#6322](https://gitlab.com/gnachman/iterm2/-/issues/6322) | very slow start when using Dynamic Profile |
| [#6303](https://gitlab.com/gnachman/iterm2/-/issues/6303) | Keypresses lag sometimes |
| [#6263](https://gitlab.com/gnachman/iterm2/-/issues/6263) | scrolling/resizing 24 bit ANSI/VT100 image render is pathologically slow; Unicode graphic drawing ch... |
| [#6220](https://gitlab.com/gnachman/iterm2/-/issues/6220) | Tab bar on the left slows down the rendering |
| [#6101](https://gitlab.com/gnachman/iterm2/-/issues/6101) | Bad drawing performance in 3.1 |
| [#5922](https://gitlab.com/gnachman/iterm2/-/issues/5922) | iterm2 Latency |
| [#5870](https://gitlab.com/gnachman/iterm2/-/issues/5870) | iTerm uses 150 % CPU, only way to stop is restart |
| [#5867](https://gitlab.com/gnachman/iterm2/-/issues/5867) | cpu usage jumps to 99% every few minutes with just one tab |
| [#5761](https://gitlab.com/gnachman/iterm2/-/issues/5761) | iTerm random cpu spikes during idle |
| [#5747](https://gitlab.com/gnachman/iterm2/-/issues/5747) | iTerm is incredibly laggy when I open profile info of a tab |
| [#5731](https://gitlab.com/gnachman/iterm2/-/issues/5731) | Unicode characters rendered much slower than in Terminal.app |
| [#5685](https://gitlab.com/gnachman/iterm2/-/issues/5685) | Possible Memory Leak? |
| [#5656](https://gitlab.com/gnachman/iterm2/-/issues/5656) | Scrolling lag with pwndbg / general scroll performance poor |
| [#5555](https://gitlab.com/gnachman/iterm2/-/issues/5555) | Increased CPU consumption when using TTF fonts |
| [#5442](https://gitlab.com/gnachman/iterm2/-/issues/5442) | Memory leak? |
| [#5380](https://gitlab.com/gnachman/iterm2/-/issues/5380) | macOS sierra Battery draining: com.googlecode.iterm2 consuming to much CPU? |
| [#5369](https://gitlab.com/gnachman/iterm2/-/issues/5369) | v3 is very slow and unresponsive |
| [#5275](https://gitlab.com/gnachman/iterm2/-/issues/5275) | Select all is slow with long history |
| [#5240](https://gitlab.com/gnachman/iterm2/-/issues/5240) | iTerm is frequently reported as an App Using Significant Energy |
| [#5239](https://gitlab.com/gnachman/iterm2/-/issues/5239) | Bad performance with inline gifs |
| [#5111](https://gitlab.com/gnachman/iterm2/-/issues/5111) | CPU utilization periodically spikes to 99/100% |
| [#5024](https://gitlab.com/gnachman/iterm2/-/issues/5024) | Slow edit of command in profile preferences |
| [#4791](https://gitlab.com/gnachman/iterm2/-/issues/4791) | DashTerm2 consumed too much memory - ran out of application memory |
| [#4695](https://gitlab.com/gnachman/iterm2/-/issues/4695) | Improve performance under memory pressure [was: All open terminals slow down if another visible wind... |
| [#4684](https://gitlab.com/gnachman/iterm2/-/issues/4684) | iTerm gets slow when playing a video in a browser |
| [#4611](https://gitlab.com/gnachman/iterm2/-/issues/4611) | cpu usage of iterm3 is high when using polysh |
| [#4524](https://gitlab.com/gnachman/iterm2/-/issues/4524) | iTerm uses 15-20% of CPU when nothing is happening |
| [#4443](https://gitlab.com/gnachman/iterm2/-/issues/4443) | Typing and vim navigation slowdowns moving to version 2.9.20160313 beta from 2.9.20160206 beta |
| [#4203](https://gitlab.com/gnachman/iterm2/-/issues/4203) | Setting Bash prompt PS1 to "(◕‿◕)" makes "ls" noticeably slower |
| [#4102](https://gitlab.com/gnachman/iterm2/-/issues/4102) | Extremely slow iterm2. |
| [#4017](https://gitlab.com/gnachman/iterm2/-/issues/4017) | window resize redraw lags actual resize |
| [#3878](https://gitlab.com/gnachman/iterm2/-/issues/3878) | iTerm 2.9.20151001 is very slow to refresh after clear |
| [#3847](https://gitlab.com/gnachman/iterm2/-/issues/3847) | drawing problems, randomly bugs out and everything gets super slow |
| [#3845](https://gitlab.com/gnachman/iterm2/-/issues/3845) | Laggy performance when several tabs are open using latest beta. |
| [#2373](https://gitlab.com/gnachman/iterm2/-/issues/2373) | Accessibility features cause poor performance [was: Frequent freezing when doing almost anything] |
| [#1273](https://gitlab.com/gnachman/iterm2/-/issues/1273) | Find takes forever (and consumes gigs of memory) |
| [#1082](https://gitlab.com/gnachman/iterm2/-/issues/1082) | Improve find performance |
| [#794](https://gitlab.com/gnachman/iterm2/-/issues/794) | Scrolling in less/more is slow |

---

## Scrollback (P2)

**Count:** 71

| Issue | Title |
|-------|-------|
| [#12642](https://gitlab.com/gnachman/iterm2/-/issues/12642) | Image background artifacts on tabs with unlimited scrollback |
| [#12388](https://gitlab.com/gnachman/iterm2/-/issues/12388) | Ctrl-f search results are missing results depending on where the in the buffer I've scrolled to |
| [#11685](https://gitlab.com/gnachman/iterm2/-/issues/11685) | Window shrinks when scroll bar appears then disappears |
| [#11609](https://gitlab.com/gnachman/iterm2/-/issues/11609) | Focus follows mouse doesn't work when entering window via scroll bar region |
| [#11357](https://gitlab.com/gnachman/iterm2/-/issues/11357) | MacOS modifiers with arrow keys do not work in shell for page/scroll movements |
| [#11282](https://gitlab.com/gnachman/iterm2/-/issues/11282) | Incorrect Scrollback Behavior When Resize |
| [#11246](https://gitlab.com/gnachman/iterm2/-/issues/11246) | Horizontal Scrolling |
| [#11210](https://gitlab.com/gnachman/iterm2/-/issues/11210) | Feature Request: Hide Scrollbar option in Profile |
| [#11201](https://gitlab.com/gnachman/iterm2/-/issues/11201) | Scrollbar oddly dark grey |
| [#11009](https://gitlab.com/gnachman/iterm2/-/issues/11009) | Focus oscillates between split panes due to scroll inertia with "Focus Follows Mouse" enabled |
| [#10944](https://gitlab.com/gnachman/iterm2/-/issues/10944) | Scrollable tabs |
| [#10814](https://gitlab.com/gnachman/iterm2/-/issues/10814) | Iterm2 3.5.0beta10 Add Option To Disable Horizontal Scrolling |
| [#10598](https://gitlab.com/gnachman/iterm2/-/issues/10598) | White Scrollbar color in second tab (with black design) |
| [#10522](https://gitlab.com/gnachman/iterm2/-/issues/10522) | api for scroll bar? |
| [#10407](https://gitlab.com/gnachman/iterm2/-/issues/10407) | Scrollback buffer broken - or am I? |
| [#10235](https://gitlab.com/gnachman/iterm2/-/issues/10235) | Mouse wheel scroll so fast |
| [#9998](https://gitlab.com/gnachman/iterm2/-/issues/9998) | Scrollback doesn't scroll back |
| [#9954](https://gitlab.com/gnachman/iterm2/-/issues/9954) | Notice that some of the lines in the scroll buffer are replaced with "XXXXX..." |
| [#9788](https://gitlab.com/gnachman/iterm2/-/issues/9788) | Screen Tearing / Flickering when scrolling in vim |
| [#9619](https://gitlab.com/gnachman/iterm2/-/issues/9619) | Add "Respect mouse acceleration" with mouse scroll reporting |
| [#9597](https://gitlab.com/gnachman/iterm2/-/issues/9597) | Scrolling using MX Master 3 mouse scroll wheel no longer works |
| [#9451](https://gitlab.com/gnachman/iterm2/-/issues/9451) | [question] Is there a way to reduce scroll sensitivity? |
| [#9448](https://gitlab.com/gnachman/iterm2/-/issues/9448) | Avoid processing scrollback on window resize? |
| [#9442](https://gitlab.com/gnachman/iterm2/-/issues/9442) | iTerm 2 reporting a wrong window size depending on macOS "Show scroll bars" setting |
| [#9131](https://gitlab.com/gnachman/iterm2/-/issues/9131) | Getting into strange "modes" (bad scrolling and pasting) |
| [#8828](https://gitlab.com/gnachman/iterm2/-/issues/8828) | Mouse clicks and scroll wheel input characters into terminal window |
| [#8824](https://gitlab.com/gnachman/iterm2/-/issues/8824) | Significant grey box rendering issues when text output causes scrolling. |
| [#8803](https://gitlab.com/gnachman/iterm2/-/issues/8803) | MacOs Mojave Build 3.3.9 - Unlimited Scrollback not working |
| [#8791](https://gitlab.com/gnachman/iterm2/-/issues/8791) | Add "Scroll to Top" Parameter for triggers. |
| [#8576](https://gitlab.com/gnachman/iterm2/-/issues/8576) | Scrolling sometimes triggers selection |
| [#8461](https://gitlab.com/gnachman/iterm2/-/issues/8461) | Scrolling breaks when prompt is updated by external process (i.e. ZSH theme) |
| [#8421](https://gitlab.com/gnachman/iterm2/-/issues/8421) | [Feature Request] Scroll two panes simultaneously |
| [#8389](https://gitlab.com/gnachman/iterm2/-/issues/8389) | Smooth scrolling (DECSCLM) |
| [#8385](https://gitlab.com/gnachman/iterm2/-/issues/8385) | Mouse scroll sensitivity too low; "Scroll on any scroll wheel movement" setting has no effect |
| [#8373](https://gitlab.com/gnachman/iterm2/-/issues/8373) | Scrolling blocks alt/tab on mac. |
| [#8312](https://gitlab.com/gnachman/iterm2/-/issues/8312) | Scroll down fails (jumps) after navigating to previous mark |
| [#8237](https://gitlab.com/gnachman/iterm2/-/issues/8237) | Scrollback continuously scrolls up and down after the scrollback limit is reached |
| [#7847](https://gitlab.com/gnachman/iterm2/-/issues/7847) | Keep unlimited scrollback from eating all the ram |
| [#7674](https://gitlab.com/gnachman/iterm2/-/issues/7674) | [Feature request] Options to send specific text when scrolling horizontally |
| [#7673](https://gitlab.com/gnachman/iterm2/-/issues/7673) | Cannot scroll up or down. It cycles through recent commands instead. |
| [#7620](https://gitlab.com/gnachman/iterm2/-/issues/7620) | Update causes lost state of windows / tabs / session information (scrollback buffer) |
| [#7491](https://gitlab.com/gnachman/iterm2/-/issues/7491) | Notification bubble number increasing during unfocused scrolling in a pager |
| [#7338](https://gitlab.com/gnachman/iterm2/-/issues/7338) | horizontal cursor movement on command-scrollwheel? |
| [#7324](https://gitlab.com/gnachman/iterm2/-/issues/7324) | Mouse Scroll Wheel of Logical MX Master 2S is not available |
| [#7070](https://gitlab.com/gnachman/iterm2/-/issues/7070) | scroll not always working |
| [#6974](https://gitlab.com/gnachman/iterm2/-/issues/6974) | Vertical scrollbar is hidden when it shouldn't be |
| [#6824](https://gitlab.com/gnachman/iterm2/-/issues/6824) | Disable shift + arrows scroll |
| [#6705](https://gitlab.com/gnachman/iterm2/-/issues/6705) | Garbled output when scrolling in ncurser application |
| [#6678](https://gitlab.com/gnachman/iterm2/-/issues/6678) | scroll wheel doesn't send arrow keys when git's pager config pipes to less |
| [#6531](https://gitlab.com/gnachman/iterm2/-/issues/6531) | Trackpad Scrolling fails with Experimental Metal Renderer |
| [#6356](https://gitlab.com/gnachman/iterm2/-/issues/6356) | mouse scroll on less like programs (e.g. man) don't work anymore |
| [#6325](https://gitlab.com/gnachman/iterm2/-/issues/6325) | Mouse scroll stop working and instead scroll commands in a shell |
| [#6103](https://gitlab.com/gnachman/iterm2/-/issues/6103) | Feature request: When using dark theme, have dark scrollbar |
| [#6040](https://gitlab.com/gnachman/iterm2/-/issues/6040) | Programmatically scroll to bottom of the terminal window |
| [#6008](https://gitlab.com/gnachman/iterm2/-/issues/6008) | "Select All" with ⌘a on a mac with lots of scrollback makes DashTerm2 unresponsive. |
| [#5863](https://gitlab.com/gnachman/iterm2/-/issues/5863) | Mouse scroll wheel seems to be ignored. |
| [#5565](https://gitlab.com/gnachman/iterm2/-/issues/5565) | Scrolling and Selecting Text Interpreted as Key Presses |
| [#5206](https://gitlab.com/gnachman/iterm2/-/issues/5206) | scrolling not working on Sierra |
| [#5153](https://gitlab.com/gnachman/iterm2/-/issues/5153) | scrolling in Vim feels slightly sluggish |
| [#5085](https://gitlab.com/gnachman/iterm2/-/issues/5085) | [Feature request] Add profile preference to reverse scroll wheel sends arrow keys direction |
| [#5039](https://gitlab.com/gnachman/iterm2/-/issues/5039) | xterm reportedly preserves selection on keydown, moves it when scroll region scrolls; copy this feat... |
| [#4995](https://gitlab.com/gnachman/iterm2/-/issues/4995) | Unable to scroll less/more with mouse wheel |
| [#4557](https://gitlab.com/gnachman/iterm2/-/issues/4557) | Don't just use all RAM and die horribly with unlimited scrollback. [was: Large scrollback buffers ca... |
| [#4430](https://gitlab.com/gnachman/iterm2/-/issues/4430) | zoom window with mouse scroll wheel |
| [#4213](https://gitlab.com/gnachman/iterm2/-/issues/4213) | Move primary buffer into scrollback when entering alternate screen mode |
| [#4186](https://gitlab.com/gnachman/iterm2/-/issues/4186) | Request: scrollbar on left hand side |
| [#4149](https://gitlab.com/gnachman/iterm2/-/issues/4149) | Using VI in an DashTerm2 window seems to permanent mess up the window size and scrolling |
| [#3505](https://gitlab.com/gnachman/iterm2/-/issues/3505) | Export the whole scrollback buffer as a PNG |
| [#3209](https://gitlab.com/gnachman/iterm2/-/issues/3209) | Smooth scrolling |
| [#2961](https://gitlab.com/gnachman/iterm2/-/issues/2961) | blocking search for large scrollback |
| [#2695](https://gitlab.com/gnachman/iterm2/-/issues/2695) | Add option to enable/disable scroll buffer repositioning when text is pasted |

---

## Font and Rendering (P2)

**Count:** 189

| Issue | Title |
|-------|-------|
| [#12657](https://gitlab.com/gnachman/iterm2/-/issues/12657) | Box drawing characters are disjointed on non-retina display |
| [#12583](https://gitlab.com/gnachman/iterm2/-/issues/12583) | wrong font size, default profile size did not affect |
| [#12507](https://gitlab.com/gnachman/iterm2/-/issues/12507) | Send control characters in "Send Text" in "Smart Selection" under Actions |
| [#12464](https://gitlab.com/gnachman/iterm2/-/issues/12464) | GPU renderer puts unexpected gaps in underlines |
| [#12461](https://gitlab.com/gnachman/iterm2/-/issues/12461) | Skipping intermediate draws while autorepeating down-arrow in emacs |
| [#12386](https://gitlab.com/gnachman/iterm2/-/issues/12386) | Unicode characterd added in or after version 5.2 are escaped when pasted into terminal |
| [#12331](https://gitlab.com/gnachman/iterm2/-/issues/12331) | Add character pacing/pauses to Send Snippet functionality |
| [#12320](https://gitlab.com/gnachman/iterm2/-/issues/12320) | Terminal corruption and character dropping with oh-my-zsh cloud theme in DashTerm2 3.5.14 |
| [#12231](https://gitlab.com/gnachman/iterm2/-/issues/12231) | some ligatures will not show even if it is enabled. |
| [#12230](https://gitlab.com/gnachman/iterm2/-/issues/12230) | Issue with a nerd font |
| [#11917](https://gitlab.com/gnachman/iterm2/-/issues/11917) | Improper Handling of ASCII Art in DashTerm2 Due to Excessive Line Spacing Between Characters |
| [#11898](https://gitlab.com/gnachman/iterm2/-/issues/11898) | Box characters incorrectly drawn |
| [#11843](https://gitlab.com/gnachman/iterm2/-/issues/11843) | Meslo Nerd Font patched for Powerlevel10k arrows render 1 pixel lower |
| [#11826](https://gitlab.com/gnachman/iterm2/-/issues/11826) | first character gets missed when typing a command |
| [#11812](https://gitlab.com/gnachman/iterm2/-/issues/11812) | Misrender of the golfer grapheme |
| [#11675](https://gitlab.com/gnachman/iterm2/-/issues/11675) | Custom font for tab names |
| [#11617](https://gitlab.com/gnachman/iterm2/-/issues/11617) | Issue with Powerline rendering without ligatures |
| [#11577](https://gitlab.com/gnachman/iterm2/-/issues/11577) | iTerm's custom box-drawing has started having vertical gaps in v3.5 |
| [#11560](https://gitlab.com/gnachman/iterm2/-/issues/11560) | latest iterm2 Build 3.5.0 - issues with rendering background image with GPU rendering enabled on mac... |
| [#11514](https://gitlab.com/gnachman/iterm2/-/issues/11514) | Screen rendering is broken in v3.5.0 |
| [#11505](https://gitlab.com/gnachman/iterm2/-/issues/11505) | Strange newline character appeared after updating |
| [#11494](https://gitlab.com/gnachman/iterm2/-/issues/11494) | Non-English characters broke in vim in iTerm 3.5.0 |
| [#11408](https://gitlab.com/gnachman/iterm2/-/issues/11408) | Chinese Character in directory name is broken when using Fish with the function "working directory r... |
| [#11386](https://gitlab.com/gnachman/iterm2/-/issues/11386) | Underline is weirdly rendered |
| [#11323](https://gitlab.com/gnachman/iterm2/-/issues/11323) | Escape or quote shell characters by default when pasting a path-like string |
| [#11231](https://gitlab.com/gnachman/iterm2/-/issues/11231) | unicode U+21B5 width interpreted incorrectly |
| [#11189](https://gitlab.com/gnachman/iterm2/-/issues/11189) | Snippet font - "smart" quotes are a problem |
| [#11178](https://gitlab.com/gnachman/iterm2/-/issues/11178) | Provide scripting variable that shows current font |
| [#11158](https://gitlab.com/gnachman/iterm2/-/issues/11158) | Synchronized font preference gets overwritten |
| [#11118](https://gitlab.com/gnachman/iterm2/-/issues/11118) | Ligatures broken in Beta 13 |
| [#11105](https://gitlab.com/gnachman/iterm2/-/issues/11105) | iTerm doesn't seem to respect ligature customizations in Iosefka. |
| [#11058](https://gitlab.com/gnachman/iterm2/-/issues/11058) | Tabs don't render correctly after entering fullscreen |
| [#11005](https://gitlab.com/gnachman/iterm2/-/issues/11005) | Central place to control all font sizing options |
| [#11002](https://gitlab.com/gnachman/iterm2/-/issues/11002) | Status bar component width cannot be set to ∞ (infinity) without resorting to the Character Viewer (... |
| [#10888](https://gitlab.com/gnachman/iterm2/-/issues/10888) | MarkdownPotholeRenderer - Compiled module was created by a different version of the compiler |
| [#10849](https://gitlab.com/gnachman/iterm2/-/issues/10849) | Pasting a string with special characters into iTerm's CLI results in escaped slashes |
| [#10845](https://gitlab.com/gnachman/iterm2/-/issues/10845) | Dimming should apply to emoji |
| [#10839](https://gitlab.com/gnachman/iterm2/-/issues/10839) | Imgcat not rendering .eps file |
| [#10818](https://gitlab.com/gnachman/iterm2/-/issues/10818) | Any way to add "count of selected characters" to Iterm2 Status Bar? |
| [#10817](https://gitlab.com/gnachman/iterm2/-/issues/10817) | When I delete the input character, it reappears. |
| [#10790](https://gitlab.com/gnachman/iterm2/-/issues/10790) | "Draw bold text in bold font" seems to use an incorrect version (variant) of the font |
| [#10777](https://gitlab.com/gnachman/iterm2/-/issues/10777) | Top and Bottom Margins are discolored / render weird |
| [#10680](https://gitlab.com/gnachman/iterm2/-/issues/10680) | Fixed font size for badge |
| [#10555](https://gitlab.com/gnachman/iterm2/-/issues/10555) | Character variants support for fonts |
| [#10554](https://gitlab.com/gnachman/iterm2/-/issues/10554) | Inline images render blurry on non-retina monitors when retina laptop is closed |
| [#10472](https://gitlab.com/gnachman/iterm2/-/issues/10472) | feature request: ability to switch fonts based on active monitor |
| [#10459](https://gitlab.com/gnachman/iterm2/-/issues/10459) | Unicode box characters not printing properly (python + curses) |
| [#10444](https://gitlab.com/gnachman/iterm2/-/issues/10444) | Multiple combining characters are rendered incorrectly |
| [#10355](https://gitlab.com/gnachman/iterm2/-/issues/10355) | setting font via cli |
| [#10298](https://gitlab.com/gnachman/iterm2/-/issues/10298) | Native macOS terminal not work in parallel to iterm2 and also font Render issue between native termi... |
| [#10285](https://gitlab.com/gnachman/iterm2/-/issues/10285) | Xterm Double High Characters |
| [#10225](https://gitlab.com/gnachman/iterm2/-/issues/10225) | Interpolated string status bar component disappears when value contains non-ascii characters |
| [#10215](https://gitlab.com/gnachman/iterm2/-/issues/10215) | When the option is turned on, the remote's screen may not be drawn at all. |
| [#10214](https://gitlab.com/gnachman/iterm2/-/issues/10214) | Italic Text Renders In a Lighter Weight Than Non-Italic Text |
| [#10190](https://gitlab.com/gnachman/iterm2/-/issues/10190) | iterm swallows character when typing in a split window |
| [#10133](https://gitlab.com/gnachman/iterm2/-/issues/10133) | Music note icon of Hack Nerd Font is not displayed |
| [#9942](https://gitlab.com/gnachman/iterm2/-/issues/9942) | Regarding of Unicode setting as default (Other language; Korean) |
| [#9828](https://gitlab.com/gnachman/iterm2/-/issues/9828) | Voiceover does not speak deleted characters |
| [#9778](https://gitlab.com/gnachman/iterm2/-/issues/9778) | Date time not fits with a bigger font |
| [#9586](https://gitlab.com/gnachman/iterm2/-/issues/9586) | Toolbelt font size |
| [#9563](https://gitlab.com/gnachman/iterm2/-/issues/9563) | Widget to inform when you've dragged the mouse across enough characters to satisfy target count |
| [#9509](https://gitlab.com/gnachman/iterm2/-/issues/9509) | Odd breakage of Crtl-Cmd-Space unicode input |
| [#9484](https://gitlab.com/gnachman/iterm2/-/issues/9484) | 3.4.4 always outputs "does not have trailing newline" character, even after trailing newlines. |
| [#9279](https://gitlab.com/gnachman/iterm2/-/issues/9279) | Catalina; 3.4.1; Ctrl-[ in Vim then toggle case of character in insert mode |
| [#9235](https://gitlab.com/gnachman/iterm2/-/issues/9235) | [Feature Request] Use custom font and size when pasting with color and style |
| [#9209](https://gitlab.com/gnachman/iterm2/-/issues/9209) | No subpixel rendering on Big Sur |
| [#9123](https://gitlab.com/gnachman/iterm2/-/issues/9123) | Font padding with non ascii text |
| [#9073](https://gitlab.com/gnachman/iterm2/-/issues/9073) | Multiple non-ASCII fonts |
| [#9028](https://gitlab.com/gnachman/iterm2/-/issues/9028) | Subtle text corruption, combining glyphs incorrectly but select-copy gets the right text ("ø " inste... |
| [#8971](https://gitlab.com/gnachman/iterm2/-/issues/8971) | FiraCode Ligatures Not Displaying |
| [#8898](https://gitlab.com/gnachman/iterm2/-/issues/8898) | imgcat show strange characters |
| [#8813](https://gitlab.com/gnachman/iterm2/-/issues/8813) | Font settings for non-ASCII fonts are not working in mac os catalina |
| [#8774](https://gitlab.com/gnachman/iterm2/-/issues/8774) | Switching between tabs with different status bar visibility causes vim to render wrong lines |
| [#8735](https://gitlab.com/gnachman/iterm2/-/issues/8735) | Unicode symbols cannot be displayed properly in 1 character width |
| [#8727](https://gitlab.com/gnachman/iterm2/-/issues/8727) | zsh-syntax-highlighting plug in for on-my-zsh breaks some ligatures. |
| [#8726](https://gitlab.com/gnachman/iterm2/-/issues/8726) | Hand-drawn bitmap font reverse (as in less prompt with TERM=screen) is illegible |
| [#8653](https://gitlab.com/gnachman/iterm2/-/issues/8653) | How to prevent fast redrawing? |
| [#8609](https://gitlab.com/gnachman/iterm2/-/issues/8609) | Tab bar font and colours (Feature Request) |
| [#8562](https://gitlab.com/gnachman/iterm2/-/issues/8562) | [Accessibility] VoiceOver does not read deleted characters |
| [#8499](https://gitlab.com/gnachman/iterm2/-/issues/8499) | GPU Rendering enabled in macOS Catalina causes text distortion |
| [#8466](https://gitlab.com/gnachman/iterm2/-/issues/8466) | Key sequence not sent to the programs - CMD+SHIFT+CTRL+<character> |
| [#8457](https://gitlab.com/gnachman/iterm2/-/issues/8457) | Feature request: support font shaping or Numderline equivalent |
| [#8318](https://gitlab.com/gnachman/iterm2/-/issues/8318) | When "zooming" in all fonts should increase |
| [#8309](https://gitlab.com/gnachman/iterm2/-/issues/8309) | Bug: Alternate styles for fonts not supported |
| [#8261](https://gitlab.com/gnachman/iterm2/-/issues/8261) | iterm unable to print ⚑ symbol using powerline fonts |
| [#8254](https://gitlab.com/gnachman/iterm2/-/issues/8254) | Warn potential contributors about the binary BetterFontPicker.framework in git |
| [#8120](https://gitlab.com/gnachman/iterm2/-/issues/8120) | FiraCode ligature for "->>" doesn't appear to display properly |
| [#8104](https://gitlab.com/gnachman/iterm2/-/issues/8104) | Unset the locale, the zsh input prompt will have more characters and cannot be rolled back. |
| [#8064](https://gitlab.com/gnachman/iterm2/-/issues/8064) | Prompts get redrawn after resizing the window |
| [#7991](https://gitlab.com/gnachman/iterm2/-/issues/7991) | Feature request: add Nerd Font characters to built-in Powerline characters |
| [#7938](https://gitlab.com/gnachman/iterm2/-/issues/7938) | Emoji warning sign U+26A0 is double width but displays as single width |
| [#7901](https://gitlab.com/gnachman/iterm2/-/issues/7901) | Unicode width not calculated correctly sometmes |
| [#7886](https://gitlab.com/gnachman/iterm2/-/issues/7886) | Feature request: high color depth rendering |
| [#7854](https://gitlab.com/gnachman/iterm2/-/issues/7854) | Does the option "Draw bold text in bright colors" removed? |
| [#7738](https://gitlab.com/gnachman/iterm2/-/issues/7738) | Status bar draws a black border with custom color |
| [#7663](https://gitlab.com/gnachman/iterm2/-/issues/7663) | Support PCF or BDF bitmapped fonts directly even though OS/X doesn't |
| [#7496](https://gitlab.com/gnachman/iterm2/-/issues/7496) | Inconsistent Titlebar Bell Glyph Behavior / Bell Glyph in DashTerm2 Title Bar Won't Go Away |
| [#7494](https://gitlab.com/gnachman/iterm2/-/issues/7494) | Multiple combining marks not rendered correctly |
| [#7463](https://gitlab.com/gnachman/iterm2/-/issues/7463) | Log has too many characters |
| [#7393](https://gitlab.com/gnachman/iterm2/-/issues/7393) | Font issue when moving terminal window between displays quickly. |
| [#7358](https://gitlab.com/gnachman/iterm2/-/issues/7358) | Rendering bug introduced in 3.2.4 |
| [#7291](https://gitlab.com/gnachman/iterm2/-/issues/7291) | Full size window goes oversized with font size increase |
| [#7284](https://gitlab.com/gnachman/iterm2/-/issues/7284) | Displaying Chinese characters in monospace? |
| [#7239](https://gitlab.com/gnachman/iterm2/-/issues/7239) | Console confused by emojis with variant selector 0xFE0F |
| [#7210](https://gitlab.com/gnachman/iterm2/-/issues/7210) | [feature] Touch bar fontsize |
| [#7202](https://gitlab.com/gnachman/iterm2/-/issues/7202) | Font super thin since 3.2.3 |
| [#7104](https://gitlab.com/gnachman/iterm2/-/issues/7104) | cut and paste adds characters before and after text |
| [#7090](https://gitlab.com/gnachman/iterm2/-/issues/7090) | Black area when windows bigger than 700 characters horizontal. |
| [#7055](https://gitlab.com/gnachman/iterm2/-/issues/7055) | Feature suggestion: automatically scale font size when leaving Full Screen mode |
| [#7032](https://gitlab.com/gnachman/iterm2/-/issues/7032) | Degraded font smoothing |
| [#7011](https://gitlab.com/gnachman/iterm2/-/issues/7011) | Fonts are bold unless resizing window |
| [#7000](https://gitlab.com/gnachman/iterm2/-/issues/7000) | Unicode soft hyphens and homeographs render differently when active vs. when idle |
| [#6926](https://gitlab.com/gnachman/iterm2/-/issues/6926) | text rendering issues for terminal emacs in 3.2.0 |
| [#6889](https://gitlab.com/gnachman/iterm2/-/issues/6889) | Transparent background with Metal renderer in Mojave |
| [#6854](https://gitlab.com/gnachman/iterm2/-/issues/6854) | Existing Profiles Disable Metal Renderer After Upgrade |
| [#6849](https://gitlab.com/gnachman/iterm2/-/issues/6849) | semantic history fails to identify unicode python strings (u prefix) |
| [#6827](https://gitlab.com/gnachman/iterm2/-/issues/6827) | Handling Colors when using the Metal renderer |
| [#6803](https://gitlab.com/gnachman/iterm2/-/issues/6803) | Bold characters missing when antialiasing is turned off |
| [#6708](https://gitlab.com/gnachman/iterm2/-/issues/6708) | Bitmap font on Retina screen? |
| [#6674](https://gitlab.com/gnachman/iterm2/-/issues/6674) | Typing a character creates a new line |
| [#6587](https://gitlab.com/gnachman/iterm2/-/issues/6587) | Metal renderer forces discrete GPU use |
| [#6558](https://gitlab.com/gnachman/iterm2/-/issues/6558) | Comments re new metal renderer |
| [#6449](https://gitlab.com/gnachman/iterm2/-/issues/6449) | Color rendering issues |
| [#6421](https://gitlab.com/gnachman/iterm2/-/issues/6421) | Can't input specific character anymore |
| [#6337](https://gitlab.com/gnachman/iterm2/-/issues/6337) | emoji width is wrongly displayed |
| [#6313](https://gitlab.com/gnachman/iterm2/-/issues/6313) | Feature request: Unable to dynamically increase and reduce fonts as in TextEdit |
| [#6311](https://gitlab.com/gnachman/iterm2/-/issues/6311) | Powerline fonts are misaligned |
| [#6283](https://gitlab.com/gnachman/iterm2/-/issues/6283) | Opentype Font Features Support |
| [#6267](https://gitlab.com/gnachman/iterm2/-/issues/6267) | Suggestion: Allow selective application of typeface ligatures via regex |
| [#6249](https://gitlab.com/gnachman/iterm2/-/issues/6249) | "htop" graphs do not render properly in DashTerm2 |
| [#6243](https://gitlab.com/gnachman/iterm2/-/issues/6243) | Using emoji in prompt causes erroneous characters to appear when ctrl+u is used |
| [#6238](https://gitlab.com/gnachman/iterm2/-/issues/6238) | Font bug introduced in 3.1 |
| [#6226](https://gitlab.com/gnachman/iterm2/-/issues/6226) | Hotkey Window has incorrect font size and background color upon first opening |
| [#6213](https://gitlab.com/gnachman/iterm2/-/issues/6213) | Feature request: allow choosing what would be bold version of glyphs |
| [#6175](https://gitlab.com/gnachman/iterm2/-/issues/6175) | Use typeface provided, or DashTerm2 provided box drawing characters based on user preference setting. |
| [#6130](https://gitlab.com/gnachman/iterm2/-/issues/6130) | Emoji in PS1 causes stray character to appear when navigating bash history |
| [#6036](https://gitlab.com/gnachman/iterm2/-/issues/6036) | pasting of text including tabs and preserving the tabs (in dialog) results in characters being lost ... |
| [#5994](https://gitlab.com/gnachman/iterm2/-/issues/5994) | Characters are not appearing while typing |
| [#5943](https://gitlab.com/gnachman/iterm2/-/issues/5943) | Doesn't respond to MacOS "Emojis & Symbols" menu. |
| [#5940](https://gitlab.com/gnachman/iterm2/-/issues/5940) | Fira Code ligatures works, but Hasklig ligatures doesn't work |
| [#5918](https://gitlab.com/gnachman/iterm2/-/issues/5918) | alt + 9 key not sending character |
| [#5879](https://gitlab.com/gnachman/iterm2/-/issues/5879) | When resizing font with Cmd-+/-, allow option for all tabs (present and future) |
| [#5806](https://gitlab.com/gnachman/iterm2/-/issues/5806) | Italics drawn in lighter weight for certain fonts |
| [#5755](https://gitlab.com/gnachman/iterm2/-/issues/5755) | Visual bugs after telnet mapscii.me (vim becomes buggy, exiting vim doesn't erase characters, etc.) |
| [#5733](https://gitlab.com/gnachman/iterm2/-/issues/5733) | Copy and paste of multibyte character and combining accent converts character |
| [#5688](https://gitlab.com/gnachman/iterm2/-/issues/5688) | Feature: offer a "wrap lines at N characters" option in Advanced Paste |
| [#5675](https://gitlab.com/gnachman/iterm2/-/issues/5675) | Line drawing characters rendered badly with Inconsolata Bold font on OSX |
| [#5674](https://gitlab.com/gnachman/iterm2/-/issues/5674) | Unable to completely erase line of text that is 10 characters or longer |
| [#5621](https://gitlab.com/gnachman/iterm2/-/issues/5621) | Setting a font family/size will not stick |
| [#5550](https://gitlab.com/gnachman/iterm2/-/issues/5550) | Font Render Issues |
| [#5534](https://gitlab.com/gnachman/iterm2/-/issues/5534) | Render the selected match from search more vividly |
| [#5523](https://gitlab.com/gnachman/iterm2/-/issues/5523) | [FEATURE REQUEST] Alternate font option for non-character codepoints |
| [#5495](https://gitlab.com/gnachman/iterm2/-/issues/5495) | Split pane + font resize => unpleasant window resizes |
| [#5456](https://gitlab.com/gnachman/iterm2/-/issues/5456) | XTERM 256 color codes not applied or applied incorrectly to font glyphs |
| [#5342](https://gitlab.com/gnachman/iterm2/-/issues/5342) | Font and colors are rendered differently in iTerm and Terminal |
| [#5324](https://gitlab.com/gnachman/iterm2/-/issues/5324) | problems with accented characters |
| [#5314](https://gitlab.com/gnachman/iterm2/-/issues/5314) | Can't select top left characters with small fonts in full screen mode |
| [#5220](https://gitlab.com/gnachman/iterm2/-/issues/5220) | Dropping lots of files on DashTerm2 takes minutes to render |
| [#5074](https://gitlab.com/gnachman/iterm2/-/issues/5074) | Highlighting doesn't invert entire character when vertical line spacing less than 100% |
| [#4982](https://gitlab.com/gnachman/iterm2/-/issues/4982) | Trigger on and capture non-printing characters |
| [#4938](https://gitlab.com/gnachman/iterm2/-/issues/4938) | iterm2 can't show italic font in code, but macvim gui can do that |
| [#4934](https://gitlab.com/gnachman/iterm2/-/issues/4934) | Ligatures deactivate in unfocused split panes |
| [#4702](https://gitlab.com/gnachman/iterm2/-/issues/4702) | Double-width characters do not display properly in iTerm 3 |
| [#4650](https://gitlab.com/gnachman/iterm2/-/issues/4650) | Enhancement: Add font and other profile information to AppleScript output |
| [#4572](https://gitlab.com/gnachman/iterm2/-/issues/4572) | cannot specify custom font size |
| [#4530](https://gitlab.com/gnachman/iterm2/-/issues/4530) | Feature Request: Separate font settings for specific Unicode ranges |
| [#4501](https://gitlab.com/gnachman/iterm2/-/issues/4501) | Corrupts unicode characters on wrap in vertical split |
| [#4318](https://gitlab.com/gnachman/iterm2/-/issues/4318) | Wrong character spacing for different widths of Input Mono font |
| [#4199](https://gitlab.com/gnachman/iterm2/-/issues/4199) | DashTerm2 is removing random characters from large pastes |
| [#4074](https://gitlab.com/gnachman/iterm2/-/issues/4074) | Color hex values rendered incorrectly |
| [#4072](https://gitlab.com/gnachman/iterm2/-/issues/4072) | Font line height versus character height incorrect |
| [#4000](https://gitlab.com/gnachman/iterm2/-/issues/4000) | Tabs in files generate spurious character sequence when logging output to spool file |
| [#3857](https://gitlab.com/gnachman/iterm2/-/issues/3857) | Feature request: Remaining font effects |
| [#3615](https://gitlab.com/gnachman/iterm2/-/issues/3615) | Hard-coded double-width character table is not flexible enough for different fonts. |
| [#3508](https://gitlab.com/gnachman/iterm2/-/issues/3508) | New configuration settings: badge location, font and size. |
| [#3455](https://gitlab.com/gnachman/iterm2/-/issues/3455) | Move "characters considered part of a word for selection" to be next to "double click performs smart... |
| [#3403](https://gitlab.com/gnachman/iterm2/-/issues/3403) | Allow users config font fallback |
| [#3384](https://gitlab.com/gnachman/iterm2/-/issues/3384) | Apple Emoji Font overrides glyphs |
| [#3227](https://gitlab.com/gnachman/iterm2/-/issues/3227) | Unicode characters not displayed correct |
| [#3063](https://gitlab.com/gnachman/iterm2/-/issues/3063) | Filenames with Korean characters lead to crazy termimal display and cursor movement (due to HFS+ re-... |
| [#3052](https://gitlab.com/gnachman/iterm2/-/issues/3052) | Handle U+200B correctly in the presense of combining characters |
| [#2688](https://gitlab.com/gnachman/iterm2/-/issues/2688) | Add a per-profile preference to disable character set switching. |
| [#2372](https://gitlab.com/gnachman/iterm2/-/issues/2372) | Specify bold font |
| [#1650](https://gitlab.com/gnachman/iterm2/-/issues/1650) | Different font configuration for full screen mode |
| [#1607](https://gitlab.com/gnachman/iterm2/-/issues/1607) | Wrong default character spacing |
| [#1578](https://gitlab.com/gnachman/iterm2/-/issues/1578) | print font size and print selection |
| [#1533](https://gitlab.com/gnachman/iterm2/-/issues/1533) | Check if selection has printable characters before copying |
| [#1340](https://gitlab.com/gnachman/iterm2/-/issues/1340) | Show numeric character spacing |
| [#1002](https://gitlab.com/gnachman/iterm2/-/issues/1002) | Req: a "Scrambler" mode for drawn text. |

---

## Window/Tab/Pane (P2)

**Count:** 667

| Issue | Title |
|-------|-------|
| [#12656](https://gitlab.com/gnachman/iterm2/-/issues/12656) | Window title doesn't display in the tab bar by default. |
| [#12627](https://gitlab.com/gnachman/iterm2/-/issues/12627) | Toggle broadcast input shotcut moves the window main display after 3.6.6 upgrade |
| [#12623](https://gitlab.com/gnachman/iterm2/-/issues/12623) | [BUG][UI] Tab Bar (Sometimes) Overlaps With Terminal Contents In Fullscreen Mode |
| [#12621](https://gitlab.com/gnachman/iterm2/-/issues/12621) | Terminal window resizes when password window opens |
| [#12608](https://gitlab.com/gnachman/iterm2/-/issues/12608) | Add an empty space between the built-in actions for the window and the tabs when stretched |
| [#12602](https://gitlab.com/gnachman/iterm2/-/issues/12602) | New window placement places window at unexpected location |
| [#12570](https://gitlab.com/gnachman/iterm2/-/issues/12570) | Open Link prompt blocks all input to global hotkey window, but is underneath global hotkey window |
| [#12563](https://gitlab.com/gnachman/iterm2/-/issues/12563) | Can’t remember the window position |
| [#12554](https://gitlab.com/gnachman/iterm2/-/issues/12554) | Black bar instead of tabs in fullscreen |
| [#12553](https://gitlab.com/gnachman/iterm2/-/issues/12553) | An outline for the active tab in full screen when multiple tabs are present |
| [#12550](https://gitlab.com/gnachman/iterm2/-/issues/12550) | iTerm remembers a window location partly off screen |
| [#12505](https://gitlab.com/gnachman/iterm2/-/issues/12505) | Hotkey windows do not work in v3.6.2 (re-opened issue) |
| [#12500](https://gitlab.com/gnachman/iterm2/-/issues/12500) | Hotkey window is overriding "remember size of previously closed windows" |
| [#12498](https://gitlab.com/gnachman/iterm2/-/issues/12498) | Improve tab visibility on new Tahoe version |
| [#12487](https://gitlab.com/gnachman/iterm2/-/issues/12487) | Pane Title Display Issue |
| [#12472](https://gitlab.com/gnachman/iterm2/-/issues/12472) | Window title no longe centred |
| [#12453](https://gitlab.com/gnachman/iterm2/-/issues/12453) | Configuration for inactive tab color |
| [#12426](https://gitlab.com/gnachman/iterm2/-/issues/12426) | Profile window dimensions being ignored |
| [#12420](https://gitlab.com/gnachman/iterm2/-/issues/12420) | Confirm Multi-Line Paste pops Triggers window |
| [#12399](https://gitlab.com/gnachman/iterm2/-/issues/12399) | Support Linear Gradient(s) as Tab Colors |
| [#12396](https://gitlab.com/gnachman/iterm2/-/issues/12396) | Add a control window for the 'Broadcast input' feature |
| [#12389](https://gitlab.com/gnachman/iterm2/-/issues/12389) | Select pane shortcut does not fully focus pane while maximized |
| [#12340](https://gitlab.com/gnachman/iterm2/-/issues/12340) | New Window shell closes immediately on Tahoe 26.0 beta 2 |
| [#12329](https://gitlab.com/gnachman/iterm2/-/issues/12329) | Top part of the terminal cannot be selected on the 3rd tab |
| [#12312](https://gitlab.com/gnachman/iterm2/-/issues/12312) | Double clicking top of window doesn't enlarge window when using minimal theme |
| [#12289](https://gitlab.com/gnachman/iterm2/-/issues/12289) | Hotkey for "Restore Window Arrangement as Tabs"? |
| [#12265](https://gitlab.com/gnachman/iterm2/-/issues/12265) | Password manager triggered when window shrinks |
| [#12247](https://gitlab.com/gnachman/iterm2/-/issues/12247) | $ITERM_SESSION_ID contains duplicate pane ids after close pane/create pane |
| [#12238](https://gitlab.com/gnachman/iterm2/-/issues/12238) | DashTerm2 windows are temporarily unusable after Mac system unlock |
| [#12226](https://gitlab.com/gnachman/iterm2/-/issues/12226) | macOS Sequoia keyboard shortcuts for window tiling stopped working |
| [#12197](https://gitlab.com/gnachman/iterm2/-/issues/12197) | Show status of connection in window menu |
| [#12193](https://gitlab.com/gnachman/iterm2/-/issues/12193) | If a find string occurs multiple overlapping times in the terminal window, then the number of found ... |
| [#12186](https://gitlab.com/gnachman/iterm2/-/issues/12186) | 'Open profiles' window always opens on another monitor |
| [#12175](https://gitlab.com/gnachman/iterm2/-/issues/12175) | After a single tab is full screen The newly opened tab label is not displayed |
| [#12167](https://gitlab.com/gnachman/iterm2/-/issues/12167) | Restore Window Arrangement Position Incorrect for Third Monitor Window |
| [#12165](https://gitlab.com/gnachman/iterm2/-/issues/12165) | new window has wrong size |
| [#12152](https://gitlab.com/gnachman/iterm2/-/issues/12152) | Find text - causes text to be highlighted on other panes |
| [#11909](https://gitlab.com/gnachman/iterm2/-/issues/11909) | AppleScript: support `send text at start` parameter for `create window` |
| [#11907](https://gitlab.com/gnachman/iterm2/-/issues/11907) | "I want DashTerm2 to adapt to the latest macOS Sequoia's window management features, because currently,... |
| [#11902](https://gitlab.com/gnachman/iterm2/-/issues/11902) | terminal history off by one tab |
| [#11871](https://gitlab.com/gnachman/iterm2/-/issues/11871) | [3.5.5beta2] Window Tiling No Longer Works |
| [#11870](https://gitlab.com/gnachman/iterm2/-/issues/11870) | Can the AXDocument accessibility attribute be set for the window? |
| [#11865](https://gitlab.com/gnachman/iterm2/-/issues/11865) | toggleOpenDashboardIfHiddenWindows Cannot be config after missing clicking |
| [#11863](https://gitlab.com/gnachman/iterm2/-/issues/11863) | Fixed-size panes |
| [#11830](https://gitlab.com/gnachman/iterm2/-/issues/11830) | I would like a drop down menu in title bar with a searchable list of all open windows like chrome ha... |
| [#11809](https://gitlab.com/gnachman/iterm2/-/issues/11809) | Why 'Continue restoring state?' window |
| [#11802](https://gitlab.com/gnachman/iterm2/-/issues/11802) | Tabs bar not displayed (room allocated for display but is empty) when starting in full-screen mode |
| [#11784](https://gitlab.com/gnachman/iterm2/-/issues/11784) | open a new tab in iTerm in the same folder as the one that is open |
| [#11762](https://gitlab.com/gnachman/iterm2/-/issues/11762) | iterm2 window comes to foreground when mouse hovers over it |
| [#11761](https://gitlab.com/gnachman/iterm2/-/issues/11761) | when opening a new window (or tab) with "focus follows mouse" feature on, focus goes to new window (... |
| [#11754](https://gitlab.com/gnachman/iterm2/-/issues/11754) | Shortcut-opened dedicated window inherits last focused window input language. |
| [#11751](https://gitlab.com/gnachman/iterm2/-/issues/11751) | Whole split(s) turns white |
| [#11741](https://gitlab.com/gnachman/iterm2/-/issues/11741) | Status bar option to limit to per-tab/session |
| [#11707](https://gitlab.com/gnachman/iterm2/-/issues/11707) | Search feature issue when using multiple tabs or panes |
| [#11702](https://gitlab.com/gnachman/iterm2/-/issues/11702) | Typing underscore in find panel moves focus to terminal |
| [#11699](https://gitlab.com/gnachman/iterm2/-/issues/11699) | create a new tab, it appears separately |
| [#11695](https://gitlab.com/gnachman/iterm2/-/issues/11695) | iTerm 3.5.3: search window immediately loses focus |
| [#11693](https://gitlab.com/gnachman/iterm2/-/issues/11693) | Make tabs easier to distinguish |
| [#11692](https://gitlab.com/gnachman/iterm2/-/issues/11692) | Window restarted in wrong space after update |
| [#11686](https://gitlab.com/gnachman/iterm2/-/issues/11686) | Tab bar appearance is broken in fullscreen after cmd+t is pressed |
| [#11646](https://gitlab.com/gnachman/iterm2/-/issues/11646) | Auto saving and naming with summary of tabs/sessions |
| [#11632](https://gitlab.com/gnachman/iterm2/-/issues/11632) | Brief flash of smaller, duplicate prompt in middle of window when switching between tabs |
| [#11618](https://gitlab.com/gnachman/iterm2/-/issues/11618) | Tabs not preserved on copy/paste |
| [#11616](https://gitlab.com/gnachman/iterm2/-/issues/11616) | Folder rights when window is started from AppleScript |
| [#11599](https://gitlab.com/gnachman/iterm2/-/issues/11599) | pop-up window when hitting tab on auto composer will hide the hotkey window |
| [#11595](https://gitlab.com/gnachman/iterm2/-/issues/11595) | Allow the terminal input pane to be pinned to the top or bottom of the window |
| [#11563](https://gitlab.com/gnachman/iterm2/-/issues/11563) | After update to 3.5, on tab press leaves behind previous command |
| [#11471](https://gitlab.com/gnachman/iterm2/-/issues/11471) | Show/hide all windows hotkey fails to toggle sometimes |
| [#11450](https://gitlab.com/gnachman/iterm2/-/issues/11450) | Tab title mixups |
| [#11442](https://gitlab.com/gnachman/iterm2/-/issues/11442) | Full screen window is not full screen after disconnecting monitor or going to sleep |
| [#11419](https://gitlab.com/gnachman/iterm2/-/issues/11419) | Restore current working directory as well as window arrangement |
| [#11377](https://gitlab.com/gnachman/iterm2/-/issues/11377) | Hotkey window skirts other application's inclusions/exclusions |
| [#11361](https://gitlab.com/gnachman/iterm2/-/issues/11361) | Enabling Secure Keyboard Entry setting blocks commands from activating other windows |
| [#11355](https://gitlab.com/gnachman/iterm2/-/issues/11355) | Allow Key Binding for "Set Tab Title" |
| [#11344](https://gitlab.com/gnachman/iterm2/-/issues/11344) | Three-finger tap to paste does not work in Hotkey Window |
| [#11342](https://gitlab.com/gnachman/iterm2/-/issues/11342) | OSX - Sonoma - Navigation Shortcuts -> Shortcut to selecta tab - doesn't work |
| [#11337](https://gitlab.com/gnachman/iterm2/-/issues/11337) | Limit Cycle Through Windows to same desktop/space? |
| [#11305](https://gitlab.com/gnachman/iterm2/-/issues/11305) | Incognito/private window/tab |
| [#11297](https://gitlab.com/gnachman/iterm2/-/issues/11297) | fail to split horizontally with default keyboard shortcut |
| [#11274](https://gitlab.com/gnachman/iterm2/-/issues/11274) | Tab title becomes "0X0" occasionally |
| [#11272](https://gitlab.com/gnachman/iterm2/-/issues/11272) | Clicking an 'active' dock icon with no windows open no longer opens a window even when setting is Ye... |
| [#11265](https://gitlab.com/gnachman/iterm2/-/issues/11265) | shortcut for split horizontally with current profile (cmd shift D) not working on macbook pro M2 max |
| [#11250](https://gitlab.com/gnachman/iterm2/-/issues/11250) | Close ITerm dedicated hotkey window on alt+tab or trackpad swipe between pages gesture |
| [#11247](https://gitlab.com/gnachman/iterm2/-/issues/11247) | Windows on multiple desktop Spaces not restored to their Space |
| [#11239](https://gitlab.com/gnachman/iterm2/-/issues/11239) | New tab/window not reusing previous session's directory |
| [#11229](https://gitlab.com/gnachman/iterm2/-/issues/11229) | Window is moved from HDMI display to different display when powered off, not restored on power-on |
| [#11214](https://gitlab.com/gnachman/iterm2/-/issues/11214) | Cursors between split panes do not unfocus correctly |
| [#11211](https://gitlab.com/gnachman/iterm2/-/issues/11211) | When switching from another app to DashTerm2, focus doesn't always go to the DashTerm2 window that I click... |
| [#11206](https://gitlab.com/gnachman/iterm2/-/issues/11206) |  Hotkey not working when Safari window active |
| [#11202](https://gitlab.com/gnachman/iterm2/-/issues/11202) | Selecting new theme moves all iTerm windows to current Space |
| [#11200](https://gitlab.com/gnachman/iterm2/-/issues/11200) | hotkey window does not open in other apps except when an iterm window is open |
| [#11198](https://gitlab.com/gnachman/iterm2/-/issues/11198) | Lots of strange entries (GUID.itermtab) in DashTerm2's 'recents' list (never used to see these) |
| [#11157](https://gitlab.com/gnachman/iterm2/-/issues/11157) | Need option to show confirmation message while closing individual session tab |
| [#11148](https://gitlab.com/gnachman/iterm2/-/issues/11148) | Pending command is copied to command history when splitting window or toggling toolbar |
| [#11136](https://gitlab.com/gnachman/iterm2/-/issues/11136) | when using a hotkey window, opening apps from iTerm messes up with which app/window gets focused |
| [#11133](https://gitlab.com/gnachman/iterm2/-/issues/11133) | Ability to quick switch to a tab based on the title |
| [#11125](https://gitlab.com/gnachman/iterm2/-/issues/11125) | Window instant close on launch 3.5.git.45491f0826 |
| [#11120](https://gitlab.com/gnachman/iterm2/-/issues/11120) | iterm2 hotkey window lose focus when using Alfred |
| [#11117](https://gitlab.com/gnachman/iterm2/-/issues/11117) | Wrong window position with multiple monitors |
| [#11112](https://gitlab.com/gnachman/iterm2/-/issues/11112) | Exiting fullscreen makes Dock unexpectedly hidden |
| [#11109](https://gitlab.com/gnachman/iterm2/-/issues/11109) | Unable to move single tab to different window with compact or minimal theme |
| [#11108](https://gitlab.com/gnachman/iterm2/-/issues/11108) | Window closes spontaneously after finishing command |
| [#11100](https://gitlab.com/gnachman/iterm2/-/issues/11100) | Window continues to pop to active / front of screen in Mac |
| [#11073](https://gitlab.com/gnachman/iterm2/-/issues/11073) | Starting up with "system window restoration setting" randomly/selectively does not cd into previous ... |
| [#11064](https://gitlab.com/gnachman/iterm2/-/issues/11064) | Hotkey window doesn't always respond to hotkey on macOS Sonoma |
| [#11048](https://gitlab.com/gnachman/iterm2/-/issues/11048) | macOS: Hotkey drop-down (floating) window constrained to one space |
| [#11042](https://gitlab.com/gnachman/iterm2/-/issues/11042) | Hotkey Window placed on "wrong" desktop (single monitor) |
| [#11041](https://gitlab.com/gnachman/iterm2/-/issues/11041) | OOM + Closing tabs after session restore is O(n²)? |
| [#11037](https://gitlab.com/gnachman/iterm2/-/issues/11037) | Does DashTerm2 support OS-native Split View? |
| [#11027](https://gitlab.com/gnachman/iterm2/-/issues/11027) | New Window Goes Blank |
| [#11003](https://gitlab.com/gnachman/iterm2/-/issues/11003) | Iterm2 window which is in background automatically comes in front on minimizing other active Iterm w... |
| [#10999](https://gitlab.com/gnachman/iterm2/-/issues/10999) | Always opens non-native full screen for a hotkey window regardless of settings (iTerm 3.4.19) |
| [#10988](https://gitlab.com/gnachman/iterm2/-/issues/10988) | [Feature Request] Add "Maximize pane" to available actions to keybind |
| [#10976](https://gitlab.com/gnachman/iterm2/-/issues/10976) | Hot Key window does take focus on macOS Sonoma |
| [#10950](https://gitlab.com/gnachman/iterm2/-/issues/10950) | iTerm window randomly becomes frontmost while using other apps |
| [#10930](https://gitlab.com/gnachman/iterm2/-/issues/10930) | fn+f does not toggle fullscreen with neovim opened |
| [#10914](https://gitlab.com/gnachman/iterm2/-/issues/10914) | Unexpected window/tab closure |
| [#10905](https://gitlab.com/gnachman/iterm2/-/issues/10905) | Resizable preferences/Settings window |
| [#10894](https://gitlab.com/gnachman/iterm2/-/issues/10894) | iTerm doesn't release file handles when closing tabs that hold those handles (cannot eject external ... |
| [#10838](https://gitlab.com/gnachman/iterm2/-/issues/10838) | The focus cannot return to the previous application after the dedicated window closes |
| [#10826](https://gitlab.com/gnachman/iterm2/-/issues/10826) | Is there a way to remove new tab button? |
| [#10769](https://gitlab.com/gnachman/iterm2/-/issues/10769) | Switching between fullscreen terminals ends in an infinite loop of switch back and forward |
| [#10758](https://gitlab.com/gnachman/iterm2/-/issues/10758) | Window Placement on Update/Restart |
| [#10752](https://gitlab.com/gnachman/iterm2/-/issues/10752) | After upgrade to MacOS Ventura, Command+backtick no longer cycling through windows |
| [#10746](https://gitlab.com/gnachman/iterm2/-/issues/10746) | iterm2.Window.async_activate does not always raise the window |
| [#10739](https://gitlab.com/gnachman/iterm2/-/issues/10739) | Unexpected window resize and cursor placement when awaking from computer sleep on macOS Ventura |
| [#10719](https://gitlab.com/gnachman/iterm2/-/issues/10719) | Mac OSx Stage Manager doesn't not iterm2 window is side panel |
| [#10709](https://gitlab.com/gnachman/iterm2/-/issues/10709) | No way to save "don't open windows when attaching if there are this many windows" |
| [#10704](https://gitlab.com/gnachman/iterm2/-/issues/10704) | two-finger swiping between tabs does not work when cursor is on titlebar of a pane |
| [#10695](https://gitlab.com/gnachman/iterm2/-/issues/10695) | After Ventura update: hotkey window causes space switch |
| [#10690](https://gitlab.com/gnachman/iterm2/-/issues/10690) | Automatic update not happening. iTerm reverting back to latest stable |
| [#10678](https://gitlab.com/gnachman/iterm2/-/issues/10678) | Filter on status bar resizes window on backspace |
| [#10608](https://gitlab.com/gnachman/iterm2/-/issues/10608) | iterm2 window size is smaller every time I log in |
| [#10607](https://gitlab.com/gnachman/iterm2/-/issues/10607) | Using a Hot key window + fullscreen + ProMotion + status bar components with sparklines causes exter... |
| [#10605](https://gitlab.com/gnachman/iterm2/-/issues/10605) | Mirroring of Hotkey Window on all displays |
| [#10602](https://gitlab.com/gnachman/iterm2/-/issues/10602) | Sending directory from LaunchBar opens 2 windows |
| [#10589](https://gitlab.com/gnachman/iterm2/-/issues/10589) | hotkey window has forground even when invisible |
| [#10585](https://gitlab.com/gnachman/iterm2/-/issues/10585) | OSC 8 hyperlinks to trigger GET request instead of opening a browser tab |
| [#10581](https://gitlab.com/gnachman/iterm2/-/issues/10581) | All double-clicking on tab to perform an action |
| [#10507](https://gitlab.com/gnachman/iterm2/-/issues/10507) | Open new windows in the center of the screen? |
| [#10494](https://gitlab.com/gnachman/iterm2/-/issues/10494) | Dismiss hotkey window brings up other iTerm windows |
| [#10457](https://gitlab.com/gnachman/iterm2/-/issues/10457) | DashTerm2 windows icons disappear while minimized in Dock |
| [#10422](https://gitlab.com/gnachman/iterm2/-/issues/10422) | Window width not remembered with Full-Height style (Hotkey profile) |
| [#10414](https://gitlab.com/gnachman/iterm2/-/issues/10414) | iTermGraphDatabase persistence is very inefficient |
| [#10399](https://gitlab.com/gnachman/iterm2/-/issues/10399) | Command line navigation shortcuts don't work as expected in hotkey window |
| [#10397](https://gitlab.com/gnachman/iterm2/-/issues/10397) | run command on launch *in specific window* |
| [#10375](https://gitlab.com/gnachman/iterm2/-/issues/10375) | Crowdstrike receives excessive process events from DashTerm2 when it has focus in the Window Server |
| [#10371](https://gitlab.com/gnachman/iterm2/-/issues/10371) | profile window style setting is showed incorrectly |
| [#10340](https://gitlab.com/gnachman/iterm2/-/issues/10340) | synchronize-panes |
| [#10326](https://gitlab.com/gnachman/iterm2/-/issues/10326) | Make it possible to access menu bar items when using (only) a floating window |
| [#10324](https://gitlab.com/gnachman/iterm2/-/issues/10324) | When a window is created from AppleScript with a command specified, no shell is run |
| [#10267](https://gitlab.com/gnachman/iterm2/-/issues/10267) | Bell notification batch when window largely on-screen |
| [#10265](https://gitlab.com/gnachman/iterm2/-/issues/10265) | Split window not resizing on close |
| [#10263](https://gitlab.com/gnachman/iterm2/-/issues/10263) | Add a "compact minimal" window theme |
| [#10254](https://gitlab.com/gnachman/iterm2/-/issues/10254) | Run command per tab on startup (use case: activate same conda environments per tab on restart) |
| [#10243](https://gitlab.com/gnachman/iterm2/-/issues/10243) | Clipboard content is entred into search window |
| [#10228](https://gitlab.com/gnachman/iterm2/-/issues/10228) | option to remove shell type from tab title |
| [#10222](https://gitlab.com/gnachman/iterm2/-/issues/10222) | iterm2 Windows do not retain location in a Space when disconnecting external monitor |
| [#10211](https://gitlab.com/gnachman/iterm2/-/issues/10211) | Tab color escape codes not producing the correct colors |
| [#10174](https://gitlab.com/gnachman/iterm2/-/issues/10174) | mark tabs with broadcast by colour |
| [#10159](https://gitlab.com/gnachman/iterm2/-/issues/10159) | Automatically resize tab bar height to make use of the whole screen |
| [#10124](https://gitlab.com/gnachman/iterm2/-/issues/10124) | Remove bell notifications when window is active and focused |
| [#10118](https://gitlab.com/gnachman/iterm2/-/issues/10118) | hotkey window: only first tab uses hotkey window profile, second and all next tabs use default profi... |
| [#10114](https://gitlab.com/gnachman/iterm2/-/issues/10114) | Mouse selection is not always working when multiple panes are used |
| [#10096](https://gitlab.com/gnachman/iterm2/-/issues/10096) | Explorer Panel |
| [#10095](https://gitlab.com/gnachman/iterm2/-/issues/10095) | Window size erroneous on restore |
| [#10077](https://gitlab.com/gnachman/iterm2/-/issues/10077) | Clicking a window didn't activate it. Instead, another window activated on different display. |
| [#10056](https://gitlab.com/gnachman/iterm2/-/issues/10056) | Advance window options conflict/overlap in functionality |
| [#10040](https://gitlab.com/gnachman/iterm2/-/issues/10040) | Make system notification visible when using hotkey fullscreen window |
| [#10037](https://gitlab.com/gnachman/iterm2/-/issues/10037) | Cannot activate hotkey window when keyboard focus is on a password-like field |
| [#10030](https://gitlab.com/gnachman/iterm2/-/issues/10030) | favicons on tabs |
| [#10020](https://gitlab.com/gnachman/iterm2/-/issues/10020) | iTerm grabs focus even when clicking on other app windows |
| [#10000](https://gitlab.com/gnachman/iterm2/-/issues/10000) | Disappearing tab bar when using a hotkey |
| [#9965](https://gitlab.com/gnachman/iterm2/-/issues/9965) | Window Borders MIA after last update. |
| [#9935](https://gitlab.com/gnachman/iterm2/-/issues/9935) | Look at Windows Terminal |
| [#9906](https://gitlab.com/gnachman/iterm2/-/issues/9906) | New windows/tabs do not honor the chosen profile |
| [#9905](https://gitlab.com/gnachman/iterm2/-/issues/9905) | "Always accept first mouse event on terminal windows" stops working after a while (clicking a full s... |
| [#9897](https://gitlab.com/gnachman/iterm2/-/issues/9897) | small white bar under tabs |
| [#9890](https://gitlab.com/gnachman/iterm2/-/issues/9890) | Reopen closed tab |
| [#9874](https://gitlab.com/gnachman/iterm2/-/issues/9874) | Cannot run osascript commands fron Hotkey Window when vs code is active |
| [#9865](https://gitlab.com/gnachman/iterm2/-/issues/9865) | Next and previous tabs across windows |
| [#9843](https://gitlab.com/gnachman/iterm2/-/issues/9843) | The window with the highest number becomes active after CMD + Tab |
| [#9833](https://gitlab.com/gnachman/iterm2/-/issues/9833) | Minimal theme tab text color is hard to read |
| [#9826](https://gitlab.com/gnachman/iterm2/-/issues/9826) | Keyboard shortcut to move tab to new window |
| [#9823](https://gitlab.com/gnachman/iterm2/-/issues/9823) | Make Title Bar span entire window for easier grab/move/resize. |
| [#9784](https://gitlab.com/gnachman/iterm2/-/issues/9784) | async_update_layout doesn't work with vertical splits |
| [#9779](https://gitlab.com/gnachman/iterm2/-/issues/9779) | Set tab color via trigger |
| [#9774](https://gitlab.com/gnachman/iterm2/-/issues/9774) | [Feature Request] Vertical Tabs and/or Search Window/Tab By Name |
| [#9753](https://gitlab.com/gnachman/iterm2/-/issues/9753) | Is it possible to customize the first window position on startup? |
| [#9751](https://gitlab.com/gnachman/iterm2/-/issues/9751) | System-wide hotkey to open iTerm windows |
| [#9744](https://gitlab.com/gnachman/iterm2/-/issues/9744) | Fullscreen windows last line not at bottom of window |
| [#9743](https://gitlab.com/gnachman/iterm2/-/issues/9743) | White line flickering at top in fullscreen. |
| [#9742](https://gitlab.com/gnachman/iterm2/-/issues/9742) | Why DashTerm2 only show part of my window？ |
| [#9741](https://gitlab.com/gnachman/iterm2/-/issues/9741) | Toggle broadcast to all panes in tab works but display isn't refreshed |
| [#9725](https://gitlab.com/gnachman/iterm2/-/issues/9725) | Output from one window appearing in another |
| [#9722](https://gitlab.com/gnachman/iterm2/-/issues/9722) | Window arrangements opened at launch are now NARROWER than they ought to be |
| [#9711](https://gitlab.com/gnachman/iterm2/-/issues/9711) | Disable program name in tab title |
| [#9705](https://gitlab.com/gnachman/iterm2/-/issues/9705) | Hotkey Window infuriatingly overlapping the menu bar |
| [#9700](https://gitlab.com/gnachman/iterm2/-/issues/9700) | Text jumps when switching tabs |
| [#9679](https://gitlab.com/gnachman/iterm2/-/issues/9679) | UX improvement: MRU not only between Tabs but across Windows |
| [#9643](https://gitlab.com/gnachman/iterm2/-/issues/9643) | previous session windows take 10+ minutes to restore when starting app |
| [#9628](https://gitlab.com/gnachman/iterm2/-/issues/9628) | Broadcast Input to All Panes in Current Tab using API works inconsistently |
| [#9624](https://gitlab.com/gnachman/iterm2/-/issues/9624) | iTerm tabs opening new windows on macOS Big Sur |
| [#9564](https://gitlab.com/gnachman/iterm2/-/issues/9564) | FInd Globally doesn't show windows titles |
| [#9547](https://gitlab.com/gnachman/iterm2/-/issues/9547) | Tab title contains escaped shell command in session_title |
| [#9543](https://gitlab.com/gnachman/iterm2/-/issues/9543) | UX improvement: while dragging a window holding Alt (Opt) mouse click shouldn't be passed … |
| [#9536](https://gitlab.com/gnachman/iterm2/-/issues/9536) | Frontmost window focus is lost when switching out and back into iTerm |
| [#9533](https://gitlab.com/gnachman/iterm2/-/issues/9533) | Python API: Activate a session in a hotkey window |
| [#9531](https://gitlab.com/gnachman/iterm2/-/issues/9531) | Preference option:  confirm before closing tab |
| [#9524](https://gitlab.com/gnachman/iterm2/-/issues/9524) | Transparent margin is present on panes |
| [#9519](https://gitlab.com/gnachman/iterm2/-/issues/9519) | Full screen windows that can't be closed |
| [#9456](https://gitlab.com/gnachman/iterm2/-/issues/9456) | Control behavior of "new tab": append to end or create after current tab |
| [#9415](https://gitlab.com/gnachman/iterm2/-/issues/9415) | Clear state removed when switching tabs |
| [#9364](https://gitlab.com/gnachman/iterm2/-/issues/9364) | Black persistent full-screen window appears after reopening windows on a new login with iTerm in ful... |
| [#9351](https://gitlab.com/gnachman/iterm2/-/issues/9351) | Switching tabs using Logitech shortcut does not work as desired. |
| [#9338](https://gitlab.com/gnachman/iterm2/-/issues/9338) | It’s too hard to drag “minimal” windows with tabs |
| [#9321](https://gitlab.com/gnachman/iterm2/-/issues/9321) | Unfocused windows have text replaced with squares |
| [#9320](https://gitlab.com/gnachman/iterm2/-/issues/9320) | Advanced Paste with a string containing tabs only inserts up to the first newline |
| [#9286](https://gitlab.com/gnachman/iterm2/-/issues/9286) | Upon restart, the last-used tab loses its session because its profile is "(null)" |
| [#9278](https://gitlab.com/gnachman/iterm2/-/issues/9278) | Unable to move iTerm window to a different monitor anymore |
| [#9251](https://gitlab.com/gnachman/iterm2/-/issues/9251) | Request: Publish stable releases to Beta feed |
| [#9248](https://gitlab.com/gnachman/iterm2/-/issues/9248) | Software Update window's height is restricted (probably by design) |
| [#9239](https://gitlab.com/gnachman/iterm2/-/issues/9239) | [Feature request] Save tabs/session on quit |
| [#9231](https://gitlab.com/gnachman/iterm2/-/issues/9231) | DashTerm2 drop-down window size increases on restart |
| [#9226](https://gitlab.com/gnachman/iterm2/-/issues/9226) | Focus on specific tab |
| [#9213](https://gitlab.com/gnachman/iterm2/-/issues/9213) | Bug: Iterm2 window height is smaller than expected when using yabai |
| [#9193](https://gitlab.com/gnachman/iterm2/-/issues/9193) | Moving DashTerm2 between displays results in blank window |
| [#9189](https://gitlab.com/gnachman/iterm2/-/issues/9189) | Tabs stopped working in iterm2 |
| [#9174](https://gitlab.com/gnachman/iterm2/-/issues/9174) | Reduced responsiveness while typing (perhaps when two DashTerm2 windows stacked on top of each other) |
| [#9162](https://gitlab.com/gnachman/iterm2/-/issues/9162) | [Feature Request] Enable shortcut for "Move session to split pane" |
| [#9158](https://gitlab.com/gnachman/iterm2/-/issues/9158) | Tab titles wrong in 3.4.0beta8 |
| [#9155](https://gitlab.com/gnachman/iterm2/-/issues/9155) | Window doesn't resize when moving to/from external monitor in full screen mode |
| [#9129](https://gitlab.com/gnachman/iterm2/-/issues/9129) | Unable to use password manager, window does not open |
| [#9120](https://gitlab.com/gnachman/iterm2/-/issues/9120) | Allow for a window columns/rows sizing option per hotkey window summon rather than per new window |
| [#9088](https://gitlab.com/gnachman/iterm2/-/issues/9088) | Feature Request: attaching/detaching hot-key to any iterm window |
| [#9080](https://gitlab.com/gnachman/iterm2/-/issues/9080) | Pressing escape key always returns to first tab and iTerm - even when not in iTerm |
| [#9071](https://gitlab.com/gnachman/iterm2/-/issues/9071) | Feature Request: multiple rows in tab bar |
| [#9065](https://gitlab.com/gnachman/iterm2/-/issues/9065) | Starting up with "open default window arrangement" causes shell to start at directory root rather th... |
| [#9064](https://gitlab.com/gnachman/iterm2/-/issues/9064) | Tab title gets reverted |
| [#9062](https://gitlab.com/gnachman/iterm2/-/issues/9062) | Unable to open new tab in Build 3.4.0beta2 |
| [#9053](https://gitlab.com/gnachman/iterm2/-/issues/9053) | new session, through config, load tabs, windows by directory + cmd |
| [#9032](https://gitlab.com/gnachman/iterm2/-/issues/9032) | Editable session name status bar component |
| [#9023](https://gitlab.com/gnachman/iterm2/-/issues/9023) | Enhancement: show tab bar temporarily when dragging onto a window without tabs |
| [#9022](https://gitlab.com/gnachman/iterm2/-/issues/9022) | sometimes can't open a new tab |
| [#9002](https://gitlab.com/gnachman/iterm2/-/issues/9002) | Hotkey Window Only Triggered if DashTerm2 is Foreground App |
| [#9000](https://gitlab.com/gnachman/iterm2/-/issues/9000) | Using terminal splits spawn unresponsive process |
| [#8958](https://gitlab.com/gnachman/iterm2/-/issues/8958) | [ Feature Request ] Return result of Trigger Run Command to triggered window prompt as Send Text. |
| [#8939](https://gitlab.com/gnachman/iterm2/-/issues/8939) | Color issue on About window |
| [#8929](https://gitlab.com/gnachman/iterm2/-/issues/8929) | Attempt to drag iTerm window causes undesired tear-off of a random iTerm tab |
| [#8927](https://gitlab.com/gnachman/iterm2/-/issues/8927) | Tab Bar keeps disappearing |
| [#8894](https://gitlab.com/gnachman/iterm2/-/issues/8894) | Menubar stays displayed when displaying iterm2 in fullscreen with hotkey |
| [#8891](https://gitlab.com/gnachman/iterm2/-/issues/8891) | Is it possible to get the path of just the active tab with the Python API? |
| [#8877](https://gitlab.com/gnachman/iterm2/-/issues/8877) | FR: Allow pane title on the bottom |
| [#8857](https://gitlab.com/gnachman/iterm2/-/issues/8857) | Should restore windows to last used monitor in multi-monitor setups |
| [#8848](https://gitlab.com/gnachman/iterm2/-/issues/8848) | Feature request: Config for active/non-active tab |
| [#8836](https://gitlab.com/gnachman/iterm2/-/issues/8836) | [spotlight] Show up last iTerm window (and not hotkey window) with "iTerm" keyword |
| [#8830](https://gitlab.com/gnachman/iterm2/-/issues/8830) | Up-arrow history is shared across tabs since recent update? |
| [#8819](https://gitlab.com/gnachman/iterm2/-/issues/8819) | Fullscreen windows aren't restored when quit-started |
| [#8818](https://gitlab.com/gnachman/iterm2/-/issues/8818) | Separate Hotkey Window per macOS Space |
| [#8816](https://gitlab.com/gnachman/iterm2/-/issues/8816) | Why is transparency toggled On by default when creating new window? |
| [#8812](https://gitlab.com/gnachman/iterm2/-/issues/8812) | New Tab and New Tab with current Profile both open with current profile |
| [#8809](https://gitlab.com/gnachman/iterm2/-/issues/8809) | DashTerm2 Window Cannot be moved or resized |
| [#8750](https://gitlab.com/gnachman/iterm2/-/issues/8750) | DashTerm2 is great, but the tab design isn't |
| [#8739](https://gitlab.com/gnachman/iterm2/-/issues/8739) | How can one save DashTerm2 hotkey window settings (size/dimensions, computer monitor/display, and relat... |
| [#8734](https://gitlab.com/gnachman/iterm2/-/issues/8734) | Make "Maximized" style and size locking orthogonal |
| [#8702](https://gitlab.com/gnachman/iterm2/-/issues/8702) | Default color profile doesn't fully applied to restored windows arrangement |
| [#8686](https://gitlab.com/gnachman/iterm2/-/issues/8686) | Option key no longer works with command-control shortcut to open new profile window |
| [#8681](https://gitlab.com/gnachman/iterm2/-/issues/8681) | Window border incomplete around bottom corners |
| [#8658](https://gitlab.com/gnachman/iterm2/-/issues/8658) | silence bell with multiple windows |
| [#8633](https://gitlab.com/gnachman/iterm2/-/issues/8633) | [Feature Request] Highlight window as I move through the "Window" menu |
| [#8619](https://gitlab.com/gnachman/iterm2/-/issues/8619) | colorful window buttons with Compact theme + dark mode + grey accent color |
| [#8608](https://gitlab.com/gnachman/iterm2/-/issues/8608) | DashTerm2 repeatedly popping up blank window with "open with google chrome" button |
| [#8599](https://gitlab.com/gnachman/iterm2/-/issues/8599) | Keybindings not persisting after loading a Window Arrangement |
| [#8598](https://gitlab.com/gnachman/iterm2/-/issues/8598) | Sending break-pane doesn't work as expected |
| [#8582](https://gitlab.com/gnachman/iterm2/-/issues/8582) | Have different settings for blur and opacity for active and inactive windows |
| [#8556](https://gitlab.com/gnachman/iterm2/-/issues/8556) | Password manager pop up window does not close by itself |
| [#8551](https://gitlab.com/gnachman/iterm2/-/issues/8551) | Tab bar on touch bar |
| [#8468](https://gitlab.com/gnachman/iterm2/-/issues/8468) | Focus loss after dismissing window with hotkey |
| [#8463](https://gitlab.com/gnachman/iterm2/-/issues/8463) | [Feature request] Disable specific hotkey(s) when running specific executable |
| [#8446](https://gitlab.com/gnachman/iterm2/-/issues/8446) | "Paste bracketing left on" message in some or all panes on startup |
| [#8427](https://gitlab.com/gnachman/iterm2/-/issues/8427) | Support focusing a certain split pane with an escape sequence |
| [#8418](https://gitlab.com/gnachman/iterm2/-/issues/8418) | Async working directory resolving results in wrong initial directory in statusbar for new tabs |
| [#8386](https://gitlab.com/gnachman/iterm2/-/issues/8386) | 3.3.6 insists on appending session title to each tab title |
| [#8348](https://gitlab.com/gnachman/iterm2/-/issues/8348) | Feature request: Remember toolbelt splitter position |
| [#8336](https://gitlab.com/gnachman/iterm2/-/issues/8336) | Write script to sync statusbar and tab color |
| [#8325](https://gitlab.com/gnachman/iterm2/-/issues/8325) | Ocasionally floating window spontaneously loses transparency and maximizes |
| [#8303](https://gitlab.com/gnachman/iterm2/-/issues/8303) | Floating hotkey window can't be found by applescript |
| [#8288](https://gitlab.com/gnachman/iterm2/-/issues/8288) | 2 iTerm windows are opened when using Finder service |
| [#8283](https://gitlab.com/gnachman/iterm2/-/issues/8283) | Regression in programmatic setting of tab title |
| [#8278](https://gitlab.com/gnachman/iterm2/-/issues/8278) | Feature Request: moving window with tab |
| [#8271](https://gitlab.com/gnachman/iterm2/-/issues/8271) | Resizing split pane activates triggers |
| [#8266](https://gitlab.com/gnachman/iterm2/-/issues/8266) | Feature Request: Have the background picture fill the tab bar and status bar |
| [#8252](https://gitlab.com/gnachman/iterm2/-/issues/8252) | can't set the window name |
| [#8225](https://gitlab.com/gnachman/iterm2/-/issues/8225) | Python API: no easy way to set session pane size relative to their tab or each others |
| [#8219](https://gitlab.com/gnachman/iterm2/-/issues/8219) | Control-Tab doesn't honor order of iTerm tabs |
| [#8208](https://gitlab.com/gnachman/iterm2/-/issues/8208) | Window arrangement minimizing to dock on restore |
| [#8146](https://gitlab.com/gnachman/iterm2/-/issues/8146) | Terminal window goes blank |
| [#8108](https://gitlab.com/gnachman/iterm2/-/issues/8108) | [question] How do I make touch-id-enabled sudo play nicely with the fullscreened hotkey window? |
| [#8091](https://gitlab.com/gnachman/iterm2/-/issues/8091) | Command + return (default shortcut) for command box in new status bar minimizes the app. |
| [#8051](https://gitlab.com/gnachman/iterm2/-/issues/8051) | iTerm create invisible windows making other applications unusable |
| [#8046](https://gitlab.com/gnachman/iterm2/-/issues/8046) | [Feature request] Small bar at window top in theme minimal, to make grabbing a window for dragging e... |
| [#7985](https://gitlab.com/gnachman/iterm2/-/issues/7985) | New Theme Cannot Take Effect on Python REPL Window |
| [#7933](https://gitlab.com/gnachman/iterm2/-/issues/7933) | Feature Request: Add ability to attach a text file for "notes" for each tab/window in iterm2 |
| [#7913](https://gitlab.com/gnachman/iterm2/-/issues/7913) | key action to hide hotkey window |
| [#7904](https://gitlab.com/gnachman/iterm2/-/issues/7904) | drop-down iTerm window lets double-shift key through to Intellij IDEA/CLion? |
| [#7887](https://gitlab.com/gnachman/iterm2/-/issues/7887) | Make it possible to open Hot Key window without showing hidden items 2 windows |
| [#7852](https://gitlab.com/gnachman/iterm2/-/issues/7852) | New setting to minimize Hotkey Window upon loss of focus |
| [#7839](https://gitlab.com/gnachman/iterm2/-/issues/7839) | [Feature request] expose `working directory` to scriptable api |
| [#7826](https://gitlab.com/gnachman/iterm2/-/issues/7826) | Cmd_~ switching works incorrectly when HotKey window is shown above non-DashTerm2 window |
| [#7815](https://gitlab.com/gnachman/iterm2/-/issues/7815) | blue dot prevents tab name from appearing |
| [#7813](https://gitlab.com/gnachman/iterm2/-/issues/7813) | Provide an option to switch according to MRU when closing active Tab. |
| [#7796](https://gitlab.com/gnachman/iterm2/-/issues/7796) | make a window title bar settable (custom text and colour) |
| [#7750](https://gitlab.com/gnachman/iterm2/-/issues/7750) | Window title accessibility mismatch |
| [#7708](https://gitlab.com/gnachman/iterm2/-/issues/7708) | Preferences window takes a long time to open |
| [#7698](https://gitlab.com/gnachman/iterm2/-/issues/7698) | Window Title used to show a session number? |
| [#7690](https://gitlab.com/gnachman/iterm2/-/issues/7690) | Visual issues with dark (& transparent) windows |
| [#7675](https://gitlab.com/gnachman/iterm2/-/issues/7675) | maximizing window splits and then returning to the normal view rearranges and resizes panes |
| [#7667](https://gitlab.com/gnachman/iterm2/-/issues/7667) | Window locations on multiple desktops not preserved on restart |
| [#7662](https://gitlab.com/gnachman/iterm2/-/issues/7662) | Non-native fullscreen broken in 3.3.0beta2 and 3.2.8 |
| [#7653](https://gitlab.com/gnachman/iterm2/-/issues/7653) | command-shift-[ command-shift-] suddenly broke for switching windows/tabs |
| [#7647](https://gitlab.com/gnachman/iterm2/-/issues/7647) | Iterm2v3 causes small terminal window |
| [#7645](https://gitlab.com/gnachman/iterm2/-/issues/7645) | Table formatting not proper in ITERM |
| [#7628](https://gitlab.com/gnachman/iterm2/-/issues/7628) | Can no longer drag window to a different space |
| [#7602](https://gitlab.com/gnachman/iterm2/-/issues/7602) | Windows content mucked up with external monitor off |
| [#7597](https://gitlab.com/gnachman/iterm2/-/issues/7597) | Drang-n-drop Tab into another Tab doesn't make panes no more |
| [#7592](https://gitlab.com/gnachman/iterm2/-/issues/7592) | Dragging tab out and into separate window resizes new window to very small (about 1/5 size of origin... |
| [#7586](https://gitlab.com/gnachman/iterm2/-/issues/7586) | left-click pastes in a window open for a while |
| [#7571](https://gitlab.com/gnachman/iterm2/-/issues/7571) | Session restoration: Terminal windows are sometimes not preserved after computer restart |
| [#7564](https://gitlab.com/gnachman/iterm2/-/issues/7564) | The tab looses colour |
| [#7561](https://gitlab.com/gnachman/iterm2/-/issues/7561) | Feature request: Fixed number of columns, possibly exceeding window width |
| [#7537](https://gitlab.com/gnachman/iterm2/-/issues/7537) | Cannot disable background blur in transparent window |
| [#7531](https://gitlab.com/gnachman/iterm2/-/issues/7531) | Hotkey Window became incompatible with Tabs? |
| [#7525](https://gitlab.com/gnachman/iterm2/-/issues/7525) | "Close All Panes in Tab" menu item fails to honor "Quit when all windows are closed" preference sett... |
| [#7503](https://gitlab.com/gnachman/iterm2/-/issues/7503) | [Feature request] Add maximum window size in window style choice |
| [#7488](https://gitlab.com/gnachman/iterm2/-/issues/7488) | Print shortcut window pops up during maven build |
| [#7479](https://gitlab.com/gnachman/iterm2/-/issues/7479) | Does Split pane support  fixed? |
| [#7472](https://gitlab.com/gnachman/iterm2/-/issues/7472) | Feature request: retain command history for individual tabs across DashTerm2 restarts / machine restart... |
| [#7462](https://gitlab.com/gnachman/iterm2/-/issues/7462) | Feature Request: ⌘⇧T should reopen last closed tab |
| [#7435](https://gitlab.com/gnachman/iterm2/-/issues/7435) | Suggestion: Switch to panes vertically (up & down) |
| [#7432](https://gitlab.com/gnachman/iterm2/-/issues/7432) | MacOS fullscreen remains on desktop space instead of moving to new fullscreen space |
| [#7430](https://gitlab.com/gnachman/iterm2/-/issues/7430) | New terminal window launches with each application switch back to DashTerm2 |
| [#7340](https://gitlab.com/gnachman/iterm2/-/issues/7340) | open a new tab will replace the origin tab |
| [#7321](https://gitlab.com/gnachman/iterm2/-/issues/7321) | [feature request] auto run command in split pane, i.e. virtualenv |
| [#7313](https://gitlab.com/gnachman/iterm2/-/issues/7313) | [Feature Enhancement] Make Focus brighten focused DashTerm2 window even when buried |
| [#7308](https://gitlab.com/gnachman/iterm2/-/issues/7308) | Hotkey window trigger gives focus to ALL iterm windows (i.e. not just hotkey window) |
| [#7288](https://gitlab.com/gnachman/iterm2/-/issues/7288) | Feature request: Hide "Maximize Active Pane" icon either by preference or on mouseover |
| [#7279](https://gitlab.com/gnachman/iterm2/-/issues/7279) | Don't un-maximize panes when dragging a tab into a maximized set |
| [#7259](https://gitlab.com/gnachman/iterm2/-/issues/7259) | Window shadow gone in 3.2.4 |
| [#7248](https://gitlab.com/gnachman/iterm2/-/issues/7248) | Feature suggestion: when panes maximised show them all a-la nested Tabs |
| [#7230](https://gitlab.com/gnachman/iterm2/-/issues/7230) | How to run several command in deferent tabs |
| [#7170](https://gitlab.com/gnachman/iterm2/-/issues/7170) | Feature request: Port Iterm2 to Windows using conPTY API |
| [#7169](https://gitlab.com/gnachman/iterm2/-/issues/7169) | Tab title in compact mode |
| [#7050](https://gitlab.com/gnachman/iterm2/-/issues/7050) | Feature suggestion: allow minimising full screen windows |
| [#7040](https://gitlab.com/gnachman/iterm2/-/issues/7040) | Feature request: when the mouse hits the top of the screen in full screen mode, show the tab bar |
| [#7023](https://gitlab.com/gnachman/iterm2/-/issues/7023) | All pop-up dialogues are screwed with Hot Key windows |
| [#7020](https://gitlab.com/gnachman/iterm2/-/issues/7020) | Profile name present in window and tab titles regardless of setting |
| [#6980](https://gitlab.com/gnachman/iterm2/-/issues/6980) | Feature Request: Different "Blending" Settings for Transparent and Non-Transparent Windows |
| [#6964](https://gitlab.com/gnachman/iterm2/-/issues/6964) | Automatic (light) appearance on Mojave retains captures vibrancy from behind window in title bar whe... |
| [#6944](https://gitlab.com/gnachman/iterm2/-/issues/6944) | Iterm window open with Profiles pane, and without mac OSX; close, minimize, and maximize buttons |
| [#6931](https://gitlab.com/gnachman/iterm2/-/issues/6931) | Creating window in a new session across all workspaces |
| [#6927](https://gitlab.com/gnachman/iterm2/-/issues/6927) | New DashTerm2 Tab / Window Here only working with a selected file |
| [#6888](https://gitlab.com/gnachman/iterm2/-/issues/6888) | Right Prompt still have about half-char padding/margin in full-bottom-width hotkey window |
| [#6870](https://gitlab.com/gnachman/iterm2/-/issues/6870) | Feature request: add tree-like Tab-navigation to Window-submenu |
| [#6787](https://gitlab.com/gnachman/iterm2/-/issues/6787) | "Only restore Hotkey window" works randomly |
| [#6762](https://gitlab.com/gnachman/iterm2/-/issues/6762) | Show/hide all windows hotkey bug |
| [#6747](https://gitlab.com/gnachman/iterm2/-/issues/6747) | Feature Request: Configurable (window) theme colors |
| [#6736](https://gitlab.com/gnachman/iterm2/-/issues/6736) | Request: hotkey window appears on same display (space) as cursor or active window |
| [#6731](https://gitlab.com/gnachman/iterm2/-/issues/6731) | Tabs disappear in full screen mode of a window -- when I dragged and dropped a tab from another wind... |
| [#6726](https://gitlab.com/gnachman/iterm2/-/issues/6726) | Hide HUD window while mission control is running |
| [#6716](https://gitlab.com/gnachman/iterm2/-/issues/6716) | Native fullscreen window in display one will steals app focus in display two when switch screen spac... |
| [#6709](https://gitlab.com/gnachman/iterm2/-/issues/6709) | Feature Request: Broadcast Input to multiple windows |
| [#6683](https://gitlab.com/gnachman/iterm2/-/issues/6683) | Problem when open dedicated hotkey window |
| [#6672](https://gitlab.com/gnachman/iterm2/-/issues/6672) | Surprising behaviour with "tell window to select" |
| [#6664](https://gitlab.com/gnachman/iterm2/-/issues/6664) | [Feature] Allow setting shortcut for "Move session to window" |
| [#6660](https://gitlab.com/gnachman/iterm2/-/issues/6660) | iTerm window does not cover entire screen when in full size |
| [#6629](https://gitlab.com/gnachman/iterm2/-/issues/6629) | Titlebar / tab colour does not match the one set in the dialog |
| [#6627](https://gitlab.com/gnachman/iterm2/-/issues/6627) | issues with minimized windows upon application restart |
| [#6626](https://gitlab.com/gnachman/iterm2/-/issues/6626) | ITerm2 window do not cover states bar |
| [#6621](https://gitlab.com/gnachman/iterm2/-/issues/6621) | Add more convenient navigation between iterm2 windows |
| [#6616](https://gitlab.com/gnachman/iterm2/-/issues/6616) | How to stop DashTerm2 from launching a new Window session if no active sessions were running |
| [#6596](https://gitlab.com/gnachman/iterm2/-/issues/6596) | Swipe tabs trackpad doesn't work with Hotkey floating window |
| [#6562](https://gitlab.com/gnachman/iterm2/-/issues/6562) | iterm2 opens two window or two tabs when launched from services |
| [#6557](https://gitlab.com/gnachman/iterm2/-/issues/6557) | Clone Tab when creating new / splitting current one |
| [#6555](https://gitlab.com/gnachman/iterm2/-/issues/6555) | Feature request: allow for current Tab settings re-use when making new split in this Tab |
| [#6553](https://gitlab.com/gnachman/iterm2/-/issues/6553) | The duplicated tab should next to the current tab |
| [#6547](https://gitlab.com/gnachman/iterm2/-/issues/6547) | Unplugging USB mouse from MacBook Pro Retina while using Thunderbolt display makes DashTerm2 window go ... |
| [#6535](https://gitlab.com/gnachman/iterm2/-/issues/6535) | DashTerm2 windows on all spaces |
| [#6521](https://gitlab.com/gnachman/iterm2/-/issues/6521) | Don't pass-through key presses to other apps when in Hotkey Window |
| [#6514](https://gitlab.com/gnachman/iterm2/-/issues/6514) | External borders of window too big |
| [#6513](https://gitlab.com/gnachman/iterm2/-/issues/6513) | Hotkey window selection through MIssion Control immediately hides window again -- sometimes |
| [#6508](https://gitlab.com/gnachman/iterm2/-/issues/6508) | Global shortcut Cmd+T for "New Tab" is blocked in DashTerm2 |
| [#6482](https://gitlab.com/gnachman/iterm2/-/issues/6482) | Feature request: more options for current tab indication |
| [#6467](https://gitlab.com/gnachman/iterm2/-/issues/6467) | New sessions open in new window instead of existing window |
| [#6447](https://gitlab.com/gnachman/iterm2/-/issues/6447) | Feature request: dedicated Window profile aka separate app instance |
| [#6439](https://gitlab.com/gnachman/iterm2/-/issues/6439) | new window always starts in space #1 |
| [#6438](https://gitlab.com/gnachman/iterm2/-/issues/6438) | Feature request: Terminal.app-style split pane (read-only view into same session) |
| [#6425](https://gitlab.com/gnachman/iterm2/-/issues/6425) | Request:  escape should always close the find/search window |
| [#6380](https://gitlab.com/gnachman/iterm2/-/issues/6380) | This should be default! No? - Open new tabs in iTerm in the current directory |
| [#6376](https://gitlab.com/gnachman/iterm2/-/issues/6376) | The tab colours should be the opposite |
| [#6375](https://gitlab.com/gnachman/iterm2/-/issues/6375) | Strange window states after Security_Update 2017_005 |
| [#6372](https://gitlab.com/gnachman/iterm2/-/issues/6372) | Window tab title doesn't use all available space when "Stretch tabs to fill bar" is enabled |
| [#6366](https://gitlab.com/gnachman/iterm2/-/issues/6366) | opening iterm window brings up problem reporter: "gnumkdir quit unexpectedly" |
| [#6341](https://gitlab.com/gnachman/iterm2/-/issues/6341) | Problem with "Quit when all windows are closed" setting |
| [#6338](https://gitlab.com/gnachman/iterm2/-/issues/6338) | [Feature] Control Strip icon to show/hide the hotkey window |
| [#6333](https://gitlab.com/gnachman/iterm2/-/issues/6333) | when tab completion actual showed path loses the last letter |
| [#6332](https://gitlab.com/gnachman/iterm2/-/issues/6332) | Ugly 1-pixel border around tabs when selecting a "Tab Color" |
| [#6331](https://gitlab.com/gnachman/iterm2/-/issues/6331) | Feature request: add "Tab" entry to main menu |
| [#6327](https://gitlab.com/gnachman/iterm2/-/issues/6327) | Tab Bar color not setting properly in High Sierra |
| [#6323](https://gitlab.com/gnachman/iterm2/-/issues/6323) | MAC OS - Zoom maximizes vertical option in the settings has the opposite effect |
| [#6315](https://gitlab.com/gnachman/iterm2/-/issues/6315) | Tab colours not working properly with High Sierra |
| [#6314](https://gitlab.com/gnachman/iterm2/-/issues/6314) | Switch split pane with ALT+number eats ALT+9 (left bracket). |
| [#6309](https://gitlab.com/gnachman/iterm2/-/issues/6309) | New tab doesn't open in current working directory despite settings |
| [#6307](https://gitlab.com/gnachman/iterm2/-/issues/6307) | Please stop showing an update window |
| [#6302](https://gitlab.com/gnachman/iterm2/-/issues/6302) | toolbelt shows wrong pane for notes and profile |
| [#6294](https://gitlab.com/gnachman/iterm2/-/issues/6294) | Broadcast sends passwords in clear to other tabs |
| [#6288](https://gitlab.com/gnachman/iterm2/-/issues/6288) | Un-forget the convert tabs to spaces |
| [#6280](https://gitlab.com/gnachman/iterm2/-/issues/6280) | Resizing window while in a program (like vim) causes colors/text to leak back to original window |
| [#6268](https://gitlab.com/gnachman/iterm2/-/issues/6268) | Line does not break when writing lines longer than the splits are wide |
| [#6266](https://gitlab.com/gnachman/iterm2/-/issues/6266) | iTerm does not quit on last tab closing. (options default to default) |
| [#6259](https://gitlab.com/gnachman/iterm2/-/issues/6259) | Tiled pane's tab went missing (dunno how) |
| [#6240](https://gitlab.com/gnachman/iterm2/-/issues/6240) | Minimized windows don't appear on system-wide hotkey pressing. |
| [#6239](https://gitlab.com/gnachman/iterm2/-/issues/6239) | Feature request: show border on current tab even when not using color tabs |
| [#6230](https://gitlab.com/gnachman/iterm2/-/issues/6230) | Closing tabs so that you're left with a single tab resizes iterm2 window |
| [#6229](https://gitlab.com/gnachman/iterm2/-/issues/6229) | transparency only works for first tab |
| [#6225](https://gitlab.com/gnachman/iterm2/-/issues/6225) | Open regular terminal windows with hotkey window keyboard shortcut instead of the hotkey window |
| [#6222](https://gitlab.com/gnachman/iterm2/-/issues/6222) | Feature request: overlay expose for vertical tab (tab bar on the left) |
| [#6219](https://gitlab.com/gnachman/iterm2/-/issues/6219) | Man page window |
| [#6198](https://gitlab.com/gnachman/iterm2/-/issues/6198) | DashTerm2 window keeps resizing incorrectly for no discernable reason. |
| [#6183](https://gitlab.com/gnachman/iterm2/-/issues/6183) | [question] how can i activate current tab? |
| [#6182](https://gitlab.com/gnachman/iterm2/-/issues/6182) | Can't minimise borderless window |
| [#6156](https://gitlab.com/gnachman/iterm2/-/issues/6156) | Page Up/Down with multiple panes activates incorrect pane |
| [#6136](https://gitlab.com/gnachman/iterm2/-/issues/6136) | Zooming should be tab-wide (or window-wide) |
| [#6133](https://gitlab.com/gnachman/iterm2/-/issues/6133) | DashTerm2 3.1.2 eats the last line of a command's output if you resize the window. |
| [#6127](https://gitlab.com/gnachman/iterm2/-/issues/6127) | Feature request: Safari-like tabs preview mode |
| [#6125](https://gitlab.com/gnachman/iterm2/-/issues/6125) | iTerm window does not stretch |
| [#6120](https://gitlab.com/gnachman/iterm2/-/issues/6120) | On update (download&install) Windows do not re-appear on former Space |
| [#6107](https://gitlab.com/gnachman/iterm2/-/issues/6107) | Tab color not showing exact color since update |
| [#6088](https://gitlab.com/gnachman/iterm2/-/issues/6088) | Feature Request: have applescript "create window" raise only the new window, not all windows |
| [#6087](https://gitlab.com/gnachman/iterm2/-/issues/6087) | Feature request: Possible to keep iTerm hotkey window when alert opens |
| [#6079](https://gitlab.com/gnachman/iterm2/-/issues/6079) | Slide down hotkey window animation only on top screens |
| [#6066](https://gitlab.com/gnachman/iterm2/-/issues/6066) | after update tabs with color are surrounded by ugly black-white border |
| [#6053](https://gitlab.com/gnachman/iterm2/-/issues/6053) | Tab title wiggles on BEL |
| [#6038](https://gitlab.com/gnachman/iterm2/-/issues/6038) | Tab white line problem |
| [#6026](https://gitlab.com/gnachman/iterm2/-/issues/6026) | Edit Actions windows doesn't appear |
| [#6010](https://gitlab.com/gnachman/iterm2/-/issues/6010) | Window size wrong in hotkey window when going from many back to one tab |
| [#6003](https://gitlab.com/gnachman/iterm2/-/issues/6003) | Ability to select multiple tabs (like Chrome)? |
| [#6002](https://gitlab.com/gnachman/iterm2/-/issues/6002) | Feature Request: add labels to split panes |
| [#5978](https://gitlab.com/gnachman/iterm2/-/issues/5978) | Strange double border with 3.1 beta.7 (tabs) |
| [#5938](https://gitlab.com/gnachman/iterm2/-/issues/5938) | Feature Request: New Tab/Window with current profile |
| [#5891](https://gitlab.com/gnachman/iterm2/-/issues/5891) | Touch bar "i" icon for words kills the current pane and opens a completely blank window |
| [#5856](https://gitlab.com/gnachman/iterm2/-/issues/5856) | Hotkey window shifts to the left on toggle |
| [#5854](https://gitlab.com/gnachman/iterm2/-/issues/5854) | Hotkey window on new desktop leaves empty space on top |
| [#5833](https://gitlab.com/gnachman/iterm2/-/issues/5833) | Vertical tab bar width shrinks but doesn't grow |
| [#5829](https://gitlab.com/gnachman/iterm2/-/issues/5829) | Tab list should be treated as a stack |
| [#5823](https://gitlab.com/gnachman/iterm2/-/issues/5823) | iTerm Window Style feature in profiles |
| [#5808](https://gitlab.com/gnachman/iterm2/-/issues/5808) | Prevent Creating new Windows when using "quake" dropdown mode |
| [#5796](https://gitlab.com/gnachman/iterm2/-/issues/5796) | Hotkey window now centered instead of top left corner |
| [#5773](https://gitlab.com/gnachman/iterm2/-/issues/5773) | Support the ability to programmatically split panes, run commands etc |
| [#5759](https://gitlab.com/gnachman/iterm2/-/issues/5759) | advanced setting: configurable step for keyboard controlled pane resizing |
| [#5744](https://gitlab.com/gnachman/iterm2/-/issues/5744) | Feature request: create/resize pane by percentage |
| [#5741](https://gitlab.com/gnachman/iterm2/-/issues/5741) | window arrangement should restore windows to the proper desktops |
| [#5739](https://gitlab.com/gnachman/iterm2/-/issues/5739) | Feature request: Window arrangements open in new tabs |
| [#5736](https://gitlab.com/gnachman/iterm2/-/issues/5736) | Request: Possible to make window border dimmer? |
| [#5710](https://gitlab.com/gnachman/iterm2/-/issues/5710) | Terminal becomes dimmed in fullscreen mode (3.1.beta.3) |
| [#5699](https://gitlab.com/gnachman/iterm2/-/issues/5699) | Preserve the Environment Variables when splitting panes |
| [#5694](https://gitlab.com/gnachman/iterm2/-/issues/5694) | Feature Request: Configure Dock/Application Switcher Hiding by Window |
| [#5664](https://gitlab.com/gnachman/iterm2/-/issues/5664) | with iTerm in full-screen mode, cmd+tab or hotkey switching sometimes takes me to an empty desktop |
| [#5648](https://gitlab.com/gnachman/iterm2/-/issues/5648) | [Feature request]: Define initial split ratio via OSA scripts API |
| [#5642](https://gitlab.com/gnachman/iterm2/-/issues/5642) | Windows Disappearing Forever After Disconnecting Second Display |
| [#5620](https://gitlab.com/gnachman/iterm2/-/issues/5620) | Pressing tab twice for completion leaves ugly artefacts on screen on zsh |
| [#5615](https://gitlab.com/gnachman/iterm2/-/issues/5615) | new tabs/windows don't seem to open to previous working dir, regardless of options selected or app r... |
| [#5596](https://gitlab.com/gnachman/iterm2/-/issues/5596) | After creating a new tab, it does not re-use previous session dir the second time a new tab is creat... |
| [#5589](https://gitlab.com/gnachman/iterm2/-/issues/5589) | New Tab submenu with profiles & "open all" menu item spontaneously disappeared |
| [#5588](https://gitlab.com/gnachman/iterm2/-/issues/5588) | Error on moving pane to tab. |
| [#5580](https://gitlab.com/gnachman/iterm2/-/issues/5580) | Terminal gap from topOfTheScreen on fullscreen apps |
| [#5573](https://gitlab.com/gnachman/iterm2/-/issues/5573) | Unable to switch tabs using a non-QWERTY keyboard |
| [#5570](https://gitlab.com/gnachman/iterm2/-/issues/5570) | Corrupted Text - Caused by: Full-screen mode, Vim, and Multiple Tabs |
| [#5563](https://gitlab.com/gnachman/iterm2/-/issues/5563) | Feature Request: Command K should clear all panes if broadcasting input |
| [#5561](https://gitlab.com/gnachman/iterm2/-/issues/5561) | the window of Iterm2 doesn't appear |
| [#5548](https://gitlab.com/gnachman/iterm2/-/issues/5548) | All window state lost on iTerm upgrade. |
| [#5545](https://gitlab.com/gnachman/iterm2/-/issues/5545) | Possibility to not automatically open a new default window on startup |
| [#5542](https://gitlab.com/gnachman/iterm2/-/issues/5542) | New Tab with Current Profile has illogical placement |
| [#5539](https://gitlab.com/gnachman/iterm2/-/issues/5539) | iterm2 and new windows settings (screen with cursor) |
| [#5519](https://gitlab.com/gnachman/iterm2/-/issues/5519) | Permanently visible window size |
| [#5511](https://gitlab.com/gnachman/iterm2/-/issues/5511) | Window becomes almost invisible in Mission Control when running system provided Display Profile for ... |
| [#5504](https://gitlab.com/gnachman/iterm2/-/issues/5504) | [FEATURE REQUEST] User can save and recall Tab and Profile "Layouts" |
| [#5502](https://gitlab.com/gnachman/iterm2/-/issues/5502) | [FEATURE REQUEST] Separate Clear and Clear All Buttons for Toolbelt Panels |
| [#5501](https://gitlab.com/gnachman/iterm2/-/issues/5501) | [FEATURE REQUEST] Ability to Remove Individual Entries from Toolbelt Panel |
| [#5497](https://gitlab.com/gnachman/iterm2/-/issues/5497) | [Feature Request] Keyboard shortcut to swap pane |
| [#5447](https://gitlab.com/gnachman/iterm2/-/issues/5447) | Pop-up box for choosing colors appears underneath the Preferences window |
| [#5441](https://gitlab.com/gnachman/iterm2/-/issues/5441) | Window Arrangements point to wrong working dir |
| [#5439](https://gitlab.com/gnachman/iterm2/-/issues/5439) | Content of window / tab goes "blank" (ALL white) in certain (unknown) circumstances and cannot be re... |
| [#5428](https://gitlab.com/gnachman/iterm2/-/issues/5428) | [Feature Request] - Automatically set margins based on window size (see comment below) |
| [#5422](https://gitlab.com/gnachman/iterm2/-/issues/5422) | Feature request: Add option to show per-pane title bar even when there is only one pane. |
| [#5406](https://gitlab.com/gnachman/iterm2/-/issues/5406) | Terminal always on top when switching windows, starts a extra visor on hotkey press |
| [#5395](https://gitlab.com/gnachman/iterm2/-/issues/5395) | Don't resize non-selected tabs on window resize until the tab gets selected |
| [#5384](https://gitlab.com/gnachman/iterm2/-/issues/5384) | Text selection does not work often in Split Pane mode |
| [#5363](https://gitlab.com/gnachman/iterm2/-/issues/5363) | DashTerm2 should allow merging of open windows |
| [#5358](https://gitlab.com/gnachman/iterm2/-/issues/5358) | Suggestion for the Window tab: Instead of "bash" for most windows, give the "pwd" directory name |
| [#5344](https://gitlab.com/gnachman/iterm2/-/issues/5344) | Window loses focus after a running a terminal based program (vim, emacs etc.) |
| [#5341](https://gitlab.com/gnachman/iterm2/-/issues/5341) | splitview window resize and restore doesnt restore the terminal content. |
| [#5338](https://gitlab.com/gnachman/iterm2/-/issues/5338) | createTabWithDefaultProfileCommand return ni |
| [#5337](https://gitlab.com/gnachman/iterm2/-/issues/5337) | Dragging a tab out of the tab bar should also drag the tabview, revealing another tab beneath it, ju... |
| [#5315](https://gitlab.com/gnachman/iterm2/-/issues/5315) | Hide overlay window when mission control is invoked |
| [#5292](https://gitlab.com/gnachman/iterm2/-/issues/5292) | Duplicated / Garbled Text when prompt is longer than window width |
| [#5263](https://gitlab.com/gnachman/iterm2/-/issues/5263) | iTerm temporarily halted and then all windows resized |
| [#5260](https://gitlab.com/gnachman/iterm2/-/issues/5260) | Other iTerm windows steal focus from visor window |
| [#5258](https://gitlab.com/gnachman/iterm2/-/issues/5258) | Minimized windows restored in a strange state on startup |
| [#5256](https://gitlab.com/gnachman/iterm2/-/issues/5256) | Feature request: tab names should be persistent if entered manually, and without any automatic addit... |
| [#5255](https://gitlab.com/gnachman/iterm2/-/issues/5255) | Feature Request Chomeless terminal window |
| [#5253](https://gitlab.com/gnachman/iterm2/-/issues/5253) | Mouse offset when using vim with multiple splits. Clicking on a pane in one place selects a pane in ... |
| [#5251](https://gitlab.com/gnachman/iterm2/-/issues/5251) | RFE: preference item requesting that available colors be distributed automatically across tabs (firs... |
| [#5250](https://gitlab.com/gnachman/iterm2/-/issues/5250) | Opening a new Tab/Window with "Previous Directory" support seems not to work anymore |
| [#5242](https://gitlab.com/gnachman/iterm2/-/issues/5242) | When a local directory is available use a represented file in the window title |
| [#5236](https://gitlab.com/gnachman/iterm2/-/issues/5236) | Hotkey window shoud open in current space and screen (following the mouse) |
| [#5221](https://gitlab.com/gnachman/iterm2/-/issues/5221) | Suggestion: Shortcut for "Next Tab" should be ⌥⌘→ instead of ⌘→ to be consistent with browser |
| [#5213](https://gitlab.com/gnachman/iterm2/-/issues/5213) | Window title not retained when switching tabs |
| [#5204](https://gitlab.com/gnachman/iterm2/-/issues/5204) | Tab key no longer expands aliases |
| [#5200](https://gitlab.com/gnachman/iterm2/-/issues/5200) | On smaller splits, indicator behave crazy |
| [#5196](https://gitlab.com/gnachman/iterm2/-/issues/5196) | Center terminal grid when using "Terminal windows resize smoothly" |
| [#5160](https://gitlab.com/gnachman/iterm2/-/issues/5160) | Iterm2 window focus bug with show/hide iterm2 with system-wide hotkey settings |
| [#5151](https://gitlab.com/gnachman/iterm2/-/issues/5151) | Terminal Windows not persisted across a software update |
| [#5149](https://gitlab.com/gnachman/iterm2/-/issues/5149) | Hotkey Window completely broken in iTerm Build 3.0.20160823-nightly |
| [#5107](https://gitlab.com/gnachman/iterm2/-/issues/5107) | split view resize requires terminal reset |
| [#5088](https://gitlab.com/gnachman/iterm2/-/issues/5088) | Hotkey window hides when focus is lost no matter on setting |
| [#5082](https://gitlab.com/gnachman/iterm2/-/issues/5082) | Flycut pasting going to wrong DashTerm2 window |
| [#5079](https://gitlab.com/gnachman/iterm2/-/issues/5079) | Color picker in fullscreen not accessible |
| [#5035](https://gitlab.com/gnachman/iterm2/-/issues/5035) | weird extra dots in full screen splits |
| [#5013](https://gitlab.com/gnachman/iterm2/-/issues/5013) | Feature request: wider grab-able/pane-resize-handle area |
| [#5008](https://gitlab.com/gnachman/iterm2/-/issues/5008) | hotkey window show on diffrent display |
| [#4999](https://gitlab.com/gnachman/iterm2/-/issues/4999) | window title doesn't appear correctly |
| [#4984](https://gitlab.com/gnachman/iterm2/-/issues/4984) | Right clicking on empty space in Tab bar should bring up "New Tab Menu" |
| [#4946](https://gitlab.com/gnachman/iterm2/-/issues/4946) | iterm 3 resizes the window on its own |
| [#4908](https://gitlab.com/gnachman/iterm2/-/issues/4908) | System wide hotkey should only show/hide window on current monitor of active monitor |
| [#4899](https://gitlab.com/gnachman/iterm2/-/issues/4899) | Reorder windows |
| [#4895](https://gitlab.com/gnachman/iterm2/-/issues/4895) | zsh/zpty process makes DashTerm2 ask for confimation when closing a tab |
| [#4875](https://gitlab.com/gnachman/iterm2/-/issues/4875) | Feature request: Improve navigation of the vertical/horizontal split modes in the right click menu u... |
| [#4855](https://gitlab.com/gnachman/iterm2/-/issues/4855) | Go back to MRU tab when one closes |
| [#4852](https://gitlab.com/gnachman/iterm2/-/issues/4852) | Cmd-Shift-O should close "Open Quickly" window |
| [#4851](https://gitlab.com/gnachman/iterm2/-/issues/4851) | Fullscreen mode tabbing obscures content |
| [#4848](https://gitlab.com/gnachman/iterm2/-/issues/4848) | Feature request: disable tab drag, and/or drag window into tab |
| [#4840](https://gitlab.com/gnachman/iterm2/-/issues/4840) | Animated loading icon causes shift in tab UI |
| [#4839](https://gitlab.com/gnachman/iterm2/-/issues/4839) | New tabs in hotkey window get default (not hotkey window) profile |
| [#4803](https://gitlab.com/gnachman/iterm2/-/issues/4803) | Tab bar doesn't appear when pressing Command in full screen |
| [#4793](https://gitlab.com/gnachman/iterm2/-/issues/4793) | Rearranging tabs does not reconfigure shortcuts associated with the tabs |
| [#4782](https://gitlab.com/gnachman/iterm2/-/issues/4782) | Windows & tabs merging |
| [#4776](https://gitlab.com/gnachman/iterm2/-/issues/4776) | password manager window does not appear when terminal is open |
| [#4749](https://gitlab.com/gnachman/iterm2/-/issues/4749) | Initial "Restore Windows Arrangement" wrong. |
| [#4724](https://gitlab.com/gnachman/iterm2/-/issues/4724) | Cannot open new tabs in Iterm2 |
| [#4711](https://gitlab.com/gnachman/iterm2/-/issues/4711) | Pasting with "Convert tabs to spaces" does not respect tabstops |
| [#4693](https://gitlab.com/gnachman/iterm2/-/issues/4693) | Full screen + split screen doesn't work anymore |
| [#4659](https://gitlab.com/gnachman/iterm2/-/issues/4659) | Separate "Hide tab number" and "Hide tab close button" |
| [#4647](https://gitlab.com/gnachman/iterm2/-/issues/4647) | Enhancement: Add a save terminal/window/tab output |
| [#4632](https://gitlab.com/gnachman/iterm2/-/issues/4632) | Wrong window selected in multi-monitor setup |
| [#4598](https://gitlab.com/gnachman/iterm2/-/issues/4598) | Allow URL scheme so that a hyperlink can open an DashTerm2 tab |
| [#4594](https://gitlab.com/gnachman/iterm2/-/issues/4594) | [Suggestion] Parent Window Title for Split Pane Windows |
| [#4575](https://gitlab.com/gnachman/iterm2/-/issues/4575) | [BUG] iTerm windows become invisible. |
| [#4555](https://gitlab.com/gnachman/iterm2/-/issues/4555) | Feature Request - Allow pop-up window to overlay over full screen apps |
| [#4545](https://gitlab.com/gnachman/iterm2/-/issues/4545) | Feature request: update window arrangement |
| [#4541](https://gitlab.com/gnachman/iterm2/-/issues/4541) | Focused window is not maintained through cmd-tab out and back in |
| [#4535](https://gitlab.com/gnachman/iterm2/-/issues/4535) | Update to oh-my-zsh causes hotkey to no longer hide hotkey window. |
| [#4474](https://gitlab.com/gnachman/iterm2/-/issues/4474) | Window vertically off-screen when OSX dock hiding is enabled |
| [#4424](https://gitlab.com/gnachman/iterm2/-/issues/4424) | Could not open any window anymore |
| [#4408](https://gitlab.com/gnachman/iterm2/-/issues/4408) | Undo Close on panel reopens the panel, but can't receive input |
| [#4372](https://gitlab.com/gnachman/iterm2/-/issues/4372) | Feature: Make "Show profile name (in tab/window)" a per-profile setting |
| [#4329](https://gitlab.com/gnachman/iterm2/-/issues/4329) | Focus Follow Mouse ignore preference window |
| [#4316](https://gitlab.com/gnachman/iterm2/-/issues/4316) | Silence bell tooltip window reappearing and not accepting actions on button press |
| [#4315](https://gitlab.com/gnachman/iterm2/-/issues/4315) | Zooming resizes window |
| [#4271](https://gitlab.com/gnachman/iterm2/-/issues/4271) | Feature proposal: an option to have the toolbelt and the tabs bar in the same window column |
| [#4207](https://gitlab.com/gnachman/iterm2/-/issues/4207) | Can resize split pan until per-pane title bar is clicked |
| [#4179](https://gitlab.com/gnachman/iterm2/-/issues/4179) | Close all the panes in the current tab except the focused pane |
| [#4142](https://gitlab.com/gnachman/iterm2/-/issues/4142) | focus "Quit DashTerm2? dialog window after starting "Install & Relaunch" |
| [#4086](https://gitlab.com/gnachman/iterm2/-/issues/4086) | Feature request: a right-click menu on the tab bar. |
| [#4069](https://gitlab.com/gnachman/iterm2/-/issues/4069) | "center" fullscreen mode |
| [#4020](https://gitlab.com/gnachman/iterm2/-/issues/4020) | Tab bar on right |
| [#3981](https://gitlab.com/gnachman/iterm2/-/issues/3981) | Switching tabs using Command-[arrow-key] is broken |
| [#3977](https://gitlab.com/gnachman/iterm2/-/issues/3977) | Window loses focus when clicked. |
| [#3938](https://gitlab.com/gnachman/iterm2/-/issues/3938) | Consider delayed resize of inactive tabs |
| [#3926](https://gitlab.com/gnachman/iterm2/-/issues/3926) | multiple layered windows |
| [#3924](https://gitlab.com/gnachman/iterm2/-/issues/3924) | Window blur issue on screenshot |
| [#3920](https://gitlab.com/gnachman/iterm2/-/issues/3920) | Cmd-Tab from another application switches to a different space |
| [#3873](https://gitlab.com/gnachman/iterm2/-/issues/3873) | Feature request: Exportable/publishable and importable/subscribable/mergable trigger lists |
| [#3856](https://gitlab.com/gnachman/iterm2/-/issues/3856) | Option to allow new panes to inherit title of 'parent' pane |
| [#3830](https://gitlab.com/gnachman/iterm2/-/issues/3830) | Feature request: remember last window size |
| [#3793](https://gitlab.com/gnachman/iterm2/-/issues/3793) | Fixed pane on each tab |
| [#3754](https://gitlab.com/gnachman/iterm2/-/issues/3754) | Fullscreen focus issue when switching spaces |
| [#3743](https://gitlab.com/gnachman/iterm2/-/issues/3743) | Feature request: Automatic/dynamic resizing of panes - active pane increases in size |
| [#3732](https://gitlab.com/gnachman/iterm2/-/issues/3732) | Feature Request: [Keys] Map a separate key as a "Close hotkey window" |
| [#3719](https://gitlab.com/gnachman/iterm2/-/issues/3719) | Add ability to search for settings in preferences window  |
| [#3714](https://gitlab.com/gnachman/iterm2/-/issues/3714) | Transparency and blur stops working when NSFullSizeContentViewWindowMask is set |
| [#3676](https://gitlab.com/gnachman/iterm2/-/issues/3676) | Dead DashTerm2 windows. |
| [#3674](https://gitlab.com/gnachman/iterm2/-/issues/3674) | New tabs in the hotkey window don't use the hotkey window profile |
| [#3658](https://gitlab.com/gnachman/iterm2/-/issues/3658) | Adding a new Tabbar Style: Flat |
| [#3643](https://gitlab.com/gnachman/iterm2/-/issues/3643) |  Programmatically enable/disable and set/unset the name of window and job. |
| [#3640](https://gitlab.com/gnachman/iterm2/-/issues/3640) | allow middle click paste to also give window focus |
| [#3626](https://gitlab.com/gnachman/iterm2/-/issues/3626) | Window gap on LHS of display on OSX 10.11 Beta |
| [#3589](https://gitlab.com/gnachman/iterm2/-/issues/3589) | Remember size & position of Profiles window, and whether tags were expanded. |
| [#3576](https://gitlab.com/gnachman/iterm2/-/issues/3576) | Add the option to kill child processes with signal 9 [was: Processes not killed when tabs are closed... |
| [#3547](https://gitlab.com/gnachman/iterm2/-/issues/3547) | Add button for new tab |
| [#3536](https://gitlab.com/gnachman/iterm2/-/issues/3536) | Feature request: A "new window like this" right-click menu item |
| [#3488](https://gitlab.com/gnachman/iterm2/-/issues/3488) | multi-column or window display  for one session/screen |
| [#3450](https://gitlab.com/gnachman/iterm2/-/issues/3450) | Maximized window resizes when closing all but the first tab |
| [#3445](https://gitlab.com/gnachman/iterm2/-/issues/3445) | Show Hide Terminal window should have option to follow mouse focus |
| [#3430](https://gitlab.com/gnachman/iterm2/-/issues/3430) | Split window without resizing other parts |
| [#3417](https://gitlab.com/gnachman/iterm2/-/issues/3417) | Profile->Window->Style->Fullscreen is a little bit from satisfying |
| [#3392](https://gitlab.com/gnachman/iterm2/-/issues/3392) | Permanently disable tab bar |
| [#3380](https://gitlab.com/gnachman/iterm2/-/issues/3380) | Use Transparency does nothing and is confusing when window doesn't have transparency set. [was: tran... |
| [#3372](https://gitlab.com/gnachman/iterm2/-/issues/3372) | Broadcast Input to multiple but not all tabs |
| [#3223](https://gitlab.com/gnachman/iterm2/-/issues/3223) | Center terminal window in full-screen mode |
| [#3215](https://gitlab.com/gnachman/iterm2/-/issues/3215) | AppleScript possibility to set background image (per tab) |
| [#3192](https://gitlab.com/gnachman/iterm2/-/issues/3192) | Scripting: add reverse hierarchy accessors for windows/tabs/sessions. |
| [#3184](https://gitlab.com/gnachman/iterm2/-/issues/3184) | 'Sticky' tabs |
| [#3167](https://gitlab.com/gnachman/iterm2/-/issues/3167) | Let background image stretch to fit whole window instead of just one pane |
| [#3166](https://gitlab.com/gnachman/iterm2/-/issues/3166) | Profiles > Window > Style: Zoom/Maximized missing. |
| [#3139](https://gitlab.com/gnachman/iterm2/-/issues/3139) | Show which tabs will alert to close |
| [#3137](https://gitlab.com/gnachman/iterm2/-/issues/3137) | restore window arrange should bring back last command used (profile option) |
| [#3116](https://gitlab.com/gnachman/iterm2/-/issues/3116) | Command-Backtick (Cmd-Tilde) doesn't work when fullscreen |
| [#3090](https://gitlab.com/gnachman/iterm2/-/issues/3090) | Make iTerm remember which terminal window/tab was selected when switching back from another app |
| [#3070](https://gitlab.com/gnachman/iterm2/-/issues/3070) | ability to perform different searches in different panes |
| [#3023](https://gitlab.com/gnachman/iterm2/-/issues/3023) | random appearance setting for new tab |
| [#2990](https://gitlab.com/gnachman/iterm2/-/issues/2990) | set a title to a window which is grouping a set of tabs |
| [#2942](https://gitlab.com/gnachman/iterm2/-/issues/2942) | Windows sized wrong after post-panic reopen |
| [#2871](https://gitlab.com/gnachman/iterm2/-/issues/2871) | Detached coloured tab is missing uncolouring |
| [#2857](https://gitlab.com/gnachman/iterm2/-/issues/2857) | Allow naming of Windows |
| [#2835](https://gitlab.com/gnachman/iterm2/-/issues/2835) | Incorrect handling of window title sequence |
| [#2668](https://gitlab.com/gnachman/iterm2/-/issues/2668) | Window Arrangement Title |
| [#2600](https://gitlab.com/gnachman/iterm2/-/issues/2600) | Shortcut key to activate tab also cycles through panes |
| [#2541](https://gitlab.com/gnachman/iterm2/-/issues/2541) | Support tab groups like Firefox |
| [#2495](https://gitlab.com/gnachman/iterm2/-/issues/2495) | Wallpaper over all splits |
| [#2368](https://gitlab.com/gnachman/iterm2/-/issues/2368) | Tabs in slit panels |
| [#2353](https://gitlab.com/gnachman/iterm2/-/issues/2353) | Text background color that overrides generic window background transparency |
| [#2265](https://gitlab.com/gnachman/iterm2/-/issues/2265) | Window arrangements opened at launch are two columns wider than they ought to be. |
| [#2234](https://gitlab.com/gnachman/iterm2/-/issues/2234) | Add Chrome's Pin Tab Feature |
| [#2195](https://gitlab.com/gnachman/iterm2/-/issues/2195) | Switch to disable circular pane selection. |
| [#2194](https://gitlab.com/gnachman/iterm2/-/issues/2194) | ability cycle through windows in pre-determined order |
| [#2140](https://gitlab.com/gnachman/iterm2/-/issues/2140) | 'alt-tab' among windows within only the current display |
| [#2127](https://gitlab.com/gnachman/iterm2/-/issues/2127) | a fullscreen mode that would work well with transparency |
| [#2107](https://gitlab.com/gnachman/iterm2/-/issues/2107) | show dimensions of all panes when reizing |
| [#2074](https://gitlab.com/gnachman/iterm2/-/issues/2074) | Option to preserve overall window size when toggling tabs |
| [#2003](https://gitlab.com/gnachman/iterm2/-/issues/2003) | 'Show border around window' menu and hotkey |
| [#1996](https://gitlab.com/gnachman/iterm2/-/issues/1996) | iterm2 window needs to be resized in order to see enough of title bar to move window after disconnec... |
| [#1890](https://gitlab.com/gnachman/iterm2/-/issues/1890) | Tab Groups |
| [#1852](https://gitlab.com/gnachman/iterm2/-/issues/1852) | Method of determining width of tab title text-field |
| [#1743](https://gitlab.com/gnachman/iterm2/-/issues/1743) | Color active pane border different from inactive ones |
| [#1728](https://gitlab.com/gnachman/iterm2/-/issues/1728) | Page up, page down, home, end in profiles window |
| [#1708](https://gitlab.com/gnachman/iterm2/-/issues/1708) | Perform text selection with keyboard [was: keyboard movement within a pane] |
| [#1706](https://gitlab.com/gnachman/iterm2/-/issues/1706) | Open new window in same monitor |
| [#1698](https://gitlab.com/gnachman/iterm2/-/issues/1698) | Offer a "Cascade" arrangement of windows |
| [#1649](https://gitlab.com/gnachman/iterm2/-/issues/1649) | "New window like this" command, from right-click menu |
| [#1577](https://gitlab.com/gnachman/iterm2/-/issues/1577) | Switching to full screen discards input while window is opening |
| [#1529](https://gitlab.com/gnachman/iterm2/-/issues/1529) | Possibility to have opaque selection in transparent window |
| [#1343](https://gitlab.com/gnachman/iterm2/-/issues/1343) | GNOME-ish command-click mouse move/resize of panes |
| [#1288](https://gitlab.com/gnachman/iterm2/-/issues/1288) | Split panes in Profiles |
| [#1264](https://gitlab.com/gnachman/iterm2/-/issues/1264) | Enable a "master pane/area" like a tiling wm (e.g xmonad) |
| [#1201](https://gitlab.com/gnachman/iterm2/-/issues/1201) | Terminal window bottom corners are incorrect under Lion |
| [#1135](https://gitlab.com/gnachman/iterm2/-/issues/1135) | Use background of terminal tabs as a progress bar |
| [#1071](https://gitlab.com/gnachman/iterm2/-/issues/1071) | Hotkey Window new tabs should also use Hotkey Window profile. |
| [#1045](https://gitlab.com/gnachman/iterm2/-/issues/1045) | Smart window placement works great... on the first monitor |
| [#1043](https://gitlab.com/gnachman/iterm2/-/issues/1043) | Key shortcut for "command-K for all splits on current tab" |
| [#1018](https://gitlab.com/gnachman/iterm2/-/issues/1018) | WMII Like movement of split panes  |
| [#1005](https://gitlab.com/gnachman/iterm2/-/issues/1005) | Add help button to config panels that opens the html help docs at the appropriate location |
| [#998](https://gitlab.com/gnachman/iterm2/-/issues/998) | Warn if command is login without -l argument and you've specified a custom dir [was: reuse previousl... |
| [#810](https://gitlab.com/gnachman/iterm2/-/issues/810) | NON CRITICAL - Colorspace conversion locked at window launch; no device colors |
| [#694](https://gitlab.com/gnachman/iterm2/-/issues/694) | Merge all windows |
| [#611](https://gitlab.com/gnachman/iterm2/-/issues/611) | "Chrome Confirm to quit" for closing windows |
| [#526](https://gitlab.com/gnachman/iterm2/-/issues/526) | move panes around |
| [#276](https://gitlab.com/gnachman/iterm2/-/issues/276) | Native fullscreen transparency should show vibrant desktop background (was: No transparency in full ... |

---

## Keyboard and Input (P2)

**Count:** 266

| Issue | Title |
|-------|-------|
| [#12649](https://gitlab.com/gnachman/iterm2/-/issues/12649) | Feature Request: new option for session title |
| [#12636](https://gitlab.com/gnachman/iterm2/-/issues/12636) | Allow multi-step keyboard shortcuts |
| [#12545](https://gitlab.com/gnachman/iterm2/-/issues/12545) | ctrl-1 sends 1 instead |
| [#12525](https://gitlab.com/gnachman/iterm2/-/issues/12525) | Hotkeys not working in languages other than English |
| [#12415](https://gitlab.com/gnachman/iterm2/-/issues/12415) | Caret does not move until after I release an arrow key / backspace |
| [#12404](https://gitlab.com/gnachman/iterm2/-/issues/12404) | Key repeat stops working after a while on macOS Tahoe |
| [#12363](https://gitlab.com/gnachman/iterm2/-/issues/12363) | 3.5.x conflict with "exec "set <M-".a:key.">=\e".a:key" in vi |
| [#12328](https://gitlab.com/gnachman/iterm2/-/issues/12328) | [Feature Request] session logging can be opened or closed for the specified session through shortcut... |
| [#12284](https://gitlab.com/gnachman/iterm2/-/issues/12284) | Ctrl + 1 Degrades over time (in neovim but i think also outside of neovim) |
| [#12282](https://gitlab.com/gnachman/iterm2/-/issues/12282) | support differentiation of left ctrl and right ctrl in key mappings |
| [#12256](https://gitlab.com/gnachman/iterm2/-/issues/12256) | Navigation keys broken on macOS |
| [#12232](https://gitlab.com/gnachman/iterm2/-/issues/12232) | Shortcut Ctrl-Shift-= does not Zoom after 3.5.12 upgrade |
| [#12225](https://gitlab.com/gnachman/iterm2/-/issues/12225) | Terminal starts listening for keypresses globally even when not focused after using aerc |
| [#12195](https://gitlab.com/gnachman/iterm2/-/issues/12195) | `option+Click moves cursor` failed |
| [#12187](https://gitlab.com/gnachman/iterm2/-/issues/12187) | Application does not obey ctrl+arrows to skip over word left and right |
| [#12184](https://gitlab.com/gnachman/iterm2/-/issues/12184) | Synergy 3 and iTerm some keys have stopped working |
| [#12183](https://gitlab.com/gnachman/iterm2/-/issues/12183) | Cannot use new custom system keybindings |
| [#12173](https://gitlab.com/gnachman/iterm2/-/issues/12173) | Mapping of keys not working |
| [#12163](https://gitlab.com/gnachman/iterm2/-/issues/12163) | The Ctrl+C interrupt (SIGINT) stop working |
| [#11913](https://gitlab.com/gnachman/iterm2/-/issues/11913) | Ctrl+D EOF not being sent to shell |
| [#11883](https://gitlab.com/gnachman/iterm2/-/issues/11883) | Keypad Enter always gives ^M, even in application keypad mode |
| [#11857](https://gitlab.com/gnachman/iterm2/-/issues/11857) | After using hotkey, terminal is not in focus |
| [#11833](https://gitlab.com/gnachman/iterm2/-/issues/11833) | alt return problem ~3 |
| [#11806](https://gitlab.com/gnachman/iterm2/-/issues/11806) | Function keys stop working in vim intermittently |
| [#11773](https://gitlab.com/gnachman/iterm2/-/issues/11773) | Intermittently, all Control keys get turned into escape sequence (e.g.: ctrl-a -> ^[[27;5;97~). Occu... |
| [#11757](https://gitlab.com/gnachman/iterm2/-/issues/11757) | Cannot disable default `Session > 'Open Autocomplete'` command shortcut (<D-;>). |
| [#11753](https://gitlab.com/gnachman/iterm2/-/issues/11753) | iterm2 nightly build option key as Alt key not working |
| [#11730](https://gitlab.com/gnachman/iterm2/-/issues/11730) | Cannot view command history in composer with german keyboard |
| [#11709](https://gitlab.com/gnachman/iterm2/-/issues/11709) | Hyper key not working properly |
| [#11462](https://gitlab.com/gnachman/iterm2/-/issues/11462) | Sparkle DSA key |
| [#11447](https://gitlab.com/gnachman/iterm2/-/issues/11447) | Feature request: searching key mappings by pressing a key or key combination |
| [#11356](https://gitlab.com/gnachman/iterm2/-/issues/11356) | Force keyboard works only with "Automatically switch to document's input source" off |
| [#11348](https://gitlab.com/gnachman/iterm2/-/issues/11348) | Press `control+c` too fast will also send a single `control` key input |
| [#11212](https://gitlab.com/gnachman/iterm2/-/issues/11212) | DashTerm2 on OSX Sonoma doesn't correctly remap keys when Command and Caps are swapped. |
| [#11129](https://gitlab.com/gnachman/iterm2/-/issues/11129) | hot key no longer works in macOS Sonoma |
| [#11068](https://gitlab.com/gnachman/iterm2/-/issues/11068) | Software update on restart option |
| [#11050](https://gitlab.com/gnachman/iterm2/-/issues/11050) | Shortcuts > Actions [paste: replace] does not work |
| [#11036](https://gitlab.com/gnachman/iterm2/-/issues/11036) | Alt key doesn't work. |
| [#11025](https://gitlab.com/gnachman/iterm2/-/issues/11025) | Support for Cascadia Code's SS01 cursive option (only defined on italic variants) |
| [#10978](https://gitlab.com/gnachman/iterm2/-/issues/10978) | Block keyboard. Prevent accidental CRTL-C |
| [#10968](https://gitlab.com/gnachman/iterm2/-/issues/10968) | Iterm2 is still running, but  unable to operate via mouse, keyboard |
| [#10961](https://gitlab.com/gnachman/iterm2/-/issues/10961) | Semantic history for docker-altered paths. |
| [#10917](https://gitlab.com/gnachman/iterm2/-/issues/10917) | Cmd + K to clear screen - can take upto 15 minutes to respond |
| [#10908](https://gitlab.com/gnachman/iterm2/-/issues/10908) | Add a keyboard shortcut to jump between multiple markers |
| [#10874](https://gitlab.com/gnachman/iterm2/-/issues/10874) | Toggleterm key mapping not working in Neovim |
| [#10869](https://gitlab.com/gnachman/iterm2/-/issues/10869) | Typing CTRL+C does not work, and the text ^[[99;5U appears on the screen. |
| [#10850](https://gitlab.com/gnachman/iterm2/-/issues/10850) | After any app comes from full screen, DashTerm2 hotkey makes OSX navigate to the specific virtual deskt... |
| [#10795](https://gitlab.com/gnachman/iterm2/-/issues/10795) | About secure keyboard entry |
| [#10640](https://gitlab.com/gnachman/iterm2/-/issues/10640) | Compound key mappings |
| [#10622](https://gitlab.com/gnachman/iterm2/-/issues/10622) | DashTerm2 default key mapping kinda sucks |
| [#10616](https://gitlab.com/gnachman/iterm2/-/issues/10616) | Space doesn't work when Full Keyboard Access is enabled on Mac |
| [#10591](https://gitlab.com/gnachman/iterm2/-/issues/10591) | Password Manager fails when Delete is bound to ctrl+h |
| [#10566](https://gitlab.com/gnachman/iterm2/-/issues/10566) | iterm2 start breaks existing usb hid keys remapping for other apps |
| [#10553](https://gitlab.com/gnachman/iterm2/-/issues/10553) | Replay DashTerm2 logs at realtime / fast forward / etc |
| [#10552](https://gitlab.com/gnachman/iterm2/-/issues/10552) | Keymappings for vim messed up in 3.4.16 |
| [#10550](https://gitlab.com/gnachman/iterm2/-/issues/10550) | Toggle "Enable Mouse Reporting" via Custom Key Binding not working |
| [#10543](https://gitlab.com/gnachman/iterm2/-/issues/10543) | DashTerm2 locks up.. won't respond to keyboard. |
| [#10429](https://gitlab.com/gnachman/iterm2/-/issues/10429) | CMD-C cut from one Mac app to CMD-V paste into iTerm doesn't function properly |
| [#10327](https://gitlab.com/gnachman/iterm2/-/issues/10327) | Disable Secure keyboard entry for specific apps |
| [#10316](https://gitlab.com/gnachman/iterm2/-/issues/10316) | Global hotkey no longer activates iterm main menu |
| [#10264](https://gitlab.com/gnachman/iterm2/-/issues/10264) | custom icon option |
| [#10239](https://gitlab.com/gnachman/iterm2/-/issues/10239) | zsh + starship -> moving mark breaks text input |
| [#10233](https://gitlab.com/gnachman/iterm2/-/issues/10233) | Quick Terminal shortcut in the style of Apple Quick Note |
| [#10171](https://gitlab.com/gnachman/iterm2/-/issues/10171) | Multi-lines / multi-cursors command input |
| [#10164](https://gitlab.com/gnachman/iterm2/-/issues/10164) | Shift-arrow selection completely disabled when "Automatically enter copy mode on Shift+Arrow key" is... |
| [#10132](https://gitlab.com/gnachman/iterm2/-/issues/10132) | Compound Profile Shortcut Keys |
| [#10063](https://gitlab.com/gnachman/iterm2/-/issues/10063) | Feature: screenkey-like screencasting feature for DashTerm2 that shows all keystrokes pressed by the us... |
| [#10054](https://gitlab.com/gnachman/iterm2/-/issues/10054) | Input line broken in a shell with fish and starship |
| [#10033](https://gitlab.com/gnachman/iterm2/-/issues/10033) | Can we get shortcuts integration for macOS Monterey? |
| [#10029](https://gitlab.com/gnachman/iterm2/-/issues/10029) | Control keys stop working in Vim in CSI u mode |
| [#10011](https://gitlab.com/gnachman/iterm2/-/issues/10011) | In some cases, the input command interface is confusing |
| [#10005](https://gitlab.com/gnachman/iterm2/-/issues/10005) | MacOS Monterey, iTerm kills Alfred hot-key |
| [#9995](https://gitlab.com/gnachman/iterm2/-/issues/9995) | after connect db in iterms2 ,I can't see inputing sql |
| [#9966](https://gitlab.com/gnachman/iterm2/-/issues/9966) | Add Options to Snippets |
| [#9921](https://gitlab.com/gnachman/iterm2/-/issues/9921) | Broadcast Input Hotkey NOT Disabling Broadcast Input |
| [#9913](https://gitlab.com/gnachman/iterm2/-/issues/9913) | invisible command line after ctrl+c from nslookup |
| [#9910](https://gitlab.com/gnachman/iterm2/-/issues/9910) | Separate key bindings for keypad in application mode |
| [#9859](https://gitlab.com/gnachman/iterm2/-/issues/9859) | Left Opt as Meta Doesn't Work on External Keyboard w/ MacOS Big Sur |
| [#9835](https://gitlab.com/gnachman/iterm2/-/issues/9835) | Using an alternative package manager for Python scripting |
| [#9610](https://gitlab.com/gnachman/iterm2/-/issues/9610) | option-click to move cursor doesn't work on multi-line commands |
| [#9594](https://gitlab.com/gnachman/iterm2/-/issues/9594) | provide python api access to session properties like "Session -> Terminal State -> Alternate Screen" |
| [#9572](https://gitlab.com/gnachman/iterm2/-/issues/9572) | Option to add a stopwatch in title bar to track time taken to run a command |
| [#9481](https://gitlab.com/gnachman/iterm2/-/issues/9481) | strange input mode in asian keyboard layout |
| [#9419](https://gitlab.com/gnachman/iterm2/-/issues/9419) | HISTCONTROL variable should not be altered |
| [#9363](https://gitlab.com/gnachman/iterm2/-/issues/9363) | Cannot navigate quit dialog with keyboard |
| [#9310](https://gitlab.com/gnachman/iterm2/-/issues/9310) | Ctrl-V Ctrl-M in vi not working |
| [#9309](https://gitlab.com/gnachman/iterm2/-/issues/9309) | Feature request: Option to add a customized frame around content in profile |
| [#9282](https://gitlab.com/gnachman/iterm2/-/issues/9282) | [Suggestion] Allow disabling Triggers in alternate screen |
| [#9212](https://gitlab.com/gnachman/iterm2/-/issues/9212) | async_send_text and sending special keys |
| [#9172](https://gitlab.com/gnachman/iterm2/-/issues/9172) | Modifier keys remapping seems to work globally |
| [#9110](https://gitlab.com/gnachman/iterm2/-/issues/9110) | Keyboard shortcuts in Vim require shift key to work |
| [#9092](https://gitlab.com/gnachman/iterm2/-/issues/9092) | Feature Request: Per Job Keymappings |
| [#9077](https://gitlab.com/gnachman/iterm2/-/issues/9077) | Output of ReportCellSize is not correct when vi mode is set in ~/.inputrc |
| [#8995](https://gitlab.com/gnachman/iterm2/-/issues/8995) | Ctrl + Space now sends C-@ |
| [#8984](https://gitlab.com/gnachman/iterm2/-/issues/8984) | Ctrl + Num in Navigation Shortcuts Setting Not Available? |
| [#8941](https://gitlab.com/gnachman/iterm2/-/issues/8941) | Request: allow copy mode key to be customized |
| [#8915](https://gitlab.com/gnachman/iterm2/-/issues/8915) | Ctrl-C doesn't work. |
| [#8904](https://gitlab.com/gnachman/iterm2/-/issues/8904) | Support fn2 key |
| [#8874](https://gitlab.com/gnachman/iterm2/-/issues/8874) | Modifier keys not swapping on keyboard attached to Mac |
| [#8838](https://gitlab.com/gnachman/iterm2/-/issues/8838) | Opacity setting is not preserved when switching with key bindings |
| [#8837](https://gitlab.com/gnachman/iterm2/-/issues/8837) | cmd-click to edit REMOTE files |
| [#8825](https://gitlab.com/gnachman/iterm2/-/issues/8825) | "Show Mark Indicators" option is turned off but the indicators still show up after restart |
| [#8790](https://gitlab.com/gnachman/iterm2/-/issues/8790) | iTerm 2 Profile>Keys>Option Key settings swaps left/right when modifier keys are swapped in System P... |
| [#8775](https://gitlab.com/gnachman/iterm2/-/issues/8775) | Touch bar adding key bindings do not work |
| [#8744](https://gitlab.com/gnachman/iterm2/-/issues/8744) | Distinguish "Natural Text Editing" key preset between bash and zshell? |
| [#8738](https://gitlab.com/gnachman/iterm2/-/issues/8738) | Caps lock prevents double-tap Hotkey from triggering |
| [#8680](https://gitlab.com/gnachman/iterm2/-/issues/8680) | [Feature request] TouchBar widget with Ctrl+R option |
| [#8660](https://gitlab.com/gnachman/iterm2/-/issues/8660) | [Feature Request] Add plist option for key presets |
| [#8591](https://gitlab.com/gnachman/iterm2/-/issues/8591) | cmd key support (in neovim?) |
| [#8577](https://gitlab.com/gnachman/iterm2/-/issues/8577) | Hotkey regex search not working |
| [#8575](https://gitlab.com/gnachman/iterm2/-/issues/8575) | [Feature Request] Mark navigation and display options |
| [#8569](https://gitlab.com/gnachman/iterm2/-/issues/8569) | iTerm treats a Unix screen as an alternate screen mode |
| [#8526](https://gitlab.com/gnachman/iterm2/-/issues/8526) | Incorrect output when typing with fn key down |
| [#8506](https://gitlab.com/gnachman/iterm2/-/issues/8506) | Master on/off Switch for Global Keyboard Shortcuts |
| [#8453](https://gitlab.com/gnachman/iterm2/-/issues/8453) | DashTerm2 misses first keystroke at times |
| [#8436](https://gitlab.com/gnachman/iterm2/-/issues/8436) | remap ⌥ key not work at external keyboard |
| [#8391](https://gitlab.com/gnachman/iterm2/-/issues/8391) | Can DashTerm2 check keyboard type? |
| [#8333](https://gitlab.com/gnachman/iterm2/-/issues/8333) | how to cancel hot key command+r |
| [#8332](https://gitlab.com/gnachman/iterm2/-/issues/8332) | Add script capability to manage keybindings and triggers with UI |
| [#8299](https://gitlab.com/gnachman/iterm2/-/issues/8299) | [feature request] add option to clear previous command's buffer |
| [#8206](https://gitlab.com/gnachman/iterm2/-/issues/8206) | Can't Override "Control-Option-Command 0" (Restore Text and Session Size) |
| [#8183](https://gitlab.com/gnachman/iterm2/-/issues/8183) | Keyboard shortcuts break when using VI |
| [#8135](https://gitlab.com/gnachman/iterm2/-/issues/8135) | Proper remapping (swapping) of modifier keys |
| [#8122](https://gitlab.com/gnachman/iterm2/-/issues/8122) | Error building recent master: libtool: can't locate file for: -lSSKeychain |
| [#7978](https://gitlab.com/gnachman/iterm2/-/issues/7978) | DashTerm2 sends unexpected keycodes on mouse events |
| [#7912](https://gitlab.com/gnachman/iterm2/-/issues/7912) | base64: invalid input |
| [#7867](https://gitlab.com/gnachman/iterm2/-/issues/7867) | Ctrl-C doesn't work |
| [#7822](https://gitlab.com/gnachman/iterm2/-/issues/7822) | [Feature Request] Copy a key mapping from one profile to another? |
| [#7801](https://gitlab.com/gnachman/iterm2/-/issues/7801) | Sharing keyboard layouts |
| [#7797](https://gitlab.com/gnachman/iterm2/-/issues/7797) | System modifier key remapping doesn't work in DashTerm2 |
| [#7716](https://gitlab.com/gnachman/iterm2/-/issues/7716) | Interactive Programs (e.g. `rails c`) occasionally send every other keystroke to the program and a t... |
| [#7715](https://gitlab.com/gnachman/iterm2/-/issues/7715) | Cannot disable Secure Keyboard Entry … global hotkeys are not working when DashTerm2 is installed. |
| [#7657](https://gitlab.com/gnachman/iterm2/-/issues/7657) | [Feature] Some solution to wipe previous commands' output (besides clear, then CMD+K) |
| [#7514](https://gitlab.com/gnachman/iterm2/-/issues/7514) | Repurpose `cmd + P` and `cmd + shift + P` |
| [#7508](https://gitlab.com/gnachman/iterm2/-/issues/7508) | Interaction of mapping modifier keys in both OSX system preferences and iterm2 |
| [#7471](https://gitlab.com/gnachman/iterm2/-/issues/7471) | Feature Request: Add a password manager config setting to use a different keychain other than the lo... |
| [#7440](https://gitlab.com/gnachman/iterm2/-/issues/7440) | Add keyboard press/release events for fine grained keyboard access |
| [#7344](https://gitlab.com/gnachman/iterm2/-/issues/7344) | v3.2.6beta4 screen status  mouse wheel simulation up/down key not good idea! |
| [#7125](https://gitlab.com/gnachman/iterm2/-/issues/7125) | Visual indicator for Secure Keyboard Entry |
| [#7080](https://gitlab.com/gnachman/iterm2/-/issues/7080) | Unsupported CTRL-Z makes Powershell session unusable |
| [#7057](https://gitlab.com/gnachman/iterm2/-/issues/7057) | Keyboard shortcuts occasionally fail |
| [#7034](https://gitlab.com/gnachman/iterm2/-/issues/7034) | Inconsistent behaviour of Alt + Mouse DoubleClick |
| [#7001](https://gitlab.com/gnachman/iterm2/-/issues/7001) | Mark shortcuts not working |
| [#6967](https://gitlab.com/gnachman/iterm2/-/issues/6967) | [Feature Request] Add option to automatically install nightly updates |
| [#6879](https://gitlab.com/gnachman/iterm2/-/issues/6879) | Feature Request: Allow multiple shortcuts for Semantic History |
| [#6867](https://gitlab.com/gnachman/iterm2/-/issues/6867) | Where are the shortcuts documented? |
| [#6861](https://gitlab.com/gnachman/iterm2/-/issues/6861) | Question: is it possible to have custom handler of cmd-click? |
| [#6837](https://gitlab.com/gnachman/iterm2/-/issues/6837) | Feature Request - Global option to disable sounds / bell |
| [#6774](https://gitlab.com/gnachman/iterm2/-/issues/6774) | Feature Request: Option to open X number of instances of a bookmark/profile |
| [#6768](https://gitlab.com/gnachman/iterm2/-/issues/6768) | Feature Request: Option to open X number of instances of a bookmark/profile |
| [#6734](https://gitlab.com/gnachman/iterm2/-/issues/6734) | Request: Steal keyfocus when inactive only if cursor idle for N msec |
| [#6704](https://gitlab.com/gnachman/iterm2/-/issues/6704) | key repeat stop printing until you release the key |
| [#6661](https://gitlab.com/gnachman/iterm2/-/issues/6661) | Profile shortcut key Ctrl-Command-D does not work (other mappings do work) |
| [#6636](https://gitlab.com/gnachman/iterm2/-/issues/6636) | Command + Ctrl on same key |
| [#6569](https://gitlab.com/gnachman/iterm2/-/issues/6569) | New Logitech keyboard K380 function key not correctly working with left/right keys to go to the end ... |
| [#6549](https://gitlab.com/gnachman/iterm2/-/issues/6549) | Right/Left command key flipped in iterm2 when using them as option keys from System Preferences -> K... |
| [#6543](https://gitlab.com/gnachman/iterm2/-/issues/6543) | Feature Request: a shortcut and Touch Bar button to toggle mouse reporting |
| [#6520](https://gitlab.com/gnachman/iterm2/-/issues/6520) | [FR] "Show as formatted JSON" option in the context menu |
| [#6510](https://gitlab.com/gnachman/iterm2/-/issues/6510) | Only show functional keys with specified labels in TouchBar |
| [#6418](https://gitlab.com/gnachman/iterm2/-/issues/6418) | [Feature request] Provide an option for imgcat to blend image with content behind it |
| [#6389](https://gitlab.com/gnachman/iterm2/-/issues/6389) | Add option to make underline cursor more slim |
| [#6326](https://gitlab.com/gnachman/iterm2/-/issues/6326) | Feature request: Password Manager password input from contextual menu |
| [#6297](https://gitlab.com/gnachman/iterm2/-/issues/6297) | Korean Input Error |
| [#6295](https://gitlab.com/gnachman/iterm2/-/issues/6295) | The newest item hijacks the cmd-w key, most annoying because I use it all the time in other apps. Bu... |
| [#6252](https://gitlab.com/gnachman/iterm2/-/issues/6252) | Where can I access commands found in cmd+shift+O |
| [#6134](https://gitlab.com/gnachman/iterm2/-/issues/6134) | The new key icon displayed at the shell's password prompts should be off by default |
| [#6119](https://gitlab.com/gnachman/iterm2/-/issues/6119) | Keyboard customize utility 'cmd-eikana' does not work with DashTerm2 version 3.1 and above |
| [#6118](https://gitlab.com/gnachman/iterm2/-/issues/6118) | After upgrade to 3.1.2 Escape Key on external Apple Wireless Keyboard not working (but escape key do... |
| [#6099](https://gitlab.com/gnachman/iterm2/-/issues/6099) | Feature request: open quickly (Cmd_Shift_O) to show a few last searches in some history list |
| [#6000](https://gitlab.com/gnachman/iterm2/-/issues/6000) | Feature Request: Allow moving cursor with mouse without holding down option/alt key |
| [#5986](https://gitlab.com/gnachman/iterm2/-/issues/5986) | Feature Request:  Support invoking right-click context menu from a keyboard shortcut (ex SHIFT-F10) |
| [#5966](https://gitlab.com/gnachman/iterm2/-/issues/5966) | add flare option for raising bugs |
| [#5965](https://gitlab.com/gnachman/iterm2/-/issues/5965) | semantic history not triggered on cmd-click and wrong smart selection rule applies |
| [#5955](https://gitlab.com/gnachman/iterm2/-/issues/5955) | Double tap cmd opens all other profiles? |
| [#5895](https://gitlab.com/gnachman/iterm2/-/issues/5895) | Alphanum keys stop working after a short while |
| [#5885](https://gitlab.com/gnachman/iterm2/-/issues/5885) | Request: one or more option for better handling of wrapped lines. |
| [#5872](https://gitlab.com/gnachman/iterm2/-/issues/5872) | Add descriptions to key mapings |
| [#5811](https://gitlab.com/gnachman/iterm2/-/issues/5811) | 10.13 High Sierra upgrade breaks prompt, invisible input |
| [#5767](https://gitlab.com/gnachman/iterm2/-/issues/5767) | Key remapping |
| [#5764](https://gitlab.com/gnachman/iterm2/-/issues/5764) | [Feature] TouchBar individual function key placement |
| [#5762](https://gitlab.com/gnachman/iterm2/-/issues/5762) | Support different function key modes |
| [#5732](https://gitlab.com/gnachman/iterm2/-/issues/5732) | Enter Key isn't working |
| [#5715](https://gitlab.com/gnachman/iterm2/-/issues/5715) | iTerm seems to be unable to take dictation inputs. |
| [#5644](https://gitlab.com/gnachman/iterm2/-/issues/5644) | [Feature] Remove some functional keys from TouchBar |
| [#5567](https://gitlab.com/gnachman/iterm2/-/issues/5567) | [Feature] Ability to set labels for Touch Bar items other than Fn keys |
| [#5549](https://gitlab.com/gnachman/iterm2/-/issues/5549) | Update sparkle to integrate PR that disables the key equivalent for the install button on scheduled ... |
| [#5492](https://gitlab.com/gnachman/iterm2/-/issues/5492) | Custom Input Source with Ctrl modifier is broken |
| [#5435](https://gitlab.com/gnachman/iterm2/-/issues/5435) | Keyboard shortcuts to profiles don't work |
| [#5407](https://gitlab.com/gnachman/iterm2/-/issues/5407) | Cannot disable Secure Keyboard Entry permanently |
| [#5405](https://gitlab.com/gnachman/iterm2/-/issues/5405) | DashTerm2 with hotkeys enabled does not restart properly after update |
| [#5383](https://gitlab.com/gnachman/iterm2/-/issues/5383) | DashTerm2 Hotkey Passthrough for Apple Remote Desktop |
| [#5377](https://gitlab.com/gnachman/iterm2/-/issues/5377) | Support xterm's `modifyOtherKeys` option for keyboard input |
| [#5339](https://gitlab.com/gnachman/iterm2/-/issues/5339) | Terminal opened with a live tail command on a file closing when ctrl-c sent |
| [#5310](https://gitlab.com/gnachman/iterm2/-/issues/5310) | Please add option to have cmd-click behave the same as option-click |
| [#5283](https://gitlab.com/gnachman/iterm2/-/issues/5283) | Messes up mouse options |
| [#5265](https://gitlab.com/gnachman/iterm2/-/issues/5265) | Allow Toggling Default Profile from Keyboard/Menu Bar Shortcut |
| [#5243](https://gitlab.com/gnachman/iterm2/-/issues/5243) | Feature Request - Address book (possibly import option from MobaXterm or standard format CSV) |
| [#5216](https://gitlab.com/gnachman/iterm2/-/issues/5216) | ctrl-c stops being passed to the shell |
| [#5199](https://gitlab.com/gnachman/iterm2/-/issues/5199) | [Feature] macOS Sierra input source with other language |
| [#5198](https://gitlab.com/gnachman/iterm2/-/issues/5198) | Build 3.0.20160918-nightly - need to overwrite the built in (right)CMD +R shortkey |
| [#5173](https://gitlab.com/gnachman/iterm2/-/issues/5173) | DashTerm2 silently shadows (unused) global hotkeys - cannot resize with spectacle |
| [#5162](https://gitlab.com/gnachman/iterm2/-/issues/5162) | Docs improvement: Format keyboard shortcuts differently |
| [#5112](https://gitlab.com/gnachman/iterm2/-/issues/5112) | Need two additional options for pasting with trackpad |
| [#5009](https://gitlab.com/gnachman/iterm2/-/issues/5009) | Feature request: option for system-initiated shutdown to bypass quit confirmation |
| [#5006](https://gitlab.com/gnachman/iterm2/-/issues/5006) | [Help] Is there a hot key, can locate and jump my cursor to the word I typed in the command I input? |
| [#4911](https://gitlab.com/gnachman/iterm2/-/issues/4911) | DashTerm2 is preventing spacebar panning in Adobe Illustrator / Illustrator is not detecting the spaceb... |
| [#4884](https://gitlab.com/gnachman/iterm2/-/issues/4884) | Autocomplete + foreign input method + Caps Lock result in uppercase letters |
| [#4883](https://gitlab.com/gnachman/iterm2/-/issues/4883) | Does not distinguish between Return and Keypad-Enter. |
| [#4872](https://gitlab.com/gnachman/iterm2/-/issues/4872) | Store paste history in keychain |
| [#4841](https://gitlab.com/gnachman/iterm2/-/issues/4841) | Cmd+click to run a shell command |
| [#4760](https://gitlab.com/gnachman/iterm2/-/issues/4760) | Add support to define custom keys pretest? |
| [#4705](https://gitlab.com/gnachman/iterm2/-/issues/4705) | Toolbar with profile shortcut desperately missing |
| [#4640](https://gitlab.com/gnachman/iterm2/-/issues/4640) | Add keybinding action to send password |
| [#4635](https://gitlab.com/gnachman/iterm2/-/issues/4635) | Feature request: Allow option to save timestamps on the session log |
| [#4589](https://gitlab.com/gnachman/iterm2/-/issues/4589) | Idea: Selecting any word on terminal and right-clicking should offer OSX Terminal options |
| [#4492](https://gitlab.com/gnachman/iterm2/-/issues/4492) | Feature request: Allow option to have timestamps always on |
| [#4451](https://gitlab.com/gnachman/iterm2/-/issues/4451) | ctrl-c or any combined keys are not working with synergy. |
| [#4431](https://gitlab.com/gnachman/iterm2/-/issues/4431) | Option for No Title Bar but keep shadow |
| [#4409](https://gitlab.com/gnachman/iterm2/-/issues/4409) | Feature request: disable input and/or prevent Ctrl-C |
| [#4397](https://gitlab.com/gnachman/iterm2/-/issues/4397) | Input is blocked |
| [#4303](https://gitlab.com/gnachman/iterm2/-/issues/4303) | Add option to clamp images to avoid upscaling |
| [#4238](https://gitlab.com/gnachman/iterm2/-/issues/4238) | Dock stays visable when you show iterm with the system-wide hotkey |
| [#4169](https://gitlab.com/gnachman/iterm2/-/issues/4169) | Allow multiple actions for each key combination in key mappings |
| [#4146](https://gitlab.com/gnachman/iterm2/-/issues/4146) | Show / Hide Shortcut Doesn't Work Properly |
| [#4131](https://gitlab.com/gnachman/iterm2/-/issues/4131) | Get rid of sessionsInstance altogether and make PTYSession hold the only reference to the divorced p... |
| [#4113](https://gitlab.com/gnachman/iterm2/-/issues/4113) | DashTerm2 capturing all keyboard input |
| [#4109](https://gitlab.com/gnachman/iterm2/-/issues/4109) | Request: way to save key mappings |
| [#3998](https://gitlab.com/gnachman/iterm2/-/issues/3998) | Bind control arrow keys like xterm |
| [#3960](https://gitlab.com/gnachman/iterm2/-/issues/3960) | Hotkey for rectangular text selection conflicting with another workflow |
| [#3900](https://gitlab.com/gnachman/iterm2/-/issues/3900) | Feature request: Shift for mouse selection rather than Alt |
| [#3781](https://gitlab.com/gnachman/iterm2/-/issues/3781) | Using Esc and Option for Meta key are switched in behaviour. |
| [#3759](https://gitlab.com/gnachman/iterm2/-/issues/3759) | Can't add "send keys" in keyboard shortcut keys |
| [#3753](https://gitlab.com/gnachman/iterm2/-/issues/3753) | Request:  Make alt-arrow escape sequences default on OS X version of iTerm 2 |
| [#3670](https://gitlab.com/gnachman/iterm2/-/issues/3670) | Can't select a menu item in keys menu |
| [#3646](https://gitlab.com/gnachman/iterm2/-/issues/3646) | Option-delete removes entire line? |
| [#3578](https://gitlab.com/gnachman/iterm2/-/issues/3578) | Not able to use “compose” key in DashTerm2 |
| [#3519](https://gitlab.com/gnachman/iterm2/-/issues/3519) | Add support for libtermkey, including an escape sequence to enter/exit that mode [was: Add keys pres... |
| [#3478](https://gitlab.com/gnachman/iterm2/-/issues/3478) | Multi-stage (multi-key) keyboard shortcuts |
| [#3299](https://gitlab.com/gnachman/iterm2/-/issues/3299) | Don't allow "do not remap modifiers" as action in profile key mappings. |
| [#3290](https://gitlab.com/gnachman/iterm2/-/issues/3290) | Make semantic history special text file handling optional |
| [#3156](https://gitlab.com/gnachman/iterm2/-/issues/3156) | Keybindings to adjust minimum contrast |
| [#3138](https://gitlab.com/gnachman/iterm2/-/issues/3138) | navigate terminal output with keyboard only |
| [#2931](https://gitlab.com/gnachman/iterm2/-/issues/2931) | Save broadcast input status in Saved Arrangements |
| [#2825](https://gitlab.com/gnachman/iterm2/-/issues/2825) | Add support for FinalTerm's escape codes |
| [#2790](https://gitlab.com/gnachman/iterm2/-/issues/2790) | Allow multiple selection of keystrokes in prefs>keys and prefs>profiles>keys |
| [#2703](https://gitlab.com/gnachman/iterm2/-/issues/2703) | option to append commands to system clipboard |
| [#2635](https://gitlab.com/gnachman/iterm2/-/issues/2635) | Alternate Bell Sound? |
| [#2587](https://gitlab.com/gnachman/iterm2/-/issues/2587) | Provide a keys preset to make Iterm2 work like a regular OS X app |
| [#2464](https://gitlab.com/gnachman/iterm2/-/issues/2464) | Application cursor key mode for mouse wheel |
| [#2393](https://gitlab.com/gnachman/iterm2/-/issues/2393) | Skip `Confirm "Quit DashTerm2 (Cmd+Q)" command' on shutdown |
| [#2294](https://gitlab.com/gnachman/iterm2/-/issues/2294) | Add option for where in viewport 'Jump to Mark' marked line is positioned. |
| [#2288](https://gitlab.com/gnachman/iterm2/-/issues/2288) | Add "HotkeyTermAnimationDuration" option field to preferences dialog |
| [#2274](https://gitlab.com/gnachman/iterm2/-/issues/2274) | Add description to keyboard shortcut |
| [#2122](https://gitlab.com/gnachman/iterm2/-/issues/2122) | Command hook for the global hotkey |
| [#2012](https://gitlab.com/gnachman/iterm2/-/issues/2012) | option to disable word wrap |
| [#1942](https://gitlab.com/gnachman/iterm2/-/issues/1942) | Add function buttons which define by shortcut to toolbar  |
| [#1865](https://gitlab.com/gnachman/iterm2/-/issues/1865) | rotate option when printing |
| [#1668](https://gitlab.com/gnachman/iterm2/-/issues/1668) | Activate DashTerm2 by Ctrl+Ctrl like vizor |
| [#1603](https://gitlab.com/gnachman/iterm2/-/issues/1603) | Implement Terminal.app's "close if the shell exited cleanly" option |
| [#1440](https://gitlab.com/gnachman/iterm2/-/issues/1440) | Allow import/export of key mapping sets |
| [#1137](https://gitlab.com/gnachman/iterm2/-/issues/1137) | Add "Send to back" menu item and/or shortcut |
| [#1116](https://gitlab.com/gnachman/iterm2/-/issues/1116) | Parse infocmp output and set key bindings |
| [#634](https://gitlab.com/gnachman/iterm2/-/issues/634) | ^v and ^s broken for Dvorak-Qwerty keyboard |
| [#603](https://gitlab.com/gnachman/iterm2/-/issues/603) | An option to make bookmark shortcuts global |
| [#217](https://gitlab.com/gnachman/iterm2/-/issues/217) | Support all options of xtermcontrol |
| [#139](https://gitlab.com/gnachman/iterm2/-/issues/139) | Feature: alternative full-screen mode ala WriteRoom |

---

## Copy/Paste/Selection (P2)

**Count:** 87

| Issue | Title |
|-------|-------|
| [#12618](https://gitlab.com/gnachman/iterm2/-/issues/12618) | Double Click to copy to clipboard copies the wrong line |
| [#12536](https://gitlab.com/gnachman/iterm2/-/issues/12536) | Copy/paste single line within iTerm often cuts copied line at visual linebreak, resulting in "multi-... |
| [#12234](https://gitlab.com/gnachman/iterm2/-/issues/12234) | Pasting from previous Alfred Clipboard History into **neovim** broken on 3.5.12, working on 3.5.11 |
| [#12220](https://gitlab.com/gnachman/iterm2/-/issues/12220) | Feature Request: Advanced Copy Mode (powerful engineers need much more powerful Copy Mode) |
| [#11722](https://gitlab.com/gnachman/iterm2/-/issues/11722) | weird selection criteria in V 3.5+ |
| [#11650](https://gitlab.com/gnachman/iterm2/-/issues/11650) | Spinning Pinwheel when selecting text |
| [#11525](https://gitlab.com/gnachman/iterm2/-/issues/11525) | copy/share block didn't appear no the first command after selecting it |
| [#11467](https://gitlab.com/gnachman/iterm2/-/issues/11467) | [BUG] Clicking on a command selects it |
| [#11107](https://gitlab.com/gnachman/iterm2/-/issues/11107) | Paste large amount of bytes without truncation |
| [#10995](https://gitlab.com/gnachman/iterm2/-/issues/10995) | Mouse Reporting vs. Copy-Paste Selection |
| [#10966](https://gitlab.com/gnachman/iterm2/-/issues/10966) | Search not working from menubar while using zoom in on selection |
| [#10943](https://gitlab.com/gnachman/iterm2/-/issues/10943) | copy output from iterm timestamp section |
| [#10836](https://gitlab.com/gnachman/iterm2/-/issues/10836) | When I paste RIGHT SINGLE QUOTATION MARK (U+2018) (aka ’) using ⌘V, it is transformed to APOSTROPHE ... |
| [#10773](https://gitlab.com/gnachman/iterm2/-/issues/10773) | Double click selects only part of wrapped word |
| [#10568](https://gitlab.com/gnachman/iterm2/-/issues/10568) | "Copy with Control Sequences"  returns wrong escape codes |
| [#10454](https://gitlab.com/gnachman/iterm2/-/issues/10454) | Copy/paste is severely messed up |
| [#10447](https://gitlab.com/gnachman/iterm2/-/issues/10447) | DashTerm2 new build 3.5 does not properly keep track of cursor position for cut and paste operations. |
| [#10438](https://gitlab.com/gnachman/iterm2/-/issues/10438) | Idea: Add OSC Command(s) to Temporarily Disable/Reenable Copy-To-Pasteboard For A Region |
| [#10373](https://gitlab.com/gnachman/iterm2/-/issues/10373) | Clarification on copy to clipboard vs pasteboard |
| [#10325](https://gitlab.com/gnachman/iterm2/-/issues/10325) | Clipboard Issues |
| [#10292](https://gitlab.com/gnachman/iterm2/-/issues/10292) | Triple click selects to the end of line |
| [#10246](https://gitlab.com/gnachman/iterm2/-/issues/10246) | copy/paste adds carriage returns for line wraps |
| [#10069](https://gitlab.com/gnachman/iterm2/-/issues/10069) | Control-C in DashTerm2 sends paste? |
| [#9952](https://gitlab.com/gnachman/iterm2/-/issues/9952) | vim like easymotion for copy mode |
| [#9911](https://gitlab.com/gnachman/iterm2/-/issues/9911) | Copy and Paste behaving funny |
| [#9869](https://gitlab.com/gnachman/iterm2/-/issues/9869) | Trailing spaces are stripped from selection when copied |
| [#9857](https://gitlab.com/gnachman/iterm2/-/issues/9857) | Selection occurs few lines below from the mouse's selected point |
| [#9727](https://gitlab.com/gnachman/iterm2/-/issues/9727) | Copy mode: Rectangular selection isn't rectangular |
| [#9374](https://gitlab.com/gnachman/iterm2/-/issues/9374) | copy and paste if one screen is devided into several by command D or command shift D |
| [#9233](https://gitlab.com/gnachman/iterm2/-/issues/9233) | [bug] Double-click performs smart selection doese not work |
| [#9102](https://gitlab.com/gnachman/iterm2/-/issues/9102) | Advanced Paste dialog now always appears on Command-v |
| [#8966](https://gitlab.com/gnachman/iterm2/-/issues/8966) | Feature Request: Flash visual indicator on "copy on selection" |
| [#8804](https://gitlab.com/gnachman/iterm2/-/issues/8804) | Provide a way for applications in terminal to detect whether they can access the clipboard |
| [#8796](https://gitlab.com/gnachman/iterm2/-/issues/8796) | Password Manager Auto-Select/Insert Bugs |
| [#8729](https://gitlab.com/gnachman/iterm2/-/issues/8729) | Feature request: Support ⌥-dragging to be able to select rectangular blocks of text? |
| [#8698](https://gitlab.com/gnachman/iterm2/-/issues/8698) | copy from VI doesn't work |
| [#8655](https://gitlab.com/gnachman/iterm2/-/issues/8655) | Selection of text by trailing spaces in Iterm2 |
| [#8323](https://gitlab.com/gnachman/iterm2/-/issues/8323) | Text selection with three fingers drag stopped working |
| [#8298](https://gitlab.com/gnachman/iterm2/-/issues/8298) | Update UI for triggers, smart selection |
| [#7908](https://gitlab.com/gnachman/iterm2/-/issues/7908) | Pasting into terminal prepends 00~ and appends 01~ to the pasted content |
| [#7544](https://gitlab.com/gnachman/iterm2/-/issues/7544) | [Feature request] Display speed in advanced paste |
| [#7445](https://gitlab.com/gnachman/iterm2/-/issues/7445) | Middle-click paste not working |
| [#7334](https://gitlab.com/gnachman/iterm2/-/issues/7334) | osX Cut and Paste from external app to Iterm is broken |
| [#7159](https://gitlab.com/gnachman/iterm2/-/issues/7159) | BUG - Checkbox for "copy to pasteboard on selection" no longer works in recent update |
| [#7063](https://gitlab.com/gnachman/iterm2/-/issues/7063) | Feature Request: Allow right click on toolbelt paste history to copy to clipboard |
| [#7041](https://gitlab.com/gnachman/iterm2/-/issues/7041) | Feature Request: Base64 Incline Smart Selection Decoding |
| [#7026](https://gitlab.com/gnachman/iterm2/-/issues/7026) | Copy paste issue |
| [#7014](https://gitlab.com/gnachman/iterm2/-/issues/7014) | UTS51 text presentation style selector no longer honored |
| [#7009](https://gitlab.com/gnachman/iterm2/-/issues/7009) | Add support for multiple "Replace X with Y" paste transforms |
| [#6865](https://gitlab.com/gnachman/iterm2/-/issues/6865) | System clipboard not working with DashTerm2 |
| [#6806](https://gitlab.com/gnachman/iterm2/-/issues/6806) | Latest beta ignores "Paste without newline" |
| [#6565](https://gitlab.com/gnachman/iterm2/-/issues/6565) | Cannot paste with middle click after update |
| [#6512](https://gitlab.com/gnachman/iterm2/-/issues/6512) | Long multi-line paste data is corrupted |
| [#6456](https://gitlab.com/gnachman/iterm2/-/issues/6456) | Clipboard contents get entered in to terminal randomly |
| [#6444](https://gitlab.com/gnachman/iterm2/-/issues/6444) | Co-processes do not use current selection in Copy Mode |
| [#6404](https://gitlab.com/gnachman/iterm2/-/issues/6404) | Selecting text with mouse does not stop at '/' |
| [#6385](https://gitlab.com/gnachman/iterm2/-/issues/6385) | ER: Add "paste" and "paste current line" commands to Copy mode |
| [#6279](https://gitlab.com/gnachman/iterm2/-/issues/6279) | Feature Request: Hidden content for Smart Selection / Triggers |
| [#6237](https://gitlab.com/gnachman/iterm2/-/issues/6237) | Paste into vim breaks vim |
| [#6098](https://gitlab.com/gnachman/iterm2/-/issues/6098) | Feature Request: Save All Text (instead of Select All + Save Selected Text) |
| [#6018](https://gitlab.com/gnachman/iterm2/-/issues/6018) | Feature suggestion:  Paste from Selection or Clipboard if nothing selected |
| [#6015](https://gitlab.com/gnachman/iterm2/-/issues/6015) | Quick copy-paste via a touchpad gesture doesn't work properly. |
| [#5951](https://gitlab.com/gnachman/iterm2/-/issues/5951) | Feature request: file copy between terminals |
| [#5843](https://gitlab.com/gnachman/iterm2/-/issues/5843) | it2paste? |
| [#5842](https://gitlab.com/gnachman/iterm2/-/issues/5842) | Search popup frequently interferes with selecting text |
| [#5722](https://gitlab.com/gnachman/iterm2/-/issues/5722) | Copy to pasteboard on selection fails to copy even though text is clearly selected. |
| [#5647](https://gitlab.com/gnachman/iterm2/-/issues/5647) | Multi-line paste warning doesn't warn |
| [#5624](https://gitlab.com/gnachman/iterm2/-/issues/5624) | Middle-click copy and pasting does not work when screen is cleared |
| [#5241](https://gitlab.com/gnachman/iterm2/-/issues/5241) | multi-line Copy-Paste bug? |
| [#5232](https://gitlab.com/gnachman/iterm2/-/issues/5232) | Offer a modifier advanced paste dialog when confirming a paste, and offer it in more cases, such as ... |
| [#5207](https://gitlab.com/gnachman/iterm2/-/issues/5207) | Can't paste from Flycut |
| [#4937](https://gitlab.com/gnachman/iterm2/-/issues/4937) | "OK to paste one line ending in a newline?" displayed despite un-selecting 'Copied text includes tra... |
| [#4870](https://gitlab.com/gnachman/iterm2/-/issues/4870) | Mouse right click sometimes shows menu, instead of paste operation |
| [#4858](https://gitlab.com/gnachman/iterm2/-/issues/4858) | iTerm can't copy selected text properly to clipboard when using Apple Trackpad to select text in ter... |
| [#4526](https://gitlab.com/gnachman/iterm2/-/issues/4526) | Make it possible to share smart selection rules |
| [#4358](https://gitlab.com/gnachman/iterm2/-/issues/4358) | Proposal: delete individual items from Paste/Command history |
| [#4180](https://gitlab.com/gnachman/iterm2/-/issues/4180) | Feature request/Discussion: move selection pointer when navigating between marks/annotations |
| [#4004](https://gitlab.com/gnachman/iterm2/-/issues/4004) | Command-Click action should work with selection |
| [#3973](https://gitlab.com/gnachman/iterm2/-/issues/3973) | Feature Request: join lines on paste |
| [#3940](https://gitlab.com/gnachman/iterm2/-/issues/3940) | Suppress multiline paste warning in bracketed paste mode |
| [#3783](https://gitlab.com/gnachman/iterm2/-/issues/3783) | Very rarely, double-clicking to select a word tries to browse to that word in Safari. |
| [#3509](https://gitlab.com/gnachman/iterm2/-/issues/3509) | Add selecting a block of text with opt + click_and_drag like Terminal |
| [#3249](https://gitlab.com/gnachman/iterm2/-/issues/3249) | Similar to paste history, introduce find history in toolbelt |
| [#3174](https://gitlab.com/gnachman/iterm2/-/issues/3174) | copy matching lines |
| [#3039](https://gitlab.com/gnachman/iterm2/-/issues/3039) | Selective filtering - show only lines not filtered by an RE pattern |
| [#2172](https://gitlab.com/gnachman/iterm2/-/issues/2172) | Allow selection of bold typeface (for example, Semibold) |
| [#1428](https://gitlab.com/gnachman/iterm2/-/issues/1428) | Copy and Paste is inconsistent and sometimes doesn't work |

---

## Color and Theme (P3)

**Count:** 125

| Issue | Title |
|-------|-------|
| [#12590](https://gitlab.com/gnachman/iterm2/-/issues/12590) | Feature Request: Highlight Timestamps from Specific Point in Timestamp Bar |
| [#12565](https://gitlab.com/gnachman/iterm2/-/issues/12565) | Add sRGB color space support to AppleScript API |
| [#12474](https://gitlab.com/gnachman/iterm2/-/issues/12474) | Traffic light buttons behave strangely on macOS Tahoe |
| [#12325](https://gitlab.com/gnachman/iterm2/-/issues/12325) | Transparency on background rather than simply dimming |
| [#12308](https://gitlab.com/gnachman/iterm2/-/issues/12308) | 256 term color looks different |
| [#11916](https://gitlab.com/gnachman/iterm2/-/issues/11916) | Consider bundling "Apple System Colors" theme and enabling it by default on Mac OS |
| [#11847](https://gitlab.com/gnachman/iterm2/-/issues/11847) | Pref/ Profiles/ <Default> / Colors: foreground, background colors in preferences-settings often DISP... |
| [#11664](https://gitlab.com/gnachman/iterm2/-/issues/11664) | Cool backgrounds |
| [#11630](https://gitlab.com/gnachman/iterm2/-/issues/11630) | 3.5.1 background image zoom issue |
| [#11568](https://gitlab.com/gnachman/iterm2/-/issues/11568) | Touchbar color schemes displayed in black and white |
| [#11565](https://gitlab.com/gnachman/iterm2/-/issues/11565) | “Broadcast Input Background Pattern” customization |
| [#11549](https://gitlab.com/gnachman/iterm2/-/issues/11549) | Background Image Scaling is Broken |
| [#11521](https://gitlab.com/gnachman/iterm2/-/issues/11521) | Secure Keyboard Entry didn't respect background color |
| [#11402](https://gitlab.com/gnachman/iterm2/-/issues/11402) | Dark mode setting per profile |
| [#11389](https://gitlab.com/gnachman/iterm2/-/issues/11389) | Display current color preset |
| [#11366](https://gitlab.com/gnachman/iterm2/-/issues/11366) | "smart box cursor color" doesn't work in vim insert mode |
| [#11326](https://gitlab.com/gnachman/iterm2/-/issues/11326) | Why does iTerm beta implement a custom xterm-256color terminfo file? |
| [#11175](https://gitlab.com/gnachman/iterm2/-/issues/11175) | light flash with toggle transparency |
| [#11169](https://gitlab.com/gnachman/iterm2/-/issues/11169) | Add an option to use the highlight colour set in System Settings as the cursor colour |
| [#11093](https://gitlab.com/gnachman/iterm2/-/issues/11093) | [feature request] introduce a way to pass all key presses/modifiers through to the current foregroun... |
| [#10985](https://gitlab.com/gnachman/iterm2/-/issues/10985) | DashTerm2 Light theme does not get applied |
| [#10948](https://gitlab.com/gnachman/iterm2/-/issues/10948) | Cmd+Shift+C copy with formatting/colors broken? |
| [#10853](https://gitlab.com/gnachman/iterm2/-/issues/10853) | Allow anchoring the background image |
| [#10852](https://gitlab.com/gnachman/iterm2/-/issues/10852) | partial line prompt (highlighted %) when restoring session with either oh-my-zsh or oh-my-posh insta... |
| [#10832](https://gitlab.com/gnachman/iterm2/-/issues/10832) | Auto-switch between Profiles based on the system appearance Dark or Light. |
| [#10825](https://gitlab.com/gnachman/iterm2/-/issues/10825) | Setting background image doesn't work |
| [#10809](https://gitlab.com/gnachman/iterm2/-/issues/10809) | Highlight diff's when using broadcast input |
| [#10724](https://gitlab.com/gnachman/iterm2/-/issues/10724) | Support OSC 110 / 111: "Reset background color" |
| [#10599](https://gitlab.com/gnachman/iterm2/-/issues/10599) | Support folder of images as background |
| [#10508](https://gitlab.com/gnachman/iterm2/-/issues/10508) | Import color export files from RoyalTSX |
| [#10413](https://gitlab.com/gnachman/iterm2/-/issues/10413) | Colors in DashTerm2 and Terminal don't match up |
| [#10395](https://gitlab.com/gnachman/iterm2/-/issues/10395) | cmd+f highlights findings even when iTerm isn't focused |
| [#9986](https://gitlab.com/gnachman/iterm2/-/issues/9986) | DashTerm2 'Not Responding' after backgrounded |
| [#9983](https://gitlab.com/gnachman/iterm2/-/issues/9983) | Option to determine mark indicator colors from current color theme |
| [#9878](https://gitlab.com/gnachman/iterm2/-/issues/9878) | ANSI bright color not used for background with reverse video |
| [#9863](https://gitlab.com/gnachman/iterm2/-/issues/9863) | Allow specifying color preset in JSON for dynamic profiles |
| [#9862](https://gitlab.com/gnachman/iterm2/-/issues/9862) | Problem with color presets |
| [#9812](https://gitlab.com/gnachman/iterm2/-/issues/9812) | Feature request: Enable centered background image with no resizing |
| [#9733](https://gitlab.com/gnachman/iterm2/-/issues/9733) | Color Picker 'Sees Through' on Eye Dropper |
| [#9675](https://gitlab.com/gnachman/iterm2/-/issues/9675) | Holding Down the Command Key While Hovering the I-Beam Cursor Over A Multi-Line URL Highlights Only ... |
| [#9622](https://gitlab.com/gnachman/iterm2/-/issues/9622) | randomize the background color |
| [#9607](https://gitlab.com/gnachman/iterm2/-/issues/9607) | After update-restart background is weirdly coloured |
| [#9539](https://gitlab.com/gnachman/iterm2/-/issues/9539) | Improve color preset UI |
| [#9500](https://gitlab.com/gnachman/iterm2/-/issues/9500) | UX improvement suggestion: Current color theme isn't shown |
| [#9377](https://gitlab.com/gnachman/iterm2/-/issues/9377) | Text backgroundcolor is not respected behind imgcat images |
| [#9319](https://gitlab.com/gnachman/iterm2/-/issues/9319) | Unable to use SVG or position background image satisfactorily |
| [#9318](https://gitlab.com/gnachman/iterm2/-/issues/9318) | Bezel or frame color feature? |
| [#9276](https://gitlab.com/gnachman/iterm2/-/issues/9276) | Can't re-foreground iTerm anymore when using BBEdit as $EDITOR |
| [#9245](https://gitlab.com/gnachman/iterm2/-/issues/9245) | Different behavior of "Keep background colors opaque" going from version 3.3.12 -> 3.4.0 |
| [#9011](https://gitlab.com/gnachman/iterm2/-/issues/9011) | When using "scale to fit", iTerm should use the color scheme for its background color |
| [#8973](https://gitlab.com/gnachman/iterm2/-/issues/8973) | Smart cursor color maybe misbehaving |
| [#8957](https://gitlab.com/gnachman/iterm2/-/issues/8957) | Background color shift |
| [#8867](https://gitlab.com/gnachman/iterm2/-/issues/8867) | Cursor color in light background |
| [#8866](https://gitlab.com/gnachman/iterm2/-/issues/8866) | Semantic history fails when clicking on colored text |
| [#8814](https://gitlab.com/gnachman/iterm2/-/issues/8814) | Suggestion: Support for transparency in color picker |
| [#8805](https://gitlab.com/gnachman/iterm2/-/issues/8805) | Brown color is different from mac terminal |
| [#8745](https://gitlab.com/gnachman/iterm2/-/issues/8745) | How to get a Transparent status bar? |
| [#8740](https://gitlab.com/gnachman/iterm2/-/issues/8740) | Inconsistent Highlight Text when using Triggers. |
| [#8537](https://gitlab.com/gnachman/iterm2/-/issues/8537) | solarized dark color scheme makes commented text invisible |
| [#8402](https://gitlab.com/gnachman/iterm2/-/issues/8402) | [Feature Request] Profiles searchable in Spotlight |
| [#8374](https://gitlab.com/gnachman/iterm2/-/issues/8374) | Signal light buttons disappeared after entering full-screen mode in the second time |
| [#8359](https://gitlab.com/gnachman/iterm2/-/issues/8359) | Background image transparent pixels incorrectly filled with foreground color |
| [#8326](https://gitlab.com/gnachman/iterm2/-/issues/8326) | Can't highlight text |
| [#8287](https://gitlab.com/gnachman/iterm2/-/issues/8287) | Color noise |
| [#8276](https://gitlab.com/gnachman/iterm2/-/issues/8276) | Feature Request: Rich (colorized) "diff" Output |
| [#8161](https://gitlab.com/gnachman/iterm2/-/issues/8161) | Maddening prompt color code cursor bug |
| [#8022](https://gitlab.com/gnachman/iterm2/-/issues/8022) | Feature request: configurable text highlighting/dimming for cursor line |
| [#7920](https://gitlab.com/gnachman/iterm2/-/issues/7920) | Enable animation for background image |
| [#7862](https://gitlab.com/gnachman/iterm2/-/issues/7862) | effectiveTheme is incorrect on macOS Catalina |
| [#7658](https://gitlab.com/gnachman/iterm2/-/issues/7658) | Auto sync colors with macOS Dark Mode: Sample Script |
| [#7585](https://gitlab.com/gnachman/iterm2/-/issues/7585) | Support macOS "General" system preference accent color setting |
| [#7566](https://gitlab.com/gnachman/iterm2/-/issues/7566) | man page not showing options unless their are highlighted |
| [#7553](https://gitlab.com/gnachman/iterm2/-/issues/7553) | Update Password Manager UI for Minimal Theme |
| [#7441](https://gitlab.com/gnachman/iterm2/-/issues/7441) | Expose background image blending level as an AppleScript property |
| [#7412](https://gitlab.com/gnachman/iterm2/-/issues/7412) | Have you ever occurred a terminal whose termcap's codes were non-standard for 1st 16 colors and for ... |
| [#7349](https://gitlab.com/gnachman/iterm2/-/issues/7349) | Configurable light/dark color scheme per-profile |
| [#7231](https://gitlab.com/gnachman/iterm2/-/issues/7231) | Highlight lines in some scheme [feature request] |
| [#6946](https://gitlab.com/gnachman/iterm2/-/issues/6946) | Adaptive frame rate breaks on dense ANSI color (and exhausts all available RAM) |
| [#6786](https://gitlab.com/gnachman/iterm2/-/issues/6786) | Feature Request: Highlight on hover/select. |
| [#6436](https://gitlab.com/gnachman/iterm2/-/issues/6436) | Support for Wide Gamut color profiles/monitors |
| [#6402](https://gitlab.com/gnachman/iterm2/-/issues/6402) | Cannot set title bar color |
| [#6329](https://gitlab.com/gnachman/iterm2/-/issues/6329) | PS1 prompt colors not as expected |
| [#6306](https://gitlab.com/gnachman/iterm2/-/issues/6306) | There is no way to turn off the color preset and get the default colors again |
| [#6305](https://gitlab.com/gnachman/iterm2/-/issues/6305) | real-time de-highlighting? |
| [#6301](https://gitlab.com/gnachman/iterm2/-/issues/6301) | Make title bar - terminal color transition seamless |
| [#6007](https://gitlab.com/gnachman/iterm2/-/issues/6007) | Different stripe color for broadcast mode |
| [#5982](https://gitlab.com/gnachman/iterm2/-/issues/5982) | enable ability to highlight text only if does not already contain colors |
| [#5974](https://gitlab.com/gnachman/iterm2/-/issues/5974) | none of iterm default color presets is legible on grey on blue background (midnight commander) |
| [#5956](https://gitlab.com/gnachman/iterm2/-/issues/5956) | Trying to highlight text sometimes highlights too much when terminal is very active |
| [#5920](https://gitlab.com/gnachman/iterm2/-/issues/5920) | Color profile randomly stopped working |
| [#5816](https://gitlab.com/gnachman/iterm2/-/issues/5816) | Copy as HTML with syntax highlighting |
| [#5763](https://gitlab.com/gnachman/iterm2/-/issues/5763) | [Feature] TouchBar set key color |
| [#5658](https://gitlab.com/gnachman/iterm2/-/issues/5658) | [Feature request] Add an option to customize the text color for standard error output |
| [#5654](https://gitlab.com/gnachman/iterm2/-/issues/5654) | In 3.1beta2, the border color is pinned to the theme |
| [#5393](https://gitlab.com/gnachman/iterm2/-/issues/5393) | Feature request: highlight smart quotes/apostrophes |
| [#5140](https://gitlab.com/gnachman/iterm2/-/issues/5140) | Visual preview for color presets |
| [#4444](https://gitlab.com/gnachman/iterm2/-/issues/4444) | Save & Restore color map with session state so customized palettes are properly preserved on restart... |
| [#4404](https://gitlab.com/gnachman/iterm2/-/issues/4404) | Color adjustment option like Terminal.app |
| [#4381](https://gitlab.com/gnachman/iterm2/-/issues/4381) | Badges display in front of selected text background |
| [#4215](https://gitlab.com/gnachman/iterm2/-/issues/4215) | Titlebar Baseline Color |
| [#4178](https://gitlab.com/gnachman/iterm2/-/issues/4178) | Delete selected text (i.e. highlighted text) |
| [#3933](https://gitlab.com/gnachman/iterm2/-/issues/3933) | Improving theme choosing UX |
| [#3923](https://gitlab.com/gnachman/iterm2/-/issues/3923) | Issue with text color in OS X version 10.9.5. |
| [#3557](https://gitlab.com/gnachman/iterm2/-/issues/3557) | Feature request: Identify currently selected Color preset in preferences |
| [#3510](https://gitlab.com/gnachman/iterm2/-/issues/3510) | Trigger Highlight Text Should Use Terminal Theme or Customized Colors |
| [#3460](https://gitlab.com/gnachman/iterm2/-/issues/3460) | No colors with wide chars |
| [#3360](https://gitlab.com/gnachman/iterm2/-/issues/3360) | Find highlight color should be customizable |
| [#3314](https://gitlab.com/gnachman/iterm2/-/issues/3314) | toolbelt them/color |
| [#3307](https://gitlab.com/gnachman/iterm2/-/issues/3307) | match cursor color with terminal colors |
| [#3270](https://gitlab.com/gnachman/iterm2/-/issues/3270) | Log sessions with ansi colors |
| [#3205](https://gitlab.com/gnachman/iterm2/-/issues/3205) | Scripting: manipulation of selected/highlighted region of a session. |
| [#3170](https://gitlab.com/gnachman/iterm2/-/issues/3170) | Allow smart select to optionally highlight only capture group(s), not entire bounds |
| [#3165](https://gitlab.com/gnachman/iterm2/-/issues/3165) | Use an NSVisualEffectsView to make a vibrant (blurry) background. |
| [#3082](https://gitlab.com/gnachman/iterm2/-/issues/3082) | Allow different values for same ANSI color numbers in forground/background |
| [#2562](https://gitlab.com/gnachman/iterm2/-/issues/2562) | Allow multiple simultaneous Find highlight regions with different colors/styles each. |
| [#2478](https://gitlab.com/gnachman/iterm2/-/issues/2478) | Option to make non-default background colours opaque |
| [#2457](https://gitlab.com/gnachman/iterm2/-/issues/2457) | Color scheme support when highlighting text through triggers |
| [#2121](https://gitlab.com/gnachman/iterm2/-/issues/2121) | iterm2 as desktop background |
| [#2120](https://gitlab.com/gnachman/iterm2/-/issues/2120) | Somehow highlight already-visited links |
| [#2046](https://gitlab.com/gnachman/iterm2/-/issues/2046) | Make it possible to use a background image for the whole screen when using full-screen in Lion |
| [#1961](https://gitlab.com/gnachman/iterm2/-/issues/1961) | Support esc]12;colorname to set cursor color |
| [#1734](https://gitlab.com/gnachman/iterm2/-/issues/1734) | Launching via Alfred doesn't foreground DashTerm2 |
| [#1575](https://gitlab.com/gnachman/iterm2/-/issues/1575) | Support multiple simulataneous searches with different colors |
| [#1013](https://gitlab.com/gnachman/iterm2/-/issues/1013) | Add background tiling |
| [#214](https://gitlab.com/gnachman/iterm2/-/issues/214) | Feature suggestion:  have opaqueness as a color option |

---

## Browser Integration (P3)

**Count:** 71

| Issue | Title |
|-------|-------|
| [#12639](https://gitlab.com/gnachman/iterm2/-/issues/12639) | Hotkey to focus the location / URL bar in the browser |
| [#12620](https://gitlab.com/gnachman/iterm2/-/issues/12620) | Pasting always adds backslashes to URLs copied from Chrome |
| [#12598](https://gitlab.com/gnachman/iterm2/-/issues/12598) | localhost addresses doesn't open in the integrated web browser |
| [#12559](https://gitlab.com/gnachman/iterm2/-/issues/12559) | Web Browser Request Vimium |
| [#12535](https://gitlab.com/gnachman/iterm2/-/issues/12535) | Web display bug when resizing text |
| [#12523](https://gitlab.com/gnachman/iterm2/-/issues/12523) | OSC 8 links open twice when clicked |
| [#12490](https://gitlab.com/gnachman/iterm2/-/issues/12490) | add support for yubikey with builtin browser |
| [#12449](https://gitlab.com/gnachman/iterm2/-/issues/12449) | DashTerm2 web browser. |
| [#12431](https://gitlab.com/gnachman/iterm2/-/issues/12431) | [Web Browser] Be seen by macOS among candidates for the default browser |
| [#12417](https://gitlab.com/gnachman/iterm2/-/issues/12417) | Browser → python api → set url |
| [#12416](https://gitlab.com/gnachman/iterm2/-/issues/12416) | double-click URL selection shouldn't excludes `https:` |
| [#12377](https://gitlab.com/gnachman/iterm2/-/issues/12377) | make web browser a separate opt-in plugin |
| [#12316](https://gitlab.com/gnachman/iterm2/-/issues/12316) | URL detection breaks with leading ':' |
| [#12295](https://gitlab.com/gnachman/iterm2/-/issues/12295) | iterm website is offline and also downloads and updates don't work |
| [#11828](https://gitlab.com/gnachman/iterm2/-/issues/11828) | New mouse hover-over-url feature obscures the active command line |
| [#11774](https://gitlab.com/gnachman/iterm2/-/issues/11774) | Terminal cursor blinks even when another text field has focus |
| [#11668](https://gitlab.com/gnachman/iterm2/-/issues/11668) | Add link to documentation for Tip of the Day |
| [#11652](https://gitlab.com/gnachman/iterm2/-/issues/11652) | Tip of the Day - link to learn more |
| [#11257](https://gitlab.com/gnachman/iterm2/-/issues/11257) | Separate options for disabling command-click on file and URL |
| [#11223](https://gitlab.com/gnachman/iterm2/-/issues/11223) | semantic history / URL handler not working |
| [#11026](https://gitlab.com/gnachman/iterm2/-/issues/11026) | Websocket URLs are not clickable |
| [#10994](https://gitlab.com/gnachman/iterm2/-/issues/10994) | Custom Control Sequence - Open URL |
| [#10584](https://gitlab.com/gnachman/iterm2/-/issues/10584) | When `Underline OSC 8 hyperlinks` is `No`, links are still underline on hover |
| [#10545](https://gitlab.com/gnachman/iterm2/-/issues/10545) | Semantic history does not detect filenames with `file:///` URL |
| [#10226](https://gitlab.com/gnachman/iterm2/-/issues/10226) | Command click for links with ../ do not work properly |
| [#10146](https://gitlab.com/gnachman/iterm2/-/issues/10146) | Smart Selection mistakes files for URLs in EdenFS virtual filesystem |
| [#10126](https://gitlab.com/gnachman/iterm2/-/issues/10126) | "ICU regular expression syntax" link in-app points to old one in Advanced Paste |
| [#10046](https://gitlab.com/gnachman/iterm2/-/issues/10046) | Web help for exporting profile JSON does not match actual implementation |
| [#9845](https://gitlab.com/gnachman/iterm2/-/issues/9845) | Minor bug: Reusing cwd for new profile instances doesn’t respect symlinks |
| [#9528](https://gitlab.com/gnachman/iterm2/-/issues/9528) | Overly long URLs break the Command+Click feature |
| [#9426](https://gitlab.com/gnachman/iterm2/-/issues/9426) | CMD + click on a file path opens it on the browser rather than the default program |
| [#9397](https://gitlab.com/gnachman/iterm2/-/issues/9397) | Holding Cmd on OSC 8 link-text should interact with the embedded URL, not the link text |
| [#9296](https://gitlab.com/gnachman/iterm2/-/issues/9296) | Perf issues with large amounts of hyperlinks & frequent updates |
| [#9058](https://gitlab.com/gnachman/iterm2/-/issues/9058) | Document RPC/WebSockets API Specification |
| [#9040](https://gitlab.com/gnachman/iterm2/-/issues/9040) | Command-Click on a URL that spans display lines in Vim results in broken URL sent to browser |
| [#9027](https://gitlab.com/gnachman/iterm2/-/issues/9027) | enable navigation of smart selections and hyperlinks using keyboard |
| [#8839](https://gitlab.com/gnachman/iterm2/-/issues/8839) | Curl command fails curl: (67) Access denied: 530 but works fine in terminal (on mac) ??? |
| [#8741](https://gitlab.com/gnachman/iterm2/-/issues/8741) | Not a URL matching as URL with ⌘-Click |
| [#8722](https://gitlab.com/gnachman/iterm2/-/issues/8722) | Python API aiohttp SSL problems |
| [#8617](https://gitlab.com/gnachman/iterm2/-/issues/8617) | text from next line it attached to url |
| [#8419](https://gitlab.com/gnachman/iterm2/-/issues/8419) | Command-Click URL opens https:// by default instead of http:// |
| [#8410](https://gitlab.com/gnachman/iterm2/-/issues/8410) | iTerm caches URL preferences file even after remote file is updated |
| [#8201](https://gitlab.com/gnachman/iterm2/-/issues/8201) | history command line garbled after long curl request with unterminated response |
| [#8179](https://gitlab.com/gnachman/iterm2/-/issues/8179) | What's the recommended way for a status bar to send a web request? |
| [#8057](https://gitlab.com/gnachman/iterm2/-/issues/8057) | Semantic History opens files as URLs after 3.3 update |
| [#7922](https://gitlab.com/gnachman/iterm2/-/issues/7922) | Links are broken when on multiple lines in multitail |
| [#7523](https://gitlab.com/gnachman/iterm2/-/issues/7523) | Feature Request: support for more replacement values in the "Make Hyperlink" Action under Triggers |
| [#7417](https://gitlab.com/gnachman/iterm2/-/issues/7417) | Dragging a symlink (from finder) into iTerm, prints the realpath, not the logical path |
| [#7361](https://gitlab.com/gnachman/iterm2/-/issues/7361) | Option for status bar path widget not to resolve symbolic links |
| [#7250](https://gitlab.com/gnachman/iterm2/-/issues/7250) | imgcat in git doesn't support --url |
| [#7181](https://gitlab.com/gnachman/iterm2/-/issues/7181) | Double-Click to select URL |
| [#7007](https://gitlab.com/gnachman/iterm2/-/issues/7007) | Regression in handling of ⌘+mouseclick on links |
| [#6775](https://gitlab.com/gnachman/iterm2/-/issues/6775) | Feature Request: Support ws:// urls for at least node-inspect-brk |
| [#6533](https://gitlab.com/gnachman/iterm2/-/issues/6533) | Improve UX of issue tracker page on product website |
| [#6483](https://gitlab.com/gnachman/iterm2/-/issues/6483) | cmd+click on a URL (using sematic history) redirects to wrong address, and selects the newline char ... |
| [#6255](https://gitlab.com/gnachman/iterm2/-/issues/6255) | Url encoding bug |
| [#5954](https://gitlab.com/gnachman/iterm2/-/issues/5954) | Quicklook for webp is showing broken image |
| [#5679](https://gitlab.com/gnachman/iterm2/-/issues/5679) | DashTerm2 3.1beta3 is herky-jerky and blinky |
| [#5499](https://gitlab.com/gnachman/iterm2/-/issues/5499) | implement copy-paste protection against malicious web pages |
| [#5181](https://gitlab.com/gnachman/iterm2/-/issues/5181) | DashTerm2 3.0.9 chokes on large curl with multiple line continuations '\'?!? |
| [#5177](https://gitlab.com/gnachman/iterm2/-/issues/5177) | create and unlink file takes a few seconds |
| [#5065](https://gitlab.com/gnachman/iterm2/-/issues/5065) | URLs not clickable when preceded by line number or filename (results from grep, ag, ack, etc.) |
| [#4694](https://gitlab.com/gnachman/iterm2/-/issues/4694) | LSOpenURLsWithRole() failed for the application |
| [#4517](https://gitlab.com/gnachman/iterm2/-/issues/4517) | Iterm2 problem with MAC, all links or buttons that target a web page is opened with the iterm 2 |
| [#4502](https://gitlab.com/gnachman/iterm2/-/issues/4502) | file links mistakenly interpreted as URLs |
| [#4377](https://gitlab.com/gnachman/iterm2/-/issues/4377) | "open selection as URL" selects trailing parenthesis when entire URL is parenthesized |
| [#3159](https://gitlab.com/gnachman/iterm2/-/issues/3159) | Support multiple URLs in open URL from selection |
| [#3098](https://gitlab.com/gnachman/iterm2/-/issues/3098) | Download link is not HTTPs |
| [#2755](https://gitlab.com/gnachman/iterm2/-/issues/2755) | Ability to open a file by Option-clicking on its name/path in the terminal - hyperlinks |
| [#1481](https://gitlab.com/gnachman/iterm2/-/issues/1481) |  Webkit instance inside iterm2 |
| [#901](https://gitlab.com/gnachman/iterm2/-/issues/901) | URL bar, wildcard/global profiles |

---

## Profile and Settings (P3)

**Count:** 142

| Issue | Title |
|-------|-------|
| [#12569](https://gitlab.com/gnachman/iterm2/-/issues/12569) | Detect prompt on paste setting not reflected in dialog |
| [#12383](https://gitlab.com/gnachman/iterm2/-/issues/12383) | Badge placement doesn't reflect settings |
| [#12376](https://gitlab.com/gnachman/iterm2/-/issues/12376) | disabling browse-style profiles doesn't actually disable them |
| [#12354](https://gitlab.com/gnachman/iterm2/-/issues/12354) | Trigger-based profile switching into remote machine via tsh/teleport causes 'Paste-Formatting left o... |
| [#12278](https://gitlab.com/gnachman/iterm2/-/issues/12278) | Format for log timestamps advanced setting does not work |
| [#12223](https://gitlab.com/gnachman/iterm2/-/issues/12223) | Auto-start a profile on launch |
| [#11834](https://gitlab.com/gnachman/iterm2/-/issues/11834) | "Selection of current command" needs explanation of framing/dimming in Settings UI |
| [#11824](https://gitlab.com/gnachman/iterm2/-/issues/11824) | Documentation / Python API inconsistencies for `async_set_preference` |
| [#11737](https://gitlab.com/gnachman/iterm2/-/issues/11737) | Allow update checker to also look for beta versions (configurable) |
| [#11461](https://gitlab.com/gnachman/iterm2/-/issues/11461) | Vim disregards t_Co setting after DashTerm2 update |
| [#11351](https://gitlab.com/gnachman/iterm2/-/issues/11351) | System Settings pops up spontaneously at random intervals |
| [#11341](https://gitlab.com/gnachman/iterm2/-/issues/11341) | automatic profile switching does not work well with wildcards |
| [#11283](https://gitlab.com/gnachman/iterm2/-/issues/11283) | Python API: Support "Duplicate Profile" action |
| [#11221](https://gitlab.com/gnachman/iterm2/-/issues/11221) | Profile with command cuts the first letter |
| [#11095](https://gitlab.com/gnachman/iterm2/-/issues/11095) | Pressing long enter to get a new prompt leads to unintended setting marker lines |
| [#11024](https://gitlab.com/gnachman/iterm2/-/issues/11024) | Assign Actions List to Profile |
| [#10970](https://gitlab.com/gnachman/iterm2/-/issues/10970) | Syncing preference between two mac using iCloud doesn't work as expected |
| [#10962](https://gitlab.com/gnachman/iterm2/-/issues/10962) | Settings/Profile always restored to default after quitting iterm |
| [#10872](https://gitlab.com/gnachman/iterm2/-/issues/10872) | Follow system/Safari search engine preference |
| [#10779](https://gitlab.com/gnachman/iterm2/-/issues/10779) | Aspect Ratio issue with Preferences Icons - See screenshots |
| [#10720](https://gitlab.com/gnachman/iterm2/-/issues/10720) | Want help. How can I get ${profile.name} in Smart Selection Rule Parameter |
| [#10582](https://gitlab.com/gnachman/iterm2/-/issues/10582) | Invalid Profile Error on 2021 M1 Macbook Pro |
| [#10574](https://gitlab.com/gnachman/iterm2/-/issues/10574) | Profile Specific Snippets |
| [#10549](https://gitlab.com/gnachman/iterm2/-/issues/10549) | Non Stop Saving of Preferences |
| [#10536](https://gitlab.com/gnachman/iterm2/-/issues/10536) | Inconsistency in setting locale automatically, English vs. English (US) |
| [#10451](https://gitlab.com/gnachman/iterm2/-/issues/10451) | iterm2 cannot properly handle setting invalid profiles |
| [#10374](https://gitlab.com/gnachman/iterm2/-/issues/10374) | Sync preferences with file on disk |
| [#10250](https://gitlab.com/gnachman/iterm2/-/issues/10250) | Allow storage of preferences in plain text instead of binary block |
| [#10248](https://gitlab.com/gnachman/iterm2/-/issues/10248) | Duplicate preference |
| [#10140](https://gitlab.com/gnachman/iterm2/-/issues/10140) | Profile seletion dialog: text entry is prefilled with garbage |
| [#10134](https://gitlab.com/gnachman/iterm2/-/issues/10134) | setting terminal width using ESC sequence is running async |
| [#10064](https://gitlab.com/gnachman/iterm2/-/issues/10064) | DashTerm2 Profile Settings Sync |
| [#9904](https://gitlab.com/gnachman/iterm2/-/issues/9904) | "Send text at start" does not always use the directory for the profile |
| [#9842](https://gitlab.com/gnachman/iterm2/-/issues/9842) | m1 macbook pro set profile close terminal profile lost Bug report |
| [#9818](https://gitlab.com/gnachman/iterm2/-/issues/9818) | Mis-setting locale |
| [#9810](https://gitlab.com/gnachman/iterm2/-/issues/9810) | Enable writing of preferences |
| [#9453](https://gitlab.com/gnachman/iterm2/-/issues/9453) | Automatic Profile Switching: either explanation is incomplete or important feature is missing |
| [#9382](https://gitlab.com/gnachman/iterm2/-/issues/9382) | Editing Smart Selection Regexes affects multiple Profiles |
| [#9262](https://gitlab.com/gnachman/iterm2/-/issues/9262) | Please restore opening default profile on startup |
| [#8831](https://gitlab.com/gnachman/iterm2/-/issues/8831) | Feature Request: configure Open Quickly dialogue to only show open sessions |
| [#8679](https://gitlab.com/gnachman/iterm2/-/issues/8679) | Version 3.8.8 - Dynamic profiles constantly complain about incomplete JSON |
| [#8566](https://gitlab.com/gnachman/iterm2/-/issues/8566) | Suggestion: move Status Bar preferences |
| [#8547](https://gitlab.com/gnachman/iterm2/-/issues/8547) | Preferences file should store long term preferences without short term state |
| [#8423](https://gitlab.com/gnachman/iterm2/-/issues/8423) | Deleting multiple profiles messes with the Profile menu |
| [#8407](https://gitlab.com/gnachman/iterm2/-/issues/8407) | [feature request] profile switching depend of current screen resolution |
| [#8361](https://gitlab.com/gnachman/iterm2/-/issues/8361) | Failed to load preferences from custom directory. Falling back to local copy. |
| [#8342](https://gitlab.com/gnachman/iterm2/-/issues/8342) | [Feature Request] Toolbelt preference to set which side it is displayed on |
| [#8253](https://gitlab.com/gnachman/iterm2/-/issues/8253) | Question about DynamicProfiles and DashTerm2 JSON |
| [#8079](https://gitlab.com/gnachman/iterm2/-/issues/8079) | Profiles not getting applied |
| [#8002](https://gitlab.com/gnachman/iterm2/-/issues/8002) | Default status bar configuration? |
| [#7927](https://gitlab.com/gnachman/iterm2/-/issues/7927) | Setting up automatically profile switching isn't obvious when using Triggers |
| [#7910](https://gitlab.com/gnachman/iterm2/-/issues/7910) | Only able to use terminal while at work, home profile not pulling up when I'm remote |
| [#7845](https://gitlab.com/gnachman/iterm2/-/issues/7845) | Dynamic profile without "Dynamic" tag. |
| [#7829](https://gitlab.com/gnachman/iterm2/-/issues/7829) | Unable to create a dynamic profile for all hosts |
| [#7802](https://gitlab.com/gnachman/iterm2/-/issues/7802) | preferences and saved sessions gone |
| [#7656](https://gitlab.com/gnachman/iterm2/-/issues/7656) | Status bar is not displayed and is not available in Preferences |
| [#7565](https://gitlab.com/gnachman/iterm2/-/issues/7565) | [Feature Request] Commands in Arrangements Preferences |
| [#7498](https://gitlab.com/gnachman/iterm2/-/issues/7498) | Feature Request: bulk ops on profiles |
| [#7385](https://gitlab.com/gnachman/iterm2/-/issues/7385) | This session’s profile, “Default”, no longer exists. A profile with that name happens to exist. |
| [#7073](https://gitlab.com/gnachman/iterm2/-/issues/7073) | Create AppleScript to open DashTerm2 with a profile different than default. |
| [#7038](https://gitlab.com/gnachman/iterm2/-/issues/7038) | [Wishlist] Ignore "session ended very soon" by command or profile instead of globally |
| [#6968](https://gitlab.com/gnachman/iterm2/-/issues/6968) | Cursor setting not working as expected |
| [#6951](https://gitlab.com/gnachman/iterm2/-/issues/6951) | Readline's vi mode indicator is moved to its own line with "Insert new line before prompt" setting |
| [#6903](https://gitlab.com/gnachman/iterm2/-/issues/6903) | feature request: support cloud sync of settings |
| [#6866](https://gitlab.com/gnachman/iterm2/-/issues/6866) | [Scripting] [Feature request] Automatic profile switching hook |
| [#6839](https://gitlab.com/gnachman/iterm2/-/issues/6839) | Feature Request - Global setting to disable full-screen |
| [#6836](https://gitlab.com/gnachman/iterm2/-/issues/6836) | Feature Request - Separate "Profiles" from "Hosts" |
| [#6835](https://gitlab.com/gnachman/iterm2/-/issues/6835) | Faster profile editing |
| [#6776](https://gitlab.com/gnachman/iterm2/-/issues/6776) | Type check dynamic profiles |
| [#6611](https://gitlab.com/gnachman/iterm2/-/issues/6611) | Improved interface for setting titles |
| [#6578](https://gitlab.com/gnachman/iterm2/-/issues/6578) | Man page ([i] button) does not respect man path, shell settings |
| [#6536](https://gitlab.com/gnachman/iterm2/-/issues/6536) | iterm2 loses default profile when respawning after logging back into my MAC |
| [#6518](https://gitlab.com/gnachman/iterm2/-/issues/6518) | Inheritance of triggers between profiles |
| [#6506](https://gitlab.com/gnachman/iterm2/-/issues/6506) | Investigate resetting more stuff on shell prompt |
| [#6502](https://gitlab.com/gnachman/iterm2/-/issues/6502) | Self-updater should use system proxy settings |
| [#6457](https://gitlab.com/gnachman/iterm2/-/issues/6457) | Feature request: Profiles thumbs preview |
| [#6367](https://gitlab.com/gnachman/iterm2/-/issues/6367) | DashTerm2 not settings automatic marks with powerlevel9k |
| [#6275](https://gitlab.com/gnachman/iterm2/-/issues/6275) | Feature request: Menu bar icon to quickly select profiles |
| [#6245](https://gitlab.com/gnachman/iterm2/-/issues/6245) | [feature request] Right click on profile in toolbelt to edit |
| [#6161](https://gitlab.com/gnachman/iterm2/-/issues/6161) | DashTerm2 is not remembering my custom preferences folder |
| [#6145](https://gitlab.com/gnachman/iterm2/-/issues/6145) | iTerm stores profiles in `~/Library/Saved Application State` which is not backed up by time machine |
| [#6135](https://gitlab.com/gnachman/iterm2/-/issues/6135) | Profiles should inherit defaults |
| [#6124](https://gitlab.com/gnachman/iterm2/-/issues/6124) | Thin strokes for anti-alias settings have no effects |
| [#5935](https://gitlab.com/gnachman/iterm2/-/issues/5935) | Profiles- Include pointer to pem files |
| [#5864](https://gitlab.com/gnachman/iterm2/-/issues/5864) | Unchecking "Copy to pasteboard on selection" in 'DashTerm2 > Preferences... > General' does not actuall... |
| [#5749](https://gitlab.com/gnachman/iterm2/-/issues/5749) | Preference for Advanced Paste to turn off "Wait for shell prompt before pasting each line" |
| [#5666](https://gitlab.com/gnachman/iterm2/-/issues/5666) | iTerm does not provide a way to launch all profiles associated with a tag |
| [#5631](https://gitlab.com/gnachman/iterm2/-/issues/5631) | Preferences fail to load when opening iterm2 |
| [#5590](https://gitlab.com/gnachman/iterm2/-/issues/5590) | Feature Request: Regular Expressions for searching profiles |
| [#5586](https://gitlab.com/gnachman/iterm2/-/issues/5586) | [Feature Request] Make GUID of currently active profile accessible via script |
| [#5571](https://gitlab.com/gnachman/iterm2/-/issues/5571) | applescript get list of profiles [INFO / FEATURE REQUEST] |
| [#5544](https://gitlab.com/gnachman/iterm2/-/issues/5544) | support emacs in preferences->profile->advanced->semantic history->open with editor |
| [#5543](https://gitlab.com/gnachman/iterm2/-/issues/5543) | support emacs in preferences->profile->advanced->semantic history->open with editor |
| [#5500](https://gitlab.com/gnachman/iterm2/-/issues/5500) | terminates when setting bounds with applescript |
| [#5470](https://gitlab.com/gnachman/iterm2/-/issues/5470) | Allow for setting profile name and creating new profile programmatically |
| [#5419](https://gitlab.com/gnachman/iterm2/-/issues/5419) | Auto Profile Switch Works For One Host But Not For Another |
| [#5373](https://gitlab.com/gnachman/iterm2/-/issues/5373) | Automatic profile switching with docker containers |
| [#5371](https://gitlab.com/gnachman/iterm2/-/issues/5371) | Feature Request - Add a description column to Profile Triggers |
| [#5335](https://gitlab.com/gnachman/iterm2/-/issues/5335) | Feature request: profile as an "app" |
| [#5307](https://gitlab.com/gnachman/iterm2/-/issues/5307) | Allow setting arbitrary environment variables in profile |
| [#5254](https://gitlab.com/gnachman/iterm2/-/issues/5254) | Number text boxes in the Preferences can not be edited while system preferred language is Persian |
| [#5246](https://gitlab.com/gnachman/iterm2/-/issues/5246) | Feature request - environment variable modification/setting on switching profiles |
| [#5165](https://gitlab.com/gnachman/iterm2/-/issues/5165) | Allow un-suppressing short-lived session warning for each profile |
| [#5131](https://gitlab.com/gnachman/iterm2/-/issues/5131) | Profile settings not sticking |
| [#5106](https://gitlab.com/gnachman/iterm2/-/issues/5106) | Feature Request: Profiles List Improvements |
| [#5083](https://gitlab.com/gnachman/iterm2/-/issues/5083) | ER: iTerm should support 'Use Style for Copy Command' for a different profile than the currently act... |
| [#5023](https://gitlab.com/gnachman/iterm2/-/issues/5023) | Unable to automatically switch profile with synced preferences |
| [#4822](https://gitlab.com/gnachman/iterm2/-/issues/4822) | Feature request: Please provide applescript for setting the terminal 'badge' |
| [#4796](https://gitlab.com/gnachman/iterm2/-/issues/4796) | Feature request: shell configuration injection (sourcing files stored in profiles rather than on rem... |
| [#4709](https://gitlab.com/gnachman/iterm2/-/issues/4709) | Problem saving Automatic Profile Switching rules |
| [#4600](https://gitlab.com/gnachman/iterm2/-/issues/4600) | Semantic history matching pattern not documented or configurable |
| [#4569](https://gitlab.com/gnachman/iterm2/-/issues/4569) | Config sync using cloud services, end-to-end encryption. |
| [#4525](https://gitlab.com/gnachman/iterm2/-/issues/4525) | Feature request: restore `Show cursor guid` and `Show timestamps` settings after iTerm restart |
| [#4328](https://gitlab.com/gnachman/iterm2/-/issues/4328) | DashTerm2 Version 3 Beta Automatic Profile Switching failed with logout from remote host |
| [#4322](https://gitlab.com/gnachman/iterm2/-/issues/4322) | Proposal: Resizable or Preferences Size setting for Paste History. |
| [#4289](https://gitlab.com/gnachman/iterm2/-/issues/4289) | [Feature] Toolbelt: Search icon slides down Search bar (hidden by default) -» more space (Profiles, ... |
| [#4227](https://gitlab.com/gnachman/iterm2/-/issues/4227) | UI glitches in preferences |
| [#4200](https://gitlab.com/gnachman/iterm2/-/issues/4200) | "Reuse previous session's directory" as "Working Directory" preference does not work |
| [#4188](https://gitlab.com/gnachman/iterm2/-/issues/4188) | Search preferences as in PyCharm |
| [#4141](https://gitlab.com/gnachman/iterm2/-/issues/4141) | Deprecate -[ProfileModel sessionsInstance] |
| [#4098](https://gitlab.com/gnachman/iterm2/-/issues/4098) | Feature request: save dynamic profiles in sync folder |
| [#4097](https://gitlab.com/gnachman/iterm2/-/issues/4097) | Add more docs for Dynamic Profiles |
| [#4051](https://gitlab.com/gnachman/iterm2/-/issues/4051) | Profile Editor: select multiple profiles |
| [#4047](https://gitlab.com/gnachman/iterm2/-/issues/4047) | Enhancement Idea: "Grouping Profiles -  Tree Group under Preferences" and "Moving multiple Profiles ... |
| [#3991](https://gitlab.com/gnachman/iterm2/-/issues/3991) | Setting LC_CTYPE=UTF-8 is problematic with many Linux systems |
| [#3902](https://gitlab.com/gnachman/iterm2/-/issues/3902) | Add setting for LSUIElement |
| [#3879](https://gitlab.com/gnachman/iterm2/-/issues/3879) | Automatic profile switching based on directory [request] |
| [#3692](https://gitlab.com/gnachman/iterm2/-/issues/3692) | IDEA: Allow trigger to switch profile |
| [#3381](https://gitlab.com/gnachman/iterm2/-/issues/3381) | Save preferences to custom folder automatically |
| [#3287](https://gitlab.com/gnachman/iterm2/-/issues/3287) | Collapse/ Expand the Tag Names in Toolbelt - Profiles |
| [#3029](https://gitlab.com/gnachman/iterm2/-/issues/3029) | Allow editing multiple profiles at once [was: "default" values for profiles] |
| [#2791](https://gitlab.com/gnachman/iterm2/-/issues/2791) | Don't allow "do not remap" in a profile. It doesn't make sense. |
| [#2787](https://gitlab.com/gnachman/iterm2/-/issues/2787) | Allow override of profile username |
| [#2710](https://gitlab.com/gnachman/iterm2/-/issues/2710) | Transparency/Blur Value (Label) in Preferences |
| [#2267](https://gitlab.com/gnachman/iterm2/-/issues/2267) | Profile Side Menu with Tree structure |
| [#2148](https://gitlab.com/gnachman/iterm2/-/issues/2148) | User configurable gutter |
| [#2095](https://gitlab.com/gnachman/iterm2/-/issues/2095) | Invoke specific profiles when starting DashTerm2 from command line |
| [#2036](https://gitlab.com/gnachman/iterm2/-/issues/2036) | Add profile for use when iTerm is opened automatically |
| [#1854](https://gitlab.com/gnachman/iterm2/-/issues/1854) | alert user to TERM settings that will break emacs and vi, e.g. ansi |
| [#1504](https://gitlab.com/gnachman/iterm2/-/issues/1504) | Encrypted Preferences |
| [#1040](https://gitlab.com/gnachman/iterm2/-/issues/1040) | Choose Profile To Open DashTerm2 |
| [#959](https://gitlab.com/gnachman/iterm2/-/issues/959) | Defuse Bookmarks from Profile |

---

## AppleScript and API (P3)

**Count:** 73

| Issue | Title |
|-------|-------|
| [#12400](https://gitlab.com/gnachman/iterm2/-/issues/12400) | Commands sent via Python API truncated / executed too early during shell startup |
| [#12151](https://gitlab.com/gnachman/iterm2/-/issues/12151) | Command PhaseScriptExecution failed with a nonzero exit code |
| [#11684](https://gitlab.com/gnachman/iterm2/-/issues/11684) | Python runtime updates are too frequent and should fail silently |
| [#11446](https://gitlab.com/gnachman/iterm2/-/issues/11446) | Can not create an initialization script that works |
| [#11352](https://gitlab.com/gnachman/iterm2/-/issues/11352) | pagesize returns 4096 in the python api but 16384 on the command line |
| [#11308](https://gitlab.com/gnachman/iterm2/-/issues/11308) | [Feature Request][Python Scripts] Add a machine-wide scripting folder |
| [#10835](https://gitlab.com/gnachman/iterm2/-/issues/10835) | Python API: Is It Possible To Pipe Command Output Between Sessions? |
| [#10740](https://gitlab.com/gnachman/iterm2/-/issues/10740) | [M2] Python script engine is x64 instead of arm64 |
| [#10711](https://gitlab.com/gnachman/iterm2/-/issues/10711) | StatusBar component with Python API |
| [#10660](https://gitlab.com/gnachman/iterm2/-/issues/10660) | it2api requires python 3.7, but Ventura defaults is python 3.9 |
| [#10642](https://gitlab.com/gnachman/iterm2/-/issues/10642) | in API, session.grid_size doesn't seem to match session |
| [#10504](https://gitlab.com/gnachman/iterm2/-/issues/10504) | When AppleScript it truly trashed, not even the Python API Security Escape Hatch can work !!!!!! |
| [#10434](https://gitlab.com/gnachman/iterm2/-/issues/10434) | Python runtime recently stopped working |
| [#10359](https://gitlab.com/gnachman/iterm2/-/issues/10359) | API dependency not installed, GUI not working |
| [#10268](https://gitlab.com/gnachman/iterm2/-/issues/10268) | Scripts deactivated upon upgrade |
| [#10231](https://gitlab.com/gnachman/iterm2/-/issues/10231) | Allow status bar components to gather their data from python scripts |
| [#9896](https://gitlab.com/gnachman/iterm2/-/issues/9896) | Pasting while in python shell |
| [#9885](https://gitlab.com/gnachman/iterm2/-/issues/9885) | Double/Triple Tap top run command and/or script |
| [#9759](https://gitlab.com/gnachman/iterm2/-/issues/9759) | Add ITERM2_COOKIE environment variable and launch shell scripts in the AutoLaunch folder. |
| [#9757](https://gitlab.com/gnachman/iterm2/-/issues/9757) | Add support for subscript and superscript |
| [#9651](https://gitlab.com/gnachman/iterm2/-/issues/9651) | NewSessionMonitor does not fire from AutoLaunch scripts for startup session |
| [#9599](https://gitlab.com/gnachman/iterm2/-/issues/9599) | Add user tmpdir to python script invocations |
| [#9349](https://gitlab.com/gnachman/iterm2/-/issues/9349) | Problem installing Python packages in runtime |
| [#9258](https://gitlab.com/gnachman/iterm2/-/issues/9258) | Python API not working in latest version |
| [#9236](https://gitlab.com/gnachman/iterm2/-/issues/9236) | Can't Download Python Runtime |
| [#9225](https://gitlab.com/gnachman/iterm2/-/issues/9225) | [Feature Request]: custom annotation notification script |
| [#9222](https://gitlab.com/gnachman/iterm2/-/issues/9222) | Python API broadcast does not work with multiple broadcast domains |
| [#9095](https://gitlab.com/gnachman/iterm2/-/issues/9095) | Feature Request: API endpoint to detect enabled/disabled API |
| [#9005](https://gitlab.com/gnachman/iterm2/-/issues/9005) | Scripting interface - Extended Scripting User Interface |
| [#8913](https://gitlab.com/gnachman/iterm2/-/issues/8913) | Expose status bar setup to Python API |
| [#8714](https://gitlab.com/gnachman/iterm2/-/issues/8714) | how to use python api without "Allow Python API Usage ?" |
| [#8440](https://gitlab.com/gnachman/iterm2/-/issues/8440) | Python API: coroutine return value should not be discarded |
| [#8393](https://gitlab.com/gnachman/iterm2/-/issues/8393) | Remove aioconsole after adopting python 3.8 |
| [#8383](https://gitlab.com/gnachman/iterm2/-/issues/8383) | Problems with scripting |
| [#8224](https://gitlab.com/gnachman/iterm2/-/issues/8224) | Python API general feedback |
| [#8036](https://gitlab.com/gnachman/iterm2/-/issues/8036) | [Feature Request] Support terminal application scripting ".iterm" files |
| [#8019](https://gitlab.com/gnachman/iterm2/-/issues/8019) | how to use bpython as REPL |
| [#7891](https://gitlab.com/gnachman/iterm2/-/issues/7891) | Triggers action "Run Command" cannot run any python script |
| [#7827](https://gitlab.com/gnachman/iterm2/-/issues/7827) | Integrating iterm2-tools into the official iterm2 Python library |
| [#7752](https://gitlab.com/gnachman/iterm2/-/issues/7752) | DashTerm2's Python environments are reported as a 30 MB download, but take up about 400 MB of space whe... |
| [#7709](https://gitlab.com/gnachman/iterm2/-/issues/7709) | In 3.3.0b there's a delay when calling shell scripts |
| [#7453](https://gitlab.com/gnachman/iterm2/-/issues/7453) | Feature Request: Nested AppleScript Scripts |
| [#7027](https://gitlab.com/gnachman/iterm2/-/issues/7027) | Python signal on shutdown of iterm is not firing |
| [#7005](https://gitlab.com/gnachman/iterm2/-/issues/7005) | Clarify state of scripting / it2API / ws:// etc... |
| [#6922](https://gitlab.com/gnachman/iterm2/-/issues/6922) | [Feature] Touch bar: scripted dynamic buttons |
| [#6740](https://gitlab.com/gnachman/iterm2/-/issues/6740) | [Scripting] What is the bare minimum required for the Python-API to work? |
| [#6725](https://gitlab.com/gnachman/iterm2/-/issues/6725) | how to disable 'API Access Request' prompt |
| [#6646](https://gitlab.com/gnachman/iterm2/-/issues/6646) | Improve/update esctest scripts |
| [#6284](https://gitlab.com/gnachman/iterm2/-/issues/6284) | Capital letters lost in restored session |
| [#6221](https://gitlab.com/gnachman/iterm2/-/issues/6221) | [Scripting-JXA] Get Content of iTerm Session |
| [#5998](https://gitlab.com/gnachman/iterm2/-/issues/5998) | CLI Automation with JXA (JavaScript) |
| [#5927](https://gitlab.com/gnachman/iterm2/-/issues/5927) | Add Script extension .applescript to Scripts menu |
| [#5907](https://gitlab.com/gnachman/iterm2/-/issues/5907) | DashTerm2 not reaping child processes when killed by init |
| [#5772](https://gitlab.com/gnachman/iterm2/-/issues/5772) | Trigger action to annotate with the result of a custom script |
| [#5771](https://gitlab.com/gnachman/iterm2/-/issues/5771) | Applescript control over DashTerm2 became sluggish in 3.1beta4 |
| [#5703](https://gitlab.com/gnachman/iterm2/-/issues/5703) | Applescript issue in beta 3.1 |
| [#5462](https://gitlab.com/gnachman/iterm2/-/issues/5462) | AppleScript can write text to a wrong iTerm session if the initial one got killed and another one wa... |
| [#5411](https://gitlab.com/gnachman/iterm2/-/issues/5411) | [Feature request] Applescript to set session title |
| [#4832](https://gitlab.com/gnachman/iterm2/-/issues/4832) | applescript write text with bracketed paste mode |
| [#4786](https://gitlab.com/gnachman/iterm2/-/issues/4786) | Runtime error when using iterm2 or iterm3 with brew installed python |
| [#4784](https://gitlab.com/gnachman/iterm2/-/issues/4784) | Please Add "text of current session" Example to AppleScript Documentation |
| [#4729](https://gitlab.com/gnachman/iterm2/-/issues/4729) | DashTerm2 3.0.0 fails both self update and fresh install on  El Capitan |
| [#4421](https://gitlab.com/gnachman/iterm2/-/issues/4421) | (Feature request) Access session through AppleScript directly using sesion ID |
| [#4354](https://gitlab.com/gnachman/iterm2/-/issues/4354) | Scripting interface to iTerm (beta) broken? |
| [#4003](https://gitlab.com/gnachman/iterm2/-/issues/4003) | yosemite 2.1.4 regressions, "2.1.4 only recommended for El Capitan" |
| [#3768](https://gitlab.com/gnachman/iterm2/-/issues/3768) | Feature Request: Applescript to set badge name |
| [#3207](https://gitlab.com/gnachman/iterm2/-/issues/3207) | Scripting: manipulation of marks |
| [#3206](https://gitlab.com/gnachman/iterm2/-/issues/3206) | Scripting: manipulation of annotations |
| [#3193](https://gitlab.com/gnachman/iterm2/-/issues/3193) | Scripting: allow position-based creation, query & navigation of sessions. |
| [#3189](https://gitlab.com/gnachman/iterm2/-/issues/3189) | Scripting: accessors for is-focused/is-visible attributes of things |
| [#2570](https://gitlab.com/gnachman/iterm2/-/issues/2570) | Trigger API |
| [#1968](https://gitlab.com/gnachman/iterm2/-/issues/1968) | Log file reaping |
| [#1092](https://gitlab.com/gnachman/iterm2/-/issues/1092) | Semantic history handler should recognize line numbers from Python stack traces. |

---

## macOS Version Specific (P2)

**Count:** 31

| Issue | Title |
|-------|-------|
| [#12573](https://gitlab.com/gnachman/iterm2/-/issues/12573) | DashTerm2 detected as malware on macOS |
| [#12496](https://gitlab.com/gnachman/iterm2/-/issues/12496) | iTerm Appearance: Compact + Tahoe Accessibility: Increased Contrast = Cosmetic bug? |
| [#12379](https://gitlab.com/gnachman/iterm2/-/issues/12379) | You Can't Use ... With This Version of MacOS |
| [#12343](https://gitlab.com/gnachman/iterm2/-/issues/12343) | Latest nightly builds (starting in the last 3.5 build) don't work with macOS 26 anymore. |
| [#11139](https://gitlab.com/gnachman/iterm2/-/issues/11139) | 3.5beta13 opens grey page under Sonoma 14.0 |
| [#11084](https://gitlab.com/gnachman/iterm2/-/issues/11084) | iTerm 3.4.20 does not launch under macOS 10.14.6 (intel) |
| [#11040](https://gitlab.com/gnachman/iterm2/-/issues/11040) | New macOS public beta broke DashTerm2 beta (not urgent) |
| [#10891](https://gitlab.com/gnachman/iterm2/-/issues/10891) | Patched MacOS Ventura 13.3. and password manager does not take my authentication anymore |
| [#10824](https://gitlab.com/gnachman/iterm2/-/issues/10824) | Terminal doesn't open where the cursor is, even when forcing second monitor. macOS Ventura 13.2.1 |
| [#10519](https://gitlab.com/gnachman/iterm2/-/issues/10519) | Even if Full Disk Access is granted, Monterey keeps asking for permission to give DashTerm2 access to p... |
| [#10515](https://gitlab.com/gnachman/iterm2/-/issues/10515) | Open DashTerm2 in folder on MacOS |
| [#10204](https://gitlab.com/gnachman/iterm2/-/issues/10204) | cannot open iterm2 on mac m1 big sur 11.6 |
| [#10200](https://gitlab.com/gnachman/iterm2/-/issues/10200) | Semantic history stopped working after upgrade to Monterey |
| [#10182](https://gitlab.com/gnachman/iterm2/-/issues/10182) | iTerm blocks MissionControl (macOS Dock) |
| [#10072](https://gitlab.com/gnachman/iterm2/-/issues/10072) | A blank screen after launching DashTerm2 in macOS 12 Monterey |
| [#10057](https://gitlab.com/gnachman/iterm2/-/issues/10057) | Cannot enable popup notifications on macOS 11.6 |
| [#9953](https://gitlab.com/gnachman/iterm2/-/issues/9953) | Trigger Action "Inject Data" is missing on macOS |
| [#9780](https://gitlab.com/gnachman/iterm2/-/issues/9780) | DashTerm2 selection goes crazy on macOS Monterey |
| [#9616](https://gitlab.com/gnachman/iterm2/-/issues/9616) | Allow the password manager to be unlocked with macOS's smart card feature |
| [#9405](https://gitlab.com/gnachman/iterm2/-/issues/9405) | OS X terminal vs DashTerm2 (new behavior in Big Sur) |
| [#9307](https://gitlab.com/gnachman/iterm2/-/issues/9307) | iTerm 3.4.2 cannot copy iTermServer on MacOS 10.15 and cannot open the Default session |
| [#9295](https://gitlab.com/gnachman/iterm2/-/issues/9295) | Dock Icon looks strange with Big Sur and Version 3.4.1 |
| [#9030](https://gitlab.com/gnachman/iterm2/-/issues/9030) | MacOS X "Update to beta test releases" being unchecked not being respected. |
| [#8965](https://gitlab.com/gnachman/iterm2/-/issues/8965) | On Big Sur, session restoration will eventually kill the shell in iTerm and Terminal.app |
| [#8964](https://gitlab.com/gnachman/iterm2/-/issues/8964) | Big Sur — Master Issue |
| [#8515](https://gitlab.com/gnachman/iterm2/-/issues/8515) | Low framerate on macOS 10.15.1 on 16" MBP with Intel UHD Graphics 630 / AMD Radeon Pro 5500M |
| [#8451](https://gitlab.com/gnachman/iterm2/-/issues/8451) | Session restore does not get correct permissions/macOS session |
| [#8372](https://gitlab.com/gnachman/iterm2/-/issues/8372) | Odd Catalina pop-up |
| [#7859](https://gitlab.com/gnachman/iterm2/-/issues/7859) | [Catalina] Add support for voice control |
| [#7336](https://gitlab.com/gnachman/iterm2/-/issues/7336) | Version 3.2.5 does not build on macOS < 10.14 |
| [#7298](https://gitlab.com/gnachman/iterm2/-/issues/7298) | MacOS Mojave: Microphone permission? |

---

## Other Issues (P3)

**Count:** 801

| Issue | Title |
|-------|-------|
| [#12652](https://gitlab.com/gnachman/iterm2/-/issues/12652) | How to control DashTerm2 progress bar |
| [#12638](https://gitlab.com/gnachman/iterm2/-/issues/12638) | Add a minimap view |
| [#12617](https://gitlab.com/gnachman/iterm2/-/issues/12617) | Feature Request: Support for tput ll |
| [#12601](https://gitlab.com/gnachman/iterm2/-/issues/12601) | white line on top of DashTerm2 |
| [#12567](https://gitlab.com/gnachman/iterm2/-/issues/12567) | Password manager with Enpass support |
| [#12557](https://gitlab.com/gnachman/iterm2/-/issues/12557) | CustomControlSequenceMonitor does not expose which session it came from |
| [#12532](https://gitlab.com/gnachman/iterm2/-/issues/12532) | Shell Extensions + bash + AutoComposer: overly aggressive uncalled-for autocomplete (ls -> lstopo) |
| [#12531](https://gitlab.com/gnachman/iterm2/-/issues/12531) | Remove `\` in the displaying commands |
| [#12503](https://gitlab.com/gnachman/iterm2/-/issues/12503) | [Filter] Weird behaviour when cleaning it |
| [#12495](https://gitlab.com/gnachman/iterm2/-/issues/12495) | OSX Seqoia firewall blocks network connectivity after upgrade |
| [#12493](https://gitlab.com/gnachman/iterm2/-/issues/12493) | Feature Request: Customisable Session Restored and Exit Messages |
| [#12492](https://gitlab.com/gnachman/iterm2/-/issues/12492) | Text jigger when sitting in stage manager |
| [#12488](https://gitlab.com/gnachman/iterm2/-/issues/12488) | iterm2 doesn't handle XTGETTCAP (which caused by LeaderF plugin of Vim) correctly? |
| [#12484](https://gitlab.com/gnachman/iterm2/-/issues/12484) | PDF reader plugin |
| [#12483](https://gitlab.com/gnachman/iterm2/-/issues/12483) | Unable to use Kitty image protocol |
| [#12466](https://gitlab.com/gnachman/iterm2/-/issues/12466) | Update checker does not work correctly when "Update to Beta test releases" is enabled |
| [#12454](https://gitlab.com/gnachman/iterm2/-/issues/12454) | Graphical glitch when unfocused with Stage Manager on |
| [#12346](https://gitlab.com/gnachman/iterm2/-/issues/12346) | Are simple conditionals possible for expressions / interpolated strings? |
| [#12335](https://gitlab.com/gnachman/iterm2/-/issues/12335) | New "after session ends" behavior: "close if successful" |
| [#12319](https://gitlab.com/gnachman/iterm2/-/issues/12319) | Go to terminal by TTY filename |
| [#12318](https://gitlab.com/gnachman/iterm2/-/issues/12318) | Output suppression indicator not needed for other apps |
| [#12301](https://gitlab.com/gnachman/iterm2/-/issues/12301) | Easier Access to Arrangements |
| [#12296](https://gitlab.com/gnachman/iterm2/-/issues/12296) | mouse reporting keeps getting turned off |
| [#12288](https://gitlab.com/gnachman/iterm2/-/issues/12288) | Stylistic sets not working since 3.5.12 |
| [#12270](https://gitlab.com/gnachman/iterm2/-/issues/12270) | Add support for Proton Pass |
| [#12263](https://gitlab.com/gnachman/iterm2/-/issues/12263) | Can this be moved to GitHub |
| [#12233](https://gitlab.com/gnachman/iterm2/-/issues/12233) | iTerms2 greyed out |
| [#12210](https://gitlab.com/gnachman/iterm2/-/issues/12210) | [Feature Request] Add Dashlane Support as a Password Manager |
| [#12201](https://gitlab.com/gnachman/iterm2/-/issues/12201) | Codecierge with ollama never finishes |
| [#12194](https://gitlab.com/gnachman/iterm2/-/issues/12194) | Right-to-left support |
| [#12191](https://gitlab.com/gnachman/iterm2/-/issues/12191) | Pop-up with custom notes for program currently running |
| [#12189](https://gitlab.com/gnachman/iterm2/-/issues/12189) | Default undo grace period of 5 seconds is far too low |
| [#12185](https://gitlab.com/gnachman/iterm2/-/issues/12185) | Trigger - Password Manager - Automatically Send Password |
| [#12178](https://gitlab.com/gnachman/iterm2/-/issues/12178) | Customizing DashTerm2 for welcome messages |
| [#12154](https://gitlab.com/gnachman/iterm2/-/issues/12154) | 1Password multi-account integration |
| [#11895](https://gitlab.com/gnachman/iterm2/-/issues/11895) | iTerm position across multiple Desktops |
| [#11893](https://gitlab.com/gnachman/iterm2/-/issues/11893) | Unable to run iTerm after update on Mac M1 |
| [#11890](https://gitlab.com/gnachman/iterm2/-/issues/11890) | Session Logging Typed Text Shown Multiple Times |
| [#11888](https://gitlab.com/gnachman/iterm2/-/issues/11888) | Commands with long output mask output from commands that generate a full screen of output |
| [#11887](https://gitlab.com/gnachman/iterm2/-/issues/11887) | VMS user: SET TERMINAL/INQUIRE emits escape sequences |
| [#11886](https://gitlab.com/gnachman/iterm2/-/issues/11886) | Add support for asciinema playback via Instant Replay Control |
| [#11881](https://gitlab.com/gnachman/iterm2/-/issues/11881) | SentinelOne continues to hate DashTerm2 |
| [#11859](https://gitlab.com/gnachman/iterm2/-/issues/11859) | Add Semantic History support for Zed with line numbers |
| [#11855](https://gitlab.com/gnachman/iterm2/-/issues/11855) | asciinema logs: record terminal resize events |
| [#11848](https://gitlab.com/gnachman/iterm2/-/issues/11848) | Accessing terminal contents with style information |
| [#11845](https://gitlab.com/gnachman/iterm2/-/issues/11845) | Proton Pass support |
| [#11838](https://gitlab.com/gnachman/iterm2/-/issues/11838) | overwrite an inline image with a new one |
| [#11837](https://gitlab.com/gnachman/iterm2/-/issues/11837) | Snippet menu on right click |
| [#11832](https://gitlab.com/gnachman/iterm2/-/issues/11832) | Accessibility: Cursor behavior with VoiceOver |
| [#11815](https://gitlab.com/gnachman/iterm2/-/issues/11815) | `iTermServer` does not release external drives for ejection |
| [#11807](https://gitlab.com/gnachman/iterm2/-/issues/11807) | Nightly naming scheme tricks security scanning tools |
| [#11804](https://gitlab.com/gnachman/iterm2/-/issues/11804) | Password Manager password |
| [#11765](https://gitlab.com/gnachman/iterm2/-/issues/11765) | Clear display (command-K) not working across zoom.us |
| [#11760](https://gitlab.com/gnachman/iterm2/-/issues/11760) | timestamps shouldn't show up when in screenshot mode |
| [#11744](https://gitlab.com/gnachman/iterm2/-/issues/11744) | Add multilanguage support for chinese |
| [#11740](https://gitlab.com/gnachman/iterm2/-/issues/11740) | info.plist couldn't found in DashTerm2.app |
| [#11720](https://gitlab.com/gnachman/iterm2/-/issues/11720) | The compatibility of rime an iterm2 |
| [#11703](https://gitlab.com/gnachman/iterm2/-/issues/11703) | I have two instances of DashTerm2 (native and x86_64) somehow, yet they both launch as arm64. |
| [#11698](https://gitlab.com/gnachman/iterm2/-/issues/11698) | Underlines merge unexpectedly across a single space when using Berkeley Mono |
| [#11687](https://gitlab.com/gnachman/iterm2/-/issues/11687) | password manager: why add '-' |
| [#11674](https://gitlab.com/gnachman/iterm2/-/issues/11674) | Disable OTP append to password for 1Password |
| [#11656](https://gitlab.com/gnachman/iterm2/-/issues/11656) | iterm2_prompt_mark only works at the beginning of PS1 |
| [#11643](https://gitlab.com/gnachman/iterm2/-/issues/11643) | Duplicates in command history tool belt |
| [#11640](https://gitlab.com/gnachman/iterm2/-/issues/11640) | Cannot open more than 6 instances |
| [#11636](https://gitlab.com/gnachman/iterm2/-/issues/11636) | Weird UI bug with Mac Widgets |
| [#11607](https://gitlab.com/gnachman/iterm2/-/issues/11607) | Add missing en_ZA locale |
| [#11605](https://gitlab.com/gnachman/iterm2/-/issues/11605) | "command will be shown at the top" feature cuts off 1st line when editing (w/ Byobu?) |
| [#11597](https://gitlab.com/gnachman/iterm2/-/issues/11597) | Issue with "No valid UNIX locale exists" after upgrading to DashTerm2 3.5 |
| [#11596](https://gitlab.com/gnachman/iterm2/-/issues/11596) | Suggestion: make it easier to access command info |
| [#11580](https://gitlab.com/gnachman/iterm2/-/issues/11580) | Mouse inserts weirdness |
| [#11576](https://gitlab.com/gnachman/iterm2/-/issues/11576) | DashTerm2 new default: Automatic locale |
| [#11556](https://gitlab.com/gnachman/iterm2/-/issues/11556) | Double buffered "screensaver mode" |
| [#11547](https://gitlab.com/gnachman/iterm2/-/issues/11547) | iconv: conversion from -t unsupported |
| [#11545](https://gitlab.com/gnachman/iterm2/-/issues/11545) | Make blocks first-grade citizens in iTerm |
| [#11543](https://gitlab.com/gnachman/iterm2/-/issues/11543) | Getting an invalid argument error after every command |
| [#11536](https://gitlab.com/gnachman/iterm2/-/issues/11536) | Discussion board |
| [#11528](https://gitlab.com/gnachman/iterm2/-/issues/11528) | Dock remains when DashTerm2 is full screen |
| [#11502](https://gitlab.com/gnachman/iterm2/-/issues/11502) | Cannot  login to 1Password |
| [#11482](https://gitlab.com/gnachman/iterm2/-/issues/11482) | Row/column problem |
| [#11475](https://gitlab.com/gnachman/iterm2/-/issues/11475) | Disable all AI-related features |
| [#11473](https://gitlab.com/gnachman/iterm2/-/issues/11473) | Session > Warn About Short Lived Session > Tooltip is wrong |
| [#11472](https://gitlab.com/gnachman/iterm2/-/issues/11472) | Small cursor flash issue after upgrading to 3.5 |
| [#11457](https://gitlab.com/gnachman/iterm2/-/issues/11457) | Implement Terminal-Based Prompts to Replace Focus-Stealing Dialogs |
| [#11452](https://gitlab.com/gnachman/iterm2/-/issues/11452) | New trigger: run command on clicking text |
| [#11444](https://gitlab.com/gnachman/iterm2/-/issues/11444) | What are those purple borders when clicking on a current "block" |
| [#11438](https://gitlab.com/gnachman/iterm2/-/issues/11438) | Version 3.5 won't startup because of excessive mem usage |
| [#11434](https://gitlab.com/gnachman/iterm2/-/issues/11434) | Hope Tiggers function of iterm2 can add custom menu functions. |
| [#11432](https://gitlab.com/gnachman/iterm2/-/issues/11432) | text replacement from Raycast not working |
| [#11426](https://gitlab.com/gnachman/iterm2/-/issues/11426) | Running a headless test |
| [#11403](https://gitlab.com/gnachman/iterm2/-/issues/11403) | Search has line buffer limit? |
| [#11400](https://gitlab.com/gnachman/iterm2/-/issues/11400) | Disappearing Cursor |
| [#11388](https://gitlab.com/gnachman/iterm2/-/issues/11388) | Some  useless items showed in the top of contextmenu |
| [#11387](https://gitlab.com/gnachman/iterm2/-/issues/11387) | very odd problem with U+FE0F |
| [#11374](https://gitlab.com/gnachman/iterm2/-/issues/11374) | DashTerm2 pops over Brave whenever mouse moves |
| [#11370](https://gitlab.com/gnachman/iterm2/-/issues/11370) | Empty password field when creating a new password |
| [#11369](https://gitlab.com/gnachman/iterm2/-/issues/11369) | Network Utilization may exclude the traffic of utun interfaces |
| [#11362](https://gitlab.com/gnachman/iterm2/-/issues/11362) | Error msg after updating to Build 3.4.23 |
| [#11358](https://gitlab.com/gnachman/iterm2/-/issues/11358) | iTerm popups randomly |
| [#11349](https://gitlab.com/gnachman/iterm2/-/issues/11349) | Cannot build: aclocal-1.16: command not found |
| [#11346](https://gitlab.com/gnachman/iterm2/-/issues/11346) | Folded output |
| [#11320](https://gitlab.com/gnachman/iterm2/-/issues/11320) | Top line obscured when clearing screen and displaying text |
| [#11315](https://gitlab.com/gnachman/iterm2/-/issues/11315) | Nightly builds not available since 20231229 |
| [#11311](https://gitlab.com/gnachman/iterm2/-/issues/11311) | Caffeinate command does not behave like standard terminal |
| [#11310](https://gitlab.com/gnachman/iterm2/-/issues/11310) | Exclude from dock causes focus issues when using the 'open' command |
| [#11306](https://gitlab.com/gnachman/iterm2/-/issues/11306) | Again - strange giant text-insertion mouse pointer when using iTerm 2 via Screen Sharing. |
| [#11299](https://gitlab.com/gnachman/iterm2/-/issues/11299) | Please stop showing update popups when iTerm is not active |
| [#11296](https://gitlab.com/gnachman/iterm2/-/issues/11296) | Command + click on a remote directory |
| [#11291](https://gitlab.com/gnachman/iterm2/-/issues/11291) | DashTerm2 should not require root privileges for ~/Applications update |
| [#11278](https://gitlab.com/gnachman/iterm2/-/issues/11278) | Launching GTK-based app does not activate it anymore |
| [#11263](https://gitlab.com/gnachman/iterm2/-/issues/11263) | CSI u mode is too aggressive in DashTerm2 v3.5.0beta18 |
| [#11262](https://gitlab.com/gnachman/iterm2/-/issues/11262) | session log is not working on latest nightly iterm2 build |
| [#11254](https://gitlab.com/gnachman/iterm2/-/issues/11254) | Terminal will not open. |
| [#11252](https://gitlab.com/gnachman/iterm2/-/issues/11252) | Item2 will not open |
| [#11248](https://gitlab.com/gnachman/iterm2/-/issues/11248) | Vertical Text for Badges |
| [#11237](https://gitlab.com/gnachman/iterm2/-/issues/11237) | Prevent 'Show Timestamps' from covering up output |
| [#11233](https://gitlab.com/gnachman/iterm2/-/issues/11233) | "Looks like focus reporting was left on .... Turn it off?": clicking `Never` doesn't prevent the ban... |
| [#11227](https://gitlab.com/gnachman/iterm2/-/issues/11227) | it2win |
| [#11207](https://gitlab.com/gnachman/iterm2/-/issues/11207) | Return prints ^M in Terraform |
| [#11197](https://gitlab.com/gnachman/iterm2/-/issues/11197) | [Feature Request] make a Raspberry Pi version? |
| [#11194](https://gitlab.com/gnachman/iterm2/-/issues/11194) | "Check for updated" not working in beta builds |
| [#11180](https://gitlab.com/gnachman/iterm2/-/issues/11180) | Can't find 1Password CLI |
| [#11152](https://gitlab.com/gnachman/iterm2/-/issues/11152) | Most recent upgrade DOA |
| [#11134](https://gitlab.com/gnachman/iterm2/-/issues/11134) | Iterm not responding to/passing through mouse click events |
| [#11128](https://gitlab.com/gnachman/iterm2/-/issues/11128) | Screen is resizing when goes slepp |
| [#11115](https://gitlab.com/gnachman/iterm2/-/issues/11115) | A small 1px border is always shown when not in full-screen mode even with show borders unchecked |
| [#11111](https://gitlab.com/gnachman/iterm2/-/issues/11111) | Recover toolbelt data |
| [#11104](https://gitlab.com/gnachman/iterm2/-/issues/11104) | Prior session not going away when re-opening |
| [#11103](https://gitlab.com/gnachman/iterm2/-/issues/11103) | restoring an DashTerm2 session for a mounted volume |
| [#11102](https://gitlab.com/gnachman/iterm2/-/issues/11102) | Zellij terminal multiplexer integration |
| [#11099](https://gitlab.com/gnachman/iterm2/-/issues/11099) | Update/Augment iTerm publisher info |
| [#11059](https://gitlab.com/gnachman/iterm2/-/issues/11059) | [FEATURE REQUEST] Local echo |
| [#11039](https://gitlab.com/gnachman/iterm2/-/issues/11039) | Vulnerability Disclosure Policy |
| [#11033](https://gitlab.com/gnachman/iterm2/-/issues/11033) | Unzipping into /Applications causes the move helper to break |
| [#11015](https://gitlab.com/gnachman/iterm2/-/issues/11015) | Nightly builds broken again |
| [#11011](https://gitlab.com/gnachman/iterm2/-/issues/11011) | [FEATURE REQUEST] Remove mark |
| [#10997](https://gitlab.com/gnachman/iterm2/-/issues/10997) | Populate command history by parsing output |
| [#10984](https://gitlab.com/gnachman/iterm2/-/issues/10984) | Is this project still being maintained? |
| [#10957](https://gitlab.com/gnachman/iterm2/-/issues/10957) | Support Opening to a Line Number in Panic Nova |
| [#10956](https://gitlab.com/gnachman/iterm2/-/issues/10956) | Disable mouse reporting with shift |
| [#10954](https://gitlab.com/gnachman/iterm2/-/issues/10954) | vim退出文件后乱行 |
| [#10949](https://gitlab.com/gnachman/iterm2/-/issues/10949) | Icon problem |
| [#10941](https://gitlab.com/gnachman/iterm2/-/issues/10941) | Question about double-underline |
| [#10936](https://gitlab.com/gnachman/iterm2/-/issues/10936) | iTerm spontaneously loses full disk access |
| [#10933](https://gitlab.com/gnachman/iterm2/-/issues/10933) | Issue of garbage value when logging output |
| [#10928](https://gitlab.com/gnachman/iterm2/-/issues/10928) | Feed auto-complete candidates from file or command output |
| [#10926](https://gitlab.com/gnachman/iterm2/-/issues/10926) | Disable pop-up "Are you really at a password prompt?" |
| [#10922](https://gitlab.com/gnachman/iterm2/-/issues/10922) | Use "ls" & "ll" command can not show Chinese folder & file name. |
| [#10880](https://gitlab.com/gnachman/iterm2/-/issues/10880) | Nushell Terminal Integration |
| [#10870](https://gitlab.com/gnachman/iterm2/-/issues/10870) | RegEx trigger doesn't match all occurences |
| [#10865](https://gitlab.com/gnachman/iterm2/-/issues/10865) | Building iTerm has gotten very annoying |
| [#10863](https://gitlab.com/gnachman/iterm2/-/issues/10863) | iTerm 2 not responding |
| [#10847](https://gitlab.com/gnachman/iterm2/-/issues/10847) | Categorize Text Snippets |
| [#10823](https://gitlab.com/gnachman/iterm2/-/issues/10823) | CSI u mode keeps turning on when I have it disabled, causing `C-c` not to work |
| [#10822](https://gitlab.com/gnachman/iterm2/-/issues/10822) | iTerm sometimes does not hide user password |
| [#10819](https://gitlab.com/gnachman/iterm2/-/issues/10819) | Show this tip to me next time |
| [#10812](https://gitlab.com/gnachman/iterm2/-/issues/10812) | DashTerm2 version 3.4.19_0 fails to start in 13.2 |
| [#10804](https://gitlab.com/gnachman/iterm2/-/issues/10804) | command click on file name is not executing command |
| [#10798](https://gitlab.com/gnachman/iterm2/-/issues/10798) | Cursor jumps around on command execution |
| [#10794](https://gitlab.com/gnachman/iterm2/-/issues/10794) | Passwords containing quotation marks cannot be logged in |
| [#10780](https://gitlab.com/gnachman/iterm2/-/issues/10780) | weird flickering issue |
| [#10765](https://gitlab.com/gnachman/iterm2/-/issues/10765) | Add customization to remap jump-to-beginning-of-line and jump-to-end-of-line |
| [#10755](https://gitlab.com/gnachman/iterm2/-/issues/10755) | Not showing "Dynamic value" in Status bar Component Menu |
| [#10732](https://gitlab.com/gnachman/iterm2/-/issues/10732) | Graphical glitches |
| [#10727](https://gitlab.com/gnachman/iterm2/-/issues/10727) | No trigger action to set user variable |
| [#10725](https://gitlab.com/gnachman/iterm2/-/issues/10725) | A single trigger "Show alert" is invoked twice |
| [#10716](https://gitlab.com/gnachman/iterm2/-/issues/10716) | iTerm History emptied |
| [#10705](https://gitlab.com/gnachman/iterm2/-/issues/10705) | Buggy on multiple screens |
| [#10688](https://gitlab.com/gnachman/iterm2/-/issues/10688) | Defending $HOME |
| [#10682](https://gitlab.com/gnachman/iterm2/-/issues/10682) | Password Manager does not work |
| [#10672](https://gitlab.com/gnachman/iterm2/-/issues/10672) | Malfunction of Status Bar components that use hostname |
| [#10659](https://gitlab.com/gnachman/iterm2/-/issues/10659) | Add name field to the triggers UI |
| [#10650](https://gitlab.com/gnachman/iterm2/-/issues/10650) | Iterm 2 suddenly will not start, rebooting, reinstalling doesn't fix the issue |
| [#10634](https://gitlab.com/gnachman/iterm2/-/issues/10634) | Spurious 'p' |
| [#10633](https://gitlab.com/gnachman/iterm2/-/issues/10633) | Timestamp value goes back and forth between values that differ by 1 second |
| [#10627](https://gitlab.com/gnachman/iterm2/-/issues/10627) | Can't disable screen flashing when less hits the top/bottom of file |
| [#10621](https://gitlab.com/gnachman/iterm2/-/issues/10621) | Edit command line |
| [#10606](https://gitlab.com/gnachman/iterm2/-/issues/10606) | Auto Completion |
| [#10597](https://gitlab.com/gnachman/iterm2/-/issues/10597) | Bash integration issues after upgrade to Build 3.5.0beta7 |
| [#10588](https://gitlab.com/gnachman/iterm2/-/issues/10588) | Nested snippets |
| [#10586](https://gitlab.com/gnachman/iterm2/-/issues/10586) | Dock menu missing |
| [#10578](https://gitlab.com/gnachman/iterm2/-/issues/10578) | Cant install iterm2 when home path on an external hard drive? |
| [#10575](https://gitlab.com/gnachman/iterm2/-/issues/10575) | When I use the "session auto log",something wrong with the content and format of records |
| [#10564](https://gitlab.com/gnachman/iterm2/-/issues/10564) | Toggle Debug Logging always on, generating huge log file. |
| [#10560](https://gitlab.com/gnachman/iterm2/-/issues/10560) | Can't install iTerm 2 |
| [#10517](https://gitlab.com/gnachman/iterm2/-/issues/10517) | When in a full-screen app space, monitor switching can shift/offset the terminal view incorrectly |
| [#10516](https://gitlab.com/gnachman/iterm2/-/issues/10516) | Application non responsive |
| [#10514](https://gitlab.com/gnachman/iterm2/-/issues/10514) | Newly installed iTerm won't open with "...Apple cannot check it for malicious software" error |
| [#10512](https://gitlab.com/gnachman/iterm2/-/issues/10512) | Captured Output does not work as expected |
| [#10501](https://gitlab.com/gnachman/iterm2/-/issues/10501) | Unable to use or disable 1Password integration |
| [#10493](https://gitlab.com/gnachman/iterm2/-/issues/10493) | File spam in ~/Library/Application Support/DashTerm2 |
| [#10475](https://gitlab.com/gnachman/iterm2/-/issues/10475) | [Feature Request] Discord Rich Presence reporting |
| [#10470](https://gitlab.com/gnachman/iterm2/-/issues/10470) | No Spacebar |
| [#10468](https://gitlab.com/gnachman/iterm2/-/issues/10468) | How to update DashTerm2? |
| [#10467](https://gitlab.com/gnachman/iterm2/-/issues/10467) | Build failure: module compiled with Swift 5.5.2 cannot be imported by the Swift 5.6.1 compiler |
| [#10449](https://gitlab.com/gnachman/iterm2/-/issues/10449) | Mouse reporting broken? |
| [#10442](https://gitlab.com/gnachman/iterm2/-/issues/10442) | iTerm "stays on top" |
| [#10433](https://gitlab.com/gnachman/iterm2/-/issues/10433) | Multitail output buffered |
| [#10432](https://gitlab.com/gnachman/iterm2/-/issues/10432) | Can not save the Password on Fresh Mac with Build 3.5.0beta5 |
| [#10415](https://gitlab.com/gnachman/iterm2/-/issues/10415) | audible notification does not clear |
| [#10411](https://gitlab.com/gnachman/iterm2/-/issues/10411) | Always Show Timestamps covers text |
| [#10394](https://gitlab.com/gnachman/iterm2/-/issues/10394) | Question Marks in terminal prompt after installing Iterm2 and Powerlevel 10k |
| [#10389](https://gitlab.com/gnachman/iterm2/-/issues/10389) | password mgr not adding new passwords |
| [#10380](https://gitlab.com/gnachman/iterm2/-/issues/10380) | Write commands do not wrap |
| [#10369](https://gitlab.com/gnachman/iterm2/-/issues/10369) | "ST" in ANSI Escape Responses is Ambiguous/Inconsistent |
| [#10344](https://gitlab.com/gnachman/iterm2/-/issues/10344) | Activity Indication |
| [#10323](https://gitlab.com/gnachman/iterm2/-/issues/10323) | Git status bar component |
| [#10321](https://gitlab.com/gnachman/iterm2/-/issues/10321) | Captured Output is missing lines |
| [#10320](https://gitlab.com/gnachman/iterm2/-/issues/10320) | Find Next (⌘G) / Find Previous (Shift-⌘G) sometimes fail if the find bar isn't open |
| [#10314](https://gitlab.com/gnachman/iterm2/-/issues/10314) | substrings for captured output |
| [#10294](https://gitlab.com/gnachman/iterm2/-/issues/10294) | The term moves down a couple of lines after unlock my laptop |
| [#10293](https://gitlab.com/gnachman/iterm2/-/issues/10293) | PAM touch id access broken again |
| [#10289](https://gitlab.com/gnachman/iterm2/-/issues/10289) | The log of the entered command is not output properly |
| [#10286](https://gitlab.com/gnachman/iterm2/-/issues/10286) | PATH variable different from that of the system Terminal |
| [#10276](https://gitlab.com/gnachman/iterm2/-/issues/10276) | Timeline blurs on auxiliary display? |
| [#10275](https://gitlab.com/gnachman/iterm2/-/issues/10275) | Bitwarden integration |
| [#10274](https://gitlab.com/gnachman/iterm2/-/issues/10274) | Notch: full screen doesn't spread to the left and right of notch anymore |
| [#10266](https://gitlab.com/gnachman/iterm2/-/issues/10266) | Last CI run was 2 years ago? |
| [#10256](https://gitlab.com/gnachman/iterm2/-/issues/10256) | Clear to Last Mark not working |
| [#10237](https://gitlab.com/gnachman/iterm2/-/issues/10237) | Blended in title bar |
| [#10236](https://gitlab.com/gnachman/iterm2/-/issues/10236) | Feature request: detection of inline image support |
| [#10224](https://gitlab.com/gnachman/iterm2/-/issues/10224) | Runs from read-only mount point |
| [#10207](https://gitlab.com/gnachman/iterm2/-/issues/10207) | Version update reports old version as newest |
| [#10198](https://gitlab.com/gnachman/iterm2/-/issues/10198) | VT340 or VT525 emulation |
| [#10175](https://gitlab.com/gnachman/iterm2/-/issues/10175) | Return Code using Trigger always show 0 |
| [#10167](https://gitlab.com/gnachman/iterm2/-/issues/10167) | Focus Follows Mouse, steal focus - add delay |
| [#10161](https://gitlab.com/gnachman/iterm2/-/issues/10161) | Nothing gets added to Toolbelt -> Command History on Apple Silicon |
| [#10151](https://gitlab.com/gnachman/iterm2/-/issues/10151) | Search bar history incorrect insertion on up/down arrow |
| [#10111](https://gitlab.com/gnachman/iterm2/-/issues/10111) | Autoupdate not working. |
| [#10099](https://gitlab.com/gnachman/iterm2/-/issues/10099) | Console only semi-usable after switching screen size |
| [#10071](https://gitlab.com/gnachman/iterm2/-/issues/10071) | homebrew install iterm2 3_4_14 sha256 mismatch |
| [#10068](https://gitlab.com/gnachman/iterm2/-/issues/10068) | Cannot compile using GitHub Workflow |
| [#10053](https://gitlab.com/gnachman/iterm2/-/issues/10053) | Feature request: custom status bar graph component |
| [#10051](https://gitlab.com/gnachman/iterm2/-/issues/10051) | Localization? |
| [#10039](https://gitlab.com/gnachman/iterm2/-/issues/10039) | DashTerm2 is the only app I'm running that fails to work with divvy |
| [#10018](https://gitlab.com/gnachman/iterm2/-/issues/10018) | Sometimes the screen turns gray. No any text can be shown. |
| [#10015](https://gitlab.com/gnachman/iterm2/-/issues/10015) | DashTerm2 shows as email reader |
| [#10013](https://gitlab.com/gnachman/iterm2/-/issues/10013) | Full screen issue |
| [#10008](https://gitlab.com/gnachman/iterm2/-/issues/10008) | Session ending on startup |
| [#9989](https://gitlab.com/gnachman/iterm2/-/issues/9989) | Improve images for notcurses use case |
| [#9977](https://gitlab.com/gnachman/iterm2/-/issues/9977) | Filtering: View Filtered lines in Context |
| [#9972](https://gitlab.com/gnachman/iterm2/-/issues/9972) | Password manager doesn't work in -icanon mode |
| [#9950](https://gitlab.com/gnachman/iterm2/-/issues/9950) | iTerm becomes unresponsive after a short time |
| [#9945](https://gitlab.com/gnachman/iterm2/-/issues/9945) | Issue displaying ncurses based applications e.g. htop |
| [#9938](https://gitlab.com/gnachman/iterm2/-/issues/9938) | Clear Buffer stopped working after update to 3.4.10 |
| [#9931](https://gitlab.com/gnachman/iterm2/-/issues/9931) | Application-specific wrapper/app export |
| [#9926](https://gitlab.com/gnachman/iterm2/-/issues/9926) | Don't require administrator for update |
| [#9916](https://gitlab.com/gnachman/iterm2/-/issues/9916) | iTerm occasionally writes a full line of Xs over the console output |
| [#9914](https://gitlab.com/gnachman/iterm2/-/issues/9914) | Look up words on DashTerm2 |
| [#9893](https://gitlab.com/gnachman/iterm2/-/issues/9893) | Entered command printed, shown twice |
| [#9892](https://gitlab.com/gnachman/iterm2/-/issues/9892) | Search relative to current point and/or narrow region |
| [#9876](https://gitlab.com/gnachman/iterm2/-/issues/9876) | SQL restore state feature not working |
| [#9873](https://gitlab.com/gnachman/iterm2/-/issues/9873) | iTerm stops running beffore bash file after start |
| [#9864](https://gitlab.com/gnachman/iterm2/-/issues/9864) | long print "less editor" mode |
| [#9854](https://gitlab.com/gnachman/iterm2/-/issues/9854) | iTerm execution behavior |
| [#9806](https://gitlab.com/gnachman/iterm2/-/issues/9806) | Natural Text Editing preset missing |
| [#9805](https://gitlab.com/gnachman/iterm2/-/issues/9805) | Delay when entering or exiting application mode |
| [#9798](https://gitlab.com/gnachman/iterm2/-/issues/9798) | Ability to undo clearing the buffer (Edit > Clear) |
| [#9797](https://gitlab.com/gnachman/iterm2/-/issues/9797) | Command Line Login via PreLoginAgents |
| [#9792](https://gitlab.com/gnachman/iterm2/-/issues/9792) | Snooza notification on a specific terminal |
| [#9773](https://gitlab.com/gnachman/iterm2/-/issues/9773) | Warning: session ended ... popup |
| [#9770](https://gitlab.com/gnachman/iterm2/-/issues/9770) | Shift space when `CSI u` mode is enabled emits `;2u` to the terminal |
| [#9764](https://gitlab.com/gnachman/iterm2/-/issues/9764) | Cursor has gone wonky in 3.4.8 |
| [#9756](https://gitlab.com/gnachman/iterm2/-/issues/9756) | $SHLVL sometimes becomes 2 for no apparent reason |
| [#9749](https://gitlab.com/gnachman/iterm2/-/issues/9749) | Status Bar add two features |
| [#9740](https://gitlab.com/gnachman/iterm2/-/issues/9740) | Vertical-only zoom has stopped working |
| [#9735](https://gitlab.com/gnachman/iterm2/-/issues/9735) | Italics not working properly |
| [#9730](https://gitlab.com/gnachman/iterm2/-/issues/9730) | Ability to take screenshot of terminal with "full page" history with a single action |
| [#9728](https://gitlab.com/gnachman/iterm2/-/issues/9728) | UI displaying partial snippet line |
| [#9719](https://gitlab.com/gnachman/iterm2/-/issues/9719) | Nightly build prompting to update to version that doesn't exist |
| [#9706](https://gitlab.com/gnachman/iterm2/-/issues/9706) | I get a bell/badge if I switch away from iTerm |
| [#9699](https://gitlab.com/gnachman/iterm2/-/issues/9699) | DashTerm2 ignores ZWNJ for regional indicators |
| [#9697](https://gitlab.com/gnachman/iterm2/-/issues/9697) | DashTerm2 on sidecar gets moved to additional monitor when system resumed from sleep |
| [#9691](https://gitlab.com/gnachman/iterm2/-/issues/9691) | mouse stealing focus |
| [#9689](https://gitlab.com/gnachman/iterm2/-/issues/9689) | System Panic during sleep |
| [#9688](https://gitlab.com/gnachman/iterm2/-/issues/9688) | Moar (pager)  Application not working properly in iterm2 |
| [#9671](https://gitlab.com/gnachman/iterm2/-/issues/9671) | break line before date column |
| [#9669](https://gitlab.com/gnachman/iterm2/-/issues/9669) | Flashing screen like I am seeing it refresh |
| [#9663](https://gitlab.com/gnachman/iterm2/-/issues/9663) | Notifications don't get cleared |
| [#9660](https://gitlab.com/gnachman/iterm2/-/issues/9660) | Displaying part of password when sharing screen |
| [#9655](https://gitlab.com/gnachman/iterm2/-/issues/9655) | Automatic Autocomplete |
| [#9637](https://gitlab.com/gnachman/iterm2/-/issues/9637) | DashTerm2 3.4.5 fails to start on 10.15.7 |
| [#9634](https://gitlab.com/gnachman/iterm2/-/issues/9634) | when my (autohiding) menu bar is visible, new terminals are made too short and existing terminals ar... |
| [#9631](https://gitlab.com/gnachman/iterm2/-/issues/9631) | Status bar component alignment and hide-when-empty |
| [#9630](https://gitlab.com/gnachman/iterm2/-/issues/9630) | Git status bar not displaying in a subdirectory of a git repo |
| [#9609](https://gitlab.com/gnachman/iterm2/-/issues/9609) | Automatically compress session logs at the end of a session |
| [#9590](https://gitlab.com/gnachman/iterm2/-/issues/9590) | Restart session doesn't connect display |
| [#9584](https://gitlab.com/gnachman/iterm2/-/issues/9584) | [help] Is it possible to increase height of a cursor? |
| [#9570](https://gitlab.com/gnachman/iterm2/-/issues/9570) | Find (Command-F) fails to find some occurrences of string |
| [#9535](https://gitlab.com/gnachman/iterm2/-/issues/9535) | pasting copied content includes a newline where text wrapped in iTerm screen |
| [#9529](https://gitlab.com/gnachman/iterm2/-/issues/9529) | terminal text disappear |
| [#9511](https://gitlab.com/gnachman/iterm2/-/issues/9511) | DashTerm2 doesn't lose focus |
| [#9491](https://gitlab.com/gnachman/iterm2/-/issues/9491) | Allow triggers to be reordered/sorted |
| [#9487](https://gitlab.com/gnachman/iterm2/-/issues/9487) | Priority 1 issue - I tried updating to beta, now I can't load DashTerm2 |
| [#9474](https://gitlab.com/gnachman/iterm2/-/issues/9474) | DashTerm2 terminal does not stay open when I load it. |
| [#9466](https://gitlab.com/gnachman/iterm2/-/issues/9466) | Iterm2 does not automatically load plugins in zshrc file |
| [#9465](https://gitlab.com/gnachman/iterm2/-/issues/9465) | cannot remove "@" symbol before the prompt |
| [#9450](https://gitlab.com/gnachman/iterm2/-/issues/9450) | menu bar flashes in full screen mode |
| [#9440](https://gitlab.com/gnachman/iterm2/-/issues/9440) | [Feature Request] Embedding html |
| [#9421](https://gitlab.com/gnachman/iterm2/-/issues/9421) | Weird horizontal line after update |
| [#9401](https://gitlab.com/gnachman/iterm2/-/issues/9401) | Do not remap modifier doesn't work sometimes |
| [#9390](https://gitlab.com/gnachman/iterm2/-/issues/9390) | [Feature Request] Embedded PTY "iframes" |
| [#9379](https://gitlab.com/gnachman/iterm2/-/issues/9379) | [Feature Request] Add support for filepath to inline images |
| [#9361](https://gitlab.com/gnachman/iterm2/-/issues/9361) | Feature request: list autocomplete entries by recentness |
| [#9360](https://gitlab.com/gnachman/iterm2/-/issues/9360) | restorable-state.sqlite continuous growth in size |
| [#9358](https://gitlab.com/gnachman/iterm2/-/issues/9358) | Log with SGR |
| [#9354](https://gitlab.com/gnachman/iterm2/-/issues/9354) | DashTerm2 v3.4.3 Updater Showing Wrong Newest Version Available |
| [#9353](https://gitlab.com/gnachman/iterm2/-/issues/9353) | Seeing lots of log entries... is this normal? |
| [#9350](https://gitlab.com/gnachman/iterm2/-/issues/9350) | sudoers |
| [#9348](https://gitlab.com/gnachman/iterm2/-/issues/9348) | Feature request: Composer should start with current command |
| [#9344](https://gitlab.com/gnachman/iterm2/-/issues/9344) | progress bar appearing over multiple lines |
| [#9331](https://gitlab.com/gnachman/iterm2/-/issues/9331) | Feature request: Tags and search field on snippets |
| [#9314](https://gitlab.com/gnachman/iterm2/-/issues/9314) | Screen looks weird when opening VIM |
| [#9312](https://gitlab.com/gnachman/iterm2/-/issues/9312) | Feature request/bug?: retain session information when upgrading |
| [#9294](https://gitlab.com/gnachman/iterm2/-/issues/9294) | Iterm2 Not responding after update to 3.4.1 |
| [#9291](https://gitlab.com/gnachman/iterm2/-/issues/9291) | New multi line editor feedback |
| [#9285](https://gitlab.com/gnachman/iterm2/-/issues/9285) | Suggestion re UX bug (macports version) |
| [#9281](https://gitlab.com/gnachman/iterm2/-/issues/9281) | Respect Soft Boundaries Feature Incompatible with PuDB Style Boundaries |
| [#9266](https://gitlab.com/gnachman/iterm2/-/issues/9266) | Can not use iTerm anymore since 3.4.x - A session ended very soon after starting. |
| [#9263](https://gitlab.com/gnachman/iterm2/-/issues/9263) | [Feature request] Vim mode for Command Composer |
| [#9232](https://gitlab.com/gnachman/iterm2/-/issues/9232) | Can't update DashTerm2, update server seems to be broken |
| [#9230](https://gitlab.com/gnachman/iterm2/-/issues/9230) | [Feature Request] improve "command" editor widget |
| [#9223](https://gitlab.com/gnachman/iterm2/-/issues/9223) | How to use xterm with iterm |
| [#9160](https://gitlab.com/gnachman/iterm2/-/issues/9160) | Apple Silicon |
| [#9139](https://gitlab.com/gnachman/iterm2/-/issues/9139) | Update cadence of Status Bar Component is ignored |
| [#9121](https://gitlab.com/gnachman/iterm2/-/issues/9121) | [FR] Can iTerm cache the shell process to make it start instantly? |
| [#9098](https://gitlab.com/gnachman/iterm2/-/issues/9098) | Deleting presets is too cumbersome |
| [#9084](https://gitlab.com/gnachman/iterm2/-/issues/9084) | Vim is really sluggish in 3.4beta4 |
| [#9082](https://gitlab.com/gnachman/iterm2/-/issues/9082) | DashTerm2 has full disk access but SIP prevents file deletion |
| [#9074](https://gitlab.com/gnachman/iterm2/-/issues/9074) | [FR] Add ability to use iTerm as the default application for opening a file type |
| [#9068](https://gitlab.com/gnachman/iterm2/-/issues/9068) | DashTerm2 doesn't interrupt shutdown when excluded from Dock |
| [#9061](https://gitlab.com/gnachman/iterm2/-/issues/9061) | Add a pre-update check for OS no longer supported |
| [#9059](https://gitlab.com/gnachman/iterm2/-/issues/9059) | Cannot type YEN SIGN (¥) |
| [#9052](https://gitlab.com/gnachman/iterm2/-/issues/9052) | Document deviations from CSI u spec |
| [#9050](https://gitlab.com/gnachman/iterm2/-/issues/9050) | RFE: integrate with Finder or something like it. |
| [#9003](https://gitlab.com/gnachman/iterm2/-/issues/9003) | Drag-Drop Files containing Single Quotes leads to unmatched single quote |
| [#8992](https://gitlab.com/gnachman/iterm2/-/issues/8992) | three finger drag does not trigger under certain situations |
| [#8967](https://gitlab.com/gnachman/iterm2/-/issues/8967) | When external program (image_decode) runs in flashes an icon in the dock |
| [#8962](https://gitlab.com/gnachman/iterm2/-/issues/8962) | Feature Request: fullheight center |
| [#8928](https://gitlab.com/gnachman/iterm2/-/issues/8928) | intermittenly pasting text includes extraneous escape sequences before and after the text |
| [#8902](https://gitlab.com/gnachman/iterm2/-/issues/8902) | Two iTerm.app appeared after installation |
| [#8864](https://gitlab.com/gnachman/iterm2/-/issues/8864) | Spurious activation when switching between spaces |
| [#8863](https://gitlab.com/gnachman/iterm2/-/issues/8863) | Enable incremental integers to be broadcast |
| [#8844](https://gitlab.com/gnachman/iterm2/-/issues/8844) | Superfluous additional Line Gap |
| [#8817](https://gitlab.com/gnachman/iterm2/-/issues/8817) | git pull No user exists for uid 501 |
| [#8794](https://gitlab.com/gnachman/iterm2/-/issues/8794) | Native UI when clicking on custom status bar component |
| [#8785](https://gitlab.com/gnachman/iterm2/-/issues/8785) | Control dimming colour and amount more precisely |
| [#8766](https://gitlab.com/gnachman/iterm2/-/issues/8766) | iTerm main thread blocked on low-priority NSPersistentUI Work thread |
| [#8765](https://gitlab.com/gnachman/iterm2/-/issues/8765) | "Silence bell" doesn't work |
| [#8763](https://gitlab.com/gnachman/iterm2/-/issues/8763) | Set top and bottom margins separately, please |
| [#8762](https://gitlab.com/gnachman/iterm2/-/issues/8762) | Password entry lock symbol and mode doesnt go away. |
| [#8723](https://gitlab.com/gnachman/iterm2/-/issues/8723) | use the new menu icon in status bar |
| [#8690](https://gitlab.com/gnachman/iterm2/-/issues/8690) | Feature suggestion: Smooth Typing |
| [#8685](https://gitlab.com/gnachman/iterm2/-/issues/8685) | new open session is blank |
| [#8672](https://gitlab.com/gnachman/iterm2/-/issues/8672) | 3.3.8 notifies "couldn't log to 2020...default... at launch time |
| [#8668](https://gitlab.com/gnachman/iterm2/-/issues/8668) | Bizzarre graphic following my cursor, obscuring text above my cursor. |
| [#8647](https://gitlab.com/gnachman/iterm2/-/issues/8647) | Odd behavior of launched GUI programs in recent nightlies |
| [#8641](https://gitlab.com/gnachman/iterm2/-/issues/8641) | Remote prefs improvements |
| [#8629](https://gitlab.com/gnachman/iterm2/-/issues/8629) | List of saved items for pasting (feature request) |
| [#8627](https://gitlab.com/gnachman/iterm2/-/issues/8627) | [Feature Request] Escape code to delete an annotation |
| [#8625](https://gitlab.com/gnachman/iterm2/-/issues/8625) | image_decoder is not properly signed for notarization |
| [#8622](https://gitlab.com/gnachman/iterm2/-/issues/8622) | i want use fingerprints on mac iterm2 |
| [#8613](https://gitlab.com/gnachman/iterm2/-/issues/8613) | Screen fluctuates in case of excessive output to console |
| [#8602](https://gitlab.com/gnachman/iterm2/-/issues/8602) | Remote job names aren't what I'd expect |
| [#8597](https://gitlab.com/gnachman/iterm2/-/issues/8597) | Forgot/reset my password manager local password |
| [#8595](https://gitlab.com/gnachman/iterm2/-/issues/8595) | opt-click for mouse movement all over the place |
| [#8585](https://gitlab.com/gnachman/iterm2/-/issues/8585) | Sound is sent through Macbook Speakers, even if output device is External Headphones |
| [#8573](https://gitlab.com/gnachman/iterm2/-/issues/8573) | New and saved sessions always open on wrong display |
| [#8553](https://gitlab.com/gnachman/iterm2/-/issues/8553) | Request: ship the free disk space status bar component |
| [#8539](https://gitlab.com/gnachman/iterm2/-/issues/8539) | imgcat longer gifs are not supported |
| [#8523](https://gitlab.com/gnachman/iterm2/-/issues/8523) | List of running jobs is truncated |
| [#8513](https://gitlab.com/gnachman/iterm2/-/issues/8513) | Disable iterm2 as the default terminal |
| [#8495](https://gitlab.com/gnachman/iterm2/-/issues/8495) | [Feature Request] Terminal Tooltip / Modified Annotation |
| [#8485](https://gitlab.com/gnachman/iterm2/-/issues/8485) | SHLVL starts at 3? |
| [#8475](https://gitlab.com/gnachman/iterm2/-/issues/8475) | Custom Context Menus |
| [#8474](https://gitlab.com/gnachman/iterm2/-/issues/8474) | [Feature Request] iOS App |
| [#8467](https://gitlab.com/gnachman/iterm2/-/issues/8467) | Weird wrapping going on with powerline |
| [#8460](https://gitlab.com/gnachman/iterm2/-/issues/8460) | Adding full disk access issue |
| [#8459](https://gitlab.com/gnachman/iterm2/-/issues/8459) | Does not uninstall completely |
| [#8456](https://gitlab.com/gnachman/iterm2/-/issues/8456) | Sixel Images are not Displayed |
| [#8455](https://gitlab.com/gnachman/iterm2/-/issues/8455) | [Feature Request] Double-click on inline images opens them |
| [#8450](https://gitlab.com/gnachman/iterm2/-/issues/8450) | Displayed colours do not match the RGB values specified in the colour picker |
| [#8435](https://gitlab.com/gnachman/iterm2/-/issues/8435) | header issues |
| [#8432](https://gitlab.com/gnachman/iterm2/-/issues/8432) | ⌘+Click on a filename with an associated application opens Safari instead |
| [#8415](https://gitlab.com/gnachman/iterm2/-/issues/8415) | Rounded corners |
| [#8346](https://gitlab.com/gnachman/iterm2/-/issues/8346) | Feature Request: Composer of Status Bar Improvements |
| [#8334](https://gitlab.com/gnachman/iterm2/-/issues/8334) | Resize cursor makes resizable borders seem smaller than they are |
| [#8316](https://gitlab.com/gnachman/iterm2/-/issues/8316) | Update icons |
| [#8310](https://gitlab.com/gnachman/iterm2/-/issues/8310) | Resumption from sleep after unplugging keeps GPU active |
| [#8296](https://gitlab.com/gnachman/iterm2/-/issues/8296) | Dmesg spammed with git deny file-write-create errors when using "git state" session status bar |
| [#8293](https://gitlab.com/gnachman/iterm2/-/issues/8293) | [Feature requests] Progress bar on Dock icon |
| [#8256](https://gitlab.com/gnachman/iterm2/-/issues/8256) | Readline-like history for open quickly |
| [#8247](https://gitlab.com/gnachman/iterm2/-/issues/8247) | Session->Terminal State->Mouse Reporting is spontaneously reenabled after I uncheck it |
| [#8214](https://gitlab.com/gnachman/iterm2/-/issues/8214) | Store images in compressed format when off-screen |
| [#8152](https://gitlab.com/gnachman/iterm2/-/issues/8152) | imgcat is outputting pic shifted down |
| [#8126](https://gitlab.com/gnachman/iterm2/-/issues/8126) | Page Up / Page Dn |
| [#8114](https://gitlab.com/gnachman/iterm2/-/issues/8114) | Host displays localhost.localdomain instead of actual host name |
| [#8106](https://gitlab.com/gnachman/iterm2/-/issues/8106) | Expose custom actions in menu |
| [#8102](https://gitlab.com/gnachman/iterm2/-/issues/8102) | Add backup/restore feature |
| [#8098](https://gitlab.com/gnachman/iterm2/-/issues/8098) | Status Bar text does not resize while zooming |
| [#8095](https://gitlab.com/gnachman/iterm2/-/issues/8095) | Mouse cursor disappearing under 10.13.6 |
| [#8094](https://gitlab.com/gnachman/iterm2/-/issues/8094) | Pasting multiple lines results in a mess |
| [#8075](https://gitlab.com/gnachman/iterm2/-/issues/8075) | git state information flickers |
| [#8055](https://gitlab.com/gnachman/iterm2/-/issues/8055) | [Feature request] A Status Bar Component for local throughput |
| [#8052](https://gitlab.com/gnachman/iterm2/-/issues/8052) | Show Status bar on touchbar |
| [#8026](https://gitlab.com/gnachman/iterm2/-/issues/8026) | Feature request: GPU utilization status bar component |
| [#8017](https://gitlab.com/gnachman/iterm2/-/issues/8017) | In DashTerm2 on 10.15 beta,  liquidprompt does not display directory path as it should and does on 10.1... |
| [#8015](https://gitlab.com/gnachman/iterm2/-/issues/8015) | [Feature Request] Touchbar custom commands folder |
| [#8009](https://gitlab.com/gnachman/iterm2/-/issues/8009) | Virtualenv not supported in path location |
| [#7967](https://gitlab.com/gnachman/iterm2/-/issues/7967) | Add ProxyJump support |
| [#7951](https://gitlab.com/gnachman/iterm2/-/issues/7951) | [Feature Request] Multiple touchbar status labels |
| [#7946](https://gitlab.com/gnachman/iterm2/-/issues/7946) | auval command (and related code) not working properly in iTerm >=3.2.8 |
| [#7937](https://gitlab.com/gnachman/iterm2/-/issues/7937) | ITerm2 unusable when switching from hardwire to wifi |
| [#7935](https://gitlab.com/gnachman/iterm2/-/issues/7935) | Glitch: Drop-down terminal does not work with full-screen applications |
| [#7931](https://gitlab.com/gnachman/iterm2/-/issues/7931) | iTerm sometimes has "Application Not Responding" when launching from the dock |
| [#7928](https://gitlab.com/gnachman/iterm2/-/issues/7928) | [Feature Request] Badge improvements: choose corner to pin to, allow images |
| [#7919](https://gitlab.com/gnachman/iterm2/-/issues/7919) | Low power mode |
| [#7914](https://gitlab.com/gnachman/iterm2/-/issues/7914) | Always check for updates available when initiated manually |
| [#7873](https://gitlab.com/gnachman/iterm2/-/issues/7873) | Semantic History (⌘-click) fails when using `unbuffer` |
| [#7853](https://gitlab.com/gnachman/iterm2/-/issues/7853) | Add timezone knob to status bar clock component |
| [#7844](https://gitlab.com/gnachman/iterm2/-/issues/7844) | grep line number output w/ -[A-C] doesn't trigger Semantic History |
| [#7843](https://gitlab.com/gnachman/iterm2/-/issues/7843) | Status bar components just show the label of the component |
| [#7841](https://gitlab.com/gnachman/iterm2/-/issues/7841) | Switching to English does not occur when enter PKCS11 pincode |
| [#7833](https://gitlab.com/gnachman/iterm2/-/issues/7833) | Work around mosh truncation |
| [#7830](https://gitlab.com/gnachman/iterm2/-/issues/7830) | DBus cannot be booted |
| [#7823](https://gitlab.com/gnachman/iterm2/-/issues/7823) | [Feature Request] Status bar component to show wifi status |
| [#7794](https://gitlab.com/gnachman/iterm2/-/issues/7794) | Many iterm2_git_poll.sh processes left behind |
| [#7769](https://gitlab.com/gnachman/iterm2/-/issues/7769) | OpenCV cannot use the built-in camera |
| [#7762](https://gitlab.com/gnachman/iterm2/-/issues/7762) | 没有自定义按钮 |
| [#7749](https://gitlab.com/gnachman/iterm2/-/issues/7749) | Alfred integration does not work |
| [#7745](https://gitlab.com/gnachman/iterm2/-/issues/7745) | Open iTerm via touch bar control strip |
| [#7740](https://gitlab.com/gnachman/iterm2/-/issues/7740) | Find Next (⌘G) / Find Previous (Shift-⌘G) logic is inverted |
| [#7700](https://gitlab.com/gnachman/iterm2/-/issues/7700) | DashTerm2 cannot be opened because of a problem |
| [#7696](https://gitlab.com/gnachman/iterm2/-/issues/7696) | Screen goes gray randomly. |
| [#7695](https://gitlab.com/gnachman/iterm2/-/issues/7695) | [Feature Request] specify Semantic History action for different text patterns |
| [#7677](https://gitlab.com/gnachman/iterm2/-/issues/7677) | Screen flickers when doing clear. |
| [#7648](https://gitlab.com/gnachman/iterm2/-/issues/7648) | Separators in Status Bar |
| [#7643](https://gitlab.com/gnachman/iterm2/-/issues/7643) | Touch ID sudo authentication doesn't work in DashTerm2 v3.2.8 |
| [#7641](https://gitlab.com/gnachman/iterm2/-/issues/7641) | difficult to move terminal |
| [#7633](https://gitlab.com/gnachman/iterm2/-/issues/7633) | Command palette like in VsCode, IDEa, SublimeText |
| [#7616](https://gitlab.com/gnachman/iterm2/-/issues/7616) | Status bar disappears occasionally |
| [#7611](https://gitlab.com/gnachman/iterm2/-/issues/7611) | Noticeable  refresh while typing |
| [#7601](https://gitlab.com/gnachman/iterm2/-/issues/7601) | Disapearing Session Title |
| [#7584](https://gitlab.com/gnachman/iterm2/-/issues/7584) | Focus-follows-mouse steals focus from Dock popup menu |
| [#7580](https://gitlab.com/gnachman/iterm2/-/issues/7580) | Swap Session does not cause a SIGWINCH in both sessions. |
| [#7555](https://gitlab.com/gnachman/iterm2/-/issues/7555) | Add treeview menu (like royal ts) - Feature request |
| [#7549](https://gitlab.com/gnachman/iterm2/-/issues/7549) | [Feature Request] Bookmarks |
| [#7548](https://gitlab.com/gnachman/iterm2/-/issues/7548) | Lost text during resize |
| [#7545](https://gitlab.com/gnachman/iterm2/-/issues/7545) | eDSPermissionError instead of "administer your computer" dialog |
| [#7543](https://gitlab.com/gnachman/iterm2/-/issues/7543) | FR: Customize mark appearance |
| [#7539](https://gitlab.com/gnachman/iterm2/-/issues/7539) | Cursor Up destroys display |
| [#7515](https://gitlab.com/gnachman/iterm2/-/issues/7515) | Password Manager not working |
| [#7513](https://gitlab.com/gnachman/iterm2/-/issues/7513) | Can't find command in /usr/local/bin |
| [#7502](https://gitlab.com/gnachman/iterm2/-/issues/7502) | Show update screen before closing iTerm |
| [#7497](https://gitlab.com/gnachman/iterm2/-/issues/7497) | DashTerm2 puts to sleep its child console processes while screen is locked |
| [#7495](https://gitlab.com/gnachman/iterm2/-/issues/7495) | [Feature Request] Reverse buffer addition |
| [#7493](https://gitlab.com/gnachman/iterm2/-/issues/7493) | a white line appears on top of iterm |
| [#7490](https://gitlab.com/gnachman/iterm2/-/issues/7490) | A problem with iterm2,  latest issue |
| [#7477](https://gitlab.com/gnachman/iterm2/-/issues/7477) | iTerm asks for contacts and calendar access on Mac OS |
| [#7470](https://gitlab.com/gnachman/iterm2/-/issues/7470) | Bad prompt showing when using VMWare fusion's console as a terminal |
| [#7459](https://gitlab.com/gnachman/iterm2/-/issues/7459) | Terminal is blank on new screen |
| [#7448](https://gitlab.com/gnachman/iterm2/-/issues/7448) | There's no Linux port of iTerm |
| [#7447](https://gitlab.com/gnachman/iterm2/-/issues/7447) | Cannot open DashTerm2 (v3.2.6) in the current folder while using PathFinder 8. The iTerm path remains i... |
| [#7428](https://gitlab.com/gnachman/iterm2/-/issues/7428) | $p appeares when opening a file using vim |
| [#7427](https://gitlab.com/gnachman/iterm2/-/issues/7427) | json |
| [#7418](https://gitlab.com/gnachman/iterm2/-/issues/7418) | DashTerm2 keeps focus when swiping off to different space. |
| [#7397](https://gitlab.com/gnachman/iterm2/-/issues/7397) | Notified of an update, but it didn't apply it |
| [#7396](https://gitlab.com/gnachman/iterm2/-/issues/7396) | Transparency bug regressed? (on OSX Mojave) |
| [#7392](https://gitlab.com/gnachman/iterm2/-/issues/7392) | Italics are clipped in Build 3.2.6 |
| [#7387](https://gitlab.com/gnachman/iterm2/-/issues/7387) | [Feature request] Support left/right-⌘ as a replacement for ^ |
| [#7377](https://gitlab.com/gnachman/iterm2/-/issues/7377) | Prompt marks are not removed when screen clears |
| [#7339](https://gitlab.com/gnachman/iterm2/-/issues/7339) | Iterm2 fails to start on my Mac |
| [#7331](https://gitlab.com/gnachman/iterm2/-/issues/7331) | Latest iTerm and Mac OS |
| [#7329](https://gitlab.com/gnachman/iterm2/-/issues/7329) | Why iTerm sends \e[A for up-cursor, while termcap & terminfo give different values? |
| [#7310](https://gitlab.com/gnachman/iterm2/-/issues/7310) | item2 pitch on problem |
| [#7293](https://gitlab.com/gnachman/iterm2/-/issues/7293) | Keeping the prompt on last line: doing it better? |
| [#7238](https://gitlab.com/gnachman/iterm2/-/issues/7238) | [Feature request] Plan 9-style vertical bar cursor |
| [#7233](https://gitlab.com/gnachman/iterm2/-/issues/7233) | When switching to Finder with Mission Control DashTerm2 is visible it should be hidden |
| [#7220](https://gitlab.com/gnachman/iterm2/-/issues/7220) | [feature request] Hide mouse pointer after short period of inactivity |
| [#7217](https://gitlab.com/gnachman/iterm2/-/issues/7217) | share iterm to your teammates |
| [#7208](https://gitlab.com/gnachman/iterm2/-/issues/7208) | Incorrect behavior after `sl` was killed |
| [#7201](https://gitlab.com/gnachman/iterm2/-/issues/7201) | Feature request: command-shift-O  -  some kind of divider between perfect and imperfect matches |
| [#7173](https://gitlab.com/gnachman/iterm2/-/issues/7173) | Unable to write \| @ since update. |
| [#7172](https://gitlab.com/gnachman/iterm2/-/issues/7172) | Feature request: persistent/CFEqual AXUIElements |
| [#7171](https://gitlab.com/gnachman/iterm2/-/issues/7171) | Feature request: access child pid of shell through AXUIElement |
| [#7168](https://gitlab.com/gnachman/iterm2/-/issues/7168) | Various minor statusbar nits |
| [#7162](https://gitlab.com/gnachman/iterm2/-/issues/7162) | Command history entries often missing a single space and become invisibly corrupted upon edits |
| [#7158](https://gitlab.com/gnachman/iterm2/-/issues/7158) | Is there a SHA-256 hash to verify the downloads? |
| [#7142](https://gitlab.com/gnachman/iterm2/-/issues/7142) | "Use thin strokes for anti-aliased text" inconsistent behaviour |
| [#7067](https://gitlab.com/gnachman/iterm2/-/issues/7067) | Problems with Expose |
| [#7053](https://gitlab.com/gnachman/iterm2/-/issues/7053) | Empty area between Status bar and main content appears sometimes |
| [#7045](https://gitlab.com/gnachman/iterm2/-/issues/7045) | Feature request: Add trigger to add annotation to string |
| [#7035](https://gitlab.com/gnachman/iterm2/-/issues/7035) | Feature suggestion: dynamic choice for full screen type |
| [#7002](https://gitlab.com/gnachman/iterm2/-/issues/7002) | Pseudo-graphic (in MC) is brocken in 3.2.1 betas |
| [#6995](https://gitlab.com/gnachman/iterm2/-/issues/6995) | Move to Applications Folder in your Home folder |
| [#6962](https://gitlab.com/gnachman/iterm2/-/issues/6962) | [Feature Request] Hide or Update Menu Shorcut Items |
| [#6920](https://gitlab.com/gnachman/iterm2/-/issues/6920) | Little blue arrow appears under/above the output |
| [#6915](https://gitlab.com/gnachman/iterm2/-/issues/6915) | Record Terminal sessions (suggested feature) |
| [#6912](https://gitlab.com/gnachman/iterm2/-/issues/6912) | [Build 3.2.0] Cursor repositioning doesn't work with multiline commands |
| [#6904](https://gitlab.com/gnachman/iterm2/-/issues/6904) | iTerm update does only quit application without updating or restarting it |
| [#6901](https://gitlab.com/gnachman/iterm2/-/issues/6901) | iTerm doesn't support ANSI overline attribute |
| [#6895](https://gitlab.com/gnachman/iterm2/-/issues/6895) | Feature Request: A way to allow triggering DashTerm2 to run specified commands |
| [#6890](https://gitlab.com/gnachman/iterm2/-/issues/6890) | Blocking visual artifacts [impossible to work] |
| [#6874](https://gitlab.com/gnachman/iterm2/-/issues/6874) | Exclude from dock disables Auto-hide menu bar |
| [#6860](https://gitlab.com/gnachman/iterm2/-/issues/6860) | rfe - when pasting, wait for newline or similar |
| [#6846](https://gitlab.com/gnachman/iterm2/-/issues/6846) | Issues Logging in using iterm2, but logs in fine using standard OSX term |
| [#6840](https://gitlab.com/gnachman/iterm2/-/issues/6840) | Full screen mode fails |
| [#6834](https://gitlab.com/gnachman/iterm2/-/issues/6834) | Trigger engages when doing shell history lookup |
| [#6822](https://gitlab.com/gnachman/iterm2/-/issues/6822) | semantic history does not consider round parentheses as separators |
| [#6821](https://gitlab.com/gnachman/iterm2/-/issues/6821) | Telnet & Serial Port Session Timeout Issues |
| [#6817](https://gitlab.com/gnachman/iterm2/-/issues/6817) | semantic history is confused if local DNS has a wildcard resolving domain in its search domains |
| [#6793](https://gitlab.com/gnachman/iterm2/-/issues/6793) | it2getvar does not work in bash command substitution |
| [#6782](https://gitlab.com/gnachman/iterm2/-/issues/6782) | Commit 72a74995 is missing in DashTerm2 version 3.1.6 |
| [#6778](https://gitlab.com/gnachman/iterm2/-/issues/6778) | Feature request: transparency percentage display |
| [#6752](https://gitlab.com/gnachman/iterm2/-/issues/6752) | Cannot move cursor to the previous line of command being entered |
| [#6751](https://gitlab.com/gnachman/iterm2/-/issues/6751) | Feature request: Center left-aligned text |
| [#6733](https://gitlab.com/gnachman/iterm2/-/issues/6733) | Terminal in Notification center. |
| [#6707](https://gitlab.com/gnachman/iterm2/-/issues/6707) | Coprocesses documentation has a creepy line of text in it |
| [#6697](https://gitlab.com/gnachman/iterm2/-/issues/6697) | Add right-button menu to convert unix timestamp into human-readable value |
| [#6695](https://gitlab.com/gnachman/iterm2/-/issues/6695) | Investigate wraptest issues |
| [#6676](https://gitlab.com/gnachman/iterm2/-/issues/6676) | New-output indicator activates after exiting full-screen mode |
| [#6635](https://gitlab.com/gnachman/iterm2/-/issues/6635) | Password manager opens but unable to enter password |
| [#6630](https://gitlab.com/gnachman/iterm2/-/issues/6630) | colour codes are not handled |
| [#6625](https://gitlab.com/gnachman/iterm2/-/issues/6625) | Open Quickly: filter types of things listed |
| [#6599](https://gitlab.com/gnachman/iterm2/-/issues/6599) | vim can't show normally when drag iterm2 from a external screen  to local screen |
| [#6541](https://gitlab.com/gnachman/iterm2/-/issues/6541) | DashTerm2 forces switch from Intel integrated GPU to discrete AMD GPU on MacBook Pro |
| [#6492](https://gitlab.com/gnachman/iterm2/-/issues/6492) | Inline images (imgcat) should always display images at size 1:1. |
| [#6487](https://gitlab.com/gnachman/iterm2/-/issues/6487) | Reverse Search and edit with customized PS1, shows a different command and executes a different comm... |
| [#6485](https://gitlab.com/gnachman/iterm2/-/issues/6485) | Launching vim |
| [#6468](https://gitlab.com/gnachman/iterm2/-/issues/6468) | Icon Missing Everywhere - High Sierra |
| [#6459](https://gitlab.com/gnachman/iterm2/-/issues/6459) | Toggling off transparency |
| [#6451](https://gitlab.com/gnachman/iterm2/-/issues/6451) | Feature request: Broadcast commands |
| [#6450](https://gitlab.com/gnachman/iterm2/-/issues/6450) | Remember last directories (histories) after restart |
| [#6448](https://gitlab.com/gnachman/iterm2/-/issues/6448) | command line editing out of sync |
| [#6432](https://gitlab.com/gnachman/iterm2/-/issues/6432) | Unable to access column number using Semantic History - Run command |
| [#6423](https://gitlab.com/gnachman/iterm2/-/issues/6423) | 1password integration |
| [#6416](https://gitlab.com/gnachman/iterm2/-/issues/6416) | Full-Width Top of Screen Terminal has a gap from the top of the screen when another app is open in f... |
| [#6413](https://gitlab.com/gnachman/iterm2/-/issues/6413) | Feature Request: Global Triggers |
| [#6409](https://gitlab.com/gnachman/iterm2/-/issues/6409) | After viewing a file with less command, the screen gets garbled |
| [#6403](https://gitlab.com/gnachman/iterm2/-/issues/6403) | Improve how external commands are run in non-interactive shells |
| [#6401](https://gitlab.com/gnachman/iterm2/-/issues/6401) | Touch ID requested only once |
| [#6384](https://gitlab.com/gnachman/iterm2/-/issues/6384) | Jump/Back word issue |
| [#6364](https://gitlab.com/gnachman/iterm2/-/issues/6364) | iTerm start tailing a directory automatically |
| [#6352](https://gitlab.com/gnachman/iterm2/-/issues/6352) | Manage command history |
| [#6350](https://gitlab.com/gnachman/iterm2/-/issues/6350) | long line indenting, query/idea |
| [#6336](https://gitlab.com/gnachman/iterm2/-/issues/6336) | Opening new session, clones previous session |
| [#6300](https://gitlab.com/gnachman/iterm2/-/issues/6300) | How to enable semantic history only for ⌘-click and favor dictionary lookup? |
| [#6292](https://gitlab.com/gnachman/iterm2/-/issues/6292) | Updates don't always apply cleanly |
| [#6274](https://gitlab.com/gnachman/iterm2/-/issues/6274) | GetAuthorizationToken: The security token included in the request is invalid. |
| [#6265](https://gitlab.com/gnachman/iterm2/-/issues/6265) | password manager confirmation |
| [#6256](https://gitlab.com/gnachman/iterm2/-/issues/6256) | feature request: terminal session restore after upgrade |
| [#6248](https://gitlab.com/gnachman/iterm2/-/issues/6248) | DashTerm2 3.1.5.beta.1 - session restore does not work |
| [#6246](https://gitlab.com/gnachman/iterm2/-/issues/6246) | Shift-R |
| [#6167](https://gitlab.com/gnachman/iterm2/-/issues/6167) | Open password manager trigger for string "Password:" |
| [#6152](https://gitlab.com/gnachman/iterm2/-/issues/6152) | Prompted to upgrade to 3.1.2 from 3.1.1 on Mac OS X Sierra never upgrades, contintues prompting. |
| [#6149](https://gitlab.com/gnachman/iterm2/-/issues/6149) | DashTerm2 possible enhancement (of transparency feature) |
| [#6121](https://gitlab.com/gnachman/iterm2/-/issues/6121) | Autocompletion and non-instant triggers |
| [#6102](https://gitlab.com/gnachman/iterm2/-/issues/6102) | Anti-aliasing results in "blurry" text |
| [#6091](https://gitlab.com/gnachman/iterm2/-/issues/6091) | White, text-less buttons |
| [#6089](https://gitlab.com/gnachman/iterm2/-/issues/6089) | Feature request: No title bar |
| [#6080](https://gitlab.com/gnachman/iterm2/-/issues/6080) | [Feature] Touch Bar customizations for irssi |
| [#6078](https://gitlab.com/gnachman/iterm2/-/issues/6078) | "bin/bash" like for DashTerm2 |
| [#6071](https://gitlab.com/gnachman/iterm2/-/issues/6071) | Captured output clear button sometimes doesn't work |
| [#6068](https://gitlab.com/gnachman/iterm2/-/issues/6068) | Post-mortem for DNS lookups issue |
| [#6054](https://gitlab.com/gnachman/iterm2/-/issues/6054) | Increase 'Job name' refresh interval |
| [#6035](https://gitlab.com/gnachman/iterm2/-/issues/6035) | number of rows discrepancy when using less |
| [#6032](https://gitlab.com/gnachman/iterm2/-/issues/6032) | A larger grab area would improve usability |
| [#6030](https://gitlab.com/gnachman/iterm2/-/issues/6030) | Inline images are always non-retina displayed. |
| [#6024](https://gitlab.com/gnachman/iterm2/-/issues/6024) | move cursor by clicking in programs that have mouse support |
| [#6012](https://gitlab.com/gnachman/iterm2/-/issues/6012) | How to use it2dl? |
| [#5984](https://gitlab.com/gnachman/iterm2/-/issues/5984) | DashTerm2 in Mac OS does not recognize composer and node |
| [#5941](https://gitlab.com/gnachman/iterm2/-/issues/5941) | Printing screen jerky (perceptibly pauses in the middle of printing). |
| [#5921](https://gitlab.com/gnachman/iterm2/-/issues/5921) | Large inline gifs being truncated |
| [#5896](https://gitlab.com/gnachman/iterm2/-/issues/5896) | feature: Hide/Show Visor style iTerm on any monitor based on mouse position. |
| [#5874](https://gitlab.com/gnachman/iterm2/-/issues/5874) | [Feature request] touch bar breadcrumbs |
| [#5848](https://gitlab.com/gnachman/iterm2/-/issues/5848) | iterm2 causes SBT to display $<3 each time <delete> is pressed |
| [#5815](https://gitlab.com/gnachman/iterm2/-/issues/5815) | PIP awareness |
| [#5784](https://gitlab.com/gnachman/iterm2/-/issues/5784) | ⌘L to clear the last command |
| [#5782](https://gitlab.com/gnachman/iterm2/-/issues/5782) | How to localize the translation？ |
| [#5769](https://gitlab.com/gnachman/iterm2/-/issues/5769) | Do not pass PATH to login |
| [#5725](https://gitlab.com/gnachman/iterm2/-/issues/5725) | feature request: session restore command whitelist |
| [#5714](https://gitlab.com/gnachman/iterm2/-/issues/5714) | RFE: Triggers - Would be a huge boon if they could be enabled/disabled with a toggle |
| [#5705](https://gitlab.com/gnachman/iterm2/-/issues/5705) | printf '\e]7;%s\a' does not erase proxy icon |
| [#5680](https://gitlab.com/gnachman/iterm2/-/issues/5680) | Password visibility |
| [#5665](https://gitlab.com/gnachman/iterm2/-/issues/5665) | strange screen flickering / screen artifacts when working in DashTerm2 |
| [#5657](https://gitlab.com/gnachman/iterm2/-/issues/5657) | Can't use Command-Control-B in Build 3.1.beta.2 |
| [#5653](https://gitlab.com/gnachman/iterm2/-/issues/5653) | Gray filled rectangle at the top of the screen |
| [#5652](https://gitlab.com/gnachman/iterm2/-/issues/5652) | Wow! Huge bad decisions to report. |
| [#5639](https://gitlab.com/gnachman/iterm2/-/issues/5639) | Feature Request: Broadcast groups |
| [#5636](https://gitlab.com/gnachman/iterm2/-/issues/5636) | Allow `Run command…` to piggyback on text magic |
| [#5629](https://gitlab.com/gnachman/iterm2/-/issues/5629) | Unable to set DashTerm2 as default terminal |
| [#5619](https://gitlab.com/gnachman/iterm2/-/issues/5619) | Session terminates immediately upon start. |
| [#5610](https://gitlab.com/gnachman/iterm2/-/issues/5610) | "Use thin strokes for anti-aliased text" is broken in 3.1 beta |
| [#5604](https://gitlab.com/gnachman/iterm2/-/issues/5604) | [Feature request] OSC 4, 10/11/12/17 support |
| [#5600](https://gitlab.com/gnachman/iterm2/-/issues/5600) | Feature Request: Expose "Toggle Smart Cursor" in View menu |
| [#5597](https://gitlab.com/gnachman/iterm2/-/issues/5597) | Mouse Pointer Slides To Top Of Screen |
| [#5595](https://gitlab.com/gnachman/iterm2/-/issues/5595) | Use Sparkle delta updates for nightly builds |
| [#5585](https://gitlab.com/gnachman/iterm2/-/issues/5585) | Launching iTerm causes different behaviours depending on where it's launched from |
| [#5556](https://gitlab.com/gnachman/iterm2/-/issues/5556) | Export Instant Replay session |
| [#5546](https://gitlab.com/gnachman/iterm2/-/issues/5546) | ﻿F4+FunctionFlip does not work correctly |
| [#5517](https://gitlab.com/gnachman/iterm2/-/issues/5517) | Transparency Adjustment |
| [#5490](https://gitlab.com/gnachman/iterm2/-/issues/5490) | [Bug] Empty space/padding/margin on right side of terminal |
| [#5476](https://gitlab.com/gnachman/iterm2/-/issues/5476) | FEATURE REQUEST: Typewriter Mode |
| [#5463](https://gitlab.com/gnachman/iterm2/-/issues/5463) | Improve visibility of italics |
| [#5451](https://gitlab.com/gnachman/iterm2/-/issues/5451) | [Feature request] Detect when natural text editing should be in use and suggest it |
| [#5445](https://gitlab.com/gnachman/iterm2/-/issues/5445) | Powershell issue |
| [#5444](https://gitlab.com/gnachman/iterm2/-/issues/5444) | suggestion: arbitrary commands for the touch bar |
| [#5443](https://gitlab.com/gnachman/iterm2/-/issues/5443) | Mouse functions frozen |
| [#5432](https://gitlab.com/gnachman/iterm2/-/issues/5432) | Unable to view session logs as it contains lots of markup which is not readable |
| [#5429](https://gitlab.com/gnachman/iterm2/-/issues/5429) | [Feature Request] Make the mouse cursor fade away when not moving for a certain amount of time |
| [#5401](https://gitlab.com/gnachman/iterm2/-/issues/5401) | Ability to have proportional shading or some other indicator at a certain column-width |
| [#5362](https://gitlab.com/gnachman/iterm2/-/issues/5362) | Feature request: golden ratio |
| [#5333](https://gitlab.com/gnachman/iterm2/-/issues/5333) | colleague DashTerm2 fork issue |
| [#5321](https://gitlab.com/gnachman/iterm2/-/issues/5321) | Iterm update broke everything |
| [#5316](https://gitlab.com/gnachman/iterm2/-/issues/5316) | Some feature requests |
| [#5306](https://gitlab.com/gnachman/iterm2/-/issues/5306) | Install Update with open terminals leaves cursor one line too high |
| [#5282](https://gitlab.com/gnachman/iterm2/-/issues/5282) | Feature request: Ability to delete/clear text from the buffer |
| [#5264](https://gitlab.com/gnachman/iterm2/-/issues/5264) | [Errno 35] write could not complete without blocking |
| [#5238](https://gitlab.com/gnachman/iterm2/-/issues/5238) | Feature Request - warn about or remove smart quotes |
| [#5228](https://gitlab.com/gnachman/iterm2/-/issues/5228) | DashTerm2 doesnt open on multiple displays |
| [#5214](https://gitlab.com/gnachman/iterm2/-/issues/5214) | SVGinOT's Feature Request |
| [#5195](https://gitlab.com/gnachman/iterm2/-/issues/5195) | iTerm trigger Run Command seems does not work 100% of the time |
| [#5194](https://gitlab.com/gnachman/iterm2/-/issues/5194) | Bash vi mode indicators |
| [#5186](https://gitlab.com/gnachman/iterm2/-/issues/5186) | "Github doesn't support issue attachments"—no longer true, I think |
| [#5185](https://gitlab.com/gnachman/iterm2/-/issues/5185) | Toolbelt Feature request(s) |
| [#5170](https://gitlab.com/gnachman/iterm2/-/issues/5170) | Prefs → Advanced, consistent styling |
| [#5169](https://gitlab.com/gnachman/iterm2/-/issues/5169) | Bold text not bold or bright |
| [#5159](https://gitlab.com/gnachman/iterm2/-/issues/5159) | Tip of the day is NOT followed by a subsequent launch iterm2 |
| [#5154](https://gitlab.com/gnachman/iterm2/-/issues/5154) | Degraded frame rate |
| [#5146](https://gitlab.com/gnachman/iterm2/-/issues/5146) | Sessions not restored after updating |
| [#5142](https://gitlab.com/gnachman/iterm2/-/issues/5142) | Sessions Not restored |
| [#5138](https://gitlab.com/gnachman/iterm2/-/issues/5138) | Check for Updates returns Update Error |
| [#5105](https://gitlab.com/gnachman/iterm2/-/issues/5105) | display problem when I type more than one line |
| [#5104](https://gitlab.com/gnachman/iterm2/-/issues/5104) | Attaching to full-screen session from another monitor produces weird results |
| [#5103](https://gitlab.com/gnachman/iterm2/-/issues/5103) | Not right "Session Restored" |
| [#5098](https://gitlab.com/gnachman/iterm2/-/issues/5098) | CJK Compatibility Ideographs get automatically converted into the non-compatibility counterparts |
| [#5094](https://gitlab.com/gnachman/iterm2/-/issues/5094) | broken pipe message is printed internally and can't be caught by a trigger |
| [#5053](https://gitlab.com/gnachman/iterm2/-/issues/5053) | Feature request: DTerm-like awareness of current working directory |
| [#5051](https://gitlab.com/gnachman/iterm2/-/issues/5051) | Restoring terminal sessions after updates does not work (anymore) |
| [#5041](https://gitlab.com/gnachman/iterm2/-/issues/5041) | Docker Quickstart Fix or Workaround: +1 |
| [#4996](https://gitlab.com/gnachman/iterm2/-/issues/4996) | toolbelt notes formatting |
| [#4974](https://gitlab.com/gnachman/iterm2/-/issues/4974) | XtraFinder bug with new iTerm in "open terminal here" |
| [#4960](https://gitlab.com/gnachman/iterm2/-/issues/4960) | Issues with Fish autocompletion |
| [#4952](https://gitlab.com/gnachman/iterm2/-/issues/4952) | Ability to navigate through stdout between places with different output rate. |
| [#4950](https://gitlab.com/gnachman/iterm2/-/issues/4950) | Ability to view JSON with folding, so you can view huge trees in console. |
| [#4928](https://gitlab.com/gnachman/iterm2/-/issues/4928) | Feature request: rolling sessiong log |
| [#4920](https://gitlab.com/gnachman/iterm2/-/issues/4920) | DashTerm2 3.0.4 issues |
| [#4877](https://gitlab.com/gnachman/iterm2/-/issues/4877) | displaying image when logged into a remote server |
| [#4871](https://gitlab.com/gnachman/iterm2/-/issues/4871) | Remove all download lists at once |
| [#4865](https://gitlab.com/gnachman/iterm2/-/issues/4865) | Feature Request: Ergonomic (Reverse) Line Ordering |
| [#4856](https://gitlab.com/gnachman/iterm2/-/issues/4856) | Markers and erased text |
| [#4827](https://gitlab.com/gnachman/iterm2/-/issues/4827) | pasting long lines wraps on top of itself |
| [#4824](https://gitlab.com/gnachman/iterm2/-/issues/4824) | I just installed build 3.0.2 on mac osx 10.10.5 this morning - within seconds of the installation my... |
| [#4817](https://gitlab.com/gnachman/iterm2/-/issues/4817) | Mouse scrooling not working anymore |
| [#4813](https://gitlab.com/gnachman/iterm2/-/issues/4813) | Updating DashTerm2 should not force a restart of the application |
| [#4808](https://gitlab.com/gnachman/iterm2/-/issues/4808) | Force Touch continues playing videos after clicking out of preview popup |
| [#4807](https://gitlab.com/gnachman/iterm2/-/issues/4807) | BASH command-line vi edit mode difficult to use |
| [#4802](https://gitlab.com/gnachman/iterm2/-/issues/4802) | "Don't offer again..." for bell ringing a lot doesn't seem to work |
| [#4788](https://gitlab.com/gnachman/iterm2/-/issues/4788) | DOC: 3.0 General usage |
| [#4787](https://gitlab.com/gnachman/iterm2/-/issues/4787) | iTerm 3 continuously runs action for trigger |
| [#4783](https://gitlab.com/gnachman/iterm2/-/issues/4783) | $10 Patreon supporters not credited in About box |
| [#4770](https://gitlab.com/gnachman/iterm2/-/issues/4770) | Upgraded to 3.0, wouldn't launch |
| [#4764](https://gitlab.com/gnachman/iterm2/-/issues/4764) | Growl / Notification Center alerts and Dock icon badge |
| [#4761](https://gitlab.com/gnachman/iterm2/-/issues/4761) | drag and drop breaks with iterm3 |
| [#4735](https://gitlab.com/gnachman/iterm2/-/issues/4735) | cursor with transparency. |
| [#4734](https://gitlab.com/gnachman/iterm2/-/issues/4734) | Feature request: reusing output without mouse |
| [#4733](https://gitlab.com/gnachman/iterm2/-/issues/4733) | Dictation doesn't add a space when you pause; should lowercase everything, maybe |
| [#4712](https://gitlab.com/gnachman/iterm2/-/issues/4712) | 3.0 Will not open for me. |
| [#4699](https://gitlab.com/gnachman/iterm2/-/issues/4699) | After an upgrade from 2.1.4 to 3 item does not start up and cannot start up after that. |
| [#4674](https://gitlab.com/gnachman/iterm2/-/issues/4674) | History feature request |
| [#4653](https://gitlab.com/gnachman/iterm2/-/issues/4653) | Enhancement: [Toolbelt Jobs] Show more info about a process |
| [#4645](https://gitlab.com/gnachman/iterm2/-/issues/4645) | Multiplex many sessions into one |
| [#4641](https://gitlab.com/gnachman/iterm2/-/issues/4641) | Docker quick start terminal not starting up. |
| [#4630](https://gitlab.com/gnachman/iterm2/-/issues/4630) | Can't set Enter to enter? (Maybe I'm stupid but I don't see a support forum so I'm posting this here... |
| [#4604](https://gitlab.com/gnachman/iterm2/-/issues/4604) | Pass context information to coprocesses |
| [#4602](https://gitlab.com/gnachman/iterm2/-/issues/4602) | Transparency behind vertical line in timestamps column |
| [#4591](https://gitlab.com/gnachman/iterm2/-/issues/4591) | screen gets cut off |
| [#4586](https://gitlab.com/gnachman/iterm2/-/issues/4586) | Feature request: Escape code for "Tile image" |
| [#4583](https://gitlab.com/gnachman/iterm2/-/issues/4583) | Feature request: close autocomplete with ^C |
| [#4577](https://gitlab.com/gnachman/iterm2/-/issues/4577) | Feature Request: Direct Edit |
| [#4567](https://gitlab.com/gnachman/iterm2/-/issues/4567) | Make processes toolbelt better |
| [#4552](https://gitlab.com/gnachman/iterm2/-/issues/4552) | Tips ignores my request to never be shown again |
| [#4537](https://gitlab.com/gnachman/iterm2/-/issues/4537) | ZSH - RPROMPT repeats line on resize |
| [#4522](https://gitlab.com/gnachman/iterm2/-/issues/4522) | user/host Badge updating *after* logging out of the host |
| [#4518](https://gitlab.com/gnachman/iterm2/-/issues/4518) | Incompatible with 'fabric' |
| [#4503](https://gitlab.com/gnachman/iterm2/-/issues/4503) | Customizable timestamp format |
| [#4496](https://gitlab.com/gnachman/iterm2/-/issues/4496) | When having the cursor set to vertical line, entering and exiting Vim causes my cursor to be block |
| [#4491](https://gitlab.com/gnachman/iterm2/-/issues/4491) | Find previous/next search in opposite direction |
| [#4475](https://gitlab.com/gnachman/iterm2/-/issues/4475) | Chromecast support |
| [#4462](https://gitlab.com/gnachman/iterm2/-/issues/4462) | After upgrade Mac OS X iTerm non functionnal |
| [#4459](https://gitlab.com/gnachman/iterm2/-/issues/4459) | Allow hover responses as a trigger action (Feature request) |
| [#4434](https://gitlab.com/gnachman/iterm2/-/issues/4434) | [Feature] Allow to open an iTerm in a specific folder from Finder |
| [#4426](https://gitlab.com/gnachman/iterm2/-/issues/4426) | feature wish |
| [#4412](https://gitlab.com/gnachman/iterm2/-/issues/4412) | RPROMPT in ZSH doesn't display |
| [#4384](https://gitlab.com/gnachman/iterm2/-/issues/4384) | Command history truncates multiline commands |
| [#4379](https://gitlab.com/gnachman/iterm2/-/issues/4379) | Feature Request: Command click file path to open in $EDITOR |
| [#4361](https://gitlab.com/gnachman/iterm2/-/issues/4361) | Issue in Arabic/Persian Strings |
| [#4326](https://gitlab.com/gnachman/iterm2/-/issues/4326) | Drag-and-drop from Mail.app fails |
| [#4311](https://gitlab.com/gnachman/iterm2/-/issues/4311) | Control Sequences: Operating System Controls (Currently Unsupported) |
| [#4273](https://gitlab.com/gnachman/iterm2/-/issues/4273) | Suppress 'Bell is Ringing' alert if computer is muted |
| [#4256](https://gitlab.com/gnachman/iterm2/-/issues/4256) | Pass variables to triggers |
| [#4241](https://gitlab.com/gnachman/iterm2/-/issues/4241) | session.shorthostname? |
| [#4220](https://gitlab.com/gnachman/iterm2/-/issues/4220) | Cursor seems a bit too short |
| [#4217](https://gitlab.com/gnachman/iterm2/-/issues/4217) | Visual bug report |
| [#4158](https://gitlab.com/gnachman/iterm2/-/issues/4158) | imgcat chops off image |
| [#4139](https://gitlab.com/gnachman/iterm2/-/issues/4139) | lrzsz doesn't work after mac osx updated to EI Captian! |
| [#4124](https://gitlab.com/gnachman/iterm2/-/issues/4124) | Make border colour customizable |
| [#4121](https://gitlab.com/gnachman/iterm2/-/issues/4121) | No cursor and incorrectly aligned text after wake or login |
| [#4090](https://gitlab.com/gnachman/iterm2/-/issues/4090) | Autoupdate downgrading a build from git |
| [#4055](https://gitlab.com/gnachman/iterm2/-/issues/4055) | GitHub project existence is causing confusions |
| [#4042](https://gitlab.com/gnachman/iterm2/-/issues/4042) | Add support for DECRQSS |
| [#4034](https://gitlab.com/gnachman/iterm2/-/issues/4034) | Improve integration with zsh/prezto |
| [#3987](https://gitlab.com/gnachman/iterm2/-/issues/3987) | strange Vim behavior |
| [#3980](https://gitlab.com/gnachman/iterm2/-/issues/3980) | Inspector |
| [#3934](https://gitlab.com/gnachman/iterm2/-/issues/3934) | Receiving Update Error when attempting to update application. |
| [#3872](https://gitlab.com/gnachman/iterm2/-/issues/3872) | Add support for Import/Export triggers |
| [#3831](https://gitlab.com/gnachman/iterm2/-/issues/3831) | Proposal: Badge feature enhancements |
| [#3817](https://gitlab.com/gnachman/iterm2/-/issues/3817) | Downloading file via right click results in error |
| [#3816](https://gitlab.com/gnachman/iterm2/-/issues/3816) | Phantom Shell unkillable |
| [#3813](https://gitlab.com/gnachman/iterm2/-/issues/3813) | Feature Request: Auto-generate bug report data |
| [#3807](https://gitlab.com/gnachman/iterm2/-/issues/3807) | Wrong screen size |
| [#3806](https://gitlab.com/gnachman/iterm2/-/issues/3806) | Multi-line command often causes first row to disappear |
| [#3804](https://gitlab.com/gnachman/iterm2/-/issues/3804) | Extraneous text on command line |
| [#3780](https://gitlab.com/gnachman/iterm2/-/issues/3780) | always unwrap wrapped text when newline occurs at the edge |
| [#3771](https://gitlab.com/gnachman/iterm2/-/issues/3771) | Random freezing which is solved by hovering over the dock |
| [#3770](https://gitlab.com/gnachman/iterm2/-/issues/3770) | Mark added to first line of multiline prompt. |
| [#3763](https://gitlab.com/gnachman/iterm2/-/issues/3763) | Path Finder 'Open in Terminal' doesn't work with DashTerm2 v3 Beta (works with GA DashTerm2 versions) |
| [#3749](https://gitlab.com/gnachman/iterm2/-/issues/3749) | Menubar not showing in secondary display |
| [#3729](https://gitlab.com/gnachman/iterm2/-/issues/3729) | Feature Request: Added a Palette Reset Code |
| [#3721](https://gitlab.com/gnachman/iterm2/-/issues/3721) | Feature Request: implement undo on pasting into a session |
| [#3715](https://gitlab.com/gnachman/iterm2/-/issues/3715) | Feature request: Allow pasting multiline buffer into one line separating items by spaces |
| [#3672](https://gitlab.com/gnachman/iterm2/-/issues/3672) | DashTerm2 does not properly handle double size text |
| [#3659](https://gitlab.com/gnachman/iterm2/-/issues/3659) | Replace is actually Append? |
| [#3648](https://gitlab.com/gnachman/iterm2/-/issues/3648) | Enhancement request: add "Notes" field for triggers |
| [#3620](https://gitlab.com/gnachman/iterm2/-/issues/3620) | Build in sudolikeaboss |
| [#3555](https://gitlab.com/gnachman/iterm2/-/issues/3555) | Investigate login -pfq $USER $COMMAND for custom commands. [was: screen width doesn't seem to get pr... |
| [#3554](https://gitlab.com/gnachman/iterm2/-/issues/3554) | feature request: "quit iterm2? all session will be closed" avoider |
| [#3540](https://gitlab.com/gnachman/iterm2/-/issues/3540) | Add "New Terminal Here" to Finder context menu |
| [#3524](https://gitlab.com/gnachman/iterm2/-/issues/3524) | Create functionality for customizing border on all edges |
| [#3436](https://gitlab.com/gnachman/iterm2/-/issues/3436) | Save environment vars on app exit and load them on restart |
| [#3433](https://gitlab.com/gnachman/iterm2/-/issues/3433) | Support xterm compatibility for proprietary escape codes |
| [#3431](https://gitlab.com/gnachman/iterm2/-/issues/3431) | Provide distinction between modifiers. |
| [#3301](https://gitlab.com/gnachman/iterm2/-/issues/3301) | HUD mode leaving gap at top of screen in nightly build |
| [#3298](https://gitlab.com/gnachman/iterm2/-/issues/3298) | sRGB support for escape codes |
| [#3280](https://gitlab.com/gnachman/iterm2/-/issues/3280) | Monitor for activity or silence |
| [#3021](https://gitlab.com/gnachman/iterm2/-/issues/3021) | password prompts not hiding composed symbols |
| [#3020](https://gitlab.com/gnachman/iterm2/-/issues/3020) | Man page in help |
| [#3007](https://gitlab.com/gnachman/iterm2/-/issues/3007) | Implement full support for OSC 4 and OSC 5 |
| [#2959](https://gitlab.com/gnachman/iterm2/-/issues/2959) | Bug with empty space on top screen |
| [#2953](https://gitlab.com/gnachman/iterm2/-/issues/2953) | temporary triggers |
| [#2936](https://gitlab.com/gnachman/iterm2/-/issues/2936) | Diacritical marks are not correctly added to some code points |
| [#2907](https://gitlab.com/gnachman/iterm2/-/issues/2907) | Global toggle for bell silence |
| [#2844](https://gitlab.com/gnachman/iterm2/-/issues/2844) | Sparkle update downloads corrupt update files |
| [#2760](https://gitlab.com/gnachman/iterm2/-/issues/2760) | Don't restart when an update is ready |
| [#2713](https://gitlab.com/gnachman/iterm2/-/issues/2713) | Allow terminal size lock |
| [#2640](https://gitlab.com/gnachman/iterm2/-/issues/2640) | Always on Top function |
| [#2626](https://gitlab.com/gnachman/iterm2/-/issues/2626) | Add minimap support |
| [#2618](https://gitlab.com/gnachman/iterm2/-/issues/2618) | Support Fraktur ANSI escape sequence |
| [#2494](https://gitlab.com/gnachman/iterm2/-/issues/2494) | Dragging into the dock icon works for folders but not for files. |
| [#2443](https://gitlab.com/gnachman/iterm2/-/issues/2443) | Ignore/replace text incoming |
| [#2426](https://gitlab.com/gnachman/iterm2/-/issues/2426) | caps lock remap |
| [#2379](https://gitlab.com/gnachman/iterm2/-/issues/2379) | Learning spellcheck as I type |
| [#2246](https://gitlab.com/gnachman/iterm2/-/issues/2246) | Make titles completely customizable |
| [#2205](https://gitlab.com/gnachman/iterm2/-/issues/2205) | Ignore part of the screen for activity notification |
| [#2187](https://gitlab.com/gnachman/iterm2/-/issues/2187) | ignore certain part of screen (for activity detection) |
| [#2111](https://gitlab.com/gnachman/iterm2/-/issues/2111) | Control session logging from the command line |
| [#2100](https://gitlab.com/gnachman/iterm2/-/issues/2100) | Local editor support for remote files |
| [#2058](https://gitlab.com/gnachman/iterm2/-/issues/2058) | Customize left mouse button behaviour |
| [#2052](https://gitlab.com/gnachman/iterm2/-/issues/2052) | Enable Text Substitution |
| [#2010](https://gitlab.com/gnachman/iterm2/-/issues/2010) | Lock Terminal / Read-Only Mode |
| [#2001](https://gitlab.com/gnachman/iterm2/-/issues/2001) | Protect session |
| [#1955](https://gitlab.com/gnachman/iterm2/-/issues/1955) | Automatically close a session when it ends: Only on clean exit |
| [#1934](https://gitlab.com/gnachman/iterm2/-/issues/1934) | Add support for moving around blocks of text for current command |
| [#1896](https://gitlab.com/gnachman/iterm2/-/issues/1896) | Add support for trigger to replace text that appears on screen |
| [#1796](https://gitlab.com/gnachman/iterm2/-/issues/1796) | Remove DashTerm2 from the Mac Application Switcher |
| [#1795](https://gitlab.com/gnachman/iterm2/-/issues/1795) | Improve session logging |
| [#1790](https://gitlab.com/gnachman/iterm2/-/issues/1790) | Provide toggle to turn on/off line wrapping |
| [#1726](https://gitlab.com/gnachman/iterm2/-/issues/1726) | Wait for shell to exit on iterm quit |
| [#1655](https://gitlab.com/gnachman/iterm2/-/issues/1655) | Dim via opacity % |
| [#1652](https://gitlab.com/gnachman/iterm2/-/issues/1652) | Allow Terminals to be named |
| [#1643](https://gitlab.com/gnachman/iterm2/-/issues/1643) | Add a status bar to the toolbelt [was: pipe a command's output to iterm Toolbelt] |
| [#1640](https://gitlab.com/gnachman/iterm2/-/issues/1640) | Large amounts of unsafe code executed in child process after fork |
| [#1637](https://gitlab.com/gnachman/iterm2/-/issues/1637) | Enhancement of coprocessors |
| [#1625](https://gitlab.com/gnachman/iterm2/-/issues/1625) | toolbelt menu is confusing |
| [#1611](https://gitlab.com/gnachman/iterm2/-/issues/1611) | Add support for RTL languages |
| [#1605](https://gitlab.com/gnachman/iterm2/-/issues/1605) | Num Lock Indicator |
| [#1591](https://gitlab.com/gnachman/iterm2/-/issues/1591) | Gist support |
| [#1521](https://gitlab.com/gnachman/iterm2/-/issues/1521) | Inline HTML output |
| [#1515](https://gitlab.com/gnachman/iterm2/-/issues/1515) | Add UI to tweak LC_CTYPE, LANG locale vars |
| [#1507](https://gitlab.com/gnachman/iterm2/-/issues/1507) | DashTerm2 Is Using the Wrong Colour Space |
| [#1333](https://gitlab.com/gnachman/iterm2/-/issues/1333) | Man pages via help menu |
| [#1307](https://gitlab.com/gnachman/iterm2/-/issues/1307) | Add timeout for growl notifications |
| [#1048](https://gitlab.com/gnachman/iterm2/-/issues/1048) | Customizable growl notifications |
| [#983](https://gitlab.com/gnachman/iterm2/-/issues/983) | Don't override user control of swipe action |
| [#867](https://gitlab.com/gnachman/iterm2/-/issues/867) | Fancy Graphics Effects |
| [#859](https://gitlab.com/gnachman/iterm2/-/issues/859) | Add a way to specify the working directory as a parameter |
| [#696](https://gitlab.com/gnachman/iterm2/-/issues/696) | Approximate string matching |
| [#676](https://gitlab.com/gnachman/iterm2/-/issues/676) | Serial port connection |
| [#503](https://gitlab.com/gnachman/iterm2/-/issues/503) | Output folding |
| [#283](https://gitlab.com/gnachman/iterm2/-/issues/283) | Make code style consistent across all source files. |
| [#252](https://gitlab.com/gnachman/iterm2/-/issues/252) | Improvements to instant replay |
| [#234](https://gitlab.com/gnachman/iterm2/-/issues/234) | Get all vttest tests working the same as xterm, or better |

---

## Implementation Plan

### Sprint 1-2: Critical Stability (P0)
Fix all 288 crash/hang issues to make DashTerm2 rock-solid.

### Sprint 3-4: Core Integrations (P1)
Fix AI, tmux, SSH, and shell integration issues.

### Sprint 5-8: User Experience (P2)
Fix performance, keyboard, window, scroll, and clipboard issues.

### Sprint 9+: Polish (P3)
Fix color, browser, profile, and API issues.

---

*This document tracks ALL open upstream DashTerm2 issues.*
*DashTerm2 aims to fix these bugs to differentiate from DashTerm2.*
