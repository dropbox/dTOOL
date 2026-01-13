# Pending Work - dterm-core

**Last Updated:** 2026-01-01 (Iteration 499)
**Status:** Phase 9 external integration in progress; GPU Renderer FFI COMPLETE; rendering gaps COMPLETE; native speech bindings COMPLETE; **WASM Plugin Phase 5 COMPLETE**; **GitHub Actions CI BLOCKED**

**See also:**
- `TO_WORKER_RENDERING_GAPS_2025-12-31.md` - Rendering gaps complete status
- `TO_DASHTERM2_GPU_FFI_READY_2025-12-31.md` - DashTerm2 integration guide (GPU Renderer FFI)
- `docs/GAP_ANALYSIS.md` - Comprehensive gap analysis vs all competitors
- `docs/BUILDKITE_PROVISIONING.md` - Buildkite agent setup steps

---

## Rendering Gaps COMPLETE (Iteration 403)

**All DashTerm2 rendering blockers are resolved and ready for integration.**

| Gap | Impact | Effort | Status |
|-----|--------|--------|--------|
| Box drawing characters (U+2500-U+257F) | Invisible glyphs | 3-5 days | **COMPLETE** |
| Block elements (U+2580-U+259F) | Missing shades/quadrants | 1-2 days | **COMPLETE** |
| Powerline glyphs (U+E0A0-U+E0D7) | Prompts broken | 2-3 days | **COMPLETE** |
| Dotted underline (SGR 4:4) | Missing style | 0.5 days | **COMPLETE** |
| Dashed underline (SGR 4:5) | Missing style | 0.5 days | **COMPLETE** |
| iTerm2 inline images (OSC 1337) | Images don't work | 3-5 days | **COMPLETE** |

**Action Required:** Integrate latest dterm-core and follow the READY-FOR-INTEGRATION files:
- `READY-FOR-INTEGRATION-BOX-DRAWING.md`
- `READY-FOR-INTEGRATION-POWERLINE.md`
- `READY-FOR-INTEGRATION-UNDERLINES.md`
- `READY-FOR-INTEGRATION-ITERM-IMAGES.md`
- `READY-FOR-INTEGRATION-METAL-SHADER.md`

### DashTerm2 Renderer Integration Notes (Iteration 450)

DashTerm2-side adjustments for the rendering gap fixes:
- `sources/DTermMetalView.swift`: set sampler filter to `.nearest` to avoid blurry text.
- `sources/DTermMetalView.swift`: box drawing line thickness tuned to 0.14/0.22 to avoid invisible strokes.

These are DashTerm2 changes, not dterm-core code.

---

## DashTerm2 GPU Renderer FFI - ALL COMPLETE (Iteration 381)

**All P0, P1, P2 requirements from DashTerm2 are IMPLEMENTED:**

| Priority | Requirement | Status |
|----------|-------------|--------|
| P0 | Renderer lifecycle FFI | **COMPLETE** |
| P0 | Frame sync with timeout | **COMPLETE** |
| P0 | Metal drawable handling | **COMPLETE** |
| P0 | Font management FFI | **COMPLETE** |
| P0 | Configuration FFI | **COMPLETE** |
| P1 | Cursor rendering (Block/Underline/Bar + blink) | **COMPLETE** |
| P1 | Selection highlighting | **COMPLETE** |
| P2 | Underline/Strikethrough rendering | **COMPLETE** |
| P2 | Background image support | **COMPLETE** |

**Swift Bindings:** `packages/dterm-swift/Sources/DTermCore/DTermGPURenderer.swift`

**DashTerm2 Next Steps:**
1. Test `DTermFrameSync` as dispatch_group replacement
2. Test `DTermHybridRenderer` vertex output
3. Migrate rendering to dterm-core vertex data
4. Delete ObjC Metal code (~6000 lines)

See `TO_DASHTERM2_GPU_FFI_READY_2025-12-31.md` for complete integration guide.

---

## Current Priorities (Iteration 487)

1. **DashTerm2 Metal shader update (external)**
   - Update DashTerm2 shader flag values to the new 7-bit layout
   - See `docs/METAL_SHADER_MIGRATION.md`
   - Directive: `TO_DASHTERM2_METAL_SHADER_MIGRATION_2025-12-31.md`
   - Ready file: `READY-FOR-INTEGRATION-METAL-SHADER.md`

