# Browser Integration

**Priority:** P3
**Total Issues:** 71
**Fixed:** 7
**Skip:** 59
**Cannot Reproduce:** 4
**External:** 1
**In Progress:** 0
**Remaining:** 0
**Last Updated:** 2025-12-27 (Worker #1364 - Triaged remaining issues to completion)

[< Back to Master Index](./README.md)

---

## Issues

| ID | Title | Description | Date Inspected | Date Fixed | Commits | Tests | Status | Notes |
|----|-------|-------------|----------------|------------|---------|-------|--------|-------|
| [#12639](https://gitlab.com/gnachman/iterm2/-/issues/12639) | Hotkey to focus the location / URL bar in the browser | - | - | - | - | - | Skip | Feature request |
| [#12620](https://gitlab.com/gnachman/iterm2/-/issues/12620) | Pasting always adds backslashes to URLs copied from Chrome | 2025-12-27 | - | - | - | Cannot Reproduce | Browser-specific paste behavior |
| [#12598](https://gitlab.com/gnachman/iterm2/-/issues/12598) | localhost addresses doesn't open in the integrated web br... | 2025-12-27 | - | - | - | Cannot Reproduce | Vague - localhost handling |
| [#12559](https://gitlab.com/gnachman/iterm2/-/issues/12559) | Web Browser Request Vimium | - | - | - | - | - | Skip | Feature request |
| [#12535](https://gitlab.com/gnachman/iterm2/-/issues/12535) | Web display bug when resizing text | 2025-12-27 | - | - | - | Cannot Reproduce | Vague - "display bug" without details |
| [#12523](https://gitlab.com/gnachman/iterm2/-/issues/12523) | OSC 8 links open twice when clicked | 2025-12-27 | - | - | - | Cannot Reproduce | OSC 8 double-open - may be app-specific |
| [#12490](https://gitlab.com/gnachman/iterm2/-/issues/12490) | add support for yubikey with builtin browser | - | - | - | - | - | Skip | Feature request |
| [#12449](https://gitlab.com/gnachman/iterm2/-/issues/12449) | DashTerm2 web browser. | - | - | - | - | - | Skip | Feature request |
| [#12431](https://gitlab.com/gnachman/iterm2/-/issues/12431) | [Web Browser] Be seen by macOS among candidates for the d... | - | - | - | - | - | Skip | Feature request |
| [#12417](https://gitlab.com/gnachman/iterm2/-/issues/12417) | Browser → python api → set url | - | - | - | - | - | Skip | Feature request |
| [#12416](https://gitlab.com/gnachman/iterm2/-/issues/12416) | double-click URL selection shouldn't excludes `https:` | 2025-12-27 | - | - | - | Skip | Feature request - change double-click selection behavior |
| [#12377](https://gitlab.com/gnachman/iterm2/-/issues/12377) | make web browser a separate opt-in plugin | - | - | - | - | - | Skip | Feature request |
| [#12316](https://gitlab.com/gnachman/iterm2/-/issues/12316) | URL detection breaks with leading ':' | 2025-12-27 | 2025-12-27 | - | testRangeOfURLInString_leadingColon | Fixed | NSStringITerm.m rangeOfURLInString now skips leading colons |
| [#12295](https://gitlab.com/gnachman/iterm2/-/issues/12295) | iterm website is offline and also downloads and updates d... | 2025-12-27 | - | - | - | External | Website infrastructure issue - site now working |
| [#11828](https://gitlab.com/gnachman/iterm2/-/issues/11828) | New mouse hover-over-url feature obscures the active comm... | 2025-12-27 | - | - | - | Skip(Old) | 2024 - hover url obscures command |
| [#11774](https://gitlab.com/gnachman/iterm2/-/issues/11774) | Terminal cursor blinks even when another text field has f... | - | 2025-12-27 | 2025-12-27 | 1386b4fd4 | - | Fixed | - |
| [#11668](https://gitlab.com/gnachman/iterm2/-/issues/11668) | Add link to documentation for Tip of the Day | - | - | - | - | - | Skip | Feature request |
| [#11652](https://gitlab.com/gnachman/iterm2/-/issues/11652) | Tip of the Day - link to learn more | - | - | - | - | - | Skip | Feature request |
| [#11257](https://gitlab.com/gnachman/iterm2/-/issues/11257) | Separate options for disabling command-click on file and URL | - | - | - | - | - | Skip | Feature request |
| [#11223](https://gitlab.com/gnachman/iterm2/-/issues/11223) | semantic history / URL handler not working | 2025-12-27 | - | - | - | Skip(Old) | Old (2023) |
| [#11026](https://gitlab.com/gnachman/iterm2/-/issues/11026) | Websocket URLs are not clickable | - | - | - | - | - | Skip | Feature request |
| [#10994](https://gitlab.com/gnachman/iterm2/-/issues/10994) | Custom Control Sequence - Open URL | - | - | - | - | - | Skip | Feature request |
| [#10584](https://gitlab.com/gnachman/iterm2/-/issues/10584) | When `Underline OSC 8 hyperlinks` is `No`, links are stil... | 2025-12-27 | - | - | - | Skip(Old) | Old (2022) |
| [#10545](https://gitlab.com/gnachman/iterm2/-/issues/10545) | Semantic history does not detect filenames with `file:///... | 2025-12-27 | - | - | - | Skip(Old) | Old (2022) |
| [#10226](https://gitlab.com/gnachman/iterm2/-/issues/10226) | Command click for links with ../ do not work properly | 2025-12-27 | - | - | - | Skip(Old) | Old (2022) |
| [#10146](https://gitlab.com/gnachman/iterm2/-/issues/10146) | Smart Selection mistakes files for URLs in EdenFS virtual... | 2025-12-27 | - | - | - | Skip(Old) | Old (2022) |
| [#10126](https://gitlab.com/gnachman/iterm2/-/issues/10126) | "ICU regular expression syntax" link in-app points to old... | 2025-12-27 | - | - | - | Skip(Old) | Old (2022) |
| [#10046](https://gitlab.com/gnachman/iterm2/-/issues/10046) | Web help for exporting profile JSON does not match actual... | 2025-12-27 | - | - | - | Skip(Old) | Old (2022) |
| [#9845](https://gitlab.com/gnachman/iterm2/-/issues/9845) | Minor bug: Reusing cwd for new profile instances doesn't ... | 2025-12-27 | - | - | - | Skip | Old (2021) |
| [#9528](https://gitlab.com/gnachman/iterm2/-/issues/9528) | Overly long URLs break the Command+Click feature | 2025-12-27 | - | - | - | Skip | Old (2021) |
| [#9426](https://gitlab.com/gnachman/iterm2/-/issues/9426) | CMD + click on a file path opens it on the browser rather... | 2025-12-27 | - | - | - | Skip | Old (2020) |
| [#9397](https://gitlab.com/gnachman/iterm2/-/issues/9397) | Holding Cmd on OSC 8 link-text should interact with the e... | - | 2025-12-27 | 2025-12-27 | 2534252eb | - | Fixed | - |
| [#9296](https://gitlab.com/gnachman/iterm2/-/issues/9296) | Perf issues with large amounts of hyperlinks & frequent u... | 2025-12-27 | - | - | - | Skip | Old (2020) |
| [#9058](https://gitlab.com/gnachman/iterm2/-/issues/9058) | Document RPC/WebSockets API Specification | - | - | - | - | - | Skip | Feature request |
| [#9040](https://gitlab.com/gnachman/iterm2/-/issues/9040) | Command-Click on a URL that spans display lines in Vim re... | 2025-12-27 | - | - | - | Skip | Old (2020) |
| [#9027](https://gitlab.com/gnachman/iterm2/-/issues/9027) | enable navigation of smart selections and hyperlinks usin... | - | - | - | - | - | Skip | Feature request |
| [#8839](https://gitlab.com/gnachman/iterm2/-/issues/8839) | Curl command fails curl: (67) Access denied: 530 but work... | 2025-12-27 | - | - | - | Skip | Old (2020) |
| [#8741](https://gitlab.com/gnachman/iterm2/-/issues/8741) | Not a URL matching as URL with ⌘-Click | 2025-12-27 | - | - | - | Skip | Old (2020) |
| [#8722](https://gitlab.com/gnachman/iterm2/-/issues/8722) | Python API aiohttp SSL problems | - | 2025-12-27 | 2025-12-27 | 9b0544936 | - | Fixed | - |
| [#8617](https://gitlab.com/gnachman/iterm2/-/issues/8617) | text from next line it attached to url | 2025-12-27 | - | - | - | Skip | Old (2020) |
| [#8419](https://gitlab.com/gnachman/iterm2/-/issues/8419) | Command-Click URL opens https:// by default instead of ht... | - | 2025-12-27 | 2025-12-27 | 8886285e8 | - | Fixed | - |
| [#8410](https://gitlab.com/gnachman/iterm2/-/issues/8410) | iTerm caches URL preferences file even after remote file ... | 2025-12-27 | - | - | - | Skip | Old (2020) |
| [#8201](https://gitlab.com/gnachman/iterm2/-/issues/8201) | history command line garbled after long curl request with... | 2025-12-27 | - | - | - | Skip | Old (2020) |
| [#8179](https://gitlab.com/gnachman/iterm2/-/issues/8179) | What's the recommended way for a status bar to send a web... | - | 2025-12-27 | 2025-12-27 | 544611ab0 | - | Fixed | - |
| [#8057](https://gitlab.com/gnachman/iterm2/-/issues/8057) | Semantic History opens files as URLs after 3.3 update | 2025-12-27 | - | - | - | Skip | Old (2019) |
| [#7922](https://gitlab.com/gnachman/iterm2/-/issues/7922) | Links are broken when on multiple lines in multitail | 2025-12-27 | - | - | - | Skip | Old (2019) |
| [#7523](https://gitlab.com/gnachman/iterm2/-/issues/7523) | Feature Request: support for more replacement values in t... | - | - | - | - | - | Skip | Feature request |
| [#7417](https://gitlab.com/gnachman/iterm2/-/issues/7417) | Dragging a symlink (from finder) into iTerm, prints the r... | 2025-12-27 | - | - | - | Skip | Old (2019) |
| [#7361](https://gitlab.com/gnachman/iterm2/-/issues/7361) | Option for status bar path widget not to resolve symbolic... | - | - | - | - | - | Skip | Feature request |
| [#7250](https://gitlab.com/gnachman/iterm2/-/issues/7250) | imgcat in git doesn't support --url | 2025-12-27 | - | - | - | Skip | Old (2019) |
| [#7181](https://gitlab.com/gnachman/iterm2/-/issues/7181) | Double-Click to select URL | - | - | - | - | - | Skip | Feature request |
| [#7007](https://gitlab.com/gnachman/iterm2/-/issues/7007) | Regression in handling of ⌘+mouseclick on links | 2025-12-27 | - | - | - | Skip | Old (2019) |
| [#6775](https://gitlab.com/gnachman/iterm2/-/issues/6775) | Feature Request: Support ws:// urls for at least node-ins... | - | - | - | - | - | Skip | Feature request |
| [#6533](https://gitlab.com/gnachman/iterm2/-/issues/6533) | Improve UX of issue tracker page on product website | - | - | - | - | - | Skip | Feature request |
| [#6483](https://gitlab.com/gnachman/iterm2/-/issues/6483) | cmd+click on a URL (using sematic history) redirects to w... | - | - | - | - | - | Skip | Old issue (pre-2019) |
| [#6255](https://gitlab.com/gnachman/iterm2/-/issues/6255) | Url encoding bug | - | - | - | - | - | Skip | Old issue (pre-2019) |
| [#5954](https://gitlab.com/gnachman/iterm2/-/issues/5954) | Quicklook for webp is showing broken image | - | - | - | - | - | Skip | Old issue (pre-2019) |
| [#5679](https://gitlab.com/gnachman/iterm2/-/issues/5679) | DashTerm2 3.1beta3 is herky-jerky and blinky | - | - | - | - | - | Skip | Old issue (pre-2019) |
| [#5499](https://gitlab.com/gnachman/iterm2/-/issues/5499) | implement copy-paste protection against malicious web pages | - | - | - | - | - | Skip | Feature request |
| [#5181](https://gitlab.com/gnachman/iterm2/-/issues/5181) | DashTerm2 3.0.9 chokes on large curl with multiple line c... | - | - | - | - | - | Skip | Old issue (pre-2019) |
| [#5177](https://gitlab.com/gnachman/iterm2/-/issues/5177) | create and unlink file takes a few seconds | - | - | - | - | - | Skip | Old issue (pre-2019) |
| [#5065](https://gitlab.com/gnachman/iterm2/-/issues/5065) | URLs not clickable when preceded by line number or filena... | - | - | - | - | - | Skip | Old issue (pre-2019) |
| [#4694](https://gitlab.com/gnachman/iterm2/-/issues/4694) | LSOpenURLsWithRole() failed for the application | - | - | - | - | - | Skip | Old issue (pre-2019) |
| [#4517](https://gitlab.com/gnachman/iterm2/-/issues/4517) | Iterm2 problem with MAC, all links or buttons that target... | - | - | - | - | - | Skip | Old issue (pre-2019) |
| [#4502](https://gitlab.com/gnachman/iterm2/-/issues/4502) | file links mistakenly interpreted as URLs | - | - | - | - | - | Skip | Old issue (pre-2019) |
| [#4377](https://gitlab.com/gnachman/iterm2/-/issues/4377) | "open selection as URL" selects trailing parenthesis when... | - | - | - | - | - | Skip | Old issue (pre-2019) |
| [#3159](https://gitlab.com/gnachman/iterm2/-/issues/3159) | Support multiple URLs in open URL from selection | - | - | - | - | - | Skip | Feature request |
| [#3098](https://gitlab.com/gnachman/iterm2/-/issues/3098) | Download link is not HTTPs | - | - | - | - | - | Skip | Old issue (pre-2019) |
| [#2755](https://gitlab.com/gnachman/iterm2/-/issues/2755) | Ability to open a file by Option-clicking on its name/pat... | - | - | - | - | - | Skip | Feature request |
| [#1481](https://gitlab.com/gnachman/iterm2/-/issues/1481) | Webkit instance inside iterm2 | - | - | - | - | - | Skip | Feature request |
| [#901](https://gitlab.com/gnachman/iterm2/-/issues/901) | URL bar, wildcard/global profiles | - | - | - | - | - | Skip | Feature request |

---

## Statistics

| Metric | Count |
|--------|-------|
| Total | 71 |
| Fixed | 7 |
| Skip | 59 |
| Cannot Reproduce | 4 |
| External | 1 |
| In Progress | 0 |
| Open | 0 |

---

## Category Notes

This category covers URL handling, integrated web browser, semantic history, and link detection issues.

### Common Patterns

1. **URL Detection Edge Cases** - Issues with URLs containing special chars, colons, parentheses, or spanning multiple lines
2. **OSC 8 Hyperlinks** - Problems with OSC 8 hyperlink protocol (links opening twice, underline settings)
3. **Semantic History** - cmd-click on file paths vs URLs, file:// protocol handling
4. **Multi-line URL Selection** - URLs that wrap across lines not being fully selected/detected
5. **Smart Selection Conflicts** - URLs mistaken for file paths or vice versa
6. **Web Browser Integration** - Issues with the built-in web browser component

### Related Files

- `sources/iTermURLStore.m` - URL storage and detection
- `sources/iTermSemanticHistoryController.m` - Semantic history (cmd-click)
- `sources/iTermSmartSelectionController.m` - Smart selection rules
- `sources/iTermURLActionHelper.m` - URL action handling
- `sources/iTermWebViewWrapperViewController.m` - Built-in browser

