# DashTerm2 Master Roadmap

**Date:** January 2, 2026
**Status:** Active Development - Visual QA Phase
**Manager:** Andrew Yates

---

## Quick Links

| Document | Purpose |
|----------|---------|
| [VISION.md](./VISION.md) | Product vision and philosophy |
| [worker-backlog.md](./worker-backlog.md) | Current worker task queue |
| [burn-list/README.md](./burn-list/README.md) | Bug triage status |
| [GPU-RENDERER-INTEGRATION.md](./GPU-RENDERER-INTEGRATION.md) | GPU renderer Swift integration |
| [future/HIGH-PERF-MCP-DESIGN.md](./future/HIGH-PERF-MCP-DESIGN.md) | MCP server design (deprioritized) |
| [HINTS_HISTORY.log](../HINTS_HISTORY.log) | Manager directives history |

## Archived Documents

Historical roadmaps and directives are in [`archive/roadmaps/`](./archive/roadmaps/):

| Document | Summary |
|----------|---------|
| [POLISHING-ROADMAP.md](./archive/roadmaps/POLISHING-ROADMAP.md) | Optimization work (92% complete) |
| [DTERM-CORE-PARITY-ROADMAP.md](./archive/roadmaps/DTERM-CORE-PARITY-ROADMAP.md) | Feature parity checklist (complete) |
| [DTERM-AI-DIRECTIVE-V3.md](./archive/roadmaps/DTERM-AI-DIRECTIVE-V3.md) | Deep integration directive (complete) |

---

## Current State (Jan 2, 2026)

### Build Status

**Status:** BUILD SUCCEEDS

**Pre-existing non-blocking issues:**
1. `WebExtensionsFramework` - Architecture mismatch (arm64 only, build wants x86_64 too)
2. `DashTerm2ImportStatus` - Info.plist processing error
3. UI tests cannot run without code signing (expected)

### Test Suite

| Metric | Value |
|--------|-------|
| Total Tests | 4967 |
| Skipped | 61 (keychain, expected) |
| Unexpected Failures | 0 |

### Crash Reports

All crash reports in `worker_logs/app_crashes/` are from Dec 28-30 and predate fixes in commits #1680+.
No new crashes have occurred since the recursive lock fix in #1680.

### Upstream Sync

**Status:** FULLY SYNCHRONIZED
**Last upstream commit:** `d893436` (Add the ability to bind a color to an expression)
**Reference repo:** `~/DashTerm2-reference`

---

## Phase Summary

| Phase | Status | Description |
|-------|--------|-------------|
| **Phase 1: Stability** | COMPLETE | 3,348 upstream bugs triaged, 367 fixed |
| **Phase 2: Core Engine** | COMPLETE | dterm-core Rust library built |
| **Phase 3: Deep Integration** | COMPLETE | GPU renderer, parser switchover |
| **Phase 4: AI-Native** | READY | In-process AI interface designed |
| **Phase 5: Platform Expansion** | FUTURE | iOS, Linux, Windows |

---

## Phase 1: Stability Hardening - COMPLETE

All upstream iTerm2 bugs have been triaged. See [burn-list/README.md](./burn-list/README.md).

| Category | Total | Fixed | Skip | External | Cannot Repro |
|----------|-------|-------|------|----------|--------------|
| P0 Crashes | 289 | 98 | 118 | 11 | 62 |
| P1 Core | 363 | 49 | 180 | 32 | 98 |
| P2 Features | 1484 | 84 | 1340 | 27 | 41 |
| P3 Minor | 1212 | 78 | 1105 | 19 | 15 |
| **TOTAL** | **3348** | **367** | **2743** | **93** | **155** |

**Key metrics achieved:**
- `assert(NO)` crashes: 0 (all fixed)
- `it_fatalError` crashes: 12 (all intentional)
- Force unwraps: 0 (all fixed)
- `objectAtIndex:` unguarded: 0 (all guarded)
- `@synchronized` in production: 0 (migrated to os_unfair_lock)

---

## Phase 2: Core Engine Rebuild - COMPLETE

dterm-core is a Rust library providing terminal emulation. It lives in `~/dterm/`.

### Performance Achieved