2. **🔴 BLOCKER: GitHub Actions hosted runners disabled**
   - Error: "GitHub Actions hosted runners are disabled for this repository"
   - Decision (Iteration 389): Use Buildkite as primary CI until hosted runners are enabled
   - Buildkite pipeline ready: `.buildkite/pipeline.yml`
   - Options: See `docs/CI_ALTERNATIVES.md`

   **Buildkite Agent Provisioning Steps:**
   See `docs/BUILDKITE_PROVISIONING.md` for the full workflow.
   Summary:
   1. Create Buildkite pipeline pointing to this repo and `.buildkite/pipeline.yml`
   2. Provision agents with queues: `macos`, `windows`, `linux` (verify uses `linux` by default)
   3. Install Rust toolchains (stable everywhere, nightly + MIRI on verify)
   4. Trigger pipeline and record results below

   **Agent Status (record when provisioned):**
   - [ ] macOS agent online (queue: `macos`)
   - [ ] Windows agent online (queue: `windows`)
   - [ ] Linux agent online (queue: `linux`)
   - [ ] Verify agent online (queue: `linux` with nightly+miri)

3. **Alacritty integration (Windows/Linux)** - BLOCKED on CI
   - GitHub Actions workflow: `.github/workflows/ci.yml`
   - Build instructions: `docs/BUILDING.md`
   - Cannot proceed until CI runners are available

4. **Microphone E2E testing (Windows/Linux)** - BLOCKED on CI
   - Requires Windows/Linux runners to execute `mic_test`
   - macOS E2E testing complete (iteration 366)

## Recent Completions (Iteration 486)

### Clippy pedantic lint cleanup - COMPLETE

**Changes:**
- `crates/dterm-core/src/plugins/storage.rs`: Replace `vec![Xu8; N]` with `[Xu8; N]` slices in tests
- `crates/dterm-core/src/plugins/bridge.rs`: Change `NativePluginProcessor::name()` to return `&'static str`
- `crates/dterm-core/src/plugins/storage.rs`: Use `sort_unstable()` for primitive key ordering
- `crates/dterm-core/src/tests/proptest.rs`: Remove unnecessary `.into()` conversions for `u16`

## Recent Completions (Iteration 469)

### Fuzz Target Hardening - COMPLETE

**Docs:** `docs/FUZZ_RESULTS.md`

**Changes:**
- Extended fuzzing across all five targets (1+ hour runs)
- Fixed plugins fuzz target infinite loop in storage bridge operations
- Added `PluginBridgeConfig` fields for failure recovery controls
- Relaxed strict assertions in checkpoint + kitty graphics fuzz targets
- Tightened FFI fuzz memory budgets (100x200 max, 10MB cap)

---

## Recent Completions (Iteration 465)

### WASM Plugin System Phase 5 - COMPLETE

**Docs:** `docs/WASM_PLUGIN_SYSTEM.md`

**Changes:**
- Failure recovery and auto-disable controls for unstable plugins
- Per-plugin health tracking and recovery APIs
- Fuzz target coverage for plugin bridge and storage behavior

---

## Recent Completions (Iteration 443)

### WASM Plugin System Phase 4 - COMPLETE

**Docs:** `docs/WASM_PLUGIN_SYSTEM.md`

**Changes:**
- `PluginBridge`: Central integration layer connecting terminal events to plugins
- Output integration: `process_output()` hooks for terminal data flow
- Input integration: `process_key()` hooks for keyboard events
- Command lifecycle: `on_command_start()`/`on_command_complete()` for OSC 133
- WASM memory access: `read_wasm_memory_mut()`, `write_wasm_memory()`, `read_wasm_string()`
- Native plugin support via `NativePluginProcessor` trait
- 11 new tests for bridge functionality

---

## Recent Completions (Iteration 438)

### WASM Plugin System Phase 3 - COMPLETE

**Docs:** `docs/WASM_PLUGIN_SYSTEM.md`

**Changes:**
- Permission gating for plugin actions and event reception
- Storage API with quotas and error codes
- Host function permission checks enforced in runtime

---

## Recent Completions (Iteration 394)

### Fuzz Documentation Refresh - COMPLETE

**Docs:** `docs/FUZZ_RESULTS.md`

**Changes:**
- Updated fuzz target documentation and results summary
- Synced with latest fuzz target coverage and requirements

---

## Recent Completions (Iteration 393)

### Fuzz Target Expansion - COMPLETE

**Changes:**
- Added 4 new fuzz targets: checkpoint, ffi, kitty_graphics, selection
- Enabled `ffi` feature in fuzz crate
- Updated `docs/FUZZ_RESULTS.md` with target list and commands

---

## Recent Completions (Iteration 370)

### GPU Renderer FFI Analysis - COMPLETE

**Analysis of DashTerm2 integration requirements for GPU rendering:**

Three rendering approaches are available:

| Approach | What dterm-core does | What platform does |
|----------|---------------------|-------------------|
| **Frame Sync Only** (`DTermFrameSync`) | Safe frame request/timeout handling | All rendering |
| **Hybrid Renderer** (`DTermHybridRenderer`) | Vertex + uniform generation, glyph atlas | GPU buffer management, rendering |
| **Full GPU Renderer** (`DTermGPURenderer`) | Complete wgpu rendering | Provide device/queue handles |

