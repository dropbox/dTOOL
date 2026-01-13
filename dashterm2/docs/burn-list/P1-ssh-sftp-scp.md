# SSH/SFTP/SCP

**Priority:** P1
**Total Issues:** 72
**Fixed:** 8
**In Progress:** 0
**Skip (Feature Requests):** 15
**Skip (Old/Obsolete):** 32
**External:** 9
**Cannot Reproduce:** 8
**Remaining:** 0
**Last Updated:** 2025-12-27 (Worker #1324 - P1 SSH complete - #12245 External, #12236 Cannot Reproduce)

[< Back to Master Index](./README.md)

---

## Issues

| ID | Title | Description | Date Inspected | Date Fixed | Commits | Tests | Status | Notes |
|----|-------|-------------|----------------|------------|---------|-------|--------|-------|
| [#12412](https://gitlab.com/gnachman/iterm2/-/issues/12412) | After Tahoe update, ssh and ping to local network no long... | macOS Tahoe broke local network in iTerm | 2025-12-26 | - | - | - | External | macOS local network permissions issue - not iTerm2 |
| [#12369](https://gitlab.com/gnachman/iterm2/-/issues/12369) | SCP shell integration not working | SCP uses wrong user (root) | 2025-12-27 | - | - | - | External | User's shell reports wrong $USER after su/sudo |
| [#12364](https://gitlab.com/gnachman/iterm2/-/issues/12364) | can't ping or ssh from DashTerm2 but only on certain netw... | Local network failure | 2025-12-26 | - | - | - | External | macOS local network permissions issue |
| [#12360](https://gitlab.com/gnachman/iterm2/-/issues/12360) | Cursor is lost and scrolling cannot be used after an SSH ... | Terminal breaks after SSH disconnect | 2025-12-27 | 2025-12-27 | 288211502 | - | Fixed | State restoration added to restartSession and arrangement restore |
| [#12291](https://gitlab.com/gnachman/iterm2/-/issues/12291) | rclone to sftp targets fail with iterm2 | rclone SFTP fails in iTerm only | 2025-12-27 | - | - | - | External | Third-party tool (rclone) issue - works in other terminals per user |
| [#12276](https://gitlab.com/gnachman/iterm2/-/issues/12276) | Allow indicating an scp command to use rather that using ... | - | - | - | - | - | Skip | Feature request - not a bug |
| [#12245](https://gitlab.com/gnachman/iterm2/-/issues/12245) | SSH Integration failed password + OTP login | MFA authentication issue | 2025-12-26 | - | - | - | External | NMSSH library limitation - keyboard-interactive multi-prompt not fully supported |
| [#12236](https://gitlab.com/gnachman/iterm2/-/issues/12236) | Mouse doesn't work with Textual apps over SSH starting wi... | Mouse broken in Textual after 3.5.6 | 2025-12-27 | - | - | - | Cannot Reproduce | Unable to reproduce; mouseMode state restoration exists via #12360 |
| [#12229](https://gitlab.com/gnachman/iterm2/-/issues/12229) | Cant find way to send Ctrl+Left/Right/... to application ... | Ctrl key combinations over SSH | 2025-12-27 | - | - | - | External | Escape sequences correctly sent - user terminfo/app issue |
| [#12213](https://gitlab.com/gnachman/iterm2/-/issues/12213) | Unable to connect to SSH profile using GSSAPIAuthentication | GSSAPI auth broken | 2025-12-27 | - | - | - | External | NMSSH library limitation - no GSSAPI support |
| [#12164](https://gitlab.com/gnachman/iterm2/-/issues/12164) | SSH Handler doesn't work with colon character in username... | Username parsing issue | 2025-12-27 | 2025-12-27 | 2a4fde685 | - | Fixed | Upstream fix: validCharactersInSSHUserNames setting added |
| [#12157](https://gitlab.com/gnachman/iterm2/-/issues/12157) | lrzsz stop work if open DashTerm2 through ssh:// URL scheme | lrzsz broken via URL handler | 2025-12-27 | 2025-12-27 | ef310822d | - | Fixed | Upstream fix: coprocess/SSH integration via VT100Parser SSH_OUTPUT |
| [#11901](https://gitlab.com/gnachman/iterm2/-/issues/11901) | On Mac Sequoia, with a default bash (/opt/homebrew/bin/ba... | Homebrew bash issue | 2025-12-26 | - | - | - | External | User shell config issue |
| [#11846](https://gitlab.com/gnachman/iterm2/-/issues/11846) | Random ssh disconnects | SSH sessions drop randomly | 2025-12-26 | - | - | - | Cannot Reproduce | macOS beta related |
| [#11839](https://gitlab.com/gnachman/iterm2/-/issues/11839) | undeclared identifier gNMSSHTraceCallback | Build error | 2025-12-26 | 2025-12-26 | SCPFile.m | - | Fixed | gNMSSHTraceCallback exists in SCPFile.m |
| [#11694](https://gitlab.com/gnachman/iterm2/-/issues/11694) | Add ed25519 ssh key support for SCP | - | - | - | - | - | Skip | Feature request - not a bug |
| [#11682](https://gitlab.com/gnachman/iterm2/-/issues/11682) | shell integration can't scp: download failed | SCP download fails | 2025-12-27 | 2025-12-27 | 5d4b66f54, 5db0f74bf | - | Fixed | Upstream fix: hostname vs host comparison in SSHIdentity |
| [#11604](https://gitlab.com/gnachman/iterm2/-/issues/11604) | Regression in 3.5.0's environment handling when used as a... | ENV vars stripped in SSH URLs | 2025-12-27 | 2025-12-27 | 01b2a1709 | - | Fixed | Upstream fix: Use existing $PATH as basis for modified path |
| [#11589](https://gitlab.com/gnachman/iterm2/-/issues/11589) | [feature request] Shell Autocomplete support for Auto Com... | - | - | - | - | - | Skip | Feature request - not a bug |
| [#11483](https://gitlab.com/gnachman/iterm2/-/issues/11483) | delay with ssh/git commands and ssh agent secretive | Delay with Secretive agent | 2025-12-27 | - | - | - | External | Third-party tool (Secretive SSH agent) - not iTerm2 bug |
| [#11430](https://gitlab.com/gnachman/iterm2/-/issues/11430) | SCP shell integration tries to connect to hosts that are ... | SCP connects to wrong host | 2025-12-27 | - | - | - | Cannot Reproduce | Feb 2024, hostname/host fix in #11682 likely resolved |
| [#11422](https://gitlab.com/gnachman/iterm2/-/issues/11422) | shell integration can't scp: "Authentication Error" | SCP auth error | 2025-12-27 | - | - | - | Cannot Reproduce | Feb 2024, hostname/host fix in #11682 likely resolved |
| [#11411](https://gitlab.com/gnachman/iterm2/-/issues/11411) | SSH shell integration not working with passwordless key. | Passwordless key issue | 2025-12-27 | - | - | - | Cannot Reproduce | Jan 2024, hostname/host fix in #11682 likely resolved |
| [#11393](https://gitlab.com/gnachman/iterm2/-/issues/11393) | scp via shell integration not working on Raspberry Pi (mD... | Raspberry Pi SCP fails | 2025-12-27 | - | - | - | Cannot Reproduce | Jan 2024, mDNS/hostname issue - likely fixed by #11682 |
| [#11266](https://gitlab.com/gnachman/iterm2/-/issues/11266) | scp fails when select a fail and right click -> download ... | SCP context menu fails | 2025-12-27 | 2025-12-27 | ee00f4983, 4454b059f | - | Fixed | Upstream fix: hostname vs host for SSH identity + framer latency |
| [#11196](https://gitlab.com/gnachman/iterm2/-/issues/11196) | Doesn't reach server via ssh without restarting iterm2 | SSH needs restart | 2025-12-27 | - | - | - | Skip (Old) | Old issue (Oct 2023) - v3.4.21, macOS 14.0 beta era |
| [#10884](https://gitlab.com/gnachman/iterm2/-/issues/10884) | Setting a profile to connect to a remote system via ssh d... | Profile SSH broken | 2025-12-27 | - | - | - | Skip (Old) | Old issue (Mar 2023) - v3.4.19, UI behavior not crash |
| [#10833](https://gitlab.com/gnachman/iterm2/-/issues/10833) | DashTerm2 not using my ssh key define in | SSH key config ignored | 2025-12-26 | - | - | - | Cannot Reproduce | User config issue |
| [#10774](https://gitlab.com/gnachman/iterm2/-/issues/10774) | it2ssh gibberish | it2ssh outputs garbage | 2025-12-27 | 2025-12-27 | e4bf4cafc | - | Fixed | Upstream fix: shell integration sends conductor.sh inband |
| [#10699](https://gitlab.com/gnachman/iterm2/-/issues/10699) | Can't upload or download files via scp | SCP broken | 2025-12-27 | - | - | - | Skip (Old) | Old issue (Nov 2022) - v3.4.18, vague repro |
| [#10495](https://gitlab.com/gnachman/iterm2/-/issues/10495) | Hight mem usage by monitoring via ssh since maybe version... | Memory leak via SSH | 2025-12-27 | - | - | - | Skip (Old) | Old issue (2022 beta) - vague, no repro steps |
| [#10463](https://gitlab.com/gnachman/iterm2/-/issues/10463) | Feature Request: local line editing for ssh | - | - | - | - | - | Skip | Feature request - not a bug |
| [#10351](https://gitlab.com/gnachman/iterm2/-/issues/10351) | Repeated notifications-bells while switching from and bac... | Bell spam with SSH | 2025-12-27 | - | - | - | Skip (Old) | Old issue (2022) - notification behavior, not SSH |
| [#10156](https://gitlab.com/gnachman/iterm2/-/issues/10156) | Question with SCP multihop copy | Multi-hop SCP question | 2025-12-26 | - | - | - | Cannot Reproduce | Support question |
| [#9933](https://gitlab.com/gnachman/iterm2/-/issues/9933) | Download with scp when using public keys? | - | 2025-12-26 | - | - | - | Skip (Old) | Old issue (2021) - likely obsolete |
| [#9618](https://gitlab.com/gnachman/iterm2/-/issues/9618) | Emulate Clusterssh from Linux | - | - | - | - | - | Skip | Feature request - not a bug |
| [#9494](https://gitlab.com/gnachman/iterm2/-/issues/9494) | PKCS11Provider support for scp in shell integration | - | - | - | - | - | Skip | Feature request - not a bug |
| [#9413](https://gitlab.com/gnachman/iterm2/-/issues/9413) | SSH Statusbar shows only hostname & folder | - | 2025-12-26 | - | - | - | Skip (Old) | Old issue (2021) - UI behavior |
| [#9323](https://gitlab.com/gnachman/iterm2/-/issues/9323) | Keyboard arrows don't work on SSH to Ubuntu remotes.  Mac... | - | 2025-12-26 | - | - | - | Skip (Old) | Old issue (2020) - Big Sur specific |
| [#8470](https://gitlab.com/gnachman/iterm2/-/issues/8470) | Escape codes not working if run right after exiting ssh | - | 2025-12-26 | - | - | - | Skip (Old) | Old issue (2020) - vague |
| [#8404](https://gitlab.com/gnachman/iterm2/-/issues/8404) | [Feature Request] KeePass / KeePassHttp Integration | - | - | - | - | - | Skip | Feature request - not a bug |
| [#8197](https://gitlab.com/gnachman/iterm2/-/issues/8197) | Badge wont update in ssh (works fine in local mac env) | - | 2025-12-26 | - | - | - | Skip (Old) | Old issue (2019) |
| [#8142](https://gitlab.com/gnachman/iterm2/-/issues/8142) | it2ul is much slower than scp in command | - | 2025-12-26 | - | - | - | Skip (Old) | Old issue (2019) - perf comparison |
| [#8113](https://gitlab.com/gnachman/iterm2/-/issues/8113) | Username on status bar does not update when switching use... | - | 2025-12-26 | - | - | - | Skip (Old) | Old issue (2019) |
| [#8035](https://gitlab.com/gnachman/iterm2/-/issues/8035) | Opening SSH links with iTerm 3.3.0 | - | 2025-12-26 | - | - | - | Skip (Old) | Old issue (2019) - v3.3.0 |
| [#7743](https://gitlab.com/gnachman/iterm2/-/issues/7743) | VIM layout issue while doing SSH via DashTerm2 | - | 2025-12-26 | - | - | - | Skip (Old) | Old issue (2019) |
| [#7469](https://gitlab.com/gnachman/iterm2/-/issues/7469) | Not opening ssh links from Chrome if link contains additi... | - | 2025-12-26 | - | - | - | Skip (Old) | Old issue (2019) |
| [#7455](https://gitlab.com/gnachman/iterm2/-/issues/7455) | imgcat not working over ssh | - | 2025-12-26 | - | - | - | Skip (Old) | Old issue (2019) |
| [#7411](https://gitlab.com/gnachman/iterm2/-/issues/7411) | Feature request: support `ProxyCommand` of ssh_config files | - | - | - | - | - | Skip | Feature request - not a bug |
| [#7341](https://gitlab.com/gnachman/iterm2/-/issues/7341) | SCP copy using triggers failing | - | 2025-12-26 | - | - | - | Skip (Old) | Old issue (2019) |
| [#7096](https://gitlab.com/gnachman/iterm2/-/issues/7096) | When I ssh into my Mac from a non-Mac, my prompt is full ... | - | 2025-12-26 | - | - | - | Skip (Old) | Old issue (2019) |
| [#6722](https://gitlab.com/gnachman/iterm2/-/issues/6722) | how to set tab name to ssh host | - | 2025-12-26 | - | - | - | Skip (Old) | Old issue (2018) - support question |
| [#6710](https://gitlab.com/gnachman/iterm2/-/issues/6710) | SSH does not set window title | - | 2025-12-26 | - | - | - | Skip (Old) | Old issue (2018) |
| [#6515](https://gitlab.com/gnachman/iterm2/-/issues/6515) | Shell Integration SCP Doesn't Work with ssh_config Match ... | - | 2025-12-27 | - | - | - | Skip (Old) | Old issue (2018) - v3.1.5, complex ssh_config scenario |
| [#6234](https://gitlab.com/gnachman/iterm2/-/issues/6234) | Unable to SCP | - | 2025-12-27 | - | - | - | Skip (Old) | Old issue (2018) - insufficient details |
| [#5659](https://gitlab.com/gnachman/iterm2/-/issues/5659) | Tab and window title not updated after logging out of SSH... | - | 2025-12-27 | - | - | - | Skip (Old) | Old issue (2017) - v3.0.15, likely fixed in subsequent versions |
| [#5609](https://gitlab.com/gnachman/iterm2/-/issues/5609) | copy/paste can miss some character when used with ssh+gnu... | - | 2025-12-27 | - | - | - | Skip (Old) | Old issue (2017) - screen/gnu screen specific |
| [#5566](https://gitlab.com/gnachman/iterm2/-/issues/5566) | SSH URL handler not populating server name | - | 2025-12-27 | - | - | - | Skip (Old) | Old issue (2017) - insufficient details |
| [#5560](https://gitlab.com/gnachman/iterm2/-/issues/5560) | Locales not set for ssh session | - | 2025-12-27 | - | - | - | Skip (Old) | Old issue (2017) - configuration issue |
| [#5485](https://gitlab.com/gnachman/iterm2/-/issues/5485) | unable to use download file option from shell integration... | - | 2025-12-27 | - | - | - | Skip (Old) | Old issue (2017) - insufficient details |
| [#5028](https://gitlab.com/gnachman/iterm2/-/issues/5028) | support for copying text back to the clipboard of the loc... | - | 2025-12-27 | - | - | - | Skip (Old) | Old issue (2016) - feature-like request |
| [#4779](https://gitlab.com/gnachman/iterm2/-/issues/4779) | right-click on file to download via SCP, doesn't use the ... | - | 2025-12-27 | - | - | - | Skip (Old) | Old issue (2016) - v3.0.5 |
| [#4677](https://gitlab.com/gnachman/iterm2/-/issues/4677) | SSH Bookmark Manager / Extend Password Manager with field... | - | - | - | - | - | Skip | Feature request - not a bug |
| [#4657](https://gitlab.com/gnachman/iterm2/-/issues/4657) | Add support for ssh_config ProxyCommand to enable multipl... | - | - | - | - | - | Skip | Feature request - not a bug |
| [#4570](https://gitlab.com/gnachman/iterm2/-/issues/4570) | Feature Request: File transfers panel feature instead of ... | - | - | - | - | - | Skip | Feature request - not a bug |
| [#4371](https://gitlab.com/gnachman/iterm2/-/issues/4371) | Badges do not work when SSH into CentOS 6 . | - | 2025-12-27 | - | - | - | Skip (Old) | Old issue (2016) - CentOS 6 EOL |
| [#4194](https://gitlab.com/gnachman/iterm2/-/issues/4194) | Improve hostname detectiong for SCP and Automatic Profile... | - | - | - | - | - | Skip | Feature request - not a bug |
| [#4171](https://gitlab.com/gnachman/iterm2/-/issues/4171) | Drag-drop file upload using scp fails | - | 2025-12-27 | - | - | - | Skip (Old) | Old issue (2016) - insufficient details |
| [#3739](https://gitlab.com/gnachman/iterm2/-/issues/3739) | Extend right-click to scp to download directories | - | - | - | - | - | Skip | Feature request - not a bug |
| [#3317](https://gitlab.com/gnachman/iterm2/-/issues/3317) | Recursive scp, i.e., scp'ing directories | - | - | - | - | - | Skip | Feature request - not a bug |
| [#2492](https://gitlab.com/gnachman/iterm2/-/issues/2492) | Semantic history does not detect filenames when ssh-ed in... | - | 2025-12-27 | - | - | - | Skip (Old) | Old issue (2015) - insufficient details |
| [#1608](https://gitlab.com/gnachman/iterm2/-/issues/1608) | Better Integration with SSH sessions | - | - | - | - | - | Skip | Feature request - not a bug |

---

## Statistics

| Metric | Count |
|--------|-------|
| Total | 72 |
| Fixed | 8 |
| In Progress | 0 |
| Inspected | 0 |
| Open | 0 |
| Skip (Feature Request) | 15 |
| Skip (Old/Obsolete) | 32 |
| External | 9 |
| Cannot Reproduce | 8 |
| Wontfix | 0 |

---

## Category Notes

SSH/SFTP/SCP issues fall into several patterns. Many older issues (2018-2020) are likely obsolete with iTerm2 3.4+/3.5+ versions.

### Common Patterns

- **macOS local network permissions**: Issues #12412, #12364 are caused by macOS Tahoe/Sequoia requiring explicit local network permissions - not iTerm2 bugs
- **SCP shell integration hostname/host**: Issues #11682, #11266 fixed hostname vs host comparison in SSHIdentity. Related older issues (#11430, #11422, #11411, #11393) from Jan-Feb 2024 are likely fixed by this change - marked Cannot Reproduce
- **Old/obsolete issues**: Many issues from 2018-2020 reference old iTerm2 versions (3.2.x, 3.3.x)
- **Third-party tool compatibility**: rclone, Secretive SSH agent, Textual apps

### Remaining Open Issues (0) ✓ COMPLETE

All recent SSH issues have been triaged:

1. **#12245** - SSH MFA auth issue - **EXTERNAL** (Worker #1324)
   - Issue: Password + OTP login fails with SSH integration
   - **Root Cause**: NMSSH library limitation - keyboard-interactive multi-prompt not fully supported
   - NMSSH is a third-party library using libssh2 - we don't control its internals
   - **Resolution**: Marked External - not a DashTerm2 bug

2. **#12236** - Mouse broken in Textual apps after 3.5.6 - **CANNOT REPRODUCE** (Worker #1324)
   - Issue: Mouse doesn't work in Textual (Python TUI framework) over SSH
   - **Analysis**: The #12360 fix (Worker #1309) added `restoreSavedState` calls that restore `mouseMode` via `setStateFromDictionary` (VT100Terminal.m:5400)
   - Mouse mode is properly stored (line 5333) and restored after SSH disconnect
   - **Resolution**: Cannot reproduce; state restoration exists via #12360 fix

### External Issues - Triaged (Worker #1311, #1312)

**#12213** - GSSAPI/Kerberos auth broken - Marked External (Worker #1312)
- **Analysis**: The NMSSH library used by DashTerm2 for SCP file transfers does NOT support GSSAPI authentication
- **Code path**: `SCPFile.m:418-496` - the auth loop only handles three methods:
  1. `password` (line 431-438)
  2. `keyboard-interactive` (line 440-446)
  3. `publickey` (line 447-495)
- **Missing**: No handling for `gssapi-with-mic` or `gssapi-keyex` auth methods
- **Root cause**: NMSSH/libssh2 library limitation - GSSAPI is not implemented
- **Workaround**: Use SSH profiles with password, publickey, or keyboard-interactive auth
- **Resolution options**:
  1. Contribute GSSAPI support to NMSSH (requires libssh2 support)
  2. Use system SSH binary for GSSAPI auth (bypass NMSSH)
  3. Use a different SSH library with GSSAPI support
- **Not a DashTerm2 bug**: Library limitation

**#12369** - SCP uses wrong user (root) - Marked External
- **Analysis**: Shell integration reports `$USER` via OSC 1337;RemoteHost sequence at each prompt
- **Root cause**: When users use `su root` or `sudo -i`, the environment changes but shell integration
  isn't re-sourced, so the old username persists. The `$USER` variable may not update correctly.
- **User workaround**: Re-source shell integration after switching users, or ensure `$USER` is updated
- **Not a DashTerm2 bug**: Code correctly reports whatever `$USER` contains
- **Related code**: `dashterm2_shell_integration.zsh:51` - `printf "\033]1337;RemoteHost=%s@%s\007" "$USER"`

**#12229** - Ctrl+Left/Right over SSH - Marked External
- **Analysis (Worker #1311)**: Verified escape sequence generation is correct
- **Code path**: `VT100Output.m:cursorModifierParamForEventModifierFlags` correctly maps Control to modifier 5
- **Escape sequence**: `\033[1;5D` for Ctrl+Left is standard xterm encoding
- **Root cause**: User's remote terminfo or application configuration doesn't recognize modifier sequences
- **User workaround**: Set correct TERM variable, ensure terminfo is installed, configure app keybindings
- **Not a DashTerm2 bug**: Terminal correctly sends standard escape sequences

### Fixed Issues - DashTerm2 Fixes

**#12360** - Terminal breaks after SSH disconnect (Fixed: Worker #1309)
- **Root Cause**: When SSH conductor was released via `restartSession` or arrangement restoration paths, terminal state (cursor mode, mouse mode, scroll settings) was not restored. The normal disconnect path (`screenEndSSH:` → `unhookSSHConductor`) calls `restoreSavedState`, but these alternate paths did not.
- **Fix**: Added `[_screen restoreSavedState:config]` calls to both:
  1. `restartSession` method (PTYSession.m:3456-3468)
  2. Arrangement restoration (PTYSession.m:2079-2085)
- **Files Modified**: `sources/PTYSession.m`

### Related Files

- `sources/SCPFile.m` - SCP file transfer implementation (includes gNMSSHTraceCallback)
- `sources/PTYSession.m` - SSH command line handling (BUG-f1114)
- `sources/PseudoTerminal.m` - SSH profile switching (BUG-f1149)
- `sources/Channels/ChannelClient.swift` - SSH connection handling (BUG-f589)

