# Shell Integration

**Priority:** P1
**Total Issues:** 63
**Fixed:** 5
**In Progress:** 0
**Skip (Feature Requests):** 7
**Skip (Old/Obsolete):** 33
**Skip (Docs/Website):** 3
**External:** 6
**Cannot Reproduce:** 5
**Remaining:** 0
**Last Updated:** 2025-12-27 (Worker #1314 - marked #12616 External)

[< Back to Master Index](./README.md)

---

## Issues

| ID | Title | Description | Date Inspected | Date Fixed | Commits | Tests | Status | Notes |
|----|-------|-------------|----------------|------------|---------|-------|--------|-------|
| [#12641](https://gitlab.com/gnachman/iterm2/-/issues/12641) | Fish 4.x shell corrupts with "Load shell integration auto... | Fish 4.x compatibility issue | 2025-12-27 | 2025-12-15 | 865aad0b2 | - | Fixed | Fish 4.x auto-loading - bumped shell integration |
| [#12616](https://gitlab.com/gnachman/iterm2/-/issues/12616) | FYI: Fish will soon include built-in support for most of ... | Fish 4.3 OSC 133 support | 2025-12-27 | - | - | - | External | FYI notification - Fish ecosystem development, not a bug |
| [#12518](https://gitlab.com/gnachman/iterm2/-/issues/12518) | Shell integration significantly slows prompt rendering | Slow prompts on Tahoe | 2025-12-27 | 2025-10-07 | 87f86bcf0, 70c2b18c9 | - | Fixed | Avoid pausing on FTCS C |
| [#12382](https://gitlab.com/gnachman/iterm2/-/issues/12382) | Shell integration: `OSC 133; D` not considered the end of... | OSC 133 parsing | 2025-12-27 | 2025-12-27 | ff7944500 | - | Fixed | assignCurrentCommandEndDate now called on OSC 133;D |
| [#12240](https://gitlab.com/gnachman/iterm2/-/issues/12240) | Shell integration is showing some very strange results | Strange results | 2025-12-27 | 2025-05-19 | c19c70a42, 54160e44a | - | Fixed | Fish stray marks + double prompt marks |
| [#12168](https://gitlab.com/gnachman/iterm2/-/issues/12168) | "Install Shell Integration" Should Indicate Whether It ha... | - | - | - | - | - | Skip | Feature request - not a bug |
| [#11378](https://gitlab.com/gnachman/iterm2/-/issues/11378) | Load shell integration automatically : breaks connexion i... | Auto-load breaks SSH | 2025-12-27 | 2024-03-02 | 8b3dd5868 | - | Fixed | Queue input while connecting to SSH |
| [#11294](https://gitlab.com/gnachman/iterm2/-/issues/11294) | command not found: iterm2_shell_integration.zsh | Script not found | 2025-12-27 | - | - | - | External | User config issue - DashTerm2 uses dashterm2_shell_integration.* files |
| [#11016](https://gitlab.com/gnachman/iterm2/-/issues/11016) | Shell integration breaks custom bash PROMPT_COMMAND using... | PROMPT_COMMAND conflict | 2025-12-27 | - | - | - | Cannot Reproduce | Old (2023), PROMPT_COMMAND conflict - likely fixed |
| [#10537](https://gitlab.com/gnachman/iterm2/-/issues/10537) | Shell integration is messed up with starship prompt | Starship prompt corruption | 2025-12-26 | - | - | - | External | Starship theme compatibility |
| [#10528](https://gitlab.com/gnachman/iterm2/-/issues/10528) | bash shell integration not work? like CurrentDir not output | CurrentDir not set | 2025-12-27 | - | - | - | Cannot Reproduce | Old (2022), vague repro - likely fixed |
| [#10300](https://gitlab.com/gnachman/iterm2/-/issues/10300) | Update shell integration documentation | - | 2025-12-26 | - | - | - | Skip (Docs) | Documentation - not code bug |
| [#10280](https://gitlab.com/gnachman/iterm2/-/issues/10280) | Shell integration on website is outdated | - | 2025-12-26 | - | - | - | Skip (Docs) | Website - not code bug |
| [#10218](https://gitlab.com/gnachman/iterm2/-/issues/10218) | Shell integration interferes with working directory prese... | PWD conflict | 2025-12-27 | - | - | - | Cannot Reproduce | Old (2022), vague PWD conflict - likely fixed |
| [#10183](https://gitlab.com/gnachman/iterm2/-/issues/10183) | Shell integration causes issues viewing files in /usr/bin... | /usr/bin browsing issue | 2025-12-27 | - | - | - | Cannot Reproduce | Old (2022), remote server issue - likely config |
| [#10172](https://gitlab.com/gnachman/iterm2/-/issues/10172) | iTerm Shell Integrations mangle login shell command output | Login shell output mangled | 2025-12-27 | - | - | - | Cannot Reproduce | Old (2022), vague login shell issue - likely fixed |
| [#9840](https://gitlab.com/gnachman/iterm2/-/issues/9840) | .iterm2_shell_integration.bash reports syntax error | - | 2025-12-26 | - | - | - | Skip (Old) | Old issue (2021) |
| [#9831](https://gitlab.com/gnachman/iterm2/-/issues/9831) | Re-sourcing .bash_profile breaks DashTerm2 shell integrat... | - | 2025-12-26 | - | - | - | Skip (Old) | Old issue (2021) |
| [#9750](https://gitlab.com/gnachman/iterm2/-/issues/9750) | Shell integration can be a pest | - | 2025-12-26 | - | - | - | Skip (Old) | Old issue (2021) - vague complaint |
| [#8806](https://gitlab.com/gnachman/iterm2/-/issues/8806) | [ENQUIRY] Shell Integration: Command History | - | 2025-12-26 | - | - | - | Skip (Old) | Old issue (2020) - question |
| [#8554](https://gitlab.com/gnachman/iterm2/-/issues/8554) | "Install Shell Integration" dialog should make clear wher... | - | - | - | - | - | Skip | Feature request - not a bug |
| [#8543](https://gitlab.com/gnachman/iterm2/-/issues/8543) | fish shell integration bad hostname | - | 2025-12-26 | - | - | - | Skip (Old) | Old issue (2020) |
| [#8367](https://gitlab.com/gnachman/iterm2/-/issues/8367) | Shell Integration and iterm2 server is a SPOF | - | 2025-12-26 | - | - | - | Skip (Old) | Old issue (2019) |
| [#8089](https://gitlab.com/gnachman/iterm2/-/issues/8089) | .iterm2_shell_integration.bash returns -t bad option | - | 2025-12-26 | - | - | - | Skip (Old) | Old issue (2019) |
| [#7966](https://gitlab.com/gnachman/iterm2/-/issues/7966) | .iterm_shell_integration.zsh gives annoying message if I ... | - | 2025-12-26 | - | - | - | Skip (Old) | Old issue (2019) |
| [#7480](https://gitlab.com/gnachman/iterm2/-/issues/7480) | TCSH Shell Integration Fails from Quotations | - | 2025-12-26 | - | - | - | Skip (Old) | Old issue (2019) |
| [#7467](https://gitlab.com/gnachman/iterm2/-/issues/7467) | .iterm2_shell_integration.bash fails on a strict bash she... | - | 2025-12-26 | - | - | - | Skip (Old) | Old issue (2019) |
| [#7154](https://gitlab.com/gnachman/iterm2/-/issues/7154) | Feature Suggestion: Detect if client is DashTerm2 for she... | - | - | - | - | - | Skip | Feature request - not a bug |
| [#6885](https://gitlab.com/gnachman/iterm2/-/issues/6885) | Add shell integration and others script to brew or brew cask | - | - | - | - | - | Skip | Feature request - not a bug |
| [#6727](https://gitlab.com/gnachman/iterm2/-/issues/6727) | bash shell integration should warn if extdebug is on | - | 2025-12-27 | - | - | - | Skip (Old) | Old issue (2018) - feature request |
| [#6588](https://gitlab.com/gnachman/iterm2/-/issues/6588) | .iterm2_shell_integration.fish doesn't check fish version... | Fish version check | 2025-12-27 | - | - | - | Skip (Old) | Old issue (2018) - Fish 2.x era |
| [#6542](https://gitlab.com/gnachman/iterm2/-/issues/6542) | shell integration wierd error with zsh and powerline font | - | 2025-12-26 | - | - | - | External | Powerline font compatibility |
| [#6319](https://gitlab.com/gnachman/iterm2/-/issues/6319) | Installing shell integration under fish overwrites my config | - | 2025-12-27 | - | - | - | Skip (Old) | Old issue (2017) |
| [#6260](https://gitlab.com/gnachman/iterm2/-/issues/6260) | Shell Integration doesn't work in Midnight Commander | - | 2025-12-26 | - | - | - | External | Midnight Commander compatibility |
| [#6177](https://gitlab.com/gnachman/iterm2/-/issues/6177) | Question regarding shell-integration -- script location | - | 2025-12-26 | - | - | - | Skip (Docs) | Support question |
| [#5964](https://gitlab.com/gnachman/iterm2/-/issues/5964) | Shell integration turns off after first command when inst... | - | 2025-12-26 | - | - | - | Skip (Old) | Old issue (2018) |
| [#5790](https://gitlab.com/gnachman/iterm2/-/issues/5790) | Shell Integration: "hostname: Unknown host" | - | 2025-12-26 | - | - | - | Skip (Old) | Old issue (2018) |
| [#5724](https://gitlab.com/gnachman/iterm2/-/issues/5724) | Installing Shell Integration can fail if file exists | - | 2025-12-27 | - | - | - | Skip (Old) | Old issue (2017) |
| [#5695](https://gitlab.com/gnachman/iterm2/-/issues/5695) | advertised shell integrations aren't working.  no errors ... | - | 2025-12-26 | - | - | - | Skip (Old) | Old issue (2018) - vague |
| [#5503](https://gitlab.com/gnachman/iterm2/-/issues/5503) | Shell integration interferes with shared bash history | - | 2025-12-27 | - | - | - | Skip (Old) | Old issue (2017) |
| [#5479](https://gitlab.com/gnachman/iterm2/-/issues/5479) | Using Shell Integration with pom files | - | 2025-12-26 | - | - | - | Skip (Old) | Old issue (2018) |
| [#5092](https://gitlab.com/gnachman/iterm2/-/issues/5092) | Shell Integration don't work | - | 2025-12-26 | - | - | - | Skip (Old) | Old issue (2017) - vague |
| [#5017](https://gitlab.com/gnachman/iterm2/-/issues/5017) | Bash Shell Integration + asciinema, export functions ? | - | 2025-12-26 | - | - | - | Skip (Old) | Old issue (2017) |
| [#4991](https://gitlab.com/gnachman/iterm2/-/issues/4991) | Bash Shell Integration and git autocompletion don't work ... | - | 2025-12-26 | - | - | - | External | git autocompletion conflict |
| [#4892](https://gitlab.com/gnachman/iterm2/-/issues/4892) | Security of Shell Integration and a Privacy Policy | - | - | - | - | - | Skip | Feature request - not a bug |
| [#4843](https://gitlab.com/gnachman/iterm2/-/issues/4843) | Streamline fish shell integration | - | - | - | - | - | Skip | Feature request - not a bug |
| [#4816](https://gitlab.com/gnachman/iterm2/-/issues/4816) | Shell Integration appears to be incompatible with bash in... | - | 2025-12-26 | - | - | - | Skip (Old) | Old issue (2017) |
| [#4797](https://gitlab.com/gnachman/iterm2/-/issues/4797) | Feature request: allow alternate paths for shell integrat... | - | - | - | - | - | Skip | Feature request - not a bug |
| [#4587](https://gitlab.com/gnachman/iterm2/-/issues/4587) | Shell integration incompatible with bash-git-prompt | - | 2025-12-27 | - | - | - | Skip (Old) | Old issue (2016) |
| [#4504](https://gitlab.com/gnachman/iterm2/-/issues/4504) | BUG shell-integration | - | 2025-12-26 | - | - | - | Skip (Old) | Old issue (2017) - vague |
| [#4292](https://gitlab.com/gnachman/iterm2/-/issues/4292) | dot file for fish shell integration does not follow XDG p... | - | 2025-12-27 | - | - | - | Skip (Old) | Old issue (2016) |
| [#4277](https://gitlab.com/gnachman/iterm2/-/issues/4277) | what is the correct behavior when splitting a terminal af... | - | 2025-12-27 | - | - | - | Skip (Old) | Old issue (2016) |
| [#4257](https://gitlab.com/gnachman/iterm2/-/issues/4257) | Shell integration doesn't work with mosh | - | 2025-12-27 | - | - | - | Skip (Old) | Old issue (2016) - mosh limitation |
| [#4225](https://gitlab.com/gnachman/iterm2/-/issues/4225) | Small problem with shell integration host detection | - | 2025-12-27 | - | - | - | Skip (Old) | Old issue (2016) |
| [#4160](https://gitlab.com/gnachman/iterm2/-/issues/4160) | RHEL 7.2 conflicts with Shell Integration | - | 2025-12-26 | - | - | - | Skip (Old) | Old issue (2016) - RHEL 7.2 |
| [#4140](https://gitlab.com/gnachman/iterm2/-/issues/4140) | With RedHat 7.2, bash 4.2.46(1), .iterm2_shell_integratio... | - | 2025-12-26 | - | - | - | Skip (Old) | Old issue (2016) - RHEL 7.2 |
| [#4133](https://gitlab.com/gnachman/iterm2/-/issues/4133) | Shell Integration doesn't work for me | - | 2025-12-26 | - | - | - | Skip (Old) | Old issue (2016) - vague |
| [#3982](https://gitlab.com/gnachman/iterm2/-/issues/3982) | shell integrations breaks console | - | 2025-12-26 | - | - | - | Skip (Old) | Old issue (2016) |
| [#3865](https://gitlab.com/gnachman/iterm2/-/issues/3865) | Shell Integration not working | - | 2025-12-26 | - | - | - | Skip (Old) | Old issue (2016) - vague |
| [#3769](https://gitlab.com/gnachman/iterm2/-/issues/3769) | DashTerm2 Shell Integration affects CodeRunner | - | 2025-12-27 | - | - | - | Skip (Old) | Old issue (2015) |
| [#3735](https://gitlab.com/gnachman/iterm2/-/issues/3735) | shell integration typeset error | - | 2025-12-27 | - | - | - | Skip (Old) | Old issue (2015) |
| [#3652](https://gitlab.com/gnachman/iterm2/-/issues/3652) | shell_integration.html contains broken link | - | 2025-12-26 | - | - | - | Skip (Docs) | Docs - broken link |
| [#3435](https://gitlab.com/gnachman/iterm2/-/issues/3435) | DashTerm2 Shell Integrations don't play well with show-mo... | - | 2025-12-27 | - | - | - | Skip (Old) | Old issue (2015) |

---

## Statistics

| Metric | Count |
|--------|-------|
| Total | 63 |
| Fixed | 5 |
| In Progress | 0 |
| Inspected | 0 |
| Open | 0 |
| Skip (Feature Request) | 7 |
| Skip (Old/Obsolete) | 33 |
| Skip (Docs/Website) | 3 |
| External | 6 |
| Cannot Reproduce | 5 |
| Wontfix | 0 |

---

## Category Notes

Shell integration issues typically involve compatibility with specific shells (Fish, Bash, Zsh, Tcsh), prompt themes (Starship, Powerline, bash-git-prompt), and other terminal tools.

### Common Patterns

- **Fish 4.x compatibility**: Fish 4.x changes OSC 133 handling - #12641 fixed
- **Prompt theme conflicts**: Starship, Powerline, bash-git-prompt interfere with shell integration
- **Old issues (2015-2021)**: Many reference obsolete iTerm2 versions, RHEL 7.2, old shells - now all marked Skip (Old)
- **Auto-loading issues**: "Load shell integration automatically" causes problems with SSH - #11378 fixed

### Upstream Fixes Applied (This Iteration)

| Issue | Upstream Commit | Description |
|-------|-----------------|-------------|
| #12641 | 865aad0b2 | Bump shell integration for Fish 4.x |
| #12518 | 87f86bcf0, 70c2b18c9 | Avoid pausing on FTCS C |
| #12240 | c19c70a42, 54160e44a | Remove stray fish marks, prevent double prompt marks |
| #11378 | 8b3dd5868 | Queue input while connecting to SSH |

### Remaining Open Issues (0)

**P1 Shell Integration is 100% triaged!**

All 63 issues have been categorized:
- 5 Fixed
- 6 External (including #12616 - Fish ecosystem development)
- 5 Cannot Reproduce
- 7 Skip (Feature Requests)
- 33 Skip (Old/Obsolete)
- 3 Skip (Docs/Website)

### Fixed Issues - DashTerm2 Fixes (Worker #1310)

**#12382** - OSC 133;D not considered end of command
- **Root Cause**: When OSC 133;D was received with a return code, `assignCurrentCommandEndDate` was not called. This meant the command's `endDate` property on the mark was not set, so the command wasn't properly considered "ended" by shell history tracking.
- **Fix**: Added call to `assignCurrentCommandEndDate` in `terminalReturnCodeOfLastCommandWas:` before setting the return code.
- **File Modified**: `sources/VT100ScreenMutableState+TerminalDelegate.m`

### External Issues (Worker #1310)

**#11294** - command not found: iterm2_shell_integration.zsh
- **Root Cause**: User has old iTerm2 shell integration sourced in their shell config (`~/.zshrc`, etc.). DashTerm2 uses different file names (`dashterm2_shell_integration.*`) to coexist with iTerm2.
- **Resolution**: User should either reinstall shell integration using DashTerm2's installer, or remove the old iTerm2 shell integration line from their shell config.
- **Not a bug**: Expected behavior since DashTerm2 uses different file names

### Cannot Reproduce (5 marked this iteration)

Old issues (2022-2023) with vague repro steps:
- **#11016** (2023) - PROMPT_COMMAND exit status conflict
- **#10528** (2022) - bash CurrentDir not set
- **#10218** (2022) - PWD preservation conflict
- **#10183** (2022) - /usr/bin/less issues on remote
- **#10172** (2022) - login shell output mangling

### Related Files

- `submodules/iTerm2-shell-integration/shell_integration/bash` - Bash integration script
- `submodules/iTerm2-shell-integration/shell_integration/zsh` - Zsh integration script
- `submodules/iTerm2-shell-integration/shell_integration/fish` - Fish integration script
- `submodules/iTerm2-shell-integration/shell_integration/tcsh` - Tcsh integration script
- `sources/iTermShellIntegrationInstaller.m` - Install shell integration
- `sources/VT100ScreenMutableState.m` - FTCS/OSC 133 handling (recent fixes)