**Recommendation for DashTerm2: Use Hybrid Renderer**

Rationale:
1. **Incremental migration** - DashTerm2 can keep existing Metal shaders initially
2. **Platform fonts** - Use `dterm_hybrid_renderer_enable_platform_glyphs()` for CoreText
3. **Memory control** - DashTerm2 controls Metal buffer allocation
4. **Debugging** - Can validate vertex correctness before deleting ObjC code

**Available FFI surface for Hybrid Renderer:**

| API | Purpose |
|-----|---------|
| `dterm_hybrid_renderer_create/free` | Lifecycle |
| `dterm_hybrid_renderer_set_font` | Font configuration |
| `dterm_hybrid_renderer_get_cell_size` | Cell metrics |
| `dterm_hybrid_renderer_build` | Generate vertices for terminal |
| `dterm_hybrid_renderer_get_vertices/uniforms` | Access vertex data |
| `dterm_hybrid_renderer_get_atlas_data` | Access glyph atlas texture |
| `dterm_hybrid_renderer_pending_glyph_count/get/clear` | Incremental atlas updates |
| `dterm_hybrid_renderer_enable_platform_glyphs` | Use CoreText glyphs |
| `dterm_hybrid_renderer_add_platform_glyph` | Register platform-rendered glyphs |

**Swift bindings:** Complete in `packages/dterm-swift/Sources/DTermCore/DTermGPURenderer.swift`

**Next steps for DashTerm2:**
1. Create `DTermMetalView.swift` wrapping `DTermHybridRenderer`
2. Use platform glyphs mode for CoreText font rendering
3. Upload vertex buffer from `dterm_hybrid_renderer_get_vertices()`
4. Render using existing iTerm2 Metal shaders (or minimal new shader)
5. Validate output matches current rendering
6. Delete `iTermMetalView.m`, `iTermMetalDriver.m`, `iTermPromise.m`

---

## Recent Completions (Iteration 369)

### GPU Renderer FFI Surface Expansion

**Commit:** 0905bce

**Changes:**
- Refactored `DtermRenderer` to own frame sync plus hybrid state
- Added drawable handling and handle-based waits
- Added renderer configuration APIs for fonts, cursor/selection overrides, cell metrics
- Added hybrid render stats support
- Updated Swift `DTermFrameSync` to pass config and wait on last requested frame
- Regenerated cbindgen headers and synced Swift include

---

## Recent Completions (Iteration 366)

### Microphone E2E Testing - macOS VALIDATED

**Documentation:** `docs/VOICE_E2E_TESTING.md`
**Result:** Authorization and capture pipeline validated on macOS; no issues observed.

---

## Recent Completions (Iteration 351)

### Windows Audio Input - FULL IMPLEMENTATION

**File:** `media/platform/windows_audio_input.rs`

**Changes:**
- Full AudioGraph-based implementation using `Windows.Media.Audio` API
- `WindowsAudioInputProvider` with complete `AudioInputProvider` trait implementation
- Components:
  - `AudioGraph` with `AudioRenderCategory::Speech` for speech capture optimization
  - `AudioDeviceInputNode` for microphone input
  - `AudioFrameOutputNode` for frame-by-frame audio retrieval
  - `QuantumStarted` event handler for synchronized audio processing
- Device enumeration via `DeviceInformation::FindAllAsyncDeviceClass`
- Float32 PCM output at 16kHz or 44.1kHz sample rates
- Low-latency quantum size selection (`QuantumSizeSelectionMode::LowestLatency`)
- Proper COM interface usage for buffer access (`IMemoryBufferByteAccess`)
- Added `Media_Audio` and `Media_Render` features to windows crate dependencies

**Platform Status:**

| Platform | TTS | STT | Audio Input |
|----------|-----|-----|-------------|
| macOS | Native (AVSpeechSynthesizer) | Native (SFSpeechRecognizer) | Native (AVAudioEngine) |
| iOS | Native (AVSpeechSynthesizer) | Native (SFSpeechRecognizer) | Native (AVAudioEngine) |
| Windows | Native (WinRT SpeechSynthesizer) | Native (WinRT SpeechRecognizer) | **Native (AudioGraph)** |
| Linux | Native (espeak-ng) | Native (Vosk) | Native (ALSA) |

**All platforms now have full native audio input implementations!**

---

## Recent Completions (Iteration 350)

### Linux Audio Input - FULL IMPLEMENTATION

**File:** `media/platform/linux_audio_input.rs`

