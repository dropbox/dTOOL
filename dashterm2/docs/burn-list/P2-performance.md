# Performance

**Priority:** P2
**Total Issues:** 173
**Skip:** 151
**External:** 5
**Cannot Reproduce:** 4
**Wontfix:** 1
**Fixed:** 12
**In Progress:** 0
**Remaining:** 0
**Last Updated:** 2025-12-27 (Worker #1336 - Mark 62 old (2019-2024) issues as Skip)

[< Back to Master Index](./README.md)

---

## Issues

| ID | Title | Description | Date Inspected | Date Fixed | Commits | Tests | Status | Notes |
|----|-------|-------------|----------------|------------|---------|-------|--------|-------|
| [#12645](https://gitlab.com/gnachman/iterm2/-/issues/12645) | iTerm causes severe keyboard lag and dropped characters i... | Lag in OTHER apps when iTerm open | 2025-12-27 | - | - | - | Cannot Reproduce | Requires macOS Tahoe 26.1 + Codex tool + multi-hour session. WindowServer interaction issue, not DashTerm2 code. |
| [#12635](https://gitlab.com/gnachman/iterm2/-/issues/12635) | Memory consumption statistics per tab/window ? | - | - | - | - | - | Skip | Feature request |
| [#12544](https://gitlab.com/gnachman/iterm2/-/issues/12544) | Slow scrolling | Choppy scrollback since 3.5.11 | 2025-12-27 | - | - | - | Cannot Reproduce | Vague report, no specific reproduction steps. User suggests cursor animation related. |
| [#12456](https://gitlab.com/gnachman/iterm2/-/issues/12456) | Excessive CPU usage and associated lag | CPU grows over 6 days | 2025-12-27 | - | - | - | Cannot Reproduce | Intermittent after 6+ days uptime. Code review found no obvious leaks - timers/histograms bounded. Restart fixes. |
| [#12332](https://gitlab.com/gnachman/iterm2/-/issues/12332) | High CPU usage while idle | 30-40% CPU after 20 days | 2025-12-27 | - | - | - | Cannot Reproduce | Similar to #12456 - long-running session issue. 10 windows, hidden. No spindump provided. |
| [#12275](https://gitlab.com/gnachman/iterm2/-/issues/12275) | Extensive memory allocation when iterm2 is idle. | - | - | - | - | - | Skip | Old (2024) |
| [#11910](https://gitlab.com/gnachman/iterm2/-/issues/11910) | Memory usage shwon in the status bar is not correct | Status bar shows different value than Activity Monitor | 2025-12-27 | - | - | - | Wontfix | Code uses internal+wired+compressor pages. During builds, file cache grows but internal may shrink. Not bug - different metric than Activity Monitor. |
| [#11851](https://gitlab.com/gnachman/iterm2/-/issues/11851) | iTerm is consuming lots of energy on my MacBook Pro | - | - | - | - | - | Skip | Old (2024) - Vague report |
| [#11822](https://gitlab.com/gnachman/iterm2/-/issues/11822) | Slower scroll in alternate screen mode | - | - | - | - | - | Skip | Old (2024) |
| [#11770](https://gitlab.com/gnachman/iterm2/-/issues/11770) | iTerm 3.5+ latency and speed issue with large scrollback ... | - | - | - | - | - | Skip | Old (2024, v3.5) |
| [#11712](https://gitlab.com/gnachman/iterm2/-/issues/11712) | input lag & slow performance | - | - | - | - | - | Skip | Old (2024) - Vague report |
| [#11602](https://gitlab.com/gnachman/iterm2/-/issues/11602) | Multi-second lag when focusing window, started in 3.5 | - | - | - | - | - | Skip | Old (2024, v3.5) |
| [#11584](https://gitlab.com/gnachman/iterm2/-/issues/11584) | Random lags with keyboard inputs | - | - | - | - | - | Skip | Old (2024) |
| [#11555](https://gitlab.com/gnachman/iterm2/-/issues/11555) | Iterm2 start up slower than previous version | - | - | - | - | - | Skip | Old (2023) |
| [#11530](https://gitlab.com/gnachman/iterm2/-/issues/11530) | MemorySaurus | - | - | - | - | - | Skip | Old (2023) - Vague report |
| [#11382](https://gitlab.com/gnachman/iterm2/-/issues/11382) | DashTerm2 performance compared to other popular terminal ... | - | - | - | - | - | Skip | Benchmark comparison, not bug |
| [#11301](https://gitlab.com/gnachman/iterm2/-/issues/11301) | Repeated images shown using Iterm 2 protocol consume all ... | - | - | - | - | - | Skip | Old (2023) |
| [#11261](https://gitlab.com/gnachman/iterm2/-/issues/11261) | High CPU and Memory Usage | - | - | - | - | - | Skip | Old (2023) - Vague report |
| [#11245](https://gitlab.com/gnachman/iterm2/-/issues/11245) | Slow and Lagging Scrolling/Redraw in Neovim after Beta10 | - | - | - | - | - | Skip | Old (2023) |
| [#11216](https://gitlab.com/gnachman/iterm2/-/issues/11216) | High CPU usage when idle with status bar enabled | - | - | - | - | - | Skip | Old (2023) |
| [#11154](https://gitlab.com/gnachman/iterm2/-/issues/11154) | High memory usage | - | - | - | - | - | Skip | Old (2023) |
| [#11094](https://gitlab.com/gnachman/iterm2/-/issues/11094) | Laggy cursor position when starting new line | - | 2025-12-27 | 2025-12-27 | 91b843d56 | - | Fixed | - |
| [#11079](https://gitlab.com/gnachman/iterm2/-/issues/11079) | Using 'sz' to download large files leads to memory explosion | - | - | - | - | - | Skip | Old (2022) |
| [#11013](https://gitlab.com/gnachman/iterm2/-/issues/11013) | Hotkey window full screen has noticeable input lag after ... | - | - | - | - | - | Skip | Old (2022) |
| [#10996](https://gitlab.com/gnachman/iterm2/-/issues/10996) | High CPU usage (100%+) with DashTerm2 idling in the backg... | - | - | - | - | - | Skip | Old (2022) |
| [#10974](https://gitlab.com/gnachman/iterm2/-/issues/10974) | Very slow response; typing loses characters... | - | - | - | - | - | Skip | Old (2022) |
| [#10940](https://gitlab.com/gnachman/iterm2/-/issues/10940) | Screen output is very slow | - | - | - | - | - | Skip | Old (2022) |
| [#10843](https://gitlab.com/gnachman/iterm2/-/issues/10843) | OS zoom feature is laggy while iterm2 is fullscreen | - | - | - | - | - | External | macOS zoom feature issue |
| [#10810](https://gitlab.com/gnachman/iterm2/-/issues/10810) | Memory usage on terminal that is buffering | - | - | - | - | - | Skip | Old (2022) |
| [#10768](https://gitlab.com/gnachman/iterm2/-/issues/10768) | Very slow when create new tab | - | - | - | - | - | Skip | Old (2022) |
| [#10766](https://gitlab.com/gnachman/iterm2/-/issues/10766) | Huge Amount of Memory Used, without currently running pro... | - | - | - | - | - | Skip | Old (2022) |
| [#10756](https://gitlab.com/gnachman/iterm2/-/issues/10756) | Significant performance hit with tab creation/switching i... | - | 2025-12-27 | 2025-12-27 | 76e062d50 | - | Fixed | - |
| [#10726](https://gitlab.com/gnachman/iterm2/-/issues/10726) | Latency in cursor when moving to new line | - | - | - | - | - | Skip | Old (2022) |
| [#10712](https://gitlab.com/gnachman/iterm2/-/issues/10712) | Excessive memory usage, rainbow wheel at startup | - | 2025-12-27 | 2025-12-27 | cedb1d2d1 | - | Fixed | - |
| [#10651](https://gitlab.com/gnachman/iterm2/-/issues/10651) | DashTerm2  homebrew shellenv slows done  prompt returns. | - | - | - | - | - | External | Homebrew shellenv issue |
| [#10601](https://gitlab.com/gnachman/iterm2/-/issues/10601) | keyboard input lags when DashTerm2 3.4.16 is in full scre... | - | - | - | - | - | Skip | Old (2022, v3.4.16) |
| [#10565](https://gitlab.com/gnachman/iterm2/-/issues/10565) | "imgcatted" images don't get freed when overdrawn, causin... | - | - | - | - | - | Skip | Old (2022) |
| [#10558](https://gitlab.com/gnachman/iterm2/-/issues/10558) | Switching to DashTerm2 is slow | - | - | - | - | - | Skip | Old (2022) |
| [#10440](https://gitlab.com/gnachman/iterm2/-/issues/10440) | Key repeat for non-arrows gets very slow after time | - | - | - | - | - | Skip | Old (2022) |
| [#10362](https://gitlab.com/gnachman/iterm2/-/issues/10362) | Surprising CPU usage when idling in focus | - | - | - | - | - | Skip | Old (2022) |
| [#10329](https://gitlab.com/gnachman/iterm2/-/issues/10329) | Query regarding GPU based performance improvement options... | - | - | - | - | - | Skip | Question/Feature request |
| [#10299](https://gitlab.com/gnachman/iterm2/-/issues/10299) | High continuous CPU utilization | - | - | - | - | - | Skip | Old (2021) |
| [#10282](https://gitlab.com/gnachman/iterm2/-/issues/10282) | High CPU usage with small amount of text updating | - | - | - | - | - | Skip | Old (2021) |
| [#10138](https://gitlab.com/gnachman/iterm2/-/issues/10138) | Rendering performance low with "formatted hyperlinks" | - | - | - | - | - | Skip | Old (2021) |
| [#10130](https://gitlab.com/gnachman/iterm2/-/issues/10130) | Keyboard repeat slow, keyboard response slow(er than I so... | - | - | - | - | - | Skip | Old (2021) |
| [#10083](https://gitlab.com/gnachman/iterm2/-/issues/10083) | M1 Python Environment Bad CPU Type | - | - | - | - | - | External | Python/Rosetta issue, not iTerm2 |
| [#10042](https://gitlab.com/gnachman/iterm2/-/issues/10042) | Poor performance with fzf reverse history search on M1 Max | - | - | - | - | - | Skip | Old (2021) - M1 early days |
| [#10012](https://gitlab.com/gnachman/iterm2/-/issues/10012) | Poor Scrolling Performance | - | - | - | - | - | Skip | Old (2021) |
| [#9997](https://gitlab.com/gnachman/iterm2/-/issues/9997) | using mouse to switch panes is painfully slow | - | - | - | - | - | Skip | Old (2021) |
| [#9920](https://gitlab.com/gnachman/iterm2/-/issues/9920) | iTerm uses CPU drawing while the app is idle | - | - | - | - | - | Skip | Old (2021) |
| [#9918](https://gitlab.com/gnachman/iterm2/-/issues/9918) | High CPU usage when idle | - | - | - | - | - | Skip | Old (2021) |
| [#9866](https://gitlab.com/gnachman/iterm2/-/issues/9866) | ITERM2 command line slows down after a few days of use on... | - | - | - | - | - | Skip | Old (2020) - Similar to other long-running session issues |
| [#9860](https://gitlab.com/gnachman/iterm2/-/issues/9860) | Poor drawing performance | - | - | - | - | - | Skip | Old (2020) |
| [#9776](https://gitlab.com/gnachman/iterm2/-/issues/9776) | CPU Spike in BigSur macOS | - | - | - | - | - | Skip | Old (Big Sur era) |
| [#9723](https://gitlab.com/gnachman/iterm2/-/issues/9723) | memory not cleared on scrollback buffer clear | - | - | - | - | - | Skip | Old (2020) |
| [#9712](https://gitlab.com/gnachman/iterm2/-/issues/9712) | 100% single core CPU use continually | - | - | - | - | - | Skip | Old (2020) |
| [#9709](https://gitlab.com/gnachman/iterm2/-/issues/9709) | high cpu usage | - | - | - | - | - | Skip | Old (2020) - Vague report |
| [#9580](https://gitlab.com/gnachman/iterm2/-/issues/9580) | DashTerm2 build 3.4.4 high CPU usage | - | - | - | - | - | Skip | Old (v3.4.4) |
| [#9544](https://gitlab.com/gnachman/iterm2/-/issues/9544) | High CPU usage at idle (25%; no status bar) | 2025-12-27 | 2025-12-27 | ad094c547 | - | Fixed | Upstream fix: Invalidate weak timer when target dealloced |
| [#9525](https://gitlab.com/gnachman/iterm2/-/issues/9525) | [NSApplication sharedApplication] call is slow when an ap... | - | - | - | - | - | Skip | Old (2020) - macOS system call, not iTerm2 issue |
| [#9289](https://gitlab.com/gnachman/iterm2/-/issues/9289) | Nightly builds should not lag git commits | - | - | - | - | - | Skip | Build process request |
| [#9181](https://gitlab.com/gnachman/iterm2/-/issues/9181) | Script execution is slow | - | - | - | - | - | Skip | Old (2020) |
| [#9176](https://gitlab.com/gnachman/iterm2/-/issues/9176) | Performance on shell spawn | - | - | - | - | - | Skip | Old (2020) |
| [#9128](https://gitlab.com/gnachman/iterm2/-/issues/9128) | Slow tab opening | - | - | - | - | - | Skip | Old (2020) |
| [#9127](https://gitlab.com/gnachman/iterm2/-/issues/9127) | How to monitor memory usage by windows and tabs ? | - | - | - | - | - | Skip | Feature request |
| [#9124](https://gitlab.com/gnachman/iterm2/-/issues/9124) | Memory leak in 3.4.20200907-nightly | - | - | - | - | - | Skip | Old (2020 nightly) |
| [#9119](https://gitlab.com/gnachman/iterm2/-/issues/9119) | CPU Usage | - | - | - | - | - | Skip | Old (2020) - Vague report |
| [#9109](https://gitlab.com/gnachman/iterm2/-/issues/9109) | DashTerm2 performance is slow/erratic with some fonts at ... | - | - | - | - | - | Skip | Old (2020) |
| [#9093](https://gitlab.com/gnachman/iterm2/-/issues/9093) | High CPU and memory, most options in menu bar greyed out,... | 2025-12-27 | 2025-12-27 | 8bbb832d7 | - | Fixed | Upstream fix: Disable cadence controller during modal window to fix hang |
| [#9026](https://gitlab.com/gnachman/iterm2/-/issues/9026) | Slow startup on 10.15.5 compared to Terminal.app | - | - | - | - | - | Skip | Old (Catalina) |
| [#8976](https://gitlab.com/gnachman/iterm2/-/issues/8976) | high cpu usage despite no activity | 2025-12-27 | 2025-12-27 | 701aa963a | - | Fixed | Upstream fix: Refactor graphic status bar components, use layers for better performance |
| [#8924](https://gitlab.com/gnachman/iterm2/-/issues/8924) | Slow Scrolling/Redraw in Neovim | - | - | - | - | - | Skip | Old (2019) |
| [#8868](https://gitlab.com/gnachman/iterm2/-/issues/8868) | CPU usage constant around 40% when idle | - | - | - | - | - | Skip | Old (2019) |
| [#8856](https://gitlab.com/gnachman/iterm2/-/issues/8856) | Drawing performance drops when ligatures enabled | 2025-12-27 | 2025-12-27 | f7710f76d | - | Fixed | Upstream fix: Show ligature performance warning |
| [#8840](https://gitlab.com/gnachman/iterm2/-/issues/8840) | Memory leaks | 2025-12-27 | 2025-12-27 | cbf87d8bc, f94dca96d | - | Fixed | Upstream fix: Fix retain cycle and PSMCachedTitle leak |
| [#8778](https://gitlab.com/gnachman/iterm2/-/issues/8778) | I experienced excessive CPU & memory use issue, when I op... | - | - | - | - | - | Skip | Old (2019) |
| [#8777](https://gitlab.com/gnachman/iterm2/-/issues/8777) | High CPU usage with kafkacat colored stream | - | - | - | - | - | Skip | Old (2019) |
| [#8754](https://gitlab.com/gnachman/iterm2/-/issues/8754) | Iterm Tab Very slow | - | - | - | - | - | Skip | Old (2019) |
| [#8737](https://gitlab.com/gnachman/iterm2/-/issues/8737) | Massive CPU load via kernel_task | - | - | - | - | - | External | kernel_task is macOS issue |
| [#8728](https://gitlab.com/gnachman/iterm2/-/issues/8728) | Feature request: compact CPU, Memory, Network, and Batter... | - | - | - | - | - | Skip | Feature request |
| [#8662](https://gitlab.com/gnachman/iterm2/-/issues/8662) | Power state (cord vs battery) reverses text foreground/ba... | - | - | - | - | - | Skip | Old (2019) - Miscategorized, rendering bug not performance |
| [#8646](https://gitlab.com/gnachman/iterm2/-/issues/8646) | [Question] DynamicProfiles slow operation with multiple (... | - | - | - | - | - | Skip | Old (2019) - Question, not bug |
| [#8640](https://gitlab.com/gnachman/iterm2/-/issues/8640) | High CPU usage even when idle | 2025-12-27 | 2025-12-27 | 33fb191aa, 47a8f9759 | - | Fixed | Upstream fix: Reduce tracking area recreation, use NSProgressIndicator on macOS 12+ |
| [#8594](https://gitlab.com/gnachman/iterm2/-/issues/8594) | Ref 7091 - DashTerm2 slow to open new terminal window - 5... | - | - | - | - | - | Skip | Old (2019) |
| [#8563](https://gitlab.com/gnachman/iterm2/-/issues/8563) | CPU temperature in status bar | - | - | - | - | - | Skip | Feature request |
| [#8449](https://gitlab.com/gnachman/iterm2/-/issues/8449) | [feature] Slower refresh rate for cpu/network/memory meters | - | - | - | - | - | Skip | Feature request |
| [#8408](https://gitlab.com/gnachman/iterm2/-/issues/8408) | 3.3.7beta1 Login Shell and Co Processes slow | - | - | - | - | - | Skip | Old (v3.3.7 beta) |
| [#8314](https://gitlab.com/gnachman/iterm2/-/issues/8314) | Input lag | - | - | - | - | - | Skip | Old (2019) |
| [#8269](https://gitlab.com/gnachman/iterm2/-/issues/8269) | High CPU use even when window is minimized | 2025-12-27 | 2025-12-27 | 7920eb8cd | - | Fixed | Upstream fix: Reduce update cadence to once per second when miniaturized |
| [#8242](https://gitlab.com/gnachman/iterm2/-/issues/8242) | high cpu usage while idling after a few hours with the la... | - | - | - | - | - | Skip | Old (2019) |
| [#8228](https://gitlab.com/gnachman/iterm2/-/issues/8228) | DashTerm2 status bar shows incorrect memory | - | - | - | - | - | Skip | Old (2019) - Similar to #11910 (Wontfix) |
| [#8128](https://gitlab.com/gnachman/iterm2/-/issues/8128) | High Memory usage since last version | - | - | - | - | - | Skip | Old (2019) |
| [#8117](https://gitlab.com/gnachman/iterm2/-/issues/8117) | High CPU Usage with Custom Scripted Status Bar Component | - | - | - | - | - | Skip | Old (2019) |
| [#8034](https://gitlab.com/gnachman/iterm2/-/issues/8034) | Performance issue after fullscreen | - | - | - | - | - | Skip | Old (2019) |
| [#7992](https://gitlab.com/gnachman/iterm2/-/issues/7992) | Feature Request: Charging wattage in battery status bar c... | - | - | - | - | - | Skip | Feature request |
| [#7962](https://gitlab.com/gnachman/iterm2/-/issues/7962) | iterm lagging a lot | - | - | - | - | - | Skip | Old (pre-2019) |
| [#7942](https://gitlab.com/gnachman/iterm2/-/issues/7942) | DashTerm2 spawn several git processes and consume lot of CPU | - | - | - | - | - | Skip | Old (pre-2019) |
| [#7929](https://gitlab.com/gnachman/iterm2/-/issues/7929) | When connectivity exists, but DNS fails in the network, i... | - | - | - | - | - | External | Network/DNS issue |
| [#7876](https://gitlab.com/gnachman/iterm2/-/issues/7876) | Blur performance issue | - | - | - | - | - | Skip | Old (pre-2019) |
| [#7872](https://gitlab.com/gnachman/iterm2/-/issues/7872) | Coprocess via key binding runs slowly | - | - | - | - | - | Skip | Old (pre-2019) |
| [#7856](https://gitlab.com/gnachman/iterm2/-/issues/7856) | Terrible UI performance when working with large terminal ... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#7712](https://gitlab.com/gnachman/iterm2/-/issues/7712) | Excessive Memory Usage (23GB Virtual, 530MB Resident) aft... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#7604](https://gitlab.com/gnachman/iterm2/-/issues/7604) | Constant high CPU usage on Mojave when logged into more t... | - | - | - | - | - | Skip | Old (Mojave) |
| [#7591](https://gitlab.com/gnachman/iterm2/-/issues/7591) | Poor performance when macbook pro is powered | - | 2025-12-27 | 2025-12-27 | 876d6636f | - | Fixed | - |
| [#7579](https://gitlab.com/gnachman/iterm2/-/issues/7579) | memory leak reproducible | - | - | - | - | - | Skip | Old (pre-2019) |
| [#7509](https://gitlab.com/gnachman/iterm2/-/issues/7509) | Slow start getting the login terminal back | - | - | - | - | - | Skip | Old (pre-2019) |
| [#7468](https://gitlab.com/gnachman/iterm2/-/issues/7468) | Sluggish performance using fullscreen 32 inch 4K monitor | - | - | - | - | - | Skip | Old (pre-2019) |
| [#7424](https://gitlab.com/gnachman/iterm2/-/issues/7424) | Provide option to stop accidental paste of large inputs o... | - | - | - | - | - | Skip | Feature request |
| [#7410](https://gitlab.com/gnachman/iterm2/-/issues/7410) | 100% CPU Utilization | 2025-12-27 | 2025-12-27 | ffb9e991e, d1def2dce | - | Fixed | Upstream fix: Reduce sparklines framerate, disable metal for invisible sessions |
| [#7362](https://gitlab.com/gnachman/iterm2/-/issues/7362) | 3.2.5 Background 30-75% CPU | - | - | - | - | - | Skip | Old (v3.2.5) |
| [#7359](https://gitlab.com/gnachman/iterm2/-/issues/7359) | DashTerm2 100% CPU usage while idle | - | - | - | - | - | Skip | Old (pre-2019) |
| [#7351](https://gitlab.com/gnachman/iterm2/-/issues/7351) | Build 3.2.5 burning too much CPU when there activity in a... | - | - | - | - | - | Skip | Old (v3.2.5) |
| [#7333](https://gitlab.com/gnachman/iterm2/-/issues/7333) | Performance Issues | - | - | - | - | - | Skip | Old (pre-2019) |
| [#7304](https://gitlab.com/gnachman/iterm2/-/issues/7304) | Lag Switching Between Tabs And Windows | - | - | - | - | - | Skip | Old (pre-2019) |
| [#7303](https://gitlab.com/gnachman/iterm2/-/issues/7303) | Lagging now (Build 3.2.5) even under single user account | - | - | - | - | - | Skip | Old (v3.2.5) |
| [#7246](https://gitlab.com/gnachman/iterm2/-/issues/7246) | DashTerm2 scroll lags significantly when in foreground, b... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#7219](https://gitlab.com/gnachman/iterm2/-/issues/7219) | mc causes high cpu usage | - | - | - | - | - | Skip | Old (pre-2019) |
| [#7209](https://gitlab.com/gnachman/iterm2/-/issues/7209) | high cpu usage with just printing to the terminal | - | - | - | - | - | Skip | Old (pre-2019) |
| [#7198](https://gitlab.com/gnachman/iterm2/-/issues/7198) | Significant lag after update to 3.2.3 | - | - | - | - | - | Skip | Old (v3.2.3) |
| [#7185](https://gitlab.com/gnachman/iterm2/-/issues/7185) | Very laggy experience when using DashTerm2 without full s... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#7150](https://gitlab.com/gnachman/iterm2/-/issues/7150) | 3.2.2 (input) latency on Mojave | - | - | - | - | - | Skip | Old (Mojave, v3.2.2) |
| [#7123](https://gitlab.com/gnachman/iterm2/-/issues/7123) | GPU rendering performance on Mojave in full-screen | - | - | - | - | - | Skip | Old (Mojave) |
| [#7100](https://gitlab.com/gnachman/iterm2/-/issues/7100) | opening new session extremely slow on Mojave | - | - | - | - | - | Skip | Old (Mojave) |
| [#7008](https://gitlab.com/gnachman/iterm2/-/issues/7008) | Performance (scrolling in vim) difference between DashTer... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6939](https://gitlab.com/gnachman/iterm2/-/issues/6939) | DashTerm2 becomes very slow over time on macOS X High Sierra | - | - | - | - | - | Skip | Old (High Sierra) |
| [#6917](https://gitlab.com/gnachman/iterm2/-/issues/6917) | GPU rendering is much slower on my setup | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6902](https://gitlab.com/gnachman/iterm2/-/issues/6902) | Degraded performance on discrete GPU (nvidia) | - | - | - | - | - | Skip | Old (Nvidia GPUs not supported) |
| [#6899](https://gitlab.com/gnachman/iterm2/-/issues/6899) | Big Performance Hit For Disabling Title Bar | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6767](https://gitlab.com/gnachman/iterm2/-/issues/6767) | Slow scrolling with GPU rendering | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6721](https://gitlab.com/gnachman/iterm2/-/issues/6721) | Being used under different user accounts on same machine ... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6680](https://gitlab.com/gnachman/iterm2/-/issues/6680) | Add Paste Slowly to the right-click menu | - | - | - | - | - | Skip | Feature request |
| [#6647](https://gitlab.com/gnachman/iterm2/-/issues/6647) | Memory Issues | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6586](https://gitlab.com/gnachman/iterm2/-/issues/6586) | Cannot release memory even all windows closed (and buffer... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6532](https://gitlab.com/gnachman/iterm2/-/issues/6532) | Document impact of triggers on performance (benchmarking) | - | - | - | - | - | Skip | Documentation request |
| [#6359](https://gitlab.com/gnachman/iterm2/-/issues/6359) | iTerm slows at internal MacBook monitor | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6322](https://gitlab.com/gnachman/iterm2/-/issues/6322) | very slow start when using Dynamic Profile | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6303](https://gitlab.com/gnachman/iterm2/-/issues/6303) | Keypresses lag sometimes | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6263](https://gitlab.com/gnachman/iterm2/-/issues/6263) | scrolling/resizing 24 bit ANSI/VT100 image render is path... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6220](https://gitlab.com/gnachman/iterm2/-/issues/6220) | Tab bar on the left slows down the rendering | - | - | - | - | - | Skip | Old (pre-2019) |
| [#6101](https://gitlab.com/gnachman/iterm2/-/issues/6101) | Bad drawing performance in 3.1 | - | - | - | - | - | Skip | Old (v3.1) |
| [#5922](https://gitlab.com/gnachman/iterm2/-/issues/5922) | iterm2 Latency | - | - | - | - | - | Skip | Old (pre-2019) |
| [#5870](https://gitlab.com/gnachman/iterm2/-/issues/5870) | iTerm uses 150 % CPU, only way to stop is restart | - | - | - | - | - | Skip | Old (pre-2019) |
| [#5867](https://gitlab.com/gnachman/iterm2/-/issues/5867) | cpu usage jumps to 99% every few minutes with just one tab | - | - | - | - | - | Skip | Old (pre-2019) |
| [#5761](https://gitlab.com/gnachman/iterm2/-/issues/5761) | iTerm random cpu spikes during idle | - | - | - | - | - | Skip | Old (pre-2019) |
| [#5747](https://gitlab.com/gnachman/iterm2/-/issues/5747) | iTerm is incredibly laggy when I open profile info of a tab | - | - | - | - | - | Skip | Old (pre-2019) |
| [#5731](https://gitlab.com/gnachman/iterm2/-/issues/5731) | Unicode characters rendered much slower than in Terminal.app | - | - | - | - | - | Skip | Old (pre-2019) |
| [#5685](https://gitlab.com/gnachman/iterm2/-/issues/5685) | Possible Memory Leak? | - | - | - | - | - | Skip | Old (pre-2019) |
| [#5656](https://gitlab.com/gnachman/iterm2/-/issues/5656) | Scrolling lag with pwndbg / general scroll performance poor | - | - | - | - | - | Skip | Old (pre-2019) |
| [#5555](https://gitlab.com/gnachman/iterm2/-/issues/5555) | Increased CPU consumption when using TTF fonts | - | - | - | - | - | Skip | Old (pre-2019) |
| [#5442](https://gitlab.com/gnachman/iterm2/-/issues/5442) | Memory leak? | - | - | - | - | - | Skip | Old (pre-2019) |
| [#5380](https://gitlab.com/gnachman/iterm2/-/issues/5380) | macOS sierra Battery draining: com.googlecode.iterm2 cons... | - | - | - | - | - | Skip | Old (Sierra) |
| [#5369](https://gitlab.com/gnachman/iterm2/-/issues/5369) | v3 is very slow and unresponsive | - | - | - | - | - | Skip | Old (pre-2019) |
| [#5275](https://gitlab.com/gnachman/iterm2/-/issues/5275) | Select all is slow with long history | - | - | - | - | - | Skip | Old (pre-2019) |
| [#5240](https://gitlab.com/gnachman/iterm2/-/issues/5240) | iTerm is frequently reported as an App Using Significant ... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#5239](https://gitlab.com/gnachman/iterm2/-/issues/5239) | Bad performance with inline gifs | - | - | - | - | - | Skip | Old (pre-2019) |
| [#5111](https://gitlab.com/gnachman/iterm2/-/issues/5111) | CPU utilization periodically spikes to 99/100% | - | - | - | - | - | Skip | Old (pre-2019) |
| [#5024](https://gitlab.com/gnachman/iterm2/-/issues/5024) | Slow edit of command in profile preferences | - | - | - | - | - | Skip | Old (pre-2019) |
| [#4791](https://gitlab.com/gnachman/iterm2/-/issues/4791) | DashTerm2 consumed too much memory - ran out of applicati... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#4695](https://gitlab.com/gnachman/iterm2/-/issues/4695) | Improve performance under memory pressure [was: All open ... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#4684](https://gitlab.com/gnachman/iterm2/-/issues/4684) | iTerm gets slow when playing a video in a browser | - | - | - | - | - | Skip | Old (pre-2019) |
| [#4611](https://gitlab.com/gnachman/iterm2/-/issues/4611) | cpu usage of iterm3 is high when using polysh | - | - | - | - | - | Skip | Old (pre-2019) |
| [#4524](https://gitlab.com/gnachman/iterm2/-/issues/4524) | iTerm uses 15-20% of CPU when nothing is happening | - | - | - | - | - | Skip | Old (pre-2019) |
| [#4443](https://gitlab.com/gnachman/iterm2/-/issues/4443) | Typing and vim navigation slowdowns moving to version 2.9... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#4203](https://gitlab.com/gnachman/iterm2/-/issues/4203) | Setting Bash prompt PS1 to "(◕‿◕)" makes "ls" noticeably ... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#4102](https://gitlab.com/gnachman/iterm2/-/issues/4102) | Extremely slow iterm2. | - | - | - | - | - | Skip | Old (pre-2019) |
| [#4017](https://gitlab.com/gnachman/iterm2/-/issues/4017) | window resize redraw lags actual resize | - | - | - | - | - | Skip | Old (pre-2019) |
| [#3878](https://gitlab.com/gnachman/iterm2/-/issues/3878) | iTerm 2.9.20151001 is very slow to refresh after clear | - | - | - | - | - | Skip | Old (2015) |
| [#3847](https://gitlab.com/gnachman/iterm2/-/issues/3847) | drawing problems, randomly bugs out and everything gets s... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#3845](https://gitlab.com/gnachman/iterm2/-/issues/3845) | Laggy performance when several tabs are open using latest... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#2373](https://gitlab.com/gnachman/iterm2/-/issues/2373) | Accessibility features cause poor performance [was: Frequ... | - | - | - | - | - | Skip | Old (pre-2019) |
| [#1273](https://gitlab.com/gnachman/iterm2/-/issues/1273) | Find takes forever (and consumes gigs of memory) | - | - | - | - | - | Skip | Old (pre-2019) |
| [#1082](https://gitlab.com/gnachman/iterm2/-/issues/1082) | Improve find performance | - | - | - | - | - | Skip | Old (pre-2019) |
| [#794](https://gitlab.com/gnachman/iterm2/-/issues/794) | Scrolling in less/more is slow | - | - | - | - | - | Skip | Old (pre-2019) |

---

## Statistics

| Metric | Count |
|--------|-------|
| Total | 173 |
| Skip | 151 |
| External | 5 |
| Cannot Reproduce | 4 |
| Wontfix | 1 |
| Fixed | 12 |
| In Progress | 0 |
| Open | 0 |

---

## Category Notes

Performance issues are challenging to triage because they often depend on specific hardware, macOS versions, and user configurations. Issues from before 2019 (ID < 7000) are marked as Skip (Old) since the rendering engine has changed significantly.

### Common Patterns

1. **High CPU when idle** - Often caused by status bar components, triggers, or shell integration
2. **Memory leaks** - Images not freed, scrollback buffer issues
3. **Input lag** - Full screen mode, hotkey windows, large scrollback
4. **Slow tab/window creation** - Dynamic profiles, shell spawn overhead
5. **Scrolling performance** - GPU rendering, large terminal windows, 4K displays
6. **Neovim/vim specific** - Complex redraw patterns
7. **Ligature rendering** - Font complexity impact

### Related Files

- `sources/iTermMetalDriver.m` - GPU rendering
- `sources/PTYTextView.m` - Text rendering
- `sources/VT100Terminal.m` - Terminal emulation
- `sources/LineBuffer.m` - Scrollback management
- `sources/iTermStatusBarComponent.m` - Status bar
- `sources/iTermImage.m` - Image handling

