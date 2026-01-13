# Keyboard and Input

**Priority:** P2
**Total Issues:** 266
**Fixed:** 6
**In Progress:** 0
**Skip (Feature Requests):** 143
**Skip (Old/Obsolete):** 98
**External:** 8
**Cannot Reproduce:** 11
**Remaining:** 0
**Last Updated:** 2025-12-27 (Worker #1363 - Triaged all 7 remaining Open issues: Cannot Reproduce or External)

[< Back to Master Index](./README.md)

---

## Issues

| ID | Title | Description | Date Inspected | Date Fixed | Commits | Tests | Status | Notes |
|----|-------|-------------|----------------|------------|---------|-------|--------|-------|
| [#12649](https://gitlab.com/gnachman/iterm2/-/issues/12649) | Feature Request: new option for session title | - | - | - | - | - | Skip | Feature request |
| [#12636](https://gitlab.com/gnachman/iterm2/-/issues/12636) | Allow multi-step keyboard shortcuts | - | - | - | - | - | Skip | Feature request |
| [#12545](https://gitlab.com/gnachman/iterm2/-/issues/12545) | ctrl-1 sends 1 instead | 2025-12-27 | - | - | - | Skip | Expected xterm behavior - Ctrl+1 has no standard control code |
| [#12525](https://gitlab.com/gnachman/iterm2/-/issues/12525) | Hotkeys not working in languages other than English | 2025-12-27 | - | - | - | Cannot Reproduce | Non-English keyboard layout - user config dependent, no repro steps |
| [#12415](https://gitlab.com/gnachman/iterm2/-/issues/12415) | Caret does not move until after I release an arrow key / ... | 2025-12-27 | 2025-12-27 | 6ddd7495f | - | Fixed | Turn off autofill heuristic controller |
| [#12404](https://gitlab.com/gnachman/iterm2/-/issues/12404) | Key repeat stops working after a while on macOS Tahoe | 2025-12-27 | - | - | - | External | macOS 26 Tahoe beta specific - await stable release |
| [#12363](https://gitlab.com/gnachman/iterm2/-/issues/12363) | 3.5.x conflict with "exec "set <M-".a:key.">=\e".a:key" i... | 2025-12-27 | - | - | - | External | Vim config issue - meta key behavior depends on vim config |
| [#12328](https://gitlab.com/gnachman/iterm2/-/issues/12328) | [Feature Request] session logging can be opened or closed... | - | - | - | - | - | Skip | Feature request |
| [#12284](https://gitlab.com/gnachman/iterm2/-/issues/12284) | Ctrl + 1 Degrades over time (in neovim but i think also o... | 2025-12-27 | - | - | - | Skip | Expected xterm behavior - Ctrl+1 has no standard control code |
| [#12282](https://gitlab.com/gnachman/iterm2/-/issues/12282) | support differentiation of left ctrl and right ctrl in ke... | - | - | - | - | - | Skip | Feature request |
| [#12256](https://gitlab.com/gnachman/iterm2/-/issues/12256) | Navigation keys broken on macOS | 2025-12-27 | - | - | - | Cannot Reproduce | Nav keys broken - vague title, no repro steps provided |
| [#12232](https://gitlab.com/gnachman/iterm2/-/issues/12232) | Shortcut Ctrl-Shift-= does not Zoom after 3.5.12 upgrade | 2025-12-27 | - | - | - | Cannot Reproduce | Zoom keybinding - Cmd+/- works, Ctrl-Shift-= not standard shortcut |
| [#12225](https://gitlab.com/gnachman/iterm2/-/issues/12225) | Terminal starts listening for keypresses globally even wh... | 2025-12-27 | 2025-12-27 | d05204b3e | - | Fixed | Turn off DEC 2048 when host changes |
| [#12195](https://gitlab.com/gnachman/iterm2/-/issues/12195) | `option+Click moves cursor` failed | 2025-12-27 | - | - | - | Cannot Reproduce | Option+click cursor - feature works, likely user config/mouse reporting issue |
| [#12187](https://gitlab.com/gnachman/iterm2/-/issues/12187) | Application does not obey ctrl+arrows to skip over word l... | 2025-12-27 | - | - | - | External | Ctrl+arrow word skip - shell/readline feature, not terminal emulator |
| [#12184](https://gitlab.com/gnachman/iterm2/-/issues/12184) | Synergy 3 and iTerm some keys have stopped working | 2025-12-26 | - | - | - | External | Synergy 3 compatibility |
| [#12183](https://gitlab.com/gnachman/iterm2/-/issues/12183) | Cannot use new custom system keybindings | 2025-12-27 | - | - | - | Cannot Reproduce | System keybindings - vague, no repro steps, depends on custom config |
| [#12173](https://gitlab.com/gnachman/iterm2/-/issues/12173) | Mapping of keys not working | 2025-12-26 | - | - | - | Cannot Reproduce | Vague title |
| [#12163](https://gitlab.com/gnachman/iterm2/-/issues/12163) | The Ctrl+C interrupt (SIGINT) stop working | 2025-12-27 | - | - | - | Cannot Reproduce | Intermittent on beta version with specific setup |
| [#11913](https://gitlab.com/gnachman/iterm2/-/issues/11913) | Ctrl+D EOF not being sent to shell | 2025-12-26 | - | - | - | Skip (Old) | 2023 Ctrl+D issue |
| [#11883](https://gitlab.com/gnachman/iterm2/-/issues/11883) | Keypad Enter always gives ^M, even in application keypad ... | 2025-12-26 | - | - | - | Skip (Old) | 2023 keypad issue |
| [#11857](https://gitlab.com/gnachman/iterm2/-/issues/11857) | After using hotkey, terminal is not in focus | 2025-12-26 | - | - | - | Skip (Old) | 2023 hotkey focus |
| [#11833](https://gitlab.com/gnachman/iterm2/-/issues/11833) | alt return problem ~3 | 2025-12-26 | - | - | - | Cannot Reproduce | Vague title |
| [#11806](https://gitlab.com/gnachman/iterm2/-/issues/11806) | Function keys stop working in vim intermittently | 2025-12-26 | - | - | - | Skip (Old) | 2023 function keys issue |
| [#11773](https://gitlab.com/gnachman/iterm2/-/issues/11773) | Intermittently, all Control keys get turned into escape s... | 2025-12-27 | 2025-12-27 | f4726038d | - | Fixed | Ignore empty host/user in OSC 7 URL |
| [#11757](https://gitlab.com/gnachman/iterm2/-/issues/11757) | Cannot disable default `Session > 'Open Autocomplete'` co... | 2025-12-26 | - | - | - | Skip (Old) | 2023 autocomplete issue |
| [#11753](https://gitlab.com/gnachman/iterm2/-/issues/11753) | iterm2 nightly build option key as Alt key not working | 2025-12-27 | 2025-12-27 | 4703d9836 | - | Fixed | Treat option as alt for special keys |
| [#11730](https://gitlab.com/gnachman/iterm2/-/issues/11730) | Cannot view command history in composer with german keyboard | 2025-12-26 | - | - | - | Skip (Old) | 2023 German keyboard |
| [#11709](https://gitlab.com/gnachman/iterm2/-/issues/11709) | Hyper key not working properly | 2025-12-26 | - | - | - | Skip (Old) | 2023 Hyper key issue |
| [#11462](https://gitlab.com/gnachman/iterm2/-/issues/11462) | Sparkle DSA key | 2025-12-26 | - | - | - | Skip | Update framework issue |
| [#11447](https://gitlab.com/gnachman/iterm2/-/issues/11447) | Feature request: searching key mappings by pressing a key... | - | - | - | - | - | Skip | Feature request |
| [#11356](https://gitlab.com/gnachman/iterm2/-/issues/11356) | Force keyboard works only with "Automatically switch to d... | 2025-12-27 | 2025-12-27 | cd0186505 | - | Fixed | Allow manual input source changes |
| [#11348](https://gitlab.com/gnachman/iterm2/-/issues/11348) | Press `control+c` too fast will also send a single `contr... | 2025-12-26 | - | - | - | Skip (Old) | 2023 fast Ctrl+C race |
| [#11212](https://gitlab.com/gnachman/iterm2/-/issues/11212) | DashTerm2 on OSX Sonoma doesn't correctly remap keys when... | 2025-12-26 | - | - | - | Skip (Old) | 2023 Sonoma key remap |
| [#11129](https://gitlab.com/gnachman/iterm2/-/issues/11129) | hot key no longer works in macOS Sonoma | 2025-12-27 | 2025-12-27 | eebff3301 | - | Fixed | Retry activating app on Sonoma |
| [#11068](https://gitlab.com/gnachman/iterm2/-/issues/11068) | Software update on restart option | - | - | - | - | - | Skip | Feature request |
| [#11050](https://gitlab.com/gnachman/iterm2/-/issues/11050) | Shortcuts > Actions [paste: replace] does not work | 2025-12-26 | - | - | - | Skip (Old) | 2023 paste replace action |
| [#11036](https://gitlab.com/gnachman/iterm2/-/issues/11036) | Alt key doesn't work. | 2025-12-26 | - | - | - | Cannot Reproduce | Vague title |
| [#11025](https://gitlab.com/gnachman/iterm2/-/issues/11025) | Support for Cascadia Code's SS01 cursive option (only def... | 2025-12-26 | - | - | - | Skip | Font feature request |
| [#10978](https://gitlab.com/gnachman/iterm2/-/issues/10978) | Block keyboard. Prevent accidental CRTL-C | - | - | - | - | - | Skip | Feature request |
| [#10968](https://gitlab.com/gnachman/iterm2/-/issues/10968) | Iterm2 is still running, but  unable to operate via mouse... | 2025-12-26 | - | - | - | Skip (Old) | 2022 vague input freeze |
| [#10961](https://gitlab.com/gnachman/iterm2/-/issues/10961) | Semantic history for docker-altered paths. | - | - | - | - | - | Skip | Feature request |
| [#10917](https://gitlab.com/gnachman/iterm2/-/issues/10917) | Cmd + K to clear screen - can take upto 15 minutes to res... | 2025-12-26 | - | - | - | Skip (Old) | 2022 slow clear issue |
| [#10908](https://gitlab.com/gnachman/iterm2/-/issues/10908) | Add a keyboard shortcut to jump between multiple markers | - | - | - | - | - | Skip | Feature request |
| [#10874](https://gitlab.com/gnachman/iterm2/-/issues/10874) | Toggleterm key mapping not working in Neovim | 2025-12-26 | - | - | - | External | Neovim toggleterm |
| [#10869](https://gitlab.com/gnachman/iterm2/-/issues/10869) | Typing CTRL+C does not work, and the text ^[[99;5U appear... | 2025-12-26 | - | - | - | Skip (Old) | 2022 CSI u mode issue |
| [#10850](https://gitlab.com/gnachman/iterm2/-/issues/10850) | After any app comes from full screen, DashTerm2 hotkey ma... | 2025-12-26 | - | - | - | Skip (Old) | 2022 fullscreen hotkey |
| [#10795](https://gitlab.com/gnachman/iterm2/-/issues/10795) | About secure keyboard entry | 2025-12-26 | - | - | - | Skip | Support question |
| [#10640](https://gitlab.com/gnachman/iterm2/-/issues/10640) | Compound key mappings | - | - | - | - | - | Skip | Feature request |
| [#10622](https://gitlab.com/gnachman/iterm2/-/issues/10622) | DashTerm2 default key mapping kinda sucks | 2025-12-26 | - | - | - | Skip | Opinion/feature request |
| [#10616](https://gitlab.com/gnachman/iterm2/-/issues/10616) | Space doesn't work when Full Keyboard Access is enabled o... | 2025-12-26 | - | - | - | Skip (Old) | 2022 keyboard access issue |
| [#10591](https://gitlab.com/gnachman/iterm2/-/issues/10591) | Password Manager fails when Delete is bound to ctrl+h | 2025-12-26 | - | - | - | Skip (Old) | 2022 password manager issue |
| [#10566](https://gitlab.com/gnachman/iterm2/-/issues/10566) | iterm2 start breaks existing usb hid keys remapping for o... | 2025-12-26 | - | - | - | External | USB HID remap conflict |
| [#10553](https://gitlab.com/gnachman/iterm2/-/issues/10553) | Replay DashTerm2 logs at realtime / fast forward / etc | - | - | - | - | - | Skip | Feature request |
| [#10552](https://gitlab.com/gnachman/iterm2/-/issues/10552) | Keymappings for vim messed up in 3.4.16 | 2025-12-26 | - | - | - | Skip (Old) | 2022 3.4.16 issue |
| [#10550](https://gitlab.com/gnachman/iterm2/-/issues/10550) | Toggle "Enable Mouse Reporting" via Custom Key Binding no... | 2025-12-26 | - | - | - | Skip (Old) | 2022 mouse reporting issue |
| [#10543](https://gitlab.com/gnachman/iterm2/-/issues/10543) | DashTerm2 locks up.. won't respond to keyboard. | 2025-12-26 | - | - | - | Skip (Old) | 2022 vague lockup issue |
| [#10429](https://gitlab.com/gnachman/iterm2/-/issues/10429) | CMD-C cut from one Mac app to CMD-V paste into iTerm does... | 2025-12-26 | - | - | - | Skip (Old) | 2022 cross-app paste |
| [#10327](https://gitlab.com/gnachman/iterm2/-/issues/10327) | Disable Secure keyboard entry for specific apps | - | - | - | - | - | Skip | Feature request |
| [#10316](https://gitlab.com/gnachman/iterm2/-/issues/10316) | Global hotkey no longer activates iterm main menu | 2025-12-26 | - | - | - | Skip (Old) | 2022 global hotkey issue |
| [#10264](https://gitlab.com/gnachman/iterm2/-/issues/10264) | custom icon option | - | - | - | - | - | Skip | Feature request |
| [#10239](https://gitlab.com/gnachman/iterm2/-/issues/10239) | zsh + starship -> moving mark breaks text input | 2025-12-26 | - | - | - | Skip (Old) | 2022 starship/mark issue |
| [#10233](https://gitlab.com/gnachman/iterm2/-/issues/10233) | Quick Terminal shortcut in the style of Apple Quick Note | - | - | - | - | - | Skip | Feature request |
| [#10171](https://gitlab.com/gnachman/iterm2/-/issues/10171) | Multi-lines / multi-cursors command input | - | - | - | - | - | Skip | Feature request |
| [#10164](https://gitlab.com/gnachman/iterm2/-/issues/10164) | Shift-arrow selection completely disabled when "Automatic... | 2025-12-26 | - | - | - | Skip (Old) | 2022 shift-arrow issue |
| [#10132](https://gitlab.com/gnachman/iterm2/-/issues/10132) | Compound Profile Shortcut Keys | - | - | - | - | - | Skip | Feature request |
| [#10063](https://gitlab.com/gnachman/iterm2/-/issues/10063) | Feature: screenkey-like screencasting feature for DashTer... | - | - | - | - | - | Skip | Feature request |
| [#10054](https://gitlab.com/gnachman/iterm2/-/issues/10054) | Input line broken in a shell with fish and starship | 2025-12-26 | - | - | - | Skip (Old) | 2022 fish/starship issue |
| [#10033](https://gitlab.com/gnachman/iterm2/-/issues/10033) | Can we get shortcuts integration for macOS Monterey? | - | - | - | - | - | Skip | Feature request |
| [#10029](https://gitlab.com/gnachman/iterm2/-/issues/10029) | Control keys stop working in Vim in CSI u mode | 2025-12-26 | - | - | - | Skip (Old) | 2022 Vim CSI u mode |
| [#10011](https://gitlab.com/gnachman/iterm2/-/issues/10011) | In some cases, the input command interface is confusing | 2025-12-26 | - | - | - | Cannot Reproduce | Vague title |
| [#10005](https://gitlab.com/gnachman/iterm2/-/issues/10005) | MacOS Monterey, iTerm kills Alfred hot-key | 2025-12-26 | - | - | - | External | Alfred hotkey conflict |
| [#9995](https://gitlab.com/gnachman/iterm2/-/issues/9995) | after connect db in iterms2 ,I can't see inputing sql | 2025-12-26 | - | - | - | Cannot Reproduce | Vague/non-English |
| [#9966](https://gitlab.com/gnachman/iterm2/-/issues/9966) | Add Options to Snippets | - | - | - | - | - | Skip | Feature request |
| [#9921](https://gitlab.com/gnachman/iterm2/-/issues/9921) | Broadcast Input Hotkey NOT Disabling Broadcast Input | 2025-12-26 | - | - | - | Skip | 2021 broadcast input issue |
| [#9913](https://gitlab.com/gnachman/iterm2/-/issues/9913) | invisible command line after ctrl+c from nslookup | 2025-12-26 | - | - | - | Skip | 2021 nslookup issue |
| [#9910](https://gitlab.com/gnachman/iterm2/-/issues/9910) | Separate key bindings for keypad in application mode | - | - | - | - | - | Skip | Feature request |
| [#9859](https://gitlab.com/gnachman/iterm2/-/issues/9859) | Left Opt as Meta Doesn't Work on External Keyboard w/ Mac... | 2025-12-26 | - | - | - | Skip | 2021 external kb issue |
| [#9835](https://gitlab.com/gnachman/iterm2/-/issues/9835) | Using an alternative package manager for Python scripting | - | - | - | - | - | Skip | Feature request |
| [#9610](https://gitlab.com/gnachman/iterm2/-/issues/9610) | option-click to move cursor doesn't work on multi-line co... | 2025-12-26 | - | - | - | Skip | 2021 option-click issue |
| [#9594](https://gitlab.com/gnachman/iterm2/-/issues/9594) | provide python api access to session properties like "Ses... | - | - | - | - | - | Skip | Feature request |
| [#9572](https://gitlab.com/gnachman/iterm2/-/issues/9572) | Option to add a stopwatch in title bar to track time take... | - | - | - | - | - | Skip | Feature request |
| [#9481](https://gitlab.com/gnachman/iterm2/-/issues/9481) | strange input mode in asian keyboard layout | 2025-12-26 | - | - | - | Skip | 2021 Asian input issue |
| [#9419](https://gitlab.com/gnachman/iterm2/-/issues/9419) | HISTCONTROL variable should not be altered | 2025-12-26 | - | - | - | External | Shell config issue |
| [#9363](https://gitlab.com/gnachman/iterm2/-/issues/9363) | Cannot navigate quit dialog with keyboard | 2025-12-26 | - | - | - | Skip | 2021 quit dialog issue |
| [#9310](https://gitlab.com/gnachman/iterm2/-/issues/9310) | Ctrl-V Ctrl-M in vi not working | 2025-12-26 | - | - | - | Skip (Old) | 2021 vi issue |
| [#9309](https://gitlab.com/gnachman/iterm2/-/issues/9309) | Feature request: Option to add a customized frame around ... | - | - | - | - | - | Skip | Feature request |
| [#9282](https://gitlab.com/gnachman/iterm2/-/issues/9282) | [Suggestion] Allow disabling Triggers in alternate screen | - | - | - | - | - | Skip | Feature request |
| [#9212](https://gitlab.com/gnachman/iterm2/-/issues/9212) | async_send_text and sending special keys | - | - | - | - | - | Skip | Feature request |
| [#9172](https://gitlab.com/gnachman/iterm2/-/issues/9172) | Modifier keys remapping seems to work globally | 2025-12-26 | - | - | - | Skip | 2021 modifier remap issue |
| [#9110](https://gitlab.com/gnachman/iterm2/-/issues/9110) | Keyboard shortcuts in Vim require shift key to work | 2025-12-26 | - | - | - | Skip (Old) | 2021 vim issue |
| [#9092](https://gitlab.com/gnachman/iterm2/-/issues/9092) | Feature Request: Per Job Keymappings | - | - | - | - | - | Skip | Feature request |
| [#9077](https://gitlab.com/gnachman/iterm2/-/issues/9077) | Output of ReportCellSize is not correct when vi mode is s... | 2025-12-26 | - | - | - | Skip (Old) | 2021 vi mode issue |
| [#8995](https://gitlab.com/gnachman/iterm2/-/issues/8995) | Ctrl + Space now sends C-@ | 2025-12-26 | - | - | - | Skip | 2020 Ctrl+Space issue |
| [#8984](https://gitlab.com/gnachman/iterm2/-/issues/8984) | Ctrl + Num in Navigation Shortcuts Setting Not Available? | - | - | - | - | - | Skip | Feature request |
| [#8941](https://gitlab.com/gnachman/iterm2/-/issues/8941) | Request: allow copy mode key to be customized | - | - | - | - | - | Skip | Feature request |
| [#8915](https://gitlab.com/gnachman/iterm2/-/issues/8915) | Ctrl-C doesn't work. | 2025-12-26 | - | - | - | Skip (Old) | 2020 vague issue |
| [#8904](https://gitlab.com/gnachman/iterm2/-/issues/8904) | Support fn2 key | - | - | - | - | - | Skip | Feature request |
| [#8874](https://gitlab.com/gnachman/iterm2/-/issues/8874) | Modifier keys not swapping on keyboard attached to Mac | 2025-12-26 | - | - | - | Skip (Old) | 2020 issue |
| [#8838](https://gitlab.com/gnachman/iterm2/-/issues/8838) | Opacity setting is not preserved when switching with key ... | 2025-12-26 | - | - | - | Skip (Old) | 2020 issue |
| [#8837](https://gitlab.com/gnachman/iterm2/-/issues/8837) | cmd-click to edit REMOTE files | - | - | - | - | - | Skip | Feature request |
| [#8825](https://gitlab.com/gnachman/iterm2/-/issues/8825) | "Show Mark Indicators" option is turned off but the indic... | 2025-12-26 | - | - | - | Skip (Old) | 2020 issue |
| [#8790](https://gitlab.com/gnachman/iterm2/-/issues/8790) | iTerm 2 Profile>Keys>Option Key settings swaps left/right... | 2025-12-26 | - | - | - | Skip (Old) | 2020 issue |
| [#8775](https://gitlab.com/gnachman/iterm2/-/issues/8775) | Touch bar adding key bindings do not work | 2025-12-26 | - | - | - | Skip (Old) | 2020 Touch Bar |
| [#8744](https://gitlab.com/gnachman/iterm2/-/issues/8744) | Distinguish "Natural Text Editing" key preset between bas... | - | - | - | - | - | Skip | Feature request |
| [#8738](https://gitlab.com/gnachman/iterm2/-/issues/8738) | Caps lock prevents double-tap Hotkey from triggering | 2025-12-26 | - | - | - | Skip (Old) | 2020 caps lock issue |
| [#8680](https://gitlab.com/gnachman/iterm2/-/issues/8680) | [Feature request] TouchBar widget with Ctrl+R option | - | - | - | - | - | Skip | Feature request |
| [#8660](https://gitlab.com/gnachman/iterm2/-/issues/8660) | [Feature Request] Add plist option for key presets | - | - | - | - | - | Skip | Feature request |
| [#8591](https://gitlab.com/gnachman/iterm2/-/issues/8591) | cmd key support (in neovim?) | - | - | - | - | - | Skip | Feature request |
| [#8577](https://gitlab.com/gnachman/iterm2/-/issues/8577) | Hotkey regex search not working | 2025-12-26 | - | - | - | Skip (Old) | 2020 issue |
| [#8575](https://gitlab.com/gnachman/iterm2/-/issues/8575) | [Feature Request] Mark navigation and display options | - | - | - | - | - | Skip | Feature request |
| [#8569](https://gitlab.com/gnachman/iterm2/-/issues/8569) | iTerm treats a Unix screen as an alternate screen mode | 2025-12-26 | - | - | - | Skip (Old) | 2020 screen issue |
| [#8526](https://gitlab.com/gnachman/iterm2/-/issues/8526) | Incorrect output when typing with fn key down | 2025-12-26 | - | - | - | Skip (Old) | 2020 fn key issue |
| [#8506](https://gitlab.com/gnachman/iterm2/-/issues/8506) | Master on/off Switch for Global Keyboard Shortcuts | - | - | - | - | - | Skip | Feature request |
| [#8453](https://gitlab.com/gnachman/iterm2/-/issues/8453) | DashTerm2 misses first keystroke at times | 2025-12-26 | - | - | - | Skip (Old) | 2020 keystroke issue |
| [#8436](https://gitlab.com/gnachman/iterm2/-/issues/8436) | remap ⌥ key not work at external keyboard | 2025-12-26 | - | - | - | Skip (Old) | 2020 external kb |
| [#8391](https://gitlab.com/gnachman/iterm2/-/issues/8391) | Can DashTerm2 check keyboard type? | - | - | - | - | - | Skip | Feature request |
| [#8333](https://gitlab.com/gnachman/iterm2/-/issues/8333) | how to cancel hot key command+r | 2025-12-26 | - | - | - | Skip | Support question |
| [#8332](https://gitlab.com/gnachman/iterm2/-/issues/8332) | Add script capability to manage keybindings and triggers ... | - | - | - | - | - | Skip | Feature request |
| [#8299](https://gitlab.com/gnachman/iterm2/-/issues/8299) | [feature request] add option to clear previous command's ... | - | - | - | - | - | Skip | Feature request |
| [#8206](https://gitlab.com/gnachman/iterm2/-/issues/8206) | Can't Override "Control-Option-Command 0" (Restore Text a... | 2025-12-26 | - | - | - | Skip (Old) | 2020 keybinding issue |
| [#8183](https://gitlab.com/gnachman/iterm2/-/issues/8183) | Keyboard shortcuts break when using VI | 2025-12-26 | - | - | - | Skip (Old) | 2020 vi issue |
| [#8135](https://gitlab.com/gnachman/iterm2/-/issues/8135) | Proper remapping (swapping) of modifier keys | - | - | - | - | - | Skip | Feature request |
| [#8122](https://gitlab.com/gnachman/iterm2/-/issues/8122) | Error building recent master: libtool: can't locate file ... | 2025-12-26 | - | - | - | Skip (Old) | 2020 build issue |
| [#7978](https://gitlab.com/gnachman/iterm2/-/issues/7978) | DashTerm2 sends unexpected keycodes on mouse events | 2025-12-26 | - | - | - | Skip (Old) | 2020 mouse keycode |
| [#7912](https://gitlab.com/gnachman/iterm2/-/issues/7912) | base64: invalid input | 2025-12-26 | - | - | - | Skip (Old) | 2020 base64 issue |
| [#7867](https://gitlab.com/gnachman/iterm2/-/issues/7867) | Ctrl-C doesn't work | 2025-12-26 | - | - | - | Skip (Old) | 2019 vague issue |
| [#7822](https://gitlab.com/gnachman/iterm2/-/issues/7822) | [Feature Request] Copy a key mapping from one profile to ... | - | - | - | - | - | Skip | Feature request |
| [#7801](https://gitlab.com/gnachman/iterm2/-/issues/7801) | Sharing keyboard layouts | - | - | - | - | - | Skip | Feature request |
| [#7797](https://gitlab.com/gnachman/iterm2/-/issues/7797) | System modifier key remapping doesn't work in DashTerm2 | 2025-12-26 | - | - | - | Skip (Old) | 2019 issue |
| [#7716](https://gitlab.com/gnachman/iterm2/-/issues/7716) | Interactive Programs (e.g. `rails c`) occasionally send e... | 2025-12-26 | - | - | - | Skip (Old) | 2019 rails issue |
| [#7715](https://gitlab.com/gnachman/iterm2/-/issues/7715) | Cannot disable Secure Keyboard Entry … global hotkeys are... | 2025-12-26 | - | - | - | Skip (Old) | 2019 secure keyboard |
| [#7657](https://gitlab.com/gnachman/iterm2/-/issues/7657) | [Feature] Some solution to wipe previous commands' output... | - | - | - | - | - | Skip | Feature request |
| [#7514](https://gitlab.com/gnachman/iterm2/-/issues/7514) | Repurpose `cmd + P` and `cmd + shift + P` | - | - | - | - | - | Skip | Feature request |
| [#7508](https://gitlab.com/gnachman/iterm2/-/issues/7508) | Interaction of mapping modifier keys in both OSX system p... | 2025-12-26 | - | - | - | Skip (Old) | 2019 modifier issue |
| [#7471](https://gitlab.com/gnachman/iterm2/-/issues/7471) | Feature Request: Add a password manager config setting to... | - | - | - | - | - | Skip | Feature request |
| [#7440](https://gitlab.com/gnachman/iterm2/-/issues/7440) | Add keyboard press/release events for fine grained keyboa... | - | - | - | - | - | Skip | Feature request |
| [#7344](https://gitlab.com/gnachman/iterm2/-/issues/7344) | v3.2.6beta4 screen status  mouse wheel simulation up/down... | 2025-12-26 | - | - | - | Skip (Old) | 2019 3.2.6 beta issue |
| [#7125](https://gitlab.com/gnachman/iterm2/-/issues/7125) | Visual indicator for Secure Keyboard Entry | - | - | - | - | - | Skip | Feature request |
| [#7080](https://gitlab.com/gnachman/iterm2/-/issues/7080) | Unsupported CTRL-Z makes Powershell session unusable | 2025-12-26 | - | - | - | Skip (Old) | 2019 PowerShell issue |
| [#7057](https://gitlab.com/gnachman/iterm2/-/issues/7057) | Keyboard shortcuts occasionally fail | 2025-12-26 | - | - | - | Skip (Old) | 2019 vague issue |
| [#7034](https://gitlab.com/gnachman/iterm2/-/issues/7034) | Inconsistent behaviour of Alt + Mouse DoubleClick | 2025-12-26 | - | - | - | Skip (Old) | 2019 issue |
| [#7001](https://gitlab.com/gnachman/iterm2/-/issues/7001) | Mark shortcuts not working | 2025-12-26 | - | - | - | Skip (Old) | 2019 marks issue |
| [#6967](https://gitlab.com/gnachman/iterm2/-/issues/6967) | [Feature Request] Add option to automatically install nig... | - | - | - | - | - | Skip | Feature request |
| [#6879](https://gitlab.com/gnachman/iterm2/-/issues/6879) | Feature Request: Allow multiple shortcuts for Semantic Hi... | - | - | - | - | - | Skip | Feature request |
| [#6867](https://gitlab.com/gnachman/iterm2/-/issues/6867) | Where are the shortcuts documented? | - | - | - | - | - | Skip | Feature request |
| [#6861](https://gitlab.com/gnachman/iterm2/-/issues/6861) | Question: is it possible to have custom handler of cmd-cl... | - | - | - | - | - | Skip | Feature request |
| [#6837](https://gitlab.com/gnachman/iterm2/-/issues/6837) | Feature Request - Global option to disable sounds / bell | - | - | - | - | - | Skip | Feature request |
| [#6774](https://gitlab.com/gnachman/iterm2/-/issues/6774) | Feature Request: Option to open X number of instances of ... | - | - | - | - | - | Skip | Feature request |
| [#6768](https://gitlab.com/gnachman/iterm2/-/issues/6768) | Feature Request: Option to open X number of instances of ... | - | - | - | - | - | Skip | Feature request |
| [#6734](https://gitlab.com/gnachman/iterm2/-/issues/6734) | Request: Steal keyfocus when inactive only if cursor idle... | - | - | - | - | - | Skip | Feature request |
| [#6704](https://gitlab.com/gnachman/iterm2/-/issues/6704) | key repeat stop printing until you release the key | 2025-12-26 | - | - | - | Skip (Old) | 2018 issue |
| [#6661](https://gitlab.com/gnachman/iterm2/-/issues/6661) | Profile shortcut key Ctrl-Command-D does not work (other ... | 2025-12-26 | - | - | - | Skip (Old) | 2018 issue |
| [#6636](https://gitlab.com/gnachman/iterm2/-/issues/6636) | Command + Ctrl on same key | 2025-12-26 | - | - | - | Skip (Old) | 2018 issue |
| [#6569](https://gitlab.com/gnachman/iterm2/-/issues/6569) | New Logitech keyboard K380 function key not correctly wor... | 2025-12-26 | - | - | - | Skip (Old) | 2018 Logitech issue |
| [#6549](https://gitlab.com/gnachman/iterm2/-/issues/6549) | Right/Left command key flipped in iterm2 when using them ... | 2025-12-26 | - | - | - | Skip (Old) | 2018 issue |
| [#6543](https://gitlab.com/gnachman/iterm2/-/issues/6543) | Feature Request: a shortcut and Touch Bar button to toggl... | - | - | - | - | - | Skip | Feature request |
| [#6520](https://gitlab.com/gnachman/iterm2/-/issues/6520) | [FR] "Show as formatted JSON" option in the context menu | - | - | - | - | - | Skip | Feature request |
| [#6510](https://gitlab.com/gnachman/iterm2/-/issues/6510) | Only show functional keys with specified labels in TouchBar | - | - | - | - | - | Skip | Feature request |
| [#6418](https://gitlab.com/gnachman/iterm2/-/issues/6418) | [Feature request] Provide an option for imgcat to blend i... | - | - | - | - | - | Skip | Feature request |
| [#6389](https://gitlab.com/gnachman/iterm2/-/issues/6389) | Add option to make underline cursor more slim | - | - | - | - | - | Skip | Feature request |
| [#6326](https://gitlab.com/gnachman/iterm2/-/issues/6326) | Feature request: Password Manager password input from con... | - | - | - | - | - | Skip | Feature request |
| [#6297](https://gitlab.com/gnachman/iterm2/-/issues/6297) | Korean Input Error | 2025-12-26 | - | - | - | Skip (Old) | 2018 Korean input issue |
| [#6295](https://gitlab.com/gnachman/iterm2/-/issues/6295) | The newest item hijacks the cmd-w key, most annoying beca... | 2025-12-26 | - | - | - | Skip (Old) | 2018 cmd-w issue |
| [#6252](https://gitlab.com/gnachman/iterm2/-/issues/6252) | Where can I access commands found in cmd+shift+O | - | - | - | - | - | Skip | Feature request |
| [#6134](https://gitlab.com/gnachman/iterm2/-/issues/6134) | The new key icon displayed at the shell's password prompt... | 2025-12-26 | - | - | - | Skip (Old) | 2018 issue |
| [#6119](https://gitlab.com/gnachman/iterm2/-/issues/6119) | Keyboard customize utility 'cmd-eikana' does not work wit... | 2025-12-26 | - | - | - | Skip (Old) | 2018 cmd-eikana |
| [#6118](https://gitlab.com/gnachman/iterm2/-/issues/6118) | After upgrade to 3.1.2 Escape Key on external Apple Wirel... | 2025-12-26 | - | - | - | Skip (Old) | 2018 3.1.2 issue |
| [#6099](https://gitlab.com/gnachman/iterm2/-/issues/6099) | Feature request: open quickly (Cmd_Shift_O) to show a few... | - | - | - | - | - | Skip | Feature request |
| [#6000](https://gitlab.com/gnachman/iterm2/-/issues/6000) | Feature Request: Allow moving cursor with mouse without h... | - | - | - | - | - | Skip | Feature request |
| [#5986](https://gitlab.com/gnachman/iterm2/-/issues/5986) | Feature Request:  Support invoking right-click context me... | - | - | - | - | - | Skip | Feature request |
| [#5966](https://gitlab.com/gnachman/iterm2/-/issues/5966) | add flare option for raising bugs | - | - | - | - | - | Skip | Feature request |
| [#5965](https://gitlab.com/gnachman/iterm2/-/issues/5965) | semantic history not triggered on cmd-click and wrong sma... | 2025-12-26 | - | - | - | Skip (Old) | 2018 semantic history |
| [#5955](https://gitlab.com/gnachman/iterm2/-/issues/5955) | Double tap cmd opens all other profiles? | 2025-12-26 | - | - | - | Skip (Old) | 2018 double tap issue |
| [#5895](https://gitlab.com/gnachman/iterm2/-/issues/5895) | Alphanum keys stop working after a short while | 2025-12-26 | - | - | - | Skip (Old) | 2018 key freeze |
| [#5885](https://gitlab.com/gnachman/iterm2/-/issues/5885) | Request: one or more option for better handling of wrappe... | - | - | - | - | - | Skip | Feature request |
| [#5872](https://gitlab.com/gnachman/iterm2/-/issues/5872) | Add descriptions to key mapings | - | - | - | - | - | Skip | Feature request |
| [#5811](https://gitlab.com/gnachman/iterm2/-/issues/5811) | 10.13 High Sierra upgrade breaks prompt, invisible input | 2025-12-26 | - | - | - | Skip (Old) | 2018 High Sierra issue |
| [#5767](https://gitlab.com/gnachman/iterm2/-/issues/5767) | Key remapping | 2025-12-26 | - | - | - | Skip (Old) | 2018 vague issue |
| [#5764](https://gitlab.com/gnachman/iterm2/-/issues/5764) | [Feature] TouchBar individual function key placement | - | - | - | - | - | Skip | Feature request |
| [#5762](https://gitlab.com/gnachman/iterm2/-/issues/5762) | Support different function key modes | - | - | - | - | - | Skip | Feature request |
| [#5732](https://gitlab.com/gnachman/iterm2/-/issues/5732) | Enter Key isn't working | 2025-12-26 | - | - | - | Skip (Old) | 2018 vague issue |
| [#5715](https://gitlab.com/gnachman/iterm2/-/issues/5715) | iTerm seems to be unable to take dictation inputs. | 2025-12-26 | - | - | - | Skip (Old) | 2018 dictation issue |
| [#5644](https://gitlab.com/gnachman/iterm2/-/issues/5644) | [Feature] Remove some functional keys from TouchBar | - | - | - | - | - | Skip | Feature request |
| [#5567](https://gitlab.com/gnachman/iterm2/-/issues/5567) | [Feature] Ability to set labels for Touch Bar items other... | - | - | - | - | - | Skip | Feature request |
| [#5549](https://gitlab.com/gnachman/iterm2/-/issues/5549) | Update sparkle to integrate PR that disables the key equi... | 2025-12-26 | - | - | - | Skip (Old) | 2018 Sparkle issue |
| [#5492](https://gitlab.com/gnachman/iterm2/-/issues/5492) | Custom Input Source with Ctrl modifier is broken | 2025-12-26 | - | - | - | Skip (Old) | 2018 input source issue |
| [#5435](https://gitlab.com/gnachman/iterm2/-/issues/5435) | Keyboard shortcuts to profiles don't work | 2025-12-26 | - | - | - | Skip (Old) | 2018 shortcuts issue |
| [#5407](https://gitlab.com/gnachman/iterm2/-/issues/5407) | Cannot disable Secure Keyboard Entry permanently | 2025-12-26 | - | - | - | Skip (Old) | 2018 secure keyboard |
| [#5405](https://gitlab.com/gnachman/iterm2/-/issues/5405) | DashTerm2 with hotkeys enabled does not restart properly ... | 2025-12-26 | - | - | - | Skip (Old) | 2017 hotkey restart |
| [#5383](https://gitlab.com/gnachman/iterm2/-/issues/5383) | DashTerm2 Hotkey Passthrough for Apple Remote Desktop | 2025-12-26 | - | - | - | Skip (Old) | 2017 ARD issue |
| [#5377](https://gitlab.com/gnachman/iterm2/-/issues/5377) | Support xterm's `modifyOtherKeys` option for keyboard input | 2025-12-26 | - | - | - | Skip | Feature request |
| [#5339](https://gitlab.com/gnachman/iterm2/-/issues/5339) | Terminal opened with a live tail command on a file closin... | 2025-12-26 | - | - | - | Skip (Old) | 2017 issue |
| [#5310](https://gitlab.com/gnachman/iterm2/-/issues/5310) | Please add option to have cmd-click behave the same as op... | - | - | - | - | - | Skip | Feature request |
| [#5283](https://gitlab.com/gnachman/iterm2/-/issues/5283) | Messes up mouse options | 2025-12-26 | - | - | - | Skip (Old) | 2017 vague issue |
| [#5265](https://gitlab.com/gnachman/iterm2/-/issues/5265) | Allow Toggling Default Profile from Keyboard/Menu Bar Sho... | - | - | - | - | - | Skip | Feature request |
| [#5243](https://gitlab.com/gnachman/iterm2/-/issues/5243) | Feature Request - Address book (possibly import option fr... | - | - | - | - | - | Skip | Feature request |
| [#5216](https://gitlab.com/gnachman/iterm2/-/issues/5216) | ctrl-c stops being passed to the shell | 2025-12-26 | - | - | - | Skip (Old) | 2016 Ctrl+C issue |
| [#5199](https://gitlab.com/gnachman/iterm2/-/issues/5199) | [Feature] macOS Sierra input source with other language | - | - | - | - | - | Skip | Feature request |
| [#5198](https://gitlab.com/gnachman/iterm2/-/issues/5198) | Build 3.0.20160918-nightly - need to overwrite the built ... | 2025-12-26 | - | - | - | Skip (Old) | 2016 build issue |
| [#5173](https://gitlab.com/gnachman/iterm2/-/issues/5173) | DashTerm2 silently shadows (unused) global hotkeys - cann... | 2025-12-26 | - | - | - | Skip (Old) | 2016 hotkey issue |
| [#5162](https://gitlab.com/gnachman/iterm2/-/issues/5162) | Docs improvement: Format keyboard shortcuts differently | - | - | - | - | - | Skip | Feature request |
| [#5112](https://gitlab.com/gnachman/iterm2/-/issues/5112) | Need two additional options for pasting with trackpad | - | - | - | - | - | Skip | Feature request |
| [#5009](https://gitlab.com/gnachman/iterm2/-/issues/5009) | Feature request: option for system-initiated shutdown to ... | - | - | - | - | - | Skip | Feature request |
| [#5006](https://gitlab.com/gnachman/iterm2/-/issues/5006) | [Help] Is there a hot key, can locate and jump my cursor ... | - | - | - | - | - | Skip | Feature request |
| [#4911](https://gitlab.com/gnachman/iterm2/-/issues/4911) | DashTerm2 is preventing spacebar panning in Adobe Illustr... | 2025-12-26 | - | - | - | Skip (Old) | 2017 Adobe issue |
| [#4884](https://gitlab.com/gnachman/iterm2/-/issues/4884) | Autocomplete + foreign input method + Caps Lock result in... | 2025-12-26 | - | - | - | Skip (Old) | 2017 autocomplete issue |
| [#4883](https://gitlab.com/gnachman/iterm2/-/issues/4883) | Does not distinguish between Return and Keypad-Enter. | 2025-12-26 | - | - | - | Skip (Old) | 2017 keypad issue |
| [#4872](https://gitlab.com/gnachman/iterm2/-/issues/4872) | Store paste history in keychain | - | - | - | - | - | Skip | Feature request |
| [#4841](https://gitlab.com/gnachman/iterm2/-/issues/4841) | Cmd+click to run a shell command | - | - | - | - | - | Skip | Feature request |
| [#4760](https://gitlab.com/gnachman/iterm2/-/issues/4760) | Add support to define custom keys pretest? | - | - | - | - | - | Skip | Feature request |
| [#4705](https://gitlab.com/gnachman/iterm2/-/issues/4705) | Toolbar with profile shortcut desperately missing | - | - | - | - | - | Skip | Feature request |
| [#4640](https://gitlab.com/gnachman/iterm2/-/issues/4640) | Add keybinding action to send password | - | - | - | - | - | Skip | Feature request |
| [#4635](https://gitlab.com/gnachman/iterm2/-/issues/4635) | Feature request: Allow option to save timestamps on the s... | - | - | - | - | - | Skip | Feature request |
| [#4589](https://gitlab.com/gnachman/iterm2/-/issues/4589) | Idea: Selecting any word on terminal and right-clicking s... | - | - | - | - | - | Skip | Feature request |
| [#4492](https://gitlab.com/gnachman/iterm2/-/issues/4492) | Feature request: Allow option to have timestamps always on | - | - | - | - | - | Skip | Feature request |
| [#4451](https://gitlab.com/gnachman/iterm2/-/issues/4451) | ctrl-c or any combined keys are not working with synergy. | 2025-12-26 | - | - | - | Skip (Old) | 2017 Synergy issue |
| [#4431](https://gitlab.com/gnachman/iterm2/-/issues/4431) | Option for No Title Bar but keep shadow | - | - | - | - | - | Skip | Feature request |
| [#4409](https://gitlab.com/gnachman/iterm2/-/issues/4409) | Feature request: disable input and/or prevent Ctrl-C | - | - | - | - | - | Skip | Feature request |
| [#4397](https://gitlab.com/gnachman/iterm2/-/issues/4397) | Input is blocked | 2025-12-26 | - | - | - | Skip (Old) | 2017 vague issue |
| [#4303](https://gitlab.com/gnachman/iterm2/-/issues/4303) | Add option to clamp images to avoid upscaling | - | - | - | - | - | Skip | Feature request |
| [#4238](https://gitlab.com/gnachman/iterm2/-/issues/4238) | Dock stays visable when you show iterm with the system-wi... | 2025-12-26 | - | - | - | Skip (Old) | 2017 dock issue |
| [#4169](https://gitlab.com/gnachman/iterm2/-/issues/4169) | Allow multiple actions for each key combination in key ma... | - | - | - | - | - | Skip | Feature request |
| [#4146](https://gitlab.com/gnachman/iterm2/-/issues/4146) | Show / Hide Shortcut Doesn't Work Properly | 2025-12-26 | - | - | - | Skip (Old) | 2017 shortcut issue |
| [#4131](https://gitlab.com/gnachman/iterm2/-/issues/4131) | Get rid of sessionsInstance altogether and make PTYSessio... | 2025-12-26 | - | - | - | Skip | Code refactor request |
| [#4113](https://gitlab.com/gnachman/iterm2/-/issues/4113) | DashTerm2 capturing all keyboard input | 2025-12-26 | - | - | - | Skip (Old) | 2017 key capture issue |
| [#4109](https://gitlab.com/gnachman/iterm2/-/issues/4109) | Request: way to save key mappings | - | - | - | - | - | Skip | Feature request |
| [#3998](https://gitlab.com/gnachman/iterm2/-/issues/3998) | Bind control arrow keys like xterm | - | - | - | - | - | Skip | Feature request |
| [#3960](https://gitlab.com/gnachman/iterm2/-/issues/3960) | Hotkey for rectangular text selection conflicting with an... | 2025-12-26 | - | - | - | Skip (Old) | 2017 hotkey conflict |
| [#3900](https://gitlab.com/gnachman/iterm2/-/issues/3900) | Feature request: Shift for mouse selection rather than Alt | - | - | - | - | - | Skip | Feature request |
| [#3781](https://gitlab.com/gnachman/iterm2/-/issues/3781) | Using Esc and Option for Meta key are switched in behaviour. | 2025-12-26 | - | - | - | Skip (Old) | 2016 meta key issue |
| [#3759](https://gitlab.com/gnachman/iterm2/-/issues/3759) | Can't add "send keys" in keyboard shortcut keys | 2025-12-26 | - | - | - | Skip (Old) | 2016 send keys issue |
| [#3753](https://gitlab.com/gnachman/iterm2/-/issues/3753) | Request:  Make alt-arrow escape sequences default on OS X... | 2025-12-26 | - | - | - | Skip | Feature request |
| [#3670](https://gitlab.com/gnachman/iterm2/-/issues/3670) | Can't select a menu item in keys menu | 2025-12-26 | - | - | - | Skip (Old) | 2016 menu issue |
| [#3646](https://gitlab.com/gnachman/iterm2/-/issues/3646) | Option-delete removes entire line? | 2025-12-26 | - | - | - | Skip (Old) | 2016 option-delete |
| [#3578](https://gitlab.com/gnachman/iterm2/-/issues/3578) | Not able to use "compose" key in DashTerm2 | 2025-12-26 | - | - | - | Skip (Old) | 2016 compose key |
| [#3519](https://gitlab.com/gnachman/iterm2/-/issues/3519) | Add support for libtermkey, including an escape sequence ... | - | - | - | - | - | Skip | Feature request |
| [#3478](https://gitlab.com/gnachman/iterm2/-/issues/3478) | Multi-stage (multi-key) keyboard shortcuts | - | - | - | - | - | Skip | Feature request |
| [#3299](https://gitlab.com/gnachman/iterm2/-/issues/3299) | Don't allow "do not remap modifiers" as action in profile... | - | - | - | - | - | Skip | Feature request |
| [#3290](https://gitlab.com/gnachman/iterm2/-/issues/3290) | Make semantic history special text file handling optional | - | - | - | - | - | Skip | Feature request |
| [#3156](https://gitlab.com/gnachman/iterm2/-/issues/3156) | Keybindings to adjust minimum contrast | - | - | - | - | - | Skip | Feature request |
| [#3138](https://gitlab.com/gnachman/iterm2/-/issues/3138) | navigate terminal output with keyboard only | - | - | - | - | - | Skip | Feature request |
| [#2931](https://gitlab.com/gnachman/iterm2/-/issues/2931) | Save broadcast input status in Saved Arrangements | - | - | - | - | - | Skip | Feature request |
| [#2825](https://gitlab.com/gnachman/iterm2/-/issues/2825) | Add support for FinalTerm's escape codes | - | - | - | - | - | Skip | Feature request |
| [#2790](https://gitlab.com/gnachman/iterm2/-/issues/2790) | Allow multiple selection of keystrokes in prefs>keys and ... | - | - | - | - | - | Skip | Feature request |
| [#2703](https://gitlab.com/gnachman/iterm2/-/issues/2703) | option to append commands to system clipboard | - | - | - | - | - | Skip | Feature request |
| [#2635](https://gitlab.com/gnachman/iterm2/-/issues/2635) | Alternate Bell Sound? | - | - | - | - | - | Skip | Feature request |
| [#2587](https://gitlab.com/gnachman/iterm2/-/issues/2587) | Provide a keys preset to make Iterm2 work like a regular ... | - | - | - | - | - | Skip | Feature request |
| [#2464](https://gitlab.com/gnachman/iterm2/-/issues/2464) | Application cursor key mode for mouse wheel | - | - | - | - | - | Skip | Feature request |
| [#2393](https://gitlab.com/gnachman/iterm2/-/issues/2393) | Skip `Confirm "Quit DashTerm2 (Cmd+Q)" command' on shutdown | - | - | - | - | - | Skip | Feature request |
| [#2294](https://gitlab.com/gnachman/iterm2/-/issues/2294) | Add option for where in viewport 'Jump to Mark' marked li... | - | - | - | - | - | Skip | Feature request |
| [#2288](https://gitlab.com/gnachman/iterm2/-/issues/2288) | Add "HotkeyTermAnimationDuration" option field to prefere... | - | - | - | - | - | Skip | Feature request |
| [#2274](https://gitlab.com/gnachman/iterm2/-/issues/2274) | Add description to keyboard shortcut | - | - | - | - | - | Skip | Feature request |
| [#2122](https://gitlab.com/gnachman/iterm2/-/issues/2122) | Command hook for the global hotkey | - | - | - | - | - | Skip | Feature request |
| [#2012](https://gitlab.com/gnachman/iterm2/-/issues/2012) | option to disable word wrap | - | - | - | - | - | Skip | Feature request |
| [#1942](https://gitlab.com/gnachman/iterm2/-/issues/1942) | Add function buttons which define by shortcut to toolbar | - | - | - | - | - | Skip | Feature request |
| [#1865](https://gitlab.com/gnachman/iterm2/-/issues/1865) | rotate option when printing | - | - | - | - | - | Skip | Feature request |
| [#1668](https://gitlab.com/gnachman/iterm2/-/issues/1668) | Activate DashTerm2 by Ctrl+Ctrl like vizor | - | - | - | - | - | Skip | Feature request |
| [#1603](https://gitlab.com/gnachman/iterm2/-/issues/1603) | Implement Terminal.app's "close if the shell exited clean... | - | - | - | - | - | Skip | Feature request |
| [#1440](https://gitlab.com/gnachman/iterm2/-/issues/1440) | Allow import/export of key mapping sets | - | - | - | - | - | Skip | Feature request |
| [#1137](https://gitlab.com/gnachman/iterm2/-/issues/1137) | Add "Send to back" menu item and/or shortcut | - | - | - | - | - | Skip | Feature request |
| [#1116](https://gitlab.com/gnachman/iterm2/-/issues/1116) | Parse infocmp output and set key bindings | - | - | - | - | - | Skip | Feature request |
| [#634](https://gitlab.com/gnachman/iterm2/-/issues/634) | ^v and ^s broken for Dvorak-Qwerty keyboard | 2025-12-26 | - | - | - | Skip (Old) | 2015 Dvorak issue |
| [#603](https://gitlab.com/gnachman/iterm2/-/issues/603) | An option to make bookmark shortcuts global | - | - | - | - | - | Skip | Feature request |
| [#217](https://gitlab.com/gnachman/iterm2/-/issues/217) | Support all options of xtermcontrol | - | - | - | - | - | Skip | Feature request |
| [#139](https://gitlab.com/gnachman/iterm2/-/issues/139) | Feature: alternative full-screen mode ala WriteRoom | - | - | - | - | - | Skip | Feature request |

---

## Statistics

| Metric | Count |
|--------|-------|
| Total | 266 |
| Fixed | 6 |
| In Progress | 0 |
| Open | 0 |
| Skip (Feature Requests) | 141 |
| Skip (Old/Obsolete) | 98 |
| External | 5 |
| Cannot Reproduce | 5 |
| Wontfix | 0 |

---

## Category Notes

Keyboard and input issues span a wide range from modifier key handling to hotkey management to international keyboard support.

### Common Patterns

1. **Control key issues** (#12163, #11913, #10869, #8995) - Ctrl+C/D not working, SIGINT not sent, CSI u mode conflicts. Often related to terminal mode or shell integration.

2. **Hotkey/global shortcut issues** (#12404, #11857, #11129, #10850, #10316) - Hotkeys not working after fullscreen, Sonoma/Tahoe specific issues, focus problems.

3. **Option/Alt key as Meta** (#11753, #9859, #8790) - Left/right option key handling, external keyboard differences, meta key behavior.

4. **Key repeat problems** (#12404, #12415) - Key repeat stops working or has delays, often macOS version-specific.

5. **Non-English keyboard layouts** (#12525, #11730, #9481, #6297) - German, Korean, Asian input methods not working properly.

6. **Vim/Neovim compatibility** (#12363, #10029, #11806) - Meta key conflicts, CSI u mode issues, function keys intermittent.

7. **Secure Keyboard Entry** (#10795, #7715, #5407) - Cannot disable, conflicts with other apps, global hotkey issues.

8. **External keyboard behavior** (#9859, #8436, #8874) - Option key mapping differs between built-in and external keyboards.

### Related Files

- `sources/iTermKeyMapper.m` - Key mapping logic
- `sources/iTermHotKeyController.m` - Global hotkey handling
- `sources/iTermModifierRemapper.m` - Modifier key remapping
- `sources/PTYTextView+Private.m` - Text view key handling
- `sources/VT100Terminal.m` - Terminal key sequences
- `sources/iTermSecureKeyboardEntry.m` - Secure keyboard entry
- `sources/iTermKeyBindingMgr.m` - Key binding management