**Changes:**
- Full ALSA PCM capture implementation using `alsa` crate
- Configuration: S16_LE format, mono, configurable sample rate (16kHz/44.1kHz)
- Dedicated capture thread with proper error recovery
- Buffer overrun (EPIPE) recovery via `pcm.prepare()`
- Non-blocking read handling (EAGAIN)
- PCM state monitoring (XRun, Suspended, Disconnected)
- Added `alsa` crate to `linux-speech` feature dependencies

**Platform Status:**

| Platform | TTS | STT | Audio Input |
|----------|-----|-----|-------------|
| macOS | Native (AVSpeechSynthesizer) | Native (SFSpeechRecognizer) | Native (AVAudioEngine) |
| iOS | Native (AVSpeechSynthesizer) | Native (SFSpeechRecognizer) | Native (AVAudioEngine) |
| Windows | Native (WinRT SpeechSynthesizer) | Native (WinRT SpeechRecognizer) | Native (AudioGraph) |
| Linux | Native (espeak-ng) | Native (Vosk) | Native (ALSA) |

---

## Recent Completions (Iteration 349)

### Windows/Linux Audio Input Providers - STRUCTURAL

**New Files:**
- `media/platform/windows_audio_input.rs` - Windows audio input via WASAPI/MediaCapture
- `media/platform/linux_audio_input.rs` - Linux audio input via ALSA

**Changes:**
- Windows `WindowsAudioInputProvider` with `AudioInputProvider` trait implementation
- Device enumeration via `DeviceInformation::FindAllAsyncDeviceClass`
- MediaCapture initialization for audio-only capture
- Updated `create_audio_input_provider()` to return platform providers
- Added Windows features: `Media_Capture`, `Media_MediaProperties`, `Media`, `Devices_Enumeration`

---

## Recent Completions (Iteration 348)

### Audio Input Integration with MediaServer - COMPLETE

**Files modified:**
- `media/server.rs` - Added audio input provider integration
- `media/tests.rs` - Added end-to-end voice flow tests with mocks

**New MediaServer APIs:**
- `with_all_providers(config, stt, tts, audio_input)` - Create server with all providers
- `set_audio_input(provider)` - Set audio input provider
- `has_audio_input()` - Check if audio input available
- `start_stt_with_microphone(client, format, language)` - Start STT with microphone capture
- `process_audio()` - Process pending audio from microphone
- `stop_stt_with_microphone()` - Stop capture and get final result
- `cancel_stt_with_microphone()` - Cancel capture and STT session
- `is_capturing_audio()` - Check if capturing
- `is_voice_active()` - Check VAD status

**New Error Type:**
- `MediaServerError::AudioInput(AudioInputError)` - Audio input errors

**Tests Added:**
- Mock STT provider for testing
- Mock audio input provider for testing
- End-to-end voice flow tests with mock providers
- Audio input integration tests

---

## Recent Completions (Iteration 347)

### macOS/iOS Audio Input - COMPLETE

**File:** `media/platform/macos_audio_input.rs`

**Changes:**
- Native audio capture via `AVAudioEngine`
- `MacOsAudioInputProvider` with full `AudioInputProvider` trait implementation
- Device enumeration via `AVCaptureDevice`
- Authorization checking for microphone access
- Shared between macOS and iOS (same Speech/AVFoundation frameworks)
- Factory function `create_audio_input_provider()` for platform detection

---

## Recent Completions (Iteration 346)

### iOS Speech Bindings - COMPLETE

**Changes:**
- iOS speech bindings share implementation with macOS
- Added `ios-speech` feature flag
- Both TTS and STT work on iOS via shared native bindings
- Updated platform status table

---

## Recent Completions (Iteration 345)

### macOS STT Full Recognition - COMPLETE

**File:** `media/platform/macos_stt_native.rs`

**Changes:**
- Full native bindings to `SFSpeechRecognizer` with block2 callback support
- `SFSpeechAudioBufferRecognitionRequest` for streaming audio input
- `SFSpeechRecognitionTask` for managing recognition sessions
- `SFSpeechRecognitionResult`, `SFTranscription`, `SFTranscriptionSegment` bindings
- `AVAudioFormat` and `AVAudioPCMBuffer` for audio data handling
- `ThreadSafeRecognizer` with full recognition flow:
  - `start(on_device, partial_results)` - Start recognition session
  - `feed_audio(&[u8])` / `feed_audio_i16(&[i16])` - Feed audio data
  - `get_partial()` - Get latest partial result
  - `get_final()` - Get final result
  - `stop()` - End audio input and wait for final result
  - `cancel()` - Cancel recognition without results

**Platform Status:**

| Platform | TTS | STT | Audio Input |
|----------|-----|-----|-------------|
| macOS | Native (AVSpeechSynthesizer) | Native (SFSpeechRecognizer) | Native (AVAudioEngine) |
| iOS | Native (AVSpeechSynthesizer) | Native (SFSpeechRecognizer) | Native (AVAudioEngine) |
| Windows | Native (WinRT SpeechSynthesizer) | Native (WinRT SpeechRecognizer) | Stub |
| Linux | Native (espeak-ng) | Native (Vosk) | Native (ALSA) |