| Metric | Target | Achieved |
|--------|--------|----------|
| ASCII throughput | 400 MB/s | ~580 MiB/s |
| SGR throughput | 150 MB/s | ~267 MiB/s |
| Memory (10K lines) | 5 MB | 0.45 MB |
| Test coverage | 1000+ | 1399 tests |
| Fuzzing | No panics | 1M iterations |

### Components

| Component | Status |
|-----------|--------|
| VT100/xterm parser | DONE |
| Screen buffer (8-byte cells) | DONE |
| Scrollback (tiered storage) | DONE |
| Image protocols (Sixel, Kitty, iTerm2) | DONE |
| Shell integration (OSC 133) | DONE |
| FFI bridge (889 symbols) | DONE |
| Search (trigram + bloom) | DONE |

---

## Phase 3: Deep Integration - COMPLETE

### 3.1 Parser Switchover - COMPLETE

dterm-core parser runs in parallel with validation enabled by default.

```
[x] dtermCoreEnabled: YES (default)
[x] dtermCoreParserComparisonEnabled: YES (default)
[x] dtermCoreParserOutputEnabled: YES (default)
[x] vttest conformance tests pass
```

### 3.2 Terminal State Migration - COMPLETE

```
[x] Shell integration (OSC 133) via FFI
[x] Output blocks and exit codes accessible
[x] Visible grid reads from dterm-core
[x] Scrollback reads from dterm-core
[x] 4967 tests pass
```

### 3.3 GPU Renderer - COMPLETE

**Problem:** ObjC Metal stack (`iTermMetalView`, `iTermMetalDriver`, `iTermPromise`) has fundamental concurrency bugs (dispatch_group crashes).

**Solution:** Move GPU rendering to dterm-core where Rust's ownership model prevents these bugs.

#### dterm-core (Rust) Components

