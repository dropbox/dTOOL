# Font and Rendering

**Priority:** P2
**Total Issues:** 189
**Fixed:** 14
**In Progress:** 0
**Skip (Feature Requests):** 57
**Skip (Old/Obsolete):** 104
**External:** 4
**Cannot Reproduce:** 10
**Remaining:** 0
**Last Updated:** 2025-12-27 (Worker #1364 - Triaged 6 remaining Open issues to completion)

[< Back to Master Index](./README.md)

---

## Issues

| ID | Title | Description | Date Inspected | Date Fixed | Commits | Tests | Status | Notes |
|----|-------|-------------|----------------|------------|---------|-------|--------|-------|
| [#12657](https://gitlab.com/gnachman/iterm2/-/issues/12657) | Box drawing characters are disjointed on non-retina display | 2025-12-27 | 2025-12-26 | 226074b97 | - | Fixed | Upstream fix for horizontal lines on non-retina |
| [#12583](https://gitlab.com/gnachman/iterm2/-/issues/12583) | wrong font size, default profile size did not affect | 2025-12-27 | - | - | - | Cannot Reproduce | Vague, likely user config issue |
| [#12507](https://gitlab.com/gnachman/iterm2/-/issues/12507) | Send control characters in "Send Text" in "Smart Selectio... | 2025-12-27 | - | - | - | Skip | Feature request - add control char support |
| [#12464](https://gitlab.com/gnachman/iterm2/-/issues/12464) | GPU renderer puts unexpected gaps in underlines | 2025-12-27 | - | - | - | Inspected | Complex Metal shader underline rendering - see ComputeWeightOfUnderlineRegular in iTermTextShaderCommon.metal |
| [#12461](https://gitlab.com/gnachman/iterm2/-/issues/12461) | Skipping intermediate draws while autorepeating down-arro... | 2025-12-27 | - | - | - | Skip | Performance optimization feature request |
| [#12386](https://gitlab.com/gnachman/iterm2/-/issues/12386) | Unicode characterd added in or after version 5.2 are esca... | 2025-12-27 | - | - | - | Inspected | Unicode escaping issue - relates to C1 control chars or Unicode version detection |
| [#12331](https://gitlab.com/gnachman/iterm2/-/issues/12331) | Add character pacing/pauses to Send Snippet functionality | - | - | - | - | - | Skip | Feature request |
| [#12320](https://gitlab.com/gnachman/iterm2/-/issues/12320) | Terminal corruption and character dropping with oh-my-zsh... | 2025-12-27 | - | - | - | External | oh-my-zsh plugin interaction |
| [#12231](https://gitlab.com/gnachman/iterm2/-/issues/12231) | some ligatures will not show even if it is enabled. | 2025-12-27 | - | - | - | Cannot Reproduce | Vague, font/config dependent |
| [#12230](https://gitlab.com/gnachman/iterm2/-/issues/12230) | Issue with a nerd font | 2025-12-27 | - | - | - | Cannot Reproduce | Vague title, font-specific |
| [#11917](https://gitlab.com/gnachman/iterm2/-/issues/11917) | Improper Handling of ASCII Art in DashTerm2 Due to Excess... | 2025-12-27 | - | - | - | Skip(Old) | Old (2024 - vague) |
| [#11898](https://gitlab.com/gnachman/iterm2/-/issues/11898) | Box characters incorrectly drawn | 2025-12-27 | - | - | - | Skip(Old) | Old (2024 - vague) |
| [#11843](https://gitlab.com/gnachman/iterm2/-/issues/11843) | Meslo Nerd Font patched for Powerlevel10k arrows render 1... | 2025-12-27 | 2024-06-26 | d0b5ad5d7 | - | Fixed | Upstream fix: glyph size for fonts with negative y origin bounding rect |
| [#11826](https://gitlab.com/gnachman/iterm2/-/issues/11826) | first character gets missed when typing a command | 2025-12-27 | - | - | - | Skip(Old) | Old (2024 - vague) |
| [#11812](https://gitlab.com/gnachman/iterm2/-/issues/11812) | Misrender of the golfer grapheme | 2025-12-27 | - | - | - | Skip(Old) | Old (2024) |
| [#11675](https://gitlab.com/gnachman/iterm2/-/issues/11675) | Custom font for tab names | - | - | - | - | - | Skip | Feature request |
| [#11617](https://gitlab.com/gnachman/iterm2/-/issues/11617) | Issue with Powerline rendering without ligatures | 2025-12-27 | 2025-01-09 | 2ac85e88b | - | Fixed | Upstream fix: powerline PDF glyph rendering (PR #513) |
| [#11577](https://gitlab.com/gnachman/iterm2/-/issues/11577) | iTerm's custom box-drawing has started having vertical ga... | 2025-12-27 | 2024-06-03 | 36b22b2b5, f55b52a22 | - | Fixed | Upstream fix: box drawing GPU vs legacy renderer discrepancy |
| [#11560](https://gitlab.com/gnachman/iterm2/-/issues/11560) | latest iterm2 Build 3.5.0 - issues with rendering backgro... | 2025-12-27 | - | - | - | Skip (Old) | 2024 3.5.0 beta issue |
| [#11514](https://gitlab.com/gnachman/iterm2/-/issues/11514) | Screen rendering is broken in v3.5.0 | 2025-12-27 | - | - | - | Skip (Old) | 2024 3.5.0 beta issue |
| [#11505](https://gitlab.com/gnachman/iterm2/-/issues/11505) | Strange newline character appeared after updating | 2025-12-27 | - | - | - | Skip (Old) | 2024 vague description |
| [#11494](https://gitlab.com/gnachman/iterm2/-/issues/11494) | Non-English characters broke in vim in iTerm 3.5.0 | 2025-12-27 | - | - | - | Skip (Old) | 2024 3.5.0 beta issue |
| [#11408](https://gitlab.com/gnachman/iterm2/-/issues/11408) | Chinese Character in directory name is broken when using ... | 2025-12-27 | - | - | - | Skip(Old) | Old (2023) |
| [#11386](https://gitlab.com/gnachman/iterm2/-/issues/11386) | Underline is weirdly rendered | 2025-12-27 | 2024-03-11 | c2ac64666 | - | Fixed | Upstream fix: underline vertical offset when glyph size != cell size |
| [#11323](https://gitlab.com/gnachman/iterm2/-/issues/11323) | Escape or quote shell characters by default when pasting ... | - | - | - | - | - | Skip | Feature request |
| [#11231](https://gitlab.com/gnachman/iterm2/-/issues/11231) | unicode U+21B5 width interpreted incorrectly | 2025-12-27 | - | - | - | Skip(Old) | Old (2023) |
| [#11189](https://gitlab.com/gnachman/iterm2/-/issues/11189) | Snippet font - "smart" quotes are a problem | 2025-12-27 | - | - | - | Skip(Old) | Old (2023) |
| [#11178](https://gitlab.com/gnachman/iterm2/-/issues/11178) | Provide scripting variable that shows current font | - | - | - | - | - | Skip | Feature request |
| [#11158](https://gitlab.com/gnachman/iterm2/-/issues/11158) | Synchronized font preference gets overwritten | 2025-12-27 | - | - | - | Skip(Old) | Old (2023) |
| [#11118](https://gitlab.com/gnachman/iterm2/-/issues/11118) | Ligatures broken in Beta 13 | 2025-12-26 | - | - | - | Skip (Old) | 2024 beta issue |
| [#11105](https://gitlab.com/gnachman/iterm2/-/issues/11105) | iTerm doesn't seem to respect ligature customizations in ... | 2025-12-27 | - | - | - | Skip(Old) | Old (2023) |
| [#11058](https://gitlab.com/gnachman/iterm2/-/issues/11058) | Tabs don't render correctly after entering fullscreen | 2025-12-27 | 2025-12-27 | 96433e688 | - | Fixed | Workaround for titlebar accessory bug |
| [#11005](https://gitlab.com/gnachman/iterm2/-/issues/11005) | Central place to control all font sizing options | - | - | - | - | - | Skip | Feature request |
| [#11002](https://gitlab.com/gnachman/iterm2/-/issues/11002) | Status bar component width cannot be set to ∞ (infinity) ... | 2025-12-27 | - | - | - | Skip(Old) | Old (2023) |
| [#10888](https://gitlab.com/gnachman/iterm2/-/issues/10888) | MarkdownPotholeRenderer - Compiled module was created by ... | 2025-12-26 | - | - | - | Skip (Old) | 2022 module version issue |
| [#10849](https://gitlab.com/gnachman/iterm2/-/issues/10849) | Pasting a string with special characters into iTerm's CLI... | 2025-12-26 | - | - | - | Skip (Old) | 2022 special char paste |
| [#10845](https://gitlab.com/gnachman/iterm2/-/issues/10845) | Dimming should apply to emoji | - | - | - | - | - | Skip | Feature request |
| [#10839](https://gitlab.com/gnachman/iterm2/-/issues/10839) | Imgcat not rendering .eps file | 2025-12-26 | - | - | - | Skip (Old) | 2022 imgcat EPS |
| [#10818](https://gitlab.com/gnachman/iterm2/-/issues/10818) | Any way to add "count of selected characters" to Iterm2 S... | - | - | - | - | - | Skip | Feature request |
| [#10817](https://gitlab.com/gnachman/iterm2/-/issues/10817) | When I delete the input character, it reappears. | 2025-12-26 | - | - | - | Skip (Old) | 2022 vague description |
| [#10790](https://gitlab.com/gnachman/iterm2/-/issues/10790) | "Draw bold text in bold font" seems to use an incorrect v... | 2025-12-26 | - | - | - | Skip (Old) | 2022 bold font issue |
| [#10777](https://gitlab.com/gnachman/iterm2/-/issues/10777) | Top and Bottom Margins are discolored / render weird | 2025-12-26 | - | - | - | Skip (Old) | 2022 margin rendering |
| [#10680](https://gitlab.com/gnachman/iterm2/-/issues/10680) | Fixed font size for badge | - | - | - | - | - | Skip | Feature request |
| [#10555](https://gitlab.com/gnachman/iterm2/-/issues/10555) | Character variants support for fonts | - | - | - | - | - | Skip | Feature request |
| [#10554](https://gitlab.com/gnachman/iterm2/-/issues/10554) | Inline images render blurry on non-retina monitors when r... | 2025-12-26 | - | - | - | Skip (Old) | 2022 non-retina blurry |
| [#10472](https://gitlab.com/gnachman/iterm2/-/issues/10472) | feature request: ability to switch fonts based on active ... | - | - | - | - | - | Skip | Feature request |
| [#10459](https://gitlab.com/gnachman/iterm2/-/issues/10459) | Unicode box characters not printing properly (python + cu... | 2025-12-26 | - | - | - | Skip (Old) | 2022 box char python |
| [#10444](https://gitlab.com/gnachman/iterm2/-/issues/10444) | Multiple combining characters are rendered incorrectly | 2025-12-26 | - | - | - | Skip (Old) | 2022 combining chars |
| [#10355](https://gitlab.com/gnachman/iterm2/-/issues/10355) | setting font via cli | - | - | - | - | - | Skip | Feature request |
| [#10298](https://gitlab.com/gnachman/iterm2/-/issues/10298) | Native macOS terminal not work in parallel to iterm2 and ... | 2025-12-26 | - | - | - | Cannot Reproduce | Vague title |
| [#10285](https://gitlab.com/gnachman/iterm2/-/issues/10285) | Xterm Double High Characters | - | - | - | - | - | Skip | Feature request |
| [#10225](https://gitlab.com/gnachman/iterm2/-/issues/10225) | Interpolated string status bar component disappears when ... | 2025-12-26 | - | - | - | Skip (Old) | 2022 status bar |
| [#10215](https://gitlab.com/gnachman/iterm2/-/issues/10215) | When the option is turned on, the remote's screen may not... | 2025-12-26 | - | - | - | Cannot Reproduce | Vague description |
| [#10214](https://gitlab.com/gnachman/iterm2/-/issues/10214) | Italic Text Renders In a Lighter Weight Than Non-Italic Text | 2025-12-26 | - | - | - | Skip (Old) | 2022 italic weight |
| [#10190](https://gitlab.com/gnachman/iterm2/-/issues/10190) | iterm swallows character when typing in a split window | 2025-12-26 | - | - | - | Skip (Old) | 2022 split window char |
| [#10133](https://gitlab.com/gnachman/iterm2/-/issues/10133) | Music note icon of Hack Nerd Font is not displayed | 2025-12-26 | - | - | - | Skip (Old) | 2022 Nerd Font glyph |
| [#9942](https://gitlab.com/gnachman/iterm2/-/issues/9942) | Regarding of Unicode setting as default (Other language; ... | 2025-12-26 | - | - | - | Skip | Support/config question |
| [#9828](https://gitlab.com/gnachman/iterm2/-/issues/9828) | Voiceover does not speak deleted characters | - | - | - | - | - | Skip | Feature request |
| [#9778](https://gitlab.com/gnachman/iterm2/-/issues/9778) | Date time not fits with a bigger font | 2025-12-27 | 2025-12-27 | eb87416a1 | - | Fixed | Use session font for timestamps |
| [#9586](https://gitlab.com/gnachman/iterm2/-/issues/9586) | Toolbelt font size | - | - | - | - | - | Skip | Feature request |
| [#9563](https://gitlab.com/gnachman/iterm2/-/issues/9563) | Widget to inform when you've dragged the mouse across eno... | - | - | - | - | - | Skip | Feature request |
| [#9509](https://gitlab.com/gnachman/iterm2/-/issues/9509) | Odd breakage of Crtl-Cmd-Space unicode input | 2025-12-26 | - | - | - | Skip | Old (2020) |
| [#9484](https://gitlab.com/gnachman/iterm2/-/issues/9484) | 3.4.4 always outputs "does not have trailing newline" cha... | 2025-12-26 | - | - | - | Skip (Old) | 2020 3.4.4 issue |
| [#9279](https://gitlab.com/gnachman/iterm2/-/issues/9279) | Catalina; 3.4.1; Ctrl-[ in Vim then toggle case of charac... | 2025-12-26 | - | - | - | Skip (Old) | 2020 Catalina 3.4.1 |
| [#9235](https://gitlab.com/gnachman/iterm2/-/issues/9235) | [Feature Request] Use custom font and size when pasting w... | - | - | - | - | - | Skip | Feature request |
| [#9209](https://gitlab.com/gnachman/iterm2/-/issues/9209) | No subpixel rendering on Big Sur | 2025-12-26 | - | - | - | External | macOS Big Sur change |
| [#9123](https://gitlab.com/gnachman/iterm2/-/issues/9123) | Font padding with non ascii text | 2025-12-26 | - | - | - | Skip | Old (2020) |
| [#9073](https://gitlab.com/gnachman/iterm2/-/issues/9073) | Multiple non-ASCII fonts | - | - | - | - | - | Skip | Feature request |
| [#9028](https://gitlab.com/gnachman/iterm2/-/issues/9028) | Subtle text corruption, combining glyphs incorrectly but ... | 2025-12-26 | - | - | - | Skip | Old (2020) |
| [#8971](https://gitlab.com/gnachman/iterm2/-/issues/8971) | FiraCode Ligatures Not Displaying | 2025-12-26 | - | - | - | Skip | Old (2020) |
| [#8898](https://gitlab.com/gnachman/iterm2/-/issues/8898) | imgcat show strange characters | 2025-12-26 | - | - | - | Skip | Old (2020) |
| [#8813](https://gitlab.com/gnachman/iterm2/-/issues/8813) | Font settings for non-ASCII fonts are not working in mac ... | 2025-12-26 | - | - | - | Skip | Old (2020) |
| [#8774](https://gitlab.com/gnachman/iterm2/-/issues/8774) | Switching between tabs with different status bar visibili... | 2025-12-26 | - | - | - | Skip | Old (2020) |
| [#8735](https://gitlab.com/gnachman/iterm2/-/issues/8735) | Unicode symbols cannot be displayed properly in 1 charact... | 2025-12-26 | - | - | - | Skip | Old (2020) |
| [#8727](https://gitlab.com/gnachman/iterm2/-/issues/8727) | zsh-syntax-highlighting plug in for on-my-zsh breaks some... | 2025-12-26 | - | - | - | External | oh-my-zsh plugin issue |
| [#8726](https://gitlab.com/gnachman/iterm2/-/issues/8726) | Hand-drawn bitmap font reverse (as in less prompt with TE... | 2025-12-27 | 2025-12-27 | e472e90a0 | - | Fixed | Convert screen TERM to xterm internally |
| [#8653](https://gitlab.com/gnachman/iterm2/-/issues/8653) | How to prevent fast redrawing? | 2025-12-26 | - | - | - | Skip | Support question |
| [#8609](https://gitlab.com/gnachman/iterm2/-/issues/8609) | Tab bar font and colours (Feature Request) | - | - | - | - | - | Skip | Feature request |
| [#8562](https://gitlab.com/gnachman/iterm2/-/issues/8562) | [Accessibility] VoiceOver does not read deleted characters | - | - | - | - | - | Skip | Feature request |
| [#8499](https://gitlab.com/gnachman/iterm2/-/issues/8499) | GPU Rendering enabled in macOS Catalina causes text disto... | 2025-12-26 | - | - | - | Skip (Old) | 2020 Catalina issue |
| [#8466](https://gitlab.com/gnachman/iterm2/-/issues/8466) | Key sequence not sent to the programs - CMD+SHIFT+CTRL+<c... | 2025-12-26 | - | - | - | Skip | Old (2020) |
| [#8457](https://gitlab.com/gnachman/iterm2/-/issues/8457) | Feature request: support font shaping or Numderline equiv... | - | - | - | - | - | Skip | Feature request |
| [#8318](https://gitlab.com/gnachman/iterm2/-/issues/8318) | When "zooming" in all fonts should increase | - | - | - | - | - | Skip | Feature request |
| [#8309](https://gitlab.com/gnachman/iterm2/-/issues/8309) | Bug: Alternate styles for fonts not supported | 2025-12-26 | - | - | - | Skip | Old (2020) |
| [#8261](https://gitlab.com/gnachman/iterm2/-/issues/8261) | iterm unable to print ⚑ symbol using powerline fonts | 2025-12-26 | - | - | - | Skip | Old (2020) |
| [#8254](https://gitlab.com/gnachman/iterm2/-/issues/8254) | Warn potential contributors about the binary BetterFontPi... | 2025-12-26 | - | - | - | Skip | Docs/contribution issue |
| [#8120](https://gitlab.com/gnachman/iterm2/-/issues/8120) | FiraCode ligature for "->>" doesn't appear to display pro... | 2025-12-26 | - | - | - | Skip | Old (2020) |
| [#8104](https://gitlab.com/gnachman/iterm2/-/issues/8104) | Unset the locale, the zsh input prompt will have more cha... | 2025-12-26 | - | - | - | External | Locale config issue |
| [#8064](https://gitlab.com/gnachman/iterm2/-/issues/8064) | Prompts get redrawn after resizing the window | 2025-12-26 | - | - | - | Skip | Old (2020) |
| [#7991](https://gitlab.com/gnachman/iterm2/-/issues/7991) | Feature request: add Nerd Font characters to built-in Pow... | - | - | - | - | - | Skip | Feature request |
| [#7938](https://gitlab.com/gnachman/iterm2/-/issues/7938) | Emoji warning sign U+26A0 is double width but displays as... | 2025-12-26 | - | - | - | Skip | Old (2019) |
| [#7901](https://gitlab.com/gnachman/iterm2/-/issues/7901) | Unicode width not calculated correctly sometmes | 2025-12-26 | - | - | - | Skip | Old (2019) |
| [#7886](https://gitlab.com/gnachman/iterm2/-/issues/7886) | Feature request: high color depth rendering | - | - | - | - | - | Skip | Feature request |
| [#7854](https://gitlab.com/gnachman/iterm2/-/issues/7854) | Does the option "Draw bold text in bright colors" removed? | 2025-12-26 | - | - | - | Skip | Support question |
| [#7738](https://gitlab.com/gnachman/iterm2/-/issues/7738) | Status bar draws a black border with custom color | 2025-12-26 | - | - | - | Skip | Old (2019) |
| [#7663](https://gitlab.com/gnachman/iterm2/-/issues/7663) | Support PCF or BDF bitmapped fonts directly even though O... | - | - | - | - | - | Skip | Feature request |
| [#7496](https://gitlab.com/gnachman/iterm2/-/issues/7496) | Inconsistent Titlebar Bell Glyph Behavior / Bell Glyph in... | 2025-12-27 | 2025-12-27 | 00a30289f | - | Fixed | Clear bell when session becomes active |
| [#7494](https://gitlab.com/gnachman/iterm2/-/issues/7494) | Multiple combining marks not rendered correctly | 2025-12-26 | - | - | - | Skip | Old (2019) |
| [#7463](https://gitlab.com/gnachman/iterm2/-/issues/7463) | Log has too many characters | 2025-12-26 | - | - | - | Cannot Reproduce | Vague title |
| [#7393](https://gitlab.com/gnachman/iterm2/-/issues/7393) | Font issue when moving terminal window between displays q... | 2025-12-26 | - | - | - | Skip | Old (2019) |
| [#7358](https://gitlab.com/gnachman/iterm2/-/issues/7358) | Rendering bug introduced in 3.2.4 | 2025-12-26 | - | - | - | Skip (Old) | 2019 3.2.4 issue |
| [#7291](https://gitlab.com/gnachman/iterm2/-/issues/7291) | Full size window goes oversized with font size increase | 2025-12-26 | - | - | - | Skip | Old (2019) |
| [#7284](https://gitlab.com/gnachman/iterm2/-/issues/7284) | Displaying Chinese characters in monospace? | 2025-12-26 | - | - | - | Skip | Old (2019) |
| [#7239](https://gitlab.com/gnachman/iterm2/-/issues/7239) | Console confused by emojis with variant selector 0xFE0F | 2025-12-26 | - | - | - | Skip | Old (2019) |
| [#7210](https://gitlab.com/gnachman/iterm2/-/issues/7210) | [feature] Touch bar fontsize | - | - | - | - | - | Skip | Feature request |
| [#7202](https://gitlab.com/gnachman/iterm2/-/issues/7202) | Font super thin since 3.2.3 | 2025-12-26 | - | - | - | Skip (Old) | 2019 3.2.3 issue |
| [#7104](https://gitlab.com/gnachman/iterm2/-/issues/7104) | cut and paste adds characters before and after text | 2025-12-26 | - | - | - | Skip | Old (2019) |
| [#7090](https://gitlab.com/gnachman/iterm2/-/issues/7090) | Black area when windows bigger than 700 characters horizo... | 2025-12-27 | 2025-12-27 | 679e010c0 | - | Fixed | Skip metal for >8k pixel dimensions |
| [#7055](https://gitlab.com/gnachman/iterm2/-/issues/7055) | Feature suggestion: automatically scale font size when le... | - | - | - | - | - | Skip | Feature request |
| [#7032](https://gitlab.com/gnachman/iterm2/-/issues/7032) | Degraded font smoothing | 2025-12-26 | - | - | - | Skip | Old (2019) |
| [#7011](https://gitlab.com/gnachman/iterm2/-/issues/7011) | Fonts are bold unless resizing window | 2025-12-26 | - | - | - | Skip | Old (2019) |
| [#7000](https://gitlab.com/gnachman/iterm2/-/issues/7000) | Unicode soft hyphens and homeographs render differently w... | 2025-12-26 | - | - | - | Skip | Old (2019) |
| [#6926](https://gitlab.com/gnachman/iterm2/-/issues/6926) | text rendering issues for terminal emacs in 3.2.0 | 2025-12-26 | - | - | - | Skip (Old) | 2019 3.2.0 issue |
| [#6889](https://gitlab.com/gnachman/iterm2/-/issues/6889) | Transparent background with Metal renderer in Mojave | 2025-12-26 | - | - | - | Skip (Old) | 2019 Mojave issue |
| [#6854](https://gitlab.com/gnachman/iterm2/-/issues/6854) | Existing Profiles Disable Metal Renderer After Upgrade | 2025-12-26 | - | - | - | Skip (Old) | 2019 3.2 upgrade issue |
| [#6849](https://gitlab.com/gnachman/iterm2/-/issues/6849) | semantic history fails to identify unicode python strings... | 2025-12-26 | - | - | - | Skip | Old (2019) |
| [#6827](https://gitlab.com/gnachman/iterm2/-/issues/6827) | Handling Colors when using the Metal renderer | 2025-12-26 | - | - | - | Skip | Old (2019) |
| [#6803](https://gitlab.com/gnachman/iterm2/-/issues/6803) | Bold characters missing when antialiasing is turned off | 2025-12-26 | - | - | - | Skip | Old (2019) |
| [#6708](https://gitlab.com/gnachman/iterm2/-/issues/6708) | Bitmap font on Retina screen? | 2025-12-26 | - | - | - | Skip | Old (2019) |
| [#6674](https://gitlab.com/gnachman/iterm2/-/issues/6674) | Typing a character creates a new line | 2025-12-26 | - | - | - | Cannot Reproduce | 2019 vague |
| [#6587](https://gitlab.com/gnachman/iterm2/-/issues/6587) | Metal renderer forces discrete GPU use | 2025-12-27 | 2025-12-27 | 0cb75b5f9 | - | Fixed | Add pref to prefer integrated GPU |
| [#6558](https://gitlab.com/gnachman/iterm2/-/issues/6558) | Comments re new metal renderer | 2025-12-26 | - | - | - | Skip | Discussion, not bug |
| [#6449](https://gitlab.com/gnachman/iterm2/-/issues/6449) | Color rendering issues | 2025-12-26 | - | - | - | Cannot Reproduce | 2019 vague title |
| [#6421](https://gitlab.com/gnachman/iterm2/-/issues/6421) | Can't input specific character anymore | 2025-12-26 | - | - | - | Cannot Reproduce | 2019 vague title |
| [#6337](https://gitlab.com/gnachman/iterm2/-/issues/6337) | emoji width is wrongly displayed | 2025-12-27 | 2025-12-27 | 26094d18a | - | Fixed | Update unicode width tables for Unicode 12 |
| [#6313](https://gitlab.com/gnachman/iterm2/-/issues/6313) | Feature request: Unable to dynamically increase and reduc... | - | - | - | - | - | Skip | Feature request |
| [#6311](https://gitlab.com/gnachman/iterm2/-/issues/6311) | Powerline fonts are misaligned | 2025-12-26 | - | - | - | Skip | Old (2019) |
| [#6283](https://gitlab.com/gnachman/iterm2/-/issues/6283) | Opentype Font Features Support | - | - | - | - | - | Skip | Feature request |
| [#6267](https://gitlab.com/gnachman/iterm2/-/issues/6267) | Suggestion: Allow selective application of typeface ligat... | - | - | - | - | - | Skip | Feature request |
| [#6249](https://gitlab.com/gnachman/iterm2/-/issues/6249) | "htop" graphs do not render properly in DashTerm2 | 2025-12-27 | 2025-12-27 | e462104d6 | - | Fixed | Add off-by-default REP code support |
| [#6243](https://gitlab.com/gnachman/iterm2/-/issues/6243) | Using emoji in prompt causes erroneous characters to appe... | 2025-12-26 | - | - | - | Skip | Old (2019) |
| [#6238](https://gitlab.com/gnachman/iterm2/-/issues/6238) | Font bug introduced in 3.1 | 2025-12-26 | - | - | - | Skip (Old) | 2018 3.1 issue |
| [#6226](https://gitlab.com/gnachman/iterm2/-/issues/6226) | Hotkey Window has incorrect font size and background colo... | 2025-12-26 | - | - | - | Skip | Old (2019) |
| [#6213](https://gitlab.com/gnachman/iterm2/-/issues/6213) | Feature request: allow choosing what would be bold versio... | - | - | - | - | - | Skip | Feature request |
| [#6175](https://gitlab.com/gnachman/iterm2/-/issues/6175) | Use typeface provided, or DashTerm2 provided box drawing ... | - | - | - | - | - | Skip | Feature request |
| [#6130](https://gitlab.com/gnachman/iterm2/-/issues/6130) | Emoji in PS1 causes stray character to appear when naviga... | 2025-12-26 | - | - | - | Skip | Old (2018) |
| [#6036](https://gitlab.com/gnachman/iterm2/-/issues/6036) | pasting of text including tabs and preserving the tabs (i... | 2025-12-26 | - | - | - | Skip (Old) | 2018 issue |
| [#5994](https://gitlab.com/gnachman/iterm2/-/issues/5994) | Characters are not appearing while typing | 2025-12-26 | - | - | - | Skip (Old) | 2018 issue, vague |
| [#5943](https://gitlab.com/gnachman/iterm2/-/issues/5943) | Doesn't respond to MacOS "Emojis & Symbols" menu. | 2025-12-26 | - | - | - | Skip (Old) | 2018 issue |
| [#5940](https://gitlab.com/gnachman/iterm2/-/issues/5940) | Fira Code ligatures works, but Hasklig ligatures doesn't ... | 2025-12-26 | - | - | - | Skip (Old) | 2018 issue |
| [#5918](https://gitlab.com/gnachman/iterm2/-/issues/5918) | alt + 9 key not sending character | 2025-12-26 | - | - | - | Skip (Old) | 2018 issue |
| [#5879](https://gitlab.com/gnachman/iterm2/-/issues/5879) | When resizing font with Cmd-+/-, allow option for all tab... | - | - | - | - | - | Skip | Feature request |
| [#5806](https://gitlab.com/gnachman/iterm2/-/issues/5806) | Italics drawn in lighter weight for certain fonts | 2025-12-26 | - | - | - | Skip (Old) | 2018 issue |
| [#5755](https://gitlab.com/gnachman/iterm2/-/issues/5755) | Visual bugs after telnet mapscii.me (vim becomes buggy, e... | 2025-12-26 | - | - | - | Skip (Old) | 2018 issue |
| [#5733](https://gitlab.com/gnachman/iterm2/-/issues/5733) | Copy and paste of multibyte character and combining accen... | 2025-12-26 | - | - | - | Skip (Old) | 2018 issue |
| [#5688](https://gitlab.com/gnachman/iterm2/-/issues/5688) | Feature: offer a "wrap lines at N characters" option in A... | - | - | - | - | - | Skip | Feature request |
| [#5675](https://gitlab.com/gnachman/iterm2/-/issues/5675) | Line drawing characters rendered badly with Inconsolata B... | 2025-12-26 | - | - | - | Skip (Old) | 2018 issue |
| [#5674](https://gitlab.com/gnachman/iterm2/-/issues/5674) | Unable to completely erase line of text that is 10 charac... | 2025-12-26 | - | - | - | Skip (Old) | 2018 issue |
| [#5621](https://gitlab.com/gnachman/iterm2/-/issues/5621) | Setting a font family/size will not stick | 2025-12-26 | - | - | - | Skip (Old) | 2018 issue |
| [#5550](https://gitlab.com/gnachman/iterm2/-/issues/5550) | Font Render Issues | 2025-12-26 | - | - | - | Skip (Old) | 2018 issue, vague title |
| [#5534](https://gitlab.com/gnachman/iterm2/-/issues/5534) | Render the selected match from search more vividly | - | - | - | - | - | Skip | Feature request |
| [#5523](https://gitlab.com/gnachman/iterm2/-/issues/5523) | [FEATURE REQUEST] Alternate font option for non-character... | - | - | - | - | - | Skip | Feature request |
| [#5495](https://gitlab.com/gnachman/iterm2/-/issues/5495) | Split pane + font resize => unpleasant window resizes | 2025-12-26 | - | - | - | Skip (Old) | 2018 issue |
| [#5456](https://gitlab.com/gnachman/iterm2/-/issues/5456) | XTERM 256 color codes not applied or applied incorrectly ... | 2025-12-26 | - | - | - | Skip (Old) | 2018 issue |
| [#5342](https://gitlab.com/gnachman/iterm2/-/issues/5342) | Font and colors are rendered differently in iTerm and Ter... | 2025-12-26 | - | - | - | Skip (Old) | 2018 issue |
| [#5324](https://gitlab.com/gnachman/iterm2/-/issues/5324) | problems with accented characters | 2025-12-26 | - | - | - | Skip (Old) | 2018 issue, vague |
| [#5314](https://gitlab.com/gnachman/iterm2/-/issues/5314) | Can't select top left characters with small fonts in full... | 2025-12-26 | - | - | - | Skip (Old) | 2018 issue |
| [#5220](https://gitlab.com/gnachman/iterm2/-/issues/5220) | Dropping lots of files on DashTerm2 takes minutes to render | 2025-12-26 | - | - | - | Skip (Old) | 2018 issue |
| [#5074](https://gitlab.com/gnachman/iterm2/-/issues/5074) | Highlighting doesn't invert entire character when vertica... | 2025-12-26 | - | - | - | Skip (Old) | 2018 issue |
| [#4982](https://gitlab.com/gnachman/iterm2/-/issues/4982) | Trigger on and capture non-printing characters | - | - | - | - | - | Skip | Feature request |
| [#4938](https://gitlab.com/gnachman/iterm2/-/issues/4938) | iterm2 can't show italic font in code, but macvim gui can... | 2025-12-26 | - | - | - | Skip (Old) | 2017 issue |
| [#4934](https://gitlab.com/gnachman/iterm2/-/issues/4934) | Ligatures deactivate in unfocused split panes | 2025-12-26 | - | - | - | Skip | Old (2017) |
| [#4702](https://gitlab.com/gnachman/iterm2/-/issues/4702) | Double-width characters do not display properly in iTerm 3 | 2025-12-26 | - | - | - | Skip (Old) | 2017 issue |
| [#4650](https://gitlab.com/gnachman/iterm2/-/issues/4650) | Enhancement: Add font and other profile information to Ap... | - | - | - | - | - | Skip | Feature request |
| [#4572](https://gitlab.com/gnachman/iterm2/-/issues/4572) | cannot specify custom font size | 2025-12-26 | - | - | - | Skip (Old) | 2017 issue |
| [#4530](https://gitlab.com/gnachman/iterm2/-/issues/4530) | Feature Request: Separate font settings for specific Unic... | - | - | - | - | - | Skip | Feature request |
| [#4501](https://gitlab.com/gnachman/iterm2/-/issues/4501) | Corrupts unicode characters on wrap in vertical split | 2025-12-26 | - | - | - | Skip (Old) | 2017 issue |
| [#4318](https://gitlab.com/gnachman/iterm2/-/issues/4318) | Wrong character spacing for different widths of Input Mon... | 2025-12-26 | - | - | - | Skip (Old) | 2017 issue |
| [#4199](https://gitlab.com/gnachman/iterm2/-/issues/4199) | DashTerm2 is removing random characters from large pastes | 2025-12-26 | - | - | - | Skip (Old) | 2017 issue |
| [#4074](https://gitlab.com/gnachman/iterm2/-/issues/4074) | Color hex values rendered incorrectly | 2025-12-26 | - | - | - | Skip (Old) | 2017 issue |
| [#4072](https://gitlab.com/gnachman/iterm2/-/issues/4072) | Font line height versus character height incorrect | 2025-12-26 | - | - | - | Skip (Old) | 2017 issue |
| [#4000](https://gitlab.com/gnachman/iterm2/-/issues/4000) | Tabs in files generate spurious character sequence when l... | 2025-12-26 | - | - | - | Skip (Old) | 2017 issue |
| [#3857](https://gitlab.com/gnachman/iterm2/-/issues/3857) | Feature request: Remaining font effects | - | - | - | - | - | Skip | Feature request |
| [#3615](https://gitlab.com/gnachman/iterm2/-/issues/3615) | Hard-coded double-width character table is not flexible e... | - | - | - | - | - | Skip | Feature request |
| [#3508](https://gitlab.com/gnachman/iterm2/-/issues/3508) | New configuration settings: badge location, font and size. | - | - | - | - | - | Skip | Feature request |
| [#3455](https://gitlab.com/gnachman/iterm2/-/issues/3455) | Move "characters considered part of a word for selection"... | - | - | - | - | - | Skip | Feature request |
| [#3403](https://gitlab.com/gnachman/iterm2/-/issues/3403) | Allow users config font fallback | - | - | - | - | - | Skip | Feature request |
| [#3384](https://gitlab.com/gnachman/iterm2/-/issues/3384) | Apple Emoji Font overrides glyphs | 2025-12-26 | - | - | - | Skip | Old (2016) |
| [#3227](https://gitlab.com/gnachman/iterm2/-/issues/3227) | Unicode characters not displayed correct | 2025-12-26 | - | - | - | Skip (Old) | 2017 issue, vague |
| [#3063](https://gitlab.com/gnachman/iterm2/-/issues/3063) | Filenames with Korean characters lead to crazy termimal d... | 2025-12-26 | - | - | - | Skip (Old) | 2017 issue |
| [#3052](https://gitlab.com/gnachman/iterm2/-/issues/3052) | Handle U+200B correctly in the presense of combining char... | 2025-12-26 | - | - | - | Skip | Old (2016) |
| [#2688](https://gitlab.com/gnachman/iterm2/-/issues/2688) | Add a per-profile preference to disable character set swi... | - | - | - | - | - | Skip | Feature request |
| [#2372](https://gitlab.com/gnachman/iterm2/-/issues/2372) | Specify bold font | - | - | - | - | - | Skip | Feature request |
| [#1650](https://gitlab.com/gnachman/iterm2/-/issues/1650) | Different font configuration for full screen mode | - | - | - | - | - | Skip | Feature request |
| [#1607](https://gitlab.com/gnachman/iterm2/-/issues/1607) | Wrong default character spacing | 2025-12-26 | - | - | - | Skip (Old) | 2016 issue |
| [#1578](https://gitlab.com/gnachman/iterm2/-/issues/1578) | print font size and print selection | - | - | - | - | - | Skip | Feature request |
| [#1533](https://gitlab.com/gnachman/iterm2/-/issues/1533) | Check if selection has printable characters before copying | - | - | - | - | - | Skip | Feature request |
| [#1340](https://gitlab.com/gnachman/iterm2/-/issues/1340) | Show numeric character spacing | - | - | - | - | - | Skip | Feature request |
| [#1002](https://gitlab.com/gnachman/iterm2/-/issues/1002) | Req: a "Scrambler" mode for drawn text. | - | - | - | - | - | Skip | Feature request |

---

## Statistics

| Metric | Count |
|--------|-------|
| Total | 189 |
| Fixed | 14 |
| In Progress | 0 |
| Open | 0 |
| Skip (Feature Requests) | 57 |
| Skip (Old/Obsolete) | 104 |
| External | 4 |
| Cannot Reproduce | 10 |
| Wontfix | 0 |

---

## Category Notes

Font and rendering issues fall into several distinct subcategories that require different investigation approaches.

### Common Patterns

1. **Unicode width issues** (#11231, #7901, #7938, #8735, #6337) - Characters displaying with incorrect width, especially emoji and special Unicode characters. Related to `wcwidth` calculations and Unicode version handling.

2. **Ligature problems** (#12231, #11617, #11105, #8971, #8120, #4934) - Font ligatures not rendering, especially with FiraCode and Powerline fonts. May involve font feature detection or GPU renderer.

3. **Box drawing/line characters** (#12657, #11898, #11577, #10459, #6249) - Box drawing characters have gaps or don't connect properly. Often related to custom box drawing vs font glyphs.

4. **Combining characters** (#10444, #9028, #7494, #3052) - Multiple combining marks (diacritics, emoji modifiers) not rendering correctly together.

5. **Emoji rendering** (#11812, #7239, #6243, #6130, #3384) - Emoji with variant selectors, skin tone modifiers, or ZWJ sequences causing display issues.

6. **Metal GPU renderer** (#12464, #6827, #6587) - Rendering issues specific to the Metal renderer including underline gaps, color handling, and GPU power usage.

7. **Non-ASCII/CJK fonts** (#11408, #8813, #7284) - Issues with Chinese, Japanese, Korean characters and non-ASCII font selection.

8. **Nerd Font glyphs** (#11843, #12230, #10133) - Missing or incorrectly rendered glyphs from Nerd Font patches.

### Related Files

- `sources/iTermTextDrawingHelper.m` - Main text rendering
- `sources/iTermCharacterSource.m` - Character/glyph source
- `sources/Metal/iTermMetalRenderer.m` - Metal GPU renderer
- `sources/iTermAdvancedGPUSettingsViewController.m` - GPU settings
- `sources/VT100Terminal.m` - Terminal emulation
- `sources/iTermUnicodeNormalization.m` - Unicode handling
- `sources/iTermBoxDrawingBezierCurveFactory.m` - Box drawing characters
- `sources/PTYTextView.m` - Text view rendering