---

## Recent Completions (Iteration 344)

### Native Speech Bindings - Windows/Linux STT

**New Files:**
- `media/platform/windows_stt_native.rs` - Windows STT bindings via WinRT `SpeechRecognizer`
- `media/platform/linux_stt_native.rs` - Linux STT bindings via Vosk

**Changes:**
- Windows STT via WinRT `SpeechRecognizer` with `ThreadSafeRecognizer` wrapper
- Linux STT via Vosk with model path discovery and `ThreadSafeRecognizer` wrapper
- Updated `WindowsSttProvider` to use native bindings when `windows-speech` enabled
- Updated `LinuxSttProvider` to use native Vosk bindings when `linux-speech` enabled
- Added `Media_SpeechRecognition` and `Globalization` features to windows crate
- Added `vosk` crate dependency for Linux STT

---

## Recent Completions (Iteration 343)

### macOS STT Partial Implementation

**Changes:**
- macOS STT partial bindings via `SFSpeechRecognizer`
- Windows TTS via WinRT `SpeechSynthesizer`
- Linux TTS via `espeak-ng`
- Added VAD based on RMS audio energy detection

---

## Recent Completions (Iteration 206)

### RLE Scrollback Attribute Compression - COMPLETED

**Files modified:**
- `scrollback/line.rs` - Added `CellAttrs` struct, RLE integration
- `grid/mod.rs` - Updated `row_to_line_static` to extract attributes

**Changes:**
- Lines now store RLE-compressed cell attributes (colors, flags)
- Typical 80-column line: 3-5 attribute runs vs 80 cells
- Compression ratio: 10-30x for styled lines
- Default-only lines optimized to store no attributes
- Backward-compatible serialization (v0 legacy + v1 with attrs)

**Benefits:**
- Preserves styling when lines scroll into scrollback
- Memory efficient: 14 bytes per run vs 8 bytes per cell
- Enables future features: styled search results, style-aware copy

---

## Performance Optimization Round 2

**Goal:** 25-40% additional throughput improvement

### Optimization 1: OSC Allocation in `dispatch_osc` - COMPLETED

**File:** `parser/mod.rs`

**Problem:** Allocates `Vec<&[u8]>` for every OSC sequence (hyperlinks, titles, shell integration).

**Fix:** Used pre-allocated `ArrayVec<(usize, usize), 8>` stored in Parser struct.

**Status:** ✅ Implemented in iteration 199

---

### Optimization 2: `#[inline(always)]` on `row_index` - COMPLETED

**File:** `grid/mod.rs`

**Problem:** Called on every cell access, not inlined across modules.

**Fix:** Added `#[inline(always)]` to force inlining.

**Status:** ✅ Implemented in iteration 199

---

### Optimization 3: Redundant Wide Character Checks - COMPLETED

**File:** `grid/row.rs:329-366`

**Problem:** Multiple bounds checks on consecutive cells for every character write.

**Fix:** Single bounds check, then use `get_unchecked`. Mark wide char fixup as `#[cold]`.

**Expected:** 10-15% faster character writes

**Status:** ✅ Implemented in iteration 207

---

### Optimization 4: FxHashMap for CellExtras - COMPLETED

**File:** `grid/extra.rs`, `grid/style.rs`

**Problem:** Uses cryptographic SipHash for simple `(u16, u16)` keys.

**Fix:** Replaced `std::HashMap` with `rustc_hash::FxHashMap`.

**Status:** ✅ Implemented in iteration 199

---

### Optimization 5: Repeated `rgb_components()` Calls

**File:** `terminal/mod.rs:4637-4665`

**Problem:** Extracts RGB components 4 times for wide characters.

**Fix:** Extract once at top of block and reuse for continuation cell.

**Expected:** 5-10% faster RGB color handling

**Status:** ✅ Implemented in iteration 208

---

### Optimization 7: ASCII Fast-path for char_width - COMPLETED

**File:** `terminal/mod.rs`

**Problem:** Calls `UnicodeWidthChar::width()` for every character including ASCII.

**Fix:** Added `char_width()` helper that returns 1 immediately for printable ASCII (0x20-0x7E).

**Status:** ✅ Implemented in iteration 199

---

### Optimization 6: `scroll_up` Batch Allocation - COMPLETED

**File:** `grid/mod.rs:scroll_up()`

**Problem:** `Row::new()` allocates for every new row during buffer growth, and counter updates happen per-iteration.

**Fix:** Pre-calculate rows to add vs reuse, batch Vec::reserve(), process in phases.