| Component | Status |
|-----------|--------|
| Frame sync (Rust channels) | DONE |
| FFI bindings (frame sync) | DONE |
| FFI bindings (GPU renderer) | DONE |
| GlyphAtlas (fontdue + guillotiere) | DONE |
| Vertex buffer builder | DONE |
| WGSL shaders | DONE |
| Basic wgpu render pass | DONE |
| Cursor animation | DONE (basic) |
| Selection highlighting | DONE (basic) |
| Image rendering | Swift side DONE (#1684-1687) |

#### DashTerm2 (Swift) Integration

| Component | Status |
|-----------|--------|
| DTermRenderer (frame sync wrapper) | DONE |
| DTermMetalView (display link) | DONE |
| dterm.h header update (GPU types) | DONE (#1661) |
| DTermGpuRenderer Swift wrapper | DONE (#1661) |
| DTermGpuRenderError enum | DONE (#1661) |
| DTermDamageRegion struct | DONE (#1661) |
| Font data bridge FFI (dterm_gpu_renderer_set_font) | DONE (#1665) |
| DTermAtlasConfig Swift wrapper | DONE (#1665) |
| NSFont extraction (extractFontData) | DONE (#1665) |
| DTermHybridRenderer Swift wrapper | DONE (#1667) |
| DTermMetalView PTYSession integration | DONE (#1668) |
| SessionView DTermMetalView selection | DONE (#1669) |
| CAMetalLayer ↔ wgpu surface | Uses hybrid via Metal shaders |
| FPS counter and performance instrumentation | DONE (#1672) |
| FPS overlay for development | DONE (#1673) |
| Runtime validation | DONE (#1674) - GPU renderer initializes correctly |
| Sixel image rendering | DONE (#1684-1687) |
| Kitty image rendering | DONE (#1684-1687) |
| Image Metal shaders (DTermHybrid.metal) | DONE (#1685) |
| Image test script | DONE (#1686) |

**Hybrid Rendering Architecture (PAUSED - #1731):**

The GPU renderer uses a hybrid approach where:
- **Rust (dterm-core)**: Generates vertex data and manages glyph atlas via `DTermHybridRenderer`
- **Swift (DTermMetalView)**: Manages Metal pipeline, textures, and draw calls using `DTermHybrid.metal` shaders
- **ObjC (SessionView)**: Integrates DTermMetalView when `dtermCoreRendererEnabled` advanced setting is YES

This architecture bypasses the complex ObjC Metal stack while leveraging Rust's memory safety for the computationally intensive parts.

**ENABLED by default (#1418, #1422).** Box drawing characters (═ ║ ╔ ╗ ╚ ╝ etc.), Powerline glyphs, and block elements are now fully implemented in dterm-core. The hybrid renderer is production-ready.

To disable (fallback to legacy): `defaults write com.dashterm.dashterm2 dtermCoreRendererEnabled -bool NO`

**To enable FPS overlay (development):** Set `defaults write com.dashterm.dashterm2 DtermCoreFPSOverlayEnabled -bool YES`

**Files to delete after migration stable:**
- `iTermMetalView.m` (~6000 lines)
- `iTermMetalDriver.m`
- `iTermPromise.m`
- `iTermMetalFrameData.m`

**Acceptance criteria:**
- 120 FPS on ProMotion displays
- <1ms input latency
- Zero dispatch_group crashes

### 3.4 In-Process AI Interface - COMPLETE

`DTermAIInterface` class provides direct memory access to terminal state.

```swift
DTermAIInterface.getAllTerminals()      // Get all PTYSession instances
DTermAIInterface.readScreen(session:)   // <0.01ms latency
DTermAIInterface.readScrollback(session:, lines:)
DTermAIInterface.getCommandHistory(session:, count:)
DTermAIInterface.sendInput(session:, text:)
DTermAIInterface.isAILocked(session:)   // Window locking
```

**Window locking UI:**
- Menu item "Lock from AI" in Session and View menus
- Lock icon indicator in SessionTitleView

---

## Phase 4: AI-Native Features - READY

### MCP Server (DEPRIORITIZED)

Not needed until there's a concrete feature request for external tool access.
Design preserved in `docs/future/HIGH-PERF-MCP-DESIGN.md`.

---

## Phase 5: Platform Expansion - FUTURE

| Platform | Approach | Status |
|----------|----------|--------|
| macOS | Current app + Rust FFI | Active |
| iOS/iPadOS | SwiftUI + dterm-core | Future |
| visionOS | Spatial terminal | Future |
| Linux | GTK + dterm-core | Future |
| Windows | WinUI + dterm-core | Future |

---

## Immediate Worker Tasks (Jan 2, 2026)

### Current Status: VISUAL QA AUTOMATION

All major development work is complete. New priority: automated visual QA.

| Metric | Value |
|--------|-------|
| Build | PASSING |
| Tests | 4967+ passed, 61 skipped, 0 failures |
| GPU Renderer | ENABLED (#1418, #1422) - box drawing fully implemented |
| Legacy Renderer | Available as fallback |
| Upstream Sync | FULLY SYNCHRONIZED |
| P0 Bugs | 0 remaining (all triaged) |
| **Visual QA** | **FAILING** (17% similarity vs iTerm2) |

---

### Priority 0: Fix Visual Comparison Test - HIGH

**Problem**: LLM visual comparison test (`scripts/llm_visual_judge.py`) shows DashTerm2 renders very differently from iTerm2.

**Root Cause**: Permission banners block terminal content. DashTerm2 prompts for:
> "A control sequence attempted to clear scrollback history. Allow this in the future?"

iTerm2 allows these by default.

**Current Metrics** (2026-01-02):
| Metric | Current | Target |
|--------|---------|--------|
| Pixel similarity | 17% | >90% |
| GPT-5.2 score | 16/100 | >80/100 |
| Verdict | FAIL (10/10) | PASS |

**Worker Checklist**:

#### Phase 1: Investigate Permission System
- [ ] Find permission banner code:
  ```bash
  grep -r "Allow this in the future" sources/
  grep -r "control sequence" sources/
  grep -r "iTermWarning" sources/
  ```
- [ ] Identify all control sequences that trigger prompts
- [ ] Find iTerm2's default permission settings in `~/Library/Preferences/com.googlecode.iterm2.plist`
- [ ] Document in `docs/PERMISSION_SYSTEM_ANALYSIS.md`

#### Phase 2: Create Test Configuration (Quick Fix)
- [ ] Create `test-fixtures/dashterm2-test-preferences.plist` with all permissions pre-granted
- [ ] Update `scripts/llm_visual_judge.py` to backup/restore preferences
- [ ] Add `--permissive` flag to test script

#### Phase 3: Fix Default Permissions (Production Fix)
- [ ] Update DashTerm2 defaults to match iTerm2:
  - [ ] Clear scrollback history (`\e[3J`) - ALLOW
  - [ ] Clear screen (`\e[2J`) - ALLOW
  - [ ] Cursor save/restore - ALLOW
  - [ ] Window title changes - ALLOW
- [ ] Keep prompts for genuinely dangerous sequences only

#### Phase 4: Theme Matching
- [ ] Compare default color schemes
- [ ] Create matching theme OR normalize in test script

#### Phase 5: Verification
- [ ] Run `./scripts/run-visual-llm-test.sh --all`
- [ ] Achieve >90% pixel similarity
- [ ] Achieve PASS from GPT-5.2

**Likely Files**:
- `sources/PTYSession.m` - Session permission handling
- `sources/VT100Terminal.m` - Control sequence processing
- `sources/iTermWarning.m` - Warning dialog system
- `sources/iTermAdvancedSettingsModel.m` - Permission defaults

**Test Command**:
```bash
export OPENAI_API_KEY="..."
./scripts/run-visual-llm-test.sh --all --gpt-only
```

**Output**: `visual-test-output/llm-judge/<timestamp>/`

---

### Priority 1: Monitor Upstream

Check for new commits periodically:
```bash
cd ~/DashTerm2-reference && git fetch origin && git log origin/master --oneline -5
```

**Last checked:** Dec 31, 2025 14:15 PST - Synced to `d893436`

### Priority 2: Legacy Metal Code Deprecation (FUTURE)

The legacy ObjC Metal stack (~23,000 lines) is kept as fallback but has documented concurrency bugs:
- `os_unfair_lock_recursive_abort` crashes in `iTermMetalDriver`
- Complex dispatch_group patterns prone to race conditions

**Files to delete when ready:**
| Directory/Pattern | Lines | Notes |
|-------------------|-------|-------|
| `sources/Metal/` | ~15,000 | Renderers, shaders, infrastructure |
| `sources/iTermMetal*` | ~11,000 | Driver, frame data, buffer pools |
| `sources/iTermPromise.*` | ~680 | Legacy async pattern (replaced by Rust channels) |

**Deletion prerequisites:**
1. Run application in production for 1-2 weeks without issues
2. Verify no crash reports from new GPU renderer path
3. Manager approval for deletion

**To disable GPU renderer (fallback):** `defaults write com.dashterm.dashterm2 dtermCoreRendererEnabled -bool NO`

### Priority 3: dterm-core Feature Parity - COMPLETE

The dterm-core renderer now has full feature parity:

| Feature | Status | Notes |
|---------|--------|-------|
| Box drawing characters | COMPLETE (#1418, #1422) | 0x2500-0x257F range |
| Powerline glyphs | COMPLETE (#1418) | 0xE0B0-0xE0D4 range |
| Block elements | COMPLETE (#1418) | 0x2580-0x259F range |
| Sextant/legacy FB | COMPLETE (#1418) | 0x1FB00-0x1FB6F range |

**GPU renderer enabled by default since #1418.** Legacy ObjC Metal renderer available as fallback.

### Priority 4: Previous Work (COMPLETE)

| Area | Status | Notes |
|------|--------|-------|
| GPU Renderer Integration | COMPLETE (#1687) | All components integrated |
| Enable by Default | REVERTED (#1731) | Box drawing not implemented |
| Bug Fixes | COMPLETE | 4967+ tests pass |
| Crash Report UI | COMPLETE (#1679) | Tab bar icon + menu |
| Build Issues | RESOLVED | Main scheme builds |

### Pre-existing Non-blocking Issues

1. **WebExtensionsFramework** - Architecture mismatch (arm64 only, build wants x86_64 too)
2. **DashTerm2ImportStatus** - Info.plist processing error

These do not affect the main DashTerm2 scheme.

---

## Code Architecture Principles

### Replace, Don't Patch

ObjC code has systemic issues. Replace with Rust or Swift.

| Component | Current | Replace With | Priority | Lines |
|-----------|---------|--------------|----------|-------|
| VT100Parser | ObjC | dterm-core (Rust) | DONE | ~1,100 |
| VT100Terminal | ObjC | dterm-core (Rust) | DONE | ~2,000 |
| LineBuffer/Grid | ObjC | dterm-core (Rust) | DONE | ~3,000 |
| Metal Renderer | ObjC | wgpu (Rust) | DONE | ~6,000 |
| iTermPromise | ObjC | Rust channels | DONE | ~600 |
| Profile Model | ObjC | Swift | FUTURE | ~1,500 |
| Preferences UI | ObjC | Swift | FUTURE | ~2,000 |
| iTermController | ObjC | Swift | FUTURE | ~1,500 |

### Rules for New Code

1. **Terminal logic** -> Rust (in dterm-core)
2. **UI code** -> Swift (not ObjC)
3. **Platform APIs** -> Swift wrappers around FFI
4. **Never** -> New ObjC code

### Goal

**Zero ObjC in 12-18 months.** All code either Rust (core) or Swift (UI).

---

## Future Work Backlog

Items deferred from previous roadmaps. Not prioritized but tracked for completeness.

### Disk-Backed Scrollback (Deferred from Polishing Roadmap)

**Status:** NOT STARTED | **Effort:** HIGH | **Impact:** HIGH

Memory-map cold scrollback blocks to disk for reduced RAM usage.

```
Hot:   Last 100 blocks     - Unpacked, fully in RAM
Warm:  Next 1000 blocks    - Packed (8 bytes/char), mmap'd
Cold:  Oldest blocks       - Packed + zstd compressed, disk file
```

**Expected improvement:** 80%+ memory reduction for large scrollback

**Reference:** [archive/roadmaps/POLISHING-ROADMAP.md](./archive/roadmaps/POLISHING-ROADMAP.md) Section 1.2

### Legacy ObjC Code Deletion

Once GPU renderer is stable in production for 2+ weeks, delete:

| Directory/Pattern | Lines | Notes |
|-------------------|-------|-------|
| `sources/VT100Parser.m` | ~1,100 | Legacy parser |
| `sources/VT100Token.m` | ~500 | Legacy tokens |
| `sources/VT100StateMachine.m` | ~800 | Legacy state machine |
| `sources/VT100CSIParser.m` | ~600 | Legacy CSI parser |
| `sources/VT100OscParser.m` | ~400 | Legacy OSC parser |
| `sources/VT100DcsParser.m` | ~300 | Legacy DCS parser |
| `sources/Metal/` | ~15,000 | Legacy Metal renderers |
| `sources/iTermMetal*` | ~11,000 | Legacy Metal driver/pools |
| `sources/iTermPromise.*` | ~680 | Legacy async pattern |

**Prerequisites:**
1. GPU renderer stable 2+ weeks
2. Zero crash reports from new renderer
3. Manager approval

**Reference:** [archive/roadmaps/DTERM-CORE-PARITY-ROADMAP.md](./archive/roadmaps/DTERM-CORE-PARITY-ROADMAP.md) Phase 5

### Sound Design (From Vision)

**Status:** NOT STARTED | **Priority:** LOW

- Subtle, optional audio feedback
- Bell that doesn't flood/brick the app (BUG-12323 fixed)
- Custom sound themes

**Reference:** [VISION.md](./VISION.md)

### Settings Cleanup (After Legacy Deletion)

Remove dterm-core feature flags once legacy code is deleted:

```
DELETE: dtermCoreEnabled (always YES)
DELETE: dtermCoreParserOutputEnabled (always YES)
DELETE: dtermCoreGridEnabled (always YES)
DELETE: dtermCoreRendererEnabled (always YES)
DELETE: dtermCoreValidationEnabled (no longer needed)
DELETE: dtermCoreParserComparisonEnabled (no longer needed)
```

---

## Build Command

```bash
xcodebuild -project DashTerm2.xcodeproj -scheme DashTerm2 -configuration Development \
  ENABLE_ADDRESS_SANITIZER=NO CODE_SIGNING_ALLOWED=NO CODE_SIGN_IDENTITY="-" build
```

---

## Commit Format

```
# N: Brief description

**Current Plan**: docs/ROADMAP.md
**Checklist**: [status from this document]

## Changes
[What and why]

## New Lessons
[If any]

## Information Expiration
[Obsolete info]

## Next AI: [Directive]
```

---

## Contact

- **Repo:** https://github.com/dropbox/dTOOL/dashterm2
- **Upstream:** https://github.com/gnachman/iTerm2
- **Reference:** `~/DashTerm2-reference` (iTerm2 3.6)