**Result:**
- 1KB ASCII: 93% improvement
- 64KB ASCII: 6% improvement
- Performance gate: 603 MB/s (vs 550 MB/s threshold)

**Status:** ✅ Implemented in iteration 202

---

### Optimization 7: Per-Character Unicode Width Lookup

**File:** `terminal/mod.rs:4567-4571`

**Problem:** Calls `UnicodeWidthChar::width()` for every character including ASCII.

**Fix:**
```rust
#[inline(always)]
fn char_width(c: char) -> usize {
    if (c as u32) < 0x80 { 1 } else { c.width().unwrap_or(1) }
}
```

**Expected:** 10-15% faster for ASCII-heavy workloads

---

### Optimization 8: CSI Dispatch Jump Table - NOT BENEFICIAL (Iteration 220)

**Analysis:** Investigated in iteration 220.
- Rust's `match` on `u8` already compiles to efficient jump tables via LLVM
- Function pointer tables would prevent inlining, hurting hot paths like SGR
- Added `#[inline]` hints to hot handlers instead (handle_sgr, handle_cursor_movement, etc.)

**Status:** ❌ NOT BENEFICIAL - LLVM already optimizes match statements

---

### Optimization 9: SIMD `find_special_byte` - NOT APPLICABLE (Iteration 221)

**Analysis:** The described problem does not exist in production code.
- Production uses `iter().position()` with simple predicate, LLVM auto-vectorizes
- The `memchr3 + linear scan` pattern only exists in benchmarks for comparison
- Current throughput: 3.48 GiB/s - no improvement needed

**Status:** ❌ NOT APPLICABLE - no code change needed

---

### Optimization 10: Damage Batching - LOW BENEFIT (Iteration 221)

**Analysis:** Hot paths already use `mark_row()`:
- `write_ascii_blast` and `write_ascii_run_styled` both use `mark_row`
- Only fallback paths (RGB, hyperlinks, insert mode) use `mark_cell`
- Fallback paths are slow for other reasons; batching would not help

**Status:** ❌ LOW BENEFIT - not worth complexity

---

### Optimization 11: Hyperlink Arc Clone - LIMITED BENEFIT (Iteration 221)

**Analysis:** We need separate Arc instances for each cell.
- Each cell stores its own `Option<Arc<Hyperlink>>`
- Two clones required for wide characters (one per cell)
- True optimization requires changing to index-based hyperlink storage (architectural change)

**Status:** ❌ LIMITED BENEFIT - would require architectural change

---

## Phase 9 Integration Status

### Pending

#### 1. Alacritty Integration (Windows/Linux) - 🔴 BLOCKED

**Goal:** Build Alacritty with `dterm-core` backend and pass tests/benchmarks.

**Verification Status (Iteration 368):**
- ✅ macOS: 309 unit tests + 5 integration tests + 3 doc tests pass
- 🔴 Windows: **BLOCKED** - GitHub Actions hosted runners disabled
- 🔴 Linux: **BLOCKED** - GitHub Actions hosted runners disabled
- ❌ Cross-compilation blocker: GitHub Enterprise settings prevent CI

**CI Configuration:**
- `.github/workflows/ci.yml` - Runs on `macos-latest`, `windows-latest`, `ubuntu-latest`
- Windows CI runs ConPTY integration tests (`-- --ignored`)
- Linux CI runs PTY integration tests and MIRI verification
- Build instructions documented in `docs/BUILDING.md`

**Blocker (as of 2025-12-31):**
> "GitHub Actions hosted runners are disabled for this repository. For more information please contact your GitHub Enterprise Administrator."

**Resolution options:**
1. Contact GitHub Enterprise Administrator to enable hosted runners for `dropbox/dTOOL/dterm`
2. Set up self-hosted runners for Windows and Linux
3. Use alternative CI service (CircleCI, Buildkite, etc.)
4. See `docs/CI_ALTERNATIVES.md` for setup notes and provider tradeoffs

**Execution notes (once unblocked):**
- Push to `main` or open PR to trigger CI
- Run Alacritty unit tests and report any API gaps in `dterm-alacritty-bridge`
- Capture performance deltas vs `alacritty_terminal` for parity tracking

**Cross-compilation limitations:**
- `zstd-sys` (used by dterm-core for scrollback compression) requires native C toolchain
- Windows target (`x86_64-pc-windows-msvc`) needs Windows SDK headers
- Linux target (`x86_64-unknown-linux-gnu`) needs `x86_64-linux-gnu-gcc`
- **Requires:** GitHub Actions hosted runners OR self-hosted runners

#### 2. SwiftTerm Integration (iOS/iPadOS) - ✅ COMPLETE

**Goal:** Wire SwiftTerm delegate callbacks and pass SwiftTerm tests/sample app.

**Status (Iteration 336):**
- ✅ Expanded `DTermTerminalDelegate` protocol to match SwiftTerm's `TerminalDelegate`
- ✅ Added 27 delegate callbacks covering all SwiftTerm events
- ✅ Added `WindowCommand` enum for XTWINOPS support
- ✅ Swift package compiles successfully
- ✅ FFI callbacks wired to delegate methods
- ✅ **DTermDemo sample app built and running successfully**

**Build requirements:**
```bash
# Build Rust library with FFI and GPU support
cargo build --release -p dterm-core --features ffi,gpu

# Build and run Swift demo
cd samples/ios-demo
swift build
./.build/debug/DTermDemo
```

**FFI callbacks implemented:**
- `dterm_terminal_set_bell_callback` → `terminalBell`
- `dterm_terminal_set_buffer_activation_callback` → `terminalBufferActivated`
- `dterm_terminal_set_title_callback` → `terminalTitleDidChange`
- `dterm_terminal_set_window_callback` → `terminalWindowCommand`
- `dterm_terminal_set_shell_callback` → `terminalCurrentDirectoryDidChange`

**Remaining work:**
- Implement macOS TTS via objc2-av-foundation

### Complete

#### 3. Windows ConPTY Support - ✅ COMPLETE

**Status:** Implemented in iteration 218.

**Files:**
- `crates/dterm-alacritty-bridge/src/tty/windows/mod.rs` (388 lines)
- `crates/dterm-alacritty-bridge/src/tty/windows/conpty.rs` (404 lines)

**Implementation includes:**
- `CreatePseudoConsole`, `ResizePseudoConsole`, `ClosePseudoConsole`
- `Pty` struct with `EventedReadWrite` implementation
- `ChildExitWatcher` for process monitoring
- Command-line argument escaping for Windows
- Default shell detection (PowerShell/cmd.exe)

---

#### 4. Additional APIs

| API | Priority | Complexity | Status |
|-----|----------|------------|--------|
| URL detection/hints | Medium | Medium | ✅ DONE (iteration 217-218) |
| Charset public API | Low | Low | ✅ DONE - `charset()`, `charset_mut()`, `CharacterSet` |
| Tab stops API | Low | Low | ✅ DONE (iteration 235) |
| Window title stack | Low | Low | ✅ DONE - `title_stack()`, XTWINOPS push/pop |
| Color query/report | Low | Medium | ✅ DONE - OSC 4/10/11/12 query support |
| Scroll region API | Low | Low | ✅ DONE - `scroll_region()`, `set_scroll_region()` |

**All additional APIs complete as of iteration 237.**

---

## Unicode/International Support Status

**Already Implemented:**

| Feature | Module | Status |
|---------|--------|--------|
| Grapheme clusters | `grapheme/mod.rs` | ✅ Full |
| Emoji sequences (ZWJ, skin tones) | `grapheme/mod.rs` | ✅ Full |
| Combining marks | `grapheme/mod.rs` | ✅ Full |
| CJK double-width | `grapheme/mod.rs` | ✅ Full |
| BiDi (Arabic/Hebrew) | `bidi/mod.rs` | ✅ Full |
| Non-BMP in 8-byte cell | `grid/cell.rs` | ✅ Overflow table |

**Optimization needed:** Per-character width lookup (Optimization 7)

---

## Verification Targets

| Gate | Target | Current | Status |
|------|--------|---------|--------|
| ASCII throughput | >= 400 MB/s | 945 MB/s | ✅ EXCEEDS |
| After Round 2 | >= 800 MB/s | 945 MB/s | ✅ EXCEEDS |
| All dterm-core tests | Pass | 1360 pass | ✅ |
| Bridge tests | Pass | 36 pass | ✅ |
| Integration tests | Pass | 5 pass | ✅ |

---

## Features from Other Terminals to Add

### High Priority (Differentiating)

| Feature | Source | Description | Complexity |
|---------|--------|-------------|------------|
| Block-based output | Warp | Command+output as atomic unit | High |
| WASM plugin system | Zellij | User-scriptable plugins with sandbox | High |
| Style deduplication | Ghostty | 12x memory savings via interning | Medium |

### Medium Priority (Parity)

| Feature | Source | Description | Complexity | Status |
|---------|--------|-------------|------------|--------|
| Smart selection rules | iTerm2 | Context-aware text selection (URLs, paths, etc.) | Medium | ✅ DONE |
| Secure keyboard entry | Terminal.app | Prevent key logging for password input | Low | ✅ DONE (Iteration 391) |
| Daemon mode | foot | Pre-fork for instant window spawn | Medium | |

### Low Priority (Nice to Have)

| Feature | Source | Description | Complexity |
|---------|--------|-------------|------------|
| Bookmarks system | Terminal.app | Save/restore scroll positions | Low |
| Workflows | Warp | Parameterized command templates | Medium |

---

## How to Prove dterm is Better Than Every Other Terminal

### 1. Throughput Benchmarks

**Current:** `cargo bench --package dterm-core --bench comparative`

Compares dterm vs vte (Alacritty parser) on:
- Pure ASCII (target: fastest)
- Mixed terminal output (target: ≥2x vte)
- Heavy escapes (target: ≥vte)
- UTF-8/CJK content
- vttest-style sequences

**Current results:** 584 MB/s ASCII (vs 400 MB/s vte = 1.46x faster)

**Target after Round 2:** 800+ MB/s ASCII, 2x vte across all workloads

### 2. Memory Benchmarks - ✅ EXISTS

**File:** `benches/memory.rs` (466 lines)

Comprehensive benchmarks for:
- Empty terminal creation
- Grid memory scaling
- Scrollback fill (100 - 100K lines)
- Tiered scrollback compression
- Line content patterns (ASCII, styled, CJK, hyperlinks)
- Resize operations
- Alternate screen switching
- Memory efficiency summary (10K/100K lines)

### 3. Latency Benchmarks - ✅ EXISTS

**File:** `benches/latency.rs` (533 lines)

Comprehensive benchmarks for:
- Keystroke latency (ASCII, UTF-8, newline, CRLF, emoji)
- Escape sequence latency (SGR, cursor, erase, scroll)
- Command output latency (pwd, echo, ls, git status)
- Line processing at various widths
- Frame budget utilization
- Typing simulation
- State query latency
- Terminal creation latency
- Interactive session simulation

**Results (from CONFORMANCE.md):**
- Single ASCII keystroke: ~11 ns (175,000x better than 2ms target)
- SGR reset: ~10 ns
- Frame budget: <3 µs (0.02% of 16.6ms budget)

### 4. Conformance Testing - ✅ TOOLING EXISTS

**File:** `scripts/vttest.sh` (288 lines)
**Documentation:** `docs/CONFORMANCE.md` (393 lines)

| Test Suite | Target | Status |
|------------|--------|--------|
| vttest script | Ready | ✅ `scripts/vttest.sh` |
| vttest unit tests | PASS | ✅ `crates/dterm-core/src/tests/vttest_conformance.rs` |
| vttest results | 100% pass | 🔶 **BLOCKED** - needs terminal GUI |
| esctest | 95%+ pass | 🔶 Optional |
| ECMA-48 | Full | ✅ Documented in CONFORMANCE.md |
| xterm extensions | 90%+ | ✅ Documented in CONFORMANCE.md |
| Kitty protocol | Full | ✅ Implemented |

**Note:** Interactive vttest requires a terminal emulator GUI. dterm-core is a library. Interactive testing will occur when integration targets (Alacritty, SwiftTerm) are complete. Unit test coverage via `vttest_conformance.rs` validates the same escape sequences.

### 5. Feature Matrix

| Feature | dterm | Alacritty | Kitty | WezTerm | iTerm2 |
|---------|-------|-----------|-------|---------|--------|
| Sixel graphics | ✅ | ❌ | ✅ | ✅ | ✅ |
| Kitty graphics | ✅ | ❌ | ✅ | ✅ | ❌ |
| iTerm2 images | ✅ | ❌ | ❌ | ✅ | ✅ |
| BiDi text | ✅ | ❌ | ✅ | ✅ | ❌ |
| Grapheme clusters | ✅ | ✅ | ✅ | ✅ | ✅ |
| Hyperlinks (OSC 8) | ✅ | ✅ | ✅ | ✅ | ✅ |
| Shell integration | ✅ | ❌ | ✅ | ✅ | ✅ |
| Tmux integration | ✅ | ❌ | ❌ | ❌ | ✅ |
| Session persistence | ✅ | ❌ | ❌ | ❌ | ✅ |
| Crash recovery | ✅ | ❌ | ❌ | ❌ | ❌ |
| Tiered scrollback | ✅ | ❌ | ❌ | ❌ | ❌ |
| DRCS fonts | ✅ | ❌ | ❌ | ❌ | ❌ |
| VT52 mode | ✅ | ❌ | ❌ | ❌ | ❌ |
| Formal verification | ✅ | ❌ | ❌ | ❌ | ❌ |

### 6. Proof Summary Script - ✅ COMPLETE

**File:** `scripts/prove-best.sh`

**Purpose:** Run throughput benchmarks and print a short feature summary (with optional
memory and conformance runs enabled by uncommenting lines).

---

## Related Documents

- `docs/ROADMAP.md` - Phase 9 details
- `docs/DTERM-AI-DIRECTIVE.md` - Original optimization targets
- `research/alacritty_ANALYSIS.md` - Alacritty architecture reference
- `research/RESEARCH_INDEX.md` - Competitive analysis
